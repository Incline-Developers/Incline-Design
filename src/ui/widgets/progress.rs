//! The status bar's task readout: a ring per running task that fills green
//! around its circumference, with the oldest task's text to their left: its
//! label and counts. Clicked, it lists the last few tasks. The bar's memory
//! readout borrows the same ring.
//!
//! Painted rather than assembled from `egui::ProgressBar`, for two reasons: a
//! ring is not a shape egui offers at all, and an indeterminate task has to
//! animate without reading as "done", which a chunk of arc sweeping round the
//! ring says and a filled ring cannot.

use thousands::Separable;

use super::{context_menu::dropdown_menu, hint::Hint, toolbar::GROUP_CORNER_RADIUS};
use crate::{
    i18n::tr,
    ui::{EditorState, state::TASK_HISTORY_LEN, widgets::shifted},
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
/// Least room the task text is given before it is dropped in favour of the
/// counts: below this an elided label is all ellipsis and says nothing.
const TASK_MIN_WIDTH: f32 = 48.0;
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
pub(crate) const FILL_DARK: egui::Color32 = egui::Color32::from_rgb(0x50, 0xC8, 0x6E);
pub(crate) const FILL_LIGHT: egui::Color32 = egui::Color32::from_rgb(0x2E, 0x96, 0x4C);

/// Text style beside the ring: the same as the status bar's readouts, which is
/// what this is.
const TEXT_STYLE: egui::TextStyle = egui::TextStyle::Body;

/// Space between one task's ring and the next.
const RING_GAP: f32 = 4.0;
/// Space either side of the readout's contents, so its hover fill reads as a
/// button rather than as a box drawn tight around the rings. The status bar
/// gives it back beside the rule before it, as it does the language picker's.
pub(crate) const READOUT_PADDING: f32 = 6.0;

/// How long a finished task's ring stays full before it goes, then how long it
/// takes to fade, then how long the rings to its left take to slide over into
/// its place. A cancelled task's ring skips the hold: it did not finish.
const HOLD_SECONDS: f64 = 1.0;
const FADE_SECONDS: f64 = 0.35;
const SLIDE_SECONDS: f64 = 0.3;
/// How long a new task's ring takes to open its place and fade in.
const APPEAR_SECONDS: f64 = 0.25;

/// The list the readout opens: its width, the height of each task's row, the
/// diameter of the ring at a row's end and the space between a row's parts.
const LIST_WIDTH: f32 = 300.0;
const LIST_ROW_HEIGHT: f32 = 22.0;
const LIST_RING_DIAMETER: f32 = 14.0;
const LIST_GAP: f32 = 10.0;

/// How the ring's fill is laid out this frame.
#[derive(Clone, Copy)]
enum RingFill {
    /// Filled clockwise from twelve o'clock to `0.0..=1.0` of the way round.
    Fraction(f32),
    /// A chunk of arc turning about the ring: a running task with no percentage.
    Spinner,
}

impl RingFill {
    fn of(fraction: Option<f32>) -> Self {
        fraction.map_or(Self::Spinner, Self::Fraction)
    }
}

/// Where one ring is in its life.
#[derive(Clone, Copy, PartialEq)]
enum SlotState {
    /// Its task is running.
    Running,
    /// Its task was the last to finish and nothing else is running, so the
    /// readout holds it, full, until another task begins.
    Parked,
    /// Going: held full for `hold` seconds from `since`, then faded, then
    /// closed up.
    Leaving { since: f64, hold: f64 },
}

/// One task's ring in the readout.
#[derive(Clone)]
struct Slot {
    ticket: u64,
    fraction: Option<f32>,
    /// When the ring first showed, in egui's clock.
    born: f64,
    state: SlotState,
}

impl Slot {
    fn leave(&mut self, now: f64, hold: f64) {
        if !matches!(self.state, SlotState::Leaving { .. }) {
            self.state = SlotState::Leaving { since: now, hold };
        }
    }

    /// Share of a ring's place this one takes: growing in as it appears,
    /// closing up once it has faded.
    fn width(&self, now: f64) -> f32 {
        let closed = match self.state {
            SlotState::Leaving { since, hold } => ease((now - since - hold - FADE_SECONDS) / SLIDE_SECONDS),
            _ => 0.0,
        };
        ease((now - self.born) / APPEAR_SECONDS) * (1.0 - closed)
    }

    fn alpha(&self, now: f64) -> f32 {
        let faded = match self.state {
            SlotState::Leaving { since, hold } => ease((now - since - hold) / FADE_SECONDS),
            _ => 0.0,
        };
        ease((now - self.born) / APPEAR_SECONDS) * (1.0 - faded)
    }

    fn gone(&self, now: f64) -> bool {
        matches!(self.state, SlotState::Leaving { since, hold } if now - since >= hold + FADE_SECONDS + SLIDE_SECONDS)
    }

    fn moving(&self, now: f64) -> bool {
        now - self.born < APPEAR_SECONDS || matches!(self.state, SlotState::Leaving { .. })
    }
}

/// Smoothstep of `t` held to `0.0..=1.0`.
fn ease(t: f64) -> f32 {
    let t = t.clamp(0.0, 1.0) as f32;
    t * t * (3.0 - 2.0 * t)
}

/// The readout's rings, oldest at the left, kept in egui's memory from frame
/// to frame: what they animate is the UI's own business, not the editor's.
#[derive(Clone, Default)]
struct Deck {
    slots: Vec<Slot>,
}

impl Deck {
    /// Bring the rings up to date with the tasks the editor reports.
    fn sync(&mut self, editor: &EditorState, now: f64) {
        // A ring whose task is no longer running either finished, and goes
        // full first, or was cancelled, and goes at once.
        for slot in &mut self.slots {
            if slot.state == SlotState::Running && !editor.status_tasks.iter().any(|task| task.ticket == slot.ticket) {
                if editor.task_history.iter().any(|task| task.ticket == slot.ticket) {
                    slot.fraction = Some(1.0);
                    slot.leave(now, HOLD_SECONDS);
                } else {
                    slot.leave(now, 0.0);
                }
            }
        }
        for task in &editor.status_tasks {
            match self.slots.iter_mut().find(|slot| slot.ticket == task.ticket) {
                Some(slot) => slot.fraction = task.progress.map(|fraction| fraction.clamp(0.0, 1.0)),
                None => self.slots.push(Slot {
                    ticket: task.ticket,
                    fraction: task.progress,
                    born: now,
                    state: SlotState::Running,
                }),
            }
        }
        if !editor.status_tasks.is_empty() {
            for slot in &mut self.slots {
                if slot.state == SlotState::Parked {
                    slot.leave(now, 0.0);
                }
            }
        } else if !self.slots.iter().any(|slot| slot.state == SlotState::Parked)
            && let Some(last) = editor.task_history.front()
        {
            // Nothing running: the last task to finish stays, full.
            match self.slots.iter_mut().find(|slot| slot.ticket == last.ticket) {
                Some(slot) => {
                    slot.state = SlotState::Parked;
                    slot.fraction = Some(1.0);
                }
                None => self.slots.push(Slot {
                    ticket: last.ticket,
                    fraction: Some(1.0),
                    born: now,
                    state: SlotState::Parked,
                }),
            }
        }
        self.slots.retain(|slot| !slot.gone(now));
    }
}

/// Draw the task progress readout for whatever the editor is reporting: a
/// ring for every task running, oldest at the left, with the oldest's label
/// and counts beside it. A finished task's ring stays full for a moment, then
/// fades, and the rings to its left slide over into its place. Clicking the
/// readout lists the last few tasks.
///
/// Once the first task of the session has run the readout stays put, sitting
/// at a full ring with the last task's "…: Finished" text; before then there
/// is nothing to report and nothing is drawn (see [`has_task`]).
pub(crate) fn draw_task_progress(ui: &mut egui::Ui, editor: &EditorState) {
    let now = ui.input(|input| input.time);
    let id = egui::Id::unique("status_task_rings");
    let mut deck = ui.ctx().data_mut(|data| data.get_temp::<Deck>(id)).unwrap_or_default();
    deck.sync(editor, now);
    if deck.slots.iter().any(|slot| slot.moving(now)) {
        ui.ctx().request_repaint();
    }

    let (status, task) = if let Some(task) = editor.status_tasks.first() {
        let status = task.progress.map_or_else(String::new, |progress| ring_status_text(progress.clamp(0.0, 1.0), task.units));
        (status, task.text.clone())
    } else if let Some(finished) = editor.task_history.front() {
        let status = ring_status_text(1.0, finished.total_units.map(|total| (total, total)));
        (status, tr!("progress-task-finished", task = finished.text.to_string()))
    } else {
        (String::new(), String::new())
    };
    let response = draw_readout(ui, &deck, now, &status, &task);
    ui.ctx().data_mut(|data| data.insert_temp(id, deck));
    if let Some(response) = response {
        dropdown_menu(&response, tr!("progress-recent-tasks"), LIST_WIDTH, |ui| draw_task_list(ui, editor));
    }
}

/// Whether there is a task to read out: one running, or one that finished.
pub(crate) fn has_task(editor: &EditorState) -> bool {
    !editor.status_tasks.is_empty() || !editor.task_history.is_empty()
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

/// Paint the readout: the texts, then the rings at the right end of them.
///
/// Only as wide as it needs to be. The bar lays it out right to left, so the
/// rings keep their place beside the readouts after them and the text grows
/// leftwards away from them - the counts nearest the rings, the task label
/// beyond them.
fn draw_readout(ui: &mut egui::Ui, deck: &Deck, now: f64, status: &str, task: &str) -> Option<egui::Response> {
    let available = ui.available_width() - 2.0 * READOUT_PADDING;
    let diameter = ring_diameter(ui);
    if available < diameter {
        return None;
    }
    let pitches: Vec<f32> = deck.slots.iter().map(|slot| slot.width(now) * (diameter + RING_GAP)).collect();
    let rings_width = (pitches.iter().sum::<f32>() - RING_GAP).max(0.0);

    // What the rings leave is the text's, and the counts have first claim on
    // it: a percentage with nothing beside it still says how far along the
    // task is, where a label with no percentage does not.
    let font = TEXT_STYLE.resolve(ui.style());
    let painter = ui.painter().clone();
    let mut text_budget = available - rings_width - TEXT_GAP;
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
    let size = egui::vec2(text_width + rings_width + 2.0 * READOUT_PADDING, ui.available_height());
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return Some(response);
    }
    // The label is elided to whatever the counts leave it, so a long one - a
    // full export path, say - is only readable on hover. One shown whole
    // has nothing more to say there.
    let response = if !task.is_empty() && task_galley.as_ref().is_none_or(|galley| galley.elided) {
        response.hint(task)
    } else {
        response
    };
    if response.hovered() {
        painter.rect_filled(
            rect.shrink2(egui::vec2(0.0, RING_VERTICAL_MARGIN / 2.0)),
            GROUP_CORNER_RADIUS,
            ui.visuals().widgets.hovered.bg_fill,
        );
    }

    // Newest at the right end, each older one to the left of it.
    let filled = ring_fill(ui.visuals());
    let mut right = rect.right() - READOUT_PADDING;
    for (slot, pitch) in deck.slots.iter().zip(&pitches).rev() {
        let center = egui::pos2(right - diameter / 2.0, rect.center().y);
        paint_ring_in(ui, &painter, center, diameter, RingFill::of(slot.fraction), filled, slot.alpha(now));
        right -= pitch;
    }

    // Both texts sit on the toolbar rather than on a ring, so neither needs
    // to change colour anywhere: the counts take the strong shade, the label
    // the ordinary one, which is the pairing the status bar uses.
    let visuals = ui.visuals();
    let mut right = rect.right() - READOUT_PADDING - rings_width - TEXT_GAP;
    for (galley, color) in [(status_galley, visuals.strong_text_color()), (task_galley, visuals.text_color())] {
        if let Some(galley) = galley {
            let pos = egui::pos2(right - galley.size().x, rect.center().y - galley.size().y / 2.0);
            painter.galley(pos, galley, color);
            right = pos.x - TEXT_GAP;
        }
    }
    Some(response)
}

/// The list the readout opens: the tasks running, newest first, then the last
/// to finish, up to [`TASK_HISTORY_LEN`] in all. Each row names the task, says
/// how long it has run or ran, and ends in its ring.
fn draw_task_list(ui: &mut egui::Ui, editor: &EditorState) {
    let running = editor
        .status_tasks
        .iter()
        .rev()
        .map(|task| (task.text.as_str(), task.started.elapsed(), RingFill::of(task.progress)));
    let finished = editor.task_history.iter().map(|task| (task.text.as_str(), task.duration, RingFill::Fraction(1.0)));
    for (name, duration, fill) in running.chain(finished).take(TASK_HISTORY_LEN) {
        draw_task_row(ui, name, &duration_label(duration), fill);
    }
}

fn draw_task_row(ui: &mut egui::Ui, name: &str, time: &str, fill: RingFill) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), LIST_ROW_HEIGHT), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    let visuals = ui.visuals();
    let center = egui::pos2(rect.right() - LIST_RING_DIAMETER / 2.0, rect.center().y);
    paint_ring_in(ui, painter, center, LIST_RING_DIAMETER, fill, ring_fill(visuals), 1.0);

    let font = TEXT_STYLE.resolve(ui.style());
    let time_galley = painter.layout_no_wrap(time.to_owned(), font.clone(), egui::Color32::PLACEHOLDER);
    let time_pos = egui::pos2(
        rect.right() - LIST_RING_DIAMETER - LIST_GAP - time_galley.size().x,
        rect.center().y - time_galley.size().y / 2.0,
    );
    let mut job = egui::text::LayoutJob::simple_singleline(name.to_owned(), font, egui::Color32::PLACEHOLDER);
    job.wrap = egui::text::TextWrapping::truncate_at_width((time_pos.x - LIST_GAP - rect.left()).max(0.0));
    let name_galley = painter.layout_job(job);
    let name_pos = egui::pos2(rect.left(), rect.center().y - name_galley.size().y / 2.0);
    painter.galley(name_pos, name_galley, visuals.strong_text_color());
    painter.galley(time_pos, time_galley, visuals.weak_text_color());
}

/// How long a task ran, to the second under an hour and to the minute past it.
fn duration_label(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 1 {
        tr!("progress-duration-under-second")
    } else if seconds < 60 {
        tr!("progress-duration-seconds", seconds = seconds.to_string())
    } else if seconds < 3600 {
        tr!("progress-duration-minutes", minutes = (seconds / 60).to_string(), seconds = (seconds % 60).to_string())
    } else {
        tr!("progress-duration-hours", hours = (seconds / 3600).to_string(), minutes = (seconds / 60 % 60).to_string())
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

/// A ring like the task readout's, filled `fraction` of the way round in
/// `color` - the status bar's memory readout, which colours by how much is
/// left.
pub(crate) fn paint_fraction_ring(ui: &egui::Ui, painter: &egui::Painter, center: egui::Pos2, diameter: f32, fraction: f32, color: egui::Color32) {
    paint_ring_in(ui, painter, center, diameter, RingFill::Fraction(fraction), color, 1.0);
}

/// The unfilled part of a ring.
fn track_color(visuals: &egui::Visuals) -> egui::Color32 {
    shifted(visuals.panel_fill, if visuals.dark_mode { TRACK_SHIFT_DARK } else { TRACK_SHIFT_LIGHT })
}

/// Paint a ring - its track, then its fill - centred on `center`, at `alpha`
/// of its full strength.
fn paint_ring_in(ui: &egui::Ui, painter: &egui::Painter, center: egui::Pos2, diameter: f32, fill: RingFill, filled: egui::Color32, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let track = track_color(ui.visuals()).gamma_multiply(alpha);
    let filled = filled.gamma_multiply(alpha);

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

/// A ring of `diameter` on its own, where the layout puts it: filled to
/// `fraction` of the way round, or turning while there is none to show.
pub(crate) fn ring(ui: &mut egui::Ui, diameter: f32, fraction: Option<f32>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(diameter, diameter), egui::Sense::hover());
    let visuals = ui.visuals();
    let track = shifted(visuals.panel_fill, if visuals.dark_mode { TRACK_SHIFT_DARK } else { TRACK_SHIFT_LIGHT });
    let width = (diameter * RING_STROKE_FRACTION).clamp(*RING_STROKE_RANGE.start(), *RING_STROKE_RANGE.end());
    let radius = (diameter - width) / 2.0;
    let painter = ui.painter();
    painter.circle_stroke(rect.center(), radius, egui::Stroke::new(width, track));
    let (start, sweep) = match fraction {
        Some(fraction) => (0.0, fraction.clamp(0.0, 1.0) * std::f32::consts::TAU),
        None => {
            ui.ctx().request_repaint();
            let turns = (ui.input(|i| i.time) / SPIN_PERIOD).rem_euclid(1.0) as f32;
            (turns * std::f32::consts::TAU, SPIN_FRACTION * std::f32::consts::TAU)
        }
    };
    paint_arc(painter, rect.center(), radius, start, sweep, egui::Stroke::new(width, ring_fill(visuals)));
    response
}

/// The green a ring fills with, for a figure drawn beside one to match.
pub(crate) fn ring_fill(visuals: &egui::Visuals) -> egui::Color32 {
    if visuals.dark_mode { FILL_DARK } else { FILL_LIGHT }
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
