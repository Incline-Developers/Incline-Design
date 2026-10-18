//! Solve an LP dumped by the MIP solver's `SOLVER_DUMP_LP` with solver, and
//! print its iterations and objective bits; with `MPS=<file>`, write it
//! there as MPS instead.

fn main() {
    let path = std::env::args().nth(1).expect("a dump");
    let bytes = std::fs::read(path).expect("read");
    let mut at = 0;
    let mut word = || {
        let v = u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
        at += 8;
        v
    };
    let nc = word() as usize;
    let nr = word() as usize;
    let mut take = |n: usize| (0..n).map(|_| f64::from_bits(word())).collect::<Vec<f64>>();
    let col_cost = take(nc);
    let col_lower = take(nc);
    let col_upper = take(nc);
    let row_lower = take(nr);
    let row_upper = take(nr);
    let mut word = || {
        let v = u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
        at += 8;
        v
    };
    let nnz = word() as usize;
    let start: Vec<usize> = (0..=nc).map(|_| word() as usize).collect();
    let index: Vec<usize> = (0..nnz).map(|_| word() as usize).collect();
    let value: Vec<f64> = (0..nnz).map(|_| f64::from_bits(word())).collect();
    // Row-wise, each row's entries in column order.
    let mut row_start = vec![0usize; nr + 1];
    for &row in &index {
        row_start[row + 1] += 1;
    }
    for row in 0..nr {
        row_start[row + 1] += row_start[row];
    }
    let mut next = row_start.clone();
    let mut row_index = vec![0; nnz];
    let mut row_value = vec![0.0; nnz];
    for col in 0..nc {
        for k in start[col]..start[col + 1] {
            let row = index[k];
            row_index[next[row]] = col;
            row_value[next[row]] = value[k];
            next[row] += 1;
        }
    }
    let problem = solver::Problem {
        sense: solver::Sense::Minimise,
        offset: 0.0,
        col_cost,
        col_lower,
        col_upper,
        integrality: Vec::new(),
        row_lower,
        row_upper,
        row_start,
        row_index,
        row_value,
    };
    if let Some(path) = std::env::var_os("MPS") {
        std::fs::write(path, solver::mps::write(&problem)).expect("write the MPS file");
        return;
    }
    let options = solver::Options {
        primal_feasibility_tolerance: 1e-6,
        dual_feasibility_tolerance: 1e-7,
        presolve: std::env::args().nth(2).is_none(),
        ..solver::Options::default()
    };
    let mut solver = solver::LpSolver::new(&problem, options);
    solver.run();
    if std::env::var_os("COLS").is_some() {
        for v in solver.col_value().unwrap() {
            println!("{:x}", v.to_bits());
        }
        let basis = solver.basis().unwrap();
        let code = |s: &solver::BasisStatus| match s {
            solver::BasisStatus::Lower => 0,
            solver::BasisStatus::Basic => 1,
            solver::BasisStatus::Upper => 2,
            solver::BasisStatus::Zero => 3,
            solver::BasisStatus::Nonbasic => 4,
        };
        println!("{}", basis.col_status.iter().map(|s| format!("{} ", code(s))).collect::<String>());
        println!("{}", basis.row_status.iter().map(|s| format!("{} ", code(s))).collect::<String>());
    }
    println!("solver iters {} obj {:x} {:.17}", solver.iterations(), solver.objective().to_bits(), solver.objective());
}
