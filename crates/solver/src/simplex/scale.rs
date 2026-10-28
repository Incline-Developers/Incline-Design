//! HiGHS's `scaleLp`: with equilibration, powers of two for the rows and
//! columns that bring each one's largest and smallest entries about one,
//! kept (unless forced) only when they improve the matrix; with max value,
//! powers of two bringing each row's and then column's largest entry
//! about one.

use super::{
    ekk::SimplexLp,
    nla::Scale,
    options::{Options, SCALE_STRATEGY_CHOOSE, SCALE_STRATEGY_EQUILIBRATION, SCALE_STRATEGY_FORCED_EQUILIBRATION},
};

/// Scale `lp` in place, returning the factors, or leave it alone; also
/// returns the strategy used (HiGHS's `scale_.strategy`).
pub(crate) fn scale_lp(options: &Options, lp: &mut SimplexLp) -> (Option<Scale>, i32) {
    let num_col = lp.num_col;
    let num_row = lp.num_row;

    // Determine the actual strategy to use: HiGHS left to choose uses
    // forced equilibration.
    let mut use_scale_strategy = options.simplex_scale_strategy;
    if use_scale_strategy == SCALE_STRATEGY_CHOOSE {
        use_scale_strategy = SCALE_STRATEGY_FORCED_EQUILIBRATION;
    }
    (scale_lp_with(options, lp, num_col, num_row, use_scale_strategy), use_scale_strategy)
}

fn scale_lp_with(options: &Options, lp: &mut SimplexLp, num_col: usize, num_row: usize, use_scale_strategy: i32) -> Option<Scale> {
    if num_col == 0 || num_row == 0 {
        return None;
    }
    let (min_value, max_value) =
        lp.a.value
            .iter()
            .fold((f64::INFINITY, 0.0f64), |(min, max), value| (min.min(value.abs()), max.max(value.abs())));
    if min_value >= 0.2 && max_value <= 5.0 {
        return None;
    }
    let mut scale = Scale {
        col: vec![1.0; num_col],
        row: vec![1.0; num_row],
    };
    let equilibration_scaling = use_scale_strategy == SCALE_STRATEGY_EQUILIBRATION || use_scale_strategy == SCALE_STRATEGY_FORCED_EQUILIBRATION;
    let scaled_matrix = if equilibration_scaling {
        equilibration_scale_matrix(options, lp, &mut scale, min_value, max_value, use_scale_strategy)
    } else {
        max_value_scale_matrix(options, lp, &mut scale)
    };
    if !scaled_matrix {
        return None;
    }
    for col in 0..num_col {
        lp.col_lower[col] /= scale.col[col];
        lp.col_upper[col] /= scale.col[col];
        lp.col_cost[col] *= scale.col[col];
    }
    for row in 0..num_row {
        lp.row_lower[row] *= scale.row[row];
        lp.row_upper[row] *= scale.row[row];
    }
    Some(scale)
}

fn equilibration_scale_matrix(
    options: &Options,
    lp: &mut SimplexLp,
    scale: &mut Scale,
    original_matrix_min_value: f64,
    original_matrix_max_value: f64,
    use_scale_strategy: i32,
) -> bool {
    let num_col = lp.num_col;
    let num_row = lp.num_row;
    let start = &lp.a.start;
    let index = &lp.a.index;
    let col_cost = &lp.col_cost;

    let min_nonzero_cost = col_cost.iter().filter(|cost| **cost != 0.0).fold(f64::INFINITY, |min, cost| min.min(cost.abs()));
    let include_cost_in_scaling = min_nonzero_cost < 0.1;

    let finite_infinity: f64 = 1e200;
    let max_allow_scale = 2f64.powi(options.allowed_matrix_scale_factor);
    let min_allow_scale = 1.0 / max_allow_scale;

    let mut row_min_value = vec![finite_infinity; num_row];
    let mut row_max_value = vec![1.0 / finite_infinity; num_row];
    let abs_value: Vec<f64> = lp.a.value.iter().map(|value| value.abs()).collect();
    for _ in 0..6 {
        for col in 0..num_col {
            let mut col_min_value = finite_infinity;
            let mut col_max_value = 1.0 / finite_infinity;
            let abs_col_cost = col_cost[col].abs();
            if include_cost_in_scaling && abs_col_cost != 0.0 {
                col_min_value = col_min_value.min(abs_col_cost);
                col_max_value = col_max_value.max(abs_col_cost);
            }
            let rows = &index[start[col]..start[col + 1]];
            let values = &abs_value[start[col]..start[col + 1]];
            for (&row, &value) in rows.iter().zip(values) {
                let value = value * scale.row[row];
                col_min_value = col_min_value.min(value);
                col_max_value = col_max_value.max(value);
            }
            let col_equilibration = 1.0 / (col_min_value * col_max_value).sqrt();
            let col_scale = col_equilibration.max(min_allow_scale).min(max_allow_scale);
            scale.col[col] = col_scale;
            for (&row, &value) in rows.iter().zip(values) {
                let value = value * col_scale;
                row_min_value[row] = row_min_value[row].min(value);
                row_max_value[row] = row_max_value[row].max(value);
            }
        }
        for row in 0..num_row {
            let row_equilibration = 1.0 / (row_min_value[row] * row_max_value[row]).sqrt();
            scale.row[row] = row_equilibration.max(min_allow_scale).min(max_allow_scale);
        }
        row_min_value.fill(finite_infinity);
        row_max_value.fill(1.0 / finite_infinity);
    }
    for value in scale.col.iter_mut().chain(scale.row.iter_mut()) {
        *value = nearest_power_of_two(*value);
    }

    let mut matrix_min_value = finite_infinity;
    let mut matrix_max_value: f64 = 0.0;
    // Each column's and row's equilibration before and after, for the
    // geometric means below.
    let mut original_col_equilibrations = Vec::with_capacity(num_col);
    let mut col_equilibrations = Vec::with_capacity(num_col);
    let mut original_row_equilibrations = Vec::with_capacity(num_row);
    let mut row_equilibrations = Vec::with_capacity(num_row);
    let mut original_row_min_value = vec![finite_infinity; num_row];
    let mut original_row_max_value = vec![1.0 / finite_infinity; num_row];
    row_min_value.fill(finite_infinity);
    row_max_value.fill(1.0 / finite_infinity);
    for col in 0..num_col {
        let mut original_col_min_value = finite_infinity;
        let mut original_col_max_value = 1.0 / finite_infinity;
        let mut col_min_value = finite_infinity;
        let mut col_max_value = 1.0 / finite_infinity;
        for k in start[col]..start[col + 1] {
            let row = index[k];
            let original_value = lp.a.value[k].abs();
            original_col_min_value = original_col_min_value.min(original_value);
            original_col_max_value = original_col_max_value.max(original_value);
            original_row_min_value[row] = original_row_min_value[row].min(original_value);
            original_row_max_value[row] = original_row_max_value[row].max(original_value);
            lp.a.value[k] *= scale.col[col] * scale.row[row];
            let value = lp.a.value[k].abs();
            col_min_value = col_min_value.min(value);
            col_max_value = col_max_value.max(value);
            row_min_value[row] = row_min_value[row].min(value);
            row_max_value[row] = row_max_value[row].max(value);
        }
        matrix_min_value = matrix_min_value.min(col_min_value);
        matrix_max_value = matrix_max_value.max(col_max_value);
        original_col_equilibrations.push(1.0 / (original_col_min_value * original_col_max_value).sqrt());
        col_equilibrations.push(1.0 / (col_min_value * col_max_value).sqrt());
    }
    for row in 0..num_row {
        original_row_equilibrations.push(1.0 / (original_row_min_value[row] * original_row_max_value[row]).sqrt());
        row_equilibrations.push(1.0 / (row_min_value[row] * row_max_value[row]).sqrt());
    }
    let original_col = Spread::of(&original_col_equilibrations);
    let original_row = Spread::of(&original_row_equilibrations);
    let col = Spread::of(&col_equilibrations);
    let row = Spread::of(&row_equilibrations);
    let extreme_equilibration_improvement = (original_col.ratio() + original_row.ratio()) / (col.ratio() + row.ratio());
    let matrix_value_ratio = matrix_max_value / matrix_min_value;
    let original_matrix_value_ratio = original_matrix_max_value / original_matrix_min_value;
    let matrix_value_ratio_improvement = original_matrix_value_ratio / matrix_value_ratio;
    // The geometric means take a logarithm per column and row. Each lies
    // within its values' range, which often settles the question alone.
    let known = extreme_equilibration_improvement * matrix_value_ratio_improvement;
    let least = known * ((original_col.least_geomean() * original_row.least_geomean()) / (col.most_geomean() * row.most_geomean())).sqrt();
    let most = known * ((original_col.most_geomean() * original_row.most_geomean()) / (col.least_geomean() * row.least_geomean())).sqrt();
    let improves = if least >= 1.0 + 1e-9 {
        true
    } else if most < 1.0 - 1e-9 {
        false
    } else {
        let geomean = |values: &[f64]| {
            let sum: f64 = values.iter().fold(0.0, |sum, value| sum + value.ln());
            let mean = (sum / values.len() as f64).exp();
            mean.max(1.0 / mean)
        };
        let mean_equilibration_improvement = ((geomean(&original_col_equilibrations) * geomean(&original_row_equilibrations))
            / (geomean(&col_equilibrations) * geomean(&row_equilibrations)))
        .sqrt();
        let improvement_factor = extreme_equilibration_improvement * mean_equilibration_improvement * matrix_value_ratio_improvement;
        !(improvement_factor < 1.0)
    };
    let possibly_abandon_scaling = use_scale_strategy != SCALE_STRATEGY_FORCED_EQUILIBRATION;
    if possibly_abandon_scaling && !improves {
        // Not worth it: undo.
        for col in 0..num_col {
            for k in start[col]..start[col + 1] {
                lp.a.value[k] /= scale.col[col] * scale.row[index[k]];
            }
        }
        return false;
    }
    true
}

/// HiGHS's `maxValueScaleMatrix`.
fn max_value_scale_matrix(options: &Options, lp: &mut SimplexLp, scale: &mut Scale) -> bool {
    let num_col = lp.num_col;
    let num_row = lp.num_row;
    let log2 = 2f64.ln();
    let max_allow_scale = 2f64.powi(options.allowed_matrix_scale_factor);
    let min_allow_scale = 1.0 / max_allow_scale;
    let start = &lp.a.start;
    let index = &lp.a.index;

    // Determine the row scaling, and the max/min original matrix values.
    let mut original_matrix_min_value = f64::INFINITY;
    let mut original_matrix_max_value: f64 = 0.0;
    let mut row_max_value = vec![0.0f64; num_row];
    for col in 0..num_col {
        for k in start[col]..start[col + 1] {
            let row = index[k];
            let value = lp.a.value[k].abs();
            row_max_value[row] = row_max_value[row].max(value);
            original_matrix_min_value = original_matrix_min_value.min(value);
            original_matrix_max_value = original_matrix_max_value.max(value);
        }
    }
    for row in 0..num_row {
        if row_max_value[row] != 0.0 {
            // Convert the row scale factor to the nearest power of two,
            // and ensure that it is not excessively large or small.
            let mut row_scale_value = 1.0 / row_max_value[row];
            row_scale_value = 2f64.powf((row_scale_value.ln() / log2 + 0.5).floor());
            row_scale_value = min_allow_scale.max(row_scale_value).min(max_allow_scale);
            scale.row[row] = row_scale_value;
        }
    }

    // Determine the column scaling, whilst applying the row scaling.
    let mut matrix_min_value = f64::INFINITY;
    let mut matrix_max_value: f64 = 0.0;
    for col in 0..num_col {
        let mut col_max_value: f64 = 0.0;
        for k in start[col]..start[col + 1] {
            lp.a.value[k] *= scale.row[index[k]];
            col_max_value = col_max_value.max(lp.a.value[k].abs());
        }
        if col_max_value != 0.0 {
            let mut col_scale_value = 1.0 / col_max_value;
            col_scale_value = 2f64.powf((col_scale_value.ln() / log2 + 0.5).floor());
            col_scale_value = min_allow_scale.max(col_scale_value).min(max_allow_scale);
            scale.col[col] = col_scale_value;
            for value_k in &mut lp.a.value[start[col]..start[col + 1]] {
                *value_k *= scale.col[col];
                let value = value_k.abs();
                matrix_min_value = matrix_min_value.min(value);
                matrix_max_value = matrix_max_value.max(value);
            }
        }
    }
    let matrix_value_ratio = matrix_max_value / matrix_min_value;
    let original_matrix_value_ratio = original_matrix_max_value / original_matrix_min_value;
    let improvement_factor = original_matrix_value_ratio / matrix_value_ratio;
    if improvement_factor <= 1.0 {
        // Not worth it: undo.
        for col in 0..num_col {
            for k in start[col]..start[col + 1] {
                lp.a.value[k] /= scale.col[col] * scale.row[index[k]];
            }
        }
        return false;
    }
    true
}

/// The range of a set of equilibration factors.
struct Spread {
    least: f64,
    most: f64,
}

impl Spread {
    fn of(values: &[f64]) -> Self {
        let mut spread = Spread { least: 1e200, most: 0.0 };
        for &value in values {
            spread.least = spread.least.min(value);
            spread.most = spread.most.max(value);
        }
        spread
    }

    fn ratio(&self) -> f64 {
        self.most / self.least
    }

    /// The least `max(mean, 1 / mean)` can be for a geometric mean within
    /// the range.
    fn least_geomean(&self) -> f64 {
        if self.least >= 1.0 {
            self.least
        } else if self.most <= 1.0 {
            1.0 / self.most
        } else {
            1.0
        }
    }

    /// The most it can be.
    fn most_geomean(&self) -> f64 {
        self.most.max(1.0 / self.least)
    }
}

/// HiGHS's `pow(2.0, floor(log(value) / log(2.0) + 0.5))`: `value` rounded
/// to a power of two, the nearest by its logarithm. Read from its exponent
/// and mantissa, which settle it but within a hair of the halfway mantissa,
/// the square root of 2, where the logarithm's rounding decides.
fn nearest_power_of_two(value: f64) -> f64 {
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i64;
    let mantissa = f64::from_bits((bits & ((1 << 52) - 1)) | (1023 << 52));
    if value <= 0.0 || exponent == 0 || exponent >= 0x7fe || (mantissa - std::f64::consts::SQRT_2).abs() < 1e-9 {
        return 2f64.powf((value.ln() / 2f64.ln() + 0.5).floor());
    }
    let power = exponent + i64::from(mantissa > std::f64::consts::SQRT_2);
    f64::from_bits((power as u64) << 52)
}
