//! SCIP's cut pools (`cutpool.c`, `struct_cutpool.h`): globally valid cuts
//! kept for later separation rounds.

use std::cell::RefCell;

use crate::{
    clock::Clock,
    cvec::CVec,
    hash::{HashOps, HashTable},
    lp,
    misc::{hash_two, real_hash_code},
    retcode::{Retcode, ScipResult},
    scip::Scip,
    sepastore,
    types::{LpSolStat, RowId, ScipResultCode, SolId},
    var::NONE,
};

/// The global cut pool in `Scip::cutpools` (`scip->cutpool`).
pub const GLOBALCUTPOOL: usize = 0;
/// The delayed global cut pool (`scip->delayedcutpool`).
pub const DELAYEDCUTPOOL: usize = 1;

pub type CutId = usize;

/// `SCIP_CUT`.
#[derive(Clone, Copy, Debug)]
pub struct Cut {
    pub row: RowId,
    pub processedlp: i64,
    pub processedlpsol: i64,
    pub age: i32,
    pub pos: i32,
}

/// `SCIP_CUTPOOL`.
#[derive(Clone, Debug)]
pub struct Cutpool {
    pub ncalls: i64,
    pub nrootcalls: i64,
    pub ncutsfound: i64,
    pub ncutsadded: i64,
    pub poolclock: Clock,
    pub hashtable: HashTable<CutId>,
    pub cuts: CVec<CutId>,
    pub processedlp: i64,
    pub processedlpsol: i64,
    pub processedlpefficacy: f64,
    pub processedlpsolefficacy: f64,
    pub cutssize: i32,
    pub ncuts: i32,
    pub nremovablecuts: i32,
    pub agelimit: i32,
    pub firstunprocessed: i32,
    pub firstunprocessedsol: i32,
    pub maxncuts: i64,
    pub globalcutpool: bool,
}

/*
 * Hash functions
 */

/// The cut pool's hash callbacks. Comparing and hashing rows sorts them
/// and computes their norms, so the callbacks reach the solver mutably;
/// the pool's table is taken out of the solver while they run.
struct CutOps<'a> {
    scip: RefCell<&'a mut Scip>,
}

impl HashOps<CutId> for CutOps<'_> {
    type Key = RowId;

    /// `hashGetKeyCut`: gets the hash key of a cut; the key of a cut is the
    /// row.
    fn get_key(&self, cut: &CutId) -> RowId {
        self.scip.borrow().cuts[*cut].row
    }

    /// `hashKeyEqCut`: returns TRUE iff both cuts are identical.
    fn key_eq(&self, key1: &RowId, key2: &RowId) -> bool {
        let scip = &mut **self.scip.borrow_mut();
        let (mut row1, mut row2) = (*key1, *key2);

        // Return true if the row is the same.
        if row1 == row2 {
            return true;
        }

        // Compare the trivial characteristics of the rows.
        {
            let (r1, r2) = (&scip.rows[row1], &scip.rows[row2]);
            if r1.len != r2.len || r1.minidx != r2.minidx || r1.maxidx != r2.maxidx {
                return false;
            }
        }

        // Set scale for the rows such that the largest absolute coefficient
        // is 1.0.
        let mut row1scale = 1.0 / lp::row_get_maxval(scip, row1);
        let mut row2scale = 1.0 / lp::row_get_maxval(scip, row2);

        // Check if scaled min value is feas equal first.
        let min1 = row1scale * lp::row_get_minval(scip, row1);
        let min2 = row2scale * lp::row_get_minval(scip, row2);
        if !scip.set.is_feas_eq(min1, min2) {
            return false;
        }

        lp::row_sort(scip, row1);
        lp::row_sort(scip, row2);

        // Set scale sign such that the rows are of the form ax <= b.
        if scip.set.is_infinity(scip.rows[row1].rhs) {
            row1scale = -row1scale;
        }
        if scip.set.is_infinity(scip.rows[row2].rhs) {
            row2scale = -row2scale;
        }

        let set = &scip.set;
        let (r1, r2) = (&scip.rows[row1], &scip.rows[row2]);

        // Both rows have LP columns, or none of them has, or one has only LP
        // colums and the other only non-LP columns, so we can rely on the
        // sorting of the columns.
        if (r1.nlpcols == 0) == (r2.nlpcols == 0) || (r1.nlpcols == 0 && r2.nlpcols == r2.len) || (r1.nlpcols == r1.len && r2.nlpcols == 0) {
            // Both rows are linked and the number of lpcolumns is not equal
            // so they cannot be equal.
            if (r1.nlpcols == 0) == (r2.nlpcols == 0) && r1.nlpcols != r2.nlpcols {
                return false;
            }

            // Compare the columns of the rows.
            for i in 0..r1.len {
                if r1.cols_index[i] != r2.cols_index[i] {
                    return false;
                }
            }

            // Compare the coefficients of the rows.
            for i in 0..r1.len {
                if !set.is_feas_eq(row1scale * r1.vals[i], row2scale * r2.vals[i]) {
                    return false;
                }
            }
        }
        // One row has LP columns, but the other not, that could be because
        // the one without was just created and isn't linked yet; in this
        // case, one column could be an LP column in one row and a non-LP
        // column in the other row, so we cannot rely on the partition; thus,
        // we iteratively check whether the next column of row1 is either the
        // next LP column of row2 or the next non-LP column of row2 and the
        // coefficients are equal.
        else {
            // Ensure that row1 is the row without LP columns, switch the
            // rows, if neccessary.
            if r2.nlpcols == 0 {
                std::mem::swap(&mut row1, &mut row2);
                std::mem::swap(&mut row1scale, &mut row2scale);
            }
            let (r1, r2) = (&scip.rows[row1], &scip.rows[row2]);

            let mut ilp = 0;
            let mut inlp = r2.nlpcols;

            // Compare the columns and coefficients of the rows.
            for i1 in 0..r1.len {
                // Current column of row1 is the current LP column of row2,
                // check the coefficient.
                if ilp < r2.nlpcols && r1.cols[i1] == r2.cols[ilp] {
                    if !set.is_feas_eq(row1scale * r1.vals[i1], row2scale * r2.vals[ilp]) {
                        return false;
                    } else {
                        ilp += 1;
                    }
                }
                // Current column of row1 is the current non-LP column of
                // row2, check the coefficient.
                else if inlp < r2.len && r1.cols[i1] == r2.cols[inlp] {
                    if !set.is_feas_eq(row1scale * r1.vals[i1], row2scale * r2.vals[inlp]) {
                        return false;
                    } else {
                        inlp += 1;
                    }
                }
                // Current column of row1 is neither the current LP column of
                // row2, nor the current non-LP column of row 2.
                else {
                    return false;
                }
            }
        }
        true
    }

    /// `hashKeyValCut`.
    fn key_val(&self, key: &RowId) -> u64 {
        let scip = &mut **self.scip.borrow_mut();
        let row = *key;

        let mut scale = 1.0 / lp::row_get_maxval(scip, row);
        if scip.set.is_infinity(scip.rows[row].rhs) {
            scale = -scale;
        }

        let r = &scip.rows[row];
        let mut hash = r.len as i64 as u64;
        for i in 0..r.len {
            let val = scale * r.vals[i];
            hash = hash.wrapping_add(hash_two(real_hash_code(val), r.cols_index[i] as u32) as u64);
        }
        hash
    }
}

/// Runs `f` on pool `p`'s hash table with the cut callbacks.
fn with_table<R>(scip: &mut Scip, p: usize, f: impl FnOnce(&mut HashTable<CutId>, &CutOps) -> R) -> R {
    let mut table = std::mem::replace(&mut scip.cutpools[p].hashtable, HashTable::new(0));
    let r = {
        let ops = CutOps { scip: RefCell::new(&mut *scip) };
        f(&mut table, &ops)
    };
    scip.cutpools[p].hashtable = table;
    r
}

/*
 * dynamic memory arrays
 */

/// `cutpoolEnsureCutsMem`: resizes cuts array to be able to store at least
/// `num` entries.
fn cutpool_ensure_cuts_mem(scip: &mut Scip, p: usize, num: i32) {
    if num > scip.cutpools[p].cutssize {
        let newsize = scip.set.calc_mem_grow_size(num);
        scip.cutpools[p].cuts.0.resize(newsize as usize, NONE);
        scip.cutpools[p].cutssize = newsize;
    }
}

/*
 * Cut methods
 */

/// `cutCreate`: creates a cut and captures the row.
fn cut_create(scip: &mut Scip, row: RowId) -> CutId {
    // Allocate cut memory.
    let cut = scip.cuts.push(Cut {
        row,
        age: 0,
        processedlp: -1,
        processedlpsol: -1,
        pos: -1,
    });

    // Capture row.
    lp::row_capture(scip, row);
    cut
}

/// `cutFree`: frees a cut and releases the row.
fn cut_free(scip: &mut Scip, cut: CutId) -> ScipResult {
    // Release row.
    let row = scip.cuts[cut].row;
    lp::row_release(scip, row)?;

    // Free cut memory.
    scip.cuts.remove(cut);
    Ok(())
}

/// `cutIsAged`: returns whether the cut's age exceeds the age limit; since
/// agelimit can be -1 cast to unsigned before comparison, then it is the
/// maximum unsigned value in that case.
fn cut_is_aged(cut: &Cut, agelimit: i32) -> bool {
    cut.age as u32 > agelimit as u32
}

/// `SCIPcutGetLPActivityQuot`: returns the ratio of LPs where the row
/// belonging to this cut was active in an LP solution, i.e. where the age
/// of its row has not been increased.
pub fn cut_get_lp_activity_quot(scip: &Scip, cut: CutId) -> f64 {
    let row = &scip.rows[scip.cuts[cut].row];
    let nlpsaftercreation = row.nlpsaftercreation;
    let activeinlpcounter = row.activeinlpcounter;
    if nlpsaftercreation > 0 {
        activeinlpcounter as f64 / nlpsaftercreation as f64
    } else {
        0.0
    }
}

/*
 * Cutpool methods
 */

/// `SCIPcutpoolCreate`: creates cut pool.
pub fn cutpool_create(scip: &Scip, agelimit: i32, globalcutpool: bool) -> Cutpool {
    let tablesize = if scip.set.misc_usesmalltables {
        crate::def::HASHSIZE_CUTPOOLS_SMALL
    } else {
        crate::def::HASHSIZE_CUTPOOLS
    };
    Cutpool {
        poolclock: Clock::new(),
        hashtable: HashTable::new(tablesize as i32),
        cuts: CVec::new(),
        cutssize: 0,
        ncuts: 0,
        nremovablecuts: 0,
        agelimit,
        processedlp: -1,
        processedlpsol: -1,
        processedlpefficacy: crate::def::INVALID,
        processedlpsolefficacy: crate::def::INVALID,
        firstunprocessed: 0,
        firstunprocessedsol: 0,
        maxncuts: 0,
        ncalls: 0,
        nrootcalls: 0,
        ncutsfound: 0,
        ncutsadded: 0,
        globalcutpool,
    }
}

/// `SCIPcutpoolClear`: removes all rows from the cut pool.
pub fn cutpool_clear(scip: &mut Scip, p: usize) -> ScipResult {
    // Free cuts.
    scip.cutpools[p].hashtable.remove_all();
    for i in 0..scip.cutpools[p].ncuts {
        let cut = scip.cutpools[p].cuts[i];
        let row = scip.cuts[cut].row;
        if scip.cutpools[p].globalcutpool {
            scip.rows[row].inglobalcutpool = false;
        }
        lp::row_unlock(scip, row);
        cut_free(scip, cut)?;
    }
    scip.cutpools[p].ncuts = 0;
    scip.cutpools[p].nremovablecuts = 0;
    Ok(())
}

/// `cutpoolDelCut`: removes the cut from the cut pool.
fn cutpool_del_cut(scip: &mut Scip, p: usize, cut: CutId) -> ScipResult {
    let pos = scip.cuts[cut].pos;
    let row = scip.cuts[cut].row;

    // Decrease the number of removable cuts counter (row might have changed
    // its removable status -> counting might not be correct.
    if scip.rows[row].removable && scip.cutpools[p].nremovablecuts > 0 {
        scip.cutpools[p].nremovablecuts -= 1;
    }

    // If this is the global cut pool of SCIP, mark the row to not be member
    // anymore.
    if scip.cutpools[p].globalcutpool {
        scip.rows[row].inglobalcutpool = false;
    }

    // Remove the cut from the hash table.
    with_table(scip, p, |table, ops| table.remove(ops, &cut));

    // Unlock the row.
    lp::row_unlock(scip, row);

    // Free the cut.
    cut_free(scip, cut)?;

    let lpcount = scip.stat.lpcount;
    let pool = &mut scip.cutpools[p];
    pool.ncuts -= 1;
    pool.firstunprocessed = pool.firstunprocessed.min(pool.ncuts);
    pool.firstunprocessedsol = pool.firstunprocessedsol.min(pool.ncuts);

    // Move the last cut of the pool to the free position.
    if pos < pool.ncuts {
        let moved = pool.cuts[pool.ncuts];
        pool.cuts[pos] = moved;
        let m = &mut scip.cuts[moved];
        m.pos = pos;
        let (plp, plpsol) = (m.processedlp, m.processedlpsol);
        let pool = &mut scip.cutpools[p];
        if plp < lpcount {
            pool.firstunprocessed = pool.firstunprocessed.min(pos);
        }
        if plpsol < lpcount {
            pool.firstunprocessedsol = pool.firstunprocessedsol.min(pos);
        }
    }
    Ok(())
}

/// The scaled right hand side of `row` in the form ax <= b, with the
/// largest absolute coefficient scaled to 1.0.
fn scaled_rhs(scip: &mut Scip, row: RowId) -> f64 {
    let scale = 1.0 / lp::row_get_maxval(scip, row);
    let r = &scip.rows[row];
    if scip.set.is_infinity(r.rhs) {
        scale * (r.constant - r.lhs)
    } else {
        scale * (r.rhs - r.constant)
    }
}

/// `SCIPcutpoolIsCutNew`: checks if cut is already existing.
pub fn cutpool_is_cut_new(scip: &mut Scip, p: usize, row: RowId) -> bool {
    if scip.rows[row].len == 0 {
        // Trivial cut is only new if it proves infeasibility.
        let r = &scip.rows[row];
        return scip.set.is_feas_lt(r.constant, r.lhs) || scip.set.is_feas_gt(r.constant, r.rhs);
    }

    let othercut = with_table(scip, p, |table, ops| table.retrieve(ops, &row));

    // Check in hash table, if cut already exists in the pool.
    match othercut {
        None => true,
        Some(othercut) if scip.cuts[othercut].row != row => {
            // Since we are comparing the improvement with an absolute value,
            // we apply a scale to both rows such that the max absolute value
            // is 1.0. Then bring the cut into the form ax <= b.
            let otherrow = scip.cuts[othercut].row;
            let otherrhs = scaled_rhs(scip, otherrow);
            let rhs = scaled_rhs(scip, row);
            scip.set.is_feas_lt(rhs, otherrhs)
        }
        Some(_) => false,
    }
}

/// `SCIPcutpoolAddRow`: if not already existing, adds row to cut pool and
/// captures it.
pub fn cutpool_add_row(scip: &mut Scip, p: usize, row: RowId) -> ScipResult {
    if scip.rows[row].len == 0 {
        return Ok(());
    }

    // Ensure validity of minidx and maxidx for hash key comparison.
    lp::row_get_maxidx(scip, row);

    let othercut = with_table(scip, p, |table, ops| table.retrieve(ops, &row));

    // Check in hash table, if cut already exists in the pool.
    match othercut {
        None => cutpool_add_new_row(scip, p, row)?,
        Some(othercut) => {
            // Since we are comparing the improvement with an absolute value,
            // we apply a scale to both rows such that the max absolute value
            // is 1.0. Then bring the cut into the form ax <= b.
            let otherrow = scip.cuts[othercut].row;
            let otherrhs = scaled_rhs(scip, otherrow);
            let rhs = scaled_rhs(scip, row);

            if scip.set.is_feas_lt(rhs, otherrhs) {
                cutpool_del_cut(scip, p, othercut)?;

                // Use recursion, since in rare cases new cut might compare
                // equal to multiple other cuts that do not compare equal
                // themselve due to non-transitivity of epsilon comparisons.
                cutpool_add_row(scip, p, row)?;
            }
        }
    }
    Ok(())
}

/// `SCIPcutpoolAddNewRow`: adds row to cut pool and captures it; doesn't
/// check for multiple cuts.
pub fn cutpool_add_new_row(scip: &mut Scip, p: usize, row: RowId) -> ScipResult {
    // Check, if row is modifiable or local.
    if scip.rows[row].modifiable {
        log::error!("cannot store modifiable row <{}> in a cut pool", scip.rows[row].name);
        return Err(Retcode::InvalidData);
    }
    if scip.rows[row].local {
        log::error!("cannot store locally valid row <{}> in a cut pool", scip.rows[row].name);
        return Err(Retcode::InvalidData);
    }

    // Ensure validity of minidx and maxidx for hash key comparison.
    lp::row_get_maxidx(scip, row);

    // Create the cut.
    let cut = cut_create(scip, row);
    scip.cuts[cut].pos = scip.cutpools[p].ncuts;

    // Add cut to the pool.
    let n = scip.cutpools[p].ncuts;
    cutpool_ensure_cuts_mem(scip, p, n + 1);
    let removable = scip.rows[row].removable;
    let pool = &mut scip.cutpools[p];
    pool.cuts[n] = cut;
    pool.ncuts += 1;
    pool.ncutsfound += 1;
    pool.maxncuts = pool.maxncuts.max(pool.ncuts as i64);
    if removable {
        pool.nremovablecuts += 1;
    }

    // Insert cut in the hash table.
    with_table(scip, p, |table, ops| table.insert(ops, cut));

    if lp::lp_get_solstat(scip) == LpSolStat::Optimal {
        let thisefficacy = lp::row_get_lp_efficacy(scip, row);
        scip.stat.bestefficacy = crate::def::max(thisefficacy, scip.stat.bestefficacy);
    }

    // If this is the global cut pool of SCIP, mark the row to be member of
    // the pool.
    if scip.cutpools[p].globalcutpool {
        scip.rows[row].inglobalcutpool = true;
    }

    // Lock the row.
    lp::row_lock(scip, row);
    Ok(())
}

/// `SCIPcutpoolDelRow`: removes the LP row from the cut pool.
pub fn cutpool_del_row(scip: &mut Scip, p: usize, row: RowId) -> ScipResult {
    // Find the cut in hash table.
    let Some(cut) = with_table(scip, p, |table, ops| table.retrieve(ops, &row)) else {
        log::error!("row <{}> is not existing in cutpool {}", scip.rows[row].name, p);
        return Err(Retcode::InvalidData);
    };

    cutpool_del_cut(scip, p, cut)
}

/// `SCIPcutpoolSeparate`: separates cuts of the cut pool; `sol` is `NONE`
/// for the LP solution.
#[allow(clippy::too_many_arguments)]
pub fn cutpool_separate(scip: &mut Scip, p: usize, sepastore: usize, eventfilter: usize, sol: SolId, cutpoolisdelayed: bool, root: bool) -> ScipResult<ScipResultCode> {
    let mut result = ScipResultCode::DidNotRun;
    let lpcount = scip.stat.lpcount;

    // Don't separate cut pool in the root node, if there are no removable
    // cuts.
    if root && scip.cutpools[p].nremovablecuts == 0 {
        return Ok(result);
    }

    let firstunproc;
    {
        let pool = &mut scip.cutpools[p];
        if sol == NONE {
            if pool.processedlp < lpcount {
                pool.firstunprocessed = 0;
            }
            if pool.firstunprocessed == pool.ncuts {
                return Ok(result);
            }
            firstunproc = pool.firstunprocessed;
        } else {
            if pool.processedlpsol < lpcount {
                pool.firstunprocessedsol = 0;
            }
            if pool.firstunprocessedsol == pool.ncuts {
                return Ok(result);
            }
            firstunproc = pool.firstunprocessedsol;
        }
    }

    result = ScipResultCode::DidNotFind;
    scip.cutpools[p].ncalls += 1;
    if root {
        scip.cutpools[p].nrootcalls += 1;
    }
    let mut found = false;

    let minefficacy = if scip.set.sepa_filtercutpoolrel {
        scip.stat.bestefficacy * scip.stat.minefficacyfac
    } else if root {
        scip.set.sepa_minefficacyroot
    } else {
        scip.set.sepa_minefficacy
    };

    let retest;
    {
        let pool = &mut scip.cutpools[p];
        if sol == NONE {
            retest = pool.processedlpefficacy > minefficacy;
            pool.processedlpefficacy = minefficacy;
        } else {
            retest = pool.processedlpsolefficacy > minefficacy;
            pool.processedlpsolefficacy = minefficacy;
        }
    }

    // Start timing.
    let enabled = scip.set.time_enabled;
    scip.cutpools[p].poolclock.start(enabled);

    // Remember the current total number of found cuts.
    let oldncutsfound = scip.sepastores[sepastore].ncuts;
    let oldncutsadded = scip.sepastores[sepastore].ncutsadded;
    let mut nefficaciouscuts = 0;

    // Process all unprocessed cuts in the pool.
    let mut cutoff = false;
    let mut c = firstunproc;
    while c < scip.cutpools[p].ncuts {
        let cut = scip.cutpools[p].cuts[c];
        let proclp = if sol == NONE { scip.cuts[cut].processedlp } else { scip.cuts[cut].processedlpsol };

        if retest || proclp < lpcount {
            if sol == NONE {
                scip.cuts[cut].processedlp = lpcount;
            } else {
                scip.cuts[cut].processedlpsol = lpcount;
            }

            let row = scip.cuts[cut].row;
            if scip.rows[row].lppos < 0 {
                // If the cut is a bound change (i.e. a row with only one
                // variable), add it as bound change instead of LP row; hence,
                // we want to remove the bound change cut from the SCIP cut
                // pool.
                if !scip.rows[row].modifiable && scip.rows[row].len == 1 {
                    // Insert bound change cut into separation store which will
                    // force that cut; fromcutpool is set for consistency.
                    scip.rows[row].fromcutpool = true;
                    cutoff = sepastore::sepastore_add_cut(scip, sepastore, eventfilter, row, false, root)?;
                    cutpool_del_cut(scip, p, cut)?;
                    if cutoff {
                        break;
                    }
                    c += 1;
                    continue;
                }

                let efficacy = if sol == NONE {
                    lp::row_get_lp_efficacy(scip, row)
                } else {
                    lp::row_get_sol_efficacy(scip, row, sol)
                };
                if scip.set.is_feas_positive(efficacy) {
                    nefficaciouscuts += 1;
                }

                if efficacy >= minefficacy {
                    // Insert cut in separation storage.
                    scip.rows[row].fromcutpool = true;
                    cutoff = sepastore::sepastore_add_cut(scip, sepastore, eventfilter, row, false, root)?;

                    // Count cuts.
                    if cutpoolisdelayed {
                        let sepa = lp::row_get_origin_sepa(scip, row);
                        if sepa != NONE {
                            crate::sepa::sepa_inc_ncuts_added(scip, sepa, true);
                            crate::sepa::sepa_inc_ncuts_found_at_node(scip, sepa);
                        } else {
                            let conshdlr = lp::row_get_origin_conshdlr(scip, row);
                            if conshdlr != NONE {
                                scip.conshdlrs[conshdlr].ncutsfound += 1;
                            }
                        }
                    }

                    found = true;
                    scip.cuts[cut].age = 0;

                    if cutoff {
                        break;
                    }
                } else {
                    scip.cuts[cut].age += 1;
                    if cut_is_aged(&scip.cuts[cut], scip.cutpools[p].agelimit) {
                        cutpool_del_cut(scip, p, cut)?;
                    }
                }
            }
        }
        c += 1;
    }

    {
        let pool = &mut scip.cutpools[p];
        if sol == NONE {
            pool.processedlp = lpcount;
            pool.firstunprocessed = pool.ncuts;
        } else {
            pool.processedlpsol = lpcount;
            pool.firstunprocessedsol = pool.ncuts;
        }
    }

    // Update the number of found and added cuts.
    scip.cutpools[p].ncutsadded += (scip.sepastores[sepastore].ncutsadded - oldncutsadded) as i64;

    // Check whether efficacy threshold should be tightened or relaxed.
    if scip.set.sepa_filtercutpoolrel && nefficaciouscuts > 0 {
        let mut maxncuts = scip.set.get_sepa_maxcuts(root);
        let ncuts = scip.sepastores[sepastore].ncuts - oldncutsfound;

        maxncuts = maxncuts.min(nefficaciouscuts);

        let stat = &mut scip.stat;
        if ncuts as f64 > 0.5 * maxncuts as f64 {
            stat.ncutpoolfails = (stat.ncutpoolfails - 1).min(-1);
        } else if ncuts == 0 || (ncuts as f64) < 0.05 * maxncuts as f64 {
            stat.ncutpoolfails = (stat.ncutpoolfails + 1).max(1);
        }

        if stat.ncutpoolfails == if root { 2 } else { 10 } {
            let pool = &mut scip.cutpools[p];
            pool.firstunprocessed = 0;
            pool.firstunprocessedsol = 0;
            let stat = &mut scip.stat;
            stat.minefficacyfac *= 0.5;
            stat.ncutpoolfails = 0;
        } else if stat.ncutpoolfails == -2 {
            stat.minefficacyfac *= 1.2;
            stat.ncutpoolfails = 0;
        }
    }

    // Stop timing.
    scip.cutpools[p].poolclock.stop(enabled);

    if cutoff {
        result = ScipResultCode::Cutoff;
    } else if found {
        result = ScipResultCode::Separated;
    }
    Ok(result)
}

/// `SCIPcutpoolGetNCuts`: gets number of cuts in the cut pool.
pub fn cutpool_get_ncuts(scip: &Scip, p: usize) -> i32 {
    scip.cutpools[p].ncuts
}

/// `SCIPcutpoolGetCuts`: gets array of cuts in the cut pool.
pub fn cutpool_get_cuts(scip: &Scip, p: usize) -> &[CutId] {
    let pool = &scip.cutpools[p];
    pool.cuts.head(pool.ncuts)
}
