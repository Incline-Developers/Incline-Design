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
    app::{commands::string_clean::forget_ring_menu, jobs::CancelFlag},
    model::{
        Command, Document, LayerId,
        control_checks::{
            BoxGrid, Crossing, MisshapenControls, PlanCells, SEARCH_MARGIN, SELF_SHAPE_LINES, canonical_order, cell_of, cells_across, control_crossings, control_points,
            control_points_spaced, elevation_along, grid_cell, nearer_end, segment_box, self_intersects, stop_if_cancelled, too_few_control_vertices, validate_controls,
        },
        kernel::{self, PolyContainment, SegSeg},
        progress::Progress,
        project::ModellingSettings,
        rbf::{self, DEFAULT_SPACING, MERGE_DISTANCE, POINT_BUDGET, RbfSurface, SteepPair},
        rbf_spans::{LatticeBox, RowRuns, SpanLattice},
        string_clean::{self, LeftOut, NotLeftOut, OddOneOut, Problem, ProblemKind},
    },
    ui::state::{StringRing, StringRingKind},
};

pub(crate) mod seam;

/// The fewest points a surface can be built from.
pub(crate) const MINIMUM_POINTS: usize = 3;

/// How many overridden picks the report names one by one before it counts the
/// rest, so a build over a big string set cannot flood the console.
const OVERRIDE_LINES: usize = 20;

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

/// How many places a line naming a string left out of the build gives
/// before it counts the rest.
const LEFT_OUT_PLACES: usize = 5;

/// The shape tolerances, in metres, a build whose control strings would
/// pass the spline's point budget thins its copy of them at, tried in turn
/// until the vertices kept fit: a vertex is kept when the string without it
/// would pass more than the tolerance from it, in plan or in height. The
/// first is the half metre within which Clean Strings joins two strings.
const SHAPE_TOLERANCES: [f64; 4] = [string_clean::JOIN_AUTOMATIC, 1.0, 2.0, 5.0];

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
    /// Control strings left out of this build so the rest pass its checks,
    /// in the order they were chosen, by place in the selection.
    left_out_strings: Vec<LeftOut>,
    /// What the build cleaned in its copy of the controls, each change by
    /// the place in the selection of the string it came from.
    cleaned: Vec<string_clean::Change>,
    /// Points those strings entered: their vertices, their segments
    /// densified at the grid spacing, and where they cross; when they would
    /// pass the spline's point budget, the points of their thinned copy.
    control_points: usize,
    /// How the copy of the control strings was thinned to fit the spline's
    /// point budget, when it was.
    thinned: Option<Thinning>,
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

/// How a build thinned its copy of the control strings to fit the
/// spline's point budget: the points the vertices kept make, with where the
/// strings cross, the shape tolerance in metres they were kept at, and the
/// spacing in plan, in metres, points were put along the strings at.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Thinning {
    kept: usize,
    tolerance: f64,
    spacing: f64,
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

/// One ring per refused position, naming its string and the drawn vertex at
/// that position when there is one.
fn refused_rings(ids: &[ObjectId], refused: &[usize], positions: &[DVec3], vertex_of: impl Fn(ObjectId, DVec3) -> Option<usize>) -> Vec<StringRing> {
    refused
        .iter()
        .zip(positions)
        .filter_map(|(&index, &at)| {
            let id = *ids.get(index)?;
            Some(StringRing {
                at,
                kind: StringRingKind::Refused,
                sides: vec![(id, vertex_of(id, at))],
                title: None,
            })
        })
        .collect()
}

impl<'a> App<'a> {
    /// The output name Build Surface offers for a surface made from
    /// `points`: the seam's surface name when they are reference points made
    /// this session, else their one layer's name, else the generic word.
    pub(crate) fn reference_surface_name(&self, points: &[ObjectId]) -> String {
        let mut layers: Vec<LayerId> = Vec::new();
        for id in points {
            if let Some(Object::Point { layer, .. }) = self.scene_document.get_object(*id)
                && !layers.contains(layer)
            {
                layers.push(*layer);
            }
        }
        let source = self.workspace.active_project().and_then(|project| self.reference_source_of(project.runtime_id, &layers));
        match (&source, layers.len()) {
            (Some(source), _) => crate::app::commands::thickness_points::seam_names::surface(source.target.name(), source.side),
            (None, 1) => self
                .scene_document
                .layer(layers[0])
                .map(|layer| layer.name.clone())
                .unwrap_or_else(|| tr!("tri-type-open-surface")),
            _ => tr!("tri-type-open-surface"),
        }
    }

    /// Grid a thin plate spline through the selected points into a new
    /// surface named `name`. Every run adds a surface; nothing is replaced.
    pub(crate) fn build_reference_surface(&mut self, points: Vec<ObjectId>, controls: Vec<ObjectId>, extent: Option<ObjectId>, name: String) -> Result<()> {
        // The last refusal's rings go as the next build starts, whatever it
        // finds; a fresh refusal rings its own.
        if !self.editor.string_rings.is_empty() {
            self.editor.clear_string_rings();
            self.redraw_requested = true;
        }
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
        let control_ids = controls;
        let controls = control_strings(&self.scene_document, &control_ids)?;
        let runtime_id = project.runtime_id;
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
        let source = self.reference_source_of(runtime_id, &layers);
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
        let keys: Vec<u64> = control_ids.iter().map(|id| id.0).collect();
        // Each control's layer, so the build's copy is cleaned layer by
        // layer as Clean Strings cleans.
        let control_layers: Vec<u64> = control_ids
            .iter()
            .map(|id| self.scene_document.get_object(*id).map_or(0, |object| object.layer().0))
            .collect();

        let compute = move |cancel: &CancelFlag, progress: &Progress| -> Result<(crate::model::triangulation::GeneratedTriangulation, Vec<LeftOut>)> {
            if cancel.is_cancelled() {
                anyhow::bail!("{}", tr!("common-cancelled"));
            }
            let selected = SelectedControls {
                strings: &controls,
                keys: &keys,
                layers: &control_layers,
            };
            grid_surface_from_points(points, selected, ring, &settings, name, &stamp, cancel, progress)
        };
        let apply = move |app: &mut App, result: Result<(crate::model::triangulation::GeneratedTriangulation, Vec<LeftOut>)>| match result {
            Ok((generated, left_out)) => {
                app.insert_generated_triangulation_in(generated, section);
                app.keep_surface_source(source);
                // The strings left out stay selected and ringed, nothing
                // hidden: the surface is there to look at beside them.
                if !left_out.is_empty() {
                    let pieces: Vec<usize> = left_out.iter().map(|left| left.piece).collect();
                    if app.select_refused_controls(runtime_id, &control_ids, &pieces) > 0 {
                        let problems: Vec<Problem> = left_out.iter().flat_map(|left| left.problems.iter().cloned()).collect();
                        app.ring_placed_controls(&control_ids, &problems);
                    }
                }
            }
            Err(error) => {
                let mut message = format!("{error:#}");
                if let Some(misshapen) = error.downcast_ref::<MisshapenControls>() {
                    let selected = app.select_refused_controls(runtime_id, &control_ids, &misshapen.refused);
                    if selected > 0 {
                        message.push('\n');
                        message.push_str(&tr!("cmd-reference-surface-count-refused-strings-selected", count = selected.to_string()));
                        let hidden = app.hide_other_controls(&control_ids, &misshapen.refused);
                        if hidden > 0 {
                            message.push('\n');
                            message.push_str(&tr!("cmd-reference-surface-count-other-strings-hidden", count = hidden.to_string()));
                        }
                        app.frame_selected_strings();
                        app.ring_refused_controls(&control_ids, &misshapen.refused, &misshapen.positions);
                    }
                } else if let Some(placed) = error.downcast_ref::<PlacedRefusal>() {
                    // Reported after the build's own text, which stands as
                    // it was; then treated as a shape refusal is.
                    message.push('\n');
                    message.push_str(&placed_report(&placed.problems, &placed.odd_ones));
                    let involved = placed_strings(&placed.problems);
                    let selected = app.select_refused_controls(runtime_id, &control_ids, &involved);
                    if selected > 0 {
                        message.push('\n');
                        message.push_str(&tr!("cmd-reference-surface-count-refused-strings-selected", count = selected.to_string()));
                        let hidden = app.hide_other_controls(&control_ids, &involved);
                        if hidden > 0 {
                            message.push('\n');
                            message.push_str(&tr!("cmd-reference-surface-count-other-strings-hidden", count = hidden.to_string()));
                        }
                        app.frame_selected_strings();
                        app.ring_placed_controls(&control_ids, &placed.problems);
                    }
                }
                crate::userspace_error!("{}", tr!("cmd-reference-surface-build-surface-failed-error", error = message))
            }
        };
        self.spawn_job_reporting_progress(tr!("cmd-reference-surface-building-surface"), vec![project_key], compute, apply);
        Ok(())
    }

    /// Replace the selection with the controls a build refused or left out,
    /// so they light up for the geologist to fix by hand, and say how
    /// many that is. Nothing changes when the build's project is no longer
    /// the active one or none of the strings can still be selected.
    fn select_refused_controls(&mut self, runtime_id: u32, ids: &[ObjectId], refused: &[usize]) -> usize {
        if self.workspace.active_project().is_none_or(|project| project.runtime_id != runtime_id) {
            return 0;
        }
        let handles = refused_selection(ids, refused, |handle| {
            matches!(handle, SceneEntityId::Object(id) if self.scene_document.get_object(id).is_some())
                && !self.editor.hidden_handles.contains(&handle)
                && !self.editor.frozen_handles.contains(&handle)
        });
        if handles.is_empty() {
            return 0;
        }
        if self.has_pending_move_delta() {
            self.cancel_move_delta();
        }
        self.editor.selected_handles = handles.into_iter().collect();
        self.editor.selected_drill_holes.clear();
        self.editor.selected_tie_ins.clear();
        self.invalidate_overlay();
        self.editor.selected_handles.len()
    }

    /// Hide every other control the build took, as Hide Selection hides,
    /// in one undo step, so the refused ones stand alone in the scene; Unhide
    /// All or one undo brings them back. Returns how many were hidden.
    fn hide_other_controls(&mut self, ids: &[ObjectId], refused: &[usize]) -> usize {
        let Some(document) = self.workspace.active_document() else {
            return 0;
        };
        let hides = isolating_hides(ids, refused, |id| document.get_object(id).is_some() && !document.is_object_hidden(id));
        if hides.is_empty() {
            return 0;
        }
        let count = hides.len();
        self.execute_edit(Command::Batch(
            hides.into_iter().map(|id| Command::SetObjectHidden { id, before: false, after: true }).collect(),
        ));
        self.invalidate_geometry();
        count
    }

    /// Ring every position where a refused control goes wrong, each one,
    /// past the report's line limit too, until the next build, Clear rings or
    /// the project is left. Drawn on the canvas only: nothing enters the
    /// project or its undo history.
    fn ring_refused_controls(&mut self, ids: &[ObjectId], refused: &[usize], positions: &[DVec3]) {
        let document = self.workspace.active_document();
        self.editor.string_rings = refused_rings(ids, refused, positions, |id, at| match document.and_then(|document| document.get_object(id)) {
            Some(Object::Polyline { verts, .. }) => crate::app::commands::string_clean::vertex_at(verts, at),
            _ => None,
        });
        self.number_rings_by_selection(ids);
        forget_ring_menu(&mut self.editor);
        self.redraw_requested = true;
    }

    /// Ring every place Clean Strings' final check found in the refused
    /// controls, as [`Self::ring_refused_controls`] rings, each ring naming
    /// the drawn vertex of each string at its position when there is one.
    fn ring_placed_controls(&mut self, ids: &[ObjectId], problems: &[Problem]) {
        let document = self.workspace.active_document();
        self.editor.string_rings = placed_rings(ids, problems, |id, at| match document.and_then(|document| document.get_object(id)) {
            Some(Object::Polyline { verts, .. }) => crate::app::commands::string_clean::vertex_at(verts, at),
            _ => None,
        });
        self.number_rings_by_selection(ids);
        forget_ring_menu(&mut self.editor);
        self.redraw_requested = true;
    }

    /// The build names its strings by their place in the selection, so a
    /// Join on its rings goes by those numbers too.
    fn number_rings_by_selection(&mut self, ids: &[ObjectId]) {
        self.editor.string_numbers = ids.iter().enumerate().map(|(place, &id)| (id, place)).collect();
    }

    /// Move the camera so the selected strings fill the view, keeping its
    /// angle, as Zoom Extents does for the whole scene.
    fn frame_selected_strings(&mut self) {
        let document = &self.scene_document;
        let bounds = strings_bounds(self.editor.selected_handles.iter().filter_map(|handle| match handle {
            SceneEntityId::Object(id) => match document.get_object(*id) {
                Some(Object::Polyline { verts, closed, .. }) => Some((verts.as_slice(), *closed)),
                _ => None,
            },
            _ => None,
        }));
        if let (Some((min, max)), Some(graphics)) = (bounds, self.graphics.as_mut()) {
            graphics.zoom_to_bounds(min, max);
            self.redraw_requested = true;
        }
    }
}

/// The controls to hide so the refused ones stand alone: every other string
/// the build took, each once, where `hideable` allows; a string refused at
/// any place in the selection is never among them.
fn isolating_hides(ids: &[ObjectId], refused: &[usize], hideable: impl Fn(ObjectId) -> bool) -> Vec<ObjectId> {
    let refused: HashSet<ObjectId> = refused.iter().filter_map(|&index| ids.get(index).copied()).collect();
    let mut seen = HashSet::new();
    ids.iter().copied().filter(|&id| !refused.contains(&id) && seen.insert(id) && hideable(id)).collect()
}

/// The box around the strings, arcs included, as the scene bounds measure
/// each string; `None` when there are none.
fn strings_bounds<'a>(strings: impl IntoIterator<Item = (&'a [crate::model::PolyVertex], bool)>) -> Option<(DVec3, DVec3)> {
    strings
        .into_iter()
        .filter_map(|(verts, closed)| crate::model::geometry::polyline_bulge_bounds(verts, closed))
        .reduce(|(min, max), (other_min, other_max)| (min.min(other_min), max.max(other_max)))
}

fn too_few_points(count: usize) -> String {
    tr!(
        "cmd-reference-surface-count-point-s-selected-surface",
        count = count.to_string(),
        minimum = MINIMUM_POINTS.to_string()
    )
}

fn control_not_finite(index: usize) -> String {
    tr!("cmd-reference-surface-control-string-index-has-non", index = (index + 1).to_string())
}

fn control_not_available(index: usize) -> String {
    tr!("cmd-reference-surface-control-string-index-no-longer", index = (index + 1).to_string())
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
        // A string of one point is the build's to leave out and name; one
        // of none has nowhere to be named.
        if string.is_empty() {
            anyhow::bail!("{}", too_few_control_vertices(index, string.len()));
        }
        strings.push(string);
    }
    Ok(strings)
}

/// Worker half: the spline through the points and controls, gridded, cut
/// to the extent or the points' outline, and reported with its run record.
#[allow(clippy::too_many_arguments)]
fn grid_surface_from_points(
    points: Vec<DVec3>,
    controls: SelectedControls,
    extent: Option<Vec<DVec2>>,
    settings: &ModellingSettings,
    name: String,
    stamp: &RunStamp,
    cancel: &CancelFlag,
    progress: &Progress,
) -> Result<(crate::model::triangulation::GeneratedTriangulation, Vec<LeftOut>)> {
    let surface = surface_mesh(&points, controls, extent.as_deref(), settings, cancel, progress)?;
    // What the build cleaned in its copy and the strings it left out come
    // first, each under its own heading, so the build line that follows
    // reads as what was built from them.
    if !surface.cleaned.is_empty() {
        userspace_log!("{}", cleaned_report(&surface.cleaned));
    }
    if !surface.left_out_strings.is_empty() {
        userspace_warn!("{}", left_out_report(&surface.left_out_strings));
    }
    if let Some(thinning) = surface.thinned {
        let line = thinned_report(&thinning, surface.used);
        if thinning.tolerance > SHAPE_TOLERANCES[0] {
            userspace_warn!("{}", line);
        } else {
            userspace_log!("{}", line);
        }
    }
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
    let generated = session::build_generated_triangulation(name, surface.vertices, surface.faces, TriSurfaceType::Surface, crate::model::triangulation::unique_edges)?;
    Ok((generated, surface.left_out_strings))
}

/// The line saying a build thinned its copy of the control strings, and
/// whether the shape tolerance had to be raised; `used` is the points the
/// spline was fitted through.
fn thinned_report(thinning: &Thinning, used: usize) -> String {
    let raised = if thinning.tolerance > SHAPE_TOLERANCES[0] {
        format!(" {}", tr!("cmd-reference-surface-thinned-raised", first = SHAPE_TOLERANCES[0].to_string()))
    } else {
        String::new()
    };
    tr!(
        "cmd-reference-surface-thinned",
        kept = thinning.kept.to_string(),
        tolerance = thinning.tolerance.to_string(),
        raised = raised,
        spacing = thinning.spacing.to_string(),
        used = used.to_string(),
        budget = POINT_BUDGET.to_string()
    )
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
/// over is left out, the string winning. When those points and the picks
/// would pass the spline's point budget, the controls enter as their
/// thinned copy instead, as [`within_budget`] makes it; their checks, the
/// picks they leave out and the outline are still read off them unthinned.
/// The controls are first cleaned in a copy, as Clean Strings and Join all
/// at halfway would clean them; the strings of the copy the build's checks
/// still refuse together are left out as [`string_clean::leave_out`]
/// chooses. No string selected changes.
fn surface_mesh(
    points: &[DVec3],
    selected: SelectedControls,
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
    let copy = string_clean::clean_copy(selected.strings, selected.layers, selected.keys, &cancelled).map_err(|_| anyhow::anyhow!("{}", tr!("common-cancelled")))?;
    let strings: Vec<Vec<DVec3>> = copy
        .pieces
        .iter()
        .map(|piece| {
            let mut string = piece.verts.clone();
            string.dedup();
            string
        })
        .collect();
    let sources: Vec<usize> = copy.pieces.iter().map(|piece| piece.source).collect();
    let keys: Vec<u64> = sources.iter().map(|&source| selected.keys.get(source).copied().unwrap_or(source as u64)).collect();
    let copied = CopiedControls {
        strings: &strings,
        sources: &sources,
        keys: &keys,
    };
    let checked = checked_controls(copied, &cancelled)?;
    let (names, controls, crossings, entered) = (&checked.names, checked.ordered.as_slice(), &checked.crossings, &checked.entered);
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
        refuse_controls_along(ring, controls, names, &cancelled)?;
        support = points.iter().filter(|point| !inside(bands, point.truncate())).count();
        let covered = points.len() - support;
        if covered < MINIMUM_POINTS {
            anyhow::bail!("{}", too_few_points_inside(covered));
        }
    }

    let (picks, overridden, left_out) = picks_off_controls(points, controls, &cancelled)?;
    let budgeted = if !controls.is_empty() && picks.len() + entered.len() > POINT_BUDGET {
        Some(within_budget(controls, names, crossings, picks.len(), &cancelled)?)
    } else {
        None
    };
    let fit_controls = budgeted.as_ref().map_or(entered.as_slice(), |(thinned, _)| thinned.as_slice());
    let mut fitted = Vec::new();
    fitted
        .try_reserve_exact(picks.len() + fit_controls.len())
        .context("Not enough memory for the surface points")?;
    fitted.extend(picks);
    fitted.extend(fit_controls.iter().copied());

    // Without a mask the points' outline is the extent, cut exactly as a
    // drawn one is. Every fitted point is the buffer inside it, so the
    // refusals above that guard a drawn mask have nothing to find in it.
    // A thinned copy's points lie on chords of the strings, inside the
    // outline of the strings unthinned, which is the one taken.
    let outline = match (extent, &budgeted) {
        (Some(_), _) => Vec::new(),
        (None, None) => buffered_outline(&fitted)?,
        (None, Some(_)) => {
            let picked = fitted.len() - fit_controls.len();
            let mut whole = Vec::new();
            whole.try_reserve_exact(picked + entered.len()).context("Not enough memory for the outline")?;
            whole.extend_from_slice(&fitted[..picked]);
            whole.extend(entered.iter().copied());
            buffered_outline(&whole)?
        }
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
        control_points: fit_controls.len(),
        thinned: budgeted.as_ref().map(|(_, thinning)| *thinning),
        crossings: crossings.len(),
        overridden,
        left_out,
        steep,
        settings: *settings,
        left_out_strings: checked.left_out,
        cleaned: copy.changes,
    })
}

/// The points the controls enter the fit as when, densified at the grid
/// spacing, they and the `picks` kept would pass the spline's point budget.
/// Each string keeps only the vertices that shape it within the first of
/// [`SHAPE_TOLERANCES`] at which those, with where the strings cross, fit
/// beside the picks; points are then put along the thinned strings at the
/// smallest spacing that fits, in whole metres from the grid spacing up,
/// one spacing for the whole build. Refused, naming the counts, when even
/// the vertices kept at the last tolerance do not fit.
fn within_budget(controls: &[Vec<DVec3>], names: &[usize], crossings: &[Crossing], picks: usize, cancelled: &dyn Fn() -> bool) -> Result<(Vec<DVec3>, Thinning)> {
    let room = POINT_BUDGET.saturating_sub(picks);
    let mut fewest = 0;
    for &tolerance in &SHAPE_TOLERANCES {
        stop_if_cancelled(cancelled)?;
        let thinned = thinned_controls(controls, crossings, tolerance);
        fewest = control_points_spaced(&thinned, names, crossings, f64::INFINITY, cancelled)?.len();
        if fewest > room {
            continue;
        }
        let spaced = |extra: f64| control_points_spaced(&thinned, names, crossings, DEFAULT_SPACING + extra, cancelled);
        // At a spacing as long as the longest segment every segment is one
        // piece, so the points are the vertices kept, which fit.
        let longest = thinned
            .iter()
            .flat_map(|string| string.windows(2))
            .map(|segment| segment[0].truncate().distance(segment[1].truncate()))
            .fold(0.0, f64::max);
        let mut fits = 0.0;
        if spaced(0.0)?.len() > room {
            let (mut short, mut long) = (0.0, (longest - DEFAULT_SPACING).ceil().max(1.0));
            while long - short > 1.0 {
                let middle = ((short + long) / 2.0).floor();
                if spaced(middle)?.len() > room {
                    short = middle;
                } else {
                    long = middle;
                }
            }
            fits = long;
        }
        let points = spaced(fits)?;
        let thinning = Thinning {
            kept: fewest,
            tolerance,
            spacing: DEFAULT_SPACING + fits,
        };
        return Ok((points, thinning));
    }
    anyhow::bail!(
        "{}",
        tr!(
            "cmd-reference-surface-thin-refused",
            budget = POINT_BUDGET.to_string(),
            tolerance = SHAPE_TOLERANCES[SHAPE_TOLERANCES.len() - 1].to_string(),
            kept = fewest.to_string(),
            picks = picks.to_string(),
            total = (fewest + picks).to_string()
        )
    )
}

/// Each control with only the vertices that shape it within `tolerance`:
/// a vertex is kept when the string without it would pass more than
/// `tolerance` from it, in plan or in height. Its ends and every place it
/// meets another string are always kept, the place put in as a vertex at
/// the string's own height where it has none, so the strings still meet
/// where and at the heights they did.
fn thinned_controls(controls: &[Vec<DVec3>], crossings: &[Crossing], tolerance: f64) -> Vec<Vec<DVec3>> {
    let mut held: Vec<Vec<bool>> = controls.iter().map(|control| vec![false; control.len()]).collect();
    let mut put_in: Vec<Vec<(usize, f64, DVec3)>> = vec![Vec::new(); controls.len()];
    for crossing in crossings {
        for side in &crossing.sides {
            let control = &controls[side.control];
            let (start, end) = (control[side.segment], control[side.segment + 1]);
            match nearer_end(crossing.position, start.truncate(), end.truncate()) {
                Some(at) => held[side.control][side.segment + at] = true,
                None => {
                    let (_, along) = kernel::project_onto_segment(crossing.position, start.truncate(), end.truncate());
                    put_in[side.control].push((side.segment, along, crossing.position.extend(elevation_along(start, end, along))));
                }
            }
        }
    }
    controls
        .iter()
        .zip(held)
        .zip(put_in)
        .map(|((control, held), mut put_in)| {
            put_in.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.total_cmp(&right.1)));
            let mut string = Vec::with_capacity(control.len() + put_in.len());
            let mut pinned = Vec::with_capacity(control.len() + put_in.len());
            let mut extra = put_in.into_iter().peekable();
            for (index, &vertex) in control.iter().enumerate() {
                string.push(vertex);
                pinned.push(held[index] || index == 0 || index + 1 == control.len());
                while let Some((_, _, at)) = extra.next_if(|&(segment, _, _)| segment == index) {
                    if string.last() != Some(&at) {
                        string.push(at);
                        pinned.push(true);
                    }
                }
            }
            shaping_vertices(&string, &pinned, tolerance)
        })
        .collect()
}

/// The vertices of a string that shape it within `tolerance`, by
/// Douglas-Peucker between the vertices `pinned`, which are all kept: of
/// the vertices between two kept ones, the one farthest off the line
/// joining them is kept when it is more than `tolerance` off it, and each
/// side is judged again. How far off is the larger of the plan distance to
/// the line and the height off it where the line comes nearest in plan.
fn shaping_vertices(string: &[DVec3], pinned: &[bool], tolerance: f64) -> Vec<DVec3> {
    let mut keep = pinned.to_vec();
    let anchors: Vec<usize> = (0..string.len()).filter(|&index| keep[index]).collect();
    let mut spans: Vec<(usize, usize)> = anchors.windows(2).map(|pair| (pair[0], pair[1])).collect();
    while let Some((first, last)) = spans.pop() {
        let (start, end) = (string[first], string[last]);
        let mut farthest: Option<(usize, f64)> = None;
        for (index, &vertex) in string.iter().enumerate().take(last).skip(first + 1) {
            let (nearest, along) = kernel::project_onto_segment(vertex.truncate(), start.truncate(), end.truncate());
            let off = nearest.distance(vertex.truncate()).max((vertex.z - elevation_along(start, end, along)).abs());
            if off > tolerance && farthest.is_none_or(|(_, most)| off > most) {
                farthest = Some((index, off));
            }
        }
        if let Some((index, _)) = farthest {
            keep[index] = true;
            spans.push((first, index));
            spans.push((index, last));
        }
    }
    string.iter().zip(keep).filter(|(_, kept)| *kept).map(|(vertex, _)| *vertex).collect()
}

/// The control strings a build was given: in selection order, each with
/// the key that breaks the last tie in choosing what to leave out (its
/// object id) and its layer.
#[derive(Clone, Copy)]
struct SelectedControls<'a> {
    strings: &'a [Vec<DVec3>],
    keys: &'a [u64],
    layers: &'a [u64],
}

/// The build's working copy of its control strings, each with the place in
/// the selection of the string it came from, by which every message names
/// it, and its key.
#[derive(Clone, Copy)]
struct CopiedControls<'a> {
    strings: &'a [Vec<DVec3>],
    sources: &'a [usize],
    keys: &'a [u64],
}

impl CopiedControls<'_> {
    /// A problem with each string named by its place in the selection.
    fn named(&self, mut problem: Problem) -> Problem {
        for side in &mut problem.sides {
            side.piece = self.sources[side.piece];
        }
        problem
    }
}

/// The control strings one build uses, past the build's own checks: their
/// places in the selection in the order the build takes them, the strings
/// in that order, where they cross, the points they enter the fit as, and
/// the strings left out so the rest pass.
struct CheckedControls {
    names: Vec<usize>,
    ordered: Vec<Vec<DVec3>>,
    crossings: Vec<Crossing>,
    entered: Vec<DVec3>,
    left_out: Vec<LeftOut>,
}

/// The build's own checks on the strings of the copy at the places
/// `kept`, run as the build has always run them: in an order read off
/// their geometry, each named by the place in the selection of the string
/// it came from.
fn run_control_checks(copied: CopiedControls, kept: &[usize], cancelled: &dyn Fn() -> bool) -> Result<CheckedControls> {
    let subset: Vec<Vec<DVec3>> = kept.iter().map(|&index| copied.strings[index].clone()).collect();
    let order: Vec<usize> = canonical_order(&subset).into_iter().map(|index| kept[index]).collect();
    let names: Vec<usize> = order.iter().map(|&index| copied.sources[index]).collect();
    let ordered: Vec<Vec<DVec3>> = order.iter().map(|&index| copied.strings[index].clone()).collect();
    validate_controls(&ordered, &names, cancelled)?;
    let crossings = control_crossings(&ordered, &names, cancelled)?;
    let entered = control_points(&ordered, &names, &crossings, cancelled)?;
    Ok(CheckedControls {
        names,
        ordered,
        crossings,
        entered,
        left_out: Vec::new(),
    })
}

/// The copy's strings past the build's checks: all of them when they pass,
/// else the rest once the strings that still clash, or are misshapen, are
/// left out of this build, each named by the place in the selection of the
/// string it came from. When none can be left out, or none would be left,
/// the refusal of all of them comes back as it always did, placed, with
/// why when none would be left.
fn checked_controls(copied: CopiedControls, cancelled: &dyn Fn() -> bool) -> Result<CheckedControls> {
    let all: Vec<usize> = (0..copied.strings.len()).collect();
    let refusal = match run_control_checks(copied, &all, cancelled) {
        Ok(checked) => return Ok(checked),
        Err(refusal) => refusal,
    };
    if cancelled() {
        return Err(refusal);
    }
    let Ok(problems) = string_clean::problems(copied.strings, cancelled) else {
        return Err(refusal);
    };
    match string_clean::leave_out(copied.strings, &problems, copied.keys, cancelled) {
        Ok(left_out) if !left_out.is_empty() => {
            // A string the clean cut into pieces is left out whole when any
            // piece of it is chosen, so what is named, selected and ringed
            // is exactly what the build left out. Leaving more out never
            // makes the rest fail the checks.
            let left_out = whole_strings_left_out(left_out, copied);
            let out: HashSet<usize> = left_out.iter().map(|left| left.piece).collect();
            let kept: Vec<usize> = all.into_iter().filter(|&index| !out.contains(&copied.sources[index])).collect();
            if kept.is_empty() {
                return Err(with_reason(placed(refusal, copied, cancelled), &tr!("cmd-reference-surface-left-out-none-left")));
            }
            let checked = run_control_checks(copied, &kept, cancelled).map_err(|error| placed(error, copied, cancelled))?;
            Ok(CheckedControls { left_out, ..checked })
        }
        Err(NotLeftOut::NoneLeft) => Err(with_reason(placed(refusal, copied, cancelled), &tr!("cmd-reference-surface-left-out-none-left"))),
        Ok(_) | Err(NotLeftOut::Unnamed | NotLeftOut::Cancelled) => Err(placed(refusal, copied, cancelled)),
    }
}

/// The pieces of the copy left out, as the strings they came from: one
/// entry per string, by its place in the selection, in the order a piece of
/// it was first chosen, with the places of all its pieces chosen, each once.
fn whole_strings_left_out(left_out: Vec<LeftOut>, copied: CopiedControls) -> Vec<LeftOut> {
    let mut strings: Vec<LeftOut> = Vec::new();
    let mut entry: HashMap<usize, usize> = HashMap::new();
    for left in left_out {
        let source = copied.sources[left.piece];
        let at = *entry.entry(source).or_insert_with(|| {
            strings.push(LeftOut {
                piece: source,
                problems: Vec::new(),
            });
            strings.len() - 1
        });
        for problem in left.problems.into_iter().map(|problem| copied.named(problem)) {
            if !strings[at].problems.contains(&problem) {
                strings[at].problems.push(problem);
            }
        }
    }
    strings
}

/// A refusal with a line saying why after its own text, keeping its type
/// so the strings it names are still selected and ringed.
fn with_reason(error: anyhow::Error, reason: &str) -> anyhow::Error {
    match error.downcast::<MisshapenControls>() {
        Ok(mut misshapen) => {
            misshapen.report.push('\n');
            misshapen.report.push_str(reason);
            misshapen.into()
        }
        Err(error) => match error.downcast::<PlacedRefusal>() {
            Ok(mut placed) => {
                placed.report.push('\n');
                placed.report.push_str(reason);
                placed.into()
            }
            Err(error) => anyhow::anyhow!("{error:#}\n{reason}"),
        },
    }
}

/// The places of a line naming a string left out: "(x, y)", "(x, y) and
/// (x, y)", up to [`LEFT_OUT_PLACES`] and then a count of the rest.
fn left_out_places(problems: &[&Problem]) -> String {
    plan_places(&problems.iter().map(|problem| problem.at).collect::<Vec<_>>())
}

/// Plan positions as a line gives them, up to [`LEFT_OUT_PLACES`] and
/// then a count of the rest.
fn plan_places(at: &[DVec3]) -> String {
    let places: Vec<String> = at.iter().map(|at| format!("({:.3}, {:.3})", at.x, at.y)).collect();
    if places.len() <= LEFT_OUT_PLACES {
        return crate::app::commands::string_clean::word_list(&places);
    }
    let mut shown = places[..LEFT_OUT_PLACES].join(", ");
    shown.push_str(&tr!("cmd-reference-surface-and-more", more = (places.len() - LEFT_OUT_PLACES).to_string()));
    shown
}

/// "string 14", "strings 14 and 22": the other strings of a left-out
/// string's clashes, numbered from one as selected, each once.
fn left_out_others(piece: usize, problems: &[&Problem]) -> String {
    let mut others: Vec<usize> = problems
        .iter()
        .flat_map(|problem| problem.sides.iter().map(|side| side.piece))
        .filter(|&other| other != piece)
        .map(|other| other + 1)
        .collect();
    others.sort_unstable();
    others.dedup();
    match others.as_slice() {
        [only] => tr!("cmd-reference-surface-left-out-other-string", string = only.to_string()),
        _ => tr!(
            "cmd-reference-surface-left-out-other-strings",
            strings = crate::app::commands::string_clean::word_list(&others.iter().map(usize::to_string).collect::<Vec<_>>())
        ),
    }
}

/// The lines naming one string left out of the build, numbered from one as
/// selected: one per fault of its own shape, else one for the strings it
/// sits above or below with the miss, and one for those it runs along.
fn left_out_lines(left: &LeftOut) -> Vec<String> {
    let string = (left.piece + 1).to_string();
    let mut lines = Vec::new();
    let mut sided: Vec<&Problem> = Vec::new();
    let mut along: Vec<&Problem> = Vec::new();
    let (mut below, mut above) = (false, false);
    let (mut low, mut high) = (f64::INFINITY, 0.0f64);
    for problem in &left.problems {
        let (x, y) = (format!("{:.3}", problem.at.x), format!("{:.3}", problem.at.y));
        match &problem.kind {
            ProblemKind::TooShort => lines.push(tr!("cmd-reference-surface-left-out-too-short", string = string.clone(), x = x, y = y)),
            ProblemKind::EndsWhereItStarts => lines.push(tr!("cmd-reference-surface-left-out-ends-where-it-starts", string = string.clone(), x = x, y = y)),
            ProblemKind::TurnsBack => lines.push(tr!("cmd-reference-surface-left-out-turns-back", string = string.clone(), x = x, y = y)),
            ProblemKind::CrossesItself => lines.push(tr!("cmd-reference-surface-left-out-crosses-itself", string = string.clone(), x = x, y = y)),
            ProblemKind::PointsDisagree { miss } => lines.push(tr!(
                "cmd-reference-surface-left-out-points-disagree",
                string = string.clone(),
                miss = format!("{miss:.2}"),
                x = x,
                y = y
            )),
            ProblemKind::Along => along.push(problem),
            ProblemKind::Crossing { .. } | ProblemKind::NearMiss { .. } => {
                let own = problem.sides.iter().find(|side| side.piece == left.piece).and_then(|side| side.height);
                let other = problem.sides.iter().find(|side| side.piece != left.piece).and_then(|side| side.height);
                if let (Some(own), Some(other)) = (own, other) {
                    below |= own < other;
                    above |= own > other;
                    low = low.min((own - other).abs());
                    high = high.max((own - other).abs());
                }
                sided.push(problem);
            }
            ProblemKind::BuildRefuses(_) => {}
        }
    }
    if !sided.is_empty() {
        let (low, high) = (format!("{:.2}", if low.is_finite() { low } else { 0.0 }), format!("{high:.2}"));
        let amount = if low == high {
            low
        } else {
            tr!("cmd-reference-surface-left-out-range", low = low, high = high)
        };
        let others = left_out_others(left.piece, &sided);
        let places = left_out_places(&sided);
        lines.push(match (below, above) {
            (true, false) => tr!(
                "cmd-reference-surface-left-out-below",
                string = string.clone(),
                amount = amount,
                others = others,
                places = places
            ),
            (false, true) => tr!(
                "cmd-reference-surface-left-out-above",
                string = string.clone(),
                amount = amount,
                others = others,
                places = places
            ),
            _ => tr!(
                "cmd-reference-surface-left-out-above-and-below",
                string = string.clone(),
                amount = amount,
                others = others,
                places = places
            ),
        });
    }
    if !along.is_empty() {
        lines.push(tr!(
            "cmd-reference-surface-left-out-along",
            string = string,
            others = left_out_others(left.piece, &along),
            places = left_out_places(&along)
        ));
    }
    lines
}

/// What a build cleaned in its copy of the controls: a heading, then one
/// line per kind of change with how many places and where, the joins split
/// by whether Clean Strings or Join all at halfway would have made them. A
/// place where a shared vertex was put in and then joined is counted as
/// joined only.
fn cleaned_report(changes: &[string_clean::Change]) -> String {
    use string_clean::{JOIN_AUTOMATIC, JOIN_ON_REQUEST};
    let places = cleaned_places(changes);
    let mut report = tr!("cmd-reference-surface-cleaned-heading");
    for (slot, at) in places.iter().enumerate().filter(|(_, at)| !at.is_empty()) {
        let (count, at) = (at.len().to_string(), plan_places(at));
        let line = match slot {
            0 => tr!("cmd-reference-surface-cleaned-repeats", count = count, places = at),
            1 => tr!("cmd-reference-surface-cleaned-spikes", count = count, places = at),
            2 => tr!("cmd-reference-surface-cleaned-retraces", count = count, places = at),
            3 => tr!("cmd-reference-surface-cleaned-loops", count = count, places = at),
            4 => tr!("cmd-reference-surface-cleaned-zeros", count = count, places = at),
            5 => tr!("cmd-reference-surface-cleaned-heights", count = count, places = at),
            6 => tr!("cmd-reference-surface-cleaned-shared-cut", count = count, places = at),
            7 => tr!("cmd-reference-surface-cleaned-removed", count = count, places = at),
            8 => tr!("cmd-reference-surface-cleaned-joined-small", count = count, limit = JOIN_AUTOMATIC.to_string(), places = at),
            9 => tr!(
                "cmd-reference-surface-cleaned-joined-on-request",
                count = count,
                low = JOIN_AUTOMATIC.to_string(),
                high = JOIN_ON_REQUEST.to_string(),
                places = at
            ),
            _ => tr!(
                "cmd-reference-surface-cleaned-vertex-shared",
                count = count,
                limit = JOIN_ON_REQUEST.to_string(),
                places = at
            ),
        };
        report.push('\n');
        report.push_str(&line);
    }
    report
}

/// The places of [`cleaned_report`], one slot per line in pipeline order,
/// each place once: a place where a shared vertex was put in and then
/// joined is counted as joined only.
fn cleaned_places(changes: &[string_clean::Change]) -> [Vec<DVec3>; 11] {
    use string_clean::{ChangeKind, JOIN_AUTOMATIC};
    let mut joined = PlanCells::default();
    for change in changes.iter().filter(|change| matches!(change.kind, ChangeKind::Joined { .. })) {
        joined.add(change.at, 0);
    }
    // One slot per line, in pipeline order; the places of each, each once,
    // filed by plan position so a place seen is found without a scan.
    let mut places: [Vec<DVec3>; 11] = Default::default();
    let mut filed: [PlanCells; 11] = Default::default();
    for change in changes {
        let slot = match &change.kind {
            ChangeKind::RepeatsMerged { .. } => 0,
            ChangeKind::SpikeDropped => 1,
            ChangeKind::RetraceDropped { .. } => 2,
            ChangeKind::LoopCut { .. } => 3,
            ChangeKind::ZeroDropped => 4,
            ChangeKind::HeightDropped { .. } => 5,
            ChangeKind::SharedCut { .. } => 6,
            ChangeKind::Removed { .. } => 7,
            ChangeKind::Joined { miss, .. } if *miss <= JOIN_AUTOMATIC => 8,
            ChangeKind::Joined { .. } => 9,
            ChangeKind::VertexShared { .. } if joined.first_near(change.at.truncate()).is_some() => continue,
            ChangeKind::VertexShared { .. } => 10,
        };
        if filed[slot].first_near(change.at.truncate()).is_none() {
            filed[slot].add(change.at, 0);
            places[slot].push(change.at);
        }
    }
    places
}

/// The warning for strings left out of a build: a line counting them, then
/// every one with where and by how much, up to [`SELF_SHAPE_LINES`] lines
/// and then a count of the rest.
fn left_out_report(left_out: &[LeftOut]) -> String {
    let lines: Vec<String> = left_out.iter().flat_map(left_out_lines).collect();
    let mut report = tr!("cmd-reference-surface-left-out-count", count = left_out.len().to_string());
    for line in lines.iter().take(SELF_SHAPE_LINES) {
        report.push('\n');
        report.push_str(line);
    }
    if lines.len() > SELF_SHAPE_LINES {
        report.push_str(&tr!("cmd-reference-surface-and-more", more = (lines.len() - SELF_SHAPE_LINES).to_string()));
    }
    report
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

/// A refusal of the control strings Clean Strings' final check can place:
/// the build's own text, word for word, and why when leaving strings out
/// would leave none, then every place the check finds in the build's copy
/// of the strings and the strings sitting on one side of every string they
/// miss by more than the join range, each string by its place in the
/// selection counting from zero.
#[derive(Debug)]
struct PlacedRefusal {
    report: String,
    problems: Vec<Problem>,
    odd_ones: Vec<OddOneOut>,
}

impl std::fmt::Display for PlacedRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.report)
    }
}

impl std::error::Error for PlacedRefusal {}

/// The build's refusal of its controls, carrying every place Clean Strings'
/// final check finds in the copy's strings, each named by its place in the
/// selection, when it finds one it can place. A shape
/// refusal, which places its strings itself, a cancelled build and a
/// refusal the check cannot place come back as they went in.
fn placed(error: anyhow::Error, copied: CopiedControls, cancelled: &dyn Fn() -> bool) -> anyhow::Error {
    if cancelled() || error.is::<MisshapenControls>() {
        return error;
    }
    let Ok(problems) = string_clean::problems(copied.strings, cancelled) else {
        return error;
    };
    if !problems.iter().any(|problem| problem.at.is_finite()) {
        return error;
    }
    let Ok(odd_ones) = string_clean::odd_ones_out(copied.strings, &problems, cancelled) else {
        return error;
    };
    let odd_ones = odd_ones
        .into_iter()
        .map(|odd| OddOneOut {
            piece: copied.sources[odd.piece],
            ..odd
        })
        .collect();
    PlacedRefusal {
        report: format!("{error:#}"),
        problems: problems.into_iter().map(|problem| copied.named(problem)).collect(),
        odd_ones,
    }
    .into()
}

/// The lines added under a placed refusal: how many places are ringed, each
/// problem biggest miss first, then each string on one side of every string
/// it misses, both up to [`SELF_SHAPE_LINES`] and then a count of the rest.
/// Strings are numbered as the build numbers them, by place in the
/// selection.
fn placed_report(problems: &[Problem], odd_ones: &[OddOneOut]) -> String {
    use crate::app::commands::string_clean::{odd_one_out_line, problem_line_named};
    let capped = |mut lines: Vec<String>, all: usize| {
        if all > SELF_SHAPE_LINES
            && let Some(last) = lines.last_mut()
        {
            last.push_str(&tr!("cmd-reference-surface-and-more", more = (all - SELF_SHAPE_LINES).to_string()));
        }
        lines
    };
    let rings = problems.iter().filter(|problem| problem.at.is_finite()).count();
    let mut lines = vec![tr!("cmd-reference-surface-count-places-stop-build", count = rings.to_string())];
    lines.extend(capped(
        problems.iter().take(SELF_SHAPE_LINES).map(|problem| problem_line_named(problem, &Some)).collect(),
        problems.len(),
    ));
    lines.extend(capped(
        odd_ones.iter().take(SELF_SHAPE_LINES).map(|odd| odd_one_out_line(odd, odd.piece)).collect(),
        odd_ones.len(),
    ));
    lines.join("\n")
}

/// Every string a placed problem concerns, by place in the selection, each
/// once, in order.
fn placed_strings(problems: &[Problem]) -> Vec<usize> {
    let mut strings: Vec<usize> = problems
        .iter()
        .filter(|problem| problem.at.is_finite())
        .flat_map(|problem| problem.sides.iter().map(|side| side.piece))
        .collect();
    strings.sort_unstable();
    strings.dedup();
    strings
}

/// One ring per placed problem, its strings by the ids they were chosen as,
/// each with the drawn vertex at the ring when there is one, titled with
/// the strings' numbers in the selection and the miss.
fn placed_rings(ids: &[ObjectId], problems: &[Problem], vertex_of: impl Fn(ObjectId, DVec3) -> Option<usize>) -> Vec<StringRing> {
    problems
        .iter()
        .filter(|problem| problem.at.is_finite())
        .map(|problem| StringRing {
            at: problem.at,
            kind: StringRingKind::Left(problem.kind.clone()),
            sides: problem
                .sides
                .iter()
                .filter_map(|side| ids.get(side.piece))
                .map(|&id| (id, vertex_of(id, problem.at)))
                .collect(),
            title: crate::app::commands::string_clean::ring_title(problem, &Some),
        })
        .collect()
}

/// The scene entities to select for the refused controls: the ids they were
/// chosen as, by their place in `ids`, kept only where `selectable` allows.
fn refused_selection(ids: &[ObjectId], refused: &[usize], selectable: impl Fn(SceneEntityId) -> bool) -> Vec<SceneEntityId> {
    refused
        .iter()
        .filter_map(|&index| ids.get(index))
        .map(|&id| SceneEntityId::Object(id))
        .filter(|&handle| selectable(handle))
        .collect()
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
            inside ^= kernel::edge_crosses_ray(point, a, b);
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
            .filter(|(a, b)| a != b && kernel::edge_crosses_ray(point, *a, *b))
            .count()
            % 2
            == 1
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
