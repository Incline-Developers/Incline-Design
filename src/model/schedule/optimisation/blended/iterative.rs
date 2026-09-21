//! The iterative fixed-grade HiGHS method for the blended model.
//!
//! # What it is
//!
//! The blended model's only nonlinearity is the mixing equality
//! `Q_recl * T_open = T_recl * Q_open`. Fix the blend to an estimate `g_hat`
//! for each pile (or chunk), interval and grade and that row becomes
//! `Q_recl = g_hat * T_recl`, which is linear, and the whole model becomes a
//! MILP that HiGHS can solve. The catch is that `g_hat` is an *assumption*:
//! the schedule the MILP returns changes what the pile actually contains, so
//! the assumption has to be checked and corrected.
//!
//! One iteration is therefore:
//!
//! 1. build the MILP at the current estimates and solve it with HiGHS;
//! 2. extract the candidate timeline;
//! 3. **replay** it physically, recomputing the real blends by division;
//! 4. update the estimates from those real blends;
//! 5. repeat until the estimates stop moving, the schedule is valid, or the
//!    budget runs out.
//!
//! Everything except the mixing row is built by
//! [`super::formulation::formulate`], the same function the SCIP backend
//! uses, so the two methods are not merely described as solving the same
//! constraints - they are built from one source.
//!
//! # Three outcomes, kept apart
//!
//! [`CandidateOutcome`] is the heart of the method, because conflating its
//! three cases is how an iterative scheme quietly reports nonsense:
//!
//! - **Physically impossible.** Negative inventory, a breached capacity, a
//!   broken authored sequence, an overused resource. The candidate is not a
//!   schedule, and no re-estimation fixes it.
//! - **Grade-assumption mismatch.** The movements replay cleanly, but the
//!   blends that actually resulted differ from the estimates the MILP was
//!   built on - so a route or a payment the MILP granted may not be earned.
//!   This is the case the iteration exists to resolve.
//! - **Publishable.** Physical, grade and valuation checks all pass on the
//!   *replayed* quantities.
//!
//! A candidate is never publishable merely because the fixed-grade MILP
//! declared it feasible.
//!
//! # What this method cannot tell you
//!
//! HiGHS's status and dual bound describe **one fixed-grade subproblem**,
//! not the blended problem. In particular:
//!
//! - An infeasible fixed-grade MILP does **not** prove the blended problem
//!   infeasible. A wrong estimate can make a feasible schedule unreachable.
//! - A proven-optimal fixed-grade MILP does **not** bound the blended
//!   problem's optimum, so this method reports no gap at all. The best it can
//!   honestly say is "here is an independently valid schedule worth X".

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use good_lp::{Expression, ProblemVariables, Solution, SolverModel, Variable, variable};

use super::{
    formulation::{BlendColumns, BlendSizes, Rows, chunk_key, formulate},
    input::BlendInput,
    replay::{BlendSolution, ChunkRow, ExtractionAdjustments, MovementRow, ReplayReport, replay},
};
use crate::model::schedule::optimisation::StockpileId;

/// Grade estimates, keyed the way [`Rows::mix`] is keyed.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Estimates {
    values: BTreeMap<(StockpileId, usize, usize), f64>,
    /// Used where a key has no estimate yet - an empty pile or chunk, whose
    /// grade is genuinely undefined.
    fallback: BTreeMap<usize, f64>,
}

impl Estimates {
    fn get(&self, pile: StockpileId, key: usize, grade: usize) -> f64 {
        self.values
            .get(&(pile, key, grade))
            .copied()
            .unwrap_or_else(|| self.fallback.get(&grade).copied().unwrap_or(0.0))
    }

    /// The largest absolute move between two estimate sets, over the keys
    /// they share and the keys only one of them has.
    fn drift(&self, other: &Self) -> f64 {
        let mut worst: f64 = 0.0;
        for (key, value) in &self.values {
            worst = worst.max((value - other.values.get(key).copied().unwrap_or(*value)).abs());
        }
        for (key, value) in &other.values {
            if !self.values.contains_key(key) {
                worst = worst.max((value - self.values.get(key).copied().unwrap_or(*value)).abs());
            }
        }
        worst
    }

    /// A stable fingerprint, used only to detect the iteration revisiting a
    /// point it has already been at.
    fn fingerprint(&self) -> Vec<(StockpileId, usize, usize, i64)> {
        self.values
            .iter()
            .map(|(&(pile, key, grade), value)| (pile, key, grade, (value / CYCLE_RESOLUTION).round() as i64))
            .collect()
    }
}

/// How the first estimate of each run is chosen. Each is a documented, purely
/// deterministic function of the scenario - none of them is allowed to look
/// at a SCIP result, which would bias the comparison the whole exercise
/// exists to make.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StartPolicy {
    /// The pile's actual opening grade, and for stock that opens empty the
    /// tonnes-weighted grade of everything that could legally be delivered
    /// into it. The natural first guess.
    OpeningThenSupply,
    /// The lowest grade any deliverable material carries. Pessimistic: it
    /// will refuse grade-limited routes on the first pass and has to discover
    /// them.
    PhysicalFloor,
    /// The highest grade any deliverable material carries. Optimistic: it
    /// will grant grade-limited routes the material may not earn, and the
    /// replay has to take them away again.
    PhysicalCeiling,
}

impl StartPolicy {
    pub(crate) const ALL: [Self; 3] = [Self::OpeningThenSupply, Self::PhysicalFloor, Self::PhysicalCeiling];

    fn initial(self, input: &BlendInput) -> Estimates {
        let grades = input.grades.count();
        let horizon = input.intervals.len();

        // Every material that can legally reach a blended pile, with the
        // tonnage that could arrive. Used for the supply-weighted start and
        // for the physical floor and ceiling.
        let mut supply: Vec<(f64, Vec<f64>)> = Vec::new();
        for candidate in &input.movements {
            let is_delivery = input
                .destinations
                .iter()
                .any(|entry| entry.id == candidate.destination && matches!(entry.kind, crate::model::schedule::optimisation::DestinationKind::Stockpile(_)));
            if !is_delivery {
                continue;
            }
            let fractions: Vec<f64> = (0..grades).map(|g| input.grades.fraction(candidate.material, g).unwrap_or(0.0)).collect();
            let tonnes: f64 = input.ground.iter().map(|source| source.tonnes_t).sum::<f64>().max(1.0);
            supply.push((tonnes, fractions));
        }

        let weighted: Vec<f64> = (0..grades)
            .map(|g| {
                let total: f64 = supply.iter().map(|(tonnes, _)| tonnes).sum();
                if total <= 0.0 {
                    return 0.0;
                }
                supply.iter().map(|(tonnes, fractions)| tonnes * fractions[g]).sum::<f64>() / total
            })
            .collect();
        let floor: Vec<f64> = (0..grades)
            .map(|g| supply.iter().map(|(_, fractions)| fractions[g]).fold(f64::INFINITY, f64::min))
            .map(|value| if value.is_finite() { value } else { 0.0 })
            .collect();
        let ceiling = input.grades.ceilings();

        let fallback: BTreeMap<usize, f64> = (0..grades)
            .map(|g| {
                let value = match self {
                    Self::OpeningThenSupply => weighted[g],
                    Self::PhysicalFloor => floor[g],
                    Self::PhysicalCeiling => ceiling[g],
                };
                (g, value)
            })
            .collect();

        let mut values = BTreeMap::new();
        for pile in &input.piles {
            let (opening_t, opening_q) = pile.total_opening(grades);
            for g in 0..grades {
                // Where stock exists, its actual opening grade is known
                // exactly and is the honest starting point. Where it does
                // not, the pile has no grade at all, and the fallback is a
                // bounded *estimate* - never reported as inventory grade.
                let seed = if opening_t > 0.0 && self == Self::OpeningThenSupply {
                    opening_q[g] / opening_t
                } else {
                    fallback[&g]
                };
                if pile.chunks.is_empty() {
                    for k in 0..horizon {
                        values.insert((pile.id, k, g), seed);
                    }
                } else {
                    for chunk in 0..pile.chunks.len() {
                        let chunk_seed = match pile.chunk_opening.get(chunk) {
                            Some((tonnes, contained)) if *tonnes > 0.0 && self == Self::OpeningThenSupply => contained.get(g).copied().unwrap_or(0.0) / tonnes,
                            _ => fallback[&g],
                        };
                        for k in 0..horizon {
                            values.insert((pile.id, chunk_key(chunk, k, horizon), g), chunk_seed);
                        }
                    }
                }
            }
        }
        Estimates { values, fallback }
    }
}

/// Estimates within this of each other are treated as the same point when
/// looking for a cycle. Coarser than [`CONVERGENCE_TOLERANCE`] on purpose:
/// a cycle is about revisiting a *region*, not an exact repeat.
const CYCLE_RESOLUTION: f64 = 1e-4;

/// The iteration has converged when no estimate moves more than this.
const CONVERGENCE_TOLERANCE: f64 = 1e-6;

/// Fraction of the way from the old estimate to the replayed blend that each
/// update travels.
///
/// Undamped updates (1.0) oscillate on threshold scenarios: an estimate just
/// below a minimum-grade boundary closes the route, which changes what is
/// reclaimed, which puts the blend back above the boundary, which opens it
/// again. Damping does not remove that - the fixed point can genuinely sit on
/// the boundary - but it stops the swing being maximal and lets the cycle
/// detector see a repeat rather than an endless walk.
const DAMPING: f64 = 0.5;

/// What the replay made of one candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CandidateOutcome {
    /// Not a schedule: the movements cannot be replayed physically.
    PhysicallyInvalid,
    /// Replayable, but the blends that resulted differ from the estimates the
    /// MILP assumed, or a grade-dependent condition it granted was not met.
    GradeMismatch,
    /// Every physical, grade and valuation check passed on the replayed
    /// quantities.
    Publishable,
}

/// Why the run stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Termination {
    /// A valid schedule whose estimates stopped moving.
    Converged,
    /// The estimates returned to a point already visited.
    Cycled,
    /// Wall-clock budget exhausted.
    Budget,
    /// Iteration limit reached without convergence.
    IterationLimit,
    /// Every start ended without a physically valid candidate.
    NoValidCandidate,
    /// Every fixed-grade MILP was infeasible. **Not** evidence that the
    /// blended problem is infeasible.
    AllSubproblemsInfeasible,
}

#[derive(Clone, Debug)]
pub(crate) struct IterationRecord {
    pub(crate) start: StartPolicy,
    pub(crate) iteration: usize,
    pub(crate) milp_objective: Option<f64>,
    pub(crate) outcome: Option<CandidateOutcome>,
    pub(crate) replayed_objective: f64,
    /// How far the estimates had to move after this candidate was replayed.
    pub(crate) estimate_drift: f64,
    pub(crate) solve_time: Duration,
}

#[derive(Clone, Debug)]
pub(crate) struct IterativeOutcome {
    /// The best **independently valid** schedule found, if any.
    pub(crate) best: Option<BlendSolution>,
    pub(crate) best_replay: Option<ReplayReport>,
    pub(crate) best_objective: Option<f64>,
    /// Wall time from the start of the run to the first independently valid
    /// schedule.
    pub(crate) time_to_first_valid: Option<Duration>,
    pub(crate) termination: Termination,
    pub(crate) iterations: usize,
    pub(crate) starts: usize,
    pub(crate) sizes: BlendSizes,
    pub(crate) formulation_time: Duration,
    pub(crate) solve_time: Duration,
    pub(crate) replay_time: Duration,
    pub(crate) total_time: Duration,
    pub(crate) history: Vec<IterationRecord>,
}

/// Limits on a run. All three are explicit because an iterative scheme with
/// no budget is not a method, it is a hope.
#[derive(Clone, Copy, Debug)]
pub(crate) struct IterationBudget {
    pub(crate) total: Duration,
    pub(crate) per_solve: Duration,
    pub(crate) max_iterations: usize,
}

impl IterationBudget {
    pub(crate) fn new(total: Duration) -> Self {
        Self {
            total,
            per_solve: total,
            max_iterations: 30,
        }
    }
}

/// The good_lp row sink. Columns must all exist before the model is built, so
/// rows are collected here and posted afterwards.
struct HighsRows {
    variables: ProblemVariables,
    columns: BlendColumns<Variable>,
    sizes: BlendSizes,
    objective: Expression,
    rows: Vec<(Vec<(Variable, f64)>, f64, f64)>,
    estimates: Estimates,
}

impl Rows for HighsRows {
    type Var = Variable;

    fn columns(&mut self) -> &mut BlendColumns<Variable> {
        &mut self.columns
    }

    fn sizes(&mut self) -> &mut BlendSizes {
        &mut self.sizes
    }

    fn valued(&mut self, upper: f64, value: f64, name: &str) -> Variable {
        let _ = name;
        self.sizes.variables += 1;
        let upper = if upper.is_finite() { upper } else { 1e20 };
        let column = self.variables.add(variable().min(0.0).max(upper));
        if value != 0.0 {
            self.objective += value * column;
        }
        column
    }

    fn binary(&mut self, name: &str) -> Variable {
        let _ = name;
        self.sizes.variables += 1;
        self.sizes.binaries += 1;
        self.variables.add(variable().binary())
    }

    fn linear(&mut self, terms: Vec<(Variable, f64)>, lhs: f64, rhs: f64, name: &str) {
        let _ = name;
        if terms.is_empty() {
            return;
        }
        self.sizes.linear_constraints += 1;
        self.rows.push((terms, lhs, rhs));
    }

    /// The linearisation. `Q_recl - g_hat * T_recl = 0`, where `g_hat` is the
    /// current estimate for this pile, mixing key and grade.
    ///
    /// Note what is *not* dropped: the contained-quantity balance and the
    /// grade-box rows around it are still posted by the shared builder, so
    /// the estimate only sets the composition of the draw. The stock it
    /// leaves behind still has to add up.
    fn mix(&mut self, pile: StockpileId, interval: usize, grade: usize, recl_q: Variable, _open_t: Variable, recl_t: Variable, _open_q: Variable, name: &str) {
        let estimate = self.estimates.get(pile, interval, grade);
        self.linear(vec![(recl_q, 1.0), (recl_t, -estimate)], 0.0, 0.0, name);
    }
}

struct MilpResult {
    solution: Option<BlendSolution>,
    objective: Option<f64>,
    sizes: BlendSizes,
    formulation_time: Duration,
    solve_time: Duration,
}

fn solve_fixed_grade(input: &BlendInput, estimates: &Estimates, limit: Duration) -> MilpResult {
    let started = Instant::now();
    let mut sink = HighsRows {
        variables: ProblemVariables::new(),
        columns: BlendColumns::new(),
        sizes: BlendSizes::default(),
        objective: Expression::from(0.0),
        rows: Vec::new(),
        estimates: estimates.clone(),
    };
    formulate(&mut sink, input).expect("iterative formulation has no cancellation signal");

    let HighsRows {
        variables,
        columns,
        sizes,
        objective,
        rows,
        ..
    } = sink;

    let mut model = variables.maximise(objective.clone()).using(good_lp::solvers::highs::highs);
    for (terms, lhs, rhs) in rows {
        let expression: Expression = terms.iter().map(|(column, coefficient)| *coefficient * *column).sum();
        if lhs == rhs {
            model.add_constraint(expression.clone().eq(lhs));
        } else {
            if rhs.is_finite() {
                model.add_constraint(expression.clone().leq(rhs));
            }
            if lhs.is_finite() {
                model.add_constraint(expression.geq(lhs));
            }
        }
    }
    let model = model.set_option("time_limit", limit.as_secs_f64()).set_option("output_flag", false);
    let formulation_time = started.elapsed();

    let solving = Instant::now();
    let solved = model.solve();
    let solve_time = solving.elapsed();

    let Ok(values) = solved else {
        return MilpResult {
            solution: None,
            objective: None,
            sizes,
            formulation_time,
            solve_time,
        };
    };

    let raw_objective = values.eval(objective);
    let mut adjustments = ExtractionAdjustments::default();
    let movements: Vec<MovementRow> = columns
        .movement
        .iter()
        .filter_map(|(&(candidate, interval, segment), column)| {
            let tonnes_t = values.value(*column);
            if tonnes_t.abs() <= 1e-9 {
                if tonnes_t != 0.0 {
                    adjustments.movement(tonnes_t);
                }
                None
            } else {
                Some(MovementRow {
                    candidate,
                    interval,
                    segment,
                    tonnes_t,
                })
            }
        })
        .collect();
    let durations = columns.duration.iter().map(|(&cell, column)| (cell, values.value(*column))).collect();
    let chunks: Vec<ChunkRow> = columns
        .chunk_open_t
        .iter()
        .map(|(&(pile, chunk, interval), open)| {
            let receipts: Vec<(usize, f64)> = columns
                .chunk_recv
                .iter()
                .filter(|((p, c, k, _), _)| *p == pile && *c == chunk && *k == interval)
                .map(|((_, _, _, movement), column)| (*movement, values.value(*column)))
                .filter(|(_, tonnes)| {
                    if tonnes.abs() <= 1e-9 {
                        if *tonnes != 0.0 {
                            adjustments.chunk_receipt(*tonnes);
                        }
                        false
                    } else {
                        true
                    }
                })
                .collect();
            ChunkRow {
                pile,
                chunk,
                interval,
                open_t: values.value(*open),
                reclaimed_t: columns.chunk_recl_t.get(&(pile, chunk, interval)).map(|column| values.value(*column)).unwrap_or(0.0),
                closed: columns
                    .chunk_closed
                    .get(&(pile, chunk, interval))
                    .map(|column| values.value(*column) > 0.5)
                    .unwrap_or(false),
                received_t: receipts.iter().map(|(_, tonnes)| tonnes).sum(),
                receipts,
            }
        })
        .collect();

    MilpResult {
        objective: Some(raw_objective),
        solution: Some(BlendSolution {
            movements,
            durations,
            chunks,
            reported_objective: raw_objective,
            adjustments,
        }),
        sizes,
        formulation_time,
        solve_time,
    }
}

/// Run the iterative method on `input`.
///
/// Returns the best **independently valid** candidate found within the
/// budget, or none, plus the full iteration history so oscillation and
/// stalling are visible rather than summarised away.
pub(crate) fn solve_iterative(input: &BlendInput, budget: IterationBudget, starts: &[StartPolicy]) -> IterativeOutcome {
    let began = Instant::now();
    let mut history = Vec::new();
    let mut best: Option<(f64, BlendSolution, ReplayReport)> = None;
    let mut time_to_first_valid = None;
    let mut sizes = BlendSizes::default();
    let (mut formulation_time, mut solve_time, mut replay_time) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    let mut iterations = 0;
    let mut used_starts = 0;
    let mut termination = Termination::NoValidCandidate;
    let mut any_feasible_subproblem = false;
    let mut any_physically_valid = false;
    // Each start ends for its own reason. The run's reported termination is
    // the reason the *best* start ended, because that is the one whose result
    // is being quoted; the full history is published either way.
    let mut best_start_termination: Option<Termination> = None;

    'starts: for policy in starts {
        used_starts += 1;
        let mut improved_here = false;
        let mut estimates = policy.initial(input);
        let mut seen: Vec<Vec<(StockpileId, usize, usize, i64)>> = Vec::new();

        for iteration in 0..budget.max_iterations {
            let elapsed = began.elapsed();
            if elapsed >= budget.total {
                termination = Termination::Budget;
                break 'starts;
            }
            let remaining = budget.total - elapsed;
            iterations += 1;

            let result = solve_fixed_grade(input, &estimates, budget.per_solve.min(remaining));
            sizes = result.sizes;
            formulation_time += result.formulation_time;
            solve_time += result.solve_time;

            let Some(candidate) = result.solution else {
                // A wrong estimate can make a feasible schedule unreachable,
                // so an infeasible subproblem ends this start - never the run
                // with a claim about the blended problem.
                history.push(IterationRecord {
                    start: *policy,
                    iteration,
                    milp_objective: None,
                    outcome: None,
                    replayed_objective: 0.0,
                    estimate_drift: 0.0,
                    solve_time: result.solve_time,
                });
                continue 'starts;
            };
            any_feasible_subproblem = true;

            let replaying = Instant::now();
            let report = replay(input, &candidate);
            replay_time += replaying.elapsed();

            // Re-estimate from what the pile *actually* held. This happens
            // whatever the outcome: a candidate that failed a grade condition
            // is precisely the one whose blends the next iteration needs.
            let mut updated = estimates.clone();
            for (key, actual) in &report.blends {
                let previous = estimates.get(key.0, key.1, key.2);
                updated.values.insert(*key, previous + DAMPING * (actual - previous));
            }
            let drift = estimates.drift(&updated);

            // Validity and convergence are different questions. A candidate
            // whose physical and grade checks both pass *is* publishable, even
            // if the estimates still have somewhere to go; the drift only
            // decides whether to iterate again.
            let outcome = if !report.is_physically_valid() {
                CandidateOutcome::PhysicallyInvalid
            } else if !report.grade_issues.is_empty() {
                CandidateOutcome::GradeMismatch
            } else {
                CandidateOutcome::Publishable
            };
            if report.is_physically_valid() {
                any_physically_valid = true;
            }

            history.push(IterationRecord {
                start: *policy,
                iteration,
                milp_objective: result.objective,
                outcome: Some(outcome),
                replayed_objective: report.replayed_objective,
                estimate_drift: drift,
                solve_time: result.solve_time,
            });

            // Keep the best candidate that is valid on its *replayed*
            // quantities. The MILP's own objective is not consulted: it was
            // computed against an assumption.
            if report.is_valid() {
                time_to_first_valid.get_or_insert_with(|| began.elapsed());
                let value = report.replayed_objective;
                if best.as_ref().is_none_or(|(previous, _, _)| value > *previous) {
                    best = Some((value, candidate.clone(), report.clone()));
                    improved_here = true;
                }
                if drift <= CONVERGENCE_TOLERANCE {
                    termination = Termination::Converged;
                    if improved_here {
                        best_start_termination = Some(Termination::Converged);
                    }
                    continue 'starts;
                }
            }

            let fingerprint = updated.fingerprint();
            if seen.contains(&fingerprint) {
                termination = Termination::Cycled;
                if improved_here {
                    best_start_termination = Some(Termination::Cycled);
                }
                continue 'starts;
            }
            seen.push(fingerprint);
            estimates = updated;

            if iteration + 1 == budget.max_iterations {
                termination = Termination::IterationLimit;
                if improved_here {
                    best_start_termination = Some(Termination::IterationLimit);
                }
            }
        }
    }

    if let Some(reason) = best_start_termination {
        termination = reason;
    }
    if best.is_none() {
        termination = if !any_feasible_subproblem {
            Termination::AllSubproblemsInfeasible
        } else if !any_physically_valid {
            Termination::NoValidCandidate
        } else {
            termination
        };
    }

    let (best_objective, best_solution, best_replay) = match best {
        Some((value, solution, report)) => (Some(value), Some(solution), Some(report)),
        None => (None, None, None),
    };

    IterativeOutcome {
        best: best_solution,
        best_replay,
        best_objective,
        time_to_first_valid,
        termination,
        iterations,
        starts: used_starts,
        sizes,
        formulation_time,
        solve_time,
        replay_time,
        total_time: began.elapsed(),
        history,
    }
}
