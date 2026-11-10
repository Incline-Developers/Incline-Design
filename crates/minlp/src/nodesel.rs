//! SCIP's node selectors and node priority queue (`nodesel.c`,
//! `struct_nodesel.h`).

use std::any::Any;

use crate::{
    clock::Clock,
    cvec::CVec,
    retcode::ScipResult,
    scip::Scip,
    types::{NodeId, NodeselId},
};

/// `SCIP_NODEPQ`.
#[derive(Clone, Debug, Default)]
pub struct NodePq {
    pub lowerboundsum: f64,
    pub nodesel: NodeselId,
    pub slots: CVec<NodeId>,
    pub bfsposs: CVec<i32>,
    pub bfsqueue: CVec<i32>,
    pub len: i32,
    pub size: i32,
}

pub type NodeselMethod = fn(&mut Scip, NodeselId) -> ScipResult;
/// A copy method: `(target scip, source scip, source plugin)`.
pub type NodeselCopy = fn(&mut Scip, &Scip, NodeselId) -> ScipResult;
pub type NodeselSelect = fn(&mut Scip, NodeselId, &mut NodeId) -> ScipResult;
pub type NodeselComp = fn(&Scip, NodeselId, NodeId, NodeId) -> i32;

/// `SCIP_NODESEL`.
pub struct Nodesel {
    pub name: String,
    pub desc: String,
    pub nodeselcopy: Option<NodeselCopy>,
    pub nodeselfree: Option<NodeselMethod>,
    pub nodeselinit: Option<NodeselMethod>,
    pub nodeselexit: Option<NodeselMethod>,
    pub nodeselinitsol: Option<NodeselMethod>,
    pub nodeselexitsol: Option<NodeselMethod>,
    pub nodeselselect: Option<NodeselSelect>,
    pub nodeselcomp: Option<NodeselComp>,
    pub setuptime: Clock,
    pub nodeseltime: Clock,
    pub nodeseldata: Option<Box<dyn Any>>,
    pub stdpriority: i32,
    pub memsavepriority: i32,
    pub initialized: bool,
}

use crate::{retcode::Retcode, tree, var::NONE};

/*
 * node priority queue methods
 */

#[inline]
fn pq_parent(q: i32) -> i32 {
    (q + 1) / 2 - 1
}
#[inline]
fn pq_leftchild(p: i32) -> i32 {
    2 * p + 1
}
#[inline]
fn pq_rightchild(p: i32) -> i32 {
    2 * p + 2
}

/// `nodepqResize`: resizes node memory to hold at least the given number
/// of nodes.
fn nodepq_resize(scip: &Scip, nodepq: &mut NodePq, minsize: i32) {
    if minsize <= nodepq.size {
        return;
    }
    nodepq.size = scip.set.calc_tree_grow_size(minsize);
    nodepq.slots.0.resize(nodepq.size as usize, NONE);
    nodepq.bfsposs.realloc(nodepq.size);
    nodepq.bfsqueue.realloc(nodepq.size);
}

/// `SCIPnodepqCreate`: creates node priority queue.
pub fn nodepq_create(_scip: &mut Scip, nodesel: NodeselId) -> ScipResult<NodePq> {
    Ok(NodePq {
        nodesel,
        slots: CVec::new(),
        bfsposs: CVec::new(),
        bfsqueue: CVec::new(),
        len: 0,
        size: 0,
        lowerboundsum: 0.0,
    })
}

/// `SCIPnodepqFree`: frees the tree's node priority queue and all nodes in
/// the queue.
pub fn nodepq_free(scip: &mut Scip, eventfilter: usize) -> ScipResult {
    // Free the nodes of the queue.
    nodepq_clear(scip, eventfilter)
}

/// `SCIPnodepqClear`: deletes all nodes in the tree's node priority queue.
pub fn nodepq_clear(scip: &mut Scip, eventfilter: usize) -> ScipResult {
    let len = treer!(scip).leaves.len;
    if len > 0 {
        // Sort the nodes downwards after their number to increase speed when
        // freeing in debug mode (node numbers are unique, so every sort
        // gives this order).
        let mut slots = tree!(scip).leaves.slots.head(len).to_vec();
        slots.sort_by(|&a, &b| scip.nodes[b].number.cmp(&scip.nodes[a].number));
        tree!(scip).leaves.slots.head_mut(len).copy_from_slice(&slots);

        // Free the nodes of the queue.
        for node in slots {
            tree::node_free(scip, node, eventfilter)?;
        }
    }

    // Reset data.
    let pq = &mut tree!(scip).leaves;
    pq.len = 0;
    pq.lowerboundsum = 0.0;
    Ok(())
}

/// `SCIPnodepqSetNodesel`: sets the node selector used for sorting the
/// nodes in the tree's queue, and resorts the queue if necessary.
pub fn nodepq_set_nodesel(scip: &mut Scip, nodesel: NodeselId) -> ScipResult {
    if treer!(scip).leaves.nodesel == nodesel {
        return Ok(());
    }

    // Create new node priority queue.
    let mut newnodepq = nodepq_create(scip, nodesel)?;

    // Resize the new node priority queue to be able to store all nodes.
    let old = std::mem::take(&mut tree!(scip).leaves);
    nodepq_resize(scip, &mut newnodepq, old.len);

    // Insert all nodes in the new node priority queue.
    for i in 0..old.len {
        nodepq_insert_into(scip, &mut newnodepq, old.slots[i]);
    }

    // Use the new node priority queue.
    tree!(scip).leaves = newnodepq;
    Ok(())
}

/// `SCIPnodepqCompare`: compares two nodes with the tree queue's node
/// selector; returns -1/0/+1 if node1 better/equal/worse than node2.
pub fn nodepq_compare(scip: &Scip, node1: NodeId, node2: NodeId) -> i32 {
    nodesel_compare(scip, scip.tree.as_ref().unwrap().leaves.nodesel, node1, node2)
}

/// `SCIPnodepqInsert` on a queue held outside the tree.
fn nodepq_insert_into(scip: &Scip, nodepq: &mut NodePq, node: NodeId) {
    let nodesel = nodepq.nodesel;
    let comp = scip.nodesels[nodesel].nodeselcomp.expect("a node comparator");

    nodepq_resize(scip, nodepq, nodepq.len + 1);
    let NodePq { slots, bfsposs, bfsqueue, .. } = nodepq;

    // Insert node as leaf in the tree, move it towards the root as long it
    // is better than its parent.
    nodepq.len += 1;
    nodepq.lowerboundsum += scip.nodes[node].lowerbound;
    let mut pos = nodepq.len - 1;
    while pos > 0 && comp(scip, nodesel, node, slots[pq_parent(pos)]) < 0 {
        slots[pos] = slots[pq_parent(pos)];
        bfsposs[pos] = bfsposs[pq_parent(pos)];
        bfsqueue[bfsposs[pos]] = pos;
        pos = pq_parent(pos);
    }
    slots[pos] = node;

    // Insert the final position into the bfs index queue.
    let mut bfspos = nodepq.len - 1;
    let lowerbound = scip.nodes[node].lowerbound;
    while bfspos > 0 && lowerbound < scip.nodes[slots[bfsqueue[pq_parent(bfspos)]]].lowerbound {
        bfsqueue[bfspos] = bfsqueue[pq_parent(bfspos)];
        bfsposs[bfsqueue[bfspos]] = bfspos;
        bfspos = pq_parent(bfspos);
    }
    bfsqueue[bfspos] = pos;
    bfsposs[pos] = bfspos;
}

/// `SCIPnodepqInsert`: inserts node into the tree's node priority queue.
pub fn nodepq_insert(scip: &mut Scip, node: NodeId) -> ScipResult {
    let mut pq = std::mem::take(&mut tree!(scip).leaves);
    nodepq_insert_into(scip, &mut pq, node);
    tree!(scip).leaves = pq;
    Ok(())
}

/// `nodepqDelPos`: deletes node at given position from the node priority
/// queue; returns TRUE, if the parent fell down to the free position.
fn nodepq_del_pos(scip: &Scip, nodepq: &mut NodePq, rempos: i32) -> bool {
    let nodesel = nodepq.nodesel;
    let comp = scip.nodesels[nodesel].nodeselcomp.expect("a node comparator");
    let lb = |n: NodeId| scip.nodes[n].lowerbound;

    nodepq.lowerboundsum -= lb(nodepq.slots[rempos]);
    let mut freepos = rempos;
    let mut freebfspos = nodepq.bfsposs[rempos];

    // Remove node of the tree and get a free slot,
    // if the removed node was the last node of the queue
    //  - do nothing
    // if the last node of the queue is better than the parent of the
    // removed node:
    //  - move the parent to the free slot, until the last node can be
    //    placed in the free slot
    // if the last node of the queue is not better than the parent of the
    // free slot:
    //  - move the better child to the free slot until the last node can be
    //    placed in the free slot
    nodepq.len -= 1;
    let len = nodepq.len;
    let NodePq { slots, bfsposs, bfsqueue, .. } = nodepq;

    // Process the slots queue ordered by the node selection comparator.
    let lastnode = slots[len];
    let lastbfspos = bfsposs[len];
    let mut parentfelldown = false;
    if freepos < len {
        // Try to move parents downwards to insert last node.
        let mut parentpos = pq_parent(freepos);
        while freepos > 0 && comp(scip, nodesel, lastnode, slots[parentpos]) < 0 {
            slots[freepos] = slots[parentpos];
            bfsposs[freepos] = bfsposs[parentpos];
            bfsqueue[bfsposs[freepos]] = freepos;
            freepos = parentpos;
            parentpos = pq_parent(freepos);
            parentfelldown = true;
        }
        if !parentfelldown {
            // Downward moving of parents was not successful -> move children
            // upwards. As long as free slot has children...
            while freepos <= pq_parent(len - 1) {
                // Select the better child of free slot.
                let mut childpos = pq_leftchild(freepos);
                let brotherpos = pq_rightchild(freepos);
                if brotherpos < len && comp(scip, nodesel, slots[brotherpos], slots[childpos]) < 0 {
                    childpos = brotherpos;
                }

                // Exit search loop if better child is not better than last
                // node.
                if comp(scip, nodesel, lastnode, slots[childpos]) <= 0 {
                    break;
                }

                // Move better child upwards, free slot is now the better
                // child's slot.
                slots[freepos] = slots[childpos];
                bfsposs[freepos] = bfsposs[childpos];
                bfsqueue[bfsposs[freepos]] = freepos;
                freepos = childpos;
            }
        }
        slots[freepos] = lastnode;
        bfsposs[freepos] = lastbfspos;
        bfsqueue[lastbfspos] = freepos;
    }

    // Process the bfs queue ordered by the lower bound.
    let lastbfsqueueidx = bfsqueue[len];
    let mut bfsparentfelldown = false;
    if freebfspos < len {
        // Try to move parents downwards to insert last queue index.
        let mut parentpos = pq_parent(freebfspos);
        let lastlowerbound = lb(slots[lastbfsqueueidx]);
        while freebfspos > 0 && lastlowerbound < lb(slots[bfsqueue[parentpos]]) {
            bfsqueue[freebfspos] = bfsqueue[parentpos];
            bfsposs[bfsqueue[freebfspos]] = freebfspos;
            freebfspos = parentpos;
            parentpos = pq_parent(freebfspos);
            bfsparentfelldown = true;
        }
        if !bfsparentfelldown {
            // Downward moving of parents was not successful -> move children
            // upwards. As long as free slot has children...
            while freebfspos <= pq_parent(len - 1) {
                // Select the better child of free slot.
                let mut childpos = pq_leftchild(freebfspos);
                let brotherpos = pq_rightchild(freebfspos);
                if brotherpos < len && lb(slots[bfsqueue[brotherpos]]) < lb(slots[bfsqueue[childpos]]) {
                    childpos = brotherpos;
                }

                // Exit search loop if better child is not better than last
                // node.
                if lastlowerbound <= lb(slots[bfsqueue[childpos]]) {
                    break;
                }

                // Move better child upwards, free slot is now the better
                // child's slot.
                bfsqueue[freebfspos] = bfsqueue[childpos];
                bfsposs[bfsqueue[freebfspos]] = freebfspos;
                freebfspos = childpos;
            }
        }
        bfsqueue[freebfspos] = lastbfsqueueidx;
        bfsposs[lastbfsqueueidx] = freebfspos;
    }

    parentfelldown
}

/// `nodepqFindNode`: returns the position of given node in the priority
/// queue, or -1 if not existing.
fn nodepq_find_node(nodepq: &NodePq, node: NodeId) -> i32 {
    // Search the node in the queue.
    let mut pos = 0;
    while pos < nodepq.len && node != nodepq.slots[pos] {
        pos += 1;
    }
    if pos == nodepq.len {
        pos = -1;
    }
    pos
}

/// `SCIPnodepqRemove`: removes node from the tree's node priority queue.
pub fn nodepq_remove(scip: &mut Scip, node: NodeId) -> ScipResult {
    let pos = nodepq_find_node(&treer!(scip).leaves, node);
    if pos == -1 {
        log::error!("node doesn't exist in node priority queue");
        return Err(Retcode::InvalidData);
    }

    let mut pq = std::mem::take(&mut tree!(scip).leaves);
    nodepq_del_pos(scip, &mut pq, pos);
    tree!(scip).leaves = pq;
    Ok(())
}

/// `SCIPnodepqFirst`: returns the best node of the queue without removing
/// it.
pub fn nodepq_first(nodepq: &NodePq) -> NodeId {
    if nodepq.len == 0 {
        return NONE;
    }
    nodepq.slots[0]
}

/// `SCIPnodepqNodes`: returns the nodes array of the queue.
pub fn nodepq_nodes(nodepq: &NodePq) -> &[NodeId] {
    nodepq.slots.head(nodepq.len)
}

/// `SCIPnodepqLen`: returns the number of nodes stored in the node
/// priority queue.
pub fn nodepq_len(nodepq: &NodePq) -> i32 {
    nodepq.len
}

/// `SCIPnodepqGetLowerbound`: gets the minimal lower bound of all nodes in
/// the tree's queue.
pub fn nodepq_get_lowerbound(scip: &Scip) -> f64 {
    let pq = &scip.tree.as_ref().unwrap().leaves;
    if pq.len > 0 {
        let bfspos = pq.bfsqueue[0];
        scip.nodes[pq.slots[bfspos]].lowerbound
    } else {
        scip.set.infinity()
    }
}

/// `SCIPnodepqGetLowerboundNode`: gets the node with minimal lower bound
/// of all nodes in the tree's queue.
pub fn nodepq_get_lowerbound_node(scip: &Scip) -> NodeId {
    // The node selector's compare method sorts the minimal lower bound to
    // the front.
    let pq = &scip.tree.as_ref().unwrap().leaves;
    if pq.len > 0 {
        let bfspos = pq.bfsqueue[0];
        pq.slots[bfspos]
    } else {
        NONE
    }
}

/// `SCIPnodepqGetLowerboundSum`: gets the sum of lower bounds of all nodes
/// in the queue.
pub fn nodepq_get_lowerbound_sum(nodepq: &NodePq) -> f64 {
    nodepq.lowerboundsum
}

/// `SCIPnodepqBound`: free all nodes from the tree's queue that are cut
/// off by the given upper bound.
pub fn nodepq_bound(scip: &mut Scip, eventfilter: usize, cutoffbound: f64) -> ScipResult {
    let mut pos = treer!(scip).leaves.len - 1;

    while pos >= 0 {
        let node = treer!(scip).leaves.slots[pos];

        // Cut off node.
        if scip.set.is_ge(scip.nodes[node].lowerbound, cutoffbound) {
            // Check if the node should be stored for reoptimization.
            if scip.set.reopt_enable {
                unimplemented!("reoptimization");
            }

            // Remove node from the queue
            // - if the slot was occupied by the parent, we have to check this
            //   slot (the parent) again; unfortunately, we will check the node
            //   which occupied the parent's slot again, even though it cannot
            //   be cut off;
            // - otherwise, the slot was the last slot or it was occupied by a
            //   node with a position greater than the current position; this
            //   node was already checked and we can decrease the position.
            let mut pq = std::mem::take(&mut tree!(scip).leaves);
            let parentfelldown = nodepq_del_pos(scip, &mut pq, pos);
            tree!(scip).leaves = pq;
            if !parentfelldown {
                pos -= 1;
            }

            let infinity = scip.set.infinity();
            let n = &mut scip.nodes[node];
            n.cutoff = true;
            n.lowerbound = infinity;
            n.estimate = infinity;

            if n.depth == 0 {
                scip.stat.rootlowerbound = infinity;
            }

            let lowerbound = tree::tree_get_lowerbound(scip);

            // Lower bound improved.
            if scip.stat.lastlowerbound < lowerbound {
                // Throw improvement event if not already done exactly.
                if scip.stat.lastlowerbound < scip.primals[crate::scip::TRANSPRIMAL].upperbound {
                    let event = crate::event::event_create_node(crate::event::EVENTTYPE_DUALBOUNDIMPROVED, NONE);
                    crate::event::event_process(scip, &event, eventfilter)?;
                }

                // Update primal-dual integrals.
                if scip.set.misc_calcintegral {
                    crate::stat::stat_update_primal_dual_integrals(scip, crate::prob::TRANSPROB, crate::prob::ORIGPROB, infinity, lowerbound);
                } else {
                    scip.stat.lastlowerbound = lowerbound;
                }
            }

            // Free node memory.
            tree::node_free(scip, node, eventfilter)?;
        } else {
            pos -= 1;
        }
    }
    Ok(())
}

/*
 * node selector methods
 */

/// `paramChgdNodeselStdPriority` and `paramChgdNodeselMemsavePriority`:
/// the parameter already wrote the priority; mark the node selectors
/// unsorted (`SCIPsetNodeselStdPriority` / `SCIPsetNodeselMemsavePriority`).
fn param_chgd_nodesel_priority(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    scip.set.nodesel = NONE;
    Ok(())
}

/// `SCIPnodeselCopyInclude`: copies the given node selector to a new scip.
pub fn nodesel_copy_include(scip: &mut Scip, sourcescip: &Scip, nodesel: NodeselId) -> ScipResult {
    if let Some(copy) = sourcescip.nodesels[nodesel].nodeselcopy {
        copy(scip, sourcescip, nodesel)?;
    }
    Ok(())
}

/// The callbacks `SCIPnodeselCreate` takes.
#[derive(Default)]
pub struct NodeselMethods {
    pub nodeselcopy: Option<NodeselCopy>,
    pub nodeselfree: Option<NodeselMethod>,
    pub nodeselinit: Option<NodeselMethod>,
    pub nodeselexit: Option<NodeselMethod>,
    pub nodeselinitsol: Option<NodeselMethod>,
    pub nodeselexitsol: Option<NodeselMethod>,
    pub nodeselselect: Option<NodeselSelect>,
    pub nodeselcomp: Option<NodeselComp>,
}

/// `SCIPnodeselCreate`: creates a node selector.
pub fn nodesel_create(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    stdpriority: i32,
    memsavepriority: i32,
    methods: NodeselMethods,
    nodeseldata: Option<Box<dyn Any>>,
) -> ScipResult<NodeselId> {
    let nodesel = scip.nodesels.push(Nodesel {
        name: name.to_string(),
        desc: desc.to_string(),
        stdpriority,
        memsavepriority,
        nodeselcopy: methods.nodeselcopy,
        nodeselfree: methods.nodeselfree,
        nodeselinit: methods.nodeselinit,
        nodeselexit: methods.nodeselexit,
        nodeselinitsol: methods.nodeselinitsol,
        nodeselexitsol: methods.nodeselexitsol,
        nodeselselect: methods.nodeselselect,
        nodeselcomp: methods.nodeselcomp,
        nodeseldata,
        initialized: false,
        // Create clocks.
        setuptime: Clock::new(),
        nodeseltime: Clock::new(),
    });

    // Add parameters.
    crate::paramset::paramset_add_int(
        scip,
        &format!("nodeselection/{name}/stdpriority"),
        &format!("priority of node selection rule <{name}> in standard mode"),
        crate::field_target!(i32, nodesels, nodesel, stdpriority),
        false,
        stdpriority,
        i32::MIN / 4,
        i32::MAX / 2,
        Some(param_chgd_nodesel_priority),
    )?;
    crate::paramset::paramset_add_int(
        scip,
        &format!("nodeselection/{name}/memsavepriority"),
        &format!("priority of node selection rule <{name}> in memory saving mode"),
        crate::field_target!(i32, nodesels, nodesel, memsavepriority),
        true,
        memsavepriority,
        i32::MIN / 4,
        i32::MAX / 4,
        Some(param_chgd_nodesel_priority),
    )?;
    Ok(nodesel)
}

/// `SCIPnodeselFree`: frees memory of node selector.
pub fn nodesel_free(scip: &mut Scip, nodesel: NodeselId) -> ScipResult {
    // Call destructor of node selector.
    if let Some(free) = scip.nodesels[nodesel].nodeselfree {
        free(scip, nodesel)?;
    }
    scip.nodesels.remove(nodesel);
    Ok(())
}

/// Runs one of the node selector's setup callbacks under its setup clock.
fn nodesel_setup_call(scip: &mut Scip, nodesel: NodeselId, method: Option<NodeselMethod>) -> ScipResult {
    if let Some(method) = method {
        // Start timing.
        let enabled = scip.set.time_enabled;
        scip.nodesels[nodesel].setuptime.start(enabled);
        method(scip, nodesel)?;

        // Stop timing.
        scip.nodesels[nodesel].setuptime.stop(enabled);
    }
    Ok(())
}

/// `SCIPnodeselInit`: initializes node selector.
pub fn nodesel_init(scip: &mut Scip, nodesel: NodeselId) -> ScipResult {
    if scip.nodesels[nodesel].initialized {
        log::error!("node selector <{}> already initialized", scip.nodesels[nodesel].name);
        return Err(Retcode::InvalidCall);
    }

    if scip.set.misc_resetstat {
        let n = &mut scip.nodesels[nodesel];
        n.setuptime.reset();
        n.nodeseltime.reset();
    }

    let init = scip.nodesels[nodesel].nodeselinit;
    nodesel_setup_call(scip, nodesel, init)?;
    scip.nodesels[nodesel].initialized = true;
    Ok(())
}

/// `SCIPnodeselExit`: deinitializes node selector.
pub fn nodesel_exit(scip: &mut Scip, nodesel: NodeselId) -> ScipResult {
    if !scip.nodesels[nodesel].initialized {
        log::error!("node selector <{}> not initialized", scip.nodesels[nodesel].name);
        return Err(Retcode::InvalidCall);
    }

    let exit = scip.nodesels[nodesel].nodeselexit;
    nodesel_setup_call(scip, nodesel, exit)?;
    scip.nodesels[nodesel].initialized = false;
    Ok(())
}

/// `SCIPnodeselInitsol`: informs node selector that the branch and bound
/// process is being started.
pub fn nodesel_initsol(scip: &mut Scip, nodesel: NodeselId) -> ScipResult {
    // Call solving process initialization method of node selector.
    let initsol = scip.nodesels[nodesel].nodeselinitsol;
    nodesel_setup_call(scip, nodesel, initsol)
}

/// `SCIPnodeselExitsol`: informs node selector that the branch and bound
/// process data is being freed.
pub fn nodesel_exitsol(scip: &mut Scip, nodesel: NodeselId) -> ScipResult {
    // Call solving process deinitialization method of node selector.
    let exitsol = scip.nodesels[nodesel].nodeselexitsol;
    nodesel_setup_call(scip, nodesel, exitsol)
}

/// `SCIPnodeselSelect`: select next node to be processed.
pub fn nodesel_select(scip: &mut Scip, nodesel: NodeselId) -> ScipResult<NodeId> {
    let enabled = scip.set.time_enabled;

    // Start timing.
    scip.nodesels[nodesel].nodeseltime.start(enabled);

    let select = scip.nodesels[nodesel].nodeselselect.expect("a node selection method");
    let mut selnode = NONE;
    select(scip, nodesel, &mut selnode)?;

    // Stop timing.
    scip.nodesels[nodesel].nodeseltime.stop(enabled);
    Ok(selnode)
}

/// `SCIPnodeselCompare`: compares two nodes; returns -1/0/+1 if node1
/// better/equal/worse than node2.
pub fn nodesel_compare(scip: &Scip, nodesel: NodeselId, node1: NodeId, node2: NodeId) -> i32 {
    let comp = scip.nodesels[nodesel].nodeselcomp.expect("a node comparator");
    comp(scip, nodesel, node1, node2)
}

/// `SCIPnodeselSetStdPriority`: sets priority of node selector in standard
/// mode.
pub fn nodesel_set_std_priority(scip: &mut Scip, nodesel: NodeselId, priority: i32) {
    scip.nodesels[nodesel].stdpriority = priority;
    scip.set.nodesel = NONE;
}

/// `SCIPnodeselSetMemsavePriority`: sets priority of node selector in
/// memory saving mode.
pub fn nodesel_set_memsave_priority(scip: &mut Scip, nodesel: NodeselId, priority: i32) {
    scip.nodesels[nodesel].memsavepriority = priority;
    scip.set.nodesel = NONE;
}

/// `SCIPnodeselEnableOrDisableClocks`: enables or disables all clocks of
/// the node selector, depending on the value of the flag.
pub fn nodesel_enable_or_disable_clocks(scip: &mut Scip, nodesel: NodeselId, enable: bool) {
    let n = &mut scip.nodesels[nodesel];
    n.setuptime.enable_or_disable(enable);
    n.nodeseltime.enable_or_disable(enable);
}

/// `SCIPsetGetNodesel` (`set.c`): returns currently active node selector.
pub fn set_get_nodesel(scip: &mut Scip) -> NodeselId {
    // Check, if old node selector is still valid.
    if scip.set.nodesel == crate::var::NONE && scip.set.nnodesels > 0 {
        scip.set.nodesel = scip.set.nodesels[0];

        // Search highest priority node selector.
        for i in 1..scip.set.nnodesels as usize {
            let cand = scip.set.nodesels[i];
            let better = if scip.stat.memsavemode {
                scip.nodesels[cand].memsavepriority > scip.nodesels[scip.set.nodesel].memsavepriority
            } else {
                scip.nodesels[cand].stdpriority > scip.nodesels[scip.set.nodesel].stdpriority
            };
            if better {
                scip.set.nodesel = cand;
            }
        }
    }

    scip.set.nodesel
}
