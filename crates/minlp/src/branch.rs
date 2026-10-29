//! SCIP's branching candidates and rules (`branch.c`, `struct_branch.h`).

use std::any::Any;

use crate::{
    clock::Clock,
    cvec::CVec,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{BranchruleId, ImplintType, ScipResultCode, VarId, VarStatus, VarType},
    var::NONE,
};

/// `SCIP_BRANCHCAND`.
#[derive(Clone, Debug, Default)]
pub struct BranchCand {
    pub lpcands: CVec<VarId>,
    pub lpcandssol: CVec<f64>,
    pub lpcandsfrac: CVec<f64>,
    pub externcands: CVec<VarId>,
    pub externcandsscore: CVec<f64>,
    pub externcandssol: CVec<f64>,
    pub pseudocands: CVec<VarId>,
    pub validlpcandslp: i64,
    pub lpcandssize: i32,
    pub nlpcands: i32,
    pub npriolpcands: i32,
    pub npriolpbins: i32,
    pub nimpllpfracs: i32,
    pub nenfoimpllpfracs: i32,
    pub lpmaxpriority: i32,
    pub externcandssize: i32,
    pub nexterncands: i32,
    pub nprioexterncands: i32,
    pub nprioexternbins: i32,
    pub nprioexternints: i32,
    pub nprioexternimpls: i32,
    pub externmaxpriority: i32,
    pub pseudocandssize: i32,
    pub npseudocands: i32,
    pub npriopseudocands: i32,
    pub npriopseudobins: i32,
    pub npriopseudoints: i32,
    pub pseudomaxpriority: i32,
}

pub type BranchMethod = fn(&mut Scip, BranchruleId) -> ScipResult;
/// A copy method: `(target scip, source scip, source plugin)`.
pub type BranchCopy = fn(&mut Scip, &Scip, BranchruleId) -> ScipResult;
/// `SCIP_DECL_BRANCHEXECLP` (and `EXECEXT`, `EXECPS`): `(allowaddcons, result)`.
pub type BranchExec = fn(&mut Scip, BranchruleId, bool, &mut ScipResultCode) -> ScipResult;

/// `SCIP_BRANCHRULE`.
pub struct Branchrule {
    pub maxbounddist: f64,
    pub nlpcalls: i64,
    pub nexterncalls: i64,
    pub npseudocalls: i64,
    pub ncutoffs: i64,
    pub ncutsfound: i64,
    pub nconssfound: i64,
    pub ndomredsfound: i64,
    pub nchildren: i64,
    pub name: String,
    pub desc: String,
    pub branchcopy: Option<BranchCopy>,
    pub branchfree: Option<BranchMethod>,
    pub branchinit: Option<BranchMethod>,
    pub branchexit: Option<BranchMethod>,
    pub branchinitsol: Option<BranchMethod>,
    pub branchexitsol: Option<BranchMethod>,
    pub branchexeclp: Option<BranchExec>,
    pub branchexecext: Option<BranchExec>,
    pub branchexecps: Option<BranchExec>,
    pub branchruledata: Option<Box<dyn Any>>,
    pub setuptime: Clock,
    pub branchclock: Clock,
    pub priority: i32,
    pub maxdepth: i32,
    pub initialized: bool,
    pub isobjbranchrule: bool,
    pub exact: bool,
}

/*
 * memory growing methods for dynamically allocated arrays
 */

/// `ensureLpcandsSize`: ensures, that lpcands array can store at least
/// `num` entries.
fn ensure_lpcands_size(scip: &mut Scip, num: i32) {
    if num > scip.branchcand.lpcandssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let b = &mut scip.branchcand;
        b.lpcands.0.resize(newsize as usize, NONE);
        b.lpcandssol.realloc(newsize);
        b.lpcandsfrac.realloc(newsize);
        b.lpcandssize = newsize;
    }
}

/// `ensurePseudocandsSize`: ensures, that pseudocands array can store at
/// least `num` entries.
fn ensure_pseudocands_size(scip: &mut Scip, num: i32) {
    if num > scip.branchcand.pseudocandssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let b = &mut scip.branchcand;
        b.pseudocands.0.resize(newsize as usize, NONE);
        b.pseudocandssize = newsize;
    }
}

/// `ensureExterncandsSize`: ensures, that externcands array can store at
/// least `num` entries.
fn ensure_externcands_size(scip: &mut Scip, num: i32) {
    if num > scip.branchcand.externcandssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        let b = &mut scip.branchcand;
        b.externcands.0.resize(newsize as usize, NONE);
        b.externcandsscore.realloc(newsize);
        b.externcandssol.realloc(newsize);
        b.externcandssize = newsize;
    }
}

/*
 * branching candidate storage methods
 */

/// `SCIPbranchcandCreate`: creates a branching candidate storage.
pub fn branchcand_create() -> BranchCand {
    let mut b = BranchCand {
        lpmaxpriority: i32::MIN,
        externmaxpriority: i32::MIN,
        pseudomaxpriority: i32::MIN,
        ..Default::default()
    };
    branchcand_invalidate(&mut b);
    b
}

/// `SCIPbranchcandInvalidate`: resets branching candidates storage.
pub fn branchcand_invalidate(branchcand: &mut BranchCand) {
    branchcand.validlpcandslp = -1;
}

/// `branchcandCalcLPCands`: calculates branching candidates for LP
/// solution branching (fractional variables).
fn branchcand_calc_lp_cands(scip: &mut Scip) {
    // Check, if the current LP branching candidate array is invalid.
    if scip.branchcand.validlpcandslp < scip.stat.lpcount {
        let ncols = crate::lp::lp_get_n_cols(scip);

        // Construct the LP branching candidate set, moving the candidates
        // with maximal priority to the front.
        ensure_lpcands_size(scip, ncols);

        {
            let b = &mut scip.branchcand;
            b.lpmaxpriority = i32::MIN / 2;
            b.nlpcands = 0;
            b.nimpllpfracs = 0;
            b.npriolpcands = 0;
            b.npriolpbins = 0;
            b.nenfoimpllpfracs = 0;
        }
        for c in 0..ncols {
            let col = scip.lp.as_ref().unwrap().cols[c];
            let primsol = crate::lp::col_get_primsol(scip, col);

            // Count values at infinity as integral.
            if scip.set.is_infinity(primsol.abs()) {
                continue;
            }

            let var = scip.cols[col].var;
            let v = &scip.vars[var];

            // LP branching candidates are fractional binary and integer
            // variables; implicit variables are kept at the end of the
            // candidates array for some rounding heuristics.
            let vartype = v.vartype;
            let impltype = v.varimpltype;
            if vartype == VarType::Continuous && impltype == ImplintType::None {
                continue;
            }

            // Ignore fixed variables (due to numerics, it is possible, that
            // the LP solution of a fixed integer variable (with large fixed
            // value) is fractional in terms of absolute feasibility measure).
            if v.locdom.lb >= v.locdom.ub - 0.5 {
                continue;
            }

            // Check, if the LP solution value is fractional.
            let frac = scip.set.feas_frac(primsol);

            // The fractionality should not be smaller than -feastol, however,
            // if the primsol is large enough and close to an integer, fixed
            // precision floating point arithmetic might give us values
            // slightly smaller than -feastol. Originally, the "frac >=
            // -feastol"-check was within SCIPsetIsFeasFracIntegral(), however,
            // we relaxed it to "frac >= -2*feastol" and have the stricter
            // check here for small-enough primsols.
            if scip.set.is_feas_frac_integral(frac) {
                continue;
            }

            // Insert candidate in candidate list.
            let mut branchpriority = v.branchpriority;
            let b = &mut scip.branchcand;
            let mut insertpos = b.nlpcands + b.nimpllpfracs;

            if impltype != ImplintType::None {
                branchpriority = i32::MIN;
            }

            macro_rules! mv {
                ($to:expr, $from:expr) => {{
                    let (to, from) = ($to, $from);
                    b.lpcands[to] = b.lpcands[from];
                    b.lpcandssol[to] = b.lpcandssol[from];
                    b.lpcandsfrac[to] = b.lpcandsfrac[from];
                }};
            }

            // Ensure that unenforced implied integral variables are stored at
            // the end of the array.
            if b.nimpllpfracs > b.nenfoimpllpfracs && vartype != VarType::Continuous {
                mv!(insertpos, b.nlpcands + b.nenfoimpllpfracs);
                insertpos = b.nlpcands + b.nenfoimpllpfracs;
            }

            // Ensure that enforced implied integral variables are stored
            // before the end of the array.
            if b.nenfoimpllpfracs > 0 && impltype == ImplintType::None {
                mv!(insertpos, b.nlpcands);
                insertpos = b.nlpcands;
            }

            if branchpriority > b.lpmaxpriority {
                // Candidate has higher priority than the current maximum:
                // move it to the front and declare it to be the single best
                // candidate.
                if insertpos != 0 {
                    mv!(insertpos, 0);
                    insertpos = 0;
                }
                b.npriolpcands = 1;
                b.npriolpbins = if vartype == VarType::Binary { 1 } else { 0 };
                b.lpmaxpriority = branchpriority;
            } else if branchpriority == b.lpmaxpriority {
                // Candidate has equal priority as the current maximum: move
                // away the first non-maximal priority candidate, move the
                // current candidate to the correct slot (binaries first) and
                // increase the number of maximal priority candidates.
                if insertpos != b.npriolpcands {
                    mv!(insertpos, b.npriolpcands);
                    insertpos = b.npriolpcands;
                }
                b.npriolpcands += 1;
                if vartype == VarType::Binary {
                    if insertpos != b.npriolpbins {
                        mv!(insertpos, b.npriolpbins);
                        insertpos = b.npriolpbins;
                    }
                    b.npriolpbins += 1;
                }
            }

            // Insert variable at the correct position of the candidates
            // storage.
            b.lpcands[insertpos] = var;
            b.lpcandssol[insertpos] = primsol;
            b.lpcandsfrac[insertpos] = frac;

            // Increase the counter depending on the variable type.
            if impltype == ImplintType::None {
                b.nlpcands += 1;
            } else {
                b.nimpllpfracs += 1;
                if vartype != VarType::Continuous {
                    b.nenfoimpllpfracs += 1;
                }
            }
        }

        scip.branchcand.validlpcandslp = scip.stat.lpcount;
    }
}

/// The counts `SCIPbranchcandGetLPCands` reports; the candidates, their
/// solution values and fractionalities are the first `nlpcands +
/// nfracimplvars` entries of `BranchCand::lpcands`, `lpcandssol` and
/// `lpcandsfrac`.
#[derive(Clone, Copy, Debug)]
pub struct LpCandCounts {
    pub nlpcands: i32,
    pub npriolpcands: i32,
    pub nfracimplvars: i32,
}

/// `SCIPbranchcandGetLPCands`: gets branching candidates for LP solution
/// branching (fractional variables).
pub fn branchcand_get_lp_cands(scip: &mut Scip) -> LpCandCounts {
    // Calculate branching candidates.
    branchcand_calc_lp_cands(scip);

    // Assign return values.
    let b = &scip.branchcand;
    LpCandCounts {
        nlpcands: b.nlpcands + b.nenfoimpllpfracs,
        npriolpcands: if scip.set.branch_preferbinary && b.npriolpbins > 0 {
            b.npriolpbins
        } else {
            b.npriolpcands
        },
        nfracimplvars: b.nimpllpfracs - b.nenfoimpllpfracs,
    }
}

/// `SCIPbranchcandAddExternCand`: insert variable, its score and its
/// solution value into the external branching candidate storage; the
/// absolute difference of the current lower and upper bounds of the
/// variable must be at least epsilon.
pub fn branchcand_add_extern_cand(scip: &mut Scip, var: VarId, score: f64, solval: f64) {
    let v = &scip.vars[var];
    let (vartype, impltype, branchpriority) = (v.vartype, v.varimpltype, v.branchpriority);

    let n = scip.branchcand.nexterncands;
    ensure_externcands_size(scip, n + 1);
    let b = &mut scip.branchcand;
    let mut insertpos = b.nexterncands;

    macro_rules! mv {
        ($to:expr, $from:expr) => {{
            let (to, from) = ($to, $from);
            b.externcands[to] = b.externcands[from];
            b.externcandsscore[to] = b.externcandsscore[from];
            b.externcandssol[to] = b.externcandssol[from];
        }};
    }

    // Insert the variable into externcands, making sure, that the highest
    // priority candidates are at the front and ordered binaries, integers,
    // implicit integers, continuous.
    if branchpriority > b.externmaxpriority {
        // Candidate has higher priority than the current maximum: move it to
        // the front and declare it to be the single best candidate.
        mv!(insertpos, 0);
        insertpos = 0;

        b.nprioexterncands = 1;
        b.nprioexternbins = 0;
        b.nprioexternints = 0;
        b.nprioexternimpls = 0;
        if impltype != ImplintType::None {
            b.nprioexternimpls = 1;
        } else if vartype == VarType::Binary {
            b.nprioexternbins = 1;
        } else if vartype == VarType::Integer {
            b.nprioexternints = 1;
        }
        b.externmaxpriority = branchpriority;
    } else if branchpriority == b.externmaxpriority {
        // Candidate has equal priority as the current maximum: move away the
        // first non-maximal priority candidate, move the current candidate to
        // the correct slot (binaries first, integers next, implicit integers
        // next, continuous last) and increase the number of maximal priority
        // candidates.
        if insertpos != b.nprioexterncands {
            mv!(insertpos, b.nprioexterncands);
            insertpos = b.nprioexterncands;
        }
        b.nprioexterncands += 1;
        if vartype != VarType::Continuous || impltype != ImplintType::None {
            let k = b.nprioexternbins + b.nprioexternints + b.nprioexternimpls;
            if insertpos != k {
                mv!(insertpos, k);
                insertpos = k;
            }
            b.nprioexternimpls += 1;
            if impltype == ImplintType::None {
                let k = b.nprioexternbins + b.nprioexternints;
                if insertpos != k {
                    mv!(insertpos, k);
                    insertpos = k;
                }
                b.nprioexternints += 1;
                b.nprioexternimpls -= 1;
                if vartype == VarType::Binary {
                    if insertpos != b.nprioexternbins {
                        mv!(insertpos, b.nprioexternbins);
                        insertpos = b.nprioexternbins;
                    }
                    b.nprioexternbins += 1;
                    b.nprioexternints -= 1;
                }
            }
        }
    }
    b.externcands[insertpos] = var;
    b.externcandsscore[insertpos] = score;
    b.externcandssol[insertpos] = solval;
    b.nexterncands += 1;
}

/// `SCIPbranchcandClearExternCands`: removes all external candidates from
/// the storage for external branching.
pub fn branchcand_clear_extern_cands(scip: &mut Scip) {
    let b = &mut scip.branchcand;
    b.nexterncands = 0;
    b.nprioexterncands = 0;
    b.nprioexternbins = 0;
    b.nprioexternints = 0;
    b.nprioexternimpls = 0;
    b.externmaxpriority = i32::MIN;
}

/// `SCIPbranchcandContainsExternCand`: checks whether the given variable
/// is contained in the candidate storage for external branching.
pub fn branchcand_contains_extern_cand(scip: &Scip, var: VarId) -> bool {
    let b = &scip.branchcand;
    let branchpriority = scip.vars[var].branchpriority;

    // Look for the variable in the externcands, using the fact, that the
    // highest priority candidates are at the front and ordered binaries,
    // integers, implicit integers, continuous.
    if branchpriority > b.externmaxpriority {
        // The branching priority of the variable is higher than the maximal
        // priority contained in the array; the variable can thus not be
        // contained.
        return false;
    }
    if branchpriority == b.externmaxpriority {
        let vartype = scip.vars[var].vartype;

        // Variable has equal priority as the current maximum: look for it in
        // the correct slot (binaries first, integers next, implicit integers
        // next, continuous last).
        if crate::var::var_is_implied_integral(scip, var) {
            // The variable is implicit integer, look at the slots containing
            // implicit integers.
            return (0..b.nprioexternimpls).any(|i| b.externcands[b.nprioexternbins + b.nprioexternints + i] == var);
        }
        if vartype == VarType::Binary {
            // The variable is binary, look at the first nprioexternbins slots.
            return (0..b.nprioexternbins).any(|i| b.externcands[i] == var);
        }
        if vartype == VarType::Integer {
            // The variable is integer, look at the slots containing integers.
            return (0..b.nprioexternints).any(|i| b.externcands[b.nprioexternbins + i] == var);
        }
        // The variable is continuous, look at the slots containing continuous
        // variables.
        return (b.nprioexternbins + b.nprioexternints + b.nprioexternimpls..b.nprioexterncands).any(|i| b.externcands[i] == var);
    }

    // The branching priority of the variable is lower than the maximal
    // priority contained in the array; look for the variable in the
    // candidates which do not have maximal priority.
    (b.nprioexterncands..b.nexterncands).any(|i| b.externcands[i] == var)
}

/// `SCIPbranchcandGetPseudoCands`: gets branching candidates for pseudo
/// solution branching (non-fixed variables); returns `(npseudocands,
/// npriopseudocands)`, the candidates being the first entries of
/// `BranchCand::pseudocands`.
pub fn branchcand_get_pseudo_cands(scip: &Scip) -> (i32, i32) {
    let b = &scip.branchcand;
    let nprio = if scip.set.branch_preferbinary && b.npriopseudobins > 0 {
        b.npriopseudobins
    } else {
        b.npriopseudocands
    };
    (b.npseudocands, nprio)
}

/// `SCIPbranchcandGetNPseudoCands`: gets number of branching candidates
/// for pseudo solution branching (non-fixed variables).
pub fn branchcand_get_n_pseudo_cands(scip: &Scip) -> i32 {
    scip.branchcand.npseudocands
}

/// `branchcandInsertPseudoCand`: insert pseudocand at given position, or
/// to the first positions of the maximal priority candidates, using the
/// given position as free slot for the other candidates.
fn branchcand_insert_pseudo_cand(scip: &mut Scip, var: VarId, mut insertpos: i32) {
    let v = &scip.vars[var];
    let (vartype, impltype, branchpriority) = (v.vartype, v.varimpltype, v.branchpriority);
    let b = &mut scip.branchcand;

    // Moves the candidate at `from` to `to` and records its new index.
    let mv = |b: &mut BranchCand, vars: &mut crate::arena::Arena<crate::var::Var>, to: i32, from: i32| {
        b.pseudocands[to] = b.pseudocands[from];
        vars[b.pseudocands[to]].pseudocandindex = to;
    };
    let vars = &mut scip.vars;

    // Insert the variable into pseudocands, making sure, that the highest
    // priority candidates are at the front and ordered binaries, integers,
    // implicit integers.
    if branchpriority > b.pseudomaxpriority {
        // Candidate has higher priority than the current maximum: move it to
        // the front and declare it to be the single best candidate.
        if insertpos != 0 {
            mv(b, vars, insertpos, 0);
            insertpos = 0;
        }
        b.npriopseudocands = 1;
        if impltype != ImplintType::None {
            b.npriopseudobins = 0;
            b.npriopseudoints = 0;
        } else {
            b.npriopseudobins = if vartype == VarType::Binary { 1 } else { 0 };
            b.npriopseudoints = if vartype == VarType::Integer { 1 } else { 0 };
        }
        b.pseudomaxpriority = branchpriority;
    } else if branchpriority == b.pseudomaxpriority {
        // Candidate has equal priority as the current maximum: move away the
        // first non-maximal priority candidate, move the current candidate to
        // the correct slot (binaries first, integers next, implicit integers
        // last) and increase the number of maximal priority candidates.
        if insertpos != b.npriopseudocands {
            let k = b.npriopseudocands;
            mv(b, vars, insertpos, k);
            insertpos = k;
        }
        b.npriopseudocands += 1;
        if vartype != VarType::Continuous && impltype == ImplintType::None {
            let k = b.npriopseudobins + b.npriopseudoints;
            if insertpos != k {
                mv(b, vars, insertpos, k);
                insertpos = k;
            }
            b.npriopseudoints += 1;
            if vartype == VarType::Binary {
                let k = b.npriopseudobins;
                if insertpos != k {
                    mv(b, vars, insertpos, k);
                    insertpos = k;
                }
                b.npriopseudobins += 1;
                b.npriopseudoints -= 1;
            }
        }
    }
    b.pseudocands[insertpos] = var;
    vars[var].pseudocandindex = insertpos;
}

/// `branchcandSortPseudoCands`: sorts the pseudo branching candidates,
/// such that the candidates of maximal priority are at the front, ordered
/// by binaries, integers, implicit integers.
fn branchcand_sort_pseudo_cands(scip: &mut Scip) {
    scip.branchcand.pseudomaxpriority = i32::MIN;

    for i in 0..scip.branchcand.npseudocands {
        let var = scip.branchcand.pseudocands[i];
        if scip.vars[var].branchpriority >= scip.branchcand.pseudomaxpriority {
            branchcand_insert_pseudo_cand(scip, var, i);
        }
    }
}

/// `branchcandRemovePseudoCand`: removes pseudo candidate from pseudocands
/// array.
fn branchcand_remove_pseudo_cand(scip: &mut Scip, var: VarId) {
    // Note that the branching priority of the variable to be removed is not
    // necessarily equal to pseudomaxpriority, since the status of the
    // variable might have changed, leading to a change in the branching
    // priority. Moreover, if the variable was part of an aggregation, even
    // other variables might at this point have different priorities.

    // Delete the variable from pseudocands, making sure, that the highest
    // priority candidates are at the front and ordered binaries, integers,
    // implicit integers.
    let mut freepos = scip.vars[var].pseudocandindex;
    scip.vars[var].pseudocandindex = -1;

    let b = &mut scip.branchcand;
    let vars = &mut scip.vars;
    let mv = |b: &mut BranchCand, vars: &mut crate::arena::Arena<crate::var::Var>, to: i32, from: i32| {
        b.pseudocands[to] = b.pseudocands[from];
        vars[b.pseudocands[to]].pseudocandindex = to;
    };

    if freepos < b.npriopseudobins {
        // A binary candidate of maximal priority was removed.
        if freepos != b.npriopseudobins - 1 {
            let k = b.npriopseudobins - 1;
            mv(b, vars, freepos, k);
            freepos = k;
        }
        b.npriopseudobins -= 1;
        b.npriopseudoints += 1;
    }
    if freepos < b.npriopseudobins + b.npriopseudoints {
        // A binary or integer candidate of maximal priority was removed.
        let k = b.npriopseudobins + b.npriopseudoints - 1;
        if freepos != k {
            mv(b, vars, freepos, k);
            freepos = k;
        }
        b.npriopseudoints -= 1;
    }
    if freepos < b.npriopseudocands {
        // A candidate of maximal priority was removed.
        let k = b.npriopseudocands - 1;
        if freepos != k {
            mv(b, vars, freepos, k);
            freepos = k;
        }
        b.npriopseudocands -= 1;
    }
    if freepos != b.npseudocands - 1 {
        let k = b.npseudocands - 1;
        mv(b, vars, freepos, k);
    }
    b.npseudocands -= 1;

    // If all maximal priority candidates were removed, resort the array s.t.
    // the new maximal priority candidates are at the front.
    if scip.branchcand.npriopseudocands == 0 {
        branchcand_sort_pseudo_cands(scip);
    }
}

/// `SCIPbranchcandRemoveVar`: removes variable from branching candidate
/// list.
pub fn branchcand_remove_var(scip: &mut Scip, var: VarId) -> ScipResult {
    // Make sure, variable is not member of the pseudo branching candidate
    // list.
    if scip.vars[var].pseudocandindex >= 0 {
        branchcand_remove_pseudo_cand(scip, var);
    }
    Ok(())
}

/// `SCIPbranchcandUpdateVar`: updates branching candidate list for a given
/// variable.
pub fn branchcand_update_var(scip: &mut Scip, var: VarId) -> ScipResult {
    let v = &scip.vars[var];
    if matches!(v.varstatus, VarStatus::Loose | VarStatus::Column) && crate::var::var_is_integral(scip, var) && scip.set.is_lt(v.locdom.lb, v.locdom.ub) {
        // Variable is neither continuous nor fixed and has non-empty domain:
        // make sure it is member of the pseudo branching candidate list.
        if v.pseudocandindex == -1 {
            let n = scip.branchcand.npseudocands;
            ensure_pseudocands_size(scip, n + 1);

            scip.branchcand.npseudocands += 1;
            branchcand_insert_pseudo_cand(scip, var, n);
        }
    } else {
        // Variable is continuous or fixed or has empty domain: make sure it
        // is not member of the pseudo branching candidate list.
        branchcand_remove_var(scip, var)?;
    }
    Ok(())
}

/// `SCIPbranchcandUpdateVarBranchPriority`: updates branching priority of
/// the given variable and update the pseudo candidate array if needed.
pub fn branchcand_update_var_branch_priority(scip: &mut Scip, var: VarId, branchpriority: i32) -> ScipResult {
    let oldbranchpriority = scip.vars[var].branchpriority;

    if oldbranchpriority == branchpriority {
        return Ok(());
    }

    let pseudomaxpriority = scip.branchcand.pseudomaxpriority;

    // If the variable currently belongs to the priority set or the new
    // branching priority is larger than the current one, remove it from the
    // pseudo branch candidate array.
    if oldbranchpriority == pseudomaxpriority || branchpriority > pseudomaxpriority {
        branchcand_remove_var(scip, var)?;
    }

    // Change the branching priority of the variable.
    crate::var::var_chg_branch_priority(scip, var, branchpriority)?;

    // If the variable is not part of the pseudo branching candidate array,
    // check if it is a pseudo branching candidate and add it if so.
    branchcand_update_var(scip, var)
}

/*
 * branching rule methods
 */

/// `SCIPbranchruleComp`: compares two branching rules w. r. to their
/// priority.
pub fn branchrule_comp(scip: &Scip, elem1: BranchruleId, elem2: BranchruleId) -> i32 {
    scip.branchrules[elem2].priority - scip.branchrules[elem1].priority
}

/// `paramChgdBranchrulePriority`: the parameter already wrote the
/// priority; mark the branching rules unsorted
/// (`SCIPsetBranchrulePriority`).
fn param_chgd_branchrule_priority(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    scip.set.branchrulessorted = false;
    Ok(())
}

/// `SCIPbranchruleCopyInclude`: copies the given branchrule to a new scip.
pub fn branchrule_copy_include(scip: &mut Scip, sourcescip: &Scip, branchrule: BranchruleId) -> ScipResult {
    if let Some(copy) = sourcescip.branchrules[branchrule].branchcopy {
        copy(scip, sourcescip, branchrule)?;
    }
    Ok(())
}

/// The callbacks `SCIPbranchruleCreate` takes.
#[derive(Default)]
pub struct BranchruleMethods {
    pub branchcopy: Option<BranchCopy>,
    pub branchfree: Option<BranchMethod>,
    pub branchinit: Option<BranchMethod>,
    pub branchexit: Option<BranchMethod>,
    pub branchinitsol: Option<BranchMethod>,
    pub branchexitsol: Option<BranchMethod>,
    pub branchexeclp: Option<BranchExec>,
    pub branchexecext: Option<BranchExec>,
    pub branchexecps: Option<BranchExec>,
}

/// `SCIPbranchruleCreate`: creates a branching rule. `maxbounddist` is the
/// maximal relative distance from current node's dual bound to primal
/// bound compared to best node's dual bound for applying branching rule
/// (0.0: only on current best node, 1.0: on all nodes).
#[allow(clippy::too_many_arguments)]
pub fn branchrule_create(
    scip: &mut Scip,
    name: &str,
    desc: &str,
    priority: i32,
    maxdepth: i32,
    maxbounddist: f64,
    methods: BranchruleMethods,
    branchruledata: Option<Box<dyn Any>>,
) -> ScipResult<BranchruleId> {
    use crate::paramset::{paramset_add_int, paramset_add_real};

    let branchrule = scip.branchrules.push(Branchrule {
        name: name.to_string(),
        desc: desc.to_string(),
        priority,
        maxdepth,
        maxbounddist,
        branchcopy: methods.branchcopy,
        branchfree: methods.branchfree,
        branchinit: methods.branchinit,
        branchexit: methods.branchexit,
        branchinitsol: methods.branchinitsol,
        branchexitsol: methods.branchexitsol,
        branchexeclp: methods.branchexeclp,
        branchexecext: methods.branchexecext,
        branchexecps: methods.branchexecps,
        branchruledata,
        setuptime: Clock::new(),
        branchclock: Clock::new(),
        nlpcalls: 0,
        nexterncalls: 0,
        npseudocalls: 0,
        ncutoffs: 0,
        ncutsfound: 0,
        nconssfound: 0,
        ndomredsfound: 0,
        nchildren: 0,
        initialized: false,
        isobjbranchrule: false,
        exact: false,
    });

    // Add parameters.
    paramset_add_int(
        scip,
        &format!("branching/{name}/priority"),
        &format!("priority of branching rule <{name}>"),
        crate::field_target!(i32, branchrules, branchrule, priority),
        false,
        priority,
        i32::MIN / 4,
        i32::MAX / 4,
        Some(param_chgd_branchrule_priority),
    )?;
    paramset_add_int(
        scip,
        &format!("branching/{name}/maxdepth"),
        &format!("maximal depth level, up to which branching rule <{name}> should be used (-1 for no limit)"),
        crate::field_target!(i32, branchrules, branchrule, maxdepth),
        false,
        maxdepth,
        -1,
        crate::def::MAXTREEDEPTH,
        None,
    )?;
    paramset_add_real(
        scip,
        &format!("branching/{name}/maxbounddist"),
        "maximal relative distance from current node's dual bound to primal bound compared to best node's dual bound for applying branching rule (0.0: only on current best node, 1.0: on all nodes)",
        crate::field_target!(f64, branchrules, branchrule, maxbounddist),
        false,
        maxbounddist,
        0.0,
        1.0,
        None,
    )?;
    Ok(branchrule)
}

/// `SCIPbranchruleFree`: frees memory of branching rule.
pub fn branchrule_free(scip: &mut Scip, branchrule: BranchruleId) -> ScipResult {
    // Call destructor of branching rule.
    if let Some(free) = scip.branchrules[branchrule].branchfree {
        free(scip, branchrule)?;
    }
    scip.branchrules.remove(branchrule);
    Ok(())
}

/// Runs one of the branching rule's setup callbacks under its setup clock.
fn branchrule_setup_call(scip: &mut Scip, branchrule: BranchruleId, method: Option<BranchMethod>) -> ScipResult {
    if let Some(method) = method {
        let enabled = scip.set.time_enabled;
        // Start timing.
        scip.branchrules[branchrule].setuptime.start(enabled);
        method(scip, branchrule)?;
        // Stop timing.
        scip.branchrules[branchrule].setuptime.stop(enabled);
    }
    Ok(())
}

/// `SCIPbranchruleInit`: initializes branching rule.
pub fn branchrule_init(scip: &mut Scip, branchrule: BranchruleId) -> ScipResult {
    if scip.branchrules[branchrule].initialized {
        log::error!("branching rule <{}> already initialized", scip.branchrules[branchrule].name);
        return Err(Retcode::InvalidCall);
    }

    if scip.set.misc_resetstat {
        let b = &mut scip.branchrules[branchrule];
        b.setuptime.reset();
        b.branchclock.reset();
        b.nlpcalls = 0;
        b.nexterncalls = 0;
        b.npseudocalls = 0;
        b.ncutoffs = 0;
        b.ncutsfound = 0;
        b.nconssfound = 0;
        b.ndomredsfound = 0;
        b.nchildren = 0;
    }

    let init = scip.branchrules[branchrule].branchinit;
    branchrule_setup_call(scip, branchrule, init)?;
    scip.branchrules[branchrule].initialized = true;
    Ok(())
}

/// `SCIPbranchruleExit`: deinitializes branching rule.
pub fn branchrule_exit(scip: &mut Scip, branchrule: BranchruleId) -> ScipResult {
    if !scip.branchrules[branchrule].initialized {
        log::error!("branching rule <{}> not initialized", scip.branchrules[branchrule].name);
        return Err(Retcode::InvalidCall);
    }

    let exit = scip.branchrules[branchrule].branchexit;
    branchrule_setup_call(scip, branchrule, exit)?;
    scip.branchrules[branchrule].initialized = false;
    Ok(())
}

/// `SCIPbranchruleInitsol`: informs branching rule that the branch and
/// bound process is being started.
pub fn branchrule_initsol(scip: &mut Scip, branchrule: BranchruleId) -> ScipResult {
    let initsol = scip.branchrules[branchrule].branchinitsol;
    branchrule_setup_call(scip, branchrule, initsol)
}

/// `SCIPbranchruleExitsol`: informs branching rule that the branch and
/// bound process data is being freed.
pub fn branchrule_exitsol(scip: &mut Scip, branchrule: BranchruleId) -> ScipResult {
    let exitsol = scip.branchrules[branchrule].branchexitsol;
    branchrule_setup_call(scip, branchrule, exitsol)
}

/// Which solution a branching rule execution branches on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ExecKind {
    Lp,
    Extern,
    Pseudo,
}

/// The shared body of `SCIPbranchruleExecLPSol`,
/// `SCIPbranchruleExecExternSol` and `SCIPbranchruleExecPseudoSol`, with
/// their small differences in what they accept and count.
fn branchrule_exec(scip: &mut Scip, branchrule: BranchruleId, kind: ExecKind, sepastore: usize, cutoffbound: f64, allowaddcons: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result = R::DidNotRun;

    let br = &scip.branchrules[branchrule];
    let exec = match kind {
        ExecKind::Lp => br.branchexeclp,
        ExecKind::Extern => br.branchexecext,
        ExecKind::Pseudo => br.branchexecps,
    };
    let Some(exec) = exec else { return Ok(result) };
    if !(br.maxdepth == -1 || br.maxdepth >= crate::tree::tree_get_current_depth(scip)) {
        return Ok(result);
    }

    let focusnode = scip.tree.as_ref().unwrap().focusnode;
    let loclowerbound = scip.nodes[focusnode].lowerbound;
    let glblowerbound = crate::tree::tree_get_lowerbound(scip);
    let maxbounddist = br.maxbounddist;

    // We distinguish between finite and infinite global lower bounds to
    // avoid comparisons between different values > SCIPinfinity().
    let runbranchrule = if scip.set.is_infinity(-glblowerbound) {
        scip.set.is_infinity(-loclowerbound) || scip.set.is_ge(maxbounddist, 1.0)
    } else {
        scip.set.is_le(loclowerbound - glblowerbound, maxbounddist * (cutoffbound - glblowerbound))
    };

    if runbranchrule {
        let stat = &scip.stat;
        let oldndomchgs = stat.nboundchgs + stat.nholechgs;
        let oldnprobdomchgs = stat.nprobboundchgs + stat.nprobholechgs;
        let oldncuts = scip.sepastores[sepastore].ncuts;
        let oldnactiveconss = if kind == ExecKind::Lp { stat.nactiveconssadded } else { stat.nactiveconss as i64 };

        // Start timing.
        let enabled = scip.set.time_enabled;
        scip.branchrules[branchrule].branchclock.start(enabled);

        // Call external method.
        exec(scip, branchrule, allowaddcons, &mut result)?;

        // Stop timing.
        scip.branchrules[branchrule].branchclock.stop(enabled);

        // Evaluate result.
        let valid = match kind {
            ExecKind::Pseudo => matches!(result, R::Cutoff | R::ConsAdded | R::ReducedDom | R::Branched | R::DidNotFind | R::DidNotRun),
            _ => matches!(result, R::Cutoff | R::ConsAdded | R::ReducedDom | R::Separated | R::Branched | R::DidNotFind | R::DidNotRun),
        };
        let what = match kind {
            ExecKind::Lp => "LP",
            ExecKind::Extern => "external",
            ExecKind::Pseudo => "pseudo",
        };
        if !valid {
            log::error!(
                "branching rule <{}> returned invalid result code <{}> from {} solution branching",
                scip.branchrules[branchrule].name,
                result as i32,
                what
            );
            return Err(Retcode::InvalidResult);
        }
        if result == R::ConsAdded && !allowaddcons {
            log::error!(
                "branching rule <{}> added a constraint in {} solution branching without permission",
                scip.branchrules[branchrule].name,
                what
            );
            return Err(Retcode::InvalidResult);
        }

        // Update statistics.
        let stat = &scip.stat;
        let ndomchgs = stat.nboundchgs + stat.nholechgs;
        let nprobdomchgs = stat.nprobboundchgs + stat.nprobholechgs;
        let nactiveconss = if kind == ExecKind::Lp { stat.nactiveconssadded } else { stat.nactiveconss as i64 };
        let ncuts = scip.sepastores[sepastore].ncuts;
        let nchildren = scip.tree.as_ref().unwrap().nchildren;
        let b = &mut scip.branchrules[branchrule];
        if result != R::DidNotRun {
            match kind {
                ExecKind::Lp => b.nlpcalls += 1,
                ExecKind::Extern => b.nexterncalls += 1,
                ExecKind::Pseudo => b.npseudocalls += 1,
            }
        }
        if result == R::Cutoff {
            b.ncutoffs += 1;
        }
        if result != R::Branched {
            // Update domain reductions; therefore remove the domain reduction
            // counts which were generated in probing mode.
            b.ndomredsfound += ndomchgs - oldndomchgs;
            b.ndomredsfound -= nprobdomchgs - oldnprobdomchgs;
            if kind != ExecKind::Pseudo {
                b.ncutsfound += (ncuts - oldncuts) as i64;
            }
            b.nconssfound += nactiveconss - oldnactiveconss;
        } else {
            b.nchildren += nchildren as i64;
        }
    }
    Ok(result)
}

/// `SCIPbranchruleExecLPSol`: executes branching rule for fractional LP
/// solution.
pub fn branchrule_exec_lp_sol(scip: &mut Scip, branchrule: BranchruleId, sepastore: usize, cutoffbound: f64, allowaddcons: bool) -> ScipResult<ScipResultCode> {
    branchrule_exec(scip, branchrule, ExecKind::Lp, sepastore, cutoffbound, allowaddcons)
}

/// `SCIPbranchruleExecExternSol`: executes branching rule for external
/// branching candidates.
pub fn branchrule_exec_extern_sol(scip: &mut Scip, branchrule: BranchruleId, sepastore: usize, cutoffbound: f64, allowaddcons: bool) -> ScipResult<ScipResultCode> {
    branchrule_exec(scip, branchrule, ExecKind::Extern, sepastore, cutoffbound, allowaddcons)
}

/// `SCIPbranchruleExecPseudoSol`: executes branching rule for not
/// completely fixed pseudo solution.
pub fn branchrule_exec_pseudo_sol(scip: &mut Scip, branchrule: BranchruleId, cutoffbound: f64, allowaddcons: bool) -> ScipResult<ScipResultCode> {
    branchrule_exec(scip, branchrule, ExecKind::Pseudo, crate::sepastore::SEPASTORE, cutoffbound, allowaddcons)
}

/// `SCIPbranchruleSetPriority`: sets priority of branching rule.
pub fn branchrule_set_priority(scip: &mut Scip, branchrule: BranchruleId, priority: i32) {
    scip.branchrules[branchrule].priority = priority;
    scip.set.branchrulessorted = false;
}

/// `SCIPbranchruleEnableOrDisableClocks`.
pub fn branchrule_enable_or_disable_clocks(scip: &mut Scip, branchrule: BranchruleId, enable: bool) {
    let b = &mut scip.branchrules[branchrule];
    b.setuptime.enable_or_disable(enable);
    b.branchclock.enable_or_disable(enable);
}

/// `SCIPsetSortBranchrules`: sorts branching rules by priorities.
pub fn set_sort_branchrules(scip: &mut Scip) {
    if !scip.set.branchrulessorted {
        let n = scip.set.nbranchrules as usize;
        let mut rules = std::mem::take(&mut scip.set.branchrules);
        crate::sort::sort(&mut rules.0[..n], (), |&a, &b| branchrule_comp(scip, a, b));
        scip.set.branchrules = rules;
        scip.set.branchrulessorted = true;
    }
}

/*
 * branching methods
 */

/// `SCIPbranchGetScore`: calculates the branching score out of the gain
/// predictions for a binary branching; `var` is `NONE` to not apply a
/// branch factor.
pub fn branch_get_score(scip: &Scip, var: VarId, mut downgain: f64, mut upgain: f64) -> f64 {
    let set = &scip.set;

    // Adjust scores near zero to always yield product score greater than 0.
    let eps = set.sumepsilon();
    if set.branch_sumadjustscore {
        // Adjust scores by adding eps to keep near zero score differences
        // between variables.
        downgain += eps;
        upgain += eps;
    } else {
        // Disregard near zero score differences between variables and
        // consider the branching factor for them.
        downgain = crate::def::max(downgain, eps);
        upgain = crate::def::max(upgain, eps);
    }

    let mut score = match set.branch_scorefunc {
        // Linear sum score function: weigh the two child nodes with
        // branchscorefac and 1-branchscorefac.
        b's' => {
            if downgain > upgain {
                set.branch_scorefac * downgain + (1.0 - set.branch_scorefac) * upgain
            } else {
                set.branch_scorefac * upgain + (1.0 - set.branch_scorefac) * downgain
            }
        }
        // Product score function.
        b'p' => downgain * upgain,
        // Quotient score function.
        b'q' => {
            if downgain > upgain {
                upgain * upgain / downgain
            } else {
                downgain * downgain / upgain
            }
        }
        c => {
            log::error!("invalid branching score function <{}>", c as char);
            0.0
        }
    };

    // Apply the branch factor of the variable.
    if var != NONE {
        score *= scip.vars[var].branchfactor;
    }
    score
}

/// `SCIPbranchGetScoreMultiple`: calculates the branching score out of the
/// gain predictions for a branching with arbitrary many children.
pub fn branch_get_score_multiple(scip: &Scip, var: VarId, gains: &[f64]) -> f64 {
    // Search for the two minimal gains in the child list and use these to
    // calculate the branching score.
    let mut min1 = scip.set.infinity();
    let mut min2 = scip.set.infinity();
    for &g in gains {
        if g < min1 {
            min2 = min1;
            min1 = g;
        } else if g < min2 {
            min2 = g;
        }
    }
    branch_get_score(scip, var, min1, min2)
}

/// `SCIPbranchGetBranchingPoint`: computes a branching point for a (not
/// necessarily discrete) variable.
///
/// A suggested branching point is first projected onto the box; if no
/// point is suggested, then the value in the current LP or pseudo solution
/// is used; if this value is at infinity, then 0.0 projected onto the
/// bounds and then moved inside the interval is used; for a discrete
/// variable, it is ensured that the returned value is fractional; for a
/// continuous variable, the parameter branching/clamp defines how far a
/// branching point need to be from the bounds of a variable; the latter is
/// only applied if no point has been suggested, or the suggested point is
/// not inside the variable's interval.
pub fn branch_get_branching_point(scip: &Scip, var: VarId, suggestion: f64) -> f64 {
    use crate::def::{max, max3, min};
    let set = &scip.set;
    let v = &scip.vars[var];
    let mut lb = v.locdom.lb;
    let mut ub = v.locdom.ub;
    let mut branchpoint;

    // For a fixed variable, we cannot branch further.
    if !set.is_infinity(suggestion.abs()) {
        // Use user suggested branching point; first, project it onto the
        // current domain.
        branchpoint = max(lb, min(suggestion, ub));

        if crate::var::var_is_integral(scip, var) {
            // If it is a discrete variable, then try to round it down and up
            // and accept this choice.
            if set.is_eq(branchpoint, ub) {
                // If branchpoint is at ub, then create branches [lb,ub-1] and
                // [ub,ub], that is branch on ub-0.5; however, if ub is too
                // large, then ub-0.5 = ub (in double precision); in that case,
                // fall back to the code below, which branches closer to the
                // middle of [lb,ub].
                branchpoint = set.floor(branchpoint) - 0.5;
                if branchpoint < ub {
                    return branchpoint;
                }
            } else {
                // Create branches [lb,branchpoint] and [branchpoint+1,ub];
                // however, if |branchpoint| is too large, then
                // branchpoint+0.5 = branchpoint (in double precision); in that
                // case, fall back to the code below, which branches closer to
                // the middle of [lb,ub].
                branchpoint = set.floor(branchpoint) + 0.5;
                if branchpoint > lb {
                    return branchpoint;
                }
            }
        } else if (set.is_infinity(-lb) || set.is_rel_gt(branchpoint, lb)) && (set.is_infinity(ub) || set.is_rel_lt(branchpoint, ub)) {
            // If it is continuous and inside the box, then accept it.
            return branchpoint;
        }
        // If it is continuous and suggestion is one of the bounds, continue
        // below.
    } else {
        // If no point is suggested, try the LP or pseudo solution.
        let haslp = crate::tree::tree_has_current_node_lp(scip);
        branchpoint = crate::var::var_get_sol(scip, var, haslp);

        if branchpoint.abs() > 1e+12 {
            // If the value in the LP or pseudosolution is large (here 1e+12),
            // use 0.0 (will be projected onto bounds below).
            branchpoint = 0.0;
        } else if haslp && set.branch_midpull > 0.0 && !set.is_infinity(-lb) && !set.is_infinity(ub) {
            // If the value is from the LP and midpull is activated, then push
            // towards middle of domain.
            let mut midpull = set.branch_midpull;

            // Shrink midpull if width of local domain, relative to global
            // domain, is small; that is, if there has been already one or
            // several branchings on this variable, then give more emphasis on
            // LP solution; do this only if the relative domain width is below
            // the minreldomainwidth value.
            let glb = v.glbdom.lb;
            let gub = v.glbdom.ub;
            let reldomainwidth = if !set.is_infinity(-glb) && !set.is_infinity(gub) {
                (ub - lb) / (gub - glb)
            } else {
                set.epsilon()
            };

            if reldomainwidth < set.branch_midpullreldomtrig {
                midpull *= reldomainwidth;
            }

            branchpoint = midpull * (lb + ub) / 2.0 + (1.0 - midpull) * branchpoint;
        }

        // Make sure branchpoint is on interval, below we make sure that it is
        // within bounds for continuous vars.
        branchpoint = max(lb, min(branchpoint, ub));
    }

    // If value is at +/- infty, then choose some value a bit off from bounds
    // or 0.0.
    if set.is_infinity(branchpoint) {
        // If value is at +infty, then the upper bound should be at infinity;
        // choose 0.0 or something above lower bound if lower bound > 0.
        branchpoint = if set.is_positive(lb) { lb + 1000.0 } else { 0.0 };
    } else if set.is_infinity(-branchpoint) {
        // If value is at -infty, then the lower bound should be at -infinity;
        // choose 0.0 or something below upper bound if upper bound < 0.
        branchpoint = if set.is_negative(ub) { ub - 1000.0 } else { 0.0 };
    }

    if !crate::var::var_is_nonimplied_integral(scip, var) {
        if !set.is_infinity(-lb) || !set.is_infinity(ub) {
            // If one bound is missing, we are temporarily guessing the other
            // one, so we can apply the clamp below.
            if set.is_infinity(ub) {
                ub = lb + min(max(0.5 * lb.abs(), 1000.0), 0.9 * (set.infinity() - lb));
            } else if set.is_infinity(-lb) {
                lb = ub - min(max(0.5 * ub.abs(), 1000.0), 0.9 * (set.infinity() + ub));
            }

            // If branching point is too close to the bounds, move more into
            // the middle of the interval.
            if crate::misc::rel_diff(ub, lb) <= 2.02 * set.epsilon() {
                // For very tiny intervals we set it exactly into the middle.
                branchpoint = (lb + ub) / 2.0;
            } else {
                // Otherwise we project it away from the bounds.
                let lbabs = lb.abs();
                let ubabs = ub.abs();
                let scale = max3(lbabs, ubabs, 1.0);

                // The minimal branching point should be
                // - set->clamp away from the lower bound - relative to the local
                //   domain size
                // - SCIPsetEpsilon(set)*scale above the lower bound - in
                //   absolute value
                let mut minbrpoint = (1.0 - set.branch_clamp) * lb + set.branch_clamp * ub;
                minbrpoint = max(lb + 1.01 * set.epsilon() * scale, minbrpoint);

                // The maximal branching point should be
                // - set->clamp away from the upper bound - relative to the local
                //   domain size
                // - SCIPsetEpsilon(set)*scale below the upper bound - in
                //   absolute value
                let mut maxbrpoint = set.branch_clamp * lb + (1.0 - set.branch_clamp) * ub;
                maxbrpoint = min(ub - 1.01 * set.epsilon() * scale, maxbrpoint);

                // Project branchpoint into [minbrpoint, maxbrpoint].
                branchpoint = max(minbrpoint, min(branchpoint, maxbrpoint));

                // If selected branching point is close to 0.0 and bounds are
                // away from 0.0, it often makes sense to branch exactly on 0.0.
                if set.is_feas_zero(branchpoint) && set.is_feas_negative(lb) && set.is_feas_positive(ub) {
                    branchpoint = 0.0;
                }
            }
        }

        // Ensure fractional branching point for implicit integer variables.
        if crate::var::var_is_implied_integral(scip, var) && set.is_integral(branchpoint) {
            // If branchpoint is integral but not on bounds, then it should be
            // one of the value {lb+1, ..., ub-1}; if branchpoint is integral,
            // create one branch with x <= x'-1 and one with x >= x'.
            return branchpoint - 0.5;
        }

        branchpoint
    } else {
        // Discrete variables.
        if branchpoint <= lb + 0.5 {
            // If branchpoint is on lower bound, create one branch with x = lb
            // and one with x >= lb+1; however, if |lb| is huge, then lb + 0.5
            // == lb; in that case, branch at middle of [lb,ub].
            branchpoint = lb + 0.5;
            if branchpoint == lb {
                branchpoint = 0.5 * (lb + ub);
            }
            branchpoint
        } else if branchpoint >= ub - 0.5 {
            // If branchpoint is on upper bound, create one branch with x = ub
            // and one with x <= ub-1; however, if |ub| is huge, then ub - 0.5
            // == ub; in that case, branch at middle of [lb,ub].
            branchpoint = ub - 0.5;
            if branchpoint == ub {
                branchpoint = 0.5 * (lb + ub);
            }
            branchpoint
        } else if set.is_integral(branchpoint) {
            // If branchpoint is integral but not on bounds, then it should be
            // one of the value {lb+1, ..., ub-1}; if branchpoint is integral,
            // create one branch with x <= x'-1 and one with x >= x'.
            branchpoint - 0.5
        } else {
            // Branchpoint is somewhere between bounds and fractional, so just
            // round down and up.
            branchpoint
        }
    }
}

/// `SCIPbranchExecLP`: calls branching rules to branch on an LP solution;
/// if no fractional variables exist, the result is DIDNOTRUN; if the branch
/// priority of an unfixed variable is larger than the maximal branch
/// priority of the fractional variables, pseudo solution branching is
/// applied on the unfixed variables with maximal branch priority.
#[allow(clippy::too_many_arguments)]
pub fn branch_exec_lp(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    eventfilter: usize,
    sepastore: usize,
    cutoffbound: f64,
    allowaddcons: bool,
) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result = R::DidNotRun;

    // Calculate branching candidates.
    branchcand_calc_lp_cands(scip);

    let nalllpcands = scip.branchcand.nlpcands + scip.branchcand.nenfoimpllpfracs;

    // Do nothing, if no fractional variables exist.
    if nalllpcands == 0 {
        return Ok(result);
    }

    // If there is a non-fixed variable with higher priority than the maximal
    // priority of the fractional candidates, use pseudo solution branching
    // instead.
    if scip.branchcand.pseudomaxpriority > scip.branchcand.lpmaxpriority {
        return branch_exec_pseudo(scip, transprob, origprob, eventfilter, cutoffbound, allowaddcons);
    }

    // Sort the branching rules by priority.
    set_sort_branchrules(scip);

    // Try all branching rules until one succeeded to branch.
    let mut i = 0;
    while i < scip.set.nbranchrules && (result == R::DidNotRun || result == R::DidNotFind) {
        let rule = scip.set.branchrules[i];
        result = branchrule_exec_lp_sol(scip, rule, sepastore, cutoffbound, allowaddcons)?;
        i += 1;
    }

    if result == R::DidNotRun || result == R::DidNotFind {
        // No branching method succeeded in choosing a branching: just branch
        // on the first fractional variable with maximal priority, and out of
        // these on the one with maximal branch factor.
        let mut bestcand = -1;
        let mut bestpriority = i32::MIN;
        let mut bestfactor = crate::def::REAL_MIN;
        for i in 0..nalllpcands {
            let v = &scip.vars[scip.branchcand.lpcands[i]];
            let priority = v.branchpriority;
            let factor = v.branchfactor;
            if priority > bestpriority || (priority == bestpriority && factor > bestfactor) {
                bestcand = i;
                bestpriority = priority;
                bestfactor = factor;
            }
        }

        let var = scip.branchcand.lpcands[bestcand];
        crate::tree::tree_branch_var(scip, transprob, origprob, eventfilter, var, crate::def::INVALID)?;
        result = R::Branched;
    }
    Ok(result)
}

/// `SCIPbranchExecExtern`: calls branching rules to branch on a external
/// candidates; if no such candidates exist, the result is DIDNOTRUN.
#[allow(clippy::too_many_arguments)]
pub fn branch_exec_extern(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    eventfilter: usize,
    sepastore: usize,
    cutoffbound: f64,
    allowaddcons: bool,
) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result = R::DidNotRun;

    // Do nothing, if no external candidates exist.
    if scip.branchcand.nexterncands == 0 {
        return Ok(result);
    }

    // If there is a non-fixed variable with higher priority than the maximal
    // priority of the external candidates, use pseudo solution branching
    // instead.
    if scip.branchcand.pseudomaxpriority > scip.branchcand.externmaxpriority {
        return branch_exec_pseudo(scip, transprob, origprob, eventfilter, cutoffbound, allowaddcons);
    }

    // Sort the branching rules by priority.
    set_sort_branchrules(scip);

    // Try all branching rules until one succeeded to branch.
    let mut i = 0;
    while i < scip.set.nbranchrules && (result == R::DidNotRun || result == R::DidNotFind) {
        let rule = scip.set.branchrules[i];
        result = branchrule_exec_extern_sol(scip, rule, sepastore, cutoffbound, allowaddcons)?;
        i += 1;
    }

    if result == R::DidNotRun || result == R::DidNotFind {
        // No branching method succeeded in choosing a branching: just branch
        // on the first branching candidates with maximal priority, and out of
        // these on the one with maximal branch factor, and out of these on
        // the one with largest domain.
        let mut bestcand = -1;
        let mut bestpriority = i32::MIN;
        let mut bestfactor = crate::def::REAL_MIN;
        let mut bestdomain = 0.0;
        for i in 0..scip.branchcand.nexterncands {
            let cand = scip.branchcand.externcands[i];
            let v = &scip.vars[cand];
            let priority = v.branchpriority;
            let factor = v.branchfactor;

            // The domain size is infinite, iff one of the bounds is infinite.
            let domain = if scip.set.is_infinity(-v.locdom.lb) || scip.set.is_infinity(v.locdom.ub) {
                scip.set.infinity()
            } else {
                v.locdom.ub - v.locdom.lb
            };

            // Choose variable with higher priority, higher factor, larger
            // domain (in that order).
            if priority > bestpriority || (priority == bestpriority && factor > bestfactor) || (priority == bestpriority && factor == bestfactor && domain > bestdomain) {
                bestcand = i;
                bestpriority = priority;
                bestfactor = factor;
                bestdomain = domain;
            }
        }

        let var = scip.branchcand.externcands[bestcand];
        let val = branch_get_branching_point(scip, var, scip.branchcand.externcandssol[bestcand]);

        crate::tree::tree_branch_var(scip, transprob, origprob, eventfilter, var, val)?;

        if scip.tree.as_ref().unwrap().nchildren >= 1 {
            result = R::Branched;
        }
        // If the bounds are too close, it may happen that we cannot branch
        // but rather fix the variable.
        else {
            result = R::ReducedDom;
        }
    }
    Ok(result)
}

/// `SCIPbranchExecPseudo`: calls branching rules to branch on a pseudo
/// solution; if no unfixed variables exist, the result is DIDNOTRUN.
pub fn branch_exec_pseudo(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize, cutoffbound: f64, allowaddcons: bool) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;
    let mut result = R::DidNotRun;

    // Do nothing, if no unfixed variables exist.
    if scip.branchcand.npseudocands == 0 {
        return Ok(result);
    }

    // Sort the branching rules by priority.
    set_sort_branchrules(scip);

    // Try all branching rules until one succeeded to branch.
    let mut i = 0;
    while i < scip.set.nbranchrules && (result == R::DidNotRun || result == R::DidNotFind) {
        let rule = scip.set.branchrules[i];
        result = branchrule_exec_pseudo_sol(scip, rule, cutoffbound, allowaddcons)?;
        i += 1;
    }

    if result == R::DidNotRun || result == R::DidNotFind {
        // No branching method succeeded in choosing a branching: just branch
        // on the first unfixed variable with maximal priority, and out of
        // these on the one with maximal branch factor.
        let mut bestcand = -1;
        let mut bestpriority = i32::MIN;
        let mut bestfactor = crate::def::REAL_MIN;
        for i in 0..scip.branchcand.npseudocands {
            let v = &scip.vars[scip.branchcand.pseudocands[i]];
            let priority = v.branchpriority;
            let factor = v.branchfactor;
            if priority > bestpriority || (priority == bestpriority && factor > bestfactor) {
                bestcand = i;
                bestpriority = priority;
                bestfactor = factor;
            }
        }

        let var = scip.branchcand.pseudocands[bestcand];
        crate::tree::tree_branch_var(scip, transprob, origprob, eventfilter, var, crate::def::INVALID)?;
        result = R::Branched;
    }
    Ok(result)
}

/// `SCIPbranchcandGetLPMaxPrio`: gets maximal branching priority of LP
/// branching candidates.
pub fn branchcand_get_lp_max_prio(scip: &Scip) -> i32 {
    scip.branchcand.lpmaxpriority
}

/// `SCIPbranchcandGetExternMaxPrio`: gets maximal branching priority of
/// external branching candidates.
pub fn branchcand_get_extern_max_prio(scip: &Scip) -> i32 {
    scip.branchcand.externmaxpriority
}

/// `SCIPbranchcandGetNExternCands`: gets number of external branching
/// candidates.
pub fn branchcand_get_n_extern_cands(scip: &Scip) -> i32 {
    scip.branchcand.nexterncands
}
