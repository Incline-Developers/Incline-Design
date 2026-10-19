//! HiGHS's `Highs` object, for linear programs: an LP that is changed and
//! solved again, keeping what the last solve learnt.
//!
//! As in HiGHS, changing column bounds or costs keeps the simplex solver's
//! basis, factor and edge weights, so the next solve starts where the last
//! one ended. Adding or deleting rows keeps only the basis (new rows' slacks
//! are basic): the simplex solver is rebuilt from it. Scale factors are
//! chosen once, at the first solve; rows added later are scaled by their
//! largest entry. A solve with presolve on and no useful basis runs HiGHS's
//! presolve, reduced solve and postsolve, then the final simplex solve from
//! the postsolved basis, whose state is kept like any other.

use crate::{
    Basis, BasisStatus, Options, Problem, Sense,
    factor::Factor,
    hvector::HVector,
    matrix::ColWise,
    presolve::{self, Presolved},
    simplex::{
        self, Infeasibilities, ModelStatus, SimplexResult,
        ekk::{Ekk, Interrupt, SimplexLp},
        nla::Scale,
        options::{SimplexStrategy, Solver},
    },
};

/// No index (HiGHS's -1).
pub const NONE: usize = usize::MAX;

/// An LP to solve, change and solve again; see the module docs.
pub struct LpSolver {
    pub options: Options,
    /// The incumbent LP, unscaled, column-wise.
    lp: SimplexLp,
    /// Whether scaling has been decided (HiGHS's `scale_.strategy` set).
    scale_tried: bool,
    scale: Option<Scale>,
    basis: Basis,
    /// The basis has one basic variable per row and matches the LP.
    basis_valid: bool,
    /// The basis is worth starting from, even if not valid (after deleting
    /// rows whose slacks were nonbasic).
    basis_useful: bool,
    ekk: Option<Ekk>,
    col_value: Vec<f64>,
    col_dual: Vec<f64>,
    row_value: Vec<f64>,
    row_dual: Vec<f64>,
    value_valid: bool,
    dual_valid: bool,
    /// The KKT check found the solution primal feasible.
    primal_feasible: bool,
    status: ModelStatus,
    objective: f64,
    iterations: usize,
    ipm_iterations: usize,
    crossover_iterations: usize,
    infeasibilities: Option<Infeasibilities>,
    interrupt: Option<Interrupt>,
    started: Option<web_time::Instant>,
    run_time: f64,
    /// HiGHS's `simplex_iterate_`, which outlives the simplex solver.
    iterate: Option<Iterate>,
}

/// HiGHS's `SimplexIterate`: a simplex basis, its factor and its dual edge
/// weights (empty when there were none).
#[derive(Clone, Debug)]
struct Iterate {
    basis: simplex::ekk::Basis,
    invert: crate::factor::Invert,
    dual_edge_weight: Vec<f64>,
}

impl LpSolver {
    /// HiGHS's `passModel`: `problem`'s LP (integrality is ignored), with
    /// bounds beyond `options.infinite_bound` infinite and entries no
    /// bigger than `options.small_matrix_value` dropped.
    pub fn new(problem: &Problem, options: Options) -> Self {
        let num_col = problem.num_cols();
        let num_row = problem.num_rows();
        let small = options.small_matrix_value;
        let mut start = vec![0usize; num_col + 1];
        for (&col, &value) in problem.row_index.iter().zip(&problem.row_value) {
            if value.abs() > small {
                start[col + 1] += 1;
            }
        }
        for col in 0..num_col {
            start[col + 1] += start[col];
        }
        let num_nz = start[num_col];
        let mut next = start.clone();
        let mut index = vec![0usize; num_nz];
        let mut value = vec![0.0; num_nz];
        for row in 0..num_row {
            for (col, entry) in problem.row(row) {
                if entry.abs() > small {
                    let at = next[col];
                    index[at] = row;
                    value[at] = entry;
                    next[col] += 1;
                }
            }
        }
        Self::from_col_wise(
            if problem.sense == Sense::Maximise { -1.0 } else { 1.0 },
            problem.offset,
            problem.col_cost.clone(),
            problem.col_lower.clone(),
            problem.col_upper.clone(),
            problem.row_lower.clone(),
            problem.row_upper.clone(),
            start,
            index,
            value,
            options,
        )
    }

    /// [`LpSolver::new`] for a column-wise LP (`sense` 1 to minimise, -1 to
    /// maximise), keeping each column's entries in the order given.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_col_wise(
        sense: f64,
        offset: f64,
        col_cost: Vec<f64>,
        col_lower: Vec<f64>,
        col_upper: Vec<f64>,
        row_lower: Vec<f64>,
        row_upper: Vec<f64>,
        start: Vec<usize>,
        index: Vec<usize>,
        value: Vec<f64>,
        options: Options,
    ) -> Self {
        let infinite = |value: f64| value.abs() >= options.infinite_bound;
        let bound = |value: f64| if infinite(value) { value.signum() * f64::INFINITY } else { value };
        let lp = simplex::simplex_lp(
            sense,
            offset,
            col_cost,
            col_lower.into_iter().map(bound).collect(),
            col_upper.into_iter().map(bound).collect(),
            row_lower.into_iter().map(bound).collect(),
            row_upper.into_iter().map(bound).collect(),
            start,
            index,
            value,
        );
        Self {
            options,
            lp,
            scale_tried: false,
            scale: None,
            basis: Basis {
                col_status: Vec::new(),
                row_status: Vec::new(),
            },
            basis_valid: false,
            basis_useful: false,
            ekk: None,
            col_value: Vec::new(),
            col_dual: Vec::new(),
            row_value: Vec::new(),
            row_dual: Vec::new(),
            value_valid: false,
            dual_valid: false,
            primal_feasible: false,
            status: ModelStatus::NotSet,
            objective: 0.0,
            iterations: 0,
            ipm_iterations: 0,
            crossover_iterations: 0,
            infeasibilities: None,
            interrupt: None,
            started: None,
            run_time: 0.0,
            iterate: None,
        }
    }

    /// Stop solves with [`ModelStatus::Interrupt`] once `interrupt` says so.
    pub fn set_interrupt(&mut self, interrupt: Option<Interrupt>) {
        self.interrupt = interrupt;
    }

    pub fn num_cols(&self) -> usize {
        self.lp.num_col
    }

    pub fn num_rows(&self) -> usize {
        self.lp.num_row
    }

    /// The LP's sense: 1 to minimise, -1 to maximise.
    pub fn sense(&self) -> f64 {
        self.lp.sense
    }

    pub fn offset(&self) -> f64 {
        self.lp.offset
    }

    pub fn col_cost(&self) -> &[f64] {
        &self.lp.col_cost
    }

    pub fn col_lower(&self) -> &[f64] {
        &self.lp.col_lower
    }

    pub fn col_upper(&self) -> &[f64] {
        &self.lp.col_upper
    }

    pub fn row_lower(&self) -> &[f64] {
        &self.lp.row_lower
    }

    pub fn row_upper(&self) -> &[f64] {
        &self.lp.row_upper
    }

    /// The matrix column-wise: column `j`'s rows and values are at
    /// `start[j]..start[j + 1]` of `index` and `value`.
    pub fn matrix(&self) -> (&[usize], &[usize], &[f64]) {
        (&self.lp.a.start, &self.lp.a.index, &self.lp.a.value)
    }

    /// HiGHS's `invalidateModelStatusSolutionAndInfo`.
    fn invalidate_solution(&mut self) {
        self.status = ModelStatus::NotSet;
        self.value_valid = false;
        self.dual_valid = false;
        self.infeasibilities = None;
    }

    /// HiGHS's `invalidateBasis`.
    fn invalidate_basis(&mut self) {
        self.basis_valid = false;
        self.basis_useful = false;
    }

    /// HiGHS's `clearSolver`: forget the solution, the basis and the
    /// simplex solver's state.
    pub fn clear_solver(&mut self) {
        self.invalidate_solution();
        self.invalidate_basis();
        self.ekk = None;
    }

    /// HiGHS's `changeColsBounds`, for each `(column, lower, upper)`.
    pub fn change_col_bounds(&mut self, changes: impl IntoIterator<Item = (usize, f64, f64)>) {
        let infinite_bound = self.options.infinite_bound;
        let bound = |value: f64| if value.abs() >= infinite_bound { value.signum() * f64::INFINITY } else { value };
        let mut changed = false;
        for (col, lower, upper) in changes {
            changed = true;
            self.lp.col_lower[col] = bound(lower);
            self.lp.col_upper[col] = bound(upper);
            self.set_nonbasic_status(col);
        }
        if !changed {
            return;
        }
        // HiGHS keeps a solution that the new bounds still allow when there
        // is no basis; it never has one here without a basis.
        self.invalidate_solution();
        if let Some(ekk) = &mut self.ekk {
            // `LpAction::kNewBounds`.
            ekk.status.has_fresh_rebuild = false;
            ekk.status.has_dual_objective_value = false;
            ekk.status.has_primal_objective_value = false;
        }
    }

    /// HiGHS's `setNonbasicStatusInterface` for one column: a nonbasic
    /// column whose bounds changed sits at the bound its status names, or
    /// the one that remains.
    fn set_nonbasic_status(&mut self, col: usize) {
        if !self.basis_valid || self.basis.col_status[col] == BasisStatus::Basic {
            return;
        }
        let lower = self.lp.col_lower[col];
        let upper = self.lp.col_upper[col];
        let mut status = self.basis.col_status[col];
        let movement: i8;
        if lower == upper {
            if status == BasisStatus::Nonbasic {
                status = BasisStatus::Lower;
            }
            movement = 0;
        } else if lower != f64::NEG_INFINITY {
            if upper != f64::INFINITY {
                if status == BasisStatus::Nonbasic {
                    if lower.abs() < upper.abs() {
                        status = BasisStatus::Lower;
                        movement = 1;
                    } else {
                        status = BasisStatus::Upper;
                        movement = -1;
                    }
                } else if status == BasisStatus::Lower {
                    movement = 1;
                } else {
                    movement = -1;
                }
            } else {
                status = BasisStatus::Lower;
                movement = 1;
            }
        } else if upper != f64::INFINITY {
            status = BasisStatus::Upper;
            movement = -1;
        } else {
            status = BasisStatus::Zero;
            movement = 0;
        }
        self.basis.col_status[col] = status;
        if let Some(ekk) = &mut self.ekk
            && ekk.status.has_basis
        {
            ekk.basis.nonbasic_flag[col] = 1;
            ekk.basis.nonbasic_move[col] = movement;
        }
    }

    /// HiGHS's `changeColsCost`, for each `(column, cost)`.
    pub fn change_col_costs(&mut self, changes: impl IntoIterator<Item = (usize, f64)>) {
        let mut changed = false;
        for (col, cost) in changes {
            changed = true;
            self.lp.col_cost[col] = cost;
        }
        if !changed {
            return;
        }
        self.invalidate_solution();
        if let Some(ekk) = &mut self.ekk {
            // `LpAction::kNewCosts`.
            ekk.status.has_fresh_rebuild = false;
            ekk.status.has_dual_objective_value = false;
            ekk.status.has_primal_objective_value = false;
        }
    }

    /// HiGHS's `addRows`: row `i` has bounds `lower[i]..upper[i]` and the
    /// entries `index[start[i]..start[i + 1]]`, `value[..]`; `start` has
    /// one entry per row, as HiGHS takes it.
    pub fn add_rows(&mut self, lower: &[f64], upper: &[f64], start: &[usize], index: &[usize], value: &[f64]) {
        let num_new_row = lower.len();
        if num_new_row == 0 {
            return;
        }
        let infinite_bound = self.options.infinite_bound;
        let bound = |value: f64| if value.abs() >= infinite_bound { value.signum() * f64::INFINITY } else { value };
        let small = self.options.small_matrix_value;
        let num_col = self.lp.num_col;
        let num_row = self.lp.num_row;
        let row_end = |row: usize| if row + 1 < num_new_row { start[row + 1] } else { index.len() };
        // The new rows, small values dropped as HiGHS's `assess` does.
        let mut rows: Vec<Vec<(usize, f64)>> = Vec::with_capacity(num_new_row);
        for row in 0..num_new_row {
            rows.push((start[row]..row_end(row)).filter(|&k| value[k].abs() > small).map(|k| (index[k], value[k])).collect());
        }
        // `HighsSparseMatrix::addRows` on a column-wise matrix: each
        // column's new entries follow its old ones, in row order.
        let a = &self.lp.a;
        let mut count = vec![0usize; num_col];
        for row in &rows {
            for &(col, _) in row {
                count[col] += 1;
            }
        }
        let mut new_start = vec![0usize; num_col + 1];
        for col in 0..num_col {
            new_start[col + 1] = new_start[col] + (a.start[col + 1] - a.start[col]) + count[col];
        }
        let num_nz = new_start[num_col];
        let mut new_index = vec![0usize; num_nz];
        let mut new_value = vec![0.0; num_nz];
        let mut next = vec![0usize; num_col];
        for col in 0..num_col {
            let old = a.start[col]..a.start[col + 1];
            let at = new_start[col];
            new_index[at..at + old.len()].copy_from_slice(&a.index[old.clone()]);
            new_value[at..at + old.len()].copy_from_slice(&a.value[old.clone()]);
            next[col] = at + old.len();
        }
        for (offset, row) in rows.iter().enumerate() {
            for &(col, entry) in row {
                new_index[next[col]] = num_row + offset;
                new_value[next[col]] = entry;
                next[col] += 1;
            }
        }
        let new_num_row = num_row + num_new_row;
        self.lp.a = ColWise {
            num_col,
            num_row: new_num_row,
            start: new_start,
            index: new_index,
            value: new_value,
        };
        self.lp.row_lower.extend(lower.iter().map(|&value| bound(value)));
        self.lp.row_upper.extend(upper.iter().map(|&value| bound(value)));
        self.lp.num_row = new_num_row;
        // New rows' scale: their largest column-scaled entry to about one
        // (`considerRowScaling`).
        if let Some(scale) = &mut self.scale {
            let max_allow_scale = 2f64.powi(self.options.allowed_matrix_scale_factor);
            let min_allow_scale = 1.0 / max_allow_scale;
            for row in &rows {
                let row_max_value = row.iter().fold(0.0f64, |max, &(col, entry)| max.max((entry * scale.col[col]).abs()));
                let row_scale = if row_max_value != 0.0 {
                    let value = 2f64.powf(((1.0 / row_max_value).ln() / 2f64.ln() + 0.5).floor());
                    value.max(min_allow_scale).min(max_allow_scale)
                } else {
                    1.0
                };
                scale.row.push(row_scale);
            }
        }
        if self.basis_useful {
            self.basis.row_status.extend(std::iter::repeat_n(BasisStatus::Basic, num_new_row));
        }
        self.invalidate_solution();
        // `LpAction::kNewRows`: HiGHS clears its simplex solver.
        self.ekk = None;
    }

    /// HiGHS's `deleteRows` by mask: deletes each row `r` with
    /// `delete[r]`, returning each row's new index ([`NONE`] if deleted).
    pub fn delete_rows(&mut self, delete: &[bool]) -> Vec<usize> {
        let num_row = self.lp.num_row;
        let mut new_index = vec![NONE; num_row];
        let mut kept = 0;
        for row in 0..num_row {
            if !delete[row] {
                new_index[row] = kept;
                kept += 1;
            }
        }
        if kept == num_row {
            return new_index;
        }
        let a = &mut self.lp.a;
        let mut at = 0;
        let mut col_start = 0;
        for col in 0..a.num_col {
            let end = a.start[col + 1];
            for k in col_start..end {
                let row = a.index[k];
                if new_index[row] != NONE {
                    a.index[at] = new_index[row];
                    a.value[at] = a.value[k];
                    at += 1;
                }
            }
            col_start = end;
            a.start[col + 1] = at;
        }
        a.index.truncate(at);
        a.value.truncate(at);
        a.num_row = kept;
        let keep = |values: &mut Vec<f64>| {
            let mut row = 0;
            values.retain(|_| {
                row += 1;
                !delete[row - 1]
            });
        };
        keep(&mut self.lp.row_lower);
        keep(&mut self.lp.row_upper);
        if let Some(scale) = &mut self.scale {
            keep(&mut scale.row);
        }
        self.lp.num_row = kept;
        self.status = ModelStatus::NotSet;
        if self.basis_useful {
            let mut deleted_nonbasic = false;
            let mut row = 0;
            self.basis.row_status.retain(|status| {
                row += 1;
                if delete[row - 1] {
                    deleted_nonbasic |= *status != BasisStatus::Basic;
                    false
                } else {
                    true
                }
            });
            if deleted_nonbasic {
                self.basis_valid = false;
            }
        }
        self.invalidate_solution();
        // `LpAction::kDelRows`.
        self.ekk = None;
        new_index
    }

    /// HiGHS's `setBasis`: start the next solve from `basis`, which must
    /// have one basic variable per row. Returns whether it was taken.
    pub fn set_basis(&mut self, basis: &Basis) -> bool {
        if basis.col_status.len() != self.lp.num_col || basis.row_status.len() != self.lp.num_row {
            return false;
        }
        let num_basic = basis.col_status.iter().chain(&basis.row_status).filter(|status| **status == BasisStatus::Basic).count();
        if num_basic != self.lp.num_row {
            return false;
        }
        self.basis = basis.clone();
        self.basis_valid = true;
        self.basis_useful = true;
        // `LpAction::kNewBasis`: the simplex solver's basis and everything
        // derived from it go.
        if let Some(ekk) = &mut self.ekk {
            ekk.status.has_basis = false;
            ekk.status.has_ar_matrix = false;
            ekk.status.has_dual_steepest_edge_weights = false;
            ekk.status.has_invert = false;
            ekk.status.has_fresh_invert = false;
            ekk.status.has_fresh_rebuild = false;
            ekk.status.has_dual_objective_value = false;
            ekk.status.has_primal_objective_value = false;
            ekk.dual_ray_record = None;
            ekk.primal_ray_record = None;
        }
        true
    }

    /// HiGHS's `setBasis` with an alien basis (`basis.alien`): one that may
    /// have the wrong number of basic variables or be singular, made a
    /// basis when the simplex method next starts. Without rows, basic
    /// columns become nonbasic.
    pub fn set_alien_basis(&mut self, basis: &Basis) -> bool {
        if self.lp.num_row == 0 {
            self.basis.col_status = basis
                .col_status
                .iter()
                .map(|&status| if status == BasisStatus::Basic { BasisStatus::Nonbasic } else { status })
                .collect();
            self.basis_valid = true;
            self.basis_useful = true;
            return true;
        }
        if basis.col_status.len() != self.lp.num_col || basis.row_status.len() != self.lp.num_row {
            return false;
        }
        self.basis = basis.clone();
        self.basis_valid = false;
        self.basis_useful = true;
        if let Some(ekk) = &mut self.ekk {
            ekk.status.has_basis = false;
            ekk.status.has_invert = false;
            ekk.status.has_fresh_invert = false;
            ekk.status.has_fresh_rebuild = false;
        }
        true
    }

    /// The basis, when it is valid.
    pub fn basis(&self) -> Option<&Basis> {
        self.basis_valid.then_some(&self.basis)
    }

    pub fn status(&self) -> ModelStatus {
        self.status
    }

    /// The objective of the last solve, in the LP's own sense, offset
    /// included.
    pub fn objective(&self) -> f64 {
        self.objective
    }

    /// Simplex iterations of the last solve.
    pub fn iterations(&self) -> usize {
        self.iterations
    }

    pub fn ipm_iterations(&self) -> usize {
        self.ipm_iterations
    }

    pub fn crossover_iterations(&self) -> usize {
        self.crossover_iterations
    }

    /// The last solve's infeasibilities in the unscaled LP, if known.
    pub fn infeasibilities(&self) -> Option<Infeasibilities> {
        self.infeasibilities
    }

    /// Seconds the last solve took, or the one running has taken so far.
    pub fn run_time(&self) -> f64 {
        self.started.map_or(self.run_time, |started| started.elapsed().as_secs_f64())
    }

    /// The primal solution, when there is one.
    pub fn col_value(&self) -> Option<&[f64]> {
        self.value_valid.then_some(&self.col_value[..])
    }

    pub fn row_value(&self) -> Option<&[f64]> {
        self.value_valid.then_some(&self.row_value[..])
    }

    /// The dual solution, when there is one.
    pub fn col_dual(&self) -> Option<&[f64]> {
        self.dual_valid.then_some(&self.col_dual[..])
    }

    pub fn row_dual(&self) -> Option<&[f64]> {
        self.dual_valid.then_some(&self.row_dual[..])
    }

    /// A new solver for this LP (HiGHS's `passModel(getLp())`), with
    /// `options` and nothing else of this one.
    pub fn copy_model(&self, options: Options) -> Self {
        let lp = &self.lp;
        Self::from_col_wise(
            lp.sense,
            lp.offset,
            lp.col_cost.clone(),
            lp.col_lower.clone(),
            lp.col_upper.clone(),
            lp.row_lower.clone(),
            lp.row_upper.clone(),
            lp.a.start.clone(),
            lp.a.index.clone(),
            lp.a.value.clone(),
            options,
        )
    }

    /// The column values and duals to change in place, as HiGHS's MIP
    /// solver does through `const_cast`.
    pub fn col_value_and_dual_mut(&mut self) -> (&mut [f64], &mut [f64]) {
        (&mut self.col_value, &mut self.col_dual)
    }

    /// HiGHS's `hasInvert`.
    pub fn has_invert(&self) -> bool {
        self.ekk.as_ref().is_some_and(|ekk| ekk.status.has_invert)
    }

    /// HiGHS's `getBasicVariablesArray`: the variable basic in each row,
    /// rows' slacks being `num_cols + row`.
    pub fn basic_variables(&self) -> &[usize] {
        &self.ekk.as_ref().expect("a factored basis").basis.basic_index
    }

    /// HiGHS's `getDualEdgeWeights`, by row.
    pub fn dual_edge_weights(&self) -> Option<&[f64]> {
        let ekk = self.ekk.as_ref()?;
        ekk.status.has_dual_steepest_edge_weights.then_some(&ekk.dual_edge_weight[..])
    }

    /// HiGHS's `getBasisInverseRowSparse`: row `row` of the basis inverse,
    /// in `row_ep`.
    pub(crate) fn basis_inverse_row(&self, row: usize, row_ep: &mut HVector) {
        let ekk = self.ekk.as_ref().expect("a factored basis");
        row_ep.clear();
        row_ep.count = 1;
        row_ep.index[0] = row;
        row_ep.array[row] = 1.0;
        row_ep.pack_flag = true;
        ekk.nla.btran(row_ep, ekk.info.row_ep_density, &ekk.basis.basic_index);
    }

    /// HiGHS's `getDualRaySparse`: the dual ray of the last infeasible
    /// solve, in `row_ep`, if it recorded one.
    pub(crate) fn dual_ray(&self, row_ep: &mut HVector) -> bool {
        let Some(ekk) = self.ekk.as_ref() else { return false };
        let Some((row, sign)) = ekk.dual_ray_record else { return false };
        row_ep.clear();
        row_ep.count = 1;
        row_ep.pack_flag = true;
        row_ep.index[0] = row;
        row_ep.array[row] = sign as f64;
        ekk.nla.btran(row_ep, ekk.info.row_ep_density, &ekk.basis.basic_index);
        true
    }

    /// HiGHS's `putIterate`: save the simplex basis, factor and dual edge
    /// weights for [`LpSolver::get_iterate`]. False without a factor.
    pub fn put_iterate(&mut self) -> bool {
        let Some(ekk) = self.ekk.as_ref().filter(|ekk| ekk.status.has_invert) else {
            return false;
        };
        self.iterate = Some(Iterate {
            basis: ekk.basis.clone(),
            invert: ekk.nla.factor.get_invert(),
            dual_edge_weight: if ekk.status.has_dual_steepest_edge_weights {
                ekk.dual_edge_weight.clone()
            } else {
                Vec::new()
            },
        });
        true
    }

    /// HiGHS's `getIterate`: restore what [`LpSolver::put_iterate`] saved,
    /// and the basis with it. False if there is nothing to restore into.
    pub fn get_iterate(&mut self) -> bool {
        let Some(ekk) = self.ekk.as_mut().filter(|ekk| ekk.status.initialised_for_new_lp) else {
            return false;
        };
        let Some(iterate) = self.iterate.as_ref() else { return false };
        ekk.nla.factor.set_invert(&iterate.invert);
        ekk.basis = iterate.basis.clone();
        if iterate.dual_edge_weight.is_empty() {
            ekk.status.has_dual_steepest_edge_weights = false;
        } else {
            ekk.dual_edge_weight.clone_from(&iterate.dual_edge_weight);
        }
        ekk.status.has_invert = true;
        self.basis = ekk.get_basis();
        self.basis_valid = true;
        self.basis_useful = true;
        self.invalidate_solution();
        true
    }

    /// HiGHS's `optimizeModel` for an LP.
    pub fn run(&mut self) -> ModelStatus {
        let started = web_time::Instant::now();
        self.started = Some(started);
        self.iterations = 0;
        self.ipm_iterations = 0;
        self.crossover_iterations = 0;
        self.invalidate_solution();
        self.run_lp();
        self.kkt_check();
        self.run_time = started.elapsed().as_secs_f64();
        self.started = None;
        if matches!(self.status, ModelStatus::NotSet | ModelStatus::SolveError) {
            self.invalidate_solution();
            self.invalidate_basis();
            self.status = ModelStatus::SolveError;
            self.iterations = 0;
        }
        self.status
    }

    /// HiGHS's `callLpKktCheck` at the end of `optimizeModel`: the
    /// objective, infeasibilities and status from the solution itself.
    fn kkt_check(&mut self) {
        if !self.value_valid || matches!(self.status, ModelStatus::NotSet | ModelStatus::SolveError) {
            return;
        }
        let lp = &self.lp;
        let kkt_lp = crate::kkt::KktLp {
            sense: lp.sense,
            offset: lp.offset,
            col_cost: &lp.col_cost,
            col_lower: &lp.col_lower,
            col_upper: &lp.col_upper,
            row_lower: &lp.row_lower,
            row_upper: &lp.row_upper,
            start: &lp.a.start,
            index: &lp.a.index,
            value: &lp.a.value,
        };
        let solution = crate::kkt::KktSolution {
            col_value: &self.col_value,
            row_value: &self.row_value,
            col_dual: &self.col_dual,
            row_dual: &self.row_dual,
            dual_valid: self.dual_valid,
        };
        let tolerances = crate::kkt::KktTolerances::new(self.options.primal_feasibility_tolerance, self.options.dual_feasibility_tolerance);
        let mut status = self.status;
        let info = crate::kkt::lp_kkt_check(&kkt_lp, &solution, self.basis_valid, tolerances, &mut status);
        self.status = status;
        self.objective = info.objective;
        let count = |n: i64| usize::try_from(n).ok();
        self.infeasibilities = Some(Infeasibilities {
            num_primal: count(info.num_primal_infeasibilities),
            max_primal: info.max_primal_infeasibility,
            sum_primal: info.sum_primal_infeasibilities,
            num_dual: count(info.num_dual_infeasibilities),
            max_dual: info.max_dual_infeasibility,
            sum_dual: info.sum_dual_infeasibilities,
        });
        self.primal_feasible = info.primal_feasible;
    }

    /// HiGHS's `primal_solution_status == kSolutionStatusFeasible` after the
    /// last run.
    pub fn primal_feasible(&self) -> bool {
        self.value_valid && self.primal_feasible
    }

    fn run_lp(&mut self) {
        let num_col = self.lp.num_col;
        let num_row = self.lp.num_row;
        if num_col == 0 {
            // Every row's activity is zero: a row that cannot hold it makes
            // the LP infeasible, as `solve_mip` says of the same problem.
            let tolerance = self.options.primal_feasibility_tolerance;
            if (0..num_row).any(|row| self.lp.row_lower[row] > tolerance || self.lp.row_upper[row] < -tolerance) {
                self.status = ModelStatus::Infeasible;
                self.invalidate_basis();
                self.value_valid = false;
                self.dual_valid = false;
                self.infeasibilities = None;
                return;
            }
            self.status = ModelStatus::Optimal;
            self.objective = self.lp.offset;
            self.col_value.clear();
            self.col_dual.clear();
            self.row_value = vec![0.0; num_row];
            self.row_dual = vec![0.0; num_row];
            self.value_valid = true;
            self.dual_valid = true;
            return;
        }
        // NaNs make HiGHS cycle; refuse them.
        let has_nan = [
            &self.lp.col_cost,
            &self.lp.col_lower,
            &self.lp.col_upper,
            &self.lp.row_lower,
            &self.lp.row_upper,
            &self.lp.a.value,
        ]
        .iter()
        .any(|values| values.iter().any(|value| value.is_nan()));
        if has_nan || !self.lp.offset.is_finite() {
            self.status = ModelStatus::SolveError;
            return;
        }
        if !self.infeasible_bounds_ok() {
            self.status = ModelStatus::Infeasible;
            self.invalidate_basis();
            self.value_valid = false;
            self.dual_valid = false;
            self.infeasibilities = None;
            return;
        }
        let solver_will_use_basis = self.options.solver == Solver::Simplex;
        if !solver_will_use_basis {
            self.invalidate_basis();
        }
        let unconstrained = self.lp.a.value.is_empty();
        let has_basis = self.basis_useful;
        if (unconstrained || has_basis || !self.options.presolve) && solver_will_use_basis {
            if self.basis_useful {
                self.refine_basis_from_solution();
            }
            let options = self.options.clone();
            self.solve_lp(&options);
            return;
        }
        let presolved = presolve::solve(&self.lp, &self.options, self.interrupt.clone());
        match presolved {
            Presolved::NotReduced => {
                let options = self.options.clone();
                self.solve_lp(&options);
            }
            Presolved::Done(result) => {
                self.ekk = None;
                self.iterations += result.iterations;
                self.ipm_iterations += result.ipm_iterations;
                self.crossover_iterations += result.crossover_iterations;
                self.status = result.status;
                self.invalidate_basis();
                self.infeasibilities = None;
            }
            Presolved::UnboundedOrInfeasible => {
                let options = Options {
                    solver: Solver::Simplex,
                    simplex_strategy: SimplexStrategy::Primal,
                    ..self.options.clone()
                };
                self.solve_lp(&options);
            }
            Presolved::Postsolved {
                basis,
                iterations,
                ipm_iterations,
                factor_pivot_threshold,
            } => {
                self.iterations += iterations;
                self.ipm_iterations += ipm_iterations.0;
                self.crossover_iterations += ipm_iterations.1;
                self.basis = basis;
                self.basis_valid = true;
                self.basis_useful = true;
                let mut options = Options {
                    solver: Solver::Simplex,
                    simplex_strategy: SimplexStrategy::Choose,
                    ..self.options.clone()
                };
                if let Some(threshold) = factor_pivot_threshold {
                    options.factor_pivot_threshold = threshold;
                }
                // `ekk_instance_.invalidate()`: the final solve starts afresh
                // from the postsolved basis.
                self.ekk = None;
                self.solve_lp(&options);
            }
        }
    }

    /// HiGHS's `infeasibleBoundsOk`: bounds crossed by less than the
    /// primal feasibility tolerance are made to meet; crossed by more, the
    /// LP is infeasible.
    fn infeasible_bounds_ok(&mut self) -> bool {
        let tolerance = self.options.primal_feasibility_tolerance;
        let mut ok = true;
        let mut repair = |lower: &mut f64, upper: &mut f64| {
            if *lower <= *upper {
                return;
            }
            let range = *upper - *lower;
            if range > -tolerance {
                let integer_lower = *lower == (*lower + 0.5).floor();
                let integer_upper = *upper == (*upper + 0.5).floor();
                if integer_lower {
                    *upper = *lower;
                } else if integer_upper {
                    *lower = *upper;
                } else {
                    let mid = 0.5 * (*lower + *upper);
                    *lower = mid;
                    *upper = mid;
                }
            } else {
                ok = false;
            }
        };
        for col in 0..self.lp.num_col {
            repair(&mut self.lp.col_lower[col], &mut self.lp.col_upper[col]);
        }
        for row in 0..self.lp.num_row {
            repair(&mut self.lp.row_lower[row], &mut self.lp.row_upper[row]);
        }
        ok
    }

    /// HiGHS's `refineBasis`: a nonbasic status that names no bound gets
    /// the bound nearest the solution, or the only finite one.
    fn refine_basis_from_solution(&mut self) {
        let have_values = self.value_valid && self.col_value.len() == self.lp.num_col && self.row_value.len() == self.lp.num_row;
        let refine = |lower: f64, upper: f64, value: Option<f64>| {
            if lower == upper {
                BasisStatus::Lower
            } else if lower != f64::NEG_INFINITY {
                if upper != f64::INFINITY {
                    match value {
                        Some(value) if value >= 0.5 * (lower + upper) => BasisStatus::Upper,
                        Some(_) => BasisStatus::Lower,
                        None if lower.abs() < upper.abs() => BasisStatus::Lower,
                        None => BasisStatus::Upper,
                    }
                } else {
                    BasisStatus::Lower
                }
            } else if upper != f64::INFINITY {
                BasisStatus::Upper
            } else {
                BasisStatus::Zero
            }
        };
        for col in 0..self.lp.num_col {
            if self.basis.col_status[col] == BasisStatus::Nonbasic {
                let value = have_values.then(|| self.col_value[col]);
                self.basis.col_status[col] = refine(self.lp.col_lower[col], self.lp.col_upper[col], value);
            }
        }
        for row in 0..self.lp.num_row {
            if self.basis.row_status[row] == BasisStatus::Nonbasic {
                let value = have_values.then(|| self.row_value[row]);
                self.basis.row_status[row] = refine(self.lp.row_lower[row], self.lp.row_upper[row], value);
            }
        }
    }

    /// HiGHS's `considerScaling`: decide the scale factors once.
    fn consider_scaling(&mut self, options: &Options) {
        if !options.scale || self.lp.num_col == 0 {
            if self.scale.is_some() {
                self.scale = None;
                self.scale_tried = false;
            }
            return;
        }
        if !self.scale_tried {
            self.scale = simplex::scale_factors(options, &self.lp);
            self.scale_tried = true;
        }
    }

    /// HiGHS's `solveLp` on the incumbent LP.
    fn solve_lp(&mut self, options: &Options) {
        if self.lp.num_row == 0 || self.lp.a.value.is_empty() {
            self.solve_unconstrained();
            return;
        }
        match options.solver {
            Solver::Simplex => self.solve_lp_simplex(options),
            Solver::Ipm => {
                self.ekk = None;
                let result = crate::ipx::solve_lp(&self.lp, options, self.interrupt.clone());
                let has_basis = result.status == ModelStatus::Optimal;
                self.take_result(result, has_basis);
            }
        }
    }

    /// HiGHS's `solveLpSimplex`.
    fn solve_lp_simplex(&mut self, options: &Options) {
        self.consider_scaling(options);
        let has_simplex_basis = self.ekk.as_ref().is_some_and(|ekk| ekk.status.has_basis);
        if !has_simplex_basis && !self.basis_valid && self.basis_useful {
            // `formSimplexLpBasisAndFactor`, for a basis with the wrong
            // number of basic variables.
            if !self.accommodate_alien_basis() {
                self.status = ModelStatus::SolveError;
                return;
            }
            self.refine_basis_from_solution();
            self.basis_valid = true;
        }
        let basis = (!has_simplex_basis && self.basis_valid).then(|| self.basis.clone());
        let result = simplex::solve_lp_simplex_in(&mut self.ekk, &self.lp, self.scale.as_ref(), options, basis.as_ref(), self.interrupt.clone());
        if matches!(result.status, ModelStatus::NotSet | ModelStatus::SolveError) {
            // The simplex solver failed (HiGHS's `HighsStatus::kError`):
            // HiGHS clears it, and keeps no solution, basis or counts.
            self.ekk = None;
            self.status = ModelStatus::SolveError;
            return;
        }
        self.take_result(result, true);
    }

    fn take_result(&mut self, result: SimplexResult, has_basis: bool) {
        self.status = result.status;
        self.objective = result.objective;
        self.iterations += result.iterations;
        self.ipm_iterations += result.ipm_iterations;
        self.crossover_iterations += result.crossover_iterations;
        self.infeasibilities = Some(result.infeasibilities);
        self.col_value = result.col_value;
        self.col_dual = result.col_dual;
        self.row_value = result.row_value;
        self.row_dual = result.row_dual;
        self.value_valid = true;
        self.dual_valid = true;
        if has_basis {
            self.basis = result.basis;
            self.basis_valid = true;
            self.basis_useful = true;
        } else {
            self.invalidate_basis();
        }
    }

    /// HiGHS's `accommodateAlienBasis`: keep the basic variables a
    /// rectangular factor finds independent, and make up the rest with
    /// the slacks of rows nothing pivoted on. Returns whether that gives one
    /// basic variable per row.
    ///
    /// With more basic variables than rows (after deleting rows whose slacks
    /// were nonbasic), HiGHS's factor names rows past the last for the
    /// surplus, and the basis it writes is not one; its solve then fails,
    /// as this one does.
    fn accommodate_alien_basis(&mut self) -> bool {
        let num_col = self.lp.num_col;
        let num_row = self.lp.num_row;
        let mut basic_index: Vec<usize> = (0..num_col).filter(|&col| self.basis.col_status[col] == BasisStatus::Basic).collect();
        basic_index.extend((0..num_row).filter(|&row| self.basis.row_status[row] == BasisStatus::Basic).map(|row| num_col + row));
        let num_basic = basic_index.len();
        let mut factor = Factor::default();
        factor.setup_general(num_col, num_row, num_basic, crate::factor::DEFAULT_PIVOT_THRESHOLD, crate::factor::DEFAULT_PIVOT_TOLERANCE);
        // HiGHS factors the LP as `considerScaling` leaves it: scaled.
        let scaled;
        let a = match &self.scale {
            Some(scale) => {
                let mut matrix = self.lp.a.clone();
                matrix.apply_scale(&scale.col, &scale.row);
                scaled = matrix;
                &scaled
            }
            None => &self.lp.a,
        };
        let rank_deficiency = factor.build(a.as_factor_matrix(), &mut basic_index);
        for status in self.basis.col_status.iter_mut().chain(self.basis.row_status.iter_mut()) {
            if *status == BasisStatus::Basic {
                *status = BasisStatus::Nonbasic;
            }
        }
        let use_basic = num_row.min(num_basic);
        let mut num_basic_variables = 0;
        for &var in &basic_index[..use_basic] {
            if var < num_col {
                self.basis.col_status[var] = BasisStatus::Basic;
            } else if var - num_col < num_row {
                self.basis.row_status[var - num_col] = BasisStatus::Basic;
            }
            num_basic_variables += 1;
        }
        let num_missing = num_row - num_basic_variables;
        for k in 0..num_missing {
            let row = factor.row_with_no_pivot[rank_deficiency + k];
            self.basis.row_status[row] = BasisStatus::Basic;
        }
        let num_basic = self
            .basis
            .col_status
            .iter()
            .chain(&self.basis.row_status)
            .filter(|status| **status == BasisStatus::Basic)
            .count();
        num_basic == num_row
    }

    /// An LP whose rows have no entries: each column sits at the bound its
    /// cost prefers, and each row must allow zero.
    fn solve_unconstrained(&mut self) {
        let problem = Problem {
            sense: if self.lp.sense < 0.0 { Sense::Maximise } else { Sense::Minimise },
            offset: self.lp.offset,
            col_cost: self.lp.col_cost.clone(),
            col_lower: self.lp.col_lower.clone(),
            col_upper: self.lp.col_upper.clone(),
            integrality: Vec::new(),
            row_lower: self.lp.row_lower.clone(),
            row_upper: self.lp.row_upper.clone(),
            row_start: vec![0; self.lp.num_row + 1],
            row_index: Vec::new(),
            row_value: Vec::new(),
        };
        let solution = crate::solve_unconstrained(
            &problem,
            &self.options,
            self.lp.sense,
            &self.lp.col_lower,
            &self.lp.col_upper,
            &self.lp.row_lower,
            &self.lp.row_upper,
        );
        self.ekk = None;
        self.status = solution.status;
        self.objective = solution.objective;
        self.infeasibilities = None;
        let basis = Basis {
            col_status: (0..self.lp.num_col)
                .map(|col| {
                    let (lower, upper, value) = (self.lp.col_lower[col], self.lp.col_upper[col], solution.col_value[col]);
                    if value == lower {
                        BasisStatus::Lower
                    } else if value == upper {
                        BasisStatus::Upper
                    } else {
                        BasisStatus::Zero
                    }
                })
                .collect(),
            row_status: vec![BasisStatus::Basic; self.lp.num_row],
        };
        self.col_value = solution.col_value;
        self.col_dual = solution.col_dual;
        self.row_value = solution.row_value;
        self.row_dual = solution.row_dual;
        self.value_valid = true;
        self.dual_valid = true;
        self.basis = basis;
        self.basis_valid = true;
        self.basis_useful = true;
    }
}
