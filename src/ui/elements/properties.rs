//! Application preferences and selection appearance controls.

use glam::DVec3;

use crate::{
    app::commands::prismoid_grid::ramp_bar_stops,
    i18n::tr,
    model::{
        Document, FillStyle, ObjectColor, ObjectId, SceneEntityId,
        block_model::{BlockModelId, OpenBlockModel},
        drill_hole::DrillColorPreset,
        prismoid_grid::{PrismoidColorBy, PrismoidGridId, ValueSpread},
        project::{LATTICE_UNIT, ModelEnvelope, ModellingSettings},
    },
    rendering::color::{color32_to_rgba, rgba_to_color32},
    ui::{
        UiCommand, UiProjectView,
        elements::block_model::vector_fields,
        state::{EditorState, PreferencesDraft, PropertyTab, Workspace},
        widgets::{
            collapsible_section::CollapsibleSection,
            context_menu::{FIELD_WIDTH, context_menu_fields},
            menu::{self, MenuButton, MenuFieldBool, MenuFieldColor32, MenuFieldCombo, MenuFieldF32, MenuFieldF64, MenuFieldU32, committed, menu_field_label},
            toolbar::GROUP_CORNER_RADIUS,
            viewport::{BlockModelProperties, ViewportDockPanel, color_ramp_bar},
        },
    },
};

pub(crate) fn draw_preferences(ui: &mut egui::Ui, editor: &mut EditorState, modelling: Option<(u32, &ModellingSettings)>, commands: &mut Vec<UiCommand>) {
    if !editor.show_preferences {
        return;
    }
    let mut open = true;
    let available = ui.ctx().content_rect().size() - egui::vec2(24.0, 24.0);
    let scale = (available.x / 600.0).min(available.y / 480.0).clamp(0.1, 1.0);
    let size = egui::vec2(600.0, 480.0) * scale;
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
                    (PropertyTab::Developer, tr!("properties-developer")),
                    (PropertyTab::Modelling, tr!("properties-modelling")),
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
                        PropertyTab::Developer => draw_developer_settings(ui, editor, commands),
                        PropertyTab::Modelling => draw_modelling_settings(ui, modelling, commands),
                        // Not one of this dialog's own tabs: set from the planning
                        // viewport's Reserves panel, which this dialog never shows.
                        PropertyTab::Reserves => {}
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
                Some(crate::model::Object::Polyline { .. } | crate::model::Object::Circle { .. }) => 0,
                Some(crate::model::Object::Point { .. }) => 1,
                Some(crate::model::Object::Text { .. }) => 2,
                None => continue,
            },
            SceneEntityId::Triangulation(_) => 3,
            SceneEntityId::BlockModel(_) => 4,
            SceneEntityId::DrillHole(_) => 5,
            SceneEntityId::PointCloud(_) => 6,
            SceneEntityId::Raster(_) => 7,
            SceneEntityId::HaulRoad(_) | SceneEntityId::HaulNode(_) | SceneEntityId::PrismoidGrid(_) => continue,
        };
        selected_types[kind] = true;
    }
    let show_headings = selected_types.into_iter().filter(|selected| *selected).count() > 1;
    for (kind, heading) in [(0, tr!("context-polylines")), (1, tr!("context-points")), (2, tr!("common-text"))] {
        let objects: Vec<_> = objects
            .iter()
            .copied()
            .filter(|id| match document.get_object(*id) {
                Some(crate::model::Object::Polyline { .. } | crate::model::Object::Circle { .. }) => kind == 0,
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
        if crate::ui::widgets::context_menu::ContextMenuAction::new(tr!("properties-move-layer")).show(ui).clicked() {
            let target_layer = project
                .projects
                .iter()
                .find(|entry| entry.is_active)
                .and_then(|entry| entry.layers.first())
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

/// Width of the prismoid grid's key in the viewport.
const PRISMOID_LEGEND_WIDTH: f32 = 240.0;
const PRISMOID_LEGEND_BAR_HEIGHT: f32 = 14.0;

/// What a prismoid grid's key says.
#[derive(Debug, PartialEq)]
struct PrismoidKeyText {
    title: String,
    bar: PrismoidKeyBar,
    /// How many cells have no value, beside the body's own colour.
    no_value: Option<String>,
}

#[derive(Debug, PartialEq)]
enum PrismoidKeyBar {
    /// The range's two ends.
    Ends(String, String),
    /// In the bar's place when no cell has a value.
    Empty(String),
}

/// The key's words for the value a grid is coloured by, or for its vertical
/// thickness with `None`.
fn prismoid_key_text(spread: Option<&ValueSpread>, thickness: (f64, f64)) -> PrismoidKeyText {
    let ends = |(min, max): (f64, f64)| PrismoidKeyBar::Ends(format!("{min:.2}"), format!("{max:.2}"));
    let Some(spread) = spread else {
        return PrismoidKeyText {
            title: tr!("thickness-points-column-vertical"),
            bar: ends(thickness),
            no_value: None,
        };
    };
    let title = if spread.unit.is_empty() {
        spread.name.clone()
    } else {
        tr!("viewport-prismoid-value-unit", value = spread.name.clone(), unit = spread.unit.clone())
    };
    let (bar, no_value) = match spread.range {
        Some(range) => (
            ends(range),
            (spread.missing > 0).then(|| tr!("viewport-prismoid-no-value-cells", count = spread.missing.to_string())),
        ),
        None => (PrismoidKeyBar::Empty(tr!("explorer-prismoid-grid-quality-no-values", quality = spread.name.clone())), None),
    };
    PrismoidKeyText { title, bar, no_value }
}

/// The Colour-by combo's value and options: vertical thickness, the grid's
/// values, and a chosen name the grid does not hold, so the combo says what
/// the item keeps while thickness is drawn.
fn prismoid_color_choices(color_by: &PrismoidColorBy, spreads: &[ValueSpread], index: Option<usize>) -> (Option<String>, Vec<(Option<String>, String)>) {
    let mut options: Vec<_> = std::iter::once((None, tr!("thickness-points-column-vertical")))
        .chain(spreads.iter().map(|spread| (Some(spread.name.clone()), spread.name.clone())))
        .collect();
    let value = match index {
        Some(index) => Some(spreads[index].name.clone()),
        None => color_by.value.clone(),
    };
    if let (None, Some(name)) = (index, &value) {
        options.push((value.clone(), tr!("viewport-prismoid-value-not-held", value = name.clone())));
    }
    (value, options)
}

/// The key's Colour-by and Colour-scale rows, both as wide as the label
/// column leaves, so a long value name neither hides a label nor widens
/// its row.
fn draw_prismoid_color_fields(ui: &mut egui::Ui, id: PrismoidGridId, color_by: &PrismoidColorBy, spreads: &[ValueSpread], index: Option<usize>, commands: &mut Vec<UiCommand>) {
    let label = tr!("viewport-prismoid-colour-by");
    let width = menu::field_column_for(ui, &label, false);
    let (mut value, options) = prismoid_color_choices(color_by, spreads, index);
    if fitted_combo(ui, ("prismoid_color_value", id), label, &mut value, options, width) {
        commands.push(UiCommand::SetPrismoidGridColorValue { id, value });
    }
    let mut preset = color_by.preset;
    let presets = DrillColorPreset::ALL.map(|preset| (preset, preset.label())).into();
    if fitted_combo(ui, ("prismoid_color_preset", id), tr!("drill-hole-colour-scale"), &mut preset, presets, width) {
        commands.push(UiCommand::SetPrismoidGridColorPreset { id, preset });
    }
}

/// A combo `width` wide with its texts cut to fit, the chosen one named in
/// full on hover when cut; true when the choice changed.
fn fitted_combo<T: PartialEq>(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, label: String, value: &mut T, options: Vec<(T, String)>, width: f32) -> bool {
    let full = options.iter().find(|(option, _)| option == value).map_or_else(String::new, |(_, text)| text.clone());
    let selected = menu::elide_for_combo(ui, &full, width);
    let options: Vec<_> = options.into_iter().map(|(option, text)| (option, menu::elide_for_combo(ui, &text, width).into())).collect();
    let response = MenuFieldCombo::new(id, label, value, selected.clone(), options).width(width).show(ui);
    if selected == full { response } else { response.on_hover_text(full) }.changed()
}

/// The key to one selected prismoid grid's colours, with what they follow
/// and their scale: the ramp, with the grid's own range at its ends.
pub(crate) fn draw_prismoid_grid_legend(
    ui: &egui::Ui,
    editor: &EditorState,
    project: &UiProjectView,
    models: &[OpenBlockModel],
    commands: &mut Vec<UiCommand>,
    canvas: egui::Rect,
) {
    if editor.active_workspace != Workspace::Geology {
        return;
    }
    let mut shown = project.prismoid_grids.iter().filter(|grid| {
        let entity = SceneEntityId::PrismoidGrid(grid.id);
        grid.is_loaded && editor.selected_handles.contains(&entity) && !editor.hidden_handles.contains(&entity)
    });
    let (Some(grid), None) = (shown.next(), shown.next()) else {
        return;
    };
    let Some(thickness) = grid.summary.vertical_thickness.filter(|_| grid.regular) else {
        return;
    };
    // Along the bottom edge clear of the tool panels' corner, as the
    // block-model card is, and above that card while it is up. Not drawn once
    // it would reach the scale bar. A dock panel hashes the id it is handed.
    let card_id = egui::Id::unique(egui::Id::unique("block_model_viewport_controls"));
    let (card, scale_bar) = ui
        .ctx()
        .memory(|memory| (memory.area_rect(card_id), memory.area_rect(egui::Id::unique("viewport_scale_bar"))));
    let left = canvas.left() + crate::ui::widgets::viewport::DOCK_PANEL_RESERVED_WIDTH + BLOCK_MODEL_CARD_GAP;
    if left + PRISMOID_LEGEND_WIDTH + 20.0 > scale_bar.map_or(canvas.right(), |rect| rect.left()) - BLOCK_MODEL_CARD_GAP {
        return;
    }
    let card_up = models.iter().any(|model| Some(model.id) == editor.viewport_block_model_id && model.state.loaded);
    let bottom = card.filter(|_| card_up).map_or(canvas.bottom(), |card| card.top());
    let spreads = &grid.summary.spreads;
    let index = grid.color_by.index(spreads.iter().map(|spread| spread.name.as_str()));
    let text = prismoid_key_text(index.map(|index| &spreads[index]), thickness);
    let stops: Vec<_> = ramp_bar_stops(grid.color_by.preset)
        .into_iter()
        .map(|(t, [red, green, blue])| (t, rgba_to_color32([red, green, blue, 1.0])))
        .collect();
    ViewportDockPanel::new("prismoid_grid_legend", text.title, canvas)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::pos2(left, bottom - BLOCK_MODEL_CARD_GAP))
        .order(egui::Order::Middle)
        .min_width(PRISMOID_LEGEND_WIDTH + 20.0)
        .max_width(PRISMOID_LEGEND_WIDTH + 20.0)
        .show(ui.ctx(), |ui| {
            ui.set_width(PRISMOID_LEGEND_WIDTH);
            draw_prismoid_color_fields(ui, grid.id, &grid.color_by, spreads, index, commands);
            let weak = ui.visuals().weak_text_color();
            let font = egui::TextStyle::Small.resolve(ui.style());
            let (row, _) = ui.allocate_exact_size(egui::vec2(PRISMOID_LEGEND_WIDTH, PRISMOID_LEGEND_BAR_HEIGHT + 8.0), egui::Sense::hover());
            match text.bar {
                PrismoidKeyBar::Ends(low, high) => {
                    let low = ui.painter().layout_no_wrap(low, font.clone(), weak);
                    let high = ui.painter().layout_no_wrap(high, font, weak);
                    let gap = 6.0;
                    let bar = egui::Rect::from_x_y_ranges(
                        row.left() + low.size().x + gap..=row.right() - high.size().x - gap,
                        row.center().y - PRISMOID_LEGEND_BAR_HEIGHT / 2.0..=row.center().y + PRISMOID_LEGEND_BAR_HEIGHT / 2.0,
                    );
                    color_ramp_bar(ui.painter(), bar, &stops);
                    let (low_at, high_at) = (row.center().y - low.size().y / 2.0, row.right() - high.size().x);
                    ui.painter().galley(egui::pos2(row.left(), low_at), low, weak);
                    ui.painter().galley(egui::pos2(high_at, low_at), high, weak);
                }
                PrismoidKeyBar::Empty(line) => {
                    ui.painter().text(row.center(), egui::Align2::CENTER_CENTER, line, font, weak);
                }
            }
            if let Some(line) = text.no_value {
                ui.horizontal(|ui| {
                    let (swatch, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(swatch, GROUP_CORNER_RADIUS, rgba_to_color32(grid.color));
                    ui.label(egui::RichText::new(line).small().color(weak));
                });
            }
        });
}

/// Width of the block-model card in the viewport.
const BLOCK_MODEL_CARD_WIDTH: f32 = 780.0;
/// Gap between that card and the viewport's bottom edge, the tool panels'
/// corner and the scale bar.
const BLOCK_MODEL_CARD_GAP: f32 = 8.0;

pub(crate) fn draw_block_model_controls(ui: &mut egui::Ui, editor: &mut EditorState, models: &[OpenBlockModel], commands: &mut Vec<UiCommand>, canvas: egui::Rect) {
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
        return;
    };
    // Centred under the window, but never into the bottom-left corner the
    // tool panels open in: a narrowing viewport pushes the card right from
    // there, and once it would reach the scale bar it is not drawn at all.
    let area_id = egui::Id::unique("block_model_viewport_controls");
    let (card_size, scale_bar) = ui.ctx().memory(|memory| {
        (
            memory.area_rect(area_id).map_or(egui::vec2(BLOCK_MODEL_CARD_WIDTH, 0.0), |rect| rect.size()),
            memory.area_rect(egui::Id::unique("viewport_scale_bar")),
        )
    });
    let card_width = card_size.x;
    let left_bound = canvas.left() + crate::ui::widgets::viewport::DOCK_PANEL_RESERVED_WIDTH + BLOCK_MODEL_CARD_GAP;
    let right_bound = scale_bar.map_or(canvas.right(), |rect| rect.left()) - BLOCK_MODEL_CARD_GAP;
    let left = (ui.ctx().content_rect().center().x - card_width / 2.0).max(left_bound);
    // Always its full height, never scrolled or squeezed: a viewport too
    // short to hold it hides it, the same as one too narrow.
    if left + card_width > right_bound || card_size.y > canvas.height() - 2.0 * BLOCK_MODEL_CARD_GAP {
        return;
    }
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
                    .on_hover_text(model.name.clone());
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
                ui.add_enabled(draft != restored, egui::Button::new(tr!("properties-restore-defaults")))
                    .on_hover_text(tr!("properties-restore-defaults-tooltip", heading = heading))
                    .clicked()
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

/// The open project's settings for Build Surface. A number being dragged is
/// held in the window's memory and sent once, when the drag ends.
fn draw_modelling_settings(ui: &mut egui::Ui, modelling: Option<(u32, &ModellingSettings)>, commands: &mut Vec<UiCommand>) {
    menu::menu_section(ui, tr!("properties-modelling"));
    let Some((session, &stored)) = modelling else {
        ui.label(egui::RichText::new(tr!("common-no-open-project")).weak());
        return;
    };
    let draft_id = ui.scope_id().with("modelling_settings_draft");
    let mut settings = ui.data(|data| data.get_temp::<ModellingSettings>(draft_id)).unwrap_or(stored);
    let responses = [
        // Every range here is exactly what `ModellingSettings::problem` accepts:
        // a narrower one would clamp a loaded value just by being drawn.
        MenuFieldF64::new(tr!("properties-steep-pair-distance"), &mut settings.steep_distance, f64::MIN_POSITIVE..=f64::MAX)
            .suffix(" m")
            .max_decimals(2)
            .help_text(tr!("properties-steep-pair-distance-help"))
            .show(ui),
        MenuFieldF64::new(tr!("properties-steep-pair-angle"), &mut settings.steep_degrees, f64::MIN_POSITIVE..=90.0)
            .suffix("°")
            .max_decimals(1)
            .show(ui),
    ];
    menu::menu_note(ui, tr!("properties-modelling-help"));
    if responses.iter().any(|response| response.dragged()) {
        ui.data_mut(|data| data.insert_temp(draft_id, settings));
    } else {
        ui.data_mut(|data| data.remove::<ModellingSettings>(draft_id));
    }
    if responses.iter().any(committed) && settings != stored && settings.problem().is_none() {
        commands.push(UiCommand::SetModellingSettings(settings));
    }
    draw_model_envelope(ui, session, stored, commands);
}

/// The model envelope rows: typed as two corners, sent only on Apply, the
/// plan edges shown snapped outward before they are.
fn draw_model_envelope(ui: &mut egui::Ui, session: u32, stored: ModellingSettings, commands: &mut Vec<UiCommand>) {
    menu::menu_section(ui, tr!("properties-envelope"));
    // Salted with the project's runtime id and its stored envelope, so no
    // other project is offered this one's draft.
    let scope = ui.scope_id().with("model_envelope_draft");
    let draft_id = |envelope: Option<ModelEnvelope>| scope.with((session, envelope.map(|envelope| envelope.bits())));
    let id = draft_id(stored.envelope);
    let seeded = stored.envelope.map(|envelope| (envelope.lower(), envelope.upper()));
    let Some((mut lower, mut upper)) = ui.data(|data| data.get_temp::<(DVec3, DVec3)>(id)).or(seeded) else {
        menu::menu_note(ui, tr!("properties-envelope-none"));
        if menu::menu_actions(ui, |ui| ui.add(MenuButton::new(tr!("properties-envelope-add"))).clicked()) {
            ui.data_mut(|data| data.insert_temp(id, (DVec3::ZERO, DVec3::splat(100.0))));
        }
        return;
    };
    vector_fields(ui, &tr!("block-model-minimum"), &tr!("properties-envelope-minimum-help"), &mut lower, f64::MIN..=f64::MAX);
    vector_fields(ui, &tr!("block-model-maximum"), &tr!("properties-envelope-maximum-help"), &mut upper, f64::MIN..=f64::MAX);
    if Some((lower, upper)) == seeded {
        ui.data_mut(|data| data.remove::<(DVec3, DVec3)>(id));
    } else {
        ui.data_mut(|data| data.insert_temp(id, (lower, upper)));
    }
    let typed = if !(lower.is_finite() && upper.is_finite() && upper.x > lower.x && upper.y > lower.y) {
        menu::menu_note(ui, tr!("properties-envelope-plan-empty"));
        None
    } else if upper.z <= lower.z {
        menu::menu_note(ui, tr!("properties-envelope-top-below"));
        None
    } else {
        Some(ModelEnvelope::from_box(lower, upper))
    };
    if let Some(envelope) = typed
        && (envelope.lower().truncate(), envelope.upper().truncate()) != (lower.truncate(), upper.truncate())
    {
        let (west_south, east_north) = (envelope.lower(), envelope.upper());
        menu::menu_note(
            ui,
            tr!(
                "properties-envelope-steer",
                west = west_south.x.to_string(),
                east = east_north.x.to_string(),
                south = west_south.y.to_string(),
                north = east_north.y.to_string(),
                unit = LATTICE_UNIT.to_string()
            ),
        );
    }
    let can_apply = typed.is_some_and(|envelope| stored.envelope != Some(envelope));
    let (apply, remove) = menu::menu_actions(ui, |ui| {
        let apply = ui.add(MenuButton::new(tr!("common-apply")).primary().enabled(can_apply)).clicked();
        let remove = stored.envelope.is_some() && ui.add(MenuButton::new(tr!("properties-envelope-remove"))).clicked();
        (apply, remove)
    });
    if !(apply || remove) {
        return;
    }
    ui.data_mut(|data| {
        data.remove::<(DVec3, DVec3)>(id);
        data.remove::<(DVec3, DVec3)>(draft_id(None));
    });
    let envelope = if apply { typed } else { None };
    commands.push(UiCommand::SetModellingSettings(ModellingSettings { envelope, ..stored }));
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

    let mut fill = first_fill;
    MenuFieldCombo::new(
        "design_fill",
        tr!("common-fill"),
        &mut fill,
        fill_style_label(first_fill),
        [FillStyle::Clear, FillStyle::Crosses, FillStyle::Slashes, FillStyle::Solid].map(|style| (style, fill_style_label(style).into())),
    )
    .width(FIELD_WIDTH)
    .show(ui);
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
