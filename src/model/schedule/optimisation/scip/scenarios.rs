//! Reproducible developer scenarios for the blended-stockpile investigation.
//! The comparison fixtures and seed probe are kept here so the measured
//! backend decision can be checked again with a different SCIP build.

use std::{collections::BTreeMap, time::Duration};

use super::{
    super::blended::{
        grade::{GradeBasis, GradeField, GradeRejection, GradeTable},
        input::{BlendInput, BlendPile, GradeLimit},
        iterative::{CandidateOutcome, IterationBudget, StartPolicy, solve_iterative},
    },
    adapter::{SolveTuning, version},
    experiments::solve_blend,
};
use crate::model::{ReserveAggregation, ReserveField, ReserveFieldId, schedule::optimisation::*};
const FE: usize = 0;

fn grade_fields(names: &[&str]) -> Vec<GradeField> {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| GradeField {
            field: ReserveFieldId(index as u64 + 10),
            name: (*name).to_string(),
            basis: GradeBasis::Percent,
        })
        .collect()
}

/// Materials carry percentages here, converted to fractions by the table.
fn grade_table(names: &[&str], materials: &[(u32, Vec<f64>)]) -> GradeTable {
    let captured: BTreeMap<MaterialId, Vec<Option<f64>>> = materials
        .iter()
        .map(|(id, values)| (MaterialId(*id), values.iter().map(|value| Some(*value)).collect()))
        .collect();
    GradeTable::build(grade_fields(names), &captured).expect("scenario grades are compatible")
}

fn intervals(count: usize, step_h: f64) -> Vec<Interval> {
    (0..count)
        .map(|index| Interval {
            index,
            start_h: index as f64 * step_h,
            end_h: (index + 1) as f64 * step_h,
        })
        .collect()
}

fn loader(id: u32, count: usize, dig: f64, reclaim: f64) -> Loader {
    Loader {
        id: LoaderId(id),
        rates: (0..count)
            .map(|interval| IntervalRate {
                interval,
                dig_tph: dig,
                reclaim_tph: reclaim,
            })
            .collect(),
    }
}

fn truck(id: u32, count: usize, hours: f64) -> TruckClass {
    TruckClass {
        id: TruckClassId(id),
        hours: vec![hours; count],
    }
}

fn candidate(loader: u32, activity: Activity, source: SourceId, material: u32, destination: u32, value: f64) -> MovementCandidate {
    MovementCandidate {
        loader: LoaderId(loader),
        activity,
        source,
        material: MaterialId(material),
        destination: DestinationId(destination),
        truck: TruckClassId(0),
        truck_hours_per_tonne: 0.0,
        routing_rule: RoutingRuleId(0),
        routing_preference: 0,
        cashflow: vec![CashflowContribution {
            rule: CashflowRuleId(0),
            value_per_tonne: value,
        }],
    }
}

fn dump(id: u32, capacity: Option<f64>) -> Destination {
    Destination {
        id: DestinationId(id),
        kind: DestinationKind::Dump,
        capacity_t: capacity,
        crusher_daily_t: Vec::new(),
    }
}

fn pile_destination(id: u32, pile: u32, capacity: Option<f64>) -> Destination {
    Destination {
        id: DestinationId(id),
        kind: DestinationKind::Stockpile(StockpileId(pile)),
        capacity_t: capacity,
        crusher_daily_t: Vec::new(),
    }
}

fn reclaim_task(id: u32, loader: u32, pile: u32, end_h: f64, maximum: Option<f64>) -> Task {
    Task {
        id: TaskId(id),
        loader: LoaderId(loader),
        priority: 0,
        window_start_h: 0.0,
        window_end_h: end_h,
        kind: TaskKind::Reclaim {
            approved_sources: vec![StockpileId(pile)],
            maximum_t: maximum,
        },
    }
}

fn dig_task(id: u32, loader: u32, ground: &[u32], end_h: f64) -> Task {
    Task {
        id: TaskId(id),
        loader: LoaderId(loader),
        priority: 0,
        window_start_h: 0.0,
        window_end_h: end_h,
        kind: TaskKind::Dig {
            sequence: ground.iter().map(|id| GroundId(*id)).collect(),
        },
    }
}

fn run(input: &BlendInput, seconds: u64) -> super::experiments::BlendRun {
    let finished = solve_blend(input, SolveTuning::new(Some(Duration::from_secs(seconds))));
    let replay = finished.replay.as_ref();
    println!(
        "SOLVE status={:?} obj={:?} bound={:.4} gap={:?} nodes={} vars={} bin={} lin={} nonlin={} form={:?} solve={:.3}s replay_issues={} max_abs_resid={:.3e} obj_diff={:.3e}",
        finished.report.status,
        finished.report.objective,
        finished.report.bound,
        finished.report.gap,
        finished.report.nodes,
        finished.sizes.variables,
        finished.sizes.binaries,
        finished.sizes.linear_constraints,
        finished.sizes.nonlinear_constraints,
        finished.formulation_time,
        finished.report.solve_time,
        replay.map(|entry| entry.issues.len()).unwrap_or(0),
        replay.map(|entry| entry.max_absolute_residual_t).unwrap_or(0.0),
        replay.map(|entry| entry.objective_difference).unwrap_or(0.0),
    );
    if let Some(entry) = replay {
        for issue in &entry.issues {
            println!("   ISSUE {issue}");
        }
        for closing in &entry.closing {
            println!("   CLOSING pile={} t={:.4} contained={:?}", closing.pile.0, closing.tonnes_t, closing.contained);
        }
    }
    finished
}

/// §7 - run both blended methods on the *same* scenario and print one row of
/// the comparison table.
///
/// The two objectives are comparable with each other (same stockpile
/// semantics, same constraints, same input) but neither is comparable with a
/// parcel-model objective.
fn compare(label: &str, input: &BlendInput, seconds: u64) {
    let scip = run(input, seconds);
    let scip_replay = scip.replay.as_ref();
    let scip_valid = scip_replay.is_some_and(|entry| entry.is_valid());

    let outcome = solve_iterative(input, IterationBudget::new(Duration::from_secs(seconds)), &StartPolicy::ALL);
    let iter_replay = outcome.best_replay.as_ref();

    println!("=== COMPARE {label} ===");
    println!(
        "  SCIP     valid={scip_valid} obj={:?} bound={:.4} gap={:?} status={:?} nodes={} vars={} bin={} nonlin={} form={:?} solve={:.3}s replay_issues={} grade_issues={} max_abs_resid={:.3e}",
        scip.report.objective,
        scip.report.bound,
        scip.report.gap,
        scip.report.status,
        scip.report.nodes,
        scip.sizes.variables,
        scip.sizes.binaries,
        scip.sizes.nonlinear_constraints,
        scip.formulation_time,
        scip.report.solve_time,
        scip_replay.map(|entry| entry.issues.len()).unwrap_or(0),
        scip_replay.map(|entry| entry.grade_issues.len()).unwrap_or(0),
        scip_replay.map(|entry| entry.max_absolute_residual_t).unwrap_or(0.0),
    );
    if let (Some(solution), Some(replay)) = (&scip.solution, scip_replay) {
        println!(
            "  SCIP     raw_obj={:.9} published_obj={:.9} delta={:.3e} omitted_movement={} / {:.3e}t (max {:.3e}t) omitted_chunk_receipts={} / {:.3e}t (max {:.3e}t)",
            solution.reported_objective,
            replay.replayed_objective,
            replay.objective_difference,
            solution.adjustments.movement_count,
            solution.adjustments.movement_total_t,
            solution.adjustments.movement_max_t,
            solution.adjustments.chunk_receipt_count,
            solution.adjustments.chunk_receipt_total_t,
            solution.adjustments.chunk_receipt_max_t,
        );
    }
    println!(
        "  ITER     valid={} obj={:?} first_valid={:?} term={:?} iters={} starts={} vars={} bin={} form={:?} solve={:?} replay={:?} total={:?} replay_issues={} grade_issues={} max_abs_resid={:.3e}",
        iter_replay.is_some_and(|entry| entry.is_valid()),
        outcome.best_objective,
        outcome.time_to_first_valid,
        outcome.termination,
        outcome.iterations,
        outcome.starts,
        outcome.sizes.variables,
        outcome.sizes.binaries,
        outcome.formulation_time,
        outcome.solve_time,
        outcome.replay_time,
        outcome.total_time,
        iter_replay.map(|entry| entry.issues.len()).unwrap_or(0),
        iter_replay.map(|entry| entry.grade_issues.len()).unwrap_or(0),
        iter_replay.map(|entry| entry.max_absolute_residual_t).unwrap_or(0.0),
    );
    if let (Some(solution), Some(replay)) = (&outcome.best, iter_replay) {
        println!(
            "  ITER     raw_obj={:.9} published_obj={:.9} delta={:.3e} omitted_movement={} / {:.3e}t (max {:.3e}t) omitted_chunk_receipts={} / {:.3e}t (max {:.3e}t)",
            solution.reported_objective,
            replay.replayed_objective,
            replay.objective_difference,
            solution.adjustments.movement_count,
            solution.adjustments.movement_total_t,
            solution.adjustments.movement_max_t,
            solution.adjustments.chunk_receipt_count,
            solution.adjustments.chunk_receipt_total_t,
            solution.adjustments.chunk_receipt_max_t,
        );
    }
    let mut invalid = 0;
    let mut mismatch = 0;
    let mut publishable = 0;
    let mut infeasible = 0;
    for record in &outcome.history {
        match record.outcome {
            None => infeasible += 1,
            Some(CandidateOutcome::PhysicallyInvalid) => invalid += 1,
            Some(CandidateOutcome::GradeMismatch) => mismatch += 1,
            Some(CandidateOutcome::Publishable) => publishable += 1,
        }
    }
    println!("  ITER outcomes: publishable={publishable} grade_mismatch={mismatch} physically_invalid={invalid} subproblem_infeasible={infeasible}");
    for record in &outcome.history {
        println!(
            "    [{:?} #{}] milp={:?} outcome={:?} replayed={:.2} drift={:.3e} solve={:?}",
            record.start, record.iteration, record.milp_objective, record.outcome, record.replayed_objective, record.estimate_drift, record.solve_time
        );
    }
}

/// §10.1 - a known blend. 1,000 t at 60% Fe, receiving 500 t at 50% and
/// 500 t at 70%, must remain 60%.
#[test]
fn known_blend_is_preserved() {
    let grades = grade_table(&["Fe"], &[(1, vec![60.0]), (2, vec![50.0]), (3, vec![70.0])]);
    let input = BlendInput {
        intervals: intervals(3, 1.0),
        segments_per_interval: 1,
        grades,
        piles: vec![BlendPile::combine(StockpileId(0), 5_000.0, &[(1_000.0, vec![0.60])], 1)],
        loaders: vec![loader(0, 3, 500.0, 0.0)],
        tasks: vec![dig_task(0, 0, &[1, 2], 3.0)],
        ground: vec![
            GroundSource {
                id: GroundId(1),
                tonnes_t: 500.0,
                material: vec![MaterialShare {
                    material: MaterialId(2),
                    fraction: 1.0,
                }],
            },
            GroundSource {
                id: GroundId(2),
                tonnes_t: 500.0,
                material: vec![MaterialShare {
                    material: MaterialId(3),
                    fraction: 1.0,
                }],
            },
        ],
        destinations: vec![pile_destination(0, 0, None)],
        trucks: vec![truck(0, 3, 1_000.0)],
        movements: vec![
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(1)), 2, 0, 1.0),
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(2)), 3, 0, 1.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: Vec::new(),
    };
    let finished = run(&input, 30);
    let replay = finished.replay.expect("an incumbent");
    assert!(replay.is_valid(), "replay issues: {:?}", replay.issues);
    let closing = &replay.closing[0];
    assert!((closing.tonnes_t - 2_000.0).abs() < 1e-6, "closing tonnes {}", closing.tonnes_t);
    let blended = closing.contained[FE] / closing.tonnes_t;
    assert!((blended - 0.60).abs() < 1e-6, "blended grade {blended} should stay 0.60");
}

/// §10.3 and §10.5 - an empty pile supplies nothing, and material received
/// during an interval cannot be reclaimed within it.
#[test]
fn empty_pile_and_release_timing() {
    let grades = grade_table(&["Fe"], &[(1, vec![60.0])]);
    let input = BlendInput {
        intervals: intervals(2, 1.0),
        segments_per_interval: 1,
        grades,
        // Opens EMPTY. Anything reclaimed in interval 0 would be phantom.
        piles: vec![BlendPile::combine(StockpileId(0), 5_000.0, &[], 1)],
        loaders: vec![loader(0, 2, 500.0, 0.0), loader(1, 2, 0.0, 500.0)],
        tasks: vec![dig_task(0, 0, &[1], 2.0), reclaim_task(1, 1, 0, 2.0, None)],
        ground: vec![GroundSource {
            id: GroundId(1),
            tonnes_t: 500.0,
            material: vec![MaterialShare {
                material: MaterialId(1),
                fraction: 1.0,
            }],
        }],
        destinations: vec![pile_destination(0, 0, None), dump(1, None)],
        trucks: vec![truck(0, 2, 10_000.0)],
        movements: vec![
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(1)), 1, 0, 1.0),
            // Reclaiming pays far better, so the solver wants to do it early.
            candidate(1, Activity::Reclaim, SourceId::Stockpile(StockpileId(0)), 1, 1, 100.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: Vec::new(),
    };
    let finished = run(&input, 30);
    let solution = finished.solution.expect("an incumbent");
    let replay = finished.replay.expect("a replay");
    assert!(replay.is_valid(), "replay issues: {:?}", replay.issues);

    let reclaimed_in_first: f64 = solution
        .movements
        .iter()
        .filter(|row| row.interval == 0 && input.movements[row.candidate].activity == Activity::Reclaim)
        .map(|row| row.tonnes_t)
        .sum();
    assert!(reclaimed_in_first < 1e-6, "reclaimed {reclaimed_in_first} t from an empty pile in interval 0");
}

/// §10.4 - a partial reclaim leaves the remaining inventory at the same
/// blend, and §10.6 - two grades are both conserved.
#[test]
fn partial_reclaim_keeps_the_blend_across_two_grades() {
    let grades = grade_table(&["Fe", "SiO2"], &[(1, vec![60.0, 5.0])]);
    let input = BlendInput {
        intervals: intervals(2, 1.0),
        segments_per_interval: 1,
        grades,
        piles: vec![BlendPile::combine(StockpileId(0), 5_000.0, &[(1_000.0, vec![0.60, 0.05])], 2)],
        loaders: vec![loader(1, 2, 0.0, 400.0)],
        tasks: vec![reclaim_task(1, 1, 0, 2.0, Some(400.0))],
        ground: Vec::new(),
        destinations: vec![dump(1, None)],
        trucks: vec![truck(0, 2, 10_000.0)],
        movements: vec![candidate(1, Activity::Reclaim, SourceId::Stockpile(StockpileId(0)), 1, 1, 10.0)],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: Vec::new(),
    };
    let finished = run(&input, 30);
    let replay = finished.replay.expect("an incumbent");
    assert!(replay.is_valid(), "replay issues: {:?}", replay.issues);
    let closing = &replay.closing[0];
    // The cap allows 400 t of the 1,000 t, so 600 t must remain at the
    // original blend in BOTH grades.
    assert!((closing.tonnes_t - 600.0).abs() < 1e-4, "closing tonnes {}", closing.tonnes_t);
    let fe = closing.contained[0] / closing.tonnes_t;
    let si = closing.contained[1] / closing.tonnes_t;
    assert!((fe - 0.60).abs() < 1e-6, "Fe drifted to {fe}");
    assert!((si - 0.05).abs() < 1e-6, "SiO2 drifted to {si}");
}

/// §3 - a **full** pile receiving material while its released stock is being
/// reclaimed. Hand-checkable, and the direct regression test for the
/// occupancy correction.
///
/// One interval, one segment, so the decision cannot be deferred. The pile
/// opens at its 1,000 t capacity. Digging pays 10/t into the pile; reclaim
/// pays 1/t out of it.
///
/// Physical occupancy is `1,000 + receipts - reclaim`, so digging 500 t while
/// reclaiming 500 t holds the pile at exactly 1,000 t throughout and is
/// legal. Objective 500 x 10 + 500 x 1 = 5,500.
///
/// The previous formulation checked `opening + receipts <= capacity` and so
/// saw 1,500 t. It refused the receipt entirely, leaving only the 500 t
/// reclaim and an objective of 500 - production refused purely because the
/// pile began full.
#[test]
fn a_full_pile_can_receive_while_it_is_reclaimed() {
    let input = BlendInput {
        intervals: intervals(1, 1.0),
        segments_per_interval: 1,
        grades: grade_table(&["Fe"], &[(1, vec![60.0])]),
        piles: vec![BlendPile::combine(StockpileId(0), 1_000.0, &[(1_000.0, vec![0.60])], 1)],
        loaders: vec![loader(0, 1, 500.0, 0.0), loader(1, 1, 0.0, 500.0)],
        tasks: vec![dig_task(0, 0, &[1], 1.0), reclaim_task(1, 1, 0, 1.0, None)],
        ground: vec![GroundSource {
            id: GroundId(1),
            tonnes_t: 500.0,
            material: vec![MaterialShare {
                material: MaterialId(1),
                fraction: 1.0,
            }],
        }],
        destinations: vec![pile_destination(0, 0, None), dump(1, None)],
        trucks: vec![truck(0, 1, 10_000.0)],
        movements: vec![
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(1)), 1, 0, 10.0),
            candidate(1, Activity::Reclaim, SourceId::Stockpile(StockpileId(0)), 1, 1, 1.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: Vec::new(),
    };
    let finished = run(&input, 30);
    let solution = finished.solution.expect("an incumbent");
    let replay = finished.replay.expect("a replay");
    assert!(replay.is_valid(), "replay issues: {:?}", replay.issues);

    let dug: f64 = solution
        .movements
        .iter()
        .filter(|row| input.movements[row.candidate].activity == Activity::Dig)
        .map(|row| row.tonnes_t)
        .sum();
    let reclaimed: f64 = solution
        .movements
        .iter()
        .filter(|row| input.movements[row.candidate].activity == Activity::Reclaim)
        .map(|row| row.tonnes_t)
        .sum();
    assert!(dug > 500.0 - 1e-4, "a full pile refused a receipt it had room for: dug {dug} t");
    assert!(reclaimed > 500.0 - 1e-4, "reclaimed only {reclaimed} t");
    assert!(
        (replay.replayed_objective - 5_500.0).abs() < 1e-3,
        "objective {} should be 5,500",
        replay.replayed_objective
    );
    // And the pile never exceeded its capacity: closing stock is back to
    // exactly the 1,000 t it opened with.
    let closing = &replay.closing[0];
    assert!((closing.tonnes_t - 1_000.0).abs() < 1e-4, "closing tonnes {}", closing.tonnes_t);
}

/// §4 - **mandatory** authored bar priority on one loader.
///
/// One loader, one hour, 500 t of capacity, two bars whose windows both cover
/// the interval:
///
/// | bar | priority | block | value |
/// |---|---|---|---|
/// | 10 | 0 | ground 1, 500 t | 1 / t |
/// | 11 | 1 | ground 2, 500 t | 100 / t |
///
/// Bar 10 has work available, so bar 11 may not be worked at all. The
/// schedule is worth 500, not 50,000. A model that treated priority as
/// advisory - or that let the loader stand down on bar 10 to make bar 11 the
/// only live option - would return the hundredfold answer.
#[test]
fn authored_bar_priority_is_mandatory() {
    let ground = |id: u32, material: u32| GroundSource {
        id: GroundId(id),
        tonnes_t: 500.0,
        material: vec![MaterialShare {
            material: MaterialId(material),
            fraction: 1.0,
        }],
    };
    let mut cheap = dig_task(10, 0, &[1], 1.0);
    cheap.priority = 0;
    let mut rich = dig_task(11, 0, &[2], 1.0);
    rich.priority = 1;
    let input = BlendInput {
        intervals: intervals(1, 1.0),
        segments_per_interval: 1,
        grades: grade_table(&["Fe"], &[(1, vec![60.0]), (2, vec![60.0])]),
        piles: Vec::new(),
        loaders: vec![loader(0, 1, 500.0, 0.0)],
        tasks: vec![cheap, rich],
        ground: vec![ground(1, 1), ground(2, 2)],
        destinations: vec![dump(1, None)],
        trucks: vec![truck(0, 1, 10_000.0)],
        movements: vec![
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(1)), 1, 1, 1.0),
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(2)), 2, 1, 100.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: Vec::new(),
    };
    let finished = run(&input, 30);
    let solution = finished.solution.expect("an incumbent");
    let replay = finished.replay.expect("a replay");
    assert!(replay.is_valid(), "replay issues: {:?}", replay.issues);

    let from_rich: f64 = solution
        .movements
        .iter()
        .filter(|row| input.movements[row.candidate].source == SourceId::Ground(GroundId(2)))
        .map(|row| row.tonnes_t)
        .sum();
    assert!(from_rich < 1e-6, "a lower-priority bar was worked for {from_rich} t while the authored bar still had work");
    assert!(
        (replay.replayed_objective - 500.0).abs() < 1e-3,
        "objective {} should be 500, not the 50,000 an unordered model would take",
        replay.replayed_objective
    );
}

/// §4 - authored bar windows are exact, not rounded out to the interval grid.
///
/// The bar runs 0 h to 0.5 h while the calendar interval runs 0 h to 1 h, so
/// the bar does not cover the interval and no work may happen under it. The
/// previous overlap test silently granted the bar the whole hour.
#[test]
fn a_partial_bar_window_does_not_win_the_whole_interval() {
    let mut half = dig_task(0, 0, &[1], 0.5);
    half.window_end_h = 0.5;
    let input = BlendInput {
        intervals: intervals(1, 1.0),
        segments_per_interval: 1,
        grades: grade_table(&["Fe"], &[(1, vec![60.0])]),
        piles: Vec::new(),
        loaders: vec![loader(0, 1, 500.0, 0.0)],
        tasks: vec![half],
        ground: vec![GroundSource {
            id: GroundId(1),
            tonnes_t: 500.0,
            material: vec![MaterialShare {
                material: MaterialId(1),
                fraction: 1.0,
            }],
        }],
        destinations: vec![dump(1, None)],
        trucks: vec![truck(0, 1, 10_000.0)],
        movements: vec![candidate(0, Activity::Dig, SourceId::Ground(GroundId(1)), 1, 1, 10.0)],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: Vec::new(),
    };
    let finished = run(&input, 30);
    let replay = finished.replay.expect("a replay");
    assert!(replay.is_valid(), "replay issues: {:?}", replay.issues);
    assert!(
        replay.replayed_objective.abs() < 1e-6,
        "a bar covering half the interval moved {} worth of material",
        replay.replayed_objective
    );
}

/// §10.2 and §6 - a minimum grade the pile can only reach by taking enough
/// high-grade supply. The optimiser has to choose the blend, not just the
/// tonnage.
#[test]
fn minimum_grade_forces_a_blending_decision() {
    // Pile opens with 1,000 t of 50% Fe. The crusher needs 60%. Only by
    // receiving enough 80% material can the pile's blend qualify.
    let grades = grade_table(&["Fe"], &[(1, vec![50.0]), (2, vec![80.0])]);
    let build = |dig_rate: f64| BlendInput {
        intervals: intervals(3, 1.0),
        segments_per_interval: 1,
        grades: grade_table(&["Fe"], &[(1, vec![50.0]), (2, vec![80.0])]),
        piles: vec![BlendPile::combine(StockpileId(0), 10_000.0, &[(1_000.0, vec![0.50])], 1)],
        loaders: vec![loader(0, 3, dig_rate, 0.0), loader(1, 3, 0.0, 500.0)],
        tasks: vec![dig_task(0, 0, &[2], 3.0), reclaim_task(1, 1, 0, 3.0, None)],
        ground: vec![GroundSource {
            id: GroundId(2),
            tonnes_t: 4_000.0,
            material: vec![MaterialShare {
                material: MaterialId(2),
                fraction: 1.0,
            }],
        }],
        destinations: vec![pile_destination(0, 0, None), dump(1, None)],
        trucks: vec![truck(0, 3, 100_000.0)],
        movements: vec![
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(2)), 2, 0, 0.1),
            candidate(1, Activity::Reclaim, SourceId::Stockpile(StockpileId(0)), 1, 1, 50.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: vec![GradeLimit {
            destination: DestinationId(1),
            grade: FE,
            minimum: 0.60,
            inclusive: true,
        }],
    };
    let _ = grades;

    let input = build(2_000.0);
    let finished = run(&input, 60);
    let replay = finished.replay.expect("an incumbent");
    assert!(replay.is_valid(), "replay issues: {:?}", replay.issues);
    let solution = finished.solution.expect("an incumbent");

    // Any interval that reclaimed must have done so at 60% or better; the
    // replay recomputes the grade by division and would have complained.
    let reclaimed: f64 = solution
        .movements
        .iter()
        .filter(|row| input.movements_activity(row.candidate) == Activity::Reclaim)
        .map(|row| row.tonnes_t)
        .sum();
    assert!(reclaimed > 1e-6, "the scenario is pointless if nothing is reclaimed");
}

/// §10.7 - direct mining and reclaim compete for one crusher budget.
#[test]
fn dig_and_reclaim_share_the_crusher_budget() {
    let grades = grade_table(&["Fe"], &[(1, vec![60.0])]);
    let crusher = Destination {
        id: DestinationId(1),
        kind: DestinationKind::Crusher,
        capacity_t: None,
        crusher_daily_t: vec![Some(600.0)],
    };
    let input = BlendInput {
        intervals: intervals(2, 1.0),
        segments_per_interval: 1,
        grades,
        piles: vec![BlendPile::combine(StockpileId(0), 5_000.0, &[(2_000.0, vec![0.60])], 1)],
        loaders: vec![loader(0, 2, 500.0, 0.0), loader(1, 2, 0.0, 500.0)],
        tasks: vec![dig_task(0, 0, &[1], 2.0), reclaim_task(1, 1, 0, 2.0, None)],
        ground: vec![GroundSource {
            id: GroundId(1),
            tonnes_t: 2_000.0,
            material: vec![MaterialShare {
                material: MaterialId(1),
                fraction: 1.0,
            }],
        }],
        destinations: vec![crusher],
        trucks: vec![truck(0, 2, 100_000.0)],
        movements: vec![
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(1)), 1, 1, 1.0),
            candidate(1, Activity::Reclaim, SourceId::Stockpile(StockpileId(0)), 1, 1, 1.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: Vec::new(),
    };
    let finished = run(&input, 30);
    let replay = finished.replay.expect("an incumbent");
    assert!(replay.is_valid(), "replay issues: {:?}", replay.issues);
    let solution = finished.solution.expect("an incumbent");
    let delivered: f64 = solution.movements.iter().map(|row| row.tonnes_t).sum();
    assert!(delivered <= 600.0 + 1e-4, "delivered {delivered} t against a 600 t shared daily budget");
    // Both loaders could supply 1,000 t each; the budget is what binds.
    assert!(delivered > 599.0, "the budget should be filled, got {delivered}");
}

/// §5 - unsupported weighting semantics are rejected with a reason rather
/// than silently coerced.
#[test]
fn grade_compatibility_is_enforced() {
    let tonnes = ReserveFieldId(1);
    let volume = ReserveFieldId(2);
    let fields = vec![
        ReserveField {
            id: tonnes,
            name: "Tonnes".into(),
            aggregation: ReserveAggregation::Sum,
        },
        ReserveField {
            id: volume,
            name: "Volume".into(),
            aggregation: ReserveAggregation::Sum,
        },
        ReserveField {
            id: ReserveFieldId(3),
            name: "Fe".into(),
            aggregation: ReserveAggregation::WeightedAverage { weight_field: tonnes },
        },
        ReserveField {
            id: ReserveFieldId(4),
            name: "Cu ppm".into(),
            aggregation: ReserveAggregation::WeightedAverage { weight_field: volume },
        },
        ReserveField {
            id: ReserveFieldId(5),
            name: "Contained Fe".into(),
            aggregation: ReserveAggregation::Sum,
        },
        ReserveField {
            id: ReserveFieldId(6),
            name: "Rock Type".into(),
            aggregation: ReserveAggregation::Category,
        },
    ];

    // Tonnes-weighted: accepted.
    assert!(GradeField::accept(ReserveFieldId(3), GradeBasis::Percent, &fields, Some(tonnes)).is_ok());

    // Weighted by something else: rejected, and the message names both.
    let rejected = GradeField::accept(ReserveFieldId(4), GradeBasis::Fraction, &fields, Some(tonnes)).unwrap_err();
    assert!(matches!(rejected, GradeRejection::ForeignWeight { .. }), "{rejected:?}");
    assert!(rejected.message().contains("Volume") && rejected.message().contains("Tonnes"));

    // A summed quantity is not a grade.
    let rejected = GradeField::accept(ReserveFieldId(5), GradeBasis::Fraction, &fields, Some(tonnes)).unwrap_err();
    assert!(matches!(rejected, GradeRejection::Summed { .. }), "{rejected:?}");

    // A category is not a number.
    let rejected = GradeField::accept(ReserveFieldId(6), GradeBasis::Fraction, &fields, Some(tonnes)).unwrap_err();
    assert!(matches!(rejected, GradeRejection::Categorical { .. }), "{rejected:?}");

    // No tonnage basis configured: nothing can be verified.
    let rejected = GradeField::accept(ReserveFieldId(3), GradeBasis::Fraction, &fields, None).unwrap_err();
    assert!(matches!(rejected, GradeRejection::NoTonnageBasis), "{rejected:?}");

    // A missing value is not zero.
    let captured = BTreeMap::from([(MaterialId(1), vec![None])]);
    let rejected = GradeTable::build(grade_fields(&["Fe"]), &captured).unwrap_err();
    assert!(matches!(rejected, GradeRejection::MissingValue { .. }), "{rejected:?}");
}

#[test]
fn report_backend_version() {
    println!("BACKEND {}", version());
}

/// §2 of the verification stage: does *this* linked SCIP build still return a
/// seeded, inferior feasible solution as `Optimal` on a nonlinear model?
///
/// Reproducer: maximise `c` subject to `c * t = m`, `t = 10 + 10 b`, `m = 6`,
/// `b` binary. True optimum `c = 0.6` at `b = 0`; the seed is the feasible
/// `b = 1` point (`c = 0.3`).
///
/// SCIP 10.0.2 (the bundled baseline) returned 0.3 reported Optimal with a
/// matching dual bound and zero nodes. This probe prints the same matrix for
/// whatever build it runs against, including the two mitigations recorded in
/// the experiment document. Only the unseeded solve is asserted: the seeded
/// configurations are the measurement, and pre-asserting an unknown build's
/// behaviour would assume the answer.
#[test]
fn seeded_nonlinear_solution_behaviour_on_this_build() {
    use russcip::{Model, ObjSense, ProblemOrSolving, prelude::*};

    enum Param {
        Bool(&'static str, bool),
        Int(&'static str, i32),
    }
    // (label, seeded, parameter override)
    let configs: &[(&str, bool, Option<Param>)] = &[
        ("unseeded, defaults", false, None),
        ("seeded, defaults", true, None),
        ("seeded, misc/allowweakdualreds=false", true, Some(Param::Bool("misc/allowweakdualreds", false))),
        ("seeded, presolving/maxrounds=0", true, Some(Param::Int("presolving/maxrounds", 0))),
    ];
    for (label, seeded, parameter) in configs {
        let mut model = Model::new()
            .hide_output()
            .include_default_plugins()
            .create_prob("seed_reproducer")
            .set_obj_sense(ObjSense::Maximize);
        let c = model.add(var().cont(0.0..=1.0).obj(1.0).name("grade"));
        let t = model.add(var().cont(1.0..=20.0).name("tonnes"));
        let m = model.add(var().cont(6.0..=6.0).name("contained"));
        let b = model.add(var().bin().name("pick"));
        // t = 10 + 10 b, so t is 10 or 20.
        model.add_cons(vec![&t, &b], &[1.0, -10.0], 10.0, 10.0, "pick_tonnes");
        // grade * tonnes - contained = 0: the nonconvex bilinear equality under
        // investigation, in the exact shape of the original smoke test.
        model.add_cons_quadratic(vec![&m], &mut [-1.0], vec![&c], vec![&t], &mut [1.0], 0.0, 0.0, "blend");
        model = match parameter {
            Some(Param::Bool(name, value)) => model.set_bool_param(name, *value).unwrap_or_else(|_| panic!("{name} is a bool parameter")),
            Some(Param::Int(name, value)) => model.set_int_param(name, *value).unwrap_or_else(|_| panic!("{name} is an int parameter")),
            None => model,
        };
        if *seeded {
            let solution = model.create_orig_sol();
            solution.set_val(&c, 0.3);
            solution.set_val(&t, 20.0);
            solution.set_val(&m, 6.0);
            solution.set_val(&b, 1.0);
            model.add_sol(solution).expect("the seed point is feasible");
        }
        let solved = model.solve();
        let objective = solved.obj_val();
        let report = super::adapter::SolveReport::read(&solved);
        let honest = (objective - 0.6).abs() < 1e-6;
        println!(
            "SEED-PROBE [{label}] on {}: objective={objective:.9} bound={:.9} status={:?} gap={:?} nodes={} correct={honest}",
            version(),
            report.bound,
            report.status,
            report.gap,
            report.nodes,
        );
        if !*seeded && parameter.is_none() {
            assert!(honest, "the unseeded solve must find 0.6; the reproducer is mis-stated otherwise");
        }
    }
}

impl BlendInput {
    fn movements_activity(&self, index: usize) -> Activity {
        self.movements[index].activity
    }
}

// ---------------------------------------------------------------------------
// §3 Experiment 1: the SAME parcel MILP, solved by both backends.
// ---------------------------------------------------------------------------

fn spec(end_h: f64, step_h: f64, parcel_t: f64) -> HorizonSpec {
    HorizonSpec {
        end_h,
        regular_step_h: step_h,
        parcel_target_t: parcel_t,
        event_segments: None,
        tolerances: NumericalTolerances::default(),
    }
}

fn reclaim_candidate(loader: u32, material: u32, destination: u32, value: f64) -> MovementCandidate {
    MovementCandidate {
        loader: LoaderId(loader),
        activity: Activity::Reclaim,
        source: SourceId::Stockpile(StockpileId(1)),
        material: MaterialId(material),
        destination: DestinationId(destination),
        truck: TruckClassId(1),
        truck_hours_per_tonne: 0.02,
        routing_rule: RoutingRuleId(3),
        routing_preference: 0,
        cashflow: vec![CashflowContribution {
            rule: CashflowRuleId(3),
            value_per_tonne: value,
        }],
    }
}

fn expanded_fixture(hours: f64, step_h: f64, parcel_t: f64) -> OptimisationInput {
    let horizon = spec(hours, step_h, parcel_t);
    let intervals = build_intervals(horizon, &[(0.0, hours)], &[]).unwrap();
    let rates = |loader| Loader {
        id: LoaderId(loader),
        rates: intervals
            .iter()
            .map(|interval| IntervalRate {
                interval: interval.index,
                dig_tph: 40.0,
                reclaim_tph: 40.0,
            })
            .collect(),
    };
    let mut input = OptimisationInput {
        horizon,
        intervals: intervals.clone(),
        materials: vec![
            Material {
                id: MaterialId(1),
                label: "A".into(),
            },
            Material {
                id: MaterialId(2),
                label: "B".into(),
            },
        ],
        loaders: vec![rates(1), rates(2), rates(3), rates(4)],
        tasks: vec![Task {
            id: TaskId(4),
            loader: LoaderId(4),
            priority: 1,
            window_start_h: 0.0,
            window_end_h: hours,
            kind: TaskKind::Reclaim {
                approved_sources: vec![StockpileId(1)],
                maximum_t: None,
            },
        }],
        ground: Vec::new(),
        stockpiles: vec![Stockpile {
            id: StockpileId(1),
            capacity_t: 1_000.0,
            opening: vec![],
            order: ReclaimOrder::Fifo,
        }],
        destinations: vec![
            Destination {
                id: DestinationId(1),
                kind: DestinationKind::Stockpile(StockpileId(1)),
                capacity_t: Some(1_000.0),
                crusher_daily_t: vec![],
            },
            Destination {
                id: DestinationId(2),
                kind: DestinationKind::Crusher,
                capacity_t: None,
                crusher_daily_t: vec![Some(300.0)],
            },
        ],
        trucks: vec![TruckClass {
            id: TruckClassId(1),
            hours: vec![10.0; intervals.len()],
        }],
        movements: vec![reclaim_candidate(4, 1, 2, 1.0), reclaim_candidate(4, 2, 2, 2.0)],
    };
    for loader in 1..=3 {
        input.tasks.insert(
            0,
            Task {
                id: TaskId(loader),
                loader: LoaderId(loader),
                priority: 0,
                window_start_h: 0.0,
                window_end_h: hours,
                kind: TaskKind::Dig { sequence: vec![GroundId(loader)] },
            },
        );
        let material = MaterialId(1 + loader % 2);
        input.ground.push(GroundSource {
            id: GroundId(loader),
            tonnes_t: 180.0,
            material: vec![MaterialShare { material, fraction: 1.0 }],
        });
        input.movements.push(MovementCandidate {
            loader: LoaderId(loader),
            activity: Activity::Dig,
            source: SourceId::Ground(GroundId(loader)),
            material,
            destination: DestinationId(1),
            truck: TruckClassId(1),
            truck_hours_per_tonne: 0.01 + f64::from(loader) * 0.001,
            routing_rule: RoutingRuleId(1),
            routing_preference: 0,
            cashflow: vec![CashflowContribution {
                rule: CashflowRuleId(1),
                value_per_tonne: 1.0,
            }],
        });
        input.movements.push(MovementCandidate {
            loader: LoaderId(loader),
            activity: Activity::Dig,
            source: SourceId::Ground(GroundId(loader)),
            material,
            destination: DestinationId(2),
            truck: TruckClassId(1),
            truck_hours_per_tonne: 0.015,
            routing_rule: RoutingRuleId(2),
            routing_preference: 1,
            cashflow: vec![CashflowContribution {
                rule: CashflowRuleId(2),
                value_per_tonne: 2.0,
            }],
        });
    }
    input
}

/// Experiment 1. Both backends, same model, same budget.
#[test]
#[ignore = "benchmark"]
fn parcel_backend_comparison() {
    let budget = Duration::from_secs(60);
    for hours in [4.0_f64, 8.0, 12.0, 24.0] {
        let input = expanded_fixture(hours, 1.0, 50.0);
        let directory = std::env::temp_dir().join("incline-scip-experiment");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("parcel_{hours}h.mps"));

        // Ask the accepted backend to write the model it is about to solve.
        super::super::highs::mps_export::request(path.clone());
        let limits = SolveLimits {
            time: Some(budget),
            ..SolveLimits::default()
        };
        let started = std::time::Instant::now();
        let highs = super::super::solve(&input, limits, &CancellationToken::default());
        let highs_wall = started.elapsed();
        let issues = super::super::validate_solution(&input, &highs);
        println!(
            "EXP1 hours={hours} backend=HiGHS status={:?} obj={:?} bound={:?} wall={:.1}s issues={}",
            highs.summary.status,
            highs.summary.objective,
            highs.summary.best_bound,
            highs_wall.as_secs_f64(),
            issues.len()
        );

        let size = std::fs::metadata(&path).map(|entry| entry.len()).unwrap_or(0);
        if size == 0 {
            println!("EXP1 hours={hours} backend=SCIP  MPS EXPORT FAILED");
            continue;
        }
        let started = std::time::Instant::now();
        match super::experiments::parcel_via_mps(&path, SolveTuning::new(Some(budget))) {
            Ok(report) => println!(
                "EXP1 hours={hours} backend=SCIP  status={:?} obj={:?} bound={:.4} gap={:?} nodes={} wall={:.1}s mps={}KB",
                report.status,
                report.objective,
                report.bound,
                report.gap,
                report.nodes,
                started.elapsed().as_secs_f64(),
                size / 1024
            ),
            Err(error) => println!("EXP1 hours={hours} backend=SCIP  ERROR {error}"),
        }
    }
}

/// §10.8 / §7 - economics cannot reorder authored work. Block 1 is worthless
/// and block 2 is valuable, but the authored sequence puts 1 first, so the
/// loader must work through it before touching 2.
#[test]
fn authored_order_survives_economics() {
    let grades = grade_table(&["Fe"], &[(1, vec![60.0]), (2, vec![60.0])]);
    let input = BlendInput {
        intervals: intervals(2, 1.0),
        segments_per_interval: 2,
        grades,
        piles: vec![BlendPile::combine(StockpileId(0), 10_000.0, &[], 1)],
        loaders: vec![loader(0, 2, 100.0, 0.0)],
        // Authored sequence: the cheap block first.
        tasks: vec![dig_task(0, 0, &[1, 2], 2.0)],
        ground: vec![
            GroundSource {
                id: GroundId(1),
                tonnes_t: 150.0,
                material: vec![MaterialShare {
                    material: MaterialId(1),
                    fraction: 1.0,
                }],
            },
            GroundSource {
                id: GroundId(2),
                tonnes_t: 150.0,
                material: vec![MaterialShare {
                    material: MaterialId(2),
                    fraction: 1.0,
                }],
            },
        ],
        destinations: vec![pile_destination(0, 0, None)],
        trucks: vec![truck(0, 2, 100_000.0)],
        movements: vec![
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(1)), 1, 0, 1.0),
            // Ten times the value: pure economic pressure to jump the queue.
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(2)), 2, 0, 10.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: Vec::new(),
    };
    let finished = run(&input, 60);
    let solution = finished.solution.expect("an incumbent");
    let replay = finished.replay.expect("a replay");
    assert!(replay.is_valid(), "replay issues: {:?}", replay.issues);

    // Total capacity over the horizon is 200 t against 300 t of ground, so a
    // solver free to choose would take the valuable block only.
    let first: f64 = solution.movements.iter().filter(|row| row.candidate == 0).map(|row| row.tonnes_t).sum();
    let second: f64 = solution.movements.iter().filter(|row| row.candidate == 1).map(|row| row.tonnes_t).sum();
    assert!(first > 149.9, "authored first block only got {first} t while the valuable one took {second} t");
    assert!(second <= 50.0 + 1e-4, "the later block ran ahead of its authored position: {second} t");
}

/// §10 horizon scaling for the blended model. Not comparable with any parcel
/// objective: different stockpile semantics.
#[test]
#[ignore = "benchmark"]
fn blended_horizon_benchmark() {
    for (label, graded) in [("plain", false), ("graded", true)] {
        for hours in [4usize, 8, 12, 24, 48] {
            let count = hours;
            let grades = grade_table(&["Fe"], &[(1, vec![55.0]), (2, vec![70.0])]);
            let input = BlendInput {
                intervals: intervals(count, 1.0),
                segments_per_interval: 2,
                grades,
                piles: vec![BlendPile::combine(StockpileId(0), 1_000.0, &[(200.0, vec![0.60])], 1)],
                loaders: vec![
                    loader(1, count, 40.0, 0.0),
                    loader(2, count, 40.0, 0.0),
                    loader(3, count, 40.0, 0.0),
                    loader(4, count, 0.0, 40.0),
                ],
                tasks: vec![
                    dig_task(1, 1, &[1], hours as f64),
                    dig_task(2, 2, &[2], hours as f64),
                    dig_task(3, 3, &[3], hours as f64),
                    reclaim_task(4, 4, 0, hours as f64, None),
                ],
                ground: (1..=3)
                    .map(|id| GroundSource {
                        id: GroundId(id),
                        tonnes_t: 180.0 * hours as f64 / 4.0,
                        material: vec![MaterialShare {
                            material: MaterialId(1 + id % 2),
                            fraction: 1.0,
                        }],
                    })
                    .collect(),
                destinations: vec![
                    pile_destination(0, 0, Some(100_000.0)),
                    Destination {
                        id: DestinationId(1),
                        kind: DestinationKind::Crusher,
                        capacity_t: None,
                        crusher_daily_t: vec![Some(300.0); hours.div_ceil(24).max(1)],
                    },
                ],
                trucks: vec![truck(0, count, 10_000.0)],
                movements: vec![
                    candidate(1, Activity::Dig, SourceId::Ground(GroundId(1)), 2, 0, 1.0),
                    candidate(2, Activity::Dig, SourceId::Ground(GroundId(2)), 1, 0, 1.0),
                    candidate(3, Activity::Dig, SourceId::Ground(GroundId(3)), 2, 0, 1.0),
                    candidate(4, Activity::Reclaim, SourceId::Stockpile(StockpileId(0)), 1, 1, 2.0),
                ],
                qualifications: Vec::new(),
                conditional_values: Vec::new(),
                grade_limits: if graded {
                    vec![GradeLimit {
                        destination: DestinationId(1),
                        grade: FE,
                        minimum: 0.62,
                        inclusive: true,
                    }]
                } else {
                    Vec::new()
                },
            };
            let started = std::time::Instant::now();
            let finished = solve_blend(&input, SolveTuning::new(Some(Duration::from_secs(60))));
            let replay = finished.replay.as_ref();
            println!(
                "BLEND variant={label} hours={hours} status={:?} obj={:?} bound={:.3} gap={:?} nodes={} vars={} bin={} lin={} nonlin={} form={:?} wall={:.1}s issues={} resid={:.2e} negligible={}/{:.2e}t",
                finished.report.status,
                finished.report.objective,
                finished.report.bound,
                finished.report.gap,
                finished.report.nodes,
                finished.sizes.variables,
                finished.sizes.binaries,
                finished.sizes.linear_constraints,
                finished.sizes.nonlinear_constraints,
                finished.formulation_time,
                started.elapsed().as_secs_f64(),
                replay.map(|entry| entry.issues.len()).unwrap_or(0),
                replay.map(|entry| entry.max_absolute_residual_t).unwrap_or(0.0),
                replay.map(|entry| entry.negligible_grade_deliveries).unwrap_or(0),
                replay.map(|entry| entry.negligible_grade_tonnes_t).unwrap_or(0.0),
            );
            if let Some(entry) = replay {
                for issue in entry.issues.iter().take(3) {
                    println!("   ISSUE {issue}");
                }
            }
        }
    }
}

/// §10.9 / §8 - chunked blended piles. FIFO and LIFO must make *different*
/// valid choices, and each must be independently replay-clean.
///
/// Two chunks open with different grades. Reclaim is paid only for contained
/// Fe, so the two orders are forced to different material and therefore
/// different objectives.
fn chunk_world(order: ReclaimOrder, chunks: usize) -> BlendInput {
    chunk_world_hours(order, chunks, 4)
}

fn chunk_world_hours(order: ReclaimOrder, chunks: usize, hours: usize) -> BlendInput {
    let grades = grade_table(&["Fe"], &[(1, vec![40.0]), (2, vec![80.0])]);
    // The pile must be able to hold its own chunks.
    let mut pile = BlendPile::combine(StockpileId(0), 200.0 * chunks as f64, &[(200.0, vec![0.40])], 1);
    pile.chunks = vec![200.0; chunks];
    pile.order = order;
    // Alternating lean and rich chunks, all released from the start, so FIFO
    // and LIFO genuinely have to choose between different material.
    pile.chunk_opening = (0..chunks)
        .map(|index| {
            let fraction = if index % 2 == 0 { 0.40 } else { 0.80 };
            (200.0, vec![200.0 * fraction])
        })
        .collect();
    BlendInput {
        intervals: intervals(hours, 1.0),
        segments_per_interval: 1,
        grades,
        piles: vec![pile],
        loaders: vec![loader(0, hours, 200.0, 0.0), loader(1, hours, 0.0, 100.0)],
        tasks: vec![dig_task(0, 0, &[2], hours as f64), reclaim_task(1, 1, 0, hours as f64, None)],
        ground: vec![GroundSource {
            id: GroundId(2),
            tonnes_t: 200.0,
            material: vec![MaterialShare {
                material: MaterialId(2),
                fraction: 1.0,
            }],
        }],
        destinations: vec![pile_destination(0, 0, None), dump(1, None)],
        trucks: vec![truck(0, hours, 100_000.0)],
        movements: vec![
            // Digging the rich material into the pile earns nothing directly.
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(2)), 2, 0, 0.0),
            // Reclaim is paid per tonne; which chunk it may draw is what the
            // authored order decides.
            candidate(1, Activity::Reclaim, SourceId::Stockpile(StockpileId(0)), 1, 1, 1.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: Vec::new(),
    }
}

#[test]
fn chunked_fifo_and_lifo_differ_and_both_validate() {
    let mut contained = Vec::new();
    for order in [ReclaimOrder::Fifo, ReclaimOrder::Lifo] {
        let input = chunk_world(order, 2);
        let finished = run(&input, 60);
        let replay = finished.replay.expect("an incumbent");
        println!("CHUNK order={order:?} issues={:?}", replay.issues);
        assert!(replay.is_valid(), "{order:?} replay issues: {:?}", replay.issues);

        let solution = finished.solution.expect("an incumbent");
        // Which chunks actually supplied material, in order.
        let mut drawn: Vec<(usize, usize)> = solution.chunks.iter().filter(|row| row.reclaimed_t > 1e-4).map(|row| (row.interval, row.chunk)).collect();
        drawn.sort();
        println!("CHUNK order={order:?} draws={drawn:?}");
        contained.push(drawn);
    }
    assert_ne!(contained[0], contained[1], "FIFO and LIFO drew the same chunks in the same order");
}

/// §8 scaling: how much does reintroducing order cost?
#[test]
#[ignore = "benchmark"]
fn chunk_scaling_benchmark() {
    for hours in [4usize, 8, 12, 24, 48] {
        for chunks in [2usize, 5, 10] {
            for order in [ReclaimOrder::Fifo, ReclaimOrder::Lifo] {
                let input = chunk_world_hours(order, chunks, hours);
                let started = std::time::Instant::now();
                let finished = solve_blend(&input, SolveTuning::new(Some(Duration::from_secs(60))));
                let replay = finished.replay.as_ref();
                println!(
                    "CHUNKBENCH hours={hours} chunks={chunks} order={order:?} status={:?} obj={:?} bound={:.3} gap={:?} nodes={} vars={} bin={} lin={} nonlin={} wall={:.2}s issues={}",
                    finished.report.status,
                    finished.report.objective,
                    finished.report.bound,
                    finished.report.gap,
                    finished.report.nodes,
                    finished.sizes.variables,
                    finished.sizes.binaries,
                    finished.sizes.linear_constraints,
                    finished.sizes.nonlinear_constraints,
                    started.elapsed().as_secs_f64(),
                    replay.map(|entry| entry.issues.len()).unwrap_or(0),
                );
                if let Some(entry) = replay {
                    for issue in entry.issues.iter().take(3) {
                        println!("   ISSUE {issue}");
                    }
                }
            }
        }
    }
}

/// §7.1 - known answer. The pile opens below the crusher's minimum grade and
/// only enough high-grade receipt can lift it, so both methods have to decide
/// the blend, not just the tonnage.
#[test]
#[ignore = "benchmark"]
fn compare_known_answer() {
    compare("known-answer", &graded_blend_world(3), 60);
}

/// §7.2 - the threshold trap. Reclaim is paid only when the blend clears the
/// minimum, and the pile sits close enough to the boundary that a fixed-grade
/// estimate can flip the route on and off between iterations.
#[test]
#[ignore = "benchmark"]
fn compare_threshold_trap() {
    compare("threshold-trap", &threshold_trap_world(), 60);
}

/// §7.3 - dynamic chunks: an initially empty pile fills, closes and is
/// reclaimed under each authored order.
#[test]
#[ignore = "benchmark"]
fn compare_dynamic_chunks() {
    compare("dynamic-chunks-fifo", &dynamic_chunk_world(ReclaimOrder::Fifo, 12), 60);
    compare("dynamic-chunks-lifo", &dynamic_chunk_world(ReclaimOrder::Lifo, 12), 60);
}

/// §7.4 - resource competition: direct mining and two stockpiles compete for
/// one truck fleet and one crusher budget.
#[test]
#[ignore = "benchmark"]
fn compare_resource_competition() {
    compare("resource-competition", &competition_world(12), 60);
}

/// §7.5 / §7.6 - the representative constrained horizons.
#[test]
#[ignore = "benchmark"]
fn compare_horizons() {
    compare("24h", &competition_world(24), 60);
    compare("48h", &competition_world(48), 60);
}

// ---------------------------------------------------------------------------
// §5 / §7 shared scenario worlds.
//
// These are the fixtures both blended methods receive, unchanged, so a §7
// table row compares methods rather than inputs. Every one of them makes the
// solver decide *what enters the pile*, not just how much comes out - the
// previous chunk fixtures handed it opening stock already filled and closed,
// which exercised the reclaim order but never the filling decision.
// ---------------------------------------------------------------------------

/// A crusher with a daily budget.
fn crusher(id: u32, days: usize, daily_t: f64) -> Destination {
    Destination {
        id: DestinationId(id),
        kind: DestinationKind::Crusher,
        capacity_t: None,
        crusher_daily_t: vec![Some(daily_t); days.max(1)],
    }
}

/// §7.1 - the known-answer world. A pile opening at 50% Fe, a crusher that
/// will only take 62%, and 80% supply that has to be dug and blended in
/// before any reclaim qualifies.
pub(crate) fn graded_blend_world(hours: usize) -> BlendInput {
    BlendInput {
        intervals: intervals(hours, 1.0),
        segments_per_interval: 2,
        grades: grade_table(&["Fe"], &[(1, vec![50.0]), (2, vec![80.0])]),
        piles: vec![BlendPile::combine(StockpileId(0), 2_000.0, &[(400.0, vec![0.50])], 1)],
        loaders: vec![loader(0, hours, 300.0, 0.0), loader(1, hours, 0.0, 300.0)],
        tasks: vec![dig_task(0, 0, &[2], hours as f64), reclaim_task(1, 1, 0, hours as f64, None)],
        ground: vec![GroundSource {
            id: GroundId(2),
            tonnes_t: 2_000.0,
            material: vec![MaterialShare {
                material: MaterialId(2),
                fraction: 1.0,
            }],
        }],
        destinations: vec![pile_destination(0, 0, None), crusher(1, hours.div_ceil(24), 600.0)],
        trucks: vec![truck(0, hours, 5_000.0)],
        movements: vec![
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(2)), 2, 0, 0.0),
            candidate(1, Activity::Reclaim, SourceId::Stockpile(StockpileId(0)), 1, 1, 10.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: vec![GradeLimit {
            destination: DestinationId(1),
            grade: FE,
            minimum: 0.62,
            inclusive: true,
        }],
    }
}

/// §7.2 - the threshold trap.
///
/// The pile opens at exactly the crusher's minimum. Reclaiming lowers nothing
/// (a blend draws its own average), but the only supply is *below* the
/// minimum, so every tonne received pushes the blend under the boundary and
/// closes the paid route. A fixed-grade estimate has to discover where the
/// boundary actually bites; an estimate that lands just the wrong side of it
/// opens or closes the route for the whole solve.
fn threshold_trap_world() -> BlendInput {
    let hours = 8;
    BlendInput {
        intervals: intervals(hours, 1.0),
        segments_per_interval: 2,
        grades: grade_table(&["Fe"], &[(1, vec![62.0]), (2, vec![55.0])]),
        piles: vec![BlendPile::combine(StockpileId(0), 1_500.0, &[(600.0, vec![0.62])], 1)],
        loaders: vec![loader(0, hours, 200.0, 0.0), loader(1, hours, 0.0, 200.0)],
        tasks: vec![dig_task(0, 0, &[2], hours as f64), reclaim_task(1, 1, 0, hours as f64, None)],
        ground: vec![GroundSource {
            id: GroundId(2),
            tonnes_t: 1_600.0,
            material: vec![MaterialShare {
                material: MaterialId(2),
                fraction: 1.0,
            }],
        }],
        destinations: vec![pile_destination(0, 0, None), crusher(1, 1, 2_000.0), dump(2, None)],
        trucks: vec![truck(0, hours, 5_000.0)],
        movements: vec![
            // Digging the lean material pays a little on its own account, so
            // the solver is genuinely torn: fill the pile and lose the paid
            // reclaim route, or leave it alone.
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(2)), 2, 0, 1.0),
            candidate(1, Activity::Reclaim, SourceId::Stockpile(StockpileId(0)), 1, 1, 6.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: vec![GradeLimit {
            destination: DestinationId(1),
            grade: FE,
            minimum: 0.62,
            inclusive: true,
        }],
    }
}

/// §5 / §7.3 - dynamic chunks from an **empty** pile.
///
/// Nothing is pre-filled. Two ground sources of very different grade feed one
/// four-chunk pile; the solver chooses what goes into each chunk, when to
/// close it, and which closed chunk to draw under the authored order. Trucks
/// and the crusher are both tight enough to bite, and the crusher takes only
/// material at 62% Fe or better, so the composition of a chunk decides
/// whether it is worth anything.
fn dynamic_chunk_world(order: ReclaimOrder, hours: usize) -> BlendInput {
    let mut pile = BlendPile::combine(StockpileId(0), 1_200.0, &[], 1);
    pile.chunks = vec![300.0; 4];
    pile.order = order;
    BlendInput {
        intervals: intervals(hours, 1.0),
        segments_per_interval: 2,
        grades: grade_table(&["Fe"], &[(1, vec![45.0]), (2, vec![85.0])]),
        piles: vec![pile],
        loaders: vec![loader(0, hours, 150.0, 0.0), loader(1, hours, 150.0, 0.0), loader(2, hours, 0.0, 200.0)],
        tasks: vec![
            dig_task(0, 0, &[1], hours as f64),
            dig_task(1, 1, &[2], hours as f64),
            reclaim_task(2, 2, 0, hours as f64, None),
        ],
        ground: vec![
            GroundSource {
                id: GroundId(1),
                tonnes_t: 900.0,
                material: vec![MaterialShare {
                    material: MaterialId(1),
                    fraction: 1.0,
                }],
            },
            GroundSource {
                id: GroundId(2),
                tonnes_t: 900.0,
                material: vec![MaterialShare {
                    material: MaterialId(2),
                    fraction: 1.0,
                }],
            },
        ],
        destinations: vec![pile_destination(0, 0, None), crusher(1, hours.div_ceil(24), 700.0)],
        // Tight enough that the two dig loaders and the reclaim loader
        // genuinely compete for haulage.
        trucks: vec![truck(0, hours, 400.0)],
        movements: vec![
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(1)), 1, 0, 0.0),
            candidate(1, Activity::Dig, SourceId::Ground(GroundId(2)), 2, 0, 0.0),
            candidate(2, Activity::Reclaim, SourceId::Stockpile(StockpileId(0)), 1, 1, 8.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: vec![GradeLimit {
            destination: DestinationId(1),
            grade: FE,
            minimum: 0.62,
            inclusive: true,
        }],
    }
}

/// §7.4 - resource competition. Direct mining to the crusher competes with
/// reclaim from **two** blended piles for one truck fleet and one daily
/// crusher budget, and the crusher enforces a minimum grade on reclaimed
/// material.
pub(crate) fn competition_world(hours: usize) -> BlendInput {
    let days = hours.div_ceil(24);
    BlendInput {
        intervals: intervals(hours, 1.0),
        segments_per_interval: 2,
        grades: grade_table(&["Fe"], &[(1, vec![48.0]), (2, vec![82.0]), (3, vec![65.0])]),
        piles: vec![
            BlendPile::combine(StockpileId(0), 1_500.0, &[(300.0, vec![0.48])], 1),
            BlendPile::combine(StockpileId(1), 1_500.0, &[(300.0, vec![0.65])], 1),
        ],
        loaders: vec![
            loader(0, hours, 120.0, 0.0),
            loader(1, hours, 120.0, 0.0),
            loader(2, hours, 0.0, 150.0),
            loader(3, hours, 0.0, 150.0),
        ],
        tasks: vec![
            dig_task(0, 0, &[1], hours as f64),
            dig_task(1, 1, &[2], hours as f64),
            reclaim_task(2, 2, 0, hours as f64, Some(600.0)),
            reclaim_task(3, 3, 1, hours as f64, Some(600.0)),
        ],
        ground: vec![
            GroundSource {
                id: GroundId(1),
                tonnes_t: 60.0 * hours as f64,
                material: vec![MaterialShare {
                    material: MaterialId(1),
                    fraction: 1.0,
                }],
            },
            GroundSource {
                id: GroundId(2),
                tonnes_t: 60.0 * hours as f64,
                material: vec![MaterialShare {
                    material: MaterialId(2),
                    fraction: 1.0,
                }],
            },
        ],
        destinations: vec![pile_destination(0, 0, None), pile_destination(1, 1, None), crusher(2, days, 900.0)],
        trucks: vec![truck(0, hours, 260.0)],
        movements: vec![
            // Lean material may be stockpiled anywhere; rich material is
            // worth more direct to the crusher, which is the competition.
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(1)), 1, 0, 0.0),
            candidate(0, Activity::Dig, SourceId::Ground(GroundId(1)), 1, 1, 0.0),
            candidate(1, Activity::Dig, SourceId::Ground(GroundId(2)), 2, 1, 0.0),
            candidate(1, Activity::Dig, SourceId::Ground(GroundId(2)), 2, 2, 5.0),
            candidate(2, Activity::Reclaim, SourceId::Stockpile(StockpileId(0)), 1, 2, 9.0),
            candidate(3, Activity::Reclaim, SourceId::Stockpile(StockpileId(1)), 3, 2, 9.0),
        ],
        qualifications: Vec::new(),
        conditional_values: Vec::new(),
        grade_limits: vec![GradeLimit {
            destination: DestinationId(2),
            grade: FE,
            minimum: 0.62,
            inclusive: true,
        }],
    }
}

/// §5 - the dynamic chunk lifecycle, checked rather than benchmarked.
///
/// An initially empty pile must be filled, closed and reclaimed, and the
/// replay has to agree independently that every chunk's composition follows
/// from its own receipts.
#[test]
fn chunks_fill_close_and_reclaim_from_empty() {
    for order in [ReclaimOrder::Fifo, ReclaimOrder::Lifo] {
        let input = dynamic_chunk_world(order, 8);
        let finished = run(&input, 60);
        let solution = finished.solution.expect("an incumbent");
        let replay = finished.replay.expect("a replay");
        assert!(replay.is_valid(), "{order:?} replay issues: {:?} / {:?}", replay.issues, replay.grade_issues);

        // Something must actually have entered a chunk: the pile opened
        // empty, so a schedule that reclaims anything had to fill first.
        let received: f64 = solution.chunks.iter().map(|row| row.received_t).sum();
        let reclaimed: f64 = solution.chunks.iter().map(|row| row.reclaimed_t).sum();
        assert!(received > 1.0, "{order:?} never filled a chunk");
        assert!(reclaimed > 1.0, "{order:?} never reclaimed from a filled chunk");

        // And the blend the replay recomputed for a drawn chunk must clear
        // the crusher's minimum, since that is the only paid route.
        let drawn: Vec<_> = solution.chunks.iter().filter(|row| row.reclaimed_t > 1e-6).collect();
        assert!(!drawn.is_empty());
        for row in drawn {
            let key = crate::model::schedule::optimisation::blended::formulation::chunk_key(row.chunk, row.interval, input.intervals.len());
            let blend = replay.blends.get(&(row.pile, key, FE)).copied().unwrap_or(0.0);
            assert!(blend >= 0.62 - 1e-9, "chunk {} drew at {blend} in interval {}", row.chunk, row.interval);
        }
    }
}
