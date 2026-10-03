//! Import & Export menus.

use crate::{
    i18n::tr,
    model::{
        LayerId,
        block_model::BlockModelId,
        drill_hole::DrillHoleId,
        formats::{
            MeshFormat,
            csv_block_model::{CsvColumnRole, FixedDimension, validate_mapping},
            csv_drill_hole::{CsvDrillColumnRole, CsvDrillFileMapping, CsvDrillFileRole, CsvDrillPreview, bundle_anchor},
        },
        triangulation::TriangulationId,
    },
    ui::{
        fonts::bold,
        state::{DataMenu, EditorState, OmfExportSection, UiCommand, UiProjectView},
        widgets::{
            explorer::{ExplorerEntry, ExplorerHeader, explorer_note, row_height, stripe_bands},
            menu::{self, DragableMenu, MenuButton, MenuFieldBool, MenuFieldCombo, MenuFieldF64, MenuFieldFilePicker},
            toolbar::GROUP_CORNER_RADIUS,
            tree_row_colors,
        },
    },
};

const MENU_HEIGHT: f32 = 450.0;
/// Wide enough for the longest format name at an entry's indent, so the tree
/// truncates nothing at the default text size.
const EXPLORER_WIDTH: f32 = 285.0;
const FIELD_WIDTH: f32 = 280.0;
const DETAILS_WIDTH: f32 = 680.0;
const MENU_WIDTH: f32 = EXPLORER_WIDTH + 4.0 + DETAILS_WIDTH;

/// One format, as a row of the dialog's type tree.
fn draw_entry(ui: &mut egui::Ui, editor: &mut EditorState, title: &str, data_menu: DataMenu) -> egui::Response {
    let response = ExplorerEntry::new(ui.id().with(("data_menu", data_menu)), bold(title))
        .selected(editor.data_menu == data_menu)
        .show(ui)
        .response;
    if response.clicked() {
        editor.data_menu = data_menu;
    }
    response
}

/// The type tree down the left of both dialogs.
///
/// Framed and banded like the data explorer's panel, so the two lists read as
/// the same object, and named and ordered as the explorer names and orders its
/// sections.
fn draw_type_explorer(ui: &mut egui::Ui, id_salt: &str, contents: impl FnOnce(&mut egui::Ui)) {
    striped_box(ui, id_salt, egui::vec2(EXPLORER_WIDTH, MENU_HEIGHT), contents);
}

/// A framed, banded list box: the data explorer's tree, boxed for a dialog.
fn striped_box(ui: &mut egui::Ui, id_salt: &str, size: egui::Vec2, contents: impl FnOnce(&mut egui::Ui)) {
    const INSET: i8 = 3;

    let (surface, stripe) = tree_row_colors(ui);
    let frame = egui::Frame::new()
        .fill(surface)
        .stroke(ui.visuals().window_stroke())
        .corner_radius(egui::CornerRadius::same(GROUP_CORNER_RADIUS))
        .inner_margin(egui::Margin::same(INSET));

    frame.show(ui, |ui| {
        let inset = f32::from(INSET);
        // A dialog lays its halves out side by side, so the box has to ask for
        // a column of its own: rows stacked in the horizontal Ui the frame
        // inherits would run across the dialog in one line.
        ui.allocate_ui_with_layout(size - egui::Vec2::splat(inset * 2.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            {
                ui.set_width(size.x - inset * 2.0);
                ui.set_height(size.y - inset * 2.0);
                let row = row_height(ui);
                // The banding belongs to the box rather than to the rows, so a
                // short list still reads as a striped tree: reserved out here,
                // where the box's own rect and scroll offset can be measured
                // once the rows are laid out.
                let stripes_slot = ui.painter().add(egui::Shape::Noop);
                let scroll = egui::ScrollArea::vertical()
                    .id_salt(id_salt)
                    .auto_shrink([false; 2])
                    .min_scrolled_height(0.0)
                    .show(ui, |ui| {
                        // Long format names end in an ellipsis rather than wrapping a
                        // row out of the banding, and rows butt together so the bands
                        // tile.
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        // A dialog's widgets are laid out taller than a tree row, which
                        // would push each row past the band behind it and leave the
                        // list a little further out of step with every row drawn.
                        ui.spacing_mut().interact_size.y = row;
                        ui.spacing_mut().button_padding.y = 0.0;
                        contents(ui);
                    });
                let box_rect = scroll.inner_rect;
                let bands = stripe_bands(box_rect.x_range(), box_rect.top() - scroll.state.offset.y, box_rect.bottom(), row, stripe);
                ui.painter().with_clip_rect(box_rect).set(stripes_slot, bands);
            }
        });
    });
}

pub(crate) fn draw_import_menu(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.show_import {
        return;
    }
    if !is_import_menu(editor.data_menu) {
        editor.data_menu = DataMenu::Omf;
    }

    let mut show_import = editor.show_import;
    let mut close_after_action = false;
    let mut cancelled = false;
    DragableMenu::new("import_dialog", tr!("io-import")).open(&mut show_import).show(ui.ctx(), |ui| {
        ui.set_height(MENU_HEIGHT);
        ui.set_width(MENU_WIDTH);

        ui.horizontal(|ui| {
            draw_import_explorer(ui, editor);
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.allocate_ui(egui::Vec2::new(DETAILS_WIDTH, MENU_HEIGHT - 25.0), |ui| {
                    ui.set_width(DETAILS_WIDTH);
                    ui.set_height(MENU_HEIGHT - 25.);
                    draw_import_details(ui, editor, commands);
                });
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(DETAILS_WIDTH, ui.spacing().interact_size.y),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        let command = import_command(editor);
                        let confirm = menu::dialog_confirm_pressed(ui.ctx());
                        cancelled = menu::dialog_cancel_pressed(ui.ctx());
                        if (ui.add(MenuButton::new(tr!("io-import")).primary().enabled(command.is_some())).clicked() || confirm)
                            && let Some(command) = command
                        {
                            commands.push(command);
                            close_after_action = true;
                        }
                        if ui.add(MenuButton::new(tr!("io-default"))).clicked() {
                            #[cfg(target_arch = "wasm32")]
                            let import_kind = editor.data_menu;
                            reset_import_defaults(editor, project);
                            #[cfg(target_arch = "wasm32")]
                            commands.push(UiCommand::ClearBrowserImportSelection(import_kind));
                        }
                    },
                );
            });
        });
    });
    if close_after_action || cancelled {
        show_import = false;
    }
    editor.show_import = show_import;
}

pub(crate) fn draw_export_menu(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.show_export {
        return;
    }
    if !is_export_menu(editor.data_menu) {
        editor.data_menu = DataMenu::Omf;
    }

    let mut show_export = editor.show_export;
    let mut close_after_action = false;
    let mut cancelled = false;
    DragableMenu::new("export_dialog", tr!("io-export")).open(&mut show_export).show(ui.ctx(), |ui| {
        ui.set_height(MENU_HEIGHT);
        ui.set_width(MENU_WIDTH);

        ui.horizontal(|ui| {
            draw_export_explorer(ui, editor);
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.allocate_ui(egui::Vec2::new(DETAILS_WIDTH, MENU_HEIGHT - 25.0), |ui| {
                    ui.set_width(DETAILS_WIDTH);
                    ui.set_height(MENU_HEIGHT - 25.);
                    draw_export_details(ui, editor, project);
                });
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(DETAILS_WIDTH, ui.spacing().interact_size.y),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        let command = export_command(editor);
                        let confirm = menu::dialog_confirm_pressed(ui.ctx());
                        cancelled = menu::dialog_cancel_pressed(ui.ctx());
                        if (ui.add(MenuButton::new(tr!("io-export")).primary().enabled(command.is_some())).clicked() || confirm)
                            && let Some(command) = command
                        {
                            commands.push(command);
                            close_after_action = true;
                        }
                        if ui.add(MenuButton::new(tr!("io-default"))).clicked() {
                            reset_export_defaults(editor, project);
                        }
                    },
                );
            });
        });
    });
    if close_after_action || cancelled {
        show_export = false;
    }
    editor.show_export = show_export;
}

fn draw_import_explorer(ui: &mut egui::Ui, editor: &mut EditorState) {
    draw_type_explorer(ui, "import_type_tree", |ui| {
        // OMF carries a whole project rather than one kind of data, so it gets
        // a section of its own above the rest - open, because it is also the
        // dialog's default selection.
        ExplorerHeader::new(egui::Id::new("import_projects_section"), tr!("io-projects"))
            .default_open(true)
            .show(ui, |ui| {
                draw_entry(ui, editor, &tr!("io-open-mining-format-2-omf"), DataMenu::Omf);
            });
        // The sections below are the data explorer's, in its order: whatever
        // comes in here lands in the section of the same name over there. They
        // carry neither the explorer's icons nor its tints - here the heading
        // names a group of file formats, not the data itself.
        ExplorerHeader::new(egui::Id::new("import_designs_section"), tr!("common-designs"))
            .default_open(false)
            .show(ui, |ui| {
                draw_entry(ui, editor, &tr!("io-drawing-exchange-format-dxf"), DataMenu::Dxf);
            });
        ExplorerHeader::new(egui::Id::new("import_triangulations_section"), tr!("common-triangulations"))
            .default_open(false)
            .show(ui, |ui| {
                draw_entry(ui, editor, &tr!("io-wavefront-obj"), DataMenu::Obj);
                draw_entry(ui, editor, &tr!("io-stl"), DataMenu::Stl);
                draw_entry(ui, editor, &tr!("io-ply"), DataMenu::Ply);
            });
        ExplorerHeader::new(egui::Id::new("import_rasters_section"), tr!("common-rasters"))
            .default_open(false)
            .show(ui, |ui| {
                draw_entry(ui, editor, &tr!("io-geotiff-tif-tiff"), DataMenu::Geotiff);
            });
        ExplorerHeader::new(egui::Id::new("import_point_clouds_section"), tr!("common-point-clouds"))
            .default_open(false)
            .show(ui, |ui| {
                draw_entry(ui, editor, &tr!("io-las-laz-las-laz"), DataMenu::Las);
                draw_entry(ui, editor, &tr!("io-ascii-points-xyz-pts"), DataMenu::Xyz);
                draw_entry(ui, editor, &tr!("io-point-cloud-data-pcd"), DataMenu::Pcd);
            });
        ExplorerHeader::new(egui::Id::new("import_block_models_section"), tr!("common-block-models"))
            .default_open(false)
            .show(ui, |ui| {
                draw_entry(ui, editor, &tr!("io-comma-separated-values-csv"), DataMenu::CsvBlockModel);
            });
        ExplorerHeader::new(egui::Id::new("import_drill_holes_section"), tr!("ws-menubar-drillholes"))
            .default_open(false)
            .show(ui, |ui| {
                draw_entry(ui, editor, &tr!("io-mapped-csv-bundle-csv"), DataMenu::CsvDrillHole);
            });
    });
}

fn draw_export_explorer(ui: &mut egui::Ui, editor: &mut EditorState) {
    draw_type_explorer(ui, "export_type_tree", |ui| {
        ExplorerHeader::new(egui::Id::new("export_projects_section"), tr!("io-projects")).show(ui, |ui| {
            draw_entry(ui, editor, &tr!("io-open-mining-format-2-omf"), DataMenu::Omf);
        });
        ExplorerHeader::new(egui::Id::new("export_designs_section"), tr!("common-designs")).show(ui, |ui| {
            draw_entry(ui, editor, &tr!("io-drawing-exchange-format-dxf"), DataMenu::Dxf);
        });
        ExplorerHeader::new(egui::Id::new("export_triangulations_section"), tr!("common-triangulations")).show(ui, |ui| {
            draw_entry(ui, editor, &tr!("io-wavefront-obj"), DataMenu::Obj);
            draw_entry(ui, editor, &tr!("io-stl"), DataMenu::Stl);
            draw_entry(ui, editor, &tr!("io-ply"), DataMenu::Ply);
        });
        ExplorerHeader::new(egui::Id::new("export_block_models_section"), tr!("common-block-models")).show(ui, |ui| {
            draw_entry(ui, editor, &tr!("io-comma-separated-values-csv"), DataMenu::CsvBlockModel);
        });
        ExplorerHeader::new(egui::Id::new("export_drill_holes_section"), tr!("ws-menubar-drillholes")).show(ui, |ui| {
            draw_entry(ui, editor, &tr!("io-mapped-csv-bundle-csv"), DataMenu::CsvDrillHole);
        });
    });
}

fn draw_import_details(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    // Scope ids by page so widgets on different pages that happen to share a
    // rect don't trip egui's id-stability check when switching pages.
    ui.push_id(editor.data_menu, |ui| match editor.data_menu {
        DataMenu::Omf => {
            ui.heading(tr!("io-import-open-mining-format-2"));
            draw_import_source_picker(ui, editor, commands, tr!("common-project"), tr!("io-no-omf-chosen"));
        }
        DataMenu::Dxf => draw_import_dxf(ui, editor, commands),
        DataMenu::Obj => draw_import_mesh(ui, editor, commands, &tr!("io-import-wavefront-obj")),
        DataMenu::Stl => draw_import_mesh(ui, editor, commands, &tr!("io-import-stl")),
        DataMenu::Ply => draw_import_mesh(ui, editor, commands, &tr!("io-import-ply")),
        DataMenu::Las => draw_import_mesh(ui, editor, commands, &tr!("io-import-las-laz-point-cloud")),
        DataMenu::Xyz => draw_import_mesh(ui, editor, commands, &tr!("io-import-ascii-point-cloud")),
        DataMenu::Pcd => draw_import_mesh(ui, editor, commands, &tr!("io-import-pcd-point-cloud")),
        DataMenu::CsvBlockModel => draw_import_csv_block_model(ui, editor, commands),
        DataMenu::Geotiff => draw_import_mesh(ui, editor, commands, &tr!("io-import-geotiff")),
        DataMenu::CsvDrillHole => draw_import_csv_drill_holes(ui, editor, commands),
        DataMenu::None => {}
    });
}

fn draw_export_details(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView) {
    ui.push_id(editor.data_menu, |ui| match editor.data_menu {
        DataMenu::Omf => draw_export_omf(ui, editor, project),
        DataMenu::Dxf => draw_export_dxf(ui, editor, project),
        DataMenu::Obj => draw_export_mesh(ui, editor, project, &tr!("io-export-wavefront-obj")),
        DataMenu::Stl => draw_export_mesh(ui, editor, project, &tr!("io-export-stl")),
        DataMenu::Ply => draw_export_mesh(ui, editor, project, &tr!("io-export-ply")),
        DataMenu::CsvBlockModel => draw_export_csv_block_model(ui, editor, project),
        DataMenu::CsvDrillHole => draw_export_csv_drill_holes(ui, editor, project),
        _ => {}
    });
}

/// The OMF export checklist: what a whole-project export writes.
///
/// One row per section of the data explorer, each ticked to begin with, so the
/// default export is still the whole project. Ticking a section takes all of
/// it, which is why the items under it are disabled while it stands ticked -
/// untick it and they decide for themselves.
fn draw_export_omf(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView) {
    const CHECKLIST_SIZE: egui::Vec2 = egui::vec2(320.0, 340.0);

    ui.heading(tr!("io-export-open-mining-format-2"));
    ui.add_space(6.0);
    let active = project.projects.iter().find(|entry| entry.is_active);
    striped_box(ui, "omf_export_checklist", CHECKLIST_SIZE, |ui| {
        let selection = &mut editor.export_omf;
        checklist_section(
            ui,
            "omf_export_designs",
            &tr!("common-designs"),
            &mut selection.designs,
            active
                .map(|entry| entry.layers.iter().map(|layer| (layer.id, layer.name.clone())).collect())
                .unwrap_or_default(),
            &tr!("common-no-design-layers"),
        );
        checklist_section(
            ui,
            "omf_export_triangulations",
            &tr!("common-triangulations"),
            &mut selection.triangulations,
            project.triangulations.iter().map(|entry| (entry.id, entry.name.clone())).collect(),
            &tr!("common-no-triangulations"),
        );
        checklist_section(
            ui,
            "omf_export_rasters",
            &tr!("common-rasters"),
            &mut selection.rasters,
            project.raster_textures.iter().map(|entry| (entry.id, entry.name.clone())).collect(),
            &tr!("explorer-no-rasters"),
        );
        checklist_section(
            ui,
            "omf_export_point_clouds",
            &tr!("common-point-clouds"),
            &mut selection.point_clouds,
            project.point_clouds.iter().map(|entry| (entry.id, entry.name.clone())).collect(),
            &tr!("common-no-point-clouds"),
        );
        checklist_section(
            ui,
            "omf_export_block_models",
            &tr!("common-block-models"),
            &mut selection.block_models,
            project.block_models.iter().map(|entry| (entry.id, entry.name.clone())).collect(),
            &tr!("common-no-block-models"),
        );
        checklist_section(
            ui,
            "omf_export_drill_holes",
            &tr!("ws-menubar-drillholes"),
            &mut selection.drill_holes,
            project.drill_holes.iter().map(|entry| (entry.id, entry.name.clone())).collect(),
            &tr!("common-no-drill-holes"),
        );
    });
}

/// One section of the export checklist: a heading over the items it writes.
///
/// The heading and its items keep each other honest - see
/// [`OmfExportSection::set_item`] for the rules they follow.
fn checklist_section<Id: Copy + Eq + std::hash::Hash>(
    ui: &mut egui::Ui,
    id_salt: &str,
    title: &str,
    section: &mut OmfExportSection<Id>,
    entries: Vec<(Id, String)>,
    empty_note: &str,
) {
    let height = row_height(ui);
    let id = egui::Id::new(id_salt);
    let state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);
    // Read out of the heading's closure rather than acted on inside it: the
    // closure cannot hold the section while the body's closure needs it too.
    let mut heading_toggle = None;
    let heading = state.show_header(ui, |ui| {
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            let mut ticked = section.all;
            if ui.checkbox(&mut ticked, bold(title)).changed() {
                heading_toggle = Some(ticked);
            }
        });
    });
    // Applied before the body draws, so the items answer the heading in the
    // same frame it was clicked rather than one frame later.
    if let Some(ticked) = heading_toggle {
        section.set_all(ticked);
    }
    heading.body(|ui| {
        if entries.is_empty() {
            explorer_note(ui, empty_note);
            return;
        }
        let every = entries.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        for (entry_id, name) in &entries {
            let mut ticked = section.includes(*entry_id);
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if ui.add(egui::Checkbox::new(&mut ticked, name.clone())).changed() {
                    section.set_item(*entry_id, ticked, &every);
                }
            });
        }
    });
}

fn draw_import_dxf(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    // Opened from Haulage, the lines become roads rather than design
    // geometry; the heading says which, since nothing else on the page does.
    ui.heading(if editor.import_as_haul_roads {
        tr!("haul-import-heading")
    } else {
        tr!("common-import-dxf")
    });
    draw_import_source_picker(ui, editor, commands, tr!("io-source-file"), tr!("io-no-dxf-chosen"));
}

fn draw_import_mesh(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>, heading: &str) {
    ui.heading(heading);
    draw_import_source_picker(ui, editor, commands, tr!("io-source-file"), tr!("common-no-file-chosen"));
}

fn draw_import_source_picker(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    commands: &mut Vec<UiCommand>,
    label: impl Into<egui::WidgetText>,
    empty_text: impl Into<egui::WidgetText>,
) {
    if MenuFieldFilePicker::new(label, selected_import_source_paths(editor))
        .help_text(tr!("io-choose-source-file-files-import"))
        .empty_text(empty_text)
        .button_text(tr!("common-choose"))
        .width(FIELD_WIDTH)
        .show(ui)
        .changed()
    {
        commands.push(UiCommand::ChooseImportSourceFiles(editor.data_menu));
    }
}

fn draw_import_csv_block_model(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    ui.heading(tr!("common-import-csv-block-model"));
    draw_import_source_picker(ui, editor, commands, tr!("io-model-file"), tr!("io-no-csv-chosen"));
    if selected_import_source_paths(editor).is_empty() {
        return;
    }
    if let Some(error) = &editor.import_csv_error {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
    let Some(preview) = editor.import_csv_preview.as_mut() else {
        return;
    };

    menu::menu_section(ui, tr!("io-column-mapping"));
    egui::ScrollArea::both().auto_shrink([false, false]).max_height(ui.available_height()).show(ui, |ui| {
        egui::Grid::new("csv_block_model_preview").striped(true).min_col_width(110.0).show(ui, |ui| {
            for (column, header) in preview.headers.iter().enumerate() {
                ui.vertical(|ui| {
                    ui.strong(if header.is_empty() { tr!("io-blank-header") } else { header.to_string() });
                    let selected = &mut preview.mapping.roles[column];
                    egui::ComboBox::from_id_salt(("csv_column_role", column))
                        .selected_text(selected.label())
                        .width(105.0)
                        .show_ui(ui, |ui| {
                            for role in CsvColumnRole::ALL {
                                ui.selectable_value(selected, role, role.label());
                            }
                        });
                });
            }
            ui.end_row();
            for row in &preview.rows {
                for column in 0..preview.headers.len() {
                    let value = row.get(column).map(String::as_str).unwrap_or("");
                    ui.label(value);
                }
                ui.end_row();
            }
        });
    });
    let unmapped_dimensions: Vec<CsvColumnRole> = [CsvColumnRole::Dx, CsvColumnRole::Dy, CsvColumnRole::Dz]
        .into_iter()
        .filter(|role| !preview.mapping.roles.contains(role))
        .collect();
    if !unmapped_dimensions.is_empty() {
        menu::menu_section(ui, tr!("io-fixed-block-size"));
        ui.small(tr!("io-no-column-mapped-these"));
        egui::Grid::new("csv_fixed_dimensions").min_col_width(110.0).show(ui, |ui| {
            for role in unmapped_dimensions {
                let fixed = match role {
                    CsvColumnRole::Dx => &mut preview.mapping.fixed_dx,
                    CsvColumnRole::Dy => &mut preview.mapping.fixed_dy,
                    CsvColumnRole::Dz => &mut preview.mapping.fixed_dz,
                    _ => unreachable!("filtered to Dx/Dy/Dz above"),
                };
                let mut value = fixed.map_or(1.0, |fixed| fixed.0);
                if MenuFieldF64::new(role.label(), &mut value, 0.0..=1.0e9).suffix("m".to_owned()).show_inline(ui).changed() || fixed.is_none() {
                    *fixed = Some(FixedDimension(value));
                }
                ui.end_row();
            }
        });
    }
    if let Err(error) = validate_mapping(&preview.mapping, preview.headers.len()) {
        ui.colored_label(ui.visuals().error_fg_color, error.to_string());
    }
}

fn draw_import_csv_drill_holes(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    ui.heading(tr!("io-import-drillhole-csv-bundle"));
    draw_import_source_picker(ui, editor, commands, tr!("io-csv-files"), tr!("io-no-csv-files-chosen"));
    if let Some(error) = &editor.import_csv_error {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
    if editor.import_drill_csv.is_empty() {
        return;
    }
    egui::ScrollArea::both().auto_shrink([false, false]).max_height(ui.available_height()).show(ui, |ui| {
        for (file_index, (mapping, preview)) in editor.import_drill_csv.iter_mut().enumerate() {
            ui.push_id(("drill_csv_file", file_index), |ui| {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong(mapping.path.file_name().and_then(|name| name.to_str()).unwrap_or("CSV"));
                    let previous = mapping.role;
                    egui::ComboBox::from_id_salt("file_role").selected_text(file_role_label(mapping.role)).show_ui(ui, |ui| {
                        for role in [
                            CsvDrillFileRole::Unassigned,
                            CsvDrillFileRole::Collar,
                            CsvDrillFileRole::Survey,
                            CsvDrillFileRole::Interval,
                            CsvDrillFileRole::ExplicitSegments,
                            CsvDrillFileRole::Geophysics,
                        ] {
                            ui.selectable_value(&mut mapping.role, role, file_role_label(role));
                        }
                    });
                    if mapping.role != previous {
                        mapping.columns = crate::model::formats::csv_drill_hole::default_columns(mapping.role, &preview.headers);
                    }
                });
                if mapping.role == CsvDrillFileRole::Unassigned {
                    ui.weak(tr!("io-choose-file-purpose-map-its"));
                }
                egui::Grid::new("mapping").striped(true).min_col_width(100.0).show(ui, |ui| {
                    for (column, header) in preview.headers.iter().enumerate() {
                        ui.vertical(|ui| {
                            ui.strong(header);
                            if mapping.role == CsvDrillFileRole::Unassigned {
                                ui.weak(tr!("io-unmapped"));
                            } else {
                                let selected = &mut mapping.columns[column];
                                let combo = egui::ComboBox::from_id_salt(("column", column))
                                    .selected_text(column_role_label(selected))
                                    .width(115.0)
                                    .show_ui(ui, |ui| {
                                        for column_role in available_column_roles(mapping.role, header) {
                                            let label = column_role_label(&column_role);
                                            ui.selectable_value(selected, column_role, label);
                                        }
                                    });
                                if let Some(help) = column_role_help(mapping.role, selected) {
                                    combo.response.on_hover_text(help);
                                }
                            }
                        });
                    }
                    ui.end_row();
                    for row in preview.rows.iter().take(3) {
                        for column in 0..preview.headers.len() {
                            ui.label(row.get(column).map_or("", String::as_str));
                        }
                        ui.end_row();
                    }
                });
            });
        }
    });
    if geophysics_without_holes(&editor.import_drill_csv) {
        ui.weak(tr!("io-add-collar-file-explicit-segments"));
    }
}

fn file_role_label(role: CsvDrillFileRole) -> String {
    match role {
        CsvDrillFileRole::Unassigned => tr!("io-choose-purpose"),
        CsvDrillFileRole::Collar => tr!("io-collar"),
        CsvDrillFileRole::Survey => tr!("ws-survey"),
        CsvDrillFileRole::Interval => tr!("io-interval"),
        CsvDrillFileRole::ExplicitSegments => tr!("io-explicit-segments"),
        CsvDrillFileRole::Geophysics => tr!("io-downhole-geophysics"),
    }
}

fn column_role_label(role: &CsvDrillColumnRole) -> String {
    match role {
        CsvDrillColumnRole::Ignore => tr!("common-ignore"),
        CsvDrillColumnRole::Dhid => "DHID".to_owned(),
        CsvDrillColumnRole::East => tr!("io-east-x"),
        CsvDrillColumnRole::North => tr!("io-north-y"),
        CsvDrillColumnRole::Elevation => tr!("io-elevation-z"),
        CsvDrillColumnRole::Depth => tr!("io-depth"),
        CsvDrillColumnRole::Azimuth => tr!("edit-azimuth"),
        CsvDrillColumnRole::Dip => tr!("edit-dip"),
        CsvDrillColumnRole::Inclination => tr!("io-inclination"),
        CsvDrillColumnRole::From => "FROM".to_owned(),
        CsvDrillColumnRole::To => "TO".to_owned(),
        CsvDrillColumnRole::StartEast => tr!("io-start-x"),
        CsvDrillColumnRole::StartNorth => tr!("io-start-y"),
        CsvDrillColumnRole::StartElevation => tr!("io-start-z"),
        CsvDrillColumnRole::EndEast => tr!("io-end-x"),
        CsvDrillColumnRole::EndNorth => tr!("io-end-y"),
        CsvDrillColumnRole::EndElevation => tr!("io-end-z"),
        CsvDrillColumnRole::Diameter => tr!("io-diameter"),
        CsvDrillColumnRole::Gamma => tr!("io-gamma-api"),
        CsvDrillColumnRole::LongDensity => tr!("io-long-spaced-density-g-cc"),
        CsvDrillColumnRole::ShortDensity => tr!("io-short-spaced-density-g-cc"),
        CsvDrillColumnRole::Attribute(_) => tr!("io-attribute"),
    }
}

/// Hover help for a geophysics column's unit convention, `None` elsewhere.
fn column_role_help(file: CsvDrillFileRole, column: &CsvDrillColumnRole) -> Option<String> {
    if file != CsvDrillFileRole::Geophysics {
        return None;
    }
    match column {
        CsvDrillColumnRole::Depth => Some(tr!("io-measured-depth-down-hole-read")),
        CsvDrillColumnRole::Gamma => Some(tr!("io-natural-gamma-read-api-units")),
        CsvDrillColumnRole::LongDensity | CsvDrillColumnRole::ShortDensity => Some(tr!("io-density-read-g-cc-exported")),
        _ => None,
    }
}

fn available_column_roles(role: CsvDrillFileRole, header: &str) -> Vec<CsvDrillColumnRole> {
    let mut roles = vec![CsvDrillColumnRole::Ignore, CsvDrillColumnRole::Dhid];
    match role {
        CsvDrillFileRole::Unassigned => roles.truncate(1),
        CsvDrillFileRole::Collar => roles.extend([
            CsvDrillColumnRole::East,
            CsvDrillColumnRole::North,
            CsvDrillColumnRole::Elevation,
            CsvDrillColumnRole::Diameter,
        ]),
        CsvDrillFileRole::Survey => roles.extend([
            CsvDrillColumnRole::Depth,
            CsvDrillColumnRole::East,
            CsvDrillColumnRole::North,
            CsvDrillColumnRole::Elevation,
            CsvDrillColumnRole::Azimuth,
            CsvDrillColumnRole::Dip,
            CsvDrillColumnRole::Inclination,
        ]),
        CsvDrillFileRole::Interval => roles.extend([CsvDrillColumnRole::From, CsvDrillColumnRole::To, CsvDrillColumnRole::Attribute(header.to_owned())]),
        CsvDrillFileRole::ExplicitSegments => roles.extend([
            CsvDrillColumnRole::From,
            CsvDrillColumnRole::To,
            CsvDrillColumnRole::StartEast,
            CsvDrillColumnRole::StartNorth,
            CsvDrillColumnRole::StartElevation,
            CsvDrillColumnRole::EndEast,
            CsvDrillColumnRole::EndNorth,
            CsvDrillColumnRole::EndElevation,
            CsvDrillColumnRole::Diameter,
            CsvDrillColumnRole::Attribute(header.to_owned()),
        ]),
        CsvDrillFileRole::Geophysics => roles.extend([
            CsvDrillColumnRole::Depth,
            CsvDrillColumnRole::Gamma,
            CsvDrillColumnRole::LongDensity,
            CsvDrillColumnRole::ShortDensity,
        ]),
    }
    roles
}

fn draw_export_dxf(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView) {
    ui.heading(tr!("io-export-dxf"));
    MenuFieldBool::new(tr!("io-export-one-layer"), &mut editor.export_dxf_layer).show(ui);
    if editor.export_dxf_layer {
        ensure_export_layer(editor, project);
        layer_combo(ui, "dxf_export_layer", &tr!("ws-menubar-active-layer"), project, &mut editor.export_layer);
    } else {
        // A workspace holds one project, so a whole-project export just takes
        // the active one rather than offering a choice of exactly one.
        ensure_export_project(editor, project);
    }
}

fn draw_export_mesh(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, heading: &str) {
    ui.heading(heading);
    ensure_export_triangulation(editor, project);
    triangulation_combo(ui, "mesh_export_triangulation", &tr!("io-triangulation"), project, &mut editor.export_triangulation);
}

fn draw_export_csv_block_model(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView) {
    ui.heading(tr!("io-export-csv-block-model"));
    ensure_export_block_model(editor, project);
    block_model_combo(ui, "csv_export_block_model", &tr!("io-block-model"), project, &mut editor.export_block_model);
}

fn draw_export_csv_drill_holes(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView) {
    ui.heading(tr!("io-export-csv-drillholes"));
    ensure_export_drill_hole(editor, project);
    drill_hole_combo(ui, "csv_export_drill_hole", &tr!("io-dataset"), project, &mut editor.export_drill_hole);
    ui.small(tr!("io-writes-three-files-beside-name"));
}

fn layer_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, field_label: &str, project: &UiProjectView, selected: &mut Option<LayerId>) {
    let active_entry = project.projects.iter().find(|entry| entry.is_active);
    let selected_label =
        selected.and_then(|selected| active_entry.and_then(|entry| entry.layers.iter().find(|layer| layer.is_loaded && layer.id == selected).map(|layer| layer.name.clone())));
    let options = active_entry
        .into_iter()
        .flat_map(|entry| entry.layers.iter().filter(|layer| layer.is_loaded).map(|layer| (Some(layer.id), layer.name.clone().into())));
    MenuFieldCombo::new(id, field_label, selected, selected_label.unwrap_or_else(|| tr!("io-choose-loaded-layer")), options)
        .width(FIELD_WIDTH)
        .show(ui);
}

fn triangulation_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, field_label: &str, project: &UiProjectView, selected: &mut Option<TriangulationId>) {
    let label = selected
        .and_then(|id| project.triangulations.iter().find(|entry| entry.id == id && entry.is_loaded))
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| tr!("io-choose-loaded-triangulation"));
    MenuFieldCombo::new(
        id,
        field_label,
        selected,
        label,
        project
            .triangulations
            .iter()
            .filter(|entry| entry.is_loaded)
            .map(|entry| (Some(entry.id), entry.name.clone().into())),
    )
    .width(FIELD_WIDTH)
    .show(ui);
}

fn block_model_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, field_label: &str, project: &UiProjectView, selected: &mut Option<BlockModelId>) {
    let label = selected
        .and_then(|id| project.block_models.iter().find(|entry| entry.id == id && entry.is_loaded))
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| tr!("io-choose-loaded-block-model"));
    MenuFieldCombo::new(
        id,
        field_label,
        selected,
        label,
        project
            .block_models
            .iter()
            .filter(|entry| entry.is_loaded)
            .map(|entry| (Some(entry.id), entry.name.clone().into())),
    )
    .width(FIELD_WIDTH)
    .show(ui);
}

fn selected_import_source_paths(editor: &EditorState) -> &[std::path::PathBuf] {
    if editor.import_source_menu == editor.data_menu {
        &editor.import_source_paths
    } else {
        &[]
    }
}

fn reset_import_defaults(editor: &mut EditorState, _project: &UiProjectView) {
    if editor.import_source_menu == editor.data_menu {
        editor.import_source_menu = DataMenu::None;
        editor.import_source_paths.clear();
    }
    match editor.data_menu {
        DataMenu::CsvBlockModel => {
            editor.import_csv_preview = None;
            editor.import_csv_error = None;
        }
        DataMenu::CsvDrillHole => {
            editor.import_drill_csv.clear();
            editor.import_csv_error = None;
        }
        _ => {}
    }
}

fn reset_export_defaults(editor: &mut EditorState, project: &UiProjectView) {
    match editor.data_menu {
        DataMenu::Omf => {
            editor.export_omf = crate::ui::state::OmfExportSelection::default();
        }
        DataMenu::Dxf => {
            editor.export_dxf_layer = false;
            editor.export_layer = first_loaded_layer(project);
            editor.export_project = active_project(project);
        }
        DataMenu::Obj | DataMenu::Stl | DataMenu::Ply => {
            editor.export_triangulation = first_loaded_triangulation(project);
        }
        DataMenu::CsvBlockModel => {
            editor.export_block_model = first_loaded_block_model(project);
        }
        DataMenu::CsvDrillHole => {
            editor.export_drill_hole = first_loaded_drill_hole(project);
        }
        _ => {}
    }
}

fn ensure_export_layer(editor: &mut EditorState, project: &UiProjectView) {
    if !has_loaded_layer(project, editor.export_layer) {
        editor.export_layer = first_loaded_layer(project);
    }
}

fn ensure_export_project(editor: &mut EditorState, project: &UiProjectView) {
    if !has_project(project, editor.export_project) {
        editor.export_project = active_project(project);
    }
}

fn ensure_export_triangulation(editor: &mut EditorState, project: &UiProjectView) {
    if !has_loaded_triangulation(project, editor.export_triangulation) {
        editor.export_triangulation = first_loaded_triangulation(project);
    }
}

fn ensure_export_drill_hole(editor: &mut EditorState, project: &UiProjectView) {
    if !has_loaded_drill_hole(project, editor.export_drill_hole) {
        editor.export_drill_hole = first_loaded_drill_hole(project);
    }
}

fn drill_hole_combo(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, field_label: &str, project: &UiProjectView, selected: &mut Option<DrillHoleId>) {
    let label = selected
        .and_then(|id| project.drill_holes.iter().find(|entry| entry.id == id && entry.is_loaded))
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| tr!("io-choose-loaded-dataset"));
    MenuFieldCombo::new(
        id,
        field_label,
        selected,
        label,
        project
            .drill_holes
            .iter()
            .filter(|entry| entry.is_loaded)
            .map(|entry| (Some(entry.id), entry.name.clone().into())),
    )
    .width(FIELD_WIDTH)
    .show(ui);
}

fn ensure_export_block_model(editor: &mut EditorState, project: &UiProjectView) {
    if !has_loaded_block_model(project, editor.export_block_model) {
        editor.export_block_model = first_loaded_block_model(project);
    }
}

/// True when the bundle carries a downhole-geophysics file but no file the
/// holes it should attach to could be named after (no collar, no
/// explicit-segments file).
fn geophysics_without_holes(files: &[(CsvDrillFileMapping, CsvDrillPreview)]) -> bool {
    files.iter().any(|(mapping, _)| mapping.role == CsvDrillFileRole::Geophysics) && bundle_anchor(files.iter().map(|(mapping, _)| mapping)).is_none()
}

fn import_command(editor: &EditorState) -> Option<UiCommand> {
    let source_paths = selected_import_source_paths(editor);
    match editor.data_menu {
        DataMenu::Omf => (!source_paths.is_empty()).then(|| UiCommand::ImportOmfPaths(source_paths.to_vec())),
        DataMenu::Dxf => (!source_paths.is_empty()).then(|| UiCommand::ImportDxfPathsInto(source_paths.to_vec())),
        DataMenu::Obj | DataMenu::Stl | DataMenu::Ply => (!source_paths.is_empty()).then(|| UiCommand::ImportTriangulationPaths(source_paths.to_vec())),
        DataMenu::Las | DataMenu::Xyz | DataMenu::Pcd => (!source_paths.is_empty()).then(|| UiCommand::ImportPointCloudPaths(source_paths.to_vec())),
        DataMenu::Geotiff => (!source_paths.is_empty()).then(|| UiCommand::ImportRasterPaths(source_paths.to_vec())),
        DataMenu::CsvBlockModel => {
            let path = source_paths.first()?.clone();
            let preview = editor.import_csv_preview.as_ref()?;
            validate_mapping(&preview.mapping, preview.headers.len()).ok()?;
            Some(UiCommand::ImportCsvBlockModel {
                path,
                mapping: preview.mapping.clone(),
            })
        }
        DataMenu::CsvDrillHole
            if editor.import_csv_error.is_none()
                && !editor.import_drill_csv.is_empty()
                && editor.import_drill_csv.iter().all(|(mapping, _)| mapping.role != CsvDrillFileRole::Unassigned)
                && !geophysics_without_holes(&editor.import_drill_csv) =>
        {
            let anchor = bundle_anchor(editor.import_drill_csv.iter().map(|(mapping, _)| mapping));
            let name = anchor
                .or_else(|| editor.import_drill_csv.first().map(|(mapping, _)| mapping))?
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_owned)
                .unwrap_or_else(|| tr!("io-drill-holes"));
            Some(UiCommand::ImportDrillHole(crate::model::drill_hole::DrillHoleSource::Csv {
                name: if editor.import_drill_csv.len() > 1 {
                    tr!("io-name-count-files", name = name.to_string(), count = (editor.import_drill_csv.len() - 1).to_string())
                } else {
                    name
                },
                files: editor.import_drill_csv.iter().map(|(mapping, _)| mapping.clone()).collect(),
                browser_path: None,
            }))
        }
        DataMenu::CsvDrillHole | DataMenu::None => None,
    }
}

fn export_command(editor: &EditorState) -> Option<UiCommand> {
    match editor.data_menu {
        DataMenu::Omf if editor.export_omf.is_empty() => None,
        DataMenu::Omf => Some(UiCommand::ExportOmf(Box::new(editor.export_omf.clone()))),
        DataMenu::Dxf if editor.export_dxf_layer => editor.export_layer.map(UiCommand::ExportLayerDxf),
        DataMenu::Dxf => editor.export_project.map(UiCommand::ExportProjectDxf),
        DataMenu::Obj | DataMenu::Stl | DataMenu::Ply => {
            let format = mesh_format(editor.data_menu)?;
            editor.export_triangulation.map(|id| UiCommand::ExportTriangulationAs(id, format))
        }
        DataMenu::CsvBlockModel => editor.export_block_model.map(UiCommand::ExportBlockModelCsv),
        DataMenu::CsvDrillHole => editor.export_drill_hole.map(UiCommand::ExportDrillHoleCsv),
        _ => None,
    }
}

fn mesh_format(data_menu: DataMenu) -> Option<MeshFormat> {
    match data_menu {
        DataMenu::Obj => Some(MeshFormat::Obj),
        DataMenu::Stl => Some(MeshFormat::Stl),
        DataMenu::Ply => Some(MeshFormat::Ply),
        _ => None,
    }
}

fn is_import_menu(data_menu: DataMenu) -> bool {
    matches!(
        data_menu,
        DataMenu::Omf
            | DataMenu::Dxf
            | DataMenu::Obj
            | DataMenu::Stl
            | DataMenu::Ply
            | DataMenu::Las
            | DataMenu::Xyz
            | DataMenu::Pcd
            | DataMenu::CsvBlockModel
            | DataMenu::CsvDrillHole
            | DataMenu::Geotiff
    )
}

fn is_export_menu(data_menu: DataMenu) -> bool {
    matches!(
        data_menu,
        DataMenu::Omf | DataMenu::Dxf | DataMenu::Obj | DataMenu::Stl | DataMenu::Ply | DataMenu::CsvBlockModel | DataMenu::CsvDrillHole
    )
}

fn first_loaded_layer(project: &UiProjectView) -> Option<LayerId> {
    project
        .projects
        .iter()
        .find(|entry| entry.is_active)
        .and_then(|entry| entry.layers.iter().find(|layer| layer.is_loaded).map(|layer| layer.id))
}

fn active_project(project: &UiProjectView) -> Option<u32> {
    project.projects.iter().find(|entry| entry.is_active).map(|entry| entry.runtime_id)
}

fn first_loaded_triangulation(project: &UiProjectView) -> Option<TriangulationId> {
    project.triangulations.iter().find(|entry| entry.is_loaded).map(|entry| entry.id)
}

fn first_loaded_drill_hole(project: &UiProjectView) -> Option<DrillHoleId> {
    project.drill_holes.iter().find(|entry| entry.is_loaded).map(|entry| entry.id)
}

fn first_loaded_block_model(project: &UiProjectView) -> Option<BlockModelId> {
    project.block_models.iter().find(|entry| entry.is_loaded).map(|entry| entry.id)
}

fn has_loaded_layer(project: &UiProjectView, selected: Option<LayerId>) -> bool {
    selected.is_some_and(|selected| {
        project
            .projects
            .iter()
            .any(|entry| entry.is_active && entry.layers.iter().any(|layer| layer.is_loaded && layer.id == selected))
    })
}

fn has_project(project: &UiProjectView, selected: Option<u32>) -> bool {
    selected.is_some_and(|runtime_id| project.projects.iter().any(|entry| entry.runtime_id == runtime_id))
}

fn has_loaded_triangulation(project: &UiProjectView, selected: Option<TriangulationId>) -> bool {
    selected.is_some_and(|id| project.triangulations.iter().any(|entry| entry.is_loaded && entry.id == id))
}

fn has_loaded_drill_hole(project: &UiProjectView, selected: Option<DrillHoleId>) -> bool {
    selected.is_some_and(|id| project.drill_holes.iter().any(|entry| entry.is_loaded && entry.id == id))
}

fn has_loaded_block_model(project: &UiProjectView, selected: Option<BlockModelId>) -> bool {
    selected.is_some_and(|id| project.block_models.iter().any(|entry| entry.is_loaded && entry.id == id))
}
