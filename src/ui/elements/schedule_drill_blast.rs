//! Schedule → Setup → Drill & Blast: the settings every blast is worked out
//! with, and the stage each blast starts the schedule at.
//!
//! The blasts are the Solids run's, listed by solid and bench. A blast's
//! starting stage is set in its row, or for a whole bench or solid from the
//! heading's menu. Its own pattern, where it differs from the default, is set
//! beside the settings once it is selected. See `docs/scheduling-drill-blast.md`.

use crate::{
    i18n::tr,
    model::schedule::{BlastRef, BlastStage, DrillBlastSettings, DrillPattern, SchedulePlan},
    ui::{
        EditorState,
        state::{BlastListEntry, BlastPatternDraft, DrillBlastDraft, ScheduleEdit, UiCommand},
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::{DataGrid, PropertyRows, PropertyTable, grid_cell_combo, grid_columns_row, grid_empty_state, grid_group_row_response, property_table_height},
        },
    },
};

/// Shares of the list's columns: the blast, then the stage it starts at.
const FRACTIONS: [f32; 2] = [0.5, 0.5];

/// Indexes into [`DrillBlastDraft::fields`].
const BURDEN: usize = 0;
const SPACING: usize = 1;
const SUBDRILL: usize = 2;
const DIAMETER: usize = 3;
const STEMMING: usize = 4;
const DENSITY: usize = 5;
const BUFFER: usize = 6;
const OPENS: usize = 7;
const CLOSES: usize = 8;

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

/// How many blasts a heading holds, and the stage they start at when they
/// all start at the same one.
fn group_detail(plan: &SchedulePlan, blasts: &[BlastListEntry]) -> String {
    let count = tr!("planning-blast-count", count = blasts.len());
    let first = blasts.first().map(|entry| stage_of(plan, entry));
    match first {
        Some(stage) if blasts.iter().all(|entry| stage_of(plan, entry) == stage) => format!("{count} · {}", stage.label()),
        _ => count,
    }
}

/// A solid or bench heading: open or closed, with a menu, titled by the
/// ground's whole `path`, setting where every blast under it starts.
#[allow(clippy::too_many_arguments)]
fn group_heading(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    path: &str,
    plan: &SchedulePlan,
    blasts: &[BlastListEntry],
    depth: usize,
    session: u32,
    commands: &mut Vec<UiCommand>,
) -> bool {
    let (open, response) = grid_group_row_response(ui, id, label, &group_detail(plan, blasts), depth);
    let response = response.on_hover_text(tr!("drill-blast-group-hint"));
    let members: Vec<BlastRef> = blasts.iter().map(|entry| entry.reference).collect();
    context_menu_popup(&response, path, |ui| stage_menu(ui, members.clone(), session, commands));
    open
}

/// Every blast of the run, by solid and bench, with the stage it starts at.
pub(crate) fn draw_blast_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let mut selected = editor.schedule_selected_blast;
    let blasts = editor.schedule_blasts.clone();
    let columns = [(tr!("solids-blast"), FRACTIONS[0]), (tr!("drill-blast-stage"), FRACTIONS[1])];
    DataGrid::new("schedule_blast_list", rect, &tr!("drill-blast-blasts")).columns(&columns).show(ui, |ui| {
        if blasts.is_empty() {
            grid_empty_state(ui, &tr!("drill-blast-no-blasts"), None);
            return;
        }
        // The run lists blasts by solid, then bench: each heading spans a run
        // of them.
        let end_of = |from: usize, same: &dyn Fn(&BlastListEntry) -> bool| blasts[from..].iter().position(|entry| !same(entry)).map_or(blasts.len(), |offset| from + offset);
        let mut solid_start = 0;
        while solid_start < blasts.len() {
            let solid = blasts[solid_start].reference.solid;
            let solid_end = end_of(solid_start, &|entry| entry.reference.solid == solid);
            let solid_name = blasts[solid_start].solid_name.clone();
            let solid_open = group_heading(
                ui,
                ("schedule_blasts_solid", solid),
                &solid_name,
                &solid_name,
                plan,
                &blasts[solid_start..solid_end],
                0,
                session,
                commands,
            );
            let mut bench_start = solid_start;
            while solid_open && bench_start < solid_end {
                let base = blasts[bench_start].bench_base;
                let bench_end = end_of(bench_start, &|entry| entry.reference.solid == solid && (entry.bench_base - base).abs() <= 1e-6).min(solid_end);
                let label = super::solids_view::format_rl(base);
                let path = super::solids_view::bench_path(&solid_name, base);
                let bench_open = group_heading(
                    ui,
                    ("schedule_blasts_bench", solid, base.to_bits()),
                    &label,
                    &path,
                    plan,
                    &blasts[bench_start..bench_end],
                    1,
                    session,
                    commands,
                );
                for (index, entry) in blasts.iter().enumerate().take(bench_end).skip(bench_start).filter(|_| bench_open) {
                    let stage = stage_of(plan, entry);
                    let is_selected = selected.is_some_and(|blast| entry.holds(&blast));
                    let (response, cells) = grid_columns_row(ui, &FRACTIONS, &[&entry.name, ""], is_selected);
                    if response.clicked() {
                        selected = Some(entry.reference);
                    }
                    let mut choice = stage;
                    if grid_cell_combo(
                        ui,
                        ("schedule_blast_stage", index),
                        cells[1],
                        &mut choice,
                        BlastStage::ALL.map(|stage| (stage, stage.label())),
                        &stage.label(),
                    ) && choice != stage
                    {
                        commands.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::SetBlastStatus {
                                blasts: vec![entry.reference],
                                stage: choice,
                            },
                        ));
                    }
                }
                bench_start = bench_end;
            }
            solid_start = solid_end;
        }
    });
    editor.schedule_selected_blast = selected;
}

fn number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|value| value.is_finite())
}

/// A time of day typed as `06:30`, `6` or `6.5`, in hours.
fn clock(text: &str) -> Option<f64> {
    let text = text.trim();
    let hours = match text.split_once(':') {
        Some((hours, minutes)) => {
            let (hours, minutes) = (hours.trim().parse::<u32>().ok()?, minutes.trim().parse::<u32>().ok()?);
            (minutes < 60).then(|| f64::from(hours) + f64::from(minutes) / 60.0)?
        }
        None => number(text)?,
    };
    (0.0..=24.0).contains(&hours).then_some(hours)
}

/// A time of day as [`clock`] reads it back: `06:30`.
fn clock_text(hours: f64) -> String {
    let minutes = (hours * 60.0).round() as i64;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// One typed setting: the draft's text, checked as `parse` reads it, and
/// written to `slot` once focus leaves it holding something new.
fn setting_row(rows: &mut PropertyRows<'_>, label: &str, text: &mut String, parse: fn(&str) -> Option<f64>, error: &str, slot: &mut f64, changed: &mut bool) {
    let value = parse(text);
    let response = rows.field(label, text, value.is_none().then_some(error));
    if response.lost_focus()
        && let Some(value) = value
        && *slot != value
    {
        *slot = value;
        *changed = true;
    }
}

/// The bench height a charge per hole is worked out for: the selected
/// blast's, or the height most of the run's benches share.
fn bench_height(editor: &EditorState) -> f64 {
    let height = |entry: &BlastListEntry| entry.bench_top - entry.bench_base;
    let selected = editor
        .schedule_selected_blast
        .and_then(|blast| editor.schedule_blasts.iter().find(|entry| entry.holds(&blast)));
    if let Some(entry) = selected {
        return height(entry);
    }
    let mut counts: Vec<(f64, usize)> = Vec::new();
    for entry in &editor.schedule_blasts {
        let value = height(entry);
        match counts.iter_mut().find(|(other, _)| (*other - value).abs() < 1e-6) {
            Some((_, count)) => *count += 1,
            None => counts.push((value, 1)),
        }
    }
    counts.into_iter().max_by_key(|(_, count)| *count).map_or(10.0, |(value, _)| value)
}

/// The settings, grouped, and beside them the selected blast's own.
pub(crate) fn draw_settings(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    let config = plan.drill_blast();
    let source = config.settings();
    if editor.drill_blast_draft.as_ref().is_none_or(|draft| draft.source != source) {
        let numbers = [
            source.pattern.burden_m,
            source.pattern.spacing_m,
            source.pattern.subdrill_m,
            source.hole_diameter_mm,
            source.stemming_m,
            source.product_density_t_m3,
            source.buffer_m,
        ];
        let mut fields = numbers.map(|value| value.to_string()).to_vec();
        fields.extend([clock_text(source.window_start_h), clock_text(source.window_end_h)]);
        editor.drill_blast_draft = Some(DrillBlastDraft {
            source,
            fields: fields.try_into().expect("nine settings"),
        });
    }
    let height = bench_height(editor);
    let draft = editor.drill_blast_draft.as_mut().expect("just ensured");
    let mut next = source;
    let mut changed = false;
    let gap = ui.spacing().item_spacing.y * 2.0;
    let general = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(rect.width(), property_table_height(ui, if source.enabled { 2 } else { 3 }).min(rect.height())),
    );
    PropertyTable::new("drill_blast_general", general, &tr!("drill-blast-step")).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        let mut enabled = source.enabled;
        if rows.checkbox(&tr!("drill-blast-enabled"), &mut enabled).changed() {
            next.enabled = enabled;
            changed = true;
        }
        if !source.enabled {
            rows.note(&tr!("drill-blast-off-note"));
        }
    });
    let number_error = tr!("drill-blast-number");
    let clock_error = tr!("drill-blast-clock");
    // Off, nothing below is used: every dig block is there from the start.
    let below = egui::Rect::from_min_max(egui::pos2(rect.left(), general.bottom() + gap), rect.max);
    if source.enabled && below.is_positive() {
        let column = (below.width() - gap) / 2.0;
        let left = egui::Rect::from_min_size(below.min, egui::vec2(column, below.height()));
        let right = egui::Rect::from_min_size(egui::pos2(left.right() + gap, below.top()), egui::vec2(column, below.height()));
        let table =
            |ui: &egui::Ui, area: egui::Rect, rows: usize| egui::Rect::from_min_size(area.min, egui::vec2(area.width(), property_table_height(ui, rows).min(area.height())));

        let pattern = table(ui, left, 5);
        PropertyTable::new("drill_blast_pattern", pattern, &tr!("drill-blast-pattern")).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            setting_row(
                rows,
                &tr!("drill-blast-burden"),
                &mut draft.fields[BURDEN],
                number,
                &number_error,
                &mut next.pattern.burden_m,
                &mut changed,
            );
            setting_row(
                rows,
                &tr!("drill-blast-spacing"),
                &mut draft.fields[SPACING],
                number,
                &number_error,
                &mut next.pattern.spacing_m,
                &mut changed,
            );
            setting_row(
                rows,
                &tr!("drill-blast-subdrill"),
                &mut draft.fields[SUBDRILL],
                number,
                &number_error,
                &mut next.pattern.subdrill_m,
                &mut changed,
            );
            let mut staggered = source.pattern.staggered;
            if rows.checkbox(&tr!("drill-blast-staggered"), &mut staggered).changed() {
                next.pattern.staggered = staggered;
                changed = true;
            }
        });

        let charge = table(ui, right, 5);
        PropertyTable::new("drill_blast_charge", charge, &tr!("drill-blast-holes")).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            setting_row(
                rows,
                &tr!("drill-blast-diameter"),
                &mut draft.fields[DIAMETER],
                number,
                &number_error,
                &mut next.hole_diameter_mm,
                &mut changed,
            );
            setting_row(
                rows,
                &tr!("drill-blast-stemming"),
                &mut draft.fields[STEMMING],
                number,
                &number_error,
                &mut next.stemming_m,
                &mut changed,
            );
            setting_row(
                rows,
                &tr!("drill-blast-density"),
                &mut draft.fields[DENSITY],
                number,
                &number_error,
                &mut next.product_density_t_m3,
                &mut changed,
            );
            let per_hole = config.charge_per_hole_t(height + source.pattern.subdrill_m);
            let label = tr!("drill-blast-charge-per-hole", bench = super::solids_view::format_rl(height));
            rows.readonly(&label, &format!("{per_hole:.2}"), Some("t"), None);
        });

        let timing_area = egui::Rect::from_min_max(egui::pos2(left.left(), pattern.bottom() + gap), left.max);
        if timing_area.is_positive() {
            let timing = table(ui, timing_area, if source.default_window { 7 } else { 5 });
            PropertyTable::new("drill_blast_timing", timing, &tr!("drill-blast-timing")).show(ui, |rows| {
                rows.header(&tr!("planning-property"), &tr!("planning-value"));
                setting_row(
                    rows,
                    &tr!("drill-blast-buffer"),
                    &mut draft.fields[BUFFER],
                    number,
                    &number_error,
                    &mut next.buffer_m,
                    &mut changed,
                );
                let mut default_window = source.default_window;
                if rows.checkbox(&tr!("drill-blast-default-window"), &mut default_window).changed() {
                    next.default_window = default_window;
                    changed = true;
                }
                if source.default_window {
                    setting_row(
                        rows,
                        &tr!("drill-blast-window-start"),
                        &mut draft.fields[OPENS],
                        clock,
                        &clock_error,
                        &mut next.window_start_h,
                        &mut changed,
                    );
                    setting_row(
                        rows,
                        &tr!("drill-blast-window-end"),
                        &mut draft.fields[CLOSES],
                        clock,
                        &clock_error,
                        &mut next.window_end_h,
                        &mut changed,
                    );
                }
                let added = config.windows.len().to_string();
                if config.effective_windows().is_empty() {
                    rows.readonly_warning(&tr!("blast-windows-label"), &added, None, Some(&tr!("drill-blast-no-windows")));
                } else {
                    rows.readonly(&tr!("blast-windows-label"), &added, None, None);
                }
                rows.note(&tr!("blast-windows-hint"));
            });
        }

        let blast_area = egui::Rect::from_min_max(egui::pos2(right.left(), charge.bottom() + gap), right.max);
        if blast_area.is_positive() {
            draw_blast_pattern(ui, blast_area, editor, plan, session, commands, source);
        }
    }
    if changed {
        commands.push(UiCommand::schedule(session, ScheduleEdit::SetDrillBlast(next)));
    }
}

/// The selected blast: its area and whether it has its own pattern.
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
        let table = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, 2).min(rect.height())));
        PropertyTable::new("drill_blast_blast", table, &tr!("solids-blast")).show(ui, |rows| rows.note(&tr!("drill-blast-select-hint")));
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
    let table = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(rect.width(), property_table_height(ui, if own.is_some() { 6 } else { 3 }).min(rect.height())),
    );
    let mut edit = None;
    PropertyTable::new("drill_blast_blast", table, &entry.label()).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        rows.readonly(&tr!("drill-blast-area"), &super::solids_view::format_area(entry.area), None, None);
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
