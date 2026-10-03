//! Whole-horizon scheduling inputs and results.
//!
//! This module is deliberately calculation-owned. Stage 5 will resolve the
//! mutable project into these stable ids, fixed material records and feasible
//! route candidates before starting a job. The solver never consults the
//! project again, and its output contains every choice needed for publication.
//!
//! # Calendar intervals and execution segments
//!
//! Calendar intervals ([`Interval`]) are where input settings are constant:
//! rates, fleet availability, bar-window eligibility and daily budget
//! boundaries. They carry no execution semantics of their own. Execution is
//! modelled inside each interval as an ordered sequence of shared *segments* -
//! event positions all loaders agree on - whose durations are optimiser
//! decisions. A loader works one source per segment and may finish a block,
//! lot or parcel and continue to the next within the same interval. Unused
//! segments take zero duration and publish nothing.

/// The blended-stockpile model the schedule optimiser solves: scenario
/// contract, the model written once, and the independent replay every
/// published schedule passes.
pub(crate) mod blended;

/// The nonlinear SCIP solver for that same blended model, separate from
/// [`blended`] because it is the only part that needs SCIP.
#[cfg(all(not(target_arch = "wasm32"), feature = "scip"))]
pub(crate) mod scip;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        pub(crate) struct $name(pub(crate) u32);
    };
}

id_type!(LoaderId);
id_type!(TaskId);
id_type!(GroundId);
id_type!(StockpileId);
id_type!(MaterialId);
id_type!(DestinationId);
id_type!(TruckClassId);
id_type!(RoutingRuleId);
id_type!(CashflowRuleId);

/// Numerical rules are part of the calculation metadata, rather than hidden
/// solver constants. They scale with the authored model where appropriate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NumericalTolerances {
    pub(crate) tonnes_t: f64,
    pub(crate) objective: f64,
    pub(crate) integrality: f64,
    pub(crate) parcel_occupancy_t: f64,
}

impl Default for NumericalTolerances {
    fn default() -> Self {
        Self {
            tonnes_t: 1e-6,
            objective: 1e-6,
            integrality: 1e-6,
            parcel_occupancy_t: 1e-5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HorizonSpec {
    pub(crate) end_h: f64,
    pub(crate) regular_step_h: f64,
    pub(crate) parcel_target_t: f64,
    /// Overrides the data-derived per-interval execution-segment budget. The
    /// derived budget is normally sufficient by construction; an override
    /// exists so a proof can repeat a fixture with more event positions and
    /// confirm the budget was not restricting transitions.
    pub(crate) event_segments: Option<u32>,
    pub(crate) tolerances: NumericalTolerances,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Interval {
    pub(crate) index: usize,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
}

impl Interval {
    pub(crate) fn duration_h(self) -> f64 {
        self.end_h - self.start_h
    }
    pub(crate) fn day(self) -> u32 {
        (self.start_h / 24.0).floor() as u32
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum InputError {
    NonFinite(&'static str),
    NonPositive(&'static str),
    InvalidWindow,
    InvalidIntervals,
}

/// Split a finite horizon at its regular grid, every midnight, every authored
/// window edge and every supplied calendar change. Near-identical boundaries
/// collapse using the configured tonnes-independent time tolerance.
pub(crate) fn build_intervals(spec: HorizonSpec, windows: &[(f64, f64)], calendar_changes_h: &[f64]) -> Result<Vec<Interval>, InputError> {
    if !spec.end_h.is_finite() || spec.end_h <= 0.0 {
        return Err(InputError::NonPositive("horizon"));
    }
    if !spec.regular_step_h.is_finite() || spec.regular_step_h <= 0.0 {
        return Err(InputError::NonPositive("interval step"));
    }
    if !spec.parcel_target_t.is_finite() || spec.parcel_target_t <= 0.0 {
        return Err(InputError::NonPositive("parcel target"));
    }
    let mut points = vec![0.0, spec.end_h];
    let add_regular = |step: f64, points: &mut Vec<f64>| {
        let mut at = step;
        while at < spec.end_h {
            points.push(at);
            at += step;
        }
    };
    add_regular(spec.regular_step_h, &mut points);
    add_regular(24.0, &mut points);
    for &(start, end) in windows {
        if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start {
            return Err(InputError::InvalidWindow);
        }
        if start < spec.end_h {
            points.push(start);
        }
        if end < spec.end_h {
            points.push(end);
        }
    }
    for &at in calendar_changes_h {
        if !at.is_finite() || at < 0.0 {
            return Err(InputError::InvalidIntervals);
        }
        if at > 0.0 && at < spec.end_h {
            points.push(at);
        }
    }
    points.sort_by(f64::total_cmp);
    points.dedup_by(|a, b| (*a - *b).abs() <= 1e-9);
    Ok(points
        .windows(2)
        .enumerate()
        .map(|(index, edge)| Interval {
            index,
            start_h: edge[0],
            end_h: edge[1],
        })
        .collect())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Activity {
    Dig,
    Reclaim,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ReclaimOrder {
    Fifo,
    Lifo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum SourceId {
    Ground(GroundId),
    Stockpile(StockpileId),
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct MaterialShare {
    pub(crate) material: MaterialId,
    pub(crate) fraction: f64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GroundSource {
    pub(crate) id: GroundId,
    pub(crate) tonnes_t: f64,
    pub(crate) material: Vec<MaterialShare>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum DestinationKind {
    Crusher,
    Dump,
    Stockpile(StockpileId),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Destination {
    pub(crate) id: DestinationId,
    pub(crate) kind: DestinationKind,
    /// Finite for dumps and stockpiles. Crushers use the daily budget instead.
    pub(crate) capacity_t: Option<f64>,
    /// One entry per day touched by the horizon. `None` is unlimited.
    pub(crate) crusher_daily_t: Vec<Option<f64>>,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct IntervalRate {
    pub(crate) interval: usize,
    pub(crate) dig_tph: f64,
    pub(crate) reclaim_tph: f64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Loader {
    pub(crate) id: LoaderId,
    pub(crate) rates: Vec<IntervalRate>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum TaskKind {
    Dig {
        sequence: Vec<GroundId>,
    },
    /// A vector now, although Stage 3 captures exactly one approved pile.
    Reclaim {
        approved_sources: Vec<StockpileId>,
        maximum_t: Option<f64>,
    },
    /// A delay bar: always has work while its window is open, and authorises
    /// no movement, so while it is the loader's highest-priority ready bar
    /// the loader stands.
    Delay,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Task {
    pub(crate) id: TaskId,
    pub(crate) loader: LoaderId,
    pub(crate) priority: u32,
    pub(crate) window_start_h: f64,
    pub(crate) window_end_h: f64,
    pub(crate) kind: TaskKind,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TruckClass {
    pub(crate) id: TruckClassId,
    /// Available shared truck-hours by interval. Segments receive their
    /// duration-proportional share of this budget: the effective fleet is
    /// `hours / interval duration` trucks at every instant of the interval.
    pub(crate) hours: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CashflowContribution {
    pub(crate) rule: CashflowRuleId,
    pub(crate) value_per_tonne: f64,
}

/// One feasible destination/truck combination. Capture creates these only when
/// the routing, trucking and cashflow selectors all resolve. Different truck
/// classes are separate candidates sharing the same physical movement.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct MovementCandidate {
    pub(crate) loader: LoaderId,
    pub(crate) activity: Activity,
    pub(crate) source: SourceId,
    pub(crate) material: MaterialId,
    pub(crate) destination: DestinationId,
    pub(crate) truck: TruckClassId,
    pub(crate) truck_hours_per_tonne: f64,
    #[serde(default)]
    pub(crate) cycle: super::trucking::CycleBreakdown,
    pub(crate) routing_rule: RoutingRuleId,
    pub(crate) routing_preference: u32,
    pub(crate) cashflow: Vec<CashflowContribution>,
}

impl MovementCandidate {
    pub(crate) fn value_per_tonne(&self) -> Result<f64, InputError> {
        let value = self
            .cashflow
            .iter()
            .try_fold(0.0_f64, |sum, item| {
                let next = sum + item.value_per_tonne;
                next.is_finite().then_some(next)
            })
            .ok_or(InputError::NonFinite("cashflow sum"))?;
        Ok(value)
    }
}

/// Ceiling applied to the *derived* per-interval segment budget. The
/// derivation governs normally; this only stops pathological inputs (for
/// example a fleet of tiny opening lots) from allocating thousands of event
/// positions. Published in the result metadata.
///
/// Solver-independent: the blended experiment's capture derives its own
/// budget by the same union-bound rule and is held to the same guard, so
/// there is one event-budget policy rather than two.
pub(crate) const SEGMENT_CEILING: usize = 24;
