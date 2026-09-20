//! What a stockpile holds: the authored opening inventory, the material
//! identities a calculation interns, and the lot arithmetic reclaim runs on.
//!
//! Three separate things live here, in that order, and the separation is the
//! point of the module.
//!
//! # Authored opening inventory
//!
//! What a stockpile already holds when the schedule starts, written by the
//! user. An ordered list of *lots*, oldest first, each holding one or more
//! *portions* of material - a portion being some tonnes with property values
//! on them. One portion is the ordinary case; several describe a lot that was
//! built out of mixed material without pretending it is several independently
//! reclaimable lots.
//!
//! Nothing here is inferred. A linked stockpile solid supplies identity and
//! geometry and says nothing about what is in it: a volume is not tonnes, and
//! a schedule that assumed a density would open with stock nobody put there.
//!
//! A portion's tonnes are the tonnes. The nominated tonnage field is not
//! entered a second time as a property - "how much is there" is asked once,
//! and asking again would let the two answers disagree.
//!
//! # Material identity
//!
//! A movement's material is named by a [`MaterialId`] into a
//! [`MaterialTable`] the calculation owns. Dug material's record comes from
//! the contributing block-model rows a scan retained; opening material's comes
//! from the authored portions. Both keep missing values *missing* - never
//! zero - and both keep their original provenance separately from wherever the
//! material happens to be being loaded from now, because a rule about ex-pit
//! ground must not match a reclaim of material that once came from that pit.
//!
//! Nothing in here is persisted. A material id belongs to one calculation and
//! means nothing in the next, so authored project data never holds one.
//!
//! # Lots
//!
//! Reclaim takes from one lot at a time and takes *all* of its portions
//! proportionally: a loader cannot pick the valuable half out of a lot. Which
//! lot is eligible is [`ReclaimOrder`]'s answer - the oldest released lot with
//! anything left, or the newest - and an exhausted lot reveals the next.
//!
//! Receipts to one stockpile within one model interval become one lot,
//! released at that interval's end. Grouping by interval rather than by
//! whatever segments an execution happened to produce is what makes the answer
//! independent of loader iteration order. Delivered material consumes the
//! stockpile's capacity immediately, before it is reclaimable: it is in the
//! pile either way.

#![allow(dead_code, reason = "pure horizon-model inputs prepared in Stage 3 and consumed when the optimiser is integrated")]

use serde::{Deserialize, Serialize};

use super::{DestinationId, DestinationKind, PortionValue, ScheduleError, ScheduleResult, checked_name, same_name};
use crate::{
    i18n::tr,
    model::{ReserveAggregation, ReserveField, ReserveFieldId, SolidId},
};

/// Tonnes below which a lot's balance is treated as gone, so floating-point
/// dust cannot leave a lot holding a millionth of a tonne and eligible for
/// ever. The same figure [`super::dispatch`] flushes ground balances at.
pub(crate) const LOT_EPSILON: f64 = 1e-9;

/// Which end of a stockpile's lots reclaim takes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum ReclaimOrder {
    /// First in, first out: the oldest released lot with anything left.
    #[default]
    Fifo,
    /// Last in, first out: the newest.
    Lifo,
}

impl ReclaimOrder {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Fifo => tr!("inventory-order-fifo"),
            Self::Lifo => tr!("inventory-order-lifo"),
        }
    }
}

/// Identity of one authored opening lot within its stockpile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct OpeningLotId(pub(crate) u64);

/// Identity of one authored portion within its stockpile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct OpeningPortionId(pub(crate) u64);

/// One authored property value on an opening portion.
///
/// A field with no entry is *missing*, which is a third state beside a number
/// and a label: it fails every condition and is never read as zero. That is
/// why absence is spelled by having no entry rather than by a value that
/// stands for nothing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum OpeningValue {
    Number(f64),
    Category(String),
}

impl OpeningValue {
    pub(crate) fn portion_value(&self) -> PortionValue {
        match self {
            Self::Number(value) if value.is_finite() => PortionValue::Number(*value),
            Self::Number(_) => PortionValue::Missing,
            Self::Category(label) => PortionValue::Category(label.clone()),
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            Self::Number(value) => value.is_finite(),
            Self::Category(label) => !label.trim().is_empty(),
        }
    }
}

/// One portion of an opening lot: some tonnes, and what they are made of.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OpeningPortion {
    pub(crate) id: OpeningPortionId,
    /// On the schedule's nominated tonnes basis. Finite and strictly positive:
    /// a portion of nothing is not a portion.
    pub(crate) tonnes_t: f64,
    /// One entry per field the user has filled in, at most one per field.
    #[serde(default)]
    pub(crate) values: Vec<(ReserveFieldId, OpeningValue)>,
}

impl OpeningPortion {
    pub(crate) fn value(&self, field: ReserveFieldId) -> Option<&OpeningValue> {
        self.values.iter().find(|(id, _)| *id == field).map(|(_, value)| value)
    }

    fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self
                .values
                .iter()
                .map(|(_, value)| match value {
                    OpeningValue::Number(_) => 0,
                    OpeningValue::Category(label) => label.len(),
                })
                .sum::<usize>()
    }
}

/// One lot of opening stock, in the pile's own oldest-to-newest order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OpeningLot {
    pub(crate) id: OpeningLotId,
    pub(crate) name: String,
    /// Never empty: a lot with no material is not stock.
    pub(crate) portions: Vec<OpeningPortion>,
}

impl OpeningLot {
    pub(crate) fn tonnes(&self) -> f64 {
        self.portions.iter().map(|portion| portion.tonnes_t).sum()
    }
}

/// One stockpile's reclaim settings and its opening stock.
///
/// Held against the destination rather than in a table of its own, so a
/// stockpile that is deleted or retyped leaves its stock behind the way it
/// leaves its capacity behind - the change is undoable, and settings thrown
/// away on the way out do not come back on the way in.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct StockpileInventory {
    pub(crate) order: ReclaimOrder,
    /// Oldest first. The order is what FIFO and LIFO read, so it is a `Vec`.
    pub(crate) lots: Vec<OpeningLot>,
    next_lot_id: u64,
    next_portion_id: u64,
}

fn checked_tonnes(value: f64) -> ScheduleResult<f64> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ScheduleError::InvalidLotTonnes);
    }
    Ok(value)
}

impl StockpileInventory {
    /// Whether this says nothing a default would not, so an untouched
    /// stockpile carries no stored inventory setting.
    pub(crate) fn is_pristine(&self) -> bool {
        self.order == ReclaimOrder::default() && self.lots.is_empty() && self.next_lot_id == 0 && self.next_portion_id == 0
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.order == ReclaimOrder::default() && self.lots.is_empty()
    }

    /// Count saved property values whose field is gone or whose kind changed.
    /// The entries stay authored and visible; this is a validation answer for
    /// future optimisation input preparation, never a repair or a widening.
    pub(crate) fn invalid_field_references(&self, fields: &[ReserveField]) -> usize {
        self.lots
            .iter()
            .flat_map(|lot| &lot.portions)
            .flat_map(|portion| &portion.values)
            .filter(|(id, value)| {
                let Some(field) = fields.iter().find(|field| field.id == *id) else {
                    return true;
                };
                matches!(value, OpeningValue::Category(_)) != (field.aggregation == ReserveAggregation::Category)
            })
            .count()
    }

    /// Everything in the pile at hour zero.
    pub(crate) fn opening_tonnes(&self) -> f64 {
        self.lots.iter().map(OpeningLot::tonnes).sum()
    }

    pub(crate) fn lot(&self, id: OpeningLotId) -> Option<&OpeningLot> {
        self.lots.iter().find(|lot| lot.id == id)
    }

    fn lot_mut(&mut self, id: OpeningLotId) -> ScheduleResult<&mut OpeningLot> {
        self.lots.iter_mut().find(|lot| lot.id == id).ok_or(ScheduleError::UnknownLot)
    }

    fn lot_name_taken(&self, name: &str, except: Option<OpeningLotId>) -> bool {
        self.lots.iter().any(|lot| Some(lot.id) != except && same_name(&lot.name, name))
    }

    fn allocate_lot(&mut self) -> ScheduleResult<OpeningLotId> {
        let id = OpeningLotId(self.next_lot_id);
        self.next_lot_id = self.next_lot_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        Ok(id)
    }

    fn allocate_portion(&mut self) -> ScheduleResult<OpeningPortionId> {
        let id = OpeningPortionId(self.next_portion_id);
        self.next_portion_id = self.next_portion_id.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        Ok(id)
    }

    /// Add a lot of `tonnes_t` at the newest end, with one portion carrying no
    /// property values yet.
    pub(crate) fn add_lot(&mut self, name: &str, tonnes_t: f64) -> ScheduleResult<OpeningLotId> {
        let name = checked_name(name)?;
        let tonnes_t = checked_tonnes(tonnes_t)?;
        if self.lot_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let id = self.allocate_lot()?;
        let portion = self.allocate_portion()?;
        self.lots.push(OpeningLot {
            id,
            name,
            portions: vec![OpeningPortion {
                id: portion,
                tonnes_t,
                values: Vec::new(),
            }],
        });
        Ok(id)
    }

    /// Copy a lot, portions included, directly after it. Fresh ids
    /// throughout: the copy is its own stock, not a second name for the same.
    pub(crate) fn duplicate_lot(&mut self, id: OpeningLotId, name: &str) -> ScheduleResult<OpeningLotId> {
        let name = checked_name(name)?;
        if self.lot_name_taken(&name, None) {
            return Err(ScheduleError::DuplicateName(name));
        }
        let position = self.lots.iter().position(|lot| lot.id == id).ok_or(ScheduleError::UnknownLot)?;
        let new_id = self.allocate_lot()?;
        let mut copy = self.lots[position].clone();
        copy.id = new_id;
        copy.name = name;
        for portion in &mut copy.portions {
            portion.id = self.allocate_portion()?;
        }
        self.lots.insert(position + 1, copy);
        Ok(new_id)
    }

    pub(crate) fn remove_lot(&mut self, id: OpeningLotId) -> ScheduleResult {
        let before = self.lots.len();
        self.lots.retain(|lot| lot.id != id);
        if self.lots.len() == before {
            return Err(ScheduleError::UnknownLot);
        }
        Ok(())
    }

    pub(crate) fn rename_lot(&mut self, id: OpeningLotId, name: &str) -> ScheduleResult {
        let name = checked_name(name)?;
        if self.lot_name_taken(&name, Some(id)) {
            return Err(ScheduleError::DuplicateName(name));
        }
        self.lot_mut(id)?.name = name;
        Ok(())
    }

    /// Move a lot one place towards the newest end, or towards the oldest.
    pub(crate) fn move_lot(&mut self, id: OpeningLotId, newer: bool) -> ScheduleResult {
        let position = self.lots.iter().position(|lot| lot.id == id).ok_or(ScheduleError::UnknownLot)?;
        let target = if newer { position + 1 } else { position.checked_sub(1).ok_or(ScheduleError::LotAtEnd)? };
        if target >= self.lots.len() {
            return Err(ScheduleError::LotAtEnd);
        }
        self.lots.swap(position, target);
        Ok(())
    }

    pub(crate) fn add_portion(&mut self, lot: OpeningLotId, tonnes_t: f64) -> ScheduleResult<OpeningPortionId> {
        let tonnes_t = checked_tonnes(tonnes_t)?;
        if self.lot(lot).is_none() {
            return Err(ScheduleError::UnknownLot);
        }
        let id = self.allocate_portion()?;
        self.lot_mut(lot)?.portions.push(OpeningPortion { id, tonnes_t, values: Vec::new() });
        Ok(id)
    }

    /// Remove one portion. The last portion of a lot is refused: an empty lot
    /// is not stock, and deleting the lot is the edit that says so.
    pub(crate) fn remove_portion(&mut self, lot: OpeningLotId, portion: OpeningPortionId) -> ScheduleResult {
        let entry = self.lot_mut(lot)?;
        if !entry.portions.iter().any(|held| held.id == portion) {
            return Err(ScheduleError::UnknownPortion);
        }
        if entry.portions.len() == 1 {
            return Err(ScheduleError::LastPortion);
        }
        entry.portions.retain(|held| held.id != portion);
        Ok(())
    }

    pub(crate) fn set_portion_tonnes(&mut self, lot: OpeningLotId, portion: OpeningPortionId, tonnes_t: f64) -> ScheduleResult {
        let tonnes_t = checked_tonnes(tonnes_t)?;
        let entry = self
            .lot_mut(lot)?
            .portions
            .iter_mut()
            .find(|held| held.id == portion)
            .ok_or(ScheduleError::UnknownPortion)?;
        entry.tonnes_t = tonnes_t;
        Ok(())
    }

    /// Set or clear one field's value on one portion. `None` clears it back to
    /// missing, which is not the same as setting it to zero.
    pub(crate) fn set_portion_value(&mut self, lot: OpeningLotId, portion: OpeningPortionId, field: ReserveFieldId, value: Option<OpeningValue>) -> ScheduleResult {
        if let Some(value) = &value
            && !value.is_valid()
        {
            return Err(ScheduleError::InvalidLotValue);
        }
        let entry = self
            .lot_mut(lot)?
            .portions
            .iter_mut()
            .find(|held| held.id == portion)
            .ok_or(ScheduleError::UnknownPortion)?;
        entry.values.retain(|(id, _)| *id != field);
        if let Some(value) = value {
            entry.values.push((field, value));
        }
        Ok(())
    }

    pub(crate) fn validate_loaded(&mut self) -> ScheduleResult {
        for (index, lot) in self.lots.iter().enumerate() {
            if self.lots[..index].iter().any(|earlier| earlier.id == lot.id) {
                return Err(ScheduleError::DuplicateId);
            }
            checked_name(&lot.name)?;
            if self.lots[..index].iter().any(|earlier| same_name(&earlier.name, &lot.name)) {
                return Err(ScheduleError::DuplicateName(lot.name.clone()));
            }
            if lot.portions.is_empty() {
                return Err(ScheduleError::LastPortion);
            }
            for (position, portion) in lot.portions.iter().enumerate() {
                if lot.portions[..position].iter().any(|earlier| earlier.id == portion.id) {
                    return Err(ScheduleError::DuplicateId);
                }
                checked_tonnes(portion.tonnes_t)?;
                for (offset, (field, value)) in portion.values.iter().enumerate() {
                    if portion.values[..offset].iter().any(|(earlier, _)| earlier == field) {
                        return Err(ScheduleError::DuplicateLotValue);
                    }
                    if !value.is_valid() {
                        return Err(ScheduleError::InvalidLotValue);
                    }
                }
            }
        }
        let highest_lot = self.lots.iter().map(|lot| lot.id.0).max();
        let highest_portion = self.lots.iter().flat_map(|lot| lot.portions.iter().map(|portion| portion.id.0)).max();
        for (counter, highest) in [(&mut self.next_lot_id, highest_lot), (&mut self.next_portion_id, highest_portion)] {
            if let Some(highest) = highest {
                *counter = (*counter).max(highest.checked_add(1).ok_or(ScheduleError::IdsExhausted)?);
            }
        }
        Ok(())
    }

    pub(crate) fn raise_allocator_to(&mut self, other: &Self) {
        self.next_lot_id = self.next_lot_id.max(other.next_lot_id);
        self.next_portion_id = self.next_portion_id.max(other.next_portion_id);
    }

    /// Fold the immutable inventory inputs an optimisation would read. Names
    /// are saved content but presentation only, and are hashed separately.
    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.order.hash(hasher);
        for lot in &self.lots {
            lot.id.hash(hasher);
            for portion in &lot.portions {
                portion.id.hash(hasher);
                portion.tonnes_t.to_bits().hash(hasher);
                for (field, value) in &portion.values {
                    field.hash(hasher);
                    match value {
                        OpeningValue::Number(number) => (0u8, number.to_bits()).hash(hasher),
                        OpeningValue::Category(label) => (1u8, label).hash(hasher),
                    }
                }
            }
        }
    }

    pub(crate) fn hash_names<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        for lot in &self.lots {
            lot.id.hash(hasher);
            lot.name.hash(hasher);
        }
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self
                .lots
                .iter()
                .map(|lot| size_of::<OpeningLot>() + lot.name.len() + lot.portions.iter().map(OpeningPortion::estimated_bytes).sum::<usize>())
                .sum::<usize>()
    }
}

/// Identity of one material record within one calculation.
///
/// Runtime only, and never written to a project: it is an index into a table
/// this calculation built, and the next calculation's table is its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct MaterialId(pub(crate) u32);

/// Where material originally came from, kept beside - never instead of -
/// wherever it is being loaded from now.
///
/// A reclaimed tonne's provenance is still the ground it was dug from, and
/// that is a reporting answer: matching reads the immediate source, so a rule
/// about digging one pit cannot pay twice when that material later leaves a
/// stockpile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MaterialProvenance {
    /// Cut from this solid, in these bands.
    Ground { solid: SolidId, bench: (f64, f64), flitch: (f64, f64) },
    /// Authored as opening stock of this stockpile.
    Opening { stockpile: DestinationId, lot: OpeningLotId },
}

/// One material's identity: where it came from and what it is made of.
///
/// The values are the material's own, as one contributing block-model row or
/// one authored portion carried them. Immutable once interned: a rule that
/// matched it is a fact about the rule, and never replaces the properties that
/// made it match.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MaterialRecord {
    pub(crate) provenance: MaterialProvenance,
    /// Ascending by field, so two records describing the same material intern
    /// to one id however their values were collected.
    pub(crate) values: Vec<(ReserveFieldId, PortionValue)>,
}

impl MaterialRecord {
    pub(crate) fn value(&self, field: ReserveFieldId) -> Option<PortionValue> {
        self.values.iter().find(|(id, _)| *id == field).map(|(_, value)| value.clone())
    }

    /// The key two records are the same material by. Written out rather than
    /// derived because the values hold `f64`s, and because a missing value must
    /// key differently from every number rather than alongside one.
    fn key(&self) -> String {
        let mut key = match self.provenance {
            MaterialProvenance::Ground { solid, bench, flitch } => {
                format!(
                    "g{}:{:x}:{:x}:{:x}:{:x}",
                    solid.0,
                    bench.0.to_bits(),
                    bench.1.to_bits(),
                    flitch.0.to_bits(),
                    flitch.1.to_bits()
                )
            }
            MaterialProvenance::Opening { stockpile, lot } => format!("o{stockpile:?}:{}", lot.0),
        };
        for (field, value) in &self.values {
            key.push('|');
            key.push_str(&field.0.to_string());
            key.push('=');
            match value {
                PortionValue::Number(number) => key.push_str(&format!("n{:x}", number.to_bits())),
                PortionValue::Category(label) => {
                    key.push('c');
                    key.push_str(label);
                }
                PortionValue::Missing => key.push('-'),
            }
        }
        key
    }
}

/// Every material one calculation knows about, interned once.
///
/// Movements and lot portions carry a [`MaterialId`] rather than a copy of the
/// property map: a pile receiving one rock type all week holds one record and
/// as many lots as it has intervals.
#[derive(Clone, Debug, Default)]
pub(crate) struct MaterialTable {
    records: Vec<MaterialRecord>,
    keys: std::collections::HashMap<String, MaterialId>,
}

impl MaterialTable {
    pub(crate) fn intern(&mut self, record: MaterialRecord) -> ScheduleResult<MaterialId> {
        let key = record.key();
        if let Some(id) = self.keys.get(&key) {
            return Ok(*id);
        }
        let id = MaterialId(u32::try_from(self.records.len()).map_err(|_| ScheduleError::IdsExhausted)?);
        self.records.push(record);
        self.keys.insert(key, id);
        Ok(id)
    }

    pub(crate) fn record(&self, id: MaterialId) -> Option<&MaterialRecord> {
        self.records.get(id.0 as usize)
    }

    /// What this material holds for one field, for a condition to test. An id
    /// this table does not know, and a field the material was not measured
    /// against, both read as missing - never as zero.
    pub(crate) fn value(&self, id: MaterialId, field: ReserveFieldId) -> Option<PortionValue> {
        self.record(id).and_then(|record| record.value(field))
    }

    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }
}

/// The model intervals a calculation groups receipts into.
///
/// An input, not a constant: nothing in this module knows how long an interval
/// is, so the horizon model can choose its own without any of the lot
/// arithmetic changing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct IntervalGrid {
    pub(crate) origin_h: f64,
    pub(crate) length_h: f64,
}

impl IntervalGrid {
    pub(crate) fn new(origin_h: f64, length_h: f64) -> ScheduleResult<Self> {
        if !origin_h.is_finite() || !length_h.is_finite() || length_h <= 0.0 {
            return Err(ScheduleError::InvalidInterval);
        }
        Ok(Self { origin_h, length_h })
    }

    /// Which interval an instant falls in. Start-inclusive and end-exclusive,
    /// like every other span in the schedule.
    pub(crate) fn index_of(&self, hour: f64) -> ScheduleResult<u64> {
        let offset = (hour - self.origin_h) / self.length_h;
        if !offset.is_finite() || offset < 0.0 {
            return Err(ScheduleError::InvalidInterval);
        }
        let index = offset.floor();
        if index > u64::MAX as f64 {
            return Err(ScheduleError::InvalidInterval);
        }
        Ok(index as u64)
    }

    /// When an interval ends, which is when what it received becomes
    /// reclaimable.
    pub(crate) fn end_of(&self, index: u64) -> f64 {
        self.origin_h + (index as f64 + 1.0) * self.length_h
    }
}

/// Identity of one lot within one calculation. Runtime only, like
/// [`MaterialId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct LotId(pub(crate) u32);

/// How a lot came to be in the pile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LotOrigin {
    /// Authored opening stock, at this place in the oldest-to-newest order.
    Opening { position: usize, lot: OpeningLotId },
    /// Everything this pile received during one model interval.
    Received { interval: u64 },
}

/// Some material, and how much of it, within a lot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LotPortion {
    pub(crate) material: MaterialId,
    pub(crate) tonnes: f64,
}

/// One lot in a pile, as a calculation holds it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Lot {
    pub(crate) id: LotId,
    pub(crate) origin: LotOrigin,
    /// When this lot becomes reclaimable. Opening stock is reclaimable from
    /// the start and carries `None`; a received lot is released at the end of
    /// the interval it was received in.
    pub(crate) released_h: Option<f64>,
    pub(crate) portions: Vec<LotPortion>,
    pub(crate) remaining_t: f64,
}

impl Lot {
    fn is_released(&self, now_h: f64) -> bool {
        self.released_h.is_none_or(|released| released <= now_h)
    }

    fn is_empty(&self) -> bool {
        self.remaining_t <= LOT_EPSILON
    }
}

/// One stockpile's lots, in release order.
///
/// Oldest first, always: opening stock in its authored order, then each
/// interval's receipts as that interval ends. Eligibility reads one end of
/// this list or the other, so the only thing [`ReclaimOrder`] changes is which
/// end.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StockpileState {
    pub(crate) stockpile: DestinationId,
    pub(crate) order: ReclaimOrder,
    /// Maximum stored tonnes, or `None` for unlimited.
    pub(crate) capacity_t: Option<f64>,
    lots: Vec<Lot>,
    next_lot: u32,
}

impl StockpileState {
    /// Seed a pile from its authored opening stock, interning the material as
    /// it goes.
    pub(crate) fn open(stockpile: DestinationId, inventory: &StockpileInventory, capacity_t: Option<f64>, table: &mut MaterialTable) -> ScheduleResult<Self> {
        let mut state = Self {
            stockpile,
            order: inventory.order,
            capacity_t,
            lots: Vec::with_capacity(inventory.lots.len()),
            next_lot: 0,
        };
        for (position, lot) in inventory.lots.iter().enumerate() {
            let mut portions = Vec::with_capacity(lot.portions.len());
            for portion in &lot.portions {
                let mut values: Vec<(ReserveFieldId, PortionValue)> = portion.values.iter().map(|(field, value)| (*field, value.portion_value())).collect();
                values.sort_by_key(|(field, _)| field.0);
                let material = table.intern(MaterialRecord {
                    provenance: MaterialProvenance::Opening { stockpile, lot: lot.id },
                    values,
                })?;
                portions.push(LotPortion {
                    material,
                    tonnes: checked_tonnes(portion.tonnes_t)?,
                });
            }
            let remaining_t: f64 = portions.iter().map(|portion| portion.tonnes).sum();
            if !remaining_t.is_finite() {
                return Err(ScheduleError::InvalidLotTonnes);
            }
            let id = state.allocate()?;
            state.lots.push(Lot {
                id,
                origin: LotOrigin::Opening { position, lot: lot.id },
                released_h: None,
                portions,
                remaining_t,
            });
        }
        if let Some(capacity) = capacity_t
            && state.stored_t() > capacity
        {
            return Err(ScheduleError::OpeningOverCapacity);
        }
        Ok(state)
    }

    fn allocate(&mut self) -> ScheduleResult<LotId> {
        let id = LotId(self.next_lot);
        self.next_lot = self.next_lot.checked_add(1).ok_or(ScheduleError::IdsExhausted)?;
        Ok(id)
    }

    /// Everything in the pile, released or not: delivered material occupies
    /// the space it was put in before anyone may take it out again.
    pub(crate) fn stored_t(&self) -> f64 {
        self.lots.iter().map(|lot| lot.remaining_t).sum()
    }

    /// How much more this pile can take, or `None` when it is unlimited.
    pub(crate) fn free_t(&self) -> Option<f64> {
        self.capacity_t.map(|capacity| (capacity - self.stored_t()).max(0.0))
    }

    pub(crate) fn lots(&self) -> &[Lot] {
        &self.lots
    }

    pub(crate) fn lot(&self, id: LotId) -> Option<&Lot> {
        self.lots.iter().find(|lot| lot.id == id)
    }

    /// Which lot may be reclaimed at this instant, or `None` when nothing
    /// released is left.
    ///
    /// An exhausted lot is skipped, which is how the next one appears without
    /// anything having to remove the empty one; a lot released later can take
    /// over LIFO eligibility at the next boundary, which is exactly what
    /// asking again at that boundary answers.
    pub(crate) fn eligible(&self, now_h: f64) -> Option<LotId> {
        let mut released = self.lots.iter().filter(|lot| lot.is_released(now_h) && !lot.is_empty());
        match self.order {
            ReclaimOrder::Fifo => released.next().map(|lot| lot.id),
            ReclaimOrder::Lifo => released.next_back().map(|lot| lot.id),
        }
    }

    /// Put material into the pile, grouped into this interval's lot.
    ///
    /// Everything one interval delivers to one pile is one lot, released when
    /// that interval ends - so which loader ran first, and how an execution
    /// happened to be split into segments, cannot change what a lot is.
    pub(crate) fn receive(&mut self, grid: IntervalGrid, interval: u64, portions: &[LotPortion]) -> ScheduleResult<LotId> {
        let added: f64 = portions.iter().map(|portion| portion.tonnes).sum();
        if !added.is_finite() || added < 0.0 {
            return Err(ScheduleError::InvalidLotTonnes);
        }
        if let Some(capacity) = self.capacity_t
            && self.stored_t() + added > capacity + LOT_EPSILON
        {
            return Err(ScheduleError::StockpileFull);
        }
        let released_h = grid.end_of(interval);
        let existing = self
            .lots
            .iter()
            .position(|lot| matches!(lot.origin, LotOrigin::Received { interval: held } if held == interval));
        let position = match existing {
            Some(position) => position,
            None => {
                let id = self.allocate()?;
                self.lots.push(Lot {
                    id,
                    origin: LotOrigin::Received { interval },
                    released_h: Some(released_h),
                    portions: Vec::new(),
                    remaining_t: 0.0,
                });
                self.lots.len() - 1
            }
        };
        let lot = &mut self.lots[position];
        for portion in portions {
            if portion.tonnes <= 0.0 {
                continue;
            }
            match lot.portions.iter_mut().find(|held| held.material == portion.material) {
                Some(held) => held.tonnes += portion.tonnes,
                None => lot.portions.push(*portion),
            }
            lot.remaining_t += portion.tonnes;
        }
        Ok(lot.id)
    }

    /// Take `tonnes` out of one lot, proportionally across every portion in
    /// it.
    ///
    /// All of it or none: a loader reclaims what the lot is made of, in the
    /// proportions the lot holds, and cannot take the valuable half. Several
    /// loaders on one lot share its balance, which is what makes asking for
    /// more than is left an error rather than a figure to be clamped - the
    /// caller decided how much to ask for and needs to know it was wrong.
    pub(crate) fn reclaim(&mut self, id: LotId, tonnes: f64) -> ScheduleResult<Vec<LotPortion>> {
        if !tonnes.is_finite() || tonnes < 0.0 {
            return Err(ScheduleError::InvalidLotTonnes);
        }
        let lot = self.lots.iter_mut().find(|lot| lot.id == id).ok_or(ScheduleError::UnknownLot)?;
        if tonnes > lot.remaining_t + LOT_EPSILON {
            return Err(ScheduleError::LotOverdrawn);
        }
        let taken = tonnes.min(lot.remaining_t);
        if lot.remaining_t <= 0.0 {
            return Ok(Vec::new());
        }
        let share = taken / lot.remaining_t;
        let mut removed = Vec::with_capacity(lot.portions.len());
        for portion in &mut lot.portions {
            let part = portion.tonnes * share;
            portion.tonnes -= part;
            removed.push(LotPortion {
                material: portion.material,
                tonnes: part,
            });
        }
        lot.remaining_t -= taken;
        // Flushed rather than left as dust, so an exhausted lot stops being
        // eligible instead of generating an event at every instant for ever.
        if lot.remaining_t <= LOT_EPSILON {
            lot.remaining_t = 0.0;
            for portion in &mut lot.portions {
                portion.tonnes = 0.0;
            }
        }
        Ok(removed)
    }
}

/// Whether a stockpile may reclaim to this kind of destination.
///
/// Crushers and dumps only for this milestone series. Stockpile-to-stockpile
/// reclaim is refused outright rather than limited: it is a transfer cycle,
/// and a horizon model that could move material between piles for free would
/// find that as an answer.
pub(crate) fn reclaim_destination_allowed(kind: DestinationKind) -> bool {
    match kind {
        DestinationKind::Crusher | DestinationKind::Dump => true,
        DestinationKind::Stockpile => false,
    }
}

/// Whether one reclaim candidate is a movement at all: a real destination, of
/// a kind reclaim may reach, that is not the pile the material is leaving.
pub(crate) fn reclaim_candidate_allowed(from: DestinationId, to: DestinationId, kind: DestinationKind) -> bool {
    from != to && reclaim_destination_allowed(kind)
}
