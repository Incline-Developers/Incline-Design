//! The window's title bar, which Incline draws itself.
//!
//! The menu bar is the title bar: it spans the window, carries the menus and
//! the workspace tabs, and around them this module adds what a system title
//! bar would have had. The open project's name in the middle, the window
//! buttons on the right, a drag on bare bar moving the window, a double-click
//! maximising it and a right-click opening the system's window menu. Off the
//! bar, the window's edges resize it.
//!
//! macOS keeps its own red, yellow and green buttons, placed over the bar by
//! `mac::place_window_buttons`, and the bar leaves room for them
//! ([`WindowFrame::leading_inset`]). In the browser there is no window to
//! manage, and only the name is drawn.
//!
//! Nothing here touches the window: each gesture is a [`WindowAction`], which
//! `App::apply_window_action` carries out.

#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
use crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS;
use crate::ui::{
    UiCommand,
    state::{UiProjectView, WindowAction, WindowFrame},
};

/// Width of a window button's tile.
#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
const BUTTON_WIDTH: f32 = 36.0;
/// Space between two window buttons' tiles.
#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
const BUTTON_GAP: f32 = 2.0;
/// Side of the glyph drawn in a window button.
#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
const GLYPH_SIZE: f32 = 10.0;
/// How far into the window its edges resize it rather than reach the UI.
#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
const RESIZE_EDGE: f32 = 4.0;
/// Side of the square at each corner that resizes both ways at once.
#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
const RESIZE_CORNER: f32 = 10.0;
/// Space kept clear on either side of the project's name.
const TITLE_MARGIN: f32 = 16.0;
/// Narrower than this, the name is left out rather than cut down to nothing.
const TITLE_MIN_WIDTH: f32 = 48.0;

/// Take a drag, double-click or right-click on the bar's bare background as
/// the window's own.
///
/// Called before anything else on the bar is laid out, so every menu, tab and
/// button laid over it takes its own pointer first.
pub(crate) fn interact_background(ui: &mut egui::Ui, bar: egui::Rect, commands: &mut Vec<UiCommand>) {
    if cfg!(target_arch = "wasm32") {
        return;
    }
    let response = ui.interact(bar, ui.scope_id().with("title_bar_background"), egui::Sense::click_and_drag());
    if response.drag_started_by(egui::PointerButton::Primary) {
        commands.push(UiCommand::Window(WindowAction::Drag));
    } else if response.double_clicked() {
        commands.push(UiCommand::Window(WindowAction::TitleDoubleClick));
    } else if response.secondary_clicked()
        && let Some(position) = response.interact_pointer_pos()
    {
        commands.push(UiCommand::Window(WindowAction::ShowMenu(position)));
    }
}

/// Draw the minimise, maximise and close buttons at the right of the bar, and
/// return the left edge of what they took.
///
/// Each button's tile sits on the row, in the house corner radius, but the
/// button takes the pointer over the bar's whole height, and the last one out
/// to the window's edge: a pointer thrown into the corner of a maximised
/// window closes it.
pub(crate) fn draw_window_controls(ui: &mut egui::Ui, bar: egui::Rect, row: egui::Rect, frame: &mut WindowFrame, commands: &mut Vec<UiCommand>) -> f32 {
    frame.maximize_button = None;
    #[cfg(any(target_os = "macos", target_arch = "wasm32"))]
    {
        let _ = (ui, row, commands);
        bar.right()
    }
    #[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
    {
        let mut right = row.right();
        let mut hit_right = bar.right();
        let mut left = right;
        for control in [Control::Close, Control::Maximize, Control::Minimize] {
            let tile = egui::Rect::from_center_size(egui::pos2(right - BUTTON_WIDTH / 2.0, row.center().y), egui::vec2(BUTTON_WIDTH, row.height()));
            let hit = egui::Rect::from_x_y_ranges(tile.left() - BUTTON_GAP / 2.0..=hit_right, bar.y_range());
            let response = draw_control(ui, control, hit, tile, frame);
            if response.clicked() {
                commands.push(match control {
                    Control::Close => UiCommand::RequestExit,
                    Control::Maximize => UiCommand::Window(WindowAction::ToggleMaximize),
                    Control::Minimize => UiCommand::Window(WindowAction::Minimize),
                });
            }
            if control == Control::Maximize {
                frame.maximize_button = Some(hit);
            }
            left = hit.left();
            hit_right = hit.left();
            right = tile.left() - BUTTON_GAP;
        }
        left
    }
}

#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    Minimize,
    Maximize,
    Close,
}

/// One window button: its tile lit on hover, red for Close, and its glyph.
#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
fn draw_control(ui: &mut egui::Ui, control: Control, hit: egui::Rect, tile: egui::Rect, frame: &WindowFrame) -> egui::Response {
    let id = ui.scope_id().with(("window_control", control as u8));
    let response = ui.interact(hit, id, egui::Sense::click());
    // On Windows the system hit-tests the maximise button (for Snap Layouts),
    // so its hover and press come from there rather than from egui.
    let system = control == Control::Maximize;
    let hovered = response.hovered() || (system && frame.maximize_hovered);
    let pressed = response.is_pointer_button_down_on() || (system && frame.maximize_pressed);
    let visuals = ui.visuals();
    let fill = match (control, pressed, hovered) {
        (Control::Close, _, true) | (Control::Close, true, _) => crate::ui::widgets::menu::danger_fill(visuals),
        (_, true, _) => visuals.widgets.active.weak_bg_fill,
        (_, false, true) => visuals.widgets.hovered.weak_bg_fill,
        _ => egui::Color32::TRANSPARENT,
    };
    let painter = ui.painter();
    painter.rect_filled(tile, GROUP_CORNER_RADIUS, fill);
    let color = if control == Control::Close && (hovered || pressed) {
        egui::Color32::WHITE
    } else if frame.focused {
        visuals.text_color()
    } else {
        visuals.weak_text_color()
    };
    paint_glyph(painter, control, frame.maximized, tile.center(), color);
    response
}

/// The button's glyph, in one-point lines on pixel centres so it stays sharp.
#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
fn paint_glyph(painter: &egui::Painter, control: Control, maximized: bool, centre: egui::Pos2, color: egui::Color32) {
    let pixel = 1.0 / painter.pixels_per_point();
    let stroke = egui::Stroke::new(pixel.max(1.0), color);
    let snap = |x: f32, y: f32| egui::pos2(painter.round_to_pixel_center(x), painter.round_to_pixel_center(y));
    let half = GLYPH_SIZE / 2.0;
    let (left, top, right, bottom) = (centre.x - half, centre.y - half, centre.x + half, centre.y + half);
    match control {
        Control::Minimize => {
            painter.line_segment([snap(left, centre.y), snap(right, centre.y)], stroke);
        }
        Control::Maximize if maximized => {
            // Restore: the window in front, and the edge of the one behind it.
            let step = GLYPH_SIZE / 5.0;
            let front = egui::Rect::from_min_max(snap(left, top + step), snap(right - step, bottom));
            painter.rect_stroke(front, 0.0, stroke, egui::StrokeKind::Middle);
            painter.line(
                vec![
                    snap(left + step, front.top()),
                    snap(left + step, top),
                    snap(right, top),
                    snap(right, bottom - step),
                    snap(front.right(), bottom - step),
                ],
                stroke,
            );
        }
        Control::Maximize => {
            painter.rect_stroke(egui::Rect::from_min_max(snap(left, top), snap(right, bottom)), 0.0, stroke, egui::StrokeKind::Middle);
        }
        Control::Close => {
            painter.line_segment([egui::pos2(left, top), egui::pos2(right, bottom)], stroke);
            painter.line_segment([egui::pos2(left, bottom), egui::pos2(right, top)], stroke);
        }
    }
}

/// Paint the open project's name centred on the window, within `span`.
///
/// Centred on the window rather than on the space left for it, as a system
/// title is, unless the tabs or the buttons are in the way: then it slides
/// along out of their way, is cut short with an ellipsis, and at last is left
/// out.
///
/// Always in the weak text colour, focused or not: it names the window, and
/// is not something to read past the menus and tabs for.
pub(crate) fn draw_title(ui: &egui::Ui, bar: egui::Rect, span: egui::Rangef, centre_y: f32, project: &UiProjectView) {
    let span = egui::Rangef::new(span.min + TITLE_MARGIN, span.max - TITLE_MARGIN);
    if span.span() < TITLE_MIN_WIDTH {
        return;
    }
    let text = project.titled_project().map_or_else(
        || crate::APP_NAME.to_owned(),
        |entry| if entry.dirty { format!("{} *", entry.name) } else { entry.name.clone() },
    );
    let color = ui.visuals().weak_text_color();
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(span.span());
    let galley = ui.painter().layout_job(job);
    let width = galley.size().x;
    let left = (bar.center().x - width / 2.0).clamp(span.min, span.max - width);
    let pos = egui::pos2(left, centre_y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, color);
}

/// Stroke the window's border where nothing else frames it: the islands'
/// hairline, along the window's edge and round its cut corners.
///
/// On the topmost layer, so a menu or tooltip reaching the edge still sits
/// inside the frame.
pub(crate) fn draw_window_border(ctx: &egui::Context, frame: &WindowFrame) {
    if !frame.border {
        return;
    }
    let stroke = crate::ui::chrome::region_stroke(&ctx.global_style().visuals);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Debug, egui::Id::unique("window_border")));
    painter.rect_stroke(ctx.content_rect(), frame.corner_radius, stroke, egui::StrokeKind::Inside);
}

/// Let the window's edges and corners resize it, while it is neither
/// maximised nor fullscreen.
///
/// Laid over everything else, so a panel reaching the edge still gives up its
/// outermost few points to the frame, as it would to a system border.
pub(crate) fn draw_resize_edges(ctx: &egui::Context, frame: &WindowFrame, commands: &mut Vec<UiCommand>) {
    #[cfg(any(target_os = "macos", target_arch = "wasm32"))]
    let _ = (ctx, frame, commands);
    #[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
    {
        use egui::CursorIcon;
        use winit::window::ResizeDirection;

        if frame.maximized || frame.fullscreen {
            return;
        }
        let window = ctx.content_rect();
        let (x, y) = (window.x_range(), window.y_range());
        let near = |from: f32, size: f32| egui::Rangef::new(from, from + size);
        let far = |to: f32, size: f32| egui::Rangef::new(to - size, to);
        let inner = |range: egui::Rangef| egui::Rangef::new(range.min + RESIZE_CORNER, range.max - RESIZE_CORNER);
        let zones = [
            (inner(x), near(y.min, RESIZE_EDGE), ResizeDirection::North, CursorIcon::ResizeNorth),
            (inner(x), far(y.max, RESIZE_EDGE), ResizeDirection::South, CursorIcon::ResizeSouth),
            (near(x.min, RESIZE_EDGE), inner(y), ResizeDirection::West, CursorIcon::ResizeWest),
            (far(x.max, RESIZE_EDGE), inner(y), ResizeDirection::East, CursorIcon::ResizeEast),
            (
                near(x.min, RESIZE_CORNER),
                near(y.min, RESIZE_CORNER),
                ResizeDirection::NorthWest,
                CursorIcon::ResizeNorthWest,
            ),
            (
                far(x.max, RESIZE_CORNER),
                near(y.min, RESIZE_CORNER),
                ResizeDirection::NorthEast,
                CursorIcon::ResizeNorthEast,
            ),
            (
                near(x.min, RESIZE_CORNER),
                far(y.max, RESIZE_CORNER),
                ResizeDirection::SouthWest,
                CursorIcon::ResizeSouthWest,
            ),
            (
                far(x.max, RESIZE_CORNER),
                far(y.max, RESIZE_CORNER),
                ResizeDirection::SouthEast,
                CursorIcon::ResizeSouthEast,
            ),
        ];
        let pressed = ctx.input(|input| input.pointer.primary_pressed());
        for (index, (x, y, direction, cursor)) in zones.into_iter().enumerate() {
            let rect = egui::Rect::from_x_y_ranges(x, y);
            egui::Area::new(egui::Id::unique(("window_resize_edge", index)))
                .order(egui::Order::Foreground)
                .fixed_pos(rect.min)
                .show(ctx, |ui| {
                    let response = ui
                        .allocate_rect(egui::Rect::from_min_size(ui.max_rect().min, rect.size()), egui::Sense::drag())
                        .on_hover_cursor(cursor);
                    // On the press itself, while the button is down: the
                    // system takes the drag from there.
                    if pressed && response.is_pointer_button_down_on() {
                        commands.push(UiCommand::Window(WindowAction::Resize(direction)));
                    }
                });
        }
    }
}
