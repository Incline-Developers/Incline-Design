use glam::DVec3;

use crate::{
    app::PICK_THRESHOLD_PX,
    i18n::tr,
    model::{
        Command, Object, SceneEntityId,
        haulage::{NodeId, NodeRole},
        schedule::{DestinationId, DestinationKind},
    },
    ui::state::{HaulDrag, HaulDragTarget, HaulEdit},
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
            let connected = index
                .access_m(point, network.settings.auto_join_m, max_grade)
                .is_some_and(|a| a <= network.settings.auto_join_m);
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
        self.apply_haul_edit(&mut after, edit)?;
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
    fn apply_haul_edit(&self, after: &mut crate::model::haulage::HaulNetwork, edit: HaulEdit) -> anyhow::Result<()> {
        let Some(active) = self.workspace.active_project() else { return Ok(()) };
        match edit {
            HaulEdit::Many(edits) => {
                for edit in edits {
                    self.apply_haul_edit(after, edit)?;
                }
            }
            HaulEdit::Draw(points) => {
                let name = crate::model::schedule::suggested_name(&tr!("haul-road"), after.roads.iter().map(|r| r.name.clone()));
                after.convert(&[(name, points)])?;
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
                            Some(points)
                        }
                        _ => None,
                    })
                    .collect();
                let mut names: Vec<String> = after.roads.iter().map(|r| r.name.clone()).collect();
                let strings: Vec<_> = strings
                    .into_iter()
                    .map(|points| {
                        let name = crate::model::schedule::suggested_name(&tr!("haul-road"), names.iter().cloned());
                        names.push(name.clone());
                        (name, points)
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
        Ok(())
    }
    pub(crate) fn start_haul_road(&mut self) {
        self.editor.active_workspace = crate::ui::state::Workspace::Planning;
        self.editor.planning_page = crate::ui::state::PlanningPage::Haulage;
        self.refresh_haulage_view();
        self.editor.haul_draw = true;
        self.editor.haul_points.clear();
        self.editor.haul_cursor = None;
        self.editor.active_tool = crate::ui::state::ActiveTool::None;
        self.editor.canvas_context_menu_open = false;
        self.invalidate_overlay();
    }
    pub(crate) fn finish_haul_road(&mut self) {
        self.editor.haul_draw = false;
        self.editor.haul_cursor = None;
        let points = std::mem::take(&mut self.editor.haul_points);
        if points.len() < 2 {
            self.invalidate_overlay();
            return;
        }
        if let Some(project) = self.workspace.active_project() {
            let runtime = project.runtime_id;
            if let Err(error) = self.edit_haulage(runtime, HaulEdit::Draw(points)) {
                crate::userspace_warn!("{error:#}");
            }
        }
        self.invalidate_overlay();
    }
    /// Where a road point under the cursor lands. A road or node within pick
    /// reach wins when `join` is set, so drawn roads connect; then a snap the
    /// user turned on; then the surface under the cursor, so roads drape on
    /// the pit; and otherwise the level of `fallback_z`.
    fn haul_cursor_point(&self, fallback_z: f64, join: bool) -> Option<DVec3> {
        let graphics = self.graphics.as_ref()?;
        if join
            && let Some((_, point)) = graphics
                .pick_at_cursor(PICK_THRESHOLD_PX, &[], &self.editor.hidden_handles, &self.editor.frozen_handles, self.editor.xray_enabled)
                .filter(|(h, _)| matches!(h, SceneEntityId::HaulRoad(_) | SceneEntityId::HaulNode(_)))
        {
            return Some(point);
        }
        if self.editor.cursor_snapped {
            return self.editor.cursor_world;
        }
        graphics
            .pick_triangulation_at_cursor(&self.triangulations, &self.editor.hidden_handles, &self.editor.frozen_handles)
            .map(|(_, point)| point)
            .or_else(|| graphics.cursor_world(fallback_z))
    }
    pub(crate) fn place_haul_point(&mut self) {
        let z = self.editor.haul_points.last().map_or(self.editor.z_level, |p| p.z);
        if let Some(point) = self.haul_cursor_point(z, true)
            && self.editor.haul_points.last().is_none_or(|last| last.distance(point) > 1e-6)
        {
            self.editor.haul_points.push(point);
        }
        self.invalidate_overlay();
    }
    /// A press on a node or shape point may become a drag. Selection still
    /// runs, so a press that never moves selects as before.
    pub(crate) fn begin_haul_drag(&mut self) {
        self.editor.haul_drag = None;
        let Some(graphics) = self.graphics.as_ref() else { return };
        let Some(cursor) = self.editor.cursor_screen_px else { return };
        let Some(document) = self.workspace.active_document() else { return };
        let network = document.haulage();
        let hit = graphics.pick_at_cursor(PICK_THRESHOLD_PX, &[], &self.editor.hidden_handles, &self.editor.frozen_handles, self.editor.xray_enabled);
        let target = match hit {
            Some((SceneEntityId::HaulNode(id), _)) => network.node(id).map(|n| (HaulDragTarget::Node(id), n.pos)),
            Some((SceneEntityId::HaulRoad(id), _)) => {
                let view_proj = graphics.view_proj();
                let reach = self.points_to_px(8.0);
                network.road(id).and_then(|road| {
                    road.verts
                        .iter()
                        .enumerate()
                        .filter_map(|(i, p)| graphics.world_to_window_px(&view_proj, *p).map(|s| (i, *p, (s.0 - cursor.0).hypot(s.1 - cursor.1))))
                        .filter(|(_, _, d)| *d <= reach)
                        .min_by(|a, b| a.2.total_cmp(&b.2))
                        .map(|(i, p, _)| (HaulDragTarget::Shape(id, i), p))
                })
            }
            _ => None,
        };
        if let Some((target, origin)) = target {
            self.editor.haul_drag = Some(HaulDrag {
                target,
                origin,
                start_px: cursor,
                pos: None,
            });
        }
    }
    pub(crate) fn track_haul_cursor(&mut self) {
        if let Some(drag) = self.editor.haul_drag {
            if drag.pos.is_none() {
                let Some(now) = self.editor.cursor_screen_px else { return };
                if (now.0 - drag.start_px.0).hypot(now.1 - drag.start_px.1) < self.points_to_px(4.0) {
                    return;
                }
                // Moving: the press was a drag, not a click or a box.
                self.editor.selection_box_start_px = None;
                self.editor.selection_box_current_px = None;
                self.pending_selection_click = None;
            }
            let pos = self.haul_cursor_point(drag.origin.z, false).or(drag.pos).or(Some(drag.origin));
            if let Some(drag) = self.editor.haul_drag.as_mut() {
                drag.pos = pos;
            }
            self.invalidate_overlay();
        } else if self.editor.haul_draw {
            let z = self.editor.haul_points.last().map_or(self.editor.z_level, |p| p.z);
            self.editor.haul_cursor = self.haul_cursor_point(z, true);
            self.invalidate_overlay();
        }
    }
    pub(crate) fn finish_haul_drag(&mut self) {
        let Some(HaulDrag { target, pos: Some(pos), .. }) = self.editor.haul_drag.take() else {
            return;
        };
        let Some(project) = self.workspace.active_project() else { return };
        let runtime = project.runtime_id;
        let edit = match target {
            HaulDragTarget::Node(id) => HaulEdit::MoveNode(id, pos),
            HaulDragTarget::Shape(id, index) => HaulEdit::MoveShape(id, index, pos),
        };
        if let Err(error) = self.edit_haulage(runtime, edit) {
            crate::userspace_warn!("{error:#}");
        }
        self.invalidate_overlay();
    }
    /// Delete key on the Haulage page. Roads go at once; a destination's node
    /// asks first, because it carries a role. Returns whether it acted.
    pub(crate) fn delete_selected_haulage(&mut self) -> bool {
        let Some(project) = self.workspace.active_project() else { return false };
        let runtime = project.runtime_id;
        let network = project.project.document.haulage();
        let mut edits = Vec::new();
        for handle in &self.editor.selected_handles {
            match *handle {
                SceneEntityId::HaulRoad(id) => edits.push(HaulEdit::DeleteRoad(id)),
                SceneEntityId::HaulNode(id) if network.node(id).is_some_and(|n| n.role.is_some()) => self.editor.haul_delete_node = Some(id),
                SceneEntityId::HaulNode(id) => edits.push(HaulEdit::DeleteNode(id)),
                _ => {}
            }
        }
        if edits.is_empty() {
            return self.editor.haul_delete_node.is_some();
        }
        if let Err(error) = self.edit_haulage(runtime, HaulEdit::Many(edits)) {
            crate::userspace_warn!("{error:#}");
        }
        true
    }
    pub(crate) fn frame_haul(&mut self, min: DVec3, max: DVec3) {
        // Padded so a lone node or short road shows its surroundings.
        let pad = DVec3::splat(30.0).max((max - min) * 0.15);
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.frame_bounds(
                min - pad,
                max + pad,
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
