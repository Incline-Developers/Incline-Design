//! The readouts that sit at the right end of the bottom toolbar: rings that
//! fill green around their circumference. The background-task ring has the
//! task text and the counts to its left; the memory ring at the very end
//! carries its percentage inside.
//!
//! Painted rather than assembled from `egui::ProgressBar`, for two reasons: a
//! ring is not a shape egui offers at all, and an indeterminate task has to
//! animate without reading as "done", which a chunk of arc sweeping round the
//! ring says and a filled ring cannot.

use thousands::Separable;

use crate::{
    i18n::tr,
    ui::{EditorState, widgets::shifted},
};

/// Space left above and below the ring, which is what sets its diameter: the
/// readout is as tall as the strip allows, less what keeps it off the edges of
/// its own panel. The same clearance the strip's tool icons keep, so the ring
/// sits in the row rather than filling it - and enough that the region's own
/// outline, drawn over the inside of its edge, has nothing to cut into.
const RING_VERTICAL_MARGIN: f32 = 5.0;
/// Range the ring's diameter is held to, whatever the strip gives it. Below
/// the floor the arc is too fine to read as a fill; above the ceiling the
/// readout starts competing with the tools beside it.
const RING_DIAMETER_RANGE: std::ops::RangeInclusive<f32> = 12.0..=22.0;
/// Thickness of the ring, as a share of its diameter, and the points it is
/// held between. Proportional so the ring keeps its weight across the range of
/// diameters the strip can hand it.
const RING_STROKE_FRACTION: f32 = 0.16;
const RING_STROKE_RANGE: std::ops::RangeInclusive<f32> = 2.0..=4.0;
/// Gap between the text and the ring, and between the task text and the counts.
const TEXT_GAP: f32 = 8.0;
/// Room left between the ring and the end of the strip. The region's own
/// outline is drawn over the inside of its edge, so a ring sitting flush
/// against it comes back cut down one side.
pub(crate) const END_INSET: f32 = 6.0;
/// Gap between the task readout and the memory ring to its right.
pub(crate) const READOUT_GAP: f32 = 10.0;
/// Least room the task text is given before it is dropped in favour of the
/// counts: below this an elided label is all ellipsis and says nothing.
const TASK_MIN_WIDTH: f32 = 48.0;
/// Starting size of the memory ring's figure, as a share of the hole it sits
/// in, and the size it is never shrunk below to fit.
const MEMORY_TEXT_FRACTION: f32 = 0.62;
const MEMORY_TEXT_MIN: f32 = 6.0;
/// One full turn of the indeterminate chunk, in seconds.
const SPIN_PERIOD: f64 = 1.4;
/// Share of the circumference the indeterminate chunk covers.
const SPIN_FRACTION: f32 = 0.25;

/// The unfilled part of the ring: the toolbar's own grey, moved a few levels
/// off it. The readout is something to glance at rather than something to look
/// at, so its track stays within a narrow band of the surface it sits on.
const TRACK_SHIFT_DARK: i16 = -10;
const TRACK_SHIFT_LIGHT: i16 = -18;

/// The filled part of the ring. Darkened in the light theme so it holds its
/// own against a bright surface rather than glowing off it.
const FILL_DARK: egui::Color32 = egui::Color32::from_rgb(0x50, 0xC8, 0x6E);
const FILL_LIGHT: egui::Color32 = egui::Color32::from_rgb(0x2E, 0x96, 0x4C);

/// Text style beside the ring: the same as the status bar's readouts, which is
/// what this is.
const TEXT_STYLE: egui::TextStyle = egui::TextStyle::Body;

/// How the ring's fill is laid out this frame.
enum RingFill {
    /// Filled clockwise from twelve o'clock to `0.0..=1.0` of the way round.
    Fraction(f32),
    /// A chunk of arc turning about the ring: a running task with no percentage.
    Spinner,
}

/// Draw the task progress readout for whatever the editor is reporting.
///
/// Once the first task of the session has run the readout stays put, sitting
/// at a full ring with the last task's "…: Finished" text; before then there
/// is nothing to report and nothing is drawn. The toolbar has a fixed height,
/// so the empty case needn't reserve any space.
///
/// `end_inset` is the room left on its right: the end of the strip's, when
/// it is last on the bar, or the gap to the readout beside it.
pub(crate) fn draw_task_progress(ui: &mut egui::Ui, editor: &EditorState, end_inset: f32) {
    match &editor.status_message {
        Some(message) => match message.progress {
            Some(progress) => {
                let progress = progress.clamp(0.0, 1.0);
                draw_ring(ui, &ring_status_text(progress, message.units), &message.text, RingFill::Fraction(progress), end_inset);
            }
            None => draw_ring(ui, "", &message.text, RingFill::Spinner, end_inset),
        },
        // Idle: hold the last task at a full ring. Nothing about the parked
        // readout moves, so it costs no repaints.
        None => {
            if let Some(finished) = &editor.last_finished_task {
                let status = ring_status_text(1.0, finished.total_units.map(|total| (total, total)));
                draw_ring(
                    ui,
                    &status,
                    &tr!("progress-task-finished", task = finished.text.to_string()),
                    RingFill::Fraction(1.0),
                    end_inset,
                );
            }
        }
    }
}

/// The counts beside the ring: percentage, plus items when the task reports them.
fn ring_status_text(fraction: f32, units: Option<(u64, u64)>) -> String {
    let percent = format!("{:.0}%", fraction * 100.0);
    match units {
        Some((done, total)) => tr!(
            "progress-percent-done-total",
            percent = percent.to_string(),
            done = done.separate_with_commas().to_string(),
            total = total.separate_with_commas().to_string()
        ),
        None => percent,
    }
}

/// Paint the readout: the texts, then the ring at the right end of them.
///
/// Only as wide as it needs to be. The strip lays it out right to left, so the
/// ring keeps one place at the end of the toolbar and the text grows leftwards
/// away from it - the counts nearest the ring, the task label beyond them.
fn draw_ring(ui: &mut egui::Ui, status: &str, task: &str, fill: RingFill, end_inset: f32) {
    let available = ui.available_width();
    let diameter = ring_diameter(ui);
    if available < diameter + end_inset {
        return;
    }

    // What the ring leaves is the text's, and the counts have first claim on
    // it: a percentage with nothing beside it still says how far along the
    // task is, where a label with no percentage does not.
    let font = TEXT_STYLE.resolve(ui.style());
    let painter = ui.painter().clone();
    let mut text_budget = available - diameter - end_inset - TEXT_GAP;
    let status_galley = (!status.is_empty() && text_budget > 0.0).then(|| {
        // Truncated as well, so a cramped strip can't hand the readout a
        // wider block of text than the toolbar has room for.
        let mut job = egui::text::LayoutJob::simple_singleline(status.to_owned(), font.clone(), egui::Color32::PLACEHOLDER);
        job.wrap = egui::text::TextWrapping::truncate_at_width(text_budget);
        let galley = painter.layout_job(job);
        text_budget -= galley.size().x + TEXT_GAP;
        galley
    });
    let task_galley = (!task.is_empty() && text_budget >= TASK_MIN_WIDTH).then(|| {
        // Elided rather than clipped: a label cut mid-glyph reads as a drawing
        // bug, an ellipsis reads as "there is more, hover for it".
        let mut job = egui::text::LayoutJob::simple_singleline(task.to_owned(), font, egui::Color32::PLACEHOLDER);
        job.wrap = egui::text::TextWrapping::truncate_at_width(text_budget);
        painter.layout_job(job)
    });

    let text_width: f32 = [status_galley.as_ref(), task_galley.as_ref()]
        .into_iter()
        .flatten()
        .map(|galley| galley.size().x + TEXT_GAP)
        .sum();
    let size = egui::vec2(text_width + diameter + end_inset, ui.available_height());
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    // The label is elided to whatever the counts leave it, so a long one - a
    // full export path, say - is only readable on hover.
    response.on_hover_text(task);

    let center = egui::pos2(rect.right() - end_inset - diameter / 2.0, rect.center().y);
    paint_ring(ui, &painter, center, diameter, fill);

    // Both texts sit on the toolbar rather than on the ring, so neither needs
    // to change colour anywhere: the counts take the strong shade, the label
    // the ordinary one, which is the pairing the status bar uses.
    let visuals = ui.visuals();
    let mut right = rect.right() - end_inset - diameter - TEXT_GAP;
    for (galley, color) in [(status_galley, visuals.strong_text_color()), (task_galley, visuals.text_color())] {
        if let Some(galley) = galley {
            let pos = egui::pos2(right - galley.size().x, rect.center().y - galley.size().y / 2.0);
            painter.galley(pos, galley, color);
            right = pos.x - TEXT_GAP;
        }
    }
}

/// The ring's diameter: as tall as the strip allows, less its margins, held
/// to [`RING_DIAMETER_RANGE`].
fn ring_diameter(ui: &egui::Ui) -> f32 {
    (ui.available_height() - 2.0 * RING_VERTICAL_MARGIN).clamp(*RING_DIAMETER_RANGE.start(), *RING_DIAMETER_RANGE.end())
}

/// Thickness of a ring of `diameter`.
fn ring_stroke_width(diameter: f32) -> f32 {
    (diameter * RING_STROKE_FRACTION).clamp(*RING_STROKE_RANGE.start(), *RING_STROKE_RANGE.end())
}

/// Paint the ring itself - its track, then its fill - centred on `center`.
fn paint_ring(ui: &egui::Ui, painter: &egui::Painter, center: egui::Pos2, diameter: f32, fill: RingFill) {
    let visuals = ui.visuals();
    let track = shifted(visuals.panel_fill, if visuals.dark_mode { TRACK_SHIFT_DARK } else { TRACK_SHIFT_LIGHT });
    let filled = if visuals.dark_mode { FILL_DARK } else { FILL_LIGHT };

    let radius = diameter / 2.0;
    let width = ring_stroke_width(diameter);
    // The track is the whole circumference, so the ring reads as a ring at 0%
    // rather than as nothing at all.
    painter.circle_stroke(center, radius - width / 2.0, egui::Stroke::new(width, track));

    let (start, sweep) = match fill {
        // Clockwise from twelve o'clock: y grows downwards, so a positive
        // angle from straight up is already clockwise on screen.
        RingFill::Fraction(fraction) => (0.0, fraction.clamp(0.0, 1.0) * std::f32::consts::TAU),
        RingFill::Spinner => {
            ui.ctx().request_repaint();
            let turns = (ui.input(|i| i.time) / SPIN_PERIOD).rem_euclid(1.0) as f32;
            (turns * std::f32::consts::TAU, SPIN_FRACTION * std::f32::consts::TAU)
        }
    };
    paint_arc(painter, center, radius - width / 2.0, start, sweep, egui::Stroke::new(width, filled));
}

/// Draw the memory readout: a ring filled to the share of the machine's
/// memory in use, with that share written inside it and nothing beside it.
///
/// Sits at the very end of the strip, `END_INSET` from its edge, so the task
/// readout to its left keeps one place whatever it says.
pub(crate) fn draw_memory_usage(ui: &mut egui::Ui, editor: &EditorState) {
    let Some(usage) = editor.memory_usage else {
        return;
    };
    // Rendering is on demand, so an idle window would otherwise hold the
    // first reading forever.
    ui.ctx().request_repaint_after(crate::app::memory_usage::SAMPLE_PERIOD);
    let diameter = ring_diameter(ui);
    if ui.available_width() < diameter + END_INSET {
        return;
    }
    let (rect, response) = ui.allocate_exact_size(egui::vec2(diameter + END_INSET, ui.available_height()), egui::Sense::hover());
    let fraction = usage.fraction();
    let percent = format!("{:.0}", fraction * 100.0);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter().clone();
        let center = egui::pos2(rect.right() - END_INSET - diameter / 2.0, rect.center().y);
        paint_ring(ui, &painter, center, diameter, RingFill::Fraction(fraction));
        // The figure fills the hole: the largest size whose width still
        // clears the ring's inner edge.
        let hole = diameter - 2.0 * ring_stroke_width(diameter);
        let mut size = hole * MEMORY_TEXT_FRACTION;
        let color = ui.visuals().strong_text_color();
        let galley = loop {
            let galley = painter.layout_no_wrap(percent.clone(), egui::FontId::proportional(size), color);
            if galley.size().x <= hole - 1.0 || size <= MEMORY_TEXT_MIN {
                break galley;
            }
            size -= 0.5;
        };
        painter.galley(center - galley.size() / 2.0, galley, color);
    }
    let used = crate::ui::dialogs::triangulation::format_bytes(usage.used);
    let total = crate::ui::dialogs::triangulation::format_bytes(usage.total);
    // A browser does not say what the machine is doing, so the web build
    // reports its own heap instead - see `MemoryUsage`.
    #[cfg(not(target_arch = "wasm32"))]
    let hover = tr!("progress-memory-usage", used = used, total = total, percent = percent);
    #[cfg(target_arch = "wasm32")]
    let hover = tr!("progress-memory-usage-web", used = used, total = total, percent = percent);
    response.on_hover_text(hover);
}

/// Stroke `sweep` radians of arc, clockwise from `start` radians past twelve
/// o'clock.
///
/// Ends in a disc of the stroke's own width, which is what rounds the caps:
/// egui strokes a path with flat ends, and a squared-off leading edge reads as
/// a notch in the ring rather than as where the fill has got to.
fn paint_arc(painter: &egui::Painter, center: egui::Pos2, radius: f32, start: f32, sweep: f32, stroke: egui::Stroke) {
    if sweep <= f32::EPSILON {
        return;
    }
    // A whole turn is a circle: the path would close on itself, and the caps
    // would be drawn one over the other at twelve o'clock.
    if sweep >= std::f32::consts::TAU - f32::EPSILON {
        painter.circle_stroke(center, radius, stroke);
        return;
    }

    let at = |angle: f32| egui::pos2(center.x + radius * angle.sin(), center.y - radius * angle.cos());
    // One segment per few degrees, so the arc stays smooth at the diameters
    // the strip hands it without paying for points a small ring can't show.
    let steps = (sweep / (std::f32::consts::TAU / 64.0)).ceil().max(1.0) as usize;
    let points: Vec<egui::Pos2> = (0..=steps).map(|step| at(start + sweep * step as f32 / steps as f32)).collect();
    painter.circle_filled(points[0], stroke.width / 2.0, stroke.color);
    painter.circle_filled(points[steps], stroke.width / 2.0, stroke.color);
    painter.add(egui::Shape::line(points, stroke));
}
