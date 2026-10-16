//! Text laid along a line on screen, the way a map labels its contours:
//! turned to run with the line, never upside down, and skipped where it
//! would cover a label already placed.

const LABEL_CASING_WIDTH: f32 = 1.5;
/// Side of a cell of the grid placed labels are filed on, in points: a few
/// labels across, so a new one is tested against its neighbours only.
const PLACED_CELL: f32 = 64.0;

/// The labels already placed this frame, each kept as its box grown by its
/// own gap and filed on a coarse grid of the screen. One set is shared by
/// every overlay that labels lines, so their labels never cover each other.
#[derive(Default)]
pub(crate) struct PlacedLabels {
    boxes: Vec<egui::Rect>,
    cells: std::collections::HashMap<(i32, i32), Vec<usize>>,
}

impl PlacedLabels {
    fn cells_of(rect: egui::Rect) -> impl Iterator<Item = (i32, i32)> {
        let cell = |value: f32| (value / PLACED_CELL).floor() as i32;
        (cell(rect.min.x)..=cell(rect.max.x)).flat_map(move |x| (cell(rect.min.y)..=cell(rect.max.y)).map(move |y| (x, y)))
    }

    fn clear_of(&self, bounds: egui::Rect) -> bool {
        Self::cells_of(bounds).all(|cell| self.cells.get(&cell).is_none_or(|filed| filed.iter().all(|&index| !self.boxes[index].intersects(bounds))))
    }

    fn push(&mut self, corners: &[egui::Pos2; 4], gap: f32) {
        let grown = egui::Rect::from_points(corners).expand(gap);
        let index = self.boxes.len();
        self.boxes.push(grown);
        for cell in Self::cells_of(grown) {
            self.cells.entry(cell).or_default().push(index);
        }
    }
}

/// How a line's labels look: a box filled in `fill`, edged in `casing`, with
/// the text in `ink` at `scale` times the usual size, and the clear space
/// each keeps around it.
#[derive(Clone, Copy)]
pub(crate) struct LabelStyle {
    pub(crate) fill: egui::Color32,
    pub(crate) casing: egui::Color32,
    pub(crate) ink: egui::Color32,
    pub(crate) scale: f32,
    pub(crate) gap: LabelGap,
}

/// Clear space kept around a placed label.
#[derive(Clone, Copy)]
pub(crate) enum LabelGap {
    /// A fixed gap, in points.
    Points(f32),
    /// So many of the label's own widths.
    Widths(f32),
}

impl LabelGap {
    pub(crate) fn points(self, label_width: f32) -> f32 {
        match self {
            Self::Points(points) => points,
            Self::Widths(widths) => widths * label_width,
        }
    }
}

/// The angle of a label standing at `anchor` and running toward `ahead`,
/// folded so it never reads upside down, and the corners of its box: text
/// `text_size` plus `padding` all round.
pub(crate) fn label_box(anchor: egui::Pos2, ahead: egui::Pos2, text_size: egui::Vec2, padding: egui::Vec2) -> (f32, [egui::Pos2; 4]) {
    let direction = (ahead - anchor).normalized();
    let angle = crate::model::geometry::upright(direction.y.atan2(direction.x));
    let half = text_size * 0.5 + padding;
    let rotation = egui::emath::Rot2::from_angle(angle);
    let corners = [
        egui::vec2(-half.x, -half.y),
        egui::vec2(half.x, -half.y),
        egui::vec2(half.x, half.y),
        egui::vec2(-half.x, half.y),
    ]
    .map(|corner| anchor + rotation * corner);
    (angle, corners)
}

/// Whether a label's box lies wholly on the canvas and clear of every label
/// already `placed`.
pub(crate) fn label_fits(corners: &[egui::Pos2; 4], canvas_rect: egui::Rect, placed: &PlacedLabels) -> bool {
    let bounds = egui::Rect::from_points(corners);
    canvas_rect.contains_rect(bounds) && placed.clear_of(bounds)
}

/// [`line_label_shapes`] painted at once.
pub(crate) fn draw_line_labels(
    painter: &egui::Painter,
    canvas_rect: egui::Rect,
    placed: &mut PlacedLabels,
    text: &str,
    style: LabelStyle,
    spots: impl IntoIterator<Item = (egui::Pos2, egui::Pos2)>,
) {
    let mut shapes = Vec::new();
    line_label_shapes(painter, canvas_rect, placed, text, style, spots, &mut shapes);
    painter.extend(shapes);
}

/// Label one line with `text` at each of its `spots` (where the label stands
/// and a point further along the line, in points). Labels that fit are added
/// to `shapes`, for the caller to paint where it wants them in the stack,
/// and to `placed`, which callers share across lines.
pub(crate) fn line_label_shapes(
    painter: &egui::Painter,
    canvas_rect: egui::Rect,
    placed: &mut PlacedLabels,
    text: &str,
    style: LabelStyle,
    spots: impl IntoIterator<Item = (egui::Pos2, egui::Pos2)>,
    shapes: &mut Vec<egui::Shape>,
) {
    let text_size = crate::model::geometry::LABEL_TEXT_POINTS as f32 * style.scale;
    let font = egui::FontId::new(text_size, egui::FontFamily::Name("noto_sans_bold".into()));
    let galley = painter.layout_no_wrap(text.to_owned(), font, style.ink);
    let padding = crate::model::geometry::label_padding(f64::from(text_size)).as_vec2();
    let padding = egui::vec2(padding.x, padding.y);
    let gap = style.gap.points(galley.size().x + padding.x * 2.0);
    for (anchor, ahead) in spots {
        let (angle, corners) = label_box(anchor, ahead, galley.size(), padding);
        if !label_fits(&corners, canvas_rect, placed) {
            continue;
        }
        shapes.push(egui::Shape::convex_polygon(
            corners.to_vec(),
            style.fill,
            egui::Stroke::new(LABEL_CASING_WIDTH, style.casing),
        ));
        let origin = anchor - egui::emath::Rot2::from_angle(angle) * (galley.size() * 0.5);
        shapes.push(egui::epaint::TextShape::new(origin, galley.clone(), style.ink).with_angle(angle).into());
        placed.push(&corners, gap);
    }
}
