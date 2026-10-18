//! Compare solver's MIP presolve with HiGHS's on MPS programs with integer
//! columns: the presolved models must match bit for bit.
//!
//! `cargo run --release -p solver --example mip_presolve -- <file.mps | directory>...`

use std::path::PathBuf;

use highs_sys::*;
use solver::{DebugPresolved, Problem, Sense};

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
    let mut mismatches = 0;
    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let problem = match std::fs::read_to_string(file).map_err(|error| error.to_string()).and_then(|text| solver::mps::read(&text)) {
            Ok(problem) => problem,
            Err(error) => {
                println!("{name:<24} skipped: {error}");
                continue;
            }
        };
        if !problem.integrality.iter().any(|&integer| integer) {
            continue;
        }
        let started = std::time::Instant::now();
        let reference = highs_presolve(&problem);
        let h_ms = started.elapsed().as_secs_f64() * 1e3;
        let started = std::time::Instant::now();
        let ours = std::panic::catch_unwind(|| solver::debug_mip_presolve(&problem));
        let r_ms = started.elapsed().as_secs_f64() * 1e3;
        if std::env::var_os("TIMES").is_some() {
            println!("{name:<24} HiGHS {h_ms:.0} ms, solver {r_ms:.0} ms");
        }
        let ours = match ours {
            Ok(ours) => ours,
            Err(_) => {
                println!("{name:<24} solver panicked");
                mismatches += 1;
                continue;
            }
        };
        if reference.status == "Infeasible" || ours.status == "Infeasible" {
            if reference.status == ours.status {
                println!("{name:<24} same: infeasible");
            } else {
                mismatches += 1;
                println!("{name:<24} DIFFERENT: HiGHS {:?}, solver {}", reference.status, ours.status);
            }
            continue;
        }
        match compare(&reference, &ours) {
            None => println!(
                "{name:<24} same: {} cols, {} rows, {} nonzeros",
                ours.col_cost.len(),
                ours.row_lower.len(),
                ours.value.len()
            ),
            Some(difference) => {
                mismatches += 1;
                println!(
                    "{name:<24} DIFFERENT: {difference} (HiGHS {} cols {} rows {} nz, solver {} cols {} rows {} nz, {})",
                    reference.col_cost.len(),
                    reference.row_lower.len(),
                    reference.value.len(),
                    ours.col_cost.len(),
                    ours.row_lower.len(),
                    ours.value.len(),
                    ours.status
                );
            }
        }
    }
    println!("{mismatches} programs differ");
}

fn compare(a: &DebugPresolved, b: &DebugPresolved) -> Option<String> {
    fn floats(name: &str, a: &[f64], b: &[f64]) -> Option<String> {
        if a.len() != b.len() {
            return Some(format!("{name} length {} vs {}", a.len(), b.len()));
        }
        (0..a.len())
            .find(|&i| a[i].to_bits() != b[i].to_bits())
            .map(|i| format!("{name}[{i}] {:e} vs {:e}", a[i], b[i]))
    }
    fn ints<T: PartialEq + std::fmt::Debug>(name: &str, a: &[T], b: &[T]) -> Option<String> {
        if a.len() != b.len() {
            return Some(format!("{name} length {} vs {}", a.len(), b.len()));
        }
        (0..a.len()).find(|&i| a[i] != b[i]).map(|i| format!("{name}[{i}] {:?} vs {:?}", a[i], b[i]))
    }
    floats("col_cost", &a.col_cost, &b.col_cost)
        .or_else(|| floats("col_lower", &a.col_lower, &b.col_lower))
        .or_else(|| floats("col_upper", &a.col_upper, &b.col_upper))
        .or_else(|| ints("integrality", &a.integrality, &b.integrality))
        .or_else(|| floats("row_lower", &a.row_lower, &b.row_lower))
        .or_else(|| floats("row_upper", &a.row_upper, &b.row_upper))
        .or_else(|| ints("start", &a.start, &b.start))
        .or_else(|| ints("index", &a.index, &b.index))
        .or_else(|| floats("value", &a.value, &b.value))
        .or_else(|| (a.offset.to_bits() != b.offset.to_bits()).then(|| format!("offset {:e} vs {:e}", a.offset, b.offset)))
}

fn highs_presolve(problem: &Problem) -> DebugPresolved {
    let (start, index, value) = problem.col_wise();
    let start: Vec<HighsInt> = start.iter().map(|&at| at as HighsInt).collect();
    let index: Vec<HighsInt> = index.iter().map(|&at| at as HighsInt).collect();
    let integrality: Vec<HighsInt> = problem.integrality.iter().map(|&integer| integer as HighsInt).collect();
    let sense = if problem.sense == Sense::Maximise {
        OBJECTIVE_SENSE_MAXIMIZE
    } else {
        OBJECTIVE_SENSE_MINIMIZE
    };
    // SAFETY: every array outlives the calls and has the length passed or
    // reported.
    unsafe {
        let highs = Highs_create();
        Highs_setBoolOptionValue(highs, c"output_flag".as_ptr(), 0);
        Highs_setIntOptionValue(highs, c"threads".as_ptr(), 1);
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
        Highs_presolve(highs);
        let infeasible = Highs_getModelStatus(highs) == MODEL_STATUS_INFEASIBLE;
        let num_col = Highs_getPresolvedNumCol(highs) as usize;
        let num_row = Highs_getPresolvedNumRow(highs) as usize;
        let num_nz = Highs_getPresolvedNumNz(highs) as usize;
        let mut out = DebugPresolved {
            col_cost: vec![0.0; num_col],
            col_lower: vec![0.0; num_col],
            col_upper: vec![0.0; num_col],
            row_lower: vec![0.0; num_row],
            row_upper: vec![0.0; num_row],
            value: vec![0.0; num_nz],
            ..DebugPresolved::default()
        };
        let mut a_start: Vec<HighsInt> = vec![0; num_col + 1];
        let mut a_index: Vec<HighsInt> = vec![0; num_nz];
        let mut integrality: Vec<HighsInt> = vec![0; num_col];
        let (mut nc, mut nr, mut nnz, mut sense) = (0, 0, 0, 0);
        Highs_getPresolvedLp(
            highs,
            MATRIX_FORMAT_COLUMN_WISE,
            &mut nc,
            &mut nr,
            &mut nnz,
            &mut sense,
            &mut out.offset,
            out.col_cost.as_mut_ptr(),
            out.col_lower.as_mut_ptr(),
            out.col_upper.as_mut_ptr(),
            out.row_lower.as_mut_ptr(),
            out.row_upper.as_mut_ptr(),
            a_start.as_mut_ptr(),
            a_index.as_mut_ptr(),
            out.value.as_mut_ptr(),
            integrality.as_mut_ptr(),
        );
        Highs_destroy(highs);
        // The C API leaves the last start alone.
        a_start[num_col] = num_nz as HighsInt;
        out.start = a_start.iter().map(|&at| at as usize).collect();
        out.index = a_index.iter().map(|&at| at as usize).collect();
        out.integrality = integrality.iter().map(|&t| t as u8).collect();
        if infeasible {
            out.status = "Infeasible".into();
        }
        out
    }
}
