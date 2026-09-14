//! The floating 3D sequence editor: one Gantt bar's dig order, picked off the
//! blocks a completed Solids run committed.
//!
//! Three things make this a draft rather than a live editor, and all three are
//! deliberate. It edits a copy held in [`SequenceDraft`], so Cancel touches no
//! durable state at all. It applies in one command, so an editing session -
//! however many blocks were added, removed and reordered inside it - is one
//! Ctrl-Z. And it refuses rather than guesses: the draft carries the session
//! it was opened under, the bar it edits, the order it was opened from and,
//! inside each new member, the run generation it was picked against, so a
//! project switch, a deleted bar, an undo underneath it or a Dig Strips rerun
//! each end in a stated refusal instead of in an overwrite.
//!
//! **Opening it starts nothing.** The window reads the artifacts a completed
//! run committed, exactly as the Solids pages do; it sets no pipeline demand,
//! and a project whose Dig Strips stage has not been run opens onto a stated
//! reason rather than onto an empty pit.

use std::collections::BTreeSet;

use crate::{
    i18n::tr,
    model::{Document, schedule::SchedulePlan},
    ui::{
        EditorState, UiProjectView,
        state::{ScheduleEdit, SequenceDraft, SequenceListDrag, SequenceListDragMode, UiCommand},
        widgets::{
            menu::{self, DragableMenu, MenuButton},
            toolbar::GROUP_CORNER_RADIUS,
        },
    },
};

/// Least size of the window. Wide enough for the 3D view and the ordered list
/// side by side, which is the whole point of the layout: a block is picked in
/// one and read in the other without either moving.
const MIN_SIZE: egui::Vec2 = egui::vec2(820.0, 520.0);
/// Width of the navigation column: the dig tree and the objects beneath it.
const NAV_WIDTH: f32 = 280.0;
const MIN_LIST_WIDTH: f32 = 360.0;
const MAX_LIST_WIDTH: f32 = 720.0;
/// Room beside the longest row for its remove cross, the scroll bar and the
/// frame.
const LIST_PADDING: f32 = 80.0;
/// Space above and below a dig order row's single line of text.
const ROW_PADDING: f32 = 5.0;
/// Side of the square a row's remove cross is drawn inside.
const REMOVE_CROSS: f32 = 18.0;
/// Half-height of the triangle marking the preview slider's place in the list.
const PLAYHEAD_MARK: f32 = 5.0;

/// One edit of the ordered list, collected during the pass and applied after
/// it so the list is never mutated while it is being drawn.
#[derive(Clone, PartialEq)]
enum ListEdit {
    Remove(usize),
    /// Take `rows` out of the order and put them back, in this order, at the
    /// gap `to` - which is counted in the list as it stands *before* anything
    /// is taken out, because that is the list the user was pointing at.
    Move {
        rows: Vec<usize>,
        to: usize,
    },
}

pub(crate) fn draw_sequence_editor(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    project: &UiProjectView,
    document: &Document,
    plan: &SchedulePlan,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let Some(mut draft) = editor.sequence_editor.clone() else {
        return;
    };
    // The project changed under the window. Its ids name that project's work,
    // and a fresh project numbers its first bar `0` too, so there is nothing
    // here that could be carried across.
    if draft.session != session {
        editor.close_sequence_editor();
        return;
    }
    let bar = plan.bar(draft.bar);
    let title = match bar {
        Some(bar) => tr!("sequence-editor-title", bar = crate::ui::elements::schedule_gantt::bar_display_name(editor, bar)),
        None => tr!("schedule-bar-edit-sequence"),
    };
    let dirty = draft.is_dirty();
    // The bar's order as it stands now. A draft opened from something else is
    // an older version of the same order, and writing it back would revert a
    // newer edit; Apply refuses it, and so does this.
    let target_changed = bar.is_some_and(|bar| bar.members() != draft.opened_from);
    // While the discard question is on screen it owns the window behind it:
    // the editor is inert - no keys, no list edits, no camera, no picks - and
    // the question is the only thing that can be answered. Enter and Escape
    // belong to it, so nothing here may consume them first.
    let confirming = draft.confirming_close;

    let mut open = true;
    let mut close = false;
    let mut edit: Option<ListEdit> = None;
    let mut reload = false;
    let mut changed = false;

    // The whole application window, nailed to it. This dialog is a page
    // rather than a tool window: there is nothing behind it worth uncovering,
    // so it neither moves nor resizes and no part of it can be dragged off
    // the screen.
    let screen = ui.ctx().content_rect();
    let menu = DragableMenu::new("sequence_editor", title)
        .min_width(MIN_SIZE.x.min(screen.width()))
        .fixed_size(screen.size())
        .pinned(screen.min);
    // No close button of its own while the question stands: the question is
    // the only way out of the window, and a second close behind it would be a
    // second answer to the same question.
    let menu = if confirming { menu } else { menu.open(&mut open) };
    menu.show(ui.ctx(), |ui| {
        let Some(bar) = bar else {
            // Deleted, or undone, while the window was open. Nothing was
            // applied, and there is nothing left to apply it to.
            ui.label(egui::RichText::new(tr!("sequence-editor-bar-gone")).color(ui.visuals().error_fg_color));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!(literal = "Close"))).clicked() {
                    close = true;
                }
            });
            return;
        };
        if target_changed {
            ui.label(egui::RichText::new(tr!("sequence-editor-target-changed")).color(ui.visuals().error_fg_color));
        }
        let available = ui.available_rect_before_wrap();
        // What the preview slider and the action row need. The columns take
        // the rest, and have to be sized before the frame reaches either - so
        // this is stated here rather than measured from rows that do not
        // exist yet. The window is exactly as tall as it says it is, so an
        // under-estimate here is a foot hanging off it.
        let footer = ui.spacing().interact_size.y * 2.0 + ui.spacing().item_spacing.y * 4.0 + 18.0;
        let body_height = (available.height() - footer).max(160.0);
        // The three columns are measured against one width, so the 3D view
        // takes exactly what the navigation tree and the dig order leave. A
        // narrow window shrinks the two side columns rather than pushing the
        // view off the edge of the dialog.
        let nav_width = NAV_WIDTH.min(available.width() * 0.28);
        let list_width = order_list_width(ui, editor, &draft).clamp(MIN_LIST_WIDTH.min(available.width() * 0.32), MAX_LIST_WIDTH.min(available.width() * 0.48));
        ui.horizontal_top(|ui| {
            // Stated rather than inherited, twice over. The columns sit in a
            // horizontal layout, and a child allocated inside one is
            // horizontal too, while both trees below indent their children -
            // which egui only permits in a vertical layout. And a column is
            // only as wide as what it allocates: the 3D pane paints into its
            // rect without allocating any of it, so without a stated minimum
            // that column collapses and the dig order beside it is drawn over
            // the image. Each tree brings its own scroll area, so neither is
            // wrapped in one here.
            ui.allocate_ui_with_layout(egui::vec2(nav_width, body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_min_size(egui::vec2(nav_width, body_height));
                // Both trees band their background across their clip rect,
                // and a vertical scroll area leaves the horizontal clip
                // exactly as it found it - so without this the objects tree
                // stripes the whole dialog and the dig order beside it reads
                // as sitting on the navigation panel.
                ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                let half = (body_height - ui.spacing().item_spacing.y) * 0.5;
                ui.allocate_ui(egui::vec2(ui.available_width(), half), |ui| {
                    ui.label(egui::RichText::new(tr!(literal = "Solids Navigation")).strong());
                    crate::ui::elements::solids_view::draw_tree(ui, editor, document);
                });
                ui.separator();
                ui.allocate_ui(egui::vec2(ui.available_width(), half), |ui| {
                    ui.label(egui::RichText::new(tr!(literal = "Objects")).strong());
                    crate::ui::elements::explorer::draw_object_tree(ui, editor, project, commands);
                });
            });
            let view_width = (available.width() - nav_width - list_width - ui.spacing().item_spacing.x * 2.0).max(200.0);
            ui.allocate_ui_with_layout(egui::vec2(view_width, body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_min_size(egui::vec2(view_width, body_height));
                changed |= draw_view(ui, editor, &mut draft, session, confirming);
            });
            ui.allocate_ui_with_layout(egui::vec2(list_width, body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_min_size(egui::vec2(list_width, body_height));
                ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                let (list_edit, selection) = draw_order_list(ui, editor, &draft, confirming);
                edit = list_edit;
                if selection != draft.selected {
                    draft.selected = selection;
                    changed = true;
                }
            });
        });
        changed |= draw_preview_slider(ui, &mut draft, confirming);
        menu::menu_actions(ui, |ui| {
            let can_apply = !target_changed;
            // Enter applies and Escape cancels, as every other dialog in
            // the app does. The keys are consumed either way, so Escape
            // closing this window does not also reach past it - except
            // while the discard question is up, when neither is asked for
            // here at all: the question owns both keys, and the draft
            // under it must not be applied by a keypress meant to answer
            // a question about throwing it away.
            let submitted = !confirming && menu::dialog_confirm_pressed(ui.ctx());
            let cancelled = !confirming && menu::dialog_cancel_pressed(ui.ctx());
            ui.add_enabled_ui(!confirming, |ui| {
                if (submitted || ui.add(MenuButton::new(tr!("sequence-editor-apply")).primary().enabled(can_apply)).clicked()) && can_apply {
                    commands.push(UiCommand::schedule(
                        session,
                        ScheduleEdit::SetBarMembers {
                            bar: bar.id,
                            expected: draft.opened_from.clone(),
                            members: draft.members.clone(),
                        },
                    ));
                    // Not closed here: the window closes when the command is
                    // accepted, so a refusal leaves the draft that caused it
                    // on screen where it can be fixed.
                }
                if target_changed && ui.add(MenuButton::new(tr!("sequence-editor-reload"))).clicked() {
                    reload = true;
                }
                if ui.add(MenuButton::new(tr!(literal = "Cancel"))).clicked() || cancelled {
                    close = true;
                }
            });
        });
    });

    if let Some(edit) = edit {
        apply_list_edit(editor, &mut draft, edit);
        changed = true;
    }
    if reload && let Some(bar) = bar {
        draft = SequenceDraft::open(session, bar.id, bar.members());
        changed = true;
    }
    if changed {
        editor.sequence_editor = Some(draft);
        ui.ctx().request_repaint();
    }
    // Closing with unapplied changes asks before discarding them; closing a
    // draft nobody changed asks nothing, because there is nothing to lose.
    if close || !open {
        if dirty && bar.is_some() && !reload {
            if let Some(draft) = editor.sequence_editor.as_mut() {
                draft.confirming_close = true;
            }
        } else {
            editor.close_sequence_editor();
        }
    }
    draw_discard_confirmation(ui, editor, plan);
}

/// Ask before throwing an editing session away.
///
/// While it is up, this question is the only interactive thing in the window:
/// the editor behind it draws but does not act - no Apply on Enter, no Cancel
/// on Escape, no list edits, no camera, no picks. Both keys are consumed here
/// instead, and both answer *Keep Editing*: Enter confirms the question's
/// primary action, Escape dismisses the question, and neither is a way to
/// apply or discard the draft underneath. Discarding is the explicit danger
/// button and nothing else.
fn draw_discard_confirmation(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan) {
    let Some((bar, confirming)) = editor.sequence_editor.as_ref().map(|draft| (draft.bar, draft.confirming_close)) else {
        return;
    };
    if !confirming {
        return;
    }
    let name = plan
        .bar(bar)
        .map(|bar| crate::ui::elements::schedule_gantt::bar_display_name(editor, bar))
        .unwrap_or_default();
    let mut open = true;
    let mut discard = false;
    let mut keep = false;
    DragableMenu::new("sequence_editor_discard", tr!("sequence-editor-discard-title"))
        .open(&mut open)
        .min_width(360.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("sequence-editor-discard-body", bar = name.clone()));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("sequence-editor-discard")).danger()).clicked() {
                    discard = true;
                }
                if ui.add(MenuButton::new(tr!("sequence-editor-keep-editing")).primary()).clicked()
                    || menu::dialog_confirm_pressed(ui.ctx())
                    || menu::dialog_cancel_pressed(ui.ctx())
                {
                    keep = true;
                }
            });
        });
    if discard {
        editor.close_sequence_editor();
    } else if (keep || !open)
        && let Some(draft) = editor.sequence_editor.as_mut()
    {
        draft.confirming_close = false;
    }
}

/// The 3D pane: the run's dig blocks, this bar's order numbered over them, and
/// the click that adds one.
///
/// The image is the same offscreen preview the Solids pages use, drawn by the
/// same renderer through this editor's own camera - one renderer, not two, and
/// nothing in the main viewport or on the Solids pages moves while it is open.
fn draw_view(ui: &mut egui::Ui, editor: &mut EditorState, draft: &mut SequenceDraft, session: u32, confirming: bool) -> bool {
    let mut changed = false;
    let rect = ui.available_rect_before_wrap();
    // Only an absent run has anything to say under the image. With one, the
    // pane is the whole column: what is on screen is the run, and a line
    // saying so is a line of ground given up to a caption.
    let unavailable = editor.sequence_unavailable.clone();
    let caption_height = if unavailable.is_some() {
        ui.text_style_height(&egui::TextStyle::Body) + 8.0
    } else {
        0.0
    };
    let image_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), (rect.bottom() - caption_height).max(rect.top())));
    if !image_rect.is_positive() {
        return false;
    }
    let has_run = unavailable.is_none();
    match editor.solid_preview_texture.filter(|_| has_run) {
        Some(texture_id) => {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(texture_id, image_rect, uv, egui::Color32::WHITE);
        }
        None => {
            ui.painter().rect_filled(image_rect, GROUP_CORNER_RADIUS, crate::ui::widgets::tree_row_colors(ui).1);
        }
    }
    // Painted first, interacted with second, so this stays the topmost
    // interactive widget over the pane whichever branch painted it. While the
    // discard question is up the pane is display only: no orbit, no zoom, and
    // above all no pick, because a pick edits the draft being asked about.
    let sense = if confirming { egui::Sense::hover() } else { egui::Sense::click_and_drag() };
    let response = ui.interact(image_rect, ui.id().with("sequence_editor_view"), sense);
    let pixels_per_point = ui.ctx().pixels_per_point();
    let size_px = [
        (image_rect.width() * pixels_per_point).round().max(1.0) as u32,
        (image_rect.height() * pixels_per_point).round().max(1.0) as u32,
    ];
    changed |= editor.solid_preview_size_px != size_px;
    editor.solid_preview_size_px = size_px;

    // The main viewport's own mapping, which the Solids View page also uses:
    // middle drag pans, right drag orbits, and left click selects without ever
    // moving the camera.
    let delta = response.drag_delta() * pixels_per_point;
    if delta != egui::Vec2::ZERO {
        let delta = [f64::from(delta.x), f64::from(delta.y)];
        if response.dragged_by(egui::PointerButton::Middle) {
            draft.view.pan_by_pixels(delta, f64::from(size_px[1]));
            changed = true;
        } else if response.dragged_by(egui::PointerButton::Secondary) {
            draft.view.orbit_by_pixels(delta, f64::from(size_px[1]));
            changed = true;
        }
    }
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(if response.dragged() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::PointingHand
        });
    }
    // Hovering survives the inert sense above, so the wheel is gated here:
    // a scroll behind the discard question must not move the camera of a
    // draft the user is being asked whether to keep.
    if response.hovered() && !confirming {
        let scroll = ui.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                        egui::MouseWheelUnit::Point => f64::from(delta.y * pixels_per_point),
                        egui::MouseWheelUnit::Line => f64::from(delta.y) * 100.0,
                        egui::MouseWheelUnit::Page => f64::from(delta.y) * f64::from(size_px[1]),
                    }),
                    _ => None,
                })
                .sum::<f64>()
        });
        if scroll != 0.0 {
            draft.view.zoom_by_scroll(scroll);
            changed = true;
        }
    }
    // A left press picks the block under it, and a left drag keeps picking as
    // it travels: one stroke takes every block it crosses. Which blocks those
    // may be is not decided here - the pane knows where the pointer is, and
    // the app knows what flitch the stroke started on - so all that is
    // recorded here is that a stroke is in progress.
    //
    // A stroke lasts exactly as long as the button is held, read from the
    // pointer rather than from the drag that began it: a release the pane
    // never saw - off the window, or with the application unfocused - ends
    // the stroke as surely as one it did.
    if !ui.input(|input| input.pointer.primary_down()) {
        editor.sequence_paint = None;
    } else if has_run && !confirming && response.drag_started_by(egui::PointerButton::Primary) {
        editor.sequence_paint = Some(crate::ui::state::SequencePaint::default());
    }
    let painting = editor.sequence_paint.is_some() && response.dragged_by(egui::PointerButton::Primary);
    // A press that never moved is a click and never a stroke, so the two can
    // never both be true on one frame.
    let picked = if painting {
        // A stroke that has wandered off the image picks nothing while it is
        // away, and picks again where it comes back on: the pointer is over
        // the dig order or the navigation tree, and a UV taken from there
        // names ground nobody is pointing at.
        ui.input(|input| input.pointer.interact_pos()).filter(|pos| image_rect.contains(*pos))
    } else if response.clicked() {
        response.interact_pointer_pos()
    } else {
        None
    };
    // The pick is addressed the moment it is made - this draft instance, this
    // run, this image - because it is resolved frames later, and an
    // unaddressed one would be answered as whatever the renderer is looking at
    // by then. The UV is a fraction of the image, not pane pixels: the
    // renderer sizes its target within limits of its own, so a pane outside
    // them is drawn at one size and would otherwise be picked against another.
    if has_run && let Some(pointer) = picked {
        let local = pointer - image_rect.min;
        editor.solid_preview_pick = Some(crate::ui::state::SolidPreviewPickRequest {
            session,
            owner: crate::ui::state::SolidPreviewPickOwner::SequenceEditor {
                bar: draft.bar,
                edition: draft.edition,
            },
            generation: editor.sequence_generation,
            image: editor.solid_preview_image_revision,
            uv: [local.x / image_rect.width().max(1.0), local.y / image_rect.height().max(1.0)],
        });
        ui.ctx().request_repaint();
    }

    if let Some(reason) = unavailable {
        let caption_rect = egui::Rect::from_min_max(egui::pos2(rect.left() + 8.0, image_rect.bottom()), rect.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(caption_rect), |ui| {
            ui.add(egui::Label::new(egui::RichText::new(tr!("sequence-editor-no-run", reason = reason)).color(ui.visuals().warn_fg_color)).truncate());
        });
    }
    changed
}

fn member_label(view: Option<&crate::ui::state::SequenceMemberView>) -> String {
    let Some(view) = view else {
        return tr!("sequence-editor-unknown-block");
    };
    if let Some(reason) = &view.unresolved {
        return reason.clone();
    }
    // Written as a path - `Pit/Pit A/348/1/348/1` - because that is what it
    // is: each field names the one inside it. Unspaced so a long row still
    // fits the column, and the two elevations carry no unit, the column
    // they are read in being nothing but elevations.
    [
        view.solid_type.as_deref(),
        view.solid_name.as_deref(),
        view.bench.as_deref(),
        view.blast.as_deref(),
        view.flitch.as_deref(),
        view.name.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("/")
}

/// How wide the dig order has to be for its longest row to fit, plus room for
/// the remove cross, the scroll bar and the row's own padding.
///
/// Only the longest label is laid out. The rows are all the same shape - the
/// same fields in the same order, differing in names and numbers - so the
/// widest of them is reliably the one with the most characters, and measuring
/// every row every frame to learn that would be a text layout per block.
fn order_list_width(ui: &egui::Ui, editor: &EditorState, draft: &SequenceDraft) -> f32 {
    let Some(longest) = (0..draft.members.len())
        .map(|index| member_label(editor.sequence_members.get(index)))
        .max_by_key(|label| label.chars().count())
    else {
        return MIN_LIST_WIDTH;
    };
    let font = egui::TextStyle::Body.resolve(ui.style());
    ui.painter().layout_no_wrap(longest, font, ui.visuals().text_color()).size().x + LIST_PADDING
}

/// The ordered list: what is in the dig order, in the order it will be dug,
/// with what the current run says about each place in it.
///
/// An unresolved member keeps its place and stays removable, saying why it did
/// not resolve where its name would otherwise be. Hiding it is the one thing
/// the identity layer exists to prevent: the reference is the user's own work,
/// and only they can decide what it should become.
///
/// Rows are one line high and all the same height, which is what lets a press
/// anywhere in the list be resolved to a row and a drag to a gap between two.
fn draw_order_list(ui: &mut egui::Ui, editor: &mut EditorState, draft: &SequenceDraft, confirming: bool) -> (Option<ListEdit>, BTreeSet<usize>) {
    let mut edit = None;
    let mut selected = draft.selected.clone();
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(tr!("sequence-editor-order")).strong());
        let unresolved = editor.sequence_members.iter().filter(|member| member.unresolved.is_some() && !member.stale_pick).count();
        let stale = editor.sequence_members.iter().filter(|member| member.stale_pick).count();
        if unresolved > 0 {
            ui.label(
                egui::RichText::new(tr!("sequence-editor-unresolved-kept", count = unresolved.to_string()))
                    .small()
                    .color(ui.visuals().warn_fg_color),
            );
        }
        if stale > 0 {
            ui.label(
                egui::RichText::new(tr!("sequence-editor-stale-picks", count = stale.to_string()))
                    .small()
                    .color(ui.visuals().error_fg_color),
            );
        }
        if draft.members.is_empty() {
            editor.sequence_list_drag = None;
            ui.label(egui::RichText::new(tr!("sequence-editor-empty-order")).weak());
            return;
        }
        // Rows and their crosses are one interaction surface, disabled whole
        // while the discard question is up: neither a selection nor a remove
        // or reorder may reach a draft the user is being asked whether to keep.
        ui.add_enabled_ui(!confirming, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let (list_edit, list_selection) = draw_order_rows(ui, editor, draft, confirming);
                edit = list_edit;
                selected = list_selection;
            });
        });
    });
    (edit, selected)
}

/// The rows themselves, inside the scroll area: drawn, hit-tested, swept,
/// carried and removed.
fn draw_order_rows(ui: &mut egui::Ui, editor: &mut EditorState, draft: &SequenceDraft, confirming: bool) -> (Option<ListEdit>, BTreeSet<usize>) {
    let mut selected = draft.selected.clone();
    let mut edit: Option<ListEdit> = None;
    // Last frame's drag, so a row being carried can be drawn as carried on the
    // frame the pointer is over its destination rather than one frame behind.
    let mut drag = editor.sequence_list_drag.filter(|_| !confirming && !draft.members.is_empty());
    let carrying = drag.is_some_and(|drag| drag.mode == SequenceListDragMode::Carry);

    let row_height = ui.text_style_height(&egui::TextStyle::Body) + ROW_PADDING * 2.0;
    let width = ui.available_width();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut rows: Vec<egui::Rect> = Vec::with_capacity(draft.members.len());
    let mut started: Option<usize> = None;
    let mut stopped = false;

    for index in 0..draft.members.len() {
        let view = editor.sequence_members.get(index);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, row_height), egui::Sense::click_and_drag());
        rows.push(rect);
        let is_selected = selected.contains(&index);
        let visuals = ui.visuals();
        if is_selected {
            ui.painter().rect_filled(rect, GROUP_CORNER_RADIUS, visuals.selection.bg_fill);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, GROUP_CORNER_RADIUS, visuals.widgets.hovered.bg_fill);
        }
        // A member that did not resolve says why in place of its name, so the
        // reason is the row rather than a second line beneath it - which is
        // what keeps every row the same height and the list a list.
        let color = match view {
            Some(view) if view.stale_pick => visuals.error_fg_color,
            Some(view) if view.unresolved.is_some() => visuals.warn_fg_color,
            _ if is_selected => visuals.strong_text_color(),
            _ => visuals.text_color(),
        };
        let cross = egui::Rect::from_center_size(egui::pos2(rect.right() - REMOVE_CROSS * 0.6, rect.center().y), egui::Vec2::splat(REMOVE_CROSS));
        let text_width = (cross.left() - rect.left() - 12.0).max(1.0);
        let galley = ui.painter().layout(member_label(view), font.clone(), color, text_width);
        let painter = ui.painter().with_clip_rect(rect);
        // The rows being carried are dimmed where they still are, so the list
        // shows what is moving as well as where it would land; so are the rows
        // the preview has already dug, which is the same statement about a
        // different thing - this row is not where the list currently stands.
        let dug = index < draft.preview;
        let opacity = if (carrying && is_selected) || dug { 0.4 } else { 1.0 };
        painter.galley(
            egui::pos2(rect.left() + 6.0, rect.center().y - galley.size().y * 0.5),
            galley,
            color.gamma_multiply(opacity),
        );
        // Registered after the row, so the cross is the topmost thing over its
        // own square and a click there removes rather than selects.
        if menu::close_cross(ui, cross, ui.id().with(("sequence_remove", index))).clicked() {
            edit = Some(ListEdit::Remove(index));
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            started = Some(index);
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            stopped = true;
        }
        // A press that never became a drag is a plain click. The command
        // modifier adds and removes one row at a time, so a row swept up by
        // mistake can be dropped without sweeping the run again.
        if response.clicked() {
            if ui.input(|input| input.modifiers.command) {
                if !selected.remove(&index) {
                    selected.insert(index);
                }
            } else {
                selected.clear();
                selected.insert(index);
            }
        }
    }

    // Which gesture the press begins is decided by the row it landed on, and
    // only here: a sweep and a carry are the same movement afterwards.
    if let Some(from) = started {
        let mode = if selected.contains(&from) {
            SequenceListDragMode::Carry
        } else {
            selected.clear();
            selected.insert(from);
            SequenceListDragMode::Sweep
        };
        drag = Some(SequenceListDrag { mode, from, to: from });
    }
    let pointer = ui.input(|input| input.pointer.interact_pos());
    if let Some(active) = drag.as_mut()
        && let Some(pos) = pointer
    {
        match active.mode {
            SequenceListDragMode::Sweep => {
                let row = row_at(pos.y, &rows);
                selected = (active.from.min(row)..=active.from.max(row)).collect();
            }
            SequenceListDragMode::Carry => active.to = drop_gap(pos.y, &rows),
        }
    }
    // Where the preview slider stands, drawn in the list as a playhead: the
    // rows above it are dug and dimmed, and the next block to be dug is the
    // one immediately below it. A block picked in the 3D pane lands here.
    let playhead = rows.get(draft.preview).map_or_else(|| rows[rows.len() - 1].bottom(), |rect| rect.top());
    let accent = ui.visuals().selection.stroke.color;
    ui.painter().line_segment(
        [egui::pos2(rows[0].left(), playhead), egui::pos2(rows[0].right(), playhead)],
        egui::Stroke::new(2.0, accent),
    );
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(rows[0].left(), playhead - PLAYHEAD_MARK),
            egui::pos2(rows[0].left() + PLAYHEAD_MARK, playhead),
            egui::pos2(rows[0].left(), playhead + PLAYHEAD_MARK),
        ],
        accent,
        egui::Stroke::NONE,
    ));
    if let Some(active) = drag.filter(|active| active.mode == SequenceListDragMode::Carry) {
        // Where the carried rows would land, drawn between the two rows it
        // would separate rather than over either of them - and in the text
        // colour, so the two markers in the list are never the same mark.
        let y = rows.get(active.to).map_or_else(|| rows[rows.len() - 1].bottom(), |rect| rect.top());
        let stroke = egui::Stroke::new(2.0, ui.visuals().strong_text_color());
        ui.painter().line_segment([egui::pos2(rows[0].left(), y), egui::pos2(rows[0].right(), y)], stroke);
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    if stopped {
        if let Some(active) = drag.filter(|active| active.mode == SequenceListDragMode::Carry) {
            let rows: Vec<usize> = selected.iter().copied().collect();
            // A carry that puts the rows back where they already were is not
            // an edit, and must not dirty the draft or cost an undo step.
            if !is_settled(&rows, active.to) {
                edit = Some(ListEdit::Move { rows, to: active.to });
            }
        }
        drag = None;
    }
    editor.sequence_list_drag = drag;
    (edit, selected)
}

/// Whether moving `rows` to the gap `to` would leave the order exactly as it
/// is: the gap sits inside, or immediately either side of, one unbroken run.
fn is_settled(rows: &[usize], to: usize) -> bool {
    let (Some(first), Some(last)) = (rows.first(), rows.last()) else {
        return true;
    };
    rows.len() == last - first + 1 && (*first..=last + 1).contains(&to)
}

/// The row a pointer at `y` is over, clamped to the ends of the list.
fn row_at(y: f32, rows: &[egui::Rect]) -> usize {
    rows.iter().position(|rect| y < rect.bottom()).unwrap_or(rows.len().saturating_sub(1))
}

/// The gap a pointer at `y` is nearest, `0..=rows.len()`: rows are split down
/// the middle, so the half of a row nearer a gap aims at that gap.
fn drop_gap(y: f32, rows: &[egui::Rect]) -> usize {
    rows.iter().position(|rect| y < rect.center().y).unwrap_or(rows.len())
}

/// The sequence preview: how far through *this bar's authored order* the 3D
/// view is showing, and nothing else.
///
/// Explicitly not a time axis. It consults no loader rate, no earliest start
/// and no other bar; the whole-schedule playback that does all three consumes
/// calculated execution segments and belongs to the dispatch stage.
///
/// It is also where a pick lands, which is why it is a control rather than a
/// readout: the pane shows the ground as it stands at this point in the
/// order, so the block clicked in it is the next one dug from here.
fn draw_preview_slider(ui: &mut egui::Ui, draft: &mut SequenceDraft, confirming: bool) -> bool {
    let total = draft.members.len();
    draft.preview = draft.preview.min(total);
    let before = draft.preview;
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tr!("sequence-editor-preview")).strong());
        let mut position = draft.preview;
        let width = (ui.available_width() - 220.0).max(120.0);
        // Previewing is not an edit of the order, but it is still an
        // interaction with a draft the discard question stands over, and the
        // question is the only interaction until it is answered.
        ui.add_enabled_ui(total > 0 && !confirming, |ui| {
            ui.spacing_mut().slider_width = width;
            ui.add(egui::Slider::new(&mut position, 0..=total).show_value(false));
        });
        draft.preview = position.min(total);
        ui.label(egui::RichText::new(tr!("sequence-editor-preview-at", dug = draft.preview.to_string(), total = total.to_string())).weak());
    });
    if draft.preview != before {
        // The display list is keyed on this, so moving the slider is what
        // re-tints the blocks rather than a rebuild every frame.
        return true;
    }
    false
}

/// Apply one list edit to the draft and to the per-run answers mirrored
/// alongside it, so the two never drift apart for a frame.
fn apply_list_edit(editor: &mut EditorState, draft: &mut SequenceDraft, edit: ListEdit) {
    match edit {
        ListEdit::Remove(index) if index < draft.members.len() => {
            draft.members.remove(index);
            if index < editor.sequence_members.len() {
                editor.sequence_members.remove(index);
            }
            // The row that moved up into this place is the one now selected;
            // past the end, nothing is, rather than the wrong thing.
            draft.selected = (!draft.members.is_empty()).then(|| index.min(draft.members.len() - 1)).into_iter().collect();
            draft.preview = draft.preview.min(draft.members.len());
        }
        ListEdit::Move { rows, to } => {
            let mut rows: Vec<usize> = rows.into_iter().filter(|index| *index < draft.members.len()).collect();
            rows.sort_unstable();
            rows.dedup();
            // Where the run lands once the rows above the gap have been taken
            // out from under it: the gap was counted in the list the user was
            // pointing at, which still held them.
            let landing = to.min(draft.members.len()) - rows.iter().filter(|index| **index < to).count();
            move_run(&mut draft.members, &rows, landing);
            // The mirror is this frame's answers in draft order, so it is
            // rearranged the same way rather than left to be rebuilt: for the
            // rest of this frame the two would otherwise describe different
            // members.
            if rows.iter().all(|index| *index < editor.sequence_members.len()) {
                move_run(&mut editor.sequence_members, &rows, landing);
            }
            draft.selected = (landing..landing + rows.len()).collect();
        }
        ListEdit::Remove(_) => {}
    }
}

/// Take the items at `rows` - ascending, in bounds - out of `items` and put
/// them back, in the same order, starting at `landing`.
fn move_run<T>(items: &mut Vec<T>, rows: &[usize], landing: usize) {
    let taken: Vec<T> = rows.iter().rev().map(|index| items.remove(*index)).collect();
    let landing = landing.min(items.len());
    items.splice(landing..landing, taken.into_iter().rev());
}
