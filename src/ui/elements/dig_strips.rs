//! The dig-block list and flitch-to-flitch strip clipboard.
use crate::{
    i18n::tr,
    ui::{
        EditorState,
        state::{BlastShapeRef, UiCommand},
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{GridRow, grid_row},
            explorer::explorer_note,
            island::{Island, IslandResponse, Side},
        },
    },
};

const PANEL_ID: &str = "planning_dig_blocks";

pub(crate) fn draw_panel(ui: &mut egui::Ui, editor: &mut EditorState, document: &crate::model::Document, commands: &mut Vec<UiCommand>) -> IslandResponse<()> {
    Island::new(PANEL_ID, Side::Right)
        .fill(crate::ui::widgets::tree_row_colors(ui).0)
        .default_width(260.0)
        .min_width(200.0)
        .max_width(420.0)
        .show(ui, |ui, _| {
            ui.label(crate::ui::fonts::bold(&tr!("planning-dig-blocks")));
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!editor.selected_handles.is_empty(), egui::Button::new(tr!("planning-dig-copy")))
                    .on_hover_text(tr!("planning-shortcut-copy"))
                    .clicked()
                {
                    commands.push(UiCommand::CopyDigStrips);
                }
                if ui
                    .add_enabled(
                        !editor.dig_clipboard.is_empty() && editor.planning_cut_target().is_some(),
                        egui::Button::new(tr!("planning-dig-paste")),
                    )
                    .on_hover_text(tr!("planning-shortcut-paste"))
                    .clicked()
                {
                    commands.push(UiCommand::PasteDigStrips);
                }
            });
            ui.separator();
            if editor.planning_cut_target().is_none() {
                explorer_note(ui, tr!("planning-dig-select-flitch"));
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for block in &editor.dig_outlines {
                    let id = BlastShapeRef::new(block.solid, block.bench_base, block.anchor);
                    // A dig outline's base is its flitch's.
                    let target = crate::model::ExclusionTarget::Block {
                        flitch: block.bench_base,
                        anchor: block.anchor,
                    };
                    let solid = document.solids().iter().find(|solid| solid.id == block.solid);
                    let excluded = solid.is_some_and(|solid| solid.exclusions.is_target_excluded(target));
                    let label = if excluded {
                        tr!("planning-dig-block-excluded", name = block.name.clone(), area = format!("{:.0}", block.area))
                    } else {
                        tr!("planning-dig-block-row", name = block.name.clone(), area = format!("{:.0}", block.area))
                    };
                    let response = grid_row(ui, GridRow::new(&label).selected(editor.selected_dig_block == Some(id)));
                    if response.clicked() {
                        commands.push(UiCommand::SelectDigBlock(id));
                    }
                    if let Some(solid) = solid {
                        context_menu_popup(&response, &label, |ui| {
                            if ContextMenuAction::new(tr!("planning-exclude-from-mining")).checked(excluded).show(ui).clicked() {
                                commands.push(UiCommand::UpdateSolid {
                                    solid: solid.id,
                                    edit: crate::model::SolidEdit::Exclusions(solid.exclusions.with(target, !excluded)),
                                });
                                ui.close();
                            }
                        });
                    }
                }
            });
        })
}
