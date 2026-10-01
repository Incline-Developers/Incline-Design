//! A reference surface: a thin plate spline through the selected points,
//! gridded on the snapped lattice and delivered as triangles between the
//! nodes, cut exactly to an extent: the mask drawn, else the points' outline
//! pushed out by a buffer. Every selected point shapes the spline, inside
//! the extent or outside it, so distant data still carries the trend; only
//! the surface is clipped to the boundary.

use anyhow::Context;
use glam::{DVec2, DVec3};

use super::*;
use crate::{
    app::jobs::CancelFlag,
    model::{
        Document, LayerId,
        kernel::{self, PolyContainment, SegSeg},
        progress::Progress,
        project::ModellingSettings,
        rbf::{self, DEFAULT_SPACING, MERGE_DISTANCE, RbfSurface, SteepPair},
        rbf_spans::{LatticeBox, RowRuns, SpanLattice},
    },
};

/// The fewest points a surface can be built from.
pub(crate) const MINIMUM_POINTS: usize = 3;

/// The fewest vertices a control string is usable from: two make a segment,
/// and a segment is what the surface is held along.
const MINIMUM_CONTROL_VERTICES: usize = 2;

/// How many overridden picks the report names one by one before it counts the
/// rest, so a build over a big string set cannot flood the console.
const OVERRIDE_LINES: usize = 20;

/// How far apart two controls' elevations may be where they cross in plan and
/// still count as one height; two interpretations meeting, so tighter than
/// [`kernel::Z_TOL`].
const CONTROL_AGREEMENT: f64 = 0.01;

/// The step the vertical extent is rounded out to, so the box reads as a
/// round number instead of an accident of where the picks happened to fall.
const EXTENT_ROUNDING: f64 = 10.0;

/// How far past the points' outline, in metres, a surface built without a
/// mask reaches.
const OUTLINE_BUFFER: f64 = 25.0;

/// The widest step, in degrees, between neighbouring points on the
/// buffer's rounded corners: at 25 m a chord sags under 0.1 m from its arc.
const OUTLINE_ARC_STEP: f64 = 10.0;

/// How many steep pairs the warning names one by one before it counts the
/// rest, as the override report does.
const STEEP_LINES: usize = 50;

/// One build's grid and the numbers the report is made of, kept apart from
/// the log line so they can be read back.
struct SurfaceMesh {
    /// The nodes the faces use and the points where they meet the extent's
    /// edge, at their heights.
    vertices: Vec<mesh_data::Vertex>,
    faces: Vec<[u32; 3]>,
    lattice: SpanLattice,
    /// Every node the build needs, in the lattice's order; NaN where the
    /// extent clips it. Counted for the log.
    heights: Vec<f64>,
    #[allow(dead_code)]
    spline: RbfSurface,
    /// Picks given.
    picks: usize,
    /// Points the spline passes through: the picks kept and the control
    /// points, after merging.
    used: usize,
    /// Points merged into a nearby one at the same height.
    merged: usize,
    /// Picks outside the extent: trend, not surface.
    support: usize,
    /// The vertical box the surface occupies, low and high.
    vertical_box: (f64, f64),
    /// Control strings the surface was made to pass through.
    controls: usize,
    /// Points those strings entered: their vertices, their segments
    /// densified at the grid spacing, and where they cross.
    control_points: usize,
    /// Plan positions where controls met each other.
    crossings: usize,
    /// Picks a control left out at another height.
    overridden: Vec<Override>,
    /// Every pick a control left out, at its height or not.
    left_out: usize,
    /// Points closer than the steep-pair distance and steeper than its
    /// angle, steepest first.
    steep: Vec<SteepPair>,
    /// The settings the surface was built with.
    settings: ModellingSettings,
}

/// A pick a control left out at another height: the plan position, the
/// pick's own elevation, and the control's.
type Override = (DVec2, f64, f64);

/// Who built a surface and when, for its run record.
#[derive(Clone, Default)]
struct RunStamp {
    author: String,
    date: String,
}

impl RunStamp {
    fn now() -> Self {
        Self {
            author: login_name(),
            date: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        }
    }
}

/// The account the build ran under. Incline keeps no user of its own, and
/// the browser offers none, so there it is unknown.
fn login_name() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    for key in ["USER", "USERNAME"] {
        if let Ok(name) = std::env::var(key)
            && !name.trim().is_empty()
        {
            return name;
        }
    }
    tr!("logging-unknown")
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
        _ => anyhow::bail!("{}", tr!("cmd-reference-surface-select-exactly-one-closed-string")),
    };
    Ok(SurfaceInput { points, layers, controls, extent })
}

impl<'a> App<'a> {
    /// Grid a thin plate spline through the selected points into a new
    /// surface, named for their layer when they share one. Every run adds a
    /// surface; nothing is replaced.
    pub(crate) fn build_reference_surface(&mut self, points: Vec<ObjectId>, controls: Vec<ObjectId>, extent: Option<ObjectId>) -> Result<()> {
        let project = self
            .workspace
            .active_project()
            .ok_or_else(|| anyhow::anyhow!("{}", tr!("cmd-reference-surface-open-project-before-building-surface")))?;
        // Read when the build starts, so a change made while it runs waits
        // for the next one.
        let settings = project.project.metadata.modelling;
        if let Some(problem) = settings.problem() {
            anyhow::bail!("{problem}");
        }
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
                .unwrap_or_else(|| tr!("tri-type-open-surface")),
            _ => tr!("tri-type-open-surface"),
        };
        // Said rather than guessed at: layers that disagree on a section send
        // the surface to its natural one, but layers that agree still keep it
        // beside them, so the line names where it actually went.
        if layers.len() > 1 {
            userspace_log!(
                "{}",
                tr!(
                    "cmd-reference-surface-selected-points-span-count-layers",
                    count = layers.len().to_string(),
                    section = crate::ui::state::ExplorerSection::from_kind(section).label().to_string()
                )
            );
        }
        // Said once, not guessed at: without a mask the surface runs to an
        // outline of the points' own, which may not be the ground the
        // geologist meant.
        if ring.is_none() {
            userspace_warn!("{}", tr!("cmd-reference-surface-no-mask-selected-surface-outline", buffer = OUTLINE_BUFFER.to_string()));
        }
        let stamp = RunStamp::now();

        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<crate::model::triangulation::GeneratedTriangulation> {
            if cancel.is_cancelled() {
                anyhow::bail!("{}", tr!("common-cancelled"));
            }
            grid_surface_from_points(points, controls, ring, &settings, name, &stamp, cancel, progress)
        };
        let apply = move |app: &mut App, result: Result<crate::model::triangulation::GeneratedTriangulation>| match result {
            Ok(generated) => app.insert_generated_triangulation_in(generated, section),
            Err(error) => crate::userspace_error!("{}", tr!("cmd-reference-surface-build-surface-failed-error", error = format!("{error:#}"))),
        };
        self.spawn_job_reporting_progress(tr!("cmd-reference-surface-building-surface"), vec![project_key], compute, apply);
        Ok(())
    }
}

fn too_few_points(count: usize) -> String {
    tr!(
        "cmd-reference-surface-count-point-s-selected-surface",
        count = count.to_string(),
        minimum = MINIMUM_POINTS.to_string()
    )
}

/// Control strings carry no name of their own, so a refusal names one by
/// where it sat in the selection, counting from one.
fn too_few_control_vertices(index: usize, count: usize) -> String {
    tr!(
        "cmd-reference-surface-control-string-index-has-count",
        index = (index + 1).to_string(),
        count = count.to_string(),
        minimum = MINIMUM_CONTROL_VERTICES.to_string()
    )
}

fn control_not_finite(index: usize) -> String {
    tr!("cmd-reference-surface-control-string-index-has-non", index = (index + 1).to_string())
}

fn control_not_available(index: usize) -> String {
    tr!("cmd-reference-surface-control-string-index-no-longer", index = (index + 1).to_string())
}

fn control_self_crossing(index: usize) -> String {
    tr!("cmd-reference-surface-control-string-index-crosses-itself", index = (index + 1).to_string())
}

fn control_doubles_back(index: usize) -> String {
    tr!("cmd-reference-surface-control-string-index-doubles-back", index = (index + 1).to_string())
}

fn control_ends_where_it_starts(index: usize) -> String {
    tr!("cmd-reference-surface-control-string-index-ends-where", index = (index + 1).to_string())
}

/// Stop the build once the job it runs in has been cancelled.
fn stop_if_cancelled(cancelled: &dyn Fn() -> bool) -> Result<()> {
    if cancelled() {
        anyhow::bail!("{}", tr!("common-cancelled"));
    }
    Ok(())
}

/// Two controls reading one plan position at two heights: which two, where,
/// and how far apart, for the geologist to decide between. The string
/// selected first is named first, each height beside its own string.
fn controls_disagree(left: usize, right: usize, position: DVec2, low: f64, high: f64) -> String {
    let (left, right, low, high) = if left <= right { (left, right, low, high) } else { (right, left, high, low) };
    tr!(
        "cmd-reference-surface-control-strings-b-disagree-x",
        a = (left + 1).to_string(),
        b = (right + 1).to_string(),
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
    tr!("cmd-reference-surface-control-strings-b-run-along", a = (left + 1).to_string(), b = (right + 1).to_string())
}

/// A control running along the extent's edge rather than across it leaves the
/// clip with no side to keep the string on.
fn control_along_extent(index: usize) -> String {
    tr!("cmd-reference-surface-control-string-index-runs-along", index = (index + 1).to_string())
}

fn too_few_points_inside(count: usize) -> String {
    tr!(
        "cmd-reference-surface-count-point-s-inside-extent",
        count = count.to_string(),
        minimum = MINIMUM_POINTS.to_string()
    )
}

/// The chosen extent as a plan ring. Only the plan shape travels: the ring's
/// elevations come from the surface, not from the height the string happens
/// to have been drawn at. Arcs are expanded because the clip works on
/// straight segments.
fn extent_ring(document: &crate::model::Document, id: ObjectId) -> Result<Vec<DVec2>> {
    let object = document
        .get_object(id)
        .ok_or_else(|| anyhow::anyhow!("{}", tr!("cmd-reference-surface-extent-string-no-longer-available")))?;
    let Object::Polyline { verts, closed: true, .. } = object else {
        anyhow::bail!("{}", tr!("cmd-reference-surface-extent-must-closed-string"));
    };
    let mut ring: Vec<DVec2> = crate::model::geometry::tessellate_polyline_bulges(verts, true)
        .iter()
        .map(|vertex| vertex.truncate())
        .collect();
    if ring.iter().any(|vertex| !vertex.is_finite()) {
        anyhow::bail!("{}", tr!("cmd-reference-surface-extent-string-has-non-finite"));
    }
    ring.dedup();
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    if ring.len() < MINIMUM_POINTS {
        anyhow::bail!("{}", tr!("cmd-reference-surface-extent-string-needs-least-three"));
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

/// Two vertices of one control too close in plan to be two points, at
/// heights too far apart to be one.
fn control_vertices_disagree(index: usize, position: DVec2) -> String {
    tr!(
        "cmd-reference-surface-control-string-index-has-two",
        index = (index + 1).to_string(),
        distance = MERGE_DISTANCE.to_string(),
        x = format!("{:.3}", position.x),
        y = format!("{:.3}", position.y)
    )
}

/// Worker half: the spline through the points and controls, gridded, cut
/// to the extent or the points' outline, and reported with its run record.
#[allow(clippy::too_many_arguments)]
fn grid_surface_from_points(
    points: Vec<DVec3>,
    controls: Vec<Vec<DVec3>>,
    extent: Option<Vec<DVec2>>,
    settings: &ModellingSettings,
    name: String,
    stamp: &RunStamp,
    cancel: &CancelFlag,
    progress: &Progress,
) -> Result<crate::model::triangulation::GeneratedTriangulation> {
    let surface = surface_mesh(&points, &controls, extent.as_deref(), settings, cancel, progress)?;
    userspace_log!(
        "{}\n{}",
        tr!(
            "cmd-reference-surface-built-surface-name-inside-grid",
            name = name.to_string(),
            inside = surface.heights.iter().filter(|height| !height.is_nan()).count().to_string(),
            vertex_count = surface.vertices.len().to_string(),
            face_count = surface.faces.len().to_string(),
            low = format!("{:.1}", surface.vertical_box.0),
            high = format!("{:.1}", surface.vertical_box.1),
            support = if surface.support == 0 {
                String::new()
            } else {
                tr!("cmd-reference-surface-count-point-s-outside-extent", count = surface.support.to_string())
            },
            controls = if surface.controls == 0 {
                String::new()
            } else {
                tr!(
                    "cmd-reference-surface-count-control-string-s-entered",
                    count = surface.controls.to_string(),
                    points = surface.control_points.to_string(),
                    crossings = if surface.crossings == 0 {
                        String::new()
                    } else {
                        format!(" {}", tr!("cmd-reference-surface-meeting-count-crossing-s", count = surface.crossings.to_string()))
                    }
                )
            }
        ),
        tr!(
            "cmd-reference-surface-run-record-used-point",
            used = surface.used.to_string(),
            picks = surface.picks.to_string(),
            merged = surface.merged.to_string(),
            left_out = surface.left_out.to_string(),
            overridden = surface.overridden.len().to_string(),
            method = surface.settings.method_description(),
            spacing = surface.lattice.spacing().to_string(),
            author = stamp.author.to_string(),
            date = stamp.date.to_string()
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
                tr!(
                    "cmd-reference-surface-control-string-overrides-pick-x",
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
            report.push_str(&tr!("cmd-reference-surface-and-more", more = (surface.overridden.len() - OVERRIDE_LINES).to_string()));
        }
        userspace_warn!("{}", report);
    }
    // So is every pair too steep for the grid, for the geologist to judge;
    // the surface is built from them all the same.
    if !surface.steep.is_empty() {
        userspace_warn!("{}", steep_report(&surface.steep, surface.settings.steep_distance, surface.settings.steep_degrees));
    }
    session::build_generated_triangulation(name, surface.vertices, surface.faces, TriSurfaceType::Surface, crate::model::triangulation::unique_edges)
}

/// Every pair of points closer than `within` in plan and steeper than
/// `degrees` between them, one line each in the order given, up to
/// [`STEEP_LINES`] and then a count of the rest.
fn steep_report(pairs: &[SteepPair], within: f64, degrees: f64) -> String {
    let mut report = tr!(
        "cmd-reference-surface-count-pair-s-points-closer",
        count = pairs.len().to_string(),
        spacing = within.to_string(),
        degrees = degrees.to_string()
    );
    for pair in pairs.iter().take(STEEP_LINES) {
        report.push('\n');
        report.push_str(&tr!(
            "cmd-reference-surface-steep-pair",
            ax = format!("{:.3}", pair.first.x),
            ay = format!("{:.3}", pair.first.y),
            az = format!("{:.3}", pair.first.z),
            bx = format!("{:.3}", pair.second.x),
            by = format!("{:.3}", pair.second.y),
            bz = format!("{:.3}", pair.second.z),
            distance = format!("{:.2}", pair.distance),
            rise = format!("{:.2}", pair.rise),
            slope = format!("{:.1}", pair.slope)
        ));
    }
    if pairs.len() > STEEP_LINES {
        report.push_str(&tr!("cmd-reference-surface-and-more", more = (pairs.len() - STEEP_LINES).to_string()));
    }
    report
}

/// Fit the spline through the picks and the controls, grid it over the
/// extent, else over the points' outline plus [`OUTLINE_BUFFER`], and
/// triangulate it cut to that ring.
///
/// A control enters as its vertices and its segments densified at the grid
/// spacing, so the spline holds the whole string; a pick the string passes
/// over is left out, the string winning.
fn surface_mesh(
    points: &[DVec3],
    controls: &[Vec<DVec3>],
    extent: Option<&[DVec2]>,
    settings: &ModellingSettings,
    cancel: &CancelFlag,
    progress: &Progress,
) -> Result<SurfaceMesh> {
    let cancelled = || cancel.is_cancelled();
    if points.len() < MINIMUM_POINTS {
        anyhow::bail!("{}", too_few_points(points.len()));
    }
    if points.iter().any(|point| !point.is_finite()) {
        anyhow::bail!("{}", tr!("cmd-reference-surface-selected-point-has-non-finite"));
    }
    // Every shape is read before anything is fitted, so a refusal names the
    // shape and builds nothing.
    // Controls in an order read off their own geometry, so the build cannot
    // depend on the order they were selected in; `names` keeps each one's
    // place in the selection for the messages.
    let names = canonical_order(controls);
    let ordered: Vec<Vec<DVec3>> = names.iter().map(|&index| controls[index].clone()).collect();
    let controls = ordered.as_slice();
    validate_controls(controls, &names, &cancelled)?;
    let crossings = control_crossings(controls, &names, &cancelled)?;
    let drawn = extent.map(RingBands::new);
    let mut support = 0usize;
    if let (Some(ring), Some(bands)) = (extent, drawn.as_ref()) {
        // A ring that crosses or touches itself bounds no single area, so
        // there is no answer to what the clip should keep. Judged on the
        // ring's own geometry: whether an extent is usable cannot depend on
        // where the picks happen to sit.
        if self_intersects(ring, true) {
            anyhow::bail!("{}", tr!("cmd-reference-surface-extent-string-crosses-itself-plan"));
        }
        refuse_controls_along(ring, controls, &names, &cancelled)?;
        support = points.iter().filter(|point| !inside(bands, point.truncate())).count();
        let covered = points.len() - support;
        if covered < MINIMUM_POINTS {
            anyhow::bail!("{}", too_few_points_inside(covered));
        }
    }

    let entered = control_points(controls, &names, &crossings, &cancelled)?;
    let (picks, overridden, left_out) = picks_off_controls(points, controls, &cancelled)?;
    let mut fitted = Vec::new();
    fitted.try_reserve_exact(picks.len() + entered.len()).context("Not enough memory for the surface points")?;
    fitted.extend(picks);
    fitted.extend(entered.iter().copied());

    // Without a mask the points' outline is the extent, cut exactly as a
    // drawn one is. Every fitted point is the buffer inside it, so the
    // refusals above that guard a drawn mask have nothing to find in it.
    let outline = match extent {
        Some(_) => Vec::new(),
        None => buffered_outline(&fitted)?,
    };
    let ring = extent.unwrap_or(&outline);
    let bands = drawn.unwrap_or_else(|| RingBands::new(ring));
    // The grid is planned before the fit, so a grid over the node budget
    // is refused before the system is allocated.
    let plan = grid_plan(ring, &bands, DEFAULT_SPACING, cancel)?;
    let spline = RbfSurface::fit(&fitted, cancel, &progress.phase(0.0, 0.5))?;
    // Read off the merged points, so a pair the merge settled is not named
    // again; a warning only, nothing is dropped or moved.
    let steep = rbf::steep_pairs(spline.points(), settings.steep_distance, settings.steep_degrees)?;
    let heights = plan.lattice.heights(|at| spline.height(at), |at| inside(&bands, at), cancel, &progress.phase(0.5, 0.95))?;
    let (vertices, faces) = grid_mesh(&plan, &heights, &spline, &cancelled)?;
    if faces.is_empty() {
        anyhow::bail!("{}", tr!("cmd-reference-surface-no-part-surface-falls-inside"));
    }
    let vertical_box = vertical_extent(
        vertices.iter().fold(f64::INFINITY, |low, vertex| low.min(vertex.z)),
        vertices.iter().fold(f64::NEG_INFINITY, |high, vertex| high.max(vertex.z)),
    );
    progress.phase(0.95, 1.0).finish();
    Ok(SurfaceMesh {
        vertices,
        faces,
        lattice: plan.lattice,
        heights,
        used: spline.point_count(),
        merged: spline.merged(),
        spline,
        picks: points.len(),
        support,
        vertical_box,
        controls: controls.len(),
        control_points: entered.len(),
        crossings: crossings.len(),
        overridden,
        left_out,
        steep,
        settings: *settings,
    })
}

/// Whether a node or a pick is on the ground a ring bounds. A pick surveyed
/// onto the string belongs to that ground, so the boundary counts as inside
/// here: the kernel names that case instead of leaving it to which way the
/// arithmetic fell.
fn inside(bands: &RingBands, at: DVec2) -> bool {
    matches!(bands.contains(at), PolyContainment::Inside | PolyContainment::OnBoundary)
}

/// The smallest plan box holding every position.
fn plan_box(positions: impl Iterator<Item = DVec2>) -> (DVec2, DVec2) {
    positions.fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(low, high), at| (low.min(at), high.max(at)))
}

/// The convex hull of the points in plan, counter-clockwise from the first
/// in plan order, with no vertex on a straight run (monotone chain). Read
/// off the points sorted, so their order does not matter. Points on one
/// spot give one vertex and points on one line its two ends, so a set the
/// fit will refuse still has an outline to size the grid by.
fn plan_hull(points: &[DVec3]) -> Result<Vec<DVec2>> {
    let mut sorted: Vec<DVec2> = Vec::new();
    sorted.try_reserve_exact(points.len()).context("Not enough memory for the outline")?;
    sorted.extend(points.iter().map(|point| point.truncate()));
    sorted.sort_unstable_by(plan_order);
    sorted.dedup();
    if sorted.len() < 3 {
        return Ok(sorted);
    }
    let mut hull: Vec<DVec2> = Vec::new();
    hull.try_reserve_exact(sorted.len() + 1).context("Not enough memory for the outline")?;
    // The lower chain left to right, then the upper one back; a vertex
    // that does not turn left is dropped.
    let turns_left = |hull: &[DVec2], at: DVec2| kernel::orient2d(hull[hull.len() - 2], hull[hull.len() - 1], at) > 0.0;
    for &at in &sorted {
        while hull.len() >= 2 && !turns_left(&hull, at) {
            hull.pop();
        }
        hull.push(at);
    }
    let lower = hull.len() + 1;
    for &at in sorted.iter().rev().skip(1) {
        while hull.len() >= lower && !turns_left(&hull, at) {
            hull.pop();
        }
        hull.push(at);
    }
    hull.pop();
    Ok(hull)
}

/// The extent of a build without a mask: the points' outline pushed out by
/// [`OUTLINE_BUFFER`]. Each hull edge moves out square to itself, and each
/// corner is rounded on the circle about its hull vertex, the points on the
/// circle at most [`OUTLINE_ARC_STEP`] apart. One spot gives a circle and
/// one line a slot with round ends. A point within [`kernel::XY_TOL`] of
/// the one before is left out, so a corner that barely turns adds no
/// sliver edge.
fn buffered_outline(points: &[DVec3]) -> Result<Vec<DVec2>> {
    let hull = plan_hull(points)?;
    let count = hull.len();
    let step = OUTLINE_ARC_STEP.to_radians();
    let most = if count == 1 {
        (std::f64::consts::TAU / step).ceil() as usize
    } else {
        count * ((std::f64::consts::PI / step).ceil() as usize + 2)
    };
    let mut ring: Vec<DVec2> = Vec::new();
    ring.try_reserve_exact(most).context("Not enough memory for the outline")?;
    let mut push = |at: DVec2| {
        if ring.last().is_none_or(|last| last.distance(at) > kernel::XY_TOL) {
            ring.push(at);
        }
    };
    let on_circle = |centre: DVec2, angle: f64| centre + OUTLINE_BUFFER * DVec2::new(libm::cos(angle), libm::sin(angle));
    if count == 1 {
        let pieces = (std::f64::consts::TAU / step).ceil() as usize;
        for piece in 0..pieces {
            push(on_circle(hull[0], std::f64::consts::TAU * piece as f64 / pieces as f64));
        }
    } else {
        // The outward side of an edge of a counter-clockwise ring is its
        // right. Two vertices make a ring of two edges, one each way, so
        // each end turns through half a circle.
        let outward = |from: DVec2, to: DVec2| {
            let along = to - from;
            DVec2::new(along.y, -along.x)
        };
        for index in 0..count {
            let (before, at, after) = (hull[(index + count - 1) % count], hull[index], hull[(index + 1) % count]);
            let (incoming, outgoing) = (outward(before, at), outward(at, after));
            let start = libm::atan2(incoming.y, incoming.x);
            let turn = libm::atan2(incoming.perp_dot(outgoing), incoming.dot(outgoing)).max(0.0);
            let pieces = (turn / step).ceil().max(1.0) as usize;
            for piece in 0..=pieces {
                push(on_circle(at, start + turn * piece as f64 / pieces as f64));
            }
        }
    }
    while ring.len() > 1 && ring[0].distance(ring[ring.len() - 1]) <= kernel::XY_TOL {
        ring.pop();
    }
    Ok(ring)
}

/// A control running along the extent's edge rather than across it has no
/// side of the clip to keep it on, so it is refused.
fn refuse_controls_along(ring: &[DVec2], controls: &[Vec<DVec3>], names: &[usize], cancelled: &dyn Fn() -> bool) -> Result<()> {
    let edges = BoxGrid::new((0..ring.len()).map(|edge| segment_box(ring[edge], ring[(edge + 1) % ring.len()])).collect());
    let mut near = Vec::new();
    for (index, control) in controls.iter().enumerate() {
        stop_if_cancelled(cancelled)?;
        for segment in control.windows(2) {
            let (start, end) = (segment[0].truncate(), segment[1].truncate());
            let (low, high) = segment_box(start, end);
            edges.overlapping(low, high, &mut near);
            for &edge in &near {
                if let SegSeg::CollinearOverlap { .. } = kernel::segment_segment(start, end, ring[edge], ring[(edge + 1) % ring.len()]) {
                    anyhow::bail!("{}", control_along_extent(names[index]));
                }
            }
        }
    }
    Ok(())
}

/// The points the controls enter the fit as: where they cross, then each
/// control's vertices with its segments densified at the grid spacing in
/// between. A point within [`MERGE_DISTANCE`] of one already entered is the
/// same point and goes in once; the two must agree on its height.
fn control_points(controls: &[Vec<DVec3>], names: &[usize], crossings: &[Crossing], cancelled: &dyn Fn() -> bool) -> Result<Vec<DVec3>> {
    let mut entered = PlanCells::default();
    for crossing in crossings {
        entered.add(crossing.position.extend(crossing.z), crossing.sides[0].control);
    }
    for (index, control) in controls.iter().enumerate() {
        stop_if_cancelled(cancelled)?;
        let mut enter = |point: DVec3| -> Result<()> {
            match entered.first_near(point.truncate()) {
                None => entered.add(point, index),
                Some((kept, owner)) if (kept.z - point.z).abs() > CONTROL_AGREEMENT => {
                    if owner == index {
                        anyhow::bail!("{}", control_vertices_disagree(names[index], point.truncate()));
                    }
                    anyhow::bail!("{}", controls_disagree(names[owner], names[index], point.truncate(), kept.z, point.z));
                }
                Some(_) => {}
            }
            Ok(())
        };
        enter(control[0])?;
        for segment in control.windows(2) {
            let (start, end) = (segment[0], segment[1]);
            let pieces = (start.truncate().distance(end.truncate()) / DEFAULT_SPACING).ceil().max(1.0) as usize;
            for step in 1..pieces {
                enter(start + (end - start) * step as f64 / pieces as f64)?;
            }
            enter(end)?;
        }
    }
    Ok(entered.points)
}

/// The picks no control passes within [`MERGE_DISTANCE`] of in plan, with
/// the ones a control left out at another height, reported, and how many it
/// left out in all. Of two controls that near a pick, the nearer speaks.
fn picks_off_controls(points: &[DVec3], controls: &[Vec<DVec3>], cancelled: &dyn Fn() -> bool) -> Result<(Vec<DVec3>, Vec<Override>, usize)> {
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
    let reach = DVec2::splat(MERGE_DISTANCE);
    let (mut kept, mut overridden, mut left_out) = (Vec::new(), Vec::new(), 0usize);
    kept.try_reserve_exact(points.len()).context("Not enough memory for the surface points")?;
    let mut near = Vec::new();
    for point in points {
        stop_if_cancelled(cancelled)?;
        let at = point.truncate();
        grid.overlapping(at - reach, at + reach, &mut near);
        let nearest = near
            .iter()
            .filter_map(|&found| {
                let (index, segment) = segments[found];
                let (start, end) = (controls[index][segment], controls[index][segment + 1]);
                let (closest, along) = kernel::project_onto_segment(at, start.truncate(), end.truncate());
                let distance = closest.distance(at);
                (distance <= MERGE_DISTANCE).then(|| (distance, elevation_along(start, end, along)))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0));
        match nearest {
            None => kept.push(*point),
            Some((_, z)) => {
                left_out += 1;
                if (z - point.z).abs() > kernel::Z_TOL {
                    overridden.push((at, point.z, z));
                }
            }
        }
    }
    Ok((kept, overridden, left_out))
}

/// Control points entered so far, filed by plan position so a new one finds
/// the first within [`MERGE_DISTANCE`] without walking them all.
#[derive(Default)]
struct PlanCells {
    points: Vec<DVec3>,
    owners: Vec<usize>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl PlanCells {
    /// Cells are a metre across, far wider than the distance, so a match is
    /// always in the cell a position falls in or one beside it.
    fn key(position: DVec2) -> (i64, i64) {
        (position.x.floor() as i64, position.y.floor() as i64)
    }

    fn add(&mut self, point: DVec3, owner: usize) {
        self.cells.entry(Self::key(point.truncate())).or_default().push(self.points.len());
        self.points.push(point);
        self.owners.push(owner);
    }

    /// The earliest point within [`MERGE_DISTANCE`] of a position, and the
    /// control that entered it.
    fn first_near(&self, position: DVec2) -> Option<(DVec3, usize)> {
        let (column, row) = Self::key(position);
        (column.saturating_sub(1)..=column.saturating_add(1))
            .flat_map(|column| (row.saturating_sub(1)..=row.saturating_add(1)).map(move |row| (column, row)))
            .filter_map(|key| self.cells.get(&key))
            .flatten()
            .copied()
            .filter(|&index| self.points[index].truncate().distance(position) < MERGE_DISTANCE)
            .min()
            .map(|index| (self.points[index], self.owners[index]))
    }
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

/// How one control reaches a crossing: which of its segments.
struct CrossingSide {
    control: usize,
    segment: usize,
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
    (CrossingSide { control: index, segment }, z)
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
            inside ^= crosses_rightward(a, b, point);
        }
        if self.edges >= 3 && inside { PolyContainment::Inside } else { PolyContainment::Outside }
    }

    /// The edges filed in the band level with `y`: every edge that comes
    /// within [`SEARCH_MARGIN`] of it in y, and perhaps others.
    fn edges_near(&self, y: f64) -> impl Iterator<Item = (DVec2, DVec2)> + '_ {
        let count = self.ring.len();
        let band = cell_of(y, self.low, self.height, self.starts.len() - 1);
        self.members[self.starts[band]..self.starts[band + 1]]
            .iter()
            .map(move |&index| (self.ring[index], self.ring[(index + 1) % count]))
    }

    /// Inside by the crossing count alone, with no tolerance: for a point
    /// known to be clear of the ring, which [`Self::contains`] could still
    /// call on it when a thin face puts its middle a hair from an edge.
    fn encloses(&self, point: DVec2) -> bool {
        let count = self.ring.len();
        if count == 0 || self.edges < 3 {
            return false;
        }
        let band = cell_of(point.y, self.low, self.height, self.starts.len() - 1);
        self.members[self.starts[band]..self.starts[band + 1]]
            .iter()
            .map(|&index| (self.ring[index], self.ring[(index + 1) % count]))
            .filter(|(a, b)| a != b && crosses_rightward(*a, *b, point))
            .count()
            % 2
            == 1
    }
}

/// Whether the edge from `a` to `b` crosses the ray from `point` towards
/// +x, counted once where two edges meet on the ray.
fn crosses_rightward(a: DVec2, b: DVec2, point: DVec2) -> bool {
    let upward = a.y <= point.y && point.y < b.y;
    let downward = b.y <= point.y && point.y < a.y;
    (upward && kernel::orient2d(a, b, point) > 0.0) || (downward && kernel::orient2d(a, b, point) < 0.0)
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

/// What the grid is sized from before the fit: the nodes the build needs,
/// the cells the extent's edge cuts with the faces covering their part
/// inside it, and the cells wholly inside it.
struct GridPlan {
    lattice: SpanLattice,
    edge_faces: Vec<[DVec2; 3]>,
    /// The cells wholly inside the extent, as runs along each row of cells.
    whole: RowRuns,
}

/// The grid over `ring` at `spacing`: the nodes inside it or on its edge,
/// which are gridded, and the corners of every cell the mesh keeps, wholly
/// inside or cut by the edge (see [`mask_outline`] and [`cut_cells`]), and
/// no others. Rows are walked only near the edges: between two edges no
/// edge comes near a row, so one test answers for every node or cell of the
/// stretch. Refuses more nodes than the budget before any is allocated.
fn grid_plan(ring: &[DVec2], bands: &RingBands, spacing: f64, cancel: &CancelFlag) -> Result<GridPlan> {
    let cancelled = || cancel.is_cancelled();
    let (lower, upper) = plan_box(ring.iter().copied());
    let bounds = LatticeBox::covering(lower, upper, spacing)?;
    let outline = mask_outline(ring, spacing, &cancelled)?;
    let outline_bands = RingBands::new(&outline);
    let (cut, edge_faces) = cut_cells(&outline, &bounds, &outline_bands, &cancelled)?;
    let (cell_columns, cell_rows) = (bounds.columns().saturating_sub(1), bounds.rows().saturating_sub(1));
    let cut_in = |row: usize| {
        let from = cut.partition_point(|&(cut_row, _)| cut_row < row);
        let to = cut.partition_point(|&(cut_row, _)| cut_row <= row);
        &cut[from..to]
    };
    // A cell the edge leaves alone is wholly inside or wholly outside, and
    // its middle, half a cell from any edge, says which.
    let middle = DVec2::splat(spacing / 2.0);
    let whole = RowRuns::collect(cell_rows, cancel, |row, runs| {
        let cut_here: Vec<usize> = cut_in(row).iter().map(|&(_, column)| column).collect();
        let keep = |column: usize| cut_here.binary_search(&column).is_err() && outline_bands.encloses(bounds.node(column, row) + middle);
        runs_near_edges(&outline_bands, bounds.node(0, row).y + middle.y, &bounds, cell_columns, &cut_here, keep, runs);
        Ok(())
    })?;
    let needed = RowRuns::collect(bounds.rows(), cancel, |row, runs| {
        let mut spans: Vec<[u32; 2]> = Vec::new();
        runs_near_edges(
            bands,
            bounds.node(0, row).y,
            &bounds,
            bounds.columns(),
            &[],
            |column| inside(bands, bounds.node(column, row)),
            &mut spans,
        );
        // A kept cell's corners in this row: the cells below it and above.
        for cells in [row.checked_sub(1), (row < cell_rows).then_some(row)].into_iter().flatten() {
            spans.extend(whole.row(cells).iter().map(|&[start, end]| [start, end + 1]));
            spans.extend(cut_in(cells).iter().map(|&(_, column)| [column as u32, column as u32 + 2]));
        }
        spans.sort_unstable();
        let first = runs.len();
        for [start, end] in spans {
            match runs.len().checked_sub(1).filter(|&last| last >= first && runs[last][1] >= start) {
                Some(last) => runs[last][1] = runs[last][1].max(end),
                None => runs.push([start, end]),
            }
        }
        Ok(())
    })?;
    let lattice = SpanLattice::new(bounds, needed, ring)?;
    Ok(GridPlan { lattice, edge_faces, whole })
}

/// Runs of the columns `0..count` along one row, at height `y`, that `keep`
/// accepts, appended to `runs`. Every column within a spacing of an edge of
/// `bands` that comes within [`SEARCH_MARGIN`] of the row is tested, as is
/// every column in `also`, ascending; each stretch between them is tested
/// once, at its first column, since no edge crosses the row or comes that
/// near it there, so every column of the stretch answers alike.
fn runs_near_edges(bands: &RingBands, y: f64, bounds: &LatticeBox, count: usize, also: &[usize], keep: impl Fn(usize) -> bool, runs: &mut Vec<[u32; 2]>) {
    if count == 0 {
        return;
    }
    let spacing = bounds.spacing();
    let column = |x: f64| x / spacing - bounds.first()[0] as f64;
    let clamp = |value: f64| value.clamp(0.0, (count - 1) as f64) as usize;
    let mut near: Vec<(usize, usize)> = bands
        .edges_near(y)
        .filter(|(a, b)| a.y.max(b.y) >= y - SEARCH_MARGIN && a.y.min(b.y) <= y + SEARCH_MARGIN)
        .map(|(a, b)| {
            // The edge's x across the band of rows within the margin.
            let x_at = |at: f64| a.x + (b.x - a.x) * ((at - a.y) / (b.y - a.y)).clamp(0.0, 1.0);
            let (from, to) = if a.y == b.y { (a.x, b.x) } else { (x_at(y - SEARCH_MARGIN), x_at(y + SEARCH_MARGIN)) };
            (clamp(column(from.min(to) - spacing).floor() - 1.0), clamp(column(from.max(to) + spacing).ceil() + 1.0))
        })
        .chain(also.iter().map(|&column| (column, column)))
        .collect();
    near.sort_unstable();
    let first = runs.len();
    let mut add = |from: usize, to: usize| match runs.len().checked_sub(1).filter(|&last| last >= first && runs[last][1] as usize == from) {
        Some(last) => runs[last][1] = to as u32,
        None => runs.push([from as u32, to as u32]),
    };
    let mut next = 0;
    for (from, to) in near {
        if next < from && keep(next) {
            add(next, from);
        }
        for column in from.max(next)..=to {
            if keep(column) {
                add(column, column + 1);
            }
        }
        next = next.max(to + 1);
    }
    if next < count && keep(next) {
        add(next, count);
    }
}

/// Two triangles per lattice cell, split along the diagonal from its lowest
/// corner, counter-clockwise in plan so their normals point up. A cell
/// wholly inside the mask keeps that split and a cell its edge passes
/// through is cut along the edge, so the surface ends on the mask itself
/// (see [`grid_plan`]). Nodes take their heights from the grid and the
/// points on the mask's edge from the spline. Memory for the vertices and
/// faces is reserved before either is made.
fn grid_mesh(plan: &GridPlan, heights: &[f64], spline: &RbfSurface, cancelled: &dyn Fn() -> bool) -> Result<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)> {
    const UNUSED: u32 = u32::MAX;
    let lattice = &plan.lattice;
    // The number of a run of `count` nodes from `column` along `row`, which
    // the plan keeps in one run of the lattice.
    let run = |column: usize, row: usize, count: usize| {
        lattice
            .index(column, row)
            .filter(|&first| lattice.index(column + count - 1, row) == Some(first + count - 1))
            .ok_or_else(|| anyhow::anyhow!("{}", cut_failed(lattice.node(column, row))))
    };
    let mut index_of: Vec<u32> = Vec::new();
    index_of.try_reserve_exact(lattice.node_count()).context("Not enough memory for the surface nodes")?;
    index_of.resize(lattice.node_count(), UNUSED);
    let mut face_count = plan.edge_faces.len();
    for row in 0..plan.whole.rows() {
        stop_if_cancelled(cancelled)?;
        for &[start, end] in plan.whole.row(row) {
            let (start, cells) = (start as usize, (end - start) as usize);
            face_count += 2 * cells;
            for corners in [row, row + 1] {
                let first = run(start, corners, cells + 1)?;
                index_of[first..=first + cells].fill(0);
            }
        }
    }
    // The edge faces' corners: nodes are nodes, the rest follow them in
    // plan order.
    let node_of = |at: DVec2| -> Result<Option<usize>> {
        match lattice.bounds().node_at(at) {
            Some((column, row)) => lattice.index(column, row).map(Some).ok_or_else(|| anyhow::anyhow!("{}", cut_failed(at))),
            None => Ok(None),
        }
    };
    let mut extra: Vec<DVec2> = Vec::new();
    extra.try_reserve_exact(3 * plan.edge_faces.len()).context("Not enough memory for the surface nodes")?;
    for &corner in plan.edge_faces.iter().flatten() {
        match node_of(corner)? {
            Some(index) => index_of[index] = 0,
            None => extra.push(corner),
        }
    }
    extra.sort_unstable_by(plan_order);
    extra.dedup();
    let used = index_of.iter().filter(|&&index| index != UNUSED).count();
    let (mut vertices, mut faces) = (Vec::new(), Vec::new());
    vertices.try_reserve_exact(used + extra.len()).context("Not enough memory for the surface nodes")?;
    faces.try_reserve_exact(face_count).context("Not enough memory for the surface faces")?;
    for ((slot, at), &height) in index_of.iter_mut().zip(lattice.nodes()).zip(heights) {
        if *slot != UNUSED {
            *slot = vertices.len() as u32;
            // A node the edge was bent onto may sit a hair outside the
            // mask and so not be gridded; the spline gives it the same
            // height the grid would have.
            let height = if height.is_nan() { spline.height(at) } else { height };
            vertices.push(mesh_data::Vertex::new(at.x, at.y, height));
        }
    }
    let first_extra = vertices.len();
    vertices.extend(extra.iter().map(|at| mesh_data::Vertex::new(at.x, at.y, spline.height(*at))));
    for row in 0..plan.whole.rows() {
        stop_if_cancelled(cancelled)?;
        for &[start, end] in plan.whole.row(row) {
            let (start, cells) = (start as usize, (end - start) as usize);
            let (below, above) = (run(start, row, cells + 1)?, run(start, row + 1, cells + 1)?);
            for cell in 0..cells {
                let (a, b, c, d) = (below + cell, below + cell + 1, above + cell + 1, above + cell);
                faces.extend([[a, b, c], [a, c, d]].map(|triangle| triangle.map(|index| index_of[index])));
            }
        }
    }
    for face in &plan.edge_faces {
        let mut corners = [0u32; 3];
        for (slot, corner) in corners.iter_mut().zip(face) {
            *slot = match node_of(*corner)? {
                Some(index) => index_of[index],
                None => extra
                    .binary_search_by(|at| plan_order(at, corner))
                    .map(|found| (first_extra + found) as u32)
                    .map_err(|_| anyhow::anyhow!("{}", cut_failed(*corner)))?,
            };
        }
        faces.push(corners);
    }
    Ok((vertices, faces))
}

fn cut_failed(position: DVec2) -> String {
    tr!(
        "cmd-reference-surface-surface-could-not-cut",
        x = format!("{:.3}", position.x),
        y = format!("{:.3}", position.y)
    )
}

/// x, then y.
fn plan_order(a: &DVec2, b: &DVec2) -> std::cmp::Ordering {
    a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y))
}

/// The gridline at or below `value`, counted in spacings from zero, and
/// whether `value` is on it. Corrected after the division so it agrees
/// exactly with the node positions, which are the count times the spacing.
fn gridline_below(value: f64, spacing: f64) -> (i64, bool) {
    let mut line = (value / spacing).floor() as i64;
    while line as f64 * spacing > value {
        line -= 1;
    }
    while (line + 1) as f64 * spacing <= value {
        line += 1;
    }
    (line, line as f64 * spacing == value)
}

/// The gridlines strictly between `low` and `high`, counted as above.
fn gridlines_between(low: f64, high: f64, spacing: f64) -> std::ops::RangeInclusive<i64> {
    let (below, _) = gridline_below(low, spacing);
    let (mut last, on) = gridline_below(high, spacing);
    if on {
        last -= 1;
    }
    below + 1..=last
}

/// A point moved onto the node nearest it when within [`kernel::XY_TOL`].
fn snap_to_node(at: DVec2, spacing: f64) -> DVec2 {
    let nearest = (at / spacing).round() * spacing;
    if at.distance(nearest) <= kernel::XY_TOL { nearest } else { at }
}

/// A mask vertex moved onto a node that close, else onto a gridline that
/// close, so the cut leaves no sliver beside either.
fn snap_to_lattice(at: DVec2, spacing: f64) -> DVec2 {
    let nearest = (at / spacing).round() * spacing;
    if at.distance(nearest) <= kernel::XY_TOL {
        return nearest;
    }
    let snap = |value: f64, line: f64| if (value - line).abs() <= kernel::XY_TOL { line } else { value };
    DVec2::new(snap(at.x, nearest.x), snap(at.y, nearest.y))
}

/// The mask's ring as the mesh follows it. A vertex within
/// [`kernel::XY_TOL`] of a node or a gridline moves onto it, and an edge
/// passing that close to a node is bent through it, so the cut leaves no
/// sliver there. Every edge is then split where it crosses a gridline, so
/// each piece lies in one cell. Edges are worked from their lower end, so
/// the points are the same whichever way the ring runs and wherever it
/// starts.
fn mask_outline(ring: &[DVec2], spacing: f64, cancelled: &dyn Fn() -> bool) -> Result<Vec<DVec2>> {
    let mut snapped: Vec<DVec2> = Vec::new();
    snapped.try_reserve_exact(ring.len()).context("Not enough memory for the extent")?;
    snapped.extend(ring.iter().map(|&vertex| snap_to_lattice(vertex, spacing)));
    snapped.dedup();
    while snapped.len() > 1 && snapped.first() == snapped.last() {
        snapped.pop();
    }
    let count = snapped.len();
    let edge = |index: usize| (snapped[index], snapped[(index + 1) % count]);
    // At most the gridlines an edge spans on each axis, and a node or two
    // beside each line along its longer axis.
    let bound = |(start, end): (DVec2, DVec2)| {
        let span = (end - start).abs() / spacing;
        (span.x + span.y + 2.0 * span.max_element() + 12.0) as usize
    };
    let most = (0..count).map(|index| bound(edge(index))).fold(count, usize::saturating_add);
    let mut outline: Vec<DVec2> = Vec::new();
    outline.try_reserve_exact(most).context("Not enough memory for the extent")?;
    let mut events: Vec<(f64, DVec2)> = Vec::new();
    for index in 0..count {
        stop_if_cancelled(cancelled)?;
        let (start, end) = edge(index);
        events.clear();
        events.try_reserve(bound((start, end))).context("Not enough memory for the extent")?;
        let forward = plan_order(&start, &end).is_le();
        let (low, high) = if forward { (start, end) } else { (end, start) };
        edge_events(low, high, spacing, &mut events);
        outline.push(start);
        if forward {
            outline.extend(events.iter().map(|&(_, at)| at));
        } else {
            outline.extend(events.iter().rev().map(|&(_, at)| at));
        }
    }
    outline.dedup();
    while outline.len() > 1 && outline.first() == outline.last() {
        outline.pop();
    }
    Ok(outline)
}

/// The points strictly inside the edge from `low` to `high`, `low` first in
/// plan order: nodes the edge passes within [`kernel::XY_TOL`] of, and its
/// gridline crossings, a crossing that close to a node taken as the node.
/// Each comes with how far along the edge it is, and they are sorted by it.
fn edge_events(low: DVec2, high: DVec2, spacing: f64, events: &mut Vec<(f64, DVec2)>) {
    let span = high - low;
    let (bottom, top) = (low.y.min(high.y), low.y.max(high.y));
    for line in gridlines_between(low.x, high.x, spacing) {
        let x = line as f64 * spacing;
        let along = (x - low.x) / span.x;
        events.push((along, snap_to_node(DVec2::new(x, low.y + span.y * along), spacing)));
    }
    for line in gridlines_between(bottom, top, spacing) {
        let y = line as f64 * spacing;
        let along = (y - low.y) / span.y;
        events.push((along, snap_to_node(DVec2::new(low.x + span.x * along, y), spacing)));
    }
    // Nodes near the edge, a gridline at a time along its longer axis,
    // where the line's offset across it is at most the tolerance times the
    // square root of two.
    let steep = span.y.abs() > span.x;
    let (from, to) = if steep { (bottom, top) } else { (low.x, high.x) };
    let reach = 2.0 * kernel::XY_TOL;
    for line in gridlines_between(from - reach, to + reach, spacing) {
        let along_axis = line as f64 * spacing;
        let across = if steep {
            low.x + span.x * (along_axis - low.y) / span.y
        } else {
            low.y + span.y * (along_axis - low.x) / span.x
        };
        for other in gridlines_between(across - reach, across + reach, spacing) {
            let other = other as f64 * spacing;
            let node = if steep { DVec2::new(other, along_axis) } else { DVec2::new(along_axis, other) };
            let (closest, along) = kernel::project_onto_segment(node, low, high);
            if node != low && node != high && node.distance(closest) <= kernel::XY_TOL {
                events.push((along, node));
            }
        }
    }
    events.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(plan_order(&a.1, &b.1)));
}

/// A lattice cell by its row, then its column, so cells sort row by row.
type CellAt = (usize, usize);

/// Where the outline meets one lattice cell: a point of it, or a piece of
/// it running through the cell's inside.
enum Touch {
    Point(DVec2),
    Piece(DVec2, DVec2),
}

/// The cells the outline passes through, ascending, and the faces covering
/// the part of each inside the mask. A cell is cut when a piece of the
/// outline runs through it or one of its points sits in it or on a side
/// between corners, so a neighbour never meets it along a side it splits.
/// Each is triangulated with the outline as constraints (spade's
/// constrained Delaunay), its corners and points loaded in plan order so
/// the result depends only on the cell, and a face is kept when its middle
/// is inside the outline.
fn cut_cells(outline: &[DVec2], lattice: &LatticeBox, bands: &RingBands, cancelled: &dyn Fn() -> bool) -> Result<(Vec<CellAt>, Vec<[DVec2; 3]>)> {
    use spade::Triangulation as _;
    let (spacing, first) = (lattice.spacing(), lattice.first());
    let (cell_columns, cell_rows) = (lattice.columns().saturating_sub(1), lattice.rows().saturating_sub(1));
    let cell = |column: i64, row: i64| {
        let (column, row) = (usize::try_from(column - first[0]).ok()?, usize::try_from(row - first[1]).ok()?);
        (column < cell_columns && row < cell_rows).then_some((row, column))
    };
    let count = outline.len();
    let mut touches: Vec<(CellAt, Touch)> = Vec::new();
    touches.try_reserve_exact(3 * count).context("Not enough memory for the extent")?;
    for index in 0..count {
        let (start, end) = (outline[index], outline[(index + 1) % count]);
        let ((column, on_column), (row, on_row)) = (gridline_below(start.x, spacing), gridline_below(start.y, spacing));
        let sides: &[(i64, i64)] = match (on_column, on_row) {
            (true, true) => &[],
            (true, false) => &[(-1, 0), (0, 0)],
            (false, true) => &[(0, -1), (0, 0)],
            (false, false) => &[(0, 0)],
        };
        for (left, down) in sides {
            if let Some(found) = cell(column + left, row + down) {
                touches.push((found, Touch::Point(start)));
            }
        }
        let along_side = (start.x == end.x && on_column) || (start.y == end.y && on_row);
        let middle = (start + end) / 2.0;
        if !along_side && let Some(found) = cell(gridline_below(middle.x, spacing).0, gridline_below(middle.y, spacing).0) {
            touches.push((found, Touch::Piece(start, end)));
        }
    }
    touches.sort_by_key(|&(found, _)| found);
    let (mut cut, mut faces) = (Vec::new(), Vec::new());
    let (mut points, mut pieces, mut kept) = (Vec::new(), Vec::new(), Vec::new());
    for group in touches.chunk_by(|a, b| a.0 == b.0) {
        stop_if_cancelled(cancelled)?;
        let found = group[0].0;
        let (row, column) = found;
        points.clear();
        points.extend([(0, 0), (1, 0), (1, 1), (0, 1)].map(|(right, up)| lattice.node(column + right, row + up)));
        for (_, touch) in group {
            match *touch {
                Touch::Point(at) => points.push(at),
                Touch::Piece(start, end) => points.extend([start, end]),
            }
        }
        points.sort_unstable_by(plan_order);
        points.dedup();
        pieces.clear();
        for (_, touch) in group {
            if let Touch::Piece(start, end) = *touch {
                let find = |at: &DVec2| points.binary_search_by(|point| plan_order(point, at)).unwrap_or_default();
                let (a, b) = (find(&start), find(&end));
                pieces.push([a.min(b), a.max(b)]);
            }
        }
        pieces.sort_unstable();
        pieces.dedup();
        let mut cdt = spade::ConstrainedDelaunayTriangulation::<spade::Point2<f64>>::new();
        let mut handles = Vec::with_capacity(points.len());
        for at in &points {
            handles.push(cdt.insert(spade::Point2::new(at.x, at.y)).map_err(|_| anyhow::anyhow!("{}", cut_failed(*at)))?);
        }
        for &[a, b] in &pieces {
            if !cdt.can_add_constraint(handles[a], handles[b]) {
                anyhow::bail!("{}", cut_failed(points[a]));
            }
            cdt.add_constraint(handles[a], handles[b]);
        }
        kept.clear();
        for face in cdt.inner_faces() {
            let [mut a, mut b, mut c] = face.positions().map(|at| DVec2::new(at.x, at.y));
            let turn = kernel::orient2d(a, b, c);
            if turn == 0.0 || !bands.encloses((a + b + c) / 3.0) {
                continue;
            }
            if turn < 0.0 {
                std::mem::swap(&mut b, &mut c);
            }
            // Turned to start at its first corner in plan order, so the
            // cell's faces sort into one order however spade listed them.
            while plan_order(&a, &b).is_gt() || plan_order(&a, &c).is_gt() {
                (a, b, c) = (b, c, a);
            }
            kept.push([a, b, c]);
        }
        kept.sort_unstable_by(|x, y| {
            x.iter()
                .zip(y)
                .map(|(p, q)| plan_order(p, q))
                .find(|order| order.is_ne())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        cut.try_reserve(1).context("Not enough memory for the surface cells")?;
        faces.try_reserve(kept.len()).context("Not enough memory for the surface faces")?;
        cut.push(found);
        faces.extend_from_slice(&kept);
    }
    Ok((cut, faces))
}
