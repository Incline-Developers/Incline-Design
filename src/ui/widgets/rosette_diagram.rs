//! A plan view of a pit's overall slope rosette: a circle split into sectors by
//! bearing, with a small pit drawn inside it, each in its own colour and labelled with its wall angle.
//!
//! Bearings run clockwise from north at the top. Each sector is where one
//! rosette row applies - from its bearing up to the next row's - so the picture
//! is the rosette's meaning at a glance: one row is the whole oval in one
//! colour, two rows are two halves, and so on.

use crate::{i18n::tr, model::optimization::SlopeSector};

pub(crate) const HEIGHT: f32 = 300.0;
/// Room kept outside the circle for the compass letters and bearings.
const LABEL_MARGIN: f32 = 36.0;
/// Toe lines of the pit's benches as fractions of the circle's radius, from
/// the highest bench in to the floor. Each is free-form; the crest above it is
/// worked out from it.
const PIT_TOES: [f32; 3] = [0.72, 0.50, 0.28];
/// Horizontal run of every bench face as a fraction of the circle's radius:
/// the face angle and bench height are constant, so each crest lies this far
/// outside its toe, all the way round.
const PIT_FACE_RUN: f32 = 0.06;
const PIT_STEPS: u32 = 72;
/// The pit is narrower than it is long (east-west against north-south).
const PIT_WIDTH: f32 = 0.78;
/// Keeps the most uneven point of the outline inside the circle.
const PIT_FIT: f32 = 0.82;
/// Angle between the points a sector's rim is cut into.
const RIM_STEP_DEGREES: f64 = 3.0;
/// Twenty calm, muted colours - neutral earth and slate tones rather than
/// saturated ones - as unlike each other as that allows, handed out to the sectors in order and
/// started over after the twentieth. A sector's colour says which zone it is,
/// not how steep: the angle is printed in it, and two zones a degree or two
/// apart still have to be told apart at a glance.
const ZONE_COLOURS: [[u8; 3]; 20] = [
    [124, 152, 182], // slate blue
    [203, 168, 126], // sand
    [133, 168, 140], // sage
    [190, 140, 138], // dusty rose
    [152, 140, 182], // lavender grey
    [168, 152, 130], // taupe
    [196, 160, 180], // soft mauve
    [124, 172, 172], // grey teal
    [180, 180, 130], // olive cream
    [152, 154, 160], // stone
    [168, 192, 216], // pale blue
    [222, 196, 164], // pale sand
    [176, 202, 182], // pale sage
    [216, 176, 174], // blush
    [192, 184, 212], // pale lavender
    [200, 188, 170], // oat
    [220, 194, 208], // pale mauve
    [166, 204, 204], // pale teal
    [208, 208, 168], // pale olive
    [196, 198, 204], // pale stone
];

/// Draw the circle for `sectors` (in bearing order), centred in the width available.
pub(crate) fn draw_rosette(ui: &mut egui::Ui, sectors: &[SlopeSector]) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), HEIGHT), egui::Sense::hover());
    let centre = rect.center();
    let radius = (rect.width().min(rect.height()) / 2.0 - LABEL_MARGIN).max(40.0);
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let line = visuals.widgets.noninteractive.fg_stroke.color;
    let weak = visuals.weak_text_color();
    let on_circle = |bearing: f64, scale: f32| {
        let angle = bearing.to_radians();
        centre + egui::vec2(radius * scale * angle.sin() as f32, -radius * scale * angle.cos() as f32)
    };

    // The sectors, each a fan of triangles from the centre to its rim.
    for (zone, sector) in sectors.iter().enumerate() {
        let colour = zone_colour(zone);
        let steps = (((sector.to - sector.from) / RIM_STEP_DEGREES).ceil() as usize).max(1);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(centre, colour);
        for step in 0..=steps {
            let bearing = sector.from + (sector.to - sector.from) * step as f64 / steps as f64;
            mesh.colored_vertex(on_circle(bearing, 1.0), colour);
        }
        for step in 0..steps as u32 {
            mesh.add_triangle(0, step + 1, step + 2);
        }
        painter.add(egui::Shape::mesh(mesh));
    }

    // The outline, and where each sector starts.
    let outline: Vec<egui::Pos2> = (0..=120).map(|step| on_circle(f64::from(step) * 3.0, 1.0)).collect();
    painter.add(egui::Shape::line(outline, egui::Stroke::new(1.5, line)));
    if sectors.len() > 1 {
        for sector in sectors {
            painter.line_segment([centre, on_circle(sector.from, 1.0)], egui::Stroke::new(1.0, line));
        }
    }

    // A small pit inside, so it reads as a pit's walls: a toe line for each
    // bench, stretched north-south and a little uneven, and above it the crest,
    // the toe pushed out by the same distance everywhere.
    let ink = egui::Color32::from_rgba_unmultiplied(40, 46, 58, 215);
    let toe_point = |scale: f32, angle: f64| {
        let wobble = 1.0 + 0.12 * (2.0 * angle + 0.6).sin() + 0.07 * (3.0 * angle + 1.9).sin() + 0.03 * (5.0 * angle).sin();
        let reach = radius * scale * PIT_FIT * wobble as f32;
        centre + egui::vec2(reach * PIT_WIDTH * angle.sin() as f32, -reach * angle.cos() as f32)
    };
    let step_angle = |step: u32| f64::from(step) / f64::from(PIT_STEPS) * std::f64::consts::TAU;
    for scale in PIT_TOES {
        let toe: Vec<egui::Pos2> = (0..=PIT_STEPS).map(|step| toe_point(scale, step_angle(step))).collect();
        let crest: Vec<egui::Pos2> = (0..=PIT_STEPS)
            .map(|step| {
                // Outward normal from the toe's own direction of travel.
                let angle = step_angle(step);
                let tangent = toe_point(scale, angle + 1e-3) - toe_point(scale, angle - 1e-3);
                let outward = egui::vec2(tangent.y, -tangent.x).normalized();
                toe[step as usize] + outward * (radius * PIT_FACE_RUN)
            })
            .collect();
        painter.add(egui::Shape::line(crest, egui::Stroke::new(2.6, ink)));
        painter.add(egui::Shape::line(toe, egui::Stroke::new(1.2, ink)));
    }

    for (zone, sector) in sectors.iter().enumerate() {
        painter.text(
            on_circle(sector.from, 1.0) + (on_circle(sector.from, 1.0) - centre).normalized() * 14.0,
            egui::Align2::CENTER_CENTER,
            format!("{}°", trim_number(sector.from.rem_euclid(360.0))),
            egui::TextStyle::Small.resolve(ui.style()),
            weak,
        );
        let middle = (sector.from + sector.to) / 2.0;
        painter.text(
            on_circle(middle, 0.78),
            egui::Align2::CENTER_CENTER,
            format!("{}°", trim_number(sector.angle)),
            egui::FontId::proportional(15.0),
            text_on(zone_colour(zone)),
        );
    }

    // The compass, so the bearings read as bearings.
    for (bearing, letter) in [
        (0.0, tr!("opt-compass-north")),
        (90.0, tr!("opt-compass-east")),
        (180.0, tr!("opt-compass-south")),
        (270.0, tr!("opt-compass-west")),
    ] {
        let direction = (on_circle(bearing, 1.0) - centre).normalized();
        painter.text(
            on_circle(bearing, 1.0) + direction * 30.0,
            egui::Align2::CENTER_CENTER,
            letter,
            egui::FontId::proportional(13.0),
            weak,
        );
    }
    response.on_hover_text(tr!("opt-rosette-diagram-hover"));
}

fn zone_colour(zone: usize) -> egui::Color32 {
    let [red, green, blue] = ZONE_COLOURS[zone % ZONE_COLOURS.len()];
    egui::Color32::from_rgba_unmultiplied(red, green, blue, 210)
}

/// Black or white, whichever reads better over `fill`.
fn text_on(fill: egui::Color32) -> egui::Color32 {
    let luminance = 0.299 * f32::from(fill.r()) + 0.587 * f32::from(fill.g()) + 0.114 * f32::from(fill.b());
    if luminance > 150.0 { egui::Color32::from_gray(20) } else { egui::Color32::WHITE }
}

fn trim_number(value: f64) -> String {
    if (value - value.round()).abs() < 1e-6 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}
