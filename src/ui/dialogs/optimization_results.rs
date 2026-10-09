//! The Results window: a run's report as the pit-by-pit chart (ore and waste
//! tonnes per shell, cash flow at the base price) over a table of the shells.
//!
//! It shows a scenario's last run this session, or an `optimization_results.csv`
//! opened from disk. See `.claude/skills/optimization/SKILL.md` (stage 3).

use crate::{
    i18n::tr,
    model::{
        optimization::format_factor,
        optimization_run::report::{Basis, OptimizationReport, WASTE, strip_ratio},
        plot::format_quantity,
    },
    ui::{
        state::{OptimizationState, UiCommand},
        widgets::{
            bar_line_chart::{BarLineChart, BarSeries, LineSeries},
            data_table::DataTable,
            menu::{self, DragableMenu, MenuButton, menu_note},
        },
    },
};

const WIDTH: f32 = 860.0;
const CHART_HEIGHT: f32 = 280.0;
const TABLE_HEIGHT: f32 = 240.0;
/// Cash flow line: the green of the run button.
const CASH_FLOW: egui::Color32 = egui::Color32::from_rgb(0x2f, 0xb3, 0x44);
/// Ore bars: a calm amber; waste bars: stone grey.
const ORE: egui::Color32 = egui::Color32::from_rgb(214, 160, 72);
const WASTE_BARS: egui::Color32 = egui::Color32::from_rgb(150, 152, 160);

pub(crate) fn draw_results_window(ui: &mut egui::Ui, state: &mut OptimizationState, commands: &mut Vec<UiCommand>) {
    let Some(view) = state.results_view.as_ref() else {
        return;
    };
    let report = state.results_report();
    let mut open = true;
    let mut close = false;
    let (mut basis, mut destination, mut selected) = (view.basis, view.destination, view.selected);
    let title = match &report {
        Some((name, _)) => tr!("opt-results-title", name = name.clone()),
        None => tr!("opt-results-title-empty"),
    };
    DragableMenu::new("optimization_results_dialog", title)
        .open(&mut open)
        .min_width(WIDTH)
        .max_width(WIDTH)
        .show(ui.ctx(), |ui| {
            match &report {
                None => menu_note(ui, tr!("opt-results-none")),
                Some((_, report)) => {
                    draw_controls(ui, report, &mut basis, &mut destination);
                    ui.add_space(6.0);
                    if let Some(clicked) = draw_chart(ui, report, basis, destination, selected) {
                        selected = (selected != Some(clicked)).then_some(clicked);
                    }
                    ui.add_space(6.0);
                    draw_table(ui, report, basis, destination);
                    menu_note(ui, tr!("opt-results-basis-note"));
                }
            }
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("common-close"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("opt-results-export")).enabled(report.is_some())).clicked() {
                    commands.push(UiCommand::ExportOptimizationReport);
                }
                if ui.add(MenuButton::new(tr!("opt-results-open"))).clicked() {
                    commands.push(UiCommand::OpenOptimizationReport);
                }
            });
        });
    if close || !open {
        state.results_view = None;
    } else if let Some(view) = state.results_view.as_mut() {
        view.basis = basis;
        view.destination = destination;
        view.selected = selected;
    }
}

/// The name a destination is shown by: waste translated, methods as named.
fn destination_label(report: &OptimizationReport, destination: usize) -> String {
    let name = &report.destinations[destination];
    if destination == 0 && name == WASTE { tr!("opt-results-waste") } else { name.clone() }
}

fn draw_controls(ui: &mut egui::Ui, report: &OptimizationReport, basis: &mut Basis, destination: &mut Option<usize>) {
    ui.horizontal(|ui| {
        ui.radio_value(basis, Basis::Cumulative, tr!("opt-results-cumulative"))
            .on_hover_text(tr!("opt-results-cumulative-hint"));
        ui.radio_value(basis, Basis::Incremental, tr!("opt-results-incremental"))
            .on_hover_text(tr!("opt-results-incremental-hint"));
        ui.add_space(16.0);
        ui.label(tr!("opt-results-destination"));
        let label = |destination: Option<usize>| destination.map_or_else(|| tr!("opt-results-all-destinations"), |index| destination_label(report, index));
        egui::ComboBox::from_id_salt("opt_results_destination")
            .selected_text(label(*destination))
            .width(180.0)
            .show_ui(ui, |ui| {
                for option in std::iter::once(None).chain((0..report.destinations.len()).map(Some)) {
                    ui.selectable_value(destination, option, label(option));
                }
            });
    });
}

/// A shell's label under its column: its revenue factor, and the share of the
/// distance for a directional shell.
fn shell_label(report: &OptimizationReport, shell: usize) -> String {
    let info = &report.shells[shell];
    match info.distance {
        None => format_factor(info.factor),
        Some(share) => format!("{} {:.0}%", format_factor(info.factor), share * 100.0),
    }
}

fn draw_chart(ui: &mut egui::Ui, report: &OptimizationReport, basis: Basis, destination: Option<usize>, selected: Option<usize>) -> Option<usize> {
    let count = report.shells.len();
    let labels: Vec<String> = (0..count).map(|shell| shell_label(report, shell)).collect();
    let buckets: Vec<_> = (0..count).map(|shell| report.bucket(basis, shell, destination)).collect();
    let cash_flow: Vec<f64> = buckets.iter().map(|bucket| bucket.cash_flow()).collect();
    let (ore, waste): (Vec<f64>, Vec<f64>) = match destination {
        None => (0..count).map(|shell| report.ore_waste(basis, shell)).unzip(),
        Some(0) => (vec![0.0; count], buckets.iter().map(|bucket| bucket.tonnes).collect()),
        Some(_) => (buckets.iter().map(|bucket| bucket.tonnes).collect(), vec![0.0; count]),
    };
    let hover = |shell: usize| {
        let bucket = &buckets[shell];
        let mut lines = vec![
            tr!(
                "opt-results-hover-shell",
                shell = report.shells[shell].number.to_string(),
                factor = shell_label(report, shell)
            ),
            tr!("opt-results-hover-tonnes", ore = format_quantity(ore[shell], 0), waste = format_quantity(waste[shell], 0)),
        ];
        if let Some(ratio) = strip_ratio(ore[shell], waste[shell]) {
            lines.push(tr!("opt-results-hover-strip", ratio = format!("{ratio:.2}")));
        }
        lines.push(tr!(
            "opt-results-hover-money",
            revenue = format_quantity(bucket.revenue(), 0),
            cost = format_quantity(bucket.total_cost(), 0),
            cash = format_quantity(bucket.cash_flow(), 0)
        ));
        lines.join("\n")
    };
    BarLineChart::new(&labels)
        .height(CHART_HEIGHT)
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
            name: tr!("opt-results-cash-flow"),
            color: CASH_FLOW,
            values: &cash_flow,
        })
        .selected(selected)
        .hover_text(&hover)
        .show(ui)
        .clicked
}

fn draw_table(ui: &mut egui::Ui, report: &OptimizationReport, basis: Basis, destination: Option<usize>) {
    let directional = report.shells.iter().any(|shell| shell.distance.is_some());
    let mut header = vec![tr!("opt-results-col-shell"), tr!("opt-results-col-raf")];
    if directional {
        header.push(tr!("opt-results-col-dir"));
    }
    header.extend([
        tr!("opt-results-col-blocks"),
        tr!("opt-results-col-tonnes"),
        tr!("opt-results-col-ore"),
        tr!("opt-results-col-waste"),
        tr!("opt-results-col-strip"),
    ]);
    for element in &report.elements {
        let unit = if element.grade_unit.is_empty() {
            String::new()
        } else {
            format!(" ({})", element.grade_unit)
        };
        header.push(format!("{}{unit}", element.name));
    }
    header.extend([tr!("opt-results-col-revenue"), tr!("opt-results-col-cost"), tr!("opt-results-col-cash-flow")]);
    let aligns: Vec<egui::Align> = header.iter().map(|_| egui::Align::Max).collect();

    let row = |shell: usize, exact: bool| -> Vec<String> {
        let bucket = report.bucket(basis, shell, destination);
        let (ore, waste) = match destination {
            None => report.ore_waste(basis, shell),
            Some(0) => (0.0, bucket.tonnes),
            Some(_) => (bucket.tonnes, 0.0),
        };
        let amount = |value: f64| if exact { format!("{value}") } else { format_quantity(value, 0) };
        let info = &report.shells[shell];
        let mut cells = vec![info.number.to_string(), format_factor(info.factor)];
        if directional {
            cells.push(info.distance.map_or(String::new(), |share| format!("{:.0}%", share * 100.0)));
        }
        cells.extend([bucket.blocks.to_string(), amount(bucket.tonnes), amount(ore), amount(waste)]);
        cells.push(strip_ratio(ore, waste).map_or(String::new(), |ratio| if exact { format!("{ratio}") } else { format!("{ratio:.2}") }));
        for element in 0..report.elements.len() {
            cells.push(
                bucket
                    .grade(element)
                    .map_or(String::new(), |grade| if exact { format!("{grade}") } else { format!("{grade:.3}") }),
            );
        }
        cells.extend([amount(bucket.revenue()), amount(bucket.total_cost()), amount(bucket.cash_flow())]);
        cells
    };
    let shown = |shell: usize| row(shell, false);
    let copied = |shell: usize| row(shell, true);
    let fingerprint = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (report.scenario.as_str(), report.shells.len(), basis == Basis::Cumulative, destination).hash(&mut hasher);
        hasher.finish()
    };
    DataTable::new("opt_results_table", &header, report.shells.len(), &shown)
        .aligns(&aligns)
        .copy_cells(&copied)
        .fingerprint(fingerprint)
        .max_height(TABLE_HEIGHT)
        .show(ui);
}
