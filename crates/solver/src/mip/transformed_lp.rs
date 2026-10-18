//! HiGHS's `HighsTransformedLp`: rows of the LP relaxation rewritten for
//! cut separation, with columns shifted to or complemented from their
//! closest simple or variable bound, and the cuts found mapped back.

use super::{
    data::MipData,
    implications::{Implications, VarBound},
    lp_relaxation::LpRelaxation,
    sparse_vector_sum::SparseVectorSum,
};
use crate::{integers::nearest_integer, util::CDouble};

const INF: f64 = f64::INFINITY;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BoundType {
    #[default]
    SimpleUb,
    SimpleLb,
    VariableUb,
    VariableLb,
}

pub(crate) struct TransformedLp {
    best_vub: Vec<(Option<usize>, VarBound)>,
    best_vlb: Vec<(Option<usize>, VarBound)>,
    simple_lb_dist: Vec<f64>,
    simple_ub_dist: Vec<f64>,
    lb_dist: Vec<f64>,
    ub_dist: Vec<f64>,
    bound_dist: Vec<f64>,
    bound_types: Vec<BoundType>,
    vectorsum: SparseVectorSum,
}

impl TransformedLp {
    /// HiGHS's constructor: the bound distances of every column and slack
    /// at the LP solution, cleaning up variable bounds on the way.
    pub fn new(lp: &LpRelaxation, data: &mut MipData) -> Self {
        let num_transformed_col = lp.num_cols() + lp.num_rows();
        let mut this = Self {
            best_vub: vec![(None, VarBound::default()); num_transformed_col],
            best_vlb: vec![(None, VarBound::default()); num_transformed_col],
            simple_lb_dist: vec![0.0; num_transformed_col],
            simple_ub_dist: vec![0.0; num_transformed_col],
            lb_dist: vec![0.0; num_transformed_col],
            ub_dist: vec![0.0; num_transformed_col],
            bound_dist: vec![0.0; num_transformed_col],
            bound_types: vec![BoundType::default(); num_transformed_col],
            vectorsum: SparseVectorSum::new(num_transformed_col),
        };
        let solver = lp.lp_solver();
        let col_value = solver.col_value().expect("an LP solution").to_vec();
        let col_dual = solver.col_dual().expect("LP duals").to_vec();
        let row_value = solver.row_value().expect("an LP solution").to_vec();
        let feastol = data.feastol;

        let cleanup = |data: &mut MipData, col: usize| {
            let mut parts = data.parts();
            parts.implications.cleanup_varbounds(col, parts.domain, parts.cliquetable, &mut parts.cx);
        };

        for &col in &data.continuous_cols.clone() {
            cleanup(data, col);
            if data.domain.infeasible() {
                return this;
            }
            if data.domain.is_fixed(col) {
                continue;
            }
            let globaldom = &data.domain;
            let nodequeue = data.nodequeue.borrow();
            let mut bestub = globaldom.col_upper[col];
            this.simple_ub_dist[col] = bestub - col_value[col];
            if this.simple_ub_dist[col] <= feastol {
                this.simple_ub_dist[col] = 0.0;
            }
            this.best_vub[col] = data.implications.get_best_vub(col, &col_value, &col_dual, &mut bestub, globaldom, &nodequeue, feastol);
            let mut bestlb = globaldom.col_lower[col];
            this.simple_lb_dist[col] = col_value[col] - bestlb;
            if this.simple_lb_dist[col] <= feastol {
                this.simple_lb_dist[col] = 0.0;
            }
            this.best_vlb[col] = data.implications.get_best_vlb(col, &col_value, &col_dual, &mut bestlb, globaldom, &nodequeue, feastol);
            this.lb_dist[col] = col_value[col] - bestlb;
            if this.lb_dist[col] <= feastol {
                this.lb_dist[col] = 0.0;
            }
            this.ub_dist[col] = bestub - col_value[col];
            if this.ub_dist[col] <= feastol {
                this.ub_dist[col] = 0.0;
            }
            this.bound_dist[col] = this.lb_dist[col].min(this.ub_dist[col]);
        }

        for &col in &data.integral_cols.clone() {
            let mut bestub = data.domain.col_upper[col];
            let mut bestlb = data.domain.col_lower[col];
            cleanup(data, col);
            if data.domain.infeasible() {
                return this;
            }
            this.simple_ub_dist[col] = bestub - col_value[col];
            if this.simple_ub_dist[col] <= feastol {
                this.simple_ub_dist[col] = 0.0;
            }
            this.simple_lb_dist[col] = col_value[col] - bestlb;
            if this.simple_lb_dist[col] <= feastol {
                this.simple_lb_dist[col] = 0.0;
            }
            let simple_bnd_dist = this.simple_lb_dist[col].min(this.simple_ub_dist[col]);
            if simple_bnd_dist > 0.0 && (nearest_integer(col_value[col]) as f64 - col_value[col]).abs() < feastol {
                let globaldom = &data.domain;
                let nodequeue = data.nodequeue.borrow();
                this.best_vub[col] = data.implications.get_best_vub(col, &col_value, &col_dual, &mut bestub, globaldom, &nodequeue, feastol);
                this.best_vlb[col] = data.implications.get_best_vlb(col, &col_value, &col_dual, &mut bestlb, globaldom, &nodequeue, feastol);
                this.lb_dist[col] = col_value[col] - bestlb;
                if this.lb_dist[col] <= feastol {
                    this.lb_dist[col] = 0.0;
                }
                this.ub_dist[col] = bestub - col_value[col];
                if this.ub_dist[col] <= feastol {
                    this.ub_dist[col] = 0.0;
                }
                this.bound_dist[col] = this.lb_dist[col].min(this.ub_dist[col]);
                if this.bound_dist[col] > simple_bnd_dist + feastol {
                    this.lb_dist[col] = this.simple_lb_dist[col];
                    this.ub_dist[col] = this.simple_ub_dist[col];
                    this.bound_dist[col] = simple_bnd_dist;
                    this.best_vub[col].0 = None;
                    this.best_vlb[col].0 = None;
                }
            } else {
                this.lb_dist[col] = this.simple_lb_dist[col];
                this.ub_dist[col] = this.simple_ub_dist[col];
                this.bound_dist[col] = simple_bnd_dist;
            }
        }

        let num_lp_row = lp.num_rows();
        let index_offset = data.model.num_col;
        for row in 0..num_lp_row {
            let slack_index = index_offset + row;
            let bestub = lp.slack_upper(row, &data.domain);
            let bestlb = lp.slack_lower(row, &data.domain);
            if bestlb == bestub {
                continue;
            }
            this.lb_dist[slack_index] = row_value[row] - bestlb;
            if this.lb_dist[slack_index] <= feastol {
                this.lb_dist[slack_index] = 0.0;
            }
            this.simple_lb_dist[slack_index] = this.lb_dist[slack_index];
            this.ub_dist[slack_index] = bestub - row_value[row];
            if this.ub_dist[slack_index] <= feastol {
                this.ub_dist[slack_index] = 0.0;
            }
            this.simple_ub_dist[slack_index] = this.ub_dist[slack_index];
            this.bound_dist[slack_index] = this.lb_dist[slack_index].min(this.ub_dist[slack_index]);
        }
        this
    }

    pub fn bound_distance(&self, col: usize) -> f64 {
        self.bound_dist[col]
    }

    /// HiGHS's `transform`: rewrite the row `vals . x <= rhs` over
    /// nonnegative columns with upper bounds `upper` and solution values
    /// `solval`; false if it cannot be (a free column, or nothing left).
    #[allow(clippy::too_many_arguments)]
    pub fn transform(
        &mut self,
        lp: &LpRelaxation,
        data: &MipData,
        vals: &mut Vec<f64>,
        upper: &mut Vec<f64>,
        solval: &mut Vec<f64>,
        inds: &mut Vec<usize>,
        rhs: &mut f64,
        integers_positive: &mut bool,
        prefer_vbds: bool,
    ) -> bool {
        let globaldom = &data.domain;
        let mut tmp_rhs = CDouble::new(*rhs);
        let slack_offset = lp.num_cols();
        let feastol = data.feastol;
        let small = data.options.small_matrix_value;
        let mut num_nz = inds.len();
        let get_lb = |col: usize| {
            if col < slack_offset {
                globaldom.col_lower[col]
            } else {
                lp.slack_lower(col - slack_offset, globaldom)
            }
        };
        let get_ub = |col: usize| {
            if col < slack_offset {
                globaldom.col_upper[col]
            } else {
                lp.slack_upper(col - slack_offset, globaldom)
            }
        };
        let remove = |inds: &mut Vec<usize>, vals: &mut Vec<f64>, num_nz: &mut usize, position: usize| {
            *num_nz -= 1;
            inds[position] = inds[*num_nz];
            vals[position] = vals[*num_nz];
            inds[*num_nz] = 0;
            vals[*num_nz] = 0.0;
        };

        let mut i = 0;
        while i < num_nz {
            let col = inds[i];
            let lb = get_lb(col);
            let ub = get_ub(col);
            if ub - lb < small {
                tmp_rhs -= lb.min(ub) * vals[i];
                remove(inds, vals, &mut num_nz, i);
                continue;
            }
            if lb == -INF && ub == INF {
                self.vectorsum.clear();
                return false;
            }
            // Tighten the best variable bounds against bounds found since.
            if self.best_vub[col].0.is_some() && self.best_vub[col].1.max_value() > ub + feastol {
                Implications::cleanup_vub(self.best_vub[col].0, &mut self.best_vub[col].1, ub, feastol, data.epsilon);
            }
            if self.best_vlb[col].0.is_some() && self.best_vlb[col].1.min_value() < lb - feastol {
                Implications::cleanup_vlb(self.best_vlb[col].0, &mut self.best_vlb[col].1, lb, feastol, data.epsilon);
            }
            let old_bound_type = self.bound_types[col];
            if lp.is_col_integral(col, data) {
                if ub - lb <= 1.5 || self.bound_dist[col] != 0.0 || self.simple_lb_dist[col] == 0.0 || self.simple_ub_dist[col] == 0.0 {
                    self.bound_types[col] = if self.simple_lb_dist[col] < self.simple_ub_dist[col] - feastol {
                        BoundType::SimpleLb
                    } else if self.simple_ub_dist[col] < self.simple_lb_dist[col] - feastol {
                        BoundType::SimpleUb
                    } else if vals[i] > 0.0 {
                        BoundType::SimpleLb
                    } else {
                        BoundType::SimpleUb
                    };
                    i += 1;
                    continue;
                }
                self.bound_types[col] = if self.best_vlb[col].0.is_none() || self.ub_dist[col] < self.lb_dist[col] - feastol {
                    BoundType::VariableUb
                } else if self.best_vub[col].0.is_none() || self.lb_dist[col] < self.ub_dist[col] - feastol {
                    BoundType::VariableLb
                } else if vals[i] > 0.0 {
                    BoundType::VariableUb
                } else {
                    BoundType::VariableLb
                };
            } else {
                self.bound_types[col] = if self.lb_dist[col] < self.ub_dist[col] - feastol {
                    if self.best_vlb[col].0.is_none() {
                        BoundType::SimpleLb
                    } else if prefer_vbds || vals[i] > 0.0 || self.simple_lb_dist[col] > self.lb_dist[col] + feastol {
                        BoundType::VariableLb
                    } else {
                        BoundType::SimpleLb
                    }
                } else if self.ub_dist[col] < self.lb_dist[col] - feastol {
                    if self.best_vub[col].0.is_none() {
                        BoundType::SimpleUb
                    } else if prefer_vbds || vals[i] < 0.0 || self.simple_ub_dist[col] > self.ub_dist[col] + feastol {
                        BoundType::VariableUb
                    } else {
                        BoundType::SimpleUb
                    }
                } else if vals[i] > 0.0 {
                    if self.best_vlb[col].0.is_some() {
                        BoundType::VariableLb
                    } else if prefer_vbds && self.best_vub[col].0.is_some() {
                        BoundType::VariableUb
                    } else {
                        BoundType::SimpleLb
                    }
                } else if self.best_vub[col].0.is_some() {
                    BoundType::VariableUb
                } else if prefer_vbds && self.best_vlb[col].0.is_some() {
                    BoundType::VariableLb
                } else {
                    BoundType::SimpleUb
                };
            }
            match self.bound_types[col] {
                BoundType::SimpleLb => {
                    if vals[i] > 0.0 {
                        tmp_rhs -= lb * vals[i];
                        self.bound_types[col] = old_bound_type;
                        remove(inds, vals, &mut num_nz, i);
                        continue;
                    }
                }
                BoundType::SimpleUb => {
                    if vals[i] < 0.0 {
                        tmp_rhs -= ub * vals[i];
                        self.bound_types[col] = old_bound_type;
                        remove(inds, vals, &mut num_nz, i);
                        continue;
                    }
                }
                BoundType::VariableLb => {
                    let (vlb_col, vlb) = self.best_vlb[col];
                    tmp_rhs -= vlb.constant * vals[i];
                    self.vectorsum.add(vlb_col.expect("a variable lower bound"), vals[i] * vlb.coef);
                    if vals[i] > 0.0 {
                        self.bound_types[col] = old_bound_type;
                        remove(inds, vals, &mut num_nz, i);
                        continue;
                    }
                }
                BoundType::VariableUb => {
                    let (vub_col, vub) = self.best_vub[col];
                    tmp_rhs -= vub.constant * vals[i];
                    self.vectorsum.add(vub_col.expect("a variable upper bound"), vals[i] * vub.coef);
                    vals[i] = -vals[i];
                    if vals[i] > 0.0 {
                        self.bound_types[col] = old_bound_type;
                        remove(inds, vals, &mut num_nz, i);
                        continue;
                    }
                }
            }
            i += 1;
        }

        if !self.vectorsum.nonzeros().is_empty() {
            for i in 0..num_nz {
                if vals[i] != 0.0 {
                    self.vectorsum.add(inds[i], vals[i]);
                }
            }
            self.vectorsum.cleanup(|_, val| val.abs() <= small);
            inds.clone_from(&self.vectorsum.nonzeroinds);
            num_nz = inds.len();
            vals.resize(num_nz, 0.0);
            for j in 0..num_nz {
                vals[j] = self.vectorsum.value(inds[j]);
            }
            self.vectorsum.clear();
        } else {
            vals.resize(num_nz, 0.0);
            inds.resize(num_nz, 0);
        }

        for j in 0..num_nz {
            let col = inds[j];
            let lb = get_lb(col);
            let ub = get_ub(col);
            if !lp.is_col_integral(col, data) {
                continue;
            }
            if matches!(self.bound_types[col], BoundType::VariableLb | BoundType::VariableUb) {
                continue;
            }
            self.bound_types[col] = if *integers_positive {
                if (lb != -INF && vals[j] > 0.0) || ub == INF {
                    BoundType::SimpleLb
                } else {
                    BoundType::SimpleUb
                }
            } else if self.lb_dist[col] < self.ub_dist[col] {
                BoundType::SimpleLb
            } else {
                BoundType::SimpleUb
            };
        }

        upper.resize(num_nz, 0.0);
        solval.resize(num_nz, 0.0);
        for j in 0..num_nz {
            let col = inds[j];
            let lb = get_lb(col);
            let ub = get_ub(col);
            upper[j] = ub - lb;
            match self.bound_types[col] {
                BoundType::SimpleLb => {
                    tmp_rhs -= lb * vals[j];
                    solval[j] = self.lb_dist[col];
                }
                BoundType::SimpleUb => {
                    tmp_rhs -= ub * vals[j];
                    vals[j] = -vals[j];
                    solval[j] = self.ub_dist[col];
                }
                BoundType::VariableLb => solval[j] = self.lb_dist[col],
                BoundType::VariableUb => solval[j] = self.ub_dist[col],
            }
            if lp.is_col_integral(col, data) {
                *integers_positive = *integers_positive && vals[j] > 0.0;
            }
        }
        *rhs = tmp_rhs.value();
        !(num_nz == 0 && *rhs >= -feastol)
    }

    /// HiGHS's `untransform`: map a cut over the transformed columns back
    /// to the original ones; false if a dropped tiny coefficient sits on an
    /// unbounded column.
    pub fn untransform(&mut self, lp: &LpRelaxation, data: &MipData, vals: &mut Vec<f64>, inds: &mut Vec<usize>, rhs: &mut f64, integral: bool) -> bool {
        let globaldom = &data.domain;
        let mut tmp_rhs = CDouble::new(*rhs);
        let slack_offset = data.model.num_col;
        let num_nz = inds.len();
        for i in 0..num_nz {
            if vals[i] == 0.0 {
                continue;
            }
            let col = inds[i];
            match self.bound_types[col] {
                BoundType::VariableLb => {
                    let (vlb_col, vlb) = self.best_vlb[col];
                    tmp_rhs += vlb.constant * vals[i];
                    self.vectorsum.add(vlb_col.expect("a variable lower bound"), -vals[i] * vlb.coef);
                    self.vectorsum.add(col, vals[i]);
                }
                BoundType::VariableUb => {
                    let (vub_col, vub) = self.best_vub[col];
                    tmp_rhs -= vub.constant * vals[i];
                    self.vectorsum.add(vub_col.expect("a variable upper bound"), vals[i] * vub.coef);
                    self.vectorsum.add(col, -vals[i]);
                }
                BoundType::SimpleLb => {
                    if col < slack_offset {
                        tmp_rhs += vals[i] * globaldom.col_lower[col];
                        self.vectorsum.add(col, vals[i]);
                    } else {
                        let row = col - slack_offset;
                        tmp_rhs += vals[i] * lp.slack_lower(row, globaldom);
                        let (rowinds, rowvals) = lp.row(row, data);
                        for j in 0..rowinds.len() {
                            self.vectorsum.add(rowinds[j], vals[i] * rowvals[j]);
                        }
                    }
                }
                BoundType::SimpleUb => {
                    if col < slack_offset {
                        tmp_rhs -= vals[i] * globaldom.col_upper[col];
                        self.vectorsum.add(col, -vals[i]);
                    } else {
                        let row = col - slack_offset;
                        tmp_rhs -= vals[i] * lp.slack_upper(row, globaldom);
                        vals[i] = -vals[i];
                        let (rowinds, rowvals) = lp.row(row, data);
                        for j in 0..rowinds.len() {
                            self.vectorsum.add(rowinds[j], vals[i] * rowvals[j]);
                        }
                    }
                }
            }
        }
        if integral {
            self.vectorsum.cleanup(|_, val| val.abs() < 0.5);
            *rhs = tmp_rhs.value().round();
        } else {
            let mut abort = false;
            let small = data.options.small_matrix_value;
            let feastol = data.feastol;
            self.vectorsum.cleanup(|col, val| {
                let absval = val.abs();
                if absval <= small {
                    return true;
                }
                if absval <= feastol {
                    if val > 0.0 {
                        if globaldom.col_lower[col] == -INF {
                            abort = true;
                        } else {
                            tmp_rhs -= val * globaldom.col_lower[col];
                        }
                    } else if globaldom.col_upper[col] == INF {
                        abort = true;
                    } else {
                        tmp_rhs -= val * globaldom.col_upper[col];
                    }
                    return true;
                }
                false
            });
            if abort {
                self.vectorsum.clear();
                return false;
            }
            *rhs = tmp_rhs.value();
        }
        inds.clone_from(&self.vectorsum.nonzeroinds);
        let num_nz = inds.len();
        vals.resize(num_nz, 0.0);
        for i in 0..num_nz {
            vals[i] = if integral {
                self.vectorsum.value(inds[i]).round()
            } else {
                self.vectorsum.value(inds[i])
            };
        }
        self.vectorsum.clear();
        true
    }
}
