//! A bar-and-line chart: stacked bars against the left axis, lines each
//! against an axis of their own on the right, one category per column - the
//! pit-by-pit graph (tonnes per shell as bars, cash flow and grade as lines),
//! and later the schedule charts.
//!
//! Painted by hand so it looks the same on every platform and needs no plot
//! crate. Hovering a column shows the caller's text for it; clicking selects it.
//!
//! The legend above the plot scrolls sideways when it does not fit. Hovering
//! an entry brings its series to the front and fades the others to a quarter;
//! clicking it hides or shows the series (its entry greyed), and the axes
//! follow what is shown. Hidden series are kept per chart (`id_salt`).

use super::{hint::Hint, toolbar::GROUP_CORNER_RADIUS};

/// Room left of the plot for the left axis numbers, and right of it for the right axis.
const AXIS_WIDTH: f32 = 56.0;
/// Room under the plot for the category labels.
const LABEL_HEIGHT: f32 = 18.0;
/// The legend's row above the plot.
const LEGEND_HEIGHT: f32 = 22.0;
/// What the series not hovered in the legend fade to (75 % transparent).
const FADED: f32 = 0.25;
/// Share of a column its bar fills.
const BAR_FILL: f32 = 0.7;
/// Gridlines (and axis numbers) each axis aims for.
const TICKS: usize = 5;

/// One stacked bar series.
pub(crate) struct BarSeries<'a> {
    pub(crate) name: String,
    pub(crate) color: egui::Color32,
    pub(crate) values: &'a [f64],
}

/// A line, against an axis of its own right of the plot (the first such
/// line's axis nearest the plot), or another line's.
pub(crate) struct LineSeries<'a> {
    pub(crate) name: String,
    pub(crate) color: egui::Color32,
    pub(crate) values: &'a [f64],
    pub(crate) scale: LineScale,
}

/// Which axis a line is drawn against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LineScale {
    /// Its own axis right of the plot. The band is where its values sit, as
    /// fractions of the plot height from the bottom (lowest value, highest
    /// value): `None` spans a round-numbered axis over the whole height,
    /// taking in 0; `Some((0.35, 0.65))` keeps the line in the middle, clear
    /// of the bars' tops and the other lines.
    Own(Option<(f32, f32)>),
    /// The axis of line `index` (an earlier one with its own), which then
    /// spans both: lines of one quantity compared on one scale.
    SameAs(usize),
}

pub(crate) struct BarLineChart<'a> {
    labels: &'a [String],
    /// Filled areas on the bars' axis, behind everything else.
    areas: Vec<BarSeries<'a>>,
    bars: Vec<BarSeries<'a>>,
    lines: Vec<LineSeries<'a>>,
    selected: Vec<usize>,
    /// Columns tinted to draw the eye, and the tint.
    marked: (Vec<usize>, egui::Color32),
    height: f32,
    hover_text: Option<&'a dyn Fn(usize) -> String>,
    /// Keys the hidden series, so they stay hidden from frame to frame.
    id_salt: Option<egui::Id>,
    /// A point drawn larger and ringed: (the line's name, its column).
    emphasized: Option<(String, usize)>,
}

/// What the reader did with the chart this frame.
#[derive(Default)]
pub(crate) struct ChartResponse {
    pub(crate) hovered: Option<usize>,
    pub(crate) clicked: Option<usize>,
}

impl<'a> BarLineChart<'a> {
    /// One column per entry of `labels`, which are printed under the columns
    /// (thinned out when they would overlap).
    pub(crate) fn new(labels: &'a [String]) -> Self {
        Self {
            labels,
            areas: Vec::new(),
            bars: Vec::new(),
            lines: Vec::new(),
            selected: Vec::new(),
            marked: (Vec::new(), egui::Color32::TRANSPARENT),
            height: 260.0,
            hover_text: None,
            id_salt: None,
            emphasized: None,
        }
    }

    /// Which chart this is, so the series hidden in its legend stay hidden.
    pub(crate) fn id_salt(mut self, salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.id_salt = Some(egui::Id::unique(salt));
        self
    }

    /// Draw line `line`'s point in `column` larger and ringed (the best value, say).
    pub(crate) fn emphasize(mut self, line: impl Into<String>, column: usize) -> Self {
        self.emphasized = Some((line.into(), column));
        self
    }

    /// A filled area against the bars' axis, drawn behind the bars (a stock
    /// over time, say).
    pub(crate) fn area(mut self, series: BarSeries<'a>) -> Self {
        self.areas.push(series);
        self
    }

    pub(crate) fn bars(mut self, series: BarSeries<'a>) -> Self {
        self.bars.push(series);
        self
    }

    pub(crate) fn line(mut self, series: LineSeries<'a>) -> Self {
        self.lines.push(series);
        self
    }

    /// Columns shown as selected: tinted and outlined.
    pub(crate) fn selected(mut self, columns: impl IntoIterator<Item = usize>) -> Self {
        self.selected = columns.into_iter().collect();
        self
    }

    /// Columns tinted with `color` (a warning, say), under any selection.
    pub(crate) fn marked(mut self, columns: impl IntoIterator<Item = usize>, color: egui::Color32) -> Self {
        self.marked = (columns.into_iter().collect(), color);
        self
    }

    pub(crate) fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The tooltip for a hovered column.
    pub(crate) fn hover_text(mut self, text: &'a dyn Fn(usize) -> String) -> Self {
        self.hover_text = Some(text);
        self
    }

    pub(crate) fn show(mut self, ui: &mut egui::Ui) -> ChartResponse {
        let columns = self.labels.len();
        let id = self.id_salt.unwrap_or_else(|| ui.scope_id().with("bar_line_chart"));
        let highlighted = self.legend(ui, id);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), (self.height - LEGEND_HEIGHT).max(40.0)), egui::Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        let visuals = ui.visuals().clone();
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, GROUP_CORNER_RADIUS, visuals.extreme_bg_color);
        let mut out = ChartResponse::default();
        if columns == 0 {
            return out;
        }

        // The axis each line is drawn against: its own (one column of numbers
        // each, right of the plot), or the one it shares.
        let owner = |index: usize| -> usize {
            match self.lines[index].scale {
                LineScale::SameAs(other) if other < self.lines.len() && matches!(self.lines[other].scale, LineScale::Own(_)) => other,
                _ => index,
            }
        };
        let own: Vec<usize> = (0..self.lines.len()).filter(|&index| owner(index) == index).collect();
        let plot = egui::Rect::from_min_max(
            rect.min + egui::vec2(AXIS_WIDTH, 8.0),
            rect.max - egui::vec2(if own.is_empty() { 12.0 } else { AXIS_WIDTH * own.len() as f32 }, LABEL_HEIGHT + 4.0),
        );
        if plot.width() <= 10.0 || plot.height() <= 10.0 {
            return out;
        }
        let text = visuals.text_color();
        let weak = visuals.weak_text_color();
        let grid = visuals.widgets.noninteractive.bg_stroke.color;
        let font = egui::FontId::proportional(11.0);

        // Left axis: from 0 to the tallest stack, or the highest area.
        let stack_top = (0..columns)
            .map(|column| self.bars.iter().map(|series| series.values.get(column).copied().unwrap_or(0.0).max(0.0)).sum::<f64>())
            .fold(0.0, f64::max);
        let stack_top = self
            .areas
            .iter()
            .flat_map(|area| area.values.iter().copied().filter(|value| value.is_finite()))
            .fold(stack_top, f64::max);
        let left = nice_axis(0.0, stack_top);
        let left_y = |value: f64| plot.bottom() - ((value - left.min) / (left.max - left.min)) as f32 * plot.height();
        for tick in left.ticks() {
            let y = left_y(tick);
            painter.line_segment([egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)], egui::Stroke::new(1.0, grid));
            painter.text(
                egui::pos2(plot.left() - 6.0, y),
                egui::Align2::RIGHT_CENTER,
                tick_label(tick, left.step),
                font.clone(),
                weak,
            );
        }

        // Right axes, one per line with its own, each a column further out;
        // an axis spans the values of every line drawn against it.
        let axes: Vec<Option<LineAxis>> = (0..self.lines.len())
            .map(|index| {
                let LineScale::Own(band) = self.lines[index].scale else {
                    return (owner(index) == index).then(|| LineAxis::of(self.lines[index].values, None));
                };
                let values: Vec<f64> = (0..self.lines.len())
                    .filter(|&other| owner(other) == index)
                    .flat_map(|other| self.lines[other].values.iter().copied())
                    .collect();
                Some(LineAxis::of(&values, band))
            })
            .collect();
        let line_y = |axis: &LineAxis, value: f64| plot.bottom() - ((value - axis.low) / (axis.high - axis.low)) as f32 * plot.height();
        for (slot, &index) in own.iter().enumerate() {
            let (line, Some(axis)) = (&self.lines[index], &axes[index]) else {
                continue;
            };
            let x = plot.right() + 6.0 + AXIS_WIDTH * slot as f32;
            for tick in axis.ticks() {
                painter.text(
                    egui::pos2(x, line_y(axis, tick)),
                    egui::Align2::LEFT_CENTER,
                    tick_label(tick, axis.step),
                    font.clone(),
                    line.color,
                );
            }
            if !matches!(line.scale, LineScale::Own(Some(_))) && axis.low < 0.0 {
                let zero = line_y(axis, 0.0);
                painter.line_segment(
                    [egui::pos2(plot.left(), zero), egui::pos2(plot.right(), zero)],
                    egui::Stroke::new(1.0, line.color.gamma_multiply(0.5)),
                );
            }
        }

        // Columns: the hovered or selected one tinted, then the stacked bars.
        let width = plot.width() / columns as f32;
        let column_rect = |column: usize| egui::Rect::from_x_y_ranges(plot.left() + width * column as f32..=plot.left() + width * (column + 1) as f32, plot.y_range());
        let pointer = response.hover_pos().filter(|pos| plot.contains(*pos));
        let hovered = pointer.map(|pos| (((pos.x - plot.left()) / width) as usize).min(columns - 1));
        // The confirm button's accent, as the rest of the optimization windows select with.
        let accent = super::menu::accent_fill(&visuals);
        for &column in self.marked.0.iter().filter(|&&column| column < columns) {
            painter.rect_filled(column_rect(column), 0.0, self.marked.1);
        }
        for &column in self.selected.iter().filter(|&&column| column < columns) {
            let area = column_rect(column);
            painter.rect_filled(area, 0.0, accent.gamma_multiply(0.25));
            painter.rect_stroke(area.shrink(0.5), 0.0, egui::Stroke::new(1.0, accent), egui::StrokeKind::Inside);
        }
        if let Some(column) = hovered {
            painter.rect_filled(column_rect(column), 0.0, accent.gamma_multiply(0.12));
        }
        // Hovered in the legend: that series in front, the others faded.
        let fade = |name: &str, color: egui::Color32| match &highlighted {
            Some(hovered) if hovered != name => color.gamma_multiply(FADED),
            _ => color,
        };
        let is_highlighted = |name: &str| highlighted.as_deref() == Some(name);
        // Areas: from the axis up to each column's value, joined across column
        // centres; a hovered one is drawn over the bars instead of behind them.
        let draw_area = |area: &BarSeries| {
            let points: Vec<egui::Pos2> = (0..columns)
                .map(|column| {
                    egui::pos2(
                        column_rect(column).center().x,
                        left_y(area.values.get(column).copied().filter(|value| value.is_finite()).unwrap_or(0.0).max(0.0)),
                    )
                })
                .collect();
            let color = fade(&area.name, area.color);
            let fill = color.gamma_multiply(0.35);
            for pair in points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let base = left_y(0.0);
                let quad = vec![egui::pos2(a.x, base), a, b, egui::pos2(b.x, base)];
                painter.add(egui::Shape::convex_polygon(quad, fill, egui::Stroke::NONE));
            }
            if points.len() == 1 {
                let bar = column_rect(0).shrink2(egui::vec2(width * (1.0 - BAR_FILL) / 2.0, 0.0));
                painter.rect_filled(egui::Rect::from_x_y_ranges(bar.x_range(), points[0].y..=left_y(0.0)), 0.0, fill);
            }
            painter.add(egui::Shape::line(points, egui::Stroke::new(1.5, color)));
        };
        for area in self.areas.iter().filter(|area| !is_highlighted(&area.name)) {
            draw_area(area);
        }
        for column in 0..columns {
            let bar = column_rect(column).shrink2(egui::vec2(width * (1.0 - BAR_FILL) / 2.0, 0.0));
            let mut base = 0.0;
            for series in &self.bars {
                let value = series.values.get(column).copied().unwrap_or(0.0).max(0.0);
                if value > 0.0 {
                    let block = egui::Rect::from_x_y_ranges(bar.x_range(), left_y(base + value)..=left_y(base));
                    painter.rect_filled(block, 0.0, fade(&series.name, series.color));
                }
                base += value;
            }
        }
        for area in self.areas.iter().filter(|area| is_highlighted(&area.name)) {
            draw_area(area);
        }

        // The lines, one point per column centre; a gap (a value that is not
        // finite) breaks a line. A hovered one is drawn last, on top.
        let mut order: Vec<usize> = (0..self.lines.len()).collect();
        order.sort_by_key(|&index| is_highlighted(&self.lines[index].name));
        for index in order {
            let line = &self.lines[index];
            let color = fade(&line.name, line.color);
            let y = |value: f64| match axes[owner(index)].as_ref() {
                Some(axis) => line_y(axis, value),
                None => left_y(value),
            };
            let mut run: Vec<egui::Pos2> = Vec::new();
            let mut runs: Vec<Vec<egui::Pos2>> = Vec::new();
            for column in 0..columns {
                match line.values.get(column).copied().filter(|value| value.is_finite()) {
                    Some(value) => run.push(egui::pos2(column_rect(column).center().x, y(value))),
                    None => runs.push(std::mem::take(&mut run)),
                }
            }
            runs.push(run);
            for run in runs.into_iter().filter(|run| !run.is_empty()) {
                painter.add(egui::Shape::line(run.clone(), egui::Stroke::new(2.0, color)));
                for point in run {
                    painter.circle_filled(point, 3.0, color);
                }
            }
            // The emphasized point: larger, ringed in the text colour.
            if let Some((_, column)) = self.emphasized.as_ref().filter(|(name, _)| *name == line.name)
                && let Some(value) = line.values.get(*column).copied().filter(|value| value.is_finite())
            {
                let point = egui::pos2(column_rect(*column).center().x, y(value));
                painter.circle_filled(point, 5.0, color);
                painter.circle_stroke(point, 8.0, egui::Stroke::new(2.0, fade(&line.name, text)));
            }
        }

        // Category labels, as many as fit side by side.
        let widest = self.labels.iter().map(|label| label.len()).max().unwrap_or(1) as f32 * 6.5 + 8.0;
        let every = ((widest / width).ceil() as usize).max(1);
        for column in (0..columns).step_by(every) {
            painter.text(
                egui::pos2(column_rect(column).center().x, plot.bottom() + 3.0),
                egui::Align2::CENTER_TOP,
                &self.labels[column],
                font.clone(),
                weak,
            );
        }

        if let Some(column) = hovered {
            out.hovered = Some(column);
            if let Some(hover_text) = self.hover_text {
                response.clone().hint_at_pointer(hover_text(column));
            }
            if response.clicked() {
                out.clicked = Some(column);
            }
        }
        out
    }
}

impl BarLineChart<'_> {
    /// The legend row: one entry per series, scrolling sideways when it does
    /// not fit. A click hides or shows a series; the hidden ones are taken out
    /// of the chart here (an axis a hidden line owned passes to the first line
    /// still shown that shared it). Returns the series hovered, if shown.
    fn legend(&mut self, ui: &mut egui::Ui, id: egui::Id) -> Option<String> {
        let hidden_id = id.with("hidden");
        let mut hidden: Vec<String> = ui.ctx().data(|data| data.get_temp(hidden_id)).unwrap_or_default();
        let entries: Vec<(String, egui::Color32, bool)> = self
            .areas
            .iter()
            .map(|series| (series.name.clone(), series.color.gamma_multiply(0.6), false))
            .chain(self.bars.iter().map(|series| (series.name.clone(), series.color, false)))
            .chain(self.lines.iter().map(|line| (line.name.clone(), line.color, true)))
            .collect();
        let mut highlighted = None;
        egui::ScrollArea::horizontal().id_salt(id.with("legend")).max_height(LEGEND_HEIGHT).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                for (name, color, is_line) in &entries {
                    let off = hidden.contains(name);
                    let response = legend_entry(ui, name, *color, *is_line, off);
                    if response.hovered() && !off {
                        highlighted = Some(name.clone());
                    }
                    if response.clicked() {
                        match hidden.iter().position(|other| other == name) {
                            Some(index) => {
                                hidden.remove(index);
                            }
                            None => hidden.push(name.clone()),
                        }
                    }
                }
            });
        });
        ui.ctx().data_mut(|data| data.insert_temp(hidden_id, hidden.clone()));

        self.areas.retain(|series| !hidden.contains(&series.name));
        self.bars.retain(|series| !hidden.contains(&series.name));
        let lines = std::mem::take(&mut self.lines);
        let owner_of = |index: usize| match lines[index].scale {
            LineScale::SameAs(other) if other < lines.len() && matches!(lines[other].scale, LineScale::Own(_)) => other,
            _ => index,
        };
        let kept: Vec<usize> = (0..lines.len()).filter(|&index| !hidden.contains(&lines[index].name)).collect();
        let scales: Vec<LineScale> = kept
            .iter()
            .map(|&index| {
                let owner = owner_of(index);
                if owner == index {
                    return lines[index].scale;
                }
                // The axis's line, or the first shown line on that axis, keeps it.
                let holder = if kept.contains(&owner) {
                    owner
                } else {
                    kept.iter().copied().find(|&other| owner_of(other) == owner).unwrap_or(index)
                };
                if holder == index {
                    lines[owner].scale
                } else {
                    LineScale::SameAs(kept.iter().position(|&other| other == holder).unwrap_or(0))
                }
            })
            .collect();
        self.lines = lines
            .into_iter()
            .enumerate()
            .filter(|(index, _)| kept.contains(index))
            .zip(scales)
            .map(|((_, line), scale)| LineSeries { scale, ..line })
            .collect();
        highlighted
    }
}

/// One legend entry: its swatch (a bar, or a line's stroke) and name, greyed
/// while its series is hidden.
fn legend_entry(ui: &mut egui::Ui, name: &str, color: egui::Color32, is_line: bool, off: bool) -> egui::Response {
    let font = egui::FontId::proportional(11.0);
    let text = if off { ui.visuals().weak_text_color() } else { ui.visuals().text_color() };
    let galley = ui.painter().layout_no_wrap(name.to_owned(), font, text);
    let size = egui::vec2(16.0 + galley.size().x, LEGEND_HEIGHT - 4.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let swatch_color = if off { color.gamma_multiply(0.25) } else { color };
    let swatch = egui::Rect::from_center_size(egui::pos2(rect.left() + 6.0, rect.center().y), egui::vec2(12.0, if is_line { 3.0 } else { 10.0 }));
    ui.painter().rect_filled(swatch, 0.0, swatch_color);
    ui.painter().galley(egui::pos2(rect.left() + 16.0, rect.center().y - galley.size().y / 2.0), galley, text);
    response.on_hover_cursor(egui::CursorIcon::PointingHand).hint(crate::i18n::tr!("chart-legend-hint"))
}

/// A line's axis: the values `low..high` span the plot's height; ticks are
/// round multiples of `step` within it.
struct LineAxis {
    low: f64,
    high: f64,
    step: f64,
}

impl LineAxis {
    fn of(values: &[f64], band: Option<(f32, f32)>) -> Self {
        let finite = values.iter().copied().filter(|value| value.is_finite());
        match band {
            None => {
                let (low, high) = finite.fold((0.0_f64, 0.0_f64), |(low, high), value| (low.min(value), high.max(value)));
                let axis = nice_axis(low, high);
                Self {
                    low: axis.min,
                    high: axis.max,
                    step: axis.step,
                }
            }
            Some((bottom, top)) => {
                let (mut low, mut high) = finite.fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| (low.min(value), high.max(value)));
                if !low.is_finite() {
                    (low, high) = (0.0, 1.0);
                }
                if high - low <= f64::EPSILON * high.abs().max(1.0) {
                    let pad = (low.abs() * 0.1).max(1e-6);
                    (low, high) = (low - pad, high + pad);
                }
                // Stretch the range so the values fill only the band.
                let (bottom, top) = (f64::from(bottom), f64::from(top.max(bottom + 0.05)));
                let span = (high - low) / (top - bottom);
                let (low, high) = (low - span * bottom, low - span * bottom + span);
                Self {
                    low,
                    high,
                    step: nice_axis(low, high).step,
                }
            }
        }
    }

    fn ticks(&self) -> impl Iterator<Item = f64> + '_ {
        let first = (self.low / self.step).ceil() as i64;
        let last = (self.high / self.step).floor() as i64;
        (first..=last).map(move |index| index as f64 * self.step)
    }
}

/// A tick's number: short for big values, with as many decimals as the step needs.
fn tick_label(value: f64, step: f64) -> String {
    if value.abs() >= 1000.0 {
        return compact(value);
    }
    let decimals = (-step.log10().floor()).clamp(0.0, 6.0) as usize;
    format!("{value:.decimals$}")
}

/// An axis from `min` to `max` stepped in round numbers.
struct Axis {
    min: f64,
    max: f64,
    step: f64,
}

impl Axis {
    fn ticks(&self) -> impl Iterator<Item = f64> + '_ {
        let count = ((self.max - self.min) / self.step).round() as usize;
        (0..=count).map(move |index| self.min + self.step * index as f64)
    }
}

/// Round `low..high` out to multiples of a 1, 2 or 5 step giving about [`TICKS`] gridlines.
fn nice_axis(low: f64, high: f64) -> Axis {
    let (low, high) = if high - low > 0.0 { (low, high) } else { (low.min(0.0), low.max(0.0) + 1.0) };
    let rough = (high - low) / TICKS as f64;
    let magnitude = 10f64.powf(rough.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|factor| factor * magnitude)
        .find(|step| *step >= rough)
        .unwrap_or(10.0 * magnitude);
    Axis {
        min: (low / step).floor() * step,
        max: (high / step).ceil() * step,
        step,
    }
}

/// A number short enough for an axis: 1.2k, 35M, 4.5B.
pub(crate) fn compact(value: f64) -> String {
    let size = value.abs();
    let (scaled, suffix) = if size >= 1e9 {
        (value / 1e9, "B")
    } else if size >= 1e6 {
        (value / 1e6, "M")
    } else if size >= 1e3 {
        (value / 1e3, "k")
    } else {
        (value, "")
    };
    let text = if scaled.fract().abs() < 1e-9 || scaled.abs() >= 100.0 {
        format!("{scaled:.0}")
    } else {
        format!("{scaled:.1}")
    };
    format!("{text}{suffix}")
}
