//! What a dig sequence would actually execute, measured against the run the
//! project currently holds.
//!
//! This is the join between three things that are deliberately kept apart:
//! the authored sequence (persistent, in [`crate::model::schedule`]), the
//! finished Solids run (session state, reached through
//! [`crate::app::commands::solids_view::PlanningSnapshot`]) and the project's
//! reserve schema. Nothing here computes geometry, starts a job or edits the
//! project: a report is a read, and asking for one can never change what is
//! being reported on.
//!
//! Four rules shape everything below:
//!
//! - **A reference that cannot be identified stays put and says why.** It is
//!   never dropped, never matched by the block's display name, and never
//!   rebound through a block's `replaces` lineage - "this ground came out of
//!   that ground" is not "this ground is that ground", and the tonnes differ.
//! - **A tonnage that is not there is not zero.** Capacity-only material,
//!   an unmeasured block, a field nothing maps onto and a partial total each
//!   produce a stated problem, so a sequence can be short of an answer but
//!   never quietly short of tonnes.
//! - **Ground is counted once.** Two members that resolve to the same block
//!   are one block dug twice, whatever anchors they were captured with;
//!   captures refuse it, and this gate refuses what a saved file or a changed
//!   run still produces.
//! - **A figure that cannot be tonnes is refused, not rounded into one.** A
//!   negative or overflowed sum on the nominated tonne field, and a total
//!   that is not finite, each stop the sequence with a named problem. A
//!   measured zero is an answer and passes.

use super::solids_view::{DigBlockRecord, MaterialState, PlanningSnapshot};
use crate::{
    i18n::tr,
    model::{
        Document, ReserveAggregation, ReserveFieldId,
        schedule::{DigBlockPick, SequenceId, sequence::BlockGround},
        solid_reserves::ReserveTotals,
    },
    ui::state::{ScheduleMemberView, ScheduleSequenceView},
};

/// Why a sequence cannot be turned into work yet.
///
/// A report can carry several: a project can be missing its tonnage field
/// *and* holding references the last run could not place, and fixing one
/// should not hide the other.
pub(crate) enum ReadinessProblem {
    /// The Solids run these blocks come from is not finished, is stale, or
    /// never happened. Carries the pipeline's own explanation.
    NoRun(String),
    /// The sequence has no dig blocks in it.
    Empty,
    /// No reserve field has been nominated as tonnes.
    NoTonnageField,
    /// The nominated field is no longer in the project's Field List.
    TonnageFieldMissing,
    /// The nominated field is averaged or categorical, so summing it would
    /// not produce a tonnage.
    TonnageFieldNotSum(String),
    /// References the current run could not place, kept in the sequence.
    Unresolved(usize),
    /// A block that resolved but has no complete measured tonnage.
    Unmeasured { block: String, reason: String },
    /// A block whose total is measured but incomplete.
    Partial { block: String },
    /// Two dig-order positions resolved to the same block, so the sequence
    /// would dig it twice. `first`/`second` are positions from 1.
    DuplicateGround { first: usize, second: usize, block: String },
    /// A block whose nominated tonne field holds a figure that cannot be
    /// tonnes: negative, or overflowed to a non-finite sum.
    InvalidTonnes { block: String, value: f64 },
    /// Every member's figure was sound, but their total is not a finite
    /// number.
    TotalNotFinite,
}

impl ReadinessProblem {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::NoRun(reason) => tr!("sequence-not-ready", reason = reason.clone()),
            Self::Empty => tr!("sequence-empty"),
            Self::NoTonnageField => tr!("sequence-no-tonnage-field"),
            Self::TonnageFieldMissing => tr!("sequence-tonnage-field-missing"),
            Self::TonnageFieldNotSum(field) => tr!("sequence-tonnage-field-not-sum", field = field.clone()),
            Self::Unresolved(count) => tr!("sequence-unresolved-count", count = count.to_string()),
            Self::Unmeasured { block, reason } => tr!("sequence-block-unmeasured", block = block.clone(), reason = reason.clone()),
            Self::Partial { block } => tr!("sequence-block-partial", block = block.clone()),
            Self::DuplicateGround { first, second, block } => {
                tr!("sequence-duplicate-ground", first = first.to_string(), second = second.to_string(), block = block.clone())
            }
            Self::InvalidTonnes { block, value } => tr!("sequence-invalid-tonnes", block = block.clone(), value = format!("{value}")),
            Self::TotalNotFinite => tr!("sequence-total-not-finite"),
        }
    }
}

/// One member of a sequence, as the current run sees it.
pub(crate) struct MemberReport {
    /// Its place in the dig order, from 1.
    pub(crate) position: usize,
    /// What the block is called in the Solids panels, when it was found.
    pub(crate) name: Option<String>,
    pub(crate) solid_name: Option<String>,
    /// Why it was not found, when it was not. The member is still in the
    /// sequence either way.
    pub(crate) unresolved: Option<String>,
    /// Its complete measured tonnage. `None` whenever that figure is not
    /// available *for any reason* - which is never the same as zero.
    pub(crate) tonnes: Option<f64>,
}

/// What one sequence would execute.
pub(crate) struct SequenceReport {
    pub(crate) sequence: SequenceId,
    pub(crate) members: Vec<MemberReport>,
    /// The sequence's total tonnes, present only when every member resolved
    /// and every one of them carries a complete measured figure.
    pub(crate) tonnes: Option<f64>,
    pub(crate) problems: Vec<ReadinessProblem>,
    /// The run this was measured against, so a caller can tell that a report
    /// it is holding belongs to an older generation.
    pub(crate) generation: Option<u64>,
}

impl SequenceReport {
    pub(crate) fn is_ready(&self) -> bool {
        self.problems.is_empty() && self.tonnes.is_some()
    }

    pub(crate) fn unresolved_count(&self) -> usize {
        self.members.iter().filter(|member| member.unresolved.is_some()).count()
    }
}

/// The project's tonnage field, or what is wrong with the choice.
enum TonnageField {
    Chosen(ReserveFieldId),
    None,
    Missing,
    NotSum(String),
}

impl crate::app::App<'_> {
    /// Measure one sequence against the current run.
    ///
    /// Returns a report in every case, including when there is no run at all:
    /// the dig order is the user's own work and stays legible whether or not
    /// Solids can currently say anything about it.
    #[allow(
        dead_code,
        reason = "the panels read the mirrored reports; this single-sequence entry is the dispatch evaluator's stage 2B gate"
    )]
    pub(crate) fn sequence_report(&self, id: SequenceId) -> Option<SequenceReport> {
        let document = self.workspace.active_document()?;
        let sequence = document.schedule().sequence(id)?;
        let snapshot = self.planning_snapshot();
        let run = snapshot.as_ref().map(|snapshot| CurrentRun {
            snapshot,
            ground: ground_of(snapshot),
        });
        Some(report_against(document, sequence, run.as_ref().map_err(|reason| *reason)))
    }

    /// Every sequence, measured. Used by the panels and, in stage 2B, by the
    /// dispatch evaluator's own gate.
    ///
    /// One snapshot serves them all: every report then describes the same
    /// generation, and the run is collected once however many sequences the
    /// project holds.
    pub(crate) fn schedule_reports(&self) -> Vec<SequenceReport> {
        let Some(document) = self.workspace.active_document() else {
            return Vec::new();
        };
        if document.schedule().sequences().is_empty() {
            return Vec::new();
        }
        let snapshot = self.planning_snapshot();
        let run = snapshot.as_ref().map(|snapshot| CurrentRun {
            snapshot,
            ground: ground_of(snapshot),
        });
        document
            .schedule()
            .sequences()
            .iter()
            .map(|sequence| report_against(document, sequence, run.as_ref().map_err(|reason| *reason)))
            .collect()
    }

    /// Mirror the sequence readiness reports into the editor state the
    /// Sequences step reads.
    ///
    /// Computed only while that step is on screen, and cleared when it leaves
    /// it, so a report can never be read against a run it was not measured
    /// from. The dig order itself is not mirrored: it is persistent plan data,
    /// already in the project view.
    pub(crate) fn mirror_schedule_reports(&mut self) {
        if !self.editor.is_schedule_sequences_step() {
            if !self.editor.schedule_sequence_reports.is_empty() {
                self.editor.schedule_sequence_reports.clear();
                self.redraw_requested = true;
            }
            return;
        }
        let views = self
            .schedule_reports()
            .into_iter()
            .map(|report| ScheduleSequenceView {
                sequence: report.sequence,
                ready: report.is_ready(),
                tonnes: report.tonnes,
                members: report
                    .members
                    .into_iter()
                    .map(|member| ScheduleMemberView {
                        position: member.position,
                        name: member.name,
                        solid_name: member.solid_name,
                        unresolved: member.unresolved,
                        tonnes: member.tonnes,
                    })
                    .collect(),
                problems: report.problems.iter().map(|problem| problem.message()).collect(),
            })
            .collect::<Vec<_>>();
        if self.editor.schedule_sequence_reports != views {
            self.editor.schedule_sequence_reports = views;
            self.redraw_requested = true;
        }
    }

    /// The ground a pick would store, for a block of the current run.
    ///
    /// The one place a [`crate::model::schedule::DigBlockRef`] is made, so
    /// what is captured and what is matched cannot drift apart. Captures are
    /// always complete - the source stamp, the band's top, the footprint
    /// digest and the volume are taken together - so nothing captured by this
    /// build can be an unverified reference. See [`Self::add_dig_block`] for
    /// the command boundary a pick has to pass through to get here.
    #[allow(dead_code, reason = "reached only through add_dig_block, the generation-checked pick boundary")]
    pub(crate) fn dig_block_reference(&self, block: &DigBlockRecord) -> crate::model::schedule::DigBlockRef {
        crate::model::schedule::DigBlockRef {
            solid: block.solid,
            source: Some(block.source),
            flitch_base: block.flitch.base,
            flitch_top: Some(block.flitch.top),
            anchor: block.anchor,
            plan_area: block.plan_area,
            footprint: Some(crate::model::schedule::Footprint::of(&block.ground)),
            volume: block.volume,
        }
    }

    /// Turn a pick - a block of *this* run - into the reference a sequence
    /// stores, or refuse it.
    ///
    /// The pick names the run it saw; a pick landing after a rerun (or from a
    /// project that is no longer open) describes a block this run never
    /// produced, so it is refused rather than matched by id. This is the only
    /// door through which a reference enters a sequence, so the UI never
    /// constructs or refreshes provenance itself.
    pub(crate) fn add_dig_block(&self, pick: &DigBlockPick) -> std::result::Result<crate::model::schedule::DigBlockRef, String> {
        let snapshot = self.planning_snapshot().map_err(|reason| reason.describe())?;
        if snapshot.generation != pick.generation {
            return Err(tr!("sequence-pick-stale", was = pick.generation.to_string(), now = snapshot.generation.to_string()));
        }
        let Some(block) = snapshot.blocks.iter().find(|block| block.id == pick.block) else {
            return Err(tr!("sequence-pick-unknown-block"));
        };
        Ok(self.dig_block_reference(block))
    }
}

/// One finished run, held in the shape resolving asks of it. Borrowed from the
/// snapshot that produced it, so measuring many sequences walks one collection
/// of the run rather than one each.
struct CurrentRun<'a> {
    snapshot: &'a PlanningSnapshot,
    ground: Vec<BlockGround>,
}

/// Measure one sequence against the run the project currently holds, or
/// against nothing - in which case the pipeline's own explanation of why is
/// carried as the report's first problem.
fn report_against(document: &Document, sequence: &crate::model::schedule::Sequence, run: Result<&CurrentRun<'_>, &super::solids_view::PlanningNotReady>) -> SequenceReport {
    let mut report = SequenceReport {
        sequence: sequence.id,
        members: Vec::with_capacity(sequence.members().len()),
        tonnes: None,
        problems: Vec::new(),
        generation: None,
    };
    if sequence.members().is_empty() {
        report.problems.push(ReadinessProblem::Empty);
    }

    let field = match document.schedule().tonnage_field() {
        None => TonnageField::None,
        Some(id) => match document.reserve_fields().iter().find(|field| field.id == id) {
            None => TonnageField::Missing,
            Some(field) if field.aggregation != ReserveAggregation::Sum => TonnageField::NotSum(field.name.clone()),
            Some(field) => TonnageField::Chosen(field.id),
        },
    };
    match &field {
        TonnageField::None => report.problems.push(ReadinessProblem::NoTonnageField),
        TonnageField::Missing => report.problems.push(ReadinessProblem::TonnageFieldMissing),
        TonnageField::NotSum(name) => report.problems.push(ReadinessProblem::TonnageFieldNotSum(name.clone())),
        TonnageField::Chosen(_) => {}
    }

    let run = match run {
        Ok(run) => run,
        Err(reason) => {
            report.problems.push(ReadinessProblem::NoRun(reason.describe()));
            report.members = sequence
                .members()
                .iter()
                .enumerate()
                .map(|(index, _)| MemberReport {
                    position: index + 1,
                    name: None,
                    solid_name: None,
                    unresolved: None,
                    tonnes: None,
                })
                .collect();
            return report;
        }
    };
    report.generation = Some(run.snapshot.generation);

    let mut total = Some(0.0);
    // Which block each resolved member landed on, by dig-order position: two
    // members that resolve to one block are one block dug twice, whatever
    // anchors they were captured with. Captures normalize this away, but a
    // saved sequence or a changed run can still produce it, and the report is
    // the last gate before a total becomes work.
    let mut claimed: Vec<(usize, usize)> = Vec::new();
    for (index, member) in sequence.members().iter().enumerate() {
        let status = member.resolve(&run.ground);
        let Some(found) = status.resolved() else {
            report.members.push(MemberReport {
                position: index + 1,
                name: None,
                solid_name: None,
                unresolved: status.message(),
                tonnes: None,
            });
            total = None;
            continue;
        };
        if let Some(&(first, _)) = claimed.iter().find(|&&(_, block)| block == found) {
            report.problems.push(ReadinessProblem::DuplicateGround {
                first: first + 1,
                second: index + 1,
                block: run.snapshot.blocks[found].name.clone(),
            });
            total = None;
        }
        claimed.push((index, found));
        let block = &run.snapshot.blocks[found];
        let tonnes = match &field {
            TonnageField::Chosen(field) => match block_tonnes(block, *field) {
                Ok(tonnes) => Some(tonnes),
                Err(problem) => {
                    report.problems.push(problem);
                    None
                }
            },
            // The figure cannot be read at all, which is already reported
            // once above; it is not a second fault of every block.
            _ => None,
        };
        if tonnes.is_none() {
            total = None;
        }
        total = total.zip(tonnes).map(|(running, tonnes)| running + tonnes);
        report.members.push(MemberReport {
            position: index + 1,
            name: Some(block.name.clone()),
            solid_name: Some(block.solid_name.clone()),
            unresolved: None,
            tonnes,
        });
    }
    let unresolved = report.unresolved_count();
    if unresolved > 0 {
        report.problems.push(ReadinessProblem::Unresolved(unresolved));
    }
    // Individually sound figures can still overflow when summed; a total
    // this build cannot hold is not a total.
    if total.is_some_and(|value| !value.is_finite()) {
        report.problems.push(ReadinessProblem::TotalNotFinite);
        total = None;
    }
    report.tonnes = total.filter(|_| !sequence.members().is_empty());
    report
}

/// The current run's ground, in the order its blocks are held, so a resolved
/// index addresses both lists. The footprint digest is computed once here,
/// not per member, so a report over many members hashes each block once.
fn ground_of(snapshot: &PlanningSnapshot) -> Vec<BlockGround> {
    snapshot
        .blocks
        .iter()
        .map(|block| BlockGround {
            solid: block.solid,
            source: block.source,
            flitch_base: block.flitch.base,
            flitch_top: block.flitch.top,
            outline: block.ground.clone(),
            footprint: crate::model::schedule::Footprint::of(&block.ground),
            volume: block.volume,
            plan_area: block.plan_area,
        })
        .collect()
}

/// One resolved block's complete measured tonnage, or why there is not one.
///
/// Every branch that cannot produce a figure produces a reason instead. None
/// of them produces zero: a block nothing was measured into and a block
/// measured as empty are different answers, and a schedule that read them
/// alike would dig the first one instantly. A figure this build refuses to
/// execute - a negative sum, or one that overflowed to a non-finite number -
/// is refused by name rather than passed on: the field was *nominated* as
/// tonnes, and tonnes are a quantity of ground.
fn block_tonnes(block: &DigBlockRecord, field: ReserveFieldId) -> Result<f64, ReadinessProblem> {
    let unmeasured = |reason: String| ReadinessProblem::Unmeasured {
        block: block.name.clone(),
        reason,
    };
    let totals: &ReserveTotals = match &block.material {
        MaterialState::Measured(totals) => totals,
        MaterialState::CapacityOnly => return Err(unmeasured(tr!("sequence-material-capacity-only"))),
        MaterialState::NoSchema => return Err(unmeasured(tr!("sequence-material-no-schema"))),
        MaterialState::Unavailable => return Err(unmeasured(tr!("sequence-material-unavailable"))),
    };
    let Some(total) = totals.all.numeric.get(&field) else {
        return Err(unmeasured(tr!("sequence-material-unmapped")));
    };
    if total.is_partial() {
        return Err(ReadinessProblem::Partial { block: block.name.clone() });
    }
    match total.value(ReserveAggregation::Sum) {
        // A measured zero stays a zero: it is an answer, not an absence.
        Some(tonnes) if tonnes >= 0.0 => Ok(tonnes),
        Some(tonnes) => Err(ReadinessProblem::InvalidTonnes {
            block: block.name.clone(),
            value: tonnes,
        }),
        // `value` rejects non-finite sums; reaching here with contributions
        // means finite parts summed to something this build cannot treat as
        // tonnes. Nothing contributed means nothing maps onto the field.
        None if total.contributions > 0 => Err(ReadinessProblem::InvalidTonnes {
            block: block.name.clone(),
            value: f64::INFINITY,
        }),
        None => Err(unmeasured(tr!("sequence-material-unmapped"))),
    }
}
