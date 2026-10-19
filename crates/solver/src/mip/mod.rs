//! HiGHS's mixed-integer solver (`highs/mip`), ported to Rust.
//!
//! HiGHS's MIP solver is a web of objects that point at each other through
//! the solver: domains register with cut and conflict pools, which notify
//! them of every change; propagation reaches back into the clique table and
//! implications; everything reads the solver's shared data. Here the shared
//! pieces are plain fields, passed to each operation as it needs them, and
//! pools keep a log of their changes that each domain replays before it next
//! acts, which leaves it in the state HiGHS's synchronous notifications
//! would.

pub(crate) mod change_log;
pub(crate) mod clique_table;
pub(crate) mod conflict_pool;
pub(crate) mod data;
pub use data::MipStatus;
pub(crate) mod cut_generation;
pub(crate) mod cut_pool;
pub(crate) mod domain;
pub(crate) mod dynamic_row_matrix;
pub(crate) mod feasibility_jump;
pub(crate) mod gfk_solve;
pub(crate) mod heuristics;
pub(crate) mod implications;
pub(crate) mod lp_relaxation;
pub(crate) mod node_queue;
pub(crate) mod objective_function;
pub(crate) mod pseudocost;
pub(crate) mod redcost_fixing;
pub(crate) mod search;
pub(crate) mod separation;
pub(crate) mod separators;
pub(crate) mod solver;
pub(crate) mod sparse_vector_sum;
pub(crate) mod symmetry;
pub(crate) mod symmetry_detection;
pub(crate) mod transformed_lp;

/// HiGHS's `kHighsTiny`.
pub(crate) const TINY: f64 = 1e-14;

/// HiGHS's `HighsVarType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum VarType {
    #[default]
    Continuous = 0,
    Integer = 1,
    SemiContinuous = 2,
    SemiInteger = 3,
    ImplicitInteger = 4,
}

/// HiGHS's `HighsBoundType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum BoundType {
    #[default]
    Lower = 0,
    Upper = 1,
}

/// HiGHS's `HighsDomainChange`: one bound of one column.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DomainChange {
    pub boundval: f64,
    pub column: usize,
    pub boundtype: BoundType,
}

impl DomainChange {
    pub fn new(boundval: f64, column: usize, boundtype: BoundType) -> Self {
        Self { boundval, column, boundtype }
    }
}

impl PartialEq for DomainChange {
    fn eq(&self, other: &Self) -> bool {
        self.boundtype == other.boundtype && self.column == other.column && self.boundval == other.boundval
    }
}

/// HiGHS's `operator<`: by column, then bound type, then value.
impl PartialOrd for DomainChange {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(
            self.column
                .cmp(&other.column)
                .then(self.boundtype.cmp(&other.boundtype))
                .then(if self.boundval < other.boundval {
                    std::cmp::Ordering::Less
                } else if other.boundval < self.boundval {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }),
        )
    }
}

/// The problem the MIP solver works on (HiGHS's `mipsolver.model_` with the
/// row-wise copy `mipdata_->ARstart_`, `ARindex_`, `ARvalue_`).
#[derive(Clone, Debug, Default)]
pub(crate) struct MipModel {
    pub num_col: usize,
    pub num_row: usize,
    pub col_cost: Vec<f64>,
    pub col_lower: Vec<f64>,
    pub col_upper: Vec<f64>,
    pub row_lower: Vec<f64>,
    pub row_upper: Vec<f64>,
    pub integrality: Vec<VarType>,
    pub offset: f64,
    /// Column-wise: column `j`'s entries at `a_start[j]..a_start[j + 1]`.
    pub a_start: Vec<usize>,
    pub a_index: Vec<usize>,
    pub a_value: Vec<f64>,
    /// Row-wise: row `i`'s entries at `ar_start[i]..ar_start[i + 1]`.
    pub ar_start: Vec<usize>,
    pub ar_index: Vec<usize>,
    pub ar_value: Vec<f64>,
}

impl MipModel {
    pub fn is_col_integral(&self, col: usize) -> bool {
        self.integrality[col] != VarType::Continuous
    }

    pub fn is_col_integer(&self, col: usize) -> bool {
        self.integrality[col] == VarType::Integer
    }

    pub fn is_col_continuous(&self, col: usize) -> bool {
        self.integrality[col] == VarType::Continuous
    }

    /// HiGHS's `numNz`: `a_start[num_col]`, which presolve's column count
    /// can move ahead of the matrix it last wrote.
    pub fn num_nonzero(&self) -> usize {
        self.a_start.get(self.num_col).copied().unwrap_or(0)
    }

    /// Row `row`'s column indices and values.
    pub fn row(&self, row: usize) -> (&[usize], &[f64]) {
        let range = self.ar_start[row]..self.ar_start[row + 1];
        (&self.ar_index[range.clone()], &self.ar_value[range])
    }

    /// Fill the row-wise copy from the column-wise matrix, each row's
    /// entries in increasing column order.
    pub fn build_row_wise(&mut self) {
        let mut count = vec![0usize; self.num_row + 1];
        for &row in &self.a_index {
            count[row + 1] += 1;
        }
        for row in 0..self.num_row {
            count[row + 1] += count[row];
        }
        let mut next = count.clone();
        let num_nz = self.a_index.len();
        self.ar_index = vec![0; num_nz];
        self.ar_value = vec![0.0; num_nz];
        for col in 0..self.num_col {
            for k in self.a_start[col]..self.a_start[col + 1] {
                let row = self.a_index[k];
                let at = next[row];
                self.ar_index[at] = col;
                self.ar_value[at] = self.a_value[k];
                next[row] += 1;
            }
        }
        self.ar_start = count;
    }
}

/// C++'s `std::max(a, b)`: `a` unless `a < b`.
pub(crate) fn cmax(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// C++'s `std::min(a, b)`: `a` unless `b < a`.
pub(crate) fn cmin(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// libstdc++'s `std::partition` (its bidirectional version, which vectors
/// use): the elements for which `pred` holds first, in the order its
/// swaps leave them. Returns how many there are.
pub(crate) fn cpp_partition<T>(data: &mut [T], mut pred: impl FnMut(&T) -> bool) -> usize {
    let mut first = 0;
    let mut last = data.len();
    loop {
        loop {
            if first == last {
                return first;
            }
            if pred(&data[first]) {
                first += 1;
            } else {
                break;
            }
        }
        last -= 1;
        loop {
            if first == last {
                return first;
            }
            if !pred(&data[last]) {
                last -= 1;
            } else {
                break;
            }
        }
        data.swap(first, last);
        first += 1;
    }
}

/// libstdc++'s `std::partial_sort`: the first `middle` elements become the
/// smallest by `less`, sorted, by its heap select and heap sort (so ties
/// land where libstdc++ puts them, even for an empty `middle`).
pub(crate) fn cpp_partial_sort<T: Copy>(data: &mut [T], middle: usize, mut less: impl FnMut(&T, &T) -> bool) {
    if data.is_empty() {
        return;
    }
    cpp_make_heap(data, middle, &mut less);
    for i in middle..data.len() {
        if less(&data[i], &data[0]) {
            let value = data[i];
            data[i] = data[0];
            cpp_adjust_heap(data, 0, middle as isize, value, &mut less);
        }
    }
    let mut end = middle;
    while end > 1 {
        end -= 1;
        let value = data[end];
        data[end] = data[0];
        cpp_adjust_heap(data, 0, end as isize, value, &mut less);
    }
}

/// libstdc++'s `std::make_heap` on `data[..len]`.
fn cpp_make_heap<T: Copy>(data: &mut [T], len: usize, less: &mut impl FnMut(&T, &T) -> bool) {
    if len < 2 {
        return;
    }
    let mut parent = (len - 2) / 2;
    loop {
        let value = data[parent];
        cpp_adjust_heap(data, parent as isize, len as isize, value, less);
        if parent == 0 {
            return;
        }
        parent -= 1;
    }
}

/// libstdc++'s `std::__adjust_heap` then `std::__push_heap`, on a heap of
/// `len` elements, with its signed index arithmetic.
fn cpp_adjust_heap<T: Copy>(data: &mut [T], mut hole: isize, len: isize, value: T, less: &mut impl FnMut(&T, &T) -> bool) {
    let top = hole;
    let mut second_child = hole;
    while second_child < (len - 1) / 2 {
        second_child = 2 * (second_child + 1);
        if less(&data[second_child as usize], &data[(second_child - 1) as usize]) {
            second_child -= 1;
        }
        data[hole as usize] = data[second_child as usize];
        hole = second_child;
    }
    if len & 1 == 0 && second_child == (len - 2) / 2 {
        second_child = 2 * (second_child + 1);
        data[hole as usize] = data[(second_child - 1) as usize];
        hole = second_child - 1;
    }
    let mut parent = (hole - 1) / 2;
    while hole > top && less(&data[parent as usize], &value) {
        data[hole as usize] = data[parent as usize];
        hole = parent;
        parent = (hole - 1) / 2;
    }
    data[hole as usize] = value;
}

/// libstdc++'s `std::priority_queue::push`: `push_heap` on the vector.
pub(crate) fn cpp_heap_push<T: Copy>(heap: &mut Vec<T>, value: T, mut less: impl FnMut(&T, &T) -> bool) {
    heap.push(value);
    let mut hole = heap.len() as isize - 1;
    let mut parent = (hole - 1) / 2;
    while hole > 0 && less(&heap[parent as usize], &value) {
        heap[hole as usize] = heap[parent as usize];
        hole = parent;
        parent = (hole - 1) / 2;
    }
    heap[hole as usize] = value;
}

/// libstdc++'s `std::priority_queue::pop`: `pop_heap`, then the last
/// element, which it returns.
pub(crate) fn cpp_heap_pop<T: Copy>(heap: &mut Vec<T>, mut less: impl FnMut(&T, &T) -> bool) -> Option<T> {
    let len = heap.len();
    if len > 1 {
        let last = len - 1;
        let value = heap[last];
        heap[last] = heap[0];
        cpp_adjust_heap(heap, 0, last as isize, value, &mut less);
    }
    heap.pop()
}

/// `problem` as HiGHS's MIP solver takes it after `passModel` (bounds
/// beyond `infinite_bound` infinite, entries no bigger than
/// `small_matrix_value` dropped): the model as given, the model minimised,
/// and the sign between them.
fn mip_models(problem: &crate::Problem) -> (MipModel, MipModel, f64) {
    let options = crate::Options::default();
    let (start, index, value) = problem.col_wise();
    let mut a_start = vec![0; start.len()];
    let mut a_index = Vec::with_capacity(index.len());
    let mut a_value = Vec::with_capacity(value.len());
    for col in 0..start.len() - 1 {
        for k in start[col]..start[col + 1] {
            if value[k].abs() > options.small_matrix_value {
                a_index.push(index[k]);
                a_value.push(value[k]);
            }
        }
        a_start[col + 1] = a_index.len();
    }
    let bounds = |values: &[f64]| {
        values
            .iter()
            .map(|&value| if value.abs() >= options.infinite_bound { value.signum() * f64::INFINITY } else { value })
            .collect::<Vec<_>>()
    };
    let sign = if problem.sense == crate::Sense::Maximise { -1.0 } else { 1.0 };
    let num_col = problem.col_cost.len();
    let orig = MipModel {
        num_col,
        num_row: problem.row_lower.len(),
        col_cost: problem.col_cost.clone(),
        col_lower: bounds(&problem.col_lower),
        col_upper: bounds(&problem.col_upper),
        row_lower: bounds(&problem.row_lower),
        row_upper: bounds(&problem.row_upper),
        integrality: (0..num_col)
            .map(|j| {
                if problem.integrality.get(j).copied().unwrap_or(false) {
                    VarType::Integer
                } else {
                    VarType::Continuous
                }
            })
            .collect(),
        offset: problem.offset,
        a_start,
        a_index,
        a_value,
        ..MipModel::default()
    };
    let mut model = orig.clone();
    model.col_cost.iter_mut().for_each(|c| *c *= sign);
    model.offset *= sign;
    model.build_row_wise();
    (orig, model, sign)
}

/// A MIP solve's limits: HiGHS's options of the same names.
#[derive(Clone, Debug)]
pub struct MipSettings {
    /// Seconds, for the whole solve: completing a seed comes out of it.
    pub time_limit: f64,
    pub mip_rel_gap: f64,
    pub mip_detect_symmetry: bool,
}

impl Default for MipSettings {
    fn default() -> Self {
        let options = data::MipOptions::default();
        Self {
            time_limit: options.time_limit,
            mip_rel_gap: options.mip_rel_gap,
            mip_detect_symmetry: options.mip_detect_symmetry,
        }
    }
}

/// What a MIP solve found, in the problem's own terms.
#[derive(Clone, Debug)]
pub struct MipSolution {
    pub status: MipStatus,
    /// The best solution found, empty when there is none.
    pub col_value: Vec<f64>,
    pub row_value: Vec<f64>,
    /// Its objective, infinite when there is none.
    pub objective: f64,
    /// The bound on the optimum the search proved.
    pub dual_bound: f64,
    pub gap: f64,
    pub nodes: i64,
    pub lp_iterations: i64,
}

/// Solve `problem` as HiGHS's `Highs::run` does a MIP with one thread,
/// starting from `seed` (a value per column) if there is one, and stopping
/// with [`MipStatus::Interrupt`] once `interrupt` says so.
pub fn solve_mip(problem: &crate::Problem, settings: &MipSettings, seed: Option<&[f64]>, interrupt: Option<crate::Interrupt>) -> MipSolution {
    let started = web_time::Instant::now();
    let mut options = data::MipOptions {
        time_limit: settings.time_limit,
        mip_rel_gap: settings.mip_rel_gap,
        mip_detect_symmetry: settings.mip_detect_symmetry,
        ..data::MipOptions::default()
    };
    let num_col = problem.num_cols();
    let num_row = problem.num_rows();
    let (orig, model, sign) = mip_models(problem);
    // `Highs::setSolution`: a primal solution needs a value per column; its
    // row values are `calculateRowValuesQuad`'s.
    let mut start = seed.filter(|seed| num_col > 0 && seed.len() >= num_col).map(|seed| {
        let col_value = seed[..num_col].to_vec();
        let row_value = calculate_row_values_quad(&orig, &col_value);
        (col_value, row_value)
    });
    let is_mip = orig.integrality.iter().any(|&t| t != VarType::Continuous);
    if is_mip && let Some((col_value, row_value)) = start.take() {
        start = complete_solution_from_discrete_assignment(problem, &orig, &options, col_value, row_value, interrupt.clone());
        // One deadline for the whole call: what completing the seed took
        // comes out of the main solve's time.
        options.time_limit = (options.time_limit - started.elapsed().as_secs_f64()).max(0.0);
    }
    if num_col == 0 {
        // HiGHS's `kModelEmpty`: nothing to choose, the offset to pay.
        let row_value = vec![0.0; num_row];
        let feasible = problem.row_lower.iter().zip(&problem.row_upper).all(|(&lower, &upper)| lower <= 0.0 && 0.0 <= upper);
        return MipSolution {
            status: if feasible { MipStatus::Optimal } else { MipStatus::Infeasible },
            col_value: Vec::new(),
            row_value,
            objective: problem.offset,
            dual_bound: problem.offset,
            gap: 0.0,
            nodes: 0,
            lp_iterations: 0,
        };
    }
    if !is_mip {
        return lp_as_mip(problem, &options, start.map(|(col_value, _)| col_value), interrupt);
    }
    let mut solver = solver::MipSolver::new(options, model, orig, sign);
    solver.data.interrupt = interrupt;
    if let Some((col_value, row_value)) = start {
        solver.data.set_start_solution(col_value, &row_value);
    }
    let result = solver.run();
    // `callSolveMip`: the incumbent and `productQuad`'s row values.
    let have_solution = result.objective != f64::INFINITY;
    let row_value = if have_solution {
        product_quad(&solver.data.orig_model, &result.solution)
    } else {
        Vec::new()
    };
    MipSolution {
        status: result.status,
        col_value: if have_solution { result.solution } else { Vec::new() },
        row_value,
        objective: result.objective,
        dual_bound: result.dual_bound,
        gap: result.gap,
        nodes: result.nodes,
        lp_iterations: result.lp_iterations,
    }
}

/// HiGHS's `calculateRowValuesQuad`: each product in quad precision.
fn calculate_row_values_quad(lp: &MipModel, col_value: &[f64]) -> Vec<f64> {
    let mut quad = vec![crate::util::CDouble::new(0.0); lp.num_row];
    for col in 0..lp.num_col {
        for k in lp.a_start[col]..lp.a_start[col + 1] {
            quad[lp.a_index[k]] += crate::util::CDouble::new(col_value[col]) * lp.a_value[k];
        }
    }
    quad.into_iter().map(crate::util::CDouble::value).collect()
}

/// HiGHS's `HighsSparseMatrix::productQuad`: each product a double, summed
/// in quad precision.
fn product_quad(lp: &MipModel, col_value: &[f64]) -> Vec<f64> {
    let mut quad = vec![crate::util::CDouble::new(0.0); lp.num_row];
    for col in 0..lp.num_col {
        for k in lp.a_start[col]..lp.a_start[col + 1] {
            quad[lp.a_index[k]] += col_value[col] * lp.a_value[k];
        }
    }
    quad.into_iter().map(crate::util::CDouble::value).collect()
}

/// HiGHS's `assessLpPrimalSolution` on a MIP: whether the solution is
/// within the MIP feasibility tolerance of every bound, integrality and
/// row.
fn assess_mip_primal_solution(lp: &MipModel, options: &data::MipOptions, primal_feasibility_tolerance: f64, col_value: &[f64], row_value: &[f64]) -> bool {
    let tolerance = options.mip_feasibility_tolerance;
    let row_values = calculate_row_values_quad(lp, col_value);
    for col in 0..lp.num_col {
        let (primal, lower, upper) = (col_value[col], lp.col_lower[col], lp.col_upper[col]);
        let col_infeasibility = if primal < lower - primal_feasibility_tolerance {
            lower - primal
        } else if primal > upper + primal_feasibility_tolerance {
            primal - upper
        } else {
            0.0
        };
        if col_infeasibility > tolerance {
            return false;
        }
        if lp.integrality[col] == VarType::Integer && crate::integers::fractionality(primal) > tolerance {
            return false;
        }
    }
    for row in 0..lp.num_row {
        let (primal, lower, upper) = (row_value[row], lp.row_lower[row], lp.row_upper[row]);
        if primal < lower - tolerance || primal > upper + tolerance {
            return false;
        }
        // A residual makes the solution invalid.
        if (primal - row_values[row]).abs() > primal_feasibility_tolerance {
            return false;
        }
    }
    true
}

/// HiGHS's `completeSolutionFromDiscreteAssignment`: a seed that is not
/// feasible becomes, if it can, the solution of the problem with its
/// integer-valued discrete columns fixed (an LP when that fixes all of
/// them, otherwise a MIP of at most `mip_max_start_nodes` nodes).
fn complete_solution_from_discrete_assignment(
    problem: &crate::Problem,
    lp: &MipModel,
    options: &data::MipOptions,
    col_value: Vec<f64>,
    row_value: Vec<f64>,
    interrupt: Option<crate::Interrupt>,
) -> Option<(Vec<f64>, Vec<f64>)> {
    let lp_options = crate::Options::default();
    let primal_feasibility_tolerance = lp_options.primal_feasibility_tolerance;
    // HiGHS's `kHighsUndefined` marks a value left for it to choose.
    let undefined = |value: f64| value == f64::INFINITY;
    let contains_undefined_values = col_value.iter().any(|&value| undefined(value));
    if !contains_undefined_values && assess_mip_primal_solution(lp, options, primal_feasibility_tolerance, &col_value, &row_value) {
        return Some((col_value, row_value));
    }
    let mut fixed = problem.clone();
    let mut num_fixed_discrete_variable = 0;
    let mut num_unfixed_discrete_variable = 0;
    for col in 0..lp.num_col {
        if lp.integrality[col] == VarType::Continuous {
            continue;
        }
        let primal = col_value[col];
        if undefined(primal) || crate::integers::fractionality(primal) > options.mip_feasibility_tolerance {
            num_unfixed_discrete_variable += 1;
        } else {
            num_fixed_discrete_variable += 1;
            fixed.col_lower[col] = primal;
            fixed.col_upper[col] = primal;
            fixed.integrality[col] = false;
        }
    }
    let num_continuous_variable = lp.num_col - num_fixed_discrete_variable - num_unfixed_discrete_variable;
    if num_unfixed_discrete_variable == 0 {
        if num_continuous_variable == 0 {
            // No feasible solution can be deduced.
            return None;
        }
        let options = crate::Options {
            time_limit: options.time_limit,
            ..lp_options
        };
        let mut solver = crate::LpSolver::new(&fixed, options);
        solver.set_interrupt(interrupt);
        solver.run();
        return Some((solver.col_value()?.to_vec(), solver.row_value()?.to_vec()));
    }
    let (orig, model, sign) = mip_models(&fixed);
    let options = data::MipOptions {
        mip_max_nodes: MIP_MAX_START_NODES,
        ..options.clone()
    };
    let mut solver = solver::MipSolver::new(options, model, orig, sign);
    solver.data.interrupt = interrupt;
    let result = solver.run();
    (result.objective != f64::INFINITY).then(|| {
        let row_value = product_quad(&solver.data.orig_model, &result.solution);
        (result.solution, row_value)
    })
}

/// HiGHS's `mip_max_start_nodes`.
const MIP_MAX_START_NODES: i64 = 500;

/// `Highs::run` on a problem with no integer columns: its LP.
fn lp_as_mip(problem: &crate::Problem, options: &data::MipOptions, _start: Option<Vec<f64>>, interrupt: Option<crate::Interrupt>) -> MipSolution {
    let lp_options = crate::Options {
        time_limit: options.time_limit,
        ..crate::Options::default()
    };
    let mut solver = crate::LpSolver::new(problem, lp_options);
    solver.set_interrupt(interrupt);
    let status = match solver.run() {
        crate::Status::Optimal => MipStatus::Optimal,
        crate::Status::Infeasible => MipStatus::Infeasible,
        crate::Status::Unbounded => MipStatus::Unbounded,
        crate::Status::UnboundedOrInfeasible => MipStatus::UnboundedOrInfeasible,
        crate::Status::TimeLimit => MipStatus::TimeLimit,
        crate::Status::Interrupt => MipStatus::Interrupt,
        _ => MipStatus::NotSet,
    };
    let col_value = solver.col_value().map(<[f64]>::to_vec).unwrap_or_default();
    let row_value = solver.row_value().map(<[f64]>::to_vec).unwrap_or_default();
    let objective = if col_value.is_empty() { f64::INFINITY } else { solver.objective() };
    MipSolution {
        status,
        dual_bound: if status == MipStatus::Optimal { objective } else { f64::NAN },
        gap: if status == MipStatus::Optimal { 0.0 } else { f64::INFINITY },
        col_value,
        row_value,
        objective,
        nodes: 0,
        lp_iterations: solver.iterations() as i64,
    }
}
