//! A row of tabs as labels on the window behind the regions, the open one on a
//! raised tile that slides across to the next one picked: Planning's page
//! tabs (`elements::planning_tabs`). A closed tab under the pointer lifts off
//! the window in a weaker fill.
//!
//! Every tab is one fixed width, so the row does not reflow from one language
//! to the next. A row whose tabs are named by the user's own data fits them
//! to their labels instead, and a tab that can be closed carries a cross.
//!
//! The workspace tabs in the menu bar are the other kind, standing up out of
//! the regions under them - see `widgets::page_tabs`, which slides its open
//! tab with [`sliding_tile`] too.

use std::hash::Hash;

use super::{shifted, toolbar::GROUP_CORNER_RADIUS};

/// How long the tile takes to slide to a newly picked tab.
const SLIDE_SECONDS: f32 = 0.15;
/// Every tab's width, whatever its label.
const TAB_WIDTH: f32 = 140.0;
/// Space either side of a label, which a longer one is shortened inside.
const LABEL_PADDING: f32 = 10.0;
/// Space between two tabs.
const TAB_GAP: f32 = 4.0;
/// The close cross on a tab that has one, and the gap between it and the
/// label.
const CLOSE_SIZE: f32 = 16.0;
const CLOSE_GAP: f32 = 4.0;

/// The raised tile under the open tab, at `rect`.
pub(crate) fn paint_tile(painter: &egui::Painter, visuals: &egui::Visuals, rect: egui::Rect) {
    let fill = shifted(visuals.panel_fill, if visuals.dark_mode { 22 } else { 12 });
    painter.rect(rect, GROUP_CORNER_RADIUS, fill, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
}

/// Where the tile is this frame, sliding towards `target`, the open tab.
pub(crate) fn sliding_tile(ctx: &egui::Context, id: egui::Id, target: egui::Rect) -> egui::Rect {
    let left = ctx.animate_value_with_time(id.with("tile_left"), target.left(), SLIDE_SECONDS);
    let right = ctx.animate_value_with_time(id.with("tile_right"), target.right(), SLIDE_SECONDS);
    egui::Rect::from_x_y_ranges(left..=right, target.y_range())
}

/// Tabs over one workspace's pages.
pub(crate) struct TileTabs<'a, H> {
    id_salt: H,
    labels: &'a [String],
    selected: usize,
    closable: &'a [bool],
    fitted: bool,
}

/// What a row of [`TileTabs`] did this frame.
pub(crate) struct TileTabsResponse {
    /// The tab clicked, when it was not already the open one.
    pub(crate) clicked: Option<usize>,
    /// The tab whose close cross was pressed, or that was middle-clicked.
    pub(crate) closed: Option<usize>,
    /// How wide the tabs came out together, for a row that scrolls.
    pub(crate) width: f32,
    pub(crate) paint: TileTabsPaint,
}

impl<'a, H: Hash + std::fmt::Debug> TileTabs<'a, H> {
    pub(crate) fn new(id_salt: H, labels: &'a [String], selected: usize) -> Self {
        Self {
            id_salt,
            labels,
            selected,
            closable: &[],
            fitted: false,
        }
    }

    /// Which tabs carry a close cross, by index; those past the end do not.
    pub(crate) fn closable(mut self, closable: &'a [bool]) -> Self {
        self.closable = closable;
        self
    }

    /// Make each tab as wide as its label, never narrower than the fixed
    /// width: for tabs named by the user's own data, which that width would
    /// cut short.
    pub(crate) fn fitted(mut self) -> Self {
        self.fitted = true;
        self
    }

    /// Lay the tabs out from the left of `row`, the full height of it, and
    /// take their clicks: [`TAB_WIDTH`] each, or their labels' widths when
    /// [`Self::fitted`].
    ///
    /// Painted later, over the regions' chrome: a row between two regions has
    /// their seam bands along its top and bottom, which would cut the tiles'
    /// rounded corners off square.
    pub(crate) fn show(self, ui: &mut egui::Ui, row: egui::Rect) -> TileTabsResponse {
        let id = ui.scope_id().with(&self.id_salt);
        let font = egui::TextStyle::Button.resolve(ui.style());
        let mut left = row.left();
        let mut clicked = None;
        let mut closed = None;
        let mut target = egui::Rect::NOTHING;
        let mut tabs = Vec::new();
        for (index, label) in self.labels.iter().enumerate() {
            let closable = self.closable.get(index).copied().unwrap_or(false);
            // What the cross takes of the tab, beside the label.
            let cross = if closable { CLOSE_SIZE + CLOSE_GAP } else { 0.0 };
            let mut job = egui::text::LayoutJob::simple_singleline(label.clone(), font.clone(), egui::Color32::PLACEHOLDER);
            job.wrap.max_width = if self.fitted { f32::INFINITY } else { TAB_WIDTH - 2.0 * LABEL_PADDING - cross };
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            let galley = ui.painter().layout_job(job);
            let width = if self.fitted {
                (galley.size().x + 2.0 * LABEL_PADDING + cross).max(TAB_WIDTH)
            } else {
                TAB_WIDTH
            };
            let rect = egui::Rect::from_min_size(egui::pos2(left, row.top()), egui::vec2(width, row.height()));
            left = rect.right() + TAB_GAP;
            let response = ui.interact(rect, id.with(index), egui::Sense::click());
            let chosen = index == self.selected;
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, galley.text()));
            // After the tab, so the cross takes the clicks that land on it.
            let close = closable.then(|| {
                let at = egui::Rect::from_center_size(
                    egui::pos2(rect.right() - LABEL_PADDING * 0.6 - CLOSE_SIZE / 2.0, rect.center().y),
                    egui::Vec2::splat(CLOSE_SIZE),
                );
                let response = ui
                    .interact(at, id.with((index, "close")), egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                (at, response)
            });
            let close_hovered = close.as_ref().is_some_and(|(_, close)| close.hovered());
            if close.as_ref().is_some_and(|(_, close)| close.clicked()) || (closable && response.middle_clicked()) {
                closed = Some(index);
            } else if response.clicked() && !chosen {
                clicked = Some(index);
            }
            if chosen {
                target = rect;
            }
            tabs.push(TileTab {
                rect,
                galley,
                chosen,
                hovered: response.hovered() || close_hovered,
                close: close.map(|(at, _)| (at, close_hovered)),
            });
        }
        let paint = TileTabsPaint {
            tile: target.is_positive().then(|| sliding_tile(ui.ctx(), id, target)),
            tabs,
            clip: row.expand(1.0).intersect(ui.clip_rect()),
            visuals: ui.visuals().clone(),
        };
        TileTabsResponse {
            clicked,
            closed,
            width: (left - TAB_GAP - row.left()).max(0.0),
            paint,
        }
    }
}

struct TileTab {
    rect: egui::Rect,
    galley: std::sync::Arc<egui::Galley>,
    chosen: bool,
    hovered: bool,
    /// The tab's close cross, when it has one: where, and whether the
    /// pointer is on it.
    close: Option<(egui::Rect, bool)>,
}

/// What [`TileTabs::show`] laid out, for painting once the regions are done.
pub(crate) struct TileTabsPaint {
    tabs: Vec<TileTab>,
    /// The open tab's tile this frame, sliding towards its slot.
    tile: Option<egui::Rect>,
    clip: egui::Rect,
    visuals: egui::Visuals,
}

impl TileTabsPaint {
    /// Paint the tabs. Call after `chrome::paint_regions`.
    pub(crate) fn paint(&self, ctx: &egui::Context) {
        let painter = ctx.layer_painter(egui::LayerId::background()).with_clip_rect(self.clip);
        let visuals = &self.visuals;
        let window = crate::ui::chrome::window_fill(visuals);
        for tab in &self.tabs {
            if tab.hovered && !tab.chosen {
                // The step signed by theme, so "lighter" means away from the
                // background in both.
                painter.rect_filled(tab.rect, GROUP_CORNER_RADIUS, shifted(window, if visuals.dark_mode { 6 } else { -6 }));
            }
        }
        if let Some(tile) = self.tile {
            paint_tile(&painter, visuals, tile);
        }
        for tab in &self.tabs {
            let color = if tab.chosen || tab.hovered { visuals.strong_text_color() } else { visuals.text_color() };
            // The label is centred in what the cross leaves of the tab.
            let cell = tab.close.map_or(tab.rect, |(close, _)| {
                egui::Rect::from_min_max(tab.rect.min, egui::pos2(close.left() - CLOSE_GAP + LABEL_PADDING, tab.rect.bottom()))
            });
            painter.galley(cell.center() - tab.galley.size() / 2.0, tab.galley.clone(), color);
            if let Some((close, hot)) = tab.close {
                if hot {
                    painter.rect_filled(close, GROUP_CORNER_RADIUS, shifted(window, if visuals.dark_mode { 34 } else { -34 }));
                }
                let mark = close.shrink(close.width() * 0.3);
                let stroke = egui::Stroke::new(1.3, if hot { visuals.strong_text_color() } else { visuals.weak_text_color() });
                painter.line_segment([mark.left_top(), mark.right_bottom()], stroke);
                painter.line_segment([mark.right_top(), mark.left_bottom()], stroke);
            }
        }
    }
}
