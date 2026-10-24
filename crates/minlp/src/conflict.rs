//! SCIP's conflict analysis (`conflict_graphanalysis.c`,
//! `conflict_general.c`, `conflict_dualproofanalysis.c`,
//! `struct_conflict.h`).
//!
//! C points at bound change infos directly: into the variables' bound
//! change arrays, or at temporary ones conflict analysis creates for
//! relaxed bounds. Here a [`BdChgInfoRef`] names either.

use std::any::Any;

use crate::{
    clock::Clock,
    cvec::CVec,
    pqueue::PQueue,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{BoundType, BranchDir, ConfType, ConflicthdlrId, NodeId, ScipResultCode, VarId},
    var::{BdChgInfo, NONE},
};

/// A bound change info C conflict analysis points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BdChgInfoRef {
    /// Entry `pos` of the variable's lower or upper bound change infos.
    Var { var: VarId, boundtype: BoundType, pos: i32 },
    /// Entry of `Conflict::tmpbdchginfos`.
    Tmp(usize),
}

impl Default for BdChgInfoRef {
    fn default() -> Self {
        BdChgInfoRef::Tmp(NONE)
    }
}

/// The bound change info `r` names.
#[inline]
pub fn bdchginfo(scip: &Scip, r: BdChgInfoRef) -> &BdChgInfo {
    match r {
        BdChgInfoRef::Var { var, boundtype, pos } => crate::var::var_bdchginfo(scip, var, boundtype, pos),
        BdChgInfoRef::Tmp(i) => &scip.conflict.as_ref().unwrap().tmpbdchginfos[i],
    }
}

/// `SCIPbdchginfoGetDepth`.
#[inline]
pub fn bdchginfo_get_depth(scip: &Scip, r: BdChgInfoRef) -> i32 {
    bdchginfo(scip, r).bdchgidx.depth
}

/// `SCIPbdchginfoGetVar`.
#[inline]
pub fn bdchginfo_get_var(scip: &Scip, r: BdChgInfoRef) -> VarId {
    bdchginfo(scip, r).var
}

/// `SCIPbdchginfoGetBoundtype`.
#[inline]
pub fn bdchginfo_get_boundtype(scip: &Scip, r: BdChgInfoRef) -> BoundType {
    bdchginfo(scip, r).boundtype
}

/// `SCIPbdchginfoGetNewbound`.
#[inline]
pub fn bdchginfo_get_newbound(scip: &Scip, r: BdChgInfoRef) -> f64 {
    bdchginfo(scip, r).newbound
}

/// `SCIPbdchginfoIsTighter`.
#[inline]
fn bdchginfo_is_tighter(scip: &Scip, a: BdChgInfoRef, b: BdChgInfoRef) -> bool {
    crate::var::bdchginfo_is_tighter(bdchginfo(scip, a), bdchginfo(scip, b))
}

pub type ConflicthdlrMethod = fn(&mut Scip, ConflicthdlrId) -> ScipResult;
/// A copy method: `(target scip, source scip, source plugin)`.
pub type ConflicthdlrCopy = fn(&mut Scip, &Scip, ConflicthdlrId) -> ScipResult;
/// The arguments `SCIP_DECL_CONFLICTEXEC` receives.
pub struct ConflictExecArgs<'a> {
    pub node: NodeId,
    pub validnode: NodeId,
    pub bdchginfos: &'a [BdChgInfoRef],
    pub relaxedbds: &'a [f64],
    pub conftype: ConfType,
    pub cutoffinvolved: bool,
    pub separate: bool,
    pub local: bool,
    pub dynamic: bool,
    pub removable: bool,
    pub resolved: bool,
}
/// `SCIP_DECL_CONFLICTEXEC`.
pub type ConflicthdlrExec = fn(&mut Scip, ConflicthdlrId, &ConflictExecArgs, &mut ScipResultCode) -> ScipResult;

/// `SCIP_CONFLICTHDLR`.
pub struct Conflicthdlr {
    pub name: String,
    pub desc: String,
    pub conflictcopy: Option<ConflicthdlrCopy>,
    pub conflictfree: Option<ConflicthdlrMethod>,
    pub conflictinit: Option<ConflicthdlrMethod>,
    pub conflictexit: Option<ConflicthdlrMethod>,
    pub conflictinitsol: Option<ConflicthdlrMethod>,
    pub conflictexitsol: Option<ConflicthdlrMethod>,
    pub conflictexec: Option<ConflicthdlrExec>,
    pub conflicthdlrdata: Option<Box<dyn Any>>,
    pub setuptime: Clock,
    pub conflicttime: Clock,
    pub priority: i32,
    pub initialized: bool,
}

/// `SCIP_CONFLICTSET`.
#[derive(Clone, Debug, Default)]
pub struct ConflictSet {
    pub bdchginfos: CVec<BdChgInfoRef>,
    pub confbdchginfo: Option<BdChgInfoRef>,
    pub relaxedbds: CVec<f64>,
    pub confrelaxedbd: f64,
    pub sortvals: CVec<i32>,
    pub bdchginfossize: i32,
    pub nbdchginfos: i32,
    pub validdepth: i32,
    pub insertdepth: i32,
    pub conflictdepth: i32,
    pub repropdepth: i32,
    pub repropagate: bool,
    pub depthcalced: bool,
    pub sorted: bool,
    pub usescutoffbound: bool,
    pub hasrelaxonlyvar: bool,
    pub conflicttype: ConfType,
}

/// `SCIP_PROOFSET`.
#[derive(Clone, Debug, Default)]
pub struct ProofSet {
    pub vals: CVec<f64>,
    pub inds: CVec<i32>,
    pub rhs: f64,
    pub nnz: i32,
    pub size: i32,
    pub validdepth: i32,
    pub conflicttype: ConfType,
    pub certificateline: i64,
}

/// `SCIP_LPBDCHGS`.
#[derive(Clone, Debug, Default)]
pub struct LpBdChgs {
    pub bdchginds: CVec<i32>,
    pub bdchglbs: CVec<f64>,
    pub bdchgubs: CVec<f64>,
    pub bdchgcolinds: CVec<i32>,
    pub usedcols: CVec<bool>,
    pub nbdchgs: i32,
}

/// `SCIP_CONFLICT` (without the generalized resolution members, whose
/// analysis is off by default and not ported).
#[derive(Default)]
pub struct Conflict {
    pub nglbchgbds: i64,
    pub nappliedglbconss: i64,
    pub nappliedglbresconss: i64,
    pub nappliedglbliterals: i64,
    pub nlocchgbds: i64,
    pub nappliedlocconss: i64,
    pub nappliedlocliterals: i64,
    pub npropcalls: i64,
    pub npropsuccess: i64,
    pub npropconfconss: i64,
    pub npropconfliterals: i64,
    pub npropreconvconss: i64,
    pub npropreconvliterals: i64,
    pub nrescalls: i64,
    pub nressuccess: i64,
    pub ninflpcalls: i64,
    pub ninflpsuccess: i64,
    pub ninflpconfconss: i64,
    pub ninflpconfliterals: i64,
    pub ninflpreconvconss: i64,
    pub ninflpreconvliterals: i64,
    pub ninflpiterations: i64,
    pub nboundlpcalls: i64,
    pub nboundlpsuccess: i64,
    pub nboundlpconfconss: i64,
    pub nboundlpconfliterals: i64,
    pub nboundlpreconvconss: i64,
    pub nboundlpreconvliterals: i64,
    pub nboundlpiterations: i64,
    pub nsbcalls: i64,
    pub nsbsuccess: i64,
    pub nsbconfconss: i64,
    pub nsbconfliterals: i64,
    pub nsbreconvconss: i64,
    pub nsbreconvliterals: i64,
    pub nsbiterations: i64,
    pub npseudocalls: i64,
    pub npseudosuccess: i64,
    pub npseudoconfconss: i64,
    pub npseudoconfliterals: i64,
    pub npseudoreconvconss: i64,
    pub npseudoreconvliterals: i64,
    pub ndualproofsinfglobal: i64,
    pub ndualproofsinflocal: i64,
    pub ndualproofsinfsuccess: i64,
    pub dualproofsinfnnonzeros: i64,
    pub ndualproofsbndglobal: i64,
    pub ndualproofsbndlocal: i64,
    pub ndualproofsbndsuccess: i64,
    pub dualproofsbndnnonzeros: i64,
    pub dibclock: Clock,
    pub propanalyzetime: Clock,
    pub resanalyzetime: Clock,
    pub inflpanalyzetime: Clock,
    pub boundlpanalyzetime: Clock,
    pub sbanalyzetime: Clock,
    pub pseudoanalyzetime: Clock,
    pub bdchgqueue: PQueue<BdChgInfoRef>,
    pub forcedbdchgqueue: PQueue<BdChgInfoRef>,
    pub proofset: Box<ProofSet>,
    pub proofsets: Vec<Box<ProofSet>>,
    pub conflictset: Box<ConflictSet>,
    pub conflictsets: Vec<Box<ConflictSet>>,
    pub conflictsetscores: CVec<f64>,
    pub conflictvarslbs: CVec<f64>,
    pub conflictvarsubs: CVec<f64>,
    pub tmpbdchginfos: CVec<BdChgInfo>,
    pub conflictprobnvars: i32,
    pub conflictsetssize: i32,
    pub nconflictsets: i32,
    pub proofsetssize: i32,
    pub nproofsets: i32,
    pub tmpbdchginfossize: i32,
    pub ntmpbdchginfos: i32,
    pub count: i32,
    pub bdchgonlyconfqueue: bool,
}

macro_rules! cf {
    ($scip:expr) => {
        (**$scip.conflict.as_mut().unwrap())
    };
}

/*
 * Conflict sets
 */

/// `conflictsetEnsureBdchginfosMem`: resizes the arrays of the conflict
/// set to be able to store at least `num` bound change entries.
fn conflictset_ensure_bdchginfos_mem(scip: &Scip, cs: &mut ConflictSet, num: i32) {
    if num > cs.bdchginfossize {
        let newsize = scip.set.calc_mem_grow_size(num);
        cs.bdchginfos.0.resize(newsize as usize, BdChgInfoRef::default());
        cs.relaxedbds.realloc(newsize);
        cs.sortvals.realloc(newsize);
        cs.bdchginfossize = newsize;
    }
}

/// `conflictsetAddBound`: adds a bound change to a conflict set.
fn conflictset_add_bound(scip: &Scip, cs: &mut ConflictSet, info: BdChgInfoRef, relaxedbd: f64) {
    // Allocate memory for additional element.
    conflictset_ensure_bdchginfos_mem(scip, cs, cs.nbdchginfos + 1);

    // Insert the new bound change in the arrays sorted by increasing variable
    // index and by bound type.
    let var = bdchginfo_get_var(scip, info);
    let boundtype = bdchginfo_get_boundtype(scip, info);
    let idx = scip.vars[var].index;
    // First sorting criteria: variable index, second criteria: boundtype.
    let sortval = 2 * idx + boundtype as i32;

    // Insert new element into the sorted arrays; if an element exits with the
    // same value insert the new element afterwards.
    let mut n = cs.nbdchginfos as usize;
    let pos = {
        let ConflictSet {
            sortvals, bdchginfos, relaxedbds, ..
        } = cs;
        let mut fields = (&mut bdchginfos.0[..], &mut relaxedbds.0[..]);
        crate::sort::sortedvec_insert(&mut sortvals.0, &mut fields, crate::sort::cmp_int, sortval, (info, relaxedbd), &mut n)
    };
    cs.nbdchginfos = n as i32;

    // Merge multiple bound changes.
    if pos > 0 && sortval == cs.sortvals[pos - 1] {
        let del = |cs: &mut ConflictSet, p: usize| {
            let mut n = cs.nbdchginfos as usize;
            let ConflictSet {
                sortvals, bdchginfos, relaxedbds, ..
            } = cs;
            let mut fields = (&mut bdchginfos.0[..], &mut relaxedbds.0[..]);
            crate::sort::sortedvec_del_pos(&mut sortvals.0[..], &mut fields, p, &mut n);
            cs.nbdchginfos = n as i32;
        };

        // This is a multiple bound change.
        if bdchginfo_is_tighter(scip, info, cs.bdchginfos[pos - 1]) {
            // Remove the "old" bound change since the "new" one in tighter.
            del(cs, pos - 1);
        } else if bdchginfo_is_tighter(scip, cs.bdchginfos[pos - 1], info) {
            // Remove the "new" bound change since the "old" one is tighter.
            del(cs, pos);
        } else {
            // Both bound change are equivalent; hence, keep the worse relaxed
            // bound and remove one of them.
            let r = cs.relaxedbds[pos - 1];
            cs.relaxedbds[pos - 1] = if boundtype == BoundType::Lower {
                crate::def::max(r, relaxedbd)
            } else {
                crate::def::min(r, relaxedbd)
            };
            del(cs, pos);
        }
    }

    if scip.vars[var].relaxationonly {
        cs.hasrelaxonlyvar = true;
    }
}

/// `conflictsetCalcConflictDepth`: calculates the conflict and the
/// repropagation depths of the conflict set.
fn conflictset_calc_conflict_depth(scip: &Scip, cs: &mut ConflictSet) {
    // Get the depth of the last and last but one bound change.
    let mut maxdepth = [cs.validdepth, cs.validdepth];
    for i in 0..cs.nbdchginfos {
        let depth = bdchginfo_get_depth(scip, cs.bdchginfos[i]);
        if depth > maxdepth[0] {
            maxdepth[1] = maxdepth[0];
            maxdepth[0] = depth;
        } else if depth > maxdepth[1] {
            maxdepth[1] = depth;
        }
    }

    cs.conflictdepth = maxdepth[0];
    cs.repropdepth = maxdepth[1];
}

/// `conflictsetCalcInsertDepth`: identifies the depth, at which the
/// conflict set should be added:
///  - if the branching rule operates on variables only, and if all
///    branching variables up to a certain depth level are member of the
///    conflict, the conflict constraint can only be violated in the subtree
///    of the node at that depth, because in all other nodes, at least one
///    of these branching variables violates its conflicting bound, such
///    that the conflict constraint is feasible
///  - if there is at least one branching variable in a node, we assume,
///    that this branching was performed on variables, and that the siblings
///    of this node are disjunct w.r.t. the branching variables' fixings
///  - we have to add the conflict set at least in the valid depth of the
///    initial conflict set, so we start searching at the first branching
///    after this depth level, i.e. validdepth+1
fn conflictset_calc_insert_depth(scip: &Scip, cs: &mut ConflictSet) {
    // The conflict set must not be inserted prior to its valid depth.
    cs.insertdepth = cs.validdepth;

    let currentdepth = crate::tree::tree_get_current_depth(scip);

    // Mark the levels for which a branching variable is included in the
    // conflict set.
    let mut branchingincluded = vec![false; (currentdepth + 2) as usize];
    for i in 0..cs.nbdchginfos {
        let mut depth = bdchginfo_get_depth(scip, cs.bdchginfos[i]);
        // Put diving/probing/strong branching changes in this depth level.
        depth = depth.min(currentdepth + 1);
        branchingincluded[depth as usize] = true;
    }

    // Skip additional depth levels where branching on the conflict variables
    // was applied.
    while cs.insertdepth < currentdepth && branchingincluded[(cs.insertdepth + 1) as usize] {
        cs.insertdepth += 1;
    }
}

/// `conflictsetIsRedundant`: checks whether the first conflict set is
/// redundant to the second one.
fn conflictset_is_redundant(scip: &Scip, cs1: &ConflictSet, cs2: &ConflictSet) -> bool {
    // If conflictset1 has smaller validdepth, it is definitely not redundant
    // to conflictset2.
    if cs1.validdepth < cs2.validdepth {
        return false;
    }

    // Check, if all bound changes in conflictset2 are also present at least
    // as tight in conflictset1; we can stop immediately, if more bound
    // changes are remaining in conflictset2 than in conflictset1.
    let (mut i1, mut i2) = (0, 0);
    while i2 < cs2.nbdchginfos && cs1.nbdchginfos - i1 >= cs2.nbdchginfos - i2 {
        let sortval = cs2.sortvals[i2];
        while i1 < cs1.nbdchginfos && cs1.sortvals[i1] < sortval {
            // While scanning conflictset1, check consistency.
            i1 += 1;
        }
        if i1 >= cs1.nbdchginfos || cs1.sortvals[i1] > sortval || bdchginfo_is_tighter(scip, cs2.bdchginfos[i2], cs1.bdchginfos[i1]) {
            return false;
        }
        i1 += 1;
        i2 += 1;
    }

    i2 == cs2.nbdchginfos
}

/// `checkRedundancy`: check conflict set for redundancy, other conflicts in
/// the same conflict analysis could have led to global reductions an made
/// this conflict set redundant.
fn check_redundancy(scip: &Scip, cs: &ConflictSet) -> bool {
    // Check all boundtypes and bounds for redundancy.
    let mut v = cs.nbdchginfos - 1;
    while v >= 0 {
        let info = cs.bdchginfos[v];
        let var = bdchginfo_get_var(scip, info);

        // Check if the relaxed bound is really a relaxed bound.
        let mut bound = cs.relaxedbds[v];

        if bdchginfo_get_boundtype(scip, info) == BoundType::Upper {
            if crate::var::var_is_integral(scip, var) {
                bound += 1.0;
            }

            // Check if the bound is already fulfilled globally.
            if scip.set.is_feas_ge(scip.vars[var].glbdom.lb, bound) {
                return true;
            }
        } else {
            if crate::var::var_is_integral(scip, var) {
                bound -= 1.0;
            }

            // Check if the bound is already fulfilled globally.
            if scip.set.is_feas_le(scip.vars[var].glbdom.ub, bound) {
                return true;
            }
        }
        v -= 1;
    }
    false
}

/// The outcome of `detectImpliedBounds`.
struct ImpliedBounds {
    nbdchgs: i32,
    nredvars: i32,
    redundant: bool,
}

/// `detectImpliedBounds`: find global fixings which can be derived from the
/// new conflict set.
fn detect_implied_bounds(scip: &mut Scip, cs: &mut ConflictSet, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult<ImpliedBounds> {
    // Only check conflict sets with more than one variable.
    let mut out = ImpliedBounds {
        nbdchgs: 0,
        nredvars: 0,
        redundant: false,
    };

    // Due to other conflict in the same conflict analysis, this conflict set
    // might have become redundant.
    out.redundant = check_redundancy(scip, cs);

    if out.redundant {
        return Ok(out);
    }

    let mut nbdchginfos = cs.nbdchginfos;

    // Check if the boolean representation of boundtypes matches the
    // 'standard' definition.
    let mut ntrivialredvars = 0;

    // Due to multiple conflict sets for one conflict, it can happen, that we
    // already have redundant information in the conflict set.
    let mut v = nbdchginfos - 1;
    while v >= 0 {
        let info = cs.bdchginfos[v];
        let var = bdchginfo_get_var(scip, info);
        let mut bound = cs.relaxedbds[v];
        let isupper = bdchginfo_get_boundtype(scip, info) == BoundType::Lower;

        // For integral variable we can increase/decrease the conflicting
        // bound.
        if crate::var::var_is_integral(scip, var) {
            bound += if isupper { -1.0 } else { 1.0 };
        }

        // If conflict variable cannot fulfill the conflict we can remove it.
        if (isupper && scip.set.is_feas_lt(bound, scip.vars[var].glbdom.lb)) || (!isupper && scip.set.is_feas_gt(bound, scip.vars[var].glbdom.ub)) {
            let last = nbdchginfos - 1;
            cs.bdchginfos[v] = cs.bdchginfos[last];
            cs.relaxedbds[v] = cs.relaxedbds[last];
            cs.sortvals[v] = cs.sortvals[last];
            nbdchginfos -= 1;
            ntrivialredvars += 1;
        }
        v -= 1;
    }
    cs.nbdchginfos = nbdchginfos;

    // All variables where removed, the conflict cannot be fulfilled, i.e., we
    // have an infeasibility proof.
    if cs.nbdchginfos == 0 {
        return Ok(out);
    }

    // Do not check to big or trivial conflicts.
    if cs.nbdchginfos > scip.set.conf_maxvarsdetectimpliedbounds || cs.nbdchginfos == 1 {
        out.nredvars = ntrivialredvars;
        return Ok(out);
    }

    // Create array of boundtypes, and bound values in conflict set.
    let n = nbdchginfos as usize;
    let mut boundtypes = vec![false; n];
    let mut bounds = vec![0.0; n];

    // Memory for the estimates for binary implications used for sorting.
    let mut nbinimpls = vec![0i64; n];

    let mut nzeroimpls = 0;

    // Collect estimates and initialize variables, boundtypes, and bounds
    // array.
    let mut v = 0;
    while v < n {
        let info = cs.bdchginfos[v];
        let var = bdchginfo_get_var(scip, info);
        boundtypes[v] = bdchginfo_get_boundtype(scip, info) == BoundType::Lower;
        bounds[v] = cs.relaxedbds[v];

        // Check if the relaxed bound is really a relaxed bound.

        // For continuous variables, we can only use the relaxed version of the
        // bounds negation: !(x <= u) -> x >= u.
        let ncliques = |scip: &Scip, fix: bool| scip.vars[var].cliquelist.as_ref().map_or(0, |cl| cl.ncliques[fix as usize]) as i64;
        let nvlbs = |scip: &Scip| crate::implics::vbounds_get_n_vbds(&scip.vars[var].vlbs) as i64;
        let nvubs = |scip: &Scip| crate::implics::vbounds_get_n_vbds(&scip.vars[var].vubs) as i64;
        if crate::var::var_is_binary(scip, var) {
            if !boundtypes[v] {
                bounds[v] = 1.0;
                nbinimpls[v] = ncliques(scip, true) * 2;
            } else {
                bounds[v] = 0.0;
                nbinimpls[v] = ncliques(scip, false) * 2;
            }
        } else if crate::var::var_is_integral(scip, var) {
            bounds[v] += if !boundtypes[v] { 1.0 } else { -1.0 };
            nbinimpls[v] = if boundtypes[v] { nvlbs(scip) } else { nvubs(scip) };
        } else if (!boundtypes[v] && scip.set.is_feas_eq(scip.vars[var].glbdom.lb, bounds[v])) || (boundtypes[v] && scip.set.is_feas_eq(scip.vars[var].glbdom.ub, bounds[v])) {
            // The literal is satisfied in global bounds (may happen due to
            // weak "negation" of continuous variables) -> discard the
            // conflict constraint.
            break;
        } else {
            nbinimpls[v] = if boundtypes[v] { nvlbs(scip) } else { nvubs(scip) };
        }

        if nbinimpls[v] == 0 {
            nzeroimpls += 1;
        }
        v += 1;
    }

    // Starting to derive global bound changes.
    let full = scip.set.conf_fullshortenconflict;
    if v == n && ((!full && nzeroimpls < 2) || (full && nzeroimpls < n)) {
        // Sort variables in increasing order of binary implications to gain
        // speed later on.
        {
            let ConflictSet { bdchginfos, relaxedbds, .. } = cs;
            crate::sort::sort(
                &mut nbinimpls[..v],
                (&mut bdchginfos.0[..v], &mut relaxedbds.0[..v], &mut bounds[..v], &mut boundtypes[..v]),
                crate::sort::cmp_long,
            );
        }

        // Initialize conflict variable data.
        let vars: Vec<VarId> = (0..n).map(|v| bdchginfo_get_var(scip, cs.bdchginfos[v])).collect();
        let mut redundants = vec![false; n];

        let mut nredvars = 0;
        let mut nbdchgs = 0;
        let mut redundant = false;
        let mut glbinfeas = false;
        crate::presolve::shrink_disjunctive_var_set(
            scip,
            &vars,
            &bounds,
            &boundtypes,
            &mut redundants,
            n as i32,
            &mut nredvars,
            &mut nbdchgs,
            &mut redundant,
            &mut glbinfeas,
            full,
        )?;
        out.nredvars = nredvars;
        out.nbdchgs = nbdchgs;
        out.redundant = redundant;

        if glbinfeas {
            let root = crate::tree::tree_get_root_node(scip);
            crate::tree::node_cutoff(scip, root, eventfilter, transprob, origprob)?;
        } else if !out.redundant && out.nredvars > 0 {
            // Remove as redundant marked variables.
            let mut v = n as i32 - 1;
            while v >= 0 {
                // If conflict variable was marked to be redundant remove it.
                if redundants[v as usize] {
                    let last = nbdchginfos - 1;
                    cs.bdchginfos[v] = cs.bdchginfos[last];
                    cs.relaxedbds[v] = cs.relaxedbds[last];
                    cs.sortvals[v] = cs.sortvals[last];
                    nbdchginfos -= 1;
                }
                v -= 1;
            }
            cs.nbdchginfos = nbdchginfos;
        }
    }

    out.nredvars += ntrivialredvars;
    Ok(out)
}

/// `conflictsetClear`: clears the given conflict set.
fn conflictset_clear(cs: &mut ConflictSet) {
    cs.nbdchginfos = 0;
    cs.validdepth = 0;
    cs.insertdepth = 0;
    cs.conflictdepth = 0;
    cs.repropdepth = 0;
    cs.repropagate = true;
    cs.usescutoffbound = false;
    cs.hasrelaxonlyvar = false;
    cs.conflicttype = ConfType::Unknown;
}

/// `SCIPconflictsetCreate`: creates an empty conflict set.
pub fn conflictset_create() -> Box<ConflictSet> {
    let mut cs = Box::<ConflictSet>::default();
    conflictset_clear(&mut cs);
    cs
}

/// `conflictsetCopy`: creates a copy of the given conflict set, allocating
/// an additional amount of memory.
fn conflictset_copy(source: &ConflictSet, nadditionalelems: i32) -> Box<ConflictSet> {
    let targetsize = source.nbdchginfos + nadditionalelems;
    let n = source.nbdchginfos;
    let mut t = Box::new(ConflictSet {
        bdchginfos: CVec::with_size(targetsize),
        relaxedbds: CVec::with_size(targetsize),
        sortvals: CVec::with_size(targetsize),
        bdchginfossize: targetsize,
        nbdchginfos: n,
        validdepth: source.validdepth,
        insertdepth: source.insertdepth,
        conflictdepth: source.conflictdepth,
        repropdepth: source.repropdepth,
        usescutoffbound: source.usescutoffbound,
        hasrelaxonlyvar: source.hasrelaxonlyvar,
        conflicttype: source.conflicttype,
        ..Default::default()
    });
    t.bdchginfos.head_mut(n).copy_from_slice(source.bdchginfos.head(n));
    t.relaxedbds.head_mut(n).copy_from_slice(source.relaxedbds.head(n));
    t.sortvals.head_mut(n).copy_from_slice(source.sortvals.head(n));
    t
}

/// `conflictsetCalcScore`: calculates the score of the conflict set; the
/// score is weighted sum of number of bound changes, repropagation depth,
/// and valid depth.
fn conflictset_calc_score(scip: &Scip, cs: &ConflictSet) -> f64 {
    let set = &scip.set;
    -(set.conf_weightsize * cs.nbdchginfos as f64 + set.conf_weightrepropdepth * cs.repropdepth as f64 + set.conf_weightvaliddepth * cs.validdepth as f64)
}

/*
 * Conflict Handler
 */

/// `SCIPconflicthdlrComp`: compares two conflict handlers w. r. to their
/// priority.
pub fn conflicthdlr_comp(scip: &Scip, elem1: ConflicthdlrId, elem2: ConflicthdlrId) -> i32 {
    scip.conflicthdlrs[elem2].priority - scip.conflicthdlrs[elem1].priority
}

/// `paramChgdConflicthdlrPriority`: marks the conflict handlers unsorted
/// (`SCIPsetConflicthdlrPriority`).
fn param_chgd_conflicthdlr_priority(scip: &mut Scip, _param: crate::paramset::ParamId) -> ScipResult {
    scip.set.conflicthdlrssorted = false;
    Ok(())
}

/// `SCIPconflicthdlrCopyInclude`: copies the given conflict handler to a
/// new scip.
pub fn conflicthdlr_copy_include(scip: &mut Scip, sourcescip: &Scip, h: ConflicthdlrId) -> ScipResult {
    if let Some(copy) = sourcescip.conflicthdlrs[h].conflictcopy {
        copy(scip, sourcescip, h)?;
    }
    Ok(())
}

/// The callbacks `SCIPconflicthdlrCreate` takes.
#[derive(Default)]
pub struct ConflicthdlrMethods {
    pub conflictcopy: Option<ConflicthdlrCopy>,
    pub conflictfree: Option<ConflicthdlrMethod>,
    pub conflictinit: Option<ConflicthdlrMethod>,
    pub conflictexit: Option<ConflicthdlrMethod>,
    pub conflictinitsol: Option<ConflicthdlrMethod>,
    pub conflictexitsol: Option<ConflicthdlrMethod>,
    pub conflictexec: Option<ConflicthdlrExec>,
}

/// `SCIPconflicthdlrCreate`: creates a conflict handler.
pub fn conflicthdlr_create(scip: &mut Scip, name: &str, desc: &str, priority: i32, methods: ConflicthdlrMethods, data: Option<Box<dyn Any>>) -> ScipResult<ConflicthdlrId> {
    let h = scip.conflicthdlrs.push(Conflicthdlr {
        name: name.to_string(),
        desc: desc.to_string(),
        priority,
        conflictcopy: methods.conflictcopy,
        conflictfree: methods.conflictfree,
        conflictinit: methods.conflictinit,
        conflictexit: methods.conflictexit,
        conflictinitsol: methods.conflictinitsol,
        conflictexitsol: methods.conflictexitsol,
        conflictexec: methods.conflictexec,
        conflicthdlrdata: data,
        setuptime: Clock::new(),
        conflicttime: Clock::new(),
        initialized: false,
    });

    // Add parameters.
    crate::paramset::paramset_add_int(
        scip,
        &format!("conflict/{name}/priority"),
        &format!("priority of conflict handler <{name}>"),
        crate::field_target!(i32, conflicthdlrs, h, priority),
        true,
        priority,
        i32::MIN,
        i32::MAX,
        Some(param_chgd_conflicthdlr_priority),
    )?;
    Ok(h)
}

/// `SCIPconflicthdlrFree`: calls destructor and frees memory of conflict
/// handler.
pub fn conflicthdlr_free(scip: &mut Scip, h: ConflicthdlrId) -> ScipResult {
    // Call destructor of conflict.
    if let Some(free) = scip.conflicthdlrs[h].conflictfree {
        free(scip, h)?;
    }
    scip.conflicthdlrs.remove(h);
    Ok(())
}

/// Runs one of the conflict handler's setup callbacks under its setup
/// clock.
fn conflicthdlr_setup_call(scip: &mut Scip, h: ConflicthdlrId, method: Option<ConflicthdlrMethod>) -> ScipResult {
    if let Some(method) = method {
        let enabled = scip.set.time_enabled;
        // Start timing.
        scip.conflicthdlrs[h].setuptime.start(enabled);
        method(scip, h)?;
        // Stop timing.
        scip.conflicthdlrs[h].setuptime.stop(enabled);
    }
    Ok(())
}

/// `SCIPconflicthdlrInit`: calls init method of conflict handler.
pub fn conflicthdlr_init(scip: &mut Scip, h: ConflicthdlrId) -> ScipResult {
    if scip.conflicthdlrs[h].initialized {
        log::error!("conflict handler <{}> already initialized", scip.conflicthdlrs[h].name);
        return Err(Retcode::InvalidCall);
    }

    if scip.set.misc_resetstat {
        let c = &mut scip.conflicthdlrs[h];
        c.setuptime.reset();
        c.conflicttime.reset();
    }

    // Call initialization method of conflict handler.
    let init = scip.conflicthdlrs[h].conflictinit;
    conflicthdlr_setup_call(scip, h, init)?;
    scip.conflicthdlrs[h].initialized = true;
    Ok(())
}

/// `SCIPconflicthdlrExit`: calls exit method of conflict handler.
pub fn conflicthdlr_exit(scip: &mut Scip, h: ConflicthdlrId) -> ScipResult {
    if !scip.conflicthdlrs[h].initialized {
        log::error!("conflict handler <{}> not initialized", scip.conflicthdlrs[h].name);
        return Err(Retcode::InvalidCall);
    }

    // Call deinitialization method of conflict handler.
    let exit = scip.conflicthdlrs[h].conflictexit;
    conflicthdlr_setup_call(scip, h, exit)?;
    scip.conflicthdlrs[h].initialized = false;
    Ok(())
}

/// `SCIPconflicthdlrInitsol`: informs conflict handler that the branch and
/// bound process is being started.
pub fn conflicthdlr_initsol(scip: &mut Scip, h: ConflicthdlrId) -> ScipResult {
    // Call solving process initialization method of conflict handler.
    let initsol = scip.conflicthdlrs[h].conflictinitsol;
    conflicthdlr_setup_call(scip, h, initsol)
}

/// `SCIPconflicthdlrExitsol`: informs conflict handler that the branch and
/// bound process data is being freed.
pub fn conflicthdlr_exitsol(scip: &mut Scip, h: ConflicthdlrId) -> ScipResult {
    // Call solving process deinitialization method of conflict handler.
    let exitsol = scip.conflicthdlrs[h].conflictexitsol;
    conflicthdlr_setup_call(scip, h, exitsol)
}

/// `SCIPconflicthdlrExec`: calls execution method of conflict handler.
#[allow(clippy::too_many_arguments)]
pub fn conflicthdlr_exec(
    scip: &mut Scip,
    h: ConflicthdlrId,
    node: NodeId,
    validnode: NodeId,
    bdchginfos: &[BdChgInfoRef],
    relaxedbds: &[f64],
    conftype: ConfType,
    usescutoffbound: bool,
    resolved: bool,
) -> ScipResult<ScipResultCode> {
    use ScipResultCode as R;

    // Call solution start method of conflict handler.
    let mut result = R::DidNotRun;
    if let Some(exec) = scip.conflicthdlrs[h].conflictexec {
        // Start timing.
        let enabled = scip.set.time_enabled;
        scip.conflicthdlrs[h].conflicttime.start(enabled);

        let args = ConflictExecArgs {
            node,
            validnode,
            bdchginfos,
            relaxedbds,
            conftype,
            cutoffinvolved: usescutoffbound,
            separate: scip.set.conf_separate,
            local: scip.nodes[validnode].depth > 0,
            dynamic: scip.set.conf_dynamic,
            removable: scip.set.conf_removable,
            resolved,
        };
        exec(scip, h, &args, &mut result)?;

        // Stop timing.
        scip.conflicthdlrs[h].conflicttime.stop(enabled);

        if !matches!(result, R::ConsAdded | R::DidNotFind | R::DidNotRun) {
            log::error!(
                "execution method of conflict handler <{}> returned invalid result <{}>",
                scip.conflicthdlrs[h].name,
                result as i32
            );
            return Err(Retcode::InvalidResult);
        }
    }
    Ok(result)
}

/// `SCIPconflicthdlrSetPriority`: sets priority of conflict handler.
pub fn conflicthdlr_set_priority(scip: &mut Scip, h: ConflicthdlrId, priority: i32) {
    scip.conflicthdlrs[h].priority = priority;
    scip.set.conflicthdlrssorted = false;
}

/// `SCIPconflictGraphApplicable`: return TRUE if conflict graph analysis is
/// applicable.
pub fn conflict_graph_applicable(scip: &Scip) -> bool {
    // Check, if propagation conflict analysis is enabled.
    if !scip.set.conf_enable || !scip.set.conf_useprop {
        return false;
    }

    // Check, if there are any conflict handlers to use a conflict set.
    if scip.set.nconflicthdlrs == 0 {
        return false;
    }

    true
}

/// `conflictCreateTmpBdchginfo`: creates a temporary bound change
/// information object that is destroyed after the conflict sets are
/// flushed.
pub fn conflict_create_tmp_bdchginfo(scip: &mut Scip, var: VarId, boundtype: BoundType, oldbound: f64, newbound: f64) -> BdChgInfoRef {
    let c = &mut cf!(scip);
    let n = c.ntmpbdchginfos;
    if n + 1 > c.tmpbdchginfossize {
        let newsize = scip.set.calc_mem_grow_size(n + 1);
        let c = &mut cf!(scip);
        c.tmpbdchginfos.realloc(newsize);
        c.tmpbdchginfossize = newsize;
    }

    // SCIPbdchginfoCreate.
    let mut info = BdChgInfo {
        oldbound,
        newbound,
        var,
        boundtype,
        inferboundtype: boundtype,
        ..Default::default()
    };
    info.inferencedata.var = var;
    info.inferencedata.reason.prop = NONE;
    info.inferencedata.reason.cons = NONE;
    info.inferencedata.info = 0;
    info.bdchgidx.depth = i32::MAX;
    info.bdchgidx.pos = -1;
    info.pos = 0;
    info.boundchgtype = crate::types::BoundchgType::Branching;
    info.redundant = false;

    let c = &mut cf!(scip);
    c.tmpbdchginfos[n] = info;
    c.ntmpbdchginfos += 1;
    BdChgInfoRef::Tmp(n as usize)
}

/// `conflictFreeTmpBdchginfos`: frees all temporarily created bound change
/// information data.
fn conflict_free_tmp_bdchginfos(scip: &mut Scip) {
    cf!(scip).ntmpbdchginfos = 0;
}

/// `incVSIDS`: increases the conflict score of the variable in the given
/// direction.
fn inc_vsids(scip: &mut Scip, var: VarId, boundtype: BoundType, value: f64, mut weight: f64) -> ScipResult {
    // Weight the VSIDS by the given weight.
    weight *= scip.stat.vsidsweight;

    if scip.set.is_zero(weight) {
        return Ok(());
    }

    let branchdir = if boundtype == BoundType::Lower { BranchDir::Upwards } else { BranchDir::Downwards };
    crate::var::var_inc_vsids(scip, var, branchdir, value, weight)?;
    scip.stat.glbhistory.inc_vsids(branchdir, weight);
    scip.stat.glbhistorycrun.inc_vsids(branchdir, weight);
    Ok(())
}

/// `updateStatistics`: update conflict statistics.
fn update_statistics(scip: &mut Scip, cs: &ConflictSet, insertdepth: i32) -> ScipResult {
    if insertdepth > 0 {
        cf!(scip).nappliedlocconss += 1;
        cf!(scip).nappliedlocliterals += cs.nbdchginfos as i64;
    } else {
        let conflictlength = cs.nbdchginfos;
        for i in 0..conflictlength {
            let info = cs.bdchginfos[i];
            let var = bdchginfo_get_var(scip, info);
            let boundtype = bdchginfo_get_boundtype(scip, info);
            let bound = cs.relaxedbds[i];

            let branchdir = if boundtype == BoundType::Lower { BranchDir::Upwards } else { BranchDir::Downwards };

            crate::var::var_inc_n_active_conflicts(scip, var, branchdir, bound, conflictlength as f64)?;
            scip.stat.glbhistory.inc_n_active_conflicts(branchdir, conflictlength as f64);
            scip.stat.glbhistorycrun.inc_n_active_conflicts(branchdir, conflictlength as f64);

            // Each variable which is part of the conflict gets an increase in
            // the VSIDS.
            let w = scip.set.conf_conflictweight;
            inc_vsids(scip, var, boundtype, bound, w)?;
        }
        cf!(scip).nappliedglbconss += 1;
        cf!(scip).nappliedglbliterals += cs.nbdchginfos as i64;
    }
    Ok(())
}

/// `SCIPsetSortConflicthdlrs`: sorts conflict handlers by priorities.
pub fn set_sort_conflicthdlrs(scip: &mut Scip) {
    if !scip.set.conflicthdlrssorted {
        let n = scip.set.nconflicthdlrs as usize;
        let mut hs = std::mem::take(&mut scip.set.conflicthdlrs);
        crate::sort::sort(&mut hs.0[..n], (), |&a, &b| conflicthdlr_comp(scip, a, b));
        scip.set.conflicthdlrs = hs;
        scip.set.conflicthdlrssorted = true;
        scip.set.conflicthdlrsnamesorted = false;
    }
}

/// `conflictAddConflictCons`: adds the given conflict set as conflict
/// constraint to the problem; returns whether it was added.
fn conflict_add_conflict_cons(scip: &mut Scip, cs: &mut ConflictSet, insertdepth: i32, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult<bool> {
    let mut success = false;

    // Try to derive global bound changes and shorten the conflictset by using
    // implication and clique and variable bound information.
    if cs.nbdchginfos > 1 && insertdepth == 0 && !scip.lp.as_ref().unwrap().strongbranching {
        let enabled = scip.set.time_enabled;
        cf!(scip).dibclock.start(enabled);

        // Find global bound changes which can be derived from the new conflict
        // set.
        let ib = detect_implied_bounds(scip, cs, transprob, origprob, eventfilter)?;

        // All variables where removed, we have an infeasibility proof.
        if cs.nbdchginfos == 0 {
            return Ok(success);
        }

        cf!(scip).dibclock.stop(enabled);

        if ib.redundant {
            if ib.nbdchgs > 0 {
                success = true;
            }
            return Ok(success);
        }
    }

    // In case the conflict set contains only one bound change which is
    // globally valid we apply that bound change directly (except if we are
    // in strong branching or diving - in this case a bound change would
    // yield an unflushed LP and is not handled when restoring the
    // information).
    //
    // A bound change can only be applied if it is are related to the active
    // node or if is a global bound change. Bound changes which are related
    // to any other node cannot be handled at point due to the internal data
    // structure.
    let lp = scip.lp.as_ref().unwrap();
    if cs.nbdchginfos == 1 && insertdepth == 0 && !lp.strongbranching && !lp.diving {
        let info = cs.bdchginfos[0_usize];
        let var = bdchginfo_get_var(scip, info);
        let boundtype = if bdchginfo_get_boundtype(scip, info) == BoundType::Lower {
            BoundType::Upper
        } else {
            BoundType::Lower
        };
        let mut bound = cs.relaxedbds[0_usize];

        // For continuous variables, we can only use the relaxed version of the
        // bounds negation: !(x <= u) -> x >= u.
        if crate::var::var_is_integral(scip, var) {
            bound += if boundtype == BoundType::Lower { 1.0 } else { -1.0 };
        }

        let node = scip.tree.as_ref().unwrap().path[cs.validdepth];
        crate::tree::node_add_boundchg(scip, node, transprob, origprob, eventfilter, var, bound, boundtype, false)?;

        success = true;
        update_statistics(scip, cs, insertdepth)?;
    } else if !cs.hasrelaxonlyvar {
        // Sort conflict handlers by priority.
        set_sort_conflicthdlrs(scip);

        // Call conflict handlers to create a conflict constraint.
        for h in 0..scip.set.nconflicthdlrs {
            let hd = scip.set.conflicthdlrs[h];
            let (node, validnode) = {
                let tree = scip.tree.as_ref().unwrap();
                (tree.path[insertdepth], tree.path[cs.validdepth])
            };
            let n = cs.nbdchginfos;
            let result = conflicthdlr_exec(
                scip,
                hd,
                node,
                validnode,
                cs.bdchginfos.head(n),
                cs.relaxedbds.head(n),
                cs.conflicttype,
                cs.usescutoffbound,
                success,
            )?;
            if result == ScipResultCode::ConsAdded {
                success = true;
                update_statistics(scip, cs, insertdepth)?;
            }
        }
    } else {
        // Would be nice to still create a constraint?, if we can make sure that
        // we the constraint does not survive a restart.
    }
    Ok(success)
}

/// `conflictCalcMaxsize`: calculates the maximal size of conflict sets to
/// be used.
pub fn conflict_calc_maxsize(scip: &Scip, prob: usize) -> i32 {
    let p = &scip.probs[prob];
    let maxsize = (scip.set.conf_maxvarsfac * (p.nvars - p.ncontvars) as f64) as i32;
    crate::def::max(maxsize, scip.set.conf_minmaxvars)
}

/// `SCIPconflictFlushConss`: adds the collected conflict constraints to the
/// corresponding nodes; the best set->conf_maxconss conflict constraints
/// are added to the node of their validdepth; additionally (if not yet
/// added, and if repropagation is activated), the conflict constraint that
/// triggers the earliest repropagation is added to the node of its
/// validdepth.
pub fn conflict_flush_conss(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult {
    // Is there anything to do?
    if cf!(scip).nconflictsets > 0 {
        // Calculate the maximal number of conflict sets to accept, and the
        // maximal size of each accepted conflict set.
        let maxconflictsets = if scip.set.conf_maxconss == -1 { i32::MAX } else { scip.set.conf_maxconss };
        let maxsize = conflict_calc_maxsize(scip, transprob);

        let path = |scip: &Scip, d: i32| scip.tree.as_ref().unwrap().path[d];

        // Insert the conflict sets at the corresponding nodes.
        let mut nconflictsetsused = 0;
        let mut cutoffdepth = i32::MAX;
        let mut repropdepth = i32::MAX;
        let mut repropconflictset: Option<usize> = None;
        let mut sets = std::mem::take(&mut cf!(scip).conflictsets);
        let nsets = cf!(scip).nconflictsets;
        let mut i = 0;
        while i < nsets && nconflictsetsused < maxconflictsets {
            let cs = &mut sets[i as usize];
            i += 1;

            // Ignore conflict sets that are only valid at a node that was
            // already cut off.
            if cs.insertdepth >= cutoffdepth {
                continue;
            }

            // If no conflict bounds exist, the node and its sub tree in the
            // conflict set's valid depth can be cut off completely.
            if cs.nbdchginfos == 0 {
                let node = path(scip, cs.validdepth);
                crate::tree::node_cutoff(scip, node, eventfilter, transprob, origprob)?;
                cutoffdepth = cs.validdepth;
                continue;
            }

            // If the conflict set is too long, use the conflict set only if it
            // decreases the repropagation depth.
            if cs.nbdchginfos > maxsize {
                if scip.set.conf_keepreprop && cs.repropagate && cs.repropdepth < repropdepth {
                    repropdepth = cs.repropdepth;
                    repropconflictset = Some((i - 1) as usize);
                }
            } else {
                // Call conflict handlers to create a conflict constraint.
                let insertdepth = cs.insertdepth;
                let success = conflict_add_conflict_cons(scip, cs, insertdepth, transprob, origprob, eventfilter)?;

                // If no conflict bounds exist, the node and its sub tree in the
                // conflict set's valid depth can be cut off completely.
                if cs.nbdchginfos == 0 {
                    let node = path(scip, cs.validdepth);
                    crate::tree::node_cutoff(scip, node, eventfilter, transprob, origprob)?;
                    cutoffdepth = cs.validdepth;
                    continue;
                }

                if success {
                    if cs.repropagate && cs.repropdepth <= repropdepth {
                        repropdepth = cs.repropdepth;
                        repropconflictset = None;
                    }
                    nconflictsetsused += 1;
                }
            }
        }

        // Reactivate propagation on the first node where one of the new
        // conflict sets trigger a deduction.
        if scip.set.conf_repropagate && repropdepth < cutoffdepth && repropdepth < scip.tree.as_ref().unwrap().pathlen {
            // If the conflict constraint of smallest repropagation depth was not
            // yet added, insert it now.
            if let Some(k) = repropconflictset {
                let cs = &mut sets[k];
                conflict_add_conflict_cons(scip, cs, repropdepth, transprob, origprob, eventfilter)?;

                // If no conflict bounds exist, the node and its sub tree in the
                // conflict set's valid depth can be cut off completely.
                if cs.nbdchginfos == 0 {
                    let node = path(scip, cs.validdepth);
                    crate::tree::node_cutoff(scip, node, eventfilter, transprob, origprob)?;
                }
            }

            // Mark the node in the repropdepth to be propagated again.
            let node = path(scip, repropdepth);
            crate::tree::node_propagate_again(scip, node);
        }

        // Free the conflict store.
        sets.clear();
        cf!(scip).conflictsets = sets;
        cf!(scip).nconflictsets = 0;
    }

    // Free all temporarily created bound change information data.
    conflict_free_tmp_bdchginfos(scip);
    Ok(())
}

/// `conflictInsertConflictset`: inserts conflict set into sorted
/// conflictsets array and deletes the conflict set pointer.
fn conflict_insert_conflictset(scip: &mut Scip, mut cs: Box<ConflictSet>) {
    // Calculate conflict and repropagation depth.
    conflictset_calc_conflict_depth(scip, &mut cs);

    // If we apply repropagations, the conflict set should be inserted at most
    // at its repropdepth.
    if scip.set.conf_repropagate {
        cs.insertdepth = cs.insertdepth.min(cs.repropdepth);
    } else {
        cs.repropdepth = i32::MAX;
    }

    // Get the score of the conflict set.
    let score = conflictset_calc_score(scip, &cs);

    // Check, if conflict set is redundant to a better conflict set.
    let c = scip.conflict.as_ref().unwrap();
    let mut pos = 0;
    while pos < c.nconflictsets && score < c.conflictsetscores[pos] {
        // Check if conflict set is redundant with respect to
        // conflictsets[pos].
        if conflictset_is_redundant(scip, &cs, &c.conflictsets[pos as usize]) {
            return;
        }
        pos += 1;
    }

    // Insert conflictset into the sorted conflictsets array.
    let n = c.nconflictsets;
    if n + 1 > c.conflictsetssize {
        let newsize = scip.set.calc_mem_grow_size(n + 1);
        let c = &mut cf!(scip);
        c.conflictsetscores.realloc(newsize);
        c.conflictsetssize = newsize;
    }
    let c = &mut cf!(scip);
    c.conflictsets.insert(pos as usize, cs);
    let mut i = c.nconflictsets;
    while i > pos {
        c.conflictsetscores[i] = c.conflictsetscores[i - 1];
        i -= 1;
    }
    c.conflictsetscores[pos] = score;
    c.nconflictsets += 1;

    // Remove worse conflictsets that are redundant to the new conflictset.
    let mut sets = std::mem::take(&mut cf!(scip).conflictsets);
    let mut scores = std::mem::take(&mut cf!(scip).conflictsetscores);
    let nsets = cf!(scip).nconflictsets;
    let mut kept: Vec<Box<ConflictSet>> = Vec::with_capacity(sets.len());
    let mut j = pos + 1;
    for (i, s) in sets.drain(..).enumerate() {
        let i = i as i32;
        if i <= pos || i >= nsets {
            kept.push(s);
            continue;
        }
        if conflictset_is_redundant(scip, &s, &kept[pos as usize]) {
            // SCIPconflictsetFree.
        } else {
            scores[j] = scores[i];
            j += 1;
            kept.push(s);
        }
    }
    let c = &mut cf!(scip);
    c.conflictsets = kept;
    c.conflictsetscores = scores;
    c.nconflictsets = j;
}

/// `conflictMarkBoundCheckPresence`: marks bound to be present in the
/// current conflict and returns whether a bound which is at least as tight
/// was already member of the current conflict (i.e., the given bound change
/// does not need to be added).
fn conflict_mark_bound_check_presence(scip: &mut Scip, info: BdChgInfoRef, relaxedbd: f64) -> bool {
    let var = bdchginfo_get_var(scip, info);
    let newbound = bdchginfo_get_newbound(scip, info);
    let boundtype = bdchginfo_get_boundtype(scip, info);
    let count = scip.conflict.as_ref().unwrap().count;
    let v = &mut scip.vars[var];

    match boundtype {
        BoundType::Lower => {
            // Check if the variables lower bound is already member of the
            // conflict.
            if v.conflictlbcount == count {
                // The variable is already member of the conflict; hence check
                // if the new bound is redundant.
                if v.conflictlb > newbound {
                    return true;
                } else if v.conflictlb == newbound {
                    v.conflictrelaxedlb = crate::def::max(v.conflictrelaxedlb, relaxedbd);
                    return true;
                }
            }

            // Add the variable lower bound to the current conflict.
            v.conflictlbcount = count;

            // Remember the lower bound and relaxed bound to allow only
            // better/tighter lower bounds for that variables w.r.t. this
            // conflict.
            v.conflictlb = newbound;
            v.conflictrelaxedlb = relaxedbd;
            false
        }
        BoundType::Upper => {
            // Check if the variables upper bound is already member of the
            // conflict.
            if v.conflictubcount == count {
                // The variable is already member of the conflict; hence check
                // if the new bound is redundant.
                if v.conflictub < newbound {
                    return true;
                } else if v.conflictub == newbound {
                    v.conflictrelaxedub = crate::def::min(v.conflictrelaxedub, relaxedbd);
                    return true;
                }
            }

            // Add the variable upper bound to the current conflict.
            v.conflictubcount = count;

            // Remember the upper bound and relaxed bound to allow only
            // better/tighter upper bounds for that variables w.r.t. this
            // conflict.
            v.conflictub = newbound;
            v.conflictrelaxedub = relaxedbd;
            false
        }
    }
}

/// `conflictAddConflictBound`: puts bound change into the current conflict
/// set.
fn conflict_add_conflict_bound(scip: &mut Scip, info: BdChgInfoRef, relaxedbd: f64) {
    // Mark the bound to be member of the conflict and check if a bound which
    // is at least as tight is already member of the conflict.
    if !conflict_mark_bound_check_presence(scip, info, relaxedbd) {
        // Add the bound change to the current conflict set.
        let mut cs = std::mem::take(&mut cf!(scip).conflictset);
        conflictset_add_bound(scip, &mut cs, info, relaxedbd);
        cf!(scip).conflictset = cs;
    }
}

/// `isBoundchgUseless`: returns whether the negation of the given bound
/// change would lead to a globally valid literal.
fn is_boundchg_useless(scip: &Scip, info: BdChgInfoRef) -> bool {
    let var = bdchginfo_get_var(scip, info);
    let boundtype = bdchginfo_get_boundtype(scip, info);
    let bound = bdchginfo_get_newbound(scip, info);

    !crate::var::var_is_integral(scip, var)
        && ((boundtype == BoundType::Lower && scip.set.is_feas_ge(bound, scip.vars[var].glbdom.ub))
            || (boundtype == BoundType::Upper && scip.set.is_feas_le(bound, scip.vars[var].glbdom.lb)))
}

/// `conflictBdchginfoComp`: the conflict queues' comparator; later bound
/// changes first.
pub fn conflict_bdchginfo_comp(scip: &Scip, a: BdChgInfoRef, b: BdChgInfoRef) -> i32 {
    if a == b {
        return 0;
    }
    if !crate::var::bdchgidx_is_earlier_non_null(&bdchginfo(scip, a).bdchgidx, &bdchginfo(scip, b).bdchgidx) {
        -1
    } else {
        1
    }
}

/// Which queue of the conflict a bound change goes to.
#[derive(Clone, Copy)]
enum Queue {
    Bdchg,
    Forced,
}

/// `SCIPpqueueInsert` on one of the conflict's queues.
fn queue_insert(scip: &mut Scip, q: Queue, info: BdChgInfoRef) {
    let mut queue = match q {
        Queue::Bdchg => std::mem::take(&mut cf!(scip).bdchgqueue),
        Queue::Forced => std::mem::take(&mut cf!(scip).forcedbdchgqueue),
    };
    queue.insert(info, |&a, &b| conflict_bdchginfo_comp(scip, a, b), |_, _, _| {});
    match q {
        Queue::Bdchg => cf!(scip).bdchgqueue = queue,
        Queue::Forced => cf!(scip).forcedbdchgqueue = queue,
    }
}

/// `SCIPpqueueRemove` on one of the conflict's queues.
fn queue_remove(scip: &mut Scip, q: Queue) -> Option<BdChgInfoRef> {
    let mut queue = match q {
        Queue::Bdchg => std::mem::take(&mut cf!(scip).bdchgqueue),
        Queue::Forced => std::mem::take(&mut cf!(scip).forcedbdchgqueue),
    };
    let r = queue.remove(|&a, &b| conflict_bdchginfo_comp(scip, a, b), |_, _, _| {});
    match q {
        Queue::Bdchg => cf!(scip).bdchgqueue = queue,
        Queue::Forced => cf!(scip).forcedbdchgqueue = queue,
    }
    r
}

/// `conflictQueueBound`: adds given bound change information to the
/// conflict candidate queue; returns whether it was queued.
fn conflict_queue_bound(scip: &mut Scip, info: BdChgInfoRef, relaxedbd: f64) -> bool {
    let mut success = false;

    if scip.set.conf_usegenres && !cf!(scip).bdchgonlyconfqueue {
        unimplemented!("generalized resolution conflict analysis");
    }

    // Mark the bound to be member of the conflict and check if a bound which
    // is at least as tight is already member of the conflict.
    if scip.set.conf_useprop && !conflict_mark_bound_check_presence(scip, info, relaxedbd) {
        // Insert the bound change into the conflict queue.
        let var = bdchginfo_get_var(scip, info);
        if (!scip.set.conf_preferbinary || crate::var::var_is_binary(scip, var)) && !is_boundchg_useless(scip, info) {
            queue_insert(scip, Queue::Bdchg, info);
        } else {
            queue_insert(scip, Queue::Forced, info);
        }
        success = true;
    }
    success
}

/// `conflictAddBound`: adds variable's bound to conflict candidate queue.
fn conflict_add_bound(scip: &mut Scip, var: VarId, boundtype: BoundType, info: BdChgInfoRef, relaxedbd: f64) -> ScipResult {
    // The local bound change may be resolved and has to be put on the
    // candidate queue; we even put bound changes without inference
    // information on the queue in order to automatically eliminate multiple
    // insertions of the same bound change.

    // Put bound change information into priority queue.
    conflict_queue_bound(scip, info, relaxedbd);

    // Each variable which is add to the conflict graph gets an increase in
    // the VSIDS; that is different to the VSIDS presented in the literature.
    let w = scip.set.conf_conflictgraphweight;
    inc_vsids(scip, var, boundtype, relaxedbd, w)
}

/// `bdchginfoIsInvalid`: check if the bound change info (which is the
/// potential next candidate which is queued) is valid for the current
/// conflict analysis; a bound change info can get invalid if after this
/// one was added to the queue, a weaker bound change was added to the
/// queue (due the bound widening idea) which immediately makes this bound
/// change redundant; due to the priority we did not removed that bound
/// change info since that cost O(log(n)); hence we have to skip/ignore it
/// now.
pub fn bdchginfo_is_invalid(scip: &Scip, info: BdChgInfoRef) -> bool {
    let var = bdchginfo_get_var(scip, info);
    let count = scip.conflict.as_ref().unwrap().count;

    // The bound change info of a binary (domained) variable can never be
    // invalid since the concepts of relaxed bounds and bound widening do not
    // make sense for these type of variables.
    if crate::var::var_is_binary(scip, var) {
        return false;
    }

    // Check if the bdchginfo is invalid since a tight/weaker bound change was
    // already explained.
    let v = &scip.vars[var];
    let newbound = bdchginfo_get_newbound(scip, info);
    if bdchginfo_get_boundtype(scip, info) == BoundType::Lower {
        v.conflictlbcount != count || v.conflictlb != newbound
    } else {
        v.conflictubcount != count || v.conflictub != newbound
    }
}

/// `conflictsetAddBounds`: adds given bound changes to a conflict set.
fn conflictset_add_bounds(scip: &Scip, cs: &mut ConflictSet, infos: &[BdChgInfoRef]) {
    let nbdchginfos = infos.len() as i32;

    // Nothing to add.
    if nbdchginfos == 0 {
        return;
    }

    // Only one element to add, use the single insertion method.
    if nbdchginfos == 1 {
        let info = infos[0];
        if !bdchginfo_is_invalid(scip, info) {
            let relaxed = crate::var::bdchginfo_get_relaxed_bound(scip, bdchginfo(scip, info));
            conflictset_add_bound(scip, cs, info, relaxed);
        }
        return;
    }

    let mut confnbdchginfos = cs.nbdchginfos;

    // Allocate memory for additional element.
    conflictset_ensure_bdchginfos_mem(scip, cs, confnbdchginfos + nbdchginfos);

    for &info in infos {
        // Add only valid bound change infos.
        if !bdchginfo_is_invalid(scip, info) {
            // Calculate sorting value.
            let boundtype = bdchginfo_get_boundtype(scip, info);
            let var = bdchginfo_get_var(scip, info);
            let idx = scip.vars[var].index;
            // First sorting criteria: variable index, second criteria:
            // boundtype.
            let sortval = 2 * idx + boundtype as i32;

            // Add new element.
            cs.bdchginfos[confnbdchginfos] = info;
            cs.relaxedbds[confnbdchginfos] = crate::var::bdchginfo_get_relaxed_bound(scip, bdchginfo(scip, info));
            cs.sortvals[confnbdchginfos] = sortval;
            confnbdchginfos += 1;

            if scip.vars[var].relaxationonly {
                cs.hasrelaxonlyvar = true;
            }
        }
    }

    // Sort and merge the new conflict set.
    if confnbdchginfos > cs.nbdchginfos {
        let n = confnbdchginfos as usize;
        {
            let ConflictSet {
                sortvals, bdchginfos, relaxedbds, ..
            } = cs;
            crate::sort::sort(&mut sortvals.0[..n], (&mut bdchginfos.0[..n], &mut relaxedbds.0[..n]), crate::sort::cmp_int);
        }

        let mut k = 0;
        let mut i = 1;

        // Merge multiple bound changes.
        while i < confnbdchginfos {
            // Is this a multiple bound change.
            if cs.sortvals[k] == cs.sortvals[i] {
                if bdchginfo_is_tighter(scip, cs.bdchginfos[k], cs.bdchginfos[i]) {
                    i += 1;
                } else if bdchginfo_is_tighter(scip, cs.bdchginfos[i], cs.bdchginfos[k]) {
                    // Replace worse bound change info by tighter bound change
                    // info.
                    cs.bdchginfos[k] = cs.bdchginfos[i];
                    cs.relaxedbds[k] = cs.relaxedbds[i];
                    cs.sortvals[k] = cs.sortvals[i];
                    i += 1;
                } else {
                    // Both bound change are equivalent; hence, keep the worse
                    // relaxed bound and remove one of them.
                    let (rk, ri) = (cs.relaxedbds[k], cs.relaxedbds[i]);
                    cs.relaxedbds[k] = if cs.sortvals[k] % 2 == 0 { crate::def::max(rk, ri) } else { crate::def::min(rk, ri) };
                    i += 1;
                }
            } else {
                // All bound change infos must be valid.
                k += 1;

                // Move next comparison element to the correct position.
                if k != i {
                    cs.bdchginfos[k] = cs.bdchginfos[i];
                    cs.relaxedbds[k] = cs.relaxedbds[i];
                    cs.sortvals[k] = cs.sortvals[i];
                }
                i += 1;
            }
        }

        // The number of bound change infos cannot be decreased, it would mean
        // that the conflict set was not merged before.
        cs.nbdchginfos = k + 1;
    }
}

/// `conflictRemoveCand`: removes and returns next conflict analysis
/// candidate from the candidate queue.
fn conflict_remove_cand(scip: &mut Scip) -> BdChgInfoRef {
    let info = if cf!(scip).forcedbdchgqueue.n_elems() > 0 {
        queue_remove(scip, Queue::Forced)
    } else {
        queue_remove(scip, Queue::Bdchg)
    }
    .expect("a candidate");

    // If we have a candidate this one should be valid for the current
    // conflict analysis.

    // Mark the bound change to be no longer in the conflict (it will be either
    // added again to the conflict set or replaced by resolving, which might
    // add a weaker change on the same bound to the queue).
    let var = bdchginfo_get_var(scip, info);
    let lower = bdchginfo_get_boundtype(scip, info) == BoundType::Lower;
    let v = &mut scip.vars[var];
    if lower {
        v.conflictlbcount = 0;
        v.conflictrelaxedlb = crate::def::REAL_MIN;
    } else {
        v.conflictubcount = 0;
        v.conflictrelaxedub = crate::def::REAL_MAX;
    }
    info
}

/// `conflictFirstCand`: returns next conflict analysis candidate from the
/// candidate queue without removing it.
fn conflict_first_cand(scip: &mut Scip) -> Option<BdChgInfoRef> {
    if cf!(scip).forcedbdchgqueue.n_elems() > 0 {
        // Get next potential candidate.
        let info = cf!(scip).forcedbdchgqueue.first().unwrap();

        // Check if this candidate is valid.
        if bdchginfo_is_invalid(scip, info) {
            // Pop the invalid bound change info from the queue.
            queue_remove(scip, Queue::Forced);

            // Call method recursively to get next conflict analysis candidate.
            return conflict_first_cand(scip);
        }
        Some(info)
    } else {
        let info = cf!(scip).bdchgqueue.first();

        // Check if this candidate is valid.
        if let Some(i) = info
            && bdchginfo_is_invalid(scip, i)
        {
            // Pop the invalid bound change info from the queue.
            queue_remove(scip, Queue::Bdchg);

            // Call method recursively to get next conflict analysis candidate.
            return conflict_first_cand(scip);
        }
        info
    }
}

/// `conflictAddConflictset`: adds the current conflict set (extended by all
/// remaining bound changes in the queue) to the pool of conflict sets;
/// returns `(success, nliterals)`.
fn conflict_add_conflictset(scip: &mut Scip, validdepth: i32, diving: bool, repropagate: bool) -> (bool, i32) {
    let mut success = false;
    let mut nliterals = 0;

    // Check, whether local conflicts are allowed.
    let validdepth = crate::def::max(validdepth, cf!(scip).conflictset.validdepth);
    if !scip.set.conf_allowlocal && validdepth > 0 {
        return (success, nliterals);
    }

    let focusdepth = crate::tree::tree_get_focus_depth(scip);
    let currentdepth = crate::tree::tree_get_current_depth(scip);

    // Get the elements of the bound change queue.
    let infos: Vec<BdChgInfoRef> = cf!(scip).bdchgqueue.elems().to_vec();

    // Create a copy of the current conflict set, allocating memory for the
    // additional elements of the queue.
    let mut cs = conflictset_copy(&cf!(scip).conflictset, infos.len() as i32);
    cs.validdepth = validdepth;
    cs.repropagate = repropagate;

    // Add the valid queue elements to the conflict set.
    conflictset_add_bounds(scip, &mut cs, &infos);

    // Calculate the depth, at which the conflictset should be inserted.
    conflictset_calc_insert_depth(scip, &mut cs);

    // If all branching variables are in the conflict set, the conflict set is
    // of no use; don't use conflict sets that are only valid in the probing
    // path but not in the problem tree.
    if (diving || cs.insertdepth < currentdepth) && cs.insertdepth <= focusdepth {
        // If the conflict should not be located only in the subtree where it
        // is useful, put it to its valid depth level.
        if !scip.set.conf_settlelocal {
            cs.insertdepth = cs.validdepth;
        }

        nliterals = cs.nbdchginfos;

        // Move conflictset to the conflictset storage.
        conflict_insert_conflictset(scip, cs);
        success = true;
    }
    (success, nliterals)
}

/// `conflictResolveBound`: tries to resolve given bound change
///  - resolutions on local constraints are only applied, if the constraint
///    is valid at the current minimal valid depth level, because this depth
///    level is the topmost level to add the conflict constraint to anyways
///
/// It is sufficient to explain the relaxed bound change.
fn conflict_resolve_bound(scip: &mut Scip, info: BdChgInfoRef, mut relaxedbd: f64, validdepth: i32) -> ScipResult<bool> {
    use crate::types::BoundchgType;
    let mut resolved = false;

    let b = *bdchginfo(scip, info);
    let actvar = b.var;

    // Check, if the bound change can and should be resolved:
    //  - resolutions on local constraints should only be applied, if the
    //    constraint is valid at the current minimal valid depth level (which
    //    is initialized with the valid depth level of the initial conflict
    //    set), because this depth level is the topmost level to add the
    //    conflict constraint to anyways.
    match b.boundchgtype {
        BoundchgType::ConsInfer => {
            let infercons = b.inferencedata.reason.cons;
            let c = &scip.conss[infercons];
            if c.is_global() || c.get_valid_depth() <= validdepth {
                // Resolve bound change by asking the constraint that inferred
                // the bound to put all bounds that were the reasons for the
                // conflicting bound change on the priority queue.
                let infervar = b.inferencedata.var;
                let inferinfo = b.inferencedata.info;
                let inferboundtype = b.inferboundtype;
                let bdchgidx = b.bdchgidx;

                // In case the inference variables is not an active variables,
                // we need to transform the relaxed bound.
                if actvar != infervar {
                    let mut scalar = 1.0;
                    let mut constant = 0.0;
                    let mut var = infervar;

                    // Transform given variable to active variable.
                    crate::var::var_get_probvar_sum(scip, &mut var, &mut scalar, &mut constant)?;

                    relaxedbd *= scalar;
                    relaxedbd += constant;
                }

                let result = crate::cons::cons_resolve_propagation(scip, infercons, infervar, inferinfo, inferboundtype, Some(bdchgidx), relaxedbd)?;
                resolved = result == ScipResultCode::Success;
            }
        }
        BoundchgType::PropInfer => {
            let inferprop = b.inferencedata.reason.prop;
            if inferprop != NONE {
                // Resolve bound change by asking the propagator that inferred
                // the bound to put all bounds that were the reasons for the
                // conflicting bound change on the priority queue.
                let infervar = b.inferencedata.var;
                let inferinfo = b.inferencedata.info;
                let inferboundtype = b.inferboundtype;
                let bdchgidx = b.bdchgidx;

                let result = crate::prop::prop_resolve_propagation(scip, inferprop, infervar, inferinfo, inferboundtype, Some(bdchgidx), relaxedbd)?;
                resolved = result == ScipResultCode::Success;
            }
        }
        BoundchgType::Branching => {}
    }
    Ok(resolved)
}

/// `conflictClear`: clears the conflict queue and the current conflict set.
fn conflict_clear(c: &mut Conflict) {
    c.bdchgqueue.clear();
    c.forcedbdchgqueue.clear();
    conflictset_clear(&mut c.conflictset);
}

/// `SCIPconflictInit`: initializes propagation and resolution conflict
/// analysis by clearing the conflict candidate queues.
pub fn conflict_init(scip: &mut Scip, prob: usize, conftype: ConfType, usescutoffbound: bool) -> ScipResult {
    // Clear the conflict candidate queue and the conflict set.
    conflict_clear(&mut cf!(scip));

    // Clear the resolution conflict analysis queues and the bounds leading to
    // conflict.
    if scip.set.conf_usegenres {
        unimplemented!("generalized resolution conflict analysis");
    }

    // Set conflict type.
    let c = &mut cf!(scip);
    c.conflictset.conflicttype = conftype;

    // Set whether a cutoff bound is involved.
    c.conflictset.usescutoffbound = usescutoffbound;

    // Increase the conflict counter, such that binary variables of new
    // conflict set and new conflict queue are labeled with this new counter.
    c.count = c.count.wrapping_add(1);
    if c.count == 0 {
        // Make sure, 0 is not a valid conflict counter (may happen due to
        // integer overflow).
        c.count = 1;
    }

    // Increase the conflict score weight for history updates of future
    // conflict reasons.
    if scip.stat.nnodes > scip.stat.lastconflictnode {
        scip.stat.vsidsweight /= scip.set.conf_scorefac;

        // If the conflict score for the next conflict exceeds 1000.0, rescale
        // all history conflict scores.
        if scip.stat.vsidsweight >= 1000.0 {
            let w = scip.stat.vsidsweight;
            for v in 0..scip.probs[prob].nvars {
                let var = scip.probs[prob].vars[v];
                crate::var::var_scale_vsids(scip, var, 1.0 / w)?;
            }
            scip.stat.glbhistory.scale_vsids(1.0 / w);
            scip.stat.glbhistorycrun.scale_vsids(1.0 / w);
            scip.stat.vsidsweight = 1.0;
        }
        scip.stat.lastconflictnode = scip.stat.nnodes;
    }
    Ok(())
}

/// `convertToActiveVar`: convert variable and bound change to active
/// variable.
fn convert_to_active_var(scip: &Scip, var: &mut VarId, boundtype: &mut BoundType, bound: Option<&mut f64>) -> ScipResult {
    let mut scalar = 1.0;
    let mut constant = 0.0;

    // Transform given variable to active variable.
    crate::var::var_get_probvar_sum(scip, var, &mut scalar, &mut constant)?;

    if scip.vars[*var].varstatus == crate::types::VarStatus::Fixed {
        return Ok(());
    }

    // If the scalar of the aggregation is negative, we have to switch the
    // bound type.
    if scalar < 0.0 {
        *boundtype = if *boundtype == BoundType::Lower { BoundType::Upper } else { BoundType::Lower };
    }

    if let Some(bound) = bound {
        *bound -= constant;
        *bound /= scalar;
    }
    Ok(())
}

/// `bdchginfoIsResolvable`: returns whether bound change has a valid
/// reason that can be resolved in conflict analysis.
fn bdchginfo_is_resolvable(scip: &Scip, info: BdChgInfoRef) -> bool {
    crate::var::bdchginfo_has_inference_reason(bdchginfo(scip, info))
}

/// `conflictCreateReconvergenceConss`: if only one conflicting bound change
/// of the last depth level was used, and if this can be resolved, creates
/// GRASP-like reconvergence conflict constraints in the conflict graph up
/// to the branching variable of this depth level; returns the number of
/// constraints and literals created.
fn conflict_create_reconvergence_conss(
    scip: &mut Scip,
    prob: usize,
    diving: bool,
    mut validdepth: i32,
    firstuip: BdChgInfoRef,
    nreconvconss: &mut i32,
    nreconvliterals: &mut i32,
) -> ScipResult {
    let focusdepth = crate::tree::tree_get_focus_depth(scip);
    let currentdepth = crate::tree::tree_get_current_depth(scip);

    // Check, whether local constraints are allowed; however, don't generate
    // reconvergence constraints that are only valid in the probing path and
    // not in the problem tree (i.e. that exceed the focusdepth).
    let maxvaliddepth = if scip.set.conf_allowlocal { (currentdepth - 1).min(focusdepth) } else { 0 };
    if validdepth > maxvaliddepth {
        return Ok(());
    }

    let firstuipdepth = bdchginfo_get_depth(scip, firstuip);

    let conftype = cf!(scip).conflictset.conflicttype;
    let usescutoffbound = cf!(scip).conflictset.usescutoffbound;

    // For each succeeding UIP pair of the last depth level, create one
    // reconvergence constraint.
    let mut uip = Some(firstuip);
    while let Some(u) = uip
        && bdchginfo_get_depth(scip, u) == bdchginfo_get_depth(scip, firstuip)
        && bdchginfo_is_resolvable(scip, u)
    {
        // Initialize conflict data.
        conflict_init(scip, prob, conftype, usescutoffbound)?;

        cf!(scip).conflictset.conflicttype = conftype;
        cf!(scip).conflictset.usescutoffbound = usescutoffbound;

        // Create a temporary bound change information for the negation of the
        // UIP's bound change; this bound change information is freed in the
        // SCIPconflictFlushConss() call; for reconvergence constraints for
        // continuous variables we can only use the "negation" !(x <= u) == (x
        // >= u); during conflict analysis, we treat a continuous bound "x >=
        // u" in the conflict set as "x > u", and in the generated constraint
        // this is negated again to "x <= u" which is correct.
        let uipvar = bdchginfo_get_var(scip, u);
        let oppositeuipboundtype = if bdchginfo_get_boundtype(scip, u) == BoundType::Lower {
            BoundType::Upper
        } else {
            BoundType::Lower
        };
        let mut oppositeuipbound = bdchginfo_get_newbound(scip, u);
        if crate::var::var_is_integral(scip, uipvar) {
            oppositeuipbound += if oppositeuipboundtype == BoundType::Lower { 1.0 } else { -1.0 };
        }
        let old = if oppositeuipboundtype == BoundType::Lower {
            crate::def::REAL_MIN
        } else {
            crate::def::REAL_MAX
        };
        let oppositeuip = conflict_create_tmp_bdchginfo(scip, uipvar, oppositeuipboundtype, old, oppositeuipbound);

        // Put the negated UIP into the conflict set.
        conflict_add_conflict_bound(scip, oppositeuip, oppositeuipbound);

        // Put positive UIP into priority queue.
        let nb = bdchginfo_get_newbound(scip, u);
        conflict_queue_bound(scip, u, nb);

        // Resolve the queue until the next UIP is reached.
        let mut info = conflict_first_cand(scip);
        let mut nextuip: Option<BdChgInfoRef> = None;
        let mut nresolutions = 0;
        while let Some(cur) = info
            && validdepth <= maxvaliddepth
        {
            // Check if the next bound change must be resolved in every case.
            let forceresolve = cf!(scip).forcedbdchgqueue.n_elems() > 0;

            // Remove currently processed candidate and get next conflicting
            // bound from the conflict candidate queue before we remove the
            // candidate we have to collect the relaxed bound since removing
            // the candidate from the queue invalidates the relaxed bound.
            let relaxedbd = crate::var::bdchginfo_get_relaxed_bound(scip, bdchginfo(scip, cur));
            let cur = conflict_remove_cand(scip);
            let nextbdchginfo = conflict_first_cand(scip);
            let bdchgdepth = bdchginfo_get_depth(scip, cur);

            // Bound changes that are higher in the tree than the valid depth
            // of the conflict can be ignored; multiple insertions of the same
            // bound change can be ignored.
            if bdchgdepth > validdepth && Some(cur) != nextbdchginfo {
                // Check if we have to resolve the bound change in this depth
                // level
                //  - the starting uip has to be resolved
                //  - a bound change should be resolved, if it is in the fuip's
                //    depth level and not the next uip (i.e., if it is not the
                //    last bound change in the fuip's depth level)
                //  - a forced bound change must be resolved in any case
                let mut resolved = false;
                if cur == u || (bdchgdepth == firstuipdepth && nextbdchginfo.is_some_and(|n| bdchginfo_get_depth(scip, n) == bdchgdepth)) || forceresolve {
                    resolved = conflict_resolve_bound(scip, cur, relaxedbd, validdepth)?;
                }

                if resolved {
                    nresolutions += 1;
                } else if forceresolve {
                    // Variable cannot enter the conflict clause: we have to
                    // make the conflict clause local, s.t. the unresolved bound
                    // change is active in the whole sub tree of the conflict
                    // clause.
                    validdepth = bdchgdepth;
                } else if cur != u {
                    // If this is the first variable of the conflict set besides
                    // the current starting UIP, it is the next UIP (or the first
                    // unresolvable bound change).
                    if bdchgdepth == firstuipdepth && cf!(scip).conflictset.nbdchginfos == 1 {
                        nextuip = Some(cur);
                    }

                    // Put bound change into the conflict set.
                    conflict_add_conflict_bound(scip, cur, relaxedbd);
                }
            }

            // Get next conflicting bound from the conflict candidate queue
            // (this does not need to be nextbdchginfo, because due to
            // resolving the bound changes, a variable could be added to the
            // queue which must be resolved before nextbdchginfo).
            info = conflict_first_cand(scip);
        }

        // If only one propagation was resolved, the reconvergence constraint
        // is already member of the constraint set (it is exactly the
        // constraint that produced the propagation).
        if nextuip.is_some() && nresolutions >= 2 && info.is_none() && validdepth <= maxvaliddepth {
            // Call the conflict handlers to create a conflict set.
            let (success, nlits) = conflict_add_conflictset(scip, validdepth, diving, false);
            if success {
                *nreconvconss += 1;
                *nreconvliterals += nlits;
            }
        }

        // Clear the conflict candidate queue and the conflict set (to make
        // sure, oppositeuip is not referenced anymore).
        conflict_clear(&mut cf!(scip));

        uip = nextuip;
    }

    cf!(scip).conflictset.conflicttype = conftype;
    cf!(scip).conflictset.usescutoffbound = usescutoffbound;
    Ok(())
}

/// The counts conflict analysis reports.
#[derive(Clone, Copy, Debug, Default)]
pub struct ConflictCounts {
    pub nconss: i32,
    pub nliterals: i32,
    pub nreconvconss: i32,
    pub nreconvliterals: i32,
}

/// `conflictAnalyze`: analyzes conflicting bound changes that were added
/// with calls to SCIPconflictAddBound() and SCIPconflictAddRelaxedBound(),
/// and on success, calls the conflict handlers to create a conflict
/// constraint out of the resulting conflict set; afterwards the conflict
/// queue and the conflict set is cleared.
pub fn conflict_analyze(scip: &mut Scip, prob: usize, diving: bool, mut validdepth: i32, mustresolve: bool) -> ScipResult<ConflictCounts> {
    let mut counts = ConflictCounts::default();

    let focusdepth = crate::tree::tree_get_focus_depth(scip);
    let currentdepth = crate::tree::tree_get_current_depth(scip);

    let fuiplevels = scip.set.conf_fuiplevels;
    let mut resolvedepth = if fuiplevels >= 0 && fuiplevels <= currentdepth {
        currentdepth - fuiplevels + 1
    } else {
        0
    };

    // If we must resolve at least one bound change, find the first UIP at
    // least in the last depth level.
    if mustresolve {
        resolvedepth = resolvedepth.min(currentdepth);
    }

    // Check, whether local conflicts are allowed; however, don't generate
    // conflict constraints that are only valid in the probing path and not in
    // the problem tree (i.e. that exceed the focusdepth).
    let maxvaliddepth = if scip.set.conf_allowlocal { (currentdepth - 1).min(focusdepth) } else { 0 };
    if validdepth > maxvaliddepth {
        return Ok(counts);
    }

    // Allocate temporary memory for storing first UIPs (in each depth level,
    // at most two bound changes can be flagged as UIP, namely a binary and a
    // non-binary bound change).
    let mut firstuips: Vec<BdChgInfoRef> = Vec::with_capacity((2 * (currentdepth + 1)) as usize);

    // Process all bound changes in the conflict candidate queue.
    let mut nresolutions = 0;
    let mut lastconsnresolutions = if mustresolve { 0 } else { -1 };
    let mut lastconsresoldepth = if mustresolve { currentdepth } else { i32::MAX };
    let mut info = conflict_first_cand(scip);

    while let Some(cur) = info
        && validdepth <= maxvaliddepth
    {
        // Check if the next bound change must be resolved in every case.
        let forceresolve = cf!(scip).forcedbdchgqueue.n_elems() > 0;

        // Resolve next bound change in queue.
        let bdchgdepth = bdchginfo_get_depth(scip, cur);

        // Create intermediate conflict constraint.
        if !forceresolve {
            if nresolutions == lastconsnresolutions {
                // All intermediate depth levels consisted of only unresolved
                // bound changes.
                lastconsresoldepth = bdchgdepth;
            } else if bdchgdepth < lastconsresoldepth && (scip.set.conf_interconss == -1 || counts.nconss < scip.set.conf_interconss) {
                // Call the conflict handlers to create a conflict set.
                let (success, nlits) = conflict_add_conflictset(scip, validdepth, diving, true);
                lastconsnresolutions = nresolutions;
                lastconsresoldepth = bdchgdepth;
                if success {
                    counts.nconss += 1;
                    counts.nliterals += nlits;
                }
            }
        }

        // Remove currently processed candidate and get next conflicting bound
        // from the conflict candidate queue before we remove the candidate we
        // have to collect the relaxed bound since removing the candidate from
        // the queue invalidates the relaxed bound.
        let relaxedbd = crate::var::bdchginfo_get_relaxed_bound(scip, bdchginfo(scip, cur));
        let cur = conflict_remove_cand(scip);
        let nextbdchginfo = conflict_first_cand(scip);

        // We don't need to resolve bound changes that are already active in
        // the valid depth of the current conflict set, because the conflict
        // set can only be added locally at the valid depth, and all bound
        // changes applied in this depth or earlier can be removed from the
        // conflict constraint, since they are already applied in the
        // constraint's subtree; if the next bound change on the remaining
        // queue is equal to the current bound change, this is a multiple
        // insertion in the conflict candidate queue and we can ignore the
        // current bound change.
        if bdchgdepth > validdepth && Some(cur) != nextbdchginfo {
            // Check if we want to resolve the bound change in this depth level
            //  - bound changes should be resolved, if
            //     (i)   we must apply at least one resolution and didn't
            //           resolve a bound change yet, or
            //     (ii)  their depth level is at least equal to the minimal
            //           resolving depth, and they are not the last remaining
            //           conflicting bound change in their depth level
            //     (iii) the bound change resolving is forced (i.e., the forced
            //           queue was non-empty)
            let mut resolved = false;
            if (mustresolve && nresolutions == 0) || (bdchgdepth >= resolvedepth && nextbdchginfo.is_some_and(|n| bdchginfo_get_depth(scip, n) == bdchgdepth)) || forceresolve {
                resolved = conflict_resolve_bound(scip, cur, relaxedbd, validdepth)?;
            }

            if resolved {
                nresolutions += 1;
            } else if forceresolve {
                // Variable cannot enter the conflict clause: we have to make
                // the conflict clause local, s.t. the unresolved bound change
                // is active in the whole sub tree of the conflict clause.
                validdepth = bdchgdepth;
            } else {
                // If this is a UIP (the last bound change in its depth level),
                // it can be used to generate a UIP reconvergence constraint.
                if nextbdchginfo.is_none_or(|n| bdchginfo_get_depth(scip, n) != bdchgdepth) {
                    firstuips.push(cur);
                }

                // Put variable into the conflict set, using the literal that is
                // currently fixed to FALSE.
                conflict_add_conflict_bound(scip, cur, relaxedbd);
            }
        }

        // Get next conflicting bound from the conflict candidate queue (this
        // needs not to be nextbdchginfo, because due to resolving the bound
        // changes, a bound change could be added to the queue which must be
        // resolved before nextbdchginfo).
        info = conflict_first_cand(scip);
    }

    // Check, if a valid conflict set was found.
    if info.is_none()
        && nresolutions > lastconsnresolutions
        && validdepth <= maxvaliddepth
        && (!mustresolve || nresolutions > 0 || cf!(scip).conflictset.nbdchginfos == 0)
        && cf!(scip).forcedbdchgqueue.n_elems() == 0
    {
        // Call the conflict handlers to create a conflict set.
        let (success, nlits) = conflict_add_conflictset(scip, validdepth, diving, true);
        if success {
            counts.nconss += 1;
            counts.nliterals += nlits;
        }
    }

    // Produce reconvergence constraints defined by succeeding UIP's of the
    // last depth level.
    if scip.set.conf_reconvlevels != 0 && validdepth <= maxvaliddepth {
        let reconvlevels = if scip.set.conf_reconvlevels == -1 { i32::MAX } else { scip.set.conf_reconvlevels };
        for &u in &firstuips {
            if crate::var::bdchginfo_has_inference_reason(bdchginfo(scip, u)) && currentdepth - bdchginfo_get_depth(scip, u) < reconvlevels {
                conflict_create_reconvergence_conss(scip, prob, diving, validdepth, u, &mut counts.nreconvconss, &mut counts.nreconvliterals)?;
            }
        }
    }

    // Store last conflict type.
    let conftype = cf!(scip).conflictset.conflicttype;

    // Clear the conflict candidate queue and the conflict set.
    conflict_clear(&mut cf!(scip));

    // Restore last conflict type.
    cf!(scip).conflictset.conflicttype = conftype;
    Ok(counts)
}

/// `calcBdchgScore`: calculates the score of a bound change within a
/// conflict.
pub fn calc_bdchg_score(scip: &Scip, prooflhs: f64, proofact: f64, proofactdelta: f64, proofcoef: f64, depth: i32, currentdepth: i32, var: VarId) -> f64 {
    use crate::types::LockType;
    let set = &scip.set;

    let mut score = set.conf_proofscorefac * (1.0 - proofactdelta / (prooflhs - proofact));
    score = crate::def::max(score, 0.0);
    score += set.conf_depthscorefac * (depth + 1) as f64 / (currentdepth + 1) as f64;

    let v = &scip.vars[var];
    let col = if v.varstatus == crate::types::VarStatus::Column { v.data.col } else { NONE };
    let nnonz = if col != NONE { scip.cols[col].len } else { 0 };

    if proofcoef > 0.0 {
        let nlocks = crate::var::var_get_n_locks_up_type(scip, var, LockType::Model);
        if col != NONE && nnonz > 0 {
            score += set.conf_uplockscorefac * nlocks as f64 / nnonz as f64;
        } else {
            score += set.conf_uplockscorefac * nlocks as f64;
        }
    } else {
        let nlocks = crate::var::var_get_n_locks_down_type(scip, var, LockType::Model);
        if col != NONE && nnonz > 0 {
            score += set.conf_downlockscorefac * nlocks as f64 / nnonz as f64;
        } else {
            score += set.conf_downlockscorefac * nlocks as f64;
        }
    }
    score
}

/// `skipRedundantBdchginfos`: after changing the global bound of a
/// variable, the bdchginfos that are now redundant are replaced with
/// oldbound = newbound = global bound; if the current bdchginfo is of such
/// kind, the bound is equal to the global bound and we can ignore it by
/// installing a -1 as the corresponding bound change info position.
pub fn skip_redundant_bdchginfos(scip: &Scip, var: VarId, lbchginfopos: &mut i32, ubchginfopos: &mut i32) {
    let v = &scip.vars[var];
    if *lbchginfopos >= 0 && *lbchginfopos < v.nlbchginfos && v.lbchginfos[*lbchginfopos].redundant {
        *lbchginfopos = -1;
    }
    if *ubchginfopos >= 0 && *ubchginfopos < v.nubchginfos && v.ubchginfos[*ubchginfopos].redundant {
        *ubchginfopos = -1;
    }
}

/// `SCIPconflictAddBound`: adds variable's bound to conflict candidate
/// queue.
pub fn conflict_add_bound_public(scip: &mut Scip, mut var: VarId, mut boundtype: BoundType, bdchgidx: Option<&crate::var::BdChgIdx>) -> ScipResult {
    use crate::types::VarStatus;

    // Convert bound to active problem variable.
    convert_to_active_var(scip, &mut var, &mut boundtype, None)?;

    // We can ignore fixed variables.
    if scip.vars[var].varstatus == VarStatus::Fixed {
        return Ok(());
    }

    // If the variable is multi-aggregated, add the bounds of all aggregation
    // variables.
    if scip.vars[var].varstatus == VarStatus::MultAggr {
        let nvars = scip.vars[var].data.multaggr.nvars;
        for i in 0..nvars {
            let (v, s) = (scip.vars[var].data.multaggr.vars[i], scip.vars[var].data.multaggr.scalars[i]);
            conflict_add_bound_public(scip, v, if s < 0.0 { boundtype.opposite() } else { boundtype }, bdchgidx)?;
        }
        return Ok(());
    }

    // Get bound change information.
    let Some(pos) = crate::var::var_get_bdchg_info(scip, var, boundtype, bdchgidx, false) else {
        // If bound of variable was not changed (this means it is still the
        // global bound), we can ignore the conflicting bound.
        return Ok(());
    };
    let info = BdChgInfoRef::Var { var, boundtype, pos };
    let newbound = bdchginfo_get_newbound(scip, info);
    conflict_add_bound(scip, var, boundtype, info, newbound)
}

/// `SCIPconflictAddRelaxedBound`: adds variable's bound to conflict
/// candidate queue.
pub fn conflict_add_relaxed_bound(scip: &mut Scip, mut var: VarId, mut boundtype: BoundType, bdchgidx: Option<&crate::var::BdChgIdx>, mut relaxedbd: f64) -> ScipResult {
    use crate::types::VarStatus;

    if !crate::var::var_is_active(scip, var) {
        // Convert bound to active problem variable.
        convert_to_active_var(scip, &mut var, &mut boundtype, Some(&mut relaxedbd))?;

        // We can ignore fixed variables.
        if scip.vars[var].varstatus == VarStatus::Fixed {
            return Ok(());
        }

        // If the variable is multi-aggregated, add the bounds of all
        // aggregation variables.
        if scip.vars[var].varstatus == VarStatus::MultAggr {
            return conflict_add_bound_public(scip, var, boundtype, bdchgidx);
        }
    }

    // Get bound change information.
    let Some(pos) = crate::var::var_get_bdchg_info(scip, var, boundtype, bdchgidx, false) else {
        // If bound of variable was not changed (this means it is still the
        // global bound), we can ignore the conflicting bound.
        return Ok(());
    };
    let mut info = BdChgInfoRef::Var { var, boundtype, pos };

    // Get the position of the bound change information within the bound
    // change array of the variable.
    let mut nbdchgs = pos;

    // If the relaxed bound should be ignored, set the relaxed bound to the
    // bound given by the bdchgidx; that ensures that the loop(s) below will
    // be skipped.
    if scip.set.conf_ignorerelaxedbd {
        relaxedbd = bdchginfo_get_newbound(scip, info);
    }

    // Search for the bound change information which includes the relaxed
    // bound.
    if boundtype == BoundType::Lower {
        // Adjust relaxed lower bound w.r.t. variable type.
        crate::var::var_adjust_lb(scip, var, &mut relaxedbd);

        // Due to numericis we compare the relaxed lower bound to the one
        // present at the particular time point and take the better one.
        let newbound = bdchginfo_get_newbound(scip, info);
        relaxedbd = crate::def::min(relaxedbd, newbound);

        // Check if relaxed lower bound is smaller or equal to global lower
        // bound; if so we can ignore the conflicting bound.
        if scip.set.is_le(relaxedbd, scip.vars[var].glbdom.lb) {
            return Ok(());
        }

        while nbdchgs > 0 {
            // Check if the old lower bound is greater than or equal to relaxed
            // lower bound; if not we found the bound change info which we need
            // to report.
            if scip.set.is_gt(relaxedbd, bdchginfo(scip, info).oldbound) {
                break;
            }

            info = BdChgInfoRef::Var { var, boundtype, pos: nbdchgs - 1 };

            // If bound change is redundant (this means it now a global bound),
            // we can ignore the conflicting bound.
            if bdchginfo(scip, info).redundant {
                return Ok(());
            }

            nbdchgs -= 1;
        }
    } else {
        // Adjust relaxed upper bound w.r.t. variable type.
        crate::var::var_adjust_ub(scip, var, &mut relaxedbd);

        // Due to numericis we compare the relaxed upper bound to the one
        // present at the particular time point and take the better one.
        let newbound = bdchginfo_get_newbound(scip, info);
        relaxedbd = crate::def::max(relaxedbd, newbound);

        // Check if relaxed upper bound is greater or equal to global upper
        // bound; if so we can ignore the conflicting bound.
        if scip.set.is_ge(relaxedbd, scip.vars[var].glbdom.ub) {
            return Ok(());
        }

        while nbdchgs > 0 {
            // Check if the old upper bound is smaller than or equal to the
            // relaxed upper bound; if not we found the bound change info which
            // we need to report.
            if scip.set.is_lt(relaxedbd, bdchginfo(scip, info).oldbound) {
                break;
            }

            info = BdChgInfoRef::Var { var, boundtype, pos: nbdchgs - 1 };

            // If bound change is redundant (this means it now a global bound),
            // we can ignore the conflicting bound.
            if bdchginfo(scip, info).redundant {
                return Ok(());
            }

            nbdchgs -= 1;
        }
    }

    // Put bound change information into priority queue.
    conflict_add_bound(scip, var, boundtype, info, relaxedbd)
}

/// `SCIPconflictIsVarUsed`: checks if the given variable is already part of
/// the current conflict set or queued for resolving with the same or even
/// stronger bound.
pub fn conflict_is_var_used(scip: &Scip, mut var: VarId, mut boundtype: BoundType, bdchgidx: Option<&crate::var::BdChgIdx>) -> ScipResult<bool> {
    use crate::types::VarStatus;

    // Convert bound to active problem variable.
    convert_to_active_var(scip, &mut var, &mut boundtype, None)?;

    let v = &scip.vars[var];
    if v.varstatus == VarStatus::Fixed || v.varstatus == VarStatus::MultAggr {
        return Ok(false);
    }
    let count = scip.conflict.as_ref().unwrap().count;
    Ok(match boundtype {
        BoundType::Lower => {
            let newbound = crate::api::get_var_lb_at_index(scip, var, bdchgidx, false);
            v.conflictlbcount == count && v.conflictlb >= newbound
        }
        BoundType::Upper => {
            let newbound = crate::api::get_var_ub_at_index(scip, var, bdchgidx, false);
            v.conflictubcount == count && v.conflictub <= newbound
        }
    })
}

/// `addBdchg`: inserts variable's new bounds into bound change arrays.
fn add_bdchg(scip: &Scip, var: VarId, newlb: f64, newub: f64, oldlpbdchgs: &mut LpBdChgs, relaxedlpbdchgs: &mut LpBdChgs) {
    use crate::types::VarStatus;

    if scip.vars[var].varstatus != VarStatus::Column {
        return;
    }
    let lpi = scip.lp.as_ref().unwrap().lpi.as_deref().unwrap();
    let col = scip.vars[var].data.col;
    let c = scip.cols[col].lppos;

    if c >= 0 {
        // Store old bound change for resetting the LP later.
        if !oldlpbdchgs.usedcols[c] {
            let idx = oldlpbdchgs.nbdchgs;
            oldlpbdchgs.usedcols[c] = true;
            oldlpbdchgs.bdchgcolinds[c] = idx;
            oldlpbdchgs.nbdchgs += 1;

            oldlpbdchgs.bdchginds[idx] = c;
            oldlpbdchgs.bdchglbs[idx] = crate::var::var_get_lb_lp(scip, var);
            oldlpbdchgs.bdchgubs[idx] = crate::var::var_get_ub_lp(scip, var);
        }

        // Store bound change for conflict analysis.
        let idx;
        if !relaxedlpbdchgs.usedcols[c] {
            idx = relaxedlpbdchgs.nbdchgs;
            relaxedlpbdchgs.usedcols[c] = true;
            relaxedlpbdchgs.bdchgcolinds[c] = idx;
            relaxedlpbdchgs.nbdchgs += 1;

            // Remember the positive for later further bound widenings.
            relaxedlpbdchgs.bdchginds[idx] = c;
        } else {
            idx = relaxedlpbdchgs.bdchgcolinds[c];
        }

        // Set the new bounds for the LP with the correct infinity value.
        relaxedlpbdchgs.bdchglbs[idx] = if scip.set.is_infinity(-newlb) { -crate::lpi::lpi_infinity(lpi) } else { newlb };
        relaxedlpbdchgs.bdchgubs[idx] = if scip.set.is_infinity(newub) { crate::lpi::lpi_infinity(lpi) } else { newub };
        if scip.set.is_infinity(-oldlpbdchgs.bdchglbs[idx]) {
            oldlpbdchgs.bdchglbs[idx] = -crate::lpi::lpi_infinity(lpi);
        }
        if scip.set.is_infinity(oldlpbdchgs.bdchgubs[idx]) {
            oldlpbdchgs.bdchgubs[idx] = crate::lpi::lpi_infinity(lpi);
        }
    }
}

/// The candidate arrays of `SCIPundoBdchgsProof`.
#[derive(Default)]
struct UndoCands {
    cands: Vec<VarId>,
    candscores: Vec<f64>,
    newbounds: Vec<f64>,
    proofactdeltas: Vec<f64>,
    ncands: usize,
}

/// `addCand`: adds variable to candidate list, if the current best bound
/// corresponding to the proof coefficient is local; the candidates are
/// sorted with respect to the following two criteria:
///  - prefer bound changes that have been applied deeper in the tree, to
///    get a more global conflict
///  - prefer variables with small Farkas coefficient to get rid of as many
///    bound changes as possible
#[allow(clippy::too_many_arguments)]
fn add_cand(scip: &Scip, currentdepth: i32, var: VarId, lbchginfopos: i32, ubchginfopos: i32, proofcoef: f64, prooflhs: f64, proofact: f64, c: &mut UndoCands, firstcand: usize) {
    use crate::quad::{prod_qd, sum_dd};

    let v = &scip.vars[var];
    let oldbound;
    let newbound;
    let depth;
    let resolvable;

    // In the infeasibility or dual bound proof, the variable's bound is chosen
    // to maximize the proof's activity.
    if proofcoef > 0.0 {
        // Calculate the difference of current bound to the previous bound the
        // variable was set to.
        if ubchginfopos == v.nubchginfos {
            // Current bound is the strong branching or diving bound.
            oldbound = crate::var::var_get_ub_lp(scip, var);
            newbound = v.locdom.ub;
            depth = currentdepth + 1;
            resolvable = false;
        } else {
            // Current bound is the result of a local bound change.
            let info = &v.ubchginfos[ubchginfopos];
            resolvable = crate::var::bdchginfo_has_inference_reason(info);
            depth = info.bdchgidx.depth;
            oldbound = info.newbound;
            newbound = info.oldbound;
        }
    } else {
        // Calculate the difference of current bound to the previous bound the
        // variable was set to.
        if lbchginfopos == v.nlbchginfos {
            // Current bound is the strong branching or diving bound.
            oldbound = crate::var::var_get_lb_lp(scip, var);
            newbound = v.locdom.lb;
            depth = currentdepth + 1;
            resolvable = false;
        } else {
            // Current bound is the result of a local bound change.
            let info = &v.lbchginfos[lbchginfopos];
            resolvable = crate::var::bdchginfo_has_inference_reason(info);
            depth = info.bdchgidx.depth;
            oldbound = info.newbound;
            newbound = info.oldbound;
        }
    }

    // Calculate the increase in the proof's activity.
    let mut proofactdelta = sum_dd(newbound, -oldbound);
    proofactdelta = prod_qd(proofactdelta, proofcoef);

    // Calculate score for undoing the bound change.
    let mut score = calc_bdchg_score(scip, prooflhs, proofact, proofactdelta.to_dbl(), proofcoef, depth, currentdepth, var);

    if !resolvable {
        score += 10.0;
        if !crate::var::var_is_binary(scip, var) {
            score += 10.0;
        }
    }

    // Get enough memory to store new candidate.
    if c.ncands + 1 > c.cands.len() {
        let newsize = scip.set.calc_mem_grow_size((c.ncands + 1) as i32) as usize;
        c.cands.resize(newsize, NONE);
        c.candscores.resize(newsize, 0.0);
        c.newbounds.resize(newsize, 0.0);
        c.proofactdeltas.resize(newsize, 0.0);
    }

    // Insert variable in candidate list without touching the already
    // processed candidates.
    let mut i = c.ncands;
    while i > firstcand && score > c.candscores[i - 1] {
        c.cands[i] = c.cands[i - 1];
        c.candscores[i] = c.candscores[i - 1];
        c.newbounds[i] = c.newbounds[i - 1];
        c.proofactdeltas[i] = c.proofactdeltas[i - 1];
        i -= 1;
    }
    c.cands[i] = var;
    c.candscores[i] = score;
    c.newbounds[i] = newbound;
    c.proofactdeltas[i] = proofactdelta.to_dbl();
    c.ncands += 1;
}

/// The LP bound change arrays `SCIPundoBdchgsProof` records into.
pub struct ProofLpBdChgs<'a> {
    pub oldlpbdchgs: &'a mut LpBdChgs,
    pub relaxedlpbdchgs: &'a mut LpBdChgs,
}

/// `SCIPundoBdchgsProof`: undoes bound changes on variables, still leaving
/// the given infeasibility proof valid; returns whether the changed LP
/// should be resolved again.
#[allow(clippy::too_many_arguments)]
pub fn undo_bdchgs_proof(
    scip: &Scip,
    prob: usize,
    currentdepth: i32,
    proofcoefs: &mut [f64],
    prooflhs: f64,
    proofact: &mut f64,
    curvarlbs: &mut [f64],
    curvarubs: &mut [f64],
    lbchginfoposs: &mut [i32],
    ubchginfoposs: &mut [i32],
    mut lpbdchgs: Option<ProofLpBdChgs>,
) -> bool {
    let mut resolve = false;

    let nvars = scip.probs[prob].nvars as usize;

    // Calculate the order in which the bound changes are tried to be undone,
    // and relax all bounds if this doesn't increase the proof's activity.
    let mut c = UndoCands {
        cands: vec![NONE; nvars],
        candscores: vec![0.0; nvars],
        newbounds: vec![0.0; nvars],
        proofactdeltas: vec![0.0; nvars],
        ncands: 0,
    };
    for v in 0..nvars {
        let var = scip.probs[prob].vars[v];

        // After changing the global bound of a variable, the bdchginfos that
        // are now redundant are replaced with oldbound = newbound = global
        // bound; if the current bdchginfo is of such kind, the bound is equal
        // to the global bound and we can ignore it.
        skip_redundant_bdchginfos(scip, var, &mut lbchginfoposs[v], &mut ubchginfoposs[v]);

        // Ignore variables already relaxed to global bounds.
        if lbchginfoposs[v] == -1 && ubchginfoposs[v] == -1 {
            proofcoefs[v] = 0.0;
            continue;
        }

        // Relax bounds that are not used in the proof to the global bounds.
        let mut relaxed = false;
        if !scip.set.is_negative(proofcoefs[v]) {
            // The lower bound is not used.
            if lbchginfoposs[v] >= 0 {
                curvarlbs[v] = scip.vars[var].glbdom.lb;
                lbchginfoposs[v] = -1;
                relaxed = true;
            }
        }
        if !scip.set.is_positive(proofcoefs[v]) {
            // The upper bound is not used.
            if ubchginfoposs[v] >= 0 {
                curvarubs[v] = scip.vars[var].glbdom.ub;
                ubchginfoposs[v] = -1;
                relaxed = true;
            }
        }
        if relaxed && let Some(l) = lpbdchgs.as_mut() {
            add_bdchg(scip, var, curvarlbs[v], curvarubs[v], l.oldlpbdchgs, l.relaxedlpbdchgs);
        }

        // Add bound to candidate list.
        if lbchginfoposs[v] >= 0 || ubchginfoposs[v] >= 0 {
            add_cand(scip, currentdepth, var, lbchginfoposs[v], ubchginfoposs[v], proofcoefs[v], prooflhs, *proofact, &mut c, 0);
        }
        // We can set the proof coefficient to zero, because the variable is not
        // needed.
        else {
            proofcoefs[v] = 0.0;
        }
    }

    // Try to undo remaining local bound changes while still keeping the proof
    // row violated: bound changes can be undone, if prooflhs > proofact +
    // proofactdelta; afterwards, the current proof activity has to be updated.
    let mut i = 0;
    while i < c.ncands {
        // When relaxing a constraint we still need to stay infeasible;
        // therefore we need to do the comparison in feasibility tolerance
        // because if 'prooflhs' is (feas-))equal to 'proofact +
        // proofactdeltas[i]' it would mean that there is no violation.
        if scip.set.is_feas_gt(prooflhs, *proofact + c.proofactdeltas[i]) {
            let cand = c.cands[i];
            let v = scip.vars[cand].probindex as usize;

            if proofcoefs[v] > 0.0 {
                curvarubs[v] = c.newbounds[i];
                ubchginfoposs[v] -= 1;
            } else {
                curvarlbs[v] = c.newbounds[i];
                lbchginfoposs[v] -= 1;
            }
            if let Some(l) = lpbdchgs.as_mut() {
                add_bdchg(scip, cand, curvarlbs[v], curvarubs[v], l.oldlpbdchgs, l.relaxedlpbdchgs);
            }
            *proofact += c.proofactdeltas[i];
            if crate::var::var_is_in_lp(scip, cand) {
                resolve = true;
            }

            // After changing the global bound of a variable, the bdchginfos
            // that are now redundant are replaced with oldbound = newbound =
            // global bound; if the current bdchginfo is of such kind, the bound
            // is equal to the global bound and we can ignore it.
            skip_redundant_bdchginfos(scip, cand, &mut lbchginfoposs[v], &mut ubchginfoposs[v]);

            // Insert the new local bound of the variable into the candidate
            // list.
            if lbchginfoposs[v] >= 0 || ubchginfoposs[v] >= 0 {
                add_cand(
                    scip,
                    currentdepth,
                    cand,
                    lbchginfoposs[v],
                    ubchginfoposs[v],
                    proofcoefs[v],
                    prooflhs,
                    *proofact,
                    &mut c,
                    i + 1,
                );
            } else {
                proofcoefs[v] = 0.0;
            }
        }
        i += 1;
    }

    resolve
}

/// `undoBdchgsDualfarkas` and `undoBdchgsDualsol`: analyzes an infeasible
/// LP (or one exceeding the objective limit) and undoes additional bound
/// changes while staying infeasible (beyond the objective limit); returns
/// `(valid, resolve)`. The Farkas variant never resolves: the old dual ray
/// is still valid, so resolving will not change the solution.
#[allow(clippy::too_many_arguments)]
fn undo_bdchgs_dual(
    scip: &Scip,
    prob: usize,
    currentdepth: i32,
    curvarlbs: &mut [f64],
    curvarubs: &mut [f64],
    lbchginfoposs: &mut [i32],
    ubchginfoposs: &mut [i32],
    lpbdchgs: Option<ProofLpBdChgs>,
    coefs: &mut [f64],
    lhs: f64,
    activity: &mut f64,
    farkas: bool,
) -> (bool, bool) {
    let mut valid = false;
    let mut resolve = false;

    // Check, if the proof row is still violated (using current bounds and
    // ignoring local rows).
    if scip.set.is_feas_gt(lhs, *activity) {
        // Undo bound changes while keeping the infeasibility proof valid.
        resolve = undo_bdchgs_proof(scip, prob, currentdepth, coefs, lhs, activity, curvarlbs, curvarubs, lbchginfoposs, ubchginfoposs, lpbdchgs);
        valid = true;
        if farkas {
            resolve = false;
        }
    }
    (valid, resolve)
}

/// `lpbdchgsCreate`: create conflict LP bound change data structure.
fn lpbdchgs_create(ncols: i32) -> LpBdChgs {
    let n = ncols as usize;
    LpBdChgs {
        bdchginds: CVec(vec![0; n]),
        bdchglbs: CVec(vec![0.0; n]),
        bdchgubs: CVec(vec![0.0; n]),
        bdchgcolinds: CVec(vec![0; n]),
        usedcols: CVec(vec![false; n]),
        nbdchgs: 0,
    }
}

/// `lpbdchgsReset`: reset conflict LP bound change data structure.
fn lpbdchgs_reset(lpbdchgs: &mut LpBdChgs, ncols: i32) {
    lpbdchgs.usedcols.0[..ncols as usize].fill(false);
    lpbdchgs.nbdchgs = 0;
}

/// The side change arrays of the propagation conflict analysis.
#[derive(Default)]
pub struct SideChgs {
    pub inds: Vec<i32>,
    pub oldlhss: Vec<f64>,
    pub oldrhss: Vec<f64>,
    pub newlhss: Vec<f64>,
    pub newrhss: Vec<f64>,
}

/// `addSideRemoval`: adds removal of row's side to side change arrays;
/// finite sides are only replaced by near infinite sides, such that the
/// row's sense in the LP solver is not changed.
pub fn add_side_removal(scip: &Scip, row: crate::types::RowId, lpiinfinity: f64, s: &mut SideChgs) {
    let r = &scip.rows[row];
    let (lhs, rhs, constant) = (r.lhs, r.rhs, r.constant);

    // Store side change.
    s.inds.push(r.lppos);
    if scip.set.is_infinity(-lhs) {
        s.oldlhss.push(-lpiinfinity);
        s.newlhss.push(-lpiinfinity);
    } else {
        s.oldlhss.push(lhs - constant);
        s.newlhss.push(-lpiinfinity);
    }
    if scip.set.is_infinity(rhs) {
        s.oldrhss.push(lpiinfinity);
        s.newrhss.push(lpiinfinity);
    } else {
        s.oldrhss.push(rhs - constant);
        s.newrhss.push(lpiinfinity);
    }
}

/// The proof arrays `SCIPrunBoundHeuristic` works on.
pub struct BoundHeuristicData<'a> {
    pub proofcoefs: &'a mut [f64],
    pub prooflhs: &'a mut f64,
    pub proofactivity: &'a mut f64,
    pub curvarlbs: &'a mut [f64],
    pub curvarubs: &'a mut [f64],
    pub lbchginfoposs: &'a mut [i32],
    pub ubchginfoposs: &'a mut [i32],
}

/// The LP solver interface of the LP.
#[inline]
fn lp_lpi(scip: &mut Scip) -> &mut crate::lpi::Lpi {
    lp!(scip).lpi.as_deref_mut().unwrap()
}

/// `SCIPrunBoundHeuristic`: try to find a subset of changed bounds leading
/// to an infeasible LP
///
///  1. call undoBdchgsDualfarkas() or undoBdchgsDualsol()
///     -> update lb/ubchginfoposs arrays
///     -> store additional changes in bdchg and curvarlbs/ubs arrays
///     -> apply additional changes to the LPI
///  2. (optional) if additional bound changes were undone:
///     -> resolve LP
///     -> goto 1.
///  3. redo all bound changes in the LPI to restore the LPI to its original
///     state
///  4. analyze conflict
///     -> put remaining changed bounds (see lb/ubchginfoposs arrays) into
///        starting conflict set
///
/// The LPI is the LP's. Adds the LP iterations used to `iterations`, sets
/// `dualproofsuccess` when dual proof analysis succeeded, and updates
/// `valid`, whether the result is still a valid proof.
#[allow(clippy::too_many_arguments)]
pub fn run_bound_heuristic(
    scip: &mut Scip,
    origprob: usize,
    transprob: usize,
    eventfilter: usize,
    d: BoundHeuristicData,
    iterations: &mut i32,
    marklpunsolved: bool,
    dualproofsuccess: &mut bool,
    valid: &mut bool,
) -> ScipResult {
    use crate::lpi::*;

    let BoundHeuristicData {
        proofcoefs,
        prooflhs,
        proofactivity,
        curvarlbs,
        curvarubs,
        lbchginfoposs,
        ubchginfoposs,
    } = d;

    // Get number of columns in the LP.
    let ncols = crate::lp::lp_get_n_cols(scip);

    // Get temporary memory for remembering bound changes on LPI columns.
    let mut oldlpbdchgs = lpbdchgs_create(ncols);
    let mut relaxedlpbdchgs = lpbdchgs_create(ncols);

    // Undo as many bound changes as possible with the current LP solution.
    let mut resolve = false;
    if *valid {
        let currentdepth = crate::tree::tree_get_current_depth(scip);
        let farkas = lpi_is_primal_infeasible(lp_lpi(scip));
        let l = ProofLpBdChgs {
            oldlpbdchgs: &mut oldlpbdchgs,
            relaxedlpbdchgs: &mut relaxedlpbdchgs,
        };
        (*valid, resolve) = undo_bdchgs_dual(
            scip,
            transprob,
            currentdepth,
            curvarlbs,
            curvarubs,
            lbchginfoposs,
            ubchginfoposs,
            Some(l),
            proofcoefs,
            *prooflhs,
            proofactivity,
            farkas,
        );
    }

    // Check if we want to solve the LP.
    let solvelp = scip.set.conf_maxlploops != 0 && scip.set.conf_lpiterations != 0;

    if *valid && resolve && solvelp {
        // Get infinity value of LP solver.
        let lpiinfinity = lpi_infinity(lp_lpi(scip));

        // Temporarily disable objective limit and install an iteration limit.
        let maxlploops = if scip.set.conf_maxlploops >= 0 { scip.set.conf_maxlploops } else { i32::MAX };
        let lpiterations = if scip.set.conf_lpiterations >= 0 { scip.set.conf_lpiterations } else { i32::MAX };
        lpi_set_realpar(lp_lpi(scip), LpParam::ObjLim, lpiinfinity)?;
        lpi_set_intpar(lp_lpi(scip), LpParam::LpItLim, lpiterations)?;

        // Get LP rows.
        let nrows = crate::lp::lp_get_n_rows(scip);

        // Get temporary memory for remembering side changes on LPI rows.
        let mut sidechgs = SideChgs::default();

        // Remove all local rows by setting their sides to infinity; finite
        // sides are only changed to near infinity, such that the row's sense
        // in the LP solver is not affected (e.g. CPLEX cannot handle free
        // rows).
        for r in 0..nrows {
            let row = lp!(scip).rows[r];
            if scip.rows[row].local {
                add_side_removal(scip, row, lpiinfinity, &mut sidechgs);
            }
        }

        // Apply changes of local rows to the LP solver.
        if !sidechgs.inds.is_empty() {
            let SideChgs { inds, newlhss, newrhss, .. } = &sidechgs;
            lpi_chg_sides(lp_lpi(scip), inds, newlhss, newrhss)?;
        }

        // Undo as many additional bound changes as possible by resolving the
        // LP.
        let mut nloops = 0;
        let mut globalinfeasible = false;
        'resolve: while *valid && resolve && nloops < maxlploops {
            nloops += 1;
            resolve = false;

            // Apply bound changes to the LP solver.
            if relaxedlpbdchgs.nbdchgs > 0 {
                let n = relaxedlpbdchgs.nbdchgs as usize;
                lpi_chg_bounds(
                    lp_lpi(scip),
                    &relaxedlpbdchgs.bdchginds.0[..n],
                    &relaxedlpbdchgs.bdchglbs.0[..n],
                    &relaxedlpbdchgs.bdchgubs.0[..n],
                )?;

                // Reset conflict LP bound change data structure.
                lpbdchgs_reset(&mut relaxedlpbdchgs, ncols);
            }

            // Start LP timer.
            scip.stat.conflictlptime.start(scip.set.time_enabled);

            // Resolve LP.
            let retcode = lpi_solve_dual(lp_lpi(scip));

            // Stop LP timer.
            scip.stat.conflictlptime.stop(scip.set.time_enabled);

            // Check return code of LP solving call.
            match retcode {
                Err(Retcode::LpError) => {
                    *valid = false;
                    break;
                }
                r => r?,
            }

            // Count number of LP iterations.
            let iter = lpi_get_iterations(lp_lpi(scip))?;
            *iterations += iter;
            scip.stat.nconflictlps += 1;
            scip.stat.nconflictlpiterations += iter as i64;

            // Evaluate result.
            let lpi = lp_lpi(scip);
            if lpi_is_dual_feasible(lpi) || lpi_is_objlim_exc(lpi) {
                let objval = lpi_get_objval(lpi)?;
                *valid = objval >= lp!(scip).lpiobjlim && !crate::lp::lp_diving_obj_changed(scip);
            } else {
                *valid = lpi_is_primal_infeasible(lpi);
            }

            if *valid {
                let currentdepth = crate::tree::tree_get_current_depth(scip);

                // Undo additional bound changes.
                let farkas = lpi_is_primal_infeasible(lp_lpi(scip));
                let mut proofrow = crate::cuts::aggr_row_create(scip);
                let validdepth;
                if farkas {
                    // The original LP exceeds the current cutoff bound, thus, we
                    // have not constructed the Farkas proof.
                    (*proofactivity, validdepth, *valid) = get_farkas_proof(scip, transprob, &mut proofrow, curvarlbs, curvarubs)?;

                    // The constructed Farkas proof is not valid, we need to break
                    // here.
                    if !*valid {
                        break;
                    }

                    // Start dual proof analysis.
                    if scip.set.conf_useinflp == b'd' || scip.set.conf_useinflp == b'b' {
                        // Change the conflict type.
                        let oldusescutoff = cf!(scip).conflictset.usescutoffbound;
                        let oldconftype = cf!(scip).conflictset.conflicttype;
                        cf!(scip).conflictset.usescutoffbound = false;
                        cf!(scip).conflictset.conflicttype = ConfType::InfeasLp;

                        // Start dual proof analysis.
                        (globalinfeasible, *dualproofsuccess) =
                            conflict_analyze_dual_proof(scip, eventfilter, origprob, transprob, &mut proofrow, validdepth, curvarlbs, curvarubs, false)?;

                        cf!(scip).conflictset.usescutoffbound = oldusescutoff;
                        cf!(scip).conflictset.conflicttype = oldconftype;
                    }

                    // todo: in theory, we could apply conflict graph analysis for
                    // locally valid proofs, too, but this needs to be implemented.
                    if globalinfeasible || validdepth > crate::tree::tree_get_effective_root_depth(scip) {
                        break 'resolve;
                    }
                } else {
                    (*proofactivity, validdepth, *valid) = get_dual_proof(scip, transprob, &mut proofrow, curvarlbs, curvarubs)?;

                    // The constructed dual proof is not valid, we need to break
                    // here.
                    if !*valid || validdepth > crate::tree::tree_get_effective_root_depth(scip) {
                        break;
                    }

                    // In contrast to the infeasible case we don't want to analyze
                    // the (probably identical) proof again.
                }

                let nvars = scip.probs[transprob].nvars as usize;
                proofcoefs[..nvars].fill(0.0);
                *prooflhs = -crate::cuts::aggr_row_get_rhs(&proofrow);
                *proofactivity = -*proofactivity;

                let nnz = crate::cuts::aggr_row_get_n_nz(&proofrow);
                for v in 0..nnz {
                    let i = crate::cuts::aggr_row_get_inds(&proofrow)[v as usize] as usize;
                    proofcoefs[i] = -crate::cuts::aggr_row_get_probvar_value(&proofrow, i as i32);
                }

                let l = ProofLpBdChgs {
                    oldlpbdchgs: &mut oldlpbdchgs,
                    relaxedlpbdchgs: &mut relaxedlpbdchgs,
                };
                (*valid, resolve) = undo_bdchgs_dual(
                    scip,
                    transprob,
                    currentdepth,
                    curvarlbs,
                    curvarubs,
                    lbchginfoposs,
                    ubchginfoposs,
                    Some(l),
                    proofcoefs,
                    *prooflhs,
                    proofactivity,
                    farkas,
                );
            }
        }

        // TERMINATE:
        // Reset variables to local bounds.
        if oldlpbdchgs.nbdchgs > 0 {
            let n = oldlpbdchgs.nbdchgs as usize;
            lpi_chg_bounds(lp_lpi(scip), &oldlpbdchgs.bdchginds.0[..n], &oldlpbdchgs.bdchglbs.0[..n], &oldlpbdchgs.bdchgubs.0[..n])?;
        }

        // Reset changes of local rows.
        if !sidechgs.inds.is_empty() {
            let SideChgs { inds, oldlhss, oldrhss, .. } = &sidechgs;
            lpi_chg_sides(lp_lpi(scip), inds, oldlhss, oldrhss)?;
        }

        // Mark the LP unsolved.
        if oldlpbdchgs.nbdchgs > 0 || !sidechgs.inds.is_empty() {
            // The LPI data are out of sync with LP data. Thus, the LP should be
            // marked unsolved. However, for strong branching calls, the LP has
            // to have status 'solved'; in this case, marklpunsolved is FALSE and
            // synchronization is performed later.
            if marklpunsolved {
                let lp = &mut lp!(scip);
                lp.solved = false;
                lp.primalfeasible = false;
                lp.primalchecked = false;
                lp.dualfeasible = false;
                lp.dualchecked = false;
                lp.lpobjval = crate::def::INVALID;
                lp.lpsolstat = crate::types::LpSolStat::NotSolved;
            }
        }

        // Reinstall old objective and iteration limits in LP solver.
        let (objlim, itlim) = (lp!(scip).lpiobjlim, lp!(scip).lpiitlim);
        lpi_set_realpar(lp_lpi(scip), LpParam::ObjLim, objlim)?;
        lpi_set_intpar(lp_lpi(scip), LpParam::LpItLim, itlim)?;
    }

    Ok(())
}

/// `SCIPconflictAnalyze`: analyzes conflicting bound changes that were
/// added with calls to SCIPconflictAddBound(), and on success, calls the
/// conflict handlers to create a conflict constraint out of the resulting
/// conflict set; updates statistics for propagation conflict analysis;
/// returns whether a conflict constraint was created.
pub fn conflict_analyze_public(scip: &mut Scip, prob: usize, validdepth: i32) -> ScipResult<bool> {
    // Check if the conflict analysis is applicable.
    if !conflict_graph_applicable(scip) {
        return Ok(false);
    }

    // Check, if the conflict set will get too large with high probability.
    {
        let c = scip.conflict.as_ref().unwrap();
        if c.conflictset.nbdchginfos + c.bdchgqueue.n_elems() as i32 + c.forcedbdchgqueue.n_elems() as i32 >= 2 * conflict_calc_maxsize(scip, prob) {
            return Ok(false);
        }
    }

    // Start timing.
    let te = scip.set.time_enabled;
    cf!(scip).propanalyzetime.start(te);

    cf!(scip).npropcalls += 1;

    // Setting this to true adds bound changes only to the conflict graph bdchg
    // queue.
    cf!(scip).bdchgonlyconfqueue = true;

    // Analyze the conflict set, and create a conflict constraint on success.
    let n = conflict_analyze(scip, prob, false, validdepth, true)?;
    let c = &mut cf!(scip);
    c.npropsuccess += if n.nconss > 0 { 1 } else { 0 };
    c.npropconfconss += n.nconss as i64;
    c.npropconfliterals += n.nliterals as i64;
    c.npropreconvconss += n.nreconvconss as i64;
    c.npropreconvliterals += n.nreconvliterals as i64;

    c.bdchgonlyconfqueue = false;

    // Stop timing.
    c.propanalyzetime.stop(te);

    Ok(n.nconss > 0)
}

/*
 * conflict_general.c
 */

/// `NUMSTOP`: because calculations might cancel out some values, we stop
/// the infeasibility analysis if a value is bigger than 2^53.
const NUMSTOP: f64 = 9007199254740992.0;
/// `SOLSTOP`: because row violations might be magnified, we stop the
/// infeasibility analysis if a dual weight is bigger than 10^7.
const SOLSTOP: f64 = 10000000.0;

/// `SCIPconflictApplicable`: return TRUE if conflict analysis is
/// applicable.
pub fn conflict_applicable(scip: &Scip) -> bool {
    // Check, if resolution or propagation conflict analysis is enabled.
    scip.set.conf_enable && (scip.set.conf_useprop || scip.set.conf_usegenres)
}

/// `SCIPconflictEnableOrDisableClocks`: enables or disables all clocks of
/// the conflict analysis data.
pub fn conflict_enable_or_disable_clocks(c: &mut Conflict, enable: bool) {
    c.boundlpanalyzetime.enable_or_disable(enable);
    c.dibclock.enable_or_disable(enable);
    c.inflpanalyzetime.enable_or_disable(enable);
    c.propanalyzetime.enable_or_disable(enable);
    c.resanalyzetime.enable_or_disable(enable);
    c.pseudoanalyzetime.enable_or_disable(enable);
    c.sbanalyzetime.enable_or_disable(enable);
}

/// `SCIPconflictCreate`: creates conflict analysis data for propagation
/// conflicts.
pub fn conflict_create(scip: &Scip) -> Box<Conflict> {
    let initsize = scip.set.mem_arraygrowinit as usize;
    let mut c = Box::new(Conflict {
        bdchgqueue: PQueue::new(initsize),
        forcedbdchgqueue: PQueue::new(initsize),
        conflictset: conflictset_create(),
        ..Conflict::default()
    });

    // Enable or disable timing depending on the parameter statistic timing.
    conflict_enable_or_disable_clocks(&mut c, scip.set.time_statistictiming);

    conflict_init_proofset(&mut c);
    c
}

/// `SCIPconflictClearQueues`: clears the conflict queue and the current
/// conflict set.
pub fn conflict_clear_queues(c: &mut Conflict) {
    c.bdchgqueue.clear();
    c.forcedbdchgqueue.clear();
}

/// `SCIPconflictGetVarLb`: returns the conflict lower bound if the
/// variable is present in the current conflict set; otherwise the global
/// lower bound.
pub fn conflict_get_var_lb(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    if v.conflictlbcount == scip.conflict.as_ref().unwrap().count {
        return v.conflictrelaxedlb;
    }
    v.glbdom.lb
}

/// `SCIPconflictGetVarUb`: returns the conflict upper bound if the
/// variable is present in the current conflict set; otherwise the global
/// upper bound.
pub fn conflict_get_var_ub(scip: &Scip, var: VarId) -> f64 {
    let v = &scip.vars[var];
    if v.conflictubcount == scip.conflict.as_ref().unwrap().count {
        return v.conflictrelaxedub;
    }
    v.glbdom.ub
}

/// `addRowToAggrRow`: adds a weighted LP row to an aggregation row;
/// returns whether adding the row was successful.
fn add_row_to_aggr_row(scip: &Scip, row: crate::types::RowId, weight: f64, aggrrow: &mut crate::cuts::AggrRow) -> bool {
    // Add minimal value to dual row's left hand side: y_i < 0 -> lhs, y_i > 0
    // -> rhs.
    let negated = weight < 0.0;
    crate::cuts::aggr_row_add_row(scip, aggrrow, row, weight, if negated { -1 } else { 1 });
    true
}

/// `checkDualFeasibility`: checks validity of an LP row and a
/// corresponding weight; returns `(valid, zerocontribution)`, the latter
/// whether every row entry is zero within tolerances.
fn check_dual_feasibility(scip: &Scip, row: crate::types::RowId, weight: f64) -> (bool, bool) {
    let set = &scip.set;
    let r = &scip.rows[row];
    let mut valid = true;
    let mut zerocontribution = true;

    // Dual solution values of 0.0 are always valid.
    if weight.abs() > crate::quad::QUAD_EPSILON {
        zerocontribution = false;

        // Check dual feasibility.
        if (set.is_infinity(-r.lhs) && weight > 0.0) || (set.is_infinity(r.rhs) && weight < 0.0) {
            // Ignore slight numerical violations if the contribution of every
            // component of the row is close to zero.
            zerocontribution = if weight > 0.0 {
                set.is_dualfeas_zero(r.rhs * weight)
            } else {
                set.is_dualfeas_zero(r.lhs * weight)
            };

            let mut i = 0;
            while i < r.len && zerocontribution {
                if !set.is_dualfeas_zero(weight * r.vals[i]) {
                    zerocontribution = false;
                }
                i += 1;
            }

            if !zerocontribution {
                valid = false;
            }
        }
    }
    (valid, zerocontribution)
}

/// `SCIPaggrRowGetMinActivity`: calculates the minimal activity of a given
/// aggregation row; `curvarlbs`/`curvarubs` are the current bounds of the
/// active problem variables (or `None` for global bounds); returns the
/// activity and whether at least one variable contributes with an infinite
/// value.
pub fn aggr_row_get_min_activity(scip: &Scip, transprob: usize, aggrrow: &crate::cuts::AggrRow, curvarlbs: Option<&[f64]>, curvarubs: Option<&[f64]>) -> (f64, bool) {
    use crate::quad::{Quad, prod_dd, sum_qq};
    let set = &scip.set;
    let vars = &scip.probs[transprob].vars;
    let nnz = aggrrow.nnz as usize;
    let inds = &aggrrow.inds;

    let mut minact = Quad::new(0.0);

    for i in 0..nnz {
        let v = inds[i] as usize;
        let val = crate::cuts::aggr_row_get_probvar_value(aggrrow, v as i32);

        let delta;
        if val > 0.0 {
            let bnd = match curvarlbs {
                None => scip.vars[vars[v]].glbdom.lb,
                Some(l) => l[v],
            };
            if set.is_infinity(-bnd) {
                return (-set.infinity(), true);
            }
            delta = prod_dd(val, bnd);
        } else {
            let bnd = match curvarubs {
                None => scip.vars[vars[v]].glbdom.ub,
                Some(u) => u[v],
            };
            if set.is_infinity(bnd) {
                return (-set.infinity(), true);
            }
            delta = prod_dd(val, bnd);
        }

        // Update minimal activity.
        minact = sum_qq(minact, delta);
    }

    // Check whether the minimal activity is infinite.
    if set.is_infinity(minact.to_dbl()) {
        return (set.infinity(), false);
    }
    if set.is_infinity(-minact.to_dbl()) {
        return (-set.infinity(), false);
    }
    (minact.to_dbl(), false)
}

/// `sortLocalRows`: sort local rows by increasing depth and number of
/// nonzeros as tie-breaker.
fn sort_local_rows(scip: &Scip, rows: &[crate::types::RowId], rowinds: &mut [i32], rowdepth: &mut [i32]) {
    let nrows = rowinds.len();

    // Sort row indices by increasing depth.
    crate::sort::sort(rowdepth, (&mut *rowinds,), crate::sort::cmp_int);

    // Get number of nonzero entries for every row.
    let mut rownnz: Vec<i32> = (0..nrows).map(|i| scip.rows[rows[rowinds[i] as usize]].len).collect();

    // Since SCIP has no stable sorting function we sort each bucket
    // separately.
    let mut i = 0;
    while i < nrows {
        let mut j = i;
        let d = rowdepth[i];

        // Search for the next row with a greater depth.
        while j + 1 < nrows && rowdepth[j + 1] == d {
            j += 1;
        }

        // The bucket has size one.
        if j == i {
            i += 1;
            continue;
        }

        // Sort row indices by increasing number of nonzero elements.
        crate::sort::sort(&mut rownnz[i..=j], (&mut rowdepth[i..=j], &mut rowinds[i..=j]), crate::sort::cmp_int);
        i = j + 1;
    }
}

/// `addLocalRows`: adds locally valid rows to the proof constraint; sets
/// `proofact`, the activity of the proof constraint, and `validdepth`, the
/// depth where it is valid, and updates `valid`.
#[allow(clippy::too_many_arguments)]
fn add_local_rows(
    scip: &Scip,
    transprob: usize,
    proofrow: &mut crate::cuts::AggrRow,
    rows: &[crate::types::RowId],
    dualsols: &[f64],
    localrowinds: &mut [i32],
    localrowdepth: &mut [i32],
    proofact: &mut f64,
    validdepth: &mut i32,
    curvarlbs: &[f64],
    curvarubs: &[f64],
    valid: &mut bool,
) {
    let set = &scip.set;
    *validdepth = 0;

    if !set.conf_uselocalrows {
        return;
    }

    // Check whether the proof is already valid, e.g., violated within the
    // local bounds.
    let (act, mut infdelta) = aggr_row_get_min_activity(scip, transprob, proofrow, Some(curvarlbs), Some(curvarubs));
    *proofact = act;

    // We stop if the minimal activity is infinite but all variables have a
    // finite activity delta (bad numerics).
    if !infdelta && set.is_infinity(proofact.abs()) {
        *valid = false;
        return;
    }

    // Break if the proof is valid w.r.t local bounds; note: it can happen that
    // the proof contains a variable with an infinite activity delta. Here, we
    // don't break immediately because we might be able to fix it by adding
    // local rows.
    if !infdelta && set.is_gt(*proofact, crate::cuts::aggr_row_get_rhs(proofrow)) {
        *valid = true;
        return;
    }

    // Sort local rows by depth.
    sort_local_rows(scip, rows, localrowinds, localrowdepth);

    // Add successively local rows.
    'terminate: {
        for i in 0..localrowinds.len() {
            let r = localrowinds[i] as usize;
            let row = rows[r];

            // Ignore dual solution values of 0.0 (in this case: y_i == 0).
            if dualsols[r].abs() > 0.0 {
                if set.is_dualfeas_zero(dualsols[r]) {
                    continue;
                }

                // Add row to dual proof.
                *valid = add_row_to_aggr_row(scip, row, -dualsols[r], proofrow);

                if !*valid {
                    break 'terminate;
                }

                // Update depth where the proof is valid.
                if *validdepth < localrowdepth[i] {
                    *validdepth = localrowdepth[i];
                }

                // Get the new minimal activity.
                (*proofact, infdelta) = aggr_row_get_min_activity(scip, transprob, proofrow, Some(curvarlbs), Some(curvarubs));

                // We stop if the minimal activity is infinite but all variables
                // have a finite activity delta (bad numerics).
                if !infdelta && set.is_infinity(proofact.abs()) {
                    *valid = false;
                    break 'terminate;
                }

                // Break if the proof is valid w.r.t local bounds.
                if !infdelta && set.is_gt(*proofact, crate::cuts::aggr_row_get_rhs(proofrow)) {
                    *valid = true;
                    break;
                }
            }
        }

        // Remove all nearly zero coefficients.
        *valid = crate::cuts::aggr_row_remove_zeros(scip, proofrow, true);
    }

    // TERMINATE:
    if *valid {
        (*proofact, infdelta) = aggr_row_get_min_activity(scip, transprob, proofrow, Some(curvarlbs), Some(curvarubs));

        // We stop if the minimal activity is infinite but all variables have a
        // finite activity delta (bad numerics).
        if (!infdelta && set.is_infinity(proofact.abs())) || infdelta || set.is_le(*proofact, crate::cuts::aggr_row_get_rhs(proofrow)) {
            *valid = false;
        }
    }
}

/// `SCIPgetFarkasProof`: calculates a Farkas proof from the current dual
/// LP solution; returns `(farkasact, validdepth, valid)`: the maximal
/// activity of the proof constraint, its valid depth, and whether it is
/// valid. The LPI is the LP's.
pub fn get_farkas_proof(scip: &mut Scip, prob: usize, farkasrow: &mut crate::cuts::AggrRow, curvarlbs: &[f64], curvarubs: &[f64]) -> ScipResult<(f64, i32, bool)> {
    use crate::lpi::*;

    let mut farkasact = 0.0;
    let mut validdepth = 0;
    let mut valid;

    // Get LP rows and problem variables.
    let rows: Vec<crate::types::RowId> = {
        let lp = scip.lp.as_ref().unwrap();
        lp.rows.0[..lp.nrows as usize].to_vec()
    };
    let nrows = rows.len();

    // It can happen that infeasibility is detetected within LP presolve. in
    // that case, the LP solver may not be able to to return the dual ray.
    if !lpi_has_dual_ray(lp_lpi(scip)) {
        return Ok((farkasact, validdepth, false));
    }

    // Allocate temporary memory.
    let mut dualfarkas = vec![0.0; nrows];
    let mut localrowinds: Vec<i32> = Vec::new();
    let mut localrowdepth: Vec<i32> = Vec::new();

    // Get dual Farkas values of rows.
    lpi_get_dualfarkas(lp_lpi(scip), &mut dualfarkas)?;

    let scip = &*scip;

    // Check whether the Farkas solution is numerically stable.
    for r in 0..nrows {
        if dualfarkas[r].abs() > SOLSTOP {
            return Ok((farkasact, validdepth, false));
        }
    }

    // Calculate the Farkas row.
    validdepth = 0;

    let focusdepth = crate::tree::tree_get_focus_depth(scip);
    for r in 0..nrows {
        let row = rows[r];

        // Ignore dual ray values of 0.0 (in this case: y_i == z_i == 0).
        if dualfarkas[r].abs() > 0.0 {
            // Check dual feasibility.
            let zerocontribution;
            (valid, zerocontribution) = check_dual_feasibility(scip, row, dualfarkas[r]);

            if !valid {
                return Ok((farkasact, validdepth, valid));
            }

            if zerocontribution {
                continue;
            }

            if scip.set.is_dualfeas_zero(dualfarkas[r]) {
                continue;
            }

            if !scip.rows[row].local {
                valid = add_row_to_aggr_row(scip, row, -dualfarkas[r], farkasrow);

                if !valid {
                    return Ok((farkasact, validdepth, valid));
                }

                // Due to numerical reasons we want to stop.
                if crate::cuts::aggr_row_get_rhs(farkasrow).abs() > NUMSTOP {
                    return Ok((farkasact, validdepth, false));
                }
            } else {
                let lpdepth = scip.rows[row].lpdepth;

                if lpdepth < focusdepth {
                    localrowinds.push(r as i32);
                    localrowdepth.push(lpdepth);
                }
            }
        }
    }

    // Remove all coefficients that are too close to zero.
    valid = crate::cuts::aggr_row_remove_zeros(scip, farkasrow, true);

    if !valid {
        return Ok((farkasact, validdepth, valid));
    }

    // Calculate the current Farkas activity, always using the best bound
    // w.r.t. the Farkas coefficient.
    let infdelta;
    (farkasact, infdelta) = aggr_row_get_min_activity(scip, prob, farkasrow, Some(curvarlbs), Some(curvarubs));

    // The constructed proof is not valid, this can happen due to numerical
    // reasons, e.g., we only consider rows r with !SCIPsetIsZero(set,
    // dualfarkas[r]), or because of local rows were ignored so far. Due to
    // the latter case, it might happen at least one variable contributes with
    // an infinite value to the activity (see:
    // https://git.zib.de/integer/scip/issues/2743).
    if infdelta || scip.set.is_feas_le(farkasact, crate::cuts::aggr_row_get_rhs(farkasrow)) {
        // Add contribution of local rows.
        if !localrowinds.is_empty() && scip.set.conf_uselocalrows {
            add_local_rows(
                scip,
                prob,
                farkasrow,
                &rows,
                &dualfarkas,
                &mut localrowinds,
                &mut localrowdepth,
                &mut farkasact,
                &mut validdepth,
                curvarlbs,
                curvarubs,
                &mut valid,
            );
        } else {
            valid = false;
        }
    }

    Ok((farkasact, validdepth, valid))
}

/// `SCIPgetDualProof`: calculates a dual proof from the current dual LP
/// solution; returns `(farkasact, validdepth, valid)`: the maximal activity
/// of the proof constraint, its valid depth, and whether it is valid. The
/// LPI is the LP's.
pub fn get_dual_proof(scip: &mut Scip, transprob: usize, farkasrow: &mut crate::cuts::AggrRow, curvarlbs: &[f64], curvarubs: &[f64]) -> ScipResult<(f64, i32, bool)> {
    use crate::lpi::*;

    let mut farkasact = 0.0;
    let mut validdepth = 0;
    let mut valid;

    // Get LP rows and problem variables.
    let rows: Vec<crate::types::RowId> = {
        let lp = scip.lp.as_ref().unwrap();
        lp.rows.0[..lp.nrows as usize].to_vec()
    };
    let nrows = rows.len();
    let ncols = crate::lp::lp_get_n_cols(scip) as usize;

    // Get temporary memory.
    let mut primsols = vec![0.0; ncols];
    let mut dualsols = vec![0.0; nrows];
    let mut redcosts = vec![0.0; ncols];
    let mut localrowinds: Vec<i32> = Vec::new();
    let mut localrowdepth: Vec<i32> = Vec::new();

    // Get solution from LPI.
    match lpi_get_sol(lp_lpi(scip), None, Some(&mut primsols), Some(&mut dualsols), None, Some(&mut redcosts)) {
        // On an error in the LP solver, just abort the conflict analysis.
        Err(Retcode::LpError) => return Ok((farkasact, validdepth, false)),
        r => r?,
    }

    let scip = &*scip;

    // Check whether the dual solution is numerically stable.
    for r in 0..nrows {
        if dualsols[r].abs() > SOLSTOP {
            return Ok((farkasact, validdepth, false));
        }
    }

    // Clear the proof.
    crate::cuts::aggr_row_clear(farkasrow);

    // Let y be the dual solution and r be the reduced cost vector. Let z be
    // defined as
    //   z_i := y_i if i is a global row,
    //   z_i := 0   if i is a local row.
    // Define the set X := {x | lhs <= Ax <= rhs, lb <= x <= ub, c^Tx <= c*},
    // with c* being the current primal bound. Then the following
    // inequalities are valid for all x \in X:
    //                      - c* <= -c^Tx
    //   <=>        z^TAx - c* <= (z^TA - c^T) x
    //   <=>        z^TAx - c* <= (y^TA - c^T - (y-z)^TA) x
    //   <=>        z^TAx - c* <= (-r^T - (y-z)^TA) x  (dual feasibility of (y,r): y^TA + r^T == c^T)
    // Because lhs <= Ax <= rhs and lb <= x <= ub, the inequality can be
    // relaxed to give
    //   min{z^Tq | lhs <= q <= rhs} - c* <= max{(-r^T - (y-z)^TA) x | lb <= x <= ub}, or X = {}.
    //
    // The resulting dual row is:  z^T{lhs,rhs} - c* <= (-r^T - (y-z)^TA){lb,ub},
    // where lhs, rhs, lb, and ub are selected in order to maximize the
    // feasibility of the row.

    // Add the objective function to the aggregation row with respect to the
    // current cutoff bound; for an integral objective the right-hand side is
    // reduced by the cutoff bound delta to cut off up to the next possible
    // objective value below the cutoff bound.
    let cutoffbound = scip.lp.as_ref().unwrap().cutoffbound;
    let delta = if scip.probs[transprob].objisintegral { scip.set.cutoffbounddelta() } else { 0.0 };
    crate::cuts::aggr_row_add_objective_function(scip, farkasrow, cutoffbound - delta, 1.0);

    // Dual row: z^T{lhs,rhs} - c* <= (-r^T - (y-z)^TA){lb,ub}; process rows:
    // add z^T{lhs,rhs} to the dual row's left hand side, and -(y-z)^TA to the
    // dual row's coefficients.
    let focusdepth = crate::tree::tree_get_focus_depth(scip);
    for r in 0..nrows {
        let row = rows[r];

        // Ignore dual solution values of 0.0 (in this case: y_i == z_i == 0).
        if dualsols[r].abs() > 0.0 {
            // Check dual feasibility.
            let zerocontribution;
            (valid, zerocontribution) = check_dual_feasibility(scip, row, dualsols[r]);

            if !valid {
                return Ok((farkasact, validdepth, valid));
            }

            if zerocontribution {
                continue;
            }

            if scip.set.is_dualfeas_zero(dualsols[r]) {
                continue;
            }

            // Skip local row.
            if !scip.rows[row].local {
                valid = add_row_to_aggr_row(scip, row, -dualsols[r], farkasrow);

                if !valid {
                    return Ok((farkasact, validdepth, valid));
                }

                // Due to numerical reasons we want to stop.
                if crate::cuts::aggr_row_get_rhs(farkasrow).abs() > NUMSTOP {
                    return Ok((farkasact, validdepth, false));
                }
            } else {
                let lpdepth = scip.rows[row].lpdepth;

                if lpdepth < focusdepth {
                    localrowinds.push(r as i32);
                    localrowdepth.push(lpdepth);
                }
            }
        }
    }

    // Remove all nearly zero coefficients.
    valid = crate::cuts::aggr_row_remove_zeros(scip, farkasrow, true);

    if !valid {
        return Ok((farkasact, validdepth, valid));
    }

    // Check validity of the proof.
    let infdelta;
    (farkasact, infdelta) = aggr_row_get_min_activity(scip, transprob, farkasrow, Some(curvarlbs), Some(curvarubs));

    // The constructed proof is not valid, this can happen due to numerical
    // reasons, e.g., we only consider rows r with !SCIPsetIsZero(set,
    // dualsol[r]), or because of local rows were ignored so far. Due to the
    // latter case, it might happen at least one variable contributes with an
    // infinite value to the activity (see:
    // https://git.zib.de/integer/scip/issues/2743).
    if infdelta || scip.set.is_feas_le(farkasact, crate::cuts::aggr_row_get_rhs(farkasrow)) {
        // Add contribution of local rows.
        if !localrowinds.is_empty() && scip.set.conf_uselocalrows {
            add_local_rows(
                scip,
                transprob,
                farkasrow,
                &rows,
                &dualsols,
                &mut localrowinds,
                &mut localrowdepth,
                &mut farkasact,
                &mut validdepth,
                curvarlbs,
                curvarubs,
                &mut valid,
            );
        } else {
            valid = false;
        }
    }

    Ok((farkasact, validdepth, valid))
}

/// `SCIPconflictAnalyzeRemainingBdchgs`: analyzes conflicting bound
/// changes that remain after the bound heuristic or the pseudo solution
/// analysis undid what it could.
pub fn conflict_analyze_remaining_bdchgs(scip: &mut Scip, prob: usize, diving: bool, lbchginfoposs: &[i32], ubchginfoposs: &[i32]) -> ScipResult<ConflictCounts> {
    let mut counts = ConflictCounts::default();

    let nvars = scip.probs[prob].nvars as usize;
    let maxsize = 2 * conflict_calc_maxsize(scip, prob);

    // Initialize conflict data.
    let conftype = cf!(scip).conflictset.conflicttype;
    let usescutoffbound = cf!(scip).conflictset.usescutoffbound;
    conflict_init(scip, prob, conftype, usescutoffbound)?;

    cf!(scip).conflictset.conflicttype = conftype;
    cf!(scip).conflictset.usescutoffbound = usescutoffbound;

    // Add remaining bound changes to conflict queue.
    let mut nbdchgs = 0;
    let mut v = 0;
    while v < nvars && nbdchgs < maxsize {
        let var = scip.probs[prob].vars[v];
        let (nlb, nub) = (scip.vars[var].nlbchginfos, scip.vars[var].nubchginfos);

        if lbchginfoposs[v] == nlb || ubchginfoposs[v] == nub {
            // The strong branching or diving bound stored in the column is
            // responsible for the conflict: it cannot be resolved and therefore
            // has to be directly put into the conflict set.

            // Create an artificial bound change information for the
            // diving/strong branching bound change; they are freed in the
            // SCIPconflictFlushConss() call.
            let (info, relaxedbd);
            if lbchginfoposs[v] == nlb {
                let (lbloc, lblp) = (scip.vars[var].locdom.lb, crate::var::var_get_lb_lp(scip, var));
                info = conflict_create_tmp_bdchginfo(scip, var, BoundType::Lower, lbloc, lblp);
                relaxedbd = lblp;
            } else {
                let (ubloc, ublp) = (scip.vars[var].locdom.ub, crate::var::var_get_ub_lp(scip, var));
                info = conflict_create_tmp_bdchginfo(scip, var, BoundType::Upper, ubloc, ublp);
                relaxedbd = ublp;
            }

            // Put variable into the conflict set.
            conflict_add_conflict_bound(scip, info, relaxedbd);

            // Each variable which is add to the conflict graph gets an increase
            // in the VSIDS; that is different to the VSIDS preseted in the
            // literature.
            let boundtype = bdchginfo_get_boundtype(scip, info);
            let w = scip.set.conf_conflictgraphweight;
            inc_vsids(scip, var, boundtype, relaxedbd, w)?;
            nbdchgs += 1;
        } else {
            // Put remaining bound changes into conflict candidate queue.
            if lbchginfoposs[v] >= 0 {
                let info = BdChgInfoRef::Var {
                    var,
                    boundtype: BoundType::Lower,
                    pos: lbchginfoposs[v],
                };
                let nb = bdchginfo_get_newbound(scip, info);
                conflict_add_bound(scip, var, BoundType::Lower, info, nb)?;
                nbdchgs += 1;
            }
            if ubchginfoposs[v] >= 0 {
                let info = BdChgInfoRef::Var {
                    var,
                    boundtype: BoundType::Upper,
                    pos: ubchginfoposs[v],
                };
                let nb = bdchginfo_get_newbound(scip, info);
                conflict_add_bound(scip, var, BoundType::Upper, info, nb)?;
                nbdchgs += 1;
            }
        }
        v += 1;
    }

    if v == nvars {
        // Check if the conflict analysis is applicable.
        if conflict_graph_applicable(scip) {
            // Analyze the conflict set, and create conflict constraints on
            // success.
            counts = conflict_analyze(scip, prob, diving, 0, false)?;
        }
    }

    Ok(counts)
}

/// `SCIPconflictAnalyzePseudo`: analyzes a pseudo solution with objective
/// value exceeding the current cutoff to find out the bound changes on
/// variables that were responsible for the objective value degradation; on
/// success, calls standard conflict analysis with the responsible variables
/// as starting conflict set, thus creating a conflict constraint out of the
/// resulting conflict set; updates statistics for pseudo solution conflict
/// analysis; returns whether a conflict constraint was created.
pub fn conflict_analyze_pseudo(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult<bool> {
    let mut success = false;

    // Check, if pseudo solution conflict analysis is enabled.
    if !scip.set.conf_enable || !scip.set.conf_usepseudo {
        return Ok(false);
    }

    // Check, if there are any conflict handlers to use a conflict set.
    if scip.set.nconflicthdlrs == 0 {
        return Ok(false);
    }

    cf!(scip).conflictset.conflicttype = ConfType::BndExceeding;
    cf!(scip).conflictset.usescutoffbound = true;

    // Start timing.
    let te = scip.set.time_enabled;
    cf!(scip).pseudoanalyzetime.start(te);
    cf!(scip).npseudocalls += 1;

    let nvars = scip.probs[transprob].nvars as usize;

    // The current primal bound c* gives an upper bound for the current pseudo
    // objective value:
    //   min{c^T x | lb <= x <= ub} <= c*.
    // We have to transform this row into a >= inequality in order to use
    // methods above:
    //   -c* <= max{-c^T x | lb <= x <= ub}.
    // In the local subproblem, this row is violated. We want to undo bound
    // changes while still keeping the row violated.

    // Get temporary memory for remembering variables' current bounds and
    // corresponding bound change information positions in variable's bound
    // change information arrays.
    let mut curvarlbs = vec![0.0; nvars];
    let mut curvarubs = vec![0.0; nvars];
    let mut lbchginfoposs = vec![0; nvars];
    let mut ubchginfoposs = vec![0; nvars];

    // Get temporary memory for infeasibility proof coefficients.
    let mut pseudocoefs = vec![0.0; nvars];

    // For an integral objective use the cutoff bound reduced by the cutoff
    // bound delta to cut off up to the next better objective value.
    let cutoffbound = scip.lp.as_ref().unwrap().cutoffbound;
    let pseudolhs = -(cutoffbound - if scip.probs[transprob].objisintegral { scip.set.cutoffbounddelta() } else { 0.0 });

    // Store the objective values as infeasibility proof coefficients, and
    // recalculate the pseudo activity.
    let mut pseudoact = 0.0;
    for v in 0..nvars {
        let var = scip.probs[transprob].vars[v];
        let x = &scip.vars[var];
        pseudocoefs[v] = -x.obj;
        curvarlbs[v] = x.locdom.lb;
        curvarubs[v] = x.locdom.ub;
        lbchginfoposs[v] = x.nlbchginfos - 1;
        ubchginfoposs[v] = x.nubchginfos - 1;

        if scip.set.is_zero(pseudocoefs[v]) {
            pseudocoefs[v] = 0.0;
            continue;
        }

        if pseudocoefs[v] > 0.0 {
            pseudoact += pseudocoefs[v] * curvarubs[v];
        } else {
            pseudoact += pseudocoefs[v] * curvarlbs[v];
        }
    }

    // Check, if the pseudo row is still violated (after recalculation of
    // pseudo activity).
    if scip.set.is_feas_gt(pseudolhs, pseudoact) {
        cf!(scip).bdchgonlyconfqueue = true;

        // Undo bound changes without destroying the infeasibility proof.
        let currentdepth = crate::tree::tree_get_current_depth(scip);
        undo_bdchgs_proof(
            scip,
            transprob,
            currentdepth,
            &mut pseudocoefs,
            pseudolhs,
            &mut pseudoact,
            &mut curvarlbs,
            &mut curvarubs,
            &mut lbchginfoposs,
            &mut ubchginfoposs,
            None,
        );

        // Analyze conflict on remaining bound changes.
        let n = conflict_analyze_remaining_bdchgs(scip, transprob, false, &lbchginfoposs, &ubchginfoposs)?;
        let c = &mut cf!(scip);
        c.npseudosuccess += if n.nconss > 0 { 1 } else { 0 };
        c.npseudoconfconss += n.nconss as i64;
        c.npseudoconfliterals += n.nliterals as i64;
        c.npseudoreconvconss += n.nreconvconss as i64;
        c.npseudoreconvliterals += n.nreconvliterals as i64;
        success = n.nconss > 0;

        c.bdchgonlyconfqueue = false;
    }

    // Flush conflict set storage.
    conflict_flush_conss(scip, transprob, origprob, eventfilter)?;

    // Stop timing.
    cf!(scip).pseudoanalyzetime.stop(te);

    Ok(success)
}

/// The outcome of `conflictAnalyzeLP`.
#[derive(Default)]
struct LpConflictResult {
    dualproofsuccess: bool,
    iterations: i32,
    counts: ConflictCounts,
}

/// `conflictAnalyzeLP`: actually performs analysis of infeasible LP.
fn conflict_analyze_lp(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize, diving: bool, marklpunsolved: bool) -> ScipResult<LpConflictResult> {
    use crate::lpi::*;

    let mut res = LpConflictResult::default();

    let nvars = scip.probs[transprob].nvars as usize;

    let mut valid;
    let validdepth;

    // Get LP solver interface.
    if !lpi_is_primal_infeasible(lp_lpi(scip)) {
        // Make sure, a dual feasible solution exists, that exceeds the
        // objective limit; With FASTMIP setting, CPLEX does not apply the final
        // pivot to reach the dual solution exceeding the objective limit.
        // Therefore, we have to either turn off FASTMIP and resolve the problem
        // or continue solving it without objective limit for at least one
        // iteration. It seems that the strategy to continue with FASTMIP for
        // one additional simplex iteration yields better results.
        let objval = lpi_get_objval(lp_lpi(scip))?;
        if objval < lp!(scip).lpiobjlim {
            // Temporarily disable objective limit and install an iteration
            // limit.
            let inf = lpi_infinity(lp_lpi(scip));
            lpi_set_realpar(lp_lpi(scip), LpParam::ObjLim, inf)?;
            lpi_set_intpar(lp_lpi(scip), LpParam::LpItLim, 1)?;

            // Start LP timer.
            scip.stat.conflictlptime.start(scip.set.time_enabled);

            // Resolve LP.
            let retcode = lpi_solve_dual(lp_lpi(scip));

            // Stop LP timer.
            scip.stat.conflictlptime.stop(scip.set.time_enabled);

            // Check return code of LP solving call.
            valid = !matches!(retcode, Err(Retcode::LpError));
            if valid {
                retcode?;

                // Count number of LP iterations.
                let iter = lpi_get_iterations(lp_lpi(scip))?;
                res.iterations += iter;
                scip.stat.nconflictlps += 1;
                scip.stat.nconflictlpiterations += iter as i64;
                let lpi = lp_lpi(scip);
                valid = lpi_is_objlim_exc(lpi) || lpi_is_primal_infeasible(lpi) || lpi_is_dual_feasible(lpi);
            }

            // Reinstall old objective and iteration limits in LP solver.
            let (objlim, itlim) = (lp!(scip).lpiobjlim, lp!(scip).lpiitlim);
            lpi_set_realpar(lp_lpi(scip), LpParam::ObjLim, objlim)?;
            lpi_set_intpar(lp_lpi(scip), LpParam::LpItLim, itlim)?;

            // Abort, if the LP produced an error.
            if !valid {
                return Ok(res);
            }
        }
    }

    if !lpi_is_primal_infeasible(lp_lpi(scip)) {
        let objval = lpi_get_objval(lp_lpi(scip))?;
        if objval < lp!(scip).lpiobjlim {
            return Ok(res);
        }
    }

    let mut lbchginfoposs = vec![0; nvars];
    let mut ubchginfoposs = vec![0; nvars];

    // Get temporary memory for remembering variables' current bounds and
    // corresponding bound change information positions in variable's bound
    // change information arrays.
    let mut curvarlbs = vec![0.0; nvars];
    let mut curvarubs = vec![0.0; nvars];

    // Get current bounds and current positions in lb/ubchginfos arrays of
    // variables.
    valid = true;
    let mut v = 0;
    while v < nvars && valid {
        let var = scip.probs[transprob].vars[v];

        curvarlbs[v] = crate::var::var_get_lb_lp(scip, var);
        curvarubs[v] = crate::var::var_get_ub_lp(scip, var);
        let x = &scip.vars[var];
        lbchginfoposs[v] = x.nlbchginfos - 1;
        ubchginfoposs[v] = x.nubchginfos - 1;

        // Check, if last bound changes were due to strong branching or diving.
        if diving {
            let lb = x.locdom.lb;
            let ub = x.locdom.ub;
            if scip.set.is_gt(curvarlbs[v], lb) {
                lbchginfoposs[v] = x.nlbchginfos;
            } else if scip.set.is_lt(curvarlbs[v], lb) {
                // The bound in the diving LP was relaxed -> the LP is not a
                // subproblem of the current node -> abort!
                // @todo we could still analyze such a conflict, but we would
                // have to take care with our data structures.
                valid = false;
            }
            if scip.set.is_lt(curvarubs[v], ub) {
                ubchginfoposs[v] = x.nubchginfos;
            } else if scip.set.is_gt(curvarubs[v], ub) {
                // The bound in the diving LP was relaxed -> the LP is not a
                // subproblem of the current node -> abort!
                // @todo we could still analyze such a conflict, but we would
                // have to take care with our data structures.
                valid = false;
            }
        }
        v += 1;
    }

    if !valid {
        return Ok(res);
    }

    let mut farkasrow = crate::cuts::aggr_row_create(scip);

    // The LP is proven to be infeasible.
    let mut farkasactivity;
    if lpi_is_primal_infeasible(lp_lpi(scip)) {
        (farkasactivity, validdepth, valid) = get_farkas_proof(scip, transprob, &mut farkasrow, &curvarlbs, &curvarubs)?;
    }
    // The LP is dual feasible and/or exceeds the current incumbant solution.
    else {
        (farkasactivity, validdepth, valid) = get_dual_proof(scip, transprob, &mut farkasrow, &curvarlbs, &curvarubs)?;
    }

    if !valid || validdepth >= crate::tree::tree_get_current_depth(scip) {
        return Ok(res);
    }

    let mut globalinfeasible = false;

    let conftype = cf!(scip).conflictset.conflicttype;
    let (useinflp, useboundlp) = (scip.set.conf_useinflp, scip.set.conf_useboundlp);

    // Start dual proof analysis.
    if ((useinflp == b'b' || useinflp == b'd') && conftype == ConfType::InfeasLp) || ((useboundlp == b'b' || useboundlp == b'd') && conftype == ConfType::BndExceeding) {
        // Start dual proof analysis.
        (globalinfeasible, res.dualproofsuccess) = conflict_analyze_dual_proof(scip, eventfilter, origprob, transprob, &mut farkasrow, validdepth, &curvarlbs, &curvarubs, true)?;
    }

    // todo: in theory, we could apply conflict graph analysis for locally
    // valid proofs, too, but this needs to be implemented; also conflict graph
    // analysis is not implemented in exact solving mode, yet.
    'flushproofsets: {
        if !globalinfeasible
            && validdepth <= crate::tree::tree_get_effective_root_depth(scip)
            && (((useinflp == b'b' || useinflp == b'c') && conftype == ConfType::InfeasLp) || ((useboundlp == b'b' || useboundlp == b'c') && conftype == ConfType::BndExceeding))
        {
            let mut farkascoefs = vec![0.0; nvars];
            let mut farkaslhs = -crate::cuts::aggr_row_get_rhs(&farkasrow);
            farkasactivity = -farkasactivity;

            let nnz = crate::cuts::aggr_row_get_n_nz(&farkasrow) as usize;
            for v in 0..nnz {
                let i = farkasrow.inds[v];
                farkascoefs[i as usize] = -crate::cuts::aggr_row_get_probvar_value(&farkasrow, i);
            }

            let d = BoundHeuristicData {
                proofcoefs: &mut farkascoefs,
                prooflhs: &mut farkaslhs,
                proofactivity: &mut farkasactivity,
                curvarlbs: &mut curvarlbs,
                curvarubs: &mut curvarubs,
                lbchginfoposs: &mut lbchginfoposs,
                ubchginfoposs: &mut ubchginfoposs,
            };
            run_bound_heuristic(
                scip,
                origprob,
                transprob,
                eventfilter,
                d,
                &mut res.iterations,
                marklpunsolved,
                &mut res.dualproofsuccess,
                &mut valid,
            )?;

            cf!(scip).bdchgonlyconfqueue = false;

            if !valid {
                break 'flushproofsets;
            }

            cf!(scip).bdchgonlyconfqueue = true;

            // Analyze the conflict starting with remaining bound changes.
            res.counts = conflict_analyze_remaining_bdchgs(scip, transprob, diving, &lbchginfoposs, &ubchginfoposs)?;

            conflict_flush_conss(scip, transprob, origprob, eventfilter)?;

            cf!(scip).bdchgonlyconfqueue = false;
        }
    }

    // FLUSHPROOFSETS:
    // Flush proof set.
    if proofset_get_n_vars(&cf!(scip).proofset) > 0 || cf!(scip).nproofsets > 0 {
        conflict_flush_proofset(scip, transprob, origprob, eventfilter)?;
    }

    Ok(res)
}

/// One strong branching direction of `SCIPconflictAnalyzeStrongbranch`:
/// resolves the LP with the column's bound changed and analyzes it;
/// returns whether a conflict constraint was created.
fn conflict_analyze_strongbranch_dir(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    eventfilter: usize,
    col: crate::types::ColId,
    cstat: &[i32],
    rstat: &[i32],
    upper: bool,
    newbound: f64,
) -> ScipResult<bool> {
    use crate::lpi::*;
    let mut conflict = false;

    cf!(scip).conflictset.conflicttype = ConfType::InfeasLp;
    cf!(scip).nsbcalls += 1;

    // Change the bound.
    let old = if upper { scip.cols[col].ub } else { scip.cols[col].lb };
    if upper {
        scip.cols[col].ub = newbound;
    } else {
        scip.cols[col].lb = newbound;
    }
    let chg = |scip: &mut Scip| {
        let c = &scip.cols[col];
        let (pos, lb, ub) = (c.lpipos, c.lb, c.ub);
        lpi_chg_bounds(lp_lpi(scip), &[pos], &[lb], &[ub])
    };
    chg(scip)?;

    // Start LP timer.
    scip.stat.conflictlptime.start(scip.set.time_enabled);

    // Resolve the LP.
    let retcode = lpi_solve_dual(lp_lpi(scip));

    // Stop LP timer.
    scip.stat.conflictlptime.stop(scip.set.time_enabled);

    // Check return code of LP solving call.
    let lperror = matches!(retcode, Err(Retcode::LpError));
    if !lperror {
        retcode?;
    }
    if !lperror && lpi_is_stable(lp_lpi(scip)) {
        // Count number of LP iterations.
        let iter = lpi_get_iterations(lp_lpi(scip))?;
        scip.stat.nconflictlps += 1;
        scip.stat.nconflictlpiterations += iter as i64;
        cf!(scip).nsbiterations += iter as i64;

        // Perform conflict analysis on infeasible LP; last parameter guarantees
        // status 'solved' on return.
        let res = conflict_analyze_lp(scip, transprob, origprob, eventfilter, true, false)?;
        let c = &mut cf!(scip);
        c.nsbsuccess += if res.counts.nconss > 0 || res.dualproofsuccess { 1 } else { 0 };
        c.nsbiterations += res.iterations as i64;
        c.nsbconfconss += res.counts.nconss as i64;
        c.nsbconfliterals += res.counts.nliterals as i64;
        c.nsbreconvconss += res.counts.nreconvconss as i64;
        c.nsbreconvliterals += res.counts.nreconvliterals as i64;
        conflict = res.counts.nconss > 0;
    }

    // Reset the bound.
    if upper {
        scip.cols[col].ub = old;
    } else {
        scip.cols[col].lb = old;
    }
    chg(scip)?;

    // Reset LP basis.
    lpi_set_base(lp_lpi(scip), Some(cstat), Some(rstat))?;

    Ok(conflict)
}

/// `SCIPconflictAnalyzeStrongbranch`: analyses infeasible strong branching
/// sub problems for conflicts; returns whether a conflict constraint was
/// created for an infeasible downwards and upwards branch.
pub fn conflict_analyze_strongbranch(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize, col: crate::types::ColId) -> ScipResult<(bool, bool)> {
    use crate::lpi::*;

    let mut downconflict = false;
    let mut upconflict = false;

    // Check, if infeasible LP conflict analysis is enabled.
    if !scip.set.conf_enable || !scip.set.conf_usesb {
        return Ok((false, false));
    }

    // Check, if there are any conflict handlers to use a conflict set.
    if scip.set.nconflicthdlrs == 0 {
        return Ok((false, false));
    }

    // Inform the LPI that strong branch is (temporarily) finished.
    lpi_end_strongbranch(lp_lpi(scip))?;

    // Start timing.
    let te = scip.set.time_enabled;
    cf!(scip).sbanalyzetime.start(te);

    // Get temporary memory for storing current LP basis.
    let mut cstat = vec![0; lp!(scip).nlpicols as usize];
    let mut rstat = vec![0; lp!(scip).nlpirows as usize];

    // Get current LP basis.
    lpi_get_base(lp_lpi(scip), Some(&mut cstat), Some(&mut rstat))?;

    let mut resolve = false;

    // Is down branch infeasible?
    let cutoffbound = lp!(scip).cutoffbound;
    let c = &scip.cols[col];
    if c.sbdownvalid && scip.set.is_ge(c.sbdown, cutoffbound) {
        let newub = scip.set.feas_ceil(c.primsol - 1.0);
        if newub >= c.lb - 0.5 {
            downconflict = conflict_analyze_strongbranch_dir(scip, transprob, origprob, eventfilter, col, &cstat, &rstat, true, newub)?;

            // Mark the LP to be resolved at the end.
            resolve = true;
        }
    }

    // Is up branch infeasible?
    let cutoffbound = lp!(scip).cutoffbound;
    let c = &scip.cols[col];
    if c.sbupvalid && scip.set.is_ge(c.sbup, cutoffbound) {
        let newlb = scip.set.feas_floor(c.primsol + 1.0);
        if newlb <= c.ub + 0.5 {
            upconflict = conflict_analyze_strongbranch_dir(scip, transprob, origprob, eventfilter, col, &cstat, &rstat, false, newlb)?;

            // Mark the LP to be resolved at the end.
            resolve = true;
        }
    }

    // Resolve LP if something has changed in order to synchronize LPI and LP.
    if resolve {
        // Start LP timer.
        scip.stat.conflictlptime.start(te);

        // Resolve the LP.
        lpi_solve_dual(lp_lpi(scip))?;

        // Stop LP timer.
        scip.stat.conflictlptime.stop(te);
    }

    // Stop timing.
    cf!(scip).sbanalyzetime.stop(te);

    // Inform the LPI that strong branch starts (again).
    lpi_start_strongbranch(lp_lpi(scip))?;

    Ok((downconflict, upconflict))
}

/// `conflictAnalyzeInfeasibleLP`: analyzes an infeasible LP to find out
/// the bound changes on variables that were responsible for the
/// infeasibility; on success, calls standard conflict analysis with the
/// responsible variables as starting conflict set, thus creating a conflict
/// constraint out of the resulting conflict set; updates statistics for
/// infeasible LP conflict analysis; returns whether a conflict constraint
/// was created.
fn conflict_analyze_infeasible_lp(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult<bool> {
    // Check, if infeasible LP conflict analysis is enabled.
    if !scip.set.conf_enable || scip.set.conf_useinflp == b'o' {
        return Ok(false);
    }

    // Check, if there are any conflict handlers to use a conflict set.
    if scip.set.nconflicthdlrs == 0 {
        return Ok(false);
    }

    // Start timing.
    let te = scip.set.time_enabled;
    cf!(scip).inflpanalyzetime.start(te);
    cf!(scip).ninflpcalls += 1;

    // @todo reset usescutoffbound flag.
    cf!(scip).conflictset.conflicttype = ConfType::InfeasLp;

    let olddualproofsuccess = cf!(scip).ndualproofsinfsuccess;

    // Perform conflict analysis.
    let diving = lp!(scip).diving;
    let res = conflict_analyze_lp(scip, transprob, origprob, eventfilter, diving, true)?;
    let c = &mut cf!(scip);
    let success = res.counts.nconss > 0 || c.ndualproofsinfsuccess > olddualproofsuccess;
    c.ninflpsuccess += if success { 1 } else { 0 };
    c.ninflpiterations += res.iterations as i64;
    c.ninflpconfconss += res.counts.nconss as i64;
    c.ninflpconfliterals += res.counts.nliterals as i64;
    c.ninflpreconvconss += res.counts.nreconvconss as i64;
    c.ninflpreconvliterals += res.counts.nreconvliterals as i64;

    // Stop timing.
    c.inflpanalyzetime.stop(te);

    Ok(success)
}

/// `conflictAnalyzeBoundexceedingLP`: analyzes a bound exceeding LP to
/// find out the bound changes on variables that were responsible for
/// exceeding the primal bound; on success, calls standard conflict analysis
/// with the responsible variables as starting conflict set, thus creating a
/// conflict constraint out of the resulting conflict set; updates
/// statistics for bound exceeding LP conflict analysis; returns whether a
/// conflict constraint was created.
fn conflict_analyze_boundexceeding_lp(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult<bool> {
    // Check, if bound exceeding LP conflict analysis is enabled.
    if !scip.set.conf_enable || scip.set.conf_useboundlp == b'o' {
        return Ok(false);
    }

    // Check, if there are any conflict handlers to use a conflict set.
    if scip.set.nconflicthdlrs == 0 {
        return Ok(false);
    }

    // Start timing.
    let te = scip.set.time_enabled;
    cf!(scip).boundlpanalyzetime.start(te);
    cf!(scip).nboundlpcalls += 1;

    // Mark the conflict to depend on the cutoff bound.
    cf!(scip).conflictset.conflicttype = ConfType::BndExceeding;
    cf!(scip).conflictset.usescutoffbound = true;

    let oldnsuccess = cf!(scip).ndualproofsbndsuccess + cf!(scip).ndualproofsinfsuccess;

    // Perform conflict analysis.
    let diving = lp!(scip).diving;
    let res = conflict_analyze_lp(scip, transprob, origprob, eventfilter, diving, true)?;
    let c = &mut cf!(scip);
    let success = res.counts.nconss > 0 || c.ndualproofsbndsuccess + c.ndualproofsinfsuccess > oldnsuccess;
    c.nboundlpsuccess += if success { 1 } else { 0 };
    c.nboundlpiterations += res.iterations as i64;
    c.nboundlpconfconss += res.counts.nconss as i64;
    c.nboundlpconfliterals += res.counts.nliterals as i64;
    c.nboundlpreconvconss += res.counts.nreconvconss as i64;
    c.nboundlpreconvliterals += res.counts.nreconvliterals as i64;

    // Stop timing.
    c.boundlpanalyzetime.stop(te);

    Ok(success)
}

/// `SCIPconflictAnalyzeLP`: analyzes an infeasible or bound exceeding LP
/// to find out the bound changes on variables that were responsible for the
/// infeasibility or for exceeding the primal bound; on success, calls
/// standard conflict analysis with the responsible variables as starting
/// conflict set, thus creating a conflict constraint out of the resulting
/// conflict set; updates statistics for infeasible or bound exceeding LP
/// conflict analysis; may only be called if SCIPprobAllColsInLP(); returns
/// whether a conflict constraint was created.
pub fn conflict_analyze_lp_public(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult<bool> {
    use crate::types::LpSolStat;

    // Check if the conflict analysis is applicable.
    if !scip.set.conf_enable || (scip.set.conf_useinflp == b'o' && scip.set.conf_useboundlp == b'o') {
        return Ok(false);
    }

    // In rare cases, it might happen that the solution stati of the LP and the
    // LPI are out of sync; in particular this happens when a new incumbent
    // which cuts off the current node is found during the LP solving loop; in
    // this case the LP has status objlimit, but if diving has been used, the
    // LPI only has the basis information, but is not solved.
    //
    // @todo: alternatively, solve the LPI
    if !crate::lpi::lpi_was_solved(lp_lpi(scip)) {
        return Ok(false);
    }

    // Save status.
    let lp = &lp!(scip);
    let stored = (
        lp.lpsolstat,
        lp.lpobjval,
        lp.primalfeasible,
        lp.primalchecked,
        lp.dualfeasible,
        lp.dualchecked,
        lp.solisbasic,
        lp.solved,
    );

    // Store solution values.
    let infeasible = lp.lpsolstat == LpSolStat::Infeasible;
    let storedcolsolvals: Vec<(f64, f64, u32)> = (0..lp.ncols as usize)
        .map(|c| {
            let col = &scip.cols[lp.cols[c]];
            (col.primsol, col.redcost, col.basisstatus)
        })
        .collect();
    let storedrowsolvals: Vec<(f64, f64, u32)> = (0..lp.nrows as usize)
        .map(|r| {
            let row = &scip.rows[lp.rows[r]];
            (if infeasible { row.dualfarkas } else { row.dualsol }, row.activity, row.basisstatus)
        })
        .collect();

    // Check, if the LP was infeasible or bound exceeding.
    let success = if crate::lpi::lpi_is_primal_infeasible(lp_lpi(scip)) {
        conflict_analyze_infeasible_lp(scip, transprob, origprob, eventfilter)?
    } else {
        conflict_analyze_boundexceeding_lp(scip, transprob, origprob, eventfilter)?
    };

    // Possibly restore solution values.
    if lp!(scip).flushed && lp!(scip).lpsolstat == LpSolStat::NotSolved {
        // Restore status.
        let lp = &mut lp!(scip);
        (
            lp.lpsolstat,
            lp.lpobjval,
            lp.primalfeasible,
            lp.primalchecked,
            lp.dualfeasible,
            lp.dualchecked,
            lp.solisbasic,
            lp.solved,
        ) = stored;
        let infeasible = lp.lpsolstat == LpSolStat::Infeasible;

        let lp = scip.lp.as_ref().unwrap();
        for c in 0..lp.ncols as usize {
            let col = &mut scip.cols[lp.cols[c]];
            (col.primsol, col.redcost, col.basisstatus) = storedcolsolvals[c];
        }
        for r in 0..lp.nrows as usize {
            let row = &mut scip.rows[lp.rows[r]];
            let (dualsol, activity, basisstatus) = storedrowsolvals[r];
            if infeasible {
                row.dualfarkas = dualsol;
            } else {
                row.dualsol = dualsol;
            }
            row.activity = activity;
            row.basisstatus = basisstatus;
        }
    }

    Ok(success)
}

/*
 * conflict_dualproofanalysis.c
 */

/// `BOUNDSWITCH`: threshold for bound switching - see cuts.c.
const BOUNDSWITCH: f64 = 0.51;
/// `POSTPROCESS`: apply postprocessing to the cut - see cuts.c.
const POSTPROCESS: bool = false;
/// `VARTYPEUSEVBDS`: we do not allow variable bound substitution - see
/// cuts.c for more information.
const VARTYPEUSEVBDS: i32 = 0;
/// `ALLOWLOCAL`: allow to generate local cuts - see cuts.
const ALLOWLOCAL: bool = false;
/// `MINFRAC`: minimal fractionality of floor(rhs) - see cuts.c.
const MINFRAC: f64 = 0.05;
/// `MAXFRAC`: maximal fractionality of floor(rhs) - see cuts.c.
const MAXFRAC: f64 = 0.999;

/// `proofsetClear`: resets the data structure of a proofset.
fn proofset_clear(p: &mut ProofSet) {
    p.nnz = 0;
    p.rhs = 0.0;
    p.validdepth = 0;
    p.conflicttype = ConfType::Unknown;
    p.certificateline = i64::MAX;
}

/// `proofsetCreate`: creates a proofset.
fn proofset_create() -> Box<ProofSet> {
    Box::new(ProofSet {
        conflicttype: ConfType::Unknown,
        certificateline: i64::MAX,
        ..ProofSet::default()
    })
}

/// `SCIPproofsetGetNVars`: return the number of variables in the proofset.
pub fn proofset_get_n_vars(p: &ProofSet) -> i32 {
    p.nnz
}

/// `proofsetAddSparseData`: adds given data as aggregation row to the
/// proofset.
fn proofset_add_sparse_data(p: &mut ProofSet, vals: &[f64], inds: &[i32], nnz: i32, rhs: f64) {
    let n = nnz as usize;
    if p.size == 0 {
        p.vals = CVec(vals[..n].to_vec());
        p.inds = CVec(inds[..n].to_vec());
        p.size = nnz;
    } else {
        if p.size < nnz {
            p.vals.0.resize(n, 0.0);
            p.inds.0.resize(n, 0);
            p.size = nnz;
        }
        for i in 0..n {
            p.vals[i] = vals[i];
            p.inds[i] = inds[i];
        }
    }
    p.rhs = rhs;
    p.nnz = nnz;
}

/// `proofsetAddAggrrow`: adds an aggregation row to the proofset.
fn proofset_add_aggrrow(p: &mut ProofSet, aggrrow: &crate::cuts::AggrRow) {
    let inds = crate::cuts::aggr_row_get_inds(aggrrow);
    let nnz = crate::cuts::aggr_row_get_n_nz(aggrrow);
    let vals: Vec<f64> = (0..nnz as usize).map(|i| crate::cuts::aggr_row_get_probvar_value(aggrrow, inds[i])).collect();
    proofset_add_sparse_data(p, &vals, inds, nnz, crate::cuts::aggr_row_get_rhs(aggrrow));
}

/// `proofsetCancelVarWithBound`: Removes a given variable `var` from
/// position `pos` from the proofset and updates the right-hand side
/// according to sign of the coefficient, i.e., rhs -= coef * bound, where
/// bound = lb if coef >= 0 and bound = ub, otherwise; returns whether the
/// proofset is still valid. The list of non-zero indices and coefficients
/// will be updated by swapping the last non-zero index to `pos`.
fn proofset_cancel_var_with_bound(scip: &Scip, p: &mut ProofSet, var: VarId, pos: usize) -> bool {
    let mut valid = true;

    // Cancel with lower bound.
    if p.vals[pos] > 0.0 {
        p.rhs -= p.vals[pos] * scip.vars[var].glbdom.lb;
    }
    // Cancel with upper bound.
    else {
        p.rhs -= p.vals[pos] * scip.vars[var].glbdom.ub;
    }

    p.nnz -= 1;
    let n = p.nnz as usize;

    p.vals[pos] = p.vals[n];
    p.inds[pos] = p.inds[n];
    p.vals[n] = 0.0;
    p.inds[n] = 0;

    if scip.set.is_infinity(p.rhs) {
        valid = false;
    }
    valid
}

/// `SCIPconflictInitProofset`: creates and clears the proofset.
pub fn conflict_init_proofset(c: &mut Conflict) {
    c.proofset = proofset_create();
}

/// `conflictInsertProofset`: inserts a proofset into the list of
/// proofsets.
fn conflict_insert_proofset(scip: &mut Scip, proofset: Box<ProofSet>) {
    let num = cf!(scip).nproofsets + 1;
    if num > cf!(scip).proofsetssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        cf!(scip).proofsetssize = newsize;
    }
    let c = &mut cf!(scip);
    c.proofsets.push(proofset);
    c.nproofsets += 1;
}

/// `tightenSingleVar`: tighten the bound of a singleton variable in a
/// constraint; if the bound is contradicting with a global bound we cannot
/// tighten the bound directly. In this case we need to create and add a
/// constraint of size one such that propagating this constraint will
/// enforce the infeasibility.
#[allow(clippy::too_many_arguments)]
fn tighten_single_var(scip: &mut Scip, eventfilter: usize, origprob: usize, transprob: usize, var: VarId, val: f64, rhs: f64, prooftype: ConfType, validdepth: i32) -> ScipResult {
    let applyglobal = validdepth <= crate::tree::tree_get_effective_root_depth(scip);

    // If variable and coefficient are integral the rhs can be rounded down.
    let mut newbound = if crate::var::var_is_integral(scip, var) && scip.set.is_integral(val) {
        scip.set.feas_floor(rhs) / val
    } else {
        rhs / val
    };

    let boundtype = if val > 0.0 { BoundType::Upper } else { BoundType::Lower };
    crate::var::var_adjust_bd(scip, var, boundtype, &mut newbound);

    let (glb, gub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);

    // Skip numerical unstable bound changes.
    if applyglobal && ((boundtype == BoundType::Lower && scip.set.is_le(newbound, glb)) || (boundtype == BoundType::Upper && scip.set.is_ge(newbound, gub))) {
        return Ok(());
    }

    // The new bound contradicts a global bound, we can cutoff the root node
    // immediately.
    if applyglobal && ((boundtype == BoundType::Lower && scip.set.is_gt(newbound, gub)) || (boundtype == BoundType::Upper && scip.set.is_lt(newbound, glb))) {
        let root = treer!(scip).path[0];
        crate::tree::node_cutoff(scip, root, eventfilter, transprob, origprob)?;
    } else if lp!(scip).strongbranching || !applyglobal {
        let name = format!("pc_fix_{}", scip.vars[var].name);

        let (conslhs, consrhs) = if boundtype == BoundType::Upper {
            (-scip.set.infinity(), newbound)
        } else {
            (newbound, scip.set.infinity())
        };

        let cons = crate::cons_linear::create_cons_linear(
            scip,
            &name,
            &[],
            &[],
            conslhs,
            consrhs,
            false,
            false,
            false,
            false,
            true,
            !applyglobal,
            false,
            true,
            true,
            false,
        )?;

        crate::cons_linear::add_coef_linear(scip, cons, var, 1.0)?;

        if applyglobal {
            crate::prob::prob_add_cons(scip, transprob, cons)?;
        } else {
            let node = treer!(scip).path[validdepth];
            crate::tree::node_add_cons(scip, node, cons)?;
        }

        crate::cons::cons_release(scip, cons)?;
    } else {
        let root = treer!(scip).path[0];
        crate::tree::node_add_boundchg(scip, root, transprob, origprob, eventfilter, var, newbound, boundtype, false)?;

        // Mark the node in the validdepth to be propagated again.
        crate::tree::node_propagate_again(scip, root);
    }

    let c = &mut cf!(scip);
    if applyglobal {
        c.nglbchgbds += 1;
    } else {
        c.nlocchgbds += 1;
    }

    if prooftype == ConfType::InfeasLp || prooftype == ConfType::AltInfProof {
        // We count a global bound reduction as size 1.
        c.dualproofsinfnnonzeros += 1;
        c.ndualproofsinfsuccess += 1;
        c.ninflpsuccess += 1;

        if applyglobal {
            c.ndualproofsinfglobal += 1;
        } else {
            c.ndualproofsinflocal += 1;
        }
    } else {
        // We count a global bound reduction as size 1.
        c.dualproofsbndnnonzeros += 1;
        c.ndualproofsbndsuccess += 1;
        c.nboundlpsuccess += 1;

        if applyglobal {
            c.ndualproofsbndglobal += 1;
        } else {
            c.ndualproofsbndlocal += 1;
        }
    }

    Ok(())
}

/// `getMinActivity` and `getMaxActivity`: calculates the minimal (or
/// maximal) activity of a given set of bounds and coefficients;
/// `curvarlbs`/`curvarubs` are the current bounds of the active problem
/// variables (or `None` for global bounds).
fn get_min_max_activity(scip: &Scip, transprob: usize, coefs: &[f64], inds: &[i32], nnz: i32, curvarlbs: Option<&[f64]>, curvarubs: Option<&[f64]>, max: bool) -> f64 {
    use crate::quad::{Quad, prod_dd, sum_qq};
    let set = &scip.set;
    let vars = &scip.probs[transprob].vars;

    let mut act = Quad::new(0.0);

    for i in 0..nnz as usize {
        let v = inds[i] as usize;
        let val = coefs[i];

        let delta;
        // Minimal activity: lower bounds for positive coefficients; maximal
        // activity: lower bounds for negative ones.
        let uselb = if max { val < 0.0 } else { val > 0.0 };
        if uselb {
            let bnd = match curvarlbs {
                None => scip.vars[vars[v]].glbdom.lb,
                Some(l) => l[v],
            };
            if set.is_infinity(-bnd) {
                return if max { set.infinity() } else { -set.infinity() };
            }
            delta = prod_dd(val, bnd);
        } else {
            let bnd = match curvarubs {
                None => scip.vars[vars[v]].glbdom.ub,
                Some(u) => u[v],
            };
            if set.is_infinity(bnd) {
                return if max { set.infinity() } else { -set.infinity() };
            }
            delta = prod_dd(val, bnd);
        }

        // Update minimal (maximal) activity.
        act = sum_qq(act, delta);
    }

    // Check whether the activity is infinite.
    if set.is_infinity(act.to_dbl()) {
        return set.infinity();
    }
    if set.is_infinity(-act.to_dbl()) {
        return -set.infinity();
    }
    act.to_dbl()
}

/// `createAndAddProofcons`: creates a proof constraint and tries to add it
/// to the storage.
fn create_and_add_proofcons(scip: &mut Scip, proofset: &ProofSet, eventfilter: usize, origprob: usize, transprob: usize) -> ScipResult {
    use crate::conflictstore::*;

    let nnz = proofset_get_n_vars(proofset);

    if nnz == 0 {
        return Ok(());
    }

    let rhs = proofset.rhs;
    let coefs = &proofset.vals.0[..nnz as usize];
    let inds = &proofset.inds.0[..nnz as usize];
    let mut conflicttype = proofset.conflicttype;

    let applyglobal = proofset.validdepth <= crate::tree::tree_get_effective_root_depth(scip);

    'updatestatistics: {
        if applyglobal {
            let globalmaxactivity = get_min_max_activity(scip, transprob, coefs, inds, nnz, None, None, true);

            // Check whether the alternative proof is redundant.
            if scip.set.is_le(globalmaxactivity, rhs) {
                return Ok(());
            }

            // Check whether the constraint proves global infeasibility.
            let globalminactivity = get_min_max_activity(scip, transprob, coefs, inds, nnz, None, None, false);
            if scip.set.is_gt(globalminactivity, rhs) {
                let node = treer!(scip).path[proofset.validdepth];
                crate::tree::node_cutoff(scip, node, eventfilter, transprob, origprob)?;
                break 'updatestatistics;
            }
        }

        let toolong;
        if scip.set.conf_minmaxvars >= nnz {
            toolong = false;
        } else {
            let p = &scip.probs[transprob];
            let maxnnz = if p.startnconss < 100 { 0.85 * p.nvars as f64 } else { p.nvars as f64 };

            let mut fillin = nnz as f64;
            let mut tl;
            if conflicttype == ConfType::InfeasLp || conflicttype == ConfType::AltInfProof {
                fillin += conflictstore_get_n_dual_inf_proofs(scip) as f64 * conflictstore_get_avg_nnz_dual_inf_proofs(scip);
                fillin /= conflictstore_get_n_dual_inf_proofs(scip) as f64 + 1.0;
                tl = fillin > crate::def::min(2.0 * scip.stat.avgnnz, maxnnz);
            } else {
                fillin += conflictstore_get_n_dual_bnd_proofs(scip) as f64 * conflictstore_get_avg_nnz_dual_bnd_proofs(scip);
                fillin /= conflictstore_get_n_dual_bnd_proofs(scip) as f64 + 1.0;
                tl = fillin > crate::def::min(1.5 * scip.stat.avgnnz, maxnnz);
            }

            tl = tl && (nnz as f64 > scip.set.conf_maxvarsfac * p.nvars as f64);
            toolong = tl;
        }

        // Don't store global dual proofs that are too long / have too many
        // non-zeros.
        if toolong {
            if applyglobal {
                propagate_long_proof(scip, eventfilter, origprob, transprob, coefs, inds, nnz, rhs, conflicttype, proofset.validdepth)?;
            }
            return Ok(());
        }

        // Check if conflict contains variables that are invalid after a restart
        // to label it appropriately.
        let mut hasrelaxvar = false;
        let mut contonly = true;
        let mut i = 0;
        while i < nnz as usize && (!hasrelaxvar || contonly) {
            let var = scip.probs[transprob].vars[inds[i] as usize];
            hasrelaxvar |= scip.vars[var].relaxationonly;

            if crate::var::var_is_integral(scip, var) {
                contonly = false;
            }
            i += 1;
        }

        if !applyglobal && contonly {
            return Ok(());
        }

        let name = if conflicttype == ConfType::InfeasLp || conflicttype == ConfType::AltInfProof {
            format!("dualproof_inf_{}", cf!(scip).ndualproofsinfsuccess)
        } else if conflicttype == ConfType::BndExceeding || conflicttype == ConfType::AltBndProof {
            format!("dualproof_bnd_{}", cf!(scip).ndualproofsbndsuccess)
        } else {
            return Err(Retcode::InvalidCall);
        };

        let inf = scip.set.infinity();
        let mut cons = crate::cons_linear::create_cons_linear(scip, &name, &[], &[], -inf, rhs, false, false, false, false, true, !applyglobal, false, true, true, false)?;

        for i in 0..nnz as usize {
            let var = scip.probs[transprob].vars[inds[i] as usize];
            crate::cons_linear::add_coef_linear(scip, cons, var, coefs[i])?;
        }

        // Do not upgrade linear constraints of size 1.
        if nnz > 1 {
            // Try to automatically convert a linear constraint into a more
            // specific and more specialized constraint.
            let upgdcons = crate::cons_linear::upgrade_cons_linear(scip, cons)?;
            if upgdcons != NONE {
                let conshdlr = scip.conss[upgdcons].conshdlr;

                // Only upgrade constraint if it can report number of variables
                // (needed by SCIPconflictstoreAddDualraycons() and
                // SCIPconflictstoreAddDualsolcons()).
                if scip.conshdlrs[conshdlr].consgetnvars.is_none() {
                    crate::cons::cons_release(scip, upgdcons)?;
                } else {
                    crate::cons::cons_release(scip, cons)?;
                    cons = upgdcons;

                    if conflicttype == ConfType::InfeasLp {
                        conflicttype = ConfType::AltInfProof;
                    } else if conflicttype == ConfType::BndExceeding {
                        conflicttype = ConfType::AltBndProof;
                    }
                }
            }
        }

        // Mark constraint to be a conflict.
        scip.conss[cons].conflict = true;

        // Add constraint to storage.
        if conflicttype == ConfType::InfeasLp || conflicttype == ConfType::AltInfProof {
            // Add dual proof to storage.
            conflictstore_add_dualraycons(scip, cons, transprob, hasrelaxvar)?;
        } else {
            let mut scale = 1.0;
            let mut updateside = false;

            // In some cases the constraint could not be updated to a more
            // special type. However, it is possible that constraint got scaled.
            // Therefore, we need to be very careful when updating the lhs/rhs
            // after the incumbent solution has improved.
            if conflicttype == ConfType::BndExceeding {
                let mut side = crate::cons_linear::get_lhs_linear(scip, cons);

                if !scip.set.is_infinity(-side) {
                    if scip.set.is_zero(side) {
                        scale = -1.0;
                    } else {
                        scale = proofset.rhs / side;
                    }
                } else {
                    side = crate::cons_linear::get_rhs_linear(scip, cons);

                    if scip.set.is_zero(side) {
                        scale = 1.0;
                    } else {
                        scale = proofset.rhs / side;
                    }
                }
                updateside = true;
            }

            // Add dual proof to storage.
            conflictstore_add_dualsolcons(scip, cons, transprob, scale, updateside, hasrelaxvar)?;
        }

        if applyglobal {
            // Add the constraint to the global problem.
            crate::prob::prob_add_cons(scip, transprob, cons)?;
        } else {
            let node = treer!(scip).path[proofset.validdepth];
            crate::tree::node_add_cons(scip, node, cons)?;
        }

        // Release the constraint.
        crate::cons::cons_release(scip, cons)?;
    }

    // UPDATESTATISTICS:
    // Update statistics.
    let c = &mut cf!(scip);
    if conflicttype == ConfType::InfeasLp || conflicttype == ConfType::AltInfProof {
        c.dualproofsinfnnonzeros += nnz as i64;
        if applyglobal {
            c.ndualproofsinfglobal += 1;
        } else {
            c.ndualproofsinflocal += 1;
        }
        c.ndualproofsinfsuccess += 1;
    } else {
        c.dualproofsbndnnonzeros += nnz as i64;
        if applyglobal {
            c.ndualproofsbndglobal += 1;
        } else {
            c.ndualproofsbndlocal += 1;
        }
        c.ndualproofsbndsuccess += 1;
    }
    Ok(())
}

/// `propagateLongProof`: propagate a long proof.
#[allow(clippy::too_many_arguments)]
fn propagate_long_proof(
    scip: &mut Scip,
    eventfilter: usize,
    origprob: usize,
    transprob: usize,
    coefs: &[f64],
    inds: &[i32],
    nnz: i32,
    rhs: f64,
    conflicttype: ConfType,
    validdepth: i32,
) -> ScipResult {
    let minact = get_min_max_activity(scip, transprob, coefs, inds, nnz, None, None, false);

    // We cannot find global tightenings.
    if scip.set.is_infinity(-minact) {
        return Ok(());
    }

    for i in 0..nnz as usize {
        let pos = inds[i] as usize;
        let val = coefs[i];
        let var = scip.probs[transprob].vars[pos];
        let lb = scip.vars[var].glbdom.lb;
        let ub = scip.vars[var].glbdom.ub;
        let mut resminact = minact;

        // We got a potential new upper bound.
        if val > 0.0 {
            resminact -= val * lb;
            let newub = (rhs - resminact) / val;

            if scip.set.is_infinity(newub) {
                continue;
            }

            // We cannot tighten the upper bound.
            if scip.set.is_ge(newub, ub) {
                continue;
            }

            tighten_single_var(scip, eventfilter, origprob, transprob, var, val, rhs - resminact, conflicttype, validdepth)?;
        }
        // We got a potential new lower bound.
        else {
            resminact -= val * ub;
            let newlb = (rhs - resminact) / val;

            if scip.set.is_infinity(-newlb) {
                continue;
            }

            // We cannot tighten the lower bound.
            if scip.set.is_le(newlb, lb) {
                continue;
            }

            tighten_single_var(scip, eventfilter, origprob, transprob, var, val, rhs - resminact, conflicttype, validdepth)?;
        }

        // The minimal activity should stay unchanged because we tightened the
        // bound that doesn't contribute to the minimal activity.
    }
    Ok(())
}

/// Tightens the single variable of a proof set with one non-zero, or
/// creates and adds its proof constraint.
fn flush_one_proofset(scip: &mut Scip, p: &ProofSet, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult {
    // Only one variable has a coefficient different to zero, we add this
    // bound change instead of a constraint.
    if proofset_get_n_vars(p) == 1 {
        let var = scip.probs[transprob].vars[p.inds[0] as usize];
        tighten_single_var(scip, eventfilter, origprob, transprob, var, p.vals[0], p.rhs, p.conflicttype, p.validdepth)
    } else {
        // Create and add proof constraint.
        create_and_add_proofcons(scip, p, eventfilter, origprob, transprob)
    }
}

/// `SCIPconflictFlushProofset`: create proof constraints out of proof sets.
pub fn conflict_flush_proofset(scip: &mut Scip, transprob: usize, origprob: usize, eventfilter: usize) -> ScipResult {
    if cf!(scip).proofset.conflicttype != ConfType::Unknown {
        let proofset = std::mem::take(&mut cf!(scip).proofset);

        // Only one variable has a coefficient different to zero, we add this
        // bound change instead of a constraint.
        if proofset_get_n_vars(&proofset) == 1 {
            let var = scip.probs[transprob].vars[proofset.inds[0] as usize];
            tighten_single_var(
                scip,
                eventfilter,
                origprob,
                transprob,
                var,
                proofset.vals[0],
                proofset.rhs,
                proofset.conflicttype,
                proofset.validdepth,
            )?;
        } else {
            let mut skipinitialproof = false;

            // Prefer an infeasibility proof
            //
            // todo: check whether this is really what we want
            if scip.set.conf_prefinfproof && proofset.conflicttype == ConfType::BndExceeding {
                let c = &cf!(scip);
                for i in 0..c.nproofsets as usize {
                    if c.proofsets[i].conflicttype == ConfType::InfeasLp {
                        skipinitialproof = true;
                        break;
                    }
                }
            }

            if !skipinitialproof {
                // Create and add the original proof.
                create_and_add_proofcons(scip, &proofset, eventfilter, origprob, transprob)?;
            }
        }

        // Clear the proof set anyway.
        cf!(scip).proofset = proofset;
        proofset_clear(&mut cf!(scip).proofset);
    }

    if cf!(scip).nproofsets > 0 {
        let proofsets = std::mem::take(&mut cf!(scip).proofsets);
        for p in &proofsets {
            flush_one_proofset(scip, p, transprob, origprob, eventfilter)?;
        }

        // Free all proofsets.
        cf!(scip).nproofsets = 0;
    }

    Ok(())
}

/// `tightenCoefficients`: apply coefficient tightening; returns the number
/// of changed coefficients and whether the proof set is redundant.
fn tighten_coefficients(scip: &Scip, p: &mut ProofSet) -> (i32, bool) {
    let mut nchgcoefs = 0;
    let ProofSet { vals, rhs, inds, nnz, .. } = p;
    let redundant = crate::cuts::cuts_tighten_coefficients(scip, false, vals, rhs, inds, nnz, &mut nchgcoefs);
    (nchgcoefs, redundant)
}

/// `separateAlternativeProofs`: try to generate alternative proofs by
/// applying subadditive functions.
fn separate_alternative_proofs(scip: &mut Scip, transprob: usize, proofrow: &mut crate::cuts::AggrRow, curvarlbs: &[f64], curvarubs: &[f64], conflicttype: ConfType) -> ScipResult {
    let nvars = scip.probs[transprob].nvars as usize;
    let nnz = crate::cuts::aggr_row_get_n_nz(proofrow);

    let (mut proofefficacy, infdelta) = aggr_row_get_min_activity(scip, transprob, proofrow, Some(curvarlbs), Some(curvarubs));

    if infdelta {
        return Ok(());
    }

    proofefficacy -= crate::cuts::aggr_row_get_rhs(proofrow);

    let efficacynorm = crate::cuts::aggr_row_calc_efficacy_norm(scip, proofrow);
    proofefficacy /= crate::def::max(1e-6, efficacynorm);

    // Create reference solution.
    let refsol = crate::api::create_sol(scip, NONE)?;

    // Initialize with average solution.
    for i in 0..nvars {
        let var = scip.probs[transprob].vars[i];
        let avg = crate::var::var_get_avg_sol(scip, var);
        crate::sol::sol_set_val(scip, refsol, var, avg)?;
    }

    // Set all variables that are part of the proof to its active local bound.
    for i in 0..nnz as usize {
        let ind = proofrow.inds[i] as usize;
        let val = crate::cuts::aggr_row_get_probvar_value(proofrow, ind as i32);
        let var = scip.probs[transprob].vars[ind];

        if val > 0.0 {
            crate::sol::sol_set_val(scip, refsol, var, curvarubs[ind])?;
        } else {
            crate::sol::sol_set_val(scip, refsol, var, curvarlbs[ind])?;
        }
    }

    let mut cutcoefs = vec![0.0; nvars];
    let mut cutinds = vec![0; nvars];
    let mut cutnnz = 0;
    let mut cutrhs = 0.0;
    let mut cutefficacy = -scip.set.infinity();
    let mut islocal = false;

    // Apply flow cover.
    let out = crate::cuts::CutOut {
        cutcoefs: &mut cutcoefs,
        cutrhs: &mut cutrhs,
        cutinds: &mut cutinds,
        cutnnz: &mut cutnnz,
        cutefficacy: Some(&mut cutefficacy),
        cutrank: None,
        cutislocal: &mut islocal,
    };
    let cutsuccess = crate::cuts::calc_flow_cover(scip, refsol, POSTPROCESS, BOUNDSWITCH, ALLOWLOCAL, proofrow, out)?;
    let mut success = cutsuccess;

    // Apply MIR.
    let out = crate::cuts::CutOut {
        cutcoefs: &mut cutcoefs,
        cutrhs: &mut cutrhs,
        cutinds: &mut cutinds,
        cutnnz: &mut cutnnz,
        cutefficacy: Some(&mut cutefficacy),
        cutrank: None,
        cutislocal: &mut islocal,
    };
    let cutsuccess = crate::cuts::cut_generation_heuristic_cmir(
        scip,
        refsol,
        POSTPROCESS,
        BOUNDSWITCH,
        VARTYPEUSEVBDS,
        ALLOWLOCAL,
        i32::MAX,
        None,
        None,
        MINFRAC,
        MAXFRAC,
        proofrow,
        out,
    )?;
    success = success || cutsuccess;

    // Replace the current proof.
    if success && !islocal && scip.set.is_positive(cutefficacy) && cutefficacy * nnz as f64 > proofefficacy * cutnnz as f64 {
        let mut alternativeproofset = proofset_create();
        alternativeproofset.conflicttype = if conflicttype == ConfType::InfeasLp {
            ConfType::AltInfProof
        } else {
            ConfType::AltBndProof
        };

        proofset_add_sparse_data(&mut alternativeproofset, &cutcoefs, &cutinds, cutnnz, cutrhs);

        // Apply coefficient tightening.
        let (_, redundant) = tighten_coefficients(scip, &mut alternativeproofset);

        if !redundant {
            conflict_insert_proofset(scip, alternativeproofset);
        }
    }

    crate::api::free_sol(scip, refsol)
}

/// `tightenDualproof`: tighten a given infeasibility proof a^Tx <= b with
/// minact > b w.r.t. local bounds
///
///  1) Apply cut generating functions
///     - c-MIR
///     - Flow-cover
///     - TODO: implement other subadditive functions
///  2) Remove continuous variables contributing with its global bound
///     - TODO: implement a variant of non-zero-cancellation
#[allow(clippy::too_many_arguments)]
fn tighten_dualproof(
    scip: &mut Scip,
    transprob: usize,
    proofrow: &mut crate::cuts::AggrRow,
    validdepth: i32,
    curvarlbs: &[f64],
    curvarubs: &[f64],
    initialproof: bool,
) -> ScipResult {
    // Try to find an alternative proof of local infeasibility that is
    // stronger.
    if scip.set.conf_sepaaltproofs {
        let conftype = cf!(scip).conflictset.conflicttype;
        separate_alternative_proofs(scip, transprob, proofrow, curvarlbs, curvarubs, conftype)?;
    }

    let mut proofset = if initialproof { std::mem::take(&mut cf!(scip).proofset) } else { proofset_create() };

    // Start with a proofset containing all variables with a non-zero
    // coefficient in the dual proof.
    proofset_add_aggrrow(&mut proofset, proofrow);
    proofset.conflicttype = cf!(scip).conflictset.conflicttype;
    proofset.validdepth = validdepth;

    // Get proof data.
    let mut nnz = proofset_get_n_vars(&proofset);

    // Remove continuous variable contributing with their global bound
    //
    // todo: check whether we also want to do that for bound exceeding proofs,
    // but then we cannot update the conflict anymore.
    if proofset.conflicttype == ConfType::InfeasLp {
        // Remove all continuous variables that have equal global and local
        // bounds (ub or lb depend on the sign) from the proof.
        let mut i = 0;
        while i < nnz as usize && nnz > 1 {
            let idx = proofset.inds[i] as usize;
            let val = proofset.vals[i];
            let var = scip.probs[transprob].vars[idx];

            // Skip integral variables.
            if crate::var::var_is_nonimplied_integral(scip, var) {
                i += 1;
                continue;
            }

            // Get appropriate global and local bounds.
            let glbbd = if val < 0.0 { scip.vars[var].glbdom.ub } else { scip.vars[var].glbdom.lb };
            let locbd = if val < 0.0 { curvarubs[idx] } else { curvarlbs[idx] };

            if !scip.set.is_eq(glbbd, locbd) {
                i += 1;
                continue;
            }

            proofset_cancel_var_with_bound(scip, &mut proofset, var, i);
            nnz -= 1;
        }
    }

    // Apply coefficient tightening to initial proof.
    let (nchgcoefs, redundant) = tighten_coefficients(scip, &mut proofset);

    // It can happen that the constraints is almost globally redundant w.r.t
    // to the maximal activity, e.g., due to numerics. in this case, we want to
    // discard the proof.
    if redundant {
        if initialproof {
            proofset_clear(&mut proofset);
            cf!(scip).proofset = proofset;
        }
    } else {
        if nchgcoefs > 0 {
            if proofset.conflicttype == ConfType::InfeasLp {
                proofset.conflicttype = ConfType::AltInfProof;
            } else if proofset.conflicttype == ConfType::BndExceeding {
                proofset.conflicttype = ConfType::AltBndProof;
            }
        }

        if initialproof {
            cf!(scip).proofset = proofset;
        } else {
            conflict_insert_proofset(scip, proofset);
        }
    }

    Ok(())
}

/// `SCIPconflictAnalyzeDualProof`: perform conflict analysis based on a
/// dual unbounded ray; given an aggregation of rows lhs <= a^Tx such that
/// lhs > maxactivity. if the constraint has size one we add a bound change
/// instead of the constraint; returns `(globalinfeasible, success)`.
#[allow(clippy::too_many_arguments)]
pub fn conflict_analyze_dual_proof(
    scip: &mut Scip,
    eventfilter: usize,
    origprob: usize,
    transprob: usize,
    proofrow: &mut crate::cuts::AggrRow,
    validdepth: i32,
    curvarlbs: &[f64],
    curvarubs: &[f64],
    initialproof: bool,
) -> ScipResult<(bool, bool)> {
    let globalinfeasible = false;
    let success = false;

    // Get sparse data.
    let nnz = crate::cuts::aggr_row_get_n_nz(proofrow);
    let rhs = crate::cuts::aggr_row_get_rhs(proofrow);

    // Get minimal activity w.r.t. local bounds.
    let (minact, infdelta) = aggr_row_get_min_activity(scip, transprob, proofrow, Some(curvarlbs), Some(curvarubs));

    if infdelta {
        return Ok((globalinfeasible, success));
    }

    // Only run is the proof proves local infeasibility.
    if scip.set.is_feas_le(minact, rhs) {
        return Ok((globalinfeasible, success));
    }

    // If the farkas-proof is empty, the node and its sub tree can be cut off
    // completely.
    if nnz == 0 {
        let node = treer!(scip).path[validdepth];
        crate::tree::node_cutoff(scip, node, eventfilter, transprob, origprob)?;

        cf!(scip).ndualproofsinfsuccess += 1;

        return Ok((true, true));
    }

    // Try to enforce the constraint based on a dual ray.
    tighten_dualproof(scip, transprob, proofrow, validdepth, curvarlbs, curvarubs, initialproof)?;

    // `globalinfeasible` is never set by tightenDualproof, so the cutoff of
    // the root that follows it in C does not run.
    Ok((globalinfeasible, success))
}
