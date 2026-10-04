//! Bounded, spreadsheet-style surfaces used by section layouts.
//!
//! [`DataGrid`] is a titled pane holding a scrollable column of rows with a
//! selector gutter; [`PropertyTable`] is the two-column key/value editor that
//! sits beside it. Both paint their own fill, border and row rules so callers
//! only supply content.

use super::explorer::row_height;
use crate::ui::{fonts::bold, unthemed_icon};

/// Height of the bold section-title strip above a grid's rows.
const TITLE_STRIP: f32 = 38.0;
/// Width of the left selector gutter in a [`DataGrid`] row.
const GUTTER: f32 = 24.0;
/// Fraction of a [`PropertyTable`] row given to the key column.
const KEY_FRACTION: f32 = 0.54;
/// Extra height a [`grid_columns_row`] takes over a plain row, so a control
/// in one of its cells sits clear of the rules above and below it.
const COLUMN_ROW_EXTRA: f32 = 6.0;
/// Tallest a [`grid_cell_combo`]'s list grows before it scrolls.
const COMBO_LIST_HEIGHT: f32 = 480.0;
/// Indent given to a row that describes the row above it rather than
/// standing on its own.
const SUB_INDENT: f32 = 18.0;

fn grid_row_height(ui: &egui::Ui) -> f32 {
    row_height(ui) + 3.0
}

/// Height a [`PropertyTable`] needs to show its title strip plus `rows` rows
/// without clipping the last one. Its rows stand as tall as a
/// [`grid_columns_row`]; a header row is shorter, so one counted here as a
/// row leaves a little to spare rather than clipping.
pub(crate) fn property_table_height(ui: &egui::Ui, rows: usize) -> f32 {
    TITLE_STRIP + rows as f32 * (grid_row_height(ui) + COLUMN_ROW_EXTRA)
}

/// Paint the tree-stripe background, title strip and border shared by both surfaces,
/// then run `content` clipped to `rect` with zero vertical row spacing.
fn framed_pane<R>(ui: &mut egui::Ui, id: &str, rect: egui::Rect, title: &str, extras: TitleExtras<'_>, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut out = None;
    ui.scope_builder(egui::UiBuilder::new().id_salt(id).max_rect(rect), |ui| {
        ui.set_clip_rect(ui.clip_rect().intersect(rect));
        ui.set_min_size(rect.size());
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.painter().rect_filled(rect, 0.0, super::tree_row_colors(ui).1);
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), TITLE_STRIP), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            ui.label(bold(title));
            if let Some(detail) = extras.detail {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(detail).color(ui.visuals().weak_text_color()));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                if let Some((tooltip, clicked)) = extras.add {
                    *clicked |= add_button(ui, (id, "title_add"), tooltip).clicked();
                }
                if let Some((label, value)) = extras.toggle {
                    ui.add_space(4.0);
                    ui.add(super::toggle::Toggle::new(value, label));
                }
                if let Some((label, tooltip, clicked)) = extras.action {
                    ui.add_space(4.0);
                    *clicked |= ui.add(super::menu::MenuButton::new(label)).on_hover_text(tooltip).clicked();
                }
            });
        });
        out = Some(content(ui));
        ui.painter().rect_stroke(rect, 0.0, ui.visuals().widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
    });
    out.expect("scope_builder always runs its body")
}

/// What a pane's title strip carries besides its title.
#[derive(Default)]
struct TitleExtras<'a> {
    /// Weak text after the title: a count, a size.
    detail: Option<&'a str>,
    /// A + at the right, with its tooltip and where its click is reported.
    add: Option<(&'a str, &'a mut bool)>,
    /// A switch at the right, for a setting of the whole pane.
    toggle: Option<(&'a str, &'a mut bool)>,
    /// A button at the right that acts on the whole pane, with its tooltip.
    action: Option<(&'a str, &'a str, &'a mut bool)>,
}

/// One row in a [`DataGrid`].
pub(crate) struct GridRow<'a> {
    label: &'a str,
    selected: bool,
    header: bool,
    error: Option<&'a str>,
}

impl<'a> GridRow<'a> {
    /// A selectable body row.
    pub(crate) fn new(label: &'a str) -> Self {
        Self {
            label,
            selected: false,
            header: false,
            error: None,
        }
    }

    /// The non-interactive column-header row.
    pub(crate) fn header(label: &'a str) -> Self {
        Self {
            label,
            selected: false,
            header: true,
            error: None,
        }
    }

    pub(crate) fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Show a red error badge at the row's right edge; the message is its tooltip.
    #[allow(dead_code)] // Feature-facing: item rows will carry validation state.
    pub(crate) fn error(mut self, message: Option<&'a str>) -> Self {
        self.error = message;
        self
    }
}

/// Draw a single spreadsheet-style row into the current grid body.
pub(crate) fn grid_row(ui: &mut egui::Ui, row: GridRow<'_>) -> egui::Response {
    let GridRow { label, selected, header, error } = row;
    let height = grid_row_height(ui);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), if header { egui::Sense::hover() } else { egui::Sense::click() });
    let visuals = ui.visuals();
    let fill = if header {
        visuals.widgets.noninteractive.bg_fill
    } else if selected {
        visuals.selection.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        super::tree_row_colors(ui).1
    };
    let stroke = visuals.widgets.noninteractive.bg_stroke;
    let text_color = if selected { visuals.selection.stroke.color } else { visuals.text_color() };
    let gutter = rect.left() + GUTTER;
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().line_segment([egui::pos2(gutter, rect.top()), egui::pos2(gutter, rect.bottom())], stroke);
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
    if selected && !header {
        let center = egui::pos2(rect.left() + 12.0, rect.center().y);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![center + egui::vec2(-3.0, -4.0), center + egui::vec2(3.0, 0.0), center + egui::vec2(-3.0, 4.0)],
            text_color,
            egui::Stroke::NONE,
        ));
    }
    if let Some(message) = error {
        let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.right() - 12.0, rect.center().y), egui::vec2(16.0, 16.0));
        egui::Image::new(unthemed_icon!("step_error.svg")).paint_at(ui, icon_rect);
        ui.interact(icon_rect, response.id.with("error_hint"), egui::Sense::hover()).on_hover_text(message);
    }
    let text = if header { bold(label) } else { egui::RichText::new(label) }.color(text_color);
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (rect.right() - gutter - 16.0 - if error.is_some() { 20.0 } else { 0.0 }).max(0.0),
        egui::TextStyle::Body,
    );
    ui.painter().galley(egui::pos2(gutter + 8.0, rect.center().y - galley.size().y * 0.5), galley, text_color);
    response
}

/// The frame every named row shares: the fill, the rules and the name, with
/// the cell its control goes in handed back.
///
/// `depth` marks a row as belonging to the one above it - a bench height
/// under the RL it starts at, a pattern under the flitch it styles - which is
/// the whole of how these lists show their nesting.
pub(crate) fn grid_named_row(ui: &mut egui::Ui, label: &str, depth: usize) -> (egui::Rect, egui::Response) {
    let height = grid_row_height(ui);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
    let (fill, stroke, label_color) = {
        let visuals = ui.visuals();
        let fill = if response.hovered() {
            visuals.widgets.hovered.bg_fill
        } else {
            super::tree_row_colors(ui).1
        };
        let color = if depth > 0 { visuals.weak_text_color() } else { visuals.text_color() };
        (fill, visuals.widgets.noninteractive.bg_stroke, color)
    };
    let split = rect.left() + rect.width() * KEY_FRACTION;
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().line_segment([egui::pos2(split, rect.top()), egui::pos2(split, rect.bottom())], stroke);
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], stroke);

    let left = rect.left() + 8.0 + SUB_INDENT * depth as f32;
    let name_rect = egui::Rect::from_min_max(egui::pos2(left, rect.top()), egui::pos2((split - 4.0).max(left), rect.bottom()));
    if name_rect.is_positive() {
        ui.put(
            name_rect,
            egui::Label::new(egui::RichText::new(label).color(label_color)).truncate().halign(egui::Align::Min),
        );
    }
    let cell = egui::Rect::from_min_max(egui::pos2(split + 4.0, rect.top() + 2.0), egui::pos2(rect.right() - 4.0, rect.bottom() - 2.0));
    (cell, response)
}

/// One named flag, checked or not. Returns whether it was just toggled.
pub(crate) fn grid_checkbox_row(ui: &mut egui::Ui, label: &str, value: &mut bool, depth: usize) -> bool {
    let (cell, _) = grid_named_row(ui, label, depth);
    if !cell.is_positive() {
        return false;
    }
    let box_rect = egui::Rect::from_min_max(cell.min, egui::pos2((cell.left() + 24.0).min(cell.right()), cell.bottom()));
    ui.put(box_rect, egui::Checkbox::without_text(value)).changed()
}

/// One named choice from a fixed set of options.
pub(crate) fn grid_choice_row<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    value: &mut T,
    options: impl IntoIterator<Item = T>,
    option_label: impl Fn(T) -> String,
    depth: usize,
) -> bool {
    let (cell, _) = grid_named_row(ui, label, depth);
    if !cell.is_positive() {
        return false;
    }
    let id = egui::Id::new(id);
    let selected = option_label(*value);
    ui.scope_builder(egui::UiBuilder::new().id_salt(id).max_rect(cell), |ui| {
        ui.set_clip_rect(ui.clip_rect().intersect(cell));
        ui.spacing_mut().interact_size.y = cell.height();
        let mut picked = false;
        egui::ComboBox::from_id_salt(id.with("combo"))
            .selected_text(selected)
            .width(cell.width())
            .truncate()
            .show_ui(ui, |ui| {
                for option in options {
                    picked |= ui.selectable_value(value, option, option_label(option)).changed();
                }
            });
        picked
    })
    .inner
}

/// One named multiple selection: a field-shaped button naming what is chosen,
/// which opens a checkable list.
///
/// A summary and a button rather than a column of checkboxes, because the
/// things being chosen from - every loader, every bench of every pit - are a
/// list of unbounded length, and a rule editor that grew with the mine would
/// push everything below it off the page.
pub(crate) fn grid_select_row(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, label: &str, summary: &str, depth: usize) -> egui::Response {
    let (cell, _) = grid_named_row(ui, label, depth);
    if !cell.is_positive() {
        // Still a response, so the caller can hang its popup off something.
        return ui.interact(egui::Rect::NOTHING, egui::Id::new(id), egui::Sense::click());
    }
    let id = egui::Id::new(id);
    ui.scope_builder(egui::UiBuilder::new().id_salt(id).max_rect(cell), |ui| {
        ui.set_clip_rect(ui.clip_rect().intersect(cell));
        ui.spacing_mut().interact_size.y = cell.height();
        ui.put(
            cell,
            // Text left, as every other value in a table reads.
            egui::Button::new((egui::RichText::new(summary), egui::Atom::grow()))
                .fill(ui.visuals().extreme_bg_color)
                .stroke(ui.visuals().widgets.inactive.bg_stroke)
                .wrap_mode(egui::TextWrapMode::Truncate)
                .min_size(cell.size()),
        )
    })
    .inner
}

/// A captioned rule inside a grid, marking off the group of rows beneath it:
/// the flitching list's styling options from the heights they style.
pub(crate) fn grid_separator_row(ui: &mut egui::Ui, label: &str, depth: usize) {
    let height = grid_row_height(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    let (fill, rule, color) = {
        let visuals = ui.visuals();
        (visuals.widgets.noninteractive.bg_fill, visuals.widgets.noninteractive.bg_stroke, visuals.weak_text_color())
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], rule);
    let text_rect = egui::Rect::from_min_max(egui::pos2(rect.left() + 8.0 + SUB_INDENT * depth as f32, rect.top()), rect.max);
    if text_rect.is_positive() {
        ui.put(text_rect, egui::Label::new(egui::RichText::new(label).color(color)).truncate().halign(egui::Align::Min));
    }
}

/// A small + button, tinted as text. Adds a row to the list it sits in.
pub(crate) fn add_button(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, tooltip: &str) -> egui::Response {
    let side = row_height(ui);
    let tint = ui.visuals().text_color();
    ui.add(
        super::toolbar::ToolbarButton::new(egui::Image::new(unthemed_icon!("add.svg")).tint(tint), tooltip)
            .button_side(side)
            .id_salt(id),
    )
}

/// Left edge and width of each column in a row, from the columns' shares.
fn column_spans(rect: egui::Rect, fractions: impl IntoIterator<Item = f32>) -> Vec<egui::Rect> {
    let mut left = rect.left() + GUTTER;
    let width = rect.right() - left;
    fractions
        .into_iter()
        .map(|fraction| {
            let cell = egui::Rect::from_min_max(egui::pos2(left, rect.top()), egui::pos2((left + width * fraction).min(rect.right()), rect.bottom()));
            left = cell.right();
            cell
        })
        .collect()
}

fn paint_column_rules(ui: &egui::Ui, rect: egui::Rect, cells: &[egui::Rect]) {
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    let gutter = rect.left() + GUTTER;
    ui.painter().line_segment([egui::pos2(gutter, rect.top()), egui::pos2(gutter, rect.bottom())], stroke);
    for cell in cells.iter().skip(1) {
        ui.painter()
            .line_segment([egui::pos2(cell.left(), rect.top()), egui::pos2(cell.left(), rect.bottom())], stroke);
    }
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
}

fn paint_cell_text(ui: &egui::Ui, cell: egui::Rect, text: egui::RichText, color: egui::Color32) {
    let galley = egui::WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Truncate), (cell.width() - 16.0).max(0.0), egui::TextStyle::Body);
    ui.painter().galley(egui::pos2(cell.left() + 8.0, cell.center().y - galley.size().y * 0.5), galley, color);
}

/// The header row of a [`DataGrid::columns`] grid.
fn grid_columns_header(ui: &mut egui::Ui, columns: &[(String, f32)]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), grid_row_height(ui)), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, ui.visuals().widgets.noninteractive.bg_fill);
    let cells = column_spans(rect, columns.iter().map(|(_, fraction)| *fraction));
    paint_column_rules(ui, rect, &cells);
    let color = ui.visuals().text_color();
    for ((heading, _), cell) in columns.iter().zip(&cells) {
        paint_cell_text(ui, *cell, bold(heading), color);
    }
}

/// A selectable row of several columns under a [`DataGrid::columns`] header,
/// with the same shares. `cells` are drawn as text, the first in the normal
/// colour and the rest weaker; an empty one is left for the caller to fill,
/// in the rect handed back for it.
pub(crate) fn grid_columns_row(ui: &mut egui::Ui, fractions: &[f32], cells: &[&str], selected: bool) -> (egui::Response, Vec<egui::Rect>) {
    let height = grid_row_height(ui) + COLUMN_ROW_EXTRA;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
    let visuals = ui.visuals();
    let fill = if selected {
        visuals.selection.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        super::tree_row_colors(ui).1
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    let spans = column_spans(rect, fractions.iter().copied());
    paint_column_rules(ui, rect, &spans);
    let (strong, weak) = if selected {
        (visuals.selection.stroke.color, visuals.selection.stroke.color)
    } else {
        (visuals.text_color(), visuals.weak_text_color())
    };
    for (index, (text, cell)) in cells.iter().zip(&spans).enumerate() {
        if !text.is_empty() {
            paint_cell_text(ui, *cell, egui::RichText::new(*text), if index == 0 { strong } else { weak });
        }
    }
    (response, spans)
}

/// One entry in a [`grid_cell_combo`]'s list.
pub(crate) enum CellOption<T> {
    Choice(T, String),
    /// Groups the choices after it, up to the next heading.
    Heading(String),
}

impl<T> From<(T, String)> for CellOption<T> {
    fn from((value, label): (T, String)) -> Self {
        Self::Choice(value, label)
    }
}

/// A choice from a fixed set, filling one grid cell.
pub(crate) fn grid_cell_combo<T: Clone + PartialEq, O: Into<CellOption<T>>>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    cell: egui::Rect,
    value: &mut T,
    options: impl IntoIterator<Item = O>,
    selected_text: &str,
) -> bool {
    // Inset from the row's rules so the box sits inside the row with room
    // above and below, rather than filling it edge to edge.
    let cell = cell.shrink2(egui::vec2(4.0, COLUMN_ROW_EXTRA / 2.0 + 1.0));
    if !cell.is_positive() {
        return false;
    }
    let id = egui::Id::new(id);
    // A child rather than a scope: the row has already claimed this space,
    // and a scope would claim it again, pulling the next row up over this
    // row's bottom rule.
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(cell));
    child.set_clip_rect(child.clip_rect().intersect(cell));
    child.spacing_mut().interact_size.y = cell.height();
    let mut picked = false;
    egui::ComboBox::from_id_salt(id.with("combo"))
        .selected_text(selected_text)
        .width(cell.width())
        // Room for a model's columns, grouped, without scrolling a short list.
        .height(COMBO_LIST_HEIGHT)
        .truncate()
        .show_ui(&mut child, |ui| {
            for option in options {
                match option.into() {
                    CellOption::Choice(option, label) => picked |= ui.selectable_value(value, option, label).changed(),
                    CellOption::Heading(heading) => super::menu::menu_section(ui, heading),
                }
            }
        });
    picked
}

/// A number typed or dragged in one grid cell, with `suffix` (a unit, say)
/// after it. Returns true once an edit is committed, not on every keystroke.
pub(crate) fn grid_cell_number(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, cell: egui::Rect, value: &mut f64, suffix: &str) -> bool {
    let cell = cell.shrink2(egui::vec2(4.0, COLUMN_ROW_EXTRA / 2.0 + 1.0));
    if !cell.is_positive() {
        return false;
    }
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(egui::Id::new(id)).max_rect(cell));
    child.set_clip_rect(child.clip_rect().intersect(cell));
    let response = child.put(
        cell,
        egui::DragValue::new(value)
            .speed(0.1)
            .custom_formatter(|value, _| trimmed_number(value))
            .suffix(suffix)
            .update_while_editing(false),
    );
    super::menu::committed(&response)
}

/// No more decimals than the value has: 360, 12.5.
fn trimmed_number(value: f64) -> String {
    let text = format!("{value:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A number typed into one grid cell, drawn like a [`PropertyTable`] value:
/// plain text with `unit` faint at the right. The text is held while the cell
/// is being edited and read when it is left; returns true then, with `value`
/// updated, if what was typed is a number.
pub(crate) fn grid_cell_entry(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, cell: egui::Rect, value: &mut f64, unit: &str) -> bool {
    let cell = cell.shrink2(egui::vec2(4.0, COLUMN_ROW_EXTRA / 2.0 + 1.0));
    if !cell.is_positive() {
        return false;
    }
    let mut text_rect = cell;
    if !unit.is_empty() {
        let galley = ui
            .painter()
            .layout_no_wrap(unit.to_owned(), egui::TextStyle::Body.resolve(ui.style()), ui.visuals().weak_text_color());
        let left = (cell.right() - galley.size().x - 4.0).max(cell.left());
        ui.painter()
            .with_clip_rect(ui.clip_rect().intersect(cell))
            .galley(egui::pos2(left, cell.center().y - galley.size().y * 0.5), galley, ui.visuals().weak_text_color());
        text_rect.max.x = left - 4.0;
    }
    if !text_rect.is_positive() {
        return false;
    }
    let id = egui::Id::new(id);
    let shown = trimmed_number(*value);
    let mut text = ui.data(|data| data.get_temp::<String>(id)).unwrap_or_else(|| shown.clone());
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(text_rect));
    child.set_clip_rect(child.clip_rect().intersect(text_rect));
    let response = child.put(
        text_rect,
        egui::TextEdit::singleline(&mut text)
            .id(id.with("text"))
            .vertical_align(egui::Align::Center)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(4, 1)))
            .desired_width(text_rect.width()),
    );
    if response.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, text.clone()));
        return false;
    }
    ui.data_mut(|data| data.remove::<String>(id));
    match text.trim().parse::<f64>() {
        Ok(parsed) if response.lost_focus() && parsed.is_finite() && text != shown => {
            *value = parsed;
            true
        }
        _ => false,
    }
}

/// A value shown but not edited in one grid cell, greyed: one this grid
/// mirrors from elsewhere rather than owns.
pub(crate) fn grid_cell_fixed(ui: &egui::Ui, cell: egui::Rect, text: &str) {
    paint_cell_text(ui, cell, egui::RichText::new(text), ui.visuals().weak_text_color());
}

/// A colour swatch filling one grid cell, opening the colour picker.
/// Returns whether the colour changed.
pub(crate) fn grid_cell_color(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, cell: egui::Rect, value: &mut [f32; 4]) -> bool {
    let cell = cell.shrink2(egui::vec2(4.0, COLUMN_ROW_EXTRA / 2.0 + 1.0));
    if !cell.is_positive() {
        return false;
    }
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(egui::Id::new(id)).max_rect(cell));
    child.set_clip_rect(child.clip_rect().intersect(cell));
    child.spacing_mut().interact_size.y = cell.height();
    super::color::edit_rgba_premultiplied(&mut child, value).changed()
}

/// Room a [`grid_cell_warning`] takes at the end of its cell: the mark and
/// the gaps either side of it. A control sharing the cell stops short by this.
pub(crate) const CELL_WARNING_WIDTH: f32 = 22.0;

/// A warning mark at the right end of a grid cell, explaining itself on hover.
pub(crate) fn grid_cell_warning(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, cell: egui::Rect, message: &str) {
    const SIDE: f32 = 14.0;
    let mark = egui::Rect::from_center_size(egui::pos2(cell.right() - CELL_WARNING_WIDTH / 2.0, cell.center().y), egui::Vec2::splat(SIDE));
    egui::Image::new(unthemed_icon!("step_warning.svg")).paint_at(ui, mark);
    ui.interact(mark, egui::Id::new(id), egui::Sense::hover()).on_hover_text(message);
}

/// A collapsible group heading inside a grid. Returns whether the group is
/// open, so the caller draws its rows only then. `depth` nests it under the
/// group heading above it.
pub(crate) fn grid_group_row(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, label: &str, detail: &str, depth: usize) -> bool {
    let id = ui.make_persistent_id(id);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), grid_row_height(ui)), egui::Sense::click());
    if response.clicked() {
        state.toggle(ui);
    }
    let open = state.is_open();
    state.store(ui.ctx());
    let visuals = ui.visuals();
    let fill = if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        visuals.widgets.noninteractive.bg_fill
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], visuals.widgets.noninteractive.bg_stroke);
    let color = visuals.text_color();
    let indent = SUB_INDENT * depth as f32;
    let center = egui::pos2(rect.left() + 12.0 + indent, rect.center().y);
    let points = if open {
        vec![center + egui::vec2(-4.0, -2.0), center + egui::vec2(4.0, -2.0), center + egui::vec2(0.0, 3.0)]
    } else {
        vec![center + egui::vec2(-2.0, -4.0), center + egui::vec2(3.0, 0.0), center + egui::vec2(-2.0, 4.0)]
    };
    ui.painter().add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
    let text_rect = egui::Rect::from_min_max(egui::pos2(rect.left() + GUTTER - 8.0 + indent, rect.top()), rect.max);
    let weak = visuals.weak_text_color();
    let mut job = egui::text::LayoutJob::default();
    let font = egui::TextStyle::Body.resolve(ui.style());
    job.append(label, 0.0, egui::TextFormat::simple(crate::ui::fonts::bold_font(font.size), color));
    if !detail.is_empty() {
        job.append(detail, 8.0, egui::TextFormat::simple(font, weak));
    }
    job.wrap = egui::text::TextWrapping::truncate_at_width((text_rect.width() - 16.0).max(0.0));
    let galley = ui.painter().layout_job(job);
    ui.painter()
        .galley(egui::pos2(text_rect.left() + 8.0, rect.center().y - galley.size().y * 0.5), galley, color);
    open
}

/// One row offered for adding elsewhere: its name, a weak detail, and a +
/// button - or, once it is there, the whole row greyed with no button.
/// Returns whether + was pressed.
pub(crate) fn grid_add_row(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, label: &str, detail: &str, added: bool, tooltip: &str) -> bool {
    let height = grid_row_height(ui);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    let visuals = ui.visuals();
    let fill = if response.hovered() && !added {
        visuals.widgets.hovered.bg_fill
    } else {
        super::tree_row_colors(ui).1
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], visuals.widgets.noninteractive.bg_stroke);
    let (text, weak) = (visuals.text_color(), visuals.weak_text_color());
    let name_rect = egui::Rect::from_min_max(egui::pos2(rect.left() + SUB_INDENT, rect.top()), egui::pos2(rect.right() - height, rect.bottom()));
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    job.append(label, 0.0, egui::TextFormat::simple(font.clone(), if added { weak } else { text }));
    if !detail.is_empty() {
        job.append(detail, 8.0, egui::TextFormat::simple(font, weak));
    }
    job.wrap = egui::text::TextWrapping::truncate_at_width((name_rect.width() - 16.0).max(0.0));
    let galley = ui.painter().layout_job(job);
    ui.painter()
        .galley(egui::pos2(name_rect.left() + 8.0, rect.center().y - galley.size().y * 0.5), galley, text);
    if added {
        return false;
    }
    let button = egui::Rect::from_min_size(egui::pos2(rect.right() - height, rect.top()), egui::vec2(height, height));
    ui.put(
        button,
        super::toolbar::ToolbarButton::new(egui::Image::new(unthemed_icon!("add.svg")).tint(text), tooltip)
            .button_side(height)
            .id_salt(id),
    )
    .clicked()
}

/// The last row of a list that can grow: + and what it adds, the whole row
/// one button. Returns whether it was clicked.
pub(crate) fn grid_add_action_row(ui: &mut egui::Ui, label: &str) -> bool {
    let height = grid_row_height(ui);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
    let visuals = ui.visuals();
    let fill = if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        super::tree_row_colors(ui).1
    };
    let color = if response.hovered() { visuals.text_color() } else { visuals.weak_text_color() };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], visuals.widgets.noninteractive.bg_stroke);
    let icon = egui::Rect::from_center_size(egui::pos2(rect.left() + GUTTER * 0.5, rect.center().y), egui::Vec2::splat(14.0));
    egui::Image::new(unthemed_icon!("add.svg")).tint(color).paint_at(ui, icon);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), egui::TextStyle::Body.resolve(ui.style()), color);
    ui.painter()
        .galley(egui::pos2(rect.left() + GUTTER + 8.0, rect.center().y - galley.size().y * 0.5), galley, color);
    response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// What an empty grid says in place of its rows: one short line, centred,
/// and a button for the first row where the grid has one.
pub(crate) fn grid_empty_state(ui: &mut egui::Ui, message: &str, action: Option<&str>) -> bool {
    let rect = ui.available_rect_before_wrap();
    let mut clicked = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Center)), |ui| {
        ui.add_space((rect.height() * 0.35).max(24.0));
        ui.label(egui::RichText::new(message).color(ui.visuals().weak_text_color()));
        if let Some(action) = action {
            ui.add_space(10.0);
            clicked = ui.add(super::menu::MenuButton::new(action).primary()).clicked();
        }
    });
    clicked
}

/// A titled, bordered pane holding a scrollable column of rows: single
/// [`grid_row`]s, or [`grid_columns_row`]s under a [`DataGrid::columns`]
/// header, with group, add and empty-state rows among them.
pub(crate) struct DataGrid<'a> {
    id: &'a str,
    rect: egui::Rect,
    title: &'a str,
    column_header: Option<&'a str>,
    columns: Option<&'a [(String, f32)]>,
    extras: TitleExtras<'a>,
}

impl<'a> DataGrid<'a> {
    pub(crate) fn new(id: &'a str, rect: egui::Rect, title: &'a str) -> Self {
        Self {
            id,
            rect,
            title,
            column_header: None,
            columns: None,
            extras: TitleExtras::default(),
        }
    }

    /// Pin a header row of several columns above the scroll area, for rows
    /// drawn with [`grid_columns_row`]. Each column is its heading and its
    /// share of the row's width.
    pub(crate) fn columns(mut self, columns: &'a [(String, f32)]) -> Self {
        self.columns = Some(columns);
        self
    }

    /// Put a + button at the right of the title strip; `clicked` is set when
    /// it is pressed. The list's own way to add a row, always in view.
    pub(crate) fn add_button(mut self, tooltip: &'a str, clicked: &'a mut bool) -> Self {
        self.extras.add = Some((tooltip, clicked));
        self
    }

    /// Weak text after the title, such as the size of what the pane shows.
    pub(crate) fn title_detail(mut self, detail: &'a str) -> Self {
        self.extras.detail = Some(detail);
        self
    }

    /// A switch at the right of the title strip, for a setting that applies
    /// to everything in the pane.
    pub(crate) fn title_toggle(mut self, label: &'a str, value: &'a mut bool) -> Self {
        self.extras.toggle = Some((label, value));
        self
    }

    /// A button at the right of the title strip that acts on everything in
    /// the pane; `clicked` is set when it is pressed.
    pub(crate) fn title_action(mut self, label: &'a str, tooltip: &'a str, clicked: &'a mut bool) -> Self {
        self.extras.action = Some((label, tooltip, clicked));
        self
    }

    /// Pin a non-interactive header row above the scroll area.
    pub(crate) fn column_header(mut self, text: &'a str) -> Self {
        self.column_header = Some(text);
        self
    }

    /// `body` draws the scrolling rows; its return value is passed back to the
    /// caller.
    pub(crate) fn show<R>(self, ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
        framed_pane(ui, self.id, self.rect, self.title, self.extras, |ui| {
            if let Some(header) = self.column_header {
                grid_row(ui, GridRow::header(header));
            }
            if let Some(columns) = self.columns {
                grid_columns_header(ui, columns);
            }
            egui::ScrollArea::vertical()
                .id_salt((self.id, "body"))
                .auto_shrink([false; 2])
                .min_scrolled_height(0.0)
                .show(ui, body)
                .inner
        })
    }
}

/// A titled, bordered two-column key/value editor.
pub(crate) struct PropertyTable<'a> {
    id: &'a str,
    rect: egui::Rect,
    title: &'a str,
}

impl<'a> PropertyTable<'a> {
    pub(crate) fn new(id: &'a str, rect: egui::Rect, title: &'a str) -> Self {
        Self { id, rect, title }
    }

    /// `body` populates the table via [`PropertyRows::header`] and
    /// [`PropertyRows::field`].
    pub(crate) fn show(self, ui: &mut egui::Ui, body: impl FnOnce(&mut PropertyRows<'_>)) {
        framed_pane(ui, self.id, self.rect, self.title, TitleExtras::default(), |ui| {
            egui::ScrollArea::vertical()
                .id_salt((self.id, "body"))
                .auto_shrink([false; 2])
                .min_scrolled_height(0.0)
                .show(ui, |ui| body(&mut PropertyRows { ui }));
        });
    }
}

/// The mark at the end of a [`PropertyRows`] value, with its message.
#[derive(Clone, Copy)]
enum Badge<'a> {
    Error(&'a str),
    Warning(&'a str),
}

/// Row sink handed to a [`PropertyTable`] body closure.
pub(crate) struct PropertyRows<'u> {
    ui: &'u mut egui::Ui,
}

impl PropertyRows<'_> {
    /// The bold "Property / Value" heading row, as a [`DataGrid::columns`]
    /// header reads.
    pub(crate) fn header(&mut self, key: &str, value: &str) {
        let (rect, split) = self.begin_row(true);
        let color = self.ui.visuals().text_color();
        paint_cell_text(self.ui, Self::key_cell(rect, split), bold(key), color);
        paint_cell_text(self.ui, Self::value_cell(rect, split), bold(value), color);
    }

    /// The key of an ordinary row, left-aligned in its cell.
    fn paint_key(&self, rect: egui::Rect, split: f32, key: &str) {
        paint_cell_text(self.ui, Self::key_cell(rect, split), egui::RichText::new(key), self.ui.visuals().text_color());
    }

    /// An editable key/value pair. `error`, when set, shows a red badge in the
    /// value column with the message as its tooltip. Returns the field's response.
    pub(crate) fn field(&mut self, key: &str, value: &mut String, error: Option<&str>) -> egui::Response {
        self.value_field(key, value, None, error.map(Badge::Error), false)
    }

    /// An editable value, with its unit faint at the cell's right as a
    /// [`grid_cell_entry`] shows one. The text is held while it is being
    /// typed, so the table can be drawn from the model every frame; it is
    /// handed back once, when focus leaves it changed from `current`.
    pub(crate) fn committed_entry(&mut self, id: impl std::hash::Hash + std::fmt::Debug, key: &str, current: &str, unit: Option<&str>) -> (egui::Response, Option<String>) {
        let id = egui::Id::new(id);
        let mut text = self.ui.data(|data| data.get_temp::<String>(id)).unwrap_or_else(|| current.to_owned());
        let response = self.value_field(key, &mut text, unit, None, false);
        if response.has_focus() {
            self.ui.data_mut(|data| data.insert_temp(id, text));
            return (response, None);
        }
        self.ui.data_mut(|data| data.remove::<String>(id));
        let committed = (response.lost_focus() && text != current).then_some(text);
        (response, committed)
    }

    /// A calculated value is rendered directly in the table cell with an optional unit.
    pub(crate) fn readonly(&mut self, key: &str, value: &str, unit: Option<&str>, error: Option<&str>) -> egui::Response {
        self.value_field(key, &mut value.to_owned(), unit, error.map(Badge::Error), true)
    }

    /// A calculated value with a yellow warning mark, for a figure that
    /// stands but rests on an assumption the message explains on hover.
    pub(crate) fn readonly_warning(&mut self, key: &str, value: &str, unit: Option<&str>, warning: Option<&str>) -> egui::Response {
        self.value_field(key, &mut value.to_owned(), unit, warning.map(Badge::Warning), true)
    }

    /// Weak text across both columns: a hint about the rows above it.
    pub(crate) fn note(&mut self, text: &str) {
        let rect = self.begin_table_row(false);
        let cell = egui::Rect::from_min_max(egui::pos2(rect.left() + GUTTER, rect.top()), rect.max);
        paint_cell_text(self.ui, cell, egui::RichText::new(text), self.ui.visuals().weak_text_color());
        self.ui.interact(cell, self.ui.id().with(("note", text)), egui::Sense::hover()).on_hover_text(text);
    }

    /// A row across both columns, `height` tall, drawn by `add` into the rect
    /// it is handed: a chart, say.
    pub(crate) fn wide(&mut self, height: f32, add: impl FnOnce(&mut egui::Ui, egui::Rect)) {
        let (rect, _) = self.ui.allocate_exact_size(egui::vec2(self.ui.available_width(), height), egui::Sense::hover());
        let inner = rect.shrink2(egui::vec2(8.0, 6.0));
        let mut child = self.ui.new_child(egui::UiBuilder::new().max_rect(inner));
        child.set_clip_rect(child.clip_rect().intersect(inner));
        add(&mut child, inner);
    }

    /// A single-choice value, drawn as a combo box filling the value column.
    /// `options` supplies each selectable value with its label; the row is
    /// identified by `id` so two tables can carry the same key.
    pub(crate) fn combo<T: Clone + PartialEq>(
        &mut self,
        id: impl std::hash::Hash + std::fmt::Debug,
        key: &str,
        value: &mut T,
        selected_text: &str,
        options: impl IntoIterator<Item = (T, String)>,
    ) -> egui::Response {
        let (rect, split) = self.begin_row(false);
        self.paint_key(rect, split, key);
        let value_rect = self.value_rect(rect, split);
        let mut changed = false;
        // See [`Self::place`]: the row has already claimed this space.
        let mut child = self.ui.new_child(egui::UiBuilder::new().max_rect(value_rect));
        child.set_clip_rect(child.clip_rect().intersect(value_rect));
        // The combo is a button, and its natural height would overrun the
        // row rule; the cell it sits in is the height it gets.
        child.spacing_mut().interact_size.y = value_rect.height();
        let mut response = egui::ComboBox::from_id_salt(id)
            .selected_text(selected_text)
            .width(value_rect.width())
            .truncate()
            .show_ui(&mut child, |ui| {
                for (option, text) in options {
                    changed |= ui.selectable_value(value, option, text).changed();
                }
            })
            .response;
        if changed {
            response.mark_changed();
        }
        response
    }

    /// A multiple selection, drawn as [`grid_select_row`] draws one: a
    /// field-shaped button naming what is chosen, for the caller to hang a
    /// checkable list off.
    pub(crate) fn select(&mut self, key: &str, summary: &str) -> egui::Response {
        let (rect, split) = self.begin_row(false);
        self.paint_key(rect, split, key);
        let value_rect = self.value_rect(rect, split);
        let visuals = self.ui.visuals().clone();
        self.place(
            value_rect,
            // Text left, as every other value in a table reads.
            egui::Button::new((egui::RichText::new(summary), egui::Atom::grow()))
                .fill(visuals.extreme_bg_color)
                .stroke(visuals.widgets.inactive.bg_stroke)
                .wrap_mode(egui::TextWrapMode::Truncate)
                .min_size(value_rect.size()),
        )
    }

    pub(crate) fn action(&mut self, key: &str, label: &str) -> egui::Response {
        let (rect, split) = self.begin_row(false);
        self.paint_key(rect, split, key);
        self.place(self.value_rect(rect, split), super::menu::MenuButton::new(label))
    }

    /// An editable boolean, drawn as a checkbox in the value column.
    pub(crate) fn checkbox(&mut self, key: &str, value: &mut bool) -> egui::Response {
        let (rect, split) = self.begin_row(false);
        self.paint_key(rect, split, key);
        self.place(self.value_rect(rect, split), egui::Checkbox::new(value, ""))
    }

    fn value_field(&mut self, key: &str, value: &mut String, unit: Option<&str>, badge: Option<Badge<'_>>, readonly: bool) -> egui::Response {
        let (rect, split) = self.begin_row(false);
        self.paint_key(rect, split, key);
        if let Some(badge) = badge {
            let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.right() - CELL_WARNING_WIDTH / 2.0, rect.center().y), egui::vec2(16.0, 16.0));
            let (icon, message) = match badge {
                Badge::Error(message) => (unthemed_icon!("step_error.svg"), message),
                Badge::Warning(message) => (unthemed_icon!("step_warning.svg"), message),
            };
            self.place(icon_rect, egui::Image::new(icon).fit_to_exact_size(icon_rect.size()).sense(egui::Sense::hover()))
                .on_hover_text(message);
        }
        let mut value_rect = self.value_rect(rect, split);
        if badge.is_some() {
            value_rect.max.x -= CELL_WARNING_WIDTH;
        }
        if let Some(unit) = unit {
            let width = self
                .ui
                .painter()
                .layout_no_wrap(unit.to_owned(), egui::TextStyle::Body.resolve(self.ui.style()), self.ui.visuals().weak_text_color())
                .size()
                .x
                + 8.0;
            let unit_rect = egui::Rect::from_min_max(egui::pos2((value_rect.right() - width).max(value_rect.left()), value_rect.top()), value_rect.max);
            let galley =
                egui::WidgetText::from(egui::RichText::new(unit).weak()).into_galley(self.ui, Some(egui::TextWrapMode::Truncate), unit_rect.width(), egui::TextStyle::Body);
            self.ui.painter().with_clip_rect(self.ui.clip_rect().intersect(unit_rect)).galley(
                egui::pos2(unit_rect.right() - galley.size().x, unit_rect.center().y - galley.size().y * 0.5),
                galley,
                self.ui.visuals().weak_text_color(),
            );
            value_rect.max.x = unit_rect.left();
        }
        if readonly {
            // TextEdit has its own minimum height and frame sizing. Calculated
            // cells need only text, bounded by the same row as their unit.
            let text_rect = value_rect.shrink2(egui::vec2(4.0, 0.0));
            let galley = egui::WidgetText::from(value.as_str()).into_galley(self.ui, Some(egui::TextWrapMode::Truncate), text_rect.width().max(0.0), egui::TextStyle::Body);
            self.ui.painter().with_clip_rect(self.ui.clip_rect().intersect(value_rect)).galley(
                egui::pos2(text_rect.left(), text_rect.center().y - galley.size().y * 0.5),
                galley,
                self.ui.visuals().text_color(),
            );
            return self
                .ui
                .interact(value_rect, self.ui.id().with(("calculated", key)), egui::Sense::hover())
                .on_hover_text(value.as_str());
        }
        self.place(
            value_rect,
            egui::TextEdit::singleline(value)
                .vertical_align(egui::Align::Center)
                .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(4, 1)))
                // egui 0.35 needs a nonzero text atom to anchor the caret
                // in an empty field. A blank hint supplies it without
                // inserting placeholder text into the stored value.
                .hint_text(" ")
                .desired_width(value_rect.width()),
        )
    }

    /// Put `widget` in `rect` of a row already allocated. `Ui::put`, or a
    /// scope, would claim the space again and leave the table's cursor at the
    /// widget's edge rather than the row's, so rows came out uneven.
    fn place(&mut self, rect: egui::Rect, widget: impl egui::Widget) -> egui::Response {
        let mut child = self.ui.new_child(egui::UiBuilder::new().max_rect(rect));
        child.set_clip_rect(child.clip_rect().intersect(rect));
        child.put(rect, widget)
    }

    /// Allocate one row and paint its gutter, column split and bottom rule:
    /// the same frame, and for an ordinary row the same height, as a
    /// [`grid_columns_row`], so a property table reads as one of the grids.
    fn begin_row(&mut self, header: bool) -> (egui::Rect, f32) {
        let rect = self.begin_table_row(header);
        let spans = column_spans(rect, [KEY_FRACTION, 1.0 - KEY_FRACTION]);
        paint_column_rules(self.ui, rect, &spans);
        (rect, spans[1].left())
    }

    /// A row with no key/value split, for the small tables set inside. Its
    /// gutter and bottom rule are painted; its own columns are the caller's.
    fn begin_table_row(&mut self, header: bool) -> egui::Rect {
        let height = grid_row_height(self.ui) + if header { 0.0 } else { COLUMN_ROW_EXTRA };
        let (rect, _) = self.ui.allocate_exact_size(egui::vec2(self.ui.available_width(), height), egui::Sense::hover());
        if header {
            self.ui.painter().rect_filled(rect, 0.0, self.ui.visuals().widgets.noninteractive.bg_fill);
        }
        paint_column_rules(self.ui, rect, &[]);
        rect
    }

    fn key_cell(rect: egui::Rect, split: f32) -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(rect.left() + GUTTER, rect.top()), egui::pos2(split, rect.bottom()))
    }

    fn value_cell(rect: egui::Rect, split: f32) -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(split, rect.top()), rect.max)
    }

    /// Where a row's control goes: its value cell, inset from the rules as
    /// the grid's own cell controls are.
    fn value_rect(&self, rect: egui::Rect, split: f32) -> egui::Rect {
        Self::value_cell(rect, split).shrink2(egui::vec2(4.0, COLUMN_ROW_EXTRA / 2.0 + 1.0))
    }
}
