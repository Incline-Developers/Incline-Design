//! A bar-and-line chart: stacked bars against the left axis, one line against
//! the right, one category per column - the pit-by-pit graph (tonnes per
//! shell as bars, cash flow as the line), and later the schedule charts.
//!
//! Painted by hand so it looks the same on every platform and needs no plot
//! crate. Hovering a column shows the caller's text for it; clicking selects it.

use super::toolbar::GROUP_CORNER_RADIUS;

/// Room left of the plot for the left axis numbers, and right of it for the right axis.
const AXIS_WIDTH: f32 = 56.0;
/// Room under the plot for the category labels.
const LABEL_HEIGHT: f32 = 18.0;
/// Room above the plot for the legend.
const LEGEND_HEIGHT: f32 = 20.0;
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

/// The line, against the right axis.
pub(crate) struct LineSeries<'a> {
    pub(crate) name: String,
    pub(crate) color: egui::Color32,
    pub(crate) values: &'a [f64],
}

pub(crate) struct BarLineChart<'a> {
    labels: &'a [String],
    bars: Vec<BarSeries<'a>>,
    line: Option<LineSeries<'a>>,
    selected: Option<usize>,
    height: f32,
    hover_text: Option<&'a dyn Fn(usize) -> String>,
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
            bars: Vec::new(),
            line: None,
            selected: None,
            height: 260.0,
            hover_text: None,
        }
    }

    pub(crate) fn bars(mut self, series: BarSeries<'a>) -> Self {
        self.bars.push(series);
        self
    }

    pub(crate) fn line(mut self, series: LineSeries<'a>) -> Self {
        self.line = Some(series);
        self
    }

    pub(crate) fn selected(mut self, column: Option<usize>) -> Self {
        self.selected = column;
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

    pub(crate) fn show(self, ui: &mut egui::Ui) -> ChartResponse {
        let columns = self.labels.len();
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), self.height), egui::Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        let visuals = ui.visuals().clone();
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, GROUP_CORNER_RADIUS, visuals.extreme_bg_color);
        let mut out = ChartResponse::default();
        if columns == 0 {
            return out;
        }

        let plot = egui::Rect::from_min_max(
            rect.min + egui::vec2(AXIS_WIDTH, LEGEND_HEIGHT + 6.0),
            rect.max - egui::vec2(if self.line.is_some() { AXIS_WIDTH } else { 12.0 }, LABEL_HEIGHT + 4.0),
        );
        if plot.width() <= 10.0 || plot.height() <= 10.0 {
            return out;
        }
        let text = visuals.text_color();
        let weak = visuals.weak_text_color();
        let grid = visuals.widgets.noninteractive.bg_stroke.color;
        let font = egui::FontId::proportional(11.0);

        // Left axis: from 0 to the tallest stack.
        let stack_top = (0..columns)
            .map(|column| self.bars.iter().map(|series| series.values.get(column).copied().unwrap_or(0.0).max(0.0)).sum::<f64>())
            .fold(0.0, f64::max);
        let left = nice_axis(0.0, stack_top);
        let left_y = |value: f64| plot.bottom() - ((value - left.min) / (left.max - left.min)) as f32 * plot.height();
        for tick in left.ticks() {
            let y = left_y(tick);
            painter.line_segment([egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)], egui::Stroke::new(1.0, grid));
            painter.text(egui::pos2(plot.left() - 6.0, y), egui::Align2::RIGHT_CENTER, compact(tick), font.clone(), weak);
        }

        // Right axis: the line's range, always taking in 0.
        let right = self.line.as_ref().map(|line| {
            let finite = line.values.iter().copied().filter(|value| value.is_finite());
            let (low, high) = finite.fold((0.0_f64, 0.0_f64), |(low, high), value| (low.min(value), high.max(value)));
            nice_axis(low, high)
        });
        let right_y = |value: f64| {
            right
                .as_ref()
                .map_or(plot.bottom(), |axis| plot.bottom() - ((value - axis.min) / (axis.max - axis.min)) as f32 * plot.height())
        };
        if let (Some(axis), Some(line)) = (&right, &self.line) {
            for tick in axis.ticks() {
                painter.text(
                    egui::pos2(plot.right() + 6.0, right_y(tick)),
                    egui::Align2::LEFT_CENTER,
                    compact(tick),
                    font.clone(),
                    line.color,
                );
            }
            if axis.min < 0.0 {
                let zero = right_y(0.0);
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
        for (column, alpha) in [(self.selected, 0.25), (hovered, 0.12)] {
            if let Some(column) = column {
                painter.rect_filled(column_rect(column), 0.0, visuals.selection.bg_fill.gamma_multiply(alpha));
            }
        }
        for column in 0..columns {
            let bar = column_rect(column).shrink2(egui::vec2(width * (1.0 - BAR_FILL) / 2.0, 0.0));
            let mut base = 0.0;
            for series in &self.bars {
                let value = series.values.get(column).copied().unwrap_or(0.0).max(0.0);
                if value > 0.0 {
                    let block = egui::Rect::from_x_y_ranges(bar.x_range(), left_y(base + value)..=left_y(base));
                    painter.rect_filled(block, 0.0, series.color);
                }
                base += value;
            }
        }

        // The line, one point per column centre.
        if let Some(line) = &self.line {
            let points: Vec<egui::Pos2> = (0..columns)
                .filter_map(|column| {
                    let value = *line.values.get(column)?;
                    value.is_finite().then(|| egui::pos2(column_rect(column).center().x, right_y(value)))
                })
                .collect();
            painter.add(egui::Shape::line(points.clone(), egui::Stroke::new(2.0, line.color)));
            for point in points {
                painter.circle_filled(point, 3.0, line.color);
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

        // Legend along the top.
        let mut x = plot.left();
        let legend_y = rect.top() + LEGEND_HEIGHT / 2.0 + 2.0;
        let entries = self
            .bars
            .iter()
            .map(|series| (&series.name, series.color, false))
            .chain(self.line.iter().map(|line| (&line.name, line.color, true)));
        for (name, color, is_line) in entries {
            let swatch = egui::Rect::from_center_size(egui::pos2(x + 6.0, legend_y), egui::vec2(12.0, if is_line { 3.0 } else { 10.0 }));
            painter.rect_filled(swatch, 0.0, color);
            let galley = painter.layout_no_wrap(name.clone(), font.clone(), text);
            let width = galley.size().x;
            painter.galley(egui::pos2(x + 16.0, legend_y - galley.size().y / 2.0), galley, text);
            x += 16.0 + width + 14.0;
        }

        if let Some(column) = hovered {
            out.hovered = Some(column);
            if let Some(hover_text) = self.hover_text {
                response.clone().on_hover_text_at_pointer(hover_text(column));
            }
            if response.clicked() {
                out.clicked = Some(column);
            }
        }
        out
    }
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
