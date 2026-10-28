//! HiGHS's `HighsSearch`: depth-first branch and bound below a node of the
//! queue, with reliability branching by strong branching and
//! pseudocosts, conflict analysis on pruned nodes, and plunging.

use std::rc::Rc;

use super::{
    BoundType, DomainChange,
    cut_generation::CutGeneration,
    data::{MipData, SolutionSource},
    domain::{Domain, DomainEnv, Reason},
    lp_relaxation::{LpDomain, LpRelaxation, LpStatus},
    node_queue::OpenNode,
    redcost_fixing::RedcostFixing,
    symmetry::{StabilizerOrbitWorkspace, StabilizerOrbits},
};
use crate::{
    Basis,
    hash_table::HashTable,
    integers::fractionality,
    simplex::ModelStatus,
    util::{CDouble, Random, ldexp},
};

const INF: f64 = f64::INFINITY;

/// The search's pseudocosts: its own copy in a heuristic dive, otherwise
/// the solver's.
macro_rules! pc {
    ($search:expr, $data:expr) => {
        match &mut $search.own_pseudocost {
            Some(own) => own,
            None => &mut $data.pseudocost,
        }
    };
}

/// HiGHS's `ChildSelectionRule`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChildSelectionRule {
    Up,
    Down,
    RootSol,
    Obj,
    Random,
    BestCost,
    WorstCost,
    Disjunction,
    HybridInferenceCost,
}

/// HiGHS's `NodeResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NodeResult {
    BoundExceeding,
    DomainInfeasible,
    LpInfeasible,
    Branched,
    SubOptimal,
    Open,
}

/// HiGHS's `NodeData`.
#[derive(Clone, Debug)]
struct NodeData {
    lower_bound: f64,
    estimate: f64,
    branching_point: f64,
    lp_objective: f64,
    other_child_lb: f64,
    node_basis: Option<Rc<Basis>>,
    stabilizer_orbits: Option<Rc<StabilizerOrbits>>,
    branchingdecision: DomainChange,
    /// The branching column, `None` before branching (HiGHS's -1).
    branching_col: Option<usize>,
    domchg_stack_pos: Option<usize>,
    skip_depth_count: u8,
    opensubtrees: u8,
}

impl NodeData {
    fn new(parentlb: f64, parentestimate: f64, parent_basis: Option<Rc<Basis>>, stabilizer_orbits: Option<Rc<StabilizerOrbits>>) -> Self {
        Self {
            lower_bound: parentlb,
            estimate: parentestimate,
            branching_point: 0.0,
            lp_objective: -INF,
            other_child_lb: parentlb,
            node_basis: parent_basis,
            stabilizer_orbits,
            branchingdecision: DomainChange::new(0.0, 0, BoundType::Lower),
            branching_col: None,
            domchg_stack_pos: None,
            skip_depth_count: 0,
            opensubtrees: 2,
        }
    }

    fn decide(&mut self, col: usize, boundval: f64, boundtype: BoundType) {
        self.branching_col = Some(col);
        self.branchingdecision = DomainChange::new(boundval, col, boundtype);
    }
}

const UP_RELIABLE: i32 = 1;
const DOWN_RELIABLE: i32 = 2;
const RELIABLE: i32 = UP_RELIABLE | DOWN_RELIABLE;

/// HiGHS's `HighsLpRelaxation::Playground`: strong branching solves from a
/// saved simplex iterate, restored when it ends.
struct Playground {
    iterate_stored: bool,
}

impl Playground {
    fn solve_lp(&mut self, lp: &mut LpRelaxation, localdom: &mut Domain, data: &mut MipData) -> LpStatus {
        if self.iterate_stored {
            lp.flush_domain(localdom, false, &data.model);
            lp.lp_solver_mut().get_iterate();
        } else {
            lp.lp_solver_mut().put_iterate();
            lp.flush_domain(localdom, false, &data.model);
            self.iterate_stored = true;
        }
        lp.run(false, data)
    }

    fn end(self, lp: &mut LpRelaxation, data: &mut MipData) {
        if self.iterate_stored {
            lp.lp_solver_mut().get_iterate();
            lp.run(true, data);
        }
    }
}

pub(crate) struct Search {
    pub localdom: Domain,
    pub random: Random,
    pub nnodes: i64,
    pub nleaves: i64,
    pub lpiterations: i64,
    pub heurlpiterations: i64,
    pub sblpiterations: i64,
    pub upper_limit: f64,
    pub treeweight: CDouble,
    inds: Vec<usize>,
    vals: Vec<f64>,
    depthoffset: i64,
    inbranching: bool,
    inheuristic: bool,
    count_tree_weight: bool,
    childselrule: ChildSelectionRule,
    subrootsol: Vec<f64>,
    nodestack: Vec<NodeData>,
    stabilizer_orbit_workspace: StabilizerOrbitWorkspace,
    reliableatnode: HashTable<usize, i32>,
    /// HiGHS's `pseudocost` member when it is not the worker's (the
    /// heuristics search with a copy).
    own_pseudocost: Option<super::pseudocost::Pseudocost>,
}

/// Run `f` on `localdom` with what its operations read from `data`.
fn on_local<R>(localdom: &mut Domain, data: &mut MipData, f: impl FnOnce(&mut Domain, &mut DomainEnv) -> R) -> R {
    let mut parts = data.parts();
    let mut env = DomainEnv {
        cx: &mut parts.cx,
        cliquetable: parts.cliquetable,
        implications: parts.implications,
    };
    f(localdom, &mut env)
}

impl Search {
    /// HiGHS's constructor, for the main search (not a sub-MIP).
    pub fn new(data: &mut MipData, submip: bool) -> Self {
        let mut localdom = data.domain.local_copy();
        on_local(&mut localdom, data, |dom, env| dom.set_domain_change_stack(&[], env));
        Self {
            localdom,
            random: Random::new(0),
            nnodes: 0,
            nleaves: 0,
            lpiterations: 0,
            heurlpiterations: 0,
            sblpiterations: 0,
            upper_limit: INF,
            treeweight: CDouble::new(0.0),
            inds: Vec::new(),
            vals: Vec::new(),
            depthoffset: 0,
            inbranching: false,
            inheuristic: false,
            count_tree_weight: true,
            childselrule: if submip { ChildSelectionRule::HybridInferenceCost } else { ChildSelectionRule::RootSol },
            subrootsol: Vec::new(),
            nodestack: Vec::new(),
            stabilizer_orbit_workspace: StabilizerOrbitWorkspace::default(),
            reliableatnode: HashTable::new(),
            own_pseudocost: None,
        }
    }

    fn reliable_flags(&self, col: usize) -> i32 {
        self.reliableatnode.find(&col).copied().unwrap_or(0)
    }

    fn mark_up_reliable(&mut self, col: usize) {
        *self.reliableatnode.entry(col) |= UP_RELIABLE;
    }

    fn mark_down_reliable(&mut self, col: usize) {
        *self.reliableatnode.entry(col) |= DOWN_RELIABLE;
    }

    fn propagate(&mut self, data: &mut MipData) {
        on_local(&mut self.localdom, data, |dom, env| dom.propagate(env));
    }

    fn change_bound(&mut self, data: &mut MipData, domchg: DomainChange) {
        on_local(&mut self.localdom, data, |dom, env| dom.change_bound(domchg, Reason::branching(), env));
    }

    fn backtrack_local(&mut self, data: &mut MipData) -> Option<DomainChange> {
        on_local(&mut self.localdom, data, |dom, env| dom.backtrack(env))
    }

    fn conflict_analysis(&mut self, data: &mut MipData) {
        let mut parts = data.parts();
        let mut env = DomainEnv {
            cx: &mut parts.cx,
            cliquetable: parts.cliquetable,
            implications: parts.implications,
        };
        self.localdom.conflict_analysis(
            parts.domain,
            match &mut self.own_pseudocost {
                Some(own) => own,
                None => parts.pseudocost,
            },
            &mut env,
        );
    }

    /// Orbital fixing in the stabilizer `orbits`, or orbitope propagation
    /// without them.
    fn orbital_fixing(&mut self, data: &mut MipData, orbits: Option<&Rc<StabilizerOrbits>>) {
        let mut parts = data.parts();
        let mut env = DomainEnv {
            cx: &mut parts.cx,
            cliquetable: parts.cliquetable,
            implications: parts.implications,
        };
        match orbits {
            Some(orbits) => {
                orbits.orbital_fixing(parts.symmetries, &mut self.localdom, &mut env);
            }
            None => {
                parts.symmetries.propagate_orbitopes(&mut self.localdom, &mut env);
            }
        }
    }

    pub fn set_heuristic(&mut self, inheuristic: bool) {
        self.inheuristic = inheuristic;
        if inheuristic {
            self.childselrule = ChildSelectionRule::HybridInferenceCost;
        }
    }

    pub fn has_node(&self) -> bool {
        !self.nodestack.is_empty()
    }

    pub fn current_node_pruned(&self) -> bool {
        self.nodestack.last().expect("a node").opensubtrees == 0
    }

    pub fn current_estimate(&self) -> f64 {
        self.nodestack.last().expect("a node").estimate
    }

    pub fn current_lower_bound(&self) -> f64 {
        self.nodestack.last().expect("a node").lower_bound
    }

    pub fn current_depth(&self) -> i64 {
        self.nodestack.len() as i64 + self.depthoffset
    }

    /// HiGHS's `checkSol`: the objective of `sol` and whether it is integral.
    pub fn check_sol(&self, sol: &[f64], data: &MipData) -> (f64, bool) {
        let mut objval = CDouble::new(0.0);
        let mut integerfeasible = true;
        for i in 0..data.model.num_col {
            objval += sol[i] * data.model.col_cost[i];
            if !integerfeasible || !data.model.is_col_integer(i) {
                continue;
            }
            if fractionality(sol[i]) > data.feastol {
                integerfeasible = false;
            }
        }
        (objval.value(), integerfeasible)
    }

    /// HiGHS's `orbitsValidInChildNode`.
    fn orbits_valid_in_child_node(&self, branch_chg: &DomainChange, data: &MipData) -> bool {
        let curr_node = self.nodestack.last().expect("a node");
        let Some(orbits) = &curr_node.stabilizer_orbits else { return true };
        if orbits.orbit_cols.is_empty() || orbits.is_stabilized(&data.symmetries, branch_chg.column) {
            return true;
        }
        let col = branch_chg.column;
        branch_chg.boundtype == BoundType::Upper && data.model.is_col_integral(col) && data.model.col_lower[col] == 0.0 && data.model.col_upper[col] == 1.0
    }

    /// HiGHS's `getCutoffBound`.
    pub fn cutoff_bound(&self, data: &MipData) -> f64 {
        data.upper_limit.min(self.upper_limit)
    }

    /// HiGHS's `setRINSNeighbourhood`.
    pub fn set_rins_neighbourhood(&mut self, basesol: &[f64], relaxsol: &[f64], data: &mut MipData) {
        for i in 0..data.model.num_col {
            if !data.model.is_col_integer(i) || self.localdom.col_lower[i] == self.localdom.col_upper[i] {
                continue;
            }
            let intval = (basesol[i] + 0.5).floor();
            if (relaxsol[i] - intval).abs() < data.feastol {
                if self.localdom.col_lower[i] < intval {
                    let val = intval.min(self.localdom.col_upper[i]);
                    on_local(&mut self.localdom, data, |dom, env| {
                        dom.change_bound(DomainChange::new(val, i, BoundType::Lower), Reason::unspecified(), env)
                    });
                }
                if self.localdom.col_upper[i] > intval {
                    let val = intval.max(self.localdom.col_lower[i]);
                    on_local(&mut self.localdom, data, |dom, env| {
                        dom.change_bound(DomainChange::new(val, i, BoundType::Upper), Reason::unspecified(), env)
                    });
                }
            }
        }
    }

    /// HiGHS's `setRENSNeighbourhood`.
    pub fn set_rens_neighbourhood(&mut self, lpsol: &[f64], data: &mut MipData) {
        for i in 0..data.model.num_col {
            if !data.model.is_col_integer(i) || self.localdom.col_lower[i] == self.localdom.col_upper[i] {
                continue;
            }
            let downval = (lpsol[i] + data.feastol).floor();
            let upval = (lpsol[i] - data.feastol).ceil();
            if self.localdom.col_lower[i] < downval {
                let val = downval.min(self.localdom.col_upper[i]);
                on_local(&mut self.localdom, data, |dom, env| {
                    dom.change_bound(DomainChange::new(val, i, BoundType::Lower), Reason::unspecified(), env)
                });
                if self.localdom.infeasible() {
                    return;
                }
            }
            if self.localdom.col_upper[i] > upval {
                let val = upval.max(self.localdom.col_lower[i]);
                on_local(&mut self.localdom, data, |dom, env| {
                    dom.change_bound(DomainChange::new(val, i, BoundType::Upper), Reason::unspecified(), env)
                });
                if self.localdom.infeasible() {
                    return;
                }
            }
        }
    }

    /// HiGHS's `createNewNode`.
    pub fn create_new_node(&mut self) {
        let mut node = NodeData::new(-INF, -INF, None, None);
        node.domchg_stack_pos = Some(self.localdom.domain_change_stack().len());
        self.nodestack.push(node);
    }

    /// HiGHS's `cutoffNode`.
    pub fn cutoff_node(&mut self) {
        self.nodestack.last_mut().expect("a node").opensubtrees = 0;
    }

    pub fn set_min_reliable(&mut self, data: &mut MipData, minreliable: i64) {
        pc!(self, data).set_min_reliable(minreliable);
    }

    /// Search with `pseudocost` instead of the solver's (HiGHS's
    /// `HighsSearch(worker, pscost)` with a copy).
    pub fn with_pseudocost(mut self, pseudocost: super::pseudocost::Pseudocost) -> Self {
        self.own_pseudocost = Some(pseudocost);
        self
    }

    /// HiGHS's `branchDownwards` / `branchUpwards`.
    fn branch_towards(&mut self, col: usize, newbound: f64, branchpoint: f64, boundtype: BoundType, data: &mut MipData) {
        let decision = DomainChange::new(newbound, col, boundtype);
        let (lower_bound, estimate, node_basis, orbits) = {
            let currnode = self.nodestack.last_mut().expect("a node");
            currnode.opensubtrees = 1;
            currnode.branching_point = branchpoint;
            currnode.decide(col, newbound, boundtype);
            (currnode.lower_bound, currnode.estimate, currnode.node_basis.clone(), currnode.stabilizer_orbits.clone())
        };
        let domchg_pos = self.localdom.domain_change_stack().len();
        let pass_stabilizer = self.orbits_valid_in_child_node(&decision, data);
        self.change_bound(data, decision);
        let mut child = NodeData::new(lower_bound, estimate, node_basis, if pass_stabilizer { orbits } else { None });
        child.domchg_stack_pos = Some(domchg_pos);
        self.nodestack.push(child);
    }

    pub fn branch_downwards(&mut self, col: usize, newub: f64, branchpoint: f64, data: &mut MipData) {
        self.branch_towards(col, newub, branchpoint, BoundType::Upper, data);
    }

    pub fn branch_upwards(&mut self, col: usize, newlb: f64, branchpoint: f64, data: &mut MipData) {
        self.branch_towards(col, newlb, branchpoint, BoundType::Lower, data);
    }

    /// HiGHS's `addBoundExceedingConflict`.
    fn add_bound_exceeding_conflict(&mut self, lp: &LpRelaxation, data: &mut MipData) {
        if data.upper_limit != INF {
            let upper_limit = data.upper_limit;
            if let Some((inds, vals, rhs)) = lp.compute_dual_proof(upper_limit, true, data) {
                self.inds = inds;
                self.vals = vals;
                if data.domain.infeasible() {
                    return;
                }
                {
                    let mut parts = data.parts();
                    let mut env = DomainEnv {
                        cx: &mut parts.cx,
                        cliquetable: parts.cliquetable,
                        implications: parts.implications,
                    };
                    self.localdom.conflict_analysis_with_proof(
                        &self.inds,
                        &self.vals,
                        rhs,
                        parts.domain,
                        match &mut self.own_pseudocost {
                            Some(own) => own,
                            None => parts.pseudocost,
                        },
                        &mut env,
                    );
                }
                let mut cut_gen = CutGeneration::new(lp, data);
                let mut rhs = rhs;
                cut_gen.generate_conflict(&self.localdom, lp, data, &mut self.inds, &mut self.vals, &mut rhs);
            }
        }
    }

    /// HiGHS's `addInfeasibleConflict`.
    fn add_infeasible_conflict(&mut self, lp: &mut LpRelaxation, data: &mut MipData) {
        if lp.lp_solver().status() == ModelStatus::ObjectiveBound {
            lp.perform_aging(false, data);
        }
        if let Some((inds, vals, rhs)) = lp.compute_dual_inf_proof() {
            self.inds = inds;
            self.vals = vals;
            if data.domain.infeasible() {
                return;
            }
            {
                let mut parts = data.parts();
                let mut env = DomainEnv {
                    cx: &mut parts.cx,
                    cliquetable: parts.cliquetable,
                    implications: parts.implications,
                };
                self.localdom.conflict_analysis_with_proof(
                    &self.inds,
                    &self.vals,
                    rhs,
                    parts.domain,
                    match &mut self.own_pseudocost {
                        Some(own) => own,
                        None => parts.pseudocost,
                    },
                    &mut env,
                );
            }
            let mut cut_gen = CutGeneration::new(lp, data);
            let mut rhs = rhs;
            cut_gen.generate_conflict(&self.localdom, lp, data, &mut self.inds, &mut self.vals, &mut rhs);
        }
    }

    pub fn heuristic_lp_iterations(&self, data: &MipData) -> i64 {
        self.heurlpiterations + data.heuristic_lp_iterations
    }

    pub fn total_lp_iterations(&self, data: &MipData) -> i64 {
        self.lpiterations + data.total_lp_iterations
    }

    pub fn strong_branching_lp_iterations(&self, data: &MipData) -> i64 {
        self.sblpiterations + data.sb_lp_iterations
    }

    /// HiGHS's `addIncumbent` (the main search, not a parallel worker).
    fn add_incumbent(&self, sol: &[f64], solobj: f64, source: SolutionSource, data: &mut MipData) -> bool {
        data.add_incumbent(sol, solobj, source, true, false)
    }

    /// HiGHS's `selectBranchingCandidate`: the fractional integer to branch
    /// on (`None` when strong branching settled the node), with the lower
    /// bounds of its two children.
    pub fn select_branching_candidate(&mut self, lp: &mut LpRelaxation, data: &mut MipData, max_sb_iters: i64, down_node_lb: &mut f64, up_node_lb: &mut f64) -> Option<usize> {
        let fracints = lp.fractional_integers().to_vec();
        let numfrac = fracints.len();
        let feastol = data.feastol;
        let lower = self.current_lower_bound();
        let mut upscore = vec![INF; numfrac];
        let mut downscore = vec![INF; numfrac];
        let mut upbound = vec![lower; numfrac];
        let mut downbound = vec![lower; numfrac];
        let mut upscorereliable = vec![false; numfrac];
        let mut downscorereliable = vec![false; numfrac];
        for k in 0..numfrac {
            let (col, fracval) = fracints[k];
            if pc!(self, data).is_reliable(col) {
                upscore[k] = pc!(self, data).pseudocost_up(col, fracval);
                downscore[k] = pc!(self, data).pseudocost_down(col, fracval);
                upscorereliable[k] = true;
                downscorereliable[k] = true;
            } else {
                let flags = self.reliable_flags(col);
                if flags & UP_RELIABLE != 0 {
                    upscore[k] = pc!(self, data).pseudocost_up(col, fracval);
                    upscorereliable[k] = true;
                }
                if flags & DOWN_RELIABLE != 0 {
                    downscore[k] = pc!(self, data).pseudocost_down(col, fracval);
                    downscorereliable[k] = true;
                }
            }
        }
        let evalqueue: Vec<usize> = (0..numfrac).collect();
        let mut min_score = feastol;
        let mut playground = Playground { iterate_stored: false };
        let result = loop {
            let must_stop = self.strong_branching_lp_iterations(data) >= max_sb_iters || data.check_limits(0);
            // `selectBestScore`.
            let candidate = {
                let mut best = None;
                let mut bestscore = -1.0;
                let mut bestnodes = -1.0;
                let mut bestnumnodes = 0i64;
                let oldminscore = min_score;
                let nodequeue = data.nodequeue.borrow();
                for &k in &evalqueue {
                    if upscore[k] <= oldminscore {
                        upscorereliable[k] = true;
                    }
                    if downscore[k] <= oldminscore {
                        downscorereliable[k] = true;
                    }
                    let s = 1e-3 * (if upscorereliable[k] { upscore[k] } else { 0.0 }).min(if downscorereliable[k] { downscore[k] } else { 0.0 });
                    min_score = s.max(min_score);
                    let score = if upscore[k] <= oldminscore || downscore[k] <= oldminscore {
                        pc!(self, data).score_with_costs(fracints[k].0, upscore[k].min(oldminscore), downscore[k].min(oldminscore))
                    } else if upscore[k] == INF || downscore[k] == INF {
                        if must_stop { pc!(self, data).score(fracints[k].0, fracints[k].1) } else { INF }
                    } else {
                        pc!(self, data).score_with_costs(fracints[k].0, upscore[k], downscore[k])
                    };
                    let upnodes = nodequeue.num_nodes_up(fracints[k].0);
                    let downnodes = nodequeue.num_nodes_down(fracints[k].0);
                    let mut nodes = 0.0;
                    let numnodes = upnodes + downnodes;
                    if upnodes != 0 || downnodes != 0 {
                        nodes = (downnodes as f64 / numnodes as f64) * (upnodes as f64 / numnodes as f64);
                    }
                    if score > bestscore || (score > bestscore - feastol && (nodes, numnodes) > (bestnodes, bestnumnodes)) {
                        bestscore = score;
                        best = Some(k);
                        bestnodes = nodes;
                        bestnumnodes = numnodes;
                    }
                }
                best.expect("a candidate")
            };
            if (upscorereliable[candidate] && downscorereliable[candidate]) || must_stop {
                *down_node_lb = downbound[candidate];
                *up_node_lb = upbound[candidate];
                break Some(candidate);
            }
            lp.set_objective_limit(data.upper_limit, data);
            let (col, fracval) = fracints[candidate];
            let upval = fracval.ceil();
            let downval = fracval.floor();
            let evaluate_up = !(!downscorereliable[candidate]
                && (upscorereliable[candidate]
                    || (downscore[candidate], pc!(self, data).avg_inferences_down(col)) >= (upscore[candidate], pc!(self, data).avg_inferences_up(col))));
            let mut scores = Scores {
                upscore: &mut upscore,
                downscore: &mut downscore,
                upbound: &mut upbound,
                downbound: &mut downbound,
                upscorereliable: &mut upscorereliable,
                downscorereliable: &mut downscorereliable,
            };
            if self.strong_branch(lp, data, &mut playground, &fracints, candidate, col, fracval, upval, downval, evaluate_up, &mut scores) {
                break None;
            }
        };
        playground.end(lp, data);
        result
    }

    /// `selectBranchingCandidate`'s `strongBranch`: evaluate one child of
    /// the candidate; true if that settled the node.
    #[allow(clippy::too_many_arguments)]
    fn strong_branch(
        &mut self,
        lp: &mut LpRelaxation,
        data: &mut MipData,
        playground: &mut Playground,
        fracints: &[(usize, f64)],
        candidate: usize,
        col: usize,
        fracval: f64,
        upval: f64,
        downval: f64,
        upbranch: bool,
        scores: &mut Scores,
    ) -> bool {
        let mut inferences = -(self.localdom.domain_change_stack().len() as i64) - 1;
        let boundtype = if upbranch { BoundType::Lower } else { BoundType::Upper };
        let boundval = if upbranch { upval } else { downval };
        let domchg = DomainChange::new(boundval, col, boundtype);
        let orbits = self.nodestack.last().expect("a node").stabilizer_orbits.clone();
        let orbital_fixing = orbits.is_some() && self.orbits_valid_in_child_node(&domchg, data);
        self.change_bound(data, domchg);
        self.propagate(data);
        if !self.localdom.infeasible() {
            if orbital_fixing {
                self.orbital_fixing(data, orbits.as_ref());
            } else {
                self.orbital_fixing(data, None);
            }
        }
        inferences += self.localdom.domain_change_stack().len() as i64;
        let settle = |this: &mut Self, data: &mut MipData, opensubtrees: u8, other_child_lb: Option<f64>| {
            if upbranch {
                this.branch_downwards(col, downval, fracval, data);
            } else {
                this.branch_upwards(col, upval, fracval, data);
            }
            let n = this.nodestack.len();
            this.nodestack[n - 2].opensubtrees = opensubtrees;
            if let Some(lb) = other_child_lb {
                this.nodestack[n - 2].other_child_lb = lb;
            }
            this.nodestack[n - 2].skip_depth_count = 1;
            this.depthoffset -= 1;
        };
        if self.localdom.infeasible() {
            self.conflict_analysis(data);
            pc!(self, data).add_cutoff_observation(col, upbranch);
            self.backtrack_local(data);
            self.localdom.clear_changed_cols();
            settle(self, data, 0, None);
            return true;
        }
        pc!(self, data).add_inference_observation(col, inferences, upbranch);
        let mut numiters = lp.num_lp_iterations();
        let status = playground.solve_lp(lp, &mut self.localdom, data);
        numiters = lp.num_lp_iterations() - numiters;
        self.lpiterations += numiters;
        self.sblpiterations += numiters;

        if status.scaled_optimal() {
            lp.perform_aging(false, data);
            let delta = if upbranch { upval - fracval } else { downval - fracval };
            let sol = lp.lp_solver().col_value().expect("an LP solution").to_vec();
            let (solobj, integerfeasible) = self.check_sol(&sol, data);
            let mut objdelta = (solobj - lp.objective()).max(0.0);
            if objdelta <= data.epsilon {
                objdelta = 0.0;
            }
            if upbranch {
                scores.upscore[candidate] = objdelta;
                scores.upscorereliable[candidate] = true;
                self.mark_up_reliable(col);
            } else {
                scores.downscore[candidate] = objdelta;
                scores.downscorereliable[candidate] = true;
                self.mark_down_reliable(col);
            }
            pc!(self, data).add_observation(col, delta, objdelta);
            self.analyze_solution(data, fracints, col, objdelta, &sol, scores);
            if status.unscaled_primal_feasible() && integerfeasible {
                let cutoffbnd = self.cutoff_bound(data);
                let source = if self.inheuristic { SolutionSource::Heuristic } else { SolutionSource::Branching };
                self.add_incumbent(&sol, solobj, source, data);
                if data.upper_limit < cutoffbnd {
                    lp.set_objective_limit(data.upper_limit, data);
                }
            }
            if status.unscaled_dual_feasible() {
                if upbranch {
                    scores.upbound[candidate] = solobj;
                } else {
                    scores.downbound[candidate] = solobj;
                }
                if solobj > data.optimality_limit {
                    self.add_bound_exceeding_conflict(lp, data);
                    let pruned = solobj > self.cutoff_bound(data);
                    self.backtrack_local(data);
                    lp.flush_domain(&mut self.localdom, false, &data.model);
                    settle(self, data, if pruned { 0 } else { 1 }, Some(solobj));
                    return true;
                }
            } else if solobj > self.cutoff_bound(data) {
                self.add_bound_exceeding_conflict(lp, data);
                self.propagate(data);
                if self.localdom.infeasible() {
                    self.backtrack_local(data);
                    lp.flush_domain(&mut self.localdom, false, &data.model);
                    settle(self, data, 0, None);
                    return true;
                }
            }
        } else if status == LpStatus::Infeasible {
            self.add_infeasible_conflict(lp, data);
            pc!(self, data).add_cutoff_observation(col, upbranch);
            self.backtrack_local(data);
            lp.flush_domain(&mut self.localdom, false, &data.model);
            settle(self, data, 0, None);
            return true;
        } else {
            scores.downscore[candidate] = 0.0;
            scores.upscore[candidate] = 0.0;
            scores.downscorereliable[candidate] = true;
            scores.upscorereliable[candidate] = true;
            self.mark_up_reliable(col);
            self.mark_down_reliable(col);
        }
        self.backtrack_local(data);
        lp.flush_domain(&mut self.localdom, false, &data.model);
        false
    }

    /// `selectBranchingCandidate`'s `analyzeSolution`: what the strong
    /// branching solution says about the other candidates.
    fn analyze_solution(&mut self, data: &mut MipData, fracints: &[(usize, f64)], col: usize, objdelta: f64, sol: &[f64], scores: &mut Scores) {
        let feastol = data.feastol;
        let num_changed_cols = self.localdom.changed_cols().len();
        let domchg_stack_size = self.localdom.domain_change_stack().len();
        for k in 0..fracints.len() {
            let (kcol, otherfracval) = fracints[k];
            if kcol == col {
                continue;
            }
            let otherdownval = otherfracval.floor();
            let otherupval = otherfracval.ceil();
            let down = sol[kcol] <= otherdownval + feastol;
            if !down && sol[kcol] < otherupval - feastol {
                continue;
            }
            let needs_check = if down {
                self.localdom.col_upper[kcol] > otherdownval + feastol
            } else {
                self.localdom.col_lower[kcol] < otherupval - feastol
            };
            if needs_check {
                let domchg = if down {
                    DomainChange::new(otherdownval, kcol, BoundType::Upper)
                } else {
                    DomainChange::new(otherupval, kcol, BoundType::Lower)
                };
                self.change_bound(data, domchg);
                if self.localdom.infeasible() {
                    self.conflict_analysis(data);
                    self.backtrack_local(data);
                    self.localdom.clear_changed_cols_from(num_changed_cols);
                    continue;
                }
                self.propagate(data);
                if self.localdom.infeasible() {
                    self.conflict_analysis(data);
                    self.backtrack_local(data);
                    self.localdom.clear_changed_cols_from(num_changed_cols);
                    continue;
                }
                let new_stack_size = self.localdom.domain_change_stack().len();
                let domchgstack = self.localdom.domain_change_stack();
                let mut solution_valid = true;
                for j in domchg_stack_size + 1..new_stack_size {
                    let change = domchgstack[j];
                    if change.boundtype == BoundType::Lower {
                        if change.boundval > sol[change.column] + feastol {
                            solution_valid = false;
                            break;
                        }
                    } else if change.boundval < sol[change.column] - feastol {
                        solution_valid = false;
                        break;
                    }
                }
                self.backtrack_local(data);
                self.localdom.clear_changed_cols_from(num_changed_cols);
                if !solution_valid {
                    continue;
                }
            }
            if down {
                if objdelta <= feastol {
                    pc!(self, data).add_observation(kcol, otherdownval - otherfracval, objdelta);
                    self.mark_down_reliable(kcol);
                }
                scores.downscore[k] = scores.downscore[k].min(objdelta);
            } else {
                if objdelta <= feastol {
                    pc!(self, data).add_observation(kcol, otherupval - otherfracval, objdelta);
                    self.mark_up_reliable(kcol);
                }
                scores.upscore[k] = scores.upscore[k].min(objdelta);
            }
        }
    }

    /// HiGHS's `currentNodeToQueue`.
    pub fn current_node_to_queue(&mut self, data: &mut MipData) {
        let oldchangedcols = self.localdom.changed_cols().len();
        let mut prune = self.nodestack.last().expect("a node").lower_bound > self.cutoff_bound(data);
        if !prune {
            self.propagate(data);
            self.localdom.clear_changed_cols_from(oldchangedcols);
            prune = self.localdom.infeasible();
            if prune {
                self.conflict_analysis(data);
            }
        }
        self.queue_or_prune(prune, data);
        self.nodestack.last_mut().expect("a node").opensubtrees = 0;
    }

    /// The shared tail of `currentNodeToQueue` and `openNodesToQueue`.
    fn queue_or_prune(&mut self, prune: bool, data: &mut MipData) {
        if !prune {
            let (domchg_stack, branch_positions) = self.localdom.reduced_domain_change_stack();
            let node = self.nodestack.last().expect("a node");
            let lower_bound = node.lower_bound.max(self.localdom.objective_lower_bound());
            let estimate = node.estimate;
            let depth = self.current_depth() as usize;
            let tmp_tree_weight = data.nodequeue.borrow_mut().emplace_node(domchg_stack, branch_positions, lower_bound, estimate, depth);
            if self.count_tree_weight {
                self.treeweight += tmp_tree_weight;
            }
        } else if self.count_tree_weight {
            self.treeweight += ldexp(1.0, (1 - self.current_depth()) as i32);
        }
    }

    /// HiGHS's `openNodesToQueue`.
    pub fn open_nodes_to_queue(&mut self, lp: &mut LpRelaxation, data: &mut MipData) {
        if self.nodestack.is_empty() {
            return;
        }
        let mut basis = None;
        for node in self.nodestack.iter_mut() {
            if node.node_basis.is_some() {
                basis = node.node_basis.take();
                break;
            }
        }
        if self.nodestack.last().expect("a node").opensubtrees == 0 {
            self.backtrack(lp, data, false);
        }
        while !self.nodestack.is_empty() {
            let oldchangedcols = self.localdom.changed_cols().len();
            let mut prune = self.nodestack.last().expect("a node").lower_bound > self.cutoff_bound(data);
            if !prune {
                self.propagate(data);
                self.localdom.clear_changed_cols_from(oldchangedcols);
                prune = self.localdom.infeasible();
                if prune {
                    self.conflict_analysis(data);
                }
            }
            self.queue_or_prune(prune, data);
            self.nodestack.last_mut().expect("a node").opensubtrees = 0;
            self.backtrack(lp, data, false);
        }
        lp.flush_domain(&mut self.localdom, false, &data.model);
        if let Some(basis) = basis {
            if basis.row_status.len() == lp.num_rows() {
                lp.set_stored_basis(Some(basis));
            }
            lp.recover_basis();
        }
    }

    /// HiGHS's `flushStatistics`.
    pub fn flush_statistics(&mut self, data: &mut MipData) {
        data.num_nodes += self.nnodes;
        self.nnodes = 0;
        data.num_leaves += self.nleaves;
        self.nleaves = 0;
        let weight = data.pruned_treeweight.get() + self.treeweight;
        data.pruned_treeweight.set(weight);
        self.treeweight = CDouble::new(0.0);
        data.total_lp_iterations += self.lpiterations;
        self.lpiterations = 0;
        data.heuristic_lp_iterations += self.heurlpiterations;
        self.heurlpiterations = 0;
        data.sb_lp_iterations += self.sblpiterations;
        self.sblpiterations = 0;
    }

    /// HiGHS's `resetLocalDomain`.
    pub fn reset_local_domain(&mut self, lp: &mut LpRelaxation, data: &MipData) {
        lp.reset_to_global_domain(&data.domain);
        self.localdom = data.domain.local_copy();
    }

    /// HiGHS's `installNode`.
    pub fn install_node(&mut self, node: OpenNode, data: &mut MipData) {
        on_local(&mut self.localdom, data, |dom, env| {
            dom.set_domain_change_stack_with_branching(&node.domchgstack, &node.branchings, env)
        });
        let mut global_symmetries_valid = true;
        if data.global_orbits.is_some() {
            let domchgstack = self.localdom.domain_change_stack();
            for &i in self.localdom.branching_positions() {
                let col = domchgstack[i].column;
                if data.symmetries.column_position[col].is_none() {
                    continue;
                }
                if !data.domain.is_binary(&data.model, col) || (domchgstack[i].boundtype == BoundType::Lower && domchgstack[i].boundval == 1.0) {
                    global_symmetries_valid = false;
                    break;
                }
            }
        }
        let orbits = if global_symmetries_valid { data.global_orbits.clone() } else { None };
        self.nodestack.push(NodeData::new(node.lower_bound, node.estimate, None, orbits));
        self.subrootsol.clear();
        self.depthoffset = node.depth as i64 - 1;
    }

    /// The pseudocost cutoff observation for the branching that led to an
    /// infeasible node.
    fn observe_parent_cutoff(&mut self, data: &mut MipData) {
        let n = self.nodestack.len();
        if n <= 1 {
            return;
        }
        let parent = &self.nodestack[n - 2];
        if parent.lp_objective != -INF && parent.branching_point != parent.branchingdecision.boundval {
            let upbranch = parent.branchingdecision.boundtype == BoundType::Lower;
            pc!(self, data).add_cutoff_observation(parent.branchingdecision.column, upbranch);
        }
    }

    /// HiGHS's `evaluateNode`.
    pub fn evaluate_node(&mut self, lp: &mut LpRelaxation, data: &mut MipData) -> NodeResult {
        let n = self.nodestack.len();
        if !self.inheuristic && self.nodestack[n - 1].lower_bound > data.optimality_limit {
            return NodeResult::SubOptimal;
        }
        self.propagate(data);
        if !self.inheuristic && !self.localdom.infeasible() {
            let parent_allows = n == 1 || self.nodestack[n - 2].stabilizer_orbits.as_ref().is_none_or(|orbits| !orbits.orbit_cols.is_empty());
            if data.symmetries.num_perms > 0 && self.nodestack[n - 1].stabilizer_orbits.is_none() && parent_allows {
                let orbits = data.symmetries.compute_stabilizer_orbits(&self.localdom, &data.model, &mut self.stabilizer_orbit_workspace);
                self.nodestack[n - 1].stabilizer_orbits = Some(orbits);
            }
            let orbits = self.nodestack[n - 1].stabilizer_orbits.clone();
            self.orbital_fixing(data, orbits.as_ref());
        }
        if n > 1 {
            let parent = &self.nodestack[n - 2];
            let inferences = self.localdom.domain_change_stack().len() as i64 - (self.nodestack[n - 1].domchg_stack_pos.expect("a stack position") as i64 + 1);
            let (col, up) = (parent.branchingdecision.column, parent.branchingdecision.boundtype == BoundType::Lower);
            pc!(self, data).add_inference_observation(col, inferences, up);
        }
        let mut result = NodeResult::Open;
        if self.localdom.infeasible() {
            result = NodeResult::DomainInfeasible;
            self.localdom.clear_changed_cols();
            self.observe_parent_cutoff(data);
            self.conflict_analysis(data);
        } else {
            lp.flush_domain(&mut self.localdom, false, &data.model);
            lp.set_objective_limit(data.upper_limit, data);
            let oldnumiters = lp.num_lp_iterations();
            let status = lp.resolve_lp(LpDomain::Local(&mut self.localdom), data);
            self.lpiterations += lp.num_lp_iterations() - oldnumiters;
            let objective_lower_bound = self.localdom.objective_lower_bound();
            {
                let currnode = self.nodestack.last_mut().expect("a node");
                currnode.lower_bound = objective_lower_bound.max(currnode.lower_bound);
            }
            if self.localdom.infeasible() {
                result = NodeResult::DomainInfeasible;
                self.localdom.clear_changed_cols();
                self.observe_parent_cutoff(data);
                self.conflict_analysis(data);
            } else if status.scaled_optimal() {
                lp.store_basis();
                lp.perform_aging(false, data);
                let estimate = lp.compute_best_estimate(self.own_pseudocost.as_ref().unwrap_or(&data.pseudocost), data);
                {
                    let currnode = self.nodestack.last_mut().expect("a node");
                    currnode.node_basis = lp.stored_basis();
                    currnode.estimate = estimate;
                    currnode.lp_objective = lp.objective();
                }
                if n > 1 {
                    let (parent, currnode) = {
                        let (head, tail) = self.nodestack.split_at(n - 1);
                        (&head[n - 2], &tail[0])
                    };
                    if parent.lp_objective != -INF && parent.branching_point != parent.branchingdecision.boundval {
                        let delta = parent.branchingdecision.boundval - parent.branching_point;
                        let objdelta = (currnode.lp_objective - parent.lp_objective).max(0.0);
                        pc!(self, data).add_observation(parent.branchingdecision.column, delta, objdelta);
                    }
                }
                if status.unscaled_primal_feasible() && lp.fractional_integers().is_empty() {
                    let cutoffbnd = self.cutoff_bound(data);
                    let sol = lp.lp_solver().col_value().expect("an LP solution").to_vec();
                    let source = if self.inheuristic { SolutionSource::Heuristic } else { SolutionSource::EvaluateNode };
                    self.add_incumbent(&sol, lp.objective(), source, data);
                    if data.upper_limit < cutoffbnd {
                        lp.set_objective_limit(data.upper_limit, data);
                    }
                    if status.unscaled_dual_feasible() {
                        self.add_bound_exceeding_conflict(lp, data);
                        result = NodeResult::BoundExceeding;
                    }
                }
                if result == NodeResult::Open {
                    if status.unscaled_dual_feasible() {
                        let lower_bound = {
                            let currnode = self.nodestack.last_mut().expect("a node");
                            currnode.lower_bound = currnode.lp_objective.max(currnode.lower_bound);
                            currnode.lower_bound
                        };
                        if lower_bound > self.cutoff_bound(data) {
                            result = NodeResult::BoundExceeding;
                            self.add_bound_exceeding_conflict(lp, data);
                        } else if data.upper_limit != INF {
                            if !self.inheuristic {
                                let gap = data.upper_limit - lp.objective();
                                let threshold = gap + (10.0 * data.feastol).max(data.epsilon * gap);
                                lp.compute_basic_degenerate_duals(threshold, Some(&mut self.localdom), data, true);
                            }
                            let upper_limit = data.upper_limit;
                            RedcostFixing::propagate_red_cost(&mut self.localdom, lp, data, upper_limit);
                            self.propagate(data);
                            if self.localdom.infeasible() {
                                result = NodeResult::DomainInfeasible;
                                self.localdom.clear_changed_cols();
                                self.observe_parent_cutoff(data);
                                self.conflict_analysis(data);
                            } else if !self.localdom.changed_cols().is_empty() {
                                return self.evaluate_node(lp, data);
                            }
                        } else if !self.inheuristic {
                            lp.compute_basic_degenerate_duals(INF, Some(&mut self.localdom), data, true);
                            self.propagate(data);
                            if self.localdom.infeasible() {
                                result = NodeResult::DomainInfeasible;
                                self.localdom.clear_changed_cols();
                                self.observe_parent_cutoff(data);
                                self.conflict_analysis(data);
                            } else if !self.localdom.changed_cols().is_empty() {
                                return self.evaluate_node(lp, data);
                            }
                        }
                    } else if lp.objective() > self.cutoff_bound(data) {
                        self.add_bound_exceeding_conflict(lp, data);
                        self.propagate(data);
                        if self.localdom.infeasible() {
                            result = NodeResult::BoundExceeding;
                        }
                    }
                }
            } else if status == LpStatus::Infeasible {
                result = if lp.lp_solver().status() == ModelStatus::ObjectiveBound {
                    NodeResult::BoundExceeding
                } else {
                    NodeResult::LpInfeasible
                };
                self.add_infeasible_conflict(lp, data);
                self.observe_parent_cutoff(data);
            }
        }
        if result != NodeResult::Open {
            self.treeweight += ldexp(1.0, (1 - self.current_depth()) as i32);
            self.nodestack.last_mut().expect("a node").opensubtrees = 0;
        } else if !self.inheuristic && self.nodestack.last().expect("a node").lower_bound > data.optimality_limit {
            result = NodeResult::SubOptimal;
            self.add_bound_exceeding_conflict(lp, data);
        }
        result
    }

    /// Set the current node's branching decision to `col` rounded up (or
    /// down) from its branching point, with the other child's bound.
    fn decide_child(&mut self, col: usize, up: bool, down_node_lb: f64, up_node_lb: f64, child_lb: &mut f64) {
        let currnode = self.nodestack.last_mut().expect("a node");
        if up {
            let boundval = currnode.branching_point.ceil();
            currnode.decide(col, boundval, BoundType::Lower);
            currnode.other_child_lb = down_node_lb;
            *child_lb = up_node_lb;
        } else {
            let boundval = currnode.branching_point.floor();
            currnode.decide(col, boundval, BoundType::Upper);
            currnode.other_child_lb = up_node_lb;
            *child_lb = down_node_lb;
        }
    }

    /// HiGHS's `branch`.
    pub fn branch(&mut self, lp: &mut LpRelaxation, data: &mut MipData) -> NodeResult {
        self.nodestack.last_mut().expect("a node").branching_col = None;
        self.inbranching = true;
        let minrel = pc!(self, data).min_reliable();
        let mut child_lb = self.current_lower_bound();
        let mut result = NodeResult::Open;
        while self.nodestack.last().expect("a node").opensubtrees == 2 && lp.status().scaled_optimal() && !lp.fractional_integers().is_empty() {
            let mut sbmaxiters = 0i64;
            if minrel > 0 {
                let sbiters = self.strong_branching_lp_iterations(data);
                sbmaxiters = 100000 + ((self.total_lp_iterations(data) - self.heuristic_lp_iterations(data) - self.strong_branching_lp_iterations(data)) >> 1);
                if sbiters > sbmaxiters {
                    pc!(self, data).set_min_reliable(0);
                } else if sbiters > (sbmaxiters >> 1) {
                    let reductionratio = (sbiters - (sbmaxiters >> 1)) as f64 / (sbmaxiters - (sbmaxiters >> 1)) as f64;
                    let minrelreduced = (minrel as f64 - reductionratio * (minrel - 1) as f64) as i32 as i64;
                    pc!(self, data).set_min_reliable(minrel.min(minrelreduced));
                }
            }
            let degeneracy_fac = lp.compute_lp_degeneracy(&self.localdom);
            pc!(self, data).set_degeneracy_factor(degeneracy_fac);
            if degeneracy_fac >= 10.0 {
                pc!(self, data).set_min_reliable(0);
            }
            let mut down_node_lb = self.current_lower_bound();
            let mut up_node_lb = self.current_lower_bound();
            let branchcand = self.select_branching_candidate(lp, data, sbmaxiters, &mut down_node_lb, &mut up_node_lb);
            child_lb = self.nodestack.last().expect("a node").lower_bound;
            if let Some(branchcand) = branchcand {
                let (col, branching_point) = lp.fractional_integers()[branchcand];
                {
                    let currnode = self.nodestack.last_mut().expect("a node");
                    currnode.branching_col = Some(col);
                    currnode.branchingdecision.column = col;
                    currnode.branching_point = branching_point;
                }
                let feastol = data.feastol;
                let epsilon = data.epsilon;
                let up = match self.childselrule {
                    ChildSelectionRule::Up => true,
                    ChildSelectionRule::Down => false,
                    ChildSelectionRule::RootSol => {
                        let mut down_prio = pc!(self, data).avg_inferences_down(col) + epsilon;
                        let mut up_prio = pc!(self, data).avg_inferences_up(col) + epsilon;
                        let down_val = branching_point.floor();
                        let up_val = branching_point.ceil();
                        let lp_objective = self.nodestack.last().expect("a node").lp_objective;
                        let clamp = |rootsol: f64| rootsol.max(down_val).min(up_val);
                        if !self.subrootsol.is_empty() {
                            let rootsol = clamp(self.subrootsol[col]);
                            up_prio *= 1.0 + (branching_point - rootsol);
                            down_prio *= 1.0 + (rootsol - branching_point);
                        } else {
                            if lp_objective != -INF {
                                self.subrootsol = lp.lp_solver().col_value().expect("an LP solution").to_vec();
                            }
                            if !data.rootlpsol.is_empty() {
                                let rootsol = clamp(data.rootlpsol[col]);
                                up_prio *= 1.0 + (branching_point - rootsol);
                                down_prio *= 1.0 + (rootsol - branching_point);
                            }
                        }
                        up_prio + epsilon >= down_prio
                    }
                    ChildSelectionRule::Obj => data.model.col_cost[col] >= 0.0,
                    ChildSelectionRule::Random => self.random.bit(),
                    ChildSelectionRule::BestCost => {
                        !(pc!(self, data).pseudocost_up_offset(col, branching_point, feastol) > pc!(self, data).pseudocost_down_offset(col, branching_point, feastol))
                    }
                    ChildSelectionRule::WorstCost => pc!(self, data).pseudocost_up(col, branching_point) >= pc!(self, data).pseudocost_down(col, branching_point),
                    ChildSelectionRule::Disjunction => {
                        let nodequeue = data.nodequeue.borrow();
                        let numnodesup = nodequeue.num_nodes_up(col);
                        let numnodesdown = nodequeue.num_nodes_down(col);
                        if numnodesup > numnodesdown {
                            true
                        } else if numnodesdown > numnodesup {
                            false
                        } else {
                            data.model.col_cost[col] >= 0.0
                        }
                    }
                    ChildSelectionRule::HybridInferenceCost => {
                        let up_score = (1.0 + pc!(self, data).avg_inferences_up(col)) / pc!(self, data).pseudocost_up_offset(col, branching_point, feastol);
                        let down_score = (1.0 + pc!(self, data).avg_inferences_down(col)) / pc!(self, data).pseudocost_down_offset(col, branching_point, feastol);
                        up_score >= down_score
                    }
                };
                self.decide_child(col, up, down_node_lb, up_node_lb, &mut child_lb);
                result = NodeResult::Branched;
                break;
            }
            result = self.evaluate_node(lp, data);
            if result == NodeResult::SubOptimal {
                break;
            }
        }
        self.inbranching = false;
        pc!(self, data).set_min_reliable(minrel);
        pc!(self, data).set_degeneracy_factor(1.0);
        if self.nodestack.last().expect("a node").opensubtrees != 2 || result == NodeResult::SubOptimal {
            return result;
        }
        if self.nodestack.last().expect("a node").branching_col.is_none() {
            let mut bestscore = -1.0;
            pc!(self, data).set_degeneracy_factor(1e6);
            let feastol = data.feastol;
            for &i in &data.integral_cols.clone() {
                let (lo, hi) = (self.localdom.col_lower[i], self.localdom.col_upper[i]);
                if hi - lo < 0.5 {
                    continue;
                }
                let fracval = if lo != -INF && hi != INF {
                    (0.5 * (lo + hi + 0.5)).floor() + 0.5
                } else if lo != -INF {
                    lo + 0.5
                } else if hi != INF {
                    hi - 0.5
                } else {
                    0.5
                };
                let score = pc!(self, data).score(i, fracval);
                if score > bestscore {
                    bestscore = score;
                    let cost = if lp.status().unscaled_dual_feasible() {
                        lp.lp_solver().col_dual().expect("duals")[i]
                    } else {
                        data.model.col_cost[i]
                    };
                    let branch_upwards = if cost.abs() > feastol && self.cutoff_bound(data) < INF {
                        cost > 0.0
                    } else if pc!(self, data).avg_inferences_up(i) > pc!(self, data).avg_inferences_down(i) + feastol {
                        true
                    } else if pc!(self, data).avg_inferences_up(i) < pc!(self, data).avg_inferences_down(i) - feastol {
                        false
                    } else {
                        let n = self.localdom.num_domain_changes() as isize;
                        let (_, col_lower_pos) = self.localdom.col_lower_pos(i, n);
                        let (_, col_upper_pos) = self.localdom.col_upper_pos(i, n);
                        col_lower_pos <= col_upper_pos
                    };
                    let currnode = self.nodestack.last_mut().expect("a node");
                    if branch_upwards {
                        let upval = fracval.ceil();
                        currnode.branching_point = upval;
                        currnode.decide(i, upval, BoundType::Lower);
                    } else {
                        let downval = fracval.floor();
                        currnode.branching_point = downval;
                        currnode.decide(i, downval, BoundType::Upper);
                    }
                }
            }
            pc!(self, data).set_degeneracy_factor(1.0);
        }
        if self.nodestack.last().expect("a node").branching_col.is_none() {
            if lp.status() == LpStatus::Optimal {
                self.nodestack.last_mut().expect("a node").opensubtrees = 0;
                return NodeResult::LpInfeasible;
            }
            lp.set_iteration_limit(i32::MAX as usize);
            // A fresh LP of the model rows only, as every integer column is
            // fixed and the cuts did not help.
            let mut lp_copy = LpRelaxation::new(data);
            lp_copy.load_model(data);
            let n = data.model.num_col;
            lp_copy
                .lp_solver_mut()
                .change_col_bounds((0..n).map(|col| (col, self.localdom.col_lower[col], self.localdom.col_upper[col])));
            lp_copy.lp_solver_mut().options.presolve = true;
            let mut result = self.evaluate_node(&mut lp_copy, data);
            if result == NodeResult::Open {
                lp_copy.lp_solver_mut().clear_solver();
                lp_copy.lp_solver_mut().options.simplex_strategy = crate::simplex::options::SimplexStrategy::Primal;
                result = self.evaluate_node(&mut lp_copy, data);
                lp_copy.lp_solver_mut().options.simplex_strategy = crate::simplex::options::SimplexStrategy::Dual;
                if result == NodeResult::Open {
                    lp_copy.lp_solver_mut().clear_solver();
                    lp_copy.lp_solver_mut().options.solver = crate::simplex::options::Solver::Ipm;
                    result = self.evaluate_node(&mut lp_copy, data);
                    if result == NodeResult::Open {
                        self.nodestack.last_mut().expect("a node").opensubtrees = 0;
                        result = NodeResult::LpInfeasible;
                    }
                }
            }
            return result;
        }
        let domchg_pos = self.localdom.domain_change_stack().len();
        let decision = self.nodestack.last().expect("a node").branchingdecision;
        let pass_stabilizer = self.orbits_valid_in_child_node(&decision, data);
        self.change_bound(data, decision);
        let currnode = self.nodestack.last_mut().expect("a node");
        currnode.opensubtrees = 1;
        let mut child = NodeData::new(
            child_lb.max(currnode.lower_bound),
            currnode.estimate,
            currnode.node_basis.clone(),
            if pass_stabilizer { currnode.stabilizer_orbits.clone() } else { None },
        );
        child.domchg_stack_pos = Some(domchg_pos);
        self.nodestack.push(child);
        NodeResult::Branched
    }

    /// Re-propagate the node backtracked to (it may have become infeasible
    /// by conflicts): `backtrack`'s and `backtrackPlunge`'s common step.
    fn repropagate_after_backtrack(&mut self, data: &mut MipData) {
        let n = self.nodestack.len();
        if self.nodestack[n - 1].opensubtrees == 0 {
            return;
        }
        self.count_tree_weight = self.nodestack[n - 1].skip_depth_count == 0;
        let old_num_domchgs = self.localdom.num_domain_changes();
        let old_num_changed_cols = self.localdom.changed_cols().len();
        self.propagate(data);
        if !self.localdom.infeasible() && old_num_domchgs != self.localdom.num_domain_changes() {
            let orbits = self.nodestack[n - 1].stabilizer_orbits.clone();
            self.orbital_fixing(data, orbits.as_ref());
        }
        if self.localdom.infeasible() {
            self.localdom.clear_changed_cols_from(old_num_changed_cols);
            if self.count_tree_weight {
                self.treeweight += ldexp(1.0, -self.current_depth() as i32);
            }
            self.nodestack[n - 1].opensubtrees = 0;
        }
    }

    /// Switch the current node's branching to its other child; the
    /// `(fallback branch, domain stack position)` of the change.
    fn flip_branching(&mut self) -> (bool, usize) {
        let domchg_pos = self.localdom.domain_change_stack().len();
        let currnode = self.nodestack.last_mut().expect("a node");
        currnode.opensubtrees = 0;
        let fallbackbranch = currnode.branchingdecision.boundval == currnode.branching_point;
        if currnode.branchingdecision.boundtype == BoundType::Lower {
            currnode.branchingdecision.boundtype = BoundType::Upper;
            currnode.branchingdecision.boundval = (currnode.branchingdecision.boundval - 0.5).floor();
        } else {
            currnode.branchingdecision.boundtype = BoundType::Lower;
            currnode.branchingdecision.boundval = (currnode.branchingdecision.boundval + 0.5).ceil();
        }
        (fallbackbranch, domchg_pos)
    }

    /// Install the other child after `flip_branching`: whether it was pruned.
    fn try_other_child(&mut self, data: &mut MipData, num_changed_cols: usize) -> (bool, bool, f64) {
        let decision = self.nodestack.last().expect("a node").branchingdecision;
        let pass_stabilizer = self.orbits_valid_in_child_node(&decision, data);
        self.change_bound(data, decision);
        let (lower_bound, other_child_lb, orbits) = {
            let currnode = self.nodestack.last().expect("a node");
            (currnode.lower_bound, currnode.other_child_lb, currnode.stabilizer_orbits.clone())
        };
        let nodelb = lower_bound.max(other_child_lb);
        let mut prune = nodelb > self.cutoff_bound(data) || self.localdom.infeasible();
        if !prune {
            self.propagate(data);
            prune = self.localdom.infeasible();
            if prune {
                self.conflict_analysis(data);
            }
        }
        if !prune {
            self.orbital_fixing(data, None);
            prune = self.localdom.infeasible();
        }
        if !prune && pass_stabilizer && orbits.is_some() {
            self.orbital_fixing(data, orbits.as_ref());
            prune = self.localdom.infeasible();
        }
        if prune {
            self.backtrack_local(data);
            self.localdom.clear_changed_cols_from(num_changed_cols);
            if self.count_tree_weight {
                self.treeweight += ldexp(1.0, -self.current_depth() as i32);
            }
        }
        (prune, pass_stabilizer, nodelb)
    }

    /// Pop finished nodes until one has an open child left (`false` when
    /// the stack empties, back in the global domain).
    fn pop_finished(&mut self, lp: &mut LpRelaxation, data: &mut MipData, recover_basis: bool) -> bool {
        while self.nodestack.last().expect("a node").opensubtrees == 0 {
            self.count_tree_weight = true;
            self.depthoffset += self.nodestack.last().expect("a node").skip_depth_count as i64;
            if self.nodestack.len() == 1 {
                let basis = self.nodestack.last_mut().expect("a node").node_basis.take();
                if recover_basis && basis.is_some() {
                    lp.set_stored_basis(basis);
                }
                self.nodestack.pop();
                on_local(&mut self.localdom, data, |dom, env| dom.backtrack_to_global(env));
                lp.flush_domain(&mut self.localdom, false, &data.model);
                if recover_basis {
                    lp.recover_basis();
                }
                return false;
            }
            self.nodestack.pop();
            self.backtrack_local(data);
            self.repropagate_after_backtrack(data);
        }
        true
    }

    /// Push the other child of the current node once `try_other_child`
    /// kept it.
    fn push_other_child(&mut self, lp: &mut LpRelaxation, data: &mut MipData, nodelb: f64, pass_stabilizer: bool, domchg_pos: usize) {
        let currnode = self.nodestack.last().expect("a node");
        let mut child = NodeData::new(
            nodelb,
            currnode.estimate,
            currnode.node_basis.clone(),
            if pass_stabilizer { currnode.stabilizer_orbits.clone() } else { None },
        );
        lp.flush_domain(&mut self.localdom, false, &data.model);
        child.domchg_stack_pos = Some(domchg_pos);
        self.nodestack.push(child);
    }

    /// HiGHS's `backtrack`: on to the next open node of the stack.
    pub fn backtrack(&mut self, lp: &mut LpRelaxation, data: &mut MipData, recover_basis: bool) -> bool {
        if self.nodestack.is_empty() {
            return false;
        }
        loop {
            if !self.pop_finished(lp, data, recover_basis) {
                return false;
            }
            let (fallbackbranch, domchg_pos) = self.flip_branching();
            if fallbackbranch {
                let currnode = self.nodestack.last_mut().expect("a node");
                currnode.branching_point = currnode.branchingdecision.boundval;
            }
            let num_changed_cols = self.localdom.changed_cols().len();
            let (prune, pass_stabilizer, nodelb) = self.try_other_child(data, num_changed_cols);
            if prune {
                continue;
            }
            self.push_other_child(lp, data, nodelb, pass_stabilizer, domchg_pos);
            break;
        }
        if recover_basis && let Some(basis) = self.nodestack.last().expect("a node").node_basis.clone() {
            lp.set_stored_basis(Some(basis));
            lp.recover_basis();
        }
        true
    }

    /// HiGHS's `backtrackPlunge`: backtrack, putting the other child into
    /// the queue when an ancestor's other branch scores better.
    pub fn backtrack_plunge(&mut self, lp: &mut LpRelaxation, data: &mut MipData) -> bool {
        if self.nodestack.is_empty() {
            return false;
        }
        loop {
            if !self.pop_finished(lp, data, true) {
                return false;
            }
            let (fallbackbranch, domchg_pos) = self.flip_branching();
            let node_score = {
                let currnode = self.nodestack.last().expect("a node");
                let point = if fallbackbranch { 0.5 } else { currnode.branching_point };
                if currnode.branchingdecision.boundtype == BoundType::Upper {
                    pc!(self, data).score_down(currnode.branchingdecision.column, point)
                } else {
                    pc!(self, data).score_up(currnode.branchingdecision.column, point)
                }
            };
            if fallbackbranch {
                let currnode = self.nodestack.last_mut().expect("a node");
                currnode.branching_point = currnode.branchingdecision.boundval;
            }
            let num_changed_cols = self.localdom.changed_cols().len();
            let (prune, pass_stabilizer, nodelb) = self.try_other_child(data, num_changed_cols);
            if prune {
                continue;
            }
            let nodelb = nodelb.max(self.localdom.objective_lower_bound());
            let mut node_to_queue = nodelb > data.optimality_limit;
            if !node_to_queue {
                for i in (0..self.nodestack.len().saturating_sub(1)).rev() {
                    let node = &self.nodestack[i];
                    if node.opensubtrees == 0 {
                        continue;
                    }
                    let fallbackbranch = node.branchingdecision.boundval == node.branching_point;
                    let branchpoint = if fallbackbranch { 0.5 } else { node.branching_point };
                    let col = node.branchingdecision.column;
                    let (active, inactive) = if node.branchingdecision.boundtype == BoundType::Lower {
                        (pc!(self, data).score_up(col, branchpoint), pc!(self, data).score_down(col, branchpoint))
                    } else {
                        (pc!(self, data).score_down(col, branchpoint), pc!(self, data).score_up(col, branchpoint))
                    };
                    node_to_queue = inactive - active > node_score + data.feastol;
                    break;
                }
            }
            if node_to_queue {
                let (domchg_stack, branch_positions) = self.localdom.reduced_domain_change_stack();
                let estimate = self.nodestack.last().expect("a node").estimate;
                let depth = (self.current_depth() + 1) as usize;
                let tmp_tree_weight = data.nodequeue.borrow_mut().emplace_node(domchg_stack, branch_positions, nodelb, estimate, depth);
                if self.count_tree_weight {
                    self.treeweight += tmp_tree_weight;
                }
                self.backtrack_local(data);
                self.localdom.clear_changed_cols_from(num_changed_cols);
                continue;
            }
            self.push_other_child(lp, data, nodelb, pass_stabilizer, domchg_pos);
            break;
        }
        if let Some(basis) = self.nodestack.last().expect("a node").node_basis.clone() {
            lp.set_stored_basis(Some(basis));
            lp.recover_basis();
        }
        true
    }

    /// HiGHS's `backtrackUntilDepth`.
    pub fn backtrack_until_depth(&mut self, lp: &mut LpRelaxation, data: &mut MipData, target_depth: i64) -> bool {
        if self.nodestack.is_empty() {
            return false;
        }
        if self.current_depth() >= target_depth {
            self.nodestack.last_mut().expect("a node").opensubtrees = 0;
        }
        while self.nodestack.last().expect("a node").opensubtrees == 0 {
            self.depthoffset += self.nodestack.last().expect("a node").skip_depth_count as i64;
            self.nodestack.pop();
            self.backtrack_local(data);
            if self.nodestack.is_empty() {
                lp.flush_domain(&mut self.localdom, false, &data.model);
                return false;
            }
            if self.current_depth() >= target_depth {
                self.nodestack.last_mut().expect("a node").opensubtrees = 0;
            }
        }
        let (fallbackbranch, domchg_pos) = self.flip_branching();
        if fallbackbranch {
            let currnode = self.nodestack.last_mut().expect("a node");
            currnode.branching_point = currnode.branchingdecision.boundval;
        }
        let decision = self.nodestack.last().expect("a node").branchingdecision;
        let pass_stabilizer = self.orbits_valid_in_child_node(&decision, data);
        self.change_bound(data, decision);
        let lower_bound = self.nodestack.last().expect("a node").lower_bound;
        self.push_other_child(lp, data, lower_bound, pass_stabilizer, domchg_pos);
        if let Some(basis) = self.nodestack.last().expect("a node").node_basis.clone()
            && basis.row_status.len() == lp.num_rows()
        {
            lp.set_stored_basis(Some(basis));
        }
        lp.recover_basis();
        true
    }

    /// HiGHS's `dive`.
    pub fn dive(&mut self, lp: &mut LpRelaxation, data: &mut MipData, node_lim: i64) -> NodeResult {
        self.reliableatnode.clear();
        loop {
            self.nnodes += 1;
            let result = self.evaluate_node(lp, data);
            if data.check_limits(self.nnodes) || result != NodeResult::Open {
                return result;
            }
            let result = self.branch(lp, data);
            if result != NodeResult::Branched || self.nnodes >= node_lim {
                return result;
            }
        }
    }

    /// HiGHS's `solveDepthFirst`.
    pub fn solve_depth_first(&mut self, lp: &mut LpRelaxation, data: &mut MipData, mut maxbacktracks: i64) {
        loop {
            if maxbacktracks == 0 {
                break;
            }
            let result = self.dive(lp, data, i64::MAX);
            if result == NodeResult::Open {
                break;
            }
            maxbacktracks -= 1;
            if !self.backtrack(lp, data, true) {
                break;
            }
        }
    }
}

/// The candidate scores `selectBranchingCandidate` shares with its
/// lambdas.
struct Scores<'a> {
    upscore: &'a mut [f64],
    downscore: &'a mut [f64],
    upbound: &'a mut [f64],
    downbound: &'a mut [f64],
    upscorereliable: &'a mut [bool],
    downscorereliable: &'a mut [bool],
}
