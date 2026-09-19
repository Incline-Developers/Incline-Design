//! Docked panel on the window's right edge, showing everything known about
//! the currently inspected hole.
//!
//! Has a Data tab (property grid) and a Log tab (strip log); the Log tab's
//! widget lives in [`crate::ui::widgets::viewport::BoreholeLog`].

use crate::{
    i18n::tr,
    model::{SceneEntityId, drill_hole::OpenDrillHoleDataset},
    ui::{
        EditorState,
        state::{BoreholeInspectorTab, UiCommand, ViewToggle},
        themed_icon, unthemed_icon,
        widgets::viewport::DrillHoleProperties,
    },
};

/// Id of the borehole inspector panel; `crate::ui::chrome` reads its resize
/// response to light up the grip.
pub(crate) const PANEL_ID: &str = "borehole_inspector_panel";

/// Default width: room for a two-column property grid.
const DEFAULT_WIDTH: f32 = 300.0;
/// Narrowest width before rows would rather truncate than shrink further.
const MIN_WIDTH: f32 = 200.0;
/// Widest, so the Log tab's strip log has room without swallowing the scene.
const MAX_WIDTH: f32 = 520.0;

/// Draw the borehole inspector panel and return what it claimed.
pub(crate) fn draw_borehole_inspector(ui: &mut egui::Ui, editor: &mut EditorState, datasets: &[OpenDrillHoleDataset], commands: &mut Vec<UiCommand>) -> egui::Rect {
    // Reuse the explorer's row colours so the two panels share a palette.
    let (surface, _stripe) = crate::ui::widgets::tree_row_colors(ui);
    egui::Panel::right(PANEL_ID)
        .resizable(true)
        .default_size(DEFAULT_WIDTH)
        .min_size(MIN_WIDTH)
        .max_size(MAX_WIDTH)
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
                    .max_rect(body)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            body_ui.set_clip_rect(body.intersect(ui.clip_rect()));
            draw_body(&mut body_ui, editor, datasets, commands);
        })
        .response
        .rect
}

/// The panel's contents, drawn into the clipped child ui the caller sized.
///
/// The tab strip and dataset name sit outside the Data tab's scroll area,
/// so neither scrolls out of view.
fn draw_body(ui: &mut egui::Ui, editor: &mut EditorState, datasets: &[OpenDrillHoleDataset], commands: &mut Vec<UiCommand>) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.selectable_value(&mut editor.borehole_inspector_tab, BoreholeInspectorTab::Data, tr!(literal = "Data"));
        ui.selectable_value(&mut editor.borehole_inspector_tab, BoreholeInspectorTab::Log, tr!(literal = "Log"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            // Rightmost, where a panel's close belongs; the lock sits inboard.
            // The same switch the View and Geology menus throw, so the stored
            // preference and the macOS check mark stay in step.
            if ui
                .add(egui::Button::image(themed_icon!(ui, "close_project.svg")).frame(false))
                .on_hover_text(tr!(literal = "Close the inspector"))
                .clicked()
            {
                commands.push(UiCommand::ToggleViewOption(ViewToggle::BoreholeInspector));
            }
            let locked = editor.borehole_inspector_locked;
            // unthemed_icon! embeds the image at compile time, so each icon
            // is named per branch rather than passed in as a value.
            let (icon, hint) = if locked {
                (unthemed_icon!("entry_locked.svg"), tr!(literal = "Holding this hole. Click to follow the selection again."))
            } else {
                (unthemed_icon!("entry_unlocked.svg"), tr!(literal = "Hold this hole while you work on the ones around it."))
            };
            if ui.add(egui::Button::image(icon).frame(locked)).on_hover_text(hint).clicked() {
                editor.borehole_inspector_locked = !locked;
            }
        });
    });
    ui.add_space(4.0);

    let Some((dataset, hole, hole_index)) = inspected_hole(editor, datasets) else {
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(tr!(literal = "No hole inspected")).weak());
        });
        return;
    };

    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.add(egui::Label::new(egui::RichText::new(dataset.name.clone()).strong().color(ui.visuals().weak_text_color())).truncate());
    });
    ui.add_space(4.0);

    match editor.borehole_inspector_tab {
        BoreholeInspectorTab::Data => {
            // The interval table scrolls to fit; the Log tab must not, since
            // its wheel zooms rather than scrolling an enclosing area.
            let list_height = (ui.available_height() - 8.0).max(120.0);
            ui.push_id((dataset.id, hole_index), |ui| {
                // Columns come from the dataset, so every hole shows the same.
                DrillHoleProperties::new(("borehole_inspector", dataset.id, hole_index), hole, &dataset.dataset.fields)
                    .max_list_height(list_height)
                    .show(ui);
            });
        }
        BoreholeInspectorTab::Log => {
            draw_log_field_pickers(ui, editor, dataset, commands);
            crate::ui::widgets::viewport::BoreholeLog::new(("borehole_log", dataset.id), hole, dataset)
                .strat_field(strat_choice_for(editor, dataset))
                .show(ui);
        }
    }
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

    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(tr!(literal = "Colour"));
        let uniform = tr!(literal = "Uniform white");
        let mut chosen = dataset.color.active_field.clone();
        let before = chosen.clone();
        egui::ComboBox::from_id_salt(("borehole_log_color_field", dataset.id))
            .selected_text(label_of(chosen.as_deref(), &uniform))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut chosen, None, uniform.clone());
                for field in fields {
                    ui.selectable_value(&mut chosen, Some(field.key.clone()), field.label.clone());
                }
            });
        if chosen != before {
            commands.push(UiCommand::SetDrillHoleColorField { id: dataset.id, field: chosen });
        }
    });
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(tr!(literal = "Strat"));
        let guessed = tr!(literal = "Guessed by name");
        egui::ComboBox::from_id_salt(("borehole_log_strat_field", dataset.id))
            .selected_text(label_of(strat_choice_for(editor, dataset).as_deref(), &guessed))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut editor.borehole_log_strat_field, None, guessed.clone());
                for field in fields
                    .iter()
                    .filter(|field| matches!(field.kind, crate::model::drill_hole::DrillFieldKind::Categorical { .. }))
                {
                    ui.selectable_value(&mut editor.borehole_log_strat_field, Some((dataset.id, field.key.clone())), field.label.clone());
                }
            });
    });
    ui.add_space(4.0);
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
