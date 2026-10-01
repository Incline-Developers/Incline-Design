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
//! stands on a delay, and the products those rules stack into a column. The
//! shot summary at the foot reads the active pattern back.

use crate::{
    i18n::{tr, tr_format},
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

/// How far the shot summary's figures sit in from the panel edge: level with
/// the row labels above them.
const SUMMARY_INDENT: f32 = 12.0;

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
                    let (toggle, header, _) = ExplorerHeader::new(egui::Id::new("delay_palette_collapse"), tr!(literal = "Delay Palette"))
                        .icon(unthemed_icon!("tie_holes.svg"))
                        .color(HEADER_DELAY_PALETTE)
                        .show(ui, |ui| draw_delay_palette(ui, editor));
                    // Adding to the palette is editing the palette rather than
                    // one product in it, so it hangs off the section heading -
                    // the way the explorer's own section menus do - instead of
                    // taking a permanent row at the foot of the list.
                    context_menu_popup(&toggle.union(header.inner), tr!(literal = "Delay Palette"), |ui| {
                        if ContextMenuAction::new(tr!(literal = "New Product")).show(ui).clicked() {
                            editor.begin_new_delay_product();
                            ui.close();
                        }
                    });

                    let (toggle, header, _) = ExplorerHeader::new(egui::Id::new("charge_rules_collapse"), tr!(literal = "Charge Rules"))
                        .icon(unthemed_icon!("charge_holes.svg"))
                        .color(HEADER_CHARGE)
                        .show(ui, |ui| draw_charge_rules(ui, editor, commands));
                    context_menu_popup(&toggle.union(header.inner), tr!(literal = "Charge Rules"), |ui| {
                        if ContextMenuAction::new(tr!(literal = "New Rule")).show(ui).clicked() {
                            editor.charge_rule_dialog = Some(ChargeRuleDialog {
                                original: None,
                                rule: new_rule(&editor.blast_library),
                            });
                            ui.close();
                        }
                    });

                    let (toggle, header, _) = ExplorerHeader::new(egui::Id::new("charge_products_collapse"), tr!(literal = "Charge Products"))
                        .color(HEADER_CHARGE)
                        .default_open(false)
                        .show(ui, |ui| draw_charge_products(ui, editor));
                    context_menu_popup(&toggle.union(header.inner), tr!(literal = "Charge Products"), |ui| {
                        if ContextMenuAction::new(tr!(literal = "New Product")).show(ui).clicked() {
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

                    ExplorerHeader::new(egui::Id::new("shot_summary_collapse"), tr!(literal = "Shot"))
                        .color(HEADER_DELAY_PALETTE)
                        .show(ui, |ui| draw_shot_summary(ui, editor));
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
        explorer_note(ui, tr!(literal = "No products"));
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
            if ContextMenuAction::new(tr!(literal = "Delete Product")).show(ui).clicked() {
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
    let base = tr!(literal = "New rule");
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
        explorer_note(ui, tr!(literal = "No rules"));
    }
    let active = editor.active_rule().map(|rule| rule.name.clone());
    let selected_holes = editor
        .active_drill_hole
        .map_or(0, |id| editor.selected_drill_holes.iter().filter(|hole| hole.dataset == id).count());
    let mut choose = None;
    for rule in &editor.blast_library.rules {
        let chosen = active.as_deref() == Some(rule.name.as_str());
        let problem = rule.problem(&editor.blast_library.products);
        let mut entry = ExplorerEntry::new(
            egui::Id::new(("charge_rule", rule.name.as_str())),
            rule_label(ui, rule, &editor.blast_library, problem.is_some()),
        )
        .selected(chosen);
        if chosen {
            entry = entry.leading_icon(unthemed_icon!("product_active.svg"), HEADER_CHARGE);
        }
        let mut response = entry.show(ui).response;
        if let Some(problem) = &problem {
            response = response.on_hover_text(problem);
        } else {
            response = response.on_hover_text(rule_description(rule));
        }
        if response.clicked() {
            choose = Some(rule.name.clone());
        }
        if response.double_clicked() {
            editor.charge_rule_dialog = Some(ChargeRuleDialog {
                original: Some(rule.name.clone()),
                rule: rule.clone(),
            });
        }
        context_menu_popup(&response, rule.name.clone(), |ui| {
            let can_load = selected_holes > 0 && problem.is_none();
            if ContextMenuAction::new(tr_format!(literal = "Load Selected Holes (%count%)", count = selected_holes))
                .enabled(can_load)
                .show(ui)
                .clicked()
            {
                commands.push(UiCommand::ChargeSelectedHoles { rule: Some(rule.name.clone()) });
                ui.close();
            }
            if ContextMenuAction::new(tr_format!(literal = "Unload Selected Holes (%count%)", count = selected_holes))
                .enabled(selected_holes > 0)
                .show(ui)
                .clicked()
            {
                commands.push(UiCommand::ChargeSelectedHoles { rule: None });
                ui.close();
            }
            if ContextMenuAction::new(tr!(literal = "Edit Rule")).show(ui).clicked() {
                editor.charge_rule_dialog = Some(ChargeRuleDialog {
                    original: Some(rule.name.clone()),
                    rule: rule.clone(),
                });
                ui.close();
            }
            if ContextMenuAction::new(tr!(literal = "Duplicate Rule")).show(ui).clicked() {
                let mut copy = rule.clone();
                copy.name = tr_format!(literal = "%name% copy", name = &rule.name);
                editor.charge_rule_dialog = Some(ChargeRuleDialog { original: None, rule: copy });
                ui.close();
            }
            if ContextMenuAction::new(tr!(literal = "Delete Rule")).show(ui).clicked() {
                editor.pending_delete_blast_item = Some(BlastLibraryItem::Rule(rule.name.clone()));
                ui.close();
            }
        });
    }
    if let Some(name) = choose {
        editor.active_charge_rule = Some(name);
    }
}

/// A rule as a row: its name, then the column it makes in the panel's
/// secondary weight - a band of each deck's colour, so rules read apart at a
/// glance - or a warning when it cannot load anything.
fn rule_label(ui: &egui::Ui, rule: &ChargeRule, library: &BlastLibrary, broken: bool) -> egui::WidgetText {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &rule.name,
        0.0,
        egui::TextFormat {
            font_id: font.clone(),
            color: ui.visuals().text_color(),
            ..Default::default()
        },
    );
    if broken {
        job.append(
            "⚠",
            LABEL_GAP,
            egui::TextFormat {
                font_id: font,
                color: ui.visuals().warn_fg_color,
                ..Default::default()
            },
        );
        return job.into();
    }
    for (index, deck) in rule.decks.iter().enumerate() {
        let color = library
            .products
            .iter()
            .find(|product| product.name == deck.product)
            .map_or(ui.visuals().weak_text_color(), |product| {
                egui::Color32::from_rgb(product.color[0], product.color[1], product.color[2])
            });
        job.append(
            "▮",
            if index == 0 { LABEL_GAP } else { 0.0 },
            egui::TextFormat {
                font_id: font.clone(),
                color,
                ..Default::default()
            },
        );
    }
    job.into()
}

/// A rule spelled out deck by deck, for its row's hover text.
fn rule_description(rule: &ChargeRule) -> String {
    let mut lines: Vec<String> = rule
        .decks
        .iter()
        .map(|deck| match deck.length {
            DeckLength::Fixed(length) => format!("{length:.2} m  {}", deck.product),
            DeckLength::Fill => tr_format!(literal = "fill  %product%", product = &deck.product),
        })
        .collect();
    lines.push(tr_format!(
        literal = "Primer %offset% m off each explosive deck's base, %booster% kg booster, %delay% ms downhole",
        offset = format!("{:.2}", rule.primer_offset),
        booster = format!("{:.2}", rule.booster_kg),
        delay = rule.downhole_delay_ms
    ));
    lines.push(tr!(literal = "Double-click to edit"));
    lines.join("\n")
}

/// The charge products: one row each with a swatch of the colour its decks
/// are drawn in.
fn draw_charge_products(ui: &mut egui::Ui, editor: &mut EditorState) {
    if editor.blast_library.products.is_empty() {
        explorer_note(ui, tr!(literal = "No products"));
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
        let detail = match product.kind {
            crate::model::blast::DeckKind::Explosive => format!("{:.2} g/cm³", product.density),
            kind => kind.label(),
        };
        job.append(
            &detail,
            LABEL_GAP,
            egui::TextFormat {
                font_id: font,
                color: ui.visuals().weak_text_color(),
                ..Default::default()
            },
        );
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
            if ContextMenuAction::new(tr!(literal = "Edit Product")).show(ui).clicked() {
                editor.charge_product_dialog = Some(ChargeProductDialog {
                    original: Some(product.name.clone()),
                    product: product.clone(),
                });
                ui.close();
            }
            if ContextMenuAction::new(tr!(literal = "Delete Product")).show(ui).clicked() {
                editor.pending_delete_blast_item = Some(BlastLibraryItem::Product(product.name.clone()));
                ui.close();
            }
        });
    }
}

/// What the active pattern adds up to: how it is tied, what it is loaded
/// with, and how hard it hits in its busiest 8 ms.
fn draw_shot_summary(ui: &mut egui::Ui, editor: &EditorState) {
    let Some(analysis) = editor.blast_analysis.as_deref() else {
        explorer_note(ui, tr!(literal = "Pick a pattern in the viewport bar"));
        return;
    };
    let round = &editor.blast_round;
    let holes = analysis.times.len();
    let number = |value: f64, decimals: usize| {
        use thousands::Separable;
        format!("{value:.decimals$}").separate_with_commas()
    };
    let mut rows: Vec<(String, String, Option<egui::Color32>)> = vec![
        (
            tr!(literal = "Holes"),
            tr_format!(literal = "%holes% · %loaded% loaded", holes = holes, loaded = analysis.charged_holes),
            None,
        ),
        (tr!(literal = "Connectors"), round.connectors.to_string(), None),
    ];
    if round.initiations.is_empty() {
        rows.push((tr!(literal = "Initiation"), tr!(literal = "None set"), Some(ui.visuals().warn_fg_color)));
    }
    if round.unreached > 0 {
        rows.push((
            tr!(literal = "Unreached"),
            tr_format!(literal = "%count% hole(s)", count = round.unreached),
            Some(ui.visuals().warn_fg_color),
        ));
    }
    if let Some(duration) = analysis.duration_ms {
        rows.push((tr!(literal = "Duration"), format!("{} ms", number(duration, 0)), None));
        rows.push((
            tr!(literal = "Peak holes / 8 ms"),
            tr_format!(
                literal = "%count% at %time% ms",
                count = analysis.peak_holes.value,
                time = number(analysis.peak_holes.start_ms, 0)
            ),
            None,
        ));
    }
    if analysis.total_mass_kg > 0.0 {
        rows.push((tr!(literal = "Explosive"), format!("{} kg", number(analysis.total_mass_kg, 0)), None));
        if analysis.duration_ms.is_some() {
            rows.push((
                tr!(literal = "MIC (8 ms)"),
                tr_format!(
                    literal = "%mass% kg at %time% ms",
                    mass = number(analysis.peak_mass.value, 0),
                    time = number(analysis.peak_mass.start_ms, 0)
                ),
                None,
            ));
        }
        if let Some(powder_factor) = analysis.powder_factor() {
            rows.push((tr!(literal = "Powder factor"), format!("{powder_factor:.2} kg/m³"), None));
        }
    }
    if analysis.duration_ms.is_some() {
        let [tight, good, slack] = analysis.band_counts(editor.blast_review.limits);
        rows.push((
            tr!(literal = "Relief"),
            tr_format!(literal = "%tight% tight · %good% good · %slack% slack", tight = tight, good = good, slack = slack),
            (tight > 0).then_some(egui::Color32::from_rgb(0xDE, 0x33, 0x38)),
        ));
    }
    let weak = ui.visuals().weak_text_color();
    let strong = ui.visuals().text_color();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(SUMMARY_INDENT);
        egui::Grid::new("shot_summary_grid").num_columns(2).spacing([10.0, 3.0]).show(ui, |ui| {
            for (label, value, color) in rows {
                ui.label(egui::RichText::new(label).color(weak));
                ui.label(egui::RichText::new(value).color(color.unwrap_or(strong)));
                ui.end_row();
            }
        });
    });
    ui.add_space(6.0);
}
