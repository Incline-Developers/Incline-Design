//! The Loader Limits step: how many loaders may work in one area at a time.
//!
//! A list of rules and the selected rule's ground - chosen from the Solids
//! run's pits, benches and flitches the way dependencies choose theirs -
//! whether the limit is on all of it, each bench in it or each blast in it,
//! and how many loaders.

use super::schedule_destinations::{scope_key, visible};
use crate::{
    i18n::tr,
    model::schedule::{
        SchedulePlan,
        loader_limits::{LimitArea, LoaderLimit},
    },
    ui::{
        EditorState,
        state::{ScheduleEdit, SourceScopeView, UiCommand},
        widgets::{
            context_menu::{ChecklistRow, ContextMenuAction, Tick, checklist_popup, context_menu_popup},
            data_grid::{DataGrid, GridRow, PropertyTable, grid_add_action_row, grid_cell_entry, grid_choice_row, grid_named_row, grid_row, grid_select_row},
            hint::Hint,
            menu,
        },
    },
};

/// A rule as "at most N loaders on each bench of Pit A", or as a new one
/// before its ground is chosen.
fn rule_label(sources: &[SourceScopeView], rule: &LoaderLimit) -> String {
    let Some(scope) = rule.scope else {
        return tr!("loader-limit-new-rule");
    };
    let ground = super::schedule_dependencies::scope_path(sources, scope);
    let loaders = rule.loaders;
    match rule.area {
        LimitArea::Whole => tr!("loader-limit-row-whole", loaders = loaders, ground = ground),
        LimitArea::EachBench => tr!("loader-limit-row-bench", loaders = loaders, ground = ground),
        LimitArea::EachBlast => tr!("loader-limit-row-blast", loaders = loaders, ground = ground),
    }
}

/// The rule list.
pub(crate) fn draw_rule_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let sources = editor.schedule_routing_sources.clone();
    let rules = plan.loader_limits();
    let mut selected = editor.schedule_selected_loader_limit;
    DataGrid::new("schedule_loader_limit_list", rect, &tr!("loader-limit-step"))
        .column_header(&tr!("loader-limit-rule"))
        .show(ui, |ui| {
            for rule in &rules.rules {
                let label = rule_label(&sources, rule);
                let response = grid_row(ui, GridRow::new(&label).selected(selected == Some(rule.id)));
                if response.clicked() {
                    selected = Some(rule.id);
                }
                context_menu_popup(&response, &label, |ui| {
                    if ContextMenuAction::new(tr!("loader-limit-delete")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteLoaderLimit(rule.id)));
                        ui.close();
                    }
                });
            }
            if grid_add_action_row(ui, &tr!("loader-limit-new")) {
                commands.push(UiCommand::schedule(session, ScheduleEdit::AddLoaderLimit));
            }
        });
    if selected.is_none_or(|id| rules.rule(id).is_none()) {
        selected = rules.rules.first().map(|rule| rule.id);
    }
    editor.schedule_selected_loader_limit = selected;
}

/// The selected rule's ground, area and loaders.
pub(crate) fn draw_rule_editor(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let Some(rule) = editor.schedule_selected_loader_limit.and_then(|id| plan.loader_limits().rule(id)).copied() else {
        PropertyTable::new("schedule_loader_limit_empty", rect, &tr!("loader-limit-step")).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("loader-limit-rule"), &tr!("loader-limit-select"), None, None)
                .hint(tr!("loader-limit-help"));
        });
        return;
    };
    let sources = editor.schedule_routing_sources.clone();
    let mut expanded = editor.schedule_source_expanded.clone();
    let mut changed = rule;
    let title = rule_label(&sources, &rule);
    DataGrid::new("schedule_loader_limit_editor", rect, &title)
        .column_header(&tr!("loader-limit-rule"))
        .show(ui, |ui| {
            let label = tr!("loader-limit-ground");
            let held = rule
                .scope
                .map_or_else(|| tr!("dependency-not-chosen"), |scope| super::schedule_dependencies::scope_path(&sources, scope));
            let response = grid_select_row(ui, ("schedule_loader_limit_ground", rule.id.0), &label, &held, 0);
            checklist_popup(&response, label.clone(), 260.0, |ui| {
                if sources.is_empty() {
                    menu::menu_note(ui, tr!("destination-no-sources"));
                }
                for (index, view) in sources.iter().enumerate() {
                    if !visible(&sources, index, &expanded) {
                        continue;
                    }
                    let children = sources.get(index + 1).is_some_and(|below| below.depth > view.depth);
                    let key = scope_key(view.scope);
                    let acted = ChecklistRow::new(&view.label, Tick::of(rule.scope == Some(view.scope), false))
                        .depth(view.depth)
                        .disclosure(children.then(|| expanded.contains(&key)))
                        .show(ui);
                    if acted.expanded {
                        match expanded.iter().position(|open| *open == key) {
                            Some(position) => {
                                expanded.remove(position);
                            }
                            None => expanded.push(key),
                        }
                    }
                    if acted.toggled && rule.scope != Some(view.scope) {
                        changed.scope = Some(view.scope);
                        ui.close();
                    }
                }
            });
            response.hint(tr!("loader-limit-ground-hint"));

            let mut area = rule.area;
            if grid_choice_row(
                ui,
                ("schedule_loader_limit_area", rule.id.0),
                &tr!("loader-limit-area"),
                &mut area,
                LimitArea::ALL,
                LimitArea::label,
                0,
            ) {
                changed.area = area;
            }

            let (cell, row) = grid_named_row(ui, &tr!("loader-limit-loaders"), 0);
            let mut loaders = f64::from(rule.loaders);
            if cell.is_positive() && grid_cell_entry(ui, ("schedule_loader_limit_loaders", rule.id.0), cell, &mut loaders, "") && loaders.is_finite() {
                changed.loaders = loaders.round().clamp(0.0, f64::from(u32::MAX)) as u32;
            }
            row.hint(tr!("loader-limit-loaders-hint"));
        });
    editor.schedule_source_expanded = expanded;
    if changed != rule {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetLoaderLimit(changed)));
    }
}
