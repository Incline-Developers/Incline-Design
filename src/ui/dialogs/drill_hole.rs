use crate::{
    i18n::{tr, tr_format},
    model::drill_hole::{
        DrillColorPreset, DrillFieldKind, DrillHoleStyle, MAX_DRILL_COLOR_STOPS, OpenDrillHoleDataset, SectionProblem, WorkingSection, default_category_colors,
        working_section_name_problem,
    },
    ui::{
        state::{EditorState, UiCommand},
        widgets::menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldBool, MenuFieldCombo, MenuFieldText},
    },
};

/// Which surface is hosting [`draw_drill_hole_color_editor`]. The modal
/// colour dialog is a fixed height and scrolls its own long lists; the
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
    DragableMenu::new("drill_hole_colour_dialog", tr!("drill-hole-colour-title", name = dataset.name.clone()))
        .open(&mut open)
        .min_width(400.0)
        .max_width(480.0)
        .show(ui.ctx(), |ui| {
            draw_drill_hole_color_editor(ui, dataset, commands, ColorEditorHost::Dialog);
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
            let row_labels = if dataset.color.by_working_section {
                dataset.color.working_section_view(&field.key, categories)
            } else {
                categories.clone()
            };
            // Rows follow the field's order, not the table's.
            let mut table: Option<Vec<crate::model::drill_hole::DrillCategoryColor>> = None;
            let row_height = ui.spacing().interact_size.y;
            let mut draw_row = |ui: &mut egui::Ui, code: &String| {
                let mut color = dataset.color.category_color(code).unwrap_or([1.0; 3]);
                MenuField::new(code.as_str()).show(ui, |ui, _, _| {
                    if crate::ui::widgets::color::edit_rgb(ui, &mut color).changed() {
                        let table = table.get_or_insert_with(|| dataset.color.categories.to_vec());
                        match table.iter_mut().find(|entry| &entry.value == code) {
                            Some(entry) => entry.color = color,
                            None => table.push(crate::model::drill_hole::DrillCategoryColor { value: code.clone(), color }),
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
                        .show_rows(ui, row_height, row_labels.len(), |ui, range| {
                            for code in &row_labels[range] {
                                draw_row(ui, code);
                            }
                        });
                }
                ColorEditorHost::Page => {
                    // The preferences page already scrolls as a whole; lay
                    // the rows out inline instead of nesting a scroll
                    // area that would fight the page's own under the mouse.
                    for code in &row_labels {
                        draw_row(ui, code);
                    }
                }
            }
            if let Some(table) = table {
                commands.push(UiCommand::SetDrillHoleCategoryColors { id, categories: table });
            }
            ui.add_space(2.0);
            if ui.add(MenuButton::new(tr!(literal = "Reset colours"))).clicked() {
                let mut reset: Vec<crate::model::drill_hole::DrillCategoryColor> =
                    dataset.color.categories.iter().filter(|entry| !row_labels.contains(&entry.value)).cloned().collect();
                reset.extend(default_category_colors(&row_labels));
                commands.push(UiCommand::SetDrillHoleCategoryColors { id, categories: reset });
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

    let existing: Vec<&WorkingSection> = dataset.color.working_sections.iter().filter(|section| section.field == field.key).collect();
    let mut remove_name: Option<String> = None;
    for section in &existing {
        MenuField::new(section.name.as_str()).show(ui, |ui, _, _| {
            ui.label(egui::RichText::new(section.codes.join(", ")).weak());
            if ui.small_button(tr!(literal = "−")).clicked() {
                remove_name = Some(section.name.clone());
            }
        });
    }
    if let Some(name) = remove_name {
        let sections: Vec<WorkingSection> = dataset
            .color
            .working_sections
            .iter()
            .filter(|section| !(section.field == field.key && section.name == name))
            .cloned()
            .collect();
        commands.push(UiCommand::SetDrillHoleWorkingSections { id: dataset.id, sections });
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

    let name_response = MenuFieldText::new(tr!(literal = "New section name"), &mut draft.name).show(ui);
    let name_problem = working_section_name_problem(&draft.name, &field.key, categories, &dataset.color.working_sections);
    if let Some(problem) = name_problem
        && matches!(problem, SectionProblem::NameIsCode | SectionProblem::DuplicateName)
    {
        ui.label(egui::RichText::new(problem.message()).weak());
    }

    // Re-checked every frame: a code leaves this list the moment some other
    // section (added, or edited elsewhere) claims it.
    let available: Vec<&String> = categories.iter().filter(|code| dataset.color.working_section_of(&field.key, code).is_none()).collect();
    let row_height = ui.spacing().interact_size.y;
    let mut draw_code_row = |ui: &mut egui::Ui, code: &&String| {
        let mut ticked = draft.codes.contains(code.as_str());
        if MenuFieldBool::new(code.as_str(), &mut ticked).show(ui).changed() {
            if ticked {
                draft.codes.insert((*code).clone());
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
                .show_rows(ui, row_height, available.len(), |ui, range| {
                    for code in &available[range] {
                        draw_code_row(ui, code);
                    }
                });
        }
        ColorEditorHost::Page => {
            for code in &available {
                draw_code_row(ui, code);
            }
        }
    }

    // Only a ticked code still available goes into the new section; a stale
    // tick left over from before another section claimed it does not.
    let ticked_available: Vec<String> = available.iter().copied().filter(|code| draft.codes.contains(code.as_str())).cloned().collect();
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
        let mut sections = dataset.color.working_sections.clone();
        sections.push(WorkingSection {
            name: draft.name.clone(),
            field: field.key.clone(),
            codes: ticked_available,
        });
        commands.push(UiCommand::SetDrillHoleWorkingSections { id: dataset.id, sections });
        draft = WorkingSectionDraft {
            field: field.key.clone(),
            ..WorkingSectionDraft::default()
        };
    }
    ui.data_mut(|data| data.insert_temp(draft_id, draft));
}
