//! The reference points dialog: one working section, one side, one layer of
//! derived points, on the holes the dialog was opened on.

use std::collections::BTreeSet;

use crate::{
    i18n::{tr, tr_format},
    model::drill_hole::{DrillField, DrillFieldKind, DrillValue, OpenDrillHoleDataset, ReferenceSide, ReferenceTarget},
    ui::{
        state::{EditorState, UiCommand},
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
        [dataset] => tr_format!(literal = "%count% holes from '%dataset%'", count = draft.holes.len(), dataset = dataset.name.clone()),
        involved => tr_format!(literal = "%count% holes from %datasets% datasets", count = draft.holes.len(), datasets = involved.len()),
    };
    // Every categorical field the selected holes' datasets carry, named once
    // where two datasets log the same one.
    let mut fields: Vec<&DrillField> = Vec::new();
    for dataset in &involved {
        for field in dataset.dataset.fields.iter().filter(|field| matches!(field.kind, DrillFieldKind::Categorical { .. })) {
            if !fields.iter().any(|seen| seen.key == field.key) {
                fields.push(field);
            }
        }
    }
    let mut open = true;
    let mut build = None;
    DragableMenu::new("reference_points_dialog", tr!(literal = "Reference Points"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            selected_source_field(
                ui,
                tr!(literal = "Holes"),
                holes_label.clone(),
                tr!(literal = "The holes the points are placed on, as selected when the dialog opened. Close the dialog to select different ones."),
                220.0,
            );
            ui.add_space(4.0);

            // The geologist names the categorical field that carries the
            // working section.
            if !draft.field.as_deref().is_some_and(|key| fields.iter().any(|field| field.key == key)) {
                draft.field = fields.first().map(|field| field.key.clone());
                draft.value = None;
            }
            let field_label = draft
                .field
                .as_deref()
                .and_then(|key| fields.iter().find(|field| field.key == key))
                .map(|field| field.label.clone())
                .unwrap_or_else(|| tr!(literal = "No categorical field"));
            if MenuFieldCombo::new(
                "reference_points_field",
                tr!(literal = "Working section field"),
                &mut draft.field,
                field_label,
                fields.iter().map(|field| (Some(field.key.clone()), field.label.clone().into())),
            )
            .show(ui)
            .changed()
            {
                draft.value = None;
            }

            // The values the selected holes actually log, not every value the
            // datasets declare: a section none of these holes hold has no
            // points to place.
            let categories: Vec<&str> = match draft.field.as_deref() {
                None => Vec::new(),
                Some(key) => {
                    let mut found: BTreeSet<&str> = BTreeSet::new();
                    for reference in &draft.holes {
                        let Some(hole) = involved
                            .iter()
                            .find(|dataset| dataset.id == reference.dataset)
                            .and_then(|dataset| dataset.dataset.holes.get(reference.hole))
                        else {
                            continue;
                        };
                        for interval in &hole.intervals {
                            if let Some(DrillValue::Category(code)) = interval.values.get(key)
                                && !code.trim().is_empty()
                            {
                                found.insert(code.as_str());
                            }
                        }
                    }
                    found.into_iter().collect()
                }
            };
            // Working sections named on the datasets come first: one of them
            // picks its codes as one run. Only those these holes log show.
            let mut sections: Vec<&str> = Vec::new();
            if let Some(key) = draft.field.as_deref() {
                for section in involved.iter().flat_map(|dataset| dataset.color.working_sections.iter()) {
                    if section.field == key && section.codes.iter().any(|code| categories.contains(&code.as_str())) && !sections.contains(&section.name.as_str()) {
                        sections.push(section.name.as_str());
                    }
                }
            }
            // A section and a code may share a name across datasets; the
            // choice says which it is, so the two never stand for each other.
            let known = |target: &ReferenceTarget| match target {
                ReferenceTarget::Section(name) => sections.contains(&name.as_str()),
                ReferenceTarget::Code(code) => categories.contains(&code.as_str()),
            };
            if !draft.value.as_ref().is_some_and(known) {
                draft.value = sections
                    .first()
                    .map(|name| ReferenceTarget::Section((*name).to_owned()))
                    .or_else(|| categories.first().map(|code| ReferenceTarget::Code((*code).to_owned())));
            }
            let value_label = draft.value.as_ref().map_or_else(|| tr!(literal = "No values"), ReferenceTarget::label);
            let options = sections
                .iter()
                .map(|name| ReferenceTarget::Section((*name).to_owned()))
                .chain(categories.iter().map(|code| ReferenceTarget::Code((*code).to_owned())))
                .map(|target| {
                    let label = target.label().into();
                    (Some(target), label)
                })
                .collect::<Vec<_>>();
            MenuFieldCombo::new("reference_points_value", tr!(literal = "Working section"), &mut draft.value, value_label, options).show(ui);

            let side_label = draft.side.label();
            MenuFieldCombo::new(
                "reference_points_side",
                tr!(literal = "Side"),
                &mut draft.side,
                side_label,
                ReferenceSide::ALL.iter().map(|side| (*side, side.label().into())),
            )
            .show(ui);

            ui.small(tr!(
                literal = "One point per hole at that boundary, as a new layer. A hole holding the section twice gives its uppermost and is flagged."
            ));
            if ui.add(MenuButton::new(tr!(literal = "Make")).primary().enabled(draft.value.is_some())).clicked()
                && let (Some(field), Some(target)) = (draft.field.clone(), draft.value.clone())
            {
                build = Some((field, target, draft.side));
            }
        });
    if let Some((field, target, side)) = build {
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
