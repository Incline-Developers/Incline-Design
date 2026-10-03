//! The Blasting step's panel: the blasts of whichever benches are selected.
//!
//! The shapes themselves are derived geometry, drawn over the scene. What
//! this panel is for is reading off and correcting their names, which is the
//! only part of a blast the user owns.

use crate::{
    i18n::tr,
    model::Document,
    ui::{
        EditorState,
        state::{BlastShapeRef, RenameTarget, UiCommand},
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{DataGrid, grid_columns_row, grid_empty_state, grid_group_row},
            island::{Island, IslandResponse, Side},
        },
    },
};

const PANEL_ID: &str = "planning_blasting_panel";
/// Shares of the grid's columns: the blast's name, then its plan area.
const FRACTIONS: [f32; 2] = [0.5, 0.5];

pub(crate) fn draw_panel(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) -> IslandResponse<()> {
    Island::new(PANEL_ID, Side::Right)
        .default_width(260.0)
        .min_width(200.0)
        .max_width(420.0)
        .flush()
        .show(ui, |ui, rect| {
            let columns = [(tr!("solids-blast"), FRACTIONS[0]), (tr!("drill-blast-area"), FRACTIONS[1])];
            DataGrid::new("planning_blasts_grid", rect, &tr!("planning-blasts")).columns(&columns).show(ui, |ui| {
                if editor.blasting_outlines.is_empty() {
                    grid_empty_state(ui, &tr!("planning-blasts-empty"), None);
                    return;
                }
                let mut last_solid = None;
                let mut last_bench = None;
                let (mut solid_open, mut bench_open) = (true, true);
                for outline in &editor.blasting_outlines {
                    // Benches nest under their solid, and blasts under their
                    // bench: numbering restarts per bench, so a blast only
                    // reads unambiguously under the RL it belongs to.
                    let count = |keep: &dyn Fn(&crate::ui::state::BlastOutline) -> bool| editor.blasting_outlines.iter().filter(|other| keep(other)).count();
                    if last_solid != Some(outline.solid) {
                        last_solid = Some(outline.solid);
                        last_bench = None;
                        let name = document.solid(outline.solid).map_or("", |solid| solid.name.as_str());
                        let blasts = count(&|other| other.solid == outline.solid);
                        solid_open = grid_group_row(ui, ("planning_blasts_solid", outline.solid), name, &tr!("planning-blast-count", count = blasts), 0);
                    }
                    if !solid_open {
                        continue;
                    }
                    let bench = outline.bench_base.to_bits();
                    if last_bench != Some(bench) {
                        last_bench = Some(bench);
                        let blasts = count(&|other| other.solid == outline.solid && other.bench_base.to_bits() == bench);
                        bench_open = grid_group_row(
                            ui,
                            ("planning_blasts_bench", outline.solid, bench),
                            &super::solids_view::format_rl(outline.bench_base),
                            &tr!("planning-blast-count", count = blasts),
                            1,
                        );
                    }
                    if !bench_open {
                        continue;
                    }
                    let blast = BlastShapeRef::new(outline.solid, outline.bench_base, anchor_of(outline));
                    let area = super::solids_view::format_area(outline.area);
                    let (response, _) = grid_columns_row(ui, &FRACTIONS, &[&outline.name, &area], editor.selected_blast == Some(blast));
                    if editor.scroll_to_blast && editor.selected_blast == Some(blast) {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                    if response.clicked() {
                        commands.push(UiCommand::SelectBlast(Some(blast)));
                    }
                    if response.double_clicked() {
                        commands.push(UiCommand::BeginRenameItem(RenameTarget::BlastShape(blast)));
                    }
                    response.clone().on_hover_text(tr!("planning-double-click-rename"));
                    context_menu_popup(&response, &outline.name, |ui| {
                        if ContextMenuAction::new(tr!("planning-rename-blast")).show(ui).clicked() {
                            commands.push(UiCommand::BeginRenameItem(RenameTarget::BlastShape(blast)));
                            ui.close();
                        }
                        if ContextMenuAction::new(tr!("planning-reset-blast-name")).show(ui).clicked() {
                            commands.push(UiCommand::ResetBlastName(blast));
                            ui.close();
                        }
                    });
                }
            });
            editor.scroll_to_blast = false;
        })
}

/// The anchor the derivation stored for this outline. Recomputing it here
/// would risk disagreeing with the stored one over a rounding step, so the
/// outline carries the point it was matched on.
fn anchor_of(outline: &crate::ui::state::BlastOutline) -> [f64; 2] {
    outline.anchor
}
