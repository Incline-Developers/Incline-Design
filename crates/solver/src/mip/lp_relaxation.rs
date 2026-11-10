//! HiGHS's `HighsLpRelaxation`: the MIP's LP relaxation, its model rows
//! plus the cuts added from the cut pool, solved by the simplex solver
//! from the last basis, with what its solves prove: fractional integers,
//! dual proofs of infeasibility or of the objective bound, and the duals of
//! degenerate basic integers.

use std::rc::Rc;

use super::{
    BoundType, DomainChange, TINY,
    cut_pool::CutSet,
    data::{MipData, SolutionSource},
    domain::{Domain, DomainEnv, Reason},
    pseudocost::Pseudocost,
    sparse_vector_sum::SparseVectorSum,
};
use crate::{
    Basis, BasisStatus, Options,
    hvector::HVector,
    integers::fractionality,
    lp_solver::{LpSolver, NONE},
    simplex::{ModelStatus, options::Solver},
    util::CDouble,
};

const INF: f64 = f64::INFINITY;

/// HiGHS's `HighsLpRelaxation::Status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LpStatus {
    NotSet,
    Optimal,
    Infeasible,
    UnscaledDualFeasible,
    UnscaledPrimalFeasible,
    UnscaledInfeasible,
    Unbounded,
    Error,
}

impl LpStatus {
    pub fn scaled_optimal(self) -> bool {
        matches!(self, Self::Optimal | Self::UnscaledDualFeasible | Self::UnscaledPrimalFeasible | Self::UnscaledInfeasible)
    }

    pub fn unscaled_primal_feasible(self) -> bool {
        matches!(self, Self::Optimal | Self::UnscaledPrimalFeasible)
    }

    pub fn unscaled_dual_feasible(self) -> bool {
        matches!(self, Self::Optimal | Self::UnscaledDualFeasible)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    Model,
    CutPool,
}

/// HiGHS's `LpRow`: a row of the LP, a model row or a cut of the pool.
#[derive(Clone, Copy, Debug)]
struct LpRow {
    origin: Origin,
    index: usize,
    age: i32,
}

impl LpRow {
    fn cut(index: usize) -> Self {
        Self {
            origin: Origin::CutPool,
            index,
            age: 0,
        }
    }

    fn model(index: usize) -> Self {
        Self {
            origin: Origin::Model,
            index,
            age: 0,
        }
    }

    fn get<'d>(&self, data: &'d MipData) -> (&'d [usize], &'d [f64]) {
        match self.origin {
            Origin::CutPool => data.cutpool.cut(self.index),
            Origin::Model => data.model.row(self.index),
        }
    }

    fn row_len(&self, data: &MipData) -> usize {
        match self.origin {
            Origin::CutPool => data.cutpool.row_length(self.index),
            Origin::Model => data.model.ar_start[self.index + 1] - data.model.ar_start[self.index],
        }
    }

    fn is_integral(&self, data: &MipData) -> bool {
        match self.origin {
            Origin::CutPool => data.cutpool.cut_is_integral(self.index),
            Origin::Model => data.rowintegral[self.index],
        }
    }

    fn max_abs_val(&self, data: &MipData) -> f64 {
        match self.origin {
            Origin::CutPool => data.cutpool.max_abs_cut_coef(self.index),
            Origin::Model => data.max_abs_row_coef[self.index],
        }
    }
}

/// Which domain [`LpRelaxation::resolve_lp`] fixes substituted columns in.
pub(crate) enum LpDomain<'d> {
    None,
    Global,
    Local(&'d mut Domain),
}

pub(crate) struct LpRelaxation {
    lpsolver: LpSolver,
    lprows: Vec<LpRow>,
    fractionalints: Vec<(usize, f64)>,
    dualproofvals: Vec<f64>,
    dualproofinds: Vec<usize>,
    col_lb_buffer: Vec<f64>,
    col_ub_buffer: Vec<f64>,
    row_ep: HVector,
    row_ap: SparseVectorSum,
    dualproofrhs: f64,
    hasdualproof: bool,
    objective: f64,
    basischeckpoint: Option<Rc<Basis>>,
    currentbasisstored: bool,
    numlpiters: i64,
    last_age_call: i64,
    avg_solve_iters: f64,
    num_solved: i64,
    epochs: usize,
    max_num_fractional: usize,
    status: LpStatus,
    adjust_sym_branching_col: bool,
    solved_first_lp: bool,
}

fn empty_lp(options: Options) -> LpSolver {
    LpSolver::from_col_wise(
        1.0,
        0.0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![0],
        Vec::new(),
        Vec::new(),
        options,
    )
}

impl LpRelaxation {
    /// HiGHS's constructor.
    pub fn new(data: &MipData) -> Self {
        let options = Options {
            random_seed: data.options.random_seed,
            primal_feasibility_tolerance: data.options.mip_feasibility_tolerance,
            dual_feasibility_tolerance: data.options.mip_feasibility_tolerance * 0.1,
            ..Options::default()
        };
        Self {
            lpsolver: empty_lp(options),
            lprows: Vec::new(),
            fractionalints: Vec::new(),
            dualproofvals: Vec::new(),
            dualproofinds: Vec::new(),
            col_lb_buffer: Vec::new(),
            col_ub_buffer: Vec::new(),
            row_ep: HVector::default(),
            row_ap: SparseVectorSum::default(),
            dualproofrhs: 0.0,
            hasdualproof: false,
            objective: -INF,
            basischeckpoint: None,
            currentbasisstored: false,
            numlpiters: 0,
            last_age_call: 0,
            avg_solve_iters: 0.0,
            num_solved: 0,
            epochs: 0,
            max_num_fractional: 0,
            status: LpStatus::NotSet,
            adjust_sym_branching_col: true,
            solved_first_lp: true,
        }
    }

    /// HiGHS's copy constructor, for the LP-based heuristics: the same LP,
    /// options and basis, with fresh statistics.
    pub fn copy(&self, data: &MipData) -> Self {
        let mut lpsolver = self.lpsolver.copy_model(self.lpsolver.options.clone());
        lpsolver.set_interrupt(data.interrupt.clone());
        if let Some(basis) = self.lpsolver.basis() {
            lpsolver.set_basis(basis);
        }
        Self {
            lpsolver,
            lprows: self.lprows.clone(),
            fractionalints: self.fractionalints.clone(),
            dualproofvals: Vec::new(),
            dualproofinds: Vec::new(),
            col_lb_buffer: vec![0.0; data.model.num_col],
            col_ub_buffer: vec![0.0; data.model.num_col],
            row_ep: HVector::default(),
            row_ap: SparseVectorSum::default(),
            dualproofrhs: 0.0,
            hasdualproof: false,
            objective: -INF,
            basischeckpoint: self.basischeckpoint.clone(),
            currentbasisstored: self.currentbasisstored,
            numlpiters: 0,
            last_age_call: 0,
            avg_solve_iters: 0.0,
            num_solved: 0,
            epochs: 0,
            max_num_fractional: 0,
            status: LpStatus::NotSet,
            adjust_sym_branching_col: self.adjust_sym_branching_col,
            solved_first_lp: true,
        }
    }

    /// HiGHS's `loadModel`: the model with the global domain's bounds, no
    /// offset and no cuts.
    pub fn load_model(&mut self, data: &MipData) {
        let model = &data.model;
        self.lprows = (0..model.num_row).map(LpRow::model).collect();
        let options = self.lpsolver.options.clone();
        self.lpsolver = LpSolver::from_col_wise(
            1.0,
            0.0,
            model.col_cost.clone(),
            data.domain.col_lower.clone(),
            data.domain.col_upper.clone(),
            model.row_lower.clone(),
            model.row_upper.clone(),
            model.a_start.clone(),
            model.a_index.clone(),
            model.a_value.clone(),
            options,
        );
        // The run's interrupt reaches every node LP, not only the search
        // loop between them; an interrupted LP leaves its node open.
        self.lpsolver.set_interrupt(data.interrupt.clone());
        self.col_lb_buffer.resize(model.num_col, 0.0);
        self.col_ub_buffer.resize(model.num_col, 0.0);
    }

    pub fn lp_solver(&self) -> &LpSolver {
        &self.lpsolver
    }

    pub fn lp_solver_mut(&mut self) -> &mut LpSolver {
        &mut self.lpsolver
    }

    /// HiGHS's `getRow`.
    pub fn row<'d>(&self, row: usize, data: &'d MipData) -> (&'d [usize], &'d [f64]) {
        self.lprows[row].get(data)
    }

    pub fn is_row_integral(&self, row: usize, data: &MipData) -> bool {
        self.lprows[row].is_integral(data)
    }

    pub fn set_adjust_symmetric_branching_col(&mut self, adjust: bool) {
        self.adjust_sym_branching_col = adjust;
    }

    /// HiGHS's `resetToGlobalDomain`.
    pub fn reset_to_global_domain(&mut self, globaldom: &Domain) {
        let n = self.lpsolver.num_cols();
        self.lpsolver.change_col_bounds((0..n).map(|col| (col, globaldom.col_lower[col], globaldom.col_upper[col])));
    }

    pub fn avg_solve_iters(&self) -> f64 {
        self.avg_solve_iters
    }

    pub fn row_len(&self, row: usize, data: &MipData) -> usize {
        self.lprows[row].row_len(data)
    }

    pub fn max_abs_row_val(&self, row: usize, data: &MipData) -> f64 {
        self.lprows[row].max_abs_val(data)
    }

    pub fn row_lower(&self, row: usize) -> f64 {
        self.lpsolver.row_lower()[row]
    }

    pub fn row_upper(&self, row: usize) -> f64 {
        self.lpsolver.row_upper()[row]
    }

    /// HiGHS's `slackLower`.
    pub fn slack_lower(&self, row: usize, globaldom: &Domain) -> f64 {
        match self.lprows[row].origin {
            Origin::CutPool => globaldom.min_cut_activity(self.lprows[row].index),
            Origin::Model => {
                let rowlower = self.row_lower(row);
                if rowlower != -INF { rowlower } else { globaldom.min_activity(self.lprows[row].index) }
            }
        }
    }

    /// HiGHS's `slackUpper`.
    pub fn slack_upper(&self, row: usize, globaldom: &Domain) -> f64 {
        let rowupper = self.row_upper(row);
        match self.lprows[row].origin {
            Origin::CutPool => rowupper,
            Origin::Model => {
                if rowupper != INF {
                    rowupper
                } else {
                    globaldom.max_activity(self.lprows[row].index)
                }
            }
        }
    }

    pub fn col_lower(&self, col: usize, globaldom: &Domain) -> f64 {
        let num_col = self.lpsolver.num_cols();
        if col < num_col {
            self.lpsolver.col_lower()[col]
        } else {
            self.slack_lower(col - num_col, globaldom)
        }
    }

    pub fn col_upper(&self, col: usize, globaldom: &Domain) -> f64 {
        let num_col = self.lpsolver.num_cols();
        if col < num_col {
            self.lpsolver.col_upper()[col]
        } else {
            self.slack_upper(col - num_col, globaldom)
        }
    }

    pub fn is_col_integral(&self, col: usize, data: &MipData) -> bool {
        let num_col = self.lpsolver.num_cols();
        if col < num_col {
            data.model.is_col_integral(col)
        } else {
            self.is_row_integral(col - num_col, data)
        }
    }

    pub fn solution_value(&self, col: usize) -> f64 {
        let num_col = self.lpsolver.num_cols();
        if col < num_col {
            self.lpsolver.col_value().expect("an LP solution")[col]
        } else {
            self.lpsolver.row_value().expect("an LP solution")[col - num_col]
        }
    }

    pub fn status(&self) -> LpStatus {
        self.status
    }

    pub fn num_lp_iterations(&self) -> i64 {
        self.numlpiters
    }

    pub fn integer_feasible(&self) -> bool {
        matches!(self.status, LpStatus::Optimal | LpStatus::UnscaledPrimalFeasible) && self.fractionalints.is_empty()
    }

    pub fn num_rows(&self) -> usize {
        self.lpsolver.num_rows()
    }

    pub fn num_cols(&self) -> usize {
        self.lpsolver.num_cols()
    }

    pub fn num_model_rows(&self, data: &MipData) -> usize {
        data.model.num_row
    }

    pub fn fractional_integers(&self) -> &[(usize, f64)] {
        &self.fractionalints
    }

    pub fn fractional_integers_mut(&mut self) -> &mut Vec<(usize, f64)> {
        &mut self.fractionalints
    }

    pub fn objective(&self) -> f64 {
        self.objective
    }

    /// HiGHS's `setIterationLimit`.
    pub fn set_iteration_limit(&mut self, limit: usize) {
        self.lpsolver.options.simplex_iteration_limit = limit;
    }

    pub fn set_solved_first_lp(&mut self, solved: bool) {
        self.solved_first_lp = solved;
    }

    /// HiGHS's `storeBasis`.
    pub fn store_basis(&mut self) {
        if !self.currentbasisstored
            && let Some(basis) = self.lpsolver.basis()
        {
            self.basischeckpoint = Some(Rc::new(basis.clone()));
            self.currentbasisstored = true;
        }
    }

    pub fn stored_basis(&self) -> Option<Rc<Basis>> {
        self.basischeckpoint.clone()
    }

    pub fn set_stored_basis(&mut self, basis: Option<Rc<Basis>>) {
        self.basischeckpoint = basis;
        self.currentbasisstored = false;
    }

    /// HiGHS's `recoverBasis`.
    pub fn recover_basis(&mut self) {
        if let Some(basis) = &self.basischeckpoint {
            self.lpsolver.set_basis(basis);
            self.currentbasisstored = true;
        }
    }

    /// HiGHS's `setObjectiveLimit`.
    pub fn set_objective_limit(&mut self, objlim: f64, data: &MipData) {
        let offset = if data.objective_function.is_integral() {
            0.5 / data.objective_function.integral_scale()
        } else {
            (1000.0 * data.feastol).max(objlim.abs() * TINY)
        };
        self.lpsolver.options.objective_bound = objlim + offset;
    }

    /// HiGHS's `getDualProof`.
    pub fn dual_proof(&self) -> (&[usize], &[f64], f64) {
        (&self.dualproofinds, &self.dualproofvals, self.dualproofrhs)
    }

    /// HiGHS's `computeBestEstimate`.
    pub fn compute_best_estimate(&self, ps: &Pseudocost, data: &MipData) -> f64 {
        let mut estimate = CDouble::new(self.objective);
        if !self.fractionalints.is_empty() {
            let mut increase = CDouble::new(0.0);
            let offset = data.feastol * self.objective.abs().max(1.0) / data.integral_cols.len() as f64;
            for &(col, val) in &self.fractionalints {
                increase += ps.pseudocost_up_offset(col, val, offset).min(ps.pseudocost_down_offset(col, val, offset));
            }
            estimate += increase.value();
        }
        estimate.value()
    }

    /// HiGHS's `computeLPDegneracy`.
    pub fn compute_lp_degeneracy(&self, localdomain: &Domain) -> f64 {
        let (Some(basis), Some(row_dual), Some(col_dual)) = (self.lpsolver.basis(), self.lpsolver.row_dual(), self.lpsolver.col_dual()) else {
            return 1.0;
        };
        let dual_feas_tol = self.max_dual_infeasibility();
        let row_lower = self.lpsolver.row_lower();
        let row_upper = self.lpsolver.row_upper();
        let mut num_fixed_rows = 0i64;
        let mut num_inequalities = 0i64;
        let mut num_basic_equalities = 0i64;
        for i in 0..self.num_rows() {
            if row_lower[i] != row_upper[i] {
                num_inequalities += 1;
                if basis.row_status[i] != BasisStatus::Basic && row_dual[i].abs() > dual_feas_tol {
                    num_fixed_rows += 1;
                }
            } else {
                num_basic_equalities += i64::from(basis.row_status[i] == BasisStatus::Basic);
            }
        }
        let mut num_already_fixed_cols = 0i64;
        let mut num_fixed_cols = 0i64;
        for i in 0..self.num_cols() {
            if basis.col_status[i] != BasisStatus::Basic {
                if col_dual[i].abs() > dual_feas_tol {
                    num_fixed_cols += 1;
                } else if localdomain.col_lower[i] == localdomain.col_upper[i] {
                    num_already_fixed_cols += 1;
                }
            }
        }
        let num_cols = self.num_cols() as i64;
        let num_rows = self.num_rows() as i64;
        let base = num_cols - num_already_fixed_cols + num_inequalities + num_basic_equalities - num_rows;
        let degenerate_column_share = if base > 0 { 1.0 - (num_fixed_cols + num_fixed_rows) as f64 / base as f64 } else { 0.0 };
        let var_cons_ratio = if num_rows > 0 {
            (num_cols + num_inequalities + num_basic_equalities - num_fixed_cols - num_fixed_rows - num_already_fixed_cols) as f64 / num_rows as f64
        } else {
            1.0
        };
        let fac1 = if degenerate_column_share < 0.8 {
            1.0
        } else {
            10f64.powf(10.0 * (degenerate_column_share - 0.7))
        };
        let fac2 = if var_cons_ratio < 2.0 { 1.0 } else { 10.0 * var_cons_ratio };
        fac1 * fac2
    }

    /// HiGHS's `info.max_dual_infeasibility` (infinite when not known).
    fn max_dual_infeasibility(&self) -> f64 {
        self.lpsolver.infeasibilities().map_or(INF, |inf| inf.max_dual)
    }

    fn max_primal_infeasibility(&self) -> f64 {
        self.lpsolver.infeasibilities().map_or(INF, |inf| inf.max_primal)
    }

    /// HiGHS's `info.basis_validity != kBasisValidityInvalid` with a dual
    /// solution and dual infeasibilities within `feastol`: what aging needs.
    fn duals_trustworthy(&self, feastol: f64) -> bool {
        self.lpsolver.basis().is_some() && self.max_dual_infeasibility() <= feastol && self.lpsolver.col_dual().is_some()
    }

    /// HiGHS's `addCuts`.
    pub fn add_cuts(&mut self, cutset: &mut CutSet) {
        let numcuts = cutset.num_cuts();
        if numcuts > 0 {
            self.status = LpStatus::NotSet;
            self.currentbasisstored = false;
            self.basischeckpoint = None;
            self.lprows.extend(cutset.cut_indices.iter().map(|&cut| LpRow::cut(cut)));
            self.lpsolver
                .add_rows(&cutset.lower, &cutset.upper, &cutset.ar_start[..numcuts], &cutset.ar_index, &cutset.ar_value);
            cutset.clear();
        }
    }

    /// HiGHS's `removeObsoleteRows`.
    pub fn remove_obsolete_rows(&mut self, notify_pool: bool, data: &mut MipData) {
        let nlprows = self.num_rows();
        let nummodelrows = data.model.num_row;
        let mut deletemask = Vec::new();
        let mut ndelcuts = 0;
        let basis = self.lpsolver.basis().expect("a basis").row_status.clone();
        for i in nummodelrows..nlprows {
            if basis[i] == BasisStatus::Basic {
                if ndelcuts == 0 {
                    deletemask.resize(nlprows, false);
                }
                ndelcuts += 1;
                deletemask[i] = true;
                if notify_pool {
                    data.cutpool.lp_cut_removed(self.lprows[i].index);
                }
            }
        }
        self.remove_cuts_masked(ndelcuts, &deletemask, data);
    }

    /// HiGHS's `removeCuts(ndelcuts, deletemask)`.
    fn remove_cuts_masked(&mut self, ndelcuts: usize, deletemask: &[bool], data: &MipData) {
        if ndelcuts > 0 {
            let mut basis = self.lpsolver.basis().expect("a basis").clone();
            let nlprows = self.lpsolver.num_rows();
            let new_index = self.lpsolver.delete_rows(deletemask);
            for i in data.model.num_row..nlprows {
                if new_index[i] != NONE {
                    self.lprows[new_index[i]] = self.lprows[i];
                    basis.row_status[new_index[i]] = basis.row_status[i];
                }
            }
            basis.row_status.truncate(basis.row_status.len() - ndelcuts);
            self.lprows.truncate(self.lprows.len() - ndelcuts);
            self.lpsolver.set_basis(&basis);
            self.lpsolver.run();
        }
    }

    /// HiGHS's `removeCuts()`: every cut.
    pub fn remove_all_cuts(&mut self, data: &mut MipData) {
        let nlprows = self.lpsolver.num_rows();
        let modelrows = data.model.num_row;
        let mut deletemask = vec![false; nlprows];
        deletemask[modelrows..].fill(true);
        self.lpsolver.delete_rows(&deletemask);
        for lprow in &self.lprows[modelrows..nlprows] {
            if lprow.origin == Origin::CutPool {
                data.cutpool.lp_cut_removed(lprow.index);
            }
        }
        self.lprows.truncate(modelrows);
    }

    /// HiGHS's `performAging`.
    pub fn perform_aging(&mut self, delete_rows: bool, data: &mut MipData) {
        if !self.duals_trustworthy(data.feastol) {
            return;
        }
        let mut agelimit: i64;
        if delete_rows {
            agelimit = data.options.mip_lp_age_limit as i64;
            self.epochs += 1;
            if self.epochs as i64 % (agelimit >> 1).max(2) != 0 {
                agelimit = i32::MAX as i64;
            } else if (self.epochs as i64) < agelimit {
                agelimit = self.epochs as i64;
            }
        } else {
            if self.last_age_call == self.numlpiters {
                return;
            }
            agelimit = i32::MAX as i64;
        }
        self.last_age_call = self.numlpiters;
        let nlprows = self.num_rows();
        let nummodelrows = data.model.num_row;
        let mut deletemask = Vec::new();
        let mut ndelcuts = 0;
        let row_status = self.lpsolver.basis().expect("a basis").row_status.clone();
        let dual_tol = self.lpsolver.options.dual_feasibility_tolerance;
        for i in nummodelrows..nlprows {
            if row_status[i] == BasisStatus::Basic {
                self.lprows[i].age += i32::from(delete_rows || self.lprows[i].age != 0);
                if self.lprows[i].age as i64 > agelimit {
                    if ndelcuts == 0 {
                        deletemask.resize(nlprows, false);
                    }
                    ndelcuts += 1;
                    deletemask[i] = true;
                    data.cutpool.lp_cut_removed(self.lprows[i].index);
                }
            } else if self.lpsolver.row_dual().expect("duals")[i].abs() > dual_tol {
                self.lprows[i].age = 0;
            }
        }
        self.remove_cuts_masked(ndelcuts, &deletemask, data);
    }

    /// HiGHS's `resetAges`.
    pub fn reset_ages(&mut self, data: &MipData) {
        if !self.duals_trustworthy(data.feastol) {
            return;
        }
        let nlprows = self.num_rows();
        let row_status = &self.lpsolver.basis().expect("a basis").row_status;
        let row_dual = self.lpsolver.row_dual().expect("duals");
        let dual_tol = self.lpsolver.options.dual_feasibility_tolerance;
        for i in data.model.num_row..nlprows {
            if row_status[i] != BasisStatus::Basic && row_dual[i].abs() > dual_tol {
                self.lprows[i].age = 0;
            }
        }
    }

    /// HiGHS's `notifyCutPoolsLpCopied`.
    pub fn notify_cut_pools_lp_copied(&self, n: i16, data: &mut MipData) {
        for lprow in &self.lprows[data.model.num_row..self.num_rows()] {
            if lprow.origin == Origin::CutPool {
                data.cutpool.increase_num_lps(lprow.index, n);
            }
        }
    }

    /// HiGHS's `flushDomain`: pass the domain's changed bounds to the LP.
    pub fn flush_domain(&mut self, domain: &mut Domain, mut continuous: bool, model: &super::MipModel) {
        if !domain.changed_cols().is_empty() {
            if domain.is_global() {
                continuous = true;
            }
            self.currentbasisstored = false;
            if !continuous {
                domain.remove_continuous_changed_cols(model);
            }
            let num_chg_cols = domain.changed_cols().len();
            if num_chg_cols == 0 {
                return;
            }
            let changes: Vec<(usize, f64, f64)> = domain.changed_cols().iter().map(|&col| (col, domain.col_lower[col], domain.col_upper[col])).collect();
            for (i, &(_, lower, upper)) in changes.iter().enumerate() {
                self.col_lb_buffer[i] = lower;
                self.col_ub_buffer[i] = upper;
            }
            self.lpsolver.change_col_bounds(changes);
            domain.clear_changed_cols();
        }
    }

    /// [`LpRelaxation::flush_domain`] on the global domain.
    pub fn flush_global_domain(&mut self, data: &mut MipData) {
        self.flush_domain(&mut data.domain, false, &data.model);
    }

    /// HiGHS's `computeBasicDegenerateDuals`.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_basic_degenerate_duals(&mut self, threshold: f64, mut localdom: Option<&mut Domain>, data: &mut MipData, getdualproof: bool) {
        if !self.lpsolver.has_invert() {
            return;
        }
        let feastol = data.feastol;
        let epsilon = data.epsilon;
        let mut k = 0;
        {
            let col_lower = self.lpsolver.col_lower().to_vec();
            let col_upper = self.lpsolver.col_upper().to_vec();
            let col_status = self.lpsolver.basis().expect("a basis").col_status.clone();
            let (col_value, col_dual) = self.lpsolver.col_value_and_dual_mut();
            for &col in &data.integral_cols {
                if col_status[col] != BasisStatus::Basic {
                    continue;
                }
                let lb = col_lower[col];
                let ub = col_upper[col];
                if ub - lb < feastol {
                    continue;
                }
                if col_value[col] - lb < ub - col_value[col] {
                    if col_value[col] > lb + feastol {
                        continue;
                    }
                    col_dual[col] = 1.0;
                } else {
                    if col_value[col] < ub - feastol {
                        continue;
                    }
                    col_dual[col] = -1.0;
                }
                k += 1;
            }
        }
        if k == 0 {
            return;
        }
        let num_row = self.lpsolver.num_rows();
        let num_col = self.lpsolver.num_cols();
        if self.row_ep.size < num_row {
            self.row_ep.setup(num_row);
            if self.row_ap.values.len() < num_col {
                self.row_ap.set_dimension(num_col);
                self.dualproofvals.reserve(num_col);
                self.dualproofinds.reserve(num_col);
            }
        }
        let basic_index = self.lpsolver.basic_variables().to_vec();
        let row_lower = self.lpsolver.row_lower().to_vec();
        let row_upper = self.lpsolver.row_upper().to_vec();
        let col_lower = self.lpsolver.col_lower().to_vec();
        let col_upper = self.lpsolver.col_upper().to_vec();
        let mut row = 0;
        while k > 0 {
            let var = basic_index[row];
            row += 1;
            if var >= num_col {
                continue;
            }
            if self.lpsolver.col_dual().expect("duals")[var].abs() != 1.0 {
                continue;
            }
            k -= 1;
            self.lpsolver.basis_inverse_row_sparse(row - 1, &mut self.row_ep);
            let row_dual = self.lpsolver.row_dual().expect("duals").to_vec();
            let row_value = self.lpsolver.row_value().expect("a solution").to_vec();
            let sign = self.lpsolver.col_dual().expect("duals")[var];
            self.lpsolver.col_value_and_dual_mut().1[var] = 0.0;
            let mut degenerate_col_dual = INF;
            for i in 0..self.row_ep.count as usize {
                let i_row = self.row_ep.index[i];
                let lb = row_lower[i_row];
                let ub = row_upper[i_row];
                if lb == ub {
                    continue;
                }
                let dual = row_dual[i_row];
                let val = -sign * self.row_ep.array[i_row];
                if val > 0.0 {
                    if row_value[i_row] - lb > feastol {
                        degenerate_col_dual = degenerate_col_dual.min(-dual / val);
                        if degenerate_col_dual < threshold {
                            break;
                        }
                    }
                } else if ub - row_value[i_row] > feastol {
                    degenerate_col_dual = degenerate_col_dual.min(-dual / val);
                    if degenerate_col_dual < threshold {
                        break;
                    }
                }
            }
            if degenerate_col_dual < threshold {
                continue;
            }
            self.row_ap.clear();
            for i in 0..self.row_ep.count as usize {
                let i_row = self.row_ep.index[i];
                let (inds, vals) = self.lprows[i_row].get(data);
                for j in 0..inds.len() {
                    self.row_ap.add(inds[j], self.row_ep.array[i_row] * vals[j]);
                }
            }
            self.row_ap.cleanup(|_, val| val.abs() <= epsilon);
            let col_value = self.lpsolver.col_value().expect("a solution");
            let col_dual = self.lpsolver.col_dual().expect("duals");
            for &i_col in &self.row_ap.nonzeroinds {
                if i_col == var {
                    continue;
                }
                let lb = col_lower[i_col];
                let ub = col_upper[i_col];
                if lb == ub {
                    continue;
                }
                let dual = col_dual[i_col];
                let val = sign * self.row_ap.value(i_col);
                if val > epsilon {
                    if col_value[i_col] - lb > feastol {
                        degenerate_col_dual = degenerate_col_dual.min(-dual / val);
                        if degenerate_col_dual < threshold {
                            break;
                        }
                    }
                } else if val < -epsilon && ub - col_value[i_col] > feastol {
                    degenerate_col_dual = degenerate_col_dual.min(-dual / val);
                    if degenerate_col_dual < threshold {
                        break;
                    }
                }
            }
            if degenerate_col_dual < threshold {
                continue;
            }
            if degenerate_col_dual == INF && getdualproof {
                let mut rhs = CDouble::new(0.0);
                for i in 0..self.row_ep.count as usize {
                    let i_row = self.row_ep.index[i];
                    let lb = row_lower[i_row];
                    let ub = row_upper[i_row];
                    let val = sign * self.row_ep.array[i_row];
                    if ub == lb || val > epsilon {
                        rhs += val * ub;
                    } else if val < -epsilon {
                        rhs += val * lb;
                    } else {
                        rhs += val * row_value[i_row];
                    }
                }
                self.dualproofvals.clear();
                for &i in &self.row_ap.nonzeroinds {
                    self.dualproofvals.push(sign * self.row_ap.value(i));
                }
                let domchg = if sign == 1.0 {
                    DomainChange::new(col_lower[var], var, BoundType::Upper)
                } else {
                    DomainChange::new(col_upper[var], var, BoundType::Lower)
                };
                let mut parts = data.parts();
                let mut env = DomainEnv {
                    cx: &mut parts.cx,
                    cliquetable: parts.cliquetable,
                    implications: parts.implications,
                };
                let localdom = localdom.as_deref_mut().expect("a local domain for the dual proof");
                localdom.conflict_analyze_reconvergence(domchg, &self.row_ap.nonzeroinds, &self.dualproofvals, rhs.value(), parts.domain, parts.pseudocost, &mut env);
                continue;
            }
            self.lpsolver.col_value_and_dual_mut().1[var] = sign * degenerate_col_dual;
        }
    }

    /// HiGHS's `computeDualProof` against the global domain: from the
    /// duals, a row `inds . x <= rhs` every solution below `upperbound`
    /// satisfies.
    pub fn compute_dual_proof(&self, upperbound: f64, extract_cliques: bool, data: &mut MipData) -> Option<(Vec<usize>, Vec<f64>, f64)> {
        let (inds, vals, rhs) = self.dual_proof_row(&data.domain, upperbound, data)?;
        if extract_cliques {
            extract_cliques_from_cut(data, &inds, &vals, rhs);
        }
        Some((inds, vals, rhs))
    }

    fn dual_proof_row(&self, globaldomain: &Domain, upperbound: f64, data: &MipData) -> Option<(Vec<usize>, Vec<f64>, f64)> {
        let mut row_dual = self.lpsolver.row_dual().expect("duals").to_vec();
        let row_lower = self.lpsolver.row_lower();
        let row_upper = self.lpsolver.row_upper();
        let mut upper = CDouble::new(upperbound);
        for i in 0..self.num_rows() {
            if row_dual[i] > 0.0 {
                if row_lower[i] != -INF {
                    upper -= CDouble::new(row_dual[i]) * row_lower[i];
                } else {
                    row_dual[i] = 0.0;
                }
            } else if row_dual[i] < 0.0 {
                if row_upper[i] != INF {
                    upper -= CDouble::new(row_dual[i]) * row_upper[i];
                } else {
                    row_dual[i] = 0.0;
                }
            }
        }
        let num_col = self.num_cols();
        let mut inds = Vec::with_capacity(num_col);
        let mut vals = Vec::with_capacity(num_col);
        let (a_start, a_index, a_value) = self.lpsolver.matrix();
        let col_cost = self.lpsolver.col_cost();
        let col_value = self.lpsolver.col_value().expect("a solution");
        for i in 0..num_col {
            let mut exact_val = CDouble::new(col_cost[i]);
            for j in a_start[i]..a_start[i + 1] {
                if row_dual[a_index[j]] == 0.0 {
                    continue;
                }
                exact_val -= CDouble::new(a_value[j]) * row_dual[a_index[j]];
            }
            let val = exact_val.value();
            // Negligible against the column's bounds, and removed in
            // double-double (HiGHS #3223).
            if super::domain::term_is_zero(val, globaldomain.col_lower[i], globaldomain.col_upper[i], data.epsilon) {
                continue;
            }
            let mut remove_value = val.abs() <= data.feastol;
            if !remove_value && (globaldomain.col_lower[i] == globaldomain.col_upper[i] || data.model.is_col_continuous(i)) {
                remove_value = if val > 0.0 {
                    col_value[i] - globaldomain.col_lower[i] <= data.feastol
                } else {
                    globaldomain.col_upper[i] - col_value[i] <= data.feastol
                };
            }
            if remove_value {
                if val < 0.0 {
                    if globaldomain.col_upper[i] == INF {
                        return None;
                    }
                    upper -= exact_val * globaldomain.col_upper[i];
                } else {
                    if globaldomain.col_lower[i] == -INF {
                        return None;
                    }
                    upper -= exact_val * globaldomain.col_lower[i];
                }
                continue;
            }
            vals.push(val);
            inds.push(i);
        }
        let mut rhs = upper.value();
        globaldomain.tighten_coefficients(&inds, &mut vals, &mut rhs, &data.model, data.feastol);
        Some((inds, vals, rhs))
    }

    /// HiGHS's `storeDualInfProof`.
    fn store_dual_inf_proof(&mut self, data: &mut MipData) {
        self.hasdualproof = false;
        if self.lpsolver.basis().is_none() {
            return;
        }
        let num_row = self.lpsolver.num_rows();
        let num_col = self.lpsolver.num_cols();
        if self.row_ep.size < num_row {
            self.row_ep.setup(num_row);
            if self.row_ap.values.len() < num_col {
                self.row_ap.set_dimension(num_col);
                self.dualproofvals.reserve(num_col);
                self.dualproofinds.reserve(num_col);
            }
        }
        self.hasdualproof = self.lpsolver.dual_ray_sparse(&mut self.row_ep);
        if !self.hasdualproof {
            return;
        }
        self.dualproofinds.clear();
        self.dualproofvals.clear();
        self.dualproofrhs = INF;
        let row_lower = self.lpsolver.row_lower();
        let row_upper = self.lpsolver.row_upper();
        let mut upper = CDouble::new(0.0);
        self.row_ap.clear();
        for i in 0..self.row_ep.count as usize {
            let i_row = self.row_ep.index[i];
            let weight = -self.row_ep.array[i_row];
            if weight.abs() * self.lprows[i_row].max_abs_val(data) <= data.epsilon {
                continue;
            } else if weight > 0.0 {
                if row_upper[i_row] == INF {
                    continue;
                }
                upper += CDouble::new(weight) * row_upper[i_row];
            } else {
                if row_lower[i_row] == -INF {
                    continue;
                }
                upper += CDouble::new(weight) * row_lower[i_row];
            }
            let (inds, vals) = self.lprows[i_row].get(data);
            for j in 0..inds.len() {
                self.row_ap.add_c(inds[j], CDouble::new(weight) * vals[j]);
            }
        }
        let globaldomain = &data.domain;
        let lp_col_lower = self.lpsolver.col_lower();
        let lp_col_upper = self.lpsolver.col_upper();
        for &i in self.row_ap.nonzeros() {
            let exact_val = self.row_ap.values[i];
            let val = exact_val.value();
            if super::domain::term_is_zero(val, globaldomain.col_lower[i], globaldomain.col_upper[i], data.epsilon) {
                continue;
            }
            let mut remove_value = val.abs() <= data.feastol;
            if !remove_value && (globaldomain.col_lower[i] == globaldomain.col_upper[i] || data.model.is_col_continuous(i)) {
                remove_value = if val > 0.0 {
                    lp_col_lower[i] - globaldomain.col_lower[i] <= data.feastol
                } else {
                    globaldomain.col_upper[i] - lp_col_upper[i] <= data.feastol
                };
            }
            if remove_value {
                if val < 0.0 {
                    if globaldomain.col_upper[i] == INF {
                        self.hasdualproof = false;
                        return;
                    }
                    upper -= exact_val * globaldomain.col_upper[i];
                } else {
                    if globaldomain.col_lower[i] == -INF {
                        self.hasdualproof = false;
                        return;
                    }
                    upper -= exact_val * globaldomain.col_lower[i];
                }
                continue;
            }
            self.dualproofvals.push(val);
            self.dualproofinds.push(i);
        }
        self.dualproofrhs = upper.value();
        data.domain
            .tighten_coefficients(&self.dualproofinds, &mut self.dualproofvals, &mut self.dualproofrhs, &data.model, data.feastol);
        let (inds, vals, rhs) = (&self.dualproofinds, &self.dualproofvals, self.dualproofrhs);
        extract_cliques_from_cut(data, inds, vals, rhs);
    }

    /// HiGHS's `storeDualUBProof`.
    fn store_dual_ub_proof(&mut self, data: &mut MipData) {
        self.dualproofinds.clear();
        self.dualproofvals.clear();
        self.hasdualproof = false;
        if self.lpsolver.col_dual().is_some() {
            let proof = self.compute_dual_proof(data.upper_limit, true, data);
            if let Some((inds, vals, rhs)) = proof {
                self.dualproofinds = inds;
                self.dualproofvals = vals;
                self.dualproofrhs = rhs;
                self.hasdualproof = true;
            }
        }
        if !self.hasdualproof {
            self.dualproofrhs = INF;
        }
    }

    /// HiGHS's `computeDualInfProof`.
    pub fn compute_dual_inf_proof(&self) -> Option<(Vec<usize>, Vec<f64>, f64)> {
        if !self.hasdualproof {
            return None;
        }
        Some((self.dualproofinds.clone(), self.dualproofvals.clone(), self.dualproofrhs))
    }

    /// HiGHS's `run`.
    pub fn run(&mut self, resolve_on_error: bool, data: &mut MipData) -> LpStatus {
        self.lpsolver.options.time_limit = (data.options.time_limit - data.elapsed()).max(0.0);
        let solver = self.lpsolver.options.solver;
        self.lpsolver.options.solver = Solver::Simplex;
        self.lpsolver.run();
        self.lpsolver.options.solver = solver;
        self.solved_first_lp = true;
        let itercount = self.lpsolver.iterations() as i64;
        self.numlpiters += itercount;

        let model_status = self.lpsolver.status();
        if model_status == ModelStatus::SolveError {
            self.lpsolver.clear_solver();
            if resolve_on_error {
                self.lpsolver.options.simplex_strategy = crate::simplex::options::SimplexStrategy::Dual;
                self.lpsolver.options.presolve = true;
                let retval = self.run(false, data);
                self.lpsolver.options.presolve = false;
                return retval;
            }
            self.recover_basis();
            return LpStatus::Error;
        }
        let record_solve = |this: &mut Self| {
            this.num_solved += 1;
            this.avg_solve_iters += (itercount as f64 - this.avg_solve_iters) / this.num_solved as f64;
        };
        match model_status {
            ModelStatus::ObjectiveBound => {
                record_solve(self);
                self.store_dual_ub_proof(data);
                LpStatus::Infeasible
            }
            ModelStatus::Infeasible => {
                record_solve(self);
                self.store_dual_inf_proof(data);
                LpStatus::Infeasible
            }
            ModelStatus::Unbounded => {
                if self.lpsolver.infeasibilities().and_then(|inf| inf.num_primal) == Some(0) {
                    let solution = self.lpsolver.col_value().expect("a solution").to_vec();
                    data.try_solution(&solution, SolutionSource::Unbounded);
                }
                LpStatus::Unbounded
            }
            ModelStatus::Unknown | ModelStatus::Optimal => {
                if model_status == ModelStatus::Unknown && self.lpsolver.basis().is_none() {
                    return LpStatus::Error;
                }
                record_solve(self);
                let max_primal = self.max_primal_infeasibility();
                let max_dual = self.max_dual_infeasibility();
                if max_primal <= data.feastol && max_dual <= data.feastol {
                    LpStatus::Optimal
                } else if max_primal <= data.feastol {
                    LpStatus::UnscaledPrimalFeasible
                } else if max_dual <= data.feastol {
                    LpStatus::UnscaledDualFeasible
                } else if model_status == ModelStatus::Optimal {
                    LpStatus::UnscaledInfeasible
                } else {
                    LpStatus::Error
                }
            }
            ModelStatus::IterationLimit => {
                // What is left of the MIP's own deadline: recovery is no
                // excuse to run past it.
                let time_left = data.options.time_limit - data.elapsed();
                if resolve_on_error && time_left > 0.0 {
                    let options = Options {
                        presolve: !data.options.mip_root_presolve_only,
                        solver: Solver::Ipm,
                        ipm_iteration_limit: 200,
                        simplex_iteration_limit: self.lpsolver.iterations(),
                        time_limit: time_left,
                        ..Options::default()
                    };
                    let mut ipm = self.lpsolver.copy_model(options);
                    ipm.set_interrupt(data.interrupt.clone());
                    ipm.run();
                    if let Some(basis) = ipm.basis() {
                        self.lpsolver.set_basis(basis);
                    }
                    return self.run(false, data);
                }
                LpStatus::Error
            }
            _ => LpStatus::Error,
        }
    }

    /// HiGHS's `resolveLp`.
    pub fn resolve_lp(&mut self, mut domain: LpDomain, data: &mut MipData) -> LpStatus {
        self.fractionalints.clear();
        loop {
            let mut solveagain = false;
            match &mut domain {
                LpDomain::None => {}
                LpDomain::Global => self.flush_global_domain(data),
                LpDomain::Local(local) => self.flush_domain(local, false, &data.model),
            }
            self.status = self.run(true, data);
            match self.status {
                LpStatus::UnscaledInfeasible | LpStatus::UnscaledDualFeasible | LpStatus::UnscaledPrimalFeasible | LpStatus::Optimal => {
                    if let Some(status) = self.process_solution(&mut domain, &mut solveagain, data) {
                        return status;
                    }
                }
                LpStatus::Infeasible => self.objective = INF,
                _ => {}
            }
            if !solveagain {
                break;
            }
        }
        self.status
    }

    /// `resolveLp`'s handling of a solved LP; returns a status to stop
    /// with, and sets `solveagain` when fixings were made.
    fn process_solution(&mut self, domain: &mut LpDomain, solveagain: &mut bool, data: &mut MipData) -> Option<LpStatus> {
        let mut fracints: crate::hash_table::HashTable<usize, (f64, i32)> = crate::hash_table::HashTable::with_capacity(self.max_num_fractional);
        let col_value = self.lpsolver.col_value().expect("a solution").to_vec();
        let lp_col_lower = self.lpsolver.col_lower().to_vec();
        let lp_col_upper = self.lpsolver.col_upper().to_vec();
        let mut objsum = CDouble::new(0.0);
        let mut roundable = true;
        let feastol = data.feastol;
        let integral_cols = data.integral_cols.clone();
        for &i in &integral_cols {
            let mut val = col_value[i].min(lp_col_upper[i]).max(lp_col_lower[i]);
            if fractionality(val) > feastol {
                let mut col = i;
                roundable = roundable && (data.uplocks[col] == 0 || data.downlocks[col] == 0);
                let mut subst = data.cliquetable.substitution(col).copied();
                while let Some(s) = subst {
                    if lp_col_lower[s.replace.col()] == lp_col_upper[s.replace.col()] {
                        let fixval = s.replace.weight(&lp_col_lower);
                        match domain {
                            LpDomain::None => break,
                            LpDomain::Global => {
                                let mut parts = data.parts();
                                parts.on_domain(|dom, env| dom.fix_col(col, fixval, Reason::unspecified(), env));
                                if data.domain.infeasible() {
                                    self.objective = INF;
                                    self.status = LpStatus::Infeasible;
                                    return Some(self.status);
                                }
                            }
                            LpDomain::Local(local) => {
                                let mut parts = data.parts();
                                let mut env = DomainEnv {
                                    cx: &mut parts.cx,
                                    cliquetable: parts.cliquetable,
                                    implications: parts.implications,
                                };
                                local.fix_col(col, fixval, Reason::unspecified(), &mut env);
                                if local.infeasible() {
                                    self.objective = INF;
                                    self.status = LpStatus::Infeasible;
                                    return Some(self.status);
                                }
                            }
                        }
                    }
                    // A global change during the search may have made a
                    // clique substitution the local domain no longer allows.
                    let dom: Option<&Domain> = match domain {
                        LpDomain::None => None,
                        LpDomain::Global => Some(&data.domain),
                        LpDomain::Local(local) => Some(local),
                    };
                    if let Some(dom) = dom {
                        let replace_val = if s.replace.val() == 0 { 1.0 - val } else { val };
                        let replace_lb = dom.col_lower[s.replace.col()];
                        let replace_ub = dom.col_upper[s.replace.col()];
                        if replace_val < replace_lb - feastol || replace_val > replace_ub + feastol {
                            break;
                        }
                    }
                    col = s.replace.col();
                    if s.replace.val() == 0 {
                        val = 1.0 - val;
                    }
                    subst = data.cliquetable.substitution(col).copied();
                }
                if self.adjust_sym_branching_col {
                    col = data.symmetries.branching_column(&lp_col_lower, &lp_col_upper, col);
                }
                let pair = fracints.entry(col);
                pair.0 += val;
                pair.1 += 1;
            } else {
                if self.lpsolver.basis().expect("a basis").col_status[i] == BasisStatus::Basic {
                    continue;
                }
                let glb = data.domain.col_lower[i];
                let gub = data.domain.col_upper[i];
                if (gub - col_value[i]).min(col_value[i] - glb) <= feastol {
                    continue;
                }
                let (a_start, a_index, _) = self.lpsolver.matrix();
                let col_start = a_start[i] + (data.model.a_start[i + 1] - data.model.a_start[i]);
                let col_end = a_start[i + 1];
                if col_start == col_end {
                    continue;
                }
                let row_value = self.lpsolver.row_value().expect("a solution");
                let row_upper = self.lpsolver.row_upper();
                for a_index_j in &a_index[col_start..col_end] {
                    let row = *a_index_j;
                    if self.lprows[row].age == 0 {
                        continue;
                    }
                    if row_value[row] < row_upper[row] - feastol {
                        continue;
                    }
                    self.lprows[row].age = 0;
                }
            }
        }
        self.max_num_fractional = self.max_num_fractional.max(fracints.len());
        let changed = match domain {
            LpDomain::None => false,
            LpDomain::Global => !data.domain.changed_cols().is_empty(),
            LpDomain::Local(local) => !local.changed_cols().is_empty(),
        };
        if changed {
            *solveagain = true;
            return None;
        }
        for (&col, &(sum, count)) in fracints.iter() {
            self.fractionalints.push((col, sum / count as f64));
        }
        let num_col = data.model.num_col;
        if roundable && !self.fractionalints.is_empty() {
            let mut roundsol = col_value.clone();
            for &(col, frac) in &self.fractionalints {
                if data.uplocks[col] == 0 && (data.model.col_cost[col] < 0.0 || data.downlocks[col] != 0) {
                    roundsol[col] = (frac - feastol)
                        .ceil()
                        .min(if lp_col_upper[col] == INF { INF } else { (lp_col_upper[col] + feastol).floor() });
                } else {
                    roundsol[col] = (frac + feastol)
                        .floor()
                        .max(if lp_col_lower[col] == -INF { -INF } else { (lp_col_lower[col] - feastol).ceil() });
                }
            }
            for subst in data.cliquetable.substitutions().iter().rev() {
                roundsol[subst.substcol] = if subst.replace.val() == 0 {
                    1.0 - roundsol[subst.replace.col()]
                } else {
                    roundsol[subst.replace.col()]
                };
            }
            for i in 0..num_col {
                objsum += CDouble::new(roundsol[i]) * data.model.col_cost[i];
            }
            data.add_incumbent(&roundsol, objsum.value(), SolutionSource::SolveLp, true, false);
            objsum = CDouble::new(0.0);
        }
        for i in 0..num_col {
            objsum += CDouble::new(col_value[i]) * data.model.col_cost[i];
        }
        if self.fractionalints.is_empty() && !self.status.unscaled_primal_feasible() {
            let mut fix_sol = col_value.clone();
            for i in 0..num_col {
                if fix_sol[i] < lp_col_lower[i] {
                    fix_sol[i] = lp_col_lower[i];
                } else if fix_sol[i] > lp_col_upper[i] {
                    fix_sol[i] = lp_col_upper[i];
                } else if data.model.is_col_integral(i) {
                    fix_sol[i] = fix_sol[i].round();
                }
            }
            if data.check_solution(&fix_sol) {
                self.lpsolver.col_value_and_dual_mut().0.copy_from_slice(&fix_sol);
                self.status = if self.status.unscaled_dual_feasible() {
                    LpStatus::Optimal
                } else {
                    LpStatus::UnscaledPrimalFeasible
                };
            }
        }
        self.objective = objsum.value();
        None
    }
}

/// The clique table's `extractCliquesFromCut` on the global domain.
fn extract_cliques_from_cut(data: &mut MipData, inds: &[usize], vals: &[f64], rhs: f64) {
    let num_entries_after_presolve = data.num_clique_entries_after_presolve;
    let mut parts = data.parts();
    parts.on_cliques(|cliquetable, domain, cc| cliquetable.extract_cliques_from_cut(cc.cx.model, domain, inds, vals, rhs, num_entries_after_presolve, cc));
}
