//! The Assistant: where the AI is talked to and set up.
//!
//! Shown by View > Assistant, as an island down the right of the main window
//! with a button in its corner that pops it out into a window of its own
//! (`app/panel_windows.rs`), as the explorer and the console can be.
//!
//! The chat has nothing behind it yet: no provider is connected, so a message
//! can be written but not sent. The `/` commands ([`SlashCommand`]) are done
//! here, and work regardless.

use crate::{
    i18n::tr,
    ui::{
        chrome,
        state::{AssistantMessage, EditorState, PanelKind, UiCommand},
        widgets::{
            composer::MessageComposer,
            island::{Island, IslandResponse, Side},
            menu, pop_out,
            toolbar::GROUP_CORNER_RADIUS,
        },
    },
};

/// Id of the Assistant's island.
const PANEL_ID: &str = "assistant_island";
/// The share of the conversation's width a message of the user's may take.
const USER_MESSAGE_WIDTH: f32 = 0.85;
/// Rows the message box grows to before it scrolls.
const DRAFT_ROWS: usize = 8;

/// Draw the Assistant docked down the right of the main window.
pub(crate) fn draw_assistant_island(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) -> IslandResponse<()> {
    let island = Island::new(PANEL_ID, Side::Right)
        .default_width(360.0)
        .min_width(260.0)
        .show(ui, |ui, _| draw_chat(ui, editor, commands));
    if let Some(region) = island.regions.first()
        && pop_out::corner_button(ui.ctx(), "assistant", chrome::region_rect(*region), tr!("panel-pop-out"))
    {
        commands.push(UiCommand::SetPanelPoppedOut(PanelKind::Assistant, true));
    }
    island
}

/// Draw the Assistant filling a window of its own, popped out. Returns its
/// region.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn draw_assistant_window(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) -> egui::Rect {
    let island = Island::new(PANEL_ID, Side::Right).filling(true).show(ui, |ui, _| draw_chat(ui, editor, commands));
    island.regions.first().copied().unwrap_or(egui::Rect::NOTHING)
}

/// A `/` command: written in the message box, and done by the Assistant's
/// window itself rather than sent anywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlashCommand {
    Clear,
    Help,
}

impl SlashCommand {
    const ALL: [Self; 2] = [Self::Clear, Self::Help];

    /// What is typed after the `/`. Not translated: it is typed.
    fn name(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Help => "help",
        }
    }

    fn description(self) -> String {
        match self {
            Self::Clear => tr!("assistant-command-clear"),
            Self::Help => tr!("assistant-command-help"),
        }
    }

    fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|command| command.name() == name)
    }
}

/// The commands `text` could be the start of, while it is a `/` and a name
/// and nothing after it.
fn slash_suggestions(text: &str) -> Vec<egui::Suggestion> {
    let Some(typed) = text.strip_prefix('/').filter(|typed| !typed.contains(char::is_whitespace)) else {
        return Vec::new();
    };
    SlashCommand::ALL
        .into_iter()
        .filter(|command| command.name().starts_with(typed) && command.name() != typed)
        .map(|command| egui::Suggestion::new(format!("/{}", command.name())).description(command.description()))
        .collect()
}

/// Whether `text` is a `/` command, to be done here rather than sent.
fn is_slash_command(text: &str) -> bool {
    text.trim_start().starts_with('/')
}

/// Do what was sent: a `/` command here, anything else into the conversation.
fn submit(editor: &mut EditorState, text: String) {
    let text = text.trim();
    let Some(command) = text.strip_prefix('/') else {
        editor.assistant.messages.push(AssistantMessage::User(text.to_owned()));
        return;
    };
    let name = command.split_whitespace().next().unwrap_or_default();
    let messages = &mut editor.assistant.messages;
    match SlashCommand::named(name) {
        Some(SlashCommand::Clear) => messages.clear(),
        Some(SlashCommand::Help) => {
            let lines: Vec<String> = SlashCommand::ALL.into_iter().map(|command| format!("/{}  {}", command.name(), command.description())).collect();
            messages.push(AssistantMessage::Notice(format!("{}\n{}", tr!("assistant-commands-heading"), lines.join("\n"))));
        }
        None => messages.push(AssistantMessage::Notice(tr!("assistant-unknown-command", name = format!("/{name}")))),
    }
}

/// The conversation, with the message being written under it.
fn draw_chat(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let _ = commands;
    menu::apply_menu_style(ui, ui.visuals().panel_fill);
    egui::Panel::bottom("assistant_composer")
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            ui.add_space(6.0);
            menu::menu_note(ui, tr!("assistant-not-connected"));
            // Commands are done here, so they go with nothing connected.
            let connected = false;
            let sendable = connected || is_slash_command(&editor.assistant.draft);
            let mut composer = MessageComposer::new("assistant_draft", &mut editor.assistant.draft, tr!("assistant-send"))
                .hint_text(tr!("assistant-message-hint"))
                .max_rows(DRAFT_ROWS)
                .completions(slash_suggestions);
            if !sendable {
                composer = composer.unavailable(tr!("assistant-send-not-connected"));
            }
            if composer.show(ui) {
                let text = std::mem::take(&mut editor.assistant.draft);
                submit(editor, text);
            }
        });
    egui::CentralPanel::no_frame().show(ui, |ui| draw_transcript(ui, &editor.assistant.messages));
}

/// The conversation so far, kept scrolled to its newest end.
fn draw_transcript(ui: &mut egui::Ui, messages: &[AssistantMessage]) {
    if messages.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new(tr!("assistant-empty")).weak());
        });
        return;
    }
    egui::ScrollArea::vertical().auto_shrink(false).stick_to_bottom(true).show(ui, |ui| {
        for message in messages {
            match message {
                AssistantMessage::User(text) => {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        egui::Frame::new()
                            .fill(ui.visuals().widgets.inactive.weak_bg_fill)
                            .corner_radius(GROUP_CORNER_RADIUS)
                            .inner_margin(egui::Margin::symmetric(8, 6))
                            .show(ui, |ui| {
                                ui.set_max_width(ui.available_width() * USER_MESSAGE_WIDTH);
                                ui.label(text.as_str());
                            });
                    });
                }
                AssistantMessage::Notice(text) => menu::menu_note(ui, text.as_str()),
            }
            ui.add_space(4.0);
        }
    });
}
