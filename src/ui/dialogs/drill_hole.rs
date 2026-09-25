use crate::{
    i18n::{tr, tr_format},
    model::drill_hole::{
        ColorRow, DrillCategoryColor, DrillColorPreset, DrillFieldKind, DrillHoleId, DrillHoleStyle, MAX_DRILL_COLOR_STOPS, OpenDrillHoleDataset, SectionProblem, WorkingSection,
        default_category_colors, suggested_working_sections, working_section_name_problem,
    },
    ui::{
        state::{EditorState, UiCommand},
        widgets::menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldBool, MenuFieldCombo, MenuFieldText},
    },
};

/// Which surface is hosting [`draw_drill_hole_color_editor`]. The modal
/// colour dialog scrolls its long lists, and itself on a short window; the
/// Preferences "Drillholes" page already scrolls as a whole
/// (`ScrollArea::both` in `ui/elements/properties.rs`), so nesting another
/// scroll area inside it fights the outer one under the mouse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColorEditorHost {
    Dialog,
    Page,
}

pub(crate) fn draw_drill_hole_color_dialog(ui: &mut egui::Ui, editor: &mut EditorState, datasets: &[OpenDrillHoleDataset], commands: &mut Vec<UiCommand>) {
    let Some(id) = editor.drill_hole_color_dialog else {
        return;
    };
    let Some(dataset) = datasets.iter().find(|dataset| dataset.id == id) else {
        editor.drill_hole_color_dialog = None;
        return;
    };
    // Captured before the window body draws: a single-line TextEdit gives up
    // focus the same frame it consumes Enter, so checking focus after the
    // body would miss the very field the user just pressed Enter in.
    let text_field_focused = ui
        .ctx()
        .memory(|memory| memory.focused())
        .is_some_and(|focused_id| egui::TextEdit::load_state(ui.ctx(), focused_id).is_some());
    let mut open = true;
    let title = tr_format!(literal = "Drill Hole Appearance: %name%", name = dataset.name.clone());
    DragableMenu::new("drill_hole_colour_dialog", title)
        .open(&mut open)
        .min_width(400.0)
        .max_width(480.0)
        .show(ui.ctx(), |ui| {
            // Room for the title bar and a gap, so a short window scrolls.
            let max_height = ui.ctx().content_rect().height() - menu::TITLE_BAR_HEIGHT - 48.0;
            egui::ScrollArea::vertical()
                .max_height(max_height.max(120.0))
                .id_salt(("drill_hole_colour_dialog_scroll", id))
                .show(ui, |ui| {
                    draw_drill_hole_color_editor(ui, dataset, commands, ColorEditorHost::Dialog);
                });
        });
    // Colour edits apply live, so there is no confirm step: both keys dismiss.
    // Enter is left alone while a text field - or a slider being typed into,
    // which is a text field under the hood - has focus, so the field gets to
    // finish its own edit instead of the dialog closing under it.
    let dismissed = {
        let confirm = !text_field_focused && menu::dialog_confirm_pressed(ui.ctx());
        let cancel = menu::dialog_cancel_pressed(ui.ctx());
        confirm || cancel
    };
    if !open || dismissed {
        editor.drill_hole_color_dialog = None;
    }
}

/// The body shared by the colour dialog and the Drillholes preferences page:
/// field, width, colour scale, category colours, and working sections for one
/// dataset.
pub(crate) fn draw_drill_hole_color_editor(ui: &mut egui::Ui, dataset: &OpenDrillHoleDataset, commands: &mut Vec<UiCommand>, host: ColorEditorHost) {
    let id = dataset.id;
    let active_label = match (dataset.color.active_field.as_deref(), dataset.color.by_working_section) {
        (None, _) => tr!(literal = "Uniform white"),
        (Some(key), by_section) => dataset.dataset.field(key).map_or_else(
            || tr!(literal = "Uniform white"),
            |field| {
                if by_section {
                    tr_format!(literal = "%field% by working section", field = field.label.clone())
                } else {
                    field.label.clone()
                }
            },
        ),
    };
    let mut active = (dataset.color.active_field.clone(), dataset.color.by_working_section);
    let mut field_options: Vec<((Option<String>, bool), egui::WidgetText)> = vec![((None, false), tr!(literal = "Uniform white").into())];
    for field in &dataset.dataset.fields {
        field_options.push(((Some(field.key.clone()), false), field.label.clone().into()));
        if matches!(field.kind, DrillFieldKind::Categorical { .. }) && dataset.color.working_sections.iter().any(|section| section.field == field.key) {
            field_options.push((
                (Some(field.key.clone()), true),
                tr_format!(literal = "%field% by working section", field = field.label.clone()).into(),
            ));
        }
    }
    if MenuFieldCombo::new(("drill_hole_field", id), tr!(literal = "Field"), &mut active, active_label, field_options)
        .show(ui)
        .changed()
    {
        let (field, by_section) = active;
        if by_section {
            if let Some(field) = field {
                commands.push(UiCommand::SetDrillHoleColorByWorkingSection { id, field });
            }
        } else {
            commands.push(UiCommand::SetDrillHoleColorField { id, field });
        }
    }

    menu::menu_section(ui, tr!(literal = "Width"));
    let mut style = dataset.color.hole_style;
    if MenuFieldCombo::new(
        ("drill_hole_style", id),
        tr!(literal = "Style"),
        &mut style,
        dataset.color.hole_style.label(),
        DrillHoleStyle::ALL.map(|style| (style, style.label().into())),
    )
    .help_text(tr!(literal = "As string and discs, where intervals overlap the shortest one is drawn as the disc."))
    .show(ui)
    .changed()
    {
        commands.push(UiCommand::SetDrillHoleStyle { id, style });
    }
    match style {
        DrillHoleStyle::TrueDiameter => {
            let mut scale = dataset.color.radius_scale;
            let mut floor = dataset.color.min_pixel_diameter;
            let mut width_changed = false;
            MenuField::new(tr!(literal = "Of drilled diameter")).show(ui, |ui, _, _| {
                width_changed |= ui
                    .add(
                        egui::Slider::new(&mut scale, crate::model::drill_hole::RADIUS_SCALE_RANGE)
                            .fixed_decimals(2)
                            .suffix(tr!(literal = "x")),
                    )
                    .on_hover_text(tr!(
                        literal = "A hole at its drilled width reads as a pipe beside the geology; a set of thousands reads as a mat."
                    ))
                    .changed();
            });
            MenuField::new(tr!(literal = "Never thinner than")).show(ui, |ui, _, _| {
                width_changed |= ui
                    .add(
                        egui::Slider::new(&mut floor, crate::model::drill_hole::MIN_PIXEL_DIAMETER_RANGE)
                            .fixed_decimals(1)
                            .suffix(tr!(literal = " px")),
                    )
                    .on_hover_text(tr!(literal = "However far the eye is, a hole is drawn at least this wide."))
                    .changed();
            });
            if width_changed {
                commands.push(UiCommand::SetDrillHoleWidth {
                    id,
                    radius_scale: scale,
                    min_pixel_diameter: floor,
                });
            }
        }
        DrillHoleStyle::StringAndDiscs => {
            let mut disc_diameter = dataset.color.disc_diameter;
            let mut string_pixel_width = dataset.color.string_pixel_width;
            let mut discs_changed = false;
            MenuField::new(tr!(literal = "Disc diameter")).show(ui, |ui, _, _| {
                discs_changed |= ui
                    .add(
                        egui::Slider::new(&mut disc_diameter, crate::model::drill_hole::DISC_DIAMETER_RANGE)
                            .logarithmic(true)
                            .fixed_decimals(2)
                            .suffix(tr!(literal = " m")),
                    )
                    .on_hover_text(tr!(
                        literal = "Every interval with a value in the colour field is drawn as a disc this wide on the string. Far away it is never narrower than a few pixels."
                    ))
                    .changed();
            });
            MenuField::new(tr!(literal = "String width")).show(ui, |ui, _, _| {
                discs_changed |= ui
                    .add(
                        egui::Slider::new(&mut string_pixel_width, crate::model::drill_hole::STRING_PIXEL_WIDTH_RANGE)
                            .fixed_decimals(1)
                            .suffix(tr!(literal = " px")),
                    )
                    .on_hover_text(tr!(literal = "The hole itself is drawn as a line this wide at every zoom."))
                    .changed();
            });
            if discs_changed {
                commands.push(UiCommand::SetDrillHoleDiscs {
                    id,
                    disc_diameter,
                    string_pixel_width,
                });
            }
        }
    }

    let Some(field) = dataset.color.active_field.as_deref().and_then(|key| dataset.dataset.field(key)) else {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(tr!(literal = "All rendered intervals are opaque white.")).weak());
        return;
    };

    menu::menu_section(ui, tr!(literal = "Colour scale"));
    match &field.kind {
        DrillFieldKind::Numeric { min, max } => {
            let mut preset = dataset.color.preset;
            if MenuFieldCombo::new(
                ("drill_hole_preset", id),
                tr!(literal = "Preset"),
                &mut preset,
                dataset.color.preset.label(),
                DrillColorPreset::ALL.map(|preset| (preset, preset.label().into())),
            )
            .show(ui)
            .changed()
            {
                commands.push(UiCommand::SetDrillHoleColorPreset { id, preset });
            }
            ui.label(
                egui::RichText::new(if dataset.color.smooth {
                    tr!(literal = "Smooth interpolation")
                } else {
                    tr!(literal = "Stepped bands")
                })
                .weak(),
            );
            ui.add_space(2.0);
            let mut stops = dataset.color.stops.clone();
            let mut changed = false;
            let mut remove = None;
            let can_remove = stops.len() > 2;
            for (index, stop) in stops.iter_mut().enumerate() {
                MenuField::new(tr!("drill-hole-colour-stop", index = ((index + 1) as u32))).show(ui, |ui, _, _| {
                    changed |= crate::ui::widgets::color::edit_rgb(ui, &mut stop.color).changed();
                    let actual = if (*max - *min).abs() <= f64::EPSILON {
                        *min
                    } else {
                        *min + f64::from(stop.t) * (*max - *min)
                    };
                    ui.monospace(format!("{actual:.6}"));
                    changed |= ui
                        .add_sized([145.0, ui.spacing().interact_size.y], egui::Slider::new(&mut stop.t, 0.0..=1.0).show_value(false))
                        .changed();
                    if can_remove && ui.small_button(tr!(literal = "−")).clicked() {
                        remove = Some(index);
                    }
                });
            }
            if let Some(index) = remove {
                stops.remove(index);
                changed = true;
            }
            ui.horizontal(|ui| {
                if stops.len() < MAX_DRILL_COLOR_STOPS && ui.add(MenuButton::new(tr!(literal = "Add stop"))).clicked() {
                    let index = stops.len() / 2;
                    let left = stops[index.saturating_sub(1)];
                    let right = stops[index.min(stops.len() - 1)];
                    stops.push(crate::model::drill_hole::DrillColorStop {
                        t: (left.t + right.t) * 0.5,
                        color: [
                            (left.color[0] + right.color[0]) * 0.5,
                            (left.color[1] + right.color[1]) * 0.5,
                            (left.color[2] + right.color[2]) * 0.5,
                        ],
                    });
                    changed = true;
                }
                if ui.add(MenuButton::new(tr!(literal = "Reset preset"))).clicked() {
                    commands.push(UiCommand::SetDrillHoleColorPreset { id, preset: dataset.color.preset });
                }
            });
            if changed {
                commands.push(UiCommand::SetDrillHoleColorStops { id, stops });
            }
        }
        DrillFieldKind::Categorical { categories } => {
            ui.label(
                egui::RichText::new(tr_format!(
                    literal = "%count% codes. An interval with no logged value stays white.",
                    count = categories.len()
                ))
                .weak(),
            );
            ui.add_space(2.0);
            let rows: Vec<ColorRow<'_>> = if dataset.color.by_working_section {
                dataset.color.working_section_view(&field.key, categories)
            } else {
                categories.iter().map(|code| ColorRow::code(code)).collect()
            };
            let needle = draw_code_filter(ui, code_filter_id("colours", id, &field.key));
            // A section's row also shows for a code it holds, so typing a ply
            // finds the section it went into.
            let shown_rows: Vec<&ColorRow<'_>> = rows
                .iter()
                .filter(|row| filter_matches(row.label, &needle) || row.section.is_some_and(|section| section.codes.iter().any(|code| filter_matches(code, &needle))))
                .collect();
            if !needle.is_empty() {
                let note = if dataset.color.by_working_section {
                    tr_format!(literal = "%shown% of %total% rows shown", shown = shown_rows.len(), total = rows.len())
                } else {
                    tr_format!(literal = "%shown% of %total% codes shown", shown = shown_rows.len(), total = rows.len())
                };
                ui.label(egui::RichText::new(note).weak());
            }
            // Rows follow the field's order, not the table's. A section's row
            // sets the section's own entry, never the code of its name.
            let mut table: Option<Vec<DrillCategoryColor>> = None;
            let row_height = ui.spacing().interact_size.y;
            let mut draw_row = |ui: &mut egui::Ui, row: &ColorRow<'_>| {
                let key: &str = &row.key;
                let mut color = dataset.color.category_color(key).unwrap_or([1.0; 3]);
                MenuField::new(row.label).show(ui, |ui, _, _| {
                    if crate::ui::widgets::color::edit_rgb(ui, &mut color).changed() {
                        let table = table.get_or_insert_with(|| dataset.color.categories.to_vec());
                        match table.iter_mut().find(|entry| entry.value == key) {
                            Some(entry) => entry.color = color,
                            None => table.push(DrillCategoryColor { value: key.to_owned(), color }),
                        }
                    }
                });
            };
            match host {
                ColorEditorHost::Dialog => {
                    egui::ScrollArea::vertical()
                        // About nine rows without leaving the screen.
                        .max_height(280.0)
                        .auto_shrink([false, true])
                        .id_salt(("drill_hole_categories_scroll", id))
                        .show_rows(ui, row_height, shown_rows.len(), |ui, range| {
                            for row in &shown_rows[range] {
                                draw_row(ui, row);
                            }
                        });
                }
                ColorEditorHost::Page => {
                    // The preferences page already scrolls as a whole; lay
                    // the rows out inline instead of nesting a scroll
                    // area that would fight the page's own under the mouse.
                    for row in &shown_rows {
                        draw_row(ui, row);
                    }
                }
            }
            if let Some(table) = table {
                commands.push(UiCommand::SetDrillHoleCategoryColors { id, categories: table });
            }
            ui.add_space(2.0);
            let reset_label = if needle.is_empty() {
                tr!(literal = "Reset colours")
            } else {
                tr!(literal = "Reset shown colours")
            };
            if ui.add(MenuButton::new(reset_label)).clicked() {
                let all: Vec<String> = rows.iter().map(|row| row.key.to_string()).collect();
                let shown: Vec<&str> = shown_rows.iter().map(|row| &*row.key).collect();
                commands.push(UiCommand::SetDrillHoleCategoryColors {
                    id,
                    categories: reset_row_colors(&dataset.color.categories, &all, &shown),
                });
            }

            draw_working_sections(ui, dataset, field, categories, commands, host);
        }
    }
}

/// Draft state for the "add a working section" builder: kept in egui memory,
/// salted only by dataset, because it is unsent UI text rather than
/// application state, and the dialog and the Preferences page must show the
/// same draft when both are open on the same dataset. `field` records which
/// field the ticked codes belong to, so switching fields can tell a stale
/// tick set from the name the user is still typing.
#[derive(Clone, Default)]
struct WorkingSectionDraft {
    field: String,
    name: String,
    codes: std::collections::BTreeSet<String>,
}

fn draw_working_sections(
    ui: &mut egui::Ui,
    dataset: &OpenDrillHoleDataset,
    field: &crate::model::drill_hole::DrillField,
    categories: &[String],
    commands: &mut Vec<UiCommand>,
    host: ColorEditorHost,
) {
    menu::menu_section(ui, tr!(literal = "Working sections"));
    ui.label(
        egui::RichText::new(tr!(
            literal = "A working section is a set of seams or plies mined as one unit. Colouring by it gives the whole set one colour."
        ))
        .weak(),
    );
    ui.add_space(2.0);

    // Every change asked for this frame goes out as one command at the end,
    // so two in one frame neither overwrite each other nor split the undo.
    let mut edits: Vec<SectionEdit> = Vec::new();
    for section in dataset.color.working_sections.iter().filter(|section| section.field == field.key) {
        MenuField::new(section.name.as_str()).show(ui, |ui, _, _| {
            ui.label(egui::RichText::new(section.codes.join(", ")).weak());
            if ui.small_button(tr!(literal = "−")).clicked() {
                edits.push(SectionEdit::Remove(section.name.clone()));
            }
        });
    }
    if let Some(added) = draw_suggested_sections(ui, dataset, field, categories) {
        edits.push(SectionEdit::Add(added));
    }

    ui.add_space(4.0);
    // Not `ui.id()`: the dialog and the Preferences page draw from different
    // widget trees, and the draft must be the same egui memory slot either
    // way, so both windows show the one section being built.
    let draft_id = egui::Id::new(("drill_hole_working_section_draft", dataset.id));
    let mut draft = ui.data_mut(|data| data.get_temp::<WorkingSectionDraft>(draft_id)).unwrap_or_default();
    if draft.field != field.key {
        // A different field's ticks mean nothing here; the typed name might
        // still be what the user wants for this field too.
        draft.field = field.key.clone();
        draft.codes.clear();
    }

    // Re-checked every frame: a code leaves this list the moment some other
    // section (added, or edited elsewhere) claims it.
    let available: Vec<&String> = categories.iter().filter(|code| dataset.color.working_section_of(&field.key, code).is_none()).collect();
    let needle = draw_code_filter(ui, code_filter_id("section codes", dataset.id, &field.key));
    let shown: Vec<&String> = available.iter().copied().filter(|code| filter_matches(code, &needle)).collect();
    if !needle.is_empty() {
        ui.label(egui::RichText::new(tr_format!(literal = "%shown% of %total% codes shown", shown = shown.len(), total = available.len())).weak());
    }
    // A tick the filter hides still counts; say so rather than add codes
    // the user cannot see.
    let hidden_ticked = available
        .iter()
        .filter(|code| draft.codes.contains(code.as_str()) && !filter_matches(code, &needle))
        .count();
    if hidden_ticked > 0 {
        ui.label(egui::RichText::new(tr_format!(literal = "Ticked but hidden by the filter: %count%", count = hidden_ticked)).weak());
    }

    let row_height = ui.spacing().interact_size.y;
    let mut draw_code_row = |ui: &mut egui::Ui, code: &String| {
        let mut ticked = draft.codes.contains(code.as_str());
        if MenuFieldBool::new(code.as_str(), &mut ticked).show(ui).changed() {
            if ticked {
                draft.codes.insert(code.clone());
            } else {
                draft.codes.remove(code.as_str());
            }
        }
    };
    match host {
        ColorEditorHost::Dialog => {
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .auto_shrink([false, true])
                .id_salt(("drill_hole_working_section_codes_scroll", dataset.id, field.key.as_str()))
                .show_rows(ui, row_height, shown.len(), |ui, range| {
                    for code in &shown[range] {
                        draw_code_row(ui, code);
                    }
                });
        }
        ColorEditorHost::Page => {
            for code in &shown {
                draw_code_row(ui, code);
            }
        }
    }

    // The name comes after the ticks, so it is checked once, against the
    // codes this frame ended with: it may be one of them. Only a ticked code
    // still available goes into the new section; a stale tick left over from
    // before another section claimed it does not.
    let name_response = MenuFieldText::new(tr!(literal = "New section name"), &mut draft.name).show(ui);
    let ticked_available: Vec<String> = available.iter().copied().filter(|code| draft.codes.contains(code.as_str())).cloned().collect();
    let name_problem = working_section_name_problem(&draft.name, &field.key, categories, &ticked_available, &dataset.color.working_sections);
    if let Some(problem) = &name_problem
        && matches!(problem, SectionProblem::NameIsCode | SectionProblem::CodeClaimed { .. } | SectionProblem::DuplicateName)
    {
        ui.label(egui::RichText::new(problem.message()).weak());
    }
    let can_add = name_problem.is_none() && !ticked_available.is_empty();
    let add_clicked = ui.add_enabled(can_add, MenuButton::new(tr!(literal = "Add working section"))).clicked();
    // A single-line TextEdit surrenders focus the same frame it sees Enter,
    // without consuming the key - do that here so Enter both adds (when
    // valid) and never reaches the dialog's own confirm-and-close check.
    let enter_pressed = name_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if enter_pressed {
        ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    }
    if add_clicked || (enter_pressed && can_add) {
        edits.push(SectionEdit::Add(vec![WorkingSection {
            name: draft.name.clone(),
            field: field.key.clone(),
            codes: ticked_available,
        }]));
        draft = WorkingSectionDraft {
            field: field.key.clone(),
            ..WorkingSectionDraft::default()
        };
    }
    ui.data_mut(|data| data.insert_temp(draft_id, draft));
    if let Some(sections) = apply_section_edits(&dataset.color.working_sections, &field.key, edits) {
        commands.push(UiCommand::SetDrillHoleWorkingSections { id: dataset.id, sections });
    }
}

/// One change to the working sections asked for in a frame.
#[derive(Debug)]
enum SectionEdit {
    /// Take out the section of this name.
    Remove(String),
    /// Add these, after those already there.
    Add(Vec<WorkingSection>),
}

/// `current` with every edit of one frame applied in the order asked, for
/// one command; `None` when none was asked. Removals are of `field`'s
/// sections only.
fn apply_section_edits(current: &[WorkingSection], field: &str, edits: Vec<SectionEdit>) -> Option<Vec<WorkingSection>> {
    if edits.is_empty() {
        return None;
    }
    let mut sections = current.to_vec();
    for edit in edits {
        match edit {
            SectionEdit::Remove(name) => sections.retain(|section| !(section.field == field && section.name == name)),
            SectionEdit::Add(added) => sections.extend(added),
        }
    }
    Some(sections)
}

/// The colour table after resetting the rows keyed `shown`: each takes what
/// a reset of every row keyed `all` would give it, so a filtered reset
/// matches the full one row for row, and every other entry stays.
fn reset_row_colors(table: &[DrillCategoryColor], all: &[String], shown: &[&str]) -> Vec<DrillCategoryColor> {
    let mut reset: Vec<DrillCategoryColor> = table.iter().filter(|entry| !shown.contains(&entry.value.as_str())).cloned().collect();
    reset.extend(default_category_colors(all).into_iter().filter(|entry| shown.contains(&entry.value.as_str())));
    reset
}

/// Where one list's filter text is kept: per dataset and field, not per
/// window, so the dialog and the Preferences page share it, and a field's
/// filter does not narrow another's codes.
fn code_filter_id(list: &str, dataset: DrillHoleId, field: &str) -> egui::Id {
    egui::Id::new(("drill_hole_code_filter", list, dataset, field))
}

/// Whether `code` shows under a filter already trimmed and lowercased by
/// [`draw_code_filter`]: a case-blind match anywhere in it. No filter shows
/// every code.
fn filter_matches(code: &str, needle: &str) -> bool {
    needle.is_empty() || code.to_lowercase().contains(needle)
}

/// A filter row above a long list of codes, with a button to clear it. The
/// text is unsent UI state kept in egui memory under `id`, salted by dataset
/// like the section draft, so the dialog and the Preferences page narrow the
/// same list the same way. Returns the text to match, trimmed and
/// lowercased.
fn draw_code_filter(ui: &mut egui::Ui, id: egui::Id) -> String {
    let mut text = ui.data_mut(|data| data.get_temp::<String>(id)).unwrap_or_default();
    let changed = MenuField::new(tr!(literal = "Filter")).show(ui, |ui, row_height, column_width| {
        let before = ui.available_width();
        let mut changed = false;
        if ui
            .add_enabled(!text.is_empty(), egui::Button::new(tr!(literal = "×")).small())
            .on_hover_text(tr!(literal = "Clear the filter"))
            .clicked()
        {
            text.clear();
            changed = true;
        }
        let clear_width = before - ui.available_width();
        changed |= ui
            .add_sized(
                [(column_width - clear_width).max(40.0), row_height],
                egui::TextEdit::singleline(&mut text).hint_text(tr!(literal = "Part of a code")),
            )
            .changed();
        changed
    });
    if changed {
        ui.data_mut(|data| data.insert_temp(id, text.clone()));
    }
    text.trim().to_lowercase()
}

/// Working sections the code names suggest, offered in a collapsed list and
/// only ever added when asked; see [`suggested_working_sections`] for the
/// rule. Returns what was asked for, for the caller's one command.
fn draw_suggested_sections(ui: &mut egui::Ui, dataset: &OpenDrillHoleDataset, field: &crate::model::drill_hole::DrillField, categories: &[String]) -> Option<Vec<WorkingSection>> {
    // Worked out again only when the codes or the sections change, not on
    // every frame the editor is open.
    let fingerprint = egui::Id::new((field.key.as_str(), categories, &dataset.color.working_sections));
    let cache_id = egui::Id::new(("drill_hole_suggested_sections", dataset.id, field.key.as_str()));
    let cached = ui.data_mut(|data| data.get_temp::<(egui::Id, std::sync::Arc<Vec<WorkingSection>>)>(cache_id));
    let suggestions = match cached {
        Some((key, suggestions)) if key == fingerprint => suggestions,
        _ => {
            let suggestions = std::sync::Arc::new(suggested_working_sections(&field.key, categories, &dataset.color.working_sections));
            ui.data_mut(|data| data.insert_temp(cache_id, (fingerprint, std::sync::Arc::clone(&suggestions))));
            suggestions
        }
    };
    if suggestions.is_empty() {
        return None;
    }
    let mut add: Option<Vec<WorkingSection>> = None;
    egui::CollapsingHeader::new(tr_format!(literal = "Suggested from code names (%count%)", count = suggestions.len()))
        .id_salt(("drill_hole_suggested_sections", dataset.id, field.key.as_str()))
        .default_open(false)
        .show(ui, |ui| {
            for suggestion in suggestions.iter() {
                MenuField::new(suggestion.name.as_str()).show(ui, |ui, _, _| {
                    if ui.small_button(tr!(literal = "Add")).clicked() {
                        add = Some(vec![suggestion.clone()]);
                    }
                    ui.label(egui::RichText::new(tr_format!(literal = "%count% codes", count = suggestion.codes.len())).weak())
                        .on_hover_text(suggestion.codes.join(", "));
                });
            }
            if ui.add(MenuButton::new(tr!(literal = "Add all"))).clicked() {
                add = Some(suggestions.to_vec());
            }
        });
    add
}
