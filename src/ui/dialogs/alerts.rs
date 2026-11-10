//! Warning and error alerts: the console's warnings and errors, raised as a
//! dialog so they are seen with the console closed.
//!
//! One alert per console activity, once it has finished, so a command that
//! warns twenty times opens one dialog listing all twenty. An activity that
//! later escalates from a warning to an error is raised again. Alerts stack:
//! the dialog shows the newest, since it is about what was just done, and
//! counts the older ones waiting behind it.

use std::collections::{HashMap, VecDeque};

use crate::{
    i18n::tr,
    logging::{ConsoleEntryId, ConsoleEntryState, ConsoleSeverity, ConsoleSnapshot},
    ui::{
        elements::console::{console_entry_text, severity_icon},
        state::EditorState,
        widgets::menu::{self, DragableMenu, MenuButton},
    },
};

const ICON_SIDE: f32 = 32.0;
/// The most of the window an alert's messages may take before they scroll,
/// so a long run of them never pushes its buttons off screen.
const MESSAGES_MAX_SHARE: f32 = 0.6;
/// One width for every alert, so the dialog does not jump as they change.
const WIDTH: f32 = 440.0;

/// Which console activities have been raised, and the ones still waiting to be read.
#[derive(Clone, Debug, Default)]
pub(crate) struct Alerts {
    scanned_revision: Option<u64>,
    raised: HashMap<ConsoleEntryId, ConsoleSeverity>,
    queue: VecDeque<ConsoleEntryId>,
}

impl Alerts {
    /// Whether an alert is waiting to be read, and so owns Enter and Escape.
    pub(crate) fn is_showing(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Queue every finished warning or error not yet raised at its severity,
    /// and return the most severe of them, to sound once for the lot.
    fn collect(&mut self, (revision, entries): &ConsoleSnapshot) -> Option<ConsoleSeverity> {
        if self.scanned_revision == Some(*revision) {
            return None;
        }
        self.scanned_revision = Some(*revision);
        let mut loudest: Option<ConsoleSeverity> = None;
        for entry in entries.iter() {
            if entry.state != ConsoleEntryState::Complete || entry.severity.rank() < ConsoleSeverity::Warn.rank() {
                continue;
            }
            if self.raised.get(&entry.id).is_some_and(|raised| raised.rank() >= entry.severity.rank()) {
                continue;
            }
            self.raised.insert(entry.id, entry.severity);
            // Newest in front; one raised again comes back to the front.
            self.queue.retain(|queued| *queued != entry.id);
            self.queue.push_front(entry.id);
            loudest = Some(loudest.map_or(entry.severity, |loudest| loudest.escalate(entry.severity)));
        }
        loudest
    }
}

/// Raise new warnings and errors, and draw the alert at the head of the queue.
pub(crate) fn draw_alert_dialog(ui: &mut egui::Ui, editor: &mut EditorState, snapshot: &ConsoleSnapshot) {
    if let Some(severity) = editor.alerts.collect(snapshot) {
        crate::system_sound::play_alert(severity);
    }
    let entries = &snapshot.1;
    // An activity the console has since trimmed away has nothing left to show.
    let entry = loop {
        let Some(&id) = editor.alerts.queue.front() else {
            return;
        };
        if let Some(entry) = entries.iter().find(|entry| entry.id == id) {
            break entry;
        }
        editor.alerts.queue.pop_front();
    };
    let waiting = editor.alerts.queue.len() - 1;

    let mut open = true;
    let mut close = false;
    DragableMenu::new("alert_dialog", entry.title.clone())
        .open(&mut open)
        .order(egui::Order::Tooltip)
        .min_width(WIDTH)
        .max_width(WIDTH)
        .show(ui.ctx(), |ui| {
            // Room for two lines of message and the line under them whatever
            // the message, so most alerts come up the same size.
            let line = ui.text_style_height(&egui::TextStyle::Body);
            let spacing = ui.spacing().item_spacing.y;
            let message_height = 2.0 * line + spacing + ui.text_style_height(&egui::TextStyle::Small);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.set_min_height(message_height);
                ui.add(egui::Image::new(severity_icon(entry.severity)).fit_to_exact_size(egui::Vec2::splat(ICON_SIDE)));
                ui.vertical(|ui| {
                    let label = if entry.severity == ConsoleSeverity::Error {
                        tr!("alerts-error")
                    } else {
                        tr!("alerts-warning")
                    };
                    let size = egui::TextStyle::Body.resolve(ui.style()).size;
                    let color = ui.visuals().text_color();
                    // Every message whole: the summary is the console's one
                    // line, shortened to fit a row, and the details are what
                    // was actually said. One that warned twenty times says all
                    // twenty, one under the other.
                    let messages: Vec<&str> = if entry.details.is_empty() {
                        vec![entry.summary.as_str()]
                    } else {
                        entry.details.iter().map(String::as_str).collect()
                    };
                    let max_height = ui.ctx().content_rect().height() * MESSAGES_MAX_SHARE;
                    egui::ScrollArea::vertical().max_height(max_height).auto_shrink([false, true]).show(ui, |ui| {
                        for (index, text) in messages.into_iter().enumerate() {
                            let mut message = egui::text::LayoutJob::default();
                            if index == 0 {
                                message.append(
                                    &format!("{label} "),
                                    0.0,
                                    egui::TextFormat::simple(egui::FontId::new(size, egui::FontFamily::Name("noto_sans_bold".into())), color),
                                );
                            }
                            message.append(text, 0.0, egui::TextFormat::simple(egui::FontId::proportional(size), color));
                            ui.add(egui::Label::new(message).wrap().selectable(true));
                        }
                    });
                    let meta = if waiting > 0 {
                        tr!("alerts-time-more-waiting", time = entry.timestamp.clone(), count = waiting)
                    } else {
                        entry.timestamp.clone()
                    };
                    ui.label(egui::RichText::new(meta).small().weak());
                });
            });
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("common-close")).primary()).clicked() || menu::dialog_confirmed(ui.ctx()) || menu::dialog_cancelled(ui.ctx()) {
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("alerts-copy-details"))).clicked() {
                    ui.ctx().copy_text(console_entry_text(entry));
                    editor.toasts.push(tr!("alerts-copied"));
                }
            });
        });

    if close || !open {
        editor.alerts.queue.pop_front();
    }
}
