//! Pit Design: the project's pit designs, the window that edits them, the
//! preview it keeps current in the background, and building a design into
//! its layer and surface.
//!
//! A design is opened on a copy ([`PitDesignDraft`]); [`App::sync_pit_design_draft`]
//! writes the copy into the project once no field is mid-edit, one undo
//! step per settled edit, and takes the project's version back when undo
//! or redo changes it. Building replaces the design's own layer and surface
//! in place, with what it was built from, as one step.

use std::sync::Arc;

use anyhow::{Result, anyhow};
use glam::{DVec2, DVec3};

use crate::{
    app::{
        App,
        jobs::{CancelFlag, Cancelled, JobKey},
    },
    i18n::tr,
    model::{
        Command, FillStyle, Layer, LayerId, Object, ObjectColor, ObjectId, OpenItem, PolyVertex, SceneEntityId, SectionKind,
        formats::mesh_data,
        pit_design::{
            self, Base, Design, StringKind,
            item::{Built, Outputs, PitDesign, PitDesignId, PitDesigns, strings_fingerprint},
            settings::PitDesignSettings,
        },
        spatial::TriangleBvh,
        triangulation::{GeneratedTriangulation, GeometryVersion, TriangulationId},
    },
    ui::{
        dialogs::pit_design::{NoDesign, PitDesignDraft, Preview, PreviewKey, end_text, no_design_text},
        state::{TriSurfaceCutSide, UiPitDesignEntry},
    },
    userspace_log, userspace_warn,
};

/// The colours strings are drawn in, in the preview and on the layer.
pub(crate) fn string_colour(kind: StringKind) -> [f32; 4] {
    match kind {
        StringKind::Toe => [0.30, 0.60, 0.95, 1.0],
        StringKind::Crest => [0.90, 0.35, 0.25, 1.0],
        StringKind::RampEdge => [0.35, 0.80, 0.40, 1.0],
        StringKind::RampCentreline => [0.62, 0.66, 0.62, 1.0],
    }
}

/// A topography's mesh and index, for a worker to read the ground from.
#[derive(Clone)]
struct Ground {
    mesh: Arc<mesh_data::Triangulation>,
    spatial: Arc<TriangleBvh>,
    version: GeometryVersion,
}

impl Ground {
    /// The ground's RL at a point in plan, where it covers it.
    fn rl(&self, point: DVec2) -> Option<f64> {
        let bounds = self.mesh.bounds();
        if point.x < bounds.min.x || point.x > bounds.max.x || point.y < bounds.min.y || point.y > bounds.max.y {
            return None;
        }
        let above = bounds.max.z + ((bounds.max.z - bounds.min.z).abs() * 1.0e-6).max(1.0);
        self.spatial.ray_hit(&self.mesh, point.extend(above), -DVec3::Z).map(|hit| hit.z)
    }
}

/// Build a design on a worker: the string, its settings, and the ground to
/// daylight against.
fn design_on_worker(points: &[DVec3], settings: &PitDesignSettings, ground: Option<&Ground>, cancel: &CancelFlag) -> Result<Preview> {
    let Some(base) = Base::from_points(points) else {
        return Ok(Preview {
            design: Err(NoDesign::NotClosed),
            base_elevation: 0.0,
            base_unevenness: 0.0,
            ramp_start: None,
        });
    };
    let default_start = points.first().map_or(DVec2::ZERO, |point| point.truncate());
    let spec = settings.spec(default_start);
    let ramp_start = spec.ramp.map(|ramp| nearest_on_string(points, ramp.start));
    let sample = |point: DVec2| ground.and_then(|ground| ground.rl(point));
    let sampler: Option<pit_design::Ground<'_>> = ground.map(|_| &sample as pit_design::Ground<'_>);
    let design = pit_design::design(&base, &spec, sampler, &|| cancel.is_cancelled());
    anyhow::ensure!(!cancel.is_cancelled(), Cancelled);
    Ok(Preview {
        design: design.map_err(NoDesign::Settings),
        base_elevation: base.elevation,
        base_unevenness: base.unevenness,
        ramp_start,
    })
}

/// The point of a closed string nearest `point` in plan, at the string's RL
/// there.
fn nearest_on_string(points: &[DVec3], point: DVec2) -> DVec3 {
    let count = points.len();
    (0..count)
        .map(|index| {
            let (a, b) = (points[index], points[(index + 1) % count]);
            let (foot, t) = crate::model::kernel::project_onto_segment(point, a.truncate(), b.truncate());
            (foot.distance(point), foot.extend(a.z + (b.z - a.z) * t))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(point.extend(0.0), |(_, nearest)| nearest)
}

/// The pit's surface from its strings, trimmed to the topography when
/// `trim` holds one.
fn surface_on_worker(design: &Design, trim: Option<&Ground>, cancel: &CancelFlag, progress: &crate::model::progress::Progress) -> Result<(Vec<mesh_data::Vertex>, Vec<[u32; 3]>)> {
    let (rings, edges) = design.breaklines();
    let (vertices, faces) = crate::app::commands::triangulation::creation::cdt_surface_from_breaklines(&rings, &edges)?;
    anyhow::ensure!(!cancel.is_cancelled(), Cancelled);
    let Some(ground) = trim else {
        return Ok((vertices, faces));
    };
    let mesh = mesh_data::Triangulation::from_vertices_and_faces(vertices, faces)?;
    crate::app::commands::triangulation::cuts::clip_mesh_by_surface(&mesh, &ground.mesh, TriSurfaceCutSide::CutTop, &progress.phase(0.2, 1.0))
}

/// The layer objects a design's strings become.
fn string_objects(design: &Design, layer: LayerId, mut allocate: impl FnMut() -> ObjectId) -> Vec<Object> {
    design
        .strings
        .iter()
        .map(|string| Object::Polyline {
            id: allocate(),
            layer,
            verts: string.points.iter().copied().map(PolyVertex::straight).collect(),
            closed: string.closed,
            color: ObjectColor::Fixed(string_colour(string.kind)),
            fill: FillStyle::Clear,
            line_weight: 1.0,
        })
        .collect()
}

/// The fingerprint of the strings on `layer`, in draw order.
fn layer_fingerprint(document: &crate::model::Document, layer: LayerId) -> u64 {
    let strings: Vec<(Vec<DVec3>, bool)> = document
        .objects()
        .iter()
        .filter(|object| object.layer() == layer)
        .filter_map(|object| object.tessellated_path())
        .collect();
    strings_fingerprint(strings.iter().map(|(points, closed)| (points.as_slice(), *closed)))
}

/// What a finished build made, for the step that installs it.
struct BuildResult {
    design: Design,
    surface: Option<GeneratedTriangulation>,
    ground: Option<GeometryVersion>,
}

impl App<'_> {
    /// The closed polyline in the selection, which a design is built from.
    fn selected_closed_string(&self) -> Option<ObjectId> {
        self.editor.selected_handles.iter().find_map(|handle| match handle {
            SceneEntityId::Object(id) => self
                .operand_object(*id)
                .filter(|object| matches!(object, Object::Polyline { .. }) && object.encloses_area())
                .map(|_| *id),
            _ => None,
        })
    }

    /// A closed polyline's points.
    fn closed_string_points(&self, id: ObjectId) -> Option<Vec<DVec3>> {
        self.operand_object(id)
            .and_then(Object::tessellated_path)
            .filter(|(points, closed)| *closed && points.len() >= 3)
            .map(|(points, _)| points)
    }

    fn active_pit_designs(&self) -> PitDesigns {
        self.workspace.active_document().map(|document| document.pit_designs().clone()).unwrap_or_default()
    }

    /// Swap in a new list of designs as one undoable step.
    fn set_pit_designs(&mut self, after: PitDesigns) {
        let before = self.active_pit_designs();
        if before != after {
            self.execute_edit(Command::SetPitDesigns {
                before: Box::new(before),
                after: Box::new(after),
            });
        }
    }

    /// Open Pit Design. On a closed string in the selection, it opens the
    /// design copied from that string, or a new one; otherwise the window
    /// opens on the design it was last on, or the project's first.
    pub(crate) fn open_pit_design(&mut self) {
        self.flush_pit_design_draft();
        if let Some(string) = self.selected_closed_string() {
            let existing = self.active_pit_designs().designs.iter().find(|design| design.base_source == Some(string)).map(|design| design.id);
            match existing {
                Some(id) => self.open_pit_design_item(id),
                None => self.new_pit_design_from(string),
            }
            return;
        }
        if self.editor.pit_design.as_ref().is_some_and(|draft| draft.design.is_some()) {
            self.update_pit_design_preview();
            return;
        }
        match self.active_pit_designs().designs.first().map(|design| design.id) {
            Some(id) => self.open_pit_design_item(id),
            None => {
                self.editor.pit_design = Some(PitDesignDraft::default());
                self.invalidate_overlay();
            }
        }
    }

    /// Open the window on design `id`.
    pub(crate) fn open_pit_design_item(&mut self, id: PitDesignId) {
        self.flush_pit_design_draft();
        let Some(design) = self.workspace.active_document().and_then(|document| document.pit_design(id)).cloned() else {
            return;
        };
        self.cancel_jobs(|key| matches!(key, JobKey::PitDesignPreview(_)));
        self.editor.pit_design = Some(PitDesignDraft::on(design));
        self.update_pit_design_preview();
        self.invalidate_overlay();
    }

    /// A new design from closed string `string`, with the settings of the
    /// design open, or the usual ones, and open it.
    fn new_pit_design_from(&mut self, string: ObjectId) {
        if !self.activate_project_for_object(string) {
            return;
        }
        match self.create_pit_design(string, None, None) {
            Ok(id) => self.open_pit_design_item(id),
            Err(error) => crate::userspace_error!("{error:#}"),
        }
    }

    /// Add a design of closed string `string` to the active project, named
    /// `name` or numbered, with `settings` or else those of the design open
    /// (or the usual ones): one undo step. Its id.
    pub(crate) fn create_pit_design(&mut self, string: ObjectId, name: Option<String>, settings: Option<PitDesignSettings>) -> Result<PitDesignId> {
        let base = self.closed_string_points(string).ok_or_else(|| anyhow!(tr!("pit-design-problem-string")))?;
        let carried = settings.is_none();
        let mut settings = settings.unwrap_or_else(|| {
            self.editor
                .pit_design
                .as_ref()
                .and_then(|draft| draft.design.as_ref())
                .map(|design| design.working.settings())
                .unwrap_or_else(|| PitDesignSettings {
                    // The topography most projects have is the obvious ground
                    // to daylight against.
                    topography: self.triangulations.iter().find(|surface| surface.state.loaded).map(|surface| surface.id),
                    ..PitDesignSettings::default()
                })
        });
        if carried {
            settings.ramp_start = None;
            // A stop RL a few benches on from the string, as a start.
            let level = base.iter().map(|point| point.z).sum::<f64>() / base.len() as f64;
            settings.stop_elevation = level + settings.direction.sign() * settings.bench_height * f64::from(settings.benches.max(1));
        }
        let mut designs = self.active_pit_designs();
        let name = name.unwrap_or_else(|| crate::model::project::unique_item_name(tr!("pit-design-default-name"), designs.designs.iter().map(|design| design.name.as_str())));
        let id = designs.allocate_id();
        let design = PitDesign::new(id, name, base, Some(string), tr!("pit-design-scenario-default"), settings);
        self.set_pit_designs(designs.with(design));
        Ok(id)
    }

    /// Put `design` in place of the project's design with its id, as one
    /// undo step; nothing if there is none.
    pub(crate) fn replace_pit_design(&mut self, design: PitDesign) {
        let designs = self.active_pit_designs();
        if designs.get(design.id).is_some() {
            self.set_pit_designs(designs.with(design));
        }
    }

    /// The active project's designs' ids, in order.
    pub(crate) fn active_pit_design_ids(&self) -> Vec<PitDesignId> {
        self.active_pit_designs().designs.iter().map(|design| design.id).collect()
    }

    /// The design open in the window takes its string from the closed string
    /// in the selection, its ramp starts reset to the new string's start.
    pub(crate) fn change_pit_design_string(&mut self) {
        let Some(string) = self.selected_closed_string() else {
            return;
        };
        let Some(base) = self.closed_string_points(string) else {
            return;
        };
        if let Some(open) = self.editor.pit_design.as_mut().and_then(|draft| draft.design.as_mut()) {
            open.working.base = base;
            open.working.base_source = Some(string);
            for scenario in &mut open.working.scenarios {
                scenario.settings.ramp_start = None;
            }
        }
        self.flush_pit_design_draft();
        self.update_pit_design_preview();
    }

    /// Copy the open design's string again from the polyline it came from.
    pub(crate) fn update_pit_design_string(&mut self) {
        let source = self.editor.pit_design.as_ref().and_then(|draft| draft.design.as_ref()).and_then(|design| design.working.base_source);
        let Some(base) = source.and_then(|source| self.closed_string_points(source)) else {
            userspace_warn!("{}", tr!("pit-design-string-source-gone"));
            return;
        };
        if let Some(open) = self.editor.pit_design.as_mut().and_then(|draft| draft.design.as_mut()) {
            open.working.base = base;
        }
        self.flush_pit_design_draft();
        self.update_pit_design_preview();
    }

    /// Write the window's copy into the project now, settled or not.
    pub(crate) fn flush_pit_design_draft(&mut self) {
        let Some(open) = self.editor.pit_design.as_ref().and_then(|draft| draft.design.as_ref()) else {
            return;
        };
        if open.working == open.synced {
            return;
        }
        let working = open.working.clone();
        let designs = self.active_pit_designs();
        if designs.get(working.id).is_none() {
            return;
        }
        self.set_pit_designs(designs.with(working.clone()));
        if let Some(open) = self.editor.pit_design.as_mut().and_then(|draft| draft.design.as_mut()) {
            open.synced = working;
        }
    }

    /// Keep the window's copy and the project's design in step: the
    /// project's version comes back when undo or redo changed it, and the
    /// copy is written into the project once no field is mid-edit. Called
    /// every frame while the window is open.
    pub(crate) fn sync_pit_design_draft(&mut self) {
        let Some(draft) = self.editor.pit_design.as_ref() else {
            return;
        };
        let Some(open) = draft.design.as_ref() else {
            return;
        };
        let settled = draft.settled;
        let now = self.workspace.active_document().and_then(|document| document.pit_design(open.synced.id)).cloned();
        match now {
            // Deleted, by undo of its creation or another project made active:
            // the window shows the first design left, if any.
            None => {
                let next = self.active_pit_designs().designs.first().map(|design| design.id);
                if let Some(draft) = self.editor.pit_design.as_mut() {
                    draft.design = None;
                    draft.clear_preview();
                }
                if let Some(next) = next {
                    self.open_pit_design_item(next);
                }
                self.invalidate_overlay();
            }
            Some(now) if now != open.synced => {
                if let Some(open) = self.editor.pit_design.as_mut().and_then(|draft| draft.design.as_mut()) {
                    open.working = now.clone();
                    open.synced = now;
                }
            }
            Some(_) if settled => self.flush_pit_design_draft(),
            Some(_) => {}
        }
    }

    /// Put the window away, with what it was editing written to the project.
    pub(crate) fn close_pit_design(&mut self) {
        self.flush_pit_design_draft();
        self.editor.pit_design = None;
        self.cancel_jobs(|key| matches!(key, JobKey::PitDesignPreview(_)));
        self.invalidate_overlay();
    }

    pub(crate) fn delete_pit_design(&mut self, id: PitDesignId) {
        self.flush_pit_design_draft();
        let designs = self.active_pit_designs();
        let Some(design) = designs.get(id) else {
            return;
        };
        let name = design.name.clone();
        self.set_pit_designs(designs.without(id));
        userspace_log!("{}", tr!("pit-design-deleted", name = name));
        self.sync_pit_design_draft();
    }

    /// A copy of design `id` under a new name, built to nothing yet, opened.
    pub(crate) fn duplicate_pit_design(&mut self, id: PitDesignId) {
        self.flush_pit_design_draft();
        let mut designs = self.active_pit_designs();
        let Some(mut copy) = designs.get(id).cloned() else {
            return;
        };
        copy.id = designs.allocate_id();
        copy.name = crate::model::project::unique_item_name(tr!("pit-design-copy-name", name = copy.name.clone()), designs.designs.iter().map(|design| design.name.as_str()));
        copy.outputs = Outputs::default();
        let new = copy.id;
        self.set_pit_designs(designs.with(copy));
        self.open_pit_design_item(new);
    }

    /// Start building the open design again if its string, its settings or
    /// its ground have changed since the preview was built. Called every
    /// frame while the window is open: when nothing has changed it costs one
    /// comparison.
    pub(crate) fn update_pit_design_preview(&mut self) {
        self.sync_pit_design_draft();
        let Some(open) = self.editor.pit_design.as_ref().and_then(|draft| draft.design.as_ref()) else {
            return;
        };
        let base = open.working.base.clone();
        let settings = open.working.settings();
        let ground = if settings.daylights() { Some(self.pit_design_ground(settings.topography)) } else { None };
        let key = PreviewKey {
            base,
            settings,
            ground: ground.as_ref().and_then(|ground| ground.as_ref().ok()).map(|ground| ground.version),
        };
        let Some(draft) = self.editor.pit_design.as_ref() else {
            return;
        };
        if draft.built_for.as_ref() == Some(&key) || draft.building.as_ref().is_some_and(|(_, building)| *building == key) {
            return;
        }
        // Reasons there is nothing to build, settled here and now.
        let ground = match ground {
            Some(Ok(ground)) => Some(ground),
            Some(Err(reason)) => return self.settle_pit_design_preview(key, reason),
            None => None,
        };

        self.cancel_jobs(|pending| matches!(pending, JobKey::PitDesignPreview(_)));
        let Some(draft) = self.editor.pit_design.as_mut() else {
            return;
        };
        draft.next_token += 1;
        let token = draft.next_token;
        draft.building = Some((token, key.clone()));
        let (points, settings) = (key.base, key.settings);
        self.spawn_job_quietly(
            tr!("pit-design-designing"),
            vec![JobKey::PitDesignPreview(token)],
            move |cancel| design_on_worker(&points, &settings, ground.as_ref(), cancel),
            move |app, result| {
                let Some(draft) = app.editor.pit_design.as_mut() else {
                    return;
                };
                if draft.building.as_ref().is_none_or(|(building, _)| *building != token) {
                    return;
                }
                let Some((_, key)) = draft.building.take() else {
                    return;
                };
                match result {
                    Ok(preview) => {
                        draft.preview = Some(preview);
                        draft.built_for = Some(key);
                    }
                    Err(error) if crate::app::jobs::is_cancelled(&error) => {}
                    Err(error) => crate::userspace_error!("{error:#}"),
                }
                app.invalidate_overlay();
            },
        );
    }

    /// A preview with no design, for a reason known without building one.
    fn settle_pit_design_preview(&mut self, key: PreviewKey, reason: NoDesign) {
        self.cancel_jobs(|pending| matches!(pending, JobKey::PitDesignPreview(_)));
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.building = None;
            draft.preview = Some(Preview {
                design: Err(reason),
                base_elevation: 0.0,
                base_unevenness: 0.0,
                ramp_start: None,
            });
            draft.built_for = Some(key);
        }
        self.invalidate_overlay();
    }

    /// The topography a design daylights against, ready for a worker.
    fn pit_design_ground(&self, topography: Option<TriangulationId>) -> Result<Ground, NoDesign> {
        let surface = topography
            .and_then(|id| self.triangulations.iter().find(|surface| surface.id == id))
            .ok_or(NoDesign::NoTopography)?;
        if !surface.state.loaded || surface.state.deferred.is_some() {
            return Err(NoDesign::TopographyNotLoaded(surface.name.clone()));
        }
        Ok(Ground {
            mesh: surface.mesh.clone(),
            spatial: surface.spatial.clone(),
            version: surface.geometry,
        })
    }

    /// Wait for a click on the string where the ramp starts.
    pub(crate) fn begin_pit_ramp_start_pick(&mut self) {
        if let Some(draft) = self.editor.pit_design.as_mut()
            && draft.design.is_some()
        {
            draft.picking_ramp_start = true;
            self.invalidate_overlay();
        }
    }

    pub(crate) fn cancel_pit_ramp_start_pick(&mut self) {
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.picking_ramp_start = false;
        }
        self.invalidate_overlay();
    }

    /// A click while picking the ramp's start: near the design's string, it
    /// sets the start; anywhere else, the pick waits. The string is the
    /// design's own copy, so the click is tested against it in plan, not
    /// against the drawing.
    pub(crate) fn pick_pit_ramp_start_at_cursor(&mut self) {
        let Some(base) = self.editor.pit_design.as_ref().and_then(|draft| draft.design.as_ref()).map(|design| design.working.base.clone()) else {
            return;
        };
        let Some(graphics) = self.graphics.as_ref() else {
            return;
        };
        let level = base.iter().map(|point| point.z).sum::<f64>() / base.len().max(1) as f64;
        let Some((cursor, tolerance)) = graphics.cursor_on_plane(level, crate::app::PICK_THRESHOLD_PX) else {
            return;
        };
        let nearest = nearest_on_string(&base, cursor.truncate());
        if nearest.truncate().distance(cursor.truncate()) > tolerance {
            userspace_warn!("{}", tr!("pit-design-pick-on-string"));
            return;
        }
        if let Some(scenario) = self.editor.pit_design.as_mut().and_then(|draft| draft.design.as_mut()).and_then(|design| design.working.scenario_mut()) {
            scenario.settings.ramp_start = Some(nearest.truncate());
        }
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.picking_ramp_start = false;
        }
        self.update_pit_design_preview();
        self.invalidate_overlay();
    }

    /// Design a pit here and now, as the window's preview does in the
    /// background: for the MCP server.
    pub(crate) fn design_pit_now(&self, base: &[DVec3], settings: &PitDesignSettings) -> Result<Preview, NoDesign> {
        let ground = if settings.daylights() { Some(self.pit_design_ground(settings.topography)?) } else { None };
        design_on_worker(base, settings, ground.as_ref(), &CancelFlag::default()).map_err(|_| NoDesign::NotClosed)
    }

    /// Build design `id` into its layer and, when its scenario asks for one,
    /// its surface: the ones it built before, replaced in place, or new ones.
    /// The design is worked out and its surface built on a worker; the
    /// result goes in as one step, with what it was built from.
    pub(crate) fn build_pit_design(&mut self, id: PitDesignId) -> Result<()> {
        self.flush_pit_design_draft();
        let document = self.workspace.active_document().ok_or_else(|| anyhow!(tr!("err-tri-open-project-first")))?;
        let design = document.pit_design(id).cloned().ok_or_else(|| anyhow!(tr!("pit-design-gone")))?;
        // Its layer's strings are replaced one by one, so they must be in
        // memory: an unloaded layer is loaded first.
        if let Some(layer) = design.outputs.layer
            && document.deferred_layers.contains_key(&layer)
        {
            self.restore_layers_for(vec![layer], move |app| {
                if let Err(error) = app.build_pit_design(id) {
                    crate::userspace_error!("{error:#}");
                }
            });
            return Ok(());
        }
        let settings = design.settings();
        let ground = if settings.daylights() {
            Some(self.pit_design_ground(settings.topography).map_err(|reason| anyhow!(no_design_text(&reason)))?)
        } else {
            None
        };
        let trim = settings.trims();
        let project = self.workspace.active_project().ok_or_else(|| anyhow!(tr!("err-tri-open-project-first")))?;
        let project_key = JobKey::Project {
            runtime_id: project.runtime_id,
            document_revision: project.project.document.revision(),
        };
        let surface_name = design.name.clone();
        self.spawn_job_reporting_progress(
            tr!("pit-design-building", name = design.name.clone()),
            vec![project_key],
            move |cancel, progress| {
                let preview = design_on_worker(&design.base, &settings, ground.as_ref(), cancel)?;
                let built = preview.design.map_err(|reason| anyhow!(no_design_text(&reason)))?;
                anyhow::ensure!(built.benches > 0, tr!("pit-design-no-bench-fits", reason = end_text(&built)));
                let surface = if settings.surface {
                    let (vertices, faces) = surface_on_worker(&built, ground.as_ref().filter(|_| trim), cancel, progress)?;
                    Some(crate::app::commands::triangulation::session::build_generated_triangulation(surface_name, vertices, faces, crate::ui::state::TriSurfaceType::Surface, |_| {
                        Vec::new()
                    })?)
                } else {
                    None
                };
                Ok(BuildResult {
                    design: built,
                    surface,
                    ground: ground.map(|ground| ground.version),
                })
            },
            move |app, result| match result.and_then(|built| app.install_pit_design_build(id, built)) {
                Ok(()) => {}
                Err(error) if crate::app::jobs::is_cancelled(&error) => {}
                Err(error) => crate::userspace_error!("{}", tr!("pit-design-build-failed", error = format!("{error:#}"))),
            },
        );
        Ok(())
    }

    /// Put a finished build into the project as one step: the design's
    /// strings in place of its layer's, its surface in place of the one it
    /// built before, and what they were built from.
    fn install_pit_design_build(&mut self, id: PitDesignId, built: BuildResult) -> Result<()> {
        let BuildResult { design: result, surface, ground } = built;
        let designs = self.active_pit_designs();
        let design = designs.get(id).cloned().ok_or_else(|| anyhow!(tr!("pit-design-gone")))?;
        let mut commands = Vec::new();

        // The strings: the layer's own replaced, or a new layer.
        let project = self.workspace.active_project_mut().ok_or_else(|| anyhow!(tr!("err-tri-open-project-first")))?;
        let document = &mut project.project.document;
        let kept_layer = design.outputs.layer.filter(|layer| document.layer(*layer).is_some());
        let layer = match kept_layer {
            Some(layer) => {
                let old: Vec<Object> = document.objects().iter().filter(|object| object.layer() == layer).cloned().collect();
                commands.extend(old.into_iter().map(Command::delete_object));
                commands.extend(string_objects(&result, layer, || document.allocate_object_id()).into_iter().map(Command::AddObject));
                layer
            }
            None => {
                let layer = document.allocate_layer_id();
                let name = crate::model::project::unique_item_name(design.name.clone(), document.layers().iter().map(|layer| layer.name.as_str()));
                let objects = string_objects(&result, layer, || document.allocate_object_id());
                commands.push(Command::AddLayerSnapshot {
                    layer: Layer {
                        id: layer,
                        name,
                        color_index: None,
                        color: [1.0, 1.0, 1.0, 1.0],
                        loaded: true,
                        hidden: false,
                        elevation: 0.0,
                        folder: None,
                        section: SectionKind::natural_layer(),
                    },
                    objects,
                });
                layer
            }
        };
        let fingerprint = strings_fingerprint(result.strings.iter().map(|string| (string.points.as_slice(), string.closed)));

        // The surface: replaced in place, new, or none asked for.
        let kept_surface = design.outputs.surface.filter(|surface| self.triangulations.iter().any(|item| item.id == *surface));
        let (surface_id, surface_geometry) = match (surface, kept_surface) {
            (Some(built), Some(existing)) => {
                let command = self.replace_triangulation_command(existing, built).ok_or_else(|| anyhow!(tr!("pit-design-gone")))?;
                let geometry = added_geometry(&command);
                commands.push(command);
                (Some(existing), geometry)
            }
            (Some(built), None) => {
                let section = SectionKind::natural_for(crate::model::MemberKind::Triangulation);
                let (ids, added) = self.add_triangulation_commands(vec![crate::app::commands::triangulation::session::new_triangulation(built, section)]);
                let geometry = added.first().and_then(added_geometry);
                commands.extend(added);
                (ids.first().copied(), geometry)
            }
            (None, existing) => {
                // A surface built before is left as it is, no longer the
                // design's: the strings no longer match it.
                if let Some(name) = existing.and_then(|id| self.triangulations.iter().find(|item| item.id == id)).map(|item| item.name.clone()) {
                    userspace_log!("{}", tr!("pit-design-surface-unlinked", name = name));
                }
                (None, None)
            }
        };

        let mut updated = design.clone();
        updated.outputs = Outputs {
            layer: Some(layer),
            surface: surface_id,
            built: Some(Built {
                scenario: design.scenario().map_or(design.active, |scenario| scenario.id),
                settings: design.settings(),
                base: design.base.clone(),
                topography: ground,
                layer_fingerprint: fingerprint,
                surface_geometry,
            }),
        };
        commands.push(Command::SetPitDesigns {
            before: Box::new(designs.clone()),
            after: Box::new(designs.with(updated)),
        });
        let Some(command) = Command::step(commands) else {
            return Ok(());
        };
        self.execute_job_edit(command);
        self.editor.active_layer = Some(layer);
        let layer_name = self
            .workspace
            .active_document()
            .and_then(|document| document.layer(layer))
            .map(|layer| layer.name.clone())
            .unwrap_or_default();
        userspace_log!(
            "{}",
            tr!(
                "pit-design-created",
                benches = result.benches.to_string(),
                strings = result.strings.len().to_string(),
                layer = layer_name,
                ended = end_text(&result)
            )
        );
        if let Some(name) = surface_id.and_then(|id| self.triangulations.iter().find(|item| item.id == id)).map(|item| item.name.clone()) {
            userspace_log!("{}", tr!("pit-design-created-surface", name = name));
        }
        self.invalidate_geometry();
        Ok(())
    }

    /// The window's and the explorer's view of the active project's designs.
    pub(crate) fn pit_design_entries(&self) -> Vec<UiPitDesignEntry> {
        let Some(document) = self.workspace.active_document() else {
            return Vec::new();
        };
        document
            .pit_designs()
            .designs
            .iter()
            .map(|design| {
                let layer = design.outputs.layer.and_then(|id| document.layer(id)).map(|layer| (layer.id, layer.name.clone()));
                let surface = design
                    .outputs
                    .surface
                    .and_then(|id| self.triangulations.iter().find(|item| item.id == id))
                    .map(|item| (item.id, item.name.clone(), item.geometry));
                let built = design.outputs.built.as_ref();
                // An unloaded layer cannot be read, and is not said to be edited.
                let layer_edited = match (built, &layer) {
                    (Some(built), Some((id, _))) if !document.deferred_layers.contains_key(id) => layer_fingerprint(document, *id) != built.layer_fingerprint,
                    _ => false,
                };
                let surface_edited = match (built, &surface) {
                    (Some(built), Some((_, _, geometry))) => built.surface_geometry.is_some_and(|built| built != *geometry),
                    _ => false,
                };
                let ground = design
                    .settings()
                    .ground()
                    .and_then(|id| self.triangulations.iter().find(|item| item.id == id))
                    .map(|item| item.geometry);
                let standing = design.standing(ground, layer.is_some(), surface.is_some());
                let source_differs = design
                    .base_source
                    .and_then(|source| self.closed_string_points(source))
                    .is_some_and(|points| points != design.base);
                UiPitDesignEntry {
                    id: design.id,
                    name: design.name.clone(),
                    layer,
                    surface: surface.map(|(id, name, _)| (id, name)),
                    layer_edited,
                    surface_edited,
                    source_differs,
                    standing,
                }
            })
            .collect()
    }
}

/// The geometry version of the surface a command adds or puts in place.
fn added_geometry(command: &Command) -> Option<GeometryVersion> {
    match command {
        Command::ReplaceItem {
            other: Some(OpenItem::Triangulation(item)),
            ..
        }
        | Command::AddItem {
            added: Some(OpenItem::Triangulation(item)),
            ..
        } => Some(item.geometry),
        _ => None,
    }
}
