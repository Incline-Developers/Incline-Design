//! The charge library's dialogs: a product, a loading rule, and deleting
//! either.

use crate::{
    i18n::tr,
    model::{
        blast::{ChargeProduct, ChargeRule, DeckKind, DeckLength, RuleDeck, lay_rule},
        drill_hole::OpenDrillHoleDataset,
    },
    ui::{
        EditorState,
        state::{BlastLibraryItem, UiCommand},
        widgets::menu::{self, DragableMenu, MenuButton, MenuFieldCombo, MenuFieldF64, MenuFieldText},
    },
};

/// Longest downhole delay the rule accepts.
const MAX_DOWNHOLE_DELAY_MS: u32 = 10_000;
/// The hole a rule is previewed on when no pattern is active to take one from.
const PREVIEW_FALLBACK_DEPTH: f64 = 12.0;
const PREVIEW_FALLBACK_DIAMETER: f64 = 0.165;
const RULE_DIALOG_WIDTH: f32 = 500.0;
/// Deck table columns.
const DECK_PRODUCT_WIDTH: f32 = 200.0;
const DECK_LENGTH_WIDTH: f32 = 112.0;
const DECK_ACTION_SIZE: f32 = 20.0;
const PREVIEW_BAR_HEIGHT: f32 = 28.0;
/// Least room between two depth labels under the preview bar.
const PREVIEW_TICK_SPACING: f32 = 34.0;

/// Add or edit one charge product.
pub(crate) fn draw_charge_product_dialog(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let Some(dialog) = editor.charge_product_dialog.as_mut() else {
        return;
    };
    let mut open = true;
    let mut close = false;
    let title = if dialog.original.is_some() {
        tr!("charging-edit-charge-product")
    } else {
        tr!("charging-new-charge-product")
    };
    let library = &editor.blast_library;
    DragableMenu::new("charge_product_dialog", title).open(&mut open).min_width(320.0).show(ui.ctx(), |ui| {
        let product = &mut dialog.product;
        MenuFieldText::new(tr!("survey-system-name"), &mut product.name)
            .hint_text(tr!("dialog-rename-field-hint"))
            .show(ui);
        let kind_label = product.kind.label();
        MenuFieldCombo::new(
            "charge_product_kind",
            tr!("survey-kind"),
            &mut product.kind,
            kind_label,
            DeckKind::ALL.map(|kind| (kind, kind.label().into())),
        )
        .help_text(tr!("charging-explosive-decks-add-mass-primed-stemming"))
        .show(ui);
        if product.kind == DeckKind::Explosive {
            MenuFieldF64::new(tr!("charging-density"), &mut product.density, 0.05..=3.0)
                .help_text(tr!("charging-density-hint"))
                .speed(0.01)
                .suffix(" g/cm³")
                .show(ui);
        }
        let mut color = egui::Color32::from_rgb(product.color[0], product.color[1], product.color[2]);
        menu::MenuFieldColor32::new(tr!("common-colour"), &mut color).show(ui);
        product.color = [color.r(), color.g(), color.b()];

        let name = product.name.trim();
        let clash = library.products.iter().any(|other| other.name == name && dialog.original.as_deref() != Some(name));
        if clash {
            ui.colored_label(ui.visuals().error_fg_color, tr!("charging-another-product-already-has-name"));
        }
        let can_save = !name.is_empty() && !clash;
        menu::menu_actions(ui, |ui| {
            if (menu::dialog_confirm_pressed(ui.ctx()) || ui.add(MenuButton::new(tr!("confirmations-save")).primary().enabled(can_save)).clicked()) && can_save {
                let mut product = product.clone();
                product.name = name.to_owned();
                commands.push(UiCommand::SaveChargeProduct {
                    original: dialog.original.clone(),
                    product,
                });
                close = true;
            }
            if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                close = true;
            }
        });
    });
    if close || !open {
        editor.charge_product_dialog = None;
    }
}

/// The hole a rule is first previewed on: the median-depth hole of the
/// active pattern, so the preview is of the holes the rule will load.
fn pattern_preview(editor: &EditorState, drill_holes: &[OpenDrillHoleDataset]) -> Option<(f64, f64)> {
    let id = editor.active_drill_hole?;
    let dataset = drill_holes.iter().find(|dataset| dataset.id == id && dataset.state.loaded)?;
    let mut holes: Vec<(f64, Option<f64>)> = dataset
        .dataset
        .holes
        .iter()
        .filter_map(|hole| Some((hole.trace.last()?.depth - hole.trace.first()?.depth, hole.diameter)))
        .filter(|(depth, _)| *depth > 0.0)
        .collect();
    holes.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (depth, diameter) = *holes.get(holes.len() / 2)?;
    Some((depth, diameter.unwrap_or(PREVIEW_FALLBACK_DIAMETER)))
}

fn product_color(products: &[ChargeProduct], name: &str) -> egui::Color32 {
    products
        .iter()
        .find(|product| product.name == name)
        .map_or(egui::Color32::GRAY, |product| egui::Color32::from_rgb(product.color[0], product.color[1], product.color[2]))
}

/// Add or edit one loading rule: the decks as a table, the priming on one
/// line, and the column it makes drawn to scale on an editable hole.
pub(crate) fn draw_charge_rule_dialog(ui: &mut egui::Ui, editor: &mut EditorState, drill_holes: &[OpenDrillHoleDataset], commands: &mut Vec<UiCommand>) {
    if editor.charge_rule_dialog.is_none() {
        return;
    }
    let from_pattern = pattern_preview(editor, drill_holes);
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
    let (depth, diameter) = *dialog.preview.get_or_insert(from_pattern.unwrap_or((PREVIEW_FALLBACK_DEPTH, PREVIEW_FALLBACK_DIAMETER)));
    let mut open = true;
    let mut close = false;
    let title = if dialog.original.is_some() {
        tr!("charging-edit-charge-rule")
    } else {
        tr!("charging-new-charge-rule")
    };
    DragableMenu::new("charge_rule_dialog", title)
        .open(&mut open)
        .min_width(RULE_DIALOG_WIDTH)
        .max_width(RULE_DIALOG_WIDTH)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui.ctx(), |ui| {
            let rule = &mut dialog.rule;
            MenuFieldText::new(tr!("survey-system-name"), &mut rule.name)
                .hint_text(tr!("dialog-rename-field-hint"))
                .show(ui);

            menu::menu_section(ui, tr!("charging-decks-collar-toe"));
            // Laid on the preview hole, for the length a fill deck comes to.
            let laid = lay_rule(rule, &products, 0.0, depth).ok();
            draw_deck_table(ui, rule, &products, laid.as_ref());

            menu::menu_section(ui, tr!("charging-priming"));
            draw_priming(ui, rule);

            menu::menu_section(ui, tr!("charging-preview"));
            let preview = dialog.preview.as_mut().expect("set above");
            // Labelled fields rather than a sentence built around the inputs, which no
            // translation could reorder.
            MenuFieldF64::new(tr!("drill-pattern-hole-depth"), &mut preview.0, 0.5..=200.0)
                .max_decimals(1)
                .suffix(format!(" {}", tr!("common-m")))
                .show(ui);
            let mut millimetres = preview.1 * 1_000.0;
            if MenuFieldF64::new(tr!("drill-pattern-hole-diameter"), &mut millimetres, 20.0..=1_000.0)
                .speed(1.0)
                .max_decimals(0)
                .suffix(" mm")
                .show(ui)
                .changed()
            {
                preview.1 = millimetres / 1_000.0;
            }
            if let Some(pattern) = from_pattern
                && (pattern.0 - preview.0).abs() + (pattern.1 - preview.1).abs() > 1.0e-9
                && ui
                    .add(MenuButton::new(tr!("charging-preview-use-pattern-hole")))
                    .on_hover_text(tr!("charging-preview-active-pattern-median-hole"))
                    .clicked()
            {
                *preview = pattern;
            }
            ui.add_space(6.0);
            let problem = rule.problem(&products);
            match (&problem, &laid) {
                (Some(problem), _) => {
                    ui.colored_label(ui.visuals().error_fg_color, problem);
                }
                (None, None) => {
                    ui.colored_label(ui.visuals().warn_fg_color, tr!("charging-fixed-decks-longer-than-hole"));
                }
                (None, Some(charge)) => {
                    draw_preview_bar(ui, charge, depth);
                    ui.add_space(4.0);
                    let explosive: f64 = charge.decks.iter().filter(|deck| deck.kind == DeckKind::Explosive).map(|deck| deck.length()).sum();
                    let mass = charge.mass_kg(Some(diameter));
                    let mut stats = vec![tr!("charging-mass-kg-explosive", mass = format!("{mass:.1}"))];
                    if explosive > 0.0 {
                        stats.push(tr!(
                            "charging-rate-kg-m",
                            rate = format!("{:.1}", (mass - charge.primers.iter().map(|primer| primer.booster_kg).sum::<f64>()) / explosive)
                        ));
                    }
                    stats.push(tr!("charging-count-primer", count = charge.primers.len().to_string()));
                    ui.label(egui::RichText::new(stats.join("  ·  ")).weak());
                }
            }

            let name = rule.name.trim().to_owned();
            let clash = rules.iter().any(|other| other.name == name && dialog.original.as_deref() != Some(name.as_str()));
            if clash {
                ui.colored_label(ui.visuals().error_fg_color, tr!("charging-another-rule-already-has-name"));
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
                if (menu::dialog_confirm_pressed(ui.ctx()) || ui.add(MenuButton::new(tr!("confirmations-save")).primary().enabled(can_save)).clicked()) && can_save {
                    save(false);
                    close = true;
                }
                // A charge is a record of what was loaded, so editing the rule
                // leaves loaded holes alone - unless asked to bring them up to it.
                if loaded_with_original > 0
                    && ui
                        .add(MenuButton::new(tr!("charging-save-reload-count-hole", count = loaded_with_original.to_string())).enabled(can_save))
                        .clicked()
                {
                    save(true);
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if close || !open {
        editor.charge_rule_dialog = None;
    }
}

/// A small frameless button for a table row's actions.
fn row_action(ui: &mut egui::Ui, glyph: &str, enabled: bool, tooltip: String) -> bool {
    ui.add_enabled(enabled, egui::Button::new(glyph).frame(false).min_size(egui::vec2(DECK_ACTION_SIZE, DECK_ACTION_SIZE)))
        .on_hover_text(tooltip)
        .clicked()
}

/// The decks as a table: product with its colour, length - or what the fill
/// deck comes to - and a Fill toggle that moves the fill to its row, then
/// reorder and remove.
fn draw_deck_table(ui: &mut egui::Ui, rule: &mut ChargeRule, products: &[ChargeProduct], laid: Option<&crate::model::blast::HoleCharge>) {
    let mut swap = None;
    let mut remove = None;
    let mut make_fill = None;
    let mut make_fixed = None;
    let count = rule.decks.len();
    let weak = ui.visuals().weak_text_color();
    egui::Grid::new("rule_deck_table").num_columns(5).spacing([8.0, 4.0]).show(ui, |ui| {
        for heading in [String::new(), tr!("confirmations-product"), tr!("charging-length"), String::new(), String::new()] {
            ui.label(egui::RichText::new(heading).small().color(weak));
        }
        ui.end_row();
        for (index, deck) in rule.decks.iter_mut().enumerate() {
            ui.label(egui::RichText::new((index + 1).to_string()).color(weak));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (swatch, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().rect_filled(swatch, 2.0, product_color(products, &deck.product));
                egui::ComboBox::from_id_salt(("rule_deck_product", index))
                    .selected_text(&deck.product)
                    .width(DECK_PRODUCT_WIDTH - 16.0)
                    .truncate()
                    .show_ui(ui, |ui| {
                        for product in products {
                            ui.selectable_value(&mut deck.product, product.name.clone(), &product.name);
                        }
                    });
            });
            ui.allocate_ui_with_layout(
                egui::vec2(DECK_LENGTH_WIDTH, DECK_ACTION_SIZE),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| match &mut deck.length {
                    DeckLength::Fixed(length) => {
                        ui.add_sized(
                            [DECK_LENGTH_WIDTH, DECK_ACTION_SIZE],
                            egui::DragValue::new(length).range(0.05..=1_000.0).speed(0.05).max_decimals(2).suffix(" m"),
                        );
                    }
                    DeckLength::Fill => {
                        let rest = laid.and_then(|charge| charge.decks.get(index)).map(|deck| deck.length());
                        let text = match rest {
                            Some(rest) => tr!("charging-rest-length-m", length = format!("{rest:.2}")),
                            None => tr!("charging-rest"),
                        };
                        ui.label(egui::RichText::new(text).italics().color(weak));
                    }
                },
            );
            let fill = deck.length == DeckLength::Fill;
            if ui
                .selectable_label(fill, tr!("common-fill"))
                .on_hover_text(tr!("charging-deck-takes-whatever-length-fixed-decks"))
                .clicked()
            {
                if fill {
                    make_fixed = Some(index);
                } else {
                    make_fill = Some(index);
                }
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if row_action(ui, "↑", index > 0, tr!("object-edit-move-up")) {
                    swap = Some((index - 1, index));
                }
                if row_action(ui, "↓", index + 1 < count, tr!("object-edit-move-down")) {
                    swap = Some((index, index + 1));
                }
                if row_action(ui, "×", true, tr!("charging-remove-deck")) {
                    remove = Some(index);
                }
            });
            ui.end_row();
        }
    });
    // A deck leaving the fill keeps the length it was filling, so moving the
    // fill to another row does not throw away the shape of the column.
    let laid_length = |index: usize| {
        laid.and_then(|charge| charge.decks.get(index))
            .map_or(1.0, |deck| (deck.length() * 10.0).round().max(1.0) / 10.0)
    };
    if let Some(index) = make_fill {
        for (other, deck) in rule.decks.iter_mut().enumerate() {
            if deck.length == DeckLength::Fill {
                deck.length = DeckLength::Fixed(laid_length(other));
            }
        }
        rule.decks[index].length = DeckLength::Fill;
    }
    if let Some(index) = make_fixed {
        rule.decks[index].length = DeckLength::Fixed(laid_length(index));
    }
    if let Some((a, b)) = swap {
        rule.decks.swap(a, b);
    }
    if let Some(index) = remove {
        rule.decks.remove(index);
    }
    ui.add_space(4.0);
    if ui.add(MenuButton::new(tr!("charging-add-deck"))).clicked() {
        let product = products.iter().find(|product| product.kind == DeckKind::Stemming).or(products.first());
        rule.decks.push(RuleDeck {
            product: product.map_or_else(String::new, |product| product.name.clone()),
            length: DeckLength::Fixed(1.0),
        });
    }
}

/// The three priming settings side by side, each named above its value.
fn draw_priming(ui: &mut egui::Ui, rule: &mut ChargeRule) {
    let weak = ui.visuals().weak_text_color();
    egui::Grid::new("rule_priming").num_columns(3).spacing([16.0, 2.0]).show(ui, |ui| {
        let heading = |ui: &mut egui::Ui, text: String, help: String| {
            ui.label(egui::RichText::new(text).small().color(weak)).on_hover_text(help);
        };
        heading(ui, tr!("charging-downhole-delay"), tr!("charging-hole-detonator-hole-fires-long-after"));
        heading(ui, tr!("charging-primer-height"), tr!("charging-how-far-above-base-each-explosive"));
        heading(ui, tr!("charging-booster"), tr!("charging-cast-booster-mass-each-primer"));
        ui.end_row();
        let width = (RULE_DIALOG_WIDTH - 32.0) / 3.0;
        ui.add_sized(
            [width, DECK_ACTION_SIZE],
            egui::DragValue::new(&mut rule.downhole_delay_ms).range(0..=MAX_DOWNHOLE_DELAY_MS).suffix(" ms"),
        );
        ui.add_sized(
            [width, DECK_ACTION_SIZE],
            egui::DragValue::new(&mut rule.primer_offset).range(0.0..=100.0).speed(0.05).max_decimals(2).suffix(" m"),
        );
        ui.add_sized(
            [width, DECK_ACTION_SIZE],
            egui::DragValue::new(&mut rule.booster_kg).range(0.0..=50.0).speed(0.05).max_decimals(2).suffix(" kg"),
        );
        ui.end_row();
    });
}

/// The loaded column on its side across the dialog, collar at the left: each
/// deck as a band to scale with its product named inside where it fits,
/// primers marked where they sit, and the depth of every boundary beneath.
fn draw_preview_bar(ui: &mut egui::Ui, charge: &crate::model::blast::HoleCharge, depth: f64) {
    let weak = ui.visuals().weak_text_color();
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, PREVIEW_BAR_HEIGHT + 16.0), egui::Sense::hover());
    let bar = egui::Rect::from_min_size(rect.min, egui::vec2(width, PREVIEW_BAR_HEIGHT));
    let painter = ui.painter();
    let x = |at: f64| bar.left() + (at / depth).clamp(0.0, 1.0) as f32 * bar.width();
    painter.rect_filled(bar, 3.0, ui.visuals().extreme_bg_color);
    let font = egui::FontId::proportional(11.0);
    for deck in &charge.decks {
        let band = egui::Rect::from_x_y_ranges(x(deck.from)..=x(deck.to), bar.y_range());
        let [red, green, blue] = deck.color.map(|channel| (channel * 255.0) as u8);
        let fill = egui::Color32::from_rgb(red, green, blue);
        painter.rect_filled(band, 0.0, fill);
        // Dark text on a light band, light on a dark one.
        let luminance = 0.299 * f32::from(red) + 0.587 * f32::from(green) + 0.114 * f32::from(blue);
        let ink = if luminance > 140.0 {
            egui::Color32::from_black_alpha(220)
        } else {
            egui::Color32::WHITE
        };
        let galley = painter.layout_no_wrap(deck.product.clone(), font.clone(), ink);
        if galley.size().x + 8.0 <= band.width() {
            painter.galley(band.center() - galley.size() * 0.5, galley, ink);
        }
    }
    for primer in &charge.primers {
        let centre = egui::pos2(x(primer.depth), bar.center().y);
        let size = 5.0;
        let diamond = vec![
            centre + egui::vec2(0.0, -size),
            centre + egui::vec2(size, 0.0),
            centre + egui::vec2(0.0, size),
            centre + egui::vec2(-size, 0.0),
        ];
        painter.add(egui::Shape::convex_polygon(
            diamond,
            egui::Color32::from_rgb(0xFF, 0xD2, 0x3D),
            egui::Stroke::new(1.0, egui::Color32::from_rgb(0x6A, 0x46, 0x00)),
        ));
    }
    painter.rect_stroke(bar, 3.0, ui.visuals().widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);

    // Depths under every boundary, collar to toe, skipping any that would
    // crowd the one before.
    let mut boundaries: Vec<f64> = std::iter::once(0.0).chain(charge.decks.iter().map(|deck| deck.to)).collect();
    boundaries.dedup_by(|a, b| (*a - *b).abs() < 1.0e-6);
    let mut last_label: Option<f32> = None;
    for (index, at) in boundaries.iter().enumerate() {
        let tick_x = x(*at);
        painter.line_segment([egui::pos2(tick_x, bar.bottom()), egui::pos2(tick_x, bar.bottom() + 3.0)], egui::Stroke::new(1.0, weak));
        let is_last = index + 1 == boundaries.len();
        if last_label.is_some_and(|previous| tick_x - previous < PREVIEW_TICK_SPACING) && !is_last {
            continue;
        }
        let align = if index == 0 {
            egui::Align2::LEFT_TOP
        } else if is_last {
            egui::Align2::RIGHT_TOP
        } else {
            egui::Align2::CENTER_TOP
        };
        painter.text(egui::pos2(tick_x, bar.bottom() + 3.0), align, format!("{at:.1}"), egui::FontId::proportional(10.0), weak);
        last_label = Some(tick_x);
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
                tr!("confirmations-product"),
                (users > 0).then(|| tr!("charging-count-rule-load-product-will-need", count = users.to_string())),
            )
        }
        BlastLibraryItem::Rule(_) => (tr!("charging-rule"), Some(tr!("charging-holes-already-loaded-keep-their-charge"))),
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
