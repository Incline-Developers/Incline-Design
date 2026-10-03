//! Schedule → Setup → Drill & Blast: the settings every blast is worked out
//! with, and the stage each blast starts the schedule at.
//!
//! The blasts are the Solids run's, listed by bench. A blast's starting stage
//! is set from its row's menu, or for a whole bench from the bench's. Its own
//! pattern, where it differs from the default, is set beside the settings
//! once it is selected. See `docs/scheduling-drill-blast.md`.

use crate::{
    i18n::tr,
    model::schedule::{BlastRef, BlastStage, DrillBlastSettings, DrillPattern, SchedulePlan},
    ui::{
        EditorState,
        state::{BlastListEntry, BlastPatternDraft, DrillBlastDraft, ScheduleEdit, UiCommand},
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{DataGrid, GridRow, PropertyTable, grid_row, property_table_height},
            explorer::explorer_note,
        },
    },
};

/// The stage a planner set for `entry`, or Not started.
pub(crate) fn stage_of(plan: &SchedulePlan, entry: &BlastListEntry) -> BlastStage {
    plan.drill_blast()
        .statuses
        .iter()
        .find(|status| entry.holds(&status.blast))
        .map_or(BlastStage::NotStarted, |status| status.stage)
}

fn stage_menu(ui: &mut egui::Ui, blasts: Vec<BlastRef>, session: u32, commands: &mut Vec<UiCommand>) {
    for stage in BlastStage::ALL {
        if ContextMenuAction::new(tr!("blast-set-stage", stage = stage.label())).show(ui).clicked() {
            commands.push(UiCommand::schedule(session, ScheduleEdit::SetBlastStatus { blasts: blasts.clone(), stage }));
            ui.close();
        }
    }
}

/// Every blast of the run, by bench, with the stage it starts at.
pub(crate) fn draw_blast_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let mut selected = editor.schedule_selected_blast;
    let blasts = editor.schedule_blasts.clone();
    DataGrid::new("schedule_blast_list", rect, &tr!("drill-blast-blasts"))
        .column_header(&tr!("planning-name"))
        .show(ui, |ui| {
            if blasts.is_empty() {
                explorer_note(ui, tr!("drill-blast-no-blasts"));
            }
            let mut index = 0;
            while index < blasts.len() {
                let first = &blasts[index];
                let bench_end = blasts[index..]
                    .iter()
                    .position(|entry| entry.reference.solid != first.reference.solid || (entry.bench_base - first.bench_base).abs() > 1e-6)
                    .map_or(blasts.len(), |offset| index + offset);
                let heading = super::solids_view::bench_path(&first.solid_name, first.bench_base);
                // Header rows only sense hover; this one carries the bench's
                // stage menu, so it has to take the right-click too.
                let response = grid_row(ui, GridRow::header(&heading)).interact(egui::Sense::click());
                let members: Vec<BlastRef> = blasts[index..bench_end].iter().map(|entry| entry.reference).collect();
                context_menu_popup(&response, &heading, |ui| stage_menu(ui, members.clone(), session, commands));
                for entry in &blasts[index..bench_end] {
                    let stage = stage_of(plan, entry);
                    let label = format!("{} · {}", entry.name, stage.label());
                    let is_selected = selected.is_some_and(|blast| entry.holds(&blast));
                    let response = grid_row(ui, GridRow::new(&label).selected(is_selected));
                    if response.clicked() {
                        selected = Some(entry.reference);
                    }
                    context_menu_popup(&response, &entry.name, |ui| stage_menu(ui, vec![entry.reference], session, commands));
                }
                index = bench_end;
            }
        });
    editor.schedule_selected_blast = selected;
}

fn number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|value| value.is_finite())
}

/// The settings, and below them the selected blast's own.
pub(crate) fn draw_settings(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let config = plan.drill_blast();
    let source = config.settings();
    if editor.drill_blast_draft.as_ref().is_none_or(|draft| draft.source != source) {
        editor.drill_blast_draft = Some(DrillBlastDraft {
            source,
            fields: [
                source.pattern.burden_m,
                source.pattern.spacing_m,
                source.pattern.subdrill_m,
                source.hole_diameter_mm,
                source.stemming_m,
                source.product_density_t_m3,
                source.buffer_m,
                source.window_start_h,
                source.window_end_h,
            ]
            .map(|value| value.to_string()),
        });
    }
    let draft = editor.drill_blast_draft.as_mut().expect("just ensured");
    let mut next = source;
    let mut changed = false;
    let rows_used = 13;
    let table = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, rows_used).min(rect.height())));
    PropertyTable::new("drill_blast_settings", table, &tr!("drill-blast-step")).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let mut enabled = source.enabled;
        if rows.checkbox(&tr!("drill-blast-enabled"), &mut enabled).changed() {
            next.enabled = enabled;
            changed = true;
        }
        let labels = [
            tr!("drill-blast-burden"),
            tr!("drill-blast-spacing"),
            tr!("drill-blast-subdrill"),
            tr!("drill-blast-diameter"),
            tr!("drill-blast-stemming"),
            tr!("drill-blast-density"),
            tr!("drill-blast-buffer"),
            tr!("drill-blast-window-start"),
            tr!("drill-blast-window-end"),
        ];
        if let Some(windows) = &config.windows {
            rows.readonly(&tr!("blast-windows-label"), &tr!("blast-windows-summary", count = windows.len().to_string()), None, None);
        }
        for (index, label) in labels.iter().enumerate() {
            if index >= 7 && config.windows.is_some() {
                continue;
            }
            let value = number(&draft.fields[index]);
            let error = value.is_none().then(|| tr!("drill-blast-number"));
            let response = rows.field(label, &mut draft.fields[index], error.as_deref());
            if response.lost_focus()
                && let Some(value) = value
            {
                let slot = match index {
                    0 => &mut next.pattern.burden_m,
                    1 => &mut next.pattern.spacing_m,
                    2 => &mut next.pattern.subdrill_m,
                    3 => &mut next.hole_diameter_mm,
                    4 => &mut next.stemming_m,
                    5 => &mut next.product_density_t_m3,
                    6 => &mut next.buffer_m,
                    7 => &mut next.window_start_h,
                    _ => &mut next.window_end_h,
                };
                if *slot != value {
                    *slot = value;
                    changed = true;
                }
            }
        }
        let mut staggered = source.pattern.staggered;
        if rows.checkbox(&tr!("drill-blast-staggered"), &mut staggered).changed() {
            next.pattern.staggered = staggered;
            changed = true;
        }
        let per_hole = config.charge_per_hole_t(10.0 + source.pattern.subdrill_m);
        rows.readonly(&tr!("drill-blast-charge-per-hole"), &format!("{per_hole:.2}"), Some("t"), None);
    });
    if changed {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetDrillBlast(next)));
    }
    let below = egui::Rect::from_min_max(egui::pos2(rect.left(), table.bottom() + ui.spacing().item_spacing.y * 2.0), rect.max);
    if below.is_positive() {
        draw_blast_pattern(ui, below, editor, plan, session, commands, source);
    }
}

/// The selected blast: what it starts at and whether it has its own pattern.
fn draw_blast_pattern(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    session: u32,
    commands: &mut Vec<UiCommand>,
    settings: DrillBlastSettings,
) {
    let Some(entry) = editor
        .schedule_selected_blast
        .and_then(|blast| editor.schedule_blasts.iter().find(|entry| entry.holds(&blast)).cloned())
    else {
        return;
    };
    let config = plan.drill_blast();
    let own = config.patterns.iter().find(|pattern| entry.holds(&pattern.blast)).map(|pattern| pattern.pattern);
    let source = own.unwrap_or(settings.pattern);
    if editor
        .blast_pattern_draft
        .as_ref()
        .is_none_or(|draft| !draft.blast.same(&entry.reference) || draft.source != source)
    {
        editor.blast_pattern_draft = Some(BlastPatternDraft {
            blast: entry.reference,
            source,
            fields: [source.burden_m, source.spacing_m, source.subdrill_m].map(|value| value.to_string()),
        });
    }
    let draft = editor.blast_pattern_draft.as_mut().expect("just ensured");
    let stored = own.map(|_| {
        config
            .patterns
            .iter()
            .find(|pattern| entry.holds(&pattern.blast))
            .map_or(entry.reference, |pattern| pattern.blast)
    });
    let table = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, 7).min(rect.height())));
    let mut edit = None;
    PropertyTable::new("drill_blast_blast", table, &entry.label()).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let mut stage = stage_of(plan, &entry);
        let current = stage.label();
        if rows
            .combo(
                "drill_blast_stage",
                &tr!("drill-blast-stage"),
                &mut stage,
                &current,
                BlastStage::ALL.map(|stage| (stage, stage.label())),
            )
            .changed()
        {
            commands.push(UiCommand::schedule(
                session,
                ScheduleEdit::SetBlastStatus {
                    blasts: vec![entry.reference],
                    stage,
                },
            ));
        }
        rows.readonly(&tr!("drill-blast-area"), &format!("{:.0}", entry.area), Some("m²"), None);
        let mut has_own = own.is_some();
        if rows.checkbox(&tr!("drill-blast-own-pattern"), &mut has_own).changed() {
            edit = Some(if has_own { Some(settings.pattern) } else { None });
        }
        if own.is_some() {
            let labels = [tr!("drill-blast-burden"), tr!("drill-blast-spacing"), tr!("drill-blast-subdrill")];
            for (index, label) in labels.iter().enumerate() {
                let value = number(&draft.fields[index]);
                let error = value.is_none().then(|| tr!("drill-blast-number"));
                if rows.field(label, &mut draft.fields[index], error.as_deref()).lost_focus()
                    && let Some(value) = value
                {
                    let mut pattern: DrillPattern = source;
                    match index {
                        0 => pattern.burden_m = value,
                        1 => pattern.spacing_m = value,
                        _ => pattern.subdrill_m = value,
                    }
                    if pattern != source {
                        edit = Some(Some(pattern));
                    }
                }
            }
        }
    });
    if let Some(pattern) = edit {
        // A pattern already stored is replaced at the reference it was
        // stored under, so the old entry goes.
        let blast = stored.unwrap_or(entry.reference);
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetBlastPattern { blast, pattern }));
    }
}
