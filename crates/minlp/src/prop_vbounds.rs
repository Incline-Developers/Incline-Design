//! Port of `prop_vbounds.c`: variable upper and lower bound propagator.
//!
//! The propagator works on bound indices: for an active variable with
//! problem index i, 2*i is its lower bound and 2*i+1 its upper bound. Bounds
//! are propagated in (reverse) topological order of the variable bound graph
//! through a priority queue; the presolving method aggregates strongly
//! connected components of the implication (clique) graph.

use std::any::Any;

use crate::{
    api,
    cons::*,
    event::*,
    pqueue::PQueue,
    retcode::ScipResult,
    scip::{Scip, Stage},
    scip_var,
    types::{BoundType, ConfType, EventhdlrId, PropId, ScipResultCode, VarId, VarType},
    var::{BdChgIdx, var_get_n_cliques, var_get_n_impls, var_is_active, var_is_binary, var_is_implied_integral, var_is_integral},
};

/* propagator properties */
const PROP_NAME: &str = "vbounds";
const PROP_DESC: &str = "propagates variable upper and lower bounds";
const PROP_TIMING: PropTiming = PROPTIMING_BEFORELP | PROPTIMING_AFTERLPLOOP;
/// Propagator priority.
const PROP_PRIORITY: i32 = 3000000;
/// Propagator frequency.
const PROP_FREQ: i32 = 1;
/// Should propagation method be delayed, if other propagators found
/// reductions?
const PROP_DELAY: bool = false;

/// Priority of the presolving method (>= 0: before, < 0: after constraint
/// handlers); combined with presolvers.
const PROP_PRESOL_PRIORITY: i32 = -90000;
const PROP_PRESOLTIMING: PresolTiming = PRESOLTIMING_MEDIUM | PRESOLTIMING_EXHAUSTIVE;
/// Maximal number of presolving rounds the presolver participates in (-1:
/// no limit).
const PROP_PRESOL_MAXROUNDS: i32 = -1;

/* event handler properties */
const EVENTHDLR_NAME: &str = "vbounds";
const EVENTHDLR_DESC: &str = "bound change event handler for for vbounds propagator";

/* default parameter values */
const DEFAULT_USEBDWIDENING: bool = true;
const DEFAULT_USEIMPLICS: bool = false;
const DEFAULT_USECLIQUES: bool = false;
const DEFAULT_USEVBOUNDS: bool = true;
const DEFAULT_DOTOPOSORT: bool = true;
const DEFAULT_SORTCLIQUES: bool = false;
const DEFAULT_DETECTCYCLES: bool = false;
const DEFAULT_MINNEWCLIQUES: f64 = 0.1;
const DEFAULT_MAXCLIQUESMEDIUM: f64 = 50.0;
const DEFAULT_MAXCLIQUESEXHAUSTIVE: f64 = 100.0;

/* propagator defines (bound indices) */
#[inline]
fn get_lb_index(idx: i32) -> i32 {
    2 * idx
}
#[inline]
fn get_ub_index(idx: i32) -> i32 {
    2 * idx + 1
}
#[inline]
fn get_var_index(idx: i32) -> usize {
    (idx / 2) as usize
}
#[inline]
fn get_boundtype(idx: i32) -> BoundType {
    if idx % 2 == 0 { BoundType::Lower } else { BoundType::Upper }
}
#[inline]
fn is_index_lowerbound(idx: i32) -> bool {
    idx % 2 == 0
}
#[inline]
fn get_other_bound_index(idx: i32) -> i32 {
    idx + 1 - 2 * (idx % 2)
}

/// Propagator data.
#[derive(Default)]
struct PropData {
    eventhdlr: EventhdlrId,
    /// Variables considered within the propagator.
    vars: Vec<VarId>,
    /// Maps a variable to its index in `vars`.
    varhashmap: std::collections::HashMap<VarId, i32>,
    /// The bounds of variables in topological order; -1 if the bound at
    /// that position has no outgoing implications, cliques, or vbounds.
    topoorder: Vec<i32>,
    /// For each bound index the bound indices influenced through variable
    /// bounds.
    vboundboundedidx: Vec<Vec<i32>>,
    vboundcoefs: Vec<Vec<f64>>,
    vboundconstants: Vec<Vec<f64>>,
    /// Number of bounds of variables regarded (two times number of active
    /// variables).
    nbounds: i32,
    /// Number of cliques created until the last call to the presolver.
    lastpresolncliques: i32,
    /// Bounds (topoorder position + 1) to propagate.
    propqueue: PQueue<usize>,
    /// Whether a topoorder position is already in propqueue.
    inqueue: Vec<bool>,
    initialized: bool,
    minnewcliques: f64,
    maxcliquesmedium: f64,
    maxcliquesexhaustive: f64,
    usebdwidening: bool,
    useimplics: bool,
    usecliques: bool,
    usevbounds: bool,
    dotoposort: bool,
    sortcliques: bool,
    detectcycles: bool,
}

fn pd(scip: &Scip, prop: PropId) -> &PropData {
    scip.props[prop].propdata.as_ref().unwrap().downcast_ref::<PropData>().unwrap()
}

fn pdm(scip: &mut Scip, prop: PropId) -> &mut PropData {
    scip.props[prop].propdata.as_mut().unwrap().downcast_mut::<PropData>().unwrap()
}

/// Comparison method for two indices in the topoorder array, preferring
/// higher indices because the order is reverse topological.
fn comp_varbound_indices(elem1: &usize, elem2: &usize) -> i32 {
    *elem2 as i32 - *elem1 as i32
}

/*
 * inference information: pos in the low 31 bits, the boundtype in bit 31
 */

fn infer_info_get_boundtype(inferinfo: i32) -> BoundType {
    if (inferinfo as u32) >> 31 == 0 { BoundType::Lower } else { BoundType::Upper }
}

fn infer_info_get_pos(inferinfo: i32) -> i32 {
    ((inferinfo as u32) & 0x7FFF_FFFF) as i32
}

fn get_infer_info(pos: i32, boundtype: BoundType) -> i32 {
    (((pos as u32) & 0x7FFF_FFFF) | ((boundtype as u32) << 31)) as i32
}

/*
 * Local methods
 */

/// `varGetLbIndex`: returns the lower bound index of a variable.
fn var_get_lb_index(propdata: &PropData, var: VarId) -> i32 {
    get_lb_index(propdata.varhashmap.get(&var).copied().unwrap_or(-1))
}

/// `varGetUbIndex`: returns the upper bound index of a variable.
fn var_get_ub_index(propdata: &PropData, var: VarId) -> i32 {
    get_ub_index(propdata.varhashmap.get(&var).copied().unwrap_or(-1))
}

/// `resetPropdata`: reset propagation data.
fn reset_propdata(propdata: &mut PropData) {
    propdata.vars = Vec::new();
    propdata.varhashmap = Default::default();
    propdata.topoorder = Vec::new();
    propdata.vboundboundedidx = Vec::new();
    propdata.vboundcoefs = Vec::new();
    propdata.vboundconstants = Vec::new();
    propdata.nbounds = 0;
    propdata.initialized = false;
}

/// `catchEvents`: catches events for variables.
fn catch_events(scip: &mut Scip, prop: PropId) -> ScipResult {
    let eventhdlr = pd(scip, prop).eventhdlr;
    let nbounds = pd(scip, prop).nbounds as usize;

    // Setup events.
    for v in 0..nbounds {
        let (var, lower, skip) = {
            let p = pd(scip, prop);
            let idx = p.topoorder[v];
            let var = p.vars[get_var_index(idx)];
            let lower = is_index_lowerbound(idx);
            (
                var,
                lower,
                p.vboundboundedidx[idx as usize].is_empty() && var_get_n_impls(scip, var, lower) == 0 && var_get_n_cliques(scip, var, lower) == 0,
            )
        };

        // If the bound does not influence another bound by implications,
        // cliques, or vbounds, we do not create an event and do not catch
        // changes of the bound; we mark this by setting the value in
        // topoorder to -1.
        if skip {
            pdm(scip, prop).topoorder[v] = -1;
            continue;
        }

        // Determine eventtype that we want to catch depending on boundtype
        // of variable.
        let eventtype = if lower {
            EVENTTYPE_LBTIGHTENED | EVENTTYPE_GLBCHANGED
        } else {
            EVENTTYPE_UBTIGHTENED | EVENTTYPE_GUBCHANGED
        };

        api::catch_var_event(scip, var, eventtype, eventhdlr, v, None)?;
    }

    Ok(())
}

/// `dropEvents`: drops events for variables.
fn drop_events(scip: &mut Scip, prop: PropId) -> ScipResult {
    let eventhdlr = pd(scip, prop).eventhdlr;
    let nbounds = pd(scip, prop).nbounds as usize;

    for v in 0..nbounds {
        let idx = pd(scip, prop).topoorder[v];

        if idx == -1 {
            continue;
        }

        let var = pd(scip, prop).vars[get_var_index(idx)];
        let lower = is_index_lowerbound(idx);

        // Determine eventtype that we catch and now want to drop depending on
        // boundtype of variable.
        let eventtype = if lower {
            EVENTTYPE_LBTIGHTENED | EVENTTYPE_GLBCHANGED
        } else {
            EVENTTYPE_UBTIGHTENED | EVENTTYPE_GUBCHANGED
        };

        api::drop_var_event(scip, var, eventtype, eventhdlr, v, -1)?;
    }

    Ok(())
}

/// `addVbound`: adds a vbound to the propagator data to store it internally
/// and allow forward propagation. (The C array growth only sizes memory.)
fn add_vbound(propdata: &mut PropData, startidx: i32, endidx: i32, coef: f64, constant: f64) {
    let s = startidx as usize;
    propdata.vboundboundedidx[s].push(endidx);
    propdata.vboundcoefs[s].push(coef);
    propdata.vboundconstants[s].push(constant);
}

/// `extractCycle`: extract bound changes or infeasibility information from a
/// cycle in the variable bound graph detected during depth-first search.
fn extract_cycle(scip: &mut Scip, prop: PropId, dfsstack: &[i32], stacknextedge: &[i32], stacksize: usize, samebound: bool, infeasible: &mut bool) -> ScipResult {
    let mut coef = 1.0;
    let mut constant = 0.0;

    // The last element on the stack is the end-point of the cycle.
    let cycleidx = dfsstack[stacksize - 1];

    // The same bound of the variable was visited already earlier on the
    // current path, so the start-point of the cycle has the same index;
    // otherwise the other bound of the variable was visited earlier.
    let startidx = if samebound { cycleidx } else { get_other_bound_index(cycleidx) };

    // Search for the start-point of the cycle; since the endpoint is at
    // position stacksize - 1 we start at stacksize - 2 and go backwards.
    let mut j = stacksize as i32 - 2;
    while j >= 0 && dfsstack[j as usize] != startidx {
        j -= 1;
    }

    while j < stacksize as i32 - 1 {
        let ju = j as usize;
        let p = pd(scip, prop);
        let currvar = p.vars[get_var_index(dfsstack[ju])];
        let currlower = is_index_lowerbound(dfsstack[ju]);

        // If we do not use implications, we assume the number of
        // implications to be 0 (as we did before).
        let ntmpimpls = if p.useimplics { var_get_n_impls(scip, currvar, currlower) } else { 0 };

        // stacknextedge[j] <= 0 --> the last outgoing edge traversed during
        // dfs starting from node dfsstack[j] was given by a clique.
        if stacknextedge[ju] <= 0 {
            let nextlower = is_index_lowerbound(dfsstack[ju + 1]);

            // In cases lb(x) -> lb(y) and ub(x) -> ub(y) coef=1.0 and
            // constant=0.0; in the others coef=-1.0 and constant=1.0.
            if currlower != nextlower {
                coef = -coef;
                constant = -constant + 1.0;
            }
        }
        // stacknextedge[j] > 0 --> the last outgoing edge traversed was given
        // by an implication or vbound. Implications are looked at first.
        else if stacknextedge[ju] <= ntmpimpls {
            let implics = scip.vars[currvar].implics.as_ref().unwrap();
            let k = (stacknextedge[ju] - 1) as usize;
            let impltype = implics.types[currlower as usize][k];
            let implbound = implics.bounds[currlower as usize][k];
            let nextvar = p.vars[get_var_index(dfsstack[ju + 1])];

            let newconstant;

            let newcoef = if impltype == BoundType::Lower {
                let mut c = implbound - scip.vars[nextvar].locdom.lb;
                if currlower {
                    newconstant = scip.vars[nextvar].locdom.lb;
                } else {
                    newconstant = implbound;
                    c *= -1.0;
                }
                c
            } else {
                let mut c = scip.vars[nextvar].locdom.ub - implbound;
                if currlower {
                    newconstant = scip.vars[nextvar].locdom.ub;
                    c *= -1.0;
                } else {
                    newconstant = implbound;
                }
                c
            };

            coef *= newcoef;
            constant = constant * newcoef + newconstant;
        }
        // The edge was given by a variable bound relation.
        else {
            let k = (stacknextedge[ju] - ntmpimpls - 1) as usize;
            let s = dfsstack[ju] as usize;
            coef *= p.vboundcoefs[s][k];
            constant = constant * p.vboundcoefs[s][k] + p.vboundconstants[s][k];
        }
        j += 1;
    }

    let mut islower = is_index_lowerbound(cycleidx);

    // We have a relation x <=/>= coef * x + constant now.
    if scip.set.is_eq(coef, 1.0) {
        if (islower && scip.set.is_feas_positive(constant)) || (!islower && scip.set.is_feas_negative(constant)) {
            *infeasible = true;
        }
    } else {
        let newbound = constant / (1.0 - coef);

        if scip.set.is_gt(coef, 1.0) {
            islower = !islower;
        }

        let var = pd(scip, prop).vars[get_var_index(cycleidx)];
        let (inf, _tightened) = if islower {
            scip_var::tighten_var_lb(scip, var, newbound, false)?
        } else {
            scip_var::tighten_var_ub(scip, var, newbound, false)?
        };
        *infeasible = inf;
    }

    Ok(())
}

/// Node visited (dfs).
const VISITED: i32 = 1;
/// Node active on the current path (dfs with cycle detection).
const ACTIVE: i32 = 2;

/// Whether the variable of bound index `idx` is globally fixed
/// (`SCIPisFeasGE(lbglobal, ubglobal)`).
fn globally_fixed(scip: &Scip, var: VarId) -> bool {
    scip.set.is_feas_ge(scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub)
}

/// `dfs`: performs depth-first-search in the implicitly given directed graph
/// from the given start index.
#[allow(clippy::too_many_arguments)]
fn dfs(
    scip: &mut Scip,
    prop: PropId,
    startnode: i32,
    visited: &mut [i32],
    dfsstack: &mut [i32],
    stacknextedge: &mut [i32],
    dfsnodes: &mut [i32],
    ndfsnodes: &mut i32,
    infeasible: &mut bool,
) -> ScipResult {
    let (detectcycles, sortcliques, usecliques, useimplics, usevbounds) = {
        let p = pd(scip, prop);
        (p.detectcycles, p.sortcliques, p.usecliques, p.useimplics, p.usevbounds)
    };
    // For cycle detection, we need to mark currently active nodes,
    // otherwise we just mark them as visited.
    let visitedflag = if detectcycles { ACTIVE } else { VISITED };

    *infeasible = false;

    // Put start node on the stack.
    dfsstack[0] = startnode;
    stacknextedge[0] = 0;
    let mut stacksize = 1usize;
    let mut idx: i32;

    // We run until no more bounds indices are on the stack, i.e. all changed
    // bounds were propagated.
    'outer: while stacksize > 0 {
        // Get next node from stack.
        let curridx = dfsstack[stacksize - 1];

        // Mark current node as visited.
        visited[curridx as usize] = visitedflag;

        let startvar = pd(scip, prop).vars[get_var_index(curridx)];
        let lower = is_index_lowerbound(curridx);

        // If the variable was fixed in the meantime, it is a loose end in the
        // variable bound graph.
        let fixed = globally_fixed(scip, startvar);

        if !fixed {
            let mut nimpls = 0;

            if sortcliques && usecliques && stacknextedge[stacksize - 1] == 0 {
                stacknextedge[stacksize - 1] = -1;
            }

            // stacknextedge is negative, if the last visited edge from the
            // current node belongs to a clique; the index of the clique in
            // the variable's clique list equals abs(stacknextedge) - 1.
            if sortcliques && usecliques && stacknextedge[stacksize - 1] < 0 {
                let cliques = var_cliques(scip, startvar, lower);
                let ncliques = cliques.len() as i32;
                let mut found = false;
                idx = -1;

                // Iterate over all not yet handled cliques and search for an
                // unvisited node.
                let mut j = -stacknextedge[stacksize - 1] - 1;
                while j < ncliques {
                    let c = &scip.cliques[cliques[j as usize]];
                    let cliquevars: Vec<VarId> = c.vars.0[..c.nvars as usize].to_vec();
                    let cliquevals: Vec<bool> = c.values.0[..c.nvars as usize].to_vec();

                    for i in 0..cliquevars.len() {
                        if cliquevars[i] == startvar {
                            continue;
                        }

                        idx = if cliquevals[i] {
                            var_get_ub_index(pd(scip, prop), cliquevars[i])
                        } else {
                            var_get_lb_index(pd(scip, prop), cliquevars[i])
                        };

                        // We reached a variable that was already visited on
                        // the active path, so we have a cycle.
                        if idx >= 0 && (visited[idx as usize] == ACTIVE || visited[get_other_bound_index(idx) as usize] == ACTIVE) && !globally_fixed(scip, cliquevars[i]) {
                            dfsstack[stacksize] = idx;
                            stacknextedge[stacksize - 1] = -j - 2;

                            let samebound = visited[idx as usize] == ACTIVE;
                            extract_cycle(scip, prop, dfsstack, stacknextedge, stacksize + 1, samebound, infeasible)?;

                            if *infeasible {
                                break;
                            }
                        }

                        // Break when the first unvisited node is reached.
                        if idx >= 0 && visited[idx as usize] == 0 {
                            found = true;
                            break;
                        }
                    }
                    if found {
                        break;
                    }
                    j += 1;
                }

                // We stopped because we found an unhandled node and not
                // because we reached the end of the list.
                if found {
                    // Put the adjacent node onto the stack.
                    dfsstack[stacksize] = idx;
                    stacknextedge[stacksize] = 0;
                    stacknextedge[stacksize - 1] = -j - 2;
                    stacksize += 1;

                    // Restart while loop, get next index from stack.
                    continue 'outer;
                } else {
                    // We did not find an edge to an unhandled node given by a
                    // clique.
                    stacknextedge[stacksize - 1] = 0;
                }
            }

            // Go over edges given by implications.
            if useimplics {
                nimpls = var_get_n_impls(scip, startvar, lower);

                if stacknextedge[stacksize - 1] < nimpls {
                    let (implvars, impltypes, implids) = {
                        let im = scip.vars[startvar].implics.as_ref().unwrap();
                        let n = nimpls as usize;
                        let l = lower as usize;
                        (im.vars[l].0[..n].to_vec(), im.types[l].0[..n].to_vec(), im.ids[l].0[..n].to_vec())
                    };

                    idx = -1;
                    let mut i = stacknextedge[stacksize - 1];
                    while i < nimpls {
                        let iu = i as usize;
                        // It might happen that implications point to inactive
                        // variables, we have to ignore these variables.
                        if !var_is_active(scip, implvars[iu]) {
                            i += 1;
                            continue;
                        }

                        // Implication is just a shortcut, so we dont regard
                        // it now, because will later go the long way, anyway;
                        // however, if we do regard cliques for the
                        // topological order, we use them to get a better
                        // order.
                        if usecliques && !sortcliques && implids[iu] < 0 {
                            i += 1;
                            continue;
                        }

                        idx = if impltypes[iu] == BoundType::Lower {
                            var_get_lb_index(pd(scip, prop), implvars[iu])
                        } else {
                            var_get_ub_index(pd(scip, prop), implvars[iu])
                        };

                        // We reached a variable that was already visited on
                        // the active path, so we have a cycle.
                        if idx >= 0 && (visited[idx as usize] == ACTIVE || visited[get_other_bound_index(idx) as usize] == ACTIVE) && !globally_fixed(scip, implvars[iu]) {
                            dfsstack[stacksize] = idx;
                            stacknextedge[stacksize - 1] = i + 1;

                            let samebound = visited[idx as usize] == ACTIVE;
                            extract_cycle(scip, prop, dfsstack, stacknextedge, stacksize + 1, samebound, infeasible)?;

                            if *infeasible {
                                break;
                            }
                        }

                        // Break when the first unvisited node is reached.
                        if idx >= 0 && visited[idx as usize] == 0 {
                            break;
                        }
                        i += 1;
                    }

                    // We stopped because we found an unhandled node and not
                    // because we reached the end of the list.
                    if i < nimpls {
                        // Put the adjacent node onto the stack.
                        dfsstack[stacksize] = idx;
                        stacknextedge[stacksize] = 0;
                        stacknextedge[stacksize - 1] = i + 1;
                        stacksize += 1;

                        // Restart while loop, get next index from stack.
                        continue 'outer;
                    } else {
                        stacknextedge[stacksize - 1] = nimpls;
                    }
                }
            }

            // Go over edges corresponding to varbounds.
            if usevbounds {
                let vboundidx = pd(scip, prop).vboundboundedidx[curridx as usize].clone();
                let nvbounds = vboundidx.len() as i32;
                idx = -1;

                // Iterate over all vbounds for the given bound.
                let mut i = stacknextedge[stacksize - 1] - nimpls;
                while i < nvbounds {
                    idx = vboundidx[i as usize];

                    if (visited[idx as usize] == ACTIVE || visited[get_other_bound_index(idx) as usize] == ACTIVE) && !globally_fixed(scip, pd(scip, prop).vars[get_var_index(idx)])
                    {
                        dfsstack[stacksize] = idx;
                        stacknextedge[stacksize - 1] = nimpls + i + 1;

                        // We reached a variable that was already visited on
                        // the active path, so we have a cycle.
                        let samebound = visited[idx as usize] == ACTIVE;
                        extract_cycle(scip, prop, dfsstack, stacknextedge, stacksize + 1, samebound, infeasible)?;

                        if *infeasible {
                            break;
                        }
                    }

                    // Break when the first unvisited node is reached.
                    if visited[idx as usize] == 0 {
                        break;
                    }
                    i += 1;
                }

                if *infeasible {
                    break 'outer;
                }

                // We stopped because we found an unhandled node and not
                // because we reached the end of the list.
                if i < nvbounds {
                    // Put the adjacent node onto the stack.
                    dfsstack[stacksize] = idx;
                    stacknextedge[stacksize] = 0;
                    stacknextedge[stacksize - 1] = nimpls + i + 1;
                    stacksize += 1;

                    // Restart while loop, get next index from stack.
                    continue 'outer;
                }
            }
        }

        // REMOVE: the current node was completely handled, remove it from
        // stack.
        stacksize -= 1;

        // Store node in the sorted nodes array.
        dfsnodes[*ndfsnodes as usize] = curridx;
        visited[curridx as usize] = VISITED;
        *ndfsnodes += 1;
    }

    Ok(())
}

/// The cliques of `var` for the given fixing (`SCIPvarGetCliques`).
fn var_cliques(scip: &Scip, var: VarId, varfixing: bool) -> Vec<usize> {
    match scip.vars[var].cliquelist.as_ref() {
        Some(l) => l.cliques[varfixing as usize].0[..l.ncliques[varfixing as usize] as usize].to_vec(),
        None => Vec::new(),
    }
}

/// `topologicalSort`: sort the bounds of variables topologically.
fn topological_sort(scip: &mut Scip, prop: PropId, infeasible: &mut bool) -> ScipResult {
    let nbounds = pd(scip, prop).nbounds as usize;

    // One extra slot: cycle detection writes the closing node behind the
    // stack top.
    let mut dfsstack = vec![0i32; nbounds + 1];
    let mut stacknextedge = vec![0i32; nbounds + 1];
    let mut visited = vec![0i32; nbounds];
    let mut topoorder = std::mem::take(&mut pdm(scip, prop).topoorder);

    let mut nsortednodes = 0;

    // While there are unvisited nodes, run dfs starting from one of these
    // nodes; the dfs orders are stored in the topoorder array, later dfs
    // calls are just appended after the stacks of previous dfs calls, which
    // gives us a reverse topological order.
    let mut ret = Ok(());
    let mut i = 0;
    while i < nbounds && !*infeasible {
        if visited[i] == 0 {
            ret = dfs(
                scip,
                prop,
                i as i32,
                &mut visited,
                &mut dfsstack,
                &mut stacknextedge,
                &mut topoorder,
                &mut nsortednodes,
                infeasible,
            );
            if ret.is_err() {
                break;
            }
        }
        i += 1;
    }
    pdm(scip, prop).topoorder = topoorder;

    ret
}

/// `initData`: initializes the internal data for the variable bounds
/// propagator.
fn init_data(scip: &mut Scip, prop: PropId, infeasible: &mut bool) -> ScipResult {
    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();
    let nbounds = 2 * nvars;

    *infeasible = false;

    // Store size of the bounds of variables array.
    pdm(scip, prop).nbounds = nbounds as i32;

    if nbounds == 0 {
        return Ok(());
    }

    {
        let p = pdm(scip, prop);
        p.initialized = true;

        // Prepare priority queue structure.
        p.propqueue = PQueue::new(nvars);
        p.inqueue = vec![false; nbounds];

        // We need to copy the variable since this array is the basis of the
        // propagator and the corresponding variable array within SCIP might
        // change during the search.
        p.vars = vars.clone();
        p.varhashmap = vars.iter().enumerate().map(|(v, &var)| (var, v as i32)).collect();

        // Allocate memory for the arrays of the propdata.
        p.topoorder = (0..nbounds as i32).collect();
        p.vboundboundedidx = vec![Vec::new(); nbounds];
        p.vboundcoefs = vec![Vec::new(); nbounds];
        p.vboundconstants = vec![Vec::new(); nbounds];
    }

    // Collect information about varbounds.
    for v in 0..nbounds as i32 {
        let var = vars[get_var_index(v)];
        let lower = is_index_lowerbound(v);

        // Get the variable bound informations for the current variable.
        let (vbvars, coefs, constants) = {
            let vb = if lower { scip.vars[var].vlbs.as_ref() } else { scip.vars[var].vubs.as_ref() };
            match vb {
                Some(vb) => {
                    let n = vb.len as usize;
                    (vb.vars.0[..n].to_vec(), vb.coefs.0[..n].to_vec(), vb.constants.0[..n].to_vec())
                }
                None => (Vec::new(), Vec::new(), Vec::new()),
            }
        };

        // Loop over all variable bounds; a variable lower bound has the form:
        // x >= b*y + d, a variable upper bound the form x <= b*y + d.
        for n in 0..vbvars.len() {
            let mut vbvar = vbvars[n];
            let mut coef = coefs[n];
            let mut constant = constants[n];

            // Transform variable bound variable to an active variable, if
            // possible.
            crate::var::var_get_probvar_sum(scip, &mut vbvar, &mut coef, &mut constant)?;

            if !var_is_active(scip, vbvar) {
                continue;
            }

            // If the coefficient is positive, the type of bound is the same
            // for the bounded and the bounding variable.
            let p = pd(scip, prop);
            let startidx = if scip.set.is_positive(coef) {
                if lower { var_get_lb_index(p, vbvar) } else { var_get_ub_index(p, vbvar) }
            } else if lower {
                var_get_ub_index(p, vbvar)
            } else {
                var_get_lb_index(p, vbvar)
            };

            // If the vbvar is binary, the vbound should be stored as an
            // implication already. However, it might happen that vbvar was
            // integer when the variable bound was added, but was converted to
            // a binary variable later during presolving when its upper bound
            // was changed to 1. In this case, the implication might not have
            // been created.
            if !(scip.vars[vbvar].vartype == VarType::Binary
                && !var_is_implied_integral(scip, vbvar)
                && crate::var::var_has_implic(scip, vbvar, is_index_lowerbound(startidx), var, get_boundtype(v)))
            {
                add_vbound(pdm(scip, prop), startidx, v, coef, constant);
            }
        }
    }

    // Sort the bounds topologically.
    if pd(scip, prop).dotoposort {
        topological_sort(scip, prop, infeasible)?;
    }

    // Catch variable events.
    catch_events(scip, prop)
}

/// `resolvePropagation`: resolves a propagation by adding the variable which
/// implied that bound change.
fn resolve_propagation(scip: &mut Scip, var: VarId, boundtype: BoundType, bdchgidx: Option<&BdChgIdx>) -> ScipResult {
    match boundtype {
        BoundType::Lower => api::add_conflict_lb(scip, var, bdchgidx),
        BoundType::Upper => api::add_conflict_ub(scip, var, bdchgidx),
    }
}

/// `relaxVbdvar`: relaxes bound of give variable as long as the given
/// inference bound still leads to a cutoff and add that bound change to the
/// conflict set.
fn relax_vbdvar(scip: &mut Scip, var: VarId, boundtype: BoundType, bdchgidx: Option<&BdChgIdx>, relaxedbd: f64) -> ScipResult {
    if boundtype == BoundType::Lower {
        api::add_conflict_relaxed_lb(scip, var, bdchgidx, relaxedbd)
    } else {
        api::add_conflict_relaxed_ub(scip, var, bdchgidx, relaxedbd)
    }
}

/// `computeRelaxedLowerbound`: compute the relaxed bound which is sufficient
/// to propagate the inference lower bound of given variable.
fn compute_relaxed_lowerbound(scip: &Scip, var: VarId, inferlb: f64, coef: f64, constant: f64) -> f64 {
    let feastol = scip.set.feastol();
    let mut relaxedbd = if var_is_integral(scip, var) && inferlb < scip.set.get_huge_value() * feastol {
        (inferlb - 1.0 + 2.0 * feastol - constant) / coef
    } else {
        (inferlb - constant) / coef
    };

    if coef > 0.0 {
        relaxedbd += feastol;
    } else {
        relaxedbd -= feastol;
    }

    relaxedbd
}

/// `computeRelaxedUpperbound`: compute the relaxed bound which is sufficient
/// to propagate the inference upper bound of given variable.
fn compute_relaxed_upperbound(scip: &Scip, var: VarId, inferub: f64, coef: f64, constant: f64) -> f64 {
    let feastol = scip.set.feastol();
    let mut relaxedbd = if var_is_integral(scip, var) && inferub < scip.set.get_huge_value() * feastol {
        (inferub + 1.0 - 2.0 * feastol - constant) / coef
    } else {
        (inferub - constant) / coef
    };

    if coef > 0.0 {
        relaxedbd -= feastol;
    } else {
        relaxedbd += feastol;
    }

    relaxedbd
}

/// `analyzeConflictLowerbound`: analyzes an infeasibility which was reached
/// by updating the lower bound of the inference variable above its upper
/// bound.
#[allow(clippy::too_many_arguments)]
fn analyze_conflict_lowerbound(
    scip: &mut Scip,
    prop: PropId,
    infervar: VarId,
    mut inferlb: f64,
    vbdvar: VarId,
    boundtype: BoundType,
    coef: f64,
    constant: f64,
    canwide: bool,
) -> ScipResult {
    // Check if conflict analysis is applicable.
    if !api::is_conflict_analysis_applicable(scip) {
        return Ok(());
    }

    if canwide && pd(scip, prop).usebdwidening {
        // Initialize conflict analysis.
        api::init_conflict_analysis(scip, ConfType::Propagation, false)?;

        // Adjust lower bound.
        inferlb = scip_var::adjusted_var_lb(scip, infervar, inferlb);

        // Compute a relaxed upper bound which would be sufficient to be still
        // infeasible.
        let mut relaxedub = if var_is_integral(scip, infervar) {
            inferlb - 1.0
        } else {
            inferlb - 2.0 * scip.set.feastol()
        };

        // Try to relax inference variable upper bound such that the
        // infeasibility is still given.
        api::add_conflict_relaxed_ub(scip, infervar, None, relaxedub)?;

        // Collect the upper bound which is reported to the conflict analysis.
        relaxedub = api::get_conflict_var_ub(scip, infervar);

        // Adjust inference bound with respect to the upper bound reported to
        // the conflict analysis.
        if var_is_integral(scip, infervar) {
            relaxedub += 1.0;
        } else {
            relaxedub += 2.0 * scip.set.feastol();
        }

        // Compute the relaxed bound which is sufficient to propagate the
        // inference lower bound of given variable.
        let relaxedbd = compute_relaxed_lowerbound(scip, infervar, relaxedub, coef, constant);

        // Try to relax variable bound variable.
        relax_vbdvar(scip, vbdvar, boundtype, None, relaxedbd)?;

        // Analyze the conflict.
        crate::scip_lp::analyze_conflict(scip, 0)?;
    } else {
        // Initialize conflict analysis.
        api::init_conflict_analysis(scip, ConfType::Propagation, false)?;

        // Add upper bound of the variable for which we tried to change the
        // lower bound.
        api::add_conflict_ub(scip, infervar, None)?;

        // Add (correct) bound of the variable which let to the new lower
        // bound.
        resolve_propagation(scip, vbdvar, boundtype, None)?;

        // Analyze the conflict.
        crate::scip_lp::analyze_conflict(scip, 0)?;
    }

    Ok(())
}

/// `analyzeConflictUpperbound`: analyzes an infeasibility which was reached
/// by updating the upper bound of the inference variable below its lower
/// bound.
#[allow(clippy::too_many_arguments)]
fn analyze_conflict_upperbound(
    scip: &mut Scip,
    prop: PropId,
    infervar: VarId,
    mut inferub: f64,
    vbdvar: VarId,
    boundtype: BoundType,
    coef: f64,
    constant: f64,
    canwide: bool,
) -> ScipResult {
    // Check if conflict analysis is applicable.
    if !api::is_conflict_analysis_applicable(scip) {
        return Ok(());
    }

    if canwide && pd(scip, prop).usebdwidening {
        // Initialize conflict analysis.
        api::init_conflict_analysis(scip, ConfType::Propagation, false)?;

        // Adjust upper bound.
        inferub = scip_var::adjusted_var_ub(scip, infervar, inferub);

        // Compute a relaxed lower bound which would be sufficient to be still
        // infeasible.
        let mut relaxedlb = if var_is_integral(scip, infervar) {
            inferub + 1.0
        } else {
            inferub + 2.0 * scip.set.feastol()
        };

        // Try to relax inference variable lower bound such that the
        // infeasibility is still given.
        api::add_conflict_relaxed_lb(scip, infervar, None, relaxedlb)?;

        // Collect the lower bound which is reported to the conflict analysis.
        relaxedlb = api::get_conflict_var_lb(scip, infervar);

        // Adjust inference bound with respect to the upper bound reported to
        // the conflict analysis.
        if var_is_integral(scip, infervar) {
            relaxedlb -= 1.0;
        } else {
            relaxedlb -= 2.0 * scip.set.feastol();
        }

        // Compute the relaxed bound which is sufficient to propagate the
        // inference upper bound of given variable.
        let relaxedbd = compute_relaxed_upperbound(scip, infervar, relaxedlb, coef, constant);

        // Try to relax variable bound variable.
        relax_vbdvar(scip, vbdvar, boundtype, None, relaxedbd)?;

        // Analyze the conflict.
        crate::scip_lp::analyze_conflict(scip, 0)?;
    } else {
        // Initialize conflict analysis.
        api::init_conflict_analysis(scip, ConfType::Propagation, false)?;

        // Add lower bound of the variable for which we tried to change the
        // upper bound.
        api::add_conflict_lb(scip, infervar, None)?;

        // Add (correct) bound of the variable which let to the new upper
        // bound.
        resolve_propagation(scip, vbdvar, boundtype, None)?;

        // Analyze the conflict.
        crate::scip_lp::analyze_conflict(scip, 0)?;
    }

    Ok(())
}

/// `tightenVarLb`: tries to tighten the (global) lower bound of the given
/// variable to the given new bound.
#[allow(clippy::too_many_arguments)]
fn tighten_var_lb(
    scip: &mut Scip,
    prop: PropId,
    var: VarId,
    newlb: f64,
    global: bool,
    vbdvar: VarId,
    boundtype: BoundType,
    mut force: bool,
    coef: f64,
    constant: f64,
    canwide: bool,
    nchgbds: &mut i32,
    result: &mut ScipResultCode,
) -> ScipResult {
    let lb = scip.vars[var].locdom.lb;

    // Check that the new upper bound is better.
    force = (var_is_integral(scip, var) && newlb - lb > 0.5) || (force && scip.set.is_gt(newlb, lb));

    // Try to tighten the lower bound.
    let (infeasible, tightened) = if global {
        scip_var::tighten_var_lb_global(scip, var, newlb, force)?
    } else {
        let p = pd(scip, prop);
        let inferinfo = get_infer_info(
            if boundtype == BoundType::Lower {
                var_get_lb_index(p, vbdvar)
            } else {
                var_get_ub_index(p, vbdvar)
            },
            boundtype,
        );

        scip_var::infer_var_lb_prop(scip, var, newlb, prop, inferinfo, force)?
    };

    if infeasible {
        // The infeasible results comes from the fact that the new lower bound
        // lies above the current upper bound.
        if global {
            // Cutoff the root node.
            let root = api::get_root_node(scip);
            api::cutoff_node(scip, root)?;
        } else {
            // Analyzes a infeasibility via conflict analysis.
            analyze_conflict_lowerbound(scip, prop, var, newlb, vbdvar, boundtype, coef, constant, canwide)?;
        }
        *result = ScipResultCode::Cutoff;
    } else if tightened {
        *nchgbds += 1;
    }

    Ok(())
}

/// `tightenVarUb`: tries to tighten the (global) upper bound of the given
/// variable to the given new bound.
#[allow(clippy::too_many_arguments)]
fn tighten_var_ub(
    scip: &mut Scip,
    prop: PropId,
    var: VarId,
    newub: f64,
    global: bool,
    vbdvar: VarId,
    boundtype: BoundType,
    mut force: bool,
    coef: f64,
    constant: f64,
    canwide: bool,
    nchgbds: &mut i32,
    result: &mut ScipResultCode,
) -> ScipResult {
    let ub = scip.vars[var].locdom.ub;

    // Check that the new upper bound is better.
    force = (var_is_integral(scip, var) && ub - newub > 0.5) || (force && scip.set.is_lt(newub, ub));

    // Try to tighten the upper bound.
    let (infeasible, tightened) = if global {
        scip_var::tighten_var_ub_global(scip, var, newub, force)?
    } else {
        let p = pd(scip, prop);
        let inferinfo = get_infer_info(
            if boundtype == BoundType::Lower {
                var_get_lb_index(p, vbdvar)
            } else {
                var_get_ub_index(p, vbdvar)
            },
            boundtype,
        );

        scip_var::infer_var_ub_prop(scip, var, newub, prop, inferinfo, force)?
    };

    if infeasible {
        // The infeasible results comes from the fact that the new upper bound
        // lies below the current lower bound.
        if global {
            // Cutoff the root node.
            let root = api::get_root_node(scip);
            api::cutoff_node(scip, root)?;
        } else {
            // Analyzes a infeasibility via conflict analysis.
            analyze_conflict_upperbound(scip, prop, var, newub, vbdvar, boundtype, coef, constant, canwide)?;
        }
        *result = ScipResultCode::Cutoff;
    } else if tightened {
        *nchgbds += 1;
    }

    Ok(())
}

/// `propagateVbounds`: performs propagation of variables lower and upper
/// bounds, implications, and cliques.
fn propagate_vbounds(scip: &mut Scip, prop: PropId, force: bool, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // We do not run the propagator in presolving, because we want to avoid
    // doing the expensive creation of the graph twice.
    if scip.set.stage == Stage::Presolving {
        return Ok(());
    }

    // Initialize propagator data needed for propagation, if not done yet.
    if !pd(scip, prop).initialized {
        let mut infeasible = false;

        init_data(scip, prop, &mut infeasible)?;

        if infeasible {
            *result = ScipResultCode::Cutoff;
            return Ok(());
        }
    }

    let nbounds = pd(scip, prop).nbounds;

    if nbounds == 0 {
        return Ok(());
    }

    // Propagate all variables if we are in repropagation.
    if crate::tree::tree_in_repropagation(scip) {
        for v in (0..nbounds as usize).rev() {
            let p = pd(scip, prop);
            let idx = p.topoorder[v];
            if idx != -1 && !p.inqueue[v] {
                let var = p.vars[get_var_index(idx)];
                let lower = is_index_lowerbound(idx);
                if !var_is_binary(scip, var) || (lower && scip.vars[var].locdom.lb > 0.5) || (!lower && scip.vars[var].locdom.ub < 0.5) {
                    let p = pdm(scip, prop);
                    p.propqueue.insert(v + 1, comp_varbound_indices, |_, _, _| {});
                    p.inqueue[v] = true;
                }
            }
        }
    }

    // Return if no bound changes are in the priority queue (no changed
    // bounds to handle since last propagation).
    if pd(scip, prop).propqueue.n_elems() == 0 {
        return Ok(());
    }

    let mut nchgbds = 0;

    *result = ScipResultCode::DidNotFind;

    // Space for variables added to the queue - needed to clean up data.
    let mut queuelist: Vec<usize> = Vec::with_capacity(nbounds as usize);

    let (useimplics, usecliques, usevbounds) = {
        let p = pd(scip, prop);
        (p.useimplics, p.usecliques, p.usevbounds)
    };

    // Get variable bound of highest priority from priority queue and try to
    // deduce bound changes for other variables; the priority queue is
    // ordered w.r.t the topological sort of the varbound graph.
    while pd(scip, prop).propqueue.n_elems() > 0 {
        let topopos = pdm(scip, prop).propqueue.remove(comp_varbound_indices, |_, _, _| {}).unwrap() - 1;
        let (startpos, startvar) = {
            let p = pd(scip, prop);
            let startpos = p.topoorder[topopos];
            (startpos, p.vars[get_var_index(startpos)])
        };
        queuelist.push(topopos);

        // Do not directly set inqueue[topopos] = FALSE: we allow only one
        // propagation sweep through the topologically ordered bounds;
        // otherwise an infinite loop could occur.

        let starttype = get_boundtype(startpos);
        let lower = starttype == BoundType::Lower;
        let v = &scip.vars[startvar];
        let startbound = if lower { v.locdom.lb } else { v.locdom.ub };
        let globalbound = if lower { v.glbdom.lb } else { v.glbdom.ub };
        let global = scip.set.is_eq(startbound, globalbound);

        if var_is_binary(scip, startvar) {
            // We only propagate binary variables if the lower bound changed to
            // 1.0 or the upper bound changed to 0.0.
            if lower != (startbound > 0.5) {
                continue;
            }

            // Propagate implications.
            if useimplics {
                // If the lower bound of the startvar was changed, it was fixed
                // to 1.0, otherwise it was fixed to 0.0; get all implications
                // for this varfixing.
                let nimplvars = var_get_n_impls(scip, startvar, lower) as usize;

                // If there are implications for the varfixing, propagate them.
                if nimplvars > 0 {
                    let (implvars, impltypes, implbounds, implids) = {
                        let im = scip.vars[startvar].implics.as_ref().unwrap();
                        let l = lower as usize;
                        (
                            im.vars[l].0[..nimplvars].to_vec(),
                            im.types[l].0[..nimplvars].to_vec(),
                            im.bounds[l].0[..nimplvars].to_vec(),
                            im.ids[l].0[..nimplvars].to_vec(),
                        )
                    };

                    for n in 0..nimplvars {
                        // Implication is just a shortcut, so we do not
                        // propagate it now, because we will propagate the
                        // longer way, anyway.
                        if implids[n] < 0 {
                            continue;
                        }

                        // It might happen that implications point to inactive
                        // variables, we have to ignore these variables.
                        if !var_is_active(scip, implvars[n]) {
                            continue;
                        }

                        if impltypes[n] == BoundType::Lower {
                            tighten_var_lb(
                                scip,
                                prop,
                                implvars[n],
                                implbounds[n],
                                global,
                                startvar,
                                starttype,
                                force,
                                0.0,
                                0.0,
                                false,
                                &mut nchgbds,
                                result,
                            )?;
                        } else {
                            tighten_var_ub(
                                scip,
                                prop,
                                implvars[n],
                                implbounds[n],
                                global,
                                startvar,
                                starttype,
                                force,
                                0.0,
                                0.0,
                                false,
                                &mut nchgbds,
                                result,
                            )?;
                        }

                        if *result == ScipResultCode::Cutoff {
                            break;
                        }
                    }
                    if *result == ScipResultCode::Cutoff {
                        break;
                    }
                }
            }

            // Propagate cliques.
            if usecliques {
                // If the lower bound of the startvar was changed, it was fixed
                // to 1.0, otherwise it was fixed to 0.0; get all cliques for
                // this varfixing.
                let cliques = var_cliques(scip, startvar, lower);

                // If there are cliques for the varfixing, propagate them.
                if !cliques.is_empty() {
                    for &clique in &cliques {
                        let (cliquevars, cliquevals) = {
                            let c = &scip.cliques[clique];
                            (c.vars.0[..c.nvars as usize].to_vec(), c.values.0[..c.nvars as usize].to_vec())
                        };

                        // Fix all variables except for the startvar to the
                        // value which is not in the clique.
                        for n in 0..cliquevars.len() {
                            if cliquevars[n] == startvar {
                                continue;
                            }

                            // Try to tighten the bound.
                            if cliquevals[n] {
                                // Unnegated variable is in clique, so it has
                                // to be fixed to 0.0.
                                tighten_var_ub(scip, prop, cliquevars[n], 0.0, global, startvar, starttype, force, 0.0, 0.0, false, &mut nchgbds, result)?;
                            } else {
                                // Negated variable is in clique, so it has to
                                // be fixed to 1.0.
                                tighten_var_lb(scip, prop, cliquevars[n], 1.0, global, startvar, starttype, force, 0.0, 0.0, false, &mut nchgbds, result)?;
                            }

                            if *result == ScipResultCode::Cutoff {
                                break;
                            }
                        }
                    }
                    if *result == ScipResultCode::Cutoff {
                        break;
                    }
                }
            }
        }

        // Propagate vbounds.
        if usevbounds && !scip.set.is_infinity(startbound.abs()) {
            let (bounded, coefs, constants) = {
                let p = pd(scip, prop);
                let s = startpos as usize;
                (p.vboundboundedidx[s].clone(), p.vboundcoefs[s].clone(), p.vboundconstants[s].clone())
            };

            // Iterate over all vbounds for the given bound.
            for n in 0..bounded.len() {
                let boundedvar = pd(scip, prop).vars[get_var_index(bounded[n])];
                let coef = coefs[n];
                let constant = constants[n];

                // Compute new bound.
                let newbound = startbound * coef + constant;

                // Try to tighten the bound.
                if is_index_lowerbound(bounded[n]) {
                    tighten_var_lb(
                        scip,
                        prop,
                        boundedvar,
                        newbound,
                        global,
                        startvar,
                        starttype,
                        force,
                        coef,
                        constant,
                        true,
                        &mut nchgbds,
                        result,
                    )?;
                } else {
                    tighten_var_ub(
                        scip,
                        prop,
                        boundedvar,
                        newbound,
                        global,
                        startvar,
                        starttype,
                        force,
                        coef,
                        constant,
                        true,
                        &mut nchgbds,
                        result,
                    )?;
                }

                if *result == ScipResultCode::Cutoff {
                    break;
                }
            }
            if *result == ScipResultCode::Cutoff {
                break;
            }
        }
    }

    // Clean up inqueue.
    let p = pdm(scip, prop);
    for &v in &queuelist {
        p.inqueue[v] = false;
    }

    // Set the result depending on whether bound changes were found or not.
    if *result != ScipResultCode::Cutoff && nchgbds > 0 {
        *result = ScipResultCode::ReducedDom;
    }

    Ok(())
}

/*
 * Callback methods of propagator
 */

/// `propCopyVbounds`: copy method for propagator plugins.
fn prop_copy_vbounds(scip: &mut Scip, _sourcescip: &Scip, _prop: PropId) -> ScipResult {
    include_prop_vbounds(scip)
}

/// `propInitpreVbounds`: presolving initialization method of propagator.
fn prop_initpre_vbounds(scip: &mut Scip, prop: PropId) -> ScipResult {
    pdm(scip, prop).lastpresolncliques = 0;
    Ok(())
}

/// `propExitsolVbounds`: solving process deinitialization method of
/// propagator.
fn prop_exitsol_vbounds(scip: &mut Scip, prop: PropId, _restart: bool) -> ScipResult {
    // Free data stored for propagation.
    if pd(scip, prop).initialized {
        // Drop all variable events.
        drop_events(scip, prop)?;

        let p = pdm(scip, prop);
        p.propqueue = PQueue::default();
        p.inqueue = Vec::new();
    }

    // Reset propagation data.
    reset_propdata(pdm(scip, prop));

    Ok(())
}

/// `tarjan`: performs Tarjan's algorithm for strongly connected components
/// in the implicitly given directed implication graph from the given start
/// index; each variable x is represented by two nodes lb(x) = 2*idx(x) and
/// ub(x) = 2*idx(x)+1. Each clique is entered at most twice: the first time
/// all its arcs from the entry node are traversed, the second time only the
/// arc to the negation of the first entry point. Infeasible fixings of
/// binary variables are identified on the way.
struct Tarjan<'a> {
    nodeonstack: &'a mut [bool],
    nodeindex: &'a mut [i32],
    nodelowlink: &'a mut [i32],
    nodeinfeasible: &'a mut [bool],
    dfsstack: &'a mut [i32],
    predstackidx: &'a mut [i32],
    stacknextclique: &'a mut [i32],
    stacknextcliquevar: &'a mut [i32],
    cliquefirstentry: &'a mut [i32],
    cliquecurrentexit: &'a mut [i32],
    sccvars: &'a mut [i32],
    sccstarts: &'a mut [i32],
    nsccs: &'a mut i32,
    infeasnodes: &'a mut [i32],
    ninfeasnodes: &'a mut i32,
}

fn tarjan(scip: &Scip, startnode: i32, startindex: &mut i32, t: &mut Tarjan, mut topoorder: Option<(&mut [i32], &mut i32)>, infeasible: &mut bool) {
    let mut label = *startindex;

    *infeasible = false;

    let vars = api::get_vars(scip);

    // Put start node on the stack.
    t.dfsstack[0] = startnode;
    t.stacknextclique[0] = 0;
    t.stacknextcliquevar[0] = 0;
    t.predstackidx[0] = -1;
    let mut stacksize: i32 = 1;
    let mut idx: i32 = -1;
    let mut currstackidx: i32 = 0;

    // We run until no more bounds indices are on the stack, i.e., no further
    // nodes are connected to the startnode.
    while stacksize > 0 {
        let mut clqidx: i32 = -1;
        let cs = currstackidx as usize;

        // Get next node from stack.
        let curridx = t.dfsstack[cs];
        let cu = curridx as usize;

        let startvar = vars[get_var_index(curridx)];
        let lower = is_index_lowerbound(curridx);

        // Mark current node as on stack, assign index and lowlink.
        if t.nodeindex[cu] == 0 {
            t.nodeonstack[cu] = true;
            t.nodeindex[cu] = label;
            t.nodelowlink[cu] = label;
            label += 1;
        }

        let cliques = var_cliques(scip, startvar, lower);
        let ncliques = cliques.len() as i32;
        let mut found = false;

        // Iterate over all not yet handled cliques and search for an
        // unvisited node.
        let mut j = t.stacknextclique[cs];
        while j < ncliques {
            let c = &scip.cliques[cliques[j as usize]];
            clqidx = c.index;
            let cq = clqidx as usize;
            let ncliquevars = c.nvars;
            let cliquevars = &c.vars.0[..ncliquevars as usize];
            let cliquevals = &c.values.0[..ncliquevars as usize];

            // We did not look at this clique before from the current node,
            // i.e., we did not backtrack now from another node which was
            // reached via this clique.
            if t.stacknextcliquevar[cs] == 0 {
                // The clique was not entered before, remember that we first
                // entered it from curridx (add 1 to distinguish it from 0
                // initialization).
                if t.cliquefirstentry[cq] == 0 {
                    t.cliquefirstentry[cq] = curridx + 1;
                } else {
                    let cliquefirstentryidx = (if t.cliquefirstentry[cq] > 0 { t.cliquefirstentry[cq] } else { -t.cliquefirstentry[cq] }) - 1;
                    let cfe = cliquefirstentryidx as usize;
                    let mut infeasnode = -1;

                    // The node by which we entered the clique the first time
                    // is still on the stack, so there is a way from that node
                    // to the node by which we are entering the clique right
                    // now. Since these two assignments together violate the
                    // clique and the second assignment is implied by the
                    // first, the first one is infeasible.
                    if t.nodeonstack[cfe] && !t.nodeinfeasible[cfe] {
                        infeasnode = cliquefirstentryidx;
                    }
                    // The first entry point of the clique was also implied by
                    // the current startnode, so this node implies two
                    // variables in the clique and is therefore infeasible.
                    else if t.nodeindex[cfe] >= *startindex && !t.nodeinfeasible[startnode as usize] {
                        infeasnode = startnode;
                    }

                    // We identified an infeasibility.
                    if infeasnode >= 0 {
                        // Both values are invalid for the variable, the whole
                        // problem is infeasible.
                        if t.nodeinfeasible[get_other_bound_index(infeasnode) as usize] {
                            *infeasible = true;
                            return;
                        }
                        t.infeasnodes[*t.ninfeasnodes as usize] = infeasnode;
                        t.nodeinfeasible[infeasnode as usize] = true;
                        *t.ninfeasnodes += 1;

                        // The last node by which the clique was exited is not
                        // the negation of the current node and still on the
                        // stack: update the lowlink of the current node.
                        let exit = t.cliquecurrentexit[cq];
                        if exit > 0 && curridx != get_other_bound_index(exit - 1) && t.nodeonstack[(exit - 1) as usize] && t.nodeindex[(exit - 1) as usize] < t.nodelowlink[cu] {
                            t.nodelowlink[cu] = t.nodeindex[(exit - 1) as usize];
                        }
                    }
                    // Clique is entered for the second time; there is only one
                    // edge left to investigate, namely the edge to the
                    // negation of the first entry point.
                    else if t.cliquefirstentry[cq] > 0 {
                        idx = get_other_bound_index(t.cliquefirstentry[cq] - 1);

                        // Node was not investigated yet, we found the next
                        // node to process.
                        if t.nodeindex[idx as usize] == 0 {
                            found = true;
                        }
                        // Update lowlink if the node is on the stack.
                        else if t.nodeonstack[idx as usize] && t.nodeindex[idx as usize] < t.nodelowlink[cu] {
                            t.nodelowlink[cu] = t.nodeindex[idx as usize];
                        }

                        // cliquefirstentry[clqidx] < 0 means that we entered
                        // the clique at least two times already.
                        t.cliquefirstentry[cq] = -t.cliquefirstentry[cq];
                    }
                    t.stacknextcliquevar[cs] = ncliquevars;
                }
            }

            // Iterate over variables in the clique; start where we stopped
            // last time.
            let mut i = t.stacknextcliquevar[cs];
            while i < ncliquevars {
                let iu = i as usize;
                if cliquevars[iu] == startvar || !var_is_active(scip, cliquevars[iu]) {
                    i += 1;
                    continue;
                }

                let probindex = scip.vars[cliquevars[iu]].probindex;
                idx = if cliquevals[iu] { get_ub_index(probindex) } else { get_lb_index(probindex) };

                // Break when the first unvisited node is reached.
                if t.nodeindex[idx as usize] == 0 {
                    t.stacknextcliquevar[cs] = i + 1;
                    found = true;
                    break;
                } else if t.nodeonstack[idx as usize] && t.nodeindex[idx as usize] < t.nodelowlink[cu] {
                    t.nodelowlink[cu] = t.nodeindex[idx as usize];
                }
                i += 1;
            }
            if found {
                if t.stacknextcliquevar[cs] < ncliquevars {
                    t.stacknextclique[cs] = j;
                } else {
                    t.stacknextclique[cs] = j + 1;
                    t.stacknextcliquevar[cs] = 0;
                }
                break;
            } else {
                t.stacknextclique[cs] = j + 1;
                t.stacknextcliquevar[cs] = 0;
            }
            j += 1;
        }

        // We stopped because we found an unhandled node and not because we
        // reached the end of the list.
        if found {
            let otheridx = get_other_bound_index(idx);
            let mut infeasnode = -1;

            // The negated node corresponding to the next node is already on
            // the stack -> the negated assignment is infeasible.
            if t.nodeonstack[otheridx as usize] && !t.nodeinfeasible[otheridx as usize] {
                infeasnode = otheridx;
            }
            // The negated node corresponding to the next node was reached from
            // the same startnode -> the startnode is infeasible.
            else if t.nodeindex[otheridx as usize] >= *startindex && !t.nodeinfeasible[startnode as usize] {
                infeasnode = startnode;
            }
            // Treat infeasible case.
            if infeasnode >= 0 {
                if t.nodeinfeasible[get_other_bound_index(infeasnode) as usize] {
                    *infeasible = true;
                    return;
                }
                t.infeasnodes[*t.ninfeasnodes as usize] = infeasnode;
                t.nodeinfeasible[infeasnode as usize] = true;
                *t.ninfeasnodes += 1;
            }

            // Put the adjacent node onto the stack.
            let ss = stacksize as usize;
            t.dfsstack[ss] = idx;
            t.stacknextclique[ss] = 0;
            t.stacknextcliquevar[ss] = 0;
            t.cliquecurrentexit[clqidx as usize] = idx + 1;
            t.predstackidx[ss] = currstackidx;
            currstackidx = stacksize;
            stacksize += 1;

            // Restart while loop, get next index from stack.
            continue;
        }

        // No node with a smaller index can be reached from this node -> it is
        // the root of a SCC, consisting of all nodes above it on the stack,
        // including the node itself.
        if t.nodelowlink[cu] == t.nodeindex[cu] {
            // We are only interested in SCCs with more than one node.
            if t.dfsstack[(stacksize - 1) as usize] != curridx {
                let mut sccvarspos = t.sccstarts[*t.nsccs as usize];

                // Store the SCC in sccvars.
                loop {
                    stacksize -= 1;
                    idx = t.dfsstack[stacksize as usize];
                    t.nodeonstack[idx as usize] = false;
                    t.sccvars[sccvarspos as usize] = idx;
                    sccvarspos += 1;
                    if idx == curridx {
                        break;
                    }
                }
                *t.nsccs += 1;
                t.sccstarts[*t.nsccs as usize] = sccvarspos;
            }
            // Trivial SCC: remove the single node from the stack, but don't
            // store it as a SCC.
            else {
                stacksize -= 1;
                idx = t.dfsstack[stacksize as usize];
                t.nodeonstack[idx as usize] = false;
            }
        }
        // In a pure dfs, the node would now leave the stack, add it to the
        // array of nodes in reverse topological order.
        if let Some((order, nordered)) = topoorder.as_mut()
            && (stacksize > 0 || label > *startindex + 1)
        {
            order[**nordered as usize] = curridx;
            **nordered += 1;
        }

        // The current node was handled, backtrack.
        if stacksize > 0 {
            idx = t.dfsstack[t.predstackidx[cs] as usize];
            t.nodelowlink[idx as usize] = crate::def::min(t.nodelowlink[idx as usize], t.nodelowlink[cu]);
            currstackidx = t.predstackidx[cs];
        }
    }

    *startindex = label;
}

/// `applyFixingsAndAggregations`: apply fixings and aggregations found by
/// the clique graph analysis.
#[allow(clippy::too_many_arguments)]
fn apply_fixings_and_aggregations(
    scip: &mut Scip,
    vars: &[VarId],
    infeasnodes: &[i32],
    ninfeasnodes: i32,
    nodeinfeasible: &mut [bool],
    sccvars: &[i32],
    sccstarts: &[i32],
    nsccs: i32,
    infeasible: &mut bool,
    nfixedvars: &mut i32,
    naggrvars: &mut i32,
    result: &mut ScipResultCode,
) -> ScipResult {
    let mut i = 0usize;

    // For all infeasible node: fix variable to the other bound.
    if !*infeasible && ninfeasnodes > 0 {
        while i < ninfeasnodes as usize {
            let var = vars[get_var_index(infeasnodes[i])];
            let lower = is_index_lowerbound(infeasnodes[i]);

            nodeinfeasible[infeasnodes[i] as usize] = false;

            let (inf, fixed) = scip_var::fix_var(scip, var, if lower { 0.0 } else { 1.0 })?;
            *infeasible = inf;

            // Fixing was infeasible.
            if *infeasible {
                break;
            }

            // Increase fixing counter and update result pointer.
            if fixed {
                *result = ScipResultCode::Success;
                *nfixedvars += 1;
            }
            i += 1;
        }
    }

    // Clear clean buffer array (if we did not enter the block above or
    // stopped early due to an infeasibility).
    while i < ninfeasnodes as usize {
        nodeinfeasible[infeasnodes[i] as usize] = false;
        i += 1;
    }

    if !*infeasible && nsccs > 0 {
        // For each strongly connected component: aggregate all variables to
        // the first one.
        for i in 0..nsccs as usize {
            // Get variable and boundtype for first node of the SCC.
            let startvar = vars[get_var_index(sccvars[sccstarts[i] as usize])];
            let lower = is_index_lowerbound(sccvars[sccstarts[i] as usize]);

            for v in (sccstarts[i] + 1) as usize..sccstarts[i + 1] as usize {
                // Aggregate variables: if both nodes represent the same bound,
                // we have x=1 <=> y=1, and thus aggregate x - y = 0; if both
                // represent different bounds we have x=1 <=> y=0, so we
                // aggregate x + y = 1.
                let same = lower == is_index_lowerbound(sccvars[v]);
                let (inf, _redundant, aggregated) = scip_var::aggregate_vars(
                    scip,
                    startvar,
                    vars[get_var_index(sccvars[v])],
                    1.0,
                    if same { -1.0 } else { 1.0 },
                    if same { 0.0 } else { 1.0 },
                )?;
                *infeasible = inf;

                // Aggregation was infeasible.
                if *infeasible {
                    break;
                }

                // Increase aggregation counter and update result pointer.
                if aggregated {
                    *naggrvars += 1;
                    *result = ScipResultCode::Success;
                }
            }
        }
    }

    Ok(())
}

/// `propPresolVbounds`: presolving method of propagator: search for strongly
/// connected components in the implication graph and aggregate all variables
/// within a component; additionally, identifies infeasible variable
/// assignments. A second run processes the bounds in (almost) topological
/// order.
fn prop_presol_vbounds(
    scip: &mut Scip,
    prop: PropId,
    _nrounds: i32,
    presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    let mut startindex = 1;
    let mut nordered = 0;
    let mut infeasible: bool;

    let mut ncliques = scip_var::get_n_cliques(scip);

    *result = ScipResultCode::DidNotRun;

    if ncliques < 2 {
        return Ok(());
    }

    let (maxcliquesmedium, maxcliquesexhaustive, minnewcliques, lastpresolncliques) = {
        let p = pd(scip, prop);
        (p.maxcliquesmedium, p.maxcliquesexhaustive, p.minnewcliques, p.lastpresolncliques)
    };

    // Too many cliques for medium presolving.
    if presoltiming == PRESOLTIMING_MEDIUM && ncliques as f64 > maxcliquesmedium * api::get_n_bin_vars(scip) as f64 {
        return Ok(());
    }

    // Too many cliques for exhaustive presolving.
    if ncliques as f64 > maxcliquesexhaustive * api::get_n_bin_vars(scip) as f64 {
        return Ok(());
    }

    // Only run if enough new cliques were created since the last successful
    // call.
    let ncreated = scip.cliquetable.as_ref().unwrap().ncreatedcliques;
    if (ncreated as f64) < (1.0 + minnewcliques) * lastpresolncliques as f64 {
        return Ok(());
    }

    *result = ScipResultCode::DidNotFind;

    let nbinvars = (api::get_n_vars(scip) - api::get_n_cont_vars(scip)) as usize;
    let nbounds = 2 * nbinvars;

    // Cleanup cliques, stop if this proved infeasibility already.
    infeasible = scip_var::cleanup_cliques(scip)?;

    if infeasible {
        *result = ScipResultCode::Cutoff;
        return Ok(());
    }

    // Duplicate variable array; needed to get the fixings right later.
    let vars: Vec<VarId> = api::get_vars(scip)[..nbinvars].to_vec();

    let mut dfsstack = vec![0i32; nbounds];
    let mut stacknextclique = vec![0i32; nbounds];
    let mut stacknextcliquevar = vec![0i32; nbounds];
    let mut predstackidx = vec![0i32; nbounds];
    let mut topoorder = vec![0i32; nbounds];
    let mut sccvars = vec![0i32; nbounds];
    let mut sccstarts = vec![0i32; nbinvars + 1];
    let mut infeasnodes = vec![0i32; nbounds];
    let mut nodeindex = vec![0i32; nbounds];
    let mut nodelowlink = vec![0i32; nbounds];
    let mut cliquefirstentry = vec![0i32; ncliques as usize];
    let mut cliquecurrentexit = vec![0i32; ncliques as usize];
    let mut nodeonstack = vec![false; nbounds];
    let mut nodeinfeasible = vec![false; nbounds];
    sccstarts[0] = 0;
    let mut nsccs = 0;
    let mut ninfeasnodes = 0;

    {
        let mut t = Tarjan {
            nodeonstack: &mut nodeonstack,
            nodeindex: &mut nodeindex,
            nodelowlink: &mut nodelowlink,
            nodeinfeasible: &mut nodeinfeasible,
            dfsstack: &mut dfsstack,
            predstackidx: &mut predstackidx,
            stacknextclique: &mut stacknextclique,
            stacknextcliquevar: &mut stacknextcliquevar,
            cliquefirstentry: &mut cliquefirstentry,
            cliquecurrentexit: &mut cliquecurrentexit,
            sccvars: &mut sccvars,
            sccstarts: &mut sccstarts,
            nsccs: &mut nsccs,
            infeasnodes: &mut infeasnodes,
            ninfeasnodes: &mut ninfeasnodes,
        };

        // While there are unvisited nodes, run Tarjan's algorithm starting
        // from one of these nodes.
        let mut i = 0;
        while i < nbounds && !infeasible {
            if t.nodeindex[i] == 0 {
                tarjan(scip, i as i32, &mut startindex, &mut t, Some((&mut topoorder, &mut nordered)), &mut infeasible);
            }
            i += 1;
        }
    }

    // Aggregate all variables within a SCC and fix all variables for which
    // one bounds was proven infeasible.
    if ninfeasnodes > 0 || nsccs > 0 {
        apply_fixings_and_aggregations(
            scip,
            &vars,
            &infeasnodes,
            ninfeasnodes,
            &mut nodeinfeasible,
            &sccvars,
            &sccstarts,
            nsccs,
            &mut infeasible,
            &mut counts.nfixedvars,
            &mut counts.naggrvars,
            result,
        )?;
    }

    // Second round, now with topological order!
    'second: {
        if !infeasible && nordered > 0 {
            // We already fixed or aggregated some variables in the first run,
            // so we better clean up the cliques.
            if *result == ScipResultCode::Success {
                infeasible = scip_var::cleanup_cliques(scip)?;

                if infeasible {
                    break 'second;
                }
            }

            let nbounds2 = 2 * (api::get_n_vars(scip) - api::get_n_cont_vars(scip)) as usize;
            ncliques = scip_var::get_n_cliques(scip);

            let vars2: Vec<VarId> = api::get_vars(scip)[..nbounds2 / 2].to_vec();

            // Clear arrays that should be initialized to 0.
            nodeonstack[..nbounds2].fill(false);
            nodeindex[..nbounds2].fill(0);
            nodelowlink[..nbounds2].fill(0);
            if cliquefirstentry.len() < ncliques as usize {
                cliquefirstentry.resize(ncliques as usize, 0);
                cliquecurrentexit.resize(ncliques as usize, 0);
            }
            cliquefirstentry[..ncliques as usize].fill(0);
            cliquecurrentexit[..ncliques as usize].fill(0);
            sccstarts[0] = 0;
            nsccs = 0;
            ninfeasnodes = 0;
            startindex = 1;

            {
                let mut t = Tarjan {
                    nodeonstack: &mut nodeonstack,
                    nodeindex: &mut nodeindex,
                    nodelowlink: &mut nodelowlink,
                    nodeinfeasible: &mut nodeinfeasible,
                    dfsstack: &mut dfsstack,
                    predstackidx: &mut predstackidx,
                    stacknextclique: &mut stacknextclique,
                    stacknextcliquevar: &mut stacknextcliquevar,
                    cliquefirstentry: &mut cliquefirstentry,
                    cliquecurrentexit: &mut cliquecurrentexit,
                    sccvars: &mut sccvars,
                    sccstarts: &mut sccstarts,
                    nsccs: &mut nsccs,
                    infeasnodes: &mut infeasnodes,
                    ninfeasnodes: &mut ninfeasnodes,
                };

                // While there are unvisited nodes, run Tarjan's algorithm
                // starting from one of these nodes.
                let mut i = nordered - 1;
                while i >= 0 && !infeasible {
                    // Variable of next node in topological order.
                    let varindex = scip.vars[vars[get_var_index(topoorder[i as usize])]].probindex;

                    // Variable was not fixed after the first run.
                    if varindex >= 0 {
                        let startpos = if is_index_lowerbound(topoorder[i as usize]) {
                            get_lb_index(varindex)
                        } else {
                            get_ub_index(varindex)
                        };
                        if t.nodeindex[startpos as usize] == 0 {
                            tarjan(scip, startpos, &mut startindex, &mut t, None, &mut infeasible);
                        }
                    }
                    i -= 1;
                }
            }

            // Aggregate all variables within a SCC and fix all variables for
            // which one bounds was proven infeasible.
            if ninfeasnodes > 0 || nsccs > 0 {
                apply_fixings_and_aggregations(
                    scip,
                    &vars2,
                    &infeasnodes,
                    ninfeasnodes,
                    &mut nodeinfeasible,
                    &sccvars,
                    &sccstarts,
                    nsccs,
                    &mut infeasible,
                    &mut counts.nfixedvars,
                    &mut counts.naggrvars,
                    result,
                )?;
            }
        }
    }

    // TERMINATE:
    if infeasible {
        *result = ScipResultCode::Cutoff;
    }

    pdm(scip, prop).lastpresolncliques = scip.cliquetable.as_ref().unwrap().ncreatedcliques;

    Ok(())
}

/// `propExecVbounds`: execution method of propagator.
fn prop_exec_vbounds(scip: &mut Scip, prop: PropId, _proptiming: PropTiming, result: &mut ScipResultCode) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Perform variable lower and upper bound propagation.
    propagate_vbounds(scip, prop, false, result)
}

/// `propRespropVbounds`: propagation conflict resolving method of
/// propagator.
#[allow(clippy::too_many_arguments)]
fn prop_resprop_vbounds(
    scip: &mut Scip,
    prop: PropId,
    infervar: VarId,
    inferinfo: i32,
    boundtype: BoundType,
    bdchgidx: Option<BdChgIdx>,
    mut relaxedbd: f64,
    result: &mut ScipResultCode,
) -> ScipResult {
    let starttype = infer_info_get_boundtype(inferinfo);
    let pos = infer_info_get_pos(inferinfo);

    let (startvar, usebdwidening) = {
        let p = pd(scip, prop);
        (p.vars[get_var_index(pos)], p.usebdwidening)
    };

    if !var_is_binary(scip, startvar) && usebdwidening {
        let (coef, constant) = {
            let p = pd(scip, prop);
            let vboundboundedidx = &p.vboundboundedidx[pos as usize];
            let nvbounds = vboundboundedidx.len();

            let inferidx = if boundtype == BoundType::Lower {
                var_get_lb_index(p, infervar)
            } else {
                var_get_ub_index(p, infervar)
            };

            let mut b = 0;
            while b < nvbounds {
                if vboundboundedidx[b] == inferidx {
                    break;
                }
                b += 1;
            }
            debug_assert!(b < nvbounds);

            (p.vboundcoefs[pos as usize][b], p.vboundconstants[pos as usize][b])
        };

        // Compute the relaxed bound which is sufficient to propagate the
        // inference bound of given variable.
        relaxedbd = if boundtype == BoundType::Lower {
            compute_relaxed_lowerbound(scip, infervar, relaxedbd, coef, constant)
        } else {
            compute_relaxed_upperbound(scip, infervar, relaxedbd, coef, constant)
        };

        // Try to relax variable bound variable.
        relax_vbdvar(scip, startvar, starttype, bdchgidx.as_ref(), relaxedbd)?;
    } else {
        resolve_propagation(scip, startvar, starttype, bdchgidx.as_ref())?;
    }

    *result = ScipResultCode::Success;

    Ok(())
}

/*
 * Callback methods of event handler
 */

/// `eventExecVbound`: execution method of bound change event handler; the
/// event handler data is the propagator, the event data the topoorder
/// position.
fn event_exec_vbound(scip: &mut Scip, eventhdlr: EventhdlrId, event: &Event, eventdata: usize) -> ScipResult {
    let prop: PropId = *scip.eventhdlrs[eventhdlr].eventhdlrdata.as_ref().unwrap().downcast_ref::<PropId>().unwrap();
    let idx = eventdata;

    if event.eventtype == EVENTTYPE_GUBCHANGED && var_is_binary(scip, event.data.var) && event.data.newbound > 0.5 {
        return Ok(());
    }

    if event.eventtype == EVENTTYPE_GLBCHANGED && var_is_binary(scip, event.data.var) && event.data.newbound < 0.5 {
        return Ok(());
    }

    // Add the bound change to the propagation queue, if it is not already
    // contained.
    let p = pdm(scip, prop);
    if !p.inqueue[idx] {
        p.propqueue.insert(idx + 1, comp_varbound_indices, |_, _, _| {});
        p.inqueue[idx] = true;
    }

    Ok(())
}

/*
 * propagator specific interface methods
 */

/// `SCIPincludePropVbounds`: creates the vbounds propagator and includes it
/// in SCIP.
pub fn include_prop_vbounds(scip: &mut Scip) -> ScipResult {
    use crate::paramset::{paramset_add_bool, paramset_add_real};

    // Create vbounds propagator data and reset propagation data.
    let mut propdata = PropData::default();
    reset_propdata(&mut propdata);

    // Include propagator.
    let prop = api::include_prop_basic(
        scip,
        PROP_NAME,
        PROP_DESC,
        PROP_PRIORITY,
        PROP_FREQ,
        PROP_DELAY,
        PROP_TIMING,
        prop_exec_vbounds,
        Some(Box::new(propdata) as Box<dyn Any>),
    )?;

    // Set optional callbacks via setter functions.
    scip.props[prop].propcopy = Some(prop_copy_vbounds);
    scip.props[prop].propinitpre = Some(prop_initpre_vbounds);
    scip.props[prop].propexitsol = Some(prop_exitsol_vbounds);
    scip.props[prop].propresprop = Some(prop_resprop_vbounds);
    crate::prop::prop_set_presol(scip, prop, prop_presol_vbounds, PROP_PRESOL_PRIORITY, PROP_PRESOL_MAXROUNDS, PROP_PRESOLTIMING)?;

    // Include event handler for bound change events.
    let eventhdlr = api::include_eventhdlr_basic(scip, EVENTHDLR_NAME, EVENTHDLR_DESC, event_exec_vbound, Some(Box::new(prop) as Box<dyn Any>))?;
    pdm(scip, prop).eventhdlr = eventhdlr;

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, props, prop, propdata, PropData, $f)
        };
    }

    paramset_add_bool(
        scip,
        "propagating/vbounds/usebdwidening",
        "should bound widening be used to initialize conflict analysis?",
        t!(bool, usebdwidening),
        false,
        DEFAULT_USEBDWIDENING,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/vbounds/useimplics",
        "should implications be propagated?",
        t!(bool, useimplics),
        true,
        DEFAULT_USEIMPLICS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/vbounds/usecliques",
        "should cliques be propagated?",
        t!(bool, usecliques),
        true,
        DEFAULT_USECLIQUES,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/vbounds/usevbounds",
        "should vbounds be propagated?",
        t!(bool, usevbounds),
        true,
        DEFAULT_USEVBOUNDS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/vbounds/dotoposort",
        "should the bounds be topologically sorted in advance?",
        t!(bool, dotoposort),
        false,
        DEFAULT_DOTOPOSORT,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/vbounds/sortcliques",
        "should cliques be regarded for the topological sort?",
        t!(bool, sortcliques),
        true,
        DEFAULT_SORTCLIQUES,
        None,
    )?;
    paramset_add_bool(
        scip,
        "propagating/vbounds/detectcycles",
        "should cycles in the variable bound graph be identified?",
        t!(bool, detectcycles),
        false,
        DEFAULT_DETECTCYCLES,
        None,
    )?;
    paramset_add_real(
        scip,
        "propagating/vbounds/minnewcliques",
        "minimum percentage of new cliques to trigger another clique table analysis",
        t!(f64, minnewcliques),
        false,
        DEFAULT_MINNEWCLIQUES,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "propagating/vbounds/maxcliquesmedium",
        "maximum number of cliques per variable to run clique table analysis in medium presolving",
        t!(f64, maxcliquesmedium),
        false,
        DEFAULT_MAXCLIQUESMEDIUM,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;
    paramset_add_real(
        scip,
        "propagating/vbounds/maxcliquesexhaustive",
        "maximum number of cliques per variable to run clique table analysis in exhaustive presolving",
        t!(f64, maxcliquesexhaustive),
        false,
        DEFAULT_MAXCLIQUESEXHAUSTIVE,
        0.0,
        crate::def::REAL_MAX,
        None,
    )?;

    Ok(())
}

/// `SCIPisPropagatedVbounds`: returns TRUE if the propagator has the status
/// that all variable lower and upper bounds are propagated.
pub fn is_propagated_vbounds(scip: &Scip) -> bool {
    let prop = crate::set::set_find_prop(scip, PROP_NAME);
    pd(scip, prop).propqueue.n_elems() == 0
}

/// `SCIPexecPropVbounds`: performs propagation of variables lower and upper
/// bounds.
pub fn exec_prop_vbounds(scip: &mut Scip, force: bool, result: &mut ScipResultCode) -> ScipResult {
    let prop = crate::set::set_find_prop(scip, PROP_NAME);

    // Perform variable lower and upper bound propagation.
    propagate_vbounds(scip, prop, force, result)
}
