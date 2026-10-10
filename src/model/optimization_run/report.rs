//! The optimization report: per shell and destination, what mining it moves,
//! what it costs and what it earns.
//!
//! Every shell is reported at the base price (revenue factor 1), with each
//! block sent where it pays best at that price - Whittle's pit-by-pit basis -
//! so shells compare like for like whatever factor found them. One pass over
//! the cells puts each block in the bucket of the shell that first holds it
//! (the increment, or pushback); a shell's pit is the sum of the increments up
//! to it.
//!
//! The CSV is tidy: one row per basis (cumulative pit / incremental pushback),
//! shell and destination, plus a `Total` row per shell. Its headers are fixed
//! English names so spreadsheets and scripts can rely on them.

use std::io::Read;

use anyhow::{Context, Result, bail};
use rayon::prelude::*;

use super::{
    grid::{Grid, NO_BLOCK},
    prepare::{Economics, finite_or_zero},
    shells::{OUTSIDE, Shells},
    values::{BlockValues, block_base, route_costs},
};
use crate::app::jobs::CancelFlag;

/// Name of the report file in the scenario's reports folder.
pub(crate) const FILE_NAME: &str = "optimization_results.csv";
/// The destination of blocks no method takes at the base price.
pub(crate) const WASTE: &str = "Waste";
const TOTAL: &str = "Total";
const CUMULATIVE: &str = "cumulative";
const INCREMENTAL: &str = "incremental";

/// One shell as the report names it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShellInfo {
    /// 1-based.
    pub(crate) number: usize,
    pub(crate) factor: f64,
    /// A directional shell's share of the distance across the final pit.
    pub(crate) distance: Option<f64>,
}

/// One element the methods recover, with its units.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ElementInfo {
    pub(crate) name: String,
    pub(crate) grade_unit: String,
    pub(crate) sales_unit: String,
}

/// One element's sums in a bucket.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ElementSums {
    /// Tonnes times grade, for the average grade.
    pub(crate) grade_tonnes: f64,
    /// Sales units in the rock.
    pub(crate) contained: f64,
    /// Sales units recovered (ore only).
    pub(crate) recovered: f64,
    /// Net of selling costs, at the base price.
    pub(crate) revenue: f64,
    pub(crate) cost: f64,
}

impl ElementSums {
    fn add(&mut self, other: &Self) {
        self.grade_tonnes += other.grade_tonnes;
        self.contained += other.contained;
        self.recovered += other.recovered;
        self.revenue += other.revenue;
        self.cost += other.cost;
    }
}

/// What one shell sends to one destination.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Bucket {
    pub(crate) blocks: u64,
    pub(crate) tonnes: f64,
    pub(crate) mining: f64,
    pub(crate) waste_haulage: f64,
    pub(crate) rehab: f64,
    pub(crate) ore_haulage: f64,
    pub(crate) processing: f64,
    pub(crate) ga: f64,
    /// One per [`OptimizationReport::elements`].
    pub(crate) elements: Vec<ElementSums>,
}

impl Bucket {
    fn empty(elements: usize) -> Self {
        Self {
            elements: vec![ElementSums::default(); elements],
            ..Self::default()
        }
    }

    pub(crate) fn add(&mut self, other: &Self) {
        self.blocks += other.blocks;
        self.tonnes += other.tonnes;
        self.mining += other.mining;
        self.waste_haulage += other.waste_haulage;
        self.rehab += other.rehab;
        self.ore_haulage += other.ore_haulage;
        self.processing += other.processing;
        self.ga += other.ga;
        for (sums, other) in self.elements.iter_mut().zip(&other.elements) {
            sums.add(other);
        }
    }

    pub(crate) fn element_cost(&self) -> f64 {
        self.elements.iter().map(|sums| sums.cost).sum()
    }

    pub(crate) fn revenue(&self) -> f64 {
        self.elements.iter().map(|sums| sums.revenue).sum()
    }

    pub(crate) fn total_cost(&self) -> f64 {
        self.mining + self.waste_haulage + self.rehab + self.ore_haulage + self.processing + self.ga + self.element_cost()
    }

    /// Revenue less every cost, undiscounted.
    pub(crate) fn cash_flow(&self) -> f64 {
        self.revenue() - self.total_cost()
    }

    /// Average grade of element `index` in the grade unit, or `None` with no tonnes.
    pub(crate) fn grade(&self, index: usize) -> Option<f64> {
        (self.tonnes > 0.0).then(|| self.elements[index].grade_tonnes / self.tonnes)
    }
}

/// Cumulative pit or incremental pushback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Basis {
    #[default]
    Cumulative,
    Incremental,
}

/// A run's report. Small: shells x destinations x elements numbers.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OptimizationReport {
    pub(crate) scenario: String,
    pub(crate) shells: Vec<ShellInfo>,
    /// The first is always [`WASTE`]; then each processing method, rows of the
    /// methods grid sharing a name being one.
    pub(crate) destinations: Vec<String>,
    /// The first is the main quality field.
    pub(crate) elements: Vec<ElementInfo>,
    /// `[shell][destination]`, each shell's increment over the one before.
    pub(crate) incremental: Vec<Vec<Bucket>>,
}

impl OptimizationReport {
    /// Shell `shell`'s (0-based) bucket for `destination`, or the sum over all
    /// destinations when `None`.
    pub(crate) fn bucket(&self, basis: Basis, shell: usize, destination: Option<usize>) -> Bucket {
        let after = match basis {
            Basis::Cumulative => None,
            Basis::Incremental => shell.checked_sub(1),
        };
        self.bucket_since(after, shell, destination)
    }

    /// What shell `shell` (0-based) adds over shell `after` (`None` = from
    /// nothing, the whole pit), for one destination or all (`None`). With
    /// `after` the shell before, this is the run's increment; with an earlier
    /// shell, the pushback between two chosen shells.
    pub(crate) fn bucket_since(&self, after: Option<usize>, shell: usize, destination: Option<usize>) -> Bucket {
        let mut sum = Bucket::empty(self.elements.len());
        for increment in &self.incremental[after.map_or(0, |after| after + 1)..=shell] {
            match destination {
                Some(destination) => sum.add(&increment[destination]),
                None => increment.iter().for_each(|bucket| sum.add(bucket)),
            }
        }
        sum
    }

    /// Ore and waste tonnes of a shell (a destination other than waste is ore).
    pub(crate) fn ore_waste(&self, basis: Basis, shell: usize) -> (f64, f64) {
        let waste = self.bucket(basis, shell, Some(0)).tonnes;
        (self.bucket(basis, shell, None).tonnes - waste, waste)
    }
}

/// Waste tonnes per ore tonne, or `None` without ore.
pub(crate) fn strip_ratio(ore: f64, waste: f64) -> Option<f64> {
    (ore > 0.0).then(|| waste / ore)
}

/// Build the report from a finished run. Runs on the worker while the values
/// and the block lookup are still there.
pub(crate) fn build(
    scenario: &str,
    economics: &Economics,
    grid: &Grid,
    block_of_cell: &[u32],
    values: &BlockValues,
    shells: &Shells,
    cancel: &CancelFlag,
) -> Result<OptimizationReport> {
    // Destinations: waste, then each method name once.
    let mut destinations = vec![WASTE.to_owned()];
    let method_destination: Vec<usize> = economics
        .methods
        .iter()
        .map(|method| match destinations.iter().skip(1).position(|name| *name == method.name) {
            Some(index) => index + 1,
            None => {
                destinations.push(method.name.clone());
                destinations.len() - 1
            }
        })
        .collect();
    let elements = economics.elements.len();
    let shell_count = shells.summaries.len();
    let volume = grid.cell.x * grid.cell.y * grid.cell.z;
    let empty = || vec![vec![Bucket::empty(elements); destinations.len()]; shell_count];

    let incremental = (0..grid.cell_count())
        .into_par_iter()
        .with_min_len(1 << 14)
        .try_fold(empty, |mut buckets, cell| {
            if cell % (1 << 16) == 0 && cancel.is_cancelled() {
                bail!("Cancelled");
            }
            let label = shells.label[cell];
            let block = block_of_cell[cell];
            if label == OUTSIDE || block == NO_BLOCK || values.air[cell] {
                return Ok(buckets);
            }
            let block = block as usize;
            let base = block_base(economics, volume, block);
            let method = values.destination(cell, 1.0).map(usize::from);
            let destination = method.map_or(0, |method| method_destination[method]);
            let bucket = &mut buckets[usize::from(label) - 1][destination];
            bucket.blocks += 1;
            bucket.tonnes += base.tonnes;
            bucket.mining += base.mining;
            for (sums, element) in bucket.elements.iter_mut().zip(&economics.elements) {
                let grade = finite_or_zero(element.values[block]);
                sums.grade_tonnes += base.tonnes * grade;
                sums.contained += base.tonnes * grade * element.factor;
            }
            match method {
                None => {
                    bucket.waste_haulage += base.waste_haulage;
                    bucket.rehab += base.rehab;
                }
                Some(method) => {
                    let costs = route_costs(economics, method, block, &base, |index, take| {
                        let sums = &mut bucket.elements[index];
                        sums.recovered += take.recovered;
                        sums.revenue += take.revenue;
                        sums.cost += take.cost;
                    })
                    .expect("the destination takes the block");
                    bucket.ore_haulage += costs.ore_haulage;
                    bucket.processing += costs.processing;
                    bucket.ga += costs.ga;
                }
            }
            Ok(buckets)
        })
        .try_reduce(empty, |mut a, b| {
            for (shell_a, shell_b) in a.iter_mut().zip(&b) {
                for (bucket_a, bucket_b) in shell_a.iter_mut().zip(shell_b) {
                    bucket_a.add(bucket_b);
                }
            }
            Ok(a)
        })?;

    Ok(OptimizationReport {
        scenario: scenario.to_owned(),
        shells: shells
            .summaries
            .iter()
            .enumerate()
            .map(|(index, summary)| ShellInfo {
                number: index + 1,
                factor: summary.factor,
                distance: summary.distance,
            })
            .collect(),
        destinations,
        elements: economics
            .elements
            .iter()
            .map(|element| ElementInfo {
                name: element.name.clone(),
                grade_unit: element.grade_unit.clone(),
                sales_unit: element.sales_unit.clone(),
            })
            .collect(),
        incremental,
    })
}

// ── CSV ──

const LEAD: [&str; 10] = [
    "scenario",
    "basis",
    "shell",
    "raf",
    "dir_pct",
    "destination",
    "blocks",
    "tonnes",
    "ore_tonnes",
    "waste_tonnes",
];
const COSTS: [&str; 10] = [
    "mining_cost",
    "waste_haulage_cost",
    "rehab_cost",
    "ore_haulage_cost",
    "processing_cost",
    "ga_cost",
    "element_cost",
    "total_cost",
    "revenue",
    "cash_flow",
];
const STRIP_RATIO: &str = "strip_ratio";

fn with_unit(label: String, unit: &str) -> String {
    if unit.is_empty() { label } else { format!("{label} ({unit})") }
}

fn element_headers(element: &ElementInfo) -> [String; 5] {
    let name = &element.name;
    [
        with_unit(format!("{name} grade"), &element.grade_unit),
        with_unit(format!("{name} contained"), &element.sales_unit),
        with_unit(format!("{name} recovered"), &element.sales_unit),
        format!("{name} revenue"),
        format!("{name} cost"),
    ]
}

fn number(value: f64) -> String {
    if value.is_finite() { format!("{value}") } else { String::new() }
}

/// The report as CSV bytes.
pub(crate) fn write_csv(report: &OptimizationReport) -> Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    let mut header: Vec<String> = LEAD.iter().map(|name| (*name).to_owned()).collect();
    header.push(STRIP_RATIO.to_owned());
    for element in &report.elements {
        header.extend(element_headers(element));
    }
    header.extend(COSTS.iter().map(|name| (*name).to_owned()));
    writer.write_record(&header)?;

    for (basis, basis_name) in [(Basis::Cumulative, CUMULATIVE), (Basis::Incremental, INCREMENTAL)] {
        for (index, shell) in report.shells.iter().enumerate() {
            let rows = (0..report.destinations.len()).map(Some).chain([None]);
            for destination in rows {
                let bucket = report.bucket(basis, index, destination);
                let (ore, waste) = match destination {
                    Some(0) => (0.0, bucket.tonnes),
                    Some(_) => (bucket.tonnes, 0.0),
                    None => report.ore_waste(basis, index),
                };
                let mut record = vec![
                    report.scenario.clone(),
                    basis_name.to_owned(),
                    shell.number.to_string(),
                    number(shell.factor),
                    shell.distance.map_or(String::new(), |share| number(share * 100.0)),
                    destination.map_or(TOTAL.to_owned(), |destination| report.destinations[destination].clone()),
                    bucket.blocks.to_string(),
                    number(bucket.tonnes),
                    number(ore),
                    number(waste),
                    match destination {
                        None => strip_ratio(ore, waste).map_or(String::new(), number),
                        Some(_) => String::new(),
                    },
                ];
                for (element, sums) in bucket.elements.iter().enumerate() {
                    record.push(bucket.grade(element).map_or(String::new(), number));
                    record.extend([sums.contained, sums.recovered, sums.revenue, sums.cost].map(number));
                }
                record.extend(
                    [
                        bucket.mining,
                        bucket.waste_haulage,
                        bucket.rehab,
                        bucket.ore_haulage,
                        bucket.processing,
                        bucket.ga,
                        bucket.element_cost(),
                        bucket.total_cost(),
                        bucket.revenue(),
                        bucket.cash_flow(),
                    ]
                    .map(number),
                );
                writer.write_record(&record)?;
            }
        }
    }
    writer.into_inner().context("Could not finish the report")
}

/// Read a report written by [`write_csv`] back, from its incremental rows.
pub(crate) fn read_csv(reader: impl Read) -> Result<OptimizationReport> {
    let mut reader = csv::Reader::from_reader(reader);
    let header: Vec<String> = reader.headers()?.iter().map(str::to_owned).collect();
    let column = |name: &str| header.iter().position(|column| column == name).with_context(|| format!("The file has no '{name}' column"));
    let lead: Vec<usize> = LEAD.iter().map(|name| column(name)).collect::<Result<_>>()?;
    let costs: Vec<usize> = COSTS.iter().map(|name| column(name)).collect::<Result<_>>()?;
    let strip = column(STRIP_RATIO)?;

    // Elements sit between the strip ratio and the costs, five columns each.
    let element_columns = &header[strip + 1..costs[0]];
    if !element_columns.len().is_multiple_of(5) {
        bail!("The element columns of the file are not in fives");
    }
    let unit_of = |header: &str, prefix: &str| -> Option<String> {
        let rest = header.strip_prefix(prefix)?;
        if rest.is_empty() {
            return Some(String::new());
        }
        rest.strip_prefix(" (")?.strip_suffix(')').map(str::to_owned)
    };
    let mut elements = Vec::new();
    for group in element_columns.chunks(5) {
        let name = group[3].strip_suffix(" revenue").context("An element column group does not end in revenue and cost")?;
        let grade_unit = unit_of(&group[0], &format!("{name} grade")).context("An element grade column is not named as expected")?;
        let sales_unit = unit_of(&group[1], &format!("{name} contained")).context("An element contained column is not named as expected")?;
        elements.push(ElementInfo {
            name: name.to_owned(),
            grade_unit,
            sales_unit,
        });
    }

    let parse = |text: &str| -> Result<f64> {
        if text.trim().is_empty() {
            Ok(0.0)
        } else {
            text.trim().parse::<f64>().with_context(|| format!("'{text}' is not a number"))
        }
    };
    let mut scenario = String::new();
    let mut shells: Vec<ShellInfo> = Vec::new();
    let mut destinations: Vec<String> = Vec::new();
    let mut rows: Vec<(usize, usize, Bucket)> = Vec::new();
    for record in reader.records() {
        let record = record?;
        let field = |index: usize| record.get(index).unwrap_or("");
        if field(lead[1]) != INCREMENTAL || field(lead[5]) == TOTAL {
            continue;
        }
        scenario = field(lead[0]).to_owned();
        let number: usize = field(lead[2]).trim().parse().context("A shell number is not a whole number")?;
        let shell = match shells.iter().position(|shell| shell.number == number) {
            Some(index) => index,
            None => {
                let distance = field(lead[4]);
                shells.push(ShellInfo {
                    number,
                    factor: parse(field(lead[3]))?,
                    distance: if distance.trim().is_empty() { None } else { Some(parse(distance)? / 100.0) },
                });
                shells.len() - 1
            }
        };
        let name = field(lead[5]).to_owned();
        let destination = match destinations.iter().position(|known| *known == name) {
            Some(index) => index,
            None => {
                destinations.push(name);
                destinations.len() - 1
            }
        };
        let mut bucket = Bucket::empty(elements.len());
        bucket.blocks = field(lead[6]).trim().parse().unwrap_or(0);
        bucket.tonnes = parse(field(lead[7]))?;
        for (index, sums) in bucket.elements.iter_mut().enumerate() {
            let at = strip + 1 + index * 5;
            sums.grade_tonnes = parse(field(at))? * bucket.tonnes;
            sums.contained = parse(field(at + 1))?;
            sums.recovered = parse(field(at + 2))?;
            sums.revenue = parse(field(at + 3))?;
            sums.cost = parse(field(at + 4))?;
        }
        bucket.mining = parse(field(costs[0]))?;
        bucket.waste_haulage = parse(field(costs[1]))?;
        bucket.rehab = parse(field(costs[2]))?;
        bucket.ore_haulage = parse(field(costs[3]))?;
        bucket.processing = parse(field(costs[4]))?;
        bucket.ga = parse(field(costs[5]))?;
        rows.push((shell, destination, bucket));
    }
    if shells.is_empty() {
        bail!("The file has no incremental rows");
    }
    // Waste first, as a run makes it.
    if let Some(waste) = destinations.iter().position(|name| name == WASTE)
        && waste != 0
    {
        destinations.swap(0, waste);
        for (_, destination, _) in &mut rows {
            if *destination == waste {
                *destination = 0;
            } else if *destination == 0 {
                *destination = waste;
            }
        }
    }
    let mut order: Vec<usize> = (0..shells.len()).collect();
    order.sort_by_key(|&index| shells[index].number);
    let position: Vec<usize> = {
        let mut position = vec![0; shells.len()];
        for (at, &index) in order.iter().enumerate() {
            position[index] = at;
        }
        position
    };
    let mut incremental = vec![vec![Bucket::empty(elements.len()); destinations.len()]; shells.len()];
    for (shell, destination, bucket) in rows {
        incremental[position[shell]][destination] = bucket;
    }
    let shells = order.into_iter().map(|index| shells[index].clone()).collect();
    Ok(OptimizationReport {
        scenario,
        shells,
        destinations,
        elements,
        incremental,
    })
}
