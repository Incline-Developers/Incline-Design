//! The Schedule workspace's Gantt: one timeline row per loader agent, along
//! elapsed project time.
//!
//! Rows, the ruler, the navigation - and the authored bars, which is where a
//! bar is created, named, copied, assigned, laned and positioned. A time
//! slider always stands across the timeline, shared with Animate's scrubber;
//! the Inspector beside it ([`super::schedule_inspector`]) reads that instant.
//!
//! An authored bar is a **work window**: the period its loader is allowed to
//! work that dig sequence or reclaim. What is actually worked in that period
//! is the optimiser's answer, drawn as a thin band along the top of the bar,
//! and it appears only from a calculated schedule - never inferred from the
//! bar itself. Which block, how many tonnes and why a machine stood are left
//! to the hover, so the bars stay uncluttered.
//!
//! Three marks carry the calculated answer, and all three vanish the moment
//! anything they were calculated from is edited:
//!
//! - a **working band** above the bar for each solved execution span: blue
//!   where it dug, green where it reclaimed. Hovering it says what was worked
//!   at that instant - block, tonnes, destinations;
//! - **grey** over the rest of a dig bar whose ground ran out before its
//!   window closed, which reads as finished early;
//! - an **idle strip** in amber along the foot of a loader's row, where it
//!   executed nothing inside the calculated horizon. Hovering it gives the
//!   reason the result found and what would change it.
//!
//! Delays are drawn from the plan, not the result: delay lists and rosters
//! as a tint of their type's colour across the machine's row, delay bars in
//! that colour. New bars are dragged onto a row from the chips in the
//! top-left corner; a delay asks for its type when it lands.
//!
//! Dragging the middle of a bar moves its whole window; dragging either edge
//! resizes it, and the same window can be typed in exactly. The drag previews
//! in [`crate::ui::state::GanttDrag`] and commits once on release, so one drag
//! is one undo step; the machine and the lane are chosen from the bar's own
//! menu, where what is being chosen is named rather than inferred from where a
//! pointer was let go.
//!
//! Time is elapsed project time in seconds - zero is `Day 1, 00:00`, not a
//! calendar date and not the computer clock. Everything is computed in `f64`
//! seconds and converted to points against whatever rect the timeline gets
//! this frame, so nothing drifts across a resize or a DPI change. Only the
//! ticks and rows actually on screen are visited, so scrolling to day 300
//! costs the same as day 1.

use crate::{
    i18n::{tr, tr_format},
    model::schedule::{
        DestinationId, LoaderAgentId, ScheduleBar, SchedulePlan, WorkWindow,
        cashflow::Activity,
        destinations::DestinationView,
        result::{CalculatedSchedule, Execution, IdleReason, IdleSpan, WorkSource},
    },
    ui::{
        EditorState, UiProjectView, chrome,
        elements::schedule_calendar::format_tonnes,
        fonts::bold,
        state::{
            BarNameDialog, BarWindowDialog, BlastBarDialog, DelayDrop, GanttDrag, GanttDragMode, GanttPaletteItem, GanttView, PlanningPage, PlanningSubpage, ReclaimBarDialog,
            ScheduleBarView, ScheduleEdit, ScheduleRepairTarget, ScheduleStep, UiCommand,
        },
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            toolbar::GROUP_CORNER_RADIUS,
        },
    },
};

/// Width of the fixed agent-name column. Wide enough for a machine name and
/// its class beneath it; never scrolled, so a horizontal pan leaves it alone.
const HEADER_WIDTH: f32 = 200.0;
/// Least height of one agent's row, which is what its name and class need.
const ROW_HEIGHT: f32 = 34.0;
/// Breathing room between loader units, also serving as a clear drop slot
/// while a sequence is moved vertically.
const LOADER_GAP: f32 = 12.0;
/// How close to the seam between two lanes a vertical drag has to come before
/// it reads as asking for a lane of its own there rather than for one of the
/// two. The gap between loader units falls inside this either side, so the
/// space between two machines is a live target as well as the seams within
/// one.
const LANE_INSERT_ZONE: f32 = 9.0;
/// Least width a bar is drawn at, whatever its window is worth on screen.
/// A window of minutes at a month's zoom is still something to grab.
const MIN_BAR_WIDTH: f32 = 18.0;
/// Height of the working band above a bar.
const WORK_BAND: f32 = 5.0;
/// From the top of a lane slot to the top of its bar: a gap, the working
/// band, and a gap.
const BAR_TOP: f32 = 2.0 + WORK_BAND + 1.0;
/// Space kept below each bar, between it and the lane under it.
const BAR_BOTTOM: f32 = 4.0;
/// Height of the idle strip along the foot of a loader's row.
const IDLE_STRIP: f32 = 4.0;
/// Room a row keeps under its lanes for the idle strip.
const IDLE_SPACE: f32 = IDLE_STRIP + 4.0;
/// How wide the grab zone on each edge of a bar is.
const RESIZE_GRIP: f32 = 5.0;
/// Shortest window a drag may leave behind. Typing a shorter one is still
/// allowed: a drag is imprecise and this stops it collapsing a bar to nothing,
/// while the dialog is exact and means what it says.
const MIN_WINDOW_SECONDS: f64 = 900.0;
/// Gap kept between two markers packed into the same sub-row, so two bars
/// close together still read as two bars.
const BAR_GAP: f32 = 4.0;
/// How close to an edge a drag has to come before the window follows it.
const EDGE_PAN_MARGIN: f32 = 24.0;
/// How far past that margin the pointer has to go for the window to pan at its
/// full rate.
const EDGE_PAN_RANGE: f32 = 80.0;
/// How much of the visible span a fully deflected edge pan covers per
/// *second*. Measured against elapsed time rather than against frames, so the
/// window follows a drag at the same speed on a 60 Hz panel and a 240 Hz one -
/// a per-frame step would be four times faster on the second.
const EDGE_PAN_SPAN_PER_SECOND: f64 = 1.2;
/// Longest frame an edge pan is allowed to integrate over. A stall - a save, a
/// geometry rebuild - must not teleport the window the length of that pause.
const EDGE_PAN_MAX_STEP: f32 = 1.0 / 20.0;
/// Height of one band of the time ruler. Two bands are drawn when the minor
/// ticks are finer than a day, so the days above can group the hours below.
const RULER_BAND: f32 = 20.0;
/// Closest two labelled ticks are allowed to sit. Interval selection works
/// back from this, which is what keeps labels legible at any pane width.
const MIN_TICK_SPACING: f32 = 72.0;
/// How much one zoom-button press changes the visible span.
const ZOOM_STEP: f64 = 1.5;

/// Write one elapsed-seconds instant as a day-and-time label.
///
/// Day 1 starts at zero, so the first day of a schedule reads `Day 1` rather
/// than `Day 0` - which is how a mine plan is written and read.
fn day_of(seconds: f64) -> i64 {
    (seconds / GanttView::DAY).floor() as i64 + 1
}

fn time_of(seconds: f64) -> String {
    let into_day = seconds - (seconds / GanttView::DAY).floor() * GanttView::DAY;
    let minutes = (into_day / 60.0).round() as i64;
    format!("{:02}:{:02}", (minutes / 60).clamp(0, 23), minutes % 60)
}

pub(crate) fn instant_label(seconds: f64) -> String {
    tr!("gantt-day-time", day = day_of(seconds).to_string(), time = time_of(seconds))
}

/// The Gantt page. Returns the rect it claimed, for the caller to round off
/// as one chrome region.
pub(crate) fn draw_details(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, document: &crate::model::Document, commands: &mut Vec<UiCommand>) -> egui::Rect {
    // Cloned rather than borrowed: the canvas takes `editor` mutably to hold
    // its drag and its selection. Every edit below is addressed to the project
    // this plan was read from, so one queued against a project that is closed
    // before the frame's commands are handled is refused rather than applied
    // to its successor.
    let plan = project.schedule.clone();
    let session = project.active_session;
    let rect = draw_timeline_page(ui, editor, &plan, document, commands, |ui, timeline, editor, destinations, commands| {
        draw_canvas(ui, timeline, editor, &plan, destinations, session, commands);
    });
    crate::ui::dialogs::schedule::draw_bar_name_dialog(ui, editor, &plan, session, commands);
    crate::ui::dialogs::schedule::draw_bar_window_dialog(ui, editor, &plan, session, commands);
    crate::ui::dialogs::schedule::draw_reclaim_bar_dialog(ui, editor, &plan, document, session, commands);
    crate::ui::dialogs::schedule::draw_blast_bar_dialog(ui, editor, &plan, session, commands);
    crate::ui::dialogs::schedule::draw_blast_window_dialog(ui, editor, &plan, session, commands);
    crate::ui::dialogs::sequence_editor::draw_sequence_editor(ui, editor, project, document, &plan, session, commands);
    rect
}

/// One priority lane of a row, and the sub-rows its markers are packed into.
///
/// Two bars may legitimately sit at the same instant in the same lane - that is
/// what copying a bar produces, deliberately - and one painted over the other
/// cannot be selected, renamed or dragged. So a lane is as many sub-rows deep
/// as it needs for no two markers in it to overlap. This is presentation only:
/// no bar's machine, lane or earliest start is changed to make room.
struct Lane {
    priority: u32,
    /// Indices into `plan.bars()`, by sub-row. Always at least one sub-row,
    /// occupied or not, so an empty lane still has somewhere to right-click.
    stacks: Vec<Vec<usize>>,
    /// Top of the lane within its row, and how tall it is.
    offset: f32,
    height: f32,
}

impl Lane {
    fn stack_height(&self) -> f32 {
        self.height / self.stacks.len().max(1) as f32
    }
}

/// One row of the Gantt: a machine, or the lane for bars that have none.
///
/// Rows are laid out once a frame from the plan, so what is drawn, what is
/// hit-tested and what a drag reads are all the same arrangement.
struct Row {
    /// The machine this row is, or `None` for the unassigned row.
    agent: Option<LoaderAgentId>,
    /// A dozer, drill or MPU: its work is drill and blast's, and it has no
    /// loader idle strip.
    drill_blast: bool,
    blasting: bool,
    title: String,
    subtitle: String,
    /// The priority lanes present in this row, ascending - lower is higher
    /// priority. Only the lanes bars actually sit in: an empty row has one, so
    /// there is somewhere to drop a bar and somewhere to right-click.
    lanes: Vec<Lane>,
    /// Top of the row in content space, before the row scroll is taken off.
    top: f32,
    height: f32,
}

impl Row {
    fn lane_rect(&self, lane: usize, left: f32, right: f32, scroll: f32, body_top: f32) -> egui::Rect {
        let lane = &self.lanes[lane];
        let top = body_top + self.top - scroll + lane.offset;
        egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, top + lane.height))
    }
}

/// Where a vertical drag would put a bar: whose row, which lane, and whether
/// that lane exists yet.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Placement {
    agent: Option<LoaderAgentId>,
    priority: u32,
    /// Whether `priority` names a lane to open rather than one to join. A bar
    /// let go on the seam between two lanes is asking for a lane of its own
    /// between them, which is the only way to make one from the Gantt.
    insert: bool,
}

/// Where one bar's window sits along the timeline.
///
/// Taken from the bar's *authored* window, never from a drag preview: a drag
/// draws somewhere else, but the rows it is dragged over stay still rather
/// than repacking under the pointer.
///
/// An open-ended window has no right edge to pack against, so it claims the
/// rest of the timeline - which is what it actually does.
#[derive(Clone, Copy, Debug, PartialEq)]
struct BarExtent {
    left: f32,
    right: f32,
}

impl BarExtent {
    fn of(view: GanttView, window: WorkWindow, body: egui::Rect) -> Self {
        let left = view.x_of(window.start_h * GanttView::HOUR, body.left(), body.width());
        let right = match window.end_h {
            Some(end) => view.x_of(end * GanttView::HOUR, body.left(), body.width()).max(left + MIN_BAR_WIDTH),
            None => f32::INFINITY,
        };
        Self { left, right }
    }

    /// Where the bar is actually painted: an open-ended one runs to the edge
    /// of the body rather than off into an infinity no rectangle can hold.
    fn drawn(self, body: egui::Rect) -> (f32, f32) {
        (self.left, if self.right.is_finite() { self.right } else { body.right() })
    }
}

/// One bar's marker: the label it carries and the window it is drawn over.
struct Marker {
    galley: std::sync::Arc<egui::Galley>,
    extent: BarExtent,
}

/// One frame's arrangement of the whole timeline: every bar's marker, and the
/// rows, lanes and sub-rows they are packed into. Laid out once and then used
/// for drawing, hit-testing and dragging alike, so those cannot disagree.
struct Layout {
    markers: Vec<Marker>,
    rows: Vec<Row>,
}

impl Layout {
    fn build(ui: &egui::Ui, editor: &EditorState, plan: &SchedulePlan, body: egui::Rect) -> Self {
        let markers = layout_markers(ui, editor, plan, body);
        let extents: Vec<BarExtent> = markers.iter().map(|marker| marker.extent).collect();
        let mut rows = layout_rows(plan, &extents);
        if let Some(row) = rows.iter_mut().find(|row| row.blasting) {
            let lanes = firing_markers(ui, editor.schedule_result.as_deref(), editor.gantt, body)
                .iter()
                .map(|marker| marker.lane + 1)
                .max()
                .unwrap_or(1);
            row.height = row.height.max(30.0 + lanes as f32 * 20.0);
        }
        Self { markers, rows }
    }

    /// How tall the rows are altogether, which is what the body scrolls over.
    fn height(&self) -> f32 {
        self.rows.last().map_or(0.0, |row| row.top + row.height)
    }
}

/// Measure every bar's marker once, for the layout and the drawing both, so
/// the rectangle that is packed is the rectangle that is painted and hit.
fn layout_markers(ui: &egui::Ui, editor: &EditorState, plan: &SchedulePlan, body: egui::Rect) -> Vec<Marker> {
    // Body rather than Small throughout the timeline: a bar's name, the day
    // under a tick and a lane's number are the page's content, not its
    // footnotes, and a marker nobody can read names nothing.
    let font = egui::TextStyle::Body.resolve(ui.style());
    let color = ui.visuals().strong_text_color();
    let view = editor.gantt;
    plan.bars()
        .iter()
        .map(|bar| {
            let report = editor.schedule_bar_reports.iter().find(|report| report.bar == bar.id);
            let galley = ui.painter().layout_no_wrap(bar_label(bar, report, Some(plan)), font.clone(), color);
            Marker {
                extent: BarExtent::of(view, bar.window, body),
                galley,
            }
        })
        .collect()
}

/// Pack one lane's bars into the fewest sub-rows in which none of them
/// overlap: the markers are sorted by where they start and each goes in the
/// first sub-row whose last marker has ended.
fn pack_lane(bars: &[usize], extents: &[BarExtent]) -> Vec<Vec<usize>> {
    let mut order = bars.to_vec();
    // Position first, then the bar's own place in the plan, so bars that start
    // at the same instant - a copy and its original - stack in a stable order
    // rather than one that depends on how the list was walked.
    order.sort_by(|left, right| extents[*left].left.total_cmp(&extents[*right].left).then(left.cmp(right)));
    let mut stacks: Vec<Vec<usize>> = Vec::new();
    let mut ends: Vec<f32> = Vec::new();
    for index in order {
        let extent = extents[index];
        match ends.iter().position(|end| *end <= extent.left) {
            Some(stack) => {
                ends[stack] = extent.right + BAR_GAP;
                stacks[stack].push(index);
            }
            None => {
                ends.push(extent.right + BAR_GAP);
                stacks.push(vec![index]);
            }
        }
    }
    stacks
}

/// Arrange the plan's machines and bars into rows, lanes and sub-rows.
///
/// The unassigned row comes first and only exists while something is in it:
/// a bar whose machine was deleted is still the user's work and has to be
/// somewhere it can be seen and reassigned, but an empty lane above the fleet
/// would be a permanent reminder of nothing.
fn layout_rows(plan: &SchedulePlan, extents: &[BarExtent]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::with_capacity(plan.agents().len() + 1);
    // A bar counts as unassigned when it names no machine, and also when it
    // names one this plan does not hold. Load refuses such a plan and no edit
    // can create one, so the second case should be unreachable - but "should
    // be unreachable" is not a reason to let a bar fall off the Gantt, and an
    // unassigned row is where it can be seen and reassigned.
    let placed = |bar: &ScheduleBar| bar.agent.filter(|agent| plan.agent(*agent).is_some());
    if plan.bars().iter().any(|bar| placed(bar).is_none()) {
        rows.push(Row {
            agent: None,
            drill_blast: false,
            blasting: false,
            title: tr!("schedule-bar-unassigned"),
            subtitle: tr!("schedule-bar-unassigned-note"),
            lanes: Vec::new(),
            top: 0.0,
            height: 0.0,
        });
    }
    for agent in plan.agents() {
        let subtitle = match plan.class(agent.class_id) {
            Some(class) => tr_format!(
                literal = "%class% · %rate% %unit%",
                class = class.name.clone(),
                rate = format!("{}", class.default_dig_rate_tph),
                unit = if class.kind.is_drill_blast() {
                    class.kind.rate_unit().to_owned()
                } else {
                    tr!("schedule-tph")
                }
            ),
            None => tr!("schedule-error-unknown-class"),
        };
        rows.push(Row {
            agent: Some(agent.id),
            drill_blast: plan.agent_kind(agent.id).is_some_and(crate::model::schedule::MachineKind::is_drill_blast),
            blasting: false,
            title: agent.name.clone(),
            subtitle,
            lanes: Vec::new(),
            top: 0.0,
            height: 0.0,
        });
    }
    rows.push(Row {
        agent: None,
        drill_blast: true,
        blasting: true,
        title: tr!("gantt-blasting-row"),
        subtitle: tr!("gantt-blasting-row-note"),
        lanes: Vec::new(),
        top: 0.0,
        height: 0.0,
    });
    // Filled with one sub-row holding everything the lane has; packed into
    // non-overlapping sub-rows once the lane is complete.
    for (index, bar) in plan.bars().iter().enumerate() {
        let agent = placed(bar);
        let row = rows.iter_mut().find(|row| !row.blasting && row.agent == agent).expect("every bar's row was made above");
        let lane = match row.lanes.binary_search_by_key(&bar.priority, |lane| lane.priority) {
            Ok(lane) => lane,
            Err(lane) => {
                row.lanes.insert(
                    lane,
                    Lane {
                        priority: bar.priority,
                        stacks: vec![Vec::new()],
                        offset: 0.0,
                        height: 0.0,
                    },
                );
                lane
            }
        };
        row.lanes[lane].stacks[0].push(index);
    }
    let mut top = 0.0;
    for row in &mut rows {
        if row.lanes.is_empty() {
            row.lanes.push(Lane {
                priority: 0,
                stacks: vec![Vec::new()],
                offset: 0.0,
                height: 0.0,
            });
        }
        let mut lanes_height = 0.0;
        for lane in &mut row.lanes {
            let members = std::mem::take(&mut lane.stacks);
            lane.stacks = pack_lane(&members[0], extents);
            if lane.stacks.is_empty() {
                lane.stacks.push(Vec::new());
            }
            lane.height = (BAR_TOP + plan.bar_height() + BAR_BOTTOM) * lane.stacks.len() as f32;
            lanes_height += lane.height;
        }
        // A row is never shorter than a machine's name and its class need.
        // Where the lanes alone do not fill that, they share the rest out
        // between them rather than leaving a dead strip under the last one.
        let scale = if lanes_height > 0.0 { ((ROW_HEIGHT - IDLE_SPACE) / lanes_height).max(1.0) } else { 1.0 };
        let mut offset = 0.0;
        for lane in &mut row.lanes {
            lane.height *= scale;
            lane.offset = offset;
            offset += lane.height;
        }
        // The idle strip has the foot of the row to itself, under every
        // lane, so no bar ever covers it.
        row.height = (offset + IDLE_SPACE).max(ROW_HEIGHT);
        row.top = top;
        top += row.height + LOADER_GAP;
    }
    rows
}

/// Run Period, Run All Periods and Cancel, as the Gantt and the Calendar
/// both show them.
///
/// The run controls are the Solids page's own, in the same order and with the
/// same icons: a schedule is run the way everything else in this project is
/// run. One control in two places rather than two that drift: `salt` keeps
/// their widget ids apart, because both pages can be laid out in the same
/// frame.
///
/// The browser build shows them disabled with the reason, and refuses the
/// commands as well: calculation needs the native solver.
pub(crate) fn draw_calculation_controls(ui: &mut egui::Ui, editor: &mut EditorState, salt: &str, commands: &mut Vec<UiCommand>) {
    use crate::ui::{
        elements::planning_setup::{CANCEL_TINT, RUN_ALL_TINT, RUN_STEP_TINT},
        widgets::toolbar::ToolbarButton,
    };

    let available = !cfg!(target_arch = "wasm32");
    let working = editor.schedule_run_working;
    let startable = available && !working;
    let tint = |color: egui::Color32, enabled: bool| if enabled { color } else { color.gamma_multiply(0.35) };
    let hint = |note: String| if available { note } else { tr!("schedule-run-desktop-only") };
    let side = ui.available_height();
    let spacing = std::mem::replace(&mut ui.spacing_mut().item_spacing.x, 0.0);
    if ui
        .add_enabled(
            startable,
            ToolbarButton::new(
                egui::Image::new(crate::ui::unthemed_icon!("play.svg")).tint(tint(RUN_STEP_TINT, startable)),
                hint(tr!("schedule-run-period-note")),
            )
            .button_side(side)
            .id_salt(format!("{salt}_run_period")),
        )
        .clicked()
    {
        commands.push(UiCommand::RunSchedulePeriod);
    }
    if ui
        .add_enabled(
            startable,
            ToolbarButton::new(
                egui::Image::new(crate::ui::unthemed_icon!("play_all.svg")).tint(tint(RUN_ALL_TINT, startable)),
                hint(tr!("schedule-run-whole-note")),
            )
            .button_side(side)
            .id_salt(format!("{salt}_run_whole")),
        )
        .clicked()
    {
        commands.push(UiCommand::RunAllSchedulePeriods);
    }
    if ui
        .add_enabled(
            working,
            ToolbarButton::new(
                egui::Image::new(crate::ui::unthemed_icon!("stop.svg")).tint(tint(CANCEL_TINT, working)),
                if editor.schedule_run_improving {
                    tr!("schedule-run-stop-early-note")
                } else {
                    tr!("schedule-run-cancel-note")
                },
            )
            .button_side(side)
            .id_salt(format!("{salt}_run_cancel")),
        )
        .clicked()
    {
        commands.push(UiCommand::CancelScheduleCalculation);
    }
    ui.spacing_mut().item_spacing.x = spacing;
    ui.add_space(6.0);
    // The hourly schedule is what every other control produces; this is the
    // one that spends the solve time looking past it.
    let improve = egui::Button::new(if editor.schedule_run_improve {
        tr!("schedule-improving")
    } else {
        tr!("schedule-improve")
    })
    .corner_radius(GROUP_CORNER_RADIUS)
    .selected(editor.schedule_run_improve);
    if ui
        .add_enabled(startable, improve)
        .on_hover_text(hint(tr!("schedule-improve-note")))
        .on_disabled_hover_text(hint(if editor.schedule_run_improve {
            tr!("schedule-improving-note")
        } else {
            tr!("schedule-improve-note")
        }))
        .clicked()
    {
        commands.push(UiCommand::ImproveSchedule);
    }
    ui.add_enabled(available, egui::Checkbox::new(&mut editor.schedule_auto_recalculate, tr!("schedule-auto")))
        .on_hover_text(hint(tr!("schedule-auto-note")));
}

/// The one status line the run controls carry, with the detail behind it on
/// hover: value, bound and gap, timings, and the approximations the result
/// rests on. Shared by the Gantt and the Calendar.
pub(crate) fn draw_run_status(ui: &mut egui::Ui, editor: &EditorState) {
    if editor.schedule_run_status.is_empty() {
        return;
    }
    let text = egui::RichText::new(&editor.schedule_run_status);
    let text = if editor.schedule_run_stale && editor.schedule_result.is_none() {
        text.color(ui.visuals().warn_fg_color)
    } else {
        text.weak()
    };
    ui.add(egui::Label::new(text).truncate()).on_hover_ui(|ui| {
        ui.set_max_width(520.0);
        ui.label(&editor.schedule_run_status);
        if let Some(schedule) = &editor.schedule_result {
            let started: f64 = schedule.ground.iter().map(|balance| balance.started_t).sum();
            let remaining: f64 = schedule.ground.iter().map(|balance| balance.remaining_t).sum();
            ui.label(tr!(
                "schedule-result-summary",
                started = format_tonnes(started),
                extracted = format_tonnes(started - remaining),
                remaining = format_tonnes(remaining)
            ));
        }
        for line in &editor.schedule_run_details {
            ui.add(egui::Label::new(egui::RichText::new(line).weak()).wrap());
        }
    });
}

/// How far the work on the timeline reaches, in elapsed seconds, or `None`
/// when there is none to frame.
///
/// Both what was authored and what was calculated: a bar left open-ended has
/// no end of its own, and the run is the only thing that knows where its work
/// actually finished.
fn scheduled_extent(editor: &EditorState, plan: &crate::model::schedule::SchedulePlan) -> Option<f64> {
    let mut end = 0.0_f64;
    for bar in plan.bars() {
        end = end.max(bar.window.end_h.unwrap_or(bar.window.start_h) * GanttView::HOUR);
    }
    if let Some(schedule) = &editor.schedule_result {
        end = end.max(schedule.last_activity_h().max(schedule.requested_end_h) * GanttView::HOUR);
    }
    (end > 0.0).then_some(end)
}

fn draw_toolbar(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, commands: &mut Vec<UiCommand>) {
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("gantt_toolbar").max_rect(rect));
    child.set_clip_rect(child.clip_rect().intersect(rect));
    child.horizontal_centered(|ui| {
        draw_calculation_controls(ui, editor, "gantt", commands);
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(8.0);
        let button = |ui: &mut egui::Ui, label: &str, tooltip: String| ui.add(egui::Button::new(label).corner_radius(GROUP_CORNER_RADIUS)).on_hover_text(tooltip);
        if button(ui, "−", tr!("gantt-zoom-out")).clicked() {
            editor.gantt.zoom_at(1.0 / ZOOM_STEP, 0.5);
        }
        if button(ui, "+", tr!("gantt-zoom-in")).clicked() {
            editor.gantt.zoom_at(ZOOM_STEP, 0.5);
        }
        if ui.add(egui::Button::new(tr!("gantt-reset-view")).corner_radius(GROUP_CORNER_RADIUS)).clicked() {
            // Reset to what is there, not to a fixed week: a sequence running
            // to day 45 is reset to day 45.
            let extent = scheduled_extent(editor, plan);
            editor.gantt.reset_to(extent);
        }
        ui.add_space(8.0);
        ui.toggle_value(&mut editor.gantt_inspector_open, tr!("gantt-inspector"))
            .on_hover_text(tr!("gantt-inspector-help"));
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(tr!(
                "gantt-range",
                from = instant_label(editor.gantt.start_seconds),
                to = instant_label(editor.gantt.end_seconds())
            ))
            .weak(),
        );
        // One status names what the calculated marks mean. When it names a
        // blocker, the adjacent action opens the exact setup step that owns
        // the repair.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            if let Some(target) = editor.schedule_run_repair {
                let label = match target {
                    ScheduleRepairTarget::Schedule(_) => tr!("schedule-open-setup"),
                    ScheduleRepairTarget::Solids(_) => tr!("schedule-open-solids-setup"),
                };
                if ui.button(label).clicked() {
                    match target {
                        ScheduleRepairTarget::Schedule(step) => editor.open_schedule_step(step),
                        ScheduleRepairTarget::Solids(step) => {
                            editor.planning_page = PlanningPage::Solids;
                            editor.solids_subpage = PlanningSubpage::Setup;
                            editor.planning_solids_step = step;
                        }
                    }
                }
            }
            draw_run_status(ui, editor);
        });
    });
}

/// A page laid out like the Gantt: the toolbar across the top, a timeline
/// canvas drawn by `draw`, and the Inspector beside it while it is open.
/// Returns the rect it claimed, for the caller to round off as one region.
pub(super) fn draw_timeline_page(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &crate::model::Document,
    commands: &mut Vec<UiCommand>,
    draw: impl FnOnce(&mut egui::Ui, egui::Rect, &mut EditorState, &[DestinationView], &mut Vec<UiCommand>),
) -> egui::Rect {
    egui::CentralPanel::default()
        .frame(chrome::region_frame(ui))
        .show(ui, |ui| {
            let available = ui.available_rect_before_wrap();
            let toolbar_height = ui.spacing().interact_size.y + 8.0;
            let toolbar = egui::Rect::from_min_size(available.min, egui::vec2(available.width(), toolbar_height.min(available.height())));
            let canvas = egui::Rect::from_min_max(egui::pos2(available.left(), toolbar.bottom()), available.max);
            draw_toolbar(ui, toolbar, editor, plan, commands);
            if canvas.is_positive() {
                // Names are resolved here, at draw time, by stable id: a
                // rename relabels a calculated span without recalculating it.
                let destinations = crate::model::schedule::destinations::available(document.solids(), plan.routing());
                let inspector_width = super::schedule_inspector::INSPECTOR_WIDTH;
                let inspect = editor.gantt_inspector_open && canvas.width() >= inspector_width + super::schedule_inspector::MIN_TIMELINE_WIDTH;
                let timeline = if inspect {
                    egui::Rect::from_min_max(canvas.min, egui::pos2(canvas.right() - inspector_width, canvas.bottom()))
                } else {
                    canvas
                };
                draw(ui, timeline, editor, &destinations, commands);
                if inspect {
                    let fields: Vec<_> = document.reserve_fields().iter().map(|field| (field.id, field.name.clone())).collect();
                    let panel = egui::Rect::from_min_max(egui::pos2(timeline.right(), canvas.top()), canvas.max);
                    super::schedule_inspector::draw_inspector(ui, panel, editor, plan, &destinations, &fields);
                }
            }
            ui.allocate_rect(available, egui::Sense::hover());
        })
        .response
        .rect
}

/// The parts of a timeline canvas: the name column, the ruler over the body,
/// the body, and the corner the column and ruler leave.
pub(super) struct TimelineFrame {
    pub(super) header: egui::Rect,
    pub(super) ruler: egui::Rect,
    pub(super) body: egui::Rect,
    pub(super) corner: egui::Rect,
    /// Whether the ruler carries a band of days above finer ticks.
    pub(super) day_band: bool,
}

impl TimelineFrame {
    pub(super) fn new(view: GanttView, rect: egui::Rect) -> Self {
        let header_width = HEADER_WIDTH.min(rect.width() * 0.5);
        let day_band = view.minor_interval(rect.width() - header_width, MIN_TICK_SPACING) < GanttView::DAY;
        let ruler_height = (RULER_BAND * if day_band { 2.0 } else { 1.0 }).min(rect.height());
        let header = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + ruler_height), egui::pos2(rect.left() + header_width, rect.bottom()));
        let ruler = egui::Rect::from_min_max(egui::pos2(rect.left() + header_width, rect.top()), egui::pos2(rect.right(), rect.top() + ruler_height));
        let body = egui::Rect::from_min_max(ruler.left_bottom(), rect.max);
        let corner = egui::Rect::from_min_max(rect.min, header.right_top());
        Self {
            header,
            ruler,
            body,
            corner,
            day_band,
        }
    }

    /// The minor tick interval the ruler and grid use at this zoom.
    pub(super) fn interval(&self, view: GanttView) -> f64 {
        view.minor_interval(self.body.width(), MIN_TICK_SPACING)
    }
}

/// The wheel, pinch and middle-drag over a timeline canvas: zoom and pan the
/// shared view. Returns whether the pointer is over the canvas and how far
/// the wheel asks the rows to scroll down.
pub(super) fn navigate(ui: &mut egui::Ui, rect: egui::Rect, body: egui::Rect, editor: &mut EditorState, salt: &str) -> (bool, f32) {
    // Input is read only while the pointer is over this canvas, so the wheel
    // still scrolls whatever else is on screen and the keyboard is untouched.
    let response = ui.interact(rect, ui.id().with(salt), egui::Sense::click_and_drag());
    // Read from the pointer rather than the canvas response, because what is
    // drawn on top of it takes the hover for itself: the wheel has to keep
    // zooming and panning while the pointer is over a bar or a chart.
    let pointer = ui.input(|input| input.pointer.hover_pos());
    let over_canvas = pointer.is_some_and(|pos| rect.contains(pos));
    let mut rows = 0.0;
    if over_canvas && body.width() > 0.0 {
        let (scroll, zoom) = ui.input(|input| (input.smooth_scroll_delta, f64::from(input.zoom_delta())));
        if (zoom - 1.0).abs() > f64::EPSILON {
            // Anchored where the pointer is, so the instant under the cursor
            // stays under it - except where the window is already against the
            // time origin, which the view clamps for us.
            let anchor = pointer.map_or(0.5, |pos| f64::from((pos.x - body.left()) / body.width()));
            editor.gantt.zoom_at(zoom, anchor);
        }
        if scroll.x != 0.0 {
            editor.gantt.pan(-f64::from(scroll.x) / f64::from(body.width()) * editor.gantt.span_seconds);
        }
        rows = -scroll.y;
    }
    if response.dragged_by(egui::PointerButton::Middle) && body.width() > 0.0 {
        editor.gantt.pan(-f64::from(response.drag_delta().x) / f64::from(body.width()) * editor.gantt.span_seconds);
    }
    (over_canvas, rows)
}

/// The header column, the ruler, the rows and the bars, plus the navigation
/// over them.
fn draw_canvas(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, destinations: &[DestinationView], session: u32, commands: &mut Vec<UiCommand>) {
    let TimelineFrame {
        header,
        ruler,
        body,
        corner,
        day_band,
    } = TimelineFrame::new(editor.gantt, rect);
    let (over_canvas, rows_scroll) = navigate(ui, rect, body, editor, "gantt_canvas");
    editor.gantt.row_scroll += rows_scroll;
    // Laid out after the navigation, so the arrangement drawn this frame is
    // the one this frame's zoom and pan produced. How deep a lane stacks
    // depends on which markers overlap, which depends on the zoom.
    let layout = Layout::build(ui, editor, plan, body);
    let rows_height = layout.height();
    let max_scroll = (rows_height - body.height()).max(0.0);
    // A row removed - or a lane that stopped stacking - under a scrolled view
    // must not leave the body blank.
    editor.gantt.row_scroll = editor.gantt.row_scroll.clamp(0.0, max_scroll);

    let visuals = ui.visuals().clone();
    let rule = visuals.widgets.noninteractive.bg_stroke;
    let (surface, stripe) = crate::ui::widgets::tree_row_colors(ui);
    ui.painter().rect_filled(rect, 0.0, surface);
    ui.painter().rect_filled(ruler, 0.0, visuals.widgets.noninteractive.bg_fill);
    ui.painter().rect_filled(corner, 0.0, visuals.widgets.noninteractive.bg_fill);

    let interval = editor.gantt.minor_interval(body.width(), MIN_TICK_SPACING);
    draw_ruler(ui, ruler, editor.gantt, interval, day_band);
    draw_rows(ui, header, body, stripe, editor, &layout.rows);
    draw_grid(ui, body, editor.gantt, interval);
    draw_calendar_delays(ui, body, editor.gantt, editor.gantt.row_scroll, plan, &layout.rows);
    draw_palette(ui, corner, editor);
    // Taken out for the duration of the frame rather than cloned: the bars
    // need it while `editor` is borrowed mutably for the drag and the
    // selection, and a calculated schedule is not a small value to copy once
    // a frame.
    let schedule = editor.schedule_result.take();

    // Rules last, so no row fill or grid line sits on top of them.
    let painter = ui.painter();
    painter.line_segment([ruler.left_bottom(), ruler.right_bottom()], rule);
    painter.line_segment([corner.right_top(), header.right_bottom()], rule);
    painter.rect_stroke(rect, 0.0, rule, egui::StrokeKind::Inside);

    // The lanes first and the bars over them, both after the canvas: a click
    // on a bar is a click on the bar rather than a pan of the timeline, and a
    // right-click on empty lane space is that lane's own menu.
    draw_row_menus(ui, body, editor, plan, &layout.rows, session, commands);
    draw_bars(ui, body, editor, plan, destinations, &layout, session, commands, schedule.as_deref());
    draw_blasting_row(ui, body, editor, plan, &layout.rows, session, commands, schedule.as_deref());
    // Idle is a row-level indicator, under every lane of the machine.
    draw_idle(ui, body, editor.gantt, editor.gantt.row_scroll, schedule.as_deref(), &layout.rows, destinations);
    draw_palette_drop(ui, body, editor, plan, &layout.rows, session, commands);
    draw_delay_drop_menu(ui, editor, plan, session, commands);
    editor.schedule_result = schedule;
    // Over everything, because it marks an instant across all of it, and last
    // so its handle takes the pointer from the bars it crosses.
    draw_time_slider(ui, ruler, body, editor, interval, over_canvas);

    if plan.agents().is_empty() && layout.rows.is_empty() {
        centred_note(ui, body, tr!("gantt-empty-fleet"));
    } else if plan.bars().is_empty()
        && let Some(agent) = plan.agents().first()
    {
        // A single empty-state action remains reachable even when the fleet's
        // empty rows fill the viewport.
        let size = egui::vec2(120.0, ui.spacing().interact_size.y);
        let button_rect = egui::Rect::from_center_size(body.center(), size);
        if ui
            .put(button_rect, egui::Button::new(tr!("schedule-add-work")).corner_radius(GROUP_CORNER_RADIUS))
            .clicked()
        {
            commands.push(UiCommand::schedule(
                session,
                ScheduleEdit::AddBar {
                    name: String::new(),
                    agent: Some(agent.id),
                    priority: 0,
                    window: WorkWindow {
                        start_h: 0.0,
                        end_h: Some(crate::model::schedule::SCHEDULE_PERIOD_H),
                    },
                    insert_lane: false,
                },
            ));
        }
    }
}

/// Width of the slider line's grab strip across the rows.
const SLIDER_GRAB_W: f32 = 9.0;
/// Height of the slider's time label, which is also its handle in the ruler.
const SLIDER_LABEL_H: f32 = 16.0;

/// Where a dragged slider lands: whole hours once the ruler counts in hours
/// or coarser steps, quarter hours when zoomed in to single hours.
pub(super) fn snap_slider(seconds: f64, interval: f64) -> f64 {
    let step = if interval > GanttView::HOUR { GanttView::HOUR } else { GanttView::HOUR / 4.0 };
    ((seconds / step).round() * step).max(0.0)
}

/// The time slider: one instant, marked across the whole timeline.
///
/// Always there, calculated schedule or not. It is the instant the Inspector
/// reads and Animate's scrubber, seen from here - the three share one time -
/// and it is where bar tools will act. Pressing anywhere on the ruler moves it
/// there, dragging along the ruler or the line scrubs it, and the arrow keys
/// step it an hour (a day with Shift) while the pointer is over the Gantt.
/// Scrolled out of view, a label pinned to that edge of the ruler says where
/// it is, and pressing it brings it back.
pub(super) fn draw_time_slider(ui: &mut egui::Ui, ruler: egui::Rect, body: egui::Rect, editor: &mut EditorState, interval: f64, over_canvas: bool) {
    if !body.is_positive() || !ruler.is_positive() {
        return;
    }
    let color = ui.visuals().selection.stroke.color;
    // Black or white, whichever reads on the label's fill in this theme.
    let on_fill = |fill: egui::Color32| {
        let luma = 0.299 * f32::from(fill.r()) + 0.587 * f32::from(fill.g()) + 0.114 * f32::from(fill.b());
        if luma > 150.0 { egui::Color32::BLACK } else { egui::Color32::WHITE }
    };
    let to_hours = |seconds: f64| seconds / GanttView::HOUR;

    if over_canvas && ui.memory(|memory| memory.focused().is_none()) {
        // Every press counts, each with its own Shift: several can land in
        // one frame, and a held key repeats.
        let step: f64 = ui.input(|input| {
            input
                .events
                .iter()
                .map(|event| match event {
                    egui::Event::Key {
                        key, pressed: true, modifiers, ..
                    } => {
                        let unit = if modifiers.shift { 24.0 } else { 1.0 };
                        match key {
                            egui::Key::ArrowRight => unit,
                            egui::Key::ArrowLeft => -unit,
                            _ => 0.0,
                        }
                    }
                    _ => 0.0,
                })
                .sum()
        });
        if step != 0.0 {
            // Stepped from the whole hour, so a slider dropped at 14:15 steps
            // to 15:00 rather than carrying the quarter along.
            let from = if step > 0.0 { editor.schedule_time_h.floor() } else { editor.schedule_time_h.ceil() };
            editor.schedule_time_h = (from + step).max(0.0);
            // Followed when stepped past an edge, so the keys never walk it
            // somewhere it cannot be seen.
            let seconds = editor.schedule_time_h * GanttView::HOUR;
            if seconds < editor.gantt.start_seconds || seconds > editor.gantt.end_seconds() {
                editor.gantt.pan(seconds - editor.gantt.start_seconds - editor.gantt.span_seconds / 2.0);
            }
        }
    }

    let ruler_response = ui
        .interact(ruler, ui.id().with("gantt_ruler"), egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    // A click is read as well as a press: a quick one can go down and up
    // inside a single frame.
    if (ruler_response.is_pointer_button_down_on() || ruler_response.dragged() || ruler_response.clicked())
        && let Some(pointer) = ruler_response.interact_pointer_pos().or_else(|| ui.input(|input| input.pointer.interact_pos()))
    {
        editor.schedule_time_h = to_hours(snap_slider(editor.gantt.seconds_at(pointer.x, body.left(), body.width()), interval));
    }

    let seconds = editor.schedule_time_h.max(0.0) * GanttView::HOUR;
    let x = editor.gantt.x_of(seconds, body.left(), body.width());
    let label = instant_label(seconds);
    let font = egui::FontId::proportional(12.0);
    let text_color = on_fill(color);
    let galley = ui.painter().layout_no_wrap(label, font, text_color);
    let label_w = galley.size().x + 12.0;
    let label_top = ruler.top() + 2.0;

    if !(body.left()..=body.right()).contains(&x) {
        // Out of view: a pinned label at the edge it lies beyond.
        let left_side = x < body.left();
        let arrow = if left_side { "‹ " } else { " ›" };
        let galley = ui.painter().layout_no_wrap(
            if left_side {
                format!("{arrow}{}", instant_label(seconds))
            } else {
                format!("{}{arrow}", instant_label(seconds))
            },
            egui::FontId::proportional(12.0),
            text_color,
        );
        let width = galley.size().x + 12.0;
        let min_x = if left_side { ruler.left() + 2.0 } else { ruler.right() - 2.0 - width };
        let pill = egui::Rect::from_min_size(egui::pos2(min_x, label_top), egui::vec2(width, SLIDER_LABEL_H));
        let response = ui
            .interact(pill, ui.id().with("gantt_slider_away"), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(tr!("gantt-slider-away"));
        if response.clicked() {
            editor.gantt.pan(seconds - editor.gantt.start_seconds - editor.gantt.span_seconds / 2.0);
        }
        let fill = if response.hovered() { color } else { color.gamma_multiply(0.8) };
        ui.painter().rect_filled(pill, GROUP_CORNER_RADIUS, fill);
        ui.painter().galley(pill.center() - galley.size() / 2.0, galley, text_color);
        return;
    }

    let grab = egui::Rect::from_min_max(egui::pos2(x - SLIDER_GRAB_W / 2.0, ruler.bottom()), egui::pos2(x + SLIDER_GRAB_W / 2.0, body.bottom()));
    let response = ui
        .interact(grab, ui.id().with("gantt_slider"), egui::Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        editor.schedule_time_h = to_hours(snap_slider(editor.gantt.seconds_at(pointer.x, body.left(), body.width()), interval));
    }
    let active = response.dragged() || response.hovered() || ruler_response.dragged() || ruler_response.hovered();
    let line = if active { color } else { color.gamma_multiply(0.8) };
    // Re-read: a press this frame has moved it.
    let x = editor.gantt.x_of(editor.schedule_time_h * GanttView::HOUR, body.left(), body.width());
    let painter = ui.painter();
    painter.line_segment([egui::pos2(x, label_top + SLIDER_LABEL_H), egui::pos2(x, body.bottom())], egui::Stroke::new(1.5, line));
    // The label is the handle, kept inside the ruler at either end.
    let left = (x - label_w / 2.0).clamp(ruler.left() + 1.0, (ruler.right() - label_w - 1.0).max(ruler.left() + 1.0));
    let pill = egui::Rect::from_min_size(egui::pos2(left, label_top), egui::vec2(label_w, SLIDER_LABEL_H));
    painter.rect_filled(pill, GROUP_CORNER_RADIUS, line);
    let galley = painter.layout_no_wrap(instant_label(editor.schedule_time_h * GanttView::HOUR), egui::FontId::proportional(12.0), text_color);
    painter.galley(pill.center() - galley.size() / 2.0, galley, text_color);
}

/// The time ruler: minor ticks with their labels, and - while the minor ticks
/// are finer than a day - a band of days above grouping them.
pub(super) fn draw_ruler(ui: &egui::Ui, rect: egui::Rect, view: GanttView, interval: f64, day_band: bool) {
    if !rect.is_positive() {
        return;
    }
    let painter = ui.painter_at(rect);
    let rule = ui.visuals().widgets.noninteractive.bg_stroke;
    let text_color = ui.visuals().weak_text_color();
    let minor_band = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - RULER_BAND.min(rect.height())), rect.max);

    if day_band {
        // Whole days across the top. Drawn from the first day boundary at or
        // before the left edge so a part-visible day still carries its label.
        let first = (view.start_seconds / GanttView::DAY).floor();
        let mut day = first;
        while day * GanttView::DAY <= view.end_seconds() {
            let start = day * GanttView::DAY;
            let left = view.x_of(start, rect.left(), rect.width()).max(rect.left());
            let right = view.x_of(start + GanttView::DAY, rect.left(), rect.width()).min(rect.right());
            if right - left > 28.0 {
                painter.text(
                    egui::pos2(left + 6.0, rect.top() + RULER_BAND * 0.5),
                    egui::Align2::LEFT_CENTER,
                    tr!("gantt-day", day = day_of(start + 1.0).to_string()),
                    egui::TextStyle::Body.resolve(ui.style()),
                    text_color,
                );
            }
            if left > rect.left() {
                painter.line_segment([egui::pos2(left, rect.top()), egui::pos2(left, rect.bottom())], rule);
            }
            day += 1.0;
        }
        painter.line_segment([minor_band.left_top(), minor_band.right_top()], rule);
    }

    for tick in view.visible_ticks(interval) {
        let x = view.x_of(tick, rect.left(), rect.width());
        painter.line_segment([egui::pos2(x, minor_band.bottom() - 5.0), egui::pos2(x, minor_band.bottom())], rule);
        let label = if interval < GanttView::DAY {
            time_of(tick)
        } else {
            tr!("gantt-day", day = day_of(tick).to_string())
        };
        painter.text(
            egui::pos2(x + 4.0, minor_band.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::TextStyle::Body.resolve(ui.style()),
            text_color,
        );
    }
}

/// The agent column and the row bands beside it, drawn together so a row's
/// name and its lane cannot come apart.
fn draw_rows(ui: &mut egui::Ui, header: egui::Rect, body: egui::Rect, stripe: egui::Color32, editor: &EditorState, rows: &[Row]) {
    if !body.is_positive() {
        return;
    }
    let rule = ui.visuals().widgets.noninteractive.bg_stroke;
    let lane_rule = egui::Stroke::new(1.0, rule.color.gamma_multiply(0.5));
    let text_color = ui.visuals().text_color();
    let weak = ui.visuals().weak_text_color();
    let scroll = editor.gantt.row_scroll;

    for (index, row) in rows.iter().enumerate() {
        let top = body.top() + row.top - scroll;
        // Only the rows the body can show are visited: several hundred agents
        // cost the same as a handful.
        if top + row.height < body.top() || top > body.bottom() {
            continue;
        }
        let band = egui::Rect::from_min_max(egui::pos2(body.left(), top), egui::pos2(body.right(), top + row.height));
        let name_cell = egui::Rect::from_min_max(egui::pos2(header.left(), top), egui::pos2(header.right(), top + row.height));
        if index % 2 == 1 {
            ui.painter_at(body).rect_filled(band, 0.0, stripe);
            ui.painter_at(header).rect_filled(name_cell, 0.0, stripe);
        }
        // Lane separators inside the row, lighter than the row rule, so the
        // priority lanes read as divisions of one machine rather than as
        // machines of their own.
        for lane in row.lanes.iter().skip(1) {
            let y = top + lane.offset;
            ui.painter_at(body).line_segment([egui::pos2(body.left(), y), egui::pos2(body.right(), y)], lane_rule);
        }
        ui.painter_at(body).line_segment([band.left_bottom(), band.right_bottom()], rule);
        ui.painter_at(header).line_segment([name_cell.left_bottom(), name_cell.right_bottom()], rule);

        // Truncated with the full text on hover, so a long machine name is
        // still readable without widening the column.
        let width = (name_cell.width() - 16.0).max(0.0);
        let name = egui::WidgetText::from(bold(&row.title).color(text_color)).into_galley(ui, Some(egui::TextWrapMode::Truncate), width, egui::TextStyle::Body);
        let detail = egui::WidgetText::from(egui::RichText::new(&row.subtitle).color(weak)).into_galley(ui, Some(egui::TextWrapMode::Truncate), width, egui::TextStyle::Body);
        let painter = ui.painter_at(header);
        painter.galley(egui::pos2(name_cell.left() + 8.0, name_cell.top() + 4.0), name, text_color);
        painter.galley(egui::pos2(name_cell.left() + 8.0, name_cell.top() + 4.0 + ROW_HEIGHT * 0.45), detail, weak);
        // Which lane is which, in the header beside the lanes themselves, so a
        // bar's priority can be read off the Gantt rather than out of a menu.
        if row.lanes.len() > 1 {
            for lane in &row.lanes {
                let label = tr!("schedule-bar-lane", lane = lane.priority.to_string());
                painter.text(
                    egui::pos2(name_cell.right() - 6.0, top + lane.offset + lane.height * 0.5),
                    egui::Align2::RIGHT_CENTER,
                    label,
                    egui::TextStyle::Body.resolve(ui.style()),
                    weak,
                );
            }
        }
        let hover = name_cell.intersect(header);
        if hover.is_positive() {
            ui.interact(hover, ui.id().with(("gantt_row", index)), egui::Sense::hover()).on_hover_text(tr_format!(
                literal = "%name%\n%detail%",
                name = row.title.clone(),
                detail = row.subtitle.clone()
            ));
        }
    }
}

/// The label a bar carries wherever it is named, for callers that hold the
/// editor rather than the bar's own report. One rule in one place: the Gantt
/// marker, the sequence editor's title and its discard prompt cannot end up
/// calling the same bar three different things.
pub(crate) fn bar_display_name(editor: &EditorState, bar: &ScheduleBar) -> String {
    bar_label(bar, editor.schedule_bar_reports.iter().find(|report| report.bar == bar.id), None)
}

/// What one bar's marker says.
///
/// A bar the user has named says that name and nothing else. An unnamed one
/// borrows the ground-derived label the readiness report built from its
/// members' pit, bench and blast. Neither carries tonnes or a block count:
/// the marker names the work, and the figures belong in the hover.
fn bar_label(bar: &ScheduleBar, report: Option<&ScheduleBarView>, plan: Option<&SchedulePlan>) -> String {
    // A delay is named by its type unless the user named it.
    if let Some(work) = bar.delay() {
        return if bar.has_custom_name() {
            bar.name().to_owned()
        } else {
            plan.map_or_else(|| tr!("gantt-palette-delay"), |plan| super::schedule_delays::delay_look(plan, work.kind).1)
        };
    }
    if bar.has_custom_name() {
        if bar.reclaim().is_some() {
            format!("↺ {}", bar.name())
        } else {
            bar.name().to_owned()
        }
    } else if bar.reclaim().is_some() {
        format!(
            "↺ {}",
            report
                .map(|report| report.default_name.clone())
                .unwrap_or_else(|| tr!("reclaim-bar-default-name", stockpile = tr!("destination-unresolved")))
        )
    } else if bar.blast_order().is_some() {
        format!("✹ {}", report.map(|report| report.default_name.clone()).unwrap_or_else(|| tr!("blast-bar-default-empty")))
    } else {
        report.map(|report| report.default_name.clone()).unwrap_or_else(|| tr!(literal = "Dig sequence"))
    }
}

/// Everything the bar's hover says: what it is, the period it may be worked
/// in, what a current run made of it, and why it is not ready.
fn bar_tooltip(bar: &ScheduleBar, report: Option<&ScheduleBarView>, window: WorkWindow, schedule: Option<&CalculatedSchedule>, plan: &SchedulePlan) -> String {
    let span = match window.end_h {
        Some(end) => tr!(
            "schedule-gantt-window-span",
            from = instant_label(window.start_h * GanttView::HOUR),
            to = instant_label(end * GanttView::HOUR)
        ),
        None => tr!("schedule-gantt-window-open", from = instant_label(window.start_h * GanttView::HOUR)),
    };
    let mut lines = vec![
        bar_label(bar, report, Some(plan)),
        tr_format!(literal = "%label%: %span%", label = tr!("schedule-gantt-window"), span = span),
    ];
    if bar.delay().is_some() {
        lines.push(tr!("gantt-delay-bar-note"));
        return lines.join("\n");
    }
    if let Some(work) = bar.reclaim() {
        // The permitted piles, named. The marker cannot list them - it says a
        // count - so this is where a planner reads back what the bar may draw
        // on, in the order it was authored and with nothing read as priority.
        if let Some(report) = report
            && !report.reclaim_sources.is_empty()
        {
            lines.push(tr!("reclaim-sources-tooltip", stockpiles = report.reclaim_sources.join(", ")));
        }
        lines.push(match work.maximum_t {
            Some(maximum) => tr!("reclaim-maximum-value", tonnes = format!("{maximum:.3}")),
            None => tr!("reclaim-maximum-unlimited"),
        });
    }
    if let Some(schedule) = schedule {
        let worked = schedule.bar_tonnes(bar.id);
        if bar.reclaim().is_some() {
            let (drawn, cap) = schedule.reclaim_progress(bar.id, schedule.requested_end_h);
            lines.push(match cap {
                Some(cap) => tr!("schedule-bar-reclaimed-of", tonnes = format_tonnes(drawn), cap = format_tonnes(cap)),
                None => tr!("schedule-bar-reclaimed", tonnes = format_tonnes(drawn)),
            });
        } else {
            lines.push(tr!("schedule-bar-worked", tonnes = format_tonnes(worked)));
            // Material, not lost work: it stays in the block, and any later
            // bar referencing that ground - this machine's or another's - may
            // take it.
            let left_behind = schedule.bar_left_behind(bar.id);
            if left_behind > 0.0 {
                lines.push(tr!("schedule-bar-left-behind", tonnes = format_tonnes(left_behind)));
            } else if let Some(completed) = schedule.bar_completion_h(bar.id)
                && window.end_h.is_some_and(|close| completed < close)
            {
                lines.push(tr!("schedule-bar-finished-early"));
            }
        }
    }
    // Every stated problem, in full. These are the reasons a schedule cannot
    // be calculated from this bar, and a truncated badge cannot carry them.
    if let Some(report) = report {
        lines.extend(report.problems.iter().cloned());
    }
    lines.push(tr!("schedule-bar-execution-note"));
    lines.join("\n")
}

/// Where a span of project time falls, as a rectangle of the given band.
fn span_rect(view: GanttView, body: egui::Rect, start_h: f64, end_h: f64, top: f32, height: f32) -> egui::Rect {
    let left = view.x_of(start_h * GanttView::HOUR, body.left(), body.width());
    let right = view.x_of(end_h * GanttView::HOUR, body.left(), body.width());
    // A zero-length span is a tick at its instant rather than nothing at all:
    // a block measured at zero tonnes is a real answer, executed in no time.
    let right = if right - left < 1.0 { left + 1.0 } else { right };
    egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, top + height))
}

/// The one colour a loader working a sequence is drawn in.
///
/// One colour, not a palette keyed by bar or block: the band sits on the bar
/// it belongs to, and which block it was on is the hover's to say.
pub(super) const WORKING_COLOR: egui::Color32 = egui::Color32::from_rgb(0x2E, 0xA0, 0xD6);
/// Reclaim: the same band in a second hue, so drawing a pile down never reads
/// as digging ground.
pub(super) const RECLAIM_COLOR: egui::Color32 = egui::Color32::from_rgb(0x3F, 0xB5, 0x8A);
/// Idle: a muted amber along the foot of the row, distinct from the working
/// band above the bar at a glance and at a small size. One colour whatever
/// the reason; the reason is the hover's to say.
pub(super) const IDLE_COLOR: egui::Color32 = egui::Color32::from_rgb(0xE8, 0xC0, 0x4A);
/// Drill and blast bars, and the palette chip that makes them.
pub(super) const BLAST_COLOR: egui::Color32 = egui::Color32::from_rgb(0xC9, 0x7B, 0x4A);

/// The band colour of each drill and blast step: prep, drill, charge.
pub(super) fn blast_activity_color(activity: crate::model::schedule::BlastActivity) -> egui::Color32 {
    match activity {
        crate::model::schedule::BlastActivity::Prep => egui::Color32::from_rgb(0xB8, 0x9A, 0x6A),
        crate::model::schedule::BlastActivity::Drill => egui::Color32::from_rgb(0xE0, 0x86, 0x3A),
        crate::model::schedule::BlastActivity::Charge => egui::Color32::from_rgb(0xD2, 0x4B, 0x4B),
    }
}

/// The published blasts a blast bar names, by position.
pub(super) fn bar_blasts(bar: &ScheduleBar, schedule: &CalculatedSchedule) -> Vec<usize> {
    let (Some(order), Some(result)) = (bar.blast_order(), schedule.drill_blast.as_ref()) else {
        return Vec::new();
    };
    order
        .members
        .iter()
        .filter_map(|member| {
            result.blasts.iter().position(|blast| {
                blast.reference.solid == member.solid
                    && (blast.reference.bench - member.bench).abs() < 1e-6
                    && crate::model::arrangement::point_in_face(&blast.face, glam::DVec2::from(member.anchor))
            })
        })
        .collect()
}

/// The short name and the explanation of one idle reason.
pub(super) fn idle_reason_text(reason: Option<IdleReason>) -> (String, String) {
    match reason {
        Some(IdleReason::Delayed) => (tr!("idle-delayed"), tr!("idle-delayed-note")),
        Some(IdleReason::Unavailable) => (tr!("idle-unavailable"), tr!("idle-unavailable-note")),
        Some(IdleReason::NoWork) => (tr!("idle-no-work"), tr!("idle-no-work-note")),
        Some(IdleReason::WorkFinished) => (tr!("idle-work-finished"), tr!("idle-work-finished-note")),
        Some(IdleReason::NoRoute) => (tr!("idle-no-route"), tr!("idle-no-route-note")),
        Some(IdleReason::DestinationsFull) => (tr!("idle-destinations-full"), tr!("idle-destinations-full-note")),
        Some(IdleReason::PileMode) => (tr!("idle-pile-mode"), tr!("idle-pile-mode-note")),
        Some(IdleReason::NoTrucks) => (tr!("idle-no-trucks"), tr!("idle-no-trucks-note")),
        Some(IdleReason::NotWorthIt) => (tr!("idle-not-worth-it"), tr!("idle-not-worth-it-note")),
        Some(IdleReason::WaitingOnBlast) => (tr!("idle-waiting-on-blast"), tr!("idle-waiting-on-blast-note")),
        None => (tr!("schedule-dispatch-idle"), String::new()),
    }
}

/// The idle strip along the foot of each loader's row: where that machine
/// executed nothing inside the calculated horizon, with the reason on hover.
///
/// Drawn from the result's explicit idle spans, never inferred from gaps
/// between bars - a gap on screen can be a lane that packed elsewhere.
fn draw_idle(ui: &mut egui::Ui, body: egui::Rect, view: GanttView, scroll: f32, schedule: Option<&CalculatedSchedule>, rows: &[Row], destinations: &[DestinationView]) {
    let Some(schedule) = schedule else {
        return;
    };
    let painter = ui.painter_at(body);
    for row in rows.iter().filter(|row| !row.drill_blast) {
        let Some(agent) = row.agent else {
            continue;
        };
        let top = body.top() + row.top - scroll + row.height - IDLE_SPACE + (IDLE_SPACE - IDLE_STRIP) * 0.5;
        if top > body.bottom() || top + IDLE_STRIP < body.top() {
            continue;
        }
        // A machine with no bars at all never reached the solve, so the
        // result holds nothing for it; over the whole horizon it had no work.
        let unassigned = [IdleSpan {
            agent,
            start_h: 0.0,
            end_h: schedule.requested_end_h,
            reason: Some(IdleReason::NoWork),
            full: Vec::new(),
            blast: None,
        }];
        let idle: &[IdleSpan] = if schedule.executions.iter().any(|execution| execution.agent == agent) || schedule.idle.iter().any(|span| span.agent == agent) {
            &schedule.idle
        } else {
            &unassigned
        };
        // Delays are drawn as delays, not as idle time.
        for span in idle.iter().filter(|span| span.agent == agent && span.reason != Some(IdleReason::Delayed)) {
            let rect = span_rect(view, body, span.start_h, span.end_h, top, IDLE_STRIP);
            if rect.intersects(body) {
                painter.rect_filled(rect.intersect(body), 0.0, IDLE_COLOR);
            }
        }

        // One hover target for the whole strip, taller than the line itself,
        // answering for the instant under the pointer.
        let target = egui::Rect::from_min_max(egui::pos2(body.left(), top - 3.0), egui::pos2(body.right(), top + IDLE_STRIP + 3.0)).intersect(body);
        if !target.is_positive() {
            continue;
        }
        let response = ui.interact(target, ui.id().with(("gantt_idle", agent)), egui::Sense::hover());
        let Some(pos) = response.hover_pos() else {
            continue;
        };
        let at_h = view.seconds_at(pos.x, body.left(), body.width()) / GanttView::HOUR;
        let Some(span) = idle
            .iter()
            .find(|span| span.agent == agent && span.reason != Some(IdleReason::Delayed) && span.start_h <= at_h && at_h < span.end_h)
        else {
            continue;
        };
        response.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(420.0);
            let (name, note) = idle_reason_text(span.reason);
            ui.label(bold(&tr!("idle-title", reason = name)));
            ui.label(tr!(
                "schedule-span-hours",
                from = instant_label(span.start_h * GanttView::HOUR),
                to = instant_label(span.end_h * GanttView::HOUR),
                hours = format!("{:.1}", span.end_h - span.start_h)
            ));
            if let Some(blast) = span.blast.and_then(|index| schedule.drill_blast.as_ref()?.blasts.get(index)) {
                ui.label(tr!("idle-waiting-on-blast-named", blast = blast_title(blast)));
                if let Some(fired) = blast.fired_h {
                    ui.label(tr!("blast-fired-at", at = instant_label(fired * GanttView::HOUR)));
                }
            }
            if !span.full.is_empty() {
                let names: Vec<String> = span.full.iter().map(|id| destination_label(*id, destinations)).collect();
                ui.label(if span.reason == Some(IdleReason::PileMode) {
                    tr!("idle-pile-list", destinations = names.join(", "))
                } else {
                    tr!("idle-full-list", destinations = names.join(", "))
                });
            }
            if !note.is_empty() {
                ui.label(egui::RichText::new(note).weak());
            }
        });
    }
}

/// The bar member a solved block is, as the readiness report currently
/// describes it.
fn member_of<'a>(
    schedule: &CalculatedSchedule,
    bar: &ScheduleBar,
    report: Option<&'a ScheduleBarView>,
    block: crate::model::DigBlockId,
) -> Option<&'a crate::ui::state::ScheduleMemberView> {
    schedule
        .bar_blocks
        .iter()
        .find(|(id, _)| *id == bar.id)
        .and_then(|(_, blocks)| blocks.iter().position(|held| *held == block))
        .and_then(|position| report.and_then(|report| report.members.get(position)))
}

/// What one solved source is called, resolved now rather than when the run
/// was made.
fn source_label(source: WorkSource, bar: &ScheduleBar, report: Option<&ScheduleBarView>, schedule: &CalculatedSchedule, destinations: &[DestinationView]) -> String {
    match source {
        WorkSource::Block(block) => member_of(schedule, bar, report, block)
            .and_then(|member| member.name.clone())
            .unwrap_or_else(|| tr!("schedule-dispatch-block", block = block.0.to_string())),
        WorkSource::Stockpile(pile) => destination_label(pile, destinations),
    }
}

/// A source as a planner names it: a block with the area it lies in.
pub(super) fn qualified_source_label(
    source: WorkSource,
    bar: &ScheduleBar,
    report: Option<&ScheduleBarView>,
    schedule: &CalculatedSchedule,
    destinations: &[DestinationView],
) -> String {
    let name = source_label(source, bar, report, schedule, destinations);
    match source {
        WorkSource::Block(block) => match member_of(schedule, bar, report, block).and_then(|member| member.area.clone()) {
            Some(area) => format!("{area} · {name}"),
            None => name,
        },
        WorkSource::Stockpile(_) => name,
    }
}

pub(super) fn destination_label(id: DestinationId, destinations: &[DestinationView]) -> String {
    destinations
        .iter()
        .find(|entry| entry.id == id)
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| tr!("destination-unresolved"))
}

/// Everything one execution band's hover says: the source, the tonnes and
/// times the model solved, the rate that implies, who else was on the same
/// ground, what is left, and where the material went.
///
/// Built only on hover: it scans the result, and nothing that scans the
/// result belongs on the paint path.
pub(super) fn execution_tooltip(
    execution: &Execution,
    bar: &ScheduleBar,
    report: Option<&ScheduleBarView>,
    schedule: &CalculatedSchedule,
    plan: &SchedulePlan,
    destinations: &[DestinationView],
) -> String {
    let kind = match execution.activity {
        Activity::Dig => tr!("schedule-dispatch-execution"),
        Activity::Reclaim => tr!("schedule-dispatch-reclaim"),
    };
    let source = qualified_source_label(execution.source, bar, report, schedule, destinations);
    let mut lines = vec![
        tr!("schedule-dispatch-heading", kind = kind, source = source),
        tr_format!(
            literal = "%from% → %to%",
            from = instant_label(execution.start_h * GanttView::HOUR),
            to = instant_label(execution.end_h * GanttView::HOUR)
        ),
        tr!(
            "schedule-dispatch-tonnes-rate",
            tonnes = format_tonnes(execution.tonnes),
            rate = format_tonnes(execution.rate_tph().round())
        ),
    ];
    match execution.source {
        WorkSource::Block(block) => {
            // Concurrent mining is inspectable rather than merely permitted.
            let mut sharers: Vec<LoaderAgentId> = schedule
                .executions
                .iter()
                .filter(|other| other.source == execution.source && other.agent != execution.agent && other.start_h < execution.end_h && other.end_h > execution.start_h)
                .map(|other| other.agent)
                .collect();
            sharers.sort();
            sharers.dedup();
            if !sharers.is_empty() {
                let agents = sharers
                    .iter()
                    .map(|agent| plan.agent(*agent).map(|agent| agent.name.clone()).unwrap_or_else(|| format!("{}", agent.0)))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(tr!("schedule-dispatch-shared-with", agents = agents));
            }
            if let Some(balance) = schedule.ground(block) {
                let taken_by_then: f64 = schedule
                    .executions
                    .iter()
                    .filter(|other| other.source == execution.source && other.end_h <= execution.end_h + 1e-9)
                    .map(|other| other.tonnes)
                    .sum();
                let left = (balance.started_t - taken_by_then).max(0.0);
                lines.push(if left > 1e-6 {
                    tr!("schedule-dispatch-remaining", tonnes = format_tonnes(left))
                } else {
                    tr!("schedule-dispatch-emptied")
                });
            }
        }
        WorkSource::Stockpile(pile) => {
            let (drawn, cap) = schedule.reclaim_progress(execution.bar, execution.end_h);
            lines.push(match cap {
                Some(cap) => tr!("schedule-bar-reclaimed-of", tonnes = format_tonnes(drawn), cap = format_tonnes(cap)),
                None => tr!("schedule-bar-reclaimed", tonnes = format_tonnes(drawn)),
            });
            // Which chunks the pile gave up in these intervals, when it is an
            // ordered chunked pile: the actual source choice, not the permitted
            // set.
            let chunks: Vec<String> = schedule
                .chunk_draws_during(pile, execution.start_h, execution.end_h)
                .map(|draw| format!("{} ({:.1} t)", draw.label, draw.tonnes))
                .collect();
            if !chunks.is_empty() {
                lines.push(tr!("schedule-dispatch-chunks", chunks = chunks.join(", ")));
            }
            if let Some((held, _)) = schedule.inventory_at(pile, execution.end_h) {
                lines.push(tr!(
                    "schedule-dispatch-pile-holds",
                    stockpile = destination_label(pile, destinations),
                    tonnes = format_tonnes(held)
                ));
            }
        }
    }
    let mut haul = crate::model::schedule::result::HaulSummary::default();
    for delivery in schedule.deliveries_of(execution) {
        let duration = delivery.end_h - delivery.start_h;
        if duration > 0.0 {
            haul.add(
                delivery,
                (delivery.end_h.min(execution.end_h) - delivery.start_h.max(execution.start_h)).max(0.0) / duration,
            );
        }
    }
    if haul.tonnes > 0.0 {
        lines.push(tr!("haul-cycle-minutes", minutes = format!("{:.1}", haul.cycle_minutes())));
    }
    // Where it went, by destination.
    let mut delivered: Vec<(DestinationId, f64)> = Vec::new();
    for delivery in schedule.deliveries_of(execution) {
        let overlap = delivery.end_h.min(execution.end_h) - delivery.start_h.max(execution.start_h);
        let tonnes = delivery.tonnes * overlap / (delivery.end_h - delivery.start_h);
        match delivered.iter_mut().find(|(id, _)| *id == delivery.destination) {
            Some((_, total)) => *total += tonnes,
            None => delivered.push((delivery.destination, tonnes)),
        }
    }
    for (destination, tonnes) in delivered {
        lines.push(tr!(
            "schedule-dispatch-delivered",
            destination = destination_label(destination, destinations),
            tonnes = format_tonnes(tonnes)
        ));
    }
    lines.join("\n")
}

/// What one frame of pointer input does to a drag already in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DragStep {
    /// Still held: the preview is here now.
    Continue(GanttDrag),
    /// Released: commit this, once.
    Commit(GanttDrag),
    /// Abandoned. Escape, or a release that happened while the Gantt was not
    /// on screen - a page switch mid-drag. Nothing is committed.
    Abandon,
}

/// Advance a drag by one frame of pointer input.
///
/// Deliberately separate from the bar that started it. A bar is culled when it
/// leaves the body - dragged past an edge, or scrolled out of the rows - and a
/// drag that could only be finished by its own culled widget would never
/// finish: the pointer would come up with the edit uncommitted and the drag
/// still live, and the next click would move a bar the user was no longer
/// dragging.
fn advance_drag(drag: GanttDrag, pointer_seconds: Option<f64>, released: bool, down: bool, cancelled: bool) -> DragStep {
    if cancelled {
        return DragStep::Abandon;
    }
    let mut drag = drag;
    if let Some(seconds) = pointer_seconds {
        let (start, end) = match drag.mode {
            // The whole window moves, keeping its length: a bar dragged along
            // the timeline is the same period of work somewhere else, not a
            // longer or shorter one.
            GanttDragMode::Move => {
                // Never before the schedule origin: time does not run
                // backwards, and a window clamped at zero is what the domain
                // would accept anyway.
                let start = (seconds - drag.grab_offset_seconds).max(0.0);
                let span = drag.from_end_seconds.map(|end| end - drag.from_start_seconds);
                (start, span.map(|span| start + span))
            }
            GanttDragMode::ResizeStart => {
                let limit = drag.preview_end_seconds.map_or(f64::MAX, |end| end - MIN_WINDOW_SECONDS);
                (seconds.max(0.0).min(limit), drag.preview_end_seconds)
            }
            GanttDragMode::ResizeEnd => (drag.preview_start_seconds, Some(seconds.max(drag.preview_start_seconds + MIN_WINDOW_SECONDS))),
        };
        if (start, end) != (drag.preview_start_seconds, drag.preview_end_seconds) {
            drag.preview_start_seconds = start;
            drag.preview_end_seconds = end;
            drag.moved = true;
        }
    }
    if released {
        DragStep::Commit(drag)
    } else if down {
        DragStep::Continue(drag)
    } else {
        // The button is not down and no release arrived: it came up somewhere
        // this page did not see.
        DragStep::Abandon
    }
}

/// How far the window follows a drag that has reached its edge over
/// `dt_seconds` of elapsed time. Zero everywhere but the last
/// [`EDGE_PAN_MARGIN`] points, so a drag inside the timeline is never nudged.
fn edge_pan_seconds(pointer_x: f32, left: f32, right: f32, span_seconds: f64, dt_seconds: f32) -> f64 {
    if right - left <= EDGE_PAN_MARGIN * 2.0 || !span_seconds.is_finite() {
        return 0.0;
    }
    let step = f64::from(dt_seconds.clamp(0.0, EDGE_PAN_MAX_STEP));
    if step <= 0.0 {
        return 0.0;
    }
    let past = if pointer_x < left + EDGE_PAN_MARGIN {
        pointer_x - (left + EDGE_PAN_MARGIN)
    } else if pointer_x > right - EDGE_PAN_MARGIN {
        pointer_x - (right - EDGE_PAN_MARGIN)
    } else {
        return 0.0;
    };
    (f64::from(past) / f64::from(EDGE_PAN_RANGE)).clamp(-1.0, 1.0) * span_seconds * EDGE_PAN_SPAN_PER_SECOND * step
}

/// Which part of a bar a press at `pointer_x` has taken hold of.
///
/// An open-ended bar has no right edge on screen - what is drawn there is the
/// edge of the pane - so it offers no resize grip there. Its end is set by
/// typing one, which is the only place it can be said exactly.
fn drag_mode_at(pointer_x: f32, left: f32, right: f32, bounded: bool) -> GanttDragMode {
    if bounded && pointer_x >= right - RESIZE_GRIP {
        GanttDragMode::ResizeEnd
    } else if pointer_x <= left + RESIZE_GRIP {
        GanttDragMode::ResizeStart
    } else {
        GanttDragMode::Move
    }
}

/// What a vertical drag at `pointer_y` is pointing at.
///
/// The seams are read before the lanes themselves. Lanes sit flush against one
/// another, so without that a pointer between two of them always lands in one
/// of the two and a new lane could never be asked for at all - which is
/// exactly what it looked like from the Gantt.
fn placement_at(pointer_y: f32, body: egui::Rect, scroll: f32, rows: &[Row]) -> Option<Placement> {
    let row = rows.iter().min_by(|left, right| {
        let distance = |row: &Row| {
            let top = body.top() + row.top - scroll;
            if pointer_y < top {
                top - pointer_y
            } else if pointer_y > top + row.height {
                pointer_y - top - row.height
            } else {
                0.0
            }
        };
        distance(left).total_cmp(&distance(right))
    })?;
    if row.blasting {
        return None;
    }
    let row_top = body.top() + row.top - scroll;
    let local = (pointer_y - row_top).clamp(0.0, row.height);
    let insert = |priority: u32| {
        Some(Placement {
            agent: row.agent,
            priority,
            insert: true,
        })
    };
    let first = row.lanes.first().expect("every row has a lane");
    let last = row.lanes.last().expect("every row has a lane");
    if local <= LANE_INSERT_ZONE {
        return insert(first.priority);
    }
    if local >= last.offset + last.height - LANE_INSERT_ZONE {
        // One lane further out than the row's last: nothing has to move for
        // this one, whatever `insert_lane` then finds at or below it.
        return insert(last.priority.saturating_add(1));
    }
    if let Some(seam) = row.lanes.iter().skip(1).find(|lane| (local - lane.offset).abs() <= LANE_INSERT_ZONE) {
        return insert(seam.priority);
    }
    let lane = row
        .lanes
        .iter()
        .min_by(|left, right| {
            let centre = |lane: &Lane| lane.offset + lane.height * 0.5;
            (local - centre(left)).abs().total_cmp(&(local - centre(right)).abs())
        })
        .expect("every row has a lane");
    Some(Placement {
        agent: row.agent,
        priority: lane.priority,
        insert: false,
    })
}

/// Top of the slot a placement names, in screen points.
///
/// A lane that is only being asked for has no height of its own yet, so what
/// comes back for one is the seam it would open at.
fn placement_top(placement: Placement, body: egui::Rect, scroll: f32, rows: &[Row]) -> Option<f32> {
    let row = rows.iter().find(|row| row.agent == placement.agent)?;
    let top = body.top() + row.top - scroll;
    match row.lanes.iter().find(|lane| lane.priority == placement.priority) {
        Some(lane) => Some(top + lane.offset),
        None if placement.insert => row.lanes.last().map(|lane| top + lane.offset + lane.height),
        None => row.lanes.first().map(|lane| top + lane.offset),
    }
}

/// The bars themselves: drawn with their calculated work, hit-tested, dragged,
/// resized and right-clicked.
#[allow(
    clippy::too_many_arguments,
    reason = "one pass over the bars needs the frame's layout, the project it was read from, the run being drawn and somewhere to put the edits; splitting it would only move the list"
)]
fn draw_bars(
    ui: &mut egui::Ui,
    body: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    destinations: &[DestinationView],
    layout: &Layout,
    session: u32,
    commands: &mut Vec<UiCommand>,
    schedule: Option<&CalculatedSchedule>,
) {
    if !body.is_positive() {
        return;
    }
    let scroll = editor.gantt.row_scroll;
    let view = editor.gantt;
    let painter = ui.painter_at(body);
    let visuals = ui.visuals().clone();
    let mut selected = editor.schedule_selected_bar;
    let mut open_dialog: Option<BarNameDialog> = None;
    let mut open_window: Option<BarWindowDialog> = None;
    let mut open_editor: Option<crate::ui::state::SequenceDraft> = None;
    let mut open_reclaim: Option<ReclaimBarDialog> = None;
    let mut open_blast: Option<BlastBarDialog> = None;

    // A bar deleted - by an edit, by undo, or with the project it belonged
    // to - leaves nothing to move. The session is checked as well as the id
    // because a fresh project numbers its first bar `0` too, and a pointer
    // held down across a project switch must not release onto its successor.
    let mut drag = editor.gantt_drag.filter(|drag| drag.session == session && plan.bar(drag.bar).is_some());
    let mut released: Option<GanttDrag> = None;
    if let Some(active) = drag {
        let (pointer, down, up, cancelled) = ui.input_mut(|input| {
            (
                input.pointer.interact_pos(),
                input.pointer.button_down(egui::PointerButton::Primary),
                input.pointer.button_released(egui::PointerButton::Primary),
                // Consumed rather than observed, so Escape abandoning a drag
                // does not also close whatever else is listening for it.
                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        // egui's own smoothed frame interval, which already excludes the
        // first frame after a repaint gap.
        let dt = ui.input(|input| input.stable_dt);
        let seconds = pointer.map(|pos| view.seconds_at(pos.x, body.left(), body.width()));
        match advance_drag(active, seconds, up, down, cancelled) {
            DragStep::Continue(mut moved) => {
                // A vertical drag is a move in its own right: crossing into
                // another lane counts as having moved even when the bar's
                // time never changed, so the release commits the reassignment.
                if moved.mode == GanttDragMode::Move
                    && let Some(pos) = pointer
                    && let Some(placement) = placement_at(pos.y, body, scroll, &layout.rows)
                    && placement != previewed(moved)
                {
                    moved.preview_agent = placement.agent;
                    moved.preview_priority = placement.priority;
                    moved.preview_insert = placement.insert;
                    moved.moved = true;
                }
                drag = Some(moved);
                // Follow the pointer out past the edge instead of leaving a
                // dead zone there: a bar can be dragged to a time the window
                // does not currently show. The pointer may then sit still, so
                // the frame has to be asked for.
                if let Some(pos) = pointer {
                    let pan = edge_pan_seconds(pos.x, body.left(), body.right(), view.span_seconds, dt);
                    if pan != 0.0 {
                        editor.gantt.pan(pan);
                        ui.ctx().request_repaint();
                    }
                }
            }
            DragStep::Commit(mut finished) => {
                if finished.mode == GanttDragMode::Move
                    && let Some(pos) = pointer
                    && let Some(placement) = placement_at(pos.y, body, scroll, &layout.rows)
                {
                    finished.preview_agent = placement.agent;
                    finished.preview_priority = placement.priority;
                    finished.preview_insert = placement.insert;
                }
                drag = None;
                released = Some(finished);
            }
            DragStep::Abandon => drag = None,
        }
    }

    if let Some(active) = drag.filter(|drag| drag.mode == GanttDragMode::Move)
        && let Some(top) = placement_top(previewed(active), body, scroll, &layout.rows)
    {
        // A lane being joined is shown as the space the bar would occupy; a
        // lane being opened has no space yet, so what is shown is the seam it
        // would part - the space appears when the drag is let go.
        let slot = if active.preview_insert {
            egui::Rect::from_min_size(egui::pos2(body.left(), top - 1.5), egui::vec2(body.width(), 3.0))
        } else {
            egui::Rect::from_min_size(egui::pos2(body.left(), top), egui::vec2(body.width(), BAR_TOP + plan.bar_height() + BAR_BOTTOM))
        };
        let fill = if active.preview_insert {
            visuals.selection.stroke.color
        } else {
            visuals.selection.bg_fill.gamma_multiply(0.18)
        };
        painter.rect_filled(slot.intersect(body), GROUP_CORNER_RADIUS, fill);
    }

    for row in &layout.rows {
        let top = body.top() + row.top - scroll;
        if top + row.height < body.top() || top > body.bottom() {
            continue;
        }
        for lane in &row.lanes {
            let stack_height = lane.stack_height();
            for (stack, members) in lane.stacks.iter().enumerate() {
                for &index in members {
                    let bar = &plan.bars()[index];
                    let marker = &layout.markers[index];
                    let report = editor.schedule_bar_reports.iter().find(|report| report.bar == bar.id);
                    // The dragged bar is drawn where the drag has it, not where
                    // the project still holds it: the edit lands on release.
                    let (window, extent) = match drag {
                        Some(drag) if drag.bar == bar.id => {
                            let window = drag.preview_window();
                            (window, BarExtent::of(view, window, body))
                        }
                        _ => (bar.window, marker.extent),
                    };
                    let (left, right) = extent.drawn(body);
                    let right = right.max(left + MIN_BAR_WIDTH);
                    // Wholly off either side: nothing to draw and nothing to
                    // hit. An active drag is resolved above, before this, so
                    // culling a bar can no longer strand one.
                    if right < body.left() || left > body.right() {
                        continue;
                    }
                    let slot_top = match drag {
                        Some(drag) if drag.bar == bar.id && drag.mode == GanttDragMode::Move => {
                            placement_top(previewed(drag), body, scroll, &layout.rows).unwrap_or(top + lane.offset)
                        }
                        _ => top + lane.offset + stack_height * stack as f32,
                    };
                    let band = egui::Rect::from_min_max(egui::pos2(left, slot_top + 2.0), egui::pos2(right, slot_top + 2.0 + WORK_BAND));
                    let rect = egui::Rect::from_min_size(egui::pos2(left, slot_top + BAR_TOP), egui::vec2(right - left, plan.bar_height()));

                    let response = ui.interact(rect.intersect(body), ui.id().with(("gantt_bar", bar.id)), egui::Sense::click_and_drag());
                    let is_selected = selected == Some(bar.id);
                    let ready = report.is_some_and(|report| report.ready);
                    let delay = bar.delay().map(|work| super::schedule_delays::delay_look(plan, work.kind).0);
                    let fill = match delay {
                        Some(color) => color,
                        None if ready => visuals.selection.bg_fill,
                        None => visuals.widgets.inactive.bg_fill,
                    };
                    let stroke = if is_selected {
                        egui::Stroke::new(2.0, visuals.selection.stroke.color)
                    } else if report.is_some_and(|report| !report.problems.is_empty()) {
                        egui::Stroke::new(1.0, visuals.error_fg_color)
                    } else {
                        egui::Stroke::new(1.0, visuals.widgets.active.bg_stroke.color)
                    };
                    painter.rect_filled(rect, GROUP_CORNER_RADIUS, fill);

                    // Grey over the rest of a bar whose ground ran out before
                    // its window closed. It says *this bar finished early*,
                    // not *this loader idled* - the machine may well be
                    // working something else there, which its other bars show.
                    let finished_early = schedule.filter(|_| bar.reclaim().is_none()).and_then(|schedule| {
                        let completed = if bar.blast_order().is_some() {
                            blast_bar_completion_h(plan, bar, schedule)?
                        } else {
                            schedule.bar_completion_h(bar.id)?
                        };
                        (completed < window.end_h.unwrap_or(schedule.requested_end_h)).then_some(completed)
                    });
                    if let Some(end) = finished_early {
                        let from = view.x_of(end * GanttView::HOUR, body.left(), body.width()).max(rect.left());
                        let grey = egui::Rect::from_min_max(egui::pos2(from, rect.top()), rect.max);
                        if grey.is_positive() {
                            painter.rect_filled(grey, GROUP_CORNER_RADIUS, visuals.widgets.noninteractive.bg_fill.gamma_multiply(0.85));
                        }
                    }
                    painter.rect_stroke(rect, GROUP_CORNER_RADIUS, stroke, egui::StrokeKind::Inside);
                    // A hard tick on each edge of the window: the bar's fill
                    // is where it may be worked, and these are exactly when
                    // that starts and stops.
                    let edge = egui::Stroke::new(2.0, visuals.strong_text_color());
                    painter.line_segment([rect.left_top(), rect.left_bottom()], edge);
                    if window.end_h.is_some() {
                        painter.line_segment([rect.right_top(), rect.right_bottom()], edge);
                    }
                    // Clipped to the bar rather than allowed to overhang it:
                    // the bar is a period now, and a label spilling past its
                    // end would read as work that runs on past the window.
                    if rect.width() > 16.0 {
                        // On a delay's own colour the label is white or black,
                        // whichever reads.
                        let text = match delay {
                            Some(color) if (u32::from(color.r()) * 299 + u32::from(color.g()) * 587 + u32::from(color.b()) * 114) / 1000 > 150 => egui::Color32::BLACK,
                            Some(_) => egui::Color32::WHITE,
                            None => visuals.strong_text_color(),
                        };
                        ui.painter_at(rect.intersect(body)).galley_with_override_text_color(
                            egui::pos2(rect.left() + 6.0, rect.center().y - marker.galley.size().y * 0.5),
                            marker.galley.clone(),
                            text,
                        );
                    }
                    // The calculated work, in the band above the bar. Drawn
                    // only from a current run: the moment anything it was
                    // calculated from is edited, `schedule` is `None` here and
                    // the band is simply not there. One hover for the whole
                    // band, answering for the instant under the pointer.
                    if let Some(schedule) = schedule
                        && bar.blast_order().is_some()
                    {
                        draw_blast_band(ui, body, band, view, bar, schedule);
                    }
                    if let Some(schedule) = schedule {
                        let clip = band.intersect(body);
                        for execution in schedule.bar_executions(bar.id) {
                            let span = span_rect(view, body, execution.start_h, execution.end_h, band.top(), WORK_BAND);
                            if span.intersects(clip) {
                                let color = match execution.activity {
                                    Activity::Dig => WORKING_COLOR,
                                    Activity::Reclaim => RECLAIM_COLOR,
                                };
                                // Square-ended, so a run of hours reads as one
                                // unbroken line rather than beads.
                                ui.painter_at(clip).rect_filled(span, 0.0, color);
                            }
                        }
                        let target = band.expand2(egui::vec2(0.0, 2.0)).intersect(body);
                        if target.is_positive() {
                            let hover = ui.interact(target, ui.id().with(("gantt_work", bar.id)), egui::Sense::hover());
                            if let Some(pos) = hover.hover_pos() {
                                let at_h = view.seconds_at(pos.x, body.left(), body.width()) / GanttView::HOUR;
                                // Every span worked at that instant, which in an
                                // hour that finished one block and started the
                                // next is two.
                                let working: Vec<&Execution> = schedule
                                    .bar_executions(bar.id)
                                    .filter(|execution| execution.start_h <= at_h && at_h < execution.end_h)
                                    .collect();
                                if !working.is_empty() {
                                    hover.on_hover_ui_at_pointer(|ui| {
                                        ui.set_min_width(260.0);
                                        for (index, execution) in working.iter().enumerate() {
                                            if index > 0 {
                                                ui.separator();
                                            }
                                            ui.label(execution_tooltip(execution, bar, report, schedule, plan, destinations));
                                        }
                                    });
                                }
                            }
                        }
                    }
                    if response.clicked() {
                        selected = Some(bar.id);
                    }
                    // Primary only: the middle button pans the timeline and the
                    // secondary one opens this bar's menu.
                    if response.drag_started_by(egui::PointerButton::Primary) {
                        selected = Some(bar.id);
                        let pointer = response.interact_pointer_pos().map_or(left, |pos| pos.x);
                        let mode = drag_mode_at(pointer, left, right, window.end_h.is_some());
                        let start_seconds = window.start_h * GanttView::HOUR;
                        drag = Some(GanttDrag {
                            session,
                            bar: bar.id,
                            mode,
                            from_start_seconds: start_seconds,
                            from_end_seconds: window.end_h.map(|end| end * GanttView::HOUR),
                            grab_offset_seconds: view.seconds_at(pointer, body.left(), body.width()) - start_seconds,
                            preview_start_seconds: start_seconds,
                            preview_end_seconds: window.end_h.map(|end| end * GanttView::HOUR),
                            from_agent: bar.agent,
                            from_priority: bar.priority,
                            preview_agent: bar.agent,
                            preview_priority: bar.priority,
                            preview_insert: false,
                            moved: false,
                        });
                    }
                    // Said with the cursor before the press, so an edge is
                    // discoverable rather than something to be found out about
                    // by accidentally resizing a bar.
                    if let Some(pos) = response.hover_pos()
                        && !matches!(drag_mode_at(pos.x, left, right, window.end_h.is_some()), GanttDragMode::Move)
                    {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    let menu_label = bar_label(bar, report, Some(plan));
                    context_menu_popup(&response, &menu_label, |ui| {
                        if bar.dig_order().is_some() && ContextMenuAction::new(tr!("schedule-bar-edit-sequence")).show(ui).clicked() {
                            // Opening reads the bar and nothing else: the
                            // draft is editor state, and no geometry, run or
                            // pipeline demand is touched by opening a window.
                            open_editor = Some(crate::ui::state::SequenceDraft::open(session, bar.id, bar.members()));
                            editor.solids_view_selection.clear();
                            editor.selected_blast = None;
                            ui.close();
                        }
                        // A delay's type, chosen from the list the Delays page
                        // keeps; the current one is ticked.
                        if let Some(work) = bar.delay() {
                            ui.label(egui::RichText::new(tr!("gantt-delay-change-type")).weak());
                            for entry in &plan.delays().types {
                                if ContextMenuAction::new(entry.name.clone()).checked(work.kind == Some(entry.id)).show(ui).clicked() {
                                    commands.push(UiCommand::schedule(
                                        session,
                                        ScheduleEdit::SetDelayBarType {
                                            bar: bar.id,
                                            kind: Some(entry.id),
                                        },
                                    ));
                                    ui.close();
                                }
                            }
                            if ContextMenuAction::new(tr!("delay-untyped")).checked(work.kind.is_none()).show(ui).clicked() {
                                commands.push(UiCommand::schedule(session, ScheduleEdit::SetDelayBarType { bar: bar.id, kind: None }));
                                ui.close();
                            }
                            ui.separator();
                        }
                        if let Some(order) = bar.blast_order()
                            && ContextMenuAction::new(tr!("blast-edit-bar")).show(ui).clicked()
                        {
                            open_blast = Some(BlastBarDialog {
                                session,
                                edition: crate::ui::state::SequenceDraft::next_edition(),
                                generation: None,
                                opened_from: order.members.clone(),
                                view: crate::ui::state::SolidPreviewView::default(),
                                target: Some(bar.id),
                                members: order.members.clone(),
                                agent: bar.agent,
                                priority: bar.priority,
                                insert_lane: false,
                                start_h: bar.window.start_h,
                                end_h: bar.window.end_h.unwrap_or(bar.window.start_h + crate::model::schedule::SCHEDULE_PERIOD_H),
                                bench: None,
                            });
                            ui.close();
                        }
                        if let Some(work) = bar.reclaim()
                            && ContextMenuAction::new(tr!("reclaim-edit-bar")).show(ui).clicked()
                        {
                            open_reclaim = Some(ReclaimBarDialog {
                                target: Some(bar.id),
                                sources: work.sources.clone(),
                                agent: bar.agent,
                                priority: bar.priority,
                                start: bar.window.start_h.to_string(),
                                end: bar.window.end_h.map(|value| value.to_string()).unwrap_or_default(),
                                maximum: work.maximum_t.map(|value| value.to_string()).unwrap_or_default(),
                            });
                            ui.close();
                        }
                        // Dragging an edge is quick and imprecise; a real
                        // schedule is written in numbers. Both routes end in
                        // the same edit, validated the same way.
                        if ContextMenuAction::new(tr!("schedule-bar-set-window")).show(ui).clicked() {
                            open_window = Some(BarWindowDialog::of(bar.id, bar.window));
                            ui.close();
                        }
                        if ContextMenuAction::new(tr!("schedule-rename-bar-action")).show(ui).clicked() {
                            open_dialog = Some(BarNameDialog {
                                target: bar.id,
                                name: bar.name().to_owned(),
                            });
                            ui.close();
                        }
                        if ContextMenuAction::new(tr!("schedule-copy-bar")).show(ui).clicked() {
                            commands.push(UiCommand::schedule(session, ScheduleEdit::CopyBar(bar.id)));
                            ui.close();
                        }
                        if ContextMenuAction::new(tr!("schedule-delete-bar")).show(ui).clicked() {
                            commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteBar(bar.id)));
                            ui.close();
                        }
                        // Lanes are numbered from zero and lower is sooner, so
                        // "raise" is one step down the number. A bar already in
                        // lane 0 cannot be raised, which the row states rather
                        // than silently doing nothing.
                        if ContextMenuAction::new(tr!("schedule-bar-raise-priority")).enabled(bar.priority > 0).show(ui).clicked() {
                            commands.push(UiCommand::schedule(
                                session,
                                ScheduleEdit::SetBarPriority {
                                    bar: bar.id,
                                    priority: bar.priority - 1,
                                },
                            ));
                            ui.close();
                        }
                        if ContextMenuAction::new(tr!("schedule-bar-lower-priority"))
                            .enabled(bar.priority < u32::MAX)
                            .show(ui)
                            .clicked()
                        {
                            commands.push(UiCommand::schedule(
                                session,
                                ScheduleEdit::SetBarPriority {
                                    bar: bar.id,
                                    priority: bar.priority + 1,
                                },
                            ));
                            ui.close();
                        }
                        // Assignment is chosen by name, never read off where a
                        // drag was released: which machine digs this ground is a
                        // decision, and an accident here would move work between
                        // machines without the user having said so.
                        if ContextMenuAction::new(tr!("schedule-bar-unassign")).checked(bar.agent.is_none()).show(ui).clicked() {
                            commands.push(UiCommand::schedule(session, ScheduleEdit::SetBarAgent { bar: bar.id, agent: None }));
                            ui.close();
                        }
                        // Only machines that can do this work: a dig bar goes to
                        // loaders, a blast bar to dozers, drills and MPUs.
                        for agent in plan
                            .agents()
                            .iter()
                            .filter(|agent| plan.agent_kind(agent.id).is_some_and(|kind| crate::model::schedule::work_fits(&bar.work, kind)))
                        {
                            if ContextMenuAction::new(agent.name.clone()).checked(bar.agent == Some(agent.id)).show(ui).clicked() {
                                commands.push(UiCommand::schedule(
                                    session,
                                    ScheduleEdit::SetBarAgent {
                                        bar: bar.id,
                                        agent: Some(agent.id),
                                    },
                                ));
                                ui.close();
                            }
                        }
                    });
                    // At the pointer: a bar can span the whole week, and its
                    // left edge says nothing about the hour being asked about.
                    response.on_hover_ui_at_pointer(|ui| {
                        ui.set_min_width(260.0);
                        ui.label(bar_tooltip(bar, report, window, schedule, plan));
                        // A delay list or roster under the pointer: the bar
                        // covers most of the row, so this is where it is read.
                        if let Some(agent) = bar.agent
                            && let Some(pos) = ui.ctx().pointer_hover_pos()
                            && let Some((heading, span)) = calendar_delay_tooltip(plan, agent, view.seconds_at(pos.x, body.left(), body.width()) / GanttView::HOUR)
                        {
                            ui.separator();
                            ui.label(bold(&heading));
                            ui.label(span);
                        }
                    });
                }
            }
        }
    }

    // One drag is one edit, committed here rather than every frame, so a bar
    // dragged across a week is one Ctrl-Z and not several hundred. A drag that
    // never moved is a selection and commits nothing.
    if let Some(finished) = released
        && finished.moved
        && ((finished.preview_start_seconds, finished.preview_end_seconds) != (finished.from_start_seconds, finished.from_end_seconds)
            || (finished.preview_agent, finished.preview_priority) != (finished.from_agent, finished.from_priority)
            || finished.preview_insert)
    {
        commands.push(UiCommand::schedule(
            session,
            ScheduleEdit::SetBarPlacement {
                bar: finished.bar,
                agent: finished.preview_agent,
                priority: finished.preview_priority,
                window: finished.preview_window(),
                insert_lane: finished.preview_insert,
            },
        ));
    }
    editor.gantt_drag = drag;
    editor.schedule_selected_bar = selected.filter(|id| plan.bar(*id).is_some());
    if let Some(dialog) = open_dialog {
        editor.bar_name_dialog = Some(dialog);
    }
    if let Some(dialog) = open_window {
        editor.bar_window_dialog = Some(dialog);
    }
    if let Some(draft) = open_editor {
        editor.sequence_editor = Some(draft);
    }
    if let Some(dialog) = open_reclaim {
        editor.reclaim_bar_dialog = Some(dialog);
    }
    if let Some(dialog) = open_blast {
        editor.blast_bar_dialog = Some(dialog);
    }
}

/// A blast bar's calculated machine work, coloured by activity. Firing
/// events belong to the dedicated blasting row.
fn draw_blast_band(ui: &mut egui::Ui, body: egui::Rect, band: egui::Rect, view: GanttView, bar: &ScheduleBar, schedule: &CalculatedSchedule) {
    let Some(result) = schedule.drill_blast.as_ref() else { return };
    let blasts = bar_blasts(bar, schedule);
    let clip = band.intersect(body);
    let rows: Vec<&crate::model::schedule::result::PublishedBlastWork> = result
        .work
        .iter()
        .filter(|row| Some(row.agent) == bar.agent && blasts.contains(&row.blast))
        .filter(|row| row.start_h >= bar.window.start_h - 1e-9 && bar.window.end_h.is_none_or(|end| row.start_h < end))
        .collect();
    for row in &rows {
        let span = span_rect(view, body, row.start_h, row.end_h, band.top(), WORK_BAND);
        if span.intersects(clip) {
            ui.painter_at(clip).rect_filled(span, 0.0, blast_activity_color(row.activity));
        }
    }
    let target = band.expand2(egui::vec2(0.0, 3.0)).intersect(body);
    if !target.is_positive() {
        return;
    }
    let hover = ui.interact(target, ui.id().with(("gantt_blast_work", bar.id)), egui::Sense::hover());
    let Some(pos) = hover.hover_pos() else { return };
    let at_h = view.seconds_at(pos.x, body.left(), body.width()) / GanttView::HOUR;
    let working: Vec<_> = rows.iter().filter(|row| row.start_h <= at_h && at_h < row.end_h).collect();
    if working.is_empty() {
        return;
    }
    hover.on_hover_ui_at_pointer(|ui| {
        ui.set_min_width(220.0);
        for row in working {
            let blast = &result.blasts[row.blast];
            let step = row.activity as usize;
            let done = result.done_share(row.blast, row.activity, at_h) * blast.quantity[step];
            ui.label(bold(&tr!("blast-work-heading", activity = row.activity.label(), blast = blast_title(blast))));
            ui.label(tr!(
                "blast-work-progress",
                done = format!("{:.0}", done.max(0.0)),
                total = format!("{:.0}", blast.quantity[step]),
                unit = row.activity.unit()
            ));
        }
    });
}

/// A published blast as hovers name it: its bench and its name.
pub(super) fn blast_title(blast: &crate::model::schedule::result::PublishedBlast) -> String {
    tr!("blast-label", bench = format!("{:.0}", blast.bench_top), name = blast.name.clone())
}

/// The placement a drag is currently previewing, as one value.
fn previewed(drag: GanttDrag) -> Placement {
    Placement {
        agent: drag.preview_agent,
        priority: drag.preview_priority,
        insert: drag.preview_insert,
    }
}

/// The empty space of each row: right-click it to add a bar to that machine,
/// in that lane.
///
/// Sensed after the bars, so a right-click that lands on a bar opens the bar's
/// menu rather than this one.
fn draw_row_menus(ui: &mut egui::Ui, body: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, rows: &[Row], session: u32, commands: &mut Vec<UiCommand>) {
    if !body.is_positive() {
        return;
    }
    let scroll = editor.gantt.row_scroll;
    let view = editor.gantt;
    for (index, row) in rows.iter().enumerate().filter(|(_, row)| !row.blasting) {
        let top = body.top() + row.top - scroll;
        if top + row.height < body.top() || top > body.bottom() {
            continue;
        }
        for lane in 0..row.lanes.len() {
            let rect = row.lane_rect(lane, body.left(), body.right(), scroll, body.top()).intersect(body);
            if !rect.is_positive() {
                continue;
            }
            let id = ui.id().with(("gantt_lane", index, lane));
            let response = ui.interact(rect, id, egui::Sense::click());
            // A delay list or roster under the pointer, named. The bars take
            // the hover where they sit, so this answers only for open lane.
            if let Some(agent) = row.agent
                && let Some(pos) = response.hover_pos()
                && let Some((heading, span)) = calendar_delay_tooltip(plan, agent, view.seconds_at(pos.x, body.left(), body.width()) / GanttView::HOUR)
            {
                response.clone().on_hover_ui_at_pointer(|ui| {
                    ui.label(bold(&heading));
                    ui.label(span);
                    ui.label(egui::RichText::new(tr!("gantt-delay-calendar-note")).weak());
                });
            }
            // Where along the timeline the menu was opened, remembered as it
            // opens: the row inside it is clicked a frame or more later, by
            // which time the pointer is over the menu rather than the lane.
            let opened_at = id.with("menu_seconds");
            if response.secondary_clicked()
                && let Some(pos) = response.interact_pointer_pos()
            {
                let seconds = view.seconds_at(pos.x, body.left(), body.width()).max(0.0);
                ui.data_mut(|data| data.insert_temp(opened_at, seconds));
            }
            let title = row.title.clone();
            let priority = row.lanes[lane].priority;
            let drill_blast = row
                .agent
                .and_then(|agent| plan.agent_kind(agent))
                .is_some_and(crate::model::schedule::MachineKind::is_drill_blast);
            context_menu_popup(&response, title, |ui| {
                if drill_blast && ContextMenuAction::new(tr!("blast-add-bar")).show(ui).clicked() {
                    let start_h = (ui.data(|data| data.get_temp::<f64>(opened_at)).unwrap_or(0.0) / GanttView::HOUR).round();
                    editor.blast_bar_dialog = Some(BlastBarDialog {
                        session,
                        edition: crate::ui::state::SequenceDraft::next_edition(),
                        generation: None,
                        opened_from: Vec::new(),
                        view: crate::ui::state::SolidPreviewView::default(),
                        target: None,
                        members: Vec::new(),
                        agent: row.agent,
                        priority,
                        insert_lane: false,
                        start_h,
                        end_h: start_h + crate::model::schedule::SCHEDULE_PERIOD_H,
                        bench: None,
                    });
                    ui.close();
                }
                if !drill_blast && ContextMenuAction::new(tr!("reclaim-add-dig-bar")).show(ui).clicked() {
                    // The new bar lands in the row, the lane and at the instant
                    // it was asked for, rather than unassigned at hour zero
                    // somewhere off screen: right-clicking a machine's lane at
                    // a place on the timeline is how the user says all three.
                    // One period long to begin with, not open-ended: a new
                    // bar is something to resize, and a window with no end
                    // has no edge to take hold of.
                    let start_h = ui.data(|data| data.get_temp::<f64>(opened_at)).unwrap_or(0.0) / GanttView::HOUR;
                    commands.push(UiCommand::schedule(
                        session,
                        ScheduleEdit::AddBar {
                            name: String::new(),
                            agent: row.agent,
                            priority,
                            window: WorkWindow {
                                start_h,
                                end_h: Some(start_h + crate::model::schedule::SCHEDULE_PERIOD_H),
                            },
                            insert_lane: false,
                        },
                    ));
                    ui.close();
                }
                if ContextMenuAction::new(tr!("delay-add-bar")).show(ui).clicked() {
                    let start_h = (ui.data(|data| data.get_temp::<f64>(opened_at)).unwrap_or(0.0) / GanttView::HOUR).round();
                    let pos = ui.ctx().pointer_latest_pos().unwrap_or(rect.center());
                    editor.gantt_delay_drop = Some(DelayDrop {
                        session,
                        agent: row.agent,
                        priority,
                        insert: false,
                        start_h,
                        pos,
                    });
                    ui.close();
                }
                if !drill_blast && ContextMenuAction::new(tr!("reclaim-add-bar")).show(ui).clicked() {
                    let start_h = ui.data(|data| data.get_temp::<f64>(opened_at)).unwrap_or(0.0) / GanttView::HOUR;
                    editor.reclaim_bar_dialog = Some(ReclaimBarDialog {
                        target: None,
                        sources: Vec::new(),
                        agent: row.agent,
                        priority,
                        start: start_h.to_string(),
                        end: (start_h + crate::model::schedule::SCHEDULE_PERIOD_H).to_string(),
                        maximum: String::new(),
                    });
                    ui.close();
                }
            });
        }
    }
}

/// Vertical grid lines under the rows, on the same ticks the ruler labels.
pub(super) fn draw_grid(ui: &egui::Ui, rect: egui::Rect, view: GanttView, interval: f64) {
    if !rect.is_positive() {
        return;
    }
    let painter = ui.painter_at(rect);
    let stroke = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color.gamma_multiply(0.6));
    for tick in view.visible_ticks(interval) {
        let x = view.x_of(tick, rect.left(), rect.width());
        painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], stroke);
    }
}

/// A single line of guidance across the middle of the timeline.
pub(super) fn centred_note(ui: &egui::Ui, rect: egui::Rect, text: String) {
    ui.painter_at(rect).text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().weak_text_color(),
    );
}

/// How long a bar dropped from the palette is to begin with: a day of work, or
/// a shift's worth of delay. Something to resize, as a right-clicked bar is.
fn palette_length_h(item: GanttPaletteItem) -> f64 {
    match item {
        GanttPaletteItem::Dig | GanttPaletteItem::Reclaim | GanttPaletteItem::Blast => crate::model::schedule::SCHEDULE_PERIOD_H,
        GanttPaletteItem::Delay => 12.0,
    }
}

fn palette_label(item: GanttPaletteItem) -> String {
    match item {
        GanttPaletteItem::Dig => tr!("gantt-palette-dig"),
        GanttPaletteItem::Reclaim => tr!("gantt-palette-reclaim"),
        GanttPaletteItem::Delay => tr!("gantt-palette-delay"),
        GanttPaletteItem::Blast => tr!("gantt-palette-blast"),
    }
}

fn palette_color(item: GanttPaletteItem) -> egui::Color32 {
    match item {
        GanttPaletteItem::Dig => WORKING_COLOR,
        GanttPaletteItem::Reclaim => RECLAIM_COLOR,
        GanttPaletteItem::Delay => super::schedule_delays::UNTYPED_DELAY_COLOR,
        GanttPaletteItem::Blast => BLAST_COLOR,
    }
}

/// The chips in the corner above the machine names: drag one onto a row to
/// make that kind of bar there.
fn draw_palette(ui: &mut egui::Ui, corner: egui::Rect, editor: &mut EditorState) {
    let items = [GanttPaletteItem::Dig, GanttPaletteItem::Reclaim, GanttPaletteItem::Delay, GanttPaletteItem::Blast];
    let inner = corner.shrink2(egui::vec2(6.0, 3.0));
    if !inner.is_positive() {
        return;
    }
    let height = inner.height().min(18.0);
    let gap = 4.0;
    let width = ((inner.width() - gap * (items.len() - 1) as f32) / items.len() as f32).min(64.0);
    let font = egui::TextStyle::Small.resolve(ui.style());
    let visuals = ui.visuals().clone();
    for (index, item) in items.into_iter().enumerate() {
        let left = inner.left() + index as f32 * (width + gap);
        let chip = egui::Rect::from_min_size(egui::pos2(left, inner.center().y - height * 0.5), egui::vec2(width, height));
        let response = ui
            .interact(chip, ui.id().with(("gantt_palette", index)), egui::Sense::drag())
            .on_hover_text(tr!("gantt-palette-hint", item = palette_label(item)));
        let fill = if response.hovered() || editor.gantt_palette_drag == Some(item) {
            visuals.widgets.hovered.bg_fill
        } else {
            visuals.widgets.inactive.bg_fill
        };
        ui.painter().rect_filled(chip, GROUP_CORNER_RADIUS, fill);
        // A swatch of the colour the bar will be, beside its name.
        let swatch = egui::Rect::from_min_size(egui::pos2(chip.left() + 4.0, chip.center().y - 3.5), egui::vec2(7.0, 7.0));
        ui.painter().rect_filled(swatch, 1.0, palette_color(item));
        ui.painter_at(chip).text(
            egui::pos2(swatch.right() + 4.0, chip.center().y),
            egui::Align2::LEFT_CENTER,
            palette_label(item),
            font.clone(),
            visuals.text_color(),
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if response.drag_started() {
            editor.gantt_palette_drag = Some(item);
        }
    }
}

/// A palette chip being carried over the timeline: where it would land is
/// shown as the slot and the instant, and letting go there makes the bar.
#[allow(clippy::too_many_arguments, reason = "the drop needs the frame's layout, the plan and somewhere to put the edit")]
fn draw_palette_drop(ui: &mut egui::Ui, body: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, rows: &[Row], session: u32, commands: &mut Vec<UiCommand>) {
    let Some(item) = editor.gantt_palette_drag else { return };
    let (pointer, released, cancelled) = ui.input_mut(|input| {
        (
            input.pointer.interact_pos(),
            !input.pointer.button_down(egui::PointerButton::Primary),
            input.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if cancelled {
        editor.gantt_palette_drag = None;
        return;
    }
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    let scroll = editor.gantt.row_scroll;
    let view = editor.gantt;
    let target = pointer.filter(|pos| body.contains(*pos)).and_then(|pos| {
        let placement = placement_at(pos.y, body, scroll, rows)?;
        // On the hour: a bar dropped by hand is never meant to start at 06:47.
        let start_h = (view.seconds_at(pos.x, body.left(), body.width()) / GanttView::HOUR).round().max(0.0);
        Some((pos, placement, start_h))
    });
    let painter = ui.painter_at(body);
    let visuals = ui.visuals().clone();
    if let Some((_, placement, start_h)) = target
        && let Some(top) = placement_top(placement, body, scroll, rows)
    {
        let slot = if placement.insert {
            egui::Rect::from_min_size(egui::pos2(body.left(), top - 1.5), egui::vec2(body.width(), 3.0))
        } else {
            egui::Rect::from_min_size(egui::pos2(body.left(), top), egui::vec2(body.width(), BAR_TOP + plan.bar_height() + BAR_BOTTOM))
        };
        let fill = if placement.insert {
            visuals.selection.stroke.color
        } else {
            visuals.selection.bg_fill.gamma_multiply(0.18)
        };
        painter.rect_filled(slot.intersect(body), GROUP_CORNER_RADIUS, fill);
        let ghost = span_rect(view, body, start_h, start_h + palette_length_h(item), top + BAR_TOP, plan.bar_height());
        painter.rect_filled(ghost, GROUP_CORNER_RADIUS, palette_color(item).gamma_multiply(0.55));
        painter.rect_stroke(ghost, GROUP_CORNER_RADIUS, egui::Stroke::new(1.0, palette_color(item)), egui::StrokeKind::Inside);
    }
    if let Some(pos) = pointer {
        // The chip under the pointer, over everything, so it is plain what is
        // being carried even outside the timeline.
        let layer = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, ui.id().with("gantt_palette_ghost")));
        let text = match target {
            Some((_, _, start_h)) => format!("{} · {}", palette_label(item), instant_label(start_h * GanttView::HOUR)),
            None => palette_label(item),
        };
        let galley = layer.layout_no_wrap(text, egui::TextStyle::Small.resolve(ui.style()), visuals.strong_text_color());
        let rect = egui::Rect::from_min_size(pos + egui::vec2(12.0, 10.0), galley.size() + egui::vec2(12.0, 6.0));
        layer.rect_filled(rect, GROUP_CORNER_RADIUS, visuals.window_fill);
        layer.rect_stroke(rect, GROUP_CORNER_RADIUS, egui::Stroke::new(1.0, palette_color(item)), egui::StrokeKind::Inside);
        layer.galley(rect.min + egui::vec2(6.0, 3.0), galley, visuals.strong_text_color());
    }
    ui.ctx().request_repaint();
    if !released {
        return;
    }
    editor.gantt_palette_drag = None;
    let Some((pos, placement, start_h)) = target else { return };
    let window = WorkWindow {
        start_h,
        end_h: Some(start_h + palette_length_h(item)),
    };
    match item {
        GanttPaletteItem::Dig => commands.push(UiCommand::schedule(
            session,
            ScheduleEdit::AddBar {
                name: String::new(),
                agent: placement.agent,
                priority: placement.priority,
                window,
                insert_lane: placement.insert,
            },
        )),
        GanttPaletteItem::Reclaim => {
            editor.reclaim_bar_dialog = Some(ReclaimBarDialog {
                target: None,
                sources: Vec::new(),
                agent: placement.agent,
                priority: placement.priority,
                start: start_h.to_string(),
                end: (start_h + palette_length_h(item)).to_string(),
                maximum: String::new(),
            });
        }
        GanttPaletteItem::Delay => {
            editor.gantt_delay_drop = Some(DelayDrop {
                session,
                agent: placement.agent,
                priority: placement.priority,
                insert: placement.insert,
                start_h,
                pos,
            });
        }
        GanttPaletteItem::Blast => {
            editor.blast_bar_dialog = Some(BlastBarDialog {
                session,
                edition: crate::ui::state::SequenceDraft::next_edition(),
                generation: None,
                opened_from: Vec::new(),
                view: crate::ui::state::SolidPreviewView::default(),
                target: None,
                members: Vec::new(),
                agent: placement.agent,
                priority: placement.priority,
                insert_lane: placement.insert,
                start_h,
                end_h: start_h + palette_length_h(item),
                bench: None,
            });
        }
    }
}

/// The menu a dropped delay bar opens: which kind of delay it is.
fn draw_delay_drop_menu(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let Some(drop) = editor.gantt_delay_drop.filter(|drop| drop.session == session) else {
        editor.gantt_delay_drop = None;
        return;
    };
    let mut chosen: Option<Option<crate::model::schedule::DelayTypeId>> = None;
    let mut close = false;
    let mut open_setup = false;
    let area = egui::Area::new(ui.id().with("gantt_delay_drop"))
        .order(egui::Order::Foreground)
        .fixed_pos(drop.pos)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(180.0);
                ui.set_max_width(220.0);
                ui.label(bold(&tr!("gantt-delay-type-title")));
                ui.add_space(2.0);
                let swatch_row = |ui: &mut egui::Ui, color: egui::Color32, name: String| {
                    let response = ui.add(egui::Button::new(format!("     {name}")).frame(false).min_size(egui::vec2(ui.available_width(), 0.0)));
                    let swatch = egui::Rect::from_min_size(egui::pos2(response.rect.left() + 4.0, response.rect.center().y - 5.0), egui::vec2(10.0, 10.0));
                    ui.painter().rect_filled(swatch, 2.0, color);
                    response.clicked()
                };
                for entry in &plan.delays().types {
                    if swatch_row(ui, super::schedule_delays::delay_color(entry.color), entry.name.clone()) {
                        chosen = Some(Some(entry.id));
                    }
                }
                if swatch_row(ui, super::schedule_delays::UNTYPED_DELAY_COLOR, tr!("delay-untyped")) {
                    chosen = Some(None);
                }
                ui.separator();
                if ui.add(egui::Button::new(tr!("gantt-delay-types-setup")).frame(false)).clicked() {
                    open_setup = true;
                }
            });
        });
    if area.response.clicked_elsewhere() || ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if let Some(kind) = chosen {
        commands.push(UiCommand::schedule(
            session,
            ScheduleEdit::AddDelayBar {
                agent: drop.agent,
                priority: drop.priority,
                window: WorkWindow {
                    start_h: drop.start_h,
                    end_h: Some(drop.start_h + palette_length_h(GanttPaletteItem::Delay)),
                },
                kind,
                insert_lane: drop.insert,
            },
        ));
        close = true;
    }
    if open_setup {
        editor.planning_page = PlanningPage::Schedule;
        editor.schedule_subpage = PlanningSubpage::Setup;
        editor.schedule_setup_step = ScheduleStep::Delays;
        editor.schedule_selected_delay = Some(crate::ui::state::DelaySelection::Types);
        close = true;
    }
    if close {
        editor.gantt_delay_drop = None;
    }
}

/// Delay lists and rosters on each machine's row: a tint of the delay's
/// colour across the row, with a solid edge along its top. Behind the bars,
/// because a delay is the machine's calendar rather than a bar of its own.
fn draw_calendar_delays(ui: &egui::Ui, body: egui::Rect, view: GanttView, scroll: f32, plan: &SchedulePlan, rows: &[Row]) {
    if plan.delays().lists.is_empty() && plan.delays().rosters.is_empty() {
        return;
    }
    let painter = ui.painter_at(body);
    let end_h = view.end_seconds() / GanttView::HOUR;
    let start_h = view.start_seconds / GanttView::HOUR;
    for row in rows {
        let Some(agent) = row.agent else { continue };
        let top = body.top() + row.top - scroll;
        if top + row.height < body.top() || top > body.bottom() {
            continue;
        }
        for span in plan.delays().spans_for(agent, end_h) {
            if span.end_h <= start_h {
                continue;
            }
            let (color, _) = super::schedule_delays::delay_look(plan, span.kind);
            let rect = span_rect(view, body, span.start_h, span.end_h, top, row.height);
            painter.rect_filled(rect, 0.0, color.gamma_multiply(0.22));
            painter.rect_filled(egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + 2.0)), 0.0, color);
        }
    }
}

/// What a calendar delay at `at_h` on `agent` is, for a hover.
fn calendar_delay_tooltip(plan: &SchedulePlan, agent: LoaderAgentId, at_h: f64) -> Option<(String, String)> {
    let span = plan
        .delays()
        .spans_for(agent, at_h + 1.0)
        .into_iter()
        .find(|span| span.start_h <= at_h && at_h < span.end_h)?;
    let (_, kind) = super::schedule_delays::delay_look(plan, span.kind);
    let source = match span.source {
        crate::model::schedule::delays::DelaySource::List(id) => plan.delays().list(id).map(|list| list.title.clone()).unwrap_or_default(),
        crate::model::schedule::delays::DelaySource::Roster(id) => plan.delays().roster(id).map(|roster| roster.name.clone()).unwrap_or_default(),
    };
    Some((
        tr!("gantt-delay-heading", kind = kind, source = source),
        tr!(
            "schedule-span-hours",
            from = instant_label(span.start_h * GanttView::HOUR),
            to = instant_label(span.end_h * GanttView::HOUR),
            hours = format!("{:.1}", span.end_h - span.start_h)
        ),
    ))
}

fn blast_bar_completion_h(plan: &SchedulePlan, bar: &ScheduleBar, schedule: &CalculatedSchedule) -> Option<f64> {
    let activity = plan.agent_kind(bar.agent?)?.activity()?;
    let blasts = bar_blasts(bar, schedule);
    if blasts.is_empty() || blasts.len() != bar.blast_order()?.members.len() {
        return None;
    }
    let result = schedule.drill_blast.as_ref()?;
    let mut completed = bar.window.start_h;
    for blast in blasts {
        completed = completed.max(result.blasts[blast].done_h[activity as usize]?);
    }
    Some(completed)
}

struct FiringMarker {
    blast: usize,
    x: f32,
    lane: usize,
    label: std::sync::Arc<egui::Galley>,
}

fn firing_markers(ui: &egui::Ui, schedule: Option<&CalculatedSchedule>, view: GanttView, body: egui::Rect) -> Vec<FiringMarker> {
    let Some(result) = schedule.and_then(|schedule| schedule.drill_blast.as_ref()) else {
        return Vec::new();
    };
    let mut events: Vec<_> = result
        .blasts
        .iter()
        .enumerate()
        .filter_map(|(index, blast)| blast.fired_h.filter(|at| at.is_finite() && *at > 0.0 && *at < f64::MAX).map(|at| (index, at)))
        .collect();
    events.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let mut ends: Vec<f32> = Vec::new();
    let mut markers = Vec::new();
    for (blast, at) in events {
        let x = view.x_of(at * GanttView::HOUR, body.left(), body.width());
        if x < body.left() - 250.0 || x > body.right() {
            continue;
        }
        let label = ui
            .painter()
            .layout_no_wrap(blast_title(&result.blasts[blast]), egui::TextStyle::Body.resolve(ui.style()), ui.visuals().text_color());
        let lane = ends.iter().position(|end| *end < x - 8.0).unwrap_or(ends.len());
        if lane == ends.len() {
            ends.push(0.0);
        }
        ends[lane] = x + 10.0 + label.size().x;
        markers.push(FiringMarker { blast, x, lane, label });
    }
    markers
}

fn open_blast_window(editor: &mut EditorState, plan: &SchedulePlan, session: u32, window: Option<crate::model::schedule::drill_blast::BlastWindow>, at_h: f64) {
    editor.blast_window_dialog = Some(crate::ui::state::BlastWindowDialog {
        session,
        opened: plan.drill_blast().effective_windows(),
        id: window.map(|window| window.id),
        start: format!("{}", window.map_or(at_h.max(0.0).floor(), |window| window.start_h)),
        end: format!("{}", window.map_or(at_h.max(0.0).floor() + 3.0, |window| window.end_h)),
        daily: window.is_some_and(|window| window.daily),
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_blasting_row(
    ui: &mut egui::Ui,
    body: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    rows: &[Row],
    session: u32,
    commands: &mut Vec<UiCommand>,
    schedule: Option<&CalculatedSchedule>,
) {
    let Some(row) = rows.iter().find(|row| row.blasting) else { return };
    let top = body.top() + row.top - editor.gantt.row_scroll;
    let rect = egui::Rect::from_min_max(egui::pos2(body.left(), top), egui::pos2(body.right(), top + row.height)).intersect(body);
    if !rect.is_positive() {
        return;
    }
    let view = editor.gantt;
    let response = ui.interact(rect, ui.id().with("blasting_row"), egui::Sense::click());
    let at = response
        .interact_pointer_pos()
        .or_else(|| response.hover_pos())
        .map_or(0.0, |pos| view.seconds_at(pos.x, body.left(), body.width()) / GanttView::HOUR);
    response.context_menu(|ui| {
        if ContextMenuAction::new(tr!("blast-window-add")).show(ui).clicked() {
            open_blast_window(editor, plan, session, None, at);
            ui.close();
        }
    });
    let windows = plan.drill_blast().window_spans(view.start_seconds / GanttView::HOUR, view.end_seconds() / GanttView::HOUR);
    let mut on_window = false;
    for (window, start, end) in windows {
        let span = span_rect(view, body, start, end, top + 4.0, 20.0).intersect(rect);
        if !span.is_positive() {
            continue;
        }
        ui.painter_at(rect).rect_filled(span, GROUP_CORNER_RADIUS, BLAST_COLOR.gamma_multiply(0.25));
        ui.painter_at(rect)
            .rect_stroke(span, GROUP_CORNER_RADIUS, egui::Stroke::new(1.0, BLAST_COLOR), egui::StrokeKind::Inside);
        let hit = ui.interact(span, ui.id().with(("blast_window", window.id, start.to_bits())), egui::Sense::click());
        on_window |= hit.hovered();
        hit.clone().on_hover_text(format!(
            "{} — {}\n{}",
            instant_label(start * GanttView::HOUR),
            instant_label(end * GanttView::HOUR),
            if window.daily { tr!("blast-window-daily") } else { tr!("blast-window-once") }
        ));
        if hit.double_clicked() {
            open_blast_window(editor, plan, session, Some(window), start);
        }
        hit.context_menu(|ui| {
            if ContextMenuAction::new(tr!("blast-window-edit")).show(ui).clicked() {
                open_blast_window(editor, plan, session, Some(window), start);
                ui.close();
            }
            if ContextMenuAction::new(tr!(literal = "Delete")).show(ui).clicked() {
                let mut windows = plan.drill_blast().effective_windows();
                windows.retain(|entry| entry.id != window.id);
                commands.push(UiCommand::schedule(session, ScheduleEdit::SetBlastWindows(windows)));
                ui.close();
            }
        });
    }
    if response.double_clicked() && !on_window {
        open_blast_window(editor, plan, session, None, at);
    }
    for marker in firing_markers(ui, schedule, view, body) {
        let centre = egui::pos2(marker.x, top + 36.0 + marker.lane as f32 * 20.0);
        let target = egui::Rect::from_min_size(centre - egui::vec2(6.0, 9.0), egui::vec2(marker.label.size().x + 18.0, 18.0)).intersect(rect);
        if !target.is_positive() {
            continue;
        }
        let points = vec![
            centre + egui::vec2(0.0, -5.0),
            centre + egui::vec2(5.0, 0.0),
            centre + egui::vec2(0.0, 5.0),
            centre + egui::vec2(-5.0, 0.0),
        ];
        let painter = ui.painter_at(rect);
        painter.add(egui::Shape::convex_polygon(points, BLAST_COLOR, egui::Stroke::new(1.0, ui.visuals().strong_text_color())));
        painter.galley(centre + egui::vec2(10.0, -marker.label.size().y * 0.5), marker.label, ui.visuals().text_color());
        if let Some(blast) = schedule
            .and_then(|schedule| schedule.drill_blast.as_ref())
            .and_then(|result| result.blasts.get(marker.blast))
        {
            ui.interact(target, ui.id().with(("blast_firing", marker.blast)), egui::Sense::hover())
                .on_hover_text(tr!("blast-fired-at", at = instant_label(blast.fired_h.unwrap_or(0.0) * GanttView::HOUR)));
        }
    }
}
