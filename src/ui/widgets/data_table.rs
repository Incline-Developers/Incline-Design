//! A read-only spreadsheet of text cells: a pinned header, banded rows that
//! scroll in both directions, a rectangular cell selection, and copy.
//!
//! Rows are drawn on demand from a callback, so a table of any length costs
//! only the rows on screen.

use std::{fmt::Debug, hash::Hash};

use super::{menu, shifted, toolbar::GROUP_CORNER_RADIUS};
use crate::i18n::tr;

/// Breathing room either side of a cell's text.
const CELL_PADDING: f32 = 6.0;
/// Added to the body text height to get a row's height.
const ROW_PADDING: f32 = 6.0;
/// Narrowest a column is drawn, even one full of blanks.
const MIN_COLUMN_WIDTH: f32 = 40.0;
/// Widest a column is drawn; text past this truncates on screen but still
/// travels whole in a copy.
const MAX_COLUMN_WIDTH: f32 = 180.0;
/// Rows measured when sizing the columns.
const WIDTH_SAMPLE: usize = 512;
/// Least height the rows scroll within, in rows.
const MIN_VISIBLE_ROWS: f32 = 3.0;

/// Cells of one row, one per header column.
pub(crate) type RowCells<'a> = &'a dyn Fn(usize) -> Vec<String>;

/// A read-only table of `row_count` rows under `header`.
pub(crate) struct DataTable<'a> {
    id: egui::Id,
    header: &'a [String],
    aligns: &'a [egui::Align],
    row_count: usize,
    cells: RowCells<'a>,
    copy_cells: Option<RowCells<'a>>,
    fingerprint: u64,
    max_height: f32,
}

/// A rectangular block of cells marked for copying: the cell the reader
/// started from and the cell they last extended to.
#[derive(Clone, Copy, PartialEq)]
struct Selection {
    anchor: (usize, usize),
    focus: (usize, usize),
}

impl Selection {
    fn rows(self) -> std::ops::RangeInclusive<usize> {
        self.anchor.0.min(self.focus.0)..=self.anchor.0.max(self.focus.0)
    }

    fn columns(self) -> std::ops::RangeInclusive<usize> {
        self.anchor.1.min(self.focus.1)..=self.anchor.1.max(self.focus.1)
    }
}

/// Column widths measured once and cached in egui's frame store, keyed by
/// a fingerprint so different rows under the same id re-measure.
#[derive(Clone)]
struct CachedWidths {
    fingerprint: u64,
    widths: Vec<f32>,
}

/// One column as the table draws it: its width and which edge its cells
/// sit against.
#[derive(Clone, Copy)]
struct Column {
    width: f32,
    align: egui::Align,
}

impl<'a> DataTable<'a> {
    /// `cells` gives the text shown for a row, one cell per header column.
    pub(crate) fn new(id_source: impl Hash + Debug, header: &'a [String], row_count: usize, cells: RowCells<'a>) -> Self {
        Self {
            id: egui::Id::new(id_source),
            header,
            aligns: &[],
            row_count,
            cells,
            copy_cells: None,
            fingerprint: 0,
            max_height: f32::INFINITY,
        }
    }

    /// Which edge each column's cells sit against; numbers read best on
    /// [`egui::Align::Max`]. Columns without one sit left.
    pub(crate) fn aligns(mut self, aligns: &'a [egui::Align]) -> Self {
        self.aligns = aligns;
        self
    }

    /// The text a copy carries for a row, when it should differ from what is
    /// shown (full precision rather than display rounding).
    pub(crate) fn copy_cells(mut self, cells: RowCells<'a>) -> Self {
        self.copy_cells = Some(cells);
        self
    }

    /// What the rows were drawn from: the columns are measured once and
    /// measured again only when this changes.
    pub(crate) fn fingerprint(mut self, fingerprint: u64) -> Self {
        self.fingerprint = fingerprint;
        self
    }

    /// Scroll the rows within `height` rather than all that is left.
    pub(crate) fn max_height(mut self, height: f32) -> Self {
        self.max_height = height;
        self
    }

    /// Draw the copy actions, the header and the rows. Never reports a width
    /// wider than the ui gave it.
    pub(crate) fn show(self, ui: &mut egui::Ui) {
        let columns = self.columns(ui);
        let total_width: f32 = columns.iter().map(|column| column.width).sum();
        let row_height = ui.text_style_height(&egui::TextStyle::Body) + ROW_PADDING;

        let selection_id = self.id.with("selection");
        let drag_id = self.id.with("drag");
        let mut selection: Option<Selection> = ui.data(|data| data.get_temp(selection_id));
        let mut drag_anchor: Option<(usize, usize)> = ui.data(|data| data.get_temp(drag_id));
        let (copy_table, copy_selection) = menu::menu_actions(ui, |ui| {
            let copy_table = ui.add(menu::MenuButton::new(tr!("data-table-copy-table"))).clicked();
            let copy_selection = ui.add(menu::MenuButton::new(tr!("data-table-copy-selection")).enabled(selection.is_some())).clicked();
            (copy_table, copy_selection)
        });

        let visuals = ui.visuals();
        let dark = visuals.dark_mode;
        // A recessed sheet, so the table reads as one object on the card,
        // with bands a step off it to carry the eye along a row.
        let base = visuals.extreme_bg_color;
        let band = shifted(base, if dark { 7 } else { -8 });
        let header_fill = visuals.widgets.noninteractive.bg_fill;
        let rule = visuals.widgets.noninteractive.bg_stroke;
        let selection_fill = visuals.selection.bg_fill.gamma_multiply(0.4);
        // Painted once the table's extent is known, under everything else.
        let background = ui.painter().add(egui::Shape::Noop);

        let available = ui.available_width();
        let spacing = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = 0.0;
        // Painted after the scroll area reports its offset, so it tracks.
        let (header_rect, _) = ui.allocate_exact_size(egui::vec2(available, row_height), egui::Sense::hover());

        let mut pending: Option<Selection> = None;
        // Captured from the first row drawn, so a live drag can map the
        // pointer back to a row even if that row scrolls out of view.
        let mut table_left: Option<f32> = None;
        let mut first_row_top: Option<f32> = None;
        let list_height = self.max_height.min(ui.available_height()).max(row_height * MIN_VISIBLE_ROWS);
        let output = egui::ScrollArea::both()
            .id_salt(self.id.with("scroll"))
            .max_width(available)
            .max_height(list_height)
            .auto_shrink([false, true])
            // Turns off drag-to-scroll, which would otherwise fight the
            // rows' own press-drag for the same pointer motion.
            .scroll_source(egui::scroll_area::ScrollSource::SCROLL_BAR | egui::scroll_area::ScrollSource::MOUSE_WHEEL)
            .show_rows(ui, row_height, self.row_count, |ui, rows| {
                let clip = ui.clip_rect();
                for index in rows {
                    let (_, row_rect) = ui.allocate_space(egui::vec2(total_width.max(ui.available_width()), row_height));
                    if table_left.is_none() {
                        table_left = Some(row_rect.left());
                        first_row_top = Some(row_rect.top() - index as f32 * row_height);
                    }
                    // An explicit id, keyed by row index, keeps the row's
                    // identity as the virtualised window slides.
                    let response = ui.interact(row_rect, self.id.with(("row", index)), egui::Sense::click_and_drag());
                    if index % 2 == 1 {
                        ui.painter().rect_filled(row_rect, 0.0, band);
                    }
                    if let Some(current) = selection.filter(|current| current.rows().contains(&index)) {
                        ui.painter().rect_filled(span(&columns, row_rect, current.columns()), GROUP_CORNER_RADIUS, selection_fill);
                    }
                    draw_row(ui, row_rect, clip, &columns, self.id.with(("cells", index)), &(self.cells)(index), false);
                    if response.clicked()
                        && let Some(position) = response.interact_pointer_pos()
                    {
                        let column = column_at(&columns, row_rect.left(), position.x);
                        let extend = ui.input(|input| input.modifiers.shift);
                        pending = Some(match selection.filter(|_| extend) {
                            Some(current) => Selection {
                                anchor: current.anchor,
                                focus: (index, column),
                            },
                            None => Selection {
                                anchor: (index, column),
                                focus: (index, column),
                            },
                        });
                    }
                    // Reads `press_origin`, not the pointer: it may already be
                    // over another row by the time the drag is confirmed.
                    if response.drag_started()
                        && let Some(origin) = ui.input(|input| input.pointer.press_origin())
                    {
                        drag_anchor = Some((index, column_at(&columns, row_rect.left(), origin.x)));
                    }
                }
            });
        ui.spacing_mut().item_spacing.y = spacing;

        // Follows the pointer from here rather than from the origin row's
        // response, since that row can scroll out of view mid-drag.
        if let Some(anchor) = drag_anchor {
            if let (Some(left), Some(top), Some(position)) = (table_left, first_row_top, ui.input(|input| input.pointer.interact_pos())) {
                let row = row_at(top, row_height, self.row_count, position.y);
                let column = column_at(&columns, left, position.x);
                pending = Some(Selection { anchor, focus: (row, column) });
            }
            if !ui.input(|input| input.pointer.primary_down()) {
                drag_anchor = None;
            }
        }

        if let Some(next) = pending {
            selection = Some(next);
            ui.data_mut(|data| data.insert_temp(selection_id, next));
        }
        // Stored as the anchor itself: egui's store is keyed by id and
        // type, so writing `Option<T>` and reading `T` back would fail.
        ui.data_mut(|data| match drag_anchor {
            Some(anchor) => {
                data.insert_temp(drag_id, anchor);
            }
            None => data.remove::<(usize, usize)>(drag_id),
        });

        let sheet = header_rect.union(output.inner_rect);
        ui.painter().set(background, egui::Shape::rect_filled(sheet, GROUP_CORNER_RADIUS, base));
        // Tracks the columns horizontally but ignores them vertically, so
        // it still names the right one however far the reader has scrolled.
        let header_row = egui::Rect::from_min_size(
            egui::pos2(header_rect.left() - output.state.offset.x, header_rect.top()),
            egui::vec2(total_width, row_height),
        );
        let header_clip = header_rect.intersect(ui.clip_rect());
        let rounding = egui::CornerRadius {
            nw: GROUP_CORNER_RADIUS,
            ne: GROUP_CORNER_RADIUS,
            sw: 0,
            se: 0,
        };
        ui.painter().rect_filled(header_rect, rounding, header_fill);
        ui.painter().line_segment([header_rect.left_bottom(), header_rect.right_bottom()], rule);
        draw_row(ui, header_row, header_clip, &columns, self.id.with("header_cells"), self.header, true);
        ui.painter().rect_stroke(sheet, GROUP_CORNER_RADIUS, rule, egui::StrokeKind::Inside);

        // Ctrl+C over the table, scoped to the pointer being over it.
        let wants_shortcut = ui.rect_contains_pointer(sheet) && ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::C));
        match selection.filter(|_| copy_selection || (wants_shortcut && !copy_table)) {
            Some(current) => ui.ctx().copy_text(self.selection_text(current)),
            None if copy_table || wants_shortcut => ui.ctx().copy_text(self.table_text()),
            None => {}
        }
    }

    fn copy_row(&self, index: usize) -> Vec<String> {
        (self.copy_cells.unwrap_or(self.cells))(index)
    }

    /// The whole table as tab separated text, header included, ready to paste.
    fn table_text(&self) -> String {
        let mut rows = Vec::with_capacity(self.row_count + 1);
        rows.push(self.header.to_vec());
        rows.extend((0..self.row_count).map(|index| self.copy_row(index)));
        tsv(&rows)
    }

    /// The marked block as tab separated text, with no header row.
    fn selection_text(&self, selection: Selection) -> String {
        let columns = selection.columns();
        let rows: Vec<Vec<String>> = selection
            .rows()
            .filter(|index| *index < self.row_count)
            .map(|index| {
                let row = self.copy_row(index);
                columns.clone().filter_map(|column| row.get(column).cloned()).collect()
            })
            .collect();
        tsv(&rows)
    }

    /// The table's columns: each measured width paired with its alignment.
    fn columns(&self, ui: &egui::Ui) -> Vec<Column> {
        self.column_widths(ui)
            .into_iter()
            .enumerate()
            .map(|(index, width)| Column {
                width,
                align: self.aligns.get(index).copied().unwrap_or(egui::Align::Min),
            })
            .collect()
    }

    /// Measure each column once, keyed on what it was measured from.
    fn column_widths(&self, ui: &egui::Ui) -> Vec<f32> {
        let id = self.id.with("columns");
        let font = egui::TextStyle::Body.resolve(ui.style());
        let fingerprint = {
            use std::hash::Hasher;
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            self.fingerprint.hash(&mut hasher);
            self.row_count.hash(&mut hasher);
            self.header.hash(&mut hasher);
            font.size.to_bits().hash(&mut hasher);
            hasher.finish()
        };
        if let Some(cached) = ui.data(|data| data.get_temp::<CachedWidths>(id)).filter(|cached| cached.fingerprint == fingerprint) {
            return cached.widths;
        }

        let measure = |text: &str| ui.painter().layout_no_wrap(text.to_owned(), font.clone(), egui::Color32::PLACEHOLDER).size().x;
        let mut widths: Vec<f32> = self.header.iter().map(|label| measure(label)).collect();
        for index in 0..self.row_count.min(WIDTH_SAMPLE) {
            for (width, cell) in widths.iter_mut().zip((self.cells)(index)) {
                *width = width.max(measure(&cell));
            }
        }
        for width in &mut widths {
            *width = (*width + CELL_PADDING * 2.0).clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);
        }

        ui.data_mut(|data| {
            data.insert_temp(
                id,
                CachedWidths {
                    fingerprint,
                    widths: widths.clone(),
                },
            )
        });
        widths
    }
}

/// Serialise rows as tab separated lines, one cell per tab.
fn tsv(rows: &[Vec<String>]) -> String {
    let mut text = String::new();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if index > 0 {
                text.push('\t');
            }
            // A tab or newline inside a value travels as a space instead.
            text.extend(cell.chars().map(|character| if matches!(character, '\t' | '\n' | '\r') { ' ' } else { character }));
        }
        text.push('\n');
    }
    text
}

/// The rect a run of columns covers within `row`, for painting behind them.
fn span(columns: &[Column], row: egui::Rect, span: std::ops::RangeInclusive<usize>) -> egui::Rect {
    let start: f32 = columns.iter().take(*span.start()).map(|column| column.width).sum();
    let end: f32 = columns.iter().take(span.end().saturating_add(1)).map(|column| column.width).sum();
    egui::Rect::from_x_y_ranges(row.left() + start..=row.left() + end, row.y_range())
}

/// Which column the pointer is over, clamped to the table's own edges.
fn column_at(columns: &[Column], left: f32, x: f32) -> usize {
    let mut edge = left;
    for (index, column) in columns.iter().enumerate() {
        edge += column.width;
        if x < edge {
            return index;
        }
    }
    columns.len().saturating_sub(1)
}

/// Which row the pointer is over during a drag, clamped to the row count.
fn row_at(first_row_top: f32, row_height: f32, total_rows: usize, y: f32) -> usize {
    if total_rows == 0 || row_height <= 0.0 {
        return 0;
    }
    let offset = ((y - first_row_top) / row_height).floor();
    if offset <= 0.0 { 0 } else { (offset as usize).min(total_rows - 1) }
}

/// Lay one row of cells across `row`, each in its column's width and
/// against its column's edge, clipped to `clip`.
fn draw_row(ui: &mut egui::Ui, row: egui::Rect, clip: egui::Rect, columns: &[Column], salt: egui::Id, cells: &[String], strong: bool) {
    let mut x = row.left();
    for (index, cell) in cells.iter().enumerate() {
        let Some(&column) = columns.get(index) else {
            break;
        };
        let rect = egui::Rect::from_min_size(egui::pos2(x, row.top()), egui::vec2(column.width, row.height()));
        x += column.width;
        let visible = rect.intersect(clip);
        // Columns scrolled off either side are not laid out at all.
        if cell.is_empty() || !visible.is_positive() {
            continue;
        }
        let layout = if column.align == egui::Align::Max {
            egui::Layout::right_to_left(egui::Align::Center)
        } else {
            egui::Layout::left_to_right(egui::Align::Center)
        };
        let mut cell_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(salt.with(index))
                .max_rect(rect.shrink2(egui::vec2(CELL_PADDING, 0.0)))
                .layout(layout),
        );
        cell_ui.set_clip_rect(visible);
        let text = egui::RichText::new(cell);
        cell_ui.add(egui::Label::new(if strong { text.strong() } else { text }).truncate().selectable(false));
    }
}
