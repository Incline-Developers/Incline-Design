//! Port of `nodesel_restartdfs.c`: node selector for depth first search
//! with periodical selection of the best node (which the sub-SCIP of the
//! LP face heuristic uses).

use crate::{
    api,
    retcode::ScipResult,
    scip::Scip,
    types::{NodeId, NodeselId},
    var::NONE,
};

const NODESEL_NAME: &str = "restartdfs";
const NODESEL_DESC: &str = "depth first search with periodical selection of the best node";
const NODESEL_STDPRIORITY: i32 = 10000;
const NODESEL_MEMSAVEPRIORITY: i32 = 50000;

/*
 * Default parameter settings
 */

/// Frequency for selecting the best node instead of the deepest one.
const SELECTBESTFREQ: i32 = 100;
/// Only count leaf nodes or all nodes.
const COUNTONLYLEAVES: bool = true;

/// `SCIP_NodeselData`: node selector data for best first search node
/// selection.
struct NodeselData {
    /// Node number where the last best node was selected.
    lastrestart: i64,
    /// Number of processed leafs since the last restart.
    nprocessedleaves: i64,
    /// Frequency for selecting the best node instead of the deepest one.
    selectbestfreq: i32,
    /// Only count leaf nodes or all nodes.
    countonlyleaves: bool,
}

fn ndm(scip: &mut Scip, nodesel: NodeselId) -> &mut NodeselData {
    scip.nodesels[nodesel].nodeseldata.as_mut().unwrap().downcast_mut::<NodeselData>().unwrap()
}

/*
 * Callback methods
 */

/// `nodeselCopyRestartdfs`: copy method for node selector plugins (called
/// when SCIP copies plugins).
fn nodesel_copy_restartdfs(scip: &mut Scip, _sourcescip: &Scip, _nodesel: NodeselId) -> ScipResult {
    // Call inclusion method of node selector.
    include_nodesel_restartdfs(scip)
}

/// `nodeselInitsolRestartdfs`: solving process initialization method of
/// node selector (called when branch and bound process is about to begin).
fn nodesel_initsol_restartdfs(scip: &mut Scip, nodesel: NodeselId) -> ScipResult {
    // Reset counters.
    let d = ndm(scip, nodesel);
    d.lastrestart = 0;
    d.nprocessedleaves = 0;
    Ok(())
}

/// `nodeselSelectRestartdfs`: node selection method of node selector.
fn nodesel_select_restartdfs(scip: &mut Scip, nodesel: NodeselId, selnode: &mut NodeId) -> ScipResult {
    // Decide if we want to select the node with lowest bound or the deepest
    // node; finish the current dive in any case.
    *selnode = api::get_prio_child(scip);
    if *selnode == NONE {
        let nnodes = api::get_n_nodes(scip);

        // Increase the number of processed leafs since we are in a leaf.
        let d = ndm(scip, nodesel);
        d.nprocessedleaves += 1;

        // Check if in case of "only leaves" the number processed leaves
        // exceeds the frequency or in the other case the number of processed
        // node does it.
        if (d.countonlyleaves && d.nprocessedleaves >= d.selectbestfreq as i64) || (!d.countonlyleaves && nnodes - d.lastrestart >= d.selectbestfreq as i64) {
            d.lastrestart = nnodes;
            d.nprocessedleaves = 0;
            *selnode = api::get_bestbound_node(scip);
        } else {
            *selnode = api::get_prio_sibling(scip);
            if *selnode == NONE {
                *selnode = api::get_best_leaf(scip);
            }
        }
    }
    Ok(())
}

/// `nodeselCompRestartdfs`: node comparison method of node selector.
fn nodesel_comp_restartdfs(scip: &Scip, _nodesel: NodeselId, node1: NodeId, node2: NodeId) -> i32 {
    (scip.nodes[node2].number - scip.nodes[node1].number) as i32
}

/*
 * restartdfs specific interface methods
 */

/// `SCIPincludeNodeselRestartdfs`: creates the node selector for restarting
/// depth first search and includes it in SCIP.
pub fn include_nodesel_restartdfs(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_int};

    // Allocate and initialize node selector data.
    let nodeseldata = NodeselData {
        lastrestart: 0,
        nprocessedleaves: 0,
        selectbestfreq: SELECTBESTFREQ,
        countonlyleaves: COUNTONLYLEAVES,
    };

    // Include node selector.
    let nodesel = api::include_nodesel_basic(
        scip,
        NODESEL_NAME,
        NODESEL_DESC,
        NODESEL_STDPRIORITY,
        NODESEL_MEMSAVEPRIORITY,
        nodesel_select_restartdfs,
        nodesel_comp_restartdfs,
        Some(Box::new(nodeseldata)),
    )?;
    scip.nodesels[nodesel].nodeselcopy = Some(nodesel_copy_restartdfs);
    scip.nodesels[nodesel].nodeselinitsol = Some(nodesel_initsol_restartdfs);

    // Add node selector parameters.
    paramset_add_int(
        scip,
        "nodeselection/restartdfs/selectbestfreq",
        "frequency for selecting the best node instead of the deepest one",
        crate::data_target!(i32, nodesels, nodesel, nodeseldata, NodeselData, selectbestfreq),
        false,
        SELECTBESTFREQ,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "nodeselection/restartdfs/countonlyleaves",
        "count only leaf nodes (otherwise all nodes)?",
        crate::data_target!(bool, nodesels, nodesel, nodeseldata, NodeselData, countonlyleaves),
        false,
        COUNTONLYLEAVES,
        None,
    )?;

    Ok(())
}
