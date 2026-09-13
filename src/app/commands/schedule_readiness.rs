//! What one Gantt bar would actually execute, measured against the run the
//! project currently holds.
//!
//! This is the join between three things that are deliberately kept apart:
//! the authored bar (persistent, in [`crate::model::schedule`]), the
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
//!   produce a stated problem, so a bar can be short of an answer but
//!   never quietly short of tonnes.
//! - **Ground is counted once.** Two members that resolve to the same block
//!   are one block dug twice, whatever anchors they were captured with;
//!   captures refuse it, and this gate refuses what a saved file or a changed
//!   run still produces. Ground shared between two *different* bars - which
//!   copying one produces on purpose - is the dispatch evaluator's to refuse,
//!   because only it knows which bars are executable.
//! - **A figure that cannot be tonnes is refused, not rounded into one.** A
//!   negative or overflowed sum on the nominated tonne field, and a total
//!   that is not finite, each stop the bar with a named problem. A measured
//!   zero is an answer and passes.

use super::solids_view::{DigBlockRecord, MaterialState, PlanningSnapshot};
use crate::{
    i18n::tr,
    model::{
        Document, ReserveAggregation, ReserveFieldId,
        schedule::{BarId, DigBlockPick, DispatchAgent, DispatchBar, DispatchBlock, DispatchError, DispatchInput, sequence::BlockGround},
        solid_reserves::ReserveTotals,
    },
    ui::state::{ScheduleBarView, ScheduleMemberView, SequenceMemberView},
};

/// Why a bar cannot be turned into work yet.
///
/// A report can carry several: a project can be missing its tonnage field
/// *and* holding references the last run could not place, and fixing one
/// should not hide the other.
pub(crate) enum ReadinessProblem {
    /// The Solids run these blocks come from is not finished, is stale, or
    /// never happened. Carries the pipeline's own explanation.
    NoRun(String),
    /// The bar has no dig blocks in it.
    Empty,
    /// No reserve field has been nominated as tonnes.
    NoTonnageField,
    /// The nominated field is no longer in the project's Field List.
    TonnageFieldMissing,
    /// The nominated field is averaged or categorical, so summing it would
    /// not produce a tonnage.
    TonnageFieldNotSum(String),
    /// References the current run could not place, kept in the bar.
    Unresolved(usize),
    /// A block that resolved but has no complete measured tonnage.
    Unmeasured { block: String, reason: String },
    /// A block whose total is measured but incomplete.
    Partial { block: String },
    /// Two dig-order positions resolved to the same block, so the bar would
    /// dig it twice. `first`/`second` are positions from 1.
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

/// One member of a bar's dig order, as the current run sees it.
pub(crate) struct MemberReport {
    /// Its place in the dig order, from 1.
    pub(crate) position: usize,
    /// What the block is called in the Solids panels, when it was found.
    pub(crate) name: Option<String>,
    pub(crate) solid_name: Option<String>,
    /// Why it was not found, when it was not. The member is still in the
    /// dig order either way.
    pub(crate) unresolved: Option<String>,
    /// Its complete measured tonnage. `None` whenever that figure is not
    /// available *for any reason* - which is never the same as zero.
    pub(crate) tonnes: Option<f64>,
    /// Identity in the current run, used to detect two persistent references
    /// that resolve to the same occupied ground across different bars.
    pub(crate) resolved: Option<crate::model::DigBlockId>,
}

/// What one bar would execute.
pub(crate) struct BarReport {
    pub(crate) bar: BarId,
    pub(crate) members: Vec<MemberReport>,
    /// The bar's total tonnes, present only when every member resolved and
    /// every one of them carries a complete measured figure.
    pub(crate) tonnes: Option<f64>,
    pub(crate) problems: Vec<ReadinessProblem>,
    /// The run this was measured against, so a caller can tell that a report
    /// it is holding belongs to an older generation.
    pub(crate) generation: Option<u64>,
}

impl BarReport {
    pub(crate) fn is_ready(&self) -> bool {
        self.problems.is_empty() && self.tonnes.is_some()
    }

    pub(crate) fn unresolved_count(&self) -> usize {
        self.members.iter().filter(|member| member.unresolved.is_some()).count()
    }
}

/// One reason a schedule could not be calculated, and which bars it is about.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScheduleRunProblem {
    pub(crate) bars: Vec<BarId>,
    pub(crate) message: String,
}

/// The project's tonnage field, or what is wrong with the choice.
enum TonnageField {
    Chosen(ReserveFieldId),
    None,
    Missing,
    NotSum(String),
}

/// Join persistent assignments to one set of readiness reports and hand back
/// the entirely validated evaluator input, or every reason it cannot be built.
///
/// Nothing partial: a schedule assembled from the bars that happened to be
/// ready would draw calculated spans beside silently omitted work, which reads
/// as a complete answer and is not one.
///
/// `expected` is the Solids run the completed Schedule Setup run was validated
/// against. Reports measured against any other run are refused rather than
/// evaluated: the gate that let this be calculated named one run, and a
/// schedule assembled from two of them would describe ground that was never
/// all there at once.
pub(crate) fn dispatch_input(
    plan: &crate::model::schedule::SchedulePlan,
    reports: &[BarReport],
    expected: u64,
    horizon_limit_h: Option<f64>,
) -> Result<DispatchInput, Vec<ScheduleRunProblem>> {
    let mut problems = Vec::new();
    if plan.bars().is_empty() {
        problems.push(ScheduleRunProblem {
            bars: Vec::new(),
            message: tr!("schedule-run-no-bars"),
        });
        return Err(problems);
    }
    for bar in plan.bars() {
        if bar.agent.is_none() {
            problems.push(ScheduleRunProblem {
                bars: vec![bar.id],
                message: tr!("schedule-dispatch-unassigned", bar = bar.name().to_owned()),
            });
        }
    }
    for (bar, report) in plan.bars().iter().zip(reports) {
        if !report.is_ready() {
            problems.push(ScheduleRunProblem {
                bars: vec![bar.id],
                message: tr!("schedule-run-bar-not-ready", bar = bar.name().to_owned()),
            });
        }
    }
    let generations = reports
        .iter()
        .filter(|report| report.is_ready())
        .filter_map(|report| report.generation)
        .collect::<std::collections::HashSet<_>>();
    if reports.len() != plan.bars().len() || generations.len() > 1 || generations.iter().any(|generation| *generation != expected) {
        problems.push(ScheduleRunProblem {
            bars: plan.bars().iter().map(|bar| bar.id).collect(),
            message: tr!("schedule-dispatch-generation-changed"),
        });
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    let agents = plan
        .agents()
        .iter()
        .filter_map(|agent| plan.effective_rate_tph(agent.id).map(|rate_tph| DispatchAgent { agent: agent.id, rate_tph }))
        .collect();
    let bars = plan
        .bars()
        .iter()
        .zip(reports)
        .map(|(bar, report)| DispatchBar {
            bar: bar.id,
            agent: bar.agent.expect("checked above"),
            priority: bar.priority,
            window: bar.window,
            blocks: bar
                .members()
                .iter()
                .copied()
                .zip(&report.members)
                .map(|(block, member)| DispatchBlock {
                    block,
                    resolved: member.resolved.expect("ready members resolve"),
                    tonnes: member.tonnes.expect("ready members have tonnes"),
                })
                .collect(),
        })
        .collect();
    Ok(DispatchInput {
        generation: expected,
        horizon_limit_h,
        agents,
        bars,
    })
}

/// Turn one evaluator refusal into something the Gantt can say, against the
/// bars it is about.
pub(crate) fn dispatch_problem(plan: &crate::model::schedule::SchedulePlan, error: DispatchError) -> ScheduleRunProblem {
    match error {
        DispatchError::UnknownAgent { bar, .. }
        | DispatchError::InvalidWindow(bar)
        | DispatchError::EmptyBar(bar)
        | DispatchError::InvalidTonnes { bar, .. }
        | DispatchError::ClockDidNotAdvance(bar) => ScheduleRunProblem {
            bars: vec![bar],
            message: tr!("schedule-dispatch-invalid-input"),
        },
        DispatchError::DuplicateAgent(agent) | DispatchError::InvalidRate(agent) => ScheduleRunProblem {
            bars: plan.bars().iter().filter(|bar| bar.agent == Some(agent)).map(|bar| bar.id).collect(),
            message: tr!("schedule-dispatch-invalid-input"),
        },
        DispatchError::InconsistentTonnes { .. } | DispatchError::IterationCap => ScheduleRunProblem {
            bars: Vec::new(),
            message: tr!("schedule-dispatch-invalid-input"),
        },
    }
}

impl crate::app::App<'_> {
    /// Measure one bar against the current run.
    ///
    /// Returns a report in every case, including when there is no run at all:
    /// the dig order is the user's own work and stays legible whether or not
    /// Solids can currently say anything about it.
    #[allow(
        dead_code,
        reason = "the Gantt reads the mirrored reports; this single-bar entry is the checkpoint 4 dispatch evaluator's gate"
    )]
    pub(crate) fn bar_report(&self, id: BarId) -> Option<BarReport> {
        let document = self.workspace.active_document()?;
        let bar = document.schedule().bar(id)?;
        let snapshot = self.planning_snapshot();
        let run = snapshot.as_ref().map(|snapshot| CurrentRun {
            snapshot,
            ground: ground_of(snapshot),
        });
        Some(report_against(document, bar, run.as_ref().map_err(|reason| *reason)))
    }

    /// Every bar, measured. Used by the Gantt and, in checkpoint 4, by the
    /// dispatch evaluator's own gate.
    ///
    /// One snapshot serves them all: every report then describes the same
    /// generation, and the run is collected once however many bars the
    /// project holds.
    pub(crate) fn schedule_reports(&self) -> Vec<BarReport> {
        let Some(document) = self.workspace.active_document() else {
            return Vec::new();
        };
        if document.schedule().bars().is_empty() {
            return Vec::new();
        }
        let snapshot = self.planning_snapshot();
        let run = snapshot.as_ref().map(|snapshot| CurrentRun {
            snapshot,
            ground: ground_of(snapshot),
        });
        document
            .schedule()
            .bars()
            .iter()
            .map(|bar| report_against(document, bar, run.as_ref().map_err(|reason| *reason)))
            .collect()
    }

    /// Mirror the bar readiness reports into the editor state the Gantt reads.
    ///
    /// Computed only while the Gantt is on screen, and cleared when it leaves,
    /// so a report can never be read against a run it was not measured from.
    /// The dig order itself is not mirrored: it is persistent plan data,
    /// already in the project view.
    pub(crate) fn mirror_schedule_reports(&mut self) {
        if !self.editor.is_schedule_gantt() {
            if !self.editor.schedule_bar_reports.is_empty() {
                self.editor.schedule_bar_reports.clear();
                self.redraw_requested = true;
            }
            if self.editor.schedule_dispatch.take().is_some() {
                self.redraw_requested = true;
            }
            // The draft itself is kept: leaving the page puts the Solids
            // preview back the way it was, and coming back shows the same
            // editing session. Only the per-run answers go, because they
            // would otherwise outlive the run they were measured against.
            if !self.editor.sequence_members.is_empty() || self.editor.sequence_generation.is_some() || self.editor.sequence_unavailable.is_some() {
                self.editor.sequence_members.clear();
                self.editor.sequence_generation = None;
                self.editor.sequence_unavailable = None;
                self.redraw_requested = true;
            }
            return;
        }
        self.mirror_sequence_editor();
        let reports = self.schedule_reports();
        // Nothing is calculated here. Inspection is unconditional and the
        // readiness of each bar is a read; the timed schedule comes only from
        // an explicit Run, and what that run found is mirrored separately by
        // `mirror_schedule_calculation`.
        let dispatch_problems = self.schedule_run_problems.clone();
        let views = reports
            .into_iter()
            .map(|report| {
                let mut problems = report.problems.iter().map(|problem| problem.message()).collect::<Vec<_>>();
                problems.extend(
                    dispatch_problems
                        .iter()
                        .filter(|problem| problem.bars.contains(&report.bar))
                        .map(|problem| problem.message.clone()),
                );
                ScheduleBarView {
                    bar: report.bar,
                    ready: report.is_ready() && problems.is_empty(),
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
                    problems,
                }
            })
            .collect::<Vec<_>>();
        if self.editor.schedule_bar_reports != views {
            self.editor.schedule_bar_reports = views;
            self.redraw_requested = true;
        }
        self.mirror_schedule_calculation();
    }

    /// Mirror what the current run says about each member of the open
    /// sequence editor's draft.
    ///
    /// Kept apart from the bar reports above because the two describe
    /// different lists: a bar report describes the order the *project* holds,
    /// and this describes the order the user is drafting - which is exactly
    /// the list that has not been applied yet. Both are reads; neither starts
    /// work, and neither edits the draft.
    fn mirror_sequence_editor(&mut self) {
        use crate::ui::state::DraftMember;

        let Some(draft) = self.editor.sequence_editor.clone() else {
            if !self.editor.sequence_members.is_empty() {
                self.editor.sequence_members.clear();
                self.redraw_requested = true;
            }
            return;
        };
        let snapshot = self.planning_snapshot();
        let (generation, unavailable) = match &snapshot {
            Ok(snapshot) => (Some(snapshot.generation), None),
            Err(reason) => (None, Some(reason.describe())),
        };
        let members = match &snapshot {
            // No finished run: every member keeps its place and its number,
            // and none of them is described. An absent run is stated once, by
            // the window, rather than as a fault of every block in the list.
            Err(_) => draft
                .members
                .iter()
                .map(|member| SequenceMemberView {
                    name: None,
                    solid_name: None,
                    unresolved: None,
                    tonnes: None,
                    is_new: matches!(member, DraftMember::Picked(_)),
                    stale_pick: false,
                    anchor: None,
                    block: None,
                })
                .collect(),
            Ok(snapshot) => {
                let ground = ground_of(snapshot);
                // The same rule the readiness report applies: a field that
                // is gone, or no longer summed, produces no figure rather
                // than a wrong one.
                let field = self.workspace.active_document().and_then(|document| {
                    let id = document.schedule().tonnage_field()?;
                    document
                        .reserve_fields()
                        .iter()
                        .find(|field| field.id == id && field.aggregation == ReserveAggregation::Sum)
                        .map(|field| field.id)
                });
                draft
                    .members
                    .iter()
                    .map(|member| {
                        // A held reference is resolved exactly as a bar's own
                        // member is; a pick names a block of a run directly,
                        // and is stale when that run is no longer this one.
                        let (found, unresolved, stale_pick) = match member {
                            DraftMember::Held(reference) => {
                                let status = reference.resolve(&ground);
                                (status.resolved(), status.message(), false)
                            }
                            DraftMember::Picked(pick) => {
                                let stale = pick.generation != snapshot.generation;
                                let found = snapshot.blocks.iter().position(|block| block.id == pick.block).filter(|_| !stale);
                                (found, stale.then(|| tr!("sequence-pick-superseded")), stale)
                            }
                        };
                        let block = found.and_then(|index| snapshot.blocks.get(index));
                        SequenceMemberView {
                            name: block.map(|block| block.name.clone()),
                            solid_name: block.map(|block| block.solid_name.clone()),
                            unresolved,
                            tonnes: block.zip(field).and_then(|(block, field)| block_tonnes(block, field).ok()),
                            is_new: matches!(member, DraftMember::Picked(_)),
                            stale_pick,
                            // The flitch top, so the number floats on the
                            // block's own upper surface rather than inside it.
                            anchor: block.map(|block| [block.anchor[0], block.anchor[1], block.flitch.top]),
                            block: block.map(|block| block.id),
                        }
                    })
                    .collect()
            }
        };
        if self.editor.sequence_members != members || self.editor.sequence_generation != generation || self.editor.sequence_unavailable != unavailable {
            self.editor.sequence_members = members;
            self.editor.sequence_generation = generation;
            self.editor.sequence_unavailable = unavailable;
            self.redraw_requested = true;
        }
    }

    /// Take one click in the sequence editor's 3D view into the draft.
    ///
    /// Clicking a block the draft already holds selects it in the ordered
    /// list rather than adding it a second time: a dig order refuses
    /// duplicate ground, and a click that appeared to do nothing would read
    /// as a dead block. A miss clears the list selection, the way clicking
    /// empty space in the viewport clears a selection there.
    ///
    /// `generation` is the run the *click* was made against, carried from the
    /// pick request - never re-read here. The two are equal by the time this
    /// is reached (the consumption gate checked), but the pick records what it
    /// was picked from, and a pick found to name a different run is refused
    /// rather than relabelled: refreshing provenance to fit is exactly the
    /// silent repair the identity layer exists to prevent.
    pub(crate) fn pick_into_sequence_draft(&mut self, block: Option<crate::model::DigBlockId>, generation: u64) {
        let Some(block) = block else {
            if let Some(draft) = self.editor.sequence_editor.as_mut() {
                draft.selected = None;
            }
            return;
        };
        // Picking needs a finished run to name; without one there is no
        // generation to record, and a pick with no generation is exactly what
        // the identity layer exists to refuse.
        let snapshot = match self.planning_snapshot() {
            Ok(snapshot) => snapshot,
            Err(reason) => {
                crate::userspace_warn!("{}", reason.describe());
                return;
            }
        };
        if generation != snapshot.generation {
            crate::userspace_warn!("{}", tr!("sequence-pick-superseded"));
            return;
        }
        let Some(index) = snapshot.blocks.iter().position(|candidate| candidate.id == block) else {
            crate::userspace_warn!("{}", tr!("sequence-pick-unknown-block"));
            return;
        };
        let ground = ground_of(&snapshot);
        if let Some(draft) = self.editor.sequence_editor.as_mut() {
            append_or_select(draft, block, index, generation, &ground);
        }
        self.redraw_requested = true;
    }

    /// The ground a pick would store, for a block of the current run.
    ///
    /// The one place a [`crate::model::schedule::DigBlockRef`] is made, so
    /// what is captured and what is matched cannot drift apart. A capture is
    /// whole by construction - the source stamp, the band's top, the footprint
    /// digest and the volume are taken together, and the reference has no
    /// optional half to leave out. See [`Self::add_dig_block`] for the command
    /// boundary a pick has to pass through to get here.
    #[allow(dead_code, reason = "reached only through add_dig_block, the generation-checked pick boundary")]
    pub(crate) fn dig_block_reference(&self, block: &DigBlockRecord) -> crate::model::schedule::DigBlockRef {
        crate::model::schedule::DigBlockRef {
            solid: block.solid,
            source: block.source,
            flitch_base: block.flitch.base,
            flitch_top: block.flitch.top,
            anchor: block.anchor,
            plan_area: block.plan_area,
            footprint: crate::model::schedule::Footprint::of(&block.ground),
            volume: block.volume,
        }
    }

    /// Turn a pick - a block of *this* run - into the reference a bar's dig
    /// order stores, or refuse it.
    ///
    /// The pick names the run it saw; a pick landing after a rerun (or from a
    /// project that is no longer open) describes a block this run never
    /// produced, so it is refused rather than matched by id. This is the only
    /// door through which a reference enters a dig order, so the UI never
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
/// snapshot that produced it, so measuring many bars walks one collection of
/// the run rather than one each.
struct CurrentRun<'a> {
    snapshot: &'a PlanningSnapshot,
    ground: Vec<BlockGround>,
}

/// Measure one bar against the run the project currently holds, or against
/// nothing - in which case the pipeline's own explanation of why is carried as
/// the report's first problem.
fn report_against(document: &Document, bar: &crate::model::schedule::ScheduleBar, run: Result<&CurrentRun<'_>, &super::solids_view::PlanningNotReady>) -> BarReport {
    let mut report = BarReport {
        bar: bar.id,
        members: Vec::with_capacity(bar.members().len()),
        tonnes: None,
        problems: Vec::new(),
        generation: None,
    };
    if bar.members().is_empty() {
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
            report.members = bar
                .members()
                .iter()
                .enumerate()
                .map(|(index, _)| MemberReport {
                    position: index + 1,
                    name: None,
                    solid_name: None,
                    unresolved: None,
                    tonnes: None,
                    resolved: None,
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
    // saved dig order or a changed run can still produce it, and the report is
    // the last gate before a total becomes work.
    let mut claimed: Vec<(usize, usize)> = Vec::new();
    for (index, member) in bar.members().iter().enumerate() {
        let status = member.resolve(&run.ground);
        let Some(found) = status.resolved() else {
            report.members.push(MemberReport {
                position: index + 1,
                name: None,
                solid_name: None,
                unresolved: status.message(),
                tonnes: None,
                resolved: None,
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
            resolved: Some(block.id),
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
    report.tonnes = total.filter(|_| !bar.members().is_empty());
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

/// Take one gated pick into the draft: append it, or select the member that
/// already holds this ground.
///
/// Kept pure so the pick's own rules can be checked without a running
/// application: a completed pick appends exactly one member, carrying the run
/// it was made against, and selects - never duplicates - ground the draft
/// already holds, whatever anchor that ground was captured with.
pub(crate) fn append_or_select(draft: &mut crate::ui::state::SequenceDraft, block: crate::model::DigBlockId, index: usize, generation: u64, ground: &[BlockGround]) {
    use crate::ui::state::DraftMember;

    let existing = draft.members.iter().position(|member| match member {
        DraftMember::Held(reference) => reference.resolve(ground).resolved() == Some(index),
        DraftMember::Picked(pick) => pick.block == block && pick.generation == generation,
    });
    match existing {
        Some(position) => draft.selected = Some(position),
        None => {
            draft.members.push(DraftMember::Picked(DigBlockPick { block, generation }));
            draft.selected = Some(draft.members.len() - 1);
        }
    }
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
/// Why the Schedule Setup readiness step could not read one block's tonnage.
///
/// Told apart by *why* rather than by the problem's own shape, because the
/// step treats the two differently: a figure that is there and cannot be
/// tonnes is a broken project and blocks the step, while one that is simply
/// absent stops only the bars that reach that ground - and each of those says
/// so itself.
pub(crate) struct StageTonnage {
    pub(crate) message: String,
    pub(crate) invalid: bool,
}

/// One block's tonnage, for the Schedule Setup readiness step.
pub(crate) fn block_tonnes_for_stage(block: &DigBlockRecord, field: ReserveFieldId) -> Result<f64, StageTonnage> {
    block_tonnes(block, field).map_err(|problem| StageTonnage {
        invalid: matches!(problem, ReadinessProblem::InvalidTonnes { .. }),
        message: problem.message(),
    })
}

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
