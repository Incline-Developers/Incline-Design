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
            data_grid::{DataGrid, grid_cell_entry, grid_columns_row, grid_empty_state, grid_group_row},
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
                // A blast under the solid's minimum size is out of mining
                // and unnamed: drawn, but not one of the blasts listed.
                for outline in editor.blasting_outlines.iter().filter(|outline| !outline.name.is_empty()) {
                    // Benches nest under their solid, and blasts under their
                    // bench: numbering restarts per bench, so a blast only
                    // reads unambiguously under the RL it belongs to.
                    let count =
                        |keep: &dyn Fn(&crate::ui::state::BlastOutline) -> bool| editor.blasting_outlines.iter().filter(|other| !other.name.is_empty() && keep(other)).count();
                    if last_solid != Some(outline.solid) {
                        last_solid = Some(outline.solid);
                        last_bench = None;
                        let name = document.solid(outline.solid).map_or("", |solid| solid.name.as_str());
                        let blasts = count(&|other| other.solid == outline.solid);
                        solid_open = grid_group_row(ui, ("planning_blasts_solid", outline.solid), name, &tr!("planning-blast-count", count = blasts), 0);
                        if solid_open && let Some(solid) = document.solid(outline.solid) {
                            let (response, cells) = grid_columns_row(ui, &FRACTIONS, &[&tr!("planning-min-blast-area"), ""], false);
                            response.on_hover_text(tr!("planning-min-blast-area-help"));
                            let mut area = solid.exclusions.min_blast_area;
                            if grid_cell_entry(ui, ("planning_min_blast_area", outline.solid), cells[1], &mut area, "m²") {
                                commands.push(UiCommand::UpdateSolid {
                                    solid: solid.id,
                                    edit: crate::model::SolidEdit::Exclusions(crate::model::MiningExclusions {
                                        min_blast_area: area.max(0.0),
                                        ..solid.exclusions.clone()
                                    }),
                                });
                            }
                        }
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
                    let name = if outline.excluded {
                        tr!("planning-excluded-label", name = outline.name.clone())
                    } else {
                        outline.name.clone()
                    };
                    let (response, _) = grid_columns_row(ui, &FRACTIONS, &[&name, &area], editor.selected_blast == Some(blast));
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
                        if outline_menu(ui, outline, OutlineKind::Blast, document, commands) {
                            ui.close();
                        }
                    });
                }
            });
            editor.scroll_to_blast = false;
        })
}

/// Which kind of ground an outline is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutlineKind {
    Blast,
    DigBlock,
}

/// What a blast or dig block offers, from its list row or from the viewport:
/// a blast's name, and taking either out of mining. Returns whether an item
/// was chosen.
///
/// Ground out of mining is left out of the dig blocks, so nothing from Dig
/// Strips on - reserves, sequencing, the schedule - sees it. It stays listed
/// and outlined here so it can be put back.
pub(crate) fn outline_menu(ui: &mut egui::Ui, outline: &crate::ui::state::BlastOutline, kind: OutlineKind, document: &Document, commands: &mut Vec<UiCommand>) -> bool {
    let mut chosen = false;
    let shape = BlastShapeRef::new(outline.solid, outline.bench_base, outline.anchor);
    if kind == OutlineKind::Blast {
        if ContextMenuAction::new(tr!("planning-rename-blast")).show(ui).clicked() {
            commands.push(UiCommand::BeginRenameItem(RenameTarget::BlastShape(shape)));
            chosen = true;
        }
        if ContextMenuAction::new(tr!("planning-reset-blast-name")).show(ui).clicked() {
            commands.push(UiCommand::ResetBlastName(shape));
            chosen = true;
        }
    }
    let Some(solid) = document.solid(outline.solid) else { return chosen };
    let target = match kind {
        OutlineKind::Blast => crate::model::ExclusionTarget::Blast {
            bench: outline.bench_base,
            anchor: outline.anchor,
        },
        // A dig outline's base is its flitch's.
        OutlineKind::DigBlock => crate::model::ExclusionTarget::Block {
            flitch: outline.bench_base,
            anchor: outline.anchor,
        },
    };
    let face: Vec<Vec<glam::DVec2>> = outline.rings.iter().map(|ring| ring.iter().map(|point| point.truncate()).collect()).collect();
    let own = solid.exclusions.face_excluded(target, &face);
    // Out with the bench or blast around it: put back from there instead.
    let inherited = outline.excluded && !own;
    if ContextMenuAction::new(tr!("planning-exclude-from-mining"))
        .checked(own || inherited)
        .enabled(!inherited)
        .show(ui)
        .clicked()
    {
        commands.push(UiCommand::UpdateSolid {
            solid: solid.id,
            edit: crate::model::SolidEdit::Exclusions(solid.exclusions.with_face(target, &face, !own)),
        });
        chosen = true;
    }
    chosen
}

/// The anchor the derivation stored for this outline. Recomputing it here
/// would risk disagreeing with the stored one over a rounding step, so the
/// outline carries the point it was matched on.
fn anchor_of(outline: &crate::ui::state::BlastOutline) -> [f64; 2] {
    outline.anchor
}
