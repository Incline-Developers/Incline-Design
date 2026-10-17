//! World-to-screen projection updates for UI/tool overlays.

use super::*;
use crate::{
    app::commands::drawing::rotate_collar::{ring_axis, ring_basis},
    model::geometry::{ContourLabelLine, ContourLevelCache, ContourLevels, contour_label_spots, contour_label_spots_along, label_middle},
    rendering::section_grid,
    ui::state::{MoveGizmoScreen, ROTATE_GIZMO_AZIMUTH_RING, ROTATE_GIZMO_DIP_RING, RotateGizmoScreen, SectionGridAxis, SectionGridLine, SectionGridLineKind},
};

pub(crate) type ScreenSegmentPx = ((f32, f32), (f32, f32));

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
            (GIZMO_LENGTH_POINTS * self.window.scale_factor()) as f32,
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
            (GIZMO_LENGTH_POINTS * self.window.scale_factor()) as f32,
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
        let points_per_pixel = 1.0 / self.window.scale_factor();
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

    pub(super) fn update_tool_projections(&self, editor: &mut EditorState, document: &Document, drill_holes: &[OpenDrillHoleDataset]) {
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
                    let spacing = points_middle(analysis.contours.iter().flat_map(|contour| contour.points.iter().copied()))
                        .and_then(|middle| self.contour_label_spacing_world(middle, probe_offset, |point| self.world_to_window_px(&view_proj, point)));
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

        if editor.offset_awaiting_side_pick {
            let vp = self.view_proj();
            // A clipped vertex must keep its slot so preview ranges stay index-aligned.
            editor.offset_preview_screen_px = editor.offset_preview_world.iter().map(|&p| self.world_to_window_px_unclipped_depth(&vp, p)).collect();
        } else {
            editor.offset_preview_screen_px.clear();
            editor.offset_preview_ranges.clear();
        }

        if editor.batter_berm_dialog_open && !editor.batter_berm_rings_world.is_empty() {
            let vp = self.view_proj();
            editor.batter_berm_source_screen_px = editor.batter_berm_source_world.iter().map(|&p| self.world_to_window_px_unclipped_depth(&vp, p)).collect();
            editor.batter_berm_rings_screen_px = editor
                .batter_berm_rings_world
                .iter()
                .map(|ring| ring.iter().map(|&p| self.world_to_window_px_unclipped_depth(&vp, p)).collect())
                .collect();
        } else {
            editor.batter_berm_source_screen_px.clear();
            editor.batter_berm_rings_screen_px.clear();
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
                        (Object::Polyline { verts, .. }, crate::model::ObjectPoint::Vertex(vertex_index)) => {
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
                            Object::Polyline { verts, .. } => {
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
            use crate::app::commands::drawing::chamfer::chamfer_corner;
            let vp = self.view_proj();
            // Preview + gizmo geometry is a foreground overlay offset from the
            // source corner, so keep it drawable even when a tightly-fitted
            // depth slab would reject the displaced point.
            let project = |w: DVec3| -> Option<(f32, f32)> { self.world_to_window_px_unclipped_depth(&vp, w) };

            let corner_data = editor.chamfer_poly_id.and_then(|oid| {
                let ci = editor.chamfer_corner_index?;
                if let Some(Object::Polyline { verts, closed: true, .. }) = document.get_object(oid) {
                    (verts.len() >= 3 && ci < verts.len()).then(|| (verts.clone(), ci))
                } else {
                    None
                }
            });

            if let Some((ref verts, ci)) = corner_data {
                use crate::app::commands::drawing::chamfer::chamfer_max_radius;
                editor.chamfer_max_radius = chamfer_max_radius(verts, ci);
                editor.chamfer_radius = editor.chamfer_radius.min(editor.chamfer_max_radius);
                let chamfered = chamfer_corner(verts, ci, editor.chamfer_radius, editor.chamfer_segments);
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

                let edge_2d = DVec2::new(next.x - corner.x, next.y - corner.y);
                let edge_len = edge_2d.length();
                if edge_len > 1e-10 {
                    let edge_dir = edge_2d / edge_len;
                    let handle_world = DVec3::new(corner.x + edge_dir.x * editor.chamfer_radius, corner.y + edge_dir.y * editor.chamfer_radius, corner.z);
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

                    editor.bezier_poly_verts_screen_px = verts.iter().map(|v| project(v.pos)).collect();

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
            if let Some(oid) = editor.split_poly_id {
                if let Some(Object::Polyline { verts, .. }) = document.get_object(oid) {
                    editor.split_poly_verts_screen_px = verts.iter().map(|v| self.world_to_window_px(&vp, v.pos)).collect();
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
    /// at `middle`, the middle of the contours, through `project` (to window
    /// pixels).
    fn contour_label_spacing_world(&self, middle: DVec3, probe_offset: DVec3, project: impl Fn(DVec3) -> Option<(f32, f32)>) -> Option<f64> {
        let at = project(middle)?;
        let probe = project(middle + probe_offset)?;
        label_spacing(f64::from((probe.0 - at.0).hypot(probe.1 - at.1)) / CARD_SCALE_PROBE_WORLD, self.window.scale_factor())
    }

    /// The contour-labelled lines as last drawn, for a plot sheet.
    pub(crate) fn contour_sheet_lines(&self) -> Vec<crate::model::plot::SheetContourLine> {
        self.contour_labels.sheet_lines()
    }

    /// Where the labels of the contour-labelled layers stand on screen, in
    /// placing order: roundest heights first. `composite_key` is the
    /// workspace key `document` was composed at, and `levels` what each
    /// labelled layer's levels say. Only the stretches of line in view are
    /// walked, each spaced for its own size on screen.
    pub(super) fn update_contour_label_projections(&mut self, editor: &mut EditorState, document: &Document, composite_key: Option<u64>, levels: &ContourLevelCache) {
        editor.contour_labels_px.clear();
        let Some(composite_key) = composite_key.filter(|_| document.layers().iter().any(|layer| layer.contour_labels)) else {
            self.contour_labels.clear();
            return;
        };
        let mut cache = std::mem::take(&mut self.contour_labels);
        cache.refresh(composite_key, document, editor, levels);
        let view_proj = self.view_proj();
        let probe_offset = self.camera.forward().cross(self.camera.up()).normalize_or_zero() * CARD_SCALE_PROBE_WORLD;
        // Past the far plane still measures; behind or too near the eye not.
        let project = |point: DVec3| in_front(&view_proj, point).then(|| self.world_to_window_px_unclipped_depth(&view_proj, point)).flatten();
        let spacing_at = |at: DVec3| self.contour_label_spacing_world(at, probe_offset, project);
        let slab = self.section_slab();
        let frustum = super::frustum::Frustum::from_view_proj(glam::Mat4::from_cols_array_2d(&self.camera_uniform.view_proj));
        let mut spots = Vec::new();
        for index in 0..cache.order.len() {
            spots.clear();
            let id = cache.order[index];
            cache.visible_spots(id, &view_proj, &frustum, self.scene_origin, spacing_at, &mut spots);
            let projected: Vec<_> = spots
                .iter()
                .filter(|(at, _)| !slab.is_some_and(|slab| !slab.contains(*at)))
                .filter_map(|&(at, ahead)| Some((self.world_to_window_px(&view_proj, at)?, self.world_to_window_px(&view_proj, ahead)?)))
                .collect();
            if !projected.is_empty() {
                let line = &cache.lines[&id];
                editor.contour_labels_px.push(crate::ui::state::ProjectedLineLabels {
                    text: line.text.clone(),
                    color: line.color,
                    quiet: line.quiet,
                    spots: projected,
                });
            }
        }
        self.contour_labels = cache;
    }
}

/// Whether `point` stands in front of the near plane of `view_proj`, where
/// a measure of its size on screen means anything.
fn in_front(view_proj: &glam::DMat4, point: DVec3) -> bool {
    let clip = *view_proj * point.extend(1.0);
    clip.w > 0.0 && clip.z <= clip.w
}

/// Most labels one chunk's visible part may get, whatever its spacing.
const MOST_CHUNK_SPOTS: f64 = 16.0;

/// The least spacing that keeps `seen` metres of line to
/// [`MOST_CHUNK_SPOTS`], a power of two like every label spacing.
fn least_spacing(seen: f64) -> f64 {
    if seen > 0.0 { 2f64.powf((seen / MOST_CHUNK_SPOTS).log2().ceil()) } else { 0.0 }
}

/// The parts of `run`'s segments that `spans` keeps, one span per segment of
/// some length, in order (as [`contour_label_spots_along`] asks for them).
fn visible_pieces<'a>(run: &'a [DVec3], spans: impl Iterator<Item = Option<(f64, f64)>> + 'a) -> impl Iterator<Item = (DVec3, DVec3)> + 'a {
    run.windows(2)
        .filter(|pair| pair[0].distance(pair[1]) > 0.0)
        .zip(spans)
        .filter_map(|(pair, span)| span.map(|(from, to)| (pair[0].lerp(pair[1], from), pair[0].lerp(pair[1], to))))
}

/// The point `distance` along `pieces`.
fn point_along(pieces: impl Iterator<Item = (DVec3, DVec3)>, distance: f64) -> Option<DVec3> {
    let mut left = distance;
    for (a, b) in pieces {
        let piece = a.distance(b);
        if piece >= left && piece > 0.0 {
            return Some(a.lerp(b, left / piece));
        }
        left -= piece;
    }
    None
}

/// The point halfway along the part of `run` that `spans` keeps, and that
/// part's length, or `None` when none of it is kept.
fn visible_middle(run: &[DVec3], spans: impl Iterator<Item = Option<(f64, f64)>> + Clone) -> Option<(DVec3, f64)> {
    let seen: f64 = visible_pieces(run, spans.clone()).map(|(a, b)| a.distance(b)).sum();
    if seen <= 0.0 {
        return None;
    }
    point_along(visible_pieces(run, spans), seen * 0.5).map(|middle| (middle, seen))
}

/// The least power of two, in world units, that keeps labels
/// [`CONTOUR_LABEL_SPACING_POINTS`] apart at `px_per_world` pixels per world
/// unit.
fn label_spacing(px_per_world: f64, scale_factor: f64) -> Option<f64> {
    if px_per_world <= 0.0 || !px_per_world.is_finite() {
        return None;
    }
    let wanted = CONTOUR_LABEL_SPACING_POINTS * scale_factor / px_per_world;
    Some(2f64.powf(wanted.log2().ceil()))
}

/// The part of segment `a`-`b` inside the view of `view_proj`, as a range of
/// 0 to 1 along it.
fn visible_span(view_proj: &glam::DMat4, a: DVec3, b: DVec3) -> Option<(f64, f64)> {
    let sides = |point: DVec3| {
        let clip = *view_proj * point.extend(1.0);
        [clip.w + clip.x, clip.w - clip.x, clip.w + clip.y, clip.w - clip.y, clip.z, clip.w - clip.z]
    };
    let (mut from, mut to) = (0.0f64, 1.0f64);
    for (side_a, side_b) in sides(a).into_iter().zip(sides(b)) {
        if side_a < 0.0 && side_b < 0.0 {
            return None;
        }
        if side_a < 0.0 {
            from = from.max(side_a / (side_a - side_b));
        } else if side_b < 0.0 {
            to = to.min(side_a / (side_a - side_b));
        }
    }
    (from <= to).then_some((from, to))
}

/// Whether the box `min`-`max` lies wholly inside the view of `view_proj`.
fn box_in_view(view_proj: &glam::DMat4, min: DVec3, max: DVec3) -> bool {
    (0..8).all(|corner| {
        let point = DVec3::new(
            if corner & 1 == 0 { min.x } else { max.x },
            if corner & 2 == 0 { min.y } else { max.y },
            if corner & 4 == 0 { min.z } else { max.z },
        );
        let clip = *view_proj * point.extend(1.0);
        clip.x.abs() <= clip.w && clip.y.abs() <= clip.w && clip.z >= 0.0 && clip.z <= clip.w
    })
}

/// Middle of the box around `points`.
fn points_middle(mut points: impl Iterator<Item = DVec3>) -> Option<DVec3> {
    let first = points.next()?;
    let (min, max) = points.fold((first, first), |(min, max), point| (min.min(point), max.max(point)));
    Some((min + max) * 0.5)
}

/// What a layer's levels say before its summary arrives: nothing round.
static UNKNOWN_LEVELS: ContourLevels = ContourLevels { decimals: 0, steps: Vec::new() };

/// Segments per chunk a labelled line is cut into for culling and spacing.
const LABEL_CHUNK_SEGMENTS: usize = 32;

/// A run of a labelled line's segments: its first and last point, how far
/// along the line it starts, its length and the point halfway along it, its
/// bounds, and its spots once walked whole.
struct LabelChunk {
    first: usize,
    last: usize,
    start: f64,
    length: f64,
    middle: DVec3,
    min: DVec3,
    max: DVec3,
    spots: Option<WalkedSpots>,
}

/// Spots walked at a spacing and the line's middle mode, kept with both.
struct WalkedSpots {
    spacing: f64,
    middle: Option<f64>,
    spots: Vec<(DVec3, DVec3)>,
}

/// One labelled line as drawn: the polyline revision it was built from, its
/// layer and colour, its text and how round its level is, and its chunks.
struct LabelledLine {
    revision: u64,
    layer: crate::model::LayerId,
    line: std::sync::Arc<ContourLabelLine>,
    color: [f32; 4],
    decimals: usize,
    text: std::sync::Arc<str>,
    rank: usize,
    quiet: bool,
    length: f64,
    chunks: Vec<LabelChunk>,
}

impl LabelledLine {
    fn new(line: ContourLabelLine, revision: u64, layer: crate::model::LayerId) -> Self {
        let mut chunks = Vec::new();
        let mut walked = 0.0;
        for first in (0..line.points.len() - 1).step_by(LABEL_CHUNK_SEGMENTS) {
            let last = (first + LABEL_CHUNK_SEGMENTS).min(line.points.len() - 1);
            let points = &line.points[first..=last];
            let (min, max) = points.iter().fold((points[0], points[0]), |(min, max), point| (min.min(*point), max.max(*point)));
            let start = walked;
            for pair in points.windows(2) {
                walked += pair[0].distance(pair[1]);
            }
            let (middle, length) = visible_middle(points, std::iter::repeat(Some((0.0, 1.0)))).unwrap_or((points[0], 0.0));
            chunks.push(LabelChunk {
                first,
                last,
                start,
                length,
                middle,
                min,
                max,
                spots: None,
            });
        }
        Self {
            revision,
            layer,
            line: std::sync::Arc::new(line),
            color: [1.0; 4],
            decimals: usize::MAX,
            text: std::sync::Arc::from(""),
            rank: 0,
            quiet: false,
            length: walked,
            chunks,
        }
    }

    /// Take the layer's colour and what its levels say; the text is written
    /// again only when the decimals change.
    fn restyle(&mut self, color: [f32; 4], levels: &ContourLevels) {
        self.color = color;
        if self.decimals != levels.decimals {
            self.decimals = levels.decimals;
            self.text = crate::model::plot::format_level(self.line.level, levels.decimals).into();
        }
        self.rank = levels.roundness(self.line.level);
        self.quiet = levels.quiet(self.line.level);
    }
}

/// The contour-labelled lines of the drawn document, kept between frames in
/// placing order. A line is rebuilt only when its polyline's revision
/// changes; a chunk's spots are walked once it is wholly in view and again
/// only when its spacing or middle label changes. Chunks at the edge of the
/// view are walked each frame, but only where seen.
#[derive(Default)]
pub(crate) struct ContourLabelCache {
    /// The workspace key, the hidden set's size and the isolated entity at
    /// the last refresh: cheap to compare every frame.
    gate: Option<(u64, usize, Option<SceneEntityId>)>,
    /// Moves on every rebuild, so a copy of the lines can tell it is stale.
    generation: u64,
    lines: std::collections::HashMap<crate::model::ObjectId, LabelledLine>,
    /// Roundest level first, the document's order within a rank.
    order: Vec<crate::model::ObjectId>,
    /// Reused each frame: the chunks of a line in view, and the clipped spans
    /// of its chunks at the edge of the view.
    in_view: Vec<ChunkInView>,
    spans: Vec<Option<(f64, f64)>>,
}

/// A chunk of a line in view: the spacing asked for at its middle, the
/// least its part in view allows, how much of it is seen, and, for a chunk
/// at the edge, where its clipped spans sit in the cache's `spans`.
struct ChunkInView {
    index: usize,
    asked: f64,
    least: f64,
    seen: f64,
    edge: Option<std::ops::Range<usize>>,
}

impl ContourLabelCache {
    /// Drop every line. The generation still moves on, so nothing that
    /// copied the lines before mistakes them for current.
    fn clear(&mut self) {
        if self.gate.is_some() || !self.lines.is_empty() {
            *self = Self {
                generation: self.generation.wrapping_add(1),
                ..Self::default()
            };
        }
    }

    /// Which rebuild the lines are at.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// The labelled lines as drawn, roundest first, for a plot sheet to
    /// label as the screen does: shared, not copied.
    pub(crate) fn sheet_lines(&self) -> Vec<crate::model::plot::SheetContourLine> {
        self.order
            .iter()
            .map(|id| {
                let labelled = &self.lines[id];
                crate::model::plot::SheetContourLine {
                    line: std::sync::Arc::clone(&labelled.line),
                    text: std::sync::Arc::clone(&labelled.text),
                    rank: labelled.rank,
                    quiet: labelled.quiet,
                    color: labelled.color,
                }
            })
            .collect()
    }

    /// Bring the lines up to date with `document` when the gate moves:
    /// lines whose polyline is unchanged are kept as they are.
    fn refresh(&mut self, composite_key: u64, document: &Document, editor: &EditorState, levels: &ContourLevelCache) {
        let gate = (composite_key, editor.hidden_handles.len(), editor.isolated_entity);
        if self.gate == Some(gate) {
            return;
        }
        self.gate = Some(gate);
        self.generation = self.generation.wrapping_add(1);
        let labelled: std::collections::HashSet<_> = document.layers().iter().filter(|layer| layer.contour_labels).map(|layer| layer.id).collect();
        let mut previous = std::mem::take(&mut self.lines);
        self.order.clear();
        for object in document.objects() {
            let Object::Polyline { verts, closed, layer, .. } = object else {
                continue;
            };
            if !labelled.contains(layer) || editor.hidden_handles.contains(&SceneEntityId::Object(object.id())) {
                continue;
            }
            let revision = document.object_revision(object.id());
            let Some(level) = levels.level(object.id(), revision) else {
                continue;
            };
            let mut line = match previous.remove(&object.id()) {
                Some(line) if line.revision == revision && line.layer == *layer => line,
                _ => match ContourLabelLine::new(level, verts, *closed) {
                    Some(line) => LabelledLine::new(line, revision, *layer),
                    None => continue,
                },
            };
            line.restyle(document.object_rgba(object), levels.layer(*layer).unwrap_or(&UNKNOWN_LEVELS));
            self.order.push(object.id());
            self.lines.insert(object.id(), line);
        }
        let lines = &self.lines;
        self.order.sort_by_key(|id| std::cmp::Reverse(lines[id].rank));
    }

    /// Line `id`'s spots in view. Each chunk is spaced by `spacing_at` the
    /// middle of its part in view, and never so tightly that the part gets
    /// more than [`MOST_CHUNK_SPOTS`]. Whether the line gets one middle label,
    /// and whether a short line is spaced as one, is decided from the
    /// spacing at the middle of the line's whole part in view, so panning a
    /// plan view never changes it. Chunks wholly inside are cached, chunks at
    /// the edge walked only where seen, each clipped once, and a chunk with
    /// no spacing, as one behind the eye, gets none.
    fn visible_spots(
        &mut self,
        id: crate::model::ObjectId,
        view_proj: &glam::DMat4,
        frustum: &super::frustum::Frustum,
        scene_origin: DVec3,
        spacing_at: impl Fn(DVec3) -> Option<f64>,
        spots: &mut Vec<(DVec3, DVec3)>,
    ) {
        let Self { lines, in_view, spans, .. } = self;
        let Some(labelled) = lines.get_mut(&id) else {
            return;
        };
        let line = &labelled.line;
        if !frustum.intersects_aabb((line.min - scene_origin).as_vec3(), (line.max - scene_origin).as_vec3()) {
            return;
        }
        let line_inside = box_in_view(view_proj, line.min, line.max);
        in_view.clear();
        spans.clear();
        for (index, chunk) in labelled.chunks.iter().enumerate() {
            if !line_inside && !frustum.intersects_aabb((chunk.min - scene_origin).as_vec3(), (chunk.max - scene_origin).as_vec3()) {
                continue;
            }
            let run = &line.points[chunk.first..=chunk.last];
            let (measured, edge) = if line_inside || box_in_view(view_proj, chunk.min, chunk.max) {
                (Some((chunk.middle, chunk.length)), None)
            } else {
                let from = spans.len();
                spans.extend(
                    run.windows(2)
                        .filter(|pair| pair[0].distance(pair[1]) > 0.0)
                        .map(|pair| visible_span(view_proj, pair[0], pair[1])),
                );
                (visible_middle(run, spans[from..].iter().copied()), Some(from..spans.len()))
            };
            if let Some((at, seen)) = measured
                && let Some(asked) = spacing_at(at)
            {
                in_view.push(ChunkInView {
                    index,
                    asked,
                    least: least_spacing(seen),
                    seen,
                    edge,
                });
            }
        }
        // The middle of the line's whole part in view.
        let mut left = in_view.iter().map(|chunk| chunk.seen).sum::<f64>() * 0.5;
        let mut line_middle = None;
        for chunk in in_view.iter() {
            if chunk.seen >= left {
                let run = &line.points[labelled.chunks[chunk.index].first..=labelled.chunks[chunk.index].last];
                line_middle = match &chunk.edge {
                    Some(range) => point_along(visible_pieces(run, spans[range.clone()].iter().copied()), left),
                    None => point_along(visible_pieces(run, std::iter::repeat(Some((0.0, 1.0)))), left),
                };
                break;
            }
            left -= chunk.seen;
        }
        let Some(line_spacing) = line_middle.and_then(&spacing_at) else {
            return;
        };
        let length = labelled.length;
        let middle = label_middle(length, line_spacing);
        // A line no longer than two of its spacings is spaced as one, so its
        // few labels cannot fall between its chunks, but never tighter than
        // any of its chunks allows.
        let uniform = (length <= line_spacing * 2.0).then(|| in_view.iter().fold(line_spacing, |spacing, chunk| spacing.max(chunk.least)));
        for chunk_in_view in in_view.iter() {
            let spacing = uniform.unwrap_or(chunk_in_view.asked.max(chunk_in_view.least));
            let chunk = &mut labelled.chunks[chunk_in_view.index];
            let run = &labelled.line.points[chunk.first..=chunk.last];
            match &chunk_in_view.edge {
                None => {
                    if chunk.spots.as_ref().is_none_or(|walked| walked.spacing != spacing || walked.middle != middle) {
                        let mut walked = Vec::new();
                        contour_label_spots_along(run, chunk.start, length, spacing, middle, |_, _| Some((0.0, 1.0)), &mut walked);
                        chunk.spots = Some(WalkedSpots { spacing, middle, spots: walked });
                    }
                    spots.extend_from_slice(chunk.spots.as_ref().map_or(&[][..], |walked| &walked.spots));
                }
                Some(range) => {
                    let mut clipped = spans[range.clone()].iter().copied();
                    contour_label_spots_along(run, chunk.start, length, spacing, middle, |_, _| clipped.next().flatten(), spots);
                }
            }
        }
    }
}
