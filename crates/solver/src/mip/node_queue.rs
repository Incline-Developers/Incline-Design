//! HiGHS's `HighsNodeQueue`: the open nodes of the search, ordered three
//! ways by intrusive red-black trees (best bound, best hybrid estimate, and
//! the suboptimal ones past the optimality limit), with each column's node
//! bound changes in ordered sets so nodes a global bound rules out can be
//! pruned.

use std::{
    cmp::{Ordering, Reverse},
    collections::{BTreeSet, BinaryHeap},
};

use super::{
    BoundType, DomainChange,
    domain::{Domain, DomainEnv, Reason},
};
use crate::{
    rb_tree::{NO_LINK, RbLinks, RbTree},
    util::{CDouble, ldexp},
};

const INF: f64 = f64::INFINITY;

/// A double ordered as C++'s `<` orders it (there are no NaNs here).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Key(pub f64);

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Key {}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.0 < other.0 {
            Ordering::Less
        } else if other.0 < self.0 {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

/// One column's node bounds: `(bound, node)`.
pub(crate) type NodeSet = BTreeSet<(Key, usize)>;

/// HiGHS's `OpenNode`.
#[derive(Clone, Debug, Default)]
pub(crate) struct OpenNode {
    pub domchgstack: Vec<DomainChange>,
    pub branchings: Vec<usize>,
    pub lower_bound: f64,
    pub estimate: f64,
    pub depth: usize,
    lower_links: RbLinks,
    hybrid_estim_links: RbLinks,
}

#[derive(Clone, Debug)]
pub(crate) struct NodeQueue {
    nodes: Vec<OpenNode>,
    freeslots: BinaryHeap<Reverse<usize>>,
    col_lower_nodes: Vec<NodeSet>,
    col_upper_nodes: Vec<NodeSet>,
    lower_root: usize,
    lower_min: usize,
    hybrid_estim_root: usize,
    hybrid_estim_min: usize,
    suboptimal_root: usize,
    suboptimal_min: usize,
    num_suboptimal: usize,
    optimality_limit: f64,
    num_col: usize,
}

impl Default for NodeQueue {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            freeslots: BinaryHeap::new(),
            col_lower_nodes: Vec::new(),
            col_upper_nodes: Vec::new(),
            lower_root: NO_LINK,
            lower_min: NO_LINK,
            hybrid_estim_root: NO_LINK,
            hybrid_estim_min: NO_LINK,
            suboptimal_root: NO_LINK,
            suboptimal_min: NO_LINK,
            num_suboptimal: 0,
            optimality_limit: INF,
            num_col: 0,
        }
    }
}

/// HiGHS's `NodeLowerRbTree`: by lower bound, then stack size, estimate
/// and index.
struct LowerTree<'a>(&'a mut NodeQueue);

impl RbTree for LowerTree<'_> {
    type Key = (f64, i32, f64, i64);
    fn root(&self) -> usize {
        self.0.lower_root
    }
    fn set_root(&mut self, root: usize) {
        self.0.lower_root = root;
    }
    fn links(&self, node: usize) -> &RbLinks {
        &self.0.nodes[node].lower_links
    }
    fn links_mut(&mut self, node: usize) -> &mut RbLinks {
        &mut self.0.nodes[node].lower_links
    }
    fn key(&self, node: usize) -> Self::Key {
        let open = &self.0.nodes[node];
        (open.lower_bound, open.domchgstack.len() as i32, open.estimate, node as i64)
    }
    fn cached_first(&mut self) -> Option<&mut usize> {
        Some(&mut self.0.lower_min)
    }
}

/// HiGHS's `NodeHybridEstimRbTree`: by the average of lower bound and
/// estimate, then deeper first, then index.
struct HybridEstimTree<'a>(&'a mut NodeQueue);

impl RbTree for HybridEstimTree<'_> {
    type Key = (f64, i32, i64);
    fn root(&self) -> usize {
        self.0.hybrid_estim_root
    }
    fn set_root(&mut self, root: usize) {
        self.0.hybrid_estim_root = root;
    }
    fn links(&self, node: usize) -> &RbLinks {
        &self.0.nodes[node].hybrid_estim_links
    }
    fn links_mut(&mut self, node: usize) -> &mut RbLinks {
        &mut self.0.nodes[node].hybrid_estim_links
    }
    fn key(&self, node: usize) -> Self::Key {
        let open = &self.0.nodes[node];
        (0.5 * open.lower_bound + 0.5 * open.estimate, -(open.domchgstack.len() as i32), node as i64)
    }
    fn cached_first(&mut self) -> Option<&mut usize> {
        Some(&mut self.0.hybrid_estim_min)
    }
}

/// HiGHS's `SuboptimalNodeRbTree`, sharing the lower-bound links.
struct SuboptimalTree<'a>(&'a mut NodeQueue);

impl RbTree for SuboptimalTree<'_> {
    type Key = (f64, i64);
    fn root(&self) -> usize {
        self.0.suboptimal_root
    }
    fn set_root(&mut self, root: usize) {
        self.0.suboptimal_root = root;
    }
    fn links(&self, node: usize) -> &RbLinks {
        &self.0.nodes[node].lower_links
    }
    fn links_mut(&mut self, node: usize) -> &mut RbLinks {
        &mut self.0.nodes[node].lower_links
    }
    fn key(&self, node: usize) -> Self::Key {
        (self.0.nodes[node].lower_bound, node as i64)
    }
    fn cached_first(&mut self) -> Option<&mut usize> {
        Some(&mut self.0.suboptimal_min)
    }
}

/// The tree weight of a node at `depth`: `2^(1 - depth)`.
fn node_weight(depth: usize) -> f64 {
    ldexp(1.0, 1 - depth as i32)
}

impl NodeQueue {
    pub fn set_optimality_limit(&mut self, optimality_limit: f64) {
        self.optimality_limit = optimality_limit;
    }

    pub fn set_num_col(&mut self, num_col: usize) {
        if self.num_col == num_col {
            return;
        }
        self.num_col = num_col;
        self.col_lower_nodes = vec![NodeSet::new(); num_col];
        self.col_upper_nodes = vec![NodeSet::new(); num_col];
    }

    fn link_domchgs(&mut self, node: usize) {
        for i in 0..self.nodes[node].domchgstack.len() {
            let domchg = self.nodes[node].domchgstack[i];
            match domchg.boundtype {
                BoundType::Lower => self.col_lower_nodes[domchg.column].insert((Key(domchg.boundval), node)),
                BoundType::Upper => self.col_upper_nodes[domchg.column].insert((Key(domchg.boundval), node)),
            };
        }
    }

    fn unlink_domchgs(&mut self, node: usize) {
        for i in 0..self.nodes[node].domchgstack.len() {
            let domchg = self.nodes[node].domchgstack[i];
            match domchg.boundtype {
                BoundType::Lower => self.col_lower_nodes[domchg.column].remove(&(Key(domchg.boundval), node)),
                BoundType::Upper => self.col_upper_nodes[domchg.column].remove(&(Key(domchg.boundval), node)),
            };
        }
    }

    fn link_suboptimal(&mut self, node: usize) {
        SuboptimalTree(self).link(node);
        self.num_suboptimal += 1;
    }

    fn unlink_suboptimal(&mut self, node: usize) {
        SuboptimalTree(self).unlink(node);
        self.num_suboptimal -= 1;
    }

    /// Link a node into the trees; returns the tree weight it takes away
    /// if it goes straight to the suboptimal nodes.
    fn link(&mut self, node: usize) -> f64 {
        if self.nodes[node].lower_bound > self.optimality_limit {
            self.nodes[node].estimate = INF;
            self.link_suboptimal(node);
            self.link_domchgs(node);
            return node_weight(self.nodes[node].depth);
        }
        HybridEstimTree(self).link(node);
        LowerTree(self).link(node);
        self.link_domchgs(node);
        0.0
    }

    fn unlink(&mut self, node: usize) {
        if self.nodes[node].estimate == INF {
            self.unlink_suboptimal(node);
        } else {
            HybridEstimTree(self).unlink(node);
            LowerTree(self).unlink(node);
        }
        self.unlink_domchgs(node);
        self.freeslots.push(Reverse(node));
    }

    /// HiGHS's `checkGlobalBounds`: prune the nodes whose bounds on `col`
    /// the global bounds `lb`/`ub` exclude.
    fn check_global_bounds(&mut self, col: usize, lb: f64, ub: f64, feastol: f64, treeweight: &mut CDouble) {
        let mut delnodes = BTreeSet::new();
        for &(_, node) in self.col_lower_nodes[col].range((Key(ub + feastol), 0)..) {
            delnodes.insert(node);
        }
        for &(_, node) in self.col_upper_nodes[col].range(..=(Key(lb - feastol), usize::MAX)) {
            delnodes.insert(node);
        }
        for delnode in delnodes {
            if self.nodes[delnode].estimate != INF {
                *treeweight += node_weight(self.nodes[delnode].depth);
            }
            self.unlink(delnode);
        }
    }

    /// HiGHS's `pruneInfeasibleNodes`: prune what the global bounds rule
    /// out, tighten them to what every open node agrees on, propagate, and
    /// repeat while that changes something. Returns the pruned tree weight.
    pub fn prune_infeasible_nodes(&mut self, globaldomain: &mut Domain, feastol: f64, env: &mut DomainEnv) -> f64 {
        let mut treeweight = CDouble::new(0.0);
        loop {
            if globaldomain.infeasible() {
                break;
            }
            let numchgs = globaldomain.domain_change_stack().len();
            for i in 0..self.num_col {
                let (lb, ub) = (globaldomain.col_lower[i], globaldomain.col_upper[i]);
                self.check_global_bounds(i, lb, ub, feastol, &mut treeweight);
            }
            let numopennodes = self.num_nodes();
            if numopennodes == 0 {
                break;
            }
            for i in 0..self.num_col {
                if self.col_lower_nodes[i].len() == numopennodes {
                    let globallb = self.col_lower_nodes[i].first().expect("open nodes").0.0;
                    if globallb > globaldomain.col_lower[i] {
                        globaldomain.change_bound(DomainChange::new(globallb, i, BoundType::Lower), Reason::unspecified(), env);
                        if globaldomain.infeasible() {
                            break;
                        }
                    }
                }
                if self.col_upper_nodes[i].len() == numopennodes {
                    let globalub = self.col_upper_nodes[i].last().expect("open nodes").0.0;
                    if globalub < globaldomain.col_upper[i] {
                        globaldomain.change_bound(DomainChange::new(globalub, i, BoundType::Upper), Reason::unspecified(), env);
                        if globaldomain.infeasible() {
                            break;
                        }
                    }
                }
            }
            globaldomain.propagate(env);
            if numchgs == globaldomain.domain_change_stack().len() {
                break;
            }
        }
        treeweight.value()
    }

    pub fn prune_node(&mut self, node: usize) -> f64 {
        let treeweight = if self.nodes[node].estimate != INF { node_weight(self.nodes[node].depth) } else { 0.0 };
        self.unlink(node);
        treeweight
    }

    /// HiGHS's `performBounding`: prune nodes at or past `upper_limit`, and
    /// set aside those past the optimality limit.
    pub fn perform_bounding(&mut self, upper_limit: f64) -> f64 {
        if LowerTree(self).is_empty() {
            return 0.0;
        }
        let mut treeweight = CDouble::new(0.0);
        let mut max_lb_node = LowerTree(self).last();
        while max_lb_node != NO_LINK {
            if self.nodes[max_lb_node].lower_bound < upper_limit {
                break;
            }
            let next = LowerTree(self).predecessor(max_lb_node);
            treeweight += self.prune_node(max_lb_node);
            max_lb_node = next;
        }
        if self.optimality_limit < upper_limit {
            while max_lb_node != NO_LINK {
                if self.nodes[max_lb_node].lower_bound < self.optimality_limit {
                    break;
                }
                let next = LowerTree(self).predecessor(max_lb_node);
                HybridEstimTree(self).unlink(max_lb_node);
                LowerTree(self).unlink(max_lb_node);
                treeweight += node_weight(self.nodes[max_lb_node].depth);
                self.nodes[max_lb_node].estimate = INF;
                self.link_suboptimal(max_lb_node);
                max_lb_node = next;
            }
        }
        if self.num_suboptimal > 0 {
            max_lb_node = SuboptimalTree(self).last();
            while max_lb_node != NO_LINK {
                if self.nodes[max_lb_node].lower_bound < upper_limit {
                    break;
                }
                let next = SuboptimalTree(self).predecessor(max_lb_node);
                self.unlink(max_lb_node);
                max_lb_node = next;
            }
        }
        treeweight.value()
    }

    /// HiGHS's `emplaceNode`; returns the tree weight taken if it is
    /// suboptimal already.
    pub fn emplace_node(&mut self, domchgs: Vec<DomainChange>, branch_positions: Vec<usize>, lower_bound: f64, estimate: f64, depth: usize) -> f64 {
        let node = OpenNode {
            domchgstack: domchgs,
            branchings: branch_positions,
            lower_bound,
            estimate,
            depth,
            lower_links: RbLinks::default(),
            hybrid_estim_links: RbLinks::default(),
        };
        let pos = match self.freeslots.pop() {
            None => {
                self.nodes.push(node);
                self.nodes.len() - 1
            }
            Some(Reverse(pos)) => {
                self.nodes[pos] = node;
                pos
            }
        };
        self.link(pos)
    }

    pub fn pop_best_node(&mut self) -> OpenNode {
        let best = self.hybrid_estim_min;
        self.unlink(best);
        std::mem::take(&mut self.nodes[best])
    }

    pub fn pop_best_bound_node(&mut self) -> OpenNode {
        let best = self.lower_min;
        self.unlink(best);
        std::mem::take(&mut self.nodes[best])
    }

    pub fn best_lower_bound(&self) -> f64 {
        let lb = if self.lower_min == NO_LINK { INF } else { self.nodes[self.lower_min].lower_bound };
        if self.suboptimal_min == NO_LINK {
            return lb;
        }
        super::cmin(self.nodes[self.suboptimal_min].lower_bound, lb)
    }

    pub fn best_bound_domchg_stack_size(&self) -> usize {
        let size = if self.lower_min == NO_LINK {
            i32::MAX as usize
        } else {
            self.nodes[self.lower_min].domchgstack.len()
        };
        if self.suboptimal_min == NO_LINK {
            return size;
        }
        self.nodes[self.suboptimal_min].domchgstack.len().min(size)
    }

    /// HiGHS's `clear`: no nodes, and no optimality limit.
    pub fn clear(&mut self) {
        let num_col = self.num_col;
        *self = Self::default();
        self.set_num_col(num_col);
    }

    pub fn num_nodes_up(&self, col: usize) -> i64 {
        self.col_lower_nodes[col].len() as i64
    }

    pub fn num_nodes_down(&self, col: usize) -> i64 {
        self.col_upper_nodes[col].len() as i64
    }

    /// Nodes with a lower bound on `col` above `val` (all of them if
    /// none is at or below it).
    pub fn num_nodes_up_above(&self, col: usize, val: f64) -> i64 {
        let set = &self.col_lower_nodes[col];
        let above = set.range((Key(val), usize::MAX)..).filter(|&&(key, _)| key > Key(val)).count();
        if above == set.len() {
            return set.len() as i64;
        }
        above as i64
    }

    /// Nodes with an upper bound on `col` below `val`.
    pub fn num_nodes_down_below(&self, col: usize, val: f64) -> i64 {
        let set = &self.col_upper_nodes[col];
        let below = set.range(..(Key(val), 0)).count();
        if below == set.len() {
            return set.len() as i64;
        }
        below as i64
    }

    pub fn up_nodes(&self, col: usize) -> &NodeSet {
        &self.col_lower_nodes[col]
    }

    pub fn down_nodes(&self, col: usize) -> &NodeSet {
        &self.col_upper_nodes[col]
    }

    pub fn num_nodes(&self) -> usize {
        self.nodes.len() - self.freeslots.len()
    }

    pub fn num_active_nodes(&self) -> usize {
        self.nodes.len() - self.freeslots.len() - self.num_suboptimal
    }

    pub fn is_empty(&self) -> bool {
        self.num_active_nodes() == 0
    }
}
