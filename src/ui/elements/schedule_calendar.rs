//! Virtualized period calendar: the authored inputs per period - loader
//! availability, utilisation and rates, truck fleets, crusher budgets - and
//! beneath them the calculated figures one accepted schedule run reports.
//!
//! Every calculated figure is read off the one shared
//! [`CalculatedSchedule`]'s per-period aggregates, which were built once when
//! it was published: dig and reclaim tonnes per loader, truck-hours per class,
//! receipts, reclaim and closing stock per destination, and movement value.
//! Nothing here applies a rate or an availability factor a second time.
//! A covered period with no activity reads zero for a flow and its standing
//! balance for a stock; a period the run did not reach is blank.

use thousands::Separable;

use crate::{
    i18n::{tr, tr_format},
    model::{
        Document,
        schedule::{
            CalendarCell, CalendarCellEdit, CalendarField, CalendarPeriod, CrusherCell, CrusherCellEdit, CrusherOverride, DestinationId, DestinationKind, LoaderAgent,
            SCHEDULE_PERIOD_H, SchedulePlan, StandaloneDestinationId, TruckCellEdit, TruckField, destinations,
            grade_targets::{GradeTargetCellEdit, GradeTargetInput, GradeTargetValue},
            result::CalculatedSchedule,
            stockpile_operation::{PileMode, PileModeCellEdit},
        },
    },
    ui::{
        EditorState, UiProjectView, chrome,
        state::{CalendarCellAddress, CalendarCellDraft, CalendarOwner, CalendarRow, CalendarSelection, PlanningSubpage, ScheduleEdit, ScheduleStep, UiCommand},
    },
};

const HIERARCHY_W: f32 = 240.0;
const DEFAULT_W: f32 = 100.0;
/// The whole calculated schedule's figure for a calculated row, pinned beside
/// the Default column so it stays in view while the days scroll.
const TOTAL_W: f32 = 112.0;
const PERIOD_W: f32 = 112.0;
const HEADER_H: f32 = 30.0;
const TOOLBAR_H: f32 = 34.0;
const OVERSCAN: i32 = 1;
/// Height of the day scroll bar along the bottom of the period columns.
const SCROLLBAR_H: f32 = 12.0;
/// How far each level of the row hierarchy is indented from the one above.
/// The gutter it leaves is shaded in the parent's own colour, so a loader's
/// rows read as sitting inside it rather than merely beside it.
const NEST_INDENT: f32 = 16.0;

/// One drawn row of the grid.
///
/// Two headings and two kinds of group, in one list: the virtualizer, the
/// selection and the clipboard all index the same list, so a destination row is
/// as much part of the rectangle as a loader row is.
#[derive(Clone, Copy)]
enum Row {
    Schedule,
    Loaders,
    Trucks,
    Destinations,
    Group(CalendarOwner),
    Field(CalendarOwner, CalendarRow),
}

/// What the Calendar needs to know about one destination: the two things Solids
/// and the rule list own between them, resolved once per frame.
#[derive(Clone)]
struct DestinationRow {
    id: DestinationId,
    name: String,
    kind: DestinationKind,
}

/// The calculated half of the grid for one frame: the held result, when it is
/// current, and the names its figures are labelled with.
///
/// Names are resolved here, per frame, rather than stored in the result: a
/// renamed grade field relabels a closing-grade hover without recalculating
/// anything.
struct Figures<'a> {
    result: Option<&'a CalculatedSchedule>,
    /// Grade names, aligned with the result's grades.
    grades: Vec<String>,
    currency: String,
    field_names: Vec<(crate::model::ReserveFieldId, String)>,
}

impl Figures<'_> {
    fn field_name(&self, field: crate::model::ReserveFieldId) -> String {
        self.field_names
            .iter()
            .find(|(id, _)| *id == field)
            .map_or_else(|| field.0.to_string(), |(_, name)| name.clone())
    }

    /// The day's priced target behind a grade Actual cell, when it received tonnes.
    fn target_result(&self, address: CalendarCellAddress) -> Option<&crate::model::schedule::result::GradeTargetResult> {
        let CalendarRow::GradeActual(field) = address.row else { return None };
        let CalendarCell::Period(CalendarPeriod(day)) = address.cell else { return None };
        let destination = address.destination()?;
        self.result?
            .grade_targets
            .iter()
            .find(|v| v.period == day && v.specification.destination == destination && v.specification.field == field && v.tonnes > 1e-6)
    }

    /// One calculated cell's figure and whether its period is only partly
    /// covered, or `None` where the calculation answers for nothing: no
    /// current result, a period beyond the calculated horizon, or the Default
    /// column, which no calculation has an opinion about.
    fn figure(&self, address: CalendarCellAddress, kind: Option<DestinationKind>) -> Option<(f64, bool)> {
        let CalendarCell::Period(CalendarPeriod(period)) = address.cell else { return None };
        let periods = &self.result?.periods;
        let value = match address.row {
            CalendarRow::DigTonnes => periods.dig(address.agent()?, period)?,
            CalendarRow::ReclaimTonnes => periods.reclaim(address.agent()?, period)?,
            CalendarRow::TruckHours => periods.truck_hours(address.truck()?, period)?,
            CalendarRow::TruckCycle => periods.truck_haul(address.truck()?, period)?.cycle_minutes(),
            CalendarRow::TruckTonneKm => periods.truck_haul(address.truck()?, period)?.loaded_t_km,
            CalendarRow::Received => periods.received(address.destination()?, period)?,
            CalendarRow::Reclaimed => periods.reclaimed(address.destination()?, period)?,
            CalendarRow::Cumulative => match kind {
                Some(DestinationKind::Stockpile) => periods.closing(address.destination()?, period)?.0,
                _ => periods.cumulative(address.destination()?, period)?,
            },
            CalendarRow::GradeActual(field) => {
                let result = self.result?;
                let grade = result.grades.iter().position(|(id, _)| *id == field)?;
                periods.received_grade(address.destination()?, period, grade)?
            }
            CalendarRow::Value => periods.value(period)?,
            CalendarRow::Input(_) | CalendarRow::Truck(_) | CalendarRow::CrusherLimit | CalendarRow::PileMode | CalendarRow::GradeInput(..) => return None,
        };
        Some((value, periods.is_partial(period)))
    }

    /// A calculated row's figure over the whole calculated schedule: flows
    /// summed, a stock or a deposit at its end, a grade weighted by the
    /// tonnes behind it. `None` for authored rows and with nothing calculated.
    fn total(&self, owner: CalendarOwner, row: CalendarRow, kind: Option<DestinationKind>) -> Option<f64> {
        let result = self.result?;
        let periods = &result.periods;
        let covered = periods.covered_periods();
        if covered == 0 {
            return None;
        }
        let sum = |value: &dyn Fn(u32) -> Option<f64>| (0..covered).map(|period| value(period).unwrap_or(0.0)).sum::<f64>();
        let owner = CalendarCellAddress {
            owner,
            row,
            cell: CalendarCell::Default,
        };
        Some(match row {
            CalendarRow::DigTonnes => sum(&|period| periods.dig(owner.agent()?, period)),
            CalendarRow::ReclaimTonnes => sum(&|period| periods.reclaim(owner.agent()?, period)),
            CalendarRow::TruckHours => sum(&|period| periods.truck_hours(owner.truck()?, period)),
            CalendarRow::TruckCycle => {
                let truck = owner.truck()?;
                let tonnes = sum(&|p| periods.truck_haul(truck, p).map(|h| h.tonnes));
                if tonnes > 0.0 {
                    sum(&|p| periods.truck_haul(truck, p).map(|h| h.cycle_t_h)) * 60.0 / tonnes
                } else {
                    0.0
                }
            }
            CalendarRow::TruckTonneKm => sum(&|p| periods.truck_haul(owner.truck()?, p).map(|h| h.loaded_t_km)),
            CalendarRow::Received => sum(&|period| periods.received(owner.destination()?, period)),
            CalendarRow::Reclaimed => sum(&|period| periods.reclaimed(owner.destination()?, period)),
            CalendarRow::Value => sum(&|period| periods.value(period)),
            CalendarRow::Cumulative => match kind {
                Some(DestinationKind::Stockpile) => periods.closing(owner.destination()?, covered - 1)?.0,
                _ => periods.cumulative(owner.destination()?, covered - 1)?,
            },
            CalendarRow::GradeActual(field) => {
                let destination = owner.destination()?;
                let grade = result.grades.iter().position(|(id, _)| *id == field)?;
                let tonnes = sum(&|period| periods.received(destination, period));
                if tonnes <= 1e-6 {
                    return None;
                }
                sum(&|period| Some(periods.received_grade(destination, period, grade)? * periods.received(destination, period)?)) / tonnes
            }
            CalendarRow::Input(_) | CalendarRow::Truck(_) | CalendarRow::CrusherLimit | CalendarRow::PileMode | CalendarRow::GradeInput(..) => return None,
        })
    }

    /// A stockpile's closing grades for a period, for the hover. Only on
    /// demand: a row per grade would bury the grid.
    fn closing_grades(&self, address: CalendarCellAddress) -> Option<String> {
        let CalendarCell::Period(CalendarPeriod(period)) = address.cell else { return None };
        let (tonnes, contained) = self.result?.periods.closing(address.destination()?, period)?;
        if tonnes <= 1e-6 || contained.is_empty() || self.grades.is_empty() {
            return None;
        }
        let grades: Vec<String> = self
            .grades
            .iter()
            .zip(contained)
            .map(|(name, contained)| format!("{name} {}", trimmed_number(contained / tonnes)))
            .collect();
        Some(tr!("schedule-calendar-closing-grades", grades = grades.join(" · ")))
    }
}

/// Every destination the Calendar shows, in the order the Setup pages list them.
///
/// Only present when routing is on: a project that does not route has nothing to
/// report per destination, and a group of empty rows would read as a result.
///
/// The plan comes from the *active project*, not from `document`: `document` is
/// the render-scene composite, which clones the reserve fields and the solids
/// every page reads but carries no schedule of its own.
fn destination_rows(plan: &SchedulePlan, document: &Document) -> Vec<DestinationRow> {
    let routing = plan.routing();
    if !routing.enabled {
        return Vec::new();
    }
    destinations::available(document.solids(), routing)
        .into_iter()
        .map(|entry| DestinationRow {
            id: entry.id,
            name: entry.name,
            kind: entry.kind,
        })
        .collect()
}

/// The rows one destination group shows, which depend on what it does with what
/// it receives.
fn destination_row_kinds(kind: DestinationKind) -> &'static [CalendarRow] {
    match kind {
        // A crusher has no storage, so it has no inventory to report - and it
        // has an editable daily budget and grade inputs beneath its receipts.
        DestinationKind::Crusher => &[CalendarRow::CrusherLimit, CalendarRow::Received],
        DestinationKind::Stockpile => &[CalendarRow::PileMode, CalendarRow::Received, CalendarRow::Reclaimed, CalendarRow::Cumulative],
        DestinationKind::Dump => &[CalendarRow::Received, CalendarRow::Cumulative],
    }
}

pub(crate) fn draw_details(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, document: &Document, commands: &mut Vec<UiCommand>) -> egui::Rect {
    if editor.schedule_calendar.runtime != project.active_session {
        editor.schedule_calendar = Default::default();
        editor.schedule_calendar.runtime = project.active_session;
    }
    let plan = &project.schedule;
    // Held by value for the frame: the grid needs the mirrored result while it
    // also holds the editor mutably, and the clone is one refcount.
    let result = editor.schedule_result.clone();
    let figures = Figures {
        result: result.as_deref(),
        grades: result
            .as_deref()
            .map(|result| {
                result
                    .grades
                    .iter()
                    .map(|(field, _)| {
                        document
                            .reserve_fields()
                            .iter()
                            .find(|entry| entry.id == *field)
                            .map_or_else(|| format!("{}", field.0), |entry| entry.name.clone())
                    })
                    .collect()
            })
            .unwrap_or_default(),
        currency: plan.currency().to_owned(),
        field_names: document.reserve_fields().iter().map(|f| (f.id, f.name.clone())).collect(),
    };
    let destinations = destination_rows(plan, document);
    editor.schedule_calendar.visible_days = editor.schedule_calendar.visible_days.max(required_days(plan, figures.result));
    egui::CentralPanel::default()
        .frame(chrome::region_frame(ui))
        .show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            let toolbar = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), TOOLBAR_H.min(rect.height())));
            let grid = egui::Rect::from_min_max(egui::pos2(rect.left(), toolbar.bottom()), rect.max);
            draw_toolbar(ui, toolbar, editor, plan, document, commands);
            if plan.agents().is_empty() && destinations.is_empty() && plan.trucks().classes.is_empty() {
                draw_empty(ui, grid, editor);
            } else if grid.is_positive() {
                draw_grid(ui, grid, editor, plan, &destinations, &figures, project.active_session, commands);
            }
            ui.allocate_rect(rect, egui::Sense::hover());
        })
        .response
        .rect
}

/// The Calendar's own toolbar: the schedule run controls, and whatever the
/// held result or the last edit has to say.
///
/// Days are reached by scrolling rather than by asking for them: the extent
/// already covers the authored overrides, the sequenced bars and the
/// calculated interval, so a button that added a fortnight of empty columns
/// and a box that jumped to one were two ways of saying the same thing the
/// scroll bar says.
fn draw_toolbar(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, document: &Document, commands: &mut Vec<UiCommand>) {
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("schedule_calendar_toolbar").max_rect(rect));
    child.set_clip_rect(child.clip_rect().intersect(rect));
    child.horizontal_centered(|ui| {
        super::schedule_gantt::draw_calculation_controls(ui, editor, "calendar", commands);
        ui.add_space(8.0);
        draw_report_menu(ui, editor, plan, document, commands);
        // Said once, here: the alternative is repeating it in every
        // calculated row. The same status the Gantt shows, with the same
        // detail behind it.
        ui.add_space(8.0);
        super::schedule_gantt::draw_run_status(ui, editor);
        if let Some(error) = &editor.schedule_calendar.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    });
}

/// The report export: how to group the rows, then copy or save. Built from
/// the held result when asked, never per frame.
fn draw_report_menu(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, document: &Document, commands: &mut Vec<UiCommand>) {
    use super::schedule_report::{ReportGrouping, ReportNames, build, to_text};
    let ready = editor.schedule_result.is_some();
    let button = ui
        .add_enabled(
            ready,
            egui::Button::new(tr!("report-export")).corner_radius(crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS),
        )
        .on_hover_text(tr!("report-export-help"))
        .on_disabled_hover_text(tr!("report-export-unavailable"));
    let text = |editor: &EditorState, separator: char| -> Option<(String, String)> {
        let schedule = editor.schedule_result.as_deref()?;
        let destinations = destinations::available(document.solids(), plan.routing())
            .into_iter()
            .map(|view| (view.id, view.name, view.kind))
            .collect();
        let grades = schedule
            .grades
            .iter()
            .map(|(field, _)| {
                document
                    .reserve_fields()
                    .iter()
                    .find(|entry| entry.id == *field)
                    .map_or_else(|| field.0.to_string(), |entry| entry.name.clone())
            })
            .collect();
        let names = ReportNames {
            destinations,
            grades,
            bar_views: &editor.schedule_bar_reports,
        };
        let grouping = editor.schedule_report_grouping;
        let tables = build(schedule, plan, &names, grouping);
        let heading = tr!(
            "report-heading",
            grouping = grouping.label(),
            end = super::schedule_gantt::instant_label(schedule.requested_end_h * crate::ui::state::GanttView::HOUR)
        );
        Some((tr!("report-file-name", grouping = grouping.label()), to_text(&tables, &heading, separator)))
    };
    // Stays open while the grouping is chosen; closes on Copy, Save or a
    // click outside.
    crate::ui::widgets::context_menu::checklist_popup(&button, tr!("report-export-title"), 240.0, |ui| {
        ui.label(egui::RichText::new(tr!("report-group-by")).weak());
        for grouping in ReportGrouping::ALL {
            ui.radio_value(&mut editor.schedule_report_grouping, grouping, grouping.label());
        }
        ui.separator();
        if ui.button(tr!("report-copy")).on_hover_text(tr!("report-copy-help")).clicked() {
            if let Some((_, text)) = text(editor, '\t') {
                ui.ctx().copy_text(text);
                crate::userspace_log!("{}", tr!("report-copied"));
            }
            ui.close();
        }
        if ui.button(tr!("report-save")).clicked() {
            if let Some((file_name, text)) = text(editor, ',') {
                commands.push(UiCommand::ExportScheduleReport(Box::new((file_name, text))));
            }
            ui.close();
        }
    });
}

fn draw_empty(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.label(tr!("schedule-calendar-empty"));
                if ui.button(tr!("schedule-calendar-add-loader")).clicked() {
                    editor.schedule_subpage = PlanningSubpage::Setup;
                    editor.schedule_setup_step = ScheduleStep::LoaderAgents;
                }
            });
        });
    });
}

fn required_days(plan: &SchedulePlan, result: Option<&CalculatedSchedule>) -> u32 {
    let override_day = plan
        .agents()
        .iter()
        .flat_map(|agent| agent.calendar.periods.keys())
        .chain(plan.routing().standalone.iter().flat_map(|entry| entry.crusher.periods.keys()))
        .chain(plan.trucks().classes.iter().flat_map(|class| class.calendar.periods.keys()))
        .chain(plan.crusher_grade_calendars().iter().flat_map(|calendar| calendar.periods.keys()))
        .map(|period| period.0.saturating_add(2))
        .max()
        .unwrap_or(0);
    let bar_day = plan
        .bars()
        .iter()
        .map(|bar| match bar.window.end_h {
            Some(end) => ((end / SCHEDULE_PERIOD_H).ceil() as u32).saturating_add(1),
            None => (bar.window.start_h / SCHEDULE_PERIOD_H).floor() as u32 + 2,
        })
        .max()
        .unwrap_or(0);
    // Whatever the calculation answers for has to be reachable, or its own
    // figures would sit past the end of the grid.
    let calculated_day = result.map_or(0, |result| result.periods.covered_periods());
    14_u32.max(override_day).max(bar_day).max(calculated_day)
}

fn rows(plan: &SchedulePlan, destinations: &[DestinationRow], editor: &EditorState) -> Vec<Row> {
    let mut rows = vec![Row::Schedule, Row::Field(CalendarOwner::Schedule, CalendarRow::Value), Row::Loaders];
    for agent in plan.agents() {
        let owner = CalendarOwner::Loader(agent.id);
        rows.push(Row::Group(owner));
        if !editor.schedule_calendar.collapsed.contains(&owner) {
            rows.extend(LOADER_ROWS.iter().map(|row| Row::Field(owner, *row)));
        }
    }
    if !plan.trucks().classes.is_empty() {
        rows.push(Row::Trucks);
        for class in &plan.trucks().classes {
            let owner = CalendarOwner::Truck(class.id);
            rows.push(Row::Group(owner));
            if !editor.schedule_calendar.collapsed.contains(&owner) {
                rows.extend(TRUCK_ROWS.iter().map(|row| Row::Field(owner, *row)));
            }
        }
    }
    if !destinations.is_empty() {
        rows.push(Row::Destinations);
        for destination in destinations {
            let owner = CalendarOwner::Destination(destination.id);
            rows.push(Row::Group(owner));
            if !editor.schedule_calendar.collapsed.contains(&owner) {
                rows.extend(destination_row_kinds(destination.kind).iter().map(|row| Row::Field(owner, *row)));
                rows.extend(destination_grade_rows(plan, destination, editor).into_iter().map(|row| Row::Field(owner, row)));
            }
        }
    }
    rows
}

/// A crusher's grade rows, shared by drawing, keyboard navigation and
/// rectangular clipboard edits: per tracked grade, the received grade, and
/// beneath it the target's inputs while that grade is expanded.
fn destination_grade_rows(plan: &SchedulePlan, destination: &DestinationRow, editor: &EditorState) -> Vec<CalendarRow> {
    if destination.kind != DestinationKind::Crusher {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for &(field, _) in &plan.experiment().grades {
        rows.push(CalendarRow::GradeActual(field));
        if editor.schedule_calendar.grade_expanded.contains(&(destination.id, field)) {
            rows.extend(GradeTargetInput::ALL.into_iter().map(|input| CalendarRow::GradeInput(field, input)));
        }
    }
    rows
}

/// The rows one truck class shows, in drawn order: three authored, and the
/// truck-hours a calculated schedule used beneath them.
const TRUCK_ROWS: [CalendarRow; 6] = [
    CalendarRow::Truck(TruckField::Units),
    CalendarRow::Truck(TruckField::Availability),
    CalendarRow::Truck(TruckField::Utilisation),
    CalendarRow::TruckHours,
    CalendarRow::TruckCycle,
    CalendarRow::TruckTonneKm,
];

/// The rows one loader group shows, in drawn order.
///
/// Two rates, one beneath the other: availability and utilisation are the
/// machine's and apply to both, so they stay above the pair rather than being
/// repeated under each.
///
/// Dig and reclaim tonnes are two rows, never summed: reclaim moves material
/// that was already mined.
const LOADER_ROWS: [CalendarRow; 6] = [
    CalendarRow::Input(CalendarField::Availability),
    CalendarRow::Input(CalendarField::Utilisation),
    CalendarRow::Input(CalendarField::Rate),
    CalendarRow::Input(CalendarField::ReclaimRate),
    CalendarRow::DigTonnes,
    CalendarRow::ReclaimTonnes,
];

#[allow(clippy::too_many_arguments)]
fn draw_grid(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    destinations: &[DestinationRow],
    figures: &Figures<'_>,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let row_h = crate::ui::widgets::explorer::row_height(ui);
    let scrollbar_top = (rect.bottom() - SCROLLBAR_H).max(rect.top() + HEADER_H);
    let body = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + HEADER_H), egui::pos2(rect.right(), scrollbar_top));
    let period_left = rect.left() + HIERARCHY_W + DEFAULT_W + TOTAL_W;
    let period_view = egui::Rect::from_min_max(egui::pos2(period_left, rect.top()), egui::pos2(rect.right(), scrollbar_top));
    let rows = rows(plan, destinations, editor);
    let max_y = (rows.len() as f32 * row_h - body.height()).max(0.0);
    let max_x = (editor.schedule_calendar.visible_days as f32 * PERIOD_W - period_view.width()).max(0.0);
    if ui.rect_contains_pointer(rect) {
        let scroll = ui.input(|input| input.smooth_scroll_delta);
        editor.schedule_calendar.scroll_y = (editor.schedule_calendar.scroll_y - scroll.y).clamp(0.0, max_y);
        editor.schedule_calendar.scroll_x = (editor.schedule_calendar.scroll_x - scroll.x).clamp(0.0, max_x);
    }
    editor.schedule_calendar.scroll_y = editor.schedule_calendar.scroll_y.clamp(0.0, max_y);
    editor.schedule_calendar.scroll_x = editor.schedule_calendar.scroll_x.clamp(0.0, max_x);

    handle_keyboard(ui, editor, plan, destinations, figures, session, commands);
    let visuals = ui.visuals().clone();
    let stroke = visuals.widgets.noninteractive.bg_stroke;
    ui.painter().rect_filled(rect, 0.0, crate::ui::widgets::tree_row_colors(ui).1);
    let hierarchy_head = egui::Rect::from_min_size(rect.min, egui::vec2(HIERARCHY_W, HEADER_H));
    let default_head = egui::Rect::from_min_size(egui::pos2(hierarchy_head.right(), rect.top()), egui::vec2(DEFAULT_W, HEADER_H));
    paint_header(ui, hierarchy_head, tr!("schedule-calendar-setting"), None);
    paint_header(ui, default_head, tr!("schedule-calendar-default"), None);
    let total_head = egui::Rect::from_min_size(egui::pos2(default_head.right(), rect.top()), egui::vec2(TOTAL_W, HEADER_H));
    paint_header(ui, total_head, tr!("schedule-calendar-total"), Some(tr!("schedule-calendar-total-help")));

    let first_period = ((editor.schedule_calendar.scroll_x / PERIOD_W).floor() as i32 - OVERSCAN).max(0) as u32;
    let last_period = (((editor.schedule_calendar.scroll_x + period_view.width()) / PERIOD_W).ceil() as i32 + OVERSCAN)
        .max(0)
        .min(editor.schedule_calendar.visible_days as i32) as u32;
    for period in first_period..last_period {
        let x = period_left + period as f32 * PERIOD_W - editor.schedule_calendar.scroll_x;
        let cell = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(PERIOD_W, HEADER_H)).intersect(period_view);
        paint_header(
            ui,
            cell,
            tr!("schedule-calendar-day", day = period.saturating_add(1).to_string()),
            Some(tr!(
                "schedule-calendar-hours",
                start = format!("{:.0}", f64::from(period) * SCHEDULE_PERIOD_H),
                end = format!("{:.0}", f64::from(period.saturating_add(1)) * SCHEDULE_PERIOD_H)
            )),
        );
    }

    let first_row = ((editor.schedule_calendar.scroll_y / row_h).floor() as i32 - OVERSCAN).max(0) as usize;
    let last_row = (((editor.schedule_calendar.scroll_y + body.height()) / row_h).ceil() as i32 + OVERSCAN)
        .max(0)
        .min(rows.len() as i32) as usize;
    for (row_index, row) in rows.iter().enumerate().take(last_row).skip(first_row) {
        let y = body.top() + row_index as f32 * row_h - editor.schedule_calendar.scroll_y;
        let row_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(rect.width(), row_h)).intersect(body);
        draw_row(
            ui,
            row_rect,
            *row,
            editor,
            plan,
            destinations,
            figures,
            first_period..last_period,
            period_left,
            session,
            commands,
        );
    }
    ui.painter().rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
    ui.painter()
        .line_segment([egui::pos2(hierarchy_head.right(), rect.top()), egui::pos2(hierarchy_head.right(), body.bottom())], stroke);
    ui.painter()
        .line_segment([egui::pos2(default_head.right(), rect.top()), egui::pos2(default_head.right(), body.bottom())], stroke);
    ui.painter()
        .line_segment([egui::pos2(total_head.right(), rect.top()), egui::pos2(total_head.right(), body.bottom())], stroke);
    ui.painter()
        .line_segment([egui::pos2(rect.left(), body.top()), egui::pos2(rect.right(), body.top())], stroke);
    draw_day_scrollbar(
        ui,
        egui::Rect::from_min_max(egui::pos2(period_left, scrollbar_top), rect.max),
        editor,
        period_view.width(),
        max_x,
    );
}

/// The day scroll bar: how the grid is moved across the horizon now that no
/// button extends it and no box jumps to a day.
///
/// Hand-painted like the grid above it, and for the same reason - the columns
/// are virtualized, so there is no laid-out content for a `ScrollArea` to
/// measure.
fn draw_day_scrollbar(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, view_width: f32, max_x: f32) {
    if !rect.is_positive() {
        return;
    }
    let total = max_x + view_width;
    ui.painter().rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    if max_x <= 0.0 || total <= 0.0 || view_width <= 0.0 {
        return;
    }
    let track = rect.shrink2(egui::vec2(1.0, 3.0));
    let response = ui.interact(rect, ui.id().with("schedule_calendar_day_scroll"), egui::Sense::click_and_drag());
    // A thumb narrower than this cannot be grabbed, so a long horizon stops
    // shrinking it and gives up proportionality instead.
    const MIN_THUMB: f32 = 24.0;
    let thumb_w = (track.width() * view_width / total).clamp(MIN_THUMB.min(track.width()), track.width());
    let travel = track.width() - thumb_w;
    if let Some(pointer) = response.interact_pointer_pos()
        && travel > 0.0
    {
        // Dragged from wherever it was grabbed, so the thumb does not jump
        // its own half-width under the pointer on the first press.
        let grab = ui.id().with("schedule_calendar_day_scroll_grab");
        let offset = if response.drag_started() || response.clicked() {
            let thumb_x = track.left() + travel * (editor.schedule_calendar.scroll_x / max_x);
            let inside = pointer.x - thumb_x;
            let inside = if (0.0..=thumb_w).contains(&inside) { inside } else { thumb_w / 2.0 };
            ui.data_mut(|data| data.insert_temp(grab, inside));
            inside
        } else {
            ui.data(|data| data.get_temp::<f32>(grab)).unwrap_or(thumb_w / 2.0)
        };
        let fraction = ((pointer.x - offset - track.left()) / travel).clamp(0.0, 1.0);
        editor.schedule_calendar.scroll_x = fraction * max_x;
    }
    let thumb_x = track.left() + travel * (editor.schedule_calendar.scroll_x / max_x);
    let thumb = egui::Rect::from_min_size(egui::pos2(thumb_x, track.top()), egui::vec2(thumb_w, track.height()));
    let visuals = ui.style().interact(&response);
    ui.painter().rect_filled(thumb, track.height() / 2.0, visuals.bg_fill);
}

fn paint_header(ui: &mut egui::Ui, rect: egui::Rect, text: String, hover: Option<String>) {
    if !rect.is_positive() {
        return;
    }
    ui.painter().rect_filled(rect, 0.0, ui.visuals().widgets.noninteractive.bg_fill);
    let response = ui.put(rect.shrink(5.0), egui::Label::new(crate::ui::fonts::bold(&text)).truncate());
    if let Some(hover) = hover {
        response.on_hover_text(hover);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    row: Row,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    destinations: &[DestinationRow],
    figures: &Figures<'_>,
    periods: std::ops::Range<u32>,
    period_left: f32,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    if !rect.is_positive() {
        return;
    }
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
    let hierarchy = egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + HIERARCHY_W, rect.bottom()));
    // The gutter every level above this row leaves to its left, each shaded
    // in that level's own colour. Unbroken down the column, so a loader's
    // three settings read as being inside the loader, which is inside Loaders.
    let depth = match row {
        Row::Schedule | Row::Loaders | Row::Trucks | Row::Destinations => 0,
        Row::Group(_) | Row::Field(CalendarOwner::Schedule, _) => 1,
        Row::Field(_, CalendarRow::GradeInput(..)) => 3,
        Row::Field(..) => 2,
    };
    for level in 0..depth {
        let band = egui::Rect::from_min_max(
            egui::pos2(hierarchy.left() + level as f32 * NEST_INDENT, rect.top()),
            egui::pos2(hierarchy.left() + (level + 1) as f32 * NEST_INDENT, rect.bottom()),
        );
        ui.painter().rect_filled(band, 0.0, nest_fill(ui, level));
    }
    let indent = depth as f32 * NEST_INDENT;
    let label_start = hierarchy.left() + indent;
    match row {
        Row::Schedule | Row::Loaders | Row::Trucks | Row::Destinations => {
            let label = match row {
                Row::Schedule => tr!("schedule-calendar-schedule"),
                Row::Loaders => tr!("schedule-calendar-loaders"),
                Row::Trucks => tr!("schedule-calendar-trucks"),
                _ => tr!("destination-destinations"),
            };
            ui.painter().rect_filled(hierarchy, 0.0, nest_fill(ui, 0));
            paint_label(ui, hierarchy, label_start + 8.0, &label, LabelStyle::Heading);
        }
        Row::Group(owner) => {
            let name = match owner {
                CalendarOwner::Schedule => return,
                CalendarOwner::Loader(id) => match plan.agent(id) {
                    Some(agent) => agent.name.clone(),
                    None => return,
                },
                CalendarOwner::Truck(id) => match plan.trucks().class(id) {
                    Some(class) => class.name.clone(),
                    None => return,
                },
                CalendarOwner::Destination(id) => match destinations.iter().find(|entry| entry.id == id) {
                    Some(entry) => tr_format!(literal = "%name% · %kind%", name = entry.name.clone(), kind = entry.kind.label()),
                    None => return,
                },
            };
            let row_rect = egui::Rect::from_min_max(egui::pos2(label_start, rect.top()), egui::pos2(hierarchy.right(), rect.bottom()));
            ui.painter().rect_filled(row_rect, 0.0, nest_fill(ui, 1));
            let collapsed = editor.schedule_calendar.collapsed.contains(&owner);
            let label = format!("{}  {}", if collapsed { "▸" } else { "▾" }, name);
            // Interacted with rather than laid out as a button: a widget put
            // over the row would centre its text, and the indent is the whole
            // point of the row.
            let response = ui.interact(row_rect, ui.id().with(("schedule_calendar_group", owner)), egui::Sense::click());
            paint_label(ui, row_rect, label_start + 8.0, &label, LabelStyle::Heading);
            if response.clicked() {
                if collapsed {
                    editor.schedule_calendar.collapsed.remove(&owner);
                } else {
                    editor.schedule_calendar.collapsed.insert(owner);
                }
            }
        }
        Row::Field(owner, kind) => {
            let agent = match owner {
                CalendarOwner::Schedule => None,
                CalendarOwner::Loader(id) => match plan.agent(id) {
                    Some(agent) => Some(agent),
                    None => return,
                },
                CalendarOwner::Truck(id) => {
                    if plan.trucks().class(id).is_none() {
                        return;
                    }
                    None
                }
                CalendarOwner::Destination(id) => {
                    if !destinations.iter().any(|entry| entry.id == id) {
                        return;
                    }
                    None
                }
            };
            let destination_kind = match owner {
                CalendarOwner::Destination(id) => destinations.iter().find(|entry| entry.id == id).map(|entry| entry.kind),
                CalendarOwner::Schedule | CalendarOwner::Loader(_) | CalendarOwner::Truck(_) => None,
            };
            let text_left = label_start + 18.0;
            let label = row_label(kind, destination_kind, &figures.currency, figures);
            if let (CalendarRow::GradeActual(field), CalendarOwner::Destination(destination)) = (kind, owner) {
                // The received grade heads its target's inputs, which fold
                // away beneath it: the grade is the summary, the band detail.
                let key = (destination, field);
                let expanded = editor.schedule_calendar.grade_expanded.contains(&key);
                let label_rect = egui::Rect::from_min_max(egui::pos2(label_start, rect.top()), egui::pos2(hierarchy.right(), rect.bottom()));
                let response = ui
                    .interact(label_rect, ui.id().with(("schedule_calendar_grade", destination, field)), egui::Sense::click())
                    .on_hover_text(tr!("grade-calendar-expand-help"));
                paint_label(
                    ui,
                    hierarchy,
                    label_start + 4.0,
                    &format!("{}  {label}", if expanded { "▾" } else { "▸" }),
                    LabelStyle::Field,
                );
                if response.clicked() {
                    if expanded {
                        editor.schedule_calendar.grade_expanded.remove(&key);
                    } else {
                        editor.schedule_calendar.grade_expanded.insert(key);
                    }
                }
            } else if kind.is_calculated() {
                // A lock beside a quieter label: the tint alone would be the
                // only thing saying this row cannot be typed into, and colour
                // alone is not enough to say it.
                paint_lock(ui, egui::pos2(text_left - 10.0, rect.center().y), ui.visuals().weak_text_color());
                paint_label(ui, hierarchy, text_left, &label, LabelStyle::Calculated);
            } else {
                paint_label(ui, hierarchy, text_left, &label, LabelStyle::Field);
            }
            let default_rect = egui::Rect::from_min_max(egui::pos2(hierarchy.right(), rect.top()), egui::pos2(hierarchy.right() + DEFAULT_W, rect.bottom()));
            draw_cell(
                ui,
                default_rect,
                CalendarCellAddress {
                    owner,
                    row: kind,
                    cell: CalendarCell::Default,
                },
                editor,
                plan,
                destinations,
                agent,
                figures,
                session,
                commands,
            );
            // The whole schedule's figure: read-only, outside the selection,
            // and blank on an authored row.
            let total_rect = egui::Rect::from_min_max(egui::pos2(default_rect.right(), rect.top()), egui::pos2(default_rect.right() + TOTAL_W, rect.bottom()));
            if kind.is_calculated() {
                ui.painter().rect_filled(total_rect, 0.0, ui.visuals().faint_bg_color);
                if let Some(total) = figures.total(owner, kind, destination_kind) {
                    ui.painter().with_clip_rect(total_rect).text(
                        total_rect.right_center() - egui::vec2(6.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        format_figure(kind, total),
                        crate::ui::fonts::bold_font(egui::TextStyle::Body.resolve(ui.style()).size),
                        ui.visuals().text_color(),
                    );
                }
            }
            for period in periods {
                let x = period_left + period as f32 * PERIOD_W - editor.schedule_calendar.scroll_x;
                let cell = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(PERIOD_W, rect.height()))
                    .intersect(egui::Rect::from_min_max(egui::pos2(period_left, rect.top()), rect.max));
                draw_cell(
                    ui,
                    cell,
                    CalendarCellAddress {
                        owner,
                        row: kind,
                        cell: CalendarCell::Period(CalendarPeriod(period)),
                    },
                    editor,
                    plan,
                    destinations,
                    agent,
                    figures,
                    session,
                    commands,
                );
            }
        }
    }
}

/// What one row is called. A crusher's receipts are what it *processed*, and a
/// stockpile's balance is its closing inventory, opening stock included.
fn row_label(row: CalendarRow, destination: Option<DestinationKind>, currency: &str, figures: &Figures<'_>) -> String {
    match row {
        CalendarRow::Input(CalendarField::Availability) => tr!("schedule-calendar-availability"),
        CalendarRow::Input(CalendarField::Utilisation) => tr!("schedule-calendar-utilisation"),
        CalendarRow::Input(CalendarField::Rate) => tr!("schedule-calendar-dig-rate"),
        CalendarRow::Input(CalendarField::ReclaimRate) => tr!("schedule-calendar-reclaim-rate"),
        CalendarRow::Truck(TruckField::Units) => tr!("truck-calendar-units"),
        CalendarRow::Truck(TruckField::Availability) => tr!("truck-calendar-availability"),
        CalendarRow::Truck(TruckField::Utilisation) => tr!("truck-calendar-utilisation"),
        CalendarRow::TruckHours => tr!("truck-calendar-hours-used"),
        CalendarRow::TruckCycle => tr!("haul-cycle"),
        CalendarRow::TruckTonneKm => tr!("haul-tonne-km"),
        CalendarRow::DigTonnes => tr!("schedule-calendar-dig-tonnes"),
        CalendarRow::ReclaimTonnes => tr!("schedule-calendar-reclaim-tonnes"),
        CalendarRow::CrusherLimit => tr!("destination-calendar-limit"),
        CalendarRow::PileMode => tr!("pile-mode-row"),
        CalendarRow::Received => match destination {
            Some(DestinationKind::Crusher) => tr!("destination-calendar-processed"),
            _ => tr!("destination-calendar-received"),
        },
        CalendarRow::Reclaimed => tr!("destination-calendar-reclaimed"),
        CalendarRow::Cumulative => match destination {
            Some(DestinationKind::Dump) => tr!("destination-calendar-deposited"),
            _ => tr!("destination-calendar-closing"),
        },
        // Nested under their grade, so they need not repeat its name.
        CalendarRow::GradeInput(_, GradeTargetInput::Penalty) => tr!("grade-calendar-penalty-row", currency = currency.to_owned()),
        CalendarRow::GradeInput(_, input) => input.label(),
        CalendarRow::GradeActual(field) => figures.field_name(field),
        CalendarRow::Value => tr!("schedule-calendar-value", currency = currency.to_owned()),
    }
}

/// How a row label is drawn: the two hierarchy levels are headings, a
/// setting's name is ordinary text, and a calculated row's is quieter.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LabelStyle {
    Heading,
    Field,
    Calculated,
}

/// One row label, left-aligned at its own indent and clipped to the hierarchy
/// column.
///
/// Painted rather than laid out: a widget put into the row would centre its
/// text, and centred text says nothing about which level it sits at.
fn paint_label(ui: &egui::Ui, row: egui::Rect, left: f32, text: &str, style: LabelStyle) {
    let size = egui::TextStyle::Body.resolve(ui.style()).size;
    let (font, color) = match style {
        LabelStyle::Heading => (crate::ui::fonts::bold_font(size), ui.visuals().strong_text_color()),
        LabelStyle::Field => (egui::FontId::proportional(size), ui.visuals().text_color()),
        LabelStyle::Calculated => (egui::FontId::proportional(size), ui.visuals().weak_text_color()),
    };
    let available = (row.right() - 8.0 - left).max(0.0);
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap.max_width = available;
    job.wrap.max_rows = 1;
    let galley = ui.painter().layout_job(job);
    ui.painter()
        .with_clip_rect(row)
        .galley(egui::pos2(left, row.center().y - galley.size().y / 2.0), galley, color);
}

/// The shade one level of the row hierarchy is drawn in.
///
/// Level 0 is the header fill the column titles use, and each level below it
/// steps back towards the row background, so the gutters read as nested bands
/// without a second palette to keep in step with the theme.
fn nest_fill(ui: &egui::Ui, level: usize) -> egui::Color32 {
    let header = ui.visuals().widgets.noninteractive.bg_fill;
    let background = crate::ui::widgets::tree_row_colors(ui).1;
    match level {
        0 => header,
        _ => blend(header, background, 0.5),
    }
}

/// Mix two colours, so a nesting band can be derived from the theme's own
/// fills rather than from constants that only suit one theme.
fn blend(from: egui::Color32, to: egui::Color32, t: f32) -> egui::Color32 {
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round().clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgb(mix(from.r(), to.r()), mix(from.g(), to.g()), mix(from.b(), to.b()))
}

/// A padlock a few pixels across: the shackle first, then the body over its
/// lower half, so the two shapes read as one silhouette at this size.
fn paint_lock(ui: &egui::Ui, center: egui::Pos2, color: egui::Color32) {
    let body = egui::Rect::from_center_size(center + egui::vec2(0.0, 2.0), egui::vec2(7.0, 5.0));
    ui.painter().circle_stroke(egui::pos2(center.x, body.top()), 2.2, egui::Stroke::new(1.2, color));
    ui.painter().rect_filled(body, 1.0, color);
}

#[allow(clippy::too_many_arguments)]
fn draw_cell(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    address: CalendarCellAddress,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    destinations: &[DestinationRow],
    agent: Option<&LoaderAgent>,
    figures: &Figures<'_>,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    if !rect.is_positive() {
        return;
    }
    let selected = selected(editor, plan, destinations, address);
    if address.row.is_calculated() {
        ui.painter().rect_filled(rect, 0.0, ui.visuals().faint_bg_color);
    }
    if selected {
        ui.painter().rect_filled(rect, 0.0, ui.visuals().selection.bg_fill);
    }
    let response = ui.interact(rect, cell_id(ui, address), egui::Sense::click());
    if response.clicked() {
        if !commit_draft(editor, plan, session, commands) {
            return;
        }
        let extend = ui.input(|input| input.modifiers.shift);
        editor.schedule_calendar.selection = Some(match (extend, editor.schedule_calendar.selection) {
            (true, Some(selection)) => CalendarSelection { focus: address, ..selection },
            _ => CalendarSelection { anchor: address, focus: address },
        });
        editor.schedule_calendar.error = None;
    }
    if response.double_clicked() && editable(address) {
        begin_edit(editor, plan, address, None);
    }
    if editor.schedule_calendar.draft.as_ref().is_some_and(|draft| draft.address == address) {
        let mut commit = false;
        let mut abandon = false;
        let mut move_key = None;
        {
            let draft = editor.schedule_calendar.draft.as_mut().expect("checked above");
            let edit = ui.put(rect.shrink2(egui::vec2(3.0, 2.0)), egui::TextEdit::singleline(&mut draft.text));
            if draft.request_focus {
                edit.request_focus();
                draft.request_focus = false;
            }
            // Enter, Tab and Escape each make the editor surrender focus, so
            // the keys arrive on the frame it reports `lost_focus` and never
            // while it still holds focus.
            if edit.has_focus() || edit.lost_focus() {
                let (escape, enter, tab, shift) = ui.input(|input| {
                    (
                        input.key_pressed(egui::Key::Escape),
                        input.key_pressed(egui::Key::Enter),
                        input.key_pressed(egui::Key::Tab),
                        input.modifiers.shift,
                    )
                });
                if escape {
                    abandon = true;
                } else if enter {
                    commit = true;
                    move_key = Some(Nav::Down);
                } else if tab {
                    commit = true;
                    move_key = Some(if shift { Nav::Left } else { Nav::Right });
                } else if edit.lost_focus() {
                    // Focus went somewhere else - another cell, the toolbar -
                    // which commits the draft rather than stranding an editor
                    // nobody is typing in.
                    commit = true;
                }
            }
        }
        if abandon || commit {
            // Tab hands focus to the next widget, and the grid reads the
            // keyboard only while nothing else holds it: without this the cell
            // the caret just moved to would ignore what the user types next.
            ui.memory_mut(|memory| memory.stop_text_input());
        }
        if abandon {
            editor.schedule_calendar.draft = None;
            editor.schedule_calendar.error = None;
        } else if commit
            && commit_draft(editor, plan, session, commands)
            && let Some(nav) = move_key
        {
            move_selection(editor, plan, destinations, nav);
        }
    } else {
        let target = figures.target_result(address);
        let color = if selected {
            ui.visuals().selection.stroke.color
        } else if let Some(v) = target {
            let grade = v.contained / v.tonnes;
            let outside = v.specification.lower.is_some_and(|lower| grade < lower - 1e-9) || v.specification.upper.is_some_and(|upper| grade > upper + 1e-9);
            if outside { ui.visuals().warn_fg_color } else { egui::Color32::from_rgb(72, 170, 110) }
        } else {
            ui.visuals().text_color()
        };
        ui.painter().with_clip_rect(rect).text(
            rect.right_center() - egui::vec2(6.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            display_text(plan, destinations, agent, figures, address),
            egui::TextStyle::Body.resolve(ui.style()),
            color,
        );
    }
    if editor.schedule_calendar.mode_menu == Some(address) {
        draw_mode_menu(ui, rect, address, editor, plan, session, commands);
    } else if let Some(hover) = hover_text(plan, destinations, agent, figures, address) {
        response.on_hover_text(hover);
    }
}

/// The choice list a Mode cell opens beneath itself. A day also offers its
/// Default back.
fn draw_mode_menu(ui: &mut egui::Ui, rect: egui::Rect, address: CalendarCellAddress, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let Some(destination) = address.destination() else {
        editor.schedule_calendar.mode_menu = None;
        return;
    };
    let operation = plan.stockpile_operation(destination);
    let current = match address.cell {
        CalendarCell::Default => Some(operation.default_mode),
        CalendarCell::Period(day) => operation.periods.get(&day).copied(),
    };
    let mut choices: Vec<(Option<PileMode>, String)> = PileMode::ALL.into_iter().map(|mode| (Some(mode), mode.label())).collect();
    if matches!(address.cell, CalendarCell::Period(_)) {
        choices.insert(0, (None, tr!("pile-mode-inherit", mode = operation.default_mode.label())));
    }
    let mut chosen = None;
    let area = egui::Area::new(ui.id().with(("pile_mode_menu", address.owner, address.cell)))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.left_bottom())
        .show(ui.ctx(), |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(rect.width());
                for (mode, label) in &choices {
                    if ui.selectable_label(*mode == current, label).clicked() {
                        chosen = Some(*mode);
                    }
                }
            });
        });
    if let Some(mode) = chosen {
        commands.push(UiCommand::schedule(
            session,
            ScheduleEdit::SetPileModeCells {
                edits: vec![PileModeCellEdit {
                    destination,
                    cell: address.cell,
                    mode,
                }],
            },
        ));
        editor.schedule_calendar.mode_menu = None;
        editor.schedule_calendar.error = None;
        return;
    }
    let dismissed = ui.input(|input| input.key_pressed(egui::Key::Escape))
        || (ui.input(|input| input.pointer.any_pressed()) && !area.response.contains_pointer() && !ui.rect_contains_pointer(rect));
    if dismissed {
        editor.schedule_calendar.mode_menu = None;
    }
}

fn destination_kind(destinations: &[DestinationRow], address: CalendarCellAddress) -> Option<DestinationKind> {
    let id = address.destination()?;
    destinations.iter().find(|entry| entry.id == id).map(|entry| entry.kind)
}

/// A calculated figure as the grid paints it: tonnes and money with
/// separators, hours with one decimal.
fn format_figure(row: CalendarRow, value: f64) -> String {
    match row {
        CalendarRow::TruckHours | CalendarRow::TruckCycle => format_hours(value),
        CalendarRow::TruckTonneKm => format_tonnes(value),
        CalendarRow::GradeActual(_) => trimmed_number(value),
        CalendarRow::Value => format_money(value),
        _ => format_tonnes(value),
    }
}

/// The same figure as plain digits, for the clipboard.
fn raw_figure(row: CalendarRow, value: f64) -> String {
    match row {
        CalendarRow::GradeActual(_) => trimmed_number(value),
        CalendarRow::Value => format!("{:.2}", if value == 0.0 { 0.0 } else { value }),
        _ => tonnes_number(value),
    }
}

/// What a cell shows. A calculated cell says nothing at all outside the
/// calculated horizon, and marks a period the horizon stops part-way through.
fn display_text(plan: &SchedulePlan, destinations: &[DestinationRow], agent: Option<&LoaderAgent>, figures: &Figures<'_>, address: CalendarCellAddress) -> String {
    match address.row {
        CalendarRow::Input(_) => agent.map(|agent| cell_text(plan, agent, address)).unwrap_or_default(),
        CalendarRow::Truck(field) => truck_text(plan, address, field),
        CalendarRow::CrusherLimit => crusher_text(plan, address),
        CalendarRow::PileMode | CalendarRow::GradeInput(..) => raw_cell_text(plan, address),
        row => match figures.figure(address, destination_kind(destinations, address)) {
            Some((value, true)) => format!("{} *", format_figure(row, value)),
            Some((value, false)) => format_figure(row, value),
            None => String::new(),
        },
    }
}

fn hover_text(plan: &SchedulePlan, destinations: &[DestinationRow], agent: Option<&LoaderAgent>, figures: &Figures<'_>, address: CalendarCellAddress) -> Option<String> {
    match address.row {
        CalendarRow::Input(_) => agent.map(|agent| resolved_hover(plan, agent, address)),
        CalendarRow::Truck(field) => Some(truck_hover(plan, address, field)),
        CalendarRow::CrusherLimit => Some(crusher_hover(plan, address)),
        CalendarRow::PileMode => {
            let operation = plan.stockpile_operation(address.destination()?);
            Some(match address.cell {
                CalendarCell::Default => tr!("pile-mode-default-hover"),
                CalendarCell::Period(day) => {
                    let source = if operation.periods.contains_key(&day) {
                        tr!("schedule-calendar-explicit")
                    } else {
                        tr!("destination-calendar-inherited")
                    };
                    tr!("pile-mode-day-hover", mode = operation.mode_at(day).label(), source = source)
                }
            })
        }
        CalendarRow::GradeInput(field, input) => {
            let destination = address.destination()?;
            let calendar = plan.crusher_grade_calendar(destination, field);
            let values = match address.cell {
                CalendarCell::Default => calendar.defaults.clone(),
                CalendarCell::Period(day) => calendar.resolved(day),
            };
            let value = values.get(input).map(trimmed_number).unwrap_or_else(|| tr!("grade-calendar-none"));
            Some(tr!("grade-calendar-input-help", value = value))
        }
        // Penalty first, then the band it was priced against.
        CalendarRow::GradeActual(_) => {
            let (_, partial) = figures.figure(address, destination_kind(destinations, address))?;
            let mut lines = Vec::new();
            if let Some(v) = figures.target_result(address) {
                let spec = &v.specification;
                let limit = |limit: Option<f64>| limit.map_or_else(|| "—".to_owned(), trimmed_number);
                lines.push(tr!(
                    "grade-calendar-actual-penalty",
                    currency = figures.currency.clone(),
                    penalty = format_money(v.penalty),
                    rate = format_money(v.penalty / v.tonnes)
                ));
                lines.push(tr!(
                    "grade-calendar-actual-band",
                    lower = limit(spec.lower),
                    target = trimmed_number(spec.target),
                    upper = limit(spec.upper)
                ));
            }
            if partial {
                lines.push(tr!("schedule-calendar-tonnes-partial", hours = trimmed_number(figures.result?.periods.coverage_end_h())));
            }
            (!lines.is_empty()).then(|| lines.join("\n"))
        }
        row => {
            let kind = destination_kind(destinations, address);
            let (_, partial) = figures.figure(address, kind)?;
            let mut lines = Vec::new();
            if partial {
                let covered = figures.result.map_or(0.0, |result| result.periods.coverage_end_h());
                lines.push(tr!("schedule-calendar-tonnes-partial", hours = trimmed_number(covered)));
            }
            if row == CalendarRow::Cumulative
                && kind == Some(DestinationKind::Stockpile)
                && let Some(grades) = figures.closing_grades(address)
            {
                lines.push(grades);
            }
            (!lines.is_empty()).then(|| lines.join("\n"))
        }
    }
}

/// A truck cell's own text.
///
/// Blank on a period means *inherit*: the default, not yesterday. Every
/// default cell holds a figure, because a new class starts at zero trucks and
/// a hundred per cent of both percentages.
fn truck_text(plan: &SchedulePlan, address: CalendarCellAddress, field: TruckField) -> String {
    let Some(class) = address.truck().and_then(|id| plan.trucks().class(id)) else {
        return String::new();
    };
    match address.cell {
        CalendarCell::Default => match field {
            TruckField::Units => class.calendar.default_units.to_string(),
            TruckField::Availability => format_percentage(class.calendar.default_availability),
            TruckField::Utilisation => format_percentage(class.calendar.default_utilisation),
        },
        CalendarCell::Period(period) => match class.calendar.periods.get(&period) {
            None => String::new(),
            Some(held) => match field {
                TruckField::Units => held.units.map(|units| units.to_string()).unwrap_or_default(),
                TruckField::Availability => held.availability.map(format_percentage).unwrap_or_default(),
                TruckField::Utilisation => held.utilisation.map(format_percentage).unwrap_or_default(),
            },
        },
    }
}

/// The same value as plain digits: what an editor opens on and what the
/// clipboard carries. Percentages lose their sign, units their separators.
fn truck_raw_text(plan: &SchedulePlan, address: CalendarCellAddress, field: TruckField) -> String {
    truck_text(plan, address, field).trim_end_matches('%').to_owned()
}

fn truck_hover(plan: &SchedulePlan, address: CalendarCellAddress, field: TruckField) -> String {
    let Some(class) = address.truck().and_then(|id| plan.trucks().class(id)) else {
        return String::new();
    };
    let period = match address.cell {
        CalendarCell::Default => return tr!("destination-calendar-default-hover"),
        CalendarCell::Period(period) => period,
    };
    let fleet = class.calendar.values_at(period);
    let explicit = class.calendar.periods.get(&period).is_some_and(|held| match field {
        TruckField::Units => held.units.is_some(),
        TruckField::Availability => held.availability.is_some(),
        TruckField::Utilisation => held.utilisation.is_some(),
    });
    let value = match field {
        TruckField::Units => fleet.units.to_string(),
        TruckField::Availability => format_percentage(fleet.availability),
        TruckField::Utilisation => format_percentage(fleet.utilisation),
    };
    let source = if explicit {
        tr!("schedule-calendar-explicit")
    } else {
        tr!("destination-calendar-inherited")
    };
    tr!("schedule-calendar-resolved", value = value, source = source)
}

/// Percentages are stored as fractions and shown out of a hundred, exactly as
/// the loader rows show theirs.
fn format_percentage(value: f64) -> String {
    format!("{}%", trimmed_number(value * 100.0))
}

/// Parse a typed truck cell. Blank clears the cell - which returns a period to
/// inheritance and a default to what a new class starts at.
fn parse_truck(field: TruckField, text: &str) -> Result<Option<f64>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    match field {
        TruckField::Units => {
            let number = trimmed.replace(',', "");
            let parsed = number.parse::<f64>().map_err(|_| tr!("schedule-calendar-invalid-number"))?;
            // Refused rather than rounded: half a truck is a typo, and a
            // rounded one would quietly change the fleet.
            if !parsed.is_finite() || parsed < 0.0 || parsed.fract() != 0.0 {
                return Err(crate::model::schedule::ScheduleError::InvalidTruckUnits.message());
            }
            Ok(Some(parsed))
        }
        TruckField::Availability | TruckField::Utilisation => {
            let number = trimmed.strip_suffix('%').unwrap_or(trimmed).trim();
            let parsed = number.parse::<f64>().map_err(|_| tr!("schedule-calendar-invalid-number"))?;
            if !parsed.is_finite() || !(0.0..=100.0).contains(&parsed) {
                return Err(tr!("schedule-calendar-invalid-percentage"));
            }
            Ok(Some(parsed / 100.0))
        }
    }
}

/// A crusher cell's own text.
///
/// Blank means *inherit* on a period and *unlimited* on the default, which is
/// why an explicit unlimited period is spelled out: inheritance and "no limit
/// today, whatever the default says" have to stay distinguishable.
fn crusher_text(plan: &SchedulePlan, address: CalendarCellAddress) -> String {
    let Some(calendar) = crusher_calendar(plan, address) else { return String::new() };
    match address.cell {
        CalendarCell::Default => match calendar.default_tpd {
            Some(limit) => format_tonnes(limit),
            None => tr!("destination-unlimited"),
        },
        CalendarCell::Period(period) => match calendar.periods.get(&period) {
            None => String::new(),
            Some(CrusherOverride::Unlimited) => tr!("destination-unlimited"),
            Some(CrusherOverride::Limit(limit)) => format_tonnes(*limit),
        },
    }
}

fn crusher_hover(plan: &SchedulePlan, address: CalendarCellAddress) -> String {
    let Some(calendar) = crusher_calendar(plan, address) else {
        return tr!("destination-error-not-a-crusher");
    };
    let period = match address.cell {
        CalendarCell::Default => return tr!("destination-calendar-default-hover"),
        CalendarCell::Period(period) => period,
    };
    let resolved = match calendar.limit_at(period) {
        Some(limit) => tr!("schedule-tonnes-per-day", value = format_tonnes(limit)),
        None => tr!("destination-unlimited"),
    };
    let source = if calendar.periods.contains_key(&period) {
        tr!("schedule-calendar-explicit")
    } else {
        tr!("destination-calendar-inherited")
    };
    tr!("schedule-calendar-resolved", value = resolved, source = source)
}

/// The crusher calendar this cell edits, when the row belongs to one.
fn crusher_calendar(plan: &SchedulePlan, address: CalendarCellAddress) -> Option<&crate::model::schedule::CrusherCalendar> {
    let DestinationId::Standalone(id) = address.destination()? else { return None };
    plan.routing().crusher(id)
}

/// The crusher a crusher-limit edit is addressed to.
fn crusher_target(address: CalendarCellAddress) -> Option<StandaloneDestinationId> {
    match address.destination()? {
        DestinationId::Standalone(id) => Some(id),
        DestinationId::Solid(_) => None,
    }
}

fn cell_id(ui: &egui::Ui, address: CalendarCellAddress) -> egui::Id {
    ui.id().with(("calendar_cell", address.owner, address.row, address.cell))
}

fn editable(address: CalendarCellAddress) -> bool {
    match address.row {
        CalendarRow::Truck(_) => address.truck().is_some(),
        CalendarRow::CrusherLimit | CalendarRow::GradeInput(..) => crusher_target(address).is_some(),
        CalendarRow::PileMode => address.destination().is_some(),
        // Both default rates are the class's, shown here and edited in Setup.
        CalendarRow::Input(field) => !(address.cell == CalendarCell::Default && matches!(field, CalendarField::Rate | CalendarField::ReclaimRate)),
        // Calculated: selectable so it can be copied, and nothing more.
        CalendarRow::DigTonnes
        | CalendarRow::ReclaimTonnes
        | CalendarRow::TruckHours
        | CalendarRow::TruckCycle
        | CalendarRow::TruckTonneKm
        | CalendarRow::Received
        | CalendarRow::Reclaimed
        | CalendarRow::Cumulative
        | CalendarRow::GradeActual(_)
        | CalendarRow::Value => false,
    }
}

/// What this cell says in its own right, which is nothing at all for an
/// inherited one. The Default rate is read-only class data rather than an
/// authored value, so it has none either.
fn explicit_value(agent: &LoaderAgent, address: CalendarCellAddress) -> Option<f64> {
    let field = address.row.field()?;
    match address.cell {
        CalendarCell::Default => match field {
            CalendarField::Availability => Some(agent.calendar.default_availability),
            CalendarField::Utilisation => Some(agent.calendar.default_utilisation),
            CalendarField::Rate | CalendarField::ReclaimRate => None,
        },
        CalendarCell::Period(period) => agent.calendar.periods.get(&period).and_then(|value| match field {
            CalendarField::Availability => value.availability,
            CalendarField::Utilisation => value.utilisation,
            CalendarField::Rate => value.rate_tph,
            CalendarField::ReclaimRate => value.reclaim_rate_tph,
        }),
    }
}

fn explicit_text(agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    match (address.row.field(), explicit_value(agent, address)) {
        (Some(field), Some(value)) => format_value(field, value),
        _ => String::new(),
    }
}

/// The same value as plain digits: what an editor opens on and what the
/// clipboard carries. The grid paints separators and units, and neither is
/// something [`parse_value`] is obliged to read back, so a cell the user
/// copies or re-opens must not hand them its painted form.
fn raw_text(agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    match (address.row.field(), explicit_value(agent, address)) {
        (Some(field), Some(value)) => trimmed_number(scaled(field, value)),
        _ => String::new(),
    }
}

/// Plain digits for any cell: what an editor opens on and what the clipboard
/// carries, whichever half of the grid the cell is in.
fn raw_cell_text(plan: &SchedulePlan, address: CalendarCellAddress) -> String {
    match address.row {
        CalendarRow::CrusherLimit => match (crusher_calendar(plan, address), address.cell) {
            (Some(calendar), CalendarCell::Default) => calendar.default_tpd.map(trimmed_number).unwrap_or_else(|| tr!("destination-unlimited")),
            (Some(calendar), CalendarCell::Period(period)) => match calendar.periods.get(&period) {
                None => String::new(),
                Some(CrusherOverride::Unlimited) => tr!("destination-unlimited"),
                Some(CrusherOverride::Limit(limit)) => trimmed_number(*limit),
            },
            (None, _) => String::new(),
        },
        CalendarRow::GradeInput(field, input) => {
            let Some(destination) = address.destination() else { return String::new() };
            let calendar = plan.crusher_grade_calendar(destination, field);
            match address.cell {
                CalendarCell::Default => calendar.defaults.get(input).map(trimmed_number).unwrap_or_default(),
                CalendarCell::Period(day) => calendar
                    .periods
                    .get(&day)
                    .and_then(|row| row.get(input))
                    .map(|v| v.number().map(trimmed_number).unwrap_or_else(|| tr!("grade-calendar-none")))
                    .unwrap_or_default(),
            }
        }
        CalendarRow::PileMode => {
            let Some(destination) = address.destination() else { return String::new() };
            let operation = plan.stockpile_operation(destination);
            match address.cell {
                CalendarCell::Default => operation.default_mode.label(),
                CalendarCell::Period(day) => operation.periods.get(&day).map(|mode| mode.label()).unwrap_or_default(),
            }
        }
        CalendarRow::Truck(field) => truck_raw_text(plan, address, field),
        _ => address.agent().and_then(|id| plan.agent(id)).map(|agent| raw_text(agent, address)).unwrap_or_default(),
    }
}

/// Which crusher-calendar cell an address names.
fn crusher_cell(cell: CalendarCell) -> CrusherCell {
    match cell {
        CalendarCell::Default => CrusherCell::Default,
        CalendarCell::Period(period) => CrusherCell::Period(period),
    }
}

/// Parse a typed crusher budget.
///
/// Three answers, because a crusher cell has three states. An empty period cell
/// inherits the default; the unlimited word - or an infinity sign - says this
/// period has no limit whatever the default is; anything else is a figure. On the
/// default cell there is nothing to inherit, so empty and unlimited coincide -
/// which is exactly why an explicit unlimited has to exist for the periods.
fn parse_crusher(text: &str) -> Result<Option<CrusherOverride>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let unlimited = tr!("destination-unlimited");
    if trimmed.eq_ignore_ascii_case(unlimited.trim()) || trimmed == "∞" {
        return Ok(Some(CrusherOverride::Unlimited));
    }
    // Separators are painted, so a figure copied out of the grid and pasted back
    // has to read as the number it was.
    let number = trimmed.replace(',', "");
    let parsed = number.parse::<f64>().map_err(|_| tr!("schedule-calendar-invalid-number"))?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(crate::model::schedule::ScheduleError::InvalidCapacity.message());
    }
    Ok(Some(CrusherOverride::Limit(parsed)))
}

fn cell_text(plan: &SchedulePlan, agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    if address.cell == CalendarCell::Default
        && let Some(field @ (CalendarField::Rate | CalendarField::ReclaimRate)) = address.row.field()
    {
        return plan.class(agent.class_id).map(|class| format_value(field, class_rate(class, field))).unwrap_or_default();
    }
    explicit_text(agent, address)
}

/// Which of a class's two rates a row reads. The shared factors have none, and
/// fall back to the dig rate so a caller that asks anyway gets a figure rather
/// than a panic.
fn class_rate(class: &crate::model::schedule::LoaderClass, field: CalendarField) -> f64 {
    match field {
        CalendarField::ReclaimRate => class.default_reclaim_rate_tph,
        _ => class.default_dig_rate_tph,
    }
}

fn resolved_hover(plan: &SchedulePlan, agent: &LoaderAgent, address: CalendarCellAddress) -> String {
    let Some(field) = address.row.field() else { return String::new() };
    let Some(class) = plan.class(agent.class_id) else {
        return tr!("schedule-error-unknown-class");
    };
    if address.cell == CalendarCell::Default && matches!(field, CalendarField::Rate | CalendarField::ReclaimRate) {
        return tr!(
            "schedule-calendar-class-default",
            value = format_value(field, class_rate(class, field)),
            class = class.name.clone()
        );
    }
    let period = match address.cell {
        CalendarCell::Default => CalendarPeriod(0),
        CalendarCell::Period(period) => period,
    };
    let kind = if field == CalendarField::ReclaimRate {
        crate::model::schedule::RateKind::Reclaim
    } else {
        crate::model::schedule::RateKind::Dig
    };
    let Ok(values) = agent.calendar.rate_values_at(period, kind, class_rate(class, field)) else {
        return tr!("schedule-calendar-invalid");
    };
    let (value, source) = match field {
        CalendarField::Availability => (
            values.availability,
            if explicit_value(agent, address).is_none() {
                tr!("schedule-calendar-loader-default")
            } else {
                tr!("schedule-calendar-explicit")
            },
        ),
        CalendarField::Utilisation => (
            values.utilisation,
            if explicit_value(agent, address).is_none() {
                tr!("schedule-calendar-loader-default")
            } else {
                tr!("schedule-calendar-explicit")
            },
        ),
        CalendarField::Rate | CalendarField::ReclaimRate => (
            values.rate_tph,
            if explicit_value(agent, address).is_none() {
                tr!("schedule-calendar-class-source")
            } else {
                tr!("schedule-calendar-explicit")
            },
        ),
    };
    tr!("schedule-calendar-resolved", value = format_value(field, value), source = source)
}

/// Percentages are stored as fractions and shown out of a hundred.
fn scaled(field: CalendarField, value: f64) -> f64 {
    match field {
        CalendarField::Rate | CalendarField::ReclaimRate => value,
        CalendarField::Availability | CalendarField::Utilisation => value * 100.0,
    }
}

fn trimmed_number(value: f64) -> String {
    let mut text = format!("{value:.3}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

/// Tonnes to at most one decimal. Presentation only: the aggregation keeps
/// full precision, and this rounding never feeds back into it.
fn tonnes_number(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    // A balance that cancelled to a hair below zero, and negative zero itself,
    // both read as nothing received - never as "-0", which looks like a
    // measurement rather than the rounding it is.
    let rounded = if rounded == 0.0 { 0.0 } else { rounded };
    if rounded.fract() == 0.0 { format!("{rounded:.0}") } else { format!("{rounded:.1}") }
}

pub(crate) fn format_tonnes(value: f64) -> String {
    tonnes_number(value).separate_with_commas()
}

fn format_hours(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    let rounded = if rounded == 0.0 { 0.0 } else { rounded };
    format!("{rounded:.1}").separate_with_commas()
}

/// A movement value to the cent, with separators. Signed: a cost reads
/// negative. Never described as profit.
pub(crate) fn format_money(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let rounded = if rounded == 0.0 { 0.0 } else { rounded };
    format!("{rounded:.2}").separate_with_commas()
}

fn format_value(field: CalendarField, value: f64) -> String {
    let text = trimmed_number(scaled(field, value)).separate_with_commas();
    match field {
        CalendarField::Rate | CalendarField::ReclaimRate => format!("{text} t/h"),
        CalendarField::Availability | CalendarField::Utilisation => format!("{text}%"),
    }
}

fn parse_value(field: CalendarField, text: &str) -> Result<Option<f64>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let number = match field {
        CalendarField::Availability | CalendarField::Utilisation => trimmed.strip_suffix('%').unwrap_or(trimmed).trim(),
        CalendarField::Rate | CalendarField::ReclaimRate => trimmed,
    };
    let parsed = number.parse::<f64>().map_err(|_| tr!("schedule-calendar-invalid-number"))?;
    match field {
        CalendarField::Availability | CalendarField::Utilisation if parsed.is_finite() && (0.0..=100.0).contains(&parsed) => Ok(Some(parsed / 100.0)),
        CalendarField::Rate | CalendarField::ReclaimRate if parsed.is_finite() && parsed > 0.0 => Ok(Some(parsed)),
        CalendarField::Availability | CalendarField::Utilisation => Err(tr!("schedule-calendar-invalid-percentage")),
        CalendarField::Rate | CalendarField::ReclaimRate => Err(tr!("schedule-error-invalid-rate")),
    }
}

fn begin_edit(editor: &mut EditorState, plan: &SchedulePlan, address: CalendarCellAddress, typed: Option<String>) {
    if !editable(address) {
        return;
    }
    // A mode is chosen from a list; typing one still works.
    if address.row == CalendarRow::PileMode && typed.is_none() {
        editor.schedule_calendar.mode_menu = Some(address);
        return;
    }
    let text = typed.unwrap_or_else(|| raw_cell_text(plan, address));
    editor.schedule_calendar.draft = Some(CalendarCellDraft {
        address,
        text,
        error: None,
        request_focus: true,
    });
}

/// A typed or pasted mode: blank returns a day to the Default.
fn parse_pile_mode(text: &str) -> Result<Option<PileMode>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    PileMode::parse(text).map(Some).ok_or_else(|| tr!("pile-mode-invalid"))
}

fn parse_grade_cell(_input: GradeTargetInput, text: &str) -> Result<Option<GradeTargetValue>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if text.eq_ignore_ascii_case(&tr!("grade-calendar-none")) || text == "—" {
        return Ok(Some(GradeTargetValue::Clear));
    }
    let value = text
        .trim_end_matches('%')
        .replace(',', "")
        .parse::<f64>()
        .map_err(|_| tr!("schedule-calendar-invalid-number"))?;
    if !value.is_finite() || value < 0.0 {
        return Err(tr!("grade-calendar-invalid"));
    }
    Ok(Some(GradeTargetValue::Number(value)))
}

fn validate_grade_edits(plan: &SchedulePlan, edits: &[GradeTargetCellEdit]) -> Result<(), String> {
    let mut trial = plan.clone();
    trial.set_grade_target_cells(edits).map_err(|e| e.message())
}

fn commit_draft(editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) -> bool {
    let Some(draft) = editor.schedule_calendar.draft.as_mut() else { return true };
    if let CalendarRow::GradeInput(field, input) = draft.address.row {
        let Some(destination) = draft.address.destination() else { return true };
        let result = parse_grade_cell(input, &draft.text).and_then(|value| {
            let edits = vec![GradeTargetCellEdit {
                destination,
                field,
                cell: draft.address.cell,
                input,
                value,
            }];
            validate_grade_edits(plan, &edits)?;
            Ok(edits)
        });
        match result {
            Ok(edits) => {
                commands.push(UiCommand::schedule(session, ScheduleEdit::SetGradeTargetCells { edits }));
                editor.schedule_calendar.draft = None;
                editor.schedule_calendar.error = None;
                return true;
            }
            Err(error) => {
                draft.error = Some(error.clone());
                draft.request_focus = true;
                editor.schedule_calendar.error = Some(error);
                return false;
            }
        }
    }
    // A crusher budget is the one destination input, and it commits through its
    // own edit: the loader calendar and the crusher calendar are different
    // things with different cells.
    if draft.address.row == CalendarRow::PileMode {
        let Some(destination) = draft.address.destination() else {
            editor.schedule_calendar.draft = None;
            return true;
        };
        match parse_pile_mode(&draft.text) {
            Ok(mode) => {
                commands.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::SetPileModeCells {
                        edits: vec![PileModeCellEdit {
                            destination,
                            cell: draft.address.cell,
                            mode,
                        }],
                    },
                ));
                editor.schedule_calendar.draft = None;
                editor.schedule_calendar.error = None;
                return true;
            }
            Err(error) => {
                editor.schedule_calendar.error = Some(error.clone());
                draft.error = Some(error);
                draft.request_focus = true;
                return false;
            }
        }
    }
    if draft.address.row == CalendarRow::CrusherLimit {
        let Some(destination) = crusher_target(draft.address) else {
            editor.schedule_calendar.draft = None;
            return true;
        };
        match parse_crusher(&draft.text) {
            Ok(value) => {
                commands.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::SetCrusherCells {
                        edits: vec![CrusherCellEdit {
                            destination,
                            cell: crusher_cell(draft.address.cell),
                            value,
                        }],
                    },
                ));
                editor.schedule_calendar.draft = None;
                editor.schedule_calendar.error = None;
                return true;
            }
            Err(error) => {
                draft.error = Some(error.clone());
                draft.request_focus = true;
                editor.schedule_calendar.error = Some(error);
                return false;
            }
        }
    }
    // A truck cell commits through its own edit, for the same reason: a truck
    // calendar and a loader calendar are different things with different cells.
    if let Some(field) = draft.address.row.truck_field() {
        let Some(class) = draft.address.truck() else {
            editor.schedule_calendar.draft = None;
            return true;
        };
        match parse_truck(field, &draft.text) {
            Ok(value) => {
                commands.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::SetTruckCells {
                        edits: vec![TruckCellEdit {
                            class,
                            cell: draft.address.cell,
                            field,
                            value,
                        }],
                    },
                ));
                editor.schedule_calendar.draft = None;
                editor.schedule_calendar.error = None;
                return true;
            }
            Err(error) => {
                draft.error = Some(error.clone());
                draft.request_focus = true;
                editor.schedule_calendar.error = Some(error);
                return false;
            }
        }
    }
    // A draft only ever opens on an editable cell, so this field is always
    // there; a calculated row has none and can reach no further than here.
    let (Some(field), Some(agent)) = (draft.address.row.field(), draft.address.agent()) else {
        editor.schedule_calendar.draft = None;
        return true;
    };
    match parse_value(field, &draft.text) {
        Ok(value) => {
            commands.push(UiCommand::schedule(
                session,
                ScheduleEdit::SetCalendarCells {
                    edits: vec![CalendarCellEdit {
                        agent,
                        cell: draft.address.cell,
                        field,
                        value,
                    }],
                },
            ));
            editor.schedule_calendar.draft = None;
            editor.schedule_calendar.error = None;
            true
        }
        Err(error) => {
            draft.error = Some(error.clone());
            // Keep the caret in the cell the user has to correct: the key that
            // asked to commit has already surrendered focus, and a draft
            // nothing can type into would swallow the next Escape too.
            draft.request_focus = true;
            editor.schedule_calendar.error = Some(error);
            false
        }
    }
}

#[derive(Clone, Copy)]
enum Nav {
    Left,
    Right,
    Up,
    Down,
}

/// Every row a loader group shows, in drawn order - the calculated row
/// included. Row numbers are what a pasted rectangle is measured against, so
/// leaving it out here would land pasted values on the wrong loader.
fn grid_rows(plan: &SchedulePlan, destinations: &[DestinationRow], editor: &EditorState) -> Vec<(CalendarOwner, CalendarRow)> {
    let mut rows = vec![(CalendarOwner::Schedule, CalendarRow::Value)];
    for agent in plan.agents() {
        let owner = CalendarOwner::Loader(agent.id);
        if editor.schedule_calendar.collapsed.contains(&owner) {
            continue;
        }
        rows.extend(LOADER_ROWS.iter().map(|row| (owner, *row)));
    }
    for class in &plan.trucks().classes {
        let owner = CalendarOwner::Truck(class.id);
        if editor.schedule_calendar.collapsed.contains(&owner) {
            continue;
        }
        rows.extend(TRUCK_ROWS.iter().map(|row| (owner, *row)));
    }
    for destination in destinations {
        let owner = CalendarOwner::Destination(destination.id);
        if editor.schedule_calendar.collapsed.contains(&owner) {
            continue;
        }
        rows.extend(destination_row_kinds(destination.kind).iter().map(|row| (owner, *row)));
        rows.extend(destination_grade_rows(plan, destination, editor).into_iter().map(|row| (owner, row)));
    }
    rows
}

fn address_index(editor: &EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], address: CalendarCellAddress) -> Option<(usize, u32)> {
    let row = grid_rows(plan, destinations, editor)
        .into_iter()
        .position(|(owner, row)| owner == address.owner && row == address.row)?;
    let column = match address.cell {
        CalendarCell::Default => 0,
        CalendarCell::Period(period) => period.0.saturating_add(1),
    };
    Some((row, column))
}

fn address_at(editor: &EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], row: usize, column: u32) -> Option<CalendarCellAddress> {
    let (owner, kind) = grid_rows(plan, destinations, editor).into_iter().nth(row)?;
    let cell = if column == 0 {
        CalendarCell::Default
    } else {
        CalendarCell::Period(CalendarPeriod(column - 1))
    };
    Some(CalendarCellAddress { owner, row: kind, cell })
}

fn selection_bounds(editor: &EditorState, plan: &SchedulePlan, destinations: &[DestinationRow]) -> Option<(usize, usize, u32, u32)> {
    let selection = editor.schedule_calendar.selection?;
    let (ar, ac) = address_index(editor, plan, destinations, selection.anchor)?;
    let (fr, fc) = address_index(editor, plan, destinations, selection.focus)?;
    Some((ar.min(fr), ar.max(fr), ac.min(fc), ac.max(fc)))
}

fn selected(editor: &EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], address: CalendarCellAddress) -> bool {
    let Some((r0, r1, c0, c1)) = selection_bounds(editor, plan, destinations) else {
        return false;
    };
    address_index(editor, plan, destinations, address).is_some_and(|(row, col)| (r0..=r1).contains(&row) && (c0..=c1).contains(&col))
}

fn move_selection(editor: &mut EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], nav: Nav) {
    let Some(selection) = editor.schedule_calendar.selection else { return };
    let Some((mut row, mut column)) = address_index(editor, plan, destinations, selection.focus) else {
        return;
    };
    let row_count = grid_rows(plan, destinations, editor).len();
    if row_count == 0 {
        return;
    }
    loop {
        let from = (row, column);
        match nav {
            Nav::Left => column = column.saturating_sub(1),
            Nav::Right => column = column.saturating_add(1).min(editor.schedule_calendar.visible_days),
            Nav::Up => row = row.saturating_sub(1),
            Nav::Down => row = (row + 1).min(row_count - 1),
        }
        // Clamped against the edge of the grid. Without this the search for the
        // next editable cell would never end when the last row is a calculated
        // one, which it always is.
        if (row, column) == from {
            return;
        }
        let Some(address) = address_at(editor, plan, destinations, row, column) else { return };
        if editable(address) {
            editor.schedule_calendar.selection = Some(CalendarSelection { anchor: address, focus: address });
            return;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_keyboard(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    destinations: &[DestinationRow],
    figures: &Figures<'_>,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    // Cells are ordinary focusable widgets, so a Tab that moves along the row
    // also lands egui's focus on the cell the selection moved to. That focus is
    // this grid's own; only focus somewhere else - the jump field, a button -
    // means the keys belong to something other than the selection.
    let selected_cell = editor.schedule_calendar.selection.map(|selection| cell_id(ui, selection.focus));
    let focused = ui.memory(|memory| memory.focused());
    if editor.schedule_calendar.draft.is_some() || (focused.is_some() && focused != selected_cell) {
        return;
    }
    let events = ui.input(|input| input.events.clone());
    for event in events {
        match event {
            egui::Event::Text(text) if !text.chars().all(char::is_whitespace) => {
                if let Some(selection) = editor.schedule_calendar.selection {
                    begin_edit(editor, plan, selection.focus, Some(text));
                }
            }
            egui::Event::Paste(text) => paste(editor, plan, destinations, session, commands, &text),
            egui::Event::Copy => copy(editor, plan, destinations, figures, ui),
            egui::Event::Key {
                key, pressed: true, modifiers, ..
            } => match key {
                egui::Key::Enter => {
                    if let Some(selection) = editor.schedule_calendar.selection {
                        begin_edit(editor, plan, selection.focus, None);
                    }
                }
                egui::Key::ArrowLeft => move_selection(editor, plan, destinations, Nav::Left),
                egui::Key::ArrowRight => move_selection(editor, plan, destinations, Nav::Right),
                egui::Key::ArrowUp => move_selection(editor, plan, destinations, Nav::Up),
                egui::Key::ArrowDown => move_selection(editor, plan, destinations, Nav::Down),
                egui::Key::Backspace | egui::Key::Delete if !modifiers.command => clear_selection(editor, plan, destinations, session, commands),
                _ => {}
            },
            _ => {}
        }
    }
}

fn clear_selection(editor: &mut EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], session: u32, commands: &mut Vec<UiCommand>) {
    let Some((r0, r1, c0, c1)) = selection_bounds(editor, plan, destinations) else { return };
    let mut edits = Vec::new();
    let mut crusher_edits = Vec::new();
    let mut truck_edits = Vec::new();
    let mut grade_edits = Vec::new();
    let mut mode_edits = Vec::new();
    for row in r0..=r1 {
        for column in c0..=c1 {
            let Some(address) = address_at(editor, plan, destinations, row, column) else { continue };
            if address.row.is_calculated() {
                editor.schedule_calendar.error = Some(tr!("schedule-calendar-calculated-selection"));
                return;
            }
            if address.row == CalendarRow::PileMode {
                if let Some(destination) = address.destination() {
                    mode_edits.push(PileModeCellEdit {
                        destination,
                        cell: address.cell,
                        mode: None,
                    });
                }
                continue;
            }
            if let CalendarRow::GradeInput(field, input) = address.row {
                if let Some(destination) = address.destination() {
                    grade_edits.push(GradeTargetCellEdit {
                        destination,
                        field,
                        cell: address.cell,
                        input,
                        value: None,
                    });
                }
                continue;
            }
            if let Some(field) = address.row.truck_field() {
                if let Some(class) = address.truck() {
                    truck_edits.push(TruckCellEdit {
                        class,
                        cell: address.cell,
                        field,
                        value: None,
                    });
                }
                continue;
            }
            if address.row == CalendarRow::CrusherLimit {
                if let Some(destination) = crusher_target(address) {
                    crusher_edits.push(CrusherCellEdit {
                        destination,
                        cell: crusher_cell(address.cell),
                        value: None,
                    });
                }
                continue;
            }
            let (Some(field), Some(agent)) = (address.row.field().filter(|_| editable(address)), address.agent()) else {
                continue;
            };
            edits.push(CalendarCellEdit {
                agent,
                cell: address.cell,
                field,
                value: None,
            });
        }
    }
    if !grade_edits.is_empty() {
        if let Err(error) = validate_grade_edits(plan, &grade_edits) {
            editor.schedule_calendar.error = Some(error);
            return;
        }
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetGradeTargetCells { edits: grade_edits }));
    }
    editor.schedule_calendar.error = None;
    if !edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetCalendarCells { edits }));
    }
    if !crusher_edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetCrusherCells { edits: crusher_edits }));
    }
    if !truck_edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetTruckCells { edits: truck_edits }));
    }
    if !mode_edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetPileModeCells { edits: mode_edits }));
    }
}

fn paste(editor: &mut EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], session: u32, commands: &mut Vec<UiCommand>, text: &str) {
    let Some(selection) = editor.schedule_calendar.selection else { return };
    let Some((start_row, start_column)) = address_index(editor, plan, destinations, selection.focus) else {
        return;
    };
    let text = text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).unwrap_or(text);
    let rows: Vec<Vec<&str>> = text.lines().map(|line| line.trim_end_matches('\r').split('\t').collect()).collect();
    let mut edits = Vec::new();
    let mut crusher_edits = Vec::new();
    let mut truck_edits = Vec::new();
    let mut grade_edits = Vec::new();
    let mut mode_edits = Vec::new();
    for (row_offset, values) in rows.iter().enumerate() {
        for (column_offset, text) in values.iter().enumerate() {
            let Some(address) = address_at(editor, plan, destinations, start_row + row_offset, start_column.saturating_add(column_offset as u32)) else {
                editor.schedule_calendar.error = Some(tr!("schedule-calendar-paste-outside"));
                return;
            };
            if address.row.is_calculated() {
                editor.schedule_calendar.error = Some(tr!("schedule-calendar-calculated-selection"));
                return;
            }
            if address.row == CalendarRow::PileMode {
                let Some(destination) = address.destination() else { return };
                match parse_pile_mode(text) {
                    Ok(mode) => mode_edits.push(PileModeCellEdit {
                        destination,
                        cell: address.cell,
                        mode,
                    }),
                    Err(error) => {
                        editor.schedule_calendar.error = Some(error);
                        return;
                    }
                }
                continue;
            }
            if let CalendarRow::GradeInput(field, input) = address.row {
                let Some(destination) = address.destination() else { return };
                match parse_grade_cell(input, text) {
                    Ok(value) => grade_edits.push(GradeTargetCellEdit {
                        destination,
                        field,
                        cell: address.cell,
                        input,
                        value,
                    }),
                    Err(error) => {
                        editor.schedule_calendar.error = Some(error);
                        return;
                    }
                }
                continue;
            }
            if let Some(field) = address.row.truck_field() {
                let Some(class) = address.truck() else {
                    editor.schedule_calendar.error = Some(tr!("schedule-calendar-paste-read-only"));
                    return;
                };
                match parse_truck(field, text) {
                    Ok(value) => truck_edits.push(TruckCellEdit {
                        class,
                        cell: address.cell,
                        field,
                        value,
                    }),
                    Err(error) => {
                        editor.schedule_calendar.error = Some(error);
                        return;
                    }
                }
                continue;
            }
            if address.row == CalendarRow::CrusherLimit {
                let Some(destination) = crusher_target(address) else {
                    editor.schedule_calendar.error = Some(tr!("schedule-calendar-paste-read-only"));
                    return;
                };
                match parse_crusher(text) {
                    Ok(value) => crusher_edits.push(CrusherCellEdit {
                        destination,
                        cell: crusher_cell(address.cell),
                        value,
                    }),
                    Err(error) => {
                        editor.schedule_calendar.error = Some(error);
                        return;
                    }
                }
                continue;
            }
            let (Some(field), Some(agent)) = (address.row.field().filter(|_| editable(address)), address.agent()) else {
                editor.schedule_calendar.error = Some(tr!("schedule-calendar-paste-read-only"));
                return;
            };
            let value = match parse_value(field, text) {
                Ok(value) => value,
                Err(error) => {
                    editor.schedule_calendar.error = Some(error);
                    return;
                }
            };
            edits.push(CalendarCellEdit {
                agent,
                cell: address.cell,
                field,
                value,
            });
        }
    }
    if !grade_edits.is_empty() {
        if let Err(error) = validate_grade_edits(plan, &grade_edits) {
            editor.schedule_calendar.error = Some(error);
            return;
        }
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetGradeTargetCells { edits: grade_edits }));
    }
    editor.schedule_calendar.error = None;
    if !edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetCalendarCells { edits }));
    }
    if !crusher_edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetCrusherCells { edits: crusher_edits }));
    }
    if !truck_edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetTruckCells { edits: truck_edits }));
    }
    if !mode_edits.is_empty() {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetPileModeCells { edits: mode_edits }));
    }
}

fn copy(editor: &EditorState, plan: &SchedulePlan, destinations: &[DestinationRow], figures: &Figures<'_>, ui: &egui::Ui) {
    let Some((r0, r1, c0, c1)) = selection_bounds(editor, plan, destinations) else { return };
    let mut lines = Vec::new();
    for row in r0..=r1 {
        let mut cells = Vec::new();
        for column in c0..=c1 {
            // Plain digits throughout, calculated cells included: separators
            // and the partial-period mark are things the grid paints, not
            // things a clipboard should carry.
            let text = match address_at(editor, plan, destinations, row, column) {
                Some(address) if address.row.is_calculated() => figures
                    .figure(address, destination_kind(destinations, address))
                    .map(|(value, _)| raw_figure(address.row, value))
                    .unwrap_or_default(),
                Some(address) => raw_cell_text(plan, address),
                None => String::new(),
            };
            cells.push(text);
        }
        lines.push(cells.join("\t"));
    }
    ui.output_mut(|output| output.commands.push(egui::OutputCommand::CopyText(lines.join("\n"))));
}
