//! The Feasibility Jump heuristic HiGHS ships (`feasibilityjump.hh`, by
//! Luteberget and Sartor): weighted local search over jump moves, with the
//! C++ standard library's `std::mt19937` and `uniform_real_distribution` as
//! libstdc++ implements them.

const INF: f64 = f64::INFINITY;

/// `std::mt19937`.
pub(crate) struct Mt19937 {
    state: [u32; 624],
    index: usize,
}

impl Mt19937 {
    pub fn new(seed: u32) -> Self {
        let mut state = [0u32; 624];
        state[0] = seed;
        for i in 1..624 {
            state[i] = 1812433253u32.wrapping_mul(state[i - 1] ^ (state[i - 1] >> 30)).wrapping_add(i as u32);
        }
        Self { state, index: 624 }
    }

    fn twist(&mut self) {
        const UPPER: u32 = 0x8000_0000;
        const LOWER: u32 = 0x7fff_ffff;
        const MATRIX: u32 = 0x9908_b0df;
        for k in 0..624 {
            let y = (self.state[k] & UPPER) | (self.state[(k + 1) % 624] & LOWER);
            let mut next = self.state[(k + 397) % 624] ^ (y >> 1);
            if y & 1 != 0 {
                next ^= MATRIX;
            }
            self.state[k] = next;
        }
        self.index = 0;
    }

    pub fn next_u32(&mut self) -> u32 {
        if self.index >= 624 {
            self.twist();
        }
        let mut z = self.state[self.index];
        self.index += 1;
        z ^= z >> 11;
        z ^= (z << 7) & 0x9d2c_5680;
        z ^= (z << 15) & 0xefc6_0000;
        z ^= z >> 18;
        z
    }

    /// libstdc++'s `generate_canonical<double, 53>`: two draws.
    fn canonical(&mut self) -> f64 {
        let r = 4294967296.0f64;
        let mut sum = 0.0f64;
        let mut tmp = 1.0f64;
        for _ in 0..2 {
            sum += self.next_u32() as f64 * tmp;
            tmp *= r;
        }
        let ret = sum / tmp;
        if ret >= 1.0 { 1.0f64.next_down() } else { ret }
    }

    /// `std::uniform_real_distribution<double>(a, b)(rng)`.
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        self.canonical() * (b - a) + a
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowType {
    Equal,
    Lte,
    Gte,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VarType {
    Continuous,
    Integer,
}

fn eq(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() < tol
}

#[derive(Clone, Copy, Debug)]
struct IdxCoeff {
    idx: usize,
    coeff: f64,
}

#[derive(Clone, Debug)]
struct Var {
    vartype: VarType,
    lb: f64,
    ub: f64,
    objective_coeff: f64,
    coeffs: Vec<IdxCoeff>,
}

#[derive(Clone, Debug)]
struct Constraint {
    sense: RowType,
    rhs: f64,
    coeffs: Vec<IdxCoeff>,
    weight: f64,
    incumbent_lhs: f64,
    violated_idx: Option<usize>,
}

impl Constraint {
    fn score(&self, lhs: f64) -> f64 {
        match self.sense {
            RowType::Equal => -(lhs - self.rhs).abs(),
            RowType::Lte => -(0.0f64.max(lhs - self.rhs)),
            RowType::Gte => -(0.0f64.max(self.rhs - lhs)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Move {
    value: f64,
    score: f64,
}

impl Move {
    fn undef() -> Self {
        Self { value: f64::NAN, score: -INF }
    }
}

impl Default for Move {
    fn default() -> Self {
        Self { value: 0.0, score: 0.0 }
    }
}

struct LhsModification {
    var_idx: usize,
    constraint_idx: usize,
    coeff: f64,
    old_lhs: f64,
    new_lhs: f64,
}

struct Problem {
    equality_tolerance: f64,
    violation_tolerance: f64,
    vars: Vec<Var>,
    constraints: Vec<Constraint>,
    incumbent_assignment: Vec<f64>,
    violated_constraints: Vec<usize>,
    n_nonzeros: usize,
    incumbent_objective: f64,
}

impl Problem {
    fn add_var(&mut self, vartype: VarType, lb: f64, ub: f64, obj_coeff: f64) -> usize {
        let idx = self.vars.len();
        self.vars.push(Var {
            vartype,
            lb,
            ub,
            objective_coeff: obj_coeff,
            coeffs: Vec::new(),
        });
        self.incumbent_assignment.push(lb);
        idx
    }

    /// `addConstraint` without continuous relaxation (HiGHS passes 0).
    fn add_constraint(&mut self, sense: RowType, rhs: f64, row_var_idxs: &[usize], row_coeffs: &[f64]) {
        let coeffs: Vec<IdxCoeff> = row_var_idxs.iter().zip(row_coeffs).map(|(&idx, &coeff)| IdxCoeff { idx, coeff }).collect();
        if coeffs.is_empty() {
            return;
        }
        let new_constraint_idx = self.constraints.len();
        for c in &coeffs {
            self.vars[c.idx].coeffs.push(IdxCoeff {
                idx: new_constraint_idx,
                coeff: c.coeff,
            });
        }
        self.n_nonzeros += coeffs.len();
        self.constraints.push(Constraint {
            sense,
            rhs,
            coeffs,
            weight: 1.0,
            incumbent_lhs: f64::NAN,
            violated_idx: None,
        });
    }

    fn reset_incumbent(&mut self, initial_values: &[f64]) {
        self.incumbent_assignment.copy_from_slice(initial_values);
        self.incumbent_objective = 0.0;
        for i in 0..self.vars.len() {
            self.incumbent_objective += self.vars[i].objective_coeff * self.incumbent_assignment[i];
        }
        self.violated_constraints.clear();
        for c_idx in 0..self.constraints.len() {
            let mut lhs = 0.0;
            for vc in &self.constraints[c_idx].coeffs {
                lhs += vc.coeff * self.incumbent_assignment[vc.idx];
            }
            let cstr = &mut self.constraints[c_idx];
            cstr.incumbent_lhs = lhs;
            if cstr.score(lhs) < -self.violation_tolerance {
                cstr.violated_idx = Some(self.violated_constraints.len());
                self.violated_constraints.push(c_idx);
            } else {
                cstr.violated_idx = None;
            }
        }
    }

    /// `setValue`: the effort, reporting each other variable's changed LHS.
    fn set_value(&mut self, var_idx: usize, new_value: f64, mut f: impl FnMut(&mut Self, LhsModification)) -> usize {
        let mut dt = 0;
        let old_value = self.incumbent_assignment[var_idx];
        let delta = new_value - old_value;
        self.incumbent_assignment[var_idx] = new_value;
        self.incumbent_objective += self.vars[var_idx].objective_coeff * delta;
        for k in 0..self.vars[var_idx].coeffs.len() {
            let cstr_coeff = self.vars[var_idx].coeffs[k];
            let c = cstr_coeff.idx;
            let old_lhs = self.constraints[c].incumbent_lhs;
            let new_lhs = old_lhs + cstr_coeff.coeff * delta;
            self.constraints[c].incumbent_lhs = new_lhs;
            let new_cost = self.constraints[c].score(new_lhs);
            if new_cost < -self.violation_tolerance && self.constraints[c].violated_idx.is_none() {
                self.constraints[c].violated_idx = Some(self.violated_constraints.len());
                self.violated_constraints.push(c);
            }
            if new_cost >= -self.violation_tolerance
                && let Some(this_violated_idx) = self.constraints[c].violated_idx
            {
                let last_violated_idx = self.violated_constraints.len() - 1;
                let last_constraint_idx = self.violated_constraints[last_violated_idx];
                self.violated_constraints.swap(this_violated_idx, last_violated_idx);
                self.constraints[last_constraint_idx].violated_idx = Some(this_violated_idx);
                self.constraints[c].violated_idx = None;
                self.violated_constraints.pop();
            }
            dt += self.constraints[c].coeffs.len();
            for j in 0..self.constraints[c].coeffs.len() {
                let var_coeff = self.constraints[c].coeffs[j];
                if var_coeff.idx != var_idx {
                    f(
                        self,
                        LhsModification {
                            var_idx: var_coeff.idx,
                            constraint_idx: c,
                            coeff: var_coeff.coeff,
                            old_lhs,
                            new_lhs,
                        },
                    );
                }
            }
        }
        dt
    }
}

/// `modifyMove`.
fn modify_move(m: &LhsModification, problem: &Problem, mv: &mut Move) {
    let c = &problem.constraints[m.constraint_idx];
    let incumbent = problem.incumbent_assignment[m.var_idx];
    let old_modified_lhs = m.old_lhs + m.coeff * (mv.value - incumbent);
    let old_score_term = c.weight * (c.score(old_modified_lhs) - c.score(m.old_lhs));
    let new_modified_lhs = m.new_lhs + m.coeff * (mv.value - incumbent);
    let new_score_term = c.weight * (c.score(new_modified_lhs) - c.score(m.new_lhs));
    mv.score += new_score_term - old_score_term;
}

/// `JumpMove`.
struct JumpMove {
    equality_tolerance: f64,
    moves: Vec<Move>,
    best_shift_buffer: Vec<(f64, f64)>,
}

impl JumpMove {
    fn update_value(&mut self, problem: &Problem, var_idx: usize) {
        self.best_shift_buffer.clear();
        let var_incumbent_value = problem.incumbent_assignment[var_idx];
        let lower = problem.vars[var_idx].lb;
        let upper = problem.vars[var_idx].ub;
        let mut current_value = lower;
        let mut current_slope = 0.0;
        for cell in &problem.vars[var_idx].coeffs {
            let constraint = &problem.constraints[cell.idx];
            let bounds: &[(f64, f64)] = match constraint.sense {
                RowType::Lte => &[(-INF, constraint.rhs)],
                RowType::Gte => &[(constraint.rhs, INF)],
                RowType::Equal => &[(-INF, constraint.rhs), (constraint.rhs, constraint.rhs), (constraint.rhs, INF)],
            };
            for &bound in bounds {
                let residual_incumbent = constraint.incumbent_lhs - cell.coeff * var_incumbent_value;
                let mut valid_range = ((1.0 / cell.coeff) * (bound.0 - residual_incumbent), (1.0 / cell.coeff) * (bound.1 - residual_incumbent));
                if problem.vars[var_idx].vartype == VarType::Integer {
                    valid_range = ((valid_range.0 - self.equality_tolerance).ceil(), (valid_range.1 + self.equality_tolerance).floor());
                }
                if valid_range.0 > valid_range.1 {
                    continue;
                }
                if valid_range.0 > current_value {
                    current_slope -= constraint.weight;
                    if valid_range.0 < upper {
                        self.best_shift_buffer.push((valid_range.0, constraint.weight));
                    }
                }
                if valid_range.1 <= current_value {
                    current_slope += constraint.weight;
                } else if valid_range.1 < upper {
                    self.best_shift_buffer.push((valid_range.1, constraint.weight));
                }
            }
        }
        if lower.is_finite() {
            self.best_shift_buffer.push((lower, 0.0));
        }
        if upper.is_finite() {
            self.best_shift_buffer.push((upper, 0.0));
        }
        self.best_shift_buffer.sort_by(|a, b| a.partial_cmp(b).expect("no NaN shifts"));
        let mut current_score = 0.0;
        current_value = self.best_shift_buffer.first().map_or(var_incumbent_value, |item| item.0);
        let mut best_score = current_score;
        let mut best_value = current_value;
        let tol = self.equality_tolerance;
        for &item in &self.best_shift_buffer {
            current_score += (item.0 - current_value) * current_slope;
            current_slope += item.1;
            current_value = item.0;
            if eq(best_value, var_incumbent_value, tol) || (!eq(current_value, var_incumbent_value, tol) && current_score < best_score) {
                best_score = current_score;
                best_value = current_value;
            }
            if !eq(best_value, var_incumbent_value, tol) && current_slope >= 0.0 {
                break;
            }
        }
        self.moves[var_idx].value = best_value;
    }
}

/// `updateGoodMoves`.
fn update_good_moves(moves: &[Move], good_vars_set: &mut Vec<usize>, good_vars_set_idx: &mut [Option<usize>], var_idx: usize) {
    let any_good_moves = moves[var_idx].score > 0.0;
    if any_good_moves && good_vars_set_idx[var_idx].is_none() {
        good_vars_set_idx[var_idx] = Some(good_vars_set.len());
        good_vars_set.push(var_idx);
    } else if let (false, Some(this_set_idx)) = (any_good_moves, good_vars_set_idx[var_idx]) {
        let last_set_idx = good_vars_set.len() - 1;
        let last_var_idx = good_vars_set[last_set_idx];
        good_vars_set.swap(this_set_idx, last_set_idx);
        good_vars_set_idx[last_var_idx] = Some(this_set_idx);
        good_vars_set_idx[var_idx] = None;
        good_vars_set.pop();
    }
}

/// What the solve reports to its callback.
pub(crate) struct FjStatus<'a> {
    pub total_effort: usize,
    pub effort_since_last_improvement: usize,
    pub solution_objective_value: f64,
    pub solution: Option<&'a [f64]>,
}

/// `FeasibilityJumpSolver`.
pub(crate) struct FeasibilityJumpSolver {
    problem: Problem,
    jump_move: JumpMove,
    good_vars_set: Vec<usize>,
    good_vars_set_idx: Vec<Option<usize>>,
    rng: Mt19937,
    best_objective: f64,
    objective_weight: f64,
    best_violation_score: usize,
    effort_at_last_callback: usize,
    effort_at_last_improvement: usize,
    total_effort: usize,
    weight_update_decay: f64,
    weight_update_increment: f64,
}

const RANDOM_VAR_PROBABILITY: f64 = 0.001;
const RANDOM_CELL_PROBABILITY: f64 = 0.01;
const MAX_MOVES_TO_EVALUATE: usize = 25;
const MIN_RELATIVE_OBJECTIVE_IMPROVEMENT: f64 = 1e-4;

impl FeasibilityJumpSolver {
    pub fn new(seed: u32, equality_tolerance: f64, violation_tolerance: f64) -> Self {
        Self {
            problem: Problem {
                equality_tolerance,
                violation_tolerance,
                vars: Vec::new(),
                constraints: Vec::new(),
                incumbent_assignment: Vec::new(),
                violated_constraints: Vec::new(),
                n_nonzeros: 0,
                incumbent_objective: f64::NAN,
            },
            jump_move: JumpMove {
                equality_tolerance,
                moves: Vec::new(),
                best_shift_buffer: Vec::new(),
            },
            good_vars_set: Vec::new(),
            good_vars_set_idx: Vec::new(),
            rng: Mt19937::new(seed),
            best_objective: INF,
            objective_weight: 0.0,
            best_violation_score: usize::MAX,
            effort_at_last_callback: 0,
            effort_at_last_improvement: 0,
            total_effort: 0,
            weight_update_decay: 1.0,
            weight_update_increment: 1.0,
        }
    }

    pub fn add_var(&mut self, vartype: VarType, lb: f64, ub: f64, obj_coeff: f64) -> usize {
        self.good_vars_set_idx.push(None);
        self.problem.add_var(vartype, lb, ub, obj_coeff)
    }

    pub fn add_constraint(&mut self, sense: RowType, rhs: f64, row_var_idxs: &[usize], row_coeffs: &[f64]) {
        self.problem.add_constraint(sense, rhs, row_var_idxs, row_coeffs);
    }

    /// `solve`: local search from `initial_values` until the callback
    /// says to stop (false).
    pub fn solve(&mut self, initial_values: &[f64], mut callback: impl FnMut(FjStatus) -> bool) {
        self.init(initial_values);
        for _step in 0..i32::MAX {
            if self.user_terminate(&mut callback, false) {
                break;
            }
            if self.problem.violated_constraints.len() < self.best_violation_score {
                self.effort_at_last_improvement = self.total_effort;
                self.best_violation_score = self.problem.violated_constraints.len();
            }
            if self.problem.violated_constraints.is_empty() && self.problem.incumbent_objective < self.best_objective {
                let relative_objective_improvement = (self.best_objective - self.problem.incumbent_objective) / 1.0f64.max(self.problem.incumbent_objective.abs());
                if relative_objective_improvement > MIN_RELATIVE_OBJECTIVE_IMPROVEMENT {
                    self.effort_at_last_improvement = self.total_effort;
                    self.best_objective = self.problem.incumbent_objective;
                    if self.user_terminate(&mut callback, true) {
                        break;
                    }
                }
            }
            if self.problem.vars.is_empty() {
                break;
            }
            let var = self.select_variable();
            self.do_variable_move(var);
        }
    }

    fn init(&mut self, initial_values: &[f64]) {
        self.problem.reset_incumbent(initial_values);
        self.jump_move.moves = vec![Move::default(); self.problem.vars.len()];
        self.total_effort += self.problem.n_nonzeros;
        self.good_vars_set.clear();
        for i in 0..self.problem.vars.len() {
            self.reset_moves(i);
        }
    }

    fn select_variable(&mut self) -> usize {
        if !self.good_vars_set.is_empty() {
            if self.rng.uniform(0.0, 1.0) < RANDOM_VAR_PROBABILITY {
                return self.good_vars_set[self.rng.next_u32() as usize % self.good_vars_set.len()];
            }
            let sample_size = MAX_MOVES_TO_EVALUATE.min(self.good_vars_set.len());
            self.total_effort += sample_size;
            let mut best_score = -INF;
            let mut best_var = None;
            for _ in 0..sample_size {
                let setidx = self.rng.next_u32() as usize % self.good_vars_set.len();
                let var_idx = self.good_vars_set[setidx];
                let mv = self.best_move(var_idx);
                if mv.score > best_score {
                    best_score = mv.score;
                    best_var = Some(var_idx);
                }
            }
            return best_var.expect("a good variable");
        }
        self.update_weights();
        if !self.problem.violated_constraints.is_empty() {
            let cstr_idx = self.problem.violated_constraints[self.rng.next_u32() as usize % self.problem.violated_constraints.len()];
            if self.rng.uniform(0.0, 1.0) < RANDOM_CELL_PROBABILITY {
                let n = self.problem.constraints[cstr_idx].coeffs.len();
                return self.problem.constraints[cstr_idx].coeffs[self.rng.next_u32() as usize % n].idx;
            }
            let mut best_score = -INF;
            let mut best_var_idx = usize::MAX;
            for k in 0..self.problem.constraints[cstr_idx].coeffs.len() {
                let idx = self.problem.constraints[cstr_idx].coeffs[k].idx;
                let mv = self.best_move(idx);
                if mv.score > best_score {
                    best_score = mv.score;
                    best_var_idx = idx;
                }
            }
            return best_var_idx;
        }
        self.rng.next_u32() as usize % self.problem.vars.len()
    }

    fn update_weights(&mut self) {
        let mut rescale_all_weights = false;
        let mut dt = 0;
        if self.problem.violated_constraints.is_empty() {
            self.objective_weight += self.weight_update_increment;
            if self.objective_weight > 1.0e20 {
                rescale_all_weights = true;
            }
            dt += self.problem.vars.len();
            for var_idx in 0..self.problem.vars.len() {
                let mv = &mut self.jump_move.moves[var_idx];
                mv.score += self.weight_update_increment * self.problem.vars[var_idx].objective_coeff * (mv.value - self.problem.incumbent_assignment[var_idx]);
            }
        } else {
            for v in 0..self.problem.violated_constraints.len() {
                let c_idx = self.problem.violated_constraints[v];
                self.problem.constraints[c_idx].weight += self.weight_update_increment;
                if self.problem.constraints[c_idx].weight > 1.0e20 {
                    rescale_all_weights = true;
                }
                dt += self.problem.constraints[c_idx].coeffs.len();
                for k in 0..self.problem.constraints[c_idx].coeffs.len() {
                    let cell = self.problem.constraints[c_idx].coeffs[k];
                    let constraint = &self.problem.constraints[c_idx];
                    let mv = &mut self.jump_move.moves[cell.idx];
                    let candidate_lhs = constraint.incumbent_lhs + cell.coeff * (mv.value - self.problem.incumbent_assignment[cell.idx]);
                    let diff = self.weight_update_increment * (constraint.score(candidate_lhs) - constraint.score(constraint.incumbent_lhs));
                    mv.score += diff;
                    self.update_good_moves(cell.idx);
                }
            }
        }
        self.weight_update_increment /= self.weight_update_decay;
        if rescale_all_weights {
            self.weight_update_increment *= 1.0e-20;
            self.objective_weight *= 1.0e-20;
            for c in self.problem.constraints.iter_mut() {
                c.weight *= 1.0e-20;
            }
            dt += self.problem.constraints.len();
            for i in 0..self.problem.vars.len() {
                self.reset_moves(i);
            }
        }
        self.total_effort += dt;
    }

    fn best_move(&self, var_idx: usize) -> Move {
        let mut best = Move::undef();
        let mv = self.jump_move.moves[var_idx];
        if mv.score > best.score {
            best = mv;
        }
        best
    }

    fn do_variable_move(&mut self, var_idx: usize) {
        let new_value = self.best_move(var_idx).value;
        let mut moves = std::mem::take(&mut self.jump_move.moves);
        let mut good_vars_set = std::mem::take(&mut self.good_vars_set);
        let mut good_vars_set_idx = std::mem::take(&mut self.good_vars_set_idx);
        let dt = self.problem.set_value(var_idx, new_value, |problem, m| {
            modify_move(&m, problem, &mut moves[m.var_idx]);
            update_good_moves(&moves, &mut good_vars_set, &mut good_vars_set_idx, m.var_idx);
        });
        self.jump_move.moves = moves;
        self.good_vars_set = good_vars_set;
        self.good_vars_set_idx = good_vars_set_idx;
        self.total_effort += dt;
        self.reset_moves(var_idx);
    }

    fn update_good_moves(&mut self, var_idx: usize) {
        update_good_moves(&self.jump_move.moves, &mut self.good_vars_set, &mut self.good_vars_set_idx, var_idx);
    }

    fn reset_moves(&mut self, var_idx: usize) {
        self.total_effort += self.problem.vars[var_idx].coeffs.len();
        self.jump_move.update_value(&self.problem, var_idx);
        let mv = &mut self.jump_move.moves[var_idx];
        mv.score = 0.0;
        mv.score += self.objective_weight * self.problem.vars[var_idx].objective_coeff * (mv.value - self.problem.incumbent_assignment[var_idx]);
        for cell in &self.problem.vars[var_idx].coeffs {
            let constraint = &self.problem.constraints[cell.idx];
            let candidate_lhs = constraint.incumbent_lhs + cell.coeff * (mv.value - self.problem.incumbent_assignment[var_idx]);
            mv.score += constraint.weight * (constraint.score(candidate_lhs) - constraint.score(constraint.incumbent_lhs));
        }
        self.update_good_moves(var_idx);
    }

    fn user_terminate(&mut self, callback: &mut impl FnMut(FjStatus) -> bool, with_solution: bool) -> bool {
        const CALLBACK_EFFORT: usize = 500000;
        if with_solution || self.total_effort - self.effort_at_last_callback > CALLBACK_EFFORT {
            self.effort_at_last_callback = self.total_effort;
            let status = FjStatus {
                total_effort: self.total_effort,
                effort_since_last_improvement: self.total_effort - self.effort_at_last_improvement,
                solution_objective_value: self.problem.incumbent_objective,
                solution: with_solution.then_some(&self.problem.incumbent_assignment[..]),
            };
            return !callback(status);
        }
        false
    }
}

/// HiGHS's `HighsMipSolverData::feasibilityJump`: feasibility jump on the
/// presolved model from the incumbent (or each column's finite bound), its
/// solution tried as an incumbent. Returns whether a column's bounds show
/// the model infeasible.
pub(crate) fn run(data: &mut super::data::MipData) -> bool {
    let model = &data.model;
    // The presolved model is minimised.
    let sense_multiplier = 1.0;
    let use_incumbent = !data.incumbent.is_empty();
    let mut solver = FeasibilityJumpSolver::new(data.options.random_seed, data.epsilon, data.feastol);
    let mut col_value = vec![0.0; model.num_col];
    for col in 0..model.num_col {
        let mut lower = model.col_lower[col];
        let mut upper = model.col_upper[col];
        let fj_var_type = if model.integrality[col] == super::VarType::Continuous {
            VarType::Continuous
        } else {
            lower = (lower - data.feastol).ceil();
            upper = (upper + data.feastol).floor();
            VarType::Integer
        };
        let legal_bounds = lower <= upper && lower < f64::INFINITY && upper > -f64::INFINITY && !lower.is_nan() && !upper.is_nan();
        if !legal_bounds {
            return true;
        }
        solver.add_var(fj_var_type, lower, upper, sense_multiplier * model.col_cost[col]);
        let initial_assignment = if use_incumbent && data.incumbent[col].is_finite() {
            super::cmax(lower, super::cmin(upper, data.incumbent[col]))
        } else if lower.is_finite() {
            lower
        } else if upper.is_finite() {
            upper
        } else {
            0.0
        };
        col_value[col] = initial_assignment;
    }
    for row in 0..model.num_row {
        let has_finite_lower = model.row_lower[row].is_finite();
        let has_finite_upper = model.row_upper[row].is_finite();
        if has_finite_lower || has_finite_upper {
            let (index, value) = model.row(row);
            if has_finite_lower {
                solver.add_constraint(RowType::Gte, model.row_lower[row], index, value);
            }
            if has_finite_upper {
                solver.add_constraint(RowType::Lte, model.row_upper[row], index, value);
            }
        }
    }
    let nnz = model.ar_index.len();
    let max_total_effort = nnz << 10;
    let max_effort_since_last_improvement = nnz << 8;
    let mut found_integer_feasible_solution = false;
    let initial = col_value.clone();
    solver.solve(&initial, |status| {
        if let Some(solution) = status.solution {
            found_integer_feasible_solution = true;
            col_value = solution.to_vec();
        }
        !(status.effort_since_last_improvement > max_effort_since_last_improvement || status.total_effort > max_total_effort)
    });
    if found_integer_feasible_solution {
        data.try_solution(&col_value, super::data::SolutionSource::FeasibilityJump);
    }
    false
}
