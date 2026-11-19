//! Pit Design: opening it on a closed string, keeping its preview current in
//! the background as its settings change, picking where the ramp starts, and
//! writing the design into the project: its strings on a new layer and,
//! when asked for, its surface, as one step.

use std::sync::Arc;

use anyhow::Result;
use glam::{DVec2, DVec3};

use crate::{
    app::{
        App,
        jobs::{CancelFlag, Cancelled, JobKey},
    },
    i18n::tr,
    model::{
        Command, FillStyle, Layer, Object, ObjectColor, ObjectId, PolyVertex, SceneEntityId, SectionKind,
        formats::mesh_data,
        pit_design::{self, Base, Design, StringKind},
        spatial::TriangleBvh,
        triangulation::TriangulationId,
    },
    ui::{
        dialogs::pit_design::{NoDesign, PitDesignDraft, PitDesignSettings, Preview, PreviewKey, StopMode},
        state::TriSurfaceCutSide,
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

    /// Open Pit Design on the closed string in the selection, or point the
    /// open dialog at it, keeping its settings. A string picked anew starts
    /// its ramp where the string starts, unless it is the same string.
    pub(crate) fn open_pit_design(&mut self) {
        let base = self.selected_closed_string();
        let mut draft = self.editor.pit_design.take().unwrap_or_else(|| {
            let mut draft = PitDesignDraft::default();
            // A design built down usually stops at a floor RL; the topography
            // most projects have is the obvious ground to daylight against.
            draft.settings.topography = self.triangulations.iter().find(|surface| surface.state.loaded).map(|surface| surface.id);
            draft
        });
        if let Some(base) = base
            && draft.base != Some(base)
        {
            draft.base = Some(base);
            draft.settings.ramp_start = None;
            // A stop RL a bench or so on from the string, as a start.
            if let Some((points, _)) = self.operand_object(base).and_then(Object::tessellated_path)
                && !points.is_empty()
            {
                let level = points.iter().map(|point| point.z).sum::<f64>() / points.len() as f64;
                let sign = draft.settings.direction.sign();
                draft.settings.stop_elevation = level + sign * draft.settings.bench_height * f64::from(draft.settings.benches.max(1));
            }
        }
        draft.picking_ramp_start = false;
        self.editor.pit_design = Some(draft);
        self.update_pit_design_preview();
        self.invalidate_overlay();
    }

    /// Put the dialog away, with its preview and anything building it.
    pub(crate) fn close_pit_design(&mut self) {
        self.editor.pit_design = None;
        self.cancel_jobs(|key| matches!(key, JobKey::PitDesignPreview(_)));
        self.invalidate_overlay();
    }

    /// Start building the design again if its string, its settings or the
    /// document have changed since the preview was built. Called every frame
    /// while the dialog is open: when nothing has changed it costs one
    /// comparison.
    pub(crate) fn update_pit_design_preview(&mut self) {
        let Some(draft) = self.editor.pit_design.as_ref() else {
            return;
        };
        let key = PreviewKey {
            revision: self.scene_document.revision(),
            base: draft.base,
            settings: draft.settings.clone(),
        };
        if draft.built_for.as_ref() == Some(&key) || draft.building.as_ref().is_some_and(|(_, building)| *building == key) {
            return;
        }
        let points = draft
            .base
            .and_then(|id| self.operand_object(id))
            .and_then(Object::tessellated_path)
            .filter(|(_, closed)| *closed)
            .map(|(points, _)| points);
        let settings = draft.settings.clone();
        // Reasons there is nothing to build, settled here and now.
        let ground = if settings.stop == StopMode::Daylight && settings.direction == pit_design::Direction::Up {
            match self.pit_design_ground(settings.topography) {
                Ok(ground) => Some(ground),
                Err(reason) => return self.settle_pit_design_preview(key, reason),
            }
        } else {
            None
        };
        let Some(points) = points else {
            let reason = if draft.base.is_some() { NoDesign::NotClosed } else { NoDesign::NoString };
            return self.settle_pit_design_preview(key, reason);
        };

        self.cancel_jobs(|pending| matches!(pending, JobKey::PitDesignPreview(_)));
        let Some(draft) = self.editor.pit_design.as_mut() else {
            return;
        };
        draft.next_token += 1;
        let token = draft.next_token;
        draft.building = Some((token, key));
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
        })
    }

    /// Wait for a click on the string where the ramp starts.
    pub(crate) fn begin_pit_ramp_start_pick(&mut self) {
        if let Some(draft) = self.editor.pit_design.as_mut()
            && draft.base.is_some()
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

    /// A click while picking the ramp's start: on the string, it sets the
    /// start; anywhere else, the pick waits.
    pub(crate) fn pick_pit_ramp_start_at_cursor(&mut self) {
        let Some(base) = self.editor.pit_design.as_ref().and_then(|draft| draft.base) else {
            return;
        };
        let picked = self.graphics.as_ref().and_then(|graphics| {
            graphics.pick_at_cursor(
                crate::app::PICK_THRESHOLD_PX,
                &[],
                &self.editor.hidden_handles,
                &self.editor.pick_frozen,
                self.editor.xray_enabled,
            )
        });
        match picked {
            Some((SceneEntityId::Object(id), world)) if id == base => {
                if let Some(draft) = self.editor.pit_design.as_mut() {
                    draft.settings.ramp_start = Some(world.truncate());
                    draft.picking_ramp_start = false;
                }
                self.update_pit_design_preview();
                self.invalidate_overlay();
            }
            _ => userspace_warn!("{}", tr!("pit-design-pick-on-string")),
        }
    }

    /// Write the previewed design into the project: its strings on a new
    /// layer and, when asked for, its surface (built on a worker), as one
    /// step that Undo takes back whole.
    pub(crate) fn commit_pit_design(&mut self) {
        let Some(draft) = self.editor.pit_design.as_ref() else {
            return;
        };
        let Some(design) = draft.current_design().cloned() else {
            return;
        };
        let settings = draft.settings.clone();
        if let Some(base) = draft.base
            && !self.activate_project_for_object(base)
        {
            return;
        }
        let trim = if settings.trims() { self.pit_design_ground(settings.topography).ok() } else { None };
        self.close_pit_design();
        if let Err(error) = self.add_pit_design(design, settings.surface, trim) {
            crate::userspace_error!("{error:#}");
        }
    }

    /// Design a pit from `base` with `settings` here and now, as the
    /// dialog's preview does in the background: for the MCP server.
    pub(crate) fn design_pit_now(&self, base: ObjectId, settings: &PitDesignSettings) -> Result<Preview, NoDesign> {
        let points = self
            .operand_object(base)
            .and_then(Object::tessellated_path)
            .filter(|(_, closed)| *closed)
            .map(|(points, _)| points)
            .ok_or(NoDesign::NotClosed)?;
        let ground = if settings.stop == StopMode::Daylight && settings.direction == pit_design::Direction::Up {
            Some(self.pit_design_ground(settings.topography)?)
        } else {
            None
        };
        design_on_worker(&points, settings, ground.as_ref(), &CancelFlag::default()).map_err(|_| NoDesign::NotClosed)
    }

    /// Add a design built by [`Self::design_pit_now`] to the active
    /// project, as Apply does.
    pub(crate) fn add_designed_pit(&mut self, design: Design, settings: &PitDesignSettings) -> Result<()> {
        let trim = if settings.trims() { self.pit_design_ground(settings.topography).ok() } else { None };
        self.add_pit_design(design, settings.surface, trim)
    }

    /// Add a design to the active project: its strings on a new layer, and
    /// its surface when `surface` holds (trimmed to `trim`'s ground when
    /// there is one), in one step.
    fn add_pit_design(&mut self, design: Design, surface: bool, trim: Option<Ground>) -> Result<()> {
        let layer_name = {
            let project = self.workspace.active_project().ok_or_else(|| anyhow::anyhow!(tr!("err-tri-open-project-first")))?;
            let document = &project.project.document;
            crate::model::project::unique_item_name(tr!("pit-design-layer-name"), document.layers().iter().map(|layer| layer.name.as_str()))
        };
        if !surface {
            let command = self.pit_design_layer(&design, layer_name.clone())?;
            self.execute_edit(command);
            self.report_pit_design(&design, &layer_name, None);
            self.invalidate_geometry();
            return Ok(());
        }
        let project = self.workspace.active_project().ok_or_else(|| anyhow::anyhow!(tr!("err-tri-open-project-first")))?;
        let project_key = JobKey::Project {
            runtime_id: project.runtime_id,
            document_revision: project.project.document.revision(),
        };
        let surface_name = layer_name.clone();
        let worker_design = design.clone();
        self.spawn_job_reporting_progress(
            tr!("pit-design-building-surface"),
            vec![project_key],
            move |cancel, progress| {
                let (vertices, faces) = surface_on_worker(&worker_design, trim.as_ref(), cancel, progress)?;
                crate::app::commands::triangulation::session::build_generated_triangulation(surface_name, vertices, faces, crate::ui::state::TriSurfaceType::Surface, |_| {
                    Vec::new()
                })
            },
            move |app, result| match result.and_then(|built| Ok((built, app.pit_design_layer(&design, layer_name.clone())?))) {
                Ok((built, layer)) => {
                    let section = SectionKind::natural_for(crate::model::MemberKind::Triangulation);
                    let ids = app.add_generated_triangulations(vec![crate::app::commands::triangulation::session::new_triangulation(built, section)], vec![layer], true);
                    let surface = ids
                        .first()
                        .and_then(|id| app.triangulations.iter().find(|item| item.id == *id))
                        .map(|item| item.name.clone());
                    app.report_pit_design(&design, &layer_name, surface.as_deref());
                    app.invalidate_geometry();
                }
                Err(error) if crate::app::jobs::is_cancelled(&error) => {}
                Err(error) => crate::userspace_error!("{}", tr!("pit-design-surface-failed", error = format!("{error:#}"))),
            },
        );
        Ok(())
    }

    /// The command adding a design's strings on a new layer of the active
    /// project, which becomes the layer drawn on.
    fn pit_design_layer(&mut self, design: &Design, name: String) -> Result<Command> {
        let project = self.workspace.active_project_mut().ok_or_else(|| anyhow::anyhow!(tr!("err-tri-open-project-first")))?;
        let document = &mut project.project.document;
        let layer_id = document.allocate_layer_id();
        let objects: Vec<Object> = design
            .strings
            .iter()
            .map(|string| Object::Polyline {
                id: document.allocate_object_id(),
                layer: layer_id,
                verts: string.points.iter().copied().map(PolyVertex::straight).collect(),
                closed: string.closed,
                color: ObjectColor::Fixed(string_colour(string.kind)),
                fill: FillStyle::Clear,
                line_weight: 1.0,
            })
            .collect();
        self.editor.active_layer = Some(layer_id);
        Ok(Command::AddLayerSnapshot {
            layer: Layer {
                id: layer_id,
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
        })
    }

    fn report_pit_design(&mut self, design: &Design, layer: &str, surface: Option<&str>) {
        let ended = crate::ui::dialogs::pit_design::end_text(design);
        userspace_log!(
            "{}",
            tr!(
                "pit-design-created",
                benches = design.benches.to_string(),
                strings = design.strings.len().to_string(),
                layer = layer.to_owned(),
                ended = ended
            )
        );
        if let Some(surface) = surface {
            userspace_log!("{}", tr!("pit-design-created-surface", name = surface.to_owned()));
        }
    }
}
