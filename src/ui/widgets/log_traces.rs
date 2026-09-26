//! The well-log trace columns either side of the borehole log's hole track:
//! density (long- and short-spaced, one shared scale) to its left, gamma to
//! its right. Drawn from [`crate::model::geophysics::GeophysicsSession`] at a
//! cost set by the column's height in pixels, never by the trace's length.

use serde::{Deserialize, Serialize};

use crate::{
    i18n::{tr, tr_format},
    model::geophysics::{HoleLogs, LogKind, LogTrace},
    ui::widgets::{
        context_menu::{ContextMenuAction, context_menu_fields, context_menu_separator, context_submenu},
        menu,
    },
};

/// Which of the log's two trace columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TraceColumn {
    /// Long- and short-spaced density, left of the hole.
    Density,
    /// Natural gamma, right of the hole.
    Gamma,
}

impl TraceColumn {
    /// The curves drawn in this column, in drawing order.
    pub(crate) fn kinds(self) -> &'static [LogKind] {
        match self {
            TraceColumn::Density => &[LogKind::LongDensity, LogKind::ShortDensity],
            TraceColumn::Gamma => &[LogKind::Gamma],
        }
    }

    pub(crate) fn unit(self) -> &'static str {
        match self {
            TraceColumn::Density => LogKind::LongDensity.unit(),
            TraceColumn::Gamma => LogKind::Gamma.unit(),
        }
    }

    /// The range a fixed scale starts from before the user picks one.
    pub(crate) fn default_fixed_range(self) -> [f32; 2] {
        match self {
            TraceColumn::Density => [1.0, 3.0],
            TraceColumn::Gamma => [0.0, 150.0],
        }
    }

    /// Floor `tidy_range` widens a flat or near-flat span to, so a trace
    /// reading one value all the way down still gets a usable scale.
    fn min_span(self) -> f32 {
        match self {
            TraceColumn::Density => 0.2,
            TraceColumn::Gamma => 20.0,
        }
    }

    /// Whether a fixed range can be drawn against: finite, increasing, and
    /// no narrower than a hundredth of a g/cc or one API, nor than a
    /// thousandth of its own magnitude, where gridlines and labels break down.
    pub(crate) fn accepts_range(self, [lo, hi]: [f32; 2]) -> bool {
        let floor: f32 = match self {
            TraceColumn::Density => 0.01,
            TraceColumn::Gamma => 1.0,
        };
        let span = hi - lo;
        // A hair of slack, so an f32 edge like 1.51 still counts as 0.01 on.
        span.is_finite() && span >= floor.max(1.0e-3 * lo.abs().max(hi.abs())) * 0.999
    }
}

/// How one trace column's value axis is set.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct TraceScale {
    /// Read the range off the inspected hole's own data.
    pub(crate) auto: bool,
    /// Value at the column's left edge, then at its right, used while `auto`
    /// is off.
    pub(crate) range: [f32; 2],
}

impl Default for TraceScale {
    fn default() -> Self {
        Self { auto: true, range: [0.0, 1.0] }
    }
}

impl TraceScale {
    /// A usable scale: a fixed range `column` cannot draw against falls back
    /// to its default.
    fn sanitized(self, column: TraceColumn) -> Self {
        let range = if column.accepts_range(self.range) { self.range } else { column.default_fixed_range() };
        Self { auto: self.auto, range }
    }
}

/// Colours and scales for the log's trace columns. App-wide rather than per
/// hole, so every hole reads against the same choices; saved with the
/// preferences.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct WellLogStyle {
    /// Unmultiplied sRGBA bytes, so the file reads as the colour picked.
    pub(crate) gamma_color: [u8; 4],
    pub(crate) long_density_color: [u8; 4],
    pub(crate) short_density_color: [u8; 4],
    pub(crate) density_scale: TraceScale,
    pub(crate) gamma_scale: TraceScale,
}

impl Default for WellLogStyle {
    fn default() -> Self {
        // Mid-tone and saturated, so each reads on the dark theme's panel
        // and the light one's alike; the two densities share a column, so
        // they sit far apart in hue.
        Self {
            gamma_color: [0x2E, 0xA0, 0x4F, 0xFF],
            long_density_color: [0xE0, 0x4A, 0x3C, 0xFF],
            short_density_color: [0x3A, 0x86, 0xD8, 0xFF],
            density_scale: TraceScale {
                auto: true,
                range: TraceColumn::Density.default_fixed_range(),
            },
            gamma_scale: TraceScale {
                auto: true,
                range: TraceColumn::Gamma.default_fixed_range(),
            },
        }
    }
}

impl WellLogStyle {
    pub(crate) fn color(&self, kind: LogKind) -> [u8; 4] {
        match kind {
            LogKind::Gamma => self.gamma_color,
            LogKind::LongDensity => self.long_density_color,
            LogKind::ShortDensity => self.short_density_color,
        }
    }

    pub(crate) fn set_color(&mut self, kind: LogKind, color: [u8; 4]) {
        match kind {
            LogKind::Gamma => self.gamma_color = color,
            LogKind::LongDensity => self.long_density_color = color,
            LogKind::ShortDensity => self.short_density_color = color,
        }
    }

    pub(crate) fn scale(&self, column: TraceColumn) -> TraceScale {
        match column {
            TraceColumn::Density => self.density_scale,
            TraceColumn::Gamma => self.gamma_scale,
        }
    }

    pub(crate) fn set_scale(&mut self, column: TraceColumn, scale: TraceScale) {
        match column {
            TraceColumn::Density => self.density_scale = scale,
            TraceColumn::Gamma => self.gamma_scale = scale,
        }
    }

    /// The style with anything unusable replaced: a hand-edited config may
    /// hold a fixed range that is empty, reversed or not a number.
    pub(crate) fn sanitized(self) -> Self {
        Self {
            density_scale: self.density_scale.sanitized(TraceColumn::Density),
            gamma_scale: self.gamma_scale.sanitized(TraceColumn::Gamma),
            ..self
        }
    }
}

// --- Drawing --------------------------------------------------------------
//
// Everything below draws one column: its header, its plot body, and its
// right-click scale menu. Geometry is built as plain data (`TraceGeometry`)
// before any painting, so the layout math is testable without an egui
// context; painting itself just walks that data.

/// Off-scale side of a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Clip {
    Below,
    Inside,
    Above,
}

/// The traces one column draws for the inspected hole, in
/// [`TraceColumn::kinds`] order.
#[derive(Clone, Copy)]
pub(crate) struct ColumnTraces<'a> {
    pub(crate) column: TraceColumn,
    traces: [Option<&'a LogTrace>; 2],
    /// The curves the column shows: those read, or while the hole is read,
    /// those its index says it has.
    shown: [bool; 2],
}

impl<'a> ColumnTraces<'a> {
    /// `None` when the hole holds none of the column's curves.
    pub(crate) fn of(column: TraceColumn, hole: Option<&'a HoleLogs>) -> Option<Self> {
        let mut traces = [None; 2];
        for (slot, &kind) in traces.iter_mut().zip(column.kinds()) {
            *slot = hole.and_then(|hole| hole.trace(kind));
        }
        let shown = traces.map(|trace| trace.is_some());
        shown.contains(&true).then_some(Self { column, traces, shown })
    }

    /// The column of a hole still being read, laid out for the curves it
    /// has, by [`LogKind::index`].
    pub(crate) fn reading(column: TraceColumn, kinds: [bool; 3]) -> Option<Self> {
        let mut shown = [false; 2];
        for (slot, kind) in shown.iter_mut().zip(column.kinds()) {
            *slot = kinds[kind.index()];
        }
        shown.contains(&true).then_some(Self { column, traces: [None; 2], shown })
    }

    /// The curves the column shows, in column order.
    pub(crate) fn curves(&self) -> impl Iterator<Item = LogKind> + '_ {
        self.column.kinds().iter().zip(self.shown).filter_map(|(kind, shown)| shown.then_some(*kind))
    }

    fn is_reading(&self) -> bool {
        self.traces.iter().all(Option::is_none)
    }

    /// Whether the scale is known: an auto one waits for the readings.
    fn has_range(&self, style: &WellLogStyle) -> bool {
        !self.is_reading() || !style.scale(self.column).auto
    }

    /// The present traces, in column order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &'a LogTrace> + '_ {
        self.traces.into_iter().flatten()
    }

    /// The present traces with the curve each is, in column order.
    fn kinded(&self) -> impl Iterator<Item = (LogKind, &'a LogTrace)> + '_ {
        self.column.kinds().iter().zip(self.traces).filter_map(|(kind, trace)| trace.map(|trace| (*kind, trace)))
    }
}

/// The name a curve is shown by.
fn kind_label(kind: LogKind) -> String {
    match kind {
        LogKind::Gamma => tr!(literal = "Gamma"),
        LogKind::LongDensity => tr!(literal = "Long density"),
        LogKind::ShortDensity => tr!(literal = "Short density"),
    }
}

fn to_color32(c: [u8; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// Union of every trace's [`LogTrace::robust_range`] in the column, tidied.
/// Reads the whole hole, so it stays put while the user pans and zooms.
/// `None` when no trace in the column has a robust range yet.
pub(crate) fn auto_range(column: TraceColumn, traces: &ColumnTraces) -> Option<[f32; 2]> {
    let mut acc: Option<(f32, f32)> = None;
    for trace in traces.iter() {
        if let Some((lo, hi)) = trace.robust_range() {
            acc = Some(match acc {
                Some((alo, ahi)) => (alo.min(lo), ahi.max(hi)),
                None => (lo, hi),
            });
        }
    }
    acc.map(|(lo, hi)| tidy_range(column, lo, hi))
}

/// Round `[lo, hi]` outward to a tidy step (see [`grid_step`]) chosen so the
/// span holds about four divisions. A flat or tiny span widens symmetrically
/// first to the column's minimum span; neither column dips below zero.
/// Non-finite input falls back to the column's default fixed range.
pub(crate) fn tidy_range(column: TraceColumn, lo: f32, hi: f32) -> [f32; 2] {
    if !lo.is_finite() || !hi.is_finite() {
        return column.default_fixed_range();
    }
    let (mut lo, mut hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let min_span = column.min_span();
    if hi - lo < min_span {
        let center = 0.5 * (lo + hi);
        lo = center - 0.5 * min_span;
        hi = center + 0.5 * min_span;
    }
    // Neither a density nor a count reads below zero.
    if lo < 0.0 {
        hi -= lo;
        lo = 0.0;
    }
    let step = grid_step([lo, hi]);
    [(lo / step).floor() * step, (hi / step).ceil() * step]
}

/// The tidy step (see [`tidy_range`]) for gridlines over any `[lo, hi]`,
/// fixed ranges included.
pub(crate) fn grid_step(range: [f32; 2]) -> f32 {
    let span = f64::from((range[1] - range[0]).abs());
    crate::model::plot::round_up_to_series(span / 4.0, &crate::model::plot::DRAWING_SCALE_STEPS) as f32
}

/// Decimal places the grid step over `range` calls for, so `1.0`/`3.0` and
/// `0`/`150` each print no more precision than the gridlines show.
fn range_decimals(range: [f32; 2]) -> usize {
    let mut step = f64::from(grid_step(range));
    let mut decimals = 0usize;
    while (step - step.round()).abs() > 1e-6 && decimals < 4 {
        step *= 10.0;
        decimals += 1;
    }
    decimals
}

/// One edge of `range` as written: at least the decimals its gridlines
/// need, and more where the edge itself has them, so a fixed 1.25 reads
/// 1.25 and never rounds to a value the column does not end at.
fn edge_label(value: f32, range: [f32; 2]) -> String {
    let value = f64::from(value);
    // Within what an f32 edge can hold, so 2.8 stored as 2.7999999 reads 2.8.
    let tolerance = value.abs().max(1.0) * 1.0e-6;
    let mut decimals = range_decimals(range);
    while decimals < 4 {
        let scale = 10f64.powi(decimals as i32);
        if ((value * scale).round() / scale - value).abs() <= tolerance {
            break;
        }
        decimals += 1;
    }
    format!("{value:.decimals$}")
}

/// The style's range for `traces`' column: auto reads the hole (falling back
/// to the column's default when the hole has nothing to read yet), fixed
/// reads the saved range as-is.
pub(crate) fn column_range(style: &WellLogStyle, traces: &ColumnTraces) -> [f32; 2] {
    let column = traces.column;
    let scale = style.scale(column);
    if scale.auto {
        auto_range(column, traces).unwrap_or_else(|| column.default_fixed_range())
    } else {
        scale.range
    }
}

/// Linear map of `range` onto `[left, right]`. An off-scale value clamps to
/// the edge it overshot and reports which one; values never wrap around.
///
/// `value` must be finite - callers treat a null/NaN sample as a gap before
/// reaching here. A non-finite input is still well defined rather than
/// propagating NaN: it reports the left edge as [`Clip::Below`].
pub(crate) fn value_x(value: f32, range: [f32; 2], left: f32, right: f32) -> (f32, Clip) {
    let [lo, hi] = range;
    if !value.is_finite() || hi <= lo {
        return (left, Clip::Below);
    }
    let t = (value - lo) / (hi - lo);
    if t < 0.0 {
        (left, Clip::Below)
    } else if t > 1.0 {
        (right, Clip::Above)
    } else {
        (left + t * (right - left), Clip::Inside)
    }
}

/// Small gap around the header's rows.
const HEADER_PADDING: f32 = 4.0;

/// Height of a column's header: one line per curve plus one for the scale.
pub(crate) fn header_height(column: TraceColumn, line_height: f32) -> f32 {
    (column.kinds().len() as f32 + 1.0) * line_height + HEADER_PADDING
}

const SWATCH_WIDTH: f32 = 10.0;
const SWATCH_STROKE: f32 = 2.0;
const SWATCH_GAP: f32 = 4.0;
/// Gap kept either side of the centred unit label before it is dropped.
const UNIT_LABEL_GAP: f32 = 4.0;

/// Draw the column's header: one line per curve present on this hole (a
/// swatch, then its name, both in the curve's colour), then a scale line -
/// range minimum at the left, unit centred, maximum at the right.
pub(crate) fn draw_header(ui: &egui::Ui, painter: &egui::Painter, rect: egui::Rect, traces: &ColumnTraces, range: [f32; 2], style: &WellLogStyle) {
    let painter = painter.with_clip_rect(rect);
    let line_height = ui.text_style_height(&egui::TextStyle::Small);
    let font = egui::TextStyle::Small.resolve(ui.style());
    let mut y = rect.top() + HEADER_PADDING * 0.5;

    for kind in traces.curves() {
        let color = to_color32(style.color(kind));
        let swatch_y = y + line_height * 0.5;
        painter.line_segment(
            [egui::pos2(rect.left(), swatch_y), egui::pos2(rect.left() + SWATCH_WIDTH, swatch_y)],
            egui::Stroke::new(SWATCH_STROKE, color),
        );
        let text_left = rect.left() + SWATCH_WIDTH + SWATCH_GAP;
        let mut job = egui::text::LayoutJob::single_section(
            kind_label(kind),
            egui::TextFormat {
                font_id: font.clone(),
                color,
                ..Default::default()
            },
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((rect.right() - text_left).max(0.0));
        let galley = painter.layout_job(job);
        painter.galley(egui::pos2(text_left, y), galley, egui::Color32::PLACEHOLDER);
        y += line_height;
    }

    let weak = ui.visuals().weak_text_color();
    let (mut left_bound, mut right_bound) = (rect.left(), rect.right());
    if traces.has_range(style) {
        let min_galley = painter.layout_no_wrap(edge_label(range[0], range), font.clone(), weak);
        let min_size = min_galley.size();
        painter.galley(egui::pos2(rect.left(), y), min_galley, weak);
        let max_galley = painter.layout_no_wrap(edge_label(range[1], range), font.clone(), weak);
        let max_size = max_galley.size();
        let max_pos = egui::pos2(rect.right() - max_size.x, y);
        painter.galley(max_pos, max_galley, weak);
        left_bound += min_size.x + UNIT_LABEL_GAP;
        right_bound = max_pos.x - UNIT_LABEL_GAP;
    }
    let unit_galley = painter.layout_no_wrap(traces.column.unit().to_owned(), font, weak);
    let unit_size = unit_galley.size();
    let unit_x = rect.center().x - unit_size.x * 0.5;
    if unit_x >= left_bound && unit_x + unit_size.x <= right_bound {
        painter.galley(egui::pos2(unit_x, y), unit_galley, weak);
    }
}

/// One trace's drawable geometry for one frame, built as plain data so the
/// layout math is testable without an egui context.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct TraceGeometry {
    /// Filled band quads (envelope mode only; empty in exact mode).
    pub(crate) quads: Vec<egui::Rect>,
    /// Polyline runs, broken at gaps: null samples, or `None` buckets.
    pub(crate) lines: Vec<Vec<egui::Pos2>>,
    /// Off-scale runs, merged from consecutive clipped buckets/samples.
    pub(crate) clips: Vec<ClipSpan>,
}

/// One run of consecutive off-scale buckets or samples on the same edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ClipSpan {
    pub(crate) side: Clip,
    pub(crate) y0: f32,
    pub(crate) y1: f32,
}

/// Whether a bucket this wide holds at most one sample, so drawing it exactly
/// (rather than as a min/max envelope) costs no more than the pixel it fills.
fn exact_mode(view_span: f64, buckets: usize, step: f64) -> bool {
    if buckets == 0 || step <= 0.0 {
        return true;
    }
    (view_span / buckets as f64) <= step
}

fn accumulate_clip(clips: &mut Vec<ClipSpan>, open: &mut Option<(Clip, f32, f32)>, side: Option<Clip>, y: f32) {
    match side {
        None => flush_clip(clips, open),
        Some(side) => match open {
            Some((open_side, _, y1)) if *open_side == side => *y1 = y,
            _ => {
                flush_clip(clips, open);
                *open = Some((side, y, y));
            }
        },
    }
}

fn flush_clip(clips: &mut Vec<ClipSpan>, open: &mut Option<(Clip, f32, f32)>) {
    if let Some((side, y0, y1)) = open.take() {
        let (y0, y1) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        clips.push(ClipSpan { side, y0, y1 });
    }
}

fn flush_line(lines: &mut Vec<Vec<egui::Pos2>>, current: &mut Vec<egui::Pos2>) {
    if !current.is_empty() {
        lines.push(std::mem::take(current));
    }
}

/// Geometry for exact mode: a polyline through `samples`, broken at nulls.
fn sample_geometry(samples: impl Iterator<Item = (f64, Option<f32>)>, rect: egui::Rect, view: (f64, f64), range: [f32; 2]) -> TraceGeometry {
    let (top, bottom) = view;
    let span = bottom - top;
    let mut lines = Vec::new();
    let mut current = Vec::new();
    let mut clips = Vec::new();
    let mut open = None;

    for (depth, value) in samples {
        // Unclamped: the samples just past either end carry the line to the
        // edge at its true slope, and the painter clips it there.
        let t = if span > 0.0 { ((depth - top) / span) as f32 } else { 0.0 };
        let y = rect.top() + t * rect.height();
        match value {
            None => {
                flush_line(&mut lines, &mut current);
                accumulate_clip(&mut clips, &mut open, None, y);
            }
            Some(v) => {
                let (x, clip) = value_x(v, range, rect.left(), rect.right());
                current.push(egui::pos2(x, y));
                accumulate_clip(&mut clips, &mut open, (clip != Clip::Inside).then_some(clip), y);
            }
        }
    }
    flush_line(&mut lines, &mut current);
    flush_clip(&mut clips, &mut open);
    TraceGeometry { quads: Vec::new(), lines, clips }
}

/// Geometry for envelope mode: one quad and one line point per `Some`
/// bucket, `count` buckets spread evenly over `rect`'s height. Quads clamp to
/// the range's edges and widen to `min_width` when the envelope collapses to
/// a point; off-scale runs (both bucket ends clipped to the same edge) merge
/// into [`ClipSpan`]s regardless.
fn envelope_geometry(buckets: impl Iterator<Item = Option<(f32, f32)>>, count: usize, rect: egui::Rect, range: [f32; 2], min_width: f32) -> TraceGeometry {
    let mut quads = Vec::with_capacity(count);
    let mut lines = Vec::new();
    let mut current = Vec::new();
    let mut clips = Vec::new();
    let mut open = None;
    let h = if count == 0 { 0.0 } else { rect.height() / count as f32 };
    let (left, right) = (rect.left(), rect.right());

    for (i, bucket) in buckets.enumerate().take(count) {
        let y0 = rect.top() + i as f32 * h;
        let y1 = rect.top() + (i + 1) as f32 * h;
        match bucket {
            None => {
                flush_line(&mut lines, &mut current);
                accumulate_clip(&mut clips, &mut open, None, 0.5 * (y0 + y1));
            }
            Some((mn, mx)) => {
                let (x0, clip_lo) = value_x(mn, range, left, right);
                let (x1, clip_hi) = value_x(mx, range, left, right);
                let (mut qx0, mut qx1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
                if qx1 - qx0 < min_width {
                    let center = 0.5 * (qx0 + qx1);
                    qx0 = center - 0.5 * min_width;
                    qx1 = center + 0.5 * min_width;
                }
                quads.push(egui::Rect::from_min_max(egui::pos2(qx0, y0), egui::pos2(qx1, y1)));
                let (xm, _) = value_x(0.5 * (mn + mx), range, left, right);
                current.push(egui::pos2(xm, 0.5 * (y0 + y1)));
                let side = (clip_lo == clip_hi && clip_lo != Clip::Inside).then_some(clip_lo);
                accumulate_clip(&mut clips, &mut open, side, 0.5 * (y0 + y1));
            }
        }
    }
    flush_line(&mut lines, &mut current);
    flush_clip(&mut clips, &mut open);
    TraceGeometry { quads, lines, clips }
}

/// Fraction of the trace colour's alpha the envelope band fills at.
const BAND_ALPHA: f32 = 0.35;
const LINE_WIDTH: f32 = 1.0;
const MARKER_WIDTH: f32 = 2.0;
/// How far inside the edge an off-scale marker sits.
const MARKER_INSET: f32 = 2.0;
/// Fraction of the frame's weak text colour gridlines are drawn at.
const GRID_ALPHA: f32 = 0.3;

fn paint_geometry(painter: &egui::Painter, rect: egui::Rect, geometry: TraceGeometry, color: [u8; 4]) {
    if !geometry.quads.is_empty() {
        let band = egui::Color32::from_rgba_unmultiplied(color[0], color[1], color[2], (f32::from(color[3]) * BAND_ALPHA).round() as u8);
        let mut mesh = egui::Mesh::default();
        for quad in &geometry.quads {
            mesh.add_colored_rect(*quad, band);
        }
        painter.add(egui::Shape::mesh(mesh));
    }
    let line_color = to_color32(color);
    for line in geometry.lines {
        if let [point] = line.as_slice() {
            painter.circle_filled(*point, LINE_WIDTH, line_color);
        } else if line.len() >= 2 {
            painter.add(egui::Shape::line(line, egui::Stroke::new(LINE_WIDTH, line_color)));
        }
    }
    for clip in &geometry.clips {
        let x = match clip.side {
            Clip::Below => rect.left() + MARKER_INSET,
            Clip::Above => rect.right() - MARKER_INSET,
            Clip::Inside => continue,
        };
        painter.line_segment(
            [egui::pos2(x, clip.y0), egui::pos2(x, clip.y1.max(clip.y0 + 1.0))],
            egui::Stroke::new(MARKER_WIDTH, line_color),
        );
    }
}

/// Most gridlines a column draws, whatever range it is handed.
const MAX_GRIDLINES: i64 = 64;

/// The values gridlines fall on across `range`: multiples of its grid step,
/// counted by index so a huge or hair-thin range cannot stall the loop, and
/// never more than [`MAX_GRIDLINES`].
fn gridlines(range: [f32; 2]) -> impl Iterator<Item = f32> {
    let [lo, hi] = range.map(f64::from);
    let step = f64::from(grid_step(range));
    let usable = lo.is_finite() && hi.is_finite() && hi > lo && step > 0.0;
    let (first, last) = if usable { ((lo / step).ceil(), (hi / step).floor()) } else { (1.0, 0.0) };
    // Counted in f64 and capped before the cast, so no range can overflow it.
    let count = if first.is_finite() && last.is_finite() && last >= first {
        (last - first + 1.0).min(MAX_GRIDLINES as f64) as i64
    } else {
        0
    };
    (0..count).map(move |index| ((first + index as f64) * step) as f32)
}

/// Draw the column's plot body: hairline frame, gridlines at the range's
/// tidy step, then each trace as an exact polyline or a min/max envelope,
/// whichever costs one bucket per pixel row rather than one per sample.
pub(crate) fn draw_body(ui: &egui::Ui, painter: &egui::Painter, rect: egui::Rect, view: (f64, f64), traces: &ColumnTraces, range: [f32; 2], style: &WellLogStyle) {
    let clipped = painter.with_clip_rect(rect);
    clipped.rect_stroke(
        rect,
        crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS,
        ui.visuals().widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );

    let grid_color = ui.visuals().weak_text_color().gamma_multiply(GRID_ALPHA);
    if traces.has_range(style) {
        for value in gridlines(range) {
            let (x, _) = value_x(value, range, rect.left(), rect.right());
            clipped.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(1.0, grid_color));
        }
    }
    if traces.is_reading() {
        let font = egui::TextStyle::Small.resolve(ui.style());
        let mut job = egui::text::LayoutJob::simple_singleline(tr!(literal = "Reading..."), font, ui.visuals().weak_text_color());
        job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width());
        let galley = clipped.layout_job(job);
        clipped.galley(rect.center() - 0.5 * galley.size(), galley, egui::Color32::PLACEHOLDER);
    }

    let (top, bottom) = view;
    let view_span = bottom - top;
    let buckets = ((rect.height() * ui.ctx().pixels_per_point()).round().max(1.0)) as usize;
    let min_width = 1.0 / ui.ctx().pixels_per_point();

    for (kind, trace) in traces.kinded() {
        let color = style.color(kind);
        let trace_step = trace.step();
        let geometry = if exact_mode(view_span, buckets, trace_step) {
            sample_geometry(trace.samples(top - trace_step, bottom + trace_step), rect, view, range)
        } else {
            envelope_geometry(trace.envelope(top, bottom, buckets), buckets, rect, range, min_width)
        };
        paint_geometry(&clipped, rect, geometry, color);
    }
}

/// The hover readout over a column: the depth under the pointer, then each
/// curve's reading there in its colour, with the file curve it came from.
pub(crate) fn readout(ui: &mut egui::Ui, traces: &ColumnTraces, depth: f64, style: &WellLogStyle) {
    ui.label(tr_format!(literal = "%depth% m", depth = format!("{depth:.2}")));
    for (kind, trace) in traces.kinded() {
        // At most the two samples either side of the depth, never a scan.
        let half = 0.5 * trace.step();
        let reading = trace.samples(depth - half, depth + half).find_map(|(_, value)| value);
        let decimals = (-kind.resolution().log10()).round().max(0.0) as usize;
        let text = match reading {
            Some(value) => tr_format!(
                literal = "%curve%: %value% %unit%",
                curve = kind_label(kind),
                value = format!("{value:.decimals$}"),
                unit = kind.unit()
            ),
            None => tr_format!(literal = "%curve%: no reading", curve = kind_label(kind)),
        };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(text).color(to_color32(style.color(kind))));
            if !trace.source().is_empty() {
                ui.weak(trace.source());
            }
        });
    }
}

pub(crate) fn menu_title(column: TraceColumn) -> String {
    match column {
        TraceColumn::Density => tr!(literal = "Density scale"),
        TraceColumn::Gamma => tr!(literal = "Gamma scale"),
    }
}

fn presets(column: TraceColumn) -> &'static [[f32; 2]] {
    match column {
        TraceColumn::Density => &[[1.0, 3.0], [1.0, 2.0], [2.0, 3.0]],
        TraceColumn::Gamma => &[[0.0, 150.0], [0.0, 300.0]],
    }
}

/// A range as the menu writes it, each edge as the header writes it:
/// 1.0 to 3.0 g/cc, 0 to 150 API.
fn range_label(column: TraceColumn, range: [f32; 2]) -> String {
    tr_format!(
        literal = "%min% to %max% %unit%",
        min = edge_label(range[0], range),
        max = edge_label(range[1], range),
        unit = column.unit()
    )
}

/// The column's scale menu: auto vs a hole-derived range, presets, a custom
/// range, and a colour submenu per curve. Edits `draft` (the style the
/// caller previews live and decides when to save) and `custom` (the custom
/// bounds being typed, held in egui memory across frames). A custom range
/// reaches `draft` only once its edit is finished and it can be drawn.
pub(crate) fn menu(ui: &mut egui::Ui, column: TraceColumn, draft: &mut WellLogStyle, custom: &mut [f32; 2], auto: Option<[f32; 2]>) {
    let scale = draft.scale(column);

    let auto_hover = match auto {
        Some(range) => tr_format!(literal = "The hole's 1st to 99th percentile, rounded outward: %range%.", range = range_label(column, range)),
        None => tr!(literal = "The hole's 1st to 99th percentile, rounded outward. This hole has no data for it yet."),
    };
    let auto_row = ContextMenuAction::new(tr!(literal = "Auto, from this hole"))
        .checked(scale.auto)
        .show(ui)
        .on_hover_text(auto_hover);
    if auto_row.clicked() {
        draft.set_scale(column, TraceScale { auto: true, range: scale.range });
        ui.close();
    }

    context_menu_separator(ui);

    for &[lo, hi] in presets(column) {
        let label = range_label(column, [lo, hi]);
        let checked = !scale.auto && scale.range == [lo, hi];
        if ContextMenuAction::new(label).checked(checked).show(ui).clicked() {
            draft.set_scale(column, TraceScale { auto: false, range: [lo, hi] });
            ui.close();
        }
    }

    context_menu_separator(ui);

    context_menu_fields(ui, |ui| {
        ui.label(tr!(literal = "Custom range"));
        ui.horizontal(|ui| {
            let span = f64::from((custom[1] - custom[0]).abs().max(0.01));
            let speed = span / 200.0;
            // Typed values land on Enter or leaving the field, a drag on
            // release: never a half-typed number.
            let responses = [
                ui.add(egui::DragValue::new(&mut custom[0]).speed(speed).update_while_editing(false)),
                ui.add(egui::DragValue::new(&mut custom[1]).speed(speed).update_while_editing(false)),
            ];
            if column.accepts_range(*custom) && responses.iter().any(menu::committed) {
                draft.set_scale(column, TraceScale { auto: false, range: *custom });
            }
        });
    });

    context_menu_separator(ui);

    for &kind in column.kinds() {
        let label = match kind {
            LogKind::Gamma => tr!(literal = "Gamma colour"),
            LogKind::LongDensity => tr!(literal = "Long density colour"),
            LogKind::ShortDensity => tr!(literal = "Short density colour"),
        };
        context_submenu(ui, &label, true, |ui| {
            let mut color = to_color32(draft.color(kind));
            if egui::color_picker::color_picker_color32(ui, &mut color, egui::color_picker::Alpha::Opaque) {
                draft.set_color(kind, color.to_srgba_unmultiplied());
            }
            if ContextMenuAction::new(tr!(literal = "Default colour")).show(ui).clicked() {
                draft.set_color(kind, WellLogStyle::default().color(kind));
                ui.close();
            }
        });
    }
}
