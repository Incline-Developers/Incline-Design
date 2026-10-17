//! Plot sheets: paper-space layout and CPU composition of a printable plan.
//!
//! A plot is the rendered map image placed on a paper-sized canvas at an exact
//! engineering scale, surrounded by the furniture a drawing needs: a frame,
//! coordinate grid ticks, a scale bar, a north arrow, a legend and a title
//! block. The renderer supplies the map pixels; everything else is drawn here
//! so the layout maths, the text and the output encoding stay testable and
//! independent of wgpu.

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};

use crate::{fonts::cosmic_text_font_system, i18n::tr};

const MM_PER_INCH: f64 = 25.4;

/// Ink and paper for the sheet furniture. Plots are printed, so they are drawn
/// dark-on-white regardless of the application theme.
const INK: [u8; 4] = [0, 0, 0, 255];
const PAPER: [u8; 4] = [255, 255, 255, 255];
const FAINT_INK: [u8; 4] = [110, 110, 110, 255];

/// A height as a contour label: `decimals` places, no thousands separators,
/// and never "-0".
pub(crate) fn format_level(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    match text.strip_prefix('-') {
        Some(unsigned) if unsigned.chars().all(|c| c == '0' || c == '.') => unsigned.to_owned(),
        _ => text,
    }
}

/// The fewest decimals, at most 3, that write every level exactly: a layer
/// contoured every 2.5 m needs 1, every 10 m none.
pub(crate) fn level_decimals(levels: impl Iterator<Item = f64> + Clone) -> usize {
    (0..3)
        .find(|&decimals| {
            let scale = 10f64.powi(decimals as i32);
            levels.clone().all(|level| (level * scale - (level * scale).round()).abs() <= 1e-6 * scale)
        })
        .unwrap_or(3)
}

/// Format a magnitude with thousands separators and a fixed number of decimals,
/// for sheet labels (grid coordinates, scale-bar ticks, the title block).
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

// ── Paper ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::EnumIter, strum::Display)]
pub(crate) enum PaperSize {
    A4,
    A3,
    A2,
    A1,
    A0,
    #[strum(to_string = "Letter")]
    Letter,
    #[strum(to_string = "Tabloid")]
    Tabloid,
    #[strum(to_string = "ANSI D")]
    AnsiD,
}

impl PaperSize {
    /// Portrait (width, height) in millimetres.
    pub(crate) fn portrait_mm(self) -> (f64, f64) {
        match self {
            Self::A4 => (210.0, 297.0),
            Self::A3 => (297.0, 420.0),
            Self::A2 => (420.0, 594.0),
            Self::A1 => (594.0, 841.0),
            Self::A0 => (841.0, 1189.0),
            Self::Letter => (215.9, 279.4),
            Self::Tabloid => (279.4, 431.8),
            Self::AnsiD => (558.8, 863.6),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::EnumIter, strum::Display)]
pub(crate) enum Orientation {
    Landscape,
    Portrait,
}

impl Orientation {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Landscape => crate::i18n::tr!("plot-landscape"),
            Self::Portrait => crate::i18n::tr!("plot-portrait"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TitleBlock {
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) project: String,
    pub(crate) author: String,
    pub(crate) drawing_number: String,
    pub(crate) revision: String,
    pub(crate) date: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LegendEntry {
    pub(crate) label: String,
    pub(crate) color: [f32; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlotSpec {
    pub(crate) paper: PaperSize,
    pub(crate) orientation: Orientation,
    pub(crate) dpi: u32,
    pub(crate) margin_mm: f64,
    /// Denominator of the plot scale: `1000.0` means 1:1000.
    pub(crate) scale: f64,
    pub(crate) title_block: Option<TitleBlock>,
    pub(crate) show_frame: bool,
    pub(crate) show_grid: bool,
    /// Grid line spacing in world metres; `None` picks a spacing that reads
    /// roughly every 50 mm on the printed sheet.
    pub(crate) grid_interval: Option<f64>,
    pub(crate) show_scale_bar: bool,
    pub(crate) show_north_arrow: bool,
    pub(crate) legend: Vec<LegendEntry>,
    /// The polylines of the contour-labelled layers, for their heights.
    pub(crate) contour_lines: Vec<SheetContourLine>,
}

/// A contour line drawn on the map, handed to the sheet as the screen labels
/// it: the line (shared, not copied), its text, how round its level is,
/// whether its label is quiet, and its colour.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SheetContourLine {
    pub(crate) line: std::sync::Arc<crate::model::geometry::ContourLabelLine>,
    pub(crate) text: std::sync::Arc<str>,
    pub(crate) rank: usize,
    pub(crate) quiet: bool,
    pub(crate) color: [f32; 4],
}

/// A rectangle in sheet pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RectPx {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl RectPx {
    fn right(self) -> f64 {
        self.x + self.width
    }

    fn bottom(self) -> f64 {
        self.y + self.height
    }
}

/// Everything paper-space derived from a [`PlotSpec`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PlotLayout {
    pub(crate) sheet_width_px: u32,
    pub(crate) sheet_height_px: u32,
    pub(crate) px_per_mm: f64,
    /// The map window, in sheet pixels.
    pub(crate) map: RectPx,
    /// World metres spanned by one sheet pixel at the requested scale.
    pub(crate) world_per_px: f64,
}

impl PlotLayout {
    /// World extent of the map window, in metres.
    pub(crate) fn map_world_size(&self) -> (f64, f64) {
        (self.map.width * self.world_per_px, self.map.height * self.world_per_px)
    }

    pub(crate) fn map_width_px(&self) -> u32 {
        self.map.width.round().max(1.0) as u32
    }

    pub(crate) fn map_height_px(&self) -> u32 {
        self.map.height.round().max(1.0) as u32
    }
}

/// Largest sheet dimension we will rasterise. An A0 at 600 dpi would need a
/// 28 000 px texture, which exceeds every GPU's limit and would allocate
/// gigabytes; the caller clamps the DPI and reports it rather than failing.
pub(crate) const MAX_SHEET_PIXELS: u32 = 12_000;

/// Highest DPI that keeps both sheet dimensions inside [`MAX_SHEET_PIXELS`].
pub(crate) fn max_dpi_for(paper: PaperSize, orientation: Orientation) -> u32 {
    let (width_mm, height_mm) = oriented_mm(paper, orientation);
    let limit = f64::from(MAX_SHEET_PIXELS) * MM_PER_INCH / width_mm.max(height_mm);
    (limit.floor() as u32).max(72)
}

fn oriented_mm(paper: PaperSize, orientation: Orientation) -> (f64, f64) {
    let (short, long) = paper.portrait_mm();
    match orientation {
        Orientation::Portrait => (short, long),
        Orientation::Landscape => (long, short),
    }
}

/// Resolve a plot specification into concrete pixel geometry.
pub(crate) fn layout(spec: &PlotSpec) -> Result<PlotLayout, String> {
    let (width_mm, height_mm) = oriented_mm(spec.paper, spec.orientation);
    let dpi = f64::from(spec.dpi.clamp(50, max_dpi_for(spec.paper, spec.orientation)));
    let px_per_mm = dpi / MM_PER_INCH;
    let sheet_width_px = (width_mm * px_per_mm).round().max(1.0) as u32;
    let sheet_height_px = (height_mm * px_per_mm).round().max(1.0) as u32;

    let margin = spec.margin_mm.clamp(0.0, width_mm.min(height_mm) * 0.4);
    let map = RectPx {
        x: margin * px_per_mm,
        y: margin * px_per_mm,
        width: (width_mm - margin * 2.0) * px_per_mm,
        height: (height_mm - margin * 2.0) * px_per_mm,
    };
    if map.width < 1.0 || map.height < 1.0 {
        return Err(tr!("plot-margins-leave-no-room-map"));
    }
    if !(spec.scale.is_finite() && spec.scale > 0.0) {
        return Err(tr!("plot-scale-must-be-positive"));
    }

    // One sheet millimetre is `scale` millimetres on the ground.
    let world_per_px = spec.scale / 1000.0 / px_per_mm;
    Ok(PlotLayout {
        sheet_width_px,
        sheet_height_px,
        px_per_mm,
        map,
        world_per_px,
    })
}

/// The scale that fits `world_width` × `world_height` metres into the map
/// window, rounded up to a conventional drawing scale.
pub(crate) fn fitted_scale(spec: &PlotSpec, world_width: f64, world_height: f64) -> Result<f64, String> {
    let layout = layout(spec)?;
    let map_width_mm = layout.map.width / layout.px_per_mm;
    let map_height_mm = layout.map.height / layout.px_per_mm;
    let needed = (world_width * 1000.0 / map_width_mm).max(world_height * 1000.0 / map_height_mm);
    Ok(round_up_to_series(needed.max(1.0), &DRAWING_SCALE_STEPS))
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

/// Round down to a 1 / 2 / 5 × 10ⁿ value, for scale bars that must not exceed
/// the space available.
fn round_down_to_nice(value: f64) -> f64 {
    if !(value.is_finite() && value > 0.0) {
        return 1.0;
    }
    let magnitude = 10f64.powf(value.log10().floor());
    let normalised = value / magnitude;
    let step = [5.0, 2.0, 1.0].into_iter().find(|step| normalised >= *step - 1.0e-9).unwrap_or(1.0);
    step * magnitude
}

/// Millimetre geometry of the sheet furniture.
///
/// The composer draws from these and the dialog's layout preview measures from
/// them, so a change to one cannot silently drift from the other.
pub(crate) mod furniture {
    /// Gap between the map frame and the boxes tucked into its corners.
    pub(crate) const INSET_MM: f64 = 8.0;
    pub(crate) const PADDING_MM: f64 = 3.0;
    pub(crate) const TITLE_BLOCK_WIDTH_MM: f64 = 95.0;
    pub(crate) const TITLE_BLOCK_HEIGHT_MM: f64 = 42.0;
    pub(crate) const NORTH_ARROW_WIDTH_MM: f64 = 20.0;
    pub(crate) const NORTH_ARROW_HEIGHT_MM: f64 = 30.0;
    /// Needle height inside the north-arrow box.
    pub(crate) const NORTH_ARROW_NEEDLE_MM: f64 = 18.0;
    pub(crate) const NORTH_ARROW_HALF_WIDTH_MM: f64 = 4.0;
    /// Distance from the map's right edge to the needle's axis.
    pub(crate) const NORTH_ARROW_CENTRE_INSET_MM: f64 = 16.0;
    pub(crate) const SCALE_BAR_BAR_MM: f64 = 2.5;
    pub(crate) const SCALE_BAR_LABEL_MM: f64 = 2.8;
    /// Two label lines (ticks above, caption below) plus padding and the bar.
    pub(crate) const SCALE_BAR_HEIGHT_MM: f64 = SCALE_BAR_BAR_MM + PADDING_MM * 2.0 + SCALE_BAR_LABEL_MM * 1.25 * 2.0;
    pub(crate) const LEGEND_SWATCH_MM: f64 = 3.5;
    pub(crate) const LEGEND_LABEL_MM: f64 = 3.0;
    pub(crate) const LEGEND_TITLE_MM: f64 = 3.4;
    pub(crate) const LEGEND_ROW_MM: f64 = 4.0;
    /// Contour height text, and the least paper between two labels.
    pub(crate) const CONTOUR_LABEL_MM: f64 = 2.0;
    pub(crate) const CONTOUR_LABEL_SPACING_MM: f64 = 80.0;
}

/// Where each piece of sheet furniture sits, in sheet pixels. A field is
/// `None` when that element is switched off.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SheetFurniture {
    pub(crate) title_block: Option<RectPx>,
    pub(crate) legend: Option<RectPx>,
    pub(crate) scale_bar: Option<RectPx>,
    pub(crate) north_arrow: Option<RectPx>,
    /// Grid spacing in world metres, resolved from the spec or chosen to read
    /// roughly every 50 mm on paper.
    pub(crate) grid_interval: Option<f64>,
}

/// Resolve where every piece of furniture lands on the sheet.
///
/// `legend_label_width` is the widest rendered legend label in pixels; the
/// composer measures it exactly while the dialog preview estimates it, which
/// only changes how wide that one box is drawn.
pub(crate) fn sheet_furniture(spec: &PlotSpec, layout: &PlotLayout, legend_label_width: f64, legend_rows: usize) -> SheetFurniture {
    let millimetre = layout.px_per_mm;
    let inset = furniture::INSET_MM * millimetre;
    let padding = furniture::PADDING_MM * millimetre;

    let title_block = spec.title_block.as_ref().map(|_| {
        let width = furniture::TITLE_BLOCK_WIDTH_MM * millimetre;
        let height = furniture::TITLE_BLOCK_HEIGHT_MM * millimetre;
        RectPx {
            x: layout.map.right() - width,
            y: layout.map.bottom() - height,
            width,
            height,
        }
    });

    let legend = (spec.legend.is_empty() || legend_rows == 0).then_some(()).map_or_else(
        || {
            Some(RectPx {
                x: layout.map.x + inset,
                y: layout.map.y + inset,
                width: legend_label_width + (furniture::LEGEND_SWATCH_MM + furniture::PADDING_MM * 3.0) * millimetre,
                height: (furniture::PADDING_MM * 2.0 + furniture::LEGEND_TITLE_MM * 1.25 + furniture::LEGEND_ROW_MM * legend_rows as f64) * millimetre,
            })
        },
        |()| None,
    );

    let scale_bar = spec.show_scale_bar.then(|| {
        let (_, bar_px) = scale_bar_length(layout, millimetre);
        let height = furniture::SCALE_BAR_HEIGHT_MM * millimetre;
        RectPx {
            x: layout.map.x + inset,
            y: layout.map.bottom() - inset - height,
            width: bar_px + padding * 2.0,
            height,
        }
    });

    let north_arrow = spec.show_north_arrow.then(|| RectPx {
        x: layout.map.right() - (furniture::NORTH_ARROW_CENTRE_INSET_MM + furniture::NORTH_ARROW_WIDTH_MM * 0.5) * millimetre,
        y: layout.map.y + (furniture::INSET_MM - furniture::PADDING_MM) * millimetre,
        width: furniture::NORTH_ARROW_WIDTH_MM * millimetre,
        height: furniture::NORTH_ARROW_HEIGHT_MM * millimetre,
    });

    SheetFurniture {
        title_block,
        legend,
        scale_bar,
        north_arrow,
        grid_interval: spec.show_grid.then(|| grid_interval_for(layout, millimetre, spec.grid_interval)),
    }
}

/// Where the map is centred and how big one pixel is on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MapFrame {
    pub(crate) center_east: f64,
    pub(crate) center_north: f64,
    pub(crate) world_per_px: f64,
}

// ── Canvas ─────────────────────────────────────────────────────────────────

/// A straight RGBA8 raster with the handful of drawing operations a plot sheet
/// needs. Coverage-based blending keeps the frame, ticks and arrow smooth at
/// any DPI without pulling in a rasteriser dependency.
pub(crate) struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Canvas {
    pub(crate) fn new(width: u32, height: u32, background: [u8; 4]) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..(width as usize * height as usize) {
            pixels.extend_from_slice(&background);
        }
        Self { width, height, pixels }
    }

    pub(crate) fn width(&self) -> u32 {
        self.width
    }

    pub(crate) fn height(&self) -> u32 {
        self.height
    }

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    fn blend(&mut self, x: i64, y: i64, color: [u8; 4], coverage: f64) {
        if coverage <= 0.0 || x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return;
        }
        let alpha = (coverage.min(1.0) * f64::from(color[3]) / 255.0).clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return;
        }
        let offset = ((y as usize * self.width as usize) + x as usize) * 4;
        for (destination, incoming) in self.pixels[offset..offset + 3].iter_mut().zip(color) {
            let existing = f64::from(*destination);
            *destination = (existing + (f64::from(incoming) - existing) * alpha).round().clamp(0.0, 255.0) as u8;
        }
        self.pixels[offset + 3] = 255;
    }

    pub(crate) fn fill_rect(&mut self, rect: RectPx, color: [u8; 4]) {
        let x0 = rect.x.floor() as i64;
        let x1 = rect.right().ceil() as i64;
        let y0 = rect.y.floor() as i64;
        let y1 = rect.bottom().ceil() as i64;
        for y in y0..y1 {
            for x in x0..x1 {
                // Per-pixel overlap with the rectangle gives clean edges at
                // fractional millimetre positions.
                let coverage = span_overlap(f64::from(x as i32), rect.x, rect.right()) * span_overlap(f64::from(y as i32), rect.y, rect.bottom());
                self.blend(x, y, color, coverage);
            }
        }
    }

    pub(crate) fn stroke_rect(&mut self, rect: RectPx, thickness: f64, color: [u8; 4]) {
        let thickness = thickness.max(0.5);
        self.fill_rect(
            RectPx {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: thickness,
            },
            color,
        );
        self.fill_rect(
            RectPx {
                x: rect.x,
                y: rect.bottom() - thickness,
                width: rect.width,
                height: thickness,
            },
            color,
        );
        self.fill_rect(
            RectPx {
                x: rect.x,
                y: rect.y,
                width: thickness,
                height: rect.height,
            },
            color,
        );
        self.fill_rect(
            RectPx {
                x: rect.right() - thickness,
                y: rect.y,
                width: thickness,
                height: rect.height,
            },
            color,
        );
    }

    pub(crate) fn draw_line(&mut self, start: (f64, f64), end: (f64, f64), thickness: f64, color: [u8; 4]) {
        let half = thickness.max(0.5) * 0.5;
        let x0 = (start.0.min(end.0) - half - 1.0).floor() as i64;
        let x1 = (start.0.max(end.0) + half + 1.0).ceil() as i64;
        let y0 = (start.1.min(end.1) - half - 1.0).floor() as i64;
        let y1 = (start.1.max(end.1) + half + 1.0).ceil() as i64;
        for y in y0..y1 {
            for x in x0..x1 {
                let distance = point_segment_distance((f64::from(x as i32) + 0.5, f64::from(y as i32) + 0.5), start, end);
                self.blend(x, y, color, (half + 0.5 - distance).clamp(0.0, 1.0));
            }
        }
    }

    pub(crate) fn fill_triangle(&mut self, points: [(f64, f64); 3], color: [u8; 4]) {
        let x0 = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min).floor() as i64;
        let x1 = points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max).ceil() as i64;
        let y0 = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min).floor() as i64;
        let y1 = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max).ceil() as i64;
        for y in y0..y1 {
            for x in x0..x1 {
                // 2×2 supersampling is enough to keep the north arrow's edges
                // from stepping visibly at print resolution.
                let mut hits = 0;
                for (offset_x, offset_y) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    if point_in_triangle((f64::from(x as i32) + offset_x, f64::from(y as i32) + offset_y), points) {
                        hits += 1;
                    }
                }
                self.blend(x, y, color, f64::from(hits) / 4.0);
            }
        }
    }

    /// Copy an opaque RGBA image into the sheet at `origin`, clipped to both.
    pub(crate) fn blit(&mut self, origin: (i64, i64), source: &[u8], source_width: u32, source_height: u32) {
        for row in 0..source_height {
            let y = origin.1 + i64::from(row);
            if y < 0 || y >= i64::from(self.height) {
                continue;
            }
            for column in 0..source_width {
                let x = origin.0 + i64::from(column);
                if x < 0 || x >= i64::from(self.width) {
                    continue;
                }
                let source_offset = ((row as usize * source_width as usize) + column as usize) * 4;
                let Some(pixel) = source.get(source_offset..source_offset + 4) else {
                    continue;
                };
                let destination = ((y as usize * self.width as usize) + x as usize) * 4;
                self.pixels[destination..destination + 3].copy_from_slice(&pixel[..3]);
                self.pixels[destination + 3] = 255;
            }
        }
    }
}

fn span_overlap(pixel: f64, low: f64, high: f64) -> f64 {
    (pixel + 1.0).min(high) - pixel.max(low)
}

fn point_segment_distance(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let length_squared = dx * dx + dy * dy;
    if length_squared <= f64::EPSILON {
        return ((point.0 - start.0).powi(2) + (point.1 - start.1).powi(2)).sqrt();
    }
    let t = (((point.0 - start.0) * dx + (point.1 - start.1) * dy) / length_squared).clamp(0.0, 1.0);
    ((point.0 - (start.0 + dx * t)).powi(2) + (point.1 - (start.1 + dy * t)).powi(2)).sqrt()
}

fn point_in_triangle(point: (f64, f64), triangle: [(f64, f64); 3]) -> bool {
    let sign = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| (a.0 - c.0) * (b.1 - c.1) - (b.0 - c.0) * (a.1 - c.1);
    let d1 = sign(point, triangle[0], triangle[1]);
    let d2 = sign(point, triangle[1], triangle[2]);
    let d3 = sign(point, triangle[2], triangle[0]);
    let has_negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_negative && has_positive)
}

// ── Text ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextAlign {
    Left,
    Center,
    Right,
}

/// Rasterises sheet labels. Owns its own font database so a plot can be
/// composed off the render thread.
pub(crate) struct TextPainter {
    font_system: FontSystem,
    cache: SwashCache,
}

impl TextPainter {
    pub(crate) fn new() -> Self {
        Self {
            font_system: cosmic_text_font_system(),
            cache: SwashCache::new(),
        }
    }

    fn shape(&mut self, text: &str, size_px: f64) -> Buffer {
        crate::fonts::request_cjk_fonts_for(text);
        let size = size_px.max(1.0) as f32;
        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(size, size * 1.25));
        buffer.set_size(None, None);
        buffer.set_text(text, &Attrs::new().family(Family::SansSerif), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, true);
        buffer
    }

    /// Rendered width of `text` in pixels.
    pub(crate) fn measure(&mut self, text: &str, size_px: f64) -> f64 {
        let buffer = self.shape(text, size_px);
        buffer.layout_runs().map(|run| f64::from(run.line_w)).fold(0.0, f64::max)
    }

    /// Draw `text` with its baseline-independent top edge at `y`, horizontally
    /// placed relative to `x` by `align`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(&mut self, canvas: &mut Canvas, text: &str, x: f64, y: f64, size_px: f64, align: TextAlign, color: [u8; 4]) {
        if text.is_empty() {
            return;
        }
        let width = self.measure(text, size_px);
        let origin_x = match align {
            TextAlign::Left => x,
            TextAlign::Center => x - width * 0.5,
            TextAlign::Right => x - width,
        };
        let mut buffer = self.shape(text, size_px);
        let font_color = cosmic_text::Color::rgba(color[0], color[1], color[2], color[3]);
        let mut spans = Vec::new();
        buffer.draw(&mut self.font_system, &mut self.cache, font_color, |glyph_x, glyph_y, width, height, pixel| {
            spans.push((glyph_x, glyph_y, width, height, pixel));
        });
        for (glyph_x, glyph_y, span_width, span_height, pixel) in spans {
            let coverage = f64::from(pixel.a()) / 255.0;
            if coverage <= 0.0 {
                continue;
            }
            let rect = RectPx {
                x: origin_x + f64::from(glyph_x),
                y: y + f64::from(glyph_y),
                width: f64::from(span_width),
                height: f64::from(span_height),
            };
            let solid = [pixel.r(), pixel.g(), pixel.b(), 255];
            let x0 = rect.x.round() as i64;
            let y0 = rect.y.round() as i64;
            for row in 0..span_height as i64 {
                for column in 0..span_width as i64 {
                    canvas.blend(x0 + column, y0 + row, solid, coverage);
                }
            }
        }
    }

    /// Draw `text` centred on (`x`, `y`) and turned `angle` radians, clockwise
    /// on the sheet whose y grows down, for a label that runs along a line.
    /// The glyphs are laid out level, then sampled turned.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_rotated(&mut self, canvas: &mut Canvas, text: &str, x: f64, y: f64, size_px: f64, angle: f64, color: [u8; 4]) {
        if text.is_empty() {
            return;
        }
        let mut buffer = self.shape(text, size_px);
        let width = buffer.layout_runs().map(|run| f64::from(run.line_w)).fold(0.0, f64::max);
        let height = self.line_height(size_px);
        let columns = width.ceil() as usize + 1;
        let rows = height.ceil() as usize + 1;
        let mut coverage = vec![0.0f64; columns * rows];
        let font_color = cosmic_text::Color::rgba(color[0], color[1], color[2], color[3]);
        buffer.draw(&mut self.font_system, &mut self.cache, font_color, |glyph_x, glyph_y, span_width, span_height, pixel| {
            for row in glyph_y..glyph_y + span_height as i32 {
                for column in glyph_x..glyph_x + span_width as i32 {
                    if let (Ok(column), Ok(row)) = (usize::try_from(column), usize::try_from(row))
                        && column < columns
                        && row < rows
                    {
                        let cell = &mut coverage[row * columns + column];
                        *cell = cell.max(f64::from(pixel.a()) / 255.0);
                    }
                }
            }
        });
        let sample = |column: f64, row: f64| {
            let (left, top) = (column.floor(), row.floor());
            let (across, down) = (column - left, row - top);
            let at = |column: f64, row: f64| {
                if column < 0.0 || row < 0.0 || column >= columns as f64 || row >= rows as f64 {
                    0.0
                } else {
                    coverage[row as usize * columns + column as usize]
                }
            };
            let upper = at(left, top) * (1.0 - across) + at(left + 1.0, top) * across;
            let lower = at(left, top + 1.0) * (1.0 - across) + at(left + 1.0, top + 1.0) * across;
            upper * (1.0 - down) + lower * down
        };
        let (sin, cos) = angle.sin_cos();
        let (half_width, half_height) = (width * 0.5, height * 0.5);
        let reach_x = half_width * cos.abs() + half_height * sin.abs();
        let reach_y = half_width * sin.abs() + half_height * cos.abs();
        for pixel_y in (y - reach_y).floor() as i64..=(y + reach_y).ceil() as i64 {
            for pixel_x in (x - reach_x).floor() as i64..=(x + reach_x).ceil() as i64 {
                let (dx, dy) = (pixel_x as f64 + 0.5 - x, pixel_y as f64 + 0.5 - y);
                let column = cos * dx + sin * dy + half_width;
                let row = -sin * dx + cos * dy + half_height;
                let coverage = sample(column - 0.5, row - 0.5);
                if coverage > 0.0 {
                    canvas.blend(pixel_x, pixel_y, [color[0], color[1], color[2], 255], coverage);
                }
            }
        }
    }

    /// Height of one line of text at `size_px`, matching [`Self::shape`].
    pub(crate) fn line_height(&self, size_px: f64) -> f64 {
        text_line_height(size_px)
    }
}

/// Height of one line of text at `size_px`, as [`TextPainter`] lays it out.
pub(crate) fn text_line_height(size_px: f64) -> f64 {
    size_px.max(1.0) * 1.25
}

impl Default for TextPainter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Sheet composition ──────────────────────────────────────────────────────

/// The rendered map plus everything needed to annotate it.
pub(crate) struct SheetInput<'map> {
    pub(crate) spec: PlotSpec,
    pub(crate) layout: PlotLayout,
    pub(crate) frame: MapFrame,
    /// RGBA pixels of the rendered map, `layout.map_width_px()` wide.
    pub(crate) map_pixels: &'map [u8],
}

/// Draw the complete sheet and return it as RGBA8 pixels.
pub(crate) fn compose_sheet(input: SheetInput<'_>, painter: &mut TextPainter) -> Canvas {
    let SheetInput { spec, layout, frame, map_pixels } = input;
    let mut canvas = Canvas::new(layout.sheet_width_px, layout.sheet_height_px, PAPER);
    canvas.blit(
        (layout.map.x.round() as i64, layout.map.y.round() as i64),
        map_pixels,
        layout.map_width_px(),
        layout.map_height_px(),
    );

    let millimetre = layout.px_per_mm;
    // Legend labels are elided and measured first because the box they sit in
    // is sized from the widest of them.
    let label_size = furniture::LEGEND_LABEL_MM * millimetre;
    let max_label_width = (layout.map.width * 0.25 - (furniture::LEGEND_SWATCH_MM + furniture::PADDING_MM * 3.0) * millimetre).max(20.0 * millimetre);
    let legend_labels: Vec<String> = spec.legend.iter().map(|entry| elide_to_width(painter, &entry.label, label_size, max_label_width)).collect();
    let legend_title = tr!("common-legend");
    let mut widest_label = painter.measure(&legend_title, furniture::LEGEND_TITLE_MM * millimetre);
    for label in &legend_labels {
        widest_label = widest_label.max(painter.measure(label, label_size));
    }
    let placement = sheet_furniture(&spec, &layout, widest_label, legend_labels.len());

    if let Some(interval) = placement.grid_interval {
        draw_grid(&mut canvas, painter, &layout, &frame, interval, millimetre);
    }
    let mut measure = |text: &str, size_px: f64| (painter.measure(text, size_px), painter.line_height(size_px));
    for label in place_sheet_contour_labels(&mut measure, &layout, &frame, &placement, &spec.contour_lines) {
        draw_contour_label(&mut canvas, painter, &label, millimetre);
    }
    if spec.show_frame {
        canvas.stroke_rect(layout.map, (0.4 * millimetre).max(1.0), INK);
    }
    if let Some(rect) = placement.north_arrow {
        draw_north_arrow(&mut canvas, painter, rect, millimetre);
    }
    if let Some(rect) = placement.scale_bar {
        draw_scale_bar(&mut canvas, painter, &layout, rect, spec.scale, millimetre);
    }
    if let Some(rect) = placement.legend {
        draw_legend(&mut canvas, painter, rect, &spec.legend, &legend_labels, millimetre);
    }
    if let Some((rect, title_block)) = placement.title_block.zip(spec.title_block.as_ref()) {
        draw_title_block(&mut canvas, painter, rect, title_block, spec.scale, millimetre);
    }
    canvas
}

/// A contour label on the sheet: its text, middle and turn (clockwise, as the
/// sheet's y grows down), text size and box in pixels, and its line's colour.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SheetContourLabel {
    pub(crate) text: String,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) angle: f64,
    pub(crate) size_px: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) color: [f32; 4],
}

/// Contour heights for the map at the plot's scale, placed as the screen
/// places them: [`furniture::CONTOUR_LABEL_MM`] text (quiet ones smaller),
/// a label every power of two of ground at least
/// [`furniture::CONTOUR_LABEL_SPACING_MM`] apart on paper, roundest first,
/// clear of each other, wholly inside the map and off every `avoid` rect.
/// `measure` gives a text's width and line height in pixels at a size.
pub(crate) fn sheet_contour_labels(
    mut measure: impl FnMut(&str, f64) -> (f64, f64),
    layout: &PlotLayout,
    frame: &MapFrame,
    lines: &[SheetContourLine],
    millimetre: f64,
    avoid: &[RectPx],
) -> Vec<SheetContourLabel> {
    if lines.is_empty() {
        return Vec::new();
    }
    let world_per_px = frame.world_per_px;
    let world_per_mm = world_per_px * millimetre;
    let height = furniture::CONTOUR_LABEL_MM * world_per_mm;
    let spacing = 2f64.powf((furniture::CONTOUR_LABEL_SPACING_MM * world_per_mm).log2().ceil());
    let (world_width, world_height) = layout.map_world_size();
    let east_min = frame.center_east - world_width * 0.5;
    let north_min = frame.center_north - world_height * 0.5;
    let to_sheet = |east: f64, north: f64| (layout.map.x + (east - east_min) / world_per_px, layout.map.bottom() - (north - north_min) / world_per_px);
    let keep = |at: glam::DVec3, half: glam::DVec2| {
        let inside = at.x - half.x >= east_min && at.x + half.x <= east_min + world_width && at.y - half.y >= north_min && at.y + half.y <= north_min + world_height;
        let (left, top) = to_sheet(at.x - half.x, at.y + half.y);
        let (right, bottom) = to_sheet(at.x + half.x, at.y - half.y);
        inside && avoid.iter().all(|rect| right < rect.x || left > rect.right() || bottom < rect.y || top > rect.bottom())
    };
    let size = |text: &str, height: f64| {
        let size_px = height / world_per_px;
        let (width, line_height) = measure(text, size_px);
        crate::model::geometry::label_box(width, line_height, size_px) * world_per_px
    };
    let ranked = lines
        .iter()
        .enumerate()
        .map(|(index, line)| crate::model::geometry::RankedLine {
            index,
            line: &line.line,
            text: &line.text,
            rank: line.rank,
            quiet: line.quiet,
        })
        .collect();
    let area = (glam::DVec2::new(east_min, north_min), glam::DVec2::new(east_min + world_width, north_min + world_height));
    crate::model::geometry::place_ranked_labels(ranked, spacing, height, Some(area), keep, size)
        .into_iter()
        .map(|label| {
            let (x, y) = to_sheet(label.centre.x, label.centre.y);
            SheetContourLabel {
                x,
                y,
                // Sheet Y grows downwards, so the turn reverses.
                angle: -label.rotation,
                size_px: label.height / world_per_px,
                width: label.size.x / world_per_px,
                height: label.size.y / world_per_px,
                color: lines[label.source].color,
                text: label.text,
            }
        })
        .collect()
}

/// Where [`draw_grid`] puts northing labels inside the map, because the left
/// margin cannot hold them: a band down the map's left edge, or `None`.
/// `measure` gives a text's width and line height in pixels at a size.
fn grid_label_band(measure: &mut impl FnMut(&str, f64) -> (f64, f64), layout: &PlotLayout, frame: &MapFrame, interval: f64, millimetre: f64) -> Option<RectPx> {
    if grid_too_dense(layout, interval) {
        return None;
    }
    let (_, world_height) = layout.map_world_size();
    let north_min = frame.center_north - world_height * 0.5;
    let label_size = 2.4 * millimetre;
    let tick = 2.0 * millimetre;
    let margin_available = layout.map.x - tick - millimetre;
    let mut widest = 0.0f64;
    let mut northing = (north_min / interval).ceil() * interval;
    while northing <= north_min + world_height {
        let (width, _) = measure(&format_coordinate(northing), label_size);
        if width > margin_available {
            widest = widest.max(width);
        }
        northing += interval;
    }
    (widest > 0.0).then_some(RectPx {
        x: layout.map.x,
        y: layout.map.y,
        width: tick + widest,
        height: layout.map.height,
    })
}

/// The contour heights a sheet carries, placed clear of its furniture and of
/// grid coordinates inside the map: what the export draws and the dialog's
/// preview shows, from the same rule. `measure` gives a text's width and
/// line height in pixels at a size.
pub(crate) fn place_sheet_contour_labels(
    measure: &mut impl FnMut(&str, f64) -> (f64, f64),
    layout: &PlotLayout,
    frame: &MapFrame,
    placement: &SheetFurniture,
    lines: &[SheetContourLine],
) -> Vec<SheetContourLabel> {
    if lines.is_empty() {
        return Vec::new();
    }
    let millimetre = layout.px_per_mm;
    let mut avoid: Vec<RectPx> = [placement.north_arrow, placement.scale_bar, placement.legend, placement.title_block]
        .into_iter()
        .flatten()
        .collect();
    avoid.extend(placement.grid_interval.and_then(|interval| grid_label_band(measure, layout, frame, interval, millimetre)));
    sheet_contour_labels(measure, layout, frame, lines, millimetre, &avoid)
}

impl SheetContourLabel {
    /// The label box's corners on the sheet, turned with the label.
    pub(crate) fn corners(&self) -> [(f64, f64); 4] {
        let (sin, cos) = self.angle.sin_cos();
        let corner = |across: f64, down: f64| {
            let (dx, dy) = (across * self.width * 0.5, down * self.height * 0.5);
            (self.x + cos * dx - sin * dy, self.y + sin * dx + cos * dy)
        };
        [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)]
    }
}

/// A contour label as the screen draws one: a box filled in its line's
/// colour (turned to sRGB as the map's lines are) and edged in ink, with
/// text in black or white by the same split the screen uses.
fn draw_contour_label(canvas: &mut Canvas, painter: &mut TextPainter, label: &SheetContourLabel, millimetre: f64) {
    let corners = label.corners();
    let fill = contour_label_fill(label.color);
    canvas.fill_triangle([corners[0], corners[1], corners[2]], fill);
    canvas.fill_triangle([corners[0], corners[2], corners[3]], fill);
    let thickness = (0.15 * millimetre).max(0.75);
    for (start, end) in corners.iter().zip(corners.iter().cycle().skip(1)) {
        canvas.draw_line(*start, *end, thickness, INK);
    }
    let ink = if crate::rendering::color::is_light(label.color) { INK } else { PAPER };
    painter.draw_rotated(canvas, &label.text, label.x, label.y, label.size_px, label.angle, ink);
}

/// A label's fill: its line's linear colour as sheet bytes, opaque.
fn contour_label_fill(color: [f32; 4]) -> [u8; 4] {
    crate::rendering::color::opaque_srgb(color)
}

/// Grid spacing that lands close to 50 mm on paper, rounded to a conventional
/// interval.
fn grid_interval_for(layout: &PlotLayout, millimetre: f64, requested: Option<f64>) -> f64 {
    requested
        .filter(|interval| interval.is_finite() && *interval > 0.0)
        .unwrap_or_else(|| round_up_to_series(50.0 * millimetre * layout.world_per_px, &DRAWING_SCALE_STEPS))
}

/// Whether a grid every `interval` would need more lines than [`draw_grid`]
/// draws. A pathological interval could ask for millions of lines; the guard
/// keeps a mistyped value from hanging the composition.
fn grid_too_dense(layout: &PlotLayout, interval: f64) -> bool {
    const MAX_GRID_LINES: f64 = 400.0;
    let (world_width, world_height) = layout.map_world_size();
    (world_width / interval) + (world_height / interval) > MAX_GRID_LINES
}

fn draw_grid(canvas: &mut Canvas, painter: &mut TextPainter, layout: &PlotLayout, frame: &MapFrame, interval: f64, millimetre: f64) {
    let (world_width, world_height) = layout.map_world_size();
    let east_min = frame.center_east - world_width * 0.5;
    let east_max = frame.center_east + world_width * 0.5;
    let north_min = frame.center_north - world_height * 0.5;
    let north_max = frame.center_north + world_height * 0.5;

    if grid_too_dense(layout, interval) {
        return;
    }

    let thickness = (0.15 * millimetre).max(0.75);
    let label_size = 2.4 * millimetre;
    let tick = 2.0 * millimetre;

    let mut easting = (east_min / interval).ceil() * interval;
    while easting <= east_max {
        let x = layout.map.x + (easting - east_min) / frame.world_per_px;
        canvas.draw_line((x, layout.map.y), (x, layout.map.bottom()), thickness, FAINT_INK);
        canvas.draw_line((x, layout.map.y - tick), (x, layout.map.y), thickness * 2.0, INK);
        painter.draw(
            canvas,
            &format_coordinate(easting),
            x,
            layout.map.y - tick - painter.line_height(label_size),
            label_size,
            TextAlign::Center,
            INK,
        );
        easting += interval;
    }

    let mut northing = (north_min / interval).ceil() * interval;
    while northing <= north_max {
        // Sheet Y grows downwards while northing grows up.
        let y = layout.map.bottom() - (northing - north_min) / frame.world_per_px;
        canvas.draw_line((layout.map.x, y), (layout.map.right(), y), thickness, FAINT_INK);
        canvas.draw_line((layout.map.x - tick, y), (layout.map.x, y), thickness * 2.0, INK);
        // Northing labels run horizontally into the left margin. Full grid
        // coordinates are wide, so where the margin cannot hold one the label
        // moves just inside the map instead of running off the sheet.
        let label = format_coordinate(northing);
        let margin_available = layout.map.x - tick - millimetre;
        let (label_x, align) = if painter.measure(&label, label_size) <= margin_available {
            (layout.map.x - tick - millimetre, TextAlign::Right)
        } else {
            (layout.map.x + tick, TextAlign::Left)
        };
        painter.draw(canvas, &label, label_x, y - painter.line_height(label_size) * 0.5, label_size, align, INK);
        northing += interval;
    }
}

fn format_coordinate(value: f64) -> String {
    if (value - value.round()).abs() < 1.0e-6 {
        format_quantity(value, 0)
    } else {
        format_quantity(value, 2)
    }
}

fn draw_north_arrow(canvas: &mut Canvas, painter: &mut TextPainter, box_rect: RectPx, millimetre: f64) {
    let half_width = furniture::NORTH_ARROW_HALF_WIDTH_MM * millimetre;
    let centre_x = box_rect.x + box_rect.width * 0.5;
    let top = box_rect.y + furniture::PADDING_MM * millimetre;
    let bottom = top + furniture::NORTH_ARROW_NEEDLE_MM * millimetre;

    canvas.fill_rect(box_rect, PAPER);
    canvas.stroke_rect(box_rect, (0.3 * millimetre).max(1.0), INK);

    // A solid half and an outlined half, the conventional split needle.
    canvas.fill_triangle([(centre_x, top), (centre_x, bottom), (centre_x - half_width, bottom)], INK);
    let outline = (0.3 * millimetre).max(1.0);
    canvas.draw_line((centre_x, top), (centre_x + half_width, bottom), outline, INK);
    canvas.draw_line((centre_x + half_width, bottom), (centre_x, bottom), outline, INK);

    let label_size = 4.0 * millimetre;
    painter.draw(canvas, "N", centre_x, bottom + 1.0 * millimetre, label_size, TextAlign::Center, INK);
}

/// Bar length in world metres, and the paper length it occupies.
fn scale_bar_length(layout: &PlotLayout, millimetre: f64) -> (f64, f64) {
    let target_paper_px = (layout.map.width * 0.25).min(80.0 * millimetre);
    let world = round_down_to_nice(target_paper_px * layout.world_per_px);
    (world, world / layout.world_per_px)
}

fn draw_scale_bar(canvas: &mut Canvas, painter: &mut TextPainter, layout: &PlotLayout, box_rect: RectPx, scale: f64, millimetre: f64) {
    let (world_length, bar_px) = scale_bar_length(layout, millimetre);
    if bar_px <= 0.0 {
        return;
    }
    let segments = 4usize;
    let bar_height = furniture::SCALE_BAR_BAR_MM * millimetre;
    let label_size = furniture::SCALE_BAR_LABEL_MM * millimetre;
    let padding = furniture::PADDING_MM * millimetre;
    canvas.fill_rect(box_rect, PAPER);
    canvas.stroke_rect(box_rect, (0.3 * millimetre).max(1.0), INK);

    let bar_x = box_rect.x + padding;
    let bar_y = box_rect.y + padding + painter.line_height(label_size);
    let segment = bar_px / segments as f64;
    for index in 0..segments {
        let rect = RectPx {
            x: bar_x + segment * index as f64,
            y: bar_y,
            width: segment,
            height: bar_height,
        };
        canvas.fill_rect(rect, if index.is_multiple_of(2) { INK } else { PAPER });
        canvas.stroke_rect(rect, (0.2 * millimetre).max(0.75), INK);
    }

    for index in 0..=segments {
        let value = world_length * index as f64 / segments as f64;
        painter.draw(
            canvas,
            &format_quantity(value, 0),
            bar_x + segment * index as f64,
            box_rect.y + padding * 0.25,
            label_size,
            TextAlign::Center,
            INK,
        );
    }
    painter.draw(
        canvas,
        &tr!("plot-metres-scale-1-scale", scale = format_quantity(scale, 0).to_string()),
        bar_x,
        bar_y + bar_height + millimetre * 0.5,
        label_size,
        TextAlign::Left,
        INK,
    );
}

fn draw_legend(canvas: &mut Canvas, painter: &mut TextPainter, box_rect: RectPx, entries: &[LegendEntry], labels: &[String], millimetre: f64) {
    let label_size = furniture::LEGEND_LABEL_MM * millimetre;
    let row_height = furniture::LEGEND_ROW_MM * millimetre;
    let swatch = furniture::LEGEND_SWATCH_MM * millimetre;
    let padding = furniture::PADDING_MM * millimetre;
    let title_size = furniture::LEGEND_TITLE_MM * millimetre;

    canvas.fill_rect(box_rect, PAPER);
    canvas.stroke_rect(box_rect, (0.3 * millimetre).max(1.0), INK);

    painter.draw(canvas, &tr!("common-legend"), box_rect.x + padding, box_rect.y + padding, title_size, TextAlign::Left, INK);
    for (index, entry) in entries.iter().enumerate() {
        let row_y = box_rect.y + padding + painter.line_height(title_size) + row_height * index as f64;
        let swatch_rect = RectPx {
            x: box_rect.x + padding,
            y: row_y + (row_height - swatch) * 0.5,
            width: swatch,
            height: swatch,
        };
        canvas.fill_rect(swatch_rect, rgba_bytes(entry.color));
        canvas.stroke_rect(swatch_rect, (0.2 * millimetre).max(0.75), INK);
        painter.draw(
            canvas,
            &labels[index],
            swatch_rect.right() + padding,
            row_y + (row_height - painter.line_height(label_size)) * 0.5,
            label_size,
            TextAlign::Left,
            INK,
        );
    }
}

fn draw_title_block(canvas: &mut Canvas, painter: &mut TextPainter, rect: RectPx, block: &TitleBlock, scale: f64, millimetre: f64) {
    canvas.fill_rect(rect, PAPER);
    let border = (0.4 * millimetre).max(1.0);
    canvas.stroke_rect(rect, border, INK);

    let padding = 2.5 * millimetre;
    let title_size = 5.0 * millimetre;
    let subtitle_size = 3.2 * millimetre;
    let field_label_size = 2.2 * millimetre;
    let field_value_size = 2.8 * millimetre;

    let mut cursor_y = rect.y + padding;
    painter.draw(canvas, &block.title, rect.x + padding, cursor_y, title_size, TextAlign::Left, INK);
    cursor_y += painter.line_height(title_size);
    if !block.subtitle.is_empty() {
        painter.draw(canvas, &block.subtitle, rect.x + padding, cursor_y, subtitle_size, TextAlign::Left, INK);
        cursor_y += painter.line_height(subtitle_size);
    }
    if !block.project.is_empty() {
        painter.draw(canvas, &block.project, rect.x + padding, cursor_y, subtitle_size, TextAlign::Left, INK);
        cursor_y += painter.line_height(subtitle_size);
    }

    // Field strip along the bottom of the block.
    let strip_height = painter.line_height(field_label_size) + painter.line_height(field_value_size) + padding;
    let strip_y = rect.bottom() - strip_height;
    canvas.draw_line((rect.x, strip_y), (rect.right(), strip_y), border, INK);

    let fields = [
        (tr!("plot-scale"), format!("1:{}", format_quantity(scale, 0))),
        (tr!("plot-drawn-by-caps"), block.author.clone()),
        (tr!("plot-date-caps"), block.date.clone()),
        (tr!("plot-drawing-no"), block.drawing_number.clone()),
        (tr!("plot-rev"), block.revision.clone()),
    ];
    let column_width = rect.width / fields.len() as f64;
    for (index, (label, value)) in fields.iter().enumerate() {
        let column_x = rect.x + column_width * index as f64;
        if index > 0 {
            canvas.draw_line((column_x, strip_y), (column_x, rect.bottom()), border * 0.6, INK);
        }
        painter.draw(
            canvas,
            label,
            column_x + padding * 0.6,
            strip_y + padding * 0.3,
            field_label_size,
            TextAlign::Left,
            FAINT_INK,
        );
        painter.draw(
            canvas,
            value,
            column_x + padding * 0.6,
            strip_y + padding * 0.3 + painter.line_height(field_label_size),
            field_value_size,
            TextAlign::Left,
            INK,
        );
    }

    let _ = cursor_y;
}

/// Shorten `text` with a trailing ellipsis until it fits `max_width` pixels.
fn elide_to_width(painter: &mut TextPainter, text: &str, size_px: f64, max_width: f64) -> String {
    if painter.measure(text, size_px) <= max_width {
        return text.to_owned();
    }
    let characters: Vec<char> = text.chars().collect();
    // Binary search the longest prefix that still fits, so a very long name
    // costs a handful of measurements rather than one per character.
    let (mut low, mut high) = (0usize, characters.len());
    while low < high {
        let middle = (low + high).div_ceil(2);
        let candidate: String = characters[..middle].iter().collect::<String>() + "…";
        if painter.measure(&candidate, size_px) <= max_width {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    characters[..low].iter().collect::<String>() + "…"
}

fn rgba_bytes(color: [f32; 4]) -> [u8; 4] {
    [
        (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        255,
    ]
}

/// Encode RGBA8 pixels as a PNG carrying the sheet's physical resolution, so
/// print dialogues and page-layout software place it at true scale.
pub(crate) fn encode_png(canvas: &Canvas, dpi: u32) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, canvas.width(), canvas.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        // pHYs is in pixels per metre.
        let pixels_per_metre = (f64::from(dpi) / MM_PER_INCH * 1000.0).round() as u32;
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: pixels_per_metre,
            yppu: pixels_per_metre,
            unit: png::Unit::Meter,
        }));
        let mut writer = encoder.write_header().map_err(|error| format!("{error}"))?;
        writer.write_image_data(canvas.pixels()).map_err(|error| format!("{error}"))?;
        writer.finish().map_err(|error| format!("{error}"))?;
    }
    Ok(bytes)
}
