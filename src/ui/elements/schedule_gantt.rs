//! The Schedule workspace's Gantt: one timeline row per loader agent, along
//! elapsed project time.
//!
//! Rows, the ruler, the navigation - and the authored bars, which is where a
//! bar is created, named, copied, assigned, laned and positioned.
//!
//! A bar is drawn as a marker at its earliest start, not as a length of time.
//! Its length is the execution the dispatch stage calculates from its tonnes
//! and its loader's rate, and that stage does not exist yet; a rectangle
//! stretched from here to some guess would look exactly like one that had been
//! calculated. So the marker is sized to its own label, says the earliest
//! start it stands at, and says that the length is calculated elsewhere.
//!
//! Dragging a bar sets its earliest start and nothing else. The drag previews
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
    model::schedule::{LoaderAgentId, ScheduleBar, SchedulePlan},
    ui::{
        EditorState, UiProjectView, chrome,
        fonts::bold,
        state::{BarNameDialog, GanttDrag, GanttView, ScheduleBarView, ScheduleEdit, UiCommand},
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
/// Height of one priority lane inside a row. A row is as tall as its lanes
/// need, and never shorter than [`ROW_HEIGHT`].
const LANE_HEIGHT: f32 = 26.0;
/// Least width of a bar marker, before its label widens it.
const MIN_BAR_WIDTH: f32 = 40.0;
/// Greatest width a bar marker's label is allowed to claim.
const MAX_BAR_WIDTH: f32 = 260.0;
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
pub(crate) fn draw_details(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) -> egui::Rect {
    // Cloned rather than borrowed: the canvas takes `editor` mutably to hold
    // its drag and its selection. Every edit below is addressed to the project
    // this plan was read from, so one queued against a project that is closed
    // before the frame's commands are handled is refused rather than applied
    // to its successor.
    let plan = project.schedule.clone();
    let session = project.active_session;
    let rect = egui::CentralPanel::default()
        .frame(chrome::region_frame(ui))
        .show(ui, |ui| {
            let available = ui.available_rect_before_wrap();
            let toolbar_height = ui.spacing().interact_size.y + 8.0;
            let toolbar = egui::Rect::from_min_size(available.min, egui::vec2(available.width(), toolbar_height.min(available.height())));
            let canvas = egui::Rect::from_min_max(egui::pos2(available.left(), toolbar.bottom()), available.max);
            draw_toolbar(ui, toolbar, editor);
            if canvas.is_positive() {
                draw_canvas(ui, canvas, editor, &plan, session, commands);
            }
            ui.allocate_rect(available, egui::Sense::hover());
        })
        .response
        .rect;
    crate::ui::dialogs::schedule::draw_bar_name_dialog(ui, editor, &plan, session, commands);
    crate::ui::dialogs::sequence_editor::draw_sequence_editor(ui, editor, &plan, session, commands);
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

/// Where one bar's marker sits along the timeline, and how wide it is.
///
/// The left edge comes from the bar's *authored* earliest start, never from a
/// drag preview: a drag draws somewhere else, but the rows it is dragged over
/// stay still rather than resizing under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
struct BarExtent {
    left: f32,
    width: f32,
}

impl BarExtent {
    fn right(self) -> f32 {
        self.left + self.width
    }
}

/// One bar's marker: the label it carries and the space that label claims.
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
        let rows = layout_rows(plan, &extents);
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
    let font = egui::TextStyle::Small.resolve(ui.style());
    let color = ui.visuals().strong_text_color();
    let view = editor.gantt;
    plan.bars()
        .iter()
        .map(|bar| {
            let report = editor.schedule_bar_reports.iter().find(|report| report.bar == bar.id);
            let galley = ui.painter().layout_no_wrap(bar_label(bar, report), font.clone(), color);
            Marker {
                extent: BarExtent {
                    left: view.x_of(bar.earliest_start_h * GanttView::HOUR, body.left(), body.width()),
                    width: (galley.size().x + 18.0).clamp(MIN_BAR_WIDTH, MAX_BAR_WIDTH),
                },
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
                ends[stack] = extent.right() + BAR_GAP;
                stacks[stack].push(index);
            }
            None => {
                ends.push(extent.right() + BAR_GAP);
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
                unit = tr!("schedule-tph")
            ),
            None => tr!("schedule-error-unknown-class"),
        };
        rows.push(Row {
            agent: Some(agent.id),
            title: agent.name.clone(),
            subtitle,
            lanes: Vec::new(),
            top: 0.0,
            height: 0.0,
        });
    }
    // Filled with one sub-row holding everything the lane has; packed into
    // non-overlapping sub-rows once the lane is complete.
    for (index, bar) in plan.bars().iter().enumerate() {
        let agent = placed(bar);
        let row = rows.iter_mut().find(|row| row.agent == agent).expect("every bar's row was made above");
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
            lane.height = LANE_HEIGHT * lane.stacks.len() as f32;
            lanes_height += lane.height;
        }
        // A row is never shorter than a machine's name and its class need.
        // Where the lanes alone do not fill that, they share the rest out
        // between them rather than leaving a dead strip under the last one.
        let scale = if lanes_height > 0.0 { (ROW_HEIGHT / lanes_height).max(1.0) } else { 1.0 };
        let mut offset = 0.0;
        for lane in &mut row.lanes {
            lane.height *= scale;
            lane.offset = offset;
            offset += lane.height;
        }
        row.height = offset.max(ROW_HEIGHT);
        row.top = top;
        top += row.height;
    }
    rows
}

/// Zoom controls, Reset View, and what window of project time is on screen.
fn draw_toolbar(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState) {
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("gantt_toolbar").max_rect(rect));
    child.set_clip_rect(child.clip_rect().intersect(rect));
    child.horizontal_centered(|ui| {
        ui.add_space(4.0);
        let button = |ui: &mut egui::Ui, label: &str, tooltip: String| ui.add(egui::Button::new(label).corner_radius(GROUP_CORNER_RADIUS)).on_hover_text(tooltip);
        if button(ui, "−", tr!("gantt-zoom-out")).clicked() {
            editor.gantt.zoom_at(1.0 / ZOOM_STEP, 0.5);
        }
        if button(ui, "+", tr!("gantt-zoom-in")).clicked() {
            editor.gantt.zoom_at(ZOOM_STEP, 0.5);
        }
        if ui.add(egui::Button::new(tr!("gantt-reset-view")).corner_radius(GROUP_CORNER_RADIUS)).clicked() {
            editor.gantt.reset();
        }
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(tr!(
                "gantt-range",
                from = instant_label(editor.gantt.start_seconds),
                to = instant_label(editor.gantt.end_seconds())
            ))
            .weak(),
        );
    });
}

/// The header column, the ruler, the rows and the bars, plus the navigation
/// over them.
fn draw_canvas(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let header_width = HEADER_WIDTH.min(rect.width() * 0.5);
    let bands = if editor.gantt.minor_interval(rect.width() - header_width, MIN_TICK_SPACING) < GanttView::DAY {
        2.0
    } else {
        1.0
    };
    let ruler_height = (RULER_BAND * bands).min(rect.height());
    let header = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + ruler_height), egui::pos2(rect.left() + header_width, rect.bottom()));
    let ruler = egui::Rect::from_min_max(egui::pos2(rect.left() + header_width, rect.top()), egui::pos2(rect.right(), rect.top() + ruler_height));
    let body = egui::Rect::from_min_max(ruler.left_bottom(), rect.max);
    let corner = egui::Rect::from_min_max(rect.min, header.right_top());

    // Input is read only while the pointer is over this canvas, so the wheel
    // still scrolls whatever else is on screen and the keyboard is untouched.
    let response = ui.interact(rect, ui.id().with("gantt_canvas"), egui::Sense::click_and_drag());
    // Read from the pointer rather than the canvas response, because the bars
    // drawn on top of it take the hover for themselves: the wheel has to keep
    // zooming and panning while the pointer is over one.
    let pointer = ui.input(|input| input.pointer.hover_pos());
    let over_canvas = pointer.is_some_and(|pos| rect.contains(pos));
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
        editor.gantt.row_scroll -= scroll.y;
    }
    if response.dragged_by(egui::PointerButton::Middle) && body.width() > 0.0 {
        editor.gantt.pan(-f64::from(response.drag_delta().x) / f64::from(body.width()) * editor.gantt.span_seconds);
    }
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
    draw_ruler(ui, ruler, editor.gantt, interval, bands > 1.0);
    draw_rows(ui, header, body, stripe, editor, &layout.rows);
    draw_grid(ui, body, editor.gantt, interval);

    // Rules last, so no row fill or grid line sits on top of them.
    let painter = ui.painter();
    painter.line_segment([ruler.left_bottom(), ruler.right_bottom()], rule);
    painter.line_segment([corner.right_top(), header.right_bottom()], rule);
    painter.rect_stroke(rect, 0.0, rule, egui::StrokeKind::Inside);

    // The lanes first and the bars over them, both after the canvas: a click
    // on a bar is a click on the bar rather than a pan of the timeline, and a
    // right-click on empty lane space is that lane's own menu.
    draw_row_menus(ui, body, editor, plan, &layout.rows);
    draw_bars(ui, body, editor, plan, &layout, session, commands);

    if plan.agents().is_empty() && layout.rows.is_empty() {
        centred_note(ui, body, tr!("gantt-empty-fleet"));
    } else if plan.bars().is_empty() {
        // Below the rows rather than across them: said plainly, so lanes with
        // nothing in them are not mistaken for a schedule that came back
        // empty, but never painted over a row.
        let used = (rows_height - editor.gantt.row_scroll).max(0.0);
        let free = egui::Rect::from_min_max(egui::pos2(body.left(), body.top() + used), body.max);
        if free.height() > 32.0 {
            centred_note(ui, free, tr!("gantt-no-bars"));
        }
    }
}

/// The time ruler: minor ticks with their labels, and - while the minor ticks
/// are finer than a day - a band of days above grouping them.
fn draw_ruler(ui: &egui::Ui, rect: egui::Rect, view: GanttView, interval: f64, day_band: bool) {
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
                    egui::TextStyle::Small.resolve(ui.style()),
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
            egui::TextStyle::Small.resolve(ui.style()),
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
        let detail =
            egui::WidgetText::from(egui::RichText::new(&row.subtitle).small().color(weak)).into_galley(ui, Some(egui::TextWrapMode::Truncate), width, egui::TextStyle::Small);
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
                    egui::TextStyle::Small.resolve(ui.style()),
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

/// What one bar's marker says: its name, its tonnes once they can be stated,
/// and how many dig blocks it holds.
///
/// Tonnes appear only when the readiness report produced a complete figure.
/// A bar whose tonnage cannot be stated says how many blocks it holds instead
/// - never a zero, and never a number carried over from an older run.
fn bar_label(bar: &ScheduleBar, report: Option<&ScheduleBarView>) -> String {
    let detail = match report.and_then(|report| report.tonnes) {
        Some(tonnes) => tr_format!(literal = "%tonnes% %unit%", tonnes = tonnes.to_string(), unit = tr!(literal = "t")),
        None if bar.members().is_empty() => tr!("schedule-bar-empty"),
        None => tr!("schedule-bar-blocks", count = bar.members().len().to_string()),
    };
    tr_format!(literal = "%name% · %detail%", name = bar.name().to_owned(), detail = detail)
}

/// Everything the bar's hover says: what it is, where it starts, why it is not
/// ready, and that its length is not this marker's width.
fn bar_tooltip(bar: &ScheduleBar, report: Option<&ScheduleBarView>, start_seconds: f64) -> String {
    let mut lines = vec![bar_label(bar, report), tr!("schedule-bar-earliest-start", instant = instant_label(start_seconds))];
    // Every stated problem, in full. These are the reasons a schedule cannot
    // be calculated from this bar, and a truncated badge cannot carry them.
    if let Some(report) = report {
        lines.extend(report.problems.iter().cloned());
    }
    lines.push(tr!("schedule-bar-length-note"));
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
/// Deliberately separate from the marker that started it. A marker is culled
/// when it leaves the body - dragged past an edge, or scrolled out of the
/// rows - and a drag that could only be finished by its own culled widget
/// would never finish: the pointer would come up with the edit uncommitted and
/// the drag still live, and the next click would move a bar the user was no
/// longer dragging.
fn advance_drag(drag: GanttDrag, pointer_seconds: Option<f64>, released: bool, down: bool, cancelled: bool) -> DragStep {
    if cancelled {
        return DragStep::Abandon;
    }
    let mut drag = drag;
    if let Some(seconds) = pointer_seconds {
        // Never before the schedule origin: time does not run backwards, and a
        // bar clamped at zero is what the domain would accept anyway.
        let preview = (seconds - drag.grab_offset_seconds).max(0.0);
        if preview != drag.preview_seconds {
            drag.preview_seconds = preview;
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

/// The bars themselves: drawn, hit-tested, dragged and right-clicked.
fn draw_bars(ui: &mut egui::Ui, body: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, layout: &Layout, session: u32, commands: &mut Vec<UiCommand>) {
    if !body.is_positive() {
        return;
    }
    let scroll = editor.gantt.row_scroll;
    let view = editor.gantt;
    let painter = ui.painter_at(body);
    let visuals = ui.visuals().clone();
    let mut selected = editor.schedule_selected_bar;
    let mut open_dialog: Option<BarNameDialog> = None;
    let mut open_editor: Option<crate::ui::state::SequenceDraft> = None;

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
            DragStep::Continue(moved) => {
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
            DragStep::Commit(finished) => {
                drag = None;
                released = Some(finished);
            }
            DragStep::Abandon => drag = None,
        }
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
                    let (start_seconds, x) = match drag {
                        Some(drag) if drag.bar == bar.id => (drag.preview_seconds, view.x_of(drag.preview_seconds, body.left(), body.width())),
                        _ => (bar.earliest_start_h * GanttView::HOUR, marker.extent.left),
                    };
                    let lane_top = top + lane.offset + stack_height * stack as f32;
                    let rect = egui::Rect::from_min_size(egui::pos2(x, lane_top + 3.0), egui::vec2(marker.extent.width, (stack_height - 6.0).max(8.0)));
                    // Wholly off either side: nothing to draw and nothing to
                    // hit. An active drag is resolved above, before this, so
                    // culling a marker can no longer strand one.
                    if rect.right() < body.left() || rect.left() > body.right() {
                        continue;
                    }
                    let response = ui.interact(rect.intersect(body), ui.id().with(("gantt_bar", bar.id)), egui::Sense::click_and_drag());
                    let is_selected = selected == Some(bar.id);
                    let ready = report.is_some_and(|report| report.ready);
                    let fill = if ready { visuals.selection.bg_fill } else { visuals.widgets.inactive.bg_fill };
                    let stroke = if is_selected {
                        egui::Stroke::new(2.0, visuals.selection.stroke.color)
                    } else if report.is_some_and(|report| !report.problems.is_empty()) {
                        egui::Stroke::new(1.0, visuals.error_fg_color)
                    } else {
                        egui::Stroke::new(1.0, visuals.widgets.active.bg_stroke.color)
                    };
                    painter.rect_filled(rect, GROUP_CORNER_RADIUS, fill);
                    painter.rect_stroke(rect, GROUP_CORNER_RADIUS, stroke, egui::StrokeKind::Inside);
                    // A hard tick on the left edge: the marker's width is its
                    // label's, but its *start* is exactly this line.
                    painter.line_segment([rect.left_top(), rect.left_bottom()], egui::Stroke::new(2.0, visuals.strong_text_color()));
                    painter.galley(
                        egui::pos2(rect.left() + 8.0, rect.center().y - marker.galley.size().y * 0.5),
                        marker.galley.clone(),
                        visuals.strong_text_color(),
                    );

                    if response.clicked() {
                        selected = Some(bar.id);
                    }
                    // Primary only: the middle button pans the timeline and the
                    // secondary one opens this bar's menu.
                    if response.drag_started_by(egui::PointerButton::Primary) {
                        selected = Some(bar.id);
                        let pointer = response.interact_pointer_pos().map_or(x, |pos| pos.x);
                        drag = Some(GanttDrag {
                            session,
                            bar: bar.id,
                            from_seconds: bar.earliest_start_h * GanttView::HOUR,
                            grab_offset_seconds: view.seconds_at(pointer, body.left(), body.width()) - bar.earliest_start_h * GanttView::HOUR,
                            preview_seconds: bar.earliest_start_h * GanttView::HOUR,
                            moved: false,
                        });
                    }
                    let menu_label = bar.name().to_owned();
                    context_menu_popup(&response, &menu_label, |ui| {
                        if ContextMenuAction::new(tr!("schedule-bar-edit-sequence")).show(ui).clicked() {
                            // Opening reads the bar and nothing else: the
                            // draft is editor state, and no geometry, run or
                            // pipeline demand is touched by opening a window.
                            open_editor = Some(crate::ui::state::SequenceDraft::open(session, bar.id, bar.members()));
                            ui.close();
                        }
                        if ContextMenuAction::new(tr!("schedule-rename-bar-action")).show(ui).clicked() {
                            open_dialog = Some(BarNameDialog {
                                target: Some(bar.id),
                                agent: bar.agent,
                                priority: bar.priority,
                                earliest_start_h: bar.earliest_start_h,
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
                        for agent in plan.agents() {
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
                    response.on_hover_text(bar_tooltip(bar, report, start_seconds));
                }
            }
        }
    }

    // One drag is one edit, committed here rather than every frame, so a bar
    // dragged across a week is one Ctrl-Z and not several hundred. A drag that
    // never moved is a selection and commits nothing.
    if let Some(finished) = released
        && finished.moved
        && finished.preview_seconds != finished.from_seconds
    {
        commands.push(UiCommand::schedule(
            session,
            ScheduleEdit::SetBarEarliestStart {
                bar: finished.bar,
                hours: finished.preview_seconds / GanttView::HOUR,
            },
        ));
    }
    editor.gantt_drag = drag;
    editor.schedule_selected_bar = selected.filter(|id| plan.bar(*id).is_some());
    if let Some(dialog) = open_dialog {
        editor.bar_name_dialog = Some(dialog);
    }
    if let Some(draft) = open_editor {
        editor.sequence_editor = Some(draft);
    }
}

/// The empty space of each row: right-click it to add a bar to that machine,
/// in that lane.
///
/// Sensed after the bars, so a right-click that lands on a bar opens the bar's
/// menu rather than this one.
fn draw_row_menus(ui: &mut egui::Ui, body: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, rows: &[Row]) {
    if !body.is_positive() {
        return;
    }
    let scroll = editor.gantt.row_scroll;
    let view = editor.gantt;
    let mut open_dialog = None;
    for (index, row) in rows.iter().enumerate() {
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
            context_menu_popup(&response, title, |ui| {
                if ContextMenuAction::new(tr!("schedule-new-bar")).show(ui).clicked() {
                    // Seeded as the dialog opens, not while it is on screen: a
                    // name refilled every frame could never be cleared and
                    // retyped.
                    // The new bar lands in the row, the lane and at the instant
                    // it was asked for, rather than unassigned at hour zero
                    // somewhere off screen: right-clicking a machine's lane at
                    // a place on the timeline is how the user says all three.
                    open_dialog = Some(BarNameDialog {
                        target: None,
                        agent: row.agent,
                        priority,
                        earliest_start_h: ui.data(|data| data.get_temp::<f64>(opened_at)).unwrap_or(0.0) / GanttView::HOUR,
                        name: crate::ui::elements::schedule_setup::suggested_bar_name(plan),
                    });
                    ui.close();
                }
            });
        }
    }
    if let Some(dialog) = open_dialog {
        editor.bar_name_dialog = Some(dialog);
    }
}

/// Vertical grid lines under the rows, on the same ticks the ruler labels.
fn draw_grid(ui: &egui::Ui, rect: egui::Rect, view: GanttView, interval: f64) {
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
fn centred_note(ui: &egui::Ui, rect: egui::Rect, text: String) {
    ui.painter_at(rect).text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().weak_text_color(),
    );
}
