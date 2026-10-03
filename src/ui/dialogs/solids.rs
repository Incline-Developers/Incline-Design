//! The Solids Setup subpage's New Solid dialog (Solids step).

use crate::{
    i18n::tr,
    model::{SolidKind, block_model::BlockModelId, triangulation::TriangulationId},
    ui::{
        EditorState, UiProjectView,
        state::UiCommand,
        widgets::menu::{self, DragableMenu, MenuButton, MenuFieldCombo, MenuFieldText},
    },
};

pub(crate) fn kind_label(kind: SolidKind) -> String {
    match kind {
        SolidKind::Pit => tr!("edit-pit"),
        SolidKind::Dump => tr!("report-dump"),
        SolidKind::Stockpile => tr!("report-stockpile"),
    }
}

/// Label for a triangulation reference: the surface's name, or a placeholder
/// when it is unset or the surface it named is no longer in the project.
pub(crate) fn triangulation_label(project: &UiProjectView, id: Option<TriangulationId>) -> String {
    match id {
        None => tr!("grade-calendar-none"),
        Some(id) => project
            .triangulations
            .iter()
            .find(|entry| entry.id == id)
            .map_or_else(|| tr!("solids-missing-surface"), |entry| entry.name.clone()),
    }
}

/// Block-model counterpart of [`triangulation_label`].
pub(crate) fn block_model_label(project: &UiProjectView, id: Option<BlockModelId>) -> String {
    match id {
        None => tr!("grade-calendar-none"),
        Some(id) => project
            .block_models
            .iter()
            .find(|entry| entry.id == id)
            .map_or_else(|| tr!("solids-missing-block-model"), |entry| entry.name.clone()),
    }
}

/// Every triangulation in the project, offered after an explicit "None".
pub(crate) fn triangulation_options(project: &UiProjectView) -> Vec<(Option<TriangulationId>, String)> {
    std::iter::once((None, tr!("grade-calendar-none")))
        .chain(project.triangulations.iter().map(|entry| (Some(entry.id), entry.name.clone())))
        .collect()
}

pub(crate) fn block_model_options(project: &UiProjectView) -> Vec<(Option<BlockModelId>, String)> {
    std::iter::once((None, tr!("grade-calendar-none")))
        .chain(project.block_models.iter().map(|entry| (Some(entry.id), entry.name.clone())))
        .collect()
}

/// Draw the dialog that adds one solid to the project.
///
/// A solid is a name, the design surface it is bounded by (a pit shell or a
/// dump design), the topography that surface is measured against, what kind
/// of volume it is, and - for a pit, whose material comes out of the ground -
/// the block model it reserves against. Dumps and stockpiles are placed
/// material, so their model stays optional.
pub(crate) fn draw_new_solid_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.new_solid_open {
        return;
    }
    let mut open = true;
    let mut close = false;
    DragableMenu::new("new_solid_dialog", tr!("solids-new-solid"))
        .open(&mut open)
        .min_width(320.0)
        .show(ui.ctx(), |ui| {
            MenuFieldText::new(tr!("planning-name"), &mut editor.new_solid_name)
                .hint_text(tr!("dialog-rename-field-hint"))
                .show(ui);
            let selected_kind = kind_label(editor.new_solid_kind);
            MenuFieldCombo::new(
                "new_solid_kind",
                tr!("destination-type"),
                &mut editor.new_solid_kind,
                selected_kind,
                SolidKind::ALL.into_iter().map(|kind| (kind, kind_label(kind).into())),
            )
            .show(ui);
            let surface_text = triangulation_label(project, editor.new_solid_surface);
            MenuFieldCombo::new(
                "new_solid_surface",
                tr!("tri-type-open-surface"),
                &mut editor.new_solid_surface,
                surface_text,
                triangulation_options(project).into_iter().map(|(id, name)| (id, name.into())),
            )
            .help_text(tr!("solids-pit-dump-design-itself"))
            .show(ui);
            let topography_text = triangulation_label(project, editor.new_solid_topography);
            MenuFieldCombo::new(
                "new_solid_topography",
                tr!("solids-topography"),
                &mut editor.new_solid_topography,
                topography_text,
                triangulation_options(project).into_iter().map(|(id, name)| (id, name.into())),
            )
            .help_text(tr!("solids-surface-design-measured-against"))
            .show(ui);
            let block_model_text = block_model_label(project, editor.new_solid_block_model);
            MenuFieldCombo::new(
                "new_solid_block_model",
                tr!("ws-menubar-block-model"),
                &mut editor.new_solid_block_model,
                block_model_text,
                block_model_options(project).into_iter().map(|(id, name)| (id, name.into())),
            )
            .help_text(if editor.new_solid_kind.requires_block_model() {
                tr!("solids-reserved-against-model")
            } else {
                tr!("solids-optional-dumps-stockpiles")
            })
            .show(ui);
            menu::menu_actions(ui, |ui| {
                let can_add = !editor.new_solid_name.trim().is_empty();
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("solids-add-solid")).primary().enabled(can_add)).clicked()) && can_add {
                    commands.push(UiCommand::AddSolid {
                        name: editor.new_solid_name.trim().to_owned(),
                        kind: editor.new_solid_kind,
                        surface: editor.new_solid_surface,
                        topography: editor.new_solid_topography,
                        block_model: editor.new_solid_block_model,
                    });
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.new_solid_open = false;
        editor.new_solid_name.clear();
        editor.new_solid_kind = SolidKind::Pit;
        editor.new_solid_surface = None;
        editor.new_solid_topography = None;
        editor.new_solid_block_model = None;
    }
}

/// Open the Update Topography dialog from a solid's menu: the solids measured
/// against the same topography as `from` are ticked, since a new survey
/// usually replaces the surface all of them were cut from.
pub(crate) fn open_topography_update(editor: &mut EditorState, solids: &[crate::model::Solid], from: Option<crate::model::SolidId>) {
    let shared = from.and_then(|id| solids.iter().find(|solid| solid.id == id)).map(|solid| solid.topography);
    let ticked = solids
        .iter()
        .filter(|solid| match shared {
            Some(topography) => solid.topography == topography,
            None => true,
        })
        .map(|solid| solid.id)
        .collect();
    editor.topography_update = Some(crate::ui::state::TopographyUpdate {
        topography: shared.flatten(),
        solids: ticked,
    });
}

/// Point several solids at one topography in a single step.
pub(crate) fn draw_topography_update_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, solids: &[crate::model::Solid], commands: &mut Vec<UiCommand>) {
    let Some(update) = editor.topography_update.as_mut() else {
        return;
    };
    let mut open = true;
    let mut close = false;
    DragableMenu::new("topography_update_dialog", tr!("planning-topography-update"))
        .open(&mut open)
        .min_width(340.0)
        .show(ui.ctx(), |ui| {
            menu::menu_note(ui, tr!("planning-topography-update-note"));
            let topography_text = triangulation_label(project, update.topography);
            MenuFieldCombo::new(
                "topography_update_surface",
                tr!("solids-topography"),
                &mut update.topography,
                topography_text,
                triangulation_options(project).into_iter().map(|(id, name)| (id, name.into())),
            )
            .show(ui);
            menu::menu_section(ui, tr!("planning-topography-update-solids"));
            for solid in solids {
                let mut on = update.solids.contains(&solid.id);
                let current = triangulation_label(project, solid.topography);
                if ui
                    .checkbox(&mut on, tr!("planning-topography-update-row", name = solid.name.clone(), current = current))
                    .changed()
                {
                    if on {
                        update.solids.push(solid.id);
                    } else {
                        update.solids.retain(|id| *id != solid.id);
                    }
                }
            }
            menu::menu_actions(ui, |ui| {
                let can_apply = !update.solids.is_empty();
                if ui.add(MenuButton::new(tr!("planning-topography-update-apply")).primary().enabled(can_apply)).clicked() && can_apply {
                    commands.push(UiCommand::SetSolidsTopography {
                        solids: update.solids.clone(),
                        topography: update.topography,
                    });
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.topography_update = None;
    }
}
