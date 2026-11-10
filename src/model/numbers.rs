//! Number formatting and rounding shared by labels, scales and axes.

/// Format a magnitude with thousands separators and a fixed number of decimals.
pub(crate) fn format_quantity(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "-".to_owned();
    }
    let rendered = format!("{value:.decimals$}");
    let (sign, digits) = rendered.strip_prefix('-').map_or(("", rendered.as_str()), |rest| ("-", rest));
    let (integer, fraction) = digits.split_once('.').map_or((digits, ""), |(integer, fraction)| (integer, fraction));
    let mut grouped = String::with_capacity(integer.len() + integer.len() / 3);
    for (index, character) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(character);
    }
    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{fraction}")
    }
}

/// The scales and grid intervals drawings conventionally use: 1 / 2 / 2.5 / 5 × 10ⁿ.
pub(crate) const DRAWING_SCALE_STEPS: [f64; 5] = [1.0, 2.0, 2.5, 5.0, 10.0];

/// Rounds `value` up to the next entry in ascending `steps` scaled by a power of ten; non-finite or non-positive input returns 1.0.
pub(crate) fn round_up_to_series(value: f64, steps: &[f64]) -> f64 {
    if !(value.is_finite() && value > 0.0) {
        return 1.0;
    }
    let magnitude = 10f64.powf(value.log10().floor());
    let normalised = value / magnitude;
    let step = steps
        .iter()
        .copied()
        .find(|step| normalised <= *step + 1.0e-9)
        .unwrap_or_else(|| steps.last().copied().unwrap_or(10.0));
    step * magnitude
}
