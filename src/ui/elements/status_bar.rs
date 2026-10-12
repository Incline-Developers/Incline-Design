//! Bottom status bar: app identity, selected count, cursor coords and, at the
//! far right, the frame rate, memory in use and the language picker. Task progress belongs to the
//! bottom toolbar - see `widgets::progress`.

use thousands::Separable;

use crate::{
    i18n::{LanguageChoice, tr},
    ui::{
        EditorState, UiCommand, themed_icon,
        widgets::{
            context_menu::{ContextMenuAction, dropdown_menu},
            progress,
            toolbar::GROUP_CORNER_RADIUS,
        },
    },
};

/// Extra padding around a coordinate readout's worst-case value.
const COORD_FIELD_PADDING: f32 = 10.0;
/// Shrink factor on the coordinate fields: the worst-case template is wider
/// than the values normally seen, so the fields don't need its full width.
const COORD_FIELD_SCALE: f32 = 0.9;

/// Fixed width for each coordinate readout, measured from a worst-case value so
/// the fields (and everything after them) don't shift as the cursor moves.
fn coord_field_width(ui: &egui::Ui) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap("RL: -8,888,888.88".to_owned(), font, egui::Color32::PLACEHOLDER);
    (galley.size().x + COORD_FIELD_PADDING) * COORD_FIELD_SCALE
}

/// One left-aligned readout occupying a fixed width regardless of its value.
fn coord_field(ui: &mut egui::Ui, width: f32, text: String) {
    let height = ui.text_style_height(&egui::TextStyle::Body);
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let color = ui.visuals().text_color();
    let font = egui::TextStyle::Body.resolve(ui.style());
    ui.painter().with_clip_rect(rect).text(rect.left_center(), egui::Align2::LEFT_CENTER, text, font, color);
}

/// Side of the globe on the language picker. Smaller than a toolbar icon: it
/// sits beside body text, not in a grid of tools.
const LANGUAGE_ICON_SIZE: f32 = 13.0;
/// Space between the language's name and that globe.
const LANGUAGE_ICON_GAP: f32 = 5.0;
/// Space either side of the picker's contents, so its hover fill reads as a
/// button rather than as a box drawn tight around the text.
const LANGUAGE_PADDING: f32 = 6.0;
/// Width of the menu the picker opens. Narrower than a menu-bar dropdown: it
/// holds one short endonym per row.
const LANGUAGE_MENU_WIDTH: f32 = 150.0;

/// The language picker at the right end of the bar: the running language named
/// in its own script, and a menu of the rest.
///
/// It is here rather than in the Preferences panel because someone who has
/// landed in a language they cannot read needs to be able to find it without
/// navigating one. Switching is live - see [`crate::i18n`].
fn draw_language_picker(ui: &mut egui::Ui, editor: &EditorState, commands: &mut Vec<UiCommand>) {
    let label = editor.language.endonym();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), egui::Color32::PLACEHOLDER);
    let size = egui::vec2(
        galley.size().x + LANGUAGE_ICON_GAP + LANGUAGE_ICON_SIZE + LANGUAGE_PADDING * 2.0,
        galley.size().y.max(LANGUAGE_ICON_SIZE),
    );
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect, GROUP_CORNER_RADIUS, ui.visuals().widgets.hovered.bg_fill);
        }
        let icon_rect = egui::Align2::RIGHT_CENTER.align_size_within_rect(egui::Vec2::splat(LANGUAGE_ICON_SIZE), rect.shrink2(egui::vec2(LANGUAGE_PADDING, 0.0)));
        egui::Image::new(themed_icon!(ui, "language.svg"))
            .fit_to_exact_size(icon_rect.size())
            .paint_at(ui, icon_rect);
        let color = ui.visuals().text_color();
        ui.painter().text(
            egui::pos2(icon_rect.left() - LANGUAGE_ICON_GAP, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            label,
            font,
            color,
        );
    }
    let title = tr!("status-language");
    dropdown_menu(&response, title, LANGUAGE_MENU_WIDTH, |ui| {
        for choice in LanguageChoice::ALL {
            // Ticked rather than selected: the menu is a set of switches with
            // one on, the same shape the View menu's toggles have.
            if ContextMenuAction::new(choice.endonym()).checked(choice == editor.language).show(ui).clicked() {
                commands.push(UiCommand::SetLanguage(choice));
                ui.close();
            }
        }
    });
}

/// Diameter of the memory readout's ring: a little over the body text's
/// height, so it sits in the line rather than standing out of it.
const MEMORY_RING_SIZE: f32 = 14.0;
/// Gap between the memory ring and its label.
const MEMORY_RING_GAP: f32 = 6.0;
/// Below this share of memory free the ring turns yellow, and below
/// [`MEMORY_LOW_FREE`] red.
const MEMORY_TIGHT_FREE: f32 = 0.4;
const MEMORY_LOW_FREE: f32 = 0.2;
/// The yellow and red fills, darkened in the light theme the way the green
/// one is - see `widgets::progress::FILL_LIGHT`.
const MEMORY_TIGHT_DARK: egui::Color32 = egui::Color32::from_rgb(0xE3, 0xB3, 0x41);
const MEMORY_TIGHT_LIGHT: egui::Color32 = egui::Color32::from_rgb(0xB0, 0x82, 0x10);
const MEMORY_LOW_DARK: egui::Color32 = egui::Color32::from_rgb(0xF8, 0x51, 0x49);
const MEMORY_LOW_LIGHT: egui::Color32 = egui::Color32::from_rgb(0xC9, 0x2A, 0x2A);

/// "memory", then a ring like the task readout's. The ring is filled to the
/// share in use: green while plenty is free, yellow under 40% free and red
/// under 20%. Natively the whole machine's memory; in the browser the app's
/// own heap - see [`crate::app::memory_usage::MemoryUsage`]. Drawn right to
/// left: the ring first, then the label before it.
fn draw_memory_usage(ui: &mut egui::Ui, editor: &EditorState) {
    let Some(usage) = editor.memory_usage else {
        return;
    };
    // Rendering is on demand, so an idle window would otherwise hold the
    // first reading forever.
    ui.ctx().request_repaint_after(crate::app::memory_usage::SAMPLE_PERIOD);
    let fraction = usage.fraction();
    let free = 1.0 - fraction;
    let color = load_color(ui.visuals().dark_mode, free);
    let (ring, _) = ui.allocate_exact_size(egui::Vec2::splat(MEMORY_RING_SIZE), egui::Sense::hover());
    progress::paint_fraction_ring(ui, ui.painter(), ring.center(), MEMORY_RING_SIZE, fraction, color);
    ui.add_space((MEMORY_RING_GAP - ui.spacing().item_spacing.x).max(0.0));
    ui.label(tr!("status-memory"));
}

/// Width of one core's bar in the CPU readout, and the gap between bars.
const CPU_BAR_WIDTH: f32 = 3.0;
const CPU_BAR_GAP: f32 = 1.0;
/// Height of the bars: the memory ring's, so the two readouts line up.
const CPU_BAR_HEIGHT: f32 = MEMORY_RING_SIZE;
/// Least share of its height a core's bar fills, whatever its load.
const CPU_BAR_MIN_FILL: f32 = 0.05;

/// "cpu", then one bar per core filling from the bottom with that core's
/// load: green, yellow over 60% and red over 80% - the memory ring's
/// thresholds, read as load rather than room. Drawn right to left: the bars
/// first, then the label before them.
fn draw_cpu_usage(ui: &mut egui::Ui, editor: &EditorState) {
    // Rendering is on demand, so an idle window would otherwise hold the
    // first reading forever.
    ui.ctx().request_repaint_after(crate::app::memory_usage::CPU_SAMPLE_PERIOD);
    let cores = &editor.cpu_usage;
    let width = cores.len() as f32 * (CPU_BAR_WIDTH + CPU_BAR_GAP) - CPU_BAR_GAP;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, CPU_BAR_HEIGHT), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        let painter = ui.painter();
        for (index, load) in cores.iter().enumerate() {
            let left = rect.left() + index as f32 * (CPU_BAR_WIDTH + CPU_BAR_GAP);
            let bar = egui::Rect::from_min_size(egui::pos2(left, rect.top()), egui::vec2(CPU_BAR_WIDTH, CPU_BAR_HEIGHT));
            // Never below a sliver, so an idle core still shows where it is.
            let shown = load.max(CPU_BAR_MIN_FILL);
            let filled = egui::Rect::from_min_max(egui::pos2(bar.left(), bar.bottom() - bar.height() * shown), bar.max);
            painter.rect_filled(filled, 0.0, load_color(visuals.dark_mode, 1.0 - load));
        }
    }
    ui.add_space((MEMORY_RING_GAP - ui.spacing().item_spacing.x).max(0.0));
    ui.label(tr!("status-cpu"));
}

/// Green while plenty is `free`, yellow under 40% and red under 20%, in the
/// theme's shade.
fn load_color(dark: bool, free: f32) -> egui::Color32 {
    let (dark_fill, light_fill) = if free < MEMORY_LOW_FREE {
        (MEMORY_LOW_DARK, MEMORY_LOW_LIGHT)
    } else if free < MEMORY_TIGHT_FREE {
        (MEMORY_TIGHT_DARK, MEMORY_TIGHT_LIGHT)
    } else {
        (progress::FILL_DARK, progress::FILL_LIGHT)
    };
    if dark { dark_fill } else { light_fill }
}

/// Draw the bottom status bar panel.
pub(crate) fn draw_status_bar(ui: &mut egui::Ui, editor: &EditorState, commands: &mut Vec<UiCommand>) -> egui::Rect {
    // The other bar that spans the window rather than sitting in a region, and
    // dressed the same way: the gap's colour, and no separator line.
    egui::Panel::bottom("status_bar")
        .show_separator_line(crate::ui::chrome::show_separator_line(ui))
        .frame(crate::ui::chrome::window_bar_frame(ui))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} {}", crate::APP_NAME, crate::APP_RELEASE));
                ui.separator();
                ui.label(tr!("status-selected", count = editor.selected_handles.len()));
                ui.separator();
                if editor.debug_surface_chunks {
                    match editor.debug_surface_stats {
                        Some(stats) => ui.label(tr!(
                            "status-faces",
                            drawn = stats.drawn_faces.separate_with_commas(),
                            total = stats.total_faces.separate_with_commas(),
                            drawn_chunks = stats.drawn_chunks,
                            total_chunks = stats.total_chunks
                        )),
                        None => ui.label(tr!("status-faces-chunks")),
                    };
                    ui.separator();
                }
                if editor.debug_clip_planes {
                    match editor.debug_clip_plane_distances {
                        Some((near, far)) => ui.label(tr!(
                            "status-clip",
                            near = format!("{near:.3}"),
                            far = format!("{far:.3}"),
                            delta = format!("{:.3}", far - near)
                        )),
                        None => ui.label(tr!("status-clip-near-far")),
                    };
                    ui.separator();
                }
                if editor.debug_point_cloud_chunks {
                    match editor.debug_point_stats {
                        Some(stats) => ui.label(tr!(
                            "status-points",
                            drawn = stats.drawn.separate_with_commas(),
                            target = stats.target.separate_with_commas(),
                            total = stats.total.separate_with_commas(),
                            drawn_chunks = stats.drawn_chunks,
                            total_chunks = stats.total_chunks
                        )),
                        None => ui.label(tr!("status-points-chunks")),
                    };
                    ui.separator();
                }
                let coord_width = coord_field_width(ui);
                let cursor_in_viewport = ui.input(|input| input.pointer.hover_pos().is_some()) && !ui.ctx().is_pointer_over_egui();
                match editor.cursor_world.filter(|_| cursor_in_viewport) {
                    Some(p) => {
                        for (axis, value) in crate::model::survey::axis_names().into_iter().zip([p.x, p.y, p.z]) {
                            coord_field(ui, coord_width, format!("{axis}: {}", format!("{value:.2}").separate_with_commas()));
                        }
                    }
                    None => {
                        for axis in crate::model::survey::axis_names() {
                            coord_field(ui, coord_width, format!("{axis}: --"));
                        }
                    }
                }
                // The picker is pinned to the far end of the bar, so it does not
                // move as the readouts before it come and go.
                // Right to left: the frame rate and memory readouts sit just
                // before it.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    draw_language_picker(ui, editor, commands);
                    ui.separator();
                    let fps = editor.measured_fps.map_or_else(|| "--".to_owned(), |fps| format!("{fps:.0}"));
                    ui.label(tr!("status-fps", fps = fps));
                    if editor.memory_usage.is_some() {
                        ui.separator();
                        draw_memory_usage(ui, editor);
                    }
                    if !editor.cpu_usage.is_empty() {
                        ui.separator();
                        draw_cpu_usage(ui, editor);
                    }
                });
            });
        })
        .response
        .rect
}
