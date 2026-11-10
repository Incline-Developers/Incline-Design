//! HiGHS's `HighsLpAggregator`, `HighsSeparator` and the path aggregation
//! separator `HighsPathSeparator`.

use super::{cut_generation::CutGeneration, data::MipData, lp_relaxation::LpRelaxation, sparse_vector_sum::SparseVectorSum, transformed_lp::TransformedLp};
use crate::{
    hash_table::HashTable,
    util::{CDouble, Random},
};

const INF: f64 = f64::INFINITY;
const TINY: f64 = super::TINY;

/// HiGHS's `HighsLpAggregator`: a weighted sum of LP rows with their slack
/// columns (`num_cols + row`), so always an equation with right hand side 0.
pub(crate) struct LpAggregator {
    vectorsum: SparseVectorSum,
}

impl LpAggregator {
    pub fn new(lp: &LpRelaxation) -> Self {
        Self {
            vectorsum: SparseVectorSum::new(lp.num_rows() + lp.num_cols()),
        }
    }

    /// HiGHS's `addRow`.
    pub fn add_row(&mut self, lp: &LpRelaxation, data: &MipData, row: usize, weight: f64) {
        let (inds, vals) = lp.row(row, data);
        for i in 0..inds.len() {
            self.vectorsum.add(inds[i], weight * vals[i]);
        }
        self.vectorsum.add(lp.num_cols() + row, -weight);
    }

    /// HiGHS's `getCurrentAggregation`.
    pub fn current_aggregation(&mut self, lp: &LpRelaxation, data: &MipData, inds: &mut Vec<usize>, vals: &mut Vec<f64>, negate: bool) {
        let droptol = data.options.small_matrix_value;
        let num_col = lp.num_cols();
        self.vectorsum.cleanup(|col, val| col < num_col && val.abs() <= droptol);
        inds.clone_from(&self.vectorsum.nonzeroinds);
        vals.clear();
        if negate {
            vals.extend(inds.iter().map(|&i| -self.vectorsum.value(i)));
        } else {
            vals.extend(inds.iter().map(|&i| self.vectorsum.value(i)));
        }
    }

    pub fn clear(&mut self) {
        self.vectorsum.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.vectorsum.nonzeroinds.is_empty()
    }
}

/// HiGHS's `HighsSeparator` counters.
#[derive(Clone, Debug, Default)]
pub(crate) struct SeparatorStats {
    pub num_cuts_found: usize,
    pub num_calls: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowType {
    Unusable,
    Geq,
    Eq,
    Leq,
}

/// HiGHS's `HighsPathSeparator`.
pub(crate) struct PathSeparator {
    randgen: Random,
    pub stats: SeparatorStats,
}

impl PathSeparator {
    pub fn new(data: &MipData) -> Self {
        Self {
            randgen: Random::new(data.options.random_seed),
            stats: SeparatorStats::default(),
        }
    }

    /// HiGHS's `HighsSeparator::run`.
    pub fn run(&mut self, lp: &LpRelaxation, aggregator: &mut LpAggregator, trans_lp: &mut TransformedLp, data: &mut MipData) {
        self.stats.num_calls += 1;
        let before = data.cutpool.num_cuts();
        self.separate_lp_solution(lp, aggregator, trans_lp, data);
        self.stats.num_cuts_found += data.cutpool.num_cuts() - before;
    }

    /// HiGHS's `separateLpSolution`.
    fn separate_lp_solution(&mut self, lp: &LpRelaxation, aggregator: &mut LpAggregator, trans_lp: &mut TransformedLp, data: &mut MipData) {
        let solver = lp.lp_solver();
        let num_row = solver.num_rows();
        let num_col = solver.num_cols();
        let row_lower = solver.row_lower().to_vec();
        let row_upper = solver.row_upper().to_vec();
        let row_value = solver.row_value().expect("an LP solution").to_vec();
        let row_dual = solver.row_dual().expect("LP duals").to_vec();
        let (a_start, a_index, a_value) = solver.matrix();
        let (a_start, a_index, a_value) = (a_start.to_vec(), a_index.to_vec(), a_value.to_vec());
        let feastol = data.feastol;

        let mut rowtype = vec![RowType::Unusable; num_row];
        for i in 0..num_row {
            if row_lower[i] == row_upper[i] {
                rowtype[i] = RowType::Eq;
                continue;
            }
            let mut lowerslack = INF;
            let mut upperslack = INF;
            if row_lower[i] != -INF {
                lowerslack = row_value[i] - row_lower[i];
            }
            if row_upper[i] != INF {
                upperslack = row_upper[i] - row_value[i];
            }
            rowtype[i] = if lowerslack > feastol && upperslack > feastol {
                RowType::Unusable
            } else if lowerslack < upperslack {
                RowType::Geq
            } else {
                RowType::Leq
            };
        }

        let mut num_continuous = vec![0usize; num_row];
        let mut max_aggr_row_size = 0;
        for &col in &data.continuous_cols {
            if trans_lp.bound_distance(col) == 0.0 {
                continue;
            }
            max_aggr_row_size += a_start[col + 1] - a_start[col];
            for i in a_start[col]..a_start[col + 1] {
                num_continuous[a_index[i]] += 1;
            }
        }

        let mut col_substitutions: Vec<(Option<usize>, f64)> = vec![(None, 0.0); num_col];
        for i in 0..num_row {
            if rowtype[i] != RowType::Eq || num_continuous[i] != 1 {
                continue;
            }
            let (rowinds, rowvals) = lp.row(i, data);
            let mut found = None;
            for j in 0..rowinds.len() {
                if data.model.is_col_integral(rowinds[j]) {
                    continue;
                }
                if trans_lp.bound_distance(rowinds[j]) == 0.0 {
                    continue;
                }
                found = Some((rowinds[j], rowvals[j]));
                break;
            }
            let (col, val) = found.expect("one continuous column with distance from its bounds");
            if col_substitutions[col].0.is_some() {
                continue;
            }
            col_substitutions[col] = (Some(i), val);
            rowtype[i] = RowType::Unusable;
        }

        let mut in_arc_rows: Vec<(usize, f64)> = Vec::with_capacity(max_aggr_row_size);
        let mut col_in_arcs = vec![(0usize, 0usize); num_col];
        let mut out_arc_rows: Vec<(usize, f64)> = Vec::with_capacity(max_aggr_row_size);
        let mut col_out_arcs = vec![(0usize, 0usize); num_col];
        for &col in &data.continuous_cols {
            if trans_lp.bound_distance(col) == 0.0 || col_substitutions[col].0.is_some() {
                continue;
            }
            col_in_arcs[col].0 = in_arc_rows.len();
            col_out_arcs[col].0 = out_arc_rows.len();
            for i in a_start[col]..a_start[col + 1] {
                let (row, value) = (a_index[i], a_value[i]);
                match rowtype[row] {
                    RowType::Unusable => continue,
                    RowType::Leq => {
                        if value < 0.0 {
                            in_arc_rows.push((row, value));
                        } else {
                            out_arc_rows.push((row, value));
                        }
                    }
                    RowType::Geq => {
                        if value > 0.0 {
                            in_arc_rows.push((row, value));
                        } else {
                            out_arc_rows.push((row, value));
                        }
                    }
                    RowType::Eq => {
                        in_arc_rows.push((row, value));
                        out_arc_rows.push((row, value));
                    }
                }
            }
            col_in_arcs[col].1 = in_arc_rows.len();
            col_out_arcs[col].1 = out_arc_rows.len();
        }

        let mut cut_gen = CutGeneration::new(lp, data);
        let mut base_row_inds: Vec<usize> = Vec::new();
        let mut base_row_vals: Vec<f64> = Vec::new();
        const MAX_PATH_LEN: usize = 6;
        let mut current_path = [0usize; MAX_PATH_LEN];
        let mut aggregated_path: Vec<(Vec<usize>, Vec<f64>)> = Vec::new();
        let epsilon = data.epsilon;
        for i in 0..num_row {
            let scales = match rowtype[i] {
                RowType::Unusable => continue,
                RowType::Eq => {
                    if row_dual[i] <= epsilon {
                        [1.0, -1.0]
                    } else {
                        [-1.0, 1.0]
                    }
                }
                RowType::Leq => [1.0, -1.0],
                RowType::Geq => [-1.0, 1.0],
            };
            let mut success = false;
            for &scale in &scales {
                aggregator.add_row(lp, data, i, scale);
                current_path[0] = i;
                let mut curr_path_len = 1;
                let mut try_negated_scale = false;
                let max_weight = 1.0 / feastol;
                let min_weight = feastol;
                let check_weight = |w: f64| {
                    let w = w.abs();
                    w <= max_weight && w >= min_weight
                };
                aggregated_path.clear();

                while curr_path_len != MAX_PATH_LEN {
                    let is_row_in_current_path = |row: usize, len: usize| current_path[..len].contains(&row);
                    aggregator.current_aggregation(lp, data, &mut base_row_inds, &mut base_row_vals, false);
                    let base_row_len = base_row_inds.len();
                    let mut added_substitution_rows = false;
                    let mut best_out_arc_col: Option<usize> = None;
                    let mut out_arc_col_val = 0.0;
                    let mut out_arc_col_bound_dist = 0.0;
                    let mut best_in_arc_col: Option<usize> = None;
                    let mut in_arc_col_val = 0.0;
                    let mut in_arc_col_bound_dist = 0.0;

                    let skip_col = |col: usize,
                                    col_arcs: &[(usize, usize)],
                                    arc_rows: &[(usize, f64)],
                                    other_col_arcs: &[(usize, usize)],
                                    other_arc_rows: &[(usize, f64)],
                                    try_negated_scale: &mut bool| {
                        if curr_path_len == 1 && !*try_negated_scale {
                            if col_arcs[col].1 - col_arcs[col].0 <= curr_path_len {
                                for arc_row in &arc_rows[col_arcs[col].0..col_arcs[col].1] {
                                    if arc_row.0 != i {
                                        *try_negated_scale = true;
                                        break;
                                    }
                                }
                            } else {
                                *try_negated_scale = true;
                            }
                        }
                        if other_col_arcs[col].0 == other_col_arcs[col].1 {
                            return true;
                        }
                        if other_col_arcs[col].1 - other_col_arcs[col].0 <= curr_path_len {
                            for other_arc_row in &other_arc_rows[other_col_arcs[col].0..other_col_arcs[col].1] {
                                if !is_row_in_current_path(other_arc_row.0, curr_path_len) {
                                    return false;
                                }
                            }
                            return true;
                        }
                        false
                    };

                    for j in 0..base_row_len {
                        let col = base_row_inds[j];
                        if col >= num_col || trans_lp.bound_distance(col) == 0.0 || lp.is_col_integral(col, data) {
                            continue;
                        }
                        if let (Some(row), val) = col_substitutions[col] {
                            added_substitution_rows = true;
                            aggregator.add_row(lp, data, row, -base_row_vals[j] / val);
                            continue;
                        }
                        if added_substitution_rows {
                            continue;
                        }
                        if base_row_vals[j] < 0.0 {
                            if skip_col(col, &col_out_arcs, &out_arc_rows, &col_in_arcs, &in_arc_rows, &mut try_negated_scale) {
                                continue;
                            }
                            if best_out_arc_col.is_none() || trans_lp.bound_distance(col) > out_arc_col_bound_dist {
                                best_out_arc_col = Some(col);
                                out_arc_col_val = base_row_vals[j];
                                out_arc_col_bound_dist = trans_lp.bound_distance(col);
                            }
                        } else {
                            if skip_col(col, &col_in_arcs, &in_arc_rows, &col_out_arcs, &out_arc_rows, &mut try_negated_scale) {
                                continue;
                            }
                            if best_in_arc_col.is_none() || trans_lp.bound_distance(col) > in_arc_col_bound_dist {
                                best_in_arc_col = Some(col);
                                in_arc_col_val = base_row_vals[j];
                                in_arc_col_bound_dist = trans_lp.bound_distance(col);
                            }
                        }
                    }
                    if added_substitution_rows {
                        continue;
                    }
                    let mut rhs = 0.0;
                    success = cut_gen.generate_cut(trans_lp, lp, data, &mut base_row_inds, &mut base_row_vals, &mut rhs, false);
                    aggregator.current_aggregation(lp, data, &mut base_row_inds, &mut base_row_vals, true);
                    if !aggregated_path.is_empty() || best_out_arc_col.is_some() || best_in_arc_col.is_some() {
                        aggregated_path.push((base_row_inds.clone(), base_row_vals.clone()));
                    }
                    rhs = 0.0;
                    success |= cut_gen.generate_cut(trans_lp, lp, data, &mut base_row_inds, &mut base_row_vals, &mut rhs, false);
                    if success || (best_out_arc_col.is_none() && best_in_arc_col.is_none()) {
                        break;
                    }
                    let randgen = &mut self.randgen;
                    let mut find_row = |best_arc_col: usize, val: f64, col_arcs: &[(usize, usize)], arc_rows: &[(usize, f64)]| -> Option<(usize, f64)> {
                        let (first, end) = col_arcs[best_arc_col];
                        let arc_row = first + randgen.integer(end - first);
                        let candidate = |k: usize| {
                            let r = arc_rows[k].0;
                            let w = -val / arc_rows[k].1;
                            (!is_row_in_current_path(r, curr_path_len) && check_weight(w)).then_some((r, w))
                        };
                        if let Some(found) = candidate(arc_row) {
                            return Some(found);
                        }
                        for next_row in arc_row + 1..end {
                            if let Some(found) = candidate(next_row) {
                                return Some(found);
                            }
                        }
                        for next_row in first..arc_row {
                            if let Some(found) = candidate(next_row) {
                                return Some(found);
                            }
                        }
                        None
                    };
                    let found = if best_in_arc_col.is_none() || (best_out_arc_col.is_some() && out_arc_col_bound_dist >= in_arc_col_bound_dist - feastol) {
                        match find_row(best_out_arc_col.expect("an out arc column"), out_arc_col_val, &col_in_arcs, &in_arc_rows) {
                            Some(found) => Some(found),
                            None => match best_in_arc_col {
                                None => None,
                                Some(col) => find_row(col, in_arc_col_val, &col_out_arcs, &out_arc_rows),
                            },
                        }
                    } else {
                        best_in_arc_col.and_then(|col| find_row(col, in_arc_col_val, &col_out_arcs, &out_arc_rows))
                    };
                    let Some((row, weight)) = found else { break };
                    current_path[curr_path_len] = row;
                    aggregator.add_row(lp, data, row, weight);
                    curr_path_len += 1;
                }

                let mut path_len = aggregated_path.len();
                if path_len > 1 {
                    success |= Self::path_mixing_cut(&mut aggregated_path, &mut path_len, lp, trans_lp, &mut cut_gen, data, num_col, num_row);
                }
                aggregator.clear();
                if !try_negated_scale {
                    break;
                }
            }
            let _ = success;
        }
    }

    /// The path mixing cut over the aggregated path's rows.
    #[allow(clippy::too_many_arguments)]
    fn path_mixing_cut(
        aggregated_path: &mut [(Vec<usize>, Vec<f64>)],
        path_len: &mut usize,
        lp: &LpRelaxation,
        trans_lp: &mut TransformedLp,
        cut_gen: &mut CutGeneration,
        data: &mut MipData,
        num_col: usize,
        num_row: usize,
    ) -> bool {
        let feastol = data.feastol;
        let mut success = false;
        let mut index_pos: HashTable<usize, usize> = HashTable::new();
        let mut inds: Vec<usize> = Vec::with_capacity(num_col + num_row);
        let mut solval: Vec<f64> = Vec::with_capacity(num_col + num_row);
        let mut upper: Vec<f64> = Vec::with_capacity(num_col + num_row);
        let mut is_integral: Vec<bool> = Vec::with_capacity(num_col + num_row);
        let mut rhs = vec![0.0; *path_len];
        let mut tmp_upper: Vec<f64> = Vec::new();
        let mut tmp_solval: Vec<f64> = Vec::new();
        let mut delta = 1.0f64;
        for k in 0..*path_len {
            let mut integral_positive = false;
            let (path_inds, path_vals) = &mut aggregated_path[k];
            if !trans_lp.transform(lp, data, path_vals, &mut tmp_upper, &mut tmp_solval, path_inds, &mut rhs[k], &mut integral_positive, false) {
                *path_len = k;
                break;
            }
            if k == 0 {
                if rhs[k] > TINY {
                    *path_len = k;
                    break;
                }
                if rhs[k] >= -feastol {
                    rhs[k] = 0.0;
                }
            } else if rhs[k] >= rhs[k - 1] - feastol {
                *path_len = k;
                break;
            }
            delta = rhs[k].abs().max(delta);
            for j in 0..path_inds.len() {
                let index = path_inds[j];
                let pos = index_pos.entry(index);
                if *pos == 0 {
                    inds.push(index);
                    solval.push(tmp_solval[j]);
                    upper.push(tmp_upper[j]);
                    is_integral.push(lp.is_col_integral(index, data));
                    if *is_integral.last().expect("just pushed") {
                        delta = path_vals[j].abs().max(delta);
                    }
                    *pos = inds.len();
                } else if is_integral[*pos - 1] {
                    delta = path_vals[j].abs().max(delta);
                }
            }
        }
        if *path_len <= 1 {
            return false;
        }
        delta = (delta + 1.0).log2().ceil().exp2();
        let mut num_inds = inds.len();
        let mut cut_rhs = CDouble::new(0.0);
        let mut cut_vals = vec![0.0f64; num_inds];
        let mut max_frac = vec![0.0f64; num_inds];
        let mut down_sum = vec![CDouble::new(0.0); num_inds];
        let mut f_sum = vec![CDouble::new(0.0); num_inds];
        let mut f_last = 0.0;
        let mut scale = -1.0 / delta;
        for k in 0..*path_len {
            let f = rhs[k] * scale;
            let f_diff = CDouble::new(f) - f_last;
            cut_rhs += f_diff;
            let (path_inds, path_vals) = &aggregated_path[k];
            for j in 0..path_inds.len() {
                let i = *index_pos.find(&path_inds[j]).expect("indexed") - 1;
                let gj = path_vals[j] * scale;
                if !is_integral[i] {
                    cut_vals[i] = cut_vals[i].max(gj);
                } else {
                    let gjdown = gj.floor();
                    let hj = gj - gjdown;
                    max_frac[i] = max_frac[i].max(hj);
                    down_sum[i] += f_diff * gjdown;
                    f_sum[i] += f_diff;
                    cut_vals[i] = if f_sum[i] < max_frac[i] {
                        (down_sum[i] + f_sum[i]).value()
                    } else {
                        (down_sum[i] + max_frac[i]).value()
                    };
                }
            }
            if k > 0 {
                let mut viol = cut_rhs.value();
                for j in 0..num_inds {
                    viol -= solval[j] * cut_vals[j];
                }
                viol *= delta;
                if viol > 10.0 * feastol {
                    scale = -delta;
                    let mut rhs = cut_rhs.value() * scale;
                    for value in cut_vals.iter_mut() {
                        *value *= scale;
                    }
                    for j in (0..num_inds).rev() {
                        if cut_vals[j].abs() <= data.epsilon {
                            num_inds -= 1;
                            cut_vals.swap(j, num_inds);
                            inds.swap(j, num_inds);
                        }
                    }
                    cut_vals.truncate(num_inds);
                    inds.truncate(num_inds);
                    if trans_lp.untransform(lp, data, &mut cut_vals, &mut inds, &mut rhs, false) {
                        success |= cut_gen.finalize_and_add_cut(lp, data, &mut inds, &mut cut_vals, &mut rhs);
                    }
                    break;
                }
            }
            f_last = f;
        }
        let _ = upper;
        success
    }
}

/// `HighsTableauSeparator`'s `FractionalInteger`.
#[derive(Clone, Debug)]
struct FractionalInteger {
    fractionality: f64,
    row_ep_norm2: f64,
    score: f64,
    basis_index: usize,
    row_ep: Vec<(usize, f64)>,
}

/// HiGHS's `HighsTableauSeparator`: cuts from rows of the simplex tableau
/// of fractional basic integers.
pub(crate) struct TableauSeparator {
    num_tries: i64,
    pub stats: SeparatorStats,
}

impl TableauSeparator {
    pub fn new() -> Self {
        Self {
            num_tries: 0,
            stats: SeparatorStats::default(),
        }
    }

    pub fn run(&mut self, lp: &LpRelaxation, aggregator: &mut LpAggregator, trans_lp: &mut TransformedLp, data: &mut MipData) {
        self.stats.num_calls += 1;
        let before = data.cutpool.num_cuts();
        self.separate_lp_solution(lp, aggregator, trans_lp, data);
        self.stats.num_cuts_found += data.cutpool.num_cuts() - before;
    }

    fn separate_lp_solution(&mut self, lp: &LpRelaxation, aggregator: &mut LpAggregator, trans_lp: &mut TransformedLp, data: &mut MipData) {
        let solver = lp.lp_solver();
        if !solver.has_invert() {
            return;
        }
        if data.cutpool.num_available_cuts() > data.options.mip_pool_soft_limit {
            return;
        }
        let basisinds = solver.basic_variables().to_vec();
        let num_row = lp.num_rows();
        let num_col = lp.num_cols();
        let mut cut_gen = CutGeneration::new(lp, data);
        let mut base_row_inds: Vec<usize> = Vec::new();
        let mut base_row_vals: Vec<f64> = Vec::new();
        let col_value = solver.col_value().expect("an LP solution");
        let row_value = solver.row_value().expect("an LP solution");
        let feastol = data.feastol;

        let mut fractional_basisvars: Vec<FractionalInteger> = Vec::with_capacity(num_row);
        for (i, basisind) in basisinds[..num_row].iter().enumerate() {
            let my_fractionality = if *basisind >= num_col {
                let row = *basisind - num_col;
                if !lp.is_row_integral(row, data) {
                    continue;
                }
                crate::integers::fractionality(row_value[row])
            } else {
                let col = *basisind;
                if data.model.is_col_continuous(col) {
                    continue;
                }
                crate::integers::fractionality(col_value[col])
            };
            if my_fractionality < 1000.0 * feastol {
                continue;
            }
            fractional_basisvars.push(FractionalInteger {
                fractionality: my_fractionality,
                row_ep_norm2: 0.0,
                score: -1.0,
                basis_index: i,
                row_ep: Vec::new(),
            });
        }
        if fractional_basisvars.is_empty() {
            return;
        }
        let mut max_tries: i64 = 5000 + self.stats.num_calls as i64 * 50 + (data.total_lp_iterations - data.heuristic_lp_iterations) / 10;
        if self.num_tries >= max_tries {
            return;
        }
        max_tries -= self.num_tries;
        max_tries = max_tries.min(200 + (0.1 * num_row.min(data.integral_cols.len()) as f64) as i64);

        let reorder = |items: &mut Vec<FractionalInteger>, order: &[usize]| {
            let mut taken: Vec<Option<FractionalInteger>> = std::mem::take(items).into_iter().map(Some).collect();
            *items = order.iter().map(|&k| taken[k].take().expect("a permutation")).collect();
        };
        if fractional_basisvars.len() as i64 > max_tries {
            let mut order: Vec<usize> = (0..fractional_basisvars.len()).collect();
            let num_tries = self.num_tries;
            let tie = |f: &FractionalInteger| crate::hash_table::hash_u64((num_tries + f.basis_index as i64) as u64);
            let vars = &fractional_basisvars;
            if let Some(edge_wt) = solver.dual_edge_weights() {
                let score = |f: &FractionalInteger| f.fractionality * (1.0 - f.fractionality) / edge_wt[f.basis_index];
                crate::pdqsort::pdqsort(&mut order, |&a, &b| (score(&vars[a]), tie(&vars[a])) > (score(&vars[b]), tie(&vars[b])));
            } else {
                crate::pdqsort::pdqsort(&mut order, |&a, &b| (vars[a].fractionality, tie(&vars[a])) > (vars[b].fractionality, tie(&vars[b])));
            }
            reorder(&mut fractional_basisvars, &order);
            fractional_basisvars.truncate(max_tries as usize);
        }

        let mut row_ep_buffer = crate::hvector::HVector::new(num_row);
        self.num_tries += fractional_basisvars.len() as i64;
        for fracvar in fractional_basisvars.iter_mut() {
            solver.basis_inverse_row_sparse(fracvar.basis_index, &mut row_ep_buffer);
            if row_ep_buffer.count == 1 {
                continue;
            }
            fracvar.row_ep_norm2 = 0.0;
            let mut min_weight = INF;
            let mut max_weight = 0.0f64;
            fracvar.row_ep.reserve(row_ep_buffer.count as usize);
            for j in 0..row_ep_buffer.count as usize {
                let row = row_ep_buffer.index[j];
                let weight = row_ep_buffer.array[row];
                let max_abs_row_val = lp.max_abs_row_val(row, data);
                let scaled_weight = max_abs_row_val * weight.abs();
                if scaled_weight <= feastol {
                    continue;
                }
                min_weight = min_weight.min(scaled_weight);
                max_weight = max_weight.max(scaled_weight);
                fracvar.row_ep_norm2 += scaled_weight * scaled_weight;
                fracvar.row_ep.push((row, weight));
            }
            if fracvar.row_ep.len() <= 1 {
                continue;
            }
            if max_weight / min_weight <= 1e4 {
                fracvar.score = fracvar.fractionality * (1.0 - fracvar.fractionality) / fracvar.row_ep_norm2;
            }
        }
        fractional_basisvars.retain(|f| f.score > feastol);
        if fractional_basisvars.is_empty() {
            return;
        }
        let mut order: Vec<usize> = (0..fractional_basisvars.len()).collect();
        {
            let vars = &fractional_basisvars;
            crate::pdqsort::pdqsort_branchless(&mut order, |&a, &b| vars[a].score > vars[b].score);
        }
        reorder(&mut fractional_basisvars, &order);
        let mut best_score = -1.0;
        let num_cuts = data.cutpool.num_cuts();
        const BEST_SCORE_FAC: [f64; 2] = [0.0025, 0.01];
        let num_model_cols = data.model.num_col;
        for fracvar in &fractional_basisvars {
            if data.cutpool.num_cuts() - num_cuts >= 1000 {
                break;
            }
            if fracvar.score < BEST_SCORE_FAC[usize::from(data.cutpool.num_cuts() - num_cuts >= 50)] * best_score {
                break;
            }
            for &(row, weight) in &fracvar.row_ep {
                aggregator.add_row(lp, data, row, weight);
            }
            aggregator.current_aggregation(lp, data, &mut base_row_inds, &mut base_row_vals, false);
            if 10 * (base_row_inds.len() - fracvar.row_ep.len()) > 10000 + num_model_cols {
                aggregator.clear();
                continue;
            }
            let len = base_row_inds.len();
            if len > fracvar.row_ep.len() {
                let mut max_abs_val = 0.0f64;
                let mut min_abs_val = INF;
                for i in 0..len {
                    if base_row_inds[i] < num_model_cols {
                        max_abs_val = base_row_vals[i].abs().max(max_abs_val);
                        min_abs_val = base_row_vals[i].abs().min(min_abs_val);
                    }
                }
                if max_abs_val / min_abs_val > 1e6 {
                    aggregator.clear();
                    continue;
                }
            }
            let mut rhs = 0.0;
            cut_gen.generate_cut(trans_lp, lp, data, &mut base_row_inds, &mut base_row_vals, &mut rhs, false);
            if data.domain.infeasible() {
                break;
            }
            aggregator.current_aggregation(lp, data, &mut base_row_inds, &mut base_row_vals, true);
            rhs = 0.0;
            cut_gen.generate_cut(trans_lp, lp, data, &mut base_row_inds, &mut base_row_vals, &mut rhs, false);
            if data.domain.infeasible() {
                break;
            }
            aggregator.clear();
            if best_score == -1.0 && data.cutpool.num_cuts() != num_cuts {
                best_score = fracvar.score;
            }
        }
    }
}

/// HiGHS's `HighsModkSeparator`: mod-k cuts for k = 2, 3, 5, 7 from tight
/// rows over integers.
pub(crate) struct ModkSeparator {
    pub stats: SeparatorStats,
}

impl ModkSeparator {
    pub fn new() -> Self {
        Self { stats: SeparatorStats::default() }
    }

    pub fn run(&mut self, lp: &LpRelaxation, aggregator: &mut LpAggregator, trans_lp: &mut TransformedLp, data: &mut MipData) {
        self.stats.num_calls += 1;
        let before = data.cutpool.num_cuts();
        Self::separate_lp_solution(lp, aggregator, trans_lp, data);
        self.stats.num_cuts_found += data.cutpool.num_cuts() - before;
    }

    fn separate_lp_solution(lp: &LpRelaxation, aggregator: &mut LpAggregator, trans_lp: &mut TransformedLp, data: &mut MipData) {
        use crate::integers::{integral_scale, nearest_integer};
        let solver = lp.lp_solver();
        let num_row = solver.num_rows();
        let num_col = solver.num_cols();
        let (a_start, a_index, _) = solver.matrix();
        let mut skip_row = vec![false; num_row];
        for &col in &data.continuous_cols {
            if trans_lp.bound_distance(col) == 0.0 {
                continue;
            }
            for i in a_start[col]..a_start[col + 1] {
                skip_row[a_index[i]] = true;
            }
        }
        let num_nz = a_index.len();
        let mut cut_gen = CutGeneration::new(lp, data);
        let mut integral_scales: Vec<(usize, f64)> = Vec::new();
        let mut int_system_value: Vec<i64> = Vec::with_capacity(num_nz + num_row);
        let mut int_system_index: Vec<usize> = Vec::with_capacity(num_nz + num_row);
        let mut int_system_start: Vec<usize> = Vec::with_capacity(num_row + 1);
        int_system_start.push(0);
        let mut inds: Vec<usize> = Vec::with_capacity(num_col);
        let mut vals: Vec<f64> = Vec::with_capacity(num_col);
        let mut scale_vals: Vec<f64> = Vec::with_capacity(num_col);
        let mut upper: Vec<f64> = Vec::new();
        let mut solval: Vec<f64> = Vec::new();
        let row_lower = solver.row_lower().to_vec();
        let row_upper = solver.row_upper().to_vec();
        let row_value = solver.row_value().expect("an LP solution").to_vec();
        let feastol = data.feastol;
        let epsilon = data.epsilon;
        let mut num_nonzero_rhs = 0;
        let max_int_row_len = (1000.0 + 0.1 * num_col as f64) as i32 as usize;
        for row in 0..num_row {
            if skip_row[row] {
                continue;
            }
            let leq_row = if row_upper[row] - row_value[row] <= feastol {
                true
            } else if row_value[row] - row_lower[row] <= feastol {
                false
            } else {
                continue;
            };
            let (rowinds, rowvals) = lp.row(row, data);
            let mut rhs;
            inds.clear();
            inds.extend_from_slice(rowinds);
            vals.clear();
            if leq_row {
                rhs = row_upper[row];
                vals.extend_from_slice(rowvals);
            } else {
                rhs = -row_lower[row];
                vals.extend(rowvals.iter().map(|&x| -x));
            }
            let mut integral_positive = false;
            if !trans_lp.transform(lp, data, &mut vals, &mut upper, &mut solval, &mut inds, &mut rhs, &mut integral_positive, true) {
                continue;
            }
            let rowlen = inds.len();
            if rowlen > max_int_row_len {
                let mut int_row_len = 0;
                for i in 0..rowlen {
                    if solval[i] <= feastol || data.model.is_col_continuous(inds[i]) {
                        continue;
                    }
                    int_row_len += 1;
                }
                if int_row_len > max_int_row_len || (int_row_len == 0 && rhs.abs() <= epsilon) {
                    continue;
                }
            }
            let intscale;
            let intrhs;
            if !lp.is_row_integral(row, data) {
                scale_vals.clear();
                for i in 0..rowlen {
                    if data.model.is_col_continuous(inds[i]) {
                        continue;
                    }
                    if solval[i] > feastol {
                        scale_vals.push(vals[i]);
                    }
                }
                if rhs.abs() > epsilon {
                    scale_vals.push(-rhs);
                }
                if scale_vals.is_empty() {
                    continue;
                }
                intscale = integral_scale(&scale_vals, feastol, epsilon);
                if intscale == 0.0 || intscale > 1e6 {
                    continue;
                }
                intrhs = nearest_integer(intscale * rhs);
                for i in 0..rowlen {
                    if data.model.is_col_continuous(inds[i]) {
                        continue;
                    }
                    if solval[i] > feastol {
                        int_system_index.push(inds[i]);
                        int_system_value.push(nearest_integer(intscale * vals[i]));
                    }
                }
            } else {
                intscale = 1.0;
                intrhs = nearest_integer(rhs);
                for i in 0..rowlen {
                    if solval[i] > feastol {
                        int_system_index.push(inds[i]);
                        int_system_value.push(nearest_integer(vals[i]));
                    }
                }
            }
            num_nonzero_rhs += usize::from(intrhs != 0);
            int_system_index.push(num_col);
            int_system_value.push(intrhs);
            int_system_start.push(int_system_value.len());
            integral_scales.push((row, intscale));
        }
        if integral_scales.is_empty() || num_nonzero_rhs == 0 {
            return;
        }
        let mut used_weights: std::collections::HashSet<Vec<super::gfk_solve::SolutionEntry>> = std::collections::HashSet::new();
        for k in [2u32, 3, 5, 7] {
            let num_cuts = data.cutpool.num_cuts();
            let mut gfk = super::gfk_solve::GfkSolve::default();
            gfk.load_csc(k, &int_system_value, &int_system_index, &int_system_start, num_col + 1);
            gfk.set_rhs(k, num_col, 1);
            gfk.solve(k, |weights| {
                if weights.is_empty() {
                    return;
                }
                crate::pdqsort::pdqsort(weights, |a, b| a.index < b.index);
                if !used_weights.insert(weights.clone()) {
                    return;
                }
                for w in weights.iter() {
                    let weight = integral_scales[w.index].1 * (((w.weight * (k - 1)) % k) as f64 / k as f64);
                    aggregator.add_row(lp, data, integral_scales[w.index].0, weight);
                }
                aggregator.current_aggregation(lp, data, &mut inds, &mut vals, false);
                let mut rhs = 0.0;
                cut_gen.generate_cut(trans_lp, lp, data, &mut inds, &mut vals, &mut rhs, true);
                if k != 2 {
                    aggregator.clear();
                    for w in weights.iter() {
                        let weight = integral_scales[w.index].1 * (w.weight as f64 / k as f64);
                        aggregator.add_row(lp, data, integral_scales[w.index].0, weight);
                    }
                }
                aggregator.current_aggregation(lp, data, &mut inds, &mut vals, true);
                rhs = 0.0;
                cut_gen.generate_cut(trans_lp, lp, data, &mut inds, &mut vals, &mut rhs, true);
                aggregator.clear();
            });
            if data.cutpool.num_cuts() != num_cuts {
                return;
            }
            used_weights.clear();
        }
    }
}
