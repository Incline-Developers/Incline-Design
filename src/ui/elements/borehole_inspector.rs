//! Docked panel on the window's right edge, showing everything known about
//! the currently inspected hole.
//!
//! Has a Data tab (summary and interval table) and a Log tab (strip log);
//! the Log tab's widget lives in [`crate::ui::widgets::viewport::BoreholeLog`].

use crate::{
    i18n::tr,
    model::{
        SceneEntityId,
        drill_hole::{DrillHoleId, OpenDrillHoleDataset, RenameScope},
        geophysics::{HoleView, LinkState},
    },
    ui::{
        EditorState,
        state::{BoreholeInspectorTab, SeamRenameDraft, UiCommand},
        themed_icon, unthemed_icon,
        widgets::{
            collapsible_section::CollapsibleSection,
            menu::{self, MenuButton, MenuFieldCombo},
            viewport::{DrillHoleProperties, SeamRename},
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
            let output = crate::ui::widgets::viewport::BoreholeLog::new(("borehole_log", dataset.id), hole, dataset)
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
        }
    }
}

/// The Data and Log tabs as a pair of held-down buttons, with the lock and
/// close at the far end of the same row.
fn draw_tab_strip(ui: &mut egui::Ui, editor: &mut EditorState) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (tab, label) in [
            (BoreholeInspectorTab::Data, tr!("borehole-inspector-data")),
            (BoreholeInspectorTab::Log, tr!("borehole-inspector-log")),
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

/// The rename dialog's opening state for a seam picked in the log: its
/// reach counted once, here, rather than every frame the dialog is up.
fn seam_rename_draft(dataset: &OpenDrillHoleDataset, hole_index: usize, rename: SeamRename) -> SeamRenameDraft {
    let scope = if rename.every_hole { RenameScope::Set } else { RenameScope::Hole(hole_index) };
    let reached = dataset
        .dataset
        .intervals_named(&rename.field, &rename.name, scope)
        .map(|((hole, _), _, _)| hole)
        .collect::<Vec<_>>();
    let mut holes = reached.clone();
    holes.dedup();
    let place = match scope {
        RenameScope::Set => dataset.name.clone(),
        RenameScope::Hole(index) => dataset.dataset.holes.get(index).map(|hole| hole.dhid.clone()).unwrap_or_default(),
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
