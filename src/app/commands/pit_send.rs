//! Sending a pit design on to the rest of the project: its ramps to the
//! haul network as roads, and the pit to Planning as a solid, with a
//! surface made from its strings as Create Triangulation would make it.
//!
//! What a design makes elsewhere is kept on it ([`Sent`]), so sending it
//! again replaces what it made before rather than adding to it. Each send is
//! one edit: the network and the design that remembers what it made, undone
//! together.

use anyhow::Result;

use super::pit_design::pit_parts;
use crate::{
    app::{App, jobs::JobKey},
    i18n::tr,
    model::{
        BenchInterval, BenchingPlan, Command, Object, ObjectId, SolidEdit, SolidKind,
        haulage::RoadId,
        pit_design::{Design, Kind, StringKind},
    },
};

/// The benching plan a design's benches make: a range for each run of
/// benches of one height, the topmost reaching a bench higher than the
/// design, for ground above its last ring. Each range keeps the flitches of
/// one of `kept`'s of its bench height, where it has one, as the planner
/// set them.
fn benching_of(design: &Design, kept: &BenchingPlan) -> BenchingPlan {
    let mut levels = design.levels.clone();
    levels.sort_by(f64::total_cmp);
    levels.dedup_by(|a, b| (*a - *b).abs() < 1.0e-6);
    let mut runs: Vec<(f64, f64)> = Vec::new();
    for pair in levels.windows(2) {
        let height = pair[1] - pair[0];
        match runs.last() {
            Some(&(_, last)) if (last - height).abs() < 1.0e-6 => {}
            _ => runs.push((pair[0], height)),
        }
    }
    let top = match (levels.last(), runs.last()) {
        (Some(&top), Some(&(_, height))) => top + height,
        _ => return kept.clone(),
    };
    BenchingPlan {
        top,
        intervals: runs
            .iter()
            .rev()
            .map(|&(base, bench)| {
                let same = kept.intervals.iter().find(|interval| (interval.bench - bench).abs() < 1.0e-6);
                BenchInterval {
                    base,
                    bench,
                    flitch: same.map_or(bench, |interval| interval.flitch),
                    styles: same.map(|interval| interval.styles.clone()).unwrap_or_default(),
                }
            })
            .collect(),
    }
}

impl App<'_> {
    /// The pit design `id`, with its design current: what can be sent on.
    fn sendable_pit(&self, id: ObjectId) -> Result<&Object> {
        let object = self.operand_object(id).ok_or_else(|| anyhow::anyhow!(tr!("pit-design-send-missing")))?;
        let (verts, pit) = pit_parts(object).ok_or_else(|| anyhow::anyhow!(tr!("pit-design-send-missing")))?;
        let ground = self.pit_ground_version(&pit.settings());
        anyhow::ensure!(pit.is_fresh(verts, ground) && pit.design().is_some(), tr!("pit-design-send-not-designed"));
        Ok(object)
    }

    /// Send pit design `id`'s ramps to the haul network: each ramp a road
    /// along its centreline, joined to the network where it comes within
    /// the network's join tolerance of it, in place of the roads it was sent
    /// as before.
    pub(crate) fn send_pit_ramps_to_haulage(&mut self, id: ObjectId) -> Result<usize> {
        let object = self.sendable_pit(id)?.clone();
        let (_, pit) = pit_parts(&object).expect("checked a pit design");
        let design = pit.design().expect("checked designed");
        let centrelines: Vec<_> = design.strings.iter().filter(|string| string.kind == StringKind::RampCentreline).collect();
        anyhow::ensure!(!centrelines.is_empty(), tr!("pit-design-send-no-ramp"));
        let several = centrelines.len() > 1;
        let strings: Vec<(String, Vec<glam::DVec3>)> = centrelines
            .iter()
            .enumerate()
            .map(|(index, string)| {
                let name = if several {
                    tr!("pit-design-haul-road-numbered", design = pit.name.clone(), number = (index + 1).to_string())
                } else {
                    tr!("pit-design-haul-road", design = pit.name.clone())
                };
                (name, string.points.clone())
            })
            .collect();
        let document = self.workspace.active_document().ok_or_else(|| anyhow::anyhow!(tr!("pit-design-send-missing")))?;
        let before = document.haulage().clone();
        let mut after = before.clone();
        for road in &pit.sent.roads {
            if after.road(*road).is_some() {
                after.delete_road(*road);
            }
        }
        let existing: std::collections::HashSet<RoadId> = after.roads.iter().map(|road| road.id).collect();
        after.convert(&strings)?;
        after.validate()?;
        let made: Vec<RoadId> = after.roads.iter().map(|road| road.id).filter(|road| !existing.contains(road)).collect();
        let mut sent = object.clone();
        if let Object::PitDesign { pit, .. } = &mut sent {
            pit.sent.roads = made.clone();
        }
        self.execute_edit(Command::Batch(vec![
            Command::SetHaulNetwork {
                before: Box::new(before),
                after: Box::new(after),
            },
            Command::Replace {
                before: object.clone(),
                after: sent,
            },
        ]));
        self.editor.haul_route = None;
        self.refresh_haulage_view();
        crate::userspace_log!("{}", tr!("pit-design-sent-roads", count = made.len(), design = pit.name.clone()));
        Ok(made.len())
    }

    /// Send pit design `id` to Planning: its surface, made from its strings,
    /// as a project surface, and a Planning solid of it against its
    /// topography, reserved from its block model, benched as it is. Sent
    /// again, the surface is rebuilt in place and the solid brought up to
    /// date. The surface is built in the background.
    pub(crate) fn send_pit_to_planning(&mut self, id: ObjectId) -> Result<()> {
        let object = self.sendable_pit(id)?;
        let (_, pit) = pit_parts(object).expect("checked a pit design");
        anyhow::ensure!(pit.settings.topography.is_some(), tr!("pit-design-send-no-topography"));
        let design = pit.design().expect("checked designed");
        let inputs = pit.derived.as_ref().map(|derived| derived.inputs);
        let name = pit.name.clone();
        let strings: Vec<(Vec<glam::DVec3>, bool)> = design
            .strings
            .iter()
            .filter(|string| !matches!(string.kind, StringKind::RampCentreline | StringKind::Daylight))
            .map(|string| (string.points.clone(), string.closed))
            .collect();
        self.spawn_job(
            tr!("pit-design-sending-planning"),
            vec![JobKey::PitDesignRefresh(id)],
            move |_| {
                let (vertices, faces) = super::triangulation::creation::surface_points_through_strings(strings)?;
                super::triangulation::session::build_generated_triangulation(name, vertices, faces, crate::ui::state::TriSurfaceType::Surface, |_| Vec::new())
            },
            move |app, result: Result<crate::model::triangulation::GeneratedTriangulation>| match result {
                Ok(built) => {
                    if let Err(error) = app.planning_from_surface(id, inputs, built) {
                        crate::userspace_error!("{error:#}");
                    }
                }
                Err(error) => crate::userspace_error!("{}", tr!("pit-design-send-planning-failed", reason = format!("{error:#}"))),
            },
        );
        Ok(())
    }

    /// The second half of [`Self::send_pit_to_planning`], once its surface
    /// is built: onto the design as it was when sent.
    fn planning_from_surface(&mut self, id: ObjectId, inputs: Option<u64>, built: crate::model::triangulation::GeneratedTriangulation) -> Result<()> {
        let object = self.sendable_pit(id)?.clone();
        let (_, pit) = pit_parts(&object).expect("checked a pit design");
        anyhow::ensure!(pit.derived.as_ref().map(|derived| derived.inputs) == inputs, tr!("pit-design-send-changed"));
        let design = pit.design().expect("checked designed");
        let settings = pit.settings();
        let mut commands = Vec::new();
        // The surface: rebuilt where it was sent before and is still here,
        // added where not.
        let kept = pit.sent.surface.filter(|surface| self.triangulations.iter().any(|item| item.id == *surface));
        let surface = match kept {
            Some(surface) => {
                commands.extend(self.replace_triangulation_command(surface, built));
                surface
            }
            None => {
                let section = crate::model::SectionKind::natural_for(crate::model::MemberKind::Triangulation);
                let (ids, added) = self.add_triangulation_commands(vec![super::triangulation::session::new_triangulation(built, section)]);
                commands.extend(added);
                *ids.first().ok_or_else(|| anyhow::anyhow!(tr!("pit-design-send-planning-failed", reason = String::new())))?
            }
        };
        let document = self.workspace.active_document().ok_or_else(|| anyhow::anyhow!(tr!("pit-design-send-missing")))?;
        let kind = match settings.kind {
            Kind::Pit => SolidKind::Pit,
            Kind::Dump => SolidKind::Dump,
        };
        // The solid: brought up to date where it was sent before, added
        // where not. Adding a solid is no edit of its own in Planning, so it
        // is not here either.
        let existing = pit.sent.solid.and_then(|solid| document.solids().iter().find(|candidate| candidate.id == solid));
        let solid = match existing {
            Some(solid) => {
                for edit in [
                    SolidEdit::Kind(kind),
                    SolidEdit::Surface(Some(surface)),
                    SolidEdit::Topography(settings.topography),
                    SolidEdit::BlockModel(settings.evaluation.block_model),
                    SolidEdit::Benching(benching_of(design, &solid.benching)),
                ] {
                    if let Some(before) = document.solid_edit_undo(solid.id, &edit).filter(|before| *before != edit) {
                        commands.push(Command::EditSolid {
                            solid: solid.id,
                            before: Box::new(before),
                            after: Box::new(edit),
                        });
                    }
                }
                solid.id
            }
            None => {
                let benching = benching_of(design, &BenchingPlan::default());
                let name = crate::model::project::unique_item_name(pit.name.clone(), document.solids().iter().map(|solid| solid.name.as_str()));
                let document = self.workspace.active_document_mut().ok_or_else(|| anyhow::anyhow!(tr!("pit-design-send-missing")))?;
                let solid = document.add_solid(name, kind, Some(surface), settings.topography, settings.evaluation.block_model);
                document.update_solid(solid, SolidEdit::Benching(benching));
                if let Some(block_model) = settings.evaluation.block_model {
                    self.set_reserve_model_included(block_model, true);
                }
                self.touch_active_project_content();
                solid
            }
        };
        let mut sent = object.clone();
        if let Object::PitDesign { pit, .. } = &mut sent {
            pit.sent.surface = Some(surface);
            pit.sent.solid = Some(solid);
        }
        commands.push(Command::Replace {
            before: object.clone(),
            after: sent,
        });
        if let Some(command) = Command::step(commands) {
            self.execute_job_edit(command);
        }
        self.editor.planning_selected_solid = Some(solid);
        self.invalidate_geometry();
        crate::userspace_log!("{}", tr!("pit-design-sent-planning", design = pit.name.clone()));
        Ok(())
    }
}
