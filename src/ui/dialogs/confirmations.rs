//! Destructive-action and unsaved-work confirmation dialogs.

use crate::{
    i18n::tr,
    ui::{
        state::{EditorState, RenameTarget, UiCommand, UiProjectView},
        widgets::menu::{self, DragableMenu, MenuButton},
    },
};

/// Draw the "Save before quit?" confirmation dialog.
pub(crate) fn draw_exit_confirm_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, _editor: &mut EditorState) {
    let mut open = true;
    let title = tr!("confirmations-exit-unsaved-changes");
    DragableMenu::new("exit_confirmation_dialog", title).open(&mut open).min_width(360.0).show(ui.ctx(), |ui| {
        #[cfg(not(target_arch = "wasm32"))]
        ui.label(tr!("confirmations-save-modified-project-before-exiting"));
        #[cfg(target_arch = "wasm32")]
        ui.label(tr!("confirmations-save-to-browser-before-exit"));
        menu::menu_actions(ui, |ui| {
            if ui.add(MenuButton::new(tr!("common-save-exit")).primary()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                commands.push(UiCommand::SaveAndExit);
            }
            // Red, and mouse-only: Enter must never be the key that throws
            // away unsaved changes.
            if ui.add(MenuButton::new(tr!("common-exit-without-saving")).danger()).clicked() {
                commands.push(UiCommand::ExitWithoutSaving);
            }
            if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                commands.push(UiCommand::CancelExit);
            }
        });
    });
    if !open {
        commands.push(UiCommand::CancelExit);
    }
}

/// Draw the confirmation required before New/Open replaces the one active
/// project. The pending action itself stays in the application core.
pub(crate) fn draw_replace_project_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState) {
    if !editor.replace_project_confirm_open {
        return;
    }
    let mut open = true;
    DragableMenu::new("replace_project_confirmation_dialog", tr!("confirmations-replace-project-unsaved-changes"))
        .open(&mut open)
        .min_width(340.0)
        .max_width(340.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("confirmations-save-changes-current-project-before"));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("confirmations-save")).primary()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                    commands.push(UiCommand::SaveAndReplaceProject);
                }
                if ui.add(MenuButton::new(tr!("confirmations-discard")).danger()).clicked() {
                    commands.push(UiCommand::DiscardAndReplaceProject);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    commands.push(UiCommand::CancelProjectReplacement);
                }
            });
        });
    if !open {
        commands.push(UiCommand::CancelProjectReplacement);
    }
}

pub(crate) fn draw_lossy_save_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState, project: &UiProjectView) {
    if !editor.lossy_save_confirm_open {
        return;
    }
    let warnings = project.projects.first().map(|entry| entry.lossy_save_warnings.as_slice()).unwrap_or_default();
    let mut open = true;
    DragableMenu::new("lossy_save_confirmation_dialog", tr!("common-confirm-omf-rewrite"))
        .open(&mut open)
        .min_width(420.0)
        .max_width(520.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("confirmations-incline-design-cannot-reproduce-all"));
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                for warning in warnings {
                    ui.label(format!("• {warning}"));
                }
            });
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("confirmations-save-anyway")).primary()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                    commands.push(UiCommand::ConfirmLossyProjectSave);
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    commands.push(UiCommand::CancelLossyProjectSave);
                }
            });
        });
    if !open {
        commands.push(UiCommand::CancelLossyProjectSave);
    }
}

/// Draw the delete-selection confirmation dialog shown when Delete/Backspace is pressed.
pub(crate) fn draw_delete_confirm_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState) {
    if !editor.delete_confirm_open {
        return;
    }
    let count = editor.selected_handles.iter().filter(|h| matches!(h, crate::model::SceneEntityId::Object(_))).count();
    let mut open = true;
    DragableMenu::new("delete_objects_confirmation_dialog", tr!("confirmations-delete-objects"))
        .open(&mut open)
        .min_width(240.0)
        .max_width(240.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("confirm-delete-count", count = count));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("confirmations-delete")).danger()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                    commands.push(UiCommand::ConfirmDeleteSelection);
                    editor.delete_confirm_open = false;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.delete_confirm_open = false;
                }
            });
        });
    if !open {
        editor.delete_confirm_open = false;
    }
}

/// Draw the confirmation dialog shown before deleting a layer and its objects.
pub(crate) fn draw_delete_layer_confirm_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState) {
    let Some((layer_id, name)) = editor.pending_delete_layer.clone() else {
        return;
    };
    let mut open = true;
    DragableMenu::new("delete_layer_confirmation_dialog", tr!("common-delete-layer"))
        .open(&mut open)
        .min_width(280.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("confirm-delete-layer", name = name.clone()));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("common-delete-layer")).danger()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                    commands.push(UiCommand::DeleteLayer(layer_id));
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.pending_delete_layer = None;
                }
            });
        });
    if !open {
        editor.pending_delete_layer = None;
    }
}

/// Draw the confirmation dialog shown before deleting a non-layer explorer
/// item (triangulation, raster, point cloud, block model, or drill hole dataset).
pub(crate) fn draw_delete_item_confirm_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState) {
    let Some((target, name)) = editor.pending_delete_item.clone() else {
        return;
    };
    // One Fluent message carries the whole "Delete <kind>" phrase so each
    // language owns the word order; the confirm button repeats that title.
    let title = tr!("dialog-delete-title", kind = target.kind_label());
    let mut open = true;
    DragableMenu::new("delete_item_confirmation_dialog", title.clone())
        .open(&mut open)
        .min_width(280.0)
        .show(ui.ctx(), |ui| {
            // A collection takes everything in it along, which the plain
            // sentence would not say.
            ui.label(match target {
                RenameTarget::Folder(..) => tr!("dialog-delete-collection-confirm", name = name.clone()),
                _ => tr!("dialog-delete-confirm", name = name.clone()),
            });
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(title.clone()).danger()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                    commands.push(target.remove_command());
                    editor.pending_delete_item = None;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.pending_delete_item = None;
                }
            });
        });
    if !open {
        editor.pending_delete_item = None;
    }
}

/// Draw the confirmation dialog shown before deleting several explorer rows
/// at once, from a right-click on a multi-row selection.
pub(crate) fn draw_delete_rows_confirm_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState) {
    let Some(rows) = editor.pending_delete_rows.clone() else {
        return;
    };
    let title = tr!("explorer-delete-selected", count = rows.len().to_string());
    let mut open = true;
    DragableMenu::new("delete_rows_confirmation_dialog", title.clone())
        .open(&mut open)
        .min_width(280.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("dialog-delete-rows-confirm", count = rows.len().to_string()));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(title.clone()).danger()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                    commands.extend(rows.iter().cloned());
                    editor.pending_delete_rows = None;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.pending_delete_rows = None;
                }
            });
        });
    if !open {
        editor.pending_delete_rows = None;
    }
}

/// Draw the confirmation dialog shown before deleting a product from the
/// Drill & Blast palette.
///
/// The palette is not part of the undo history - it lives in the config file,
/// not in the document - so the only way back from a deletion is to add the
/// product again. That is what earns it the same confirmation the explorer's
/// destructive deletes get.
pub(crate) fn draw_delete_delay_product_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState) {
    let Some((id, name)) = editor.pending_delete_delay_product.clone() else {
        return;
    };
    let title = tr!("dialog-delete-title", kind = tr!("confirmations-product"));
    let mut open = true;
    DragableMenu::new("delete_delay_product_confirmation_dialog", title.clone())
        .open(&mut open)
        .min_width(280.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("confirm-delete-product", name = name.clone()));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(title.clone()).danger()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                    commands.push(UiCommand::DeleteDelayProduct(id));
                    editor.pending_delete_delay_product = None;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.pending_delete_delay_product = None;
                }
            });
        });
    if !open {
        editor.pending_delete_delay_product = None;
    }
}

/// Draw the confirmation dialog shown before closing a dirty project.
pub(crate) fn draw_close_project_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState, project: &UiProjectView) {
    let Some(runtime_id) = editor.pending_close_project else {
        return;
    };
    let name = project
        .projects
        .iter()
        .find(|entry| entry.runtime_id == runtime_id)
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| tr!("confirmations-project"));
    let mut open = true;
    let removing = editor.remove_project_after_close;
    let title = if removing {
        tr!("confirmations-remove-project-unsaved-changes")
    } else {
        tr!("confirmations-close-project-unsaved-changes")
    };
    DragableMenu::new("close_project_confirmation_dialog", title)
        .open(&mut open)
        .min_width(320.0)
        .max_width(320.0)
        .show(ui.ctx(), |ui| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                ui.label(if removing {
                    tr!("confirmations-save-changes-name-before-removing", name = name.to_string())
                } else {
                    tr!("confirmations-save-changes-name-before-closing", name = name.to_string())
                });
                menu::menu_actions(ui, |ui| {
                    if ui
                        .add(MenuButton::new(if removing { tr!("confirmations-save-remove") } else { tr!("confirmations-save-close") }).primary())
                        .clicked()
                        || menu::dialog_confirm_pressed(ui.ctx())
                    {
                        commands.push(UiCommand::SaveAndCloseProject(runtime_id));
                    }
                    if ui
                        .add(
                            MenuButton::new(if removing {
                                tr!("confirmations-remove-without-saving")
                            } else {
                                tr!("confirmations-close-without-saving")
                            })
                            .danger(),
                        )
                        .clicked()
                    {
                        commands.push(UiCommand::CloseProjectForce(runtime_id));
                    }
                    if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                        commands.push(UiCommand::CancelCloseProject);
                    }
                });
            }
            #[cfg(target_arch = "wasm32")]
            {
                ui.label(if removing {
                    tr!("confirmations-remove-name-delete-its-browser", name = name.to_string())
                } else {
                    tr!("confirmations-save-changes-name-before-closing", name = name.to_string())
                });
                menu::menu_actions(ui, |ui| {
                    if removing {
                        // Removing always discards, so it is deliberately not bound to Enter.
                        if ui.add(MenuButton::new(tr!("common-remove-project")).danger()).clicked() {
                            commands.push(UiCommand::CloseProjectForce(runtime_id));
                        }
                    } else {
                        if ui.add(MenuButton::new(tr!("confirmations-save-close")).primary()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                            commands.push(UiCommand::SaveAndCloseProject(runtime_id));
                        }
                        if ui.add(MenuButton::new(tr!("confirmations-close-without-saving")).danger()).clicked() {
                            commands.push(UiCommand::CloseProjectForce(runtime_id));
                        }
                    }
                    if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                        commands.push(UiCommand::CancelCloseProject);
                    }
                });
            }
        });
    if !open {
        commands.push(UiCommand::CancelCloseProject);
    }
}

/// Draw the confirmation dialog shown before discarding a dirty project's
/// changes (reverting to the last saved version on disk).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn draw_discard_project_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState, project: &UiProjectView) {
    let Some(runtime_id) = editor.pending_discard_project else {
        return;
    };
    let name = project
        .projects
        .iter()
        .find(|entry| entry.runtime_id == runtime_id)
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| tr!("confirmations-project"));
    let mut open = true;
    DragableMenu::new("discard_project_confirmation_dialog", tr!("confirmations-discard-changes"))
        .open(&mut open)
        .min_width(320.0)
        .max_width(320.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("confirmations-discard-all-unsaved-changes-name", name = name.to_string()));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("confirmations-discard-changes")).danger()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                    commands.push(UiCommand::DiscardProjectChanges(runtime_id));
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.pending_discard_project = None;
                }
            });
        });
    if !open {
        editor.pending_discard_project = None;
    }
}

/// Confirm restoring just one dirty layer from its saved project while retaining
/// unsaved work on the other layers.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn draw_discard_layer_dialog(ui: &mut egui::Ui, commands: &mut Vec<UiCommand>, editor: &mut EditorState) {
    let Some((layer_id, name)) = editor.pending_discard_layer.clone() else {
        return;
    };
    let mut open = true;
    DragableMenu::new("discard_layer_confirmation_dialog", tr!("common-discard-layer-changes"))
        .open(&mut open)
        .min_width(320.0)
        .max_width(320.0)
        .show(ui.ctx(), |ui| {
            ui.label(tr!("confirmations-discard-all-unsaved-changes-layer", name = name.to_string()));
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("confirmations-discard-changes")).danger()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                    commands.push(UiCommand::DiscardLayerChanges(layer_id));
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    editor.pending_discard_layer = None;
                }
            });
        });
    if !open {
        editor.pending_discard_layer = None;
    }
}
