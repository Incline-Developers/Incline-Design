//! The Drill & Blast workspace's products island below the data explorer.
//!
//! It is built out of the explorer's own parts - the banded rows, the coloured
//! section heading carrying the only symbol in the panel, the lit row for the
//! one that is armed - so the two islands read as one interface rather
//! than as a tree beside a board of cards. A product fits a row: the delay, in
//! the colour a tie-in laid with it is drawn in, and the name after it.
//! Interhole delays are the only product so far - see
//! [`crate::ui::state::DelayProduct`] - so the palette is one section, and a
//! new kind would be another beside it.
//!
//! Nothing in the panel is a button: a product is chosen by clicking its row,
//! and the palette itself is edited from the section heading's right-click
//! menu, so the list holds products and only products.
//!
//! The charge library sits below the delays in the same form: loading rules,
//! one of which the Charge Holes tool stands on the way the Tie Holes tool
//! stands on a delay, and the products those rules stack into a column.

use crate::{
    i18n::tr,
    model::blast::{BlastLibrary, ChargeProduct, ChargeRule, DeckLength},
    ui::{
        EditorState,
        state::{BlastLibraryItem, ChargeProductDialog, ChargeRuleDialog, DelayProduct, UiCommand},
        unthemed_icon,
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            explorer::{ExplorerEntry, ExplorerHeader, explorer_note, paint_fixed_stripes, reserve_fixed_stripes},
        },
    },
};

/// Id of the products panel. Shared with [`crate::ui::chrome`], which reads
/// the panel's resize interaction to light up its grip.
pub(crate) const PANEL_ID: &str = "products_panel";

/// Heading tint for the palette: the red the heading's own tie-in icon is
/// drawn in, so the section reads as one mark rather than as a symbol beside
/// a differently coloured name. Like the explorer's tints it is one colour for
/// both themes, holding better than 4:1 against the light panel and the dark
/// one alike.
const HEADER_DELAY_PALETTE: egui::Color32 = egui::Color32::from_rgb(0xE2, 0x3B, 0x46);

/// Heading tint for the charge sections: the explosive pink of their icon.
const HEADER_CHARGE: egui::Color32 = egui::Color32::from_rgb(0xE0, 0x4F, 0x8C);

/// The column bar at the right of a rule's row.
const RULE_BAR_WIDTH: f32 = 58.0;
const RULE_BAR_HEIGHT: f32 = 8.0;
/// Clear space between the bar and the panel's right edge.
const RULE_BAR_INSET: f32 = 10.0;
/// The hole a rule is pictured on when no pattern is active.
const RULE_BAR_FALLBACK_DEPTH: f64 = 12.0;

/// Space between a product's delay and the name after it.
const LABEL_GAP: f32 = 6.0;

/// Draw the products panel and return what it claimed.
pub(crate) fn draw_products_panel(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) -> egui::Rect {
    // The explorer's row colours, because these are the explorer's rows: the
    // two islands share one palette rather than each mixing its own.
    let (surface, stripe) = crate::ui::widgets::tree_row_colors(ui);
    let (min_height, max_height) = crate::ui::chrome::panel_size_limits(ui.ctx(), ui.available_height());
    egui::Panel::bottom(PANEL_ID)
        .resizable(true)
        .default_size(max_height)
        .min_size(min_height)
        .max_size(max_height)
        .show_separator_line(crate::ui::chrome::show_separator_line(ui))
        .frame(crate::ui::chrome::region_frame(ui).fill(surface).inner_margin(egui::Margin::ZERO))
        .show(ui, |ui| {
            // Prevent content from forcing the panel wider than the user has dragged it.
            ui.set_max_width(ui.available_width());
            // Scroll overflowing products within the island's chosen height
            // so the data explorer keeps its share of the column.
            egui::ScrollArea::vertical()
                .id_salt("products_panel_scroll")
                .auto_shrink([false; 2])
                .min_scrolled_height(0.0)
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    // A name longer than the column ends in an ellipsis rather
                    // than widening the panel.
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                    // Rows carry their own height and butt up against each
                    // other, so the banding tiles the list without gaps.
                    ui.spacing_mut().item_spacing.y = 0.0;

                    // Reserved before any row is laid out and filled once the
                    // list's height is known: see `paint_fixed_stripes`.
                    let (stripes_slot, stripes_top) = reserve_fixed_stripes(ui);
                    let (toggle, header, _) = ExplorerHeader::new(egui::Id::new("delay_palette_collapse"), tr!("products-delay-palette"))
                        .icon(unthemed_icon!("tie_holes.svg"))
                        .color(HEADER_DELAY_PALETTE)
                        .show(ui, |ui| draw_delay_palette(ui, editor));
                    // Adding to the palette is editing the palette rather than
                    // one product in it, so it hangs off the section heading -
                    // the way the explorer's own section menus do - instead of
                    // taking a permanent row at the foot of the list.
                    context_menu_popup(&toggle.union(header.inner), tr!("products-delay-palette"), |ui| {
                        if ContextMenuAction::new(tr!("common-new-product")).show(ui).clicked() {
                            editor.begin_new_delay_product();
                            ui.close();
                        }
                    });

                    let (toggle, header, _) = ExplorerHeader::new(egui::Id::new("charge_rules_collapse"), tr!("products-charge-rules"))
                        .icon(unthemed_icon!("charge_holes.svg"))
                        .color(HEADER_CHARGE)
                        .show(ui, |ui| draw_charge_rules(ui, editor, commands));
                    context_menu_popup(&toggle.union(header.inner), tr!("products-charge-rules"), |ui| {
                        if ContextMenuAction::new(tr!("products-new-rule")).show(ui).clicked() {
                            editor.charge_rule_dialog = Some(ChargeRuleDialog::new(None, new_rule(&editor.blast_library)));
                            ui.close();
                        }
                    });

                    let (toggle, header, _) = ExplorerHeader::new(egui::Id::new("charge_products_collapse"), tr!("products-charge-products"))
                        .icon(unthemed_icon!("charge_products.svg"))
                        .color(HEADER_CHARGE)
                        .default_open(false)
                        .show(ui, |ui| draw_charge_products(ui, editor));
                    context_menu_popup(&toggle.union(header.inner), tr!("products-charge-products"), |ui| {
                        if ContextMenuAction::new(tr!("common-new-product")).show(ui).clicked() {
                            editor.charge_product_dialog = Some(ChargeProductDialog {
                                original: None,
                                product: ChargeProduct {
                                    name: String::new(),
                                    kind: crate::model::blast::DeckKind::Explosive,
                                    density: 1.0,
                                    color: [0xD0, 0x7A, 0x4A],
                                },
                            });
                            ui.close();
                        }
                    });

                    paint_fixed_stripes(ui, stripes_slot, stripes_top, stripe);
                });
        })
        .response
        .rect
}

/// The palette itself: one row per stored product.
///
/// Every row is live whether or not the Tie Holes tool is armed. Which
/// product a tie-in will be laid with is a standing choice, not a state of
/// the tool: it is worth setting before the tool is picked up, and the
/// palette always stands on one - the first, until another is clicked - so
/// arming the tool never lands on nothing.
fn draw_delay_palette(ui: &mut egui::Ui, editor: &mut EditorState) {
    let mut select = None;
    let mut delete = None;

    if editor.delay_products.is_empty() {
        explorer_note(ui, tr!("products-no-products"));
    }
    // The palette stands on its first product whenever the selection has
    // nothing to point at - a config that never named one, say - so the row
    // a tie-in would use is always a marked one. Settled before the rows are
    // laid out, so the mark appears in the same frame rather than the next.
    if editor.active_delay_product.is_none() {
        editor.active_delay_product = editor.delay_products.first().map(|product| product.id);
    }
    for product in &editor.delay_products {
        let chosen = editor.active_delay_product == Some(product.id);
        // Two marks, because the row has to be readable as chosen at a
        // glance in a list of near-identical rows: the lit fill the explorer
        // gives its active layer, and a dot in the label gutter in the
        // product's own colour - the colour a tie-in laid with it is drawn
        // in. The gutter is reserved on every row, so the dot marks one
        // without shifting the others.
        let mut entry = ExplorerEntry::new(egui::Id::new(("delay_product", product.id)), product_label(ui, product)).selected(chosen);
        if chosen {
            entry = entry.leading_icon(unthemed_icon!("product_active.svg"), product.color);
        }
        let response = entry.show(ui).response;
        if response.clicked() {
            select = Some(product.id);
        }
        let label = format!("{} {}", product.delay_ms, product.name);
        context_menu_popup(&response, label.clone(), |ui| {
            if ContextMenuAction::new(tr!("common-delete-product")).show(ui).clicked() {
                delete = Some((product.id, label.clone()));
                ui.close();
            }
        });
    }

    if let Some(id) = select {
        editor.active_delay_product = Some(id);
    }
    // Deleting a product is not undoable, so it goes through the same
    // confirmation every other destructive delete does; the command is only
    // pushed once that dialog is accepted.
    if let Some(pending) = delete {
        editor.pending_delete_delay_product = Some(pending);
    }
}

/// One product as a row's label: the delay in the product's own colour, with
/// its name after it in the weight the panel gives secondary text.
///
/// Laid out as one [`egui::text::LayoutJob`] rather than as two widgets, so
/// the row stays a single label that carries the click, the context menu and
/// the selection fill, and truncates as a whole when the panel is narrowed.
fn product_label(ui: &egui::Ui, product: &DelayProduct) -> egui::WidgetText {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &product.delay_ms.to_string(),
        0.0,
        egui::TextFormat {
            font_id: crate::ui::fonts::bold_font(font.size),
            color: product.color,
            ..Default::default()
        },
    );
    job.append(
        &product.name,
        LABEL_GAP,
        egui::TextFormat {
            font_id: font,
            color: ui.visuals().weak_text_color(),
            ..Default::default()
        },
    );
    job.into()
}

/// A fresh rule to start the New Rule dialog from: stemming over a column of
/// the first explosive in the library, named so it does not clash.
fn new_rule(library: &BlastLibrary) -> ChargeRule {
    let stemming = library.products.iter().find(|product| product.kind == crate::model::blast::DeckKind::Stemming);
    let explosive = library.products.iter().find(|product| product.kind == crate::model::blast::DeckKind::Explosive);
    let mut decks = Vec::new();
    if let Some(stemming) = stemming {
        decks.push(crate::model::blast::RuleDeck {
            product: stemming.name.clone(),
            length: DeckLength::Fixed(3.0),
        });
    }
    if let Some(explosive) = explosive {
        decks.push(crate::model::blast::RuleDeck {
            product: explosive.name.clone(),
            length: DeckLength::Fill,
        });
    }
    let base = tr!("products-new-rule-default-name");
    let mut name = base.clone();
    let mut counter = 2;
    while library.rules.iter().any(|rule| rule.name == name) {
        name = format!("{base} ({counter})");
        counter += 1;
    }
    ChargeRule {
        name,
        decks,
        downhole_delay_ms: 500,
        primer_offset: 0.5,
        booster_kg: 0.4,
    }
}

/// The loading rules: one row each, the one the Charge Holes tool loads with
/// lit. A row's menu loads or unloads the selected holes with it directly.
fn draw_charge_rules(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    if editor.blast_library.rules.is_empty() {
        explorer_note(ui, tr!("products-no-rules"));
    }
    let active = editor.active_rule().map(|rule| rule.name.clone());
    let selected_holes = editor
        .active_drill_hole
        .map_or(0, |id| editor.selected_drill_holes.iter().filter(|hole| hole.dataset == id).count());
    // Rules are pictured on a hole of the active pattern's median depth, so
    // the bar shows the proportions the rule will actually load.
    let depth = editor
        .blast_analysis
        .as_deref()
        .and_then(|analysis| analysis.median_depth)
        .unwrap_or(RULE_BAR_FALLBACK_DEPTH);
    let mut choose = None;
    for rule in &editor.blast_library.rules {
        let chosen = active.as_deref() == Some(rule.name.as_str());
        let problem = rule.problem(&editor.blast_library.products);
        let mut entry = ExplorerEntry::new(egui::Id::new(("charge_rule", rule.name.as_str())), rule_label(ui, rule, problem.is_some()))
            .selected(chosen)
            .trailing(RULE_BAR_WIDTH + RULE_BAR_INSET);
        if chosen {
            entry = entry.leading_icon(unthemed_icon!("product_active.svg"), HEADER_CHARGE);
        }
        let shown = entry.show(ui);
        if let Some(slot) = shown.trailing
            && problem.is_none()
        {
            paint_rule_bar(ui, slot, rule, &editor.blast_library, depth);
        }
        let mut response = shown.response;
        if let Some(problem) = &problem {
            response = response.on_hover_text(problem);
        } else {
            response = response.on_hover_text(rule_description(rule));
        }
        if response.clicked() {
            choose = Some(rule.name.clone());
        }
        if response.double_clicked() {
            editor.charge_rule_dialog = Some(ChargeRuleDialog::new(Some(rule.name.clone()), rule.clone()));
        }
        context_menu_popup(&response, rule.name.clone(), |ui| {
            let can_load = selected_holes > 0 && problem.is_none();
            if ContextMenuAction::new(tr!("products-load-selected-holes-count", count = selected_holes.to_string()))
                .enabled(can_load)
                .show(ui)
                .clicked()
            {
                commands.push(UiCommand::ChargeSelectedHoles { rule: Some(rule.name.clone()) });
                ui.close();
            }
            if ContextMenuAction::new(tr!("products-unload-selected-holes-count", count = selected_holes.to_string()))
                .enabled(selected_holes > 0)
                .show(ui)
                .clicked()
            {
                commands.push(UiCommand::ChargeSelectedHoles { rule: None });
                ui.close();
            }
            if ContextMenuAction::new(tr!("products-edit-rule")).show(ui).clicked() {
                editor.charge_rule_dialog = Some(ChargeRuleDialog::new(Some(rule.name.clone()), rule.clone()));
                ui.close();
            }
            if ContextMenuAction::new(tr!("products-duplicate-rule")).show(ui).clicked() {
                let mut copy = rule.clone();
                copy.name = tr!("cmd-layer-name-copy", name = rule.name.to_string());
                editor.charge_rule_dialog = Some(ChargeRuleDialog::new(None, copy));
                ui.close();
            }
            if ContextMenuAction::new(tr!("products-delete-rule")).show(ui).clicked() {
                editor.pending_delete_blast_item = Some(BlastLibraryItem::Rule(rule.name.clone()));
                ui.close();
            }
        });
    }
    if let Some(name) = choose {
        editor.active_charge_rule = Some(name);
    }
}

/// A rule as a row: its name, with a warning mark when it cannot load
/// anything. The column it makes is painted beside it - see
/// [`paint_rule_bar`].
fn rule_label(ui: &egui::Ui, rule: &ChargeRule, broken: bool) -> egui::WidgetText {
    let mut text = egui::RichText::new(&rule.name).color(ui.visuals().text_color());
    if broken {
        text = egui::RichText::new(format!("{}  ⚠", rule.name)).color(ui.visuals().warn_fg_color);
    }
    text.into()
}

/// The column a rule loads, lying on its side at the right of its row:
/// collar at the left, each deck as long as it would be down a hole of
/// `depth` metres, so "a long column under a short stem" reads at a glance
/// and two rules that differ only in their lengths read apart.
fn paint_rule_bar(ui: &egui::Ui, slot: egui::Rect, rule: &ChargeRule, library: &BlastLibrary, depth: f64) {
    let Ok(charge) = crate::model::blast::lay_rule(rule, &library.products, 0.0, depth) else {
        return;
    };
    let bar = egui::Rect::from_min_size(
        egui::pos2(slot.left(), slot.center().y - RULE_BAR_HEIGHT * 0.5),
        egui::vec2(RULE_BAR_WIDTH, RULE_BAR_HEIGHT),
    );
    let painter = ui.painter();
    painter.rect_filled(bar, 2.0, ui.visuals().extreme_bg_color);
    let x = |at: f64| bar.left() + (at / depth).clamp(0.0, 1.0) as f32 * bar.width();
    for deck in &charge.decks {
        let [red, green, blue] = deck.color.map(|channel| (channel * 255.0) as u8);
        painter.rect_filled(
            egui::Rect::from_x_y_ranges(x(deck.from)..=x(deck.to), bar.y_range()),
            0.0,
            egui::Color32::from_rgb(red, green, blue),
        );
    }
    painter.rect_stroke(bar, 2.0, egui::Stroke::new(1.0, egui::Color32::from_black_alpha(90)), egui::StrokeKind::Inside);
}

/// A rule spelled out deck by deck, for its row's hover text.
fn rule_description(rule: &ChargeRule) -> String {
    let mut lines: Vec<String> = rule
        .decks
        .iter()
        .map(|deck| match deck.length {
            DeckLength::Fixed(length) => format!("{length:.2} m  {}", deck.product),
            DeckLength::Fill => tr!("products-fill-product", product = deck.product.to_string()),
        })
        .collect();
    lines.push(tr!(
        "products-primer-offset-m-off-each-explosive",
        offset = format!("{:.2}", rule.primer_offset),
        booster = format!("{:.2}", rule.booster_kg),
        delay = rule.downhole_delay_ms.to_string()
    ));
    lines.push(tr!("products-double-click-edit"));
    lines.join("\n")
}

/// The charge products: one row each with a swatch of the colour its decks
/// are drawn in.
fn draw_charge_products(ui: &mut egui::Ui, editor: &mut EditorState) {
    if editor.blast_library.products.is_empty() {
        explorer_note(ui, tr!("products-no-products"));
    }
    for product in &editor.blast_library.products {
        let color = egui::Color32::from_rgb(product.color[0], product.color[1], product.color[2]);
        let font = egui::TextStyle::Body.resolve(ui.style());
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &product.name,
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: ui.visuals().text_color(),
                ..Default::default()
            },
        );
        // Only an explosive has anything to add: the inert products'
        // names already say what they are.
        if product.kind == crate::model::blast::DeckKind::Explosive {
            job.append(
                &format!("{:.2} g/cm³", product.density),
                LABEL_GAP,
                egui::TextFormat {
                    font_id: font,
                    color: ui.visuals().weak_text_color(),
                    ..Default::default()
                },
            );
        }
        let response = ExplorerEntry::new(egui::Id::new(("charge_product", product.name.as_str())), job)
            .leading_icon(unthemed_icon!("charge_swatch.svg"), color)
            .show(ui)
            .response;
        if response.double_clicked() {
            editor.charge_product_dialog = Some(ChargeProductDialog {
                original: Some(product.name.clone()),
                product: product.clone(),
            });
        }
        context_menu_popup(&response, product.name.clone(), |ui| {
            if ContextMenuAction::new(tr!("products-edit-product")).show(ui).clicked() {
                editor.charge_product_dialog = Some(ChargeProductDialog {
                    original: Some(product.name.clone()),
                    product: product.clone(),
                });
                ui.close();
            }
            if ContextMenuAction::new(tr!("common-delete-product")).show(ui).clicked() {
                editor.pending_delete_blast_item = Some(BlastLibraryItem::Product(product.name.clone()));
                ui.close();
            }
        });
    }
}
