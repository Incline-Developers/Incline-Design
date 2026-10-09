//! The reference points dialog: one working section, one side, one layer of
//! derived points, on the holes the dialog was opened on.

use std::collections::BTreeSet;

use crate::{
    i18n::tr,
    model::drill_hole::{DrillField, DrillFieldKind, DrillHole, DrillHoleRef, DrillValue, OpenDrillHoleDataset, ReferenceSide, ReferenceTarget},
    ui::{
        state::{EditorState, SeamChoice, UiCommand},
        widgets::menu::{DragableMenu, MenuButton, MenuFieldCombo, selected_source_field},
    },
};

pub(crate) fn draw_reference_points_dialog(ui: &mut egui::Ui, editor: &mut EditorState, datasets: &[OpenDrillHoleDataset], commands: &mut Vec<UiCommand>) {
    let Some(draft) = editor.reference_points_dialog.as_mut() else {
        return;
    };
    // The holes came in when the dialog opened, so the datasets they name are
    // gathered here rather than being chosen: one dataset or several, and one
    // that has since been unloaded drops out.
    let mut involved: Vec<&OpenDrillHoleDataset> = Vec::new();
    for hole in &draft.holes {
        if involved.iter().any(|dataset| dataset.id == hole.dataset) {
            continue;
        }
        if let Some(dataset) = datasets.iter().find(|dataset| dataset.id == hole.dataset && dataset.state.loaded) {
            involved.push(dataset);
        }
    }
    let holes_label = match involved.as_slice() {
        [dataset] => tr!(
            "reference-points-count-holes-from-dataset",
            count = draft.holes.len().to_string(),
            dataset = dataset.name.clone().to_string()
        ),
        involved => tr!(
            "reference-points-holes-from-datasets",
            count = draft.holes.len().to_string(),
            datasets = involved.len().to_string()
        ),
    };
    // The values the selected holes actually log, not every value the
    // datasets declare: a section none of these holes hold has no points to
    // place.
    let categories = draft
        .seam
        .field
        .as_deref()
        .map(|key| logged_codes(draft.holes.iter().filter_map(|reference| hole_of(&involved, *reference)), key))
        .unwrap_or_default();
    let mut open = true;
    let mut build = None;
    let mut collars = false;
    DragableMenu::new("reference_points_dialog", tr!("reference-points-reference-points"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            selected_source_field(
                ui,
                tr!("reference-points-holes"),
                holes_label.clone(),
                tr!("reference-points-holes-points-placed-selected-when"),
                220.0,
            );
            ui.add_space(4.0);

            // A ground surface is built from the collars where the project
            // has no topography of its own.
            let source_label = source_label_of(draft.collars);
            MenuFieldCombo::new(
                ("reference_points", "source"),
                tr!("reference-points-points-at"),
                &mut draft.collars,
                source_label,
                [false, true].map(|collars| (collars, source_label_of(collars).into())),
            )
            .show(ui);
            if draft.collars {
                ui.small(tr!("reference-points-one-point-per-hole-collar"));
                if ui.add(MenuButton::new(tr!("reference-points-make")).primary()).clicked() {
                    collars = true;
                }
                return;
            }

            seam_controls(ui, "reference_points", &involved, &categories, &mut draft.seam);

            ui.small(tr!("reference-points-one-point-per-hole-boundary"));
            if ui
                .add(MenuButton::new(tr!("reference-points-make")).primary().enabled(draft.seam.value.is_some()))
                .clicked()
                && let (Some(field), Some(target)) = (draft.seam.field.clone(), draft.seam.value.clone())
            {
                build = Some((field, target, draft.seam.side));
            }
        });
    if collars {
        commands.push(UiCommand::BuildCollarPoints { holes: draft.holes.clone() });
        open = false;
    }
    if let Some((field, target, side)) = build {
        editor.last_seam = Some(draft.seam.clone());
        commands.push(UiCommand::BuildReferencePoints {
            holes: draft.holes.clone(),
            field,
            target,
            side,
        });
        open = false;
    }
    if !open {
        editor.reference_points_dialog = None;
    }
}

/// Where the points go: at a logged pick, or at the collars.
fn source_label_of(collars: bool) -> String {
    if collars {
        tr!("reference-points-at-collars")
    } else {
        tr!("reference-points-at-logged-pick")
    }
}

/// The hole `reference` names among `involved`, when its dataset is there.
pub(crate) fn hole_of<'a>(involved: &[&'a OpenDrillHoleDataset], reference: DrillHoleRef) -> Option<&'a DrillHole> {
    involved
        .iter()
        .find(|dataset| dataset.id == reference.dataset)
        .and_then(|dataset| dataset.dataset.holes.get(reference.hole))
}

/// Every code `holes` log in `field`, once each, sorted.
pub(crate) fn logged_codes<'a>(holes: impl Iterator<Item = &'a DrillHole>, field: &str) -> Vec<String> {
    let mut found: BTreeSet<&str> = BTreeSet::new();
    for hole in holes {
        for interval in &hole.intervals {
            if let Some(DrillValue::Category(code)) = interval.values.get(field)
                && !code.trim().is_empty()
            {
                found.insert(code.as_str());
            }
        }
    }
    found.into_iter().map(str::to_owned).collect()
}

/// The seam a tool works on: the categorical field carrying the working
/// section, the section (or one code) and the side. `categories` are the
/// codes the tool's holes log in the field chosen; a field changed here
/// clears the section until they are read for the new one.
pub(crate) fn seam_controls(ui: &mut egui::Ui, id: &str, involved: &[&OpenDrillHoleDataset], categories: &[String], seam: &mut SeamChoice) {
    // Every categorical field the datasets carry, named once where two
    // datasets log the same one.
    let mut fields: Vec<&DrillField> = Vec::new();
    for dataset in involved {
        for field in dataset.dataset.fields.iter().filter(|field| matches!(field.kind, DrillFieldKind::Categorical { .. })) {
            if !fields.iter().any(|seen| seen.key == field.key) {
                fields.push(field);
            }
        }
    }
    // The geologist names the categorical field that carries the working
    // section.
    if !seam.field.as_deref().is_some_and(|key| fields.iter().any(|field| field.key == key)) {
        seam.field = fields.first().map(|field| field.key.clone());
        seam.value = None;
    }
    let field_label = seam
        .field
        .as_deref()
        .and_then(|key| fields.iter().find(|field| field.key == key))
        .map(|field| field.label.clone())
        .unwrap_or_else(|| tr!("reference-points-no-categorical-field"));
    if MenuFieldCombo::new(
        (id, "field"),
        tr!("reference-points-working-section-field"),
        &mut seam.field,
        field_label,
        fields.iter().map(|field| (Some(field.key.clone()), field.label.clone().into())),
    )
    .show(ui)
    .changed()
    {
        seam.value = None;
        return;
    }
    let logged = |code: &str| categories.iter().any(|category| category == code);
    // Working sections named on the datasets come first: one of them picks
    // its codes as one run. Only those the holes log show.
    let mut sections: Vec<&str> = Vec::new();
    if let Some(key) = seam.field.as_deref() {
        for section in involved.iter().flat_map(|dataset| dataset.color.working_sections.iter()) {
            if section.field == key && section.codes.iter().any(|code| logged(code)) && !sections.contains(&section.name.as_str()) {
                sections.push(section.name.as_str());
            }
        }
    }
    // A section and a code may share a name across datasets; the choice
    // says which it is, so the two never stand for each other.
    let known = |target: &ReferenceTarget| match target {
        ReferenceTarget::Section(name) => sections.contains(&name.as_str()),
        ReferenceTarget::Code(code) => logged(code),
    };
    if !seam.value.as_ref().is_some_and(known) {
        seam.value = sections
            .first()
            .map(|name| ReferenceTarget::Section((*name).to_owned()))
            .or_else(|| categories.first().map(|code| ReferenceTarget::Code(code.clone())));
    }
    let value_label = seam.value.as_ref().map_or_else(|| tr!("reference-points-no-values"), ReferenceTarget::label);
    let options = sections
        .iter()
        .map(|name| ReferenceTarget::Section((*name).to_owned()))
        .chain(categories.iter().map(|code| ReferenceTarget::Code(code.clone())))
        .map(|target| {
            let label = target.label().into();
            (Some(target), label)
        })
        .collect::<Vec<_>>();
    MenuFieldCombo::new((id, "value"), tr!("reference-points-working-section"), &mut seam.value, value_label, options).show(ui);

    let side_label = seam.side.label();
    MenuFieldCombo::new(
        (id, "side"),
        tr!("reference-points-side"),
        &mut seam.side,
        side_label,
        ReferenceSide::ALL.iter().map(|side| (*side, side.label().into())),
    )
    .show(ui);
}
