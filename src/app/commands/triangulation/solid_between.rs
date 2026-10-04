//! Build a closed solid from two surfaces - the staple "volume between a
//! design and the ground" construction.
//!
//! Feeds both the Triangulation menu's own tool and the Solids Setup page's
//! inspection preview, so the two can never disagree about what a solid is.

use anyhow::Result;
use rayon::prelude::*;

use super::{
    cuts::clip_mesh_by_surface,
    session::{self},
    *,
};
use crate::ui::state::SolidRegion;

/// The two sheets a solid is bounded by, before they are welded into one mesh.
///
/// A solid is the space between the design surface and the topography over the
/// area where the design is on the wanted side of the ground - below it for a
/// cut, above it for a fill. Its floor and roof are one piece of each surface:
/// for a cut, the design is the floor and the topography the roof, and for a
/// fill the two swap. Both are clipped at the line where the surfaces cross,
/// where the two clips agree exactly, so the welded mesh closes along it with
/// no stitched side wall.
///
/// The *other* region the two surfaces bound - the fill either side of a pit's
/// crest, say - belongs to a different solid and is deliberately not part of
/// this one.
struct SolidEnvelopes {
    lower: (Vec<mesh_data::Vertex>, Vec<[u32; 3]>),
    upper: (Vec<mesh_data::Vertex>, Vec<[u32; 3]>),
}

/// Face-winding convention for one envelope: outward from the enclosed volume.
#[derive(Clone, Copy, PartialEq)]
enum Facing {
    /// The floor, whose outward normal points down.
    Down,
    /// The roof, whose outward normal points up.
    Up,
}

/// Append `(vertices, faces)` onto `output`, rewinding each face so its normal
/// points the way `facing` asks.
///
/// The inputs are 2.5D fragments of clipped surfaces - `clip_mesh_by_surface`
/// drops the vertical and degenerate faces that have no XY area - so the sign
/// of a face's normal Z is exactly its up/down orientation, and flipping two
/// indices is all a face ever needs. The source meshes' own winding is
/// therefore irrelevant, which matters because an imported design surface and
/// an imported topography rarely agree about it.
fn append_oriented(source: (Vec<mesh_data::Vertex>, Vec<[u32; 3]>), facing: Facing, output: &mut (Vec<mesh_data::Vertex>, Vec<[u32; 3]>)) {
    let (vertices, faces) = source;
    let base = output.0.len() as u32;
    for face in faces {
        let corners = face.map(|index| vertices[index as usize]);
        let edge_a = glam::DVec2::new(corners[1].x - corners[0].x, corners[1].y - corners[0].y);
        let edge_b = glam::DVec2::new(corners[2].x - corners[0].x, corners[2].y - corners[0].y);
        let normal_z = edge_a.x * edge_b.y - edge_a.y * edge_b.x;
        let points_up = normal_z > 0.0;
        let wanted_up = facing == Facing::Up;
        let face = [base + face[0], base + face[1], base + face[2]];
        output.1.push(if points_up == wanted_up { face } else { [face[0], face[2], face[1]] });
    }
    output.0.extend(vertices);
}

/// Clip each surface to the wanted region and hand back the two sheets that
/// bound it.
fn build_envelopes(
    design: &mesh_data::Triangulation,
    topography: &mesh_data::Triangulation,
    region: SolidRegion,
    progress: &crate::model::progress::Phase,
) -> Result<SolidEnvelopes> {
    // `CutTop` keeps what lies below the reference, `CutBottom` what lies
    // above it. For a cut the floor is the design where it runs below the
    // ground and the roof is the ground above it; for a fill the roles - and
    // so the sides - are the other way round.
    let (design_side, topography_side) = match region {
        SolidRegion::Cut => (TriSurfaceCutSide::CutTop, TriSurfaceCutSide::CutBottom),
        SolidRegion::Fill => (TriSurfaceCutSide::CutBottom, TriSurfaceCutSide::CutTop),
    };
    let design_sheet = clip_mesh_by_surface(design, topography, design_side, &progress.phase(0.0, 0.5))?;
    let topography_sheet = clip_mesh_by_surface(topography, design, topography_side, &progress.phase(0.5, 1.0))?;

    let (floor, roof) = match region {
        SolidRegion::Cut => (design_sheet, topography_sheet),
        SolidRegion::Fill => (topography_sheet, design_sheet),
    };

    let mut lower = (Vec::new(), Vec::new());
    append_oriented(floor, Facing::Down, &mut lower);
    let mut upper = (Vec::new(), Vec::new());
    append_oriented(roof, Facing::Up, &mut upper);

    Ok(SolidEnvelopes { lower, upper })
}

/// The volume enclosed between two surfaces on one side of their crossing, as
/// one mesh.
///
/// Returns the solid's vertices and faces plus the volume it encloses, in
/// cubic model units. Errors when there is nothing to enclose: surfaces whose
/// XY footprints miss each other, or a design that never runs on the wanted
/// side of the ground.
pub(crate) fn build_solid_between_surfaces(
    design: &mesh_data::Triangulation,
    topography: &mesh_data::Triangulation,
    region: SolidRegion,
    progress: &crate::model::progress::Phase,
) -> Result<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>, f64)> {
    // Surface intersections must be computed near the origin: line and
    // barycentric arithmetic on mine eastings/northings loses enough precision
    // to tear matching rims apart. Restore domain coordinates only afterwards.
    let origin = design.bounds().min;
    let local = |mesh: &mesh_data::Triangulation| -> Result<mesh_data::Triangulation> {
        mesh_data::Triangulation::from_vertices_and_faces(
            mesh.vertices()
                .iter()
                .map(|v| mesh_data::Vertex::new(v.x - origin.x, v.y - origin.y, v.z - origin.z))
                .collect(),
            mesh.face_vertex_indices_iter().map(|f| f.map(|i| i as u32)).collect(),
        )
        .map_err(|error| anyhow::anyhow!("{error}"))
    };
    let (mut vertices, faces, volume) = build_solid_local(&local(design)?, &local(topography)?, region, progress)?;
    for v in &mut vertices {
        v.x += origin.x;
        v.y += origin.y;
        v.z += origin.z;
    }
    Ok((vertices, faces, volume))
}

fn build_solid_local(
    design: &mesh_data::Triangulation,
    topography: &mesh_data::Triangulation,
    region: SolidRegion,
    progress: &crate::model::progress::Phase,
) -> Result<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>, f64)> {
    let SolidEnvelopes { lower, upper } = build_envelopes(design, topography, region, progress)?;
    if lower.1.is_empty() || upper.1.is_empty() {
        anyhow::bail!(
            "The two surfaces enclose no {} volume: the design surface is nowhere {} the topography within the area they share",
            region.label().to_lowercase(),
            match region {
                SolidRegion::Cut => "below",
                SolidRegion::Fill => "above",
            }
        );
    }

    let volume = volume_between(&lower, &upper);
    let walls = close_region_sides(&lower, &upper);

    let mut vertices = lower.0;
    let mut faces = lower.1;
    for part in [upper, walls] {
        let base = vertices.len() as u32;
        vertices.extend(part.0);
        faces.extend(part.1.into_iter().map(|face| [base + face[0], base + face[1], base + face[2]]));
    }

    // The sheets close against each other along the line where the surfaces
    // cross, and against the wall everywhere else. Anything still open is a
    // rim the wall could not trace - a sheet torn into pieces too small to
    // ring, say - and is worth saying rather than passing off as a solid.
    let open_edges = open_edge_count(&vertices, &faces);
    if open_edges > 0 {
        userspace_warn!("{}", tr!("tri-solid-open-along-edge", count = open_edges.to_string()));
    }

    Ok((vertices, faces, volume))
}

/// The volume between the two sheets, by integrating their height difference
/// over the region they share.
///
/// Both sheets cover exactly the same XY region - they are the two surfaces
/// clipped by the same predicate - so the volume is the roof's integral of z
/// minus the floor's, face by face. Unlike a divergence-theorem sum over the
/// whole mesh this needs no closed surface, so it stays exact for a solid
/// whose sides are open (see [`open_edge_count`]).
fn volume_between(lower: &(Vec<mesh_data::Vertex>, Vec<[u32; 3]>), upper: &(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)) -> f64 {
    let origin_z = lower.0.first().map_or(0.0, |vertex| vertex.z);
    fn height_integral((vertices, faces): &(Vec<mesh_data::Vertex>, Vec<[u32; 3]>), origin_z: f64) -> f64 {
        faces
            .iter()
            .map(|face| {
                let corners = face.map(|index| vertices[index as usize]);
                let area = ((corners[1].x - corners[0].x) * (corners[2].y - corners[0].y) - (corners[1].y - corners[0].y) * (corners[2].x - corners[0].x)).abs() / 2.0;
                area * ((corners[0].z - origin_z) + (corners[1].z - origin_z) + (corners[2].z - origin_z)) / 3.0
            })
            .sum()
    }
    height_integral(upper, origin_z) - height_integral(lower, origin_z)
}

/// How many edges of the mesh are not shared by exactly two faces, once
/// coincident vertices are welded.
///
/// A closed solid has none. Ones that do appear are the sides: where the
/// region ends because a surface simply runs out - a design that stops short
/// of the ground rather than meeting it - the floor and the roof end at
/// different heights with nothing between them. The volume is still exact
/// there, but the mesh is a shell rather than a solid, which is worth saying.
pub(crate) fn open_edge_count(vertices: &[mesh_data::Vertex], faces: &[[u32; 3]]) -> usize {
    use std::collections::HashMap;
    let scale = weld_scale(vertices);
    let key = |index: u32| point_key_scaled(vertices[index as usize], scale);
    let mut counts: HashMap<EdgeKey, usize> = HashMap::new();
    for face in faces {
        for (a, b) in [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])] {
            let (a, b) = (key(a), key(b));
            *counts.entry(if a <= b { (a, b) } else { (b, a) }).or_default() += 1;
        }
    }
    counts.values().filter(|count| **count != 2).count()
}

impl App<'_> {
    /// Create a new closed solid from a design surface and the topography it
    /// meets, as the Triangulation menu's own tool. The build runs on a
    /// worker; the result arrives as a normal generated triangulation.
    pub(crate) fn create_solid_from_surfaces(&mut self, design_id: TriangulationId, topography_id: TriangulationId, region: SolidRegion, name: String) -> Result<()> {
        if design_id == topography_id {
            anyhow::bail!("A solid needs two different surfaces");
        }
        let (design_mesh, design_name) = {
            let design = self
                .triangulations
                .iter()
                .find(|triangulation| triangulation.id == design_id)
                .ok_or_else(|| anyhow::anyhow!("Design surface not found"))?;
            (design.mesh.clone(), design.name.clone())
        };
        let (topography_mesh, topography_name) = {
            let topography = self
                .triangulations
                .iter()
                .find(|triangulation| triangulation.id == topography_id)
                .ok_or_else(|| anyhow::anyhow!("Topography surface not found"))?;
            (topography.mesh.clone(), topography.name.clone())
        };

        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> Result<crate::model::triangulation::GeneratedTriangulationLog> {
            let (vertices, faces, volume) = build_solid_between_surfaces(&design_mesh, &topography_mesh, region, &progress.phase(0.0, 0.85))?;
            if cancel.is_cancelled() {
                anyhow::bail!("Cancelled");
            }
            let generated = session::build_generated_triangulation(name, vertices, faces, TriSurfaceType::SolidClosed, crate::model::triangulation::unique_edges)?;
            Ok(crate::model::triangulation::GeneratedTriangulationLog {
                generated,
                message: tr!(
                    "tri-built-solid-between",
                    region = (region.label().to_lowercase()).to_string(),
                    design = design_name.to_string(),
                    topography = topography_name.to_string(),
                    volume = format!("{volume:.1}")
                ),
            })
        };
        let apply = move |app: &mut App, result: Result<crate::model::triangulation::GeneratedTriangulationLog>| {
            app.apply_generated_triangulation_job(result, &[]);
        };
        self.spawn_job_reporting_progress(
            tr!("tri-building-solid-surfaces"),
            vec![crate::app::jobs::JobKey::Triangulation(design_id), crate::app::jobs::JobKey::Triangulation(topography_id)],
            compute,
            apply,
        );
        Ok(())
    }
}

/// One generated slab: its welded vertices, and the faces indexing them.
pub(crate) type Slab = (Vec<mesh_data::Vertex>, Vec<[u32; 3]>);

/// Cut closed slabs out of a solid between successive elevations - the benches
/// of a pit, or the flitches inside one.
///
/// A plain Z clip of a solid gives back only the *surfaces* that fall in the
/// band: for a pit that is a thin ring of wall, which reads as nothing at all
/// next to the lid at the top. A slab is that ring capped at both elevations,
/// so it is a solid in its own right - which is also what a bench or flitch
/// has to be to carry a tonnage later.
///
/// The caps are the solid's own cross-sections: the clip leaves an open rim
/// lying exactly on each plane, and those rims are traced into rings and
/// filled. A solid that is open at the sides (see the warning
/// [`build_solid_between_surfaces`] emits) has rims that do not close, and
/// those are left uncapped rather than guessed at.
///
/// Bands are expected in ascending order and must not overlap. A face only
/// touches the bands its own height range reaches, so cutting every bench of a
/// pit at once costs one walk of the mesh rather than one per bench.
pub(crate) fn slabs_between_elevations(mesh: &mesh_data::Triangulation, bands: &[(f64, f64)], cancel: Option<&crate::app::jobs::CancelFlag>) -> Result<Vec<Slab>> {
    if bands.iter().any(|(base, top)| !base.is_finite() || !top.is_finite() || base >= top) || bands.windows(2).any(|pair| pair[0].1 > pair[1].0 + 1e-6) {
        anyhow::bail!("Bench bands must be finite, ascending and non-overlapping");
    }
    let mut slabs: Vec<Slab> = vec![(Vec::new(), Vec::new()); bands.len()];
    let source = mesh.vertices();
    for (face_index, face) in mesh.face_vertex_indices_iter().enumerate() {
        if face_index % 1024 == 0 && cancel.is_some_and(|cancel| cancel.is_cancelled()) {
            anyhow::bail!("Cancelled");
        }
        let triangle = [source[face[0]], source[face[1]], source[face[2]]];
        let lowest = triangle[0].z.min(triangle[1].z).min(triangle[2].z);
        let highest = triangle[0].z.max(triangle[1].z).max(triangle[2].z);
        let first = bands.partition_point(|(_, top)| *top < lowest);
        for (index, (base, top)) in bands.iter().enumerate().skip(first).take_while(|(_, (base, _))| *base <= highest) {
            if top <= base || highest < *base || lowest > *top {
                continue;
            }
            // A horizontal boundary face belongs only to the slab on its
            // material side. Copying it into both bands creates a zero-thickness
            // sheet which cap triangulation can cover a second time.
            if highest - lowest < 1e-8 {
                let normal_z = (triangle[1].x - triangle[0].x) * (triangle[2].y - triangle[0].y) - (triangle[1].y - triangle[0].y) * (triangle[2].x - triangle[0].x);
                if ((lowest - base).abs() < 1e-8 && normal_z > 0.0) || ((highest - top).abs() < 1e-8 && normal_z < 0.0) {
                    continue;
                }
            }
            let slab = &mut slabs[index];
            for clipped in super::cuts::clip_triangle_z(triangle, *base, *top) {
                if collapsed(&clipped) {
                    continue;
                }
                let vertex = slab.0.len() as u32;
                slab.0.extend_from_slice(&clipped);
                slab.1.push([vertex, vertex + 1, vertex + 2]);
            }
        }
    }
    for (slab, (base, top)) in slabs.iter_mut().zip(bands) {
        if slab.1.is_empty() {
            continue;
        }
        if cancel.is_some_and(|cancel| cancel.is_cancelled()) {
            anyhow::bail!("Cancelled");
        }
        cap_slab(slab, *base, *top);
    }
    Ok(slabs)
}

/// Whether a clipped triangle has two corners at one point, and so covers
/// nothing its neighbours do not already meet across.
///
/// Not whether its area is small: a sliver cut off along a plane has long
/// sides, and its neighbours meet it along them. Dropping it leaves a crack
/// in the body, and a rim through the crack that no cap can close.
fn collapsed(triangle: &[mesh_data::Vertex; 3]) -> bool {
    let [a, b, c] = triangle.map(|p| glam::DVec3::new(p.x, p.y, p.z));
    a.distance_squared(b).min(b.distance_squared(c)).min(c.distance_squared(a)) <= 1e-18
}

/// Fill the open rims a slab was cut at, turning the band of surface into a
/// solid.
fn cap_slab(slab: &mut (Vec<mesh_data::Vertex>, Vec<[u32; 3]>), base: f64, top: f64) {
    let weld = Weld::of(&slab.0);
    // Only points on a cut plane can be a rim's, and only faces with two of
    // their corners on one can own a rim edge - every face using an edge in
    // the plane has both its ends there. Looking at those alone keeps a cut
    // through a large body from hashing every face of it. Each point is
    // classed once: 1 on the base, 2 on the top, 0 on neither.
    let plane_of: Vec<u8> = slab
        .0
        .iter()
        .map(|vertex| {
            if (vertex.z - base).abs() <= weld.tolerance {
                1
            } else if (vertex.z - top).abs() <= weld.tolerance {
                2
            } else {
                0
            }
        })
        .collect();
    let edge_faces: Vec<[u32; 3]> = slab
        .1
        .iter()
        .copied()
        .filter(|face| {
            let [a, b, c] = face.map(|index| plane_of[index as usize]);
            (a != 0 && (a == b || a == c)) || (b != 0 && b == c)
        })
        .collect();
    let mut rims = boundary_segments(&slab.0, &edge_faces);
    // Points are merged only where the rims do not already close. A cut can
    // leave distinct rim points a hair apart, and merging those pinches a
    // closed rim into one that no longer traces as a ring - the cap is lost
    // and every later cut through the body inherits the hole.
    if !rims_close(&rims, &[base, top], weld) {
        let planar: Vec<usize> = (0..slab.0.len()).filter(|&index| plane_of[index] != 0).collect();
        merge_coincident(&mut slab.0, &planar, weld);
        rims = boundary_segments(&slab.0, &edge_faces);
    }
    let sides = cap_sides(&slab.0, &edge_faces, weld);
    for (plane, upwards) in [(base, false), (top, true)] {
        let rim: Vec<[mesh_data::Vertex; 2]> = rims
            .iter()
            .copied()
            .filter(|[a, b]| (a.z - plane).abs() <= weld.tolerance && (b.z - plane).abs() <= weld.tolerance)
            .collect();
        if rim.is_empty() {
            continue;
        }
        let rings = planar_cap_rings(&rim, weld);
        let capped: Vec<bool> = rings.iter().map(|ring| ring_is_material(ring, &sides, weld)).collect();
        append_caps(&rings, &capped, plane, upwards, &mut slab.0, &mut slab.1);
    }
}

/// Whether every rim point on `planes` meets an even number of rim edges,
/// so the rims there are closed loops.
fn rims_close(rims: &[[mesh_data::Vertex; 2]], planes: &[f64], weld: Weld) -> bool {
    use std::collections::HashMap;
    let mut degree: HashMap<PointKey, usize, foldhash::fast::RandomState> = HashMap::default();
    for [a, b] in rims {
        if !planes.iter().any(|plane| (a.z - plane).abs() <= weld.tolerance && (b.z - plane).abs() <= weld.tolerance) {
            continue;
        }
        let (ka, kb) = (weld.key(*a), weld.key(*b));
        if ka == kb {
            continue;
        }
        *degree.entry(ka).or_default() += 1;
        *degree.entry(kb).or_default() += 1;
    }
    degree.values().all(|count| count % 2 == 0)
}

/// Move each of `indices`' points onto the first one within the weld's
/// tolerance of it.
///
/// Welding rounds points onto a grid, and two points a rounding error apart
/// can land either side of a grid line: the same crossing reached along two
/// collinear edges does, and the slab then has a crack there and its cap a
/// rim that never closes. Merging by distance first means the grid only ever
/// sees identical points.
fn merge_coincident(vertices: &mut [mesh_data::Vertex], indices: &[usize], weld: Weld) {
    use std::collections::HashMap;
    // Cells two tolerances wide, so the points within tolerance of one lie in
    // at most two cells along each axis rather than three.
    let cell = move |value: f64| (value * weld.scale / 2.0).floor() as i64;
    let span = move |value: f64| cell(value - weld.tolerance)..=cell(value + weld.tolerance);
    let mut cells: HashMap<PointKey, Vec<mesh_data::Vertex>, foldhash::fast::RandomState> = HashMap::default();
    for &index in indices {
        let point = vertices[index];
        let found = span(point.x)
            .flat_map(|x| span(point.y).flat_map(move |y| span(point.z).map(move |z| (x, y, z))))
            .filter_map(|key| cells.get(&key))
            .flatten()
            .find(|kept| weld.same(**kept, point))
            .copied();
        match found {
            Some(kept) => vertices[index] = kept,
            None => cells.entry((cell(point.x), cell(point.y), cell(point.z))).or_default().push(point),
        }
    }
}

/// Rim edges keyed by their welded ends, each with the direction that has the
/// cap on its left: see [`cap_sides`].
type CapSides = std::collections::HashMap<EdgeKey, (PointKey, PointKey), foldhash::fast::RandomState>;

/// For each rim edge - an edge only one face uses - the direction along it
/// that has the solid, and so the cap, on its left.
///
/// The face that owns the edge says so without looking anywhere else: the
/// solid is on the side its outward normal points away from, read in plan; a
/// face lying flat in the plane has the cap on the other side of it, since
/// one cannot cover the other.
fn cap_sides(vertices: &[mesh_data::Vertex], faces: &[[u32; 3]], weld: Weld) -> CapSides {
    use std::collections::HashMap;
    let mut owners: HashMap<EdgeKey, (usize, PointKey, PointKey, bool), foldhash::fast::RandomState> = HashMap::default();
    for face in faces {
        let [a, b, c] = face.map(|index| vertices[index as usize]);
        let normal = glam::DVec3::new(b.x - a.x, b.y - a.y, b.z - a.z).cross(glam::DVec3::new(c.x - a.x, c.y - a.y, c.z - a.z));
        for (from, to, other) in [(a, b, c), (b, c, a), (c, a, b)] {
            let (kf, kt) = (weld.key(from), weld.key(to));
            let span = glam::DVec2::new(to.x - from.x, to.y - from.y);
            // Left of from -> to is where the solid is when the normal leans
            // the other way, or, lying flat, where this face is not.
            let flat = normal.truncate().length_squared() <= 1e-12 * normal.length_squared();
            let left = if flat {
                span.perp_dot(glam::DVec2::new(other.x - from.x, other.y - from.y)) < 0.0
            } else {
                span.perp_dot(-normal.truncate()) > 0.0
            };
            let key = if kf <= kt { (kf, kt) } else { (kt, kf) };
            owners.entry(key).and_modify(|entry| entry.0 += 1).or_insert((1, kf, kt, left));
        }
    }
    owners
        .into_iter()
        .filter(|(_, (count, ..))| *count == 1)
        .map(|(key, (_, from, to, left))| (key, if left { (from, to) } else { (to, from) }))
        .collect()
}

/// Whether a traced cap ring encloses solid: its rim edges, walked with the
/// ring's inside on their left, mostly agree with [`cap_sides`].
fn ring_is_material(ring: &[mesh_data::Vertex], sides: &CapSides, weld: Weld) -> bool {
    let mut agree = 0usize;
    let mut disagree = 0usize;
    for (index, point) in ring.iter().enumerate() {
        let (from, to) = (weld.key(*point), weld.key(ring[(index + 1) % ring.len()]));
        let key = if from <= to { (from, to) } else { (to, from) };
        match sides.get(&key) {
            Some(direction) if *direction == (from, to) => agree += 1,
            Some(_) => disagree += 1,
            None => {}
        }
    }
    agree > disagree
}

/// The edges of a triangle soup that only one face uses, welded by position.
fn boundary_segments(vertices: &[mesh_data::Vertex], faces: &[[u32; 3]]) -> Vec<[mesh_data::Vertex; 2]> {
    use std::collections::HashMap;
    let weld = Weld::of(vertices);
    let key = |vertex: mesh_data::Vertex| weld.key(vertex);
    let mut counts: HashMap<EdgeKey, (usize, [mesh_data::Vertex; 2]), foldhash::fast::RandomState> = HashMap::default();
    for face in faces {
        let corners = face.map(|index| vertices[index as usize]);
        for (a, b) in [(corners[0], corners[1]), (corners[1], corners[2]), (corners[2], corners[0])] {
            let (ka, kb) = (key(a), key(b));
            let entry = counts.entry(if ka <= kb { (ka, kb) } else { (kb, ka) }).or_insert((0, [a, b]));
            entry.0 += 1;
        }
    }
    counts.into_values().filter(|(count, _)| *count == 1).map(|(_, segment)| segment).collect()
}

/// Trace the rings bounding a closed slab's plan footprint - every XY point
/// a vertical ray through the slab hits, flattened onto `plane`.
///
/// Only the upward-facing faces are used, and they are exactly the footprint:
/// every point with material below it has one topmost boundary above it, so
/// the roof covers the footprint once and only once. That holds whether the
/// roof is a horizontal cut cap or the topography itself, which is why this
/// works for the top bench of a pit, where there is no cap to trace, and why
/// it beats the cap elsewhere - ground under a dip in the topography never
/// reaches the band top but is still ground.
///
/// Vertical walls project to slivers and are dropped; their edges are already
/// shared with the roof and floor they join.
pub(crate) fn plan_footprint_rings(mesh: &mesh_data::Triangulation, plane: f64) -> Vec<Vec<mesh_data::Vertex>> {
    let source = mesh.vertices();
    if source.is_empty() {
        return Vec::new();
    }
    let sliver = Weld::of(source).tolerance.powi(2);
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for face in mesh.face_vertex_indices_iter() {
        let [a, b, c] = face.map(|index| source[index]);
        // Counter-clockwise in XY is an outward normal pointing up. A wall
        // that only rounding keeps off vertical leans either way at random and
        // projects to a line; counted, it laid stray edges along the outline.
        let plan = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        let full = glam::DVec3::new(b.x - a.x, b.y - a.y, b.z - a.z)
            .cross(glam::DVec3::new(c.x - a.x, c.y - a.y, c.z - a.z))
            .length();
        if plan <= sliver.max(full * VERTICAL_WALL_RATIO) {
            continue;
        }
        let base = vertices.len() as u32;
        vertices.extend([a, b, c].map(|vertex| mesh_data::Vertex::new(vertex.x, vertex.y, plane)));
        faces.push([base, base + 1, base + 2]);
    }
    if faces.is_empty() {
        return Vec::new();
    }
    // Flattening first makes the shared 3D weld an XY weld, so the roof's
    // interior edges cancel and only its outline is left. Points a rounding
    // error apart are merged before that, or the outline cracks between them.
    let weld = Weld::of(&vertices);
    let all: Vec<usize> = (0..vertices.len()).collect();
    merge_coincident(&mut vertices, &all, weld);
    // Ground often pinches to a point - two pieces of a bench touching at a
    // corner - so the outline is traced the way a cap is, past nodes where
    // more than two of its edges meet, rather than abandoned there.
    planar_cap_rings(&boundary_segments(&vertices, &faces), weld)
}

/// A face whose plan area is this small a fraction of its own is a vertical
/// wall that rounding tilted: steeper than 89.9999°.
const VERTICAL_WALL_RATIO: f64 = 1.0e-6;

/// How many other rings each ring sits inside, which says which rings are
/// nested directly in which.
///
/// Judged by a point strictly inside each ring, never by one of its corners:
/// the rings of a cut plane often meet at a corner - a sliver of ground
/// pinched off the main outline - and a corner on another ring's edge reads as
/// inside it or not by rounding alone, which filled holes and left ground
/// uncapped.
fn ring_depths(rings: &[Vec<mesh_data::Vertex>], probes: &[Option<mesh_data::Vertex>]) -> Vec<usize> {
    // A ring's box rules most others out before the crossing test walks it.
    let boxes: Vec<_> = rings.iter().map(|ring| ring_box(ring)).collect();
    probes
        .iter()
        .enumerate()
        .map(|(index, probe)| {
            let Some(probe) = probe else { return 0 };
            rings
                .iter()
                .enumerate()
                .filter(|(other, candidate)| *other != index && box_holds(boxes[*other], *probe) && ring_contains(candidate, *probe))
                .count()
        })
        .collect()
}

/// A ring's plan bounds: min x, min y, max x, max y.
fn ring_box(ring: &[mesh_data::Vertex]) -> [f64; 4] {
    ring.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, p| {
        [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)]
    })
}

fn box_holds(bounds: [f64; 4], point: mesh_data::Vertex) -> bool {
    point.x >= bounds[0] && point.y >= bounds[1] && point.x <= bounds[2] && point.y <= bounds[3]
}

/// A point strictly inside a ring: the centre of the largest triangle of its
/// own triangulation. `None` for a ring with no area.
fn interior_point(ring: &[mesh_data::Vertex]) -> Option<mesh_data::Vertex> {
    if ring.len() < 3 {
        return None;
    }
    let origin = ring[0];
    let mut indices: Vec<usize> = Vec::new();
    earcut::Earcut::new().earcut(ring.iter().map(|point| [point.x - origin.x, point.y - origin.y]), &[], &mut indices);
    indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|triangle| triangle.map(|corner| ring[corner]))
        .max_by(|a, b| triangle_plan_area(*a).total_cmp(&triangle_plan_area(*b)))
        .filter(|triangle| triangle_plan_area(*triangle) > 0.0)
        .map(|[a, b, c]| mesh_data::Vertex::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0, a.z))
}

fn triangle_plan_area([a, b, c]: [mesh_data::Vertex; 3]) -> f64 {
    ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs()
}

/// Even-odd crossing test in XY, shared by the cap filler and the blast
/// outlines so both agree on what is inside a ring.
pub(crate) fn ring_contains(ring: &[mesh_data::Vertex], point: mesh_data::Vertex) -> bool {
    let mut inside = false;
    for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()) {
        if (a.y > point.y) != (b.y > point.y) && point.x < a.x + (point.y - a.y) * (b.x - a.x) / (b.y - a.y) {
            inside = !inside;
        }
    }
    inside
}

/// Fill the rings that enclose solid, each less the rings nested directly
/// inside it - filled in turn if they are solid too, so an island in a hole
/// is capped and the hole is not.
fn append_caps(rings: &[Vec<mesh_data::Vertex>], capped: &[bool], plane: f64, upwards: bool, vertices: &mut Vec<mesh_data::Vertex>, faces: &mut Vec<[u32; 3]>) {
    let contains = ring_contains;
    // Which outer a hole belongs to is judged the same way as its depth. A
    // lone ring - most caps - neither sits in another nor holds one, and
    // finding a point inside it means triangulating it a second time.
    let probes: Vec<_> = if rings.len() > 1 {
        rings.iter().map(|ring| interior_point(ring)).collect()
    } else {
        vec![None; rings.len()]
    };
    let depths = ring_depths(rings, &probes);
    for (index, outer) in rings.iter().enumerate() {
        if outer.len() < 3 || !capped[index] {
            continue;
        }
        let outer_box = ring_box(outer);
        let mut cap = CapPolygon::default();
        if !cap.add_ring(outer, false) {
            continue;
        }
        for (j, hole) in rings.iter().enumerate() {
            if depths[j] == depths[index] + 1 && probes[j].is_some_and(|probe| box_holds(outer_box, probe) && contains(outer, probe)) {
                cap.add_ring(hole, true);
            }
        }
        let indices = cap.delaunay().unwrap_or_else(|| {
            let mut indices = Vec::new();
            earcut::Earcut::new().earcut(cap.corners.iter().map(|&corner| [cap.points[corner].x, cap.points[corner].y]), &cap.holes, &mut indices);
            indices
        });
        let base = vertices.len() as u32;
        vertices.extend(cap.points.iter().map(|point| mesh_data::Vertex { x: point.x, y: point.y, z: plane }));
        let mut push = |face: [u32; 3], vertices: &[mesh_data::Vertex]| {
            let corners = face.map(|index| vertices[index as usize]);
            let points_up = (corners[1].x - corners[0].x) * (corners[2].y - corners[0].y) - (corners[1].y - corners[0].y) * (corners[2].x - corners[0].x) > 0.0;
            faces.push(if points_up == upwards { face } else { [face[0], face[2], face[1]] });
        };
        let mut outline = Vec::new();
        for triangle in indices.as_chunks::<3>().0 {
            let corners = triangle.map(|corner| cap.corners[corner]);
            // The ring points earcut skipped go back on the edges they lie
            // along, so the cap keeps every rim vertex and meets the walls
            // without a T-junction.
            outline.clear();
            for (from, to) in [(corners[0], corners[1]), (corners[1], corners[2]), (corners[2], corners[0])] {
                outline.push(from);
                if let Some(skipped) = cap.skipped.get(&(from, to)) {
                    outline.extend(skipped.iter().copied());
                } else if let Some(skipped) = cap.skipped.get(&(to, from)) {
                    outline.extend(skipped.iter().rev().copied());
                }
            }
            if outline.len() == 3 {
                push(corners.map(|corner| base + corner as u32), vertices);
                continue;
            }
            // Points on the triangle's edges: fan from its centroid, which
            // sees every one of them from strictly inside.
            let [a, b, c] = corners.map(|corner| cap.points[corner]);
            let centre = vertices.len() as u32;
            vertices.push(mesh_data::Vertex {
                x: (a.x + b.x + c.x) / 3.0,
                y: (a.y + b.y + c.y) / 3.0,
                z: plane,
            });
            for (position, &from) in outline.iter().enumerate() {
                let to = outline[(position + 1) % outline.len()];
                push([centre, base + from as u32, base + to as u32], vertices);
            }
        }
    }
}

/// How far a ring point may sit off the straight run through it and still be
/// left out of a cap's triangulation: a nanometre, far below anything drawn
/// or measured.
const COLLINEAR_TOLERANCE: f64 = 1.0e-9;

/// One cap's rings, with the corners earcut is given kept apart from the
/// points it is not.
///
/// A vertical cut through a flat, finely triangulated bench crosses its floor
/// and roof in long straight chains - a thousand ring points with a score of
/// corners among them - and earcut falls back to its quadratic search on
/// chains like that, taking a tenth of a second over a cap that has twenty
/// points' worth of shape. So it triangulates the corners alone, and
/// [`append_caps`] puts the skipped points back on the edges they lie along.
#[derive(Default)]
struct CapPolygon {
    /// Every ring point, in ring order.
    points: Vec<mesh_data::Vertex>,
    /// Indices into `points` of each ring's corners, rings one after another.
    corners: Vec<usize>,
    /// Where each hole's corners start in `corners`, as earcut wants.
    holes: Vec<usize>,
    /// The points skipped between two consecutive corners, keyed by those
    /// corners in ring order.
    skipped: HashMap<(usize, usize), Vec<usize>, foldhash::fast::RandomState>,
}

impl CapPolygon {
    /// The corners triangulated as earcut would - indices into `corners`,
    /// three to a triangle - but Delaunay, so no triangle is a sliver.
    ///
    /// Earcut cuts ears off the outline, and across a wide cap - the floor of
    /// a flitch, a kilometre over - its ears are long slivers from one side to
    /// the other. Every later cut through the cap splits each sliver it
    /// crosses, so a block cut out of a flitch came away with hundreds of
    /// pieces of them; Delaunay triangles cross a block a few at a time.
    ///
    /// `None` where the rings are not a clean set of constraints - a ring that
    /// touches itself, or another, at a point or along an edge - and earcut,
    /// which takes those as they are, is used instead.
    fn delaunay(&self) -> Option<Vec<usize>> {
        use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation as _};

        let mut cdt: ConstrainedDelaunayTriangulation<Point2<f64>> = ConstrainedDelaunayTriangulation::new();
        let mut handles = Vec::with_capacity(self.corners.len());
        let mut corner_of: Vec<Option<usize>> = Vec::new();
        for (position, &corner) in self.corners.iter().enumerate() {
            let point = self.points[corner];
            let handle = cdt.insert(Point2::new(point.x, point.y)).ok()?;
            if handle.index() >= corner_of.len() {
                corner_of.resize(handle.index() + 1, None);
            }
            if corner_of[handle.index()].replace(position).is_some() {
                return None;
            }
            handles.push(handle);
        }
        let mut start = 0;
        for end in self.holes.iter().copied().chain([self.corners.len()]) {
            for index in start..end {
                let (from, to) = (handles[index], handles[if index + 1 == end { start } else { index + 1 }]);
                if !cdt.can_add_constraint(from, to) || !cdt.add_constraint(from, to) {
                    return None;
                }
            }
            start = end;
        }

        // Inside is an odd number of rings crossed from outside the hull.
        let mut inside: Vec<Option<bool>> = vec![None; cdt.num_all_faces()];
        let mut pending = Vec::new();
        for edge in cdt.convex_hull() {
            if let Some(face) = edge.rev().face().as_inner()
                && inside[face.index()].is_none()
            {
                inside[face.index()] = Some(edge.is_constraint_edge());
                pending.push(face.fix());
            }
        }
        while let Some(face) = pending.pop() {
            let parity = inside[face.index()].expect("set when queued");
            for edge in cdt.face(face).adjacent_edges() {
                if let Some(neighbour) = edge.rev().face().as_inner()
                    && inside[neighbour.index()].is_none()
                {
                    inside[neighbour.index()] = Some(parity != edge.is_constraint_edge());
                    pending.push(neighbour.fix());
                }
            }
        }
        let mut indices = Vec::new();
        for face in cdt.inner_faces() {
            if inside[face.index()] == Some(true) {
                for vertex in face.vertices() {
                    indices.push(corner_of[vertex.fix().index()]?);
                }
            }
        }
        Some(indices)
    }

    /// Whether the ring had enough shape to add.
    ///
    /// A skipped point is within [`COLLINEAR_TOLERANCE`] of the edge that
    /// replaces it, not merely of its own two neighbours, so a gentle curve
    /// cannot be straightened a nanometre at a time. The walk starts at the
    /// lowest point in (x, y) order, which is never strictly between its
    /// neighbours on a line.
    fn add_ring(&mut self, ring: &[mesh_data::Vertex], hole: bool) -> bool {
        let count = ring.len();
        if count < 3 {
            return false;
        }
        let start = (0..count)
            .min_by(|&a, &b| ring[a].x.total_cmp(&ring[b].x).then(ring[a].y.total_cmp(&ring[b].y)))
            .unwrap_or(0);
        let at = |step: usize| ring[(start + step) % count];
        // Steps round the ring, from `start`, of the points kept as corners.
        let mut kept = vec![0];
        let mut anchor = 0;
        while anchor < count {
            // Stretch the edge from `anchor` for as long as every point it
            // skips stays on it; step `count` is the start again. Each
            // skipped point allows the edge only the directions that pass
            // within the tolerance of it, so the directions still allowed
            // are kept as one range, narrowed point by point: one test per
            // point rather than every skipped point again at each step.
            let from = glam::DVec2::new(at(anchor).x, at(anchor).y);
            let offset = |step: usize| glam::DVec2::new(at(step).x, at(step).y) - from;
            let reference = offset(anchor + 1).try_normalize().unwrap_or(glam::DVec2::X);
            let angle = |vector: glam::DVec2| reference.perp_dot(vector).atan2(reference.dot(vector));
            let (mut low, mut high) = (f64::NEG_INFINITY, f64::INFINITY);
            let mut end = anchor + 1;
            while end < count {
                let skipped = offset(end);
                let reach = skipped.length();
                // A point this close to the anchor lies on every line through it.
                let (narrow_low, narrow_high) = if reach <= COLLINEAR_TOLERANCE {
                    (low, high)
                } else {
                    let spread = (COLLINEAR_TOLERANCE / reach).asin();
                    let direction = angle(skipped);
                    (low.max(direction - spread), high.min(direction + spread))
                };
                let next = offset(end + 1);
                let direction = angle(next);
                if next.length() <= COLLINEAR_TOLERANCE || direction < narrow_low || direction > narrow_high {
                    break;
                }
                (low, high) = (narrow_low, narrow_high);
                end += 1;
            }
            if end < count {
                kept.push(end);
            }
            anchor = end;
        }
        if kept.len() < 3 {
            return false;
        }
        let base = self.points.len();
        self.points.extend((0..count).map(at));
        if hole {
            self.holes.push(self.corners.len());
        }
        self.corners.extend(kept.iter().map(|&step| base + step));
        for (position, &from) in kept.iter().enumerate() {
            let to = kept.get(position + 1).copied().unwrap_or(count);
            if to > from + 1 {
                self.skipped.insert((base + from, base + to % count), (from + 1..to).map(|step| base + step).collect());
            }
        }
        true
    }
}

/// Close the sides of the region with a vertical wall between the two sheets.
///
/// The floor and roof meet on their own along the line where the surfaces
/// cross - both clips stop there, at the same heights - but nowhere else. Where
/// the region ends because a surface simply runs out inside the other's
/// footprint, the two sheets end one above the other with a gap between them,
/// and that gap is what this fills.
///
/// Both sheets cover the same XY region, so their rims trace the same closed
/// path with different tessellations. The wall is built on the merge of the
/// two: every rim vertex of either sheet becomes a wall vertex, so the wall's
/// edges line up with the sheet edges on both sides and the result is closed
/// rather than merely gapless.
fn close_region_sides(floor: &(Vec<mesh_data::Vertex>, Vec<[u32; 3]>), roof: &(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    // One weld for both sheets: they are welded against each other, so they
    // cannot each pick their own grid from their own extent.
    let weld = Weld::of(&floor.0);
    let floor_rings = boundary_rings(floor, weld);
    let mut roof_rings = boundary_rings(roof, weld);
    if floor_rings.is_empty() || roof_rings.is_empty() {
        return (vertices, faces);
    }
    let outward = outward_lookup(floor, weld);

    for floor_ring in &floor_rings {
        // The rims trace the same path, so the roof ring that belongs with
        // this one is the one that runs closest to it.
        let Some(index) = nearest_ring(floor_ring, &roof_rings) else {
            continue;
        };
        let roof_ring = roof_rings.remove(index);
        append_wall(floor_ring, &roof_ring, &outward, weld, &mut vertices, &mut faces);
    }
    (vertices, faces)
}

/// The rim of a sheet, as closed rings of points.
fn boundary_rings(sheet: &(Vec<mesh_data::Vertex>, Vec<[u32; 3]>), weld: Weld) -> Vec<Vec<mesh_data::Vertex>> {
    let segments = boundary_segments(&sheet.0, &sheet.1);
    if segments.is_empty() {
        return Vec::new();
    }
    let mut rings = closed_boundary_rings(&segments, weld);
    for ring in &mut rings {
        // A ring that repeats its first point closes itself; the wall walks
        // the loop and would emit a zero-width quad on that repeat.
        if ring.len() > 1 && weld.same_xy(ring[0], ring[ring.len() - 1]) {
            ring.pop();
        }
    }
    rings.retain(|ring| ring.len() >= 3);
    rings
}

/// Trace the rings bounding a planar rim, getting past nodes where more than
/// two rim edges meet.
///
/// [`closed_boundary_rings`] abandons a ring at any such node, which is right
/// when building a solid - bridging an ambiguous rim would invent geometry -
/// but wrong for capping a cut. A vertical cut through a bench slab reaches
/// those nodes routinely: the cross-section pinches to a point, two pieces
/// touch at a corner, or independently clipped triangles leave a T-junction.
/// Giving up there left the cell uncapped, so it stayed open and its volume
/// leaked out of the total.
///
/// Turning the sharpest way available at every node walks the faces of the
/// planar subdivision the rim draws. Each rim ring is then walked once in each
/// direction, so keeping the positive-turning walks yields every ring exactly
/// once - outer rings and hole rings alike, which is what [`append_caps`] wants
/// before sorting them by nesting.
fn planar_cap_rings(segments: &[[mesh_data::Vertex; 2]], weld: Weld) -> Vec<Vec<mesh_data::Vertex>> {
    use std::collections::{HashMap, HashSet};

    let mut points: HashMap<PointKey, mesh_data::Vertex, foldhash::fast::RandomState> = HashMap::default();
    let mut adjacency: HashMap<PointKey, Vec<PointKey>, foldhash::fast::RandomState> = HashMap::default();
    for [a, b] in segments {
        let (ka, kb) = (weld.key(*a), weld.key(*b));
        if ka == kb {
            continue;
        }
        points.insert(ka, *a);
        points.insert(kb, *b);
        for (from, to) in [(ka, kb), (kb, ka)] {
            let neighbours = adjacency.entry(from).or_default();
            // A rim edge repeated by two faces is one edge of the outline.
            if !neighbours.contains(&to) {
                neighbours.push(to);
            }
        }
    }

    let angle = |from: PointKey, to: PointKey| {
        let (origin, target) = (points[&from], points[&to]);
        (target.y - origin.y).atan2(target.x - origin.x)
    };
    for (key, neighbours) in &mut adjacency {
        neighbours.sort_by(|a, b| angle(*key, *a).total_cmp(&angle(*key, *b)));
    }

    let mut directed: Vec<(PointKey, PointKey)> = adjacency.iter().flat_map(|(from, tos)| tos.iter().map(|to| (*from, *to))).collect();
    directed.sort_unstable();

    let mut visited: HashSet<(PointKey, PointKey), foldhash::fast::RandomState> = HashSet::default();
    let mut rings = Vec::new();
    for start in directed {
        if visited.contains(&start) {
            continue;
        }
        let mut ring: Vec<mesh_data::Vertex> = Vec::new();
        let mut edge = start;
        while visited.insert(edge) {
            let (from, to) = edge;
            ring.push(points[&from]);
            let neighbours = &adjacency[&to];
            // Step back the way we came, then take the next neighbour turning
            // one way round. At a plain degree-two node that is simply the
            // other edge, so a clean rim traces exactly as it did before.
            let incoming = neighbours.iter().position(|neighbour| *neighbour == from).expect("edges are added both ways");
            edge = (to, neighbours[(incoming + neighbours.len() - 1) % neighbours.len()]);
        }
        if ring.len() >= 3 && ring_signed_area(&ring) > 0.0 {
            rings.push(ring);
        }
    }
    rings
}

/// Twice the signed area of a ring in the plane it was traced in.
fn ring_signed_area(ring: &[mesh_data::Vertex]) -> f64 {
    (0..ring.len())
        .map(|index| {
            let (a, b) = (ring[index], ring[(index + 1) % ring.len()]);
            a.x * b.y - b.x * a.y
        })
        .sum()
}

/// Trace only closed, unambiguous loops. The general include tool bridges
/// open contours with chords; manufacturing those edges is unsafe for solids.
fn closed_boundary_rings(segments: &[[mesh_data::Vertex; 2]], weld: Weld) -> Vec<Vec<mesh_data::Vertex>> {
    use std::collections::{HashMap, HashSet};
    let mut nodes: HashMap<PointKey, (mesh_data::Vertex, Vec<PointKey>)> = HashMap::new();
    for [a, b] in segments {
        let (ka, kb) = (weld.key(*a), weld.key(*b));
        if ka == kb {
            continue;
        }
        nodes.entry(ka).or_insert((*a, Vec::new())).1.push(kb);
        nodes.entry(kb).or_insert((*b, Vec::new())).1.push(ka);
    }
    let mut starts: Vec<_> = nodes.keys().copied().collect();
    starts.sort_unstable();
    let mut visited = HashSet::new();
    let mut rings = Vec::new();
    for start in starts {
        if visited.contains(&start) {
            continue;
        }
        let mut ring = Vec::new();
        let mut previous = None;
        let mut current = start;
        loop {
            if !visited.insert(current) {
                if current == start && ring.len() >= 3 {
                    rings.push(ring);
                }
                break;
            }
            let (point, neighbors) = &nodes[&current];
            if neighbors.len() != 2 {
                break;
            }
            ring.push(*point);
            let next = if Some(neighbors[0]) == previous { neighbors[1] } else { neighbors[0] };
            previous = Some(current);
            current = next;
        }
    }
    rings
}

/// For each rim edge of a sheet, which horizontal side of it the sheet lies
/// on - keyed by the edge's welded endpoints.
///
/// The face that owns a rim edge has a third corner, and the sheet is on that
/// corner's side. That is what tells the wall which way is out, without having
/// to work out which rings are outer boundaries and which are holes.
fn outward_lookup(sheet: &(Vec<mesh_data::Vertex>, Vec<[u32; 3]>), weld: Weld) -> std::collections::HashMap<EdgeKey, bool> {
    use std::collections::HashMap;
    let point_key = |vertex| weld.key(vertex);
    let mut counts: HashMap<EdgeKey, (usize, bool)> = HashMap::new();
    for face in &sheet.1 {
        let corners = face.map(|index| sheet.0[index as usize]);
        for (a, b, c) in [
            (corners[0], corners[1], corners[2]),
            (corners[1], corners[2], corners[0]),
            (corners[2], corners[0], corners[1]),
        ] {
            let (ka, kb) = (point_key(a), point_key(b));
            // Keyed on the sorted pair, with the side recorded in the
            // direction the key is stored, so a lookup can undo the sort.
            let sorted = ka <= kb;
            let (first, second) = if sorted { (a, b) } else { (b, a) };
            let entry = counts.entry(if sorted { (ka, kb) } else { (kb, ka) }).or_insert((0, false));
            entry.0 += 1;
            entry.1 = left_of(first, second, c);
        }
    }
    counts.into_iter().filter(|(_, (count, _))| *count == 1).map(|(key, (_, left))| (key, left)).collect()
}

/// Whether `point` lies to the left of the line `from` -> `to`, in XY.
fn left_of(from: mesh_data::Vertex, to: mesh_data::Vertex, point: mesh_data::Vertex) -> bool {
    (to.x - from.x) * (point.y - from.y) - (to.y - from.y) * (point.x - from.x) > 0.0
}

type PointKey = (i64, i64, i64);

/// An edge, keyed by its two welded endpoints in sorted order.
type EdgeKey = (PointKey, PointKey);

/// Grid a set of points is welded on, in units per metre.
///
/// Account for a small number of f64 rounding steps without scaling tolerance
/// to centimetres at mine coordinates. Intersection construction is rebased
/// before clipping; this weld only absorbs residual roundoff.
fn weld_scale(vertices: &[mesh_data::Vertex]) -> f64 {
    let magnitude = vertices.iter().map(|vertex| vertex.x.abs().max(vertex.y.abs()).max(vertex.z.abs())).fold(0.0_f64, f64::max);
    // At least one micrometre, otherwise 64 machine epsilons at this magnitude.
    (1.0 / (magnitude * f64::EPSILON * 64.0).max(1e-6)).min(1e6)
}

/// The grid one solid's points are welded on, and the distance below which two
/// of them are the same point. Computed once per solid and carried, so every
/// step - the rim trace, the ring walk, the degenerate test - agrees about
/// which points coincide.
#[derive(Clone, Copy)]
struct Weld {
    scale: f64,
    tolerance: f64,
}

impl Weld {
    fn of(vertices: &[mesh_data::Vertex]) -> Self {
        let scale = weld_scale(vertices);
        Self { scale, tolerance: 1.0 / scale }
    }

    fn key(self, vertex: mesh_data::Vertex) -> PointKey {
        point_key_scaled(vertex, self.scale)
    }

    fn same_xy(self, a: mesh_data::Vertex, b: mesh_data::Vertex) -> bool {
        (a.x - b.x).abs() <= self.tolerance && (a.y - b.y).abs() <= self.tolerance
    }

    fn same(self, a: mesh_data::Vertex, b: mesh_data::Vertex) -> bool {
        self.same_xy(a, b) && (a.z - b.z).abs() <= self.tolerance
    }
}

fn point_key_scaled(vertex: mesh_data::Vertex, scale: f64) -> PointKey {
    ((vertex.x * scale).round() as i64, (vertex.y * scale).round() as i64, (vertex.z * scale).round() as i64)
}

/// Match XY footprints rather than centroids: concentric outer and hole
/// rings have the same centroid, but must never be joined to one another.
fn nearest_ring(ring: &[mesh_data::Vertex], candidates: &[Vec<mesh_data::Vertex>]) -> Option<usize> {
    let bounds = |ring: &[mesh_data::Vertex]| {
        ring.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, p| {
            [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)]
        })
    };
    let target = bounds(ring);
    let tolerance = Weld::of(ring).tolerance * 4.0;
    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, ring)| {
            let candidate = bounds(ring);
            let error = target.iter().zip(candidate).map(|(a, b)| (a - b).abs()).fold(0.0_f64, f64::max);
            (error <= tolerance).then_some((index, error))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
}

/// A closed ring parameterised by its XY arc length, so two tessellations of
/// the same path can be sampled against each other.
struct RingPath {
    points: Vec<mesh_data::Vertex>,
    /// Cumulative XY length at each point, normalised to `0.0..1.0`.
    params: Vec<f64>,
}

impl RingPath {
    fn new(points: &[mesh_data::Vertex]) -> Option<Self> {
        let mut lengths = Vec::with_capacity(points.len() + 1);
        let mut total = 0.0;
        lengths.push(0.0);
        for index in 0..points.len() {
            let a = points[index];
            let b = points[(index + 1) % points.len()];
            total += ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            lengths.push(total);
        }
        (total > 1e-9).then(|| Self {
            points: points.to_vec(),
            params: lengths.iter().map(|length| length / total).collect(),
        })
    }

    /// Signed XY area, whose sign is the direction the ring is traced in.
    fn signed_area(&self) -> f64 {
        let mut sum = 0.0;
        let origin = self.points[0];
        for index in 0..self.points.len() {
            let a = self.points[index];
            let b = self.points[(index + 1) % self.points.len()];
            sum += (a.x - origin.x) * (b.y - origin.y) - (b.x - origin.x) * (a.y - origin.y);
        }
        sum / 2.0
    }

    fn reverse(&mut self) {
        self.points.reverse();
        *self = Self::new(&self.points).unwrap_or_else(|| {
            std::mem::replace(
                self,
                Self {
                    points: Vec::new(),
                    params: Vec::new(),
                },
            )
        });
    }

    /// The parameter at which the ring passes closest to `target` in XY.
    fn nearest_param(&self, target: mesh_data::Vertex) -> f64 {
        let mut best = (f64::MAX, 0.0);
        for index in 0..self.points.len() {
            let a = self.points[index];
            let b = self.points[(index + 1) % self.points.len()];
            let (ax, ay) = (b.x - a.x, b.y - a.y);
            let length_sq = ax * ax + ay * ay;
            let t = if length_sq > 0.0 {
                (((target.x - a.x) * ax + (target.y - a.y) * ay) / length_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let distance = (target.x - (a.x + ax * t)).powi(2) + (target.y - (a.y + ay * t)).powi(2);
            if distance < best.0 {
                let span = self.params[index + 1] - self.params[index];
                best = (distance, self.params[index] + span * t);
            }
        }
        best.1
    }

    /// The point at a parameter, interpolating along the ring.
    fn sample(&self, param: f64) -> mesh_data::Vertex {
        let param = param.rem_euclid(1.0);
        let index = self.params.partition_point(|value| *value <= param).saturating_sub(1).min(self.points.len() - 1);
        let a = self.points[index];
        let b = self.points[(index + 1) % self.points.len()];
        let span = self.params[index + 1] - self.params[index];
        let t = if span > 0.0 { ((param - self.params[index]) / span).clamp(0.0, 1.0) } else { 0.0 };
        mesh_data::Vertex {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
            z: a.z + (b.z - a.z) * t,
        }
    }
}

/// Build the wall between one floor rim and the roof rim that matches it.
fn append_wall(
    floor_ring: &[mesh_data::Vertex],
    roof_ring: &[mesh_data::Vertex],
    outward: &std::collections::HashMap<EdgeKey, bool>,
    weld: Weld,
    vertices: &mut Vec<mesh_data::Vertex>,
    faces: &mut Vec<[u32; 3]>,
) {
    let (Some(floor), Some(mut roof)) = (RingPath::new(floor_ring), RingPath::new(roof_ring)) else {
        return;
    };
    // Walk both rims the same way round, from the same place, so the two
    // parameterisations describe the same journey.
    if floor.signed_area().is_sign_negative() != roof.signed_area().is_sign_negative() {
        roof.reverse();
    }
    let roof_offset = roof.nearest_param(floor.points[0]);

    // Which side of the rim the sheet lies on decides which way the wall
    // faces. It is the same all the way round a ring, so one edge answers it.
    let sheet_on_left = floor
        .points
        .iter()
        .enumerate()
        .find_map(|(index, point)| {
            let next = floor.points[(index + 1) % floor.points.len()];
            let (ka, kb) = (weld.key(*point), weld.key(next));
            let sorted = ka <= kb;
            let left = *outward.get(&if sorted { (ka, kb) } else { (kb, ka) })?;
            // The lookup recorded the side relative to the sorted direction,
            // so an edge walked the other way has the sheet on the other side.
            Some(if sorted { left } else { !left })
        })
        .unwrap_or(true);

    // Every rim vertex of either sheet becomes a wall vertex, so the wall's
    // edges match the sheet edges on both sides.
    let mut params: Vec<f64> = floor.params[..floor.points.len()].to_vec();
    // A roof vertex sits at its own parameter less the offset, since the roof
    // is walked from wherever it passes closest to the floor's own start.
    params.extend(roof.params[..roof.points.len()].iter().map(|param| (param - roof_offset).rem_euclid(1.0)));
    params.sort_by(f64::total_cmp);
    params.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    if params.len() < 2 {
        return;
    }

    let base = vertices.len() as u32;
    for param in &params {
        vertices.push(floor.sample(*param));
        vertices.push(roof.sample(param + roof_offset));
    }
    for index in 0..params.len() {
        let next = (index + 1) % params.len();
        let (floor_a, roof_a) = (base + index as u32 * 2, base + index as u32 * 2 + 1);
        let (floor_b, roof_b) = (base + next as u32 * 2, base + next as u32 * 2 + 1);
        // Where the two sheets already meet - along the line the surfaces
        // cross - there is no wall to build.
        let closed = weld.same(vertices[floor_a as usize], vertices[roof_a as usize]) && weld.same(vertices[floor_b as usize], vertices[roof_b as usize]);
        if closed {
            continue;
        }
        // Wound floor-then-roof along the direction of travel, a quad's
        // normal points to the right of it. That is out of the solid when the
        // sheet is on the left, and into it when the sheet is on the right.
        for face in [[floor_a, floor_b, roof_b], [floor_a, roof_b, roof_a]] {
            // Where the wall runs out to nothing - a corner at which the two
            // sheets meet - the quad degenerates into a sliver with two
            // corners in the same place. It has no area to contribute and its
            // repeated edge would read as a hole, so it is not emitted.
            let corners = face.map(|index| vertices[index as usize]);
            if weld.same(corners[0], corners[1]) || weld.same(corners[1], corners[2]) || weld.same(corners[2], corners[0]) {
                continue;
            }
            faces.push(if sheet_on_left { face } else { [face[0], face[2], face[1]] });
        }
    }
}

/// What clipping a solid to a plan polygon produced.
pub(crate) struct ClippedSolid {
    /// Vertices and faces of the clipped body.
    pub(crate) slab: Slab,
    /// The volume it encloses, summed per cell while cutting.
    pub(crate) volume: f64,
    /// Which faces are wall the clip cut along the polygon's own boundary,
    /// rather than the body's original surfaces or an internal wall between
    /// decomposition cells. Parallel to `slab.1`.
    pub(crate) boundary_wall: Vec<bool>,
    /// Which faces are wall the clip cut along a decomposition diagonal:
    /// inside the body, shared with the cell next door. Parallel to `slab.1`.
    pub(crate) internal_wall: Vec<bool>,
}

/// A body being cut down to a cell, with which of its faces are walls a cut
/// made along the polygon's own boundary, and which along an internal line.
/// Both masks are parallel to the slab's faces.
type CutPiece = (Slab, Vec<bool>, Vec<bool>);

/// One finished cell: its piece and its volume.
type Cell = (Slab, Vec<bool>, Vec<bool>, f64);

/// A region is cut cell by cell once it has no more ring points than this and
/// the body over it no more faces than [`CLIP_LEAF_FACES`]; until then it is
/// halved. Cutting cell by cell costs about the region's points times the
/// body's faces, so both have to be small.
const CLIP_LEAF_POINTS: usize = 12;
const CLIP_LEAF_FACES: usize = 2048;

/// A halving is kept only while its two halves hold less than this many times
/// the faces of the body they split. Each cut adds a cap, so halves never
/// quite halve; kept regardless, a split whose caps outweigh what it removed
/// would grow the work at every level instead of shrinking it.
const CLIP_MAX_GROWTH: f64 = 1.6;

/// Halvings stop here whatever the region: at most `2^12` pieces.
const CLIP_MAX_DEPTH: usize = 12;

/// Intersect a slab with a vertical polygon (including holes).
///
/// Earcut partitions its plan into disjoint convex cells. Each cell is clipped
/// and capped independently; shared vertical walls cancel in signed volume
/// integration and have zero contribution to block-overlap columns.
///
/// A cell's cut walks every face of the body it starts from, so a polygon of
/// a few thousand points - the outline of ground the topography crosses - is
/// first halved, body and polygon together, until each half is small; its
/// cells then start from that half alone. Cost goes from cells times body to
/// roughly body times the depth of the halving.
///
/// The result is a soup of those cells, and is deliberately *not* edge-manifold:
/// cells meeting at an internal wall triangulate it independently, so
/// [`open_edge_count`] over the result always reports it open even when every
/// cell is sound. It is no use as a trust signal here - the caller should judge
/// the volume on whether the body going *in* was closed, which is the only
/// thing that decides it, since a closed body clips to an exact one.
///
/// An open body is clipped and returned rather than rejected: bench bodies are
/// allowed to be open (`build_solid_between_surfaces` warns rather than
/// failing, and the View page reports it through `planning-reserve-open-solid`).
/// Failing here would take the whole View render down for a condition the page
/// already knows how to show.
pub(crate) fn clip_solid_to_plan(mesh: &mesh_data::Triangulation, face: &[Vec<glam::DVec2>], cancel: &crate::app::jobs::CancelFlag) -> Result<ClippedSolid> {
    let original: Slab = (mesh.vertices().to_vec(), mesh.face_vertex_indices_iter().map(|f| f.map(|i| i as u32)).collect());
    clip_piece_to_plan(original, face, cancel)
}

/// [`clip_solid_to_plan`] over a body already held as a slab - one piece of
/// separate ground split off a flitch. Never an earlier clip's output: that
/// is a soup of unwelded cells, and clipping it again grows without bound.
pub(crate) fn clip_piece_to_plan(slab: Slab, face: &[Vec<glam::DVec2>], cancel: &crate::app::jobs::CancelFlag) -> Result<ClippedSolid> {
    let floor = slab.0.iter().map(|vertex| vertex.z).fold(f64::INFINITY, f64::min);
    let count = slab.1.len();
    // The body's own surfaces are not walls the clip made.
    let cells = clip_region((slab, vec![false; count], vec![false; count]), face.to_vec(), face, floor, 0, cancel)?;
    Ok(gather_cells(cells))
}

/// One body's cells as a single clipped piece.
fn gather_cells(cells: Vec<Cell>) -> ClippedSolid {
    let mut result: Slab = (Vec::new(), Vec::new());
    let mut boundary_wall = Vec::new();
    let mut internal_wall = Vec::new();
    let mut volume = 0.0;
    for (slab, wall, internal, cell_volume) in cells {
        volume += cell_volume;
        boundary_wall.extend(wall);
        internal_wall.extend(internal);
        let offset = result.0.len() as u32;
        result.0.extend(slab.0);
        result.1.extend(slab.1.into_iter().map(|face| face.map(|index| index + offset)));
    }
    debug_assert_eq!(boundary_wall.len(), result.1.len());
    ClippedSolid {
        slab: result,
        volume,
        boundary_wall,
        internal_wall,
    }
}

/// How far outside a face the cuts that narrow a body down to it run, so
/// their caps never meet the face's own walls and are cut away whole.
const NARROW_MARGIN: f64 = 1.0;

/// A body is not halved further for its faces once it has no more faces
/// than this: cut down to each face's extent, it is already small.
const NARROW_LEAF_FACES: usize = 4096;

/// [`clip_piece_to_plan`] for many faces of one body at once, giving the same
/// pieces. Each face is cut out with its `plans` entry, and lies within its
/// `grounds` entry: the body has no material in the plan outside the ground.
///
/// Cut one at a time, every face pays for cuts across the whole body: the
/// halvings that cut a face apart run through it, so the pieces they leave
/// still stretch across the body. Here the body is halved first, and each
/// half again, as far as its faces fall wholly to one side, so the faces of
/// one half share the cuts that made it; each face is then cut down to its
/// own extent before it is clipped, so its cuts cross only the ground around
/// it. A face that crosses a halving is clipped from the body before it.
pub(crate) fn clip_piece_to_plans(slab: Slab, plans: &[&[Vec<glam::DVec2>]], grounds: &[&[Vec<glam::DVec2>]], cancel: &crate::app::jobs::CancelFlag) -> Result<Vec<ClippedSolid>> {
    let floor = slab.0.iter().map(|vertex| vertex.z).fold(f64::INFINITY, f64::min);
    let extent = |points: &mut dyn Iterator<Item = glam::DVec2>| {
        points.fold((glam::DVec2::splat(f64::INFINITY), glam::DVec2::splat(f64::NEG_INFINITY)), |(min, max), point| {
            (min.min(point), max.max(point))
        })
    };
    let body = extent(&mut slab.0.iter().map(|vertex| glam::DVec2::new(vertex.x, vertex.y)));
    let bounds: Vec<_> = grounds.iter().map(|ground| extent(&mut ground.iter().flatten().copied())).collect();
    let count = slab.1.len();
    let narrow = Narrowing {
        faces: plans,
        bounds: &bounds,
        floor,
        cancel,
    };
    let mut clipped: Vec<_> = narrow.share((slab, vec![false; count], vec![false; count]), body, (0..plans.len()).collect())?;
    clipped.sort_unstable_by_key(|(index, _)| *index);
    Ok(clipped.into_iter().map(|(_, piece)| piece).collect())
}

/// The faces [`clip_piece_to_plans`] cuts, with the plan extents of their ground.
struct Narrowing<'a> {
    faces: &'a [&'a [Vec<glam::DVec2>]],
    bounds: &'a [(glam::DVec2, glam::DVec2)],
    floor: f64,
    cancel: &'a crate::app::jobs::CancelFlag,
}

impl Narrowing<'_> {
    /// Clip `members` out of `piece`, which covers no more than `area` in
    /// plan, halving it across the longer side of `area` while that leaves
    /// some of them wholly to one side.
    fn share(&self, piece: CutPiece, area: (glam::DVec2, glam::DVec2), members: Vec<usize>) -> Result<Vec<(usize, ClippedSolid)>> {
        anyhow::ensure!(!self.cancel.is_cancelled(), "Cancelled");
        let (min, max) = area;
        let size = max - min;
        let axis = if size.x >= size.y { 0 } else { 1 };
        let middle = (min[axis] + max[axis]) / 2.0;
        // Each half reaches past the middle, so a face lying on it still
        // falls wholly to one side.
        let (mut low, mut high, mut here) = (Vec::new(), Vec::new(), Vec::new());
        if piece.0.1.len() > NARROW_LEAF_FACES && size[axis] > 8.0 * NARROW_MARGIN {
            for index in members {
                let (face_min, face_max) = self.bounds[index];
                let centre = (face_min[axis] + face_max[axis]) / 2.0;
                let fits_low = face_max[axis] <= middle + NARROW_MARGIN;
                let fits_high = face_min[axis] >= middle - NARROW_MARGIN;
                match (fits_low, fits_high) {
                    (true, true) if centre <= middle => low.push(index),
                    (true, false) => low.push(index),
                    (_, true) => high.push(index),
                    (false, false) => here.push(index),
                }
            }
        } else {
            here = members;
        }
        let side = |keep_low: bool, members: Vec<usize>| -> Result<Vec<(usize, ClippedSolid)>> {
            if members.is_empty() {
                return Ok(Vec::new());
            }
            let edge = if keep_low { middle + 2.0 * NARROW_MARGIN } else { middle - 2.0 * NARROW_MARGIN };
            let half = keep_side(&piece, area, axis, edge, keep_low, self.floor);
            let mut half_area = area;
            if keep_low {
                half_area.1[axis] = edge;
            } else {
                half_area.0[axis] = edge;
            }
            self.share(half, half_area, members)
        };
        let ((low, high), here) = rayon::join(
            || rayon::join(|| side(true, low), || side(false, high)),
            || {
                here.into_par_iter()
                    .map(|index| -> Result<(usize, ClippedSolid)> {
                        anyhow::ensure!(!self.cancel.is_cancelled(), "Cancelled");
                        let (face_min, face_max) = self.bounds[index];
                        let mut cropped = None;
                        let mut cropped_area = area;
                        for (axis, keep_low, edge) in [
                            (0, false, face_min.x - NARROW_MARGIN),
                            (0, true, face_max.x + NARROW_MARGIN),
                            (1, false, face_min.y - NARROW_MARGIN),
                            (1, true, face_max.y + NARROW_MARGIN),
                        ] {
                            // Already inside: nothing of the body lies past it.
                            if !edge.is_finite() || (keep_low && edge >= cropped_area.1[axis]) || (!keep_low && edge <= cropped_area.0[axis]) {
                                continue;
                            }
                            cropped = Some(keep_side(cropped.as_ref().unwrap_or(&piece), cropped_area, axis, edge, keep_low, self.floor));
                            if keep_low {
                                cropped_area.1[axis] = edge;
                            } else {
                                cropped_area.0[axis] = edge;
                            }
                        }
                        let cropped = cropped.unwrap_or_else(|| piece.clone());
                        let face = self.faces[index];
                        Ok((index, gather_cells(clip_region(cropped, face.to_vec(), face, self.floor, 0, self.cancel)?)))
                    })
                    .collect::<Result<Vec<_>>>()
            },
        );
        let mut clipped = low?;
        clipped.extend(high?);
        clipped.extend(here?);
        Ok(clipped)
    }
}

/// The part of `piece` on one side of the line where `axis` is `edge`, closed
/// with an internal cap: the low side when `keep_low`. `area` bounds the
/// piece in plan, so the line reaches past it.
fn keep_side(piece: &CutPiece, area: (glam::DVec2, glam::DVec2), axis: usize, edge: f64, keep_low: bool, floor: f64) -> CutPiece {
    let (min, max) = area;
    let reach = (max - min).max_element().max(1.0);
    let (from, to) = (min[1 - axis] - reach, max[1 - axis] + reach);
    let at = |along: f64| if axis == 0 { glam::DVec2::new(edge, along) } else { glam::DVec2::new(along, edge) };
    // `clip_half` keeps what lies left of `a -> b`.
    let upwards = (axis == 0) == keep_low;
    let (a, b) = if upwards { (at(from), at(to)) } else { (at(to), at(from)) };
    clip_half(piece, a, b, false, floor)
}

/// One plan face's share of a body split by [`split_solid_by_plan`]: its
/// faces, the volume they enclose, and which source vertex each of its
/// vertices was, so per-vertex data such as outline edges can follow it.
pub(crate) struct SplitPiece {
    pub(crate) slab: Slab,
    pub(crate) volume: f64,
    pub(crate) source_vertex: Vec<u32>,
}

/// Split a body among plan faces without cutting it, when every face is
/// whole separate ground: the flitch footprint split only where the ground
/// itself comes apart, with no line drawn across it.
///
/// The body's edge-connected pieces are each handed to the face holding a
/// point of their largest plan triangle. `None` - and the caller clips
/// instead - when a piece lands in no face or in two, or a face gets none.
/// The caller must only ask when no line crosses the ground: a piece is
/// never checked to lie wholly inside its face.
pub(crate) fn split_solid_by_plan(mesh: &mesh_data::Triangulation, faces: &[&[Vec<glam::DVec2>]]) -> Option<Vec<SplitPiece>> {
    use std::collections::HashMap;

    let vertices = mesh.vertices();
    let triangles: Vec<[u32; 3]> = mesh.face_vertex_indices_iter().map(|f| f.map(|i| i as u32)).collect();
    let weld = Weld::of(vertices);

    // Faces sharing an edge, by welded position, are one piece.
    let mut parent: Vec<u32> = (0..triangles.len() as u32).collect();
    fn root(parent: &mut [u32], mut index: u32) -> u32 {
        while parent[index as usize] != index {
            parent[index as usize] = parent[parent[index as usize] as usize];
            index = parent[index as usize];
        }
        index
    }
    let mut first: HashMap<EdgeKey, u32, foldhash::fast::RandomState> = HashMap::default();
    for (index, triangle) in triangles.iter().enumerate() {
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let (ka, kb) = (weld.key(vertices[triangle[a] as usize]), weld.key(vertices[triangle[b] as usize]));
            if ka == kb {
                continue;
            }
            let other = *first.entry(if ka <= kb { (ka, kb) } else { (kb, ka) }).or_insert(index as u32);
            let (x, y) = (root(&mut parent, other), root(&mut parent, index as u32));
            if x != y {
                parent[x as usize] = y;
            }
        }
    }

    // Each piece's probe: the centroid of its largest triangle in plan.
    let mut probe: HashMap<u32, (f64, glam::DVec2), foldhash::fast::RandomState> = HashMap::default();
    for (index, triangle) in triangles.iter().enumerate() {
        let [a, b, c] = triangle.map(|i| glam::DVec2::new(vertices[i as usize].x, vertices[i as usize].y));
        let area = (b - a).perp_dot(c - a).abs();
        let entry = probe.entry(root(&mut parent, index as u32)).or_insert((-1.0, glam::DVec2::ZERO));
        if area > entry.0 {
            *entry = (area, (a + b + c) / 3.0);
        }
    }
    let mut owner: HashMap<u32, usize, foldhash::fast::RandomState> = HashMap::default();
    for (piece, (area, point)) in probe {
        // A piece with no extent in plan is all wall, and says nothing.
        if area <= 0.0 {
            return None;
        }
        let mut holders = faces.iter().enumerate().filter(|(_, face)| crate::model::arrangement::point_in_face(face, point));
        let (face, None) = (holders.next()?.0, holders.next()) else { return None };
        owner.insert(piece, face);
    }

    let origin = mesh.bounds().min;
    let mut pieces: Vec<SplitPiece> = (0..faces.len())
        .map(|_| SplitPiece {
            slab: (Vec::new(), Vec::new()),
            volume: 0.0,
            source_vertex: Vec::new(),
        })
        .collect();
    let mut local: Vec<HashMap<u32, u32, foldhash::fast::RandomState>> = (0..faces.len()).map(|_| HashMap::default()).collect();
    for (index, triangle) in triangles.iter().enumerate() {
        let face = owner[&root(&mut parent, index as u32)];
        let piece = &mut pieces[face];
        let corners = triangle.map(|source| {
            *local[face].entry(source).or_insert_with(|| {
                piece.slab.0.push(vertices[source as usize]);
                piece.source_vertex.push(source);
                piece.slab.0.len() as u32 - 1
            })
        });
        piece.slab.1.push(corners);
        let [a, b, c] = triangle.map(|i| {
            let v = vertices[i as usize];
            glam::DVec3::new(v.x - origin.x, v.y - origin.y, v.z - origin.z)
        });
        piece.volume += a.dot(b.cross(c)) / 6.0;
    }
    if pieces.iter().any(|piece| piece.slab.1.is_empty()) {
        return None;
    }
    for piece in &mut pieces {
        piece.volume = piece.volume.abs();
    }
    Some(pieces)
}

/// Cut `piece` to `region`, a part of `face` the piece already lies over.
/// Halves both along a line across the region's longer side while either is
/// big and halving pays, and cuts it cell by cell once not. Cells come back in a fixed
/// order - the left half's before the right's - so the result does not depend
/// on how the halves were scheduled.
fn clip_region(piece: CutPiece, region: Vec<Vec<glam::DVec2>>, face: &[Vec<glam::DVec2>], floor: f64, depth: usize, cancel: &crate::app::jobs::CancelFlag) -> Result<Vec<Cell>> {
    anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
    if piece.0.1.is_empty() {
        return Ok(Vec::new());
    }
    let points: usize = region.iter().map(Vec::len).sum();
    if (points > CLIP_LEAF_POINTS || piece.0.1.len() > CLIP_LEAF_FACES)
        && depth < CLIP_MAX_DEPTH
        && let Some(((a, b), left, right)) = halve_region(&region)
    {
        let (left_piece, right_piece) = rayon::join(|| clip_half(&piece, a, b, false, floor), || clip_half(&piece, b, a, false, floor));
        if (left_piece.0.1.len() + right_piece.0.1.len()) as f64 > piece.0.1.len() as f64 * CLIP_MAX_GROWTH {
            // The split cost more than it saved: cut this region whole.
            drop((left_piece, right_piece));
            return clip_cells(&piece, &region, face, floor, cancel);
        }
        drop(piece);
        let (left_cells, right_cells) = rayon::join(
            || -> Result<Vec<Cell>> {
                let mut cells = Vec::new();
                for part in left {
                    cells.extend(clip_region(left_piece.clone(), part, face, floor, depth + 1, cancel)?);
                }
                Ok(cells)
            },
            || -> Result<Vec<Cell>> {
                let mut cells = Vec::new();
                for part in right {
                    cells.extend(clip_region(right_piece.clone(), part, face, floor, depth + 1, cancel)?);
                }
                Ok(cells)
            },
        );
        let mut cells = left_cells?;
        cells.extend(right_cells?);
        return Ok(cells);
    }
    clip_cells(&piece, &region, face, floor, cancel)
}

/// Split a region along a line through the middle of its longer side: the
/// line, and the parts either side of it. `None` when the split leaves no part
/// on one side, so the region is cut whole instead.
#[allow(clippy::type_complexity, reason = "the split line and its two sides, used once by the caller")]
fn halve_region(region: &[Vec<glam::DVec2>]) -> Option<((glam::DVec2, glam::DVec2), Vec<Vec<Vec<glam::DVec2>>>, Vec<Vec<Vec<glam::DVec2>>>)> {
    let (min, max) = region
        .iter()
        .flatten()
        .fold((glam::DVec2::splat(f64::INFINITY), glam::DVec2::splat(f64::NEG_INFINITY)), |(min, max), point| {
            (min.min(*point), max.max(*point))
        });
    let size = max - min;
    if !size.is_finite() || size.max_element() <= 0.0 {
        return None;
    }
    let middle = (min + max) / 2.0;
    let margin = size.max_element() + 1.0;
    // Left of a -> b is the low side of the split axis.
    let (a, b) = if size.x >= size.y {
        (glam::DVec2::new(middle.x, max.y + margin), glam::DVec2::new(middle.x, min.y - margin))
    } else {
        (glam::DVec2::new(min.x - margin, middle.y), glam::DVec2::new(max.x + margin, middle.y))
    };
    let mut left = Vec::new();
    let mut right = Vec::new();
    for part in crate::model::arrangement::subdivide(region, &[vec![a, b]]) {
        let point = crate::model::arrangement::representative_point(&part)?;
        if (b - a).perp_dot(point - a) > 0.0 {
            left.push(part);
        } else {
            right.push(part);
        }
    }
    (!left.is_empty() && !right.is_empty()).then_some(((a, b), left, right))
}

/// Keep the part of `piece` left of the vertical plane through `a -> b`, and
/// close it with a cap there - a wall along the polygon's boundary when
/// `on_boundary`, an internal one otherwise.
fn clip_half(piece: &CutPiece, a: glam::DVec2, b: glam::DVec2, on_boundary: bool, floor: f64) -> CutPiece {
    let (slab, wall, internal) = piece;
    let edge = (b - a).normalize();
    let tangent = glam::DVec3::new(edge.x, edge.y, 0.0);
    let normal = glam::DVec3::new(-edge.y, edge.x, 0.0);
    let origin = glam::DVec3::new(a.x, a.y, floor);
    let local: Vec<_> = slab
        .0
        .iter()
        .map(|p| {
            let delta = glam::DVec3::new(p.x, p.y, p.z) - origin;
            let distance = delta.dot(normal);
            mesh_data::Vertex::new(delta.z, delta.dot(tangent), if distance.abs() < 1e-8 { 0.0 } else { distance })
        })
        .collect();
    let high = local.iter().map(|p| p.z).fold(0.0, f64::max) + 1.0;
    let mut clipped: Slab = (Vec::new(), Vec::new());
    let mut clipped_wall = Vec::new();
    let mut clipped_internal = Vec::new();
    for (index, indices) in slab.1.iter().enumerate() {
        let triangle = indices.map(|index| local[index as usize]);
        // A coplanar face with material on the discarded side is not
        // a zero-thickness part of the retained volume.
        if triangle.iter().all(|p| p.z == 0.0)
            && (triangle[1].x - triangle[0].x) * (triangle[2].y - triangle[0].y) - (triangle[1].y - triangle[0].y) * (triangle[2].x - triangle[0].x) > 0.0
        {
            continue;
        }
        for tri in super::cuts::clip_triangle_z(triangle, 0.0, high) {
            if collapsed(&tri) {
                continue;
            }
            let start = clipped.0.len() as u32;
            clipped.0.extend(tri);
            clipped.1.push([start, start + 1, start + 2]);
            clipped_wall.push(wall[index]);
            clipped_internal.push(internal[index]);
        }
    }
    if !clipped.1.is_empty() {
        cap_slab(&mut clipped, 0.0, high);
    }
    // Everything `cap_slab` just appended closes this cut plane, so it
    // is wall - and bounds the body only where this edge does.
    clipped_wall.resize(clipped.1.len(), on_boundary);
    clipped_internal.resize(clipped.1.len(), !on_boundary);
    for p in &mut clipped.0 {
        let world = origin + glam::DVec3::Z * p.x + tangent * p.y + normal * p.z;
        *p = mesh_data::Vertex::new(world.x, world.y, world.z);
    }
    (clipped, clipped_wall, clipped_internal)
}

/// Cut `piece` to each earcut cell of `region`, in parallel and returned in
/// earcut order. A cell edge cuts a boundary wall only where it runs along
/// `face`, the whole polygon - not along a line a halving drew.
fn clip_cells(piece: &CutPiece, region: &[Vec<glam::DVec2>], face: &[Vec<glam::DVec2>], floor: f64, cancel: &crate::app::jobs::CancelFlag) -> Result<Vec<Cell>> {
    let mut points = Vec::new();
    let mut holes = Vec::new();
    for (i, ring) in region.iter().enumerate() {
        if i > 0 {
            holes.push(points.len());
        }
        points.extend_from_slice(ring);
    }
    let mut triangles = Vec::new();
    earcut::Earcut::new().earcut(points.iter().map(|p| [p.x, p.y]), &holes, &mut triangles);
    let clip_cell = |cell: &Vec<usize>| -> Result<Option<Cell>> {
        anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
        let mut cut = piece.clone();
        for (a, b, on_boundary) in cell_edges(&points, cell, face) {
            if cut.0.1.is_empty() {
                break;
            }
            cut = clip_half(&cut, a, b, on_boundary, floor);
        }
        let (mut slab, wall, internal) = cut;
        if slab.1.is_empty() {
            return Ok(None);
        }
        // Each cell's own volume, as the integral between its floor and its
        // roof over the ground it covers.
        //
        // Deliberately not a divergence sum over the cell's surface. That is
        // only exact on a closed cell, and a cell is not reliably closed: the
        // clip leaves T-junctions, and the walls it caps can come out
        // incomplete where the body thins to nothing at a cell corner. A
        // divergence sum over such a cell reads near zero, which is how a
        // bench's dig blocks came to total less than the bench itself by an
        // amount that moved with the decomposition the clip happened to
        // choose. The cut walls are vertical, so they project to no ground
        // and contribute nothing here; only floor and roof do, and those
        // cover the cell exactly whatever the walls did.
        let signed = prism_volume(&slab.0, &slab.1);
        // Volume is taken per cell as a magnitude, but the block-model overlap
        // integrates signed prisms over floor and roof from one shared origin,
        // so it needs every cell wound the same way. No cell in the test fixture
        // comes out inverted, and none should - this guards the invariant the
        // overlap relies on rather than fixing an observed fault.
        if signed < 0.0 {
            for face in &mut slab.1 {
                face.swap(1, 2);
            }
        }
        Ok(Some((slab, wall, internal, signed.abs())))
    };
    Ok(convex_cells(&points, &triangles)
        .par_iter()
        .map(clip_cell)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

/// Earcut's triangles merged across the diagonals they share while the
/// merge stays convex, each as its corners anticlockwise.
///
/// A convex cell is cut along its own edges however many it has, so every
/// diagonal merged away is a cut through the body saved, and the pair of
/// internal walls it would have left inside it: a rectangle is one cell of
/// four cuts rather than two triangles of three.
fn convex_cells(points: &[glam::DVec2], triangles: &[usize]) -> Vec<Vec<usize>> {
    let mut cells: Vec<Vec<usize>> = triangles
        .as_chunks::<3>()
        .0
        .iter()
        .map(|&[a, b, c]| {
            if (points[b] - points[a]).perp_dot(points[c] - points[a]) < 0.0 {
                vec![a, c, b]
            } else {
                vec![a, b, c]
            }
        })
        .collect();
    let convex = |ring: &[usize]| {
        let mut seen = std::collections::HashSet::with_capacity(ring.len());
        ring.iter().all(|corner| seen.insert(*corner))
            && (0..ring.len()).all(|index| {
                let [a, b, c] = [0, 1, 2].map(|step| points[ring[(index + step) % ring.len()]]);
                let (inward, outward) = (b - a, c - b);
                inward.perp_dot(outward) >= -COLLINEAR_TOLERANCE * inward.length() * outward.length()
            })
    };
    'merging: loop {
        for first in 0..cells.len() {
            for second in first + 1..cells.len() {
                let (left, right) = (&cells[first], &cells[second]);
                for (index, &a) in left.iter().enumerate() {
                    let b = left[(index + 1) % left.len()];
                    // The shared diagonal runs b -> a round the other cell.
                    let Some(at) = right.iter().position(|corner| *corner == b) else { continue };
                    if right[(at + 1) % right.len()] != a {
                        continue;
                    }
                    let mut merged: Vec<usize> = (1..=left.len()).map(|step| left[(index + step) % left.len()]).collect();
                    merged.extend((2..right.len()).map(|step| right[(at + step) % right.len()]));
                    if convex(&merged) {
                        cells[first] = merged;
                        cells.swap_remove(second);
                        continue 'merging;
                    }
                }
            }
        }
        return cells;
    }
}

/// The lines a convex cell is cut along, anticlockwise, each with whether it
/// runs along `face`'s own boundary. Corners a merge left straight are passed
/// over where the edges either side are the same kind, so one line is not
/// cut twice.
fn cell_edges(points: &[glam::DVec2], cell: &[usize], face: &[Vec<glam::DVec2>]) -> Vec<(glam::DVec2, glam::DVec2, bool)> {
    let mut edges: Vec<(glam::DVec2, glam::DVec2, bool)> = Vec::with_capacity(cell.len());
    for (index, &corner) in cell.iter().enumerate() {
        let (a, b) = (points[corner], points[cell[(index + 1) % cell.len()]]);
        edges.push((a, b, segment_follows(face, a, b)));
    }
    let straight = |(a, b, _): (glam::DVec2, glam::DVec2, bool), (_, c, _): (glam::DVec2, glam::DVec2, bool)| {
        (b - a).perp_dot(c - b).abs() <= COLLINEAR_TOLERANCE * (b - a).length() * (c - b).length() && (b - a).dot(c - b) > 0.0
    };
    let mut joined: Vec<(glam::DVec2, glam::DVec2, bool)> = Vec::with_capacity(edges.len());
    for edge in edges {
        match joined.last_mut() {
            Some(last) if last.2 == edge.2 && straight(*last, edge) => last.1 = edge.1,
            _ => joined.push(edge),
        }
    }
    // The last edge may run straight on into the first.
    if joined.len() > 3
        && let (Some(&last), Some(&first)) = (joined.last(), joined.first())
        && last.2 == first.2
        && straight(last, first)
    {
        joined[0].0 = last.0;
        joined.pop();
    }
    joined
}

/// The volume between a vertically-closed body's floor and its roof.
///
/// Every face contributes its projected ground times its mean height, signed
/// by which way it faces, which telescopes to the height between the roof and
/// the floor over every point of the footprint. Vertical faces project to
/// nothing and drop out - which is the point: it holds whether or not the
/// walls are closed, and cannot be changed by re-triangulating them.
fn prism_volume(vertices: &[mesh_data::Vertex], faces: &[[u32; 3]]) -> f64 {
    // Rebased, and on the height above all: a mine RL is three digits of
    // elevation over a metre of slab, and the floor's contribution cancels
    // the roof's almost exactly. Measured from the lowest vertex, the terms
    // are the height of the material rather than the height of the datum.
    let Some(origin) = vertices.first().copied() else {
        return 0.0;
    };
    let floor = vertices.iter().map(|vertex| vertex.z).fold(f64::INFINITY, f64::min);
    faces
        .iter()
        .map(|face| {
            let [a, b, c] = face.map(|index| {
                let vertex = vertices[index as usize];
                [vertex.x - origin.x, vertex.y - origin.y, vertex.z - floor]
            });
            let projected = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            projected / 2.0 * (a[2] + b[2] + c[2]) / 3.0
        })
        .sum()
}

/// Whether the segment `a`-`b` runs along one of `rings` rather than cutting
/// across the ground they enclose.
///
/// The midpoint decides it: a cell edge either lies on a ring, having been
/// taken from it, or is a diagonal whose middle is clear of every ring.
fn segment_follows(rings: &[Vec<glam::DVec2>], a: glam::DVec2, b: glam::DVec2) -> bool {
    const TOLERANCE: f64 = 1.0e-3;

    let midpoint = a.midpoint(b);
    rings.iter().any(|ring| {
        ring.len() >= 2
            && (0..ring.len()).any(|index| {
                let (start, end) = (ring[index], ring[(index + 1) % ring.len()]);
                let span = end - start;
                let length_squared = span.length_squared();
                let closest = if length_squared < 1e-18 {
                    start
                } else {
                    start + span * ((midpoint - start).dot(span) / length_squared).clamp(0.0, 1.0)
                };
                closest.distance_squared(midpoint) <= TOLERANCE * TOLERANCE
            })
    })
}

/// The edges where a body turns a corner - a bench's crest and toe, and where
/// a cut wall meets the ground - rather than every triangle edge across its
/// flat faces. An edge is kept when the planes either side of it meet at more
/// than `min_angle` radians.
///
/// Welded by position, as [`boundary_wall_outline`] is, because a clipped
/// body's faces do not share vertex indices. `internal_wall` is the clip's own
/// mask of the walls between its cells, parallel to the faces.
pub(crate) fn crease_outline(slab: &Slab, internal_wall: &[bool], min_angle: f64) -> Vec<[u32; 2]> {
    /// Twice a face's area over its longest side squared, below which the
    /// face is a sliver: about a 1:1000 height to length.
    const SLIVER_RATIO: f64 = 1e-3;

    use std::collections::HashMap;

    let (vertices, faces) = slab;
    let weld = Weld::of(vertices);
    let position = |index: u32| {
        let vertex = vertices[index as usize];
        glam::DVec3::new(vertex.x, vertex.y, vertex.z)
    };
    let mut edges: HashMap<EdgeKey, ([u32; 2], Vec<glam::DVec3>), foldhash::fast::RandomState> = HashMap::default();
    // The walls between the clip's cells are inside the body: where one meets
    // the ground is a seam of the decomposition, not a corner of the blast.
    for (face, _) in faces.iter().zip(internal_wall).filter(|(_, internal)| !**internal) {
        let [a, b, c] = face.map(position);
        let cross = (b - a).cross(c - a);
        // A sliver's normal is rounding noise, and the clip leaves fans of
        // them across flat faces: left in, every one reads as a corner.
        let longest = (b - a).length_squared().max((c - b).length_squared()).max((a - c).length_squared());
        if cross.length() <= longest * SLIVER_RATIO {
            continue;
        }
        let normal = cross.normalize();
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let (ia, ib) = (face[a], face[b]);
            let (ka, kb) = (weld.key(vertices[ia as usize]), weld.key(vertices[ib as usize]));
            if ka == kb {
                continue;
            }
            edges
                .entry(if ka <= kb { (ka, kb) } else { (kb, ka) })
                .or_insert_with(|| ([ia, ib], Vec::new()))
                .1
                .push(normal);
        }
    }
    let threshold = min_angle.cos();
    edges
        .into_values()
        // Unsigned: the clip does not wind every cell's faces the same way,
        // and two coplanar faces wound apart are no corner. An edge only one
        // face uses is where the clip's cells meet out of step - a T-junction
        // across a flat face, not a corner - so it is never drawn.
        .filter(|(_, normals)| normals.len() > 1 && normals[1..].iter().any(|normal| normals[0].dot(*normal).abs() < threshold))
        .map(|(edge, _)| edge)
        .collect()
}

/// The rim of the walls the clip cut: the outline separating this piece from
/// the one beside it.
///
/// A wall's own triangulation, and the joins where two cells' walls meet in the
/// same plane, are used by two wall faces and drop out; the top and bottom rims
/// and the wall's ends are used once and remain. Welded by position, because a
/// clipped body's faces do not share vertex indices.
pub(crate) fn boundary_wall_outline(slab: &Slab, boundary_wall: &[bool]) -> Vec<[u32; 2]> {
    use std::collections::HashMap;

    let (vertices, faces) = slab;
    let weld = Weld::of(vertices);
    let mut counts: HashMap<EdgeKey, (usize, [u32; 2]), foldhash::fast::RandomState> = HashMap::default();
    for (face, _) in faces.iter().zip(boundary_wall).filter(|(_, wall)| **wall) {
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let (ia, ib) = (face[a], face[b]);
            let (ka, kb) = (weld.key(vertices[ia as usize]), weld.key(vertices[ib as usize]));
            if ka == kb {
                continue;
            }
            let entry = counts.entry(if ka <= kb { (ka, kb) } else { (kb, ka) }).or_insert((0, [ia, ib]));
            entry.0 += 1;
        }
    }
    counts.into_values().filter(|(count, _)| *count == 1).map(|(_, edge)| edge).collect()
}
