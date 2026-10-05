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

/// The id of [`crate::app::App::haul_block_surface`]: generated geometry
/// like the solid preview's, numbered below the ids those take.
const HAUL_BLOCK_SURFACE_ID: crate::model::triangulation::TriangulationId = crate::model::triangulation::TriangulationId(u64::MAX - 1_000_000);

impl crate::app::App<'_> {
    pub(crate) fn haul_destination_points(&self, document: &crate::model::Document) -> std::collections::BTreeMap<DestinationId, DVec3> {
        let mut points = std::collections::BTreeMap::new();
        let Some(runtime) = self.workspace.active_project().map(|project| project.runtime_id) else {
            return points;
        };
        for solid in document.solids().iter().filter(|s| s.kind != crate::model::SolidKind::Pit) {
            // Whether or not its surface is loaded: hiding a layer must not
            // move a dump off the roads.
            let Some(mesh) = self.solid_design_surface(runtime, solid) else {
                continue;
            };
            let mut area = 0.0;
            let mut center = DVec3::ZERO;
            for triangle in mesh.triangles() {
                let p = triangle.vertices.map(|v| DVec3::new(v.x, v.y, v.z));
                let weight = (p[1] - p[0]).truncate().perp_dot((p[2] - p[0]).truncate()).abs();
                center += (p[0] + p[1] + p[2]) / 3.0 * weight;
                area += weight;
            }
            if area <= 0.0 {
                continue;
            }
            center /= area;
            let top = mesh.vertices().iter().map(|v| v.z).fold(f64::NEG_INFINITY, f64::max);
            center.z = mesh
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
    /// The loaded path of every route the shown schedule hauls by, for
    /// Animate to draw its flows along. Rebuilt only when the schedule or the
    /// project changes: one search per destination and truck class serves
    /// every block, as it does in capture.
    pub(crate) fn sync_animation_routes(&mut self) {
        let Some(schedule) = self.editor.schedule_result.clone() else {
            self.editor.animation_routes.clear();
            self.editor.animation_routes_key = None;
            return;
        };
        let Some(document) = self.workspace.active_document() else { return };
        let key = (std::sync::Arc::as_ptr(&schedule) as usize, document.revision());
        if self.editor.animation_routes_key == Some(key) {
            return;
        }
        self.editor.animation_routes_key = Some(key);
        let mut routes = std::collections::HashMap::new();
        let network = document.haulage();
        if !network.roads.is_empty() {
            let centroids = self.haul_destination_points(document);
            let blocks: std::collections::HashMap<_, _> = self
                .planning_snapshot()
                .map(|snapshot| {
                    snapshot
                        .blocks
                        .iter()
                        .map(|b| {
                            let point = DVec3::new(b.anchor[0], b.anchor[1], b.flitch.base);
                            (b.id, (point, network.block_link(b.solid, b.flitch.base, &b.ground).to_vec()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let index = crate::model::haulage::network::RoadIndex::new(network);
            let trucks = document.schedule().trucks();
            let mut searches = std::collections::HashMap::new();
            let wanted: std::collections::HashSet<_> = schedule.deliveries.iter().map(|d| (d.destination, d.truck, d.source)).collect();
            for (destination, truck, source) in wanted {
                let (Some(class), Some(target)) = (trucks.class(truck), network.destination_point(destination, false, centroids.get(&destination).copied())) else {
                    continue;
                };
                let (from, link, bench) = match source {
                    crate::model::schedule::result::WorkSource::Block(id) => match blocks.get(&id) {
                        Some((point, link)) => (*point, link.as_slice(), true),
                        None => continue,
                    },
                    crate::model::schedule::result::WorkSource::Stockpile(pile) => match network.destination_point(pile, true, centroids.get(&pile).copied()) {
                        Some(point) => (point, &[][..], false),
                        None => continue,
                    },
                };
                let search = searches
                    .entry((destination, truck))
                    .or_insert_with(|| crate::model::haulage::routing::DestinationSearch::new(network, &index, class, target));
                if let Some(route) = search.as_ref().and_then(|s| s.route(&index, from, link, bench, 0.0, 0.0, None))
                    && route.loaded_path.len() >= 2
                {
                    routes.insert((source, destination, truck), route.loaded_path);
                }
            }
        }
        self.editor.animation_routes = routes;
    }
    pub(crate) fn refresh_haulage_view(&mut self) {
        if self.editor.planning_page != crate::ui::state::PlanningPage::Haulage {
            return;
        }
        self.editor.haul_view_revision = self.editor.haul_view_revision.wrapping_add(1);
        self.editor.haul_blocks.clear();
        self.editor.haul_issues.clear();
        let snapshot = self.planning_snapshot().ok();
        let Some(document) = self.workspace.active_document() else { return };
        let network = document.haulage();
        let index = crate::model::haulage::network::RoadIndex::new(network);
        let max_grade = max_grade(document.schedule().trucks());
        let reach = network.settings.auto_join_m;
        for block in snapshot.iter().flat_map(|s| &s.blocks) {
            let point = DVec3::new(block.anchor[0], block.anchor[1], block.flitch.base);
            let links: Vec<_> = network
                .block_link(block.solid, block.flitch.base, &block.ground)
                .iter()
                .filter_map(|id| index.node_join(*id).map(|join| (*id, join.2)))
                .collect();
            let join = if links.is_empty() {
                index.joins(point, &[], max_grade).first().map(|j| j.2)
            } else {
                None
            };
            let access_m = links
                .iter()
                .map(|l| l.1)
                .chain(join)
                .map(|j| crate::model::haulage::network::access_length(point, j, max_grade))
                .reduce(f64::min)
                .unwrap_or(f64::INFINITY);
            let (_, _, _, _, area) = crate::app::commands::schedule_readiness::block_labels(document, block);
            self.editor.haul_blocks.push(crate::ui::state::HaulBlock {
                id: block.id,
                solid: block.solid,
                bench: block.bench,
                flitch: block.flitch,
                blast: block.blast,
                name: crate::app::commands::schedule_readiness::block_path(document, block),
                anchor: block.anchor,
                face: block.ground.clone(),
                rings: block
                    .ground
                    .iter()
                    .map(|ring| ring.iter().map(|p| DVec3::new(p.x, p.y, block.flitch.base)).collect())
                    .collect(),
                dug: document.solid(block.solid).is_some_and(|s| s.kind == crate::model::SolidKind::Pit),
                area,
                connected: !links.is_empty() || access_m <= reach,
                links,
                join,
                access_m,
            });
        }
        let blocks = &self.editor.haul_blocks;
        self.editor.haul_selected_blocks.retain(|id| blocks.iter().any(|b| b.id == *id));
        let destinations = haul_destinations(document);
        let anchors: Vec<_> = self.editor.haul_blocks.iter().map(crate::ui::state::HaulBlock::point).collect();
        let mut issues = network.issues(&destinations, max_grade, &anchors);
        // Dug blocks no road reaches, one issue per blast: a block each would
        // bury the network's own issues.
        if !network.roads.is_empty() {
            let mut far: std::collections::BTreeMap<&str, Vec<&crate::ui::state::HaulBlock>> = std::collections::BTreeMap::new();
            for block in self.editor.haul_blocks.iter().filter(|b| b.dug && !b.connected) {
                far.entry(&block.area).or_default().push(block);
            }
            issues.extend(far.into_iter().map(|(area, blocks)| crate::model::haulage::network::NetworkIssue {
                kind: crate::model::haulage::network::IssueKind::OutOfReach,
                pos: blocks[0].point(),
                road: None,
                node: None,
                blocks: blocks.iter().map(|b| b.id).collect(),
                area: area.to_owned(),
            }));
        }
        self.editor.haul_issues = issues;
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
                // Every clicked point is a node, so blocks can join the road
                // anywhere it was drawn without promoting points first.
                let name = crate::model::schedule::suggested_name(&tr!("haul-road"), after.roads.iter().map(|r| r.name.clone()));
                let segments: Vec<_> = points.windows(2).map(|pair| (name.clone(), pair.to_vec())).collect();
                after.convert(&segments)?;
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
            HaulEdit::RemoveNode(id) => after.merge_at(id)?,
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
            HaulEdit::Pin(role, pos) => {
                let id = after.join_point(pos)?;
                after.set_role(id, Some(role))?;
            }
            HaulEdit::LinkBlocks { blocks, at } => {
                let nodes = at.into_iter().map(|p| after.join_point(p)).collect::<anyhow::Result<Vec<_>>>()?;
                for block in blocks {
                    after.set_block_link(block.solid, block.flitch_base, &block.face, block.probe, nodes.clone())?;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn start_haul_road(&mut self) {
        self.editor.active_workspace = crate::ui::state::Workspace::Planning;
        self.editor.planning_page = crate::ui::state::PlanningPage::Haulage;
        self.editor.haulage_subpage = crate::ui::state::PlanningSubpage::Layout;
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
    /// Where a road point under the cursor lands. With a snap mode on, only
    /// what it snapped to, so a cursor over nothing it can take - a locked
    /// surface, empty ground - lands nowhere. The plain cursor sits on the
    /// level of `level_z`, or on a road or node within pick reach when
    /// `join` is set, so drawn roads connect.
    fn haul_cursor_point(&self, level_z: f64, join: bool) -> Option<DVec3> {
        if self.editor.snapping_active() {
            return self.editor.cursor_snapped.then_some(self.editor.cursor_world).flatten();
        }
        let graphics = self.graphics.as_ref()?;
        if join
            && let Some((_, point)) = graphics
                .pick_at_cursor(PICK_THRESHOLD_PX, &[], &self.editor.hidden_handles, &self.editor.frozen_handles, self.editor.xray_enabled)
                .filter(|(h, _)| matches!(h, SceneEntityId::HaulRoad(_) | SceneEntityId::HaulNode(_)))
        {
            return Some(point);
        }
        graphics.cursor_world(level_z)
    }
    /// A click while picking nodes for the selected blocks: a node, or a
    /// point on a road, which is split there to make one when they are
    /// joined. Clicking a picked one again drops it.
    pub(crate) fn pick_haul_link(&mut self) {
        let Some(graphics) = self.graphics.as_ref() else { return };
        let Some(document) = self.workspace.active_document() else { return };
        let at = match graphics.pick_at_cursor(PICK_THRESHOLD_PX, &[], &self.editor.hidden_handles, &self.editor.frozen_handles, self.editor.xray_enabled) {
            Some((SceneEntityId::HaulNode(id), _)) => document.haulage().node(id).map(|n| n.pos),
            Some((SceneEntityId::HaulRoad(_), point)) => Some(point),
            _ => None,
        };
        let Some(at) = at else {
            crate::userspace_log!("{}", tr!("haul-link-missed"));
            return;
        };
        let tolerance = document.haulage().settings.join_tolerance_m;
        let points = &mut self.editor.haul_link_points;
        match points.iter().position(|p| p.distance(at) <= tolerance) {
            Some(index) => {
                points.remove(index);
            }
            None => points.push(at),
        }
    }
    /// Hold the selected blocks to the nodes picked for them, or with none
    /// picked leave them to the nearest road. One undo step either way.
    pub(crate) fn link_haul_blocks(&mut self, at: Vec<DVec3>) {
        self.editor.haul_link_pick = false;
        self.editor.haul_link_points.clear();
        let blocks: Vec<_> = self
            .editor
            .haul_selected_blocks
            .iter()
            .filter_map(|id| self.editor.haul_blocks.iter().find(|b| b.id == *id))
            .map(crate::ui::state::HaulBlock::reference)
            .collect();
        let Some(runtime) = self.workspace.active_project().map(|p| p.runtime_id) else { return };
        if blocks.is_empty() {
            self.invalidate_overlay();
            return;
        }
        if let Err(error) = self.edit_haulage(runtime, HaulEdit::LinkBlocks { blocks, at }) {
            crate::userspace_warn!("{error:#}");
        }
        self.invalidate_overlay();
    }
    pub(crate) fn place_haul_point(&mut self) {
        if let Some(point) = self.haul_cursor_point(self.editor.z_level, true)
            && self.editor.haul_points.last().is_none_or(|last| last.distance(point) > 1e-6)
        {
            self.editor.haul_points.push(point);
        }
        self.invalidate_overlay();
    }
    /// A press on a node or shape point may become a drag. Selection still
    /// runs, so a press that never moves selects as before.
    /// The bend point of `road` nearest the cursor, within the reach a drag
    /// takes one from.
    pub(crate) fn haul_bend_at_cursor(&self, road: crate::model::haulage::RoadId) -> Option<DVec3> {
        let graphics = self.graphics.as_ref()?;
        let cursor = self.editor.cursor_screen_px?;
        let road = self.workspace.active_document()?.haulage().road(road)?;
        let view_proj = graphics.view_proj();
        let reach = self.points_to_px(8.0);
        road.verts
            .iter()
            .filter_map(|p| graphics.world_to_window_px(&view_proj, *p).map(|s| (*p, (s.0 - cursor.0).hypot(s.1 - cursor.1))))
            .filter(|(_, d)| *d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(p, _)| p)
    }
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
            self.editor.haul_cursor = self.haul_cursor_point(self.editor.z_level, true);
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
    /// What the Haulage Layout shows: its roads and the dig blocks not hidden
    /// in Solids Navigation.
    pub(crate) fn haul_layout_bounds(&self) -> Option<(DVec3, DVec3)> {
        let network = self.workspace.active_document()?.haulage();
        let roads = network.roads.iter().flat_map(|road| network.points(road));
        let blocks = self
            .editor
            .haul_blocks
            .iter()
            .filter(|b| !self.editor.haul_hidden.hides(b.solid, b.bench, b.flitch, b.blast))
            .flat_map(|b| b.rings.iter().flatten().copied());
        roads
            .chain(blocks)
            .filter(|p| p.is_finite())
            .fold(None, |bounds, p| Some(bounds.map_or((p, p), |(min, max): (DVec3, DVec3)| (min.min(p), max.max(p)))))
    }

    /// Fit the view to the Layout once when it opens on a project, as soon as
    /// there is something to fit: the project's other geometry may be hidden
    /// or far away, and the page is about its roads and blocks.
    pub(crate) fn sync_haulage_frame(&mut self) {
        if !self.editor.is_haulage_page() {
            self.editor.haulage_framed_key = None;
            return;
        }
        let Some(runtime) = self.workspace.active_project().map(|project| project.runtime_id) else {
            return;
        };
        if self.editor.haulage_framed_key == Some(runtime) {
            return;
        }
        // Nor while a swing into plan view is running: it holds the camera
        // on the target it started from until it lands.
        if self.graphics.as_ref().is_some_and(|graphics| graphics.view_transition_running()) {
            return;
        }
        let Some((min, max)) = self.haul_layout_bounds() else { return };
        self.editor.haulage_framed_key = Some(runtime);
        self.frame_haul(min, max);
    }

    /// Rebuild [`crate::app::App::haul_block_surface`] from the blocks shown.
    pub(crate) fn rebuild_haul_block_surface(&mut self) {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for block in self
            .editor
            .haul_blocks
            .iter()
            .filter(|b| !self.editor.haul_hidden.hides(b.solid, b.bench, b.flitch, b.blast))
        {
            let start = vertices.len();
            let mut points = Vec::new();
            let mut holes = Vec::new();
            for (i, ring) in block.rings.iter().enumerate() {
                if i > 0 {
                    holes.push(points.len());
                }
                points.extend(ring.iter().copied());
            }
            let Some(origin) = points.first().copied() else { continue };
            let mut indices: Vec<usize> = Vec::new();
            earcut::Earcut::new().earcut(points.iter().map(|p| [p.x - origin.x, p.y - origin.y]), &holes, &mut indices);
            vertices.extend(points.iter().map(|p| crate::model::formats::mesh_data::Vertex { x: p.x, y: p.y, z: p.z }));
            faces.extend(
                indices
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .filter_map(|t| Some([u32::try_from(start + t[0]).ok()?, u32::try_from(start + t[1]).ok()?, u32::try_from(start + t[2]).ok()?])),
            );
        }
        self.haul_block_surface.clear();
        let Ok(mesh) = crate::model::formats::mesh_data::Triangulation::from_vertices_and_faces(vertices, faces) else {
            return;
        };
        let mesh = std::sync::Arc::new(mesh);
        let spatial = std::sync::Arc::new(crate::model::spatial::TriangleBvh::build(&mesh));
        let mut surface = crate::app::commands::solids::preview_triangulation(String::new(), mesh, spatial, Vec::new(), std::sync::Arc::new(Vec::new()), [0.0; 4], [0.0; 4]);
        surface.id = HAUL_BLOCK_SURFACE_ID;
        self.haul_block_surface.push(surface);
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
        // A new pile is tipped and loaded at the node it was made from until
        // a separate reclaim point is chosen.
        next.set_role(
            node,
            Some(if kind == DestinationKind::Stockpile {
                NodeRole::DumpAndReclaim(destination)
            } else {
                NodeRole::Dump(destination)
            }),
        )?;
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

/// How one side of a destination - where trucks tip, or where they load -
/// meets the roads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HaulLink {
    /// Its node is on the main road network.
    Connected,
    /// It has no node: trucks reach its surface from the nearest road.
    Surface,
    /// A stockpile with no reclaim node is loaded where it is tipped.
    AtDumpPoint,
    /// A crusher, or a destination with no surface, that has no node.
    NoPoint,
    /// Its node has no road.
    OffRoad,
    /// It meets a piece of road that does not join the main network.
    SeparatePiece,
    /// There are no roads at all.
    NoRoads,
}

impl HaulLink {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Connected => tr!("haul-link-connected"),
            Self::Surface => tr!("haul-link-surface"),
            Self::AtDumpPoint => tr!("haul-link-at-dump"),
            Self::NoPoint => tr!("haul-link-no-point"),
            Self::OffRoad => tr!("haul-link-off-road"),
            Self::SeparatePiece => tr!("haul-link-separate"),
            Self::NoRoads => tr!("haul-link-no-roads"),
        }
    }

    /// Whether trucks cannot get there this way.
    pub(crate) fn is_problem(self) -> bool {
        matches!(self, Self::NoPoint | Self::OffRoad | Self::SeparatePiece | Self::NoRoads)
    }
}

/// How a destination meets the roads: where trucks tip, and for a
/// stockpile where they load.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DestinationLinks {
    pub(crate) name: String,
    pub(crate) kind: DestinationKind,
    pub(crate) dump: HaulLink,
    pub(crate) reclaim: Option<HaulLink>,
}

/// How many of a pit's dig blocks reach the main road network.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PitLinks {
    pub(crate) name: String,
    pub(crate) reached: usize,
    pub(crate) total: usize,
}

/// What the Schedule's Haulage step shows: whether everything trucks haul
/// between reaches the roads.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct HaulConnections {
    pub(crate) destinations: Vec<DestinationLinks>,
    pub(crate) pits: Vec<PitLinks>,
}

impl crate::app::App<'_> {
    /// Whether each destination and each pit's dig blocks reach the roads.
    ///
    /// "The roads" are the main network: the largest piece, as the Layout's
    /// issues judge it. A destination on another piece can only be reached
    /// from blocks on that piece too, which is seldom what was meant.
    pub(crate) fn haul_connections(&self) -> HaulConnections {
        use std::collections::BTreeMap;

        let Some(document) = self.workspace.active_document() else {
            return HaulConnections::default();
        };
        let network = document.haulage();
        let mut adjacency: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
        for road in &network.roads {
            adjacency.entry(road.from).or_default().push(road.to);
            adjacency.entry(road.to).or_default().push(road.from);
        }
        let mut piece: BTreeMap<NodeId, usize> = BTreeMap::new();
        let mut sizes = Vec::new();
        for &seed in adjacency.keys() {
            if piece.contains_key(&seed) {
                continue;
            }
            let mut stack = vec![seed];
            let mut size = 0;
            while let Some(id) = stack.pop() {
                if piece.contains_key(&id) {
                    continue;
                }
                piece.insert(id, sizes.len());
                size += 1;
                stack.extend(&adjacency[&id]);
            }
            sizes.push(size);
        }
        let main = sizes.iter().enumerate().max_by_key(|(_, size)| **size).map(|(index, _)| index);
        let on_main = |node: NodeId| main.is_some() && piece.get(&node).copied() == main;
        let road_on_main = |road: crate::model::haulage::RoadId| network.road(road).is_some_and(|road| on_main(road.from));
        let index = crate::model::haulage::network::RoadIndex::new(network);
        let roads = !network.roads.is_empty();

        // A side served by a node of its own: on the roads, off them, or on
        // a separate piece.
        let node_link = |served: &dyn Fn(NodeRole) -> bool| {
            network
                .nodes
                .iter()
                .find(|node| node.role.is_some_and(served))
                .map(|node| match (adjacency.contains_key(&node.id), on_main(node.id)) {
                    (false, _) => HaulLink::OffRoad,
                    (true, false) => HaulLink::SeparatePiece,
                    (true, true) => HaulLink::Connected,
                })
        };
        let centroids = self.haul_destination_points(document);
        let mut destinations: Vec<DestinationLinks> = crate::model::schedule::destinations::available(document.solids(), document.schedule().routing())
            .into_iter()
            .map(|view| {
                let dump = if !roads {
                    HaulLink::NoRoads
                } else {
                    node_link(&|role| role.destination() == view.id && role.dumps()).unwrap_or_else(|| match centroids.get(&view.id) {
                        Some(point) => match index.candidates(*point, 0.0).first() {
                            Some((road, _, _)) if road_on_main(*road) => HaulLink::Surface,
                            _ => HaulLink::SeparatePiece,
                        },
                        None => HaulLink::NoPoint,
                    })
                };
                let reclaim = (view.kind == DestinationKind::Stockpile).then(|| {
                    if roads {
                        node_link(&|role| role == NodeRole::Reclaim(view.id)).unwrap_or(HaulLink::AtDumpPoint)
                    } else {
                        HaulLink::NoRoads
                    }
                });
                DestinationLinks {
                    name: view.name,
                    kind: view.kind,
                    dump,
                    reclaim,
                }
            })
            .collect();
        destinations.sort_by_key(|entry| entry.kind as u8);

        // A dug block reaches the roads through the nodes it is held to, or
        // else the nearest node within the auto-join distance.
        let max_grade = max_grade(document.schedule().trucks());
        let reach = network.settings.auto_join_m;
        let mut pits: Vec<PitLinks> = Vec::new();
        for block in self.planning_snapshot().ok().iter().flat_map(|snapshot| &snapshot.blocks) {
            let Some(solid) = document.solid(block.solid).filter(|solid| solid.kind == crate::model::SolidKind::Pit) else {
                continue;
            };
            let point = DVec3::new(block.anchor[0], block.anchor[1], block.flitch.base);
            let links = network.block_link(block.solid, block.flitch.base, &block.ground);
            let reached = if links.iter().any(|node| index.node_join(*node).is_some()) {
                links.iter().any(|node| on_main(*node))
            } else {
                index
                    .joins(point, &[], max_grade)
                    .first()
                    .is_some_and(|join| crate::model::haulage::network::access_length(point, join.2, max_grade) <= reach && road_on_main(join.0))
            };
            match pits.iter_mut().find(|pit| pit.name == solid.name) {
                Some(pit) => {
                    pit.total += 1;
                    pit.reached += usize::from(reached);
                }
                None => pits.push(PitLinks {
                    name: solid.name.clone(),
                    reached: usize::from(reached),
                    total: 1,
                }),
            }
        }
        HaulConnections { destinations, pits }
    }
}

/// Every destination a road node can serve: the standalone ones and the
/// solids that are not pits.
pub(crate) fn haul_destinations(document: &crate::model::Document) -> Vec<DestinationId> {
    document
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
        .collect()
}

/// The steepest grade every truck class can drive, which is what a road is
/// judged against.
pub(crate) fn max_grade(trucks: &crate::model::schedule::TruckFleetConfig) -> f64 {
    trucks.classes.iter().map(|c| c.maximum_grade).reduce(f64::min).unwrap_or(0.1)
}
