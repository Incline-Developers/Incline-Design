//! The Dig Strips step's panel: the dig blocks of the selected flitch, under
//! the blast each lies in.
//!
//! Strips are copied and pasted between flitches with Ctrl+C and Ctrl+V,
//! handled with the rest of the keyboard in `app/events.rs`.

use crate::{
    i18n::tr,
    ui::{
        EditorState,
        state::{BlastShapeRef, UiCommand},
        widgets::{
            context_menu::context_menu_popup,
            data_grid::{DataGrid, grid_cell_entry, grid_columns_row, grid_empty_state, grid_group_row},
            island::{Island, IslandResponse, Side},
        },
    },
};

const PANEL_ID: &str = "planning_dig_blocks";
/// Shares of the grid's columns: the block's number, then its plan area.
const FRACTIONS: [f32; 2] = [0.5, 0.5];

pub(crate) fn draw_panel(ui: &mut egui::Ui, editor: &mut EditorState, document: &crate::model::Document, commands: &mut Vec<UiCommand>) -> IslandResponse<()> {
    Island::new(PANEL_ID, Side::Right)
        .default_width(260.0)
        .min_width(200.0)
        .max_width(420.0)
        .flush()
        .show(ui, |ui, rect| {
            let columns = [(tr!("planning-dig-block"), FRACTIONS[0]), (tr!("drill-blast-area"), FRACTIONS[1])];
            DataGrid::new("planning_dig_blocks_grid", rect, &tr!("planning-dig-blocks"))
                .columns(&columns)
                .show(ui, |ui| {
                    // The minimum strip of the solid the step is on, above its
                    // blocks, as the Blasts panel carries the minimum blast.
                    if let Some(solid) = editor.solids_view_selection.first().and_then(|row| document.solid(row.solid)) {
                        let (response, cells) = grid_columns_row(ui, &FRACTIONS, &[&tr!("planning-min-strip-area"), ""], false);
                        response.on_hover_text(tr!("planning-min-strip-area-help"));
                        let mut area = solid.exclusions.min_strip_area;
                        if grid_cell_entry(ui, ("planning_min_strip_area", solid.id), cells[1], &mut area, "m²") {
                            commands.push(UiCommand::UpdateSolid {
                                solid: solid.id,
                                edit: crate::model::SolidEdit::Exclusions(crate::model::MiningExclusions {
                                    min_strip_area: area.max(0.0),
                                    ..solid.exclusions.clone()
                                }),
                            });
                        }
                    }
                    if editor.dig_outlines.is_empty() {
                        grid_empty_state(ui, &tr!("planning-dig-blocks-empty"), None);
                        return;
                    }
                    let mut last_blast: Option<Option<&str>> = None;
                    let mut open = true;
                    for block in &editor.dig_outlines {
                        // Block numbers run across the flitch, so the blast a
                        // block lies in is what places it, as its ground path does.
                        let blast = block.blast.as_deref();
                        if last_blast != Some(blast) {
                            last_blast = Some(blast);
                            let count = editor
                                .dig_outlines
                                .iter()
                                .filter(|other| other.solid == block.solid && other.blast.as_deref() == blast)
                                .count();
                            let label = blast.map_or_else(|| tr!("schedule-unblasted"), |name| tr!("planning-dig-blast-group", name = name.to_owned()));
                            open = grid_group_row(
                                ui,
                                ("planning_dig_blast", block.solid, block.bench_base.to_bits(), blast),
                                &label,
                                &tr!("planning-dig-block-count", count = count),
                                0,
                            );
                        }
                        if !open {
                            continue;
                        }
                        let id = BlastShapeRef::new(block.solid, block.bench_base, block.anchor);
                        let name = if block.excluded {
                            tr!("planning-excluded-label", name = block.name.clone())
                        } else {
                            block.name.clone()
                        };
                        let area = super::solids_view::format_area(block.area);
                        let (response, _) = grid_columns_row(ui, &FRACTIONS, &[&name, &area], editor.selected_dig_block == Some(id));
                        if response.clicked() {
                            commands.push(UiCommand::SelectDigBlock(id));
                        }
                        context_menu_popup(&response, &name, |ui| {
                            if super::blasting::outline_menu(ui, block, super::blasting::OutlineKind::DigBlock, document, commands) {
                                ui.close();
                            }
                        });
                    }
                });
        })
}
