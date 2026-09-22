//! A reference surface: the selected points triangulated in plan and cut
//! back to an optional extent. The Delaunay is over every selected point,
//! inside the extent or outside it, so distant data still shapes the trend;
//! only the finished surface is clipped to the boundary.

use glam::{DVec2, DVec3};

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
}

/// What a surface build takes from a selection: the points to triangulate,
/// the layers they came from, and the one closed string clipping the result.
pub(crate) struct SurfaceInput {
    pub(crate) points: Vec<ObjectId>,
    /// Distinct layers the points sit on, in document order. One layer is the
    /// ordinary case; more than one means the surface has no single source.
    pub(crate) layers: Vec<LayerId>,
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
    Ok(SurfaceInput { points, layers, extent })
}

impl<'a> App<'a> {
    /// Triangulate the selected points into a new surface, named for their
    /// layer when they share one. Every run adds a surface; nothing is
    /// replaced.
    pub(crate) fn build_reference_surface(&mut self, points: Vec<ObjectId>, extent: Option<ObjectId>) -> Result<()> {
        let project = self
            .workspace
            .active_project()
            .ok_or_else(|| anyhow::anyhow!("{}", tr!(literal = "Open a project before building a surface")))?;
        let project_key = crate::app::jobs::JobKey::Project {
            runtime_id: project.runtime_id,
            document_revision: project.project.document.revision(),
        };
        // Points and extent both come from the scene document, which is what
        // the selection was read against: a string the geologist could see
        // and choose is the string that clips.
        let ring = extent.map(|id| extent_ring(&self.scene_document, id)).transpose()?;
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

        let compute = move |cancel: &crate::app::jobs::CancelFlag| -> Result<crate::model::triangulation::GeneratedTriangulation> {
            if cancel.is_cancelled() {
                anyhow::bail!("{}", tr!(literal = "Cancelled"));
            }
            delaunay_surface_from_points(points, ring, name)
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

/// Worker half: a Delaunay in plan over the points, elevations carried on the
/// vertices, clipped to the extent when there is one.
fn delaunay_surface_from_points(points: Vec<DVec3>, extent: Option<Vec<DVec2>>, name: String) -> Result<crate::model::triangulation::GeneratedTriangulation> {
    let surface = surface_mesh(&points, extent.as_deref())?;
    userspace_log!(
        "{}",
        tr_format!(
            literal = "Built surface %name% from %vertex_count% point(s) into %face_count% face(s), box z %low% to %high%%support%%coincident%",
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
            }
        )
    );
    session::build_generated_triangulation(name, surface.vertices, surface.faces, TriSurfaceType::Surface, crate::model::triangulation::unique_edges)
}

/// Triangulate the points in plan and, given an extent, cut the result back
/// to it. Two points at one plan position keep one vertex, so a twin hole
/// never fails the build; the count is reported.
fn surface_mesh(points: &[DVec3], extent: Option<&[DVec2]>) -> Result<SurfaceMesh> {
    use spade::{FloatTriangulation as _, Triangulation as _};

    if points.len() < MINIMUM_POINTS {
        anyhow::bail!("{}", too_few_points(points.len()));
    }
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

    if let Some(ring) = extent {
        // A ring that crosses or touches itself bounds no single area, so
        // there is no answer to what the clip should keep. Judged on the
        // ring's own geometry: whether an extent is usable cannot depend on
        // where the picks happen to sit.
        if ring_self_intersects(ring) {
            anyhow::bail!("{}", tr!(literal = "The extent string crosses itself in plan"));
        }
        // A pick surveyed onto the string belongs to the ground the string
        // bounds, so the boundary counts as inside here: the kernel names
        // that case instead of leaving it to which way the arithmetic fell.
        support = tin
            .vertices()
            .filter(|vertex| {
                !matches!(
                    kernel::point_in_polyline(plan(vertex.position()), ring.iter().copied()),
                    PolyContainment::Inside | PolyContainment::OnBoundary
                )
            })
            .count();
        let inside = triangulated - support;
        if inside < MINIMUM_POINTS {
            anyhow::bail!("{}", too_few_points_inside(inside));
        }
        // The ring's own vertices are given the elevation the picks' trend
        // has at that plan position, so the clipped edge lies on the surface
        // rather than at the height the string was drawn at. Read before the
        // constraints go in, while the triangulation is still the picks'.
        let ring_vertices: Vec<SurfaceVertex> = {
            let trend = tin.barycentric();
            ring.iter()
                .map(|vertex| {
                    let position = spade::Point2::new(vertex.x, vertex.y);
                    let z = trend
                        .interpolate(|vertex| vertex.data().z, position)
                        .or_else(|| nearest_pick_z(&tin, position))
                        .unwrap_or_default();
                    SurfaceVertex { position, z }
                })
                .collect()
        };
        let mut handles = Vec::with_capacity(ring_vertices.len());
        for vertex in ring_vertices {
            // A pick already at this plan position keeps its surveyed
            // elevation; only a genuinely new ring vertex takes the
            // interpolated one.
            let handle = match tin.locate_vertex(vertex.position) {
                Some(existing) => existing.fix(),
                None => tin.insert(vertex).map_err(|error| anyhow::anyhow!("{}", insert_failed(error)))?,
            };
            handles.push(handle);
        }
        for index in 0..handles.len() {
            let (from, to) = (handles[index], handles[(index + 1) % handles.len()]);
            if from == to {
                continue;
            }
            // The ring was checked for crossings above; this keeps spade
            // from panicking on anything that check cannot see, because a
            // panic in the worker takes the browser build down with it.
            if !tin.can_add_constraint(from, to) {
                anyhow::bail!("{}", tr!(literal = "The extent string crosses itself in plan"));
            }
            tin.add_constraint(from, to);
        }
    }

    // Every face now lies wholly inside the ring or wholly outside it, so its
    // centroid decides.
    // A centroid on the boundary belongs to no side, and a face whose middle
    // lands within the kernel's tolerance of the ring is a sliver along the
    // clip rather than ground inside it, so only strictly inside is kept.
    let (vertices, faces) = clipped_mesh(&tin, |centroid| {
        extent.is_none_or(|ring| kernel::point_in_polyline(centroid, ring.iter().copied()) == PolyContainment::Inside)
    });
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
    })
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
fn nearest_pick_z(tin: &SurfaceTriangulation, position: spade::Point2<f64>) -> Option<f64> {
    use spade::Triangulation as _;

    let distance = |vertex: spade::Point2<f64>| (vertex.x - position.x).powi(2) + (vertex.y - position.y).powi(2);
    tin.vertices()
        .min_by(|left, right| distance(left.position()).total_cmp(&distance(right.position())))
        .map(|vertex| vertex.data().z)
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
fn clipped_mesh(tin: &SurfaceTriangulation, keep: impl Fn(DVec2) -> bool) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
    use spade::Triangulation as _;

    let mut vertices: Vec<mesh_data::Vertex> = Vec::new();
    let mut index_of: HashMap<usize, u32> = HashMap::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();
    for face in tin.inner_faces() {
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
    (vertices, faces)
}

/// Whether any two of the ring's segments meet away from the ends they share
/// with their neighbours. Adjacent pairs, the first and last edge included,
/// share an endpoint by construction and are never asked; for the rest any
/// contact counts, since none leaves a single area to clip to.
fn ring_self_intersects(ring: &[DVec2]) -> bool {
    let count = ring.len();
    (0..count).any(|first| {
        ((first + 2)..count)
            .filter(|second| !(first == 0 && *second == count - 1))
            .any(|second| kernel::segment_segment(ring[first], ring[first + 1], ring[second], ring[(second + 1) % count]) != SegSeg::Disjoint)
    })
}
