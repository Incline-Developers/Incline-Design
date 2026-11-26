//! A labelled coordinate grid over a plan picture, for `render_view`.
//!
//! Positions read off a bare picture are guesses. With grid lines labelled
//! with their eastings and northings, the AI reads where things are against
//! them, as a person would off a plotted plan.

use std::cell::RefCell;

use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};

use crate::rendering::graphics::offscreen::ViewportPixels;

/// About how many grid lines cross the picture's longer side, when the
/// spacing is left to suit it.
const LINES_ACROSS: f64 = 6.0;
/// Label text size and line height, in pixels.
const LABEL_SIZE: f32 = 13.0;
const LABEL_LINE: f32 = 16.0;
/// Room round a label inside its backing.
const LABEL_PAD: i32 = 3;
/// How far a label sits from its line and the picture's edge.
const LABEL_GAP: i32 = 4;
/// Grid lines: dark, with a light edge either side so they show on any
/// ground.
const LINE: [u8; 3] = [24, 24, 24];
const LINE_ALPHA: f32 = 0.6;
const EDGE: [u8; 3] = [255, 255, 255];
const EDGE_ALPHA: f32 = 0.35;
/// Labels: dark text on a light backing.
const LABEL_TEXT: [u8; 3] = [16, 16, 16];
const LABEL_BACKING: [u8; 3] = [255, 255, 255];
const LABEL_BACKING_ALPHA: f32 = 0.85;

thread_local! {
    /// Fonts for the labels, loaded the first time a grid is drawn.
    static FONTS: RefCell<Option<(FontSystem, SwashCache)>> = const { RefCell::new(None) };
}

/// The world coordinates of a plan picture's edges.
#[derive(Clone, Copy, Debug)]
pub(super) struct PlanEdges {
    pub(super) west: f64,
    pub(super) east: f64,
    pub(super) south: f64,
    pub(super) north: f64,
}

/// Draw grid lines every `spacing` (one to suit the picture if `None`) over
/// `pixels`, each labelled with its easting along the top or its northing
/// down the left.
pub(super) fn draw(pixels: &mut ViewportPixels, plan: PlanEdges, spacing: Option<f64>) {
    let (width, height) = (pixels.width as i32, pixels.height as i32);
    let span = (plan.east - plan.west).max(plan.north - plan.south);
    if !(span.is_finite() && span > 0.0) {
        return;
    }
    let spacing = spacing.unwrap_or_else(|| suited_spacing(span));
    let to_x = |easting: f64| ((easting - plan.west) / (plan.east - plan.west) * f64::from(width)).round() as i32;
    let to_y = |northing: f64| ((plan.north - northing) / (plan.north - plan.south) * f64::from(height)).round() as i32;
    let eastings = lines(plan.west, plan.east, spacing);
    let northings = lines(plan.south, plan.north, spacing);
    let mut canvas = Canvas { pixels };
    for &easting in &eastings {
        let x = to_x(easting);
        canvas.column(x - 1, EDGE, EDGE_ALPHA);
        canvas.column(x + 1, EDGE, EDGE_ALPHA);
        canvas.column(x, LINE, LINE_ALPHA);
    }
    for &northing in &northings {
        let y = to_y(northing);
        canvas.row(y - 1, EDGE, EDGE_ALPHA);
        canvas.row(y + 1, EDGE, EDGE_ALPHA);
        canvas.row(y, LINE, LINE_ALPHA);
    }
    let decimals = decimals(spacing);
    FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        let (font_system, cache) = fonts.get_or_insert_with(|| (crate::fonts::cosmic_text_font_system(), SwashCache::new()));
        for &easting in &eastings {
            let text = format!("E {easting:.decimals$}");
            canvas.label(font_system, cache, &text, to_x(easting) + LABEL_GAP, LABEL_GAP);
        }
        for &northing in &northings {
            let text = format!("N {northing:.decimals$}");
            canvas.label(font_system, cache, &text, LABEL_GAP, to_y(northing) - LABEL_GAP - LABEL_LINE as i32 - 2 * LABEL_PAD);
        }
    });
}

/// A round spacing (1, 2 or 5 of a power of ten) giving about
/// [`LINES_ACROSS`] lines over `span`.
fn suited_spacing(span: f64) -> f64 {
    let rough = span / LINES_ACROSS;
    let power = 10f64.powf(rough.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|step| step * power)
        .find(|step| *step >= rough)
        .unwrap_or(10.0 * power)
}

/// Every multiple of `spacing` from `low` to `high`.
fn lines(low: f64, high: f64, spacing: f64) -> Vec<f64> {
    let first = (low / spacing).ceil() as i64;
    let last = (high / spacing).floor() as i64;
    (first..=last).map(|index| index as f64 * spacing).collect()
}

/// How many decimals a label needs to tell lines `spacing` apart.
fn decimals(spacing: f64) -> usize {
    if spacing >= 1.0 { 0 } else { (-spacing.log10()).ceil().max(0.0) as usize }
}

/// The picture being drawn on.
struct Canvas<'a> {
    pixels: &'a mut ViewportPixels,
}

impl Canvas<'_> {
    /// Blend `colour` at `alpha` into the pixel at `x`, `y`, if it is in the
    /// picture.
    fn blend(&mut self, x: i32, y: i32, colour: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x >= self.pixels.width as i32 || y >= self.pixels.height as i32 {
            return;
        }
        let at = (y as usize * self.pixels.width as usize + x as usize) * 4;
        for (channel, value) in colour.into_iter().enumerate() {
            let old = f32::from(self.pixels.rgba[at + channel]);
            self.pixels.rgba[at + channel] = (old + (f32::from(value) - old) * alpha).round() as u8;
        }
    }

    fn column(&mut self, x: i32, colour: [u8; 3], alpha: f32) {
        for y in 0..self.pixels.height as i32 {
            self.blend(x, y, colour, alpha);
        }
    }

    fn row(&mut self, y: i32, colour: [u8; 3], alpha: f32) {
        for x in 0..self.pixels.width as i32 {
            self.blend(x, y, colour, alpha);
        }
    }

    /// `text` on a light backing whose top left corner is at `x`, `y`.
    fn label(&mut self, font_system: &mut FontSystem, cache: &mut SwashCache, text: &str, x: i32, y: i32) {
        let mut buffer = Buffer::new(font_system, Metrics::new(LABEL_SIZE, LABEL_LINE));
        buffer.set_size(None, None);
        buffer.set_text(text, &Attrs::new().family(Family::SansSerif), Shaping::Advanced, None);
        buffer.shape_until_scroll(font_system, false);
        let text_width = buffer.layout_runs().map(|run| run.line_w).fold(0.0f32, f32::max).ceil() as i32;
        for row in y..y + LABEL_LINE as i32 + 2 * LABEL_PAD {
            for column in x..x + text_width + 2 * LABEL_PAD {
                self.blend(column, row, LABEL_BACKING, LABEL_BACKING_ALPHA);
            }
        }
        let [red, green, blue] = LABEL_TEXT;
        buffer.draw(font_system, cache, Color::rgb(red, green, blue), |left, top, glyph_width, glyph_height, colour| {
            let alpha = f32::from(colour.a()) / 255.0;
            if alpha <= 0.0 {
                return;
            }
            for row in 0..glyph_height as i32 {
                for column in 0..glyph_width as i32 {
                    self.blend(x + LABEL_PAD + left + column, y + LABEL_PAD + top + row, [colour.r(), colour.g(), colour.b()], alpha);
                }
            }
        });
    }
}
