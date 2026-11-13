//! The playback deck over the viewport: play, back to the start, the step
//! shown, the steps a second, keep the outer blocks, and a strip of the steps
//! coloured by group that scrubs the playhead. The deck only moves the
//! playhead; the model follows through `UiCommand::ApplyFieldAnimation`,
//! sent only when the step shown changes. See `app/commands/field_animation.rs`.

use crate::{
    i18n::tr,
    ui::{
        EditorState,
        state::{FieldAnimation, UiCommand},
        widgets::{
            hint::Hint,
            menu::SlidingToggle,
            playback::{TILE_MARGIN, Transport, tile_frame, transport_button},
            toolbar::GROUP_CORNER_RADIUS,
        },
    },
};

const DECK_MAX_WIDTH: f32 = 720.0;
const STRIP_HEIGHT: f32 = 18.0;
/// Steps per second offered, slowest to fastest.
const SPEED_RANGE: std::ops::RangeInclusive<f64> = 0.1..=500.0;

pub(crate) fn draw_field_animation(ui: &egui::Ui, editor: &mut EditorState, canvas_rect: egui::Rect, commands: &mut Vec<UiCommand>) {
    let Some(animation) = editor.field_animation.as_mut() else {
        return;
    };
    let count = animation.steps.len() as f64;
    if animation.playing {
        let dt = f64::from(ui.ctx().input(|input| input.stable_dt).min(0.1));
        animation.playhead += dt * animation.speed;
        if animation.playhead >= count {
            animation.playhead = count;
            animation.playing = false;
        }
        ui.ctx().request_repaint();
    }
    let mut close = false;
    let width = (canvas_rect.width() - 2.0 * TILE_MARGIN).clamp(320.0, DECK_MAX_WIDTH);
    egui::Area::new(egui::Id::unique("field_animation_deck"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(canvas_rect.center_bottom() - egui::vec2(0.0, TILE_MARGIN))
        .show(ui.ctx(), |ui| {
            tile_frame(ui.visuals()).show(ui, |ui| {
                ui.set_width(width - 20.0);
                ui.horizontal(|ui| {
                    if animation.playing {
                        if transport_button(ui, Transport::Pause, None).clicked() {
                            animation.playing = false;
                        }
                    } else if transport_button(ui, Transport::Play, None).clicked() {
                        if animation.playhead >= count {
                            animation.playhead = 0.0;
                        }
                        animation.playing = true;
                    }
                    if transport_button(ui, Transport::Rewind, None).clicked() {
                        animation.playhead = 0.0;
                        animation.playing = false;
                    }
                    let done = animation.done();
                    ui.label(egui::RichText::new(tr!("anim-step-of", step = done.to_string(), count = animation.steps.len().to_string())).strong());
                    let current = animation.steps.get(done.saturating_sub(1)).filter(|_| done > 0);
                    ui.label(egui::RichText::new(current.map_or_else(|| animation.title.clone(), |step| step.label.clone())).weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        close = transport_button(ui, Transport::Close, None).clicked();
                        ui.add_space(8.0);
                        ui.add(
                            egui::DragValue::new(&mut animation.speed)
                                .range(SPEED_RANGE)
                                .speed(0.1)
                                .max_decimals(1)
                                .suffix(format!(" {}", tr!("anim-steps-per-second"))),
                        )
                        .hint(tr!("anim-speed-hint"));
                        ui.add_space(8.0);
                        // Right to left: the switch, then its label before it.
                        ui.add(SlidingToggle::new(&mut animation.keep_outer)).hint(tr!("anim-keep-outer-hint"));
                        ui.label(tr!("anim-keep-outer")).hint(tr!("anim-keep-outer-hint"));
                    });
                });
                ui.add_space(6.0);
                if let Some(playhead) = strip(ui, animation) {
                    animation.playhead = playhead;
                    animation.playing = false;
                }
            });
        });
    if close || ui.ctx().input(|input| input.key_pressed(egui::Key::Escape)) {
        commands.push(UiCommand::CloseFieldAnimation);
    } else if animation.stale() {
        commands.push(UiCommand::ApplyFieldAnimation);
    }
}

/// The steps as a strip, each coloured by its group, the ones done dimmed;
/// a click or drag puts the playhead there.
fn strip(ui: &mut egui::Ui, animation: &FieldAnimation) -> Option<f64> {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), STRIP_HEIGHT), egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.rect_filled(rect, GROUP_CORNER_RADIUS, visuals.extreme_bg_color);
    let count = animation.steps.len().max(1) as f32;
    let x = |step: f32| rect.left() + step / count * rect.width();
    let done = animation.done();
    // One rectangle per run of steps of a group.
    let mut start = 0;
    for step in 1..=animation.steps.len() {
        if step < animation.steps.len() && animation.steps[step].group == animation.steps[start].group {
            continue;
        }
        let [r, g, b, _] = animation.steps[start].color;
        let color = egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
        let band = egui::Rect::from_x_y_ranges(x(start as f32)..=x(step as f32), rect.y_range().shrink(3.0));
        painter.rect_filled(band, 0.0, color);
        start = step;
    }
    if done > 0 {
        let gone = egui::Rect::from_x_y_ranges(rect.left()..=x(done as f32), rect.y_range());
        painter.rect_filled(gone, GROUP_CORNER_RADIUS, visuals.extreme_bg_color.gamma_multiply(0.75));
    }
    let head = x(animation.playhead as f32);
    painter.line_segment(
        [egui::pos2(head, rect.top()), egui::pos2(head, rect.bottom())],
        egui::Stroke::new(2.0, visuals.strong_text_color()),
    );
    painter.rect_stroke(rect, GROUP_CORNER_RADIUS, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
    let response = response.hint(tr!("anim-strip-hint"));
    (response.dragged() || response.clicked())
        .then(|| response.interact_pointer_pos())
        .flatten()
        .map(|pointer| (((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0) * count).round() as f64)
}
