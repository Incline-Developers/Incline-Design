//! Port of `presol_gateextraction.c`: presolver extracting gate (and)
//! constraints and set-partitioning constraints from logicor and set-packing
//! constraints.
//!
//! The tables of C that hash constraint pointers only answer membership, so
//! they are sets; the tables of hashdata objects are SCIP's hash tables, as
//! their key-eq lets a constraint-less key match any constraint.

use std::{
    any::Any,
    collections::{HashMap, HashSet},
};

use crate::{
    api,
    cons::*,
    cons_setppc::SetppcType,
    hash::{HashOps, HashTable},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    scip_cons, scip_var,
    types::{ConsId, PresolId, ScipResultCode, VarId, VarStatus},
    var::NONE,
};

const PRESOL_NAME: &str = "gateextraction";
const PRESOL_DESC: &str = "presolver extracting gate(and)-constraints";
/// Priority of the presolver (>= 0: before, < 0: after constraint handlers);
/// combined with propagators.
const PRESOL_PRIORITY: i32 = 1000000;
/// Maximal number of presolving rounds the presolver participates in (-1: no
/// limit).
const PRESOL_MAXROUNDS: i32 = -1;
/// Timing of the presolver (fast, medium, or exhaustive).
const PRESOL_TIMING: PresolTiming = PRESOLTIMING_EXHAUSTIVE;

/// Minimal size of hash table in logicor constraint tables.
const HASHSIZE_LOGICORCONS: i32 = 500;
/// Minimal size of hash table in setppc constraint tables.
const HASHSIZE_SETPPCCONS: i32 = 500;

/// Should only set-partitioning constraints be extracted and no
/// and-constraints.
const DEFAULT_ONLYSETPART: bool = false;
/// Should we try to extract set-partitioning constraint out of one logicor
/// and one corresponding set-packing constraint.
const DEFAULT_SEARCHEQUATIONS: bool = true;
/// Order logicor contraints to extract big-gates before smaller ones (-1), do
/// not order them (0) or order them to extract smaller gates at first (1).
const DEFAULT_SORTING: i32 = 1;

/*
 * Data structures
 */

/// `HASHDATA` of the two-variable set-packing constraints.
#[derive(Clone, Copy, Debug)]
struct HashData {
    /// The corresponding constraint (`NONE` for a key derived from a logicor).
    cons: ConsId,
    /// Constraint variables used for hash comparison.
    vars: [VarId; 2],
}

/// `SCIP_PresolData`: presolver data.
struct PresolData {
    /// Setppc-hashdata storage (its length is C's `ssetppchashdatas`).
    setppchashdatas: Vec<HashData>,
    /// Setppc-hashdata hashtable for usable setppc constraints (`NULL`:
    /// None), holding positions in setppchashdatas.
    hashdatatable: Option<HashTable<usize>>,
    /// Setppc hashtable for usable setppc constraints.
    setppchashtable: HashSet<ConsId>,
    /// Logicor hashtable for usable logicor constraints.
    logicorhashtable: HashSet<ConsId>,
    /// Array for usable logicors (its length is C's `susefullogicor`).
    usefullogicor: Vec<ConsId>,
    /// Number of usable logicors.
    nusefullogicor: i32,
    /// Number of setppchashdata elements added to the hashtable.
    nsetppchashdatas: i32,
    /// Number of found gates in presolving.
    ngates: i32,
    /// Position of the first new/changed logicor constraint in the
    /// usefullogicor array.
    firstchangedlogicor: i32,
    /// Maximal number of variables a logicor constraint has.
    maxnvarslogicor: i32,
    /// Integer parameter how to sort logicor constraints for extracting
    /// gates.
    sorting: i32,
    /// Did we find usable set-packing constraints for gate extraction.
    usefulsetppcexist: bool,
    /// Did we find usable logicor constraints for gate extraction.
    usefullogicorexist: bool,
    /// Flag indicating whether we found new set-packing constraint with two
    /// variables since the last presolving round.
    newsetppchashdatas: bool,
    /// Was data alredy be initialized.
    initialized: bool,
    /// Boolean parameter whetehr we only want to extract linear gates.
    onlysetpart: bool,
    /// Boolean parameter whetehr we want to search for equations arising from
    /// logicor and setppc constraints.
    searchequations: bool,
}

fn pd(scip: &Scip, presol: PresolId) -> &PresolData {
    scip.presols[presol].presoldata.as_ref().unwrap().downcast_ref::<PresolData>().unwrap()
}

/*
 * Local methods
 */

/// The operations of the two-variable hashdata table (`hashdataKeyEqCons`,
/// `hashdataKeyValCons`).
struct HashdataOps<'a> {
    scip: &'a Scip,
    datas: &'a [HashData],
}

impl HashOps<usize> for HashdataOps<'_> {
    type Key = HashData;

    fn get_key(&self, element: &usize) -> HashData {
        self.datas[*element]
    }

    // Returns TRUE iff both keys are equal; two constraints are equal if they
    // have the same pointer.
    fn key_eq(&self, hashdata1: &HashData, hashdata2: &HashData) -> bool {
        for v in (0..2).rev() {
            // Tests if variables are equal.
            if hashdata1.vars[v] != hashdata2.vars[v] {
                return false;
            }
        }

        // A hashdata object is only equal if it has the same constraint
        // pointer, or one has no constraint pointer, latter means that this
        // hashdata object is derived from a logicor constraint.
        hashdata1.cons == NONE || hashdata2.cons == NONE || hashdata1.cons == hashdata2.cons
    }

    // Returns the hash value of the key.
    fn key_val(&self, hashdata: &HashData) -> u64 {
        // If we have only two variables we store at each 16 bits of the hash
        // value the index of a variable.
        let hashval = ((self.scip.vars[hashdata.vars[1]].index as u32) << 16).wrapping_add(self.scip.vars[hashdata.vars[0]].index as u32);
        hashval as u64
    }
}

/// `HASHDATA` of the set-packing/-partitioning constraints of any length.
#[derive(Clone, Debug)]
struct SetppcHashData {
    /// The corresponding constraint (`NONE` for a key derived from a logicor).
    cons: ConsId,
    /// Constraint variables used for hash comparison.
    vars: Vec<VarId>,
}

/// The operations of the local set-packing table (`setppcHashdataKeyEqCons`,
/// `setppcHashdataKeyValCons`).
struct SetppcHashdataOps<'a> {
    scip: &'a Scip,
    datas: &'a [SetppcHashData],
}

impl HashOps<usize> for SetppcHashdataOps<'_> {
    type Key = SetppcHashData;

    fn get_key(&self, element: &usize) -> SetppcHashData {
        self.datas[*element].clone()
    }

    // Returns TRUE iff both keys are equal; two constraints are equal if they
    // have the same pointer.
    fn key_eq(&self, hashdata1: &SetppcHashData, hashdata2: &SetppcHashData) -> bool {
        if hashdata1.vars.len() != hashdata2.vars.len() {
            return false;
        }

        for v in (0..hashdata1.vars.len()).rev() {
            // Tests if variables are equal.
            if hashdata1.vars[v] != hashdata2.vars[v] {
                return false;
            }
        }

        // A hashdata object is only equal if it has the same constraint
        // pointer, or one has no constraint pointer, latter means that this
        // hashdata object is derived from a logicor constraint.
        hashdata1.cons == NONE || hashdata2.cons == NONE || hashdata1.cons == hashdata2.cons
    }

    // Returns the hash value of the key.
    fn key_val(&self, hashdata: &SetppcHashData) -> u64 {
        let n = hashdata.vars.len();
        let idx = |v: VarId| self.scip.vars[v].index as u32;
        crate::misc::hash_four(n as u32, idx(hashdata.vars[0]), idx(hashdata.vars[n / 2]), idx(hashdata.vars[n - 1])) as u64
    }
}

/// `presoldataInit`: initialize gateextraction presolver data.
fn presoldata_init(onlysetpart: bool, searchequations: bool, sorting: i32) -> PresolData {
    PresolData {
        setppchashdatas: Vec::new(),
        hashdatatable: None,
        setppchashtable: HashSet::new(),
        logicorhashtable: HashSet::new(),
        usefullogicor: Vec::new(),
        nusefullogicor: 0,
        nsetppchashdatas: 0,
        ngates: 0,
        firstchangedlogicor: -1,
        maxnvarslogicor: 0,
        sorting,
        usefulsetppcexist: false,
        usefullogicorexist: false,
        newsetppchashdatas: false,
        initialized: false,
        onlysetpart,
        searchequations,
    }
}

impl PresolData {
    fn table_insert(&mut self, scip: &Scip, h: usize) {
        let ops = HashdataOps {
            scip,
            datas: &self.setppchashdatas,
        };
        self.hashdatatable.as_mut().unwrap().insert(&ops, h);
    }

    fn table_remove(&mut self, scip: &Scip, h: usize) {
        let ops = HashdataOps {
            scip,
            datas: &self.setppchashdatas,
        };
        self.hashdatatable.as_mut().unwrap().remove(&ops, &h);
    }

    fn table_retrieve(&self, scip: &Scip, key: &HashData) -> Option<usize> {
        let ops = HashdataOps {
            scip,
            datas: &self.setppchashdatas,
        };
        self.hashdatatable.as_ref().unwrap().retrieve(&ops, key)
    }
}

/// Whether a variable is fixed or multi-aggregated.
fn fixed_or_multaggr(scip: &Scip, var: VarId) -> bool {
    let s = scip.vars[var].varstatus;
    s == VarStatus::Fixed || s == VarStatus::MultAggr
}

/// Whether a set-packing constraint with two variables can take part.
fn is_useful_setppc(scip: &Scip, cons: ConsId) -> bool {
    !scip.conss[cons].modifiable
        && scip.conss[cons].nupgradelocks == 0
        && crate::cons_setppc::get_type_setppc(scip, cons) == SetppcType::Packing
        && crate::cons_setppc::get_n_vars_setppc(scip, cons) == 2
}

/// The hashdata of a two-variable set-packing constraint at the next free
/// slot, or `None` if one of its representatives is fixed or
/// multi-aggregated (the block shared by `createPresoldata` and
/// `correctPresoldata`).
fn setppc_hashdata(scip: &mut Scip, cons: ConsId) -> ScipResult<Option<HashData>> {
    let setppcvars = crate::cons_setppc::get_vars_setppc(scip, cons).to_vec();
    let mut vars = [setppcvars[0], setppcvars[1]];

    vars[0] = scip_var::get_binvar_representative(scip, vars[0])?.0;
    vars[1] = scip_var::get_binvar_representative(scip, vars[1])?.0;

    if fixed_or_multaggr(scip, vars[0]) || fixed_or_multaggr(scip, vars[1]) {
        return Ok(None);
    }

    // Capture variables.
    crate::var::var_capture(scip, vars[0]);
    crate::var::var_capture(scip, vars[1]);

    // Order the variables after their index.
    if scip.vars[vars[0]].index > scip.vars[vars[1]].index {
        vars.swap(0, 1);
    }

    Ok(Some(HashData { cons, vars }))
}

/// `createPresoldata`: create useful set-packing information by adding new
/// set-packing constraints with two variables.
fn create_presoldata(scip: &mut Scip, presoldata: &mut PresolData, setppcs: &[ConsId], logicors: &[ConsId]) -> ScipResult {
    presoldata.initialized = true;

    // Temporary memory for collecting set-packing constraints.
    let mut usefulconss = Vec::new();

    if !presoldata.usefulsetppcexist {
        // Find set-packing constraints with exactly two variables.
        for &c in setppcs {
            if is_useful_setppc(scip, c) {
                // Insert new element in hashtable.
                presoldata.setppchashtable.insert(c);

                usefulconss.push(c);
            }
        }

        // Add usefulconss constraints to hashdata elements.
        if !usefulconss.is_empty() {
            presoldata.usefulsetppcexist = true;
            presoldata.setppchashdatas = vec![HashData { cons: NONE, vars: [NONE; 2] }; usefulconss.len()];

            let mut h = 0;
            for &c in &usefulconss {
                let Some(hd) = setppc_hashdata(scip, c)? else {
                    continue;
                };

                presoldata.setppchashdatas[h] = hd;

                presoldata.table_insert(scip, h);
                scip_cons::capture_cons(scip, c);

                h += 1;
            }
            presoldata.nsetppchashdatas = h as i32;

            if presoldata.nsetppchashdatas > 0 {
                presoldata.newsetppchashdatas = true;
            }
        }
    }

    usefulconss.clear();

    if !presoldata.usefullogicorexist {
        // Capture all logicor constraints.
        for &c in logicors {
            let n = crate::cons_logicor::get_n_vars_logicor(scip, c);
            if !scip.conss[c].modifiable && scip.conss[c].nupgradelocks == 0 && n >= 3 {
                // Insert new element in hashtable.
                presoldata.logicorhashtable.insert(c);
                scip_cons::capture_cons(scip, c);

                usefulconss.push(c);

                // Update maximal entries in a logicor constraint.
                if presoldata.maxnvarslogicor < n {
                    presoldata.maxnvarslogicor = n;
                }
            }
        }

        // No usefulconss constraints.
        if !usefulconss.is_empty() {
            presoldata.firstchangedlogicor = 0;
            presoldata.usefullogicorexist = true;
            presoldata.nusefullogicor = usefulconss.len() as i32;
            presoldata.usefullogicor = usefulconss;
        }
    }

    Ok(())
}

/// `cleanupHashDatas`: remove old setppchashdatas objects, so that the
/// allocated memory will stay low.
fn cleanup_hash_datas(scip: &mut Scip, presoldata: &mut PresolData) -> ScipResult {
    if !presoldata.usefulsetppcexist {
        return Ok(());
    }

    for c in (0..presoldata.nsetppchashdatas as usize).rev() {
        let hd = presoldata.setppchashdatas[c];
        let cons = hd.cons;
        let mut removeentry = false;

        let co = &scip.conss[cons];
        if co.deleted
            || co.modifiable
            || co.nupgradelocks >= 1
            || crate::cons_setppc::get_type_setppc(scip, cons) != SetppcType::Packing
            || crate::cons_setppc::get_n_vars_setppc(scip, cons) != 2
        {
            removeentry = true;
        } else {
            let v0 = scip_var::get_binvar_representative(scip, hd.vars[0])?.0;
            let v1 = scip_var::get_binvar_representative(scip, hd.vars[1])?.0;

            if fixed_or_multaggr(scip, v0) || fixed_or_multaggr(scip, v1) || hd.vars[0] != v0 || hd.vars[1] != v1 {
                removeentry = true;
            }
        }

        if removeentry {
            // Remove constraint from setppc-hashtable.
            presoldata.setppchashtable.remove(&cons);

            // Remove hashdata entry from hashtable.
            presoldata.table_remove(scip, c);

            // Release old constraints.
            let mut co = cons;
            scip_cons::release_cons(scip, &mut co)?;

            // Release variables.
            let (mut v0, mut v1) = (hd.vars[0], hd.vars[1]);
            api::release_var(scip, &mut v0)?;
            api::release_var(scip, &mut v1)?;

            let last = presoldata.nsetppchashdatas as usize - 1;
            if c < last {
                // Remove old hashdata entry from hashtable.
                presoldata.table_remove(scip, last);
            }

            // Move last content to free position.
            presoldata.setppchashdatas[c] = presoldata.setppchashdatas[last];

            if c < last {
                // Add new hashdata entry from hashtable.
                presoldata.table_insert(scip, c);
            }
            presoldata.nsetppchashdatas -= 1;
        }
    }

    Ok(())
}

/// `correctPresoldata`: refresh useful set-packing information, delete
/// redundant constraints and add new constraints; `logicors` is read live
/// (C's conshdlr array).
fn correct_presoldata(scip: &mut Scip, presoldata: &mut PresolData, setppcs: &[ConsId], logicors: &[ConsId]) -> ScipResult {
    // Check if there already exist some set-packing and some logicor
    // constraints with the right amount of variables.
    if !presoldata.usefulsetppcexist || !presoldata.usefullogicorexist {
        let usefullogicorexisted = presoldata.usefullogicorexist;

        create_presoldata(scip, presoldata, setppcs, logicors)?;

        // If we already had useful logicor constraints but did not find any
        // useful setppc constraint, the maximal number of variables appearing
        // in a logicor constraint was not updated, so we do it here.
        if usefullogicorexisted && !presoldata.usefulsetppcexist {
            // Correct maximal number of varables in logicor constraints.
            for &c in logicors.iter().rev() {
                // Update maximal entries in a logicor constraint.
                let n = crate::cons_logicor::get_n_vars_logicor(scip, c);
                if presoldata.maxnvarslogicor < n {
                    presoldata.maxnvarslogicor = n;
                }
            }
        }

        // No correct logicor or set-packing constraints available, so abort.
        if !presoldata.usefulsetppcexist || !presoldata.usefullogicorexist {
            return Ok(());
        }
    }

    // Correct old data.
    cleanup_hash_datas(scip, presoldata)?;

    let oldnsetppchashdatas = presoldata.nsetppchashdatas;

    // First update setppc part: add new setppc constraints.
    for &cons in setppcs.iter().rev() {
        if is_useful_setppc(scip, cons) {
            // Check if constraint is new, and correct array size if necessary.
            if !presoldata.setppchashtable.contains(&cons) {
                // Resize array if necessary.
                let n = presoldata.nsetppchashdatas as usize;
                if n == presoldata.setppchashdatas.len() {
                    let newsize = scip.set.calc_mem_grow_size(n as i32 + 1) as usize;

                    // Array already at maximal size.
                    if newsize <= presoldata.setppchashdatas.len() {
                        return Err(Retcode::NoMemory);
                    }

                    // Correct hashtable, remove old elements.
                    presoldata.hashdatatable.as_mut().unwrap().remove_all();

                    presoldata.setppchashdatas.resize(newsize, HashData { cons: NONE, vars: [NONE; 2] });

                    // Add all elements to the hashtable again.
                    for d in (0..n).rev() {
                        presoldata.table_insert(scip, d);
                    }
                }

                // Insert new element in hashtable.
                presoldata.setppchashtable.insert(cons);

                let Some(hd) = setppc_hashdata(scip, cons)? else {
                    continue;
                };

                presoldata.setppchashdatas[n] = hd;

                presoldata.table_insert(scip, n);
                scip_cons::capture_cons(scip, cons);

                presoldata.nsetppchashdatas += 1;
            }
        }
    }

    // If we found new set-packing constraints, we want to check against all
    // logicors.
    if oldnsetppchashdatas < presoldata.nsetppchashdatas {
        presoldata.newsetppchashdatas = true;
    }

    // Now logicor part: removed last deleted logicor constraints from local
    // presolver data.
    while presoldata.nusefullogicor > 0 && !scip.conss[presoldata.usefullogicor[presoldata.nusefullogicor as usize - 1]].is_active() {
        let last = presoldata.nusefullogicor as usize - 1;
        presoldata.logicorhashtable.remove(&presoldata.usefullogicor[last]);
        scip_cons::release_cons(scip, &mut presoldata.usefullogicor[last])?;

        presoldata.nusefullogicor -= 1;
    }

    // Remove old inactive logicor constraints.
    for c in (0..presoldata.nusefullogicor as usize).rev() {
        let cons = presoldata.usefullogicor[c];
        let n = crate::cons_logicor::get_n_vars_logicor(scip, cons);

        // Update maximal entries in a logicor constraint.
        if presoldata.maxnvarslogicor < n {
            presoldata.maxnvarslogicor = n;
        }

        let co = &scip.conss[cons];
        if !co.is_active() || co.modifiable || co.nupgradelocks >= 1 || n < 3 {
            presoldata.logicorhashtable.remove(&cons);
            scip_cons::release_cons(scip, &mut presoldata.usefullogicor[c])?;

            presoldata.usefullogicor[c] = presoldata.usefullogicor[presoldata.nusefullogicor as usize - 1];
            presoldata.nusefullogicor -= 1;
        }
    }

    presoldata.firstchangedlogicor = presoldata.nusefullogicor;

    // Add new logicor constraints.
    for &cons in logicors.iter().rev() {
        let n = crate::cons_logicor::get_n_vars_logicor(scip, cons);
        if !scip.conss[cons].modifiable && scip.conss[cons].nupgradelocks == 0 && n >= 3 {
            // Check if constraint is new, and correct array size if necessary.
            if !presoldata.logicorhashtable.contains(&cons) {
                // Insert new element in hashtable.
                presoldata.logicorhashtable.insert(cons);
                scip_cons::capture_cons(scip, cons);

                let pos = presoldata.nusefullogicor as usize;
                presoldata.usefullogicor.truncate(pos);
                presoldata.usefullogicor.push(cons);
                presoldata.nusefullogicor += 1;

                // Update maximal entries in a logicor constraint.
                if presoldata.maxnvarslogicor < n {
                    presoldata.maxnvarslogicor = n;
                }
            }
        }
    }

    Ok(())
}

/// The cached active representative of a variable (`varmap` with
/// `SCIPgetBinvarRepresentative`).
fn active_var(scip: &mut Scip, varmap: &mut HashMap<VarId, VarId>, var: VarId) -> ScipResult<VarId> {
    if let Some(&v) = varmap.get(&var) {
        return Ok(v);
    }
    let v = scip_var::get_binvar_representative(scip, var)?.0;
    varmap.insert(var, v);
    Ok(v)
}

/// The flags of a constraint (`SCIPconsIsInitial(cons)`, ...).
fn cons_flags(scip: &Scip, cons: ConsId) -> ConsFlags {
    let c = &scip.conss[cons];
    ConsFlags {
        initial: c.initial,
        separate: c.separate,
        enforce: c.enforce,
        check: c.check,
        propagate: c.propagate,
        local: c.local,
        modifiable: c.modifiable,
        dynamic: c.dynamic,
        removable: c.removable,
        stickingatnode: c.stickingatnode,
    }
}

/// `extractGates`: extract and-constraints and set-partitioning constraints.
fn extract_gates(scip: &mut Scip, presoldata: &mut PresolData, pos: usize, varmap: &mut HashMap<VarId, VarId>, ndelconss: &mut i32, naddconss: &mut i32) -> ScipResult {
    let logicor = presoldata.usefullogicor[pos];

    if !scip.conss[logicor].is_active() {
        return Ok(());
    }

    let logicorvars = crate::cons_logicor::get_vars_logicor(scip, logicor).to_vec();
    let nlogicorvars = logicorvars.len();

    let mut activevars = vec![NONE; nlogicorvars];
    let mut posresultants = Vec::with_capacity(nlogicorvars);

    // Get active logicor variables and determine all possible resultants.
    for d in (0..nlogicorvars).rev() {
        // Do not work with fixed variables.
        let v = &scip.vars[logicorvars[d]];
        if v.locdom.lb > 0.5 || v.locdom.ub < 0.5 {
            return Ok(());
        }

        activevars[d] = active_var(scip, varmap, logicorvars[d])?;

        // Determine possible resultants a check if the other variables can
        // appear in a set-packing constraint.
        let av = activevars[d];
        if scip.vars[av].varstatus == VarStatus::Negated {
            let nlocksdown = crate::var::var_get_n_locks_down_type(scip, scip.vars[av].negatedvar, crate::types::LockType::Model);
            if nlocksdown >= nlogicorvars as i32 - 1 {
                posresultants.push(av);
            } else if nlocksdown == 0 {
                return Ok(());
            }
        } else {
            let nlocksup = crate::var::var_get_n_locks_up_type(scip, av, crate::types::LockType::Model);
            if nlocksup >= nlogicorvars as i32 - 1 {
                posresultants.push(av);
            } else if nlocksup == 0 {
                return Ok(());
            }
        }
    }

    if posresultants.is_empty() {
        return Ok(());
    }

    // Sort variables after indices.
    crate::sort::sort(&mut activevars[..], (), |&a, &b| crate::var::var_comp(scip, a, b));

    // Check that we have really different variables, if not remove the
    // constraint from the hashmap and the data storage.
    for d in (1..nlogicorvars).rev() {
        if scip.vars[activevars[d]].index == scip.vars[activevars[d - 1]].index {
            presoldata.logicorhashtable.remove(&logicor);
            let mut l = logicor;
            scip_cons::release_cons(scip, &mut l)?;

            presoldata.usefullogicor[pos] = presoldata.usefullogicor[presoldata.nusefullogicor as usize - 1];
            presoldata.nusefullogicor -= 1;

            return Ok(());
        }
    }

    let mut ngateconss = 0;
    let mut gateconss = Vec::with_capacity(nlogicorvars);
    let mut hashdata = HashData { cons: NONE, vars: [NONE; 2] };

    let mut d = posresultants.len() as isize - 1;
    while d >= 0 {
        ngateconss = 0;
        gateconss.clear();
        let res = posresultants[d as usize];

        for v in (0..nlogicorvars).rev() {
            if activevars[v] == res {
                continue;
            }

            // Variables need to be sorted.
            if crate::var::var_compare(scip, res, activevars[v]) > 0 {
                hashdata.vars = [activevars[v], res];
            } else {
                hashdata.vars = [res, activevars[v]];
            }

            match presoldata.table_retrieve(scip, &hashdata) {
                Some(h) if scip.conss[presoldata.setppchashdatas[h].cons].is_active() => {
                    gateconss.push(presoldata.setppchashdatas[h].cons);
                    ngateconss += 1;
                }
                _ => break,
            }
        }
        if ngateconss == nlogicorvars - 1 {
            break;
        }
        d -= 1;
    }

    // @todo, check for clique of all variables except the resultant
    // Check if we have a set-partitioning 'gate'.
    if ngateconss == nlogicorvars - 1 && nlogicorvars == 3 {
        let res = posresultants[d as usize];
        hashdata.vars = if activevars[0] == res {
            [activevars[1], activevars[2]]
        } else if activevars[1] == res {
            [activevars[0], activevars[2]]
        } else {
            [activevars[0], activevars[1]]
        };

        if let Some(h) = presoldata.table_retrieve(scip, &hashdata)
            && scip.conss[presoldata.setppchashdatas[h].cons].is_active()
        {
            gateconss.push(presoldata.setppchashdatas[h].cons);
            ngateconss += 1;
        }
    }

    // Did we find enough (>= number of variables in logicor - 1) set-packing
    // constraints for an upgrade to either an and-constraint or even a
    // set-partitioning constraint.
    if ngateconss == nlogicorvars || (ngateconss + 1 >= nlogicorvars && !presoldata.onlysetpart) {
        let mut f = cons_flags(scip, logicor);

        for &g in gateconss.iter().rev() {
            let gc = &scip.conss[g];
            f.initial |= gc.initial;
            f.separate |= gc.separate;
            f.enforce |= gc.enforce;
            f.check |= gc.check;
            f.propagate |= gc.propagate;
            f.local &= gc.local;
            f.modifiable &= gc.modifiable;
            f.dynamic &= gc.dynamic;
            f.removable &= gc.removable;
            f.stickingatnode &= gc.stickingatnode;

            scip_cons::del_cons(scip, g)?;
            *ndelconss += 1;
        }

        if ngateconss + 1 == nlogicorvars {
            let res = posresultants[d as usize];
            let mut consvars = Vec::with_capacity(ngateconss);

            // Determine and operands.
            for v in (0..nlogicorvars).rev() {
                if activevars[v] == res {
                    continue;
                }

                consvars.push(scip_var::get_negated_var(scip, activevars[v])?);
            }

            // Create and add "and" constraint for the extracted gate.
            let name = format!("andgate_{}", presoldata.ngates);
            let mut newcons = crate::cons_and::create_cons_and(scip, &name, res, &consvars, &f)?;

            scip_cons::add_cons(scip, newcons)?;
            scip_cons::release_cons(scip, &mut newcons)?;

            *naddconss += 1;
            presoldata.ngates += 1;

            scip_cons::del_cons(scip, logicor)?;
            *ndelconss += 1;
        } else {
            // Create and add set-partitioning constraint.
            let name = format!("setpart_{}", presoldata.ngates);
            let mut newcons = crate::cons_setppc::create_cons_setpart(scip, &name, &activevars, &f)?;

            scip_cons::add_cons(scip, newcons)?;
            scip_cons::release_cons(scip, &mut newcons)?;

            *naddconss += 1;
            presoldata.ngates += 1;

            scip_cons::del_cons(scip, logicor)?;
            *ndelconss += 1;
        }
    }

    Ok(())
}

/*
 * Callback methods of presolver
 */

/// `presolCopyGateextraction`: copy method for constraint handler plugins.
fn presol_copy_gateextraction(scip: &mut Scip, _sourcescip: &Scip, _presol: PresolId) -> ScipResult {
    include_presol_gateextraction(scip)
}

/// `presolFreeGateextraction`: destructor of presolver to free user data.
fn presol_free_gateextraction(scip: &mut Scip, presol: PresolId) -> ScipResult {
    scip.presols[presol].presoldata = None;
    Ok(())
}

/// `presolExitGateextraction`: deinitialization method of presolver.
fn presol_exit_gateextraction(scip: &mut Scip, presol: PresolId) -> ScipResult {
    let mut data = scip.presols[presol].presoldata.take().unwrap();
    let presoldata = data.downcast_mut::<PresolData>().unwrap();

    let ret = (|| -> ScipResult {
        // Release old constraints.
        for c in (0..presoldata.nusefullogicor as usize).rev() {
            scip_cons::release_cons(scip, &mut presoldata.usefullogicor[c])?;
        }

        presoldata.usefullogicor = Vec::new();

        if presoldata.usefulsetppcexist {
            for c in (0..presoldata.nsetppchashdatas as usize).rev() {
                let hd = presoldata.setppchashdatas[c];

                // Remove constraint from setppc-hashtable.
                presoldata.setppchashtable.remove(&hd.cons);

                // Remove hashdata entry from hashtable.
                presoldata.table_remove(scip, c);

                // Release old constraints.
                let mut co = hd.cons;
                scip_cons::release_cons(scip, &mut co)?;

                // Release variables.
                let (mut v0, mut v1) = (hd.vars[0], hd.vars[1]);
                api::release_var(scip, &mut v0)?;
                api::release_var(scip, &mut v1)?;
            }

            presoldata.setppchashdatas = Vec::new();
        }

        if let Some(t) = presoldata.hashdatatable.as_mut() {
            // Clear old hashtable entries.
            t.remove_all();
            presoldata.setppchashtable.clear();
            presoldata.logicorhashtable.clear();
        }

        presoldata.nusefullogicor = 0;
        presoldata.nsetppchashdatas = 0;
        presoldata.firstchangedlogicor = -1;
        presoldata.ngates = 0;
        presoldata.usefullogicorexist = false;
        presoldata.usefulsetppcexist = false;
        presoldata.newsetppchashdatas = false;
        presoldata.initialized = false;

        Ok(())
    })();

    scip.presols[presol].presoldata = Some(data);
    ret
}

/// `presolExecGateextraction`: execution method of presolver.
fn presol_exec_gateextraction(
    scip: &mut Scip,
    presol: PresolId,
    _nrounds: i32,
    _presoltiming: PresolTiming,
    _newcounts: &PresolCounts,
    counts: &mut PresolCounts,
    result: &mut ScipResultCode,
) -> ScipResult {
    *result = ScipResultCode::DidNotRun;

    // Get necessary constraint handlers.
    let conshdlrsetppc = crate::set::set_find_conshdlr(scip, "setppc");
    let conshdlrlogicor = crate::set::set_find_conshdlr(scip, "logicor");

    if conshdlrsetppc == NONE || conshdlrlogicor == NONE {
        return Ok(());
    }

    // Get number of active constraints.
    let nsetppcconss = scip.conshdlrs[conshdlrsetppc].nactiveconss as usize;
    let nlogicorconss = scip.conshdlrs[conshdlrlogicor].nactiveconss as usize;

    if nsetppcconss == 0 || nlogicorconss == 0 {
        return Ok(());
    }

    let conshdlrand = crate::set::set_find_conshdlr(scip, "and");

    // Need and-constraint handler to extract and-gates.
    if conshdlrand == NONE {
        // Nothing to do when we cannot extract anything.
        if !pd(scip, presol).searchequations {
            return Ok(());
        }
        // Make sure that we correct the parameter for only extrating
        // set-partitioning constraints.
        if crate::paramset::paramset_is_fixed(scip, "presolving/gateextraction/onlysetpart")? {
            log::warn!("unfixing parameter <presolving/gateextraction/onlysetpart> in gate extration presolver");
            crate::paramset::paramset_fix(scip, "presolving/gateextraction/onlysetpart", false)?;
        }
        crate::paramset::paramset_set_bool(scip, "presolving/gateextraction/onlysetpart", true)?;
    }

    if conshdlrand != NONE && crate::paramset::paramset_get_bool(scip, "constraints/and/linearize").unwrap_or(false) {
        log::warn!("Gate-presolving is the 'counterpart' of linearizing all and-constraints, so enabling both presolving steps simultaneously does not make sense.");
    }
    *result = ScipResultCode::DidNotFind;

    // The presolver data is taken out while the presolver runs.
    let mut data = scip.presols[presol].presoldata.take().unwrap();
    let ret = exec(
        scip,
        data.downcast_mut::<PresolData>().unwrap(),
        conshdlrsetppc,
        conshdlrlogicor,
        nsetppcconss,
        nlogicorconss,
        counts,
    );
    scip.presols[presol].presoldata = Some(data);
    ret
}

/// The body of `presolExecGateextraction` once the constraint handlers are
/// known.
fn exec(
    scip: &mut Scip,
    presoldata: &mut PresolData,
    conshdlrsetppc: crate::types::ConshdlrId,
    conshdlrlogicor: crate::types::ConshdlrId,
    nsetppcconss: usize,
    nlogicorconss: usize,
    counts: &mut PresolCounts,
) -> ScipResult {
    // Get active constraints (the logicors are read live from the constraint
    // handler, as C keeps its array).
    let setppcconss = scip.conshdlrs[conshdlrsetppc].conss.0[..nsetppcconss].to_vec();
    let logicorconss_now = scip.conshdlrs[conshdlrlogicor].conss.0[..nlogicorconss].to_vec();

    // First we need to initialized the hashtables if not yet done.
    if presoldata.hashdatatable.is_none() {
        presoldata.hashdatatable = Some(HashTable::new(HASHSIZE_SETPPCCONS));
        let _ = HASHSIZE_LOGICORCONS;
    }

    presoldata.newsetppchashdatas = false;

    if !presoldata.initialized {
        // Create useful set-packing information by adding new set-packing
        // constraints with two variables.
        create_presoldata(scip, presoldata, &setppcconss, &logicorconss_now)?;
    } else {
        // Refresh useful set-packing information, delete redundant constraints
        // and add new constraints.
        correct_presoldata(scip, presoldata, &setppcconss, &logicorconss_now)?;
    }

    if presoldata.nusefullogicor != 0 {
        exec_gates(scip, presoldata, conshdlrlogicor, &setppcconss, nlogicorconss, counts)?;
    }

    // TERMINATE: remove old setppchashdatas objects.
    cleanup_hash_datas(scip, presoldata)
}

/// The part of `presolExecGateextraction` from the sorting of the logicors to
/// the gate search.
fn exec_gates(
    scip: &mut Scip,
    presoldata: &mut PresolData,
    conshdlrlogicor: crate::types::ConshdlrId,
    setppcconss: &[ConsId],
    nlogicorconss: usize,
    counts: &mut PresolCounts,
) -> ScipResult {
    // Move the biggate extraction to front or back by sort the logicors after
    // number of variables.
    if presoldata.sorting != 0 {
        let n = presoldata.nusefullogicor as usize;
        let mut lengths: Vec<i32> = presoldata.usefullogicor[..n].iter().map(|&c| crate::cons_logicor::get_n_vars_logicor(scip, c)).collect();

        if presoldata.sorting == -1 {
            crate::sort::sort_down(&mut lengths[..], (&mut presoldata.usefullogicor[..n],), crate::sort::cmp_int);
        } else {
            crate::sort::sort(&mut lengths[..], (&mut presoldata.usefullogicor[..n],), crate::sort::cmp_int);
        }
    }

    // Create the variable mapping hash map.
    let mut varmap: HashMap<VarId, VarId> = HashMap::new();

    // Search for set-partitioning constraints arising from a logicor and a
    // set-packing constraints with equal variables.
    if presoldata.searchequations && !api::is_stopped(scip) {
        // Maximal number of binary variables.
        let size = presoldata.maxnvarslogicor as usize;

        let mut setppchashdatas: Vec<SetppcHashData> = Vec::new();
        let mut setppchashdatatable = HashTable::<usize>::new(setppcconss.len() as i32);

        // Collect all set-packing/-partitioning constraints and corresponding
        // data to be able to search faster.
        for &setppc in setppcconss.iter().rev() {
            let co = &scip.conss[setppc];
            if co.deleted || co.modifiable || co.nupgradelocks >= 1 {
                continue;
            }

            // @todo if of interest could also be implemented for set-covering
            // constraints
            if crate::cons_setppc::get_type_setppc(scip, setppc) == SetppcType::Covering {
                continue;
            }

            let setppcvars = crate::cons_setppc::get_vars_setppc(scip, setppc).to_vec();
            let nsetppcvars = setppcvars.len();

            // To big setppc constraints are picked out.
            if nsetppcvars < 2 || nsetppcvars > size {
                continue;
            }

            // Get active setppc variables.
            let mut activevarssetppc = vec![NONE; nsetppcvars];
            let mut fixed = false;
            for v in (0..nsetppcvars).rev() {
                // Do not work with fixed variables.
                let var = &scip.vars[setppcvars[v]];
                if var.locdom.lb > 0.5 || var.locdom.ub < 0.5 {
                    fixed = true;
                    break;
                }

                activevarssetppc[v] = active_var(scip, &mut varmap, setppcvars[v])?;
            }

            // If we found a fixed variable we want disregard this constraint.
            if fixed {
                continue;
            }

            // Variables need to be sorted after indices to be able to do a fast
            // comparison.
            crate::sort::sort(&mut activevarssetppc[..], (), |&a, &b| crate::var::var_comp(scip, a, b));

            // Memorize set-packing data; need to capture this constraint,
            // because it might get deleted during the process.
            setppchashdatas.push(SetppcHashData {
                cons: setppc,
                vars: activevarssetppc,
            });
            scip_cons::capture_cons(scip, setppc);

            // Add entry to local hashtable.
            let h = setppchashdatas.len() - 1;
            setppchashdatatable.insert(&SetppcHashdataOps { scip, datas: &setppchashdatas }, h);
        }

        // Check all (new) logicors against all collected
        // set-packing/-partitioning constraints.
        let mut c = nlogicorconss as isize - 1;
        while c >= 0 && !api::is_stopped(scip) {
            let logicor = scip.conshdlrs[conshdlrlogicor].conss[c as usize];
            c -= 1;

            let co = &scip.conss[logicor];
            if co.deleted || co.modifiable || co.nupgradelocks >= 1 {
                continue;
            }

            let logicorvars = crate::cons_logicor::get_vars_logicor(scip, logicor).to_vec();
            let nlogicorvars = logicorvars.len();

            if nlogicorvars < 2 {
                continue;
            }

            // Get active logicor variables.
            let mut activevarslogicor = vec![NONE; nlogicorvars];
            let mut complete = true;
            for v in (0..nlogicorvars).rev() {
                // Do not work with fixed variables.
                let var = &scip.vars[logicorvars[v]];
                if var.locdom.lb > 0.5 || var.locdom.ub < 0.5 {
                    complete = false;
                    break;
                }

                // If image does not exist, then there is no corresponding
                // set-packing constraint.
                match varmap.get(&logicorvars[v]) {
                    Some(&a) => activevarslogicor[v] = a,
                    None => {
                        complete = false;
                        break;
                    }
                }
            }

            if complete {
                // Need sorting to be able to find the correct hashdata element.
                crate::sort::sort(&mut activevarslogicor[..], (), |&a, &b| crate::var::var_comp(scip, a, b));

                let hashdata = SetppcHashData {
                    cons: NONE,
                    vars: activevarslogicor.clone(),
                };

                let found = setppchashdatatable.retrieve(&SetppcHashdataOps { scip, datas: &setppchashdatas }, &hashdata);

                if let Some(h) = found
                    && !scip.conss[setppchashdatas[h].cons].deleted
                {
                    let setppc = setppchashdatas[h].cons;
                    let (l, s) = (cons_flags(scip, logicor), cons_flags(scip, setppc));

                    let f = ConsFlags {
                        initial: l.initial || s.initial,
                        separate: l.separate || s.separate,
                        enforce: l.enforce || s.enforce,
                        check: l.check || s.check,
                        propagate: l.propagate || s.propagate,
                        local: l.local && s.local,
                        modifiable: l.modifiable && s.modifiable,
                        dynamic: l.dynamic && s.dynamic,
                        removable: l.removable && s.removable,
                        stickingatnode: l.stickingatnode && s.stickingatnode,
                    };

                    // Check if logicor is redundant against a set-partitioning
                    // constraint.
                    if crate::cons_setppc::get_type_setppc(scip, setppc) == SetppcType::Partitioning {
                        scip_cons::set_cons_initial(scip, setppc, f.initial);
                        scip_cons::set_cons_separated(scip, setppc, f.separate);
                        scip_cons::set_cons_enforced(scip, setppc, f.enforce);
                        scip_cons::set_cons_checked(scip, setppc, f.check)?;
                        scip_cons::set_cons_propagated(scip, setppc, f.propagate);
                        // SCIPsetConsLocal, ...Modifiable, ...Dynamic,
                        // ...Removable, ...StickingAtNode.
                        let sc = &mut scip.conss[setppc];
                        sc.local = f.local;
                        if !f.local {
                            sc.validdepth = 0;
                        }
                        sc.modifiable = f.modifiable;
                        sc.dynamic = f.dynamic;
                        sc.removable = f.removable;
                        sc.stickingatnode = f.stickingatnode;
                    } else {
                        // Create and add set-partitioning constraint.
                        let name = format!("setpart_{}", presoldata.ngates);
                        let mut newcons = crate::cons_setppc::create_cons_setpart(scip, &name, &activevarslogicor, &f)?;

                        scip_cons::add_cons(scip, newcons)?;
                        scip_cons::release_cons(scip, &mut newcons)?;

                        counts.naddconss += 1;
                        presoldata.ngates += 1;

                        // Delete redundant set-packing constraint.
                        scip_cons::del_cons(scip, setppc)?;
                        counts.ndelconss += 1;
                    }

                    // Delete redundant logicor constraint.
                    scip_cons::del_cons(scip, logicor)?;
                    counts.ndelconss += 1;
                }
            }
        }

        // Need to clear/release parts of hashdata objects.
        for d in (0..setppchashdatas.len()).rev() {
            // Need to release captured constraint.
            scip_cons::release_cons(scip, &mut setppchashdatas[d].cons)?;
        }
    }

    // We do not have any useful set-packing or logicor constraint, or since
    // last run did not get any new constraints, so abort.
    if presoldata.nsetppchashdatas == 0 || (presoldata.firstchangedlogicor == presoldata.nusefullogicor && !presoldata.newsetppchashdatas) {
        return Ok(());
    }

    // Search for gates.
    if presoldata.nsetppchashdatas > 0 && !api::is_stopped(scip) {
        // If we found new setppcs we want to check all logicors again.
        let endloop = if presoldata.newsetppchashdatas {
            0
        } else if presoldata.firstchangedlogicor >= 0 {
            presoldata.firstchangedlogicor
        } else {
            0
        };

        // Check all (new) logicors against all set-packing constraints, to
        // extract and-constraints with two or more operands or
        // set-partitioning constraints three or more variables.
        //
        // logicor constraint has the form: x + y + z >= 1
        //
        // find set-packing constraints:  (~x + ~y >= 1 and ~x + ~z >= 1)  <=>  (x + y <= 1 and x + z <= 1)
        //
        // - these three constraints are equivalent to: x = ~y * ~z (x = AND(~y,~z))
        //
        // if an additional set-packing constraint exists: y + z <= 1
        //
        // - these four constraints are equivalent to: x + y + z = 1
        let mut c = presoldata.nusefullogicor - 1;
        while c >= endloop && !api::is_stopped(scip) {
            extract_gates(scip, presoldata, c as usize, &mut varmap, &mut counts.ndelconss, &mut counts.naddconss)?;
            c -= 1;
        }
    }

    Ok(())
}

/*
 * presolver specific interface methods
 */

/// `SCIPincludePresolGateextraction`: creates the gateextraction presolver
/// and includes it in SCIP.
pub fn include_presol_gateextraction(scip: &mut Scip) -> ScipResult {
    // Alloc presolve data object and initialize gateextraction presolver data.
    let presoldata = presoldata_init(false, false, 0);

    // Include presolver.
    let presol = api::include_presol_basic(
        scip,
        PRESOL_NAME,
        PRESOL_DESC,
        PRESOL_PRIORITY,
        PRESOL_MAXROUNDS,
        PRESOL_TIMING,
        presol_exec_gateextraction,
        Some(Box::new(presoldata) as Box<dyn Any>),
    )?;

    let p = &mut scip.presols[presol];
    p.presolcopy = Some(presol_copy_gateextraction);
    p.presolfree = Some(presol_free_gateextraction);
    p.presolexit = Some(presol_exit_gateextraction);

    macro_rules! t {
        ($ty:ty, $f:ident) => {
            crate::data_target!($ty, presols, presol, presoldata, PresolData, $f)
        };
    }

    // Add gateextraction presolver parameters.
    crate::paramset::paramset_add_bool(
        scip,
        "presolving/gateextraction/onlysetpart",
        "should we only try to extract set-partitioning constraints and no and-constraints",
        t!(bool, onlysetpart),
        true,
        DEFAULT_ONLYSETPART,
        None,
    )?;
    crate::paramset::paramset_add_bool(
        scip,
        "presolving/gateextraction/searchequations",
        "should we try to extract set-partitioning constraint out of one logicor and one corresponding set-packing constraint",
        t!(bool, searchequations),
        true,
        DEFAULT_SEARCHEQUATIONS,
        None,
    )?;
    crate::paramset::paramset_add_int(
        scip,
        "presolving/gateextraction/sorting",
        "order logicor contraints to extract big-gates before smaller ones (-1), do not order them (0) or order them to extract smaller gates at first (1)",
        t!(i32, sorting),
        true,
        DEFAULT_SORTING,
        -1,
        1,
        None,
    )?;

    Ok(())
}
