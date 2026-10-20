use std::collections::{HashMap, VecDeque};

use super::{
    cuts::{lerp_at_z, point_in_triangle_bary_z},
    *,
};

impl<'a> App<'a> {
    /// Generate contour polylines from a triangulation and store them in a new
    /// or existing layer in the active project. Major contours (multiples
    /// of `major_interval`) use `major_color`; all others use `minor_color`.
    /// `contour_labels` also writes each major line's height on it as text.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn generate_contour_triangulation(
        &mut self,
        tri_id: TriangulationId,
        major_interval: f64,
        minor_interval: f64,
        major_color: [f32; 4],
        minor_color: [f32; 4],
        z_range: Option<(f64, f64)>,
        contour_labels: bool,
        output_layer: ContourOutputLayer,
    ) -> Result<()> {
        if !minor_interval.is_finite() || !major_interval.is_finite() || minor_interval <= 0.0 || major_interval <= 0.0 {
            anyhow::bail!("Intervals must be positive finite numbers");
        }
        if minor_interval < 1e-6 {
            anyhow::bail!("Minor contour interval is too small (minimum 0.000001); this would generate an unbounded number of contour levels");
        }
        if major_interval < minor_interval {
            anyhow::bail!("Major interval must be >= minor interval");
        }

        let (mesh, spatial, tri_name) = {
            let tri = self
                .triangulations
                .iter()
                .find(|t| t.id == tri_id)
                .ok_or_else(|| anyhow::anyhow!("Triangulation not found"))?;
            (tri.mesh.clone(), tri.spatial.clone(), tri.name.clone())
        };

        let bounds = mesh.bounds();
        let mut z_lo = bounds.min.z;
        let mut z_hi = bounds.max.z;
        if let Some((range_lo, range_hi)) = z_range {
            if !range_lo.is_finite() || !range_hi.is_finite() || range_lo >= range_hi {
                anyhow::bail!("Contour Z range must be finite with min < max");
            }
            z_lo = z_lo.max(range_lo);
            z_hi = z_hi.min(range_hi);
            if z_lo >= z_hi {
                anyhow::bail!("Contour Z range does not overlap the triangulation");
            }
        }

        if (z_hi - z_lo).abs() < 1e-10 {
            anyhow::bail!("Triangulation has no Z extent to contour");
        }

        const MAX_CONTOUR_LEVELS: f64 = 100_000.0;
        let level_estimate = (z_hi - z_lo) / minor_interval;
        if level_estimate > MAX_CONTOUR_LEVELS {
            anyhow::bail!(
                "These settings would generate about {level_estimate:.0} contour levels over a Z extent of {:.1} m. \
                 Increase the minor interval, or restrict the Z range to contour a slice of the triangulation.",
                z_hi - z_lo
            );
        }

        let active_project = self
            .workspace
            .active_project()
            .ok_or_else(|| anyhow::anyhow!("No active project to store the contours in"))?;
        match &output_layer {
            ContourOutputLayer::New(name) => {
                if name.trim().is_empty() {
                    anyhow::bail!("Enter a name for the new contour layer");
                }
                if active_project.project.document.layer_id_by_name(name).is_some() {
                    anyhow::bail!("A layer named '{name}' already exists; select it as the output layer or choose a different name");
                }
            }
            ContourOutputLayer::Existing(layer_id) => {
                if active_project.project.document.layer(*layer_id).is_none() {
                    anyhow::bail!("The selected contour output layer no longer exists");
                }
            }
        }

        // Snapshot the project identity so the apply step can verify the
        // contours still have a home when the job completes.
        let project_runtime_id = active_project.runtime_id;

        let apply_tri_name = tri_name.clone();
        type ContourLines = Vec<(Vec<glam::DVec3>, crate::model::ObjectColor)>;
        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> Result<(ContourLines, Vec<ContourLabel>)> {
            let polylines = compute_contour_polylines(&mesh, z_lo, z_hi, minor_interval, major_interval, major_color, minor_color, cancel, progress)?;
            // The generator reports on the whole bar, so the labels then
            // count their lines on it afresh.
            let labels = if contour_labels {
                contour_height_labels(&mesh, &spatial, &polylines, major_interval, cancel, progress)?
            } else {
                userspace_log!("contour labels: off"); // R177 diag
                Vec::new()
            };
            Ok((polylines, labels))
        };
        let apply = move |app: &mut App, result: Result<(ContourLines, Vec<ContourLabel>)>| {
            let (polylines, labels) = match result {
                Ok(output) => output,
                Err(error) => {
                    userspace_warn!("{}", tr!("cmd-contours-contour-generation-failed-error", error = format!("{error:#}")));
                    return;
                }
            };
            let project_is_active = app.workspace.active_project().is_some_and(|project| project.runtime_id == project_runtime_id);
            let Some(project) = app.workspace.projects.iter_mut().find(|project| project.runtime_id == project_runtime_id) else {
                userspace_warn!("{}", tr!("cmd-contours-discarded-project-closed", name = apply_tri_name.to_string()));
                return;
            };
            let (layer_id, layer_name, create_layer) = match output_layer {
                ContourOutputLayer::New(layer_name) => {
                    if project.project.document.layer_id_by_name(&layer_name).is_some() {
                        userspace_warn!(
                            "{}",
                            tr!(
                                "cmd-contours-discarded-layer-exists",
                                name = apply_tri_name.to_string(),
                                layer_name = layer_name.to_string()
                            )
                        );
                        return;
                    }
                    let layer_id = project.project.document.allocate_layer_id();
                    (layer_id, layer_name, true)
                }
                ContourOutputLayer::Existing(layer_id) => {
                    let Some(layer_name) = project.project.document.layer(layer_id).map(|layer| layer.name.clone()) else {
                        userspace_warn!("{}", tr!("cmd-contours-discarded-layer-deleted", name = apply_tri_name.to_string()));
                        return;
                    };
                    (layer_id, layer_name, false)
                }
            };
            let mut objects: Vec<Object> = polylines
                .into_iter()
                .map(|(verts, color)| Object::Polyline {
                    id: project.project.document.allocate_object_id(),
                    layer: layer_id,
                    verts: verts.into_iter().map(crate::model::PolyVertex::straight).collect(),
                    closed: false,
                    color,
                    fill: crate::model::FillStyle::Clear,
                    line_weight: 1.0,
                })
                .collect();
            let line_count = objects.len();
            objects.extend(labels.into_iter().map(|label| Object::Text {
                id: project.project.document.allocate_object_id(),
                layer: layer_id,
                pos: label.pos,
                content: label.content,
                height: label.height,
                rotation: label.rotation,
                color: crate::model::ObjectColor::Fixed(major_color),
            }));
            userspace_log!("contour labels: adding {} text objects to {layer_name}", objects.len() - line_count); // R177 diag
            let command = if create_layer {
                let layer = Layer {
                    id: layer_id,
                    name: layer_name.clone(),
                    color_index: None,
                    color: [1.0, 1.0, 1.0, 1.0],
                    loaded: true,
                    hidden: false,
                    elevation: 0.0,
                    folder: None,
                    section: SectionKind::natural_layer(),
                };
                crate::model::Command::AddLayerSnapshot { layer, objects }
            } else {
                crate::model::Command::Batch(objects.into_iter().map(crate::model::Command::AddObject).collect())
            };
            app.execute_edit_for(project_runtime_id, command);
            if project_is_active {
                app.editor.active_layer = Some(layer_id);
            }
            userspace_log!(
                "{}",
                tr!(
                    "cmd-contours-generated",
                    line_count = line_count.to_string(),
                    name = apply_tri_name.to_string(),
                    layer_name = layer_name.to_string()
                )
            );
            app.invalidate_geometry();
        };
        self.spawn_job_reporting_progress("Generating contours…", vec![crate::app::jobs::JobKey::Triangulation(tri_id)], compute, apply);
        Ok(())
    }
}

/// Contour a mesh with a Z sweep: faces are sorted by minimum Z once and an
/// active-face set follows the ascending levels, so intermediate memory stays
/// O(faces + active faces + output) instead of copying each face index into
/// every level bucket it crosses (tall/vertical triangles made that
/// O(face-level intersections)).
#[allow(clippy::too_many_arguments)]
fn compute_contour_polylines(
    mesh: &mesh_data::Triangulation,
    z_lo: f64,
    z_hi: f64,
    minor_interval: f64,
    major_interval: f64,
    major_color: [f32; 4],
    minor_color: [f32; 4],
    cancel: &crate::app::jobs::CancelFlag,
    progress: &crate::model::progress::Progress,
) -> Result<Vec<(Vec<glam::DVec3>, crate::model::ObjectColor)>> {
    let verts_raw = mesh.vertices();
    let mut contour_faces: Vec<ContourFace> = mesh
        .face_vertex_indices_iter()
        .map(|face| {
            let vertices = [verts_raw[face[0]], verts_raw[face[1]], verts_raw[face[2]]];
            let z_min = vertices.iter().map(|vertex| vertex.z).fold(f64::INFINITY, f64::min);
            let z_max = vertices.iter().map(|vertex| vertex.z).fold(f64::NEG_INFINITY, f64::max);
            ContourFace { vertices, z_min, z_max }
        })
        .collect();
    contour_faces.sort_by(|a, b| a.z_min.total_cmp(&b.z_min));

    let levels = contour_levels(z_lo, z_hi, minor_interval, major_interval);
    // One pass per level, so levels finished is an exact measure of the work.
    let level_count = levels.len() as u64;
    let mut polylines: Vec<(Vec<glam::DVec3>, crate::model::ObjectColor)> = Vec::new();
    let mut active: Vec<usize> = Vec::new();
    let mut next_face = 0usize;

    for (levels_done, z_level) in levels.into_iter().enumerate() {
        if cancel.is_cancelled() {
            anyhow::bail!("Cancelled");
        }
        progress.set_items(levels_done as u64, level_count);
        // Admit faces whose span has reached this level, then retire the ones
        // it has passed. A face crosses the level when z_min <= level < z_max.
        while next_face < contour_faces.len() && contour_faces[next_face].z_min <= z_level {
            active.push(next_face);
            next_face += 1;
        }
        active.retain(|&face_index| contour_faces[face_index].z_max > z_level);

        let is_major = is_major_contour(z_level, major_interval);
        let color = if is_major { major_color } else { minor_color };
        let line_color = crate::model::ObjectColor::Fixed(color);
        let mut level_segments = Vec::new();
        for &face_index in &active {
            let face = &contour_faces[face_index];
            if let Some(seg) = triangle_contour_segment(face.vertices, z_level) {
                level_segments.push([glam::DVec3::new(seg[0].x, seg[0].y, seg[0].z), glam::DVec3::new(seg[1].x, seg[1].y, seg[1].z)]);
            }
        }
        polylines.extend(chain_contour_segments(level_segments).into_iter().map(|verts| (verts, line_color)));
    }

    if polylines.is_empty() {
        anyhow::bail!("No contour segments were generated for the selected intervals");
    }
    Ok(polylines)
}

struct ContourFace {
    vertices: [mesh_data::Vertex; 3],
    z_min: f64,
    z_max: f64,
}

fn contour_levels(z_lo: f64, z_hi: f64, minor_interval: f64, major_interval: f64) -> Vec<f64> {
    let mut levels = Vec::new();
    append_levels(&mut levels, z_lo, z_hi, minor_interval);
    append_levels(&mut levels, z_lo, z_hi, major_interval);
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|a, b| (*a - *b).abs() <= 1e-8);
    levels
}

fn append_levels(levels: &mut Vec<f64>, z_lo: f64, z_hi: f64, interval: f64) {
    let first = (z_lo / interval).ceil();
    let last = (z_hi / interval).floor();
    if first > last {
        return;
    }
    let count = (last - first) as usize;
    for i in 0..=count {
        levels.push((first + i as f64) * interval);
    }
}

fn is_major_contour(z_level: f64, major_interval: f64) -> bool {
    let nearest = (z_level / major_interval).round() * major_interval;
    (z_level - nearest).abs() <= 1e-6
}

/// Quantized endpoint key. Both triangles adjacent to a mesh edge interpolate
/// the crossing from the same two vertices, so matching endpoints agree to
/// float noise; quantizing at the kernel tolerance makes them hash-equal.
fn contour_point_key(p: glam::DVec3) -> (i64, i64, i64) {
    let q = crate::model::kernel::XY_TOL;
    ((p.x / q).round() as i64, (p.y / q).round() as i64, (p.z / q).round() as i64)
}

fn chain_contour_segments(segments: Vec<[glam::DVec3; 2]>) -> Vec<Vec<glam::DVec3>> {
    // Endpoint key -> segments touching that point, for O(1) chain extension.
    let mut by_endpoint: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (index, segment) in segments.iter().enumerate() {
        by_endpoint.entry(contour_point_key(segment[0])).or_default().push(index);
        by_endpoint.entry(contour_point_key(segment[1])).or_default().push(index);
    }

    let mut used = vec![false; segments.len()];
    let mut chains = Vec::new();

    let mut take_neighbor = |used: &mut Vec<bool>, point: glam::DVec3| -> Option<glam::DVec3> {
        let candidates = by_endpoint.get_mut(&contour_point_key(point))?;
        while let Some(index) = candidates.pop() {
            if used[index] {
                continue;
            }
            used[index] = true;
            let [a, b] = segments[index];
            let key = contour_point_key(point);
            return Some(if contour_point_key(a) == key { b } else { a });
        }
        None
    };

    for seed in 0..segments.len() {
        if used[seed] {
            continue;
        }
        used[seed] = true;
        let [a, b] = segments[seed];
        let mut chain: VecDeque<glam::DVec3> = VecDeque::from([a, b]);
        while let Some(next) = take_neighbor(&mut used, *chain.back().unwrap()) {
            chain.push_back(next);
        }
        while let Some(previous) = take_neighbor(&mut used, *chain.front().unwrap()) {
            chain.push_front(previous);
        }
        if chain.len() >= 2 {
            chains.push(chain.into_iter().collect());
        }
    }
    chains
}

pub(super) fn triangle_contour_segment(v: [mesh_data::Vertex; 3], z_level: f64) -> Option<[mesh_data::Vertex; 2]> {
    let mut pts: Vec<mesh_data::Vertex> = Vec::with_capacity(2);
    for i in 0..3 {
        let a = v[i];
        let b = v[(i + 1) % 3];
        if (a.z <= z_level && z_level < b.z) || (b.z <= z_level && z_level < a.z) {
            pts.push(lerp_at_z(a, b, z_level));
        }
    }
    if pts.len() == 2 { Some([pts[0], pts[1]]) } else { None }
}

/// Label height as a share of the plan spacing between neighbouring major
/// lines: centred on its line, a label then clears the next minor line at
/// the usual five minors to a major.
const CONTOUR_LABEL_SPACING_SHARE: f64 = 0.3;

/// The steepest ground, as rise over run (1:1, 45 degrees), whose spacing
/// sets the label height: walls and faces would shrink it to nothing.
const CONTOUR_LABEL_STEEPEST_SLOPE: f64 = 1.0;

/// A major line's height as text on it, anchored top-left as `Object::Text`.
#[derive(Debug)]
struct ContourLabel {
    pos: glam::DVec3,
    content: String,
    height: f64,
    rotation: f64,
}

/// One label per major line on the exposed straight stretch nearest its
/// middle, read along the line with its top uphill; a line shorter than
/// twice its label gets none. All share one height, a round share of the
/// typical plan spacing between major lines on gentle ground, capped by the
/// mesh's extent.
fn contour_height_labels(
    mesh: &mesh_data::Triangulation,
    spatial: &crate::model::spatial::TriangleBvh,
    polylines: &[(Vec<glam::DVec3>, crate::model::ObjectColor)],
    major_interval: f64,
    cancel: &crate::app::jobs::CancelFlag,
    progress: &crate::model::progress::Progress,
) -> Result<Vec<ContourLabel>> {
    let majors: Vec<(&[glam::DVec3], Vec<f64>)> = polylines
        .iter()
        .map(|(verts, _)| verts.as_slice())
        .filter(|verts| verts.len() >= 2 && is_major_contour(verts[0].z, major_interval))
        .map(|line| (line, plan_stations(line)))
        .collect();
    let bounds = mesh.bounds();
    let extent = (bounds.max.x - bounds.min.x).max(bounds.max.y - bounds.min.y);
    let mut stack = Vec::new();
    // The spacing of major lines is the interval over the slope across them,
    // read on gentle ground when there is any.
    let mut slopes: Vec<f64> = majors
        .iter()
        .filter_map(|(line, stations)| {
            let (middle, _) = point_along(line, stations, stations[stations.len() - 1] * 0.5);
            surface_slope_at(mesh, spatial, &mut stack, middle.truncate(), middle.z).filter(|&slope| slope > 1e-9)
        })
        .collect();
    if slopes.iter().any(|&slope| slope <= CONTOUR_LABEL_STEEPEST_SLOPE) {
        slopes.retain(|&slope| slope <= CONTOUR_LABEL_STEEPEST_SLOPE);
    }
    slopes.sort_by(f64::total_cmp);
    let raw_height = slopes
        .get(slopes.len() / 2)
        .map_or(extent / 100.0, |slope| major_interval / slope * CONTOUR_LABEL_SPACING_SHARE);
    let Some(height) = round_down_1_2_5(raw_height.min(extent / 40.0)) else {
        userspace_log!("contour labels: majors {}, no height (raw {raw_height}, extent {extent})", majors.len()); // R177 diag
        return Ok(Vec::new());
    };
    let decimals = contour_label_decimals(major_interval);
    let mut text_system = crate::rendering::text::TextSystem::new();

    let mut labels = Vec::new();
    let (mut too_short, mut first_width) = (0usize, None); // R177 diag
    for (done, (line, stations)) in majors.iter().enumerate() {
        if cancel.is_cancelled() {
            anyhow::bail!("Cancelled");
        }
        progress.set_items(done as u64, majors.len() as u64);
        let level = line[0].z;
        let content = contour_label_text(level, decimals);
        let layout_width = shaped_text_width(&mut text_system, &content, height);
        first_width.get_or_insert(layout_width); // R177 diag
        let level_box = crate::model::geometry::text_bounds_corners_with_layout_width(glam::DVec3::ZERO, &content, height, 0.0, layout_width);
        let box_width = level_box[1].x - level_box[0].x;
        let length = stations[stations.len() - 1];
        if length < 2.0 * box_width {
            too_short += 1; // R177 diag
            continue;
        }
        let window = 1.2 * box_width;
        let middle = length * 0.5;
        // Exposed ground first: a stretch under another sheet of the mesh (a
        // floor line under its roof, or under an overhang) is hidden.
        let mut exposed = |start: f64, end: f64| {
            let (centre, _) = point_along(line, stations, (start + end) * 0.5);
            !is_covered(mesh, spatial, &mut stack, centre.truncate(), level, 0.01 * height)
        };
        let (start, end) = window_near(length, middle, window, |start, end| {
            is_straight(line, stations, start, end, 0.25 * height) && exposed(start, end)
        })
        .or_else(|| window_near(length, middle, window, &mut exposed))
        .unwrap_or((middle - window * 0.5, middle + window * 0.5));
        let (from, _) = point_along(line, stations, start);
        let (to, _) = point_along(line, stations, end);
        let (centre, segment) = point_along(line, stations, (start + end) * 0.5);
        let Some(along) = (to - from).truncate().try_normalize() else {
            continue;
        };
        let rise = surface_rise_across(mesh, spatial, &mut stack, centre, along.perp(), segment_reach(line, segment, height));
        // Off the surface, or level: read along +x.
        let uphill_left = rise.filter(|rise| rise.abs() > 1e-12).map_or(along.x >= 0.0, |rise| rise > 0.0);
        let reading = if uphill_left { along } else { -along };
        let rotation = reading.y.atan2(reading.x);
        // Centred on the line, raised clear of the ground under its uphill
        // half: its top corners and top middle, or the slope off the surface.
        let corners = crate::model::geometry::text_bounds_corners_with_layout_width(glam::DVec3::ZERO, &content, height, rotation, layout_width);
        let mut pos = centre - (corners[0] + corners[2]) * 0.5;
        let half_up = (corners[0] - corners[3]) * 0.5;
        let mut ground: Option<f64> = None;
        for point in [pos + corners[0], pos + corners[1], centre + half_up] {
            if let Some(z) = surface_z_near(mesh, spatial, &mut stack, point.truncate(), level) {
                ground = Some(ground.map_or(z, |highest| highest.max(z)));
            }
        }
        let clear = ground.map_or_else(|| level + rise.map_or(0.0, f64::abs) * half_up.length(), |ground| ground.max(level));
        pos.z = clear + 0.01 * height;
        labels.push(ContourLabel { pos, content, height, rotation });
    }
    // R177 diag
    userspace_log!(
        "contour labels: majors {}, height {height}, too short {too_short}, placed {}, first text width {first_width:?}, plan extent {extent:.0}, first label {:?} at {:?}",
        majors.len(),
        labels.len(),
        labels.first().map(|label| label.content.as_str()),
        labels.first().map(|label| label.pos.to_array().map(|value| value.round()))
    );
    Ok(labels)
}

/// The width the renderer shapes `content` to as document text at `height`,
/// measured as rendering/scene/build.rs measures it for text bounds.
fn shaped_text_width(text_system: &mut crate::rendering::text::TextSystem, content: &str, height: f64) -> f64 {
    let mut textbox = crate::rendering::text::TextBox::new(vec![crate::rendering::text::Text::new(content.to_owned(), [1.0; 4])], 1.0);
    textbox.font_size = crate::rendering::graphics::DOC_TEXT_FONT_SIZE;
    let scale = (height / f64::from(textbox.font_size)).max(f64::EPSILON);
    f64::from(textbox.layout_width(text_system, (f32::MAX, f32::MAX))) * scale
}

/// Plan distance from the start of `line` to each of its vertices.
fn plan_stations(line: &[glam::DVec3]) -> Vec<f64> {
    let mut total = 0.0;
    std::iter::once(0.0)
        .chain(line.windows(2).map(|pair| {
            total += pair[0].truncate().distance(pair[1].truncate());
            total
        }))
        .collect()
}

/// The point `station` along `line` in plan, and the segment it lies on.
fn point_along(line: &[glam::DVec3], stations: &[f64], station: f64) -> (glam::DVec3, usize) {
    let segment = stations.partition_point(|&at| at <= station).clamp(1, line.len() - 1) - 1;
    let span = stations[segment + 1] - stations[segment];
    let t = if span > 0.0 { ((station - stations[segment]) / span).clamp(0.0, 1.0) } else { 0.0 };
    (line[segment].lerp(line[segment + 1], t), segment)
}

/// How far either side of a segment to sample the ground: a quarter of its
/// plan length, which follows the size of the triangle it was cut from, and
/// at least a hundredth of the label height.
fn segment_reach(line: &[glam::DVec3], segment: usize, height: f64) -> f64 {
    (0.25 * line[segment].truncate().distance(line[segment + 1].truncate())).max(0.01 * height).max(1e-6)
}

/// Start and end stations of the `window` long stretch of a `length` long
/// line nearest `middle` that `accept` takes.
fn window_near(length: f64, middle: f64, window: f64, mut accept: impl FnMut(f64, f64) -> bool) -> Option<(f64, f64)> {
    let half = window * 0.5;
    let mut fits = |centre: f64| {
        let (start, end) = (centre - half, centre + half);
        (start >= 0.0 && end <= length && accept(start, end)).then_some((start, end))
    };
    if let Some(found) = fits(middle) {
        return Some(found);
    }
    let mut offset = window * 0.1;
    while middle - offset - half >= 0.0 || middle + offset + half <= length {
        if let Some(found) = fits(middle + offset).or_else(|| fits(middle - offset)) {
            return Some(found);
        }
        offset += window * 0.1;
    }
    None
}

/// Whether every point of `line` from `start` to `end` lies within
/// `tolerance` of the chord between them.
fn is_straight(line: &[glam::DVec3], stations: &[f64], start: f64, end: f64, tolerance: f64) -> bool {
    let (from, first) = point_along(line, stations, start);
    let (to, last) = point_along(line, stations, end);
    let Some(along) = (to - from).truncate().try_normalize() else {
        return false;
    };
    line[first + 1..=last]
        .iter()
        .all(|vertex| along.perp_dot(vertex.truncate() - from.truncate()).abs() <= tolerance)
}

/// How fast the ground rises from `centre` towards `across`, a unit plan
/// direction, sampled `reach` either side; `None` off the surface.
fn surface_rise_across(
    mesh: &mesh_data::Triangulation,
    spatial: &crate::model::spatial::TriangleBvh,
    stack: &mut Vec<usize>,
    centre: glam::DVec3,
    across: glam::DVec2,
    reach: f64,
) -> Option<f64> {
    let point = centre.truncate();
    let ahead = surface_z_near(mesh, spatial, stack, point + across * reach, centre.z);
    let behind = surface_z_near(mesh, spatial, stack, point - across * reach, centre.z);
    match (ahead, behind) {
        (Some(ahead), Some(behind)) => Some((ahead - behind) / (2.0 * reach)),
        (Some(ahead), None) => Some((ahead - centre.z) / reach),
        (None, Some(behind)) => Some((centre.z - behind) / reach),
        (None, None) => None,
    }
}

/// The ground height at `point` on the sheet nearest `level`, where folds
/// and overhangs stack several.
fn surface_z_near(mesh: &mesh_data::Triangulation, spatial: &crate::model::spatial::TriangleBvh, stack: &mut Vec<usize>, point: glam::DVec2, level: f64) -> Option<f64> {
    let vertices = mesh.vertices();
    let mut nearest: Option<f64> = None;
    spatial.for_each_xy_bounds_candidate_index_with_stack(point, point, stack, |index| {
        if let Some(face) = mesh.face_vertex_indices(index)
            && let Some(z) = point_in_triangle_bary_z(point.x, point.y, [vertices[face[0]], vertices[face[1]], vertices[face[2]]])
            && nearest.is_none_or(|best| (z - level).abs() < (best - level).abs())
        {
            nearest = Some(z);
        }
    });
    nearest
}

/// The steepness, rise over run, of the triangle under `point` on the sheet
/// nearest `level`; a vertical one has none.
fn surface_slope_at(mesh: &mesh_data::Triangulation, spatial: &crate::model::spatial::TriangleBvh, stack: &mut Vec<usize>, point: glam::DVec2, level: f64) -> Option<f64> {
    let vertices = mesh.vertices();
    let mut nearest: Option<(f64, f64)> = None;
    spatial.for_each_xy_bounds_candidate_index_with_stack(point, point, stack, |index| {
        if let Some(face) = mesh.face_vertex_indices(index)
            && let triangle = [vertices[face[0]], vertices[face[1]], vertices[face[2]]]
            && let Some(z) = point_in_triangle_bary_z(point.x, point.y, triangle)
            && nearest.is_none_or(|(best, _)| (z - level).abs() < (best - level).abs())
        {
            let [a, b, c] = triangle.map(|vertex| glam::DVec3::new(vertex.x, vertex.y, vertex.z));
            let normal = (b - a).cross(c - a);
            nearest = Some((z, normal.truncate().length() / normal.z.abs()));
        }
    });
    nearest.map(|(_, slope)| slope).filter(|slope| slope.is_finite())
}

/// Whether another sheet of the mesh lies more than `clearance` above
/// `level` at `point`: a roof over a floor line, or an overhang.
fn is_covered(mesh: &mesh_data::Triangulation, spatial: &crate::model::spatial::TriangleBvh, stack: &mut Vec<usize>, point: glam::DVec2, level: f64, clearance: f64) -> bool {
    let vertices = mesh.vertices();
    let mut covered = false;
    spatial.for_each_xy_bounds_candidate_index_with_stack(point, point, stack, |index| {
        if !covered
            && let Some(face) = mesh.face_vertex_indices(index)
            && let Some(z) = point_in_triangle_bary_z(point.x, point.y, [vertices[face[0]], vertices[face[1]], vertices[face[2]]])
        {
            covered = z > level + clearance;
        }
    });
    covered
}

/// The largest 1, 2 or 5 step at or below `value`.
fn round_down_1_2_5(value: f64) -> Option<f64> {
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let magnitude = 10f64.powf(value.log10().floor());
    [5.0, 2.0, 1.0].into_iter().map(|step| step * magnitude).find(|&step| step <= value * (1.0 + 1e-9))
}

/// The fewest decimals that write every multiple of `interval` apart: none
/// at 10 m, one at 2.5 m, six at the finest interval the dialog allows, at
/// most the fifteen an f64 carries.
fn contour_label_decimals(interval: f64) -> usize {
    (0..15)
        .find(|&decimals| {
            let scaled = interval * 10f64.powi(decimals as i32);
            scaled.round() >= 1.0 && (scaled - scaled.round()).abs() <= 1e-6 * scaled
        })
        .unwrap_or(15)
}

/// `level` with `decimals` places, never "-0".
fn contour_label_text(level: f64, decimals: usize) -> String {
    let text = format!("{level:.decimals$}");
    match text.strip_prefix('-') {
        Some(unsigned) if unsigned.chars().all(|c| c == '0' || c == '.') => unsigned.to_owned(),
        _ => text,
    }
}
