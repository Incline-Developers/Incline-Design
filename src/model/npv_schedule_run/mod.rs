//! Running a schedule: from a scenario's run to the NPV of every
//! candidate pit, scheduled three ways.
//!
//! The scenario's run keeps only its grid and each cell's shell, so the job
//! values the cells again ([`values`](crate::model::optimization_run::values),
//! without the solver) and lays them into panels ([`panels`]): what each
//! shell holds on each bench. Every shell is then a candidate final pit,
//! mined in the worst, best and fixed-lead orders ([`sequence`]) and
//! simulated period by period ([`simulate`]).

pub(crate) mod panels;
pub(crate) mod report;
pub(crate) mod sequence;
pub(crate) mod simulate;

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use anyhow::{Result, bail};
use rayon::prelude::*;

use self::{
    panels::{ELEMENT_GRADE_TONNES, ELEMENT_RECOVERED, Layout, Panels, Slot, add_panels},
    simulate::{Context, Limits, NpvPitSchedule, simulate},
};
use crate::{
    app::jobs::CancelFlag,
    i18n::tr,
    model::{
        npv_schedule::{PeriodCase, PeriodLimit, ResolvedNpvSchedule},
        optimization::round_factor,
        optimization_run::{
            RunResult, ShellField,
            grid::{BlockLayout, Grid, NO_BLOCK},
            prepare::{Air, Economics, Prepared},
            report::{self as run_report, ElementInfo, ShellInfo},
            shells::OUTSIDE,
            values,
        },
        progress::Progress,
    },
};

/// Everything a schedule run needs, owned so it can go to a worker.
pub(crate) struct NpvScheduleInput {
    pub(crate) scenario_name: String,
    pub(crate) schedule_name: String,
    pub(crate) prepared: Prepared,
    pub(crate) layout: BlockLayout,
    /// The scenario's run being scheduled.
    pub(crate) run: Arc<RunResult>,
    pub(crate) settings: ResolvedNpvSchedule,
    /// The case whose mining order is kept (for the period field and the
    /// animation), at the pit where its NPV peaks.
    pub(crate) case: PeriodCase,
    /// Make the period field too.
    pub(crate) write_field: bool,
}

/// [`MiningOrder::block_order`] of air and of blocks on no cell of the grid.
pub(crate) const ORDER_AIR: u32 = 0;
/// [`MiningOrder::block_order`] of rock outside the pit: the ground around it.
pub(crate) const ORDER_GROUND: u32 = u32::MAX;

/// The floor of grid layer `layer`: its blocks' centroid less half the block
/// height, from the grid's definition alone. Rounded to a micrometre, so
/// float noise (`304.79999999999995` for 20 benches of 15.24) does not make
/// neighbouring benches look unevenly spaced: they differ by one block height.
pub(crate) fn bench_floor(grid: &Grid, layer: usize) -> f64 {
    let centroid = grid.origin.z + grid.cell.z * (layer as f64 + 0.5);
    ((centroid - grid.cell.z / 2.0) * 1e6).round() / 1e6
}

/// A bench elevation as shown: whole (`1250`) unless it has a fraction
/// (`1250.5`), never trailing zeros. The value itself stays exact.
pub(crate) fn format_elevation(elevation: f64) -> String {
    if (elevation - elevation.round()).abs() < 1e-6 {
        format!("{:.0}", elevation.round())
    } else {
        let text = format!("{elevation:.3}");
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// One step of a mining order: one pushback's bench.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OrderStep {
    /// 1-based.
    pub(crate) pushback: usize,
    /// The bench's elevation: its floor, the bottom of its block layer (a
    /// centroid less half the block height), as mining names a bench.
    pub(crate) elevation: f64,
    /// The period mining the middle tonne of the panel.
    pub(crate) period: u32,
}

/// How one pit is mined in one case: what the period field and the
/// animation read. Small - a few entries per pushback bench - because a
/// block's step follows from its cell's shell and bench, which the
/// scenario's run holds ([`Self::block_order`]).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MiningOrder {
    pub(crate) case: PeriodCase,
    /// 0-based.
    pub(crate) pit: usize,
    pub(crate) steps: Vec<OrderStep>,
    /// The step (1-based, into `steps`) of each (0-based pushback, bench).
    step_of: HashMap<(usize, u32), u32>,
    /// Per shell of the pit (0-based), its pushback.
    pushback_of_shell: Vec<usize>,
}

impl MiningOrder {
    /// Per block its step (1-based), or [`ORDER_AIR`] or [`ORDER_GROUND`]:
    /// built from `run`'s shell and air per cell, with `block_of_cell` of
    /// the block model on `run`'s grid.
    pub(crate) fn block_order(&self, run: &RunResult, block_of_cell: &[u32], blocks: usize) -> Vec<u32> {
        let grid = &run.grid;
        let top = grid.dims[2] - 1;
        let layer = grid.column_count();
        let mut order = vec![ORDER_AIR; blocks];
        for (cell, &block) in block_of_cell.iter().enumerate() {
            if block == NO_BLOCK || run.air.get(cell) {
                continue;
            }
            let shell = run.shells.label[cell];
            order[block as usize] = if shell == OUTSIDE || usize::from(shell) > self.pit + 1 {
                ORDER_GROUND
            } else {
                let bench = (top - cell / layer) as u32;
                self.step_of.get(&(self.pushback_of_shell[usize::from(shell) - 1], bench)).copied().unwrap_or(ORDER_GROUND)
            };
        }
        order
    }

    /// The categorical period field: per block its step's period, blank for
    /// air and ground.
    pub(crate) fn period_field(&self, block_order: &[u32]) -> ShellField {
        let mut categories = BTreeMap::new();
        let codes = block_order
            .iter()
            .map(|&order| match order {
                ORDER_AIR | ORDER_GROUND => f64::NAN,
                step => match self.steps[step as usize - 1].period {
                    0 => f64::NAN,
                    period => {
                        categories.entry(period).or_insert_with(|| period.to_string());
                        f64::from(period)
                    }
                },
            })
            .collect();
        ShellField {
            codes: Arc::new(codes),
            categories,
        }
    }
}

/// One bench of one pushback (what the pushback adds there).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PushbackBench {
    /// The bench's elevation: its floor, the bottom of its block layer (a
    /// centroid less half the block height), as mining names a bench.
    pub(crate) elevation: f64,
    pub(crate) ore: f64,
    pub(crate) waste: f64,
    /// Ore tonnes times main-quality grade.
    pub(crate) grade_tonnes: f64,
    /// The schedule's period mining it (fixed lead), when known.
    pub(crate) period: Option<u32>,
}

/// One pushback's outer shell as the final pit, scheduled three ways.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CandidatePit {
    /// 0-based shell of the scenario's run.
    pub(crate) shell: usize,
    /// The pit's tonnes: ore (to any method at the base price) and waste.
    pub(crate) ore: f64,
    pub(crate) waste: f64,
    /// The pushback's own benches (its pit less the one before), top first.
    /// Not read back from a file.
    pub(crate) benches: Vec<PushbackBench>,
    pub(crate) worst: NpvPitSchedule,
    pub(crate) best: NpvPitSchedule,
    pub(crate) lead: NpvPitSchedule,
}

impl CandidatePit {
    pub(crate) fn case(&self, case: PeriodCase) -> &NpvPitSchedule {
        match case {
            PeriodCase::FixedLead => &self.lead,
            PeriodCase::Best => &self.best,
            PeriodCase::Worst => &self.worst,
        }
    }
}

/// What a schedule run found, kept for the session (stage 3 reads it).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NpvScheduleResult {
    pub(crate) scenario: String,
    pub(crate) name: String,
    /// Discount rate as a fraction per period.
    pub(crate) rate: f64,
    /// Spent in period 0, undiscounted.
    pub(crate) capex_initial: f64,
    /// Benches each pushback stays ahead of the next (the fixed lead).
    pub(crate) lead: u32,
    pub(crate) shells: Vec<ShellInfo>,
    /// As the scenario's report: waste, then each method.
    pub(crate) destinations: Vec<String>,
    pub(crate) elements: Vec<ElementInfo>,
    pub(crate) stockpiles: Vec<String>,
    /// Per pushback, in order, its outer shell as the final pit. The last is
    /// the schedule: every pushback mined.
    pub(crate) pits: Vec<CandidatePit>,
    /// The kept case's mining order of the schedule. Not read back from a file.
    pub(crate) order: Option<MiningOrder>,
}

impl NpvScheduleResult {
    /// The schedule: the last pushback's pit, every pushback mined.
    pub(crate) fn schedule(&self) -> Option<&CandidatePit> {
        self.pits.last()
    }

    /// The pushback (0-based, into `pits`) where `case`'s NPV peaks, among
    /// those mined out.
    pub(crate) fn peak(&self, case: PeriodCase) -> Option<usize> {
        self.pits
            .iter()
            .enumerate()
            .filter(|(_, pit)| !pit.case(case).unfinished)
            .max_by(|a, b| a.1.case(case).npv.total_cmp(&b.1.case(case).npv))
            .map(|(index, _)| index)
    }
}

/// A finished schedule run.
pub(crate) struct NpvScheduleOutcome {
    pub(crate) result: NpvScheduleResult,
    /// The periods to write into the block model: per block the period its
    /// panel is mined in.
    pub(crate) field: Option<ShellField>,
}

pub(crate) fn run(input: NpvScheduleInput, cancel: &CancelFlag, progress: &Progress) -> Result<NpvScheduleOutcome> {
    let NpvScheduleInput {
        scenario_name,
        schedule_name,
        prepared,
        layout,
        run,
        settings,
        case,
        write_field,
    } = input;
    let economics = &prepared.economics;
    let grid = layout.grid;
    if grid != run.grid {
        bail!("{}", tr!("sched-run-grid-changed"));
    }
    let block_of_cell = layout.block_of_cell(cancel)?;
    progress.set_fraction(0.05);
    let heights = match &economics.air {
        Air::Topography(surface) => Some(surface.column_heights(&grid)),
        _ => None,
    };
    let values = values::compute(economics, &grid, &block_of_cell, heights.as_deref(), cancel)?;
    progress.set_fraction(0.15);

    let (destinations, method_destination) = run_report::destinations(economics);
    let parcels = Layout::new(&destinations, method_destination, economics.elements.len(), &settings.stockpiles);
    let shell_count = run.shells.summaries.len();
    let shell_panels = panels::build(economics, &grid, &block_of_cell, &values, &run.shells, &parcels, cancel)?;
    progress.set_fraction(0.25);

    let named = |rows: &[(String, PeriodLimit)], name: &str| rows.iter().find(|(row, _)| row == name).map_or(PeriodLimit::NONE, |(_, limit)| limit.clone());
    let limits = Limits {
        mining: settings.mining.clone(),
        methods: destinations
            .iter()
            .enumerate()
            .map(|(destination, name)| if destination == 0 { PeriodLimit::NONE } else { named(&settings.methods, name) })
            .collect(),
        total: settings.total.clone(),
        selling: economics.elements.iter().map(|element| named(&settings.selling, &element.name)).collect(),
    };
    let lots = initial_lots(economics, &destinations, &settings, &parcels)?;
    let context = Context::new(&parcels, &limits, &settings, &destinations, &lots);
    let pushbacks = Pushbacks::new(&settings, &run.report.shells, &shell_panels);

    // Pits in batches: the whole pit and the pushback being built are summed
    // shell by shell; at each pushback's end its pit joins the batch, and a
    // full batch is scheduled side by side.
    let batch_size = rayon::current_num_threads().max(1) * 2;
    let mut pits: Vec<CandidatePit> = Vec::with_capacity(shell_count);
    let mut whole = Panels::new();
    let mut tail = Panels::new();
    let mut next = 0;
    while next < shell_count {
        let mut batch: Vec<(usize, Panels, Panels)> = Vec::with_capacity(batch_size);
        while batch.len() < batch_size && next < shell_count {
            add_panels(&mut whole, &shell_panels[next]);
            add_panels(&mut tail, &shell_panels[next]);
            if pushbacks.ends[next] {
                batch.push((next, whole.clone(), std::mem::take(&mut tail)));
            }
            next += 1;
        }
        if cancel.is_cancelled() {
            bail!("Cancelled");
        }
        let scheduled: Vec<CandidatePit> = batch
            .par_iter()
            .map(|(pit, whole, tail)| {
                let worst = [whole];
                let best: Vec<&Panels> = shell_panels[..=*pit].iter().collect();
                let lead = pushbacks.lead(*pit, tail);
                let total: f64 = whole.iter().map(|(_, panel)| parcels.tonnes(panel)).sum();
                let waste: f64 = whole.iter().map(|(_, panel)| panel[parcels.at(0, Slot::Tonnes)]).sum();
                let top = grid.dims[2] - 1;
                let benches = tail
                    .iter()
                    .map(|(bench, panel)| {
                        let tonnes = parcels.tonnes(panel);
                        let waste = panel[parcels.at(0, Slot::Tonnes)];
                        let grade_tonnes = if parcels.elements > 0 {
                            (1..parcels.parcels.len()).map(|parcel| panel[parcels.element_at(parcel, 0, ELEMENT_GRADE_TONNES)]).sum()
                        } else {
                            0.0
                        };
                        PushbackBench {
                            elevation: bench_floor(&grid, top - *bench as usize),
                            ore: tonnes - waste,
                            waste,
                            grade_tonnes,
                            period: None,
                        }
                    })
                    .collect();
                CandidatePit {
                    shell: *pit,
                    ore: total - waste,
                    waste,
                    benches,
                    worst: simulate(&context, &worst, &sequence::order(&worst, Some(0)), None),
                    best: simulate(&context, &best, &sequence::order(&best, None), None),
                    lead: simulate(&context, &lead, &sequence::order(&lead, Some(settings.lead)), None),
                }
            })
            .collect();
        pits.extend(scheduled);
        progress.set_fraction(0.25 + 0.7 * next as f32 / shell_count.max(1) as f32);
    }

    let mut result = NpvScheduleResult {
        scenario: scenario_name,
        name: schedule_name,
        rate: settings.rate,
        capex_initial: settings.capex_initial,
        lead: settings.lead,
        shells: run.report.shells.clone(),
        destinations,
        elements: run.report.elements.clone(),
        stockpiles: settings.stockpiles.iter().map(|stockpile| stockpile.name.clone()).collect(),
        pits,
        order: None,
    };
    if let Some(pit) = result.schedule().map(|pit| pit.shell) {
        // The pit's pushbacks in this case, and each shell's pushback.
        let mut pushback_of_shell = vec![0usize; pit + 1];
        let mut owned = Panels::new();
        match case {
            PeriodCase::Worst => shell_panels[..=pit].iter().for_each(|panels| add_panels(&mut owned, panels)),
            PeriodCase::Best => pushback_of_shell.iter_mut().enumerate().for_each(|(shell, pushback)| *pushback = shell),
            PeriodCase::FixedLead if pushbacks.every_shell => pushback_of_shell.iter_mut().enumerate().for_each(|(shell, pushback)| *pushback = shell),
            PeriodCase::FixedLead => {
                let mut pushback = 0;
                for shell in 0..=pit {
                    pushback_of_shell[shell] = pushback;
                    add_panels(&mut owned, &shell_panels[shell]);
                    if pushbacks.ends[shell] && shell < pit {
                        pushback += 1;
                        owned.clear();
                    }
                }
            }
        }
        let (list, lead): (Vec<&Panels>, Option<u32>) = match case {
            PeriodCase::Worst => (vec![&owned], Some(0)),
            PeriodCase::Best => (shell_panels[..=pit].iter().collect(), None),
            PeriodCase::FixedLead => (pushbacks.lead(pit, &owned), Some(settings.lead)),
        };
        let steps = sequence::order(&list, lead);
        let mut periods = vec![0u32; steps.len()];
        simulate(&context, &list, &steps, Some(&mut periods));
        let top = grid.dims[2] - 1;
        let step_of: HashMap<(usize, u32), u32> = steps
            .iter()
            .enumerate()
            .map(|(index, step)| ((step.pushback, list[step.pushback][step.panel].0), index as u32 + 1))
            .collect();
        let order_steps: Vec<OrderStep> = steps
            .iter()
            .zip(&periods)
            .map(|(step, &period)| {
                let bench = list[step.pushback][step.panel].0 as usize;
                OrderStep {
                    pushback: step.pushback + 1,
                    elevation: bench_floor(&grid, top - bench),
                    period,
                }
            })
            .collect();
        // The fixed-lead schedule's pushbacks are the pits': each bench's period.
        if case == PeriodCase::FixedLead {
            for step in &order_steps {
                if let Some(bench) = result
                    .pits
                    .get_mut(step.pushback - 1)
                    .and_then(|pit| pit.benches.iter_mut().find(|bench| (bench.elevation - step.elevation).abs() < 1e-6))
                {
                    bench.period = (step.period > 0).then_some(step.period);
                }
            }
        }
        result.order = Some(MiningOrder {
            case,
            pit,
            steps: order_steps,
            step_of,
            pushback_of_shell,
        });
    }
    let field = write_field
        .then(|| {
            result
                .order
                .as_ref()
                .map(|order| order.period_field(&order.block_order(&run, &block_of_cell, layout.blocks.len())))
        })
        .flatten();
    progress.set_fraction(1.0);
    Ok(NpvScheduleOutcome { result, field })
}

/// The fixed-lead pushbacks: which shells end one.
struct Pushbacks<'a> {
    shell_panels: &'a [Panels],
    /// Per shell, whether a pushback ends at it.
    ends: Vec<bool>,
    /// Every shell its own pushback.
    every_shell: bool,
    /// The chosen pushbacks, by the shell they end at.
    chosen: Vec<(usize, Panels)>,
}

impl<'a> Pushbacks<'a> {
    /// With chosen factors, each ends at the outermost shell of its factor.
    fn new(settings: &ResolvedNpvSchedule, shells: &[ShellInfo], shell_panels: &'a [Panels]) -> Self {
        let mut ends = vec![settings.pushback_factors.is_none(); shells.len()];
        if let Some(factors) = &settings.pushback_factors {
            for factor in factors {
                if let Some(last) = shells.iter().rposition(|shell| round_factor(shell.factor) == round_factor(*factor)) {
                    ends[last] = true;
                }
            }
        }
        let every_shell = settings.pushback_factors.is_none();
        let mut chosen = Vec::new();
        if !every_shell {
            let mut tail = Panels::new();
            for (shell, panels) in shell_panels.iter().enumerate() {
                add_panels(&mut tail, panels);
                if ends[shell] {
                    chosen.push((shell, std::mem::take(&mut tail)));
                }
            }
        }
        Self {
            shell_panels,
            ends,
            every_shell,
            chosen,
        }
    }

    /// Pit `pit`'s pushbacks: the chosen ones inside it, then `tail` (what
    /// it holds beyond the last of them).
    fn lead<'b>(&'b self, pit: usize, tail: &'b Panels) -> Vec<&'b Panels> {
        if self.every_shell {
            return self.shell_panels[..=pit].iter().collect();
        }
        let mut list: Vec<&Panels> = self.chosen.iter().filter(|(end, _)| *end < pit).map(|(_, panels)| panels).collect();
        list.push(tail);
        list
    }
}

/// Each stockpile's opening tonnes as a lot of its first fed method, valued
/// with that method's main-quality element only; their mining is already paid.
fn initial_lots(economics: &Economics, destinations: &[String], settings: &ResolvedNpvSchedule, layout: &Layout) -> Result<Vec<Vec<f64>>> {
    let stride = layout.stride;
    let mut lots = Vec::with_capacity(settings.stockpiles.len());
    for stockpile in &settings.stockpiles {
        let mut lot = vec![0.0; destinations.len() * stride];
        let tonnes = stockpile.initial_tonnes;
        if let Some(feed) = stockpile.feeds.first()
            && tonnes > 0.0
        {
            let grade = stockpile.initial_grade;
            let Some(destination) = destinations.iter().position(|name| name == feed) else {
                bail!("{}", tr!("sched-run-initial-grade", stockpile = stockpile.name.clone(), method = feed.clone()));
            };
            let Some(method) = economics.methods.iter().find(|method| method.name == *feed && grade >= method.min && grade < method.max) else {
                bail!("{}", tr!("sched-run-initial-grade", stockpile = stockpile.name.clone(), method = feed.clone()));
            };
            let values = &mut lot[destination * stride..(destination + 1) * stride];
            values[Slot::Tonnes as usize] = tonnes;
            values[layout.element_at(0, 0, ELEMENT_GRADE_TONNES)] = tonnes * grade;
            let mut cost = tonnes * (method.processing + method.ga);
            for element in method.elements.iter().filter(|element| element.index == 0) {
                let recovered = tonnes * (grade - element.threshold).max(0.0) * element.factor * element.recovery;
                values[layout.element_at(0, 0, ELEMENT_RECOVERED)] += recovered;
                values[Slot::Revenue as usize] += recovered * element.net_price;
                cost += tonnes * grade * element.factor * element.cost;
            }
            values[Slot::ProcessCost as usize] = cost;
        }
        lots.push(lot);
    }
    Ok(lots)
}
