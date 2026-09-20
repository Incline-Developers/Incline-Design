//! Volume-prorated reserves for closed planning slabs.
//!
//! The input contract is a *partition*, not a flat list of slabs. Planning
//! divides a solid vertically into bands - benches, then the flitches inside
//! them - and then divides each band laterally into blasts and dig blocks. The
//! vertical bands are ordered and never overlap, but two dig blocks in one
//! flitch share that band's elevations exactly. Flattening both levels into one
//! ordered list is impossible, which is why [`ReservePartition`] keeps them
//! apart: bands carry the ordering, pieces carry the lateral split.
mod intersection;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use rayon::prelude::*;

use super::{
    ReserveAggregation, ReserveField, ReserveFieldId, ReserveFieldIssue,
    block_model::{BlockBoundsSource, ReserveFieldMapping, ReserveMappingSource},
    formats::{block_model_data::BlockModelData, mesh_data::Triangulation},
    spatial::TriangleBvh,
};

/// How much of one block may be claimed across the whole partition before the
/// pieces are treated as genuinely overlapping rather than merely rounded.
///
/// A block is clipped independently against every piece it touches, so the
/// error grows with the number of clips rather than staying fixed. The budget
/// is therefore per touched piece, on top of a floor that covers a single
/// clip's own conditioning. Both are fractions of one block's volume; the
/// clipper itself works in block-local units, so they are scale free.
const OVERLAP_FLOOR: f64 = 1e-7;
const OVERLAP_PER_PIECE: f64 = 1e-9;

/// How many blocks are measured before their totals are added up.
///
/// Large enough that the threads have real work to divide and the join at the
/// end of each chunk costs nothing next to it; small enough that the
/// measurements waiting to be accumulated stay a couple of megabytes instead
/// of growing with the model.
const MEASURED_CHUNK: usize = 8192;

/// One block's overlaps with the partition, in the order a single-threaded
/// scan would have found them.
struct BlockOverlaps {
    volume: f64,
    hits: Vec<(usize, f64)>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct NumericTotal {
    pub(crate) sum: f64,
    pub(crate) weight: f64,
    /// Blocks that actually contributed a finite value to `sum`.
    ///
    /// The accumulator's own availability, carried at every scope it is kept
    /// at - piece, category and merged selection alike. Without it a field
    /// nothing contributed to is indistinguishable from one that genuinely
    /// summed to nothing, and a dash's reason computed for the whole solid
    /// cannot speak for one selected block.
    pub(crate) contributions: u64,
    /// Blocks in range whose value was not a finite number.
    pub(crate) missing: u64,
    /// Blocks in range whose weight was absent, zero or negative.
    pub(crate) unusable_weights: u64,
}
impl NumericTotal {
    /// The figure, or `None` when nothing measured contributed to it.
    ///
    /// A measured zero keeps its zero; an absent one stays absent. The two
    /// must never render alike, which is what returning `Some(0.0)` for an
    /// all-missing column did.
    pub(crate) fn value(&self, aggregation: ReserveAggregation) -> Option<f64> {
        if self.contributions == 0 {
            return None;
        }
        match aggregation {
            ReserveAggregation::Sum => self.sum.is_finite().then_some(self.sum),
            ReserveAggregation::WeightedAverage { .. } => (self.weight > 0.0 && self.weight.is_finite() && self.sum.is_finite()).then(|| self.sum / self.weight),
            ReserveAggregation::Category => None,
        }
    }

    /// Whether some blocks in range contributed and others did not, so a
    /// figure can be shown as a partial one rather than a complete total.
    pub(crate) fn is_partial(&self) -> bool {
        self.contributions > 0 && (self.missing > 0 || self.unusable_weights > 0)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ReserveGroup {
    pub(crate) equivalent_blocks: f64,
    /// Cubic metres of block-model ground measured into this group. Kept
    /// apart from the piece's own geometric volume so a solid that the model
    /// only partly covers cannot read as fully measured.
    pub(crate) covered_volume: f64,
    pub(crate) numeric: HashMap<ReserveFieldId, NumericTotal>,
}
impl ReserveGroup {
    fn merge(&mut self, other: &Self) {
        self.equivalent_blocks += other.equivalent_blocks;
        self.covered_volume += other.covered_volume;
        // A missing mapping must not masquerade as a complete selected total.
        self.numeric.retain(|id, total| {
            if let Some(value) = other.numeric.get(id) {
                total.sum += value.sum;
                total.weight += value.weight;
                total.contributions += value.contributions;
                total.missing += value.missing;
                total.unusable_weights += value.unusable_weights;
                true
            } else {
                false
            }
        });
    }
}
#[derive(Clone, Debug, Default)]
pub(crate) struct ReserveTotals {
    pub(crate) all: ReserveGroup,
    pub(crate) categories: HashMap<ReserveFieldId, BTreeMap<String, ReserveGroup>>,
}
impl ReserveTotals {
    pub(crate) fn merge(&mut self, other: &Self) {
        self.all.merge(&other.all);
        self.categories.retain(|id, groups| {
            let Some(incoming) = other.categories.get(id) else {
                return false;
            };
            for (name, group) in incoming {
                if let Some(existing) = groups.get_mut(name) {
                    existing.merge(group);
                } else {
                    groups.insert(name.clone(), group.clone());
                }
            }
            true
        });
    }
}

/// One field whose per-row mapped values a scan retained, and how to read them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CapturedField {
    pub(crate) field: ReserveFieldId,
    /// A categorical field's captured value is an index into
    /// [`MaterialCapture::labels`]; a numerical one's is the mapped number.
    pub(crate) categorical: bool,
}

/// One group of contributing block-model rows that agree on every captured
/// value, and how much of them a piece holds.
///
/// The *row's own* values, before any spatial proration - which is the whole
/// point of capturing them. A dig block's average grade cannot say whether a
/// particular category and a particular grade occur in the same material, and
/// the marginal category breakdowns in [`ReserveTotals`] cannot either. This
/// can: every value here was read off one row.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MaterialPortion {
    /// One value per entry of [`MaterialCapture::fields`], in that order. A
    /// value that was missing or not finite is `NaN` and fails every test - it
    /// is never read as zero.
    pub(crate) values: Vec<f64>,
    /// The intersection fractions of the rows in this group, summed. Tonnes are
    /// deliberately *not* stored: multiplying this by the group's own mapped
    /// value of whichever field a schedule reads as tonnes gives them, so the
    /// capture owes nothing to a schedule's configuration and does not have to
    /// be recomputed when that choice changes.
    pub(crate) fraction: f64,
}

/// Every piece's material composition, as one scan measured it.
///
/// Grouped by identical captured values, so a project whose rules turn on a
/// rock type keeps one portion per rock type per dig block rather than one per
/// contributing row. A project whose fields are all continuous grades collapses
/// nothing, which is correct and is bounded by the intersections the scan
/// already walked.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MaterialCapture {
    pub(crate) fields: Vec<CapturedField>,
    /// Category labels, referenced by index from a portion's values.
    pub(crate) labels: Vec<String>,
    /// Per result slot, its portions in first-seen row order.
    pub(crate) portions: Vec<Vec<MaterialPortion>>,
}

impl MaterialCapture {
    /// Where one field's value sits in every portion's value list.
    pub(crate) fn position(&self, field: ReserveFieldId) -> Option<usize> {
        self.fields.iter().position(|entry| entry.field == field)
    }

    pub(crate) fn is_categorical(&self, position: usize) -> bool {
        self.fields.get(position).is_some_and(|entry| entry.categorical)
    }

    /// The label a captured categorical value names, or `None` when the value
    /// was missing or names no label.
    pub(crate) fn label(&self, value: f64) -> Option<&str> {
        if !value.is_finite() || value < 0.0 {
            return None;
        }
        self.labels.get(value as usize).map(String::as_str)
    }
}

/// Groups being accumulated for one slot while a scan runs.
#[derive(Default)]
struct SlotPortions {
    /// Value tuples as raw bits, so two rows that agree are one group and a
    /// `NaN` groups with other `NaN`s rather than with nothing.
    index: HashMap<Vec<u64>, usize>,
    portions: Vec<MaterialPortion>,
}

/// One closed piece of ground within a band: a whole flitch, a blast's share
/// of one, or a single dig block.
pub(crate) struct ReservePiece {
    pub(crate) mesh: Arc<Triangulation>,
    pub(crate) spatial: Arc<TriangleBvh>,
    /// Where this piece's totals belong in the caller's own result list.
    /// Several pieces may share a slot - a dig block split in two by a hole
    /// is still one dig block - and their totals then add together.
    pub(crate) slot: usize,
}

/// One vertical band of a solid, and the pieces it is laterally divided into.
pub(crate) struct ReserveBand {
    pub(crate) base: f64,
    pub(crate) top: f64,
    pub(crate) pieces: Vec<ReservePiece>,
}

/// A whole solid's partition: ordered bands, bottom up, each cut laterally.
pub(crate) struct ReservePartition {
    pub(crate) bands: Vec<ReserveBand>,
    /// How many result slots to return. Slots with no piece come back empty
    /// rather than missing, so the caller's own indexing still lines up.
    pub(crate) slots: usize,
}

impl ReservePartition {
    /// Bands must be finite, non-degenerate and disjoint, bottom up; every
    /// piece must name a slot that exists. Checked once before the scan so a
    /// malformed partition is a reported error rather than a wrong number.
    fn validate(&self) -> anyhow::Result<()> {
        for band in &self.bands {
            anyhow::ensure!(
                band.base.is_finite() && band.top.is_finite() && band.base < band.top,
                "Reserve band {}–{} is not a finite, non-empty elevation range",
                band.base,
                band.top
            );
            anyhow::ensure!(
                band.pieces.iter().all(|piece| piece.slot < self.slots),
                "Reserve piece names a result slot outside the partition"
            );
        }
        anyhow::ensure!(
            self.bands.windows(2).all(|pair| pair[0].top <= pair[1].base + 1e-9),
            "Reserve bands must be ordered bottom up and must not overlap"
        );
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.bands.iter().all(|band| band.pieces.is_empty())
    }
}

enum Values {
    Column(Arc<Vec<f64>>),
    Constant(f64),
}
impl Values {
    fn at(&self, index: usize) -> f64 {
        match self {
            Self::Column(values) => values[index],
            Self::Constant(value) => *value,
        }
    }
}
struct NumericField {
    id: ReserveFieldId,
    values: Values,
    weights: Option<Values>,
}
struct CategoryField {
    id: ReserveFieldId,
    values: Values,
    labels: BTreeMap<u32, String>,
}

/// Why each field of the project's list did or did not reach the totals.
///
/// Carried beside the numbers so the panels can say what a dash means instead
/// of drawing one for every absent value alike.
#[derive(Clone, Debug, Default)]
pub(crate) struct ReserveResolution {
    /// Fields that produced no accumulator at all, and why.
    pub(crate) issues: Vec<(ReserveFieldId, ReserveFieldIssue)>,
    /// Blocks whose mapped value was not a finite number, per field.
    pub(crate) missing_values: HashMap<ReserveFieldId, u64>,
    /// Blocks whose weight was absent or not positive, per weighted field.
    pub(crate) unusable_weights: HashMap<ReserveFieldId, u64>,
    /// Category values with no label in the model's own dictionary.
    pub(crate) unmapped_labels: HashMap<ReserveFieldId, u64>,
}

impl ReserveResolution {
    pub(crate) fn issue(&self, field: ReserveFieldId) -> Option<&ReserveFieldIssue> {
        self.issues.iter().find(|(id, _)| *id == field).map(|(_, issue)| issue)
    }
}

#[derive(Debug)]
pub(crate) struct ReserveOutcome {
    pub(crate) totals: Vec<ReserveTotals>,
    pub(crate) resolution: ReserveResolution,
    /// What each piece is made of, when the scan was asked to retain it.
    pub(crate) material: Option<MaterialCapture>,
}

/// Resolve one field's mapping onto a model's own columns, reporting why it
/// could not be resolved rather than dropping it silently.
///
/// Shared with the Block Models step's whole-model statistics so a field that
/// is dashed there is dashed for the same stated reason here.
pub(crate) fn resolve_mapping(
    model: &BlockModelData,
    fields: &[ReserveField],
    mapping: &[ReserveFieldMapping],
    field: ReserveFieldId,
    blocks: usize,
) -> Result<(), ReserveFieldIssue> {
    let Some(entry) = mapping.iter().find(|entry| entry.field == field) else {
        return Err(ReserveFieldIssue::Unmapped);
    };
    let name = match &entry.source {
        ReserveMappingSource::Constant(value) => {
            return if value.is_finite() { Ok(()) } else { Err(ReserveFieldIssue::ConstantNotFinite) };
        }
        ReserveMappingSource::Column(name) => name,
    };
    let categorical = fields.iter().any(|entry| entry.id == field && entry.aggregation == ReserveAggregation::Category);
    let declared = if categorical { model.categorical_variables() } else { model.numeric_variables() };
    if !declared.iter().any(|variable| variable.name == *name) {
        return Err(if model.variable(name).is_some() {
            ReserveFieldIssue::WrongColumnKind {
                column: name.clone(),
                wanted_categorical: categorical,
            }
        } else {
            ReserveFieldIssue::MissingColumn(name.clone())
        });
    }
    let Some(values) = model.shared_numeric_values(name) else {
        return Err(ReserveFieldIssue::ColumnNotResident(name.clone()));
    };
    if values.len() != blocks {
        return Err(ReserveFieldIssue::LengthMismatch {
            column: name.clone(),
            values: values.len(),
            blocks,
        });
    }
    Ok(())
}

/// The issue that stops a weighted average, if anything does.
fn weight_issue(fields: &[ReserveField], weight_field: ReserveFieldId) -> Option<ReserveFieldIssue> {
    match fields.iter().find(|field| field.id == weight_field) {
        None => Some(ReserveFieldIssue::WeightFieldMissing),
        Some(field) if field.aggregation != ReserveAggregation::Sum => Some(ReserveFieldIssue::WeightNotSummed(field.name.clone())),
        Some(_) => None,
    }
}

/// Compute one partition's reserves, and optionally retain what each piece is
/// made of.
///
/// `capture` asks for [`MaterialCapture`]: the per-row mapped values behind
/// every piece's totals, which is what destination routing needs and what the
/// totals themselves cannot reconstruct. It roughly doubles what one scan keeps,
/// so it is asked for by the scope that needs it - dig blocks - and not by the
/// bench scan beside it.
#[allow(
    clippy::too_many_arguments,
    reason = "one scan's whole input: the model, its geometry, the mapping, the schema, the partition, the capture flag, and the two job handles"
)]
pub(crate) fn compute(
    model: &BlockModelData,
    blocks: &BlockBoundsSource,
    mapping: &[ReserveFieldMapping],
    fields: &[ReserveField],
    partition: &ReservePartition,
    capture: bool,
    cancel: &crate::app::jobs::CancelFlag,
    progress: &super::progress::Progress,
) -> anyhow::Result<ReserveOutcome> {
    anyhow::ensure!(blocks.len() == model.metadata.n_blocks, "Block geometry and attribute counts disagree");
    partition.validate()?;

    let mut resolution = ReserveResolution::default();
    let note = |resolution: &mut ReserveResolution, id: ReserveFieldId, issue: ReserveFieldIssue| {
        if resolution.issue(id).is_none() {
            resolution.issues.push((id, issue));
        }
    };
    let resolve = |id: ReserveFieldId, resolution: &mut ReserveResolution| -> Option<Values> {
        if let Err(issue) = resolve_mapping(model, fields, mapping, id, blocks.len()) {
            note(resolution, id, issue);
            return None;
        }
        match &mapping.iter().find(|entry| entry.field == id)?.source {
            ReserveMappingSource::Constant(value) => Some(Values::Constant(*value)),
            ReserveMappingSource::Column(name) => model.shared_numeric_values(name).map(Values::Column),
        }
    };
    let mut numeric = Vec::new();
    let mut categories = Vec::new();
    for field in fields {
        let Some(values) = resolve(field.id, &mut resolution) else {
            continue;
        };
        match field.aggregation {
            ReserveAggregation::Sum => numeric.push(NumericField {
                id: field.id,
                values,
                weights: None,
            }),
            ReserveAggregation::WeightedAverage { weight_field } => {
                if let Some(issue) = weight_issue(fields, weight_field) {
                    note(&mut resolution, field.id, issue);
                    continue;
                }
                match resolve(weight_field, &mut resolution) {
                    Some(weights) => numeric.push(NumericField {
                        id: field.id,
                        values,
                        weights: Some(weights),
                    }),
                    // The weight's own reason is recorded against the weight
                    // field; this one fails because that one did.
                    None => note(&mut resolution, field.id, ReserveFieldIssue::WeightUnresolved),
                }
            }
            ReserveAggregation::Category => {
                let labels = mapping
                    .iter()
                    .find(|entry| entry.field == field.id)
                    .and_then(|entry| match &entry.source {
                        ReserveMappingSource::Column(name) => model.variable(name).map(|variable| variable.strings.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                categories.push(CategoryField { id: field.id, values, labels });
            }
        }
    }

    let empty = ReserveGroup {
        equivalent_blocks: 0.0,
        covered_volume: 0.0,
        numeric: numeric.iter().map(|field| (field.id, NumericTotal::default())).collect(),
    };
    let mut totals = vec![
        ReserveTotals {
            all: empty.clone(),
            categories: categories.iter().map(|field| (field.id, BTreeMap::new())).collect()
        };
        partition.slots
    ];
    // The captured value list: every numerical field the scan resolved, then
    // every categorical one, in the order they will be read back from.
    let captured_fields: Vec<CapturedField> = numeric
        .iter()
        .map(|field| CapturedField {
            field: field.id,
            categorical: false,
        })
        .chain(categories.iter().map(|field| CapturedField {
            field: field.id,
            categorical: true,
        }))
        .collect();
    let mut material = capture.then(|| MaterialCapture {
        fields: captured_fields.clone(),
        labels: Vec::new(),
        portions: vec![Vec::new(); partition.slots],
    });
    if partition.is_empty() {
        return Ok(ReserveOutcome { totals, resolution, material });
    }

    let mut counters = Counters::default();
    // Which fields have already had a missing value counted for the block
    // being scanned, so a block split across several pieces reports one
    // missing value rather than one per piece.
    let mut counted: Vec<bool> = vec![false; numeric.len()];
    let mut contributing_blocks = 0u64;
    // Portion accumulators, and the interned label table they index into. Kept
    // beside the totals rather than derived afterwards: the row values are only
    // in hand while the row is being accumulated.
    let mut slots: Vec<SlotPortions> = if capture {
        (0..partition.slots).map(|_| SlotPortions::default()).collect()
    } else {
        Vec::new()
    };
    let mut label_index: HashMap<String, usize> = HashMap::new();
    let mut row_values: Vec<f64> = vec![f64::NAN; captured_fields.len()];
    let mut row_key: Vec<u64> = vec![0; captured_fields.len()];

    // Measured in parallel, added up in block order.
    //
    // The clipping is effectively the whole cost of a run and every block's is
    // independent of every other's, but the totals cannot simply be summed per
    // thread: float addition is not associative, so a sum split across threads
    // is a different number, and one split differently on the next run is
    // different again - the same project would quietly stop agreeing with
    // itself. The threads therefore only measure. The accumulation below walks
    // the measurements in the order one thread would have produced them, so
    // the figures are bit for bit the ones this scan has always reported.
    let mut chunk_start = 0usize;
    while chunk_start < blocks.len() {
        anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
        progress.set_items(chunk_start as u64, blocks.len() as u64);
        let chunk_end = (chunk_start + MEASURED_CHUNK).min(blocks.len());
        // One scratch per worker rather than per block: the clipper's buffers
        // are the reason a block costs what it does.
        let measured: Vec<anyhow::Result<BlockOverlaps>> = (chunk_start..chunk_end)
            .into_par_iter()
            .map_init(intersection::Scratch::default, |scratch, index| {
                let bounds = blocks.get(index).ok_or_else(|| anyhow::anyhow!("Missing block geometry at {index}"))?;
                let block = intersection::Block::new(model, bounds)?;
                // Bands are ordered, so the first that can reach this block is
                // found by bisection and the scan stops as soon as one starts
                // above it.
                let first = partition.bands.partition_point(|band| band.top <= block.min.z);
                let mut hits = Vec::new();
                for band in &partition.bands[first..] {
                    if band.base >= block.max.z {
                        break;
                    }
                    for piece in &band.pieces {
                        let fraction = block.fraction(piece, scratch, cancel)?;
                        if fraction > 0.0 {
                            hits.push((piece.slot, fraction));
                        }
                    }
                }
                Ok(BlockOverlaps {
                    volume: block.world_volume(),
                    hits,
                })
            })
            .collect();

        for (offset, measured) in measured.into_iter().enumerate() {
            let index = chunk_start + offset;
            // Taken in index order, so a chunk whose blocks failed in several
            // places reports the same one the sequential scan reported.
            let BlockOverlaps { volume: block_volume, hits } = measured?;
            let mut assigned = 0.0;
            let mut touched = 0usize;
            counted.iter_mut().for_each(|flag| *flag = false);
            // This row's own mapped values, read once however many pieces it
            // touches. A categorical value is interned into a label index; a
            // numerical one is kept as it was mapped, and a missing or
            // non-finite one stays `NaN` so it fails every test.
            if capture {
                for (position, field) in numeric.iter().enumerate() {
                    row_values[position] = field.values.at(index);
                }
                for (offset, field) in categories.iter().enumerate() {
                    let position = numeric.len() + offset;
                    let value = field.values.at(index);
                    row_values[position] = match category_label(&field.labels, value) {
                        None => f64::NAN,
                        Some(label) => {
                            let next = label_index.len();
                            let slot = *label_index.entry(label.clone()).or_insert(next);
                            if slot == next
                                && let Some(material) = material.as_mut()
                            {
                                material.labels.push(label);
                            }
                            slot as f64
                        }
                    };
                }
                for (position, value) in row_values.iter().enumerate() {
                    row_key[position] = if value.is_nan() {
                        f64::NAN.to_bits()
                    } else if *value == 0.0 {
                        0.0_f64.to_bits()
                    } else {
                        value.to_bits()
                    };
                }
            }
            for (slot, fraction) in hits {
                touched += 1;
                assigned += fraction;
                anyhow::ensure!(
                    assigned <= 1.0 + OVERLAP_FLOOR + OVERLAP_PER_PIECE * touched as f64,
                    "Block overlap exceeds its volume across planning pieces; the partition overlaps itself"
                );
                if capture {
                    let accumulator = &mut slots[slot];
                    match accumulator.index.get(&row_key) {
                        Some(position) => accumulator.portions[*position].fraction += fraction,
                        None => {
                            accumulator.index.insert(row_key.clone(), accumulator.portions.len());
                            accumulator.portions.push(MaterialPortion {
                                values: row_values.clone(),
                                fraction,
                            });
                        }
                    }
                }
                let total = &mut totals[slot];
                add(&mut total.all, &numeric, index, fraction, block_volume, Some((&mut counted, &mut counters)));
                for field in &categories {
                    let value = field.values.at(index);
                    let first_touch = touched == 1;
                    let label = if !value.is_finite() {
                        if first_touch {
                            *counters.missing_values.entry(field.id).or_default() += 1;
                        }
                        crate::i18n::tr!("planning-empty-category")
                    } else if value.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&value) {
                        match field.labels.get(&(value as u32)) {
                            Some(label) => label.clone(),
                            None => {
                                if first_touch {
                                    *counters.unmapped_labels.entry(field.id).or_default() += 1;
                                }
                                value.to_string()
                            }
                        }
                    } else {
                        if first_touch {
                            *counters.unmapped_labels.entry(field.id).or_default() += 1;
                        }
                        value.to_string()
                    };
                    let group = total.categories.get_mut(&field.id).unwrap().entry(label).or_insert_with(|| empty.clone());
                    add(group, &numeric, index, fraction, block_volume, None);
                }
            }
            contributing_blocks += u64::from(touched > 0);
        }
        chunk_start = chunk_end;
    }
    progress.set_items(blocks.len() as u64, blocks.len() as u64);
    // A field every contributing block was missing is not a measured zero.
    if contributing_blocks > 0 {
        for field in &numeric {
            if counters.missing_values.get(&field.id).copied().unwrap_or(0) >= contributing_blocks {
                note(&mut resolution, field.id, ReserveFieldIssue::AllValuesMissing);
            }
        }
        for field in &categories {
            if counters.missing_values.get(&field.id).copied().unwrap_or(0) >= contributing_blocks {
                note(&mut resolution, field.id, ReserveFieldIssue::AllValuesMissing);
            }
        }
    }
    resolution.missing_values = counters.missing_values;
    resolution.unusable_weights = counters.unusable_weights;
    resolution.unmapped_labels = counters.unmapped_labels;
    if let Some(material) = material.as_mut() {
        material.portions = slots.into_iter().map(|slot| slot.portions).collect();
    }
    Ok(ReserveOutcome { totals, resolution, material })
}

/// The label one categorical value names, or `None` when there is no value.
///
/// A value that is not a finite number has no label here, which is *not* how
/// the totals treat it: there it joins an explicit empty-category group so the
/// breakdown adds up. A routing condition has to be able to fail on it, so the
/// capture keeps the absence rather than a name for it.
///
/// A finite value with no entry in the model's dictionary keeps its own number
/// as its label, exactly as the totals spell it, so the two agree on what a
/// value is called wherever there is one.
fn category_label(labels: &BTreeMap<u32, String>, value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    if value.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&value) {
        return Some(labels.get(&(value as u32)).cloned().unwrap_or_else(|| value.to_string()));
    }
    Some(value.to_string())
}

/// Per-block data-quality tallies gathered during one scan.
#[derive(Default)]
struct Counters {
    missing_values: HashMap<ReserveFieldId, u64>,
    unusable_weights: HashMap<ReserveFieldId, u64>,
    unmapped_labels: HashMap<ReserveFieldId, u64>,
}

/// Accumulate one block's share of every numeric field into a group.
///
/// `tally` is present only for the group that owns the block's counts - the
/// `all` group. Category sub-groups measure the same block again and would
/// otherwise double-count its missing values.
fn add(group: &mut ReserveGroup, numeric: &[NumericField], index: usize, fraction: f64, block_volume: f64, mut tally: Option<(&mut Vec<bool>, &mut Counters)>) {
    group.equivalent_blocks += fraction;
    group.covered_volume += fraction * block_volume;
    for (slot, field) in numeric.iter().enumerate() {
        let total = group.numeric.get_mut(&field.id).unwrap();
        let value = field.values.at(index);
        if !value.is_finite() {
            // Counted on the accumulator itself as well as in the run-wide
            // tally, so a selection of one piece can say what is missing from
            // that piece rather than from the solid it belongs to.
            total.missing += 1;
            if let Some((counted, counters)) = tally.as_mut()
                && !counted[slot]
            {
                counted[slot] = true;
                *counters.missing_values.entry(field.id).or_default() += 1;
            }
            continue;
        }
        match &field.weights {
            None => {
                total.sum += value * fraction;
                total.contributions += 1;
            }
            Some(weights) => {
                let weight = weights.at(index) * fraction;
                if weight.is_finite() && weight > 0.0 {
                    total.sum += value * weight;
                    total.weight += weight;
                    total.contributions += 1;
                } else {
                    total.unusable_weights += 1;
                    if let Some((counted, counters)) = tally.as_mut()
                        && !counted[slot]
                    {
                        counted[slot] = true;
                        *counters.unusable_weights.entry(field.id).or_default() += 1;
                    }
                }
            }
        }
    }
}
