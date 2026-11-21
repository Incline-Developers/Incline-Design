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
        Command, Object, ObjectId, PolyVertex, SceneEntityId,
        formats::mesh_data,
        pit_design::{
            self, Base, Design,
            object::{Derived, Outcome, PitObject, base_points, inputs_key},
            settings::{PitDesignSettings, StopMode},
        },
        spatial::TriangleBvh,
        triangulation::{GeometryVersion, TriangulationId},
    },
    ui::{
        dialogs::pit_design::{DraftFacts, NoDesign, PitDesignDraft, PitStage, Preview, PreviewKey},
        state::ActiveTool,
    },
    userspace_log,
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
            ramp_starts: Vec::new(),
        });
    };
    let default_start = points.first().map_or(DVec2::ZERO, |point| point.truncate());
    let spec = settings.spec(default_start);
    let ramp_starts = spec.ramps.iter().map(|ramp| nearest_on_string(&points, ramp.start)).collect();
    let sample = |point: DVec2| ground.and_then(|ground| ground.rl(point));
    let sampler: Option<pit_design::Ground<'_>> = ground.map(|_| &sample as pit_design::Ground<'_>);
    let design = pit_design::design(&base, &spec, sampler, &|| cancel.is_cancelled());
    anyhow::ensure!(!cancel.is_cancelled(), Cancelled);
    Ok(Preview {
        design: design.map_err(NoDesign::Settings),
        base_elevation: base.elevation,
        base_unevenness: base.unevenness,
        ramp_starts,
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

/// Start ramp `index` of `settings` at `start`: a new ramp, cut as the last
/// one is, where `index` is one past the last.
fn place_ramp(settings: &mut PitDesignSettings, index: usize, start: DVec2) {
    let ramps = &mut settings.ramps;
    if index >= ramps.len() {
        let like = ramps.last().cloned().unwrap_or_default();
        ramps.push(crate::model::pit_design::settings::RampSettings { from: None, to: None, ..like });
    }
    let index = index.min(ramps.len() - 1);
    ramps[index].start = Some(start);
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
    /// if it daylights against one in the project: known whether or not it
    /// is loaded, so a design against ground put away unchanged stays
    /// current.
    pub(super) fn pit_ground_version(&self, settings: &PitDesignSettings) -> Option<GeometryVersion> {
        settings
            .ground()
            .and_then(|id| self.triangulations.iter().find(|surface| surface.id == id))
            .map(|surface| surface.geometry)
    }

    /// Whether the ground a design with `settings` daylights against is
    /// loaded, or it needs none: whether it can be worked out now.
    fn pit_ground_loaded(&self, settings: &PitDesignSettings) -> bool {
        settings.ground().is_none_or(|id| {
            self.triangulations
                .iter()
                .any(|surface| surface.id == id && surface.state.loaded && surface.state.deferred.is_none())
        })
    }

    /// Take up the Pit Design tool, from the menu or the context menu: on the
    /// pit design selected, or on a closed polyline selected, which becomes a
    /// pit design in its place; otherwise the tool waits for a click on one.
    pub(crate) fn open_pit_design(&mut self) {
        // Taking the tool up opens what is selected; with it up already,
        // this does.
        if self.editor.active_tool != ActiveTool::PitDesign {
            self.set_active_tool_from_toolbar(ActiveTool::PitDesign);
        } else {
            self.open_selected_pit();
        }
    }

    /// While Pit Design is up, the cursor is the regular one, the mode it
    /// replaced put aside; once the tool is down, that mode is back. Its
    /// strings are drawn level at their RL, which a snap would pull off.
    pub(crate) fn hold_pit_cursor_mode(&mut self) {
        use crate::ui::state::CursorMode;
        let editor = &mut self.editor;
        match (editor.cursor_modes_held(), editor.pit_held_cursor_mode) {
            (true, None) => {
                editor.pit_held_cursor_mode = Some(editor.cursor_mode);
                editor.cursor_mode = CursorMode::Select;
                editor.cursor_snapped = false;
                self.redraw_requested = true;
            }
            (false, Some(mode)) => {
                editor.cursor_mode = mode;
                editor.pit_held_cursor_mode = None;
                self.redraw_requested = true;
            }
            _ => {}
        }
    }

    /// Open the selected pit design, or closed polyline that could become
    /// one, in the tool's panel: the tool is up.
    pub(crate) fn open_selected_pit(&mut self) {
        if let Some(id) = self.selected_pit_or_string() {
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
        // Open, it is drawn in the colours of its parts, which the
        // selection's highlight would cover, and seen through the ground.
        self.editor.selected_handles.remove(&SceneEntityId::Object(id));
        self.editor.pit_design = Some(PitDesignDraft::on(id, pit));
        self.invalidate_geometry();
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
                .unwrap_or_else(|| {
                    // The topography most projects have is the obvious ground
                    // to daylight against, and a pit with ground to meet is
                    // built until it meets it.
                    let topography = self.pit_topography_guess();
                    let stop = if topography.is_some() { StopMode::Daylight } else { StopMode::Benches };
                    PitDesignSettings {
                        topography,
                        stop,
                        ..PitDesignSettings::default()
                    }
                })
        });
        if carried {
            // The ramps are placed once the benches are set.
            settings.ramps.clear();
            let level = points.iter().map(|point| point.z).sum::<f64>() / points.len().max(1) as f64;
            settings.stop_elevation = level + settings.direction.sign() * settings.bench_height * f64::from(settings.benches.max(1));
        }
        let name = name.unwrap_or_else(|| {
            let names: Vec<String> = self
                .workspace
                .active_document()
                .map(|document| document.objects().iter().filter_map(pit_parts).map(|(_, pit)| pit.name.clone()).collect())
                .unwrap_or_default();
            // Named for what it is, numbered past the others: Pit 1, Pit 2.
            let kind = match settings.kind {
                pit_design::Kind::Pit => tr!("pit-design-kind-pit"),
                pit_design::Kind::Dump => tr!("pit-design-kind-dump"),
            };
            (1u32..)
                .map(|number| tr!("pit-design-default-name", kind = kind.clone(), number = number.to_string()))
                .find(|candidate| !names.contains(candidate))
                .unwrap_or_default()
        });
        PitObject::new(name, settings)
    }

    /// The topography a new pit would be designed against: the last pit's,
    /// while it is still loaded, or else the first loaded.
    fn pit_topography_guess(&self) -> Option<TriangulationId> {
        let loaded = |id: TriangulationId| self.triangulations.iter().any(|surface| surface.id == id && surface.state.loaded);
        self.editor
            .pit_last_settings
            .as_ref()
            .and_then(|settings| settings.topography)
            .filter(|id| loaded(*id))
            .or_else(|| self.triangulations.iter().find(|surface| surface.state.loaded).map(|surface| surface.id))
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
            let only_derived = synced.as_ref().is_some_and(|synced| {
                PitObject {
                    derived: now.derived.clone(),
                    evaluation: now.evaluation.clone(),
                    sent: now.sent.clone(),
                    ..synced.clone()
                } == now
            });
            if let Some(draft) = self.editor.pit_design.as_mut() {
                draft.working = match (only_derived, working) {
                    (true, Some(mut working)) => {
                        working.derived = now.derived.clone();
                        working.evaluation = now.evaluation.clone();
                        working.sent = now.sent.clone();
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
        let ground = self.pit_ground_version(&pit.settings());
        let facts = DraftFacts {
            fresh: pit.is_fresh(verts, ground),
            waiting: self.pit_evaluation_waiting(id),
            fields: self.workspace.active_document().map(|document| document.reserve_fields().to_vec()).unwrap_or_default(),
            designs: self.compared_pit_designs(id),
        };
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.facts = facts;
        }
    }

    /// Every pit design in the project as last measured, the one open
    /// first, the rest in the order they were made.
    fn compared_pit_designs(&self, open: ObjectId) -> Vec<crate::ui::dialogs::pit_design::ComparedDesign> {
        let Some(document) = self.workspace.active_document() else {
            return Vec::new();
        };
        let mut designs: Vec<crate::ui::dialogs::pit_design::ComparedDesign> = document
            .objects()
            .iter()
            .filter_map(|object| {
                let (_, pit) = pit_parts(object)?;
                let evaluation = pit.evaluation.as_ref().filter(|evaluation| evaluation.problem.is_none());
                Some(crate::ui::dialogs::pit_design::ComparedDesign {
                    id: object.id(),
                    name: pit.name.clone(),
                    total: evaluation.map(|evaluation| evaluation.total.clone()),
                    shell: evaluation.and_then(|evaluation| evaluation.shell.clone()),
                    current: self.pit_evaluation_waiting(object.id()).is_none(),
                })
            })
            .collect();
        if let Some(index) = designs.iter().position(|design| design.id == open) {
            let this = designs.remove(index);
            designs.insert(0, this);
        }
        designs
    }

    /// Put the panel away, with what it was editing written to the object:
    /// the tool is being put down.
    pub(crate) fn close_pit_design(&mut self) {
        self.flush_pit_design_draft();
        if let Some(settings) = self.editor.pit_design.as_ref().and_then(|draft| draft.working.as_ref()).map(PitObject::settings) {
            self.editor.pit_last_settings = Some(settings);
        }
        self.editor.pending_stroke.clear();
        // Closed, it is drawn as the rest of the scene is again.
        self.editor.pit_design = None;
        self.editor.tool_highlight_id = None;
        self.invalidate_geometry();
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
        let mut settings = working.settings();
        // While a ramp's start is picked, the design is worked out with the
        // ramp where a click would start it, so it follows the pointer.
        let following = draft.picking_ramp.is_some();
        if let Some((index, start)) = draft.ramp_previewed() {
            place_ramp(&mut settings, index, start.truncate());
        }
        let Some((verts, _)) = self.operand_object(id).and_then(pit_parts) else {
            return;
        };
        let verts = verts.to_vec();
        let ground = if settings.daylights() {
            Some(self.pit_design_ground(settings.topography))
        } else {
            None
        };
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
        // Following the pointer, a design under way is let finish: cut off
        // at every move, none would ever be shown. The next starts from
        // wherever the pointer is then.
        if following && draft.building.is_some() {
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
                ramp_starts: Vec::new(),
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
                if !self.pit_ground_loaded(&settings) || pit.is_fresh(verts, ground) {
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

    /// Go to a step of the panel: on to the ramp, its start picked first
    /// if it has none, or back to the benches.
    pub(crate) fn set_pit_stage(&mut self, stage: PitStage) {
        let Some(draft) = self.editor.pit_design.as_mut() else {
            return;
        };
        draft.stage = stage;
        let has_ramp = draft.working.as_ref().is_some_and(|working| !working.settings().ramps.is_empty());
        match stage {
            PitStage::Ramp if !has_ramp => self.begin_pit_ramp_start_pick(0),
            PitStage::Ramp => {}
            PitStage::Benches => self.cancel_pit_ramp_start_pick(),
        }
        self.invalidate_overlay();
    }

    /// Where on the string the ramp would start were the cursor clicked:
    /// the marker that follows it while the start is picked.
    pub(crate) fn update_pit_ramp_hover(&mut self) {
        let Some(id) = self
            .editor
            .pit_design
            .as_ref()
            .filter(|draft| draft.picking_ramp.is_some())
            .and_then(PitDesignDraft::object_id)
        else {
            return;
        };
        let Some(points) = self.operand_object(id).and_then(Object::tessellated_path).map(|(points, _)| points) else {
            return;
        };
        let level = points.iter().map(|point| point.z).sum::<f64>() / points.len().max(1) as f64;
        let hover = self
            .graphics
            .as_ref()
            .and_then(|graphics| graphics.cursor_on_plane(level, crate::app::PICK_THRESHOLD_PX))
            .map(|(cursor, _)| nearest_on_string(&points, cursor.truncate()));
        if let Some(draft) = self.editor.pit_design.as_mut()
            && draft.ramp_hover != hover
        {
            draft.ramp_hover = hover;
            self.invalidate_overlay();
        }
    }

    /// While a pit's string is drawn, the ground under the pointer: the
    /// topography the new pit would meet, read where the pointer points.
    pub(crate) fn update_pit_ground_under_cursor(&mut self) {
        let reading = if self.editor.pit_drawing() {
            self.pit_topography_guess()
                .and_then(|topography| self.pit_design_ground(Some(topography)).ok())
                .map(|ground| {
                    let level = self.editor.z_level;
                    self.graphics
                        .as_ref()
                        .and_then(|graphics| graphics.cursor_on_plane(level, crate::app::PICK_THRESHOLD_PX))
                        .and_then(|(cursor, _)| ground.rl(cursor.truncate()))
                })
        } else {
            None
        };
        if self.editor.pit_ground_under_cursor != reading {
            self.editor.pit_ground_under_cursor = reading;
            self.redraw_requested = true;
        }
    }

    /// The flags of the design the panel shows: its preview's.
    fn pit_flags(&self) -> &[crate::model::pit_design::checks::Flag] {
        self.editor
            .pit_design
            .as_ref()
            .and_then(|draft| draft.preview.as_ref())
            .and_then(|preview| preview.design.as_ref().ok())
            .map_or(&[], |design| design.flags.as_slice())
    }

    /// What the pointer rests on in the panel, for the viewport to pick
    /// out.
    pub(crate) fn set_pit_design_hover(&mut self, hover: crate::ui::dialogs::pit_design::PitHover) {
        if let Some(draft) = self.editor.pit_design.as_mut()
            && draft.hover != hover
        {
            draft.hover = hover;
            self.invalidate_overlay();
        }
    }

    /// Turn the view to flag `index`, with the ground round it.
    pub(crate) fn frame_pit_flag(&mut self, index: usize) {
        let Some(at) = self.pit_flags().get(index).map(|flag| flag.at) else {
            return;
        };
        self.frame_haul(at, at);
    }

    /// Which flag is under the pointer in the viewport, if any.
    pub(crate) fn update_pit_flag_hover(&mut self) {
        let under = (|| {
            let graphics = self.graphics.as_ref()?;
            let cursor = graphics.window_to_viewport_px(self.editor.cursor_screen_px?);
            let (view_proj, screen) = (graphics.view_proj(), graphics.screen_size_pub());
            let reach = f64::from(crate::app::PICK_THRESHOLD_PX * 2.0);
            self.pit_flags()
                .iter()
                .enumerate()
                .filter_map(|(index, flag)| {
                    let at = crate::rendering::pick::world_to_screen(&view_proj, flag.at, screen)?;
                    Some((index, at.distance(glam::DVec2::new(f64::from(cursor.0), f64::from(cursor.1)))))
                })
                .filter(|(_, distance)| *distance <= reach)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(index, _)| index)
        })();
        if let Some(draft) = self.editor.pit_design.as_mut()
            && draft.flag_under_cursor != under
        {
            draft.flag_under_cursor = under;
            self.invalidate_overlay();
            self.redraw_requested = true;
        }
    }

    /// Wait for a click on the string where ramp `index` starts: a new
    /// ramp, where it is one past the last.
    pub(crate) fn begin_pit_ramp_start_pick(&mut self, index: usize) {
        if let Some(draft) = self.editor.pit_design.as_mut()
            && draft.object_id().is_some()
        {
            draft.picking_region = false;
            draft.picking_ramp = Some(index);
            self.invalidate_overlay();
        }
    }

    pub(crate) fn cancel_pit_ramp_start_pick(&mut self) {
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.picking_ramp = None;
            draft.ramp_hover = None;
            draft.dragging_ramp = None;
        }
        self.invalidate_overlay();
    }

    /// Which ramp start marker is under the pointer, if any: a press there
    /// drags it.
    pub(crate) fn update_pit_ramp_marker_hover(&mut self) {
        let under = (|| {
            // A handle under the pointer, or one being dragged, has it.
            let draft = self.editor.pit_design.as_ref().filter(|draft| {
                draft.picking_ramp.is_none()
                    && !draft.picking_region
                    && draft.gizmo_hover.is_none()
                    && draft.widening.is_none()
                    && draft.grading.is_none()
                    && draft.wall_drag.is_none()
            })?;
            let graphics = self.graphics.as_ref()?;
            let cursor = graphics.window_to_viewport_px(self.editor.cursor_screen_px?);
            let cursor = DVec2::new(f64::from(cursor.0), f64::from(cursor.1));
            let (view_proj, screen) = (graphics.view_proj(), graphics.screen_size_pub());
            let reach = f64::from(crate::app::PICK_THRESHOLD_PX * 2.0);
            draft
                .preview
                .as_ref()?
                .ramp_starts
                .iter()
                .enumerate()
                .filter_map(|(index, start)| Some((index, crate::rendering::pick::world_to_screen(&view_proj, *start, screen)?.distance(cursor))))
                .filter(|(_, distance)| *distance <= reach)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(index, _)| index)
        })();
        if let Some(draft) = self.editor.pit_design.as_mut()
            && draft.ramp_marker_under_cursor != under
        {
            draft.ramp_marker_under_cursor = under;
            self.invalidate_overlay();
            self.redraw_requested = true;
        }
    }

    /// Whether a gizmo's handle is being dragged.
    pub(crate) fn dragging_pit_gizmo(&self) -> bool {
        self.editor
            .pit_design
            .as_ref()
            .is_some_and(|draft| draft.widening.is_some() || draft.grading.is_some() || draft.wall_drag.is_some())
    }

    /// Escape while a gizmo's handle is dragged: the drag is let go of and
    /// what it changed put back.
    pub(crate) fn cancel_pit_gizmo_drag(&mut self) {
        if let Some(draft) = self.editor.pit_design.as_mut() {
            if let (Some(from), Some(working)) = (draft.drag_from.take(), draft.working.as_mut()) {
                working.settings = from;
            }
            draft.widening = None;
            draft.grading = None;
            draft.wall_drag = None;
        }
        self.redraw_requested = true;
    }

    /// The part of a ramp's gizmo under the pointer, or, while one is
    /// dragged, the ramp it changes changed to where the pointer is.
    pub(crate) fn update_pit_gizmo(&mut self) {
        use crate::{
            app::commands::drawing::rotate_collar::ring_angle,
            ui::dialogs::pit_design::{GizmoPart, RING_STEPS},
        };
        let Some(cursor) = self.editor.cursor_screen_px else {
            return;
        };
        // In round steps, or with Shift held, in fine ones.
        let step = if self.modifiers.shift_key() { 0.1 } else { 0.5 };
        let snapped = |value: f64| (value / step).round() * step;
        let Some(draft) = self.editor.pit_design.as_mut() else {
            return;
        };
        if let Some((index, from, width)) = draft.widening {
            let Some(gizmo) = draft.gizmos.iter().find(|gizmo| gizmo.ramp == index) else {
                return;
            };
            let moved = (cursor.0 - from.0) * gizmo.across.0 + (cursor.1 - from.1) * gizmo.across.1;
            // A metre wide at least.
            let widened = snapped(width + f64::from(moved / gizmo.pixels_per_metre.max(1.0e-3))).max(1.0);
            if let Some(ramp) = draft.working.as_mut().and_then(|working| working.settings.ramps.get_mut(index))
                && ramp.width != widened
            {
                ramp.width = widened;
                self.redraw_requested = true;
            }
            return;
        }
        if let Some((index, pressed, from)) = draft.grading {
            let Some(angle) = draft.gizmos.iter().find(|gizmo| gizmo.ramp == index).and_then(|gizmo| ring_angle(&gizmo.ring, cursor)) else {
                return;
            };
            // Turned by as much as the pointer has gone round since the
            // press, as a rotate gizmo turns: the short way round.
            let turned = (angle - pressed + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
            let angle = (from + turned * GRADE_RING_GEAR).clamp(0.0, std::f64::consts::FRAC_PI_2 - 1.0e-6);
            // The ring shows the grade exaggerated; the grade is its own.
            let gradient = snapped(angle.tan() / crate::ui::dialogs::pit_design::GRADE_EXAGGERATION * 100.0).clamp(MIN_GRADE, MAX_GRADE);
            if let Some(ramp) = draft.working.as_mut().and_then(|working| working.settings.ramps.get_mut(index))
                && ramp.gradient != gradient
            {
                ramp.gradient = gradient;
                self.redraw_requested = true;
            }
            return;
        }
        if let Some((part, from, value, pressed)) = draft.wall_drag {
            let (Some(gizmo), Some(working)) = (draft.wall_gizmo.as_ref(), draft.working.as_mut()) else {
                return;
            };
            let settings = &mut working.settings;
            let moved = |way: (f32, f32), per_metre: f32| f64::from(((cursor.0 - from.0) * way.0 + (cursor.1 - from.1) * way.1) / per_metre.max(1.0e-3));
            let (field, changed) = match part {
                GizmoPart::BenchHeight => (&mut settings.bench_height, snapped(value + moved(gizmo.up, gizmo.pixels_per_metre_up)).max(1.0)),
                GizmoPart::Berm => (&mut settings.berm_width, snapped(value + moved(gizmo.out, gizmo.pixels_per_metre_out)).max(0.0)),
                GizmoPart::FaceAngle => {
                    let Some(angle) = ring_angle(&gizmo.angle_ring, cursor) else {
                        return;
                    };
                    let turned = (angle - pressed + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
                    (
                        &mut settings.face_angle,
                        snapped(value + turned.to_degrees() * FACE_RING_GEAR).clamp(MIN_FACE_ANGLE, MAX_FACE_ANGLE),
                    )
                }
                _ => return,
            };
            if *field != changed {
                *field = changed;
                self.redraw_requested = true;
            }
            return;
        }
        let near = |point: (f32, f32)| (point.0 - cursor.0).hypot(point.1 - cursor.1) <= GIZMO_REACH;
        let near_chain = |chain: &[(f32, f32)]| {
            chain.windows(2).any(|pair| {
                let (a, b) = (glam::Vec2::from(pair[0]), glam::Vec2::from(pair[1]));
                let point = glam::Vec2::from(cursor);
                let along = (b - a).length_squared();
                let t = if along > 0.0 { ((point - a).dot(b - a) / along).clamp(0.0, 1.0) } else { 0.0 };
                (a + (b - a) * t).distance(point) <= GIZMO_REACH
            })
        };
        let wall = draft.wall_gizmo.as_ref().and_then(|gizmo| {
            if gizmo.ring_fade > 0.0 && (near(gizmo.angle_handle) || near_chain(&gizmo.angle_ring[..=RING_STEPS / 4])) {
                Some(GizmoPart::FaceAngle)
            } else if gizmo.height_fade > 0.0 && near_chain(&[gizmo.height_from, gizmo.height_tip]) {
                Some(GizmoPart::BenchHeight)
            } else if gizmo.berm_fade > 0.0 && near_chain(&[gizmo.berm_from, gizmo.berm_tip]) {
                Some(GizmoPart::Berm)
            } else {
                None
            }
        });
        let hover = wall.or_else(|| {
            draft.gizmos.iter().find_map(|gizmo| {
                // The handles first, then the ring and arrow they sit on.
                if gizmo.ring_fade > 0.0 && near(gizmo.turn_handle) {
                    Some(GizmoPart::Turn(gizmo.ramp))
                } else if gizmo.ring_fade > 0.0 && (near(gizmo.grade_handle) || near_chain(&gizmo.ring[..=RING_STEPS / 2])) {
                    Some(GizmoPart::Grade(gizmo.ramp))
                } else if gizmo.width_fade > 0.0
                    && near_chain(&[
                        // Clear of the foot, which the ramp's start is dragged by.
                        (
                            gizmo.width_from.0 + (gizmo.width_tip.0 - gizmo.width_from.0) * 0.25,
                            gizmo.width_from.1 + (gizmo.width_tip.1 - gizmo.width_from.1) * 0.25,
                        ),
                        gizmo.width_tip,
                    ])
                {
                    Some(GizmoPart::Width(gizmo.ramp))
                } else {
                    None
                }
            })
        });
        if draft.gizmo_hover != hover {
            draft.gizmo_hover = hover;
            self.redraw_requested = true;
        }
    }

    /// A left press in the viewport with a pit open: on a ramp's width
    /// handle, it takes it up to drag; on its arrow, the ramp climbs the
    /// other way; on its start marker, it takes the marker up to drag along
    /// the string.
    pub(crate) fn press_pit_design(&mut self) {
        use crate::ui::dialogs::pit_design::GizmoPart;
        let pressed_at = self.editor.cursor_screen_px;
        if let Some(draft) = self.editor.pit_design.as_mut() {
            if draft.gizmo_hover.is_some() {
                draft.drag_from = draft.working.as_ref().map(PitObject::settings);
            }
            match draft.gizmo_hover {
                Some(GizmoPart::Width(index)) => {
                    let width = draft.working.as_ref().and_then(|working| working.settings.ramps.get(index)).map(|ramp| ramp.width);
                    if let (Some(width), Some(at)) = (width, pressed_at) {
                        draft.widening = Some((index, at, width));
                    }
                    return;
                }
                Some(GizmoPart::Grade(index)) => {
                    let pressed = pressed_at.and_then(|at| {
                        let gizmo = draft.gizmos.iter().find(|gizmo| gizmo.ramp == index)?;
                        crate::app::commands::drawing::rotate_collar::ring_angle(&gizmo.ring, at)
                    });
                    let gradient = draft.working.as_ref().and_then(|working| working.settings.ramps.get(index)).map(|ramp| ramp.gradient);
                    if let (Some(pressed), Some(gradient)) = (pressed, gradient) {
                        draft.grading = Some((index, pressed, (gradient / 100.0 * crate::ui::dialogs::pit_design::GRADE_EXAGGERATION).atan()));
                    }
                    return;
                }
                Some(GizmoPart::Turn(index)) => {
                    if let Some(ramp) = draft.working.as_mut().and_then(|working| working.settings.ramps.get_mut(index)) {
                        ramp.climbs_clockwise = !ramp.climbs_clockwise;
                    }
                    self.redraw_requested = true;
                    return;
                }
                Some(part @ (GizmoPart::BenchHeight | GizmoPart::Berm | GizmoPart::FaceAngle)) => {
                    let (Some(at), Some(settings)) = (pressed_at, draft.working.as_ref().map(|working| &working.settings)) else {
                        return;
                    };
                    let value = match part {
                        GizmoPart::BenchHeight => settings.bench_height,
                        GizmoPart::Berm => settings.berm_width,
                        _ => settings.face_angle,
                    };
                    let pressed = draft
                        .wall_gizmo
                        .as_ref()
                        .and_then(|gizmo| crate::app::commands::drawing::rotate_collar::ring_angle(&gizmo.angle_ring, at))
                        .unwrap_or(0.0);
                    draft.wall_drag = Some((part, at, value, pressed));
                    return;
                }
                None => {}
            }
        }
        let Some(index) = self.editor.pit_design.as_ref().and_then(|draft| draft.ramp_marker_under_cursor) else {
            return;
        };
        self.begin_pit_ramp_start_pick(index);
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.dragging_ramp = pressed_at.or(Some((0.0, 0.0)));
            draft.ramp_marker_under_cursor = None;
        }
        self.update_pit_ramp_hover();
    }

    /// The left button let go: a ramp start dragged is placed where it was
    /// let go; one pressed and let go where it was waits for a click, as
    /// Move does.
    pub(crate) fn release_pit_design(&mut self) {
        if let Some(draft) = self.editor.pit_design.as_mut()
            && (draft.widening.take().is_some() | draft.grading.take().is_some() | draft.wall_drag.take().is_some() | draft.drag_from.take().is_some())
        {
            self.redraw_requested = true;
        }
        let Some(pressed_at) = self.editor.pit_design.as_mut().and_then(|draft| draft.dragging_ramp.take()) else {
            return;
        };
        let moved = self
            .editor
            .cursor_screen_px
            .is_some_and(|(x, y)| (x - pressed_at.0).hypot(y - pressed_at.1) > crate::app::PICK_THRESHOLD_PX);
        if moved {
            self.pick_pit_ramp_start_at_cursor();
        }
        self.invalidate_overlay();
    }

    /// Have the scene draw the preview in place of the open object's own
    /// design, or the object's again, as the preview comes and goes.
    pub(crate) fn note_pit_preview_cover(&mut self) {
        let Some(draft) = self.editor.pit_design.as_mut() else {
            return;
        };
        let covers = draft.covers_object();
        if draft.covering != covers {
            draft.covering = covers;
            self.invalidate_geometry();
        }
    }

    /// A click while picking a ramp's start: it places the ramp there, at
    /// the point of the string nearest the click; a new ramp, cut as the
    /// last one is, where the pick was for one.
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
        let Some((cursor, _)) = graphics.cursor_on_plane(level, crate::app::PICK_THRESHOLD_PX) else {
            return;
        };
        let nearest = nearest_on_string(&points, cursor.truncate());
        let index = self.editor.pit_design.as_ref().and_then(|draft| draft.picking_ramp).unwrap_or(0);
        if let Some(working) = self.editor.pit_design.as_mut().and_then(|draft| draft.working.as_mut()) {
            place_ramp(&mut working.settings, index, nearest.truncate());
        }
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.picking_ramp = None;
            draft.ramp_hover = None;
            draft.dragging_ramp = None;
        }
        self.update_pit_design_preview();
        self.invalidate_overlay();
    }

    /// Wait for a click on a closed polyline, the outline of a new slope
    /// region. Picking the ramp's start gives way to it.
    pub(crate) fn begin_pit_region_pick(&mut self) {
        if let Some(draft) = self.editor.pit_design.as_mut()
            && draft.object_id().is_some()
        {
            draft.picking_ramp = None;
            draft.ramp_hover = None;
            draft.picking_region = true;
            self.invalidate_overlay();
        }
    }

    pub(crate) fn cancel_pit_region_pick(&mut self) {
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.picking_region = false;
        }
        self.editor.tool_highlight_id = None;
        self.editor.viewport_pick_hover_label = None;
        self.invalidate_geometry();
    }

    /// A click while picking a slope region: a closed polyline (or circle)
    /// of the project under the cursor, other than the pit itself, becomes
    /// a new region's outline, cut as the design is until changed.
    pub(crate) fn pick_pit_region_at_cursor(&mut self) {
        let Some(pit) = self.editor.pit_design.as_ref().and_then(PitDesignDraft::object_id) else {
            return;
        };
        let frozen = &self.editor.pick_frozen;
        let active = self.active_project_object_ids();
        let Some((crate::model::SceneEntityId::Object(id), _)) = self
            .graphics
            .as_ref()
            .and_then(|graphics| graphics.pick_at_cursor(crate::app::PICK_THRESHOLD_PX, &[], &self.editor.hidden_handles, frozen, self.editor.xray_enabled))
        else {
            return;
        };
        if id == pit || !active.contains(&id) {
            return;
        }
        let Some(outline) = self.scene_document.get_object(id).and_then(Object::closed_boundary) else {
            return;
        };
        if let Some(working) = self.editor.pit_design.as_mut().and_then(|draft| draft.working.as_mut()) {
            working.settings.bench_rules.regions.push(crate::model::pit_design::walls::Region {
                outline: outline.iter().map(|point| point.truncate()).collect(),
                ..Default::default()
            });
        }
        self.cancel_pit_region_pick();
        self.update_pit_design_preview();
    }

    /// Work a design out here and now, as the preview does in the
    /// background: for the MCP server.
    pub(crate) fn design_pit_now(&self, verts: &[PolyVertex], settings: &PitDesignSettings) -> Result<Preview, NoDesign> {
        let ground = if settings.daylights() {
            Some(self.pit_design_ground(settings.topography)?)
        } else {
            None
        };
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

/// How near a ramp handle the pointer must come to take it, in physical
/// pixels.
const GIZMO_REACH: f32 = 10.0;

/// How much of a sweep round a ring turns what it sets: geared down, as the
/// rings are small on screen and faces and grades are set to fine steps.
const FACE_RING_GEAR: f64 = 0.25;
const GRADE_RING_GEAR: f64 = 0.5;

/// The gentlest and steepest a face's ring turns a bench face to, in
/// degrees.
const MIN_FACE_ANGLE: f64 = 20.0;
const MAX_FACE_ANGLE: f64 = 89.0;

/// The gentlest and steepest grades a ramp's ring turns to, in percent.
const MIN_GRADE: f64 = 0.5;
const MAX_GRADE: f64 = 50.0;
