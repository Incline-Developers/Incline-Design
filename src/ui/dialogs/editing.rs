//! Object editing and viewport tool dialogs.

use crate::{
    i18n::tr,
    model::{Axis, Document},
    ui::{
        state::{ActiveTool, BatterBermMode, DrapePhase, EditorState, HeightMode, OffsetMeasure, RelimitMode, TrimEnd, UiCommand, UiProjectView},
        themed_icon,
        widgets::{
            context_menu::{ContextMenu, ContextMenuAction, context_menu_popup, context_menu_separator},
            menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldBool, MenuFieldCombo, MenuFieldF64, MenuFieldRgba, MenuFieldText, MenuFieldU32},
            viewport::ViewportDockPanel,
        },
    },
};

/// Draw the two-step selection prompt for Drape to Topology at the top of the viewport.
pub(crate) fn draw_drape_selection_panel(ui: &mut egui::Ui, editor: &EditorState, commands: &mut Vec<UiCommand>, viewport_rect: egui::Rect) {
    if editor.active_tool != ActiveTool::DrapeToTopology {
        return;
    }

    let selected_count = match editor.drape_phase {
        DrapePhase::Designs => editor
            .selected_handles
            .iter()
            .filter(|handle| matches!(handle, crate::model::SceneEntityId::Object(_)))
            .count(),
        DrapePhase::Topologies => editor
            .selected_handles
            .iter()
            .filter(|handle| matches!(handle, crate::model::SceneEntityId::Triangulation(_)))
            .count(),
    };

    ViewportDockPanel::new("drape_to_topology_panel", tr!("common-drape-topology"), viewport_rect)
        .min_width(280.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("ui-selected-count", count = selected_count));
            ui.add_space(6.0);
            if ui.add(MenuButton::new(tr!("edit-confirm-selection")).primary().enabled(selected_count > 0)).clicked() {
                commands.push(UiCommand::ConfirmDrapeSelection);
            }
        });
}

/// Title for the canvas context menu, named after the kind of entity that is
/// selected (e.g. "Triangulation Properties"). Selections spanning several
/// kinds fall back to a generic label.
fn object_kind_label(object: &crate::model::Object) -> String {
    match object {
        crate::model::Object::Point { .. } => tr!("common-point"),
        crate::model::Object::Polyline { verts, .. } if verts.len() == 2 => tr!("common-line"),
        crate::model::Object::Polyline { .. } => tr!("common-polyline"),
        crate::model::Object::Circle { .. } => tr!("common-circle"),
        crate::model::Object::Text { .. } => tr!("common-text"),
    }
}

fn canvas_context_menu_title(editor: &EditorState, document: &Document) -> String {
    use crate::model::SceneEntityId;

    // (label, whether it came from a document object)
    let mut kind: Option<(String, bool)> = None;
    for &handle in &editor.selected_handles {
        let (label, is_object) = match handle {
            SceneEntityId::Object(id) => (document.get_object(id).map_or_else(|| tr!("edit-object"), object_kind_label), true),
            SceneEntityId::Triangulation(_) => (tr!("ws-menubar-triangulation"), false),
            SceneEntityId::BlockModel(_) => (tr!("ws-menubar-block-model"), false),
            SceneEntityId::DrillHole(_) => (tr!("edit-drill-hole"), false),
            SceneEntityId::PointCloud(_) => (tr!("ws-menubar-point-cloud"), false),
            SceneEntityId::Raster(_) => (tr!("ws-menubar-raster"), false),
        };
        match kind.as_ref() {
            None => kind = Some((label, is_object)),
            Some((existing, _)) if existing == &label => {}
            // Mixed object kinds (a line and a text object, say) still share a menu.
            Some((_, true)) if is_object => kind = Some((tr!("edit-object"), true)),
            Some(_) => return tr!("edit-properties"),
        }
    }

    kind.map_or_else(
        || tr!("edit-properties"),
        |(label, _)| tr!("edit-kind-properties", kind = label.to_string(), properties = (tr!("edit-properties")).to_string()),
    )
}

/// Draw the canvas right-click context menu for selected objects and triangulations.
///
/// Groups design appearance and editing controls before shared selection actions.
/// Updates `geometry_dirty` when changes are made.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_right_click_context(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    project: &UiProjectView,
    commands: &mut Vec<UiCommand>,
    geometry_dirty: &mut bool,
    document: &Document,
    px: f32,
    py: f32,
) {
    let ppp = ui.ctx().pixels_per_point();
    let pos = egui::pos2(px / ppp + 4.0, py / ppp + 4.0);
    let title = canvas_context_menu_title(editor, document);
    ContextMenu::new("canvas_properties", title).position(pos).width(220.0).show(ui.ctx(), |ui| {
        crate::ui::elements::properties::draw_selection_appearance(ui, editor, project, document, commands, geometry_dirty);
        let selected_drill_hole = editor
            .selected_handles
            .iter()
            .find_map(|&h| match h {
                crate::model::SceneEntityId::DrillHole(id) => Some(id),
                _ => None,
            })
            .or_else(|| editor.canvas_context_menu_hole.map(|hole| hole.dataset))
            .or_else(|| editor.selected_drill_holes.iter().map(|hole| hole.dataset).min_by_key(|id| id.0));

        // --- Drill holes ---
        // The inspector row acts on the hole the pick named; colouring acts on
        // its whole dataset, which is what the selection names.
        let context_hole = editor.canvas_context_menu_hole;
        if context_hole.is_some() || selected_drill_hole.is_some() {
            if let Some(hole) = context_hole
                && ContextMenuAction::new(tr!("common-borehole-inspector")).show(ui).clicked()
            {
                commands.push(UiCommand::InspectDrillHole(hole));
                commands.push(UiCommand::CloseCanvasContextMenu);
            }

            if let Some(drill_hole_id) = selected_drill_hole
                && ContextMenuAction::new(tr!("common-appearance")).show(ui).clicked()
            {
                commands.push(UiCommand::OpenDrillHoleColorDialog(drill_hole_id));
                commands.push(UiCommand::CloseCanvasContextMenu);
            }

            if let Some(drill_hole_id) = selected_drill_hole
                && ContextMenuAction::new(tr!("common-link-geophysics")).show(ui).clicked()
            {
                commands.push(UiCommand::LinkGeophysics(drill_hole_id));
                commands.push(UiCommand::CloseCanvasContextMenu);
            }

            context_menu_separator(ui);
        }

        if !editor.selected_handles.is_empty() {
            if ContextMenuAction::new(tr!("common-hide-selection")).show(ui).clicked() {
                commands.push(UiCommand::HideSelection);
                commands.push(UiCommand::CloseCanvasContextMenu);
            }
            if ContextMenuAction::new(tr!("common-lock-selection")).show(ui).clicked() {
                *geometry_dirty |= editor.apply_action(crate::ui::state::EditorAction::FreezeSelection);
                commands.push(UiCommand::CloseCanvasContextMenu);
            }
            context_menu_separator(ui);
        }

        if ContextMenuAction::new(tr!("survey-close")).show(ui).clicked() {
            commands.push(UiCommand::CloseCanvasContextMenu);
        }
    });
}

pub(crate) fn draw_move_to_layer_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    let Some(active_project) = project.projects.iter().find(|entry| entry.is_active) else {
        editor.move_to_layer_dialog = None;
        return;
    };
    let Some(dialog) = editor.move_to_layer_dialog.as_mut() else {
        return;
    };

    if dialog.target_layer.is_none_or(|id| !active_project.layers.iter().any(|layer| layer.id == id)) {
        dialog.target_layer = active_project.layers.first().map(|layer| layer.id);
    }

    let selected_label = dialog
        .target_layer
        .and_then(|id| active_project.layers.iter().find(|layer| layer.id == id))
        .map(|layer| layer.name.clone())
        .unwrap_or_else(|| tr!("edit-choose-layer"));
    let layer_options = active_project.layers.iter().map(|layer| (Some(layer.id), layer.name.clone().into()));
    let can_apply = dialog.target_layer.is_some() && !dialog.object_ids.is_empty();
    let object_count = dialog.object_ids.len();
    let mut close = false;
    let mut apply = false;
    let mut open = true;

    DragableMenu::new("move_to_layer_dialog", tr!("edit-move-layer"))
        .open(&mut open)
        .min_width(260.0)
        .max_width(280.0)
        .show(ui.ctx(), |ui| {
            // Added in reverse: a field row lays its control out from the right.
            MenuField::new(tr!("edit-action")).show(ui, |ui, _, _| {
                ui.horizontal(|ui| {
                    if ui.add(MenuButton::new(tr!("edit-copy")).selected(dialog.copy).min_width(64.0)).clicked() {
                        dialog.copy = true;
                    }
                    if ui.add(MenuButton::new(tr!("edit-move")).selected(!dialog.copy).min_width(64.0)).clicked() {
                        dialog.copy = false;
                    }
                })
                .response
            });
            MenuFieldCombo::new("move_to_layer_target", tr!("common-layer"), &mut dialog.target_layer, selected_label, layer_options)
                .width(180.0)
                .show(ui);
            menu::menu_actions(ui, |ui| {
                let action_label = if dialog.copy { tr!("edit-copy") } else { tr!("edit-move") };
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(action_label).primary().enabled(can_apply)).clicked() || (confirm && can_apply) {
                    apply = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
            ui.label(tr!("ui-selected-objects", count = object_count));
        });

    if apply {
        if let Some(target_layer) = dialog.target_layer {
            commands.push(UiCommand::MoveObjectsToLayer {
                object_ids: dialog.object_ids.clone(),
                target_layer,
                copy: dialog.copy,
            });
        }
        editor.move_to_layer_dialog = None;
    } else if close || !open {
        editor.move_to_layer_dialog = None;
    }
}

pub(crate) fn draw_move_to_axis_dialog(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let Some(dialog) = editor.move_to_axis_dialog.as_mut() else {
        return;
    };

    let axis = dialog.axis;
    let axis_label = axis.label();
    let object_count = dialog.object_ids.len();
    let can_apply = dialog.value.is_finite() && object_count > 0;
    let mut close = false;
    let mut apply = false;
    let mut open = true;

    DragableMenu::new("move_to_axis_dialog", tr!("edit-set-axis", axis = axis_label.to_string()))
        .open(&mut open)
        .min_width(260.0)
        .max_width(280.0)
        .show(ui.ctx(), |ui| {
            MenuFieldF64::new(tr!("edit-axis-value", axis = axis_label.to_string()), &mut dialog.value, f64::MIN..=f64::MAX)
                .width(120.0)
                .show(ui);
            if !dialog.value.is_finite() {
                ui.colored_label(egui::Color32::from_rgb(200, 70, 70), tr!("ui-invalid-axis-value", axis = axis_label));
            }
            ui.add_space(4.0);
            let submitted = menu::dialog_confirm_pressed(ui.ctx());
            let cancelled = menu::dialog_cancel_pressed(ui.ctx());
            menu::menu_actions(ui, |ui| {
                if (submitted || ui.add(MenuButton::new(tr!("edit-apply")).primary().enabled(can_apply)).clicked()) && can_apply {
                    apply = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || cancelled {
                    close = true;
                }
            });
            ui.label(tr!("ui-selected-objects", count = object_count));
        });

    if apply {
        let value = dialog.value;
        commands.push(UiCommand::BatchSetAxisValue(dialog.object_ids.clone(), axis, value));
        if axis == Axis::Z {
            editor.z_input = value;
            editor.z_level = value;
        }
        editor.move_to_axis_dialog = None;
    } else if close || !open {
        editor.move_to_axis_dialog = None;
    }
}

pub(crate) fn draw_insert_point_at_elevation_dialog(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let Some(dialog) = editor.insert_point_at_elevation_dialog.as_mut() else {
        return;
    };

    let object_count = dialog.object_ids.len();
    let can_apply = dialog.elevation.is_finite() && object_count > 0;
    let mut close = false;
    let mut apply = false;
    let mut open = true;

    DragableMenu::new("insert_point_at_elevation_dialog", tr!("edit-insert-point-elevation"))
        .open(&mut open)
        .min_width(280.0)
        .max_width(300.0)
        .show(ui.ctx(), |ui| {
            MenuFieldF64::new(tr!("common-elevation"), &mut dialog.elevation, dialog.min_elevation..=dialog.max_elevation)
                .width(120.0)
                .show(ui);
            if !dialog.elevation.is_finite() {
                ui.colored_label(egui::Color32::from_rgb(200, 70, 70), tr!("edit-enter-valid-elevation"));
            }
            if dialog.min_elevation > f64::MIN {
                ui.label(tr!(
                    "ui-selection-spans",
                    min = format!("{:.2}", dialog.min_elevation),
                    max = format!("{:.2}", dialog.max_elevation)
                ));
            }
            ui.label(tr!("edit-segments-lying-elevation-ignored"));
            ui.add_space(4.0);
            let submitted = menu::dialog_confirm_pressed(ui.ctx());
            let cancelled = menu::dialog_cancel_pressed(ui.ctx());
            menu::menu_actions(ui, |ui| {
                if (submitted || ui.add(MenuButton::new(tr!("edit-apply")).primary().enabled(can_apply)).clicked()) && can_apply {
                    apply = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || cancelled {
                    close = true;
                }
            });
            ui.label(tr!("ui-selected-polylines", count = object_count));
        });

    if apply {
        commands.push(UiCommand::InsertPointsAtElevation {
            object_ids: dialog.object_ids.clone(),
            elevation: dialog.elevation,
        });
        editor.z_input = dialog.elevation;
        editor.insert_point_at_elevation_dialog = None;
    } else if close || !open {
        editor.insert_point_at_elevation_dialog = None;
    }
}

/// Draw the welcome splash the application starts on.
///
/// A project is already open behind it - startup lands on an empty, never-saved
/// one - so the splash is an offer, not a gate: Escape, a click on the backdrop,
/// or `New project` all simply dismiss it and leave that project in place.
pub(crate) fn draw_select_project_dialog(ui: &mut egui::Ui, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    const PANEL_SIZE: f32 = 500.0;
    const COLUMN_WIDTH: f32 = 190.0;
    const ROW_HEIGHT: f32 = 22.0;
    const RECENT_HEIGHT: f32 = 100.0;
    /// Grow the splash by exactly as much as the Recent box has grown from the
    /// original two-row grid, keeping the footer and surrounding spacing put,
    /// and by however much taller the artwork is than the 120pt banner it
    /// replaced.
    const PANEL_HEIGHT: f32 = PANEL_SIZE * 0.7 + (RECENT_HEIGHT - 48.0) + (SPLASH_ARTWORK_HEIGHT + SPLASH_ARTWORK_GAP - 120.0);

    // The splash is the only place a remembered project can be picked up now
    // that the explorer shows the open one alone. The active project is never
    // among these: the splash is only up when nothing but the startup project
    // is open.
    let recent: Vec<&crate::ui::state::UiTrackedProjectEntry> = project.recent_projects().collect();

    // The splash floats over a live window, so something has to catch the
    // clicks that dismiss it, or they would fall through to the viewport and
    // orbit the camera instead. It paints nothing: the window behind the
    // splash reads as it always does.
    let screen = ui.ctx().viewport_rect();
    let backdrop = egui::Area::new(egui::Id::new("select_project_backdrop"))
        .fixed_pos(screen.min)
        .order(egui::Order::Middle)
        .show(ui.ctx(), |ui| ui.allocate_exact_size(screen.size(), egui::Sense::click()).1);
    if backdrop.inner.clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
        commands.push(UiCommand::CloseStartupDialog);
    }

    egui::Area::new(egui::Id::new("select_project_dialog"))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(ui.visuals().window_fill())
                .stroke(ui.visuals().window_stroke())
                .corner_radius(egui::CornerRadius::same(crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS))
                .inner_margin(egui::Margin::ZERO)
                .show(ui, |ui| {
                    ui.set_width(PANEL_SIZE);
                    ui.set_height(PANEL_HEIGHT);
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 16.0);

                    // Claimed before the content above it, so that content -
                    // and the Recent list in particular - lays out against
                    // what is genuinely left rather than pushing this out of
                    // the frame. It still draws along the bottom edge.
                    egui::Panel::bottom("meta_splash").show_separator_line(false).show(ui, |ui| {
                        // Housekeeping, not content: set back from the actions
                        // the way the site sets back its platform line.
                        let weak = splash_caption_color(ui);
                        ui.horizontal_centered(|ui| {
                            ui.label(egui::RichText::new(format!("{} {}", crate::APP_NAME, crate::APP_RELEASE)).size(12.0).color(weak));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(tr!("edit-mit-license")).size(12.0).color(weak))
                            });
                        });
                    });

                    draw_splash_artwork(ui, PANEL_SIZE);

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                        ui.add_space(30.0);
                        select_project_action_column(ui, tr!("common-project"), COLUMN_WIDTH, |ui| {
                            // Startup is already sitting on an empty project,
                            // so this only has to get the splash out of the way.
                            if select_project_action_row(
                                ui,
                                egui::Image::new(themed_icon!(ui, "create_project.svg")),
                                tr!("edit-new-project"),
                                COLUMN_WIDTH,
                                ROW_HEIGHT,
                            )
                            .clicked()
                            {
                                commands.push(UiCommand::CloseStartupDialog);
                            }
                            if select_project_action_row(
                                ui,
                                egui::Image::new(themed_icon!(ui, "open_project.svg")),
                                tr!("edit-load-project"),
                                COLUMN_WIDTH,
                                ROW_HEIGHT,
                            )
                            .clicked()
                            {
                                commands.push(UiCommand::OpenProject);
                            }
                        });

                        ui.add_space(PANEL_SIZE - (COLUMN_WIDTH * 2.0) - 60.0);

                        select_project_action_column(ui, tr!("edit-application"), COLUMN_WIDTH, |ui| {
                            if select_project_action_row(ui, egui::Image::new(themed_icon!(ui, "open_website.svg")), tr!("about-website"), COLUMN_WIDTH, ROW_HEIGHT).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab("https://inclinedesign.net"));
                            }
                            // Escape dismisses the splash rather than firing
                            // this row: leaving is a deliberate click only.
                            #[cfg(not(target_arch = "wasm32"))]
                            if select_project_action_row(ui, egui::Image::new(themed_icon!(ui, "close_project.svg")), tr!("menu-file-exit"), COLUMN_WIDTH, ROW_HEIGHT).clicked() {
                                commands.push(UiCommand::RequestExit);
                            }
                        });
                    });

                    // The splash is a fixed shape, so the Recent list takes
                    // the room left below the two columns rather than making
                    // the frame taller. 16pt of that is the gap above its
                    // heading. The box presents its entries as one full-width
                    // scrolling list and keeps two rows visible at once.
                    let list_width = PANEL_SIZE - 60.0;
                    // Drawn even with nothing remembered: the splash keeps one
                    // shape on every launch, and the empty box says where
                    // projects will show up rather than leaving a hole.
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                        ui.add_space(30.0);
                        select_project_action_column(ui, tr!("edit-recent"), list_width, |ui| {
                            draw_recent_projects(ui, &recent, list_width, RECENT_HEIGHT, ROW_HEIGHT, commands);
                        });
                    });
                });

            #[cfg(target_arch = "wasm32")]
            {
                let warning_color = if ui.visuals().dark_mode {
                    egui::Color32::from_rgb(255, 190, 80)
                } else {
                    egui::Color32::from_rgb(145, 85, 0)
                };

                ui.add_space(8.0);
                egui::Frame::new()
                    .fill(ui.visuals().window_fill())
                    .stroke(egui::Stroke::new(1.0, warning_color))
                    .corner_radius(3.0)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_width(PANEL_SIZE - 24.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(tr!("edit-app-web-not-recommended-production", app = crate::APP_NAME.to_string())));
                            ui.hyperlink_to(tr!("edit-download-free-native-version-our"), "https://inclinedesign.net");
                        });
                    });
            }
        });
}

/// The banner's height at the splash's width, from its 1000x280 pixels.
const SPLASH_ARTWORK_HEIGHT: f32 = 500.0 * 280.0 / 1000.0;
/// Extra room below the banner, on top of the usual item gap, so the first
/// row does not crowd it.
const SPLASH_ARTWORK_GAP: f32 = 6.0;

/// Draw the banner across the top of the splash, edge to edge, the way
/// Blender opens on a piece of release artwork: the benches of an open pit,
/// rendered as a triangulation in Incline itself (the website's
/// triangulations feature image, cropped to a strip).
///
/// A PNG rather than an SVG, which is all egui's installed loaders read, so
/// it is decoded once here and its texture kept in the context.
fn draw_splash_artwork(ui: &mut egui::Ui, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, SPLASH_ARTWORK_HEIGHT + SPLASH_ARTWORK_GAP), egui::Sense::hover());
    let Some(texture) = splash_artwork_texture(ui.ctx()) else {
        return;
    };
    // Held a point inside the card's top and sides so its border still runs
    // round the banner, with the top corners following the card's rounding.
    let art = egui::Rect::from_min_max(rect.min + egui::vec2(1.0, 1.0), egui::pos2(rect.right() - 1.0, rect.top() + SPLASH_ARTWORK_HEIGHT));
    let radius = crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS;
    egui::Image::new((texture.id(), art.size()))
        .corner_radius(egui::CornerRadius {
            nw: radius,
            ne: radius,
            sw: 0,
            se: 0,
        })
        .paint_at(ui, art);
}

/// The splash artwork's texture, decoded on first use. `None` only if the
/// embedded file fails to decode, in which case the splash goes without.
fn splash_artwork_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    const ARTWORK: &[u8] = include_bytes!("../../../res/ui/splash_artwork.png");

    let id = egui::Id::new("splash_artwork_texture");
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let decode = || -> Result<egui::ColorImage, png::DecodingError> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(ARTWORK));
        // The file is palette-coloured to keep it small; expand it to RGBA.
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
        let mut reader = decoder.read_info()?;
        let mut pixels = vec![0; reader.output_buffer_size().unwrap_or_default()];
        let info = reader.next_frame(&mut pixels)?;
        pixels.truncate(info.buffer_size());
        Ok(egui::ColorImage::from_rgba_unmultiplied([info.width as usize, info.height as usize], &pixels))
    };
    let image = match decode() {
        Ok(image) => image,
        Err(error) => {
            log::warn!("splash artwork failed to decode: {error}");
            return None;
        }
    };
    let texture = ctx.load_texture("splash_artwork", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    Some(texture)
}

/// The splash's section headings and footer: set back from the actions, but a
/// step darker than egui's weak text so they still read at a glance.
fn splash_caption_color(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().weak_text_color().lerp_to_gamma(ui.visuals().text_color(), 0.25)
}

fn draw_recent_projects(ui: &mut egui::Ui, recent: &[&crate::ui::state::UiTrackedProjectEntry], width: f32, height: f32, row_height: f32, commands: &mut Vec<UiCommand>) {
    const INSET: i8 = 3;

    let (surface, stripe) = crate::ui::widgets::tree_row_colors(ui);
    let frame = egui::Frame::new()
        .fill(surface)
        .stroke(ui.visuals().window_stroke())
        .corner_radius(egui::CornerRadius::same(crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS))
        .inner_margin(egui::Margin::same(INSET));

    frame.show(ui, |ui| {
        let inset = f32::from(INSET);
        ui.set_width(width - inset * 2.0);
        // The banding belongs to the box, not to the entries: it tiles the
        // whole list the way the explorer's does, so a couple of entries read
        // as a striped list rather than as one stray dark bar. Reserved here,
        // outside the scroll area, so the bands sit behind the rows and can be
        // measured against the box itself once its rect and scroll offset are
        // known - inside, the only rect wide enough to band is the dialog's
        // clip rect, which is the whole window.
        let stripes_slot = ui.painter().add(egui::Shape::Noop);
        let scroll = egui::ScrollArea::vertical()
            .id_salt("recent_projects_scroll")
            .max_height(height - inset * 2.0)
            .min_scrolled_height(0.0)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for entry in recent.iter() {
                    let label = if entry.dirty { format!("{} *", entry.name) } else { entry.name.clone() };
                    let row = select_project_action_row(
                        ui,
                        egui::Image::new(themed_icon!(ui, "recent_project.svg")),
                        label.clone(),
                        ui.available_width(),
                        row_height,
                    );
                    if row.clicked() {
                        #[cfg(not(target_arch = "wasm32"))]
                        commands.push(UiCommand::ActivateTrackedProject(entry.path.clone()));
                        #[cfg(target_arch = "wasm32")]
                        commands.push(UiCommand::ActivateTrackedProject(entry.id));
                    }
                    context_menu_popup(&row, entry.name.as_str(), |ui| {
                        #[cfg(not(target_arch = "wasm32"))]
                        if ContextMenuAction::new(crate::ui::elements::main_menu::show_in_file_manager_label()).show(ui).clicked() {
                            commands.push(UiCommand::ShowTrackedProjectInFileManager(entry.path.clone()));
                            ui.close();
                        }
                        if ContextMenuAction::new(tr!("edit-remove-from-list")).show(ui).clicked() {
                            #[cfg(not(target_arch = "wasm32"))]
                            commands.push(UiCommand::RemoveTrackedProject(entry.path.clone()));
                            #[cfg(target_arch = "wasm32")]
                            commands.push(UiCommand::RemoveTrackedProject(entry.id));
                            ui.close();
                        }
                    });
                }
            });
        // Anchored to the content's top edge - the box top less however far the
        // list is scrolled - so the bands travel with the rows rather than
        // staying put under them.
        let box_rect = scroll.inner_rect;
        let bands = crate::ui::widgets::explorer::stripe_bands(box_rect.x_range(), box_rect.top() - scroll.state.offset.y, box_rect.bottom(), row_height, stripe);
        // Clipped to the box: when the list is scrolled the first band starts
        // above the box's top edge.
        ui.painter().with_clip_rect(box_rect).set(stripes_slot, bands);
    });
}

fn select_project_action_column(ui: &mut egui::Ui, heading: impl Into<String>, width: f32, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.label(egui::RichText::new(heading.into()).size(12.0).color(splash_caption_color(ui)));
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
        add_contents(ui);
    });
}

fn select_project_action_row(ui: &mut egui::Ui, icon: egui::Image<'static>, label: impl Into<String>, width: f32, height: f32) -> egui::Response {
    select_project_action_row_with_fill(ui, icon, label, width, height, egui::Color32::TRANSPARENT)
}

fn select_project_action_row_with_fill(ui: &mut egui::Ui, icon: egui::Image<'static>, label: impl Into<String>, width: f32, height: f32, fill: egui::Color32) -> egui::Response {
    /// How far a hovered row's content steps right, in points.
    const HOVER_NUDGE: f32 = 3.0;

    let label = label.into();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    // The nudge is the whole hover cue - no fill behind the row. Eased in and
    // out, so sweeping the pointer down the list reads as one motion rather
    // than rows jumping.
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.12);
    if fill != egui::Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, egui::CornerRadius::ZERO, fill);
    }
    let nudge = HOVER_NUDGE * hover;

    let icon_size = egui::vec2(22.0, 22.0);
    let icon_rect = egui::Rect::from_min_size(egui::pos2(rect.left() + 2.0 + nudge, rect.center().y - icon_size.y / 2.0), icon_size);
    icon.fit_to_exact_size(icon_size).paint_at(ui, icon_rect);

    // Rows are laid out to a fixed width, so long labels truncate rather than
    // running beyond their action area.
    let text_left = rect.left() + 30.0 + nudge;
    let text_color = ui.visuals().text_color();
    let mut job = egui::text::LayoutJob::single_section(label.to_owned(), egui::TextFormat::simple(egui::FontId::proportional(13.0), text_color));
    job.wrap = egui::text::TextWrapping::truncate_at_width((rect.right() - 4.0 - text_left).max(0.0));
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(egui::pos2(text_left, rect.center().y - galley.size().y / 2.0), galley, text_color);

    response
}

/// Draw the browser-only prompt used to name a new project before it is created.
#[cfg(target_arch = "wasm32")]
pub(crate) fn draw_create_project_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState, viewport_rect: egui::Rect) {
    ViewportDockPanel::new("create_project_panel", tr!("edit-create-new-project"), viewport_rect)
        .min_width(240.0)
        .show(ui.ctx(), |ui| {
            let can_create = !editor.new_project_name.trim().is_empty();
            MenuFieldText::new(tr!("edit-project-name"), &mut editor.new_project_name)
                .hint_text(tr!("dialog-rename-field-hint"))
                .show(ui);
            let submitted = menu::dialog_confirm_pressed(ui.ctx());
            let cancelled = menu::dialog_cancel_pressed(ui.ctx());
            menu::menu_actions(ui, |ui| {
                if (submitted || ui.add(MenuButton::new(tr!("edit-create-project")).primary().enabled(can_create)).clicked()) && can_create {
                    commands.push(UiCommand::CreateBrowserProject {
                        name: editor.new_project_name.trim().to_owned(),
                    });
                    editor.new_project_dialog_open = false;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || cancelled {
                    editor.new_project_dialog_open = false;
                }
            });
        });
}

/// Draw the "Create a new layer" dialog (opened when NewLayer tool is active).
pub(crate) fn draw_create_layer_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState, project: &UiProjectView, viewport_rect: egui::Rect) {
    if !project.has_active_project {
        editor.new_layer_dialog_open = false;
        return;
    }

    ViewportDockPanel::new("create_layer_panel", tr!("edit-create-new-layer"), viewport_rect)
        .min_width(220.0)
        .show(ui.ctx(), |ui| {
            let can_save = !editor.new_layer_name.trim().is_empty();
            MenuFieldText::new(tr!("edit-layer-name"), &mut editor.new_layer_name)
                .hint_text(tr!("dialog-rename-field-hint"))
                .show(ui);
            let submitted = menu::dialog_confirm_pressed(ui.ctx());
            let cancelled = menu::dialog_cancel_pressed(ui.ctx());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let create_clicked = ui.add(MenuButton::new(tr!("common-create-layer")).primary().enabled(can_save)).clicked();
                if (submitted || create_clicked) && can_save {
                    commands.push(UiCommand::CreateLayer {
                        name: editor.new_layer_name.trim().to_string(),
                    });
                    editor.new_layer_dialog_open = false;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || cancelled {
                    editor.new_layer_dialog_open = false;
                }
            });
        });
}

/// Draw the rename floating dialog for whichever explorer item is being renamed.
pub(crate) fn draw_rename_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState) {
    let Some((target, _)) = editor.renaming_item else {
        return;
    };
    // Work on a local copy of the name buffer to avoid borrow conflicts inside the closure.
    let mut name_buf = editor.renaming_item.as_ref().map(|(_, n)| n.clone()).unwrap_or_default();
    let mut close = false;
    let mut rename_to: Option<String> = None;
    let mut open = true;
    DragableMenu::new("rename_dialog", tr!("dialog-rename-title", kind = target.kind_label()))
        .open(&mut open)
        .max_width(380.0)
        .show(ui.ctx(), |ui| {
            // `menu_field_row` gives the entry its requested width and leaves the
            // rest of the row to the label, so the extra width here is the label's.
            MenuFieldText::new(tr!("dialog-rename-field"), &mut name_buf)
                .width(260.0)
                .hint_text(tr!("dialog-rename-field-hint"))
                .show(ui);
            menu::menu_actions(ui, |ui| {
                let can_rename = !name_buf.trim().is_empty();
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("dialog-rename-submit")).primary().enabled(can_rename)).clicked()) && can_rename {
                    rename_to = Some(name_buf.trim().to_string());
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    // Write the edited buffer back.
    if let Some((_, ref mut buf)) = editor.renaming_item {
        *buf = name_buf;
    }
    if let Some(new_name) = rename_to {
        commands.push(UiCommand::RenameItem { target, new_name });
    } else if close || !open {
        editor.renaming_item = None;
    }
}

/// Draw the text-editing properties popup (height, rotation, colour, content).
pub(crate) fn draw_text_edit_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState, geometry_dirty: &mut bool, viewport_rect: egui::Rect) {
    let Some(object_id) = editor.editing_labels_id else {
        return;
    };
    ViewportDockPanel::new("text_edit_panel", tr!("common-edit-text"), viewport_rect)
        .min_width(260.0)
        .show(ui.ctx(), |ui| {
            let response = MenuFieldText::new(tr!("common-text"), &mut editor.pending_text)
                .width(240.0)
                .hint_text(tr!("common-text"))
                .show(ui);
            if response.changed() {
                *geometry_dirty = true;
            }
            if editor.text_edit_focus_requested {
                response.request_focus();
                editor.text_edit_focus_requested = false;
            }
            *geometry_dirty |= MenuFieldF64::new(tr!("edit-height"), &mut editor.pending_text_height, 0.001..=1.0e9)
                .speed(0.25)
                .max_decimals(3)
                .suffix(tr!("common-m"))
                .show(ui)
                .changed();
            *geometry_dirty |= MenuFieldF64::new(tr!("drill-pattern-rotation"), &mut editor.pending_text_rotation_degrees, f64::MIN..=f64::MAX)
                .speed(1.0)
                .suffix(tr!("common-degree-suffix"))
                .show(ui)
                .changed();
            *geometry_dirty |= MenuFieldRgba::new(tr!("common-colour"), &mut editor.pending_text_color)
                .help_text(tr!("edit-text-colour-opacity"))
                .show(ui)
                .changed();

            let apply_from_enter = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            let cancel_from_escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let apply = ui.add(MenuButton::new(tr!("edit-apply")).primary()).clicked() || apply_from_enter;
                let cancel = ui
                    .add(MenuButton::new(if editor.text_edit_created { tr!("confirmations-discard") } else { tr!("common-cancel") }))
                    .clicked()
                    || cancel_from_escape;
                if apply {
                    commands.push(UiCommand::CommitTextEdit(
                        object_id,
                        editor.pending_text.clone(),
                        editor.pending_text_height,
                        editor.pending_text_rotation_degrees,
                        editor.pending_text_color,
                    ));
                    editor.text_editing_enabled = false;
                } else if cancel {
                    commands.push(UiCommand::CancelTextEdit);
                    editor.text_editing_enabled = false;
                }
            });
        });
    editor.text_edit_position_frames = editor.text_edit_position_frames.saturating_sub(1);
}

/// Draw the polyline finish dialog (Close / Leave open / Cancel) near the cursor.
pub(crate) fn draw_finish_polyline_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState, viewport_rect: egui::Rect) {
    ViewportDockPanel::new("finish_poly_dialog", tr!("edit-finish-polyline"), viewport_rect).show(ui, |ui| {
        MenuField::new(tr!("common-shape")).show(ui, |ui, _, _| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Drain the key from egui's queue every frame so it never leaks
                // elsewhere, but honour it only once the Enter that opened this
                // dialog has been released - a key held down to finish the line
                // would otherwise confirm "Open" the instant the dialog appears.
                let confirm_key = menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!("common-open")).primary()).clicked() || (confirm_key && editor.poly_finish_dialog_confirm_armed) {
                    commands.push(UiCommand::CommitStrokeOpen);
                    editor.poly_finish_dialog = false;
                }
                if ui.add(MenuButton::new(tr!("common-closed"))).clicked() {
                    commands.push(UiCommand::FinishPolyClose);
                    editor.poly_finish_dialog = false;
                }
            });
        });
    });
}

// I dont like this - we need to clean this up later - too many options and sub menus
/// Draw the Offset Element dialog.
pub(crate) fn draw_offset_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState, viewport_rect: egui::Rect) {
    if editor.offset_target_ids.is_empty() {
        return;
    }

    ViewportDockPanel::new("offset_element_panel", tr!("edit-offset-element"), viewport_rect)
        .min_width(350.0)
        .show(ui.ctx(), |ui| {
            let response = MenuFieldF64::new(tr!("edit-angle"), &mut editor.offset_angle_degrees, -90.0..=90.0)
                .help_text(tr!("edit-slope-angle-offset-positive-negative"))
                .width(70.0)
                .speed(0.5)
                .suffix(tr!("common-degree-suffix"))
                .show(ui);
            if response.changed() {
                editor.offset_angle_degrees = editor.offset_angle_degrees.clamp(-90.0, 90.0);
            }

            ui.add_space(4.0);

            MenuField::new(tr!("edit-measure")).help_text(tr!("edit-measure-help")).show(ui, |ui, _, _| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut editor.offset_measure, OffsetMeasure::Distance, tr!("edit-distance"));
                    ui.selectable_value(&mut editor.offset_measure, OffsetMeasure::Width, tr!("drill-hole-width"));
                    let height_active = matches!(editor.offset_measure, OffsetMeasure::Height(_));
                    if ui.add(egui::Button::selectable(height_active, tr!("edit-height"))).clicked() && !height_active {
                        editor.offset_measure = OffsetMeasure::Height(HeightMode::Relative);
                    }
                })
                .response
            });
            if let OffsetMeasure::Height(ref mut mode) = editor.offset_measure {
                MenuField::new(tr!("edit-height-mode")).help_text(tr!("edit-elevation-mode-help")).show(ui, |ui, _, _| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(mode, HeightMode::Relative, tr!("edit-relative"));
                        ui.selectable_value(mode, HeightMode::AbsoluteRL, tr!("edit-absolute-rl"));
                    })
                    .response
                });
            }

            ui.add_space(4.0);

            // Value label adapts to context
            let value_label = match editor.offset_measure {
                OffsetMeasure::Distance => tr!("edit-distance-along-slope"),
                OffsetMeasure::Width => tr!("edit-horizontal-distance"),
                OffsetMeasure::Height(HeightMode::Relative) => tr!("edit-height-change"),
                OffsetMeasure::Height(HeightMode::AbsoluteRL) => tr!("edit-target-rl"),
            };
            let value_range = if matches!(editor.offset_measure, OffsetMeasure::Height(_)) {
                f64::MIN..=f64::MAX
            } else {
                0.0..=f64::MAX
            };
            MenuFieldF64::new(value_label, &mut editor.offset_value_input, value_range)
                .help_text(tr!("edit-value-help"))
                .speed(0.1)
                .suffix(tr!("common-m"))
                .show(ui);

            MenuFieldBool::new(tr!("edit-collide-triangulation"), &mut editor.offset_collide_with_triangulation)
                .help_text(tr!("edit-stop-generated-offset-where-its"))
                .show(ui);

            ui.add_space(8.0);
            let can_pick_side = matches!(editor.offset_measure, OffsetMeasure::Height(HeightMode::AbsoluteRL)) || editor.offset_value_input.abs() > 1e-9;
            let enter_pressed = ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let pick_side_clicked = ui.add(MenuButton::new(tr!("edit-pick-side")).enabled(can_pick_side)).clicked();
                if can_pick_side && (pick_side_clicked || enter_pressed) {
                    queue_begin_offset_pick(commands, editor);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() {
                    commands.push(UiCommand::CancelOffset);
                }
            });
        });
}

fn queue_begin_offset_pick(commands: &mut Vec<UiCommand>, editor: &EditorState) {
    let rad = editor.offset_angle_degrees.to_radians();
    let tan = rad.tan();
    let (horiz_dist, z_delta, project_to_rl) = match editor.offset_measure {
        OffsetMeasure::Distance => {
            let h = editor.offset_value_input * rad.sin();
            let horiz = editor.offset_value_input * rad.cos();
            (horiz, h, None)
        }
        OffsetMeasure::Width => (editor.offset_value_input, editor.offset_value_input * tan, None),
        OffsetMeasure::Height(HeightMode::Relative) => {
            if tan.abs() < 1e-9 {
                (editor.offset_value_input, 0.0, None)
            } else {
                (editor.offset_value_input / tan, editor.offset_value_input, None)
            }
        }
        OffsetMeasure::Height(HeightMode::AbsoluteRL) => {
            // Project each vertex individually along the batter angle so the
            // whole string lands flat at the target RL.
            (0.0, 0.0, Some((tan, editor.offset_value_input)))
        }
    };

    commands.push(UiCommand::BeginOffsetPick {
        object_ids: editor.offset_target_ids.clone(),
        horiz_dist,
        z_delta,
        project_to_rl,
        collide_with_triangulation: editor.offset_collide_with_triangulation,
    });
}

/// Draw the Generate Batter-Berms dialog.
pub(crate) fn draw_batter_berm_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState, viewport_rect: egui::Rect) {
    const CONTROL_WIDTH: f32 = 120.0;

    if editor.batter_berm_target_id.is_none() {
        return;
    }

    ViewportDockPanel::new("batter_berm_panel", tr!("edit-generate-batter-berms"), viewport_rect)
        .min_width(310.0)
        .show(ui.ctx(), |ui| {
            MenuFieldF64::new(tr!("edit-berm-width"), &mut editor.batter_berm_width, 0.1..=500.0)
                .help_text(tr!("edit-horizontal-width-each-flat-berm"))
                .width(CONTROL_WIDTH)
                .speed(0.1)
                .max_decimals(2)
                .suffix(tr!("common-m"))
                .show(ui);
            MenuFieldF64::new(tr!("edit-batter-angle"), &mut editor.batter_berm_angle, 1.0..=89.0)
                .help_text(tr!("edit-slope-angle-each-batter-face"))
                .width(CONTROL_WIDTH)
                .speed(0.5)
                .max_decimals(1)
                .show(ui);
            MenuFieldF64::new(tr!("edit-bench-height"), &mut editor.batter_berm_bench_height, 0.1..=500.0)
                .help_text(tr!("edit-vertical-rise-fall-each-bench"))
                .width(CONTROL_WIDTH)
                .speed(0.1)
                .max_decimals(2)
                .suffix(tr!("common-m"))
                .show(ui);
            let max_benches = editor.batter_berm_max_benches;
            if max_benches == 0 {
                editor.batter_berm_benches = 0;
            } else {
                editor.batter_berm_benches = editor.batter_berm_benches.clamp(1, max_benches);
            }
            ui.add_enabled_ui(max_benches > 0, |ui| {
                MenuFieldU32::new(tr!("edit-benches"), &mut editor.batter_berm_benches, if max_benches == 0 { 0..=0 } else { 1..=max_benches })
                    .help_text(tr!("edit-number-complete-batter-berm-levels"))
                    .width(CONTROL_WIDTH)
                    .speed(0.1)
                    .show(ui);
            });

            MenuField::new(tr!("edit-type"))
                .help_text(tr!("edit-type-direction-together-set-offset"))
                .show(ui, |ui, row_height, _| {
                    let gap = ui.spacing().item_spacing.x;
                    let button_width = (CONTROL_WIDTH - gap) * 0.5;
                    ui.allocate_ui_with_layout(egui::vec2(CONTROL_WIDTH, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                MenuButton::new(tr!("edit-pit"))
                                    .selected(editor.batter_berm_mode == BatterBermMode::Pit)
                                    .min_width(button_width),
                            )
                            .clicked()
                        {
                            editor.batter_berm_mode = BatterBermMode::Pit;
                        }
                        if ui
                            .add(
                                MenuButton::new(tr!("edit-stockpile"))
                                    .selected(editor.batter_berm_mode == BatterBermMode::Stockpile)
                                    .min_width(button_width),
                            )
                            .clicked()
                        {
                            editor.batter_berm_mode = BatterBermMode::Stockpile;
                        }
                    })
                    .response
                });

            MenuField::new(tr!("edit-direction"))
                .help_text(tr!("edit-bench-direction-help"))
                .show(ui, |ui, row_height, _| {
                    let gap = ui.spacing().item_spacing.x;
                    let button_width = (CONTROL_WIDTH - gap) * 0.5;
                    ui.allocate_ui_with_layout(egui::vec2(CONTROL_WIDTH, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if ui
                            .add(MenuButton::new(tr!("common-up")).selected(editor.batter_berm_direction_up).min_width(button_width))
                            .clicked()
                        {
                            editor.batter_berm_direction_up = true;
                        }
                        if ui
                            .add(MenuButton::new(tr!("common-down")).selected(!editor.batter_berm_direction_up).min_width(button_width))
                            .clicked()
                        {
                            editor.batter_berm_direction_up = false;
                        }
                    })
                    .response
                });

            ui.add_space(8.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(MenuButton::new(tr!("edit-apply")).primary().enabled(!editor.batter_berm_rings_world.is_empty()))
                    .clicked()
                {
                    commands.push(UiCommand::CommitBatterBerm);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() {
                    commands.push(UiCommand::CancelBatterBerm);
                }
            });
        });
}

/// Draw the Relimit Line dialog (intersect, absolute length, or relative length modes).
pub(crate) fn draw_relimit_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState, viewport_rect: egui::Rect) {
    ViewportDockPanel::new("relimit_line_panel", tr!("common-relimit-line"), viewport_rect)
        // The three mode labels need enough room to stay visually separate
        // from the field's info marker.
        .min_width(335.0)
        .max_width(335.0)
        .show(ui.ctx(), |ui| {
            // Mode tabs
            let previous_mode = editor.relimit_mode;
            MenuField::new(tr!("edit-mode"))
                .help_text(tr!("relimit-mode-help"))
                .show(ui, |ui, _, _| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut editor.relimit_mode,
                            RelimitMode::Intersect,
                            tr!("edit-intersect"),
                        );
                        ui.selectable_value(
                            &mut editor.relimit_mode,
                            RelimitMode::AbsoluteLength,
                            tr!("edit-absolute-length"),
                        );
                        ui.selectable_value(
                            &mut editor.relimit_mode,
                            RelimitMode::RelativeLength,
                            tr!("edit-relative"),
                        );
                    })
                    .response
                });
            if editor.relimit_mode == RelimitMode::Intersect
                && previous_mode != RelimitMode::Intersect
            {
                editor.relimit_waiting_for_pick = true;
                editor.relimit_confirming_end = false;
            }

            ui.add_space(4.0);

            match editor.relimit_mode {
                RelimitMode::Intersect => {
                    if editor.relimit_waiting_for_pick {
                        ui.label(tr!("relimit-click-boundary"));
                    } else if editor.relimit_confirming_end {
                        ui.label(tr!("edit-hover-choose-which-end-move"));
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 180, 0),
                            match editor.relimit_hover_end {
                                TrimEnd::Start => tr!("edit-moving-start-endpoint"),
                                TrimEnd::End => tr!("edit-moving-end-endpoint"),
                            },
                        );
                    }
                }
                RelimitMode::AbsoluteLength | RelimitMode::RelativeLength => {
                    let label = if matches!(editor.relimit_mode, RelimitMode::AbsoluteLength) {
                        tr!("edit-new-length-m")
                    } else {
                        tr!("edit-delta-length-m-use")
                    };
                    MenuFieldF64::new(label, &mut editor.relimit_value_input, f64::MIN..=f64::MAX)
                        .help_text(tr!("edit-selected-start-end-point-moves"))
                        .speed(0.1)
                        .suffix(tr!("common-m"))
                        .show(ui);
                    MenuField::new(tr!("edit-move-which-end"))
                        .help_text(tr!("edit-endpoint-help"))
                        .show(ui, |ui, _, _| {
                            ui.horizontal(|ui| {
                                ui.selectable_value(
                                    &mut editor.relimit_resize_end,
                                    TrimEnd::Start,
                                    tr!("edit-start"),
                                );
                                ui.selectable_value(
                                    &mut editor.relimit_resize_end,
                                    TrimEnd::End,
                                    tr!("edit-end"),
                                );
                            })
                            .response
                        });
                }
            }

            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match editor.relimit_mode {
                    RelimitMode::Intersect => {
                        if ui.add(MenuButton::new(tr!("edit-apply-pick-target")).primary()).clicked() {
                            editor.relimit_dialog_open = false;
                            editor.relimit_waiting_for_pick = true;
                        }
                    }
                    RelimitMode::AbsoluteLength | RelimitMode::RelativeLength => {
                        if ui.add(MenuButton::new(tr!("edit-apply")).primary()).clicked()
                            && editor.relimit_value_input.is_finite()
                            && let Some(source_id) = editor.relimit_source_id
                        {
                            commands.push(UiCommand::RelimitLineResize {
                                source_id,
                                mode: editor.relimit_mode,
                                value: editor.relimit_value_input,
                            });
                            editor.relimit_dialog_open = false;
                        }
                    }
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() {
                    commands.push(UiCommand::CancelRelimit);
                }
            });
        });
}

/// Draw the floating Move tool panel with dX/dY/dZ inputs and an Apply button.
pub(crate) fn draw_move_panel(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>, viewport_rect: egui::Rect) {
    // One panel for both translate tools, titled by the one running it.
    let title = if editor.active_tool == ActiveTool::MoveCollar {
        tr!("common-move-collar")
    } else {
        tr!("common-move-design")
    };
    ViewportDockPanel::new("move_panel", title, viewport_rect).min_width(210.0).show(ui.ctx(), |ui| {
        let dx_resp = MenuFieldF64::new(tr!("edit-dx"), &mut editor.move_panel_delta[0], f64::MIN..=f64::MAX)
            .help_text(tr!("edit-translation-axis-help", axis = crate::model::survey::axis_name(0).to_string()))
            .speed(0.1)
            .show(ui);
        let dy_resp = MenuFieldF64::new(tr!("edit-dy"), &mut editor.move_panel_delta[1], f64::MIN..=f64::MAX)
            .help_text(tr!("edit-translation-axis-help", axis = crate::model::survey::axis_name(1).to_string()))
            .speed(0.1)
            .show(ui);
        let dz_resp = MenuFieldF64::new(tr!("edit-dz"), &mut editor.move_panel_delta[2], f64::MIN..=f64::MAX)
            .help_text(tr!("edit-translation-axis-help", axis = crate::model::survey::axis_name(2).to_string()))
            .speed(0.1)
            .show(ui);
        if dx_resp.changed() || dy_resp.changed() || dz_resp.changed() {
            commands.push(UiCommand::PreviewMoveDelta(glam::DVec3::new(
                editor.move_panel_delta[0],
                editor.move_panel_delta[1],
                editor.move_panel_delta[2],
            )));
        }

        ui.add_space(4.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(MenuButton::new(tr!("edit-apply")).primary()).clicked() {
                commands.push(UiCommand::ApplyMoveDelta(glam::DVec3::new(
                    editor.move_panel_delta[0],
                    editor.move_panel_delta[1],
                    editor.move_panel_delta[2],
                )));
                editor.active_tool = ActiveTool::None;
            }
            if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() {
                commands.push(UiCommand::CancelMoveDelta);
                editor.active_tool = ActiveTool::None;
            }
        });
    });
}

/// Draw the floating Rotate Collar panel: the two angles a drill plan is
/// written in, and an Apply button.
///
/// The values are absolute, not a delta - a round is drilled at one angle, so
/// Apply points every selected hole this way. A ring drag is the delta gesture,
/// and it drives these same fields as it goes, so the panel always reads out
/// the setup the holes are standing at.
pub(crate) fn draw_rotate_collar_panel(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>, viewport_rect: egui::Rect) {
    use crate::model::drill_hole::{CollarRotation, HoleOrientation, MAX_HOLE_DIP};

    ViewportDockPanel::new("rotate_collar_panel", tr!("common-rotate-collar"), viewport_rect)
        .min_width(230.0)
        .show(ui.ctx(), |ui| {
            if editor.rotate_panel_mixed {
                ui.label(tr!("edit-selected-holes-point-different-ways"));
                ui.add_space(2.0);
            }
            let azimuth = MenuFieldF64::new(tr!("edit-azimuth"), &mut editor.rotate_panel_azimuth, 0.0..=360.0)
                .help_text(tr!("edit-azimuth-help"))
                .speed(0.25)
                .show(ui);
            let dip = MenuFieldF64::new(tr!("edit-dip"), &mut editor.rotate_panel_dip, -MAX_HOLE_DIP..=MAX_HOLE_DIP)
                .help_text(tr!("edit-dip-help"))
                .speed(0.25)
                .show(ui);
            let target = CollarRotation::Absolute(HoleOrientation {
                azimuth: editor.rotate_panel_azimuth,
                dip: editor.rotate_panel_dip,
            });
            if azimuth.changed() || dip.changed() {
                commands.push(UiCommand::PreviewCollarRotation(target));
            }

            ui.add_space(4.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(MenuButton::new(tr!("edit-apply")).primary()).clicked() {
                    commands.push(UiCommand::ApplyCollarRotation);
                    editor.active_tool = ActiveTool::None;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() {
                    commands.push(UiCommand::CancelCollarRotation);
                    editor.active_tool = ActiveTool::None;
                }
            });
        });
}

/// Chamfer tool viewport dock: segments input + Apply / Cancel.
pub(crate) fn draw_chamfer_panel(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>, viewport_rect: egui::Rect) {
    let corner_picked = editor.chamfer_poly_id.is_some() && editor.chamfer_corner_index.is_some();

    ViewportDockPanel::new("chamfer_panel", tr!("common-chamfer"), viewport_rect)
        .min_width(200.0)
        .show(ui.ctx(), |ui| {
            if !corner_picked {
                ui.label(tr!("edit-click-corner-closed-polyline"));
            } else {
                MenuFieldU32::new(tr!("edit-segments"), &mut editor.chamfer_segments, 1..=64)
                    .help_text(tr!("edit-chamfer-segments-help"))
                    .speed(0.1)
                    .show(ui);

                let mut r = editor.chamfer_radius;
                let max_r = if editor.chamfer_max_radius.is_finite() { editor.chamfer_max_radius } else { f64::MAX };
                MenuFieldF64::new(tr!("edit-radius"), &mut r, 0.0..=max_r)
                    .help_text(tr!("edit-corner-radius-limited-so-replacement"))
                    .speed(0.05)
                    .show(ui);
                editor.chamfer_radius = r.clamp(0.0, max_r);
            }

            ui.add_space(4.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Grey out when the displayed value is "0.00" (2 dp) - matches user perception.
                let can_apply = corner_picked && (editor.chamfer_radius * 100.0).round() > 0.0;
                if ui.add(MenuButton::new(tr!("edit-apply")).primary().enabled(can_apply)).clicked() && can_apply {
                    commands.push(UiCommand::ApplyChamfer);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() {
                    commands.push(UiCommand::CancelChamfer);
                }
            });
        });
}

fn draw_bezier_xyz(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, help_text: impl Into<egui::WidgetText>, point: &mut [f64; 3]) {
    const TOTAL_WIDTH: f32 = 270.0;
    MenuField::new(label).help_text(help_text).show(ui, |ui, row_height, _| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let value_width = (TOTAL_WIDTH - ui.spacing().item_spacing.x * 2.0) / 3.0;
        ui.horizontal(|ui| {
            for (axis, value) in ["X ", "Y ", "Z "].into_iter().zip(point.iter_mut()) {
                ui.add_sized([value_width, row_height], egui::DragValue::new(value).speed(0.1).max_decimals(3).prefix(axis));
            }
        })
        .response
    });
}

/// Bezier curve editor: vertex selection status, control point inputs, and Apply/Cancel.
pub(crate) fn draw_bezier_panel(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>, viewport_rect: egui::Rect) {
    let both_selected = editor.bezier_selected_verts[0].is_some() && editor.bezier_selected_verts[1].is_some();

    ViewportDockPanel::new("bezier_panel", tr!("edit-bezier-curve"), viewport_rect)
        .min_width(400.0)
        .show(ui.ctx(), |ui| {
            if editor.bezier_poly_id.is_none() {
                ui.label(tr!("edit-click-open-closed-polyline-begin"));
            } else if !both_selected {
                match editor.bezier_selected_verts[0] {
                    None => {
                        ui.label(tr!("edit-click-vertex-start-replacement-span"));
                    }
                    Some(_) => {
                        ui.label(tr!("edit-click-second-vertex-replacement-span"));
                    }
                }
            } else {
                MenuFieldU32::new(tr!("edit-segments"), &mut editor.bezier_segments, 2..=64)
                    .help_text(tr!("edit-bezier-segments-help"))
                    .speed(0.1)
                    .show(ui);

                if editor.bezier_poly_closed {
                    let previous = editor.bezier_replace_longer;
                    MenuField::new(tr!("edit-replace-path"))
                        .help_text(tr!("edit-choose-which-two-polyline-paths"))
                        .show(ui, |ui, row_height, _| {
                            ui.horizontal(|ui| {
                                if ui
                                    .add_sized([82.0, row_height], egui::Button::selectable(!editor.bezier_replace_longer, tr!("edit-shortest")))
                                    .clicked()
                                {
                                    editor.bezier_replace_longer = false;
                                }
                                if ui
                                    .add_sized([82.0, row_height], egui::Button::selectable(editor.bezier_replace_longer, tr!("edit-longest")))
                                    .clicked()
                                {
                                    editor.bezier_replace_longer = true;
                                }
                            })
                            .response
                        });
                    if editor.bezier_replace_longer != previous {
                        editor.bezier_selected_verts.swap(0, 1);
                        std::mem::swap(&mut editor.bezier_cp1, &mut editor.bezier_cp2);
                    }
                }

                draw_bezier_xyz(ui, tr!("edit-control-point-1"), tr!("edit-bezier-control-point-1-help"), &mut editor.bezier_cp1);
                draw_bezier_xyz(ui, tr!("edit-control-point-2"), tr!("edit-bezier-control-point-2-help"), &mut editor.bezier_cp2);
            }

            ui.add_space(4.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_apply = both_selected;
                let apply_clicked = ui.add(MenuButton::new(tr!("edit-apply")).primary().enabled(can_apply)).clicked();
                let enter_pressed = ui.input(|input| input.key_pressed(egui::Key::Enter));
                if can_apply && (apply_clicked || enter_pressed) {
                    commands.push(UiCommand::ApplyBezier);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() {
                    commands.push(UiCommand::CancelBezier);
                }
            });
        });
}

/// Slice view dock: slab width, movement speed, Q/E rotate rate, reset, and exit.
pub(crate) fn draw_slice_panel(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>, viewport_rect: egui::Rect) {
    ViewportDockPanel::new("slice_panel", tr!("edit-slice-view"), viewport_rect)
        .min_width(210.0)
        .show(ui.ctx(), |ui| {
            MenuFieldF64::new(tr!("drill-hole-width"), &mut editor.slice_width_input, 0.1..=1.0e6)
                .help_text(tr!("edit-thickness-visible-slice-slab-centred"))
                .speed(0.5)
                .suffix(tr!("common-m"))
                .show(ui);
            MenuFieldF64::new(tr!("edit-speed"), &mut editor.slice_speed_input, 0.0..=1.0e6)
                .help_text(tr!("edit-movement-speed-slice-when-using"))
                .speed(1.0)
                .suffix(tr!("edit-m-s"))
                .show(ui);
            MenuFieldF64::new(tr!("edit-rotate"), &mut editor.slice_rotate_input, 1.0..=720.0)
                .help_text(tr!("edit-rotation-speed-slice-when-using"))
                .speed(1.0)
                .suffix(tr!("edit-s"))
                .show(ui);
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(MenuButton::new(tr!("edit-exit-slice"))).clicked() {
                    commands.push(UiCommand::SetSliceModeEnabled(false));
                }
            });
        });
}
