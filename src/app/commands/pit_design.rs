//! Pit Design: pit design objects, the window that edits them, the preview
//! it keeps current in the background, working designs out again when what
//! they are designed from changes, building their surfaces, and exploding
//! them into polylines.
//!
//! A pit design is an object in a design layer ([`Object::PitDesign`]). Pit
//! Design opened on a closed polyline turns it into one, in its place. The
//! window edits a copy of what the object is designed with; once no field is
//! mid-edit, [`App::sync_pit_design_draft`] writes the copy back as one undo
//! step, with the design worked out for it when the preview has it.
//! [`App::refresh_pit_designs`] works out again, in the background, any
//! design whose string or ground has changed since: that is not an edit of
//! its own, so undo takes back the change that caused it.

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
        Command, Object, ObjectId, OpenItem, PolyVertex, SceneEntityId, SectionKind,
        formats::mesh_data,
        pit_design::{
            self, Base, Design,
            object::{Derived, Outcome, PitObject, SurfaceLink, base_points, inputs_key},
            settings::PitDesignSettings,
        },
        spatial::TriangleBvh,
        triangulation::{GeneratedTriangulation, GeometryVersion, TriangulationId},
    },
    ui::{
        dialogs::pit_design::{DraftFacts, NoDesign, PitDesignDraft, Preview, PreviewKey, end_text, no_design_text},
        state::{ActiveTool, TriSurfaceCutSide},
    },
    userspace_log, userspace_warn,
};

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

/// Work a design out on a worker: the string, its settings, and the ground
/// to daylight against.
fn design_on_worker(verts: &[PolyVertex], settings: &PitDesignSettings, ground: Option<&Ground>, cancel: &CancelFlag) -> Result<Preview> {
    let points = base_points(verts);
    let Some(base) = Base::from_points(&points) else {
        return Ok(Preview {
            design: Err(NoDesign::NotClosed),
            base_elevation: 0.0,
            base_unevenness: 0.0,
            ramp_start: None,
        });
    };
    let default_start = points.first().map_or(DVec2::ZERO, |point| point.truncate());
    let spec = settings.spec(default_start);
    let ramp_start = spec.ramp.map(|ramp| nearest_on_string(&points, ramp.start));
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

/// What a worked-out design is kept on its object as.
fn derived_from(verts: &[PolyVertex], settings: &PitDesignSettings, ground: Option<GeometryVersion>, design: &Result<Design, NoDesign>) -> Option<Derived> {
    let outcome = match design {
        Ok(design) => Outcome::Design(Box::new(design.clone())),
        Err(NoDesign::NotClosed) => Outcome::NotClosed,
        Err(NoDesign::Settings(problem)) => Outcome::Problem(*problem),
        // Missing ground is not an outcome: it is worked out once there is.
        Err(NoDesign::NoTopography | NoDesign::TopographyNotLoaded(_)) => return None,
    };
    Some(Derived {
        inputs: inputs_key(verts, settings),
        ground: settings.ground().and(ground),
        outcome,
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

/// A pit design object's parts, for the code that only reads them.
pub(crate) fn pit_parts(object: &Object) -> Option<(&[PolyVertex], &PitObject)> {
    match object {
        Object::PitDesign { verts, pit, .. } => Some((verts.as_slice(), pit)),
        _ => None,
    }
}

/// `object` with `pit` in place of what it is designed with.
fn with_pit(object: &Object, pit: PitObject) -> Object {
    let mut object = object.clone();
    if let Object::PitDesign { pit: held, .. } = &mut object {
        **held = pit;
    }
    object
}

/// What a finished surface build made, for the step that installs it.
struct SurfaceBuild {
    surface: GeneratedTriangulation,
    derived: Option<Derived>,
    inputs: u64,
    ground: Option<GeometryVersion>,
    trimmed: bool,
}

impl App<'_> {
    /// The pit design object or closed polyline in the selection, which Pit
    /// Design opens on.
    fn selected_pit_or_string(&self) -> Option<ObjectId> {
        self.editor.selected_handles.iter().find_map(|handle| match handle {
            SceneEntityId::Object(id) => self
                .operand_object(*id)
                .filter(|object| matches!(object, Object::PitDesign { .. }) || (matches!(object, Object::Polyline { .. }) && object.encloses_area()))
                .map(|_| *id),
            _ => None,
        })
    }

    /// The version of the ground a design with `settings` daylights against,
    /// if it daylights against one that is loaded.
    fn pit_ground_version(&self, settings: &PitDesignSettings) -> Option<GeometryVersion> {
        settings
            .ground()
            .and_then(|id| self.triangulations.iter().find(|surface| surface.id == id))
            .filter(|surface| surface.state.loaded && surface.state.deferred.is_none())
            .map(|surface| surface.geometry)
    }

    /// Take up the Pit Design tool, from the menu or the context menu: on the
    /// pit design selected, or on a closed polyline selected, which becomes a
    /// pit design in its place; otherwise the tool waits for a click on one.
    pub(crate) fn open_pit_design(&mut self) {
        let selected = self.selected_pit_or_string();
        if self.editor.active_tool != ActiveTool::PitDesign {
            self.set_active_tool_from_toolbar(ActiveTool::PitDesign);
        }
        // Editing a pit, or making one of a polyline already drawn, needs no
        // layer to draw on: only drawing a new string does.
        if self.editor.active_tool != ActiveTool::PitDesign && selected.is_some() && self.active_layer().is_none() && !self.editor.text_editing_enabled && !self.editor.slice_mode_enabled {
            self.editor.active_tool = ActiveTool::PitDesign;
        }
        if self.editor.active_tool != ActiveTool::PitDesign {
            return;
        }
        if let Some(id) = selected {
            self.open_pit_design_target(id);
        }
        self.invalidate_overlay();
    }

    /// Close the string drawn with Pit Design into a new pit design on the
    /// active layer, its corners rounded if the panel says so, and open it in
    /// the panel: one undo step.
    pub(crate) fn finish_pit_string(&mut self) {
        let mut points = std::mem::take(&mut self.editor.pending_stroke);
        // Clicking the first vertex to close records it a second time.
        if points.len() >= 2 && points.first().zip(points.last()).is_some_and(|(first, last)| first.distance_squared(*last) <= 1.0e-16) {
            points.pop();
        }
        self.invalidate_geometry();
        if points.len() < 3 {
            return;
        }
        let Some(layer) = self.active_layer() else {
            return;
        };
        let verts = if self.editor.pit_round_corners {
            crate::model::pit_design::object::rounded_corners(&points, self.editor.pit_corner_radius)
        } else {
            points.iter().copied().map(PolyVertex::straight).collect()
        };
        let Some(project) = self.workspace.active_project_mut() else {
            return;
        };
        let id = project.project.document.allocate_object_id();
        let object = Object::PitDesign {
            id,
            layer,
            verts,
            color: crate::model::ObjectColor::Fixed(self.editor.tool_line_color),
            line_weight: self.editor.tool_line_weight,
            pit: Box::new(self.new_pit(&points, None, None)),
        };
        let mut object = object;
        if let Object::PitDesign { verts, pit, .. } = &mut object {
            self.work_out_now(verts, pit);
        }
        self.execute_edit(Command::AddObject(object));
        self.open_pit_design_on(id);
        self.invalidate_geometry();
    }

    /// Done with the pit in the panel: what it was editing is kept, and the
    /// tool is put down, as it is once its one pit is made.
    pub(crate) fn finish_pit_editing(&mut self) {
        self.close_pit_design();
        self.editor.active_tool = ActiveTool::None;
    }

    /// Escape with Pit Design: a string being drawn is dropped, and with none
    /// the tool is put down, the pit being edited kept as it is.
    pub(crate) fn escape_pit_design(&mut self) {
        if !self.editor.pending_stroke.is_empty() {
            self.editor.pending_stroke.clear();
            self.invalidate_geometry();
        } else {
            self.finish_pit_editing();
        }
    }

    /// Open `id` in the panel, making a pit design of it first if it is a
    /// closed polyline.
    fn open_pit_design_target(&mut self, id: ObjectId) {
        if matches!(self.operand_object(id), Some(Object::PitDesign { .. })) {
            self.open_pit_design_on(id);
            return;
        }
        self.flush_pit_design_draft();
        match self.make_pit_design(id, None, None) {
            Ok(()) => self.open_pit_design_on(id),
            Err(error) => crate::userspace_error!("{error:#}"),
        }
    }

    /// Open pit design object `id` in the tool's panel.
    pub(crate) fn open_pit_design_on(&mut self, id: ObjectId) {
        if self.editor.pit_design.as_ref().and_then(PitDesignDraft::object_id) != Some(id) {
            self.flush_pit_design_draft();
        }
        let Some((_, pit)) = self.operand_object(id).and_then(pit_parts) else {
            return;
        };
        let pit = pit.clone();
        if !self.activate_project_for_object(id) {
            return;
        }
        self.cancel_jobs(|key| matches!(key, JobKey::PitDesignPreview(_)));
        self.editor.pit_design = Some(PitDesignDraft::on(id, pit));
        self.update_pit_design_preview();
        self.invalidate_overlay();
    }

    /// Turn closed polyline `string` into a pit design in its place, named
    /// `name` or numbered, designed with `settings` or else those of the
    /// design open (or the usual ones): one undo step. The design is worked
    /// out here and now, so it draws at once.
    pub(crate) fn make_pit_design(&mut self, string: ObjectId, name: Option<String>, settings: Option<PitDesignSettings>) -> Result<()> {
        if !self.activate_project_for_object(string) {
            return Err(anyhow!(tr!("pit-design-problem-string")));
        }
        let Some(before @ Object::Polyline { .. }) = self.operand_object(string).cloned().filter(Object::encloses_area) else {
            return Err(anyhow!(tr!("pit-design-problem-string")));
        };
        let Object::Polyline {
            verts, color, line_weight, layer, ..
        } = before.clone()
        else {
            return Err(anyhow!(tr!("pit-design-problem-string")));
        };
        let points: Vec<_> = verts.iter().map(|vertex| vertex.pos).collect();
        let mut pit = self.new_pit(&points, name, settings);
        self.work_out_now(&verts, &mut pit);
        let after = Object::PitDesign {
            id: string,
            layer,
            verts,
            color,
            line_weight,
            pit: Box::new(pit),
        };
        self.execute_edit(Command::Replace { before, after });
        self.invalidate_geometry();
        Ok(())
    }

    /// What a new pit design of the string through `points` is designed
    /// with: named `name` or numbered, with `settings` or else those of the
    /// design last open (or the usual ones), its ramp starting where the
    /// string does and its stop RL a few benches on.
    fn new_pit(&self, points: &[glam::DVec3], name: Option<String>, settings: Option<PitDesignSettings>) -> PitObject {
        let carried = settings.is_none();
        let mut settings = settings.unwrap_or_else(|| {
            self.editor
                .pit_design
                .as_ref()
                .and_then(|draft| draft.working.as_ref())
                .map(PitObject::settings)
                .or_else(|| self.editor.pit_last_settings.clone())
                .unwrap_or_else(|| PitDesignSettings {
                    // The topography most projects have is the obvious ground
                    // to daylight against.
                    topography: self.triangulations.iter().find(|surface| surface.state.loaded).map(|surface| surface.id),
                    ..PitDesignSettings::default()
                })
        });
        if carried {
            settings.ramp_start = None;
            let level = points.iter().map(|point| point.z).sum::<f64>() / points.len().max(1) as f64;
            settings.stop_elevation = level + settings.direction.sign() * settings.bench_height * f64::from(settings.benches.max(1));
        }
        let name = name.unwrap_or_else(|| {
            let names: Vec<String> = self
                .workspace
                .active_document()
                .map(|document| document.objects().iter().filter_map(pit_parts).map(|(_, pit)| pit.name.clone()).collect())
                .unwrap_or_default();
            crate::model::project::unique_item_name(tr!("pit-design-default-name"), names.iter().map(String::as_str))
        });
        PitObject::new(name, tr!("pit-design-scenario-default"), settings)
    }

    /// Work `pit`'s design out here and now, so a new one draws at once.
    fn work_out_now(&self, verts: &[PolyVertex], pit: &mut PitObject) {
        let settings = pit.settings();
        let ground = if settings.daylights() { self.pit_design_ground(settings.topography).ok() } else { None };
        if let Ok(preview) = design_on_worker(verts, &settings, ground.as_ref(), &CancelFlag::default()) {
            pit.derived = derived_from(verts, &settings, ground.as_ref().map(|ground| ground.version), &preview.design);
        }
    }

    /// Write the window's copy into the project now, settled or not, with
    /// the design the preview worked out for it if it has one.
    pub(crate) fn flush_pit_design_draft(&mut self) {
        let Some(draft) = self.editor.pit_design.as_ref() else {
            return;
        };
        let (Some(id), Some(working), Some(synced)) = (draft.object_id(), draft.working.clone(), draft.synced.as_ref()) else {
            return;
        };
        if working == *synced {
            return;
        }
        let Some(before) = self.operand_object(id).filter(|object| matches!(object, Object::PitDesign { .. })).cloned() else {
            return;
        };
        let Some((verts, _)) = pit_parts(&before) else {
            return;
        };
        let mut working = working;
        // The preview's design, if it is of this copy, saves working it out
        // again; otherwise the design is left stale, and worked out again.
        let settings = working.settings();
        if let (Some(preview), Some(key)) = (draft.preview.as_ref(), draft.built_for.as_ref())
            && key.verts == verts
            && key.settings == settings
            && let Some(derived) = derived_from(verts, &settings, key.ground, &preview.design)
        {
            working.derived = Some(derived);
        }
        let after = with_pit(&before, working.clone());
        self.execute_edit(Command::Replace { before, after });
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.synced = Some(working.clone());
            draft.working = Some(working);
        }
        self.invalidate_geometry();
    }

    /// Keep the window's copy and the object in step: the object's version
    /// comes back when undo, redo or a new design worked out changed it, and
    /// the copy is written to the object once no field is mid-edit. Called
    /// every frame while the window is open.
    pub(crate) fn sync_pit_design_draft(&mut self) {
        let Some(draft) = self.editor.pit_design.as_ref() else {
            return;
        };
        let Some(id) = draft.object_id() else {
            return;
        };
        let now = self.operand_object(id).and_then(pit_parts).map(|(_, pit)| pit.clone());
        let Some(now) = now else {
            // Gone: deleted, exploded, or undone back to the polyline it was.
            if let Some(draft) = self.editor.pit_design.as_mut() {
                *draft = PitDesignDraft::default();
            }
            self.invalidate_overlay();
            return;
        };
        let settled = draft.settled;
        let synced = draft.synced.clone();
        let working = draft.working.clone();
        if synced.as_ref() != Some(&now) {
            // A new design worked out keeps what is being typed; anything else
            // the object now holds replaces it.
            let only_derived = synced.as_ref().is_some_and(|synced| PitObject { derived: now.derived.clone(), ..synced.clone() } == now);
            if let Some(draft) = self.editor.pit_design.as_mut() {
                draft.working = match (only_derived, working) {
                    (true, Some(mut working)) => {
                        working.derived = now.derived.clone();
                        Some(working)
                    }
                    _ => Some(now.clone()),
                };
                draft.synced = Some(now);
            }
        } else if settled && working.as_ref() != synced.as_ref() {
            self.flush_pit_design_draft();
        }
        self.update_pit_draft_facts();
    }

    /// What the window shows of the object beyond what it edits: its layer,
    /// its surface and how that stands, and whether its design is current.
    fn update_pit_draft_facts(&mut self) {
        let Some(id) = self.editor.pit_design.as_ref().and_then(PitDesignDraft::object_id) else {
            return;
        };
        let Some(object) = self.operand_object(id) else {
            return;
        };
        let Some((verts, pit)) = pit_parts(object) else {
            return;
        };
        let surface = pit.surface.as_ref().and_then(|link| self.triangulations.iter().find(|item| item.id == link.id));
        let ground = self.pit_ground_version(&pit.settings());
        let facts = DraftFacts {
            surface: surface.map(|item| (item.id, item.name.clone())),
            surface_standing: pit.surface_standing(verts, surface.map(|item| item.geometry), ground),
            fresh: pit.is_fresh(verts, ground),
        };
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.facts = facts;
        }
    }

    /// Put the panel away, with what it was editing written to the object:
    /// the tool is being put down.
    pub(crate) fn close_pit_design(&mut self) {
        self.flush_pit_design_draft();
        if let Some(settings) = self.editor.pit_design.as_ref().and_then(|draft| draft.working.as_ref()).map(PitObject::settings) {
            self.editor.pit_last_settings = Some(settings);
        }
        self.editor.pending_stroke.clear();
        self.editor.pit_design = None;
        if self.editor.tool_highlight_id.take().is_some() {
            self.invalidate_geometry();
        }
        self.cancel_jobs(|key| matches!(key, JobKey::PitDesignPreview(_)));
        self.invalidate_overlay();
    }

    /// Start working the window's copy out again if it, the string or the
    /// ground have changed since the preview was built. Called every frame
    /// while the window is open: when nothing has changed it costs one
    /// comparison.
    pub(crate) fn update_pit_design_preview(&mut self) {
        self.sync_pit_design_draft();
        let Some(draft) = self.editor.pit_design.as_ref() else {
            return;
        };
        let (Some(id), Some(working)) = (draft.object_id(), draft.working.as_ref()) else {
            return;
        };
        let settings = working.settings();
        let Some((verts, _)) = self.operand_object(id).and_then(pit_parts) else {
            return;
        };
        let verts = verts.to_vec();
        let ground = if settings.daylights() { Some(self.pit_design_ground(settings.topography)) } else { None };
        let key = PreviewKey {
            verts,
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
        let (verts, settings) = (key.verts, key.settings);
        self.spawn_job_quietly(
            tr!("pit-design-designing"),
            vec![JobKey::PitDesignPreview(token)],
            move |cancel| design_on_worker(&verts, &settings, ground.as_ref(), cancel),
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

    /// Work out again, in the background, the design of every pit design
    /// object in the active project whose string or ground has changed since
    /// it was last worked out. Called every frame; it looks again only when
    /// the project or the surfaces have changed.
    pub(crate) fn refresh_pit_designs(&mut self) {
        let Some(project) = self.workspace.active_project() else {
            return;
        };
        let checked = {
            use std::hash::{DefaultHasher, Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            project.runtime_id.hash(&mut hasher);
            project.project.document.revision().hash(&mut hasher);
            for surface in &self.triangulations {
                (surface.id, surface.geometry, surface.state.loaded, surface.state.deferred.is_some()).hash(&mut hasher);
            }
            self.editor.pit_refreshes.len().hash(&mut hasher);
            hasher.finish()
        };
        if self.editor.pit_refresh_checked == Some(checked) {
            return;
        }
        self.editor.pit_refresh_checked = Some(checked);
        let stale: Vec<(ObjectId, Vec<PolyVertex>, PitDesignSettings, u64)> = project
            .project
            .document
            .objects()
            .iter()
            .filter_map(|object| {
                let (verts, pit) = pit_parts(object)?;
                let settings = pit.settings();
                let ground = self.pit_ground_version(&settings);
                // A design against ground that is not loaded waits for it.
                if (settings.ground().is_some() && ground.is_none()) || pit.is_fresh(verts, ground) {
                    return None;
                }
                let key = inputs_key(verts, &settings) ^ ground.map_or(0, |ground| ground.0.as_u64_pair().0);
                (self.editor.pit_refreshes.get(&object.id()) != Some(&key)).then(|| (object.id(), verts.to_vec(), settings, key))
            })
            .collect();
        for (id, verts, settings, key) in stale {
            let ground = if settings.daylights() { self.pit_design_ground(settings.topography).ok() } else { None };
            self.editor.pit_refreshes.insert(id, key);
            self.spawn_job_quietly(
                tr!("pit-design-designing"),
                vec![JobKey::PitDesignRefresh(id)],
                move |cancel| {
                    let preview = design_on_worker(&verts, &settings, ground.as_ref(), cancel)?;
                    let derived = derived_from(&verts, &settings, ground.as_ref().map(|ground| ground.version), &preview.design);
                    Ok((derived, verts, settings))
                },
                move |app, result| {
                    if app.editor.pit_refreshes.get(&id) == Some(&key) {
                        app.editor.pit_refreshes.remove(&id);
                    }
                    let Ok((Some(derived), verts, settings)) = result else {
                        return;
                    };
                    // Only onto the object as it was worked out for: one
                    // edited since is worked out again for what it is now.
                    let Some(document) = app.workspace.active_document_mut() else {
                        return;
                    };
                    let Some(object) = document
                        .get_object(id)
                        .filter(|object| pit_parts(object).is_some_and(|(now, pit)| now == verts.as_slice() && pit.settings() == settings))
                    else {
                        return;
                    };
                    let mut object = object.clone();
                    if let Object::PitDesign { pit, .. } = &mut object {
                        pit.derived = Some(derived);
                    }
                    document.replace_object(object);
                    app.invalidate_geometry();
                },
            );
        }
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
            && draft.object_id().is_some()
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
    /// sets the start; anywhere else, the pick waits.
    pub(crate) fn pick_pit_ramp_start_at_cursor(&mut self) {
        let Some(id) = self.editor.pit_design.as_ref().and_then(PitDesignDraft::object_id) else {
            return;
        };
        let Some(points) = self.operand_object(id).and_then(Object::tessellated_path).map(|(points, _)| points) else {
            return;
        };
        let Some(graphics) = self.graphics.as_ref() else {
            return;
        };
        let level = points.iter().map(|point| point.z).sum::<f64>() / points.len().max(1) as f64;
        let Some((cursor, tolerance)) = graphics.cursor_on_plane(level, crate::app::PICK_THRESHOLD_PX) else {
            return;
        };
        let nearest = nearest_on_string(&points, cursor.truncate());
        if nearest.truncate().distance(cursor.truncate()) > tolerance {
            userspace_warn!("{}", tr!("pit-design-pick-on-string"));
            return;
        }
        if let Some(scenario) = self.editor.pit_design.as_mut().and_then(|draft| draft.working.as_mut()).and_then(PitObject::scenario_mut) {
            scenario.settings.ramp_start = Some(nearest.truncate());
        }
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.picking_ramp_start = false;
        }
        self.update_pit_design_preview();
        self.invalidate_overlay();
    }

    /// Work a design out here and now, as the preview does in the
    /// background: for the MCP server.
    pub(crate) fn design_pit_now(&self, verts: &[PolyVertex], settings: &PitDesignSettings) -> Result<Preview, NoDesign> {
        let ground = if settings.daylights() { Some(self.pit_design_ground(settings.topography)?) } else { None };
        design_on_worker(verts, settings, ground.as_ref(), &CancelFlag::default()).map_err(|_| NoDesign::NotClosed)
    }

    /// Put `pit` on pit design object `id` as one undo step, its design
    /// worked out here and now: for the MCP server.
    pub(crate) fn set_pit_now(&mut self, id: ObjectId, mut pit: PitObject) -> Result<Result<Design, NoDesign>> {
        let before = self
            .operand_object(id)
            .filter(|object| matches!(object, Object::PitDesign { .. }))
            .cloned()
            .ok_or_else(|| anyhow!(tr!("pit-design-gone")))?;
        let Some((verts, _)) = pit_parts(&before) else {
            return Err(anyhow!(tr!("pit-design-gone")));
        };
        let settings = pit.settings();
        let design = self.design_pit_now(verts, &settings).and_then(|preview| preview.design);
        if let Some(derived) = derived_from(verts, &settings, self.pit_ground_version(&settings), &design) {
            pit.derived = Some(derived);
        }
        let after = with_pit(&before, pit);
        if after != before {
            self.execute_edit(Command::Replace { before, after });
            self.invalidate_geometry();
        }
        Ok(design)
    }

    /// Build pit design `id`'s surface from its strings, trimmed to the
    /// topography it daylights against when its settings say so: the surface
    /// it built before, replaced in place, or a new one. The design is worked
    /// out again with the surface if it is not current.
    pub(crate) fn build_pit_surface(&mut self, id: ObjectId) -> Result<()> {
        self.flush_pit_design_draft();
        let object = self.operand_object(id).cloned().ok_or_else(|| anyhow!(tr!("pit-design-gone")))?;
        let Some((verts, pit)) = pit_parts(&object) else {
            return Err(anyhow!(tr!("pit-design-gone")));
        };
        if !self.activate_project_for_object(id) {
            return Err(anyhow!(tr!("pit-design-gone")));
        }
        let settings = pit.settings();
        let ground = if settings.daylights() {
            Some(self.pit_design_ground(settings.topography).map_err(|reason| anyhow!(no_design_text(&reason)))?)
        } else {
            None
        };
        let fresh = pit.is_fresh(verts, ground.as_ref().map(|ground| ground.version));
        let known = pit.design().filter(|_| fresh).cloned();
        let trim = settings.trims();
        let name = pit.name.clone();
        let verts = verts.to_vec();
        // Building again replaces a build of the same design still running.
        self.cancel_jobs(|key| *key == JobKey::PitSurface(id));
        self.spawn_job_reporting_progress(
            tr!("pit-design-building-surface", name = name.clone()),
            vec![JobKey::PitSurface(id)],
            move |cancel, progress| {
                let (design, derived) = match known {
                    Some(design) => (design, None),
                    None => {
                        let preview = design_on_worker(&verts, &settings, ground.as_ref(), cancel)?;
                        let derived = derived_from(&verts, &settings, ground.as_ref().map(|ground| ground.version), &preview.design);
                        (preview.design.map_err(|reason| anyhow!(no_design_text(&reason)))?, derived)
                    }
                };
                anyhow::ensure!(design.benches > 0, tr!("pit-design-no-bench-fits", reason = end_text(&design)));
                let (vertices, faces) = surface_on_worker(&design, ground.as_ref().filter(|_| trim), cancel, progress)?;
                let surface = crate::app::commands::triangulation::session::build_generated_triangulation(name, vertices, faces, crate::ui::state::TriSurfaceType::Surface, |_| Vec::new())?;
                Ok(SurfaceBuild {
                    surface,
                    derived,
                    inputs: inputs_key(&verts, &settings),
                    ground: settings.ground().and(ground.map(|ground| ground.version)),
                    trimmed: trim,
                })
            },
            move |app, result| match result.and_then(|built| app.install_pit_surface(id, built)) {
                Ok(()) => {}
                Err(error) if crate::app::jobs::is_cancelled(&error) => {}
                Err(error) => crate::userspace_error!("{}", tr!("pit-design-surface-failed", error = format!("{error:#}"))),
            },
        );
        Ok(())
    }

    /// Put a finished surface build into the project as one step: the
    /// surface in place of the one the design built before (unless another
    /// design copied from it links it too), and the link on the object.
    fn install_pit_surface(&mut self, id: ObjectId, built: SurfaceBuild) -> Result<()> {
        let SurfaceBuild {
            surface,
            derived,
            inputs,
            ground,
            trimmed,
        } = built;
        let before = self.operand_object(id).cloned().ok_or_else(|| anyhow!(tr!("pit-design-gone")))?;
        let Some((verts, pit)) = pit_parts(&before) else {
            return Err(anyhow!(tr!("pit-design-gone")));
        };
        // Only onto the design it was built from: one changed since says so,
        // and is built again on request.
        if inputs_key(verts, &pit.settings()) != inputs {
            userspace_warn!("{}", tr!("pit-design-surface-overtaken", name = pit.name.clone()));
            return Ok(());
        }
        let linked = pit.surface.as_ref().map(|link| link.id).filter(|surface| self.triangulations.iter().any(|item| item.id == *surface));
        // A design copied from this one carries the same link: the surface is
        // then the original's, and the copy builds its own.
        let shared = linked.is_some_and(|surface| {
            self.workspace.active_document().is_some_and(|document| {
                document
                    .objects()
                    .iter()
                    .filter(|object| object.id() != id)
                    .filter_map(pit_parts)
                    .any(|(_, other)| other.surface.as_ref().is_some_and(|link| link.id == surface))
            })
        });
        let mut commands = Vec::new();
        let (surface_id, geometry, name) = match linked.filter(|_| !shared) {
            Some(existing) => {
                let command = self.replace_triangulation_command(existing, surface).ok_or_else(|| anyhow!(tr!("pit-design-gone")))?;
                let geometry = added_geometry(&command);
                commands.push(command);
                let name = self.triangulations.iter().find(|item| item.id == existing).map(|item| item.name.clone()).unwrap_or_default();
                (existing, geometry, name)
            }
            None => {
                let section = SectionKind::natural_for(crate::model::MemberKind::Triangulation);
                let (ids, added) = self.add_triangulation_commands(vec![crate::app::commands::triangulation::session::new_triangulation(surface, section)]);
                let geometry = added.first().and_then(added_geometry);
                let name = added
                    .first()
                    .and_then(|command| match command {
                        Command::AddItem {
                            added: Some(OpenItem::Triangulation(item)),
                            ..
                        } => Some(item.name.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                commands.extend(added);
                (ids.first().copied().ok_or_else(|| anyhow!(tr!("pit-design-gone")))?, geometry, name)
            }
        };
        let mut pit = pit.clone();
        if let Some(derived) = derived {
            pit.derived = Some(derived);
        }
        pit.surface = geometry.map(|geometry| SurfaceLink {
            id: surface_id,
            inputs,
            ground,
            trimmed,
            geometry,
        });
        let after = with_pit(&before, pit);
        commands.push(Command::Replace { before, after });
        let Some(command) = Command::step(commands) else {
            return Ok(());
        };
        self.execute_job_edit(command);
        userspace_log!("{}", tr!("pit-design-created-surface", name = name));
        self.invalidate_geometry();
        Ok(())
    }

    /// Turn pit design `id` into polylines on its layer: its string, in its
    /// place, and each of its benches' strings after it, in their colours.
    /// One undo step. The polylines' ids, the string's first.
    pub(crate) fn explode_pit_design(&mut self, id: ObjectId) -> Result<Vec<ObjectId>> {
        self.flush_pit_design_draft();
        let before = self.operand_object(id).cloned().ok_or_else(|| anyhow!(tr!("pit-design-gone")))?;
        if !matches!(before, Object::PitDesign { .. }) || !self.activate_project_for_object(id) {
            return Err(anyhow!(tr!("pit-design-gone")));
        }
        let mut parts = before.pit_polylines().into_iter();
        let Some(string) = parts.next() else {
            return Err(anyhow!(tr!("pit-design-gone")));
        };
        let project = self.workspace.active_project_mut().ok_or_else(|| anyhow!(tr!("err-tri-open-project-first")))?;
        let document = &mut project.project.document;
        let mut ids = vec![id];
        let mut commands = vec![Command::Replace { before, after: string }];
        for part in parts {
            let object = part.with_id_and_layer(document.allocate_object_id(), part.layer());
            ids.push(object.id());
            commands.push(Command::AddObject(object));
        }
        if let Some(command) = Command::step(commands) {
            self.execute_edit(command);
        }
        if self.editor.pit_design.as_ref().and_then(PitDesignDraft::object_id) == Some(id) {
            self.editor.pit_design = Some(PitDesignDraft::default());
        }
        userspace_log!("{}", tr!("pit-design-exploded", count = ids.len()));
        self.invalidate_geometry();
        Ok(ids)
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
