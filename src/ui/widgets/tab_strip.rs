//! A row of tabs as labels on the window behind the regions, the open one on a
//! raised tile that slides across to the next one picked: Planning's page
//! tabs (`elements::planning_tabs`). A closed tab under the pointer lifts off
//! the window in a weaker fill.
//!
//! Every tab is one fixed width, so the row does not reflow from one language
//! to the next.
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

/// The raised tile under the open tab, at `rect`.
fn paint_tile(painter: &egui::Painter, visuals: &egui::Visuals, rect: egui::Rect) {
    let fill = shifted(visuals.panel_fill, if visuals.dark_mode { 22 } else { 12 });
    painter.rect(rect, GROUP_CORNER_RADIUS, fill, visuals.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
}

/// Where the tile is this frame, sliding towards `target`, the open tab.
pub(crate) fn sliding_tile(ctx: &egui::Context, id: egui::Id, target: egui::Rect) -> egui::Rect {
    let left = ctx.animate_value_with_time(id.with("tile_left"), target.left(), SLIDE_SECONDS);
    let right = ctx.animate_value_with_time(id.with("tile_right"), target.right(), SLIDE_SECONDS);
    egui::Rect::from_x_y_ranges(left..=right, target.y_range())
}

/// Tabs over one workspace's pages, each as wide as its label.
pub(crate) struct TileTabs<'a, H> {
    id_salt: H,
    labels: &'a [String],
    selected: usize,
}

impl<'a, H: Hash + std::fmt::Debug> TileTabs<'a, H> {
    pub(crate) fn new(id_salt: H, labels: &'a [String], selected: usize) -> Self {
        Self { id_salt, labels, selected }
    }

    /// Lay the tabs out from the left of `row`, the full height of it and
    /// [`TAB_WIDTH`] each, and take their clicks. Returns the index clicked
    /// this frame, and the tabs to paint.
    ///
    /// Painted later, over the regions' chrome: a row between two regions has
    /// their seam bands along its top and bottom, which would cut the tiles'
    /// rounded corners off square.
    pub(crate) fn show(self, ui: &mut egui::Ui, row: egui::Rect) -> (Option<usize>, TileTabsPaint) {
        let id = ui.scope_id().with(&self.id_salt);
        let font = egui::TextStyle::Button.resolve(ui.style());
        let mut left = row.left();
        let mut clicked = None;
        let mut target = egui::Rect::NOTHING;
        let mut tabs = Vec::new();
        for (index, label) in self.labels.iter().enumerate() {
            let mut job = egui::text::LayoutJob::simple_singleline(label.clone(), font.clone(), egui::Color32::PLACEHOLDER);
            job.wrap.max_width = TAB_WIDTH - 2.0 * LABEL_PADDING;
            job.wrap.max_rows = 1;
            job.wrap.break_anywhere = true;
            let galley = ui.painter().layout_job(job);
            let rect = egui::Rect::from_min_size(egui::pos2(left, row.top()), egui::vec2(TAB_WIDTH, row.height()));
            left = rect.right() + TAB_GAP;
            let response = ui.interact(rect, id.with(index), egui::Sense::click());
            let chosen = index == self.selected;
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, galley.text()));
            if response.clicked() && !chosen {
                clicked = Some(index);
            }
            if chosen {
                target = rect;
            }
            tabs.push(TileTab {
                rect,
                galley,
                chosen,
                hovered: response.hovered(),
            });
        }
        let paint = TileTabsPaint {
            tile: target.is_positive().then(|| sliding_tile(ui.ctx(), id, target)),
            tabs,
            clip: row.expand(1.0).intersect(ui.clip_rect()),
            visuals: ui.visuals().clone(),
        };
        (clicked, paint)
    }
}

struct TileTab {
    rect: egui::Rect,
    galley: std::sync::Arc<egui::Galley>,
    chosen: bool,
    hovered: bool,
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
            painter.galley(tab.rect.center() - tab.galley.size() / 2.0, tab.galley.clone(), color);
        }
    }
}
