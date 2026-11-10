//! Browser-style tabs: the open one stands up out of the content under it,
//! the way a browser's open tab does, and the others sit flat on the window
//! behind. The workspace tabs in the menu bar are drawn this way, standing on
//! the viewport bar.
//!
//! The open tab is a raised surface with rounded shoulders and two small
//! flared feet, outlined in the regions' own hairline so its outline runs on
//! into the top edge of whatever region is under it. The closed tabs are just
//! labels. A closed tab under the pointer takes the open tab's shape in a
//! weaker fill, without its outline. Picking another tab slides the
//! open one across to it.
//!
//! The caller lays the tabs out and takes their clicks, so a row can space,
//! reorder or disable its tabs as it likes, and hands them to
//! [`PageTabsPaint::new`]. The open tab reaches down through the gap below the
//! row to meet the regions, so it has to be painted over their chrome: the
//! [`PageTabsPaint`] is stood on the regions' top edge once that is known
//! ([`PageTabsPaint::standing_on`]) and painted after `chrome::paint_regions`.

use std::sync::Arc;

use egui::{Color32, Pos2, Rect, Shape, Stroke, pos2, vec2};

use super::{shifted, toolbar::GROUP_CORNER_RADIUS};
use crate::ui::chrome;

/// One tab as its row laid it out this frame.
pub(crate) struct PageTab {
    /// The tab's cell in the row, which its label is centred in. The open
    /// tab's surface runs on below it down to the regions.
    pub(crate) rect: Rect,
    pub(crate) galley: Arc<egui::Galley>,
    pub(crate) hovered: bool,
    /// A tab with nothing behind it yet: its label in the weak text colour,
    /// and no hover.
    pub(crate) enabled: bool,
}

/// A row of tabs ready to paint once the regions are done.
pub(crate) struct PageTabsPaint {
    tabs: Vec<PageTab>,
    selected: Option<usize>,
    /// The open tab's span this frame, sliding towards its slot.
    open: egui::Rangef,
    /// Where the regions under the row start: the line the open tab stands on.
    base: f32,
    /// Across, the row's clip, so a row scrolled sideways stays inside its
    /// bar; down, nothing, since the open tab reaches past the row.
    clip: Rect,
    /// How far below its cell's centre each label sits.
    label_offset: f32,
    visuals: egui::Visuals,
}

impl PageTabsPaint {
    /// `tabs` as laid out this frame, with `selected` open. The open tab slides
    /// to a newly picked slot. Stands on the bottom of the row until
    /// [`Self::standing_on`] says otherwise.
    pub(crate) fn new(ui: &egui::Ui, id: egui::Id, tabs: Vec<PageTab>, selected: Option<usize>) -> Self {
        let target = selected.and_then(|index| tabs.get(index)).map_or(Rect::NOTHING, |tab| tab.rect);
        let sliding = super::tab_strip::sliding_tile(ui.ctx(), id, target);
        let open = sliding.x_range();
        let base = tabs.iter().map(|tab| tab.rect.bottom()).fold(f32::NEG_INFINITY, f32::max);
        Self {
            tabs,
            selected,
            open,
            base,
            clip: Rect::from_x_y_ranges(ui.clip_rect().x_range(), egui::Rangef::EVERYTHING),
            label_offset: 0.0,
            visuals: ui.visuals().clone(),
        }
    }

    /// Stand the open tab on `base`, the top of the regions' fills under the row.
    pub(crate) fn standing_on(mut self, base: f32) -> Self {
        self.base = base;
        self
    }

    /// Sit each label `offset` below its cell's centre, to line it up with
    /// whatever else its row carries. The tabs' shapes stay where they are.
    pub(crate) fn label_offset(mut self, offset: f32) -> Self {
        self.label_offset = offset;
        self
    }

    /// Paint the row. Call after `chrome::paint_regions`, so the open tab
    /// covers the gap between its row and the regions under it.
    pub(crate) fn paint(&self, ctx: &egui::Context) {
        let painter = ctx.layer_painter(egui::LayerId::background()).with_clip_rect(self.clip);
        let visuals = &self.visuals;
        let rule = chrome::region_stroke(visuals);
        // Partway from the window to the open tab's fill.
        let hover_fill = shifted(chrome::window_fill(visuals), if visuals.dark_mode { 6 } else { 9 });
        let surface = |tab: &PageTab, span: egui::Rangef| Rect::from_x_y_ranges(span, tab.rect.top()..=self.base);

        // Under the open tab, so it stays whole while it slides across.
        for (index, tab) in self.tabs.iter().enumerate() {
            if tab.hovered && tab.enabled && Some(index) != self.selected {
                // A pixel short of the base, clear of the region's top edge:
                // the edge stroke would darken that hairline where the feet
                // meet it, and the fill is the window's colour near enough
                // that the gap does not show.
                let base = self.base - 1.0;
                let outline = tab_outline(Rect::from_x_y_ranges(tab.rect.x_range(), tab.rect.top()..=base), base);
                fill_tab(&painter, &outline, base, hover_fill);
                // Edged in its own colour, which smooths the mesh's edges.
                painter.add(Shape::line(outline, Stroke::new(1.0, hover_fill)));
            }
        }

        if let Some(open) = self.selected.and_then(|index| self.tabs.get(index))
            && self.open.span() > 0.0
        {
            let outline = tab_outline(surface(open, self.open), self.base);
            fill_tab(&painter, &outline, self.base, visuals.panel_fill);
            // The region's own top edge, under the tab from one foot to the
            // other, painted out in the tab's fill: the two are one surface,
            // and the edge runs up into the feet and over the tab instead.
            // The hairline is drawn inside the region, so it is the pixel row
            // just under the base; half a pixel either side takes its
            // feathering too.
            if let (Some(first), Some(last)) = (outline.first(), outline.last()) {
                let seam = Rect::from_x_y_ranges(first.x..=last.x, self.base - 0.5..=self.base + 1.5);
                painter.rect_filled(seam, 0.0, visuals.panel_fill);
            }
            // Stroked along everything but the base, over that.
            painter.add(Shape::line(outline, rule));
        }

        for (index, tab) in self.tabs.iter().enumerate() {
            let color = if !tab.enabled {
                visuals.weak_text_color()
            } else if Some(index) == self.selected || tab.hovered {
                visuals.strong_text_color()
            } else {
                visuals.text_color()
            };
            let at = tab.rect.center() - tab.galley.size() / 2.0 + vec2(0.0, self.label_offset);
            painter.galley(at, tab.galley.clone(), color);
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
