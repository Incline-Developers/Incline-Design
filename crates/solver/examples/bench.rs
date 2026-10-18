//! Solve MPS programs with HiGHS and report time, iterations and objective.
//!
//! `cargo run --release -p solver --example bench -- <file.mps | directory>...`
//!
//! `--no-presolve` turns presolve off in both; `--ipm` runs the interior
//! point solver (then crossover and simplex), and reports its iterations
//! as `ipm/crossover/simplex`. `--solver-only` skips HiGHS and
//! `--highs-only` solver, for profiling.
//!
//! Each program is solved repeatedly until a tenth of a second (or
//! `REPEAT_S` seconds) has gone, and the fastest solve is reported, so
//! small programs time steadily.

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use solver::{Problem, Sense};

unsafe extern "C" {
    fn prctl(option: i32, arg: usize, ...) -> i32;
}

fn main() {
    // Let a profiler that is not our parent attach (Yama's ptrace scope).
    // SAFETY: PR_SET_PTRACER with PR_SET_PTRACER_ANY changes only who may
    // trace this process.
    unsafe { prctl(0x59616d61, usize::MAX) };
    let mut files: Vec<PathBuf> = Vec::new();
    let mut presolve = true;
    let mut ipm = false;
    let mut solver_only = false;
    let mut highs_only = false;
    for arg in std::env::args().skip(1) {
        if arg == "--no-presolve" {
            presolve = false;
            continue;
        }
        if arg == "--highs-only" {
            highs_only = true;
            continue;
        }
        if arg == "--solver-only" {
            solver_only = true;
            continue;
        }
        if arg == "--ipm" {
            ipm = true;
            continue;
        }
        let path = PathBuf::from(arg);
        if path.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(&path)
                .expect("read directory")
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.extension().is_some_and(|ext| ext == "mps"))
                .collect();
            found.sort();
            files.extend(found);
        } else {
            files.push(path);
        }
    }
    println!(
        "{:<26} {:>6} {:>6} {:>7}  {:>9} {:>12}  {:>9} {:>12} {:>6}  {:>20}  result",
        "program", "rows", "cols", "nonzeros", "HiGHS ms", "iters", "solver ms", "iters", "ratio", "objective"
    );
    let mut total_highs = Duration::ZERO;
    let mut total_solver = Duration::ZERO;
    let mut disagreements = 0;
    let mut iteration_mismatches = 0;
    let options = solver::Options {
        presolve,
        solver: if ipm { solver::Solver::Ipm } else { solver::Solver::Simplex },
        ..solver::Options::default()
    };
    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let problem = match read(file) {
            Ok(problem) => problem,
            Err(error) => {
                println!("{name:<26} skipped: {error}");
                continue;
            }
        };
        if !problem.integrality.is_empty() {
            println!("{name:<26} skipped: integer columns");
            continue;
        }
        if highs_only {
            let (highs_time, reference) = fastest(|| highs::solve(&problem, presolve, ipm));
            println!(
                "{name:<26} HiGHS {:.3} ms, {} iterations, {}",
                highs_time.as_secs_f64() * 1e3,
                reference.iterations,
                reference.status
            );
            continue;
        }
        let (solver_time, solution) = fastest(|| solver::solve(&problem, &options));
        if solver_only {
            println!(
                "{name:<26} solver {:.3} ms, {}/{}/{} iterations, {:?} {:.12e}",
                solver_time.as_secs_f64() * 1e3,
                solution.ipm_iterations,
                solution.crossover_iterations,
                solution.iterations,
                solution.status,
                solution.objective
            );
            continue;
        }
        let (highs_time, reference) = fastest(|| highs::solve(&problem, presolve, ipm));
        total_highs += highs_time;
        total_solver += solver_time;
        let solver_status = status_name(solution.status);
        let agree =
            solver_status == reference.status && (reference.status != "optimal" || (solution.objective - reference.objective).abs() <= 1e-6 * (1.0 + reference.objective.abs()));
        let violation = if solution.status == solver::Status::Optimal {
            max_violation(&problem, &solution)
        } else {
            0.0
        };
        let verdict = if !agree {
            disagreements += 1;
            format!("DIFFERS: solver {solver_status} {:.12e}", solution.objective)
        } else if violation > 1e-6 {
            disagreements += 1;
            format!("INFEASIBLE by {violation:e}")
        } else {
            reference.status.clone()
        };
        let iterations = if ipm {
            format!("{}/{}/{}", solution.ipm_iterations, solution.crossover_iterations, solution.iterations)
        } else {
            solution.iterations.to_string()
        };
        let iterations_differ = iterations != reference.iterations;
        if iterations_differ {
            iteration_mismatches += 1;
        }
        let marker = if iterations_differ { "*" } else { " " };
        println!(
            "{name:<26} {:>6} {:>6} {:>7}  {:>9.3} {:>12}  {:>9.3} {:>12}{marker}{:>6.2}  {:>20.12e}  {verdict}",
            problem.num_rows(),
            problem.num_cols(),
            problem.num_nonzeros(),
            highs_time.as_secs_f64() * 1e3,
            reference.iterations,
            solver_time.as_secs_f64() * 1e3,
            iterations,
            solver_time.as_secs_f64() / highs_time.as_secs_f64(),
            reference.objective,
        );
    }
    println!(
        "total HiGHS {:.3} ms, solver {:.3} ms ({:.2}x) over {} programs; {disagreements} disagreements, {iteration_mismatches} iteration counts differ",
        total_highs.as_secs_f64() * 1e3,
        total_solver.as_secs_f64() * 1e3,
        total_solver.as_secs_f64() / total_highs.as_secs_f64(),
        files.len()
    );
}

fn status_name(status: solver::Status) -> String {
    match status {
        solver::Status::Optimal => "optimal".into(),
        solver::Status::Infeasible => "infeasible".into(),
        solver::Status::Unbounded => "unbounded".into(),
        solver::Status::UnboundedOrInfeasible => "unbounded or infeasible".into(),
        other => format!("{other:?}"),
    }
}

/// The largest bound violation of `solution`'s columns and row activities.
fn max_violation(problem: &Problem, solution: &solver::Solution) -> f64 {
    let mut worst: f64 = 0.0;
    let mut check = |value: f64, lower: f64, upper: f64| {
        let scale = 1.0f64.max(lower.abs().min(upper.abs()).min(1e6));
        let violation = (lower - value).max(value - upper).max(0.0) / scale;
        worst = worst.max(violation);
    };
    for col in 0..problem.num_cols() {
        check(solution.col_value[col], problem.col_lower[col], problem.col_upper[col]);
    }
    for row in 0..problem.num_rows() {
        // Both solvers drop entries this small, so the check does too.
        let activity: f64 = problem
            .row(row)
            .filter(|(_, value)| value.abs() > 1e-9)
            .map(|(col, value)| value * solution.col_value[col])
            .sum();
        check(activity, problem.row_lower[row], problem.row_upper[row]);
    }
    worst
}

fn read(path: &Path) -> Result<Problem, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    solver::mps::read(&text)
}

fn fastest<T>(mut solve: impl FnMut() -> T) -> (Duration, T) {
    let started = Instant::now();
    let mut best = Duration::MAX;
    let mut last;
    loop {
        let begun = Instant::now();
        last = solve();
        best = best.min(begun.elapsed());
        let repeat = std::env::var("REPEAT_S").ok().and_then(|seconds| seconds.parse().ok()).unwrap_or(0.1);
        if started.elapsed() > Duration::from_secs_f64(repeat) {
            return (best, last);
        }
    }
}

struct Outcome {
    status: String,
    objective: f64,
    iterations: String,
}

mod highs {
    use highs_sys::*;

    use super::{Outcome, Problem, Sense};

    pub fn solve(problem: &Problem, presolve: bool, ipm: bool) -> Outcome {
        let (start, index, value) = problem.col_wise();
        let start: Vec<HighsInt> = start.iter().map(|&at| at as HighsInt).collect();
        let index: Vec<HighsInt> = index.iter().map(|&at| at as HighsInt).collect();
        let sense = if problem.sense == Sense::Maximise {
            OBJECTIVE_SENSE_MAXIMIZE
        } else {
            OBJECTIVE_SENSE_MINIMIZE
        };
        // SAFETY: every array outlives the calls and has the length passed.
        unsafe {
            let highs = Highs_create();
            Highs_setBoolOptionValue(highs, c"output_flag".as_ptr(), 0);
            Highs_setIntOptionValue(highs, c"threads".as_ptr(), 1);
            if !presolve {
                Highs_setStringOptionValue(highs, c"presolve".as_ptr(), c"off".as_ptr());
            }
            if ipm {
                Highs_setStringOptionValue(highs, c"solver".as_ptr(), c"ipm".as_ptr());
            }
            Highs_passLp(
                highs,
                problem.num_cols() as HighsInt,
                problem.num_rows() as HighsInt,
                value.len() as HighsInt,
                MATRIX_FORMAT_COLUMN_WISE,
                sense,
                problem.offset,
                problem.col_cost.as_ptr(),
                problem.col_lower.as_ptr(),
                problem.col_upper.as_ptr(),
                problem.row_lower.as_ptr(),
                problem.row_upper.as_ptr(),
                start.as_ptr(),
                index.as_ptr(),
                value.as_ptr(),
            );
            Highs_run(highs);
            finish(highs, ipm)
        }
    }

    unsafe fn finish(highs: *mut std::ffi::c_void, ipm: bool) -> Outcome {
        unsafe {
            let status = match Highs_getModelStatus(highs) {
                MODEL_STATUS_OPTIMAL => "optimal".to_string(),
                MODEL_STATUS_INFEASIBLE => "infeasible".to_string(),
                MODEL_STATUS_UNBOUNDED => "unbounded".to_string(),
                MODEL_STATUS_UNBOUNDED_OR_INFEASIBLE => "unbounded or infeasible".to_string(),
                MODEL_STATUS_UNKNOWN => "Unknown".to_string(),
                MODEL_STATUS_REACHED_TIME_LIMIT => "TimeLimit".to_string(),
                MODEL_STATUS_REACHED_ITERATION_LIMIT => "IterationLimit".to_string(),
                MODEL_STATUS_SOLVE_ERROR => "SolveError".to_string(),
                other => format!("status {other}"),
            };
            let objective = Highs_getObjectiveValue(highs);
            let count = |name: &std::ffi::CStr| {
                let mut count: HighsInt = 0;
                Highs_getIntInfoValue(highs, name.as_ptr(), &mut count);
                count
            };
            let simplex = count(c"simplex_iteration_count");
            let iterations = if ipm {
                format!("{}/{}/{simplex}", count(c"ipm_iteration_count"), count(c"crossover_iteration_count"))
            } else {
                simplex.to_string()
            };
            Highs_destroy(highs);
            Outcome { status, objective, iterations }
        }
    }
}
