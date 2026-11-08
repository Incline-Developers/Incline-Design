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

use glam::DVec3;
use rayon::prelude::*;

use super::{
    ReserveAggregation, ReserveField, ReserveFieldId, ReserveFieldIssue,
    block_model::{BlockBoundsSource, BlockValues as Values, ReserveFieldMapping, ReserveMappingSource},
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
const MEASURED_CHUNK: usize = 32_768;

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
    /// Blocks whose value was negative - a model's -99 for a blank, typically -
    /// and so were read as zero. A quantity of ground cannot be less than
    /// none, and one such block must not take tonnes away from the rest; an
    /// averaged grade reads it as a zero grade over its weight, the same way
    /// the schedule does. Counted among `contributions`, as the zero it was
    /// read as.
    pub(crate) negatives: u64,
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
            ReserveAggregation::WeightedAverage { .. } | ReserveAggregation::VolumeAverage => {
                (self.weight > 0.0 && self.weight.is_finite() && self.sum.is_finite()).then_some(self.sum / self.weight)
            }
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
                total.negatives += value.negatives;
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
    /// Per result slot, its portions in first-seen row order. Shared, so a
    /// later scan can keep the slots it did not have to measure again.
    pub(crate) portions: Vec<Arc<Vec<MaterialPortion>>>,
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

    /// The box all the partition's pieces lie in.
    fn reach(&self) -> (DVec3, DVec3) {
        let (mut min, mut max) = (DVec3::INFINITY, DVec3::NEG_INFINITY);
        for piece in self.bands.iter().flat_map(|band| &band.pieces) {
            let bounds = piece.mesh.bounds();
            min = min.min(DVec3::new(bounds.min.x, bounds.min.y, bounds.min.z));
            max = max.max(DVec3::new(bounds.max.x, bounds.max.y, bounds.max.z));
        }
        (min, max)
    }
}

/// The part of a block model one solid's reserves can ever read: every block
/// within reach of the solid's body, and the columns its mapping names.
///
/// A scan of any partition of that body over this part gives the same figures
/// as over the whole model, to the bit. Every piece cut from the body lies
/// within the body's bounds, so a block out of the body's reach is out of
/// every piece's, and a scan skips a block out of reach before it counts or
/// reads anything of it; the blocks it keeps keep their order, which is the
/// order the scan adds them up in. Kept so that measuring an edited solid
/// need not load the whole model.
pub(crate) struct ReserveSource {
    pub(crate) model: BlockModelData,
    pub(crate) blocks: Arc<BlockBoundsSource>,
}

impl ReserveSource {
    /// The columns `mapping` reads, by name, each once and in order.
    pub(crate) fn columns(mapping: &[ReserveFieldMapping]) -> Vec<String> {
        let mut names: Vec<String> = mapping
            .iter()
            .filter_map(|entry| match &entry.source {
                ReserveMappingSource::Column(name) | ReserveMappingSource::PerVolume(name) => Some(name.clone()),
                ReserveMappingSource::Constant(_) => None,
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// The box around `meshes`, a millimetre wider each way than their own
    /// bounds, so that no rounding in a later cut of them reaches past it.
    pub(crate) fn reach_of<'a>(meshes: impl IntoIterator<Item = &'a Triangulation>) -> Option<(DVec3, DVec3)> {
        let (mut min, mut max) = (DVec3::INFINITY, DVec3::NEG_INFINITY);
        for mesh in meshes {
            let bounds = mesh.bounds();
            min = min.min(DVec3::new(bounds.min.x, bounds.min.y, bounds.min.z));
            max = max.max(DVec3::new(bounds.max.x, bounds.max.y, bounds.max.z));
        }
        if !min.is_finite() || !max.is_finite() {
            return None;
        }
        let margin = |corner: DVec3| DVec3::splat(1e-3) + corner.abs() * 1e-9;
        Some((min - margin(min), max + margin(max)))
    }

    /// The part of `model` within `reach` that `mapping` reads; `None` when a
    /// column it names is declared but not in memory, or nothing is in reach.
    pub(crate) fn extract(model: &BlockModelData, blocks: &BlockBoundsSource, mapping: &[ReserveFieldMapping], reach: (DVec3, DVec3)) -> Option<Self> {
        if blocks.len() != model.metadata.n_blocks {
            return None;
        }
        let rows: Vec<usize> = (0..blocks.len())
            .into_par_iter()
            .filter(|&row| blocks.get(row).is_some_and(|bounds| !intersection::Block::out_of_reach(model, bounds, reach)))
            .collect();
        if rows.is_empty() {
            return None;
        }
        let part = model.rows_of(&rows, &Self::columns(mapping))?;
        Some(Self {
            model: part,
            blocks: Arc::new(BlockBoundsSource::Explicit(blocks.select(&rows))),
        })
    }
}

struct NumericField<'a> {
    id: ReserveFieldId,
    values: Values<'a>,
    weights: Option<Values<'a>>,
}
struct CategoryField<'a> {
    id: ReserveFieldId,
    values: Values<'a>,
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

/// What an earlier scan of the same model and fields measured, for the pieces of
/// a new partition that are the very meshes it measured. A piece's figures do
/// not depend on the pieces around it, so they are the ones a full scan would
/// give, to the bit, and only the changed ground has to be scanned.
pub(crate) struct KeptMeasures {
    /// Each kept slot in the new partition, and in the scan it comes from.
    pub(crate) slots: Vec<(usize, usize)>,
    pub(crate) totals: Arc<Vec<ReserveTotals>>,
    pub(crate) material: Option<Arc<MaterialCapture>>,
}

impl ReserveOutcome {
    /// Fill in the slots this scan was not asked to measure.
    pub(crate) fn keep(&mut self, kept: &KeptMeasures) {
        for &(slot, old) in &kept.slots {
            self.totals[slot] = kept.totals[old].clone();
        }
        // This scan's material joins the earlier scan's label table, as a
        // portion names its categories by their place in it.
        if let (Some(material), Some(earlier)) = (self.material.as_mut(), kept.material.as_ref()) {
            let mut labels = earlier.labels.clone();
            let moved: Vec<f64> = material
                .labels
                .iter()
                .map(|label| {
                    let place = labels.iter().position(|known| known == label).unwrap_or_else(|| {
                        labels.push(label.clone());
                        labels.len() - 1
                    });
                    place as f64
                })
                .collect();
            let categorical: Vec<usize> = (0..material.fields.len()).filter(|position| material.is_categorical(*position)).collect();
            for portions in &mut material.portions {
                if !categorical.is_empty() && !portions.is_empty() {
                    for portion in Arc::make_mut(portions) {
                        for &position in &categorical {
                            let value = portion.values[position];
                            if value.is_finite() {
                                portion.values[position] = moved[value as usize];
                            }
                        }
                    }
                }
            }
            material.labels = labels;
            for &(slot, old) in &kept.slots {
                material.portions[slot] = earlier.portions[old].clone();
            }
        }
        // "Every value is missing" was worked out from what this scan saw.
        self.resolution.issues.retain(|(_, issue)| !matches!(issue, ReserveFieldIssue::AllValuesMissing));
        let mut totals = self.totals.iter();
        let Some(first) = totals.next() else { return };
        let numeric: Vec<ReserveFieldId> = first.all.numeric.keys().copied().collect();
        for field in numeric {
            let tally = |pick: fn(&NumericTotal) -> u64| self.totals.iter().filter_map(|total| total.all.numeric.get(&field)).map(pick).sum::<u64>();
            if tally(|total| total.contributions) == 0 && tally(|total| total.missing) > 0 {
                self.resolution.issues.push((field, ReserveFieldIssue::AllValuesMissing));
            }
        }
        let empty = crate::i18n::tr!("planning-empty-category");
        let categories: Vec<ReserveFieldId> = first.categories.keys().copied().collect();
        for field in categories {
            let mut groups = self.totals.iter().filter_map(|total| total.categories.get(&field)).flat_map(BTreeMap::keys).peekable();
            if groups.peek().is_some() && groups.all(|label| *label == empty) {
                self.resolution.issues.push((field, ReserveFieldIssue::AllValuesMissing));
            }
        }
    }
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
    let categorical = fields.iter().any(|entry| entry.id == field && entry.aggregation == ReserveAggregation::Category);
    let name = match &entry.source {
        ReserveMappingSource::Constant(value) => {
            return if value.is_finite() { Ok(()) } else { Err(ReserveFieldIssue::ConstantNotFinite) };
        }
        // A category has no quantity to scale by volume.
        ReserveMappingSource::PerVolume(name) if categorical => {
            return Err(ReserveFieldIssue::WrongColumnKind {
                column: name.clone(),
                wanted_categorical: true,
            });
        }
        ReserveMappingSource::Column(name) | ReserveMappingSource::PerVolume(name) => name,
    };
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
            ReserveMappingSource::PerVolume(name) => crate::model::block_model::per_volume_values(model, blocks, name),
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
            ReserveAggregation::VolumeAverage => match crate::model::block_model::block_volumes(model, blocks) {
                Some(volumes) => numeric.push(NumericField {
                    id: field.id,
                    values,
                    weights: Some(volumes),
                }),
                None => note(&mut resolution, field.id, ReserveFieldIssue::WeightUnresolved),
            },
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
        portions: vec![Arc::default(); partition.slots],
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
    partition.bands.par_iter().flat_map(|band| &band.pieces).for_each(|piece| piece.spatial.ensure_built());
    let grids: Vec<PieceGrid> = partition.bands.par_iter().map(|band| PieceGrid::new(&band.pieces)).collect();
    let reach = partition.reach();
    let mut chunk_start = 0usize;
    while chunk_start < blocks.len() {
        anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
        progress.set_items(chunk_start as u64, blocks.len() as u64);
        let chunk_end = (chunk_start + MEASURED_CHUNK).min(blocks.len());
        // One scratch per worker rather than per block: the clipper's buffers
        // are the reason a block costs what it does.
        let measured: Vec<anyhow::Result<BlockOverlaps>> = (chunk_start..chunk_end)
            .into_par_iter()
            .map_init(
                || (intersection::Scratch::default(), Vec::new()),
                |(scratch, nearby), index| {
                    let bounds = blocks.get(index).ok_or_else(|| anyhow::anyhow!("Missing block geometry at {index}"))?;
                    if intersection::Block::out_of_reach(model, bounds, reach) {
                        return Ok(BlockOverlaps { volume: 0.0, hits: Vec::new() });
                    }
                    let block = intersection::Block::new(model, bounds)?;
                    // Bands are ordered, so the first that can reach this block is
                    // found by bisection and the scan stops as soon as one starts
                    // above it.
                    let first = partition.bands.partition_point(|band| band.top <= block.min.z);
                    let mut hits = Vec::new();
                    for (band, grid) in partition.bands[first..].iter().zip(&grids[first..]) {
                        if band.base >= block.max.z {
                            break;
                        }
                        grid.pieces_near(block.min.truncate(), block.max.truncate(), nearby);
                        for piece in nearby.iter().map(|&piece| &band.pieces[piece as usize]) {
                            // Said with where, so a bad piece can be found: the
                            // fraction alone names neither the piece nor the block.
                            let fraction = block.fraction(piece, scratch, cancel).map_err(|error| {
                                let (centre, size) = ((block.min + block.max) / 2.0, block.max - block.min);
                                error.context(format!(
                                    "in the piece from RL {:.2} to {:.2}, against the block centred at ({:.2}, {:.2}, RL {:.2}) of {:.2} x {:.2} x {:.2}",
                                    band.base, band.top, centre.x, centre.y, centre.z, size.x, size.y, size.z
                                ))
                            })?;
                            if fraction > 0.0 {
                                hits.push((piece.slot, fraction));
                            }
                        }
                    }
                    Ok(BlockOverlaps {
                        volume: block.world_volume(),
                        hits,
                    })
                },
            )
            .collect();

        for (offset, measured) in measured.into_iter().enumerate() {
            let index = chunk_start + offset;
            // Taken in index order, so a chunk whose blocks failed in several
            // places reports the same one the sequential scan reported.
            let BlockOverlaps { volume: block_volume, hits } = measured?;
            // Most of a model lies outside any one solid, and what is read
            // below for a block in it is the slowest part of a large model's scan.
            if hits.is_empty() {
                continue;
            }
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
        material.portions = slots.into_iter().map(|slot| Arc::new(slot.portions)).collect();
    }
    Ok(ReserveOutcome { totals, resolution, material })
}

/// A band's pieces by where they lie in plan, so a block is only measured
/// against those whose bounds could reach it. A band can hold thousands of
/// pieces and a model hundreds of millions of blocks.
struct PieceGrid {
    /// Every piece in order, when there are too few for cells to be worth it.
    all: Vec<u32>,
    min: glam::DVec2,
    cells_per_metre: glam::DVec2,
    dims: [usize; 2],
    /// Offsets into `pieces`, one per cell and one past the end.
    starts: Vec<u32>,
    pieces: Vec<u32>,
}

impl PieceGrid {
    const WORTH_CELLS: usize = 8;

    fn new(pieces: &[ReservePiece]) -> Self {
        let all: Vec<u32> = (0..pieces.len() as u32).collect();
        let mut grid = Self {
            all,
            min: glam::DVec2::ZERO,
            cells_per_metre: glam::DVec2::ZERO,
            dims: [0; 2],
            starts: Vec::new(),
            pieces: Vec::new(),
        };
        if pieces.len() < Self::WORTH_CELLS {
            return grid;
        }
        let plan = |piece: &ReservePiece| {
            let bounds = piece.mesh.bounds();
            (glam::DVec2::new(bounds.min.x, bounds.min.y), glam::DVec2::new(bounds.max.x, bounds.max.y))
        };
        let (mut min, mut max) = (glam::DVec2::INFINITY, glam::DVec2::NEG_INFINITY);
        for piece in pieces {
            let (low, high) = plan(piece);
            min = min.min(low);
            max = max.max(high);
        }
        let extent = max - min;
        if !extent.is_finite() || extent.min_element() <= 0.0 {
            return grid;
        }
        // About one cell a piece, shaped like the band.
        let side = (pieces.len() as f64 / (extent.x * extent.y)).sqrt();
        let dims = [((extent.x * side).ceil() as usize).max(1), ((extent.y * side).ceil() as usize).max(1)];
        grid.min = min;
        grid.cells_per_metre = glam::DVec2::new(dims[0] as f64, dims[1] as f64) / extent;
        grid.dims = dims;
        let mut counts = vec![0u32; dims[0] * dims[1] + 1];
        for piece in pieces {
            let (low, high) = plan(piece);
            grid.for_each_cell(low, high, |cell| counts[cell + 1] += 1);
        }
        for cell in 1..counts.len() {
            counts[cell] += counts[cell - 1];
        }
        grid.starts = counts.clone();
        grid.pieces = vec![0; *counts.last().unwrap_or(&0) as usize];
        // In piece order within each cell.
        for (index, piece) in pieces.iter().enumerate() {
            let (low, high) = plan(piece);
            let (starts, cells) = (&mut counts, &mut grid.pieces);
            Self::cells(grid.min, grid.cells_per_metre, grid.dims, low, high, |cell| {
                cells[starts[cell] as usize] = index as u32;
                starts[cell] += 1;
            });
        }
        grid
    }

    fn for_each_cell(&self, low: glam::DVec2, high: glam::DVec2, visit: impl FnMut(usize)) {
        Self::cells(self.min, self.cells_per_metre, self.dims, low, high, visit);
    }

    fn cells(min: glam::DVec2, cells_per_metre: glam::DVec2, dims: [usize; 2], low: glam::DVec2, high: glam::DVec2, mut visit: impl FnMut(usize)) {
        let at = |value: glam::DVec2| {
            let cell = (value - min) * cells_per_metre;
            [(cell.x.max(0.0) as usize).min(dims[0] - 1), (cell.y.max(0.0) as usize).min(dims[1] - 1)]
        };
        let ([x0, y0], [x1, y1]) = (at(low), at(high));
        for y in y0..=y1 {
            for x in x0..=x1 {
                visit(y * dims[0] + x);
            }
        }
    }

    /// The pieces whose bounds may reach this plan box, ascending, into `out`.
    fn pieces_near(&self, low: glam::DVec2, high: glam::DVec2, out: &mut Vec<u32>) {
        out.clear();
        if self.starts.is_empty() {
            out.extend_from_slice(&self.all);
            return;
        }
        let outside = high.x < self.min.x
            || high.y < self.min.y
            || low.x > self.min.x + self.dims[0] as f64 / self.cells_per_metre.x
            || low.y > self.min.y + self.dims[1] as f64 / self.cells_per_metre.y;
        if outside {
            return;
        }
        let mut cells = 0;
        self.for_each_cell(low, high, |cell| {
            out.extend_from_slice(&self.pieces[self.starts[cell] as usize..self.starts[cell + 1] as usize]);
            cells += 1;
        });
        if cells > 1 {
            out.sort_unstable();
            out.dedup();
        }
    }
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
fn add(group: &mut ReserveGroup, numeric: &[NumericField<'_>], index: usize, fraction: f64, block_volume: f64, mut tally: Option<(&mut Vec<bool>, &mut Counters)>) {
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
            None if value < 0.0 => {
                total.negatives += 1;
                total.contributions += 1;
            }
            None => {
                total.sum += value * fraction;
                total.contributions += 1;
            }
            Some(weights) => {
                let weight = weights.at(index) * fraction;
                if weight.is_finite() && weight > 0.0 {
                    if value < 0.0 {
                        total.negatives += 1;
                    } else {
                        total.sum += value * weight;
                    }
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
