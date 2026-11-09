//! The build surface dialog: the points, controls and optional extent
//! selected when it was opened, gridded into one new surface.

use crate::{
    app::commands::triangulation::reference_surface::GRID_SPACINGS,
    i18n::tr,
    ui::{
        elements::properties::read_only_row,
        state::{EditorState, UiCommand, UiProjectView},
        widgets::menu::{self, DragableMenu, MenuButton, MenuFieldCombo, MenuFieldText, selected_source_field},
    },
};

pub(crate) fn draw_reference_surface_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    let Some(draft) = editor.reference_surface_dialog.as_mut() else {
        return;
    };
    let mut open = true;
    let mut build = false;
    // Both are read after the menu closes: `open` is borrowed by the window
    // for as long as its body runs, so neither button can clear it in place.
    let mut cancel = false;
    let mut over_budget = false;
    DragableMenu::new("reference_surface_dialog", tr!("common-build-surface"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            let width = 220.0;
            selected_source_field(
                ui,
                tr!("context-points"),
                draft.points_label.clone(),
                tr!("reference-surface-points-surface-built-from-selected"),
                width,
            );
            ui.add_space(4.0);
            selected_source_field(
                ui,
                tr!("reference-surface-controls"),
                draft.controls_label.clone(),
                tr!("reference-surface-selected-open-strings-surface-made"),
                width,
            );
            ui.add_space(4.0);
            selected_source_field(ui, tr!("reference-surface-extent"), draft.extent_label.clone(), tr!("reference-surface-extent-help"), width);
            ui.add_space(4.0);
            if let Some(quality) = &draft.quality {
                let value = tr!("tri-quality-surface-value", quality = quality.name.clone(), unit = quality.unit_label());
                read_only_row(ui, &tr!("tri-quality-surface"), &value);
                ui.add_space(4.0);
            }
            let spacing_label = |spacing: f64| format!("{spacing} {}", tr!("common-m"));
            let selected = spacing_label(editor.reference_surface_spacing);
            MenuFieldCombo::new(
                "reference_surface_spacing",
                tr!("reference-surface-spacing"),
                &mut editor.reference_surface_spacing,
                selected,
                GRID_SPACINGS.map(|spacing| (spacing, spacing_label(spacing).into())),
            )
            .help_text(tr!("reference-surface-spacing-help"))
            .width(width)
            .show(ui);
            let spacing = editor.reference_surface_spacing;
            if let Some(plan) = draft.plan_extent {
                let nodes = plan.nodes(spacing).to_string();
                if plan.over_budget(spacing) {
                    over_budget = true;
                    let budget = crate::model::rbf::NODE_BUDGET.to_string();
                    let words = match plan.finest_fitting() {
                        Some(fits) => tr!("reference-surface-spacing-over-budget", nodes = nodes, budget = budget, spacing = fits.to_string()),
                        None => tr!("reference-surface-spacing-none-fits", nodes = nodes, budget = budget),
                    };
                    ui.colored_label(ui.visuals().warn_fg_color, words);
                } else {
                    ui.small(tr!("reference-surface-spacing-nodes", nodes = nodes));
                }
            }
            menu::menu_note(ui, tr!("reference-surface-grids-selected-points-plan-into"));
            ui.small(tr!("reference-surface-points-outside-extent-still-shape"));
            // Read only here: one place to change them, and the build uses
            // what the project holds.
            ui.small(project.modelling.summary());
            ui.small(tr!("reference-surface-change-these-in-preferences"));
            MenuFieldText::new(tr!("tri-create-output-name"), &mut draft.name)
                .help_text(tr!("tri-create-output-name-help"))
                .width(width)
                .hint_text(tr!("tri-create-output-name-hint"))
                .show(ui);
            menu::menu_actions(ui, |ui| {
                let ready = !draft.name.trim().is_empty() && !over_budget;
                let confirm = ready && menu::dialog_confirmed(ui.ctx());
                if ui.add(MenuButton::new(tr!("reference-points-make")).primary().enabled(ready)).clicked() || confirm {
                    build = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    cancel = true;
                }
            });
        });
    if build {
        commands.push(UiCommand::BuildReferenceSurface {
            points: draft.points.clone(),
            controls: draft.controls.clone(),
            extent: draft.extent,
            spacing: editor.reference_surface_spacing,
            name: draft.name.trim().to_owned(),
        });
        open = false;
    }
    if !open || cancel {
        editor.reference_surface_dialog = None;
    }
}
