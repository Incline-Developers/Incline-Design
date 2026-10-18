//! HiGHS's `HighsDomain`: the bounds of every column in one node of the
//! search (or globally), the stack of bound changes that led there with
//! their reasons, row activities for propagation over the model's rows, the
//! cut pool, the conflict pool and the objective, and conflict analysis.

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BinaryHeap},
};

use super::{
    BoundType, DomainChange, MipModel, VarType,
    change_log::Listener,
    clique_table::CliqueTable,
    cmax, cmin,
    conflict_pool::{ConflictEvent, ConflictPool},
    cut_pool::{CutEvent, CutPool},
    implications::Implications,
    node_queue::NodeQueue,
    objective_function::ObjectiveFunction,
    pseudocost::Pseudocost,
};
use crate::{
    hash_table::HashSet,
    rb_tree::{NO_LINK, RbLinks, RbTree},
    util::CDouble,
};

const INF: f64 = f64::INFINITY;
/// HiGHS's -1 for a stack position.
pub(crate) const NO_POS: isize = -1;
/// HiGHS's `kHighsIInf` for a stack position.
const POS_INF: isize = i32::MAX as isize;

/// HiGHS's `HighsDomain::Reason`: why a bound changed. A type of zero or
/// more names a cut pool, then the conflict pools after them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Reason {
    pub kind: i32,
    pub index: usize,
}

impl Reason {
    pub const BRANCHING: i32 = -1;
    pub const UNKNOWN: i32 = -2;
    pub const MODEL_ROW_UPPER: i32 = -3;
    pub const MODEL_ROW_LOWER: i32 = -4;
    pub const CLIQUE_TABLE: i32 = -5;
    pub const CONFLICTING_BOUNDS: i32 = -6;
    pub const OBJECTIVE: i32 = -7;

    pub fn branching() -> Self {
        Self { kind: Self::BRANCHING, index: 0 }
    }
    pub fn unspecified() -> Self {
        Self { kind: Self::UNKNOWN, index: 0 }
    }
    pub fn clique_table(col: usize, val: usize) -> Self {
        Self {
            kind: Self::CLIQUE_TABLE,
            index: 2 * col + val,
        }
    }
    pub fn model_row_upper(row: usize) -> Self {
        Self {
            kind: Self::MODEL_ROW_UPPER,
            index: row,
        }
    }
    pub fn model_row_lower(row: usize) -> Self {
        Self {
            kind: Self::MODEL_ROW_LOWER,
            index: row,
        }
    }
    pub fn cut(pool: usize, cut: usize) -> Self {
        Self { kind: pool as i32, index: cut }
    }
    pub fn conflicting_bounds(pos: usize) -> Self {
        Self {
            kind: Self::CONFLICTING_BOUNDS,
            index: pos,
        }
    }
    pub fn objective() -> Self {
        Self { kind: Self::OBJECTIVE, index: 0 }
    }
}

/// What a domain reads or changes besides itself. The clique table and
/// implications are read while a bound changes; they are borrowed
/// separately so that their own methods can lend themselves while they
/// change a domain.
pub(crate) struct MipCtx<'a> {
    pub model: &'a MipModel,
    pub feastol: f64,
    pub epsilon: f64,
    pub upper_limit: f64,
    pub objective: &'a ObjectiveFunction,
    pub cutpool: &'a mut CutPool,
    pub conflictpool: &'a mut ConflictPool,
    /// Read by conflict analysis and pruned by new cliques; absent while
    /// the node queue itself changes a domain.
    pub nodequeue: Option<&'a RefCell<NodeQueue>>,
    /// HiGHS's `pruned_treeweight`, which pruned nodes add to.
    pub pruned_treeweight: Option<&'a Cell<CDouble>>,
    /// The global domain's number of integral columns
    /// (`mipdata_->integral_cols.size()`).
    pub num_integral_cols: usize,
}

pub(crate) struct DomainEnv<'e, 'a> {
    pub cx: &'e mut MipCtx<'a>,
    pub cliquetable: &'e CliqueTable,
    pub implications: &'e Implications,
}

impl<'a> std::ops::Deref for DomainEnv<'_, 'a> {
    type Target = MipCtx<'a>;
    fn deref(&self) -> &MipCtx<'a> {
        self.cx
    }
}

impl<'a> std::ops::DerefMut for DomainEnv<'_, 'a> {
    fn deref_mut(&mut self) -> &mut MipCtx<'a> {
        self.cx
    }
}

fn activity_contribution_min(coef: f64, lb: f64, ub: f64) -> f64 {
    if coef < 0.0 {
        if ub == INF { -INF } else { coef * ub }
    } else if lb == -INF {
        -INF
    } else {
        coef * lb
    }
}

fn activity_contribution_max(coef: f64, lb: f64, ub: f64) -> f64 {
    if coef < 0.0 {
        if lb == -INF { INF } else { coef * lb }
    } else if ub == INF {
        INF
    } else {
        coef * ub
    }
}

/// HiGHS's `computeDelta`: the activity change of moving a bound from
/// `oldbound` to `newbound`, counting infinite contributions in `numinfs`.
fn compute_delta(val: f64, oldbound: f64, newbound: f64, inf: f64, numinfs: &mut i32) -> CDouble {
    if oldbound == inf {
        *numinfs -= 1;
        CDouble::new(newbound) * val
    } else if newbound == inf {
        *numinfs += 1;
        CDouble::new(-oldbound) * val
    } else {
        (CDouble::new(newbound) - CDouble::new(oldbound)) * val
    }
}

/// HiGHS's `boundRange`: a column's range less what a propagation must
/// beat to count.
fn bound_range(upper_bound: f64, lower_bound: f64, tolerance: f64, var_type: VarType) -> f64 {
    let range = upper_bound - lower_bound;
    range
        - if var_type == VarType::Continuous {
            cmax(0.3 * range, 1000.0 * tolerance)
        } else {
            tolerance
        }
}

/// `std::max({a, b, c})`.
fn max3(a: f64, b: f64, c: f64) -> f64 {
    cmax(cmax(a, b), c)
}

/// HiGHS's `CutpoolPropagation`: activities and propagation marks of the
/// cuts of the pool.
#[derive(Clone, Debug)]
pub(crate) struct CutpoolPropagation {
    cutpoolindex: usize,
    listener: Listener,
    activitycuts: Vec<CDouble>,
    activitycutsinf: Vec<i32>,
    /// Bit 0: marked for propagation; bit 1: deleted.
    propagatecutflags: Vec<u8>,
    propagatecutinds: Vec<usize>,
    capacity_threshold: Vec<f64>,
}

/// HiGHS's `ConflictPoolPropagation::WatchedLiteral`.
#[derive(Clone, Copy, Debug)]
struct WatchedLiteral {
    domchg: DomainChange,
    /// The watched column, or `None` when nothing is watched.
    column: Option<usize>,
    prev: isize,
    next: isize,
}

impl Default for WatchedLiteral {
    fn default() -> Self {
        Self {
            domchg: DomainChange::default(),
            column: None,
            prev: -1,
            next: -1,
        }
    }
}

/// HiGHS's `ConflictPoolPropagation`.
#[derive(Clone, Debug)]
pub(crate) struct ConflictPoolPropagation {
    conflictpoolindex: usize,
    listener: Listener,
    col_lower_watched: Vec<isize>,
    col_upper_watched: Vec<isize>,
    /// Bits 0-1: inactive watched literals; 2: marked; 3: deleted.
    conflict_flag: Vec<u8>,
    propagate_conflict_inds: Vec<usize>,
    watched_literals: Vec<WatchedLiteral>,
}

/// One column's place in a clique partition's tree of objective
/// contributions.
#[derive(Clone, Copy, Debug, Default)]
struct ObjectiveContribution {
    contribution: f64,
    col: usize,
    partition: usize,
    links: RbLinks,
}

#[derive(Clone, Copy, Debug, Default)]
struct PartitionCliqueData {
    multiplier: f64,
    rhs: i32,
}

/// HiGHS's `ObjectivePropagation`.
#[derive(Clone, Debug, Default)]
struct ObjectivePropagation {
    active: bool,
    objective_lower: CDouble,
    num_inf_obj_lower: i32,
    capacity_threshold: f64,
    is_propagated: bool,
    objective_lower_contributions: Vec<ObjectiveContribution>,
    /// Each partition's tree: `(root, cached first)`.
    contribution_partition_sets: Vec<(usize, usize)>,
    propagation_cons_buffer: Vec<f64>,
    partition_clique_data: Vec<PartitionCliqueData>,
}

/// HiGHS's `ObjectiveContributionTree`: partition `partition`'s columns
/// ordered by decreasing contribution.
struct ContributionTree<'a> {
    prop: &'a mut ObjectivePropagation,
    partition: usize,
}

impl RbTree for ContributionTree<'_> {
    type Key = (f64, usize);
    fn root(&self) -> usize {
        self.prop.contribution_partition_sets[self.partition].0
    }
    fn set_root(&mut self, root: usize) {
        self.prop.contribution_partition_sets[self.partition].0 = root;
    }
    fn links(&self, node: usize) -> &RbLinks {
        &self.prop.objective_lower_contributions[node].links
    }
    fn links_mut(&mut self, node: usize) -> &mut RbLinks {
        &mut self.prop.objective_lower_contributions[node].links
    }
    fn key(&self, node: usize) -> (f64, usize) {
        let contribution = &self.prop.objective_lower_contributions[node];
        (-contribution.contribution, contribution.col)
    }
    fn cached_first(&mut self) -> Option<&mut usize> {
        Some(&mut self.prop.contribution_partition_sets[self.partition].1)
    }
}

impl ObjectivePropagation {
    fn tree(&mut self, partition: usize) -> ContributionTree<'_> {
        ContributionTree { prop: self, partition }
    }

    /// The partition's cached first (worst) column.
    fn tree_first(&self, partition: usize) -> usize {
        self.contribution_partition_sets[partition].1
    }
}

/// HiGHS's `HighsDomain`.
#[derive(Clone, Debug)]
pub(crate) struct Domain {
    changedcolsflags: Vec<bool>,
    changedcols: Vec<usize>,
    prop_row_num_changed_bounds: Vec<(usize, usize)>,
    domchgstack: Vec<DomainChange>,
    domchgreason: Vec<Reason>,
    /// The bound before each change and the stack position of the change
    /// before it to the same bound.
    prevboundval: Vec<(f64, isize)>,
    activitymin: Vec<CDouble>,
    activitymax: Vec<CDouble>,
    activitymininf: Vec<i32>,
    activitymaxinf: Vec<i32>,
    capacity_threshold: Vec<f64>,
    propagateflags: Vec<bool>,
    propagateinds: Vec<usize>,
    obj_prop: ObjectivePropagation,
    cutpoolpropagation: Vec<CutpoolPropagation>,
    conflict_pool_propagation: Vec<ConflictPoolPropagation>,
    infeasible: bool,
    infeasible_reason: Reason,
    infeasible_pos: isize,
    col_lower_pos: Vec<isize>,
    col_upper_pos: Vec<isize>,
    branch_pos: Vec<usize>,
    redundant_rows: HashSet<usize>,
    record_redundant_rows: bool,
    pub col_lower: Vec<f64>,
    pub col_upper: Vec<f64>,
    /// This is the solver's global domain (HiGHS compares its address).
    is_global: bool,
}

impl Domain {
    /// HiGHS's `HighsDomain(mipsolver)`: the model's bounds, no changes.
    pub fn new(model: &MipModel) -> Self {
        Self {
            changedcolsflags: vec![false; model.num_col],
            changedcols: Vec::with_capacity(model.num_col),
            prop_row_num_changed_bounds: Vec::new(),
            domchgstack: Vec::new(),
            domchgreason: Vec::new(),
            prevboundval: Vec::new(),
            activitymin: Vec::new(),
            activitymax: Vec::new(),
            activitymininf: Vec::new(),
            activitymaxinf: Vec::new(),
            capacity_threshold: Vec::new(),
            propagateflags: Vec::new(),
            propagateinds: Vec::new(),
            obj_prop: ObjectivePropagation::default(),
            cutpoolpropagation: Vec::new(),
            conflict_pool_propagation: Vec::new(),
            infeasible: false,
            infeasible_reason: Reason::unspecified(),
            infeasible_pos: 0,
            col_lower_pos: vec![NO_POS; model.num_col],
            col_upper_pos: vec![NO_POS; model.num_col],
            branch_pos: Vec::new(),
            redundant_rows: HashSet::new(),
            record_redundant_rows: false,
            col_lower: model.col_lower.clone(),
            col_upper: model.col_upper.clone(),
            is_global: false,
        }
    }

    /// Mark this as the solver's global domain.
    pub fn set_global(&mut self, is_global: bool) {
        self.is_global = is_global;
    }

    pub fn is_global(&self) -> bool {
        self.is_global
    }

    /// A copy that is not the global domain (HiGHS's copy constructor).
    pub fn local_copy(&self) -> Self {
        let mut copy = self.clone();
        copy.is_global = false;
        copy
    }

    pub fn add_cutpool(&mut self, cutpool: &CutPool) {
        let cutpoolindex = self.cutpoolpropagation.len();
        self.cutpoolpropagation.push(CutpoolPropagation {
            cutpoolindex,
            listener: cutpool.log.listen(),
            activitycuts: Vec::new(),
            activitycutsinf: Vec::new(),
            propagatecutflags: Vec::new(),
            propagatecutinds: Vec::new(),
            capacity_threshold: Vec::new(),
        });
    }

    pub fn add_conflict_pool(&mut self, conflictpool: &ConflictPool, num_col: usize) {
        let conflictpoolindex = self.conflict_pool_propagation.len();
        self.conflict_pool_propagation.push(ConflictPoolPropagation {
            conflictpoolindex,
            listener: conflictpool.log.listen(),
            col_lower_watched: vec![-1; num_col],
            col_upper_watched: vec![-1; num_col],
            conflict_flag: Vec::new(),
            propagate_conflict_inds: Vec::new(),
            watched_literals: Vec::new(),
        });
    }

    pub fn infeasible(&self) -> bool {
        self.infeasible
    }

    /// HiGHS's `markInfeasible` with an unspecified reason.
    pub fn mark_infeasible(&mut self) {
        self.infeasible = true;
        self.infeasible_pos = self.domchgstack.len() as isize;
        self.infeasible_reason = Reason::unspecified();
    }

    pub fn infeasible_reason(&self) -> Reason {
        self.infeasible_reason
    }

    pub fn changed_cols(&self) -> &[usize] {
        &self.changedcols
    }

    pub fn clear_changed_cols(&mut self) {
        for &col in &self.changedcols {
            self.changedcolsflags[col] = false;
        }
        self.changedcols.clear();
    }

    pub fn remove_continuous_changed_cols(&mut self, model: &MipModel) {
        for &col in &self.changedcols {
            self.changedcolsflags[col] = model.is_col_integral(col);
        }
        let flags = &self.changedcolsflags;
        self.changedcols.retain(|&col| flags[col]);
    }

    pub fn clear_changed_cols_from(&mut self, start: usize) {
        for &col in &self.changedcols[start..] {
            self.changedcolsflags[col] = false;
        }
        self.changedcols.truncate(start);
    }

    pub fn is_changed_col(&self, col: usize) -> bool {
        self.changedcolsflags[col]
    }

    pub fn is_active(&self, domchg: &DomainChange) -> bool {
        match domchg.boundtype {
            BoundType::Lower => domchg.boundval <= self.col_lower[domchg.column],
            BoundType::Upper => domchg.boundval >= self.col_upper[domchg.column],
        }
    }

    pub fn branching_positions(&self) -> &[usize] {
        &self.branch_pos
    }

    pub fn previous_bounds(&self) -> &[(f64, isize)] {
        &self.prevboundval
    }

    pub fn domain_change_stack(&self) -> &[DomainChange] {
        &self.domchgstack
    }

    pub fn domain_change_reason(&self) -> &[Reason] {
        &self.domchgreason
    }

    pub fn objective_lower_bound(&self) -> f64 {
        if self.obj_prop.active && self.obj_prop.num_inf_obj_lower == 0 {
            return self.obj_prop.objective_lower.value();
        }
        -INF
    }

    pub fn num_domain_changes(&self) -> usize {
        self.domchgstack.len()
    }

    pub fn col_bounds_are_global(&self, col: usize) -> bool {
        self.col_lower_pos[col] == NO_POS && self.col_upper_pos[col] == NO_POS
    }

    pub fn branch_depth(&self) -> usize {
        self.branch_pos.len()
    }

    pub fn min_activity(&self, row: usize) -> f64 {
        if self.activitymininf[row] == 0 { self.activitymin[row].value() } else { -INF }
    }

    pub fn max_activity(&self, row: usize) -> f64 {
        if self.activitymaxinf[row] == 0 { self.activitymax[row].value() } else { INF }
    }

    pub fn is_binary(&self, model: &MipModel, col: usize) -> bool {
        model.is_col_integral(col) && self.col_lower[col] == 0.0 && self.col_upper[col] == 1.0
    }

    pub fn is_fixed(&self, col: usize) -> bool {
        self.col_lower[col] == self.col_upper[col]
    }

    pub fn clear_redundant_rows(&mut self) {
        self.redundant_rows.clear();
    }

    pub fn redundant_rows(&self) -> &HashSet<usize> {
        &self.redundant_rows
    }

    pub fn set_record_redundant_rows(&mut self, value: bool) {
        self.record_redundant_rows = value;
    }

    /// HiGHS's `getReducedDomainChangeStack`: each column's tightest bound
    /// changes, and where the branching ones are in that list.
    pub fn reduced_domain_change_stack(&self) -> (Vec<DomainChange>, Vec<usize>) {
        let mut reducedstack = Vec::with_capacity(self.domchgstack.len());
        let mut branching_positions = Vec::with_capacity(self.branch_pos.len());
        for i in 0..self.domchgstack.len() {
            let domchg = &self.domchgstack[i];
            let pos = match domchg.boundtype {
                BoundType::Lower => self.col_lower_pos[domchg.column],
                BoundType::Upper => self.col_upper_pos[domchg.column],
            };
            if pos != i as isize {
                continue;
            }
            if self.domchgreason[i].kind == Reason::BRANCHING {
                branching_positions.push(reducedstack.len());
            } else {
                let mut k = i as isize;
                while self.prevboundval[k as usize].1 != NO_POS {
                    k = self.prevboundval[k as usize].1;
                    if self.domchgreason[k as usize].kind == Reason::BRANCHING {
                        branching_positions.push(reducedstack.len());
                        break;
                    }
                }
            }
            reducedstack.push(*domchg);
        }
        reducedstack.shrink_to_fit();
        (reducedstack, branching_positions)
    }

    // ---- replaying the pools' changes -----------------------------------

    /// Catch up on the pools' changes since this domain last looked: what
    /// HiGHS's pools would have told it as they happened.
    pub fn sync(&mut self, env: &mut DomainEnv) {
        if !self.cutpoolpropagation.is_empty() {
            let range = env.cx.cutpool.log.take_unseen(&self.cutpoolpropagation[0].listener);
            for at in range {
                let event = env.cx.cutpool.log.event(at).clone();
                match event {
                    CutEvent::Added {
                        cut,
                        propagate,
                        index,
                        value,
                        rhs,
                    } => self.cut_added(0, cut, propagate, &index, &value, rhs, env),
                    CutEvent::Deleted { cut, only_for_propagation } => self.cut_deleted(0, cut, only_for_propagation),
                }
            }
        }
        if !self.conflict_pool_propagation.is_empty() {
            let range = env.cx.conflictpool.log.take_unseen(&self.conflict_pool_propagation[0].listener);
            for at in range {
                let event = env.cx.conflictpool.log.event(at).clone();
                match event {
                    ConflictEvent::Added { conflict, entries } => self.conflict_added(0, conflict, &entries),
                    ConflictEvent::Deleted { conflict } => self.conflict_deleted(0, conflict),
                }
            }
        }
    }

    // ---- cut pool propagation -------------------------------------------

    fn cutpool_recompute_capacity_threshold(&mut self, pool: usize, cut: usize, index: &[usize], value: &[f64], env: &DomainEnv) {
        let feastol = env.feastol;
        let mut threshold_value = -feastol;
        for (&col, &val) in index.iter().zip(value) {
            if self.col_upper[col] == self.col_lower[col] {
                continue;
            }
            let threshold = val.abs() * bound_range(self.col_upper[col], self.col_lower[col], feastol, env.model.integrality[col]);
            threshold_value = max3(threshold_value, threshold, feastol);
        }
        self.cutpoolpropagation[pool].capacity_threshold[cut] = threshold_value;
    }

    #[allow(clippy::too_many_arguments)]
    fn cut_added(&mut self, pool: usize, cut: usize, propagate: bool, index: &[usize], value: &[f64], rhs: f64, env: &DomainEnv) {
        if !propagate && !self.is_global {
            return;
        }
        let prop = &mut self.cutpoolpropagation[pool];
        if prop.activitycuts.len() <= cut {
            prop.activitycuts.resize(cut + 1, CDouble::new(0.0));
            prop.activitycutsinf.resize(cut + 1, 0);
            prop.propagatecutflags.resize(cut + 1, 2);
            prop.capacity_threshold.resize(cut + 1, 0.0);
        }
        prop.propagatecutflags[cut] &= !2;
        let (ninf, activity) = self.compute_min_activity(index, value);
        let prop = &mut self.cutpoolpropagation[pool];
        prop.activitycutsinf[cut] = ninf;
        prop.activitycuts[cut] = activity;
        if propagate {
            self.cutpool_recompute_capacity_threshold(pool, cut, index, value, env);
            self.mark_propagate_cut_with_rhs(pool, cut, rhs);
        }
    }

    fn cut_deleted(&mut self, pool: usize, cut: usize, only_for_propagation: bool) {
        if only_for_propagation && self.is_global {
            return;
        }
        let prop = &mut self.cutpoolpropagation[pool];
        if cut < prop.propagatecutflags.len() {
            prop.propagatecutflags[cut] |= 2;
        }
    }

    fn mark_propagate_cut_with_rhs(&mut self, pool: usize, cut: usize, rhs: f64) {
        let prop = &mut self.cutpoolpropagation[pool];
        if prop.propagatecutflags[cut] == 0 && (prop.activitycutsinf[cut] == 1 || rhs - prop.activitycuts[cut].value() <= prop.capacity_threshold[cut]) {
            prop.propagatecutinds.push(cut);
            prop.propagatecutflags[cut] |= 1;
        }
    }

    fn mark_propagate_cut_in_pool(&mut self, pool: usize, cut: usize, cutpool: &CutPool) {
        let rhs = cutpool.rhs()[cut];
        self.mark_propagate_cut_with_rhs(pool, cut, rhs);
    }

    /// `CutpoolPropagation::updateActivityLbChange`.
    #[allow(clippy::too_many_arguments)]
    fn cutpool_update_activity_lb_change(&mut self, pool: usize, col: usize, oldbound: f64, newbound: f64, threshold: bool, activity: bool, infeasdomain: bool, env: &DomainEnv) {
        let cutpool: &CutPool = env.cx.cutpool;
        let feastol = env.feastol;
        let var_type = env.model.integrality[col];
        if newbound < oldbound && threshold {
            let col_upper = self.col_upper[col];
            let thresholds = &mut self.cutpoolpropagation[pool].capacity_threshold;
            cutpool.matrix().for_each_negative_column_entry(col, |row, val| {
                update_threshold_lb_change(col_upper, newbound, val, &mut thresholds[row], feastol, var_type);
                true
            });
        }
        if !activity {
            return;
        }
        if !infeasdomain {
            let col_upper = self.col_upper[col];
            let mut found_infeasible = None;
            {
                let prop = &mut self.cutpoolpropagation[pool];
                cutpool.matrix().for_each_positive_column_entry(col, |row, val| {
                    let deltamin = compute_delta(val, oldbound, newbound, -INF, &mut prop.activitycutsinf[row]);
                    prop.activitycuts[row] += deltamin;
                    if deltamin <= 0.0 {
                        update_threshold_lb_change(col_upper, newbound, val, &mut prop.capacity_threshold[row], feastol, var_type);
                        return true;
                    }
                    if prop.activitycutsinf[row] == 0 && (prop.activitycuts[row] - cutpool.rhs()[row]).value() > feastol {
                        found_infeasible = Some(row);
                        return false;
                    }
                    // markPropagateCut, in visiting order.
                    if prop.propagatecutflags[row] == 0 && (prop.activitycutsinf[row] == 1 || cutpool.rhs()[row] - prop.activitycuts[row].value() <= prop.capacity_threshold[row]) {
                        prop.propagatecutinds.push(row);
                        prop.propagatecutflags[row] |= 1;
                    }
                    true
                });
            }
            if let Some(row) = found_infeasible {
                self.infeasible = true;
                self.infeasible_pos = self.domchgstack.len() as isize;
                self.infeasible_reason = Reason::cut(self.cutpoolpropagation[pool].cutpoolindex, row);
            }
        }
        if self.infeasible {
            let (old, new) = (newbound, oldbound);
            let reason = self.infeasible_reason;
            let poolindex = self.cutpoolpropagation[pool].cutpoolindex as i32;
            let prop = &mut self.cutpoolpropagation[pool];
            cutpool.matrix().for_each_positive_column_entry(col, |row, val| {
                let delta = compute_delta(val, old, new, -INF, &mut prop.activitycutsinf[row]);
                prop.activitycuts[row] += delta;
                !(reason.index == row && reason.kind == poolindex)
            });
        }
    }

    /// `CutpoolPropagation::updateActivityUbChange`.
    #[allow(clippy::too_many_arguments)]
    fn cutpool_update_activity_ub_change(&mut self, pool: usize, col: usize, oldbound: f64, newbound: f64, threshold: bool, activity: bool, infeasdomain: bool, env: &DomainEnv) {
        let cutpool: &CutPool = env.cx.cutpool;
        let feastol = env.feastol;
        let var_type = env.model.integrality[col];
        if newbound > oldbound && threshold {
            let col_lower = self.col_lower[col];
            let thresholds = &mut self.cutpoolpropagation[pool].capacity_threshold;
            cutpool.matrix().for_each_positive_column_entry(col, |row, val| {
                update_threshold_ub_change(col_lower, newbound, val, &mut thresholds[row], feastol, var_type);
                true
            });
        }
        if !activity {
            return;
        }
        if !infeasdomain {
            let col_lower = self.col_lower[col];
            let mut found_infeasible = None;
            {
                let prop = &mut self.cutpoolpropagation[pool];
                cutpool.matrix().for_each_negative_column_entry(col, |row, val| {
                    let deltamin = compute_delta(val, oldbound, newbound, INF, &mut prop.activitycutsinf[row]);
                    prop.activitycuts[row] += deltamin;
                    if deltamin <= 0.0 {
                        update_threshold_ub_change(col_lower, newbound, val, &mut prop.capacity_threshold[row], feastol, var_type);
                        return true;
                    }
                    if prop.activitycutsinf[row] == 0 && (prop.activitycuts[row] - cutpool.rhs()[row]).value() > feastol {
                        found_infeasible = Some(row);
                        return false;
                    }
                    if prop.propagatecutflags[row] == 0 && (prop.activitycutsinf[row] == 1 || cutpool.rhs()[row] - prop.activitycuts[row].value() <= prop.capacity_threshold[row]) {
                        prop.propagatecutinds.push(row);
                        prop.propagatecutflags[row] |= 1;
                    }
                    true
                });
            }
            if let Some(row) = found_infeasible {
                self.infeasible = true;
                self.infeasible_pos = self.domchgstack.len() as isize;
                self.infeasible_reason = Reason::cut(self.cutpoolpropagation[pool].cutpoolindex, row);
            }
        }
        if self.infeasible {
            let (old, new) = (newbound, oldbound);
            let reason = self.infeasible_reason;
            let poolindex = self.cutpoolpropagation[pool].cutpoolindex as i32;
            let prop = &mut self.cutpoolpropagation[pool];
            cutpool.matrix().for_each_negative_column_entry(col, |row, val| {
                let delta = compute_delta(val, old, new, INF, &mut prop.activitycutsinf[row]);
                prop.activitycuts[row] += delta;
                !(reason.index == row && reason.kind == poolindex)
            });
        }
    }

    // ---- conflict pool propagation --------------------------------------

    /// HiGHS's `linkWatchedLiteral`, faithfully: HiGHS only makes a literal
    /// the head of its column's list when the list is not empty, so the
    /// lists stay empty and watched literals never trigger propagation on
    /// a bound change.
    fn link_watched_literal(&mut self, pool: usize, link_pos: usize) {
        let prop = &mut self.conflict_pool_propagation[pool];
        let literal = prop.watched_literals[link_pos];
        let column = literal.domchg.column;
        let head = match literal.domchg.boundtype {
            BoundType::Lower => prop.col_lower_watched[column],
            BoundType::Upper => prop.col_upper_watched[column],
        };
        prop.watched_literals[link_pos].column = Some(column);
        prop.watched_literals[link_pos].prev = -1;
        prop.watched_literals[link_pos].next = head;
        if head != -1 {
            prop.watched_literals[head as usize].prev = link_pos as isize;
            match literal.domchg.boundtype {
                BoundType::Lower => prop.col_lower_watched[column] = link_pos as isize,
                BoundType::Upper => prop.col_upper_watched[column] = link_pos as isize,
            }
        }
    }

    fn unlink_watched_literal(&mut self, pool: usize, link_pos: usize) {
        let prop = &mut self.conflict_pool_propagation[pool];
        let literal = prop.watched_literals[link_pos];
        let Some(column) = literal.column else { return };
        prop.watched_literals[link_pos].column = None;
        let (prev, next) = (literal.prev, literal.next);
        if prev != -1 {
            prop.watched_literals[prev as usize].next = next;
        } else {
            match literal.domchg.boundtype {
                BoundType::Lower => prop.col_lower_watched[column] = next,
                BoundType::Upper => prop.col_upper_watched[column] = next,
            }
        }
        if next != -1 {
            prop.watched_literals[next as usize].prev = prev;
        }
    }

    fn conflict_deleted(&mut self, pool: usize, conflict: usize) {
        self.conflict_pool_propagation[pool].conflict_flag[conflict] |= 8;
        self.unlink_watched_literal(pool, 2 * conflict);
        self.unlink_watched_literal(pool, 2 * conflict + 1);
    }

    fn conflict_added(&mut self, pool: usize, conflict: usize, entries: &[DomainChange]) {
        {
            let prop = &mut self.conflict_pool_propagation[pool];
            if prop.conflict_flag.len() <= conflict {
                prop.watched_literals.resize(2 * conflict + 2, WatchedLiteral::default());
                prop.conflict_flag.resize(conflict + 1, 0);
            }
        }
        let mut num_watched = 0;
        for entry in entries {
            if self.is_active(entry) {
                continue;
            }
            let watch_pos = 2 * conflict + num_watched;
            self.conflict_pool_propagation[pool].watched_literals[watch_pos].domchg = *entry;
            self.link_watched_literal(pool, watch_pos);
            num_watched += 1;
            if num_watched == 2 {
                break;
            }
        }
        let bound_pos = |domain: &Domain, entry: &DomainChange| match entry.boundtype {
            BoundType::Lower => domain.col_lower_pos[entry.column],
            BoundType::Upper => domain.col_upper_pos[entry.column],
        };
        match num_watched {
            0 => {
                let mut latest_active = [(0isize, 0usize); 2];
                let mut num_active = 0;
                for (i, entry) in entries.iter().enumerate() {
                    let pos = bound_pos(self, entry);
                    match num_active {
                        0 => {
                            latest_active[0] = (pos, i);
                            num_active = 1;
                        }
                        1 => {
                            latest_active[1] = (pos, i);
                            num_active = 2;
                            if latest_active[0].0 < latest_active[1].0 {
                                latest_active.swap(0, 1);
                            }
                        }
                        _ => {
                            if pos > latest_active[1].0 {
                                latest_active[1] = (pos, i);
                                if latest_active[0].0 < latest_active[1].0 {
                                    latest_active.swap(0, 1);
                                }
                            }
                        }
                    }
                }
                for (i, &(_, at)) in latest_active.iter().enumerate().take(num_active) {
                    let watch_pos = 2 * conflict + i;
                    self.conflict_pool_propagation[pool].watched_literals[watch_pos].domchg = entries[at];
                    self.link_watched_literal(pool, watch_pos);
                }
            }
            1 => {
                let mut latest_active = None;
                let mut latest_pos = -1;
                for (i, entry) in entries.iter().enumerate() {
                    let pos = bound_pos(self, entry);
                    if pos > latest_pos {
                        latest_active = Some(i);
                        latest_pos = pos;
                    }
                }
                if let Some(at) = latest_active {
                    let watch_pos = 2 * conflict + 1;
                    self.conflict_pool_propagation[pool].watched_literals[watch_pos].domchg = entries[at];
                    self.link_watched_literal(pool, watch_pos);
                }
            }
            _ => {}
        }
        let prop = &mut self.conflict_pool_propagation[pool];
        prop.conflict_flag[conflict] = num_watched as u8 | (prop.conflict_flag[conflict] & 4);
        self.mark_propagate_conflict(pool, conflict);
    }

    fn mark_propagate_conflict(&mut self, pool: usize, conflict: usize) {
        let prop = &mut self.conflict_pool_propagation[pool];
        if prop.conflict_flag[conflict] < 2 {
            prop.propagate_conflict_inds.push(conflict);
            prop.conflict_flag[conflict] |= 4;
        }
    }

    /// `ConflictPoolPropagation::updateActivityLbChange`.
    fn conflict_update_activity_lb_change(&mut self, pool: usize, col: usize, oldbound: f64, newbound: f64) {
        let mut i = self.conflict_pool_propagation[pool].col_lower_watched[col];
        while i != -1 {
            let prop = &self.conflict_pool_propagation[pool];
            let literal = prop.watched_literals[i as usize];
            let conflict = (i >> 1) as usize;
            let delta = u8::from(literal.domchg.boundval > newbound).wrapping_sub(u8::from(literal.domchg.boundval > oldbound));
            if delta != 0 {
                let prop = &mut self.conflict_pool_propagation[pool];
                prop.conflict_flag[conflict] = prop.conflict_flag[conflict].wrapping_add(delta);
                self.mark_propagate_conflict(pool, conflict);
            }
            i = literal.next;
        }
    }

    /// `ConflictPoolPropagation::updateActivityUbChange`.
    fn conflict_update_activity_ub_change(&mut self, pool: usize, col: usize, oldbound: f64, newbound: f64) {
        let mut i = self.conflict_pool_propagation[pool].col_upper_watched[col];
        while i != -1 {
            let prop = &self.conflict_pool_propagation[pool];
            let literal = prop.watched_literals[i as usize];
            let conflict = (i >> 1) as usize;
            let delta = u8::from(literal.domchg.boundval < newbound).wrapping_sub(u8::from(literal.domchg.boundval < oldbound));
            if delta != 0 {
                let prop = &mut self.conflict_pool_propagation[pool];
                prop.conflict_flag[conflict] = prop.conflict_flag[conflict].wrapping_add(delta);
                self.mark_propagate_conflict(pool, conflict);
            }
            i = literal.next;
        }
    }

    fn propagate_conflict(&mut self, pool: usize, conflict: usize, env: &mut DomainEnv) {
        {
            let prop = &mut self.conflict_pool_propagation[pool];
            prop.conflict_flag[conflict] &= 3 | 8;
            if prop.conflict_flag[conflict] >= 2 {
                return;
            }
        }
        if self.infeasible {
            return;
        }
        let Some(entries) = env.cx.conflictpool.conflict(conflict).map(<[DomainChange]>::to_vec) else {
            self.unlink_watched_literal(pool, 2 * conflict);
            self.unlink_watched_literal(pool, 2 * conflict + 1);
            return;
        };
        let mut inactive = [0usize; 2];
        let mut num_inactive = 0;
        for (i, entry) in entries.iter().enumerate() {
            if self.is_active(entry) {
                continue;
            }
            inactive[num_inactive] = i;
            num_inactive += 1;
            if num_inactive == 2 {
                break;
            }
        }
        self.conflict_pool_propagation[pool].conflict_flag[conflict] = num_inactive as u8;
        let reason_kind = self.cutpoolpropagation.len() + self.conflict_pool_propagation[pool].conflictpoolindex;
        match num_inactive {
            0 => {
                self.infeasible = true;
                self.infeasible_reason = Reason::cut(reason_kind, conflict);
                self.infeasible_pos = self.domchgstack.len() as isize;
                env.cx.conflictpool.reset_age(conflict);
            }
            1 => {
                let domchg = self.flip(&entries[inactive[0]], env.cx);
                if !self.is_active(&domchg) {
                    self.change_bound(domchg, Reason::cut(reason_kind, conflict), env);
                    env.cx.conflictpool.reset_age(conflict);
                }
            }
            _ => {
                let watched0 = self.conflict_pool_propagation[pool].watched_literals[2 * conflict].domchg;
                if watched0 != entries[inactive[0]] {
                    self.unlink_watched_literal(pool, 2 * conflict);
                    self.conflict_pool_propagation[pool].watched_literals[2 * conflict].domchg = entries[inactive[0]];
                    self.link_watched_literal(pool, 2 * conflict);
                }
                let watched1 = self.conflict_pool_propagation[pool].watched_literals[2 * conflict + 1].domchg;
                if watched1 != entries[inactive[1]] {
                    self.unlink_watched_literal(pool, 2 * conflict + 1);
                    self.conflict_pool_propagation[pool].watched_literals[2 * conflict + 1].domchg = entries[inactive[1]];
                    self.link_watched_literal(pool, 2 * conflict + 1);
                }
            }
        }
    }
}

fn update_threshold_lb_change(col_upper: f64, newbound: f64, val: f64, threshold: &mut f64, feastol: f64, var_type: VarType) {
    if newbound != col_upper {
        let threshold_new = val.abs() * bound_range(col_upper, newbound, feastol, var_type);
        *threshold = max3(*threshold, threshold_new, feastol);
    }
}

fn update_threshold_ub_change(col_lower: f64, newbound: f64, val: f64, threshold: &mut f64, feastol: f64, var_type: VarType) {
    if newbound != col_lower {
        let threshold_new = val.abs() * bound_range(newbound, col_lower, feastol, var_type);
        *threshold = max3(*threshold, threshold_new, feastol);
    }
}

// ---- objective propagation ----------------------------------------------

impl Domain {
    /// HiGHS's `setupObjectivePropagation` (`ObjectivePropagation(domain)`).
    pub fn setup_objective_propagation(&mut self, env: &DomainEnv) {
        let objective = env.objective;
        let cost = &env.model.col_cost;
        let obj_nonzeros = objective.objective_nonzeros();
        let partition_starts = objective.clique_partition_starts();
        let num_partitions = objective.num_clique_partitions();
        let mut prop = ObjectivePropagation {
            active: true,
            objective_lower: CDouble::new(0.0),
            num_inf_obj_lower: 0,
            capacity_threshold: INF,
            is_propagated: false,
            objective_lower_contributions: vec![ObjectiveContribution::default(); partition_starts[num_partitions]],
            contribution_partition_sets: vec![(NO_LINK, NO_LINK); num_partitions],
            propagation_cons_buffer: Vec::new(),
            partition_clique_data: Vec::new(),
        };
        if num_partitions != 0 {
            prop.propagation_cons_buffer = objective.objective_values_packed().to_vec();
            prop.partition_clique_data = vec![PartitionCliqueData::default(); num_partitions];
        }
        // In each clique partition, every column first counts with its
        // larger contribution; then the largest one is taken back out.
        for i in 0..num_partitions {
            prop.partition_clique_data[i].rhs = 1;
            for j in partition_starts[i]..partition_starts[i + 1] {
                let col = obj_nonzeros[j];
                prop.objective_lower_contributions[j].col = col;
                prop.objective_lower_contributions[j].partition = i;
                if cost[col] > 0.0 {
                    prop.objective_lower += cost[col];
                    prop.objective_lower_contributions[j].contribution = cost[col];
                    prop.partition_clique_data[i].rhs -= 1;
                    if self.col_lower[col] == 0.0 {
                        prop.tree(i).link(j);
                    }
                } else {
                    prop.objective_lower_contributions[j].contribution = -cost[col];
                    if self.col_upper[col] == 1.0 {
                        prop.tree(i).link(j);
                    }
                }
            }
            let worst_pos = prop.tree_first(i);
            if worst_pos != NO_LINK {
                prop.objective_lower -= prop.objective_lower_contributions[worst_pos].contribution;
            }
        }
        for &col in &obj_nonzeros[partition_starts[num_partitions]..] {
            if cost[col] > 0.0 {
                if self.col_lower[col] == -INF {
                    prop.num_inf_obj_lower += 1;
                } else {
                    prop.objective_lower += self.col_lower[col] * cost[col];
                }
            } else if self.col_upper[col] == INF {
                prop.num_inf_obj_lower += 1;
            } else {
                prop.objective_lower += self.col_upper[col] * cost[col];
            }
        }
        self.obj_prop = prop;
        self.obj_recompute_capacity_threshold(env);
    }

    /// HiGHS's `getPropagationConstraint`: the objective cutoff row as the
    /// stack stood at `domchg_stack_size`, with each clique partition's
    /// largest contribution (other than `domchg_col`'s) folded in.
    fn objective_propagation_constraint(&mut self, domchg_stack_size: isize, domchg_col: Option<usize>, env: &DomainEnv) -> (Vec<usize>, Vec<f64>, f64) {
        let objective = env.objective;
        let cost = &env.model.col_cost;
        let num_partitions = objective.num_clique_partitions();
        let inds = objective.objective_nonzeros().to_vec();
        if num_partitions == 0 {
            return (inds, objective.objective_values_packed().to_vec(), env.upper_limit);
        }
        let partition_starts = objective.clique_partition_starts();
        let mut tmp_rhs = CDouble::new(env.upper_limit);
        for i in 0..num_partitions {
            let (start, end) = (partition_starts[i], partition_starts[i + 1]);
            let mut largest = 0.0;
            for &c in &inds[start..end] {
                if Some(c) == domchg_col {
                    continue;
                }
                if cost[c] > 0.0 {
                    let (lb, _) = self.col_lower_pos(c, domchg_stack_size);
                    if lb < 1.0 {
                        largest = cmax(largest, cost[c]);
                    }
                } else {
                    let (ub, _) = self.col_upper_pos(c, domchg_stack_size);
                    if ub > 0.0 {
                        largest = cmax(largest, -cost[c]);
                    }
                }
            }
            tmp_rhs += largest * self.obj_prop.partition_clique_data[i].rhs as f64;
            if self.obj_prop.partition_clique_data[i].multiplier != largest {
                self.obj_prop.partition_clique_data[i].multiplier = largest;
                let packed_obj_vals = objective.objective_values_packed();
                for j in start..end {
                    self.obj_prop.propagation_cons_buffer[j] = packed_obj_vals[j] - largest.copysign(packed_obj_vals[j]);
                }
            }
        }
        (inds, self.obj_prop.propagation_cons_buffer.clone(), tmp_rhs.value())
    }

    /// HiGHS's `getCutoffConstraint`.
    pub fn cutoff_constraint(&mut self, env: &DomainEnv) -> (Vec<usize>, Vec<f64>, f64) {
        self.objective_propagation_constraint(self.domchgstack.len() as isize, None, env)
    }

    fn obj_recompute_capacity_threshold(&mut self, env: &DomainEnv) {
        let objective = env.objective;
        let cost = &env.model.col_cost;
        let feastol = env.feastol;
        let partition_starts = objective.clique_partition_starts();
        let num_partitions = objective.num_clique_partitions();
        let mut threshold = -feastol;
        for i in 0..num_partitions {
            let worst_pos = self.obj_prop.tree_first(i);
            if worst_pos == NO_LINK {
                continue;
            }
            if self.is_fixed(self.obj_prop.objective_lower_contributions[worst_pos].col) {
                continue;
            }
            let mut contribution = self.obj_prop.objective_lower_contributions[worst_pos].contribution;
            let best_pos = self.obj_prop.tree(i).last();
            if best_pos != worst_pos {
                contribution -= self.obj_prop.objective_lower_contributions[best_pos].contribution;
            }
            threshold = cmax(threshold, contribution * (1.0 - feastol));
        }
        for &col in &objective.objective_nonzeros()[partition_starts[num_partitions]..] {
            threshold = cmax(
                threshold,
                cost[col].abs() * bound_range(self.col_upper[col], self.col_lower[col], feastol, env.model.integrality[col]),
            );
        }
        self.obj_prop.capacity_threshold = threshold;
    }

    fn obj_update_activity_lb_change(&mut self, col: usize, oldbound: f64, newbound: f64, env: &DomainEnv) {
        let cost = env.model.col_cost[col];
        let feastol = env.feastol;
        let var_type = env.model.integrality[col];
        if cost <= 0.0 {
            if cost != 0.0 && newbound < oldbound {
                self.obj_prop.capacity_threshold = cmax(self.obj_prop.capacity_threshold, -cost * bound_range(self.col_upper[col], newbound, feastol, var_type));
                self.obj_prop.is_propagated = false;
            }
            return;
        }
        self.obj_prop.is_propagated = false;
        match env.objective.col_clique_partition(col) {
            None => {
                if oldbound == -INF {
                    self.obj_prop.num_inf_obj_lower -= 1;
                } else {
                    self.obj_prop.objective_lower -= oldbound * cost;
                }
                if newbound == -INF {
                    self.obj_prop.num_inf_obj_lower += 1;
                } else {
                    self.obj_prop.objective_lower += newbound * cost;
                }
                if newbound < oldbound {
                    self.obj_prop.capacity_threshold = cmax(
                        self.obj_prop.capacity_threshold,
                        cost * bound_range(self.col_upper[col], self.col_lower[col], feastol, var_type),
                    );
                } else if self.obj_prop.num_inf_obj_lower == 0 && self.obj_prop.objective_lower > env.upper_limit {
                    self.infeasible = true;
                    self.infeasible_pos = self.domchgstack.len() as isize;
                    self.infeasible_reason = Reason::objective();
                    self.obj_update_activity_lb_change(col, newbound, oldbound, env);
                }
            }
            Some(partition_pos) => {
                let partition = self.obj_prop.objective_lower_contributions[partition_pos].partition;
                if newbound == 0.0 {
                    // A binary's lower bound relaxed to 0.
                    let curr_first = self.obj_prop.tree_first(partition);
                    self.obj_prop.tree(partition).link(partition_pos);
                    let old_contribution = if curr_first != NO_LINK {
                        self.obj_prop.objective_lower_contributions[curr_first].contribution
                    } else {
                        0.0
                    };
                    let contribution = self.obj_prop.objective_lower_contributions[partition_pos].contribution;
                    if partition_pos == self.obj_prop.tree_first(partition) && contribution != old_contribution {
                        self.obj_prop.objective_lower += old_contribution;
                        self.obj_prop.objective_lower -= contribution;
                        let best_pos = self.obj_prop.tree(partition).last();
                        let mut delta = contribution;
                        if best_pos != partition_pos {
                            delta -= self.obj_prop.objective_lower_contributions[best_pos].contribution;
                        }
                        self.obj_prop.capacity_threshold = cmax(delta * (1.0 - feastol), self.obj_prop.capacity_threshold);
                    } else {
                        self.obj_prop.capacity_threshold = cmax((old_contribution - contribution) * (1.0 - feastol), self.obj_prop.capacity_threshold);
                    }
                } else {
                    // A binary's lower bound tightened to 1.
                    let was_first = self.obj_prop.tree_first(partition) == partition_pos;
                    if was_first {
                        self.obj_prop.objective_lower += self.obj_prop.objective_lower_contributions[partition_pos].contribution;
                    }
                    self.obj_prop.tree(partition).unlink(partition_pos);
                    if was_first {
                        let new_worst = self.obj_prop.tree_first(partition);
                        if new_worst != NO_LINK {
                            self.obj_prop.objective_lower -= self.obj_prop.objective_lower_contributions[new_worst].contribution;
                        }
                    }
                    if self.obj_prop.num_inf_obj_lower == 0 && self.obj_prop.objective_lower > env.upper_limit {
                        self.infeasible = true;
                        self.infeasible_pos = self.domchgstack.len() as isize;
                        self.infeasible_reason = Reason::objective();
                        self.obj_update_activity_lb_change(col, newbound, oldbound, env);
                    }
                }
            }
        }
    }

    fn obj_update_activity_ub_change(&mut self, col: usize, oldbound: f64, newbound: f64, env: &DomainEnv) {
        let cost = env.model.col_cost[col];
        let feastol = env.feastol;
        let var_type = env.model.integrality[col];
        if cost >= 0.0 {
            if cost != 0.0 && newbound > oldbound {
                self.obj_prop.capacity_threshold = cmax(self.obj_prop.capacity_threshold, cost * bound_range(newbound, self.col_lower[col], feastol, var_type));
                self.obj_prop.is_propagated = false;
            }
            return;
        }
        self.obj_prop.is_propagated = false;
        match env.objective.col_clique_partition(col) {
            None => {
                if oldbound == INF {
                    self.obj_prop.num_inf_obj_lower -= 1;
                } else {
                    self.obj_prop.objective_lower -= oldbound * cost;
                }
                if newbound == INF {
                    self.obj_prop.num_inf_obj_lower += 1;
                } else {
                    self.obj_prop.objective_lower += newbound * cost;
                }
                if newbound > oldbound {
                    self.obj_prop.capacity_threshold = cmax(
                        self.obj_prop.capacity_threshold,
                        -cost * bound_range(self.col_upper[col], self.col_lower[col], feastol, var_type),
                    );
                } else if self.obj_prop.num_inf_obj_lower == 0 && self.obj_prop.objective_lower > env.upper_limit {
                    self.infeasible = true;
                    self.infeasible_pos = self.domchgstack.len() as isize;
                    self.infeasible_reason = Reason::objective();
                    self.obj_update_activity_ub_change(col, newbound, oldbound, env);
                }
            }
            Some(partition_pos) => {
                let partition = self.obj_prop.objective_lower_contributions[partition_pos].partition;
                if newbound == 1.0 {
                    // A binary's upper bound relaxed to 1.
                    let curr_first = self.obj_prop.tree_first(partition);
                    self.obj_prop.tree(partition).link(partition_pos);
                    let old_contribution = if curr_first != NO_LINK {
                        self.obj_prop.objective_lower_contributions[curr_first].contribution
                    } else {
                        0.0
                    };
                    let contribution = self.obj_prop.objective_lower_contributions[partition_pos].contribution;
                    if partition_pos == self.obj_prop.tree_first(partition) && contribution != old_contribution {
                        self.obj_prop.objective_lower += old_contribution;
                        self.obj_prop.objective_lower -= contribution;
                        let best_pos = self.obj_prop.tree(partition).last();
                        let mut delta = contribution;
                        if best_pos != partition_pos {
                            delta -= self.obj_prop.objective_lower_contributions[best_pos].contribution;
                        }
                        self.obj_prop.capacity_threshold = cmax(delta * (1.0 - feastol), self.obj_prop.capacity_threshold);
                    } else {
                        self.obj_prop.capacity_threshold = cmax((old_contribution - contribution) * (1.0 - feastol), self.obj_prop.capacity_threshold);
                    }
                } else {
                    // A binary's upper bound tightened to 0.
                    let was_first = self.obj_prop.tree_first(partition) == partition_pos;
                    if was_first {
                        self.obj_prop.objective_lower += self.obj_prop.objective_lower_contributions[partition_pos].contribution;
                    }
                    self.obj_prop.tree(partition).unlink(partition_pos);
                    if was_first {
                        let new_worst = self.obj_prop.tree_first(partition);
                        if new_worst != NO_LINK {
                            self.obj_prop.objective_lower -= self.obj_prop.objective_lower_contributions[new_worst].contribution;
                        }
                    }
                    if self.obj_prop.num_inf_obj_lower == 0 && self.obj_prop.objective_lower > env.upper_limit {
                        self.infeasible = true;
                        self.infeasible_pos = self.domchgstack.len() as isize;
                        self.infeasible_reason = Reason::objective();
                        self.obj_update_activity_ub_change(col, newbound, oldbound, env);
                    }
                }
            }
        }
    }

    fn obj_should_be_propagated(&self, upper_limit: f64) -> bool {
        if self.obj_prop.is_propagated || self.obj_prop.num_inf_obj_lower > 1 || self.infeasible {
            return false;
        }
        if upper_limit == INF {
            return false;
        }
        upper_limit - self.obj_prop.objective_lower.value() <= self.obj_prop.capacity_threshold
    }

    fn obj_propagate(&mut self, env: &mut DomainEnv) {
        let upper_limit = env.upper_limit;
        if !self.obj_should_be_propagated(upper_limit) {
            return;
        }
        if self.obj_prop.num_inf_obj_lower == 0 && self.obj_prop.objective_lower > upper_limit {
            self.infeasible = true;
            self.infeasible_pos = self.domchgstack.len() as isize;
            self.infeasible_reason = Reason::objective();
            return;
        }
        let objective = env.objective;
        let model = env.model;
        let cost = &model.col_cost;
        let obj_nonzeros = objective.objective_nonzeros();
        let mut capacity = CDouble::new(upper_limit) - self.obj_prop.objective_lower;
        if self.obj_prop.num_inf_obj_lower == 1 {
            // Only the one column with an infinite contribution can move.
            for &col in &obj_nonzeros[objective.num_binaries_in_objective()..] {
                if (cost[col] > 0.0 && self.col_lower[col] != -INF) || (cost[col] < 0.0 && self.col_upper[col] != INF) {
                    continue;
                }
                let boundtype = if cost[col] > 0.0 { BoundType::Upper } else { BoundType::Lower };
                self.check_change_bound(boundtype, col, capacity / cost[col], Reason::objective(), env);
                break;
            }
        } else {
            let num_partitions = objective.num_clique_partitions();
            loop {
                let mut num_bound_changes = 0;
                for i in 0..num_partitions {
                    let worst = self.obj_prop.tree_first(i);
                    if worst == NO_LINK {
                        continue;
                    }
                    let mut contribution = self.obj_prop.objective_lower_contributions[worst].contribution;
                    let second_worst = self.obj_prop.tree(i).successor(worst);
                    if second_worst != NO_LINK {
                        contribution -= self.obj_prop.objective_lower_contributions[second_worst].contribution;
                    }
                    // The upper limit has its tolerance already: cut off hard.
                    if contribution > capacity.value() {
                        let col = self.obj_prop.objective_lower_contributions[worst].col;
                        if cost[col] > 0.0 {
                            if self.col_upper[col] > 0.0 {
                                num_bound_changes += 1;
                                self.change_bound(DomainChange::new(0.0, col, BoundType::Upper), Reason::objective(), env);
                                if self.infeasible {
                                    break;
                                }
                            }
                        } else if self.col_lower[col] < 1.0 {
                            num_bound_changes += 1;
                            self.change_bound(DomainChange::new(1.0, col, BoundType::Lower), Reason::objective(), env);
                            if self.infeasible {
                                break;
                            }
                        }
                    } else if second_worst != NO_LINK {
                        // The column with the least contribution may be fixed
                        // where it contributes most.
                        let mut best = self.obj_prop.tree(i).last();
                        while best != self.obj_prop.tree_first(i) {
                            let first = self.obj_prop.tree_first(i);
                            if self.obj_prop.objective_lower_contributions[first].contribution - self.obj_prop.objective_lower_contributions[best].contribution > capacity.value() {
                                let col = self.obj_prop.objective_lower_contributions[best].col;
                                num_bound_changes += 1;
                                if cost[col] > 0.0 {
                                    self.change_bound(DomainChange::new(1.0, col, BoundType::Lower), Reason::objective(), env);
                                } else {
                                    self.change_bound(DomainChange::new(0.0, col, BoundType::Upper), Reason::objective(), env);
                                }
                                if self.infeasible {
                                    break;
                                }
                            } else {
                                break;
                            }
                            best = self.obj_prop.tree(i).last();
                        }
                        if self.infeasible {
                            break;
                        }
                    }
                }
                if self.infeasible {
                    break;
                }
                for &col in &obj_nonzeros[objective.clique_partition_starts()[num_partitions]..] {
                    let changed = if cost[col] > 0.0 {
                        let bound = (capacity + self.col_lower[col] * cost[col]) / cost[col];
                        self.check_change_bound(BoundType::Upper, col, bound, Reason::objective(), env)
                    } else {
                        let bound = (capacity + self.col_upper[col] * cost[col]) / cost[col];
                        self.check_change_bound(BoundType::Lower, col, bound, Reason::objective(), env)
                    };
                    if changed {
                        num_bound_changes += 1;
                    }
                    if self.infeasible {
                        break;
                    }
                }
                if self.infeasible || num_bound_changes == 0 {
                    break;
                }
                capacity = CDouble::new(upper_limit) - self.obj_prop.objective_lower;
            }
        }
        self.obj_recompute_capacity_threshold(env);
        self.obj_prop.is_propagated = true;
    }
}

// ---- activities, bound changes and propagation --------------------------

impl Domain {
    /// HiGHS's `computeMinActivity` over `index`/`value`: the infinite
    /// contributions and the finite sum. In an infeasible domain, the bounds
    /// before the infeasibility are used.
    pub fn compute_min_activity(&self, index: &[usize], value: &[f64]) -> (i32, CDouble) {
        let mut activity = CDouble::new(0.0);
        let mut ninf = 0;
        for (&col, &val) in index.iter().zip(value) {
            let (lb, ub) = if self.infeasible {
                (self.col_lower_pos(col, self.infeasible_pos - 1).0, self.col_upper_pos(col, self.infeasible_pos - 1).0)
            } else {
                (self.col_lower[col], self.col_upper[col])
            };
            let contribution = activity_contribution_min(val, lb, ub);
            if contribution == -INF {
                ninf += 1;
            } else {
                activity += contribution;
            }
        }
        activity.renormalize();
        (ninf, activity)
    }

    /// HiGHS's `computeMaxActivity`.
    pub fn compute_max_activity(&self, index: &[usize], value: &[f64]) -> (i32, CDouble) {
        let mut activity = CDouble::new(0.0);
        let mut ninf = 0;
        for (&col, &val) in index.iter().zip(value) {
            let (lb, ub) = if self.infeasible {
                (self.col_lower_pos(col, self.infeasible_pos - 1).0, self.col_upper_pos(col, self.infeasible_pos - 1).0)
            } else {
                (self.col_lower[col], self.col_upper[col])
            };
            let contribution = activity_contribution_max(val, lb, ub);
            if contribution == INF {
                ninf += 1;
            } else {
                activity += contribution;
            }
        }
        activity.renormalize();
        (ninf, activity)
    }

    /// HiGHS's `adjustedUb`: the new upper bound `bound_val` suggests, and
    /// whether it improves enough to accept.
    pub fn adjusted_ub(&self, col: usize, bound_val: CDouble, cx: &MipCtx) -> (f64, bool) {
        let feastol = cx.feastol;
        if cx.model.is_col_integral(col) {
            let bound = (bound_val + feastol).floor().value();
            let accept = bound < self.col_upper[col] && self.col_upper[col] - bound > 1000.0 * feastol * bound.abs();
            (bound, accept)
        } else {
            let bound = if (bound_val.value() - self.col_lower[col]).abs() <= cx.epsilon {
                self.col_lower[col]
            } else {
                bound_val.value()
            };
            let accept = if self.col_upper[col] == INF {
                true
            } else if bound + 1000.0 * feastol < self.col_upper[col] {
                let mut relative_improve = self.col_upper[col] - bound;
                if self.col_lower[col] != -INF {
                    relative_improve /= self.col_upper[col] - self.col_lower[col];
                } else {
                    relative_improve /= cmax(self.col_upper[col].abs(), bound.abs());
                }
                relative_improve >= 0.3
            } else {
                false
            };
            (bound, accept)
        }
    }

    /// HiGHS's `adjustedLb`.
    pub fn adjusted_lb(&self, col: usize, bound_val: CDouble, cx: &MipCtx) -> (f64, bool) {
        let feastol = cx.feastol;
        if cx.model.is_col_integral(col) {
            let bound = (bound_val - feastol).ceil().value();
            let accept = bound > self.col_lower[col] && bound - self.col_lower[col] > 1000.0 * feastol * bound.abs();
            (bound, accept)
        } else {
            let bound = if (self.col_upper[col] - bound_val.value()).abs() <= cx.epsilon {
                self.col_upper[col]
            } else {
                bound_val.value()
            };
            let accept = if self.col_lower[col] == -INF {
                true
            } else if bound - 1000.0 * feastol > self.col_lower[col] {
                let mut relative_improve = bound - self.col_lower[col];
                if self.col_upper[col] != INF {
                    relative_improve /= self.col_upper[col] - self.col_lower[col];
                } else {
                    relative_improve /= cmax(self.col_lower[col].abs(), bound.abs());
                }
                relative_improve >= 0.3
            } else {
                false
            };
            (bound, accept)
        }
    }

    /// HiGHS's `propagateRowUpper`: the bound changes the row `<= rupper`
    /// implies, appended to `boundchgs`; returns how many.
    #[allow(clippy::too_many_arguments)]
    pub fn propagate_row_upper(&self, rindex: &[usize], rvalue: &[f64], rupper: f64, minactivity: CDouble, ninfmin: i32, boundchgs: &mut Vec<DomainChange>, cx: &MipCtx) -> usize {
        if ninfmin > 1 {
            return 0;
        }
        let mut numchgs = 0;
        for (&col, &val) in rindex.iter().zip(rvalue) {
            let actcontribution = activity_contribution_min(val, self.col_lower[col], self.col_upper[col]);
            let minresact = if ninfmin == 1 {
                if actcontribution != -INF {
                    continue;
                }
                minactivity
            } else {
                minactivity - CDouble::new(actcontribution)
            };
            let bound_val = (CDouble::new(rupper) - minresact) / val;
            if (bound_val.value() * super::TINY).abs() > cx.feastol {
                continue;
            }
            if val > 0.0 {
                let (bound, accept) = self.adjusted_ub(col, bound_val, cx);
                if accept {
                    boundchgs.push(DomainChange::new(bound, col, BoundType::Upper));
                    numchgs += 1;
                }
            } else {
                let (bound, accept) = self.adjusted_lb(col, bound_val, cx);
                if accept {
                    boundchgs.push(DomainChange::new(bound, col, BoundType::Lower));
                    numchgs += 1;
                }
            }
        }
        numchgs
    }

    /// HiGHS's `propagateRowLower`.
    #[allow(clippy::too_many_arguments)]
    pub fn propagate_row_lower(&self, rindex: &[usize], rvalue: &[f64], rlower: f64, maxactivity: CDouble, ninfmax: i32, boundchgs: &mut Vec<DomainChange>, cx: &MipCtx) -> usize {
        if ninfmax > 1 {
            return 0;
        }
        let mut numchgs = 0;
        for (&col, &val) in rindex.iter().zip(rvalue) {
            let actcontribution = activity_contribution_max(val, self.col_lower[col], self.col_upper[col]);
            let maxresact = if ninfmax == 1 {
                if actcontribution != INF {
                    continue;
                }
                maxactivity
            } else {
                maxactivity - CDouble::new(actcontribution)
            };
            let bound_val = (CDouble::new(rlower) - maxresact) / val;
            if (bound_val.value() * super::TINY).abs() > cx.feastol {
                continue;
            }
            if val < 0.0 {
                let (bound, accept) = self.adjusted_ub(col, bound_val, cx);
                if accept {
                    boundchgs.push(DomainChange::new(bound, col, BoundType::Upper));
                    numchgs += 1;
                }
            } else {
                let (bound, accept) = self.adjusted_lb(col, bound_val, cx);
                if accept {
                    boundchgs.push(DomainChange::new(bound, col, BoundType::Lower));
                    numchgs += 1;
                }
            }
        }
        numchgs
    }

    fn update_activity_lb_change(&mut self, col: usize, oldbound: f64, newbound: f64, env: &mut DomainEnv) {
        if self.obj_prop.active {
            self.obj_update_activity_lb_change(col, oldbound, newbound, env);
            if self.infeasible {
                return;
            }
        }
        let model = env.model;
        let feastol = env.feastol;
        let start = model.a_start[col];
        let mut end = model.a_start[col + 1];
        let var_type = model.integrality[col];
        for i in start..end {
            let row = model.a_index[i];
            let val = model.a_value[i];
            if val > 0.0 {
                let deltamin = compute_delta(val, oldbound, newbound, -INF, &mut self.activitymininf[row]);
                self.activitymin[row] += deltamin;
                if self.record_redundant_rows && model.row_lower[row] != -INF && model.row_upper[row] == INF {
                    self.update_redundant_rows(row, model, feastol);
                }
                if deltamin <= 0.0 {
                    update_threshold_lb_change(self.col_upper[col], newbound, val, &mut self.capacity_threshold[row], feastol, var_type);
                    continue;
                }
                if model.row_upper[row] != INF && self.activitymininf[row] == 0 && (self.activitymin[row] - model.row_upper[row]).value() > feastol {
                    self.infeasible = true;
                    self.infeasible_pos = self.domchgstack.len() as isize;
                    self.infeasible_reason = Reason::model_row_upper(row);
                    end = i + 1;
                    break;
                }
                if self.activitymininf[row] <= 1 && !self.propagateflags[row] && model.row_upper[row] != INF {
                    self.mark_propagate(row, model, feastol);
                }
            } else {
                let deltamax = compute_delta(val, oldbound, newbound, -INF, &mut self.activitymaxinf[row]);
                self.activitymax[row] += deltamax;
                if self.record_redundant_rows && model.row_lower[row] == -INF && model.row_upper[row] != INF {
                    self.update_redundant_rows(row, model, feastol);
                }
                if deltamax >= 0.0 {
                    update_threshold_lb_change(self.col_upper[col], newbound, val, &mut self.capacity_threshold[row], feastol, var_type);
                    continue;
                }
                if model.row_lower[row] != -INF && self.activitymaxinf[row] == 0 && (CDouble::new(model.row_lower[row]) - self.activitymax[row]).value() > feastol {
                    self.infeasible = true;
                    self.infeasible_pos = self.domchgstack.len() as isize;
                    self.infeasible_reason = Reason::model_row_lower(row);
                    end = i + 1;
                    break;
                }
                if self.activitymaxinf[row] <= 1 && !self.propagateflags[row] && model.row_lower[row] != -INF {
                    self.mark_propagate(row, model, feastol);
                }
            }
        }
        if !self.infeasible {
            // If one pool finds infeasibility, the pools after it still update
            // their thresholds, and those before it take their activity
            // changes back.
            let mut infeascutpool = None;
            for pool in 0..self.cutpoolpropagation.len() {
                if !self.infeasible {
                    self.cutpool_update_activity_lb_change(pool, col, oldbound, newbound, true, true, self.infeasible, env);
                    if self.infeasible {
                        infeascutpool = Some(pool);
                    }
                } else {
                    self.cutpool_update_activity_lb_change(pool, col, oldbound, newbound, true, false, self.infeasible, env);
                }
            }
            for pool in 0..infeascutpool.unwrap_or(0) {
                self.cutpool_update_activity_lb_change(pool, col, oldbound, newbound, false, true, self.infeasible, env);
            }
        }
        if self.infeasible {
            let (oldbound, newbound) = (newbound, oldbound);
            for i in start..end {
                let row = model.a_index[i];
                let val = model.a_value[i];
                if val > 0.0 {
                    let delta = compute_delta(val, oldbound, newbound, -INF, &mut self.activitymininf[row]);
                    self.activitymin[row] += delta;
                } else {
                    let delta = compute_delta(val, oldbound, newbound, -INF, &mut self.activitymaxinf[row]);
                    self.activitymax[row] += delta;
                }
            }
            if self.obj_prop.active {
                self.obj_update_activity_lb_change(col, oldbound, newbound, env);
            }
        } else {
            for pool in 0..self.conflict_pool_propagation.len() {
                self.conflict_update_activity_lb_change(pool, col, oldbound, newbound);
            }
        }
    }

    fn update_activity_ub_change(&mut self, col: usize, oldbound: f64, newbound: f64, env: &mut DomainEnv) {
        if self.obj_prop.active {
            self.obj_update_activity_ub_change(col, oldbound, newbound, env);
            if self.infeasible {
                return;
            }
        }
        let model = env.model;
        let feastol = env.feastol;
        let start = model.a_start[col];
        let mut end = model.a_start[col + 1];
        let var_type = model.integrality[col];
        for i in start..end {
            let row = model.a_index[i];
            let val = model.a_value[i];
            if val > 0.0 {
                let deltamax = compute_delta(val, oldbound, newbound, INF, &mut self.activitymaxinf[row]);
                self.activitymax[row] += deltamax;
                if self.record_redundant_rows && model.row_lower[row] == -INF && model.row_upper[row] != INF {
                    self.update_redundant_rows(row, model, feastol);
                }
                if deltamax >= 0.0 {
                    update_threshold_ub_change(self.col_lower[col], newbound, val, &mut self.capacity_threshold[row], feastol, var_type);
                    continue;
                }
                if model.row_lower[row] != -INF && self.activitymaxinf[row] == 0 && (CDouble::new(model.row_lower[row]) - self.activitymax[row]).value() > feastol {
                    self.infeasible = true;
                    self.infeasible_pos = self.domchgstack.len() as isize;
                    self.infeasible_reason = Reason::model_row_lower(row);
                    end = i + 1;
                    break;
                }
                if self.activitymaxinf[row] <= 1 && !self.propagateflags[row] && model.row_lower[row] != -INF {
                    self.mark_propagate(row, model, feastol);
                }
            } else {
                let deltamin = compute_delta(val, oldbound, newbound, INF, &mut self.activitymininf[row]);
                self.activitymin[row] += deltamin;
                if self.record_redundant_rows && model.row_lower[row] != -INF && model.row_upper[row] == INF {
                    self.update_redundant_rows(row, model, feastol);
                }
                if deltamin <= 0.0 {
                    update_threshold_ub_change(self.col_lower[col], newbound, val, &mut self.capacity_threshold[row], feastol, var_type);
                    continue;
                }
                if model.row_upper[row] != INF && self.activitymininf[row] == 0 && (self.activitymin[row] - model.row_upper[row]).value() > feastol {
                    self.infeasible = true;
                    self.infeasible_pos = self.domchgstack.len() as isize;
                    self.infeasible_reason = Reason::model_row_upper(row);
                    end = i + 1;
                    break;
                }
                if self.activitymininf[row] <= 1 && !self.propagateflags[row] && model.row_upper[row] != INF {
                    self.mark_propagate(row, model, feastol);
                }
            }
        }
        if !self.infeasible {
            let mut infeascutpool = None;
            for pool in 0..self.cutpoolpropagation.len() {
                if !self.infeasible {
                    self.cutpool_update_activity_ub_change(pool, col, oldbound, newbound, true, true, self.infeasible, env);
                    if self.infeasible {
                        infeascutpool = Some(pool);
                    }
                } else {
                    self.cutpool_update_activity_ub_change(pool, col, oldbound, newbound, true, false, self.infeasible, env);
                }
            }
            for pool in 0..infeascutpool.unwrap_or(0) {
                self.cutpool_update_activity_ub_change(pool, col, oldbound, newbound, false, true, self.infeasible, env);
            }
        }
        if self.infeasible {
            let (oldbound, newbound) = (newbound, oldbound);
            for i in start..end {
                let row = model.a_index[i];
                let val = model.a_value[i];
                if val > 0.0 {
                    let delta = compute_delta(val, oldbound, newbound, INF, &mut self.activitymaxinf[row]);
                    self.activitymax[row] += delta;
                } else {
                    let delta = compute_delta(val, oldbound, newbound, INF, &mut self.activitymininf[row]);
                    self.activitymin[row] += delta;
                }
            }
            if self.obj_prop.active {
                self.obj_update_activity_ub_change(col, oldbound, newbound, env);
            }
        } else {
            for pool in 0..self.conflict_pool_propagation.len() {
                self.conflict_update_activity_ub_change(pool, col, oldbound, newbound);
            }
        }
    }

    fn recompute_capacity_threshold(&mut self, row: usize, model: &MipModel, feastol: f64) {
        let (index, value) = model.row(row);
        let mut threshold_value = -feastol;
        for (&col, &val) in index.iter().zip(value) {
            if self.col_upper[col] == self.col_lower[col] {
                continue;
            }
            let threshold = val.abs() * bound_range(self.col_upper[col], self.col_lower[col], feastol, model.integrality[col]);
            threshold_value = max3(threshold_value, threshold, feastol);
        }
        self.capacity_threshold[row] = threshold_value;
    }

    fn update_redundant_rows(&mut self, row: usize, model: &MipModel, feastol: f64) {
        if !self.is_redundant_row(row, model, feastol) {
            return;
        }
        self.redundant_rows.insert(row, ());
    }

    pub fn redundant_row_value(&self, row: usize, model: &MipModel) -> f64 {
        if model.row_lower[row] != -INF {
            (self.activitymin[row] - model.row_lower[row]).value()
        } else {
            (self.activitymax[row] - model.row_upper[row]).value()
        }
    }

    pub fn is_redundant_row(&self, row: usize, model: &MipModel, feastol: f64) -> bool {
        self.min_activity(row) >= model.row_lower[row] - feastol && self.max_activity(row) <= model.row_upper[row] + feastol
    }

    /// HiGHS's `markPropagateCut(Reason)`: mark the cut or conflict that
    /// caused a change for propagation again.
    fn mark_propagate_cut(&mut self, reason: Reason, cutpool: &CutPool) {
        if reason.kind < 0 {
            return;
        }
        let kind = reason.kind as usize;
        if kind < self.cutpoolpropagation.len() {
            self.mark_propagate_cut_in_pool(kind, reason.index, cutpool);
        } else {
            self.mark_propagate_conflict(kind - self.cutpoolpropagation.len(), reason.index);
        }
    }

    pub fn mark_propagate(&mut self, row: usize, model: &MipModel, feastol: f64) {
        if self.propagateflags[row] {
            return;
        }
        let row_lower = model.row_lower[row];
        let row_upper = model.row_upper[row];
        let proplower = row_lower != -INF
            && (self.activitymininf[row] != 0 || self.activitymin[row] < row_lower - feastol)
            && (self.activitymaxinf[row] == 1 || self.activitymax[row].value() - row_lower <= self.capacity_threshold[row]);
        let propupper = row_upper != INF
            && (self.activitymaxinf[row] != 0 || self.activitymax[row] > row_upper + feastol)
            && (self.activitymininf[row] == 1 || row_upper - self.activitymin[row].value() <= self.capacity_threshold[row]);
        if proplower || propupper {
            self.propagateinds.push(row);
            self.propagateflags[row] = true;
        }
    }

    pub fn compute_row_activities(&mut self, model: &MipModel, feastol: f64) {
        let num_row = model.num_row;
        self.activitymin = vec![CDouble::new(0.0); num_row];
        self.activitymininf = vec![0; num_row];
        self.activitymax = vec![CDouble::new(0.0); num_row];
        self.activitymaxinf = vec![0; num_row];
        self.capacity_threshold = vec![0.0; num_row];
        self.propagateflags = vec![false; num_row];
        self.propagateinds.reserve(num_row);
        for i in 0..num_row {
            let (index, value) = model.row(i);
            (self.activitymininf[i], self.activitymin[i]) = self.compute_min_activity(index, value);
            (self.activitymaxinf[i], self.activitymax[i]) = self.compute_max_activity(index, value);
            self.recompute_capacity_threshold(i, model, feastol);
            if (self.activitymininf[i] <= 1 && model.row_upper[i] != INF) || (self.activitymaxinf[i] <= 1 && model.row_lower[i] != -INF) {
                self.mark_propagate(i, model, feastol);
            }
        }
    }

    fn do_change_bound(&mut self, boundchg: DomainChange, env: &mut DomainEnv) -> f64 {
        let col = boundchg.column;
        let oldbound;
        match boundchg.boundtype {
            BoundType::Lower => {
                oldbound = self.col_lower[col];
                self.col_lower[col] = boundchg.boundval;
                if oldbound != boundchg.boundval {
                    if !self.infeasible {
                        self.update_activity_lb_change(col, oldbound, boundchg.boundval, env);
                    }
                    if !self.is_changed_col(col) {
                        self.changedcolsflags[col] = true;
                        self.changedcols.push(col);
                    }
                }
            }
            BoundType::Upper => {
                oldbound = self.col_upper[col];
                self.col_upper[col] = boundchg.boundval;
                if oldbound != boundchg.boundval {
                    if !self.infeasible {
                        self.update_activity_ub_change(col, oldbound, boundchg.boundval, env);
                    }
                    if !self.is_changed_col(col) {
                        self.changedcolsflags[col] = true;
                        self.changedcols.push(col);
                    }
                }
            }
        }
        oldbound
    }

    /// HiGHS's `changeBound`: tighten a bound (a branching may also restate
    /// one), record it on the stack with its reason, and fix what the clique
    /// table and implications imply when a binary is fixed.
    pub fn change_bound(&mut self, mut boundchg: DomainChange, reason: Reason, env: &mut DomainEnv) {
        if crate::util::tracing() {
            eprintln!(
                "B {} {} {:x} {} {}",
                boundchg.boundtype as i32,
                boundchg.column,
                boundchg.boundval.to_bits(),
                reason.kind,
                self.is_global as i32
            );
        }
        self.sync(env);
        let col = boundchg.column;
        let feastol = env.feastol;
        let prev_pos;
        match boundchg.boundtype {
            BoundType::Lower => {
                if boundchg.boundval <= self.col_lower[col] {
                    if reason.kind != Reason::BRANCHING {
                        return;
                    }
                    boundchg.boundval = self.col_lower[col];
                }
                if boundchg.boundval > self.col_upper[col] {
                    if boundchg.boundval - self.col_upper[col] > feastol {
                        if !self.infeasible {
                            self.infeasible_pos = self.domchgstack.len() as isize;
                            self.infeasible = true;
                            self.infeasible_reason = Reason::conflicting_bounds(self.domchgstack.len());
                        }
                    } else {
                        boundchg.boundval = self.col_upper[col];
                        if boundchg.boundval == self.col_lower[col] {
                            return;
                        }
                    }
                }
                prev_pos = self.col_lower_pos[col];
                self.col_lower_pos[col] = self.domchgstack.len() as isize;
            }
            BoundType::Upper => {
                if boundchg.boundval >= self.col_upper[col] {
                    if reason.kind != Reason::BRANCHING {
                        return;
                    }
                    boundchg.boundval = self.col_upper[col];
                }
                if boundchg.boundval < self.col_lower[col] {
                    if self.col_lower[col] - boundchg.boundval > feastol {
                        if !self.infeasible {
                            self.infeasible_pos = self.domchgstack.len() as isize;
                            self.infeasible = true;
                            self.infeasible_reason = Reason::conflicting_bounds(self.domchgstack.len());
                        }
                    } else {
                        boundchg.boundval = self.col_lower[col];
                        if boundchg.boundval == self.col_upper[col] {
                            return;
                        }
                    }
                }
                prev_pos = self.col_upper_pos[col];
                self.col_upper_pos[col] = self.domchgstack.len() as isize;
            }
        }
        if reason.kind == Reason::BRANCHING {
            self.branch_pos.push(self.domchgstack.len());
        }
        let binary = self.is_binary(env.model, col);
        let oldbound = self.do_change_bound(boundchg, env);
        self.prevboundval.push((oldbound, prev_pos));
        self.domchgstack.push(boundchg);
        self.domchgreason.push(reason);
        if binary && !self.infeasible && self.is_fixed(col) {
            let val = usize::from(self.col_lower[col] > 0.5);
            let cliquetable = env.cliquetable;
            cliquetable.add_implications(self, col, val, env);
            if !self.infeasible {
                let implications = env.implications;
                implications.apply_implications(self, col, val, env);
            }
        }
    }

    /// HiGHS's `checkChangeBound`: change the bound if the suggested value
    /// is accepted; returns whether it was.
    pub fn check_change_bound(&mut self, boundtype: BoundType, col: usize, boundval: CDouble, reason: Reason, env: &mut DomainEnv) -> bool {
        if (boundval * super::TINY).value().abs() > env.feastol {
            return false;
        }
        let (bound, accept) = match boundtype {
            BoundType::Lower => self.adjusted_lb(col, boundval, env.cx),
            BoundType::Upper => self.adjusted_ub(col, boundval, env.cx),
        };
        if !accept {
            return false;
        }
        self.change_bound(DomainChange::new(bound, col, boundtype), reason, env);
        true
    }

    /// HiGHS's `fixCol`.
    pub fn fix_col(&mut self, col: usize, val: f64, reason: Reason, env: &mut DomainEnv) {
        if self.col_lower[col] < val {
            self.change_bound(DomainChange::new(val, col, BoundType::Lower), reason, env);
            if !self.infeasible {
                self.propagate(env);
            }
        }
        if !self.infeasible && self.col_upper[col] > val {
            self.change_bound(DomainChange::new(val, col, BoundType::Upper), reason, env);
        }
    }

    fn clear_stack_positions(&mut self) {
        for domchg in &self.domchgstack {
            match domchg.boundtype {
                BoundType::Lower => self.col_lower_pos[domchg.column] = NO_POS,
                BoundType::Upper => self.col_upper_pos[domchg.column] = NO_POS,
            }
        }
        self.prevboundval.clear();
        self.domchgstack.clear();
        self.domchgreason.clear();
        self.branch_pos.clear();
    }

    /// HiGHS's `setDomainChangeStack`: replay `domchgstack` from this
    /// domain's current bounds.
    pub fn set_domain_change_stack(&mut self, domchgstack: &[DomainChange], env: &mut DomainEnv) {
        self.infeasible = false;
        self.clear_stack_positions();
        for domchg in domchgstack {
            if domchg.boundtype == BoundType::Lower && domchg.boundval <= self.col_lower[domchg.column] {
                continue;
            }
            if domchg.boundtype == BoundType::Upper && domchg.boundval >= self.col_upper[domchg.column] {
                continue;
            }
            self.change_bound(*domchg, Reason::unspecified(), env);
            if self.infeasible {
                break;
            }
        }
    }

    /// HiGHS's `setDomainChangeStack` with branching positions: replay the
    /// stack, propagating after each change, the branchings as branchings.
    pub fn set_domain_change_stack_with_branching(&mut self, domchgstack: &[DomainChange], branching_positions: &[usize], env: &mut DomainEnv) {
        self.infeasible = false;
        self.clear_stack_positions();
        let stacksize = domchgstack.len();
        let mut k = 0;
        for &branch_pos in branching_positions {
            while k < branch_pos {
                let domchg = domchgstack[k];
                k += 1;
                if domchg.boundtype == BoundType::Lower && domchg.boundval <= self.col_lower[domchg.column] {
                    continue;
                }
                if domchg.boundtype == BoundType::Upper && domchg.boundval >= self.col_upper[domchg.column] {
                    continue;
                }
                self.change_bound(domchg, Reason::unspecified(), env);
                if !self.infeasible {
                    self.propagate(env);
                }
                if self.infeasible {
                    return;
                }
            }
            if k == stacksize {
                return;
            }
            // A branching made redundant only by its subtree being done (and
            // the global bound updated) stays a branching, for symmetry
            // handling; it goes only when strictly redundant globally or
            // already implied by a local change.
            let domchg = domchgstack[k];
            if domchg.boundtype == BoundType::Lower {
                if domchg.boundval <= self.col_lower[domchg.column] && (domchg.boundval < self.col_lower[domchg.column] || self.col_lower_pos[domchg.column] != NO_POS) {
                    continue;
                }
            } else if domchg.boundval >= self.col_upper[domchg.column] && (domchg.boundval > self.col_upper[domchg.column] || self.col_upper_pos[domchg.column] != NO_POS) {
                continue;
            }
            self.change_bound(domchg, Reason::branching(), env);
            if !self.infeasible {
                self.propagate(env);
            }
            if self.infeasible {
                return;
            }
        }
        while k < stacksize {
            let domchg = domchgstack[k];
            k += 1;
            if domchg.boundtype == BoundType::Lower && domchg.boundval <= self.col_lower[domchg.column] {
                continue;
            }
            if domchg.boundtype == BoundType::Upper && domchg.boundval >= self.col_upper[domchg.column] {
                continue;
            }
            self.change_bound(domchg, Reason::unspecified(), env);
            if !self.infeasible {
                self.propagate(env);
            }
            if self.infeasible {
                break;
            }
        }
    }

    /// HiGHS's `backtrackToGlobal`: undo every change on the stack.
    pub fn backtrack_to_global(&mut self, env: &mut DomainEnv) {
        self.sync(env);
        let mut k = self.domchgstack.len() as isize - 1;
        let old_infeasible = self.infeasible;
        let old_reason = self.infeasible_reason;
        if self.infeasible && self.infeasible_pos == self.domchgstack.len() as isize {
            self.infeasible = false;
            self.infeasible_reason = Reason::unspecified();
        }
        while k >= 0 {
            let ku = k as usize;
            let (prevbound, prevpos) = self.prevboundval[ku];
            let domchg = self.domchgstack[ku];
            match domchg.boundtype {
                BoundType::Lower => self.col_lower_pos[domchg.column] = prevpos,
                BoundType::Upper => self.col_upper_pos[domchg.column] = prevpos,
            }
            if prevbound != domchg.boundval {
                self.do_change_bound(DomainChange::new(prevbound, domchg.column, domchg.boundtype), env);
            }
            if self.infeasible && self.infeasible_pos == k {
                self.infeasible = false;
                self.infeasible_reason = Reason::unspecified();
            }
            k -= 1;
        }
        if old_infeasible {
            self.mark_propagate_cut(old_reason, env.cx.cutpool);
            self.infeasible_reason = Reason::unspecified();
            self.infeasible = false;
        }
        let numreason = self.domchgreason.len();
        for i in (k + 1) as usize..numreason {
            let reason = self.domchgreason[i];
            self.mark_propagate_cut(reason, env.cx.cutpool);
        }
        self.domchgstack.clear();
        self.prevboundval.clear();
        self.domchgreason.clear();
        self.branch_pos.clear();
    }

    /// HiGHS's `backtrack`: undo the changes back to and including the last
    /// branching, returning that branching (column `None` if there was none).
    pub fn backtrack(&mut self, env: &mut DomainEnv) -> Option<DomainChange> {
        self.sync(env);
        let mut k = self.domchgstack.len() as isize - 1;
        let old_infeasible = self.infeasible;
        let old_reason = self.infeasible_reason;
        if self.infeasible && self.infeasible_pos == self.domchgstack.len() as isize {
            self.infeasible = false;
            self.infeasible_reason = Reason::unspecified();
        }
        while k >= 0 {
            let ku = k as usize;
            let (prevbound, prevpos) = self.prevboundval[ku];
            let domchg = self.domchgstack[ku];
            match domchg.boundtype {
                BoundType::Lower => self.col_lower_pos[domchg.column] = prevpos,
                BoundType::Upper => self.col_upper_pos[domchg.column] = prevpos,
            }
            self.do_change_bound(DomainChange::new(prevbound, domchg.column, domchg.boundtype), env);
            if self.infeasible && self.infeasible_pos == k {
                self.infeasible = false;
                self.infeasible_reason = Reason::unspecified();
            }
            if self.domchgreason[ku].kind == Reason::BRANCHING {
                self.branch_pos.pop();
                break;
            }
            k -= 1;
        }
        if old_infeasible {
            self.mark_propagate_cut(old_reason, env.cx.cutpool);
            self.infeasible_reason = Reason::unspecified();
            self.infeasible = false;
        }
        let numreason = self.domchgreason.len();
        for i in (k + 1) as usize..numreason {
            let reason = self.domchgreason[i];
            self.mark_propagate_cut(reason, env.cx.cutpool);
        }
        if k < 0 {
            self.domchgstack.clear();
            self.prevboundval.clear();
            self.domchgreason.clear();
            self.branch_pos.clear();
            return None;
        }
        let ku = k as usize;
        let backtrackboundchg = self.domchgstack[ku];
        self.domchgstack.truncate(ku);
        self.domchgreason.truncate(ku);
        self.prevboundval.truncate(ku);
        Some(backtrackboundchg)
    }

    fn have_propagation_rows(&self, upper_limit: f64) -> bool {
        !self.propagateinds.is_empty()
            || (self.obj_prop.active && self.obj_should_be_propagated(upper_limit))
            || self.cutpoolpropagation.iter().any(|prop| !prop.propagatecutinds.is_empty())
            || self.conflict_pool_propagation.iter().any(|prop| !prop.propagate_conflict_inds.is_empty())
    }

    /// HiGHS's `propagate`: propagate the objective, the conflicts, the
    /// model's rows and the cuts until nothing is left marked. Returns
    /// whether anything was.
    pub fn propagate(&mut self, env: &mut DomainEnv) -> bool {
        self.sync(env);
        if !self.have_propagation_rows(env.upper_limit) {
            return false;
        }
        let mut propagateinds: Vec<usize> = Vec::new();
        let mut changedbounds: Vec<DomainChange> = Vec::new();
        while self.have_propagation_rows(env.upper_limit) {
            if self.obj_prop.active {
                self.obj_propagate(env);
            }
            for pool in 0..self.conflict_pool_propagation.len() {
                while !self.conflict_pool_propagation[pool].propagate_conflict_inds.is_empty() {
                    std::mem::swap(&mut propagateinds, &mut self.conflict_pool_propagation[pool].propagate_conflict_inds);
                    for &conflict in &propagateinds {
                        self.propagate_conflict(pool, conflict, env);
                    }
                    propagateinds.clear();
                }
            }
            if !self.propagateinds.is_empty() {
                std::mem::swap(&mut propagateinds, &mut self.propagateinds);
                let numproprows = propagateinds.len();
                for &row in &propagateinds {
                    self.propagateflags[row] = false;
                }
                if !self.infeasible {
                    let model = env.model;
                    let feastol = env.feastol;
                    self.prop_row_num_changed_bounds = vec![(0, 0); numproprows];
                    changedbounds.clear();
                    let mut starts = Vec::with_capacity(numproprows);
                    for k in 0..numproprows {
                        let i = propagateinds[k];
                        starts.push(changedbounds.len());
                        let (rindex, rvalue) = model.row(i);
                        let mut recompute_cap_threshold = false;
                        if model.row_upper[i] != INF && (self.activitymaxinf[i] != 0 || self.activitymax[i] > model.row_upper[i] + feastol) {
                            self.activitymin[i].renormalize();
                            self.prop_row_num_changed_bounds[k].0 =
                                self.propagate_row_upper(rindex, rvalue, model.row_upper[i], self.activitymin[i], self.activitymininf[i], &mut changedbounds, env.cx);
                            recompute_cap_threshold = true;
                        }
                        if model.row_lower[i] != -INF && (self.activitymininf[i] != 0 || self.activitymin[i] < model.row_lower[i] - feastol) {
                            self.activitymax[i].renormalize();
                            self.prop_row_num_changed_bounds[k].1 =
                                self.propagate_row_lower(rindex, rvalue, model.row_lower[i], self.activitymax[i], self.activitymaxinf[i], &mut changedbounds, env.cx);
                            recompute_cap_threshold = true;
                        }
                        if recompute_cap_threshold {
                            self.recompute_capacity_threshold(i, model, feastol);
                        }
                    }
                    for k in 0..numproprows {
                        let i = propagateinds[k];
                        let (upper_count, lower_count) = self.prop_row_num_changed_bounds[k];
                        if upper_count != 0 {
                            let start = starts[k];
                            for j in start..start + upper_count {
                                if self.infeasible {
                                    break;
                                }
                                self.change_bound(changedbounds[j], Reason::model_row_upper(i), env);
                            }
                            if self.infeasible {
                                break;
                            }
                        }
                        if lower_count != 0 {
                            let start = starts[k] + upper_count;
                            for j in start..start + lower_count {
                                if self.infeasible {
                                    break;
                                }
                                self.change_bound(changedbounds[j], Reason::model_row_lower(i), env);
                            }
                            if self.infeasible {
                                break;
                            }
                        }
                    }
                }
                propagateinds.clear();
            }
            for pool in 0..self.cutpoolpropagation.len() {
                if self.cutpoolpropagation[pool].propagatecutinds.is_empty() {
                    continue;
                }
                std::mem::swap(&mut propagateinds, &mut self.cutpoolpropagation[pool].propagatecutinds);
                let numproprows = propagateinds.len();
                for &cut in &propagateinds {
                    self.cutpoolpropagation[pool].propagatecutflags[cut] &= 2;
                }
                if !self.infeasible {
                    self.prop_row_num_changed_bounds = vec![(0, 0); numproprows];
                    changedbounds.clear();
                    let mut starts = Vec::with_capacity(numproprows);
                    for k in 0..numproprows {
                        let i = propagateinds[k];
                        starts.push(changedbounds.len());
                        // A cut marked as deleted is skipped.
                        if self.cutpoolpropagation[pool].propagatecutflags[i] & 2 != 0 {
                            continue;
                        }
                        let cutpool: &CutPool = env.cx.cutpool;
                        let (rindex, rvalue) = cutpool.cut(i);
                        self.cutpoolpropagation[pool].activitycuts[i].renormalize();
                        let activity = self.cutpoolpropagation[pool].activitycuts[i];
                        let ninf = self.cutpoolpropagation[pool].activitycutsinf[i];
                        self.prop_row_num_changed_bounds[k].0 = self.propagate_row_upper(rindex, rvalue, cutpool.rhs()[i], activity, ninf, &mut changedbounds, env.cx);
                        let (rindex, rvalue) = (rindex.to_vec(), rvalue.to_vec());
                        self.cutpool_recompute_capacity_threshold(pool, i, &rindex, &rvalue, env);
                    }
                    for k in 0..numproprows {
                        let i = propagateinds[k];
                        let count = self.prop_row_num_changed_bounds[k].0;
                        if count != 0 {
                            env.cx.cutpool.reset_age(i);
                            let start = starts[k];
                            for j in start..start + count {
                                if self.infeasible {
                                    break;
                                }
                                self.change_bound(changedbounds[j], Reason::cut(pool, i), env);
                            }
                        }
                        if self.infeasible {
                            break;
                        }
                    }
                }
                propagateinds.clear();
            }
        }
        true
    }

    /// HiGHS's `getColLowerPos`: column `col`'s lower bound as the stack
    /// stood at `stackpos`, and the position of the change that set it.
    pub fn col_lower_pos(&self, col: usize, stackpos: isize) -> (f64, isize) {
        let mut lb = self.col_lower[col];
        let mut pos = self.col_lower_pos[col];
        while pos > stackpos || (pos != NO_POS && self.prevboundval[pos as usize].0 == lb) {
            lb = self.prevboundval[pos as usize].0;
            pos = self.prevboundval[pos as usize].1;
        }
        (lb, pos)
    }

    /// HiGHS's `getColUpperPos`.
    pub fn col_upper_pos(&self, col: usize, stackpos: isize) -> (f64, isize) {
        let mut ub = self.col_upper[col];
        let mut pos = self.col_upper_pos[col];
        while pos > stackpos || (pos != NO_POS && self.prevboundval[pos as usize].0 == ub) {
            ub = self.prevboundval[pos as usize].0;
            pos = self.prevboundval[pos as usize].1;
        }
        (ub, pos)
    }

    /// HiGHS's `tightenCoefficients` of the row `vals . x <= rhs`.
    pub fn tighten_coefficients(&self, inds: &[usize], vals: &mut [f64], rhs: &mut f64, model: &MipModel, feastol: f64) {
        let mut maxactivity = CDouble::new(0.0);
        for (&col, &val) in inds.iter().zip(vals.iter()) {
            if val > 0.0 {
                if self.col_upper[col] == INF {
                    return;
                }
                maxactivity += self.col_upper[col] * val;
            } else {
                if self.col_lower[col] == -INF {
                    return;
                }
                maxactivity += self.col_lower[col] * val;
            }
        }
        let maxabscoef = maxactivity - *rhs;
        if maxabscoef > feastol {
            let mut upper = CDouble::new(*rhs);
            let mut tightened = 0;
            for (i, &col) in inds.iter().enumerate() {
                if model.is_col_continuous(col) {
                    continue;
                }
                if vals[i] > maxabscoef.value() {
                    let delta = CDouble::new(vals[i]) - maxabscoef;
                    upper -= delta * self.col_upper[col];
                    vals[i] = maxabscoef.value();
                    tightened += 1;
                } else if vals[i] < -maxabscoef.value() {
                    let delta = CDouble::new(-vals[i]) - maxabscoef;
                    upper += delta * self.col_lower[col];
                    vals[i] = -maxabscoef.value();
                    tightened += 1;
                }
            }
            if tightened != 0 {
                *rhs = upper.value();
            }
        }
    }

    /// HiGHS's `getMinCutActivity`.
    pub fn min_cut_activity(&self, cut: usize) -> f64 {
        let Some(prop) = self.cutpoolpropagation.first() else {
            return -INF;
        };
        if cut < prop.propagatecutflags.len() && prop.propagatecutflags[cut] & 2 == 0 && prop.activitycutsinf[cut] == 0 {
            prop.activitycuts[cut].value()
        } else {
            -INF
        }
    }

    pub fn is_fixing(&self, domchg: &DomainChange, epsilon: f64) -> bool {
        let otherbound = match domchg.boundtype {
            BoundType::Upper => self.col_lower[domchg.column],
            BoundType::Lower => self.col_upper[domchg.column],
        };
        (domchg.boundval - otherbound).abs() <= epsilon
    }

    /// HiGHS's `flip`: the bound change that excludes `domchg`.
    pub fn flip(&self, domchg: &DomainChange, cx: &MipCtx) -> DomainChange {
        match domchg.boundtype {
            BoundType::Lower => {
                let mut flipped = DomainChange::new(domchg.boundval - cx.feastol, domchg.column, BoundType::Upper);
                if cx.model.is_col_integral(domchg.column) {
                    flipped.boundval = flipped.boundval.floor();
                }
                flipped
            }
            BoundType::Upper => {
                let mut flipped = DomainChange::new(domchg.boundval + cx.feastol, domchg.column, BoundType::Lower);
                if cx.model.is_col_integral(domchg.column) {
                    flipped.boundval = flipped.boundval.ceil();
                }
                flipped
            }
        }
    }
}

// ---- conflict analysis --------------------------------------------------

/// HiGHS's `ConflictSet::ResolveCandidate`, ordered by priority (higher
/// first), then stack position.
#[derive(Clone, Copy, Debug, Default)]
struct ResolveCandidate {
    delta: f64,
    base_bound: f64,
    prio: f64,
    bound_pos: isize,
    value_pos: usize,
}

fn sort_candidates(buffer: &mut [ResolveCandidate]) {
    buffer.sort_by(|a, b| {
        if a.prio > b.prio {
            std::cmp::Ordering::Less
        } else if b.prio > a.prio {
            std::cmp::Ordering::Greater
        } else {
            a.bound_pos.cmp(&b.bound_pos)
        }
    });
}

fn compute_prio(val: f64, bound: f64, globalbound: f64, numnodes: i64) -> f64 {
    (val * (bound - globalbound) * (1 + numnodes) as f64).abs()
}

/// HiGHS's `ConflictSet`. Frontiers map stack positions to the (possibly
/// relaxed) bound change they stand for.
struct ConflictSet {
    reason_side_frontier: BTreeMap<isize, DomainChange>,
    reconvergence_frontier: BTreeMap<isize, DomainChange>,
    resolve_queue: BinaryHeap<isize>,
    resolved_domain_changes: Vec<(isize, DomainChange)>,
    resolve_buffer: Vec<ResolveCandidate>,
}

/// Which frontier an explanation reads.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Frontier {
    ReasonSide,
    Reconvergence,
    None,
}

impl ConflictSet {
    fn new() -> Self {
        Self {
            reason_side_frontier: BTreeMap::new(),
            reconvergence_frontier: BTreeMap::new(),
            resolve_queue: BinaryHeap::new(),
            resolved_domain_changes: Vec::new(),
            resolve_buffer: Vec::new(),
        }
    }

    fn frontier(&self, which: Frontier) -> Option<&BTreeMap<isize, DomainChange>> {
        match which {
            Frontier::ReasonSide => Some(&self.reason_side_frontier),
            Frontier::Reconvergence => Some(&self.reconvergence_frontier),
            Frontier::None => None,
        }
    }

    fn frontier_mut(&mut self, which: Frontier) -> &mut BTreeMap<isize, DomainChange> {
        match which {
            Frontier::ReasonSide => &mut self.reason_side_frontier,
            _ => &mut self.reconvergence_frontier,
        }
    }
}

impl Domain {
    /// HiGHS's `explainBoundChangeGeq`.
    #[allow(clippy::too_many_arguments)]
    fn explain_bound_change_geq(
        &self,
        set: &mut ConflictSet,
        frontier: Frontier,
        domchg_pos: isize,
        domchg: DomainChange,
        inds: &[usize],
        vals: &[f64],
        rhs: f64,
        max_act: f64,
        globaldom: &Domain,
        cx: &MipCtx,
    ) -> bool {
        if max_act == INF {
            return false;
        }
        let mut domchg_val = 0.0;
        set.resolve_buffer.clear();
        let nodequeue = cx.nodequeue.expect("conflict analysis reads the node queue").borrow();
        let mut m = CDouble::new(max_act);
        for (i, (&col, &val)) in inds.iter().zip(vals).enumerate() {
            if col == domchg.column {
                domchg_val = val;
                continue;
            }
            let mut cand = ResolveCandidate {
                value_pos: i,
                ..ResolveCandidate::default()
            };
            if val > 0.0 {
                let (ub, bound_pos) = self.col_upper_pos(col, domchg_pos);
                cand.bound_pos = bound_pos;
                if globaldom.col_upper[col] <= ub || bound_pos == NO_POS {
                    continue;
                }
                if let Some(existing) = set.frontier(frontier).and_then(|f| f.get(&bound_pos)) {
                    cand.base_bound = existing.boundval;
                    if cand.base_bound != globaldom.col_upper[col] {
                        m += val * (cand.base_bound - globaldom.col_upper[col]);
                    }
                    if cand.base_bound <= ub {
                        continue;
                    }
                } else {
                    cand.base_bound = globaldom.col_upper[col];
                }
                cand.delta = val * (ub - cand.base_bound);
                cand.prio = compute_prio(val, ub, globaldom.col_upper[col], nodequeue.num_nodes_down(col));
            } else {
                let (lb, bound_pos) = self.col_lower_pos(col, domchg_pos);
                cand.bound_pos = bound_pos;
                if globaldom.col_lower[col] >= lb || bound_pos == NO_POS {
                    continue;
                }
                if let Some(existing) = set.frontier(frontier).and_then(|f| f.get(&bound_pos)) {
                    cand.base_bound = existing.boundval;
                    if cand.base_bound != globaldom.col_lower[col] {
                        m += val * (cand.base_bound - globaldom.col_lower[col]);
                    }
                    if cand.base_bound >= lb {
                        continue;
                    }
                } else {
                    cand.base_bound = globaldom.col_lower[col];
                }
                cand.delta = val * (lb - cand.base_bound);
                cand.prio = compute_prio(val, lb, globaldom.col_lower[col], nodequeue.num_nodes_up(col));
            }
            set.resolve_buffer.push(cand);
        }
        if domchg_val == 0.0 {
            return false;
        }
        sort_candidates(&mut set.resolve_buffer);
        let mut b0 = domchg.boundval;
        if !cx.model.is_col_continuous(domchg.column) {
            // An integral bound was rounded: relax it by 1 - 10 feastol.
            if domchg.boundtype == BoundType::Lower {
                b0 -= 1.0 - 10.0 * cx.feastol;
            } else {
                b0 += 1.0 - 10.0 * cx.feastol;
            }
        } else if domchg.boundtype == BoundType::Lower {
            b0 -= cx.epsilon;
        } else {
            b0 += cx.epsilon;
        }
        b0 *= domchg_val;
        let m_upper = rhs - b0;
        if domchg_val < 0.0 {
            m -= domchg_val * globaldom.col_lower[domchg.column];
        } else {
            m -= domchg_val * globaldom.col_upper[domchg.column];
        }
        self.resolve_linear_geq(set, m, m_upper, vals, cx)
    }

    /// HiGHS's `explainBoundChangeLeq`.
    #[allow(clippy::too_many_arguments)]
    fn explain_bound_change_leq(
        &self,
        set: &mut ConflictSet,
        frontier: Frontier,
        domchg_pos: isize,
        domchg: DomainChange,
        inds: &[usize],
        vals: &[f64],
        rhs: f64,
        min_act: f64,
        globaldom: &Domain,
        cx: &MipCtx,
    ) -> bool {
        if min_act == -INF {
            return false;
        }
        let mut domchg_val = 0.0;
        set.resolve_buffer.clear();
        let nodequeue = cx.nodequeue.expect("conflict analysis reads the node queue").borrow();
        let mut m = CDouble::new(min_act);
        for (i, (&col, &val)) in inds.iter().zip(vals).enumerate() {
            if col == domchg.column {
                domchg_val = val;
                continue;
            }
            let mut cand = ResolveCandidate {
                value_pos: i,
                ..ResolveCandidate::default()
            };
            if val > 0.0 {
                let (lb, bound_pos) = self.col_lower_pos(col, domchg_pos);
                cand.bound_pos = bound_pos;
                if globaldom.col_lower[col] >= lb || bound_pos == NO_POS {
                    continue;
                }
                if let Some(existing) = set.frontier(frontier).and_then(|f| f.get(&bound_pos)) {
                    cand.base_bound = existing.boundval;
                    if cand.base_bound != globaldom.col_lower[col] {
                        m += val * (cand.base_bound - globaldom.col_lower[col]);
                    }
                    if cand.base_bound >= lb {
                        continue;
                    }
                } else {
                    cand.base_bound = globaldom.col_lower[col];
                }
                cand.delta = val * (lb - cand.base_bound);
                cand.prio = compute_prio(val, lb, globaldom.col_lower[col], nodequeue.num_nodes_up(col));
            } else {
                let (ub, bound_pos) = self.col_upper_pos(col, domchg_pos);
                cand.bound_pos = bound_pos;
                if globaldom.col_upper[col] <= ub || bound_pos == NO_POS {
                    continue;
                }
                if let Some(existing) = set.frontier(frontier).and_then(|f| f.get(&bound_pos)) {
                    cand.base_bound = existing.boundval;
                    if cand.base_bound != globaldom.col_upper[col] {
                        m += val * (cand.base_bound - globaldom.col_upper[col]);
                    }
                    if cand.base_bound <= ub {
                        continue;
                    }
                } else {
                    cand.base_bound = globaldom.col_upper[col];
                }
                cand.delta = val * (ub - cand.base_bound);
                cand.prio = compute_prio(val, ub, globaldom.col_upper[col], nodequeue.num_nodes_down(col));
            }
            set.resolve_buffer.push(cand);
        }
        if domchg_val == 0.0 {
            return false;
        }
        sort_candidates(&mut set.resolve_buffer);
        let mut b0 = domchg.boundval;
        if !cx.model.is_col_continuous(domchg.column) {
            if domchg.boundtype == BoundType::Lower {
                b0 -= 1.0 - 10.0 * cx.feastol;
            } else {
                b0 += 1.0 - 10.0 * cx.feastol;
            }
        } else if domchg.boundtype == BoundType::Lower {
            b0 -= cx.epsilon;
        } else {
            b0 += cx.epsilon;
        }
        b0 *= domchg_val;
        let m_lower = rhs - b0;
        if domchg_val < 0.0 {
            m -= domchg_val * globaldom.col_upper[domchg.column];
        } else {
            m -= domchg_val * globaldom.col_lower[domchg.column];
        }
        self.resolve_linear_leq(set, m, m_lower, vals, cx)
    }

    /// HiGHS's `resolveLinearGeq`: take candidates until the bound change is
    /// explained, then relax or drop what is not needed.
    fn resolve_linear_geq(&self, set: &mut ConflictSet, mut m: CDouble, m_upper: f64, vals: &[f64], cx: &MipCtx) -> bool {
        let feastol = cx.feastol;
        set.resolved_domain_changes.clear();
        let mut covered = (m - m_upper).value();
        if covered > 0.0 {
            for cand in &set.resolve_buffer {
                m += cand.delta;
                set.resolved_domain_changes.push((cand.bound_pos, self.domchgstack[cand.bound_pos as usize]));
                covered = (m - m_upper).value();
                if covered <= 0.0 {
                    break;
                }
            }
            if covered > 0.0 {
                return false;
            }
            if covered < -feastol {
                let mut k = set.resolved_domain_changes.len() as isize - 1;
                while k >= 0 {
                    let ku = k as usize;
                    let cand = set.resolve_buffer[ku];
                    let (pos, domchg) = set.resolved_domain_changes[ku];
                    let i = cand.value_pos;
                    let col = domchg.column;
                    if domchg.boundtype == BoundType::Lower {
                        let lb = domchg.boundval;
                        let glb = cand.base_bound;
                        let mut relax_lb = (((CDouble::new(m_upper) - (m - cand.delta)) / vals[i]) + glb).value();
                        if !cx.model.is_col_continuous(col) {
                            relax_lb = relax_lb.ceil();
                        }
                        if relax_lb - lb >= -feastol {
                            k -= 1;
                            continue;
                        }
                        set.resolved_domain_changes[ku].1.boundval = relax_lb;
                        if relax_lb - glb <= cx.epsilon {
                            let last = set.resolved_domain_changes.len() - 1;
                            set.resolved_domain_changes.swap(last, ku);
                            set.resolved_domain_changes.truncate(last);
                            m -= cand.delta;
                        } else {
                            let mut pos = pos;
                            while relax_lb <= self.prevboundval[pos as usize].0 {
                                pos = self.prevboundval[pos as usize].1;
                            }
                            set.resolved_domain_changes[ku].0 = pos;
                            m += vals[i] * (relax_lb - lb);
                        }
                        covered = (m - m_upper).value();
                        if covered >= -feastol {
                            break;
                        }
                    } else {
                        let ub = domchg.boundval;
                        let gub = cand.base_bound;
                        let mut relax_ub = (((CDouble::new(m_upper) - (m - cand.delta)) / vals[i]) + gub).value();
                        if !cx.model.is_col_continuous(col) {
                            relax_ub = relax_ub.floor();
                        }
                        if relax_ub - ub <= feastol {
                            k -= 1;
                            continue;
                        }
                        set.resolved_domain_changes[ku].1.boundval = relax_ub;
                        if relax_ub - gub >= -cx.epsilon {
                            let last = set.resolved_domain_changes.len() - 1;
                            set.resolved_domain_changes.swap(last, ku);
                            set.resolved_domain_changes.truncate(last);
                            m -= cand.delta;
                        } else {
                            let mut pos = pos;
                            while relax_ub >= self.prevboundval[pos as usize].0 {
                                pos = self.prevboundval[pos as usize].1;
                            }
                            set.resolved_domain_changes[ku].0 = pos;
                            m += vals[i] * (relax_ub - ub);
                        }
                        covered = (m - m_upper).value();
                        if covered >= -feastol {
                            break;
                        }
                    }
                    k -= 1;
                }
            }
        }
        true
    }

    /// HiGHS's `resolveLinearLeq`.
    fn resolve_linear_leq(&self, set: &mut ConflictSet, mut m: CDouble, m_lower: f64, vals: &[f64], cx: &MipCtx) -> bool {
        let feastol = cx.feastol;
        set.resolved_domain_changes.clear();
        let mut covered = (m - m_lower).value();
        if covered < 0.0 {
            for cand in &set.resolve_buffer {
                m += cand.delta;
                set.resolved_domain_changes.push((cand.bound_pos, self.domchgstack[cand.bound_pos as usize]));
                covered = (m - m_lower).value();
                if covered >= 0.0 {
                    break;
                }
            }
            if covered < 0.0 {
                return false;
            }
            if covered > feastol {
                let mut k = set.resolved_domain_changes.len() as isize - 1;
                while k >= 0 {
                    let ku = k as usize;
                    let cand = set.resolve_buffer[ku];
                    let (pos, domchg) = set.resolved_domain_changes[ku];
                    let i = cand.value_pos;
                    let col = domchg.column;
                    if domchg.boundtype == BoundType::Lower {
                        let lb = domchg.boundval;
                        let glb = cand.base_bound;
                        let mut relax_lb = (((CDouble::new(m_lower) - (m - cand.delta)) / vals[i]) + glb).value();
                        if !cx.model.is_col_continuous(col) {
                            relax_lb = relax_lb.ceil();
                        }
                        if relax_lb - lb >= -feastol {
                            k -= 1;
                            continue;
                        }
                        set.resolved_domain_changes[ku].1.boundval = relax_lb;
                        if relax_lb - glb <= cx.epsilon {
                            let last = set.resolved_domain_changes.len() - 1;
                            set.resolved_domain_changes.swap(last, ku);
                            set.resolved_domain_changes.truncate(last);
                            m -= cand.delta;
                        } else {
                            let mut pos = pos;
                            while relax_lb <= self.prevboundval[pos as usize].0 {
                                pos = self.prevboundval[pos as usize].1;
                            }
                            set.resolved_domain_changes[ku].0 = pos;
                            m += vals[i] * (relax_lb - lb);
                        }
                        covered = (m - m_lower).value();
                        if covered <= feastol {
                            break;
                        }
                    } else {
                        let ub = domchg.boundval;
                        let gub = cand.base_bound;
                        let mut relax_ub = (((CDouble::new(m_lower) - (m - cand.delta)) / vals[i]) + gub).value();
                        if !cx.model.is_col_continuous(col) {
                            relax_ub = relax_ub.floor();
                        }
                        if relax_ub - ub <= feastol {
                            k -= 1;
                            continue;
                        }
                        set.resolved_domain_changes[ku].1.boundval = relax_ub;
                        if relax_ub - gub >= -cx.epsilon {
                            let last = set.resolved_domain_changes.len() - 1;
                            set.resolved_domain_changes.swap(last, ku);
                            set.resolved_domain_changes.truncate(last);
                            m -= cand.delta;
                        } else {
                            let mut pos = pos;
                            while relax_ub >= self.prevboundval[pos as usize].0 {
                                pos = self.prevboundval[pos as usize].1;
                            }
                            set.resolved_domain_changes[ku].0 = pos;
                            m += vals[i] * (relax_ub - ub);
                        }
                        covered = (m - m_lower).value();
                        if covered <= feastol {
                            break;
                        }
                    }
                    k -= 1;
                }
            }
        }
        true
    }

    /// HiGHS's `explainInfeasibilityConflict`.
    fn explain_infeasibility_conflict(&self, set: &mut ConflictSet, conflict: &[DomainChange], globaldom: &Domain) -> bool {
        set.resolved_domain_changes.clear();
        for entry in conflict {
            if globaldom.is_active(entry) {
                continue;
            }
            let mut pos;
            if entry.boundtype == BoundType::Lower {
                let (lb, p) = self.col_lower_pos(entry.column, self.infeasible_pos);
                pos = p;
                if pos == NO_POS || lb < entry.boundval {
                    return false;
                }
                while self.prevboundval[pos as usize].0 >= entry.boundval {
                    pos = self.prevboundval[pos as usize].1;
                }
            } else {
                let (ub, p) = self.col_upper_pos(entry.column, self.infeasible_pos);
                pos = p;
                if pos == NO_POS || ub > entry.boundval {
                    return false;
                }
                while self.prevboundval[pos as usize].0 <= entry.boundval {
                    pos = self.prevboundval[pos as usize].1;
                }
            }
            set.resolved_domain_changes.push((pos, *entry));
        }
        true
    }

    /// HiGHS's `explainInfeasibilityGeq`.
    fn explain_infeasibility_geq(&self, set: &mut ConflictSet, inds: &[usize], vals: &[f64], rhs: f64, max_act: f64, globaldom: &Domain, cx: &MipCtx) -> bool {
        if max_act == INF {
            return false;
        }
        let infeasible_pos = if self.infeasible { self.infeasible_pos } else { POS_INF };
        set.resolve_buffer.clear();
        let nodequeue = cx.nodequeue.expect("conflict analysis reads the node queue").borrow();
        for (i, (&col, &val)) in inds.iter().zip(vals).enumerate() {
            let mut cand = ResolveCandidate {
                value_pos: i,
                ..ResolveCandidate::default()
            };
            if val > 0.0 {
                let (ub, bound_pos) = self.col_upper_pos(col, infeasible_pos);
                cand.bound_pos = bound_pos;
                cand.base_bound = globaldom.col_upper[col];
                if cand.base_bound <= ub || bound_pos == NO_POS {
                    continue;
                }
                cand.delta = val * (ub - cand.base_bound);
                cand.prio = compute_prio(val, ub, globaldom.col_upper[col], nodequeue.num_nodes_down(col));
            } else {
                let (lb, bound_pos) = self.col_lower_pos(col, infeasible_pos);
                cand.bound_pos = bound_pos;
                cand.base_bound = globaldom.col_lower[col];
                if cand.base_bound >= lb || bound_pos == NO_POS {
                    continue;
                }
                cand.delta = val * (lb - cand.base_bound);
                cand.prio = compute_prio(val, lb, globaldom.col_lower[col], nodequeue.num_nodes_up(col));
            }
            set.resolve_buffer.push(cand);
        }
        sort_candidates(&mut set.resolve_buffer);
        let m_upper = rhs - cmax(10.0, rhs.abs()) * cx.feastol;
        self.resolve_linear_geq(set, CDouble::new(max_act), m_upper, vals, cx)
    }

    /// HiGHS's `explainInfeasibilityLeq`.
    fn explain_infeasibility_leq(&self, set: &mut ConflictSet, inds: &[usize], vals: &[f64], rhs: f64, min_act: f64, globaldom: &Domain, cx: &MipCtx) -> bool {
        if min_act == -INF {
            return false;
        }
        let infeasible_pos = if self.infeasible { self.infeasible_pos } else { POS_INF };
        set.resolve_buffer.clear();
        let nodequeue = cx.nodequeue.expect("conflict analysis reads the node queue").borrow();
        for (i, (&col, &val)) in inds.iter().zip(vals).enumerate() {
            let mut cand = ResolveCandidate {
                value_pos: i,
                ..ResolveCandidate::default()
            };
            if val > 0.0 {
                let (lb, bound_pos) = self.col_lower_pos(col, infeasible_pos);
                cand.bound_pos = bound_pos;
                cand.base_bound = globaldom.col_lower[col];
                if cand.base_bound >= lb || bound_pos == NO_POS {
                    continue;
                }
                cand.delta = val * (lb - cand.base_bound);
                cand.prio = compute_prio(val, lb, globaldom.col_lower[col], nodequeue.num_nodes_up(col));
            } else {
                let (ub, bound_pos) = self.col_upper_pos(col, infeasible_pos);
                cand.bound_pos = bound_pos;
                cand.base_bound = globaldom.col_upper[col];
                if cand.base_bound <= ub || bound_pos == NO_POS {
                    continue;
                }
                cand.delta = val * (ub - cand.base_bound);
                cand.prio = compute_prio(val, ub, globaldom.col_upper[col], nodequeue.num_nodes_down(col));
            }
            set.resolve_buffer.push(cand);
        }
        sort_candidates(&mut set.resolve_buffer);
        let m_lower = rhs + cmax(10.0, rhs.abs()) * cx.feastol;
        self.resolve_linear_leq(set, CDouble::new(min_act), m_lower, vals, cx)
    }

    /// HiGHS's `explainInfeasibility`.
    fn explain_infeasibility(&mut self, set: &mut ConflictSet, globaldom: &Domain, env: &DomainEnv) -> bool {
        let reason = self.infeasible_reason;
        let cx: &MipCtx = env.cx;
        match reason.kind {
            Reason::UNKNOWN | Reason::BRANCHING => false,
            Reason::CONFLICTING_BOUNDS => {
                set.resolved_domain_changes.clear();
                let conflicting_bound_pos = reason.index;
                let domchg = self.domchgstack[conflicting_bound_pos];
                set.resolved_domain_changes.push((conflicting_bound_pos as isize, domchg));
                let other_bound_pos = if domchg.boundtype == BoundType::Lower {
                    self.col_upper_pos(domchg.column, conflicting_bound_pos as isize).1
                } else {
                    self.col_lower_pos(domchg.column, conflicting_bound_pos as isize).1
                };
                if other_bound_pos != NO_POS {
                    set.resolved_domain_changes.push((other_bound_pos, self.domchgstack[other_bound_pos as usize]));
                }
                true
            }
            Reason::CLIQUE_TABLE => false,
            Reason::MODEL_ROW_LOWER => {
                let row = reason.index;
                let (inds, vals) = cx.model.row(row);
                let max_act = globaldom.max_activity(row);
                self.explain_infeasibility_geq(set, inds, vals, cx.model.row_lower[row], max_act, globaldom, cx)
            }
            Reason::MODEL_ROW_UPPER => {
                let row = reason.index;
                let (inds, vals) = cx.model.row(row);
                let min_act = globaldom.min_activity(row);
                self.explain_infeasibility_leq(set, inds, vals, cx.model.row_upper[row], min_act, globaldom, cx)
            }
            Reason::OBJECTIVE => {
                let (inds, vals, rhs) = self.objective_propagation_constraint(self.infeasible_pos, None, env);
                let (ninfmin, min_act) = globaldom.compute_min_activity(&inds, &vals);
                // Only when a globally unbounded column is bounded locally,
                // say by a heuristic's branching.
                if ninfmin > 0 {
                    return false;
                }
                self.explain_infeasibility_leq(set, &inds, &vals, rhs, min_act.value(), globaldom, cx)
            }
            kind => {
                let kind = kind as usize;
                if kind < self.cutpoolpropagation.len() {
                    let cut = reason.index;
                    let (inds, vals) = cx.cutpool.cut(cut);
                    let min_act = globaldom.min_cut_activity(cut);
                    self.explain_infeasibility_leq(set, inds, vals, cx.cutpool.rhs()[cut], min_act, globaldom, cx)
                } else {
                    let pool = kind - self.cutpoolpropagation.len();
                    let conflict = reason.index;
                    if self.conflict_pool_propagation[pool].conflict_flag[conflict] & 8 != 0 {
                        return false;
                    }
                    let Some(entries) = cx.conflictpool.conflict(conflict) else { return false };
                    self.explain_infeasibility_conflict(set, entries, globaldom)
                }
            }
        }
    }

    /// HiGHS's `explainBoundChange`.
    fn explain_bound_change(&mut self, set: &mut ConflictSet, frontier: Frontier, pos: isize, domchg: DomainChange, globaldom: &Domain, env: &DomainEnv) -> bool {
        let reason = self.domchgreason[pos as usize];
        let cx: &MipCtx = env.cx;
        match reason.kind {
            Reason::UNKNOWN | Reason::BRANCHING | Reason::CONFLICTING_BOUNDS => false,
            Reason::CLIQUE_TABLE => {
                let col = reason.index >> 1;
                let val = reason.index & 1;
                set.resolved_domain_changes.clear();
                let bound_pos = if val != 0 { self.col_lower_pos(col, pos).1 } else { self.col_upper_pos(col, pos).1 };
                if bound_pos != NO_POS {
                    set.resolved_domain_changes.push((bound_pos, self.domchgstack[bound_pos as usize]));
                }
                true
            }
            Reason::MODEL_ROW_LOWER => {
                let row = reason.index;
                let (inds, vals) = cx.model.row(row);
                let max_act = globaldom.max_activity(row);
                self.explain_bound_change_geq(set, frontier, pos, domchg, inds, vals, cx.model.row_lower[row], max_act, globaldom, cx)
            }
            Reason::MODEL_ROW_UPPER => {
                let row = reason.index;
                let (inds, vals) = cx.model.row(row);
                let min_act = globaldom.min_activity(row);
                self.explain_bound_change_leq(set, frontier, pos, domchg, inds, vals, cx.model.row_upper[row], min_act, globaldom, cx)
            }
            Reason::OBJECTIVE => {
                let (inds, vals, rhs) = self.objective_propagation_constraint(pos, Some(domchg.column), env);
                let (ninfmin, min_act) = globaldom.compute_min_activity(&inds, &vals);
                if ninfmin == 1 {
                    return false;
                }
                self.explain_bound_change_leq(set, frontier, pos, domchg, &inds, &vals, rhs, min_act.value(), globaldom, cx)
            }
            kind => {
                let kind = kind as usize;
                if kind < self.cutpoolpropagation.len() {
                    let cut = reason.index;
                    let (inds, vals) = cx.cutpool.cut(cut);
                    let min_act = globaldom.min_cut_activity(cut);
                    self.explain_bound_change_leq(set, frontier, pos, domchg, inds, vals, cx.cutpool.rhs()[cut], min_act, globaldom, cx)
                } else {
                    let pool = kind - self.cutpoolpropagation.len();
                    let conflict = reason.index;
                    if self.conflict_pool_propagation[pool].conflict_flag[conflict] & 8 != 0 {
                        return false;
                    }
                    let Some(entries) = cx.conflictpool.conflict(conflict) else { return false };
                    self.explain_bound_change_conflict(set, pos, domchg, entries, globaldom, cx)
                }
            }
        }
    }

    /// HiGHS's `explainBoundChangeConflict`.
    fn explain_bound_change_conflict(&self, set: &mut ConflictSet, locpos: isize, locdomchg: DomainChange, conflict: &[DomainChange], globaldom: &Domain, cx: &MipCtx) -> bool {
        set.resolved_domain_changes.clear();
        let domchg = self.flip(&locdomchg, cx);
        let mut found_domchg = false;
        for entry in conflict {
            if !found_domchg && entry.column == domchg.column && entry.boundtype == domchg.boundtype {
                let found = match entry.boundtype {
                    BoundType::Lower => entry.boundval <= domchg.boundval,
                    BoundType::Upper => entry.boundval >= domchg.boundval,
                };
                if found {
                    found_domchg = true;
                    continue;
                }
            }
            if globaldom.is_active(entry) {
                continue;
            }
            let mut pos;
            if entry.boundtype == BoundType::Lower {
                let (lb, p) = self.col_lower_pos(entry.column, locpos - 1);
                pos = p;
                if pos == NO_POS || lb < entry.boundval {
                    return false;
                }
                while self.prevboundval[pos as usize].0 >= entry.boundval {
                    pos = self.prevboundval[pos as usize].1;
                }
            } else {
                let (ub, p) = self.col_upper_pos(entry.column, locpos - 1);
                pos = p;
                if pos == NO_POS || ub > entry.boundval {
                    return false;
                }
                while self.prevboundval[pos as usize].0 <= entry.boundval {
                    pos = self.prevboundval[pos as usize].1;
                }
            }
            set.resolved_domain_changes.push((pos, self.domchgstack[pos as usize]));
        }
        found_domchg
    }

    fn resolvable(&self, pos: isize) -> bool {
        !matches!(self.domchgreason[pos as usize].kind, Reason::BRANCHING | Reason::UNKNOWN)
    }

    /// HiGHS's `resolveDepth`: resolve the frontier's changes at this
    /// branching depth and after, latest first, until `stop_size` remain
    /// queued (and at least `min_resolve` were resolved). Returns how many
    /// were, or `None` if the depth level had none.
    #[allow(clippy::too_many_arguments)]
    fn resolve_depth(
        &mut self,
        set: &mut ConflictSet,
        frontier: Frontier,
        mut depth_level: usize,
        stop_size: usize,
        pseudocost: &mut Pseudocost,
        min_resolve: usize,
        increase_conflict_score: bool,
        globaldom: &Domain,
        env: &DomainEnv,
    ) -> Option<usize> {
        set.resolve_queue.clear();
        let start_pos: isize = if depth_level == 0 { 0 } else { self.branch_pos[depth_level - 1] as isize + 1 };
        while depth_level < self.branch_pos.len() {
            let branch_pos = self.branch_pos[depth_level];
            if self.domchgstack[branch_pos].boundval != self.prevboundval[branch_pos].0 {
                break;
            }
            depth_level += 1;
        }
        let end_pos = if depth_level == self.branch_pos.len() {
            None
        } else {
            Some(self.branch_pos[depth_level] as isize)
        };
        let mut empty = true;
        let positions: Vec<isize> = match end_pos {
            None => set.frontier_mut(frontier).range(start_pos..).map(|(&pos, _)| pos).collect(),
            Some(end) => set.frontier_mut(frontier).range(start_pos..=end).map(|(&pos, _)| pos).collect(),
        };
        for pos in positions {
            empty = false;
            if self.resolvable(pos) {
                set.resolve_queue.push(pos);
            }
        }
        if empty {
            return None;
        }
        let mut num_resolved = 0;
        while set.resolve_queue.len() > stop_size || (!set.resolve_queue.is_empty() && num_resolved < min_resolve) {
            let pos = set.resolve_queue.pop().expect("a queued change");
            let domchg = set.frontier_mut(frontier)[&pos];
            if !self.explain_bound_change(set, frontier, pos, domchg, globaldom, env) {
                continue;
            }
            num_resolved += 1;
            set.frontier_mut(frontier).remove(&pos);
            let resolved = std::mem::take(&mut set.resolved_domain_changes);
            for &(ipos, idomchg) in &resolved {
                let map = set.frontier_mut(frontier);
                if let std::collections::btree_map::Entry::Vacant(slot) = map.entry(ipos) {
                    slot.insert(idomchg);
                    if increase_conflict_score {
                        let stack_change = self.domchgstack[ipos as usize];
                        if stack_change.boundtype == BoundType::Lower {
                            pseudocost.increase_conflict_score_up(stack_change.column);
                        } else {
                            pseudocost.increase_conflict_score_down(stack_change.column);
                        }
                    }
                    if ipos >= start_pos && self.resolvable(ipos) {
                        set.resolve_queue.push(ipos);
                    }
                } else {
                    let existing = map.get_mut(&ipos).expect("an existing change");
                    if idomchg.boundtype == BoundType::Lower {
                        existing.boundval = cmax(existing.boundval, idomchg.boundval);
                    } else {
                        existing.boundval = cmin(existing.boundval, idomchg.boundval);
                    }
                }
            }
            set.resolved_domain_changes = resolved;
        }
        Some(num_resolved)
    }

    /// HiGHS's `computeCuts`: resolve this depth into a conflict cut, and a
    /// reconvergence cut through its UIP when there is one.
    fn compute_cuts(&mut self, set: &mut ConflictSet, depth_level: usize, pseudocost: &mut Pseudocost, globaldom: &Domain, env: &mut DomainEnv) -> Option<usize> {
        let min_resolve = usize::from(depth_level == self.branch_pos.len());
        let num_resolved = self.resolve_depth(set, Frontier::ReasonSide, depth_level, 1, pseudocost, min_resolve, true, globaldom, env)?;
        let mut num_conflicts = 0;
        if num_resolved > 0 {
            self.add_conflict_cut(set.reason_side_frontier.values().copied().collect(), env);
            num_conflicts += 1;
        }
        // A single queued change is a resolvable UIP that is not the
        // branching.
        if set.resolve_queue.len() == 1 {
            let uip_pos = set.resolve_queue.pop().expect("one queued change");
            let uip = set.reason_side_frontier[&uip_pos];
            set.resolve_queue.clear();
            set.reconvergence_frontier.clear();
            set.reconvergence_frontier.insert(uip_pos, uip);
            let num_resolved = self.resolve_depth(set, Frontier::Reconvergence, depth_level, 0, pseudocost, 0, false, globaldom, env);
            if num_resolved.is_some_and(|n| n > 0) && !set.reconvergence_frontier.contains_key(&uip_pos) {
                self.add_reconvergence_cut(set.reconvergence_frontier.values().copied().collect(), uip, env);
                num_conflicts += 1;
            }
        }
        Some(num_conflicts)
    }

    fn add_conflict_cut(&mut self, frontier: Vec<DomainChange>, env: &mut DomainEnv) {
        let model = env.model;
        let feastol = env.feastol;
        env.cx.conflictpool.add_conflict_cut(frontier.into_iter(), |col| model.is_col_continuous(col), feastol);
        // HiGHS's pool tells this domain at once.
        self.sync(env);
    }

    fn add_reconvergence_cut(&mut self, frontier: Vec<DomainChange>, reconvergence: DomainChange, env: &mut DomainEnv) {
        let flipped = self.flip(&reconvergence, env.cx);
        let model = env.model;
        let feastol = env.feastol;
        env.cx
            .conflictpool
            .add_reconvergence_cut(frontier.into_iter(), flipped, |col| model.is_col_continuous(col), feastol);
        self.sync(env);
    }

    /// The loop over depth levels HiGHS's two `ConflictSet::conflictAnalysis`
    /// share, once the frontier holds the explanation of infeasibility.
    fn conflict_analysis_from_frontier(&mut self, set: &mut ConflictSet, pseudocost: &mut Pseudocost, globaldom: &Domain, env: &mut DomainEnv) {
        pseudocost.increase_conflict_weight();
        for &(_, domchg) in &set.resolved_domain_changes {
            if domchg.boundtype == BoundType::Lower {
                pseudocost.increase_conflict_score_up(domchg.column);
            } else {
                pseudocost.increase_conflict_score_down(domchg.column);
            }
        }
        if 10 * set.resolved_domain_changes.len() > 1000 + 3 * env.num_integral_cols {
            return;
        }
        for &(pos, domchg) in &set.resolved_domain_changes {
            set.reason_side_frontier.entry(pos).or_insert(domchg);
        }
        let mut num_conflicts = 0;
        let mut last_depth = self.branch_pos.len() as isize;
        let mut curr_depth = last_depth;
        while curr_depth >= 0 {
            if curr_depth > 0 {
                // Skip redundant branchings, there only for symmetry handling.
                let branchpos = self.branch_pos[curr_depth as usize - 1];
                if self.domchgstack[branchpos].boundval == self.prevboundval[branchpos].0 {
                    last_depth -= 1;
                    curr_depth -= 1;
                    continue;
                }
            }
            let Some(num_new_conflicts) = self.compute_cuts(set, curr_depth as usize, pseudocost, globaldom, env) else {
                last_depth -= 1;
                curr_depth -= 1;
                continue;
            };
            num_conflicts += num_new_conflicts;
            if num_conflicts == 0 {
                break;
            }
            if last_depth - curr_depth >= 4 && num_new_conflicts == 0 {
                break;
            }
            curr_depth -= 1;
        }
        // Stopping at the deepest non-empty level means nothing was added:
        // the branching itself caused the infeasibility and was not
        // propagated a level up, so keep at least the current frontier.
        if curr_depth == last_depth {
            self.add_conflict_cut(set.reason_side_frontier.values().copied().collect(), env);
        }
    }

    /// HiGHS's `conflictAnalysis(conflictPool, globaldom, pseudocost)` of an
    /// infeasible local domain.
    pub fn conflict_analysis(&mut self, globaldom: &mut Domain, pseudocost: &mut Pseudocost, env: &mut DomainEnv) {
        if globaldom.infeasible() || !self.infeasible {
            return;
        }
        globaldom.propagate(env);
        if globaldom.infeasible() {
            return;
        }
        self.sync(env);
        let mut set = ConflictSet::new();
        if !self.explain_infeasibility(&mut set, globaldom, env) {
            return;
        }
        self.conflict_analysis_from_frontier(&mut set, pseudocost, globaldom, env);
    }

    /// HiGHS's `conflictAnalysis` from a proof `proofvals . x <= proofrhs`
    /// the local domain violates.
    pub fn conflict_analysis_with_proof(
        &mut self,
        proofinds: &[usize],
        proofvals: &[f64],
        proofrhs: f64,
        globaldom: &mut Domain,
        pseudocost: &mut Pseudocost,
        env: &mut DomainEnv,
    ) {
        if globaldom.infeasible() {
            return;
        }
        globaldom.propagate(env);
        if globaldom.infeasible() {
            return;
        }
        self.sync(env);
        let mut set = ConflictSet::new();
        let (ninfmin, activitymin) = globaldom.compute_min_activity(proofinds, proofvals);
        if ninfmin != 0 {
            return;
        }
        if !self.explain_infeasibility_leq(&mut set, proofinds, proofvals, proofrhs, activitymin.value(), globaldom, env.cx) {
            return;
        }
        self.conflict_analysis_from_frontier(&mut set, pseudocost, globaldom, env);
    }

    /// HiGHS's `conflictAnalyzeReconvergence`.
    #[allow(clippy::too_many_arguments)]
    pub fn conflict_analyze_reconvergence(
        &mut self,
        domchg: DomainChange,
        proofinds: &[usize],
        proofvals: &[f64],
        proofrhs: f64,
        globaldom: &mut Domain,
        pseudocost: &mut Pseudocost,
        env: &mut DomainEnv,
    ) {
        if globaldom.infeasible() {
            return;
        }
        globaldom.propagate(env);
        if globaldom.infeasible() {
            return;
        }
        self.sync(env);
        let mut set = ConflictSet::new();
        let (ninfmin, activitymin) = globaldom.compute_min_activity(proofinds, proofvals);
        if ninfmin != 0 {
            return;
        }
        if !self.explain_bound_change_leq(
            &mut set,
            Frontier::Reconvergence,
            self.domchgstack.len() as isize,
            domchg,
            proofinds,
            proofvals,
            proofrhs,
            activitymin.value(),
            globaldom,
            env.cx,
        ) {
            return;
        }
        if 10 * set.resolved_domain_changes.len() > 1000 + 3 * env.num_integral_cols {
            return;
        }
        for &(pos, change) in &set.resolved_domain_changes {
            set.reconvergence_frontier.entry(pos).or_insert(change);
        }
        let mut depth = self.branch_pos.len();
        while depth > 0 {
            let branch_pos = self.branch_pos[depth - 1];
            if self.domchgstack[branch_pos].boundval != self.prevboundval[branch_pos].0 {
                break;
            }
            depth -= 1;
        }
        self.resolve_depth(&mut set, Frontier::Reconvergence, depth, 0, pseudocost, 0, false, globaldom, env);
        self.add_reconvergence_cut(set.reconvergence_frontier.values().copied().collect(), domchg, env);
    }
}
