//! A row of browser-style tabs across the top of a workspace: the open page
//! stands up out of the content under it, the way a browser's open tab does,
//! and the others sit flat on the window behind.
//!
//! The open tab is a raised surface with rounded shoulders and two small
//! flared feet, outlined in the regions' own hairline so its outline runs on
//! into the top edge of whatever region is under it. The closed tabs are just
//! labels, parted by short rules that give way around the open tab and the
//! one under the pointer. A closed tab under the pointer takes the open tab's
//! shape in a weaker fill, without its outline. Picking another tab slides the
//! open one across to it.
//!
//! The row is not a region. Its open tab reaches down through the gap below it
//! to meet the regions, so it has to be painted over their chrome: [`show`]
//! lays the row out and takes its clicks, and the [`PageTabsPaint`] it returns
//! is painted after `chrome::paint_regions`.
//!
//! [`show`]: PageTabs::show

use std::{hash::Hash, sync::Arc};

use egui::{Color32, Pos2, Rect, Shape, Stroke, pos2, vec2};

use super::{shifted, toolbar::GROUP_CORNER_RADIUS};
use crate::ui::chrome;

/// The widest a tab grows: past this the labels would float in empty tabs.
const MAX_TAB_WIDTH: f32 = 150.0;
/// Space either side of a label, kept even when the row is squeezed.
const LABEL_PADDING: f32 = 10.0;
/// How far the separators stand off the tab's top and bottom, so they sit
/// inside the row rather than running its full height.
const RULE_INSET: f32 = 6.0;

/// The tabs over one workspace's pages.
pub(crate) struct PageTabs<'a, H> {
    id_salt: H,
    labels: &'a [String],
    selected: usize,
}

/// One closed or open tab, ready to paint.
struct Tab {
    rect: Rect,
    galley: Arc<egui::Galley>,
    hovered: bool,
}

/// What [`PageTabs::show`] laid out, for painting once the regions are done.
pub(crate) struct PageTabsPaint {
    tabs: Vec<Tab>,
    selected: usize,
    /// The open tab's rect this frame, sliding towards its slot.
    open: Rect,
    /// Where the regions under the row start: the line the open tab stands on.
    base: f32,
    visuals: egui::Visuals,
}

impl<'a, H: Hash + std::fmt::Debug> PageTabs<'a, H> {
    pub(crate) fn new(id_salt: H, labels: &'a [String], selected: usize) -> Self {
        Self { id_salt, labels, selected }
    }

    /// Lay the tabs out across `row`, standing on `base` - the top of the
    /// regions' fills under it - and take their clicks. Returns the index
    /// clicked this frame, and what to paint after the regions.
    pub(crate) fn show(self, ui: &mut egui::Ui, row: Rect, base: f32) -> (Option<usize>, PageTabsPaint) {
        let id = ui.scope_id().with(&self.id_salt);
        let font = egui::TextStyle::Button.resolve(ui.style());
        let galleys: Vec<_> = self
            .labels
            .iter()
            .map(|label| ui.painter().layout_no_wrap(label.clone(), font.clone(), Color32::PLACEHOLDER))
            .collect();
        // Every tab one width, as a browser's are: they shrink together when
        // the window does, down to the widest label.
        let widest = galleys.iter().map(|galley| galley.size().x).fold(0.0, f32::max) + 2.0 * LABEL_PADDING;
        let count = self.labels.len().max(1) as f32;
        // The feet reach a radius past the end tabs; keep them inside the row.
        let feet = f32::from(GROUP_CORNER_RADIUS);
        let width = ((row.width() - 2.0 * feet) / count).clamp(widest, MAX_TAB_WIDTH.max(widest));
        let left = row.left() + feet;

        let mut clicked = None;
        let tabs: Vec<Tab> = galleys
            .into_iter()
            .enumerate()
            .map(|(index, galley)| {
                let rect = Rect::from_min_max(pos2(left + width * index as f32, row.top()), pos2(left + width * (index + 1) as f32, base));
                // Clicks only over the row itself: the strip the open tab
                // reaches down through belongs to the region margins below.
                let response = ui.interact(rect.intersect(row), id.with(index), egui::Sense::click());
                let chosen = index == self.selected;
                response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, galley.text()));
                if response.clicked() && !chosen {
                    clicked = Some(index);
                }
                Tab {
                    rect,
                    galley,
                    hovered: response.hovered(),
                }
            })
            .collect();

        let target = tabs.get(self.selected).map_or(Rect::NOTHING, |tab| tab.rect);
        let open = super::tab_strip::sliding_tile(ui.ctx(), id, target);
        let paint = PageTabsPaint {
            tabs,
            selected: self.selected,
            open,
            base,
            visuals: ui.visuals().clone(),
        };
        (clicked, paint)
    }
}

impl PageTabsPaint {
    /// Paint the row. Call after `chrome::paint_regions`, so the open tab
    /// covers the gap between its row and the regions under it.
    pub(crate) fn paint(&self, ctx: &egui::Context) {
        let painter = ctx.layer_painter(egui::LayerId::background());
        let visuals = &self.visuals;
        let rule = chrome::region_stroke(visuals);
        // Partway from the window to the open tab's fill.
        let hover_fill = shifted(chrome::window_fill(visuals), if visuals.dark_mode { 6 } else { 9 });

        // The rules between closed tabs: one at each inner seam, unless the
        // open tab or the one under the pointer is on either side of it - a
        // browser drops them there, so the lit tab reads as one shape.
        let open_span = self.open.x_range();
        for pair in self.tabs.windows(2) {
            let x = pair[0].rect.right();
            let beside_open = (open_span.min - 1.0..=open_span.max + 1.0).contains(&x);
            if beside_open || pair[0].hovered || pair[1].hovered {
                continue;
            }
            let top = pair[0].rect.top() + RULE_INSET;
            let bottom = self.base - RULE_INSET;
            if bottom > top {
                painter.line_segment([pos2(x, top), pos2(x, bottom)], Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color));
            }
        }

        // Under the open tab, so it stays whole while it slides across.
        for (index, tab) in self.tabs.iter().enumerate() {
            if tab.hovered && index != self.selected {
                let outline = tab_outline(tab.rect, self.base);
                fill_tab(&painter, &outline, self.base, hover_fill);
                // Edged in its own colour, which smooths the mesh's edges.
                painter.add(Shape::line(outline, Stroke::new(1.0, hover_fill)));
            }
        }

        if self.open.is_positive() {
            let outline = tab_outline(self.open, self.base);
            fill_tab(&painter, &outline, self.base, visuals.panel_fill);
            // Not stroked along the base: the region's own top edge runs on
            // under the tab, so the tab reads as standing on whatever is below
            // it, scene or panel.
            painter.add(Shape::line(outline, rule));
        }

        for (index, tab) in self.tabs.iter().enumerate() {
            let chosen = index == self.selected;
            let color = if chosen || tab.hovered { visuals.strong_text_color() } else { visuals.text_color() };
            painter.galley(tab.rect.center() - tab.galley.size() / 2.0, tab.galley.clone(), color);
        }
    }
}

/// Fill a tab's `outline` down to `base`.
///
/// The outline starts and ends on the base, at the tips of the feet, so the
/// fill closes along it. Fanned from the middle of the base, which sees every
/// point of the outline: the shape is not convex at its feet.
fn fill_tab(painter: &egui::Painter, outline: &[Pos2], base: f32, color: Color32) {
    let (Some(first), Some(last)) = (outline.first(), outline.last()) else {
        return;
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(pos2((first.x + last.x) / 2.0, base), color);
    for point in outline {
        mesh.colored_vertex(*point, color);
    }
    let points = outline.len() as u32;
    for index in 0..points {
        mesh.add_triangle(0, 1 + index, 1 + (index + 1) % points);
    }
    painter.add(Shape::mesh(mesh));
}

/// A tab's outline, from the tip of its left foot to the tip of its
/// right: up out of the base with a concave flare, over two rounded
/// shoulders, and back down into the base the same way.
fn tab_outline(tab: Rect, base: f32) -> Vec<Pos2> {
    let radius = f32::from(GROUP_CORNER_RADIUS);
    let mut points = Vec::new();
    // Half a pixel in, so the 1-point hairline lands on whole pixels.
    let (left, right, top) = (tab.left() + 0.5, tab.right() - 0.5, tab.top() + 0.5);
    let arc = |points: &mut Vec<Pos2>, centre: Pos2, from: f32, to: f32| {
        const STEPS: usize = 6;
        for step in 0..=STEPS {
            let angle = from + (to - from) * step as f32 / STEPS as f32;
            points.push(centre + radius * vec2(angle.cos(), angle.sin()));
        }
    };
    use std::f32::consts::{FRAC_PI_2, PI};
    // Left foot: centred out beside the tab, sweeping from the base up to its side.
    arc(&mut points, pos2(left - radius, base - radius), FRAC_PI_2, 0.0);
    // Left shoulder.
    arc(&mut points, pos2(left + radius, top + radius), PI, PI + FRAC_PI_2);
    // Right shoulder.
    arc(&mut points, pos2(right - radius, top + radius), -FRAC_PI_2, 0.0);
    // Right foot.
    arc(&mut points, pos2(right + radius, base - radius), PI, FRAC_PI_2);
    points
}
