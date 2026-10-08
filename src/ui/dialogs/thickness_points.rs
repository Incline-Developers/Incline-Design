//! The thickness points and thickness surfaces dialogs, each opened on the
//! one surface selected, and the tables of what they made.

use super::reference_points::{hole_of, logged_codes, seam_controls};
use crate::{
    i18n::tr,
    model::{
        drill_hole::OpenDrillHoleDataset,
        thickness_points::{InterceptSource, ThicknessPoint},
    },
    ui::{
        state::{EditorState, GridCheck, UiCommand},
        widgets::{
            collapsible_section::CollapsibleSection,
            data_table::DataTable,
            menu::{self, DragableMenu, MenuButton, selected_source_field},
        },
    },
};

pub(crate) fn draw_thickness_points_dialog(ui: &mut egui::Ui, editor: &mut EditorState, datasets: &[OpenDrillHoleDataset], commands: &mut Vec<UiCommand>) {
    let Some(draft) = editor.thickness_points_dialog.as_mut() else {
        return;
    };
    // The selected holes' datasets, or every loaded one when no hole came
    // in with the surface.
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
    DragableMenu::new("thickness_points_dialog", tr!("common-thickness-points"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            let width = 220.0;
            selected_source_field(
                ui,
                tr!("thickness-points-surface"),
                draft.surface_label.clone(),
                tr!("thickness-points-surface-help"),
                width,
            );
            ui.add_space(4.0);
            selected_source_field(ui, tr!("thickness-points-holes"), holes_label.clone(), tr!("thickness-points-holes-help"), width);
            ui.add_space(4.0);
            seam_controls(ui, "thickness_points", &involved, &codes, &mut draft.seam);
            ui.small(tr!("thickness-points-side-note"));
            ui.add_space(4.0);
            // Folded away: most runs use the holes alone.
            CollapsibleSection::new("thickness_points_field_measurements", tr!("thickness-points-field-measurements")).show(ui, |ui| {
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
            grid_note(ui, &draft.grid);
            menu::menu_actions(ui, |ui| {
                let ready = draft.seam.value.is_some() && draft.grid == GridCheck::Grid;
                let confirm = ready && menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!("reference-points-make")).primary().enabled(ready)).clicked() || confirm {
                    make = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    cancel = true;
                }
            });
        });
    if make && let (Some(field), Some(target)) = (draft.seam.field.clone(), draft.seam.value.clone()) {
        editor.last_seam = Some(draft.seam.clone());
        commands.push(UiCommand::MakeThicknessPoints {
            surface: draft.surface,
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

pub(crate) fn draw_seam_surface_dialog(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let Some(draft) = editor.seam_surface_dialog.as_ref() else {
        return;
    };
    let mut open = true;
    let mut make = false;
    let mut cancel = false;
    DragableMenu::new("seam_surface_dialog", tr!("common-thickness-surfaces"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            let width = 220.0;
            selected_source_field(ui, tr!("thickness-points-surface"), draft.surface_label.clone(), tr!("seam-surface-reference-help"), width);
            ui.add_space(4.0);
            selected_source_field(ui, tr!("seam-surface-run"), draft.run_label.clone(), tr!("seam-surface-run-help"), width);
            ui.add_space(4.0);
            selected_source_field(ui, tr!("seam-surface-output"), draft.output_label.clone(), tr!("seam-surface-output-help"), width);
            menu::menu_note(ui, tr!("seam-surface-note"));
            grid_note(ui, &draft.grid);
            menu::menu_actions(ui, |ui| {
                let ready = draft.grid == GridCheck::Grid;
                let confirm = ready && menu::dialog_confirm_pressed(ui.ctx());
                if ui.add(MenuButton::new(tr!("reference-points-make")).primary().enabled(ready)).clicked() || confirm {
                    make = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    cancel = true;
                }
            });
        });
    if make {
        commands.push(UiCommand::MakeSeamSurface { surface: draft.surface });
        open = false;
    }
    if !open || cancel {
        editor.seam_surface_dialog = None;
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
