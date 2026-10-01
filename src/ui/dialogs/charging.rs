//! The charge library's dialogs: a product, a loading rule, and deleting
//! either.

use crate::{
    i18n::{tr, tr_format},
    model::{
        blast::{ChargeProduct, ChargeRule, DeckKind, DeckLength, RuleDeck, charge_hole},
        drill_hole::{DrillHole, OpenDrillHoleDataset, TraceStation},
    },
    ui::{
        EditorState,
        state::{BlastLibraryItem, UiCommand},
        widgets::menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldCombo, MenuFieldF64, MenuFieldText, MenuFieldU32},
    },
};

/// Longest downhole delay the rule accepts.
const MAX_DOWNHOLE_DELAY_MS: u32 = 10_000;
/// The hole a rule is previewed on when no pattern is active to take one from.
const PREVIEW_FALLBACK_DEPTH: f64 = 12.0;
const PREVIEW_FALLBACK_DIAMETER: f64 = 0.165;
const DECK_LENGTH_WIDTH: f32 = 64.0;
const DECK_BUTTON_WIDTH: f32 = 24.0;

/// Add or edit one charge product.
pub(crate) fn draw_charge_product_dialog(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let Some(dialog) = editor.charge_product_dialog.as_mut() else {
        return;
    };
    let mut open = true;
    let mut close = false;
    let title = if dialog.original.is_some() {
        tr!(literal = "Edit Charge Product")
    } else {
        tr!(literal = "New Charge Product")
    };
    let library = &editor.blast_library;
    DragableMenu::new("charge_product_dialog", title).open(&mut open).min_width(320.0).show(ui.ctx(), |ui| {
        let product = &mut dialog.product;
        MenuFieldText::new(tr!(literal = "Name"), &mut product.name).hint_text(tr!(literal = "Required")).show(ui);
        let kind_label = product.kind.label();
        MenuFieldCombo::new(
            "charge_product_kind",
            tr!(literal = "Kind"),
            &mut product.kind,
            kind_label,
            DeckKind::ALL.map(|kind| (kind, kind.label().into())),
        )
        .help_text(tr!(literal = "Explosive decks add mass and are primed; stemming and air decks take length only."))
        .show(ui);
        if product.kind == DeckKind::Explosive {
            MenuFieldF64::new(tr!(literal = "Density"), &mut product.density, 0.05..=3.0)
                .help_text(tr!(literal = "In-hole density. Mass per metre is this times the hole's cross-section."))
                .speed(0.01)
                .suffix(" g/cm³")
                .show(ui);
        }
        let mut color = egui::Color32::from_rgb(product.color[0], product.color[1], product.color[2]);
        menu::MenuFieldColor32::new(tr!(literal = "Colour"), &mut color).show(ui);
        product.color = [color.r(), color.g(), color.b()];

        let name = product.name.trim();
        let clash = library.products.iter().any(|other| other.name == name && dialog.original.as_deref() != Some(name));
        if clash {
            ui.colored_label(ui.visuals().error_fg_color, tr!(literal = "Another product already has this name"));
        }
        let can_save = !name.is_empty() && !clash;
        menu::menu_actions(ui, |ui| {
            if (menu::dialog_confirm_pressed(ui.ctx()) || ui.add(MenuButton::new(tr!(literal = "Save")).primary().enabled(can_save)).clicked()) && can_save {
                let mut product = product.clone();
                product.name = name.to_owned();
                commands.push(UiCommand::SaveChargeProduct {
                    original: dialog.original.clone(),
                    product,
                });
                close = true;
            }
            if ui.add(MenuButton::new(tr!(literal = "Cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                close = true;
            }
        });
    });
    if close || !open {
        editor.charge_product_dialog = None;
    }
}

/// A hole to preview a rule on: the median-depth hole of the active pattern,
/// so the preview is of the holes it will actually load.
fn preview_hole(editor: &EditorState, drill_holes: &[OpenDrillHoleDataset]) -> (DrillHole, bool) {
    let from_pattern = editor
        .active_drill_hole
        .and_then(|id| drill_holes.iter().find(|dataset| dataset.id == id && dataset.state.loaded))
        .and_then(|dataset| {
            let mut holes: Vec<&DrillHole> = dataset.dataset.holes.iter().filter(|hole| hole.trace.len() >= 2).collect();
            holes.sort_by(|a, b| {
                let depth = |hole: &DrillHole| hole.trace.last().map_or(0.0, |station| station.depth);
                depth(a).total_cmp(&depth(b))
            });
            holes.get(holes.len() / 2).map(|hole| (*hole).clone())
        });
    match from_pattern {
        Some(hole) => (hole, true),
        None => {
            let collar = glam::DVec3::ZERO;
            (
                DrillHole {
                    dhid: String::new(),
                    collar,
                    diameter: Some(PREVIEW_FALLBACK_DIAMETER),
                    trace: vec![
                        TraceStation { depth: 0.0, position: collar },
                        TraceStation {
                            depth: PREVIEW_FALLBACK_DEPTH,
                            position: collar - glam::DVec3::Z * PREVIEW_FALLBACK_DEPTH,
                        },
                    ],
                    render_ranges: Vec::new(),
                    intervals: Vec::new(),
                },
                false,
            )
        }
    }
}

/// Add or edit one loading rule, with the column it makes previewed live on
/// a hole from the active pattern.
pub(crate) fn draw_charge_rule_dialog(ui: &mut egui::Ui, editor: &mut EditorState, drill_holes: &[OpenDrillHoleDataset], commands: &mut Vec<UiCommand>) {
    if editor.charge_rule_dialog.is_none() {
        return;
    }
    let (sample, from_pattern) = preview_hole(editor, drill_holes);
    // How many holes of the active pattern were loaded by the rule being
    // edited, for the offer to reload them with the edit.
    let loaded_with_original = editor
        .charge_rule_dialog
        .as_ref()
        .and_then(|dialog| dialog.original.clone())
        .and_then(|original| {
            let id = editor.active_drill_hole?;
            let dataset = drill_holes.iter().find(|dataset| dataset.id == id)?;
            Some(dataset.dataset.charges.values().filter(|charge| charge.rule == original).count())
        })
        .unwrap_or(0);
    let products = editor.blast_library.products.clone();
    let rules = editor.blast_library.rules.clone();
    let Some(dialog) = editor.charge_rule_dialog.as_mut() else {
        return;
    };
    let mut open = true;
    let mut close = false;
    let title = if dialog.original.is_some() {
        tr!(literal = "Edit Charge Rule")
    } else {
        tr!(literal = "New Charge Rule")
    };
    DragableMenu::new("charge_rule_dialog", title)
        .open(&mut open)
        .min_width(460.0)
        .max_width(520.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui.ctx(), |ui| {
            let rule = &mut dialog.rule;
            MenuFieldText::new(tr!(literal = "Name"), &mut rule.name).hint_text(tr!(literal = "Required")).show(ui);

            menu::menu_section(ui, tr!(literal = "Decks, collar to toe"));
            draw_deck_rows(ui, rule, &products);

            menu::menu_section(ui, tr!(literal = "Priming"));
            MenuFieldU32::new(tr!(literal = "Downhole delay"), &mut rule.downhole_delay_ms, 0..=MAX_DOWNHOLE_DELAY_MS)
                .suffix(" ms")
                .help_text(tr!(literal = "The in-hole detonator. A hole fires this long after its surface signal arrives."))
                .show(ui);
            MenuFieldF64::new(tr!(literal = "Primer height"), &mut rule.primer_offset, 0.0..=100.0)
                .help_text(tr!(literal = "How far above the base of each explosive deck its primer sits."))
                .speed(0.05)
                .suffix(" m")
                .show(ui);
            MenuFieldF64::new(tr!(literal = "Booster"), &mut rule.booster_kg, 0.0..=50.0)
                .help_text(tr!(literal = "Cast booster mass in each primer."))
                .speed(0.05)
                .suffix(" kg")
                .show(ui);

            menu::menu_section(ui, tr!(literal = "Preview"));
            let problem = rule.problem(&products);
            let name = rule.name.trim().to_owned();
            let clash = rules.iter().any(|other| other.name == name && dialog.original.as_deref() != Some(name.as_str()));
            let depth = sample.trace.last().map_or(0.0, |station| station.depth);
            let hole_text = match sample.diameter {
                Some(diameter) => tr_format!(
                    literal = "%depth% m hole, Ø %diameter% mm",
                    depth = format!("{depth:.1}"),
                    diameter = format!("{:.0}", diameter * 1_000.0)
                ),
                None => tr_format!(literal = "%depth% m hole", depth = format!("{depth:.1}")),
            };
            let source = if from_pattern {
                tr_format!(literal = "On the pattern's median hole: %hole%", hole = hole_text)
            } else {
                tr_format!(literal = "On a typical %hole%", hole = hole_text)
            };
            ui.label(egui::RichText::new(source).weak());
            match (&problem, charge_hole(rule, &products, &sample)) {
                (Some(problem), _) => {
                    ui.colored_label(ui.visuals().error_fg_color, problem);
                }
                (None, Err(_)) => {
                    ui.colored_label(ui.visuals().warn_fg_color, tr!(literal = "The fixed decks do not fit this hole"));
                }
                (None, Ok(charge)) => {
                    let top = sample.trace.first().map_or(0.0, |station| station.depth);
                    crate::ui::elements::blast::draw_column_with_legend(ui, &charge.decks, &charge.primers, top, depth, 110.0);
                    let mass = charge.mass_kg(sample.diameter);
                    if mass > 0.0 {
                        ui.label(tr_format!(literal = "%mass% kg of explosive", mass = format!("{mass:.1}")));
                    }
                }
            }
            if clash {
                ui.colored_label(ui.visuals().error_fg_color, tr!(literal = "Another rule already has this name"));
            }

            let can_save = problem.is_none() && !clash;
            menu::menu_actions(ui, |ui| {
                let mut save = |reload: bool| {
                    let mut rule = rule.clone();
                    rule.name = name.clone();
                    commands.push(UiCommand::SaveChargeRule {
                        original: dialog.original.clone(),
                        rule,
                        reload,
                    });
                };
                if (menu::dialog_confirm_pressed(ui.ctx()) || ui.add(MenuButton::new(tr!(literal = "Save")).primary().enabled(can_save)).clicked()) && can_save {
                    save(false);
                    close = true;
                }
                // A charge is a record of what was loaded, so editing the rule
                // leaves loaded holes alone - unless asked to bring them up to it.
                if loaded_with_original > 0
                    && ui
                        .add(MenuButton::new(tr_format!(literal = "Save and Reload %count% Hole(s)", count = loaded_with_original)).enabled(can_save))
                        .clicked()
                {
                    save(true);
                    close = true;
                }
                if ui.add(MenuButton::new(tr!(literal = "Cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.charge_rule_dialog = None;
    }
}

/// One row per deck: product, fixed length or fill, and reorder and remove
/// buttons; then a button to add one.
fn draw_deck_rows(ui: &mut egui::Ui, rule: &mut ChargeRule, products: &[ChargeProduct]) {
    let mut swap = None;
    let mut remove = None;
    let count = rule.decks.len();
    let has_fill = rule.decks.iter().any(|deck| deck.length == DeckLength::Fill);
    for (index, deck) in rule.decks.iter_mut().enumerate() {
        MenuField::new(tr_format!(literal = "Deck %number%", number = index + 1)).show(ui, |ui, row_height, column_width| {
            ui.allocate_ui_with_layout(egui::vec2(column_width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let buttons = 3.0 * (DECK_BUTTON_WIDTH + 4.0);
                let combo_width = (column_width - DECK_LENGTH_WIDTH * 2.0 - buttons - 12.0).max(90.0);
                egui::ComboBox::from_id_salt(("rule_deck_product", index))
                    .selected_text(&deck.product)
                    .width(combo_width)
                    .truncate()
                    .show_ui(ui, |ui| {
                        for product in products {
                            ui.selectable_value(&mut deck.product, product.name.clone(), &product.name);
                        }
                    });
                let mut fill = deck.length == DeckLength::Fill;
                let fill_label = if fill { tr!(literal = "Fill") } else { tr!(literal = "Fixed") };
                ui.add_enabled_ui(fill || !has_fill, |ui| {
                    egui::ComboBox::from_id_salt(("rule_deck_mode", index))
                        .selected_text(fill_label)
                        .width(DECK_LENGTH_WIDTH)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut fill, false, tr!(literal = "Fixed"));
                            ui.selectable_value(&mut fill, true, tr!(literal = "Fill"));
                        })
                        .response
                        .on_hover_text(tr!(literal = "A fill deck takes whatever length the fixed decks leave. One per rule."));
                });
                match (fill, deck.length) {
                    (true, DeckLength::Fixed(_)) => deck.length = DeckLength::Fill,
                    (false, DeckLength::Fill) => deck.length = DeckLength::Fixed(1.0),
                    _ => {}
                }
                match &mut deck.length {
                    DeckLength::Fixed(length) => {
                        ui.add_sized(
                            [DECK_LENGTH_WIDTH, row_height],
                            egui::DragValue::new(length).range(0.05..=1_000.0).speed(0.05).max_decimals(2).suffix(" m"),
                        );
                    }
                    DeckLength::Fill => {
                        ui.add_sized([DECK_LENGTH_WIDTH, row_height], egui::Label::new(egui::RichText::new(tr!(literal = "rest")).weak()));
                    }
                }
                if ui
                    .add_enabled(index > 0, egui::Button::new("↑").min_size(egui::vec2(DECK_BUTTON_WIDTH, 0.0)))
                    .on_hover_text(tr!(literal = "Move up"))
                    .clicked()
                {
                    swap = Some((index - 1, index));
                }
                if ui
                    .add_enabled(index + 1 < count, egui::Button::new("↓").min_size(egui::vec2(DECK_BUTTON_WIDTH, 0.0)))
                    .on_hover_text(tr!(literal = "Move down"))
                    .clicked()
                {
                    swap = Some((index, index + 1));
                }
                if ui
                    .add(egui::Button::new("×").min_size(egui::vec2(DECK_BUTTON_WIDTH, 0.0)))
                    .on_hover_text(tr!(literal = "Remove deck"))
                    .clicked()
                {
                    remove = Some(index);
                }
            })
            .response
        });
    }
    if let Some((a, b)) = swap {
        rule.decks.swap(a, b);
    }
    if let Some(index) = remove {
        rule.decks.remove(index);
    }
    ui.add_space(2.0);
    if ui.add(MenuButton::new(tr!(literal = "Add Deck"))).clicked() {
        let product = products.iter().find(|product| product.kind == DeckKind::Stemming).or(products.first());
        rule.decks.push(RuleDeck {
            product: product.map_or_else(String::new, |product| product.name.clone()),
            length: DeckLength::Fixed(1.0),
        });
    }
}

/// Confirm deleting a product or rule from the library. Not undoable - the
/// library is configuration - so it gets the confirmation every destructive
/// delete does.
pub(crate) fn draw_delete_blast_item_dialog(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let Some(item) = editor.pending_delete_blast_item.clone() else {
        return;
    };
    let (kind, note) = match &item {
        BlastLibraryItem::Product(name) => {
            let users = editor.blast_library.rules.iter().filter(|rule| rule.decks.iter().any(|deck| &deck.product == name)).count();
            (
                tr!(literal = "Product"),
                (users > 0).then(|| tr_format!(literal = "%count% rule(s) load this product and will need another chosen.", count = users)),
            )
        }
        BlastLibraryItem::Rule(_) => (tr!(literal = "Rule"), Some(tr!(literal = "Holes already loaded with it keep their charge."))),
    };
    let title = tr!("dialog-delete-title", kind = kind);
    let mut open = true;
    DragableMenu::new("delete_blast_item_dialog", title.clone())
        .open(&mut open)
        .min_width(300.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("confirm-delete-product", name = item.name().to_owned()));
            if let Some(note) = note {
                menu::menu_note(ui, note);
            }
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(title.clone()).danger()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                    commands.push(UiCommand::DeleteBlastLibraryItem(item.clone()));
                    editor.pending_delete_blast_item = None;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.pending_delete_blast_item = None;
                }
            });
        });
    if !open {
        editor.pending_delete_blast_item = None;
    }
}
