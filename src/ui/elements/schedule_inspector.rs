//! The Inspector: the whole operation at the time slider's instant, beside
//! the Gantt.
//!
//! One line per machine, stockpile, crusher, dump and truck class, each with
//! the one figure that answers "what is it doing now" and a weak second line
//! saying why or where. Everything else - grades, bands, the span a figure
//! came from - is on hover, so the panel reads at a glance and the slider can
//! be dragged with it open.
//!
//! It reads only the published schedule and the plan; nothing here asks the
//! solver anything, and nothing is drawn for an instant the calculation does
//! not cover.

use crate::{
    i18n::tr,
    model::{
        ReserveFieldId,
        schedule::{
            BarWork, CalendarPeriod, DestinationKind, SchedulePlan,
            cashflow::Activity,
            destinations::DestinationView,
            experiment::StockpileRepresentation,
            result::{CalculatedSchedule, IdleReason, WorkSource},
            stockpile_operation::PileMode,
        },
    },
    ui::{
        EditorState,
        elements::{
            schedule_calendar::format_tonnes,
            schedule_gantt::{IDLE_COLOR, RECLAIM_COLOR, WORKING_COLOR, destination_label, execution_tooltip, idle_reason_text, instant_label, qualified_source_label},
        },
        fonts::bold,
        state::GanttView,
    },
};

/// Width the Inspector takes from the Gantt while open.
pub(crate) const INSPECTOR_WIDTH: f32 = 300.0;
/// The Gantt keeps at least this much timeline beside it; narrower and the
/// Inspector is not drawn, rather than squeezing the timeline to nothing.
pub(crate) const MIN_TIMELINE_WIDTH: f32 = 420.0;

const NAME_ROW_H: f32 = 20.0;
const DETAIL_ROW_H: f32 = 16.0;
const METER_H: f32 = 3.0;
const DOT_R: f32 = 4.0;
const INDENT: f32 = 16.0;

/// One line of the Inspector.
#[derive(Default)]
struct Entry {
    dot: Option<egui::Color32>,
    name: String,
    value: String,
    value_weak: bool,
    detail: Option<String>,
    /// A second line under the detail, in the warning colour.
    warning: Option<String>,
    /// How full, `0..=1`, drawn as a thin bar under the name.
    meter: Option<f32>,
}

pub(crate) fn draw_inspector(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    destinations: &[DestinationView],
    fields: &[(ReserveFieldId, String)],
) {
    let visuals = ui.visuals().clone();
    let (surface, _) = crate::ui::widgets::tree_row_colors(ui);
    ui.painter().rect_filled(rect, 0.0, surface);
    ui.painter().line_segment([rect.left_top(), rect.left_bottom()], visuals.widgets.noninteractive.bg_stroke);

    let inner = rect.shrink2(egui::vec2(12.0, 8.0));
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("gantt_inspector").max_rect(inner));
    child.set_clip_rect(rect);
    let ui = &mut child;

    let hour = editor.schedule_time_h.max(0.0);
    ui.horizontal(|ui| {
        ui.label(bold(&instant_label(hour * GanttView::HOUR)).size(16.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if ui.button("›").on_hover_text(tr!("inspector-next-hour")).clicked() {
                editor.schedule_time_h = editor.schedule_time_h.floor() + 1.0;
            }
            if ui.button("‹").on_hover_text(tr!("inspector-previous-hour")).clicked() {
                editor.schedule_time_h = (editor.schedule_time_h.ceil() - 1.0).max(0.0);
            }
        });
    });
    let hour = editor.schedule_time_h.max(0.0);

    let Some(schedule) = editor.schedule_result.clone() else {
        note(ui, tr!("inspector-no-schedule"));
        return;
    };
    if hour >= schedule.requested_end_h {
        note(ui, tr!("inspector-beyond", end = instant_label(schedule.requested_end_h * GanttView::HOUR)));
        return;
    }
    let grade_names: Vec<String> = schedule
        .grades
        .iter()
        .map(|(field, _)| fields.iter().find(|(id, _)| id == field).map_or_else(|| field.0.to_string(), |(_, name)| name.clone()))
        .collect();
    let day = CalendarPeriod((hour / crate::model::schedule::SCHEDULE_PERIOD_H).floor() as u32);

    egui::ScrollArea::vertical().id_salt("gantt_inspector_scroll").auto_shrink([false, false]).show(ui, |ui| {
        loaders(ui, editor, plan, destinations, &schedule, hour);
        stockpiles(ui, plan, destinations, &schedule, &grade_names, hour, day);
        crushers(ui, plan, destinations, &schedule, &grade_names, hour, day);
        dumps(ui, destinations, &schedule, hour);
        trucks(ui, plan, &schedule, hour, day);
    });
}

fn note(ui: &mut egui::Ui, text: String) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(text).weak());
}

fn section(ui: &mut egui::Ui, title: String) {
    ui.add_space(10.0);
    ui.label(egui::RichText::new(title).small().weak());
    ui.add_space(2.0);
}

fn loaders(ui: &mut egui::Ui, editor: &EditorState, plan: &SchedulePlan, destinations: &[DestinationView], schedule: &CalculatedSchedule, hour: f64) {
    if plan.agents().is_empty() {
        return;
    }
    section(ui, tr!("inspector-loaders"));
    for agent in plan.agents() {
        let mut entry = Entry {
            name: agent.name.clone(),
            ..Entry::default()
        };
        let mut hover: Option<String> = None;
        if let Some(execution) = schedule.executions_at(hour).find(|execution| execution.agent == agent.id) {
            let bar = plan.bar(execution.bar);
            let report = editor.schedule_bar_reports.iter().find(|report| report.bar == execution.bar);
            entry.dot = Some(match execution.activity {
                Activity::Dig => WORKING_COLOR,
                Activity::Reclaim => RECLAIM_COLOR,
            });
            entry.value = tr!(
                "inspector-rate",
                rate = format_tonnes(schedule.deliveries_at(hour).filter(|d| d.agent == agent.id).map(rate).sum::<f64>().round())
            );
            let source = match bar {
                Some(bar) => qualified_source_label(execution.source, bar, report, schedule, destinations),
                None => String::new(),
            };
            let mut targets: Vec<String> = Vec::new();
            for delivery in schedule.deliveries_at(hour).filter(|delivery| delivery.agent == agent.id) {
                let name = destination_label(delivery.destination, destinations);
                if !targets.contains(&name) {
                    targets.push(name);
                }
            }
            entry.detail = Some(if targets.is_empty() { source } else { format!("{source} → {}", targets.join(", ")) });
            if let Some(bar) = bar {
                hover = Some(execution_tooltip(execution, bar, report, schedule, plan, destinations));
            }
        } else if let Some(span) = schedule.idle_at(agent.id, hour) {
            if span.reason == Some(IdleReason::Delayed) {
                let (color, kind) = delay_at(plan, agent.id, hour).unwrap_or((IDLE_COLOR, None));
                entry.dot = Some(color);
                entry.value = match kind {
                    Some(kind) => tr!("inspector-delay-of", kind = kind),
                    None => tr!("inspector-delay"),
                };
            } else {
                let (name, note) = idle_reason_text(span.reason);
                entry.dot = Some(IDLE_COLOR);
                entry.value = sentence_case(&name);
                if !span.full.is_empty() {
                    let names: Vec<String> = span.full.iter().map(|id| destination_label(*id, destinations)).collect();
                    entry.detail = Some(if span.reason == Some(IdleReason::PileMode) {
                        tr!("idle-pile-list", destinations = names.join(", "))
                    } else {
                        tr!("idle-full-list", destinations = names.join(", "))
                    });
                }
                let mut lines = vec![tr!(
                    "schedule-span-hours",
                    from = instant_label(span.start_h * GanttView::HOUR),
                    to = instant_label(span.end_h * GanttView::HOUR),
                    hours = format!("{:.1}", span.end_h - span.start_h)
                )];
                if !note.is_empty() {
                    lines.push(note);
                }
                hover = Some(lines.join("\n"));
            }
        } else {
            // A machine with no bars never reached the calculation.
            let (name, _) = idle_reason_text(Some(IdleReason::NoWork));
            entry.dot = Some(IDLE_COLOR);
            entry.value = sentence_case(&name);
        }
        if let Some(execution) = schedule.executions_at(hour).find(|e| e.agent == agent.id) {
            let mut matching = 0.0;
            let nominal = plan.agent(agent.id).and_then(|a| plan.class(a.class_id)).map_or(0.0, |c| {
                if execution.activity == Activity::Dig {
                    c.default_dig_rate_tph
                } else {
                    c.default_reclaim_rate_tph
                }
            });
            let mut tonnes_rate = 0.0;
            for d in schedule.deliveries_at(hour).filter(|d| d.agent == agent.id) {
                let rate = d.tonnes / (d.end_h - d.start_h).max(1e-9);
                tonnes_rate += rate;
                matching += rate * d.truck_hours / d.tonnes.max(1e-9);
            }
            if tonnes_rate > 0.0 {
                let line = format!("{}: {:.1}", tr!("haul-match"), nominal * matching / tonnes_rate);
                hover = Some(hover.map_or_else(|| line.clone(), |text| format!("{text}\n{line}")));
            }
        }
        if schedule.idle_at(agent.id, hour).is_some_and(|s| s.reason == Some(IdleReason::NoTrucks)) {
            let day = (hour / crate::model::schedule::SCHEDULE_PERIOD_H).floor().max(0.0) as u32;
            let lines: Vec<_> = plan
                .trucks()
                .classes
                .iter()
                .filter_map(|c| {
                    let h = schedule.periods.truck_haul(c.id, day)?;
                    (h.tonnes > 0.0).then(|| format!("{} · {}", c.name, tr!("haul-cycle-minutes", minutes = format!("{:.1}", h.cycle_minutes()))))
                })
                .collect();
            if !lines.is_empty() {
                let detail = lines.join("\n");
                hover = Some(hover.map_or_else(|| detail.clone(), |text| format!("{text}\n{detail}")));
            }
        }
        let response = draw_entry(ui, &entry);
        if let Some(text) = hover {
            response.on_hover_text(text);
        }
    }
}

/// What delay holds a machine at `hour`: a calendar delay first, then a
/// delay bar. Its type's colour, and its type's name when it has one.
fn delay_at(plan: &SchedulePlan, agent: crate::model::schedule::LoaderAgentId, hour: f64) -> Option<(egui::Color32, Option<String>)> {
    let kind = plan
        .delays()
        .spans_for(agent, hour + 1.0)
        .into_iter()
        .find(|span| span.start_h <= hour && hour < span.end_h)
        .map(|span| span.kind)
        .or_else(|| {
            plan.bars().iter().find_map(|bar| match bar.work {
                BarWork::Delay(work) if bar.agent == Some(agent) && bar.window.start_h <= hour && bar.window.end_h.is_none_or(|end| hour < end) => Some(work.kind),
                _ => None,
            })
        })?;
    let (color, name) = super::schedule_delays::delay_look(plan, kind);
    Some((color, kind.and_then(|kind| plan.delays().delay_type(kind)).map(|_| name)))
}

/// The idle reasons are written to follow "Idle ·"; standing alone, they
/// start with a capital.
fn sentence_case(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

fn rate(delivery: &crate::model::schedule::result::Delivery) -> f64 {
    let duration = delivery.end_h - delivery.start_h;
    if duration > 0.0 { delivery.tonnes / duration } else { 0.0 }
}

pub(super) fn grades_text(names: &[String], tonnes: f64, contained: &[f64]) -> Option<String> {
    if tonnes <= 1e-6 || contained.is_empty() {
        return None;
    }
    let parts: Vec<String> = names
        .iter()
        .zip(contained)
        .map(|(name, quantity)| format!("{name} {}", grade_number(quantity / tonnes)))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

pub(super) fn grade_number(value: f64) -> String {
    let mut text = format!("{value:.2}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn stockpiles(ui: &mut egui::Ui, plan: &SchedulePlan, destinations: &[DestinationView], schedule: &CalculatedSchedule, grades: &[String], hour: f64, day: CalendarPeriod) {
    let piles: Vec<&DestinationView> = destinations.iter().filter(|view| view.kind == DestinationKind::Stockpile).collect();
    if piles.is_empty() {
        return;
    }
    section(ui, tr!("inspector-stockpiles"));
    for pile in piles {
        let (tonnes, contained) = schedule.inventory_at(pile.id, hour).unwrap_or((pile.opening_t, Vec::new()));
        let incoming: f64 = schedule.deliveries_at(hour).filter(|d| d.destination == pile.id).map(rate).sum();
        let outgoing: f64 = schedule.deliveries_at(hour).filter(|d| d.source == WorkSource::Stockpile(pile.id)).map(rate).sum();
        let operation = plan.stockpile_operation(pile.id);
        let mode = operation.mode_at(day);

        let mut parts = Vec::new();
        if incoming > 1e-6 {
            parts.push(tr!("inspector-pile-building", rate = format_tonnes(incoming.round())));
        }
        if outgoing > 1e-6 {
            parts.push(tr!("inspector-pile-reclaiming", rate = format_tonnes(outgoing.round())));
        }
        let full = pile.capacity_t.is_some_and(|capacity| tonnes >= capacity - 0.5);
        if parts.is_empty() {
            parts.push(if full { tr!("inspector-pile-full") } else { tr!("inspector-pile-standing") });
        }
        if mode != PileMode::default() {
            parts.push(mode.label());
        }
        // Rest is per pile only when the pile is one blend; a chunked pile
        // rests chunk by chunk, which the hover on its reclaim says.
        if operation.rest_h > 0.0
            && incoming <= 1e-6
            && plan.experiment().representation(pile.id) == StockpileRepresentation::Blended
            && let Some(last) = schedule.last_receipt_before(pile.id, hour)
            && hour < last + operation.rest_h
        {
            parts.push(tr!("inspector-pile-resting", until = instant_label((last + operation.rest_h) * GanttView::HOUR)));
        }

        let fill = pile
            .capacity_t
            .filter(|capacity| *capacity > 0.0)
            .map(|capacity| (tonnes / capacity).clamp(0.0, 1.0) as f32);
        let entry = Entry {
            name: pile.name.clone(),
            value: tr!("inspector-tonnes", tonnes = format_tonnes(tonnes.round())),
            detail: Some(parts.join(" · ")),
            meter: fill,
            ..Entry::default()
        };
        let mut lines = Vec::new();
        if let Some(text) = grades_text(grades, tonnes, &contained) {
            lines.push(text);
        }
        if let (Some(capacity), Some(fill)) = (pile.capacity_t, fill) {
            lines.push(tr!(
                "inspector-pile-capacity",
                capacity = format_tonnes(capacity.round()),
                percent = format!("{:.0}", fill * 100.0)
            ));
        }
        let response = draw_entry(ui, &entry);
        if !lines.is_empty() {
            response.on_hover_text(lines.join("\n"));
        }
    }
}

fn crushers(ui: &mut egui::Ui, plan: &SchedulePlan, destinations: &[DestinationView], schedule: &CalculatedSchedule, grades: &[String], hour: f64, day: CalendarPeriod) {
    let crushers: Vec<&DestinationView> = destinations.iter().filter(|view| view.kind == DestinationKind::Crusher).collect();
    if crushers.is_empty() {
        return;
    }
    section(ui, tr!("inspector-crushers"));
    let day_start = f64::from(day.0) * crate::model::schedule::SCHEDULE_PERIOD_H;
    for crusher in crushers {
        let feed: f64 = schedule.deliveries_at(hour).filter(|d| d.destination == crusher.id).map(rate).sum();
        let (today, _) = schedule.received_between(crusher.id, day_start, hour);
        let limit = match crusher.id {
            crate::model::schedule::DestinationId::Standalone(id) => plan.routing().crusher(id).and_then(|calendar| calendar.limit_at(day)),
            _ => None,
        };
        let mut entry = Entry {
            name: crusher.name.clone(),
            ..Entry::default()
        };
        if feed > 1e-6 {
            entry.value = tr!("inspector-rate", rate = format_tonnes(feed.round()));
        } else if limit.is_some_and(|limit| today >= limit - 0.5) {
            entry.value = tr!("inspector-crusher-at-limit");
        } else {
            entry.value = tr!("inspector-not-fed");
            entry.value_weak = true;
        }
        entry.detail = Some(match limit {
            Some(limit) => {
                entry.meter = (limit > 0.0).then(|| (today / limit).clamp(0.0, 1.0) as f32);
                tr!("inspector-crusher-today-of", tonnes = format_tonnes(today.round()), limit = format_tonnes(limit.round()))
            }
            None => tr!("inspector-crusher-today", tonnes = format_tonnes(today.round())),
        });

        // The day's blend is what a grade target prices, so the flag is
        // raised on the day's figure, not on one hour of it.
        let mut lines = Vec::new();
        let now: Vec<_> = schedule.deliveries_at(hour).filter(|d| d.destination == crusher.id).collect();
        let now_tonnes: f64 = now.iter().map(|d| rate(d)).sum();
        let mut now_contained = vec![0.0; grades.len()];
        for delivery in &now {
            let duration = (delivery.end_h - delivery.start_h).max(f64::EPSILON);
            for (total, quantity) in now_contained.iter_mut().zip(&delivery.contained) {
                *total += quantity / duration;
            }
        }
        if let Some(text) = grades_text(grades, now_tonnes, &now_contained) {
            lines.push(tr!("inspector-crusher-feed-grade", grades = text));
        }
        let mut outside = Vec::new();
        for target in schedule
            .grade_targets
            .iter()
            .filter(|target| target.period == day.0 && target.specification.destination == crusher.id && target.tonnes > 1e-6)
        {
            let spec = &target.specification;
            let Some(index) = schedule.grades.iter().position(|(field, _)| *field == spec.field) else {
                continue;
            };
            let name = grades.get(index).cloned().unwrap_or_default();
            let grade = target.contained / target.tonnes;
            let bound = |value: Option<f64>| value.map_or_else(|| "—".to_owned(), grade_number);
            lines.push(tr!(
                "inspector-crusher-day-grade",
                grade = format!("{name} {}", grade_number(grade)),
                lower = bound(spec.lower),
                target = grade_number(spec.target),
                upper = bound(spec.upper)
            ));
            if spec.lower.is_some_and(|lower| grade < lower - 1e-9) || spec.upper.is_some_and(|upper| grade > upper + 1e-9) {
                outside.push(tr!("inspector-crusher-outside", grade = format!("{name} {}", grade_number(grade))));
            }
        }
        if !outside.is_empty() {
            entry.warning = Some(outside.join(" · "));
        }
        let response = draw_entry(ui, &entry);
        if !lines.is_empty() {
            response.on_hover_text(lines.join("\n"));
        }
    }
}

fn dumps(ui: &mut egui::Ui, destinations: &[DestinationView], schedule: &CalculatedSchedule, hour: f64) {
    let dumps: Vec<&DestinationView> = destinations.iter().filter(|view| view.kind == DestinationKind::Dump).collect();
    if dumps.is_empty() {
        return;
    }
    section(ui, tr!("inspector-dumps"));
    for dump in dumps {
        let feed: f64 = schedule.deliveries_at(hour).filter(|d| d.destination == dump.id).map(rate).sum();
        let (received, _) = schedule.received_between(dump.id, 0.0, hour);
        let entry = Entry {
            name: dump.name.clone(),
            value: if feed > 1e-6 {
                tr!("inspector-rate", rate = format_tonnes(feed.round()))
            } else {
                tr!("inspector-not-receiving")
            },
            value_weak: feed <= 1e-6,
            detail: Some(tr!("inspector-dump-to-date", tonnes = format_tonnes(received.round()))),
            ..Entry::default()
        };
        draw_entry(ui, &entry);
    }
}

fn trucks(ui: &mut egui::Ui, plan: &SchedulePlan, schedule: &CalculatedSchedule, hour: f64, day: CalendarPeriod) {
    let classes = &plan.trucks().classes;
    if classes.is_empty() {
        return;
    }
    section(ui, tr!("inspector-trucks"));
    for class in classes {
        // Truck-hours per hour is trucks: how many are turning a wheel.
        let busy: f64 = schedule
            .deliveries_at(hour)
            .filter(|delivery| delivery.truck == class.id)
            .map(|delivery| {
                let duration = delivery.end_h - delivery.start_h;
                if duration > 0.0 { delivery.truck_hours / duration } else { 0.0 }
            })
            .sum();
        let fleet = class.calendar.values_at(day).effective_units();
        let entry = Entry {
            name: class.name.clone(),
            value: tr!("inspector-trucks-in-use", busy = format!("{busy:.1}"), fleet = format!("{fleet:.1}")),
            meter: (fleet > 0.0).then(|| (busy / fleet).clamp(0.0, 1.0) as f32),
            ..Entry::default()
        };
        let mut haul = crate::model::schedule::result::HaulSummary::default();
        for delivery in schedule.deliveries_at(hour).filter(|d| d.truck == class.id) {
            let duration = delivery.end_h - delivery.start_h;
            if duration > 0.0 {
                haul.add(delivery, 1.0 / duration);
            }
        }
        draw_entry(ui, &entry).on_hover_text(format!(
            "{}\n{}\n{}: {:.2}",
            tr!("inspector-trucks-help"),
            tr!("haul-cycle-minutes", minutes = format!("{:.1}", haul.cycle_minutes())),
            tr!("haul-distance"),
            haul.distance_km()
        ));
    }
}

/// A single line of text, cut with an ellipsis to fit.
fn fitted(ui: &egui::Ui, text: String, font: egui::FontId, color: egui::Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
    job.wrap.max_width = width.max(1.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    ui.painter().layout_job(job)
}

fn draw_entry(ui: &mut egui::Ui, entry: &Entry) -> egui::Response {
    let height =
        NAME_ROW_H + entry.detail.as_ref().map_or(0.0, |_| DETAIL_ROW_H) + entry.warning.as_ref().map_or(0.0, |_| DETAIL_ROW_H) + entry.meter.map_or(0.0, |_| METER_H + 3.0) + 4.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    let visuals = ui.visuals();
    if response.hovered() {
        ui.painter()
            .rect_filled(rect.expand2(egui::vec2(4.0, 0.0)), 3.0, visuals.widgets.hovered.weak_bg_fill.gamma_multiply(0.5));
    }
    let left = rect.left() + if entry.dot.is_some() { INDENT } else { 0.0 };
    let name_y = rect.top() + NAME_ROW_H / 2.0;
    if let Some(color) = entry.dot {
        ui.painter().circle_filled(egui::pos2(rect.left() + DOT_R + 1.0, name_y), DOT_R, color);
    }
    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = egui::FontId::proportional(body.size - 1.5);
    let value_color = if entry.value_weak { visuals.weak_text_color() } else { visuals.strong_text_color() };
    let value = fitted(ui, entry.value.clone(), body.clone(), value_color, rect.width() * 0.6);
    let value_pos = egui::pos2(rect.right() - value.size().x, name_y - value.size().y / 2.0);
    let name = fitted(ui, entry.name.clone(), body, visuals.text_color(), (value_pos.x - left - 8.0).max(20.0));
    ui.painter().galley(egui::pos2(left, name_y - name.size().y / 2.0), name, visuals.text_color());
    ui.painter().galley(value_pos, value, value_color);
    let mut y = rect.top() + NAME_ROW_H;
    if let Some(fill) = entry.meter {
        let track = egui::Rect::from_min_size(egui::pos2(left, y), egui::vec2(rect.right() - left, METER_H));
        ui.painter().rect_filled(track, 1.5, visuals.extreme_bg_color);
        let color = if fill >= 0.98 {
            visuals.warn_fg_color
        } else {
            visuals.selection.stroke.color.gamma_multiply(0.7)
        };
        ui.painter()
            .rect_filled(egui::Rect::from_min_size(track.min, egui::vec2(track.width() * fill, METER_H)), 1.5, color);
        y += METER_H + 3.0;
    }
    for (line, color) in [(&entry.detail, visuals.weak_text_color()), (&entry.warning, visuals.warn_fg_color)] {
        if let Some(line) = line {
            let galley = fitted(ui, line.clone(), small.clone(), color, rect.right() - left);
            ui.painter().galley(egui::pos2(left, y + (DETAIL_ROW_H - galley.size().y) / 2.0), galley, color);
            y += DETAIL_ROW_H;
        }
    }
    response
}
