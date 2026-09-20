//! Left-side explorer panel for the active project's retained content.

use crate::{
    i18n::{tr, tr_format},
    model::{Folder, FolderId, FolderMember, ItemRef, SceneEntityId, SectionKind},
    ui::{
        EditorState, UiCommand, UiProjectView,
        fonts::bold,
        state::{ExplorerRow, ExplorerSection, RenameTarget, UiBlockModelEntry, UiDrillHoleEntry, UiLayerEntry, UiPointCloudEntry, UiRasterTextureEntry},
        unthemed_icon,
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup, context_menu_separator, context_submenu},
            explorer::{EntryToggles, ExplorerEntry, ExplorerHeader, explorer_note, paint_fixed_stripes, reserve_fixed_stripes},
        },
    },
};

/// Grey colour used for inactive (not loaded) layers and triangulations.
const INACTIVE_TEXT_COLOR: egui::Color32 = egui::Color32::from_gray(140);

/// Section heading tints, keyed to the icons each section's entries use.
///
/// One colour serves both themes: each holds at least a 3:1 contrast ratio
/// against the light panel (white) and the dark one alike.
const HEADER_DESIGNS: egui::Color32 = egui::Color32::from_rgb(0x44, 0x62, 0xBF);
const HEADER_TRIANGULATIONS: egui::Color32 = egui::Color32::from_rgb(0xAE, 0x58, 0xDB);
const HEADER_RASTERS: egui::Color32 = egui::Color32::from_rgb(0x2F, 0x91, 0x99);
const HEADER_POINT_CLOUDS: egui::Color32 = egui::Color32::from_rgb(0xC9, 0x3B, 0x2C);
const HEADER_BLOCK_MODELS: egui::Color32 = egui::Color32::from_rgb(0x69, 0x8F, 0x3F);
const HEADER_DRILL_HOLES: egui::Color32 = egui::Color32::from_rgb(0xDB, 0x5F, 0x58);

/// What every dragged explorer row has in common: the folder member it
/// stands for, tag and all.
trait DragPayload: Send + Sync + Clone + 'static {
    fn member(&self) -> FolderMember;
}

/// What a dragged row of each section carries.
///
/// Typed per section so a drop zone can never match another kind's payload.
macro_rules! drag_payload {
    ($($name:ident),* $(,)?) => {
        $(
            #[derive(Clone, Copy)]
            struct $name(FolderMember);
            impl DragPayload for $name {
                fn member(&self) -> FolderMember {
                    self.0
                }
            }
        )*
    };
}
drag_payload!(DesignLayerDrag, TriangulationDrag, RasterDrag, PointCloudDrag, BlockModelDrag, DrillHoleDrag);

/// Whether a zone of `section` takes `member`.
///
/// The zone must show that kind of item, and the item must already be
/// tagged for this section - moving between sections is not what a drag does.
fn accepts(section: SectionKind, member: FolderMember) -> bool {
    section.admits(member.kind()) && member.section() == section
}

/// The member a drop of payload type `P` onto a zone of `section` delivers.
///
/// `None` unless the row was released with a payload of that type and this
/// zone [`accepts`] it: the payload type gates which drags can even be
/// offered here, and `accepts` gates the rest.
fn dropped_here<P: DragPayload>(row: &egui::Response, section: SectionKind) -> Option<FolderMember> {
    let member = row.dnd_release_payload::<P>()?.member();
    accepts(section, member).then_some(member)
}

/// Outline a row currently held over by a drag this zone would accept.
///
/// Nothing is drawn for a payload of another type, nor for one this zone
/// would refuse, so a drag this row cannot accept passes over it without
/// offering anything.
fn paint_drop_target<P: DragPayload>(ui: &egui::Ui, row: &egui::Response, section: SectionKind) {
    let Some(payload) = egui::DragAndDrop::payload::<P>(ui.ctx()) else {
        return;
    };
    if !accepts(section, payload.member()) || !row.contains_pointer() {
        return;
    }
    ui.painter().rect_stroke(
        row.rect,
        crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS,
        ui.visuals().selection.stroke,
        egui::StrokeKind::Inside,
    );
}

/// What the explorer needs of any row it can draw under a section heading.
trait SectionEntry {
    fn section(&self) -> SectionKind;
    fn folder(&self) -> Option<FolderId>;
}

macro_rules! section_entry {
    ($($ty:ty),* $(,)?) => {
        $(
            impl SectionEntry for $ty {
                fn section(&self) -> SectionKind {
                    self.section
                }
                fn folder(&self) -> Option<FolderId> {
                    self.folder
                }
            }
        )*
    };
}
section_entry!(
    UiLayerEntry,
    crate::ui::UiTriangulationEntry,
    UiRasterTextureEntry,
    UiPointCloudEntry,
    UiBlockModelEntry,
    UiDrillHoleEntry
);

/// The entries of `items` shown under `section` in `folder`, or at its root
/// when `folder` is `None`.
///
/// An entry whose folder no longer exists draws at the root, not nowhere.
fn shown_in<'a, T: SectionEntry>(items: &'a [T], section: SectionKind, folder: Option<FolderId>, known: &'a [Folder]) -> impl Iterator<Item = &'a T> + 'a {
    items.iter().filter(move |item| {
        item.section() == section
            && match folder {
                Some(id) => item.folder() == Some(id),
                None => item.folder().is_none_or(|folder| !known.iter().any(|existing| existing.id == folder)),
            }
    })
}

/// How many of `items` are tagged with `section`, for a heading's item count
/// and its empty-section notes - never the whole collection, which may also
/// hold items tagged elsewhere.
fn tagged_count<T: SectionEntry>(items: &[T], section: SectionKind) -> usize {
    items.iter().filter(|item| item.section() == section).count()
}

/// Draw one explorer folder: a collapsible group holding its members, whose
/// heading row is both its right-click menu and the target a row of payload
/// type `P` is dropped onto to move it in here.
fn folder_group<P: DragPayload>(ui: &mut egui::Ui, section: SectionKind, folder: &Folder, commands: &mut Vec<UiCommand>, body: impl FnOnce(&mut egui::Ui, &mut Vec<UiCommand>)) {
    // Keyed by section and id, not by name, so renaming a folder leaves it as
    // open or as shut as the user left it.
    let (toggle, heading, _) = ExplorerHeader::new(egui::Id::new((section.key(), folder.id)), folder.name.as_str()).show(ui, |ui| body(ui, commands));
    let row = toggle.union(heading.inner);
    if let Some(member) = dropped_here::<P>(&row, section) {
        commands.push(UiCommand::MoveToFolder { member, folder: Some(folder.id) });
    }
    paint_drop_target::<P>(ui, &row, section);
    context_menu_popup(&row, folder.name.as_str(), |ui| {
        if ContextMenuAction::new(tr!(literal = "Rename")).show(ui).clicked() {
            commands.push(UiCommand::BeginRenameItem(RenameTarget::Folder(section, folder.id)));
            ui.close();
        }
        context_menu_separator(ui);
        // Deleting a folder is not deleting what is in it: its members return
        // to the section root.
        if ContextMenuAction::new(tr!(literal = "Delete Collection")).show(ui).clicked() {
            commands.push(UiCommand::DeleteFolder { section, folder: folder.id });
            ui.close();
        }
    });
}

/// Draw one section's rows: a group per folder, then whatever sits at the
/// section root.
fn draw_section_body<P: DragPayload, T: SectionEntry>(
    ui: &mut egui::Ui,
    section: SectionKind,
    folders: &[Folder],
    items: &[T],
    commands: &mut Vec<UiCommand>,
    mut row: impl FnMut(&mut egui::Ui, &mut Vec<UiCommand>, &T),
) {
    for folder in folders {
        folder_group::<P>(ui, section, folder, commands, |ui, commands| {
            let mut in_folder = shown_in(items, section, Some(folder.id), folders).peekable();
            if in_folder.peek().is_none() {
                explorer_note(ui, tr!(literal = "Empty collection"));
            }
            for item in in_folder {
                row(ui, commands, item);
            }
        });
    }
    for item in shown_in(items, section, None, folders) {
        row(ui, commands, item);
    }
}

/// Attach a section heading's root drop zone: releasing a row of payload
/// type `P` here returns it to the section root.
fn attach_header_drop<P: DragPayload>(ui: &egui::Ui, row: &egui::Response, section: SectionKind, commands: &mut Vec<UiCommand>) {
    if let Some(member) = dropped_here::<P>(row, section) {
        commands.push(UiCommand::MoveToFolder { member, folder: None });
    }
    paint_drop_target::<P>(ui, row, section);
}

/// The "Move to Folder" submenu shared by every section's row context menu:
/// "No Folder" plus one entry per folder, ticked on whichever the row is
/// currently in.
fn move_to_folder_submenu(ui: &mut egui::Ui, folders: &[Folder], current: Option<FolderId>, member: FolderMember, commands: &mut Vec<UiCommand>) {
    context_submenu(ui, &tr!(literal = "Move to Collection"), !folders.is_empty(), |ui| {
        if ContextMenuAction::new(tr!(literal = "No Collection")).checked(current.is_none()).show(ui).clicked() {
            commands.push(UiCommand::MoveToFolder { member, folder: None });
            ui.close();
        }
        context_menu_separator(ui);
        for folder in folders {
            if ContextMenuAction::new(folder.name.as_str()).checked(current == Some(folder.id)).show(ui).clicked() {
                commands.push(UiCommand::MoveToFolder { member, folder: Some(folder.id) });
                ui.close();
            }
        }
    });
}

/// Attach a data section heading's right-click menu.
///
/// The four actions do to the whole section exactly what its rows' own eye
/// and padlock do one at a time. Both controls work on unloaded entries; the
/// menu greys out when the section is empty.
fn section_heading_menu(response: &egui::Response, section: ExplorerSection, item_count: usize, commands: &mut Vec<UiCommand>) {
    context_menu_popup(response, section.label(), |ui| {
        let enabled = item_count > 0;
        if ContextMenuAction::new(tr!(literal = "New Collection")).show(ui).clicked() {
            commands.push(UiCommand::CreateFolder(section.kind()));
            ui.close();
        }
        context_menu_separator(ui);
        if ContextMenuAction::new(tr!(literal = "Reveal All")).enabled(enabled).show(ui).clicked() {
            commands.push(UiCommand::SetSectionVisible(section, true));
            ui.close();
        }
        if ContextMenuAction::new(tr!(literal = "Hide All")).enabled(enabled).show(ui).clicked() {
            commands.push(UiCommand::SetSectionVisible(section, false));
            ui.close();
        }
        context_menu_separator(ui);
        if ContextMenuAction::new(tr!(literal = "Lock All")).enabled(enabled).show(ui).clicked() {
            commands.push(UiCommand::SetSectionLocked(section, true));
            ui.close();
        }
        if ContextMenuAction::new(tr!(literal = "Unlock All")).enabled(enabled).show(ui).clicked() {
            commands.push(UiCommand::SetSectionLocked(section, false));
            ui.close();
        }
    });
}

/// Id of the explorer's column panel. Shared with [`crate::ui::chrome`],
/// which reads the panel's resize interaction to light up its grip.
pub(crate) const PANEL_ID: &str = "explorer_panel";

/// The explorer column and its separate island surfaces.
pub(crate) struct ExplorerLayout {
    /// The whole column, gaps included: what the panels drawn after it lay out
    /// against.
    pub(crate) column: egui::Rect,
    /// What the data tree claimed.
    pub(crate) tree: egui::Rect,
    /// What the products island claimed, when the workspace uses it.
    pub(crate) products: Option<egui::Rect>,
}

/// Draw the full-height project explorer.
pub(crate) fn draw_explorer(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) -> ExplorerLayout {
    let (surface, stripe) = crate::ui::widgets::tree_row_colors(ui);
    let (_, max_width) = crate::ui::chrome::panel_size_limits(ui.ctx(), ui.available_width());
    let column = egui::Panel::left(PANEL_ID)
        .resizable(true)
        .show_separator_line(crate::ui::chrome::show_separator_line(ui))
        .default_size(280.0_f32.min(max_width))
        .min_size(220.0_f32.min(max_width))
        .max_size(max_width)
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            // Prevent content from forcing the panel wider than the user has dragged it.
            ui.set_max_width(ui.available_width());

            let products = (editor.active_workspace == crate::ui::state::Workspace::DrillAndBlast).then(|| super::products::draw_products_panel(ui, editor));
            if products.is_none() {
                ui.skip_ahead_auto_ids(1);
            }

            // Every row records itself here as it is drawn, so a Shift-click
            // can be resolved against the rows the user is looking at: see
            // `EditorState::explorer_rows`.
            let mut rows: Vec<ExplorerRow> = Vec::new();
            let tree = crate::ui::chrome::region_frame(ui)
                .fill(surface)
                .inner_margin(egui::Margin::ZERO)
                .show(ui, |ui| {
                    // The tree only reads editor state. Destructuring here keeps the
                    // row closures below from capturing `editor` mutably, which the
                    // borrow checker would otherwise reject against these shared reads.
                    let EditorState {
                        active_layer,
                        selected_handles,
                        locked_layers,
                        locked_rasters,
                        frozen_handles,
                        ..
                    } = &*editor;

                    // Keep the scroll area's contents as wide as the side panel even
                    // when every section is collapsed. `ScrollArea` otherwise shrinks
                    // horizontally to the headers' intrinsic width; a visible
                    // `ExplorerEntry` masks that by requesting all available width,
                    // which made panel resizing depend on whether an entry existed.
                    //
                    // Vertical shrinking is off so the banding below the last row has
                    // the full panel height to run into: see `paint_fixed_stripes`.
                    egui::ScrollArea::vertical().auto_shrink([false; 2]).min_scrolled_height(0.0).show(ui, |ui| {
                        // Empty-state messages should behave like explorer entries at
                        // narrow widths: stay on one line and end with an ellipsis.
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        // Rows carry their own height and butt up against each other,
                        // so the stripes tile the list without gaps.
                        ui.spacing_mut().item_spacing.y = 0.0;

                        // Reserved before any row is laid out, and filled once the
                        // tree's final height is known below: see `paint_fixed_stripes`.
                        let (stripes_slot, stripes_top) = reserve_fixed_stripes(ui);

                        // A row drag and a list drag are one gesture under a
                        // finger, so rows offer a drag to a pointer only;
                        // touch moves an item with Move to Folder.
                        let rows_draggable = !ui.ctx().input(|input| input.any_touches());

                        let designs_dirty = project.projects.first().is_some_and(|entry| entry.designs_dirty);
                        let (designs_header_toggle, designs_header, _) = ExplorerHeader::new(egui::Id::new("designs_collapse"), tr!(literal = "Designs"))
                            .icon(unthemed_icon!("layer.svg"))
                            .color(HEADER_DESIGNS)
                            .dirty(designs_dirty)
                            .show(ui, |ui| {
                                let Some(entry) = project.projects.first() else {
                                    explorer_note(ui, tr!(literal = "No open project"));
                                    return;
                                };
                                let design_folders = project.folders.folders(SectionKind::Designs);
                                if tagged_count(&entry.layers, SectionKind::Designs) == 0 && design_folders.is_empty() {
                                    explorer_note(ui, tr!(literal = "No design layers"));
                                }
                                // One row builder for both places a layer can
                                // sit: inside a folder, or loose at the root.
                                let layer_row = |ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, layer: &UiLayerEntry| {
                                    let layer_id = layer.id;
                                    rows.push(ExplorerRow::Layer(layer_id));
                                    let is_active = *active_layer == Some(layer_id);
                                    let layer_locked = locked_layers.contains(&layer_id);
                                    let layer_name = if layer.dirty { format!("{} *", layer.name) } else { layer.name.clone() };
                                    let layer_label = if layer.is_loaded {
                                        bold(&layer_name)
                                    } else {
                                        bold(&layer_name).color(INACTIVE_TEXT_COLOR)
                                    };
                                    // Named `row` rather than `entry`: `entry` is the
                                    // enclosing project this layer belongs to.
                                    let row = ExplorerEntry::new(egui::Id::new(("explorer_layer", layer_id)), layer_label)
                                        .selected(is_active)
                                        .draggable(rows_draggable)
                                        .toggles(EntryToggles {
                                            visible: layer.is_loaded,
                                            locked: layer_locked,
                                        })
                                        .show(ui);
                                    if row.visibility_clicked {
                                        commands.push(if layer.is_loaded {
                                            UiCommand::UnloadLayer(layer_id)
                                        } else {
                                            UiCommand::LoadLayer(layer_id)
                                        });
                                    }
                                    if row.lock_clicked {
                                        commands.push(UiCommand::ToggleLayerLocked(layer_id));
                                    }
                                    let layer_resp = row.response;
                                    layer_resp.dnd_set_drag_payload(DesignLayerDrag(FolderMember::layer(layer.section, layer_id)));
                                    if layer_resp.dragged() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                    }
                                    // A layer stands for what is on it: clicking
                                    // the row takes every object standing on it,
                                    // and the modifiers run across the rest of the
                                    // tree from here as they do anywhere else.
                                    if layer_resp.clicked() {
                                        commands.push(UiCommand::SelectExplorerRow(ExplorerRow::Layer(layer_id)));
                                    }

                                    context_menu_popup(&layer_resp, layer.name.as_str(), |ui| {
                                        if ContextMenuAction::new(if layer_locked { tr!(literal = "Unlock") } else { tr!(literal = "Lock") })
                                            .show(ui)
                                            .clicked()
                                        {
                                            commands.push(UiCommand::ToggleLayerLocked(layer_id));
                                            ui.close();
                                        }
                                        if layer.is_loaded {
                                            if ContextMenuAction::new(tr!(literal = "Unload")).show(ui).clicked() {
                                                commands.push(UiCommand::UnloadLayer(layer_id));
                                                ui.close();
                                            }
                                            if ContextMenuAction::new(tr!(literal = "Select All Objects")).show(ui).clicked() {
                                                commands.push(UiCommand::SelectAllObjectsInLayer(layer_id));
                                                ui.close();
                                            }
                                        } else if ContextMenuAction::new(tr!(literal = "Load")).show(ui).clicked() {
                                            commands.push(UiCommand::LoadLayer(layer_id));
                                            ui.close();
                                        }
                                        if ContextMenuAction::new(tr!(literal = "Rename")).enabled(!layer_locked).show(ui).clicked() {
                                            commands.push(UiCommand::BeginRenameItem(RenameTarget::Layer(layer_id)));
                                            ui.close();
                                        }
                                        if ContextMenuAction::new(tr!(literal = "Duplicate")).show(ui).clicked() {
                                            commands.push(UiCommand::DuplicateLayer(layer_id));
                                            ui.close();
                                        }
                                        move_to_folder_submenu(ui, design_folders, layer.folder, FolderMember::layer(layer.section, layer_id), commands);
                                        #[cfg(not(target_arch = "wasm32"))]
                                        if layer.dirty
                                            && entry.path.is_some()
                                            && ContextMenuAction::new(tr!(literal = "Discard Changes...")).enabled(!layer_locked).show(ui).clicked()
                                        {
                                            commands.push(UiCommand::RequestDiscardLayerChanges(layer_id));
                                            ui.close();
                                        }
                                        context_menu_separator(ui);
                                        if ContextMenuAction::new(tr!(literal = "Delete from Project")).enabled(!layer_locked).show(ui).clicked() {
                                            commands.push(UiCommand::RequestDeleteLayer(layer_id));
                                            ui.close();
                                        }
                                    });
                                };
                                draw_section_body::<DesignLayerDrag, _>(ui, SectionKind::Designs, design_folders, &entry.layers, commands, layer_row);
                            });
                        let designs_row = designs_header_toggle.union(designs_header.inner);
                        attach_header_drop::<DesignLayerDrag>(ui, &designs_row, SectionKind::Designs, commands);
                        section_heading_menu(
                            &designs_row,
                            ExplorerSection::Designs,
                            project.projects.first().map_or(0, |entry| tagged_count(&entry.layers, SectionKind::Designs)),
                            commands,
                        );

                        let triangulations_dirty = project.triangulations_membership_dirty || project.triangulations.iter().any(|item| item.dirty);
                        let (triangulations_header_toggle, triangulations_header, _) =
                            ExplorerHeader::new(egui::Id::new("triangulations_collapse"), tr!(literal = "Triangulations"))
                                .icon(unthemed_icon!("triangulation.svg"))
                                .color(HEADER_TRIANGULATIONS)
                                .dirty(triangulations_dirty)
                                .show(ui, |ui| {
                                    let triangulation_folders = project.folders.folders(SectionKind::Triangulations);
                                    if tagged_count(&project.triangulations, SectionKind::Triangulations) == 0 && triangulation_folders.is_empty() {
                                        explorer_note(ui, tr!(literal = "No triangulations"));
                                    }
                                    // Helper closure: render one tri entry row and attach its context menu.
                                    let render_tri_entry = |ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, tri: &crate::ui::UiTriangulationEntry| {
                                        rows.push(ExplorerRow::Entity(SceneEntityId::Triangulation(tri.id)));
                                        let source_suffix = tri
                                            .source_name
                                            .as_deref()
                                            .map(|name| tr_format!(literal = "\nSource: %name%", name = name))
                                            .unwrap_or_default();
                                        let tri_path = tr_format!(literal = "ID: triangulation:%id%%source%", id = tri.id.0, source = source_suffix);
                                        let tri_id = tri.id;

                                        let dirty_marker = if tri.dirty { " *" } else { "" };
                                        let stats = format!("{}{}", tri.name, dirty_marker);
                                        let label = if tri.is_loaded { bold(&stats) } else { bold(&stats).color(INACTIVE_TEXT_COLOR) };

                                        let tri_handle = SceneEntityId::Triangulation(tri_id);
                                        let tri_locked = frozen_handles.contains(&tri_handle);
                                        let row = ExplorerEntry::new(egui::Id::new(("explorer_triangulation", tri.id)), label)
                                            .selected(tri.is_active || selected_handles.contains(&SceneEntityId::Triangulation(tri_id)))
                                            .draggable(rows_draggable)
                                            .toggles(EntryToggles {
                                                visible: tri.is_loaded,
                                                locked: tri_locked,
                                            })
                                            .show(ui);
                                        if row.visibility_clicked {
                                            commands.push(if tri.is_loaded {
                                                UiCommand::CloseTriangulation(tri_id)
                                            } else {
                                                UiCommand::LoadTriangulation(tri_id)
                                            });
                                        }
                                        if row.lock_clicked {
                                            commands.push(UiCommand::ToggleEntityLocked(SceneEntityId::Triangulation(tri_id)));
                                        }
                                        let response = row.response.on_hover_text(&tri_path);
                                        response.dnd_set_drag_payload(TriangulationDrag(FolderMember::item(tri.section, ItemRef::Triangulation(tri_id))));
                                        if response.dragged() {
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                        }

                                        if response.clicked() {
                                            commands.push(UiCommand::SelectExplorerRow(ExplorerRow::Entity(tri_handle)));
                                        }

                                        let tri_loaded = tri.is_loaded;
                                        context_menu_popup(&response, tri.name.as_str(), |ui| {
                                            if tri_loaded {
                                                let mut color = crate::rendering::color::rgba_to_color32(tri.color);
                                                if crate::ui::widgets::menu::MenuFieldColor32::new(tr!(literal = "Face colour"), &mut color).show(ui).changed() {
                                                    let targets: Vec<_> = if selected_handles.contains(&SceneEntityId::Triangulation(tri_id)) {
                                                        project
                                                            .triangulations
                                                            .iter()
                                                            .filter(|item| item.is_loaded && selected_handles.contains(&SceneEntityId::Triangulation(item.id)))
                                                            .map(|item| item.id)
                                                            .collect()
                                                    } else {
                                                        vec![tri_id]
                                                    };
                                                    for id in targets {
                                                        commands.push(UiCommand::SetTriangulationColor(id, crate::rendering::color::color32_to_rgba(color)));
                                                    }
                                                }
                                                context_menu_separator(ui);
                                            }
                                            if ContextMenuAction::new(if tri_locked { tr!(literal = "Unlock") } else { tr!(literal = "Lock") })
                                                .show(ui)
                                                .clicked()
                                            {
                                                commands.push(UiCommand::ToggleEntityLocked(SceneEntityId::Triangulation(tri_id)));
                                                ui.close();
                                            }
                                            if tri_loaded {
                                                if ContextMenuAction::new(tr!(literal = "Unload")).show(ui).clicked() {
                                                    commands.push(UiCommand::CloseTriangulation(tri_id));
                                                    ui.close();
                                                }
                                            } else if ContextMenuAction::new(tr!(literal = "Load")).show(ui).clicked() {
                                                commands.push(UiCommand::LoadTriangulation(tri_id));
                                                ui.close();
                                            }
                                            #[cfg(target_arch = "wasm32")]
                                            if ContextMenuAction::new(tr!(literal = "Download")).show(ui).clicked() {
                                                commands.push(UiCommand::ExportTriangulationAs(tri_id, crate::model::formats::MeshFormat::Obj));
                                                ui.close();
                                            }
                                            if ContextMenuAction::new(tr!(literal = "Rename")).enabled(!tri_locked).show(ui).clicked() {
                                                commands.push(UiCommand::BeginRenameItem(RenameTarget::Triangulation(tri_id)));
                                                ui.close();
                                            }
                                            move_to_folder_submenu(
                                                ui,
                                                triangulation_folders,
                                                tri.folder,
                                                FolderMember::item(tri.section, ItemRef::Triangulation(tri_id)),
                                                commands,
                                            );
                                            context_menu_separator(ui);
                                            if ContextMenuAction::new(tr!(literal = "Delete from Project")).enabled(!tri_locked).show(ui).clicked() {
                                                commands.push(UiCommand::RequestDeleteItem(RenameTarget::Triangulation(tri_id)));
                                                ui.close();
                                            }
                                        });
                                    };

                                    draw_section_body::<TriangulationDrag, _>(
                                        ui,
                                        SectionKind::Triangulations,
                                        triangulation_folders,
                                        &project.triangulations,
                                        commands,
                                        render_tri_entry,
                                    );
                                });
                        let triangulations_row = triangulations_header_toggle.union(triangulations_header.inner);
                        attach_header_drop::<TriangulationDrag>(ui, &triangulations_row, SectionKind::Triangulations, commands);
                        section_heading_menu(
                            &triangulations_row,
                            ExplorerSection::Triangulations,
                            tagged_count(&project.triangulations, SectionKind::Triangulations),
                            commands,
                        );

                        let rasters_dirty = project.rasters_membership_dirty || project.raster_textures.iter().any(|item| item.dirty);
                        let (rasters_header_toggle, rasters_header, _) = ExplorerHeader::new("rasters_collapse".into(), tr!(literal = "Rasters"))
                            .icon(unthemed_icon!("raster.svg"))
                            .color(HEADER_RASTERS)
                            .dirty(rasters_dirty)
                            .show(ui, |ui| {
                                let raster_folders = project.folders.folders(SectionKind::Rasters);
                                if tagged_count(&project.raster_textures, SectionKind::Rasters) == 0 && raster_folders.is_empty() {
                                    explorer_note(ui, tr!("explorer-no-rasters"));
                                }
                                let render_raster_entry = |ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, raster: &UiRasterTextureEntry| {
                                    let raster_label = if raster.dirty { format!("{} *", raster.name) } else { raster.name.clone() };
                                    let label = if raster.is_loaded {
                                        bold(&raster_label)
                                    } else {
                                        bold(&raster_label).color(INACTIVE_TEXT_COLOR)
                                    };
                                    let source_suffix = raster
                                        .source_name
                                        .as_deref()
                                        .map(|name| tr_format!(literal = "\nSource: %name%", name = name))
                                        .unwrap_or_default();
                                    let details = tr_format!(
                                        literal = "ID: raster:%id%%source%\n%driver% · %width% × %height%\n%projection%",
                                        id = raster.id.0,
                                        source = source_suffix,
                                        driver = raster.driver_name,
                                        width = raster.source_size[0],
                                        height = raster.source_size[1],
                                        projection = raster.projection
                                    );
                                    let details = if raster.is_draped {
                                        format!("{details}\n{}", tr!(literal = "Draped over a surface"))
                                    } else {
                                        details
                                    };
                                    let raster_handle = SceneEntityId::Raster(raster.id);
                                    rows.push(ExplorerRow::Entity(raster_handle));
                                    let raster_locked = locked_rasters.contains(&raster.id);
                                    let row = ExplorerEntry::new(egui::Id::new(("explorer_raster", raster.id)), label)
                                        .selected(selected_handles.contains(&raster_handle))
                                        .draggable(rows_draggable)
                                        .toggles(EntryToggles {
                                            visible: raster.is_loaded,
                                            locked: raster_locked,
                                        })
                                        .show(ui);
                                    if row.visibility_clicked {
                                        commands.push(if raster.is_loaded {
                                            UiCommand::UnloadRaster(raster.id)
                                        } else {
                                            UiCommand::LoadRaster(raster.id)
                                        });
                                    }
                                    if row.lock_clicked {
                                        commands.push(UiCommand::ToggleRasterLocked(raster.id));
                                    }
                                    let response = row.response.on_hover_text(&details);
                                    response.dnd_set_drag_payload(RasterDrag(FolderMember::item(raster.section, ItemRef::Raster(raster.id))));
                                    if response.dragged() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                    }
                                    if response.clicked() {
                                        commands.push(UiCommand::SelectExplorerRow(ExplorerRow::Entity(raster_handle)));
                                    }

                                    context_menu_popup(&response, raster.name.as_str(), |ui| {
                                        if ContextMenuAction::new(if raster_locked { tr!(literal = "Unlock") } else { tr!(literal = "Lock") })
                                            .show(ui)
                                            .clicked()
                                        {
                                            commands.push(UiCommand::ToggleRasterLocked(raster.id));
                                            ui.close();
                                        }
                                        if raster.is_loaded {
                                            if ContextMenuAction::new(tr!(literal = "Unload")).show(ui).clicked() {
                                                commands.push(UiCommand::UnloadRaster(raster.id));
                                                ui.close();
                                            }
                                            if ContextMenuAction::new(tr!(literal = "Drape Over Surface")).enabled(!raster_locked).show(ui).clicked() {
                                                commands.push(UiCommand::DrapeRaster(raster.id));
                                                ui.close();
                                            }
                                        } else if ContextMenuAction::new(tr!(literal = "Load")).show(ui).clicked() {
                                            commands.push(UiCommand::LoadRaster(raster.id));
                                            ui.close();
                                        }
                                        // Unloading a raster keeps its drape, so offer the undrape in both states.
                                        if raster.is_draped && ContextMenuAction::new(tr!(literal = "Undrape All")).enabled(!raster_locked).show(ui).clicked() {
                                            commands.push(UiCommand::UndrapeRaster(raster.id));
                                            ui.close();
                                        }
                                        if project.active_triangulation_for_menu.is_some()
                                            && ContextMenuAction::new(tr!(literal = "Clear Active Triangulation Texture")).show(ui).clicked()
                                        {
                                            commands.push(UiCommand::ClearActiveTriangulationRaster);
                                            ui.close();
                                        }
                                        if ContextMenuAction::new(tr!(literal = "Rename")).enabled(!raster_locked).show(ui).clicked() {
                                            commands.push(UiCommand::BeginRenameItem(RenameTarget::Raster(raster.id)));
                                            ui.close();
                                        }
                                        move_to_folder_submenu(ui, raster_folders, raster.folder, FolderMember::item(raster.section, ItemRef::Raster(raster.id)), commands);
                                        context_menu_separator(ui);
                                        if ContextMenuAction::new(tr!(literal = "Delete from Project")).enabled(!raster_locked).show(ui).clicked() {
                                            commands.push(UiCommand::RequestDeleteItem(RenameTarget::Raster(raster.id)));
                                            ui.close();
                                        }
                                    });
                                };
                                draw_section_body::<RasterDrag, _>(ui, SectionKind::Rasters, raster_folders, &project.raster_textures, commands, render_raster_entry);
                            });
                        let rasters_row = rasters_header_toggle.union(rasters_header.inner);
                        attach_header_drop::<RasterDrag>(ui, &rasters_row, SectionKind::Rasters, commands);
                        section_heading_menu(
                            &rasters_row,
                            ExplorerSection::Rasters,
                            tagged_count(&project.raster_textures, SectionKind::Rasters),
                            commands,
                        );

                        let point_clouds_dirty = project.point_clouds_membership_dirty || project.point_clouds.iter().any(|item| item.dirty);
                        let (point_clouds_header_toggle, point_clouds_header, _) = ExplorerHeader::new(egui::Id::new("point_clouds_collapse"), tr!(literal = "Point Clouds"))
                            .icon(unthemed_icon!("section_point_clouds.svg"))
                            .color(HEADER_POINT_CLOUDS)
                            .dirty(point_clouds_dirty)
                            .show(ui, |ui| {
                                let point_cloud_folders = project.folders.folders(SectionKind::PointClouds);
                                if tagged_count(&project.point_clouds, SectionKind::PointClouds) == 0 && point_cloud_folders.is_empty() {
                                    explorer_note(ui, tr!(literal = "No point clouds"));
                                }
                                let render_point_cloud_entry = |ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, point_cloud: &UiPointCloudEntry| {
                                    let dirty_marker = if point_cloud.dirty { " *" } else { "" };
                                    let label_text = format!("{}{dirty_marker}", point_cloud.name);
                                    let label = if point_cloud.is_loaded {
                                        bold(&label_text)
                                    } else {
                                        bold(&label_text).color(INACTIVE_TEXT_COLOR)
                                    };
                                    let source_suffix = point_cloud
                                        .source_name
                                        .as_deref()
                                        .map(|name| tr_format!(literal = "\nSource: %name%", name = name))
                                        .unwrap_or_default();
                                    let tooltip = tr_format!(
                                        literal = "ID: point-cloud:%id%%source%\n%count% point(s)",
                                        id = point_cloud.id.0,
                                        source = source_suffix,
                                        count = point_cloud.point_count
                                    );
                                    let cloud_locked = frozen_handles.contains(&SceneEntityId::PointCloud(point_cloud.id));
                                    let cloud_handle = SceneEntityId::PointCloud(point_cloud.id);
                                    rows.push(ExplorerRow::Entity(cloud_handle));
                                    let row = ExplorerEntry::new(egui::Id::new(("explorer_point_cloud", point_cloud.id)), label)
                                        .draggable(rows_draggable)
                                        .selected(selected_handles.contains(&cloud_handle))
                                        .toggles(EntryToggles {
                                            visible: point_cloud.is_loaded,
                                            locked: cloud_locked,
                                        })
                                        .show(ui);
                                    if row.visibility_clicked {
                                        commands.push(if point_cloud.is_loaded {
                                            UiCommand::ClosePointCloud(point_cloud.id)
                                        } else {
                                            UiCommand::LoadPointCloud(point_cloud.id)
                                        });
                                    }
                                    if row.lock_clicked {
                                        commands.push(UiCommand::ToggleEntityLocked(SceneEntityId::PointCloud(point_cloud.id)));
                                    }
                                    let response = row.response.on_hover_text(&tooltip);
                                    response.dnd_set_drag_payload(PointCloudDrag(FolderMember::item(point_cloud.section, ItemRef::PointCloud(point_cloud.id))));
                                    if response.dragged() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                    }
                                    // A cloud has no handles to click in the
                                    // viewport at this zoom, so the tree is the
                                    // practical way to select one for the tools
                                    // that run on a selected cloud.
                                    if response.clicked() {
                                        commands.push(UiCommand::SelectExplorerRow(ExplorerRow::Entity(cloud_handle)));
                                    }

                                    context_menu_popup(&response, point_cloud.name.as_str(), |ui| {
                                        if ContextMenuAction::new(if cloud_locked { tr!(literal = "Unlock") } else { tr!(literal = "Lock") })
                                            .show(ui)
                                            .clicked()
                                        {
                                            commands.push(UiCommand::ToggleEntityLocked(SceneEntityId::PointCloud(point_cloud.id)));
                                            ui.close();
                                        }
                                        if point_cloud.is_loaded {
                                            if ContextMenuAction::new(tr!(literal = "Unload")).show(ui).clicked() {
                                                commands.push(UiCommand::ClosePointCloud(point_cloud.id));
                                                ui.close();
                                            }
                                        } else if ContextMenuAction::new(tr!(literal = "Load")).show(ui).clicked() {
                                            commands.push(UiCommand::LoadPointCloud(point_cloud.id));
                                            ui.close();
                                        }
                                        if ContextMenuAction::new(tr!(literal = "Rename")).enabled(!cloud_locked).show(ui).clicked() {
                                            commands.push(UiCommand::BeginRenameItem(RenameTarget::PointCloud(point_cloud.id)));
                                            ui.close();
                                        }
                                        move_to_folder_submenu(
                                            ui,
                                            point_cloud_folders,
                                            point_cloud.folder,
                                            FolderMember::item(point_cloud.section, ItemRef::PointCloud(point_cloud.id)),
                                            commands,
                                        );
                                        context_menu_separator(ui);
                                        if ContextMenuAction::new(tr!(literal = "Delete from Project")).enabled(!cloud_locked).show(ui).clicked() {
                                            commands.push(UiCommand::RequestDeleteItem(RenameTarget::PointCloud(point_cloud.id)));
                                            ui.close();
                                        }
                                    });
                                };
                                draw_section_body::<PointCloudDrag, _>(
                                    ui,
                                    SectionKind::PointClouds,
                                    point_cloud_folders,
                                    &project.point_clouds,
                                    commands,
                                    render_point_cloud_entry,
                                );
                            });
                        let point_clouds_row = point_clouds_header_toggle.union(point_clouds_header.inner);
                        attach_header_drop::<PointCloudDrag>(ui, &point_clouds_row, SectionKind::PointClouds, commands);
                        section_heading_menu(
                            &point_clouds_row,
                            ExplorerSection::PointClouds,
                            tagged_count(&project.point_clouds, SectionKind::PointClouds),
                            commands,
                        );

                        let block_models_dirty = project.block_models_membership_dirty || project.block_models.iter().any(|item| item.dirty);
                        let (block_models_header_toggle, block_models_header, _) = ExplorerHeader::new(egui::Id::new("block_models_collapse"), tr!(literal = "Block Models"))
                            .icon(unthemed_icon!("section_block_models.svg"))
                            .color(HEADER_BLOCK_MODELS)
                            .dirty(block_models_dirty)
                            .show(ui, |ui| {
                                let block_model_folders = project.folders.folders(SectionKind::BlockModels);
                                if tagged_count(&project.block_models, SectionKind::BlockModels) == 0 && block_model_folders.is_empty() {
                                    explorer_note(ui, tr!(literal = "No block models"));
                                }
                                let render_block_model_entry = |ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, block_model: &UiBlockModelEntry| {
                                    let block_model_handle = SceneEntityId::BlockModel(block_model.id);
                                    rows.push(ExplorerRow::Entity(block_model_handle));
                                    let is_selected = selected_handles.contains(&block_model_handle);
                                    let dirty_marker = if block_model.dirty { " *" } else { "" };
                                    let label_text = format!("{}{dirty_marker}", block_model.name);
                                    let label = if block_model.is_loaded {
                                        bold(&label_text)
                                    } else {
                                        bold(&label_text).color(INACTIVE_TEXT_COLOR)
                                    };
                                    let model_locked = frozen_handles.contains(&SceneEntityId::BlockModel(block_model.id));
                                    let row = ExplorerEntry::new(egui::Id::new(("explorer_block_model", block_model.id)), label)
                                        .selected(is_selected)
                                        .draggable(rows_draggable)
                                        .toggles(EntryToggles {
                                            visible: block_model.is_loaded,
                                            locked: model_locked,
                                        })
                                        .show(ui);
                                    if row.visibility_clicked {
                                        commands.push(if block_model.is_loaded {
                                            UiCommand::CloseBlockModel(block_model.id)
                                        } else {
                                            UiCommand::LoadBlockModel(block_model.id)
                                        });
                                    }
                                    if row.lock_clicked {
                                        commands.push(UiCommand::ToggleEntityLocked(SceneEntityId::BlockModel(block_model.id)));
                                    }
                                    let block_model_source_suffix = block_model
                                        .source_name
                                        .as_deref()
                                        .map(|name| tr_format!(literal = "\nSource: %name%", name = name))
                                        .unwrap_or_default();
                                    let response = row.response.on_hover_text(tr_format!(
                                        literal = "ID: block-model:%id%%source%\n%count% colour variable(s)",
                                        id = block_model.id.0,
                                        source = block_model_source_suffix,
                                        count = block_model.variable_count
                                    ));
                                    response.dnd_set_drag_payload(BlockModelDrag(FolderMember::item(block_model.section, ItemRef::BlockModel(block_model.id))));
                                    if response.dragged() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                    }
                                    if response.clicked() {
                                        // Selecting here is what reveals the model's
                                        // viewport controls, the same as picking it in
                                        // the viewport does.
                                        commands.push(UiCommand::SelectExplorerRow(ExplorerRow::Entity(block_model_handle)));
                                    }

                                    context_menu_popup(&response, block_model.name.as_str(), |ui| {
                                        if ContextMenuAction::new(if model_locked { tr!(literal = "Unlock") } else { tr!(literal = "Lock") })
                                            .show(ui)
                                            .clicked()
                                        {
                                            commands.push(UiCommand::ToggleEntityLocked(SceneEntityId::BlockModel(block_model.id)));
                                            ui.close();
                                        }
                                        if block_model.is_loaded {
                                            if ContextMenuAction::new(tr!(literal = "Unload")).show(ui).clicked() {
                                                commands.push(UiCommand::CloseBlockModel(block_model.id));
                                                ui.close();
                                            }
                                        } else if ContextMenuAction::new(tr!(literal = "Load")).show(ui).clicked() {
                                            commands.push(UiCommand::LoadBlockModel(block_model.id));
                                            ui.close();
                                        }
                                        if ContextMenuAction::new(tr!(literal = "Rename")).enabled(!model_locked).show(ui).clicked() {
                                            commands.push(UiCommand::BeginRenameItem(RenameTarget::BlockModel(block_model.id)));
                                            ui.close();
                                        }
                                        move_to_folder_submenu(
                                            ui,
                                            block_model_folders,
                                            block_model.folder,
                                            FolderMember::item(block_model.section, ItemRef::BlockModel(block_model.id)),
                                            commands,
                                        );
                                        context_menu_separator(ui);
                                        if ContextMenuAction::new(tr!(literal = "Delete from Project")).enabled(!model_locked).show(ui).clicked() {
                                            commands.push(UiCommand::RequestDeleteItem(RenameTarget::BlockModel(block_model.id)));
                                            ui.close();
                                        }
                                    });
                                };
                                draw_section_body::<BlockModelDrag, _>(
                                    ui,
                                    SectionKind::BlockModels,
                                    block_model_folders,
                                    &project.block_models,
                                    commands,
                                    render_block_model_entry,
                                );
                            });
                        let block_models_row = block_models_header_toggle.union(block_models_header.inner);
                        attach_header_drop::<BlockModelDrag>(ui, &block_models_row, SectionKind::BlockModels, commands);
                        section_heading_menu(
                            &block_models_row,
                            ExplorerSection::BlockModels,
                            tagged_count(&project.block_models, SectionKind::BlockModels),
                            commands,
                        );

                        let drill_holes_dirty = project.drill_holes_membership_dirty || project.drill_holes.iter().any(|item| item.dirty);
                        let (drill_holes_header_toggle, drill_holes_header, _) = ExplorerHeader::new(egui::Id::new("drill_holes_collapse"), tr!(literal = "Drill Holes"))
                            .icon(unthemed_icon!("drill_hole.svg"))
                            .color(HEADER_DRILL_HOLES)
                            .dirty(drill_holes_dirty)
                            .show(ui, |ui| {
                                let drill_hole_folders = project.folders.folders(SectionKind::DrillHoles);
                                if tagged_count(&project.drill_holes, SectionKind::DrillHoles) == 0 && drill_hole_folders.is_empty() {
                                    explorer_note(ui, tr!(literal = "No drill holes"));
                                }
                                let render_drill_hole_entry = |ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, dataset: &UiDrillHoleEntry| {
                                    let dataset_label = if dataset.dirty { format!("{} *", dataset.name) } else { dataset.name.clone() };
                                    let label = if dataset.is_loaded {
                                        bold(&dataset_label)
                                    } else {
                                        bold(&dataset_label).color(INACTIVE_TEXT_COLOR)
                                    };
                                    let source_suffix = dataset
                                        .source_name
                                        .as_deref()
                                        .map(|name| tr_format!(literal = "\nSource: %name%", name = name))
                                        .unwrap_or_default();
                                    let tooltip = tr_format!(
                                        literal = "ID: drill-holes:%id%%source%\n%holes% hole(s)\n%fields% colour field(s)",
                                        id = dataset.id.0,
                                        source = source_suffix,
                                        holes = dataset.hole_count,
                                        fields = dataset.field_count
                                    );
                                    let dataset_handle = SceneEntityId::DrillHole(dataset.id);
                                    rows.push(ExplorerRow::Entity(dataset_handle));
                                    let dataset_locked = frozen_handles.contains(&dataset_handle);
                                    let row = ExplorerEntry::new(egui::Id::new(("explorer_drill_hole", dataset.id)), label)
                                        .draggable(rows_draggable)
                                        .selected(selected_handles.contains(&dataset_handle))
                                        .toggles(EntryToggles {
                                            visible: dataset.is_loaded,
                                            locked: dataset_locked,
                                        })
                                        .show(ui);
                                    if row.visibility_clicked {
                                        commands.push(if dataset.is_loaded {
                                            UiCommand::CloseDrillHole(dataset.id)
                                        } else {
                                            UiCommand::LoadDrillHole(dataset.id)
                                        });
                                    }
                                    if row.lock_clicked {
                                        commands.push(UiCommand::ToggleEntityLocked(SceneEntityId::DrillHole(dataset.id)));
                                    }
                                    let response = row.response.on_hover_text(&tooltip);
                                    response.dnd_set_drag_payload(DrillHoleDrag(FolderMember::item(dataset.section, ItemRef::DrillHole(dataset.id))));
                                    if response.dragged() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                    }
                                    if response.clicked() {
                                        commands.push(UiCommand::SelectExplorerRow(ExplorerRow::Entity(dataset_handle)));
                                    }

                                    context_menu_popup(&response, dataset.name.as_str(), |ui| {
                                        if ContextMenuAction::new(if dataset_locked { tr!(literal = "Unlock") } else { tr!(literal = "Lock") })
                                            .show(ui)
                                            .clicked()
                                        {
                                            commands.push(UiCommand::ToggleEntityLocked(SceneEntityId::DrillHole(dataset.id)));
                                            ui.close();
                                        }
                                        if dataset.is_loaded {
                                            if ContextMenuAction::new(tr!(literal = "Unload")).show(ui).clicked() {
                                                commands.push(UiCommand::CloseDrillHole(dataset.id));
                                                ui.close();
                                            }
                                            if ContextMenuAction::new(tr!(literal = "Colour by...")).show(ui).clicked() {
                                                commands.push(UiCommand::OpenDrillHoleColorDialog(dataset.id));
                                                ui.close();
                                            }
                                        } else if ContextMenuAction::new(tr!(literal = "Load")).show(ui).clicked() {
                                            commands.push(UiCommand::LoadDrillHole(dataset.id));
                                            ui.close();
                                        }
                                        if ContextMenuAction::new(tr!(literal = "Rename")).enabled(!dataset_locked).show(ui).clicked() {
                                            commands.push(UiCommand::BeginRenameItem(RenameTarget::DrillHole(dataset.id)));
                                            ui.close();
                                        }
                                        move_to_folder_submenu(
                                            ui,
                                            drill_hole_folders,
                                            dataset.folder,
                                            FolderMember::item(dataset.section, ItemRef::DrillHole(dataset.id)),
                                            commands,
                                        );
                                        context_menu_separator(ui);
                                        if ContextMenuAction::new(tr!(literal = "Delete from Project")).enabled(!dataset_locked).show(ui).clicked() {
                                            commands.push(UiCommand::RequestDeleteItem(RenameTarget::DrillHole(dataset.id)));
                                            ui.close();
                                        }
                                    });
                                };
                                draw_section_body::<DrillHoleDrag, _>(ui, SectionKind::DrillHoles, drill_hole_folders, &project.drill_holes, commands, render_drill_hole_entry);
                            });
                        let drill_holes_row = drill_holes_header_toggle.union(drill_holes_header.inner);
                        attach_header_drop::<DrillHoleDrag>(ui, &drill_holes_row, SectionKind::DrillHoles, commands);
                        section_heading_menu(
                            &drill_holes_row,
                            ExplorerSection::DrillHoles,
                            tagged_count(&project.drill_holes, SectionKind::DrillHoles),
                            commands,
                        );

                        paint_fixed_stripes(ui, stripes_slot, stripes_top, stripe);
                    });
                })
                .response
                .rect;
            editor.explorer_rows = rows;
            (tree, products)
        });

    let (tree, products) = column.inner;
    ExplorerLayout {
        column: column.response.rect,
        tree,
        products,
    }
}
