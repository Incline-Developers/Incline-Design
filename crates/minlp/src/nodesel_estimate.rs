//! Port of `nodesel_estimate.c`: node selector for best estimate search.

use crate::{
    api,
    retcode::ScipResult,
    scip::Scip,
    types::{NodeId, NodeType, NodeselId},
    var::NONE,
};

const NODESEL_NAME: &str = "estimate";
const NODESEL_DESC: &str = "best estimate search";
const NODESEL_STDPRIORITY: i32 = 200000;
const NODESEL_MEMSAVEPRIORITY: i32 = 100;

/*
 * Default parameter settings
 */

/// Minimal plunging depth, before new best node may be selected (-1 for
/// dynamic setting).
const DEFAULT_MINPLUNGEDEPTH: i32 = -1;
/// Maximal plunging depth, before new best node is forced to be selected
/// (-1 for dynamic setting).
const DEFAULT_MAXPLUNGEDEPTH: i32 = -1;
/// Maximal quotient (curlowerbound - lowerbound)/(cutoffbound - lowerbound)
/// where plunging is performed.
const DEFAULT_MAXPLUNGEQUOT: f64 = 0.25;
/// Frequency at which the best node instead of the best estimate is
/// selected (0: never).
const DEFAULT_BESTNODEFREQ: i32 = 10;
/// Depth until breadth-first search is applied (-1: never).
const DEFAULT_BREADTHFIRSTDEPTH: i32 = -1;
/// Number of nodes before doing plunging the first time.
const DEFAULT_PLUNGEOFFSET: i32 = 0;

/// `SCIP_NodeselData`: node selector data for best estimate search node
/// selection.
#[derive(Default)]
pub struct NodeselData {
    /// Maximal quotient (curlowerbound - lowerbound)/(cutoffbound -
    /// lowerbound) where plunging is performed.
    pub maxplungequot: f64,
    /// Minimal plunging depth, before new best node may be selected (-1 for
    /// dynamic setting).
    pub minplungedepth: i32,
    /// Maximal plunging depth, before new best node is forced to be selected
    /// (-1 for dynamic setting).
    pub maxplungedepth: i32,
    /// Frequency at which the best node instead of the best estimate is
    /// selected (0: never).
    pub bestnodefreq: i32,
    /// Depth until breadth-fisrt search is applied.
    pub breadthfirstdepth: i32,
    /// Number of nodes before doing plunging the first time.
    pub plungeoffset: i32,
}

fn nd(scip: &Scip, nodesel: NodeselId) -> &NodeselData {
    scip.nodesels[nodesel].nodeseldata.as_ref().unwrap().downcast_ref::<NodeselData>().unwrap()
}

/*
 * Callback methods
 */

/// The best bound or best node, by `bestnodefreq`.
fn best_or_bestbound(scip: &Scip, bestnodefreq: i32) -> NodeId {
    if api::get_n_nodes(scip) % bestnodefreq as i64 == 0 {
        api::get_bestbound_node(scip)
    } else {
        api::get_best_node(scip)
    }
}

/// `nodeselSelectEstimate`: node selection method of node selector.
fn nodesel_select_estimate(scip: &mut Scip, nodesel: NodeselId, selnode: &mut NodeId) -> ScipResult {
    *selnode = NONE;

    // Get node selector user data.
    let d = nd(scip, nodesel);
    let (breadthfirstdepth, bestnodefreq0, plungeoffset) = (d.breadthfirstdepth, d.bestnodefreq, d.plungeoffset);
    let (mut minplungedepth, mut maxplungedepth, maxplungequot) = (d.minplungedepth, d.maxplungedepth, d.maxplungequot);

    // Check if the breadth-first search should be applied.
    if api::get_depth(scip) <= breadthfirstdepth {
        let node = api::get_prio_sibling(scip);
        if node != NONE {
            *selnode = node;
            return Ok(());
        }

        let node = api::get_prio_child(scip);
        if node != NONE {
            *selnode = node;
            return Ok(());
        }
    }

    let bestnodefreq = if bestnodefreq0 == 0 { i32::MAX } else { bestnodefreq0 };

    // Check if we don't want to do plunging yet.
    if api::get_n_nodes(scip) < plungeoffset as i64 {
        // We don't want to plunge yet: select best node from the tree.
        *selnode = best_or_bestbound(scip, bestnodefreq);
        return Ok(());
    }

    // Calculate minimal and maximal plunging depth.
    if minplungedepth == -1 {
        minplungedepth = api::get_max_depth(scip) / 10;
        if api::get_n_strongbranch_lp_iterations(scip) > 2 * api::get_n_node_lp_iterations(scip) {
            minplungedepth += 10;
        }
        if maxplungedepth >= 0 {
            minplungedepth = crate::def::min(minplungedepth, maxplungedepth);
        }
    }
    if maxplungedepth == -1 {
        maxplungedepth = api::get_max_depth(scip) / 2;
    }
    maxplungedepth = crate::def::max(maxplungedepth, minplungedepth);

    // Check, if we exceeded the maximal plunging depth.
    let plungedepth = api::get_plunge_depth(scip);
    if plungedepth > maxplungedepth {
        // We don't want to plunge again: select best node from the tree.
        *selnode = best_or_bestbound(scip, bestnodefreq);
    } else {
        // Get global lower and cutoff bound.
        let lowerbound = api::get_lowerbound(scip);
        let mut cutoffbound = api::get_cutoffbound(scip);

        // If we didn't find a solution yet, the cutoff bound is usually very bad:
        // use only 20% of the gap as cutoff bound.
        if api::get_n_sols_found(scip) == 0 {
            cutoffbound = lowerbound + 0.2 * (cutoffbound - lowerbound);
        }

        // Check, if plunging is forced at the current depth.
        let maxbound = if plungedepth < minplungedepth {
            scip.set.infinity()
        } else {
            // Calculate maximal plunging bound.
            lowerbound + maxplungequot * (cutoffbound - lowerbound)
        };

        // We want to plunge again: prefer children over siblings, and siblings
        // over leaves, but only select a child or sibling, if its estimate is
        // small enough; prefer using nodes with higher node selection priority
        // assigned by the branching rule.
        let small = |scip: &Scip, node: NodeId| node != NONE && scip.nodes[node].estimate < maxbound;
        let node = api::get_prio_child(scip);
        if small(scip, node) {
            *selnode = node;
        } else {
            let node = api::get_best_child(scip);
            if small(scip, node) {
                *selnode = node;
            } else {
                let node = api::get_prio_sibling(scip);
                if small(scip, node) {
                    *selnode = node;
                } else {
                    let node = api::get_best_sibling(scip);
                    if small(scip, node) {
                        *selnode = node;
                    } else {
                        *selnode = best_or_bestbound(scip, bestnodefreq);
                    }
                }
            }
        }
    }

    Ok(())
}

/// `nodeselCompEstimate`: node comparison method of node selector.
fn nodesel_comp_estimate(scip: &Scip, _nodesel: NodeselId, node1: NodeId, node2: NodeId) -> i32 {
    let set = &scip.set;
    let n1 = &scip.nodes[node1];
    let n2 = &scip.nodes[node2];
    let estimate1 = n1.estimate;
    let estimate2 = n2.estimate;

    if (set.is_infinity(estimate1) && set.is_infinity(estimate2)) || (set.is_infinity(-estimate1) && set.is_infinity(-estimate2)) || set.is_eq(estimate1, estimate2) {
        let lowerbound1 = n1.lowerbound;
        let lowerbound2 = n2.lowerbound;

        if set.is_lt(lowerbound1, lowerbound2) {
            return -1;
        } else if set.is_gt(lowerbound1, lowerbound2) {
            return 1;
        } else {
            let nodetype1 = n1.nodetype;
            let nodetype2 = n2.nodetype;
            if nodetype1 == NodeType::Child && nodetype2 != NodeType::Child {
                return -1;
            } else if nodetype1 != NodeType::Child && nodetype2 == NodeType::Child {
                return 1;
            } else if nodetype1 == NodeType::Sibling && nodetype2 != NodeType::Sibling {
                return -1;
            } else if nodetype1 != NodeType::Sibling && nodetype2 == NodeType::Sibling {
                return 1;
            } else {
                let depth1 = n1.depth;
                let depth2 = n2.depth;
                if depth1 < depth2 {
                    return -1;
                } else if depth1 > depth2 {
                    return 1;
                } else {
                    return 0;
                }
            }
        }
    }

    if set.is_lt(estimate1, estimate2) {
        return -1;
    }

    1
}

/*
 * estimate specific interface methods
 */

/// `nodeselCopyEstimate`: copy method for node selector plugins (called
/// when SCIP copies plugins).
fn nodesel_copy_estimate(scip: &mut Scip, _sourcescip: &Scip, _nodesel: NodeselId) -> ScipResult {
    // Call inclusion method of node selector.
    include_nodesel_estimate(scip)?;
    Ok(())
}

/// `SCIPincludeNodeselEstimate`: creates the node selector for best
/// estimate search and includes it in SCIP.
pub fn include_nodesel_estimate(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_int, paramset_add_real};

    // Include node selector.
    let nodesel = api::include_nodesel_basic(
        scip,
        NODESEL_NAME,
        NODESEL_DESC,
        NODESEL_STDPRIORITY,
        NODESEL_MEMSAVEPRIORITY,
        nodesel_select_estimate,
        nodesel_comp_estimate,
        Some(Box::new(NodeselData::default())),
    )?;
    scip.nodesels[nodesel].nodeselcopy = Some(nodesel_copy_estimate);

    // Add node selector parameters.
    paramset_add_int(
        scip,
        "nodeselection/estimate/minplungedepth",
        "minimal plunging depth, before new best node may be selected (-1 for dynamic setting)",
        crate::data_target!(i32, nodesels, nodesel, nodeseldata, NodeselData, minplungedepth),
        true,
        DEFAULT_MINPLUNGEDEPTH,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "nodeselection/estimate/maxplungedepth",
        "maximal plunging depth, before new best node is forced to be selected (-1 for dynamic setting)",
        crate::data_target!(i32, nodesels, nodesel, nodeseldata, NodeselData, maxplungedepth),
        true,
        DEFAULT_MAXPLUNGEDEPTH,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "nodeselection/estimate/maxplungequot",
        "maximal quotient (estimate - lowerbound)/(cutoffbound - lowerbound) where plunging is performed",
        crate::data_target!(f64, nodesels, nodesel, nodeseldata, NodeselData, maxplungequot),
        true,
        DEFAULT_MAXPLUNGEQUOT,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "nodeselection/estimate/bestnodefreq",
        "frequency at which the best node instead of the best estimate is selected (0: never)",
        crate::data_target!(i32, nodesels, nodesel, nodeseldata, NodeselData, bestnodefreq),
        false,
        DEFAULT_BESTNODEFREQ,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "nodeselection/estimate/breadthfirstdepth",
        "depth until breadth-first search is applied",
        crate::data_target!(i32, nodesels, nodesel, nodeseldata, NodeselData, breadthfirstdepth),
        false,
        DEFAULT_BREADTHFIRSTDEPTH,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "nodeselection/estimate/plungeoffset",
        "number of nodes before doing plunging the first time",
        crate::data_target!(i32, nodesels, nodesel, nodeseldata, NodeselData, plungeoffset),
        false,
        DEFAULT_PLUNGEOFFSET,
        0,
        i32::MAX,
        None,
    )?;

    Ok(())
}
