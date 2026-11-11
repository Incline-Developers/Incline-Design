//! The thickness points and thickness surfaces dialogs, each on its chosen
//! surface, Misfit at Holes on its chosen prismoid grid, and the tables of
//! what they made.

use super::{
    reference_points::{hole_of, logged_codes, seam_controls, seam_value_controls},
    triangulation::{prismoid_grid_field, subject_surface_field},
};
use crate::{
    i18n::tr,
    model::{
        drill_hole::OpenDrillHoleDataset,
        thickness_points::{InterceptSource, MisfitRow, ThicknessPoint, three_places},
    },
    ui::{
        state::{EditorState, GridCheck, HoleMisfitTable, SelectionPickTool, ToolSubject, TriangulationPickTarget, UiCommand, UiProjectView},
        widgets::{
            collapsible_section::CollapsibleSection,
            data_table::DataTable,
            menu::{self, DragableMenu, MenuButton, picked_selection_field, selected_source_field},
        },
    },
};

pub(crate) fn draw_thickness_points_dialog(ui: &mut egui::Ui, editor: &mut EditorState, datasets: &[OpenDrillHoleDataset], project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    // Hidden while either of its picks has the viewport.
    if editor.selection_pick.is_some() || editor.triangulation_pick_target.is_some() {
        return;
    }
    let Some(draft) = editor.thickness_points_dialog.as_mut() else {
        return;
    };
    // A surface unloaded or deleted from under the dialog reads as none
    // chosen.
    let surface = draft.surface.filter(|id| project.triangulations.iter().any(|entry| entry.is_loaded && entry.id == *id));
    // The picked holes' datasets, or every loaded one when none is picked.
    let involved: Vec<&OpenDrillHoleDataset> = datasets
        .iter()
        .filter(|dataset| dataset.state.loaded && (draft.holes.is_empty() || draft.holes.iter().any(|hole| hole.dataset == dataset.id)))
        .collect();
    let holes_label = match (draft.holes.len(), involved.as_slice()) {
        (0, involved) => tr!("thickness-points-every-hole", datasets = involved.len().to_string()),
        (count, [dataset]) => tr!("reference-points-count-holes-from-dataset", count = count.to_string(), dataset = dataset.name.clone()),
        (count, involved) => tr!("reference-points-holes-from-datasets", count = count.to_string(), datasets = involved.len().to_string()),
    };
    if let Some(field) = draft.seam.field.clone()
        && draft.codes.as_ref().is_none_or(|(read, _)| *read != field)
    {
        let codes = if draft.holes.is_empty() {
            logged_codes(involved.iter().flat_map(|dataset| dataset.dataset.holes.iter()), &field)
        } else {
            logged_codes(draft.holes.iter().filter_map(|reference| hole_of(&involved, *reference)), &field)
        };
        draft.codes = Some((field, codes));
    }
    let codes = draft.codes.as_ref().map(|(_, codes)| codes.clone()).unwrap_or_default();
    let mut open = true;
    let mut make = false;
    let mut cancel = false;
    let mut pick_surface = false;
    DragableMenu::new("thickness_points_dialog", tr!("common-thickness-points"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            let width = 220.0;
            let (chosen, pick) = subject_surface_field(ui, "thickness_points_surface", surface, tr!("thickness-points-surface-help"), project, width);
            if let Some(id) = chosen {
                commands.push(UiCommand::SetToolSubject(ToolSubject::ThicknessSurface(id)));
            }
            pick_surface = pick;
            ui.add_space(4.0);
            if picked_selection_field(ui, tr!("thickness-points-holes"), holes_label.clone(), tr!("thickness-points-holes-help"), width) {
                commands.push(UiCommand::BeginSelectionPick(SelectionPickTool::ThicknessPoints));
            }
            ui.add_space(4.0);
            seam_controls(ui, "thickness_points", &involved, &codes, &mut draft.seam);
            ui.small(tr!("thickness-points-side-note"));
            ui.add_space(4.0);
            // Folded away: most runs use the holes alone.
            CollapsibleSection::new(egui::Id::unique("thickness_points_field_measurements"), tr!("thickness-points-field-measurements")).show(ui, |ui| {
                let pairs = draft.pairs.as_ref().map_or_else(|| tr!("thickness-points-no-pairs"), |file| file.name.clone());
                selected_source_field(ui, tr!("thickness-points-pairs"), pairs, tr!("thickness-points-pairs-help"), width);
                ui.horizontal(|ui| {
                    if ui.add(MenuButton::new(tr!("thickness-points-choose-pairs"))).clicked() {
                        commands.push(UiCommand::ChooseThicknessPairs);
                    }
                    if ui.add(MenuButton::new(tr!("thickness-points-clear-pairs")).enabled(draft.pairs.is_some())).clicked() {
                        draft.pairs = None;
                    }
                });
            });
            ui.add_space(4.0);
            menu::MenuFieldBool::new(tr!("thickness-points-then-surface"), &mut draft.then_surface)
                .help_text(tr!("thickness-points-then-surface-help"))
                .show(ui);
            menu::menu_note(ui, tr!("thickness-points-note"));
            if surface.is_some() {
                grid_note(ui, &draft.grid);
            }
            menu::menu_actions(ui, |ui| {
                let ready = surface.is_some() && draft.seam.value.is_some() && draft.grid == GridCheck::Grid;
                let confirm = ready && menu::dialog_confirmed(ui.ctx());
                if ui.add(MenuButton::new(tr!("reference-points-make")).primary().enabled(ready)).clicked() || confirm {
                    make = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    cancel = true;
                }
            });
        });
    if make && let (Some(surface), Some(field), Some(target)) = (surface, draft.seam.field.clone(), draft.seam.value.clone()) {
        editor.last_seam = Some(draft.seam.clone());
        commands.push(UiCommand::MakeThicknessPoints {
            surface,
            holes: draft.holes.clone(),
            field,
            target,
            side: draft.seam.side,
            pairs: draft.pairs.clone(),
            then_surface: draft.then_surface,
        });
        open = false;
    }
    if !open || cancel {
        editor.thickness_points_dialog = None;
    } else if pick_surface {
        editor.triangulation_pick_target = Some(TriangulationPickTarget::ThicknessSurface);
    }
}

/// Why Make waits: the surface is still being read, or it is not one
/// regular grid and cannot be used.
fn grid_note(ui: &mut egui::Ui, grid: &GridCheck) {
    match grid {
        GridCheck::Checking => {
            ui.small(tr!("thickness-points-checking-surface"));
        }
        GridCheck::Grid => {}
        GridCheck::Refused(reason) => {
            ui.label(egui::RichText::new(reason).color(ui.visuals().warn_fg_color));
        }
    }
}

/// The point set's table shown, one row per point.
pub(crate) fn draw_thickness_table(ui: &mut egui::Ui, editor: &mut EditorState) {
    let Some(table) = editor.thickness_table.clone() else {
        return;
    };
    let mut open = true;
    let mut close = false;
    let header = [
        tr!("thickness-points-column-source"),
        tr!("thickness-points-column-x"),
        tr!("thickness-points-column-y"),
        tr!("thickness-points-column-true"),
        tr!("thickness-points-column-vertical"),
        tr!("thickness-points-column-along"),
        tr!("thickness-points-column-dip"),
        tr!("thickness-points-column-direction"),
        tr!("thickness-points-column-roof"),
        tr!("thickness-points-column-floor"),
    ];
    let aligns = [
        egui::Align::Min,
        egui::Align::Max,
        egui::Align::Max,
        egui::Align::Max,
        egui::Align::Max,
        egui::Align::Max,
        egui::Align::Max,
        egui::Align::Max,
        egui::Align::Max,
        egui::Align::Max,
    ];
    let shown = |row: usize| table.points.get(row).map_or_else(Vec::new, |point| cells(point, true));
    let copied = |row: usize| table.points.get(row).map_or_else(Vec::new, |point| cells(point, false));
    DragableMenu::new("thickness_points_table", tr!("thickness-points-table-title", name = table.name.clone()))
        .open(&mut open)
        .min_width(520.0)
        .max_width(900.0)
        .show(ui.ctx(), |ui| {
            ui.small(tr!(
                "thickness-points-table-surface",
                surface = table.surface.clone(),
                count = table.points.len().to_string()
            ));
            DataTable::new(("thickness_points_table", table.name.as_str()), &header, table.points.len(), &shown)
                .aligns(&aligns)
                .copy_cells(&copied)
                .max_height(320.0)
                .show(ui);
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("common-close"))).clicked() {
                    close = true;
                }
            });
        });
    if !open || close {
        editor.thickness_table = None;
    }
}

/// A point's row: rounded for the eye, or whole for a copy. Depths stand
/// for a hole's roof and floor, surveyed heights for a measured pair's.
fn cells(point: &ThicknessPoint, rounded: bool) -> Vec<String> {
    let number = |value: f64, places: usize| if rounded { format!("{value:.places$}") } else { value.to_string() };
    let (source, roof, floor) = match &point.source {
        InterceptSource::Hole { dhid, roof_depth, floor_depth } => (dhid.clone(), number(*roof_depth, 3), floor_depth.map_or_else(String::new, |depth| number(depth, 3))),
        InterceptSource::Measured { id, .. } => (id.clone(), number(point.roof.z, 3), number(point.floor.z, 3)),
    };
    vec![
        source,
        number(point.roof.x, 3),
        number(point.roof.y, 3),
        number(point.true_thickness, 3),
        number(point.vertical_thickness, 3),
        number(point.along_hole, 3),
        number(point.dip.dip, 2),
        number(point.dip.direction, 2),
        roof,
        floor,
    ]
}

/// Misfit at Holes: the grid chosen from the loaded ones, the holes and
/// quality points picked in the viewport, and the seam the grid models.
pub(crate) fn draw_hole_misfit_dialog(ui: &mut egui::Ui, editor: &mut EditorState, datasets: &[OpenDrillHoleDataset], project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    // Hidden while its Pick has the viewport.
    if editor.selection_pick.is_some() {
        return;
    }
    let Some(draft) = editor.hole_misfit_dialog.as_mut() else {
        return;
    };
    // A grid unloaded or deleted from under the dialog reads as none chosen.
    let grid = draft.grid.filter(|id| project.prismoid_grids.iter().any(|entry| entry.is_loaded && entry.id == *id));
    let involved: Vec<&OpenDrillHoleDataset> = datasets
        .iter()
        .filter(|dataset| dataset.state.loaded && (draft.holes.is_empty() || draft.holes.iter().any(|hole| hole.dataset == dataset.id)))
        .collect();
    let holes_label = match (draft.holes.len(), involved.as_slice()) {
        (0, involved) => tr!("hole-misfit-every-hole", datasets = involved.len().to_string()),
        (count, [dataset]) => tr!("reference-points-count-holes-from-dataset", count = count.to_string(), dataset = dataset.name.clone()),
        (count, involved) => tr!("reference-points-holes-from-datasets", count = count.to_string(), datasets = involved.len().to_string()),
    };
    if let Some(field) = draft.seam.field.clone()
        && draft.codes.as_ref().is_none_or(|(read, _)| *read != field)
    {
        let codes = if draft.holes.is_empty() {
            logged_codes(involved.iter().flat_map(|dataset| dataset.dataset.holes.iter()), &field)
        } else {
            logged_codes(draft.holes.iter().filter_map(|reference| hole_of(&involved, *reference)), &field)
        };
        draft.codes = Some((field, codes));
    }
    let codes = draft.codes.as_ref().map(|(_, codes)| codes.clone()).unwrap_or_default();
    // Quality points need no seam: with no hole chosen they compare alone.
    let quality_only = draft.holes.is_empty() && !draft.quality_points.is_empty();
    let mut open = true;
    let mut compare = false;
    let mut cancel = false;
    DragableMenu::new("hole_misfit_dialog", tr!("common-misfit-at-holes"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            let width = 220.0;
            if let Some(id) = prismoid_grid_field(ui, "hole_misfit_grid", tr!("hole-misfit-grid"), grid, tr!("hole-misfit-grid-help"), project, width) {
                commands.push(UiCommand::SetToolSubject(ToolSubject::MisfitGrid(id)));
            }
            ui.add_space(4.0);
            if picked_selection_field(ui, tr!("thickness-points-holes"), holes_label.clone(), tr!("hole-misfit-holes-help"), width) {
                commands.push(UiCommand::BeginSelectionPick(SelectionPickTool::HoleMisfit));
            }
            ui.add_space(4.0);
            selected_source_field(ui, tr!("hole-misfit-qualities"), draft.qualities_label.clone(), tr!("hole-misfit-qualities-help"), width);
            ui.add_space(4.0);
            seam_value_controls(ui, "hole_misfit", &involved, &codes, &mut draft.seam, false);
            let chosen = draft.seam.value.is_some();
            let ready = grid.is_some() && (chosen || quality_only);
            ui.small(match (chosen, quality_only) {
                (true, _) => tr!("hole-misfit-seam-help"),
                (false, true) => tr!("hole-misfit-no-seam"),
                (false, false) => tr!("hole-misfit-pick-seam"),
            });
            menu::menu_note(ui, tr!("hole-misfit-note"));
            menu::menu_actions(ui, |ui| {
                let confirm = ready && menu::dialog_confirmed(ui.ctx());
                if ui.add(MenuButton::new(tr!("hole-misfit-compare")).primary().enabled(ready)).clicked() || confirm {
                    compare = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    cancel = true;
                }
            });
        });
    // Closed by the command once the table is made.
    if compare && let Some(grid) = grid {
        commands.push(UiCommand::ExecuteHoleMisfit {
            grid,
            holes: draft.holes.clone(),
            quality_points: draft.quality_points.clone(),
            seam: draft.seam.field.clone().zip(draft.seam.value.clone()),
        });
    }
    if !open || cancel {
        editor.hole_misfit_dialog = None;
    }
}

/// The misfit table shown: each measure's totals, then one row per
/// measurement in the order measured.
pub(crate) fn draw_hole_misfit_table(ui: &mut egui::Ui, editor: &mut EditorState) {
    let Some(table) = editor.hole_misfit_table.clone() else {
        return;
    };
    let mut open = true;
    let mut close = false;
    let header = [
        tr!("thickness-points-column-source"),
        tr!("hole-misfit-column-measure"),
        tr!("thickness-points-column-x"),
        tr!("thickness-points-column-y"),
        tr!("hole-misfit-column-picked"),
        tr!("hole-misfit-column-model"),
        tr!("hole-misfit-column-difference"),
        tr!("hole-misfit-column-unit"),
        tr!("hole-misfit-column-note"),
    ];
    let mut aligns = [egui::Align::Max; 9];
    for column in [0, 1, 7, 8] {
        aligns[column] = egui::Align::Min;
    }
    let shown = |row: usize| table.rows.get(row).map_or_else(Vec::new, |row| misfit_cells(&table, row, true));
    let copied = |row: usize| table.rows.get(row).map_or_else(Vec::new, |row| misfit_cells(&table, row, false));
    DragableMenu::new("hole_misfit_table", tr!("hole-misfit-table-title", grid = table.grid.clone(), seam = table.seam.clone()))
        .open(&mut open)
        .min_width(620.0)
        .max_width(1000.0)
        .show(ui.ctx(), |ui| {
            for total in table.totals.iter().filter(|total| total.compared + total.not_compared > 0) {
                let measure = table.units.get(total.measure.slot()).map_or_else(String::new, |unit| unit.total_name.clone());
                let not = total.not_compared.to_string();
                let line = match (total.mean, total.largest) {
                    (Some(mean), Some((largest, row))) => tr!(
                        "hole-misfit-totals",
                        measure = measure,
                        compared = total.compared.to_string(),
                        mean = three_places(mean),
                        largest = three_places(largest),
                        source = table.rows.get(row).map_or_else(String::new, |row| row.source.clone()),
                        not = not
                    ),
                    _ => tr!("hole-misfit-totals-none", measure = measure, not = not),
                };
                ui.small(line);
            }
            if let Some(not_read) = &table.not_read {
                ui.small(not_read);
            }
            DataTable::new(("hole_misfit_table", table.grid.as_str(), table.seam.as_str()), &header, table.rows.len(), &shown)
                .aligns(&aligns)
                .copy_cells(&copied)
                .fingerprint(table.fingerprint)
                .max_height(320.0)
                .show(ui);
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("common-close"))).clicked() {
                    close = true;
                }
            });
        });
    if !open || close {
        editor.hole_misfit_table = None;
    }
}

/// A misfit row: rounded for the eye, or whole for a copy; blank where
/// nothing was read, with the reason in the note.
fn misfit_cells(table: &HoleMisfitTable, row: &MisfitRow, rounded: bool) -> Vec<String> {
    let number = |value: Option<f64>| value.map_or_else(String::new, |value| if rounded { three_places(value) } else { value.to_string() });
    let Some(unit) = table.units.get(row.measure.slot()) else {
        return Vec::new();
    };
    let note = match row.result {
        Ok(_) => unit.note.clone().unwrap_or_default(),
        Err(reason) => reason.words(&table.seam, &table.grid, &unit.name),
    };
    vec![
        row.source.clone(),
        unit.name.clone(),
        number(row.at.map(|at| at.x)),
        number(row.at.map(|at| at.y)),
        number(row.picked),
        number(row.model),
        number(row.result.ok()),
        unit.unit.clone(),
        note,
    ]
}

pub(crate) fn draw_seam_surface_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    // Hidden while its Pick has the viewport.
    if editor.triangulation_pick_target.is_some() {
        return;
    }
    let Some(draft) = editor.seam_surface_dialog.as_ref() else {
        return;
    };
    // A surface unloaded or deleted from under the dialog reads as none
    // chosen.
    let surface = draft.surface.filter(|id| project.triangulations.iter().any(|entry| entry.is_loaded && entry.id == *id));
    let mut open = true;
    let mut make = false;
    let mut cancel = false;
    let mut pick_surface = false;
    DragableMenu::new("seam_surface_dialog", tr!("common-thickness-surfaces"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            let width = 220.0;
            let (chosen, pick) = subject_surface_field(ui, "seam_surface_surface", surface, tr!("seam-surface-reference-help"), project, width);
            if let Some(id) = chosen {
                commands.push(UiCommand::SetToolSubject(ToolSubject::SeamSurface(id)));
            }
            pick_surface = pick;
            let usable = surface.is_some() && draft.problem.is_none();
            if usable {
                ui.add_space(4.0);
                selected_source_field(ui, tr!("seam-surface-run"), draft.run_label.clone(), tr!("seam-surface-run-help"), width);
                ui.add_space(4.0);
                selected_source_field(ui, tr!("seam-surface-output"), draft.output_label.clone(), tr!("seam-surface-output-help"), width);
            }
            menu::menu_note(ui, tr!("seam-surface-note"));
            if let Some(problem) = draft.problem.as_ref().filter(|_| surface.is_some()) {
                ui.colored_label(egui::Color32::from_rgb(200, 70, 70), problem);
            }
            if usable {
                grid_note(ui, &draft.grid);
            }
            menu::menu_actions(ui, |ui| {
                let ready = usable && draft.grid == GridCheck::Grid;
                let confirm = ready && menu::dialog_confirmed(ui.ctx());
                if ui.add(MenuButton::new(tr!("reference-points-make")).primary().enabled(ready)).clicked() || confirm {
                    make = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    cancel = true;
                }
            });
        });
    if make && let Some(surface) = surface {
        commands.push(UiCommand::MakeSeamSurface { surface });
        open = false;
    }
    if !open || cancel {
        editor.seam_surface_dialog = None;
    } else if pick_surface {
        editor.triangulation_pick_target = Some(TriangulationPickTarget::SeamSurface);
    }
}

/// The thickness grid's table shown, one row per node of the surface.
pub(crate) fn draw_seam_table(ui: &mut egui::Ui, editor: &mut EditorState) {
    let Some(table) = editor.seam_table.clone() else {
        return;
    };
    let mut open = true;
    let mut close = false;
    let header = [
        tr!("thickness-points-column-x"),
        tr!("thickness-points-column-y"),
        tr!("seam-surface-column-reference"),
        tr!("thickness-points-column-true"),
        tr!("thickness-points-column-vertical"),
        tr!("seam-surface-column-other"),
    ];
    let aligns = [egui::Align::Max; 6];
    let row = |index: usize, rounded: bool| {
        table.nodes.get(index).map_or_else(Vec::new, |node| {
            [node.at.x, node.at.y, node.reference, node.thickness, node.vertical, node.other]
                .map(|value| if rounded { format!("{value:.3}") } else { value.to_string() })
                .to_vec()
        })
    };
    let shown = |index: usize| row(index, true);
    let copied = |index: usize| row(index, false);
    DragableMenu::new("seam_surface_table", tr!("seam-surface-table-title", name = table.name.clone()))
        .open(&mut open)
        .min_width(520.0)
        .max_width(900.0)
        .show(ui.ctx(), |ui| {
            ui.small(tr!("seam-surface-table-surface", surface = table.surface.clone(), count = table.nodes.len().to_string()));
            DataTable::new(("seam_surface_table", table.name.as_str()), &header, table.nodes.len(), &shown)
                .aligns(&aligns)
                .copy_cells(&copied)
                .max_height(320.0)
                .show(ui);
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("common-close"))).clicked() {
                    close = true;
                }
            });
        });
    if !open || close {
        editor.seam_table = None;
    }
}
