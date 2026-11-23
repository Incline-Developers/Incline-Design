//! The reference points dialog: one working section, one side, one layer of
//! derived points, on the holes selected when it opened or picked since.

use std::collections::BTreeSet;

use crate::{
    i18n::tr,
    model::drill_hole::{DrillField, DrillFieldKind, DrillHole, DrillHoleRef, DrillValue, OpenDrillHoleDataset, ReferenceSide, ReferenceTarget},
    ui::{
        state::{CodesRead, EditorState, SeamChoice, SelectionPickTool, UiCommand},
        widgets::{
            hint::Hint,
            menu::{self, DragableMenu, MenuButton, MenuFieldCombo, picked_selection_field},
        },
    },
};

pub(crate) fn draw_reference_points_dialog(ui: &mut egui::Ui, editor: &mut EditorState, datasets: &[OpenDrillHoleDataset], commands: &mut Vec<UiCommand>) {
    // Hidden while its Pick has the viewport.
    if editor.selection_pick.is_some() {
        return;
    }
    let Some(draft) = editor.reference_points_dialog.as_mut() else {
        return;
    };
    // The holes come in from the selection, so the datasets they name are
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
        [] => tr!("reference-points-no-holes"),
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
    // place. Read again only when the field or the datasets change.
    if let Some(field) = draft.seam.field.clone()
        && !codes_current(draft.codes.as_ref(), &field, &involved)
    {
        let codes = logged_codes(draft.holes.iter().filter_map(|reference| hole_of(&involved, *reference)), &field);
        draft.codes = Some((codes_read(field, &involved), codes));
    }
    let categories = draft.codes.as_ref().map(|(_, codes)| codes.clone()).unwrap_or_default();
    let mut open = true;
    let mut make = false;
    let mut cancel = false;
    DragableMenu::new("reference_points_dialog", tr!("reference-points-reference-points"))
        .open(&mut open)
        .min_width(380.0)
        .max_width(460.0)
        .show(ui.ctx(), |ui| {
            if picked_selection_field(ui, tr!("reference-points-holes"), holes_label.clone(), tr!("reference-points-holes-help"), 220.0) {
                commands.push(UiCommand::BeginSelectionPick(SelectionPickTool::ReferencePoints));
            }
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
            } else {
                seam_controls(ui, "reference_points", &involved, &categories, &mut draft.seam);
                ui.small(tr!("reference-points-one-point-per-hole-boundary"));
            }
            let has_holes = draft.holes.iter().any(|reference| hole_of(&involved, *reference).is_some());
            let blocker = reference_points_blocker(has_holes, draft.collars, draft.seam.value.is_some());
            menu::menu_actions(ui, |ui| {
                let confirm = blocker.is_none() && menu::dialog_confirmed(ui.ctx());
                let button = ui.add(MenuButton::new(tr!("reference-points-make")).primary().enabled(blocker.is_none()));
                if let Some(reason) = &blocker {
                    button.hint(reason);
                } else if button.clicked() || confirm {
                    make = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    cancel = true;
                }
            });
        });
    // Closed by the command once the points are made.
    if make {
        if draft.collars {
            commands.push(UiCommand::BuildCollarPoints { holes: draft.holes.clone() });
        } else if let (Some(field), Some(target)) = (draft.seam.field.clone(), draft.seam.value.clone()) {
            editor.last_seam = Some(draft.seam.clone());
            commands.push(UiCommand::BuildReferencePoints {
                holes: draft.holes.clone(),
                field,
                target,
                side: draft.seam.side,
            });
        }
    }
    if !open || cancel {
        editor.reference_points_dialog = None;
    }
}

/// Why Make waits, if it does: holes to place points on, and for points at
/// a logged pick a section or code to place them on.
fn reference_points_blocker(has_holes: bool, collars: bool, has_section: bool) -> Option<String> {
    if !has_holes {
        return Some(tr!("reference-points-blocker-no-holes"));
    }
    (!collars && !has_section).then(|| tr!("reference-points-blocker-no-section"))
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

/// What codes logged in `field` are read from: the field and the datasets
/// `involved` as they stand.
pub(crate) fn codes_read(field: String, involved: &[&OpenDrillHoleDataset]) -> CodesRead {
    (field, involved.iter().map(|dataset| (dataset.id, dataset.state.revision())).collect())
}

/// Whether `codes` were read from `field` and the datasets `involved` as
/// they stand now.
pub(crate) fn codes_current(codes: Option<&(CodesRead, Vec<String>)>, field: &str, involved: &[&OpenDrillHoleDataset]) -> bool {
    codes.is_some_and(|((read, datasets), _)| read == field && datasets.iter().copied().eq(involved.iter().map(|dataset| (dataset.id, dataset.state.revision()))))
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
    if seam_value_controls(ui, id, involved, categories, seam, true) {
        return;
    }
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

/// Every categorical field the datasets carry, named once where two
/// datasets log the same one.
pub(crate) fn categorical_fields<'a>(involved: &[&'a OpenDrillHoleDataset]) -> Vec<&'a DrillField> {
    let mut fields: Vec<&DrillField> = Vec::new();
    for dataset in involved {
        for field in dataset.dataset.fields.iter().filter(|field| matches!(field.kind, DrillFieldKind::Categorical { .. })) {
            if !fields.iter().any(|seen| seen.key == field.key) {
                fields.push(field);
            }
        }
    }
    fields
}

/// The working sections on `field` named on the datasets, once each, that
/// hold a code among `categories`.
pub(crate) fn logged_sections<'a>(involved: &[&'a OpenDrillHoleDataset], field: &str, categories: &[String]) -> Vec<&'a str> {
    let mut sections: Vec<&str> = Vec::new();
    for section in involved.iter().flat_map(|dataset| dataset.color.working_sections.iter()) {
        if section.field == field && section.codes.iter().any(|code| categories.contains(code)) && !sections.contains(&section.name.as_str()) {
            sections.push(section.name.as_str());
        }
    }
    sections
}

/// The section, else the code, named exactly `name`, as the seam controls
/// list them; `None` without an exact match.
pub(crate) fn seam_preset(name: &str, sections: &[&str], codes: &[String]) -> Option<ReferenceTarget> {
    if sections.contains(&name) {
        return Some(ReferenceTarget::Section(name.to_owned()));
    }
    codes.iter().any(|code| code == name).then(|| ReferenceTarget::Code(name.to_owned()))
}

/// The field and section half of [`seam_controls`]. With `fill_first` a
/// choice that is not valid becomes the first section or code; without it,
/// none until the user picks. Whether the field changed this frame.
pub(crate) fn seam_value_controls(ui: &mut egui::Ui, id: &str, involved: &[&OpenDrillHoleDataset], categories: &[String], seam: &mut SeamChoice, fill_first: bool) -> bool {
    let fields = categorical_fields(involved);
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
        return true;
    }
    let logged = |code: &str| categories.iter().any(|category| category == code);
    // Working sections named on the datasets come first: one of them picks
    // its codes as one run. Only those the holes log show.
    let sections = seam.field.as_deref().map(|key| logged_sections(involved, key, categories)).unwrap_or_default();
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
            .or_else(|| categories.first().map(|code| ReferenceTarget::Code(code.clone())))
            .filter(|_| fill_first);
    }
    let options = sections
        .iter()
        .map(|name| ReferenceTarget::Section((*name).to_owned()))
        .chain(categories.iter().map(|code| ReferenceTarget::Code(code.clone())))
        .map(|target| {
            let label = target.label().into();
            (Some(target), label)
        })
        .collect::<Vec<_>>();
    let value_label = match &seam.value {
        Some(target) => target.label(),
        None if options.is_empty() => tr!("reference-points-no-values"),
        None => String::new(),
    };
    MenuFieldCombo::new((id, "value"), tr!("reference-points-working-section"), &mut seam.value, value_label, options).show(ui);
    false
}
