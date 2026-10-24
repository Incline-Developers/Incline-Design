//! HiGHS's `HighsPrimalHeuristics`: RENS, RINS, root reduced cost, the
//! feasibility pump, central, linesearch and randomized rounding, shifting
//! and ZI round, with sub-MIPs over the neighbourhoods they fix.

use std::collections::{HashMap, HashSet};

use super::{
    cpp_partition,
    cut_generation::CutGeneration,
    data::{MipData, SolutionSource},
    domain::{Domain, DomainEnv, Reason},
    lp_relaxation::{LpDomain, LpRelaxation, LpStatus},
    search::Search,
};
use crate::{
    Basis,
    hash_table::{HashSet as HighsHashSet, hash_u64},
    integers::{fractionality, nearest_integer},
    pdqsort::pdqsort,
    util::{CDouble, Random},
};

const INF: f64 = f64::INFINITY;

/// HiGHS's `HighsMipWorker` heuristic statistics, flushed into the solver
/// after each heuristic.
#[derive(Clone, Debug, Default)]
pub(crate) struct HeurStats {
    pub total_repair_lp: i64,
    pub total_repair_lp_feasible: i64,
    pub total_repair_lp_iterations: i64,
    pub lp_iterations: i64,
    pub success_observations: f64,
    pub num_success_observations: i64,
    pub infeas_observations: f64,
    pub num_infeas_observations: i64,
    pub max_submip_level: i32,
    /// The termination status a sub-MIP's terminator reported.
    pub terminated: bool,
}

/// What [`MipData::run_sub_mip`] reports of a sub-MIP.
pub(crate) struct SubMipOutcome {
    pub infeasible: bool,
    pub solution: Vec<f64>,
    pub node_count: i64,
    pub total_lp_iterations: i64,
    pub total_repair_lp: i64,
    pub total_repair_lp_feasible: i64,
    pub total_repair_lp_iterations: i64,
    pub max_submip_level: i32,
    pub terminated: bool,
}

/// The LP of a sub-MIP, column-wise (the model's integrality applies).
pub(crate) struct SubMipLp {
    pub col_cost: Vec<f64>,
    pub row_lower: Vec<f64>,
    pub row_upper: Vec<f64>,
    pub a_start: Vec<usize>,
    pub a_index: Vec<usize>,
    pub a_value: Vec<f64>,
}

impl SubMipLp {
    /// The LP relaxation's LP, cuts included.
    pub fn of_lp(lp: &LpRelaxation) -> Self {
        let solver = lp.lp_solver();
        let (start, index, value) = solver.matrix();
        Self {
            col_cost: solver.col_cost().to_vec(),
            row_lower: solver.row_lower().to_vec(),
            row_upper: solver.row_upper().to_vec(),
            a_start: start.to_vec(),
            a_index: index.to_vec(),
            a_value: value.to_vec(),
        }
    }

    /// The model's LP.
    pub fn of_model(data: &MipData) -> Self {
        let model = &data.model;
        Self {
            col_cost: model.col_cost.clone(),
            row_lower: model.row_lower.clone(),
            row_upper: model.row_upper.clone(),
            a_start: model.a_start.clone(),
            a_index: model.a_index.clone(),
            a_value: model.a_value.clone(),
        }
    }
}

/// A sub-MIP to solve: the LP, its bounds, its basis and limits, with the
/// options HiGHS's `solveSubMip` sets.
pub(crate) struct SubMipRequest<'a> {
    pub lp: SubMipLp,
    pub basis: Option<&'a Basis>,
    pub col_lower: Vec<f64>,
    pub col_upper: Vec<f64>,
    pub max_leaves: i64,
    pub max_nodes: i64,
    pub stall_nodes: i64,
}

fn local_env<R>(dom: &mut Domain, data: &mut MipData, f: impl FnOnce(&mut Domain, &mut DomainEnv) -> R) -> R {
    let mut parts = data.parts();
    let mut env = DomainEnv {
        cx: &mut parts.cx,
        cliquetable: parts.cliquetable,
        implications: parts.implications,
    };
    f(dom, &mut env)
}

fn propagate(dom: &mut Domain, data: &mut MipData) {
    local_env(dom, data, |dom, env| dom.propagate(env));
}

/// Conflict analysis of an infeasible `dom` with the worker's pseudocosts.
fn conflict(dom: &mut Domain, data: &mut MipData) {
    let mut parts = data.parts();
    let mut env = DomainEnv {
        cx: &mut parts.cx,
        cliquetable: parts.cliquetable,
        implications: parts.implications,
    };
    dom.conflict_analysis(parts.domain, parts.pseudocost, &mut env);
}

fn fix_col(dom: &mut Domain, data: &mut MipData, col: usize, val: f64) {
    local_env(dom, data, |dom, env| dom.fix_col(col, val, Reason::branching(), env));
}

/// HiGHS's `HeuristicNeighbourhood`: the share of integral columns a dive
/// has fixed.
struct HeuristicNeighbourhood {
    num_fixed: usize,
    fixed_cols: HighsHashSet<usize>,
    start_checked_changes: usize,
    n_checked_changes: usize,
    num_total: usize,
}

impl HeuristicNeighbourhood {
    fn new(data: &MipData, localdom: &Domain) -> Self {
        let num_fixed = data.integral_cols.iter().filter(|&&i| localdom.col_lower[i] == localdom.col_upper[i]).count();
        let start = localdom.domain_change_stack().len();
        Self {
            num_fixed,
            fixed_cols: HighsHashSet::new(),
            start_checked_changes: start,
            n_checked_changes: start,
            num_total: data.integral_cols.len() - num_fixed,
        }
    }

    fn fixing_rate(&mut self, localdom: &Domain, data: &MipData) -> f64 {
        while self.n_checked_changes < localdom.domain_change_stack().len() {
            let col = localdom.domain_change_stack()[self.n_checked_changes].column;
            self.n_checked_changes += 1;
            if data.model.is_col_continuous(col) {
                continue;
            }
            if localdom.is_fixed(col) {
                self.fixed_cols.insert(col, ());
            }
        }
        let _ = self.num_fixed;
        if self.num_total != 0 {
            self.fixed_cols.len() as f64 / self.num_total as f64
        } else {
            0.0
        }
    }

    fn backtracked(&mut self) {
        self.n_checked_changes = self.start_checked_changes;
        if !self.fixed_cols.is_empty() {
            self.fixed_cols.clear();
        }
    }
}

/// HiGHS's `calcFixVal`.
fn calc_fix_val(rootchange: f64, fracval: f64, cost: f64) -> f64 {
    if rootchange >= 0.4 {
        fracval.ceil()
    } else if rootchange <= -0.4 {
        fracval.floor()
    } else if cost > 0.0 {
        fracval.ceil()
    } else if cost < 0.0 {
        fracval.floor()
    } else {
        (fracval + 0.5).floor()
    }
}

pub(crate) struct PrimalHeuristics {
    intcols: Vec<usize>,
    success_observations: f64,
    num_success_observations: i64,
    infeas_observations: f64,
    num_infeas_observations: i64,
    randgen: Random,
    /// The worker's statistics (HiGHS keeps them on the worker).
    pub stats: HeurStats,
}

impl PrimalHeuristics {
    pub fn new(data: &MipData) -> Self {
        Self {
            intcols: Vec::new(),
            success_observations: 0.0,
            num_success_observations: 0,
            infeas_observations: 0.0,
            num_infeas_observations: 0,
            randgen: Random::new(data.options.random_seed),
            stats: HeurStats::default(),
        }
    }

    /// HiGHS's `setupIntCols`.
    pub fn setup_int_cols(&mut self, data: &mut MipData) {
        self.intcols = data.integer_cols.clone();
        let feastol = data.feastol;
        let (uplocks, downlocks) = (&data.uplocks, &data.downlocks);
        let cliquetable = &data.cliquetable;
        pdqsort(&mut self.intcols, |&c1, &c2| {
            let lock_score1 = (feastol + uplocks[c1] as f64) * (feastol + downlocks[c1] as f64);
            let lock_score2 = (feastol + uplocks[c2] as f64) * (feastol + downlocks[c2] as f64);
            if lock_score1 > lock_score2 {
                return true;
            }
            if lock_score2 > lock_score1 {
                return false;
            }
            let clique_score1 = (feastol + cliquetable.num_implications_val(c1, true) as f64) * (feastol + cliquetable.num_implications_val(c1, false) as f64);
            let clique_score2 = (feastol + cliquetable.num_implications_val(c2, true) as f64) * (feastol + cliquetable.num_implications_val(c2, false) as f64);
            (clique_score1, hash_u64(c1 as u64), c1) > (clique_score2, hash_u64(c2 as u64), c2)
        });
    }

    fn num_success_observations(&self) -> i64 {
        self.num_success_observations + self.stats.num_success_observations
    }

    fn num_infeas_observations(&self) -> i64 {
        self.num_infeas_observations + self.stats.num_infeas_observations
    }

    fn success_observations(&self) -> f64 {
        self.success_observations + self.stats.success_observations
    }

    fn infeas_observations(&self) -> f64 {
        self.infeas_observations + self.stats.infeas_observations
    }

    /// HiGHS's `solveSubMip`: false if the sub-MIP was infeasible at its
    /// root or a terminator stopped it.
    #[allow(clippy::too_many_arguments)]
    fn solve_sub_mip(&mut self, data: &mut MipData, request: SubMipRequest, fixing_rate: f64) -> bool {
        let outcome = data.run_sub_mip(request);
        self.stats.max_submip_level = self.stats.max_submip_level.max(outcome.max_submip_level + 1);
        if outcome.terminated {
            self.stats.terminated = true;
            return false;
        }
        let num_unfixed = (data.integral_cols.len() + data.continuous_cols.len()) as f64;
        let adjustmentfactor = ((1.0 - fixing_rate) * data.integral_cols.len() as f64 + data.continuous_cols.len() as f64) / num_unfixed.max(1.0);
        let adjusted_lp_iterations = (adjustmentfactor * outcome.total_lp_iterations as f64) as usize as i64;
        self.stats.lp_iterations += adjusted_lp_iterations;
        self.stats.total_repair_lp += outcome.total_repair_lp;
        self.stats.total_repair_lp_feasible += outcome.total_repair_lp_feasible;
        self.stats.total_repair_lp_iterations += outcome.total_repair_lp_iterations;
        if data.submip {
            data.num_nodes += 1i64.max((adjustmentfactor * outcome.node_count as f64) as i64);
        }
        if outcome.infeasible {
            self.stats.infeas_observations += fixing_rate;
            self.stats.num_infeas_observations += 1;
        }
        if outcome.node_count <= 1 && outcome.infeasible {
            return false;
        }
        let old_upper_limit = data.upper_limit;
        if !outcome.infeasible && !outcome.solution.is_empty() {
            data.try_solution(&outcome.solution, SolutionSource::SubMip);
        }
        if data.upper_limit < old_upper_limit {
            self.stats.success_observations += fixing_rate;
            self.stats.num_success_observations += 1;
        }
        true
    }

    /// HiGHS's `determineTargetFixingRate`.
    fn determine_target_fixing_rate(&mut self) -> f64 {
        let mut low_fixing_rate = 0.6f64;
        let mut high_fixing_rate = 0.6f64;
        if self.num_infeas_observations() != 0 {
            let infeas_rate = self.infeas_observations() / self.num_infeas_observations() as f64;
            high_fixing_rate = 0.9 * infeas_rate;
            low_fixing_rate = low_fixing_rate.min(high_fixing_rate);
        }
        if self.num_success_observations() != 0 {
            let success_fixing_rate = self.success_observations() / self.num_success_observations() as f64;
            low_fixing_rate = low_fixing_rate.min(0.9 * success_fixing_rate);
            high_fixing_rate = (success_fixing_rate * 1.1).max(high_fixing_rate);
        }
        self.randgen.real(low_fixing_rate, high_fixing_rate)
    }

    /// HiGHS's `rootReducedCost`.
    pub fn root_reduced_cost(&mut self, lp: &LpRelaxation, data: &mut MipData) {
        let mut lurking_bounds = data.redcostfixing.lurking_bounds(data);
        if 10 * lurking_bounds.len() < data.integral_cols.len() {
            return;
        }
        pdqsort(&mut lurking_bounds, |a, b| a.0 > b.0);
        let mut localdom = data.domain.local_copy();
        let mut neighbourhood = HeuristicNeighbourhood::new(data, &localdom);
        let lower_bound = data.lower_bound + data.feastol;
        for &(curr_cutoff, domchg) in &lurking_bounds {
            if curr_cutoff <= lower_bound {
                break;
            }
            if localdom.is_active(&domchg) {
                continue;
            }
            local_env(&mut localdom, data, |dom, env| dom.change_bound(domchg, Reason::branching(), env));
            loop {
                propagate(&mut localdom, data);
                if localdom.infeasible() {
                    conflict(&mut localdom, data);
                    let new_lower = data.lower_bound.max(curr_cutoff);
                    data.update_lower_bound(new_lower);
                    local_env(&mut localdom, data, |dom, env| dom.backtrack(env));
                    if localdom.branch_depth() == 0 {
                        break;
                    }
                    neighbourhood.backtracked();
                    continue;
                }
                break;
            }
            if neighbourhood.fixing_rate(&localdom, data) >= 0.5 {
                break;
            }
        }
        let fixing_rate = neighbourhood.fixing_rate(&localdom, data);
        if fixing_rate < 0.3 {
            return;
        }
        let basis = data.firstrootbasis.clone();
        let max_nodes = 200 + (data.num_nodes / 20) as i32 as i64;
        let _ = lp;
        self.solve_sub_mip(
            data,
            SubMipRequest {
                lp: SubMipLp::of_model(data),
                basis: basis.as_ref(),
                col_lower: localdom.col_lower.clone(),
                col_upper: localdom.col_upper.clone(),
                max_leaves: 500,
                max_nodes,
                stall_nodes: 12,
            },
            fixing_rate,
        );
    }

    /// The integer columns not yet fixed globally (HiGHS keeps the pruned
    /// list).
    fn prune_intcols(&mut self, data: &MipData) {
        let globaldom = &data.domain;
        self.intcols.retain(|&i| !globaldom.is_fixed(i));
    }

    /// The heuristics' LP: a copy of `lp` with the global bounds, limited
    /// by the global upper limit so dual proofs stay valid.
    fn heuristic_lp(lp: &LpRelaxation, localdom: &Domain, data: &MipData) -> LpRelaxation {
        let mut heurlp = lp.copy(data);
        heurlp.set_objective_limit(data.upper_limit, data);
        heurlp.set_adjust_symmetric_branching_col(false);
        let n = data.model.num_col;
        heurlp
            .lp_solver_mut()
            .change_col_bounds((0..n).map(|col| (col, localdom.col_lower[col], localdom.col_upper[col])));
        heurlp
    }

    /// The end of RENS and RINS: a depth-first search, or a sub-MIP over
    /// the neighbourhood, retrying with a smaller fixing rate when the
    /// sub-MIP is infeasible at its root. Returns true to retry.
    fn finish_dive(
        &mut self,
        heur: &mut Search,
        heurlp: &mut LpRelaxation,
        neighbourhood: &mut HeuristicNeighbourhood,
        data: &mut MipData,
        targetdepth: &mut i64,
        maxfixingrate: &mut f64,
    ) -> bool {
        if !heur.has_node() {
            self.stats.lp_iterations += heur.lpiterations;
            return false;
        }
        let fixingrate = neighbourhood.fixing_rate(&heur.localdom, data);
        if fixingrate < 0.1 || (data.submip && data.num_improving_sols != 0) {
            heur.set_min_reliable(data, 0);
            heur.solve_depth_first(heurlp, data, 10);
            self.stats.lp_iterations += heur.lpiterations;
            if data.submip {
                data.num_nodes += heur.nnodes;
            }
            return false;
        }
        heurlp.remove_obsolete_rows(false, data);
        let max_nodes = 200 + data.num_nodes / 20;
        let basis = heurlp.lp_solver().basis().cloned();
        let solve_sub_mip_return = self.solve_sub_mip(
            data,
            SubMipRequest {
                lp: SubMipLp::of_lp(heurlp),
                basis: basis.as_ref(),
                col_lower: heur.localdom.col_lower.clone(),
                col_upper: heur.localdom.col_upper.clone(),
                max_leaves: 500,
                max_nodes,
                stall_nodes: 12,
            },
            fixingrate,
        );
        if self.stats.terminated {
            return false;
        }
        if !solve_sub_mip_return {
            let new_lp_iterations = self.stats.lp_iterations + heur.lpiterations;
            if new_lp_iterations + data.heuristic_lp_iterations > 100000 + ((data.total_lp_iterations - data.heuristic_lp_iterations - data.sb_lp_iterations) >> 1) {
                self.stats.lp_iterations = new_lp_iterations;
                return false;
            }
            *targetdepth = heur.current_depth() / 2;
            if *targetdepth <= 1 || data.check_limits(0) {
                self.stats.lp_iterations = new_lp_iterations;
                return false;
            }
            *maxfixingrate = fixingrate * 0.5;
            return true;
        }
        self.stats.lp_iterations += heur.lpiterations;
        false
    }

    /// Branch `col` towards `fixval` in a heuristic dive, propagating each
    /// branch: false if that made the domain infeasible.
    fn branch_to(heur: &mut Search, data: &mut MipData, col: usize, fixval: f64, up_point: f64, down_point: f64, propagate_each: bool, num_branched: &mut i64) -> bool {
        if heur.localdom.col_lower[col] < fixval {
            *num_branched += 1;
            heur.branch_upwards(col, fixval, up_point, data);
            if propagate_each {
                propagate(&mut heur.localdom, data);
            }
            if heur.localdom.infeasible() {
                conflict(&mut heur.localdom, data);
                return false;
            }
        }
        if heur.localdom.col_upper[col] > fixval {
            *num_branched += 1;
            heur.branch_downwards(col, fixval, down_point, data);
            if propagate_each {
                propagate(&mut heur.localdom, data);
            }
            if heur.localdom.infeasible() {
                conflict(&mut heur.localdom, data);
                return false;
            }
        }
        true
    }

    /// HiGHS's `RENS`.
    pub fn rens(&mut self, lp: &LpRelaxation, data: &mut MipData, _relaxationsol: &[f64]) {
        if data.domain.infeasible() {
            return;
        }
        let pscost = data.pseudocost.clone();
        let mut heur = Search::new(data, data.submip).with_pseudocost(pscost);
        heur.set_heuristic(true);
        self.prune_intcols(data);
        let mut heurlp = Self::heuristic_lp(lp, &heur.localdom, data);
        heur.localdom.clear_changed_cols();
        heur.create_new_node();
        let mut maxfixingrate = self.determine_target_fixing_rate();
        let mut fixingrate;
        let mut targetdepth: i64 = 1;
        let mut nbacktracks: i64 = -1;
        let mut neighbourhood = HeuristicNeighbourhood::new(data, &heur.localdom);
        loop {
            nbacktracks += 1;
            neighbourhood.backtracked();
            if heur.current_depth() > targetdepth && !heur.backtrack_until_depth(&mut heurlp, data, targetdepth) {
                self.stats.lp_iterations += heur.lpiterations;
                return;
            }
            loop {
                heur.evaluate_node(&mut heurlp, data);
                if heur.current_node_pruned() {
                    nbacktracks += 1;
                    if data.domain.infeasible() {
                        self.stats.lp_iterations += heur.lpiterations;
                        return;
                    }
                    if !heur.backtrack(&mut heurlp, data, true) {
                        break;
                    }
                    neighbourhood.backtracked();
                    continue;
                }
                fixingrate = neighbourhood.fixing_rate(&heur.localdom, data);
                if fixingrate >= maxfixingrate || nbacktracks >= 10 {
                    break;
                }
                let mut num_branched = 0i64;
                let stop_fixing_rate = (1.0 - (1.0 - neighbourhood.fixing_rate(&heur.localdom, data)) * 0.9).min(maxfixingrate);
                let relaxationsol = heurlp.lp_solver().col_value().expect("an LP solution").to_vec();
                for &i in &self.intcols {
                    if heur.localdom.col_lower[i] == heur.localdom.col_upper[i] {
                        continue;
                    }
                    let downval = (relaxationsol[i] + data.feastol).floor().min(heur.localdom.col_upper[i]);
                    let upval = (relaxationsol[i] - data.feastol).ceil().max(heur.localdom.col_lower[i]);
                    if heur.localdom.col_lower[i] < downval {
                        num_branched += 1;
                        heur.branch_upwards(i, downval, downval - 0.5, data);
                        propagate(&mut heur.localdom, data);
                        if heur.localdom.infeasible() {
                            conflict(&mut heur.localdom, data);
                            break;
                        }
                    }
                    if heur.localdom.col_upper[i] > upval {
                        num_branched += 1;
                        heur.branch_downwards(i, upval, upval + 0.5, data);
                        propagate(&mut heur.localdom, data);
                        if heur.localdom.infeasible() {
                            conflict(&mut heur.localdom, data);
                            break;
                        }
                    }
                    if neighbourhood.fixing_rate(&heur.localdom, data) >= stop_fixing_rate {
                        break;
                    }
                }
                if num_branched == 0 {
                    let rootlpsol = data.rootlpsol.clone();
                    let col_cost = data.model.col_cost.clone();
                    let get_fix_val = |heur: &Search, col: usize, fracval: f64| {
                        let rootchange = if rootlpsol.is_empty() { 0.0 } else { fracval - rootlpsol[col] };
                        calc_fix_val(rootchange, fracval, col_cost[col])
                            .min(heur.localdom.col_upper[col])
                            .max(heur.localdom.col_lower[col])
                    };
                    let nfrac = heurlp.fractional_integers().len() as u64;
                    {
                        let heur_ref = &heur;
                        let key = |a: &(usize, f64)| ((get_fix_val(heur_ref, a.0, a.1) - a.1).abs(), hash_u64(((a.0 as u64) << 32) + nfrac));
                        pdqsort(heurlp.fractional_integers_mut(), |a, b| key(a) < key(b));
                    }
                    let mut change = 0.0;
                    for (col, fracval) in heurlp.fractional_integers().to_vec() {
                        let fixval = get_fix_val(&heur, col, fracval);
                        if heur.localdom.col_lower[col] < fixval {
                            num_branched += 1;
                            heur.branch_upwards(col, fixval, fracval, data);
                            propagate(&mut heur.localdom, data);
                            if heur.localdom.infeasible() {
                                conflict(&mut heur.localdom, data);
                                break;
                            }
                            fixingrate = neighbourhood.fixing_rate(&heur.localdom, data);
                        }
                        if heur.localdom.col_upper[col] > fixval {
                            num_branched += 1;
                            heur.branch_downwards(col, fixval, fracval, data);
                            propagate(&mut heur.localdom, data);
                            if heur.localdom.infeasible() {
                                conflict(&mut heur.localdom, data);
                                break;
                            }
                            fixingrate = neighbourhood.fixing_rate(&heur.localdom, data);
                        }
                        if fixingrate >= maxfixingrate {
                            break;
                        }
                        change += (fixval - fracval).abs();
                        if change >= 0.5 {
                            break;
                        }
                    }
                }
                if num_branched == 0 {
                    break;
                }
                heurlp.flush_domain(&mut heur.localdom, false, &data.model);
            }
            if !self.finish_dive(&mut heur, &mut heurlp, &mut neighbourhood, data, &mut targetdepth, &mut maxfixingrate) {
                return;
            }
        }
    }

    /// HiGHS's `RINS`.
    pub fn rins(&mut self, lp: &LpRelaxation, data: &mut MipData, relaxationsol: &[f64]) {
        if data.domain.infeasible() || relaxationsol.len() != data.model.num_col {
            return;
        }
        self.prune_intcols(data);
        let pscost = data.pseudocost.clone();
        let mut heur = Search::new(data, data.submip).with_pseudocost(pscost);
        heur.set_heuristic(true);
        let mut heurlp = Self::heuristic_lp(lp, &heur.localdom, data);
        heur.localdom.clear_changed_cols();
        heur.create_new_node();
        let mut maxfixingrate = self.determine_target_fixing_rate();
        let minfixingrate = 0.25;
        let mut fixingrate;
        let mut nbacktracks: i64 = -1;
        let mut targetdepth: i64 = 1;
        let mut neighbourhood = HeuristicNeighbourhood::new(data, &heur.localdom);
        loop {
            nbacktracks += 1;
            neighbourhood.backtracked();
            if heur.current_depth() > targetdepth && !heur.backtrack_until_depth(&mut heurlp, data, targetdepth) {
                self.stats.lp_iterations += heur.lpiterations;
                return;
            }
            loop {
                heur.evaluate_node(&mut heurlp, data);
                if heur.current_node_pruned() {
                    nbacktracks += 1;
                    if data.domain.infeasible() {
                        self.stats.lp_iterations += heur.lpiterations;
                        return;
                    }
                    if !heur.backtrack(&mut heurlp, data, true) {
                        break;
                    }
                    neighbourhood.backtracked();
                    continue;
                }
                fixingrate = neighbourhood.fixing_rate(&heur.localdom, data);
                if fixingrate >= maxfixingrate || nbacktracks >= 10 {
                    break;
                }
                let feastol = data.feastol;
                let incumbent = data.incumbent.clone();
                let mut fixcandend = cpp_partition(heurlp.fractional_integers_mut(), |fracvar| {
                    (relaxationsol[fracvar.0] - incumbent[fracvar.0]).abs() <= feastol
                });
                let mut fixtolpsol = true;
                let rootlpsol = data.rootlpsol.clone();
                let col_cost = data.model.col_cost.clone();
                let get_fix_val = |heur: &Search, fixtolpsol: bool, col: usize, fracval: f64| {
                    let fixval = if fixtolpsol {
                        (relaxationsol[col] + 0.5).floor()
                    } else {
                        calc_fix_val(fracval - rootlpsol[col], fracval, col_cost[col])
                    };
                    fixval.min(heur.localdom.col_upper[col]).max(heur.localdom.col_lower[col])
                };
                let mut num_branched = 0i64;
                if fixcandend == 0 {
                    fixingrate = neighbourhood.fixing_rate(&heur.localdom, data);
                    let stop_fixing_rate = maxfixingrate.min(1.0 - (1.0 - fixingrate) * 0.9);
                    let currlpsol = heurlp.lp_solver().col_value().expect("an LP solution").to_vec();
                    for &i in &self.intcols {
                        if heur.localdom.col_lower[i] == heur.localdom.col_upper[i] {
                            continue;
                        }
                        if (currlpsol[i] - incumbent[i]).abs() <= feastol {
                            let fixval = nearest_integer(currlpsol[i]) as f64;
                            if heur.localdom.col_lower[i] < fixval {
                                num_branched += 1;
                                heur.branch_upwards(i, fixval, fixval - 0.5, data);
                                propagate(&mut heur.localdom, data);
                                if heur.localdom.infeasible() {
                                    conflict(&mut heur.localdom, data);
                                    break;
                                }
                                fixingrate = neighbourhood.fixing_rate(&heur.localdom, data);
                            }
                            if heur.localdom.col_upper[i] > fixval {
                                num_branched += 1;
                                heur.branch_downwards(i, fixval, fixval + 0.5, data);
                                propagate(&mut heur.localdom, data);
                                if heur.localdom.infeasible() {
                                    conflict(&mut heur.localdom, data);
                                    break;
                                }
                                fixingrate = neighbourhood.fixing_rate(&heur.localdom, data);
                            }
                            if fixingrate >= stop_fixing_rate {
                                break;
                            }
                        }
                    }
                    if num_branched != 0 {
                        heurlp.flush_domain(&mut heur.localdom, false, &data.model);
                        continue;
                    }
                    if fixingrate >= minfixingrate {
                        break;
                    }
                    fixcandend = heurlp.fractional_integers().len();
                    fixtolpsol = false;
                }
                let nfrac = heurlp.fractional_integers().len() as u64;
                {
                    let heur_ref = &heur;
                    let key = |a: &(usize, f64)| ((get_fix_val(heur_ref, fixtolpsol, a.0, a.1) - a.1).abs(), hash_u64(((a.0 as u64) << 32) + nfrac));
                    pdqsort(&mut heurlp.fractional_integers_mut()[..fixcandend], |a, b| key(a) < key(b));
                }
                let mut change = 0.0;
                for &(col, fracval) in &heurlp.fractional_integers()[..fixcandend] {
                    let fixval = get_fix_val(&heur, fixtolpsol, col, fracval);
                    if !Self::branch_to(&mut heur, data, col, fixval, fracval, fracval, false, &mut num_branched) {
                        break;
                    }
                    fixingrate = neighbourhood.fixing_rate(&heur.localdom, data);
                    if fixingrate >= maxfixingrate {
                        break;
                    }
                    change += (fixval - fracval).abs();
                    if change >= 0.5 {
                        break;
                    }
                }
                if num_branched == 0 {
                    break;
                }
                heurlp.flush_domain(&mut heur.localdom, false, &data.model);
            }
            if !self.finish_dive(&mut heur, &mut heurlp, &mut neighbourhood, data, &mut targetdepth, &mut maxfixingrate) {
                return;
            }
        }
    }

    /// A fresh LP of the model rows with `localdom`'s bounds, for the
    /// rounding heuristics.
    fn rounding_lp(&self, data: &MipData, localdom: &Domain, num_rounded: usize) -> LpRelaxation {
        let mut lprelax = LpRelaxation::new(data);
        lprelax.load_model(data);
        lprelax.set_iteration_limit(10000i64.max(2 * data.firstrootlpiters) as usize);
        let n = data.model.num_col;
        lprelax
            .lp_solver_mut()
            .change_col_bounds((0..n).map(|col| (col, localdom.col_lower[col], localdom.col_upper[col])));
        if data.options.mip_root_presolve_only {
            lprelax.lp_solver_mut().options.presolve = false;
        }
        if !data.options.mip_root_presolve_only && (5 * num_rounded) / n >= 1 {
            lprelax.lp_solver_mut().options.presolve = true;
        } else if let Some(basis) = &data.firstrootbasis {
            lprelax.lp_solver_mut().set_basis(basis);
        }
        lprelax
    }

    /// HiGHS's `tryRoundedPoint`.
    pub fn try_rounded_point(&mut self, data: &mut MipData, point: &[f64], source: SolutionSource) -> bool {
        let mut localdom = data.domain.local_copy();
        let mut integer_feasible = true;
        let numintcols = self.intcols.len();
        for i in 0..numintcols {
            let col = self.intcols[i];
            let rounded = point[col].round();
            let feasible = fractionality(point[col]) <= data.feastol;
            integer_feasible = integer_feasible && feasible;
            if !feasible {
                continue;
            }
            let intval = rounded.min(localdom.col_upper[col]).max(localdom.col_lower[col]);
            fix_col(&mut localdom, data, col, intval);
            if localdom.infeasible() {
                conflict(&mut localdom, data);
                return false;
            }
            propagate(&mut localdom, data);
            if localdom.infeasible() {
                conflict(&mut localdom, data);
                return false;
            }
        }
        if numintcols != data.model.num_col {
            let mut lprelax = self.rounding_lp(data, &localdom, numintcols);
            let st = lprelax.resolve_lp(LpDomain::None, data);
            if st == LpStatus::Infeasible {
                if let Some((mut inds, mut vals, mut rhs)) = lprelax.compute_dual_inf_proof() {
                    let mut cut_gen = CutGeneration::new(&lprelax, data);
                    cut_gen.generate_conflict(&localdom, &lprelax, data, &mut inds, &mut vals, &mut rhs);
                }
                return false;
            } else if st.unscaled_primal_feasible() {
                let lpsol = lprelax.lp_solver().col_value().expect("an LP solution").to_vec();
                if !integer_feasible {
                    self.zi_round(data, &lpsol);
                    return data.try_solution(&lpsol, source);
                } else {
                    data.add_incumbent(&lpsol, lprelax.objective(), source, true, false);
                    return true;
                }
            }
        }
        data.try_solution(&localdom.col_lower.clone(), source)
    }

    /// HiGHS's `linesearchRounding`.
    pub fn linesearch_rounding(&mut self, data: &mut MipData, point1: &[f64], point2: &[f64], source: SolutionSource) -> bool {
        let mut roundedpoint = vec![0.0; data.model.num_col];
        let mut alpha = 0.0;
        while alpha < 1.0 {
            let mut nextalpha = 1.0;
            let mut reachedpoint2 = true;
            for &col in &self.intcols {
                if data.uplocks[col] == 0 {
                    roundedpoint[col] = (point1[col].max(point2[col]) - data.feastol).ceil();
                    continue;
                }
                if data.downlocks[col] == 0 {
                    roundedpoint[col] = (point1[col].min(point2[col]) + data.feastol).floor();
                    continue;
                }
                let convexcomb = (1.0 - alpha) * point1[col] + alpha * point2[col];
                let intpoint2 = (point2[col] + 0.5).floor();
                roundedpoint[col] = (convexcomb + 0.5).floor();
                if roundedpoint[col] == intpoint2 {
                    continue;
                }
                reachedpoint2 = false;
                let tmpalpha = (roundedpoint[col] + 0.5 + data.feastol - point1[col]) / (point2[col] - point1[col]).abs();
                if tmpalpha < nextalpha && tmpalpha > alpha + 1e-2 {
                    nextalpha = tmpalpha;
                }
            }
            if self.try_rounded_point(data, &roundedpoint, source) {
                return true;
            }
            if reachedpoint2 {
                return false;
            }
            alpha = nextalpha;
        }
        false
    }

    /// HiGHS's `randomizedRounding`.
    pub fn randomized_rounding(&mut self, data: &mut MipData, relaxationsol: &[f64]) {
        if relaxationsol.len() != data.model.num_col {
            return;
        }
        let mut localdom = data.domain.local_copy();
        for idx in 0..self.intcols.len() {
            let i = self.intcols[idx];
            let intval = if data.uplocks[i] == 0 {
                (relaxationsol[i] - data.feastol).ceil()
            } else if data.downlocks[i] == 0 {
                (relaxationsol[i] + data.feastol).floor()
            } else {
                (relaxationsol[i] + self.randgen.real(0.1, 0.9)).floor()
            };
            let intval = intval.min(localdom.col_upper[i]).max(localdom.col_lower[i]);
            fix_col(&mut localdom, data, i, intval);
            if localdom.infeasible() {
                conflict(&mut localdom, data);
                return;
            }
            propagate(&mut localdom, data);
            if localdom.infeasible() {
                conflict(&mut localdom, data);
                return;
            }
        }
        if data.integer_cols.len() != data.model.num_col {
            let mut lprelax = self.rounding_lp(data, &localdom, self.intcols.len());
            let st = lprelax.resolve_lp(LpDomain::None, data);
            if st == LpStatus::Infeasible {
                if let Some((mut inds, mut vals, mut rhs)) = lprelax.compute_dual_inf_proof() {
                    let mut cut_gen = CutGeneration::new(&lprelax, data);
                    cut_gen.generate_conflict(&localdom, &lprelax, data, &mut inds, &mut vals, &mut rhs);
                }
            } else if st.unscaled_primal_feasible() {
                let sol = lprelax.lp_solver().col_value().expect("an LP solution").to_vec();
                data.add_incumbent(&sol, lprelax.objective(), SolutionSource::RandomizedRounding, true, false);
            }
        } else {
            data.try_solution(&localdom.col_lower.clone(), SolutionSource::RandomizedRounding);
        }
    }

    /// HiGHS's `shifting`.
    pub fn shifting(&mut self, lp: &LpRelaxation, data: &mut MipData, relaxationsol: &[f64]) {
        if relaxationsol.len() != data.model.num_col {
            return;
        }
        let mut current_relax_solution = relaxationsol.to_vec();
        let mut t: i64 = 0;
        let mut current_fractional_integers = lp.fractional_integers().to_vec();
        let mut current_infeasible_rows = data.infeasible_rows(&current_relax_solution);
        let mut previous_infeasible_rows_size = current_infeasible_rows.len();
        let mut has_infeasible_constraints = !current_infeasible_rows.is_empty();
        let mut iterations_without_reductions = 0;
        let max_iterations_without_reductions = 5;
        let mut shift_iterations_set: HashMap<usize, Vec<i64>> = HashMap::new();
        let feastol = data.feastol;
        let is_maximization = data.orig_sense < 0.0;
        let find_pair = |vec: &[(usize, f64)], k: usize| vec.iter().position(|p| p.0 == k);
        while (!current_fractional_integers.is_empty() || has_infeasible_constraints)
            && iterations_without_reductions <= max_iterations_without_reductions
            && t <= data.integer_cols.len() as i64
        {
            t += 1;
            let mut fractional_integers_reduced = false;
            iterations_without_reductions += 1;
            let model = &data.model;
            if has_infeasible_constraints {
                let mut fractional_integer_found = false;
                let mut r_index = 0;
                while !fractional_integer_found && r_index != current_infeasible_rows.len() {
                    let r = current_infeasible_rows[r_index].0;
                    for j_ind in model.ar_start[r]..model.ar_start[r + 1] {
                        fractional_integer_found = find_pair(&current_fractional_integers, model.ar_index[j_ind]).is_some();
                        if fractional_integer_found {
                            break;
                        }
                    }
                    r_index += 1;
                }
                let row_index = if fractional_integer_found {
                    r_index - 1
                } else {
                    self.randgen.integer(current_infeasible_rows.len())
                };
                let (row, row_sense, infeasibility) = current_infeasible_rows[row_index];
                let mut score_min = INF;
                let mut j_min: Option<usize> = None;
                let mut x_j_min = INF;
                let mut aij_min = 0.0;
                let mut move_value_up = false;
                for j_ind in model.ar_start[row]..model.ar_start[row + 1] {
                    let j = model.ar_index[j_ind];
                    if model.col_lower[j] == model.col_upper[j] {
                        continue;
                    }
                    let mut repair = |col: usize, direction: i64, num_locks: usize, coef: f64, cost: f64, is_integer: bool, is_at_bound: bool| {
                        if (row_sense < 0 || direction as f64 * coef > 0.0) && (row_sense > 0 || (direction as f64) * coef < 0.0) {
                            return;
                        }
                        if is_at_bound {
                            return;
                        }
                        let found = find_pair(&current_fractional_integers, col).is_some();
                        let score = if found {
                            -1.0 + 1.0 / (num_locks + 1) as f64
                        } else {
                            let mut score = match shift_iterations_set.get(&col) {
                                None => direction as f64 * if is_maximization { -cost } else { cost },
                                Some(shifts) if shifts.is_empty() => direction as f64 * if is_maximization { -cost } else { cost },
                                Some(shifts) => {
                                    let mut score = 0.0;
                                    for &shift in shifts {
                                        let shift = shift as f64;
                                        if direction as f64 * shift > 0.0 {
                                            score += 1.1f64.powf(direction as f64 * shift - t as f64);
                                        }
                                    }
                                    score
                                }
                            };
                            if is_integer {
                                score += 1.0;
                            }
                            score
                        };
                        if score < score_min {
                            score_min = score;
                            j_min = Some(col);
                            aij_min = coef;
                            x_j_min = current_relax_solution[col];
                            move_value_up = direction > 0;
                        }
                    };
                    let is_integer = model.is_col_integer(j);
                    repair(
                        j,
                        1,
                        data.downlocks[j],
                        model.ar_value[j_ind],
                        model.col_cost[j],
                        is_integer,
                        (model.col_upper[j] - current_relax_solution[j]).abs() <= feastol,
                    );
                    repair(
                        j,
                        -1,
                        data.uplocks[j],
                        model.ar_value[j_ind],
                        model.col_cost[j],
                        is_integer,
                        (current_relax_solution[j] - model.col_lower[j]).abs() <= feastol,
                    );
                }
                if let Some(j_min) = j_min {
                    if let Some(at) = find_pair(&current_fractional_integers, j_min) {
                        current_fractional_integers.remove(at);
                        fractional_integers_reduced = true;
                    }
                    if move_value_up {
                        if fractional_integers_reduced {
                            current_relax_solution[j_min] = (x_j_min - feastol).ceil();
                        } else {
                            current_relax_solution[j_min] = if model.is_col_integer(j_min) {
                                x_j_min + 1.0
                            } else {
                                (x_j_min + infeasibility / aij_min.abs()).min(model.col_upper[j_min] + feastol)
                            };
                            shift_iterations_set.entry(j_min).or_default().push(t);
                        }
                    } else if fractional_integers_reduced {
                        current_relax_solution[j_min] = (x_j_min + feastol).floor();
                    } else {
                        current_relax_solution[j_min] = if model.is_col_integer(j_min) {
                            x_j_min - 1.0
                        } else {
                            (x_j_min - infeasibility / aij_min.abs()).max(model.col_lower[j_min] - feastol)
                        };
                        shift_iterations_set.entry(j_min).or_default().push(-t);
                    }
                }
            } else {
                let mut xi_max = -1.0;
                let mut delta_c_min = INF;
                let mut pind_j_min: Option<usize> = None;
                let mut j_min = 0;
                let mut x_j_min = INF;
                let mut sigma = 0;
                for i in 0..current_fractional_integers.len() {
                    let it = current_fractional_integers[i];
                    let col = it.0;
                    let mut is_better = |xi: f64, roundedval: f64, direction: i32| {
                        let c_min = model.col_cost[col] * (roundedval - it.1);
                        if xi > xi_max || (xi == xi_max && c_min < delta_c_min) {
                            xi_max = xi;
                            delta_c_min = c_min;
                            pind_j_min = Some(i);
                            j_min = col;
                            x_j_min = roundedval;
                            sigma = direction;
                        }
                    };
                    is_better(data.uplocks[col] as f64, (it.1 + feastol).floor(), -1);
                    is_better(data.downlocks[col] as f64, (it.1 - feastol).ceil(), 1);
                }
                if sigma != 0 {
                    current_relax_solution[j_min] = x_j_min;
                }
                if let Some(at) = pind_j_min {
                    current_fractional_integers.remove(at);
                    fractional_integers_reduced = true;
                }
            }
            current_infeasible_rows = data.infeasible_rows(&current_relax_solution);
            has_infeasible_constraints = !current_infeasible_rows.is_empty();
            if current_infeasible_rows.len() < previous_infeasible_rows_size || fractional_integers_reduced {
                iterations_without_reductions = 0;
            }
            previous_infeasible_rows_size = current_infeasible_rows.len();
        }
        if has_infeasible_constraints {
            self.try_rounded_point(data, &current_relax_solution, SolutionSource::Shifting);
        } else if !current_fractional_integers.is_empty() {
            self.zi_round(data, &current_relax_solution);
        } else {
            data.try_solution(&current_relax_solution, SolutionSource::Shifting);
        }
    }

    /// HiGHS's `ziRound`.
    pub fn zi_round(&mut self, data: &mut MipData, relaxationsol: &[f64]) {
        if relaxationsol.len() != data.model.num_col {
            return;
        }
        let feastol = data.feastol;
        let zi = |x: f64| ((x - feastol).ceil() - x).min(x - (x + feastol).floor());
        let mut current_relax_solution = relaxationsol.to_vec();
        let mut zi_total = CDouble::new(0.0);
        for &i in &self.intcols {
            zi_total += zi(current_relax_solution[i]);
        }
        if zi_total <= feastol {
            return;
        }
        let model = &data.model;
        let num_row = model.num_row;
        let mut row_activities = vec![0.0; num_row];
        let mut loop_count = 0;
        let max_loop_count = 5;
        let mut improvement_in_feasibility = CDouble::new(INF);
        while zi_total > feastol && improvement_in_feasibility > feastol && loop_count <= max_loop_count {
            let previous_zi_total = zi_total;
            loop_count += 1;
            for &j in &self.intcols {
                let relax_solution = current_relax_solution[j];
                if fractionality(relax_solution) <= feastol {
                    continue;
                }
                // HiGHS's `calculateRowValuesQuad`, column by column.
                let mut quad = vec![CDouble::new(0.0); num_row];
                for col in 0..model.num_col {
                    for k in model.a_start[col]..model.a_start[col + 1] {
                        quad[model.a_index[k]] += current_relax_solution[col] * model.a_value[k];
                    }
                }
                for (activity, q) in row_activities.iter_mut().zip(&quad) {
                    *activity = q.value();
                }
                let mut min_row_ratio_for_upper = INF;
                let mut min_row_ratio_for_lower = INF;
                for el in model.a_start[j]..model.a_start[j + 1] {
                    let i = model.a_index[el];
                    let aij = model.a_value[el];
                    let slack_upper = model.row_upper[i] - row_activities[i];
                    let slack_lower = row_activities[i] - model.row_lower[i];
                    min_row_ratio_for_upper = min_row_ratio_for_upper.min((if aij > 0.0 { slack_upper } else { -slack_lower }) / aij);
                    min_row_ratio_for_lower = min_row_ratio_for_lower.min((if aij > 0.0 { slack_lower } else { -slack_upper }) / aij);
                }
                let upper_bound = (model.col_upper[j] - relax_solution).min(min_row_ratio_for_upper);
                let lower_bound = (relax_solution - model.col_lower[j]).min(min_row_ratio_for_lower);
                let mut perform_updates = |col: usize, change: f64| {
                    let old = current_relax_solution[col];
                    current_relax_solution[col] += change;
                    zi_total = zi_total - zi(old) + zi(current_relax_solution[col]);
                };
                let (zu, zl, z) = (zi(relax_solution + upper_bound), zi(relax_solution - lower_bound), zi(relax_solution));
                if (zu - zl).abs() <= feastol && zu < z {
                    let xcol_cost = model.col_cost[j];
                    let ub_obj_change_smaller = xcol_cost * (relax_solution + upper_bound) <= xcol_cost * (relax_solution - lower_bound);
                    // The model is always minimised here.
                    if ub_obj_change_smaller {
                        perform_updates(j, upper_bound);
                    } else {
                        perform_updates(j, -lower_bound);
                    }
                } else if zu < zl && zu < z {
                    perform_updates(j, upper_bound);
                } else if zu > zl && zl < z {
                    perform_updates(j, -lower_bound);
                }
            }
            improvement_in_feasibility = previous_zi_total - zi_total;
        }
        data.try_solution(&current_relax_solution, SolutionSource::ZiRound);
    }

    /// HiGHS's `feasibilityPump`.
    pub fn feasibility_pump(&mut self, lp: &LpRelaxation, data: &mut MipData) {
        let mut lprelax = lp.copy(data);
        let mut referencepoints: HashSet<Vec<i64>> = HashSet::new();
        let mut status = lprelax.resolve_lp(LpDomain::None, data);
        self.stats.lp_iterations += lprelax.num_lp_iterations();
        let n = data.model.num_col;
        let mut cost = vec![0.0; n];
        lprelax.lp_solver_mut().options.simplex_strategy = crate::simplex::options::SimplexStrategy::Primal;
        lprelax.set_objective_limit(INF, data);
        lprelax.lp_solver_mut().options.primal_simplex_bound_perturbation_multiplier = 0.0;
        lprelax.set_iteration_limit((5.0 * data.avgrootlpiters) as i32 as usize);
        while !lprelax.fractional_integers().is_empty() {
            let lpsol = lprelax.lp_solver().col_value().expect("an LP solution").to_vec();
            let mut roundedsol = lpsol.clone();
            let mut referencepoint: Vec<i64> = Vec::with_capacity(data.integer_cols.len());
            let mut localdom = data.domain.local_copy();
            for &i in &data.integer_cols.clone() {
                let intval = (roundedsol[i] + self.randgen.real(0.4, 0.6)).floor().max(localdom.col_lower[i]).min(localdom.col_upper[i]);
                roundedsol[i] = intval;
                referencepoint.push(intval as i32 as i64);
                if !localdom.infeasible() {
                    fix_col(&mut localdom, data, i, intval);
                    if localdom.infeasible() {
                        conflict(&mut localdom, data);
                        continue;
                    }
                    propagate(&mut localdom, data);
                    if localdom.infeasible() {
                        conflict(&mut localdom, data);
                        continue;
                    }
                }
            }
            let mut havecycle = !referencepoints.insert(referencepoint.clone());
            let mut k = 0;
            while havecycle && k < 2 {
                for _ in 0..10 {
                    let flippos = self.randgen.integer(data.integer_cols.len());
                    let col = data.integer_cols[flippos];
                    roundedsol[col] = if roundedsol[col] > lpsol[col] {
                        lpsol[col].floor() as i32 as f64
                    } else if roundedsol[col] < lpsol[col] {
                        lpsol[col].ceil() as i32 as f64
                    } else if roundedsol[col] < data.domain.col_upper[col] {
                        data.domain.col_upper[col]
                    } else {
                        data.domain.col_lower[col]
                    };
                    referencepoint[flippos] = roundedsol[col] as i32 as i64;
                }
                havecycle = !referencepoints.insert(referencepoint.clone());
                k += 1;
            }
            if havecycle {
                return;
            }
            if self.linesearch_rounding(data, &lpsol, &roundedsol, SolutionSource::FeasibilityPump) {
                return;
            }
            if lprelax.num_lp_iterations() as f64 >= 1000.0 + data.avgrootlpiters * 5.0 {
                break;
            }
            for &i in &data.integer_cols.clone() {
                cost[i] = if data.uplocks[i] == 0 || data.downlocks[i] == 0 {
                    0.0
                } else if lpsol[i] > roundedsol[i] - data.feastol {
                    -1.0 + self.randgen.real(-1e-4, 1e-4)
                } else {
                    1.0 + self.randgen.real(-1e-4, 1e-4)
                };
            }
            lprelax.lp_solver_mut().change_col_costs(cost.iter().copied().enumerate());
            let mut niters = -lprelax.num_lp_iterations();
            status = lprelax.resolve_lp(LpDomain::None, data);
            niters += lprelax.num_lp_iterations();
            if niters == 0 {
                break;
            }
            self.stats.lp_iterations += niters;
        }
        if lprelax.fractional_integers().is_empty() && status.unscaled_primal_feasible() {
            let sol = lprelax.lp_solver().col_value().expect("an LP solution").to_vec();
            data.add_incumbent(&sol, lprelax.objective(), SolutionSource::FeasibilityPump, true, false);
        }
    }

    /// HiGHS's `centralRounding`.
    pub fn central_rounding(&mut self, data: &mut MipData) {
        if data.analytic_center.len() != data.model.num_col {
            return;
        }
        let center = data.analytic_center.clone();
        let start = if !data.firstlpsol.is_empty() {
            data.firstlpsol.clone()
        } else if !data.rootlpsol.is_empty() {
            data.rootlpsol.clone()
        } else {
            center.clone()
        };
        self.linesearch_rounding(data, &start, &center, SolutionSource::CentralRounding);
    }

    /// HiGHS's `flushStatistics`.
    pub fn flush_statistics(&mut self, data: &mut MipData) {
        let stats = std::mem::take(&mut self.stats);
        data.total_repair_lp += stats.total_repair_lp;
        data.total_repair_lp_feasible += stats.total_repair_lp_feasible;
        data.total_repair_lp_iterations += stats.total_repair_lp_iterations;
        data.heuristic_lp_iterations += stats.lp_iterations;
        data.total_lp_iterations += stats.lp_iterations;
        data.max_submip_level = data.max_submip_level.max(stats.max_submip_level);
        if stats.terminated {
            data.terminated = true;
        }
        self.success_observations += stats.success_observations;
        self.num_success_observations += stats.num_success_observations;
        self.infeas_observations += stats.infeas_observations;
        self.num_infeas_observations += stats.num_infeas_observations;
    }
}
