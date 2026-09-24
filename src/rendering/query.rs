//! Unified scene interaction queries, independent of GPU buffer ownership.

use std::{borrow::Cow, collections::HashSet};

use glam::{DMat4, DVec2, DVec3};

use crate::{
    Size,
    model::{
        Document, SceneEntityId,
        drill_hole::{
            COLLAR_MARKER_MIN_PIXEL_DIAMETER, COLLAR_MARKER_RADIUS_SCALE, DISC_MIN_PIXEL_LENGTH, DrillHoleRef, DrillHoleStyle, HoleDisc, OpenDrillHoleDataset, hole_discs,
        },
        spatial::ObjectSnapIndex,
        triangulation::OpenTriangulation,
    },
    rendering::{
        camera::SectionSlab,
        scene::{drill_hole_cache::DiscSpans, gpu_cache::ray_aabb_distance},
        snap,
    },
    ui::state::CursorMode,
};

pub(crate) struct SceneQuery;

impl SceneQuery {
    pub(crate) fn nearest_surface(
        triangulations: &[OpenTriangulation],
        hidden: &HashSet<SceneEntityId>,
        frozen: Option<&HashSet<SceneEntityId>>,
        ray_origin: DVec3,
        ray_direction: DVec3,
    ) -> Option<(SceneEntityId, DVec3)> {
        triangulations
            .iter()
            .filter(|triangulation| {
                let entity = triangulation.entity_id();
                triangulation.state.loaded && !hidden.contains(&entity) && frozen.is_none_or(|set| !set.contains(&entity))
            })
            .filter_map(|triangulation| {
                triangulation
                    .spatial
                    .ray_hit(&triangulation.mesh, ray_origin, ray_direction)
                    .map(|point| (triangulation.entity_id(), point))
            })
            .min_by(|(_, a), (_, b)| (*a - ray_origin).dot(ray_direction).total_cmp(&(*b - ray_origin).dot(ray_direction)))
    }

    /// Test a rendered document pick at its own screen position. Frozen
    /// surfaces still hide geometry, even though they cannot be selected; one
    /// the slab clips away is not drawn, so it hides nothing.
    pub(crate) fn surface_occludes_pick(
        triangulations: &[OpenTriangulation],
        hidden: &HashSet<SceneEntityId>,
        view_projection: &DMat4,
        scene_origin: DVec3,
        candidate: DVec3,
        slab: Option<SectionSlab>,
    ) -> bool {
        let Some((origin, direction)) = ray_through_world_point(view_projection, candidate) else {
            return false;
        };
        let Some(surface) = nearest_drawn_surface(triangulations, hidden, origin, direction, slab) else {
            return false;
        };
        // Pick vertices come from rebased f32 render buffers, whereas the BVH
        // retains f64 coordinates. Allow their rounding error so a line on the
        // surface remains selectable from either side after rotating the view.
        let rounding = (candidate - scene_origin).abs().max_element() * f64::from(f32::EPSILON);
        let surface_depth = (surface - origin).dot(direction);
        let tolerance = 1.0e-5_f64.max(rounding).max(surface_depth.abs() * 1.0e-9);
        (candidate - surface).dot(direction) > tolerance
    }

    /// Whether an opaque filled polyline is drawn in front of a candidate and
    /// hides it. A fill is the one document primitive that occludes; strokes
    /// and text are drawn over what they cross, so they never do.
    pub(crate) fn opaque_fill_occludes_pick(
        document: &Document,
        snap_index: &ObjectSnapIndex,
        hidden: &HashSet<SceneEntityId>,
        view_projection: &DMat4,
        candidate: DVec3,
        slab: Option<SectionSlab>,
    ) -> bool {
        let Some((origin, direction)) = ray_through_world_point(view_projection, candidate) else {
            return false;
        };
        // Only the nearest fill is known here, so a farther one inside the slab
        // is missed: that lets a pick through, the safe way to be wrong.
        let Some(fill) = nearest_opaque_document_fill(document, snap_index, hidden, origin, direction).filter(|point| slab.is_none_or(|slab| slab.contains(*point))) else {
            return false;
        };
        let fill_depth = (fill - origin).dot(direction);
        let tolerance = 1.0e-5_f64.max(fill_depth.abs() * 1.0e-9);
        (candidate - fill).dot(direction) > tolerance
    }

    /// Nearest selectable drill hole under a ray, named down to the hole
    /// itself - which dataset it belongs to is [`DrillHoleRef::dataset`].
    /// The hit geometry is the collar marker, the trace (string or cylinder,
    /// by style) and, for `StringAndDiscs`, the value discs. The trace is
    /// walked through [`crate::model::drill_hole::DrillHole::trace_pieces`],
    /// the pieces the cache draws, so a hidden stretch is never clickable.
    /// `discs` is the cache's record of each hole's drawn discs
    /// ([`crate::rendering::scene::DrillHoleGpuCache::disc_spans`]);
    /// [`hole_discs`] stands in where it has none yet.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn nearest_drill_hole(
        drill_holes: &[OpenDrillHoleDataset],
        hidden: &HashSet<SceneEntityId>,
        frozen: &HashSet<SceneEntityId>,
        ray_origin: DVec3,
        ray_direction: DVec3,
        view_direction: DVec3,
        view_projection: &DMat4,
        screen: Size,
        threshold_px: f32,
        discs: &DiscSpans,
    ) -> Option<(DrillHoleRef, DVec3)> {
        let mut nearest = f64::INFINITY;
        let mut nearest_hole = None;
        // The pixel floor depends on the view, not on the hole, so its basis
        // is worked out once here rather than inside the loop.
        let floor = PixelFloor::new(view_direction, view_projection, screen);
        // The string's world radius is 0, which the ray test always misses,
        // and without a PixelFloor nothing widens it: pick at a millimetre.
        const DEGENERATE_STRING_PICK_RADIUS: f64 = 1.0e-3;
        for dataset in drill_holes.iter().filter(|dataset| dataset.state.loaded) {
            let entity = dataset.entity_id();
            if hidden.contains(&entity) || frozen.contains(&entity) {
                continue;
            }
            // Per-style inputs that do not vary by hole, worked out once per
            // dataset so one loop below covers both styles.
            let true_diameter_floor_px = dataset.color.min_pixel_diameter;
            let disc_radius = dataset.color.disc_diameter * 0.5;
            let disc_floor_px = dataset.color.disc_min_pixel_diameter();
            let string_floor_px = dataset.color.string_pixel_width;
            // Same field gate the cache builds discs under: an unset, or
            // since-removed, field draws string only.
            let active_field = dataset.color.active_field.as_deref().filter(|key| dataset.dataset.field(key).is_some());

            for (index, hole) in dataset.dataset.holes.iter().enumerate() {
                let drilled_radius = hole.render_radius();
                let collar_source_radius = drilled_radius * COLLAR_MARKER_RADIUS_SCALE;
                // The trace's radius, fallback and floor, then what lifts the
                // collar clear: off the drilled radius for TrueDiameter, off
                // the disc for StringAndDiscs, as the marker sits over discs.
                let (trace_source_radius, trace_fallback_radius, trace_floor_px, lift_source_radius, lift_floor_px) = match dataset.color.hole_style {
                    DrillHoleStyle::TrueDiameter => {
                        let hole_radius = drilled_radius * dataset.color.radius_scale;
                        let drawn_radius = hole.diameter.map_or(0.0, |diameter| diameter * 0.5 * dataset.color.radius_scale);
                        (drawn_radius, hole_radius, true_diameter_floor_px, hole_radius, true_diameter_floor_px)
                    }
                    DrillHoleStyle::StringAndDiscs => (0.0, DEGENERATE_STRING_PICK_RADIUS, string_floor_px, disc_radius, disc_floor_px),
                };

                // Reaches as far as both tests it gates: the collar disc with
                // its 1.5x lift, and the walk off the lift radius and floor.
                // The string's floor is below the disc's and a disc never
                // lengthens past its floored diameter, so lengthened pieces
                // are covered too.
                let gate_reach = GateReach {
                    world_reach: lift_source_radius.max(collar_source_radius) + lift_source_radius * 1.5,
                    pixel_radius: f64::from(COLLAR_MARKER_MIN_PIXEL_DIAMETER).max(f64::from(lift_floor_px)) * 0.5 + 1.5 * f64::from(lift_floor_px) * 0.5,
                };
                if let Some(hole_box) = dataset.dataset.hole_box(index)
                    && !trace_box_hit(floor.as_ref(), hole_box, gate_reach, ray_origin, ray_direction, threshold_px, nearest)
                {
                    continue;
                }

                // The collar is a camera-facing disc substantially wider than
                // the trace. Test that visible marker explicitly; otherwise
                // only the narrow cylinder beneath it can ever be picked.
                // Keep the same small lift toward the camera as the shader so
                // depth ordering agrees with what is on screen.
                let collar = hole.collar_position();
                let rendered_lift_radius = floor
                    .as_ref()
                    .and_then(|floor| floor.floored_radius(collar, lift_source_radius, f64::from(lift_floor_px), 0.0))
                    .unwrap_or(lift_source_radius);
                let collar_radius = floor
                    .as_ref()
                    .and_then(|floor| floor.floored_radius(collar, collar_source_radius, f64::from(COLLAR_MARKER_MIN_PIXEL_DIAMETER), threshold_px))
                    .unwrap_or(collar_source_radius);
                let lifted_collar = collar - view_direction * rendered_lift_radius * 1.5;
                if let Some(distance) = ray_disc_distance(ray_origin, ray_direction, lifted_collar, view_direction, collar_radius)
                    && distance < nearest
                {
                    nearest = distance;
                    nearest_hole = Some(DrillHoleRef { dataset: dataset.id, hole: index });
                }

                // The trace, string and true diameter alike: the exact pieces
                // the cache draws, cut at render_ranges ends.
                if let Some((first, last)) = hole.trace.first().zip(hole.trace.last()) {
                    for piece in hole.trace_pieces(first.depth, last.depth, &[]) {
                        let radius = segment_pick_radius(
                            floor.as_ref(),
                            piece.start,
                            piece.end,
                            trace_source_radius,
                            trace_fallback_radius,
                            trace_floor_px,
                            threshold_px,
                        );
                        if let Some(distance) = ray_capped_cylinder_distance(ray_origin, ray_direction, piece.start, piece.end, radius)
                            && distance < nearest
                        {
                            nearest = distance;
                            nearest_hole = Some(DrillHoleRef { dataset: dataset.id, hole: index });
                        }
                    }
                }

                // Discs are the expensive part (hole_discs allocates), so
                // only holes the gate already passed reach here, and only
                // for StringAndDiscs with an active field.
                if !matches!(dataset.color.hole_style, DrillHoleStyle::StringAndDiscs) {
                    continue;
                }
                let Some(field) = active_field else { continue };
                // What the cache last drew, so a click matches the screen;
                // hole_discs stands in until the cache has built this hole.
                let cached = discs.get(dataset).and_then(|holes| holes.get(index));
                let disc_list: Cow<[HoleDisc]> = match cached {
                    Some(cached) => Cow::Borrowed(cached),
                    None => Cow::Owned(hole_discs(hole, field)),
                };
                for disc in disc_list.iter() {
                    let (Some(disc_start), Some(disc_end)) = (hole.position_at_depth(disc.from), hole.position_at_depth(disc.to)) else {
                        continue;
                    };
                    let Some(stretch) = disc_stretch(floor.as_ref(), disc_start, disc_end, disc_radius, disc_floor_px, DISC_MIN_PIXEL_LENGTH) else {
                        continue;
                    };
                    // Every piece endpoint moves by the same whole-disc
                    // stretch, so pieces stay joined and the disc grows
                    // symmetrically about its own middle rather than each
                    // piece growing about its own.
                    for piece in hole.trace_pieces(disc.from, disc.to, &[]) {
                        let start = stretch.apply(piece.start);
                        let end = stretch.apply(piece.end);
                        let Some(axis) = (end - start).try_normalize() else { continue };
                        let radius = ring_pixel_radius(floor.as_ref(), start, axis, disc_radius, disc_floor_px, threshold_px).max(ring_pixel_radius(
                            floor.as_ref(),
                            end,
                            axis,
                            disc_radius,
                            disc_floor_px,
                            threshold_px,
                        ));
                        if let Some(distance) = ray_capped_cylinder_distance(ray_origin, ray_direction, start, end, radius)
                            && distance < nearest
                        {
                            nearest = distance;
                            nearest_hole = Some(DrillHoleRef { dataset: dataset.id, hole: index });
                        }
                    }
                }
            }
        }
        nearest_hole.map(|hole| (hole, ray_origin + ray_direction * nearest))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn snap(
        document: &Document,
        snap_index: &ObjectSnapIndex,
        triangulations: &[OpenTriangulation],
        hidden: &HashSet<SceneEntityId>,
        frozen: &HashSet<SceneEntityId>,
        mode: &CursorMode,
        view_projection: &DMat4,
        screen: Size,
        cursor: (f32, f32),
        threshold: f32,
        xray_enabled: bool,
        slab: Option<SectionSlab>,
    ) -> Option<DVec3> {
        let candidate = snap::snap_cursor(document, snap_index, triangulations, hidden, frozen, mode, view_projection, screen, cursor, threshold, slab)?;
        if xray_enabled {
            return Some(candidate.world);
        }
        // Test visibility along the candidate's own screen-space ray. Using the
        // cursor ray here skews the accepted snap region on sloped surfaces:
        // the candidate may be several pixels from the cursor, so the cursor ray
        // can hit the same surface in front of an otherwise visible vertex.
        let (ray_origin, ray_direction) = ray_through_world_point(view_projection, candidate.world)?;
        let candidate_depth = (candidate.world - ray_origin).dot(ray_direction);
        // Surface snapping already found the nearest triangulation along this
        // ray. Other snap modes still need the triangulation visibility test.
        let surface_depth = (!matches!(mode, CursorMode::SnapToSurface))
            .then(|| nearest_drawn_surface(triangulations, hidden, ray_origin, ray_direction, slab))
            .flatten()
            .map(|point| (point - ray_origin).dot(ray_direction));
        // A fill the slab clips away hides nothing. Only the nearest fill is
        // known here, so a farther one inside the slab is missed: that lets a
        // snap through, which is the safe way to be wrong.
        let document_fill_depth = nearest_opaque_document_fill(document, snap_index, hidden, ray_origin, ray_direction)
            .filter(|point| slab.is_none_or(|slab| slab.contains(*point)))
            .map(|point| (point - ray_origin).dot(ray_direction));
        let occluder_depth = surface_depth.into_iter().chain(document_fill_depth).min_by(f64::total_cmp);
        // A triangulation must occlude its own back-side vertices too. The small
        // relative tolerance only absorbs ray/triangle floating-point noise at
        // the visible surface; it does not open a path through the mesh.
        if occluder_depth.is_some_and(|depth| {
            let tolerance = 1.0e-5_f64.max(depth.abs() * 1.0e-9);
            candidate_depth > depth + tolerance
        }) {
            None
        } else {
            Some(candidate.world)
        }
    }
}

struct PixelFloor {
    view_projection: DMat4,
    radial_clip: [glam::DVec4; 2],
    half_viewport: DVec2,
    radial_scale: Option<f64>,
}

impl PixelFloor {
    fn new(view_direction: DVec3, view_projection: &DMat4, screen: Size) -> Option<Self> {
        let view_direction = view_direction.try_normalize()?;
        let helper = if view_direction.z.abs() > 0.9 { DVec3::Y } else { DVec3::Z };
        let right = helper.cross(view_direction).try_normalize()?;
        let up = view_direction.cross(right);
        let half_viewport = DVec2::new(f64::from(screen.0), f64::from(screen.1)) * 0.5;
        if !half_viewport.is_finite() || half_viewport.min_element() <= 0.0 {
            return None;
        }
        let radial_clip = [*view_projection * right.extend(0.0), *view_projection * up.extend(0.0)];
        if !radial_clip.iter().all(|clip| clip.is_finite()) {
            return None;
        }
        // Only when both radials carry w == 0 is pixels_per_world an affine
        // function of the box (k / |w|); that is the case this scale serves.
        let radial_scale = radial_clip
            .iter()
            .all(|clip| clip.w.abs() <= 1.0e-12)
            .then(|| radial_clip.iter().map(|clip| (clip.truncate().truncate() * half_viewport).length()).fold(0.0_f64, f64::max))
            .filter(|k| k.is_finite() && *k > 0.0);
        Some(Self {
            view_projection: *view_projection,
            radial_clip,
            half_viewport,
            radial_scale,
        })
    }

    fn pixels_per_world(&self, point: DVec3) -> Option<f64> {
        let center_clip = self.view_projection * point.extend(1.0);
        if !center_clip.is_finite() {
            return None;
        }
        let safe_w = center_clip.w.abs().max(1.0e-6);
        let pixels = self
            .radial_clip
            .iter()
            .map(|radial_clip| {
                let ndc_per_world = (radial_clip.truncate().truncate() * center_clip.w - center_clip.truncate().truncate() * radial_clip.w) / (safe_w * safe_w);
                (ndc_per_world * self.half_viewport).length()
            })
            .fold(0.0_f64, f64::max);
        (pixels.is_finite() && pixels > 0.0).then_some(pixels)
    }

    fn floored_radius(&self, point: DVec3, source_radius: f64, minimum_pixel_diameter: f64, threshold_px: f32) -> Option<f64> {
        let pixels_per_world = self.pixels_per_world(point)?;
        let minimum_radius = (minimum_pixel_diameter * 0.5 + f64::from(threshold_px)) / pixels_per_world;
        minimum_radius.is_finite().then(|| source_radius.max(minimum_radius))
    }

    /// Screen pixels one world unit along `direction` covers at `point`, as
    /// the shader's `pixels_per_world` works it; unlike the view-basis
    /// `pixels_per_world` above, it takes a disc piece's own axis.
    fn axial_pixels_per_world(&self, point: DVec3, direction: DVec3) -> Option<f64> {
        let center_clip = self.view_projection * point.extend(1.0);
        if !center_clip.is_finite() {
            return None;
        }
        let safe_w = center_clip.w.abs().max(1.0e-6);
        let direction_clip = self.view_projection * direction.extend(0.0);
        if !direction_clip.is_finite() {
            return None;
        }
        let ndc_per_world = (direction_clip.truncate().truncate() * center_clip.w - center_clip.truncate().truncate() * direction_clip.w) / (safe_w * safe_w);
        let pixels = (ndc_per_world * self.half_viewport).length();
        (pixels.is_finite() && pixels > 0.0).then_some(pixels)
    }

    fn clip_w_span(&self, min: DVec3, max: DVec3) -> (f64, f64) {
        let w_row = self.view_projection.row(3);
        let row = w_row.truncate();
        let (low, high) = ((row * min), (row * max));
        let span_min = w_row.w + low.min(high).element_sum();
        let span_max = w_row.w + low.max(high).element_sum();
        (span_min, span_max)
    }
}

/// A string or true-diameter trace floors with the view's own fixed
/// cross-section basis (`PixelFloor::floored_radius`, the view-plane right
/// and up), not the per-segment ring frame below that a disc's own axis
/// needs: a trace is walked as one continuous cylinder, so the view's basis
/// alone is enough to keep it a constant width on screen.
fn segment_pick_radius(floor: Option<&PixelFloor>, start: DVec3, end: DVec3, source_radius: f64, fallback_radius: f64, floor_px: f32, threshold_px: f32) -> f64 {
    let Some(floor) = floor else {
        return fallback_radius;
    };
    // Neither end projects only on a degenerate view, where a diameterless
    // hole would otherwise be walked at nothing at all.
    let floored = [start, end]
        .into_iter()
        .filter_map(|point| floor.floored_radius(point, source_radius, f64::from(floor_px), threshold_px))
        .fold(f64::NEG_INFINITY, f64::max);
    if floored.is_finite() { floored } else { fallback_radius }
}

/// The middle, chord axis, and lengthening scale of a thin disc, taken from
/// its whole interval `start..end`: the same rule `drill_hole.wgsl` applies
/// to every piece of a disc from the `disc_start`/`disc_end` each piece
/// carries (same chord, same ring frame for the drawn radius, same
/// `wanted`/`longest` cap), so a disc cut into pieces at stations stretches
/// as one here as it does on screen. The shader works in f32 from the scene
/// origin, this in f64; nothing else differs. `apply` moves a point along
/// the axis only, so every piece stays joined and the disc grows
/// symmetrically about its own middle. `None` when `start` and `end`
/// coincide, which earns no chord to lengthen along. A scale of 1 is a
/// no-op `apply`: what a disc already long enough on screen, or with no
/// `PixelFloor` to floor against, gets.
fn disc_stretch(floor: Option<&PixelFloor>, start: DVec3, end: DVec3, disc_radius: f64, disc_floor_px: f32, min_pixel_length: f32) -> Option<DiscStretch> {
    let axis_vector = end - start;
    let length = axis_vector.length();
    if length <= 1.0e-12 {
        return None;
    }
    let axis = axis_vector / length;
    let middle = (start + end) * 0.5;
    let no_op = DiscStretch { middle, axis, scale: 1.0 };
    let Some(floor) = floor else { return Some(no_op) };
    let Some(axial_pixels_per_world) = floor.axial_pixels_per_world(middle, axis) else {
        return Some(no_op);
    };
    let span_px = length * axial_pixels_per_world;
    let min_px = f64::from(min_pixel_length);
    if span_px >= min_px {
        return Some(no_op);
    }
    let Some(ring_pixels_per_world) = ring_pixels_per_world(floor, middle, axis) else {
        return Some(no_op);
    };
    let drawn_radius = disc_radius.max(f64::from(disc_floor_px) * 0.5 / ring_pixels_per_world);
    let longest = length.max(2.0 * drawn_radius);
    let wanted = min_px / axial_pixels_per_world.max(1.0e-6);
    Some(DiscStretch {
        middle,
        axis,
        scale: wanted.min(longest) / length,
    })
}

struct DiscStretch {
    middle: DVec3,
    axis: DVec3,
    scale: f64,
}

impl DiscStretch {
    /// `point` moved along the chord only: unchanged perpendicular to
    /// `axis`, its along-axis offset from `middle` scaled by `scale`.
    fn apply(&self, point: DVec3) -> DVec3 {
        let offset = point - self.middle;
        self.middle + offset + self.axis * offset.dot(self.axis) * (self.scale - 1.0)
    }
}

/// The ring frame's own pixel scale at `center` for a ring whose axis is
/// `axis` - the shader's own `helper`/`right`/`up` construction, worked out
/// for an arbitrary axis rather than `PixelFloor`'s fixed view-plane basis
/// (see `axial_pixels_per_world`'s doc comment). `None` when neither
/// cross-section direction projects.
fn ring_pixels_per_world(floor: &PixelFloor, center: DVec3, axis: DVec3) -> Option<f64> {
    let helper = if axis.z.abs() > 0.9 { DVec3::Y } else { DVec3::Z };
    let right = helper.cross(axis).try_normalize()?;
    let up = axis.cross(right);
    let pixels_per_world = [right, up]
        .into_iter()
        .filter_map(|direction| floor.axial_pixels_per_world(center, direction))
        .fold(0.0_f64, f64::max);
    (pixels_per_world > 0.0).then_some(pixels_per_world)
}

/// The drawn radius of one ring of a (possibly stretched) disc piece at
/// `center`, in the ring frame of `axis`: the larger of the disc's own
/// radius and its floored diameter, plus the click tolerance at that ring's
/// own scale - the same shape `disc_stretch` uses for its own floor check.
/// Falls back to `disc_radius` alone with no `PixelFloor` or a degenerate
/// ring frame; the gate already treats those as "could be a hit", so this
/// only needs to not shrink what it is given.
fn ring_pixel_radius(floor: Option<&PixelFloor>, center: DVec3, axis: DVec3, disc_radius: f64, disc_floor_px: f32, threshold_px: f32) -> f64 {
    let Some(floor) = floor else { return disc_radius };
    let Some(pixels_per_world) = ring_pixels_per_world(floor, center, axis) else {
        return disc_radius;
    };
    disc_radius.max(f64::from(disc_floor_px) * 0.5 / pixels_per_world) + f64::from(threshold_px) / pixels_per_world
}

struct GateReach {
    world_reach: f64,
    pixel_radius: f64,
}

/// Whether a hole's box could still hold the nearest pick along this ray, so
/// its stations are worth walking. `nearest` is the best distance found so
/// far this query; a box entered no closer than that can never win, so it is
/// skipped exactly as the walk it stands in for would be.
///
/// The gate is allowed to be generous and say yes to a hole the walk then
/// rejects. It is never allowed to say no to a hole the walk would have hit,
/// so every way of not knowing below answers yes and lets the walk decide.
fn trace_box_hit(
    floor: Option<&PixelFloor>,
    hole_box: crate::model::drill_hole::WorldBox,
    reach: GateReach,
    ray_origin: DVec3,
    ray_direction: DVec3,
    threshold_px: f32,
    nearest: f64,
) -> bool {
    let (min, max) = hole_box;
    if !min.is_finite() || !max.is_finite() || !ray_origin.is_finite() || !ray_direction.is_finite() || !reach.world_reach.is_finite() {
        return true;
    }
    let Some(floor) = floor else {
        return true;
    };
    let (w_min, w_max) = floor.clip_w_span(min, max);
    if !(w_min > 1.0e-6 || w_max < -1.0e-6) {
        return true;
    }
    let Some(k) = floor.radial_scale else {
        return true;
    };
    // w is affine over the box, so the farthest-from-zero corner bounds the
    // floor everywhere in it; add that pixel floor to the world reach.
    let w_abs = w_min.abs().max(w_max.abs());
    let inflation = reach.world_reach + (reach.pixel_radius + f64::from(threshold_px)) * w_abs / k;
    if !inflation.is_finite() {
        return true;
    }
    let pad = DVec3::splat(inflation);
    ray_aabb_distance(ray_origin, ray_direction, min - pad, max + pad).is_some_and(|distance| distance < nearest)
}

/// Distance along a normalized ray to a finite cylinder, including its flat
/// end caps. Returns the first forward intersection.
fn ray_capped_cylinder_distance(origin: DVec3, direction: DVec3, start: DVec3, end: DVec3, radius: f64) -> Option<f64> {
    let axis_vector = end - start;
    let length = axis_vector.length();
    if length <= 1.0e-12 || radius <= 0.0 {
        return None;
    }
    let axis = axis_vector / length;
    let offset = origin - start;
    let direction_axial = direction.dot(axis);
    let offset_axial = offset.dot(axis);
    let direction_radial = direction - axis * direction_axial;
    let offset_radial = offset - axis * offset_axial;
    let a = direction_radial.length_squared();
    let b = 2.0 * direction_radial.dot(offset_radial);
    let c = offset_radial.length_squared() - radius * radius;
    let mut nearest = f64::INFINITY;

    if a > 1.0e-18 {
        let discriminant = b * b - 4.0 * a * c;
        if discriminant >= 0.0 {
            let root = discriminant.sqrt();
            for distance in [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)] {
                let axial = offset_axial + distance * direction_axial;
                if distance >= 0.0 && axial >= 0.0 && axial <= length {
                    nearest = nearest.min(distance);
                }
            }
        }
    }

    if direction_axial.abs() > 1.0e-18 {
        for cap_axial in [0.0, length] {
            let distance = (cap_axial - offset_axial) / direction_axial;
            let radial = offset_radial + direction_radial * distance;
            if distance >= 0.0 && radial.length_squared() <= radius * radius {
                nearest = nearest.min(distance);
            }
        }
    }

    nearest.is_finite().then_some(nearest)
}

/// Distance along a normalized ray to a camera-facing disc.
fn ray_disc_distance(origin: DVec3, direction: DVec3, center: DVec3, normal: DVec3, radius: f64) -> Option<f64> {
    let normal = normal.try_normalize()?;
    let denominator = direction.dot(normal);
    if denominator.abs() <= 1.0e-18 || radius <= 0.0 {
        return None;
    }
    let distance = (center - origin).dot(normal) / denominator;
    if distance < 0.0 {
        return None;
    }
    let hit = origin + direction * distance;
    (hit.distance_squared(center) <= radius * radius).then_some(distance)
}

/// Nearest surface the view actually draws along a ray. Inside a section the
/// slab discards every fragment outside it, so a surface it clips away is not
/// there to hide a snap target behind it.
fn nearest_drawn_surface(
    triangulations: &[OpenTriangulation],
    hidden: &HashSet<SceneEntityId>,
    ray_origin: DVec3,
    ray_direction: DVec3,
    slab: Option<SectionSlab>,
) -> Option<DVec3> {
    triangulations
        .iter()
        .filter(|triangulation| triangulation.state.loaded && !hidden.contains(&triangulation.entity_id()))
        .filter_map(|triangulation| {
            triangulation
                .spatial
                .ray_hit_details_where(&triangulation.mesh, ray_origin, ray_direction, |point| slab.is_none_or(|slab| slab.contains(point)))
                .map(|hit| hit.point)
        })
        .min_by(|a, b| (*a - ray_origin).dot(ray_direction).total_cmp(&(*b - ray_origin).dot(ray_direction)))
}

fn nearest_opaque_document_fill(document: &Document, snap_index: &ObjectSnapIndex, hidden: &HashSet<SceneEntityId>, ray_origin: DVec3, ray_direction: DVec3) -> Option<DVec3> {
    snap_index.nearest_filled_polyline_hit(ray_origin, ray_direction, |object_index| {
        let Some(object) = document.objects().get(object_index) else {
            return false;
        };
        let entity = SceneEntityId::Object(object.id());
        !hidden.contains(&entity) && document.layer(object.layer()).is_none_or(|layer| layer.loaded) && document.object_fill_rgba(object)[3] >= 1.0 - f32::EPSILON
    })
}

pub(crate) fn ray_through_world_point(view_projection: &DMat4, point: DVec3) -> Option<(DVec3, DVec3)> {
    let clip = *view_projection * point.extend(1.0);
    if clip.w.abs() <= f64::EPSILON {
        return None;
    }

    let ndc = clip.truncate() / clip.w;
    let inverse = view_projection.inverse();
    let near_h = inverse * DVec3::new(ndc.x, ndc.y, 1.0).extend(1.0);
    let far_h = inverse * DVec3::new(ndc.x, ndc.y, 0.0).extend(1.0);
    if near_h.w.abs() <= f64::EPSILON || far_h.w.abs() <= f64::EPSILON {
        return None;
    }

    let near = near_h.truncate() / near_h.w;
    let far = far_h.truncate() / far_h.w;
    let direction = (far - near).try_normalize()?;
    Some((near, direction))
}
