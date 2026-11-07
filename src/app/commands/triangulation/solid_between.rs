//! Build a closed solid from two surfaces - the staple "volume between a
//! design and the ground" construction.
//!
//! Feeds both the Triangulation menu's own tool and the Solids Setup page's
//! inspection preview, so the two can never disagree about what a solid is.

use anyhow::Result;
use rayon::prelude::*;

use super::{
    cuts::{FoldLayer, PreparedReferenceSurface, REPAIR_WELD, clip_surfaces_against_each_other, single_valued_surface, snap_surface_points},
    *,
};
use crate::ui::state::SolidRegion;

/// Every hash map here is seeded the same way in every run. Their iteration
/// order decides which of a welded group's points stands for it and which way
/// an outline is walked round a branch, so a per-process seed cuts the same
/// ground a hair differently on each opening - enough to break the exact
/// footprints a dig order holds its blocks by.
type FixedState = foldhash::fast::FixedState;

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

/// `sheet` with each face rewound in place so its normal points the way
/// `facing` asks: a sheet cut from a survey topography is too large to copy
/// for it.
///
/// The inputs are 2.5D fragments of clipped surfaces - `clip_mesh_by_surface`
/// drops the vertical and degenerate faces that have no XY area - so the sign
/// of a face's normal Z is exactly its up/down orientation, and flipping two
/// indices is all a face ever needs. The source meshes' own winding is
/// therefore irrelevant, which matters because an imported design surface and
/// an imported topography rarely agree about it.
fn oriented(sheet: (Vec<mesh_data::Vertex>, Vec<[u32; 3]>), facing: Facing) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>) {
    let (vertices, mut faces) = sheet;
    for face in &mut faces {
        let corners = face.map(|index| vertices[index as usize]);
        let edge_a = glam::DVec2::new(corners[1].x - corners[0].x, corners[1].y - corners[0].y);
        let edge_b = glam::DVec2::new(corners[2].x - corners[0].x, corners[2].y - corners[0].y);
        let normal_z = edge_a.x * edge_b.y - edge_a.y * edge_b.x;
        let points_up = normal_z > 0.0;
        let wanted_up = facing == Facing::Up;
        if points_up != wanted_up {
            face.swap(1, 2);
        }
    }
    (vertices, faces)
}

/// How close in height, in model units, the design and the topography may run
/// and still be taken to meet rather than enclose anything.
///
/// A design is often built from the topography - a pit stitched into the
/// ground it was cut from - and agrees with it beyond the crest only to within
/// rounding, or to where the two triangulate the same ground differently.
/// Taken literally, that noise is thousands of slivers a hair thick, each its
/// own scrap of solid with sides too thin to close.
const SURFACE_CONTACT: f64 = 1.0e-2;

/// Clip each surface to the wanted region and hand back the two sheets that
/// bound it.
fn build_envelopes(design: &PreparedReferenceSurface, topography: &PreparedReferenceSurface, region: SolidRegion, progress: &crate::model::progress::Phase) -> SolidEnvelopes {
    // `CutTop` keeps what lies below the reference, `CutBottom` what lies
    // above it. For a cut the floor is the design where it runs below the
    // ground and the roof is the ground above it; for a fill the roles - and
    // so the sides - are the other way round.
    let (design_side, topography_side) = match region {
        SolidRegion::Cut => (TriSurfaceCutSide::CutTop, TriSurfaceCutSide::CutBottom),
        SolidRegion::Fill => (TriSurfaceCutSide::CutBottom, TriSurfaceCutSide::CutTop),
    };
    // The design's faces are few and large, and come out whole wherever they
    // lie clear of the ground; the topography's are already as fine as the
    // ground. Both are cut from one pass over the overlaps between them.
    let (design_sheet, topography_sheet) = clip_surfaces_against_each_other(design, topography, design_side, topography_side, SURFACE_CONTACT, progress);

    let (floor, roof) = match region {
        SolidRegion::Cut => (design_sheet, topography_sheet),
        SolidRegion::Fill => (topography_sheet, design_sheet),
    };

    SolidEnvelopes {
        lower: oriented(floor, Facing::Down),
        upper: oriented(roof, Facing::Up),
    }
}

/// The volume enclosed between two surfaces on one side of their crossing, as
/// one mesh.
///
/// Returns the solid ready to draw, with the volume it encloses in cubic
/// model units. Errors when there is nothing to enclose: surfaces whose XY
/// footprints miss each other, or a design that never runs on the wanted
/// side of the ground.
///
/// `label` names what is being built, a solid's name, for the console notes
/// about repaired surfaces; empty where there is nothing to call it.
pub(crate) fn build_solid_mesh(
    design: &mesh_data::Triangulation,
    topography: &mesh_data::Triangulation,
    region: SolidRegion,
    label: &str,
    progress: &crate::model::progress::Phase,
) -> Result<SolidMesh> {
    // The same pair is built more than once - the Setup page's inspection
    // preview and the Solids stage each build it, and the preview again on
    // every return to the solid - so a recent build of exactly these surfaces
    // is handed back rather than built again: the same mesh, not a copy, so
    // the two hold one solid between them.
    let key = (mesh_fingerprint(design), mesh_fingerprint(topography), region == SolidRegion::Cut);
    if let Some(built) = recent_builds().iter().find(|(recent, _)| *recent == key).map(|(_, built)| built.clone()) {
        progress.set_fraction(1.0);
        return Ok(built);
    }
    let mut notes = Vec::new();
    let (vertices, faces, volume) = build_solid_between_surfaces_uncached(design, topography, region, &progress.phase(0.0, 0.9), &mut notes)?;
    let generated = session::build_generated_triangulation(String::new(), vertices, faces, TriSurfaceType::SolidClosed, |_| Vec::new())?;
    progress.set_fraction(1.0);
    let built = SolidMesh {
        mesh: generated.mesh,
        spatial: generated.spatial,
        edges: std::sync::Arc::new(generated.edges),
        surface_face_order: generated.surface_face_order,
        volume,
    };
    // Once per pair of surfaces: the Setup preview and the Solids stage can
    // build the same pair at once, and the second would repeat the first.
    if first_report(key) {
        for note in notes {
            if label.is_empty() {
                userspace_log!("{note}");
            } else {
                userspace_log!("{label}: {note}");
            }
        }
    }
    let mut recent = recent_builds();
    recent.retain(|(recent, _)| *recent != key);
    recent.push((key, built.clone()));
    // The newest is kept whatever its size: whichever of the preview and the
    // Solids stage waited on this build is about to ask for it.
    while recent.len() > 1 && (recent.len() > RECENT_BUILDS || recent.iter().map(|(_, built)| built.bytes()).sum::<usize>() > RECENT_BUILD_BYTES) {
        recent.remove(0);
    }
    Ok(built)
}

/// A solid built between two surfaces, with what every view of it needs.
///
/// Each part is shared, so the Setup page's preview, the Solids stage and a
/// recent build kept for either hold the one solid between them: one built
/// from a survey topography runs to gigabytes.
#[derive(Clone)]
pub(crate) struct SolidMesh {
    pub(crate) mesh: std::sync::Arc<mesh_data::Triangulation>,
    pub(crate) spatial: std::sync::Arc<crate::model::spatial::TriangleBvh>,
    pub(crate) edges: std::sync::Arc<Vec<[u32; 2]>>,
    pub(crate) surface_face_order: std::sync::Arc<Vec<u32>>,
    /// The volume enclosed, in cubic model units.
    pub(crate) volume: f64,
}

impl SolidMesh {
    /// Roughly what the solid holds, for keeping recent builds in bounds.
    fn bytes(&self) -> usize {
        std::mem::size_of_val(self.mesh.vertices())
            + self.mesh.face_count() * std::mem::size_of::<[u32; 3]>()
            + std::mem::size_of_val(self.edges.as_slice())
            + std::mem::size_of_val(self.surface_face_order.as_slice())
    }
}

/// How many recent builds [`build_solid_mesh`] keeps: enough for the solid
/// on screen and the few the Solids stage is working through.
const RECENT_BUILDS: usize = 4;

/// How much memory the recent builds may hold between them. A solid cut from
/// a survey topography can be tens of millions of faces, and four of those
/// kept for a second look would cost more than building them again.
const RECENT_BUILD_BYTES: usize = 256 << 20;

type BuiltSolid = (Vec<mesh_data::Vertex>, Vec<[u32; 3]>, f64);
type RecentBuilds = Vec<((u64, u64, bool), SolidMesh)>;

fn recent_builds() -> std::sync::MutexGuard<'static, RecentBuilds> {
    static RECENT: std::sync::Mutex<RecentBuilds> = std::sync::Mutex::new(Vec::new());
    // A build that panicked mid-update leaves nothing worse than a stale
    // list, which is still a list of finished builds.
    RECENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Whether this pair of surfaces has not had its repairs reported yet this
/// session, marking it reported.
fn first_report(key: (u64, u64, bool)) -> bool {
    static REPORTED: std::sync::Mutex<Vec<(u64, u64, bool)>> = std::sync::Mutex::new(Vec::new());
    let mut reported = REPORTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if reported.contains(&key) {
        return false;
    }
    reported.push(key);
    true
}

/// A hash of a mesh's every point and face, so two meshes with the same
/// geometry - however they were loaded - are known for the same one.
fn mesh_fingerprint(mesh: &mesh_data::Triangulation) -> u64 {
    use std::hash::{BuildHasher, Hasher};

    use rayon::prelude::*;
    const CHUNK: usize = 16 * 1024;
    let hash_with = |write: &dyn Fn(&mut foldhash::fast::FoldHasher)| {
        let mut hasher = FixedState::default().build_hasher();
        write(&mut hasher);
        hasher.finish()
    };
    let vertices = mesh.vertices();
    let vertex_hashes: Vec<u64> = vertices
        .par_chunks(CHUNK)
        .map(|chunk| {
            hash_with(&|hasher| {
                for vertex in chunk {
                    hasher.write_u64(vertex.x.to_bits());
                    hasher.write_u64(vertex.y.to_bits());
                    hasher.write_u64(vertex.z.to_bits());
                }
            })
        })
        .collect();
    let face_hashes: Vec<u64> = (0..mesh.face_count())
        .into_par_iter()
        .step_by(CHUNK)
        .map(|start| {
            hash_with(&|hasher| {
                for face in start..(start + CHUNK).min(mesh.face_count()) {
                    for index in mesh.face_vertex_indices(face).unwrap_or_default() {
                        hasher.write_usize(index);
                    }
                }
            })
        })
        .collect();
    hash_with(&|hasher| {
        hasher.write_usize(vertices.len());
        hasher.write_usize(mesh.face_count());
        vertex_hashes.iter().chain(&face_hashes).for_each(|hash| hasher.write_u64(*hash));
    })
}

fn build_solid_between_surfaces_uncached(
    design: &mesh_data::Triangulation,
    topography: &mesh_data::Triangulation,
    region: SolidRegion,
    progress: &crate::model::progress::Phase,
    notes: &mut Vec<String>,
) -> Result<BuiltSolid> {
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
    let design = local(design)?;
    let Some(topography) = topography_under(&design, topography, origin)? else {
        anyhow::bail!(no_volume(region));
    };
    let (mut vertices, faces, volume) = build_solid_local(&design, topography, region, progress, notes)?;
    for v in &mut vertices {
        v.x += origin.x;
        v.y += origin.y;
        v.z += origin.z;
    }
    Ok((vertices, faces, volume))
}

/// How far past the design's faces in plan, in model units, the topography is
/// kept: well beyond [`REPAIR_WELD`], so every topography point that could
/// snap onto one of the design's is kept with it.
const TOPOGRAPHY_MARGIN: f64 = 1.0;

/// The faces of `topography` that lie under the `design`'s faces in plan,
/// moved by `-origin` as the design already has been; `None` when no face
/// does.
///
/// A face no design face's bounds reach takes part in neither clip - the
/// topography's sheet is only what lies over the design, and the design's
/// only what lies under the topography - so leaving it out changes nothing
/// but what it costs. A survey topography can run to tens of millions of
/// faces around a pit a fraction of its size, and every stage of the build
/// holds a few hundred bytes for each face it is given.
fn topography_under(design: &mesh_data::Triangulation, topography: &mesh_data::Triangulation, origin: mesh_data::Vertex) -> Result<Option<mesh_data::Triangulation>> {
    use rayon::prelude::*;

    let reach = crate::model::spatial::TriangleBvh::build(design);
    let vertices = topography.vertices();
    // The design has been moved already; the topography's faces are asked
    // about where they will be once moved too.
    let shift = glam::DVec2::new(origin.x, origin.y);
    let margin = glam::DVec2::splat(TOPOGRAPHY_MARGIN);
    let kept: Vec<bool> = (0..topography.face_count())
        .into_par_iter()
        .map_init(Vec::new, |stack, face| {
            let corners = topography.face_vertex_indices(face).unwrap_or_default().map(|index| vertices[index]);
            let (min, max) = super::cuts::triangle_xy_bounds(corners);
            let mut reached = false;
            reach.for_each_xy_bounds_candidate_index_with_stack(min - shift - margin, max - shift + margin, stack, |_| reached = true);
            reached
        })
        .collect();
    let mut index_of = vec![u32::MAX; vertices.len()];
    let mut kept_vertices = Vec::new();
    let mut kept_faces = Vec::with_capacity(kept.iter().filter(|&&kept| kept).count());
    for (face, _) in kept.iter().enumerate().filter(|(_, kept)| **kept) {
        let corners = topography.face_vertex_indices(face).unwrap_or_default();
        kept_faces.push(corners.map(|index| {
            if index_of[index] == u32::MAX {
                index_of[index] = kept_vertices.len() as u32;
                let vertex = vertices[index];
                kept_vertices.push(mesh_data::Vertex::new(vertex.x - origin.x, vertex.y - origin.y, vertex.z - origin.z));
            }
            index_of[index]
        }));
    }
    if kept_faces.is_empty() {
        return Ok(None);
    }
    mesh_data::Triangulation::from_vertices_and_faces(kept_vertices, kept_faces)
        .map(Some)
        .map_err(|error| anyhow::anyhow!("{error}"))
}

fn build_solid_local(
    design: &mesh_data::Triangulation,
    topography: mesh_data::Triangulation,
    region: SolidRegion,
    progress: &crate::model::progress::Phase,
    notes: &mut Vec<String>,
) -> Result<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>, f64)> {
    // The two surfaces share the ground beyond the crest when one was built
    // from the other, and points rounding moved a hair apart are made one.
    let topography = match snap_surface_points(&topography, design)? {
        Some(snapped) => snapped,
        None => topography,
    };
    // A surface that folds over itself in plan cannot say which side of it a
    // point is on, and one that is cracked leaves the solid open. Each is
    // repaired first; where it folds, towards the outside of the solid - the
    // floor keeps its lowest sheet and the roof its highest - so the solid
    // takes in a fold rather than tearing round it.
    let (design_keep, topography_keep) = match region {
        SolidRegion::Cut => (FoldLayer::Lowest, FoldLayer::Highest),
        SolidRegion::Fill => (FoldLayer::Highest, FoldLayer::Lowest),
    };
    let (design_prepared, topography_prepared) = rayon::join(|| single_valued_surface(design, design_keep), || single_valued_surface(&topography, topography_keep));
    // Prepared, the topography holds its own copy; this one is done with.
    drop(topography);
    let mut surfaces = Vec::with_capacity(2);
    for (result, role) in [(design_prepared, tr!("tri-solid-role-design")), (topography_prepared, tr!("tri-solid-role-topography"))] {
        let (prepared, repaired) = result?;
        // Repaired as a matter of course, and on every Solids run of the
        // project, so said in the console rather than as a warning: a
        // warning opens a dialog, and these would open one on every open.
        if repaired {
            notes.push(tr!("tri-solid-repaired-surface", role = role));
        } else if prepared.skipped_vertical_faces > 0 {
            notes.push(tr!("cmd-cuts-ignored-vertical-faces", count = prepared.skipped_vertical_faces.to_string()));
        }
        surfaces.push(prepared);
    }
    let SolidEnvelopes { lower, upper } = build_envelopes(&surfaces[0], &surfaces[1], region, progress);
    // The prepared surfaces are as large as the solid's sheets, and nothing
    // after the clip reads them.
    drop(surfaces);
    // By plan area, not face count: a topography with a hole cut out where the
    // design surface lies meets it only along the hole's rim, and the slivers
    // that leaves there cover nothing and weld away to no faces at all.
    if plan_area(&lower) < MIN_SOLID_PLAN_AREA || plan_area(&upper) < MIN_SOLID_PLAN_AREA {
        anyhow::bail!(no_volume(region));
    }

    let volume = volume_between(&lower, &upper);

    let mut vertices = lower.0;
    let mut faces = lower.1;
    let floor_faces = faces.len();
    let base = vertices.len() as u32;
    vertices.extend(upper.0);
    faces.extend(upper.1.into_iter().map(|face| face.map(|index| base + index)));
    // From here on every point is one index, so the edges are told apart by
    // their indices: one sorted tally answers every later question about them.
    let (vertices, mut faces, floor_faces) = merge_close_points(vertices, faces, floor_faces, SOLID_MERGE, None);
    let weld = Weld::of(&vertices);
    let walls = close_region_sides(&vertices, &edge_uses(&faces, floor_faces), weld);
    faces.extend(walls);
    let patches = fill_flat_holes(&vertices, &edge_uses(&faces, floor_faces));
    faces.extend(patches);
    // A knot of points still open where the sheets part is closed by merging
    // just those, within the weld the surfaces themselves were repaired to.
    let mut uses = edge_uses(&faces, floor_faces);
    let loose: std::collections::HashSet<u32> = uses.iter().filter(|edge| !edge.closes()).flat_map(|edge| [edge.low, edge.high]).collect();
    let (vertices, faces) = if loose.is_empty() {
        (vertices, faces)
    } else {
        let (vertices, mut faces, _) = merge_close_points(vertices, faces, 0, REPAIR_WELD, Some(&loose));
        let patches = fill_flat_holes(&vertices, &edge_uses(&faces, 0));
        faces.extend(patches);
        uses = edge_uses(&faces, 0);
        (vertices, faces)
    };

    // The sheets close against each other along the line where the surfaces
    // cross, and against the wall everywhere else. Anything still open is a
    // rim the wall could not trace - a sheet torn into pieces too small to
    // ring, say - and is worth saying rather than passing off as a solid.
    let open_edges = leaking_edges(&vertices, &uses);
    if open_edges > 0 {
        userspace_warn!("{}", tr!("tri-solid-open-along-edge", count = open_edges.to_string()));
    }

    Ok((vertices, faces, volume))
}

/// Why there is no solid: the surfaces enclose nothing on the wanted side.
fn no_volume(region: SolidRegion) -> String {
    match region {
        SolidRegion::Cut => tr!("tri-solid-no-volume-cut"),
        SolidRegion::Fill => tr!("tri-solid-no-volume-fill"),
    }
}

/// The least plan area, in square model units, two sheets must share to bound
/// a solid; anything less is what rounding leaves along a line.
const MIN_SOLID_PLAN_AREA: f64 = 1.0e-3;

/// The area a sheet covers in plan.
fn plan_area(sheet: &(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)) -> f64 {
    sheet
        .1
        .iter()
        .map(|face| {
            let [a, b, c] = face.map(|index| sheet.0[index as usize]);
            0.5 * ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs()
        })
        .sum()
}

/// The volume between the two sheets, by integrating their height difference
/// over the region they share.
///
/// Both sheets cover exactly the same XY region - they are the two surfaces
/// clipped by the same predicate - so the volume is the roof's integral of z
/// minus the floor's, face by face. Unlike a divergence-theorem sum over the
/// whole mesh this needs no closed surface, so it stays exact for a solid
/// whose sides are open (see [`open_edge_report`]).
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

/// [`edge_uses`] once coincident points are welded.
fn welded_edge_uses(vertices: &[mesh_data::Vertex], faces: &[[u32; 3]]) -> Vec<EdgeUse> {
    use rayon::prelude::*;
    // Welded first: a mesh handed in here may repeat its points, a copy per
    // face, so its indices alone do not say which edges are one.
    let scale = weld_scale(vertices);
    let keys: Vec<PointKey> = vertices.par_iter().map(|vertex| point_key_scaled(*vertex, scale)).collect();
    let mut order: Vec<u32> = (0..vertices.len() as u32).collect();
    order.par_sort_unstable_by_key(|&index| keys[index as usize]);
    let mut welded = vec![0u32; vertices.len()];
    let mut representative = 0;
    for (position, &index) in order.iter().enumerate() {
        if position == 0 || keys[index as usize] != keys[order[position - 1] as usize] {
            representative = index;
        }
        welded[index as usize] = representative;
    }
    let faces: Vec<[u32; 3]> = faces.par_iter().map(|face| face.map(|index| welded[index as usize])).collect();
    edge_uses(&faces, 0)
}

/// Where a mesh fails to close, once coincident vertices are welded: how many
/// of its edges do not close, how long they are end to end, and the middle of
/// the longest; `None` when it closes. Edges that do not close are those used
/// by an odd number of faces, and those used by more than two whose faces do
/// not run them as often one way as the other.
///
/// A closed solid has none. Ones that do appear are the sides: where the
/// region ends because a surface simply runs out - a design that stops short
/// of the ground rather than meeting it - the floor and the roof end at
/// different heights with nothing between them. The volume is still exact
/// there, but the mesh is a shell rather than a solid, which is worth saying.
///
/// An edge four faces share is where two parts of a solid touch along a line,
/// as a wall standing on a crest that pinches against the ground does, and is
/// as closed as any, so long as half its faces run it each way.
///
/// Edges that leak no more than [`LEAK_TOLERANCE`] between them, end to end,
/// are not counted: a hole that small is a rounding error's worth of crack
/// where two surfaces built one from the other were clipped against each
/// other, and holds nothing a volume could measure. A real open side runs
/// the length of a wall.
pub(crate) fn open_edge_report(vertices: &[mesh_data::Vertex], faces: &[[u32; 3]]) -> Option<(usize, f64, mesh_data::Vertex)> {
    let uses = welded_edge_uses(vertices, faces);
    if leaking_edges(vertices, &uses) == 0 {
        return None;
    }
    let ends = |edge: &EdgeUse| (vertices[edge.low as usize], vertices[edge.high as usize]);
    let length = |(a, b): (mesh_data::Vertex, mesh_data::Vertex)| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
    let open: Vec<&EdgeUse> = uses.iter().filter(|edge| !edge.closes()).collect();
    let total = open.iter().map(|edge| length(ends(edge))).sum();
    let (a, b) = ends(open.iter().max_by(|x, y| length(ends(x)).total_cmp(&length(ends(y))))?);
    Some((open.len(), total, mesh_data::Vertex::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0)))
}

/// How many of `uses` do not close, or none when, end to end, they leak no
/// more than [`LEAK_TOLERANCE`]: see [`open_edge_report`].
fn leaking_edges(vertices: &[mesh_data::Vertex], uses: &[EdgeUse]) -> usize {
    let (open, leak) = uses.iter().filter(|edge| !edge.closes()).fold((0, 0.0), |(open, leak), edge| {
        let (a, b) = (vertices[edge.low as usize], vertices[edge.high as usize]);
        (open + 1, leak + ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt())
    });
    if leak <= LEAK_TOLERANCE { 0 } else { open }
}

/// One edge of an indexed mesh and how its faces use it.
#[derive(Clone, Copy)]
struct EdgeUse {
    /// Its ends, the lower index first.
    low: u32,
    high: u32,
    /// How many faces use it.
    count: u32,
    /// How many more of them run it from `low` to `high` than back.
    balance: i32,
    /// The way the first face to use it runs it, and whether that face is
    /// one of the floor's.
    from: u32,
    to: u32,
    floor: bool,
}

impl EdgeUse {
    /// Whether the faces close over it: two of them, or more in balance, as
    /// where two parts of a solid touch along a line.
    fn closes(&self) -> bool {
        self.count == 2 || (self.count.is_multiple_of(2) && self.balance == 0)
    }
}

/// Every edge of `faces` once, with how its faces use it; the first
/// `floor_faces` faces are the floor's. In order of their lower end, then
/// their upper.
///
/// Each use is filed under its edge's lower end, a few to a point, so a tally
/// of tens of millions of edges is one parallel pass and a short sort per
/// point - not one sort of them all, whose first partitions run on one
/// thread - and holds eight bytes a use rather than sixteen.
fn edge_uses(faces: &[[u32; 3]], floor_faces: usize) -> Vec<EdgeUse> {
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    use rayon::prelude::*;
    const RUN: usize = 1 << 14;
    let sides = |corners: &[u32; 3]| {
        [(corners[0], corners[1]), (corners[1], corners[2]), (corners[2], corners[0])]
            .into_iter()
            .filter(|(a, b)| a != b)
    };
    assert!(faces.len() < 1 << 31, "a face's index is packed into 31 bits");
    let points = faces.par_iter().map(|face| face.iter().copied().max().unwrap_or(0)).max().map_or(0, |max| max as usize + 1);
    // Where each point's uses start: counted, then summed.
    let counts: Vec<AtomicU32> = (0..points).map(|_| AtomicU32::new(0)).collect();
    faces.par_chunks(RUN).for_each(|run| {
        for corners in run {
            for (a, b) in sides(corners) {
                counts[a.min(b) as usize].fetch_add(1, Ordering::Relaxed);
            }
        }
    });
    let mut starts: Vec<usize> = Vec::with_capacity(points + 1);
    let mut total = 0;
    starts.push(0);
    for count in &counts {
        total += count.load(Ordering::Relaxed) as usize;
        starts.push(total);
    }
    // Each use as its upper end, face and direction, packed so that sorting
    // a point's uses orders them as the edges are listed.
    let cursors: Vec<AtomicU32> = counts;
    cursors.par_iter().for_each(|cursor| cursor.store(0, Ordering::Relaxed));
    let filed: Vec<AtomicU64> = (0..total).map(|_| AtomicU64::new(0)).collect();
    faces.par_chunks(RUN).enumerate().for_each(|(chunk, run)| {
        for (offset, corners) in run.iter().enumerate() {
            let face = (chunk * RUN + offset) as u64;
            for (a, b) in sides(corners) {
                let low = a.min(b) as usize;
                let slot = starts[low] + cursors[low].fetch_add(1, Ordering::Relaxed) as usize;
                filed[slot].store((u64::from(a.max(b)) << 32) | (face << 1) | u64::from(a < b), Ordering::Relaxed);
            }
        }
    });
    drop(cursors);
    let mut filed: Vec<u64> = filed.into_iter().map(AtomicU64::into_inner).collect();
    // Split into runs of points, each sorted and tallied on its own.
    let mut runs: Vec<(usize, &mut [u64])> = Vec::with_capacity(points.div_ceil(RUN));
    let mut rest = filed.as_mut_slice();
    for first in (0..points).step_by(RUN) {
        let last = (first + RUN).min(points);
        let (here, after) = rest.split_at_mut(starts[last] - starts[first]);
        runs.push((first, here));
        rest = after;
    }
    let starts = &starts;
    runs.into_par_iter()
        .flat_map_iter(|(first, filed)| {
            let mut uses = Vec::new();
            for low in first..(first + RUN).min(points) {
                let filed = &mut filed[starts[low] - starts[first]..starts[low + 1] - starts[first]];
                filed.sort_unstable();
                let low = low as u32;
                for group in filed.chunk_by(|left, right| left >> 32 == right >> 32) {
                    let high = (group[0] >> 32) as u32;
                    let face = ((group[0] >> 1) & 0x7fff_ffff) as usize;
                    let forward = group[0] & 1 == 1;
                    let balance = group.iter().map(|run| if run & 1 == 1 { 1 } else { -1 }).sum();
                    let (from, to) = if forward { (low, high) } else { (high, low) };
                    uses.push(EdgeUse {
                        low,
                        high,
                        count: group.len() as u32,
                        balance,
                        from,
                        to,
                        floor: face < floor_faces,
                    });
                }
            }
            uses
        })
        .collect()
}

/// The most open edge, end to end, a mesh may have and still be closed: see
/// [`open_edge_report`].
const LEAK_TOLERANCE: f64 = REPAIR_WELD;

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
            let solid = build_solid_mesh(&design_mesh, &topography_mesh, region, "", &progress.phase(0.0, 0.95))?;
            if cancel.is_cancelled() {
                anyhow::bail!("Cancelled");
            }
            let volume = solid.volume;
            let generated = crate::model::triangulation::GeneratedTriangulation {
                name,
                mesh: solid.mesh,
                spatial: solid.spatial,
                edges: std::sync::Arc::unwrap_or_clone(solid.edges),
                surface_face_order: solid.surface_face_order,
                surface_type: TriSurfaceType::SolidClosed,
            };
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
/// [`build_solid_mesh`] emits) has rims that do not close, and
/// those are left uncapped rather than guessed at.
///
/// Bands are expected in ascending order and must not overlap. A face only
/// touches the bands its own height range reaches, so cutting every bench of a
/// pit at once costs one walk of the mesh rather than one per bench.
pub(crate) fn slabs_between_elevations(mesh: &mesh_data::Triangulation, bands: &[(f64, f64)], cancel: Option<&crate::app::jobs::CancelFlag>) -> Result<Vec<Slab>> {
    if bands.iter().any(|(base, top)| !base.is_finite() || !top.is_finite() || base >= top) || bands.windows(2).any(|pair| pair[0].1 > pair[1].0 + 1e-6) {
        anyhow::bail!("Bench bands must be finite, ascending and non-overlapping");
    }
    use std::sync::atomic::{AtomicU32, Ordering};

    use rayon::prelude::*;
    // A point a rounding error off a cut plane - a crest built at a bench's
    // own elevation, say - is put on it. Left a hair to either side, the cut
    // trims slivers off the faces meeting there, too thin to keep and too
    // long to drop without cracking the rim.
    let tolerance = Weld::of(mesh.vertices()).tolerance;
    let mut planes: Vec<f64> = bands.iter().flat_map(|&(base, top)| [base, top]).collect();
    planes.sort_by(f64::total_cmp);
    let source: Vec<mesh_data::Vertex> = mesh
        .vertices()
        .par_iter()
        .map(|vertex| {
            let next = planes.partition_point(|plane| *plane < vertex.z);
            let nearest = [next.checked_sub(1), Some(next)]
                .into_iter()
                .flatten()
                .filter_map(|index| planes.get(index))
                .copied()
                .find(|plane| (plane - vertex.z).abs() <= tolerance);
            nearest.map_or(*vertex, |plane| mesh_data::Vertex::new(vertex.x, vertex.y, plane))
        })
        .collect();
    let faces: Vec<[u32; 3]> = mesh.face_indices().collect();
    let triangle = |face: usize| faces[face].map(|index| source[index as usize]);
    // The bands each face's height range reaches.
    let reach = |face: usize| {
        let triangle = triangle(face);
        let lowest = triangle[0].z.min(triangle[1].z).min(triangle[2].z);
        let highest = triangle[0].z.max(triangle[1].z).max(triangle[2].z);
        let first = bands.partition_point(|(_, top)| *top < lowest);
        first..first + bands[first..].partition_point(|(base, _)| *base <= highest)
    };
    // Each band's faces, filed in parallel and put back in order, so each
    // band is cut on its own - side by side - from its faces as they come.
    let counts: Vec<AtomicU32> = bands.iter().map(|_| AtomicU32::new(0)).collect();
    (0..faces.len()).into_par_iter().for_each(|face| {
        for band in reach(face) {
            counts[band].fetch_add(1, Ordering::Relaxed);
        }
    });
    let mut starts = Vec::with_capacity(bands.len() + 1);
    starts.push(0usize);
    for count in &counts {
        starts.push(starts.last().copied().unwrap_or(0) + count.load(Ordering::Relaxed) as usize);
    }
    counts.par_iter().for_each(|count| count.store(0, Ordering::Relaxed));
    let filed: Vec<AtomicU32> = (0..starts[bands.len()]).map(|_| AtomicU32::new(0)).collect();
    (0..faces.len()).into_par_iter().for_each(|face| {
        for band in reach(face) {
            let slot = starts[band] + counts[band].fetch_add(1, Ordering::Relaxed) as usize;
            filed[slot].store(face as u32, Ordering::Relaxed);
        }
    });
    drop(counts);
    let mut filed: Vec<u32> = filed.into_iter().map(AtomicU32::into_inner).collect();
    let mut by_band: Vec<&mut [u32]> = Vec::with_capacity(bands.len());
    let mut rest = filed.as_mut_slice();
    for band in 0..bands.len() {
        let (here, after) = rest.split_at_mut(starts[band + 1] - starts[band]);
        by_band.push(here);
        rest = after;
    }
    by_band
        .into_par_iter()
        .zip(bands)
        .map(|(band_faces, &(base, top))| -> Result<Slab> {
            band_faces.sort_unstable();
            // Welded as it is cut, one vertex per bit-identical position in
            // the order they come: a survey's slabs as loose triangles cost
            // three times the memory. See [`cap_slab`] for why that is the
            // same slab.
            let mut seen: std::collections::HashMap<[u64; 3], u32, FixedState> = std::collections::HashMap::default();
            let mut slab: Slab = (Vec::new(), Vec::new());
            for (count, &face) in band_faces.iter().enumerate() {
                if count % 1024 == 0 && cancel.is_some_and(|cancel| cancel.is_cancelled()) {
                    anyhow::bail!("Cancelled");
                }
                let triangle = triangle(face as usize);
                let lowest = triangle[0].z.min(triangle[1].z).min(triangle[2].z);
                let highest = triangle[0].z.max(triangle[1].z).max(triangle[2].z);
                if top <= base || highest < base || lowest > top {
                    continue;
                }
                // A horizontal boundary face belongs only to the slab on its
                // material side. Copying it into both bands creates a
                // zero-thickness sheet which cap triangulation can cover a
                // second time.
                if highest - lowest < 1e-8 {
                    let normal_z = (triangle[1].x - triangle[0].x) * (triangle[2].y - triangle[0].y) - (triangle[1].y - triangle[0].y) * (triangle[2].x - triangle[0].x);
                    if ((lowest - base).abs() < 1e-8 && normal_z > 0.0) || ((highest - top).abs() < 1e-8 && normal_z < 0.0) {
                        continue;
                    }
                }
                for clipped in super::cuts::clip_triangle_z(triangle, base, top) {
                    if collapsed(&clipped) {
                        continue;
                    }
                    slab.1.push(clipped.map(|vertex| {
                        *seen.entry([vertex.x.to_bits(), vertex.y.to_bits(), vertex.z.to_bits()]).or_insert_with(|| {
                            slab.0.push(vertex);
                            (slab.0.len() - 1) as u32
                        })
                    }));
                }
            }
            drop(seen);
            if slab.1.is_empty() {
                return Ok(slab);
            }
            if cancel.is_some_and(|cancel| cancel.is_cancelled()) {
                anyhow::bail!("Cancelled");
            }
            cap_slab(&mut slab, base, top);
            Ok(slab)
        })
        .collect::<Result<Vec<Slab>>>()
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
///
/// The slab comes welded by bit-identical position, its points in the order
/// its faces first use them. Everything here reads points by position, so
/// that is the same slab as one with a copy of each corner per face, and the
/// weld after capping gives the same mesh - except where the rims do not
/// close, which is put back to the copies first.
fn cap_slab(slab: &mut (Vec<mesh_data::Vertex>, Vec<[u32; 3]>), base: f64, top: f64) {
    let weld = Weld::of(&slab.0);
    // Only points on a cut plane can be a rim's, and only faces with two of
    // their corners on one can own a rim edge - every face using an edge in
    // the plane has both its ends there. Looking at those alone keeps a cut
    // through a large body from hashing every face of it. Each point is
    // classed once: 1 on the base, 2 on the top, 0 on neither.
    let classify = |vertices: &[mesh_data::Vertex]| -> Vec<u8> {
        vertices
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
            .collect()
    };
    let rim_faces = |faces: &[[u32; 3]], plane_of: &[u8]| -> Vec<[u32; 3]> {
        faces
            .iter()
            .copied()
            .filter(|face| {
                let [a, b, c] = face.map(|index| plane_of[index as usize]);
                (a != 0 && (a == b || a == c)) || (b != 0 && b == c)
            })
            .collect()
    };
    let plane_of = classify(&slab.0);
    let edge_faces = rim_faces(&slab.1, &plane_of);
    let mut rims = boundary_segments(&slab.0, &edge_faces);
    // Points are merged only where the rims do not already close. A cut can
    // leave distinct rim points a hair apart, and merging those pinches a
    // closed rim into one that no longer traces as a ring - the cap is lost
    // and every later cut through the body inherits the hole.
    let mut edge_faces = edge_faces;
    if !rims_close(&rims, &[base, top], weld) {
        // Merging moves each point onto the first kept one within reach, a
        // copy at a time, and two copies of one point can come to rest on
        // different ones: the slab goes back to a copy of each corner per
        // face, as it was cut, so the merge is what it always was.
        *slab = (
            slab.1.iter().flatten().map(|&index| slab.0[index as usize]).collect(),
            (0..slab.1.len() as u32).map(|face| [3 * face, 3 * face + 1, 3 * face + 2]).collect(),
        );
        let plane_of = classify(&slab.0);
        edge_faces = rim_faces(&slab.1, &plane_of);
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
        let (rings, capped) = match directed_cap_rings(&rim, &sides, weld) {
            Some((directed, seams)) => {
                for seam in seams {
                    let base = slab.0.len() as u32;
                    slab.0.extend_from_slice(&seam);
                    slab.1.push([base, base + 1, base + 2]);
                }
                directed.into_iter().unzip()
            }
            None => {
                let rings = planar_cap_rings(&rim, weld);
                let capped: Vec<bool> = rings.iter().map(|ring| ring_is_material(ring, &sides, weld)).collect();
                (rings, capped)
            }
        };
        append_caps(&rings, &capped, plane, upwards, &mut slab.0, &mut slab.1);
    }
}

/// Whether every rim point on `planes` meets an even number of rim edges,
/// so the rims there are closed loops.
fn rims_close(rims: &[[mesh_data::Vertex; 2]], planes: &[f64], weld: Weld) -> bool {
    use std::collections::HashMap;
    let mut degree: HashMap<PointKey, usize, FixedState> = HashMap::default();
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
    let mut cells: HashMap<PointKey, Vec<mesh_data::Vertex>, FixedState> = HashMap::default();
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
type CapSides = std::collections::HashMap<EdgeKey, (PointKey, PointKey), FixedState>;

/// For each rim edge - an edge only one face uses - the direction along it
/// that has the solid, and so the cap, on its left.
///
/// The face that owns the edge says so without looking anywhere else: the
/// solid is on the side its outward normal points away from, read in plan; a
/// face lying flat in the plane has the cap on the other side of it, since
/// one cannot cover the other.
fn cap_sides(vertices: &[mesh_data::Vertex], faces: &[[u32; 3]], weld: Weld) -> CapSides {
    use std::collections::HashMap;
    let mut owners: HashMap<EdgeKey, (usize, PointKey, PointKey, bool), FixedState> = HashMap::default();
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
    // A wall's few faces, a few dozen at most, are matched up faster sorted
    // in a list than hashed into a table built for each one.
    if faces.len() <= 512 {
        let mut sides: Vec<(EdgeKey, [mesh_data::Vertex; 2])> = Vec::with_capacity(faces.len() * 3);
        for face in faces {
            let corners = face.map(|index| vertices[index as usize]);
            for (a, b) in [(corners[0], corners[1]), (corners[1], corners[2]), (corners[2], corners[0])] {
                let (ka, kb) = (key(a), key(b));
                sides.push((if ka <= kb { (ka, kb) } else { (kb, ka) }, [a, b]));
            }
        }
        sides.sort_by_key(|(key, _)| *key);
        return sides
            .chunk_by(|left, right| left.0 == right.0)
            .filter(|group| group.len() == 1)
            .map(|group| group[0].1)
            .collect();
    }
    let mut counts: HashMap<EdgeKey, (usize, [mesh_data::Vertex; 2]), FixedState> = HashMap::default();
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
    // On the body's own points, each flattened once and in the order the
    // faces first use it: a body cut from a survey shares most of them, and
    // three to every face would be as many again to merge and hash.
    let mut flattened = vec![u32::MAX; source.len()];
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
        faces.push(face.map(|index| {
            if flattened[index] == u32::MAX {
                flattened[index] = vertices.len() as u32;
                vertices.push(mesh_data::Vertex::new(source[index].x, source[index].y, plane));
            }
            flattened[index]
        }));
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
/// Judged by a point strictly inside each ring and next to its own edge
/// ([`boundary_probe`]), never by one of its corners:
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

/// A point strictly inside `rings[index]`, next to its own boundary: in from
/// the middle of one of its edges, half way to the nearest boundary of any
/// ring. `None` for a ring with no area.
///
/// Inside a ring is not enough. Nesting is read by which rings hold the
/// point, and a point well inside a large outline - the centre of its largest
/// triangle, say - can as well be inside one of its holes, which made the
/// outline read as nested, left its holes unattached, and laid the cap over
/// them. A point this close to the ring's own edge is held by exactly the
/// rings that hold that stretch of edge.
fn boundary_probe(index: usize, rings: &[Vec<mesh_data::Vertex>]) -> Option<mesh_data::Vertex> {
    let ring = &rings[index];
    if ring.len() < 3 {
        return None;
    }
    let area = ring_signed_area(ring);
    if area == 0.0 {
        return None;
    }
    let point = |vertex: mesh_data::Vertex| glam::DVec2::new(vertex.x, vertex.y);
    let segment_distance = |p: glam::DVec2, a: glam::DVec2, b: glam::DVec2| {
        let span = b - a;
        let t = ((p - a).dot(span) / span.length_squared().max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
        p.distance(a + span * t)
    };
    let boxes: Vec<[f64; 4]> = rings.iter().map(|ring| ring_box(ring)).collect();
    // The longest edges first: the most room beside them.
    let mut edges: Vec<usize> = (0..ring.len()).collect();
    edges.sort_by(|&x, &y| {
        let length = |at: usize| point(ring[at]).distance(point(ring[(at + 1) % ring.len()]));
        length(y).total_cmp(&length(x))
    });
    for &edge in edges.iter().take(8) {
        let (a, b) = (point(ring[edge]), point(ring[(edge + 1) % ring.len()]));
        let length = a.distance(b);
        if length == 0.0 {
            continue;
        }
        let middle = (a + b) / 2.0;
        // Inward is to the left of an anticlockwise ring, right of a
        // clockwise one.
        let inward = (b - a).perp() / length * area.signum();
        let mut clearance = length / 2.0;
        for (other, candidate) in rings.iter().enumerate() {
            let bounds = boxes[other];
            let outside = (bounds[0] - middle.x).max(middle.x - bounds[2]).max(bounds[1] - middle.y).max(middle.y - bounds[3]);
            if outside > clearance {
                continue;
            }
            for at in 0..candidate.len() {
                if other == index && at == edge {
                    continue;
                }
                clearance = clearance.min(segment_distance(middle, point(candidate[at]), point(candidate[(at + 1) % candidate.len()])));
            }
        }
        if clearance > 0.0 {
            let probe = middle + inward * (clearance / 2.0);
            return Some(mesh_data::Vertex::new(probe.x, probe.y, ring[0].z));
        }
    }
    None
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
    // needs no point found inside it.
    let probes: Vec<_> = if rings.len() > 1 {
        (0..rings.len()).map(|index| boundary_probe(index, rings)).collect()
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
    skipped: HashMap<(usize, usize), Vec<usize>, FixedState>,
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

/// Points of an assembled solid closer than this, in model units, are one.
/// Where the two surfaces were built one from the other, clipping each against
/// the other leaves clusters of points microns apart at the corners where they
/// part; no weld at rounding scale will join them.
const SOLID_MERGE: f64 = 1.0e-4;

/// Merge the points of `vertices` within `distance` of each other - of those
/// in `only`, when given - and drop the faces that collapse. The floor's
/// faces, the first `floor_faces`, stay first, and how many are left is said.
/// Two faces that collapse onto each other facing opposite ways are a sliver
/// closed flat, and both go.
fn merge_close_points(
    vertices: Vec<mesh_data::Vertex>,
    faces: Vec<[u32; 3]>,
    floor_faces: usize,
    distance: f64,
    only: Option<&std::collections::HashSet<u32>>,
) -> (Vec<mesh_data::Vertex>, Vec<[u32; 3]>, usize) {
    use std::collections::HashMap;

    use rayon::prelude::*;
    // Cells four distances wide: the points within reach of one lie in its
    // own cell, and only near a cell's side in the next one too, so most
    // points look in one cell rather than twenty-seven.
    let size = distance * 4.0;
    let cell = move |value: f64| (value / size).floor() as i64;
    let span = move |value: f64| cell(value - distance)..=cell(value + distance);
    // Most points repeat one exactly - a copy for each face meeting there.
    // A parallel sort finds every repeat at once, and each takes whatever
    // its first copy becomes, so the search below sees each place once.
    let first_copy = crate::model::triangulation::first_of_equal(vertices.len(), |index| {
        let vertex = vertices[index];
        (vertex.x.to_bits(), vertex.y.to_bits(), vertex.z.to_bits())
    });

    // The points the search below is made for, and of those, the ones with no
    // other within reach. One of those is kept on its own whatever comes
    // before it, and nothing after it can join it, since reach is the same
    // both ways: the search skips it and leaves it out of the cells, which
    // for a survey's points is nearly all of them. Found side by side.
    let in_only = |index: usize| only.is_none_or(|only| only.contains(&(index as u32)));
    let searched = |index: usize| in_only(index) && (first_copy[index] as usize == index || only.is_some_and(|only| !only.contains(&first_copy[index])));
    let lonely = {
        use std::{
            hash::BuildHasher,
            sync::atomic::{AtomicU32, Ordering},
        };
        const EMPTY: u32 = u32::MAX;
        let key = |index: u32| {
            let vertex = vertices[index as usize];
            (cell(vertex.x), cell(vertex.y), cell(vertex.z))
        };
        let points: Vec<u32> = (0..vertices.len() as u32).into_par_iter().filter(|&index| searched(index as usize)).collect();
        // Every point filed by its cell in one open table, at most half full,
        // filled and read side by side.
        let slots = (points.len() * 2).next_power_of_two().max(2);
        let hasher = FixedState::default();
        let home = |key: PointKey| hasher.hash_one(key) as usize & (slots - 1);
        let table: Vec<AtomicU32> = (0..slots).map(|_| AtomicU32::new(EMPTY)).collect();
        points.par_iter().for_each(|&index| {
            let mut slot = home(key(index));
            while table[slot].compare_exchange(EMPTY, index, Ordering::Relaxed, Ordering::Relaxed).is_err() {
                slot = (slot + 1) & (slots - 1);
            }
        });
        let alone: Vec<u32> = points
            .into_par_iter()
            .filter(|&index| {
                let vertex = vertices[index as usize];
                !span(vertex.x)
                    .flat_map(|x| span(vertex.y).flat_map(move |y| span(vertex.z).map(move |z| (x, y, z))))
                    .any(|cell| {
                        let mut slot = home(cell);
                        loop {
                            let other_index = table[slot].load(Ordering::Relaxed);
                            if other_index == EMPTY {
                                return false;
                            }
                            if other_index != index && key(other_index) == cell {
                                let other = vertices[other_index as usize];
                                if (other.x - vertex.x).powi(2) + (other.y - vertex.y).powi(2) + (other.z - vertex.z).powi(2) <= distance * distance {
                                    return true;
                                }
                            }
                            slot = (slot + 1) & (slots - 1);
                        }
                    })
            })
            .collect();
        drop(table);
        let mut lonely = vec![false; vertices.len()];
        for index in alone {
            lonely[index as usize] = true;
        }
        lonely
    };

    let mut cells: HashMap<PointKey, Vec<u32>, FixedState> = HashMap::default();
    let mut kept: Vec<mesh_data::Vertex> = Vec::with_capacity(vertices.len() / 2);
    let mut remap: Vec<u32> = vec![0; vertices.len()];
    for (index, vertex) in vertices.iter().enumerate() {
        remap[index] = if only.is_some_and(|only| !only.contains(&(index as u32))) {
            kept.push(*vertex);
            (kept.len() - 1) as u32
        } else if first_copy[index] as usize != index && only.is_none_or(|only| only.contains(&first_copy[index])) {
            remap[first_copy[index] as usize]
        } else if lonely[index] {
            kept.push(*vertex);
            (kept.len() - 1) as u32
        } else {
            let found = span(vertex.x)
                .flat_map(|x| span(vertex.y).flat_map(move |y| span(vertex.z).map(move |z| (x, y, z))))
                .filter_map(|key| cells.get(&key))
                .flatten()
                .copied()
                .find(|&index| {
                    let other = kept[index as usize];
                    (other.x - vertex.x).powi(2) + (other.y - vertex.y).powi(2) + (other.z - vertex.z).powi(2) <= distance * distance
                });
            found.unwrap_or_else(|| {
                kept.push(*vertex);
                let index = (kept.len() - 1) as u32;
                cells.entry((cell(vertex.x), cell(vertex.y), cell(vertex.z))).or_default().push(index);
                index
            })
        };
    }
    drop(cells);
    drop(first_copy);
    drop(vertices);

    // Faces that collapse go. Of faces left on the same three points, those
    // facing opposite ways pair off and go too, earliest kept: a sliver
    // closed flat.
    let mut merged: Vec<(u32, [u32; 3], [u32; 3], bool)> = faces
        .par_iter()
        .enumerate()
        .filter_map(|(index, face)| {
            let face = face.map(|corner| remap[corner as usize]);
            if face[0] == face[1] || face[1] == face[2] || face[2] == face[0] {
                return None;
            }
            let mut sorted = face;
            sorted.sort_unstable();
            Some((index as u32, sorted, face, index < floor_faces))
        })
        .collect();
    // Faces on the same three points, each group in order: the first of each
    // is found in parallel, and only the few that repeat one are gathered.
    let first_on_points = crate::model::triangulation::first_of_equal(merged.len(), |position| merged[position].1);
    let mut groups: std::collections::HashMap<u32, Vec<u32>, FixedState> = std::collections::HashMap::default();
    for (position, first) in first_on_points.iter().enumerate() {
        if *first as usize != position {
            groups.entry(*first).or_insert_with(|| vec![*first]).push(position as u32);
        }
    }
    drop(first_on_points);
    let mut dropped = vec![false; merged.len()];
    for group in groups.values() {
        // Even: the same way round as the sorted points.
        let even = |position: u32| {
            let (sorted, face) = (merged[position as usize].1, merged[position as usize].2);
            let start = face.iter().position(|&corner| corner == sorted[0]).unwrap_or(0);
            face[(start + 1) % 3] == sorted[1]
        };
        let (forward, backward): (Vec<u32>, Vec<u32>) = group.iter().partition(|&&position| even(position));
        for (&a, &b) in forward.iter().zip(&backward) {
            dropped[a as usize] = true;
            dropped[b as usize] = true;
        }
    }
    let mut index = 0;
    merged.retain(|_| {
        index += 1;
        !dropped[index - 1]
    });
    let floor = merged.iter().filter(|(.., floor)| *floor).count();
    // The floor's faces stay first, as they came.
    merged.sort_by_key(|&(index, .., floor)| (!floor, index));
    (kept, merged.into_iter().map(|(_, _, face, _)| face).collect(), floor)
}

/// Close the sides of the region with a vertical wall between the two sheets,
/// an edge at a time, and any step a fold left within one. `faces` holds the
/// floor's faces first, the first `floor_faces` of them, then the roof's.
///
/// The floor and roof meet on their own along the line where the surfaces
/// cross - both clips stop there, at the same points - but nowhere else.
/// Where the region ends because a surface simply runs out inside the other's
/// footprint, or stands up in a wall of its own, the two sheets end one above
/// the other with a gap between them, and that gap is what this fills.
///
/// Both rims are cut from the same overlaps of design and topography faces,
/// so they share their points in plan, and each open floor edge has an open
/// roof edge over exactly the same ground: the quad between the two closes
/// that much of the side. Pairing edges rather than whole rims means a rim
/// that touches itself - a crest pinching against the ground, where the
/// floor's passes meet at one point and the roof's at two heights - needs no
/// deciding which way round it goes.
fn close_region_sides(vertices: &[mesh_data::Vertex], uses: &[EdgeUse], weld: Weld) -> Vec<[u32; 3]> {
    use std::collections::HashMap;
    let key = |index: u32| weld.key(vertices[index as usize]);
    let plan = |index: u32| {
        let (x, y, _) = key(index);
        (x, y)
    };
    // Open edges by their ends in plan. The floor is wound facing down and the
    // roof facing up, so the two walk a shared rim opposite ways; so do the
    // two sides of a step within one sheet, where a fold left a wall.
    type PlanEdge = ((i64, i64), (i64, i64));
    let mut open: Vec<(u32, u32, bool)> = uses.iter().filter(|edge| edge.count == 1).map(|edge| (edge.from, edge.to, edge.floor)).collect();
    // Floor edges first, so a floor rim pairs with the roof over it before
    // anything else over the same ground.
    open.sort_unstable_by_key(|&(a, b, floor)| (!floor, key(a), key(b)));
    let mut waiting: HashMap<PlanEdge, Vec<(u32, u32)>, FixedState> = HashMap::default();
    for &(a, b, _) in open.iter().rev() {
        waiting.entry((plan(a), plan(b))).or_default().push((a, b));
    }
    let mut curtain = Vec::new();
    for (a, b, _) in open {
        // Taken already, as the other side of an earlier pair.
        let Some(position) = waiting.get(&(plan(a), plan(b))).and_then(|edges| edges.iter().position(|&edge| edge == (a, b))) else {
            continue;
        };
        // Edge a -> b facing edge c -> d over the same ground, c over b and
        // d over a, at another height.
        let Some(&(c, d)) = waiting
            .get(&(plan(b), plan(a)))
            .and_then(|edges| edges.iter().find(|&&(c, d)| key(c) != key(b) || key(d) != key(a)))
        else {
            continue;
        };
        if let Some(edges) = waiting.get_mut(&(plan(a), plan(b))) {
            edges.remove(position);
        }
        if let Some(edges) = waiting.get_mut(&(plan(b), plan(a))) {
            edges.retain(|&edge| edge != (c, d));
        }
        for face in [[b, a, d], [b, d, c]] {
            let corners = face.map(|index| vertices[index as usize]);
            // Where floor and roof meet at a corner the quad narrows to one
            // triangle; the other has no area and is not emitted.
            if weld.same(corners[0], corners[1]) || weld.same(corners[1], corners[2]) || weld.same(corners[2], corners[0]) {
                continue;
            }
            curtain.push(face);
        }
    }
    curtain
}

/// Close the holes left in a mesh that are flat: a loop of open edges lying
/// in one plane to within a weld, of at most [`FLAT_HOLE_POINTS`] points.
///
/// What [`close_region_sides`] cannot pair is where the floor's rim and the
/// roof's break at points a hair apart, which leaves a sliver of wall - flat,
/// a hair wide, and as tall as the wall. Filling a hole that is not flat would
/// be inventing a surface, so those are left for the open-edge warning.
fn fill_flat_holes(vertices: &[mesh_data::Vertex], uses: &[EdgeUse]) -> Vec<[u32; 3]> {
    use std::collections::HashMap;
    // Each open edge, run the way the faces it is short of a partner wind it:
    // an edge two faces run one way and one the other is open once.
    let open: Vec<(u32, u32)> = uses
        .iter()
        .filter(|edge| edge.count != 2 && edge.balance != 0)
        .flat_map(|edge| {
            std::iter::repeat_n(
                if edge.balance > 0 { (edge.low, edge.high) } else { (edge.high, edge.low) },
                edge.balance.unsigned_abs() as usize,
            )
        })
        .collect();
    let mut outgoing: HashMap<u32, Vec<usize>, FixedState> = HashMap::default();
    for (index, &(a, _)) in open.iter().enumerate() {
        outgoing.entry(a).or_default().push(index);
    }

    let mut used = vec![false; open.len()];
    let mut patches = Vec::new();
    for start in 0..open.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut ring = vec![open[start].0];
        let mut current = start;
        let closed = loop {
            let at = open[current].1;
            if at == open[start].0 {
                break true;
            }
            let Some(&next) = outgoing.get(&at).into_iter().flatten().find(|&&edge| !used[edge]) else {
                break false;
            };
            used[next] = true;
            ring.push(open[next].0);
            current = next;
            if ring.len() > FLAT_HOLE_POINTS {
                break false;
            }
        };
        if closed && ring.len() >= 3 {
            // The patch runs each open edge the other way, as its neighbour
            // across the hole would.
            ring.reverse();
            patches.extend(fill_flat_ring(vertices, &ring, SOLID_MERGE));
        }
    }
    patches
}

/// The most points a hole [`fill_flat_holes`] fills may have.
const FLAT_HOLE_POINTS: usize = 64;

/// Ear-clip `ring` in its own plane, keeping its winding, or nothing when it
/// is not flat to within `flatness`. A ring along one line is fanned flat.
fn fill_flat_ring(vertices: &[mesh_data::Vertex], ring: &[u32], flatness: f64) -> Vec<[u32; 3]> {
    let point = |index: u32| {
        let vertex = vertices[index as usize];
        glam::DVec3::new(vertex.x, vertex.y, vertex.z)
    };
    let centre = ring.iter().map(|&index| point(index)).sum::<glam::DVec3>() / ring.len() as f64;
    // Newell's normal: sound for a loop that is thin or not quite flat.
    let normal = (0..ring.len())
        .map(|index| (point(ring[index]) - centre).cross(point(ring[(index + 1) % ring.len()]) - centre))
        .sum::<glam::DVec3>();
    let Some(normal) = normal.try_normalize() else {
        // A loop with no area is points along one line - an edge two others
        // were split from - and is closed by flat triangles along it.
        return (1..ring.len() - 1).map(|index| [ring[0], ring[index], ring[index + 1]]).collect();
    };
    if ring.iter().any(|&index| (point(index) - centre).dot(normal).abs() > flatness) {
        return Vec::new();
    }
    let u = normal.any_orthonormal_vector();
    let v = normal.cross(u);
    let flat: Vec<glam::DVec2> = ring
        .iter()
        .map(|&index| point(index) - centre)
        .map(|offset| glam::DVec2::new(offset.dot(u), offset.dot(v)))
        .collect();

    // In this frame the ring winds anticlockwise, since `normal` is its own.
    let mut remaining: Vec<usize> = (0..ring.len()).collect();
    let mut triangles = Vec::with_capacity(ring.len() - 2);
    while remaining.len() > 3 {
        let count = remaining.len();
        let ear = (0..count).find(|&slot| {
            let (a, b, c) = (remaining[(slot + count - 1) % count], remaining[slot], remaining[(slot + 1) % count]);
            let (pa, pb, pc) = (flat[a], flat[b], flat[c]);
            (pb - pa).perp_dot(pc - pb) > 0.0
                && remaining.iter().all(|&other| {
                    other == a || other == b || other == c || {
                        let p = flat[other];
                        (pb - pa).perp_dot(p - pa) < 0.0 || (pc - pb).perp_dot(p - pb) < 0.0 || (pa - pc).perp_dot(p - pc) < 0.0
                    }
                })
        });
        let Some(slot) = ear else {
            return Vec::new();
        };
        let (a, b, c) = (remaining[(slot + count - 1) % count], remaining[slot], remaining[(slot + 1) % count]);
        triangles.push([ring[a], ring[b], ring[c]]);
        remaining.remove(slot);
    }
    triangles.push([ring[remaining[0]], ring[remaining[1]], ring[remaining[2]]]);
    triangles
}

/// The rings of a cut plane's rim, each anticlockwise and saying whether it
/// encloses solid, traced from which side of each rim edge the solid lies on
/// ([`cap_sides`]); `None` when some rim edge does not say, or a ring does not
/// close, so the caller can trace it the older way.
///
/// With the solid kept on the left of every edge, an outline runs
/// anticlockwise and a hole clockwise, so neither the walk nor the nesting
/// needs to guess. A solid can carry a fin - two faces back to back along a
/// line, where surfaces built one from the other part company - and a cut
/// through it leaves a slit: rim run out along a line and straight back,
/// with solid on both sides. Splitting each rim edge at the rim points lying
/// along it lines the two passes up edge for edge, and run opposite ways
/// they cancel, as the slit is not an edge of the cap at all.
///
/// A rim edge split that way still runs whole along its own face, while the
/// cap runs its pieces. Each comes back with a seam: flat triangles fanned
/// from the edge's start along its points, wound against its face, which use
/// the whole edge once and each piece once, so the slab stays closed. They
/// lie on the edge and enclose nothing.
fn directed_cap_rings(rim: &[[mesh_data::Vertex; 2]], sides: &CapSides, weld: Weld) -> Option<CapRings> {
    use std::collections::HashMap;
    let mut points: HashMap<PointKey, mesh_data::Vertex, FixedState> = HashMap::default();
    // Each rim edge with the solid on its left, and whether its face runs it
    // that way too.
    let mut directed: Vec<(PointKey, PointKey, bool)> = Vec::with_capacity(rim.len());
    for [a, b] in rim {
        let (ka, kb) = (weld.key(*a), weld.key(*b));
        if ka == kb {
            continue;
        }
        points.entry(ka).or_insert(*a);
        points.entry(kb).or_insert(*b);
        let (from, to) = *sides.get(&if ka <= kb { (ka, kb) } else { (kb, ka) })?;
        directed.push((from, to, from == ka));
    }

    // Rim points by plan cell, to find those lying along each edge.
    let size =
        (directed.iter().map(|(a, b, _)| (points[a].x - points[b].x).hypot(points[a].y - points[b].y)).sum::<f64>() / directed.len().max(1) as f64).max(weld.tolerance * 16.0);
    let cell = |point: mesh_data::Vertex| ((point.x / size).floor() as i64, (point.y / size).floor() as i64);
    let mut cells: HashMap<(i64, i64), Vec<PointKey>, FixedState> = HashMap::default();
    for (key, point) in &points {
        cells.entry(cell(*point)).or_default().push(*key);
    }
    let reach = weld.tolerance * 16.0;
    let mut balance: HashMap<(PointKey, PointKey), isize, FixedState> = HashMap::default();
    let mut along: Vec<(f64, PointKey)> = Vec::new();
    let mut seams = Vec::new();
    for (from, to, face_runs_it) in directed {
        let (a, b) = (points[&from], points[&to]);
        let span = glam::DVec2::new(b.x - a.x, b.y - a.y);
        let length = span.length();
        let (lo, hi) = (
            cell(mesh_data::Vertex::new(a.x.min(b.x) - reach, a.y.min(b.y) - reach, 0.0)),
            cell(mesh_data::Vertex::new(a.x.max(b.x) + reach, a.y.max(b.y) + reach, 0.0)),
        );
        along.clear();
        for x in lo.0..=hi.0 {
            for y in lo.1..=hi.1 {
                for key in cells.get(&(x, y)).into_iter().flatten() {
                    if *key == from || *key == to {
                        continue;
                    }
                    let point = points[key];
                    let offset = glam::DVec2::new(point.x - a.x, point.y - a.y);
                    let t = offset.dot(span) / (length * length);
                    if t > 0.0 && t < 1.0 && span.perp_dot(offset).abs() / length <= reach {
                        along.push((t, *key));
                    }
                }
            }
        }
        along.sort_by(|left, right| left.0.total_cmp(&right.0));
        let chain: Vec<PointKey> = std::iter::once(from).chain(along.iter().map(|&(_, key)| key)).chain(std::iter::once(to)).collect();
        if chain.len() > 2 {
            // Walked the way the face runs the edge, the fan uses the whole
            // edge back from its end to its start, against the face, and each
            // piece forwards, against the cap.
            let mut walk: Vec<mesh_data::Vertex> = chain.iter().map(|key| points[key]).collect();
            if !face_runs_it {
                walk.reverse();
            }
            for pair in walk[1..].windows(2) {
                seams.push([walk[0], pair[0], pair[1]]);
            }
        }
        for pair in chain.windows(2) {
            let (u, v) = (pair[0], pair[1]);
            if u == v {
                continue;
            }
            if u <= v {
                *balance.entry((u, v)).or_default() += 1;
            } else {
                *balance.entry((v, u)).or_default() -= 1;
            }
        }
    }
    let mut segments = Vec::new();
    let mut edges: Vec<_> = balance.into_iter().filter(|(_, net)| *net != 0).collect();
    edges.sort_unstable();
    for ((u, v), net) in edges {
        let (from, to) = if net > 0 { (u, v) } else { (v, u) };
        for _ in 0..net.unsigned_abs() {
            segments.push([points[&from], points[&to]]);
        }
    }

    let rings = closed_boundary_rings(&segments, weld);
    // Every segment must have landed in a ring, or the rim did not close.
    if rings.iter().map(|ring| ring.len()).sum::<usize>() != segments.len() {
        return None;
    }
    let rings = rings
        .into_iter()
        .map(|mut ring| {
            let solid = ring_signed_area(&ring) > 0.0;
            if !solid {
                ring.reverse();
            }
            (ring, solid)
        })
        .collect();
    Some((rings, seams))
}

/// Trace the rings bounding a planar rim, getting past nodes where more than
/// two rim edges meet.
///
/// The fallback for [`directed_cap_rings`], for a rim whose sides cannot all
/// be told. Abandoning a ring at such a node, as a plain walk would, is wrong
/// for capping a cut. A vertical cut through a bench slab reaches
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

    let mut points: HashMap<PointKey, mesh_data::Vertex, FixedState> = HashMap::default();
    let mut adjacency: HashMap<PointKey, Vec<PointKey>, FixedState> = HashMap::default();
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

    let mut visited: HashSet<(PointKey, PointKey), FixedState> = HashSet::default();
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
///
/// Taken about the ring's first point: products of mine coordinates run to
/// trillions, and their rounding alone outweighs the area of a sliver a
/// millimetre wide, which then read as no area at all - neither solid nor
/// hole - and was left uncapped.
fn ring_signed_area(ring: &[mesh_data::Vertex]) -> f64 {
    let Some(&origin) = ring.first() else { return 0.0 };
    (0..ring.len())
        .map(|index| {
            let (a, b) = (ring[index], ring[(index + 1) % ring.len()]);
            (a.x - origin.x) * (b.y - origin.y) - (b.x - origin.x) * (a.y - origin.y)
        })
        .sum()
}

/// Trace the closed loops of a rim whose every segment has the solid on its
/// left. A rim that does not close is left alone: bridging it would invent
/// geometry.
///
/// A rim can touch itself at a point - two parts of a cut meeting at a
/// corner, or a crest pinching against the ground - and there two ways in and
/// two ways out meet. Leaving by the first way out that keeps the solid on the
/// same side hugs it, so the rim splits there into loops that touch without
/// crossing.
fn closed_boundary_rings(segments: &[[mesh_data::Vertex; 2]], weld: Weld) -> Vec<Vec<mesh_data::Vertex>> {
    use std::collections::HashMap;
    let mut points: HashMap<PointKey, mesh_data::Vertex, FixedState> = HashMap::default();
    let mut outgoing: HashMap<PointKey, Vec<usize>, FixedState> = HashMap::default();
    let mut edges: Vec<(PointKey, PointKey)> = Vec::with_capacity(segments.len());
    for [a, b] in segments {
        let (ka, kb) = (weld.key(*a), weld.key(*b));
        if ka == kb {
            continue;
        }
        points.entry(ka).or_insert(*a);
        points.entry(kb).or_insert(*b);
        outgoing.entry(ka).or_default().push(edges.len());
        edges.push((ka, kb));
    }
    let direction = |from: PointKey, to: PointKey| {
        let (a, b) = (points[&from], points[&to]);
        glam::DVec2::new(b.x - a.x, b.y - a.y)
    };

    let mut used = vec![false; edges.len()];
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_unstable_by_key(|&edge| edges[edge]);
    let mut rings = Vec::new();
    for start in order {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut ring = vec![points[&edges[start].0]];
        let mut current = start;
        let closed = loop {
            let (from, at) = edges[current];
            // The way back, and each way on, as a turn away from it; the
            // smallest turn clockwise is the way that hugs the solid.
            let back = direction(at, from);
            let turn = |edge: usize| {
                let angle = -back.perp_dot(direction(at, edges[edge].1)).atan2(back.dot(direction(at, edges[edge].1)));
                if angle <= 0.0 { angle + std::f64::consts::TAU } else { angle }
            };
            let next = outgoing
                .get(&at)
                .into_iter()
                .flatten()
                .copied()
                .filter(|&edge| !used[edge] || edge == start)
                .min_by(|&left, &right| turn(left).total_cmp(&turn(right)));
            match next {
                Some(edge) if edge == start => break true,
                Some(edge) => {
                    used[edge] = true;
                    ring.push(points[&at]);
                    current = edge;
                }
                None => break false,
            }
        };
        if closed && ring.len() >= 3 {
            rings.push(ring);
        }
    }
    rings
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

/// One plan's share of a body clipped by [`clip_piece_to_plans`].
pub(crate) struct ClippedSolid {
    /// Vertices and faces of the clipped body.
    pub(crate) slab: Slab,
    /// The volume it encloses, between its floor and its roof.
    pub(crate) volume: f64,
    /// Which faces are wall the clip built along the plan's own outline,
    /// rather than the body's original surfaces. Parallel to `slab.1`.
    pub(crate) boundary_wall: Vec<bool>,
}

/// A cap's rings, each with whether it bounds solid (wound that way), and
/// the seam triangles that keep the slab closed.
type CapRings = (Vec<(Vec<mesh_data::Vertex>, bool)>, Vec<[mesh_data::Vertex; 3]>);

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
    // Every face's sides by their welded ends, sorted so the faces sharing
    // one sit together: keyed and sorted side by side, joined in one pass.
    let keys: Vec<PointKey> = vertices.par_iter().map(|vertex| weld.key(*vertex)).collect();
    let mut sides: Vec<(EdgeKey, u32)> = triangles
        .par_iter()
        .enumerate()
        .flat_map_iter(|(index, triangle)| {
            let keys = &keys;
            [(0, 1), (1, 2), (2, 0)].into_iter().filter_map(move |(a, b)| {
                let (ka, kb) = (keys[triangle[a] as usize], keys[triangle[b] as usize]);
                (ka != kb).then_some((if ka <= kb { (ka, kb) } else { (kb, ka) }, index as u32))
            })
        })
        .collect();
    drop(keys);
    sides.par_sort_unstable();
    for pair in sides.windows(2) {
        if pair[0].0 == pair[1].0 {
            let (x, y) = (root(&mut parent, pair[0].1), root(&mut parent, pair[1].1));
            if x != y {
                parent[x as usize] = y;
            }
        }
    }
    drop(sides);

    // Each piece's probe: the centroid of its largest triangle in plan.
    let mut probe: HashMap<u32, (f64, glam::DVec2), FixedState> = HashMap::default();
    for (index, triangle) in triangles.iter().enumerate() {
        let [a, b, c] = triangle.map(|i| glam::DVec2::new(vertices[i as usize].x, vertices[i as usize].y));
        let area = (b - a).perp_dot(c - a).abs();
        let entry = probe.entry(root(&mut parent, index as u32)).or_insert((-1.0, glam::DVec2::ZERO));
        if area > entry.0 {
            *entry = (area, (a + b + c) / 3.0);
        }
    }
    let mut owner: HashMap<u32, usize, FixedState> = HashMap::default();
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
    let mut local: Vec<HashMap<u32, u32, FixedState>> = (0..faces.len()).map(|_| HashMap::default()).collect();
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

/// The edges where a body turns a corner - a bench's crest and toe, and where
/// a cut wall meets the ground - rather than every triangle edge across its
/// flat faces. An edge is kept when the planes either side of it meet at more
/// than `min_angle` radians.
///
/// Welded by position, as [`boundary_wall_outline`] is, because a clipped
/// body's faces do not share vertex indices.
pub(crate) fn crease_outline(slab: &Slab, min_angle: f64) -> Vec<[u32; 2]> {
    /// Twice a face's area over its longest side squared, below which the
    /// face is a sliver: about a 1:1000 height to length.
    const SLIVER_RATIO: f64 = 1e-3;

    let (vertices, faces) = slab;
    let weld = Weld::of(vertices);
    let position = |index: u32| {
        let vertex = vertices[index as usize];
        glam::DVec3::new(vertex.x, vertex.y, vertex.z)
    };
    let threshold = min_angle.cos();
    // Every side of every face with a sound normal, by its welded ends, sorted
    // so a side's faces sit together in face order. A survey's bench has
    // millions of these, so they are keyed and sorted side by side.
    let keys: Vec<PointKey> = vertices.par_iter().map(|vertex| weld.key(*vertex)).collect();
    let mut sides: Vec<(EdgeKey, u32, u8)> = faces
        .par_iter()
        .enumerate()
        .flat_map_iter(|(index, face)| {
            let [a, b, c] = face.map(position);
            let cross = (b - a).cross(c - a);
            // A sliver's normal is rounding noise, and splitting leaves them along
            // the cut lines: left in, every one reads as a corner.
            let longest = (b - a).length_squared().max((c - b).length_squared()).max((a - c).length_squared());
            let sound = cross.length() > longest * SLIVER_RATIO;
            let keys = &keys;
            (0..3u8).filter(move |_| sound).filter_map(move |side| {
                let (ka, kb) = (keys[face[side as usize] as usize], keys[face[(side as usize + 1) % 3] as usize]);
                (ka != kb).then_some((if ka <= kb { (ka, kb) } else { (kb, ka) }, index as u32, side))
            })
        })
        .collect();
    drop(keys);
    sides.par_sort_unstable();
    let normal = |face: u32| {
        let [a, b, c] = faces[face as usize].map(position);
        (b - a).cross(c - a).normalize()
    };
    // A side is a corner when a later face turns against its first. Unsigned:
    // two coplanar faces wound apart are no corner. A side only one face uses
    // is a T-junction across a flat face, where a split stops short of the
    // face beside it, not a corner - so it is never drawn.
    sides
        .par_chunk_by(|left, right| left.0 == right.0)
        .filter_map(|group| {
            let (_, first, side) = group[0];
            let facing = normal(first);
            group[1..].iter().any(|&(_, later, _)| facing.dot(normal(later)).abs() < threshold).then(|| {
                let face = faces[first as usize];
                [face[side as usize], face[(side as usize + 1) % 3]]
            })
        })
        .collect()
}

/// The rim of the walls the clip built: the outline separating this piece
/// from the one beside it.
///
/// A wall's own triangulation, and the corners where two of a piece's walls
/// meet, are used by two wall faces and drop out; the top and bottom rims and
/// a wall's free ends are used once and remain. Welded by position, because a
/// clipped body's faces do not share vertex indices.
pub(crate) fn boundary_wall_outline(slab: &Slab, boundary_wall: &[bool]) -> Vec<[u32; 2]> {
    use std::collections::HashMap;

    let (vertices, faces) = slab;
    let weld = Weld::of(vertices);
    let mut counts: HashMap<EdgeKey, (usize, [u32; 2]), FixedState> = HashMap::default();
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

/// Intersect a slab with each of a set of disjoint plan polygons (holes
/// included), returning one clipped body per plan, in order.
///
/// Each face of the body is sent straight to the plans it lies over. A face
/// clear of every plan edge - nearly all of a survey's faces, which are small
/// beside a dig block - goes to its plan whole; the rest are split only along
/// the plan edges that cross them. Each plan's walls are then built once,
/// panel by panel along its own outline, from the rims those splits leave.
/// Cost follows the faces plus the crossings, not the faces times the cuts.
///
/// A crossing is worked out from the face side it lies on and the edge's line,
/// each taken in a fixed order, so two faces sharing a side split it at the
/// same point and the rim along a wall has no gaps to weld over.
///
/// A vertical face belongs to the plan its material lies in, read just inside
/// it: one lying along a plan edge is part of that plan's wall, not of both.
///
/// The volume is the integral between floor and roof (see [`prism_volume`]),
/// which the walls do not change. An open body is clipped rather than
/// rejected: bench bodies are allowed to be open (`build_solid_mesh` warns
/// rather than failing, and the Reserves view reports it through
/// `planning-reserve-open-solid`).
pub(crate) fn clip_piece_to_plans(slab: Slab, plans: &[&[Vec<glam::DVec2>]], cancel: &crate::app::jobs::CancelFlag) -> Result<Vec<ClippedSolid>> {
    /// Faces routed together on one thread: small enough that a run of
    /// large faces, which a design's floor puts together, is shared out.
    const CHUNK: usize = 1 << 11;

    let empty = || ClippedSolid {
        slab: (Vec::new(), Vec::new()),
        volume: 0.0,
        boundary_wall: Vec::new(),
    };
    if slab.1.is_empty() {
        return Ok(plans.iter().map(|_| empty()).collect());
    }
    let floor = slab.0.iter().map(|vertex| vertex.z).fold(f64::INFINITY, f64::min);
    let overlay = PlanOverlay::new(plans, &slab.0);
    let routed = slab
        .1
        .par_chunks(CHUNK)
        .map(|faces| -> Result<Routed> {
            anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
            Ok(overlay.route(&slab.0, faces))
        })
        .collect::<Result<Vec<_>>>()?;
    // Everything from here on is in the routed parts.
    drop(slab);

    let mut panel_plan = vec![0u32; overlay.panels.len()];
    for (plan, panels) in overlay.plan_panels.iter().enumerate() {
        panel_plan[panels.start as usize..panels.end as usize].fill(plan as u32);
    }
    // Each wall face as its run's sheet within its plan's, and its place there.
    let mut sheets: Vec<Vec<Vec<[mesh_data::Vertex; 3]>>> = (0..plans.len()).map(|_| Vec::new()).collect();
    let mut walls: Vec<Vec<(u32, u32)>> = (0..overlay.panels.len()).map(|_| Vec::new()).collect();
    for part in routed {
        for (plan, faces) in part.sheets {
            sheets[plan as usize].push(faces);
        }
        for (panel, face) in part.walls {
            let run = sheets[panel_plan[panel as usize] as usize].len() - 1;
            walls[panel as usize].push((run as u32, face));
        }
    }
    drop(panel_plan);
    let mut walls = walls.into_iter();
    let jobs: Vec<_> = sheets
        .into_iter()
        .zip(&overlay.plan_panels)
        .map(|(sheet, panels)| (sheet, walls.by_ref().take(panels.len()).collect::<Vec<_>>(), panels.start))
        .collect();
    jobs.into_par_iter()
        .map(|(sheet, panel_faces, first_panel)| -> Result<ClippedSolid> {
            anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
            let sheet_faces: usize = sheet.iter().map(Vec::len).sum();
            if sheet_faces == 0 {
                return Ok(empty());
            }
            // Welded as it is gathered, one vertex per bit-identical position
            // in the order they come: held as loose triangles until its level
            // was dressed, each piece cost three times the memory, and the
            // pieces of the levels cut side by side outgrew the slabs they
            // were cut from. The weld is the one the piece is dressed with, so
            // that one finds nothing left to merge.
            let mut vertices: Vec<mesh_data::Vertex> = Vec::with_capacity(sheet_faces / 2 + 16);
            let mut faces: Vec<[u32; 3]> = Vec::with_capacity(sheet_faces + sheet_faces / 8);
            let mut seen: std::collections::HashMap<[u64; 3], u32, FixedState> = std::collections::HashMap::with_capacity_and_hasher(sheet_faces / 2 + 16, FixedState::default());
            let mut weld = |triangle: [mesh_data::Vertex; 3]| {
                faces.push(triangle.map(|vertex| {
                    *seen.entry([vertex.x.to_bits(), vertex.y.to_bits(), vertex.z.to_bits()]).or_insert_with(|| {
                        vertices.push(vertex);
                        (vertices.len() - 1) as u32
                    })
                }));
            };
            for part in &sheet {
                part.iter().copied().for_each(&mut weld);
            }
            let mut faces_here = Vec::new();
            for (offset, refs) in panel_faces.iter().enumerate() {
                faces_here.clear();
                faces_here.extend(refs.iter().map(|&(run, face)| sheet[run as usize][face as usize]));
                panel_wall(&overlay.panels[first_panel as usize + offset], &faces_here, floor)
                    .into_iter()
                    .for_each(&mut weld);
            }
            drop((sheet, panel_faces, faces_here));
            drop(seen);
            let mut boundary_wall = vec![false; sheet_faces];
            boundary_wall.resize(faces.len(), true);
            vertices.shrink_to_fit();
            faces.shrink_to_fit();
            let mut slab: Slab = (vertices, faces);
            // Every piece wound the same way: the block-model overlap
            // integrates signed prisms over floor and roof from one origin.
            let signed = prism_volume(&slab.0, &slab.1);
            if signed < 0.0 {
                for face in &mut slab.1 {
                    face.swap(1, 2);
                }
            }
            Ok(ClippedSolid {
                slab,
                volume: signed.abs(),
                boundary_wall,
            })
        })
        .collect()
}

/// How far a point may lie off a plan edge's line and still be on it: the
/// same snap the cut planes have always used.
const ON_LINE: f64 = 1.0e-8;

/// How close two corners of a split face may be and still be one corner.
const PIECE_MERGE: f64 = 1.0e-7;

/// How far inside a vertical face its plan is read, on its material side.
const WALL_PROBE: f64 = 1.0e-5;

/// How close to a panel's plane a face's corners must lie for the face to
/// bound that panel's wall: the weld a wall is traced with.
const PANEL_REACH: f64 = 1.0e-6;

/// How close to a panel's end a loose end of its rim must lie to be closed
/// down the corner there.
const PANEL_END: f64 = 1.0e-4;

const NO_PLAN: u32 = u32::MAX;
const UNKNOWN_PLAN: u32 = u32::MAX - 1;

/// One straight run of a plan's outline, with the plan's ground on its left
/// from `start` to `end`: the stretch one wall stands along.
struct Panel {
    start: glam::DVec2,
    end: glam::DVec2,
}

/// One edge of the plans' outlines, its ends in a fixed order, so the plans
/// either side of it share it.
struct PlanEdge {
    a: glam::DVec2,
    b: glam::DVec2,
    length: f64,
    /// Each plan it bounds: the plan, whether its ground lies left of
    /// `a -> b`, and the panel of that plan's outline it is part of.
    owners: Vec<(u32, bool, u32)>,
}

impl PlanEdge {
    fn distance(&self, point: glam::DVec2) -> f64 {
        (self.b - self.a).perp_dot(point - self.a) / self.length
    }

    /// Which side of the line `point` is on, zero when within [`ON_LINE`].
    fn side(&self, point: glam::DVec2) -> f64 {
        let distance = self.distance(point);
        if distance.abs() <= ON_LINE { 0.0 } else { distance }
    }

    /// Where the line crosses the segment `from`-`to`, worked out from the
    /// segment's ends in a fixed order, so every face sharing it agrees.
    fn crossing(&self, from: glam::DVec3, to: glam::DVec3) -> glam::DVec3 {
        let (from, to) = if position_order(from, to).is_le() { (from, to) } else { (to, from) };
        let (near, far) = (self.distance(from.truncate()), self.distance(to.truncate()));
        from + (to - from) * (near / (near - far))
    }
}

fn position_order(a: glam::DVec3, b: glam::DVec3) -> std::cmp::Ordering {
    a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z))
}

/// The plan edges bucketed on a grid, so a face finds the few that can
/// cross it, and the plan of any point clear of them is known for its cell.
struct PlanGrid {
    min: glam::DVec2,
    size: glam::DVec2,
    columns: usize,
    rows: usize,
    offsets: Vec<u32>,
    items: Vec<u32>,
    /// The plan of each cell no edge reaches, found on first use.
    labels: Vec<std::sync::atomic::AtomicU32>,
}

impl PlanGrid {
    fn new(min: glam::DVec2, max: glam::DVec2, edges: &[PlanEdge]) -> Self {
        // About four cells an edge, which leaves most cells clear of every
        // edge and most faces in clear cells.
        let extent = (max - min).max(glam::DVec2::splat(1e-6));
        let target = (edges.len() * 4).clamp(64, 1 << 22) as f64;
        let side = (extent.x * extent.y / target).sqrt().max(1e-6);
        let columns = ((extent.x / side).ceil() as usize).clamp(1, 1 << 13);
        let rows = ((extent.y / side).ceil() as usize).clamp(1, 1 << 13);
        let mut grid = Self {
            min,
            size: glam::DVec2::new(extent.x / columns as f64, extent.y / rows as f64),
            columns,
            rows,
            offsets: vec![0; columns * rows + 1],
            items: Vec::new(),
            labels: (0..columns * rows).map(|_| std::sync::atomic::AtomicU32::new(UNKNOWN_PLAN)).collect(),
        };
        let mut counts = vec![0u32; columns * rows];
        for edge in edges {
            grid.cover(edge.a, edge.b, |cell| counts[cell] += 1);
        }
        for (cell, count) in counts.iter().enumerate() {
            grid.offsets[cell + 1] = grid.offsets[cell] + count;
        }
        let mut next: Vec<u32> = grid.offsets[..columns * rows].to_vec();
        let mut items = vec![0u32; grid.offsets[columns * rows] as usize];
        for (index, edge) in edges.iter().enumerate() {
            grid.cover(edge.a, edge.b, |cell| {
                items[next[cell] as usize] = index as u32;
                next[cell] += 1;
            });
        }
        grid.items = items;
        grid
    }

    fn cell_of(&self, point: glam::DVec2) -> (usize, usize) {
        let cell = ((point - self.min) / self.size).floor();
        ((cell.x.max(0.0) as usize).min(self.columns - 1), (cell.y.max(0.0) as usize).min(self.rows - 1))
    }

    fn edges_in(&self, cell: usize) -> &[u32] {
        &self.items[self.offsets[cell] as usize..self.offsets[cell + 1] as usize]
    }

    /// Every cell the segment `a`-`b` passes through, padded so a point on a
    /// cell's border finds it from either side.
    fn cover(&self, a: glam::DVec2, b: glam::DVec2, mut visit: impl FnMut(usize)) {
        let pad = ON_LINE + self.size.min_element() * 1e-6;
        let (low, high) = (a.y.min(b.y), a.y.max(b.y));
        let (_, first) = self.cell_of(glam::DVec2::new(a.x, low - pad));
        let (_, last) = self.cell_of(glam::DVec2::new(a.x, high + pad));
        for row in first..=last {
            let (x0, x1) = if high <= low {
                (a.x.min(b.x), a.x.max(b.x))
            } else {
                let bottom = (self.min.y + row as f64 * self.size.y - pad).clamp(low, high);
                let top = (self.min.y + (row + 1) as f64 * self.size.y + pad).clamp(low, high);
                let at = |y: f64| a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y);
                let (p, q) = (at(bottom), at(top));
                (p.min(q), p.max(q))
            };
            let (from, _) = self.cell_of(glam::DVec2::new(x0 - pad, low));
            let (to, _) = self.cell_of(glam::DVec2::new(x1 + pad, low));
            for column in from..=to {
                visit(row * self.columns + column);
            }
        }
    }
}

/// The plans a body is cut to: their edges, the panels their walls stand
/// along (each plan's a contiguous run), and the grid that finds them.
struct PlanOverlay {
    edges: Vec<PlanEdge>,
    panels: Vec<Panel>,
    plan_panels: Vec<std::ops::Range<u32>>,
    grid: PlanGrid,
}

/// What one run of faces left in each plan: its faces, by plan, and the faces
/// bounding each panel's wall, as the panel and the face's place in the
/// panel's plan's sheet. A wall face is one of its plan's faces, and a
/// survey's walls run to millions of them, so they are not copied.
struct Routed {
    sheets: Vec<(u32, Vec<[mesh_data::Vertex; 3]>)>,
    walls: Vec<(u32, u32)>,
}

/// A convex piece of a face being split: its corners in order, each with the
/// sides of the original face it lies on, as bits.
type FacePiece = Vec<(glam::DVec3, u8)>;

impl PlanOverlay {
    fn new(plans: &[&[Vec<glam::DVec2>]], body: &[mesh_data::Vertex]) -> Self {
        use std::collections::HashMap;
        let mut edges: Vec<PlanEdge> = Vec::new();
        let mut index: HashMap<[u64; 4], u32, FixedState> = HashMap::default();
        let mut panels = Vec::new();
        let mut plan_panels = Vec::with_capacity(plans.len());
        for (plan_index, plan) in plans.iter().enumerate() {
            let first = panels.len() as u32;
            for ring in plan.iter() {
                let mut points: Vec<glam::DVec2> = Vec::with_capacity(ring.len());
                for &point in ring {
                    if points.last() != Some(&point) {
                        points.push(point);
                    }
                }
                while points.len() > 1 && points.first() == points.last() {
                    points.pop();
                }
                let count = points.len();
                if count < 3 {
                    continue;
                }
                // Which side of this ring the plan's ground is on, read just
                // off its longest edge.
                let (a, b) = (0..count)
                    .map(|i| (points[i], points[(i + 1) % count]))
                    .max_by(|x, y| x.0.distance_squared(x.1).total_cmp(&y.0.distance_squared(y.1)))
                    .expect("a ring has edges");
                let span = b - a;
                let step = (span.length() * 1e-6).max(1e-6);
                let left = crate::model::arrangement::point_in_face(plan, a.midpoint(b) + span.perp().normalize() * step);
                // A wall stands along each straight run, however many points
                // the outline has along it.
                let turns = |i: usize| {
                    let (before, here, after) = (points[(i + count - 1) % count], points[i], points[(i + 1) % count]);
                    let (inward, outward) = ((here - before).normalize(), (after - here).normalize());
                    inward.perp_dot(outward).abs() > 1e-9 || inward.dot(outward) < 0.0
                };
                let Some(start) = (0..count).find(|&i| turns(i)) else { continue };
                let mut panel_start = points[start];
                for step in 0..count {
                    let i = (start + step) % count;
                    let (a, b) = (points[i], points[(i + 1) % count]);
                    let panel = panels.len() as u32;
                    let (low, high, ground_left) = if a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).is_le() {
                        (a, b, left)
                    } else {
                        (b, a, !left)
                    };
                    let key = [low.x.to_bits(), low.y.to_bits(), high.x.to_bits(), high.y.to_bits()];
                    let edge = *index.entry(key).or_insert_with(|| {
                        edges.push(PlanEdge {
                            a: low,
                            b: high,
                            length: (high - low).length(),
                            owners: Vec::new(),
                        });
                        edges.len() as u32 - 1
                    });
                    edges[edge as usize].owners.push((plan_index as u32, ground_left, panel));
                    if turns((i + 1) % count) {
                        panels.push(if left {
                            Panel { start: panel_start, end: b }
                        } else {
                            Panel { start: b, end: panel_start }
                        });
                        panel_start = b;
                    }
                }
            }
            plan_panels.push(first..panels.len() as u32);
        }
        // Over the plans as well as the body: finding a point's plan walks
        // right to the first edge, which may lie beyond the body.
        let mut min = glam::DVec2::splat(f64::INFINITY);
        let mut max = glam::DVec2::splat(f64::NEG_INFINITY);
        for point in body
            .iter()
            .map(|vertex| glam::DVec2::new(vertex.x, vertex.y))
            .chain(edges.iter().flat_map(|edge| [edge.a, edge.b]))
        {
            min = min.min(point);
            max = max.max(point);
        }
        let grid = PlanGrid::new(min, max, &edges);
        Self { edges, panels, plan_panels, grid }
    }

    /// The plan `point` lies in: walking right from it to the first plan
    /// edge it reaches, the plan on its side of that edge, or the plan of a
    /// cell it reaches first that no edge crosses.
    fn locate(&self, point: glam::DVec2) -> u32 {
        use std::sync::atomic::Ordering;
        let grid = &self.grid;
        let (mut column, row) = grid.cell_of(point);
        let start = row * grid.columns + column;
        let clear = grid.edges_in(start).is_empty();
        if clear {
            let known = grid.labels[start].load(Ordering::Relaxed);
            if known != UNKNOWN_PLAN {
                return known;
            }
        }
        let mut hits: Vec<(f64, u32)> = Vec::new();
        let found = loop {
            let cell = row * grid.columns + column;
            let here = grid.edges_in(cell);
            if here.is_empty() {
                let known = grid.labels[cell].load(Ordering::Relaxed);
                if cell != start && known != UNKNOWN_PLAN {
                    break known;
                }
            } else {
                let right = if column + 1 == grid.columns {
                    f64::INFINITY
                } else {
                    grid.min.x + (column + 1) as f64 * grid.size.x + ON_LINE
                };
                hits.clear();
                for &index in here {
                    let edge = &self.edges[index as usize];
                    // Half-open in y, so a walk through a corner meets
                    // exactly one of its edges, or neither.
                    if (edge.a.y > point.y) != (edge.b.y > point.y) {
                        let x = edge.a.x + (point.y - edge.a.y) * (edge.b.x - edge.a.x) / (edge.b.y - edge.a.y);
                        if x >= point.x && x <= right {
                            hits.push((x, index));
                        }
                    }
                }
                if let Some(nearest) = hits.iter().map(|(x, _)| *x).min_by(f64::total_cmp) {
                    hits.retain(|(x, _)| *x <= nearest + ON_LINE);
                    hits.sort_unstable_by_key(|(_, index)| *index);
                    let plan = hits.iter().find_map(|(_, index)| {
                        let edge = &self.edges[*index as usize];
                        let left = edge.distance(point) > 0.0;
                        edge.owners.iter().find(|(_, ground_left, _)| *ground_left == left).map(|(plan, _, _)| *plan)
                    });
                    break plan.unwrap_or(NO_PLAN);
                }
            }
            column += 1;
            if column == grid.columns {
                break NO_PLAN;
            }
        };
        if clear {
            grid.labels[start].store(found, Ordering::Relaxed);
        }
        found
    }

    fn route(&self, vertices: &[mesh_data::Vertex], faces: &[[u32; 3]]) -> Routed {
        use std::collections::HashMap;
        let mut sheets: HashMap<u32, Vec<[mesh_data::Vertex; 3]>, FixedState> = HashMap::default();
        let mut walls = Vec::new();
        let mut candidates: Vec<u32> = Vec::new();
        let mut panels_hit: Vec<u32> = Vec::new();
        for face in faces {
            let corners = face.map(|index| vertices[index as usize]);
            let low = glam::DVec2::new(corners[0].x.min(corners[1].x).min(corners[2].x), corners[0].y.min(corners[1].y).min(corners[2].y)) - ON_LINE;
            let high = glam::DVec2::new(corners[0].x.max(corners[1].x).max(corners[2].x), corners[0].y.max(corners[1].y).max(corners[2].y)) + ON_LINE;
            let ((first_column, first_row), (last_column, last_row)) = (self.grid.cell_of(low), self.grid.cell_of(high));
            candidates.clear();
            for row in first_row..=last_row {
                for column in first_column..=last_column {
                    candidates.extend_from_slice(self.grid.edges_in(row * self.grid.columns + column));
                }
            }
            if candidates.is_empty() {
                let centre = glam::DVec2::new(corners.iter().map(|p| p.x).sum::<f64>() / 3.0, corners.iter().map(|p| p.y).sum::<f64>() / 3.0);
                let plan = self.locate(centre);
                if plan != NO_PLAN && !collapsed(&corners) {
                    sheets.entry(plan).or_default().push(corners);
                }
                continue;
            }
            candidates.sort_unstable();
            candidates.dedup();
            self.cut_face(corners, &candidates, &mut sheets, &mut walls, &mut panels_hit);
        }
        let mut sheets: Vec<_> = sheets.into_iter().collect();
        sheets.sort_unstable_by_key(|(plan, _)| *plan);
        Routed { sheets, walls }
    }

    /// Split one face along the `candidates` that cross it and send each part
    /// to its plan, and to the wall of any panel it meets.
    fn cut_face(
        &self,
        corners: [mesh_data::Vertex; 3],
        candidates: &[u32],
        sheets: &mut std::collections::HashMap<u32, Vec<[mesh_data::Vertex; 3]>, FixedState>,
        walls: &mut Vec<(u32, u32)>,
        panels_hit: &mut Vec<u32>,
    ) {
        let face = corners.map(|p| glam::DVec3::new(p.x, p.y, p.z));
        let normal = (face[1] - face[0]).cross(face[2] - face[0]);
        let vertical = normal.z.abs() <= 1e-9 * normal.length();
        let mut pieces: Vec<FacePiece> = vec![(0..3).map(|k| (face[k], (1u8 << k) | (1u8 << ((k + 2) % 3)))).collect()];
        let mut split = false;
        for &index in candidates {
            let edge = &self.edges[index as usize];
            let mut next = Vec::with_capacity(pieces.len() + 1);
            for piece in pieces {
                match split_piece(&piece, &face, edge) {
                    Some((left, right)) => {
                        split = true;
                        next.extend([left, right].into_iter().filter(|part| part.len() >= 3));
                    }
                    None => next.push(piece),
                }
            }
            pieces = next;
        }
        let vertex = |p: glam::DVec3| mesh_data::Vertex::new(p.x, p.y, p.z);
        for piece in &pieces {
            let centre = piece.iter().map(|(p, _)| p.truncate()).sum::<glam::DVec2>() / piece.len() as f64;
            let outward = normal.truncate();
            let probe = if vertical && outward.length_squared() > 0.0 {
                centre - outward.normalize() * WALL_PROBE
            } else {
                centre
            };
            let plan = self.locate(probe);
            if plan == NO_PLAN {
                continue;
            }
            let triangles: Vec<[mesh_data::Vertex; 3]> = if split {
                // Two plan edges meeting inside a face each put their corner
                // there, a rounding error apart: one point, or the fan below
                // leaves a sliver whose sides cancel the rim along the wall.
                let mut points: Vec<glam::DVec3> = Vec::with_capacity(piece.len());
                for (point, _) in piece {
                    if points.last().is_none_or(|last: &glam::DVec3| last.distance_squared(*point) > PIECE_MERGE * PIECE_MERGE) {
                        points.push(*point);
                    }
                }
                while points.len() > 1 && points[0].distance_squared(points[points.len() - 1]) <= PIECE_MERGE * PIECE_MERGE {
                    points.pop();
                }
                if points.len() < 3 {
                    continue;
                }
                // Fanned from its sharpest corner, which no other corner lies
                // in line with.
                let count = points.len();
                let turn = |k: usize| {
                    let (before, here, after) = (points[(k + count - 1) % count], points[k], points[(k + 1) % count]);
                    (here - before).cross(after - here).length()
                };
                let apex = (0..count).max_by(|a, b| turn(*a).total_cmp(&turn(*b))).unwrap_or(0);
                (1..count - 1)
                    .map(|k| [points[apex], points[(apex + k) % count], points[(apex + k + 1) % count]].map(vertex))
                    .collect()
            } else {
                vec![corners]
            };
            for triangle in triangles {
                if collapsed(&triangle) {
                    continue;
                }
                panels_hit.clear();
                for &index in candidates {
                    let edge = &self.edges[index as usize];
                    for &(owner, _, panel) in &edge.owners {
                        if owner == plan && triangle.iter().filter(|p| edge.distance(glam::DVec2::new(p.x, p.y)).abs() <= PANEL_REACH).count() >= 2 {
                            panels_hit.push(panel);
                        }
                    }
                }
                panels_hit.sort_unstable();
                panels_hit.dedup();
                let sheet = sheets.entry(plan).or_default();
                walls.extend(panels_hit.iter().map(|&panel| (panel, sheet.len() as u32)));
                sheet.push(triangle);
            }
        }
    }
}

/// `piece` split along `edge`'s line, or `None` when the line misses it or
/// crosses it only beyond the edge's ends.
fn split_piece(piece: &FacePiece, face: &[glam::DVec3; 3], edge: &PlanEdge) -> Option<(FacePiece, FacePiece)> {
    let sides: Vec<f64> = piece.iter().map(|(point, _)| edge.side(point.truncate())).collect();
    if !(sides.iter().any(|side| *side > 0.0) && sides.iter().any(|side| *side < 0.0)) {
        return None;
    }
    let span = edge.b - edge.a;
    let along = |point: glam::DVec3| (point.truncate() - edge.a).dot(span) / (edge.length * edge.length);
    let (mut lowest, mut highest) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut left = Vec::with_capacity(piece.len() + 1);
    let mut right = Vec::with_capacity(piece.len() + 1);
    for index in 0..piece.len() {
        let next = (index + 1) % piece.len();
        let ((point, bits), (other, other_bits)) = (piece[index], piece[next]);
        let (side, other_side) = (sides[index], sides[next]);
        if side >= 0.0 {
            left.push((point, bits));
        }
        if side <= 0.0 {
            right.push((point, bits));
        }
        if side == 0.0 {
            lowest = lowest.min(along(point));
            highest = highest.max(along(point));
        }
        if (side > 0.0 && other_side < 0.0) || (side < 0.0 && other_side > 0.0) {
            // On a side of the original face, from that side's own ends, so
            // the face across it splits it at the same point.
            let shared = bits & other_bits;
            let crossing = if shared != 0 {
                let side = shared.trailing_zeros() as usize;
                (edge.crossing(face[side], face[(side + 1) % 3]), 1u8 << side)
            } else {
                (edge.crossing(point, other), 0)
            };
            lowest = lowest.min(along(crossing.0));
            highest = highest.max(along(crossing.0));
            left.push(crossing);
            right.push(crossing);
        }
    }
    let reach = ON_LINE / edge.length;
    (highest >= -reach && lowest <= 1.0 + reach).then_some((left, right))
}

/// The wall along `panel`, closing the faces of one plan's piece that meet
/// its plane: the cap their rim draws there, as [`cap_slab`] caps a cut, with
/// the rim's loose ends at the panel's corners joined down each corner.
fn panel_wall(panel: &Panel, faces: &[[mesh_data::Vertex; 3]], floor: f64) -> Vec<[mesh_data::Vertex; 3]> {
    use std::collections::HashMap;
    let span = panel.end - panel.start;
    let length = span.length();
    if faces.is_empty() || length <= 0.0 {
        return Vec::new();
    }
    // The frame a cut is capped in: height, distance along, and distance into
    // the plan's ground, which is left of start -> end.
    let tangent = glam::DVec3::new(span.x / length, span.y / length, 0.0);
    let normal = glam::DVec3::new(-tangent.y, tangent.x, 0.0);
    let origin = glam::DVec3::new(panel.start.x, panel.start.y, floor);
    let local = |p: &mesh_data::Vertex| {
        let delta = glam::DVec3::new(p.x, p.y, p.z) - origin;
        mesh_data::Vertex::new(delta.z, delta.dot(tangent), delta.dot(normal))
    };
    let mut vertices: Vec<mesh_data::Vertex> = faces.iter().flatten().map(local).collect();
    let weld = Weld::of(&vertices);
    // Points on the plane keep their own positions, so the wall meets the
    // faces it closes exactly.
    let mut world: HashMap<(u64, u64), mesh_data::Vertex, FixedState> = HashMap::default();
    for (point, source) in vertices.iter_mut().zip(faces.iter().flatten()) {
        if point.z.abs() <= weld.tolerance {
            point.z = 0.0;
            world.entry((point.x.to_bits(), point.y.to_bits())).or_insert(*source);
        }
    }
    let mut indices: Vec<[u32; 3]> = (0..faces.len() as u32).map(|face| [3 * face, 3 * face + 1, 3 * face + 2]).collect();
    let rim = |vertices: &[mesh_data::Vertex]| -> Vec<[mesh_data::Vertex; 2]> {
        boundary_segments(vertices, &indices)
            .into_iter()
            .filter(|[a, b]| a.z == 0.0 && b.z == 0.0 && (a.y + b.y) / 2.0 >= -weld.tolerance && (a.y + b.y) / 2.0 <= length + weld.tolerance)
            .collect()
    };
    // The rim stops at the panel's corners, where the next panel's wall
    // takes over: its loose ends there, paired up the corner from the
    // bottom, close it - rising at the start, with the ground on their
    // left, and falling at the end.
    let corners = |rims: &[[mesh_data::Vertex; 2]]| -> Vec<[mesh_data::Vertex; 2]> {
        let mut degree: HashMap<PointKey, (usize, mesh_data::Vertex), FixedState> = HashMap::default();
        for [a, b] in rims {
            let (ka, kb) = (weld.key(*a), weld.key(*b));
            if ka == kb {
                continue;
            }
            degree.entry(ka).or_insert((0, *a)).0 += 1;
            degree.entry(kb).or_insert((0, *b)).0 += 1;
        }
        let (mut starts, mut ends) = (Vec::new(), Vec::new());
        for (count, point) in degree.into_values() {
            if count % 2 == 1 {
                if point.y.abs() <= PANEL_END {
                    starts.push(point);
                } else if (point.y - length).abs() <= PANEL_END {
                    ends.push(point);
                }
            }
        }
        let mut closing = Vec::new();
        for (points, rising) in [(&mut starts, true), (&mut ends, false)] {
            if points.len() % 2 != 0 {
                continue;
            }
            points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
            for pair in points.chunks(2) {
                closing.push(if rising { [pair[0], pair[1]] } else { [pair[1], pair[0]] });
            }
        }
        closing
    };
    let closes = |rims: &[[mesh_data::Vertex; 2]], closing: &[[mesh_data::Vertex; 2]]| {
        let all: Vec<[mesh_data::Vertex; 2]> = rims.iter().chain(closing).copied().collect();
        rims_close(&all, &[0.0], weld)
    };
    let mut rims = rim(&vertices);
    if rims.is_empty() {
        return Vec::new();
    }
    let mut closing = corners(&rims);
    // As in `cap_slab`: points are merged only where the rim does not
    // already close.
    if !closes(&rims, &closing) {
        let planar: Vec<usize> = (0..vertices.len()).filter(|&index| vertices[index].z == 0.0).collect();
        merge_coincident(&mut vertices, &planar, weld);
        rims = rim(&vertices);
        closing = corners(&rims);
    }
    let mut sides = cap_sides(&vertices, &indices, weld);
    for [from, to] in &closing {
        let (kf, kt) = (weld.key(*from), weld.key(*to));
        if kf != kt {
            sides.insert(if kf <= kt { (kf, kt) } else { (kt, kf) }, (kf, kt));
        }
    }
    rims.extend(closing);
    let (rings, capped, seams): (Vec<Vec<mesh_data::Vertex>>, Vec<bool>, Vec<[mesh_data::Vertex; 3]>) = match directed_cap_rings(&rims, &sides, weld) {
        Some((directed, seams)) => {
            let (rings, capped) = directed.into_iter().unzip();
            (rings, capped, seams)
        }
        None => {
            let rings = planar_cap_rings(&rims, weld);
            let capped = rings.iter().map(|ring| ring_is_material(ring, &sides, weld)).collect();
            (rings, capped, Vec::new())
        }
    };
    let first = indices.len();
    append_caps(&rings, &capped, 0.0, false, &mut vertices, &mut indices);
    let back = |point: mesh_data::Vertex| {
        (point.z == 0.0)
            .then(|| world.get(&(point.x.to_bits(), point.y.to_bits())).copied())
            .flatten()
            .unwrap_or_else(|| {
                let p = origin + glam::DVec3::Z * point.x + tangent * point.y + normal * point.z;
                mesh_data::Vertex::new(p.x, p.y, p.z)
            })
    };
    seams
        .into_iter()
        .map(|seam| seam.map(back))
        .chain(indices[first..].iter().map(|face| face.map(|index| back(vertices[index as usize]))))
        .collect()
}
