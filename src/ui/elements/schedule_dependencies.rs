//! The Dependencies step: ground that is not dug until other ground is.
//!
//! A list of rules and the selected rule's two sides, each chosen from the
//! Solids run's pits, benches and flitches the way routing rules choose
//! their sources.

use super::schedule_destinations::{scope_key, visible};
use crate::{
    i18n::tr,
    model::schedule::{SchedulePlan, SourceScope, dependencies::GroundDependency},
    ui::{
        EditorState,
        state::{ScheduleEdit, SourceScopeView, UiCommand},
        widgets::{
            context_menu::{ChecklistRow, ContextMenuAction, Tick, checklist_popup, context_menu_popup},
            data_grid::{DataGrid, GridRow, PropertyTable, grid_add_action_row, grid_row, grid_select_row},
            hint::Hint,
            menu,
        },
    },
};

/// A scope written as the app writes ground: `Pit A`, `Pit A/392`, or for a
/// flitch `Pit A/392/*/396`.
pub(crate) fn scope_path(sources: &[SourceScopeView], scope: SourceScope) -> String {
    let Some(index) = sources.iter().position(|view| view.scope == scope) else {
        return tr!("destination-source-unplaced");
    };
    let mut parts = vec![sources[index].label.clone()];
    let mut depth = sources[index].depth;
    for above in sources[..index].iter().rev() {
        if above.depth < depth {
            parts.push(above.label.clone());
            depth = above.depth;
        }
    }
    parts.reverse();
    if parts.len() == 3 {
        parts.insert(2, "*".to_owned());
    }
    super::solids_view::ground_path(&parts)
}

fn side_label(sources: &[SourceScopeView], side: Option<SourceScope>) -> String {
    side.map_or_else(|| tr!("dependency-not-chosen"), |scope| scope_path(sources, scope))
}

/// A rule as "ground after ground", or as a new one before either side is
/// chosen.
fn rule_label(sources: &[SourceScopeView], rule: &GroundDependency) -> String {
    if rule.ground.is_none() && rule.after.is_none() {
        return tr!("dependency-new-rule");
    }
    tr!("dependency-row", ground = side_label(sources, rule.ground), after = side_label(sources, rule.after))
}

/// The rule list: each rule as "ground after ground".
pub(crate) fn draw_rule_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let sources = editor.schedule_routing_sources.clone();
    let rules = plan.dependencies();
    let mut selected = editor.schedule_selected_dependency;
    DataGrid::new("schedule_dependency_list", rect, &tr!("dependency-step"))
        .column_header(&tr!("dependency-rule"))
        .show(ui, |ui| {
            for rule in &rules.rules {
                let label = rule_label(&sources, rule);
                let response = grid_row(ui, GridRow::new(&label).selected(selected == Some(rule.id)));
                if response.clicked() {
                    selected = Some(rule.id);
                }
                context_menu_popup(&response, &label, |ui| {
                    if ContextMenuAction::new(tr!("dependency-delete")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteDependency(rule.id)));
                        ui.close();
                    }
                });
            }
            if grid_add_action_row(ui, &tr!("dependency-new")) {
                commands.push(UiCommand::schedule(session, ScheduleEdit::AddDependency));
            }
        });
    if selected.is_none_or(|id| rules.rule(id).is_none()) {
        selected = rules.rules.first().map(|rule| rule.id);
    }
    editor.schedule_selected_dependency = selected;
}

/// The selected rule's two sides.
pub(crate) fn draw_rule_editor(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let Some(rule) = editor.schedule_selected_dependency.and_then(|id| plan.dependencies().rule(id)).copied() else {
        PropertyTable::new("schedule_dependency_empty", rect, &tr!("dependency-step")).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("dependency-rule"), &tr!("dependency-select"), None, None).hint(tr!("dependency-help"));
        });
        return;
    };
    let sources = editor.schedule_routing_sources.clone();
    let mut expanded = editor.schedule_source_expanded.clone();
    let mut next: Option<GroundDependency> = None;
    let title = rule_label(&sources, &rule);
    DataGrid::new("schedule_dependency_editor", rect, &title)
        .column_header(&tr!("dependency-rule"))
        .show(ui, |ui| {
            for waits in [true, false] {
                let (label, held) = if waits {
                    (tr!("dependency-ground"), rule.ground)
                } else {
                    (tr!("dependency-after"), rule.after)
                };
                let response = grid_select_row(ui, ("schedule_dependency_side", rule.id.0, waits), &label, &side_label(&sources, held), 0);
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
                        let acted = ChecklistRow::new(&view.label, Tick::of(held == Some(view.scope), false))
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
                        if acted.toggled && held != Some(view.scope) {
                            let mut changed = rule;
                            if waits {
                                changed.ground = Some(view.scope);
                            } else {
                                changed.after = Some(view.scope);
                            }
                            next = Some(changed);
                            ui.close();
                        }
                    }
                });
                response.hint(if waits { tr!("dependency-ground-hint") } else { tr!("dependency-after-hint") });
            }
        });
    editor.schedule_source_expanded = expanded;
    if let Some(changed) = next {
        commands.push(UiCommand::schedule(
            session,
            ScheduleEdit::SetDependency {
                id: changed.id,
                ground: changed.ground,
                after: changed.after,
            },
        ));
    }
}
