//! The Modelling branch's settings: the coordinate system the model is built
//! in, and how Build Surface draws its grid. They are the project's settings,
//! shown and chosen here because this is where a modeller looks for them.

use crate::{
    i18n::tr,
    model::{
        crs,
        project::{ModellingSettings, SurfaceMethod},
    },
    ui::{
        state::{EditorState, UiCommand, UiProjectView},
        widgets::menu::{self, DragableMenu, MenuFieldCombo, MenuFieldF64},
    },
};

pub(crate) fn draw_modelling_settings_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.show_modelling_settings {
        return;
    }
    let mut open = true;
    DragableMenu::new("modelling_settings_dialog", tr!("modelling-settings-modelling-settings"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            if !project.has_active_project {
                menu::menu_note(ui, tr!("common-no-open-project"));
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
                (None, true) => tr!("modelling-settings-not-set"),
                // A spelling from another program, or one no definition
                // matches: shown as the project carries it, never guessed at.
                (None, false) => match crs::CoordinateSystem::parse_stored(current) {
                    crs::StoredSystem::Unrecognised(text) => text,
                    _ => current.to_owned(),
                },
            };
            let options = std::iter::once((None, tr!("common-none").into())).chain(definitions.iter().map(|(name, _)| (Some(name.clone()), name.clone().into())));
            if MenuFieldCombo::new("modelling_settings_datum", tr!("modelling-settings-survey-datum"), &mut chosen, shown, options)
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
            ui.small(tr!("modelling-settings-crs-help"));
            surface_settings(ui, project, commands);
        });
    if !open {
        editor.show_modelling_settings = false;
    }
}

/// Build Surface's method and checks. A number being dragged is held in the
/// window's memory and sent once, when the drag ends.
fn surface_settings(ui: &mut egui::Ui, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    menu::menu_section(ui, tr!("common-build-surface"));
    let draft_id = ui.id().with("modelling_settings_surface_draft");
    let mut settings = ui.data(|data| data.get_temp::<ModellingSettings>(draft_id)).unwrap_or(project.modelling);
    let method_label = |method: SurfaceMethod| match method {
        SurfaceMethod::ThinPlateSpline => tr!("modelling-settings-thin-plate-spline-exact"),
    };
    // No settings: the same picks always give the same surface.
    let methods = [SurfaceMethod::ThinPlateSpline];
    let shown = method_label(settings.surface_method);
    let responses = [
        MenuFieldCombo::new(
            "modelling_settings_surface_method",
            tr!("modelling-settings-surface-method"),
            &mut settings.surface_method,
            shown,
            methods.map(|method| (method, method_label(method).into())),
        )
        .unavailable(tr!("modelling-settings-anisotropic-spline"))
        .unavailable(tr!("modelling-settings-auto-axis-spline"))
        .unavailable(tr!("modelling-settings-hermite-dips"))
        .show(ui),
        // Every range here is exactly what `ModellingSettings::problem` accepts:
        // a narrower one would clamp a loaded value just by being drawn.
        MenuFieldF64::new(tr!("modelling-settings-steep-pair-distance"), &mut settings.steep_distance, f64::MIN_POSITIVE..=f64::MAX)
            .suffix(" m")
            .max_decimals(2)
            .help_text(tr!("modelling-settings-steep-pair-distance-help"))
            .show(ui),
        MenuFieldF64::new(tr!("modelling-settings-steep-pair-angle"), &mut settings.steep_degrees, f64::MIN_POSITIVE..=90.0)
            .suffix("°")
            .max_decimals(1)
            .show(ui),
    ];
    ui.small(tr!("modelling-settings-surface-help"));
    if responses.iter().any(|response| response.dragged()) {
        ui.data_mut(|data| data.insert_temp(draft_id, settings));
    } else {
        ui.data_mut(|data| data.remove::<ModellingSettings>(draft_id));
    }
    if responses.iter().any(menu::committed) && settings != project.modelling && settings.problem().is_none() {
        commands.push(UiCommand::SetModellingSettings(settings));
    }
}
