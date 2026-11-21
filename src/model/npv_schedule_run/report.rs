//! A schedule's CSV files: `<schedule>_periods.csv`, which reads back into a
//! [`NpvScheduleResult`], and `<schedule>_pits.csv`, a summary.
//!
//! The periods file is tidy: one row per pushback pit, case and period
//! (period 0 holds the initial capital), each row carrying its pit's figures
//! too, so the file alone rebuilds the result. Headers are fixed English
//! names; the per-method, per-stockpile and per-element columns are named
//! `method:<name>:<figure>`, `stockpile:<name>:<figure>` and
//! `element:<name>:<figure> (<unit>)`, so they read back whatever the names.

use std::io::Read;

use anyhow::{Context, Result, bail};

use super::{
    CandidatePit, NpvScheduleResult,
    simulate::{Held, NpvPitSchedule, PeriodResult},
};
use crate::model::{
    npv_schedule::PeriodCase,
    optimization_run::report::{ElementInfo, ShellInfo, WASTE},
};

/// File name endings, after the schedule's name.
pub(crate) const PERIODS_FILE: &str = "periods.csv";
pub(crate) const PITS_FILE: &str = "pits.csv";

const CASES: [(PeriodCase, &str); 3] = [(PeriodCase::Worst, "worst"), (PeriodCase::FixedLead, "fixed_lead"), (PeriodCase::Best, "best")];

const PIT: [&str; 16] = [
    "scenario",
    "schedule",
    "pushback",
    "shell",
    "raf",
    "dir_pct",
    "case",
    "discount_rate",
    "capex_initial",
    "npv",
    "unfinished",
    "abandoned_t",
    "pit_ore_t",
    "pit_waste_t",
    "is_schedule",
    "lead_benches",
];
const PERIOD: [&str; 17] = [
    "period",
    "held",
    "mined_t",
    "waste_t",
    "stocked_t",
    "reclaimed_t",
    "revenue",
    "mining_cost",
    "processing_cost",
    "rehandle_cost",
    "fixed_cost",
    "capex",
    "total_cost",
    "cash_flow",
    "discounted_cash_flow",
    "processed_t",
    "stockpile_balance_t",
];
/// Per method: ore mined straight to it, and all it processed (reclaimed too).
const METHOD_FIGURES: [&str; 2] = ["direct_t", "processed_t"];
/// Per stockpile: put on it from the mine, reclaimed from it to its method,
/// and on it at the end, each with its main-quality grade.
const STOCKPILE_FIGURES: [&str; 6] = ["stocked_t", "stocked_grade", "reclaimed_t", "reclaimed_grade", "balance_t", "balance_grade"];

fn held_name(held: Option<Held>) -> &'static str {
    match held {
        None => "",
        Some(Held::NoStockpile) => "no_stockpile",
        Some(Held::StockpilesFull) => "stockpiles_full",
    }
}

fn number(value: f64) -> String {
    if value.is_finite() { format!("{value}") } else { String::new() }
}

fn with_unit(label: String, unit: &str) -> String {
    if unit.is_empty() { label } else { format!("{label} ({unit})") }
}

fn element_headers(element: &ElementInfo) -> [String; 2] {
    [
        with_unit(format!("element:{}:feed_grade", element.name), &element.grade_unit),
        with_unit(format!("element:{}:recovered", element.name), &element.sales_unit),
    ]
}

/// The periods file as CSV bytes.
pub(crate) fn write_periods_csv(result: &NpvScheduleResult) -> Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    let mut header: Vec<String> = PIT.iter().chain(&PERIOD).map(|name| (*name).to_owned()).collect();
    for method in &result.destinations[1..] {
        header.extend(METHOD_FIGURES.iter().map(|figure| format!("method:{method}:{figure}")));
    }
    for stockpile in &result.stockpiles {
        header.extend(STOCKPILE_FIGURES.iter().map(|figure| format!("stockpile:{stockpile}:{figure}")));
    }
    for element in &result.elements {
        header.extend(element_headers(element));
    }
    writer.write_record(&header)?;
    let last = result.pits.len().saturating_sub(1);
    for (pushback, pit) in result.pits.iter().enumerate() {
        let shell = &result.shells[pit.shell];
        for (case, name) in CASES {
            let schedule = pit.case(case);
            let lead = vec![
                result.scenario.clone(),
                result.name.clone(),
                (pushback + 1).to_string(),
                shell.number.to_string(),
                number(shell.factor),
                shell.distance.map_or(String::new(), |share| number(share * 100.0)),
                name.to_owned(),
                number(result.rate),
                number(result.capex_initial),
                number(schedule.npv),
                schedule.unfinished.to_string(),
                number(schedule.abandoned),
                number(pit.ore),
                number(pit.waste),
                (pushback == last).to_string(),
                result.lead.to_string(),
            ];
            // Period 0: the initial capital, undiscounted.
            let opening = PeriodResult {
                capex: result.capex_initial,
                cost: result.capex_initial,
                cash_flow: -result.capex_initial,
                discounted_cash_flow: -result.capex_initial,
                mined_direct: vec![0.0; result.destinations.len()],
                processed: vec![0.0; result.destinations.len()],
                stocked_by: vec![0.0; result.stockpiles.len()],
                reclaimed_by: vec![0.0; result.stockpiles.len()],
                stockpile_balance: vec![0.0; result.stockpiles.len()],
                stocked_grade_tonnes: vec![0.0; result.stockpiles.len()],
                reclaimed_grade_tonnes: vec![0.0; result.stockpiles.len()],
                balance_grade_tonnes: vec![0.0; result.stockpiles.len()],
                feed_grade_tonnes: vec![0.0; result.elements.len()],
                recovered: vec![0.0; result.elements.len()],
                ..PeriodResult::default()
            };
            for (period, figures) in std::iter::once(&opening).chain(&schedule.periods).enumerate() {
                let mut record = lead.clone();
                record.extend([period.to_string(), held_name(figures.held).to_owned()]);
                record.extend(
                    [
                        figures.mined,
                        figures.waste,
                        figures.stocked,
                        figures.reclaimed,
                        figures.revenue,
                        figures.mining_cost,
                        figures.processing_cost,
                        figures.rehandle_cost,
                        figures.fixed_cost,
                        figures.capex,
                        figures.cost,
                        figures.cash_flow,
                        figures.discounted_cash_flow,
                        figures.processed_total(),
                        figures.balance_total(),
                    ]
                    .map(number),
                );
                for destination in 1..result.destinations.len() {
                    record.extend([figures.mined_direct[destination], figures.processed[destination]].map(number));
                }
                let grade = |stockpile: usize, figure: usize| figures.stockpile_grade(stockpile, figure).map_or(String::new(), number);
                for stockpile in 0..result.stockpiles.len() {
                    record.extend([
                        number(figures.stocked_by[stockpile]),
                        grade(stockpile, 0),
                        number(figures.reclaimed_by[stockpile]),
                        grade(stockpile, 1),
                        number(figures.stockpile_balance[stockpile]),
                        grade(stockpile, 2),
                    ]);
                }
                for element in 0..result.elements.len() {
                    record.push(figures.feed_grade(element).map_or(String::new(), number));
                    record.push(number(figures.recovered[element]));
                }
                writer.write_record(&record)?;
            }
        }
    }
    writer.into_inner().context("Could not finish the schedule report")
}

/// The pits summary as CSV bytes: one row per bench of each pushback (top
/// first), with its pushback's pit figures in every case beside it. A
/// pushback whose benches are not known (read from a file) has one row.
pub(crate) fn write_pits_csv(result: &NpvScheduleResult) -> Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    let mut header: Vec<String> = [
        "scenario",
        "schedule",
        "pushback",
        "shell",
        "raf",
        "dir_pct",
        "bench_elevation",
        "bench_ore_t",
        "bench_waste_t",
        "bench_grade",
        "bench_period",
        "pit_ore_t",
        "pit_waste_t",
    ]
    .iter()
    .map(|name| (*name).to_owned())
    .collect();
    for (_, name) in CASES {
        header.extend([format!("npv_{name}"), format!("periods_{name}"), format!("unfinished_{name}")]);
    }
    header.extend(["is_schedule".to_owned(), "is_peak".to_owned()]);
    writer.write_record(&header)?;
    let last = result.pits.len().saturating_sub(1);
    let peak = result.peak(PeriodCase::FixedLead);
    for (pushback, pit) in result.pits.iter().enumerate() {
        let shell = &result.shells[pit.shell];
        let mut tail: Vec<String> = vec![number(pit.ore), number(pit.waste)];
        for (case, _) in CASES {
            let schedule = pit.case(case);
            tail.extend([number(schedule.npv), schedule.periods.len().to_string(), schedule.unfinished.to_string()]);
        }
        tail.extend([(pushback == last).to_string(), (peak == Some(pushback)).to_string()]);
        let lead = [
            result.scenario.clone(),
            result.name.clone(),
            (pushback + 1).to_string(),
            shell.number.to_string(),
            number(shell.factor),
            shell.distance.map_or(String::new(), |share| number(share * 100.0)),
        ];
        let benches: Vec<[String; 5]> = if pit.benches.is_empty() {
            vec![Default::default()]
        } else {
            pit.benches
                .iter()
                .map(|bench| {
                    [
                        number(bench.elevation),
                        number(bench.ore),
                        number(bench.waste),
                        if bench.ore > 0.0 { number(bench.grade_tonnes / bench.ore) } else { String::new() },
                        bench.period.map_or(String::new(), |period| period.to_string()),
                    ]
                })
                .collect()
        };
        for bench in benches {
            writer.write_record(lead.iter().cloned().chain(bench).chain(tail.iter().cloned()))?;
        }
    }
    writer.into_inner().context("Could not finish the schedule summary")
}

/// Read a periods file written by [`write_periods_csv`] back.
pub(crate) fn read_periods_csv(reader: impl Read) -> Result<NpvScheduleResult> {
    let mut reader = csv::Reader::from_reader(reader);
    let header: Vec<String> = reader.headers()?.iter().map(str::to_owned).collect();
    let column = |name: &str| header.iter().position(|column| column == name).with_context(|| format!("The file has no '{name}' column"));
    let pit_columns: Vec<usize> = PIT.iter().map(|name| column(name)).collect::<Result<_>>()?;
    let period_columns: Vec<usize> = PERIOD.iter().map(|name| column(name)).collect::<Result<_>>()?;

    // The named groups, in file order.
    let mut methods: Vec<String> = Vec::new();
    let mut stockpiles: Vec<String> = Vec::new();
    let mut elements: Vec<ElementInfo> = Vec::new();
    let split = |text: &str| -> Option<(String, String, String)> {
        let (kind, rest) = text.split_once(':')?;
        let (name, figure) = rest.rsplit_once(':')?;
        Some((kind.to_owned(), name.to_owned(), figure.to_owned()))
    };
    let unit_of = |figure: &str, base: &str| -> Option<String> {
        let rest = figure.strip_prefix(base)?;
        if rest.is_empty() {
            return Some(String::new());
        }
        rest.strip_prefix(" (")?.strip_suffix(')').map(str::to_owned)
    };
    for text in &header {
        let Some((kind, name, figure)) = split(text) else {
            continue;
        };
        match kind.as_str() {
            "method" if !methods.contains(&name) => methods.push(name),
            "stockpile" if !stockpiles.contains(&name) => stockpiles.push(name),
            "element" => {
                if let Some(unit) = unit_of(&figure, "feed_grade") {
                    elements.push(ElementInfo {
                        name,
                        grade_unit: unit,
                        sales_unit: String::new(),
                    });
                } else if let (Some(unit), Some(element)) = (unit_of(&figure, "recovered"), elements.iter_mut().find(|element| element.name == name)) {
                    element.sales_unit = unit;
                }
            }
            _ => {}
        }
    }
    let group = |kind: &str, name: &str, figure: &str| header.iter().position(|text| split(text).is_some_and(|(k, n, f)| k == kind && n == name && f == figure));
    let method_columns: Vec<[usize; 2]> = methods
        .iter()
        .map(|method| {
            let at = |figure| group("method", method, figure).context("A method column is missing");
            Ok([at(METHOD_FIGURES[0])?, at(METHOD_FIGURES[1])?])
        })
        .collect::<Result<_>>()?;
    let stockpile_columns: Vec<[usize; 6]> = stockpiles
        .iter()
        .map(|stockpile| {
            let at = |figure| group("stockpile", stockpile, figure).context("A stockpile column is missing");
            Ok([
                at(STOCKPILE_FIGURES[0])?,
                at(STOCKPILE_FIGURES[1])?,
                at(STOCKPILE_FIGURES[2])?,
                at(STOCKPILE_FIGURES[3])?,
                at(STOCKPILE_FIGURES[4])?,
                at(STOCKPILE_FIGURES[5])?,
            ])
        })
        .collect::<Result<_>>()?;
    let element_columns: Vec<[usize; 2]> = elements
        .iter()
        .map(|element| {
            let [grade, recovered] = element_headers(element);
            Ok([column(&grade)?, column(&recovered)?])
        })
        .collect::<Result<_>>()?;

    let parse = |text: &str| -> Result<f64> {
        if text.trim().is_empty() {
            Ok(0.0)
        } else {
            text.trim().parse::<f64>().with_context(|| format!("'{text}' is not a number"))
        }
    };
    let destinations = std::iter::once(WASTE.to_owned()).chain(methods.iter().cloned()).collect::<Vec<_>>();
    let mut result = NpvScheduleResult {
        scenario: String::new(),
        name: String::new(),
        rate: 0.0,
        capex_initial: 0.0,
        lead: 0,
        shells: Vec::new(),
        destinations: destinations.clone(),
        elements: elements.clone(),
        stockpiles: stockpiles.clone(),
        pits: Vec::new(),
        order: None,
    };
    // Rows arrive grouped by pushback, then case, then period.
    let mut current: Option<(usize, PeriodCase)> = None;
    for record in reader.records() {
        let record = record?;
        let field = |index: usize| record.get(index).unwrap_or("");
        let pushback: usize = field(pit_columns[2]).trim().parse().context("A pushback number is not a whole number")?;
        let Some(&(case, _)) = CASES.iter().find(|(_, name)| *name == field(pit_columns[6])) else {
            bail!("'{}' is not a case", field(pit_columns[6]));
        };
        if pushback == 0 {
            bail!("Pushbacks are numbered from 1");
        }
        if current != Some((pushback, case)) {
            current = Some((pushback, case));
            result.scenario = field(pit_columns[0]).to_owned();
            result.name = field(pit_columns[1]).to_owned();
            result.rate = parse(field(pit_columns[7]))?;
            result.capex_initial = parse(field(pit_columns[8]))?;
            result.lead = field(pit_columns[15]).trim().parse().unwrap_or(0);
            while result.pits.len() < pushback {
                let distance = field(pit_columns[5]);
                result.shells.push(ShellInfo {
                    number: field(pit_columns[3]).trim().parse().context("A shell number is not a whole number")?,
                    factor: parse(field(pit_columns[4]))?,
                    distance: if distance.trim().is_empty() { None } else { Some(parse(distance)? / 100.0) },
                });
                result.pits.push(CandidatePit {
                    shell: result.shells.len() - 1,
                    ore: parse(field(pit_columns[12]))?,
                    waste: parse(field(pit_columns[13]))?,
                    benches: Vec::new(),
                    worst: NpvPitSchedule::default(),
                    best: NpvPitSchedule::default(),
                    lead: NpvPitSchedule::default(),
                });
            }
            let schedule = case_mut(&mut result.pits[pushback - 1], case);
            schedule.npv = parse(field(pit_columns[9]))?;
            schedule.unfinished = field(pit_columns[10]).trim() == "true";
            schedule.abandoned = parse(field(pit_columns[11]))?;
        }
        let period: usize = field(period_columns[0]).trim().parse().context("A period is not a whole number")?;
        if period == 0 {
            continue;
        }
        let value = |index: usize| parse(field(index));
        let mut figures = PeriodResult {
            held: match field(period_columns[1]) {
                "no_stockpile" => Some(Held::NoStockpile),
                "stockpiles_full" => Some(Held::StockpilesFull),
                _ => None,
            },
            mined: value(period_columns[2])?,
            waste: value(period_columns[3])?,
            stocked: value(period_columns[4])?,
            reclaimed: value(period_columns[5])?,
            revenue: value(period_columns[6])?,
            mining_cost: value(period_columns[7])?,
            processing_cost: value(period_columns[8])?,
            rehandle_cost: value(period_columns[9])?,
            fixed_cost: value(period_columns[10])?,
            capex: value(period_columns[11])?,
            cost: value(period_columns[12])?,
            cash_flow: value(period_columns[13])?,
            discounted_cash_flow: value(period_columns[14])?,
            mined_direct: vec![0.0; destinations.len()],
            processed: vec![0.0; destinations.len()],
            ..PeriodResult::default()
        };
        for (index, columns) in method_columns.iter().enumerate() {
            figures.mined_direct[index + 1] = value(columns[0])?;
            figures.processed[index + 1] = value(columns[1])?;
        }
        for columns in &stockpile_columns {
            let (stocked, reclaimed, balance) = (value(columns[0])?, value(columns[2])?, value(columns[4])?);
            figures.stocked_by.push(stocked);
            figures.stocked_grade_tonnes.push(value(columns[1])? * stocked);
            figures.reclaimed_by.push(reclaimed);
            figures.reclaimed_grade_tonnes.push(value(columns[3])? * reclaimed);
            figures.stockpile_balance.push(balance);
            figures.balance_grade_tonnes.push(value(columns[5])? * balance);
        }
        let processed = figures.processed_total();
        for columns in &element_columns {
            figures.feed_grade_tonnes.push(value(columns[0])? * processed);
            figures.recovered.push(value(columns[1])?);
        }
        let schedule = case_mut(&mut result.pits[pushback - 1], case);
        if schedule.periods.len() + 1 != period {
            bail!("The periods of pushback {pushback} are not in order");
        }
        schedule.periods.push(figures);
    }
    if result.pits.is_empty() {
        bail!("The file has no schedule rows");
    }
    Ok(result)
}

fn case_mut(pit: &mut CandidatePit, case: PeriodCase) -> &mut NpvPitSchedule {
    match case {
        PeriodCase::FixedLead => &mut pit.lead,
        PeriodCase::Best => &mut pit.best,
        PeriodCase::Worst => &mut pit.worst,
    }
}
