//! HiGHS's `HighsLinearSumBounds`: bounds on linear sums (row activities,
//! or the dual activities of columns), kept current as variable bounds
//! change, with infinite contributions counted rather than summed.
//!
//! Each sum has two pairs of bounds: "orig" ones from the variables' own
//! bounds, and ones that also use their implied bounds, except an
//! implied bound that came from the sum itself.

#![allow(clippy::too_many_arguments, reason = "HiGHS's residual queries, argument for argument")]
use super::NONE;
use crate::util::CDouble;

/// The variable bounds a set of sums is over, which HiGHS keeps as
/// pointers into presolve's arrays.
#[derive(Clone, Copy)]
pub(crate) struct Bounds<'a> {
    pub lower: &'a [f64],
    pub upper: &'a [f64],
    pub impl_lower: &'a [f64],
    pub impl_upper: &'a [f64],
    pub impl_lower_source: &'a [usize],
    pub impl_upper_source: &'a [usize],
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SumBounds {
    sum_lower_orig: Vec<CDouble>,
    sum_upper_orig: Vec<CDouble>,
    num_inf_sum_lower_orig: Vec<i32>,
    num_inf_sum_upper_orig: Vec<i32>,
    sum_lower: Vec<CDouble>,
    sum_upper: Vec<CDouble>,
    num_inf_sum_lower: Vec<i32>,
    num_inf_sum_upper: Vec<i32>,
}

impl SumBounds {
    pub fn set_num_sums(&mut self, num_sums: usize) {
        self.num_inf_sum_lower.resize(num_sums, 0);
        self.num_inf_sum_upper.resize(num_sums, 0);
        self.sum_lower.resize(num_sums, CDouble::default());
        self.sum_upper.resize(num_sums, CDouble::default());
        self.num_inf_sum_lower_orig.resize(num_sums, 0);
        self.num_inf_sum_upper_orig.resize(num_sums, 0);
        self.sum_lower_orig.resize(num_sums, CDouble::default());
        self.sum_upper_orig.resize(num_sums, CDouble::default());
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn add(&mut self, b: Bounds, sum: usize, var: usize, coefficient: f64) {
        self.handle_var_upper(sum, coefficient, b.upper[var], 1);
        self.handle_var_lower(sum, coefficient, b.lower[var], 1);
        self.handle_impl_var_upper(sum, coefficient, impl_var_upper(b, sum, var), 1);
        self.handle_impl_var_lower(sum, coefficient, impl_var_lower(b, sum, var), 1);
    }

    pub fn remove(&mut self, b: Bounds, sum: usize, var: usize, coefficient: f64) {
        self.handle_var_upper(sum, coefficient, b.upper[var], -1);
        self.handle_var_lower(sum, coefficient, b.lower[var], -1);
        self.handle_impl_var_upper(sum, coefficient, impl_var_upper(b, sum, var), -1);
        self.handle_impl_var_lower(sum, coefficient, impl_var_lower(b, sum, var), -1);
    }

    pub fn updated_var_upper(&mut self, b: Bounds, sum: usize, var: usize, coefficient: f64, old_var_upper: f64) {
        self.handle_var_upper(sum, coefficient, old_var_upper, -1);
        self.handle_var_upper(sum, coefficient, b.upper[var], 1);
        self.updated_impl_var_upper_full(b, sum, var, coefficient, old_var_upper, b.impl_upper[var], b.impl_upper_source[var]);
    }

    pub fn updated_var_lower(&mut self, b: Bounds, sum: usize, var: usize, coefficient: f64, old_var_lower: f64) {
        self.handle_var_lower(sum, coefficient, old_var_lower, -1);
        self.handle_var_lower(sum, coefficient, b.lower[var], 1);
        self.updated_impl_var_lower_full(b, sum, var, coefficient, old_var_lower, b.impl_lower[var], b.impl_lower_source[var]);
    }

    pub fn updated_impl_var_upper(&mut self, b: Bounds, sum: usize, var: usize, coefficient: f64, old_impl_var_upper: f64, old_impl_var_upper_source: usize) {
        self.updated_impl_var_upper_full(b, sum, var, coefficient, b.upper[var], old_impl_var_upper, old_impl_var_upper_source);
    }

    pub fn updated_impl_var_lower(&mut self, b: Bounds, sum: usize, var: usize, coefficient: f64, old_impl_var_lower: f64, old_impl_var_lower_source: usize) {
        self.updated_impl_var_lower_full(b, sum, var, coefficient, b.lower[var], old_impl_var_lower, old_impl_var_lower_source);
    }

    /// The lower bound on the sum without `var`'s term, with `bound_var`'s
    /// bound replaced by `bound_var_value` if there is a `bound_var`.
    pub fn residual_sum_lower(&self, b: Bounds, sum: usize, var: usize, coefficient: f64, bound_var: usize, bound_var_coefficient: f64, bound_var_value: f64) -> f64 {
        let mut activity = self.sum_lower[sum];
        let mut num_infs = self.num_inf_sum_lower[sum];
        let var_bound = if coefficient > 0.0 { impl_var_lower(b, sum, var) } else { impl_var_upper(b, sum, var) };
        let old_bound_var_bound = if bound_var != NONE {
            if bound_var_coefficient > 0.0 {
                impl_var_lower(b, sum, bound_var)
            } else {
                impl_var_upper(b, sum, bound_var)
            }
        } else {
            f64::INFINITY
        };
        residual(
            &mut num_infs,
            &mut activity,
            var_bound,
            coefficient,
            bound_var,
            bound_var_coefficient,
            old_bound_var_bound,
            bound_var_value,
        );
        if num_infs == 0 { activity.value() } else { f64::NEG_INFINITY }
    }

    pub fn residual_sum_upper(&self, b: Bounds, sum: usize, var: usize, coefficient: f64, bound_var: usize, bound_var_coefficient: f64, bound_var_value: f64) -> f64 {
        let mut activity = self.sum_upper[sum];
        let mut num_infs = self.num_inf_sum_upper[sum];
        let var_bound = if coefficient < 0.0 { impl_var_lower(b, sum, var) } else { impl_var_upper(b, sum, var) };
        let old_bound_var_bound = if bound_var != NONE {
            if bound_var_coefficient < 0.0 {
                impl_var_lower(b, sum, bound_var)
            } else {
                impl_var_upper(b, sum, bound_var)
            }
        } else {
            f64::INFINITY
        };
        residual(
            &mut num_infs,
            &mut activity,
            var_bound,
            coefficient,
            bound_var,
            bound_var_coefficient,
            old_bound_var_bound,
            bound_var_value,
        );
        if num_infs == 0 { activity.value() } else { f64::INFINITY }
    }

    pub fn residual_sum_lower_orig(&self, b: Bounds, sum: usize, var: usize, coefficient: f64, bound_var: usize, bound_var_coefficient: f64, bound_var_value: f64) -> f64 {
        let mut activity = self.sum_lower_orig[sum];
        let mut num_infs = self.num_inf_sum_lower_orig[sum];
        let var_bound = if coefficient > 0.0 { b.lower[var] } else { b.upper[var] };
        let old_bound_var_bound = if bound_var != NONE {
            if bound_var_coefficient > 0.0 { b.lower[bound_var] } else { b.upper[bound_var] }
        } else {
            f64::INFINITY
        };
        residual(
            &mut num_infs,
            &mut activity,
            var_bound,
            coefficient,
            bound_var,
            bound_var_coefficient,
            old_bound_var_bound,
            bound_var_value,
        );
        if num_infs == 0 { activity.value() } else { f64::NEG_INFINITY }
    }

    pub fn residual_sum_upper_orig(&self, b: Bounds, sum: usize, var: usize, coefficient: f64, bound_var: usize, bound_var_coefficient: f64, bound_var_value: f64) -> f64 {
        let mut activity = self.sum_upper_orig[sum];
        let mut num_infs = self.num_inf_sum_upper_orig[sum];
        let var_bound = if coefficient < 0.0 { b.lower[var] } else { b.upper[var] };
        let old_bound_var_bound = if bound_var != NONE {
            if bound_var_coefficient < 0.0 { b.lower[bound_var] } else { b.upper[bound_var] }
        } else {
            f64::INFINITY
        };
        residual(
            &mut num_infs,
            &mut activity,
            var_bound,
            coefficient,
            bound_var,
            bound_var_coefficient,
            old_bound_var_bound,
            bound_var_value,
        );
        if num_infs == 0 { activity.value() } else { f64::INFINITY }
    }

    pub fn sum_lower_orig(&self, sum: usize, offset: CDouble) -> f64 {
        if self.num_inf_sum_lower_orig[sum] == 0 {
            (self.sum_lower_orig[sum] + offset).value()
        } else {
            f64::NEG_INFINITY
        }
    }

    pub fn sum_upper_orig(&self, sum: usize, offset: CDouble) -> f64 {
        if self.num_inf_sum_upper_orig[sum] == 0 {
            (self.sum_upper_orig[sum] + offset).value()
        } else {
            f64::INFINITY
        }
    }

    pub fn sum_lower(&self, sum: usize, offset: CDouble) -> f64 {
        if self.num_inf_sum_lower[sum] == 0 {
            (self.sum_lower[sum] + offset).value()
        } else {
            f64::NEG_INFINITY
        }
    }

    pub fn sum_upper(&self, sum: usize, offset: CDouble) -> f64 {
        if self.num_inf_sum_upper[sum] == 0 {
            (self.sum_upper[sum] + offset).value()
        } else {
            f64::INFINITY
        }
    }

    pub fn num_inf_sum_lower(&self, sum: usize) -> i32 {
        self.num_inf_sum_lower[sum]
    }

    pub fn num_inf_sum_upper(&self, sum: usize) -> i32 {
        self.num_inf_sum_upper[sum]
    }

    /// HiGHS's `sumScaled`: the sum's activity bounds after scaling its
    /// row by `scale`.
    pub fn sum_scaled(&mut self, sum: usize, scale: f64) {
        self.sum_lower_orig[sum] *= scale;
        self.sum_upper_orig[sum] *= scale;
        self.sum_lower[sum] *= scale;
        self.sum_upper[sum] *= scale;
        if scale < 0.0 {
            let (lower, upper) = (self.sum_lower[sum], self.sum_upper[sum]);
            self.sum_lower[sum] = upper;
            self.sum_upper[sum] = lower;
            let (lower, upper) = (self.sum_lower_orig[sum], self.sum_upper_orig[sum]);
            self.sum_lower_orig[sum] = upper;
            self.sum_upper_orig[sum] = lower;
            std::mem::swap(&mut self.num_inf_sum_lower[sum], &mut self.num_inf_sum_upper[sum]);
            std::mem::swap(&mut self.num_inf_sum_lower_orig[sum], &mut self.num_inf_sum_upper_orig[sum]);
        }
    }

    pub fn num_inf_sum_lower_orig(&self, sum: usize) -> i32 {
        self.num_inf_sum_lower_orig[sum]
    }

    pub fn num_inf_sum_upper_orig(&self, sum: usize) -> i32 {
        self.num_inf_sum_upper_orig[sum]
    }

    /// Move each sum to `new_index` (`NONE` drops it) and keep `new_size`.
    pub fn shrink(&mut self, new_index: &[usize], new_size: usize) {
        for (i, &to) in new_index.iter().enumerate() {
            if to != NONE {
                self.sum_lower[to] = self.sum_lower[i];
                self.sum_upper[to] = self.sum_upper[i];
                self.num_inf_sum_lower[to] = self.num_inf_sum_lower[i];
                self.num_inf_sum_upper[to] = self.num_inf_sum_upper[i];
                self.sum_lower_orig[to] = self.sum_lower_orig[i];
                self.sum_upper_orig[to] = self.sum_upper_orig[i];
                self.num_inf_sum_lower_orig[to] = self.num_inf_sum_lower_orig[i];
                self.num_inf_sum_upper_orig[to] = self.num_inf_sum_upper_orig[i];
            }
        }
        self.set_num_sums(new_size);
        self.sum_lower.truncate(new_size);
        self.sum_upper.truncate(new_size);
        self.num_inf_sum_lower.truncate(new_size);
        self.num_inf_sum_upper.truncate(new_size);
        self.sum_lower_orig.truncate(new_size);
        self.sum_upper_orig.truncate(new_size);
        self.num_inf_sum_lower_orig.truncate(new_size);
        self.num_inf_sum_upper_orig.truncate(new_size);
    }

    fn handle_var_upper(&mut self, sum: usize, coefficient: f64, var_upper: f64, direction: i32) {
        if coefficient > 0.0 {
            update(&mut self.num_inf_sum_upper_orig[sum], &mut self.sum_upper_orig[sum], direction, var_upper, coefficient);
        } else {
            update(&mut self.num_inf_sum_lower_orig[sum], &mut self.sum_lower_orig[sum], direction, var_upper, coefficient);
        }
    }

    fn handle_var_lower(&mut self, sum: usize, coefficient: f64, var_lower: f64, direction: i32) {
        if coefficient > 0.0 {
            update(&mut self.num_inf_sum_lower_orig[sum], &mut self.sum_lower_orig[sum], direction, var_lower, coefficient);
        } else {
            update(&mut self.num_inf_sum_upper_orig[sum], &mut self.sum_upper_orig[sum], direction, var_lower, coefficient);
        }
    }

    fn handle_impl_var_upper(&mut self, sum: usize, coefficient: f64, impl_var_upper: f64, direction: i32) {
        if coefficient > 0.0 {
            update(&mut self.num_inf_sum_upper[sum], &mut self.sum_upper[sum], direction, impl_var_upper, coefficient);
        } else {
            update(&mut self.num_inf_sum_lower[sum], &mut self.sum_lower[sum], direction, impl_var_upper, coefficient);
        }
    }

    fn handle_impl_var_lower(&mut self, sum: usize, coefficient: f64, impl_var_lower: f64, direction: i32) {
        if coefficient > 0.0 {
            update(&mut self.num_inf_sum_lower[sum], &mut self.sum_lower[sum], direction, impl_var_lower, coefficient);
        } else {
            update(&mut self.num_inf_sum_upper[sum], &mut self.sum_upper[sum], direction, impl_var_lower, coefficient);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn updated_impl_var_upper_full(&mut self, b: Bounds, sum: usize, var: usize, coefficient: f64, old_var_upper: f64, old_impl_var_upper: f64, old_impl_var_upper_source: usize) {
        let old_v_upper = impl_bound_upper(sum, old_var_upper, old_impl_var_upper, old_impl_var_upper_source);
        let v_upper = impl_var_upper(b, sum, var);
        if v_upper == old_v_upper {
            return;
        }
        self.handle_impl_var_upper(sum, coefficient, old_v_upper, -1);
        self.handle_impl_var_upper(sum, coefficient, v_upper, 1);
    }

    #[allow(clippy::too_many_arguments)]
    fn updated_impl_var_lower_full(&mut self, b: Bounds, sum: usize, var: usize, coefficient: f64, old_var_lower: f64, old_impl_var_lower: f64, old_impl_var_lower_source: usize) {
        let old_v_lower = impl_bound_lower(sum, old_var_lower, old_impl_var_lower, old_impl_var_lower_source);
        let v_lower = impl_var_lower(b, sum, var);
        if v_lower == old_v_lower {
            return;
        }
        self.handle_impl_var_lower(sum, coefficient, old_v_lower, -1);
        self.handle_impl_var_lower(sum, coefficient, v_lower, 1);
    }
}

/// The upper bound `sum` may use for `var`: its implied bound too, unless
/// the sum implied it.
pub(crate) fn impl_var_upper(b: Bounds, sum: usize, var: usize) -> f64 {
    impl_bound_upper(sum, b.upper[var], b.impl_upper[var], b.impl_upper_source[var])
}

pub(crate) fn impl_var_lower(b: Bounds, sum: usize, var: usize) -> f64 {
    impl_bound_lower(sum, b.lower[var], b.impl_lower[var], b.impl_lower_source[var])
}

fn impl_bound_upper(sum: usize, var_upper: f64, impl_var_upper: f64, impl_var_upper_source: usize) -> f64 {
    if impl_var_upper_source == sum { var_upper } else { impl_var_upper.min(var_upper) }
}

fn impl_bound_lower(sum: usize, var_lower: f64, impl_var_lower: f64, impl_var_lower_source: usize) -> f64 {
    if impl_var_lower_source == sum { var_lower } else { impl_var_lower.max(var_lower) }
}

fn update(num_infs: &mut i32, activity: &mut CDouble, direction: i32, bound: f64, coefficient: f64) {
    if bound.abs() != f64::INFINITY {
        *activity += CDouble::new(direction as f64 * bound) * coefficient;
    } else {
        *num_infs += direction;
    }
}

fn update_bound(num_infs: &mut i32, activity: &mut CDouble, old_bound: f64, new_bound: f64, coefficient: f64) {
    if old_bound == new_bound {
        return;
    }
    update(num_infs, activity, -1, old_bound, coefficient);
    update(num_infs, activity, 1, new_bound, coefficient);
}

#[allow(clippy::too_many_arguments)]
fn residual(
    num_infs: &mut i32,
    activity: &mut CDouble,
    var_bound: f64,
    coefficient: f64,
    bound_var: usize,
    bound_var_coefficient: f64,
    old_bound_var_bound: f64,
    new_bound_var_bound: f64,
) {
    update(num_infs, activity, -1, var_bound, coefficient);
    if bound_var != NONE {
        update_bound(num_infs, activity, old_bound_var_bound, new_bound_var_bound, bound_var_coefficient);
    }
}
