//! Scene bounds helpers.

use glam::{DMat3, DVec3, Vec3};

use crate::model::{
    Document, Object, SceneEntityId,
    block_model::OpenBlockModel,
    drill_hole::OpenDrillHoleDataset,
    geometry::{polyline_bulge_bounds, text_bounds_corners},
    point_cloud::OpenPointCloud,
    triangulation::OpenTriangulation,
};

pub(crate) fn scene_bounds(
    document: &Document,
    triangulations: &[OpenTriangulation],
    block_models: &[OpenBlockModel],
    drill_holes: &[OpenDrillHoleDataset],
    point_clouds: &[OpenPointCloud],
    hidden: &std::collections::HashSet<SceneEntityId>,
) -> Option<(DVec3, DVec3)> {
    let mut min = DVec3::splat(f64::MAX);
    let mut max = DVec3::splat(f64::MIN);
    let mut any = false;
    for_each_visible_object_aabb(document, triangulations, block_models, drill_holes, point_clouds, hidden, &mut |object_min, object_max| {
        min = min.min(object_min);
        max = max.max(object_max);
        any = true;
    });
    any.then_some((min, max))
}

/// World-space AABB of every visible object, one entry per object. Depth-range
/// fitting needs these individually (not the merged scene AABB) so a distant
/// model that is outside the current view sideways can be rejected before it
/// blows up the near/far clip range.
pub(crate) fn visible_object_aabbs(
    document: &Document,
    triangulations: &[OpenTriangulation],
    block_models: &[OpenBlockModel],
    drill_holes: &[OpenDrillHoleDataset],
    point_clouds: &[OpenPointCloud],
    hidden: &std::collections::HashSet<SceneEntityId>,
) -> Vec<(DVec3, DVec3)> {
    let mut aabbs = Vec::new();
    for_each_visible_object_aabb(document, triangulations, block_models, drill_holes, point_clouds, hidden, &mut |object_min, object_max| {
        aabbs.push((object_min, object_max))
    });
    aabbs
}

/// Shared visible-object iteration. Each object contributes one AABB; text,
/// polylines and roads collapse their many points into a single min/max so
/// callers see one box per object.
fn for_each_visible_object_aabb(
    document: &Document,
    triangulations: &[OpenTriangulation],
    block_models: &[OpenBlockModel],
    drill_holes: &[OpenDrillHoleDataset],
    point_clouds: &[OpenPointCloud],
    hidden: &std::collections::HashSet<SceneEntityId>,
    emit: &mut impl FnMut(DVec3, DVec3),
) {
    let hidden_layers: std::collections::HashSet<_> = document.layers().iter().filter(|layer| !layer.loaded).map(|layer| layer.id).collect();
    for object in document.objects() {
        if hidden_layers.contains(&object.layer()) || hidden.contains(&SceneEntityId::Object(object.id())) {
            continue;
        }

        let mut object_min = DVec3::splat(f64::MAX);
        let mut object_max = DVec3::splat(f64::MIN);
        let mut object_any = false;
        let mut include = |point: DVec3| {
            object_min = object_min.min(point);
            object_max = object_max.max(point);
            object_any = true;
        };
        match object {
            Object::Point { pos, .. } => include(*pos),
            Object::Text {
                pos, content, height, rotation, ..
            } => {
                for corner in text_bounds_corners(*pos, content, *height, *rotation) {
                    include(corner);
                }
            }
            Object::Circle { center, radius, .. } => {
                include(*center - DVec3::new(*radius, *radius, 0.0));
                include(*center + DVec3::new(*radius, *radius, 0.0));
            }
            Object::Polyline { verts, closed, .. } => {
                if let Some((verts_min, verts_max)) = polyline_bulge_bounds(verts, *closed) {
                    include(verts_min);
                    include(verts_max);
                }
            }
        }
        if object_any {
            emit(object_min, object_max);
        }
    }

    for triangulation in triangulations
        .iter()
        .filter(|triangulation| triangulation.state.loaded && !hidden.contains(&triangulation.entity_id()))
    {
        let bounds = triangulation.mesh.bounds();
        emit(DVec3::new(bounds.min.x, bounds.min.y, bounds.min.z), DVec3::new(bounds.max.x, bounds.max.y, bounds.max.z));
    }

    for block_model in block_models
        .iter()
        .filter(|block_model| block_model.state.loaded && !hidden.contains(&block_model.entity_id()))
    {
        if let Some((block_min, block_max)) = block_model.visible_world_bounds() {
            emit(block_min, block_max);
        }
    }

    for dataset in drill_holes.iter().filter(|dataset| dataset.state.loaded && !hidden.contains(&dataset.entity_id())) {
        if let Some((min, max)) = dataset.dataset.bounds {
            emit(min, max);
        }
    }

    for point_cloud in point_clouds
        .iter()
        .filter(|point_cloud| point_cloud.state.loaded && !hidden.contains(&point_cloud.entity_id()))
    {
        let (cloud_min, cloud_max) = point_cloud.bounds;
        emit(cloud_min, cloud_max);
    }
}

/// Fit a chunk's culling box to its geometry, returning the world-space
/// centre, the axes and the half extents.
///
/// A chunk of a steep pit wall is a nearly planar inclined band; the axis-
/// aligned box around it is mostly air, and air inside the box is what lets a
/// chunk wholly out of view survive the frustum test. So the box is a slab
/// laid on the chunk: its thin axis is one of a few candidate normals (the
/// covariance's weakest direction, the mean surface normal, vertical), and for
/// each the in-plane rotation is searched for the smallest footprint.
///
/// Principal axes alone are not enough for that rotation: a kd cell's
/// footprint is near-square, its in-plane variance near-isotropic, and the
/// eigenvectors land anywhere - at 45° the box needs twice the area. The
/// search scores orientations on a vertex subsample and only the winner is
/// measured over every vertex, so the box stays conservative. Falls back to
/// the axis-aligned box whenever that is no larger, so no chunk culls worse.
///
/// `vertices` are chunk-local (relative to `chunk_origin`, the AABB centre),
/// read through `position`, and `aabb_size` is the chunk's full AABB extent.
/// `extra_normals` adds slab normals to try beside the covariance's weakest
/// direction and vertical - a surface passes its mean face normal; a point
/// cloud, having no normals, passes none.
pub(crate) fn fit_chunk_box<P>(vertices: &[P], position: impl Fn(&P) -> DVec3, extra_normals: &[DVec3], chunk_origin: DVec3, aabb_size: DVec3) -> (DVec3, [Vec3; 3], Vec3) {
    /// Vertices the orientation search scores against.
    const SEARCH_SAMPLES: usize = 1024;
    /// Coarse in-plane sweep over the quarter turn a box repeats in, then a
    /// fine one around the best coarse angle.
    const COARSE_STEP_DEGREES: f64 = 3.0;
    const FINE_STEP_DEGREES: f64 = 0.5;

    // Vertices were rounded to f32 on their way into `vertices`; pad by more
    // than that rounding so the box never clips the geometry it bounds. Also
    // keeps a flat chunk's zero thickness from making every volume zero.
    let padding = 1e-6 * aabb_size.max_element() + 1e-3;
    let aabb = (chunk_origin, [Vec3::X, Vec3::Y, Vec3::Z], (aabb_size * 0.5 + padding).as_vec3());
    if vertices.len() < 3 {
        return aabb;
    }
    let stride = vertices.len().div_ceil(SEARCH_SAMPLES).max(1);
    let samples: Vec<DVec3> = vertices.iter().step_by(stride).map(&position).collect();
    let range = |points: &[DVec3], axis: DVec3| {
        let (low, high) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), point| {
            let along = axis.dot(*point);
            (low.min(along), high.max(along))
        });
        high - low + 2.0 * padding
    };

    let mean = samples.iter().sum::<DVec3>() / samples.len() as f64;
    let mut covariance = DMat3::ZERO;
    for point in &samples {
        let offset = *point - mean;
        covariance += DMat3::from_cols(offset * offset.x, offset * offset.y, offset * offset.z);
    }
    let weakest = symmetric_eigenvectors(covariance)
        .into_iter()
        .min_by(|a, b| a.dot(covariance * *a).total_cmp(&b.dot(covariance * *b)))
        .unwrap_or(DVec3::Z);

    // Smallest-volume `[u, v, normal]` over the candidates.
    let mut best: Option<(f64, [DVec3; 3])> = None;
    for &normal in std::iter::once(&weakest).chain(extra_normals).chain(std::iter::once(&DVec3::Z)) {
        if normal == DVec3::ZERO {
            continue;
        }
        // Angle zero lays the box's first axis along world X, so a kd cell
        // on gentle ground can come out aligned with its own edges.
        let reference = if normal.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
        let u0 = (reference - normal * normal.dot(reference)).normalize();
        let v0 = normal.cross(u0);
        let in_plane = |degrees: f64| {
            let (sin, cos) = degrees.to_radians().sin_cos();
            let u = u0 * cos + v0 * sin;
            (u, normal.cross(u))
        };
        let area = |degrees: f64| {
            let (u, v) = in_plane(degrees);
            range(&samples, u) * range(&samples, v)
        };
        let coarse = (0..(90.0 / COARSE_STEP_DEGREES) as usize)
            .map(|step| step as f64 * COARSE_STEP_DEGREES)
            .min_by(|a, b| area(*a).total_cmp(&area(*b)))
            .unwrap_or(0.0);
        let fine_steps = (COARSE_STEP_DEGREES / FINE_STEP_DEGREES) as i32;
        let degrees = (-fine_steps..=fine_steps)
            .map(|step| coarse + f64::from(step) * FINE_STEP_DEGREES)
            .min_by(|a, b| area(*a).total_cmp(&area(*b)))
            .unwrap_or(coarse);
        let volume = area(degrees) * range(&samples, normal);
        if best.is_none_or(|(best_volume, _)| volume < best_volume) {
            let (u, v) = in_plane(degrees);
            best = Some((volume, [u, v, normal]));
        }
    }
    let Some((_, axes)) = best else {
        return aabb;
    };

    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = DVec3::splat(f64::NEG_INFINITY);
    for point in vertices.iter().map(&position) {
        let projected = DVec3::new(axes[0].dot(point), axes[1].dot(point), axes[2].dot(point));
        min = min.min(projected);
        max = max.max(projected);
    }
    let half_extents = (max - min) * 0.5 + padding;
    if half_extents.element_product() >= 0.9 * aabb.2.as_dvec3().element_product() {
        return aabb;
    }
    let center_local = (min + max) * 0.5;
    let center = chunk_origin + axes[0] * center_local.x + axes[1] * center_local.y + axes[2] * center_local.z;
    (center, axes.map(|axis| axis.as_vec3()), half_extents.as_vec3())
}

/// Orthonormal eigenvectors of a symmetric 3x3 matrix by cyclic Jacobi
/// rotations: each rotation zeroes one off-diagonal entry, and a handful of
/// sweeps drives all three to rounding noise.
fn symmetric_eigenvectors(matrix: DMat3) -> [DVec3; 3] {
    let mut a = matrix;
    let mut vectors = DMat3::IDENTITY;
    let scale = a.x_axis.x.abs() + a.y_axis.y.abs() + a.z_axis.z.abs();
    for _ in 0..16 {
        let off_diagonal = a.y_axis.x.powi(2) + a.z_axis.x.powi(2) + a.z_axis.y.powi(2);
        if off_diagonal <= (scale * 1e-12).powi(2) {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            let apq = a.col(q)[p];
            if apq.abs() <= f64::MIN_POSITIVE {
                continue;
            }
            let theta = (a.col(q)[q] - a.col(p)[p]) / (2.0 * apq);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            let mut rotation = DMat3::IDENTITY;
            rotation.col_mut(p)[p] = c;
            rotation.col_mut(q)[q] = c;
            rotation.col_mut(q)[p] = s;
            rotation.col_mut(p)[q] = -s;
            a = rotation.transpose() * a * rotation;
            vectors *= rotation;
        }
    }
    [vectors.x_axis.normalize(), vectors.y_axis.normalize(), vectors.z_axis.normalize()]
}
