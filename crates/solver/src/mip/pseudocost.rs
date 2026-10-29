//! HiGHS's `HighsPseudocost`: per column, the average objective gain per
//! unit of branching up and down, inference counts, cutoffs and conflict
//! scores, combined into a branching score.

const MIN_THRESHOLD: f64 = 1e-6;

/// HiGHS's `HighsPseudocostDelta`: what changed since the last flush.
#[derive(Clone, Copy, Debug, Default)]
struct PseudocostDelta {
    col: usize,
    nsamplesup: i64,
    nsamplesdown: i64,
    ninferencesup: i64,
    ninferencesdown: i64,
    ncutoffsup: i64,
    ncutoffsdown: i64,
    pseudocostup_sum: f64,
    pseudocostdown_sum: f64,
    inferencesup_sum: f64,
    inferencesdown_sum: f64,
    conflictscoreup_sum: f64,
    conflictscoredown_sum: f64,
}

/// HiGHS's `HighsPseudocostInitialization`: pseudocosts carried into a
/// restart or a sub-MIP.
#[derive(Clone, Debug, Default)]
pub(crate) struct PseudocostInitialization {
    pub pseudocostup: Vec<f64>,
    pub pseudocostdown: Vec<f64>,
    pub nsamplesup: Vec<i64>,
    pub nsamplesdown: Vec<i64>,
    pub inferencesup: Vec<f64>,
    pub inferencesdown: Vec<f64>,
    pub ninferencesup: Vec<i64>,
    pub ninferencesdown: Vec<i64>,
    pub conflictscoreup: Vec<f64>,
    pub conflictscoredown: Vec<f64>,
    pub cost_total: f64,
    pub inferences_total: f64,
    pub conflict_avg_score: f64,
    pub nsamplestotal: i64,
    pub ninferencestotal: i64,
}

impl PseudocostInitialization {
    /// From `pscost`, sample counts capped at `max_count`, over the same
    /// columns.
    pub fn new(pscost: &Pseudocost, max_count: i64) -> Self {
        let ncol = pscost.pseudocostup.len();
        let mut init = Self {
            pseudocostup: pscost.pseudocostup.clone(),
            pseudocostdown: pscost.pseudocostdown.clone(),
            nsamplesup: pscost.nsamplesup.clone(),
            nsamplesdown: pscost.nsamplesdown.clone(),
            inferencesup: pscost.inferencesup.clone(),
            inferencesdown: pscost.inferencesdown.clone(),
            ninferencesup: pscost.ninferencesup.clone(),
            ninferencesdown: pscost.ninferencesdown.clone(),
            conflictscoreup: vec![0.0; ncol],
            conflictscoredown: vec![0.0; ncol],
            cost_total: pscost.cost_total,
            inferences_total: pscost.inferences_total,
            conflict_avg_score: pscost.conflict_avg_score,
            nsamplestotal: pscost.nsamplestotal.min(1),
            ninferencestotal: pscost.ninferencestotal.min(1),
        };
        init.conflict_avg_score /= ncol as f64 * pscost.conflict_weight;
        for i in 0..ncol {
            init.nsamplesup[i] = init.nsamplesup[i].min(max_count);
            init.nsamplesdown[i] = init.nsamplesdown[i].min(max_count);
            init.ninferencesup[i] = init.ninferencesup[i].min(1);
            init.ninferencesdown[i] = init.ninferencesdown[i].min(1);
            init.conflictscoreup[i] = pscost.conflictscoreup[i] / pscost.conflict_weight;
            init.conflictscoredown[i] = pscost.conflictscoredown[i] / pscost.conflict_weight;
        }
        init
    }

    /// From `pscost` over presolved columns, mapped back to the original
    /// columns by `orig_col_index` (the postsolve stack's map).
    pub fn new_original(pscost: &Pseudocost, max_count: i64, orig_num_col: usize, orig_col_index: impl Fn(usize) -> usize) -> Self {
        let ncols = pscost.pseudocostup.len();
        let mut init = Self {
            pseudocostup: vec![0.0; orig_num_col],
            pseudocostdown: vec![0.0; orig_num_col],
            nsamplesup: vec![0; orig_num_col],
            nsamplesdown: vec![0; orig_num_col],
            inferencesup: vec![0.0; orig_num_col],
            inferencesdown: vec![0.0; orig_num_col],
            ninferencesup: vec![0; orig_num_col],
            ninferencesdown: vec![0; orig_num_col],
            conflictscoreup: vec![0.0; orig_num_col],
            conflictscoredown: vec![0.0; orig_num_col],
            cost_total: pscost.cost_total,
            inferences_total: pscost.inferences_total,
            conflict_avg_score: pscost.conflict_avg_score,
            nsamplestotal: pscost.nsamplestotal.min(1),
            ninferencestotal: pscost.ninferencestotal.min(1),
        };
        init.conflict_avg_score /= ncols as f64 * pscost.conflict_weight;
        for i in 0..ncols {
            let orig = orig_col_index(i);
            init.pseudocostup[orig] = pscost.pseudocostup[i];
            init.pseudocostdown[orig] = pscost.pseudocostdown[i];
            init.nsamplesup[orig] = max_count.min(pscost.nsamplesup[i]);
            init.nsamplesdown[orig] = max_count.min(pscost.nsamplesdown[i]);
            init.inferencesup[orig] = pscost.inferencesup[i];
            init.inferencesdown[orig] = pscost.inferencesdown[i];
            init.ninferencesup[orig] = 1;
            init.ninferencesdown[orig] = 1;
            init.conflictscoreup[orig] = pscost.conflictscoreup[i] / pscost.conflict_weight;
            init.conflictscoredown[orig] = pscost.conflictscoredown[i] / pscost.conflict_weight;
        }
        init
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Pseudocost {
    pseudocostup: Vec<f64>,
    pseudocostdown: Vec<f64>,
    nsamplesup: Vec<i64>,
    nsamplesdown: Vec<i64>,
    inferencesup: Vec<f64>,
    inferencesdown: Vec<f64>,
    ninferencesup: Vec<i64>,
    ninferencesdown: Vec<i64>,
    ncutoffsup: Vec<i64>,
    ncutoffsdown: Vec<i64>,
    conflictscoreup: Vec<f64>,
    conflictscoredown: Vec<f64>,
    changedpos: Vec<Option<usize>>,
    deltas: Vec<PseudocostDelta>,
    conflict_weight: f64,
    conflict_avg_score: f64,
    cost_total: f64,
    inferences_total: f64,
    delta_cost_sum: f64,
    delta_inferences_sum: f64,
    nsamplestotal: i64,
    ninferencestotal: i64,
    ncutoffstotal: i64,
    delta_nsamplestotal: i64,
    delta_ninferencestotal: i64,
    minreliable: i64,
    degeneracy_factor: f64,
}

fn add_delta_average(avg: &mut f64, count: &mut i64, delta_sum: f64, delta_count: i64) {
    if delta_count <= 0 {
        return;
    }
    *avg = (*avg * *count as f64 + delta_sum) / (*count + delta_count) as f64;
    *count += delta_count;
}

fn map_score(score: f64) -> f64 {
    1.0 - 1.0 / (1.0 + score)
}

impl Pseudocost {
    /// HiGHS's `HighsPseudocost(mipsolver)`: empty, or from `init` with
    /// `orig_col_index` mapping each column to the one `init` describes.
    pub fn new(num_col: usize, minreliable: i64, init: Option<(&PseudocostInitialization, &dyn Fn(usize) -> usize)>) -> Self {
        let mut pscost = Self {
            pseudocostup: vec![0.0; num_col],
            pseudocostdown: vec![0.0; num_col],
            nsamplesup: vec![0; num_col],
            nsamplesdown: vec![0; num_col],
            inferencesup: vec![0.0; num_col],
            inferencesdown: vec![0.0; num_col],
            ninferencesup: vec![0; num_col],
            ninferencesdown: vec![0; num_col],
            ncutoffsup: vec![0; num_col],
            ncutoffsdown: vec![0; num_col],
            conflictscoreup: vec![0.0; num_col],
            conflictscoredown: vec![0.0; num_col],
            changedpos: vec![None; num_col],
            deltas: Vec::with_capacity(num_col.min(256)),
            conflict_weight: 1.0,
            conflict_avg_score: 0.0,
            cost_total: 0.0,
            inferences_total: 0.0,
            delta_cost_sum: 0.0,
            delta_inferences_sum: 0.0,
            nsamplestotal: 0,
            ninferencestotal: 0,
            ncutoffstotal: 0,
            delta_nsamplestotal: 0,
            delta_ninferencestotal: 0,
            minreliable,
            degeneracy_factor: 1.0,
        };
        if let Some((init, orig_col_index)) = init {
            pscost.cost_total = init.cost_total;
            pscost.inferences_total = init.inferences_total;
            pscost.nsamplestotal = init.nsamplestotal;
            pscost.ninferencestotal = init.ninferencestotal;
            pscost.conflict_avg_score = init.conflict_avg_score * num_col as f64;
            for i in 0..num_col {
                let orig = orig_col_index(i);
                pscost.pseudocostup[i] = init.pseudocostup[orig];
                pscost.nsamplesup[i] = init.nsamplesup[orig];
                pscost.pseudocostdown[i] = init.pseudocostdown[orig];
                pscost.nsamplesdown[i] = init.nsamplesdown[orig];
                pscost.inferencesup[i] = init.inferencesup[orig];
                pscost.ninferencesup[i] = init.ninferencesup[orig];
                pscost.inferencesdown[i] = init.inferencesdown[orig];
                pscost.ninferencesdown[i] = init.ninferencesdown[orig];
                pscost.conflictscoreup[i] = init.conflictscoreup[orig];
                pscost.conflictscoredown[i] = init.conflictscoredown[orig];
            }
        }
        pscost
    }

    fn mark_changed(&mut self, col: usize) -> &mut PseudocostDelta {
        let pos = match self.changedpos[col] {
            Some(pos) => pos,
            None => {
                let pos = self.deltas.len();
                self.changedpos[col] = Some(pos);
                self.deltas.push(PseudocostDelta {
                    col,
                    ..PseudocostDelta::default()
                });
                pos
            }
        };
        &mut self.deltas[pos]
    }

    pub fn increase_conflict_weight(&mut self) {
        self.conflict_weight *= 1.02;
        if self.conflict_weight > 1000.0 {
            let scale = 1.0 / self.conflict_weight;
            self.conflict_weight = 1.0;
            self.conflict_avg_score *= scale;
            for i in 0..self.conflictscoreup.len() {
                self.conflictscoreup[i] *= scale;
                self.conflictscoredown[i] *= scale;
            }
            for delta in &mut self.deltas {
                delta.conflictscoreup_sum *= scale;
                delta.conflictscoredown_sum *= scale;
            }
        }
    }

    pub fn set_degeneracy_factor(&mut self, degeneracy_factor: f64) {
        self.degeneracy_factor = degeneracy_factor;
    }

    pub fn increase_conflict_score_up(&mut self, col: usize) {
        let weight = self.conflict_weight;
        self.mark_changed(col).conflictscoreup_sum += weight;
        self.conflictscoreup[col] += weight;
        self.conflict_avg_score += weight;
    }

    pub fn increase_conflict_score_down(&mut self, col: usize) {
        let weight = self.conflict_weight;
        self.mark_changed(col).conflictscoredown_sum += weight;
        self.conflictscoredown[col] += weight;
        self.conflict_avg_score += weight;
    }

    pub fn set_min_reliable(&mut self, minreliable: i64) {
        self.minreliable = minreliable;
    }

    pub fn min_reliable(&self) -> i64 {
        self.minreliable
    }

    pub fn num_observations(&self, col: usize) -> i64 {
        self.nsamplesup[col] + self.nsamplesdown[col]
    }

    pub fn num_observations_up(&self, col: usize) -> i64 {
        self.nsamplesup[col]
    }

    pub fn num_observations_down(&self, col: usize) -> i64 {
        self.nsamplesdown[col]
    }

    pub fn add_cutoff_observation(&mut self, col: usize, upbranch: bool) {
        self.ncutoffstotal += 1;
        if upbranch {
            self.ncutoffsup[col] += 1;
            self.mark_changed(col).ncutoffsup += 1;
        } else {
            self.ncutoffsdown[col] += 1;
            self.mark_changed(col).ncutoffsdown += 1;
        }
    }

    pub fn add_observation(&mut self, col: usize, delta: f64, objdelta: f64) {
        let ps_delta_pos = {
            self.mark_changed(col);
            self.changedpos[col].expect("marked")
        };
        let unit_gain;
        if delta > 0.0 {
            unit_gain = objdelta / delta;
            let d = unit_gain - self.pseudocostup[col];
            self.nsamplesup[col] += 1;
            self.pseudocostup[col] += d / self.nsamplesup[col] as f64;
            self.deltas[ps_delta_pos].nsamplesup += 1;
            self.deltas[ps_delta_pos].pseudocostup_sum += unit_gain;
        } else {
            unit_gain = -objdelta / delta;
            let d = unit_gain - self.pseudocostdown[col];
            self.nsamplesdown[col] += 1;
            self.pseudocostdown[col] += d / self.nsamplesdown[col] as f64;
            self.deltas[ps_delta_pos].nsamplesdown += 1;
            self.deltas[ps_delta_pos].pseudocostdown_sum += unit_gain;
        }
        let d = unit_gain - self.cost_total;
        self.nsamplestotal += 1;
        self.cost_total += d / self.nsamplestotal as f64;
        self.delta_nsamplestotal += 1;
        self.delta_cost_sum += unit_gain;
    }

    pub fn add_inference_observation(&mut self, col: usize, ninferences: i64, upbranch: bool) {
        let ninf = ninferences as f64;
        let d = ninf - self.inferences_total;
        self.ninferencestotal += 1;
        self.inferences_total += d / self.ninferencestotal as f64;
        self.delta_ninferencestotal += 1;
        self.delta_inferences_sum += ninf;
        if upbranch {
            let d = ninf - self.inferencesup[col];
            self.ninferencesup[col] += 1;
            self.inferencesup[col] += d / self.ninferencesup[col] as f64;
            let delta = self.mark_changed(col);
            delta.ninferencesup += 1;
            delta.inferencesup_sum += ninf;
        } else {
            let d = ninf - self.inferencesdown[col];
            self.ninferencesdown[col] += 1;
            self.inferencesdown[col] += d / self.ninferencesdown[col] as f64;
            let delta = self.mark_changed(col);
            delta.ninferencesdown += 1;
            delta.inferencesdown_sum += ninf;
        }
    }

    pub fn is_reliable(&self, col: usize) -> bool {
        self.nsamplesup[col].min(self.nsamplesdown[col]) >= self.minreliable
    }

    pub fn is_reliable_up(&self, col: usize) -> bool {
        self.nsamplesup[col] >= self.minreliable
    }

    pub fn is_reliable_down(&self, col: usize) -> bool {
        self.nsamplesdown[col] >= self.minreliable
    }

    pub fn avg_pseudocost(&self) -> f64 {
        self.cost_total
    }

    /// HiGHS's `getPseudocostUp(col, frac, offset)`.
    pub fn pseudocost_up_offset(&self, col: usize, frac: f64, offset: f64) -> f64 {
        let up = frac.ceil() - frac;
        let cost = if self.nsamplesup[col] == 0 || self.nsamplesup[col] < self.minreliable {
            let weight_ps = if self.nsamplesup[col] == 0 {
                0.0
            } else {
                0.9 + 0.1 * self.nsamplesup[col] as f64 / self.minreliable as f64
            };
            weight_ps * self.pseudocostup[col] + (1.0 - weight_ps) * self.avg_pseudocost()
        } else {
            self.pseudocostup[col]
        };
        up * (offset + cost)
    }

    /// HiGHS's `getPseudocostDown(col, frac, offset)`.
    pub fn pseudocost_down_offset(&self, col: usize, frac: f64, offset: f64) -> f64 {
        let down = frac - frac.floor();
        let cost = if self.nsamplesdown[col] == 0 || self.nsamplesdown[col] < self.minreliable {
            let weight_ps = if self.nsamplesdown[col] == 0 {
                0.0
            } else {
                0.9 + 0.1 * self.nsamplesdown[col] as f64 / self.minreliable as f64
            };
            weight_ps * self.pseudocostdown[col] + (1.0 - weight_ps) * self.avg_pseudocost()
        } else {
            self.pseudocostdown[col]
        };
        down * (offset + cost)
    }

    pub fn pseudocost_up(&self, col: usize, frac: f64) -> f64 {
        let up = frac.ceil() - frac;
        if self.nsamplesup[col] == 0 {
            return up * self.cost_total;
        }
        up * self.pseudocostup[col]
    }

    pub fn pseudocost_down(&self, col: usize, frac: f64) -> f64 {
        let down = frac - frac.floor();
        if self.nsamplesdown[col] == 0 {
            return down * self.cost_total;
        }
        down * self.pseudocostdown[col]
    }

    pub fn conflict_score_up(&self, col: usize) -> f64 {
        self.conflictscoreup[col] / self.conflict_weight
    }

    pub fn conflict_score_down(&self, col: usize) -> f64 {
        self.conflictscoredown[col] / self.conflict_weight
    }

    fn avg_cutoffs(&self) -> f64 {
        self.ncutoffstotal as f64 / (1.0f64).max(self.ncutoffstotal as f64 + self.nsamplestotal as f64)
    }

    /// HiGHS's `getScore(col, upcost, downcost)`.
    pub fn score_with_costs(&self, col: usize, upcost: f64, downcost: f64) -> f64 {
        let cmax = super::cmax;
        let cost_score = cmax(upcost, MIN_THRESHOLD) * cmax(downcost, MIN_THRESHOLD) / cmax(MIN_THRESHOLD, self.cost_total * self.cost_total);
        let inference_score =
            cmax(self.inferencesup[col], MIN_THRESHOLD) * cmax(self.inferencesdown[col], MIN_THRESHOLD) / cmax(MIN_THRESHOLD, self.inferences_total * self.inferences_total);
        let cut_off_score_up = self.ncutoffsup[col] as f64 / cmax(1.0, self.ncutoffsup[col] as f64 + self.nsamplesup[col] as f64);
        let cut_off_score_down = self.ncutoffsdown[col] as f64 / cmax(1.0, self.ncutoffsdown[col] as f64 + self.nsamplesdown[col] as f64);
        let avg_cutoffs = self.avg_cutoffs();
        let cutoff_score = cmax(cut_off_score_up, MIN_THRESHOLD) * cmax(cut_off_score_down, MIN_THRESHOLD) / cmax(MIN_THRESHOLD, avg_cutoffs * avg_cutoffs);
        let conflict_score_up = self.conflictscoreup[col] / self.conflict_weight;
        let conflict_score_down = self.conflictscoredown[col] / self.conflict_weight;
        let conflict_score_avg = self.conflict_avg_score / (self.conflict_weight * self.conflictscoreup.len() as f64);
        let conflict_score = cmax(conflict_score_up, MIN_THRESHOLD) * cmax(conflict_score_down, MIN_THRESHOLD) / cmax(MIN_THRESHOLD, conflict_score_avg * conflict_score_avg);
        map_score(cost_score) / self.degeneracy_factor + self.degeneracy_factor * (1e-2 * map_score(conflict_score) + 1e-4 * (map_score(cutoff_score) + map_score(inference_score)))
    }

    /// HiGHS's `getScore(col, frac)`.
    pub fn score(&self, col: usize, frac: f64) -> f64 {
        self.score_with_costs(col, self.pseudocost_up(col, frac), self.pseudocost_down(col, frac))
    }

    pub fn score_up(&self, col: usize, frac: f64) -> f64 {
        let cmax = super::cmax;
        let cost_score = self.pseudocost_up(col, frac) / cmax(MIN_THRESHOLD, self.cost_total);
        let inference_score = self.inferencesup[col] / cmax(MIN_THRESHOLD, self.inferences_total);
        let cut_off_score_up = self.ncutoffsup[col] as f64 / cmax(1.0, self.ncutoffsup[col] as f64 + self.nsamplesup[col] as f64);
        let cutoff_score = cut_off_score_up / cmax(MIN_THRESHOLD, self.avg_cutoffs());
        let conflict_score_up = self.conflictscoreup[col] / self.conflict_weight;
        let conflict_score_avg = self.conflict_avg_score / (self.conflict_weight * self.conflictscoreup.len() as f64);
        let conflict_score = conflict_score_up / cmax(MIN_THRESHOLD, conflict_score_avg);
        map_score(cost_score) + (1e-2 * map_score(conflict_score) + 1e-4 * (map_score(cutoff_score) + map_score(inference_score)))
    }

    pub fn score_down(&self, col: usize, frac: f64) -> f64 {
        let cmax = super::cmax;
        let cost_score = self.pseudocost_down(col, frac) / cmax(MIN_THRESHOLD, self.cost_total);
        let inference_score = self.inferencesdown[col] / cmax(MIN_THRESHOLD, self.inferences_total);
        let cut_off_score_down = self.ncutoffsdown[col] as f64 / cmax(1.0, self.ncutoffsdown[col] as f64 + self.nsamplesdown[col] as f64);
        let cutoff_score = cut_off_score_down / cmax(MIN_THRESHOLD, self.avg_cutoffs());
        let conflict_score_down = self.conflictscoredown[col] / self.conflict_weight;
        let conflict_score_avg = self.conflict_avg_score / (self.conflict_weight * self.conflictscoredown.len() as f64);
        let conflict_score = conflict_score_down / cmax(MIN_THRESHOLD, conflict_score_avg);
        map_score(cost_score) + (1e-2 * map_score(conflict_score) + 1e-4 * (map_score(cutoff_score) + map_score(inference_score)))
    }

    pub fn avg_inferences_up(&self, col: usize) -> f64 {
        self.inferencesup[col]
    }

    pub fn avg_inferences_down(&self, col: usize) -> f64 {
        self.inferencesdown[col]
    }

    /// HiGHS's `flushPseudoCost`: add `other`'s changes since its last flush
    /// into this one.
    pub fn flush(&mut self, other: &mut Pseudocost) {
        for delta in &other.deltas {
            let col = delta.col;
            add_delta_average(&mut self.pseudocostup[col], &mut self.nsamplesup[col], delta.pseudocostup_sum, delta.nsamplesup);
            add_delta_average(&mut self.pseudocostdown[col], &mut self.nsamplesdown[col], delta.pseudocostdown_sum, delta.nsamplesdown);
            add_delta_average(&mut self.inferencesup[col], &mut self.ninferencesup[col], delta.inferencesup_sum, delta.ninferencesup);
            add_delta_average(
                &mut self.inferencesdown[col],
                &mut self.ninferencesdown[col],
                delta.inferencesdown_sum,
                delta.ninferencesdown,
            );
            let conflict_scale = self.conflict_weight / other.conflict_weight;
            let conflict_delta_up = conflict_scale * delta.conflictscoreup_sum;
            let conflict_delta_down = conflict_scale * delta.conflictscoredown_sum;
            self.conflictscoreup[col] += conflict_delta_up;
            self.conflictscoredown[col] += conflict_delta_down;
            self.conflict_avg_score += conflict_delta_up + conflict_delta_down;
            self.ncutoffsup[col] += delta.ncutoffsup;
            self.ncutoffsdown[col] += delta.ncutoffsdown;
            self.ncutoffstotal += delta.ncutoffsup + delta.ncutoffsdown;
        }
        add_delta_average(&mut self.cost_total, &mut self.nsamplestotal, other.delta_cost_sum, other.delta_nsamplestotal);
        add_delta_average(
            &mut self.inferences_total,
            &mut self.ninferencestotal,
            other.delta_inferences_sum,
            other.delta_ninferencestotal,
        );
        other.remove_changed();
    }

    /// HiGHS's `syncPseudoCost`: copy this one into `other`.
    pub fn sync_into(&self, other: &mut Pseudocost) {
        other.pseudocostup.copy_from_slice(&self.pseudocostup);
        other.pseudocostdown.copy_from_slice(&self.pseudocostdown);
        other.nsamplesup.copy_from_slice(&self.nsamplesup);
        other.nsamplesdown.copy_from_slice(&self.nsamplesdown);
        other.inferencesup.copy_from_slice(&self.inferencesup);
        other.inferencesdown.copy_from_slice(&self.inferencesdown);
        other.ninferencesup.copy_from_slice(&self.ninferencesup);
        other.ninferencesdown.copy_from_slice(&self.ninferencesdown);
        other.ncutoffsup.copy_from_slice(&self.ncutoffsup);
        other.ncutoffsdown.copy_from_slice(&self.ncutoffsdown);
        other.conflictscoreup.copy_from_slice(&self.conflictscoreup);
        other.conflictscoredown.copy_from_slice(&self.conflictscoredown);
        other.conflict_weight = self.conflict_weight;
        other.conflict_avg_score = self.conflict_avg_score;
        other.cost_total = self.cost_total;
        other.inferences_total = self.inferences_total;
        other.nsamplestotal = self.nsamplestotal;
        other.ninferencestotal = self.ninferencestotal;
        other.ncutoffstotal = self.ncutoffstotal;
        other.remove_changed();
    }

    pub fn remove_changed(&mut self) {
        for delta in &self.deltas {
            self.changedpos[delta.col] = None;
        }
        self.deltas.clear();
        self.delta_cost_sum = 0.0;
        self.delta_inferences_sum = 0.0;
        self.delta_nsamplestotal = 0;
        self.delta_ninferencestotal = 0;
    }
}
