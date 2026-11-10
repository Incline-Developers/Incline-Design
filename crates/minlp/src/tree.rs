//! SCIP's branch-and-bound tree (`tree.c`, `struct_tree.h`).

use std::cmp::Ordering;

use crate::{
    cons::{self, ConsSetChg},
    cvec::CVec,
    event, lp,
    lpi::{LpiNorms, LpiState},
    nodesel::NodePq,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{BoundType, BoundchgType, BranchDir, ColId, ConsId, NodeId, NodeType, PropId, RowId, VarId, VarStatus},
    var::{DomChg, NONE},
};

/// `SCIP_PROBINGNODE`.
#[derive(Clone, Debug, Default)]
pub struct ProbingNode {
    pub lpistate: Option<Box<LpiState>>,
    pub lpinorms: Option<Box<LpiNorms>>,
    pub ninitialcols: i32,
    pub ninitialrows: i32,
    pub ncols: i32,
    pub nrows: i32,
    pub origobjvars: CVec<VarId>,
    pub origobjvals: CVec<f64>,
    pub nchgdobjs: i32,
    pub lpwasprimfeas: bool,
    pub lpwasprimchecked: bool,
    pub lpwasdualfeas: bool,
    pub lpwasdualchecked: bool,
}
/// `SCIP_SIBLING`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sibling {
    pub arraypos: i32,
}
/// `SCIP_CHILD`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Child {
    pub arraypos: i32,
}
/// `SCIP_LEAF`.
#[derive(Clone, Copy, Debug)]
pub struct Leaf {
    pub lpstatefork: NodeId,
}
/// `SCIP_JUNCTION`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Junction {
    pub nchildren: i32,
}
/// `SCIP_PSEUDOFORK`.
#[derive(Clone, Debug, Default)]
pub struct PseudoFork {
    pub addedcols: CVec<ColId>,
    pub addedrows: CVec<RowId>,
    pub naddedcols: i32,
    pub naddedrows: i32,
    pub nchildren: i32,
}
/// `SCIP_FORK`.
#[derive(Clone, Debug, Default)]
pub struct Fork {
    pub addedcols: CVec<ColId>,
    pub addedrows: CVec<RowId>,
    pub lpistate: Option<Box<LpiState>>,
    pub lpobjval: f64,
    pub naddedcols: i32,
    pub naddedrows: i32,
    pub nlpistateref: i32,
    pub nchildren: u32,
    pub lpwasprimfeas: bool,
    pub lpwasprimchecked: bool,
    pub lpwasdualfeas: bool,
    pub lpwasdualchecked: bool,
}
/// `SCIP_SUBROOT`.
#[derive(Clone, Debug, Default)]
pub struct Subroot {
    pub cols: CVec<ColId>,
    pub rows: CVec<RowId>,
    pub lpistate: Option<Box<LpiState>>,
    pub lpobjval: f64,
    pub ncols: i32,
    pub nrows: i32,
    pub nlpistateref: i32,
    pub nchildren: u32,
    pub lpwasprimfeas: bool,
    pub lpwasprimchecked: bool,
    pub lpwasdualfeas: bool,
    pub lpwasdualchecked: bool,
}
/// `SCIP_NODE`.
#[derive(Clone, Debug)]
pub struct Node {
    pub number: i64,
    pub lowerbound: f64,
    pub estimate: f64,
    pub data: NodeData,
    pub parent: NodeId,
    pub conssetchg: Option<Box<ConsSetChg>>,
    pub domchg: Option<Box<DomChg>>,
    pub depth: u32,
    pub reoptid: u32,
    pub reopttype: u32,
    pub repropsubtreemark: u32,
    pub active: bool,
    pub cutoff: bool,
    pub reprop: bool,
    pub nodetype: NodeType,
}
/// `SCIP_PENDINGBDCHG`.
#[derive(Clone, Copy, Debug)]
pub struct PendingBdchg {
    pub node: NodeId,
    pub var: VarId,
    pub newbound: f64,
    pub boundtype: BoundType,
    pub infercons: ConsId,
    pub inferprop: PropId,
    pub inferinfo: i32,
    pub probingchange: bool,
}
/// `SCIP_TREE`.
pub struct Tree {
    pub root: NodeId,
    pub leaves: NodePq,
    pub path: CVec<NodeId>,
    pub focusnode: NodeId,
    pub focuslpfork: NodeId,
    pub focuslpstatefork: NodeId,
    pub focussubroot: NodeId,
    pub probingroot: NodeId,
    pub children: CVec<NodeId>,
    pub siblings: CVec<NodeId>,
    pub childrenprio: CVec<f64>,
    pub siblingsprio: CVec<f64>,
    pub divebdchgvars: [CVec<VarId>; 2],
    pub divebdchgdirs: [CVec<BranchDir>; 2],
    pub divebdchgvals: [CVec<f64>; 2],
    pub pathnlpcols: CVec<i32>,
    pub pathnlprows: CVec<i32>,
    pub probinglpistate: Option<Box<LpiState>>,
    pub focuslpistate: Option<Box<LpiState>>,
    pub probinglpinorms: Option<Box<LpiNorms>>,
    pub pendingbdchgs: CVec<PendingBdchg>,
    pub probdiverelaxsol: CVec<f64>,
    pub probinglpobjval: f64,
    pub nprobdiverelaxsol: i32,
    pub focuslpstateforklpcount: i64,
    pub lastbranchparentid: i64,
    pub divebdchgsize: [i32; 2],
    pub ndivebdchanges: [i32; 2],
    pub pendingbdchgssize: i32,
    pub npendingbdchgs: i32,
    pub childrensize: i32,
    pub nchildren: i32,
    pub siblingssize: i32,
    pub nsiblings: i32,
    pub pathlen: i32,
    pub pathsize: i32,
    pub effectiverootdepth: i32,
    pub updatedeffectiverootdepth: i32,
    pub correctlpdepth: i32,
    pub cutoffdepth: i32,
    pub repropdepth: i32,
    pub repropsubtreecount: i32,
    pub probingsumchgdobjs: i32,
    pub focusnodehaslp: bool,
    pub probingnodehaslp: bool,
    pub focuslpconstructed: bool,
    pub cutoffdelayed: bool,
    pub probinglpwasflushed: bool,
    pub probinglpwassolved: bool,
    pub probingloadlpistate: bool,
    pub probinglpwasrelax: bool,
    pub probingsolvedlp: bool,
    pub forcinglpmessage: bool,
    pub probingobjchanged: bool,
    pub sbprobing: bool,
    pub probinglpwasprimfeas: bool,
    pub probinglpwasprimchecked: bool,
    pub probinglpwasdualfeas: bool,
    pub probinglpwasdualchecked: bool,
    pub probinglphadsafebound: bool,
    pub probdiverelaxstored: bool,
    pub probdiverelaxincludeslp: bool,
}

/// The `data` union of `SCIP_NODE`.
#[derive(Clone, Debug, Default)]
pub struct NodeData {
    pub probingnode: Option<Box<ProbingNode>>,
    pub sibling: Sibling,
    pub child: Child,
    pub leaf: Leaf,
    pub junction: Junction,
    pub pseudofork: Option<Box<PseudoFork>>,
    pub fork: Option<Box<Fork>>,
    pub subroot: Option<Box<Subroot>>,
}

impl Default for Leaf {
    fn default() -> Self {
        Self { lpstatefork: NONE }
    }
}

/// Maximal subtree repropagation marker; must correspond to node data
/// structure.
pub const MAXREPROPMARK: u32 = 511;

/*
 * dynamic memory arrays
 */

/// `treeEnsureChildrenMem`: resizes children arrays to be able to store at
/// least `num` nodes.
fn tree_ensure_children_mem(scip: &mut Scip, num: i32) {
    if num > treer!(scip).childrensize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let tree = &mut tree!(scip);
        tree.children.realloc(newsize);
        tree.childrenprio.realloc(newsize);
        tree.childrensize = newsize;
    }
}

/// `treeEnsurePathMem`: resizes path array to be able to store at least
/// `num` nodes.
fn tree_ensure_path_mem(scip: &mut Scip, num: i32) {
    if num > treer!(scip).pathsize {
        let newsize = scip.set.calc_path_grow_size(num);
        let tree = &mut tree!(scip);
        tree.path.realloc(newsize);
        tree.pathnlpcols.realloc(newsize);
        tree.pathnlprows.realloc(newsize);
        tree.pathsize = newsize;
    }
}

/// `treeEnsurePendingbdchgsMem`: resizes pendingbdchgs array to be able
/// to store at least `num` nodes.
fn tree_ensure_pendingbdchgs_mem(scip: &mut Scip, num: i32) {
    if num > treer!(scip).pendingbdchgssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let tree = &mut tree!(scip);
        tree.pendingbdchgs.realloc(newsize);
        tree.pendingbdchgssize = newsize;
    }
}

impl Default for PendingBdchg {
    fn default() -> Self {
        Self {
            node: NONE,
            var: NONE,
            newbound: 0.0,
            boundtype: BoundType::Lower,
            infercons: NONE,
            inferprop: NONE,
            inferinfo: 0,
            probingchange: false,
        }
    }
}

/*
 * Node methods
 */

/// `SCIPnodeCompLowerbound`: node comparator for best lower bound.
pub fn node_comp_lowerbound(scip: &Scip, elem1: NodeId, elem2: NodeId) -> Ordering {
    let (a, b) = (scip.nodes[elem1].lowerbound, scip.nodes[elem2].lowerbound);
    if a < b {
        Ordering::Less
    } else if a > b {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// `forkCaptureLPIState`: increases the reference counter of the LP state
/// in the fork.
fn fork_capture_lpistate(fork: &mut Fork, nuses: i32) {
    fork.nlpistateref += nuses;
}

/// `forkReleaseLPIState`: decreases the reference counter of the LP state
/// in the fork.
fn fork_release_lpistate(fork: &mut Fork) {
    fork.nlpistateref -= 1;
    if fork.nlpistateref == 0 {
        fork.lpistate = None;
    }
}

/// `subrootCaptureLPIState`: increases the reference counter of the LP
/// state in the subroot.
fn subroot_capture_lpistate(subroot: &mut Subroot, nuses: i32) {
    subroot.nlpistateref += nuses;
}

/// `subrootReleaseLPIState`: decreases the reference counter of the LP
/// state in the subroot.
fn subroot_release_lpistate(subroot: &mut Subroot) {
    subroot.nlpistateref -= 1;
    if subroot.nlpistateref == 0 {
        subroot.lpistate = None;
    }
}

/// `SCIPnodeCaptureLPIState`: increases the reference counter of the LP
/// state in the fork or subroot node.
pub fn node_capture_lpistate(scip: &mut Scip, node: NodeId, nuses: i32) -> ScipResult {
    let n = &mut scip.nodes[node];
    match n.nodetype {
        NodeType::Fork => fork_capture_lpistate(n.data.fork.as_mut().unwrap(), nuses),
        NodeType::Subroot => subroot_capture_lpistate(n.data.subroot.as_mut().unwrap(), nuses),
        _ => {
            log::error!("node for capturing the LPI state is neither fork nor subroot");
            return Err(Retcode::InvalidData);
        }
    }
    Ok(())
}

/// `SCIPnodeReleaseLPIState`: decreases the reference counter of the LP
/// state in the fork or subroot node.
pub fn node_release_lpistate(scip: &mut Scip, node: NodeId) -> ScipResult {
    let n = &mut scip.nodes[node];
    match n.nodetype {
        NodeType::Fork => fork_release_lpistate(n.data.fork.as_mut().unwrap()),
        NodeType::Subroot => subroot_release_lpistate(n.data.subroot.as_mut().unwrap()),
        _ => {
            log::error!("node for releasing the LPI state is neither fork nor subroot");
            return Err(Retcode::InvalidData);
        }
    }
    Ok(())
}

/// `probingnodeCreate`: creates probingnode data without LP information.
fn probingnode_create(scip: &Scip) -> Box<ProbingNode> {
    let ninitialcols = lp::lp_get_n_cols(scip);
    let ninitialrows = lp::lp_get_n_rows(scip);
    Box::new(ProbingNode {
        lpistate: None,
        lpinorms: None,
        ninitialcols,
        ninitialrows,
        ncols: ninitialcols,
        nrows: ninitialrows,
        origobjvars: CVec::new(),
        origobjvals: CVec::new(),
        nchgdobjs: 0,
        ..Default::default()
    })
}

/// `probingnodeUpdate`: updates LP information in probingnode data.
fn probingnode_update(scip: &mut Scip, node: NodeId) -> ScipResult {
    let mut storenorms = false;
    let mut pn = scip.nodes[node].data.probingnode.take().unwrap();

    // Free old LP state.
    pn.lpistate = None;

    // Free old LP norms.
    if pn.lpinorms.is_some() {
        pn.lpinorms = None;
        storenorms = true;
    }

    // Get current LP state.
    if lp!(scip).flushed && lp!(scip).solved {
        pn.lpistate = lp::lp_get_state(scip)?;

        // If LP norms were stored at this node before, store the new ones.
        if storenorms {
            pn.lpinorms = lp::lp_get_norms(scip)?;
        }
        let lp = &lp!(scip);
        pn.lpwasprimfeas = lp.primalfeasible;
        pn.lpwasprimchecked = lp.primalchecked;
        pn.lpwasdualfeas = lp.dualfeasible;
        pn.lpwasdualchecked = lp.dualchecked;
    } else {
        pn.lpistate = None;
    }

    pn.ncols = lp::lp_get_n_cols(scip);
    pn.nrows = lp::lp_get_n_rows(scip);
    scip.nodes[node].data.probingnode = Some(pn);
    Ok(())
}

/// `junctionInit`: initializes junction data.
fn junction_init(scip: &mut Scip, node: NodeId) -> ScipResult {
    let nchildren = treer!(scip).nchildren;
    scip.nodes[node].data.junction.nchildren = nchildren;

    // Increase the LPI state usage counter of the current LP fork.
    let focuslpstatefork = treer!(scip).focuslpstatefork;
    if focuslpstatefork != NONE {
        node_capture_lpistate(scip, focuslpstatefork, nchildren)?;
    }
    Ok(())
}

/// `pseudoforkCreate`: creates pseudofork data.
fn pseudofork_create(scip: &mut Scip) -> ScipResult<Box<PseudoFork>> {
    let mut pseudofork = Box::new(PseudoFork {
        addedcols: CVec::new(),
        addedrows: CVec::new(),
        naddedcols: lp::lp_get_n_newcols(scip),
        naddedrows: lp::lp_get_n_newrows(scip),
        nchildren: treer!(scip).nchildren,
    });

    if pseudofork.naddedcols > 0 {
        // Copy the newly created columns to the pseudofork's col array.
        pseudofork.addedcols = CVec(lp::lp_get_newcols(scip).to_vec());
    }
    if pseudofork.naddedrows > 0 {
        // Copy the newly created rows to the pseudofork's row array.
        pseudofork.addedrows = CVec(lp::lp_get_newrows(scip).to_vec());

        // Capture the added rows.
        for i in 0..pseudofork.naddedrows {
            lp::row_capture(scip, pseudofork.addedrows[i]);
        }
    }

    // Increase the LPI state usage counter of the current LP fork.
    let (focuslpstatefork, nchildren) = (treer!(scip).focuslpstatefork, treer!(scip).nchildren);
    if focuslpstatefork != NONE {
        node_capture_lpistate(scip, focuslpstatefork, nchildren)?;
    }
    Ok(pseudofork)
}

/// `pseudoforkFree`: frees pseudofork data.
fn pseudofork_free(scip: &mut Scip, pseudofork: PseudoFork) -> ScipResult {
    // Release the added rows.
    for i in 0..pseudofork.naddedrows {
        lp::row_release(scip, pseudofork.addedrows[i])?;
    }
    Ok(())
}

/// `forkCreate`: creates fork data.
fn fork_create(scip: &mut Scip, prob: usize) -> ScipResult<Box<Fork>> {
    let lpistate = lp::lp_get_state(scip)?;
    let lpobjval = lp::lp_get_objval(scip, prob);
    let lp = &lp!(scip);
    let mut fork = Box::new(Fork {
        lpistate,
        lpwasprimfeas: lp.primalfeasible,
        lpwasprimchecked: lp.primalchecked,
        lpwasdualfeas: lp.dualfeasible,
        lpwasdualchecked: lp.dualchecked,
        lpobjval,
        nlpistateref: 0,
        addedcols: CVec::new(),
        addedrows: CVec::new(),
        naddedcols: lp::lp_get_n_newcols(scip),
        naddedrows: lp::lp_get_n_newrows(scip),
        nchildren: treer!(scip).nchildren as u32,
    });

    if fork.naddedcols > 0 {
        // Copy the newly created columns to the fork's col array.
        fork.addedcols = CVec(lp::lp_get_newcols(scip).to_vec());
    }
    if fork.naddedrows > 0 {
        // Copy the newly created rows to the fork's row array.
        fork.addedrows = CVec(lp::lp_get_newrows(scip).to_vec());

        // Capture the added rows.
        for i in 0..fork.naddedrows {
            lp::row_capture(scip, fork.addedrows[i]);
        }
    }

    // Capture the LPI state for the children.
    let nchildren = treer!(scip).nchildren;
    fork_capture_lpistate(&mut fork, nchildren);
    Ok(fork)
}

/// `forkFree`: frees fork data.
fn fork_free(scip: &mut Scip, fork: Fork) -> ScipResult {
    // Release the added rows.
    for i in (0..fork.naddedrows).rev() {
        lp::row_release(scip, fork.addedrows[i])?;
    }
    Ok(())
}

/// `subrootFree`: frees subroot.
fn subroot_free(scip: &mut Scip, subroot: Subroot) -> ScipResult {
    // Release the rows of the subroot.
    for i in 0..subroot.nrows {
        lp::row_release(scip, subroot.rows[i])?;
    }
    Ok(())
}

/// `treeRemoveSibling`: removes given sibling node from the siblings
/// array.
fn tree_remove_sibling(scip: &mut Scip, sibling: NodeId) {
    let delpos = scip.nodes[sibling].data.sibling.arraypos;
    let tree = &mut tree!(scip);

    // Move last sibling in array to position of removed sibling.
    let last = tree.nsiblings - 1;
    tree.siblings[delpos] = tree.siblings[last];
    tree.siblingsprio[delpos] = tree.siblingsprio[last];
    let moved = tree.siblings[delpos];
    tree.nsiblings -= 1;
    scip.nodes[moved].data.sibling.arraypos = delpos;
    scip.nodes[sibling].data.sibling.arraypos = -1;
}

/// `treeAddChild`: adds given child node to children array of focus node.
fn tree_add_child(scip: &mut Scip, child: NodeId, nodeselprio: f64) {
    let n = treer!(scip).nchildren;
    tree_ensure_children_mem(scip, n + 1);
    let tree = &mut tree!(scip);
    tree.children[n] = child;
    tree.childrenprio[n] = nodeselprio;
    tree.nchildren += 1;
    scip.nodes[child].data.child.arraypos = n;
}

/// `treeRemoveChild`: removes given child node from the children array.
fn tree_remove_child(scip: &mut Scip, child: NodeId) {
    let delpos = scip.nodes[child].data.child.arraypos;
    let tree = &mut tree!(scip);

    // Move last child in array to position of removed child.
    let last = tree.nchildren - 1;
    tree.children[delpos] = tree.children[last];
    tree.childrenprio[delpos] = tree.childrenprio[last];
    let moved = tree.children[delpos];
    tree.nchildren -= 1;
    scip.nodes[moved].data.child.arraypos = delpos;
    scip.nodes[child].data.child.arraypos = -1;
}

/// `nodeAssignParent`: makes node a child of the given parent node, which
/// must be the focus node; if the child is a probing node, the parent node
/// can also be a refocused node or a probing node.
fn node_assign_parent(scip: &mut Scip, node: NodeId, parent: NodeId, nodeselprio: f64) -> ScipResult {
    // Link node to parent.
    scip.nodes[node].parent = parent;
    if parent != NONE {
        let (lowerbound, estimate, depth) = {
            let p = &scip.nodes[parent];
            (p.lowerbound, p.estimate, p.depth)
        };
        let n = &mut scip.nodes[node];
        n.lowerbound = lowerbound;
        n.estimate = estimate;
        n.depth = depth + 1;
        if depth as i32 >= crate::def::MAXTREEDEPTH {
            log::error!("maximal depth level exceeded");
            return Err(Retcode::MaxDepthLevel);
        }
    }

    // Register node in the childlist of the focus (the parent) node.
    if scip.nodes[node].nodetype == NodeType::Child {
        tree_add_child(scip, node, nodeselprio);
    }
    Ok(())
}

/// `nodeReleaseParent`: frees node memory, decreases number of children of
/// the parent, and replaces node with parent if no children left; returns
/// the next node to free, or `NONE`.
fn node_release_parent(scip: &mut Scip, node: NodeId) -> ScipResult<NodeId> {
    let mut free_parent = false;
    let parent = scip.nodes[node].parent;
    if parent != NONE {
        match scip.nodes[parent].nodetype {
            NodeType::FocusNode => {
                if scip.nodes[node].nodetype == NodeType::Child {
                    tree_remove_child(scip, node);
                }
                // Don't kill the focus node at this point => freeParent = FALSE.
            }
            NodeType::ProbingNode => {
                // Probing nodes have to be freed individually => freeParent = FALSE.
            }
            NodeType::Sibling => {
                log::error!("sibling cannot be a parent node");
                return Err(Retcode::InvalidData);
            }
            NodeType::Child => {
                log::error!("child cannot be a parent node");
                return Err(Retcode::InvalidData);
            }
            NodeType::Leaf => {
                log::error!("leaf cannot be a parent node");
                return Err(Retcode::InvalidData);
            }
            NodeType::DeadEnd => {
                log::error!("dead-end cannot be a parent node");
                return Err(Retcode::InvalidData);
            }
            NodeType::Junction => {
                let p = &mut scip.nodes[parent].data.junction;
                p.nchildren -= 1;
                free_parent = p.nchildren == 0; // free parent if it has no more children
            }
            NodeType::PseudoFork => {
                let p = scip.nodes[parent].data.pseudofork.as_mut().unwrap();
                p.nchildren -= 1;
                free_parent = p.nchildren == 0;
            }
            NodeType::Fork => {
                let p = scip.nodes[parent].data.fork.as_mut().unwrap();
                p.nchildren -= 1;
                free_parent = p.nchildren == 0;
            }
            NodeType::Subroot => {
                let p = scip.nodes[parent].data.subroot.as_mut().unwrap();
                p.nchildren -= 1;
                free_parent = p.nchildren == 0;
            }
            NodeType::RefocusNode => {
                // The only possible child a refocused node can have in its
                // refocus state is the probing root node; we don't want to
                // free the refocused node, because we first have to convert
                // it back to its original type (where it possibly has
                // children) => freeParent = FALSE.
            }
        }

        // Update the effective root depth if active parent has children and
        // neither reoptimization nor certificate printing is enabled.
        if !free_parent && scip.nodes[parent].active && !scip.set.reopt_enable {
            let focusdepth = tree_get_focus_depth(scip);
            while treer!(scip).updatedeffectiverootdepth < focusdepth {
                let effectiveroot = treer!(scip).path[treer!(scip).updatedeffectiverootdepth];
                let e = &scip.nodes[effectiveroot];
                let single_child = match e.nodetype {
                    NodeType::FocusNode => {
                        log::error!("focus shallower than focus depth");
                        return Err(Retcode::InvalidData);
                    }
                    NodeType::ProbingNode => {
                        log::error!("probing shallower than focus depth");
                        return Err(Retcode::InvalidData);
                    }
                    NodeType::Sibling => {
                        log::error!("sibling shallower than focus depth");
                        return Err(Retcode::InvalidData);
                    }
                    NodeType::Child => {
                        log::error!("child shallower than focus depth");
                        return Err(Retcode::InvalidData);
                    }
                    NodeType::Leaf => {
                        log::error!("leaf on focus path");
                        return Err(Retcode::InvalidData);
                    }
                    NodeType::DeadEnd => {
                        log::error!("dead-end on focus path");
                        return Err(Retcode::InvalidData);
                    }
                    NodeType::Junction => e.data.junction.nchildren == 1,
                    NodeType::PseudoFork => e.data.pseudofork.as_ref().unwrap().nchildren == 1,
                    NodeType::Fork => e.data.fork.as_ref().unwrap().nchildren == 1,
                    NodeType::Subroot => e.data.subroot.as_ref().unwrap().nchildren == 1,
                    NodeType::RefocusNode => false,
                };

                if !single_child {
                    break;
                }

                tree!(scip).updatedeffectiverootdepth += 1;
            }
        }
    }

    scip.nodes.remove(node);

    // Free parent iteratively if it is not on the current active path.
    if free_parent && !scip.nodes[parent].active {
        return Ok(parent);
    }
    Ok(NONE)
}

/// `nodeCreate`: creates a node data structure.
fn node_create(scip: &mut Scip) -> NodeId {
    let infinity = scip.set.infinity();
    scip.nodes.push(Node {
        parent: NONE,
        conssetchg: None,
        domchg: None,
        number: 0,
        lowerbound: -infinity,
        estimate: -infinity,
        reoptid: 0,
        reopttype: 0, // SCIP_REOPTTYPE_NONE
        depth: 0,
        active: false,
        cutoff: false,
        reprop: false,
        repropsubtreemark: 0,
        data: NodeData::default(),
        nodetype: NodeType::default(),
    })
}

/// `SCIPnodeCreateChild`: creates a child node of the focus node.
pub fn node_create_child(scip: &mut Scip, nodeselprio: f64, estimate: f64) -> ScipResult<NodeId> {
    scip.stat.ncreatednodes += 1;
    scip.stat.ncreatednodesrun += 1;

    // Create the node data structure.
    let node = node_create(scip);
    scip.nodes[node].number = scip.stat.ncreatednodesrun;

    // Mark node to be a child node.
    scip.nodes[node].nodetype = NodeType::Child;
    scip.nodes[node].data.child.arraypos = -1;

    // Make focus node the parent of the new child.
    let focusnode = treer!(scip).focusnode;
    node_assign_parent(scip, node, focusnode, nodeselprio)?;

    // Update the estimate of the child.
    node_set_estimate(scip, node, estimate);

    tree!(scip).lastbranchparentid = if focusnode == NONE { -1 } else { scip.nodes[focusnode].number };
    Ok(node)
}

/// `SCIPtreeWasNodeLastBranchParent`: query if focus node was already
/// branched on; `node` is `NONE` for the focus node.
pub fn tree_was_node_last_branch_parent(scip: &mut Scip, node: NodeId) -> bool {
    let node = if node == NONE { treer!(scip).focusnode } else { node };
    node != NONE && scip.nodes[node].number == treer!(scip).lastbranchparentid
}

/// `SCIPnodeFree`: frees node and inactive path iteratively.
pub fn node_free(scip: &mut Scip, node: NodeId, eventfilter: usize) -> ScipResult {
    let mut node = node;
    loop {
        if scip.nodes[node].nodetype != NodeType::ProbingNode {
            // Trigger a node deletion event.
            let event = event::event_create_node(event::EVENTTYPE_NODEDELETE, node);
            event::event_process(scip, &event, eventfilter)?;
        }

        // Check, if the node to be freed is the root node.
        let isroot = scip.nodes[node].depth == 0;

        // Free nodetype specific data, and release no longer needed LPI
        // states.
        match scip.nodes[node].nodetype {
            NodeType::FocusNode => {
                log::error!("cannot free focus node - has to be converted into a dead end first");
                return Err(Retcode::InvalidData);
            }
            NodeType::ProbingNode => {
                scip.nodes[node].data.probingnode = None;
            }
            NodeType::Sibling => {
                let focuslpstatefork = treer!(scip).focuslpstatefork;
                if focuslpstatefork != NONE {
                    node_release_lpistate(scip, focuslpstatefork)?;
                }
                tree_remove_sibling(scip, node);
            }
            NodeType::Child => {
                // The children capture the LPI state at the moment, where
                // the focus node is converted into a junction, pseudofork,
                // fork, or subroot, and a new node is focused. At the same
                // time, they become siblings or leaves, such that freeing a
                // child of the focus node doesn't require to release the LPI
                // state; we don't need to call treeRemoveChild(), because
                // this is done in nodeReleaseParent().
            }
            NodeType::Leaf => {
                let lpstatefork = scip.nodes[node].data.leaf.lpstatefork;
                if lpstatefork != NONE {
                    node_release_lpistate(scip, lpstatefork)?;
                }
            }
            NodeType::DeadEnd | NodeType::Junction => {}
            NodeType::PseudoFork => {
                let pseudofork = scip.nodes[node].data.pseudofork.take().unwrap();
                pseudofork_free(scip, *pseudofork)?;
            }
            NodeType::Fork => {
                // Release special root LPI state capture which is used to
                // keep the root LPI state over the whole solving process.
                if isroot {
                    node_release_lpistate(scip, node)?;
                }
                let fork = scip.nodes[node].data.fork.take().unwrap();
                fork_free(scip, *fork)?;
            }
            NodeType::Subroot => {
                let subroot = scip.nodes[node].data.subroot.take().unwrap();
                subroot_free(scip, *subroot)?;
            }
            NodeType::RefocusNode => {
                log::error!("cannot free node as long it is refocused");
                return Err(Retcode::InvalidData);
            }
        }

        // Free common data.
        cons::conssetchg_free(scip, node)?;
        let mut domchg = scip.nodes[node].domchg.take();
        crate::var::domchg_free(scip, &mut domchg)?;

        // Check, if the node is the current probing root.
        if node == treer!(scip).probingroot {
            tree!(scip).probingroot = NONE;
        }

        // Delete the tree's root node pointer, if the freed node was the
        // root.
        if isroot {
            tree!(scip).root = NONE;
        }

        node = node_release_parent(scip, node)?;
        if node == NONE {
            break;
        }
    }
    Ok(())
}

/// `SCIPnodeCutoff`: cuts off node and whole sub tree from branch and
/// bound tree. Must not be used on a leaf because the node priority queue
/// remains untouched.
pub fn node_cutoff(scip: &mut Scip, node: NodeId, eventfilter: usize, transprob: usize, origprob: usize) -> ScipResult {
    // Check if the node should be stored for reoptimization.
    if scip.set.reopt_enable {
        unimplemented!("reoptimization");
    }

    let nodetype = scip.nodes[node].nodetype;
    let infinity = scip.set.infinity();
    {
        let n = &mut scip.nodes[node];
        n.cutoff = true;
        n.lowerbound = infinity;
        n.estimate = infinity;
    }

    let (active, depth) = (scip.nodes[node].active, scip.nodes[node].depth);
    if active && treer!(scip).cutoffdepth > depth as i32 {
        tree!(scip).cutoffdepth = depth as i32;
    }

    if depth == 0 {
        scip.stat.rootlowerbound = infinity;
    }

    // Update lower bound statistics during solving.
    if !scip.stat.inrestart && scip.set.stage >= crate::scip::Stage::InitSolve && (nodetype == NodeType::FocusNode || nodetype == NodeType::Child || nodetype == NodeType::Sibling)
    {
        let lowerbound = tree_get_lowerbound(scip);

        // Lower bound improved.
        if scip.stat.lastlowerbound < lowerbound {
            // Throw improvement event if not already done exactly.
            if scip.stat.lastlowerbound < scip.primals[crate::scip::TRANSPRIMAL].upperbound {
                let event = event::event_create_node(event::EVENTTYPE_DUALBOUNDIMPROVED, NONE);
                event::event_process(scip, &event, eventfilter)?;
            }

            // Update primal-dual integrals.
            if scip.set.misc_calcintegral {
                crate::stat::stat_update_primal_dual_integrals(scip, transprob, origprob, infinity, lowerbound);
            } else {
                scip.stat.lastlowerbound = lowerbound;
            }
        }
    }
    Ok(())
}

/// `SCIPnodePropagateAgain`: marks node, that propagation should be
/// applied again the next time, a node of its subtree is focused.
pub fn node_propagate_again(scip: &mut Scip, node: NodeId) {
    let n = &mut scip.nodes[node];
    if !n.reprop {
        n.reprop = true;
        if n.active {
            let depth = n.depth as i32;
            let tree = &mut tree!(scip);
            tree.repropdepth = tree.repropdepth.min(depth);
        }
    }
}

/// `SCIPnodeMarkPropagated`: marks node, that it is completely propagated
/// in the current repropagation subtree level.
pub fn node_mark_propagated(scip: &mut Scip, node: NodeId) {
    let parent = scip.nodes[node].parent;
    if parent != NONE {
        scip.nodes[node].repropsubtreemark = scip.nodes[parent].repropsubtreemark;
    }
    scip.nodes[node].reprop = false;

    // If the node was the highest repropagation node in the path, update the
    // repropdepth in the tree data.
    let (active, depth) = (scip.nodes[node].active, scip.nodes[node].depth as i32);
    if active && depth == treer!(scip).repropdepth {
        loop {
            tree!(scip).repropdepth += 1;
            let rd = treer!(scip).repropdepth;
            if !(rd < treer!(scip).pathlen && !scip.nodes[treer!(scip).path[rd]].reprop) {
                break;
            }
        }
        if treer!(scip).repropdepth == treer!(scip).pathlen {
            tree!(scip).repropdepth = i32::MAX;
        }
    }
}

/// `treeNextRepropsubtreecount`: moves the subtree repropagation counter
/// to the next value.
fn tree_next_repropsubtreecount(tree: &mut Tree) {
    tree.repropsubtreecount += 1;
    tree.repropsubtreecount %= (MAXREPROPMARK + 1) as i32;
}

/// `nodeRepropagate`: applies propagation on the node, that was marked to
/// be propagated again; returns whether the node was cut off.
fn node_repropagate(scip: &mut Scip, node: NodeId, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult<bool> {
    let initialreprop = scip.nodes[node].reprop;

    // Process the delayed events in order to flush the problem changes.
    event::eventqueue_process_filter(scip, eventfilter)?;

    // Stop node activation timer.
    let clockisrunning = scip.stat.nodeactivationtime.is_running();
    if clockisrunning {
        scip.stat.nodeactivationtime.stop(scip.set.time_enabled);
    }

    // Mark the node refocused and temporarily install it as focus node.
    let oldtype = scip.nodes[node].nodetype;
    let tree = &mut tree!(scip);
    let oldfocusnode = tree.focusnode;
    let oldfocuslpfork = tree.focuslpfork;
    let oldfocuslpstatefork = tree.focuslpstatefork;
    let oldfocussubroot = tree.focussubroot;
    let oldfocuslpstateforklpcount = tree.focuslpstateforklpcount;
    let oldnchildren = tree.nchildren;
    let oldnsiblings = tree.nsiblings;
    let oldfocusnodehaslp = tree.focusnodehaslp;
    tree.focusnode = node;
    tree.focuslpfork = NONE;
    tree.focuslpstatefork = NONE;
    tree.focussubroot = NONE;
    tree.focuslpstateforklpcount = -1;
    tree.nchildren = 0;
    tree.nsiblings = 0;
    tree.focusnodehaslp = false;
    scip.nodes[node].nodetype = NodeType::RefocusNode;

    // Propagate the domains again.
    let oldnboundchgs = scip.stat.nboundchgs;
    let depth = scip.nodes[node].depth as i32;
    let mut cutoff = false;
    crate::solve::propagate_domains(scip, transprob, origprob, eventfilter, depth, 0, cons::PROPTIMING_ALWAYS, &mut cutoff)?;
    scip.stat.nreprops += 1;
    scip.stat.nrepropboundchgs += scip.stat.nboundchgs - oldnboundchgs;
    if cutoff {
        scip.stat.nrepropcutoffs += 1;
    }

    // If a propagation marked with the reprop flag was successful, we want
    // to repropagate the whole subtree.
    if initialreprop && !cutoff && scip.stat.nboundchgs > oldnboundchgs {
        tree_next_repropsubtreecount(&mut tree!(scip));
        scip.nodes[node].repropsubtreemark = treer!(scip).repropsubtreecount as u32;
    }

    // Reset the node's type and reinstall the old focus node.
    scip.nodes[node].nodetype = oldtype;
    let tree = &mut tree!(scip);
    tree.focusnode = oldfocusnode;
    tree.focuslpfork = oldfocuslpfork;
    tree.focuslpstatefork = oldfocuslpstatefork;
    tree.focussubroot = oldfocussubroot;
    tree.focuslpstateforklpcount = oldfocuslpstateforklpcount;
    tree.nchildren = oldnchildren;
    tree.nsiblings = oldnsiblings;
    tree.focusnodehaslp = oldfocusnodehaslp;

    // Make the domain change data static again to save memory.
    if scip.nodes[node].nodetype != NodeType::FocusNode {
        let mut domchg = scip.nodes[node].domchg.take();
        crate::var::domchg_make_static(scip, &mut domchg)?;
        scip.nodes[node].domchg = domchg;
    }

    // Start node activation timer again.
    if clockisrunning {
        scip.stat.nodeactivationtime.start(scip.set.time_enabled);
    }

    // Delay events in path switching.
    event::eventqueue_delay(scip);

    // Mark the node to be cut off if a cutoff was detected.
    if cutoff {
        node_cutoff(scip, node, eventfilter, transprob, origprob)?;
    }
    Ok(cutoff)
}

/// `nodeActivate`: informs node, that it is now on the active path and
/// applies any domain and constraint set changes.
fn node_activate(scip: &mut Scip, node: NodeId, transprob: usize, origprob: usize, eventfilter: usize, cutoff: &mut bool) -> ScipResult {
    // Apply lower bound, variable domain, and constraint set changes.
    let parent = scip.nodes[node].parent;
    if parent != NONE {
        let parentlb = scip.nodes[parent].lowerbound;
        node_update_lowerbound(scip, node, eventfilter, transprob, origprob, parentlb)?;
    }
    let depth = scip.nodes[node].depth as i32;
    let isfocus = scip.nodes[node].nodetype == NodeType::FocusNode;
    cons::conssetchg_apply(scip, node, depth, isfocus)?;
    crate::var::domchg_apply(scip, node, depth, cutoff)?;

    // Mark node active.
    scip.nodes[node].active = true;
    scip.stat.nactivatednodes += 1;

    // Check if the domain change produced a cutoff.
    if *cutoff {
        // Try to repropagate the node to see, if the propagation also leads
        // to a conflict and a conflict constraint could be generated; if
        // propagation conflict analysis is turned off, repropagating the
        // node makes no sense, since it is already cut off.
        scip.nodes[node].reprop = scip.set.conf_enable && scip.set.conf_useprop;

        // Mark the node to be cut off.
        node_cutoff(scip, node, eventfilter, transprob, origprob)?;
    }

    // Propagate node again, if the reprop flag is set; in the new focus
    // node, no repropagation is necessary, because the focus node is
    // propagated anyways.
    let n = &scip.nodes[node];
    if n.nodetype != NodeType::FocusNode && (n.reprop || (n.parent != NONE && n.repropsubtreemark != scip.nodes[n.parent].repropsubtreemark)) {
        let propcutoff = node_repropagate(scip, node, transprob, origprob, eventfilter)?;
        *cutoff = *cutoff || propcutoff;
    }
    Ok(())
}

/// `nodeDeactivate`: informs node, that it is no longer on the active path
/// and undoes any domain and constraint set changes.
fn node_deactivate(scip: &mut Scip, node: NodeId) -> ScipResult {
    // Undo domain and constraint set changes.
    crate::var::domchg_undo(scip, node)?;
    cons::conssetchg_undo(scip, node)?;

    // Mark node inactive.
    scip.nodes[node].active = false;

    // Count number of deactivated nodes (ignoring probing switches).
    if !tree_probing(scip) {
        scip.stat.ndeactivatednodes += 1;
    }
    Ok(())
}

/// `SCIPnodeAddCons`: adds constraint locally to the node and captures
/// it; activates constraint, if node is active; if a local constraint is
/// added to the root node, it is automatically upgraded into a global
/// constraint.
pub fn node_add_cons(scip: &mut Scip, node: NodeId, cons: ConsId) -> ScipResult {
    // Add constraint addition to the node's constraint set change data, and
    // activate constraint if node is active.
    let n = &scip.nodes[node];
    let (depth, isfocus, active, isprobing) = (n.depth as i32, n.nodetype == NodeType::FocusNode, n.active, n.nodetype == NodeType::ProbingNode);
    cons::conssetchg_add_added_cons(scip, node, cons, depth, isfocus, active)?;

    // If the constraint is added to an active node which is not a probing
    // node, increment the corresponding counter.
    if active && !isprobing {
        scip.stat.nactiveconssadded += 1;
    }
    Ok(())
}

/// `SCIPnodeDelCons`: locally deletes constraint at the given node by
/// disabling its separation, enforcing, and propagation capabilities at
/// the node; captures constraint; disables constraint, if node is active.
pub fn node_del_cons(scip: &mut Scip, node: NodeId, cons: ConsId) -> ScipResult {
    // Add constraint disabling to the node's constraint set change data.
    cons::conssetchg_add_disabled_cons(scip, node, cons);

    // Disable constraint, if node is active.
    if scip.nodes[node].active && scip.conss[cons].enabled && !scip.conss[cons].updatedisable {
        cons::cons_disable(scip, cons)?;
    }
    Ok(())
}

/// `SCIPnodeGetAddedConss`: returns all constraints added to a given node.
pub fn node_get_added_conss(scip: &Scip, node: NodeId) -> Vec<ConsId> {
    match &scip.nodes[node].conssetchg {
        Some(c) => c.addedconss.head(c.naddedconss).to_vec(),
        None => Vec::new(),
    }
}

/// `SCIPnodeGetNAddedConss`: returns the number of added constraints to
/// the given node.
pub fn node_get_n_added_conss(scip: &Scip, node: NodeId) -> i32 {
    match &scip.nodes[node].conssetchg {
        None => 0,
        Some(c) => c.naddedconss,
    }
}

/// `treeAddPendingBdchg`: adds the given bound change to the list of
/// pending bound changes.
#[allow(clippy::too_many_arguments)]
fn tree_add_pending_bdchg(
    scip: &mut Scip,
    node: NodeId,
    var: VarId,
    newbound: f64,
    boundtype: BoundType,
    infercons: ConsId,
    inferprop: PropId,
    inferinfo: i32,
    probingchange: bool,
) {
    // Make sure that enough memory is allocated for the pendingbdchgs array.
    let n = treer!(scip).npendingbdchgs;
    tree_ensure_pendingbdchgs_mem(scip, n + 1);

    // Capture the variable.
    crate::var::var_capture(scip, var);

    // Add the bound change to the pending list.
    let tree = &mut tree!(scip);
    tree.pendingbdchgs[n] = PendingBdchg {
        node,
        var,
        newbound,
        boundtype,
        infercons,
        inferprop,
        inferinfo,
        probingchange,
    };
    tree.npendingbdchgs += 1;
}

/// `SCIPnodeAddBoundinfer`: adds bound change with inference information
/// to focus node, child of focus node, or probing node; if possible,
/// adjusts bound to integral value; at most one of `infercons` and
/// `inferprop` may be set.
#[allow(clippy::too_many_arguments)]
pub fn node_add_boundinfer(
    scip: &mut Scip,
    node: NodeId,
    transprob: usize,
    origprob: usize,
    eventfilter: usize,
    var: VarId,
    newbound: f64,
    boundtype: BoundType,
    infercons: ConsId,
    inferprop: PropId,
    inferinfo: i32,
    probingchange: bool,
) -> ScipResult {
    let (mut var, mut newbound, mut boundtype) = (var, newbound, boundtype);
    let useglobal = scip.nodes[node].depth as i32 <= treer!(scip).effectiverootdepth;
    let bounds = |scip: &Scip, var: VarId| {
        let v = &scip.vars[var];
        if useglobal { (v.glbdom.lb, v.glbdom.ub) } else { (v.locdom.lb, v.locdom.ub) }
    };

    // Remember variable as inference variable, and get corresponding active
    // variable, bound and bound type.
    let infervar = var;
    let inferboundtype = boundtype;

    crate::var::var_get_probvar_bound(scip, &mut var, &mut newbound, &mut boundtype)?;

    if scip.vars[var].varstatus == VarStatus::MultAggr {
        log::error!("cannot change bounds of multi-aggregated variable <{}>", scip.vars[var].name);
        return Err(Retcode::InvalidData);
    }

    // The variable may have changed, make sure we have the correct bounds.
    let (oldlb, oldub) = bounds(scip, var);

    let oldbound;
    if boundtype == BoundType::Lower {
        // Adjust lower bound w.r.t. to integrality.
        crate::var::var_adjust_lb(scip, var, &mut newbound);
        oldbound = oldlb;
        newbound = crate::def::min(newbound, oldub);

        if scip.set.stage == crate::scip::Stage::Solving && scip.set.is_infinity(newbound) {
            log::error!("cannot change lower bound of variable <{}> to infinity.", scip.vars[var].name);
            return Err(Retcode::InvalidData);
        }
    } else {
        // Adjust the new upper bound.
        crate::var::var_adjust_ub(scip, var, &mut newbound);
        oldbound = oldub;
        newbound = crate::def::max(newbound, oldlb);

        if scip.set.stage == crate::scip::Stage::Solving && scip.set.is_infinity(-newbound) {
            log::error!("cannot change upper bound of variable <{}> to minus infinity.", scip.vars[var].name);
            return Err(Retcode::InvalidData);
        }
    }

    // After switching to the active variable, the bounds might become
    // redundant; if this happens, ignore the bound change.
    if (boundtype == BoundType::Lower && !scip.set.is_gt(newbound, oldlb)) || (boundtype == BoundType::Upper && !scip.set.is_lt(newbound, oldub)) {
        return Ok(());
    }

    // If the bound change takes place at an active node but is conflicting
    // with the current local bounds, we cannot apply it immediately because
    // this would introduce inconsistencies to the bound change data
    // structures in the tree and to the bound change information data in
    // the variable; instead we have to remember the bound change as a
    // pending bound change and mark the affected nodes on the active path to
    // be infeasible.
    if scip.nodes[node].active {
        let conflictingdepth = crate::var::var_get_conflicting_bdchg_depth(scip, var, boundtype, newbound);
        if conflictingdepth >= 0 {
            // Remember the pending bound change.
            tree_add_pending_bdchg(scip, node, var, newbound, boundtype, infercons, inferprop, inferinfo, probingchange);

            // Mark the node with the conflicting bound change to be cut off.
            let cnode = treer!(scip).path[conflictingdepth];
            node_cutoff(scip, cnode, eventfilter, transprob, origprob)?;
            return Ok(());
        }
    }

    scip.stat.nboundchgs += 1;

    // If we are in probing mode we have to additionally count the bound
    // changes for the probing statistic.
    if treer!(scip).probingroot != NONE {
        scip.stat.nprobboundchgs += 1;
    }

    // If the node is the root node: change local and global bound
    // immediately.
    if scip.nodes[node].depth as i32 <= treer!(scip).effectiverootdepth {
        crate::var::var_chg_bd_global(scip, var, newbound, boundtype)?;

        if scip.set.stage == crate::scip::Stage::Solving {
            // The root should be repropagated due to the bound change.
            let root = treer!(scip).root;
            node_propagate_again(scip, root);
        }
        return Ok(());
    }

    // If the node is a child, or the bound is a temporary probing bound
    //  - the bound change is a branching decision
    //  - the child's lower bound can be updated due to the changed pseudo
    //    solution
    // otherwise:
    //  - the bound change is an inference
    if scip.nodes[node].nodetype == NodeType::Child || probingchange {
        // Compute the child's new lower bound.
        let newpseudoobjval = lp::lp_get_modified_pseudo_objval(scip, transprob, var, oldbound, newbound, boundtype);

        if scip.set.is_infinity(newpseudoobjval) {
            log::warn!(
                "Cutting off node with pseudo objective bound {} larger than numerics/infinity ({})",
                newpseudoobjval,
                scip.set.infinity()
            );
            node_cutoff(scip, node, eventfilter, transprob, origprob)?;
            return Ok(());
        }

        // Get the solution value of variable in last solved LP on the
        // active path:
        //  - if the LP was solved at the current node, the LP values of the
        //    columns are valid
        //  - if the last solved LP was the one in the current lpstatefork,
        //    the LP value in the columns are still valid
        //  - otherwise, the LP values are invalid
        let lpsolval = if tree_has_current_node_lp(scip) || (treer!(scip).focuslpstateforklpcount == scip.stat.lpcount && scip.vars[var].varstatus == VarStatus::Column) {
            crate::var::var_get_lp_sol(scip, var)
        } else {
            crate::def::INVALID
        };

        // Remember the bound change as branching decision
        // (infervar/infercons/inferprop are not important: use NULL).
        crate::var::domchg_add_boundchg(scip, node, var, newbound, boundtype, BoundchgType::Branching, lpsolval, NONE, NONE, NONE, 0, inferboundtype)?;

        // Update the child's lower bound.
        node_update_lowerbound(scip, node, eventfilter, transprob, origprob, newpseudoobjval)?;
    } else {
        // Remember the bound change as inference (lpsolval is not important:
        // use 0.0).
        let boundchgtype = if infercons != NONE { BoundchgType::ConsInfer } else { BoundchgType::PropInfer };
        crate::var::domchg_add_boundchg(
            scip,
            node,
            var,
            newbound,
            boundtype,
            boundchgtype,
            0.0,
            infervar,
            infercons,
            inferprop,
            inferinfo,
            inferboundtype,
        )?;
    }

    // If node is active, apply the bound change immediately.
    if scip.nodes[node].active {
        let mut cutoff = false;
        let depth = scip.nodes[node].depth as i32;
        let pos = scip.nodes[node].domchg.as_ref().unwrap().nboundchgs as i32 - 1;
        let mut boundchg = scip.nodes[node].domchg.as_ref().unwrap().boundchgs[pos];
        crate::var::boundchg_apply(scip, &mut boundchg, depth, pos, &mut cutoff)?;
        scip.nodes[node].domchg.as_mut().unwrap().boundchgs[pos] = boundchg;
    }
    Ok(())
}

/// `SCIPnodeAddBoundchg`: adds bound change to focus node, or child of
/// focus node, or probing node; if possible, adjusts bound to integral
/// value.
#[allow(clippy::too_many_arguments)]
pub fn node_add_boundchg(
    scip: &mut Scip,
    node: NodeId,
    transprob: usize,
    origprob: usize,
    eventfilter: usize,
    var: VarId,
    newbound: f64,
    boundtype: BoundType,
    probingchange: bool,
) -> ScipResult {
    node_add_boundinfer(scip, node, transprob, origprob, eventfilter, var, newbound, boundtype, NONE, NONE, 0, probingchange)
}

/// `treeApplyPendingBdchgs`: applies the pending bound changes.
fn tree_apply_pending_bdchgs(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult {
    if treer!(scip).cutoffdepth > treer!(scip).effectiverootdepth {
        let npendingbdchgs = treer!(scip).npendingbdchgs;
        for i in 0..npendingbdchgs {
            let p = treer!(scip).pendingbdchgs[i];
            if scip.nodes[p.node].cutoff {
                continue;
            }

            let var = p.var;

            let conflictdepth = crate::var::var_get_conflicting_bdchg_depth(scip, var, p.boundtype, p.newbound);

            // It can happen, that a pending bound change conflicts with the
            // global bounds, because when it was collected, it just
            // conflicted with the local bounds, but a conflicting global
            // bound change was applied afterwards. In this case, we can cut
            // off the node where the pending bound change should be applied.
            if conflictdepth == 0 {
                node_cutoff(scip, p.node, eventfilter, transprob, origprob)?;

                // Break here to clear all pending bound changes below.
                if treer!(scip).effectiverootdepth >= treer!(scip).cutoffdepth {
                    break;
                } else {
                    continue;
                }
            }

            // Ignore bounds that are now redundant (for example, multiple
            // entries in the pendingbdchgs for the same variable).
            if p.boundtype == BoundType::Lower {
                let lb = scip.vars[var].locdom.lb;
                if !scip.set.is_gt(p.newbound, lb) {
                    continue;
                }
            } else {
                let ub = scip.vars[var].locdom.ub;
                if !scip.set.is_lt(p.newbound, ub) {
                    continue;
                }
            }

            node_add_boundinfer(
                scip,
                p.node,
                transprob,
                origprob,
                eventfilter,
                var,
                p.newbound,
                p.boundtype,
                p.infercons,
                p.inferprop,
                p.inferinfo,
                p.probingchange,
            )?;
        }
    }

    // Clear pending bound changes.
    for i in 0..treer!(scip).npendingbdchgs {
        let mut var = treer!(scip).pendingbdchgs[i].var;

        // Release the variable.
        crate::var::var_release(scip, &mut var)?;
    }

    tree!(scip).npendingbdchgs = 0;
    Ok(())
}

/// `SCIPnodeUpdateLowerbound`: if given value is larger than the node's
/// lower bound, sets the node's lower bound to the new value. Must not be
/// used on a leaf because the node priority queue remains untouched.
pub fn node_update_lowerbound(scip: &mut Scip, node: NodeId, eventfilter: usize, transprob: usize, origprob: usize, newbound: f64) -> ScipResult {
    if scip.nodes[node].lowerbound < newbound {
        let nodetype = scip.nodes[node].nodetype;
        let n = &mut scip.nodes[node];
        n.lowerbound = newbound;
        if n.estimate < newbound {
            n.estimate = newbound;
        }
        if n.depth == 0 {
            scip.stat.rootlowerbound = newbound;
        }

        // Update real lower bound statistics during solving.
        if !scip.stat.inrestart
            && scip.set.stage >= crate::scip::Stage::InitSolve
            && (nodetype == NodeType::FocusNode || nodetype == NodeType::Child || nodetype == NodeType::Sibling)
        {
            let lowerbound = tree_get_lowerbound(scip);

            // Lower bound improved.
            if scip.stat.lastlowerbound < lowerbound {
                // Throw improvement event if not already done exactly.
                let event = event::event_create_node(event::EVENTTYPE_DUALBOUNDIMPROVED, NONE);
                event::event_process(scip, &event, eventfilter)?;

                // Update primal-dual integrals.
                if scip.set.misc_calcintegral {
                    let infinity = scip.set.infinity();
                    crate::stat::stat_update_primal_dual_integrals(scip, transprob, origprob, infinity, lowerbound);
                } else {
                    scip.stat.lastlowerbound = lowerbound;
                }
            }
        }
    }
    Ok(())
}

/// `SCIPnodeUpdateLowerboundLP`: updates lower bound of node using lower
/// bound of LP.
pub fn node_update_lowerbound_lp(scip: &mut Scip, node: NodeId, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult {
    use crate::types::LpSolStat;

    // In case of iteration or time limit, the LP value may not be a valid
    // dual bound.
    let lpsolstat = lp!(scip).lpsolstat;
    if lpsolstat == LpSolStat::IterLimit || lpsolstat == LpSolStat::TimeLimit {
        return Ok(());
    }

    let lpobjval = lp::lp_get_objval(scip, transprob);

    // Check for cutoff.
    if lpsolstat == LpSolStat::Infeasible || lpsolstat == LpSolStat::ObjLimit {
        node_cutoff(scip, node, eventfilter, transprob, origprob)?;
    } else if scip.set.is_infinity(lpobjval) {
        log::warn!(
            "Cutting off node with feasible LP relaxation but LP bound {} larger than numerics/infinity ({})",
            lpobjval,
            scip.set.infinity()
        );
        node_cutoff(scip, node, eventfilter, transprob, origprob)?;
    } else {
        node_update_lowerbound(scip, node, eventfilter, transprob, origprob, lpobjval)?;
    }
    Ok(())
}

/// `SCIPchildChgNodeselPrio`: change the node selection priority of the
/// given child.
pub fn child_chg_nodesel_prio(scip: &mut Scip, child: NodeId, priority: f64) {
    let pos = scip.nodes[child].data.child.arraypos;
    tree!(scip).childrenprio[pos] = priority;
}

/// `SCIPnodeSetEstimate`: sets the node's estimated bound to the new
/// value.
pub fn node_set_estimate(scip: &mut Scip, node: NodeId, newestimate: f64) {
    // Due to numerical reasons we need this check, see
    // https://git.zib.de/integer/scip/issues/2866
    let n = &mut scip.nodes[node];
    if n.lowerbound <= newestimate {
        n.estimate = newestimate;
    }
}

/// `SCIPnodePropagateImplics`: propagates implications of binary fixings
/// at the given node triggered by the implication graph and the clique
/// table; returns whether the node can be cut off.
pub fn node_propagate_implics(scip: &mut Scip, node: NodeId, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult<bool> {
    use crate::var::{var_get_probvar, var_is_binary};
    let status = |scip: &Scip, v: VarId| scip.vars[v].varstatus;

    let cutoff = false;

    // Propagate all fixings of binary variables performed at this node.
    let nboundchgs = scip.nodes[node].domchg.as_ref().map_or(0, |d| d.nboundchgs as i32);
    let mut i = 0;
    while i < nboundchgs && !cutoff {
        let boundchg = scip.nodes[node].domchg.as_ref().unwrap().boundchgs[i];
        i += 1;

        // Ignore redundant bound changes.
        if boundchg.redundant {
            continue;
        }

        let var = boundchg.var;
        if var_is_binary(scip, var) {
            let varfixing = boundchg.boundtype == BoundType::Lower;
            let fix = varfixing as usize;
            let (implvars, impltypes, implbounds) = match &scip.vars[var].implics {
                Some(im) => (
                    im.vars[fix].head(im.nimpls[fix]).to_vec(),
                    im.types[fix].head(im.nimpls[fix]).to_vec(),
                    im.bounds[fix].head(im.nimpls[fix]).to_vec(),
                ),
                None => (Vec::new(), Vec::new(), Vec::new()),
            };

            // Apply implications.
            for j in 0..implvars.len() {
                // Should this be checked here (because SCIPnodeAddBoundinfer
                // fails for multi-aggregated variables) or should
                // SCIPnodeAddBoundinfer() just return for multi-aggregated
                // variables?
                if status(scip, implvars[j]) == VarStatus::MultAggr || status(scip, var_get_probvar(scip, implvars[j])) == VarStatus::MultAggr {
                    continue;
                }

                // Check for infeasibility.
                let lb = scip.vars[implvars[j]].locdom.lb;
                let ub = scip.vars[implvars[j]].locdom.ub;
                if impltypes[j] == BoundType::Lower {
                    if scip.set.is_feas_gt(implbounds[j], ub) {
                        return Ok(true);
                    }
                    if scip.set.is_feas_le(implbounds[j], lb) {
                        continue;
                    }
                } else {
                    if scip.set.is_feas_lt(implbounds[j], lb) {
                        return Ok(true);
                    }
                    if scip.set.is_feas_ge(implbounds[j], ub) {
                        continue;
                    }
                }

                // The implication might affect a fixed variable (after
                // resolving (multi-)aggregations); normally, the implication
                // should have been deleted in that case, but this is only
                // possible if the implied variable has the reverse
                // implication stored as a variable bound; due to numerics,
                // the variable bound may not be present and so the
                // implication is not deleted.
                if status(scip, var_get_probvar(scip, implvars[j])) == VarStatus::Fixed {
                    continue;
                }

                // Apply the implication.
                node_add_boundinfer(scip, node, transprob, origprob, eventfilter, implvars[j], implbounds[j], impltypes[j], NONE, NONE, 0, false)?;
            }

            // Apply cliques.
            let cliques = match &scip.vars[var].cliquelist {
                Some(cl) => cl.cliques[fix].head(cl.ncliques[fix]).to_vec(),
                None => Vec::new(),
            };
            for &clique in &cliques {
                let (vars, values) = {
                    let c = &scip.cliques[clique];
                    (c.vars.head(c.nvars).to_vec(), c.values.head(c.nvars).to_vec())
                };
                for k in 0..vars.len() {
                    if status(scip, vars[k]) == VarStatus::MultAggr || status(scip, var_get_probvar(scip, vars[k])) == VarStatus::MultAggr {
                        continue;
                    }

                    if vars[k] == var && values[k] == varfixing {
                        continue;
                    }

                    // Check for infeasibility.
                    let lb = scip.vars[vars[k]].locdom.lb;
                    let ub = scip.vars[vars[k]].locdom.ub;
                    if !values[k] {
                        if ub < 0.5 {
                            return Ok(true);
                        }
                        if lb > 0.5 {
                            continue;
                        }
                    } else {
                        if lb > 0.5 {
                            return Ok(true);
                        }
                        if ub < 0.5 {
                            continue;
                        }
                    }

                    if status(scip, var_get_probvar(scip, vars[k])) == VarStatus::Fixed {
                        continue;
                    }

                    // Apply the clique implication.
                    let boundtype = if values[k] { BoundType::Upper } else { BoundType::Lower };
                    node_add_boundinfer(
                        scip,
                        node,
                        transprob,
                        origprob,
                        eventfilter,
                        vars[k],
                        (!values[k]) as i32 as f64,
                        boundtype,
                        NONE,
                        NONE,
                        0,
                        false,
                    )?;
                }
            }
        }
    }
    Ok(cutoff)
}

/*
 * Path Switching
 */

/// `treeUpdatePathLPSize`: updates the LP sizes of the active path
/// starting at the given depth.
fn tree_update_path_lp_size(scip: &mut Scip, startdepth: i32) -> ScipResult {
    let (mut ncols, mut nrows);
    if startdepth == 0 {
        ncols = 0;
        nrows = 0;
    } else {
        ncols = treer!(scip).pathnlpcols[startdepth - 1];
        nrows = treer!(scip).pathnlprows[startdepth - 1];
    }

    let pathlen = treer!(scip).pathlen;
    for i in startdepth..pathlen {
        let node = treer!(scip).path[i];
        let n = &scip.nodes[node];
        match n.nodetype {
            NodeType::FocusNode => {}
            NodeType::ProbingNode => {
                let pn = n.data.probingnode.as_ref().unwrap();
                if i < pathlen - 1 {
                    ncols = pn.ncols;
                    nrows = pn.nrows;
                } else {
                    // For the current probing node, the initial LP size is
                    // stored in the path.
                    ncols = pn.ninitialcols;
                    nrows = pn.ninitialrows;
                }
            }
            NodeType::Sibling => {
                log::error!("sibling cannot be in the active path");
                return Err(Retcode::InvalidData);
            }
            NodeType::Child => {
                log::error!("child cannot be in the active path");
                return Err(Retcode::InvalidData);
            }
            NodeType::Leaf => {
                log::error!("leaf cannot be in the active path");
                return Err(Retcode::InvalidData);
            }
            NodeType::DeadEnd => {
                log::error!("dead-end cannot be in the active path");
                return Err(Retcode::InvalidData);
            }
            NodeType::Junction => {}
            NodeType::PseudoFork => {
                let p = n.data.pseudofork.as_ref().unwrap();
                ncols += p.naddedcols;
                nrows += p.naddedrows;
            }
            NodeType::Fork => {
                let f = n.data.fork.as_ref().unwrap();
                ncols += f.naddedcols;
                nrows += f.naddedrows;
            }
            NodeType::Subroot => {
                let s = n.data.subroot.as_ref().unwrap();
                ncols = s.ncols;
                nrows = s.nrows;
            }
            NodeType::RefocusNode => {
                log::error!("node cannot be of type REFOCUSNODE at this point");
                return Err(Retcode::InvalidData);
            }
        }
        tree!(scip).pathnlpcols[i] = ncols;
        tree!(scip).pathnlprows[i] = nrows;
    }
    Ok(())
}

/// The result of `treeFindSwitchForks`.
struct SwitchForks {
    commonfork: NodeId,
    newlpfork: NodeId,
    newlpstatefork: NodeId,
    newsubroot: NodeId,
    /// Whether the given node can be cut off and no path switching should
    /// be performed.
    cutoff: bool,
}

/// `treeFindSwitchForks`: finds the common fork node, the new LP state
/// defining fork, and the new focus subroot, if the path is switched to
/// the given node.
fn tree_find_switch_forks(scip: &mut Scip, node: NodeId) -> SwitchForks {
    let mut r = SwitchForks {
        commonfork: NONE,
        newlpfork: NONE,
        newlpstatefork: NONE,
        newsubroot: NONE,
        cutoff: false,
    };
    let ty = |scip: &Scip, n: NodeId| scip.nodes[n].nodetype;
    let is_lpfork = |t: NodeType| matches!(t, NodeType::PseudoFork | NodeType::Fork | NodeType::Subroot);
    let is_lpstatefork = |t: NodeType| matches!(t, NodeType::Fork | NodeType::Subroot);

    // If the new focus node is NULL, there is no common fork node, and the
    // new LP fork, LP state fork, and subroot are NULL.
    if node == NONE {
        tree!(scip).repropdepth = i32::MAX;
        return r;
    }

    // Check if the new node is marked to be cut off.
    if scip.nodes[node].cutoff {
        r.cutoff = true;
        return r;
    }

    // If the old focus node is NULL, there is no common fork node, and we
    // have to search the new LP fork, LP state fork and subroot.
    if treer!(scip).focusnode == NONE {
        let mut lpfork = node;
        while !is_lpfork(ty(scip, lpfork)) {
            lpfork = scip.nodes[lpfork].parent;
            if lpfork == NONE {
                return r;
            }
            if scip.nodes[lpfork].cutoff {
                r.cutoff = true;
                return r;
            }
        }
        r.newlpfork = lpfork;

        let mut lpstatefork = lpfork;
        while !is_lpstatefork(ty(scip, lpstatefork)) {
            lpstatefork = scip.nodes[lpstatefork].parent;
            if lpstatefork == NONE {
                return r;
            }
            if scip.nodes[lpstatefork].cutoff {
                r.cutoff = true;
                return r;
            }
        }
        r.newlpstatefork = lpstatefork;

        let mut subroot = lpstatefork;
        while ty(scip, subroot) != NodeType::Subroot {
            subroot = scip.nodes[subroot].parent;
            if subroot == NONE {
                return r;
            }
            if scip.nodes[subroot].cutoff {
                r.cutoff = true;
                return r;
            }
        }
        r.newsubroot = subroot;

        let mut fork = subroot;
        while scip.nodes[fork].parent != NONE {
            fork = scip.nodes[fork].parent;
            if scip.nodes[fork].cutoff {
                r.cutoff = true;
                return r;
            }
        }
        return r;
    }

    // Find the common fork node, the new LP defining fork, the new LP state
    // defining fork, and the new focus subroot.
    let mut fork = node;
    let mut lpfork = NONE;
    let mut lpstatefork = NONE;
    let mut subroot = NONE;
    while !scip.nodes[fork].active {
        fork = scip.nodes[fork].parent;
        if scip.nodes[fork].cutoff {
            r.cutoff = true;
            return r;
        }
        let t = ty(scip, fork);
        if lpfork == NONE && is_lpfork(t) {
            lpfork = fork;
        }
        if lpstatefork == NONE && is_lpstatefork(t) {
            lpstatefork = fork;
        }
        if subroot == NONE && t == NodeType::Subroot {
            subroot = fork;
        }
    }

    // If the common fork node is below the current cutoff depth, the cutoff
    // node is an ancestor of the common fork and thus an ancestor of the new
    // focus node, s.t. the new node can also be cut off.
    if scip.nodes[fork].depth as i32 > treer!(scip).cutoffdepth {
        r.cutoff = true;
        return r;
    }

    let forkdepth = scip.nodes[fork].depth;

    // If not already found, continue searching the LP defining fork; it
    // cannot be deeper than the common fork.
    if lpfork == NONE {
        let focuslpfork = treer!(scip).focuslpfork;
        if focuslpfork != NONE && scip.nodes[focuslpfork].depth > forkdepth {
            // focuslpfork is not on the same active path as the new node: we
            // have to continue searching.
            lpfork = fork;
            while lpfork != NONE && !is_lpfork(ty(scip, lpfork)) {
                lpfork = scip.nodes[lpfork].parent;
            }
        } else {
            // focuslpfork is on the same active path as the new node: old
            // and new node have the same lpfork.
            lpfork = focuslpfork;
        }
    }

    // If not already found, continue searching the LP state defining fork;
    // it cannot be deeper than the LP defining fork and the common fork.
    if lpstatefork == NONE {
        let focuslpstatefork = treer!(scip).focuslpstatefork;
        if focuslpstatefork != NONE && scip.nodes[focuslpstatefork].depth > forkdepth {
            // focuslpstatefork is not on the same active path as the new
            // node: we have to continue searching.
            if lpfork != NONE && scip.nodes[lpfork].depth < forkdepth {
                lpstatefork = lpfork;
            } else {
                lpstatefork = fork;
            }
            while lpstatefork != NONE && !is_lpstatefork(ty(scip, lpstatefork)) {
                lpstatefork = scip.nodes[lpstatefork].parent;
            }
        } else {
            // focuslpstatefork is on the same active path as the new node:
            // old and new node have the same lpstatefork.
            lpstatefork = focuslpstatefork;
        }
    }

    // If not already found, continue searching the subroot; it cannot be
    // deeper than the LP defining fork, the LP state fork and the common
    // fork.
    if subroot == NONE {
        let focussubroot = treer!(scip).focussubroot;
        if focussubroot != NONE && scip.nodes[focussubroot].depth > forkdepth {
            // focussubroot is not on the same active path as the new node:
            // we have to continue searching.
            if lpstatefork != NONE && scip.nodes[lpstatefork].depth < forkdepth {
                subroot = lpstatefork;
            } else if lpfork != NONE && scip.nodes[lpfork].depth < forkdepth {
                subroot = lpfork;
            } else {
                subroot = fork;
            }
            while subroot != NONE && ty(scip, subroot) != NodeType::Subroot {
                subroot = scip.nodes[subroot].parent;
            }
        } else {
            subroot = focussubroot;
        }
    }

    // If a node prior to the common fork should be repropagated, we select
    // the node to be repropagated as common fork in order to undo all bound
    // changes up to this node, repropagate the node, and redo the bound
    // changes afterwards.
    if forkdepth as i32 > treer!(scip).repropdepth {
        fork = treer!(scip).path[treer!(scip).repropdepth];
    }

    r.commonfork = fork;
    r.newlpfork = lpfork;
    r.newlpstatefork = lpstatefork;
    r.newsubroot = subroot;

    tree!(scip).repropdepth = i32::MAX;
    r
}

/// `treeSwitchPath`: switches the active path to the new focus node, frees
/// dead end, applies domain and constraint set changes.
#[allow(clippy::too_many_arguments)]
fn tree_switch_path(scip: &mut Scip, fork: NodeId, focusnode: NodeId, transprob: usize, origprob: usize, eventfilter: usize, cutoff: &mut bool) -> ScipResult {
    // Set new focus node.
    let oldfocusnode = treer!(scip).focusnode;
    tree!(scip).focusnode = focusnode;

    // Get the nodes' depths.
    let focusnodedepth = if focusnode != NONE { scip.nodes[focusnode].depth as i32 } else { -1 };
    let forklen = if fork != NONE { scip.nodes[fork].depth as i32 + 1 } else { 0 };

    // Delay events in node deactivations to fork and node activations to
    // parent of new focus node.
    event::eventqueue_delay(scip);

    // Undo the domain and constraint set changes of the old active path by
    // deactivating the path's nodes.
    while treer!(scip).pathlen > forklen {
        let n = treer!(scip).path[treer!(scip).pathlen - 1];
        node_deactivate(scip, n)?;
        tree!(scip).pathlen -= 1;
    }

    // Apply the pending bound changes.
    tree_apply_pending_bdchgs(scip, transprob, origprob, eventfilter)?;

    // Create the new active path.
    tree_ensure_path_mem(scip, focusnodedepth + 1);
    let mut focusnode = focusnode;
    while focusnode != fork {
        let depth = scip.nodes[focusnode].depth as i32;
        tree!(scip).path[depth] = focusnode;
        focusnode = scip.nodes[focusnode].parent;
    }

    // If the old focus node is a dead end (has no children), delete it.
    if oldfocusnode != NONE {
        let o = &scip.nodes[oldfocusnode];
        let free_node = match o.nodetype {
            NodeType::FocusNode | NodeType::Sibling | NodeType::Child | NodeType::Leaf | NodeType::RefocusNode => false,
            NodeType::DeadEnd => true,
            NodeType::Junction => o.data.junction.nchildren == 0,
            NodeType::PseudoFork => o.data.pseudofork.as_ref().unwrap().nchildren == 0,
            NodeType::Fork => o.data.fork.as_ref().unwrap().nchildren == 0,
            NodeType::Subroot => o.data.subroot.as_ref().unwrap().nchildren == 0,
            NodeType::ProbingNode => {
                log::error!("probing node could not be the focus node");
                return Err(Retcode::InvalidData);
            }
        };

        if free_node {
            node_free(scip, oldfocusnode, eventfilter)?;
        }
    }

    // Apply effective root shift up to the new focus node.
    *cutoff = false;
    tree!(scip).cutoffdepth = i32::MAX;
    let neweffectiverootdepth = tree!(scip).updatedeffectiverootdepth.min(focusnodedepth);

    // Promote the constraint set and bound changes up to the new effective
    // root to be global changes.
    if treer!(scip).effectiverootdepth < neweffectiverootdepth {
        // At first globalize constraint changes to update constraint
        // handlers before changing bounds.
        for i in treer!(scip).effectiverootdepth + 1..=neweffectiverootdepth {
            let n = treer!(scip).path[i];
            cons::conssetchg_make_global(scip, n, transprob)?;
        }

        // At last globalize bound changes triggering delayed events
        // processed after the path switch.
        let mut i = treer!(scip).effectiverootdepth + 1;
        while i <= neweffectiverootdepth && !*cutoff {
            let n = treer!(scip).path[i];
            crate::var::domchg_apply_global(scip, n, cutoff)?;
            i += 1;
        }

        // Update applied effective root depth.
        tree!(scip).effectiverootdepth = neweffectiverootdepth;
    }

    // Fork might be cut off when applying the pending bound changes.
    if fork != NONE && scip.nodes[fork].cutoff {
        *cutoff = true;
    } else if fork != NONE && scip.nodes[fork].reprop && !*cutoff {
        // Propagate common fork again, if the reprop flag is set.
        *cutoff = node_repropagate(scip, fork, transprob, origprob, eventfilter)?;
    }

    // Apply domain and constraint set changes of the new path by activating
    // the path's nodes; on the way, domain propagation might be applied
    // again to the path's nodes, which can result in the cutoff of the node
    // (and its subtree). We only activate all nodes down to the parent of
    // the new focus node, because the events in this process are delayed,
    // which means that multiple changes of a bound of a variable are merged
    // (and might even be cancelled out, if the bound is first relaxed when
    // deactivating a node on the old path and then tightened to the same
    // value when activating a node on the new path). This is valid for all
    // nodes down to the parent of the new focus node, since they have
    // already been propagated. Bound change events on the new focus node,
    // however, must not be cancelled out, since they need to be propagated
    // and thus, the event must be thrown and catched by the constraint
    // handlers to mark constraints for propagation.
    while treer!(scip).pathlen < focusnodedepth && !*cutoff {
        // Activate the node, and apply domain propagation if the reprop flag
        // is set.
        tree!(scip).pathlen += 1;
        let n = treer!(scip).path[treer!(scip).pathlen - 1];
        node_activate(scip, n, transprob, origprob, eventfilter, cutoff)?;
    }

    // Process the delayed events.
    event::eventqueue_process_filter(scip, eventfilter)?;

    // Activate the new focus node; there is no need to delay these events.
    if treer!(scip).pathlen == focusnodedepth && !*cutoff {
        // Activate the node, and apply domain propagation if the reprop flag
        // is set.
        tree!(scip).pathlen += 1;
        let n = treer!(scip).path[treer!(scip).pathlen - 1];
        node_activate(scip, n, transprob, origprob, eventfilter, cutoff)?;
    }

    // Count the new LP sizes of the path.
    tree_update_path_lp_size(scip, forklen)?;
    Ok(())
}

/// `subrootConstructLP`: loads the subroot's LP data.
fn subroot_construct_lp(scip: &mut Scip, subroot: NodeId) -> ScipResult {
    let s = scip.nodes[subroot].data.subroot.as_ref().unwrap();
    let cols = s.cols.head(s.ncols).to_vec();
    let rows = s.rows.head(s.nrows).to_vec();
    let depth = scip.nodes[subroot].depth as i32;
    for c in cols {
        lp::lp_add_col(scip, c, depth)?;
    }
    for r in rows {
        lp::lp_add_row(scip, r, depth)?;
    }
    Ok(())
}

/// `forkAddLP`: loads the fork's additional LP data.
fn fork_add_lp(scip: &mut Scip, fork: NodeId) -> ScipResult {
    let f = scip.nodes[fork].data.fork.as_ref().unwrap();
    let cols = f.addedcols.head(f.naddedcols).to_vec();
    let rows = f.addedrows.head(f.naddedrows).to_vec();
    let depth = scip.nodes[fork].depth as i32;
    for c in cols {
        lp::lp_add_col(scip, c, depth)?;
    }
    for r in rows {
        lp::lp_add_row(scip, r, depth)?;
    }
    Ok(())
}

/// `pseudoforkAddLP`: loads the pseudofork's additional LP data.
fn pseudofork_add_lp(scip: &mut Scip, pseudofork: NodeId) -> ScipResult {
    let p = scip.nodes[pseudofork].data.pseudofork.as_ref().unwrap();
    let cols = p.addedcols.head(p.naddedcols).to_vec();
    let rows = p.addedrows.head(p.naddedrows).to_vec();
    let depth = scip.nodes[pseudofork].depth as i32;
    for c in cols {
        lp::lp_add_col(scip, c, depth)?;
    }
    for r in rows {
        lp::lp_add_row(scip, r, depth)?;
    }
    Ok(())
}

/// `SCIPtreeLoadLP`: constructs the LP relaxation of the focus node;
/// returns whether the root LP relaxation has to be initialized.
pub fn tree_load_lp(scip: &mut Scip) -> ScipResult<bool> {
    let lpfork = treer!(scip).focuslpfork;

    // Find out the lpfork's depth (or -1, if lpfork is NULL).
    let lpforkdepth = if lpfork == NONE { -1 } else { scip.nodes[lpfork].depth as i32 };

    // Find out, if we are in the same subtree.
    let correctlpdepth = treer!(scip).correctlpdepth;
    if correctlpdepth >= 0 {
        // Same subtree: shrink LP to the deepest node with correct LP.
        let (ncols, nrows) = (treer!(scip).pathnlpcols[correctlpdepth], treer!(scip).pathnlprows[correctlpdepth]);
        lp::lp_shrink_cols(scip, ncols)?;
        lp::lp_shrink_rows(scip, nrows)?;
    } else {
        // Other subtree: fill LP with the subroot LP data.
        lp::lp_clear(scip)?;
        let focussubroot = treer!(scip).focussubroot;
        if focussubroot != NONE {
            subroot_construct_lp(scip, focussubroot)?;
            tree!(scip).correctlpdepth = scip.nodes[focussubroot].depth as i32;
        }
    }

    // Add the missing columns and rows.
    for d in treer!(scip).correctlpdepth + 1..=lpforkdepth {
        let pathnode = treer!(scip).path[d];
        match scip.nodes[pathnode].nodetype {
            NodeType::Fork => fork_add_lp(scip, pathnode)?,
            NodeType::PseudoFork => pseudofork_add_lp(scip, pathnode)?,
            _ => {}
        }
    }
    let tree = &mut tree!(scip);
    tree.correctlpdepth = tree.correctlpdepth.max(lpforkdepth);

    // Mark the LP's size, such that we know which rows and columns were
    // added in the new node.
    lp::lp_mark_size(scip);

    // If the correct LP depth is still -1, the root LP relaxation has to be
    // initialized.
    let initroot = treer!(scip).correctlpdepth == -1;

    // Mark the LP of the focus node constructed.
    tree!(scip).focuslpconstructed = true;
    Ok(initroot)
}

/// `SCIPtreeLoadLPState`: loads LP state for fork/subroot of the focus
/// node.
pub fn tree_load_lp_state(scip: &mut Scip, prob: usize) -> ScipResult {
    let lpstatefork = treer!(scip).focuslpstatefork;

    // If there is no LP state defining fork, nothing can be done.
    if lpstatefork == NONE {
        return Ok(());
    }

    // Get the lpstatefork's depth.
    let lpstateforkdepth = scip.nodes[lpstatefork].depth as i32;

    // Load LP state.
    let (updatefeas, checkbdchgs);
    if treer!(scip).focuslpstateforklpcount != scip.stat.lpcount {
        // The LP state is moved out of the node for the call and put back:
        // lp_set_state reads it only.
        if scip.nodes[lpstatefork].nodetype == NodeType::Fork {
            let mut f = scip.nodes[lpstatefork].data.fork.take().unwrap();
            let r = lp::lp_set_state(scip, prob, f.lpistate.as_deref(), f.lpwasprimfeas, f.lpwasprimchecked, f.lpwasdualfeas, f.lpwasdualchecked);
            scip.nodes[lpstatefork].data.fork = Some(std::mem::take(&mut f));
            r?;
        } else {
            let mut s = scip.nodes[lpstatefork].data.subroot.take().unwrap();
            let r = lp::lp_set_state(scip, prob, s.lpistate.as_deref(), s.lpwasprimfeas, s.lpwasprimchecked, s.lpwasdualfeas, s.lpwasdualchecked);
            scip.nodes[lpstatefork].data.subroot = Some(std::mem::take(&mut s));
            r?;
        }
        updatefeas = !lp!(scip).solved || !lp!(scip).solisbasic;
        checkbdchgs = true;
    } else {
        updatefeas = true;

        // We do not need to check the bounds, since primalfeasible is
        // updated anyway when flushing the LP.
        checkbdchgs = false;
    }

    if updatefeas {
        // Check whether the size of the LP increased (destroying
        // primal/dual feasibility).
        let tree = scip.tree.as_ref().unwrap();
        let samerows = tree.pathnlprows[tree.correctlpdepth] == tree.pathnlprows[lpstateforkdepth];
        let samecols = tree.pathnlpcols[tree.correctlpdepth] == tree.pathnlpcols[lpstateforkdepth];
        let lp = &mut lp!(scip);
        lp.primalfeasible = lp.primalfeasible && samerows;
        lp.primalchecked = lp.primalchecked && samerows;
        lp.dualfeasible = lp.dualfeasible && samecols;
        lp.dualchecked = lp.dualchecked && samecols;

        // Check the path from LP fork to focus node for domain changes
        // (destroying primal feasibility of LP basis).
        if checkbdchgs {
            let focusdepth = scip.nodes[treer!(scip).focusnode].depth as i32;
            let mut d = lpstateforkdepth;
            while d < focusdepth && lp!(scip).primalfeasible {
                let pathnode = treer!(scip).path[d];
                let feasible = scip.nodes[pathnode].domchg.as_ref().is_none_or(|dc| dc.nboundchgs == 0);
                lp!(scip).primalfeasible = feasible;
                lp!(scip).primalchecked = feasible;
                d += 1;
            }
        }
    }
    Ok(())
}

/*
 * Node Conversion
 */

/// `nodeToLeaf`: converts node into LEAF and moves it into the array of
/// the node queue; if node's lower bound is greater or equal than the
/// given upper bound, the node is deleted; otherwise, it is moved to the
/// node queue; anyways, the caller's pointer to it is NULL after the call.
fn node_to_leaf(scip: &mut Scip, node: NodeId, eventfilter: usize, lpstatefork: NodeId, cutoffbound: f64) -> ScipResult {
    // If node is good enough to keep, put it on the node queue.
    let lowerbound = scip.nodes[node].lowerbound;
    if !scip.set.is_infinity(lowerbound) && scip.set.is_lt(lowerbound, cutoffbound) {
        // Convert node into leaf.
        scip.nodes[node].nodetype = NodeType::Leaf;
        scip.nodes[node].data.leaf.lpstatefork = lpstatefork;

        // Insert leaf in node queue.
        crate::nodesel::nodepq_insert(scip, node)?;

        // Make the domain change data static to save memory.
        let mut domchg = scip.nodes[node].domchg.take();
        crate::var::domchg_make_static(scip, &mut domchg)?;
        scip.nodes[node].domchg = domchg;
    } else {
        // Delete node due to bound cut off.
        node_cutoff(scip, node, eventfilter, crate::prob::TRANSPROB, crate::prob::ORIGPROB)?;
        if scip.nodes[node].nodetype == NodeType::Child && lpstatefork != NONE {
            node_release_lpistate(scip, lpstatefork)?;
        }
        node_free(scip, node, eventfilter)?;
    }
    Ok(())
}

/// `focusnodeCleanupVars`: removes variables from the problem, that are
/// marked to be deletable, and were created at the focusnode; only removes
/// variables that were created at the focusnode, unless `inlp` is TRUE
/// (e.g., when the node is cut off, anyway).
fn focusnode_cleanup_vars(scip: &mut Scip, _inlp: bool) -> ScipResult {
    // Check the settings, whether variables should be deleted.
    let needdel = if treer!(scip).focusnode == treer!(scip).root {
        scip.set.price_delvarsroot
    } else {
        scip.set.price_delvars
    };

    if !needdel {
        return Ok(());
    }

    unimplemented!("deleting priced variables");
}

/// `SCIP_VERBLEVEL_FULL`.
const VERBLEVEL_FULL: i32 = 5;

/// `focusnodeToDeadend`: converts the focus node into a dead-end node.
fn focusnode_to_deadend(scip: &mut Scip, eventfilter: usize, transprob: usize, origprob: usize) -> ScipResult {
    let focusnode = treer!(scip).focusnode;

    // Delete focus node due to cut off.
    node_cutoff(scip, focusnode, eventfilter, transprob, origprob)?;

    // Remove variables from the problem that are marked as deletable and
    // were created at this node.
    focusnode_cleanup_vars(scip, true)?;

    scip.nodes[focusnode].nodetype = NodeType::DeadEnd;

    // Release LPI state.
    let focuslpstatefork = treer!(scip).focuslpstatefork;
    if focuslpstatefork != NONE {
        node_release_lpistate(scip, focuslpstatefork)?;
    }
    Ok(())
}

/// `focusnodeToLeaf`: converts the focus node into a leaf node (if it was
/// postponed).
fn focusnode_to_leaf(scip: &mut Scip, eventfilter: usize, lpstatefork: NodeId, cutoffbound: f64) -> ScipResult {
    let focusnode = treer!(scip).focusnode;
    node_to_leaf(scip, focusnode, eventfilter, lpstatefork, cutoffbound)?;
    tree!(scip).focusnode = NONE;
    Ok(())
}

/// `focusnodeToJunction`: converts the focus node into a junction node.
fn focusnode_to_junction(scip: &mut Scip) -> ScipResult {
    let focusnode = treer!(scip).focusnode;

    // Convert node into junction.
    scip.nodes[focusnode].nodetype = NodeType::Junction;

    junction_init(scip, focusnode)?;

    // Release LPI state.
    let focuslpstatefork = treer!(scip).focuslpstatefork;
    if focuslpstatefork != NONE {
        node_release_lpistate(scip, focuslpstatefork)?;
    }

    // Make the domain change data static to save memory.
    let mut domchg = scip.nodes[focusnode].domchg.take();
    crate::var::domchg_make_static(scip, &mut domchg)?;
    scip.nodes[focusnode].domchg = domchg;
    Ok(())
}

/// `focusnodeToPseudofork`: converts the focus node into a pseudofork
/// node.
fn focusnode_to_pseudofork(scip: &mut Scip) -> ScipResult {
    let focusnode = treer!(scip).focusnode;

    // Remove variables from the problem that are marked as deletable and
    // were created at this node.
    focusnode_cleanup_vars(scip, false)?;

    // Create pseudofork data.
    let pseudofork = pseudofork_create(scip)?;

    scip.nodes[focusnode].nodetype = NodeType::PseudoFork;
    scip.nodes[focusnode].data.pseudofork = Some(pseudofork);

    // Release LPI state.
    let focuslpstatefork = treer!(scip).focuslpstatefork;
    if focuslpstatefork != NONE {
        node_release_lpistate(scip, focuslpstatefork)?;
    }

    // Make the domain change data static to save memory.
    let mut domchg = scip.nodes[focusnode].domchg.take();
    crate::var::domchg_make_static(scip, &mut domchg)?;
    scip.nodes[focusnode].domchg = domchg;
    Ok(())
}

/// `focusnodeToFork`: converts the focus node into a fork node.
fn focusnode_to_fork(scip: &mut Scip, transprob: usize) -> ScipResult {
    use crate::types::LpSolStat;
    let focusnode = treer!(scip).focusnode;

    // Usually, the LP should be solved to optimality; otherwise, numerical
    // troubles occured, and we have to forget about the LP and transform
    // the node into a junction (see below).
    let mut lperror = false;
    if !lp!(scip).resolvelperror && lp::lp_get_solstat(scip) == LpSolStat::Optimal {
        // Clean up newly created part of LP to keep only necessary columns
        // and rows.
        let root = scip.nodes[focusnode].depth == 0;
        lp::lp_cleanup_new(scip, root)?;

        // Resolve LP after cleaning up.
        lperror = lp::lp_solve_and_eval(scip, transprob, -1, false, false, true, false)?;
    }

    // There are two reasons, that the (reduced) LP is not solved to
    // optimality:
    //  - The primal heuristics (called after the current node's LP was
    //    solved) found a new solution, that is better than the current
    //    node's lower bound. (But in this case, all children should be cut
    //    off and the node should be converted into a dead-end instead of a
    //    fork.)
    //  - Something numerically weird happened after cleaning up or after
    //    resolving a diving or probing LP.
    // The only thing we can do, is to completely forget about the LP and
    // treat the node as if it was only a pseudo-solution node. Therefore we
    // have to remove all additional columns and rows from the LP and
    // convert the node into a junction. However, the node's lower bound is
    // kept, thus automatically throwing away nodes that were cut off due to
    // a primal solution.
    if lperror || lp!(scip).resolvelperror || lp::lp_get_solstat(scip) != LpSolStat::Optimal {
        if scip.set.disp_verblevel >= VERBLEVEL_FULL {
            log::info!(
                "(node {}) numerical troubles: LP {} not optimal -- convert node into junction instead of fork",
                scip.stat.nnodes,
                scip.stat.nlps
            );
        }

        // Remove all additions to the LP at this node.
        let ncols = lp::lp_get_n_cols(scip) - lp::lp_get_n_newcols(scip);
        lp::lp_shrink_cols(scip, ncols)?;
        let nrows = lp::lp_get_n_rows(scip) - lp::lp_get_n_newrows(scip);
        lp::lp_shrink_rows(scip, nrows)?;

        // Convert node into a junction.
        focusnode_to_junction(scip)?;

        return Ok(());
    }

    // Remove variables from the problem that are marked as deletable, were
    // created at this node and are not contained in the LP.
    focusnode_cleanup_vars(scip, false)?;

    // Create fork data.
    let mut fork = fork_create(scip, transprob)?;

    // Capture the LPI state of the root node to ensure that the LPI state of
    // the root stays for the whole solving process.
    if focusnode == treer!(scip).root {
        fork_capture_lpistate(&mut fork, 1);
    }

    scip.nodes[focusnode].nodetype = NodeType::Fork;
    scip.nodes[focusnode].data.fork = Some(fork);

    // Release LPI state.
    let focuslpstatefork = treer!(scip).focuslpstatefork;
    if focuslpstatefork != NONE {
        node_release_lpistate(scip, focuslpstatefork)?;
    }

    // Make the domain change data static to save memory.
    let mut domchg = scip.nodes[focusnode].domchg.take();
    crate::var::domchg_make_static(scip, &mut domchg)?;
    scip.nodes[focusnode].domchg = domchg;
    Ok(())
}

/// Which node array `treeNodesToQueue` empties.
#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeArray {
    Children,
    Siblings,
}

/// `treeNodesToQueue`: puts all nodes in the array on the node queue and
/// makes them LEAFs.
fn tree_nodes_to_queue(scip: &mut Scip, which: NodeArray, eventfilter: usize, lpstatefork: NodeId, cutoffbound: f64) -> ScipResult {
    let count = |scip: &mut Scip| match which {
        NodeArray::Children => treer!(scip).nchildren,
        NodeArray::Siblings => treer!(scip).nsiblings,
    };
    fn slot(scip: &mut Scip, which: NodeArray, i: i32) -> &mut NodeId {
        match which {
            NodeArray::Children => &mut tree!(scip).children[i],
            NodeArray::Siblings => &mut tree!(scip).siblings[i],
        }
    }

    // As long as the node array has slots.
    loop {
        let n = count(scip);
        if n < 1 {
            break;
        }
        // Convert last node to LEAF and put it into leaves queue, or delete
        // it if its lower bound exceeds the cutoff bound.
        let node = *slot(scip, which, n - 1);
        if node != NONE {
            node_to_leaf(scip, node, eventfilter, lpstatefork, cutoffbound)?;
            *slot(scip, which, n - 1) = NONE;
        } else {
            match which {
                NodeArray::Children => tree!(scip).nchildren -= 1,
                NodeArray::Siblings => tree!(scip).nsiblings -= 1,
            }
        }
    }
    Ok(())
}

/// `treeChildrenToSiblings`: converts children into siblings, clears
/// children array.
fn tree_children_to_siblings(scip: &mut Scip) {
    let tree = &mut tree!(scip);
    std::mem::swap(&mut tree.siblings, &mut tree.children);
    std::mem::swap(&mut tree.siblingsprio, &mut tree.childrenprio);
    std::mem::swap(&mut tree.siblingssize, &mut tree.childrensize);
    tree.nsiblings = tree.nchildren;
    tree.nchildren = 0;

    for i in 0..tree.nsiblings {
        let s = tree.siblings[i];
        // Because CHILD and SIBLING structs contain the same data in the
        // same order, we do not have to copy it.
        scip.nodes[s].data.sibling.arraypos = scip.nodes[s].data.child.arraypos;
        scip.nodes[s].nodetype = NodeType::Sibling;
    }
}

/// `SCIPnodeFocus`: installs a child, a sibling, or a leaf node as the new
/// focus node; `node` is `NONE` to remove focus, and is set to `NONE` if
/// it was freed because it lies in a cut off subtree.
#[allow(clippy::too_many_arguments)]
pub fn node_focus(scip: &mut Scip, node: &mut NodeId, transprob: usize, origprob: usize, eventfilter: usize, cutoff: &mut bool, postponed: bool, exitsolve: bool) -> ScipResult {
    let primal = crate::scip::TRANSPRIMAL;

    // Find the common fork node, the new LP defining fork, and the new focus
    // subroot, thereby checking, if the new node can be cut off.
    let forks = tree_find_switch_forks(scip, *node);
    let fork = forks.commonfork;
    let mut lpfork = forks.newlpfork;
    let mut lpstatefork = forks.newlpstatefork;
    let subroot = forks.newsubroot;
    *cutoff = forks.cutoff;

    // Free the new node, if it is located in a cut off subtree.
    if *cutoff {
        // Cut off node.
        if scip.nodes[*node].nodetype == NodeType::Leaf {
            // Check if the node should be stored for reoptimization.
            if scip.set.reopt_enable {
                unimplemented!("reoptimization");
            }

            // Remove node from the queue.
            crate::nodesel::nodepq_remove(scip, *node)?;

            let infinity = scip.set.infinity();
            let n = &mut scip.nodes[*node];
            n.cutoff = true;
            n.lowerbound = infinity;
            n.estimate = infinity;

            if n.depth == 0 {
                scip.stat.rootlowerbound = infinity;
            }

            let lowerbound = tree_get_lowerbound(scip);

            // Lower bound improved.
            if scip.stat.lastlowerbound < lowerbound {
                // Throw improvement event if not already done exactly.
                if scip.stat.lastlowerbound < scip.primals[primal].upperbound {
                    let event = event::event_create_node(event::EVENTTYPE_DUALBOUNDIMPROVED, NONE);
                    event::event_process(scip, &event, eventfilter)?;
                }

                // Update primal-dual integrals.
                if scip.set.misc_calcintegral {
                    crate::stat::stat_update_primal_dual_integrals(scip, transprob, origprob, infinity, lowerbound);
                } else {
                    scip.stat.lastlowerbound = lowerbound;
                }
            }
        } else {
            node_cutoff(scip, *node, eventfilter, transprob, origprob)?;
        }

        // Free node memory.
        node_free(scip, *node, eventfilter)?;
        *node = NONE;

        return Ok(());
    }

    // Remember the depth of the common fork node for LP updates.
    if subroot == treer!(scip).focussubroot && fork != NONE && lpfork != NONE {
        // We are in the same subtree with valid LP fork: the LP is correct
        // at most upto the common fork depth.
        let forkdepth = scip.nodes[fork].depth as i32;
        let tree = &mut tree!(scip);
        tree.correctlpdepth = tree.correctlpdepth.min(forkdepth);
    } else {
        // We are in a different subtree, or no valid LP fork exists: the LP
        // is completely incorrect.
        tree!(scip).correctlpdepth = -1;
    }

    // If the LP state fork changed, the lpcount information for the new LP
    // state fork is unknown.
    if lpstatefork != treer!(scip).focuslpstatefork {
        tree!(scip).focuslpstateforklpcount = -1;
    }

    // In exitsolve we only need to take care of open children.
    //
    // Because we might do a 'newstart' and converted cuts to constraints
    // might have rendered the LP in the current focusnode unsolved the
    // latter code would have resolved the LP unnecessarily.
    if exitsolve && treer!(scip).nchildren > 0 {
        let inf = scip.set.infinity();
        tree_nodes_to_queue(scip, NodeArray::Children, eventfilter, NONE, -inf)?;
    }

    // If the old focus node was cut off, we can delete its children; if the
    // old focus node's parent was cut off, we can also delete the focus
    // node's siblings.
    let focusnode = treer!(scip).focusnode;
    if focusnode != NONE && treer!(scip).cutoffdepth <= scip.nodes[focusnode].depth as i32 {
        // Delete the focus node's children by converting them to leaves with
        // a cutoffbound of -SCIPsetInfinity(set); we cannot delete them
        // directly, because in SCIPnodeFree(), the children array is
        // changed, which is the same array we would have to iterate over
        // here; the children don't have an LP fork, because the old focus
        // node is not yet converted into a fork or subroot.
        let inf = scip.set.infinity();
        tree_nodes_to_queue(scip, NodeArray::Children, eventfilter, NONE, -inf)?;

        if treer!(scip).cutoffdepth < scip.nodes[focusnode].depth as i32 {
            // Delete the focus node's siblings by converting them to leaves
            // with a cutoffbound of -SCIPsetInfinity(set); we cannot delete
            // them directly, because in SCIPnodeFree(), the siblings array is
            // changed, which is the same array we would have to iterate over
            // here; the siblings have the same LP state fork as the old focus
            // node.
            let focuslpstatefork = treer!(scip).focuslpstatefork;
            tree_nodes_to_queue(scip, NodeArray::Siblings, eventfilter, focuslpstatefork, -inf)?;
        }
    }

    // Convert the old focus node into a fork or subroot node, if it has
    // children; otherwise, convert it into a dead-end, which will be freed
    // later in treeSwitchPath(); if the node was postponed, make it a leaf.
    let mut childrenlpstatefork = treer!(scip).focuslpstatefork;

    if postponed {
        // If the node is infeasible, convert it into a dead-end; otherwise,
        // put it into the LEAF queue.
        let focusnode = treer!(scip).focusnode;
        if scip.set.is_ge(scip.nodes[focusnode].lowerbound, scip.primals[primal].cutoffbound) {
            // In case the LP was not constructed (due to the parameter
            // settings for example) we have the finally remember the old size
            // of the LP (if it was constructed in an earlier node) before we
            // change the current node into a dead-end.
            if !treer!(scip).focuslpconstructed {
                lp::lp_mark_size(scip);
            }

            // Convert old focus node into dead-end.
            focusnode_to_deadend(scip, eventfilter, transprob, origprob)?;
        } else {
            let focuslpstatefork = treer!(scip).focuslpstatefork;
            let inf = scip.set.infinity();
            focusnode_to_leaf(scip, eventfilter, focuslpstatefork, inf)?;
        }
    } else if treer!(scip).nchildren > 0 {
        // Update real lower bounds of focus and children.
        let focusnode = treer!(scip).focusnode;
        let focuslowerbound = scip.nodes[focusnode].lowerbound;
        let mut childrenlowerbound = scip.set.infinity();

        for c in 0..treer!(scip).nchildren {
            let child = treer!(scip).children[c];
            let nodelowerbound = scip.nodes[child].lowerbound;
            if childrenlowerbound > nodelowerbound {
                childrenlowerbound = nodelowerbound;
            }
            node_update_lowerbound(scip, child, eventfilter, transprob, origprob, focuslowerbound)?;
        }

        node_update_lowerbound(scip, focusnode, eventfilter, transprob, origprob, childrenlowerbound)?;

        // Check whether the next focus node is a child of the old focus
        // node.
        let selectedchild = *node != NONE && scip.nodes[*node].nodetype == NodeType::Child;

        if treer!(scip).focusnodehaslp && lp!(scip).isrelax {
            // Convert old focus node into a fork node.
            focusnode_to_fork(scip, transprob)?;

            // Check, if the conversion into a subroot or fork was successful.
            let focusnode = treer!(scip).focusnode;
            if matches!(scip.nodes[focusnode].nodetype, NodeType::Fork | NodeType::Subroot) {
                childrenlpstatefork = focusnode;

                // If a child of the old focus node was selected as new focus
                // node, the old node becomes the new focus LP fork and LP
                // state fork.
                if selectedchild {
                    lpfork = focusnode;
                    tree!(scip).correctlpdepth = scip.nodes[focusnode].depth as i32;
                    lpstatefork = focusnode;
                    tree!(scip).focuslpstateforklpcount = scip.stat.lpcount;
                }
            }

            // Update the path's LP size.
            let depth = scip.nodes[focusnode].depth as i32;
            let (ncols, nrows) = (lp::lp_get_n_cols(scip), lp::lp_get_n_rows(scip));
            tree!(scip).pathnlpcols[depth] = ncols;
            tree!(scip).pathnlprows[depth] = nrows;
        } else if treer!(scip).focuslpconstructed && (lp::lp_get_n_newcols(scip) > 0 || lp::lp_get_n_newrows(scip) > 0) {
            // Convert old focus node into pseudofork.
            focusnode_to_pseudofork(scip)?;

            // Update the path's LP size.
            let depth = scip.nodes[focusnode].depth as i32;
            let (ncols, nrows) = (lp::lp_get_n_cols(scip), lp::lp_get_n_rows(scip));
            tree!(scip).pathnlpcols[depth] = ncols;
            tree!(scip).pathnlprows[depth] = nrows;

            // If a child of the old focus node was selected as new focus
            // node, the old node becomes the new focus LP fork.
            if selectedchild {
                lpfork = focusnode;
                tree!(scip).correctlpdepth = depth;
            }
        } else {
            // In case the LP was not constructed (due to the parameter
            // settings for example) we have the finally remember the old size
            // of the LP (if it was constructed in an earlier node) before we
            // change the current node into a junction.
            lp::lp_mark_size(scip);

            // Convert old focus node into junction.
            focusnode_to_junction(scip)?;
        }
    } else if treer!(scip).focusnode != NONE {
        // In case the LP was not constructed (due to the parameter settings
        // for example) we have the finally remember the old size of the LP
        // (if it was constructed in an earlier node) before we change the
        // current node into a dead-end.
        if !treer!(scip).focuslpconstructed {
            lp::lp_mark_size(scip);
        }

        // Convert old focus node into dead-end.
        focusnode_to_deadend(scip, eventfilter, transprob, origprob)?;
    }

    // Set up the new lists of siblings and children.
    let cutoffbound = scip.primals[primal].cutoffbound;
    if *node == NONE {
        // Move siblings to the queue, make them LEAFs.
        let focuslpstatefork = treer!(scip).focuslpstatefork;
        tree_nodes_to_queue(scip, NodeArray::Siblings, eventfilter, focuslpstatefork, cutoffbound)?;

        // Move children to the queue, make them LEAFs.
        tree_nodes_to_queue(scip, NodeArray::Children, eventfilter, childrenlpstatefork, cutoffbound)?;
    } else {
        match scip.nodes[*node].nodetype {
            NodeType::Sibling => {
                // Reset plunging depth, if the selected node is better than
                // all leaves.
                let bestleaf = tree_get_best_leaf(scip);
                if bestleaf == NONE || crate::nodesel::nodepq_compare(scip, *node, bestleaf) <= 0 {
                    scip.stat.plungedepth = 0;
                }

                // Move children to the queue, make them LEAFs.
                tree_nodes_to_queue(scip, NodeArray::Children, eventfilter, childrenlpstatefork, cutoffbound)?;

                // Remove selected sibling from the siblings array.
                tree_remove_sibling(scip, *node);
            }
            NodeType::Child => {
                // Reset plunging depth, if the selected node is better than
                // all leaves; otherwise, increase plunging depth.
                let bestleaf = tree_get_best_leaf(scip);
                if bestleaf == NONE || crate::nodesel::nodepq_compare(scip, *node, bestleaf) <= 0 {
                    scip.stat.plungedepth = 0;
                } else {
                    scip.stat.plungedepth += 1;
                }

                // Move siblings to the queue, make them LEAFs.
                let focuslpstatefork = treer!(scip).focuslpstatefork;
                tree_nodes_to_queue(scip, NodeArray::Siblings, eventfilter, focuslpstatefork, cutoffbound)?;

                // Remove selected child from the children array.
                tree_remove_child(scip, *node);

                // Move remaining children to the siblings array, make them
                // SIBLINGs.
                tree_children_to_siblings(scip);
            }
            NodeType::Leaf => {
                // Move siblings to the queue, make them LEAFs.
                let focuslpstatefork = treer!(scip).focuslpstatefork;
                tree_nodes_to_queue(scip, NodeArray::Siblings, eventfilter, focuslpstatefork, cutoffbound)?;

                // Encounter an early backtrack if there is a child which does
                // not exceed given reference bound.
                if !scip.set.is_infinity(scip.stat.referencebound) {
                    // Loop over children and stop if we find a child with a
                    // lower bound below given reference bound.
                    for c in 0..treer!(scip).nchildren {
                        let child = treer!(scip).children[c];
                        if scip.set.is_lt(scip.nodes[child].lowerbound, scip.stat.referencebound) {
                            scip.stat.nearlybacktracks += 1;
                            break;
                        }
                    }
                }

                // Move children to the queue, make them LEAFs.
                tree_nodes_to_queue(scip, NodeArray::Children, eventfilter, childrenlpstatefork, cutoffbound)?;

                // Remove node from the queue.
                crate::nodesel::nodepq_remove(scip, *node)?;

                scip.stat.plungedepth = 0;
                if scip.nodes[*node].depth > 0 {
                    scip.stat.nbacktracks += 1;
                }
            }
            t => {
                log::error!("selected node is neither sibling, child, nor leaf (nodetype={})", t as i32);
                return Err(Retcode::InvalidData);
            }
        }

        // Convert node into the focus node.
        scip.nodes[*node].nodetype = NodeType::FocusNode;
    }

    // Set LP fork, LP state fork, and subroot.
    let tree = &mut tree!(scip);
    tree.focuslpfork = lpfork;
    tree.focuslpstatefork = lpstatefork;
    tree.focussubroot = subroot;
    tree.focuslpconstructed = false;
    lp!(scip).resolvelperror = false;

    // Track the path from the old focus node to the new node, free dead end,
    // set new focus node, and perform domain and constraint set changes.
    tree_switch_path(scip, fork, *node, transprob, origprob, eventfilter, cutoff)?;
    Ok(())
}

/*
 * Tree methods
 */

/// `SCIPtreeCreate`: creates an initialized tree data structure.
pub fn tree_create(scip: &mut Scip, nodesel: crate::types::NodeselId) -> ScipResult {
    let leaves = crate::nodesel::nodepq_create(scip, nodesel)?;

    // Allocate one slot for the prioritized and the unprioritized bound
    // change.
    let tree = Tree {
        root: NONE,
        leaves,
        divebdchgdirs: [CVec::with_size(1), CVec::with_size(1)],
        divebdchgvars: [crate::cvec![NONE; 1], crate::cvec![NONE; 1]],
        divebdchgvals: [CVec::with_size(1), CVec::with_size(1)],
        ndivebdchanges: [0, 0],
        divebdchgsize: [1, 1],
        path: CVec::new(),
        focusnode: NONE,
        focuslpfork: NONE,
        focuslpstatefork: NONE,
        focussubroot: NONE,
        children: CVec::new(),
        siblings: CVec::new(),
        probingroot: NONE,
        childrenprio: CVec::new(),
        siblingsprio: CVec::new(),
        pathnlpcols: CVec::new(),
        pathnlprows: CVec::new(),
        probinglpistate: None,
        focuslpistate: None,
        probinglpinorms: None,
        pendingbdchgs: CVec::new(),
        probdiverelaxsol: CVec::new(),
        probinglpobjval: 0.0,
        nprobdiverelaxsol: 0,
        pendingbdchgssize: 0,
        npendingbdchgs: 0,
        focuslpstateforklpcount: -1,
        childrensize: 0,
        nchildren: 0,
        siblingssize: 0,
        nsiblings: 0,
        pathlen: 0,
        pathsize: 0,
        effectiverootdepth: 0,
        updatedeffectiverootdepth: 0,
        lastbranchparentid: -1,
        correctlpdepth: -1,
        cutoffdepth: i32::MAX,
        repropdepth: i32::MAX,
        repropsubtreecount: 0,
        probingsumchgdobjs: 0,
        focusnodehaslp: false,
        probingnodehaslp: false,
        focuslpconstructed: false,
        cutoffdelayed: false,
        probinglpwasflushed: false,
        probinglpwassolved: false,
        probingloadlpistate: false,
        probinglpwasrelax: false,
        probingsolvedlp: false,
        forcinglpmessage: false,
        probingobjchanged: false,
        sbprobing: false,
        probinglpwasprimfeas: true,
        probinglpwasprimchecked: false,
        probinglpwasdualfeas: true,
        probinglpwasdualchecked: false,
        probinglphadsafebound: false,
        probdiverelaxstored: false,
        probdiverelaxincludeslp: false,
    };
    scip.tree = Some(Box::new(tree));
    Ok(())
}

/// `SCIPtreeFree`: frees tree data structure.
pub fn tree_free(scip: &mut Scip, eventfilter: usize) -> ScipResult {
    // Free node queue.
    crate::nodesel::nodepq_free(scip, eventfilter)?;
    scip.tree = None;
    Ok(())
}

/// `SCIPtreeClear`: clears and resets tree data structure and deletes all
/// nodes.
pub fn tree_clear(scip: &mut Scip, eventfilter: usize) -> ScipResult {
    // Clear node queue.
    crate::nodesel::nodepq_clear(scip, eventfilter)?;

    // We have to remove the captures of the variables within the pending
    // bound change data structure.
    let mut v = treer!(scip).npendingbdchgs - 1;
    while v >= 0 {
        let mut var = treer!(scip).pendingbdchgs[v].var;

        // Release the variable.
        crate::var::var_release(scip, &mut var)?;
        v -= 1;
    }

    // Mark working arrays to be empty and reset data.
    let tree = &mut tree!(scip);
    tree.focuslpstateforklpcount = -1;
    tree.nchildren = 0;
    tree.nsiblings = 0;
    tree.pathlen = 0;
    tree.effectiverootdepth = 0;
    tree.updatedeffectiverootdepth = 0;
    tree.correctlpdepth = -1;
    tree.cutoffdepth = i32::MAX;
    tree.repropdepth = i32::MAX;
    tree.repropsubtreecount = 0;
    tree.npendingbdchgs = 0;
    tree.focusnodehaslp = false;
    tree.probingnodehaslp = false;
    tree.cutoffdelayed = false;
    tree.probinglpwasflushed = false;
    tree.probinglpwassolved = false;
    tree.probingloadlpistate = false;
    tree.probinglpwasrelax = false;
    tree.probingsolvedlp = false;
    Ok(())
}

/// `SCIPtreeCreateRoot`: creates the root node of the tree and puts it
/// into the leaves queue.
pub fn tree_create_root(scip: &mut Scip, eventfilter: usize) -> ScipResult {
    // Create root node.
    let inf = scip.set.infinity();
    let root = node_create_child(scip, 0.0, -inf)?;
    tree!(scip).root = root;

    // Move root to the queue, convert it to LEAF.
    tree_nodes_to_queue(scip, NodeArray::Children, eventfilter, NONE, inf)?;
    Ok(())
}

/// `SCIPtreeCreatePresolvingRoot`: creates a temporary presolving root
/// node of the tree and installs it as focus node.
pub fn tree_create_presolving_root(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult {
    // Create temporary presolving root node.
    tree_create_root(scip, eventfilter)?;

    // Install the temporary root node as focus node.
    let mut cutoff = false;
    let mut root = treer!(scip).root;
    node_focus(scip, &mut root, transprob, origprob, eventfilter, &mut cutoff, false, false)?;
    tree!(scip).root = root;
    Ok(())
}

/// `SCIPtreeFreePresolvingRoot`: frees the temporary presolving root and
/// resets tree data structure.
pub fn tree_free_presolving_root(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult {
    // Unfocus the temporary root node.
    let mut node = NONE;
    let mut cutoff = false;
    node_focus(scip, &mut node, transprob, origprob, eventfilter, &mut cutoff, false, false)?;

    // Reset tree data structure.
    tree_clear(scip, eventfilter)?;
    Ok(())
}

/// `SCIPtreeGetNodesel`: returns the node selector associated with the
/// given node priority queue.
pub fn tree_get_nodesel(scip: &Scip) -> crate::types::NodeselId {
    scip.tree.as_ref().unwrap().leaves.nodesel
}

/// `SCIPtreeSetNodesel`: sets the node selector used for sorting the nodes
/// in the priority queue, and resorts the queue if necessary.
pub fn tree_set_nodesel(scip: &mut Scip, nodesel: crate::types::NodeselId) -> ScipResult {
    if tree_get_nodesel(scip) != nodesel {
        // Change the node selector used in the priority queue and resort the
        // queue.
        crate::nodesel::nodepq_set_nodesel(scip, nodesel)?;

        // Issue message.
        if scip.stat.nnodes > 0 && scip.set.disp_verblevel >= VERBLEVEL_FULL {
            log::info!("(node {}) switching to node selector <{}>", scip.stat.nnodes, scip.nodesels[nodesel].name);
        }
    }
    Ok(())
}

/// `SCIPtreeCutoff`: cuts off nodes with lower bound not better than given
/// cutoff bound.
pub fn tree_cutoff(scip: &mut Scip, eventfilter: usize, cutoffbound: f64) -> ScipResult {
    // If we are in diving mode, it is not allowed to cut off nodes, because
    // this can lead to deleting LP rows which would modify the currently
    // unavailable (due to diving modifications) SCIP_LP -> the cutoff must
    // be delayed and executed after the diving ends.
    if lp!(scip).diving {
        tree!(scip).cutoffdelayed = true;
        return Ok(());
    }

    tree!(scip).cutoffdelayed = false;

    // Cut off leaf nodes in the queue.
    crate::nodesel::nodepq_bound(scip, eventfilter, cutoffbound)?;

    let (transprob, origprob) = (crate::prob::TRANSPROB, crate::prob::ORIGPROB);

    // Cut off siblings: we have to loop backwards, because a removal leads
    // to moving the last node in empty slot.
    let mut i = treer!(scip).nsiblings - 1;
    while i >= 0 {
        let node = treer!(scip).siblings[i];
        let lb = scip.nodes[node].lowerbound;
        if lb >= cutoffbound || scip.set.is_ge(lb, cutoffbound) {
            // Delete sibling due to bound cut off.
            node_cutoff(scip, node, eventfilter, transprob, origprob)?;
            node_free(scip, node, eventfilter)?;
        }
        i -= 1;
    }

    // Cut off children: we have to loop backwards, because a removal leads
    // to moving the last node in empty slot.
    let mut i = treer!(scip).nchildren - 1;
    while i >= 0 {
        let node = treer!(scip).children[i];
        let lb = scip.nodes[node].lowerbound;
        if lb >= cutoffbound || scip.set.is_ge(lb, cutoffbound) {
            // Delete child due to bound cut off.
            node_cutoff(scip, node, eventfilter, transprob, origprob)?;
            node_free(scip, node, eventfilter)?;
        }
        i -= 1;
    }
    Ok(())
}

/// `SCIPtreeCalcNodeselPriority`: calculates the node selection priority
/// for moving the given variable's LP value to the given target value;
/// this node selection priority can be given to the SCIPcreateChild()
/// call. `branchdir` fixed should only be used, when both bounds changed.
pub fn tree_calc_nodesel_priority(scip: &Scip, var: VarId, branchdir: BranchDir, targetvalue: f64) -> f64 {
    use crate::var::{var_get_avg_inferences, var_get_pseudocost, var_get_root_sol, var_get_sol};

    // Extract necessary information.
    let isroot = tree_get_current_depth(scip) == 0;
    let haslp = tree_has_focus_node_lp(scip);
    let varsol = var_get_sol(scip, var, haslp);
    let varrootsol = var_get_root_sol(scip, var);
    let downinfs = var_get_avg_inferences(scip, var, BranchDir::Downwards);
    let upinfs = var_get_avg_inferences(scip, var, BranchDir::Upwards);
    let vardir = scip.vars[var].branchdirection;
    let childsel = scip.set.nodesel_childsel;
    let eps = scip.set.epsilon();

    let invalid_dir = || {
        log::error!("invalid preferred branching direction <{}> of variable <{}>", vardir as i32, scip.vars[var].name);
        0.0
    };
    let invalid_rule = || {
        log::error!("invalid child selection rule <{}>", childsel as char);
        0.0
    };

    match branchdir {
        BranchDir::Downwards => match vardir {
            BranchDir::Downwards => 1.0,
            BranchDir::Upwards => -1.0,
            BranchDir::Auto => match childsel {
                b'd' => 1.0,
                b'u' => -1.0,
                b'p' => -var_get_pseudocost(scip, var, targetvalue - varsol),
                b'i' => downinfs,
                b'l' => targetvalue - varsol,
                b'r' => varrootsol - varsol,
                b'h' => {
                    let mut prio = downinfs + eps;
                    if !isroot && haslp {
                        prio *= varrootsol - varsol + 1.0;
                    }
                    prio
                }
                _ => invalid_rule(),
            },
            _ => invalid_dir(),
        },
        BranchDir::Upwards => {
            // The branch is directed upwards.
            match vardir {
                BranchDir::Downwards => -1.0,
                BranchDir::Upwards => 1.0,
                BranchDir::Auto => {
                    let mut prio = match childsel {
                        b'd' => -1.0,
                        b'u' => 1.0,
                        b'p' => -var_get_pseudocost(scip, var, targetvalue - varsol),
                        b'i' => upinfs,
                        b'l' => varsol - targetvalue,
                        b'r' => varsol - varrootsol,
                        b'h' => {
                            let mut prio = upinfs + eps;
                            if !isroot && haslp {
                                prio *= varsol - varrootsol + 1.0;
                            }
                            prio
                        }
                        _ => invalid_rule(),
                    };
                    // Since choosing the upwards direction is usually superior
                    // than the downwards direction (see results of
                    // Achterberg's thesis (2007)), we break ties towards
                    // upwards branching.
                    prio += eps;
                    prio
                }
                _ => invalid_dir(),
            }
        }
        BranchDir::Fixed => scip.set.infinity(),
        BranchDir::Auto => {
            log::error!("invalid branching direction <{}> of variable <{}>", vardir as i32, scip.vars[var].name);
            0.0
        }
    }
}

/// `SCIPtreeCalcChildEstimate`: calculates an estimate for the objective
/// of the best feasible solution contained in the subtree after applying
/// the given branching; this estimate can be given to the
/// SCIPcreateChild() call.
pub fn tree_calc_child_estimate(scip: &Scip, var: VarId, targetvalue: f64) -> f64 {
    use crate::var::{var_get_pseudocost, var_get_sol, var_is_integral};

    let tree = scip.tree.as_ref().unwrap();
    let mut estimate = scip.nodes[tree.focusnode].estimate;
    let varsol = var_get_sol(scip, var, tree_has_focus_node_lp(scip));

    // Compute increase above parent node's (i.e., focus node's) estimate
    // value.
    let estimateinc = if !var_is_integral(scip, var) {
        var_get_pseudocost(scip, var, targetvalue - varsol)
    } else {
        // Calculate estimate based on pseudo costs:
        //  estimate = lowerbound + sum(min{f_j * pscdown_j, (1-f_j) * pscup_j})
        //           = parentestimate - min{f_b * pscdown_b, (1-f_b) * pscup_b} + (targetvalue-oldvalue)*{pscdown_b or pscup_b}
        let pscdown = var_get_pseudocost(scip, var, scip.set.feas_floor(varsol) - varsol);
        let pscup = var_get_pseudocost(scip, var, scip.set.feas_ceil(varsol) - varsol);
        var_get_pseudocost(scip, var, targetvalue - varsol) - crate::def::min(pscdown, pscup)
    };

    // Due to rounding errors estimateinc might be slightly negative; in this
    // case return the parent node's estimate.
    if estimateinc > 0.0 {
        estimate += estimateinc;
    }
    estimate
}

/// The children `SCIPtreeBranchVar` created (`NONE` where none was).
#[derive(Clone, Copy, Debug)]
pub struct BranchChildren {
    pub downchild: NodeId,
    pub eqchild: NodeId,
    pub upchild: NodeId,
}

/// `SCIPtreeBranchVar`: branches on a variable x.
///
/// If x is a continuous variable, then two child nodes will be created
/// (x <= x', x >= x'), but if the bounds of x are such that their relative
/// difference is smaller than epsilon, the variable is fixed to val (if
/// not SCIP_INVALID) or a well chosen alternative in the current node,
/// i.e., no children are created. If x is not a continuous variable, then:
/// if solution value x' is fractional, two child nodes will be created (x
/// <= floor(x'), x >= ceil(x')); if solution value is integral, the x' is
/// equal to lower or upper bound of the branching variable and the bounds
/// of x are finite, then two child nodes will be created (x <= x", x >=
/// x"+1 with x" = floor((lb + ub)/2)); otherwise (up to) three child nodes
/// will be created (x <= x'-1, x == x', x >= x'+1). If solution value is
/// equal to one of the bounds and the other bound is infinite, only two
/// child nodes will be created (the third one would be infeasible anyway).
///
/// `val` is the value to branch on or `INVALID` for branching on current
/// LP/pseudo solution. A branching value is required for branching on
/// continuous variables.
pub fn tree_branch_var(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize, var: VarId, val: f64) -> ScipResult<BranchChildren> {
    use crate::{
        def::INVALID,
        var::{var_get_probvar, var_get_probvar_sum, var_get_sol, var_get_worst_bound_local, var_is_integral},
    };

    // Initialize children pointer.
    let mut children = BranchChildren {
        downchild: NONE,
        eqchild: NONE,
        upchild: NONE,
    };
    let (mut var, mut val) = (var, val);

    // Store whether a valid value was given for branching.
    let validval = val != INVALID;

    // Get the corresponding active problem variable; if branching value is
    // given, then transform it to the value of the active variable.
    if validval {
        let mut scalar = 1.0;
        let mut constant = 0.0;
        var_get_probvar_sum(scip, &mut var, &mut scalar, &mut constant)?;
        if scalar == 0.0 {
            log::error!("cannot branch on fixed variable <{}>", scip.vars[var].name);
            return Err(Retcode::InvalidData);
        }

        // We should have givenvariable = scalar * activevariable + constant.
        val = (val - constant) / scalar;
    } else {
        var = var_get_probvar(scip, var);
    }

    if matches!(scip.vars[var].varstatus, VarStatus::Fixed | VarStatus::MultAggr) {
        log::error!("cannot branch on fixed or multi-aggregated variable <{}>", scip.vars[var].name);
        return Err(Retcode::InvalidData);
    }

    // Ensure that branching on continuous variables will only be performed
    // when a branching point is given.
    if !var_is_integral(scip, var) && !validval {
        log::error!("Cannot branch on continuous variable <{}> without a given branching value.", scip.vars[var].name);
        return Err(Retcode::InvalidData);
    }

    // Get value of variable in current LP or pseudo solution.
    let lpval = var_get_sol(scip, var, treer!(scip).focusnodehaslp);

    // If there was no explicit value given for branching, branch on current
    // LP or pseudo solution value.
    if !validval {
        val = lpval;

        // Avoid branching on infinite values in pseudo solution.
        if scip.set.is_infinity(-val) || scip.set.is_infinity(val) {
            val = var_get_worst_bound_local(scip, var);

            // If both bounds are infinite, choose zero as branching point.
            if scip.set.is_infinity(-val) || scip.set.is_infinity(val) {
                val = 0.0;
            }
        }
    }

    let mut downub = INVALID;
    let mut fixval = INVALID;
    let mut uplb = INVALID;
    let (lb, ub) = (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub);
    let eps = scip.set.epsilon();

    if !var_is_integral(scip, var) {
        if scip.set.is_rel_eq(lb, ub) {
            // If val is at least epsilon away from both bounds, then we change
            // both bounds to this value; otherwise, we fix the variable to its
            // worst bound.
            let focusnode = treer!(scip).focusnode;
            let current = tree_get_current_node(scip);
            if scip.set.is_gt(val, lb) && scip.set.is_lt(val, ub) {
                node_add_boundchg(scip, focusnode, transprob, origprob, eventfilter, var, val, BoundType::Lower, false)?;
                node_add_boundchg(scip, focusnode, transprob, origprob, eventfilter, var, val, BoundType::Upper, false)?;
            } else if scip.vars[var].obj >= 0.0 {
                node_add_boundchg(scip, current, transprob, origprob, eventfilter, var, ub, BoundType::Lower, false)?;
            } else {
                node_add_boundchg(scip, current, transprob, origprob, eventfilter, var, lb, BoundType::Upper, false)?;
            }
        } else if crate::misc::rel_diff(ub, lb) <= 2.02 * eps {
            // If the only way to branch is such that in both sides the
            // relative domain width becomes smaller epsilon, then fix the
            // variable in both branches right away; however, if one of the
            // bounds is at infinity (and thus the other bound is at most 2eps
            // away from the same infinity (in relative sense), then fix the
            // variable to the non-infinite value, as we cannot fix a variable
            // to infinity.
            let current = tree_get_current_node(scip);
            if scip.set.is_infinity(-lb) {
                node_add_boundchg(scip, current, transprob, origprob, eventfilter, var, ub, BoundType::Lower, false)?;
            } else if scip.set.is_infinity(ub) {
                node_add_boundchg(scip, current, transprob, origprob, eventfilter, var, lb, BoundType::Upper, false)?;
            } else {
                downub = lb;
                uplb = ub;
            }
        } else {
            // In the general case, there is enough space for two branches; a
            // sophisticated user should have also chosen the branching value
            // such that it is not very close to the bounds, so here we only
            // ensure that it is at least epsilon away from both bounds.
            downub = crate::def::min(val, ub - eps);
            uplb = crate::def::max(val, lb + eps);
        }
    } else if scip.set.is_feas_integral(val) {
        // If there was no explicit value given for branching, the variable
        // has a finite domain, and the current LP/pseudo solution is one of
        // the bounds, we branch in the center of the domain.
        if !validval && !scip.set.is_infinity(-lb) && !scip.set.is_infinity(ub) && (scip.set.is_feas_eq(val, lb) || scip.set.is_feas_eq(val, ub)) {
            // Create child nodes with x <= x", and x >= x"+1 with x" =
            // floor((lb + ub)/2); if x" is integral, make the interval
            // smaller in the child in which the current solution x' is still
            // feasible.
            let center = (ub + lb) / 2.0;
            if val <= center {
                downub = scip.set.feas_floor(center);
                uplb = downub + 1.0;
            } else {
                uplb = scip.set.feas_ceil(center);
                downub = uplb - 1.0;
            }
        } else {
            // Create child nodes with x <= x'-1, x = x', and x >= x'+1.
            fixval = val.round();

            // Create child node with x <= x'-1, if this would be feasible.
            if scip.set.is_feas_ge(fixval - 1.0, lb) {
                downub = fixval - 1.0;
            }

            // Create child node with x >= x'+1, if this would be feasible.
            if scip.set.is_feas_le(fixval + 1.0, ub) {
                uplb = fixval + 1.0;
            }
        }
    } else {
        // Create child nodes with x <= floor(x'), and x >= ceil(x').
        downub = scip.set.feas_floor(val);
        uplb = downub + 1.0;
    }

    // Perform the branching; set the node selection priority in a way, s.t.
    // a node is preferred whose branching goes in the same direction as the
    // deviation from the variable's root solution.
    let focusnode = treer!(scip).focusnode;
    if downub != INVALID {
        // Create child node x <= downub.
        let priority = tree_calc_nodesel_priority(scip, var, BranchDir::Downwards, downub);

        // If LP solution is cutoff in child, compute a new estimate;
        // otherwise we cannot expect a direct change in the best solution,
        // so we keep the estimate of the parent node.
        let estimate = if scip.set.is_gt(lpval, downub) {
            tree_calc_child_estimate(scip, var, downub)
        } else {
            scip.nodes[focusnode].estimate
        };
        let node = node_create_child(scip, priority, estimate)?;
        node_add_boundchg(scip, node, transprob, origprob, eventfilter, var, downub, BoundType::Upper, false)?;
        children.downchild = node;
    }

    if fixval != INVALID {
        // Create child node with x = fixval.
        let priority = tree_calc_nodesel_priority(scip, var, BranchDir::Fixed, fixval);
        let estimate = tree_calc_child_estimate(scip, var, fixval);
        let node = node_create_child(scip, priority, estimate)?;
        if !scip.set.is_feas_eq(scip.vars[var].locdom.lb, fixval) {
            node_add_boundchg(scip, node, transprob, origprob, eventfilter, var, fixval, BoundType::Lower, false)?;
        }
        if !scip.set.is_feas_eq(scip.vars[var].locdom.ub, fixval) {
            node_add_boundchg(scip, node, transprob, origprob, eventfilter, var, fixval, BoundType::Upper, false)?;
        }
        children.eqchild = node;
    }

    if uplb != INVALID {
        // Create child node with x >= uplb.
        let priority = tree_calc_nodesel_priority(scip, var, BranchDir::Upwards, uplb);
        let estimate = if scip.set.is_lt(lpval, uplb) {
            tree_calc_child_estimate(scip, var, uplb)
        } else {
            scip.nodes[focusnode].estimate
        };
        let node = node_create_child(scip, priority, estimate)?;
        node_add_boundchg(scip, node, transprob, origprob, eventfilter, var, uplb, BoundType::Lower, false)?;
        children.upchild = node;
    }
    Ok(children)
}

/// `ARRAYGROWTH`: growth of the dive bound change arrays.
const ARRAYGROWTH: i32 = 5;

/// `SCIPtreeAddDiveBoundChange`: adds a diving bound change to the tree
/// together with the information if this is a bound change for the
/// preferred direction or not.
pub fn tree_add_dive_bound_change(scip: &mut Scip, var: VarId, dir: BranchDir, value: f64, preferred: bool) {
    let tree = &mut tree!(scip);
    let idx = if preferred { 0 } else { 1 };
    let pos = tree.ndivebdchanges[idx];

    if pos == tree.divebdchgsize[idx] - 1 {
        let newsize = tree.divebdchgsize[idx] + ARRAYGROWTH;
        tree.divebdchgdirs[idx].realloc(newsize);
        tree.divebdchgvars[idx].0.resize(newsize as usize, NONE);
        tree.divebdchgvals[idx].realloc(newsize);
        tree.divebdchgsize[idx] = newsize;
    }

    tree.divebdchgvars[idx][pos] = var;
    tree.divebdchgdirs[idx][pos] = dir;
    tree.divebdchgvals[idx][pos] = value;

    tree.ndivebdchanges[idx] += 1;
}

/// `SCIPtreeGetDiveBoundChangeData`: get the dive bound change data for
/// the preferred or the alternative direction.
pub fn tree_get_dive_bound_change_data(scip: &Scip, preferred: bool) -> (&[VarId], &[BranchDir], &[f64]) {
    let tree = scip.tree.as_ref().unwrap();
    let idx = if preferred { 0 } else { 1 };
    let n = tree.ndivebdchanges[idx];
    (tree.divebdchgvars[idx].head(n), tree.divebdchgdirs[idx].head(n), tree.divebdchgvals[idx].head(n))
}

/// `SCIPtreeClearDiveBoundChanges`: clear the tree bound change data
/// structure.
pub fn tree_clear_dive_bound_changes(scip: &mut Scip) {
    for p in 0..2 {
        tree!(scip).ndivebdchanges[p] = 0;
    }
}

/// `treeCreateProbingNode`: creates a probing child node of the current
/// node, which must be the focus node, the current refocused node, or
/// another probing node; if the current node is the focus or a refocused
/// node, the created probing node is installed as probing root node.
fn tree_create_probing_node(scip: &mut Scip) -> ScipResult {
    // Get the current node.
    let currentnode = tree_get_current_node(scip);

    // Create the node data structure.
    let node = node_create(scip);

    // Mark node to be a probing node.
    scip.nodes[node].nodetype = NodeType::ProbingNode;

    // Create the probingnode data.
    let pn = probingnode_create(scip);
    scip.nodes[node].data.probingnode = Some(pn);

    // Make the current node the parent of the new probing node.
    let retcode = node_assign_parent(scip, node, currentnode, 0.0);

    // If we reached the maximal depth level we clean up the allocated memory
    // and stop.
    if retcode == Err(Retcode::MaxDepthLevel) {
        scip.nodes.remove(node);
    }
    retcode?;

    // Check, if the node is the probing root node.
    if treer!(scip).probingroot == NONE {
        tree!(scip).probingroot = node;
    } else {
        let (ncols, nrows) = (lp::lp_get_n_cols(scip), lp::lp_get_n_rows(scip));
        let pn = scip.nodes[currentnode].data.probingnode.as_mut().unwrap();
        pn.ncols = ncols;
        pn.nrows = nrows;
    }

    // Create the new active path.
    let pathlen = treer!(scip).pathlen;
    tree_ensure_path_mem(scip, pathlen + 1);
    scip.nodes[node].active = true;
    tree!(scip).path[pathlen] = node;
    tree!(scip).pathlen += 1;

    // Update the path LP size for the previous node and set the (initial)
    // path LP size for the newly created node.
    tree_update_path_lp_size(scip, treer!(scip).pathlen - 2)?;

    // Mark the LP's size.
    lp::lp_mark_size(scip);

    // The current probing node does not yet have a solved LP.
    tree!(scip).probingnodehaslp = false;
    Ok(())
}

/// `SCIPtreeStartProbing`: switches to probing mode and creates a probing
/// root.
pub fn tree_start_probing(scip: &mut Scip, transprob: usize, strongbranching: bool) -> ScipResult {
    // Store all marked constraints for propagation.
    let conshdlrs = scip.set.conshdlrs.0[..scip.set.nconshdlrs as usize].to_vec();
    cons::conshdlrs_store_propagation_status(scip, &conshdlrs);

    // Inform LP about probing mode.
    lp::lp_start_probing(scip);

    // Remember, whether the LP was flushed and solved.
    let (flushed, solved, isrelax, hasprovedbound) = {
        let lp = &lp!(scip);
        (lp.flushed, lp.solved, lp.isrelax, lp.hasprovedbound)
    };
    let tree = &mut tree!(scip);
    tree.probinglpwasflushed = flushed;
    tree.probinglpwassolved = solved;
    tree.probingloadlpistate = false;
    tree.probinglpwasrelax = isrelax;
    tree.probingsolvedlp = false;
    tree.probingobjchanged = false;
    tree.probingsumchgdobjs = 0;
    tree.sbprobing = strongbranching;
    tree.probinglphadsafebound = hasprovedbound;
    lp!(scip).isrelax = true;
    lp!(scip).divingobjchg = false;

    // Remember the LP state in order to restore the LP solution quickly
    // after probing.
    if flushed && solved {
        let state = lp::lp_get_state(scip)?;
        let norms = lp::lp_get_norms(scip)?;
        let (pf, pc, df, dc) = {
            let lp = &lp!(scip);
            (lp.primalfeasible, lp.primalchecked, lp.dualfeasible, lp.dualchecked)
        };
        let tree = &mut tree!(scip);
        tree.probinglpistate = state;
        tree.probinglpinorms = norms;
        tree.probinglpwasprimfeas = pf;
        tree.probinglpwasprimchecked = pc;
        tree.probinglpwasdualfeas = df;
        tree.probinglpwasdualchecked = dc;
    }

    // Remember the relaxation solution to reset it later.
    if crate::relax::relaxation_is_sol_valid(scip) {
        tree_store_relax_sol(scip, transprob)?;
    }

    // Create temporary probing root node.
    tree_create_probing_node(scip)
}

/// `SCIPtreeCreateProbingNode`: creates a new probing child node in the
/// probing path.
pub fn tree_create_probing_node_pub(scip: &mut Scip) -> ScipResult {
    // Create temporary probing root node.
    tree_create_probing_node(scip)
}

/// `SCIPtreeSetProbingLPState`: sets the LP state for the current probing
/// node; state and norms are stored at the node and later released by
/// SCIP.
pub fn tree_set_probing_lp_state(scip: &mut Scip, lpistate: Option<Box<LpiState>>, lpinorms: Option<Box<LpiNorms>>, primalfeas: bool, dualfeas: bool) -> ScipResult {
    // Get the current probing node.
    let node = tree_get_current_node(scip);

    // This check is necessary to avoid cppcheck warnings.
    if node == NONE {
        return Err(Retcode::InvalidData);
    }

    // Free already present LP state and pricing norms.
    let pn = scip.nodes[node].data.probingnode.as_mut().unwrap();
    pn.lpistate = lpistate;
    pn.lpinorms = lpinorms;
    pn.lpwasprimfeas = primalfeas;
    pn.lpwasdualfeas = dualfeas;

    tree!(scip).probingloadlpistate = true;
    Ok(())
}

/// An LP state found on a probing node: the state and norms, and whether
/// the LP was primal feasible, primal checked, dual feasible and dual checked.
type SavedLpState = (Option<LpiState>, Option<LpiNorms>, bool, bool, bool, bool);

/// `SCIPtreeLoadProbingLPState`: loads the LP state for the current
/// probing node.
pub fn tree_load_probing_lp_state(scip: &mut Scip, prob: usize) -> ScipResult {
    // Loading the LP state is only necessary if we backtracked.
    if !treer!(scip).probingloadlpistate {
        return Ok(());
    }

    // Get the current probing node.
    let mut node = tree_get_current_node(scip);

    // Search the last node where an LP state information was attached; the
    // state and norms are copied out (the C passes pointers it only reads).
    let mut found: Option<SavedLpState> = None;
    loop {
        let pn = scip.nodes[node].data.probingnode.as_ref().unwrap();
        if let Some(state) = &pn.lpistate {
            found = Some((
                Some((**state).clone()),
                pn.lpinorms.as_deref().cloned(),
                pn.lpwasprimfeas,
                pn.lpwasprimchecked,
                pn.lpwasdualfeas,
                pn.lpwasdualchecked,
            ));
            break;
        }
        node = scip.nodes[node].parent;
        if scip.nodes[node].nodetype != NodeType::ProbingNode {
            break;
        }
    }

    // If there was no LP information stored in the probing nodes, use the
    // one stored before probing started.
    let (lpistate, lpinorms, lpwasprimfeas, lpwasprimchecked, lpwasdualfeas, lpwasdualchecked) = found.unwrap_or_else(|| {
        let tree = scip.tree.as_ref().unwrap();
        (
            tree.probinglpistate.as_deref().cloned(),
            tree.probinglpinorms.as_deref().cloned(),
            tree.probinglpwasprimfeas,
            tree.probinglpwasprimchecked,
            tree.probinglpwasdualfeas,
            tree.probinglpwasdualchecked,
        )
    });

    // Set the LP state.
    if lpistate.is_some() {
        lp::lp_set_state(scip, prob, lpistate.as_ref(), lpwasprimfeas, lpwasprimchecked, lpwasdualfeas, lpwasdualchecked)?;
    }

    // Set the LP pricing norms.
    if lpinorms.is_some() {
        lp::lp_set_norms(scip, lpinorms.as_ref())?;
    }

    // Now we don't need to load the LP state again until the next
    // backtracking.
    tree!(scip).probingloadlpistate = false;
    Ok(())
}

/// `SCIPtreeMarkProbingNodeHasLP`: marks the probing node to have a solved
/// LP relaxation.
pub fn tree_mark_probing_node_has_lp(scip: &mut Scip) -> ScipResult {
    // Mark the probing node to have an LP.
    tree!(scip).probingnodehaslp = true;

    // Get current probing node.
    let node = tree_get_current_node(scip);

    // Update LP information in probingnode data.
    probingnode_update(scip, node)
}

/// `treeBacktrackProbing`: undoes all changes to the problem applied in
/// probing up to the given probing depth; `probingdepth` is -1 to even
/// deactivate the probing root, thus exiting probing mode.
fn tree_backtrack_probing(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize, probingdepth: i32) -> ScipResult {
    let newpathlen = scip.nodes[treer!(scip).probingroot].depth as i32 + probingdepth + 1;

    // Check if we have to do any backtracking.
    if newpathlen < treer!(scip).pathlen {
        // The correct LP size of the node to which we backtracked is stored
        // as initial LP size for its child.
        let (ncols, nrows) = {
            let pn = scip.nodes[treer!(scip).path[newpathlen]].data.probingnode.as_ref().unwrap();
            (pn.ninitialcols, pn.ninitialrows)
        };

        while treer!(scip).pathlen > newpathlen {
            let node = treer!(scip).path[treer!(scip).pathlen - 1];

            let nchgdobjs = scip.nodes[node].data.probingnode.as_ref().unwrap().nchgdobjs;
            if nchgdobjs > 0 {
                // Only do this if we don't backtrack to the root node - in
                // that case, we can just restore the unchanged objective
                // values.
                let mut i = nchgdobjs - 1;
                while i >= 0 {
                    let (v, val) = {
                        let pn = scip.nodes[node].data.probingnode.as_ref().unwrap();
                        (pn.origobjvars[i], pn.origobjvals[i])
                    };
                    crate::var::var_chg_obj(scip, v, transprob, val)?;
                    i -= 1;
                }
                tree!(scip).probingsumchgdobjs -= nchgdobjs;

                // Reset probingobjchanged flag and cutoff bound.
                if treer!(scip).probingsumchgdobjs == 0 {
                    lp::lp_unmark_diving_obj_changed(scip);
                    tree!(scip).probingobjchanged = false;
                    let cutoffbound = scip.primals[crate::scip::TRANSPRIMAL].cutoffbound;
                    lp::lp_set_cutoffbound(scip, transprob, cutoffbound)?;
                }

                // Recompute global and local pseudo objective values.
                lp::lp_recompute_local_and_global_pseudo_objval(scip, transprob);
            }

            // Undo bound changes by deactivating the probing node.
            node_deactivate(scip, node)?;

            // Free the probing node.
            node_free(scip, node, eventfilter)?;
            let tree = &mut tree!(scip);
            tree.path[tree.pathlen - 1] = NONE;
            tree.pathlen -= 1;
        }

        // Reset the path LP size to the initial size of the probing node.
        let last = treer!(scip).path[treer!(scip).pathlen - 1];
        if scip.nodes[last].nodetype == NodeType::ProbingNode {
            let (ic, ir) = {
                let pn = scip.nodes[last].data.probingnode.as_ref().unwrap();
                (pn.ninitialcols, pn.ninitialrows)
            };
            let tree = &mut tree!(scip);
            let p = tree.pathlen - 1;
            tree.pathnlpcols[p] = ic;
            tree.pathnlprows[p] = ir;
        }

        // Undo LP extensions.
        lp::lp_shrink_cols(scip, ncols)?;
        lp::lp_shrink_rows(scip, nrows)?;
        tree!(scip).probingloadlpistate = true; // LP state must be reloaded if the next LP is solved

        // Reset the LP's marked size to the initial size of the LP at the
        // node stored in the path.
        let (r, c) = {
            let tree = scip.tree.as_ref().unwrap();
            (tree.pathnlprows[tree.pathlen - 1], tree.pathnlpcols[tree.pathlen - 1])
        };
        lp::lp_set_size_mark(scip, r, c);

        // If the highest cutoff or repropagation depth is inside the deleted
        // part of the probing path, reset them to infinity.
        if treer!(scip).cutoffdepth >= treer!(scip).pathlen {
            // Apply the pending bound changes.
            tree_apply_pending_bdchgs(scip, transprob, origprob, eventfilter)?;

            // Applying the pending bound changes might have changed the
            // cutoff depth; so the highest cutoff depth might be outside of
            // the deleted part of the probing path now.
            if treer!(scip).cutoffdepth >= treer!(scip).pathlen {
                tree!(scip).cutoffdepth = i32::MAX;
            }
        }
        if treer!(scip).repropdepth >= treer!(scip).pathlen {
            tree!(scip).repropdepth = i32::MAX;
        }
    }
    Ok(())
}

/// `SCIPtreeBacktrackProbing`: undoes all changes to the problem applied
/// in probing up to the given probing depth; the changes of the probing
/// node of the given probing depth are the last ones that remain active;
/// changes that were applied before calling SCIPtreeCreateProbingNode()
/// cannot be undone.
pub fn tree_backtrack_probing_pub(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize, probingdepth: i32) -> ScipResult {
    // Undo the domain and constraint set changes and free the temporary
    // probing nodes below the given probing depth.
    tree_backtrack_probing(scip, transprob, origprob, eventfilter, probingdepth)
}

/// `SCIPtreeEndProbing`: switches back from probing to normal operation
/// mode, frees all nodes on the probing path, restores bounds of all
/// variables and restores active constraints arrays of focus node.
pub fn tree_end_probing(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult {
    use crate::types::LpSolStat;

    // Undo the domain and constraint set changes of the temporary probing
    // nodes and free the probing nodes.
    tree_backtrack_probing(scip, transprob, origprob, eventfilter, -1)?;

    // If the LP was flushed before probing starts, flush it again.
    if treer!(scip).probinglpwasflushed {
        lp::lp_flush(scip, transprob)?;

        // If the LP was solved before probing starts, solve it again to
        // restore the LP solution.
        if treer!(scip).probinglpwassolved {
            // Reset the LP state before probing started.
            if tree!(scip).probinglpistate.is_none() {
                crate::lpi::lpi_clear_state(lp!(scip).lpi.as_mut().unwrap())?;
                let lp = &mut lp!(scip);
                let empty = lp.nlpicols == 0 && lp.nlpirows == 0;
                lp.primalfeasible = empty;
                lp.primalchecked = empty;
                lp.dualfeasible = empty;
                lp.dualchecked = empty;
                lp.solisbasic = false;
            } else {
                let state = tree!(scip).probinglpistate.take();
                let tree = scip.tree.as_ref().unwrap();
                let (pf, pc, df, dc) = (
                    tree.probinglpwasprimfeas,
                    tree.probinglpwasprimchecked,
                    tree.probinglpwasdualfeas,
                    tree.probinglpwasdualchecked,
                );
                lp::lp_set_state(scip, transprob, state.as_deref(), pf, pc, df, dc)?;
                if let Some(norms) = tree!(scip).probinglpinorms.take() {
                    lp::lp_set_norms(scip, Some(&norms))?;
                }
            }
            let wasrelax = treer!(scip).probinglpwasrelax;
            lp::lp_set_is_relax(scip, wasrelax);

            // Resolve LP to reset solution.
            let lperror = lp::lp_solve_and_eval(scip, transprob, -1, false, false, false, false)?;

            let solstat = lp::lp_get_solstat(scip);
            if lperror {
                if scip.set.disp_verblevel >= VERBLEVEL_FULL {
                    log::info!(
                        "(node {}) unresolved numerical troubles while resolving LP {} after probing",
                        scip.stat.nnodes,
                        scip.stat.nlps
                    );
                }
                lp!(scip).resolvelperror = true;
                tree!(scip).focusnodehaslp = false;
            } else if !matches!(solstat, LpSolStat::Optimal | LpSolStat::Infeasible | LpSolStat::UnboundedRay | LpSolStat::ObjLimit) {
                if scip.set.disp_verblevel >= VERBLEVEL_FULL {
                    log::info!("LP was not resolved to a sufficient status after probing");
                }
                lp!(scip).resolvelperror = true;
                tree!(scip).focusnodehaslp = false;
            } else if treer!(scip).focuslpconstructed && lp!(scip).isrelax && crate::prob::prob_all_cols_in_lp(scip, transprob) {
                let focusnode = treer!(scip).focusnode;
                node_update_lowerbound_lp(scip, focusnode, transprob, origprob, eventfilter)?;
            }
        }
    } else {
        lp!(scip).flushed = false;
    }

    // If the LP was not solved before probing it should be marked unsolved
    // now; this can occur if a probing LP was solved in between.
    if !treer!(scip).probinglpwassolved {
        lp!(scip).solved = false;
        lp!(scip).lpsolstat = LpSolStat::NotSolved;
    }

    // If the LP was solved during probing, but had been unsolved before
    // probing started, we discard the LP state.
    if scip.set.lp_clearinitialprobinglp && treer!(scip).probingsolvedlp && !treer!(scip).probinglpwassolved {
        crate::lpi::lpi_clear_state(lp!(scip).lpi.as_mut().unwrap())?;
    }

    // If a relaxation was stored before probing, restore it now.
    if treer!(scip).probdiverelaxstored {
        tree_restore_relax_sol(scip, transprob)?;
    }

    // Reset flags.
    let tree = &mut tree!(scip);
    tree.probinglpwasflushed = false;
    tree.probinglpwassolved = false;
    tree.probingloadlpistate = false;
    tree.probinglpwasrelax = false;
    tree.probingsolvedlp = false;
    tree.sbprobing = false;

    // Inform LP about end of probing mode.
    lp::lp_end_probing(scip);

    // Reset all marked constraints for propagation.
    let conshdlrs = scip.set.conshdlrs.0[..scip.set.nconshdlrs as usize].to_vec();
    cons::conshdlrs_reset_propagation_status(scip, &conshdlrs)?;
    Ok(())
}

/// `SCIPtreeStoreRelaxSol`: stores relaxation solution before diving or
/// probing.
pub fn tree_store_relax_sol(scip: &mut Scip, transprob: usize) -> ScipResult {
    let nvars = scip.probs[transprob].nvars;

    // Check if memory still needs to be allocated or resized.
    if nvars > treer!(scip).nprobdiverelaxsol || tree!(scip).probdiverelaxsol.is_empty() {
        tree!(scip).probdiverelaxsol.realloc(nvars);
        tree!(scip).nprobdiverelaxsol = nvars;
    }

    // Iterate over all variables to save the relaxation solution.
    for v in 0..nvars {
        let var = scip.probs[transprob].vars[v];
        let val = crate::var::var_get_relax_sol(scip, var);
        tree!(scip).probdiverelaxsol[v] = val;
    }

    tree!(scip).probdiverelaxstored = true;
    tree!(scip).probdiverelaxincludeslp = crate::relax::relaxation_is_lp_included_for_sol(scip);
    Ok(())
}

/// `SCIPtreeRestoreRelaxSol`: restores relaxation solution after diving or
/// probing.
pub fn tree_restore_relax_sol(scip: &mut Scip, transprob: usize) -> ScipResult {
    let nvars = scip.probs[transprob].nvars;

    // Iterate over all variables to restore the relaxation solution.
    for v in 0..nvars {
        let var = scip.probs[transprob].vars[v];
        let val = treer!(scip).probdiverelaxsol[v];
        crate::var::var_set_relax_sol(scip, var, val, true)?;
    }

    tree!(scip).probdiverelaxstored = false;
    let includeslp = treer!(scip).probdiverelaxincludeslp;
    crate::relax::relaxation_set_sol_valid(scip, true, includeslp);
    Ok(())
}

/// `SCIPtreeGetPrioChild`: gets the best child of the focus node w.r.t.
/// the node selection priority assigned by the branching rule.
pub fn tree_get_prio_child(scip: &Scip) -> NodeId {
    let tree = scip.tree.as_ref().unwrap();
    let mut bestnode = NONE;
    let mut bestprio = crate::def::REAL_MIN;
    for i in 0..tree.nchildren {
        if tree.childrenprio[i] > bestprio {
            bestnode = tree.children[i];
            bestprio = tree.childrenprio[i];
        }
    }
    bestnode
}

/// `SCIPtreeGetPrioSibling`: gets the best sibling of the focus node
/// w.r.t. the node selection priority assigned by the branching rule.
pub fn tree_get_prio_sibling(scip: &Scip) -> NodeId {
    let tree = scip.tree.as_ref().unwrap();
    let mut bestnode = NONE;
    let mut bestprio = crate::def::REAL_MIN;
    for i in 0..tree.nsiblings {
        if tree.siblingsprio[i] > bestprio {
            bestnode = tree.siblings[i];
            bestprio = tree.siblingsprio[i];
        }
    }
    bestnode
}

/// `SCIPtreeGetBestChild`: gets the best child of the focus node w.r.t.
/// the node selection strategy.
pub fn tree_get_best_child(scip: &Scip) -> NodeId {
    let tree = scip.tree.as_ref().unwrap();
    let nodesel = tree.leaves.nodesel;
    let mut bestnode = NONE;
    for i in 0..tree.nchildren {
        if bestnode == NONE || crate::nodesel::nodesel_compare(scip, nodesel, tree.children[i], bestnode) < 0 {
            bestnode = tree.children[i];
        }
    }
    bestnode
}

/// `SCIPtreeGetBestSibling`: gets the best sibling of the focus node
/// w.r.t. the node selection strategy.
pub fn tree_get_best_sibling(scip: &Scip) -> NodeId {
    let tree = scip.tree.as_ref().unwrap();
    let nodesel = tree.leaves.nodesel;
    let mut bestnode = NONE;
    for i in 0..tree.nsiblings {
        if bestnode == NONE || crate::nodesel::nodesel_compare(scip, nodesel, tree.siblings[i], bestnode) < 0 {
            bestnode = tree.siblings[i];
        }
    }
    bestnode
}

/// `SCIPtreeGetBestLeaf`: gets the best leaf from the node queue w.r.t.
/// the node selection strategy.
pub fn tree_get_best_leaf(scip: &Scip) -> NodeId {
    crate::nodesel::nodepq_first(&scip.tree.as_ref().unwrap().leaves)
}

/// `SCIPtreeGetBestNode`: gets the best node from the tree (child,
/// sibling, or leaf) w.r.t. the node selection strategy.
pub fn tree_get_best_node(scip: &Scip) -> NodeId {
    let nodesel = scip.tree.as_ref().unwrap().leaves.nodesel;

    // Get the best child, sibling, and leaf.
    let bestchild = tree_get_best_child(scip);
    let bestsibling = tree_get_best_sibling(scip);
    let bestleaf = tree_get_best_leaf(scip);

    // Return the best of the three.
    let mut bestnode = bestchild;
    if bestsibling != NONE && (bestnode == NONE || crate::nodesel::nodesel_compare(scip, nodesel, bestsibling, bestnode) < 0) {
        bestnode = bestsibling;
    }
    if bestleaf != NONE && (bestnode == NONE || crate::nodesel::nodesel_compare(scip, nodesel, bestleaf, bestnode) < 0) {
        bestnode = bestleaf;
    }
    bestnode
}

/// `SCIPtreeGetLowerbound`: gets the minimal lower bound of all nodes in
/// the tree.
pub fn tree_get_lowerbound(scip: &Scip) -> f64 {
    let tree = scip.tree.as_ref().unwrap();

    // Get the lower bound from the queue.
    let mut lowerbound = crate::nodesel::nodepq_get_lowerbound(scip);

    // Compare lower bound with siblings.
    for i in 0..tree.nsiblings {
        lowerbound = crate::def::min(lowerbound, scip.nodes[tree.siblings[i]].lowerbound);
    }

    // Compare lower bound with focus node.
    if tree.focusnode != NONE {
        lowerbound = crate::def::min(lowerbound, scip.nodes[tree.focusnode].lowerbound);
    }
    lowerbound
}

/// `SCIPtreeGetLowerboundNode`: gets the node with minimal lower bound of
/// all nodes in the tree (child, sibling, or leaf).
pub fn tree_get_lowerbound_node(scip: &Scip) -> NodeId {
    let tree = scip.tree.as_ref().unwrap();

    // Get the lower bound from the queue.
    let mut lowerboundnode = crate::nodesel::nodepq_get_lowerbound_node(scip);
    let mut bestprio = -scip.set.infinity();
    let mut lowerbound = if lowerboundnode != NONE {
        scip.nodes[lowerboundnode].lowerbound
    } else {
        scip.set.infinity()
    };

    // Compare lower bound with children.
    for i in 0..tree.nchildren {
        let lb = scip.nodes[tree.children[i]].lowerbound;
        if scip.set.is_le(lb, lowerbound) && (scip.set.is_lt(lb, lowerbound) || tree.childrenprio[i] > bestprio) {
            lowerboundnode = tree.children[i];
            bestprio = tree.childrenprio[i];
            lowerbound = lb;
        }
    }

    // Compare lower bound with siblings.
    for i in 0..tree.nsiblings {
        let lb = scip.nodes[tree.siblings[i]].lowerbound;
        if scip.set.is_le(lb, lowerbound) && (scip.set.is_lt(lb, lowerbound) || tree.siblingsprio[i] > bestprio) {
            lowerboundnode = tree.siblings[i];
            bestprio = tree.siblingsprio[i];
            lowerbound = lb;
        }
    }
    lowerboundnode
}

/// `SCIPtreeGetAvgLowerbound`: gets the average lower bound of all nodes
/// in the tree.
pub fn tree_get_avg_lowerbound(scip: &Scip, cutoffbound: f64) -> f64 {
    let tree = scip.tree.as_ref().unwrap();

    // Get sum of lower bounds from nodes in the queue.
    let mut lowerboundsum = tree.leaves.lowerboundsum;
    let mut nnodes = tree_get_n_leaves(scip);

    // Add lower bound of focus node.
    if tree.focusnode != NONE && scip.nodes[tree.focusnode].lowerbound < cutoffbound {
        lowerboundsum += scip.nodes[tree.focusnode].lowerbound;
        nnodes += 1;
    }

    // Add lower bounds of siblings.
    for i in 0..tree.nsiblings {
        lowerboundsum += scip.nodes[tree.siblings[i]].lowerbound;
    }
    nnodes += tree.nsiblings;

    // Add lower bounds of children.
    for i in 0..tree.nchildren {
        lowerboundsum += scip.nodes[tree.children[i]].lowerbound;
    }
    nnodes += tree.nchildren;

    if nnodes == 0 { 0.0 } else { lowerboundsum / nnodes as f64 }
}

/*
 * simple functions implemented as defines
 */

/// `SCIPnodeGetNDomchg`: counts the number of bound changes due to
/// branching, constraint propagation, and propagation; returns
/// `(nbranchings, nconsprop, nprop)`.
pub fn node_get_n_domchg(scip: &Scip, node: NodeId) -> (i32, i32, i32) {
    let (mut nbranchings, mut nconsprop, mut nprop) = (0, 0, 0);
    let Some(d) = &scip.nodes[node].domchg else { return (0, 0, 0) };
    let n = d.nboundchgs as i32;

    // Branching bound changes are always at beginning, count them in i.
    let mut i = 0;
    while i < n {
        if d.boundchgs[i].boundchgtype != BoundchgType::Branching {
            break;
        }
        i += 1;
    }
    nbranchings = nbranchings.max(i);

    while i < n {
        if d.boundchgs[i].boundchgtype == BoundchgType::ConsInfer {
            nconsprop += 1;
        } else {
            nprop += 1;
        }
        i += 1;
    }
    (nbranchings, nconsprop, nprop)
}

/// `SCIPnodeGetParentBranchings`: returns the set of variable branchings
/// that were performed in the parent node to create this node, as
/// `(var, bound, boundtype)`.
pub fn node_get_parent_branchings(scip: &Scip, node: NodeId) -> Vec<(VarId, f64, BoundType)> {
    let n = &scip.nodes[node];
    let mut out = Vec::new();
    if n.depth == 0 {
        return out;
    }
    let Some(d) = &n.domchg else { return out };

    // Count the number of branching decisions; branching decisions have to
    // be in the beginning of the bound change array.
    for i in 0..d.nboundchgs as i32 {
        let b = &d.boundchgs[i];
        if b.boundchgtype != BoundchgType::Branching {
            break;
        }
        out.push((b.var, b.newbound, b.boundtype));
    }
    out
}

/// `SCIPnodeGetAncestorBranchings`: returns the set of variable branchings
/// that were performed in all ancestor nodes (nodes on the path to the
/// root) to create this node.
pub fn node_get_ancestor_branchings(scip: &Scip, node: NodeId) -> Vec<(VarId, f64, BoundType)> {
    let mut out = Vec::new();
    let mut node = node;
    while scip.nodes[node].depth != 0 {
        out.extend(node_get_parent_branchings(scip, node));
        node = scip.nodes[node].parent;
    }
    out
}

/// `SCIPnodeGetAncestorBranchingsPart`: returns the set of variable
/// branchings that were performed between the given node and the given
/// parent node.
pub fn node_get_ancestor_branchings_part(scip: &Scip, node: NodeId, parent: NodeId) -> Vec<(VarId, f64, BoundType)> {
    let mut out = Vec::new();
    let mut node = node;
    while node != parent {
        out.extend(node_get_parent_branchings(scip, node));
        node = scip.nodes[node].parent;
    }
    out
}

/// `SCIPnodesSharePath`: checks for two nodes whether they share the same
/// root path, i.e., whether one is an ancestor of the other.
pub fn nodes_share_path(scip: &Scip, node1: NodeId, node2: NodeId) -> bool {
    let (mut node1, mut node2) = (node1, node2);

    // If node2 is deeper than node1, follow the path until the level of
    // node2.
    while scip.nodes[node1].depth < scip.nodes[node2].depth {
        node2 = scip.nodes[node2].parent;
    }

    // If node1 is deeper than node2, follow the path until the level of
    // node1.
    while scip.nodes[node2].depth < scip.nodes[node1].depth {
        node1 = scip.nodes[node1].parent;
    }
    node1 == node2
}

/// `SCIPnodesGetCommonAncestor`: finds the common ancestor node of two
/// given nodes.
pub fn nodes_get_common_ancestor(scip: &Scip, node1: NodeId, node2: NodeId) -> NodeId {
    let (mut node1, mut node2) = (node1, node2);
    while scip.nodes[node1].depth < scip.nodes[node2].depth {
        node2 = scip.nodes[node2].parent;
    }
    while scip.nodes[node2].depth < scip.nodes[node1].depth {
        node1 = scip.nodes[node1].parent;
    }

    // Move up level by level until you found a common ancestor.
    while node1 != node2 {
        node1 = scip.nodes[node1].parent;
        node2 = scip.nodes[node2].parent;
    }
    node1
}

/// `SCIPtreeGetNChildren`.
pub fn tree_get_n_children(scip: &Scip) -> i32 {
    scip.tree.as_ref().unwrap().nchildren
}

/// `SCIPtreeGetNSiblings`.
pub fn tree_get_n_siblings(scip: &Scip) -> i32 {
    scip.tree.as_ref().unwrap().nsiblings
}

/// `SCIPtreeGetNLeaves`: gets number of leaves in the tree (excluding
/// children and siblings of focus nodes).
pub fn tree_get_n_leaves(scip: &Scip) -> i32 {
    scip.tree.as_ref().unwrap().leaves.len
}

/// `SCIPtreeGetNNodes`: gets number of open nodes in the tree (children +
/// siblings + leaves).
pub fn tree_get_n_nodes(scip: &Scip) -> i32 {
    let tree = scip.tree.as_ref().unwrap();
    tree.nchildren + tree.nsiblings + tree_get_n_leaves(scip)
}

/// `SCIPtreeIsPathComplete`: returns whether the active path goes
/// completely down to the focus node.
pub fn tree_is_path_complete(scip: &Scip) -> bool {
    let tree = scip.tree.as_ref().unwrap();
    tree.focusnode == NONE || (scip.nodes[tree.focusnode].depth as i32) < tree.pathlen
}

/// `SCIPtreeProbing`: returns whether the current node is a temporary
/// probing node.
pub fn tree_probing(scip: &Scip) -> bool {
    scip.tree.as_ref().unwrap().probingroot != NONE
}

/// `SCIPtreeGetProbingRoot`.
pub fn tree_get_probing_root(scip: &Scip) -> NodeId {
    scip.tree.as_ref().unwrap().probingroot
}

/// `SCIPtreeGetFocusNode`.
pub fn tree_get_focus_node(scip: &Scip) -> NodeId {
    scip.tree.as_ref().unwrap().focusnode
}

/// `SCIPtreeGetFocusDepth`: gets depth of focus node in the tree.
pub fn tree_get_focus_depth(scip: &Scip) -> i32 {
    let tree = scip.tree.as_ref().unwrap();
    if tree.focusnode != NONE { scip.nodes[tree.focusnode].depth as i32 } else { -1 }
}

/// `SCIPtreeHasFocusNodeLP`: returns whether the LP was or is to be solved
/// in the focus node.
pub fn tree_has_focus_node_lp(scip: &Scip) -> bool {
    scip.tree.as_ref().unwrap().focusnodehaslp
}

/// `SCIPtreeSetFocusNodeLP`: sets mark to solve or to ignore the LP while
/// processing the focus node.
pub fn tree_set_focus_node_lp(scip: &mut Scip, solvelp: bool) {
    tree!(scip).focusnodehaslp = solvelp;
}

/// `SCIPtreeIsFocusNodeLPConstructed`.
pub fn tree_is_focus_node_lp_constructed(scip: &Scip) -> bool {
    scip.tree.as_ref().unwrap().focuslpconstructed
}

/// `SCIPtreeInRepropagation`: returns whether the focus node is already
/// solved and only propagated again.
pub fn tree_in_repropagation(scip: &Scip) -> bool {
    let tree = scip.tree.as_ref().unwrap();
    tree.focusnode != NONE && scip.nodes[tree.focusnode].nodetype == NodeType::RefocusNode
}

/// `SCIPtreeGetCurrentNode`: gets current node of the tree, i.e. the last
/// node in the active path, or `NONE` if no current node exists.
pub fn tree_get_current_node(scip: &Scip) -> NodeId {
    let tree = scip.tree.as_ref().unwrap();
    if tree.pathlen > 0 { tree.path[tree.pathlen - 1] } else { NONE }
}

/// `SCIPtreeGetCurrentDepth`: gets depth of current node in the tree, i.e.
/// the length of the active path minus 1, or -1 if no current node exists.
pub fn tree_get_current_depth(scip: &Scip) -> i32 {
    scip.tree.as_ref().unwrap().pathlen - 1
}

/// `SCIPtreeHasCurrentNodeLP`: returns whether the LP was or is to be
/// solved in the current node.
pub fn tree_has_current_node_lp(scip: &Scip) -> bool {
    if tree_probing(scip) {
        scip.tree.as_ref().unwrap().probingnodehaslp
    } else {
        tree_has_focus_node_lp(scip)
    }
}

/// `SCIPtreeGetProbingDepth`: returns the current probing depth, i.e. the
/// number of probing sub nodes existing in the probing path.
pub fn tree_get_probing_depth(scip: &Scip) -> i32 {
    tree_get_current_depth(scip) - scip.nodes[scip.tree.as_ref().unwrap().probingroot].depth as i32
}

/// `SCIPtreeGetEffectiveRootDepth`: returns the depth of the effective
/// root node (i.e. the first depth level of a node with at least two
/// children).
pub fn tree_get_effective_root_depth(scip: &Scip) -> i32 {
    scip.tree.as_ref().unwrap().effectiverootdepth
}

/// `SCIPtreeGetRootNode`.
pub fn tree_get_root_node(scip: &Scip) -> NodeId {
    scip.tree.as_ref().unwrap().root
}

/// `SCIPtreeProbingObjChanged`: returns whether we are in probing and the
/// objective value of at least one column was changed.
pub fn tree_probing_obj_changed(scip: &Scip) -> bool {
    scip.tree.as_ref().unwrap().probingobjchanged
}

/// `SCIPtreeMarkProbingObjChanged`: marks the current probing node to have
/// a changed objective function.
pub fn tree_mark_probing_obj_changed(scip: &mut Scip) {
    tree!(scip).probingobjchanged = true;
}
