//! The Modelling branch's settings: the coordinate system the model is built
//! in. It is the project's setting, shown and chosen here because this is
//! where a modeller looks for it.

use crate::{
    i18n::tr,
    model::crs,
    ui::{
        state::{EditorState, UiCommand, UiProjectView},
        widgets::menu::{self, DragableMenu, MenuFieldCombo},
    },
};

pub(crate) fn draw_modelling_settings_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.show_modelling_settings {
        return;
    }
    let mut open = true;
    DragableMenu::new("modelling_settings_dialog", tr!(literal = "Modelling Settings"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            if !project.has_active_project {
                menu::menu_note(ui, tr!(literal = "No open project"));
                return;
            }
            let current = project.coordinate_reference_system.trim();
            // Each named definition's stored spelling, so the one the project
            // carries can be recognised among them.
            let definitions: Vec<(String, String)> = editor
                .survey
                .definitions
                .iter()
                .filter_map(|definition| {
                    let stored = editor.survey.resolve(&Some(definition.name.clone())).ok()?.to_stored();
                    Some((definition.name.clone(), stored))
                })
                .collect();
            let mut chosen: Option<String> = definitions.iter().find(|(_, stored)| stored == current).map(|(name, _)| name.clone());
            let shown = match (&chosen, current.is_empty()) {
                (Some(name), _) => name.clone(),
                (None, true) => tr!(literal = "Not set"),
                // A spelling from another program, or one no definition
                // matches: shown as the project carries it, never guessed at.
                (None, false) => match crs::CoordinateSystem::parse_stored(current) {
                    crs::StoredSystem::Unrecognised(text) => text,
                    _ => current.to_owned(),
                },
            };
            let options = std::iter::once((None, tr!(literal = "None").into())).chain(definitions.iter().map(|(name, _)| (Some(name.clone()), name.clone().into())));
            if MenuFieldCombo::new("modelling_settings_datum", tr!(literal = "Survey datum"), &mut chosen, shown, options)
                .show(ui)
                .changed()
            {
                let stored = chosen
                    .as_deref()
                    .and_then(|name| definitions.iter().find(|(candidate, _)| candidate == name))
                    .map(|(_, stored)| stored.clone())
                    .unwrap_or_default();
                commands.push(UiCommand::SetProjectCoordinateSystem(stored));
            }
            ui.small(tr!(
                literal = "The coordinate system every point and surface in this project is in. Defined under Survey; a project-level setting, whichever place it is set from."
            ));
        });
    if !open {
        editor.show_modelling_settings = false;
    }
}
