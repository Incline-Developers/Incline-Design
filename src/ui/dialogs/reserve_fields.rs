//! The Solids Setup's New Field dialog (Field List step).

use crate::{
    i18n::tr,
    model::{Document, ReserveAggregation},
    ui::{
        EditorState,
        state::{ReserveFieldKind, UiCommand},
        widgets::menu::{self, DragableMenu, MenuButton, MenuFieldCombo, MenuFieldText},
    },
};

fn kind_label(kind: ReserveFieldKind) -> String {
    match kind {
        ReserveFieldKind::Sum => tr!("planning-stat-sum"),
        ReserveFieldKind::Average => tr!("reserve-average"),
        ReserveFieldKind::Category => tr!("csv-block-model-category"),
    }
}

/// Draw the dialog that adds one field to the project's Reserves Field List.
///
/// A field is a name plus how it aggregates: summed outright, averaged
/// weighted by block volume or by another (necessarily summed) field - e.g. a
/// grade weighted by tonnes - or, for `Category`, not aggregated at all: a grouping label (e.g.
/// "Rock Type") each block model maps onto one of its own categorical
/// columns, for a future breakdown of the numeric fields by category.
pub(crate) fn draw_new_reserve_field_dialog(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) {
    if !editor.new_reserve_field_open {
        return;
    }
    let mut open = true;
    let mut close = false;
    DragableMenu::new("new_reserve_field_dialog", tr!("reserve-new-field"))
        .open(&mut open)
        .min_width(300.0)
        .show(ui.ctx(), |ui| {
            MenuFieldText::new(tr!("planning-name"), &mut editor.new_reserve_field_name)
                .hint_text(tr!("reserve-field-name-hint"))
                .show(ui);
            const KINDS: [ReserveFieldKind; 3] = [ReserveFieldKind::Sum, ReserveFieldKind::Average, ReserveFieldKind::Category];
            let selected_kind_text = kind_label(editor.new_reserve_field_kind);
            MenuFieldCombo::new(
                "new_reserve_field_kind",
                tr!("planning-combines-as"),
                &mut editor.new_reserve_field_kind,
                selected_kind_text,
                KINDS.into_iter().map(|kind| (kind, kind_label(kind).into())),
            )
            .show(ui);
            let sum_fields: Vec<_> = document
                .reserve_fields()
                .iter()
                .filter(|field| matches!(field.aggregation, ReserveAggregation::Sum))
                .collect();
            let note = match editor.new_reserve_field_kind {
                ReserveFieldKind::Sum => tr!("reserve-kind-sum-note"),
                ReserveFieldKind::Average => tr!("reserve-kind-average-note"),
                ReserveFieldKind::Category => tr!("reserve-kind-category-note"),
            };
            menu::menu_note(ui, note);
            if editor.new_reserve_field_kind == ReserveFieldKind::Average {
                // Weighted by block volume (`None`) or by a Sum field.
                if editor.new_reserve_field_weight_field.is_some_and(|id| !sum_fields.iter().any(|field| field.id == id)) {
                    editor.new_reserve_field_weight_field = None;
                }
                let weight_name = |weight: Option<crate::model::ReserveFieldId>| {
                    weight
                        .and_then(|id| sum_fields.iter().find(|field| field.id == id))
                        .map_or_else(|| tr!("reserve-block-volume"), |field| field.name.clone())
                };
                let selected_text = weight_name(editor.new_reserve_field_weight_field);
                MenuFieldCombo::new(
                    "new_reserve_field_weight",
                    tr!("reserve-weighted"),
                    &mut editor.new_reserve_field_weight_field,
                    selected_text,
                    std::iter::once(None)
                        .chain(sum_fields.iter().map(|field| Some(field.id)))
                        .map(|weight| (weight, weight_name(weight).into())),
                )
                .show(ui);
            }
            menu::menu_actions(ui, |ui| {
                let can_add = !editor.new_reserve_field_name.trim().is_empty();
                let submitted = menu::dialog_confirmed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("reserve-add-field")).primary().enabled(can_add)).clicked()) && can_add {
                    let aggregation = match editor.new_reserve_field_kind {
                        ReserveFieldKind::Sum => ReserveAggregation::Sum,
                        ReserveFieldKind::Average => match editor.new_reserve_field_weight_field {
                            Some(weight_field) => ReserveAggregation::WeightedAverage { weight_field },
                            None => ReserveAggregation::VolumeAverage,
                        },
                        ReserveFieldKind::Category => ReserveAggregation::Category,
                    };
                    commands.push(UiCommand::AddReserveField {
                        name: editor.new_reserve_field_name.trim().to_owned(),
                        aggregation,
                    });
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.new_reserve_field_open = false;
        editor.new_reserve_field_name.clear();
        editor.new_reserve_field_kind = ReserveFieldKind::Sum;
        editor.new_reserve_field_weight_field = None;
    }
}

/// Ask before deleting an unused reserve field, which cannot be undone.
pub(crate) fn draw_delete_reserve_field_dialog(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) {
    let Some(field) = editor.pending_delete_field.and_then(|id| document.reserve_fields().iter().find(|field| field.id == id)) else {
        editor.pending_delete_field = None;
        return;
    };
    let mut open = true;
    let mut close = false;
    DragableMenu::new("delete_reserve_field_dialog", tr!("reserve-field-delete-title", name = field.name.clone()))
        .open(&mut open)
        .min_width(320.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("reserve-field-delete-note"));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("planning-delete-field")).danger()).clicked() || menu::dialog_confirmed(ui.ctx()) {
                    commands.push(UiCommand::DeleteReserveField(field.id));
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.pending_delete_field = None;
    }
}

/// Ask before deleting a solid, which takes its benches, cut lines and dig
/// order references with it and cannot be undone.
pub(crate) fn draw_delete_solid_dialog(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) {
    let Some(solid) = editor.pending_delete_solid.and_then(|id| document.solids().iter().find(|solid| solid.id == id)) else {
        editor.pending_delete_solid = None;
        return;
    };
    let mut open = true;
    let mut close = false;
    DragableMenu::new("delete_solid_dialog", tr!("planning-delete-solid-title", name = solid.name.clone()))
        .open(&mut open)
        .min_width(320.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("planning-delete-solid-note"));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("planning-delete-solid")).danger()).clicked() || menu::dialog_confirmed(ui.ctx()) {
                    commands.push(UiCommand::DeleteSolid(solid.id));
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.pending_delete_solid = None;
    }
}
