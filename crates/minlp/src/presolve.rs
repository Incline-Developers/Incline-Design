//! SCIP's presolving helpers (`presolve.c`): shrinking a disjunctive set of
//! bound conditions by implications, cliques and variable bounds.

use crate::{
    def::INVALID,
    retcode::ScipResult,
    scip::Scip,
    types::{BoundType, VarId},
};

/// The arrays the collectors of `SCIPshrinkDisjunctiveVarSet` share.
struct Shrink<'a> {
    /// Array of bounds where one of them must be fullfilled.
    bounds: &'a [f64],
    /// Array of bound types (TRUE == upper).
    boundtypes: &'a [bool],
    /// Array of implied bounds (size is two times number of variables, first
    /// half for implied lower bounds, second for implied upper bounds).
    newbounds: Vec<f64>,
    /// Array of number of implication on a bound (size is two times number of
    /// variables, first half for implied lower bounds, second for implied
    /// upper bounds).
    counts: Vec<i32>,
    /// Array to store the indices of non-zero entries in the counts array.
    countnonzeros: Vec<i32>,
    ncountnonzeros: i32,
    /// Array containing for set variables the position in the current set,
    /// or 0 if it is not a set variable or -1, if it is a redundant (i.e.
    /// implies another set variable) set variables (size is two times number
    /// of variables, first half for implied lower bounds, second for implied
    /// upper bounds).
    issetvar: Vec<i32>,
    /// Number of problem variables.
    nvars: i32,
    /// The lowest index of a binary implication variable when found.
    foundbin: i32,
    /// The lowest index of a non-binary implication variable when found.
    foundnonbin: i32,
    /// Array to store the variable indices (for upper bound 'nvars' is added
    /// to the index) which are implied.
    implidx: Vec<i32>,
    nimplidx: i32,
    /// Array to remember last implied bounds before taken the current
    /// variable into account, first nvars for lower bound, second nvars for
    /// upper bound; this array is used when a set variable got redundant,
    /// because it implies another set variable, and we need to correct the
    /// counts array.
    lastbounds: Vec<f64>,
}

impl Shrink<'_> {
    /// Records an index in the counts' non-zeros on its first count.
    #[inline]
    fn note_nonzero(&mut self, idx: usize) {
        if self.counts[idx] == 1 {
            self.countnonzeros[self.ncountnonzeros as usize] = idx as i32;
            self.ncountnonzeros += 1;
        }
    }

    /// Records an implied index.
    #[inline]
    fn push_impl(&mut self, idx: usize) {
        self.implidx[self.nimplidx as usize] = idx as i32;
        self.nimplidx += 1;
    }

    /// Counts a binary implication on `idx`.
    #[inline]
    fn count_bin(&mut self, idx: usize) {
        self.counts[idx] += 1;
        self.foundbin = self.foundbin.min(idx as i32);
        self.note_nonzero(idx);
        self.push_impl(idx);
    }

    /// Counts a non-binary implication on `idx` with implied bound `bound`,
    /// keeping the weaker bound (the larger with `keep_larger`).
    #[inline]
    fn count_nonbin(&mut self, idx: usize, bound: f64, keep_larger: bool) {
        self.counts[idx] += 1;
        self.update_nonbin_bound(idx, bound, keep_larger);
        self.foundnonbin = self.foundnonbin.min(idx as i32);
        self.push_impl(idx);
    }

    /// The bound bookkeeping of a non-binary implication whose count was just
    /// increased.
    #[inline]
    fn update_nonbin_bound(&mut self, idx: usize, bound: f64, keep_larger: bool) {
        let n = self.nimplidx as usize;
        if self.counts[idx] == 1 {
            self.countnonzeros[self.ncountnonzeros as usize] = idx as i32;
            self.ncountnonzeros += 1;
            self.newbounds[idx] = bound;
            self.lastbounds[n] = INVALID;
        } else if if keep_larger { self.newbounds[idx] < bound } else { self.newbounds[idx] > bound } {
            self.lastbounds[n] = self.newbounds[idx];
            self.newbounds[idx] = bound;
        } else {
            self.lastbounds[n] = INVALID;
        }
    }
}

/// `collectNonBinaryVBoundData`: collect variable bound information for a
/// variable set reduction and global implication; only variable which have
/// the vartype != SCIP_VARTYPE_BINARY have variable bounds. `varidx` is,
/// for lower bound set variable index, for upper bound set variable index +
/// number of variables; `pos` the variables's position in bdchinfos.
fn collect_non_binary_vbound_data(scip: &Scip, d: &mut Shrink, var: VarId, varidx: usize, pos: i32, nredvars: i32) {
    let p = pos as usize;
    let bound = d.bounds[p];
    let v = &scip.vars[var];

    // 1. case: lower bound in set, 2. case: upper bound in set.
    let upperset = d.boundtypes[p];

    // Update implication counter of set variable.
    if d.counts[varidx] == pos - nredvars {
        d.counts[varidx] += 1;
        d.update_nonbin_bound(varidx, bound, upperset);
        d.foundnonbin = d.foundnonbin.min(varidx as i32);
        d.push_impl(varidx);
    }

    let vbds = if upperset { &v.vlbs } else { &v.vubs };
    let nimpls = crate::implics::vbounds_get_n_vbds(vbds);
    let Some(vb) = vbds.as_ref() else {
        return;
    };

    let mut w = nimpls - 1;
    while w >= 0 {
        let wu = w as usize;
        w -= 1;
        let implvar = vb.vars[wu];
        let implcoef = vb.coefs[wu];
        let implconst = vb.constants[wu];
        let iv = &scip.vars[implvar];
        let mut idx = iv.probindex;

        // Do not use inactive variables.
        // @todo if implvars[x] is aggregated, we could transform the variable
        // into the active representation.
        if idx < 0 || scip.set.is_feas_eq(iv.glbdom.lb, iv.glbdom.ub) {
            continue;
        }

        let isbin = crate::var::var_is_binary(scip, implvar);

        if !upperset {
            // The variable upper bound with coefficient c and constant d bounds
            // var from above: var <= c * implvar + d, and var >= bounds[pos].
            if implcoef < 0.0 {
                // The upper bound of implvars[w] is bounding upper bound of var.
                idx += d.nvars;
                let i = idx as usize;
                if d.counts[i] == pos - nredvars {
                    if isbin {
                        // Do not look at fixed variables.
                        if iv.glbdom.lb > 0.5 || iv.glbdom.ub < 0.5 {
                            continue;
                        }

                        // (implvars[w] = 1 ===> var <= implcoefs[w] + implconsts[w]
                        // and if implcoefs[w] + implconsts[w] < bounds[pos]) ===>
                        // (because var => bounds[v] ===> implvars[w] = 0).
                        if scip.set.is_feas_lt(implcoef + implconst, bound) {
                            // Set variable 'var' with bound implies other set
                            // variable 'implvars[w]' with corresponding bound so we
                            // can remove the set variable 'var'.
                            if d.issetvar[i] > 0 {
                                d.issetvar[varidx] = -1;
                                break;
                            }

                            d.count_bin(i);
                        }
                    }
                    // If (implvars[w] = ub(implvars[w]) ==> var <=
                    // implcoefs[w]*SCIPvarGetUbGlobal(implvars[w]) + implconsts[w])
                    // but (var >= bounds[pos] with bounds[pos] >
                    // implcoefs[w]*SCIPvarGetUbGlobal(implvars[w]) + implconsts[w])
                    // it follows (new_ub(var) < ub(var)).
                    else if scip.set.is_feas_lt(implcoef * iv.glbdom.ub + implconst, bound) {
                        let newub = (bound - implconst) / implcoef;

                        // Set variable 'var' with bound implies other set variable
                        // 'implvars[w]' with corresponding bound so we can remove
                        // the set variable 'var'.
                        if d.issetvar[i] > 0 && newub <= d.bounds[(d.issetvar[i] - 1) as usize] {
                            d.issetvar[varidx] = -1;
                            break;
                        }

                        d.count_nonbin(i, newub, true);
                    }
                }
            } else {
                let i = idx as usize;
                // Update the counters and implied bounds.
                if d.counts[i] == pos - nredvars {
                    if isbin {
                        // Do not look at fixed variables.
                        if iv.glbdom.lb > 0.5 || iv.glbdom.ub < 0.5 {
                            continue;
                        }

                        // (implvars[w] = 0 ===> var <= implconsts[w] and if
                        // implconsts[w] < bounds[pos]) ===> (because var >=
                        // bounds[pos] ===> implvars[w] = 1).
                        if scip.set.is_feas_lt(implconst, bound) {
                            // Set variable 'var' with bound implies other set
                            // variable 'implvars[w]' with corresponding bound so we
                            // can remove the set variable 'var'.
                            if d.issetvar[i] > 0 {
                                d.issetvar[varidx] = -1;
                                break;
                            }

                            d.count_bin(i);
                        }
                    }
                    // If (implvars[w] = lb(implvars[w]) => var <=
                    // implcoefs[w]*SCIPvarGetLbGlobal(implvars[w]) + implconsts[w])
                    // but (var >= bounds[pos] with bounds[pos] >
                    // implcoefs[w]*SCIPvarGetLbGlobal(implvars[w]) + implconsts[w])
                    // it follows (new_lb(var) > lb(var)).
                    else if scip.set.is_feas_lt(implcoef * iv.glbdom.lb + implconst, bound) {
                        let newlb = (bound - implconst) / implcoef;

                        // Set variable 'var' with bound implies other set variable
                        // 'implvars[w]' with corresponding bound so we can remove
                        // the set variable 'var'.
                        if d.issetvar[i] > 0 && newlb >= d.bounds[(d.issetvar[i] - 1) as usize] {
                            d.issetvar[varidx] = -1;
                            break;
                        }

                        d.count_nonbin(i, newlb, false);
                    }
                }
            }
        } else {
            // The variable lower bound bounds var from below, and var <=
            // bounds[pos].
            if implcoef < 0.0 {
                // The lower bound of implvars[w] is bounding lower bound of var.
                let i = idx as usize;
                if d.counts[i] == pos - nredvars {
                    if isbin {
                        // Do not look at fixed variables.
                        if iv.glbdom.lb > 0.5 || iv.glbdom.ub < 0.5 {
                            continue;
                        }

                        // (implvars[w] = 0 ===> var >= implconsts[w] and if
                        // implconsts[w] > bounds[pos]) ===> (because var <=
                        // bounds[pos] ===> implvars[w] = 1).
                        if scip.set.is_feas_gt(implconst, bound) {
                            if d.issetvar[i] > 0 {
                                d.issetvar[varidx] = -1;
                                break;
                            }

                            d.count_bin(i);
                        }
                    }
                    // If (implvars[w] = lb(implvars[w]) ==> var <=
                    // implcoefs[w]*SCIPvarGetLbGlobal(implvars[w]) + implconsts[w])
                    // but (var <= bounds[pos] with bounds[pos] <
                    // implcoefs[w]*SCIPvarGetLbGlobal(implvars[w]) + implconsts[w])
                    // it follows (new_lb(var) > ub(var)).
                    else if scip.set.is_feas_gt(implcoef * iv.glbdom.lb + implconst, bound) {
                        let newlb = (bound - implconst) / implcoef;

                        if d.issetvar[i] > 0 && newlb >= d.bounds[(d.issetvar[i] - 1) as usize] {
                            d.issetvar[varidx] = -1;
                            break;
                        }

                        d.count_nonbin(i, newlb, false);
                    }
                }
            } else {
                // Update the counters and implied bounds.
                idx += d.nvars;
                let i = idx as usize;
                if d.counts[i] == pos - nredvars {
                    if isbin {
                        // Do not look at fixed variables.
                        if iv.glbdom.lb > 0.5 || iv.glbdom.ub < 0.5 {
                            continue;
                        }

                        // (implvars[w] = 1 ===> var >= implcoefs[w] + implconsts[w]
                        // and if implcoefs[w] + implconsts[w] > bounds[pos]) ===>
                        // (because var <= bounds[pos] ===> implvars[w] = 0).
                        if scip.set.is_feas_gt(implcoef + implconst, bound) {
                            if d.issetvar[i] > 0 {
                                d.issetvar[varidx] = -1;
                                break;
                            }

                            d.count_bin(i);
                        }
                    }
                    // If (implvars[w] = ub(implvars[w]) => var <=
                    // implcoefs[w]*SCIPvarGetUbGlobal(implvars[w]) + implconsts[w])
                    // but (var <= bounds[pos] with bounds[pos] <
                    // implcoefs[w]*SCIPvarGetUbGlobal(implvars[w]) + implconsts[w])
                    // it follows (new_ub(var) < ub(var)).
                    else if scip.set.is_feas_gt(implcoef * iv.glbdom.ub + implconst, bound) {
                        let newub = (bound - implconst) / implcoef;

                        if d.issetvar[i] > 0 && newub <= d.bounds[(d.issetvar[i] - 1) as usize] {
                            d.issetvar[varidx] = -1;
                            break;
                        }

                        d.count_nonbin(i, newub, true);
                    }
                }
            }
        }
    }
}

/// `collectNonBinaryImplicationData`: collect non-binary implication data
/// for variable set reduction and global bound implications; only variable
/// which have the vartype SCIP_VARTYPE_BINARY have implications, otherwise
/// the implications are saved as variable bounds; `value` is the value used
/// for clique and implication info.
fn collect_non_binary_implication_data(scip: &Scip, d: &mut Shrink, var: VarId, varidx: usize, pos: i32, nredvars: i32, value: bool) {
    if d.issetvar[varidx] <= 0 {
        return;
    }

    // Get implication information.
    let Some(implics) = scip.vars[var].implics.as_deref() else {
        return;
    };
    let val = value as usize;

    let mut w = implics.nimpls[val] - 1;
    while w >= 0 {
        let wu = w as usize;
        w -= 1;
        let implvar = implics.vars[val][wu];
        let implbound = implics.bounds[val][wu];
        let iv = &scip.vars[implvar];

        // No self implication should exist in the implication data structure.
        let mut idx = iv.probindex;

        // Do not use inactive variables.
        // @todo if implvars[x] is aggregated, we could transform the variable
        // into the active representation.
        if idx < 0 || scip.set.is_feas_eq(iv.glbdom.lb, iv.glbdom.ub) {
            continue;
        }

        let upper = implics.types[val][wu] == BoundType::Upper;
        if upper {
            idx += d.nvars;
        }
        let i = idx as usize;

        // Set variable 'var' with bound implies other set variable
        // 'implvars[w]' with a non-worse bound than the bound so we can remove
        // the set variable 'var'.
        if d.issetvar[i] > 0
            && if upper {
                d.bounds[(d.issetvar[i] - 1) as usize] >= implbound
            } else {
                d.bounds[(d.issetvar[i] - 1) as usize] <= implbound
            }
        {
            d.issetvar[varidx] = -1;
            break;
        }

        // Ignore implications that are redundant with respect to the current
        // global bounds (see #2888).
        let redundant = if upper {
            scip.set.is_le(iv.glbdom.ub, implbound)
        } else {
            scip.set.is_ge(iv.glbdom.lb, implbound)
        };

        // Update implication counter and implied bound.
        if d.counts[i] == pos - nredvars && !redundant {
            d.counts[i] += 1;
            if crate::var::var_is_binary(scip, implvar) {
                // The implied bound on a binary variable should not be trivial,
                // otherwise we might globally fix this variable to a wrong value.
                //
                // Note: it is possible that the implied bound is outside [0,1],
                // when the implied variable has become binary during the search.
                d.foundbin = d.foundbin.min(idx);
                d.note_nonzero(i);
            } else {
                d.foundnonbin = d.foundnonbin.min(idx);
                d.update_nonbin_bound(i, implbound, upper);
            }

            d.push_impl(i);
        }
    }
}

/// `collectBinaryCliqueData`: collect clique data on binary variables for
/// variable set reduction and global bound implications.
fn collect_binary_clique_data(scip: &Scip, d: &mut Shrink, var: VarId, varidx: usize, pos: i32, nredvars: i32, value: bool) {
    // Implication counter cannot exceed number implication variables.

    // If the set variable is not yet redundant we might increase the self
    // implication counter.
    if d.issetvar[varidx] > 0 {
        // Update implication counter for set variables.
        if d.counts[varidx] == pos - nredvars {
            d.count_bin(varidx);
        }
    }

    let Some(cl) = scip.vars[var].cliquelist.as_deref() else {
        return;
    };
    let val = value as usize;

    // Update implication counter on all by cliques implied variables.
    let mut c = cl.ncliques[val] - 1;
    while c >= 0 {
        let clique = &scip.cliques[cl.cliques[val][c as usize]];
        c -= 1;

        let mut w = clique.nvars - 1;
        while w >= 0 {
            let wu = w as usize;
            w -= 1;
            let clqvar = clique.vars[wu];
            let cv = &scip.vars[clqvar];

            // Already handle self-implication and do not look at fixed
            // variables.
            if clqvar == var || cv.glbdom.lb > 0.5 || cv.glbdom.ub < 0.5 {
                continue;
            }

            let mut idx = cv.probindex;
            if clique.values[wu] {
                idx += d.nvars;
            }
            let i = idx as usize;

            // Set variable 'var' with bound implies other set variable
            // 'clqvars[w]' with corresponding set bound so we can remove the set
            // variable 'var'.
            if d.issetvar[i] > 0 {
                d.issetvar[varidx] = -1;
                break;
            }

            // Update implication counter.
            if d.counts[i] == pos - nredvars {
                d.count_bin(i);
            }
        }
    }
}

/// `CLEARRATIO`.
const CLEARRATIO: f64 = 0.8;

/// `SCIPshrinkDisjunctiveVarSet`: try to reduce the necessary variable in a
/// set of variables with corresponding bounds and boundtypes for which one
/// must be fulfilled, e.g. a set of logicor or bounddisjunctive constraint
/// variables would be such a set.
///
/// Consider the following set:
///
///   x1 >= 1, x2 >= 3, x3 >= 1, x4 <= 0
///
/// by (global) implication data (cliques, implications, and variable bounds)
/// we have also the following implications given:
///
///   x1 >= 1 => x3 >= 1
///   x2 >= 2 => x3 >= 1
///   x4 <= 0 => x1 >= 1
///
/// Because of the last implication x4 is redundant, because x1 >= 1 would
/// also be fulfilled in the variable set, so we can reduce the set by x4.
/// Also, the both other implications and x3 >= 1 (in the given variable set)
/// all imply exactly x3 >= 1, so we tighten the global lower bound of x3 to
/// 1 and the set of variables gets redundant.
///
/// `boundtypes` are TRUE == SCIP_BOUNDTYPE_UPPER, FALSE ==
/// SCIP_BOUNDTYPE_LOWER; `redundants` is filled to indicate if a variable in
/// the set is redundant; `nredvars` counts how many variables can be
/// removed, `nglobalred` the global reductions on variable bounds found
/// through this set; `setredundant` is set if a global reduction on a
/// variable which was part of the given set makes this disjunction
/// redundant; `glbinfeas` if global infeasibility was detected;
/// `fullshortening`: do we want to try the shortening procedure over the
/// whole set (which might be expensive).
#[allow(clippy::too_many_arguments)]
pub fn shrink_disjunctive_var_set(
    scip: &mut Scip,
    vars: &[VarId],
    bounds: &[f64],
    boundtypes: &[bool],
    redundants: &mut [bool],
    nvars: i32,
    nredvars: &mut i32,
    nglobalred: &mut i32,
    setredundant: &mut bool,
    glbinfeas: &mut bool,
    fullshortening: bool,
) -> ScipResult {
    use crate::types::VarType;

    if nvars == 0 {
        return Ok(());
    }

    let transprob = crate::prob::TRANSPROB;
    let nprobvars = scip.probs[transprob].nvars;
    let n2 = 2 * nprobvars as usize;

    // Allocate temporary memory.
    let mut d = Shrink {
        bounds,
        boundtypes,
        newbounds: vec![0.0; n2],
        counts: vec![0; n2],
        countnonzeros: vec![0; n2],
        ncountnonzeros: 0,
        issetvar: vec![0; n2],
        nvars: nprobvars,
        foundbin: 0,
        foundnonbin: 0,
        implidx: vec![0; n2],
        nimplidx: 0,
        lastbounds: vec![0.0; n2],
    };

    let mut usebin = true;
    let mut usenonbin = true;
    let mut globalred = true;
    let mut start = i32::MAX;

    *nredvars = 0;
    *glbinfeas = false;

    // Initialize variable indices data.
    for v in 0..nvars as usize {
        let mut varidx = scip.vars[vars[v]].probindex;
        if boundtypes[v] {
            varidx += nprobvars;
        }

        // Initialize issetvar array.
        d.issetvar[varidx as usize] = v as i32 + 1;
    }

    // Check if implicit binary variables exist, because for these variables
    // the implications can be stored in the variable bounds instead of the
    // 'normal' implications.
    let nbinvars = scip.probs[transprob].nbinvars;
    let nintegers = scip.probs[transprob].nvars - scip.probs[transprob].ncontvars;
    let mut implbinvarsexist = false;
    let mut v = nintegers - 1;
    while v >= nbinvars {
        if crate::var::var_is_binary(scip, scip.probs[transprob].vars[v]) {
            implbinvarsexist = true;
            break;
        }
        v -= 1;
    }

    // Check for same implicit binary variables.
    for v in 0..nvars as usize {
        let var = vars[v];
        d.foundbin = i32::MAX;
        d.foundnonbin = i32::MAX;
        let mut reducedset = false;
        d.nimplidx = 0;

        let value = !boundtypes[v];

        let mut varidx = scip.vars[var].probindex;
        if !value {
            varidx += nprobvars;
        }
        let vi = varidx as usize;

        if usebin {
            // Collect clique data on binary variables.
            if crate::var::var_is_binary(scip, var) {
                collect_binary_clique_data(scip, &mut d, var, vi, v as i32, *nredvars, value);
            }
        }

        let isbintype = scip.vars[var].vartype == VarType::Binary && !crate::var::var_is_implied_integral(scip, var);

        // Only variable which have the vartype SCIP_VARTYPE_BINARY have
        // implications, otherwise the implications are saved as variable
        // bounds; we only check binary to non-binary implications if we can
        // detect further implications which either lead to global reductions
        // or to redundant set variables.
        if isbintype && (usenonbin || (usebin && implbinvarsexist)) {
            collect_non_binary_implication_data(scip, &mut d, var, vi, v as i32, *nredvars, value);
        }
        // Only variable which have the vartype != SCIP_VARTYPE_BINARY have
        // variable bounds; we only check the variable bounds if we can detect
        // further implications which either lead to global reductions or to
        // redundant set variables.
        else if !isbintype && (usenonbin || (usebin && implbinvarsexist)) {
            collect_non_binary_vbound_data(scip, &mut d, var, vi, v as i32, *nredvars);
        }

        // Reduce implication counters on all variables which are implied by a
        // variable now marked as redundant.
        if d.issetvar[vi] < 0 {
            // Correct implication counters and bounds, if the redundant variable
            // implies other variables we need to reduce the counter and get the
            // last bounds before this implication.
            let mut w = d.nimplidx - 1;
            while w >= 0 {
                let wu = w as usize;
                let ii = d.implidx[wu] as usize;
                d.counts[ii] -= 1;

                if d.implidx[wu] == d.countnonzeros[(d.ncountnonzeros - 1) as usize] && d.counts[ii] == 0 {
                    d.ncountnonzeros -= 1;
                }

                let probidx = if d.implidx[wu] < nprobvars { d.implidx[wu] } else { d.implidx[wu] - nprobvars };

                // Only for non-binary variables we need to correct the bounds.
                if !crate::var::var_is_binary(scip, scip.probs[transprob].vars[probidx]) && d.lastbounds[wu] != INVALID {
                    d.newbounds[ii] = d.lastbounds[wu];
                }
                w -= 1;
            }

            reducedset = true;
            *nredvars += 1;
        }

        // Check if we want to shorten the whole set of variables, or terminate
        // early if we did not find any further implication.
        if !fullshortening {
            // Check if it makes sense to look for further binary implications.
            if d.foundbin < i32::MAX && !reducedset {
                usebin = false;
            }
            // Check if it makes sense to look for further non-binary
            // implications.
            if d.foundnonbin < i32::MAX && !reducedset {
                usenonbin = false;
            }
        }

        // Are global reductions still possible.
        globalred = globalred && (d.foundbin < i32::MAX || d.foundnonbin < i32::MAX);

        // Remember the first possible position for a global bound change.
        if globalred {
            // Get correct variable index (we added nprobvars for the upper bound
            // implication).
            if d.foundbin < i32::MAX && d.foundbin >= nprobvars {
                d.foundbin -= nprobvars;
            }

            // Get correct variable index (we added nprobvars for the upper bound
            // implication).
            if d.foundnonbin < i32::MAX && d.foundnonbin >= nprobvars {
                d.foundnonbin -= nprobvars;
            }

            if start > d.foundbin {
                start = d.foundbin;
            }

            if start > d.foundnonbin {
                start = d.foundnonbin;
            }
        } else {
            start = i32::MAX;
        }

        // Check if it can find any global implications anymore.
        if !usebin && !usenonbin {
            break;
        }
    }

    // Remove redundant set variables.
    if *nredvars > 0 {
        let mut v = nvars - 1;
        while v >= 0 {
            let vu = v as usize;
            let mut varidx = scip.vars[vars[vu]].probindex;
            if boundtypes[vu] {
                varidx += nprobvars;
            }

            // If set variable was marked to be redundant remove it.
            if d.issetvar[varidx as usize] < 0 {
                redundants[vu] = true;
            }
            v -= 1;
        }
    }

    // If we found some global boundchanges, we perform then now.
    if globalred {
        let root = treer!(scip).root;
        let eventfilter = crate::scip::GLOBALEVENTFILTER;
        let origprob = crate::prob::ORIGPROB;

        // Check for same implicit binary variables.
        let mut v = start;
        while v < nprobvars {
            let vu = v as usize;
            let var = scip.probs[transprob].vars[vu];
            let (glb, gub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);

            if d.counts[vu] + *nredvars == nvars {
                if crate::var::var_is_binary(scip, var) {
                    if glb < 0.5 {
                        crate::tree::node_add_boundchg(scip, root, transprob, origprob, eventfilter, var, 1.0, BoundType::Lower, false)?;
                        *nglobalred += 1;

                        if d.issetvar[vu] > 0 {
                            *setredundant = true;
                        }
                    }
                } else {
                    // The new lower bound is greater than the global upper bound =>
                    // the problem is global infeasible.
                    if scip.set.is_lt(gub, d.newbounds[vu]) {
                        *glbinfeas = true;
                        break;
                    }

                    if scip.set.is_lt(glb, d.newbounds[vu]) {
                        crate::tree::node_add_boundchg(scip, root, transprob, origprob, eventfilter, var, d.newbounds[vu], BoundType::Lower, false)?;
                        *nglobalred += 1;

                        if d.issetvar[vu] > 0 && d.newbounds[vu] >= bounds[(d.issetvar[vu] - 1) as usize] {
                            *setredundant = true;
                        }
                    }
                }
            } else if d.counts[nprobvars as usize + vu] + *nredvars == nvars {
                let idx = nprobvars as usize + vu;
                if crate::var::var_is_binary(scip, var) {
                    if gub > 0.5 {
                        crate::tree::node_add_boundchg(scip, root, transprob, origprob, eventfilter, var, 0.0, BoundType::Upper, false)?;
                        *nglobalred += 1;

                        if d.issetvar[idx] > 0 {
                            *setredundant = true;
                        }
                    }
                } else {
                    // The new upper bound is small than the global upper bound =>
                    // the problem is global infeasible.
                    if scip.set.is_gt(glb, d.newbounds[idx]) {
                        *glbinfeas = true;
                        break;
                    }

                    if scip.set.is_gt(gub, d.newbounds[idx]) {
                        crate::tree::node_add_boundchg(scip, root, transprob, origprob, eventfilter, var, d.newbounds[idx], BoundType::Upper, false)?;
                        *nglobalred += 1;

                        if d.issetvar[idx] > 0 && d.newbounds[idx] <= bounds[(d.issetvar[idx] - 1) as usize] {
                            *setredundant = true;
                        }
                    }
                }
            }
            v += 1;
        }
    }

    // The C code resets its clean buffers (issetvar, counts) here; ours are
    // local, and CLEARRATIO only picks how C clears them.
    let _ = CLEARRATIO;

    Ok(())
}
