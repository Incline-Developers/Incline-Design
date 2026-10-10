//! The Results window: a run's report as the pit-by-pit chart (ore and waste
//! tonnes per shell, cash flow at the base price, the main grade) over a table
//! of the shells whose columns the user picks.
//!
//! It shows a scenario's last run this session, or an `optimization_results.csv`
//! opened from disk. See `.claude/skills/optimization/SKILL.md` (stage 3).

use crate::{
    i18n::tr,
    model::{
        optimization::format_factor,
        optimization_run::report::{Basis, Bucket, OptimizationReport, WASTE, strip_ratio},
        plot::format_quantity,
    },
    ui::{
        state::{OptimizationState, ShellFilter, UiCommand},
        themed_icon,
        widgets::{
            bar_line_chart::{BarLineChart, BarSeries, LineSeries},
            data_table::DataTable,
            menu::{self, DragableMenu, MenuButton, menu_note},
            toolbar::ToolbarButton,
        },
    },
};

const WIDTH: f32 = 860.0;
const CHART_HEIGHT: f32 = 280.0;
const TABLE_HEIGHT: f32 = 240.0;
/// Cash flow line: the green of the run button.
const CASH_FLOW: egui::Color32 = egui::Color32::from_rgb(0x2f, 0xb3, 0x44);
/// Main grade line: a muted violet, apart from the bars and the cash flow.
const GRADE: egui::Color32 = egui::Color32::from_rgb(150, 110, 210);
/// Ore bars: a calm amber; waste bars: stone grey.
const ORE: egui::Color32 = egui::Color32::from_rgb(214, 160, 72);
const WASTE_BARS: egui::Color32 = egui::Color32::from_rgb(150, 152, 160);
/// Where the grade line sits, as fractions of the chart's height: the middle,
/// clear of the bars' tops and of the cash flow's ends.
const GRADE_BAND: (f32, f32) = (0.35, 0.65);
const ICON_SIDE: f32 = 26.0;

pub(crate) fn draw_results_window(ui: &mut egui::Ui, state: &mut OptimizationState, commands: &mut Vec<UiCommand>) {
    let Some(view) = state.results_view.as_ref() else {
        return;
    };
    let report = state.results_report();
    let mut open = true;
    let mut close = false;
    let (mut basis, mut destination, mut selected, mut filter) = (view.basis, view.destination, view.selected, view.filter.clone());
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
                    let columns = catalogue(report);
                    let mut shown = shown_columns(report, &columns, state.report_columns.get(&report.scenario));
                    if draw_controls(ui, report, &mut basis, &mut destination, &mut filter, &columns, &mut shown) {
                        state.report_columns.insert(report.scenario.clone(), shown.iter().map(|column| column.id(report)).collect());
                        commands.push(UiCommand::SaveOptimizationReportColumns);
                    }
                    if let ShellFilter::Choosing(chosen) = &mut filter {
                        match draw_choosing_bar(ui, chosen.len()) {
                            Some(true) => {
                                let mut shells = std::mem::take(chosen);
                                shells.sort_unstable();
                                filter = ShellFilter::Applied(shells);
                            }
                            Some(false) => filter = ShellFilter::Off,
                            None if menu::dialog_cancel_pressed(ui.ctx()) => filter = ShellFilter::Off,
                            None => {}
                        }
                    }
                    ui.add_space(6.0);
                    // Choosing shows every shell to click; applied, only the chosen.
                    let visible: Vec<usize> = match &filter {
                        ShellFilter::Applied(shells) => shells.iter().copied().filter(|&shell| shell < report.shells.len()).collect(),
                        _ => (0..report.shells.len()).collect(),
                    };
                    let rows = rows(report, basis, destination, &visible);
                    let highlighted: Vec<usize> = match &filter {
                        ShellFilter::Choosing(chosen) => chosen.clone(),
                        _ => selected.into_iter().collect(),
                    };
                    if let Some(clicked) = draw_chart(ui, report, &rows, &highlighted) {
                        match &mut filter {
                            // A click adds the shell; a click on a chosen one takes it out.
                            ShellFilter::Choosing(chosen) => match chosen.iter().position(|&shell| shell == clicked) {
                                Some(index) => {
                                    chosen.remove(index);
                                }
                                None => chosen.push(clicked),
                            },
                            _ => selected = (selected != Some(clicked)).then_some(clicked),
                        }
                    }
                    ui.add_space(6.0);
                    draw_table(ui, report, basis, destination, &rows, &shown);
                    menu_note(
                        ui,
                        if matches!(filter, ShellFilter::Applied(_)) && basis == Basis::Incremental {
                            tr!("opt-results-basis-note-chosen")
                        } else {
                            tr!("opt-results-basis-note")
                        },
                    );
                }
            }
            menu::menu_actions(ui, |ui| {
                // Escape leaves choosing first; the window stays.
                let escape = !matches!(filter, ShellFilter::Choosing(_)) && menu::dialog_cancel_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!("common-close"))).clicked() || escape {
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
        view.filter = filter;
    }
}

/// The name a destination is shown by: waste translated, methods as named.
fn destination_label(report: &OptimizationReport, destination: usize) -> String {
    let name = &report.destinations[destination];
    if destination == 0 && name == WASTE { tr!("opt-results-waste") } else { name.clone() }
}

/// Basis, destination, the shell filter and the table's column chooser.
/// Returns whether the columns changed.
fn draw_controls(
    ui: &mut egui::Ui,
    report: &OptimizationReport,
    basis: &mut Basis,
    destination: &mut Option<usize>,
    filter: &mut ShellFilter,
    columns: &[Column],
    shown: &mut Vec<Column>,
) -> bool {
    let mut changed = false;
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
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let cog = ToolbarButton::new(egui::Image::new(themed_icon!(ui, "open_preferences.svg")), tr!("opt-results-columns"))
                .id_salt("opt_results_columns")
                .button_side(ICON_SIDE);
            let response = ui.add(cog);
            egui::Popup::menu(&response).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.set_min_width(220.0);
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for column in columns {
                        let mut on = shown.contains(column);
                        // The shell number names the row: always shown.
                        let fixed = *column == Column::Shell;
                        if ui.add_enabled(!fixed, egui::Checkbox::new(&mut on, column.header(report))).changed() {
                            *shown = columns.iter().copied().filter(|other| if other == column { on } else { shown.contains(other) }).collect();
                            changed = true;
                        }
                    }
                });
                ui.separator();
                if ui.add(MenuButton::new(tr!("opt-results-columns-default"))).clicked() {
                    *shown = default_columns(columns);
                    changed = true;
                }
            });
            // Off: start choosing. Choosing or applied (shown pressed): drop it.
            let tooltip = match filter {
                ShellFilter::Off => tr!("opt-results-filter"),
                ShellFilter::Choosing(_) => tr!("opt-results-filter-choosing"),
                ShellFilter::Applied(shells) => tr!("opt-results-filter-applied", count = shells.len().to_string()),
            };
            let button = ToolbarButton::new(egui::Image::new(themed_icon!(ui, "filter_shells.svg")), tooltip)
                .id_salt("opt_results_filter")
                .button_side(ICON_SIDE)
                .selected(*filter != ShellFilter::Off);
            if ui.add(button).clicked() {
                *filter = match filter {
                    ShellFilter::Off => ShellFilter::Choosing(Vec::new()),
                    _ => ShellFilter::Off,
                };
            }
        });
    });
    changed
}

/// While shells are being chosen: what to do, how many are chosen, and Apply
/// and Cancel. `Some(true)` when applied, `Some(false)` when cancelled.
fn draw_choosing_bar(ui: &mut egui::Ui, chosen: usize) -> Option<bool> {
    let mut outcome = None;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tr!("opt-results-choosing-hint")).color(ui.visuals().selection.stroke.color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() {
                outcome = Some(false);
            }
            if ui.add(MenuButton::new(tr!("opt-results-apply")).primary().enabled(chosen > 0)).clicked() {
                outcome = Some(true);
            }
            ui.label(tr!("opt-results-chosen-count", count = chosen.to_string()));
        });
    });
    outcome
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

/// One shell as the chart and the table show it.
struct Row {
    /// 0-based.
    shell: usize,
    bucket: Bucket,
    ore: f64,
    waste: f64,
    /// The main grade: of the ore with every destination shown (waste would
    /// dilute it), otherwise of the destination shown.
    grade: Option<f64>,
}

/// The figures of each shown shell. Cumulative is the shell's pit; an
/// increment is measured from the shell shown before it - the run's previous
/// shell, or with a filter the previous chosen one (the pushback between them).
fn rows(report: &OptimizationReport, basis: Basis, destination: Option<usize>, visible: &[usize]) -> Vec<Row> {
    visible
        .iter()
        .enumerate()
        .map(|(index, &shell)| {
            let after = match basis {
                Basis::Cumulative => None,
                Basis::Incremental => index.checked_sub(1).map(|before| visible[before]),
            };
            let bucket = report.bucket_since(after, shell, destination);
            let (ore, waste) = match destination {
                None => {
                    let waste = report.bucket_since(after, shell, Some(0)).tonnes;
                    (bucket.tonnes - waste, waste)
                }
                Some(0) => (0.0, bucket.tonnes),
                Some(_) => (bucket.tonnes, 0.0),
            };
            let grade = if report.elements.is_empty() {
                None
            } else {
                match destination {
                    None => {
                        let mut ore = (1..report.destinations.len()).map(|destination| report.bucket_since(after, shell, Some(destination)));
                        ore.next().and_then(|mut sum| {
                            ore.for_each(|bucket| sum.add(&bucket));
                            sum.grade(0)
                        })
                    }
                    Some(_) => bucket.grade(0),
                }
            };
            Row { shell, bucket, ore, waste, grade }
        })
        .collect()
}

/// The chart; returns the shell (0-based) of a clicked column.
fn draw_chart(ui: &mut egui::Ui, report: &OptimizationReport, rows: &[Row], highlighted: &[usize]) -> Option<usize> {
    let labels: Vec<String> = rows.iter().map(|row| shell_label(report, row.shell)).collect();
    let cash_flow: Vec<f64> = rows.iter().map(|row| row.bucket.cash_flow()).collect();
    let ore: Vec<f64> = rows.iter().map(|row| row.ore).collect();
    let waste: Vec<f64> = rows.iter().map(|row| row.waste).collect();
    let grade: Vec<f64> = rows.iter().map(|row| row.grade.unwrap_or(f64::NAN)).collect();
    let grade_name = report.elements.first().map(|element| {
        let unit = if element.grade_unit.is_empty() {
            String::new()
        } else {
            format!(" ({})", element.grade_unit)
        };
        tr!("opt-results-main-grade", element = format!("{}{unit}", element.name))
    });
    let hover = |column: usize| {
        let row = &rows[column];
        let mut lines = vec![
            tr!(
                "opt-results-hover-shell",
                shell = report.shells[row.shell].number.to_string(),
                factor = shell_label(report, row.shell)
            ),
            tr!("opt-results-hover-tonnes", ore = format_quantity(row.ore, 0), waste = format_quantity(row.waste, 0)),
        ];
        if let Some(ratio) = strip_ratio(row.ore, row.waste) {
            lines.push(tr!("opt-results-hover-strip", ratio = format!("{ratio:.2}")));
        }
        if let (Some(name), Some(grade)) = (&grade_name, row.grade) {
            lines.push(format!("{name}: {grade:.3}"));
        }
        lines.push(tr!(
            "opt-results-hover-money",
            revenue = format_quantity(row.bucket.revenue(), 0),
            cost = format_quantity(row.bucket.total_cost(), 0),
            cash = format_quantity(row.bucket.cash_flow(), 0)
        ));
        lines.join("\n")
    };
    let mut chart = BarLineChart::new(&labels)
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
            band: None,
        });
    if let Some(name) = &grade_name {
        chart = chart.line(LineSeries {
            name: name.clone(),
            color: GRADE,
            values: &grade,
            band: Some(GRADE_BAND),
        });
    }
    let columns = rows.iter().enumerate().filter(|(_, row)| highlighted.contains(&row.shell)).map(|(column, _)| column);
    chart.selected(columns).hover_text(&hover).show(ui).clicked.map(|column| rows[column].shell)
}

/// One column the table can show: every figure the report holds per shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Column {
    Shell,
    Raf,
    Direction,
    Blocks,
    Tonnes,
    Ore,
    Waste,
    Strip,
    Grade(usize),
    Contained(usize),
    Recovered(usize),
    ElementRevenue(usize),
    ElementCost(usize),
    Mining,
    WasteHaulage,
    Rehab,
    OreHaulage,
    Processing,
    Ga,
    ElementCosts,
    TotalCost,
    Revenue,
    CashFlow,
}

impl Column {
    /// What the column is saved as: the CSV's own column names, so a saved
    /// choice survives a rerun and holds for the report read back from CSV.
    fn id(self, report: &OptimizationReport) -> String {
        let element = |index: usize| &report.elements[index].name;
        match self {
            Self::Shell => "shell".to_owned(),
            Self::Raf => "raf".to_owned(),
            Self::Direction => "dir_pct".to_owned(),
            Self::Blocks => "blocks".to_owned(),
            Self::Tonnes => "tonnes".to_owned(),
            Self::Ore => "ore_tonnes".to_owned(),
            Self::Waste => "waste_tonnes".to_owned(),
            Self::Strip => "strip_ratio".to_owned(),
            Self::Grade(index) => format!("{} grade", element(index)),
            Self::Contained(index) => format!("{} contained", element(index)),
            Self::Recovered(index) => format!("{} recovered", element(index)),
            Self::ElementRevenue(index) => format!("{} revenue", element(index)),
            Self::ElementCost(index) => format!("{} cost", element(index)),
            Self::Mining => "mining_cost".to_owned(),
            Self::WasteHaulage => "waste_haulage_cost".to_owned(),
            Self::Rehab => "rehab_cost".to_owned(),
            Self::OreHaulage => "ore_haulage_cost".to_owned(),
            Self::Processing => "processing_cost".to_owned(),
            Self::Ga => "ga_cost".to_owned(),
            Self::ElementCosts => "element_cost".to_owned(),
            Self::TotalCost => "total_cost".to_owned(),
            Self::Revenue => "revenue".to_owned(),
            Self::CashFlow => "cash_flow".to_owned(),
        }
    }

    fn header(self, report: &OptimizationReport) -> String {
        let element = |index: usize| report.elements[index].name.clone();
        let with_unit = |text: String, unit: &str| if unit.is_empty() { text } else { format!("{text} ({unit})") };
        match self {
            Self::Shell => tr!("opt-results-col-shell"),
            Self::Raf => tr!("opt-results-col-raf"),
            Self::Direction => tr!("opt-results-col-dir"),
            Self::Blocks => tr!("opt-results-col-blocks"),
            Self::Tonnes => tr!("opt-results-col-tonnes"),
            Self::Ore => tr!("opt-results-col-ore"),
            Self::Waste => tr!("opt-results-col-waste"),
            Self::Strip => tr!("opt-results-col-strip"),
            Self::Grade(index) => with_unit(element(index), &report.elements[index].grade_unit),
            Self::Contained(index) => with_unit(tr!("opt-results-col-contained", element = element(index)), &report.elements[index].sales_unit),
            Self::Recovered(index) => with_unit(tr!("opt-results-col-recovered", element = element(index)), &report.elements[index].sales_unit),
            Self::ElementRevenue(index) => tr!("opt-results-col-element-revenue", element = element(index)),
            Self::ElementCost(index) => tr!("opt-results-col-element-cost", element = element(index)),
            Self::Mining => tr!("opt-results-col-mining"),
            Self::WasteHaulage => tr!("opt-results-col-waste-haulage"),
            Self::Rehab => tr!("opt-results-col-rehab"),
            Self::OreHaulage => tr!("opt-results-col-ore-haulage"),
            Self::Processing => tr!("opt-results-col-processing"),
            Self::Ga => tr!("opt-results-col-ga"),
            Self::ElementCosts => tr!("opt-results-col-element-costs"),
            Self::TotalCost => tr!("opt-results-col-cost"),
            Self::Revenue => tr!("opt-results-col-revenue"),
            Self::CashFlow => tr!("opt-results-col-cash-flow"),
        }
    }

    /// Shown until the user picks columns.
    fn default_shown(self) -> bool {
        matches!(
            self,
            Self::Shell
                | Self::Raf
                | Self::Direction
                | Self::Blocks
                | Self::Tonnes
                | Self::Ore
                | Self::Waste
                | Self::Strip
                | Self::Grade(_)
                | Self::Revenue
                | Self::TotalCost
                | Self::CashFlow
        )
    }

    /// The cell's text: rounded to read, or exact for a copy.
    fn cell(self, report: &OptimizationReport, shell: usize, bucket: &Bucket, (ore, waste): (f64, f64), exact: bool) -> String {
        let amount = |value: f64| if exact { format!("{value}") } else { format_quantity(value, 0) };
        let fine = |value: f64, decimals: usize| if exact { format!("{value}") } else { format!("{value:.decimals$}") };
        let info = &report.shells[shell];
        match self {
            Self::Shell => info.number.to_string(),
            Self::Raf => format_factor(info.factor),
            Self::Direction => info.distance.map_or(String::new(), |share| format!("{:.0}%", share * 100.0)),
            Self::Blocks => bucket.blocks.to_string(),
            Self::Tonnes => amount(bucket.tonnes),
            Self::Ore => amount(ore),
            Self::Waste => amount(waste),
            Self::Strip => strip_ratio(ore, waste).map_or(String::new(), |ratio| fine(ratio, 2)),
            Self::Grade(index) => bucket.grade(index).map_or(String::new(), |grade| fine(grade, 3)),
            Self::Contained(index) => amount(bucket.elements[index].contained),
            Self::Recovered(index) => amount(bucket.elements[index].recovered),
            Self::ElementRevenue(index) => amount(bucket.elements[index].revenue),
            Self::ElementCost(index) => amount(bucket.elements[index].cost),
            Self::Mining => amount(bucket.mining),
            Self::WasteHaulage => amount(bucket.waste_haulage),
            Self::Rehab => amount(bucket.rehab),
            Self::OreHaulage => amount(bucket.ore_haulage),
            Self::Processing => amount(bucket.processing),
            Self::Ga => amount(bucket.ga),
            Self::ElementCosts => amount(bucket.element_cost()),
            Self::TotalCost => amount(bucket.total_cost()),
            Self::Revenue => amount(bucket.revenue()),
            Self::CashFlow => amount(bucket.cash_flow()),
        }
    }
}

/// Every column the report can fill, in the CSV's order. Direction only for
/// directional shells.
fn catalogue(report: &OptimizationReport) -> Vec<Column> {
    let directional = report.shells.iter().any(|shell| shell.distance.is_some());
    let mut columns = vec![Column::Shell, Column::Raf];
    if directional {
        columns.push(Column::Direction);
    }
    columns.extend([Column::Blocks, Column::Tonnes, Column::Ore, Column::Waste, Column::Strip]);
    for index in 0..report.elements.len() {
        columns.extend([
            Column::Grade(index),
            Column::Contained(index),
            Column::Recovered(index),
            Column::ElementRevenue(index),
            Column::ElementCost(index),
        ]);
    }
    columns.extend([
        Column::Mining,
        Column::WasteHaulage,
        Column::Rehab,
        Column::OreHaulage,
        Column::Processing,
        Column::Ga,
        Column::ElementCosts,
        Column::TotalCost,
        Column::Revenue,
        Column::CashFlow,
    ]);
    columns
}

fn default_columns(columns: &[Column]) -> Vec<Column> {
    columns.iter().copied().filter(|column| column.default_shown()).collect()
}

/// The columns saved for this report, in catalogue order, or the default ones.
fn shown_columns(report: &OptimizationReport, columns: &[Column], saved: Option<&Vec<String>>) -> Vec<Column> {
    let Some(saved) = saved else {
        return default_columns(columns);
    };
    columns
        .iter()
        .copied()
        .filter(|column| *column == Column::Shell || saved.contains(&column.id(report)))
        .collect()
}

fn draw_table(ui: &mut egui::Ui, report: &OptimizationReport, basis: Basis, destination: Option<usize>, rows: &[Row], shown: &[Column]) {
    let header: Vec<String> = shown.iter().map(|column| column.header(report)).collect();
    let aligns: Vec<egui::Align> = shown.iter().map(|_| egui::Align::Max).collect();
    let row = |index: usize, exact: bool| -> Vec<String> {
        let row = &rows[index];
        shown
            .iter()
            .map(|column| column.cell(report, row.shell, &row.bucket, (row.ore, row.waste), exact))
            .collect()
    };
    let rounded = |index: usize| row(index, false);
    let exact = |index: usize| row(index, true);
    let fingerprint = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let shells: Vec<usize> = rows.iter().map(|row| row.shell).collect();
        (report.scenario.as_str(), &shells, basis == Basis::Cumulative, destination, &header).hash(&mut hasher);
        hasher.finish()
    };
    DataTable::new("opt_results_table", &header, rows.len(), &rounded)
        .aligns(&aligns)
        .copy_cells(&exact)
        .fingerprint(fingerprint)
        .max_height(TABLE_HEIGHT)
        .show(ui);
}
