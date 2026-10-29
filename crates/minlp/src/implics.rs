//! SCIP's implications, variable bounds and cliques (`implics.c`,
//! `struct_implics.h`). Variable bounds, implications and clique lists
//! live on their variable ([`crate::var`]); cliques and the clique table
//! here.

use crate::{
    cvec::CVec,
    def::{max, min},
    hash::{HashMap, HashOps, HashTable},
    retcode::ScipResult,
    scip::Scip,
    sort,
    types::{BoundType, VarId},
    var::{Implics, NONE, VBounds},
};

/// `SCIPvarComp` on ids, which are the variables' indices.
#[inline(always)]
fn var_comp(a: &VarId, b: &VarId) -> i32 {
    (a > b) as i32 - (a < b) as i32
}

/// `SCIP_CLIQUE`.
#[derive(Clone, Debug, Default)]
pub struct Clique {
    pub vars: CVec<VarId>,
    pub values: CVec<bool>,
    pub nvars: i32,
    pub size: i32,
    pub startcleanup: i32,
    pub index: i32,
    pub id: u32,
    pub eventsissued: bool,
    pub equation: bool,
}

/// `SCIP_CLIQUETABLE`. Cliques are ids into `Scip::cliques`.
#[derive(Clone, Debug)]
pub struct CliqueTable {
    pub hashtable: HashTable<usize>,
    pub varidxtable: Option<HashMap<i32>>,
    pub djset: Option<crate::misc::DisjointSet>,
    pub cliques: CVec<usize>,
    pub nentries: i64,
    pub ncliques: i32,
    pub size: i32,
    pub ncreatedcliques: i32,
    pub ncleanupfixedvars: i32,
    pub ncleanupaggrvars: i32,
    pub ndirtycliques: i32,
    pub ncliquecomponents: i32,
    pub incleanup: bool,
    pub compsfromscratch: bool,
}

/// `vboundsSearchPos`: where `var` with a coefficient of the given sign is
/// (`found`) or would be inserted in the variable bounds, sorted by
/// variable, a negative coefficient before a positive one for the same
/// variable.
fn vbounds_search_pos(vbounds: &Option<Box<VBounds>>, var: VarId, negativecoef: bool, insertpos: &mut i32, found: &mut bool) {
    let Some(vb) = vbounds else {
        *insertpos = 0;
        *found = false;
        return;
    };

    // Binary search for the given variable.
    match sort::sortedvec_find(vb.vars.head(vb.len), var_comp, var) {
        Ok(pos) => {
            let pos = pos as i32;
            // We found the variable: check whether the variable bound with
            // the right sign is at this position.
            if (vb.coefs[pos] < 0.0) == negativecoef {
                *insertpos = pos;
                *found = true;
            } else if negativecoef {
                // The positive coefficient is at this position, the
                // negative one would be before.
                if pos + 1 < vb.len && vb.vars[pos + 1] == var {
                    *insertpos = pos + 1;
                    *found = true;
                } else {
                    *insertpos = pos + 1;
                    *found = false;
                }
            } else if pos > 0 && vb.vars[pos - 1] == var {
                *insertpos = pos - 1;
                *found = true;
            } else {
                *insertpos = pos;
                *found = false;
            }
        }
        Err(pos) => {
            *insertpos = pos as i32;
            *found = false;
        }
    }
}

/// `SCIPvboundsAdd`: adds the variable bound `x >=/<= coef*var + constant`
/// to the variable bounds of `owner` (its `vlbs` for lower, `vubs` for
/// upper), keeping the tighter one if `var` is already there with a
/// coefficient of the same sign; returns whether something changed.
pub fn vbounds_add(scip: &mut Scip, owner: VarId, vboundtype: BoundType, var: VarId, coef: f64, constant: f64) -> bool {
    let calc = |scip: &Scip, num: i32| scip.set.calc_mem_grow_size(num);
    let mut vbounds = if vboundtype == BoundType::Lower {
        scip.vars[owner].vlbs.take()
    } else {
        scip.vars[owner].vubs.take()
    };
    let mut added = false;

    // Identify insertion position of variable.
    let (mut insertpos, mut found) = (0, false);
    vbounds_search_pos(&vbounds, var, coef < 0.0, &mut insertpos, &mut found);
    if found {
        // Variable already exists in variable bound array: keep the
        // tighter variable bound.
        let vb = vbounds.as_mut().unwrap();
        if vboundtype == BoundType::Upper {
            if constant + min(coef, 0.0) < vb.constants[insertpos] + min(vb.coefs[insertpos], 0.0) {
                vb.coefs[insertpos] = coef;
                vb.constants[insertpos] = constant;
                added = true;
            }
        } else if constant + max(coef, 0.0) > vb.constants[insertpos] + max(vb.coefs[insertpos], 0.0) {
            vb.coefs[insertpos] = coef;
            vb.constants[insertpos] = constant;
            added = true;
        }
    } else {
        // Insert the variable bound at the correct position.
        let num = vbounds.as_ref().map_or(1, |vb| vb.len + 1);
        let vb = vbounds.get_or_insert_with(Default::default);
        if num > vb.size {
            let newsize = calc(scip, num);
            vb.vars.realloc(newsize);
            vb.coefs.realloc(newsize);
            vb.constants.realloc(newsize);
            vb.size = newsize;
        }
        let mut i = vb.len;
        while i > insertpos {
            vb.vars[i] = vb.vars[i - 1];
            vb.coefs[i] = vb.coefs[i - 1];
            vb.constants[i] = vb.constants[i - 1];
            i -= 1;
        }
        vb.vars[insertpos] = var;
        vb.coefs[insertpos] = coef;
        vb.constants[insertpos] = constant;
        vb.len += 1;
        added = true;
    }
    if vboundtype == BoundType::Lower {
        scip.vars[owner].vlbs = vbounds;
    } else {
        scip.vars[owner].vubs = vbounds;
    }
    added
}

/// `SCIPvboundsDel`: removes `vbdvar` with a coefficient of the given sign;
/// frees the variable bounds when empty.
pub fn vbounds_del(vbounds: &mut Option<Box<VBounds>>, vbdvar: VarId, negativecoef: bool) {
    // Search for variable.
    let (mut pos, mut found) = (0, false);
    vbounds_search_pos(vbounds, vbdvar, negativecoef, &mut pos, &mut found);
    if !found {
        return;
    }

    // Remove variable from the variable bounds array.
    let vb = vbounds.as_mut().unwrap();
    for i in pos..vb.len - 1 {
        vb.vars[i] = vb.vars[i + 1];
        vb.coefs[i] = vb.coefs[i + 1];
        vb.constants[i] = vb.constants[i + 1];
    }
    vb.len -= 1;
    if vb.len == 0 {
        *vbounds = None;
    }
}

/// `SCIPvboundsShrink`.
pub fn vbounds_shrink(vbounds: &mut Option<Box<VBounds>>, newnvbds: i32) {
    if newnvbds == 0 {
        *vbounds = None;
    } else {
        vbounds.as_mut().unwrap().len = newnvbds;
    }
}

/// `implicsEnsureSize`.
fn implics_ensure_size(scip: &Scip, implics: &mut Option<Box<Implics>>, varfixing: bool, num: i32) {
    let im = implics.get_or_insert_with(Default::default);
    let f = varfixing as usize;
    if num > im.size[f] {
        let newsize = scip.set.calc_mem_grow_size(num);
        im.vars[f].realloc(newsize);
        im.types[f].realloc(newsize);
        im.bounds[f].realloc(newsize);
        im.ids[f].realloc(newsize);
        im.size[f] = newsize;
    }
}

/// `implicsSearchVar`: the positions of the lower and upper implication on
/// `implvar` (-1 if none) and where a new one would go.
fn implics_search_var(implics: &Implics, varfixing: bool, implvar: VarId, poslower: &mut i32, posupper: &mut i32, posadd: &mut i32) {
    let f = varfixing as usize;

    // Check for an empty implication list.
    if implics.nimpls[f] == 0 {
        *posadd = 0;
        *poslower = -1;
        *posupper = -1;
        return;
    }
    let right = implics.nimpls[f] - 1;

    // Search for the position in the sorted list (could be easily
    // implemented by binary search).
    match sort::sortedvec_find(implics.vars[f].head(right + 1), var_comp, implvar) {
        Err(pos) => {
            // Zero implications found.
            *poslower = -1;
            *posupper = -1;
            *posadd = pos as i32;
        }
        Ok(pos) => {
            let pos = pos as i32;

            // There exists at least one implication.
            if implics.types[f][pos] == BoundType::Lower {
                // Lower implication found: check for an upper one after
                // it.
                *poslower = pos;
                if pos + 1 < implics.nimpls[f] && implics.vars[f][pos + 1] == implvar {
                    *posupper = pos + 1;
                } else {
                    *posupper = -1;
                }
                *posadd = pos;
            } else {
                // Upper implication found: check for a lower one before
                // it.
                *posupper = pos;
                if pos > 0 && implics.vars[f][pos - 1] == implvar {
                    *poslower = pos - 1;
                    *posadd = pos - 1;
                } else {
                    *poslower = -1;
                    *posadd = pos;
                }
            }
        }
    }
}

/// `implicsSearchImplic`: whether the implication exists, and where it is
/// or would be added.
fn implics_search_implic(implics: &Implics, varfixing: bool, implvar: VarId, impltype: BoundType, poslower: &mut i32, posupper: &mut i32, posadd: &mut i32) -> bool {
    implics_search_var(implics, varfixing, implvar, poslower, posupper, posadd);
    if impltype == BoundType::Lower {
        *poslower >= 0
    } else {
        if *poslower >= 0 {
            *posadd += 1;
        }
        *posupper >= 0
    }
}

/// `SCIPimplicsAdd`: adds `(x == varfixing) => implvar >=/<= implbound`
/// to the implications of `var`, keeping the tighter of two on the same
/// bound, and reporting a conflict with the opposite one.
pub fn implics_add(scip: &mut Scip, var: VarId, varfixing: bool, implvar: VarId, impltype: BoundType, implbound: f64, isshortcut: bool, conflict: &mut bool, added: &mut bool) {
    *conflict = false;
    *added = false;
    let f = varfixing as usize;
    let mut implics = scip.vars[var].implics.take();

    // Check if variable is already contained in the implications list.
    let (mut poslower, mut posupper, mut posadd) = (-1, -1, 0);
    let found = match implics.as_ref() {
        Some(im) => implics_search_implic(im, varfixing, implvar, impltype, &mut poslower, &mut posupper, &mut posadd),
        None => false,
    };
    'add: {
        if impltype == BoundType::Lower {
            // Check if the implication is redundant or in conflict.
            if poslower >= 0 && scip.set.is_feas_le(implbound, implics.as_ref().unwrap().bounds[f][poslower]) {
                break 'add;
            }
            if posupper >= 0 && scip.set.is_feas_gt(implbound, implics.as_ref().unwrap().bounds[f][posupper]) {
                *conflict = true;
                break 'add;
            }
            *added = true;

            // Only the lower bound has to be adjusted: implication was
            // already there.
            if found {
                implics.as_mut().unwrap().bounds[f][poslower] = implbound;
                break 'add;
            }
        } else {
            if posupper >= 0 && scip.set.is_feas_ge(implbound, implics.as_ref().unwrap().bounds[f][posupper]) {
                break 'add;
            }
            if poslower >= 0 && scip.set.is_feas_lt(implbound, implics.as_ref().unwrap().bounds[f][poslower]) {
                *conflict = true;
                break 'add;
            }
            *added = true;
            if found {
                implics.as_mut().unwrap().bounds[f][posupper] = implbound;
                break 'add;
            }
        }

        // Add the new implication at its position.
        let num = implics.as_ref().map_or(1, |im| im.nimpls[f] + 1);
        implics_ensure_size(scip, &mut implics, varfixing, num);
        let im = implics.as_mut().unwrap();
        let mut k = im.nimpls[f];
        while k > posadd {
            im.types[f][k] = im.types[f][k - 1];
            im.ids[f][k] = im.ids[f][k - 1];
            im.vars[f][k] = im.vars[f][k - 1];
            im.bounds[f][k] = im.bounds[f][k - 1];
            k -= 1;
        }
        im.vars[f][posadd] = implvar;
        im.types[f][posadd] = impltype;
        im.bounds[f][posadd] = implbound;
        im.ids[f][posadd] = if isshortcut { -scip.stat.nimplications } else { scip.stat.nimplications };
        im.nimpls[f] += 1;
        scip.stat.nimplications += 1;
    };
    scip.vars[var].implics = implics;
}

/// `SCIPimplicsDel`. Like C, it shifts types, variables and bounds, but not
/// the ids.
pub fn implics_del(implics: &mut Option<Box<Implics>>, varfixing: bool, implvar: VarId, impltype: BoundType) {
    let f = varfixing as usize;
    let (mut poslower, mut posupper, mut posadd) = (-1, -1, 0);
    let Some(im) = implics.as_mut() else { return };
    let found = implics_search_implic(im, varfixing, implvar, impltype, &mut poslower, &mut posupper, &mut posadd);
    if !found {
        return;
    }

    // Removes the implication from the array.
    if im.nimpls[f] - posadd > 1 {
        let amount = im.nimpls[f] - posadd - 1;
        for k in 0..amount {
            im.types[f][posadd + k] = im.types[f][posadd + k + 1];
            im.vars[f][posadd + k] = im.vars[f][posadd + k + 1];
            im.bounds[f][posadd + k] = im.bounds[f][posadd + k + 1];
        }
    }
    im.nimpls[f] -= 1;

    // Free implications data structure, if it is empty.
    if im.nimpls[0] == 0 && im.nimpls[1] == 0 {
        *implics = None;
    }
}

/// `SCIPimplicsGetVarImplics`.
pub fn implics_get_var_implics(implics: &Implics, varfixing: bool, implvar: VarId, haslowerimplic: &mut bool, hasupperimplic: &mut bool) {
    let (mut poslower, mut posupper, mut posadd) = (-1, -1, 0);
    implics_search_var(implics, varfixing, implvar, &mut poslower, &mut posupper, &mut posadd);
    *haslowerimplic = poslower >= 0;
    *hasupperimplic = posupper >= 0;
}

/// `SCIPimplicsGetVarImplicPoss`.
pub fn implics_get_var_implic_poss(implics: &Implics, varfixing: bool, implvar: VarId, lowerimplicpos: &mut i32, upperimplicpos: &mut i32) {
    let mut posadd = 0;
    implics_search_var(implics, varfixing, implvar, lowerimplicpos, upperimplicpos, &mut posadd);
}

/// `SCIPimplicsContainsImpl`.
pub fn implics_contains_impl(implics: &Implics, varfixing: bool, implvar: VarId, impltype: BoundType) -> bool {
    let (mut poslower, mut posupper, mut posadd) = (-1, -1, 0);
    implics_search_implic(implics, varfixing, implvar, impltype, &mut poslower, &mut posupper, &mut posadd)
}

/// `cliquetableSwapCliques`.
fn cliquetable_swap_cliques(scip: &mut Scip, first: i32, second: i32) {
    if first != second {
        let ct = scip.cliquetable.as_mut().unwrap();
        ct.cliques.swap(first as usize, second as usize);
        let (a, b) = (ct.cliques[first], ct.cliques[second]);
        scip.cliques[b].index = second;
        scip.cliques[a].index = first;
    }
}

/// `cliquetableMarkCliqueForCleanup`: moves the clique to the front part of
/// the table, which cleanup visits.
fn cliquetable_mark_clique_for_cleanup(scip: &mut Scip, clique: usize) {
    let ndirty = scip.cliquetable.as_ref().unwrap().ndirtycliques;
    if scip.cliques[clique].index > ndirty {
        let idx = scip.cliques[clique].index;
        cliquetable_swap_cliques(scip, idx, ndirty);
    }
    scip.cliquetable.as_mut().unwrap().ndirtycliques += 1;
}

/// `cliqueCreateWithData`.
fn clique_create_with_data(scip: &mut Scip, size: i32, vars: &[VarId], values: &[bool], nvars: i32, id: i32, isequation: bool) -> usize {
    let mut cvars = CVec(vars[..size as usize].to_vec());
    let mut cvalues = CVec(values[..size as usize].to_vec());
    cvars.realloc(size);
    cvalues.realloc(size);
    scip.cliques.push(Clique {
        vars: cvars,
        values: cvalues,
        nvars,
        size,
        startcleanup: -1,
        index: -1,
        id: id as u32,
        eventsissued: false,
        equation: isequation,
    })
}

/// `cliqueEnsureSize`.
fn clique_ensure_size(scip: &mut Scip, clique: usize, num: i32) {
    if num > scip.cliques[clique].size {
        let newsize = scip.set.calc_mem_grow_size(num);
        let c = &mut scip.cliques[clique];
        c.vars.realloc(newsize);
        c.values.realloc(newsize);
        c.size = newsize;
    }
}

/// `SCIPcliqueSearchVar`: the position of `var` with `value` in the
/// clique, or -1.
pub fn clique_search_var(clique: &Clique, var: VarId, value: bool) -> i32 {
    let varidx = var;
    let mut left = -1;
    let mut right = clique.nvars;
    while left < right - 1 {
        let middle = (left + right) / 2;
        let idx = clique.vars[middle];
        if varidx < idx {
            right = middle;
        } else if varidx > idx {
            left = middle;
        } else {
            // Variable found: check the value.
            if !clique.values[middle] & value {
                // The value at middle is false, look for true after it.
                let mut i = middle + 1;
                while i < clique.nvars && clique.vars[i] == var {
                    if clique.values[i] == value {
                        return i;
                    }
                    i += 1;
                }
                return -1;
            }
            if clique.values[middle] & !value {
                let mut i = middle - 1;
                while i >= 0 && clique.vars[i] == var {
                    if clique.values[i] == value {
                        return i;
                    }
                    i -= 1;
                }
                return -1;
            }
            return middle;
        }
    }
    -1
}

/// `SCIPcliqueHasVar`.
pub fn clique_has_var(clique: &Clique, var: VarId, value: bool) -> bool {
    clique_search_var(clique, var, value) >= 0
}

/// `SCIPcliqueAddVar`: inserts `var` with `value` in the clique, sorted by
/// variable (false before true), and reports whether it is now in twice
/// with the same value or with both values.
pub fn clique_add_var(scip: &mut Scip, clique: usize, var: VarId, mut value: bool, doubleentry: &mut bool, oppositeentry: &mut bool) {
    *doubleentry = false;
    *oppositeentry = false;

    // Allocate memory.
    let n = scip.cliques[clique].nvars + 1;
    clique_ensure_size(scip, clique, n);
    let c = &mut scip.cliques[clique];

    // Search for insertion position by binary variable, note that first
    // the entries are order after variable index and second after the bool
    // value of the corresponding variable.
    let mut pos = match sort::sortedvec_find(c.vars.head(c.nvars), var_comp, var) {
        Ok(p) | Err(p) => p as i32,
    };
    let i = pos;

    // Remember insertion position for later, pos might change.
    if pos < c.nvars {
        let amount = c.nvars - pos;

        // Moving elements to correct position.
        let mut k = amount;
        while k > 0 {
            c.vars[pos + k] = c.vars[pos + k - 1];
            c.values[pos + k] = c.values[pos + k - 1];
            k -= 1;
        }
        c.nvars += 1;

        // Insertion for a variable with cliquevalue FALSE.
        if !value {
            // Find last entry with the same variable and value behind the
            // insertion position.
            while pos < c.nvars - 1 && c.vars[pos + 1] == var && c.values[pos + 1] == value {
                pos += 1;
            }

            // Check if the same variable with other value also exists.
            if pos < c.nvars - 1 && c.vars[pos + 1] == var {
                *oppositeentry = true;
            }

            // If we might have had the same variable and value before, mark
            // it.
            if pos != i {
                *doubleentry = true;
            } else {
                // Find last entry with the same variable and different
                // value before the insertion position.
                while pos > 0 && c.vars[pos - 1] == var && c.values[pos - 1] != value {
                    pos -= 1;
                }

                // Check if the same variable with the same value also
                // exists.
                if pos > 0 && c.vars[pos - 1] == var {
                    *doubleentry = true;
                }

                // If we found the same variable with a different value
                // before, we need to switch the entries.
                if pos != i {
                    c.values[pos] = value;
                    value = !value;
                }
            }
        } else {
            // Insertion for a variable with cliquevalue TRUE: find last
            // entry with the same variable and different value behind the
            // insertion position.
            while pos < c.nvars - 1 && c.vars[pos + 1] == var && c.values[pos + 1] != value {
                pos += 1;
            }

            // Check if the same variable with the same value also exists.
            if pos < c.nvars - 1 && c.vars[pos + 1] == var {
                *doubleentry = true;
            }

            // Switch the entries, because the variable with value FALSE
            // has to be behind.
            if pos != i {
                *oppositeentry = true;
                c.values[pos] = value;
                value = !value;
            } else {
                // Find last entry with the same variable and value before
                // the insertion position.
                while pos > 0 && c.vars[pos - 1] == var && c.values[pos - 1] == value {
                    pos -= 1;
                }
                if pos != i {
                    *doubleentry = true;
                }

                // Check if the same variable with the other value also
                // exists.
                if pos > 0 && c.vars[pos - 1] == var && c.values[pos - 1] != value {
                    *oppositeentry = true;
                }
            }
        }
    } else {
        c.nvars += 1;
    }
    c.vars[i] = var;
    c.values[i] = value;
    c.eventsissued = false;
}

/// `SCIPcliqueDelVar`: marks the entry for removal at the next cleanup.
pub fn clique_del_var(scip: &mut Scip, clique: usize, var: VarId, value: bool) {
    // Don't remove the variable from the clique that is currently being
    // cleaned up.
    if scip.cliquetable.as_ref().unwrap().incleanup && scip.cliques[clique].index == 0 {
        return;
    }
    let pos = clique_search_var(&scip.cliques[clique], var, value);

    // Mark the clique for cleanup and remember the first position to clean.
    if scip.cliques[clique].startcleanup == -1 {
        cliquetable_mark_clique_for_cleanup(scip, clique);
    }
    let c = &mut scip.cliques[clique];
    if c.startcleanup == -1 || pos < c.startcleanup {
        c.startcleanup = pos;
    }
}

/// `cliquesSearchClique`: binary search by id in a clique list.
fn cliques_search_clique(scip: &Scip, cliques: &[usize], clique: usize) -> i32 {
    let cliqueid = scip.cliques[clique].id;
    let mut left = -1;
    let mut right = cliques.len() as i32;
    while left < right - 1 {
        let middle = (left + right) / 2;
        let id = scip.cliques[cliques[middle as usize]].id;
        if cliqueid < id {
            right = middle;
        } else if cliqueid > id {
            left = middle;
        } else {
            return middle;
        }
    }
    -1
}

/// `SCIPcliquelistAdd`: adds the clique to `var`'s clique list for `value`,
/// sorted by id, unless already there.
pub fn cliquelist_add(scip: &mut Scip, var: VarId, value: bool, clique: usize) {
    let id = scip.cliques[clique].id;
    let v = value as usize;
    let mut i = 0;
    match scip.vars[var].cliquelist.as_ref() {
        None => scip.vars[var].cliquelist = Some(Default::default()),
        Some(cl) => {
            if !cl.cliques[v].is_empty() {
                i = cl.ncliques[v];
                while i > 0 && scip.cliques[cl.cliques[v][i - 1]].id > id {
                    i -= 1;
                }
                if i > 0 && scip.cliques[cl.cliques[v][i - 1]].id == id {
                    return;
                }
            }
        }
    }
    let n = scip.vars[var].cliquelist.as_ref().unwrap().ncliques[v] + 1;
    let newsize = if n > scip.vars[var].cliquelist.as_ref().unwrap().size[v] {
        Some(scip.set.calc_mem_grow_size(n))
    } else {
        None
    };
    let cl = scip.vars[var].cliquelist.as_mut().unwrap();
    if let Some(newsize) = newsize {
        cl.cliques[v].realloc(newsize);
        cl.size[v] = newsize;
    }
    let mut k = cl.ncliques[v];
    while k > i {
        cl.cliques[v][k] = cl.cliques[v][k - 1];
        k -= 1;
    }
    cl.cliques[v][i] = clique;
    cl.ncliques[v] += 1;
}

/// `SCIPcliquelistDel`.
pub fn cliquelist_del(scip: &mut Scip, var: VarId, value: bool, clique: usize) {
    let v = value as usize;
    let Some(cl) = scip.vars[var].cliquelist.as_ref() else { return };
    let pos = cliques_search_clique(scip, cl.cliques[v].head(cl.ncliques[v]), clique);

    // Clique does not exist in cliquelist: the call came from cleanup.
    if pos < 0 {
        return;
    }
    let cl = scip.vars[var].cliquelist.as_mut().unwrap();
    cl.ncliques[v] -= 1;
    let n = cl.ncliques[v];
    if pos < n {
        for k in pos..n {
            cl.cliques[v][k] = cl.cliques[v][k + 1];
        }
    }
    if cl.ncliques[0] == 0 && cl.ncliques[1] == 0 {
        scip.vars[var].cliquelist = None;
    }
}

/// `SCIPcliquelistsHaveCommonClique`.
pub fn cliquelists_have_common_clique(scip: &Scip, var1: VarId, value1: bool, var2: VarId, value2: bool) -> bool {
    let (Some(cl1), Some(cl2)) = (scip.vars[var1].cliquelist.as_ref(), scip.vars[var2].cliquelist.as_ref()) else {
        return false;
    };
    let mut ncliques1 = cl1.ncliques[value1 as usize];
    let mut cliques1 = &cl1.cliques[value1 as usize];
    let mut ncliques2 = cl2.ncliques[value2 as usize];
    let mut cliques2 = &cl2.cliques[value2 as usize];
    let mut i1 = 0;
    let mut i2 = 0;
    let id = |c: usize| scip.cliques[c].id;
    if i1 < ncliques1 && i2 < ncliques2 {
        // Make the bigger clique the first one.
        if ncliques2 > ncliques1 {
            std::mem::swap(&mut cliques1, &mut cliques2);
            std::mem::swap(&mut ncliques1, &mut ncliques2);
        }

        // Check whether both clique lists have a same clique.
        loop {
            let mut cliqueid = id(cliques2[i2]);

            // If last item in clique1 has a smaller index than the actual
            // clique in clique2, than cause of increasing order there will
            // be no same item and we can stop.
            if id(cliques1[ncliques1 - 1]) < cliqueid {
                break;
            }
            while id(cliques1[i1]) < cliqueid {
                i1 += 1;
            }
            cliqueid = id(cliques1[i1]);

            // If last item in clique2 has a smaller index than the actual
            // clique in clique1, than cause of increasing order there will
            // be no same item and we can stop.
            if id(cliques2[ncliques2 - 1]) < cliqueid {
                break;
            }
            while id(cliques2[i2]) < cliqueid {
                i2 += 1;
            }
            if id(cliques2[i2]) == cliqueid {
                return true;
            }
        }
    }
    false
}

/// `SCIPcliquelistRemoveFromCliques`: marks `var` for removal from all its
/// cliques.
pub fn cliquelist_remove_from_cliques(scip: &mut Scip, var: VarId, irrelevantvar: bool) {
    if scip.vars[var].cliquelist.is_none() {
        return;
    }
    for value in 0..2 {
        let mut i = scip.vars[var].cliquelist.as_ref().map_or(0, |cl| cl.ncliques[value]) - 1;
        while i >= 0 {
            let clique = scip.vars[var].cliquelist.as_ref().unwrap().cliques[value][i];
            clique_del_var(scip, clique, var, value == 1);
            if irrelevantvar {
                scip.cliques[clique].equation = false;
            }
            i -= 1;
        }
    }
}

/// The clique table's hash callbacks: `hashgetkeyClique`,
/// `hashkeyeqClique`, `hashkeyvalClique`.
pub struct CliqueKeys<'a>(pub &'a crate::arena::Arena<Clique>);

impl HashOps<usize> for CliqueKeys<'_> {
    type Key = usize;
    fn get_key(&self, element: &usize) -> usize {
        *element
    }
    fn key_eq(&self, key1: &usize, key2: &usize) -> bool {
        let (c1, c2) = (&self.0[*key1], &self.0[*key2]);
        if c1.nvars != c2.nvars {
            return false;
        }
        for i in 0..c1.nvars {
            if c1.vars[i] != c2.vars[i] || c1.values[i] != c2.values[i] {
                return false;
            }
        }
        true
    }
    fn key_val(&self, key: &usize) -> u64 {
        let c = &self.0[*key];
        if c.nvars == 0 {
            0
        } else {
            crate::misc::hash_four(
                c.vars[0] as u32,
                c.vars[c.nvars - 1] as u32,
                c.nvars as u32,
                2 * c.values[0] as u32 + c.values[c.nvars - 1] as u32,
            ) as u64
        }
    }
}

const HASHTABLE_CLIQUETABLE_SIZE: i32 = 100;

/// `SCIPcliquetableCreate`.
pub fn cliquetable_create(scip: &mut Scip) {
    let mut hashtablesize = HASHTABLE_CLIQUETABLE_SIZE;
    let small = if scip.set.misc_usesmalltables {
        crate::def::HASHSIZE_CLIQUES_SMALL
    } else {
        crate::def::HASHSIZE_CLIQUES
    } as i32;
    hashtablesize = max(hashtablesize, small);
    scip.cliquetable = Some(Box::new(CliqueTable {
        hashtable: HashTable::new(hashtablesize),
        varidxtable: None,
        djset: None,
        cliques: CVec::new(),
        nentries: 0,
        ncliques: 0,
        size: 0,
        ncreatedcliques: 0,
        ncleanupfixedvars: 0,
        ncleanupaggrvars: 0,
        ndirtycliques: 0,
        ncliquecomponents: -1,
        incleanup: false,
        compsfromscratch: false,
    }));
}

/// `SCIPcliquetableFree`.
pub fn cliquetable_free(scip: &mut Scip) {
    if let Some(ct) = scip.cliquetable.take() {
        let mut i = ct.ncliques - 1;
        while i >= 0 {
            scip.cliques.remove(ct.cliques[i]);
            i -= 1;
        }
    }
}

/// `cliquetableEnsureSize`.
fn cliquetable_ensure_size(scip: &mut Scip, num: i32) {
    if num > scip.cliquetable.as_ref().unwrap().size {
        let newsize = scip.set.calc_mem_grow_size(num);
        let ct = scip.cliquetable.as_mut().unwrap();
        ct.cliques.realloc(newsize);
        ct.size = newsize;
    }
}

/// `sortAndMergeClique`: sorts the clique's variables, removes duplicates
/// (fixing variables that appear twice), and drops or fixes globally fixed
/// variables. With `clique`, the clique is removed from the clique lists
/// of the variables it loses.
fn sort_and_merge_clique(
    scip: &mut Scip,
    clqvars: &mut CVec<VarId>,
    clqvalues: &mut CVec<bool>,
    nclqvars: &mut i32,
    isequation: &mut bool,
    clique: usize,
    transprob: usize,
    origprob: usize,
    nbdchgs: &mut i32,
    infeasible: &mut bool,
) -> ScipResult {
    use crate::var::{var_del_clique_from_list, var_fix_binary};
    if *nclqvars <= 1 {
        return Ok(());
    }

    // Sort variables and corresponding clique values after variables
    // indices.
    let n = *nclqvars as usize;
    sort::sort(clqvars.head_mut(n as i32), (clqvalues.head_mut(n as i32),), var_comp);
    let noldbdchgs = *nbdchgs;

    // Check for multiple occurrences or pairs of negations in the variable
    // array, this should be very rare when creating a new clique.
    let mut startidx = *nclqvars - 1;
    while startidx >= 0 {
        let var = clqvars[startidx];
        let (mut nones, mut nzeros) = if clqvalues[startidx] { (1, 0) } else { (0, 1) };
        let mut curr = startidx - 1;

        // Loop over all variables with the same index.
        while curr >= 0 {
            if clqvars[curr] == var {
                if clqvalues[curr] {
                    nones += 1;
                } else {
                    nzeros += 1;
                }
            } else {
                // Variable index changed.
                break;
            }
            curr -= 1;
        }

        // Single occurrence of the variable.
        if nones + nzeros == 1 {
            startidx = curr;
            continue;
        }

        // At least two occurrences of both the variable and its negation;
        // infeasible.
        if nones >= 2 && nzeros >= 2 {
            *infeasible = true;
            break;
        }

        // Do we have the same variable with the same clique value twice?
        if nones >= 2 || nzeros >= 2 {
            // Variable cannot be one or zero.
            if clique != NONE {
                if nones >= 1 {
                    var_del_clique_from_list(scip, var, true, clique);
                }
                if nzeros >= 1 {
                    var_del_clique_from_list(scip, var, false, clique);
                }
            }

            // Fixing the variable to its opposite value.
            let fixvalue = nones < 2;
            var_fix_binary(scip, var, transprob, origprob, fixvalue, infeasible, Some(nbdchgs))?;
            if *infeasible {
                break;
            }
        }

        // Do we have a pair of negated variables?
        if nones >= 1 && nzeros >= 1 {
            // Fix all other variables of the clique to the opposite of their
            // clique value.
            if nzeros + nones < *nclqvars {
                let mut w = *nclqvars - 1;
                while w >= 0 {
                    if w == startidx {
                        if curr == -1 {
                            break;
                        }
                        w = curr;
                    }

                    // Check for other negated variables.
                    while w > 0 && clqvars[w - 1] == clqvars[w] {
                        if clqvalues[w - 1] != clqvalues[w] {
                            *infeasible = true;
                            break;
                        }
                        w -= 1;
                    }
                    if *infeasible {
                        break;
                    }
                    if clique != NONE {
                        var_del_clique_from_list(scip, clqvars[w], clqvalues[w], clique);
                    }
                    var_fix_binary(scip, clqvars[w], transprob, origprob, !clqvalues[w], infeasible, Some(nbdchgs))?;
                    if *infeasible {
                        break;
                    }
                    w -= 1;
                }
            }

            // All variables except for the negated pair are fixed: the
            // clique is empty.
            if clique != NONE {
                var_del_clique_from_list(scip, var, true, clique);
                var_del_clique_from_list(scip, var, false, clique);
            }
            *nclqvars = 0;
            *isequation = false;
            break;
        }
        startidx = curr;
    }

    // Did we find an infeasibility or do we have an empty clique?
    if *infeasible || *nclqvars == 0 {
        return Ok(());
    }

    // Check if we fixed any variable.
    if *nbdchgs > noldbdchgs {
        let mut onefixedvalue = true;
        let mut onefixedvar = NONE;
        let mut startidx = 0;
        let mut w = 0;

        // Remove all fixed variables, and check whether a variable is fixed
        // to its clique value.
        while startidx < *nclqvars {
            let var = clqvars[startidx];

            // Check if we have a variable fixed to its clique value.
            if (clqvalues[startidx] && scip.vars[var].glbdom.lb > 0.5) || (!clqvalues[startidx] && scip.vars[var].glbdom.ub < 0.5) {
                // Variable fixed to its clique value: the other variables
                // have to be fixed to the opposite.
                if onefixedvar != NONE {
                    *infeasible = true;
                    return Ok(());
                }
                onefixedvar = var;
                onefixedvalue = clqvalues[startidx];
            } else if scip.vars[var].glbdom.lb < 0.5 && scip.vars[var].glbdom.ub > 0.5 {
                // Keep unfixed variables.
                if startidx > w {
                    clqvars[w] = clqvars[startidx];
                    clqvalues[w] = clqvalues[startidx];
                }
                w += 1;
            } else if clique != NONE {
                // Variable fixed to the opposite of its clique value.
                var_del_clique_from_list(scip, var, clqvalues[startidx], clique);
            }
            startidx += 1;
        }

        // Update array index.
        *nclqvars = w;

        // Do we have a variable fixed to its clique value?
        if onefixedvar != NONE {
            // Fix all remaining variables to the opposite of their clique
            // value; C starts one past the remaining entries.
            let mut startidx = *nclqvars;
            while startidx >= 0 {
                if onefixedvalue != clqvalues[startidx] || clqvars[startidx] != onefixedvar {
                    if clique != NONE {
                        var_del_clique_from_list(scip, clqvars[startidx], clqvalues[startidx], clique);
                    }
                    while startidx > 0 && clqvars[startidx - 1] == clqvars[startidx] {
                        if clqvalues[startidx - 1] != clqvalues[startidx] {
                            *infeasible = true;
                            return Ok(());
                        }
                        startidx -= 1;
                    }
                    var_fix_binary(scip, clqvars[startidx], transprob, origprob, !clqvalues[startidx], infeasible, Some(nbdchgs))?;
                    if *infeasible {
                        return Ok(());
                    }
                }
                startidx -= 1;
            }

            // Delete clique from the clique list of the fixed variable.
            if clique != NONE {
                var_del_clique_from_list(scip, onefixedvar, onefixedvalue, clique);
            }
            *nclqvars = 0;
            *isequation = false;
        }
    }

    // An equation with at most one variable is fixed.
    if *isequation {
        if *nclqvars == 0 {
            *infeasible = true;
        } else if *nclqvars == 1 {
            if clique != NONE {
                var_del_clique_from_list(scip, clqvars[0], clqvalues[0], clique);
            }

            // Clearing data and fixing the variable to its clique value.
            var_fix_binary(scip, clqvars[0], transprob, origprob, clqvalues[0], infeasible, Some(nbdchgs))?;
            *nclqvars = 0;
            *isequation = false;
        }
    }
    Ok(())
}

/// `cliquetableGetNodeIndexBinvar`: the node of the binary variable in the
/// clique graph, or -1.
fn cliquetable_get_node_index_binvar(scip: &mut Scip, binvar: VarId) -> i32 {
    if scip.cliquetable.as_ref().unwrap().varidxtable.is_none() {
        return -1;
    }

    // Get active representative of variable.
    let activevar = if !crate::var::var_is_active(scip, binvar) {
        let activevar = crate::var::var_get_probvar(scip, binvar);
        if !crate::var::var_is_active(scip, activevar) {
            return -1;
        }
        activevar
    } else {
        binvar
    };
    let ct = scip.cliquetable.as_mut().unwrap();
    match ct.varidxtable.as_ref().unwrap().get_image(activevar as u64) {
        Some(nodeindex) => nodeindex,
        None => {
            ct.compsfromscratch = true;
            -1
        }
    }
}

/// `cliquetableUpdateConnectednessClique`: unites the clique's variables in
/// the component structure.
fn cliquetable_update_connectedness_clique(scip: &mut Scip, clique: usize) {
    // Check if the disjoint set data structure needs to be rebuilt from
    // scratch.
    {
        let ct = scip.cliquetable.as_mut().unwrap();
        if ct.djset.is_none() {
            ct.compsfromscratch = true;
        }
        if ct.compsfromscratch {
            return;
        }
    }
    let mut lastnode = -1;
    let nclqvars = scip.cliques[clique].nvars;
    let mut i = 0;
    while i < nclqvars && !scip.cliquetable.as_ref().unwrap().compsfromscratch {
        let v = scip.cliques[clique].vars[i];
        let currnode = cliquetable_get_node_index_binvar(scip, v);
        if currnode == -1 {
            i += 1;
            continue;
        }
        if lastnode != -1 {
            scip.cliquetable.as_mut().unwrap().djset.as_mut().unwrap().union(lastnode, currnode, false);
        }
        lastnode = currnode;
        i += 1;
    }
}

/// `SCIPcliquetableGetVarComponentIdx`.
pub fn cliquetable_get_var_component_idx(scip: &mut Scip, var: VarId) -> i32 {
    let mut cmpidx = -1;
    if crate::var::var_is_binary(scip, var) {
        let nodeindex = cliquetable_get_node_index_binvar(scip, var);
        if nodeindex >= 0 {
            cmpidx = scip.cliquetable.as_mut().unwrap().djset.as_mut().unwrap().find(nodeindex);
        }
    }
    cmpidx
}

/// `SCIPcliquetableAdd`: adds a clique on the given variables (`values`
/// empty for TRUE on all); fixes variables if the clique contains the same
/// variable twice or one already fixed to its clique value.
#[allow(clippy::too_many_arguments)]
pub fn cliquetable_add(
    scip: &mut Scip,
    transprob: usize,
    origprob: usize,
    vars: &[VarId],
    values: &[bool],
    isequation: bool,
    infeasible: &mut bool,
    mut nbdchgs: Option<&mut i32>,
) -> ScipResult {
    use crate::{types::VarStatus, var::var_fix_binary};
    let mut isequation = isequation;
    let mut nvars = vars.len() as i32;
    let mut nlocalbdchgs = 0;
    let size = nvars;

    // Copy clique data.
    let mut clqvalues: CVec<bool> = if values.is_empty() {
        CVec(vec![true; size as usize])
    } else {
        CVec(values[..size as usize].to_vec())
    };
    let mut clqvars: CVec<VarId> = CVec(vars[..size as usize].to_vec());

    // Get active variables.
    crate::var::vars_get_probvar_binary(scip, &mut clqvars.0, &mut clqvalues.0)?;

    // Remove all inactive vars.
    let mut v = nvars - 1;
    'outer: while v >= 0 {
        let var = clqvars[v];

        // If we have a variable already fixed to one in the clique, fix all
        // other to zero.
        let (lb, ub) = (scip.vars[var].glbdom.lb, scip.vars[var].glbdom.ub);
        if !scip.vars[var].delglobalstructs && ((clqvalues[v] && lb > 0.5) || (!clqvalues[v] && ub < 0.5)) {
            let mut w = nvars - 1;
            while w >= 0 {
                let clqvar = clqvars[w];

                // The rare case where the same variable appears several
                // times is handled later during sort and merge.
                if clqvar != var {
                    let clqval = clqvalues[w];
                    let (clb, cub) = (scip.vars[clqvar].glbdom.lb, scip.vars[clqvar].glbdom.ub);

                    // Check if variable is fixed already and terminate with
                    // infeasible if this fixing contradicts the clique info.
                    if clb > cub - 0.5 {
                        if (clqval && clb > 0.5) || (!clqval && cub < 0.5) {
                            *infeasible = true;
                            return Ok(());
                        }
                        w -= 1;
                        continue;
                    }

                    // Fix the variable.
                    var_fix_binary(scip, clqvar, transprob, origprob, !clqval, infeasible, Some(&mut nlocalbdchgs))?;
                    if *infeasible {
                        break;
                    }
                }
                w -= 1;
            }

            // All variables are fixed so we can stop.
            break 'outer;
        }

        // Only unfixed column and loose variables may be member of a clique.
        let status = scip.vars[var].varstatus;
        if ub - lb < 0.5 || (status != VarStatus::Column && status != VarStatus::Loose) || scip.vars[var].delglobalstructs {
            nvars -= 1;
            clqvars[v] = clqvars[nvars];
            clqvalues[v] = clqvalues[nvars];
            isequation = isequation && status != VarStatus::MultAggr && !scip.vars[var].delglobalstructs;
        }
        v -= 1;
    }

    if let Some(n) = nbdchgs.as_deref_mut() {
        *n += nlocalbdchgs;
    }

    // Did we fix all variables or are we infeasible?
    if v >= 0 {
        return Ok(());
    }

    // Check if only one or less variables are left.
    if v < 0 && nvars <= 1 {
        if isequation {
            if nvars == 1 {
                nlocalbdchgs = 0;
                var_fix_binary(scip, clqvars[0], transprob, origprob, clqvalues[0], infeasible, Some(&mut nlocalbdchgs))?;
                if let Some(n) = nbdchgs.as_deref_mut() {
                    *n += nlocalbdchgs;
                }
            } else if nvars == 0 {
                *infeasible = true;
            }
        }
        return Ok(());
    }

    nlocalbdchgs = 0;

    // Sort the variables and values and remove multiple entries of the same
    // variable.
    sort_and_merge_clique(
        scip,
        &mut clqvars,
        &mut clqvalues,
        &mut nvars,
        &mut isequation,
        NONE,
        transprob,
        origprob,
        &mut nlocalbdchgs,
        infeasible,
    )?;

    if let Some(n) = nbdchgs {
        *n += nlocalbdchgs;
    }

    // Did we stop early due to a pair of negated variables?
    if nvars == 0 || *infeasible {
        return Ok(());
    }

    if !scip.set.is_infinity(scip.set.presol_clqtablefac) && (cliquetable_get_n_entries(scip) + nvars as i64) as f64 > scip.set.presol_clqtablefac * scip.stat.nnz as f64 {
        return Ok(());
    }

    // If less than two variables are left over, the clique is redundant.
    if nvars > 1 {
        // Create the clique data structure.
        let id = scip.cliquetable.as_ref().unwrap().ncreatedcliques;
        let clique = clique_create_with_data(scip, nvars, &clqvars.0, &clqvalues.0, nvars, id, isequation);
        let sameclique = scip.cliquetable.as_ref().unwrap().hashtable.retrieve(&CliqueKeys(&scip.cliques), &clique);

        match sameclique {
            None => {
                scip.cliquetable.as_mut().unwrap().ncreatedcliques += 1;

                // Add clique to clique table.
                let n = scip.cliquetable.as_ref().unwrap().ncliques;
                cliquetable_ensure_size(scip, n + 1);
                let ct = scip.cliquetable.as_mut().unwrap();
                ct.cliques[ct.ncliques] = clique;
                scip.cliques[clique].index = ct.ncliques;
                ct.ncliques += 1;
                ct.nentries += nvars as i64;
                cliquetable_update_connectedness_clique(scip, clique);
                let ct = scip.cliquetable.as_mut().unwrap();
                ct.hashtable.insert(&CliqueKeys(&scip.cliques), clique);

                // Add filled clique to the cliquelists of all corresponding
                // variables.
                crate::var::vars_add_clique(scip, clqvars.head(nvars), clqvalues.head(nvars), clique);
            }
            Some(sameclique) => {
                // Update equation status of clique.
                if !scip.cliques[sameclique].equation && scip.cliques[clique].equation {
                    scip.cliques[sameclique].equation = true;
                }
                scip.cliques.remove(clique);
                return Ok(());
            }
        }
    }
    Ok(())
}

/// `cliqueCleanup`: replaces inactive variables by their active
/// representatives, removes fixed ones, and fixes the rest when a variable
/// is fixed to its clique value.
fn clique_cleanup(scip: &mut Scip, clique: usize, transprob: usize, origprob: usize, nchgbds: &mut i32, infeasible: &mut bool) -> ScipResult {
    use crate::{
        types::VarStatus,
        var::{var_add_clique_to_list, var_del_clique_from_list, var_fix_binary, var_get_probvar_binary},
    };

    // Do we need to clean up fixed variables?
    if clique_is_cleaned_up(&scip.cliques[clique]) {
        return Ok(());
    }
    let mut onefixedvar = NONE;
    let mut onefixedvalue = false;
    let mut needsorting = false;
    let mut w = scip.cliques[clique].startcleanup;

    // Exchange inactive by active variables.
    let mut v = w;
    while v < scip.cliques[clique].nvars {
        // Has the variable status changed such that it needs to be replaced
        // by an active representative?
        let mut addvartoclique = false;
        let status = scip.vars[scip.cliques[clique].vars[v]].varstatus;
        if status == VarStatus::Aggregated || status == VarStatus::Negated || status == VarStatus::MultAggr {
            needsorting = true;
            let (mut cv, mut cval) = (scip.cliques[clique].vars[v], scip.cliques[clique].values[v]);
            var_del_clique_from_list(scip, cv, cval, clique);
            var_get_probvar_binary(scip, &mut cv, &mut cval)?;
            if scip.vars[cv].varstatus == VarStatus::Negated {
                cv = scip.vars[cv].negatedvar;
                cval = !cval;
                scip.cliques[clique].vars[v] = cv;
                scip.cliques[clique].values[v] = cval;
            } else if scip.vars[cv].varstatus == VarStatus::MultAggr {
                scip.cliques[clique].vars[v] = cv;
                scip.cliques[clique].values[v] = cval;
                scip.cliques[clique].equation = false;
                v += 1;
                continue;
            } else {
                scip.cliques[clique].vars[v] = cv;
                scip.cliques[clique].values[v] = cval;
            }
            addvartoclique = true;
        }

        let (cv, cval) = (scip.cliques[clique].vars[v], scip.cliques[clique].values[v]);
        let (lb, ub, del) = (scip.vars[cv].glbdom.lb, scip.vars[cv].glbdom.ub, scip.vars[cv].delglobalstructs);

        // Check for variables that are either fixed to zero or marked for
        // deletion from global structures.
        if (cval && ub < 0.5) || (!cval && lb > 0.5) || del {
            if !addvartoclique {
                var_del_clique_from_list(scip, cv, cval, clique);
            }
            if scip.cliques[clique].equation && del {
                scip.cliques[clique].equation = false;
            }

            // The variable will be overwritten by subsequent active
            // variables.
            v += 1;
            continue;
        }
        // Check for a variable fixed to one in the clique.
        else if (cval && lb > 0.5) || (!cval && ub < 0.5) {
            if onefixedvar != NONE {
                *infeasible = true;
                return Ok(());
            }
            onefixedvar = cv;
            onefixedvalue = cval;
        } else {
            if w < v {
                let c = &mut scip.cliques[clique];
                c.vars[w] = c.vars[v];
                c.values[w] = c.values[v];
            }

            // Add clique to active variable if it replaced an aggregated or
            // negated var.
            if addvartoclique {
                let (wv, wval) = (scip.cliques[clique].vars[w], scip.cliques[clique].values[w]);
                var_add_clique_to_list(scip, wv, wval, clique);
            }

            // Increase indexer of last active, i.e. unfixed, variable in
            // clique.
            w += 1;
        }
        v += 1;
    }
    scip.cliques[clique].nvars = w;

    if onefixedvar != NONE {
        let mut v = 0;
        while v < scip.cliques[clique].nvars {
            let clqvar = scip.cliques[clique].vars[v];
            let clqval = scip.cliques[clique].values[v];
            if onefixedvalue != clqval || clqvar != onefixedvar {
                // The variable could have been fixed already because it
                // appears more than once in the clique.
                let (lb, ub) = (scip.vars[clqvar].glbdom.lb, scip.vars[clqvar].glbdom.ub);
                if lb > ub - 0.5 {
                    // A variable occurring with its negation contradicts
                    // any other variable fixed to one in this clique.
                    if (clqval && lb > 0.5) || (!clqval && ub < 0.5) {
                        *infeasible = true;
                        return Ok(());
                    }
                    v += 1;
                    continue;
                }
                var_del_clique_from_list(scip, clqvar, clqval, clique);
                var_fix_binary(scip, clqvar, transprob, origprob, !clqval, infeasible, Some(nchgbds))?;
                if *infeasible {
                    return Ok(());
                }
            }
            v += 1;
        }
        let status = scip.vars[onefixedvar].varstatus;
        if status == VarStatus::Column || status == VarStatus::Loose {
            var_del_clique_from_list(scip, onefixedvar, onefixedvalue, clique);
        }
        let c = &mut scip.cliques[clique];
        c.nvars = 0;
        c.equation = false;
        c.startcleanup = -1;
        return Ok(());
    }

    if scip.cliques[clique].equation {
        if scip.cliques[clique].nvars == 0 {
            *infeasible = true;
            return Ok(());
        } else if scip.cliques[clique].nvars == 1 {
            // Clearing data and removing variable from its clique list.
            let (v0, val0) = (scip.cliques[clique].vars[0], scip.cliques[clique].values[0]);
            var_del_clique_from_list(scip, v0, val0, clique);
            var_fix_binary(scip, v0, transprob, origprob, val0, infeasible, Some(nchgbds))?;
            let c = &mut scip.cliques[clique];
            c.nvars = 0;
            c.equation = false;
            c.startcleanup = -1;
            return Ok(());
        }
    }

    if needsorting {
        let mut isequation = scip.cliques[clique].equation;

        // Remove multiple entries of the same variable. Nothing the merge
        // reaches reads this clique's entries (`clique_del_var` skips the
        // clique in cleanup), so they are taken out meanwhile.
        let mut vars = std::mem::take(&mut scip.cliques[clique].vars);
        let mut values = std::mem::take(&mut scip.cliques[clique].values);
        let mut nvars = scip.cliques[clique].nvars;
        let result = sort_and_merge_clique(scip, &mut vars, &mut values, &mut nvars, &mut isequation, clique, transprob, origprob, nchgbds, infeasible);
        let c = &mut scip.cliques[clique];
        c.vars = vars;
        c.values = values;
        c.nvars = nvars;
        result?;
        c.equation = isequation;
    }
    scip.cliques[clique].startcleanup = -1;
    Ok(())
}

/// `SCIPcliquetableCleanup`: removes all empty and single variable cliques
/// from the clique table, and double entries.
pub fn cliquetable_cleanup(scip: &mut Scip, transprob: usize, origprob: usize, nchgbds: &mut i32, infeasible: &mut bool) -> ScipResult {
    use crate::event::{event_create_impl_added, eventqueue_add, eventqueue_delay, eventqueue_process};
    *infeasible = false;

    // Check if we have anything to do.
    {
        let ct = scip.cliquetable.as_ref().unwrap();
        if scip.stat.npresolfixedvars == ct.ncleanupfixedvars && scip.stat.npresolaggrvars == ct.ncleanupaggrvars && ct.ndirtycliques == 0 {
            return Ok(());
        }
    }

    // Delay events.
    eventqueue_delay(scip);
    scip.cliquetable.as_mut().unwrap().incleanup = true;
    while scip.cliquetable.as_ref().unwrap().ndirtycliques > 0 && !*infeasible {
        let clique = scip.cliquetable.as_ref().unwrap().cliques[0];

        // Remove not cleaned up clique from hashtable.
        let ct = scip.cliquetable.as_mut().unwrap();
        ct.hashtable.remove(&CliqueKeys(&scip.cliques), &clique);
        ct.nentries -= scip.cliques[clique].nvars as i64;
        clique_cleanup(scip, clique, transprob, origprob, nchgbds, infeasible)?;
        if *infeasible {
            break;
        }

        // Swap freshly cleaned clique with last dirty clique.
        scip.cliquetable.as_mut().unwrap().ndirtycliques -= 1;
        let ndirty = scip.cliquetable.as_ref().unwrap().ndirtycliques;
        cliquetable_swap_cliques(scip, 0, ndirty);

        let sameclique = scip.cliquetable.as_ref().unwrap().hashtable.retrieve(&CliqueKeys(&scip.cliques), &clique);

        // Check if the clique is already contained in the clique table, or
        // if it is redundant (too small).
        if scip.cliques[clique].nvars <= 1 || sameclique.is_some() {
            // If the clique which is already in the hashtable is an
            // inequality and the current clique is an equation, we update
            // the equation status of the old one.
            if scip.cliques[clique].nvars > 1 && scip.cliques[clique].equation && !scip.cliques[sameclique.unwrap()].equation {
                scip.cliques[sameclique.unwrap()].equation = true;
            }

            // Delete the clique from the variables' clique lists.
            for j in 0..scip.cliques[clique].nvars {
                let (v, val) = (scip.cliques[clique].vars[j], scip.cliques[clique].values[j]);
                crate::var::var_del_clique_from_list(scip, v, val, clique);
            }

            // Free clique and remove it from clique table.
            scip.cliques.remove(clique);
            let ct = scip.cliquetable.as_mut().unwrap();
            ct.ncliques -= 1;

            // Insert a clean clique from the end of the array.
            if ct.ndirtycliques < ct.ncliques {
                ct.cliques[ct.ndirtycliques] = ct.cliques[ct.ncliques];
                scip.cliques[ct.cliques[ct.ndirtycliques]].index = ct.ndirtycliques;
            }
        } else {
            let ct = scip.cliquetable.as_mut().unwrap();
            ct.nentries += scip.cliques[clique].nvars as i64;
            ct.hashtable.insert(&CliqueKeys(&scip.cliques), clique);
            if !scip.cliques[clique].eventsissued {
                // Issue IMPLADDED event on each variable in the clique.
                for j in 0..scip.cliques[clique].nvars {
                    let event = event_create_impl_added(scip.cliques[clique].vars[j]);
                    eventqueue_add(scip, event)?;
                }
                scip.cliques[clique].eventsissued = true;
            }
        }
    }
    let ct = scip.cliquetable.as_mut().unwrap();
    ct.incleanup = false;

    // Remember the number of fixed variables and cliques in order to avoid
    // unnecessary cleanups.
    ct.ncleanupfixedvars = scip.stat.npresolfixedvars;
    ct.ncleanupaggrvars = scip.stat.npresolaggrvars;

    // Process events.
    eventqueue_process(scip)?;
    Ok(())
}

/// `SCIPcliquetableComputeCliqueComponents`: computes the connected
/// components of the clique graph. `vars` are the problem variables,
/// sorted by type.
pub fn cliquetable_compute_clique_components(scip: &mut Scip, vars: &[VarId], nbinvars: i32, nintvars: i32, nimplvars: i32) -> ScipResult {
    use crate::{
        misc::DisjointSet,
        var::{var_get_probvar, var_is_active, var_is_binary},
    };
    let mut nimplbinvars = 0;
    scip.cliquetable.as_mut().unwrap().compsfromscratch = false;
    let ndiscvars = nbinvars + nintvars + nimplvars;

    // Detect integer and implicit integer variables with bounds {0,1}
    // because they might appear in cliques, as well.
    for v in nbinvars..ndiscvars {
        if var_is_binary(scip, vars[v as usize]) {
            nimplbinvars += 1;
        }
    }

    // Count binary and implicit binary variables.
    let nbinvarstotal = nbinvars + nimplbinvars;

    // If there are no binary variables, return.
    if nbinvarstotal == 0 {
        scip.cliquetable.as_mut().unwrap().ncliquecomponents = 0;
        return Ok(());
    }

    // No cliques are present.
    if scip.cliquetable.as_ref().unwrap().ncliques == 0 {
        scip.cliquetable.as_mut().unwrap().ncliquecomponents = nbinvarstotal;
        return Ok(());
    }

    // Create or clear the variable index mapping.
    {
        let ct = scip.cliquetable.as_mut().unwrap();
        match ct.varidxtable.as_mut() {
            None => ct.varidxtable = Some(HashMap::new(ndiscvars)),
            Some(t) => t.remove_all(),
        }
    }

    // Loop through variables and store their respective positions in the
    // hash map if they are binary.
    for v in 0..ndiscvars {
        let mut var = vars[v as usize];
        if var_is_binary(scip, var) {
            // Consider only active representatives.
            if !var_is_active(scip, var) {
                var = var_get_probvar(scip, var);
                if !var_is_active(scip, var) {
                    continue;
                }
            }
            let _ = scip.cliquetable.as_mut().unwrap().varidxtable.as_mut().unwrap().insert(var as u64, v);
        }
    }

    // All integer and implicit integer variables are nodes of the graph;
    // the nonbinary ones are subtracted afterwards.
    let ct = scip.cliquetable.as_mut().unwrap();
    ct.djset = Some(DisjointSet::new(ndiscvars));

    // Subtract all (implicit) integer for which SCIPvarIsBinary() returns
    // FALSE.
    let nnonbinvars = (nintvars + nimplvars) - nimplbinvars;

    // For every clique, we connect clique variable components, treating a
    // clique as a path; stop once the graph is connected (except for the
    // nonbinary variables).
    let mut c = 0;
    while c < scip.cliquetable.as_ref().unwrap().ncliques && scip.cliquetable.as_ref().unwrap().djset.as_ref().unwrap().component_count() > 1 + nnonbinvars {
        let clique = scip.cliquetable.as_ref().unwrap().cliques[c];
        cliquetable_update_connectedness_clique(scip, clique);
        c += 1;
    }

    // Subtract superfluous integer and implicit integer variables added to
    // the auxiliary graph.
    let ct = scip.cliquetable.as_mut().unwrap();
    ct.ncliquecomponents = ct.djset.as_ref().unwrap().component_count() - nnonbinvars;
    Ok(())
}

/// `SCIPvboundsGetNVbds`.
pub fn vbounds_get_n_vbds(vbounds: &Option<Box<VBounds>>) -> i32 {
    vbounds.as_ref().map_or(0, |vb| vb.len)
}

/// `SCIPcliqueIsCleanedUp`.
pub fn clique_is_cleaned_up(clique: &Clique) -> bool {
    clique.startcleanup == -1
}

/// `SCIPcliquelistCheck`: only checks assertions in debug builds of SCIP.
#[inline(always)]
pub fn cliquelist_check(_scip: &Scip, _var: VarId) {}

/// `SCIPcliquetableGetNEntries`.
pub fn cliquetable_get_n_entries(scip: &Scip) -> i64 {
    scip.cliquetable.as_ref().unwrap().nentries
}

/// `SCIPcliquetableGetNCliqueComponents`.
pub fn cliquetable_get_n_clique_components(scip: &Scip) -> i32 {
    let ct = scip.cliquetable.as_ref().unwrap();
    if ct.compsfromscratch { -1 } else { ct.ncliquecomponents }
}

/// `SCIPcliquetableNeedsComponentUpdate`.
pub fn cliquetable_needs_component_update(scip: &Scip) -> bool {
    let ct = scip.cliquetable.as_ref().unwrap();
    ct.compsfromscratch || ct.djset.is_none()
}
