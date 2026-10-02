use glam::DVec3;

use crate::{
    i18n::tr,
    model::{
        Command, Object, SceneEntityId,
        haulage::{NodeId, NodeRole},
        schedule::{DestinationId, DestinationKind},
    },
    ui::state::HaulEdit,
};

impl crate::app::App<'_> {
    pub(crate) fn haul_destination_points(&self, document: &crate::model::Document) -> std::collections::BTreeMap<DestinationId, DVec3> {
        let mut points = std::collections::BTreeMap::new();
        for solid in document.solids().iter().filter(|s| s.kind != crate::model::SolidKind::Pit) {
            let Some(surface) = solid.surface.and_then(|id| self.triangulations.iter().find(|t| t.id == id)) else {
                continue;
            };
            let mut area = 0.0;
            let mut center = DVec3::ZERO;
            for triangle in surface.mesh.triangles() {
                let p = triangle.vertices.map(|v| DVec3::new(v.x, v.y, v.z));
                let weight = (p[1] - p[0]).truncate().perp_dot((p[2] - p[0]).truncate()).abs();
                center += (p[0] + p[1] + p[2]) / 3.0 * weight;
                area += weight;
            }
            if area <= 0.0 {
                continue;
            }
            center /= area;
            let top = surface.mesh.vertices().iter().map(|v| v.z).fold(f64::NEG_INFINITY, f64::max);
            center.z = surface
                .mesh
                .triangles()
                .filter_map(|triangle| {
                    let p = triangle.vertices.map(|v| DVec3::new(v.x, v.y, v.z));
                    let a = p[0].truncate();
                    let b = p[1].truncate();
                    let c = p[2].truncate();
                    let q = center.truncate();
                    let denominator = (b - a).perp_dot(c - a);
                    if denominator.abs() < 1e-9 {
                        return None;
                    }
                    let v = (q - a).perp_dot(c - a) / denominator;
                    let w = (b - a).perp_dot(q - a) / denominator;
                    (v >= -1e-9 && w >= -1e-9 && v + w <= 1.0 + 1e-9).then_some(p[0].z * (1.0 - v - w) + p[1].z * v + p[2].z * w)
                })
                .reduce(f64::max)
                .unwrap_or(top);
            points.insert(DestinationId::Solid(solid.id), center);
        }
        points
    }
    pub(crate) fn refresh_haulage_view(&mut self) {
        if self.editor.planning_page != crate::ui::state::PlanningPage::Haulage {
            return;
        }
        self.editor.haul_view_revision = self.editor.haul_view_revision.wrapping_add(1);
        self.editor.haul_blocks.clear();
        if let Some(document) = self.workspace.active_document() {
            let destinations: Vec<_> = document
                .schedule()
                .routing()
                .standalone
                .iter()
                .map(|d| DestinationId::Standalone(d.id))
                .chain(
                    document
                        .solids()
                        .iter()
                        .filter(|s| s.kind != crate::model::SolidKind::Pit)
                        .map(|s| DestinationId::Solid(s.id)),
                )
                .collect();
            let grade = document.schedule().trucks().classes.iter().map(|c| c.maximum_grade).reduce(f64::min).unwrap_or(0.1);
            self.editor.haul_issues = document.haulage().issues(&destinations, grade);
        }
        let Ok(snapshot) = self.planning_snapshot() else { return };
        let Some(document) = self.workspace.active_document() else { return };
        let network = document.haulage();
        let index = crate::model::haulage::network::RoadIndex::new(network);
        let max_grade = document.schedule().trucks().classes.iter().map(|c| c.maximum_grade).reduce(f64::min).unwrap_or(0.1);
        for block in snapshot.blocks {
            let point = DVec3::new(block.anchor[0], block.anchor[1], block.flitch.base);
            let connected = index.candidates(point, network.settings.auto_join_m).iter().any(|(_, _, p)| {
                let direct = point.distance(*p);
                direct <= network.settings.auto_join_m && (p.z - point.z).abs() / max_grade <= direct + 1e-6
            });
            self.editor.haul_blocks.push((
                crate::ui::state::BlastOutline {
                    solid: block.solid,
                    bench_base: block.flitch.base,
                    plane: block.flitch.base,
                    name: format!("{} · {:.0} · {}", block.solid_name, block.flitch.base, block.name),
                    anchor: block.anchor,
                    area: block.plan_area,
                    rings: block
                        .ground
                        .iter()
                        .map(|ring| ring.iter().map(|p| DVec3::new(p.x, p.y, block.flitch.base)).collect())
                        .collect(),
                },
                connected,
            ));
        }
        self.invalidate_overlay();
    }
    pub(crate) fn edit_haulage(&mut self, project: u32, edit: HaulEdit) -> anyhow::Result<()> {
        self.editor.canvas_context_menu_open = false;
        let Some(active) = self.workspace.active_project() else { return Ok(()) };
        anyhow::ensure!(active.runtime_id == project, "{}", tr!("schedule-stale-edit"));
        let before = active.project.document.haulage().clone();
        let mut after = before.clone();
        match edit {
            HaulEdit::Draw(points) => {
                after.convert(&[(tr!("haul-road"), points)])?;
            }
            HaulEdit::ConvertSelection => {
                let strings: Vec<_> = active
                    .project
                    .document
                    .objects()
                    .iter()
                    .filter(|o| self.editor.selected_handles.contains(&SceneEntityId::Object(o.id())))
                    .filter_map(|o| match o {
                        Object::Polyline { verts, closed, .. } => {
                            let mut points = crate::model::geometry::tessellate_polyline_bulges(verts, *closed);
                            if *closed && let Some(&first) = points.first() {
                                points.push(first);
                            }
                            Some((tr!("haul-road"), points))
                        }
                        _ => None,
                    })
                    .collect();
                after.convert(&strings)?;
            }
            HaulEdit::MoveNode(id, pos) => after.move_node(id, pos)?,
            HaulEdit::MoveShape(id, index, pos) => {
                anyhow::ensure!(pos.is_finite(), "{}", tr!("haul-invalid-position"));
                *after
                    .roads
                    .iter_mut()
                    .find(|r| r.id == id)
                    .and_then(|r| r.verts.get_mut(index))
                    .ok_or_else(|| anyhow::anyhow!(tr!("haul-missing-road")))? = pos;
            }
            HaulEdit::DeleteRoad(id) => after.delete_road(id),
            HaulEdit::DeleteNode(id) => after.delete_node(id),
            HaulEdit::Join(keep, remove) => after.join(keep, remove)?,
            HaulEdit::Split(id, pos) => {
                let r = after.road(id).ok_or_else(|| anyhow::anyhow!(tr!("haul-missing-road")))?;
                let (segment, point) = after
                    .points(r)
                    .windows(2)
                    .enumerate()
                    .map(|(i, p)| (i, crate::model::haulage::network::project(pos, p[0], p[1])))
                    .min_by(|a, b| a.1.distance_squared(pos).total_cmp(&b.1.distance_squared(pos)))
                    .ok_or_else(|| anyhow::anyhow!(tr!("haul-missing-road")))?;
                after.split(id, segment, point)?;
            }
            HaulEdit::Role(id, role) => after.set_role(id, role)?,
            HaulEdit::RoadProperties(ids, name, speed) => {
                for road in &mut after.roads {
                    if ids.contains(&road.id) {
                        if let Some(name) = &name {
                            road.name = name.trim().to_owned();
                        }
                        road.speed_limit_kph = speed;
                    }
                }
            }
            HaulEdit::Settings(settings) => after.settings = settings,
            HaulEdit::Fixed(destination, fixed) => {
                after.fixed_destinations.retain(|id| *id != destination);
                if fixed {
                    after.fixed_destinations.push(destination);
                }
            }
            HaulEdit::Pin(destination, pos) => {
                let id = after.join_point(pos)?;
                after.set_role(id, Some(NodeRole::Dump(destination)))?;
                after.fixed_destinations.retain(|id| *id != destination);
            }
        }
        after.validate()?;
        if before != after {
            self.execute_edit(Command::SetHaulNetwork {
                before: Box::new(before),
                after: Box::new(after),
            });
            self.editor.haul_route = None;
            self.refresh_haulage_view();
        }
        Ok(())
    }
    pub(crate) fn start_haul_road(&mut self) {
        self.editor.active_workspace = crate::ui::state::Workspace::Planning;
        self.editor.planning_page = crate::ui::state::PlanningPage::Haulage;
        self.refresh_haulage_view();
        self.editor.haul_draw = true;
        self.editor.haul_points.clear();
        self.editor.active_tool = crate::ui::state::ActiveTool::None;
        self.editor.canvas_context_menu_open = false;
        self.invalidate_overlay();
    }
    pub(crate) fn finish_haul_road(&mut self) {
        self.editor.haul_draw = false;
        let points = std::mem::take(&mut self.editor.haul_points);
        if let Some(project) = self.workspace.active_project() {
            let runtime = project.runtime_id;
            if let Err(error) = self.edit_haulage(runtime, HaulEdit::Draw(points)) {
                crate::userspace_warn!("{error:#}");
            }
        }
        self.invalidate_overlay();
    }
    pub(crate) fn frame_haul_point(&mut self, point: DVec3) {
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.frame_bounds(
                point - DVec3::splat(30.0),
                point + DVec3::splat(30.0),
                &self.scene_document,
                &self.triangulations,
                &self.block_models,
                &self.drill_holes,
                &self.point_clouds,
                &self.editor.hidden_handles,
            );
        }
        self.redraw_requested = true;
    }
    pub(crate) fn new_haul_destination(&mut self, node: NodeId, kind: DestinationKind) -> anyhow::Result<()> {
        self.editor.canvas_context_menu_open = false;
        let Some(project) = self.workspace.active_project() else { return Ok(()) };
        let before = project.project.document.schedule().clone();
        let mut after = before.clone();
        let network = project.project.document.haulage().clone();
        anyhow::ensure!(network.node(node).is_some(), "{}", tr!("haul-invalid-position"));
        let name = crate::model::schedule::suggested_name(&kind.label(), after.routing().standalone.iter().map(|d| d.name.clone()));
        let added = after.routing_mut().add_standalone(&name, kind).map_err(|e| anyhow::anyhow!(e.message()))?;
        let destination = DestinationId::Standalone(added);
        let mut next = network.clone();
        next.set_role(node, Some(NodeRole::Dump(destination)))?;
        self.execute_edit(Command::Batch(vec![
            Command::SetSchedulePlan {
                before: Box::new(before),
                after: Box::new(after),
            },
            Command::SetHaulNetwork {
                before: Box::new(network),
                after: Box::new(next),
            },
        ]));
        self.editor.schedule_selected_destination = Some(destination);
        self.editor.planning_page = crate::ui::state::PlanningPage::Schedule;
        self.editor.schedule_subpage = crate::ui::state::PlanningSubpage::Setup;
        self.editor.schedule_setup_step = match kind {
            DestinationKind::Stockpile => crate::ui::state::ScheduleStep::Stockpiles,
            DestinationKind::Dump => crate::ui::state::ScheduleStep::Dumps,
            DestinationKind::Crusher => crate::ui::state::ScheduleStep::Crushers,
        };
        Ok(())
    }
}
