//! The schedule Results window: a schedule's last run (or a periods CSV read
//! back) as two views, in the scenario Results window's style.
//!
//! - **Period by period** (the main one): the schedule - every pushback,
//!   in the chosen case - mined period by period, as mining (waste, ore to
//!   its method, ore to stockpiles) or processing (per method), with a table.
//! - **Pit by pit**: each pushback's pit as the final pit, its NPV in the
//!   three cases on one axis, with a table; picking pushbacks makes a new
//!   schedule mining just those.

use std::hash::{Hash, Hasher};

use super::optimization_results::{CASH_FLOW, ICON_SIDE, ORE, WASTE_BARS, chart_height, column_switches, draw_choosing_bar};
use crate::{
    i18n::tr,
    model::{
        npv_schedule::{PeriodCase, format_money},
        npv_schedule_run::{
            NpvScheduleResult, format_elevation,
            simulate::{Held, PeriodResult},
        },
        numbers::format_quantity,
        optimization::format_factor,
        optimization_run::report::WASTE,
    },
    ui::{
        state::{NpvScheduleResultsSource, NpvScheduleResultsTab, OptimizationState, PeriodContent, UiCommand},
        themed_icon,
        widgets::{
            bar_line_chart::{BarLineChart, BarSeries, LineScale, LineSeries},
            context_menu::context_menu_separator,
            data_table::DataTable,
            hint::Hint,
            menu::{self, DragableMenu, MenuButton, SlidingToggle, menu_note},
            toolbar::ToolbarButton,
        },
    },
};

/// The stockpile balance area: a calm teal.
const STOCKED: egui::Color32 = egui::Color32::from_rgb(95, 160, 170);
/// Each method's bars after the first (which takes [`ORE`]).
const METHODS: [egui::Color32; 4] = [
    ORE,
    egui::Color32::from_rgb(222, 120, 80),
    egui::Color32::from_rgb(190, 100, 150),
    egui::Color32::from_rgb(130, 170, 90),
];
/// Ore mined onto stockpiles in a period: a deeper teal than the balance area.
const STOCKED_BARS: egui::Color32 = egui::Color32::from_rgb(60, 130, 145);
/// Cumulative discounted cash flow: a steady blue, beside the cash flow's green.
const DISCOUNTED: egui::Color32 = egui::Color32::from_rgb(90, 140, 220);
/// Recovered metal.
const RECOVERED: egui::Color32 = egui::Color32::from_rgb(200, 150, 50);
/// The worst case's NPV line; the best takes the cash flow's green.
const WORST: egui::Color32 = egui::Color32::from_rgb(210, 110, 90);
/// The tint of a period whose mining was held back.
const HELD: egui::Color32 = egui::Color32::from_rgba_premultiplied(48, 34, 8, 48);
/// Under this share of the best case, best and worst differ too little for
/// the sequence to matter (Whittle short-course notes: 2-3 %).
const GAP_THRESHOLD: f64 = 3.0;

const CASES: [PeriodCase; 3] = [PeriodCase::FixedLead, PeriodCase::Worst, PeriodCase::Best];

pub(crate) fn draw_schedule_results_window(ui: &mut egui::Ui, state: &mut OptimizationState, canvas_rect: egui::Rect, commands: &mut Vec<UiCommand>) {
    let Some(mut view) = state.schedule_results_view.clone() else {
        return;
    };
    let result = state.schedule_results_result();
    let title = match (&view.source, &result) {
        (NpvScheduleResultsSource::File { name, .. }, _) => tr!("sched-res-title-file", name = name.clone()),
        (_, Some(result)) => tr!("sched-res-title", scenario = result.scenario.clone(), schedule = result.name.clone()),
        (_, None) => tr!("sched-res-title-empty"),
    };
    let from_run = match view.source {
        NpvScheduleResultsSource::Run { parent, id } => Some((parent, id)),
        NpvScheduleResultsSource::File { .. } => None,
    };
    let mut open = true;
    let mut close = false;
    let mut columns_changed = false;
    // The whole 3D canvas, for room to read the chart and the table.
    DragableMenu::new("schedule_results_dialog", title).open(&mut open).cover(canvas_rect).show(ui.ctx(), |ui| {
        // The notes and the buttons are pinned to the bottom; the controls,
        // chart and table fill exactly what is left above them, whatever the
        // canvas's size.
        let notes = match (result.as_deref(), view.tab) {
            (None, _) => vec![tr!("sched-res-none")],
            (Some(result), NpvScheduleResultsTab::Periods) => period_notes(result, view.case),
            (Some(result), NpvScheduleResultsTab::Pits) => pit_notes(result),
        };
        egui::Panel::bottom("sched_results_bottom")
            .frame(egui::Frame::NONE.inner_margin(egui::Margin { top: 6, ..egui::Margin::ZERO }))
            .resizable(false)
            .show_separator_line(false)
            .show(ui, |ui| {
                for note in notes {
                    menu_note(ui, note);
                }
                menu::menu_actions(ui, |ui| {
                    let escape = view.picking.is_none() && menu::dialog_cancelled(ui.ctx());
                    if ui.add(MenuButton::new(tr!("common-close"))).clicked() || escape {
                        close = true;
                    }
                    if ui.add(MenuButton::new(tr!("sched-res-export")).enabled(result.is_some())).clicked() {
                        commands.push(UiCommand::ExportNpvScheduleReport);
                    }
                    if ui.add(MenuButton::new(tr!("sched-res-open"))).clicked() {
                        commands.push(UiCommand::OpenNpvScheduleReport);
                    }
                });
            });
        let Some(result) = result.as_deref() else {
            return;
        };
        egui::CentralPanel::no_frame().show(ui, |ui| {
            // The tab, the case, and on the right the table's columns (and, in
            // the pit view, picking pushbacks).
            let table = match view.tab {
                NpvScheduleResultsTab::Periods => "periods",
                NpvScheduleResultsTab::Pits => "pits",
            };
            let catalogue = match view.tab {
                NpvScheduleResultsTab::Periods => period_catalogue(result),
                NpvScheduleResultsTab::Pits => pit_catalogue(result),
            };
            let mut shown = shown_ids(&catalogue, state.schedule_columns.get(table));
            // One line: the view, the case, the bars (or, in the pit view, the
            // other cases), then the table's columns and picking on the right.
            ui.horizontal(|ui| {
                let tab_label = |tab: NpvScheduleResultsTab| match tab {
                    NpvScheduleResultsTab::Periods => tr!("sched-res-periods"),
                    NpvScheduleResultsTab::Pits => tr!("sched-res-pits"),
                };
                let before = view.tab;
                egui::ComboBox::from_id_salt("sched_res_tab")
                    .selected_text(tab_label(view.tab))
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for tab in [NpvScheduleResultsTab::Periods, NpvScheduleResultsTab::Pits] {
                            ui.selectable_value(&mut view.tab, tab, tab_label(tab));
                        }
                    });
                if view.tab != before {
                    view.picking = None;
                }
                match view.tab {
                    NpvScheduleResultsTab::Periods => {
                        let case_label = |case: PeriodCase| match case {
                            PeriodCase::FixedLead => tr!("sched-res-case-lead", lead = result.lead.to_string()),
                            other => other.label(),
                        };
                        egui::ComboBox::from_id_salt("sched_res_case")
                            .selected_text(case_label(view.case))
                            .width(150.0)
                            .show_ui(ui, |ui| {
                                for case in CASES {
                                    ui.selectable_value(&mut view.case, case, case_label(case));
                                }
                            });
                        let content_label = |content: PeriodContent| match content {
                            PeriodContent::Mining => tr!("sched-res-mining"),
                            PeriodContent::Processing => tr!("sched-res-processing"),
                        };
                        egui::ComboBox::from_id_salt("sched_res_content")
                            .selected_text(content_label(view.content))
                            .width(130.0)
                            .show_ui(ui, |ui| {
                                for content in [PeriodContent::Mining, PeriodContent::Processing] {
                                    ui.selectable_value(&mut view.content, content, content_label(content));
                                }
                            });
                    }
                    NpvScheduleResultsTab::Pits => {
                        ui.add_space(8.0);
                        ui.label(tr!("sched-res-all-cases")).hint(tr!("sched-res-all-cases-hint"));
                        ui.add(SlidingToggle::new(&mut view.all_cases)).hint(tr!("sched-res-all-cases-hint"));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    columns_changed = column_chooser(ui, &catalogue, &mut shown);
                    if view.tab == NpvScheduleResultsTab::Pits && from_run.is_some() {
                        let tooltip = if view.picking.is_some() { tr!("sched-res-pick-stop") } else { tr!("sched-res-pick") };
                        let button = ToolbarButton::new(egui::Image::new(themed_icon!(ui, "filter_shells.svg")), tooltip)
                            .id_salt("sched_res_pick")
                            .button_side(ICON_SIDE)
                            .selected(view.picking.is_some());
                        if ui.add(button).clicked() {
                            view.picking = if view.picking.is_some() { None } else { Some(Vec::new()) };
                        }
                    }
                });
            });
            if columns_changed {
                state.schedule_columns.insert(table.to_owned(), shown.clone());
            }
            ui.add_space(6.0);
            match view.tab {
                NpvScheduleResultsTab::Periods => {
                    draw_periods(ui, result, view.case, view.content, &catalogue, &shown);
                }
                NpvScheduleResultsTab::Pits => {
                    if let Some(picked) = view.picking.as_mut() {
                        match draw_choosing_bar(ui, picked.len()) {
                            Some(true) => {
                                if let Some((parent, id)) = from_run {
                                    let factors = picked.iter().filter_map(|&pit| result.pits.get(pit)).map(|pit| result.shells[pit.shell].factor).collect();
                                    commands.push(UiCommand::NpvScheduleFromPushbacks { parent, id, factors });
                                }
                                view.picking = None;
                            }
                            Some(false) => view.picking = None,
                            None if menu::dialog_cancelled(ui.ctx()) => view.picking = None,
                            None => {}
                        }
                    }
                    if let Some(clicked) = draw_pits(ui, result, view.picking.as_deref(), view.all_cases, &catalogue, &shown)
                        && let Some(picked) = view.picking.as_mut()
                    {
                        match picked.iter().position(|&pit| pit == clicked) {
                            Some(index) => {
                                picked.remove(index);
                            }
                            None => picked.push(clicked),
                        }
                    }
                }
            }
        });
    });
    if columns_changed {
        commands.push(UiCommand::SaveNpvScheduleColumns);
    }
    state.schedule_results_view = (!close && open).then_some(view);
}

/// A method's (1-based destination's) colour.
fn method_color(destination: usize) -> egui::Color32 {
    METHODS[(destination - 1) % METHODS.len()]
}

fn element_name(result: &NpvScheduleResult, element: usize) -> String {
    let info = &result.elements[element];
    if info.grade_unit.is_empty() {
        info.name.clone()
    } else {
        format!("{} ({})", info.name, info.grade_unit)
    }
}

fn destination_name(result: &NpvScheduleResult, destination: usize) -> String {
    let name = &result.destinations[destination];
    if destination == 0 && name == WASTE { tr!("opt-results-waste") } else { name.clone() }
}

// ── Period by period ──

/// The schedule's periods in `case`: its chart, its table and notes.
fn draw_periods(ui: &mut egui::Ui, result: &NpvScheduleResult, case: PeriodCase, content: PeriodContent, catalogue: &[Column], shown: &[String]) {
    let chart_height = chart_height(ui);
    let Some(pit) = result.schedule() else {
        menu_note(ui, tr!("sched-res-none"));
        return;
    };
    let schedule = pit.case(case);
    let periods = &schedule.periods;
    let labels: Vec<String> = (1..=periods.len()).map(|period| period.to_string()).collect();
    let methods = 1..result.destinations.len();
    let per_period = |figure: &dyn Fn(&PeriodResult) -> f64| -> Vec<f64> { periods.iter().map(figure).collect() };
    let cash = per_period(&|period| period.cash_flow);
    let mut cumulative = -result.capex_initial;
    let discounted: Vec<f64> = periods
        .iter()
        .map(|period| {
            cumulative += period.discounted_cash_flow;
            cumulative
        })
        .collect();
    let waste = per_period(&|period| period.waste);
    let stocked = per_period(&|period| period.stocked);
    let direct: Vec<Vec<f64>> = methods.clone().map(|method| per_period(&|period| period.mined_direct[method])).collect();
    let processed: Vec<Vec<f64>> = methods.clone().map(|method| per_period(&|period| period.processed[method])).collect();
    let balance = per_period(&|period| period.balance_total());
    let recovered = per_period(&|period| period.recovered.first().copied().unwrap_or(0.0));

    let hover = |column: usize| period_hover(result, &periods[column], column + 1, discounted[column]);
    let salt = match content {
        PeriodContent::Mining => "sched_res_periods_mining",
        PeriodContent::Processing => "sched_res_periods_processing",
    };
    let mut chart = BarLineChart::new(&labels).height(chart_height).id_salt(salt);
    match content {
        PeriodContent::Mining => {
            // Every stockpile's balance as one area, at the back.
            if !result.stockpiles.is_empty() {
                chart = chart.area(BarSeries {
                    name: tr!("sched-res-stockpile-balance"),
                    color: STOCKED,
                    values: &balance,
                });
            }
            chart = chart.bars(BarSeries {
                name: tr!("opt-results-waste-tonnes"),
                color: WASTE_BARS,
                values: &waste,
            });
            for (index, method) in methods.clone().enumerate() {
                chart = chart.bars(BarSeries {
                    name: tr!("sched-res-ore-to", method = destination_name(result, method)),
                    color: method_color(method),
                    values: &direct[index],
                });
            }
            if !result.stockpiles.is_empty() {
                chart = chart.bars(BarSeries {
                    name: tr!("sched-res-ore-stocked"),
                    color: STOCKED_BARS,
                    values: &stocked,
                });
            }
            chart = chart
                .line(LineSeries {
                    name: tr!("opt-results-cash-flow"),
                    color: CASH_FLOW,
                    values: &cash,
                    scale: LineScale::Own(None),
                })
                .line(LineSeries {
                    name: tr!("sched-res-npv"),
                    color: DISCOUNTED,
                    values: &discounted,
                    scale: LineScale::SameAs(0),
                });
        }
        PeriodContent::Processing => {
            for (index, method) in methods.clone().enumerate() {
                chart = chart.bars(BarSeries {
                    name: tr!("sched-res-processed-by", method = destination_name(result, method)),
                    color: method_color(method),
                    values: &processed[index],
                });
            }
            chart = chart.line(LineSeries {
                name: tr!("opt-results-cash-flow"),
                color: CASH_FLOW,
                values: &cash,
                scale: LineScale::Own(None),
            });
            if !result.elements.is_empty() {
                chart = chart.line(LineSeries {
                    name: tr!("sched-res-recovered", element = result.elements[0].name.clone()),
                    color: RECOVERED,
                    values: &recovered,
                    scale: LineScale::Own(None),
                });
            }
        }
    }
    let held = periods.iter().enumerate().filter(|(_, period)| period.held.is_some()).map(|(column, _)| column);
    chart.marked(held, HELD).hover_text(&hover).show(ui);

    ui.add_space(6.0);
    // Period 0 holds the initial capital, as in the CSV.
    let opening = PeriodResult {
        capex: result.capex_initial,
        cost: result.capex_initial,
        cash_flow: -result.capex_initial,
        discounted_cash_flow: -result.capex_initial,
        ..PeriodResult::default()
    };
    let rows: Vec<&PeriodResult> = std::iter::once(&opening).chain(periods).collect();
    let mut running = 0.0;
    let cumulative: Vec<f64> = rows
        .iter()
        .map(|period| {
            running += period.discounted_cash_flow;
            running
        })
        .collect();
    let columns: Vec<&Column> = catalogue.iter().filter(|column| shown.contains(&column.id)).collect();
    let header: Vec<String> = columns.iter().map(|column| column.header.clone()).collect();
    let cell = |row: usize, exact: bool| -> Vec<String> {
        columns
            .iter()
            .map(|column| match column.kind {
                Kind::Period(figure) => figure.cell(rows[row], row, cumulative[row], exact),
                Kind::Pit(_) => String::new(),
            })
            .collect()
    };
    // The table takes all that is left.
    draw_table(ui, ("periods", result.name.as_str(), case_key(case)), &header, rows.len(), &cell, f32::INFINITY);
}

/// The period view's notes, under the table: periods, NPV, what is left on
/// the stockpiles, and whether every pushback was mined.
fn period_notes(result: &NpvScheduleResult, case: PeriodCase) -> Vec<String> {
    let Some(pit) = result.schedule() else {
        return Vec::new();
    };
    let schedule = pit.case(case);
    let mut notes = vec![tr!(
        "sched-res-period-notes",
        periods = schedule.periods.len().to_string(),
        npv = format_money(schedule.npv),
        abandoned = format_quantity(schedule.abandoned, 0)
    )];
    if schedule.unfinished {
        notes.push(tr!("sched-res-unfinished"));
    }
    notes
}

fn period_hover(result: &NpvScheduleResult, period: &PeriodResult, number: usize, cumulative: f64) -> String {
    let amount = |value: f64| format_quantity(value, 0);
    let mut lines = vec![tr!("sched-res-hover-period", period = number.to_string())];
    lines.push(tr!("sched-res-hover-mined", mined = amount(period.mined), waste = amount(period.waste)));
    for method in 1..result.destinations.len() {
        lines.push(tr!(
            "sched-res-hover-method",
            method = destination_name(result, method),
            direct = amount(period.mined_direct[method]),
            processed = amount(period.processed[method])
        ));
    }
    if !result.stockpiles.is_empty() {
        lines.push(tr!("sched-res-hover-stock", stocked = amount(period.stocked), reclaimed = amount(period.reclaimed)));
        for (index, name) in result.stockpiles.iter().enumerate() {
            lines.push(tr!("sched-res-hover-balance", stockpile = name.clone(), balance = amount(period.stockpile_balance[index])));
        }
    }
    if let Some(grade) = period.feed_grade(0) {
        lines.push(format!("{}: {grade:.3}", tr!("sched-res-feed-grade", element = element_name(result, 0))));
    }
    lines.push(tr!(
        "opt-results-hover-money",
        revenue = amount(period.revenue),
        cost = amount(period.cost),
        cash = amount(period.cash_flow)
    ));
    lines.push(tr!("sched-res-hover-dcf", dcf = amount(period.discounted_cash_flow), cumulative = amount(cumulative)));
    match period.held {
        Some(Held::NoStockpile) => lines.push(tr!("sched-res-held-no-stockpile")),
        Some(Held::StockpilesFull) => lines.push(tr!("sched-res-held-stockpiles-full")),
        None => {}
    }
    lines.join("\n")
}

// ── Pit by pit ──

/// Each pushback's pit: the chart (NPV per case on one axis over the pit's
/// tonnes), the table and notes. Returns a clicked pushback (0-based).
fn draw_pits(ui: &mut egui::Ui, result: &NpvScheduleResult, picking: Option<&[usize]>, all_cases: bool, catalogue: &[Column], shown: &[String]) -> Option<usize> {
    let chart_height = chart_height(ui);
    let labels: Vec<String> = (0..result.pits.len()).map(|pit| pit_label(result, pit)).collect();
    let npv = |case: PeriodCase| -> Vec<f64> {
        result
            .pits
            .iter()
            .map(|pit| {
                let schedule = pit.case(case);
                if schedule.unfinished { f64::NAN } else { schedule.npv }
            })
            .collect()
    };
    let (lead, worst, best) = (npv(PeriodCase::FixedLead), npv(PeriodCase::Worst), npv(PeriodCase::Best));
    let ore: Vec<f64> = result.pits.iter().map(|pit| pit.ore).collect();
    let waste: Vec<f64> = result.pits.iter().map(|pit| pit.waste).collect();
    let accent = menu::accent_fill(ui.visuals());
    let hover = |column: usize| {
        let pit = &result.pits[column];
        let mut lines = vec![labels[column].clone()];
        lines.push(tr!("opt-results-hover-tonnes", ore = format_quantity(pit.ore, 0), waste = format_quantity(pit.waste, 0)));
        for case in CASES {
            let schedule = pit.case(case);
            let mut line = tr!(
                "sched-res-hover-case",
                case = case.label(),
                npv = format_money(schedule.npv),
                periods = schedule.periods.len().to_string()
            );
            if schedule.unfinished {
                line.push(' ');
                line.push_str(&tr!("sched-res-unfinished-mark"));
            }
            lines.push(line);
        }
        if pit.lead.abandoned > 0.0 {
            lines.push(tr!("sched-res-hover-abandoned", tonnes = format_quantity(pit.lead.abandoned, 0)));
        }
        lines.join("\n")
    };
    let peak = result.peak(PeriodCase::FixedLead);
    // Only pushbacks being picked are outlined; the peak is told in a note.
    let selected: Vec<usize> = picking.map(<[usize]>::to_vec).unwrap_or_default();
    let lead_name = tr!("sched-res-npv-case", case = PeriodCase::FixedLead.label());
    let mut chart = BarLineChart::new(&labels)
        .height(chart_height)
        .id_salt("sched_res_pits")
        .bars(BarSeries {
            name: tr!("opt-results-ore-tonnes"),
            color: ORE,
            values: &ore,
        })
        .bars(BarSeries {
            name: tr!("opt-results-waste-tonnes"),
            color: WASTE_BARS,
            values: &waste,
        })
        .line(LineSeries {
            name: lead_name.clone(),
            color: accent,
            values: &lead,
            scale: LineScale::Own(None),
        });
    // Best and worst only when asked: the bounds fixed lead should fall within.
    if all_cases {
        chart = chart
            .line(LineSeries {
                name: tr!("sched-res-npv-case", case = PeriodCase::Worst.label()),
                color: WORST,
                values: &worst,
                scale: LineScale::SameAs(0),
            })
            .line(LineSeries {
                name: tr!("sched-res-npv-case", case = PeriodCase::Best.label()),
                color: CASH_FLOW,
                values: &best,
                scale: LineScale::SameAs(0),
            });
    }
    // The highest fixed-lead NPV: its node ringed.
    if let Some(peak) = peak {
        chart = chart.emphasize(lead_name, peak);
    }
    let clicked = chart.selected(selected).hover_text(&hover).show(ui).clicked;

    ui.add_space(6.0);
    let columns: Vec<&Column> = catalogue.iter().filter(|column| shown.contains(&column.id)).collect();
    let header: Vec<String> = columns.iter().map(|column| column.header.clone()).collect();
    // One row per bench of each pushback, top first; a pushback whose benches
    // are not known (read from a file) has one row.
    let rows: Vec<(usize, Option<usize>)> = result
        .pits
        .iter()
        .enumerate()
        .flat_map(|(pit, candidate)| {
            let benches: Vec<Option<usize>> = if candidate.benches.is_empty() {
                vec![None]
            } else {
                (0..candidate.benches.len()).map(Some).collect()
            };
            benches.into_iter().map(move |bench| (pit, bench))
        })
        .collect();
    let cell = |row: usize, exact: bool| -> Vec<String> {
        let (pit, bench) = rows[row];
        columns
            .iter()
            .map(|column| match column.kind {
                Kind::Pit(figure) => figure.cell(result, pit, bench, exact),
                Kind::Period(_) => String::new(),
            })
            .collect()
    };
    // The table takes all that is left.
    draw_table(ui, ("pits", result.name.as_str(), ""), &header, rows.len(), &cell, f32::INFINITY);
    clicked
}

/// The pit view's notes: the gap test and the bounds at the schedule, then
/// where the NPV peaks.
fn pit_notes(result: &NpvScheduleResult) -> Vec<String> {
    let mut notes = Vec::new();
    if let Some(pit) = result.schedule() {
        let (best, worst, lead) = (pit.best.npv, pit.worst.npv, pit.lead.npv);
        if best.abs() > 0.0 {
            let gap = (best - worst) / best.abs() * 100.0;
            let percent = format!("{gap:.1}");
            notes.push(if gap < GAP_THRESHOLD {
                tr!("sched-res-gap-small", percent = percent)
            } else {
                tr!("sched-res-gap", percent = percent)
            });
        }
        let slack = 1e-6 * best.abs().max(worst.abs()).max(1.0);
        if lead < best.min(worst) - slack || lead > best.max(worst) + slack {
            notes.push(tr!("sched-res-outside-bounds"));
        }
    }
    if let Some(peak) = result.peak(PeriodCase::FixedLead).filter(|&peak| peak + 1 < result.pits.len()) {
        notes.push(tr!("sched-res-peak", pushback = (peak + 1).to_string()));
    }
    notes
}

/// A pushback's column label: `PB 2 · RAF 0.85`, with the share of the
/// distance for a directional shell.
fn pit_label(result: &NpvScheduleResult, pit: usize) -> String {
    let shell = &result.shells[result.pits[pit].shell];
    let factor = match shell.distance {
        None => format_factor(shell.factor),
        Some(share) => format!("{} {:.0}%", format_factor(shell.factor), share * 100.0),
    };
    tr!("sched-res-pit-label", pushback = (pit + 1).to_string(), factor = factor)
}

fn case_key(case: PeriodCase) -> &'static str {
    match case {
        PeriodCase::FixedLead => "fixed_lead",
        PeriodCase::Worst => "worst",
        PeriodCase::Best => "best",
    }
}

// ── Tables ──

/// One column a table can show: its id (the CSV's column name, so a choice
/// holds for a file read back), its header, and what fills it.
#[derive(Clone, Debug, PartialEq)]
struct Column {
    id: String,
    header: String,
    kind: Kind,
    /// Always shown: it names the row.
    fixed: bool,
    default_shown: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Period(PeriodFigure),
    Pit(PitFigure),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PeriodFigure {
    Period,
    Held,
    Mined,
    Waste,
    Direct(usize),
    Processed(usize),
    ProcessedTotal,
    Stocked,
    Reclaimed,
    StockpileStocked(usize),
    StockpileReclaimed(usize),
    Balance(usize),
    /// A stockpile's grade: of what was put on (0), reclaimed (1), or is on it (2).
    StockpileGrade(usize, usize),
    BalanceTotal,
    FeedGrade(usize),
    Recovered(usize),
    Revenue,
    MiningCost,
    ProcessingCost,
    RehandleCost,
    FixedCost,
    Capex,
    TotalCost,
    CashFlow,
    Discounted,
    Cumulative,
}

impl PeriodFigure {
    fn cell(self, period: &PeriodResult, number: usize, cumulative: f64, exact: bool) -> String {
        let amount = |value: f64| if exact { format!("{value}") } else { format_quantity(value, 0) };
        let get = |values: &[f64], index: usize| values.get(index).copied().unwrap_or(0.0);
        match self {
            Self::Period => number.to_string(),
            Self::Held => match period.held {
                None => String::new(),
                Some(Held::NoStockpile) => tr!("sched-res-held-short-no-stockpile"),
                Some(Held::StockpilesFull) => tr!("sched-res-held-short-full"),
            },
            Self::Mined => amount(period.mined),
            Self::Waste => amount(period.waste),
            Self::Direct(method) => amount(get(&period.mined_direct, method)),
            Self::Processed(method) => amount(get(&period.processed, method)),
            Self::ProcessedTotal => amount(period.processed_total()),
            Self::Stocked => amount(period.stocked),
            Self::Reclaimed => amount(period.reclaimed),
            Self::StockpileStocked(stockpile) => amount(get(&period.stocked_by, stockpile)),
            Self::StockpileReclaimed(stockpile) => amount(get(&period.reclaimed_by, stockpile)),
            Self::Balance(stockpile) => amount(get(&period.stockpile_balance, stockpile)),
            Self::StockpileGrade(stockpile, figure) => period
                .stockpile_grade(stockpile, figure)
                .map_or(String::new(), |grade| if exact { format!("{grade}") } else { format!("{grade:.3}") }),
            Self::BalanceTotal => amount(period.balance_total()),
            Self::FeedGrade(element) => period
                .feed_grade(element)
                .map_or(String::new(), |grade| if exact { format!("{grade}") } else { format!("{grade:.3}") }),
            Self::Recovered(element) => amount(get(&period.recovered, element)),
            Self::Revenue => amount(period.revenue),
            Self::MiningCost => amount(period.mining_cost),
            Self::ProcessingCost => amount(period.processing_cost),
            Self::RehandleCost => amount(period.rehandle_cost),
            Self::FixedCost => amount(period.fixed_cost),
            Self::Capex => amount(period.capex),
            Self::TotalCost => amount(period.cost),
            Self::CashFlow => amount(period.cash_flow),
            Self::Discounted => amount(period.discounted_cash_flow),
            Self::Cumulative => amount(cumulative),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PitFigure {
    Pushback,
    Bench,
    BenchOre,
    BenchWaste,
    BenchGrade,
    BenchPeriod,
    Shell,
    Raf,
    Direction,
    Ore,
    Waste,
    Npv(PeriodCase),
    Periods(PeriodCase),
    Abandoned(PeriodCase),
}

impl PitFigure {
    fn cell(self, result: &NpvScheduleResult, pit: usize, bench: Option<usize>, exact: bool) -> String {
        let amount = |value: f64| if exact { format!("{value}") } else { format_quantity(value, 0) };
        let candidate = &result.pits[pit];
        let bench = bench.and_then(|bench| candidate.benches.get(bench));
        let shell = &result.shells[candidate.shell];
        let mark = |case: PeriodCase| {
            if candidate.case(case).unfinished {
                format!(" {}", tr!("sched-res-unfinished-mark"))
            } else {
                String::new()
            }
        };
        match self {
            Self::Pushback => (pit + 1).to_string(),
            Self::Bench => bench.map_or(
                String::new(),
                |bench| if exact { format!("{}", bench.elevation) } else { format_elevation(bench.elevation) },
            ),
            Self::BenchOre => bench.map_or(String::new(), |bench| amount(bench.ore)),
            Self::BenchWaste => bench.map_or(String::new(), |bench| amount(bench.waste)),
            Self::BenchGrade => bench.filter(|bench| bench.ore > 0.0).map_or(String::new(), |bench| {
                let grade = bench.grade_tonnes / bench.ore;
                if exact { format!("{grade}") } else { format!("{grade:.3}") }
            }),
            Self::BenchPeriod => bench.and_then(|bench| bench.period).map_or(String::new(), |period| period.to_string()),
            Self::Shell => shell.number.to_string(),
            Self::Raf => format_factor(shell.factor),
            Self::Direction => shell.distance.map_or(String::new(), |share| format!("{:.0}%", share * 100.0)),
            Self::Ore => amount(candidate.ore),
            Self::Waste => amount(candidate.waste),
            Self::Npv(case) => format!("{}{}", amount(candidate.case(case).npv), mark(case)),
            Self::Periods(case) => candidate.case(case).periods.len().to_string(),
            Self::Abandoned(case) => amount(candidate.case(case).abandoned),
        }
    }
}

fn column(id: impl Into<String>, header: String, kind: Kind, default_shown: bool) -> Column {
    Column {
        id: id.into(),
        header,
        kind,
        fixed: false,
        default_shown,
    }
}

/// Every column of the period table, in the CSV's order.
fn period_catalogue(result: &NpvScheduleResult) -> Vec<Column> {
    use PeriodFigure as F;
    let period = |id: &str, header: String, figure: F, default_shown: bool| column(id, header, Kind::Period(figure), default_shown);
    let mut columns = vec![Column {
        fixed: true,
        ..period("period", tr!("sched-res-col-period"), F::Period, true)
    }];
    columns.extend([
        period("held", tr!("sched-res-col-held"), F::Held, true),
        period("mined_t", tr!("sched-res-col-mined"), F::Mined, true),
        period("waste_t", tr!("sched-res-col-waste"), F::Waste, true),
        period("processed_t", tr!("sched-res-col-processed"), F::ProcessedTotal, true),
        period("stocked_t", tr!("sched-res-col-stocked"), F::Stocked, !result.stockpiles.is_empty()),
        period("reclaimed_t", tr!("sched-res-col-reclaimed"), F::Reclaimed, !result.stockpiles.is_empty()),
        period("stockpile_balance_t", tr!("sched-res-col-balance-total"), F::BalanceTotal, !result.stockpiles.is_empty()),
    ]);
    // The three ways ore moves: mine to a method, mine to a stockpile, and
    // stockpile to a method (reclaim). A method's feed is direct plus reclaimed.
    for method in 1..result.destinations.len() {
        let name = destination_name(result, method);
        let key = &result.destinations[method];
        columns.extend([
            period(
                &format!("method:{key}:direct_t"),
                tr!("sched-res-col-direct", method = name.clone()),
                F::Direct(method),
                true,
            ),
            period(
                &format!("method:{key}:processed_t"),
                tr!("sched-res-col-method-processed", method = name),
                F::Processed(method),
                true,
            ),
        ]);
    }
    let grade_unit = result.elements.first().map_or_else(String::new, |element| element.grade_unit.clone());
    let graded = |text: String| if grade_unit.is_empty() { text } else { format!("{text} ({grade_unit})") };
    for (index, name) in result.stockpiles.iter().enumerate() {
        let figure = |field: &str| format!("stockpile:{name}:{field}");
        columns.extend([
            period(
                &figure("stocked_t"),
                tr!("sched-res-col-stockpile-stocked", stockpile = name.clone()),
                F::StockpileStocked(index),
                true,
            ),
            period(
                &figure("stocked_grade"),
                graded(tr!("sched-res-col-stockpile-stocked-grade", stockpile = name.clone())),
                F::StockpileGrade(index, 0),
                true,
            ),
            period(
                &figure("reclaimed_t"),
                tr!("sched-res-col-stockpile-reclaimed", stockpile = name.clone()),
                F::StockpileReclaimed(index),
                true,
            ),
            period(
                &figure("reclaimed_grade"),
                graded(tr!("sched-res-col-stockpile-reclaimed-grade", stockpile = name.clone())),
                F::StockpileGrade(index, 1),
                true,
            ),
            period(&figure("balance_t"), tr!("sched-res-col-balance", stockpile = name.clone()), F::Balance(index), true),
            period(
                &figure("balance_grade"),
                graded(tr!("sched-res-col-balance-grade", stockpile = name.clone())),
                F::StockpileGrade(index, 2),
                true,
            ),
        ]);
    }
    for (index, element) in result.elements.iter().enumerate() {
        columns.extend([
            period(
                &format!("element:{}:feed_grade", element.name),
                tr!("sched-res-col-feed-grade", element = element_name(result, index)),
                F::FeedGrade(index),
                index == 0,
            ),
            period(
                &format!("element:{}:recovered", element.name),
                tr!("sched-res-col-recovered", element = element.name.clone()),
                F::Recovered(index),
                index == 0,
            ),
        ]);
    }
    columns.extend([
        period("revenue", tr!("opt-results-col-revenue"), F::Revenue, true),
        period("mining_cost", tr!("sched-res-col-mining-cost"), F::MiningCost, false),
        period("processing_cost", tr!("sched-res-col-processing-cost"), F::ProcessingCost, false),
        period("rehandle_cost", tr!("sched-res-col-rehandle-cost"), F::RehandleCost, false),
        period("fixed_cost", tr!("sched-res-col-fixed-cost"), F::FixedCost, false),
        period("capex", tr!("sched-res-col-capex"), F::Capex, false),
        period("total_cost", tr!("opt-results-col-cost"), F::TotalCost, true),
        period("cash_flow", tr!("opt-results-col-cash-flow"), F::CashFlow, true),
        period("discounted_cash_flow", tr!("sched-res-col-dcf"), F::Discounted, true),
        period("cumulative_discounted_cash_flow", tr!("sched-res-col-cumulative"), F::Cumulative, true),
    ]);
    columns
}

/// Every column of the pit table.
fn pit_catalogue(result: &NpvScheduleResult) -> Vec<Column> {
    use PitFigure as F;
    let pit = |id: String, header: String, figure: F, default_shown: bool| column(id, header, Kind::Pit(figure), default_shown);
    let directional = result.shells.iter().any(|shell| shell.distance.is_some());
    let mut columns = vec![
        Column {
            fixed: true,
            ..pit("pushback".into(), tr!("sched-res-col-pushback"), F::Pushback, true)
        },
        Column {
            fixed: true,
            ..pit("bench_elevation".into(), tr!("sched-res-col-bench"), F::Bench, true)
        },
    ];
    let grade = result.elements.first().map_or_else(String::new, |_| element_name(result, 0));
    columns.extend([
        pit("bench_ore_t".into(), tr!("sched-res-col-bench-ore"), F::BenchOre, true),
        pit("bench_waste_t".into(), tr!("sched-res-col-bench-waste"), F::BenchWaste, true),
        pit(
            "bench_grade".into(),
            tr!("sched-res-col-bench-grade", element = grade),
            F::BenchGrade,
            !result.elements.is_empty(),
        ),
        pit("bench_period".into(), tr!("sched-res-col-bench-period"), F::BenchPeriod, true),
        pit("shell".into(), tr!("opt-results-col-shell"), F::Shell, true),
        pit("raf".into(), tr!("opt-results-col-raf"), F::Raf, true),
    ]);
    if directional {
        columns.push(pit("dir_pct".into(), tr!("opt-results-col-dir"), F::Direction, true));
    }
    columns.extend([
        pit("pit_ore_t".into(), tr!("opt-results-col-ore"), F::Ore, true),
        pit("pit_waste_t".into(), tr!("opt-results-col-waste"), F::Waste, true),
    ]);
    for case in [PeriodCase::Worst, PeriodCase::FixedLead, PeriodCase::Best] {
        let key = case_key(case);
        columns.extend([
            pit(format!("npv_{key}"), tr!("sched-res-col-npv", case = case.label()), F::Npv(case), true),
            pit(format!("periods_{key}"), tr!("sched-res-col-periods", case = case.label()), F::Periods(case), true),
            pit(format!("abandoned_{key}"), tr!("sched-res-col-abandoned", case = case.label()), F::Abandoned(case), false),
        ]);
    }
    columns
}

/// The column ids saved for this table, in catalogue order, or the default ones.
fn shown_ids(catalogue: &[Column], saved: Option<&Vec<String>>) -> Vec<String> {
    catalogue
        .iter()
        .filter(|column| column.fixed || saved.map_or(column.default_shown, |saved| saved.contains(&column.id)))
        .map(|column| column.id.clone())
        .collect()
}

/// The cog and its popup of columns. Returns whether the choice changed.
fn column_chooser(ui: &mut egui::Ui, catalogue: &[Column], shown: &mut Vec<String>) -> bool {
    let mut changed = false;
    let cog = ToolbarButton::new(egui::Image::new(themed_icon!(ui, "open_preferences.svg")), tr!("opt-results-columns"))
        .id_salt("sched_res_columns")
        .button_side(ICON_SIDE);
    let response = ui.add(cog);
    egui::Popup::menu(&response).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_min_width(240.0);
        let rows: Vec<(String, bool)> = catalogue.iter().map(|column| (column.header.clone(), column.fixed)).collect();
        let mut on: Vec<bool> = catalogue.iter().map(|column| shown.contains(&column.id)).collect();
        if column_switches(ui, &rows, &mut on) {
            *shown = catalogue.iter().zip(&on).filter(|(_, on)| **on).map(|(column, _)| column.id.clone()).collect();
            changed = true;
        }
        context_menu_separator(ui);
        if ui.add(MenuButton::new(tr!("opt-results-columns-default"))).clicked() {
            *shown = shown_ids(catalogue, None);
            changed = true;
        }
    });
    changed
}

fn draw_table(ui: &mut egui::Ui, key: (&str, &str, &str), header: &[String], rows: usize, cell: &dyn Fn(usize, bool) -> Vec<String>, height: f32) {
    let aligns: Vec<egui::Align> = header.iter().map(|_| egui::Align::Max).collect();
    let rounded = |row: usize| cell(row, false);
    let exact = |row: usize| cell(row, true);
    let fingerprint = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (key, header, rows).hash(&mut hasher);
        hasher.finish()
    };
    DataTable::new(("sched_res_table", key.0), header, rows, &rounded)
        .aligns(&aligns)
        .copy_cells(&exact)
        .fingerprint(fingerprint)
        .max_height(height)
        .show(ui);
}
