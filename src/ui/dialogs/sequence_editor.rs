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

use crate::{
    i18n::{tr, tr_format},
    model::schedule::SchedulePlan,
    ui::{
        EditorState,
        state::{ScheduleEdit, SequenceDraft, UiCommand},
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
/// Width of the ordered-list column.
const LIST_WIDTH: f32 = 300.0;

/// One edit of the ordered list, collected during the pass and applied after
/// it so the list is never mutated while it is being drawn.
#[derive(Clone, Copy, PartialEq)]
enum ListEdit {
    Remove(usize),
    Move { from: usize, to: usize },
}

pub(crate) fn draw_sequence_editor(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
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
        Some(bar) => tr!("sequence-editor-title", bar = bar.name().to_owned()),
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

    let menu = DragableMenu::new("sequence_editor", title).min_width(MIN_SIZE.x).fixed_size(MIN_SIZE);
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
        let body_height = (available.height() - ui.spacing().interact_size.y * 3.0).max(160.0);
        ui.horizontal_top(|ui| {
            let view_width = (available.width() - LIST_WIDTH - ui.spacing().item_spacing.x).max(200.0);
            ui.allocate_ui(egui::vec2(view_width, body_height), |ui| {
                changed |= draw_view(ui, editor, &mut draft, session, confirming);
            });
            ui.allocate_ui(egui::vec2(LIST_WIDTH, body_height), |ui| {
                let (list_edit, selection) = draw_order_list(ui, editor, &draft, confirming);
                edit = list_edit;
                if selection != draft.selected {
                    draft.selected = selection;
                    changed = true;
                }
            });
        });
        changed |= draw_preview_slider(ui, editor, &mut draft, confirming);
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
    let name = plan.bar(bar).map(|bar| bar.name().to_owned()).unwrap_or_default();
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
    let caption_height = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
    let image_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), (rect.bottom() - caption_height).max(rect.top())));
    if !image_rect.is_positive() {
        return false;
    }
    let has_run = editor.sequence_unavailable.is_none();
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
    // A click that did not drag picks the block under it. The click is
    // addressed the moment it is made - this draft instance, this run, this
    // image - because it is resolved frames later, and an unaddressed click
    // would be answered as whatever the renderer is looking at by then. The UV
    // is a fraction of the image, not pane pixels: the renderer sizes its
    // target within limits of its own, so a pane outside them is drawn at one
    // size and would otherwise be picked against another.
    if response.clicked()
        && has_run
        && let Some(pointer) = response.interact_pointer_pos()
    {
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

    draw_order_numbers(ui, editor, image_rect);

    let caption_rect = egui::Rect::from_min_max(egui::pos2(rect.left() + 8.0, image_rect.bottom()), rect.max);
    let caption = match &editor.sequence_unavailable {
        Some(reason) => tr!("sequence-editor-no-run", reason = reason.clone()),
        None => match editor.sequence_generation {
            Some(generation) => format!(
                "{} · {}",
                tr!("sequence-editor-generation", generation = generation.to_string()),
                tr!("sequence-editor-hint")
            ),
            None => tr!("sequence-editor-hint"),
        },
    };
    ui.scope_builder(egui::UiBuilder::new().max_rect(caption_rect), |ui| {
        ui.add(egui::Label::new(egui::RichText::new(caption).weak().small()).truncate());
    });
    changed
}

/// Number each member over the block it names.
///
/// The positions come from the renderer, projected through the camera the
/// image was actually drawn with, so a number cannot drift from its block. A
/// draft that has moved since that render leaves them off for one frame rather
/// than drawing them in the wrong places.
fn draw_order_numbers(ui: &egui::Ui, editor: &EditorState, image_rect: egui::Rect) {
    if editor.sequence_label_uv.len() != editor.sequence_members.len() {
        return;
    }
    let painter = ui.painter().with_clip_rect(image_rect);
    let visuals = ui.visuals().clone();
    for (index, uv) in editor.sequence_label_uv.iter().enumerate() {
        let Some(uv) = uv else { continue };
        let center = image_rect.min + egui::vec2(uv[0] * image_rect.width(), uv[1] * image_rect.height());
        if !image_rect.contains(center) {
            continue;
        }
        let text = (index + 1).to_string();
        let galley = painter.layout_no_wrap(text, egui::TextStyle::Small.resolve(ui.style()), visuals.strong_text_color());
        let chip = egui::Rect::from_center_size(center, galley.size() + egui::vec2(10.0, 4.0));
        painter.rect_filled(chip, GROUP_CORNER_RADIUS, visuals.extreme_bg_color.gamma_multiply(0.85));
        painter.rect_stroke(chip, GROUP_CORNER_RADIUS, visuals.widgets.active.bg_stroke, egui::StrokeKind::Inside);
        painter.galley(chip.center() - galley.size() * 0.5, galley, visuals.strong_text_color());
    }
}

/// The ordered list: what is in the dig order, in the order it will be dug,
/// with what the current run says about each place in it.
///
/// An unresolved member keeps its place, keeps its number and stays removable.
/// Hiding it is the one thing the identity layer exists to prevent: the
/// reference is the user's own work, and only they can decide what it should
/// become.
fn draw_order_list(ui: &mut egui::Ui, editor: &EditorState, draft: &SequenceDraft, confirming: bool) -> (Option<ListEdit>, Option<usize>) {
    let mut edit = None;
    let mut selected = draft.selected;
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
            ui.label(egui::RichText::new(tr!("sequence-editor-empty-order")).weak());
            return;
        }
        // Rows and their buttons are one interaction surface, disabled whole
        // while the discard question is up: neither a selection nor a remove
        // or reorder may reach a draft the user is being asked whether to keep.
        ui.add_enabled_ui(!confirming, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for index in 0..draft.members.len() {
                    let view = editor.sequence_members.get(index);
                    let label = view.and_then(|view| view.name.clone()).unwrap_or_else(|| tr!("sequence-editor-unknown-block"));
                    let is_selected = selected == Some(index);
                    let row = ui.selectable_label(is_selected, format!("{}. {label}", index + 1));
                    if row.clicked() {
                        selected = Some(index);
                    }
                    if let Some(view) = view {
                        ui.indent(("sequence_member", index), |ui| {
                            if let Some(solid) = &view.solid_name {
                                ui.label(egui::RichText::new(solid.clone()).weak().small());
                            }
                            if let Some(reason) = &view.unresolved {
                                let color = if view.stale_pick { ui.visuals().error_fg_color } else { ui.visuals().warn_fg_color };
                                ui.label(egui::RichText::new(reason.clone()).small().color(color));
                            }
                            // A tonnage that is not there is not zero: a member
                            // with no figure says nothing rather than saying "0 t".
                            if let Some(tonnes) = view.tonnes {
                                ui.label(egui::RichText::new(tr_format!(literal = "%tonnes% t", tonnes = format!("{tonnes:.0}"))).weak().small());
                            }
                            if view.is_new {
                                ui.label(egui::RichText::new(tr!("sequence-editor-new-block")).small());
                            }
                        });
                    }
                    if is_selected {
                        ui.horizontal(|ui| {
                            if ui.add(MenuButton::new(tr!("sequence-editor-move-up")).enabled(index > 0)).clicked() {
                                edit = Some(ListEdit::Move { from: index, to: index - 1 });
                            }
                            if ui.add(MenuButton::new(tr!("sequence-editor-move-down")).enabled(index + 1 < draft.members.len())).clicked() {
                                edit = Some(ListEdit::Move { from: index, to: index + 1 });
                            }
                            if ui.add(MenuButton::new(tr!("sequence-editor-remove")).danger()).clicked() {
                                edit = Some(ListEdit::Remove(index));
                            }
                        });
                    }
                }
            });
        });
    });
    (edit, selected)
}

/// The order preview: how far through *this bar's authored order* the 3D view
/// is showing, and nothing else.
///
/// Explicitly not a time axis. It consults no loader rate, no earliest start
/// and no other bar; the whole-schedule playback that does all three consumes
/// calculated execution segments and belongs to the dispatch stage. Saying so
/// under the slider is what keeps the two from being read as one control.
fn draw_preview_slider(ui: &mut egui::Ui, editor: &mut EditorState, draft: &mut SequenceDraft, confirming: bool) -> bool {
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
    ui.label(egui::RichText::new(tr!("sequence-editor-preview-note")).weak().small());
    if draft.preview != before {
        // The display list is keyed on this, so moving the slider is what
        // re-tints the blocks rather than a rebuild every frame.
        editor.sequence_label_uv.clear();
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
            editor.sequence_label_uv.clear();
            // The row that moved up into this place is the one now selected;
            // past the end, nothing is, rather than the wrong thing.
            draft.selected = (!draft.members.is_empty()).then(|| index.min(draft.members.len() - 1));
            draft.preview = draft.preview.min(draft.members.len());
        }
        ListEdit::Move { from, to } if from < draft.members.len() && to < draft.members.len() => {
            let member = draft.members.remove(from);
            draft.members.insert(to, member);
            if from < editor.sequence_members.len() && to < editor.sequence_members.len() {
                let view = editor.sequence_members.remove(from);
                editor.sequence_members.insert(to, view);
            }
            editor.sequence_label_uv.clear();
            draft.selected = Some(to);
        }
        _ => {}
    }
}
