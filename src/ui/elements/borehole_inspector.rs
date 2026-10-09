//! Docked panel on the window's right edge, showing everything known about
//! the currently inspected hole.
//!
//! Has a Data tab (summary and interval table), a Log tab (strip log) and a
//! Column tab (the set's strat column beside its working sections); the Log
//! tab's widget lives in [`crate::ui::widgets::borehole_log::BoreholeLog`].

use std::{fmt::Debug, hash::Hash};

use crate::{
    i18n::tr,
    model::{
        SceneEntityId,
        drill_hole::{CorrectionNote, DrillField, DrillFieldKind, DrillHoleId, DrillHoleRef, OpenDrillHoleDataset, RenameScope, WorkingSection, section_color_key},
        geophysics::{HoleView, LinkState},
        strat_order::{FlagKind, FlagLevel, HoleFlag},
    },
    ui::{
        EditorState,
        elements::properties::read_only_row,
        state::{BoreholeInspectorTab, NameShiftDraft, SeamRenameDraft, StratCheckReport, UiCommand},
        themed_icon, unthemed_icon,
        widgets::{
            borehole_log::{NameShift, SeamRename},
            collapsible_section::CollapsibleSection,
            data_table::DataTable,
            menu::{self, MenuButton, MenuField, MenuFieldCombo},
            toolbar::GROUP_CORNER_RADIUS,
            viewport::format_grade,
        },
    },
};

/// Id of the borehole inspector panel; `crate::ui::chrome` reads its resize
/// response to light up the grip.
pub(crate) const PANEL_ID: &str = "borehole_inspector_panel";

/// Default width: room for a two-column property grid, and for the Log tab's
/// density, strat and gamma columns at their narrowest beside the hole.
const DEFAULT_WIDTH: f32 = 360.0;
/// Space between the panel's edge and its contents.
const BODY_MARGIN: f32 = 8.0;
/// Narrowest width before rows would rather truncate than shrink further.
const MIN_WIDTH: f32 = 200.0;
/// Widest, so the Log tab's strip log fits every column at full width
/// without swallowing the scene.
const MAX_WIDTH: f32 = 760.0;
/// Share of the window the panel may take at most, so a small display
/// keeps a usable scene beside it.
const MAX_WINDOW_SHARE: f32 = 0.5;

/// The panel's widest for a window `window_width` wide: [`MAX_WIDTH`], or
/// half the window when that is less, but never under [`MIN_WIDTH`].
fn max_width(window_width: f32) -> f32 {
    (window_width * MAX_WINDOW_SHARE).clamp(MIN_WIDTH, MAX_WIDTH)
}

/// Draw the borehole inspector panel and return what it claimed.
pub(crate) fn draw_borehole_inspector(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    datasets: &[OpenDrillHoleDataset],
    well_logs: &crate::model::geophysics::GeophysicsSession,
    commands: &mut Vec<UiCommand>,
) -> egui::Rect {
    // Reuse the explorer's row colours so the two panels share a palette.
    let (surface, _stripe) = crate::ui::widgets::tree_row_colors(ui);
    let widest = max_width(ui.ctx().content_rect().width());
    egui::Panel::right(PANEL_ID)
        .resizable(true)
        .default_size(DEFAULT_WIDTH.min(widest))
        .min_size(MIN_WIDTH)
        .max_size(widest)
        .show_separator_line(crate::ui::chrome::show_separator_line(ui))
        .frame(crate::ui::chrome::region_frame(ui).fill(surface).inner_margin(egui::Margin::ZERO))
        .show(ui, |ui| {
            // Report exactly the rect we were offered rather than whatever
            // the tab content measures, so wide content cannot drag the
            // panel's width along with it.
            let body = ui.available_rect_before_wrap();
            ui.allocate_rect(body, egui::Sense::hover());
            let mut body_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("borehole_inspector_body")
                    .max_rect(body.shrink2(egui::vec2(BODY_MARGIN, BODY_MARGIN * 0.75)))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            body_ui.set_clip_rect(body.intersect(ui.clip_rect()));
            // The same controls the floating menus and settings pages use.
            menu::apply_menu_style(&mut body_ui, surface);
            draw_body(&mut body_ui, editor, datasets, well_logs, commands);
        })
        .response
        .rect
}

/// The panel's contents, drawn into the clipped child ui the caller sized.
///
/// The tab strip and hole name sit above the tab's sections, so neither
/// scrolls or folds out of view.
fn draw_body(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    datasets: &[OpenDrillHoleDataset],
    well_logs: &crate::model::geophysics::GeophysicsSession,
    commands: &mut Vec<UiCommand>,
) {
    draw_tab_strip(ui, editor);

    let Some((dataset, hole, hole_index)) = inspected_hole(editor, datasets) else {
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(tr!("borehole-inspector-no-hole-inspected")).weak());
        });
        return;
    };

    // Named once for both tabs: the hole, then the dataset it belongs to.
    ui.add(egui::Label::new(egui::RichText::new(&hole.dhid).strong()).truncate());
    ui.add(egui::Label::new(egui::RichText::new(dataset.name.clone()).weak()).truncate());
    ui.add_space(2.0);

    match editor.borehole_inspector_tab {
        BoreholeInspectorTab::Data => {
            // Columns come from the dataset, so every hole shows the same.
            let properties = DrillHoleProperties::new(("borehole_inspector", dataset.id, hole_index), hole, &dataset.dataset.fields);
            CollapsibleSection::new("borehole_inspector_summary", tr!("borehole-inspector-summary"))
                .default_open(true)
                .show(ui, |ui| ui.push_id((dataset.id, hole_index), |ui| properties.show_summary(ui)));
            if !hole.intervals.is_empty() {
                ui.add_space(4.0);
                CollapsibleSection::new("borehole_inspector_intervals", tr!("viewport-interval-data"))
                    .default_open(true)
                    .show(ui, |ui| {
                        // The table scrolls to fit what the panel has left,
                        // less the section's own bottom margin.
                        let max_height = (ui.available_height() - 12.0).max(0.0);
                        ui.push_id((dataset.id, hole_index), |ui| properties.show_intervals(ui, max_height));
                    });
            }
        }
        BoreholeInspectorTab::Log => {
            CollapsibleSection::new("borehole_inspector_log_display", tr!("borehole-inspector-display"))
                .default_open(true)
                .show(ui, |ui| draw_log_field_pickers(ui, editor, dataset, commands));
            ui.add_space(4.0);
            // Matched on the hole id exactly as the dataset spells it.
            let view = well_logs.view(dataset, &hole.dhid);
            if matches!(view, HoleView::Wanted) {
                commands.push(UiCommand::ReadHoleGeophysics {
                    dataset: dataset.id,
                    dhid: hole.dhid.clone(),
                });
            }
            let index = dataset.geophysics.as_deref();
            let (logs, linked, reading) = match view {
                HoleView::Shown(logs) => (Some(logs), true, None),
                HoleView::NotInFiles => (None, true, None),
                // Laid out from the index, so the readings only fill in.
                HoleView::Wanted | HoleView::Reading => (None, true, index.map(|link| link.kinds_of(&hole.dhid))),
                _ => (None, false, None),
            };
            // Only until shown: the index counts a stray reading the read
            // leaves out.
            let logged = matches!(view, HoleView::Wanted | HoleView::Reading)
                .then(|| index.and_then(|link| link.depths_of(&hole.dhid)))
                .flatten();
            draw_geophysics_note(ui, &view, dataset.id, commands);
            // The log's wheel zooms rather than scrolling an enclosing area,
            // so it takes the rest of the panel instead of sitting in one.
            let output = crate::ui::widgets::borehole_log::BoreholeLog::new(("borehole_log", dataset.id), hole, dataset)
                .strat_field(strat_choice_for(editor, dataset))
                .well_logs(logs, linked)
                .reading(reading)
                .logged_depths(logged)
                .well_log_style(editor.well_log_style)
                .show(ui);
            if let Some(style) = output.saved {
                commands.push(UiCommand::SetWellLogStyle(style));
            }
            if let Some(rename) = output.rename {
                editor.seam_rename_dialog = Some(seam_rename_draft(dataset, hole_index, rename));
            }
            if let Some(shift) = output.shift {
                editor.name_shift_dialog = Some(name_shift_draft(dataset, hole_index, shift));
            }
        }
        BoreholeInspectorTab::Column => {
            CollapsibleSection::new("borehole_inspector_column_display", tr!("borehole-inspector-display"))
                .default_open(true)
                .show(ui, |ui| draw_strat_field_picker(ui, editor, dataset));
            ui.add_space(4.0);
            let chosen = strat_choice_for(editor, dataset);
            match crate::ui::widgets::borehole_log::strat_field_of(dataset, chosen.as_deref()) {
                // Scrolls in what the panel has left, so a long column never
                // runs off the bottom of the window.
                Some(field) => {
                    egui::ScrollArea::vertical()
                        .id_salt(("borehole_inspector_column_scroll", dataset.id))
                        .max_height(ui.available_height().max(0.0))
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            draw_strat_check(ui, editor, dataset, field, commands);
                            ui.add_space(4.0);
                            draw_strat_column(ui, dataset, field, commands);
                            draw_strat_flags(ui, editor, dataset, field, commands);
                        });
                }
                None => {
                    ui.label(egui::RichText::new(tr!("borehole-inspector-no-categorical-field")).weak());
                }
            }
        }
    }
}

/// Most flagged holes listed at once; the count covers them all.
const FLAG_LIST_LIMIT: usize = 500;
/// Tallest the flagged holes' list grows before it scrolls on its own.
const FLAG_LIST_HEIGHT: f32 = 240.0;

/// The last check of `field` in `dataset`, when it read the holes as they
/// are now; a check of holes since edited is no answer.
fn current_strat_check<'a>(editor: &'a EditorState, dataset: &OpenDrillHoleDataset, field: &DrillField) -> (Option<&'a StratCheckReport>, bool) {
    let report = editor.strat_check(dataset.id, &field.key);
    let current = report.is_some_and(|report| std::ptr::eq(report.checked.as_ptr(), std::sync::Arc::as_ptr(&dataset.dataset)));
    (report, current)
}

/// How many holes `flags` name; they come sorted by hole.
fn flagged_holes(flags: &[HoleFlag]) -> usize {
    let mut holes = flags.iter().map(|flag| flag.hole).collect::<Vec<_>>();
    holes.dedup();
    holes.len()
}

/// The Check section: the button, what the last check found, and when the
/// order most holes give differs from the column, the two side by side
/// with an Accept that sets the column, one undo step.
fn draw_strat_check(ui: &mut egui::Ui, editor: &mut EditorState, dataset: &OpenDrillHoleDataset, field: &DrillField, commands: &mut Vec<UiCommand>) {
    let column = dataset.color.strat_column(&field.key);
    let (report, current) = current_strat_check(editor, dataset, field);
    let flags = report.filter(|_| current).and_then(|report| report.flags_for(column));
    let title = match flags {
        Some(flags) => tr!("borehole-inspector-check-flagged", count = flagged_holes(flags).to_string()),
        None => tr!("borehole-inspector-check"),
    };
    let mut comparing: Option<bool> = None;
    let mut checked = false;
    CollapsibleSection::new("borehole_inspector_check", title).default_open(true).show(ui, |ui| {
        if ui
            .add(MenuButton::new(tr!("borehole-inspector-check-column")))
            .on_hover_text(tr!("borehole-inspector-check-column-hint"))
            .clicked()
        {
            commands.push(UiCommand::CheckStratColumn {
                id: dataset.id,
                field: field.key.clone(),
            });
            checked = true;
        }
        let Some(report) = report else {
            menu::menu_note(ui, tr!("borehole-inspector-check-not-run"));
            return;
        };
        if !current {
            menu::menu_note(ui, tr!("borehole-inspector-check-stale"));
            return;
        }
        let Some(order) = &report.order else {
            menu::menu_note(ui, tr!("borehole-inspector-check-too-many-codes"));
            return;
        };
        let mut summary = match flags {
            Some(flags) => tr!(
                "borehole-inspector-check-summary",
                holes = report.holes.to_string(),
                flagged = flagged_holes(flags).to_string()
            ),
            None => tr!("borehole-inspector-check-column-changed"),
        };
        if report.overruled > 0 {
            summary = format!("{summary} {}", tr!("borehole-inspector-check-overruled", count = report.overruled.to_string()));
        }
        menu::menu_note(ui, summary);
        if column.is_empty() || order.as_slice() == column {
            return;
        }
        if !report.comparing {
            menu::menu_note(ui, tr!("borehole-inspector-check-order-differs"));
            if ui.add(MenuButton::new(tr!("borehole-inspector-check-show-differences"))).clicked() {
                comparing = Some(true);
            }
            return;
        }
        draw_order_differences(ui, dataset, column, order);
        ui.horizontal(|ui| {
            if ui.add(MenuButton::new(tr!("borehole-inspector-check-accept")).primary()).clicked() {
                commands.push(UiCommand::SetStratColumn {
                    id: dataset.id,
                    field: field.key.clone(),
                    codes: order.clone(),
                });
                comparing = Some(false);
            }
            if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() {
                comparing = Some(false);
            }
        });
    });
    // The flags are read against the hole's log, so go there at once.
    if checked {
        editor.borehole_inspector_tab = BoreholeInspectorTab::Log;
    }
    if let Some(open) = comparing
        && let Some(report) = editor.strat_checks.iter_mut().find(|report| report.dataset == dataset.id && report.field == field.key)
    {
        report.comparing = open;
    }
}

/// The column as it is beside the order proposed, place by place, with each
/// proposed code that would move marked.
fn draw_order_differences(ui: &mut egui::Ui, dataset: &OpenDrillHoleDataset, column: &[String], order: &[String]) {
    menu::menu_note(ui, tr!("borehole-inspector-check-differences-note"));
    let header = [
        tr!("borehole-inspector-check-place"),
        tr!("borehole-inspector-check-now"),
        tr!("borehole-inspector-check-proposed"),
        String::new(),
    ];
    let moved = tr!("borehole-inspector-check-moved");
    let cells = |row: usize| {
        let now = column.get(row).cloned().unwrap_or_default();
        let proposed = order.get(row).cloned().unwrap_or_default();
        let mark = if !proposed.is_empty() && column.get(row) != order.get(row) {
            moved.clone()
        } else {
            String::new()
        };
        vec![(row + 1).to_string(), now, proposed, mark]
    };
    let fingerprint = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        column.hash(&mut hasher);
        order.hash(&mut hasher);
        hasher.finish()
    };
    crate::ui::widgets::data_table::DataTable::new(("strat_check_differences", dataset.id), &header, column.len().max(order.len()), &cells)
        .fingerprint(fingerprint)
        .max_height(FLAG_LIST_HEIGHT)
        .show(ui);
}

/// The holes the last check found disagreeing with the column, each with
/// what kind and the codes involved; inspecting one sends it to this panel.
fn draw_strat_flags(ui: &mut egui::Ui, editor: &EditorState, dataset: &OpenDrillHoleDataset, field: &DrillField, commands: &mut Vec<UiCommand>) {
    let (report, current) = current_strat_check(editor, dataset, field);
    let Some(flags) = report.filter(|_| current).and_then(|report| report.flags_for(dataset.color.strat_column(&field.key))) else {
        return;
    };
    ui.add_space(4.0);
    CollapsibleSection::new(
        "borehole_inspector_flagged_holes",
        tr!("borehole-inspector-flagged-holes", count = flagged_holes(flags).to_string()),
    )
    .default_open(true)
    .show(ui, |ui| {
        if flags.is_empty() {
            menu::menu_note(ui, tr!("borehole-inspector-no-holes-flagged"));
            return;
        }
        if flags.len() > FLAG_LIST_LIMIT {
            menu::menu_note(
                ui,
                tr!("borehole-inspector-flags-first-shown", shown = FLAG_LIST_LIMIT.to_string(), count = flags.len().to_string()),
            );
        }
        egui::ScrollArea::vertical()
            .id_salt(("borehole_inspector_flags_scroll", dataset.id))
            .max_height(FLAG_LIST_HEIGHT)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (row, flag) in flags.iter().take(FLAG_LIST_LIMIT).enumerate() {
                    let dhid = dataset.dataset.holes.get(flag.hole).map_or("", |hole| hole.dhid.as_str());
                    let kind = match flag.level {
                        FlagLevel::Group => tr!("borehole-inspector-flag-of-groups", kind = flag_kind_label(flag.kind)),
                        FlagLevel::Name => flag_kind_label(flag.kind),
                    };
                    let label = format!("{dhid}  {kind}: {}", flag.codes.join(", "));
                    ui.push_id(("strat_flag", row), |ui| {
                        MenuField::new(label).show(ui, |ui, _, _| {
                            if ui.add(MenuButton::new(tr!("borehole-inspector-inspect-hole"))).clicked() {
                                commands.push(UiCommand::InspectDrillHole(DrillHoleRef {
                                    dataset: dataset.id,
                                    hole: flag.hole,
                                }));
                            }
                        });
                    });
                }
            });
    });
}

/// What a flag's kind is called in the list.
fn flag_kind_label(kind: FlagKind) -> String {
    match kind {
        FlagKind::OutOfPlace => tr!("borehole-inspector-flag-out-of-place"),
        FlagKind::Overturned => tr!("borehole-inspector-flag-overturned"),
        FlagKind::Repeat => tr!("borehole-inspector-flag-repeat"),
    }
}

/// Height of one code's row in the Column tab, the house button's and the
/// section blocks' unit.
const COLUMN_ROW_HEIGHT: f32 = 24.0;
/// Width of the working sections beside the column, at most.
const SECTION_LANE_WIDTH: f32 = 96.0;

/// The Column tab: the dataset's working sections of `field` beside its strat
/// column, top to bottom, then the field's codes the column does not list.
/// Every change goes out as the whole new column, one undo step each.
fn draw_strat_column(ui: &mut egui::Ui, dataset: &OpenDrillHoleDataset, field: &DrillField, commands: &mut Vec<UiCommand>) {
    let column = dataset.color.strat_column(&field.key);
    let codes: &[String] = match &field.kind {
        DrillFieldKind::Categorical { categories } => categories,
        _ => &[],
    };
    let mut edited: Option<Vec<String>> = None;
    CollapsibleSection::new("borehole_inspector_column", tr!("borehole-inspector-strat-column"))
        .default_open(true)
        .show(ui, |ui| {
            if column.is_empty() {
                menu::menu_note(ui, tr!("borehole-inspector-column-empty-check"));
                return;
            }
            let lane = SECTION_LANE_WIDTH.min(ui.available_width() * 0.3);
            let height = COLUMN_ROW_HEIGHT * column.len() as f32;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
            let sections = egui::Rect::from_min_size(rect.min, egui::vec2(lane, height));
            draw_section_blocks(ui, sections, dataset, &field.key, column);
            let rows = egui::Rect::from_min_max(egui::pos2(sections.right() + 6.0, rect.top()), rect.max);
            for (place, code) in column.iter().enumerate() {
                let row = egui::Rect::from_min_size(rows.min + egui::vec2(0.0, COLUMN_ROW_HEIGHT * place as f32), egui::vec2(rows.width(), COLUMN_ROW_HEIGHT));
                let mut row_ui = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
                row_ui.push_id(("strat_column_row", place), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let moved = |to: usize| {
                        let mut codes = column.to_vec();
                        codes.swap(place, to);
                        Some(codes)
                    };
                    if ui
                        .add(MenuButton::new(tr!("common-minus-sign")).min_width(24.0))
                        .on_hover_text(tr!("borehole-inspector-remove-from-column"))
                        .clicked()
                    {
                        let mut codes = column.to_vec();
                        codes.remove(place);
                        edited = Some(codes);
                    }
                    if ui.add(MenuButton::new(tr!("common-down")).min_width(0.0).enabled(place + 1 < column.len())).clicked() {
                        edited = moved(place + 1);
                    }
                    if ui.add(MenuButton::new(tr!("common-up")).min_width(0.0).enabled(place > 0)).clicked() {
                        edited = moved(place - 1);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        // The colour the log and the scene give this code.
                        let [red, green, blue] = crate::ui::widgets::borehole_log::strat_run_color(&dataset.color, field, code);
                        let (swatch, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        ui.painter()
                            .rect_filled(swatch, GROUP_CORNER_RADIUS, crate::rendering::color::rgba_to_color32([red, green, blue, 1.0]));
                        let text = format!("{}  {code}", place + 1);
                        if codes.contains(code) {
                            ui.add(egui::Label::new(text).truncate());
                        } else {
                            // Listed by the geologist but held by no interval
                            // of this set: kept, and shown as such.
                            ui.add(egui::Label::new(egui::RichText::new(text).weak()).truncate())
                                .on_hover_text(tr!("borehole-inspector-code-not-in-set"));
                        }
                    });
                });
            }
        });
    ui.add_space(4.0);
    // A repeat (a code with a trailing R beside the code it repeats) and UNK
    // are never offered: the column is the abstract stack.
    let present = |base: &str| codes.iter().chain(column).any(|code| code == base);
    let mut missing = codes
        .iter()
        .filter(|code| !column.contains(code) && code.as_str() != crate::model::drill_hole::UNKNOWN_NAME && !crate::model::strat_order::is_repeat_code(code, present))
        .collect::<Vec<_>>();
    missing.sort_by(|a, b| crate::natural_sort::natural_cmp(a, b));
    CollapsibleSection::new(
        "borehole_inspector_column_missing",
        tr!("borehole-inspector-not-in-column", count = missing.len().to_string()),
    )
    .default_open(true)
    .show(ui, |ui| {
        if missing.is_empty() {
            menu::menu_note(ui, tr!("borehole-inspector-every-code-placed"));
            return;
        }
        menu::menu_note(ui, tr!("borehole-inspector-place-codes-note"));
        // Appended in name order: nothing about their order is guessed here.
        if ui.add(MenuButton::new(tr!("borehole-inspector-add-every-code"))).clicked() {
            let mut codes = column.to_vec();
            codes.extend(missing.iter().map(|code| (*code).clone()));
            edited = Some(codes);
        }
        for code in &missing {
            MenuField::new(code.as_str()).show(ui, |ui, _, _| {
                if ui.add(MenuButton::new(tr!("borehole-inspector-add-to-column"))).clicked() {
                    let mut codes = column.to_vec();
                    codes.push((*code).clone());
                    edited = Some(codes);
                }
            });
        }
    });
    if let Some(codes) = edited {
        commands.push(UiCommand::SetStratColumn {
            id: dataset.id,
            field: field.key.clone(),
            codes,
        });
    }
}

/// The working sections of `field` as blocks beside the column rows they
/// hold, in `rect`; a section whose codes do not sit together in the column
/// is drawn in pieces, so the gap shows. Display only: sections are edited
/// in the colour dialog.
fn draw_section_blocks(ui: &egui::Ui, rect: egui::Rect, dataset: &OpenDrillHoleDataset, field: &str, column: &[String]) {
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let text_color = visuals.strong_text_color();
    let font = egui::TextStyle::Small.resolve(ui.style());
    for span in section_spans(&dataset.color.working_sections, field, column) {
        let section = &dataset.color.working_sections[span.section];
        let colour = dataset
            .color
            .category_color(&section_color_key(&section.name))
            .map_or(visuals.widgets.inactive.bg_fill, |[r, g, b]| {
                egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
            });
        for run in &span.runs {
            let block = egui::Rect::from_min_max(
                egui::pos2(rect.left(), rect.top() + COLUMN_ROW_HEIGHT * run.start as f32 + 1.0),
                egui::pos2(rect.right(), rect.top() + COLUMN_ROW_HEIGHT * run.end as f32 - 1.0),
            );
            painter.rect(
                block,
                GROUP_CORNER_RADIUS,
                colour.gamma_multiply(0.35),
                egui::Stroke::new(1.0, colour),
                egui::StrokeKind::Inside,
            );
            let galley = painter.layout(section.name.clone(), font.clone(), text_color, block.width() - 6.0);
            painter
                .with_clip_rect(block.shrink(1.0))
                .galley(block.left_top() + egui::vec2(3.0, 3.0), galley, text_color);
        }
    }
}

/// One working section's place beside the strat column: the index of the
/// section and each run of consecutive column rows it holds, end exclusive.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SectionSpan {
    section: usize,
    runs: Vec<std::ops::Range<usize>>,
}

/// Where each working section of `field` sits beside `column`: one run for a
/// section whose codes sit together, more than one for a section broken by
/// a code it does not hold. A section holding no listed code has no span,
/// and a row held by no section lies in none.
fn section_spans(sections: &[WorkingSection], field: &str, column: &[String]) -> Vec<SectionSpan> {
    sections
        .iter()
        .enumerate()
        .filter(|(_, section)| section.field == field)
        .filter_map(|(index, section)| {
            let mut runs: Vec<std::ops::Range<usize>> = Vec::new();
            for (row, code) in column.iter().enumerate() {
                if !section.codes.contains(code) {
                    continue;
                }
                match runs.last_mut() {
                    Some(run) if run.end == row => run.end = row + 1,
                    _ => runs.push(row..row + 1),
                }
            }
            (!runs.is_empty()).then_some(SectionSpan { section: index, runs })
        })
        .collect()
}

/// The Data and Log tabs as a pair of held-down buttons, with the lock and
/// close at the far end of the same row.
fn draw_tab_strip(ui: &mut egui::Ui, editor: &mut EditorState) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (tab, label) in [
            (BoreholeInspectorTab::Data, tr!("borehole-inspector-data")),
            (BoreholeInspectorTab::Log, tr!("borehole-inspector-log")),
            (BoreholeInspectorTab::Column, tr!("borehole-inspector-column")),
        ] {
            if ui.add(MenuButton::new(label).selected(editor.borehole_inspector_tab == tab)).clicked() {
                editor.borehole_inspector_tab = tab;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // Rightmost, where a panel's close belongs; the lock sits inboard.
            // The same switch Geology's viewport bar button throws.
            if ui
                .add(egui::Button::image(themed_icon!(ui, "close_project.svg")).frame(false))
                .on_hover_text(tr!("borehole-inspector-close-inspector"))
                .clicked()
            {
                editor.show_borehole_inspector = false;
            }
            let locked = editor.borehole_inspector_locked;
            // unthemed_icon! embeds the image at compile time, so each icon
            // is named per branch rather than passed in as a value.
            let (icon, hint) = if locked {
                (unthemed_icon!("entry_locked.svg"), tr!("borehole-inspector-holding-hole-click-follow-selection"))
            } else {
                (unthemed_icon!("entry_unlocked.svg"), tr!("borehole-inspector-hold-hole-while-you-work"))
            };
            if ui.add(egui::Button::image(icon).frame(locked)).on_hover_text(hint).clicked() {
                editor.borehole_inspector_locked = !locked;
            }
        });
    });
    ui.add_space(4.0);
}

/// The strat field chosen for this dataset: the two conditions the log itself
/// applies, so the picker never names a column the log is not reading.
fn strat_choice_for(editor: &EditorState, dataset: &OpenDrillHoleDataset) -> Option<String> {
    let (id, key) = editor.borehole_log_strat_field.as_ref()?;
    (*id == dataset.id)
        .then(|| dataset.dataset.field(key))
        .flatten()
        .filter(|field| matches!(field.kind, crate::model::drill_hole::DrillFieldKind::Categorical { .. }))
        .map(|field| field.key.clone())
}

/// Two pickers above the log: what the ribbon is coloured by, and what the
/// strat column reads. Colour drives the dataset's own field, the one the
/// scene is drawn by, because a categorical field's colours are held per
/// dataset for that field alone. Strat is the panel's own, since which
/// column names the rock is about this log.
fn draw_log_field_pickers(ui: &mut egui::Ui, editor: &mut EditorState, dataset: &OpenDrillHoleDataset, commands: &mut Vec<UiCommand>) {
    let fields = &dataset.dataset.fields;
    let label_of = |key: Option<&str>, fallback: &str| {
        key.and_then(|key| dataset.dataset.field(key))
            .map_or_else(|| fallback.to_owned(), |field| field.label.clone())
    };

    let uniform = tr!("common-uniform-white");
    let mut chosen = dataset.color.active_field.clone();
    let selected = label_of(chosen.as_deref(), &uniform);
    let options = std::iter::once((None, uniform.into())).chain(fields.iter().map(|field| (Some(field.key.clone()), field.label.clone().into())));
    if MenuFieldCombo::new(("borehole_log_color_field", dataset.id), tr!("common-colour"), &mut chosen, selected, options)
        .show(ui)
        .changed()
    {
        commands.push(UiCommand::SetDrillHoleColorField { id: dataset.id, field: chosen });
    }
    draw_strat_field_picker(ui, editor, dataset);
}

/// What the strat column reads, the panel's own choice: the Log and Column
/// tabs share it, so both read the same field.
fn draw_strat_field_picker(ui: &mut egui::Ui, editor: &mut EditorState, dataset: &OpenDrillHoleDataset) {
    let fields = &dataset.dataset.fields;
    let label_of = |key: Option<&str>, fallback: &str| {
        key.and_then(|key| dataset.dataset.field(key))
            .map_or_else(|| fallback.to_owned(), |field| field.label.clone())
    };
    let guessed = tr!("borehole-inspector-guessed-name");
    let selected = label_of(strat_choice_for(editor, dataset).as_deref(), &guessed);
    let options = std::iter::once((None, guessed.into())).chain(
        fields
            .iter()
            .filter(|field| matches!(field.kind, crate::model::drill_hole::DrillFieldKind::Categorical { .. }))
            .map(|field| (Some((dataset.id, field.key.clone())), field.label.clone().into())),
    );
    MenuFieldCombo::new(
        ("borehole_log_strat_field", dataset.id),
        tr!("borehole-inspector-strat"),
        &mut editor.borehole_log_strat_field,
        selected,
        options,
    )
    .show(ui);
}

/// A short note above the log for the states between a link existing and its
/// logs being on screen; nothing is drawn once the logs are shown or being
/// read, or when there was never a link, since the log covers those itself.
fn draw_geophysics_note(ui: &mut egui::Ui, view: &HoleView, dataset_id: DrillHoleId, commands: &mut Vec<UiCommand>) {
    let (weak, message, button) = match view {
        HoleView::Link(LinkState::Checking) => (true, tr!("borehole-inspector-checking-linked-geophysics-file"), None),
        HoleView::Link(LinkState::Indexing) => (true, tr!("borehole-inspector-reading-geophysics-file-its-index"), None),
        HoleView::Link(LinkState::Missing { file }) => (
            false,
            tr!("borehole-inspector-file-not-where-was-linked", file = file.to_string()),
            Some(tr!("common-link-geophysics")),
        ),
        HoleView::Link(LinkState::NeedsPick { file }) => (
            false,
            tr!("borehole-inspector-pick-file-again-show-its", file = file.to_string()),
            Some(tr!("borehole-inspector-pick-file", file = file.to_string())),
        ),
        HoleView::Link(LinkState::Failed(error)) => (false, error.clone(), Some(tr!("common-link-geophysics"))),
        HoleView::Failed(error) => (false, (*error).to_owned(), None),
        HoleView::Link(LinkState::Ready) | HoleView::Reading | HoleView::Wanted | HoleView::Shown(_) | HoleView::NotInFiles | HoleView::Unlinked => return,
    };

    let mut text = egui::RichText::new(message);
    if weak {
        text = text.weak();
    }
    ui.add(egui::Label::new(text).wrap());
    if let Some(label) = button
        && ui.add(MenuButton::new(label)).clicked()
    {
        commands.push(UiCommand::LinkGeophysics(dataset_id));
    }
    ui.add_space(4.0);
}

/// The shift dialog's opening state for the hole's names: what the shift
/// would do, counted once, here, against the column as it stands.
fn name_shift_draft(dataset: &OpenDrillHoleDataset, hole_index: usize, shift: NameShift) -> NameShiftDraft {
    let column = dataset.color.strat_column(&shift.field);
    let worked = dataset
        .dataset
        .column_shift(hole_index, &shift.field, column, shift.direction, shift.from, &CorrectionNote::default());
    NameShiftDraft {
        dataset: dataset.id,
        hole: hole_index,
        dhid: dataset.dataset.holes.get(hole_index).map(|hole| hole.dhid.clone()).unwrap_or_default(),
        field: shift.field,
        direction: shift.direction,
        from: shift.from,
        moved: worked.corrections.len() - worked.unknown,
        unknown: worked.unknown,
        untouched: worked.untouched,
        reason: String::new(),
    }
}

/// The rename dialog's opening state for a seam picked in the log: its
/// reach counted once, here, rather than every frame the dialog is up.
fn seam_rename_draft(dataset: &OpenDrillHoleDataset, hole_index: usize, rename: SeamRename) -> SeamRenameDraft {
    let scope = if rename.every_hole {
        RenameScope::Set
    } else {
        RenameScope::Horizon {
            hole: hole_index,
            interval: rename.interval,
        }
    };
    let reached = dataset
        .dataset
        .intervals_named(&rename.field, &rename.name, scope)
        .map(|((hole, _), _, _)| hole)
        .collect::<Vec<_>>();
    let mut holes = reached.clone();
    holes.dedup();
    let place = match scope {
        RenameScope::Set => dataset.name.clone(),
        RenameScope::Horizon { hole, .. } => dataset.dataset.holes.get(hole).map(|hole| hole.dhid.clone()).unwrap_or_default(),
    };
    SeamRenameDraft {
        dataset: dataset.id,
        scope,
        field: rename.field,
        to: rename.name.clone(),
        from: rename.name,
        place,
        intervals: reached.len(),
        holes: holes.len(),
        reason: String::new(),
        out_of_sequence: None,
    }
}

/// Resolve the currently inspected hole to its dataset, the hole itself, and
/// the hole's index (tab widgets key on the index, so callers do not need to
/// re-derive it).
fn inspected_hole<'a>(editor: &EditorState, datasets: &'a [OpenDrillHoleDataset]) -> Option<(&'a OpenDrillHoleDataset, &'a crate::model::drill_hole::DrillHole, usize)> {
    let inspected = editor.inspected_hole?;
    let dataset = datasets.iter().find(|dataset| dataset.id == inspected.dataset && dataset.state.loaded)?;
    // A hidden dataset hides its inspected hole too.
    if editor.hidden_handles.contains(&SceneEntityId::DrillHole(dataset.id)) {
        return None;
    }
    let hole = dataset.dataset.holes.get(inspected.hole)?;
    Some((dataset, hole, inspected.hole))
}

/// Read-only summary of one drill hole: collar, trace extent, orientation,
/// provenance, and the interval table as a grid. The two halves draw
/// separately, so the caller can fold each into a section of its own.
pub(crate) struct DrillHoleProperties<'a> {
    id: egui::Id,
    hole: &'a crate::model::drill_hole::DrillHole,
    /// The dataset's fields, in order, become the columns after From and
    /// To, so every hole gets the same columns even if it never recorded one.
    fields: &'a [crate::model::drill_hole::DrillField],
}

impl<'a> DrillHoleProperties<'a> {
    pub(crate) fn new(id_source: impl Hash + Debug, hole: &'a crate::model::drill_hole::DrillHole, fields: &'a [crate::model::drill_hole::DrillField]) -> Self {
        Self {
            id: egui::Id::new(id_source),
            hole,
            fields,
        }
    }

    /// The hole's collar, trace, orientation and interval count, as rows
    /// lined up with the fields of the panel around them.
    pub(crate) fn show_summary(&self, ui: &mut egui::Ui) {
        let hole = self.hole;
        let collar = hole.collar_position();
        read_only_row(ui, &tr!("common-easting"), &format!("{:.2}", collar.x));
        read_only_row(ui, &tr!("common-northing"), &format!("{:.2}", collar.y));
        read_only_row(ui, &tr!("common-elevation"), &format!("{:.2}", collar.z));
        let extent = match (hole.trace.first(), hole.trace.last()) {
            (Some(first), Some(last)) => tr!("viewport-from", from = format!("{:.2}", first.depth), to = format!("{:.2}", last.depth)),
            _ => tr!("viewport-no-trace"),
        };
        read_only_row(ui, &tr!("viewport-trace-extent"), &extent);
        let orientation = match hole.orientation() {
            Some(orientation) => tr!(
                "viewport-azimuth-dip",
                azimuth = format!("{:.1}", orientation.azimuth),
                dip = format!("{:.1}", orientation.dip)
            ),
            // Not "none": nobody recorded one, the same as its source.
            None => tr!("common-unknown"),
        };
        read_only_row(ui, &tr!("common-orientation"), &orientation);
        read_only_row(ui, &tr!("viewport-orientation-source"), &hole.orientation_source.label());
        read_only_row(ui, &tr!("viewport-intervals"), &hole.intervals.len().to_string());
    }

    /// The interval spreadsheet, scrolling within `max_height`.
    pub(crate) fn show_intervals(&self, ui: &mut egui::Ui, max_height: f32) {
        let hole = self.hole;
        let header = drill_table_header(self.fields);
        let aligns = drill_table_aligns(self.fields);
        let shown = |index: usize| hole.intervals.get(index).map(|interval| drill_table_row(interval, self.fields, false)).unwrap_or_default();
        let copied = |index: usize| hole.intervals.get(index).map(|interval| drill_table_row(interval, self.fields, true)).unwrap_or_default();
        DataTable::new(self.id.with("intervals"), &header, hole.intervals.len(), &shown)
            .aligns(&aligns)
            .copy_cells(&copied)
            .fingerprint(self.fingerprint())
            .max_height(max_height)
            .show(ui);
    }

    /// What the table's columns were measured from; a change re-measures.
    fn fingerprint(&self) -> u64 {
        use std::hash::Hasher;

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hole
            .intervals
            .last()
            .map(|interval| (interval.from.to_bits(), interval.to.to_bits()))
            .hash(&mut hasher);
        hasher.finish()
    }
}

/// The table's header row: From, To, then one column per dataset field.
fn drill_table_header(fields: &[crate::model::drill_hole::DrillField]) -> Vec<String> {
    let mut header = Vec::with_capacity(fields.len() + 2);
    header.push(tr!("survey-from"));
    header.push(tr!("survey-to"));
    header.extend(fields.iter().map(|field| field.label.clone()));
    header
}

/// One interval as text, one cell per header column, `raw` for full
/// precision instead of display rounding. An unrecorded field is an empty cell.
///
/// A corrected cell shows the interpreted value with the logged one in
/// brackets; raw text, which a copy carries, is the interpreted value alone.
fn drill_table_row(interval: &crate::model::drill_hole::DrillInterval, fields: &[crate::model::drill_hole::DrillField], raw: bool) -> Vec<String> {
    let (logged_from, logged_to, logged_values) = interval.logged();
    let corrected = !raw && interval.is_corrected();
    let beside = |shown: String, logged: String| if corrected && shown != logged { format!("{shown} ({logged})") } else { shown };
    let depth = |depth: f64| if raw { depth.to_string() } else { format!("{depth:.2}") };
    let value = |value: Option<&crate::model::drill_hole::DrillValue>| match value {
        Some(crate::model::drill_hole::DrillValue::Numeric(number)) => {
            if raw {
                number.to_string()
            } else {
                format_grade(*number)
            }
        }
        Some(crate::model::drill_hole::DrillValue::Category(category)) => category.clone(),
        None => String::new(),
    };
    let mut row = Vec::with_capacity(fields.len() + 2);
    row.push(beside(depth(interval.from), depth(logged_from)));
    row.push(beside(depth(interval.to), depth(logged_to)));
    row.extend(
        fields
            .iter()
            .map(|field| beside(value(interval.values.get(&field.key)), value(logged_values.get(&field.key)))),
    );
    row
}

/// Which edge a column's cells sit against: numbers right, categories left.
fn drill_table_aligns(fields: &[crate::model::drill_hole::DrillField]) -> Vec<egui::Align> {
    let mut aligns = vec![egui::Align::Max, egui::Align::Max];
    aligns.extend(fields.iter().map(|field| match field.kind {
        crate::model::drill_hole::DrillFieldKind::Numeric { .. } => egui::Align::Max,
        crate::model::drill_hole::DrillFieldKind::Categorical { .. } => egui::Align::Min,
    }));
    aligns
}
