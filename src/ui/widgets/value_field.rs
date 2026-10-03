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

use crate::{
    i18n::tr,
    model::optimization::{Constant, ConstantValue, ConstantsRow, FieldValue, ValueKind, ValueType},
    ui::{
        themed_icon,
        widgets::{
            menu::{self, DragableMenu, MenuButton},
            toolbar::ToolbarButton,
        },
    },
};

/// Side of the pick/clear button at the field's right edge.
const BUTTON_SIDE: f32 = 22.0;
const PICKER_WIDTH: f32 = 520.0;
const PICKER_LIST_HEIGHT: f32 = 260.0;

fn constants_key() -> egui::Id {
    egui::Id::new("opt_value_field_constants")
}

fn request_key() -> egui::Id {
    egui::Id::new("opt_value_field_picker_request")
}

fn choice_key() -> egui::Id {
    egui::Id::new("opt_value_field_picker_choice")
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
    let constants: Arc<Vec<Constant>> = Arc::new(rows.iter().filter_map(ConstantsRow::constant).cloned().collect());
    ctx.data_mut(|data| data.insert_temp(constants_key(), constants));
}

fn published_constants(ctx: &egui::Context) -> Arc<Vec<Constant>> {
    ctx.data(|data| data.get_temp::<Arc<Vec<Constant>>>(constants_key())).unwrap_or_default()
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
    fn edit(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, range: (f64, f64)) -> egui::Response;
}

impl FieldEditor for f64 {
    fn edit(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, range: (f64, f64)) -> egui::Response {
        let (mut response, parsed) = edit_number(ui, id, self.to_string(), width, range);
        if let Some(parsed) = parsed {
            *self = parsed;
            response.mark_changed();
        }
        response
    }
}

impl FieldEditor for f32 {
    fn edit(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, range: (f64, f64)) -> egui::Response {
        let (mut response, parsed) = edit_number(ui, id, self.to_string(), width, range);
        if let Some(parsed) = parsed {
            *self = parsed as f32;
            response.mark_changed();
        }
        response
    }
}

impl FieldEditor for String {
    fn edit(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, _range: (f64, f64)) -> egui::Response {
        ui.add(egui::TextEdit::singleline(self).id(id).desired_width(width))
    }
}

impl FieldEditor for bool {
    fn edit(&mut self, ui: &mut egui::Ui, id: egui::Id, width: f32, _range: (f64, f64)) -> egui::Response {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, ui.spacing().interact_size.y), egui::Sense::hover());
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect).id_salt(id), |ui| ui.checkbox(self, "")).inner
    }
}

/// A text box for a whole number of at least `range.0`, that takes digits only.
pub(crate) fn edit_count(ui: &mut egui::Ui, id: egui::Id, value: &mut u32, width: f32, range: (u32, u32)) -> egui::Response {
    let (mut response, parsed) = edit_number(ui, id, value.to_string(), width, (f64::from(range.0), f64::from(range.1)));
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
fn edit_number(ui: &mut egui::Ui, id: egui::Id, shown: String, width: f32, range: (f64, f64)) -> (egui::Response, Option<f64>) {
    let buffer_id = id.with("typed");
    let focused = ui.memory(|memory| memory.has_focus(id));
    let mut text = if focused {
        ui.data_mut(|data| data.get_temp::<String>(buffer_id)).unwrap_or(shown)
    } else {
        shown
    };
    let response = ui.add(egui::TextEdit::singleline(&mut text).id(id).desired_width(width).horizontal_align(egui::Align::RIGHT));
    let filtered = filter_number(&text, range.0 < 0.0);
    let mut parsed = None;
    if response.changed() || filtered != text {
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
}

impl<'a, T: FieldEditor> ValueField<'a, T> {
    pub(crate) fn new(id_salt: impl Hash + Debug, value: &'a mut FieldValue<T>) -> Self {
        Self {
            id: egui::Id::new(("opt_value_field", id_salt)),
            value,
            range: (f64::MIN, f64::MAX),
        }
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
        let Self { id, value, range, .. } = self;
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
            .horizontal(|ui| {
                let response = match &value.constant {
                    Some(name) => {
                        let found = constants.iter().find(|constant| constant.name == *name);
                        let fitting = found.and_then(|constant| T::from_constant(&constant.value));
                        let mut shown = name.clone();
                        let response = ui.add_enabled(false, egui::TextEdit::singleline(&mut shown).desired_width(editor_width));
                        let message = match (found, fitting) {
                            (None, _) => Some(tr!("opt-constant-missing", name = name.clone())),
                            (Some(_), None) => Some(tr!("opt-constant-wrong-type", name = name.clone())),
                            (Some(constant), Some(_)) => return_value_hint(&response, &constant.value),
                        };
                        finish_locked(response, message)
                    }
                    None => value.value.edit(ui, id, editor_width, range),
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
                        ToolbarButton::new(egui::Image::new(themed_icon!(ui, "constant_pick.svg")), tr!("opt-pick-constant"))
                            .id_salt(id.with("pick"))
                            .button_side(BUTTON_SIDE),
                    )
                    .clicked()
                {
                    requested = true;
                }
                response
            })
            .inner;
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
    let _ = response.clone().on_disabled_hover_text(value.display());
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
            response.on_disabled_hover_text(message)
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
    let constants = published_constants(ctx);
    let candidates: Vec<&Constant> = constants.iter().filter(|constant| constant.value.value_type() == request.value_type).collect();

    let sheet = egui::Area::new(egui::Id::new("opt_constant_picker_sheet"))
        .order(egui::Order::Foreground)
        .fixed_pos(ctx.content_rect().min)
        .interactable(true)
        .show(ctx, |ui| {
            let rect = ctx.content_rect();
            ui.allocate_rect(rect, egui::Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_black_alpha(70));
        });
    ctx.move_to_top(sheet.response.layer_id);

    let selected_id = egui::Id::new("opt_constant_picker_selected");
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
                    egui::Grid::new("opt_constant_picker_grid")
                        .num_columns(3)
                        .striped(true)
                        .spacing([14.0, 4.0])
                        .show(ui, |ui| {
                            for header in [tr!("opt-col-name"), tr!("opt-col-value"), tr!("opt-col-description")] {
                                ui.label(egui::RichText::new(header).weak());
                            }
                            ui.end_row();
                            for constant in &candidates {
                                let is_selected = selected.as_deref() == Some(constant.name.as_str());
                                let row = ui.selectable_label(is_selected, &constant.name);
                                ui.label(constant.value.display());
                                ui.label(&constant.description);
                                ui.end_row();
                                if row.clicked() {
                                    selected = Some(constant.name.clone());
                                }
                                if row.double_clicked() {
                                    choose = Some(constant.name.clone());
                                }
                            }
                        });
                });
            }
            let ready = selected.as_ref().is_some_and(|name| candidates.iter().any(|constant| constant.name == *name));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("opt-select")).primary().enabled(ready)).clicked() {
                    choose = selected.clone();
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    cancel = true;
                }
            });
        });
    if let Some(dialog) = dialog {
        ctx.move_to_top(dialog.response.layer_id);
    }

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

fn close_picker(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        data.remove::<PickerRequest>(request_key());
        data.remove::<String>(egui::Id::new("opt_constant_picker_selected"));
    });
    ctx.request_repaint();
}

/// Whether the picker is up, which the editor uses to hold back its own Escape.
pub(crate) fn picker_open(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<PickerRequest>(request_key())).is_some()
}
