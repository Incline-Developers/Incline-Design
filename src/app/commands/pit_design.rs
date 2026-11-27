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
        dialogs::pit_design::{DraftFacts, NoDesign, PitDesignDraft, Preview, PreviewKey},
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

/// The pit an object holds, if it is a pit design.
fn before_pit(object: &Object) -> Option<&PitObject> {
    pit_parts(object).map(|(_, pit)| pit)
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
/// one is (but for where it runs and turns back, which are its own), where
/// `index` is one past the last. The ramp's place in the list.
fn place_ramp(settings: &mut PitDesignSettings, index: usize, start: DVec2) -> usize {
    let ramps = &mut settings.ramps;
    if index >= ramps.len() {
        let like = ramps.last().cloned().unwrap_or_default();
        ramps.push(crate::model::pit_design::settings::RampSettings {
            from: None,
            to: None,
            switchback: false,
            switchback_every: 0,
            turns_at: Vec::new(),
            ..like
        });
    }
    let index = index.min(ramps.len() - 1);
    ramps[index].start = Some(start);
    index
}

/// The way along the closed string through `points` it runs at `at`, in
/// the order of its points, and whether that order goes round clockwise.
fn string_way_at(points: &[DVec3], at: DVec3) -> Option<(DVec2, bool)> {
    let count = points.len();
    if count < 3 {
        return None;
    }
    let ring: Vec<DVec2> = points.iter().map(|point| point.truncate()).collect();
    let edge = (crate::model::pit_design::ring::nearest(&ring, at.truncate()).0.floor() as usize).min(count - 1);
    let way = (ring[(edge + 1) % count] - ring[edge]).try_normalize()?;
    let twice_area: f64 = (0..count).map(|index| ring[index].perp_dot(ring[(index + 1) % count])).sum();
    Some((way, twice_area < 0.0))
}

/// The way along the string through `points` a ramp climbing clockwise or
/// not leaves its foot at `foot`, in plan: the way it climbs where the
/// design is built up from the string, the way it falls where down.
fn leaving_way(points: &[DVec3], foot: DVec3, climbs_clockwise: bool, direction: pit_design::Direction) -> Option<DVec2> {
    let (along, order_clockwise) = string_way_at(points, foot)?;
    let leaves_clockwise = climbs_clockwise == (direction == pit_design::Direction::Up);
    Some(if leaves_clockwise == order_clockwise { along } else { -along })
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
                    let middle = points.iter().map(|point| point.truncate()).sum::<DVec2>() / points.len().max(1) as f64;
                    let topography = self.pit_topography_guess(Some(middle));
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

    /// The topography a new pit at `at` would be designed against: the last
    /// pit's, while it is still loaded and reaches there; or else the widest
    /// loaded surface that does (a topography spreads far beyond the shells
    /// and designs that sit in it), or failing any, the widest loaded.
    fn pit_topography_guess(&self, at: Option<DVec2>) -> Option<TriangulationId> {
        let reaches = |surface: &&crate::model::triangulation::OpenTriangulation| {
            at.is_none_or(|at| {
                let bounds = surface.mesh.bounds();
                at.x >= bounds.min.x && at.x <= bounds.max.x && at.y >= bounds.min.y && at.y <= bounds.max.y
            })
        };
        let loaded = || self.triangulations.iter().filter(|surface| surface.state.loaded);
        let widest = |surfaces: &mut dyn Iterator<Item = &crate::model::triangulation::OpenTriangulation>| {
            surfaces
                .map(|surface| {
                    let bounds = surface.mesh.bounds();
                    ((bounds.max.x - bounds.min.x) * (bounds.max.y - bounds.min.y), surface.id)
                })
                .max_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id)
        };
        self.editor
            .pit_last_settings
            .as_ref()
            .and_then(|settings| settings.topography)
            .filter(|id| loaded().filter(reaches).any(|surface| surface.id == *id))
            .or_else(|| widest(&mut loaded().filter(reaches)))
            .or_else(|| widest(&mut loaded()))
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
        self.write_pit_design_draft(false);
    }

    /// [`Self::flush_pit_design_draft`], and with `finished`, the design
    /// settled as it is put down: see [`PitObject::settled`]. In the same
    /// undo step as the edits, so undo takes it back to where it was being
    /// designed.
    fn write_pit_design_draft(&mut self, finished: bool) {
        let Some(draft) = self.editor.pit_design.as_ref() else {
            return;
        };
        let (Some(id), Some(working), Some(synced)) = (draft.object_id(), draft.working.clone(), draft.synced.as_ref()) else {
            return;
        };
        if working == *synced && !finished {
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
        // Settled as it is put down, once it is measured; one put down before
        // its measure lands settles when it does.
        if finished
            && self.pit_ready_to_settle(id)
            && let Some(settled) = working.settled(verts)
        {
            working = settled;
        }
        if before_pit(&before) == Some(&working) {
            return;
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
                    ..synced.clone()
                } == now
            });
            if let Some(draft) = self.editor.pit_design.as_mut() {
                draft.working = match (only_derived, working) {
                    (true, Some(mut working)) => {
                        working.derived = now.derived.clone();
                        working.evaluation = now.evaluation.clone();
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
            ground_guess: self.pit_topography_guess(verts.first().map(|vertex| vertex.pos.truncate())),
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
        self.write_pit_design_draft(true);
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
        // While a ramp's start is dragged, the design is worked out with the
        // ramp where letting go would start it, so it follows the pointer;
        // while one is added, with it climbing the way it is dragged.
        let following = draft.placing_ramp() || draft.pointer_held;
        if let Some((index, start, clockwise)) = draft.ramp_previewed() {
            let index = place_ramp(&mut settings, index, start.truncate());
            if let Some(clockwise) = clockwise {
                settings.ramps[index].climbs_clockwise = clockwise;
            }
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
        // Following the pointer, or anything dragged, a design under way is
        // let finish: cut off at every move, none would show until the drag
        // stopped. The next starts from wherever the drag is then.
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
        let checked = self.editor.pit_refresh_checked;
        self.refresh_stale_pit_designs();
        self.settle_designed_pits();
        // Whatever changed the project (an edit, undo or redo, a design
        // worked out), the pits' ramps are the network's roads still.
        if self.editor.pit_refresh_checked != checked {
            self.keep_pit_roads();
        }
    }

    /// Whether pit design `id` is ready to settle: its design worked out for
    /// what it is now and measured, or with nothing it can be measured
    /// against loaded. Not while it is being measured, since that needs the
    /// topography settling lets go of.
    fn pit_ready_to_settle(&self, id: ObjectId) -> bool {
        matches!(self.pit_evaluation_waiting(id), None | Some(crate::model::pit_design::evaluation::Waiting::NotLoaded(_)))
    }

    /// Settle every pit design that is designed and measured, but the one
    /// open in the tool, which settles as it is put down: see
    /// [`PitObject::settled`]. Not an edit of its own, as working a design
    /// out is not; undo takes back the change that made it.
    fn settle_designed_pits(&mut self) {
        let open = self.editor.pit_design.as_ref().and_then(PitDesignDraft::object_id);
        let Some(document) = self.workspace.active_document() else {
            return;
        };
        let settled: Vec<Object> = document
            .objects()
            .iter()
            .filter(|object| Some(object.id()) != open)
            .filter_map(|object| {
                let (verts, pit) = pit_parts(object)?;
                // Settled only once it is designed and measured: a design
                // waiting for its ground keeps it until it has the count to
                // keep. Checked before anything is copied: this runs every
                // frame.
                if !pit.has_links() {
                    return None;
                }
                // With its ground loaded, it is designed against the ground
                // as it is now, and measured, first. With it not, there is
                // nothing to design or measure against: the design it was
                // saved with stands (`settled` keeps only one current for its
                // string and settings).
                if self.pit_ground_loaded(&pit.settings) && (!pit.is_fresh(verts, self.pit_ground_version(&pit.settings)) || !self.pit_ready_to_settle(object.id())) {
                    return None;
                }
                pit.settled(verts).map(|settled| with_pit(object, settled))
            })
            .collect();
        if settled.is_empty() {
            return;
        }
        let Some(document) = self.workspace.active_document_mut() else {
            return;
        };
        for object in settled {
            document.replace_object(object);
        }
        self.invalidate_geometry();
    }

    /// [`Self::refresh_pit_designs`]'s designs worked out again.
    fn refresh_stale_pit_designs(&mut self) {
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
            let level = self.editor.z_level;
            self.graphics
                .as_ref()
                .and_then(|graphics| graphics.cursor_on_plane(level, crate::app::PICK_THRESHOLD_PX))
                .map(|(cursor, _)| cursor.truncate())
                .map(|cursor| {
                    self.pit_topography_guess(Some(cursor))
                        .and_then(|topography| self.pit_design_ground(Some(topography)).ok())
                        .and_then(|ground| ground.rl(cursor))
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

    /// Take up ramp `index`'s start, to drag along the string: the preview
    /// follows the pointer until it is let go.
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
                !draft.placing_ramp() && !draft.picking_region && draft.gizmo_hover.is_none() && draft.widening.is_none() && draft.grading.is_none() && draft.wall_drag.is_none()
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
            .is_some_and(|draft| draft.widening.is_some() || draft.grading.is_some() || draft.wall_drag.is_some() || draft.moving_wall)
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
            draft.moving_wall = false;
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
        let pixels_per_point = self.points_to_px(1.0);
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
        if let Some(drag) = draft.wall_drag {
            let Some(working) = draft.working.as_mut() else {
                return;
            };
            let settings = &mut working.settings;
            // Along the way it was taken up, signed: past the toe, the walls
            // go the other way.
            let pulled = drag.value + f64::from(((cursor.0 - drag.from.0) * drag.way.0 + (cursor.1 - drag.from.1) * drag.way.1) / drag.per_metre.max(1.0e-3));
            let (kind, direction) = match drag.part {
                // Up or down, out or in as it was.
                GizmoPart::BenchHeight if pulled < 0.0 => (drag.kind.other(), drag.direction.other()),
                // Out or in, up or down as it was: a berm pulled back past
                // the crest it starts from.
                GizmoPart::Berm if pulled < -FLIP_BERM => (drag.kind.other(), drag.direction),
                _ => (drag.kind, drag.direction),
            };
            if (settings.kind, settings.direction) != (kind, direction) {
                (settings.kind, settings.direction) = (kind, direction);
                self.redraw_requested = true;
            }
            let (field, changed) = match drag.part {
                GizmoPart::BenchHeight => (&mut settings.bench_height, snapped(pulled.abs()).max(1.0)),
                GizmoPart::Berm => (
                    &mut settings.berm_width,
                    snapped((pulled.abs() - if pulled < -FLIP_BERM { FLIP_BERM } else { 0.0 }).max(0.0)),
                ),
                // The crest pushed in or out across the bench at its height:
                // the face turns about the toe to meet it.
                GizmoPart::FaceAngle => {
                    let angle = if pulled <= 1.0e-6 { MAX_FACE_ANGLE } else { drag.height.atan2(pulled).to_degrees() };
                    (&mut settings.face_angle, snapped(angle).clamp(MIN_FACE_ANGLE, MAX_FACE_ANGLE))
                }
                _ => return,
            };
            if *field != changed {
                *field = changed;
                self.redraw_requested = true;
            }
            return;
        }
        if draft.moving_wall {
            // Its foot follows the pointer along the string.
            let Some(id) = draft.object_id() else {
                return;
            };
            let Some(points) = self.operand_object(id).and_then(Object::tessellated_path).map(|(points, _)| points) else {
                return;
            };
            let level = points.iter().map(|point| point.z).sum::<f64>() / points.len().max(1) as f64;
            if let Some((cursor, _)) = self.graphics.as_ref().and_then(|graphics| graphics.cursor_on_plane(level, crate::app::PICK_THRESHOLD_PX)) {
                let place = nearest_on_string(&points, cursor.truncate()).truncate();
                if let Some(draft) = self.editor.pit_design.as_mut()
                    && draft.wall_place != Some(place)
                {
                    draft.wall_place = Some(place);
                    self.redraw_requested = true;
                }
            }
            return;
        }
        // Every part within reach, the nearest taking it: a ball or spot, a
        // small mark, before the long arm or ring it stands on, by a few
        // pixels, so a spot a road's arrow crosses can still be clicked.
        let to_point = |point: (f32, f32)| (point.0 - cursor.0).hypot(point.1 - cursor.1) - POINT_PREFERENCE;
        let to_chain = |chain: &[(f32, f32)]| {
            chain
                .windows(2)
                .map(|pair| {
                    let (a, b) = (glam::Vec2::from(pair[0]), glam::Vec2::from(pair[1]));
                    let point = glam::Vec2::from(cursor);
                    let along = (b - a).length_squared();
                    let t = if along > 0.0 { ((point - a).dot(b - a) / along).clamp(0.0, 1.0) } else { 0.0 };
                    (a + (b - a) * t).distance(point)
                })
                .fold(f32::INFINITY, f32::min)
        };
        let mut parts: Vec<(f32, GizmoPart)> = Vec::new();
        if let Some(gizmo) = draft.wall_gizmo.as_ref() {
            // Only its handles: its lines are drawn, the string about its
            // foot left free for a ramp.
            parts.push((to_point(gizmo.toe), GizmoPart::WallFoot));
            if gizmo.out_fade > 0.0 && gizmo.handles_apart(pixels_per_point) {
                parts.push((to_point(gizmo.crest), GizmoPart::FaceAngle));
                parts.push((to_point(gizmo.berm_end), GizmoPart::Berm));
            }
            if gizmo.height_fade > 0.0 {
                let middle = ((gizmo.height_from.0 + gizmo.height_tip.0) * 0.5, (gizmo.height_from.1 + gizmo.height_tip.1) * 0.5);
                parts.push((to_point(gizmo.height_tip).min(to_chain(&[middle, gizmo.height_tip])), GizmoPart::BenchHeight));
            }
        }
        for gizmo in &draft.gizmos {
            let [corner, tip, other] = gizmo.climbs_back;
            parts.push((
                to_point(((corner.0 + tip.0 + other.0) / 3.0, (corner.1 + tip.1 + other.1) / 3.0)),
                GizmoPart::Turn(gizmo.ramp),
            ));
            if gizmo.ring_fade > 0.0 {
                parts.push((to_point(gizmo.grade_handle).min(to_chain(&gizmo.ring[..=RING_STEPS / 2])), GizmoPart::Grade(gizmo.ramp)));
            }
            // Long enough on screen to be drawn: at its true width, a road
            // seen from far off is too narrow to take.
            let reach = (gizmo.width_tip.0 - gizmo.width_from.0).hypot(gizmo.width_tip.1 - gizmo.width_from.1);
            if gizmo.width_fade > 0.0 && reach > crate::ui::GIZMO_HEAD_LENGTH * pixels_per_point {
                // Clear of the foot, which the ramp's start is dragged by.
                let from = (
                    gizmo.width_from.0 + (gizmo.width_tip.0 - gizmo.width_from.0) * 0.25,
                    gizmo.width_from.1 + (gizmo.width_tip.1 - gizmo.width_from.1) * 0.25,
                );
                parts.push((to_chain(&[from, gizmo.width_tip]), GizmoPart::Width(gizmo.ramp)));
            }
        }
        for (index, spot) in draft.switch_spots.iter().enumerate() {
            parts.push((to_point(spot.at), GizmoPart::Switch(index)));
        }
        let hover = parts
            .into_iter()
            .filter(|(distance, _)| *distance <= GIZMO_REACH)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, part)| part);
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
                Some(GizmoPart::Switch(index)) => {
                    let Some(spot) = draft.switch_spots.get(index).copied() else {
                        return;
                    };
                    // The turns it makes now, each at the RL it was asked at,
                    // and those asked where none fits, with this one made or
                    // unmade: those, and no others.
                    let mut turns: Vec<f64> = draft
                        .switch_spots
                        .iter()
                        .filter(|other| other.ramp == spot.ramp && (other.turn || other.refused))
                        .map(|other| other.rl)
                        .collect();
                    // A turn asked here but made further on is this spot's
                    // too: a click takes it away rather than asking twice.
                    let moved_from_here = draft
                        .switch_spots
                        .iter()
                        .any(|other| other.ramp == spot.ramp && other.turn && (other.rl - spot.rl).abs() <= 1.0e-6);
                    if spot.turn || spot.refused || moved_from_here {
                        turns.retain(|rl| (rl - spot.rl).abs() > 1.0e-6);
                    } else {
                        turns.push(spot.rl);
                    }
                    if let Some(ramp) = draft.working.as_mut().and_then(|working| working.settings.ramps.get_mut(spot.ramp)) {
                        ramp.switchback = !turns.is_empty();
                        ramp.switchback_every = 0;
                        ramp.turns_at = turns;
                    }
                    self.redraw_requested = true;
                    return;
                }
                Some(part @ (GizmoPart::BenchHeight | GizmoPart::Berm | GizmoPart::FaceAngle)) => {
                    let (Some(at), Some(settings), Some(gizmo)) = (pressed_at, draft.working.as_ref().map(|working| &working.settings), draft.wall_gizmo.as_ref()) else {
                        return;
                    };
                    let height = settings.bench_height;
                    // The face is pushed by its crest, which stands this far out.
                    let value = match part {
                        GizmoPart::BenchHeight => height,
                        GizmoPart::Berm => settings.berm_width,
                        _ => height / settings.face_angle.to_radians().tan(),
                    };
                    let (way, per_metre) = match part {
                        GizmoPart::BenchHeight => (gizmo.up, gizmo.pixels_per_metre_up),
                        _ => (gizmo.out, gizmo.pixels_per_metre_out),
                    };
                    draft.wall_drag = Some(crate::ui::dialogs::pit_design::WallDrag {
                        part,
                        from: at,
                        value,
                        height,
                        way,
                        per_metre,
                        kind: settings.kind,
                        direction: settings.direction,
                    });
                    return;
                }
                Some(GizmoPart::WallFoot) => {
                    draft.moving_wall = true;
                    return;
                }
                None => {}
            }
        }
        if let Some(index) = self.editor.pit_design.as_ref().and_then(|draft| draft.ramp_marker_under_cursor) {
            self.begin_pit_ramp_start_pick(index);
            if let Some(draft) = self.editor.pit_design.as_mut() {
                draft.dragging_ramp = pressed_at.or(Some((0.0, 0.0)));
                draft.ramp_marker_under_cursor = None;
            }
            self.update_pit_ramp_hover();
            return;
        }
        // On the string with nothing else there, a new ramp: it climbs the
        // way the last does until dragged along the string.
        let (Some(foot), Some(pressed)) = (self.editor.pit_design.as_ref().and_then(|draft| draft.string_hover), pressed_at) else {
            return;
        };
        let Some(id) = self.editor.pit_design.as_ref().and_then(PitDesignDraft::object_id) else {
            return;
        };
        let Some((points, _)) = self.operand_object(id).and_then(Object::tessellated_path) else {
            return;
        };
        let Some(settings) = self.editor.pit_design.as_ref().and_then(|draft| draft.working.as_ref()).map(PitObject::settings) else {
            return;
        };
        let clockwise = settings.ramps.last().map_or_else(
            || crate::model::pit_design::settings::RampSettings::default().climbs_clockwise,
            |ramp| ramp.climbs_clockwise,
        );
        let Some(way) = leaving_way(&points, foot, clockwise, settings.direction) else {
            return;
        };
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.adding_ramp = Some(crate::ui::dialogs::pit_design::AddingRamp {
                foot,
                pressed,
                clockwise,
                way,
                dragged: false,
            });
            draft.string_hover = None;
            draft.string_hover_px = None;
        }
        self.update_pit_design_preview();
        self.invalidate_overlay();
    }

    /// While a ramp is added, the way it climbs: as the pointer has been
    /// dragged along the string from its foot.
    pub(crate) fn update_pit_adding_ramp(&mut self) {
        let Some(adding) = self.editor.pit_design.as_ref().and_then(|draft| draft.adding_ramp) else {
            return;
        };
        let (Some(id), Some(cursor_px)) = (self.editor.pit_design.as_ref().and_then(PitDesignDraft::object_id), self.editor.cursor_screen_px) else {
            return;
        };
        let dragged = adding.dragged || (cursor_px.0 - adding.pressed.0).hypot(cursor_px.1 - adding.pressed.1) > crate::app::PICK_THRESHOLD_PX;
        if !dragged {
            return;
        }
        let Some((points, _)) = self.operand_object(id).and_then(Object::tessellated_path) else {
            return;
        };
        let Some(direction) = self
            .editor
            .pit_design
            .as_ref()
            .and_then(|draft| draft.working.as_ref())
            .map(|working| working.settings.direction)
        else {
            return;
        };
        let Some((cursor, _)) = self
            .graphics
            .as_ref()
            .and_then(|graphics| graphics.cursor_on_plane(adding.foot.z, crate::app::PICK_THRESHOLD_PX))
        else {
            return;
        };
        let Some((along, order_clockwise)) = string_way_at(&points, adding.foot) else {
            return;
        };
        // Short of a metre either way, it keeps the way it had.
        let moved = (cursor.truncate() - adding.foot.truncate()).dot(along);
        let clockwise = if moved.abs() < 1.0 {
            adding.clockwise
        } else {
            let leaves_clockwise = (moved > 0.0) == order_clockwise;
            leaves_clockwise == (direction == pit_design::Direction::Up)
        };
        let way = leaving_way(&points, adding.foot, clockwise, direction).unwrap_or(adding.way);
        if let Some(draft) = self.editor.pit_design.as_mut()
            && let Some(held) = draft.adding_ramp.as_mut()
            && (held.clockwise, held.dragged) != (clockwise, true)
        {
            (held.clockwise, held.way, held.dragged) = (clockwise, way, true);
            self.redraw_requested = true;
            self.invalidate_overlay();
        }
    }

    /// A ramp just put down at `foot`: the walls' gizmo, standing where its
    /// gizmo would cover it on screen, is put somewhere clear of it. Only
    /// then: turning the view never moves it.
    fn make_way_for_ramp(&mut self, foot: DVec3) {
        let Some(graphics) = self.graphics.as_ref() else {
            return;
        };
        let (view_proj, screen) = (graphics.view_proj(), graphics.screen_size_pub());
        let on_screen = |point: DVec3| crate::rendering::pick::world_to_screen(&view_proj, point, screen);
        let wanted = crate::rendering::graphics::projections::WALL_GIZMO_CLEAR_POINTS * f64::from(self.points_to_px(1.0));
        let Some(draft) = self.editor.pit_design.as_mut() else {
            return;
        };
        let level = draft.built_for.as_ref().and_then(|key| key.verts.first()).map_or(foot.z, |vertex| vertex.pos.z);
        let covered = draft
            .wall_place
            .and_then(|place| Some(on_screen(place.extend(level))?.distance(on_screen(foot)?)))
            .is_some_and(|apart| apart < wanted);
        if covered {
            draft.wall_place = None;
        }
    }

    /// Escape while a ramp is added: it is not.
    pub(crate) fn cancel_pit_adding_ramp(&mut self) {
        if let Some(draft) = self.editor.pit_design.as_mut() {
            draft.adding_ramp = None;
            draft.adding_arrow = None;
        }
        self.invalidate_overlay();
    }

    /// Whether a ramp is being added.
    pub(crate) fn adding_pit_ramp(&self) -> bool {
        self.editor.pit_design.as_ref().is_some_and(|draft| draft.adding_ramp.is_some())
    }

    /// The place on the pit's string under the pointer, where nothing else
    /// is: where a press would add a ramp.
    pub(crate) fn update_pit_string_hover(&mut self) {
        let hover = (|| {
            let draft = self.editor.pit_design.as_ref().filter(|draft| {
                !draft.placing_ramp()
                    && !draft.picking_region
                    && draft.gizmo_hover.is_none()
                    && draft.field_hover.is_none()
                    && draft.ramp_marker_under_cursor.is_none()
                    && draft.flag_under_cursor.is_none()
                    && draft.widening.is_none()
                    && draft.grading.is_none()
                    && draft.wall_drag.is_none()
            })?;
            let (points, _) = self.operand_object(draft.object_id()?)?.tessellated_path()?;
            let graphics = self.graphics.as_ref()?;
            let cursor = graphics.window_to_viewport_px(self.editor.cursor_screen_px?);
            let cursor = DVec2::new(f64::from(cursor.0), f64::from(cursor.1));
            let (view_proj, screen) = (graphics.view_proj(), graphics.screen_size_pub());
            let on_screen: Vec<Option<DVec2>> = points.iter().map(|point| crate::rendering::pick::world_to_screen(&view_proj, *point, screen)).collect();
            let count = points.len();
            let reach = f64::from(crate::app::PICK_THRESHOLD_PX * 1.5);
            (0..count)
                .filter_map(|index| {
                    let next = (index + 1) % count;
                    let (a, b) = (on_screen[index]?, on_screen[next]?);
                    let (foot, t) = crate::model::kernel::project_onto_segment(cursor, a, b);
                    Some((foot.distance(cursor), points[index].lerp(points[next], t)))
                })
                .filter(|(distance, _)| *distance <= reach)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, at)| at)
        })();
        if let Some(draft) = self.editor.pit_design.as_mut()
            && draft.string_hover != hover
        {
            draft.string_hover = hover;
            self.invalidate_overlay();
            self.redraw_requested = true;
        }
    }

    /// The point of an open bench design's strings an orbit begun at `at`
    /// (window pixels) turns about: the nearest along them, within the reach
    /// an orbit takes any string at. The open design is not in the project
    /// yet, so the orbit's own search for strings does not see it; past
    /// reach, the orbit picks its centre as it always does.
    pub(crate) fn pit_orbit_pivot(&self, at: (f32, f32)) -> Option<DVec3> {
        let draft = self.editor.pit_design.as_ref().filter(|draft| draft.object_id().is_some())?;
        let design = draft.preview.as_ref()?.design.as_ref().ok()?;
        let graphics = self.graphics.as_ref()?;
        let cursor = graphics.window_to_viewport_px(at);
        let cursor = DVec2::new(f64::from(cursor.0), f64::from(cursor.1));
        let (view_proj, screen) = (graphics.view_proj(), graphics.screen_size_pub());
        let on_screen = |point: DVec3| crate::rendering::pick::world_to_screen(&view_proj, point, screen);
        let reach = f64::from(crate::rendering::graphics::camera::ROTATION_CENTRE_LINE_PX);
        // Along the strings, not only at their corners: a bench runs a long
        // way between its points.
        let along = |a: DVec3, b: DVec3| {
            let (screen_a, screen_b) = (on_screen(a)?, on_screen(b)?);
            let t = crate::rendering::pick::closest_t_on_segment(cursor, screen_a, screen_b);
            let distance = (screen_a + (screen_b - screen_a) * t).distance(cursor);
            Some((distance, crate::rendering::pick::perspective_correct_segment_point(&view_proj, a, b, t)))
        };
        let nearest = design
            .strings
            .iter()
            .flat_map(|string| {
                let count = string.points.len();
                let segments = match count {
                    0 | 1 => 0,
                    _ if string.closed => count,
                    _ => count - 1,
                };
                // A lone point is a segment of no length.
                let lone = (count == 1).then(|| (string.points[0], string.points[0]));
                (0..segments).map(move |index| (string.points[index], string.points[(index + 1) % count])).chain(lone)
            })
            .filter_map(|(a, b)| along(a, b))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        nearest.filter(|(distance, _)| *distance <= reach).map(|(_, point)| point)
    }

    /// Delete with the pointer on a ramp's start: the ramp is taken off the
    /// pit. Whether there was one to take.
    pub(crate) fn remove_pit_ramp_under_cursor(&mut self) -> bool {
        let Some(draft) = self.editor.pit_design.as_mut() else {
            return false;
        };
        let Some(index) = draft.ramp_marker_under_cursor.filter(|_| !draft.placing_ramp()) else {
            return false;
        };
        let Some(working) = draft.working.as_mut().filter(|working| index < working.settings.ramps.len()) else {
            return false;
        };
        working.settings.ramps.remove(index);
        draft.ramp_marker_under_cursor = None;
        self.update_pit_design_preview();
        self.invalidate_overlay();
        true
    }

    /// The left button let go: a ramp being added is made, climbing the way
    /// it was dragged; a ramp's start dragged is placed where it was let go
    /// (one let go where it was pressed stays where it was).
    pub(crate) fn release_pit_design(&mut self) {
        if let Some(draft) = self.editor.pit_design.as_mut()
            && (draft.widening.take().is_some()
                | draft.grading.take().is_some()
                | draft.wall_drag.take().is_some()
                | draft.drag_from.take().is_some()
                | std::mem::take(&mut draft.moving_wall))
        {
            self.redraw_requested = true;
        }
        if let Some(draft) = self.editor.pit_design.as_mut()
            && let Some(adding) = draft.adding_ramp.take()
        {
            draft.adding_arrow = None;
            if let Some(working) = draft.working.as_mut() {
                let settings = &mut working.settings;
                let index = place_ramp(settings, settings.ramps.len(), adding.foot.truncate());
                settings.ramps[index].climbs_clockwise = adding.clockwise;
            }
            self.make_way_for_ramp(adding.foot);
            self.update_pit_design_preview();
            self.invalidate_overlay();
            return;
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
        } else {
            self.cancel_pit_ramp_start_pick();
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
        self.make_way_for_ramp(nearest);
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
    #[cfg(not(target_arch = "wasm32"))]
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
    #[cfg(not(target_arch = "wasm32"))]
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

/// How much nearer than it is a ball or spot counts against an arm or ring
/// it stands on or crosses, in physical pixels.
const POINT_PREFERENCE: f32 = 4.0;

/// How much of a sweep round a ring turns what it sets: geared down, as the
/// rings are small on screen and grades are set to fine steps.
const GRADE_RING_GEAR: f64 = 0.5;

/// How far past no berm at all a berm's end is pulled back before the walls
/// step the other way, in metres: a berm taken to nothing stays nothing.
const FLIP_BERM: f64 = 1.0;

/// The gentlest and steepest a face's ring turns a bench face to, in
/// degrees.
const MIN_FACE_ANGLE: f64 = 20.0;
const MAX_FACE_ANGLE: f64 = 89.0;

/// The gentlest and steepest grades a ramp's ring turns to, in percent.
const MIN_GRADE: f64 = 0.5;
const MAX_GRADE: f64 = 50.0;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use glam::DVec3;

    use crate::{
        app::App,
        model::{
            Command, Object, ObjectColor, PolyVertex, SectionKind,
            formats::mesh_data,
            pit_design::settings::{PitDesignSettings, StopMode},
            triangulation::GeneratedTriangulation,
        },
    };

    /// Flat ground at `level` over the square 400 m either side of the origin.
    fn flat_ground(app: &mut App, level: f64) -> crate::model::triangulation::TriangulationId {
        let mesh = mesh_data::Triangulation::from_vertices_and_faces(
            [(-400.0, -400.0), (400.0, -400.0), (400.0, 400.0), (-400.0, 400.0)]
                .map(|(x, y)| mesh_data::Vertex::new(x, y, level))
                .to_vec(),
            vec![[0, 1, 2], [0, 2, 3]],
        )
        .unwrap();
        let built = GeneratedTriangulation {
            name: "Ground".to_owned(),
            spatial: Arc::new(crate::model::spatial::TriangleBvh::build(&mesh)),
            surface_face_order: Arc::new((0..mesh.face_count() as u32).collect()),
            mesh: Arc::new(mesh),
            edges: Vec::new(),
            surface_type: crate::ui::state::TriSurfaceType::Surface,
        };
        let (ids, commands) = app.add_triangulation_commands(vec![crate::app::commands::triangulation::session::new_triangulation(built, SectionKind::Triangulations)]);
        app.execute_edit(Command::Batch(commands));
        ids[0]
    }

    fn pit(app: &App, id: crate::model::ObjectId) -> crate::model::pit_design::object::PitObject {
        let object = app.workspace.active_document().unwrap().get_object(id).unwrap();
        super::pit_parts(object).unwrap().1.clone()
    }

    /// Run the background refresh until `done`.
    fn settle(app: &mut App, done: impl Fn(&App) -> bool) {
        let started = std::time::Instant::now();
        while !done(app) {
            assert!(started.elapsed() < std::time::Duration::from_secs(120), "the pit never got there");
            app.refresh_pit_designs();
            app.refresh_pit_evaluations();
            app.poll_jobs();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Designed against a topography, a pit keeps the bench count it daylit
    /// at and lets go of the topography: the topography gone, it is designed
    /// again to that count.
    #[test]
    fn a_designed_pit_no_longer_needs_its_topography() {
        let mut app = App::default();
        app.start_untitled_project().unwrap();
        let ground = flat_ground(&mut app, 60.0);
        let document = app.workspace.active_document_mut().unwrap();
        let layer = document.add_layer("Pits".to_owned(), None, [1.0; 4], true, 0.0);
        let floor = document.add_object(|id| Object::Polyline {
            id,
            layer,
            verts: [(-60.0, -40.0), (60.0, -40.0), (60.0, 40.0), (-60.0, 40.0)]
                .map(|(x, y)| PolyVertex::straight(DVec3::new(x, y, 0.0)))
                .to_vec(),
            closed: true,
            color: ObjectColor::ByLayer,
            fill: Default::default(),
            line_weight: 1.0,
        });
        let settings = PitDesignSettings {
            stop: StopMode::Daylight,
            topography: Some(ground),
            ramps: Vec::new(),
            ..PitDesignSettings::default()
        };
        app.make_pit_design(floor, None, Some(settings)).unwrap();
        settle(&mut app, |app| pit(app, floor).settings.topography.is_none());

        let settled = pit(&app, floor);
        let design = settled.design().expect("designed");
        assert_eq!(settled.settings.stop, StopMode::Benches);
        assert_eq!(settled.settings.benches, design.benches, "it keeps the count it daylit at");
        assert!(design.benches >= 3, "60 m of 15 m benches: {}", design.benches);
        assert!(
            settled.evaluation.as_ref().is_some_and(|evaluation| evaluation.problem.is_none()),
            "measured before it settled"
        );
        assert_eq!(app.pit_evaluation_waiting(floor), None, "its measure stands");

        // The topography gone, an edit designs it again to the same count.
        app.triangulations.retain(|surface| surface.id != ground);
        let before = app.workspace.active_document().unwrap().get_object(floor).unwrap().clone();
        let mut after = before.clone();
        if let Object::PitDesign { pit, .. } = &mut after {
            pit.settings.berm_width += 1.0;
        }
        app.execute_edit(Command::Replace { before, after });
        settle(&mut app, |app| {
            let object = app.workspace.active_document().unwrap().get_object(floor).unwrap();
            super::pit_parts(object).is_some_and(|(verts, pit)| pit.is_fresh(verts, None))
        });
        assert_eq!(pit(&app, floor).design().map(|design| design.benches), Some(settled.settings.benches));
    }
}
