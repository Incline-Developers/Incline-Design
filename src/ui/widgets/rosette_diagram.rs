//! A plan view of a pit's overall slope rosette: an oval split into sectors by
//! bearing, each coloured and labelled with its wall angle.
//!
//! Bearings run clockwise from north at the top. Each sector is where one
//! rosette row applies - from its bearing up to the next row's - so the picture
//! is the rosette's meaning at a glance: one row is the whole oval in one
//! colour, two rows are two halves, and so on.

use crate::{i18n::tr, model::optimization::SlopeSector};

const HEIGHT: f32 = 300.0;
const MAX_RADIUS_X: f32 = 200.0;
const MAX_RADIUS_Y: f32 = 105.0;
/// Angle between the points a sector's rim is cut into.
const RIM_STEP_DEGREES: f64 = 3.0;
/// Wall angles the colour ramp runs between.
const RAMP_FROM: f64 = 20.0;
const RAMP_TO: f64 = 80.0;

/// Draw the oval for `sectors` (in bearing order), centred in the width available.
pub(crate) fn draw_rosette(ui: &mut egui::Ui, sectors: &[SlopeSector]) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), HEIGHT), egui::Sense::hover());
    let centre = rect.center();
    let radius_x = MAX_RADIUS_X.min(rect.width() / 2.0 - 40.0).max(60.0);
    let radius_y = MAX_RADIUS_Y.min(radius_x * 0.55);
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let line = visuals.widgets.noninteractive.fg_stroke.color;
    let weak = visuals.weak_text_color();
    let on_oval = |bearing: f64, scale: f32| {
        let angle = bearing.to_radians();
        centre + egui::vec2(radius_x * scale * angle.sin() as f32, -radius_y * scale * angle.cos() as f32)
    };

    // The sectors, each a fan of triangles from the centre to its rim.
    for sector in sectors {
        let colour = angle_colour(sector.angle, visuals.dark_mode);
        let steps = (((sector.to - sector.from) / RIM_STEP_DEGREES).ceil() as usize).max(1);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(centre, colour);
        for step in 0..=steps {
            let bearing = sector.from + (sector.to - sector.from) * step as f64 / steps as f64;
            mesh.colored_vertex(on_oval(bearing, 1.0), colour);
        }
        for step in 0..steps as u32 {
            mesh.add_triangle(0, step + 1, step + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
    }

    // The outline, and where each sector starts.
    let outline: Vec<egui::Pos2> = (0..=120).map(|step| on_oval(f64::from(step) * 3.0, 1.0)).collect();
    painter.add(egui::Shape::line(outline, egui::Stroke::new(1.5, line)));
    let boundaries = sectors.len() > 1;
    for sector in sectors {
        if boundaries {
            painter.line_segment([centre, on_oval(sector.from, 1.0)], egui::Stroke::new(1.0, line));
        }
        painter.text(
            on_oval(sector.from, 1.0) + (on_oval(sector.from, 1.0) - centre).normalized() * 14.0,
            egui::Align2::CENTER_CENTER,
            format!("{}°", trim_number(sector.from.rem_euclid(360.0))),
            egui::TextStyle::Small.resolve(ui.style()),
            weak,
        );
        let middle = (sector.from + sector.to) / 2.0;
        painter.text(
            on_oval(middle, 0.58),
            egui::Align2::CENTER_CENTER,
            format!("{}°", trim_number(sector.angle)),
            egui::FontId::proportional(18.0),
            line,
        );
    }

    // The compass, so the bearings read as bearings.
    for (bearing, letter) in [
        (0.0, tr!("opt-compass-north")),
        (90.0, tr!("opt-compass-east")),
        (180.0, tr!("opt-compass-south")),
        (270.0, tr!("opt-compass-west")),
    ] {
        let direction = (on_oval(bearing, 1.0) - centre).normalized();
        let anchor = on_oval(bearing, 1.0) + direction * if bearing == 0.0 || bearing == 180.0 { 30.0 } else { 40.0 };
        painter.text(anchor, egui::Align2::CENTER_CENTER, letter, egui::FontId::proportional(13.0), weak);
    }
    response.on_hover_text(tr!("opt-rosette-diagram-hover"));
}

/// Steep walls warm, shallow ones cool, so a rosette's tightest side stands out.
fn angle_colour(angle: f64, dark_mode: bool) -> egui::Color32 {
    let t = ((angle - RAMP_FROM) / (RAMP_TO - RAMP_FROM)).clamp(0.0, 1.0) as f32;
    let hue = 0.58 - 0.5 * t;
    let value = if dark_mode { 0.85 } else { 0.95 };
    egui::ecolor::Hsva::new(hue, 0.5, value, 0.55).into()
}

fn trim_number(value: f64) -> String {
    if (value - value.round()).abs() < 1e-6 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}
