//! The schedule report: the calculated schedule as plain tables, grouped by
//! day, by week or over the whole schedule, for a spreadsheet.
//!
//! Exported from the Calendar, either copied (tab-separated, which a
//! spreadsheet pastes straight into columns) or saved as CSV. Every figure is
//! read off the published schedule - its deliveries, executions, idle spans
//! and balances - and apportioned to a period by how much of a span falls
//! inside it, the same way the Calendar's own per-day figures are. Numbers
//! are written plain, without separators or units, so a spreadsheet reads
//! them as numbers; the units are in the headings.
//!
//! This is the fixed, automatic export. A user-built reporting page is
//! planned separately.

use std::collections::BTreeMap;

use crate::{
    i18n::tr,
    model::schedule::{
        CalendarPeriod, DestinationId, DestinationKind, LoaderAgentId, SCHEDULE_PERIOD_H, SchedulePlan,
        cashflow::Activity,
        result::{CalculatedSchedule, IdleReason, WorkSource},
    },
    ui::{elements::schedule_gantt::idle_reason_text, state::ScheduleBarView},
};

/// How the report's rows are grouped in time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ReportGrouping {
    #[default]
    Day,
    /// Seven schedule days at a time, from Day 1: days 1-7, 8-14, and so on.
    Week,
    /// One row per item over everything calculated.
    Whole,
}

impl ReportGrouping {
    pub(crate) const ALL: [Self; 3] = [Self::Day, Self::Week, Self::Whole];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Day => tr!("report-group-day"),
            Self::Week => tr!("report-group-week"),
            Self::Whole => tr!("report-group-whole"),
        }
    }
}

/// One table of the report.
pub(crate) struct ReportTable {
    title: String,
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// What the report needs that the schedule does not hold: names.
pub(crate) struct ReportNames<'a> {
    pub(crate) destinations: Vec<(DestinationId, String, DestinationKind)>,
    /// Aligned with the schedule's tracked grades.
    pub(crate) grades: Vec<String>,
    pub(crate) bar_views: &'a [ScheduleBarView],
}

/// One period of the report: its label and the hours it covers, held inside
/// what was calculated.
struct Group {
    label: String,
    first_day: u32,
    /// Exclusive.
    end_day: u32,
    start_h: f64,
    end_h: f64,
}

fn groups(grouping: ReportGrouping, schedule: &CalculatedSchedule) -> Vec<Group> {
    let end_h = schedule.requested_end_h.max(0.0);
    let days = schedule.periods.covered_periods();
    let make = |label: String, first_day: u32, end_day: u32| Group {
        label,
        first_day,
        end_day,
        start_h: f64::from(first_day) * SCHEDULE_PERIOD_H,
        end_h: (f64::from(end_day) * SCHEDULE_PERIOD_H).min(end_h),
    };
    match grouping {
        ReportGrouping::Day => (0..days).map(|day| make(tr!("report-day", day = (day + 1).to_string()), day, day + 1)).collect(),
        ReportGrouping::Week => (0..days.div_ceil(7))
            .map(|week| {
                let first = week * 7;
                let end = (first + 7).min(days);
                make(
                    tr!("report-week", week = (week + 1).to_string(), first = (first + 1).to_string(), last = end.to_string()),
                    first,
                    end,
                )
            })
            .collect(),
        ReportGrouping::Whole if days > 0 => vec![make(tr!("report-days", first = "1".to_owned(), last = days.to_string()), 0, days)],
        ReportGrouping::Whole => Vec::new(),
    }
}

fn overlap(start_h: f64, end_h: f64, group: &Group) -> f64 {
    (end_h.min(group.end_h) - start_h.max(group.start_h)).max(0.0)
}

fn tonnes(value: f64) -> String {
    plain(value, 1)
}

fn hours(value: f64) -> String {
    plain(value, 2)
}

fn money(value: f64) -> String {
    plain(value, 2)
}

/// A number with at most `decimals` places, trailing zeros dropped, and
/// never a negative zero.
fn plain(value: f64, decimals: usize) -> String {
    let mut text = format!("{value:.decimals$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" { "0".to_owned() } else { text }
}

fn grade(contained: f64, tonnes: f64) -> String {
    if tonnes > 1e-6 { plain(contained / tonnes, 4) } else { String::new() }
}

/// Build every table of the report.
pub(crate) fn build(schedule: &CalculatedSchedule, plan: &SchedulePlan, names: &ReportNames<'_>, grouping: ReportGrouping) -> Vec<ReportTable> {
    let groups = groups(grouping, schedule);
    let destination_name = |id: DestinationId| {
        names
            .destinations
            .iter()
            .find(|(entry, _, _)| *entry == id)
            .map_or_else(|| tr!("destination-unresolved"), |(_, name, _)| name.clone())
    };
    let loader_name = |id: LoaderAgentId| plan.agent(id).map_or_else(|| id.0.to_string(), |agent| agent.name.clone());
    let currency = plan.currency().to_owned();
    let grade_headers: Vec<String> = names.grades.iter().map(|name| tr!("report-grade", grade = name.clone())).collect();

    let mut tables = Vec::new();
    let mut movements = movements(schedule, names, &groups, &grade_headers, &currency, &destination_name, &loader_name);
    let first = 7 + grade_headers.len();
    let mut header = movements.header[..6].to_vec();
    header.extend(movements.header[first..first + 9].iter().cloned());
    let rows = movements
        .rows
        .iter()
        .map(|row| {
            let mut cells = row[..6].to_vec();
            cells.extend(row[first..first + 9].iter().cloned());
            cells
        })
        .collect();
    let haulage = ReportTable {
        title: tr!("haul-table"),
        header,
        rows,
    };
    movements.header.drain(first..first + 9);
    for row in &mut movements.rows {
        row.drain(first..first + 9);
    }
    tables.push(movements);
    tables.push(haulage);
    tables.push(loaders(schedule, plan, &groups, &loader_name));
    let of_kind = |kind: DestinationKind| {
        names
            .destinations
            .iter()
            .filter(move |(_, _, entry)| *entry == kind)
            .map(|(id, name, _)| (*id, name.clone()))
    };

    // Stockpiles: opening and closing at the period's bounds, and what moved
    // in and out of it between.
    let piles: Vec<_> = of_kind(DestinationKind::Stockpile).collect();
    if !piles.is_empty() {
        let mut header = vec![
            tr!("report-period"),
            tr!("report-stockpile"),
            tr!("report-opening-t"),
            tr!("report-received-t"),
            tr!("report-reclaimed-t"),
            tr!("report-closing-t"),
        ];
        header.extend(grade_headers.iter().map(|name| tr!("report-closing-grade", grade = name.clone())));
        let mut rows = Vec::new();
        for group in &groups {
            for (pile, name) in &piles {
                let (opening, _) = schedule.inventory_at(*pile, group.start_h).unwrap_or((0.0, Vec::new()));
                let (closing, contained) = schedule.inventory_at(*pile, group.end_h).unwrap_or((0.0, Vec::new()));
                let (received, _) = schedule.received_between(*pile, group.start_h, group.end_h);
                let reclaimed: f64 = schedule
                    .deliveries
                    .iter()
                    .filter(|delivery| delivery.source == WorkSource::Stockpile(*pile) && delivery.end_h > delivery.start_h)
                    .map(|delivery| delivery.tonnes * overlap(delivery.start_h, delivery.end_h, group) / (delivery.end_h - delivery.start_h))
                    .sum();
                let mut row = vec![group.label.clone(), name.clone(), tonnes(opening), tonnes(received), tonnes(reclaimed), tonnes(closing)];
                row.extend((0..names.grades.len()).map(|index| grade(contained.get(index).copied().unwrap_or(0.0), closing)));
                rows.push(row);
            }
        }
        tables.push(ReportTable {
            title: tr!("report-stockpiles"),
            header,
            rows,
        });
    }

    // Crushers: what they processed, against the period's budget, and what
    // the grade targets charged for the days in it.
    let crushers: Vec<_> = of_kind(DestinationKind::Crusher).collect();
    if !crushers.is_empty() {
        let mut header = vec![tr!("report-period"), tr!("report-crusher"), tr!("report-processed-t"), tr!("report-limit-t")];
        header.extend(grade_headers.iter().cloned());
        header.push(tr!("report-penalty", currency = currency.clone()));
        let mut rows = Vec::new();
        for group in &groups {
            for (crusher, name) in &crushers {
                let (received, contained) = schedule.received_between(*crusher, group.start_h, group.end_h);
                // Unlimited on any day means unlimited over the period.
                let limit: Option<f64> = match crusher {
                    DestinationId::Standalone(id) => plan
                        .routing()
                        .crusher(*id)
                        .and_then(|calendar| (group.first_day..group.end_day).map(|day| calendar.limit_at(CalendarPeriod(day))).sum::<Option<f64>>()),
                    _ => None,
                };
                let penalty: f64 = schedule
                    .grade_targets
                    .iter()
                    .filter(|target| target.specification.destination == *crusher && (group.first_day..group.end_day).contains(&target.period))
                    .map(|target| target.penalty)
                    .sum();
                let mut row = vec![group.label.clone(), name.clone(), tonnes(received), limit.map(tonnes).unwrap_or_default()];
                row.extend((0..names.grades.len()).map(|index| grade(contained.get(index).copied().unwrap_or(0.0), received)));
                row.push(money(penalty));
                rows.push(row);
            }
        }
        tables.push(ReportTable {
            title: tr!("report-crushers"),
            header,
            rows,
        });
    }

    let dumps: Vec<_> = of_kind(DestinationKind::Dump).collect();
    if !dumps.is_empty() {
        let mut rows = Vec::new();
        for group in &groups {
            for (dump, name) in &dumps {
                let (received, _) = schedule.received_between(*dump, group.start_h, group.end_h);
                let (to_date, _) = schedule.received_between(*dump, 0.0, group.end_h);
                rows.push(vec![group.label.clone(), name.clone(), tonnes(received), tonnes(to_date)]);
            }
        }
        tables.push(ReportTable {
            title: tr!("report-dumps"),
            header: vec![tr!("report-period"), tr!("report-dump"), tr!("report-received-t"), tr!("report-to-date-t")],
            rows,
        });
    }

    // Trucks: hours hauling, against the hours the fleet had - units after
    // availability and utilisation, over the hours of each day calculated.
    if !plan.trucks().classes.is_empty() {
        let mut rows = Vec::new();
        for group in &groups {
            for class in &plan.trucks().classes {
                let used: f64 = schedule
                    .deliveries
                    .iter()
                    .filter(|delivery| delivery.truck == class.id && delivery.end_h > delivery.start_h)
                    .map(|delivery| delivery.truck_hours * overlap(delivery.start_h, delivery.end_h, group) / (delivery.end_h - delivery.start_h))
                    .sum();
                let available: f64 = (group.first_day..group.end_day)
                    .map(|day| {
                        let from = f64::from(day) * SCHEDULE_PERIOD_H;
                        let to = (from + SCHEDULE_PERIOD_H).min(group.end_h);
                        class.calendar.values_at(CalendarPeriod(day)).effective_units() * (to - from).max(0.0)
                    })
                    .sum();
                let mut haul = crate::model::schedule::result::HaulSummary::default();
                for d in schedule.deliveries.iter().filter(|d| d.truck == class.id && d.end_h > d.start_h) {
                    haul.add(d, overlap(d.start_h, d.end_h, group) / (d.end_h - d.start_h));
                }
                rows.push(vec![
                    group.label.clone(),
                    class.name.clone(),
                    hours(used),
                    hours(available),
                    hours(haul.cycle_minutes()),
                    hours(haul.distance_km()),
                    hours(haul.average(haul.rise_t_m)),
                    hours(haul.loaded_t_km),
                ]);
            }
        }
        tables.push(ReportTable {
            title: tr!("report-trucks"),
            header: vec![
                tr!("report-period"),
                tr!("report-truck-class"),
                tr!("report-truck-hours-used"),
                tr!("report-truck-hours-available"),
                tr!("haul-cycle"),
                tr!("haul-distance"),
                tr!("haul-rise"),
                tr!("haul-tonne-km"),
            ],
            rows,
        });
    }
    tables
}

/// Movements: who moved what from where to where, one row per period,
/// loader, source and destination. A block is named by its dig area, so a
/// day's movements read as a handful of rows rather than one per block.
fn movements(
    schedule: &CalculatedSchedule,
    names: &ReportNames<'_>,
    groups: &[Group],
    grade_headers: &[String],
    currency: &str,
    destination_name: &dyn Fn(DestinationId) -> String,
    loader_name: &dyn Fn(LoaderAgentId) -> String,
) -> ReportTable {
    #[derive(Default)]
    struct Moved {
        tonnes: f64,
        contained: Vec<f64>,
        truck_hours: f64,
        value: f64,
        haul: crate::model::schedule::result::HaulSummary,
    }
    let source_name = |delivery: &crate::model::schedule::result::Delivery| match delivery.source {
        WorkSource::Stockpile(pile) => destination_name(pile),
        WorkSource::Block(block) => {
            let member = schedule
                .bar_blocks
                .iter()
                .find(|(bar, _)| *bar == delivery.bar)
                .and_then(|(_, blocks)| blocks.iter().position(|held| *held == block))
                .and_then(|position| names.bar_views.iter().find(|view| view.bar == delivery.bar)?.members.get(position));
            member
                .and_then(|member| member.area.clone().or_else(|| member.name.clone()))
                .unwrap_or_else(|| tr!("schedule-dispatch-block", block = block.0.to_string()))
        }
    };
    // Keyed by names so the rows sort as they read.
    let mut moved: BTreeMap<(usize, String, bool, String, String), Moved> = BTreeMap::new();
    for delivery in &schedule.deliveries {
        let duration = delivery.end_h - delivery.start_h;
        if duration <= 0.0 {
            continue;
        }
        for (index, group) in groups.iter().enumerate() {
            let inside = overlap(delivery.start_h, delivery.end_h, group);
            if inside <= 0.0 {
                continue;
            }
            let share = inside / duration;
            let key = (
                index,
                loader_name(delivery.agent),
                delivery.activity == Activity::Reclaim,
                source_name(delivery),
                destination_name(delivery.destination),
            );
            let entry = moved.entry(key).or_default();
            entry.tonnes += delivery.tonnes * share;
            entry.contained.resize(entry.contained.len().max(delivery.contained.len()), 0.0);
            for (total, quantity) in entry.contained.iter_mut().zip(&delivery.contained) {
                *total += quantity * share;
            }
            entry.truck_hours += delivery.truck_hours * share;
            entry.haul.add(delivery, share);
            entry.value += delivery.value * share;
        }
    }
    let mut header = vec![
        tr!("report-period"),
        tr!("report-loader"),
        tr!("report-activity"),
        tr!("report-source"),
        tr!("report-destination"),
        tr!("report-tonnes"),
    ];
    header.extend(grade_headers.iter().cloned());
    header.push(tr!("report-truck-hours-used"));
    header.extend([
        tr!("haul-spot-min"),
        tr!("haul-load-min"),
        tr!("haul-loaded-min"),
        tr!("haul-dump-min"),
        tr!("haul-return-min"),
        tr!("haul-cycle"),
        tr!("haul-distance"),
        tr!("haul-rise"),
        tr!("haul-tonne-km"),
    ]);
    header.push(tr!("report-value", currency = currency.to_owned()));
    let rows = moved
        .into_iter()
        .map(|((index, loader, reclaim, source, destination), entry)| {
            let mut row = vec![
                groups[index].label.clone(),
                loader,
                if reclaim { tr!("report-reclaim") } else { tr!("report-dig") },
                source,
                destination,
                tonnes(entry.tonnes),
            ];
            row.extend((0..grade_headers.len()).map(|grade_index| grade(entry.contained.get(grade_index).copied().unwrap_or(0.0), entry.tonnes)));
            row.push(hours(entry.truck_hours));
            let h = entry.haul;
            row.extend(
                [
                    h.average(h.spot_t_h) * 60.0,
                    h.average(h.load_t_h) * 60.0,
                    h.average(h.loaded_t_h) * 60.0,
                    h.average(h.dump_t_h) * 60.0,
                    h.average(h.empty_t_h) * 60.0,
                    h.cycle_minutes(),
                    h.distance_km(),
                    h.average(h.rise_t_m),
                    h.loaded_t_km,
                ]
                .into_iter()
                .map(hours),
            );
            row.push(money(entry.value));
            row
        })
        .collect();
    ReportTable {
        title: tr!("report-movements"),
        header,
        rows,
    }
}

/// Loader time: tonnes, and where each machine's hours went - digging,
/// reclaiming, delayed, and idle by the reason the schedule gives. A machine
/// the calculation never reached had no work for the whole period.
fn loaders(schedule: &CalculatedSchedule, plan: &SchedulePlan, groups: &[Group], loader_name: &dyn Fn(LoaderAgentId) -> String) -> ReportTable {
    const REASONS: [IdleReason; 8] = [
        IdleReason::Unavailable,
        IdleReason::NoWork,
        IdleReason::WorkFinished,
        IdleReason::NoRoute,
        IdleReason::DestinationsFull,
        IdleReason::PileMode,
        IdleReason::NoTrucks,
        IdleReason::NotWorthIt,
    ];
    struct Time {
        dug: f64,
        reclaimed: f64,
        dig_h: f64,
        reclaim_h: f64,
        delay_h: f64,
        idle: [f64; REASONS.len()],
    }
    let mut table: Vec<(String, String, Time)> = Vec::new();
    let mut used = [false; REASONS.len()];
    for group in groups {
        for agent in plan.agents() {
            let mut time = Time {
                dug: 0.0,
                reclaimed: 0.0,
                dig_h: 0.0,
                reclaim_h: 0.0,
                delay_h: 0.0,
                idle: [0.0; REASONS.len()],
            };
            // Tonnes add span by span; hours are the time covered, because a
            // machine working two blocks in one interval has two spans over
            // the same hours.
            let mut spans: [Vec<(f64, f64)>; 2] = [Vec::new(), Vec::new()];
            for execution in schedule.executions.iter().filter(|execution| execution.agent == agent.id) {
                let inside = overlap(execution.start_h, execution.end_h, group);
                if inside <= 0.0 {
                    continue;
                }
                let share = inside / (execution.end_h - execution.start_h);
                let index = match execution.activity {
                    Activity::Dig => {
                        time.dug += execution.tonnes * share;
                        0
                    }
                    Activity::Reclaim => {
                        time.reclaimed += execution.tonnes * share;
                        1
                    }
                };
                spans[index].push((execution.start_h.max(group.start_h), execution.end_h.min(group.end_h)));
            }
            time.dig_h = covered(&mut spans[0]);
            time.reclaim_h = covered(&mut spans[1]);
            for span in schedule.idle.iter().filter(|span| span.agent == agent.id) {
                let inside = overlap(span.start_h, span.end_h, group);
                match span.reason {
                    Some(IdleReason::Delayed) => time.delay_h += inside,
                    Some(reason) => {
                        if let Some(index) = REASONS.iter().position(|entry| *entry == reason) {
                            time.idle[index] += inside;
                        }
                    }
                    None => {}
                }
            }
            // What the result says nothing about - a machine with no bars
            // never reached the calculation - had no work.
            let accounted = time.dig_h + time.reclaim_h + time.delay_h + time.idle.iter().sum::<f64>();
            let left = (group.end_h - group.start_h) - accounted;
            if left > 1e-6
                && let Some(index) = REASONS.iter().position(|reason| *reason == IdleReason::NoWork)
            {
                time.idle[index] += left;
            }
            for (flag, value) in used.iter_mut().zip(time.idle) {
                *flag |= value > 1e-6;
            }
            table.push((group.label.clone(), loader_name(agent.id), time));
        }
    }
    let mut header = vec![
        tr!("report-period"),
        tr!("report-loader"),
        tr!("report-dug-t"),
        tr!("report-reclaimed-t"),
        tr!("report-dig-h"),
        tr!("report-reclaim-h"),
        tr!("report-delay-h"),
        tr!("report-idle-h"),
    ];
    for (reason, used) in REASONS.iter().zip(used) {
        if used {
            header.push(tr!("report-idle-reason-h", reason = idle_reason_text(Some(*reason)).0));
        }
    }
    let rows = table
        .into_iter()
        .map(|(period, loader, time)| {
            let mut row = vec![
                period,
                loader,
                tonnes(time.dug),
                tonnes(time.reclaimed),
                hours(time.dig_h),
                hours(time.reclaim_h),
                hours(time.delay_h),
                hours(time.idle.iter().sum()),
            ];
            row.extend(time.idle.iter().zip(used).filter(|(_, used)| *used).map(|(value, _)| hours(*value)));
            row
        })
        .collect();
    ReportTable {
        title: tr!("report-loaders"),
        header,
        rows,
    }
}

/// The hours a set of spans covers, overlaps counted once.
fn covered(spans: &mut [(f64, f64)]) -> f64 {
    spans.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut total = 0.0;
    let mut reach = f64::NEG_INFINITY;
    for &(start, end) in spans.iter() {
        let from = start.max(reach);
        if end > from {
            total += end - from;
        }
        reach = reach.max(end);
    }
    total
}

/// The report as text: each table under its title, a blank line between.
/// `','` writes CSV, quoting where a field needs it; `'\t'` writes what a
/// spreadsheet pastes into columns.
pub(crate) fn to_text(tables: &[ReportTable], heading: &str, separator: char) -> String {
    let field = |text: &str| -> String {
        // The translations wrap each value they place in invisible direction
        // marks, which a spreadsheet would keep inside the cell.
        let text = &text.replace(['\u{2068}', '\u{2069}'], "");
        if separator == ',' {
            if text.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", text.replace('"', "\"\""))
            } else {
                text.to_owned()
            }
        } else {
            text.replace(['\t', '\n', '\r'], " ")
        }
    };
    let line = |cells: &[String]| cells.iter().map(|cell| field(cell)).collect::<Vec<_>>().join(&separator.to_string());
    let mut out = String::new();
    out.push_str(&field(heading));
    out.push('\n');
    for table in tables {
        out.push('\n');
        out.push_str(&field(&table.title));
        out.push('\n');
        out.push_str(&line(&table.header));
        out.push('\n');
        for row in &table.rows {
            out.push_str(&line(row));
            out.push('\n');
        }
    }
    out
}
