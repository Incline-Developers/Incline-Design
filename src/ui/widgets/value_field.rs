//! The input/constant field: a value the user types, or - from the small
//! button at its right edge - the name of a constant in the scenario's
//! constants grid standing in for it.
//!
//! The field is typed (number, text or yes/no) and only offers constants of
//! its own type. A number field refuses to take anything but digits, a point
//! and, where negatives make sense, a leading minus. Choosing a constant locks
//! the field to the constant's name and swaps the button for a cross that
//! removes it again, returning the value that was typed before.
//!
//! The constants live in the scenario being edited; the editor window
//! publishes them with [`publish_constants`] each frame, so a field needs
//! nothing but its own value. The picker is one floating, movable, modal
//! window drawn by [`draw_constant_picker`] after everything else; a field asks
//! for it and collects the answer a frame later through egui's memory.

use std::{fmt::Debug, hash::Hash, sync::Arc};

use super::hint::Hint;
use crate::{
    i18n::tr,
    model::optimization::{Constant, ConstantValue, ConstantsRow, FieldValue, ValueKind, ValueType},
    ui::{
        themed_icon,
        widgets::{
            editable_grid::group_colour,
            menu::{self, DragableMenu, MenuButton},
            toolbar::ToolbarButton,
        },
    },
};

/// Side of the pick/clear button at the field's right edge.
const BUTTON_SIDE: f32 = crate::ui::widgets::toolbar::TOOL_CELL_SIZE;
const PICKER_WIDTH: f32 = 520.0;
const PICKER_LIST_HEIGHT: f32 = 260.0;

fn constants_key() -> egui::Id {
    egui::Id::unique("opt_value_field_constants")
}

fn request_key() -> egui::Id {
    egui::Id::unique("opt_value_field_picker_request")
}

fn choice_key() -> egui::Id {
    egui::Id::unique("opt_value_field_picker_choice")
}

#[derive(Clone)]
struct PickerRequest {
    field: egui::Id,
    value_type: ValueType,
}

#[derive(Clone)]
struct PickerChoice {
    field: egui::Id,
    name: String,
}

/// Make the scenario's constants available to the fields drawn this frame.
pub(crate) fn publish_constants(ctx: &egui::Context, rows: &[ConstantsRow]) {
    ctx.data_mut(|data| data.insert_temp(constants_key(), Arc::new(rows.to_vec())));
}

fn published_rows(ctx: &egui::Context) -> Arc<Vec<ConstantsRow>> {
    ctx.data(|data| data.get_temp::<Arc<Vec<ConstantsRow>>>(constants_key())).unwrap_or_default()
}

fn published_constants(ctx: &egui::Context) -> Vec<Constant> {
    published_rows(ctx).iter().filter_map(ConstantsRow::constant).cloned().collect()
}

fn take_choice(ctx: &egui::Context, field: egui::Id) -> Option<String> {
    ctx.data_mut(|data| {
        let choice = data.get_temp::<PickerChoice>(choice_key())?;
        (choice.field == field).then(|| {
            data.remove::<PickerChoice>(choice_key());
            choice.name
        })
    })
}

// ── Editing a literal ──

/// A value type's editor: what the field draws while no constant is chosen.
pub(crate) trait FieldEditor: ValueKind {
    /// Draw the editor in `width`, and report whether the value changed.
    fn edit(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, range: (f64, f64)) -> egui::Response {
        self.edit_with_suffix(ui, id, width, range, "")
    }

    /// [`Self::edit`], with a unit after the value while it is not being
    /// typed in, as `egui::DragValue::suffix` shows one. Only numbers have one.
    fn edit_with_suffix(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, range: (f64, f64), suffix: &str) -> egui::Response;

    /// [`Self::edit_with_suffix`], keeping `inset` points free at the box's
    /// left inside its frame (for a switch drawn there). Only numbers keep it.
    fn edit_inset(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, range: (f64, f64), suffix: &str, inset: f32) -> egui::Response {
        let _ = inset;
        self.edit_with_suffix(ui, id, width, range, suffix)
    }
}

impl FieldEditor for f64 {
    fn edit_with_suffix(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, range: (f64, f64), suffix: &str) -> egui::Response {
        self.edit_inset(ui, id, width, range, suffix, 0.0)
    }

    fn edit_inset(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, range: (f64, f64), suffix: &str, inset: f32) -> egui::Response {
        let (mut response, parsed) = edit_number(ui, id, self.to_string(), width, range, suffix, inset);
        if let Some(parsed) = parsed {
            *self = parsed;
            response.mark_changed();
        }
        response
    }
}

impl FieldEditor for f32 {
    fn edit_with_suffix(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, range: (f64, f64), suffix: &str) -> egui::Response {
        self.edit_inset(ui, id, width, range, suffix, 0.0)
    }

    fn edit_inset(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, range: (f64, f64), suffix: &str, inset: f32) -> egui::Response {
        let (mut response, parsed) = edit_number(ui, id, self.to_string(), width, range, suffix, inset);
        if let Some(parsed) = parsed {
            *self = parsed as f32;
            response.mark_changed();
        }
        response
    }
}

impl FieldEditor for String {
    fn edit_with_suffix(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, _range: (f64, f64), _suffix: &str) -> egui::Response {
        ui.add(egui::TextEdit::singleline(self).id(id).desired_width(width))
    }
}

impl FieldEditor for bool {
    fn edit_with_suffix(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, _range: (f64, f64), _suffix: &str) -> egui::Response {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, ui.spacing().interact_size.y), egui::Sense::hover());
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect).id_salt(id), |ui| ui.checkbox(self, "")).inner
    }
}

/// A text box for a whole number of at least `range.0`, that takes digits only.
pub(crate) fn edit_count(ui: &mut egui::Ui, id: egui::Id, value: &mut u32, width: f32, range: (u32, u32)) -> egui::Response {
    let (mut response, parsed) = edit_number(ui, id, value.to_string(), width, (f64::from(range.0), f64::from(range.1)), "", 0.0);
    if let Some(parsed) = parsed.filter(|parsed| parsed.fract() == 0.0) {
        *value = parsed as u32;
        response.mark_changed();
    }
    response
}

/// Keep the characters a number can be written with: digits, one point, and a
/// minus in front when negatives are allowed.
pub(crate) fn filter_number(text: &str, allow_negative: bool) -> String {
    let mut seen_point = false;
    text.chars()
        .enumerate()
        .filter(|(index, character)| match character {
            '0'..='9' => true,
            '.' if !seen_point => {
                seen_point = true;
                true
            }
            '-' => allow_negative && *index == 0,
            _ => false,
        })
        .map(|(_, character)| character)
        .collect()
}

/// A text box that cannot hold anything but a number.
///
/// What is typed is kept apart from the value while the box has focus, so a
/// half-written number ("-", "1.") does not fight the value; the value follows
/// as soon as the text parses, and is clamped into `range` when focus leaves.
/// At rest the box shows `suffix` after the number; typing drops it.
fn edit_number(ui: &mut egui::Ui, id: egui::Id, shown: String, width: f32, range: (f64, f64), suffix: &str, inset: f32) -> (egui::Response, Option<f64>) {
    let buffer_id = id.with("typed");
    let focused = ui.memory(|memory| memory.has_focus(id));
    let mut text = if focused {
        ui.data_mut(|data| data.get_temp::<String>(buffer_id)).unwrap_or(shown)
    } else {
        // Text typed into a box whose window closed before the focus left
        // would otherwise come back the next time the box is focused.
        ui.data_mut(|data| data.remove::<String>(buffer_id));
        format!("{shown}{suffix}")
    };
    let mut edit = egui::TextEdit::singleline(&mut text).id(id).desired_width(width).align(egui::Align2::RIGHT_TOP);
    if inset > 0.0 {
        edit = edit.margin(inset_margin(inset));
    }
    let response = ui.add(edit);
    // A box at rest holds its suffix, which is not something typed in.
    let typing = response.has_focus() || response.changed();
    let filtered = filter_number(&text, range.0 < 0.0);
    let mut parsed = None;
    if typing && (response.changed() || filtered != text) {
        text = filtered;
        parsed = text.parse::<f64>().ok().filter(|number| number.is_finite());
        ui.data_mut(|data| data.insert_temp(buffer_id, text.clone()));
    }
    if response.lost_focus() {
        if let Some(number) = text.parse::<f64>().ok().filter(|number| number.is_finite()) {
            let clamped = number.clamp(range.0, range.1);
            if clamped != number {
                parsed = Some(clamped);
            }
        }
        ui.data_mut(|data| data.remove::<String>(buffer_id));
    }
    (response, parsed)
}

// ── The field ──

pub(crate) struct ValueField<'a, T: FieldEditor> {
    id: egui::Id,
    value: &'a mut FieldValue<T>,
    range: (f64, f64),
    suffix: String,
    /// A switch inside the box at its left: off greys the field out.
    switch: Option<&'a mut bool>,
}

/// Width of the switch inside a field's box, and its gap from the box's edge.
const INNER_SWITCH: egui::Vec2 = egui::vec2(28.0, 14.0);
const INNER_SWITCH_GAP: f32 = 4.0;

/// A box's text margin with `inset` points kept free at the left.
fn inset_margin(inset: f32) -> egui::Margin {
    egui::Margin {
        left: inset.round().clamp(0.0, 127.0) as i8,
        right: 4,
        top: 2,
        bottom: 2,
    }
}

impl<'a, T: FieldEditor> ValueField<'a, T> {
    pub(crate) fn new(id_salt: impl Hash + Debug, value: &'a mut FieldValue<T>) -> Self {
        Self {
            id: egui::Id::unique(("opt_value_field", id_salt)),
            value,
            range: (f64::MIN, f64::MAX),
            suffix: String::new(),
            switch: None,
        }
    }

    /// A switch inside the box, pressed against its left edge: on, the field
    /// takes a value; off, it is greyed (its value kept) - an optional limit,
    /// say. The field's label names what is switched, so the switch has none.
    pub(crate) fn switch(mut self, on: &'a mut bool) -> Self {
        self.switch = Some(on);
        self
    }

    /// The unit shown after a typed number, as `egui::DragValue::suffix`
    /// shows one ("°", " t/m³"). Not shown on a constant, which names itself.
    pub(crate) fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }

    /// The values a number field accepts. A lower bound at or above zero also
    /// stops the field taking a minus sign.
    pub(crate) fn range(mut self, min: f64, max: f64) -> Self {
        self.range = (min, max);
        self
    }

    /// Draw the field across the width available (a grid cell, or the control
    /// column of a form row); the response is changed when its value or
    /// constant is.
    pub(crate) fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let width = ui.available_width();
        self.control(ui, width)
    }

    fn control(self, ui: &mut egui::Ui, width: f32) -> egui::Response {
        let Self { id, value, range, suffix, switch } = self;
        let enabled = switch.as_deref().copied().unwrap_or(true);
        let inset = if switch.is_some() { INNER_SWITCH.x + 2.0 * INNER_SWITCH_GAP } else { 0.0 };
        let ctx = ui.ctx().clone();
        let mut chosen = false;
        if let Some(name) = take_choice(&ctx, id) {
            value.constant = Some(name);
            chosen = true;
        }
        let constants = published_constants(&ctx);
        let editor_width = (width - BUTTON_SIDE - ui.spacing().item_spacing.x).max(40.0);
        let mut cleared = false;
        let mut requested = false;
        let mut response = ui
            .add_enabled_ui(enabled, |ui| {
                ui.horizontal(|ui| {
                    let response = match &value.constant {
                        Some(name) => {
                            let found = constants.iter().find(|constant| constant.name == *name);
                            let fitting = found.and_then(|constant| T::from_constant(&constant.value));
                            let mut shown = name.clone();
                            let mut locked = egui::TextEdit::singleline(&mut shown).desired_width(editor_width);
                            if inset > 0.0 {
                                locked = locked.margin(inset_margin(inset));
                            }
                            let response = ui.add_enabled(false, locked);
                            let message = match (found, fitting) {
                                (None, _) => Some(tr!("opt-constant-missing", name = name.clone())),
                                (Some(_), None) => Some(tr!("opt-constant-wrong-type", name = name.clone())),
                                (Some(constant), Some(_)) => return_value_hint(&response, &constant.value),
                            };
                            finish_locked(response, message)
                        }
                        None => value.value.edit_inset(ui, id, editor_width, range, &suffix, inset),
                    };
                    if value.constant.is_some() {
                        if ui
                            .add(
                                ToolbarButton::new(egui::Image::new(themed_icon!(ui, "constant_clear.svg")), tr!("opt-clear-constant"))
                                    .id_salt(id.with("clear"))
                                    .button_side(BUTTON_SIDE),
                            )
                            .clicked()
                        {
                            cleared = true;
                        }
                    } else if ui
                        .add(
                            ToolbarButton::new(egui::Image::new(themed_icon!(ui, "constant_pick_tool.svg")), tr!("opt-pick-constant"))
                                .id_salt(id.with("pick"))
                                .button_side(BUTTON_SIDE),
                        )
                        .clicked()
                    {
                        requested = true;
                    }
                    response
                })
                .inner
            })
            .inner;
        // The switch, in the box's left corner: on top of the box, and usable
        // while the field is greyed.
        if let Some(on) = switch {
            let rect = egui::Rect::from_min_size(
                egui::pos2(response.rect.left() + INNER_SWITCH_GAP, response.rect.center().y - INNER_SWITCH.y / 2.0),
                INNER_SWITCH,
            );
            let switch = ui.put(rect, crate::ui::widgets::menu::SlidingToggle::new(on).size(INNER_SWITCH));
            // The box under it asks for the text cursor; the switch is not text.
            if switch.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
            }
            if switch.changed() {
                response.mark_changed();
            }
        }
        if cleared {
            value.constant = None;
        }
        if requested {
            ctx.data_mut(|data| data.insert_temp(request_key(), PickerRequest { field: id, value_type: T::TYPE }));
            ctx.request_repaint();
        }
        if cleared || chosen {
            response.mark_changed();
        }
        response
    }
}

/// A locked field with a usable constant has no problem to report; it says what
/// the constant's value is on hover instead.
fn return_value_hint(response: &egui::Response, value: &ConstantValue) -> Option<String> {
    let _ = response.clone().disabled_hint(value.display());
    None
}

/// Outline a locked field in red with the reason, when its constant cannot be used.
fn finish_locked(response: egui::Response, problem: Option<String>) -> egui::Response {
    match problem {
        Some(message) => {
            response
                .ctx
                .layer_painter(response.layer_id)
                .rect_stroke(response.rect, 2.0, egui::Stroke::new(1.5, egui::Color32::RED), egui::StrokeKind::Outside);
            response.disabled_hint(message)
        }
        None => response,
    }
}

/// Outline `response`'s widget in red: a required value is missing or wrong.
pub(crate) fn mark_invalid(ui: &egui::Ui, response: &egui::Response) {
    ui.painter()
        .rect_stroke(response.rect, 2.0, egui::Stroke::new(1.5, egui::Color32::RED), egui::StrokeKind::Outside);
}

// ── The picker ──

/// Draw the constant picker if a field has asked for one. Drawn last, so it is
/// above the editor; an invisible sheet under it keeps the editor from taking
/// clicks while it is up.
pub(crate) fn draw_constant_picker(ctx: &egui::Context) {
    let Some(request) = ctx.data(|data| data.get_temp::<PickerRequest>(request_key())) else {
        return;
    };
    let rows = published_rows(ctx);
    let candidates: Vec<&Constant> = rows
        .iter()
        .filter_map(ConstantsRow::constant)
        .filter(|constant| constant.value.value_type() == request.value_type)
        .collect();
    let collapsed_id = egui::Id::unique("opt_constant_picker_collapsed");
    // Every group starts open; the ones folded here are remembered by their
    // place in the list until the picker closes.
    let mut collapsed: Vec<usize> = ctx.data(|data| data.get_temp(collapsed_id)).unwrap_or_default();

    menu::modal_sheet(ctx, "opt_constant_picker_sheet");

    let selected_id = egui::Id::unique("opt_constant_picker_selected");
    let mut selected: Option<String> = ctx.data(|data| data.get_temp(selected_id));
    let mut open = true;
    let mut cancel = false;
    let mut choose: Option<String> = None;
    let dialog = DragableMenu::new("opt_constant_picker", tr!("opt-pick-constant-title"))
        .open(&mut open)
        .min_width(PICKER_WIDTH)
        .max_width(PICKER_WIDTH)
        .show(ctx, |ui| {
            if candidates.is_empty() {
                menu::menu_note(ui, tr!("opt-no-constants-of-type"));
            } else {
                egui::ScrollArea::vertical().max_height(PICKER_LIST_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let widths = picker_columns(ui.available_width());
                    picker_row(ui, &widths, [tr!("opt-col-name"), tr!("opt-col-value"), tr!("opt-col-description")]);
                    let mut group_ordinal = 0;
                    let mut group: Option<(usize, egui::Color32)> = None;
                    for (index, row) in rows.iter().enumerate() {
                        match row {
                            ConstantsRow::Group { name, .. } => {
                                let colour = group_colour(group_ordinal, ui.visuals().dark_mode);
                                group_ordinal += 1;
                                group = Some((index, colour));
                                // A group none of whose constants fit is left out.
                                let has_candidates = rows[index + 1..]
                                    .iter()
                                    .take_while(|row| row.constant().is_some())
                                    .filter_map(ConstantsRow::constant)
                                    .any(|constant| constant.value.value_type() == request.value_type);
                                if !has_candidates {
                                    group = Some((usize::MAX, colour));
                                    continue;
                                }
                                let folded = collapsed.contains(&index);
                                let response = picker_group_row(ui, name, folded, colour);
                                if response.clicked() {
                                    if folded {
                                        collapsed.retain(|other| *other != index);
                                    } else {
                                        collapsed.push(index);
                                    }
                                }
                            }
                            ConstantsRow::Constant(constant) => {
                                if constant.value.value_type() != request.value_type {
                                    continue;
                                }
                                let tint = match group {
                                    Some((owner, colour)) => {
                                        if collapsed.contains(&owner) {
                                            continue;
                                        }
                                        Some(colour)
                                    }
                                    None => None,
                                };
                                let is_selected = selected.as_deref() == Some(constant.name.as_str());
                                let response = picker_constant_row(ui, &widths, constant, is_selected, tint);
                                if response.clicked() {
                                    selected = Some(constant.name.clone());
                                }
                                if response.double_clicked() {
                                    choose = Some(constant.name.clone());
                                }
                            }
                        }
                    }
                });
            }
            let ready = selected.as_ref().is_some_and(|name| candidates.iter().any(|constant| constant.name == *name));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("opt-select")).primary().enabled(ready)).clicked() {
                    choose = selected.clone();
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                    cancel = true;
                }
            });
        });
    if let Some(dialog) = dialog {
        ctx.move_to_top(dialog.response.layer_id);
    }

    ctx.data_mut(|data| data.insert_temp(collapsed_id, collapsed));
    ctx.data_mut(|data| match &selected {
        Some(name) => {
            data.insert_temp(selected_id, name.clone());
        }
        None => data.remove::<String>(selected_id),
    });
    if let Some(name) = choose {
        ctx.data_mut(|data| data.insert_temp(choice_key(), PickerChoice { field: request.field, name }));
        close_picker(ctx);
    } else if cancel || !open {
        close_picker(ctx);
    }
}

/// Close the picker, if it is up, without choosing anything.
pub(crate) fn close_constant_picker(ctx: &egui::Context) {
    if picker_open(ctx) {
        close_picker(ctx);
    }
}

fn close_picker(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        data.remove::<PickerRequest>(request_key());
        data.remove::<String>(egui::Id::unique("opt_constant_picker_selected"));
        data.remove::<Vec<usize>>(egui::Id::unique("opt_constant_picker_collapsed"));
    });
    ctx.request_repaint();
}

/// Whether the picker is up, which the editor uses to hold back its own Escape.
pub(crate) fn picker_open(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<PickerRequest>(request_key())).is_some()
}

// ── The picker's list ──

const PICKER_ROW_HEIGHT: f32 = 24.0;
const PICKER_GUTTER: f32 = 14.0;

/// Widths of the name, value and description columns.
fn picker_columns(available: f32) -> [f32; 3] {
    let usable = (available - PICKER_GUTTER - 12.0).max(120.0);
    [usable * 0.34, usable * 0.2, usable * 0.46]
}

/// The list's heading row.
fn picker_row(ui: &mut egui::Ui, widths: &[f32; 3], cells: [String; 3]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), PICKER_ROW_HEIGHT), egui::Sense::hover());
    paint_cells(ui, rect, widths, cells, ui.visuals().weak_text_color());
}

fn paint_cells(ui: &egui::Ui, rect: egui::Rect, widths: &[f32; 3], cells: [String; 3], colour: egui::Color32) {
    let mut x = rect.left() + PICKER_GUTTER;
    for (cell, width) in cells.into_iter().zip(widths) {
        let clip = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(*width, rect.height()));
        ui.painter().with_clip_rect(clip.intersect(ui.clip_rect())).text(
            clip.left_center() + egui::vec2(4.0, 0.0),
            egui::Align2::LEFT_CENTER,
            cell,
            egui::TextStyle::Body.resolve(ui.style()),
            colour,
        );
        x += width;
    }
}

/// A group's heading: a small triangle and the name, clickable anywhere along
/// the row to fold or unfold the group.
fn picker_group_row(ui: &mut egui::Ui, name: &str, folded: bool, colour: egui::Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), PICKER_ROW_HEIGHT), egui::Sense::click());
    let fill = colour.gamma_multiply(if response.hovered() { 0.6 } else { 0.5 });
    ui.painter().rect_filled(rect, 2.0, fill);
    let arrow = if folded { "▶" } else { "▼" };
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        arrow,
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().text_color(),
    );
    ui.painter().text(
        egui::pos2(rect.left() + PICKER_GUTTER + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::TextStyle::Button.resolve(ui.style()),
        ui.visuals().text_color(),
    );
    response
}

/// One constant: the whole row lights up under the pointer and takes the
/// click, wherever in it that lands.
fn picker_constant_row(ui: &mut egui::Ui, widths: &[f32; 3], constant: &Constant, selected: bool, group: Option<egui::Color32>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), PICKER_ROW_HEIGHT), egui::Sense::click());
    let fill = if selected {
        Some(menu::accent_fill(ui.visuals()))
    } else if response.hovered() {
        Some(ui.visuals().widgets.hovered.weak_bg_fill)
    } else {
        group.map(|colour| colour.gamma_multiply(0.14))
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, 2.0, fill);
    }
    if let Some(colour) = group {
        let bar = egui::Rect::from_min_max(egui::pos2(rect.left() + 5.0, rect.top()), egui::pos2(rect.left() + 9.0, rect.bottom()));
        ui.painter().rect_filled(bar, 1.0, colour);
    }
    // White on the accent, as on a confirm button.
    let text = if selected { egui::Color32::WHITE } else { ui.visuals().text_color() };
    paint_cells(ui, rect, widths, [constant.name.clone(), constant.value.display(), constant.description.clone()], text);
    response
}
