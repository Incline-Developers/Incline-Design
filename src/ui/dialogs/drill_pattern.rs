//! Interactive Drill & Blast pattern creation.

use crate::{
    i18n::tr,
    model::{Document, Object, ObjectId, drill_hole::DrillPatternLayout},
    ui::{
        state::{EditorState, UiCommand},
        widgets::menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldCombo, MenuFieldF64, MenuFieldText},
    },
};

const DIALOG_MIN_WIDTH: f32 = 410.0;
const PICK_BUTTON_WIDTH: f32 = 64.0;

/// Everything the generated collars depend on. Compared each frame so the
/// pattern is rebuilt on a real change rather than on every repaint - filling
/// a densely tessellated boundary is far too expensive to redo blind.
#[derive(Clone, PartialEq)]
pub(crate) struct PatternPreviewKey {
    boundary: ObjectId,
    boundary_revision: u64,
    burden: f64,
    spacing: f64,
    rotation_deg: f64,
    offset_x: f64,
    offset_y: f64,
    layout: DrillPatternLayout,
}

fn refresh_preview(editor: &mut EditorState, document: &Document) -> bool {
    if let Some(id) = editor.drill_pattern_boundary_id
        && document.get_object(id).is_none_or(|object| !object.encloses_area())
    {
        editor.drill_pattern_boundary_id = None;
        editor.drill_pattern_boundary_name.clear();
        editor.tool_highlight_id = None;
    }

    let key = editor.drill_pattern_boundary_id.map(|boundary| PatternPreviewKey {
        boundary,
        boundary_revision: document.object_revision(boundary),
        burden: editor.drill_pattern_burden,
        spacing: editor.drill_pattern_spacing,
        rotation_deg: editor.drill_pattern_rotation_deg,
        offset_x: editor.drill_pattern_offset_x,
        offset_y: editor.drill_pattern_offset_y,
        layout: editor.drill_pattern_layout,
    });
    let mut changed = false;
    if key.is_none() || editor.drill_pattern_preview_key != key {
        let result = editor
            .drill_pattern_boundary_id
            .and_then(|id| document.get_object(id))
            .and_then(Object::closed_boundary)
            .ok_or_else(|| "Pick a closed polyline or circle to define the blast shape".to_owned())
            .and_then(|boundary| {
                crate::model::drill_hole::generate_pattern_collars(
                    &boundary,
                    editor.drill_pattern_burden,
                    editor.drill_pattern_spacing,
                    editor.drill_pattern_rotation_deg,
                    glam::DVec2::new(editor.drill_pattern_offset_x, editor.drill_pattern_offset_y),
                    editor.drill_pattern_layout,
                    crate::model::drill_hole::MAX_PATTERN_HOLES,
                )
            });

        let (collars, error) = match result {
            Ok(collars) => (collars, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        changed = editor.drill_pattern_preview_collars != collars || editor.drill_pattern_preview_error != error;
        editor.drill_pattern_preview_collars = collars;
        editor.drill_pattern_preview_error = error;
        editor.drill_pattern_preview_key = key;
    }

    let diameter = editor.drill_pattern_diameter_mm / 1_000.0;
    changed |= editor.drill_pattern_preview_depth != editor.drill_pattern_depth || editor.drill_pattern_preview_diameter != diameter;
    editor.drill_pattern_preview_depth = editor.drill_pattern_depth;
    editor.drill_pattern_preview_diameter = diameter;
    changed
}

/// Draw the movable pattern builder and keep its world-space preview current.
/// Returns whether overlay geometry changed or needs to be removed.
pub(crate) fn draw_drill_pattern_dialog(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) -> bool {
    if !editor.drill_pattern_open {
        return false;
    }

    let mut geometry_dirty = refresh_preview(editor, document);
    let mut open = true;
    let mut close = false;
    let boundary_label = if editor.drill_pattern_boundary_id.is_some() {
        editor.drill_pattern_boundary_name.clone()
    } else {
        tr!("drill-pattern-none-picked")
    };
    let layout_label = editor.drill_pattern_layout.label();

    DragableMenu::new("drill_pattern_dialog", tr!("common-create-drill-pattern"))
        .open(&mut open)
        .min_width(DIALOG_MIN_WIDTH)
        .max_width(440.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui.ctx(), |ui| {
            menu::menu_note(ui, tr!("drill-pattern-choose-closed-blast-boundary-then"));
            ui.add_space(6.0);

            MenuField::new(tr!("drill-pattern-blast-shape"))
                .help_text(tr!("drill-pattern-closed-design-polyline-whose-xy"))
                .show(ui, |ui, row_height, column_width| {
                    let button_width = PICK_BUTTON_WIDTH;
                    let label_width = (column_width - ui.spacing().item_spacing.x - button_width).max(80.0);
                    ui.allocate_ui_with_layout(egui::vec2(column_width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let mut display = boundary_label.clone();
                        ui.add_sized([label_width, row_height], egui::TextEdit::singleline(&mut display).interactive(false))
                            .on_hover_text(&boundary_label);
                        let button_text = if editor.drill_pattern_awaiting_shape_pick {
                            tr!("common-cancel")
                        } else {
                            tr!("drill-pattern-pick")
                        };
                        if ui.add(MenuButton::new(button_text).min_width(button_width)).clicked() {
                            if editor.drill_pattern_awaiting_shape_pick {
                                editor.drill_pattern_awaiting_shape_pick = false;
                                editor.viewport_pick_hover_label = None;
                                editor.tool_highlight_id = editor.drill_pattern_boundary_id;
                                geometry_dirty = true;
                            } else {
                                commands.push(UiCommand::BeginDrillPatternShapePick);
                            }
                        }
                    })
                    .response
                });

            if editor.drill_pattern_awaiting_shape_pick {
                let status = editor
                    .viewport_pick_hover_label
                    .clone()
                    .unwrap_or_else(|| tr!("drill-pattern-move-over-closed-polyline-then"));
                ui.add_space(4.0);
                menu::menu_note(ui, status);
            }

            ui.add_space(4.0);
            MenuFieldF64::new(tr!("drill-pattern-burden"), &mut editor.drill_pattern_burden, 0.01..=1_000_000.0)
                .help_text(tr!("drill-pattern-spacing-help"))
                .speed(0.1)
                .suffix(format!(" {}", tr!("common-m")))
                .show(ui);
            MenuFieldF64::new(tr!("drill-pattern-spacing"), &mut editor.drill_pattern_spacing, 0.01..=1_000_000.0)
                .help_text(tr!("drill-pattern-distance-between-holes-along-each"))
                .speed(0.1)
                .suffix(format!(" {}", tr!("common-m")))
                .show(ui);
            MenuFieldF64::new(tr!("drill-pattern-rotation"), &mut editor.drill_pattern_rotation_deg, -360.0..=360.0)
                .help_text(tr!("drill-pattern-rotation-help", axis = crate::model::survey::axis_name(0).to_string()))
                .speed(1.0)
                .suffix("°")
                .show(ui);
            MenuFieldF64::new(
                tr!("drill-pattern-axis-offset", axis = crate::model::survey::axis_name(0).to_string()),
                &mut editor.drill_pattern_offset_x,
                -1_000_000.0..=1_000_000.0,
            )
            .help_text(tr!("drill-pattern-shift-pattern-grid-along-global", axis = crate::model::survey::axis_name(0).to_string()))
            .speed(0.1)
            .suffix(format!(" {}", tr!("common-m")))
            .show(ui);
            MenuFieldF64::new(
                tr!("drill-pattern-axis-offset", axis = crate::model::survey::axis_name(1).to_string()),
                &mut editor.drill_pattern_offset_y,
                -1_000_000.0..=1_000_000.0,
            )
            .help_text(tr!("drill-pattern-shift-pattern-grid-along-global", axis = crate::model::survey::axis_name(1).to_string()))
            .speed(0.1)
            .suffix(format!(" {}", tr!("common-m")))
            .show(ui);
            MenuFieldCombo::new(
                "drill_pattern_layout",
                tr!("drill-pattern-arrangement"),
                &mut editor.drill_pattern_layout,
                layout_label,
                crate::model::drill_hole::DrillPatternLayout::ALL.map(|layout| (layout, layout.label().into())),
            )
            .help_text(tr!("drill-pattern-staggered-offsets-every-second-row"))
            .show(ui);
            MenuFieldF64::new(tr!("drill-pattern-hole-diameter"), &mut editor.drill_pattern_diameter_mm, 25.0..=1_000.0)
                .help_text(tr!("drill-pattern-diameter-help"))
                .speed(1.0)
                .suffix(" mm")
                .show(ui);
            MenuFieldF64::new(tr!("drill-pattern-hole-depth"), &mut editor.drill_pattern_depth, 0.01..=100_000.0)
                .help_text(tr!("drill-pattern-vertical-depth-below-each-collar"))
                .speed(0.5)
                .suffix(format!(" {}", tr!("common-m")))
                .show(ui);
            MenuFieldText::new(tr!("drill-pattern-pattern-name"), &mut editor.drill_pattern_name)
                .help_text(tr!("drill-pattern-name-help"))
                .hint_text(tr!("drill-pattern-name-hint"))
                .show(ui);

            ui.add_space(6.0);
            if let Some(error) = &editor.drill_pattern_preview_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            } else {
                let count = editor.drill_pattern_preview_collars.len();
                menu::menu_note(
                    ui,
                    tr!(
                        "drill-pattern-preview-count-hole-s-diameter",
                        count = count.to_string(),
                        diameter = format!("{:.0}", editor.drill_pattern_diameter_mm),
                        depth = format!("{:.2}", editor.drill_pattern_depth)
                    ),
                );
            }

            let can_create = !editor.drill_pattern_preview_collars.is_empty()
                && editor.drill_pattern_diameter_mm.is_finite()
                && editor.drill_pattern_diameter_mm > 0.0
                && editor.drill_pattern_depth.is_finite()
                && editor.drill_pattern_depth > 0.0
                && !editor.drill_pattern_name.trim().is_empty()
                && !editor.drill_pattern_awaiting_shape_pick;
            menu::menu_actions(ui, |ui| {
                let confirm = menu::dialog_confirm_pressed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("common-create")).primary().enabled(can_create)).clicked() || (confirm && can_create)) && can_create {
                    commands.push(UiCommand::CreateDrillPattern {
                        name: editor.drill_pattern_name.trim().to_owned(),
                        collars: editor.drill_pattern_preview_collars.clone(),
                        depth: editor.drill_pattern_depth,
                        diameter: editor.drill_pattern_diameter_mm / 1_000.0,
                    });
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });

    if close || !open {
        editor.close_drill_pattern();
        return true;
    }
    geometry_dirty |= refresh_preview(editor, document);
    geometry_dirty
}
