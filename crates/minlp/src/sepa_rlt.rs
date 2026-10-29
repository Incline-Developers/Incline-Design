//! Port of `sepa_rlt.c`: reformulation-linearization-technique separator.
//!
//! Detecting hidden products (`separating/rlt/detecthidden`, and with it
//! implicit products and their McCormick cuts) and projected rows
//! (`separating/rlt/useprojection`) are not ported; both are off by default.

use std::{any::Any, collections::HashMap};

use crate::{
    api,
    cons_nonlinear::{AuxExpr, get_bilin_term_idx_nonlinear, get_bilin_terms_nonlinear, get_n_bilin_terms_nonlinear},
    paramset::{paramset_add_bool, paramset_add_int, paramset_add_real},
    retcode::ScipResult,
    scip::Scip,
    scip_lp,
    types::{ConshdlrId, LpSolStat, RowId, ScipResultCode, SepaId, SolId, VarId, VarStatus, VarType},
    var::NONE,
};

const SEPA_NAME: &str = "rlt";
const SEPA_DESC: &str = "reformulation-linearization-technique separator";
/// Priority for separation.
const SEPA_PRIORITY: i32 = 10;
/// Frequency for separating cuts; zero means to separate only in the root
/// node.
const SEPA_FREQ: i32 = 0;
/// Maximal relative distance from the current node's dual bound to primal
/// bound compared to best node's dual bound for applying separation.
const SEPA_MAXBOUNDDIST: f64 = 1.0;
/// Does the separator use a secondary SCIP instance?
const SEPA_USESSUBSCIP: bool = false;
/// Should separation method be delayed, if other separators found cuts?
const SEPA_DELAY: bool = false;

/// Maximum number of unknown bilinear terms a row can have to be used.
const DEFAULT_MAXUNKNOWNTERMS: i32 = 0;
/// Maximum number of variables that will be used to compute rlt cuts.
const DEFAULT_MAXUSEDVARS: i32 = 100;
/// Maximum number of cuts that will be added per round.
const DEFAULT_MAXNCUTS: i32 = -1;
/// Maximum number of separation rounds per node (-1: unlimited).
const DEFAULT_MAXROUNDS: i32 = 1;
/// Maximum number of separation rounds in the root node (-1: unlimited).
const DEFAULT_MAXROUNDSROOT: i32 = 10;
/// Whether only equality rows should be used for rlt cuts.
const DEFAULT_ONLYEQROWS: bool = false;
/// Whether only continuous rows should be used for rlt cuts.
const DEFAULT_ONLYCONTROWS: bool = false;
/// Whether only original variables and rows should be used for rlt cuts.
const DEFAULT_ONLYORIGINAL: bool = true;
/// Whether the separator should also be used in sub-scips.
const DEFAULT_USEINSUBSCIP: bool = false;
/// Whether the separator should first check projected rows.
const DEFAULT_USEPROJECTION: bool = false;
/// Whether implicit products should be detected and separated by McCormick.
const DEFAULT_DETECTHIDDEN: bool = false;
/// Whether RLT cuts should be added for hidden products.
const DEFAULT_HIDDENRLT: bool = false;
/// Whether globally valid RLT cuts are added to the global cut pool.
const DEFAULT_ADDTOPOOL: bool = true;

/// Threshold for score of cut relative to best score to be considered good,
/// so that less strict filtering is applied.
const DEFAULT_GOODSCORE: f64 = 1.0;
/// Threshold for score of cut relative to best score to be discarded.
const DEFAULT_BADSCORE: f64 = 0.5;
/// Weight of objective parallelism in cut score calculation.
const DEFAULT_OBJPARALWEIGHT: f64 = 0.0;
/// Weight of efficacy in cut score calculation.
const DEFAULT_EFFICACYWEIGHT: f64 = 1.0;
/// Weight of directed cutoff distance in cut score calculation.
const DEFAULT_DIRCUTOFFDISTWEIGHT: f64 = 0.0;
/// Maximum parallelism for good cuts.
const DEFAULT_GOODMAXPARALL: f64 = 0.1;
/// Maximum parallelism for non-good cuts.
const DEFAULT_MAXPARALL: f64 = 0.1;

/// Maximum allowed variable bound for computing an RLT-cut.
const MAXVARBOUND: f64 = 1e+5;

/*
 * Data structures
 */

/// `SCIP_SepaData`: separator data.
#[derive(Default)]
struct SepaData {
    /// Nonlinear constraint handler.
    conshdlr: ConshdlrId,
    /// Indicates whether the sepadata has been initialized yet.
    iscreated: bool,
    /// Indicates that this is the first round and original rows are used.
    isinitialround: bool,

    /* bilinear variables */
    /// Variables that occur in bilinear terms sorted by priority.
    varssorted: Vec<VarId>,
    /// Maps each bilinear var (by index) to the vars appearing together with
    /// it in bilinear products, sorted by index (`ADJACENTVARDATA`).
    bilinvardatamap: Option<HashMap<i32, Vec<VarId>>>,
    /// Priorities of variables.
    varpriorities: Vec<i32>,

    /* information about bilinear terms */
    /// Position of the auxexpr that is equal to the product (-1 if none).
    eqauxexpr: Vec<i32>,
    /// Total number of bilinear terms.
    nbilinterms: i32,

    /* parameters */
    /// Maximum number of unknown bilinear terms a row can have to be used
    /// (-1: unlimited).
    maxunknownterms: i32,
    /// Maximum number of variables that will be used to compute rlt cuts (-1:
    /// unlimited).
    maxusedvars: i32,
    /// Maximum number of cuts that will be added per round (-1: unlimited).
    maxncuts: i32,
    /// Maximum number of separation rounds per node (-1: unlimited).
    maxrounds: i32,
    /// Maximum number of separation rounds in the root node (-1: unlimited).
    maxroundsroot: i32,
    /// Whether only equality rows should be used for rlt cuts.
    onlyeqrows: bool,
    /// Whether only continuous rows should be used for rlt cuts.
    onlycontrows: bool,
    /// Whether only original rows and variables should be used for rlt cuts.
    onlyoriginal: bool,
    /// Whether the separator should also be used in sub-scips.
    useinsubscip: bool,
    /// Whether the separator should first check projected rows.
    useprojection: bool,
    /// Whether implicit products should be detected and separated by
    /// McCormick.
    detecthidden: bool,
    /// Whether RLT cuts should be added for hidden products.
    hiddenrlt: bool,
    /// Whether globally valid RLT cuts are added to the global cut pool.
    addtopool: bool,

    /* cut selection parameters */
    /// Threshold for score of cut relative to best score to be considered
    /// good, so that less strict filtering is applied.
    goodscore: f64,
    /// Threshold for score of cut relative to best score to be discarded.
    badscore: f64,
    /// Weight of objective parallelism in cut score calculation.
    objparalweight: f64,
    /// Weight of efficacy in cut score calculation.
    efficacyweight: f64,
    /// Weight of directed cutoff distance in cut score calculation.
    dircutoffdistweight: f64,
    /// Maximum parallelism for good cuts.
    goodmaxparall: f64,
    /// Maximum parallelism for non-good cuts.
    maxparall: f64,
}

impl SepaData {
    fn nbilinvars(&self) -> i32 {
        self.varssorted.len() as i32
    }
}

#[inline]
fn cmax(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

#[inline]
fn cmin(a: f64, b: f64) -> f64 {
    if a <= b { a } else { b }
}

/*
 * Local methods
 */

/// `addAdjacentVars`: store a pair of adjacent variables.
fn add_adjacent_vars(scip: &Scip, adjvarmap: &mut HashMap<i32, Vec<VarId>>, vars: [VarId; 2]) {
    // Repeat for each variable of the new pair.
    for v1 in 0..2 {
        let v2 = 1 - v1;

        // Look for data of the first variable; if the first variable has not
        // been added to adjvarmap yet, add it here.
        let adjacentvars = adjvarmap.entry(scip.vars[vars[v1]].index).or_default();

        // Look for second variable in adjacentvars of the first variable.
        if adjacentvars.is_empty() {
            adjacentvars.push(vars[v2]);
        } else {
            // Add second var to adjacentvars at the correct position, if not
            // already added.
            if let Err(pos2) = crate::sort::sortedvec_find(adjacentvars, |&a, &b| crate::var::var_comp(scip, a, b), vars[v2]) {
                adjacentvars.insert(pos2, vars[v2]);
            }
        }

        // If this is a self-adjacent var, only need to add the connection once.
        if vars[v1] == vars[v2] {
            break;
        }
    }
}

/// `getAdjacentVars`: returns the array of adjacent variables for a given
/// variable.
fn get_adjacent_vars(scip: &Scip, adjvarmap: &HashMap<i32, Vec<VarId>>, var: VarId) -> Vec<VarId> {
    adjvarmap.get(&scip.vars[var].index).cloned().unwrap_or_default()
}

/// `freeSepaData`: free separator data.
fn free_sepa_data(scip: &mut Scip, sepadata: &mut SepaData) -> ScipResult {
    if sepadata.nbilinvars() != 0 {
        // Release bilinvars that were captured for rlt and free all related
        // arrays.
        sepadata.bilinvardatamap = None;

        for mut v in std::mem::take(&mut sepadata.varssorted) {
            api::release_var(scip, &mut v)?;
        }

        sepadata.varpriorities = Vec::new();
    }

    // Free the remaining array.
    sepadata.eqauxexpr = Vec::new();

    sepadata.iscreated = false;

    Ok(())
}

/// `getOriginalRows`: creates and returns rows of original linear
/// constraints.
fn get_original_rows(scip: &Scip) -> Vec<RowId> {
    let prob = &scip.probs[crate::prob::TRANSPROB];
    let mut rows = Vec::new();

    for &cons in &prob.conss.0[..api::get_n_conss(scip) as usize] {
        let row = cons_get_row(scip, cons);

        if row != NONE {
            rows.push(row);
        }
    }

    rows
}

/// `SCIPconsGetRow` (`misc_linear.c`): the row of a constraint, or `NONE`.
fn cons_get_row(scip: &Scip, cons: crate::types::ConsId) -> RowId {
    let conshdlrname = scip.conshdlrs[scip.conss[cons].conshdlr].name.as_str();

    match conshdlrname {
        "linear" => crate::cons_linear::get_row_linear(scip, cons),
        "setppc" => crate::cons_setppc::get_row_setppc(scip, cons),
        "logicor" => crate::cons_logicor::get_row_logicor(scip, cons),
        "knapsack" => crate::cons_knapsack::get_row_knapsack(scip, cons),
        "varbound" => crate::cons_varbound::get_row_varbound(scip, cons),
        "exactlinear" => unimplemented!("SCIPgetRowExactLinear"),
        _ => NONE,
    }
}

/// `storeSuitableRows`: fills an array of rows suitable for RLT cut
/// generation, with `row_to_pos` linking row indices to positions in it.
fn store_suitable_rows(scip: &Scip, sepa: SepaId, sepadata: &SepaData, prob_rows: &[RowId], row_to_pos: &mut HashMap<i32, usize>, allowlocal: bool) -> Vec<RowId> {
    let mut rows = Vec::new();

    for &r in prob_rows {
        let row = &scip.rows[r];
        let iseqrow = scip.set.is_eq(row.lhs, row.rhs);

        // If equality rows are requested, only those can be used.
        if sepadata.onlyeqrows && !iseqrow {
            continue;
        }

        // If global cuts are requested, only globally valid rows can be used.
        if !allowlocal && row.local {
            continue;
        }

        // If continuous rows are requested, only those can be used.
        if sepadata.onlycontrows {
            let mut iscontrow = true;

            // Check row for integral variables.
            for j in 0..row.len as usize {
                if scip.cols[row.cols[j]].integral {
                    iscontrow = false;
                    break;
                }
            }

            if !iscontrow {
                continue;
            }
        }

        // Don't try to use rows that have been generated by the RLT separator.
        if crate::lp::row_get_origin_sepa(scip, r) == sepa {
            continue;
        }

        // If we are here, the row has passed all checks and should be added to
        // rows.
        row_to_pos.insert(row.index, rows.len());
        rows.push(r);
    }

    rows
}

/// `addProductVars`: saves variables x and y to separator data and stores
/// information about their connection; variables must be captured
/// separately.
fn add_product_vars(scip: &Scip, sepadata: &mut SepaData, x: VarId, y: VarId, varmap: &mut HashMap<i32, usize>, nlocks: i32) {
    if sepadata.bilinvardatamap.is_none() {
        sepadata.bilinvardatamap = Some(HashMap::new());
    }

    let xidx = scip.vars[x].index;
    let yidx = scip.vars[y].index;

    let xpos = match varmap.get(&xidx) {
        Some(&pos) => pos,
        None => {
            // Add x to sepadata and initialise its priority.
            varmap.insert(xidx, sepadata.varssorted.len());
            sepadata.varssorted.push(x);
            sepadata.varpriorities.push(0);
            sepadata.varssorted.len() - 1
        }
    };

    // Add locks to priority of x.
    sepadata.varpriorities[xpos] += nlocks;

    if xidx != yidx {
        let ypos = match varmap.get(&yidx) {
            Some(&pos) => pos,
            None => {
                // Add y to sepadata and initialise its priority.
                varmap.insert(yidx, sepadata.varssorted.len());
                sepadata.varssorted.push(y);
                sepadata.varpriorities.push(0);
                sepadata.varssorted.len() - 1
            }
        };

        // Add locks to priority of y.
        sepadata.varpriorities[ypos] += nlocks;
    }

    // Remember the connection between x and y.
    add_adjacent_vars(scip, sepadata.bilinvardatamap.as_mut().unwrap(), [x, y]);
}

/// `createSepaData`: creates separation data.
fn create_sepa_data(scip: &mut Scip, sepadata: &mut SepaData) -> ScipResult {
    // Initialize some fields of sepadata.
    sepadata.varssorted = Vec::new();
    sepadata.varpriorities = Vec::new();
    sepadata.bilinvardatamap = None;
    sepadata.eqauxexpr = Vec::new();

    // Get total number of bilinear terms.
    sepadata.nbilinterms = get_n_bilin_terms_nonlinear(scip, sepadata.conshdlr) as i32;

    // Skip if there are no bilinear terms and implicit product detection is
    // off.
    if sepadata.nbilinterms == 0 && !sepadata.detecthidden {
        return Ok(());
    }

    // Create variable map.
    let mut varmap = HashMap::new();

    // Store the information of all variables that appear bilinearly.
    for i in 0..sepadata.nbilinterms as usize {
        let (x, y, auxvar, nlocks) = {
            let t = &get_bilin_terms_nonlinear(scip, sepadata.conshdlr)[i];
            let auxvar = if t.nauxexprs == 0 { t.auxvar } else { NONE };
            (t.x, t.y, auxvar, t.nlockspos + t.nlocksneg)
        };

        // Skip bilinear term if it does not have an auxiliary variable (the
        // union `aux` holds the variable only without auxexprs).
        if get_bilin_terms_nonlinear(scip, sepadata.conshdlr)[i].nauxexprs == 0 && auxvar == NONE {
            continue;
        }

        // If only original variables should be used, skip products that
        // contain at least one auxiliary variable.
        if sepadata.onlyoriginal && (scip.vars[x].relaxationonly || scip.vars[y].relaxationonly) {
            continue;
        }

        add_product_vars(scip, sepadata, x, y, &mut varmap, nlocks);
    }

    if sepadata.detecthidden {
        unimplemented!("separating/rlt/detecthidden (detectHiddenProducts)");
    }

    if sepadata.nbilinterms == 0 {
        return Ok(());
    }

    // Mark positions of aux.exprs that must be equal to the product.
    let terms = get_bilin_terms_nonlinear(scip, sepadata.conshdlr);
    sepadata.eqauxexpr = vec![-1; sepadata.nbilinterms as usize];
    for i in 0..sepadata.nbilinterms as usize {
        for (j, auxexpr) in terms[i].auxexprs[..terms[i].nauxexprs as usize].iter().enumerate() {
            if auxexpr.underestimate & auxexpr.overestimate {
                sepadata.eqauxexpr[i] = j as i32;
                break;
            }
        }
    }

    // Find maxnumber of variables that occur most often and sort them by
    // number of occurrences (same as normal sort, except that entries at
    // positions maxusedvars..nbilinvars may be unsorted at end).
    let nbilinvars = sepadata.nbilinvars();
    let k = if sepadata.maxusedvars < nbilinvars { sepadata.maxusedvars } else { nbilinvars - 1 };
    if k >= 0 {
        crate::sort::select_down(&mut sepadata.varpriorities[..], (&mut sepadata.varssorted[..],), crate::sort::cmp_int, k as usize);
    }

    // Capture all variables.
    for &v in &sepadata.varssorted {
        crate::var::var_capture(scip, v);
    }

    // Mark that separation data has been created.
    sepadata.iscreated = true;
    sepadata.isinitialround = true;

    Ok(())
}

/// `isAcceptableRow`: tests if a row contains too many unknown bilinear
/// terms w.r.t. the parameters; returns `(acceptable, currentnunknown)`.
fn is_acceptable_row(scip: &Scip, sepadata: &SepaData, row: RowId, var: VarId) -> (bool, i32) {
    let mut currentnunknown = 0;
    let terms = get_bilin_terms_nonlinear(scip, sepadata.conshdlr);
    let r = &scip.rows[row];

    let mut i = 0;
    while i < r.len as usize && (sepadata.maxunknownterms < 0 || currentnunknown <= sepadata.maxunknownterms) {
        let idx = get_bilin_term_idx_nonlinear(scip, sepadata.conshdlr, var, scip.cols[r.cols[i]].var);
        i += 1;

        // If the product hasn't been found, no auxiliary expressions for it are
        // known.
        if idx < 0 {
            currentnunknown += 1;
            continue;
        }

        // Known terms are only those that have an aux.var or equality
        // estimators.
        let t = &terms[idx as usize];
        if sepadata.eqauxexpr[idx as usize] == -1 && !(t.nauxexprs == 0 && t.auxvar != NONE) {
            currentnunknown += 1;
        }
    }

    (sepadata.maxunknownterms < 0 || currentnunknown <= sepadata.maxunknownterms, currentnunknown)
}

/// `addAuxexprCoefs`: adds coefficients and constant of an auxiliary
/// expression.
#[allow(clippy::too_many_arguments)]
fn add_auxexpr_coefs(scip: &Scip, var1: VarId, var2: VarId, auxexpr: &AuxExpr, coef: f64, coefaux: &mut f64, coef1: &mut f64, coef2: &mut f64, cst: &mut f64) {
    *coefaux += auxexpr.coefs[0] * coef;

    // In auxexpr, x goes before y and has the smaller index, so compare vars to
    // figure out which one is x and which is y.
    if crate::var::var_compare(scip, var1, var2) < 1 {
        *coef1 += auxexpr.coefs[1] * coef;
        *coef2 += auxexpr.coefs[2] * coef;
    } else {
        *coef1 += auxexpr.coefs[2] * coef;
        *coef2 += auxexpr.coefs[1] * coef;
    }
    *cst += coef * auxexpr.cst;
}

/// `addRltTerm`: add a linear term `coef`*`colvar` multiplied by a bound
/// factor (var - lb(var)) or (ub(var) - var); adds the linear term with
/// `colvar` to `cut` and updates `coefvar` and `cst`.
#[allow(clippy::too_many_arguments)]
fn add_rlt_term(
    scip: &mut Scip,
    sepadata: &SepaData,
    sol: SolId,
    bestunderest: Option<&[i32]>,
    bestoverest: Option<&[i32]>,
    cut: RowId,
    var: VarId,
    colvar: VarId,
    coef: f64,
    uselb: bool,
    uselhs: bool,
    local: bool,
    compute_eq_cut: bool,
    coefvar: &mut f64,
    cst: &mut f64,
    success: &mut bool,
) -> ScipResult {
    let (lbvar, ubvar) = if compute_eq_cut {
        (0.0, 0.0)
    } else if local {
        (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub)
    } else {
        (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub)
    };

    let refpointvar = cmax(lbvar, cmin(ubvar, api::get_sol_val(scip, sol, var)));

    let signfactor = if uselb { 1.0 } else { -1.0 };
    let boundfactor = if uselb { -lbvar } else { ubvar };

    // Coefficient of the bilinear term.
    let coefterm = coef * signfactor;
    // Coefficient of the linear term.
    let mut coefcolvar = coef * boundfactor;
    // Coefficient of the auxiliary variable corresponding to the bilinear
    // term.
    let mut coefauxvar = 0.0;
    let mut auxvar = NONE;

    // First, add the linearisation of the bilinear term.
    let idx = get_bilin_term_idx_nonlinear(scip, sepadata.conshdlr, var, colvar);
    let mut auxpos = -1;

    let (nauxexprs, termauxvar) = if idx >= 0 {
        let t = &get_bilin_terms_nonlinear(scip, sepadata.conshdlr)[idx as usize];
        (t.nauxexprs, t.auxvar)
    } else {
        (0, NONE)
    };

    // For an implicit term, get the position of the best estimator.
    if idx >= 0 && nauxexprs > 0 {
        auxpos = if compute_eq_cut {
            // Use an equality auxiliary expression (which should exist for
            // computeEqCut to be TRUE).
            sepadata.eqauxexpr[idx as usize]
        } else if (uselhs && coefterm > 0.0) || (!uselhs && coefterm < 0.0) {
            // Use an overestimator.
            bestoverest.unwrap()[idx as usize]
        } else {
            // Use an underestimator.
            bestunderest.unwrap()[idx as usize]
        };
    }

    // If the term is implicit and a suitable auxiliary expression for
    // var*colvar exists, add the coefficients of the auxiliary expression for
    // coefterm*var*colvar to coefauxvar, coefcolvar, coefvar and cst.
    if auxpos >= 0 {
        let auxexpr = get_bilin_terms_nonlinear(scip, sepadata.conshdlr)[idx as usize].auxexprs[auxpos as usize];
        add_auxexpr_coefs(scip, var, colvar, &auxexpr, coefterm, &mut coefauxvar, coefvar, &mut coefcolvar, cst);
        auxvar = auxexpr.auxvar;
    }
    // For an existing term, use the auxvar if there is one.
    else if idx >= 0 && nauxexprs == 0 && termauxvar != NONE {
        coefauxvar += coefterm;
        auxvar = termauxvar;
    }
    // Otherwise, use clique information or the McCormick estimator in place of
    // the bilinear term.
    else if colvar != var {
        let mut found_clique = false;
        let (lbcolvar, ubcolvar) = if local {
            (scip.vars[colvar].locdom.lb, scip.vars[colvar].locdom.ub)
        } else {
            (scip.vars[colvar].glbdom.lb, scip.vars[colvar].glbdom.ub)
        };
        let refpointcolvar = cmax(lbcolvar, cmin(ubcolvar, api::get_sol_val(scip, sol, colvar)));

        if lbcolvar.abs() > MAXVARBOUND || ubcolvar.abs() > MAXVARBOUND {
            *success = false;
            return Ok(());
        }

        // If both variables are binary, check if they are contained together in
        // some clique.
        if scip.vars[var].vartype == VarType::Binary && scip.vars[colvar].vartype == VarType::Binary {
            let varcliques = |scip: &Scip, value: bool| -> Vec<usize> {
                scip.vars[var]
                    .cliquelist
                    .as_ref()
                    .map_or(Vec::new(), |l| l.cliques[value as usize].0[..l.ncliques[value as usize] as usize].to_vec())
            };

            // Look through cliques containing var.
            for c in varcliques(scip, true) {
                // var + colvar <= 1 => var*colvar = 0
                if crate::implics::clique_has_var(&scip.cliques[c], colvar, true) {
                    // Product is zero, add nothing.
                    found_clique = true;
                    break;
                }

                // var + (1-colvar) <= 1 => var*colvar = var
                if crate::implics::clique_has_var(&scip.cliques[c], colvar, false) {
                    *coefvar += coefterm;
                    found_clique = true;
                    break;
                }
            }

            if !found_clique {
                // Look through cliques containing complement of var.
                for c in varcliques(scip, false) {
                    // (1-var) + colvar <= 1 => var*colvar = colvar
                    if crate::implics::clique_has_var(&scip.cliques[c], colvar, true) {
                        coefcolvar += coefterm;
                        found_clique = true;
                        break;
                    }

                    // (1-var) + (1-colvar) <= 1 => var*colvar = var + colvar - 1
                    if crate::implics::clique_has_var(&scip.cliques[c], colvar, false) {
                        *coefvar += coefterm;
                        coefcolvar += coefterm;
                        *cst -= coefterm;
                        found_clique = true;
                        break;
                    }
                }
            }
        }

        if !found_clique {
            crate::nlhdlr_bilinear::add_bilin_mccormick(
                scip,
                coefterm,
                lbvar,
                ubvar,
                refpointvar,
                lbcolvar,
                ubcolvar,
                refpointcolvar,
                uselhs,
                coefvar,
                &mut coefcolvar,
                cst,
                success,
            );
            if !*success {
                return Ok(());
            }
        }
    }
    // Or, if it's a quadratic term, use a secant for overestimation and a
    // gradient for underestimation.
    else {
        // For a binary var, var^2 = var.
        if scip.vars[var].vartype == VarType::Binary {
            *coefvar += coefterm;
        } else {
            // Depending on over-/underestimation and the sign of the column
            // variable, compute secant or tangent.
            if (uselhs && coefterm > 0.0) || (!uselhs && coefterm < 0.0) {
                crate::expr_pow::add_square_secant(scip, coefterm, lbvar, ubvar, coefvar, cst, success);
            } else {
                let isint = crate::var::var_is_integral(scip, var);
                crate::expr_pow::add_square_linearization(scip, coefterm, refpointvar, isint, coefvar, cst, success);
            }

            if !*success {
                return Ok(());
            }
        }
    }

    // Add the auxiliary variable if its coefficient is nonzero.
    if !scip.set.is_zero(coefauxvar) {
        api::add_var_to_row(scip, cut, auxvar, coefauxvar)?;
    }

    // We are done with the product linearisation, now add the term which comes
    // from multiplying coef*colvar by the constant part of the bound factor.
    if colvar != var {
        api::add_var_to_row(scip, cut, colvar, coefcolvar)?;
    } else {
        *coefvar += coefcolvar;
    }

    Ok(())
}

/// `computeRltCut`: creates the RLT cut formed by multiplying a given row
/// with (x - lb) or (ub - x).
///
/// In detail:
/// - The row is multiplied either with (x - lb(x)) or with (ub(x) - x),
///   depending on parameter `uselb`, or by x if this is an equality cut
/// - The (inequality) cut is computed either for lhs or rhs, depending on
///   parameter `uselhs`.
/// - Terms for which no auxiliary variable and no clique relation exists are
///   replaced by either McCormick, secants, or gradient linearization cuts.
///
/// Returns the cut (`NONE` if none was created); `success` tells whether it
/// was created successfully.
#[allow(clippy::too_many_arguments)]
fn compute_rlt_cut(
    scip: &mut Scip,
    sepa: SepaId,
    sepadata: &SepaData,
    row: RowId,
    sol: SolId,
    bestunderest: Option<&[i32]>,
    bestoverest: Option<&[i32]>,
    var: VarId,
    success: &mut bool,
    uselb: bool,
    uselhs: bool,
    local: bool,
    compute_eq_cut: bool,
) -> ScipResult<RowId> {
    let (lhs, rhs, rowname, rowcst) = {
        let r = &scip.rows[row];
        (r.lhs, r.rhs, r.name.clone(), r.constant)
    };

    // Get data for given variable.
    let (lbvar, ubvar) = if compute_eq_cut {
        (0.0, 0.0)
    } else if local {
        (scip.vars[var].locdom.lb, scip.vars[var].locdom.ub)
    } else {
        (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub)
    };

    // Get row side.
    let consside = if uselhs { lhs } else { rhs };

    // If the bounds are too large or the respective side is infinity, skip
    // this cut.
    if (uselb && lbvar.abs() > MAXVARBOUND) || (!uselb && ubvar.abs() > MAXVARBOUND) || scip.set.is_infinity(consside.abs()) {
        *success = false;
        return Ok(NONE);
    }

    // Initialize some factors needed for computation.
    let mut coefvar = 0.0;
    let mut cstterm = 0.0;
    let signfactor = if uselb { 1.0 } else { -1.0 };
    let boundfactor = if uselb { -lbvar } else { ubvar };
    *success = true;

    // Create an empty row which we then fill with variables step by step.
    let cutname = format!(
        "rlt_cut_{}_{}_{}_{}_{}",
        rowname,
        if uselhs { "lhs" } else { "rhs" },
        scip.vars[var].name,
        if uselb { "lb" } else { "ub" },
        api::get_n_lps(scip)
    );
    let infinity = scip.set.infinity();
    let cutlocal = api::get_depth(scip) > 0 && local;
    let cut = scip_lp::create_empty_row_sepa(scip, sepa, &cutname, -infinity, infinity, cutlocal, false, false)?;

    scip_lp::cache_row_extensions(scip, cut);

    // Iterate over all variables in the row and add the corresponding terms
    // coef*colvar*(bound factor) to the cuts.
    let (rowcols, rowvals) = {
        let r = &scip.rows[row];
        (r.cols.0[..r.len as usize].to_vec(), r.vals.0[..r.len as usize].to_vec())
    };
    for i in 0..rowcols.len() {
        let colvar = scip.cols[rowcols[i]].var;
        add_rlt_term(
            scip,
            sepadata,
            sol,
            bestunderest,
            bestoverest,
            cut,
            var,
            colvar,
            rowvals[i],
            uselb,
            uselhs,
            local,
            compute_eq_cut,
            &mut coefvar,
            &mut cstterm,
            success,
        )?;
    }

    if cstterm.abs() > MAXVARBOUND {
        *success = false;
        return Ok(cut);
    }

    // Multiply (x-lb) or (ub -x) with the lhs and rhs of the row.
    coefvar += signfactor * (rowcst - consside);
    let finalside = boundfactor * (consside - rowcst) - cstterm;

    // Set the coefficient of var and update the side.
    api::add_var_to_row(scip, cut, var, coefvar)?;
    scip_lp::flush_row_extensions(scip, cut)?;
    if uselhs || compute_eq_cut {
        crate::lp::row_chg_lhs(scip, cut, finalside)?;
    }
    if !uselhs || compute_eq_cut {
        crate::lp::row_chg_rhs(scip, cut, finalside)?;
    }

    Ok(cut)
}

/// `addRowMark`: depending on the sign of the coefficient and violation,
/// set or update mark which cut is required:
/// - 1 - cuts for axy < aw case,
/// - 2 - cuts for axy > aw case,
/// - 3 - cuts for both cases
fn add_row_mark(ridx: i32, a: f64, violatedbelow: bool, violatedabove: bool, row_idcs: &mut Vec<i32>, row_marks: &mut Vec<u32>) {
    let newmark = if (a > 0.0 && violatedbelow) || (a < 0.0 && violatedabove) {
        // axy < aw case
        1
    } else {
        // axy > aw case
        2
    };

    // Find row idx in row_idcs.
    match crate::sort::sortedvec_find(row_idcs, crate::sort::cmp_int, ridx) {
        // We found the row index: update the mark at pos.
        Ok(pos) => row_marks[pos] |= newmark,
        // The given row index does not yet exist in row_idcs: insert row index
        // at the correct position.
        Err(pos) => {
            row_idcs.insert(pos, ridx);
            row_marks.insert(pos, newmark);
        }
    }
}

/// `markRowsXj`: mark all rows that should be multiplied by xj; returns the
/// marked row indices and their marks.
#[allow(clippy::too_many_arguments)]
fn mark_rows_xj(
    scip: &Scip,
    sepadata: &SepaData,
    conshdlr: ConshdlrId,
    sol: SolId,
    j: usize,
    local: bool,
    row_to_pos: &HashMap<i32, usize>,
    bestunderest: Option<&[i32]>,
    bestoverest: Option<&[i32]>,
) -> (Vec<i32>, Vec<u32>) {
    let mut row_idcs = Vec::new();
    let mut row_marks = Vec::new();

    let xj = sepadata.varssorted[j];

    let valj = api::get_sol_val(scip, sol, xj);
    let (vlb, vub) = if local {
        (scip.vars[xj].locdom.lb, scip.vars[xj].locdom.ub)
    } else {
        (scip.vars[xj].glbdom.lb, scip.vars[xj].glbdom.ub)
    };

    if sepadata.useprojection && (scip.set.is_feas_eq(vlb, valj) || scip.set.is_feas_eq(vub, valj)) {
        // We don't want to multiply by variables that are at bound.
        return (row_idcs, row_marks);
    }

    let terms = get_bilin_terms_nonlinear(scip, conshdlr);
    let bilinadjvars = get_adjacent_vars(scip, sepadata.bilinvardatamap.as_ref().unwrap(), xj);

    // For each var which appears in a bilinear product together with xj, mark
    // rows.
    for &xi in &bilinadjvars {
        if scip.vars[xi].varstatus != VarStatus::Column {
            continue;
        }

        let vali = api::get_sol_val(scip, sol, xi);
        let (vlb, vub) = if local {
            (scip.vars[xi].locdom.lb, scip.vars[xi].locdom.ub)
        } else {
            (scip.vars[xi].glbdom.lb, scip.vars[xi].glbdom.ub)
        };

        // If we use projection, we aren't interested in products with variables
        // that are at bound.
        if sepadata.useprojection && (scip.set.is_feas_eq(vlb, vali) || scip.set.is_feas_eq(vub, vali)) {
            continue;
        }

        // Get the index of the bilinear product.
        let idx = get_bilin_term_idx_nonlinear(scip, conshdlr, xj, xi) as usize;
        let term = &terms[idx];

        // Skip implicit products if we don't want to add RLT cuts for them.
        if !sepadata.hiddenrlt && !term.existing {
            continue;
        }

        // Use the most violated under- and overestimators for this product; if
        // equality cuts are computed, we might end up using a different
        // auxiliary expression; so this is an optimistic (i.e. taking the
        // largest possible violation) estimation.
        let violatedbelow = if bestunderest.is_none_or(|b| b[idx] == -1) {
            // No violated implicit underestimation relations -> either use
            // auxvar or set violatedbelow to FALSE.
            term.nauxexprs == 0 && term.auxvar != NONE && scip.set.is_feas_positive(api::get_sol_val(scip, sol, term.auxvar) - valj * vali)
        } else {
            // If we are here, the relation with the best underestimator must be
            // violated.
            true
        };

        let violatedabove = if bestoverest.is_none_or(|b| b[idx] == -1) {
            // No violated implicit overestimation relations -> either use
            // auxvar or set violatedabove to FALSE.
            term.nauxexprs == 0 && term.auxvar != NONE && scip.set.is_feas_positive(valj * vali - api::get_sol_val(scip, sol, term.auxvar))
        } else {
            // If we are here, the relation with the best overestimator must be
            // violated.
            true
        };

        // Only violated products contribute to row marks.
        if !violatedbelow && !violatedabove {
            continue;
        }

        // Get the column of xi.
        let coli = &scip.cols[scip.vars[xi].data.col];

        // Mark the rows.
        for r in 0..coli.len as usize {
            let ridx = scip.rows[coli.rows[r]].index;

            if !row_to_pos.contains_key(&ridx) {
                // If row index is not in row_to_pos, it means that
                // storeSuitableRows decided to ignore this row.
                continue;
            }

            let a = coli.vals[r];
            if a == 0.0 {
                continue;
            }

            add_row_mark(ridx, a, violatedbelow, violatedabove, &mut row_idcs, &mut row_marks);
        }
    }

    (row_idcs, row_marks)
}

/// `separateRltCuts`: builds and adds the RLT cuts.
#[allow(clippy::too_many_arguments)]
fn separate_rlt_cuts(
    scip: &mut Scip,
    sepa: SepaId,
    sepadata: &SepaData,
    conshdlr: ConshdlrId,
    sol: SolId,
    row_to_pos: &HashMap<i32, usize>,
    rows: &[RowId],
    allowlocal: bool,
    bestunderestimators: Option<&[i32]>,
    bestoverestimators: Option<&[i32]>,
    result: &mut ScipResultCode,
) -> ScipResult {
    let uselb = [true, true, false, false];
    let uselhs = [true, false, true, false];
    let mut cuts: Vec<RowId> = Vec::new();
    *result = ScipResultCode::DidNotFind;

    // Loop through all variables that appear in bilinear products.
    let mut j = 0;
    while j < sepadata.varssorted.len() && (sepadata.maxusedvars < 0 || (j as i32) < sepadata.maxusedvars) {
        let xj = sepadata.varssorted[j];

        // Mark all rows for multiplier xj.
        let (row_idcs, mut row_marks) = mark_rows_xj(scip, sepadata, conshdlr, sol, j, allowlocal, row_to_pos, bestunderestimators, bestoverestimators);

        // Generate the projected cut and if it is violated, generate the actual
        // cut.
        for r in 0..row_idcs.len() {
            let pos = row_to_pos[&row_idcs[r]];
            let row = rows[pos];

            // Check whether this row and var fulfill the conditions.
            let (accepted, currentnunknown) = is_acceptable_row(scip, sepadata, row, xj);
            if !accepted {
                row_marks[r] = 0;
                continue;
            }

            let iseqrow = scip.set.is_eq(scip.rows[row].lhs, scip.rows[row].rhs);

            // If all terms are known and it is an equality row, compute equality
            // cut, that is, multiply row with (x-lb) and/or (ub-x) (but see also
            // @todo at top) otherwise, multiply row w.r.t. lhs and/or rhs with
            // (x-lb) and/or (ub-x) and estimate product terms that have no
            // aux.var or aux.expr.
            let buildeqcut = currentnunknown == 0 && iseqrow;

            // Go over all suitable combinations of sides and bounds and compute
            // the respective cuts.
            for k in 0..4 {
                // If equality cuts are possible, lhs and rhs cuts are equal so
                // skip rhs.
                if buildeqcut {
                    if k != 1 {
                        continue;
                    }
                }
                // Otherwise which cuts are generated depends on the marks.
                else {
                    if row_marks[r] == 1 && uselb[k] == uselhs[k] {
                        continue;
                    }

                    if row_marks[r] == 2 && uselb[k] != uselhs[k] {
                        continue;
                    }
                }

                let mut success = true;
                let mut cut = NONE;

                if sepadata.useprojection {
                    unimplemented!("separating/rlt/useprojection");
                }

                // If we don't use projection or if the projected cut was generated
                // successfully and is violated, generate the actual cut.
                if success {
                    cut = compute_rlt_cut(
                        scip,
                        sepa,
                        sepadata,
                        row,
                        sol,
                        bestunderestimators,
                        bestoverestimators,
                        xj,
                        &mut success,
                        uselb[k],
                        uselhs[k],
                        allowlocal,
                        buildeqcut,
                    )?;
                }

                if success {
                    let feasibility = get_row_feasibility(scip, cut);
                    success = scip.set.is_feas_negative(feasibility) || (sepadata.addtopool && !scip.rows[cut].local);
                }

                // If the cut was created successfully and is violated or (if
                // addtopool == TRUE) globally valid, it is added to the cuts
                // array.
                if success {
                    cuts.push(cut);
                } else {
                    // Release the cut.
                    if cut != NONE {
                        scip_lp::release_row(scip, &mut cut)?;
                    }
                }
            }

            // Clear row_marks[r] since it will be used for the next multiplier.
            row_marks[r] = 0;
        }
        j += 1;
    }

    // If cuts were found, we apply an additional filtering procedure, which is
    // similar to sepastore.
    let ncuts = cuts.len();
    if ncuts > 0 {
        let mut nselectedcuts = 0;

        let maxncuts = if sepadata.maxncuts == -1 { ncuts as i32 } else { sepadata.maxncuts };
        crate::cutsel_hybrid::select_cuts_hybrid(
            scip,
            &mut cuts,
            &[],
            None,
            sepadata.goodscore,
            sepadata.badscore,
            sepadata.goodmaxparall,
            sepadata.maxparall,
            sepadata.dircutoffdistweight,
            sepadata.efficacyweight,
            sepadata.objparalweight,
            0.0,
            maxncuts,
            &mut nselectedcuts,
        )?;

        for (i, cut) in cuts.iter_mut().enumerate() {
            if (i as i32) < nselectedcuts {
                // If selected, add global cuts to the pool and local cuts to the
                // sepastore.
                if scip.rows[*cut].local || !sepadata.addtopool {
                    let infeasible = scip_lp::add_row(scip, *cut, false)?;

                    *result = if infeasible { ScipResultCode::Cutoff } else { ScipResultCode::Separated };
                } else {
                    api::add_pool_cut(scip, *cut)?;
                }
            }

            // Release current cut.
            scip_lp::release_row(scip, cut)?;
        }
    }

    Ok(())
}

/// `SCIPgetRowFeasibility` (`scip_lp.c`).
fn get_row_feasibility(scip: &mut Scip, row: RowId) -> f64 {
    if crate::tree::tree_has_current_node_lp(scip) {
        crate::lp::row_get_lp_feasibility(scip, row)
    } else {
        crate::lp::row_get_pseudo_feasibility(scip, row)
    }
}

/*
 * Callback methods of separator
 */

/// `sepaCopyRlt`: copy method for separator plugins.
fn sepa_copy_rlt(scip: &mut Scip, _source: &Scip, _sepa: SepaId) -> ScipResult {
    include_sepa_rlt(scip)
}

/// `sepaFreeRlt`: destructor of separator to free user data.
fn sepa_free_rlt(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    scip.sepas[sepa].sepadata = None;
    Ok(())
}

/// `sepaExitsolRlt`: solving process deinitialization method of separator.
fn sepa_exitsol_rlt(scip: &mut Scip, sepa: SepaId) -> ScipResult {
    let mut data = scip.sepas[sepa].sepadata.take().unwrap();
    let sepadata = data.downcast_mut::<SepaData>().unwrap();

    let ret = if sepadata.iscreated { free_sepa_data(scip, sepadata) } else { Ok(()) };

    scip.sepas[sepa].sepadata = Some(data);
    ret
}

/// `sepaExeclpRlt`: LP solution separation method of separator.
fn sepa_execlp_rlt(scip: &mut Scip, sepa: SepaId, result: &mut ScipResultCode, allowlocal: bool, depth: i32) -> ScipResult {
    // The separator data is taken out while the separator runs.
    let mut data = scip.sepas[sepa].sepadata.take().unwrap();
    let ret = execlp(scip, sepa, data.downcast_mut::<SepaData>().unwrap(), result, allowlocal, depth);
    scip.sepas[sepa].sepadata = Some(data);
    ret
}

/// The body of `sepaExeclpRlt`.
fn execlp(scip: &mut Scip, sepa: SepaId, sepadata: &mut SepaData, result: &mut ScipResultCode, allowlocal: bool, depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    if sepadata.maxncuts == 0 {
        return Ok(());
    }

    // Don't run in a sub-SCIP or in probing.
    if crate::copy::get_subscip_depth(scip) > 0 && !sepadata.useinsubscip {
        return Ok(());
    }

    // Don't run in probing.
    if api::in_probing(scip) {
        return Ok(());
    }

    // Only call separator a given number of times at each node.
    let ncalls = scip.sepas[sepa].ncallsatnode;
    if (depth == 0 && sepadata.maxroundsroot >= 0 && ncalls >= sepadata.maxroundsroot) || (depth > 0 && sepadata.maxrounds >= 0 && ncalls >= sepadata.maxrounds) {
        return Ok(());
    }

    // If this is called for the first time, create the sepadata and start the
    // initial separation round.
    if !sepadata.iscreated {
        *result = ScipResultCode::DidNotFind;
        create_sepa_data(scip, sepadata)?;
    }

    // No bilinear terms available -> skip.
    if sepadata.nbilinvars() == 0 {
        return Ok(());
    }

    // Only call separator, if we are not close to terminating.
    if api::is_stopped(scip) {
        return Ok(());
    }

    // Only call separator, if an optimal LP solution is at hand.
    if api::get_lp_solstat(scip) != LpSolStat::Optimal {
        return Ok(());
    }

    // Get the rows, depending on settings.
    let prob_rows = if sepadata.isinitialround || sepadata.onlyoriginal {
        get_original_rows(scip)
    } else {
        api::get_lp_rows(scip)
    };

    // Save the suitable rows.
    let mut row_to_pos = HashMap::new();
    let rows = store_suitable_rows(scip, sepa, sepadata, &prob_rows, &mut row_to_pos, allowlocal);

    if rows.is_empty() {
        // No suitable rows found, free memory and exit.
        if sepadata.isinitialround || sepadata.onlyoriginal {
            sepadata.isinitialround = false;
        }
        return Ok(());
    }

    // Create the projected problem.
    if sepadata.useprojection {
        unimplemented!("separating/rlt/useprojection (createProjRows)");
    }

    // Separate the cuts.
    if sepadata.detecthidden {
        unimplemented!("separating/rlt/detecthidden (getBestEstimators, separateMcCormickImplicit)");
    } else {
        let conshdlr = sepadata.conshdlr;
        separate_rlt_cuts(scip, sepa, sepadata, conshdlr, NONE, &row_to_pos, &rows, allowlocal, None, None, result)?;
    }

    if sepadata.isinitialround || sepadata.onlyoriginal {
        sepadata.isinitialround = false;
    }

    Ok(())
}

/*
 * separator specific interface methods
 */

/// `SCIPincludeSepaRlt`: creates the RLT separator and includes it in SCIP.
pub fn include_sepa_rlt(scip: &mut Scip) -> ScipResult {
    // Create RLT separator data.
    let sepadata = SepaData {
        conshdlr: crate::set::set_find_conshdlr(scip, "nonlinear"),
        ..Default::default()
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
        Some(sepa_execlp_rlt),
        None,
        Some(Box::new(sepadata) as Box<dyn Any>),
    )?;

    // Set non fundamental callbacks via setter functions.
    let s = &mut scip.sepas[sepa];
    s.sepacopy = Some(sepa_copy_rlt);
    s.sepafree = Some(sepa_free_rlt);
    s.sepaexitsol = Some(sepa_exitsol_rlt);

    // Add RLT separator parameters.
    macro_rules! t {
        ($ty:ty, $field:ident) => {
            crate::data_target!($ty, sepas, sepa, sepadata, SepaData, $field)
        };
    }
    paramset_add_int(
        scip,
        "separating/rlt/maxncuts",
        "maximal number of rlt-cuts that are added per round (-1: unlimited)",
        t!(i32, maxncuts),
        false,
        DEFAULT_MAXNCUTS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/rlt/maxunknownterms",
        "maximal number of unknown bilinear terms a row is still used with (-1: unlimited)",
        t!(i32, maxunknownterms),
        false,
        DEFAULT_MAXUNKNOWNTERMS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/rlt/maxusedvars",
        "maximal number of variables used to compute rlt cuts (-1: unlimited)",
        t!(i32, maxusedvars),
        false,
        DEFAULT_MAXUSEDVARS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/rlt/maxrounds",
        "maximal number of separation rounds per node (-1: unlimited)",
        t!(i32, maxrounds),
        false,
        DEFAULT_MAXROUNDS,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_int(
        scip,
        "separating/rlt/maxroundsroot",
        "maximal number of separation rounds in the root node (-1: unlimited)",
        t!(i32, maxroundsroot),
        false,
        DEFAULT_MAXROUNDSROOT,
        -1,
        i32::MAX,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/rlt/onlyeqrows",
        "if set to true, only equality rows are used for rlt cuts",
        t!(bool, onlyeqrows),
        false,
        DEFAULT_ONLYEQROWS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/rlt/onlycontrows",
        "if set to true, only continuous rows are used for rlt cuts",
        t!(bool, onlycontrows),
        false,
        DEFAULT_ONLYCONTROWS,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/rlt/onlyoriginal",
        "if set to true, only original rows and variables are used",
        t!(bool, onlyoriginal),
        false,
        DEFAULT_ONLYORIGINAL,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/rlt/useinsubscip",
        "if set to true, rlt is also used in sub-scips",
        t!(bool, useinsubscip),
        false,
        DEFAULT_USEINSUBSCIP,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/rlt/useprojection",
        "if set to true, projected rows are checked first",
        t!(bool, useprojection),
        false,
        DEFAULT_USEPROJECTION,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/rlt/detecthidden",
        "if set to true, hidden products are detected and separated by McCormick cuts",
        t!(bool, detecthidden),
        false,
        DEFAULT_DETECTHIDDEN,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/rlt/hiddenrlt",
        "whether RLT cuts (TRUE) or only McCormick inequalities (FALSE) should be added for hidden products",
        t!(bool, hiddenrlt),
        false,
        DEFAULT_HIDDENRLT,
        None,
    )?;
    paramset_add_bool(
        scip,
        "separating/rlt/addtopool",
        "if set to true, globally valid RLT cuts are added to the global cut pool",
        t!(bool, addtopool),
        false,
        DEFAULT_ADDTOPOOL,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/rlt/goodscore",
        "threshold for score of cut relative to best score to be considered good, so that less strict filtering is applied",
        t!(f64, goodscore),
        true,
        DEFAULT_GOODSCORE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/rlt/badscore",
        "threshold for score of cut relative to best score to be discarded",
        t!(f64, badscore),
        true,
        DEFAULT_BADSCORE,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/rlt/objparalweight",
        "weight of objective parallelism in cut score calculation",
        t!(f64, objparalweight),
        true,
        DEFAULT_OBJPARALWEIGHT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/rlt/efficacyweight",
        "weight of efficacy in cut score calculation",
        t!(f64, efficacyweight),
        true,
        DEFAULT_EFFICACYWEIGHT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/rlt/dircutoffdistweight",
        "weight of directed cutoff distance in cut score calculation",
        t!(f64, dircutoffdistweight),
        true,
        DEFAULT_DIRCUTOFFDISTWEIGHT,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/rlt/goodmaxparall",
        "maximum parallelism for good cuts",
        t!(f64, goodmaxparall),
        true,
        DEFAULT_GOODMAXPARALL,
        0.0,
        1.0,
        None,
    )?;
    paramset_add_real(
        scip,
        "separating/rlt/maxparall",
        "maximum parallelism for non-good cuts",
        t!(f64, maxparall),
        true,
        DEFAULT_MAXPARALL,
        0.0,
        1.0,
        None,
    )?;

    Ok(())
}
