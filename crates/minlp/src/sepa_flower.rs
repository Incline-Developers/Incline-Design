//! Port of `sepa_flower.c`: flower-inequality separator.
//!
//! The hypergraph of multilinear products is built from AND constraints
//! (`separating/flower/scanand`, on by default) and product expressions
//! (`scanproduct`, off). Only the current 1- and 2-flower separation is
//! ported (`USE_OLD_*_SEPARATION` are not defined); timing statistics are
//! not kept.

use std::{any::Any, collections::HashMap};

use crate::{
    api,
    hypergraph::{Edge, Hypergraph, Overlap, Vertex},
    paramset::{paramset_add_bool, paramset_add_int},
    retcode::ScipResult,
    scip::Scip,
    types::{NodeId, RowId, ScipResultCode, SepaId, SolId, VarId},
    var::NONE,
};

const SEPA_NAME: &str = "flower";
const SEPA_DESC: &str = "flower cut separator";
const SEPA_PRIORITY: i32 = 100000;
const SEPA_FREQ: i32 = 1;
const SEPA_MAXBOUNDDIST: f64 = 1.0;
/// Does the separator use a secondary SCIP instance?
const SEPA_USESSUBSCIP: bool = false;
/// Should separation method be delayed if other separators found cuts?
const SEPA_DELAY: bool = false;

const DEFAULT_MIN_OVERLAPS: i32 = 1;
const DEFAULT_SCAN_AND: bool = true;
const DEFAULT_SCAN_PRODUCT: bool = false;
const DEFAULT_MAX_STANDARD: i32 = 0;
const DEFAULT_MAX_ONEFLOWER: i32 = 10000000;
const DEFAULT_MAX_TWOFLOWER: i32 = 10000000;
const DEFAULT_DELAY_STANDARD: bool = false;
/// Number of useless separation rounds after which we stop separating.
const DEFAULT_MAX_USELESS_ONEFLOWER: i32 = 1;
/// Number of useless separation rounds after which we stop separating.
const DEFAULT_MAX_USELESS_TWOFLOWER: i32 = 1;

/// `SCIP_Hypergraph_NodeData`: data associated with each hypergraph node.
#[derive(Clone, Default)]
struct VertexData {
    /// Variable associated with this node.
    var: VarId,
    /// Factor to scale a cut's coefficient with; equals 1/ub.
    coefscale: f64,
    /// Solution value in [0,1]-transformed space.
    solval: f64,
}

/// `SCIP_Hypergraph_EdgeData`: data associated with each hypergraph edge.
#[derive(Clone, Default)]
struct EdgeData {
    /// Variable of the product.
    var: VarId,
    /// Coefficient of the product.
    coefficient: f64,
    /// Factor to scale a cut's coefficient with; equals 1/(product of ubs *
    /// coefficient).
    coefscale: f64,
    /// Value of the variable in [0,1]-transformed space.
    solval: f64,
    /// The slack value of that edge.
    slackval: f64,
}

/// `SCIP_Hypergraph_OverlapData`: data associated with each overlap, i.e.,
/// intersection of two edges.
#[derive(Clone, Default)]
struct OverlapData {
    /// Sum of 1-z_v for all v in this overlap.
    sumnodecomplements: f64,
    /// 1 minus the maximum solution value of any edge incident to this
    /// overlap.
    minedgecomplement: f64,
    /// An edge for which minedgecomplement is attained.
    minedge: Edge,
}

type FlowerHypergraph = Hypergraph<VertexData, EdgeData, OverlapData>;

/// `SCIP_SepaData`: separator data.
struct SepaData {
    /// Last run for which we constructed a hypergraph.
    lastrun: i32,
    /// Last node for which we separated.
    lastnode: NodeId,
    /// Whether to scan AND constraints when constructing a hypergraph.
    scanand: bool,
    /// Whether to scan product expressions when constructing a hypergraph.
    scanproduct: bool,
    /// The hypergraph (`NULL`: None).
    hypergraph: Option<FlowerHypergraph>,
    /// Total number of generated cuts.
    nsepacuts: i32,
    /// Total number of generated standard relaxation inequalities.
    nsepastandard: i32,
    /// Total number of generated 1-flower inequalities.
    nsepaoneflower: i32,
    /// Total number of generated 2-flower inequalities.
    nsepatwoflower: i32,
    /// Minimum number of overlaps needed to actually try separation.
    minnoverlaps: i32,
    /// Maximum number of standard relaxation inequalities per round.
    maxstandard: i32,
    /// Maximum number of 1-flower inequalities per round.
    maxoneflower: i32,
    /// Maximum number of 2-flower inequalities per round.
    maxtwoflower: i32,
    /// Whether to only generate standard inequalities if also flower.
    delaystandard: bool,
    /// Number of useless separation rounds after which we stop separating.
    maxuselessoneflower: i32,
    /// Number of useless separation rounds after which we stop separating.
    maxuselesstwoflower: i32,
    /// Number of recent useless separation rounds for 1-flowers.
    nuselessoneflower: i32,
    /// Number of recent useless separation rounds for 2-flowers.
    nuselesstwoflower: i32,
}

fn sd(scip: &mut Scip, sepa: SepaId) -> &mut SepaData {
    scip.sepas[sepa].sepadata.as_mut().unwrap().downcast_mut::<SepaData>().unwrap()
}

/*
 * Local methods
 */

/// `constructHypergraph`: constructs the hypergraph from transformed
/// problem.
fn construct_hypergraph(scip: &mut Scip, sepa: SepaId) -> ScipResult<FlowerHypergraph> {
    let (scanand, scanproduct) = {
        let d = sd(scip, sepa);
        (d.scanand, d.scanproduct)
    };

    let mut nvars = api::get_n_vars(scip);
    if nvars == 0 {
        nvars += 1;
    }
    let mut hypergraph = FlowerHypergraph::new(nvars, nvars, nvars, 4);

    // Create map from variables to nodes and vertex array.
    let mut varsnodes: HashMap<VarId, Vertex> = HashMap::new();
    let mut vertices: Vec<Vertex> = Vec::new();

    if scanand {
        // Scan AND constraints.
        let conshdlr = crate::set::set_find_conshdlr(scip, "and");
        if conshdlr != NONE {
            for cons in crate::cons::conshdlr_get_conss(scip, conshdlr) {
                // Get number of variables.
                vertices.clear();
                let vars = crate::cons_and::get_vars_and(scip, cons).to_vec();
                for &var in &vars {
                    let v = match varsnodes.get(&var) {
                        Some(&v) => v,
                        None => {
                            let v = hypergraph.add_vertex();
                            hypergraph.vertex_data_mut(v).var = var;
                            varsnodes.insert(var, v);
                            v
                        }
                    };
                    vertices.push(v);
                }
                let edge = hypergraph.add_edge(&vertices);
                let edgedata = hypergraph.edge_data_mut(edge);
                edgedata.var = crate::cons_and::get_resultant_and(scip, cons);
                edgedata.coefficient = 1.0;
            }
        }
    }

    if scanproduct {
        // Scan nonlinear constraints for product expressions.
        let conshdlr = crate::set::set_find_conshdlr(scip, "nonlinear");
        if conshdlr != NONE {
            let conss = crate::cons::conshdlr_get_conss(scip, conshdlr);

            // Prepare iteration such that we visit every expression only once.
            let mut it = crate::expr::ExprIter::create();
            it.init(scip, NONE, crate::expr::ExprIterType::Dfs, false)?;
            it.set_stages_dfs(scip, crate::expr::EXPRITER_ENTEREXPR);

            for &cons in &conss {
                // Iterate through all expressions of the nonlinear constraint
                // that we haven't seen so far.
                let consexpr = crate::cons_nonlinear::get_expr_nonlinear(scip, cons);
                let mut expr = it.restart_dfs(scip, consexpr);
                while !it.is_end() {
                    if crate::expr::expr_is_product(scip, expr) {
                        let exprvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, expr);
                        if exprvar != NONE {
                            let children = scip.exprs[expr].children.clone();
                            vertices.clear();
                            let mut complete = true;
                            for &child in &children {
                                let auxvar = crate::cons_nonlinear::get_expr_aux_var_nonlinear(scip, child);
                                if auxvar == NONE || scip.set.is_lt(scip.vars[auxvar].glbdom.lb, 0.0) {
                                    complete = false;
                                    break;
                                }
                                let v = match varsnodes.get(&auxvar) {
                                    Some(&v) => v,
                                    None => {
                                        let v = hypergraph.add_vertex();
                                        hypergraph.vertex_data_mut(v).var = auxvar;
                                        varsnodes.insert(auxvar, v);
                                        v
                                    }
                                };
                                vertices.push(v);
                            }
                            if complete {
                                // If the lower bound is nonnegative, then we can
                                // consider its relaxed domain [0, ub].
                                let edge = hypergraph.add_edge(&vertices);
                                let coefficient = crate::expr_product::get_coef_expr_product(scip, expr);
                                let edgedata = hypergraph.edge_data_mut(edge);
                                edgedata.var = exprvar;
                                edgedata.coefficient = coefficient;
                            }
                        }
                    }
                    expr = it.get_next(scip);
                }
            }

            it.free(scip);
        }
    }

    // Compute each node's incident edges.
    hypergraph.compute_vertices_edges();

    // Find all pair-wise intersections of edges (of size at least 2).
    hypergraph.compute_overlaps(None)?;

    hypergraph.compute_overlaps_edges();

    Ok(hypergraph)
}

/// `prepareSeparation`: extracts the solution values of the hypergraph's
/// vertices, edges and overlaps.
fn prepare_separation(scip: &Scip, hypergraph: &mut FlowerHypergraph, sol: SolId) {
    let feastol = scip.set.feastol();
    let cmax = |a: f64, b: f64| if a >= b { a } else { b };
    let cmin = |a: f64, b: f64| if a <= b { a } else { b };

    // We extract the solution values for the hypergraph's vertices.
    for vertex in 0..hypergraph.get_n_vertices() {
        let var = hypergraph.vertex_data(vertex).var;
        let value = api::get_sol_val(scip, sol, var);
        let ub = scip.vars[var].locdom.ub;

        // Compute the scaling factor for scaling the domain [lb,ub] to [0,1].
        let vertexdata = hypergraph.vertex_data_mut(vertex);
        vertexdata.coefscale = 1.0 / cmax(ub, feastol);
        vertexdata.solval = value * vertexdata.coefscale;
    }

    // We extract the solution values for the hypergraph's edges.
    let nedges = hypergraph.get_n_edges();
    for edge in 0..nedges {
        let vertices = hypergraph.edge_vertices(edge).to_vec();
        let mut ubprod = 1.0;
        for &v in &vertices {
            ubprod /= hypergraph.vertex_data(v).coefscale;
        }
        let value = api::get_sol_val(scip, sol, hypergraph.edge_data(edge).var);

        // Compute the scaling factor for scaling the domain [0, product of ubs
        // * edge-coefficient ] to [0,1].
        let mut slackval;
        {
            let edgedata = hypergraph.edge_data_mut(edge);
            edgedata.coefscale = 1.0 / (edgedata.coefficient * ubprod);
            edgedata.coefscale = cmin(edgedata.coefscale, 1.0 / feastol);
            edgedata.solval = value * edgedata.coefscale;
            slackval = edgedata.solval - 1.0;
        }
        for &v in &vertices {
            slackval += 1.0 - hypergraph.vertex_data(v).solval;
        }
        hypergraph.edge_data_mut(edge).slackval = slackval;
    }

    // Initialize overlap's sumnodecomplements to \sum_{v in o} (1-z_v).
    // Together with minedgecomplement, which is the minimum value of 1-z_e over
    // all incident edges e, the difference sumnodecomplements -
    // minedgecomplement represents the violation increase if the sum of (1-z_v)
    // is replaced by (1-z_e) in k-flower inequalities.
    let noverlaps = hypergraph.get_n_overlaps();
    for overlap in 0..noverlaps {
        let mut sum = 0.0;
        for &v in hypergraph.overlap_vertices(overlap) {
            sum += 1.0 - hypergraph.vertex_data(v).solval;
        }
        let overlapdata = hypergraph.overlap_data_mut(overlap);
        overlapdata.minedgecomplement = 2.0;
        overlapdata.sumnodecomplements = sum;
    }

    // Iterate over all edge-overlap incidences to compute each overlap's
    // maximum-value edge. Note that the initial value of minedgecomplement is
    // guaranteed to be larger than 1 - z_e for any incident edge e, so it will
    // be updated in the loop below.
    for edge in 0..hypergraph.get_n_edges() {
        let edgecomplement = 1.0 - hypergraph.edge_data(edge).solval;
        let beyond = hypergraph.edges_overlaps_beyond(edge);
        for i in hypergraph.edges_overlaps_first(edge)..beyond {
            let overlap = hypergraph.edges_overlaps_get_at_index(i);
            let overlapdata = hypergraph.overlap_data_mut(overlap);
            if edgecomplement < overlapdata.minedgecomplement {
                overlapdata.minedgecomplement = edgecomplement;
                overlapdata.minedge = edge;
            }
        }
    }
}

/// `addCut`: add a generated cut row to the cut pool (for the root node) or
/// as a row (otherwise).
fn add_cut(scip: &mut Scip, sol: SolId, row: RowId, pnumseparated: &mut i32, presult: &mut ScipResultCode) -> ScipResult {
    if crate::scip_lp::is_cut_efficacious(scip, sol, row) {
        if api::get_depth(scip) == 0 {
            api::add_pool_cut(scip, row)?;
        } else {
            let infeasible = crate::scip_lp::add_row(scip, row, false)?;
            if infeasible {
                *presult = ScipResultCode::Cutoff;
            }
        }
        if *presult != ScipResultCode::Cutoff {
            *presult = ScipResultCode::Separated;
        }
        *pnumseparated += 1;
    }

    Ok(())
}

/// Creates a flower cut row `lhs <= sum coefs * vars` (`SCIPcreateRowSepa`
/// with the variables added through the row cache).
fn create_cut(scip: &mut Scip, sepa: SepaId, name: &str, lhs: f64, terms: &[(VarId, f64)]) -> ScipResult<RowId> {
    let infinity = scip.set.infinity();
    let local = api::get_depth(scip) > 0;
    let row = crate::scip_lp::create_empty_row_sepa(scip, sepa, name, lhs, infinity, local, false, true)?;
    crate::scip_lp::cache_row_extensions(scip, row);
    for &(var, coef) in terms {
        api::add_var_to_row(scip, row, var, coef)?;
    }
    crate::scip_lp::flush_row_extensions(scip, row)?;
    Ok(row)
}

/// `separateStandard`: separate missing inequalities from the standard
/// relaxation.
fn separate_standard(scip: &mut Scip, sepa: SepaId, hypergraph: &FlowerHypergraph, sol: SolId, maxnsepa: i32, presult: &mut ScipResultCode) -> ScipResult {
    let mut nseparated = 0;

    let mut edge = 0;
    while edge < hypergraph.get_n_edges() && *presult != ScipResultCode::Cutoff && nseparated < maxnsepa {
        let edgedata = hypergraph.edge_data(edge).clone();
        let vertices = hypergraph.edge_vertices(edge);
        let mut i = 0;
        while i < vertices.len() && *presult != ScipResultCode::Cutoff && nseparated < maxnsepa {
            let vertexdata = hypergraph.vertex_data(vertices[i]).clone();
            if api::is_efficacious(scip, edgedata.solval - vertexdata.solval) {
                // 0 <= z_v - z_e
                sd(scip, sepa).nsepacuts += 1;
                let name = format!("flower_{:05}_standard", sd(scip, sepa).nsepacuts);
                let infinity = scip.set.infinity();
                let local = api::get_depth(scip) > 0;
                let mut row = crate::scip_lp::create_empty_row_sepa(scip, sepa, &name, 0.0, infinity, local, false, true)?;
                crate::scip_lp::add_vars_to_row(scip, row, &[vertexdata.var, edgedata.var], &[1.0 * vertexdata.coefscale, -1.0 * edgedata.coefscale])?;

                add_cut(scip, sol, row, &mut nseparated, presult)?;
                crate::scip_lp::release_row(scip, &mut row)?;
            }
            i += 1;
        }
        edge += 1;
    }

    sd(scip, sepa).nsepastandard += nseparated;

    Ok(())
}

/// `separateOneFlower`: separate 1-flower inequalities.
fn separate_one_flower(scip: &mut Scip, sepa: SepaId, hypergraph: &FlowerHypergraph, sol: SolId, maxnsepa: i32, presult: &mut ScipResultCode) -> ScipResult {
    let mut nseparated = 0;

    let mut base = 0;
    while base < hypergraph.get_n_edges() && nseparated < maxnsepa {
        let basedata = hypergraph.edge_data(base).clone();
        let mut bestgain = -1.0;
        let mut bestoverlap: Overlap = -1;

        // We need to test if z_base + (1-z_overlap) + sum_{v in base \ overlap}
        // (1-z_v) < 1. This is done by minimizing the change (1-z_overlap) -
        // sum_{v in overlap) (1-z) w.r.t. the standard inequality over all
        // overlaps that are incident to base.
        for i in hypergraph.edges_overlaps_first(base)..hypergraph.edges_overlaps_beyond(base) {
            let overlap = hypergraph.edges_overlaps_get_at_index(i);
            let overlapdata = hypergraph.overlap_data(overlap);
            let gain = overlapdata.sumnodecomplements - overlapdata.minedgecomplement;
            if gain > bestgain {
                bestgain = gain;
                bestoverlap = overlap;
            }
        }

        if bestgain > 0.0 && api::is_efficacious(scip, bestgain - basedata.slackval) {
            let basevertices = hypergraph.edge_vertices(base);
            let overlapvertices = hypergraph.overlap_vertices(bestoverlap);
            let adjacent = hypergraph.overlap_data(bestoverlap).minedge;
            let adjacentdata = hypergraph.edge_data(adjacent);

            let mut terms = vec![(basedata.var, 1.0 * basedata.coefscale), (adjacentdata.var, -1.0 * adjacentdata.coefscale)];
            for &v in basevertices {
                let vertexdata = hypergraph.vertex_data(v);
                terms.push((vertexdata.var, -1.0 * vertexdata.coefscale));
            }
            for &v in overlapvertices {
                let vertexdata = hypergraph.vertex_data(v);
                terms.push((vertexdata.var, 1.0 * vertexdata.coefscale));
            }

            sd(scip, sepa).nsepacuts += 1;
            let name = format!("flower_{:05}_1flower", sd(scip, sepa).nsepacuts);
            let lhs = 1.0 * (overlapvertices.len() as i32 - basevertices.len() as i32) as f64;
            let mut row = create_cut(scip, sepa, &name, lhs, &terms)?;

            add_cut(scip, sol, row, &mut nseparated, presult)?;
            crate::scip_lp::release_row(scip, &mut row)?;
        }
        base += 1;
    }

    sd(scip, sepa).nsepaoneflower += nseparated;

    Ok(())
}

/// `separateTwoFlower`: separate 2-flower inequalities.
fn separate_two_flower(scip: &mut Scip, sepa: SepaId, hypergraph: &FlowerHypergraph, sol: SolId, maxnsepa: i32, presult: &mut ScipResultCode) -> ScipResult {
    let mut nseparated = 0;

    let mut base = 0;
    while base < hypergraph.get_n_edges() && nseparated < maxnsepa {
        let basedata = hypergraph.edge_data(base).clone();
        let basesize = hypergraph.edge_size(base);
        let first = hypergraph.edges_overlaps_first(base);
        let beyond = hypergraph.edges_overlaps_beyond(base);
        let mut bestoverlap1: Overlap = -1;
        let mut bestoverlap2: Overlap = -1;
        let mut bestgain = -1.0;

        // We need to test if z_base + (1-z_overlap1) + (1-z_overlap2) + sum_{v
        // in base \ (overlap1 \cup overlap2)} (1-z_v) < 1. This is done by
        // minimizing the change (1-z_overlap1) - sum_{v in overlap1) (1-z) and of
        // (1-z_overlap2) - sum_{v in overlap2) (1-z), both w.r.t. the standard
        // inequality over all overlaps that are incident to base. However, this
        // only correctly reflects the changes violation if the two overlaps are
        // disjoint.
        for i1 in first..beyond {
            let overlap1 = hypergraph.edges_overlaps_get_at_index(i1);
            let overlap1size = hypergraph.overlap_size(overlap1);

            // If |o1| > |e|/2 then for sure |o2| > holds for sure, so we can
            // stop.
            if overlap1size > basesize / 2 {
                break;
            }

            let overlap1data = hypergraph.overlap_data(overlap1);
            let gain1 = overlap1data.sumnodecomplements - overlap1data.minedgecomplement;

            for i2 in i1 + 1..beyond {
                let overlap2 = hypergraph.edges_overlaps_get_at_index(i2);
                let overlap2size = hypergraph.overlap_size(overlap2);
                if (overlap1size + overlap2size > basesize) || ((overlap1size == overlap2size) && (overlap1 >= overlap2)) || !hypergraph.overlaps_disjoint(overlap1, overlap2) {
                    continue;
                }

                let overlap2data = hypergraph.overlap_data(overlap2);
                let gain2 = overlap2data.sumnodecomplements - overlap2data.minedgecomplement;
                let totalgain = gain1 + gain2;

                if totalgain > bestgain {
                    bestgain = totalgain;
                    bestoverlap1 = overlap1;
                    bestoverlap2 = overlap2;
                }
            }
        }

        if bestgain > 0.0 && api::is_efficacious(scip, bestgain - basedata.slackval) {
            let basevertices = hypergraph.edge_vertices(base);
            let overlap1vertices = hypergraph.overlap_vertices(bestoverlap1);
            let adjacent1data = hypergraph.edge_data(hypergraph.overlap_data(bestoverlap1).minedge);
            let overlap2vertices = hypergraph.overlap_vertices(bestoverlap2);
            let adjacent2data = hypergraph.edge_data(hypergraph.overlap_data(bestoverlap2).minedge);

            let mut terms = vec![
                (basedata.var, 1.0 * basedata.coefscale),
                (adjacent1data.var, -1.0 * adjacent1data.coefscale),
                (adjacent2data.var, -1.0 * adjacent2data.coefscale),
            ];
            for &v in basevertices {
                let data = hypergraph.vertex_data(v);
                terms.push((data.var, -1.0 * data.coefscale));
            }
            for &v in overlap1vertices {
                let data = hypergraph.vertex_data(v);
                terms.push((data.var, 1.0 * data.coefscale));
            }
            for &v in overlap2vertices {
                let data = hypergraph.vertex_data(v);
                terms.push((data.var, 1.0 * data.coefscale));
            }

            sd(scip, sepa).nsepacuts += 1;
            let name = format!("flower_{:05}_2flower", sd(scip, sepa).nsepacuts);
            let lhs = (overlap1vertices.len() + overlap2vertices.len()) as f64 - basevertices.len() as f64 - 1.0;
            let mut row = create_cut(scip, sepa, &name, lhs, &terms)?;
            add_cut(scip, sol, row, &mut nseparated, presult)?;
            crate::scip_lp::release_row(scip, &mut row)?;
        }
        base += 1;
    }

    sd(scip, sepa).nsepatwoflower += nseparated;

    Ok(())
}

/// `separate`: separates flower inequalities (and standard relaxation
/// inequalities).
fn separate(scip: &mut Scip, sepa: SepaId, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotFind;

    // We do not apply the cuts for subscips.
    if crate::copy::get_subscip_depth(scip) > 0 {
        return Ok(());
    }

    let nruns = api::get_n_runs(scip);
    if sd(scip, sepa).lastrun != nruns {
        // If we still have a hypergraph, we delete it.
        sd(scip, sepa).hypergraph = None;

        let hypergraph = construct_hypergraph(scip, sepa)?;

        // If the hypergraph is not interesting then we just delete it again.
        let d = sd(scip, sepa);
        d.hypergraph = if hypergraph.get_n_overlaps() < d.minnoverlaps { None } else { Some(hypergraph) };

        d.lastrun = nruns;
        d.nuselessoneflower = 0;
        d.nuselesstwoflower = 0;
    }

    let node = crate::tree::tree_get_current_node(scip);
    let d = sd(scip, sepa);
    if node != d.lastnode {
        d.lastnode = node;
        d.nuselessoneflower = 0;
        d.nuselesstwoflower = 0;
    }

    if d.hypergraph.is_some() && (d.nuselessoneflower <= d.maxuselessoneflower || d.nuselesstwoflower <= d.maxuselesstwoflower) {
        *result = ScipResultCode::DidNotFind;

        // The hypergraph is taken out while separating.
        let mut hypergraph = d.hypergraph.take().unwrap();
        let ret = separate_with(scip, sepa, &mut hypergraph, sol, result);
        sd(scip, sepa).hypergraph = Some(hypergraph);
        ret?;
    }

    Ok(())
}

/// The separation part of `separate` once a hypergraph exists.
fn separate_with(scip: &mut Scip, sepa: SepaId, hypergraph: &mut FlowerHypergraph, sol: SolId, result: &mut ScipResultCode) -> ScipResult {
    prepare_separation(scip, hypergraph, sol);

    let (maxstandard, delaystandard, maxoneflower, maxtwoflower) = {
        let d = sd(scip, sepa);
        (d.maxstandard, d.delaystandard, d.maxoneflower, d.maxtwoflower)
    };

    if maxstandard != 0 && !delaystandard {
        separate_standard(scip, sepa, hypergraph, sol, maxstandard, result)?;
        if *result == ScipResultCode::Cutoff {
            return Ok(());
        }
    }

    if maxoneflower != 0 && sd(scip, sepa).nuselessoneflower <= sd(scip, sepa).maxuselessoneflower {
        let oldnsepaoneflower = sd(scip, sepa).nsepaoneflower;
        separate_one_flower(scip, sepa, hypergraph, sol, maxoneflower, result)?;
        let d = sd(scip, sepa);
        if d.nsepaoneflower > oldnsepaoneflower {
            d.nuselessoneflower = 0;
        } else {
            d.nuselessoneflower += 1;
        }
        if *result == ScipResultCode::Cutoff {
            return Ok(());
        }
    }

    if maxtwoflower != 0 && sd(scip, sepa).nuselesstwoflower <= sd(scip, sepa).maxuselesstwoflower {
        let oldnsepatwoflower = sd(scip, sepa).nsepatwoflower;
        separate_two_flower(scip, sepa, hypergraph, sol, maxtwoflower, result)?;
        let d = sd(scip, sepa);
        if d.nsepatwoflower > oldnsepatwoflower {
            d.nuselesstwoflower = 0;
        } else {
            d.nuselesstwoflower += 1;
        }
        if *result == ScipResultCode::Cutoff {
            return Ok(());
        }
    }

    if *result == ScipResultCode::Separated && maxstandard != 0 && delaystandard {
        separate_standard(scip, sepa, hypergraph, sol, maxstandard, result)?;
    }

    Ok(())
}

/*
 * Callback methods of separator.
 */

/// `sepaCopyFlower`: copy method for separator plugins.
fn sepa_copy_flower(scip: &mut Scip, _source: &Scip, _sepa: SepaId) -> ScipResult {
    include_sepa_flower(scip)
}

/// `sepaFreeFlower`: destructor of separator to free user data.
fn sepa_free_flower(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    scip.sepas[sepa].sepadata = None;
    Ok(())
}

/// `sepaExitsolFlower`: solving process deinitialization method of
/// separator.
fn sepa_exitsol_flower(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    if crate::copy::get_subscip_depth(scip) > 0 {
        return Ok(());
    }

    sd(scip, sepa).hypergraph = None;

    Ok(())
}

/// `sepaExeclpFlower`: LP solution separation method of separator.
fn sepa_execlp_flower(scip: &mut Scip, sepa: SepaId, result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    separate(scip, sepa, NONE, result)
}

/// `sepaExecsolFlower`: arbitrary primal solution separation method of
/// separator.
fn sepa_execsol_flower(scip: &mut Scip, sepa: SepaId, sol: SolId, result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    separate(scip, sepa, sol, result)
}

/*
 * separator specific interface methods
 */

/// `SCIPincludeSepaFlower`: creates the flower separator and includes it
/// in SCIP.
pub fn include_sepa_flower(scip: &mut Scip) -> ScipResult {
    // Create flower separator data.
    let sepadata = SepaData {
        lastrun: -1,
        lastnode: NONE,
        scanand: false,
        scanproduct: false,
        hypergraph: None,
        nsepacuts: 0,
        nsepastandard: 0,
        nsepaoneflower: 0,
        nsepatwoflower: 0,
        minnoverlaps: 0,
        maxstandard: 0,
        maxoneflower: 0,
        maxtwoflower: 0,
        delaystandard: false,
        maxuselessoneflower: 0,
        maxuselesstwoflower: 0,
        nuselessoneflower: 0,
        nuselesstwoflower: 0,
    };

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
        Some(sepa_execlp_flower),
        Some(sepa_execsol_flower),
        Some(Box::new(sepadata) as Box<dyn Any>),
    )?;

    // Set non fundamental callbacks via setter functions (the init callback
    // only resets timing statistics).
    let s = &mut scip.sepas[sepa];
    s.sepacopy = Some(sepa_copy_flower);
    s.sepafree = Some(sepa_free_flower);
    s.sepaexitsol = Some(sepa_exitsol_flower);

    // Add flower separator parameters.
    macro_rules! target {
        ($ty:ty, $field:ident) => {
            crate::data_target!($ty, sepas, sepa, sepadata, SepaData, $field)
        };
    }
    paramset_add_bool(
        scip,
        "separating/flower/scanand",
        "Whether to scan AND constraints when constructing hypergraph",
        target!(bool, scanand),
        false,
        DEFAULT_SCAN_AND,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/flower/scanproduct",
        "Whether to scan product expressions when constructing hypergraph",
        target!(bool, scanproduct),
        false,
        DEFAULT_SCAN_PRODUCT,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/flower/maxstandard",
        "Maximum number of standard relaxation inequalities per cut round",
        target!(i32, maxstandard),
        false,
        DEFAULT_MAX_STANDARD,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/flower/maxoneflower",
        "Maximum number of 1-flower inequalities per cut round",
        target!(i32, maxoneflower),
        false,
        DEFAULT_MAX_ONEFLOWER,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/flower/maxtwoflower",
        "Maximum number of 2-flower inequalities per cut round",
        target!(i32, maxtwoflower),
        false,
        DEFAULT_MAX_TWOFLOWER,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/flower/minnoverlaps",
        "Minimum number of overlaps necessary to try separation",
        target!(i32, minnoverlaps),
        false,
        DEFAULT_MIN_OVERLAPS,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/flower/delaystandard",
        "Whether to only generate standard inequalities if also flowers were generated",
        target!(bool, delaystandard),
        false,
        DEFAULT_DELAY_STANDARD,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/flower/maxuselessoneflower",
        "Number of useless separation rounds after which we stop separating 1-flowers",
        target!(i32, maxuselessoneflower),
        false,
        DEFAULT_MAX_USELESS_ONEFLOWER,
        0,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/flower/maxuselesstwoflower",
        "Number of useless separation rounds after which we stop separating 2-flowers",
        target!(i32, maxuselesstwoflower),
        false,
        DEFAULT_MAX_USELESS_TWOFLOWER,
        0,
        i32::MAX,
        None,
    )?;

    Ok(())
}
