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

use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

use super::solids_view::{DigBlockRecord, MaterialState, PlanningSnapshot};
use crate::{
    i18n::{tr, tr_format},
    model::{
        Document, ReserveAggregation, ReserveFieldId,
        schedule::{
            BarId, DigBlockPick, DispatchAgent, DispatchBar, DispatchBlock, DispatchError, DispatchInput,
            sequence::{BlockGround, GroundIndex},
        },
        solid_reserves::ReserveTotals,
    },
    ui::state::{ScheduleBarView, ScheduleMemberView, SequenceMemberView},
};

fn block_labels(document: &Document, block: &DigBlockRecord) -> (String, String, String, String, String) {
    let solid_type = document
        .solid(block.solid)
        .map(|solid| crate::ui::dialogs::solids::kind_label(solid.kind))
        .unwrap_or_else(|| tr!(literal = "Solid"));
    // The same RL spelling the Solids tree beside these rows uses, so a
    // bench read in the navigation panel and the same bench read in the dig
    // order are recognisably the one bench. Bare: these are read in a row
    // that is nothing but elevations and names, where a unit on two of the
    // six fields is length rather than information.
    let rl = crate::ui::elements::solids_view::format_rl;
    let bench = rl(block.bench.base);
    let flitch = rl(block.flitch.base);
    let blast = block
        .blast
        .and_then(|reference| {
            document
                .solid(reference.solid)?
                .blasting
                .bench(reference.bench_base())?
                .blasts
                .iter()
                .find(|blast| blast.anchor == reference.anchor())
                .map(|blast| blast.name.clone())
        })
        .unwrap_or_else(|| tr!(literal = "Unblasted"));
    // The bar's own name when nothing was authored: pit, bench and blast,
    // which is how a dig area is spoken about. The bench drops its unit here
    // - the hyphens already say which field is which, and a marker is short.
    let area = tr_format!(
        literal = "%pit%-%bench%-%blast%",
        pit = block.solid_name.clone(),
        bench = rl(block.bench.base),
        blast = blast.clone()
    );
    (solid_type, bench, blast, flitch, area)
}

/// What an unnamed bar is called: the dig area it covers.
///
/// A bar usually holds one pit, bench and blast, and is named after it. One
/// that spans more says the first and how many others follow, rather than
/// listing them - a marker has room for a name, not an inventory. The order
/// is the dig order, so the leading area is the one dug first, and repeats
/// are counted once however many blocks of that area the bar holds.
fn default_bar_name<'a>(areas: impl Iterator<Item = &'a str>) -> String {
    let mut distinct: Vec<&str> = Vec::new();
    for area in areas {
        if !distinct.contains(&area) {
            distinct.push(area);
        }
    }
    match distinct.as_slice() {
        [] => tr!(literal = "Dig sequence"),
        [only] => (*only).to_owned(),
        [first, rest @ ..] => tr_format!(literal = "%area% (+%count%)", area = (*first).to_owned(), count = rest.len().to_string()),
    }
}

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
    pub(crate) solid_type: Option<String>,
    pub(crate) bench: Option<String>,
    pub(crate) blast: Option<String>,
    pub(crate) flitch: Option<String>,
    /// The pit/bench/blast identity used to derive an unnamed bar's label.
    pub(crate) area_name: Option<String>,
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

/// Scheduling ground and readiness derived for one exact set of inputs.
/// Shared by the Gantt, run preparation, and the sequence inspector so an
/// unchanged frame neither recollects blocks nor resolves references again.
pub(crate) struct ScheduleReportCache {
    key: u64,
    snapshot: Option<Arc<PlanningSnapshot>>,
    ground: Arc<Vec<BlockGround>>,
    index: Arc<GroundIndex>,
    unavailable: Option<String>,
    reports: Arc<Vec<BarReport>>,
    bar_views_key: Option<u64>,
    sequence_key: Option<u64>,
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

    let mut agents = Vec::with_capacity(plan.agents().len());
    for agent in plan.agents() {
        let Some(class) = plan.class(agent.class_id) else {
            problems.push(ScheduleRunProblem {
                bars: plan.bars().iter().filter(|bar| bar.agent == Some(agent.id)).map(|bar| bar.id).collect(),
                message: tr!("schedule-error-unknown-class"),
            });
            continue;
        };
        match agent.calendar.compile(class.default_dig_rate_tph) {
            Ok(calendar) => agents.push(DispatchAgent {
                agent: agent.id,
                rate_tph: class.default_dig_rate_tph,
                calendar,
            }),
            Err(error) => problems.push(ScheduleRunProblem {
                bars: plan.bars().iter().filter(|bar| bar.agent == Some(agent.id)).map(|bar| bar.id).collect(),
                message: error.message(),
            }),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
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
        let ground = snapshot.as_ref().ok().map(ground_of);
        let index = ground.as_deref().map(GroundIndex::build);
        let run = snapshot.as_ref().map(|snapshot| CurrentRun {
            snapshot,
            ground: ground.as_deref().expect("built for a finished snapshot"),
            index: index.as_ref().expect("built for a finished snapshot"),
        });
        Some(report_against(document, bar, run.as_ref().map_err(|reason| *reason)))
    }

    fn schedule_report_key(&self) -> u64 {
        let runtime = self.workspace.active_project().map_or(0, |project| project.runtime_id);
        let mut status_hasher = std::collections::hash_map::DefaultHasher::new();
        match self.planning_snapshot_status() {
            Ok(generation) => (0_u8, generation, String::new()).hash(&mut status_hasher),
            Err(reason) => (1_u8, 0_u64, reason.describe()).hash(&mut status_hasher),
        }
        let status_key = status_hasher.finish();
        let document_revision = self.workspace.active_document().map_or(0, Document::revision);
        if let Some((held_runtime, held_revision, held_status, key)) = self.schedule_report_key_cache.get()
            && (held_runtime, held_revision, held_status) == (runtime, document_revision, status_key)
        {
            return key;
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        runtime.hash(&mut hasher);
        // Conservatively include the document revision: reserve mappings and
        // model content can change measured tonnes without changing the
        // selected field's identity. Presentation-only edits may rebuild this
        // derived cache once, but still do not invalidate dispatch results.
        document_revision.hash(&mut hasher);
        status_key.hash(&mut hasher);
        let Some(document) = self.workspace.active_document() else {
            let key = hasher.finish();
            self.schedule_report_key_cache.set(Some((runtime, document_revision, status_key, key)));
            return key;
        };
        let plan = document.schedule();
        plan.tonnage_field().map(|field| field.0).hash(&mut hasher);
        if let Some(field) = plan.tonnage_field().and_then(|id| document.reserve_fields().iter().find(|field| field.id == id)) {
            field.id.0.hash(&mut hasher);
            field.name.hash(&mut hasher);
            format!("{:?}", field.aggregation).hash(&mut hasher);
        }
        for bar in plan.bars() {
            bar.id.0.hash(&mut hasher);
            for member in bar.members() {
                member.solid.hash(&mut hasher);
                member.source.hash(&mut hasher);
                member.flitch_base.to_bits().hash(&mut hasher);
                member.flitch_top.to_bits().hash(&mut hasher);
                member.anchor.map(f64::to_bits).hash(&mut hasher);
                member.plan_area.to_bits().hash(&mut hasher);
                member.footprint.hash(&mut hasher);
                member.volume.map(f64::to_bits).hash(&mut hasher);
            }
        }
        // These names feed resolved member labels and unnamed bar labels but
        // do not alter scheduling numerics. Keeping them in this presentation
        // key refreshes text without retiring a calculated result.
        for solid in document.solids() {
            solid.id.hash(&mut hasher);
            solid.name.hash(&mut hasher);
            (solid.kind as u8).hash(&mut hasher);
            for bench in &solid.blasting.benches {
                bench.base.to_bits().hash(&mut hasher);
                for blast in &bench.blasts {
                    blast.name.hash(&mut hasher);
                    blast.anchor.map(f64::to_bits).hash(&mut hasher);
                }
            }
        }
        let key = hasher.finish();
        self.schedule_report_key_cache.set(Some((runtime, document_revision, status_key, key)));
        key
    }

    fn ensure_schedule_report_cache(&mut self) {
        let key = self.schedule_report_key();
        if self.schedule_report_cache.as_ref().is_some_and(|cache| cache.key == key) {
            return;
        }
        let rebuild_started = web_time::Instant::now();
        let snapshot = self.planning_snapshot();
        let Some(document) = self.workspace.active_document() else {
            self.schedule_report_cache = Some(ScheduleReportCache {
                key,
                snapshot: None,
                ground: Arc::default(),
                index: Arc::new(GroundIndex::build(&[])),
                unavailable: snapshot.err().map(|reason| reason.describe()),
                reports: Arc::default(),
                bar_views_key: None,
                sequence_key: None,
            });
            log::debug!("schedule report cache rebuilt without a project in {:?}", rebuild_started.elapsed());
            return;
        };
        let (snapshot, ground, index, unavailable, reports) = match snapshot {
            Ok(snapshot) => {
                let snapshot = Arc::new(snapshot);
                let ground = Arc::new(ground_of(&snapshot));
                let index = Arc::new(GroundIndex::build(&ground));
                let run = CurrentRun {
                    snapshot: &snapshot,
                    ground: &ground,
                    index: &index,
                };
                let reports: Vec<BarReport> = document.schedule().bars().iter().map(|bar| report_against(document, bar, Ok(&run))).collect();
                (Some(snapshot), ground, index, None, Arc::new(reports))
            }
            Err(reason) => {
                let unavailable = reason.describe();
                let reports: Vec<BarReport> = document.schedule().bars().iter().map(|bar| report_against(document, bar, Err(&reason))).collect();
                (None, Arc::default(), Arc::new(GroundIndex::build(&[])), Some(unavailable), Arc::new(reports))
            }
        };
        let block_count = snapshot.as_ref().map_or(0, |snapshot| snapshot.blocks.len());
        let report_count = reports.len();
        self.schedule_report_cache = Some(ScheduleReportCache {
            key,
            snapshot,
            ground,
            index,
            unavailable,
            reports,
            bar_views_key: None,
            sequence_key: None,
        });
        log::debug!(
            "schedule report cache rebuilt {block_count} blocks / {report_count} bars in {:?}",
            rebuild_started.elapsed()
        );
    }

    /// Every bar measured against one cached, coherent ground snapshot.
    pub(crate) fn schedule_reports(&mut self) -> Arc<Vec<BarReport>> {
        self.ensure_schedule_report_cache();
        self.schedule_report_cache.as_ref().map(|cache| cache.reports.clone()).unwrap_or_default()
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
            // The Calendar reports per-period tonnes off the same held result,
            // so on that page the result stays mirrored - under the same
            // currentness gate - rather than being taken off the page.
            if self.editor.is_schedule_calendar() {
                self.mirror_schedule_calculation();
            } else {
                let dropped = self.editor.schedule_dispatch.take().is_some();
                let dropped_production = self.editor.schedule_production.take().is_some();
                if dropped || dropped_production {
                    self.redraw_requested = true;
                }
            }
            if let Some(cache) = self.schedule_report_cache.as_mut() {
                cache.bar_views_key = None;
                cache.sequence_key = None;
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
        let diagnostics_are_current = self
            .schedule_run_diagnostics
            .as_ref()
            .is_some_and(|diagnostics| self.schedule_run_inputs().ok() == Some(diagnostics.inputs) && self.schedule_plan_revision() == diagnostics.plan_revision);
        if !diagnostics_are_current {
            self.schedule_run_diagnostics = None;
        }
        let dispatch_problems: &[ScheduleRunProblem] = self.schedule_run_diagnostics.as_ref().map_or(&[], |diagnostics| &diagnostics.problems);
        let mut views_hasher = std::collections::hash_map::DefaultHasher::new();
        self.schedule_report_cache.as_ref().map(|cache| cache.key).hash(&mut views_hasher);
        for problem in dispatch_problems {
            problem.bars.hash(&mut views_hasher);
            problem.message.hash(&mut views_hasher);
        }
        let views_key = views_hasher.finish();
        if self.schedule_report_cache.as_ref().is_some_and(|cache| cache.bar_views_key == Some(views_key)) {
            self.mirror_schedule_calculation();
            return;
        }
        let views = reports
            .iter()
            .map(|report| {
                let mut problems = report.problems.iter().map(|problem| problem.message()).collect::<Vec<_>>();
                problems.extend(
                    dispatch_problems
                        .iter()
                        .filter(|problem| problem.bars.contains(&report.bar))
                        .map(|problem| problem.message.clone()),
                );
                let default_name = default_bar_name(report.members.iter().filter_map(|member| member.area_name.as_deref()));
                ScheduleBarView {
                    bar: report.bar,
                    default_name,
                    // A refused attempt is historical evidence about these
                    // still-current inputs, not part of live readiness. The
                    // messages remain available, while repairs immediately
                    // restore the readiness computed above.
                    ready: report.is_ready(),
                    tonnes: report.tonnes,
                    members: report
                        .members
                        .iter()
                        .map(|member| ScheduleMemberView {
                            position: member.position,
                            name: member.name.clone(),
                            solid_name: member.solid_name.clone(),
                            solid_type: member.solid_type.clone(),
                            bench: member.bench.clone(),
                            blast: member.blast.clone(),
                            flitch: member.flitch.clone(),
                            unresolved: member.unresolved.clone(),
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
        if let Some(cache) = self.schedule_report_cache.as_mut() {
            cache.bar_views_key = Some(views_key);
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

        self.ensure_schedule_report_cache();
        let Some(draft) = self.editor.sequence_editor.as_ref() else {
            if !self.editor.sequence_members.is_empty() || self.editor.sequence_generation.is_some() || self.editor.sequence_unavailable.is_some() {
                self.editor.sequence_members.clear();
                self.editor.sequence_generation = None;
                self.editor.sequence_unavailable = None;
                self.redraw_requested = true;
            }
            if let Some(cache) = self.schedule_report_cache.as_mut() {
                cache.sequence_key = None;
            }
            return;
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.schedule_report_cache.as_ref().map(|cache| cache.key).hash(&mut hasher);
        draft.edition.hash(&mut hasher);
        for member in &draft.members {
            match member {
                DraftMember::Held(reference) => {
                    0_u8.hash(&mut hasher);
                    reference.solid.hash(&mut hasher);
                    reference.source.hash(&mut hasher);
                    reference.flitch_base.to_bits().hash(&mut hasher);
                    reference.flitch_top.to_bits().hash(&mut hasher);
                    reference.anchor.map(f64::to_bits).hash(&mut hasher);
                    reference.plan_area.to_bits().hash(&mut hasher);
                    reference.footprint.hash(&mut hasher);
                    reference.volume.map(f64::to_bits).hash(&mut hasher);
                }
                DraftMember::Picked(pick) => {
                    1_u8.hash(&mut hasher);
                    pick.block.hash(&mut hasher);
                    pick.generation.hash(&mut hasher);
                }
            }
        }
        let sequence_key = hasher.finish();
        if self.schedule_report_cache.as_ref().is_some_and(|cache| cache.sequence_key == Some(sequence_key)) {
            return;
        }
        let draft = draft.clone();
        let (snapshot, ground, index, unavailable) = self
            .schedule_report_cache
            .as_ref()
            .map(|cache| (cache.snapshot.clone(), cache.ground.clone(), cache.index.clone(), cache.unavailable.clone()))
            .unwrap_or_else(|| (None, Arc::default(), Arc::new(GroundIndex::build(&[])), None));
        let generation = snapshot.as_ref().map(|snapshot| snapshot.generation);
        let members = match snapshot.as_deref() {
            // No finished run: every member keeps its place and its number,
            // and none of them is described. An absent run is stated once, by
            // the window, rather than as a fault of every block in the list.
            None => draft
                .members
                .iter()
                .map(|_| SequenceMemberView {
                    name: None,
                    solid_name: None,
                    solid_type: None,
                    bench: None,
                    blast: None,
                    flitch: None,
                    unresolved: None,
                    tonnes: None,
                    stale_pick: false,
                    block: None,
                })
                .collect(),
            Some(snapshot) => {
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
                // Looked up once rather than once per member, and kept as an
                // Option: the block above already allows for there being no
                // active document, so a label that cannot be built is left
                // unstated here the same way a tonnage figure is.
                let document = self.workspace.active_document();
                draft
                    .members
                    .iter()
                    .map(|member| {
                        // A held reference is resolved exactly as a bar's own
                        // member is; a pick names a block of a run directly,
                        // and is stale when that run is no longer this one.
                        let (found, unresolved, stale_pick) = match member {
                            DraftMember::Held(reference) => {
                                let status = reference.resolve_indexed(&ground, &index);
                                (status.resolved(), status.message(), false)
                            }
                            DraftMember::Picked(pick) => {
                                let stale = pick.generation != snapshot.generation;
                                let found = snapshot.blocks.iter().position(|block| block.id == pick.block).filter(|_| !stale);
                                (found, stale.then(|| tr!("sequence-pick-superseded")), stale)
                            }
                        };
                        let block = found.and_then(|index| snapshot.blocks.get(index));
                        let labels = block.zip(document).map(|(block, document)| block_labels(document, block));
                        SequenceMemberView {
                            name: block.map(|block| block.name.clone()),
                            solid_name: block.map(|block| block.solid_name.clone()),
                            solid_type: labels.as_ref().map(|labels| labels.0.clone()),
                            bench: labels.as_ref().map(|labels| labels.1.clone()),
                            blast: labels.as_ref().map(|labels| labels.2.clone()),
                            flitch: labels.as_ref().map(|labels| labels.3.clone()),
                            unresolved,
                            tonnes: block.zip(field).and_then(|(block, field)| block_tonnes(block, field).ok()),
                            stale_pick,
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
        if let Some(cache) = self.schedule_report_cache.as_mut() {
            cache.sequence_key = Some(sequence_key);
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
        // A stroke that ran off the ground has not finished: the pointer is
        // still down and will cross more blocks. Only a click clears.
        let painting = self.editor.sequence_paint.is_some();
        let Some(block) = block else {
            if let Some(draft) = self.editor.sequence_editor.as_mut().filter(|_| !painting) {
                draft.selected.clear();
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
        // A stroke works one flitch at a time, and the first block it took
        // said which. The pane cannot enforce that - it is a picture, and the
        // flitch below is behind what the pointer is crossing rather than
        // beside it - so the stroke is bounded here, where the block's own
        // flitch is known. A block off it is skipped, silently: sliding over
        // the level below is part of drawing the stroke, not a mistake to be
        // reported.
        let flitch = snapshot.blocks[index].flitch.base;
        if let Some(paint) = self.editor.sequence_paint.as_mut() {
            match paint.flitch {
                Some(started) if started != flitch => return,
                Some(_) => {}
                None => paint.flitch = Some(flitch),
            }
        }
        let ground = ground_of(&snapshot);
        if let Some(draft) = self.editor.sequence_editor.as_mut() {
            insert_or_select(draft, block, index, generation, &ground);
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
            return Err(tr!("sequence-pick-stale"));
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
    ground: &'a [BlockGround],
    index: &'a GroundIndex,
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
                    solid_type: None,
                    bench: None,
                    blast: None,
                    flitch: None,
                    area_name: None,
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
        let status = member.resolve_indexed(run.ground, run.index);
        let Some(found) = status.resolved() else {
            report.members.push(MemberReport {
                position: index + 1,
                name: None,
                solid_name: None,
                solid_type: None,
                bench: None,
                blast: None,
                flitch: None,
                area_name: None,
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
        let labels = block_labels(document, block);
        report.members.push(MemberReport {
            position: index + 1,
            name: Some(block.name.clone()),
            solid_name: Some(block.solid_name.clone()),
            solid_type: Some(labels.0),
            bench: Some(labels.1),
            blast: Some(labels.2),
            flitch: Some(labels.3),
            area_name: Some(labels.4),
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

/// Take one gated pick into the draft: insert it where the preview slider
/// stands, or select the member that already holds this ground.
///
/// The slider is the insertion point rather than the end of the list because
/// it is where the user is *looking*: the 3D pane shows the ground as it
/// stands at that point in the order, so the block just clicked is the next
/// one to be dug from what is on screen. Wound back to the start, picks build
/// the order from the beginning; left at the end - where it sits after every
/// previous pick - they append, which is the same behaviour a list that only
/// ever appended had.
///
/// Kept pure so the pick's own rules can be checked without a running
/// application: a completed pick adds exactly one member, carrying the run it
/// was made against, and selects - never duplicates - ground the draft
/// already holds, whatever anchor that ground was captured with.
pub(crate) fn insert_or_select(draft: &mut crate::ui::state::SequenceDraft, block: crate::model::DigBlockId, index: usize, generation: u64, ground: &[BlockGround]) {
    use crate::ui::state::DraftMember;

    let existing = draft.members.iter().position(|member| match member {
        DraftMember::Held(reference) => reference.resolve(ground).resolved() == Some(index),
        DraftMember::Picked(pick) => pick.block == block && pick.generation == generation,
    });
    match existing {
        Some(position) => draft.selected = std::iter::once(position).collect(),
        None => {
            let at = draft.preview.min(draft.members.len());
            draft.members.insert(at, DraftMember::Picked(DigBlockPick { block, generation }));
            draft.selected = std::iter::once(at).collect();
            // The picked block has just been dug, so the slider steps over it:
            // it disappears from the pane, the next pick lands after it, and a
            // stroke lays its blocks down in the order it crossed them.
            draft.preview = at + 1;
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
