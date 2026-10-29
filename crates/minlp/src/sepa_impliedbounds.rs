//! Port of `sepa_impliedbounds.c`: implied bounds separator.

use std::any::Any;

use crate::{
    api,
    retcode::ScipResult,
    scip::Scip,
    scip_lp,
    types::{BoundType, ScipResultCode, SepaId, SolId, VarId, VarType},
    var::NONE,
};

const SEPA_NAME: &str = "impliedbounds";
const SEPA_DESC: &str = "implied bounds separator";
const SEPA_PRIORITY: i32 = -50;
const SEPA_FREQ: i32 = 10;
const SEPA_MAXBOUNDDIST: f64 = 1.0;
/// Does the separator use a secondary SCIP instance?
const SEPA_USESSUBSCIP: bool = false;
/// Should separation method be delayed, if other separators found cuts?
const SEPA_DELAY: bool = false;

/// Maximal allowed range of cut coefficients, relative to 1/feastol.
const RELCUTCOEFMAXRANGE: f64 = 1.0;
/// Should violated inequalities for cliques with 2 variables be separated?
const DEFAULT_USETWOSIZECLIQUES: bool = true;

/// Separator-specific data for the implied bounds separator.
struct SepaData {
    /// Should violated inequalities for cliques with 2 variables be
    /// separated?
    usetwosizecliques: bool,
}

/*
 * Local methods
 */

/// `addCut`: adds given cut with two variables, if it is violated.
#[allow(clippy::too_many_arguments)]
fn add_cut(scip: &mut Scip, sepa: SepaId, val1: f64, var1: VarId, solval1: f64, val2: f64, var2: VarId, solval2: f64, rhs: f64, cutoff: &mut bool, ncuts: &mut i32) -> ScipResult {
    *cutoff = false;

    // Calculate activity of cut.
    let activity = val1 * solval1 + val2 * solval2;

    // Check, if cut is violated.
    if api::is_efficacious(scip, activity - rhs) {
        // Create cut.
        let cutname = format!("implbd{}_{}", scip.stat.nlps, *ncuts);
        let inf = scip.set.infinity();
        let mut cut = scip_lp::create_empty_row_sepa(scip, sepa, &cutname, -inf, rhs, false, false, true)?;
        scip_lp::cache_row_extensions(scip, cut);
        api::add_var_to_row(scip, cut, var1, val1)?;
        api::add_var_to_row(scip, cut, var2, val2)?;
        scip_lp::flush_row_extensions(scip, cut)?;

        // Set cut rank: for implied bounds we always set to 1.
        scip.rows[cut].rank = 1;

        api::add_pool_cut(scip, cut)?;
        *ncuts += 1;

        // Release cut.
        scip_lp::release_row(scip, &mut cut)?;
    }

    Ok(())
}

/// `separateCuts`: searches and adds implied bound cuts that are violated
/// by the given solution value array.
#[allow(clippy::too_many_arguments)]
fn separate_cuts(scip: &mut Scip, sepa: SepaId, sol: SolId, solvals: &[f64], fracvars: &[VarId], fracvals: &[f64], cutoff: &mut bool, ncuts: &mut i32) -> ScipResult {
    *cutoff = false;
    *ncuts = 0;
    let feastol = scip.set.feastol();

    // Search binary variables for violated implications.
    for (i, &fracvar) in fracvars.iter().enumerate() {
        // Only process binary variables.
        if scip.vars[fracvar].vartype != VarType::Binary || crate::var::var_is_implied_integral(scip, fracvar) {
            continue;
        }

        // Implications of x == 1, then of x == 0.
        for varfixing in [true, false] {
            let (implvars, impltypes, implbounds) = match scip.vars[fracvar].implics.as_ref() {
                Some(im) => {
                    let k = varfixing as usize;
                    let n = im.nimpls[k] as usize;
                    (im.vars[k].0[..n].to_vec(), im.types[k].0[..n].to_vec(), im.bounds[k].0[..n].to_vec())
                }
                None => Default::default(),
            };

            // Try to add cuts for implications of x == 1
            //    x == 1 -> y <= p:  y <= ub + x * (p - ub)  <==>  y + (ub - p) * x <=  ub
            //    x == 1 -> y >= p:  y >= lb + x * (p - lb)  <==> -y + (p - lb) * x <= -lb
            // with lb (ub) = global lower (upper) bound of y; and for implications
            // of x == 0
            //    x == 0 -> y <= p:  y <= p + x * (ub - p)  <==>  y + (p - ub) * x <=  p
            //    x == 0 -> y >= p:  y >= p + x * (lb - p)  <==> -y + (lb - p) * x <= -p.
            for j in 0..implvars.len() {
                // Consider only implications with active implvar.
                let probindex = scip.vars[implvars[j]].probindex;
                if probindex < 0 {
                    continue;
                }

                let solval = solvals[probindex as usize];
                if impltypes[j] == BoundType::Upper {
                    let ub = scip.vars[implvars[j]].glbdom.ub;

                    // Consider only nonredundant and numerical harmless implications;
                    // note the C's `<=` for x == 1 and `<` for x == 0.
                    let harmless = if varfixing {
                        (ub - implbounds[j]) * feastol <= RELCUTCOEFMAXRANGE
                    } else {
                        (ub - implbounds[j]) * feastol < RELCUTCOEFMAXRANGE
                    };
                    if scip.set.is_le(implbounds[j], ub) && harmless {
                        // Add cut if violated.
                        if varfixing {
                            add_cut(scip, sepa, 1.0, implvars[j], solval, ub - implbounds[j], fracvar, fracvals[i], ub, cutoff, ncuts)?;
                        } else {
                            add_cut(scip, sepa, 1.0, implvars[j], solval, implbounds[j] - ub, fracvar, fracvals[i], implbounds[j], cutoff, ncuts)?;
                        }
                        if *cutoff {
                            return Ok(());
                        }
                    }
                } else {
                    let lb = scip.vars[implvars[j]].glbdom.lb;

                    // Consider only nonredundant and numerical harmless implications.
                    let harmless = if varfixing {
                        (implbounds[j] - lb) * feastol <= RELCUTCOEFMAXRANGE
                    } else {
                        (implbounds[j] - lb) * feastol < RELCUTCOEFMAXRANGE
                    };
                    if scip.set.is_ge(implbounds[j], lb) && harmless {
                        // Add cut if violated.
                        if varfixing {
                            add_cut(scip, sepa, -1.0, implvars[j], solval, implbounds[j] - lb, fracvar, fracvals[i], -lb, cutoff, ncuts)?;
                        } else {
                            add_cut(
                                scip,
                                sepa,
                                -1.0,
                                implvars[j],
                                solval,
                                lb - implbounds[j],
                                fracvar,
                                fracvals[i],
                                -implbounds[j],
                                cutoff,
                                ncuts,
                            )?;
                        }
                        if *cutoff {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    // Stop separation here if cliques should not be separated.
    if !scip.sepas[sepa].sepadata.as_ref().unwrap().downcast_ref::<SepaData>().unwrap().usetwosizecliques {
        return Ok(());
    }

    // Prepare clean clique data.
    *cutoff = crate::scip_var::cleanup_cliques(scip)?;
    if *cutoff {
        return Ok(());
    }

    let cliques = {
        let t = scip.cliquetable.as_ref().unwrap();
        t.cliques.0[..t.ncliques as usize].to_vec()
    };

    // Loop over cliques of size 2 which are essentially implications and add
    // cuts if they are violated.
    for clique in cliques {
        // Only consider inequality cliques of size 2.
        let c = &scip.cliques[clique];
        if c.nvars != 2 || c.equation {
            continue;
        }

        // Get variables and values of the clique.
        let (v0, v1) = (c.vars[0usize], c.vars[1usize]);
        let (b0, b1) = (c.values[0usize], c.values[1usize]);

        // Calculate right hand side of clique inequality, which is initially 1
        // and decreased by 1 for every occurence of a negated variable.
        let mut rhs = 1.0;
        if !b0 {
            rhs -= 1.0;
        }
        if !b1 {
            rhs -= 1.0;
        }

        // Basic clique inequality is
        //
        //   (a1 * x1 + a2 * x2 <= rhs),
        //
        // where x1 and x2 are the clique variables and a1, a2 are 1.0 for
        // positive, -1.0 for negated variables.
        let s0 = api::get_sol_val(scip, sol, v0);
        let s1 = api::get_sol_val(scip, sol, v1);
        add_cut(scip, sepa, if b0 { 1.0 } else { -1.0 }, v0, s0, if b1 { 1.0 } else { -1.0 }, v1, s1, rhs, cutoff, ncuts)?;

        // Terminate if cutoff was found.
        if *cutoff {
            return Ok(());
        }
    }

    Ok(())
}

/*
 * Callback methods of separator
 */

/// `sepaCopyImpliedbounds`: copy method for separator plugins.
fn sepa_copy_impliedbounds(scip: &mut Scip, _sourcescip: &Scip, _sepa: SepaId) -> ScipResult {
    // Call inclusion method of constraint handler.
    include_sepa_impliedbounds(scip)
}

/// `sepaExeclpImpliedbounds`: LP solution separation method of separator.
fn sepa_execlp_impliedbounds(scip: &mut Scip, sepa: SepaId, result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Gets active problem variables.
    let vars = api::get_vars(scip).to_vec();
    let nbinvars = api::get_n_bin_vars(scip);
    if nbinvars == 0 {
        return Ok(());
    }

    // Get fractional problem variables.
    let (fracvars, fracvals, _, nfracs, _, _) = api::get_lp_branch_cands(scip)?;
    if nfracs == 0 {
        return Ok(());
    }
    let nfracs = nfracs as usize;

    // Get solution values for all variables.
    let mut solvals = vec![0.0; vars.len()];
    api::get_var_sols(scip, &vars, &mut solvals);

    // Call the cut separation.
    let mut cutoff = false;
    let mut ncuts = 0;
    separate_cuts(scip, sepa, NONE, &solvals, &fracvars[..nfracs], &fracvals[..nfracs], &mut cutoff, &mut ncuts)?;

    // Adjust result code.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if ncuts > 0 {
        ScipResultCode::Separated
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/// `sepaExecsolImpliedbounds`: arbitrary primal solution separation method
/// of separator.
fn sepa_execsol_impliedbounds(scip: &mut Scip, sepa: SepaId, sol: SolId, result: &mut ScipResultCode, _allowlocal: bool, _depth: i32) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Gets active problem variables.
    let vars = api::get_vars(scip).to_vec();
    let nbinvars = api::get_n_bin_vars(scip) as usize;
    if nbinvars == 0 {
        return Ok(());
    }

    // Get solution values for all variables.
    let mut solvals = vec![0.0; vars.len()];
    api::get_sol_vals(scip, sol, &vars, &mut solvals);

    // Get binary problem variables that are fractional in given solution.
    let mut fracvars = Vec::with_capacity(nbinvars);
    let mut fracvals = Vec::with_capacity(nbinvars);
    for i in 0..nbinvars {
        if !scip.set.is_feas_integral(solvals[i]) {
            fracvars.push(vars[i]);
            fracvals.push(solvals[i]);
        }
    }

    // Call the cut separation.
    let mut ncuts = 0;
    let mut cutoff = false;
    if !fracvars.is_empty() {
        separate_cuts(scip, sepa, sol, &solvals, &fracvars, &fracvals, &mut cutoff, &mut ncuts)?;
    }

    // Adjust result code.
    *result = if cutoff {
        ScipResultCode::Cutoff
    } else if ncuts > 0 {
        ScipResultCode::Separated
    } else {
        ScipResultCode::DidNotFind
    };

    Ok(())
}

/*
 * separator specific interface methods
 */

/// `SCIPincludeSepaImpliedbounds`: creates the impliedbounds separator and
/// includes it in SCIP.
pub fn include_sepa_impliedbounds(scip: &mut Scip) -> ScipResult {
    // Create impliedbounds separator data (uninitialised in C until the
    // parameter sets it).
    let sepadata = SepaData {
        usetwosizecliques: DEFAULT_USETWOSIZECLIQUES,
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
        Some(sepa_execlp_impliedbounds),
        Some(sepa_execsol_impliedbounds),
        Some(Box::new(sepadata) as Box<dyn Any>),
    )?;

    // Set non-NULL pointers to callback methods.
    scip.sepas[sepa].sepacopy = Some(sepa_copy_impliedbounds);

    // Add separator parameters.
    crate::paramset::paramset_add_bool(
        scip,
        "separating/impliedbounds/usetwosizecliques",
        "should violated inequalities for cliques with 2 variables be separated?",
        crate::data_target!(bool, sepas, sepa, sepadata, SepaData, usetwosizecliques),
        true,
        DEFAULT_USETWOSIZECLIQUES,
        None,
    )?;

    Ok(())
}
