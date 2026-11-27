//! Application preferences and selection appearance controls.

use crate::{
    i18n::tr,
    model::{
        Document, FillStyle, ObjectColor, ObjectId, SceneEntityId,
        block_model::{BlockModelId, OpenBlockModel},
    },
    rendering::color::{color32_to_rgba, rgba_to_color32},
    ui::{
        UiCommand, UiProjectView,
        state::{EditorState, PreferencesDraft, PropertyTab},
        widgets::{
            collapsible_section::CollapsibleSection,
            context_menu::{FIELD_WIDTH, context_menu_fields},
            hint::Hint,
            menu::{self, MenuFieldBool, MenuFieldColor32, MenuFieldCombo, MenuFieldF32, MenuFieldF64, MenuFieldU32, committed, menu_field_label},
            viewport::{BlockModelProperties, ViewportDockPanel},
        },
    },
};

/// The Preferences window's size, on a screen with room for it. The import
/// and export dialogs are as tall (`dialogs::import_export`).
pub(crate) const PREFERENCES_SIZE: egui::Vec2 = egui::vec2(600.0, 380.0);

pub(crate) fn draw_preferences(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.show_preferences {
        return;
    }
    let mut open = true;
    let available = ui.ctx().content_rect().size() - egui::vec2(24.0, 24.0);
    let scale = (available.x / PREFERENCES_SIZE.x).min(available.y / PREFERENCES_SIZE.y).clamp(0.1, 1.0);
    let size = PREFERENCES_SIZE * scale;
    crate::ui::widgets::menu::DragableMenu::new("preferences", tr!("preferences-title"))
        .open(&mut open)
        .fixed_size(size)
        .inner_margin(egui::Margin::ZERO)
        .show(ui.ctx(), |ui| {
            let body_height = (size.y - menu::TITLE_BAR_HEIGHT).max(0.0);
            let (body, _) = ui.allocate_exact_size(egui::vec2(size.x, body_height), egui::Sense::hover());
            let navigation = egui::Rect::from_min_max(body.min, egui::pos2(body.left() + 150.0 * scale, body.bottom()));
            let page = egui::Rect::from_min_max(egui::pos2(navigation.right() + 1.0, body.top()), body.max);
            let (surface, stripe) = crate::ui::widgets::tree_row_colors(ui);
            ui.painter().rect_filled(navigation, 0.0, surface);
            ui.painter().add(crate::ui::widgets::explorer::stripe_bands(
                navigation.x_range(),
                navigation.top(),
                navigation.bottom(),
                crate::ui::widgets::explorer::row_height(ui),
                stripe,
            ));
            ui.painter()
                .line_segment([navigation.right_top(), navigation.right_bottom()], ui.visuals().widgets.noninteractive.bg_stroke);
            ui.scope_builder(egui::UiBuilder::new().id_salt("preferences_navigation").max_rect(navigation), |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(navigation));
                let row_height = crate::ui::widgets::explorer::row_height(ui);
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.spacing_mut().interact_size.y = row_height;
                ui.spacing_mut().button_padding.y = 0.0;
                for (tab, label) in [
                    (PropertyTab::Interface, tr!("properties-interface")),
                    (PropertyTab::Camera, tr!("properties-camera")),
                    (PropertyTab::Performance, tr!("properties-performance")),
                    (PropertyTab::Data, tr!("properties-data")),
                    (PropertyTab::Mcp, tr!("mcp-title")),
                    (PropertyTab::Developer, tr!("properties-developer")),
                ] {
                    let response = crate::ui::widgets::explorer::ExplorerEntry::new(ui.scope_id().with(tab as u8), label)
                        .selected(editor.active_property_tab == tab)
                        .show(ui)
                        .response;
                    if response.clicked() {
                        editor.active_property_tab = tab;
                    }
                }
            });
            ui.scope_builder(egui::UiBuilder::new().id_salt("preferences_page").max_rect(page.shrink2(egui::vec2(12.0, 8.0))), |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(page));
                egui::ScrollArea::both()
                    .id_salt(("preferences_page_scroll", editor.active_property_tab as u8))
                    .auto_shrink([false; 2])
                    .show(ui, |ui| match editor.active_property_tab {
                        PropertyTab::Interface => draw_interface_settings(ui, editor, commands),
                        PropertyTab::Camera => draw_camera_settings(ui, editor, commands),
                        PropertyTab::Performance => draw_performance_settings(ui, editor, commands),
                        PropertyTab::Data => draw_data_settings(ui, project, commands),
                        PropertyTab::Mcp => draw_mcp_settings(ui, editor, commands),
                        PropertyTab::Developer => draw_developer_settings(ui, editor, commands),
                    });
            });
        });
    editor.show_preferences = open;
}

struct PropertyContext {
    objects: Vec<ObjectId>,
    polylines: Vec<ObjectId>,
}

pub(crate) fn draw_selection_appearance(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    project: &UiProjectView,
    document: &Document,
    commands: &mut Vec<UiCommand>,
    geometry_dirty: &mut bool,
) {
    let mut objects: Vec<_> = editor
        .selected_handles
        .iter()
        .filter_map(|id| match id {
            SceneEntityId::Object(id) if document.get_object(*id).is_some() => Some(*id),
            _ => None,
        })
        .collect();
    objects.sort_by_key(|id| id.0);
    let mut selected_types = [false; 8];
    for handle in &editor.selected_handles {
        let kind = match handle {
            SceneEntityId::Object(id) => match document.get_object(*id) {
                Some(crate::model::Object::Polyline { .. } | crate::model::Object::Circle { .. } | crate::model::Object::PitDesign { .. }) => 0,
                Some(crate::model::Object::Point { .. }) => 1,
                Some(crate::model::Object::Text { .. }) => 2,
                None => continue,
            },
            SceneEntityId::Triangulation(_) => 3,
            SceneEntityId::BlockModel(_) => 4,
            SceneEntityId::DrillHole(_) => 5,
            SceneEntityId::PointCloud(_) => 6,
            SceneEntityId::Raster(_) => 7,
            SceneEntityId::HaulRoad(_) | SceneEntityId::HaulNode(_) => continue,
        };
        selected_types[kind] = true;
    }
    let show_headings = selected_types.into_iter().filter(|selected| *selected).count() > 1;
    for (kind, heading) in [(0, tr!("context-polylines")), (1, tr!("context-points")), (2, tr!("common-text"))] {
        let objects: Vec<_> = objects
            .iter()
            .copied()
            .filter(|id| match document.get_object(*id) {
                Some(crate::model::Object::Polyline { .. } | crate::model::Object::Circle { .. } | crate::model::Object::PitDesign { .. }) => kind == 0,
                Some(crate::model::Object::Point { .. }) => kind == 1,
                Some(crate::model::Object::Text { .. }) => kind == 2,
                None => false,
            })
            .collect();
        if objects.is_empty() {
            continue;
        }
        if show_headings {
            context_menu_fields(ui, |ui| menu::menu_section(ui, heading));
        }
        let polylines = if kind == 0 { objects.clone() } else { Vec::new() };
        context_menu_fields(ui, |ui| {
            draw_design_tab(
                ui,
                editor,
                document,
                &PropertyContext {
                    objects: objects.clone(),
                    polylines,
                },
                commands,
                geometry_dirty,
            );
        });
        // The object editor needs exactly one selected design object.
        if editor.selected_handles.len() == 1
            && let [object_id] = objects.as_slice()
            && crate::ui::widgets::context_menu::ContextMenuAction::new(tr!("properties-edit-object")).show(ui).clicked()
        {
            commands.push(UiCommand::OpenObjectEditDialog(*object_id));
            commands.push(UiCommand::CloseCanvasContextMenu);
        }
        // A pit design opens in Pit Design, and a closed polyline can become
        // one there.
        if editor.selected_handles.len() == 1
            && let [object_id] = objects.as_slice()
            && let Some(object) = document.get_object(*object_id)
        {
            let pit = matches!(object, crate::model::Object::PitDesign { .. });
            if (pit || (matches!(object, crate::model::Object::Polyline { .. }) && object.encloses_area()))
                && crate::ui::widgets::context_menu::ContextMenuAction::new(tr!("pit-design-menu")).show(ui).clicked()
            {
                commands.push(UiCommand::OpenPitDesign);
                commands.push(UiCommand::CloseCanvasContextMenu);
            }
            if pit && crate::ui::widgets::context_menu::ContextMenuAction::new(tr!("pit-design-explode")).show(ui).clicked() {
                commands.push(UiCommand::ExplodePitDesign(*object_id));
                commands.push(UiCommand::CloseCanvasContextMenu);
            }
        }
        if crate::ui::widgets::context_menu::ContextMenuAction::new(tr!("properties-move-layer")).show(ui).clicked() {
            let target_layer = project
                .projects
                .iter()
                .find(|entry| entry.is_active)
                .and_then(|entry| entry.layers.iter().find(|layer| layer.reference.is_none()))
                .map(|layer| layer.id);
            editor.move_to_layer_dialog = Some(crate::ui::state::MoveToLayerDialog {
                object_ids: objects,
                target_layer,
                copy: false,
            });
            commands.push(UiCommand::CloseCanvasContextMenu);
        }
    }
    let triangulations: Vec<_> = project
        .triangulations
        .iter()
        .filter(|tri| tri.is_loaded && editor.selected_handles.contains(&SceneEntityId::Triangulation(tri.id)))
        .collect();
    let has_appearance = !objects.is_empty() || !triangulations.is_empty();
    if let Some(first) = triangulations.first() {
        context_menu_fields(ui, |ui| {
            if show_headings {
                menu::menu_section(ui, tr!("common-triangulations"));
            }
            let mut color = rgba_to_color32(first.color);
            if committed(&MenuFieldColor32::new(tr!("explorer-face-colour"), &mut color).show(ui)) {
                for tri in triangulations {
                    commands.push(UiCommand::SetTriangulationColor(tri.id, color32_to_rgba(color)));
                }
                *geometry_dirty = true;
            }
        });
    }
    if has_appearance {
        crate::ui::widgets::context_menu::context_menu_separator(ui);
    }
}

/// Width of the block-model card in the viewport.
const BLOCK_MODEL_CARD_WIDTH: f32 = 780.0;
/// Gap between that card and the viewport's bottom edge, the tool panels'
/// corner and the scale bar.
const BLOCK_MODEL_CARD_GAP: f32 = 8.0;

/// Draw the block-model card along the viewport's bottom edge, and return
/// whether the scale bar, laid out at `scale_bar`, keeps its place there.
///
/// The edge is shared in order of priority: a tool panel open in its
/// bottom-left corner, then this card, then the scale bar. The card sits
/// centred under the window where it can, slides left towards the tool panel,
/// or the viewport's edge while none is open, when it cannot, and takes the
/// scale bar's room too before it gives way. Once it cannot fit even then it
/// is not drawn, and the scale bar comes back; the scale bar gives way only
/// to a tool panel.
pub(crate) fn draw_block_model_controls(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    models: &[OpenBlockModel],
    commands: &mut Vec<UiCommand>,
    canvas: egui::Rect,
    scale_bar: Option<egui::Rect>,
) -> bool {
    let left_bound = crate::ui::widgets::viewport::docked_left_edge(ui.ctx()).unwrap_or(canvas.left()) + BLOCK_MODEL_CARD_GAP;
    let scale_bar = scale_bar.filter(|bar| bar.left() >= left_bound);
    let selected: Vec<_> = models
        .iter()
        .filter(|model| model.state.loaded && editor.selected_handles.contains(&SceneEntityId::BlockModel(model.id)))
        .collect();
    let selection_id = egui::Id::unique("block_model_controls_selection");
    let selection: Vec<_> = selected.iter().map(|model| model.id).collect();
    let selection_changed = ui.ctx().data_mut(|data| {
        let changed = data.get_temp::<Vec<crate::model::block_model::BlockModelId>>(selection_id).as_ref() != Some(&selection);
        data.insert_temp(selection_id, selection);
        changed
    });
    if selection_changed && !selected.is_empty() && !selected.iter().any(|model| Some(model.id) == editor.viewport_block_model_id) {
        editor.viewport_block_model_id = Some(selected[0].id);
    }
    let Some(model) = models.iter().find(|model| Some(model.id) == editor.viewport_block_model_id && model.state.loaded) else {
        return scale_bar.is_some();
    };
    let area_id = egui::Id::unique("block_model_viewport_controls");
    let card_size = ui
        .ctx()
        .memory(|memory| memory.area_rect(area_id).map_or(egui::vec2(BLOCK_MODEL_CARD_WIDTH, 0.0), |rect| rect.size()));
    let centred = ui.ctx().content_rect().center().x - card_size.x / 2.0;
    let place = |right_bound: f32| {
        let left = centred.max(left_bound);
        (left + card_size.x <= right_bound).then_some(left)
    };
    // Always its full height, never scrolled or squeezed: a viewport too
    // short to hold it hides it, the same as one too narrow.
    if card_size.y > canvas.height() - 2.0 * BLOCK_MODEL_CARD_GAP {
        return scale_bar.is_some();
    }
    let (left, keeps_scale_bar) = if let Some(left) = scale_bar.and_then(|bar| place(bar.left() - BLOCK_MODEL_CARD_GAP)) {
        (left, true)
    } else if let Some(left) = place(canvas.right() - BLOCK_MODEL_CARD_GAP) {
        (left, false)
    } else {
        return scale_bar.is_some();
    };
    // The heading runs after the body, so it works on its own copy of the
    // shown model and the body keeps the editor.
    let mut shown = editor.viewport_block_model_id;
    ViewportDockPanel::new(area_id, tr!("common-block-model"), canvas)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::pos2(left, canvas.bottom() - BLOCK_MODEL_CARD_GAP))
        .order(egui::Order::Middle)
        .min_width(BLOCK_MODEL_CARD_WIDTH)
        .max_width(BLOCK_MODEL_CARD_WIDTH)
        // No top margin: the row gap under the heading already gives the
        // body the room it had when the card drew its own frame.
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 0,
            bottom: 10,
        })
        .show_with_heading(
            ui.ctx(),
            |ui, rect, surface| draw_block_model_heading_controls(ui, &mut shown, models, model, rect, surface),
            |ui| {
                ui.push_id(model.id, |ui| {
                    BlockModelProperties::new(("block_model_controls", model.id), model).show(ui, editor, commands)
                });
            },
        );
    editor.viewport_block_model_id = shown;
    keeps_scale_bar
}

/// The block-model card's heading controls: the model it is showing - a
/// picker when there is more than one loaded - and a close cross. Closing
/// clears the shown model; selecting one reopens it.
fn draw_block_model_heading_controls(
    ui: &mut egui::Ui,
    shown: &mut Option<BlockModelId>,
    models: &[OpenBlockModel],
    model: &OpenBlockModel,
    rect: egui::Rect,
    surface: egui::Color32,
) {
    let loaded: Vec<_> = models.iter().filter(|model| model.state.loaded).collect();
    let picker_width = (rect.width() * 0.4).clamp(120.0, 280.0);
    let picker_rect = egui::Rect::from_min_size(
        egui::pos2(rect.right() - menu::TITLE_BAR_HEIGHT - 2.0 - picker_width, rect.center().y - 11.0),
        egui::vec2(picker_width, 22.0),
    );
    if loaded.len() > 1 {
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(picker_rect).layout(egui::Layout::right_to_left(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().interact_size.y = 22.0;
                egui::ComboBox::from_id_salt("filter_model")
                    .selected_text(model.name.clone())
                    .width(picker_width)
                    .truncate()
                    .show_ui(ui, |ui| {
                        for option in &loaded {
                            ui.selectable_value(shown, Some(option.id), option.name.clone());
                        }
                    })
                    .response
                    .hint(model.name.clone());
            },
        );
    } else {
        // Nothing to switch to: name the model without offering a choice.
        let mut job = egui::text::LayoutJob::single_section(
            model.name.clone(),
            egui::TextFormat {
                font_id: egui::FontId::proportional(12.0),
                color: ui.visuals().weak_text_color(),
                ..Default::default()
            },
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(picker_width);
        let galley = ui.painter().layout_job(job);
        ui.painter().galley(
            egui::pos2(picker_rect.right() - galley.size().x, picker_rect.center().y - galley.size().y / 2.0),
            galley,
            ui.visuals().weak_text_color(),
        );
    }

    if menu::title_bar_close_button(ui, rect, surface) {
        *shown = None;
    }
}

/// Runs `add_fields` against the editor's preferences draft and applies it as
/// soon as a field reports a finished edit.
///
/// The draft is held by the editor rather than rebuilt each frame because a
/// `DragValue` computes each step from the value it is handed: one that reset
/// to the unedited value every frame could never accumulate.
fn settings_section(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    commands: &mut Vec<UiCommand>,
    heading: &str,
    add_fields: impl FnOnce(&mut egui::Ui, &mut PreferencesDraft) -> bool,
    restore_defaults: Option<fn(&mut PreferencesDraft)>,
) {
    let saved = editor.current_preferences();
    let draft = editor.preferences_draft.get_or_insert(saved);
    menu::menu_section(ui, heading);
    let changed = add_fields(ui, draft);
    let draft = *draft;

    if let Some(reset_tab) = restore_defaults {
        ui.add_space(10.0);
        let mut restored = draft;
        reset_tab(&mut restored);
        // Extend, not the panel's global Truncate: a full-width button on a
        // narrow panel should keep its label and be reachable by scrolling
        // rather than read "Restore Def…".
        let restore_clicked = ui
            .scope(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.add_enabled(draft != restored, egui::Button::new(tr!("properties-restore-defaults"))).clicked()
            })
            .inner;
        if restore_clicked {
            commands.push(UiCommand::ApplyPreferences(restored));
        } else if changed && draft != saved {
            commands.push(UiCommand::ApplyPreferences(draft));
        }
    } else if changed && draft != saved {
        commands.push(UiCommand::ApplyPreferences(draft));
    }
    ui.add_space(6.0);
}

fn reset_interface_defaults(draft: &mut PreferencesDraft) {
    let defaults = PreferencesDraft::default();
    draft.dark_mode = defaults.dark_mode;
    draft.show_console = defaults.show_console;
    draft.ui_size_percent = defaults.ui_size_percent;
    draft.recent_functions_shown = defaults.recent_functions_shown;
}

fn reset_developer_defaults(draft: &mut PreferencesDraft) {
    let defaults = PreferencesDraft::default();
    draft.debug_surface_chunks = defaults.debug_surface_chunks;
    draft.debug_clip_planes = defaults.debug_clip_planes;
    draft.debug_point_cloud_chunks = defaults.debug_point_cloud_chunks;
}

fn reset_camera_defaults(draft: &mut PreferencesDraft) {
    let defaults = PreferencesDraft::default();
    draft.plan_orbit_sensitivity = defaults.plan_orbit_sensitivity;
    draft.plan_zoom_sensitivity = defaults.plan_zoom_sensitivity;
    draft.plan_invert_vertical_look = defaults.plan_invert_vertical_look;
    draft.plan_invert_horizontal_look = defaults.plan_invert_horizontal_look;
    draft.fly_field_of_view_degrees = defaults.fly_field_of_view_degrees;
    draft.fly_mouse_look_sensitivity = defaults.fly_mouse_look_sensitivity;
    draft.fly_invert_vertical_look = defaults.fly_invert_vertical_look;
    draft.fly_invert_horizontal_look = defaults.fly_invert_horizontal_look;
    draft.fly_near_clip_limit = defaults.fly_near_clip_limit;
    draft.fly_max_clip_span = defaults.fly_max_clip_span;
}

fn reset_performance_defaults(draft: &mut PreferencesDraft) {
    let defaults = PreferencesDraft::default();
    draft.snap_poll_rate = defaults.snap_poll_rate;
    draft.vsync_enabled = defaults.vsync_enabled;
    draft.frame_rate_cap = defaults.frame_rate_cap;
    draft.resize_frame_rate_cap = defaults.resize_frame_rate_cap;
    draft.block_model_interaction_resolution_divisor = defaults.block_model_interaction_resolution_divisor;
    draft.show_block_model_boundary_highlights = defaults.show_block_model_boundary_highlights;
    draft.downscale_raster_previews = defaults.downscale_raster_previews;
}

fn draw_interface_settings(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    // Language is not here: it is a status bar picker, so it is reachable
    // without opening a panel - see `elements::status_bar`.
    settings_section(
        ui,
        editor,
        commands,
        &tr!("properties-interface"),
        |ui, draft| {
            let mut changed = false;

            changed |= committed(&MenuFieldBool::new(tr!("properties-dark-mode"), &mut draft.dark_mode).show(ui));
            changed |= committed(&MenuFieldBool::new(tr!("properties-show-console"), &mut draft.show_console).show(ui));
            changed |= committed(
                &MenuFieldF64::new(tr!("preferences-ui-size"), &mut draft.ui_size_percent, 50.0..=200.0)
                    .help_text(tr!("preferences-ui-size-help"))
                    .suffix("%")
                    .speed(1.0)
                    .max_decimals(0)
                    .show(ui),
            );
            let shown = draft.recent_functions_shown.to_string();
            let options = [10, 15, 20].map(|count: u32| (count, count.to_string().into()));
            changed |= committed(
                &MenuFieldCombo::new(
                    "preferences_recent_functions_shown",
                    tr!("preferences-recent-functions-shown"),
                    &mut draft.recent_functions_shown,
                    shown,
                    options,
                )
                .show(ui),
            );
            changed
        },
        Some(reset_interface_defaults),
    );
}

/// The MCP server: on or off, its port, and where it is. Each change is made as it is committed, as the rest of Preferences'
/// are, and saved apart from them (`mcp`).
/// Preferences > Data: the OMF files every project shows read-only.
fn draw_data_settings(ui: &mut egui::Ui, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    menu::menu_section(ui, tr!("data-references-title"));
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (project, commands);
        menu::menu_note(ui, tr!("data-references-desktop-only"));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::model::project::ReferenceStatus;

        if project.data_references.is_empty() {
            ui.label(egui::RichText::new(tr!("data-references-none")).weak());
        }
        const ICON: f32 = 16.0;
        for reference in &project.data_references {
            // Loaded, a reference says what it holds with its sections'
            // icons; otherwise, why not.
            let status = match &reference.status {
                ReferenceStatus::Loading => Some((tr!("data-reference-loading"), false)),
                ReferenceStatus::Loaded { .. } => None,
                ReferenceStatus::Missing => Some((tr!("data-reference-missing"), true)),
                ReferenceStatus::OpenAsProject => Some((tr!("data-reference-open-as-project"), false)),
                ReferenceStatus::Failed(error) => Some((tr!("data-reference-failed", error = error.clone()), true)),
            };
            const BUTTON: f32 = 24.0;
            ui.horizontal(|ui| {
                // Room is kept for the two buttons, so a long path never
                // pushes them out of the panel.
                let room = (ui.available_width() - 2.0 * BUTTON - 2.0 - ui.spacing().item_spacing.x - 6.0).max(0.0);
                ui.vertical(|ui| {
                    ui.set_max_width(room);
                    // Where the file is, which is what tells two references
                    // apart. Too long for the panel, it loses its start: the
                    // end names the file. Whole in the hint.
                    let path = reference.path.display().to_string();
                    let font = egui::TextStyle::Body.resolve(ui.style());
                    let shown = elide_start(ui, &path, &font, room);
                    ui.add(egui::Label::new(egui::RichText::new(shown).font(font).color(ui.visuals().strong_text_color())).wrap_mode(egui::TextWrapMode::Extend))
                        .hint(path);
                    match status {
                        Some((text, failed)) => {
                            let text = egui::RichText::new(text).small();
                            ui.label(if failed { text.color(ui.visuals().warn_fg_color) } else { text.weak() });
                        }
                        None => {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                for (kind, label) in &reference.sections {
                                    ui.add(egui::Image::new(crate::ui::elements::explorer::kind_icon(*kind)).fit_to_exact_size(egui::vec2(ICON, ICON)))
                                        .hint(label.clone());
                                }
                            });
                        }
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let button = |ui: &mut egui::Ui, image: egui::Image<'static>, tooltip: String, salt: &str| {
                        ui.add(
                            crate::ui::widgets::toolbar::ToolbarButton::new(image, tooltip)
                                .id_salt(("data_reference", reference.id, salt))
                                .button_side(BUTTON),
                        )
                        .clicked()
                    };
                    if button(
                        ui,
                        egui::Image::new(crate::ui::unthemed_icon!("delete_scenario.svg")),
                        tr!("data-references-remove"),
                        "remove",
                    ) {
                        commands.push(UiCommand::RemoveDataReference(reference.id));
                    }
                    if button(ui, egui::Image::new(crate::ui::themed_icon!(ui, "reload.svg")), tr!("data-references-reload"), "reload") {
                        commands.push(UiCommand::ReloadDataReference(reference.id));
                    }
                });
            });
            ui.add_space(6.0);
        }
        menu::menu_actions(ui, |ui| {
            if ui.add(menu::MenuButton::new(tr!("data-references-add")).primary()).clicked() {
                commands.push(UiCommand::ChooseDataReference);
            }
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// `text` as it fits in `width`: whole, or its end after an ellipsis.
fn elide_start(ui: &egui::Ui, text: &str, font: &egui::FontId, width: f32) -> String {
    let fits = |candidate: &str| ui.painter().layout_no_wrap(candidate.to_owned(), font.clone(), egui::Color32::PLACEHOLDER).size().x <= width;
    if fits(text) {
        return text.to_owned();
    }
    // The shortest cut from the start that fits: every cut longer fits too.
    let cuts = text.char_indices().map(|(index, _)| index).collect::<Vec<_>>();
    let (mut low, mut high) = (0, cuts.len());
    while low < high {
        let middle = (low + high) / 2;
        if fits(&format!("…{}", &text[cuts[middle]..])) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    cuts.get(low).map_or_else(|| "…".to_owned(), |&cut| format!("…{}", &text[cut..]))
}

fn draw_mcp_settings(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    menu::menu_section(ui, tr!("mcp-title"));
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (editor, commands);
        menu::menu_note(ui, tr!("mcp-desktop-only"));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::ui::state::McpStatus;

        let mcp = &editor.mcp;
        let mut enabled = mcp.enabled;
        if committed(&MenuFieldBool::new(tr!("mcp-enabled"), &mut enabled).show(ui)) {
            commands.push(UiCommand::SetMcpEnabled(enabled));
        }
        let mut port = u32::from(mcp.port);
        let port_field = MenuFieldU32::new(tr!("mcp-port"), &mut port, 1024..=65535).help_text(tr!("mcp-port-help")).show(ui);
        if committed(&port_field)
            && let Ok(port) = u16::try_from(port)
        {
            commands.push(UiCommand::SetMcpPort(port));
        }
        let status = match &mcp.status {
            McpStatus::Off => tr!("mcp-status-off"),
            McpStatus::Running => tr!("mcp-status-running"),
            McpStatus::Failed(error) => tr!("mcp-status-failed", error = error.clone()),
        };
        read_only_row(ui, &tr!("mcp-status"), &status);
        if mcp.status == McpStatus::Running {
            read_only_row(ui, &tr!("mcp-address"), &crate::mcp::url(mcp.port));
        }
        ui.add_space(6.0);
    }
}

fn draw_camera_settings(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    settings_section(
        ui,
        editor,
        commands,
        &tr!("properties-camera"),
        |ui, draft| {
            let mut changed = false;
            CollapsibleSection::new(egui::Id::unique("camera_plan_mode"), tr!("properties-plan-mode"))
                .default_open(true)
                .show(ui, |ui| {
                    changed |= committed(
                        &MenuFieldF64::new(tr!("properties-orbit-sensitivity"), &mut draft.plan_orbit_sensitivity, 0.0001..=0.02)
                            .speed(0.0001)
                            .max_decimals(4)
                            .show(ui),
                    );
                    changed |= committed(
                        &MenuFieldF64::new(tr!("properties-zoom-sensitivity"), &mut draft.plan_zoom_sensitivity, 0.0001..=0.05)
                            .speed(0.0001)
                            .max_decimals(4)
                            .show(ui),
                    );
                    changed |= committed(&MenuFieldBool::new(tr!("properties-invert-vertical"), &mut draft.plan_invert_vertical_look).show(ui));
                    changed |= committed(&MenuFieldBool::new(tr!("properties-invert-horizontal"), &mut draft.plan_invert_horizontal_look).show(ui));
                });

            ui.add_space(4.0);
            CollapsibleSection::new(egui::Id::unique("camera_fly_mode"), tr!("common-fly-mode")).show(ui, |ui| {
                changed |= committed(
                    &MenuFieldF64::new(tr!("properties-field-view"), &mut draft.fly_field_of_view_degrees, 20.0..=120.0)
                        .suffix(tr!("common-degree-suffix"))
                        .show(ui),
                );
                changed |= committed(
                    &MenuFieldF64::new(tr!("properties-look-sensitivity"), &mut draft.fly_mouse_look_sensitivity, 0.0001..=0.02)
                        .speed(0.0001)
                        .max_decimals(4)
                        .show(ui),
                );
                changed |= committed(&MenuFieldBool::new(tr!("properties-invert-vertical"), &mut draft.fly_invert_vertical_look).show(ui));
                changed |= committed(&MenuFieldBool::new(tr!("properties-invert-horizontal"), &mut draft.fly_invert_horizontal_look).show(ui));
                changed |= committed(
                    &MenuFieldF64::new(tr!("properties-near-clip-limit"), &mut draft.fly_near_clip_limit, 0.01..=100.0)
                        .speed(0.01)
                        .suffix(tr!("common-m"))
                        .show(ui),
                );
                changed |= committed(
                    &MenuFieldF64::new(tr!("properties-max-clip-span"), &mut draft.fly_max_clip_span, 100.0..=1_000_000.0)
                        .speed(100.0)
                        .suffix(tr!("common-m"))
                        .show(ui),
                );
            });
            changed
        },
        Some(reset_camera_defaults),
    );
}

fn draw_performance_settings(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    // An adapter with no way to present out of step - a browser surface, say -
    // has nothing to offer here, and the cap it gates would never be applied.
    let vsync_switchable = editor.vsync_switchable;
    settings_section(
        ui,
        editor,
        commands,
        &tr!("properties-performance"),
        |ui, draft| {
            let mut changed = false;
            changed |= committed(
                &MenuFieldU32::new(tr!("properties-snap-polling"), &mut draft.snap_poll_rate, 5..=1000)
                    .suffix(format!(" {}", tr!("properties-hz")))
                    .show(ui),
            );
            if vsync_switchable {
                changed |= committed(
                    &MenuFieldBool::new(tr!("properties-vertical-sync"), &mut draft.vsync_enabled)
                        .help_text(tr!("properties-presents-step-display-no-tearing"))
                        .show(ui),
                );
            }
            if !draft.vsync_enabled {
                changed |= committed(
                    &MenuFieldU32::new(tr!("properties-frame-rate-cap"), &mut draft.frame_rate_cap, 20..=1000)
                        .suffix(format!(" {}", tr!("properties-fps")))
                        .show(ui),
                );
            }
            changed |= committed(
                &MenuFieldU32::new(tr!("properties-cap-while-resizing"), &mut draft.resize_frame_rate_cap, 20..=1000)
                    .suffix(format!(" {}", tr!("properties-fps")))
                    .show(ui),
            );
            changed |= committed(
                &MenuFieldU32::new(tr!("properties-block-model-downscale"), &mut draft.block_model_interaction_resolution_divisor, 1..=64)
                    .suffix(tr!("common-x"))
                    .show(ui),
            );
            changed |= committed(
                &MenuFieldBool::new(tr!("properties-reflective-block-edges"), &mut draft.show_block_model_boundary_highlights)
                    .help_text(tr!("properties-adds-view-dependent-rim-highlight"))
                    .show(ui),
            );
            changed |= committed(
                &MenuFieldBool::new(tr!("properties-downscale-rasters"), &mut draft.downscale_raster_previews)
                    .help_text(tr!("properties-limits-newly-loaded-geotiff-previews"))
                    .show(ui),
            );
            changed
        },
        Some(reset_performance_defaults),
    );
}

fn draw_developer_settings(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    settings_section(
        ui,
        editor,
        commands,
        &tr!("properties-developer"),
        |ui, draft| {
            let mut changed = false;
            changed |= committed(
                &MenuFieldBool::new(tr!("properties-camera-clip-planes"), &mut draft.debug_clip_planes)
                    .help_text(tr!("properties-shows-live-near-far-projection"))
                    .show(ui),
            );
            changed |= committed(
                &MenuFieldBool::new(tr!("properties-surface-chunk-debug-view"), &mut draft.debug_surface_chunks)
                    .help_text(tr!("properties-colours-each-surface-chunk-outlines"))
                    .show(ui),
            );
            changed |= committed(
                &MenuFieldBool::new(tr!("properties-point-cloud-chunk-debug-view"), &mut draft.debug_point_cloud_chunks)
                    .help_text(tr!("properties-colours-each-point-cloud-chunk"))
                    .show(ui),
            );
            changed
        },
        Some(reset_developer_defaults),
    );
}

/// A labelled value the panel only reports, laid out like the editable fields
/// beside it so the columns line up.
pub(crate) fn read_only_row(ui: &mut egui::Ui, label: &str, value: &str) {
    let row_height = ui.spacing().interact_size.y;
    let row_width = ui.available_width();
    // The same column width the fields resolve for themselves, or the values
    // would not line up under them.
    let value_width = crate::ui::widgets::menu::field_column_for(ui, label, false);
    ui.allocate_ui_with_layout(egui::vec2(row_width, row_height), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.allocate_ui_with_layout(egui::vec2(value_width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).color(ui.visuals().weak_text_color()));
        });
        let label_width = ui.available_width();
        ui.allocate_ui_with_layout(egui::vec2(label_width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            menu_field_label(ui, label.into(), None);
        });
    });
}

pub(crate) fn fill_style_label(style: FillStyle) -> String {
    match style {
        FillStyle::Clear => tr!("common-clear"),
        FillStyle::Crosses => tr!("common-crosses"),
        FillStyle::Slashes => tr!("common-slashes"),
        FillStyle::Solid => tr!("tri-type-solid-closed"),
    }
}

fn draw_design_tab(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, context: &PropertyContext, commands: &mut Vec<UiCommand>, geometry_dirty: &mut bool) {
    // Fields describe the first selection and write to all of it, matching how
    // the batch commands behave.
    let first_color = context
        .objects
        .first()
        .and_then(|&id| document.get_object(id))
        .map(|object| document.object_rgba(object))
        .unwrap_or([0.0; 4]);
    let mut color32 = rgba_to_color32(first_color);
    let colour_label = match context.objects.first().and_then(|&id| document.get_object(id)) {
        Some(crate::model::Object::Text { .. }) => tr!("context-text-colour"),
        _ => tr!("properties-line-colour"),
    };
    let response = MenuFieldColor32::new(colour_label, &mut color32).show(ui);
    if committed(&response) {
        commands.push(UiCommand::BatchSetObjectColor(context.objects.clone(), ObjectColor::Fixed(color32_to_rgba(color32))));
        *geometry_dirty = true;
    }

    if context.polylines.is_empty() {
        return;
    }

    // Circles share this group - they carry fill and line weight - so read
    // those through the accessors rather than naming the polyline variant.
    let first_object = context.polylines.first().and_then(|&id| document.get_object(id));
    let first_closed = first_object.and_then(crate::model::Object::string_geometry).is_some_and(|(_, closed)| closed);
    let first_fill = first_object.and_then(crate::model::Object::fill).unwrap_or(FillStyle::Clear);
    let first_line_weight = first_object.and_then(crate::model::Object::line_weight).unwrap_or(1.0);

    // A circle is closed by definition, so the Shape row appears only when the
    // selection holds something that can actually be opened.
    let any_openable = context
        .polylines
        .iter()
        .any(|&id| matches!(document.get_object(id), Some(crate::model::Object::Polyline { .. })));
    if any_openable {
        let closed_label = tr!("common-closed");
        let open_label = tr!("common-open");
        let mut closed = first_closed;
        MenuFieldCombo::new(
            "design_shape",
            tr!("common-shape"),
            &mut closed,
            if first_closed { closed_label.clone() } else { open_label.clone() },
            [(true, closed_label.into()), (false, open_label.into())],
        )
        .width(FIELD_WIDTH)
        .show(ui);
        if closed != first_closed {
            commands.push(UiCommand::BatchSetPolylineClosed(context.polylines.clone(), closed));
            *geometry_dirty = true;
        }
    }

    // A pit design is no area to fill: the row is for the rest.
    let fillable = context
        .polylines
        .iter()
        .any(|id| !matches!(document.get_object(*id), Some(crate::model::Object::PitDesign { .. })));
    let mut fill = first_fill;
    if fillable {
        MenuFieldCombo::new(
            "design_fill",
            tr!("common-fill"),
            &mut fill,
            fill_style_label(first_fill),
            [FillStyle::Clear, FillStyle::Crosses, FillStyle::Slashes, FillStyle::Solid].map(|style| (style, fill_style_label(style).into())),
        )
        .width(FIELD_WIDTH)
        .show(ui);
    }
    if fill != first_fill {
        commands.push(UiCommand::BatchSetObjectFill(context.polylines.clone(), fill));
        *geometry_dirty = true;
    }

    // Seed the in-progress value from the document whenever the selection
    // changes; between those points the drag owns it.
    if editor.design_line_weight_input.as_ref().is_none_or(|(object_ids, _)| object_ids != &context.polylines) {
        editor.design_line_weight_input = Some((context.polylines.clone(), first_line_weight));
    }
    if let Some((_, line_weight)) = editor.design_line_weight_input.as_mut() {
        let response = MenuFieldF32::new(tr!("common-line-weight"), line_weight, 0.1..=20.0).width(FIELD_WIDTH).speed(0.1).show(ui);
        let line_weight = *line_weight;
        if committed(&response) {
            commands.push(UiCommand::BatchSetPolylineLineWeight(context.polylines.clone(), line_weight));
            *geometry_dirty = true;
        }
    }
}
