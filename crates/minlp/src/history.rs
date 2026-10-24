//! SCIP's branching and inference history (`history.c`; the reference is
//! built without `NDEBUG`, so the functions, not `history.h`'s macros).

use crate::{def::max, set::Set, sort, types::BranchDir};

/// `SCIP_HISTORY`. Arrays are indexed by `SCIP_BRANCHDIR` downwards (0)
/// and upwards (1).
#[derive(Clone, Debug, Default)]
pub struct History {
    pub pscostcount: [f64; 2],
    pub pscostweightedmean: [f64; 2],
    pub pscostvariance: [f64; 2],
    pub ancpscostcount: [f64; 2],
    pub ancpscostweightedmean: [f64; 2],
    pub vsids: [f64; 2],
    pub conflengthsum: [f64; 2],
    pub inferencesum: [f64; 2],
    pub cutoffsum: [f64; 2],
    pub ratio: f64,
    pub balance: f64,
    pub gmieff: f64,
    pub gmieffsum: f64,
    pub ngmi: f64,
    pub ratiovalid: bool,
    pub nactiveconflicts: [i64; 2],
    pub nbranchings: [i64; 2],
    pub branchdepthsum: [i64; 2],
}

/// `SCIPbranchdirOpposite`.
pub fn branchdir_opposite(dir: BranchDir) -> BranchDir {
    match dir {
        BranchDir::Downwards => BranchDir::Upwards,
        BranchDir::Upwards => BranchDir::Downwards,
        _ => BranchDir::Auto,
    }
}

impl History {
    /// `SCIPhistoryCreate`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `SCIPhistoryReset`.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// `SCIPhistoryUnite`: adds `addhistory`, with its directions swapped
    /// if `switcheddirs`.
    pub fn unite(&mut self, addhistory: &History, switcheddirs: bool) {
        for i in 0..=1 {
            let d = if switcheddirs { 1 - i } else { i };
            self.pscostcount[i] += addhistory.pscostcount[d];
            self.ancpscostcount[i] += addhistory.ancpscostcount[d];
            if self.pscostcount[i] > 0.0 {
                let oldmean = self.pscostweightedmean[i];
                self.pscostweightedmean[i] += addhistory.pscostcount[d] * (addhistory.pscostweightedmean[d] - self.pscostweightedmean[i]) / self.pscostcount[i];
                self.pscostvariance[i] = self.pscostvariance[i]
                    + oldmean * oldmean * (self.pscostcount[i] - addhistory.pscostcount[d])
                    + addhistory.pscostvariance[d]
                    + addhistory.pscostcount[d] * addhistory.pscostweightedmean[d] * addhistory.pscostweightedmean[d]
                    - self.pscostcount[i] * self.pscostweightedmean[i] * self.pscostweightedmean[i];
                self.pscostvariance[i] = max(self.pscostvariance[i], 0.0);
            }
            if self.ancpscostcount[i] > 0.0 {
                self.ancpscostweightedmean[i] += addhistory.ancpscostcount[d] * (addhistory.ancpscostweightedmean[d] - self.ancpscostweightedmean[i]) / self.ancpscostcount[i];
            }
            self.vsids[i] += addhistory.vsids[d];
            self.conflengthsum[i] += addhistory.conflengthsum[d];
            self.inferencesum[i] += addhistory.inferencesum[d];
            self.cutoffsum[i] += addhistory.cutoffsum[d];
            self.nactiveconflicts[i] += addhistory.nactiveconflicts[d];
            self.nbranchings[i] += addhistory.nbranchings[d];
            self.branchdepthsum[i] += addhistory.branchdepthsum[d];
        }
    }

    /// `SCIPhistoryUpdatePseudocost`.
    pub fn update_pseudocost(&mut self, set: &Set, solvaldelta: f64, mut objdelta: f64, weight: f64) {
        let dir;
        let mut distance;
        if set.is_positive(solvaldelta) {
            dir = 1;
            distance = solvaldelta;
        } else if set.is_negative(solvaldelta) {
            dir = 0;
            distance = -solvaldelta;
        } else {
            return;
        }
        let eps = set.pseudocosteps();
        distance = max(distance, eps);
        objdelta += set.pseudocostdelta();
        let sumcontribution = objdelta / distance;
        let olddelta = sumcontribution - self.pscostweightedmean[dir];
        self.pscostcount[dir] += weight;
        self.pscostweightedmean[dir] += weight * olddelta / self.pscostcount[dir];
        self.pscostvariance[dir] = self.pscostvariance[dir] + weight * olddelta * (sumcontribution - self.pscostweightedmean[dir]);
    }

    /// `SCIPhistoryUpdateAncPseudocost`.
    pub fn update_anc_pseudocost(&mut self, set: &Set, solvaldelta: f64, mut objdelta: f64, weight: f64) {
        let dir;
        let mut distance;
        if set.is_positive(solvaldelta) {
            dir = 1;
            distance = solvaldelta;
        } else if set.is_negative(solvaldelta) {
            dir = 0;
            distance = -solvaldelta;
        } else {
            return;
        }
        let eps = set.pseudocosteps();
        distance = max(distance, eps);
        objdelta += set.pseudocostdelta();
        let sumcontribution = objdelta / distance;
        let olddelta = sumcontribution - self.ancpscostweightedmean[dir];
        self.ancpscostcount[dir] += weight;
        self.ancpscostweightedmean[dir] += weight * olddelta / self.ancpscostcount[dir];
    }

    /// `SCIPhistoryGetPseudocost`.
    #[inline]
    pub fn get_pseudocost(&self, solvaldelta: f64) -> f64 {
        if solvaldelta >= 0.0 {
            solvaldelta * if self.pscostcount[1] > 0.0 { self.pscostweightedmean[1] } else { 1.0 }
        } else {
            -solvaldelta * if self.pscostcount[0] > 0.0 { self.pscostweightedmean[0] } else { 1.0 }
        }
    }

    /// `SCIPhistoryGetAncPseudocost`.
    #[inline]
    pub fn get_anc_pseudocost(&self, solvaldelta: f64) -> f64 {
        if solvaldelta >= 0.0 {
            solvaldelta * if self.ancpscostcount[1] > 0.0 { self.ancpscostweightedmean[1] } else { 1.0 }
        } else {
            -solvaldelta * if self.ancpscostcount[0] > 0.0 { self.ancpscostweightedmean[0] } else { 1.0 }
        }
    }

    /// `SCIPhistoryGetPseudocostVariance`: the (unbiased) variance of
    /// pseudo costs for a given direction (the function: the reference is
    /// built without `NDEBUG`).
    #[inline]
    pub fn get_pseudocost_variance(&self, direction: BranchDir) -> f64 {
        let dir = if direction == BranchDir::Upwards { 1 } else { 0 };
        let correctionfactor = self.pscostcount[dir] - 1.0;

        if correctionfactor > 0.9 { self.pscostvariance[dir] / correctionfactor } else { 0.0 }
    }

    /// `SCIPhistoryGetPseudocostCount`.
    #[inline]
    pub fn get_pseudocost_count(&self, dir: BranchDir) -> f64 {
        self.pscostcount[dir as usize]
    }

    /// `SCIPhistoryGetAncPseudocostCount`.
    #[inline]
    pub fn get_anc_pseudocost_count(&self, dir: BranchDir) -> f64 {
        self.ancpscostcount[dir as usize]
    }

    /// `SCIPhistoryIsPseudocostEmpty`.
    #[inline]
    pub fn is_pseudocost_empty(&self, dir: BranchDir) -> bool {
        self.pscostcount[dir as usize] == 0.0
    }

    /// `SCIPhistoryIsAncPseudocostEmpty`.
    #[inline]
    pub fn is_anc_pseudocost_empty(&self, dir: BranchDir) -> bool {
        self.ancpscostcount[dir as usize] == 0.0
    }

    /// `SCIPhistoryIncVSIDS`.
    #[inline]
    pub fn inc_vsids(&mut self, dir: BranchDir, weight: f64) {
        self.vsids[dir as usize] += weight;
    }

    /// `SCIPhistoryScaleVSIDS`.
    #[inline]
    pub fn scale_vsids(&mut self, scalar: f64) {
        self.vsids[0] *= scalar;
        self.vsids[1] *= scalar;
    }

    /// `SCIPhistoryGetVSIDS`.
    #[inline]
    pub fn get_vsids(&self, dir: BranchDir) -> f64 {
        self.vsids[dir as usize]
    }

    /// `SCIPhistoryIncNActiveConflicts`.
    #[inline]
    pub fn inc_n_active_conflicts(&mut self, dir: BranchDir, length: f64) {
        self.nactiveconflicts[dir as usize] += 1;
        self.conflengthsum[dir as usize] += length;
    }

    /// `SCIPhistoryGetNActiveConflicts`.
    #[inline]
    pub fn get_n_active_conflicts(&self, dir: BranchDir) -> i64 {
        self.nactiveconflicts[dir as usize]
    }

    /// `SCIPhistoryGetAvgConflictlength`.
    #[inline]
    pub fn get_avg_conflictlength(&self, dir: BranchDir) -> f64 {
        let d = dir as usize;
        if self.conflengthsum[d] > 0.0 {
            self.nactiveconflicts[d] as f64 / self.conflengthsum[d]
        } else {
            0.0
        }
    }

    /// `SCIPhistoryIncNBranchings`.
    #[inline]
    pub fn inc_n_branchings(&mut self, dir: BranchDir, depth: i32) {
        self.nbranchings[dir as usize] += 1;
        self.branchdepthsum[dir as usize] += depth as i64;
    }

    /// `SCIPhistoryIncInferenceSum`.
    #[inline]
    pub fn inc_inference_sum(&mut self, dir: BranchDir, weight: f64) {
        self.inferencesum[dir as usize] += weight;
    }

    /// `SCIPhistoryIncCutoffSum`.
    #[inline]
    pub fn inc_cutoff_sum(&mut self, dir: BranchDir, weight: f64) {
        self.cutoffsum[dir as usize] += weight;
    }

    /// `SCIPhistoryGetNBranchings`.
    #[inline]
    pub fn get_n_branchings(&self, dir: BranchDir) -> i64 {
        self.nbranchings[dir as usize]
    }

    /// `SCIPhistoryGetInferenceSum`.
    #[inline]
    pub fn get_inference_sum(&self, dir: BranchDir) -> f64 {
        self.inferencesum[dir as usize]
    }

    /// `SCIPhistoryGetAvgInferences`.
    #[inline]
    pub fn get_avg_inferences(&self, dir: BranchDir) -> f64 {
        let d = dir as usize;
        if self.nbranchings[d] > 0 {
            self.inferencesum[d] / self.nbranchings[d] as f64
        } else {
            0.0
        }
    }

    /// `SCIPhistoryGetCutoffSum`.
    #[inline]
    pub fn get_cutoff_sum(&self, dir: BranchDir) -> f64 {
        self.cutoffsum[dir as usize]
    }

    /// `SCIPhistoryGetAvgCutoffs`.
    #[inline]
    pub fn get_avg_cutoffs(&self, dir: BranchDir) -> f64 {
        let d = dir as usize;
        if self.nbranchings[d] > 0 { self.cutoffsum[d] / self.nbranchings[d] as f64 } else { 0.0 }
    }

    /// `SCIPhistoryGetAvgBranchdepth`.
    #[inline]
    pub fn get_avg_branchdepth(&self, dir: BranchDir) -> f64 {
        let d = dir as usize;
        if self.nbranchings[d] > 0 {
            self.branchdepthsum[d] as f64 / self.nbranchings[d] as f64
        } else {
            1.0
        }
    }

    /// `SCIPhistoryIsRatioValid`.
    pub fn is_ratio_valid(&self) -> bool {
        self.ratiovalid
    }

    /// `SCIPhistoryGetLastRatio`.
    pub fn get_last_ratio(&self) -> f64 {
        self.ratio
    }

    /// `SCIPhistorySetRatioHistory`.
    pub fn set_ratio_history(&mut self, valid: bool, ratio: f64, balance: f64) {
        self.ratiovalid = valid;
        self.ratio = ratio;
        self.balance = balance;
    }

    /// `SCIPhistoryGetLastBalance`.
    pub fn get_last_balance(&self) -> f64 {
        self.balance
    }

    /// `SCIPhistoryGetLastGMIeff`.
    pub fn get_last_gmieff(&self) -> f64 {
        self.gmieff
    }

    /// `SCIPhistorySetLastGMIeff`.
    pub fn set_last_gmieff(&mut self, gmieff: f64) {
        self.gmieff = gmieff;
    }

    /// `SCIPhistoryGetAvgGMIeff`.
    pub fn get_avg_gmieff(&self) -> f64 {
        if self.ngmi > 0.0 { self.gmieffsum / self.ngmi } else { 0.0 }
    }

    /// `SCIPhistoryIncGMIeffSum`.
    pub fn inc_gmieff_sum(&mut self, gmieff: f64) {
        self.gmieffsum += gmieff;
        self.ngmi += 1.0;
    }
}

/// `SCIP_VALUEHISTORY`: a history per domain value, sorted by value.
#[derive(Clone, Debug, Default)]
pub struct ValueHistory {
    pub histories: Vec<History>,
    pub values: Vec<f64>,
}

impl ValueHistory {
    /// `SCIPvaluehistoryFind`: the history of `value`, created if new.
    pub fn find(&mut self, value: f64) -> &mut History {
        let pos = if self.values.is_empty() {
            Err(0)
        } else {
            sort::sortedvec_find(&self.values, sort::cmp_real, value)
        };
        let pos = match pos {
            Ok(pos) => pos,
            // Where `SCIPsortedvecInsertRealPtr` puts a value not yet there.
            Err(pos) => {
                self.values.insert(pos, value);
                self.histories.insert(pos, History::new());
                pos
            }
        };
        &mut self.histories[pos]
    }

    /// `SCIPvaluehistoryScaleVSIDS`.
    pub fn scale_vsids(&mut self, scalar: f64) {
        for history in self.histories.iter_mut().rev() {
            history.scale_vsids(scalar);
        }
    }

    /// `SCIPvaluehistoryGetNValues`.
    pub fn get_n_values(&self) -> usize {
        self.values.len()
    }
}
