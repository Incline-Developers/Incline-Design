//! Compare solver's MIP solver with HiGHS's (one thread, symmetry detection
//! off in both) on MPS programs with integer columns: status, objective,
//! nodes and LP iterations. Both go through their public entry points
//! (`Highs_run`, `solver::solve_mip`). `SYMMETRY` turns symmetry detection
//! on; `SEED` hands both the same start, made from HiGHS's own answer:
//! `opt` as it is, `cont` with its continuous columns zeroed (completed by
//! an LP), `frac` with every other integer column at a half (completed by
//! a MIP), `zero` all zeros, `file` the values in the program's `.seed` file
//! beside it. `TIME` sets both time limits. `GAP` sets `mip_rel_gap`.
//!
//! `cargo run --release -p solver --example mip_solve -- <file.mps | directory>...`

use std::{path::PathBuf, time::Instant};

use highs_sys::*;
use solver::{Problem, Sense};

fn main() {
    let mut files: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
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
        "{:<22} {:>14} {:>14} {:>9} {:>9} {:>9} {:>9} {:>8} {:>8}",
        "program", "HiGHS obj", "solver obj", "H nodes", "r nodes", "H iters", "r iters", "H ms", "r ms"
    );
    let mut differ = 0;
    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let Ok(problem) = std::fs::read_to_string(file).map_err(|error| error.to_string()).and_then(|text| solver::mps::read(&text)) else {
            continue;
        };
        if !problem.integrality.iter().any(|&integer| integer) {
            continue;
        }
        let seed_file = file.with_extension("seed");
        let seed = std::env::var("SEED").ok().and_then(|kind| {
            if kind == "file" {
                let text = std::fs::read_to_string(&seed_file).ok()?;
                return Some(text.lines().map(|line| line.parse::<f64>().unwrap()).collect::<Vec<f64>>());
            }
            let (_, _, _, _, solution) = highs_solve(&problem, None);
            let mut seed = solution?;
            let mut integer_count = 0;
            for (col, value) in seed.iter_mut().enumerate() {
                match kind.as_str() {
                    "cont" if !problem.integrality[col] => *value = 0.0,
                    "frac" if problem.integrality[col] => {
                        integer_count += 1;
                        if integer_count % 2 == 0 {
                            *value = 0.5;
                        }
                    }
                    "zero" => *value = 0.0,
                    _ => {}
                }
            }
            Some(seed)
        });
        let started = Instant::now();
        let (h_status, h_obj, h_nodes, h_iters, _) = highs_solve(&problem, seed.as_deref());
        let h_ms = started.elapsed().as_secs_f64() * 1e3;
        let started = Instant::now();
        let settings = solver::MipSettings {
            mip_detect_symmetry: std::env::var_os("SYMMETRY").is_some(),
            mip_rel_gap: gap(),
            time_limit: time_limit(),
        };
        let ours = std::panic::catch_unwind(|| solver::solve_mip(&problem, &settings, seed.as_deref(), None));
        let r_ms = started.elapsed().as_secs_f64() * 1e3;
        let Ok(ours) = ours else {
            println!("{name:<22} solver panicked");
            differ += 1;
            continue;
        };
        let objective = if ours.status == solver::MipStatus::Optimal { ours.objective } else { f64::INFINITY };
        let same = ours.nodes == h_nodes && ours.lp_iterations == h_iters && (objective == h_obj || (objective.is_infinite() && h_obj.is_infinite()));
        if !same {
            differ += 1;
        }
        println!(
            "{name:<22} {h_obj:>14.6e} {:>14.6e} {h_nodes:>9} {:>9} {h_iters:>9} {:>9} {h_ms:>8.0} {r_ms:>8.0} {} {h_status} {:?}",
            objective,
            ours.nodes,
            ours.lp_iterations,
            if same { "" } else { "DIFFERENT" },
            ours.status
        );
    }
    println!("{differ} programs differ");
}

fn time_limit() -> f64 {
    std::env::var("TIME").ok().and_then(|time| time.parse().ok()).unwrap_or(f64::INFINITY)
}

fn gap() -> f64 {
    std::env::var("GAP").ok().and_then(|gap| gap.parse().ok()).unwrap_or(1e-4)
}

fn highs_solve(problem: &Problem, seed: Option<&[f64]>) -> (String, f64, i64, i64, Option<Vec<f64>>) {
    let (start, index, value) = problem.col_wise();
    let start: Vec<HighsInt> = start.iter().map(|&at| at as HighsInt).collect();
    let index: Vec<HighsInt> = index.iter().map(|&at| at as HighsInt).collect();
    let integrality: Vec<HighsInt> = problem.integrality.iter().map(|&integer| integer as HighsInt).collect();
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
        let symmetry = std::env::var_os("SYMMETRY").is_some();
        Highs_setBoolOptionValue(highs, c"mip_detect_symmetry".as_ptr(), symmetry as HighsInt);
        Highs_passMip(
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
            integrality.as_ptr(),
        );
        Highs_setDoubleOptionValue(highs, c"mip_rel_gap".as_ptr(), gap());
        Highs_setDoubleOptionValue(highs, c"time_limit".as_ptr(), time_limit());
        if let Some(seed) = seed {
            Highs_setSolution(highs, seed.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null());
        }
        Highs_run(highs);
        let status = Highs_getModelStatus(highs);
        let objective = if status == MODEL_STATUS_OPTIMAL {
            Highs_getObjectiveValue(highs)
        } else {
            f64::INFINITY
        };
        let mut nodes: i64 = 0;
        Highs_getInt64InfoValue(highs, c"mip_node_count".as_ptr(), &mut nodes);
        let mut iters: HighsInt = 0;
        Highs_getIntInfoValue(highs, c"simplex_iteration_count".as_ptr(), &mut iters);
        let iters = iters as i64;
        let mut solution = None;
        if Highs_getObjectiveValue(highs).is_finite() && status != MODEL_STATUS_INFEASIBLE {
            let mut values = vec![0.0; problem.num_cols()];
            Highs_getSolution(highs, values.as_mut_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            solution = Some(values);
        }
        Highs_destroy(highs);
        (format!("{status}"), objective, nodes, iters, solution)
    }
}
