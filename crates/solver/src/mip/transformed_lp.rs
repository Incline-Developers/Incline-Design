//! HiGHS's `HighsTransformedLp`: rows of the LP relaxation rewritten for
//! cut separation, with columns shifted to or complemented from their
//! closest simple or variable bound, and the cuts found mapped back.

use super::{
    data::MipData,
    implications::{Cleanup, Implications, VarBound},
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

// Cut rows visit columns sparsely, so keep the distances they read together.
#[derive(Clone, Copy, Default)]
struct ColumnBounds {
    simple_lb_dist: f64,
    simple_ub_dist: f64,
    lb_dist: f64,
    ub_dist: f64,
    bound_dist: f64,
    bound_type: BoundType,
    integral: bool,
}

pub(crate) struct TransformedLp {
    best_vub: Vec<(Option<usize>, VarBound)>,
    best_vlb: Vec<(Option<usize>, VarBound)>,
    bounds: Vec<ColumnBounds>,
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
            // Column and row integrality stays fixed throughout this separation
            // round. Keep it with the distances read for each transformed term.
            bounds: (0..num_transformed_col)
                .map(|col| ColumnBounds {
                    integral: lp.is_col_integral(col, data),
                    ..ColumnBounds::default()
                })
                .collect(),
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
            this.bounds[col].simple_ub_dist = bestub - col_value[col];
            if this.bounds[col].simple_ub_dist <= feastol {
                this.bounds[col].simple_ub_dist = 0.0;
            }
            this.best_vub[col] = data.implications.get_best_vub(col, &col_value, &col_dual, &mut bestub, globaldom, &nodequeue, feastol);
            let mut bestlb = globaldom.col_lower[col];
            this.bounds[col].simple_lb_dist = col_value[col] - bestlb;
            if this.bounds[col].simple_lb_dist <= feastol {
                this.bounds[col].simple_lb_dist = 0.0;
            }
            this.best_vlb[col] = data.implications.get_best_vlb(col, &col_value, &col_dual, &mut bestlb, globaldom, &nodequeue, feastol);
            this.bounds[col].lb_dist = col_value[col] - bestlb;
            if this.bounds[col].lb_dist <= feastol {
                this.bounds[col].lb_dist = 0.0;
            }
            this.bounds[col].ub_dist = bestub - col_value[col];
            if this.bounds[col].ub_dist <= feastol {
                this.bounds[col].ub_dist = 0.0;
            }
            this.bounds[col].bound_dist = this.bounds[col].lb_dist.min(this.bounds[col].ub_dist);
        }

        for &col in &data.integral_cols.clone() {
            let mut bestub = data.domain.col_upper[col];
            let mut bestlb = data.domain.col_lower[col];
            cleanup(data, col);
            if data.domain.infeasible() {
                return this;
            }
            this.bounds[col].simple_ub_dist = bestub - col_value[col];
            if this.bounds[col].simple_ub_dist <= feastol {
                this.bounds[col].simple_ub_dist = 0.0;
            }
            this.bounds[col].simple_lb_dist = col_value[col] - bestlb;
            if this.bounds[col].simple_lb_dist <= feastol {
                this.bounds[col].simple_lb_dist = 0.0;
            }
            let simple_bnd_dist = this.bounds[col].simple_lb_dist.min(this.bounds[col].simple_ub_dist);
            if simple_bnd_dist > 0.0 && (nearest_integer(col_value[col]) as f64 - col_value[col]).abs() < feastol {
                let globaldom = &data.domain;
                let nodequeue = data.nodequeue.borrow();
                this.best_vub[col] = data.implications.get_best_vub(col, &col_value, &col_dual, &mut bestub, globaldom, &nodequeue, feastol);
                this.best_vlb[col] = data.implications.get_best_vlb(col, &col_value, &col_dual, &mut bestlb, globaldom, &nodequeue, feastol);
                this.bounds[col].lb_dist = col_value[col] - bestlb;
                if this.bounds[col].lb_dist <= feastol {
                    this.bounds[col].lb_dist = 0.0;
                }
                this.bounds[col].ub_dist = bestub - col_value[col];
                if this.bounds[col].ub_dist <= feastol {
                    this.bounds[col].ub_dist = 0.0;
                }
                this.bounds[col].bound_dist = this.bounds[col].lb_dist.min(this.bounds[col].ub_dist);
                if this.bounds[col].bound_dist > simple_bnd_dist + feastol {
                    this.bounds[col].lb_dist = this.bounds[col].simple_lb_dist;
                    this.bounds[col].ub_dist = this.bounds[col].simple_ub_dist;
                    this.bounds[col].bound_dist = simple_bnd_dist;
                    this.best_vub[col].0 = None;
                    this.best_vlb[col].0 = None;
                }
            } else {
                this.bounds[col].lb_dist = this.bounds[col].simple_lb_dist;
                this.bounds[col].ub_dist = this.bounds[col].simple_ub_dist;
                this.bounds[col].bound_dist = simple_bnd_dist;
            }
        }

        let num_lp_row = lp.num_rows();
        let index_offset = data.model.num_col;
        for (row, value) in row_value[..num_lp_row].iter().enumerate() {
            let slack_index = index_offset + row;
            let bestub = lp.slack_upper(row, &data.domain);
            let bestlb = lp.slack_lower(row, &data.domain);
            if bestlb == bestub {
                continue;
            }
            this.bounds[slack_index].lb_dist = *value - bestlb;
            if this.bounds[slack_index].lb_dist <= feastol {
                this.bounds[slack_index].lb_dist = 0.0;
            }
            this.bounds[slack_index].simple_lb_dist = this.bounds[slack_index].lb_dist;
            this.bounds[slack_index].ub_dist = bestub - *value;
            if this.bounds[slack_index].ub_dist <= feastol {
                this.bounds[slack_index].ub_dist = 0.0;
            }
            this.bounds[slack_index].simple_ub_dist = this.bounds[slack_index].ub_dist;
            this.bounds[slack_index].bound_dist = this.bounds[slack_index].lb_dist.min(this.bounds[slack_index].ub_dist);
        }
        this
    }

    pub fn bound_distance(&self, col: usize) -> f64 {
        self.bounds[col].bound_dist
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
            // A variable bound made redundant is dropped, falling back to
            // the simple bound (HiGHS #3177).
            if self.best_vub[col].0.is_some()
                && self.best_vub[col].1.max_value() > ub + feastol
                && matches!(
                    Implications::cleanup_vub(self.best_vub[col].0, &mut self.best_vub[col].1, ub, feastol, data.epsilon),
                    Cleanup::Redundant
                )
            {
                self.best_vub[col].0 = None;
                self.bounds[col].ub_dist = self.bounds[col].simple_ub_dist;
                self.bounds[col].bound_dist = self.bounds[col].lb_dist.min(self.bounds[col].ub_dist);
            }
            if self.best_vlb[col].0.is_some()
                && self.best_vlb[col].1.min_value() < lb - feastol
                && matches!(
                    Implications::cleanup_vlb(self.best_vlb[col].0, &mut self.best_vlb[col].1, lb, feastol, data.epsilon),
                    Cleanup::Redundant
                )
            {
                self.best_vlb[col].0 = None;
                self.bounds[col].lb_dist = self.bounds[col].simple_lb_dist;
                self.bounds[col].bound_dist = self.bounds[col].lb_dist.min(self.bounds[col].ub_dist);
            }
            let old_bound_type = self.bounds[col].bound_type;
            if self.bounds[col].integral {
                if ub - lb <= 1.5 || self.bounds[col].bound_dist != 0.0 || self.bounds[col].simple_lb_dist == 0.0 || self.bounds[col].simple_ub_dist == 0.0 {
                    self.bounds[col].bound_type = if self.bounds[col].simple_lb_dist < self.bounds[col].simple_ub_dist - feastol {
                        BoundType::SimpleLb
                    } else if self.bounds[col].simple_ub_dist < self.bounds[col].simple_lb_dist - feastol {
                        BoundType::SimpleUb
                    } else if vals[i] > 0.0 {
                        BoundType::SimpleLb
                    } else {
                        BoundType::SimpleUb
                    };
                    i += 1;
                    continue;
                }
                self.bounds[col].bound_type = if self.best_vlb[col].0.is_none() || self.bounds[col].ub_dist < self.bounds[col].lb_dist - feastol {
                    BoundType::VariableUb
                } else if self.best_vub[col].0.is_none() || self.bounds[col].lb_dist < self.bounds[col].ub_dist - feastol {
                    BoundType::VariableLb
                } else if vals[i] > 0.0 {
                    BoundType::VariableUb
                } else {
                    BoundType::VariableLb
                };
            } else {
                self.bounds[col].bound_type = if self.bounds[col].lb_dist < self.bounds[col].ub_dist - feastol {
                    if self.best_vlb[col].0.is_none() {
                        BoundType::SimpleLb
                    } else if prefer_vbds || vals[i] > 0.0 || self.bounds[col].simple_lb_dist > self.bounds[col].lb_dist + feastol {
                        BoundType::VariableLb
                    } else {
                        BoundType::SimpleLb
                    }
                } else if self.bounds[col].ub_dist < self.bounds[col].lb_dist - feastol {
                    if self.best_vub[col].0.is_none() {
                        BoundType::SimpleUb
                    } else if prefer_vbds || vals[i] < 0.0 || self.bounds[col].simple_ub_dist > self.bounds[col].ub_dist + feastol {
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
            match self.bounds[col].bound_type {
                BoundType::SimpleLb => {
                    if vals[i] > 0.0 {
                        tmp_rhs -= lb * vals[i];
                        self.bounds[col].bound_type = old_bound_type;
                        remove(inds, vals, &mut num_nz, i);
                        continue;
                    }
                }
                BoundType::SimpleUb => {
                    if vals[i] < 0.0 {
                        tmp_rhs -= ub * vals[i];
                        self.bounds[col].bound_type = old_bound_type;
                        remove(inds, vals, &mut num_nz, i);
                        continue;
                    }
                }
                BoundType::VariableLb => {
                    let (vlb_col, vlb) = self.best_vlb[col];
                    tmp_rhs -= vlb.constant * vals[i];
                    self.vectorsum.add(vlb_col.expect("a variable lower bound"), vals[i] * vlb.coef);
                    if vals[i] > 0.0 {
                        self.bounds[col].bound_type = old_bound_type;
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
                        self.bounds[col].bound_type = old_bound_type;
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
            // Negligible against the column's bounds (HiGHS #3223).
            self.vectorsum.cleanup(|col, val| super::domain::term_is_zero(val, get_lb(col), get_ub(col), data.epsilon));
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
            if !self.bounds[col].integral {
                continue;
            }
            if matches!(self.bounds[col].bound_type, BoundType::VariableLb | BoundType::VariableUb) {
                continue;
            }
            self.bounds[col].bound_type = if *integers_positive {
                if (lb != -INF && vals[j] > 0.0) || ub == INF {
                    BoundType::SimpleLb
                } else {
                    BoundType::SimpleUb
                }
            } else if self.bounds[col].lb_dist < self.bounds[col].ub_dist {
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
            match self.bounds[col].bound_type {
                BoundType::SimpleLb => {
                    tmp_rhs -= lb * vals[j];
                    solval[j] = self.bounds[col].lb_dist;
                }
                BoundType::SimpleUb => {
                    tmp_rhs -= ub * vals[j];
                    vals[j] = -vals[j];
                    solval[j] = self.bounds[col].ub_dist;
                }
                BoundType::VariableLb => solval[j] = self.bounds[col].lb_dist,
                BoundType::VariableUb => solval[j] = self.bounds[col].ub_dist,
            }
            if self.bounds[col].integral {
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
            match self.bounds[col].bound_type {
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
            let feastol = data.feastol;
            self.vectorsum.cleanup(|col, val| {
                if super::domain::term_is_zero(val, globaldom.col_lower[col], globaldom.col_upper[col], data.epsilon) {
                    return true;
                }
                let absval = val.abs();
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
