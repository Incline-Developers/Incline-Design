//! Port of SCIP's `tclique` library (`tclique_branch.c`,
//! `tclique_coloring.c`): a branch and bound search for a maximum weight
//! clique, bounding by a coloring of the remaining subgraph.
//!
//! The graph is given by the [`TcliqueGraph`] callbacks (the default graph
//! of `tclique_graph.c` is not ported: its only user, the clique separator,
//! brings its own). C's linked lists of color intervals are vectors of
//! `(inf, sup)` pairs, edited in the same steps.

/// `TCLIQUE_WEIGHT`.
pub type TcliqueWeight = i32;

/// `TCLIQUE_STATUS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcliqueStatus {
    Error = 0,
    NodeLimit = 1,
    UserAbort = 2,
    Optimal = 3,
}

/// The graph callbacks `TCLIQUE_GETNNODES`, `TCLIQUE_GETWEIGHTS`,
/// `TCLIQUE_ISEDGE` and `TCLIQUE_SELECTADJNODES`.
pub trait TcliqueGraph {
    fn nnodes(&self) -> usize;
    fn weights(&self) -> &[TcliqueWeight];
    fn is_edge(&self, node1: i32, node2: i32) -> bool;
    /// Selects all nodes from `nodes` which are adjacent to `node`, in the
    /// given order.
    fn select_adj_nodes(&self, node: i32, nodes: &[i32]) -> Vec<i32>;
}

/// `TCLIQUE_NEWSOL`: called on every new solution with the clique's nodes
/// and weight; may raise `minweight`; sets whether the solution is accepted
/// as incumbent and whether solving should stop.
pub type NewSol<'a> = dyn FnMut(&[i32], TcliqueWeight, &mut TcliqueWeight, &mut bool, &mut bool) + 'a;

/// `CLIQUEHASH_INITSIZE`.
const CLIQUEHASH_INITSIZE: usize = 1024;

/// `CLIQUE`: a clique of the clique hash table, its nodes sorted.
struct Clique {
    nodes: Vec<i32>,
}

/// `createClique`: creates a clique, sorting the nodes into the clique's
/// node array.
fn create_clique(nodes: &[i32]) -> Clique {
    let mut out: Vec<i32> = vec![0; nodes.len()];
    for i in 0..nodes.len() {
        let node = nodes[i];
        let mut j = i;
        while j > 0 && node < out[j - 1] {
            out[j] = out[j - 1];
            j -= 1;
        }
        out[j] = node;
    }
    Clique { nodes: out }
}

/// `compSubcliques`: checks whether clique1 is a subset of clique2 and
/// returns: -1 if clique1 is not a subset of clique2 and lex-smaller, 0 if
/// it is a subset, +1 if it is not a subset and lex-larger.
fn comp_subcliques(clique1: &Clique, clique2: &Clique) -> i32 {
    let (mut pos1, mut pos2) = (0, 0);
    let mut clique2smaller = false;

    while pos1 < clique1.nodes.len() && pos2 < clique2.nodes.len() {
        if clique1.nodes[pos1] < clique2.nodes[pos2] {
            // Clique1 has an element not contained in clique2: clique1 is
            // lex-smaller, if clique2 was not lex-smaller before.
            return if clique2smaller { 1 } else { -1 };
        } else if clique1.nodes[pos1] > clique2.nodes[pos2] {
            // Clique2 has an element not contained in clique1: clique2 is
            // lex-smaller, but probably clique1 is contained in clique2.
            pos2 += 1;
            clique2smaller = true;
        } else {
            pos1 += 1;
            pos2 += 1;
        }
    }

    // If clique1 has additional elements, it is not contained in clique2.
    if pos1 < clique1.nodes.len() {
        return if clique2smaller { 1 } else { -1 };
    }

    // Clique1 is contained in clique2.
    0
}

/// `CLIQUEHASH`: the cliques found so far, sorted.
struct CliqueHash {
    cliques: Vec<Clique>,
}

impl CliqueHash {
    /// `inCliquehash`: searches the given clique in the clique hash table and
    /// returns whether it (or a stronger clique) exists, with the position
    /// where the clique should be inserted in the table.
    fn contains(&self, clique: &Clique) -> (bool, usize) {
        // Perform a binary search on the clique hash table.
        let mut left: i32 = 0;
        let mut right: i32 = self.cliques.len() as i32 - 1;
        while left <= right {
            let middle = (left + right) / 2;
            let cmp = comp_subcliques(clique, &self.cliques[middle as usize]);
            if cmp > 0 {
                left = middle + 1;
            } else if cmp < 0 {
                right = middle - 1;
            } else {
                return (true, middle as usize);
            }
        }

        // We found the correct insertion position for the clique, but it might
        // be contained in a lex-smaller clique.
        let insertpos = left as usize;
        for middle in (0..insertpos).rev() {
            if comp_subcliques(clique, &self.cliques[middle]) == 0 {
                return (true, insertpos);
            }
        }

        (false, insertpos)
    }
}

/*
 * Interval colorings (tclique_coloring.c)
 */

/// `NBC`: neighbor color information of a node.
#[derive(Clone, Default)]
pub struct Nbc {
    /// Number of colors used by neighbors.
    satdeg: i32,
    /// The neighbors' color intervals, sorted.
    lcitv: Vec<(i32, i32)>,
}

/// `getMaxSatdegIndex`: gets index of the uncolored node in a given array
/// of nodes in V with maximum satdeg; in case of a tie choose node with
/// maximum weight; if no uncolored node is found, -1 is returned.
fn get_max_satdeg_index(v: &[i32], gsd: &[Nbc], iscolored: &[bool], weights: &[TcliqueWeight]) -> i32 {
    let mut maxweight = -1;
    let mut maxsatdeg = -1;
    let mut maxsatdegindex = -1;

    for i in 0..v.len() {
        // Check only uncolored nodes.
        if iscolored[i] {
            continue;
        }

        let weight = weights[v[i] as usize];
        let satdeg = gsd[i].satdeg;

        if satdeg > maxsatdeg || (satdeg == maxsatdeg && weight > maxweight) {
            maxsatdeg = satdeg;
            maxweight = weight;
            maxsatdegindex = i as i32;
        }
    }

    maxsatdegindex
}

/// `getMaxWeightIndex`: gets index of the node in a given set of nodes with
/// maximum weight.
fn get_max_weight_index(weights: &[TcliqueWeight], v: &[i32]) -> i32 {
    let mut maxweightindex = -1;
    let mut maxweight = 0;

    // Try to improve maxweight.
    for i in 0..v.len() {
        if weights[v[i] as usize] > maxweight {
            // Node has larger weight.
            maxweight = weights[v[i] as usize];
            maxweightindex = i as i32;
        }
    }

    maxweightindex
}

/// `updateNeighbor`: updates the neighbor colors information of a node:
/// updates the list of neighbor color intervals by making the union of the
/// existing list and the given list of color intervals, and updates the
/// saturation degree.
fn update_neighbor(pgsd: &mut Nbc, pnc: &[(i32, i32)]) {
    let list = &mut pgsd.lcitv;

    // Construct the union of the two intervals: `apciv` is the position after
    // the last kept interval, `pciv` the current one.
    let mut pnci = 0;
    let mut pciv = 0;
    while pnci < pnc.len() && pciv < list.len() {
        if pnc[pnci].0 < list[pciv].0 {
            list.insert(pciv, pnc[pnci]);
            pciv += 1;
            pnci += 1;
        } else if pnc[pnci].0 <= list[pciv].1 {
            if pnc[pnci].1 > list[pciv].1 {
                list[pciv].1 = pnc[pnci].1;
            }
            pnci += 1;
        } else {
            pciv += 1;
        }
    }

    while pnci < pnc.len() {
        list.push(pnc[pnci]);
        pnci += 1;
    }

    // Try to reduce the number of intervals.
    pgsd.satdeg = 0;
    let mut apciv = 0;
    while apciv + 1 < list.len() {
        let next = list[apciv + 1];
        if list[apciv].1 < next.0 - 1 {
            pgsd.satdeg += list[apciv].1 - list[apciv].0 + 1;
            apciv += 1;
        } else {
            if list[apciv].1 < next.1 {
                list[apciv].1 = next.1;
            }
            list.remove(apciv + 1);
        }
    }
    pgsd.satdeg += list[apciv].1 - list[apciv].0 + 1;
}

/// `tcliqueColoring`: colors the positive weighted nodes of a given set of
/// nodes V with the lowest possible number of colors and finds a clique in
/// the graph induced by V, an upper bound and an apriori bound for further
/// branching steps; returns the maximal saturation degree (the upper bound).
#[allow(clippy::too_many_arguments)]
fn tclique_coloring(
    g: &dyn TcliqueGraph,
    v: &[i32],
    gsd: &mut [Nbc],
    iscolored: &mut [bool],
    apbound: &mut [TcliqueWeight],
    clique: &mut [i32],
    nclique: &mut usize,
    weightclique: &mut TcliqueWeight,
) -> TcliqueWeight {
    let weights = g.weights();
    let nv = v.len();

    // Initialize maximum weight clique found so far.
    let mut growclique = true;
    *nclique = 0;
    *weightclique = 0;

    // Get node of V with maximum weight.
    let mut nodevindex = get_max_weight_index(weights, v) as usize;
    let mut node = v[nodevindex];
    let mut range = weights[node as usize];

    // Set up data structures for coloring.
    for i in 0..nv {
        iscolored[i] = false;
        gsd[i] = Nbc::default();
    }
    iscolored[nodevindex] = true;

    // Set apriori bound: apbound(v_i) = satdeg(v_i) + weight(v_i).
    apbound[nodevindex] = range;

    // Update maximum saturation degree: maxsatdeg = max { satdeg(v_i) +
    // weight(v_i) | v_i in V }.
    let mut maxsatdegree = range;

    // Set neighbor color of the adjacent nodes of node.
    let vadj = g.select_adj_nodes(node, v);
    let mut adjidx = 0;
    let mut i = 0;
    while i < nv && adjidx < vadj.len() {
        if v[i] == vadj[adjidx] {
            // Node is adjacent to itself, but we do not need to color it again.
            if i == nodevindex {
                // Go to the next node in Vadj.
                adjidx += 1;
                i += 1;
                continue;
            }

            // Sets satdeg for adjacent node.
            gsd[i].satdeg = range;

            // Creates new color interval [1,range]; colorinterval is the first
            // added element of the list of neighborcolors of the adjacent node.
            gsd[i].lcitv = vec![(1, range)];

            // Go to the next node in Vadj.
            adjidx += 1;
        }
        i += 1;
    }

    // Set up data structures for the current clique (C swaps the two buffers;
    // the work clique is kept apart and copied to `clique` at the end).
    let mut currentclique: Vec<i32> = vec![0; nv];
    let mut workclique: Vec<i32> = vec![0; nv];

    // Add node to the current clique.
    currentclique[0] = node;
    let mut ncurrentclique = 1;
    let mut weightcurrentclique = range;

    // Color all other nodes of V.
    for _ in 0..nv.saturating_sub(1) {
        // Selects the next uncolored node to color.
        let idx = get_max_satdeg_index(v, gsd, iscolored, weights);
        if idx == -1 {
            // No uncolored nodes left.
            break;
        }
        nodevindex = idx as usize;

        node = v[nodevindex];
        range = weights[node as usize];
        iscolored[nodevindex] = true;

        // Set apriori bound: apbound(v_i) = satdeg(v_i) + weight(v_i).
        apbound[nodevindex] = gsd[nodevindex].satdeg + range;

        // Update maximum saturation degree.
        if maxsatdegree < apbound[nodevindex] {
            maxsatdegree = apbound[nodevindex];
        }

        // Update clique.
        if gsd[nodevindex].satdeg == 0 {
            // Current node is not adjacent to nodes of current clique, i.e. current
            // clique can not be increased.

            // Check, if weight of current clique is larger than weight of maximum
            // weight clique found so far.
            if weightcurrentclique > *weightclique {
                // Update maximum weight clique found so far.
                *weightclique = weightcurrentclique;
                *nclique = ncurrentclique;
                std::mem::swap(&mut workclique, &mut currentclique);
            }
            weightcurrentclique = 0;
            ncurrentclique = 0;
            growclique = true;
        }
        if growclique {
            // Check, if the current node is still adjacent to all nodes in the
            // clique.
            if gsd[nodevindex].satdeg == weightcurrentclique {
                currentclique[ncurrentclique] = node;
                ncurrentclique += 1;
                weightcurrentclique += range;
            } else {
                growclique = false;
            }
        }

        // Search for fitting color intervals for current node.
        let mut nwcitv: Vec<(i32, i32)> = Vec::new();
        if gsd[nodevindex].lcitv.is_empty() {
            // Current node has no colored neighbors yet: create new color interval
            // [1,range]; add the new colorinterval [1, range] to the list of
            // chosen colorintervals for node.
            nwcitv.push((1, range));
        } else {
            // Current node has colored neighbors.
            let mut tocolor = range;
            let lcitv = &gsd[nodevindex].lcitv;
            let mut l = 0;

            // Check, if first neighbor color interval [inf, sup] has inf > 1.
            if lcitv[l].0 != 1 {
                // Create new interval [1, min{range, inf}].
                let mut dif = lcitv[l].0 - 1;
                if dif > tocolor {
                    dif = tocolor;
                }

                nwcitv.push((1, dif));
                tocolor -= dif;
            }

            // As long as node is not colored with all colors, create new color
            // interval by filling the gaps in the existing neighbor color
            // intervals of the neighbors of node.
            while tocolor > 0 {
                let mut dif = tocolor;

                let inf = lcitv[l].1 + 1;
                if l + 1 < lcitv.len() {
                    let min = lcitv[l + 1].0 - lcitv[l].1 - 1;

                    if dif > min {
                        dif = min;
                    }
                    l += 1;
                }

                let sup = inf + dif - 1;
                tocolor -= dif;
                nwcitv.push((inf, sup));
            }
        }

        // Update saturation degree and neighbor colorintervals of all neighbors
        // of node.
        let vadj = g.select_adj_nodes(node, v);
        let mut adjidx = 0;
        let mut j = 0;
        while j < nv && adjidx < vadj.len() {
            if v[j] == vadj[adjidx] {
                if !iscolored[j] {
                    update_neighbor(&mut gsd[j], &nwcitv);
                }

                // Go to the next node in Vadj.
                adjidx += 1;
            }
            j += 1;
        }

        // Free data structure of neighbor colorinterval of node just colored.
        gsd[nodevindex].lcitv.clear();
    }

    // Update maximum weight clique found so far.
    if weightcurrentclique > *weightclique {
        *weightclique = weightcurrentclique;
        *nclique = ncurrentclique;
        std::mem::swap(&mut workclique, &mut currentclique);
    }

    // Move the found clique to the provided clique pointer.
    clique[..*nclique].copy_from_slice(&workclique[..*nclique]);

    maxsatdegree
}

/*
 * Clique search (tclique_branch.c)
 */

/// The search state `branch` passes down the tree.
struct Search<'a, 'g> {
    g: &'g dyn TcliqueGraph,
    newsol: Option<&'a mut NewSol<'a>>,
    cliquehash: Option<CliqueHash>,
    vzero: Vec<i32>,
    gsd: Vec<Nbc>,
    iscolored: Vec<bool>,
    k: Vec<i32>,
    maxcliquenodes: Vec<i32>,
    nmaxcliquenodes: usize,
    maxcliqueweight: TcliqueWeight,
    curcliquenodes: Vec<i32>,
    ncurcliquenodes: usize,
    curcliqueweight: TcliqueWeight,
    tmpcliquenodes: Vec<i32>,
    maxfirstnodeweight: TcliqueWeight,
    ntreenodes: i32,
    maxntreenodes: i32,
    backtrackfreq: i32,
    maxnzeroextensions: i32,
    status: TcliqueStatus,
}

impl Search<'_, '_> {
    /// `extendCliqueZeroWeight`: extends given clique by additional
    /// zero-weight nodes of the given node set.
    fn extend_clique_zero_weight(&mut self) {
        if self.maxnzeroextensions == 0 {
            return;
        }

        // Initialize the zero-weighted candidates for clique extension.
        let mut zerocands = self.vzero.clone();

        // For each node in the clique, remove non-adjacent nodes from the zero
        // extension candidates.
        let mut i = 0;
        while i < self.ncurcliquenodes && !zerocands.is_empty() {
            zerocands = self.g.select_adj_nodes(self.curcliquenodes[i], &zerocands);
            i += 1;
        }

        // Put zero-weighted candidates into the clique, and remove non-adjacent
        // nodes from the candidate set.
        let mut nzeroextensions = 0;
        let mut start = 0;
        while start < zerocands.len() {
            // Put first candidate into the clique.
            self.curcliquenodes[self.ncurcliquenodes] = zerocands[start];
            self.ncurcliquenodes += 1;
            start += 1;
            nzeroextensions += 1;

            if nzeroextensions >= self.maxnzeroextensions {
                break;
            }

            // Remove candidates that are not adjacent to the inserted
            // zero-weighted node.
            let rest = self.g.select_adj_nodes(self.curcliquenodes[self.ncurcliquenodes - 1], &zerocands[start..]);
            zerocands = rest;
            start = 0;
        }
    }

    /// `newSolution`: calls user callback after a new solution was found,
    /// that is better than the current incumbent; returns whether solving
    /// should be stopped.
    fn new_solution(&mut self) -> bool {
        let mut acceptsol = true;
        let mut stopsolving = false;
        let mut clique: Option<Clique> = None;
        let mut insertpos = 0;

        if self.newsol.is_some() {
            // Check whether the clique is already stored in the table.
            let hash = self.cliquehash.as_ref().unwrap();
            if !hash.cliques.is_empty() {
                let c = create_clique(&self.curcliquenodes[..self.ncurcliquenodes]);
                let (found, pos) = hash.contains(&c);
                acceptsol = !found;
                insertpos = pos;
                clique = Some(c);
            }
        }

        // Check, if this is a new clique.
        if acceptsol {
            // Extend the clique with the zero-weighted nodes.
            self.extend_clique_zero_weight();

            if let Some(newsol) = self.newsol.as_deref_mut() {
                // Call user callback method.
                newsol(
                    &self.curcliquenodes[..self.ncurcliquenodes],
                    self.curcliqueweight,
                    &mut self.maxcliqueweight,
                    &mut acceptsol,
                    &mut stopsolving,
                );

                // If clique was accepted, clear the clique hash table; otherwise,
                // insert it into the clique hash table, such that the same or a weaker
                // clique is not presented to the user again.
                let hash = self.cliquehash.as_mut().unwrap();
                if acceptsol {
                    hash.cliques.clear();
                } else {
                    // If the clique was not yet created, do it now.
                    let c = clique.take().unwrap_or_else(|| create_clique(&self.curcliquenodes[..self.ncurcliquenodes]));

                    // Insert clique into clique hash table.
                    hash.cliques.insert(insertpos, c);
                }
            }
        }

        if acceptsol {
            // Copy the solution to the incumbent.
            let n = self.ncurcliquenodes;
            self.maxcliquenodes[..n].copy_from_slice(&self.curcliquenodes[..n]);
            self.nmaxcliquenodes = n;
            if self.curcliqueweight > self.maxcliqueweight {
                self.maxcliqueweight = self.curcliqueweight;
            }
        }

        stopsolving
    }

    /// `reduced`: tries to find a clique, if V has only one or two nodes.
    fn reduced(&mut self, v: &[i32], apbound: &mut [TcliqueWeight], ntmpcliquenodes: &mut usize, tmpcliqueweight: &mut TcliqueWeight) {
        let weights = self.g.weights();
        let nv = v.len();

        if nv >= 1 {
            apbound[0] = weights[v[0] as usize];
        }
        if nv >= 2 {
            apbound[1] = weights[v[1] as usize];
        }

        // Check if nodes are adjacent.
        if nv >= 2 && self.g.is_edge(v[0], v[1]) {
            // Put nodes into clique.
            self.tmpcliquenodes[0] = v[0];
            self.tmpcliquenodes[1] = v[1];
            *ntmpcliquenodes = 2;
            *tmpcliqueweight = weights[v[0] as usize] + weights[v[1] as usize];
            apbound[0] += weights[v[1] as usize];
        } else if nv >= 2 && weights[v[1] as usize] > weights[v[0] as usize] {
            // Put V[1] into clique.
            self.tmpcliquenodes[0] = v[1];
            *ntmpcliquenodes = 1;
            *tmpcliqueweight = weights[v[1] as usize];
        } else if nv >= 1 {
            // Put V[0] into clique.
            self.tmpcliquenodes[0] = v[0];
            *ntmpcliquenodes = 1;
            *tmpcliqueweight = weights[v[0] as usize];
        } else {
            *tmpcliqueweight = 0;
            *ntmpcliquenodes = 0;
        }
    }

    /// `boundSubgraph`: calculates upper bound on remaining subgraph, and
    /// heuristically generates a clique.
    fn bound_subgraph(&mut self, v: &[i32], apbound: &mut [TcliqueWeight], ntmpcliquenodes: &mut usize, tmpcliqueweight: &mut TcliqueWeight) -> TcliqueWeight {
        // Check if we are in an easy case with at most 2 nodes left.
        if v.len() <= 2 {
            // Get 1- or 2-clique and bounds without coloring.
            self.reduced(v, apbound, ntmpcliquenodes, tmpcliqueweight);
            *tmpcliqueweight
        } else {
            // Color the graph induces by nodes of V to get an upper bound for the
            // remaining subgraph.
            tclique_coloring(
                self.g,
                v,
                &mut self.gsd,
                &mut self.iscolored,
                apbound,
                &mut self.tmpcliquenodes,
                ntmpcliquenodes,
                tmpcliqueweight,
            )
        }
    }

    /// `branch`: branches the searching tree, branching nodes are selected in
    /// decreasing order of their apriori bound, returns the level to which
    /// we should backtrack, or INT_MAX for continuing normally.
    fn branch(&mut self, mut level: usize, v: &mut [i32], weightk: TcliqueWeight, fixednode: i32) -> i32 {
        let nv = v.len();

        // Increase the number of nodes, and stop solving, if the node limit is
        // exceeded.
        self.ntreenodes += 1;
        if self.ntreenodes > self.maxntreenodes {
            self.status = TcliqueStatus::NodeLimit;
            return 1;
        }

        let mut backtracklevel = i32::MAX;
        let mut isleaf = true;

        // Allocate temporary memory for a priori bounds.
        let mut apbound: Vec<TcliqueWeight> = vec![0; nv];

        // Use coloring relaxation to generate an upper bound for the current
        // subtree and a heuristic solution.
        let mut ntmpcliquenodes = 0;
        let mut tmpcliqueweight = 0;
        let subgraphweight = self.bound_subgraph(v, &mut apbound, &mut ntmpcliquenodes, &mut tmpcliqueweight);

        // Check, whether the heuristic solution is better than the current
        // subtree's solution; if the user requested a fixed node, we can only
        // use the heuristic solution if it includes the fixed node.
        if weightk + tmpcliqueweight > self.curcliqueweight && (level > 0 || fixednode == -1) {
            // Install the newly generated clique as current clique.
            for i in 0..level {
                self.curcliquenodes[i] = self.k[i];
            }
            for i in 0..ntmpcliquenodes {
                self.curcliquenodes[level + i] = self.tmpcliquenodes[i];
            }
            self.ncurcliquenodes = level + ntmpcliquenodes;
            self.curcliqueweight = weightk + tmpcliqueweight;
        }

        // Discard subtree, if the upper bound is not better than the weight of
        // the currently best clique; if only 2 nodes are left, the maximal
        // weighted clique was already calculated in boundSubgraph and nothing
        // more has to be done; however, if the user requested a fixed node, we
        // have to continue with the branching.
        if weightk + subgraphweight > self.maxcliqueweight && (nv > 2 || (fixednode >= 0 && level == 0)) {
            let weights = self.g.weights();

            // Process current subtree.
            level += 1;

            // Set up data structures.
            let mut nvalive = nv;
            let weightkold = weightk;

            // Branch on the nodes of V by decreasing order of their apriori bound.
            while backtracklevel >= level as i32 && nvalive > 0 {
                // Check if we meet the backtracking frequency - in this case abort the
                // search until we have reached first level.
                if level > 1 && self.backtrackfreq > 0 && self.ntreenodes % self.backtrackfreq == 0 {
                    backtracklevel = 1;
                    break;
                }

                // Get next branching node.
                let branchidx: i32 = if level == 1 && fixednode >= 0 {
                    // Select the fixed node as first "branching" candidate.
                    let mut b = 0;
                    while b < nvalive && v[b] != fixednode {
                        b += 1;
                    }
                    b as i32
                } else if level == 1 && self.maxfirstnodeweight > 0 {
                    get_max_ap_bound_index_not_max_weight(&v[..nvalive], &apbound[..nvalive], weights, self.maxfirstnodeweight)
                } else {
                    get_max_ap_bound_index(&apbound[..nvalive])
                };
                if branchidx < 0 {
                    break;
                }
                let branchidx = branchidx as usize;

                // Test a priori bound.
                if weightkold + apbound[branchidx] <= self.maxcliqueweight {
                    break;
                }

                // Because we branch on this node, the node is no leaf in the tree.
                isleaf = false;

                // Update the set of nodes from the b&b tree K = K & {branchingnode}.
                let branchingnode = v[branchidx];
                self.k[level - 1] = branchingnode;
                let weightk = weightkold + weights[branchingnode as usize];

                // Update the set of nodes for branching V = V \ {branchingnode}.
                nvalive -= 1;
                for i in branchidx..nvalive {
                    v[i] = v[i + 1];
                    apbound[i] = apbound[i + 1];
                }

                // Set the nodes for the next level of b&b tree; Vcurrent =
                // nodes of V, that are adjacent to branchingnode.
                let mut vcurrent = self.g.select_adj_nodes(branchingnode, &v[..nvalive]);
                let nvcurrent = vcurrent.len();

                // Process the selected subtree.
                backtracklevel = self.branch(level, &mut vcurrent, weightk, -1);

                // If all other candidates stayed in the candidate list, the current
                // branching was optimal and there is no need to try the remaining
                // ones.
                if nvcurrent == nvalive {
                    nvalive = 0;
                }

                // If we had a fixed node, ignore all other nodes.
                if fixednode >= 0 {
                    nvalive = 0;
                }
            }
        }

        // Check, whether any branchings have been applied, or if this node is a
        // leaf of the branching tree.
        if isleaf {
            // The current clique is the best clique found on the path to this
            // leaf -> check, whether it is an improvement to the currently best
            // clique.
            if self.curcliqueweight > self.maxcliqueweight {
                let stopsolving = self.new_solution();
                if stopsolving {
                    backtracklevel = 0;
                }
            }

            // Discard the current clique.
            self.ncurcliquenodes = 0;
            self.curcliqueweight = 0;
        }

        backtracklevel
    }
}

/// `getMaxApBoundIndex`: gets the index of the node of V with the maximum
/// apriori bound; returns -1, if no node exists.
fn get_max_ap_bound_index(apbound: &[TcliqueWeight]) -> i32 {
    let mut maxapbound = 0;
    let mut maxindex = -1;

    for (i, &b) in apbound.iter().enumerate() {
        if b >= maxapbound {
            maxapbound = b;
            maxindex = i as i32;
        }
    }

    maxindex
}

/// `getMaxApBoundIndexNotMaxWeight`: gets the index of the node of V with
/// the maximum apriori bound, but ignores nodes with weights larger than the
/// given maximal weight; returns -1 if no node with weight smaller or equal
/// than maxweight is found.
fn get_max_ap_bound_index_not_max_weight(v: &[i32], apbound: &[TcliqueWeight], weights: &[TcliqueWeight], maxweight: TcliqueWeight) -> i32 {
    let mut maxapbound = 0;
    let mut maxindex = -1;

    for i in 0..v.len() {
        if apbound[i] >= maxapbound && weights[v[i] as usize] <= maxweight {
            maxapbound = apbound[i];
            maxindex = i as i32;
        }
    }

    maxindex
}

/// The result of [`tclique_max_clique`].
pub struct MaxClique {
    pub nodes: Vec<i32>,
    pub weight: TcliqueWeight,
    pub ntreenodes: i32,
    pub status: TcliqueStatus,
}

/// `tcliqueMaxClique`: finds maximum weight clique.
#[allow(clippy::too_many_arguments)]
pub fn tclique_max_clique<'a>(
    g: &dyn TcliqueGraph,
    newsol: Option<&'a mut NewSol<'a>>,
    maxfirstnodeweight: TcliqueWeight,
    minweight: TcliqueWeight,
    maxntreenodes: i32,
    backtrackfreq: i32,
    maxnzeroextensions: i32,
    fixednode: i32,
) -> MaxClique {
    // Get number of nodes.
    let nnodes = g.nnodes();

    // Set up data structures.
    let cliquehash = newsol.as_ref().map(|_| CliqueHash {
        cliques: Vec::with_capacity(CLIQUEHASH_INITSIZE),
    });

    // Set up V and Vzero.
    let weights = g.weights();
    let mut v: Vec<i32> = Vec::with_capacity(nnodes);
    let mut vzero: Vec<i32> = Vec::with_capacity(nnodes);
    for i in 0..nnodes {
        if weights[i] == 0 {
            vzero.push(i as i32);
        } else {
            v.push(i as i32);
        }
    }

    let mut s = Search {
        g,
        newsol,
        cliquehash,
        vzero,
        gsd: vec![Nbc::default(); nnodes],
        iscolored: vec![false; nnodes],
        k: vec![0; nnodes],
        maxcliquenodes: vec![0; nnodes],
        nmaxcliquenodes: 0,
        // Set weight and number of nodes of maximum weighted clique.
        maxcliqueweight: minweight - 1,
        curcliquenodes: vec![0; nnodes],
        ncurcliquenodes: 0,
        curcliqueweight: 0,
        tmpcliquenodes: vec![0; nnodes],
        maxfirstnodeweight,
        ntreenodes: 0,
        maxntreenodes,
        backtrackfreq,
        maxnzeroextensions,
        status: TcliqueStatus::Optimal,
    };

    // Branch to find maximum weight clique.
    let backtracklevel = s.branch(0, &mut v, 0, fixednode);

    if backtracklevel != i32::MAX && s.status == TcliqueStatus::Optimal {
        s.status = TcliqueStatus::UserAbort;
    }

    MaxClique {
        nodes: s.maxcliquenodes[..s.nmaxcliquenodes].to_vec(),
        weight: s.maxcliqueweight,
        ntreenodes: s.ntreenodes,
        status: s.status,
    }
}
