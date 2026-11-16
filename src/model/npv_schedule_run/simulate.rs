//! The period simulator: mines a sequence of panels period by period within
//! the capacities, sends the ore to its method or a stockpile, reclaims, and
//! discounts each period's cash flow.
//!
//! Each period:
//! 1. panels are mined in sequence order up to the mining capacity, the last
//!    one perhaps in part;
//! 2. ore no stockpile may take goes to its method; the rest fills what the
//!    method has left, highest value per tonne first, and what is still left
//!    goes onto the stockpiles feeding the method, highest min grade first;
//! 3. when some ore cannot be placed the period mines less - the most whose
//!    ore can all be placed (found by halving, as placing is monotone in what
//!    is mined);
//! 4. methods with room left reclaim the lots they own, held at the start of
//!    the period, while a lot is worth more than its rehandling.
//!
//! Every method stays within its limit, the total limit and the selling
//! limits. A stockpile keeps one lot per method it feeds, each at its own
//! average grade; the lots share the stockpile's capacity.

use super::{
    panels::{ELEMENT_GRADE_TONNES, ELEMENT_RECOVERED, Layout, Panels, Slot},
    sequence::Step,
};
use crate::model::npv_schedule::{PeriodLimit, ResolvedNpvSchedule};

/// Below this many tonnes (or sales units) an amount counts as none.
const TOLERANCE: f64 = 1e-6;
/// Halvings when a period's last panel is cut back.
const HALVINGS: usize = 48;

/// Every capacity, per period.
#[derive(Clone, Debug)]
pub(crate) struct Limits {
    pub(crate) mining: PeriodLimit,
    /// Per destination; waste (0) has no limit.
    pub(crate) methods: Vec<PeriodLimit>,
    pub(crate) total: PeriodLimit,
    /// Per element of the report.
    pub(crate) selling: Vec<PeriodLimit>,
}

/// What every simulation of one schedule shares.
pub(crate) struct Context<'a> {
    pub(crate) layout: &'a Layout,
    pub(crate) limits: &'a Limits,
    pub(crate) settings: &'a ResolvedNpvSchedule,
    /// Per stockpile, its lots at the start: per destination one parcel's
    /// numbers (`stride` each).
    pub(crate) initial_lots: &'a [Vec<f64>],
    /// Per destination, the stockpiles reclaimed into it, highest min grade first.
    pub(crate) reclaim_order: Vec<Vec<usize>>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(layout: &'a Layout, limits: &'a Limits, settings: &'a ResolvedNpvSchedule, destinations: &[String], initial_lots: &'a [Vec<f64>]) -> Self {
        let stockpiles = &settings.stockpiles;
        let reclaim_order = destinations
            .iter()
            .enumerate()
            .map(|(destination, name)| {
                let mut order: Vec<usize> = (0..stockpiles.len()).filter(|&index| destination > 0 && stockpiles[index].feeds.contains(name)).collect();
                order.sort_by(|&a, &b| stockpiles[b].min_grade.total_cmp(&stockpiles[a].min_grade).then(a.cmp(&b)));
                order
            })
            .collect();
        Self {
            layout,
            limits,
            settings,
            initial_lots,
            reclaim_order,
        }
    }

    fn destinations(&self) -> usize {
        self.limits.methods.len()
    }
}

/// Why a period mined less than its capacity allowed: the ore it would have
/// mined next had nowhere to go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Held {
    /// Its method was full and the ore was below the min grade of every
    /// stockpile feeding the method (or none feeds it).
    NoStockpile,
    /// Its method was full and the stockpiles that take it were full too.
    StockpilesFull,
}

/// One period of a schedule.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PeriodResult {
    /// Mining was held back this period, and why.
    pub(crate) held: Option<Held>,
    /// Rock mined, ore and waste.
    pub(crate) mined: f64,
    pub(crate) waste: f64,
    /// Per destination (waste, 0, stays 0): ore mined and sent straight to
    /// its method.
    pub(crate) mined_direct: Vec<f64>,
    /// Per destination (waste, 0, stays 0), tonnes processed, direct and reclaimed.
    pub(crate) processed: Vec<f64>,
    pub(crate) stocked: f64,
    pub(crate) reclaimed: f64,
    /// Per stockpile: tonnes put on it, tonnes reclaimed from it, and on it
    /// at the end of the period (all its lots).
    pub(crate) stocked_by: Vec<f64>,
    pub(crate) reclaimed_by: Vec<f64>,
    pub(crate) stockpile_balance: Vec<f64>,
    /// Per stockpile, tonnes times main-quality grade of the same three: put
    /// on, reclaimed, and on it at the end (each over its tonnes is its grade).
    pub(crate) stocked_grade_tonnes: Vec<f64>,
    pub(crate) reclaimed_grade_tonnes: Vec<f64>,
    pub(crate) balance_grade_tonnes: Vec<f64>,
    /// Per element: tonnes times grade of what was processed (the feed
    /// grade, over the tonnes processed), and the sales units recovered.
    pub(crate) feed_grade_tonnes: Vec<f64>,
    pub(crate) recovered: Vec<f64>,
    pub(crate) revenue: f64,
    /// Mining, waste haulage, rehabilitation and ore haulage of what was mined.
    pub(crate) mining_cost: f64,
    /// Processing, G&A and element costs of what was processed.
    pub(crate) processing_cost: f64,
    pub(crate) rehandle_cost: f64,
    pub(crate) fixed_cost: f64,
    /// The events' capital.
    pub(crate) capex: f64,
    /// Every cost of the period: the five above.
    pub(crate) cost: f64,
    pub(crate) cash_flow: f64,
    /// The cash flow discounted to period 0 (end of period).
    pub(crate) discounted_cash_flow: f64,
}

impl PeriodResult {
    /// Tonnes processed by every method.
    pub(crate) fn processed_total(&self) -> f64 {
        self.processed.iter().sum()
    }

    /// Tonnes on every stockpile at the end of the period.
    pub(crate) fn balance_total(&self) -> f64 {
        self.stockpile_balance.iter().sum()
    }

    /// Stockpile `stockpile`'s grade (main quality) of what was put on it,
    /// reclaimed from it, or is on it: `figure` 0, 1 or 2.
    pub(crate) fn stockpile_grade(&self, stockpile: usize, figure: usize) -> Option<f64> {
        let (tonnes, grade_tonnes) = match figure {
            0 => (&self.stocked_by, &self.stocked_grade_tonnes),
            1 => (&self.reclaimed_by, &self.reclaimed_grade_tonnes),
            _ => (&self.stockpile_balance, &self.balance_grade_tonnes),
        };
        let tonnes = tonnes.get(stockpile).copied().unwrap_or(0.0);
        (tonnes > TOLERANCE).then(|| grade_tonnes.get(stockpile).copied().unwrap_or(0.0) / tonnes)
    }

    /// The head grade (of what was processed, direct and reclaimed) of
    /// element `element`, or `None` with nothing processed.
    pub(crate) fn feed_grade(&self, element: usize) -> Option<f64> {
        let tonnes = self.processed_total();
        (tonnes > 0.0).then(|| self.feed_grade_tonnes.get(element).copied().unwrap_or(0.0) / tonnes)
    }
}

/// One candidate pit scheduled one way.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NpvPitSchedule {
    /// Period 1 first.
    pub(crate) periods: Vec<PeriodResult>,
    /// Initial capital included.
    pub(crate) npv: f64,
    /// Not mined out within the most periods.
    pub(crate) unfinished: bool,
    /// Tonnes left on the stockpiles at the end.
    pub(crate) abandoned: f64,
}

/// The room left in a period.
#[derive(Clone, Debug)]
struct Room {
    /// Per destination.
    method: Vec<f64>,
    total: f64,
    /// Per element.
    sell: Vec<f64>,
}

impl Room {
    /// Take `share` of the parcel (or lot) whose numbers start at `values[0]`
    /// into `destination`.
    fn take(&mut self, layout: &Layout, values: &[f64], destination: usize, share: f64) {
        let tonnes = values[Slot::Tonnes as usize] * share;
        self.method[destination] -= tonnes;
        self.total -= tonnes;
        for (element, sell) in self.sell.iter_mut().enumerate() {
            *sell -= values[layout.element_at(0, element, ELEMENT_RECOVERED)] * share;
        }
    }

    /// The largest share (up to 1) of the parcel or lot `values` that
    /// `destination` can still take.
    fn share_for(&self, layout: &Layout, values: &[f64], destination: usize) -> f64 {
        let tonnes = values[Slot::Tonnes as usize];
        let mut share: f64 = 1.0;
        if tonnes > 0.0 {
            share = share.min(self.method[destination].max(0.0) / tonnes).min(self.total.max(0.0) / tonnes);
        }
        for (element, sell) in self.sell.iter().enumerate() {
            let recovered = values[layout.element_at(0, element, ELEMENT_RECOVERED)];
            if recovered > 0.0 {
                share = share.min(sell.max(0.0) / recovered);
            }
        }
        share.max(0.0)
    }

    fn overdrawn(&self) -> bool {
        self.method.iter().any(|room| *room < -TOLERANCE) || self.total < -TOLERANCE || self.sell.iter().any(|room| *room < -TOLERANCE)
    }
}

/// Where a period's mined parcels go.
struct Placement {
    /// Per parcel, the share processed direct.
    direct: Vec<f64>,
    /// (parcel, stockpile, share of the parcel).
    stocked: Vec<(usize, usize, f64)>,
    room: Room,
}

fn parcel<'v>(layout: &Layout, values: &'v [f64], parcel: usize) -> &'v [f64] {
    &values[parcel * layout.stride..(parcel + 1) * layout.stride]
}

/// Place the mined parcels, or say why some ore fits nowhere.
fn place(layout: &Layout, mined: &[f64], room: &Room, space: &[f64]) -> Result<Placement, Held> {
    let mut room = room.clone();
    let mut space = space.to_vec();
    let mut direct = vec![0.0; layout.parcels.len()];
    let mut stocked = Vec::new();
    let tonnes = |index: usize| mined[layout.at(index, Slot::Tonnes)];
    // Ore no stockpile may take must go to its method.
    for (index, info) in layout.parcels.iter().enumerate().skip(1) {
        if info.stockpiles.is_empty() && tonnes(index) > TOLERANCE {
            room.take(layout, parcel(layout, mined, index), info.destination, 1.0);
            direct[index] = 1.0;
        }
    }
    if room.overdrawn() {
        return Err(Held::NoStockpile);
    }
    // The rest fills what is left, highest value per tonne first.
    let mut optional: Vec<(usize, f64)> = (1..layout.parcels.len())
        .filter(|&index| !layout.parcels[index].stockpiles.is_empty() && tonnes(index) > TOLERANCE)
        .map(|index| {
            let value = (mined[layout.at(index, Slot::Revenue)] - mined[layout.at(index, Slot::ProcessCost)]) / tonnes(index);
            (index, value)
        })
        .collect();
    optional.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (index, _) in optional {
        let info = &layout.parcels[index];
        let values = parcel(layout, mined, index);
        let share = room.share_for(layout, values, info.destination);
        room.take(layout, values, info.destination, share);
        direct[index] = share;
        // What the method cannot take goes onto its stockpiles.
        let total = tonnes(index);
        let mut rest = (1.0 - share) * total;
        for &stockpile in &info.stockpiles {
            let put = rest.min(space[stockpile].max(0.0));
            if put > 0.0 {
                stocked.push((index, stockpile, put / total));
                space[stockpile] -= put;
                rest -= put;
            }
        }
        if rest > TOLERANCE {
            return Err(Held::StockpilesFull);
        }
    }
    Ok(Placement { direct, stocked, room })
}

fn add_scaled(into: &mut [f64], values: &[f64], share: f64) {
    into.iter_mut().zip(values).for_each(|(into, value)| *into += value * share);
}

/// Schedule one candidate pit: `pushbacks` mined in `steps` order. With
/// `record`, each step's period - the one mining its middle tonne - is
/// written there (one per step, 0 for never).
pub(crate) fn simulate(context: &Context, pushbacks: &[&Panels], steps: &[Step], mut record: Option<&mut [u32]>) -> NpvPitSchedule {
    let layout = context.layout;
    let settings = context.settings;
    let limits = context.limits;
    let stride = layout.stride;
    let destinations = context.destinations();
    let stockpiles = &settings.stockpiles;
    let mut lots: Vec<Vec<f64>> = context.initial_lots.to_vec();
    // The main quality's tonnes x grade of a parcel or lot (none without elements).
    let main_grade_tonnes = |values: &[f64]| {
        if layout.elements > 0 {
            values[layout.element_at(0, 0, ELEMENT_GRADE_TONNES)]
        } else {
            0.0
        }
    };
    let lot_tonnes = |lots: &[Vec<f64>], stockpile: usize| -> f64 { (0..destinations).map(|destination| lots[stockpile][destination * stride + Slot::Tonnes as usize]).sum() };

    let mut position = 0;
    let mut done = 0.0;
    let mut schedule = NpvPitSchedule {
        npv: -settings.capex_initial,
        ..NpvPitSchedule::default()
    };
    let mut mined = vec![0.0; layout.panel_len()];
    let mut trial = vec![0.0; layout.panel_len()];
    for period in 1..=settings.max_periods {
        let finished_mining = position >= steps.len();
        let on_stockpiles: f64 = (0..stockpiles.len()).map(|stockpile| lot_tonnes(&lots, stockpile)).sum();
        if finished_mining && on_stockpiles <= TOLERANCE {
            break;
        }
        let room = Room {
            method: (0..destinations)
                .map(|destination| if destination == 0 { f64::INFINITY } else { limits.methods[destination].at(period) })
                .collect(),
            total: limits.total.at(period),
            sell: limits.selling.iter().map(|limit| limit.at(period)).collect(),
        };
        let space: Vec<f64> = (0..stockpiles.len())
            .map(|stockpile| stockpiles[stockpile].capacity - lot_tonnes(&lots, stockpile))
            .collect();
        let capacity = limits.mining.at(period);

        // 1-3: mine as far as capacity and placing allow.
        mined.iter_mut().for_each(|value| *value = 0.0);
        let mut mined_tonnes = 0.0;
        let mut step_position = position;
        let mut step_done = done;
        let mut crossed: Vec<usize> = Vec::new();
        let mut held_by = None;
        while step_position < steps.len() && (capacity.is_infinite() || capacity - mined_tonnes > TOLERANCE.max(capacity * 1e-12)) {
            let step = steps[step_position];
            let panel = &pushbacks[step.pushback][step.panel].1;
            let panel_tonnes = layout.tonnes(panel);
            let left: f64 = 1.0 - step_done;
            let mut take = if panel_tonnes > 0.0 { left.min((capacity - mined_tonnes) / panel_tonnes) } else { left };
            let fits = |share: f64, trial: &mut Vec<f64>| {
                trial.copy_from_slice(&mined);
                add_scaled(trial, panel, share);
                place(layout, trial, &room, &space).err()
            };
            let held = fits(take, &mut trial);
            if let Some(reason) = held {
                held_by = Some(reason);
                let (mut low, mut high) = (0.0, take);
                for _ in 0..HALVINGS {
                    let middle = 0.5 * (low + high);
                    if fits(middle, &mut trial).is_none() {
                        low = middle;
                    } else {
                        high = middle;
                    }
                }
                take = low;
            }
            add_scaled(&mut mined, panel, take);
            mined_tonnes += panel_tonnes * take;
            let before = step_done;
            step_done += take;
            if before < 0.5 && step_done >= 0.5 {
                crossed.push(step_position);
            }
            if step_done >= 1.0 - 1e-9 {
                step_position += 1;
                step_done = 0.0;
            }
            if held.is_some() {
                break;
            }
        }
        let placement = place(layout, &mined, &room, &space).unwrap_or_else(|_| Placement {
            direct: vec![0.0; layout.parcels.len()],
            stocked: Vec::new(),
            room: room.clone(),
        });

        // What this period processes and earns.
        let mut result = PeriodResult {
            held: held_by,
            mined_direct: vec![0.0; destinations],
            processed: vec![0.0; destinations],
            stocked_by: vec![0.0; stockpiles.len()],
            reclaimed_by: vec![0.0; stockpiles.len()],
            stocked_grade_tonnes: vec![0.0; stockpiles.len()],
            reclaimed_grade_tonnes: vec![0.0; stockpiles.len()],
            feed_grade_tonnes: vec![0.0; layout.elements],
            recovered: vec![0.0; layout.elements],
            ..PeriodResult::default()
        };
        let mut process_cost = 0.0;
        let mut mine_cost = 0.0;
        let process = |result: &mut PeriodResult, process_cost: &mut f64, values: &[f64], destination: usize, share: f64| {
            result.processed[destination] += values[Slot::Tonnes as usize] * share;
            result.revenue += values[Slot::Revenue as usize] * share;
            *process_cost += values[Slot::ProcessCost as usize] * share;
            for (element, recovered) in result.recovered.iter_mut().enumerate() {
                *recovered += values[layout.element_at(0, element, ELEMENT_RECOVERED)] * share;
            }
            for (element, grade_tonnes) in result.feed_grade_tonnes.iter_mut().enumerate() {
                *grade_tonnes += values[layout.element_at(0, element, ELEMENT_GRADE_TONNES)] * share;
            }
        };
        for (index, info) in layout.parcels.iter().enumerate() {
            let values = parcel(layout, &mined, index);
            result.mined += values[Slot::Tonnes as usize];
            mine_cost += values[Slot::MineCost as usize];
            if index == 0 {
                result.waste += values[Slot::Tonnes as usize];
            } else if placement.direct[index] > 0.0 {
                result.mined_direct[info.destination] += values[Slot::Tonnes as usize] * placement.direct[index];
                process(&mut result, &mut process_cost, values, info.destination, placement.direct[index]);
            }
        }
        // 4: reclaim from what the lots held at the start.
        let opening = lots.clone();
        for &(index, stockpile, share) in &placement.stocked {
            let destination = layout.parcels[index].destination;
            let values = parcel(layout, &mined, index);
            result.stocked += values[Slot::Tonnes as usize] * share;
            result.stocked_by[stockpile] += values[Slot::Tonnes as usize] * share;
            result.stocked_grade_tonnes[stockpile] += main_grade_tonnes(values) * share;
            add_scaled(&mut lots[stockpile][destination * stride..(destination + 1) * stride], values, share);
        }
        let mut room = placement.room;
        let mut rehandle = 0.0;
        for destination in 1..destinations {
            for &stockpile in &context.reclaim_order[destination] {
                let lot = &opening[stockpile][destination * stride..(destination + 1) * stride];
                let tonnes = lot[Slot::Tonnes as usize];
                if tonnes <= TOLERANCE {
                    continue;
                }
                let worth = (lot[Slot::Revenue as usize] - lot[Slot::ProcessCost as usize]) / tonnes - stockpiles[stockpile].rehandle_cost;
                if worth <= 0.0 {
                    continue;
                }
                let share = room.share_for(layout, lot, destination);
                if share <= 0.0 {
                    continue;
                }
                room.take(layout, lot, destination, share);
                process(&mut result, &mut process_cost, lot, destination, share);
                result.reclaimed += tonnes * share;
                result.reclaimed_by[stockpile] += tonnes * share;
                result.reclaimed_grade_tonnes[stockpile] += main_grade_tonnes(lot) * share;
                rehandle += tonnes * share * stockpiles[stockpile].rehandle_cost;
                add_scaled(&mut lots[stockpile][destination * stride..(destination + 1) * stride], lot, -share);
            }
        }
        result.mining_cost = mine_cost;
        result.processing_cost = process_cost;
        result.rehandle_cost = rehandle;
        result.fixed_cost = settings.fixed_cost;
        result.capex = settings.capex_in(period);
        result.cost = mine_cost + process_cost + rehandle + result.fixed_cost + result.capex;
        result.cash_flow = result.revenue - result.cost;
        result.discounted_cash_flow = result.cash_flow / (1.0 + settings.rate).powi(period as i32);

        // Once the pit is mined out, a period only reclaiming runs while it pays.
        if finished_mining && result.cash_flow <= 0.0 {
            lots = opening;
            break;
        }
        result.stockpile_balance = (0..stockpiles.len()).map(|stockpile| lot_tonnes(&lots, stockpile).max(0.0)).collect();
        result.balance_grade_tonnes = (0..stockpiles.len())
            .map(|stockpile| {
                (0..destinations)
                    .map(|destination| main_grade_tonnes(&lots[stockpile][destination * stride..(destination + 1) * stride]))
                    .sum::<f64>()
                    .max(0.0)
            })
            .collect();
        schedule.npv += result.discounted_cash_flow;
        schedule.periods.push(result);
        if let Some(record) = record.as_deref_mut() {
            for step in crossed {
                record[step] = period;
            }
        }
        position = step_position;
        done = step_done;
    }
    schedule.unfinished = position < steps.len();
    schedule.abandoned = (0..stockpiles.len()).map(|stockpile| lot_tonnes(&lots, stockpile).max(0.0)).sum();
    // Capital an event puts after the last period run (closure, say) is still
    // spent: each such period is charged and discounted, with nothing mined
    // or processed and no fixed cost.
    if !schedule.unfinished {
        let last = schedule.periods.len() as u32;
        if let Some(end) = (last + 1..=settings.max_periods).filter(|&period| settings.capex_in(period) > 0.0).max() {
            let balance = schedule
                .periods
                .last()
                .map(|period| (period.stockpile_balance.clone(), period.balance_grade_tonnes.clone()));
            for period in last + 1..=end {
                let capex = settings.capex_in(period);
                let discounted = -capex / (1.0 + settings.rate).powi(period as i32);
                let (stockpile_balance, balance_grade_tonnes) = balance.clone().unwrap_or_else(|| (vec![0.0; stockpiles.len()], vec![0.0; stockpiles.len()]));
                schedule.npv += discounted;
                schedule.periods.push(PeriodResult {
                    mined_direct: vec![0.0; destinations],
                    processed: vec![0.0; destinations],
                    stocked_by: vec![0.0; stockpiles.len()],
                    reclaimed_by: vec![0.0; stockpiles.len()],
                    stocked_grade_tonnes: vec![0.0; stockpiles.len()],
                    reclaimed_grade_tonnes: vec![0.0; stockpiles.len()],
                    stockpile_balance,
                    balance_grade_tonnes,
                    feed_grade_tonnes: vec![0.0; layout.elements],
                    recovered: vec![0.0; layout.elements],
                    capex,
                    cost: capex,
                    cash_flow: -capex,
                    discounted_cash_flow: discounted,
                    ..PeriodResult::default()
                });
            }
        }
    }
    schedule
}
