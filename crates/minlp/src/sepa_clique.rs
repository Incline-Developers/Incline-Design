//! Port of `sepa_clique.c`: clique separator of the stable set relaxation,
//! searching maximum weight cliques of the clique table's graph with
//! [`crate::tclique`].

use std::any::Any;

use crate::{
    api,
    def::{REAL_MAX, max},
    paramset::{paramset_add_int, paramset_add_real},
    retcode::ScipResult,
    scip::Scip,
    scip_lp,
    tclique::{TcliqueGraph, TcliqueWeight, tclique_max_clique},
    types::{ScipResultCode, SepaId, SolId, VarId, VarType},
    var::NONE,
};

const SEPA_NAME: &str = "clique";
const SEPA_DESC: &str = "clique separator of stable set relaxation";
const SEPA_PRIORITY: i32 = -5000;
const SEPA_FREQ: i32 = 0;
const SEPA_MAXBOUNDDIST: f64 = 0.0;
/// Does the separator use a secondary SCIP instance?
const SEPA_USESSUBSCIP: bool = false;
/// Should separation method be delayed, if other separators found cuts?
const SEPA_DELAY: bool = false;

/// Factor for scaling weights.
const DEFAULT_SCALEVAL: f64 = 1000.0;
/// Maximal number of nodes in branch and bound tree (-1: no limit).
const DEFAULT_MAXTREENODES: i32 = 10000;
/// Frequency for premature backtracking up to tree level 1 (0: no
/// backtracking).
const DEFAULT_BACKTRACKFREQ: i32 = 1000;
/// Maximal number of clique cuts separated per separation round (-1: no
/// limit).
const DEFAULT_MAXSEPACUTS: i32 = 10;
/// Maximal number of zero-valued variables extending the clique (-1: no
/// limit).
const DEFAULT_MAXZEROEXTENSIONS: i32 = 1000;
/// Maximal memory size of dense clique table (in kb).
const DEFAULT_CLIQUETABLEMEM: f64 = 20000.0;
/// Minimal density of cliques to use a dense clique table.
const DEFAULT_CLIQUEDENSITY: f64 = 0.00;

/// Separator data.
#[derive(Default)]
struct SepaData {
    /// Tclique graph data structure (`None` for C's NULL).
    tcliquegraph: Option<CliqueGraph>,
    /// Factor for scaling weights.
    scaleval: f64,
    /// Number of calls to the clique separator.
    ncalls: i64,
    maxtreenodes: i32,
    backtrackfreq: i32,
    maxsepacuts: i32,
    maxzeroextensions: i32,
    cliquetablemem: f64,
    cliquedensity: f64,
    /// Number of cuts found.
    ncuts: i32,
    /// TRUE if tcliquegraph is already loaded (tcliquegraph can be NULL),
    /// FALSE otherwise.
    tcliquegraphloaded: bool,
    /// Whether the clique algorithm detected a cutoff.
    cutoff: bool,
}

fn data(scip: &Scip, sepa: SepaId) -> &SepaData {
    scip.sepas[sepa].sepadata.as_ref().unwrap().downcast_ref::<SepaData>().unwrap()
}

fn data_mut(scip: &mut Scip, sepa: SepaId) -> &mut SepaData {
    scip.sepas[sepa].sepadata.as_mut().unwrap().downcast_mut::<SepaData>().unwrap()
}

/// `TCLIQUE_GRAPH`: tclique graph data.
#[derive(Default)]
struct CliqueGraph {
    /// Active problem variables (or negated variables) the nodes belong to.
    vars: Vec<VarId>,
    /// Weight of nodes.
    weights: Vec<TcliqueWeight>,
    /// Indices in adjnodes array of first adjacent nodes for each node.
    adjnodesidxs: Vec<i32>,
    /// Indices in cliqueids array of first clique the node is contained in.
    cliqueidsidxs: Vec<i32>,
    /// Adjacent nodes of edges (never filled, as in C).
    adjnodes: Vec<i32>,
    /// Unique ids of cliques.
    cliqueids: Vec<u32>,
    /// Dense bitvector clique table (array stored as a vector).
    cliquetable: Option<Vec<u32>>,
    /// Number of unsigned ints per row in the table.
    tablewidth: i32,
}

impl CliqueGraph {
    /// `tcliquegraphCreate`: creates an empty tclique graph data structure.
    fn new() -> Self {
        Self {
            adjnodesidxs: vec![0],
            cliqueidsidxs: vec![0],
            ..Default::default()
        }
    }

    fn nnodes(&self) -> usize {
        self.vars.len()
    }

    /// `nodesHaveCommonClique`: returns whether the nodes are member of a
    /// common clique.
    fn nodes_have_common_clique(&self, node1: i32, node2: i32) -> bool {
        // Return TRUE for equal nodes.
        if node1 == node2 {
            return true;
        }

        // Check whether the dense clique table was constructed.
        if let Some(cliquetable) = self.cliquetable.as_ref() {
            // Check entry in the table.
            let nbits = 32;
            let mask = 1u32 << (node2 % nbits);
            let colofs = node2 / nbits;
            (cliquetable[(node1 * self.tablewidth + colofs) as usize] & mask) != 0
        } else {
            let cliqueids = &self.cliqueids;
            let mut i1 = self.cliqueidsidxs[node1 as usize] as usize;
            let endi1 = self.cliqueidsidxs[node1 as usize + 1] as usize;
            let mut i2 = self.cliqueidsidxs[node2 as usize] as usize;
            let endi2 = self.cliqueidsidxs[node2 as usize + 1] as usize;
            while i1 < endi1 && i2 < endi2 {
                while i1 < endi1 && cliqueids[i1] < cliqueids[i2] {
                    i1 += 1;
                }
                if i1 == endi1 {
                    break;
                }

                while i2 < endi2 && cliqueids[i2] < cliqueids[i1] {
                    i2 += 1;
                }
                if i2 == endi2 {
                    break;
                }

                if cliqueids[i1] == cliqueids[i2] {
                    return true;
                }
            }

            false
        }
    }
}

impl TcliqueGraph for CliqueGraph {
    /// `tcliqueGetnnodesClique`.
    fn nnodes(&self) -> usize {
        self.vars.len()
    }

    /// `tcliqueGetweightsClique`.
    fn weights(&self) -> &[TcliqueWeight] {
        &self.weights
    }

    /// `tcliqueIsedgeClique`: returns whether the edge (node1, node2) is in
    /// the graph.
    fn is_edge(&self, node1: i32, node2: i32) -> bool {
        // Check if node2 is contained in adjacency list of node1 (list is
        // ordered by adjacent nodes).
        let mut left = self.adjnodesidxs[node1 as usize];
        let mut right = self.adjnodesidxs[node1 as usize + 1] - 1;
        while left <= right {
            let middle = (left + right) / 2;
            let node = self.adjnodes[middle as usize];
            if node < node2 {
                left = middle + 1;
            } else if node > node2 {
                right = middle - 1;
            } else {
                return true;
            }
        }

        // Check if the nodes are member of a common clique.
        self.nodes_have_common_clique(node1, node2)
    }

    /// `tcliqueSelectadjnodesClique`: selects all nodes from a given set of
    /// nodes which are adjacent to a given node.
    fn select_adj_nodes(&self, node: i32, nodes: &[i32]) -> Vec<i32> {
        let mut adjnodes = Vec::with_capacity(nodes.len());

        // Check for each node in given nodes set, if it is adjacent to the given
        // node or shares a common clique.
        let mut nodeadjindex = self.adjnodesidxs[node as usize] as usize;
        let nodeadjend = self.adjnodesidxs[node as usize + 1] as usize;
        for &n in nodes {
            // Check if the node is adjacent to the given node (nodes and adjacent
            // nodes are ordered by node index).
            while nodeadjindex < nodeadjend && self.adjnodes[nodeadjindex] < n {
                nodeadjindex += 1;
            }
            if nodeadjindex < nodeadjend && self.adjnodes[nodeadjindex] == n {
                // Current node is adjacent to given node.
                adjnodes.push(n);
            } else {
                // Current node is not adjacent to given node: check if they share a
                // common clique.
                if self.nodes_have_common_clique(node, n) {
                    adjnodes.push(n);
                }
            }
        }

        adjnodes
    }
}

/// The cliques `var == value` is in.
fn var_cliques(scip: &Scip, var: VarId, value: bool) -> Vec<usize> {
    match scip.vars[var].cliquelist.as_ref() {
        Some(l) => l.cliques[value as usize].0[..l.ncliques[value as usize] as usize].to_vec(),
        None => Vec::new(),
    }
}

/// `tcliquegraphAddNode`: adds a node to the tclique graph defined as a
/// variable-value pair; adds all cliques to the cliqueids array the
/// variable is contained in with the given value.
fn tcliquegraph_add_node(scip: &mut Scip, tcliquegraph: &mut Option<CliqueGraph>, var: VarId, value: bool) -> ScipResult {
    // Create tclique graph data if not yet existing.
    if tcliquegraph.is_none() {
        *tcliquegraph = Some(CliqueGraph::new());
    }

    // If the value is FALSE, use the negated variable for the node.
    let nodevar = if !value { crate::scip_var::get_negated_var(scip, var)? } else { var };

    let g = tcliquegraph.as_mut().unwrap();

    // Get the current number of used entries in adjnodes and cliqueids arrays.
    let nnodes = g.nnodes();
    let nadjnodes = g.adjnodesidxs[nnodes];

    // Insert the variable into the tclique graph.
    crate::var::var_capture(scip, nodevar);
    g.vars.push(nodevar);
    g.weights.push(0);

    // Store the ids of the variable's cliques in the cliqueids array.
    for c in var_cliques(scip, var, value) {
        g.cliqueids.push(scip.cliques[c].id);
    }

    // Store the new number of used entries in adjnodes and cliqueids arrays.
    g.adjnodesidxs.push(nadjnodes);
    g.cliqueidsidxs.push(g.cliqueids.len() as i32);

    Ok(())
}

/// `tcliquegraphAddCliqueVars`: adds all variable/value pairs to the
/// tclique graph that are contained in an existing 3-clique.
fn tcliquegraph_add_clique_vars(scip: &mut Scip, tcliquegraph: &mut Option<CliqueGraph>) -> ScipResult {
    // Get binary problem variables.
    let nvars = api::get_n_bin_vars(scip) as usize;
    let vars = api::get_vars(scip)[..nvars].to_vec();

    for var in vars {
        for value in [false, true] {
            if !var_cliques(scip, var, value).is_empty() {
                // All cliques stored in the clique table are at least 3-cliques.
                tcliquegraph_add_node(scip, tcliquegraph, var, value)?;
            }
        }
    }

    Ok(())
}

/// `tcliquegraphConstructCliqueTable`: constructs dense clique incidence
/// matrix.
fn tcliquegraph_construct_clique_table(scip: &mut Scip, g: &mut CliqueGraph, cliquetablemem: f64, cliquedensity: f64) {
    let cliques = {
        let t = scip.cliquetable.as_ref().unwrap();
        t.cliques.0[..t.ncliques as usize].to_vec()
    };
    if cliques.is_empty() {
        return;
    }

    // Calculate size of dense clique table.
    let nbits = 32;
    let nnodes = g.nnodes() as i32;
    // Number of ints needed.
    g.tablewidth = (nnodes + nbits - 1) / nbits;

    // Check if dense clique table is too large (calculate as Reals to avoid
    // overflow).
    if nnodes as f64 * g.tablewidth as f64 / 1024.0 > cliquetablemem {
        return;
    }

    // Calculate clique entry density.
    let mut nelems = 0;
    for &c in &cliques {
        nelems += scip.cliques[c].nvars;
    }
    let density = nelems as f64 / (cliques.len() as f64 * nnodes as f64);
    if density < cliquedensity {
        return;
    }

    // Allocate memory.
    let tablewidth = g.tablewidth;
    let tablesize = (nnodes * tablewidth) as usize;
    let mut cliquetable = vec![0u32; tablesize];

    // Insert the cliques as complete graphs to the incidence matrix.
    let mut varids = vec![0i32; nnodes as usize];
    let not_in_graph = |scip: &Scip, v: VarId| scip.vars[v].vartype != VarType::Binary || crate::var::var_is_implied_integral(scip, v);
    let mut i = 0;
    while i < cliques.len() && !crate::solve::solve_is_stopped(scip, false) {
        let c = &scip.cliques[cliques[i]];
        let nvars = c.nvars as usize;
        let vars = c.vars.0[..nvars].to_vec();
        let vals = c.values.0[..nvars].to_vec();

        // Get the node numbers of the variables.
        let mut u = 0;
        while u < nvars && !crate::solve::solve_is_stopped(scip, false) {
            // Implicit integer and integer variables are currently not present in
            // the constructed tclique graph.
            if not_in_graph(scip, vars[u]) {
                u += 1;
                continue;
            }

            let var = if vals[u] { vars[u] } else { scip.vars[vars[u]].negatedvar };
            let mut v = 0;
            while (v as usize) < g.vars.len() && var != g.vars[v as usize] {
                v += 1;
            }
            varids[u] = v;
            u += 1;
        }

        // Flag the edges in the incidence matrix (excluding diagonal entries).
        let mut u = 0;
        while u + 1 < nvars && !crate::solve::solve_is_stopped(scip, false) {
            // Implicit integer and integer variables are currently not present in
            // the constructed tclique graph.
            if not_in_graph(scip, vars[u]) {
                u += 1;
                continue;
            }

            let nu = varids[u];
            let rowstart = nu * tablewidth;
            let colofs = nu / nbits;
            let colmask = 1u32 << (nu % nbits);
            for v in u + 1..nvars {
                // Implicit integer and integer variables are currently not present in
                // the constructed tclique graph.
                if not_in_graph(scip, vars[v]) {
                    continue;
                }

                let nv = varids[v];
                let mask = 1u32 << (nv % nbits);
                cliquetable[(rowstart + nv / nbits) as usize] |= mask;
                cliquetable[(nv * tablewidth + colofs) as usize] |= colmask;
            }
            u += 1;
        }
        i += 1;
    }

    g.cliquetable = Some(cliquetable);
}

/// `tcliquegraphFree`: frees the tclique graph data structure and releases
/// all captured variables.
fn tcliquegraph_free(scip: &mut Scip, g: CliqueGraph) -> ScipResult {
    // Release variables.
    for mut v in g.vars {
        api::release_var(scip, &mut v)?;
    }
    Ok(())
}

/// `loadTcliquegraph`: creates tclique data structure using the implication
/// graph; only variables that are contained in a 3-clique are added as
/// nodes to the clique graph.
fn load_tcliquegraph(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    // There is nothing to do, if no binary variables are present in the
    // problem.
    if api::get_n_bin_vars(scip) == 0 {
        return Ok(());
    }

    // Insert all variable/value pairs that are contained in an existing
    // 3-clique.
    let mut graph = data_mut(scip, sepa).tcliquegraph.take();
    tcliquegraph_add_clique_vars(scip, &mut graph)?;

    // It occurs that it might be that some cliques were not yet removed from
    // the global clique array, so SCIPgetNClique(scip) can be larger than 0,
    // even if no cliques of size at least 3 exist.
    let (cliquetablemem, cliquedensity) = {
        let d = data(scip, sepa);
        (d.cliquetablemem, d.cliquedensity)
    };
    if let Some(g) = graph.as_mut() {
        // Construct the dense clique table.
        tcliquegraph_construct_clique_table(scip, g, cliquetablemem, cliquedensity);
    }

    if crate::solve::solve_is_stopped(scip, false)
        && let Some(g) = graph.take()
    {
        tcliquegraph_free(scip, g)?;
    }
    data_mut(scip, sepa).tcliquegraph = graph;

    Ok(())
}

/// `separateCuts`: searches and adds clique cuts that separate the given
/// primal solution.
fn separate_cuts(scip: &mut Scip, sepa: SepaId, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    // Get clique table.
    let infeasible = crate::scip_var::cleanup_cliques(scip)?;
    if infeasible {
        return Ok(());
    }

    // Get separator data.
    let ncalls = scip.sepas[sepa].ncalls;
    {
        let d = data_mut(scip, sepa);
        d.ncalls = ncalls;
        d.cutoff = false;
        d.ncuts = 0;
    }

    // If we already detected that no implications between binary variables
    // exist, nothing has to be done.
    let d = data(scip, sepa);
    if d.tcliquegraph.is_none() && d.tcliquegraphloaded {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    // Load tclique data structure.
    if !data(scip, sepa).tcliquegraphloaded {
        load_tcliquegraph(scip, sepa)?;
        data_mut(scip, sepa).tcliquegraphloaded = true;

        if data(scip, sepa).tcliquegraph.is_none() {
            if crate::solve::solve_is_stopped(scip, false) {
                data_mut(scip, sepa).tcliquegraphloaded = false;
            }
            // We did not find any variables that are contained in a clique with at
            // least 3 variables in the implication graph or in the clique table ->
            // nothing has to be done.
            return Ok(());
        }
    }

    let mut g = data_mut(scip, sepa).tcliquegraph.take().unwrap();

    // Store LP-solution in sepadata and update weights in tclique data
    // structure.
    let mut varsolvals = vec![0.0; g.nnodes()];
    api::get_sol_vals(scip, sol, &g.vars, &mut varsolvals);

    // `updateTcliquegraph`: updates weight of all nodes in tclique data
    // structure.
    let d = data(scip, sepa);
    let scaleval = d.scaleval;
    for i in 0..g.nnodes() {
        let weight = scip.set.feas_floor(varsolvals[i] * scaleval) as TcliqueWeight;
        g.weights[i] = max(weight, 0);
    }

    // Get maximal number of tree nodes and maximal zero-extensions.
    let maxtreenodes = if d.maxtreenodes == -1 { i32::MAX } else { d.maxtreenodes };
    let maxzeroextensions = if d.maxzeroextensions == -1 { i32::MAX } else { d.maxzeroextensions };
    let (backtrackfreq, maxsepacuts, ncallsd) = (d.backtrackfreq, d.maxsepacuts, d.ncalls);

    // Finds maximum weight clique in tclique.
    let mut ncuts = 0;
    let mut retcode: ScipResult = Ok(());
    {
        let vars = g.vars.clone();
        let scip_ref: &mut Scip = scip;
        // `tcliqueNewsolClique`: generates cuts using a clique found by
        // algorithm for maximum weight clique and decides whether to stop
        // generating cliques with the algorithm for maximal weight clique.
        let mut newsol = |cliquenodes: &[i32], cliqueweight: TcliqueWeight, minweight: &mut TcliqueWeight, acceptsol: &mut bool, stopsolving: &mut bool| {
            // We don't accept the solution as new incumbent, because we want to
            // find many violated clique inequalities.
            *acceptsol = false;
            *stopsolving = false;

            // Slightly increase the minimal weight for additional cliques.
            let mut minweightinc = (cliqueweight - *minweight) / 10;
            minweightinc = max(minweightinc, 1);
            *minweight += minweightinc;

            // Adds cut if weight of the clique is greater than 1.
            if cliqueweight as f64 > scaleval {
                // Calculate the weight of the clique in unscaled fractional variable
                // space.
                let mut unscaledweight = 0.0;
                for &n in cliquenodes {
                    unscaledweight += varsolvals[n as usize];
                }

                if api::is_efficacious(scip_ref, unscaledweight - 1.0) {
                    // Explicitly handle return code.
                    let r = newsol_clique_add_row(scip_ref, sepa, ncallsd, ncuts, &vars, cliquenodes);
                    if r.is_ok() {
                        ncuts += 1;

                        // If we found more than half the cuts we are allowed to generate,
                        // we accept the clique as new incumbent, such that only more
                        // violated cuts are generated afterwards.
                        if maxsepacuts >= 0 {
                            if ncuts > maxsepacuts / 2 {
                                *acceptsol = true;
                            }
                            if ncuts >= maxsepacuts {
                                *stopsolving = true;
                            }
                        }
                    } else {
                        // In case an internal SCIP error occurred we stop the algorithm and
                        // store the error code for later evaluation.
                        retcode = r;
                        *stopsolving = true;
                    }
                }
            }
        };
        tclique_max_clique(
            &g,
            Some(&mut newsol),
            scaleval as i32 - 1,
            scaleval as i32 + 1,
            maxtreenodes,
            backtrackfreq,
            maxzeroextensions,
            -1,
        );
    }

    data_mut(scip, sepa).tcliquegraph = Some(g);
    data_mut(scip, sepa).ncuts = ncuts;

    // In case an internal error occurred during the maximal clique
    // computation, evaluate that one.
    retcode?;

    // Adjust result code.
    if data(scip, sepa).cutoff {
        *result = ScipResultCode::Cutoff;
    } else if ncuts > 0 {
        *result = ScipResultCode::Separated;
    }

    Ok(())
}

/// `newsolCliqueAddRow`: basic code for new cliques.
fn newsol_clique_add_row(scip: &mut Scip, sepa: SepaId, ncalls: i64, ncuts: i32, vars: &[VarId], cliquenodes: &[i32]) -> ScipResult {
    // Create the cut.
    let cutname = format!("clique{ncalls}_{ncuts}");
    let inf = scip.set.infinity();
    let mut cut = scip_lp::create_empty_row_sepa(scip, sepa, &cutname, -inf, 1.0, false, false, true)?;

    scip_lp::cache_row_extensions(scip, cut);
    for &n in cliquenodes {
        api::add_var_to_row(scip, cut, vars[n as usize], 1.0)?;
    }
    scip_lp::flush_row_extensions(scip, cut)?;

    // Set cut rank: for clique cuts we always set to 1.
    scip.rows[cut].rank = 1;

    api::add_pool_cut(scip, cut)?;

    // Release the row.
    scip_lp::release_row(scip, &mut cut)
}

/*
 * Callback methods of separator
 */

/// `sepaCopyClique`: copy method for separator plugins.
fn sepa_copy_clique(scip: &mut Scip, _sourcescip: &Scip, _sepa: SepaId) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_sepa_clique(scip)
}

/// `sepaExitsolClique`: solving process deinitialization method of
/// separator.
fn sepa_exitsol_clique(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    // Free tclique data.
    if let Some(g) = data_mut(scip, sepa).tcliquegraph.take() {
        tcliquegraph_free(scip, g)?;
    }
    data_mut(scip, sepa).tcliquegraphloaded = false;

    Ok(())
}

/// `sepaExeclpClique`: LP solution separation method of separator.
fn sepa_execlp_clique(scip: &mut Scip, sepa: SepaId, result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Only call separator, if we are not close to terminating.
    if crate::solve::solve_is_stopped(scip, false) {
        return Ok(());
    }

    // Only call separator, if an optimal LP solution is at hand.
    if api::get_lp_solstat(scip) != crate::types::LpSolStat::Optimal {
        return Ok(());
    }

    // Only call separator, if there are fractional variables.
    if api::get_n_lp_branch_cands(scip) == 0 {
        return Ok(());
    }

    // Separate cuts on the LP solution.
    separate_cuts(scip, sepa, NONE, result)
}

/// `sepaExecsolClique`: arbitrary primal solution separation method of
/// separator.
fn sepa_execsol_clique(scip: &mut Scip, sepa: SepaId, sol: SolId, result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Separate cuts on the given primal solution.
    separate_cuts(scip, sepa, sol, result)
}

/*
 * separator specific interface methods
 */

/// `SCIPincludeSepaClique`: creates the clique separator and includes it in
/// SCIP.
pub fn include_sepa_clique(scip: &mut Scip) -> ScipResult {
    // Create clique separator data.
    let sepadata = SepaData::default();

    // Include separator.
    let sepa = api::include_sepa_basic(
        scip,
        SEPA_NAME,
        SEPA_DESC,
        SEPA_PRIORITY,
        SEPA_FREQ,
        SEPA_MAXBOUNDDIST,
        SEPA_USESSUBSCIP,
        SEPA_DELAY,
        Some(sepa_execlp_clique),
        Some(sepa_execsol_clique),
        Some(Box::new(sepadata) as Box<dyn Any>),
    )?;

    // Set non-NULL pointers to callback methods.
    let s = &mut scip.sepas[sepa];
    s.sepacopy = Some(sepa_copy_clique);
    s.sepaexitsol = Some(sepa_exitsol_clique);

    // Add clique separator parameters.
    macro_rules! target {
        ($ty:ty, $field:ident) => {
            crate::data_target!($ty, sepas, sepa, sepadata, SepaData, $field)
        };
    }
    paramset_add_real(
        scip,
        "separating/clique/scaleval",
        "factor for scaling weights",
        target!(f64, scaleval),
        true,
        DEFAULT_SCALEVAL,
        1.0,
        REAL_MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/clique/maxtreenodes",
        "maximal number of nodes in branch and bound tree (-1: no limit)",
        target!(i32, maxtreenodes),
        true,
        DEFAULT_MAXTREENODES,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/clique/backtrackfreq",
        "frequency for premature backtracking up to tree level 1 (0: no backtracking)",
        target!(i32, backtrackfreq),
        true,
        DEFAULT_BACKTRACKFREQ,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/clique/maxsepacuts",
        "maximal number of clique cuts separated per separation round (-1: no limit)",
        target!(i32, maxsepacuts),
        false,
        DEFAULT_MAXSEPACUTS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/clique/maxzeroextensions",
        "maximal number of zero-valued variables extending the clique (-1: no limit)",
        target!(i32, maxzeroextensions),
        true,
        DEFAULT_MAXZEROEXTENSIONS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/clique/cliquetablemem",
        "maximal memory size of dense clique table (in kb)",
        target!(f64, cliquetablemem),
        true,
        DEFAULT_CLIQUETABLEMEM,
        0.0,
        i32::MAX as f64 / 1024.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/clique/cliquedensity",
        "minimal density of cliques to use a dense clique table",
        target!(f64, cliquedensity),
        true,
        DEFAULT_CLIQUEDENSITY,
        0.0,
        1.0,
        None,
    )?;

    Ok(())
}
