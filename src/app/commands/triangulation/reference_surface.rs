//! A reference surface: the selected points triangulated in plan and cut
//! back to an optional extent. The Delaunay is over every selected point,
//! inside the extent or outside it, so distant data still shapes the trend;
//! only the finished surface is clipped to the boundary.

use glam::{DVec2, DVec3};
use spade::handles::FixedVertexHandle;

use super::*;
use crate::model::{
    Document, LayerId,
    kernel::{self, PolyContainment, SegSeg},
};

/// A point with its elevation carried through spade, which works in XY.
#[derive(Clone, Copy)]
struct SurfaceVertex {
    position: spade::Point2<f64>,
    z: f64,
}

impl spade::HasPosition for SurfaceVertex {
    type Scalar = f64;

    fn position(&self) -> spade::Point2<Self::Scalar> {
        self.position
    }
}

/// The picks in plan. Constrained because the extent's edges are forced into
/// the mesh; with no extent there is simply nothing to constrain.
type SurfaceTriangulation = spade::ConstrainedDelaunayTriangulation<SurfaceVertex>;

/// The fewest points a surface can be built from.
pub(crate) const MINIMUM_POINTS: usize = 3;

/// The fewest vertices a control string is usable from: two make a segment,
/// and a segment is what the mesh is creased along.
const MINIMUM_CONTROL_VERTICES: usize = 2;

/// How many overridden picks the report names one by one before it counts the
/// rest, so a build over a big string set cannot flood the console.
const OVERRIDE_LINES: usize = 20;

/// How far apart two controls' elevations may be where they cross in plan and
/// still count as one height; two interpretations meeting, so tighter than
/// [`kernel::Z_TOL`].
const CONTROL_AGREEMENT: f64 = 0.01;

/// The share of the picks' squared plan span the largest triangle must
/// cover before the picks count as spread out rather than lined up. Relative
/// because on a mine grid the rounding slivers between collinear picks
/// carry more area than any small fixed epsilon.
const DEGENERATE_AREA_FRACTION: f64 = 1e-9;

/// The step the vertical extent is rounded out to, so the box reads as a
/// round number instead of an accident of where the picks happened to fall.
const EXTENT_ROUNDING: f64 = 10.0;

/// One build's geometry and the numbers the report is made of, kept apart
/// from the log line so they can be read back.
struct SurfaceMesh {
    vertices: Vec<mesh_data::Vertex>,
    faces: Vec<[u32; 3]>,
    /// Points that shared a plan position with an earlier one.
    coincident: usize,
    /// Points the Delaunay was built from, support included.
    triangulated: usize,
    /// Of those, the ones outside the extent: trend, not surface.
    support: usize,
    /// The vertical box the surface occupies, low and high.
    vertical_box: (f64, f64),
    /// Control strings the surface was made to pass through.
    controls: usize,
    /// Vertices those strings own, their own and the ones a crossing or an
    /// overridden pick added along them.
    control_vertices: usize,
    /// Plan positions where controls met each other, each now one vertex.
    crossings: usize,
    /// Picks a control took over: the plan position, the pick's own
    /// elevation, and the control's.
    overridden: Vec<(DVec2, f64, f64)>,
}

/// What a surface build takes from a selection: the points to triangulate,
/// the layers they came from, the open strings the surface is made to pass
/// through, and the one closed string clipping the result.
pub(crate) struct SurfaceInput {
    pub(crate) points: Vec<ObjectId>,
    /// Distinct layers the points sit on, in document order. One layer is the
    /// ordinary case; more than one means the surface has no single source.
    pub(crate) layers: Vec<LayerId>,
    /// Open strings, in document order. Each is a breakline the finished
    /// surface runs along, at the string's own elevations.
    pub(crate) controls: Vec<ObjectId>,
    pub(crate) extent: Option<ObjectId>,
}

/// Read a surface build's inputs out of `selected`, or say why there are none.
///
/// Walked over `document.objects()` rather than `selected` itself, so the
/// points and the layer list come back in document order rather than
/// whatever order the selection's `HashSet` happens to iterate in.
pub(crate) fn surface_input(document: &Document, selected: &HashSet<SceneEntityId>) -> Result<SurfaceInput> {
    let mut points = Vec::new();
    let mut layers = Vec::new();
    let mut controls = Vec::new();
    let mut extents = Vec::new();
    for object in document.objects() {
        if !selected.contains(&SceneEntityId::Object(object.id())) {
            continue;
        }
        match object {
            Object::Point { id, layer, .. } => {
                points.push(*id);
                if !layers.contains(layer) {
                    layers.push(*layer);
                }
            }
            // Shape tells the two kinds of string apart: an open one is a
            // control the surface passes through, a closed one bounds the
            // ground the surface covers.
            Object::Polyline { id, closed: false, .. } => controls.push(*id),
            // `extent_ring` below accepts a closed polyline and nothing else,
            // so that is all that is offered as an extent here - not
            // everything `Object::encloses_area()` would admit.
            Object::Polyline { id, closed: true, .. } => extents.push(*id),
            _ => {}
        }
    }
    if points.len() < MINIMUM_POINTS {
        anyhow::bail!("{}", too_few_points(points.len()));
    }
    let extent = match extents.len() {
        0 => None,
        1 => Some(extents[0]),
        _ => anyhow::bail!("{}", tr!(literal = "Select exactly one closed string to clip the surface to")),
    };
    Ok(SurfaceInput { points, layers, controls, extent })
}

impl<'a> App<'a> {
    /// Triangulate the selected points into a new surface, named for their
    /// layer when they share one. Every run adds a surface; nothing is
    /// replaced.
    pub(crate) fn build_reference_surface(&mut self, points: Vec<ObjectId>, controls: Vec<ObjectId>, extent: Option<ObjectId>) -> Result<()> {
        let project = self
            .workspace
            .active_project()
            .ok_or_else(|| anyhow::anyhow!("{}", tr!(literal = "Open a project before building a surface")))?;
        let project_key = crate::app::jobs::JobKey::Project {
            runtime_id: project.runtime_id,
            document_revision: project.project.document.revision(),
        };
        // Points, controls and extent all come from the scene document, which
        // is what the selection was read against: a string the geologist could
        // see and choose is the string that clips.
        let ring = extent.map(|id| extent_ring(&self.scene_document, id)).transpose()?;
        let controls = control_strings(&self.scene_document, &controls)?;
        // Snapshot the geometry and its source layers on the UI thread; the
        // worker never sees the scene document. A point whose layer was
        // hidden since the dialog opened no longer resolves and is dropped,
        // not refused.
        let mut layers: Vec<LayerId> = Vec::new();
        let points: Vec<DVec3> = points
            .into_iter()
            .filter_map(|id| match self.scene_document.get_object(id) {
                Some(Object::Point { layer, pos, .. }) => {
                    if !layers.contains(layer) {
                        layers.push(*layer);
                    }
                    Some(*pos)
                }
                _ => None,
            })
            .collect();
        // Not counted again here: `surface_mesh` owns that contract, and a
        // selection thinned by a layer going hidden is reported by the build
        // it fails rather than by a second count beside it.
        // Born beside the points' own layer when they agree on a section that
        // can show a surface, at the natural section otherwise - one rule for
        // every source, not a case carved out for any one section.
        let sections = layers.iter().filter_map(|id| self.scene_document.layer(*id)).map(|layer| layer.section);
        let section = SectionKind::derived_for(MemberKind::Triangulation, sections);
        let name = match layers.len() {
            1 => self
                .scene_document
                .layer(layers[0])
                .map(|layer| layer.name.clone())
                .unwrap_or_else(|| tr!(literal = "Surface")),
            _ => tr!(literal = "Surface"),
        };
        // Said rather than guessed at: layers that disagree on a section send
        // the surface to its natural one, but layers that agree still keep it
        // beside them, so the line names where it actually went.
        if layers.len() > 1 {
            userspace_log!(
                "{}",
                tr_format!(
                    literal = "The selected points span %count% layers; the surface is placed under %section%",
                    count = layers.len(),
                    section = crate::ui::state::ExplorerSection::from_kind(section).label()
                )
            );
        }
        // Said once, not guessed at: an unclipped surface runs to the hull of
        // whatever was selected, which is rarely the ground the geologist meant.
        if ring.is_none() {
            userspace_warn!("{}", tr!(literal = "No mask selected; the surface is unclipped"));
        }

        let compute = move |cancel: &crate::app::jobs::CancelFlag| -> Result<crate::model::triangulation::GeneratedTriangulation> {
            if cancel.is_cancelled() {
                anyhow::bail!("{}", tr!(literal = "Cancelled"));
            }
            delaunay_surface_from_points(points, controls, ring, name, &|| cancel.is_cancelled())
        };
        let apply = move |app: &mut App, result: Result<crate::model::triangulation::GeneratedTriangulation>| match result {
            Ok(generated) => app.insert_generated_triangulation_in(generated, section),
            Err(error) => crate::userspace_error!("{}", tr_format!(literal = "Build Surface failed: %error%", error = format!("{error:#}"))),
        };
        self.spawn_job(tr!(literal = "Building surface…"), vec![project_key], compute, apply);
        Ok(())
    }
}

fn too_few_points(count: usize) -> String {
    tr_format!(
        literal = "%count% point(s) selected; a surface needs at least %minimum%",
        count = count,
        minimum = MINIMUM_POINTS
    )
}

/// Spade refusing a vertex is not a geologist's mistake, so the reason is
/// carried through rather than summarised.
fn insert_failed(error: impl std::fmt::Debug) -> String {
    tr_format!(literal = "Delaunay insert failed: %error%", error = format!("{error:?}"))
}

/// Control strings carry no name of their own, so a refusal names one by
/// where it sat in the selection, counting from one.
fn too_few_control_vertices(index: usize, count: usize) -> String {
    tr_format!(
        literal = "Control string %index% has %count% distinct vertex(es); a control needs at least %minimum%",
        index = index + 1,
        count = count,
        minimum = MINIMUM_CONTROL_VERTICES
    )
}

fn control_not_finite(index: usize) -> String {
    tr_format!(literal = "Control string %index% has non-finite coordinates", index = index + 1)
}

fn control_not_available(index: usize) -> String {
    tr_format!(literal = "Control string %index% is no longer available", index = index + 1)
}

fn control_self_crossing(index: usize) -> String {
    tr_format!(literal = "Control string %index% crosses itself in plan", index = index + 1)
}

fn control_doubles_back(index: usize) -> String {
    tr_format!(literal = "Control string %index% doubles back on itself in plan", index = index + 1)
}

fn control_ends_where_it_starts(index: usize) -> String {
    tr_format!(literal = "Control string %index% ends where it starts; close it to use it as a mask", index = index + 1)
}

/// Stop the build once the job it runs in has been cancelled.
fn stop_if_cancelled(cancelled: &dyn Fn() -> bool) -> Result<()> {
    if cancelled() {
        anyhow::bail!("{}", tr!(literal = "Cancelled"));
    }
    Ok(())
}

/// Two controls reading one plan position at two heights: which two, where,
/// and how far apart, for the geologist to decide between. The string
/// selected first is named first, each height beside its own string.
fn controls_disagree(left: usize, right: usize, position: DVec2, low: f64, high: f64) -> String {
    let (left, right, low, high) = if left <= right { (left, right, low, high) } else { (right, left, high, low) };
    tr_format!(
        literal = "Control strings %a% and %b% disagree at (%x%, %y%): %za% m against %zb% m, %difference% m apart",
        a = left + 1,
        b = right + 1,
        x = format!("{:.3}", position.x),
        y = format!("{:.3}", position.y),
        za = format!("{low:.2}"),
        zb = format!("{high:.2}"),
        difference = format!("{:.2}", (high - low).abs())
    )
}

/// Two controls sharing a stretch of plan rather than a point: every position
/// along it is claimed twice, which is a job of its own.
fn controls_along_each_other(left: usize, right: usize) -> String {
    let (left, right) = (left.min(right), left.max(right));
    tr_format!(
        literal = "Control strings %a% and %b% run along each other in plan; that is not supported yet",
        a = left + 1,
        b = right + 1
    )
}

/// A control running along the extent's edge rather than across it leaves the
/// clip with no side to keep the string on.
fn control_along_extent(index: usize) -> String {
    tr_format!(
        literal = "Control string %index% runs along the extent string in plan; that is not supported yet",
        index = index + 1
    )
}

/// Spade refusing a control's constraint, once every nameable shape has
/// already been refused by name.
fn control_not_added(index: usize) -> String {
    tr_format!(literal = "Control string %index% could not be added to the mesh", index = index + 1)
}

fn too_few_points_inside(count: usize) -> String {
    tr_format!(
        literal = "%count% point(s) inside the extent; a surface needs at least %minimum%",
        count = count,
        minimum = MINIMUM_POINTS
    )
}

/// The chosen extent as a plan ring. Only the plan shape travels: the ring's
/// elevations come from the surface, not from the height the string happens
/// to have been drawn at. Arcs are expanded because the clip works on
/// straight segments.
fn extent_ring(document: &crate::model::Document, id: ObjectId) -> Result<Vec<DVec2>> {
    let object = document
        .get_object(id)
        .ok_or_else(|| anyhow::anyhow!("{}", tr!(literal = "The extent string is no longer available")))?;
    let Object::Polyline { verts, closed: true, .. } = object else {
        anyhow::bail!("{}", tr!(literal = "The extent must be a closed string"));
    };
    let mut ring: Vec<DVec2> = crate::model::geometry::tessellate_polyline_bulges(verts, true)
        .iter()
        .map(|vertex| vertex.truncate())
        .collect();
    if ring.iter().any(|vertex| !vertex.is_finite()) {
        anyhow::bail!("{}", tr!(literal = "The extent string has non-finite coordinates"));
    }
    ring.dedup();
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    if ring.len() < MINIMUM_POINTS {
        anyhow::bail!("{}", tr!(literal = "The extent string needs at least three distinct vertices"));
    }
    Ok(ring)
}

/// The chosen control strings as 3D lines, in selection order, arcs
/// expanded into straight segments. A string whose layer went hidden since
/// the dialog opened is refused by its place in `ids`, like the extent, so
/// every later string keeps the number the dialog gave it.
fn control_strings(document: &crate::model::Document, ids: &[ObjectId]) -> Result<Vec<Vec<DVec3>>> {
    let mut strings: Vec<Vec<DVec3>> = Vec::new();
    for (index, id) in ids.iter().enumerate() {
        let Some(Object::Polyline { verts, closed: false, .. }) = document.get_object(*id) else {
            anyhow::bail!("{}", control_not_available(index));
        };
        let mut string = crate::model::geometry::tessellate_polyline_bulges(verts, false);
        if string.iter().any(|vertex| !vertex.is_finite()) {
            anyhow::bail!("{}", control_not_finite(index));
        }
        string.dedup();
        if string.len() < MINIMUM_CONTROL_VERTICES {
            anyhow::bail!("{}", too_few_control_vertices(index, string.len()));
        }
        strings.push(string);
    }
    Ok(strings)
}

/// Worker half: a Delaunay in plan over the points, elevations carried on the
/// vertices, creased along the controls and clipped to the extent when there
/// is one.
fn delaunay_surface_from_points(
    points: Vec<DVec3>,
    controls: Vec<Vec<DVec3>>,
    extent: Option<Vec<DVec2>>,
    name: String,
    cancelled: &dyn Fn() -> bool,
) -> Result<crate::model::triangulation::GeneratedTriangulation> {
    let surface = surface_mesh(&points, &controls, extent.as_deref(), cancelled)?;
    userspace_log!(
        "{}",
        tr_format!(
            literal = "Built surface %name% from %vertex_count% point(s) into %face_count% face(s), box z %low% to %high%%support%%coincident%%controls%",
            name = name,
            vertex_count = surface.triangulated,
            face_count = surface.faces.len(),
            low = format!("{:.1}", surface.vertical_box.0),
            high = format!("{:.1}", surface.vertical_box.1),
            support = if surface.support == 0 {
                String::new()
            } else {
                tr_format!(literal = "; %count% point(s) outside the extent shaped it as support", count = surface.support)
            },
            coincident = if surface.coincident == 0 {
                String::new()
            } else {
                tr_format!(literal = "; %count% point(s) shared a plan position and were kept once", count = surface.coincident)
            },
            controls = if surface.controls == 0 {
                String::new()
            } else {
                tr_format!(
                    literal = "; %count% control string(s) with %vertices% vertex(es)%crossings%",
                    count = surface.controls,
                    vertices = surface.control_vertices,
                    crossings = if surface.crossings == 0 {
                        String::new()
                    } else {
                        tr_format!(literal = " meeting at %count% crossing(s)", count = surface.crossings)
                    }
                )
            }
        )
    );
    // Each pick a control took over is named, for the geologist to explain,
    // in one message so the worker takes the console once.
    if !surface.overridden.is_empty() {
        let mut report = surface
            .overridden
            .iter()
            .take(OVERRIDE_LINES)
            .map(|(position, pick, control)| {
                tr_format!(
                    literal = "Control string overrides the pick at (%x%, %y%): pick %pick% m, control %control% m, difference %difference% m",
                    x = format!("{:.3}", position.x),
                    y = format!("{:.3}", position.y),
                    pick = format!("{pick:.2}"),
                    control = format!("{control:.2}"),
                    difference = format!("{:+.2}", control - pick)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        if surface.overridden.len() > OVERRIDE_LINES {
            report.push_str(&tr_format!(literal = ", … and %more% more", more = surface.overridden.len() - OVERRIDE_LINES));
        }
        userspace_warn!("{}", report);
    }
    session::build_generated_triangulation(name, surface.vertices, surface.faces, TriSurfaceType::Surface, crate::model::triangulation::unique_edges)
}

/// Triangulate the points in plan and, given an extent, cut the result back
/// to it. Two points at one plan position keep one vertex, so a twin hole
/// never fails the build; the count is reported.
///
/// Every control vertex is a vertex of the mesh and every control segment an
/// edge of it, so the surface honours the whole string; where a control
/// meets a pick, the control's elevation is kept.
fn surface_mesh(points: &[DVec3], controls: &[Vec<DVec3>], extent: Option<&[DVec2]>, cancelled: &dyn Fn() -> bool) -> Result<SurfaceMesh> {
    use spade::{FloatTriangulation as _, Triangulation as _};

    if points.len() < MINIMUM_POINTS {
        anyhow::bail!("{}", too_few_points(points.len()));
    }
    // Both read before any vertex goes in, so a refusal names the shape and
    // builds nothing.
    // Controls in an order read off their own geometry, so the build cannot
    // depend on the order they were selected in; `names` keeps each one's
    // place in the selection for the messages.
    let names = canonical_order(controls);
    let ordered: Vec<Vec<DVec3>> = names.iter().map(|&index| controls[index].clone()).collect();
    let controls = ordered.as_slice();
    validate_controls(controls, &names, cancelled)?;
    let crossings = control_crossings(controls, &names, cancelled)?;
    let mut tin = SurfaceTriangulation::new();
    let mut coincident = 0usize;
    for point in points {
        if !point.is_finite() {
            anyhow::bail!("{}", tr!(literal = "A selected point has non-finite coordinates"));
        }
        let before = tin.num_vertices();
        tin.insert(SurfaceVertex {
            position: spade::Point2::new(point.x, point.y),
            z: point.z,
        })
        .map_err(|error| anyhow::anyhow!("{}", insert_failed(error)))?;
        if tin.num_vertices() == before {
            coincident += 1;
        }
    }
    if tin.num_vertices() < MINIMUM_POINTS {
        anyhow::bail!("{}", too_few_points(tin.num_vertices()));
    }
    // Judged on the picks alone, before any extent vertex joins them: a line
    // of picks propped up by the corners of a box is still a line of picks,
    // and the surface it would make is interpolated from nothing.
    if !has_plan_area(&tin) {
        anyhow::bail!("{}", tr!(literal = "The points are collinear in plan; a surface needs three that are not"));
    }
    let triangulated = tin.num_vertices();
    let mut support = 0usize;
    let bands = extent.map(RingBands::new);

    if let Some(ring) = extent {
        // A ring that crosses or touches itself bounds no single area, so
        // there is no answer to what the clip should keep. Judged on the
        // ring's own geometry: whether an extent is usable cannot depend on
        // where the picks happen to sit.
        if self_intersects(ring, true) {
            anyhow::bail!("{}", tr!(literal = "The extent string crosses itself in plan"));
        }
        // A pick surveyed onto the string belongs to the ground the string
        // bounds, so the boundary counts as inside here: the kernel names
        // that case instead of leaving it to which way the arithmetic fell.
        support = tin
            .vertices()
            .filter(|vertex| {
                !bands
                    .as_ref()
                    .is_some_and(|bands| matches!(bands.contains(plan(vertex.position())), PolyContainment::Inside | PolyContainment::OnBoundary))
            })
            .count();
        let inside = triangulated - support;
        if inside < MINIMUM_POINTS {
            anyhow::bail!("{}", too_few_points_inside(inside));
        }
    }

    // Every control vertex goes in at its own x, y and z, a pick sharing its
    // plan position giving way to it.
    let mut owned = Owners::default();
    let mut overridden: Vec<(DVec2, f64, f64)> = Vec::new();
    let mut grid = VertexGrid::new(&tin, controls);
    let mut chains: Vec<Vec<FixedVertexHandle>> = Vec::with_capacity(controls.len());
    for (index, control) in controls.iter().enumerate() {
        stop_if_cancelled(cancelled)?;
        let mut chain = Vec::with_capacity(control.len());
        for vertex in control {
            chain.push(insert_control_vertex(&mut tin, &mut grid, &mut owned, &mut overridden, names[index], *vertex)?);
        }
        chains.push(chain);
    }
    // Where each control segment and ring edge must be split, collected
    // first so every constraint is added in one pass.
    let mut control_splits: Vec<Vec<Vec<(f64, FixedVertexHandle)>>> = controls.iter().map(|control| vec![Vec::new(); control.len() - 1]).collect();
    let ring_length = extent.map_or(0, |ring| ring.len());
    let mut ring_splits: Vec<Vec<(f64, FixedVertexHandle)>> = vec![Vec::new(); ring_length];
    // A ring vertex a control put into the mesh itself: it keeps the
    // control's elevation instead of taking one from the trend.
    let mut ring_pinned: Vec<Option<FixedVertexHandle>> = vec![None; ring_length];

    // One vertex where controls meet, every one of them constrained through
    // it: a control's own vertex there when it has one, else a new one that
    // a pick beneath gives way to.
    for crossing in &crossings {
        let owns = crossing.sides.iter().find_map(|side| side.vertex.map(|vertex| chains[side.control][vertex]));
        let handle = match owns {
            Some(handle) => handle,
            None => insert_control_vertex(
                &mut tin,
                &mut grid,
                &mut owned,
                &mut overridden,
                names[crossing.sides[0].control],
                crossing.position.extend(crossing.z),
            )?,
        };
        for side in &crossing.sides {
            owned.add(handle.index(), names[side.control]);
            if side.vertex.is_none() {
                control_splits[side.control][side.segment].push((side.along, handle));
            }
        }
    }

    if let Some(ring) = extent {
        let edges = BoxGrid::new((0..ring.len()).map(|edge| segment_box(ring[edge], ring[(edge + 1) % ring.len()])).collect());
        let mut near = Vec::new();
        for (index, control) in controls.iter().enumerate() {
            stop_if_cancelled(cancelled)?;
            for segment in 0..control.len() - 1 {
                let (start, end) = (control[segment], control[segment + 1]);
                let (low, high) = segment_box(start.truncate(), end.truncate());
                edges.overlapping(low, high, &mut near);
                for &edge in &near {
                    let (corner, next) = (ring[edge], ring[(edge + 1) % ring.len()]);
                    let (point, along, across) = match kernel::segment_segment(start.truncate(), end.truncate(), corner, next) {
                        SegSeg::Disjoint => continue,
                        SegSeg::CollinearOverlap { .. } => anyhow::bail!("{}", control_along_extent(names[index])),
                        SegSeg::Crossing { point, t, u } | SegSeg::Touching { point, t, u } => (point, t, u),
                    };
                    let at_control_vertex = nearer_end(point, start.truncate(), end.truncate()).map(|end| chains[index][segment + end]);
                    let at_ring_vertex = nearer_end(point, corner, next).map(|end| (edge + end) % ring.len());
                    match (at_control_vertex, at_ring_vertex) {
                        // One plan position for both, so the ring takes the
                        // control's vertex, elevation and all.
                        (Some(handle), Some(pinned)) => ring_pinned[pinned] = Some(handle),
                        // A control vertex sitting along a ring edge: the
                        // ring is constrained through it.
                        (Some(handle), None) => ring_splits[edge].push((across, handle)),
                        // A ring vertex sitting along a control: it goes in
                        // at the control's elevation there, and both the
                        // control and the ring run through it.
                        (None, Some(pinned)) => {
                            let vertex = ring[pinned].extend(elevation_along(start, end, along));
                            let handle = insert_control_vertex(&mut tin, &mut grid, &mut owned, &mut overridden, names[index], vertex)?;
                            ring_pinned[pinned] = Some(handle);
                            control_splits[index][segment].push((along, handle));
                        }
                        // A crossing proper: one vertex at the elevation the
                        // control has there, kept whole as support while the
                        // clip runs through it.
                        (None, None) => {
                            let vertex = point.extend(elevation_along(start, end, along));
                            let handle = insert_control_vertex(&mut tin, &mut grid, &mut owned, &mut overridden, names[index], vertex)?;
                            control_splits[index][segment].push((along, handle));
                            ring_splits[edge].push((across, handle));
                        }
                    }
                }
            }
        }
    }
    // A pick under a control segment takes the elevation the segment has
    // above it, so the crease runs along the whole string rather than
    // stopping at the picks it passes over. Another control's vertex there
    // is shared when the two agree on its height, whichever came first.
    for (index, control) in controls.iter().enumerate() {
        stop_if_cancelled(cancelled)?;
        for segment in 0..control.len() - 1 {
            let (start, end) = (control[segment], control[segment + 1]);
            let ends = [chains[index][segment], chains[index][segment + 1]];
            for (along, handle) in vertices_along(&grid, start.truncate(), end.truncate()) {
                if ends.contains(&handle) || owned.has(handle.index(), names[index]) || control_splits[index][segment].iter().any(|(_, split)| *split == handle) {
                    continue;
                }
                take_over_vertex(&mut tin, &mut owned, &mut overridden, names[index], handle, elevation_along(start, end, along))?;
                control_splits[index][segment].push((along, handle));
            }
        }
    }
    let control_vertices = owned.len();
    for (index, chain) in chains.iter().enumerate() {
        stop_if_cancelled(cancelled)?;
        for segment in 0..chain.len() - 1 {
            let through = std::mem::take(&mut control_splits[index][segment]);
            constrain_chain(&mut tin, chain[segment], through, chain[segment + 1], || control_not_added(names[index]))?;
        }
    }

    if let Some(ring) = extent {
        // The ring's own vertices are given the elevation the surface has at
        // that plan position, so the clipped edge lies on the surface rather
        // than at the height the string was drawn at. Read with the controls
        // already in, so the edge lands on the surface they shaped.
        let ring_vertices: Vec<SurfaceVertex> = {
            let trend = tin.barycentric();
            ring.iter()
                .map(|vertex| {
                    let position = spade::Point2::new(vertex.x, vertex.y);
                    let z = trend
                        .interpolate(|vertex| vertex.data().z, position)
                        .or_else(|| nearest_pick_z(&tin, &grid, position))
                        .unwrap_or_default();
                    SurfaceVertex { position, z }
                })
                .collect()
        };
        let mut handles = Vec::with_capacity(ring_vertices.len());
        for (index, vertex) in ring_vertices.into_iter().enumerate() {
            // A control's vertex here keeps the control's elevation, a pick
            // already at this plan position its surveyed one; only a
            // genuinely new ring vertex takes the interpolated one.
            let handle = match ring_pinned[index] {
                Some(pinned) => pinned,
                None => match tin.locate_vertex(vertex.position) {
                    Some(existing) => existing.fix(),
                    None => tin.insert(vertex).map_err(|error| anyhow::anyhow!("{}", insert_failed(error)))?,
                },
            };
            handles.push(handle);
        }
        for index in 0..handles.len() {
            let (from, to) = (handles[index], handles[(index + 1) % handles.len()]);
            let through = std::mem::take(&mut ring_splits[index]);
            // Crossings were resolved above; the guard is for what they miss.
            constrain_chain(&mut tin, from, through, to, || tr!(literal = "The extent string crosses itself in plan"))?;
        }
    }

    // Every face now lies wholly inside the ring or wholly outside it, so its
    // centroid decides.
    // A centroid on the boundary belongs to no side, and a face whose middle
    // lands within the kernel's tolerance of the ring is a sliver along the
    // clip rather than ground inside it, so only strictly inside is kept.
    let (vertices, faces) = clipped_mesh(
        &tin,
        |centroid| bands.as_ref().is_none_or(|bands| bands.contains(centroid) == PolyContainment::Inside),
        cancelled,
    )?;
    if faces.is_empty() {
        anyhow::bail!("{}", tr!(literal = "No part of the surface falls inside the extent"));
    }
    let vertical_box = vertical_extent(
        vertices.iter().fold(f64::INFINITY, |low, vertex| low.min(vertex.z)),
        vertices.iter().fold(f64::NEG_INFINITY, |high, vertex| high.max(vertex.z)),
    );
    Ok(SurfaceMesh {
        vertices,
        faces,
        coincident,
        triangulated,
        support,
        vertical_box,
        controls: controls.len(),
        control_vertices,
        crossings: crossings.len(),
        overridden,
    })
}

/// The controls' indices sorted by their vertices, x then y then z at each
/// in turn, a string that runs out first coming first.
fn canonical_order(controls: &[Vec<DVec3>]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..controls.len()).collect();
    order.sort_by(|&left, &right| {
        let (left, right) = (&controls[left], &controls[right]);
        left.iter()
            .zip(right)
            .map(|(a, b)| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)))
            .find(|order| order.is_ne())
            .unwrap_or_else(|| left.len().cmp(&right.len()))
    });
    order
}

/// Refuse a control too short to make a segment, closed without the flag,
/// or crossing or doubling back on itself. Two controls crossing are
/// [`control_crossings`]'s concern.
fn validate_controls(controls: &[Vec<DVec3>], names: &[usize], cancelled: &dyn Fn() -> bool) -> Result<()> {
    for (index, control) in controls.iter().enumerate() {
        stop_if_cancelled(cancelled)?;
        if control.len() < MINIMUM_CONTROL_VERTICES {
            anyhow::bail!("{}", too_few_control_vertices(names[index], control.len()));
        }
        let plan: Vec<DVec2> = control.iter().map(|vertex| vertex.truncate()).collect();
        if plan.len() >= 4 && plan[0].distance(plan[plan.len() - 1]) <= kernel::XY_TOL {
            anyhow::bail!("{}", control_ends_where_it_starts(names[index]));
        }
        if doubles_back(&plan) {
            anyhow::bail!("{}", control_doubles_back(names[index]));
        }
        if self_intersects(&plan, false) {
            anyhow::bail!("{}", control_self_crossing(names[index]));
        }
    }
    Ok(())
}

/// Whether a segment of an open string turns back along the one before it,
/// its far end on that segment's line and pointing the other way.
fn doubles_back(string: &[DVec2]) -> bool {
    string.windows(3).any(|corner| {
        let (before, after) = (corner[1] - corner[0], corner[2] - corner[1]);
        before.perp_dot(corner[2] - corner[0]).abs() <= kernel::XY_TOL * before.length() && before.dot(after) < 0.0
    })
}

/// A plan position more than one control runs through: one vertex of the
/// surface, at the elevation they agree on, with every control that reaches it
/// constrained through it.
struct Crossing {
    position: DVec2,
    z: f64,
    sides: Vec<CrossingSide>,
}

impl Crossing {
    /// Keep one part per control segment. Three strings through one point meet
    /// pairwise, so each of their parts arrives twice.
    fn add(&mut self, side: CrossingSide) {
        if !self.sides.iter().any(|kept| (kept.control, kept.segment) == (side.control, side.segment)) {
            self.sides.push(side);
        }
    }
}

/// How one control reaches a crossing: which of its segments, how far along
/// it, and the vertex it already has there when it has one. A control ending
/// on another owns the crossing's vertex; one passing over it is split at it.
struct CrossingSide {
    control: usize,
    segment: usize,
    along: f64,
    vertex: Option<usize>,
}

/// Every plan position two different controls run through, each with the
/// elevation both give it. Read off the strings themselves, before anything is
/// inserted, so controls that disagree leave no half-built surface behind.
fn control_crossings(controls: &[Vec<DVec3>], names: &[usize], cancelled: &dyn Fn() -> bool) -> Result<Vec<Crossing>> {
    let mut crossings: Vec<Crossing> = Vec::new();
    // Every segment of every control on one grid, so only segments that come
    // near each other are compared; the pairs are then taken in the order a
    // walk over every pair would meet them, so the first refusal is the same.
    let segments: Vec<(usize, usize)> = controls
        .iter()
        .enumerate()
        .flat_map(|(index, control)| (0..control.len() - 1).map(move |segment| (index, segment)))
        .collect();
    let grid = BoxGrid::new(
        segments
            .iter()
            .map(|&(index, segment)| segment_box(controls[index][segment].truncate(), controls[index][segment + 1].truncate()))
            .collect(),
    );
    let mut found = CrossingCells::default();
    let (mut near, mut pairs) = (Vec::new(), Vec::new());
    for (left, first) in controls.iter().enumerate() {
        stop_if_cancelled(cancelled)?;
        pairs.clear();
        for a in 0..first.len() - 1 {
            let (low, high) = segment_box(first[a].truncate(), first[a + 1].truncate());
            grid.overlapping(low, high, &mut near);
            pairs.extend(near.iter().map(|&other| segments[other]).filter(|&(right, _)| right > left).map(|(right, b)| (right, a, b)));
        }
        pairs.sort_unstable();
        for &(right, a, b) in &pairs {
            let second = &controls[right];
            let meeting = kernel::segment_segment(first[a].truncate(), first[a + 1].truncate(), second[b].truncate(), second[b + 1].truncate());
            // A string ending on another reads the same as one running
            // across it: one plan position, two interpretations of it.
            let (point, t, u) = match meeting {
                SegSeg::Disjoint => continue,
                SegSeg::CollinearOverlap { .. } => anyhow::bail!("{}", controls_along_each_other(names[left], names[right])),
                SegSeg::Crossing { point, t, u } | SegSeg::Touching { point, t, u } => (point, t, u),
            };
            let (one, one_z) = crossing_side(first, left, a, t, point);
            let (other, other_z) = crossing_side(second, right, b, u, point);
            if (one_z - other_z).abs() > CONTROL_AGREEMENT {
                anyhow::bail!("{}", controls_disagree(names[left], names[right], point, one_z, other_z));
            }
            match found.first_near(&crossings, point) {
                Some(index) => {
                    crossings[index].add(one);
                    crossings[index].add(other);
                }
                None => {
                    found.add(point, crossings.len());
                    crossings.push(Crossing {
                        position: point,
                        z: one_z,
                        sides: vec![one, other],
                    });
                }
            }
        }
    }
    Ok(crossings)
}

/// The crossings found so far, filed by plan position so a new meeting finds
/// the first one within the kernel's tolerance without walking them all.
#[derive(Default)]
struct CrossingCells {
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl CrossingCells {
    /// Cells are a metre across, far wider than the tolerance, so a match is
    /// always in the cell a position falls in or one beside it.
    fn key(position: DVec2) -> (i64, i64) {
        (position.x.floor() as i64, position.y.floor() as i64)
    }

    fn add(&mut self, position: DVec2, index: usize) {
        self.cells.entry(Self::key(position)).or_default().push(index);
    }

    /// The earliest crossing within [`kernel::XY_TOL`] of a position.
    fn first_near(&self, crossings: &[Crossing], position: DVec2) -> Option<usize> {
        let (column, row) = Self::key(position);
        (column.saturating_sub(1)..=column.saturating_add(1))
            .flat_map(|column| (row.saturating_sub(1)..=row.saturating_add(1)).map(move |row| (column, row)))
            .filter_map(|key| self.cells.get(&key))
            .flatten()
            .copied()
            .filter(|&index| crossings[index].position.distance(position) <= kernel::XY_TOL)
            .min()
    }
}

/// One control's part in a crossing, with the elevation it reads there: its
/// own vertex's when the crossing lands on one, the segment's otherwise.
fn crossing_side(control: &[DVec3], index: usize, segment: usize, along: f64, point: DVec2) -> (CrossingSide, f64) {
    let vertex = nearer_end(point, control[segment].truncate(), control[segment + 1].truncate()).map(|end| segment + end);
    let z = match vertex {
        Some(vertex) => control[vertex].z,
        None => elevation_along(control[segment], control[segment + 1], along),
    };
    (
        CrossingSide {
            control: index,
            segment,
            along,
            vertex,
        },
        z,
    )
}

/// Whether any two of a string's segments meet away from the ends they share
/// with their neighbours, the first and last included when it is closed.
/// Any contact counts: an open string touching itself gives one plan
/// position two elevations, and a ring doing so bounds no single area.
fn self_intersects(points: &[DVec2], closed: bool) -> bool {
    let count = points.len();
    let segments = if closed { count } else { count.saturating_sub(1) };
    let grid = BoxGrid::new((0..segments).map(|segment| segment_box(points[segment], points[(segment + 1) % count])).collect());
    let mut near = Vec::new();
    (0..segments).any(|first| {
        let (low, high) = segment_box(points[first], points[(first + 1) % count]);
        grid.overlapping(low, high, &mut near);
        near.iter()
            .filter(|&&second| second >= first + 2 && !(closed && first == 0 && second == segments - 1))
            .any(|&second| kernel::segment_segment(points[first], points[first + 1], points[second], points[(second + 1) % count]) != SegSeg::Disjoint)
    })
}

/// How far a box is widened for the grids below: twice the kernel's plan
/// tolerance, so rounding can never hide a pair the kernel would call close.
const SEARCH_MARGIN: f64 = 2.0 * kernel::XY_TOL;

/// A segment's plan box, widened by [`SEARCH_MARGIN`].
fn segment_box(start: DVec2, end: DVec2) -> (DVec2, DVec2) {
    (start.min(end) - DVec2::splat(SEARCH_MARGIN), start.max(end) + DVec2::splat(SEARCH_MARGIN))
}

/// Boxes filed on a uniform grid, so only boxes sharing a cell are compared.
struct BoxGrid {
    low: DVec2,
    cell: f64,
    columns: usize,
    rows: usize,
    /// Where each cell's run of `members` starts, one more than the cells.
    starts: Vec<usize>,
    members: Vec<usize>,
    boxes: Vec<(DVec2, DVec2)>,
}

impl BoxGrid {
    fn new(boxes: Vec<(DVec2, DVec2)>) -> Self {
        let (low, high) = boxes
            .iter()
            .fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(low, high), (from, to)| (low.min(*from), high.max(*to)));
        let (low, span) = if boxes.is_empty() { (DVec2::ZERO, DVec2::ZERO) } else { (low, high - low) };
        let cell = grid_cell(span, boxes.len());
        let (columns, rows) = (cells_across(span.x, cell), cells_across(span.y, cell));
        let mut grid = Self {
            low,
            cell,
            columns,
            rows,
            starts: vec![0; columns * rows + 1],
            members: Vec::new(),
            boxes,
        };
        for index in 0..grid.boxes.len() {
            for cell in grid.cells(grid.boxes[index]) {
                grid.starts[cell + 1] += 1;
            }
        }
        for cell in 0..columns * rows {
            grid.starts[cell + 1] += grid.starts[cell];
        }
        let mut next = grid.starts.clone();
        let mut members = vec![0; grid.starts[columns * rows]];
        for index in 0..grid.boxes.len() {
            for cell in grid.cells(grid.boxes[index]) {
                members[next[cell]] = index;
                next[cell] += 1;
            }
        }
        grid.members = members;
        grid
    }

    /// The cells a box covers, clamped to the grid.
    fn cells(&self, (from, to): (DVec2, DVec2)) -> impl Iterator<Item = usize> + use<> {
        let columns = self.columns;
        let (first_column, last_column) = (cell_of(from.x, self.low.x, self.cell, columns), cell_of(to.x, self.low.x, self.cell, columns));
        let (first_row, last_row) = (cell_of(from.y, self.low.y, self.cell, self.rows), cell_of(to.y, self.low.y, self.cell, self.rows));
        (first_row..=last_row).flat_map(move |row| (first_column..=last_column).map(move |column| row * columns + column))
    }

    /// The boxes overlapping the one from `from` to `to`, ascending and each
    /// once, into `found`.
    fn overlapping(&self, from: DVec2, to: DVec2, found: &mut Vec<usize>) {
        found.clear();
        for cell in self.cells((from, to)) {
            for &index in &self.members[self.starts[cell]..self.starts[cell + 1]] {
                let (low, high) = self.boxes[index];
                if low.x <= to.x && from.x <= high.x && low.y <= to.y && from.y <= high.y {
                    found.push(index);
                }
            }
        }
        found.sort_unstable();
        found.dedup();
    }
}

/// A cell size giving a grid about as many cells as it holds items, never so
/// fine along a thin span that one axis outnumbers them. A span that is not
/// a finite size gets one cell.
fn grid_cell(span: DVec2, count: usize) -> f64 {
    let count = count.max(1) as f64;
    let cell = (span.x * span.y / count).sqrt().max(span.x.max(span.y) / count);
    if cell.is_finite() && cell > 0.0 { cell } else { f64::INFINITY }
}

fn cells_across(span: f64, cell: f64) -> usize {
    ((span / cell).floor() as usize).saturating_add(1)
}

/// The cell a coordinate falls in along one axis, clamped to the grid.
fn cell_of(value: f64, low: f64, cell: f64, count: usize) -> usize {
    (((value - low) / cell).floor().max(0.0) as usize).min(count - 1)
}

/// Put a control's vertex into the triangulation. A vertex already within the
/// kernel's plan tolerance keeps its place and takes the control's
/// elevation: the string wins, and a pick it wins over is reported.
fn insert_control_vertex(
    tin: &mut SurfaceTriangulation,
    grid: &mut VertexGrid,
    owned: &mut Owners,
    overridden: &mut Vec<(DVec2, f64, f64)>,
    control: usize,
    vertex: DVec3,
) -> Result<FixedVertexHandle> {
    use spade::Triangulation as _;

    if let Some(handle) = nearest_plan_vertex(grid, vertex.truncate()) {
        take_over_vertex(tin, owned, overridden, control, handle, vertex.z)?;
        return Ok(handle);
    }
    let before = tin.num_vertices();
    let handle = tin
        .insert(SurfaceVertex {
            position: spade::Point2::new(vertex.x, vertex.y),
            z: vertex.z,
        })
        .map_err(|error| anyhow::anyhow!("{}", insert_failed(error)))?;
    if tin.num_vertices() > before {
        grid.add(plan(tin.vertex(handle).position()), handle);
    }
    owned.add(handle.index(), control);
    Ok(handle)
}

/// The controls each control vertex belongs to, by vertex index and each
/// control's place in the selection. The first to reach it gave it its
/// elevation.
#[derive(Default)]
struct Owners {
    first: HashMap<usize, usize>,
    also: HashSet<(usize, usize)>,
}

impl Owners {
    fn has(&self, vertex: usize, control: usize) -> bool {
        self.first.get(&vertex) == Some(&control) || self.also.contains(&(vertex, control))
    }

    fn add(&mut self, vertex: usize, control: usize) {
        match self.first.get(&vertex) {
            None => {
                self.first.insert(vertex, control);
            }
            Some(&first) if first != control => {
                self.also.insert((vertex, control));
            }
            Some(_) => {}
        }
    }

    fn len(&self) -> usize {
        self.first.len()
    }
}

/// Give a vertex already in the mesh a control's elevation. A pick's is
/// replaced, and reported when the two differ by more than
/// [`kernel::Z_TOL`]; a control's own vertex keeps its first, and another
/// control reaching it must agree with it or the build is refused.
fn take_over_vertex(tin: &mut SurfaceTriangulation, owned: &mut Owners, overridden: &mut Vec<(DVec2, f64, f64)>, control: usize, handle: FixedVertexHandle, z: f64) -> Result<()> {
    use spade::Triangulation as _;

    let vertex = tin.vertex_data_mut(handle);
    match owned.first.get(&handle.index()).copied() {
        None => {
            if (z - vertex.z).abs() > kernel::Z_TOL {
                overridden.push((plan(vertex.position), vertex.z, z));
            }
            vertex.z = z;
        }
        Some(_) if owned.has(handle.index(), control) => {}
        Some(owner) if (z - vertex.z).abs() > CONTROL_AGREEMENT => {
            anyhow::bail!("{}", controls_disagree(owner, control, plan(vertex.position), vertex.z, z));
        }
        Some(_) => {}
    }
    owned.add(handle.index(), control);
    Ok(())
}

/// The vertex within the kernel's plan tolerance of a position, if any; of
/// two as near, the earlier. Looked up on the grid because spade's own lookup
/// matches exact coordinates only.
fn nearest_plan_vertex(grid: &VertexGrid, position: DVec2) -> Option<FixedVertexHandle> {
    let mut nearest: Option<(f64, FixedVertexHandle)> = None;
    grid.within(position - DVec2::splat(SEARCH_MARGIN), position + DVec2::splat(SEARCH_MARGIN), |at, handle| {
        let distance = at.distance_squared(position);
        if distance <= kernel::XY_TOL * kernel::XY_TOL && nearest.is_none_or(|kept| (distance, handle.index()) < (kept.0, kept.1.index())) {
            nearest = Some((distance, handle));
        }
    });
    nearest.map(|(_, handle)| handle)
}

/// The vertices that lie along a segment in plan, in mesh order, each with
/// where along it they fall. The ends are left out: a vertex there is the
/// control's own.
fn vertices_along(grid: &VertexGrid, start: DVec2, end: DVec2) -> Vec<(f64, FixedVertexHandle)> {
    let (low, high) = segment_box(start, end);
    let mut found = Vec::new();
    grid.within(low, high, |position, handle| {
        let (closest, along) = kernel::project_onto_segment(position, start, end);
        let interior = nearer_end(closest, start, end).is_none();
        if interior && position.distance(closest) <= kernel::XY_TOL {
            found.push((along, handle));
        }
    });
    found.sort_unstable_by_key(|(_, handle)| handle.index());
    found
}

/// Which end of a segment a point coincides with in plan, when it coincides
/// with either: `0` for the start, `1` for the end.
fn nearer_end(point: DVec2, start: DVec2, end: DVec2) -> Option<usize> {
    let (to_start, to_end) = (point.distance(start), point.distance(end));
    (to_start.min(to_end) <= kernel::XY_TOL).then(|| usize::from(to_end < to_start))
}

/// The elevation a segment has at the fraction `along` of its length.
fn elevation_along(start: DVec3, end: DVec3, along: f64) -> f64 {
    start.z + (end.z - start.z) * along
}

/// Constrain the run from `from` to `to` through the vertices found along it,
/// in order. Guarded because a constraint spade cannot add panics, and a
/// panic in the worker takes the browser build down with it.
fn constrain_chain(
    tin: &mut SurfaceTriangulation,
    from: FixedVertexHandle,
    mut through: Vec<(f64, FixedVertexHandle)>,
    to: FixedVertexHandle,
    refused: impl Fn() -> String,
) -> Result<()> {
    through.sort_by(|left, right| left.0.total_cmp(&right.0));
    let sequence: Vec<FixedVertexHandle> = std::iter::once(from)
        .chain(through.into_iter().map(|(_, handle)| handle))
        .chain(std::iter::once(to))
        .collect();
    for pair in sequence.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if from == to {
            continue;
        }
        if !tin.can_add_constraint(from, to) {
            anyhow::bail!("{}", refused());
        }
        tin.add_constraint(from, to);
    }
    Ok(())
}

/// Whether the triangulation has a triangle with real area in plan, rather
/// than only the slivers a line of points makes, judged against the picks'
/// own span: see [`DEGENERATE_AREA_FRACTION`].
fn has_plan_area(tin: &SurfaceTriangulation) -> bool {
    use spade::Triangulation as _;

    let (low, high) = tin.vertices().fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(low, high), vertex| {
        let position = plan(vertex.position());
        (low.min(position), high.max(position))
    });
    // The bounding box's diagonal squared: the area a triangle spanning the
    // picks would be in the region of, and zero when they are all one point.
    let span_squared = (high - low).length_squared();
    if !span_squared.is_finite() || span_squared <= 0.0 {
        return false;
    }
    let largest = tin
        .inner_faces()
        .fold(0.0f64, |largest, face| largest.max(twice_plan_area(face.vertices().map(|vertex| vertex.position())).abs()));
    largest > DEGENERATE_AREA_FRACTION * span_squared
}

/// A spade position as a plan vector.
fn plan(position: spade::Point2<f64>) -> DVec2 {
    DVec2::new(position.x, position.y)
}

/// The elevation of the pick nearest a plan position. Beyond the picks' hull
/// there is no triangle to interpolate on, so the nearest pick stands in.
/// Of two as near, the earlier.
fn nearest_pick_z(tin: &SurfaceTriangulation, grid: &VertexGrid, position: spade::Point2<f64>) -> Option<f64> {
    use spade::Triangulation as _;

    let distance = |vertex: DVec2| (vertex.x - position.x).powi(2) + (vertex.y - position.y).powi(2);
    grid.nearest(plan(position), distance).map(|handle| tin.vertex(handle).data().z)
}

/// The mesh's vertices filed by plan position on a uniform grid, kept in step
/// with the mesh as control vertices go in.
struct VertexGrid {
    low: DVec2,
    cell: f64,
    cells: HashMap<(i64, i64), Vec<(DVec2, FixedVertexHandle)>>,
    /// The lowest and highest occupied cells.
    occupied: Option<((i64, i64), (i64, i64))>,
}

impl VertexGrid {
    /// Sized for the picks already in the mesh and the control vertices to
    /// come, filled with the picks.
    fn new(tin: &SurfaceTriangulation, controls: &[Vec<DVec3>]) -> Self {
        use spade::Triangulation as _;

        let positions = tin
            .vertices()
            .map(|vertex| plan(vertex.position()))
            .chain(controls.iter().flatten().map(|vertex| vertex.truncate()));
        let (low, high, count) = positions.fold((DVec2::INFINITY, DVec2::NEG_INFINITY, 0usize), |(low, high, count), position| {
            (low.min(position), high.max(position), count + 1)
        });
        let (low, span) = if count == 0 { (DVec2::ZERO, DVec2::ZERO) } else { (low, high - low) };
        let mut grid = Self {
            low,
            cell: grid_cell(span, count),
            cells: HashMap::new(),
            occupied: None,
        };
        for vertex in tin.vertices() {
            grid.add(plan(vertex.position()), vertex.fix());
        }
        grid
    }

    fn key(&self, position: DVec2) -> (i64, i64) {
        (
            ((position.x - self.low.x) / self.cell).floor() as i64,
            ((position.y - self.low.y) / self.cell).floor() as i64,
        )
    }

    fn add(&mut self, position: DVec2, handle: FixedVertexHandle) {
        let key = self.key(position);
        self.cells.entry(key).or_default().push((position, handle));
        self.occupied = Some(match self.occupied {
            None => (key, key),
            Some((low, high)) => ((low.0.min(key.0), low.1.min(key.1)), (high.0.max(key.0), high.1.max(key.1))),
        });
    }

    /// Every vertex in the cells the box from `from` to `to` touches, in no
    /// particular order.
    fn within(&self, from: DVec2, to: DVec2, mut visit: impl FnMut(DVec2, FixedVertexHandle)) {
        let Some((low, high)) = self.occupied else {
            return;
        };
        let (first, last) = (self.key(from), self.key(to));
        let (first, last) = ((first.0.max(low.0), first.1.max(low.1)), (last.0.min(high.0), last.1.min(high.1)));
        if first.0 > last.0 || first.1 > last.1 {
            return;
        }
        let covered = (last.0 as f64 - first.0 as f64 + 1.0) * (last.1 as f64 - first.1 as f64 + 1.0);
        let in_range = |key: &(i64, i64)| (first.0..=last.0).contains(&key.0) && (first.1..=last.1).contains(&key.1);
        // A box wider than the vertices are spread is cheaper walked cell by
        // occupied cell than by every empty one it covers.
        if covered > self.cells.len() as f64 {
            for (_, members) in self.cells.iter().filter(|(key, _)| in_range(key)) {
                members.iter().for_each(|&(position, handle)| visit(position, handle));
            }
            return;
        }
        for column in first.0..=last.0 {
            for row in first.1..=last.1 {
                if let Some(members) = self.cells.get(&(column, row)) {
                    members.iter().for_each(|&(position, handle)| visit(position, handle));
                }
            }
        }
    }

    /// The vertex `distance` puts nearest a position, the earlier of two as
    /// near. The search widens until the nearest found is closer than any
    /// vertex outside it could be.
    fn nearest(&self, position: DVec2, distance: impl Fn(DVec2) -> f64) -> Option<FixedVertexHandle> {
        let (low, high) = self.occupied?;
        let occupied_from = self.low + DVec2::new(low.0 as f64, low.1 as f64) * self.cell;
        let occupied_to = self.low + DVec2::new(high.0 as f64 + 1.0, high.1 as f64 + 1.0) * self.cell;
        let mut reach = (position.clamp(occupied_from, occupied_to).distance(position) + self.cell).min(f64::MAX);
        loop {
            let (from, to) = (position - DVec2::splat(reach + SEARCH_MARGIN), position + DVec2::splat(reach + SEARCH_MARGIN));
            let everything = !reach.is_finite() || {
                let (first, last) = (self.key(from), self.key(to));
                first.0 <= low.0 && first.1 <= low.1 && last.0 >= high.0 && last.1 >= high.1
            };
            let mut nearest: Option<(f64, FixedVertexHandle)> = None;
            let mut consider = |at: DVec2, handle: FixedVertexHandle| {
                let squared = distance(at);
                if (everything || squared <= reach * reach) && nearest.is_none_or(|kept| (squared, handle.index()) < (kept.0, kept.1.index())) {
                    nearest = Some((squared, handle));
                }
            };
            if everything {
                self.cells.values().flatten().for_each(|&(at, handle)| consider(at, handle));
            } else {
                self.within(from, to, consider);
            }
            if nearest.is_some() || everything {
                return nearest.map(|(_, handle)| handle);
            }
            reach *= 2.0;
        }
    }
}

/// The most band entries a mask edge may make on average, so a ring of tall
/// edges cannot make the index outgrow the ring many times over.
const BAND_ENTRIES_PER_EDGE: usize = 8;

/// A ring's edges filed by the horizontal bands they span, so a point is
/// tested only against the edges level with it. Answers exactly as
/// [`kernel::point_in_polyline`] does over the whole ring.
struct RingBands<'a> {
    ring: &'a [DVec2],
    low: f64,
    height: f64,
    /// Where each band's run of `members` starts, one more than the bands.
    starts: Vec<usize>,
    members: Vec<usize>,
    /// Edges of non-zero length; under three and nothing is inside.
    edges: usize,
}

impl<'a> RingBands<'a> {
    fn new(ring: &'a [DVec2]) -> Self {
        let count = ring.len();
        let edge = |index: usize| (ring[index], ring[(index + 1) % count]);
        let span = |index: usize| {
            let (a, b) = edge(index);
            (a.y.min(b.y) - SEARCH_MARGIN, a.y.max(b.y) + SEARCH_MARGIN)
        };
        let (low, high) = (0..count)
            .map(span)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), (from, to)| (low.min(from), high.max(to)));
        let (low, mut height) = if count == 0 {
            (0.0, f64::INFINITY)
        } else {
            (low, grid_cell(DVec2::new(0.0, high - low), count))
        };
        let mut bands = cells_across(high - low, height).min(count.max(1));
        // Tall edges sit in many bands, so bands are widened until the index
        // holds a few entries per edge; one band is the plain walk.
        let filed = |height: f64, bands: usize| {
            (0..count)
                .map(span)
                .map(|(from, to)| cell_of(to, low, height, bands) - cell_of(from, low, height, bands) + 1)
                .sum::<usize>()
        };
        while bands > 1 && filed(height, bands) > BAND_ENTRIES_PER_EDGE * count {
            height *= 2.0;
            bands = cells_across(high - low, height).min(count.max(1));
        }
        let mut starts = vec![0; bands + 1];
        let band_of = |y: f64| cell_of(y, low, height, bands);
        for index in 0..count {
            let (from, to) = span(index);
            for band in band_of(from)..=band_of(to) {
                starts[band + 1] += 1;
            }
        }
        for band in 0..bands {
            starts[band + 1] += starts[band];
        }
        let mut next = starts.clone();
        let mut members = vec![0; starts[bands]];
        for index in 0..count {
            let (from, to) = span(index);
            for band in band_of(from)..=band_of(to) {
                members[next[band]] = index;
                next[band] += 1;
            }
        }
        Self {
            ring,
            low,
            height,
            starts,
            members,
            edges: (0..count).filter(|&index| edge(index).0 != edge(index).1).count(),
        }
    }

    fn contains(&self, point: DVec2) -> PolyContainment {
        let count = self.ring.len();
        if count == 0 {
            return PolyContainment::Outside;
        }
        let band = cell_of(point.y, self.low, self.height, self.starts.len() - 1);
        let mut inside = false;
        for &index in &self.members[self.starts[band]..self.starts[band + 1]] {
            let (a, b) = (self.ring[index], self.ring[(index + 1) % count]);
            if a == b {
                continue;
            }
            let (closest, _) = kernel::project_onto_segment(point, a, b);
            if point.distance(closest) <= kernel::XY_TOL {
                return PolyContainment::OnBoundary;
            }
            let upward = a.y <= point.y && point.y < b.y;
            let downward = b.y <= point.y && point.y < a.y;
            if (upward && kernel::orient2d(a, b, point) > 0.0) || (downward && kernel::orient2d(a, b, point) < 0.0) {
                inside = !inside;
            }
        }
        if self.edges >= 3 && inside { PolyContainment::Inside } else { PolyContainment::Outside }
    }
}

/// The vertical box the surface is modelled in: the surface's own range R
/// added above and below it, then rounded outward to the next 10 m so the box
/// is a round number rather than an accident of the data.
fn vertical_extent(low_z: f64, high_z: f64) -> (f64, f64) {
    let range = high_z - low_z;
    (
        ((low_z - range) / EXTENT_ROUNDING).floor() * EXTENT_ROUNDING,
        ((high_z + range) / EXTENT_ROUNDING).ceil() * EXTENT_ROUNDING,
    )
}

/// Twice the signed plan area of a face. Through the kernel's adaptive
/// predicate, so the sign is exact and a face that is flat is flat rather
/// than however the rounding of coordinates near 1e6 happened to fall.
fn twice_plan_area(positions: [spade::Point2<f64>; 3]) -> f64 {
    kernel::orient2d(plan(positions[0]), plan(positions[1]), plan(positions[2]))
}

/// The faces the filter keeps, counter-clockwise in plan so their normals
/// point up, with only the vertices those faces use. Points dropped by the
/// clip were support: they shaped the trend and leave no geometry behind.
fn clipped_mesh(tin: &SurfaceTriangulation, keep: impl Fn(DVec2) -> bool, cancelled: &dyn Fn() -> bool) -> Result<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)> {
    use spade::Triangulation as _;

    let mut vertices: Vec<mesh_data::Vertex> = Vec::new();
    let mut index_of: HashMap<usize, u32> = HashMap::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();
    for face in tin.inner_faces() {
        stop_if_cancelled(cancelled)?;
        let face_vertices = face.vertices();
        let positions = face_vertices.map(|vertex| vertex.position());
        let twice_area = twice_plan_area(positions);
        // Exactly flat, so it has no winding to read a normal from and no
        // surface to contribute. Exact because the area came from the
        // kernel's predicate; no epsilon stands in for it.
        if twice_area == 0.0 {
            continue;
        }
        let centroid = DVec2::new(
            (positions[0].x + positions[1].x + positions[2].x) / 3.0,
            (positions[0].y + positions[1].y + positions[2].y) / 3.0,
        );
        if !keep(centroid) {
            continue;
        }
        let mut triangle = face_vertices.map(|vertex| {
            *index_of.entry(vertex.fix().index()).or_insert_with(|| {
                let position = vertex.position();
                vertices.push(mesh_data::Vertex::new(position.x, position.y, vertex.data().z));
                (vertices.len() - 1) as u32
            })
        });
        // Counter-clockwise in plan, so the normal points up.
        if twice_area < 0.0 {
            triangle.swap(1, 2);
        }
        faces.push(triangle);
    }
    Ok((vertices, faces))
}
