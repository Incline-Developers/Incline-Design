//! A message box with its send button inside it, for writing to something
//! that answers: the Assistant's chat.
//!
//! One rounded card, sunk into the surface as every entry box is
//! ([`super::menu::apply_menu_style`]), outlined in the accent while it has
//! focus. The text grows a row at a time as it is written, up to a limit past
//! which it scrolls, and the send button sits at the card's bottom right.
//! Enter sends; Shift+Enter starts a new line, as in other chat boxes.
//!
//! It can offer completions for what is being written (the Assistant's `/`
//! commands): a popup above the box, chosen from with the arrow keys and
//! taken with Tab, or with Enter, which sends it straight away.

use super::{hint::Hint, toolbar::GROUP_CORNER_RADIUS};

/// Side of the send button's tile.
const SEND_SIZE: f32 = 22.0;
/// Space between the card's edge and what is in it.
const PADDING: i8 = 6;
/// Length of the send arrow, tip to tail.
const ARROW_LENGTH: f32 = 10.0;

/// Suggestions for the text before the cursor, given the whole of it.
type Suggest<'a> = Box<dyn FnOnce(&str) -> Vec<egui::Suggestion> + 'a>;

pub(crate) struct MessageComposer<'a> {
    id_salt: &'a str,
    suggest: Option<Suggest<'a>>,
    text: &'a mut String,
    hint: egui::WidgetText,
    can_send: bool,
    send_label: egui::WidgetText,
    unavailable: Option<egui::WidgetText>,
    max_rows: usize,
}

impl<'a> MessageComposer<'a> {
    /// `send_label` names the send button, in its hint: the button itself is
    /// an arrow.
    pub(crate) fn new(id_salt: &'a str, text: &'a mut String, send_label: impl Into<egui::WidgetText>) -> Self {
        Self {
            id_salt,
            suggest: None,
            text,
            hint: egui::WidgetText::default(),
            can_send: true,
            send_label: send_label.into(),
            unavailable: None,
            max_rows: 8,
        }
    }

    /// Greyed-out text in the empty box.
    pub(crate) fn hint_text(mut self, hint: impl Into<egui::WidgetText>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Nothing can be sent, and `why` says so on the button. The box can still
    /// be written in.
    pub(crate) fn unavailable(mut self, why: impl Into<egui::WidgetText>) -> Self {
        self.can_send = false;
        self.unavailable = Some(why.into());
        self
    }

    /// Offer `suggest`'s completions of the text before the cursor, whole.
    pub(crate) fn completions(mut self, suggest: impl FnOnce(&str) -> Vec<egui::Suggestion> + 'a) -> Self {
        self.suggest = Some(Box::new(suggest));
        self
    }

    /// Rows the text grows to before it scrolls.
    pub(crate) fn max_rows(mut self, rows: usize) -> Self {
        self.max_rows = rows.max(1);
        self
    }

    /// Draw the box across the width `ui` has, and say whether its text was
    /// sent: by Enter or by the button, and only if there is something to
    /// send. Clearing the text after sending is the caller's.
    pub(crate) fn show(self, ui: &mut egui::Ui) -> bool {
        let Self {
            id_salt,
            suggest,
            text,
            hint,
            can_send,
            send_label,
            unavailable,
            max_rows,
        } = self;
        let id = ui.make_persistent_id(id_salt);
        let focused = ui.memory(|memory| memory.has_focus(id));
        // Read before the box runs: an open popup takes the Enter for itself,
        // and taking a suggestion with it sends it.
        let enter_pressed = focused && ui.input(|input| input.key_pressed(egui::Key::Enter) && input.modifiers.is_none());
        let sendable = can_send && !text.trim().is_empty();
        let visuals = ui.visuals();
        let stroke = if focused {
            egui::Stroke::new(1.0, visuals.selection.stroke.color)
        } else {
            visuals.widgets.noninteractive.bg_stroke
        };
        let row_height = ui.text_style_height(&egui::TextStyle::Body);
        let width = ui.available_width();
        let mut clicked = false;
        let mut taken = false;
        let card = egui::Frame::new()
            .fill(visuals.text_edit_bg_color())
            .stroke(stroke)
            .corner_radius(GROUP_CORNER_RADIUS)
            .inner_margin(egui::Margin::same(PADDING))
            .show(ui, |ui| {
                let inner = (width - 2.0 * f32::from(PADDING) - stroke.width * 2.0).max(0.0);
                ui.set_width(inner);
                let gap = ui.spacing().item_spacing.x;
                let text_width = (inner - SEND_SIZE - gap).max(0.0);
                // The text on the left, growing downwards; the button at the
                // bottom right of whatever height that comes to.
                let top_left = ui.cursor().min;
                let mut text_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(egui::Rect::from_min_size(top_left, egui::vec2(text_width, f32::INFINITY)))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                egui::ScrollArea::vertical()
                    .id_salt((id_salt, "scroll"))
                    .max_height(row_height * max_rows as f32)
                    .auto_shrink([false, true])
                    .stick_to_bottom(true)
                    .show(&mut text_ui, |ui| {
                        let edit = egui::TextEdit::multiline(text)
                            .id(id)
                            .frame(egui::Frame::NONE)
                            .hint_text(hint)
                            .desired_rows(1)
                            .desired_width(text_width)
                            .margin(egui::Margin::ZERO)
                            // A new line is Shift+Enter: Enter sends.
                            .return_key(egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::Enter));
                        match suggest {
                            Some(suggest) => {
                                let output = egui::CompletionPopup::new(id)
                                    .word_boundary(|_| false)
                                    .align(egui::RectAlign::TOP_START)
                                    .show(ui, edit, |query| suggest(query.text));
                                taken = output.accepted.is_some();
                            }
                            None => {
                                ui.add(edit);
                            }
                        }
                    });
                let height = text_ui.min_rect().height().max(SEND_SIZE);
                let row = egui::Rect::from_min_size(top_left, egui::vec2(inner, height));
                ui.allocate_rect(row, egui::Sense::hover());
                let button = egui::Rect::from_min_size(egui::pos2(row.right() - SEND_SIZE, row.bottom() - SEND_SIZE), egui::Vec2::splat(SEND_SIZE));
                clicked = send_button(ui, button, id.with("send"), sendable, &send_label, unavailable).clicked();
            });
        // A press anywhere on the card is a press in the box.
        if card.response.interact(egui::Sense::click()).clicked() {
            ui.memory_mut(|memory| memory.request_focus(id));
        }
        // Whatever Enter the popup left is for sending. Taken whether or not
        // there is anything to send, so it never reaches anything else.
        let entered = enter_pressed && (taken || ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)));
        can_send && !text.trim().is_empty() && (entered || clicked)
    }
}

/// The send button: an arrow on an accent tile when there is something to
/// send, greyed out when not.
fn send_button(ui: &mut egui::Ui, rect: egui::Rect, id: egui::Id, enabled: bool, label: &egui::WidgetText, unavailable: Option<egui::WidgetText>) -> egui::Response {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let response = ui.interact(rect, id, sense);
    let visuals = ui.visuals();
    let (fill, arrow) = if enabled {
        let accent = visuals.selection.stroke.color;
        let fill = if response.is_pointer_button_down_on() {
            accent.gamma_multiply(0.8)
        } else if response.hovered() {
            accent.gamma_multiply(0.9)
        } else {
            accent
        };
        (fill, egui::Color32::WHITE)
    } else {
        (visuals.widgets.inactive.weak_bg_fill, visuals.weak_text_color())
    };
    let painter = ui.painter();
    painter.rect_filled(rect, GROUP_CORNER_RADIUS, fill);
    let stroke = egui::Stroke::new(1.5, arrow);
    let centre = rect.center();
    let half = ARROW_LENGTH / 2.0;
    let tip = egui::pos2(centre.x, centre.y - half);
    painter.line_segment([egui::pos2(centre.x, centre.y + half), tip], stroke);
    painter.line_segment([egui::pos2(centre.x - half * 0.8, tip.y + half * 0.8), tip], stroke);
    painter.line_segment([egui::pos2(centre.x + half * 0.8, tip.y + half * 0.8), tip], stroke);
    match unavailable {
        Some(why) if !enabled => response.hint(why),
        _ => response.hint(label.clone()),
    }
}
