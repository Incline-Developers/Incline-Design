//! World-to-screen projection updates for UI/tool overlays.

use super::*;
use crate::{
    app::commands::drawing::rotate_collar::{ring_axis, ring_basis},
    rendering::section_grid,
    ui::state::{MoveGizmoScreen, ROTATE_GIZMO_AZIMUTH_RING, ROTATE_GIZMO_DIP_RING, RotateGizmoScreen, SectionGridAxis, SectionGridLine, SectionGridLineKind},
};

pub(crate) type ScreenSegmentPx = ((f32, f32), (f32, f32));

/// How long, in points on screen, a machine has to be before Animate draws
/// its model rather than its icon.
const MACHINE_MODEL_POINTS: f32 = 32.0;

fn point_to_screen_segment_distance_sq(point: (f32, f32), segment: ScreenSegmentPx) -> f32 {
    let (a, b) = segment;
    let ab = (b.0 - a.0, b.1 - a.1);
    let ap = (point.0 - a.0, point.1 - a.1);
    let length_sq = ab.0 * ab.0 + ab.1 * ab.1;
    let t = if length_sq > f32::EPSILON {
        ((ap.0 * ab.0 + ap.1 * ab.1) / length_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let delta = (point.0 - (a.0 + t * ab.0), point.1 - (a.1 + t * ab.1));
    delta.0 * delta.0 + delta.1 * delta.1
}

fn nearest_screen_segment(cursor: (f32, f32), segments: impl IntoIterator<Item = (usize, ScreenSegmentPx)>) -> Option<(usize, ScreenSegmentPx)> {
    segments
        .into_iter()
        .filter_map(|(index, segment)| {
            let distance_sq = point_to_screen_segment_distance_sq(cursor, segment);
            distance_sq.is_finite().then_some((index, segment, distance_sq))
        })
        .min_by(|(_, _, a), (_, _, b)| a.total_cmp(b))
        .map(|(index, segment, _)| (index, segment))
}

/// How far off the run between the anchor and the cursor a collar may stand
/// and still be tied in, in physical pixels.
///
/// Measured on screen rather than in the ground: a row of holes is never
/// exactly straight, and what the user is aiming along is the row as they can
/// see it. The collar marker is 11px across, so this is a corridor a little
/// wider than the marks themselves.
pub(crate) const TIE_CORRIDOR_PIXELS: f32 = 12.0;

/// Screen length of a Move gizmo axis that lies square to the camera, in
/// logical points. Every other part of the gizmo is sized from this.
const GIZMO_LENGTH_POINTS: f64 = 80.0;
/// Ring radius as a fraction of the axis length, matching Blender's
/// view-aligned translate handle.
const GIZMO_RING_RATIO: f32 = 0.2;
/// Distance along the diagonal between two axes at which their plane handle
/// sits, and the handle's half-extent, both as fractions of the axis length.
const GIZMO_PLANE_OFFSET: f64 = 0.8;
const GIZMO_PLANE_HALF_EXTENT: f64 = 0.1;
/// Foreshortening below which an axis handle is hidden, and above which it is
/// fully opaque. An axis pointing at the camera projects to nothing useful, so
/// it fades out instead of collapsing into a stub pointing in a direction that
/// flips with sub-pixel camera movement.
const GIZMO_AXIS_FADE_MIN: f32 = 0.2;
const GIZMO_AXIS_FADE_FULL: f32 = 0.44;
/// The same for plane handles, which stay useful until they are nearly
/// edge-on: measured as how squarely the plane faces the camera.
const GIZMO_PLANE_FADE_MIN: f32 = 0.175;
const GIZMO_PLANE_FADE_FULL: f32 = 0.25;

/// Points sampled around each Rotate Collar ring. Enough that a ring reads as
/// a circle at the size it is drawn, and cheap enough to reproject per frame.
const ROTATE_RING_SEGMENTS: usize = 72;
/// Ring radius as a fraction of the Move gizmo's arm length, so the two tools'
/// gizmos come up at about the same size over the same selection.
const ROTATE_RING_RATIO: f64 = 0.85;
/// A ring turning edge-on cannot be followed round, so it fades out over this
/// band of how squarely it faces the camera and stops being clickable at the
/// bottom of it - the same rule the Move gizmo's plane handles follow.
const ROTATE_RING_FADE_MIN: f32 = 0.1;
const ROTATE_RING_FADE_FULL: f32 = 0.28;

/// How far across the view the initiation card's scale probe reaches, in world
/// units. Long enough to measure cleanly, short enough that a perspective
/// view's scale is still the collar's own.
const CARD_SCALE_PROBE_WORLD: f64 = 1.0;

/// Least room left along a contour between one label and the next, in
/// points. The spacing on the ground is the power of two that keeps them at
/// least this far apart on screen.
const CONTOUR_LABEL_SPACING_POINTS: f64 = 280.0;

/// Where along a contour its labels stand, every `spacing` world units from
/// its start: each spot, and a point a little further on that gives the
/// line's direction there. A line too short for one spot gets one at its
/// middle.
///
/// Measured on the ground rather than on screen, so a label stays where it
/// is as the camera moves. Spacings are powers of two, so zooming out drops
/// every other label and keeps the rest in place.
fn contour_label_spots(points: &[DVec3], closed: bool, spacing: f64) -> Vec<(DVec3, DVec3)> {
    let closing = closed.then(|| points.first().copied()).flatten();
    let run: Vec<DVec3> = points.iter().copied().chain(closing).collect();
    let length: f64 = run.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
    if length <= 0.0 {
        return Vec::new();
    }
    let mut distances: Vec<f64> = (1..).map(|step| step as f64 * spacing).take_while(|distance| *distance < length - spacing * 0.25).collect();
    if distances.is_empty() && length > spacing * 0.4 {
        distances.push(length * 0.5);
    }
    let mut spots = Vec::with_capacity(distances.len());
    let mut walked = 0.0;
    let mut wanted = distances.into_iter().peekable();
    for pair in run.windows(2) {
        let step = pair[0].distance(pair[1]);
        while let Some(distance) = wanted.peek().copied().filter(|distance| step > 0.0 && walked + step >= *distance) {
            let direction = (pair[1] - pair[0]) / step;
            let at = pair[0] + direction * (distance - walked);
            spots.push((at, at + direction * (spacing * 0.05).max(0.01)));
            wanted.next();
        }
        walked += step;
    }
    spots
}

/// Cap on grid lines per axis; `grid_values` coarsens spacing to stay under
/// it rather than dropping the axis.
const MAX_GRID_LINES: usize = 64;

/// How far the ruled area may stretch under tilt, in square-on screens.
const GRID_REACH_MAX: f64 = 4.0;

fn fade_ramp(value: f32, min: f32, max: f32) -> f32 {
    if value <= min {
        0.0
    } else if value >= max {
        1.0
    } else {
        (value - min) / (max - min)
    }
}

/// The holes a tie-in run from `anchor` to the cursor would pass through, in
/// the order the round travels, starting with the anchor itself.
///
/// Everything standing in the corridor between the two is taken, not just what
/// is under the pointer: aiming down a row is how a round is tied in, and a
/// row is tied one leg per click only when the legs between are found for the
/// user. Order is by distance along the run, so a chain drawn back on itself
/// still fires the way it was drawn.
pub(crate) fn tie_chain_between(holes: &[crate::model::drill_hole::DrillHole], anchor: usize, cursor_px: (f32, f32), project: impl Fn(DVec3) -> Option<(f32, f32)>) -> Vec<usize> {
    let Some(anchor_hole) = holes.get(anchor) else {
        return Vec::new();
    };
    let Some(start) = project(anchor_hole.collar_position()) else {
        return Vec::new();
    };
    let run = (cursor_px.0 - start.0, cursor_px.1 - start.1);
    let length_sq = run.0 * run.0 + run.1 * run.1;
    if length_sq <= f32::EPSILON {
        return Vec::new();
    }
    let mut along: Vec<(f32, usize)> = holes
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != anchor)
        .filter_map(|(index, hole)| {
            let point = project(hole.collar_position())?;
            let offset = (point.0 - start.0, point.1 - start.1);
            let t = (offset.0 * run.0 + offset.1 * run.1) / length_sq;
            // Behind the anchor, or beyond where the pointer has reached: not
            // on the run being drawn.
            if !(0.0..=1.0).contains(&t) {
                return None;
            }
            let across = (offset.0 - t * run.0, offset.1 - t * run.1);
            (across.0.hypot(across.1) <= TIE_CORRIDOR_PIXELS).then_some((t, index))
        })
        .collect();
    if along.is_empty() {
        return Vec::new();
    }
    along.sort_by(|(a, _), (b, _)| a.total_cmp(b));
    std::iter::once(anchor).chain(along.into_iter().map(|(_, index)| index)).collect()
}

pub(crate) fn projected_relimit_candidate_nearest_cursor(
    cursor: (f32, f32),
    candidates: &[crate::ui::state::RelimitCandidate],
    source_start: DVec3,
    source_end: DVec3,
    view_proj: &DMat4,
    screen: Size,
) -> Option<(usize, ScreenSegmentPx)> {
    use crate::ui::state::TrimEnd;

    let projected = candidates.iter().enumerate().filter_map(|(index, candidate)| {
        let moving = match candidate.end {
            TrimEnd::Start => source_start,
            TrimEnd::End => source_end,
        };
        // Extension targets can lie beyond the committed scene's depth range.
        // This foreground guide must remain selectable there, like offset previews.
        let from = crate::rendering::pick::world_to_screen_unclipped_depth(view_proj, moving, screen)?;
        let to = crate::rendering::pick::world_to_screen_unclipped_depth(view_proj, candidate.target, screen)?;
        Some((index, ((from.x as f32, from.y as f32), (to.x as f32, to.y as f32))))
    });
    nearest_screen_segment(cursor, projected)
}

/// The camera-plane reference a gizmo sizes itself against: two world
/// directions lying in the view plane, the screen vectors one world unit along
/// each produces, and the pixels per world unit that follows from them.
///
/// Both gizmos measure against this, which is what makes "80 logical points"
/// mean the same size on screen whichever one is up.
struct GizmoScreenScale {
    right: DVec3,
    up: DVec3,
    right_px: (f64, f64),
    up_px: (f64, f64),
    px_per_world: f64,
}

fn gizmo_screen_scale(center: DVec3, center_px: (f32, f32), forward: DVec3, camera_up_hint: DVec3, project: &impl Fn(DVec3) -> Option<(f32, f32)>) -> Option<GizmoScreenScale> {
    let up_hint = if forward.cross(camera_up_hint).length_squared() > 1.0e-9 {
        camera_up_hint
    } else {
        DVec3::X
    };
    let right = forward.cross(up_hint).normalize();
    let up = right.cross(forward).normalize();
    let screen_vector = |direction: DVec3| -> Option<(f64, f64)> {
        let tip = project(center + direction)?;
        Some((f64::from(tip.0 - center_px.0), f64::from(tip.1 - center_px.1)))
    };
    let right_px = screen_vector(right).unwrap_or((1.0, 0.0));
    let up_px = screen_vector(up).unwrap_or((0.0, -1.0));
    let px_per_world = right_px.0.hypot(right_px.1).max(up_px.0.hypot(up_px.1));
    (px_per_world.partial_cmp(&1.0e-9) == Some(std::cmp::Ordering::Greater)).then_some(GizmoScreenScale {
        right,
        up,
        right_px,
        up_px,
        px_per_world,
    })
}

/// Project the Rotate Collar gizmo around `center`: an azimuth ring lying flat
/// and a dip ring standing in the vertical plane the holes point along, so the
/// dip ring swings round as the bearing is turned and always shows the plane
/// the toe actually moves in.
///
/// Rings are sampled in world space and projected point by point, so
/// perspective shapes them into the ellipses they really are.
fn build_rotate_gizmo(
    center: DVec3,
    azimuth_degrees: f64,
    forward: DVec3,
    camera_up_hint: DVec3,
    length_px: f32,
    project: impl Fn(DVec3) -> Option<(f32, f32)>,
) -> RotateGizmoScreen {
    let Some(center_px) = project(center) else {
        return RotateGizmoScreen::default();
    };
    let Some(scale) = gizmo_screen_scale(center, center_px, forward, camera_up_hint, &project) else {
        return RotateGizmoScreen::default();
    };
    let radius = f64::from(length_px) * ROTATE_RING_RATIO / scale.px_per_world;

    let mut gizmo = RotateGizmoScreen {
        center_px: Some(center_px),
        scale_factor: (f64::from(length_px) / GIZMO_LENGTH_POINTS) as f32,
        ..RotateGizmoScreen::default()
    };
    for ring in [ROTATE_GIZMO_AZIMUTH_RING, ROTATE_GIZMO_DIP_RING] {
        let index = usize::from(ring);
        let normal = ring_axis(ring, azimuth_degrees);
        let towards_view = normal.dot(forward);
        // A ring is fully readable face-on and useless edge-on, which is
        // exactly how squarely its axis points along the view.
        gizmo.ring_fade[index] = fade_ramp(towards_view.abs() as f32, ROTATE_RING_FADE_MIN, ROTATE_RING_FADE_FULL);
        if gizmo.ring_fade[index] <= 0.0 {
            continue;
        }
        let [across, along] = ring_basis(ring, azimuth_degrees);
        gizmo.ring_px[index] = (0..ROTATE_RING_SEGMENTS)
            .filter_map(|step| {
                let theta = std::f64::consts::TAU * step as f64 / ROTATE_RING_SEGMENTS as f64;
                project(center + (across * theta.cos() + along * theta.sin()) * radius)
            })
            .collect();
    }
    gizmo
}

/// Project the Move gizmo around `center`, Blender-style: fixed on-screen
/// size, each axis foreshortened by its own angle to the camera, and
/// handles faded out as they turn towards the view direction.
fn build_move_gizmo(center: DVec3, forward: DVec3, camera_up_hint: DVec3, length_px: f32, project: impl Fn(DVec3) -> Option<(f32, f32)>) -> MoveGizmoScreen {
    let Some(center_px) = project(center) else {
        return MoveGizmoScreen::default();
    };

    // Camera-plane basis: the reference for "unforeshortened" screen size,
    // and the drag basis for the view-aligned ring.
    let Some(scale) = gizmo_screen_scale(center, center_px, forward, camera_up_hint, &project) else {
        return MoveGizmoScreen::default();
    };
    let world_length = f64::from(length_px) / scale.px_per_world;

    let mut gizmo = MoveGizmoScreen {
        center_px: Some(center_px),
        scale_factor: (f64::from(length_px) / GIZMO_LENGTH_POINTS) as f32,
        ring_radius_px: length_px * GIZMO_RING_RATIO,
        view_axes: Some([scale.right, scale.up]),
        view_basis_px: [scale.right_px, scale.up_px],
        ..MoveGizmoScreen::default()
    };

    let axes = [DVec3::X, DVec3::Y, DVec3::Z];
    // How much of its full screen length each axis keeps: 1 when square to
    // the camera, 0 when aimed at it.
    let mut projected_ratio = [0.0f32; 3];
    for (index, axis) in axes.into_iter().enumerate() {
        let tip = project(center + axis * world_length);
        gizmo.axis_tip_px[index] = tip;
        let span = tip.map_or(0.0, |tip| f64::from(tip.0 - center_px.0).hypot(f64::from(tip.1 - center_px.1)));
        projected_ratio[index] = (span / f64::from(length_px)).clamp(0.0, 1.0) as f32;
        gizmo.axis_fade[index] = fade_ramp(projected_ratio[index], GIZMO_AXIS_FADE_MIN, GIZMO_AXIS_FADE_FULL);
        gizmo.axis_px_per_world[index] = (span / world_length).max(1.0e-6);
    }

    // Plane handles are small diamonds on the diagonal between their two
    // axes, built in world space so perspective shapes them correctly.
    let offset = GIZMO_PLANE_OFFSET * world_length;
    let half_extent = GIZMO_PLANE_HALF_EXTENT * world_length;
    for (index, [first, second]) in [[0usize, 1], [0, 2], [1, 2]].into_iter().enumerate() {
        let normal = 3 - first - second;
        // A plane stops being usable as it turns edge-on, which is exactly
        // when its normal turns square to the view.
        let facing = (1.0 - projected_ratio[normal] * projected_ratio[normal]).max(0.0).sqrt();
        gizmo.plane_fade[index] = fade_ramp(facing, GIZMO_PLANE_FADE_MIN, GIZMO_PLANE_FADE_FULL);
        if gizmo.plane_fade[index] <= 0.0 {
            continue;
        }
        let diagonal = (axes[first] + axes[second]).normalize();
        let across = (axes[second] - axes[first]).normalize();
        let handle_center = center + diagonal * offset;
        let corners = [
            project(handle_center - diagonal * half_extent),
            project(handle_center + across * half_extent),
            project(handle_center + diagonal * half_extent),
            project(handle_center - across * half_extent),
        ];
        gizmo.plane_quad_px[index] = match corners {
            [Some(a), Some(b), Some(c), Some(d)] => Some([a, b, c, d]),
            _ => None,
        };
    }

    gizmo
}

/// Where a collar gesture's gizmo stands, and the bearing its dip ring should
/// lie in: the mean of the selected collars, and the orientation of the anchor
/// hole among them.
///
/// The anchor is the lowest-numbered hole of the lowest-numbered dataset - the
/// same order `App::collar_targets` sorts into - so which hole the panel and
/// the dip ring speak for does not shift about with hash order.
fn selected_collar_anchor(editor: &EditorState, drill_holes: &[OpenDrillHoleDataset]) -> Option<(DVec3, f64)> {
    let hole_at = |target: &crate::model::drill_hole::DrillHoleRef| {
        drill_holes
            .iter()
            .find(|dataset| dataset.id == target.dataset && dataset.state.loaded)
            .and_then(|dataset| dataset.dataset.holes.get(target.hole))
    };
    let mut selected: Vec<_> = editor.selected_drill_holes.iter().filter(|target| hole_at(target).is_some()).collect();
    if selected.is_empty() {
        return None;
    }
    selected.sort_by_key(|target| (target.dataset.0, target.hole));
    let sum: DVec3 = selected.iter().filter_map(|target| hole_at(target)).map(|hole| hole.collar).sum();
    // A hole with no length below its collar has no bearing of its own; north
    // is the same stand-in the survey resolver falls back on.
    let azimuth = selected
        .first()
        .and_then(|target| hole_at(target))
        .and_then(|hole| hole.orientation())
        .map_or(0.0, |orientation| orientation.azimuth);
    Some((sum / selected.len() as f64, azimuth))
}

impl<'a> Graphics<'a> {
    /// Project the Move gizmo around `center` using the live camera.
    fn project_move_gizmo(&self, center: DVec3) -> MoveGizmoScreen {
        let view_proj = self.view_proj();
        build_move_gizmo(
            center,
            self.camera.forward(),
            self.camera.up(),
            (GIZMO_LENGTH_POINTS * self.scale_factor()) as f32,
            // The gizmo is a foreground overlay sized to constant screen space,
            // so its arms reach furthest in world units exactly when the camera
            // is zoomed in and the scene-fitted depth slab is thinnest - project
            // without that slab's rejection or the arms vanish.
            |world| self.world_to_window_px_unclipped_depth(&view_proj, world),
        )
    }

    /// Project the Rotate Collar gizmo around `center`, with its dip ring
    /// standing in the vertical plane `azimuth_degrees` names.
    fn project_rotate_gizmo(&self, center: DVec3, azimuth_degrees: f64) -> RotateGizmoScreen {
        let view_proj = self.view_proj();
        build_rotate_gizmo(
            center,
            azimuth_degrees,
            self.camera.forward(),
            self.camera.up(),
            (GIZMO_LENGTH_POINTS * self.scale_factor()) as f32,
            // Sized to constant screen space like the Move gizmo, so it wants
            // the same freedom from the scene-fitted depth slab.
            |world| self.world_to_window_px_unclipped_depth(&view_proj, world),
        )
    }

    /// The section grid's spacing: which world axis the uprights follow and
    /// the two spacings, sized from the square-on spans so an orbit does not
    /// re-rule the grid; exaggeration keeps a screen cell from being square.
    pub(super) fn section_grid_spacing(&self, level_spacing: Option<f64>) -> Option<(SectionGridAxis, f64, f64)> {
        let slice = self.slice_view.as_ref()?;
        let screen = self.screen_size();
        let half_length = slice_visible_half_length(self.projection.zoom, screen);
        let half_height = self.projection.zoom;
        let world_per_pixel = 2.0 * self.projection.zoom / f64::from(screen.1.max(1.0));
        let points_per_pixel = 1.0 / self.scale_factor();
        let strike_pt = 2.0 * half_length / world_per_pixel * points_per_pixel;
        let height_pt = 2.0 * half_height / world_per_pixel * points_per_pixel;
        let axis = section_grid::upright_axis(slice.direction);
        let axis_spacing = section_grid::grid_spacing(section_grid::upright_span(slice.direction, half_length), strike_pt);
        // half_height is display units (exaggerated); the span is true metres.
        let true_span = 2.0 * half_height / self.vertical_exaggeration;
        // A chosen spacing is thinned once here, for the lines and their labels
        // alike, so the widest reach still fits the line budget.
        let elevation_spacing = level_spacing.map_or_else(
            || section_grid::grid_spacing(true_span, height_pt),
            |chosen| section_grid::coarsen_to_fit(chosen, true_span * GRID_REACH_MAX, MAX_GRID_LINES),
        );
        Some((axis, axis_spacing, elevation_spacing))
    }

    /// Section grid in window pixels: elevation levels plus world easting/northing lines where the cut crosses them.
    /// The lines are drawn on the plane by the section grid shader; these
    /// place the labels.
    fn build_section_grid(&self, level_spacing: Option<f64>) -> (Vec<SectionGridLine>, Option<f64>) {
        let Some(slice) = self.slice_view.as_ref() else { return (Vec::new(), None) };
        let Some((axis, axis_spacing, elevation_spacing)) = self.section_grid_spacing(level_spacing) else {
            return (Vec::new(), None);
        };
        let screen = self.screen_size();

        // Sized from zoom alone, not the viewport corners, so an orbit does not re-rule the grid.
        let half_length = slice_visible_half_length(self.projection.zoom, screen);
        let half_height = self.projection.zoom;

        // Ruled about the anchor, the eye's foot on the plane (`set_eye`).
        let (forward, right, up) = slice.camera_basis();
        // The shader fades the lines out towards edge-on; the labels go too.
        if forward.dot(slice.normal()).abs() < 0.1 {
            return (Vec::new(), None);
        }
        let strike = slice.direction.extend(0.0);
        let foot = slice.center;
        // Spacing is sized from the square-on spans, so an orbit does not
        // re-rule the grid; the reach grows with the tilt, which shows more.
        let stretch = |cosine: f64| (1.0 / cosine.abs()).min(GRID_REACH_MAX);
        let strike_reach = half_length * stretch(right.dot(strike));
        // Yawed and pitched together, screen-up also runs along the strike.
        let height_reach = ((half_height + strike_reach * up.dot(strike).abs()) / up.z.abs()).min(half_height * GRID_REACH_MAX);
        // The reach is display units (exaggerated); the ruled range is true
        // metres via unexaggeration - eastings/northings need no such step.
        let rule_bottom = self.unexaggerate_point(foot - DVec3::Z * height_reach).z;
        let rule_top = self.unexaggerate_point(foot + DVec3::Z * height_reach).z;

        let view_proj = self.view_proj();
        let center_xy = foot.truncate();
        // Unclipped depth: a line must not vanish for reaching outside the section's thin depth slab.
        let project =
            |along_strike: f64, elevation: f64| self.world_to_window_px_unclipped_depth(&view_proj, section_grid::plane_point(center_xy, slice.direction, along_strike, elevation));
        let mut lines = Vec::new();
        let mut push = |from: (f64, f64), to: (f64, f64), value: f64, kind: SectionGridLineKind| {
            if let (Some(from_px), Some(to_px)) = (project(from.0, from.1), project(to.0, to.1)) {
                lines.push(SectionGridLine { from_px, to_px, value, kind });
            }
        };

        // Level = constant true elevation; Upright = fixed at a world easting/northing regardless of orbit.
        for elevation in section_grid::grid_values((rule_bottom, rule_top), elevation_spacing, MAX_GRID_LINES) {
            push((-strike_reach, elevation), (strike_reach, elevation), elevation, SectionGridLineKind::Level);
        }
        for crossing in section_grid::upright_crossings(center_xy, slice.direction, strike_reach, axis_spacing, MAX_GRID_LINES) {
            push(
                (crossing.along_strike, rule_bottom),
                (crossing.along_strike, rule_top),
                crossing.value,
                SectionGridLineKind::Upright(axis),
            );
        }
        (lines, Some(elevation_spacing))
    }

    /// Which of Animate's machines are close enough to draw as their model -
    /// one spanning at least [`MACHINE_MODEL_POINTS`] on screen - where the
    /// icons of the rest go, and where the models are labelled, in window
    /// pixels.
    pub(super) fn split_machines(
        &self,
        placements: &[crate::rendering::machines::MachinePlacement],
        scale_factor: f32,
    ) -> (
        Vec<crate::rendering::machines::MachinePlacement>,
        Vec<crate::rendering::machines::MachineIcon>,
        Vec<crate::rendering::machines::MachineLabel>,
    ) {
        let view_proj = self.view_proj();
        let mut near = Vec::new();
        let mut icons = Vec::new();
        let mut labels = Vec::new();
        for placement in placements {
            let length = placement.model.length();
            let span = |axis: glam::DVec3| {
                self.world_to_window_px(&view_proj, placement.position)
                    .zip(self.world_to_window_px(&view_proj, placement.position + axis * length))
                    .map_or(0.0, |(a, b)| (a.0 - b.0).hypot(a.1 - b.1))
            };
            if span(glam::DVec3::X).max(span(glam::DVec3::Y)) >= MACHINE_MODEL_POINTS * scale_factor {
                near.push(*placement);
                if let Some(at) = self.world_to_window_px(&view_proj, placement.position) {
                    labels.push(crate::rendering::machines::MachineLabel { agent: placement.agent, at });
                }
            } else if let Some(at) = self.world_to_window_px(&view_proj, placement.anchor) {
                icons.push(crate::rendering::machines::MachineIcon {
                    model: placement.model,
                    agent: placement.agent,
                    at,
                    working: placement.working,
                });
            }
        }
        (near, icons, labels)
    }

    /// Animate's flows at the shown instant: every delivery under way adds
    /// its tonnes per hour to each road segment of its loaded route, so a road
    /// shared by several blocks carries their sum, drawn once.
    pub(super) fn project_haul_flows(&self, editor: &mut EditorState) {
        let mut flows = std::mem::take(&mut editor.animation_flows);
        flows.clear();
        if editor.is_schedule_animation()
            && editor.schedule_animation_enabled
            && let Some(schedule) = editor.schedule_result.as_ref()
        {
            let mut rates: std::collections::HashMap<crate::ui::state::HaulFlowKey, f64> = std::collections::HashMap::new();
            for delivery in schedule.deliveries_at(editor.schedule_animation_shown_h) {
                let duration = delivery.end_h - delivery.start_h;
                if duration > 0.0 && delivery.tonnes > 0.0 {
                    *rates.entry((delivery.source, delivery.destination, delivery.truck)).or_default() += delivery.tonnes / duration;
                }
            }
            let view_proj = self.view_proj();
            // Centimetre keys: the same road vertex reached by two routes is
            // the same segment.
            let key = |p: glam::DVec3| [(p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64, (p.z * 100.0).round() as i64];
            let mut segments: std::collections::HashMap<([i64; 3], [i64; 3]), usize> = std::collections::HashMap::new();
            let mut ordered: Vec<_> = rates.into_iter().collect();
            // Busiest first, so a shared road takes its stripes' phase from
            // the route most of its tonnes are on.
            ordered.sort_by(|a, b| b.1.total_cmp(&a.1));
            for (route, tph) in ordered {
                let Some(path) = editor.animation_routes.get(&route) else { continue };
                let mut along = 0.0f32;
                for pair in path.windows(2) {
                    let (Some(a), Some(b)) = (self.world_to_window_px(&view_proj, pair[0]), self.world_to_window_px(&view_proj, pair[1])) else {
                        continue;
                    };
                    let segment = (key(pair[0]), key(pair[1]));
                    match segments.get(&segment) {
                        Some(&index) => flows[index].tph += tph,
                        None => {
                            segments.insert(segment, flows.len());
                            flows.push(crate::ui::state::HaulFlowSegment {
                                from: pair[0],
                                to: pair[1],
                                a,
                                b,
                                offset: along,
                                tph,
                            });
                        }
                    }
                    along += (b.0 - a.0).hypot(b.1 - a.1);
                }
            }
        }
        editor.animation_flows = flows;
    }

    /// Pit Design's handles and flags where they stand on screen: each
    /// ramp's arrow and width handle at its foot, and each flag of the
    /// design shown.
    fn project_pit_design(&self, editor: &mut EditorState, document: &Document) {
        use crate::model::pit_design::{StringKind, object::base_points, ring};
        let Some(draft) = editor.pit_design.as_mut() else {
            return;
        };
        draft.gizmos.clear();
        draft.wall_gizmo = None;
        draft.switch_spots.clear();
        let view_proj = self.view_proj();
        let project = |world: DVec3| self.world_to_window_px(&view_proj, world);
        let design = draft.preview.as_ref().and_then(|preview| preview.design.as_ref().ok());
        draft.flag_px = design.map_or_else(Vec::new, |design| design.flags.iter().map(|flag| project(flag.at)).collect());
        // Where a press on the string would add a ramp, and while one is
        // added, the way it climbs from its foot.
        draft.string_hover_px = draft.string_hover.and_then(project);
        draft.adding_arrow = draft.adding_ramp.filter(|adding| adding.dragged).and_then(|adding| {
            let centre = project(adding.foot)?;
            let scale = gizmo_screen_scale(adding.foot, centre, self.camera.forward(), self.camera.up(), &project)?;
            let reach = 1.6 * CLIMB_ARROW_POINTS * self.scale_factor() / scale.px_per_world;
            let way = adding.way.extend(0.0);
            let across = DVec3::new(-adding.way.y, adding.way.x, 0.0);
            let (tip, back) = (adding.foot + way * reach, adding.foot + way * (reach * 0.7));
            Some([centre, project(tip)?, project(back + across * (reach * 0.18))?, project(back - across * (reach * 0.18))?])
        });
        // While a start is moved, a ramp added or a region chosen, the
        // ramps' handles stand down; the walls' stay where they are.
        let placing = draft.placing_ramp() || draft.picking_region;
        let (Some(id), Some(working), Some(preview), Some(design)) = (draft.object_id(), draft.working.as_ref(), draft.preview.as_ref(), design) else {
            return;
        };
        let Some(Object::PitDesign { verts, .. }) = document.get_object(id) else {
            return;
        };
        let ring: Vec<glam::DVec2> = base_points(verts).iter().map(|point| point.truncate()).collect();
        let count = ring.len();
        if count < 3 {
            return;
        }
        let centrelines: Vec<&[DVec3]> = design
            .strings
            .iter()
            .filter(|string| string.kind == StringKind::RampCentreline)
            .map(|string| string.points.as_slice())
            .collect();
        let mut gizmos = Vec::new();
        let mut spots = Vec::new();
        for (index, (ramp, foot)) in working.settings.ramps.iter().zip(&preview.ramp_starts).enumerate().filter(|_| !placing) {
            let at = foot.truncate();
            let edge = (ring::nearest(&ring, at).0.floor() as usize).min(count - 1);
            let Some(tangent) = (ring[(edge + 1) % count] - ring[edge]).try_normalize() else {
                continue;
            };
            let normal = glam::DVec2::new(tangent.y, -tangent.x);
            // The road's own way from its foot, and its side of the string:
            // as its centreline leaves, whichever way the design is built.
            let Some((leaving, ahead, line)) = centrelines
                .iter()
                .filter_map(|line| {
                    let (first, last) = (line.first()?.truncate(), line.last()?.truncate());
                    let (end, next) = if first.distance(at) <= last.distance(at) {
                        (first, line.get(line.len().min(4) - 1)?.truncate())
                    } else {
                        (last, line.get(line.len().saturating_sub(4))?.truncate())
                    };
                    Some((end.distance(at), end, next, *line))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, end, next, line)| (end, next, line))
            else {
                continue;
            };
            // The RLs turns were asked at that some bench made, there or
            // further on.
            let made: Vec<f64> = design
                .report
                .iter()
                .filter_map(|bench| {
                    let road = bench.ramps.iter().find(|road| road.ramp == index && road.switchback && road.turn_at.is_some())?;
                    Some(road.turn_moved_from.unwrap_or(bench.elevation))
                })
                .collect();
            let near = |a: f64, b: f64| (a - b).abs() < 1.0e-6;
            // Where it can switch back: each bench's stretch where it begins,
            // but its first, which it starts on; where it turns, the turn.
            for bench in design.report.iter().filter(|bench| bench.ramps.iter().any(|road| road.ramp == index)).skip(1) {
                let Some(road) = bench.ramps.iter().find(|road| road.ramp == index) else {
                    continue;
                };
                let (rl, place, turn) = match road.turn_at.filter(|_| road.switchback) {
                    Some(turn_at) => (road.turn_moved_from.unwrap_or(bench.elevation), Some(turn_at), true),
                    // Where the road reaches the bench's RL: a graded turn
                    // or a flat can leave no point at it exactly.
                    None => (bench.elevation, at_rl(line, bench.elevation), false),
                };
                let refused = !turn && ramp.turns_at.iter().any(|asked| near(*asked, rl)) && !made.iter().any(|done| near(*done, rl));
                if let Some(at) = place.and_then(project) {
                    spots.push(crate::ui::dialogs::pit_design::SwitchSpot {
                        ramp: index,
                        rl,
                        at,
                        turn,
                        refused,
                    });
                }
            }
            let goes = tangent * (next_sign(ahead - leaving, tangent));
            let across = normal * next_sign(leaving - at, normal);
            if let Some(gizmo) = self.ramp_gizmo(index, *foot, (goes, across), (ramp.gradient, ramp.width), &project) {
                gizmos.push(gizmo);
            }
        }
        // The walls' handles and the ramps' are up together: each stands
        // where it sets what it sets.
        let (clear_view, kept) = (draft.clear_view, draft.wall_place);
        let (wall, place) = self.wall_gizmo(working, design, &ring, (&preview.ramp_starts, clear_view, kept), &project);
        draft.wall_place = place;
        draft.wall_gizmo = wall;
        draft.gizmos = gizmos;
        draft.switch_spots = spots;
    }

    /// The walls' gizmo: the first two benches' section as the settings cut
    /// them, standing upright on the pit's string at their true size, so its
    /// corners lie on the design's rings. It stays where it was put on the string (its
    /// foot is dragged to move it), so turning the view walks round it; it
    /// is put somewhere else only when it has no place yet, or a ramp now
    /// starts on it. Gone from the view while its foot is out of it, when
    /// the panel sets what it sets.
    fn wall_gizmo(
        &self,
        working: &crate::model::pit_design::object::PitObject,
        design: &crate::model::pit_design::Design,
        ring: &[glam::DVec2],
        (ramp_starts, clear_view, kept): (&[DVec3], Option<[f32; 4]>, Option<glam::DVec2>),
        project: &impl Fn(DVec3) -> Option<(f32, f32)>,
    ) -> (Option<crate::ui::dialogs::pit_design::WallGizmo>, Option<glam::DVec2>) {
        use crate::model::pit_design::ring as rings;
        let count = ring.len();
        let (Some(&base), Some(&top)) = (design.levels.first(), design.levels.get(1)) else {
            return (None, kept);
        };
        let margin = (WALL_GIZMO_MARGIN * self.scale_factor()) as f32;
        let shown = |place: glam::DVec2| {
            clear_view.is_none_or(|[left, top, right, bottom]| {
                project(place.extend(base)).is_some_and(|(x, y)| x > left + margin && x < right - margin && y > top + margin && y < bottom - margin)
            })
        };
        let clear_of_ramps = |place: glam::DVec2| ramp_starts.iter().map(|start| start.truncate().distance(place)).fold(f64::INFINITY, f64::min);
        // Where it was put, on the string as it is now, unless a ramp starts
        // on it; or else the place clear of the ramps nearest the middle of
        // what the panel leaves of the view, or failing any in view, the one
        // furthest from the ramps.
        let kept = kept
            .map(|place| rings::point_at(ring, rings::nearest(ring, place).0))
            .filter(|place| clear_of_ramps(*place) >= WALL_GIZMO_RAMP_ON);
        let at = kept.or_else(|| {
            let places: Vec<(glam::DVec2, f64)> = (0..count)
                .flat_map(|edge| {
                    let (a, b) = (ring[edge], ring[(edge + 1) % count]);
                    let steps = (a.distance(b) / WALL_GIZMO_STEP).ceil().max(1.0) as usize;
                    (0..steps).map(move |step| a.lerp(b, (step as f64 + 0.5) / steps as f64))
                })
                .map(|place| (place, clear_of_ramps(place)))
                .collect();
            // Clear of the ramps' gizmos on screen, as they are seen now.
            let feet: Vec<(f32, f32)> = ramp_starts.iter().filter_map(|start| project(*start)).collect();
            let wanted = (WALL_GIZMO_CLEAR_POINTS * self.scale_factor()) as f32;
            let clear_on_screen = |(x, y): (f32, f32)| feet.iter().map(|foot| (foot.0 - x).hypot(foot.1 - y)).fold(f32::INFINITY, f32::min);
            clear_view
                .and_then(|[left, top, right, bottom]| {
                    let (middle_x, middle_y) = ((left + right) * 0.5, (top + bottom) * 0.5);
                    let in_view: Vec<(glam::DVec2, f32, f32)> = places
                        .iter()
                        .filter(|(place, clear)| *clear >= WALL_GIZMO_RAMP_ON && shown(*place))
                        .filter_map(|(place, _)| {
                            let at = project(place.extend(base))?;
                            Some((*place, (at.0 - middle_x).hypot(at.1 - middle_y), clear_on_screen(at)))
                        })
                        .collect();
                    // Nearest the middle of the view among those clear of the
                    // ramps; or failing any, the clearest.
                    in_view
                        .iter()
                        .filter(|(_, _, clear)| *clear >= wanted)
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .or_else(|| in_view.iter().max_by(|a, b| a.2.total_cmp(&b.2)))
                        .map(|(place, ..)| *place)
                })
                .or_else(|| places.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|(place, _)| *place))
        });
        let Some(at) = at else {
            return (None, None);
        };
        if !shown(at) {
            return (None, Some(at));
        }
        (self.bench_section(working, design, ring, at, (base, top), project), Some(at))
    }

    /// [`Self::wall_gizmo`]'s section, standing on the string at `at`.
    fn bench_section(
        &self,
        working: &crate::model::pit_design::object::PitObject,
        design: &crate::model::pit_design::Design,
        ring: &[glam::DVec2],
        at: glam::DVec2,
        (base, top): (f64, f64),
        project: &impl Fn(DVec3) -> Option<(f32, f32)>,
    ) -> Option<crate::ui::dialogs::pit_design::WallGizmo> {
        use crate::{model::pit_design::ring as rings, ui::dialogs::pit_design::WallGizmo};
        let count = ring.len();
        // Out across the bench: the way to its end ring there.
        let crest_ring: Vec<glam::DVec2> = design
            .strings
            .iter()
            .find(|string| string.closed && string.points.first().is_some_and(|point| (point.z - top).abs() < 1.0e-6))?
            .points
            .iter()
            .map(|point| point.truncate())
            .collect();
        // Square to the string there, to the side its bench ends on.
        let edge = (rings::nearest(ring, at).0.floor() as usize).min(count - 1);
        let tangent = (ring[(edge + 1) % count] - ring[edge]).try_normalize()?;
        let normal = glam::DVec2::new(tangent.y, -tangent.x);
        let out = normal * next_sign(rings::point_at(&crest_ring, rings::nearest(&crest_ring, at).0) - at, normal);
        let toe = at.extend(base);
        let forward = self.camera.forward();
        let centre = project(toe)?;
        let scale = gizmo_screen_scale(toe, centre, forward, self.camera.up(), project)?;
        // At its true size, up the way the design is built: its corners lie
        // on the design's own rings, as the settings cut them. Its marks are
        // a size on screen, short of the bench they mark.
        let out3 = out.extend(0.0);
        let up3 = DVec3::Z * (top - base).signum();
        let settings = &working.settings;
        let (height, berm) = (settings.bench_height, settings.berm_width);
        let angle = settings.face_angle.to_radians();
        let run = height / angle.tan();
        let section = |x: f64, y: f64| project(toe + out3 * x + up3 * y);
        let points = |points: f64| points * self.scale_factor() / scale.px_per_world;
        let gap = points(HEIGHT_MARK_POINTS).min(height * 0.6);
        let arc = points(ANGLE_ARC_POINTS).min(0.45 * height / angle.sin());
        // The pixels a metre takes up and out, from a reach a size on screen.
        let probe = points(SECTION_PROBE_POINTS);
        let unit_px = |to: (f32, f32)| {
            let length = (to.0 - centre.0).hypot(to.1 - centre.1);
            (length > 1.0e-3).then_some(((to.0 - centre.0) / length, (to.1 - centre.1) / length, length / probe as f32))
        };
        // Seen down a way, it has none on screen: what moves along it fades.
        let (up_x, up_y, up_px) = unit_px(section(0.0, probe)?).unwrap_or((0.0, -1.0, 1.0e-3));
        let (out_x, out_y, out_px) = unit_px(section(probe, 0.0)?).unwrap_or((1.0, 0.0, 1.0e-3));
        let along = project(toe + tangent.extend(0.0) * probe).and_then(unit_px).map_or((1.0, 0.0), |(x, y, _)| (x, y));
        let seen = |way: DVec3| (1.0 - way.dot(forward).powi(2)).max(0.0).sqrt() as f32;
        Some(WallGizmo {
            height_fade: fade_ramp(seen(up3), GIZMO_AXIS_FADE_MIN, GIZMO_AXIS_FADE_FULL),
            arc_fade: fade_ramp(out3.cross(up3).dot(forward).abs() as f32, ROTATE_RING_FADE_MIN, ROTATE_RING_FADE_FULL),
            out_fade: fade_ramp(seen(out3), GIZMO_AXIS_FADE_MIN, GIZMO_AXIS_FADE_FULL),
            toe: centre,
            crest: section(run, height)?,
            berm_end: section(run + berm, height)?,
            next_top: section(run + berm + run, height * 2.0)?,
            floor_back: section(-points(FLOOR_STUB_POINTS).min(height), 0.0)?,
            height_from: section(-gap, 0.0)?,
            height_tip: section(-gap, height)?,
            angle_arc: (0..=ANGLE_ARC_STEPS)
                .map(|step| {
                    let turn = angle * step as f64 / ANGLE_ARC_STEPS as f64;
                    section(arc * turn.cos(), arc * turn.sin())
                })
                .collect::<Option<_>>()?,
            up: (up_x, up_y),
            out: (out_x, out_y),
            along,
            pixels_per_metre_up: up_px,
            pixels_per_metre_out: out_px,
        })
    }

    /// A ramp's gizmo at its foot: its grade ring standing upright on the
    /// road's line, faded out as it goes edge on, and arrows along the string
    /// the way it leaves its foot and the other way, a fixed size on screen;
    /// and its width arrow across the road at its true width.
    fn ramp_gizmo(
        &self,
        ramp: usize,
        foot: DVec3,
        (goes, across): (glam::DVec2, glam::DVec2),
        (gradient, width): (f64, f64),
        project: &impl Fn(DVec3) -> Option<(f32, f32)>,
    ) -> Option<crate::ui::dialogs::pit_design::RampGizmo> {
        use crate::ui::dialogs::pit_design::{RING_STEPS, RampGizmo};
        let centre = project(foot)?;
        let forward = self.camera.forward();
        let scale = gizmo_screen_scale(foot, centre, forward, self.camera.up(), project)?;
        let length = GIZMO_LENGTH_POINTS * self.scale_factor();
        let radius = length * ROTATE_RING_RATIO / scale.px_per_world;
        let goes3 = goes.extend(0.0);
        let across3 = across.extend(0.0);
        // Seen edge on, from straight above, as a Blender rotate ring, it
        // fades out; its value stays in its card.
        let ring_fade = fade_ramp(goes3.cross(DVec3::Z).dot(forward).abs() as f32, ROTATE_RING_FADE_MIN, ROTATE_RING_FADE_FULL);
        let on_ring = |angle: f64| foot + (goes3 * angle.cos() + DVec3::Z * angle.sin()) * radius;
        let ring: Vec<(f32, f32)> = (0..RING_STEPS)
            .map(|step| project(on_ring(std::f64::consts::TAU * step as f64 / RING_STEPS as f64)))
            .collect::<Option<_>>()?;
        // Drawn exaggerated, as a long section is: a road's grade is a few
        // degrees, too little to see or turn on a ring at its true angle.
        let grade = (gradient / 100.0 * crate::ui::dialogs::pit_design::GRADE_EXAGGERATION).atan();
        let steps = ((grade / GRADE_ARC_STEP).ceil() as usize).max(1);
        let grade_arc: Vec<(f32, f32)> = (0..=steps).filter_map(|step| project(on_ring(grade * step as f64 / steps as f64))).collect();
        // From the foot, the ring's centre, across the road at its true
        // width: its tip on the road's far edge.
        let width_from = centre;
        let width_tip = project(foot + across3 * width)?;
        let run = (width_tip.0 - width_from.0).hypot(width_tip.1 - width_from.1).max(1.0e-3);
        // Pointing at the camera, as a Blender axis arrow, it fades out.
        let width_fade = fade_ramp((1.0 - across3.dot(forward).powi(2)).max(0.0).sqrt() as f32, GIZMO_AXIS_FADE_MIN, GIZMO_AXIS_FADE_FULL);
        // The way it climbs, along the string from the foot, and the way
        // it would the other way round: arrows a click turns it by.
        // Clear of the start's marker, which a press there drags.
        let chevron = |way: f64| -> Option<[(f32, f32); 3]> {
            let reach = CLIMB_ARROW_POINTS * self.scale_factor() / scale.px_per_world;
            let tip = foot + goes3 * (way * reach);
            let back = foot + goes3 * (way * reach * 0.7);
            let wing = across3 * (reach * 0.17);
            Some([project(back + wing)?, project(tip)?, project(back - wing)?])
        };
        Some(RampGizmo {
            ramp,
            ring_fade,
            width_fade,
            centre,
            ring,
            grade_arc,
            grade_handle: project(on_ring(grade)).unwrap_or(centre),
            climbs: chevron(1.0)?,
            climbs_back: chevron(-1.0)?,
            width_from,
            width_tip,
            across: ((width_tip.0 - width_from.0) / run, (width_tip.1 - width_from.1) / run),
            pixels_per_metre: run / (width as f32).max(1.0e-3),
        })
    }

    pub(super) fn update_tool_projections(&self, editor: &mut EditorState, document: &Document, drill_holes: &[OpenDrillHoleDataset]) {
        self.project_haul_flows(editor);
        self.project_pit_design(editor, document);
        editor.haul_pins = if editor.shows_haul_network() {
            document
                .haulage()
                .nodes
                .iter()
                .filter_map(|node| {
                    let role = node.role?;
                    self.world_to_window_px(&self.view_proj(), node.pos).map(|point| (point, role))
                })
                .collect()
        } else {
            Vec::new()
        };
        editor.blast_labels = (if editor.is_dig_strips_step() { &editor.dig_outlines } else { &editor.blasting_outlines })
            .iter()
            // A blast too small to mine has no name to show.
            .filter(|outline| !outline.name.is_empty())
            .filter_map(|outline| {
                let world = DVec3::new(outline.anchor[0], outline.anchor[1], outline.plane);
                let shape = crate::ui::state::BlastShapeRef::new(outline.solid, outline.bench_base, outline.anchor);
                let selected = if editor.is_dig_strips_step() {
                    editor.selected_dig_blocks.contains(&shape)
                } else {
                    editor.selected_blast == Some(shape)
                };
                self.world_to_window_px(&self.view_proj(), world).map(|point| (outline.name.clone(), point, selected))
            })
            .collect();
        // Where the snap landed, for the drawn cursor to mark. The snapped
        // point is the one the tool will use, and it is not the pointer: it
        // can sit anywhere inside the snap threshold of it.
        editor.snap_marker_px = editor
            .cursor_snapped
            .then_some(editor.cursor_world)
            .flatten()
            .and_then(|world| self.world_to_window_px(&self.view_proj(), world));

        if editor.active_workspace == crate::ui::state::Workspace::DrillAndBlast {
            let view_proj = self.view_proj();
            let slab = self.section_slab();
            // The delay card is drawn at a world size, so each one carries the
            // screen scale measured at its own collar: project a probe one
            // world unit across the view and take the pixels it covers. Under
            // perspective that scale falls off with depth, so it cannot be one
            // constant for the whole view.
            let forward = self.camera.forward();
            let probe_offset = forward.cross(self.camera.up()).normalize_or_zero() * CARD_SCALE_PROBE_WORLD;
            editor.initiation_cards = drill_holes
                .iter()
                .filter(|dataset| dataset.state.loaded && !editor.hidden_handles.contains(&dataset.entity_id()))
                .flat_map(|dataset| {
                    dataset.dataset.initiations.iter().filter_map(|initiation| {
                        let hole = dataset.dataset.holes.get(initiation.hole)?;
                        let collar = hole.collar_position();
                        // Drop the card when its anchor is outside the section slab.
                        if slab.is_some_and(|slab| !slab.contains(collar)) {
                            return None;
                        }
                        let screen_px = self.world_to_window_px(&view_proj, collar)?;
                        let probe_px = self.world_to_window_px_unclipped_depth(&view_proj, collar + probe_offset)?;
                        Some(crate::ui::state::InitiationCard {
                            target: crate::model::drill_hole::DrillHoleRef {
                                dataset: dataset.id,
                                hole: initiation.hole,
                            },
                            delay_ms: initiation.delay_ms,
                            screen_px,
                            px_per_world: (probe_px.0 - screen_px.0).hypot(probe_px.1 - screen_px.1) / CARD_SCALE_PROBE_WORLD as f32,
                        })
                    })
                })
                .collect();
            // Lines of equal time across the active pattern, while that review is on.
            let contours_visible = editor.blast_review.contours
                && editor.active_drill_hole.is_some_and(|id| {
                    drill_holes
                        .iter()
                        .any(|dataset| dataset.id == id && dataset.state.loaded && !editor.hidden_handles.contains(&dataset.entity_id()))
                });
            editor.blast_contours_px = match editor.blast_analysis.as_deref().filter(|_| contours_visible) {
                Some(analysis) => {
                    let spacing = self.contour_label_spacing_world(&analysis.contours, &view_proj, probe_offset);
                    analysis
                        .contours
                        .iter()
                        .map(|contour| crate::ui::state::ProjectedContour {
                            time_ms: contour.time_ms,
                            major: contour.major,
                            labels: match spacing.filter(|_| contour.major) {
                                Some(spacing) => contour_label_spots(&contour.points, contour.closed, spacing)
                                    .into_iter()
                                    .filter(|(at, _)| !slab.is_some_and(|slab| !slab.contains(*at)))
                                    .filter_map(|(at, ahead)| Some((self.world_to_window_px(&view_proj, at)?, self.world_to_window_px(&view_proj, ahead)?)))
                                    .collect(),
                                None => Vec::new(),
                            },
                            points: contour
                                .points
                                .iter()
                                .map(|point| {
                                    if slab.is_some_and(|slab| !slab.contains(*point)) {
                                        return None;
                                    }
                                    self.world_to_window_px(&view_proj, *point)
                                })
                                .collect(),
                            closed: contour.closed,
                        })
                        .collect()
                }
                None => Vec::new(),
            };
            // Every collar of the pattern under review, for the timeline's
            // signal and detonations and the heatmap's surface.
            let timeline = editor.active_drill_hole.filter(|id| editor.review_showing_over(*id)).and_then(|id| {
                drill_holes
                    .iter()
                    .find(|dataset| dataset.id == id && dataset.state.loaded && !editor.hidden_handles.contains(&dataset.entity_id()))
            });
            match timeline {
                Some(dataset) => {
                    let holes = &dataset.dataset.holes;
                    editor.blast_collars_px = holes
                        .iter()
                        .map(|hole| {
                            let collar = hole.collar_position();
                            if slab.is_some_and(|slab| !slab.contains(collar)) {
                                return None;
                            }
                            self.world_to_window_px(&view_proj, collar)
                        })
                        .collect();
                    // Measured at the middle of the pattern, like a card's
                    // scale at its own collar.
                    editor.blast_px_per_world = dataset
                        .dataset
                        .bounds
                        .and_then(|(min, max)| {
                            let centre = (min + max) * 0.5;
                            let at = self.world_to_window_px_unclipped_depth(&view_proj, centre)?;
                            let probe = self.world_to_window_px_unclipped_depth(&view_proj, centre + probe_offset)?;
                            Some((probe.0 - at.0).hypot(probe.1 - at.1) / CARD_SCALE_PROBE_WORLD as f32)
                        })
                        .unwrap_or(0.0);
                }
                None => {
                    editor.blast_collars_px.clear();
                    editor.blast_px_per_world = 0.0;
                }
            }
        } else {
            editor.initiation_cards.clear();
            editor.blast_contours_px.clear();
            editor.blast_collars_px.clear();
        }

        if let Some(failure) = &editor.tri_create_failure {
            let vp = self.view_proj();
            let slab = self.section_slab();
            // Drop a marker or segment outside the slab; a segment needs both ends inside.
            editor.tri_create_diagnostic_markers_screen_px = failure
                .diagnostic
                .markers_world
                .iter()
                .filter(|&&point| slab.is_none_or(|slab| slab.contains(point)))
                .filter_map(|&point| self.world_to_window_px(&vp, point))
                .collect();
            editor.tri_create_diagnostic_segments_screen_px = failure
                .diagnostic
                .segments_world
                .iter()
                .filter(|segment| segment.iter().all(|&point| slab.is_none_or(|slab| slab.contains(point))))
                .map(|segment| segment.map(|point| self.world_to_window_px(&vp, point)))
                .collect();
        } else {
            editor.tri_create_diagnostic_markers_screen_px.clear();
            editor.tri_create_diagnostic_segments_screen_px.clear();
        }

        // The Next Vertex panel tests a typed point where it would land on
        // the section.
        if let Some(panel) = editor.next_vertex_panel.as_mut() {
            panel.section = self.section_slab();
        }

        // Rings follow the edits of their strings and their strings' hiding
        // before they are drawn, then are projected again only when the view
        // or the rings changed: thousands of them must not cost a frame.
        editor.settle_string_rings(document);
        if !editor.string_rings.is_empty() {
            use std::hash::{Hash, Hasher};
            let vp = self.view_proj();
            let slab = self.section_slab();
            let pixels_per_point = self.scale_factor() as f32 * (editor.ui_size_percent as f32 / 100.0);
            let ring_px = crate::ui::state::STRING_RING_RADIUS * pixels_per_point;
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            vp.to_cols_array().map(f64::to_bits).hash(&mut hasher);
            slab.map(|slab| [slab.point.x, slab.point.y, slab.point.z, slab.normal.x, slab.normal.y, slab.normal.z, slab.half_width].map(f64::to_bits))
                .hash(&mut hasher);
            (
                self.size.width,
                self.size.height,
                self.viewport_rect.x,
                self.viewport_rect.y,
                self.viewport_rect.width,
                self.viewport_rect.height,
            )
                .hash(&mut hasher);
            ring_px.to_bits().hash(&mut hasher);
            // Off the window by more than a ring, a ring paints nothing.
            let margin = ring_px + 4.0;
            let bounds = [-margin, -margin, self.size.width as f32 + margin, self.size.height as f32 + margin];
            // A section shows the rings inside its slab.
            editor.project_string_rings(hasher.finish(), ring_px, bounds, |at| {
                slab.is_none_or(|slab| slab.contains(at)).then(|| self.world_to_window_px(&vp, at)).flatten()
            });
        }

        if editor.offset_awaiting_side_pick {
            let vp = self.view_proj();
            // A clipped vertex must keep its slot so preview ranges stay index-aligned.
            editor.offset_preview_screen_px = editor.offset_preview_world.iter().map(|&p| self.world_to_window_px_unclipped_depth(&vp, p)).collect();
        } else {
            editor.offset_preview_screen_px.clear();
            editor.offset_preview_ranges.clear();
        }

        if editor.slice_grid_enabled {
            // The spacing in force is kept; the grid's options open on it.
            let (lines, level_spacing) = self.build_section_grid(editor.section_grid_style.level_spacing);
            editor.section_grid_px = lines;
            if let Some(level_spacing) = level_spacing {
                editor.section_grid_level_spacing = level_spacing;
            }
        } else {
            editor.section_grid_px.clear();
        }

        use crate::ui::state::ActiveTool;
        // Both collar tools stand their gizmo at the middle of the collars
        // they are holding, read from the datasets themselves so it follows a
        // live preview the way the design gizmo follows the moved objects.
        let collar_anchor = editor.active_tool.acts_on_collars().then(|| selected_collar_anchor(editor, drill_holes)).flatten();
        editor.rotate_gizmo = match collar_anchor.filter(|_| editor.active_tool.rotates()) {
            // The dip ring stands in the vertical plane the anchor hole points
            // along, so it always shows the plane its toe would swing in.
            Some((center, azimuth)) => self.project_rotate_gizmo(center, editor.rotate_gizmo_azimuth.unwrap_or(azimuth)),
            None => RotateGizmoScreen::default(),
        };
        if editor.active_tool == ActiveTool::MoveCollar {
            editor.move_gizmo = match collar_anchor {
                Some((center, _)) => self.project_move_gizmo(center),
                None => MoveGizmoScreen::default(),
            };
        } else if editor.active_tool == ActiveTool::Move && (!editor.selected_handles.is_empty() || editor.move_vertex_target.is_some()) {
            let mut sum = DVec3::ZERO;
            let mut count = 0usize;
            if let Some((target_id, point)) = editor.move_vertex_target {
                if let Some(obj) = document.get_object(target_id) {
                    match (obj, point) {
                        (Object::Polyline { verts, .. } | Object::PitDesign { verts, .. }, crate::model::ObjectPoint::Vertex(vertex_index)) => {
                            if let Some(vertex) = verts.get(vertex_index) {
                                sum += vertex.pos;
                                count += 1;
                            }
                        }
                        (Object::Circle { center, .. }, crate::model::ObjectPoint::Center) => {
                            sum += *center;
                            count += 1;
                        }
                        (Object::Point { pos, .. }, crate::model::ObjectPoint::Vertex(0)) => {
                            sum += *pos;
                            count += 1;
                        }
                        _ => {}
                    }
                }
            } else {
                for &handle in &editor.selected_handles {
                    if let SceneEntityId::Object(id) = handle
                        && let Some(obj) = document.get_object(id)
                    {
                        match obj {
                            Object::Polyline { verts, .. } | Object::PitDesign { verts, .. } => {
                                for vertex in verts {
                                    sum += vertex.pos;
                                    count += 1;
                                }
                            }
                            Object::Point { pos, .. } | Object::Text { pos, .. } => {
                                sum += *pos;
                                count += 1;
                            }
                            Object::Circle { center, .. } => {
                                sum += *center;
                                count += 1;
                            }
                        }
                    }
                }
            }
            if count > 0 {
                editor.move_gizmo = self.project_move_gizmo(sum / count as f64);
            } else {
                editor.move_gizmo = MoveGizmoScreen::default();
            }
        } else {
            editor.move_gizmo = MoveGizmoScreen::default();
        }

        if editor.active_tool == ActiveTool::Chamfer {
            use crate::app::commands::drawing::chamfer::{chamfer_corner_on, chamfers_corner};
            let vp = self.view_proj();
            let slab = (editor.slice_mode_enabled && editor.active_workspace.edits_in_slice_view())
                .then(|| self.section_slab())
                .flatten();
            // Preview + gizmo geometry is a foreground overlay offset from the
            // source corner, so keep it drawable even when a tightly-fitted
            // depth slab would reject the displaced point.
            let project = |w: DVec3| -> Option<(f32, f32)> { self.world_to_window_px_unclipped_depth(&vp, w) };

            let corner_data = editor.chamfer_poly_id.and_then(|oid| {
                let ci = editor.chamfer_corner_index?;
                if let Some(Object::Polyline { verts, closed, .. }) = document.get_object(oid) {
                    chamfers_corner(verts, *closed, ci, slab.is_some()).then(|| (verts.clone(), ci))
                } else {
                    None
                }
            });

            if let Some((ref verts, ci)) = corner_data {
                use crate::app::commands::drawing::chamfer::chamfer_max_radius_on;
                editor.chamfer_max_radius = chamfer_max_radius_on(verts, ci, slab);
                editor.chamfer_radius = editor.chamfer_radius.min(editor.chamfer_max_radius);
                let chamfered = chamfer_corner_on(verts, ci, editor.chamfer_radius, editor.chamfer_segments, slab);
                editor.chamfer_preview_screen_px = chamfered.iter().map(|v| project(v.pos)).collect();
                editor.chamfer_hover_corner_px = None;

                let n = verts.len();
                let corner = verts[ci].pos;
                let next = verts[(ci + 1) % n].pos;
                let c_px = project(corner);
                let n_px = project(next);
                let edge_stub_dir = if let (Some(cp), Some(np)) = (c_px, n_px) {
                    let dx = np.0 - cp.0;
                    let dy = np.1 - cp.1;
                    let l = (dx * dx + dy * dy).sqrt().max(1e-6);
                    Some((dx / l, dy / l))
                } else {
                    None
                };
                editor.chamfer_gizmo_bisector_px = edge_stub_dir;

                // The handle runs along the edge, in the radius's plane.
                let (corner_k, next_k) = slab.map_or((corner, next), |slab| (slab.frame_coords(corner), slab.frame_coords(next)));
                let edge_2d = DVec2::new(next_k.x - corner_k.x, next_k.y - corner_k.y);
                let edge_len = edge_2d.length();
                if edge_len > 1e-10 {
                    let edge_dir = edge_2d / edge_len;
                    let handle_k = DVec3::new(corner_k.x + edge_dir.x * editor.chamfer_radius, corner_k.y + edge_dir.y * editor.chamfer_radius, corner_k.z);
                    let handle_world = slab.map_or(handle_k, |slab| slab.world_from_frame(handle_k));
                    editor.chamfer_gizmo_corner_px = c_px;
                    editor.chamfer_gizmo_handle_px = project(handle_world);
                    if let (Some(cp), Some(np)) = (c_px, n_px) {
                        let dx = np.0 - cp.0;
                        let dy = np.1 - cp.1;
                        let screen_edge_len = (dx * dx + dy * dy).sqrt();
                        if screen_edge_len > 1e-4 {
                            editor.chamfer_gizmo_edge_screen_dir = Some((dx / screen_edge_len, dy / screen_edge_len));
                            editor.chamfer_gizmo_px_per_world = screen_edge_len as f64 / edge_len;
                        }
                    }
                } else {
                    editor.chamfer_gizmo_corner_px = None;
                    editor.chamfer_gizmo_handle_px = None;
                }
            } else {
                editor.chamfer_preview_screen_px.clear();
                editor.chamfer_gizmo_corner_px = None;
                editor.chamfer_gizmo_handle_px = None;
                editor.chamfer_gizmo_bisector_px = None;
                editor.chamfer_gizmo_edge_screen_dir = None;
                editor.chamfer_max_radius = f64::MAX;
            }
        } else {
            editor.chamfer_preview_screen_px.clear();
            editor.chamfer_hover_corner_px = None;
            editor.chamfer_gizmo_corner_px = None;
            editor.chamfer_gizmo_handle_px = None;
            editor.chamfer_gizmo_bisector_px = None;
            editor.chamfer_gizmo_edge_screen_dir = None;
            editor.chamfer_gizmo_drag_start_px = None;
            editor.chamfer_gizmo_hovered = false;
            editor.chamfer_max_radius = f64::MAX;
        }

        if editor.active_tool == ActiveTool::Bezier {
            use crate::app::commands::drawing::bezier::{bezier_eval, directed_span_points};
            let vp = self.view_proj();
            // Control points and the curve they bow toward are a foreground
            // overlay that can sit well off the source line's depth, so project
            // without the scene-fitted depth slab's rejection.
            let project = |w: DVec3| -> Option<(f32, f32)> { self.world_to_window_px_unclipped_depth(&vp, w) };

            if let Some(oid) = editor.bezier_poly_id {
                if let Some(Object::Polyline { verts, closed, .. }) = document.get_object(oid) {
                    let verts = verts.clone();
                    let n = verts.len();
                    editor.bezier_poly_closed = *closed;

                    let slab = self.section_slab();
                    editor.bezier_poly_verts_screen_px = verts
                        .iter()
                        .map(|v| slab.is_none_or(|slab| slab.contains(v.pos)).then(|| project(v.pos)).flatten())
                        .collect();

                    if let [Some(vi), Some(vj)] = editor.bezier_selected_verts
                        && vi < n
                        && vj < n
                    {
                        let cp1 = DVec3::from(editor.bezier_cp1);
                        let cp2 = DVec3::from(editor.bezier_cp2);
                        editor.bezier_cp1_screen_px = project(cp1);
                        editor.bezier_cp2_screen_px = project(cp2);

                        let segs = editor.bezier_segments.max(2) as usize;
                        let source_span = directed_span_points(&verts, *closed, vi, vj);
                        editor.bezier_span_screen_px = source_span.iter().map(|&point| project(point)).collect();

                        let start = verts[vi].pos;
                        let end = verts[vj].pos;
                        editor.bezier_preview_screen_px = (0..=segs)
                            .map(|sample| {
                                let t = sample as f64 / segs as f64;
                                project(bezier_eval(start, cp1, cp2, end, t))
                            })
                            .collect();
                    } else {
                        editor.bezier_cp1_screen_px = None;
                        editor.bezier_cp2_screen_px = None;
                        editor.bezier_span_screen_px.clear();
                        editor.bezier_preview_screen_px.clear();
                    }
                } else {
                    editor.bezier_poly_verts_screen_px.clear();
                    editor.bezier_cp1_screen_px = None;
                    editor.bezier_cp2_screen_px = None;
                    editor.bezier_span_screen_px.clear();
                    editor.bezier_preview_screen_px.clear();
                }
            } else {
                editor.bezier_poly_verts_screen_px.clear();
                editor.bezier_cp1_screen_px = None;
                editor.bezier_cp2_screen_px = None;
                editor.bezier_span_screen_px.clear();
                editor.bezier_preview_screen_px.clear();
            }
        } else {
            editor.bezier_poly_verts_screen_px.clear();
            editor.bezier_cp1_screen_px = None;
            editor.bezier_cp2_screen_px = None;
            editor.bezier_span_screen_px.clear();
            editor.bezier_preview_screen_px.clear();
            editor.bezier_hover_cp = None;
            editor.bezier_dragging_cp = None;
        }

        if editor.active_tool == ActiveTool::SplitAtPoints {
            let vp = self.view_proj();
            let slab = self.section_slab();
            if let Some(oid) = editor.split_poly_id {
                if let Some(Object::Polyline { verts, .. }) = document.get_object(oid) {
                    editor.split_poly_verts_screen_px = verts
                        .iter()
                        .map(|v| slab.is_none_or(|slab| slab.contains(v.pos)).then(|| self.world_to_window_px(&vp, v.pos)).flatten())
                        .collect();
                } else {
                    editor.split_poly_verts_screen_px.clear();
                }
            } else {
                editor.split_poly_verts_screen_px.clear();
            }
        } else {
            editor.split_poly_verts_screen_px.clear();
        }

        {
            use crate::ui::state::{RelimitMode, TrimEnd};
            let vp = self.view_proj();
            let screen = self.screen_size();

            editor.relimit_intersection_3d = None;
            editor.relimit_preview_from_px = None;
            editor.relimit_preview_to_px = None;
            if editor.relimit_confirming_end
                && let Some(cursor) = editor.cursor_screen_px
                && let Some((start, end)) = editor.relimit_source_id.and_then(|oid| match document.get_object(oid) {
                    Some(Object::Polyline { verts, .. }) if verts.len() >= 2 => Some((verts.first()?.pos, verts.last()?.pos)),
                    _ => None,
                })
                && let Some((index, (from, to))) =
                    projected_relimit_candidate_nearest_cursor(self.window_to_viewport_px(cursor), &editor.relimit_candidates, start, end, &vp, screen)
                && let Some(candidate) = editor.relimit_candidates.get(index)
            {
                editor.relimit_hover_end = candidate.end;
                editor.relimit_intersection_3d = Some(candidate.target);
                editor.relimit_preview_is_extension = candidate.is_extension;
                editor.relimit_preview_from_px = Some(self.viewport_to_window_px(from));
                editor.relimit_preview_to_px = Some(self.viewport_to_window_px(to));
            }

            let show = editor.relimit_dialog_open && matches!(editor.relimit_mode, RelimitMode::AbsoluteLength | RelimitMode::RelativeLength);
            if show {
                editor.relimit_resize_end_px = editor.relimit_source_id.and_then(|oid| {
                    if let Some(Object::Polyline { verts, .. }) = document.get_object(oid) {
                        let v = match editor.relimit_resize_end {
                            TrimEnd::Start => verts.first()?.pos,
                            TrimEnd::End => verts.last()?.pos,
                        };
                        self.world_to_window_px(&vp, v)
                    } else {
                        None
                    }
                });
            } else {
                editor.relimit_resize_end_px = None;
            }
        }
    }
}

impl Graphics<'_> {
    /// The ground spacing between contour labels for the view: the least power
    /// of two that clears [`CONTOUR_LABEL_SPACING_POINTS`] on screen, measured
    /// at the middle of the contours.
    fn contour_label_spacing_world(&self, contours: &[crate::model::blast::TimeContour], view_proj: &glam::DMat4, probe_offset: DVec3) -> Option<f64> {
        let mut points = contours.iter().flat_map(|contour| contour.points.iter().copied());
        let first = points.next()?;
        let (min, max) = points.fold((first, first), |(min, max), point| (min.min(point), max.max(point)));
        let middle = (min + max) * 0.5;
        let at = self.world_to_window_px(view_proj, middle)?;
        let probe = self.world_to_window_px(view_proj, middle + probe_offset)?;
        let px_per_world = f64::from((probe.0 - at.0).hypot(probe.1 - at.1)) / CARD_SCALE_PROBE_WORLD;
        if px_per_world <= 0.0 || !px_per_world.is_finite() {
            return None;
        }
        let wanted = CONTOUR_LABEL_SPACING_POINTS * self.scale_factor() / px_per_world;
        Some(2f64.powf(wanted.log2().ceil()))
    }
}

/// The angle between the points of a ramp's grade sector.
const GRADE_ARC_STEP: f64 = 0.02;

/// How far apart the places the walls' gizmo may stand are tried along the
/// string, how far from any ramp's start they must be, in metres, and how
/// far inside the clear view, in logical points.
const WALL_GIZMO_STEP: f64 = 5.0;
const WALL_GIZMO_MARGIN: f64 = 130.0;

/// How far on screen the walls' gizmo is put from the ramps' feet, when it is
/// put somewhere, in logical points: clear of their gizmos.
pub(crate) const WALL_GIZMO_CLEAR_POINTS: f64 = 170.0;

/// How near a ramp's start the walls' gizmo may stand before it is put
/// somewhere else, in metres: on it, the two would cover each other.
const WALL_GIZMO_RAMP_ON: f64 = 15.0;

/// The walls' section's marks, in logical points: how far off its face the
/// mark of its height stands, the radius of the arc of its face's angle, and
/// how far the floor is drawn behind its toe, each no more than the bench
/// it marks allows; the reach a metre's pixels are measured over; and the
/// arc's steps.
const HEIGHT_MARK_POINTS: f64 = 18.0;
const ANGLE_ARC_POINTS: f64 = 30.0;
const FLOOR_STUB_POINTS: f64 = 30.0;
const SECTION_PROBE_POINTS: f64 = 50.0;
const ANGLE_ARC_STEPS: usize = 24;

/// How far along the string from a ramp's foot the arrow saying which way
/// it climbs reaches, in logical points.
const CLIMB_ARROW_POINTS: f64 = 46.0;

/// Which way along `axis` a vector points: 1 or -1.
fn next_sign(vector: glam::DVec2, axis: glam::DVec2) -> f64 {
    if vector.dot(axis) < 0.0 { -1.0 } else { 1.0 }
}

/// Where a ramp's centreline first reaches `rl`: at a point of it, or
/// between two that straddle it.
fn at_rl(line: &[DVec3], rl: f64) -> Option<DVec3> {
    line.iter().find(|point| (point.z - rl).abs() < 1.0e-3).copied().or_else(|| {
        line.windows(2).find_map(|pair| {
            let (a, b) = (pair[0], pair[1]);
            ((a.z - rl) * (b.z - rl) < 0.0).then(|| a.lerp(b, (rl - a.z) / (b.z - a.z)))
        })
    })
}
