//! Solve one MPS program with solver's MIP solver again and again (with the
//! `.seed` beside it if there is one), for sampling profilers.
//!
//! `cargo run --release -p solver --example mip_loop -- <file.mps> [times]`;
//! with `HIGHS` set, HiGHS solves it instead (one thread).

unsafe extern "C" {
    fn prctl(option: i32, arg: usize, ...) -> i32;
}

fn main() {
    // Let a profiler that is not our parent attach (Yama's ptrace scope).
    // SAFETY: PR_SET_PTRACER with PR_SET_PTRACER_ANY changes only who may
    // trace this process.
    unsafe { prctl(0x59616d61, usize::MAX) };
    let mut args = std::env::args().skip(1);
    let file = std::path::PathBuf::from(args.next().expect("an MPS file"));
    let times: usize = args.next().and_then(|times| times.parse().ok()).unwrap_or(10);
    let problem = solver::mps::read(&std::fs::read_to_string(&file).expect("read the program")).expect("parse the program");
    let seed = std::fs::read_to_string(file.with_extension("seed"))
        .ok()
        .map(|text| text.lines().map(|line| line.parse::<f64>().unwrap()).collect::<Vec<f64>>());
    let settings = solver::MipSettings::default();
    if std::env::var_os("HIGHS").is_some() {
        for _ in 0..times {
            let started = std::time::Instant::now();
            highs_solve(&problem, seed.as_deref());
            println!("HiGHS {:.0} ms", started.elapsed().as_secs_f64() * 1e3);
        }
        return;
    }
    for _ in 0..times {
        let started = std::time::Instant::now();
        let solution = solver::solve_mip(&problem, &settings, seed.as_deref(), None);
        println!(
            "{:?} {:e} {} nodes {:.0} ms",
            solution.status,
            solution.objective,
            solution.nodes,
            started.elapsed().as_secs_f64() * 1e3
        );
    }
}

fn highs_solve(problem: &solver::Problem, seed: Option<&[f64]>) {
    use highs_sys::*;
    let (start, index, value) = problem.col_wise();
    let start: Vec<HighsInt> = start.iter().map(|&at| at as HighsInt).collect();
    let index: Vec<HighsInt> = index.iter().map(|&at| at as HighsInt).collect();
    let integrality: Vec<HighsInt> = problem.integrality.iter().map(|&integer| integer as HighsInt).collect();
    let sense = if problem.sense == solver::Sense::Maximise {
        OBJECTIVE_SENSE_MAXIMIZE
    } else {
        OBJECTIVE_SENSE_MINIMIZE
    };
    // SAFETY: every array outlives the calls and has the length passed.
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
        if let Some(seed) = seed {
            Highs_setSolution(highs, seed.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null());
        }
        Highs_run(highs);
        Highs_destroy(highs);
    }
}
