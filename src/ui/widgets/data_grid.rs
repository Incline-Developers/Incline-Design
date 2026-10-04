//! An editable grid: a row of buttons over a header and a run of rows whose
//! cells the caller draws.
//!
//! `DataTable` shows text read-only; this is for rows a user builds. The grid
//! owns the chrome - column widths by weight, the header, row selection, the
//! banding, drag handles, collapsible group rows, the buttons and the group
//! name prompt - and nothing about what a row holds. The caller draws each
//! cell, and applies the [`GridAction`]s the grid hands back to its own rows,
//! so the same grid serves a list of constants, of rock type costs or of
//! processing methods. [`apply_flat`] does that for the common plain list.
//!
//! Buttons are opt-in: add and delete always; move up/down, "add all" and
//! groups through [`GridButtons`]. A grid without groups simply never draws a
//! group row.

use std::hash::Hash;

use crate::{
    i18n::tr,
    ui::{
        themed_icon, unthemed_icon,
        widgets::{
            menu::{self, DragableMenu, MenuButton},
            toolbar::ToolbarButton,
        },
    },
};

const HANDLE_WIDTH: f32 = 18.0;
const CELL_GAP: f32 = 4.0;
const SCROLLBAR_ALLOWANCE: f32 = 14.0;
const MAX_BODY_HEIGHT: f32 = 280.0;
/// The least a grid asked to fill a height leaves for its rows.
const MIN_FILLED_BODY_HEIGHT: f32 = 60.0;
const BUTTON_SIDE: f32 = 26.0;
/// Room at the left of every row of a grid with groups, where the bar that
/// marks a row as belonging to its group is drawn.
const GROUP_GUTTER: f32 = 14.0;

/// One column: its header and its share of the row. Shares are weights, not
/// pixels: 25, 15, 25, 35 gives the four columns those percentages.
pub(crate) struct GridColumn {
    pub(crate) title: String,
    pub(crate) weight: f32,
}

impl GridColumn {
    pub(crate) fn new(title: impl Into<String>, weight: f32) -> Self {
        Self { title: title.into(), weight }
    }
}

/// Which optional buttons the grid shows. Add and delete are always there.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GridButtons {
    /// Move the selected row up or down.
    pub(crate) reorder: bool,
    /// Add a row for everything the caller can offer that is not yet in the grid.
    pub(crate) fill_all: bool,
    /// Add group rows, which hold the rows below them and fold away.
    pub(crate) groups: bool,
}

impl GridButtons {
    pub(crate) fn reorder(mut self) -> Self {
        self.reorder = true;
        self
    }

    pub(crate) fn fill_all(mut self) -> Self {
        self.fill_all = true;
        self
    }

    pub(crate) fn groups(mut self) -> Self {
        self.groups = true;
        self
    }
}

/// What the grid is told about each row: a plain row the caller draws, or a
/// group row the grid draws itself.
#[derive(Clone, Debug)]
pub(crate) enum GridRow {
    Item,
    Group { name: String, collapsed: bool },
}

/// What the user asked of the grid. Indices are positions in the caller's rows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GridAction {
    /// A new blank row, after the selected one if any.
    Add,
    Delete(usize),
    MoveUp(usize),
    MoveDown(usize),
    FillAll,
    /// A group row named as typed; the caller makes the name unique.
    AddGroup(String),
    ToggleGroup(usize),
    RenameGroup(usize, String),
    /// A dragged row dropped so that it lands before `slot` (`rows.len()` is the end).
    Drop {
        from: usize,
        slot: usize,
    },
}

/// The width of each column of a grid of `columns` laid out across
/// `available` pixels. Exposed so a row drawn beside the grid (a total, a
/// field under two of its columns) can line up with it.
pub(crate) fn column_widths(available: f32, columns: &[GridColumn], with_groups: bool) -> Vec<f32> {
    let gutter = if with_groups { GROUP_GUTTER } else { 0.0 };
    let total = (available - HANDLE_WIDTH - gutter - SCROLLBAR_ALLOWANCE - CELL_GAP * columns.len() as f32).max(80.0);
    let weight_sum: f32 = columns.iter().map(|column| column.weight).sum::<f32>().max(f32::EPSILON);
    columns.iter().map(|column| total * column.weight / weight_sum).collect()
}

/// The colour that marks the rows of the group with this position among the grid's groups.
pub(crate) fn group_colour(ordinal: usize, dark_mode: bool) -> egui::Color32 {
    let hue = (0.08 + ordinal as f32 * 0.17).fract();
    egui::ecolor::Hsva::new(hue, 0.55, if dark_mode { 0.85 } else { 0.8 }, 1.0).into()
}

pub(crate) struct DataGrid {
    id: egui::Id,
    columns: Vec<GridColumn>,
    buttons: GridButtons,
    fill_height: Option<f32>,
}

impl DataGrid {
    pub(crate) fn new(id_salt: impl Hash + std::fmt::Debug, columns: Vec<GridColumn>) -> Self {
        Self {
            id: egui::Id::new(("data_grid", id_salt)),
            columns,
            buttons: GridButtons::default(),
            fill_height: None,
        }
    }

    /// Grow the grid, buttons and header included, to `height` from where it
    /// starts, instead of stopping at the usual body height.
    pub(crate) fn fill_height(mut self, height: f32) -> Self {
        self.fill_height = Some(height);
        self
    }

    /// The width a row spans: handle, gutter and every column with the gaps
    /// between them. Group rows and plain rows share it so their backgrounds
    /// end at the same edge.
    fn row_width(&self, widths: &[f32]) -> f32 {
        let widgets = widths.len() + 1 + usize::from(self.buttons.groups);
        self.gutter() + HANDLE_WIDTH + widths.iter().sum::<f32>() + CELL_GAP * (widgets - 1) as f32
    }

    fn gutter(&self) -> f32 {
        if self.buttons.groups { GROUP_GUTTER } else { 0.0 }
    }

    pub(crate) fn buttons(mut self, buttons: GridButtons) -> Self {
        self.buttons = buttons;
        self
    }

    /// Draw the grid. `cell` draws one cell of a plain row, given the row and
    /// column; it should fill the width it is given.
    pub(crate) fn show(self, ui: &mut egui::Ui, selected: &mut Option<usize>, rows: &[GridRow], mut cell: impl FnMut(&mut egui::Ui, usize, usize)) -> Vec<GridAction> {
        let top = ui.cursor().top();
        let mut actions = Vec::new();
        if selected.is_some_and(|row| row >= rows.len()) {
            *selected = rows.len().checked_sub(1);
        }
        self.draw_buttons(ui, *selected, &mut actions);

        let widths = column_widths(ui.available_width(), &self.columns, self.buttons.groups);
        let row_height = ui.spacing().interact_size.y + 4.0;

        ui.spacing_mut().item_spacing = egui::vec2(CELL_GAP, 2.0);
        self.draw_header(ui, &widths, row_height);

        let dragging = egui::DragAndDrop::payload::<usize>(ui.ctx()).map(|payload| *payload);
        if dragging.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        let released = ui.input(|input| input.pointer.any_released());
        let body_height = self
            .fill_height
            .map_or(MAX_BODY_HEIGHT, |height| (height - (ui.cursor().top() - top)).max(MIN_FILLED_BODY_HEIGHT));
        egui::ScrollArea::vertical()
            .id_salt(self.id.with("scroll"))
            .max_height(body_height)
            .min_scrolled_height(if self.fill_height.is_some() { body_height } else { 0.0 })
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let mut folded = false;
                let mut group: Option<egui::Color32> = None;
                let mut groups_seen = 0;
                for (index, row) in rows.iter().enumerate() {
                    let band = index % 2 == 1;
                    match row {
                        GridRow::Group { name, collapsed } => {
                            folded = *collapsed;
                            let colour = group_colour(groups_seen, ui.visuals().dark_mode);
                            groups_seen += 1;
                            group = Some(colour);
                            self.draw_group_row(
                                ui,
                                index,
                                name,
                                *collapsed,
                                self.row_width(&widths),
                                row_height,
                                colour,
                                selected,
                                &mut actions,
                                dragging.is_some().then_some(released),
                            );
                        }
                        GridRow::Item if folded => {}
                        GridRow::Item => {
                            let row_rect = self.draw_item_row(ui, index, &widths, row_height, band, group, selected, &mut cell);
                            self.handle_drop(ui, index, row_rect, false, dragging, released, &mut actions);
                        }
                    }
                }
            });
        self.draw_group_prompt(ui, &mut actions);
        actions
    }

    fn draw_buttons(&self, ui: &mut egui::Ui, selected: Option<usize>, actions: &mut Vec<GridAction>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let has_selection = selected.is_some();
            if self.icon_button(ui, "add", egui::Image::new(themed_icon!(ui, "grid_add.svg")), tr!("opt-grid-add"), true) {
                actions.push(GridAction::Add);
            }
            if self.icon_button(ui, "delete", egui::Image::new(unthemed_icon!("delete_scenario.svg")), tr!("opt-grid-delete"), has_selection)
                && let Some(row) = selected
            {
                actions.push(GridAction::Delete(row));
            }
            if self.buttons.reorder {
                if self.icon_button(ui, "up", egui::Image::new(themed_icon!(ui, "grid_up.svg")), tr!("opt-grid-up"), has_selection)
                    && let Some(row) = selected
                {
                    actions.push(GridAction::MoveUp(row));
                }
                if self.icon_button(ui, "down", egui::Image::new(themed_icon!(ui, "grid_down.svg")), tr!("opt-grid-down"), has_selection)
                    && let Some(row) = selected
                {
                    actions.push(GridAction::MoveDown(row));
                }
            }
            if self.buttons.fill_all && self.icon_button(ui, "fill", egui::Image::new(themed_icon!(ui, "grid_add_all.svg")), tr!("opt-grid-add-all"), true) {
                actions.push(GridAction::FillAll);
            }
            if self.buttons.groups && self.icon_button(ui, "group", egui::Image::new(themed_icon!(ui, "grid_add_group.svg")), tr!("opt-grid-add-group"), true) {
                ui.data_mut(|data| data.insert_temp(self.id.with("prompt"), String::new()));
            }
        });
    }

    /// One of the grid's icon buttons; `true` when clicked while enabled.
    fn icon_button(&self, ui: &mut egui::Ui, name: &str, icon: egui::Image<'static>, tooltip: String, enabled: bool) -> bool {
        let button = ToolbarButton::new(icon, tooltip).id_salt(self.id.with(("button", name))).button_side(BUTTON_SIDE);
        ui.add_enabled_ui(enabled, |ui| ui.add(button)).inner.clicked()
    }

    fn draw_header(&self, ui: &mut egui::Ui, widths: &[f32], row_height: f32) {
        ui.horizontal(|ui| {
            ui.allocate_exact_size(egui::vec2(HANDLE_WIDTH + self.gutter(), row_height), egui::Sense::hover());
            for (column, width) in self.columns.iter().zip(widths) {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(*width, row_height), egui::Sense::hover());
                ui.painter().text(
                    rect.left_center() + egui::vec2(4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &column.title,
                    egui::TextStyle::Body.resolve(ui.style()),
                    ui.visuals().weak_text_color(),
                );
            }
        });
        ui.separator();
    }

    /// Draw one plain row and return the rectangle it occupies.
    #[allow(clippy::too_many_arguments)]
    fn draw_item_row(
        &self,
        ui: &mut egui::Ui,
        index: usize,
        widths: &[f32],
        row_height: f32,
        band: bool,
        group: Option<egui::Color32>,
        selected: &mut Option<usize>,
        cell: &mut impl FnMut(&mut egui::Ui, usize, usize),
    ) -> egui::Rect {
        let background = ui.painter().add(egui::Shape::Noop);
        let response = ui
            .horizontal(|ui| {
                if self.buttons.groups {
                    // The bar down the left edge ties a row to its group.
                    let (gutter, _) = ui.allocate_exact_size(egui::vec2(GROUP_GUTTER, row_height), egui::Sense::hover());
                    if let Some(colour) = group {
                        let bar = egui::Rect::from_min_max(
                            egui::pos2(gutter.center().x - 2.0, gutter.top() - 1.0),
                            egui::pos2(gutter.center().x + 2.0, gutter.bottom() + 1.0),
                        );
                        ui.painter().rect_filled(bar, 1.0, colour);
                    }
                }
                let (handle, handle_response) = ui.allocate_exact_size(egui::vec2(HANDLE_WIDTH, row_height), egui::Sense::drag());
                ui.painter().text(
                    handle.center(),
                    egui::Align2::CENTER_CENTER,
                    "≡",
                    egui::TextStyle::Button.resolve(ui.style()),
                    ui.visuals().weak_text_color(),
                );
                if handle_response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
                if handle_response.drag_started() {
                    handle_response.dnd_set_drag_payload(index);
                    *selected = Some(index);
                }
                for (column, width) in widths.iter().enumerate() {
                    ui.allocate_ui_with_layout(egui::vec2(*width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(*width);
                        ui.set_max_width(*width);
                        cell(ui, index, column);
                    });
                }
            })
            .response;
        let rect = egui::Rect::from_min_size(response.rect.min, egui::vec2(self.row_width(widths), response.rect.height()));
        if ui.rect_contains_pointer(rect) && ui.input(|input| input.pointer.any_pressed()) {
            *selected = Some(index);
        }
        let fill = if *selected == Some(index) {
            Some(ui.visuals().selection.bg_fill.gamma_multiply(0.35))
        } else if let Some(colour) = group {
            Some(colour.gamma_multiply(0.16))
        } else if band {
            Some(ui.visuals().faint_bg_color)
        } else {
            None
        };
        if let Some(fill) = fill {
            ui.painter().set(background, egui::Shape::rect_filled(rect.expand2(egui::vec2(2.0, 1.0)), 2.0, fill));
        }
        rect
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_group_row(
        &self,
        ui: &mut egui::Ui,
        index: usize,
        name: &str,
        collapsed: bool,
        row_width: f32,
        row_height: f32,
        colour: egui::Color32,
        selected: &mut Option<usize>,
        actions: &mut Vec<GridAction>,
        drag_released: Option<bool>,
    ) {
        let background = ui.painter().add(egui::Shape::Noop);
        let response = ui
            .horizontal(|ui| {
                let (gutter, _) = ui.allocate_exact_size(egui::vec2(GROUP_GUTTER, row_height), egui::Sense::hover());
                let bar = egui::Rect::from_min_max(
                    egui::pos2(gutter.center().x - 3.0, gutter.top() - 1.0),
                    egui::pos2(gutter.center().x + 3.0, gutter.bottom() + 1.0),
                );
                ui.painter().rect_filled(bar, 1.0, colour);
                let arrow = if collapsed { "▶" } else { "▼" };
                if ui.add_sized([HANDLE_WIDTH, row_height], egui::Button::new(arrow).frame(false)).clicked() {
                    actions.push(GridAction::ToggleGroup(index));
                }
                let mut edited = name.to_owned();
                let width = (row_width - GROUP_GUTTER - HANDLE_WIDTH - 2.0 * CELL_GAP).max(60.0);
                let response = ui.add_sized([width, row_height - 2.0], egui::TextEdit::singleline(&mut edited).font(egui::TextStyle::Button));
                if name.trim().is_empty() {
                    crate::ui::widgets::value_field::mark_invalid(ui, &response);
                }
                if response.changed() {
                    actions.push(GridAction::RenameGroup(index, edited));
                }
            })
            .response;
        let rect = egui::Rect::from_min_size(response.rect.min, egui::vec2(row_width, response.rect.height()));
        if ui.rect_contains_pointer(rect) && ui.input(|input| input.pointer.any_pressed()) {
            *selected = Some(index);
        }
        let fill = if *selected == Some(index) {
            ui.visuals().selection.bg_fill.gamma_multiply(0.5)
        } else {
            colour.gamma_multiply(0.5)
        };
        ui.painter().set(background, egui::Shape::rect_filled(rect.expand2(egui::vec2(2.0, 1.0)), 2.0, fill));
        if let Some(released) = drag_released {
            let dragging = egui::DragAndDrop::payload::<usize>(ui.ctx()).map(|payload| *payload);
            self.handle_drop(ui, index, rect, true, dragging, released, actions);
        }
    }

    /// Show where a dragged row would land over `rect`, and drop it there when
    /// the pointer is let go. A group row always takes the row in below itself.
    #[allow(clippy::too_many_arguments)]
    fn handle_drop(&self, ui: &egui::Ui, index: usize, rect: egui::Rect, is_group: bool, dragging: Option<usize>, released: bool, actions: &mut Vec<GridAction>) {
        let Some(from) = dragging else {
            return;
        };
        let Some(pointer) = ui.input(|input| input.pointer.hover_pos()).filter(|_| ui.rect_contains_pointer(rect)) else {
            return;
        };
        let after = is_group || pointer.y > rect.center().y;
        let slot = if after { index + 1 } else { index };
        let line_y = if after { rect.bottom() } else { rect.top() };
        ui.painter().hline(rect.x_range(), line_y, egui::Stroke::new(2.0, ui.visuals().selection.bg_fill));
        if released {
            actions.push(GridAction::Drop { from, slot });
        }
    }

    /// The small window that asks for a group's name.
    fn draw_group_prompt(&self, ui: &mut egui::Ui, actions: &mut Vec<GridAction>) {
        let key = self.id.with("prompt");
        let Some(mut name) = ui.data(|data| data.get_temp::<String>(key)) else {
            return;
        };
        let mut open = true;
        let mut cancel = false;
        let mut accept = false;
        DragableMenu::new(self.id.with("prompt_dialog"), tr!("opt-grid-add-group"))
            .open(&mut open)
            .min_width(280.0)
            .show(ui.ctx(), |ui| {
                let response = ui.add(egui::TextEdit::singleline(&mut name).hint_text(tr!("opt-grid-group-name")).desired_width(f32::INFINITY));
                response.request_focus();
                let ready = !name.trim().is_empty();
                menu::menu_actions(ui, |ui| {
                    if ui.add(MenuButton::new(tr!("opt-grid-add")).primary().enabled(ready)).clicked() || (ready && menu::dialog_confirm_pressed(ui.ctx())) {
                        accept = true;
                    }
                    if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                        cancel = true;
                    }
                });
            });
        if accept {
            actions.push(GridAction::AddGroup(name.trim().to_owned()));
            ui.data_mut(|data| data.remove::<String>(key));
        } else if cancel || !open {
            ui.data_mut(|data| data.remove::<String>(key));
        } else {
            ui.data_mut(|data| data.insert_temp(key, name));
        }
    }
}

/// Move `rows[from]` to land before the row at `slot` (as it was before the
/// move), and return where it ended up.
pub(crate) fn move_to_slot<T>(rows: &mut Vec<T>, from: usize, slot: usize) -> usize {
    if from >= rows.len() {
        return from;
    }
    let row = rows.remove(from);
    let to = if slot > from { slot - 1 } else { slot }.min(rows.len());
    rows.insert(to, row);
    to
}

/// Apply the actions every plain list handles the same way - add, delete, move
/// and drop - leaving the rest in `actions` for the caller.
pub(crate) fn apply_flat<T>(rows: &mut Vec<T>, selected: &mut Option<usize>, actions: &mut Vec<GridAction>, new_row: impl Fn() -> T) {
    let mut remaining = Vec::new();
    for action in actions.drain(..) {
        match action {
            GridAction::Add => {
                let at = selected.map_or(rows.len(), |row| (row + 1).min(rows.len()));
                rows.insert(at, new_row());
                *selected = Some(at);
            }
            GridAction::Delete(row) if row < rows.len() => {
                rows.remove(row);
                *selected = rows.len().checked_sub(1).map(|last| row.min(last));
            }
            GridAction::MoveUp(row) if row > 0 && row < rows.len() => {
                rows.swap(row, row - 1);
                *selected = Some(row - 1);
            }
            GridAction::MoveDown(row) if row + 1 < rows.len() => {
                rows.swap(row, row + 1);
                *selected = Some(row + 1);
            }
            GridAction::Drop { from, slot } => *selected = Some(move_to_slot(rows, from, slot)),
            GridAction::Delete(_) | GridAction::MoveUp(_) | GridAction::MoveDown(_) => {}
            other => remaining.push(other),
        }
    }
    *actions = remaining;
}
