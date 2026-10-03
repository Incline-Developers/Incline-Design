use std::collections::{BTreeMap, BTreeSet, HashMap};

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::model::{SolidId, schedule::DestinationId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct NodeId(pub(crate) u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct RoadId(pub(crate) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum NodeRole {
    Dump(DestinationId),
    Reclaim(DestinationId),
    /// A stockpile tipped and loaded from the same place.
    DumpAndReclaim(DestinationId),
}
impl NodeRole {
    pub(crate) fn destination(self) -> DestinationId {
        match self {
            Self::Dump(id) | Self::Reclaim(id) | Self::DumpAndReclaim(id) => id,
        }
    }
    pub(crate) fn dumps(self) -> bool {
        matches!(self, Self::Dump(_) | Self::DumpAndReclaim(_))
    }
    pub(crate) fn reclaims(self) -> bool {
        matches!(self, Self::Reclaim(_) | Self::DumpAndReclaim(_))
    }
}

/// A dig block held to a chosen node instead of the nearest road.
///
/// Named by where the block is rather than by its session id: the solid, its
/// flitch's base RL and a point inside it. A rerun that redraws the strips
/// keeps the link with whichever block then covers that point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlockLink {
    pub(crate) solid: SolidId,
    pub(crate) flitch_base: f64,
    pub(crate) probe: [f64; 2],
    pub(crate) node: NodeId,
}
impl BlockLink {
    pub(crate) fn covers(&self, solid: SolidId, flitch_base: f64, face: &[Vec<DVec2>]) -> bool {
        self.solid == solid && (self.flitch_base - flitch_base).abs() < 1e-6 && crate::model::arrangement::point_in_face(face, DVec2::from(self.probe))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HaulNode {
    pub(crate) id: NodeId,
    pub(crate) pos: DVec3,
    pub(crate) role: Option<NodeRole>,
}

/// Endpoints live on nodes; verts contains only intermediate shape points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HaulRoad {
    pub(crate) id: RoadId,
    pub(crate) name: String,
    pub(crate) from: NodeId,
    pub(crate) to: NodeId,
    pub(crate) verts: Vec<DVec3>,
    pub(crate) speed_limit_kph: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct HaulSettings {
    pub(crate) join_tolerance_m: f64,
    pub(crate) auto_join_m: f64,
    pub(crate) bench_speed_kph: f64,
    pub(crate) acceleration_kph_s: f64,
}
impl Default for HaulSettings {
    fn default() -> Self {
        Self {
            join_tolerance_m: 2.0,
            auto_join_m: 300.0,
            bench_speed_kph: 15.0,
            acceleration_kph_s: 1.5,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct HaulNetwork {
    pub(crate) nodes: Vec<HaulNode>,
    pub(crate) roads: Vec<HaulRoad>,
    pub(crate) settings: HaulSettings,
    pub(crate) fixed_destinations: Vec<DestinationId>,
    pub(crate) block_links: Vec<BlockLink>,
    next_node_id: u64,
    next_road_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IssueKind {
    DeadEnd,
    NearMiss,
    SeparatePiece,
    SteepRoad,
    MissingDestination,
}
#[derive(Clone, Debug)]
pub(crate) struct NetworkIssue {
    pub(crate) kind: IssueKind,
    pub(crate) pos: DVec3,
    pub(crate) road: Option<RoadId>,
    pub(crate) node: Option<NodeId>,
}

impl HaulNetwork {
    pub(crate) fn apply_namespace(&mut self, namespace: u32) {
        let id = |id: u64| (u64::from(namespace) << 32) | (id & u64::from(u32::MAX));
        for node in &mut self.nodes {
            node.id.0 = id(node.id.0);
        }
        for road in &mut self.roads {
            road.id.0 = id(road.id.0);
            road.from.0 = id(road.from.0);
            road.to.0 = id(road.to.0);
        }
        for link in &mut self.block_links {
            link.node.0 = id(link.node.0);
        }
        self.next_node_id = id(self.next_node_id);
        self.next_road_id = id(self.next_road_id);
    }
    pub(crate) fn local_copy(&self) -> Self {
        let mut copy = self.clone();
        copy.apply_namespace(0);
        copy
    }
    pub(crate) fn node(&self, id: NodeId) -> Option<&HaulNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub(crate) fn road(&self, id: RoadId) -> Option<&HaulRoad> {
        self.roads.iter().find(|r| r.id == id)
    }
    pub(crate) fn points(&self, road: &HaulRoad) -> Vec<DVec3> {
        let Some(from) = self.node(road.from) else { return Vec::new() };
        let Some(to) = self.node(road.to) else { return Vec::new() };
        std::iter::once(from.pos).chain(road.verts.iter().copied()).chain(std::iter::once(to.pos)).collect()
    }
    pub(crate) fn raise_allocator_to(&mut self, other: &Self) {
        self.next_node_id = self.next_node_id.max(other.next_node_id);
        self.next_road_id = self.next_road_id.max(other.next_road_id);
    }
    pub(crate) fn add_node(&mut self, pos: DVec3) -> anyhow::Result<NodeId> {
        anyhow::ensure!(pos.is_finite(), "invalid road position");
        if let Some(id) = self.nodes.iter().map(|n| n.id.0).max() {
            self.next_node_id = self.next_node_id.max(id.checked_add(1).ok_or_else(|| anyhow::anyhow!("node ids exhausted"))?);
        }
        let id = NodeId(self.next_node_id);
        self.next_node_id = self
            .next_node_id
            .checked_add(1)
            .filter(|n| n & u64::from(u32::MAX) != 0)
            .ok_or_else(|| anyhow::anyhow!("road node ids exhausted"))?;
        self.nodes.push(HaulNode { id, pos, role: None });
        Ok(id)
    }
    pub(crate) fn add_road(&mut self, name: String, from: NodeId, to: NodeId, verts: Vec<DVec3>) -> anyhow::Result<RoadId> {
        anyhow::ensure!(from != to && self.node(from).is_some() && self.node(to).is_some(), "road needs two distinct nodes");
        anyhow::ensure!(verts.iter().all(|p| p.is_finite()), "invalid road shape");
        if let Some(id) = self.roads.iter().map(|r| r.id.0).max() {
            self.next_road_id = self.next_road_id.max(id.checked_add(1).ok_or_else(|| anyhow::anyhow!("road ids exhausted"))?);
        }
        let id = RoadId(self.next_road_id);
        self.next_road_id = self
            .next_road_id
            .checked_add(1)
            .filter(|n| n & u64::from(u32::MAX) != 0)
            .ok_or_else(|| anyhow::anyhow!("road ids exhausted"))?;
        self.roads.push(HaulRoad {
            id,
            name,
            from,
            to,
            verts,
            speed_limit_kph: None,
        });
        Ok(id)
    }
    pub(crate) fn move_node(&mut self, id: NodeId, pos: DVec3) -> anyhow::Result<()> {
        anyhow::ensure!(pos.is_finite(), "invalid road position");
        self.nodes.iter_mut().find(|n| n.id == id).ok_or_else(|| anyhow::anyhow!("unknown road node"))?.pos = pos;
        Ok(())
    }
    pub(crate) fn delete_node(&mut self, id: NodeId) {
        self.roads.retain(|r| r.from != id && r.to != id);
        self.nodes.retain(|n| n.id != id);
        self.prune_loose_nodes();
    }
    pub(crate) fn delete_road(&mut self, id: RoadId) {
        self.roads.retain(|r| r.id != id);
        self.prune_loose_nodes();
    }
    /// A node left with no road and no destination role marks nothing, so
    /// deleting the last road at it takes it too.
    fn prune_loose_nodes(&mut self) {
        let roads = &self.roads;
        self.nodes.retain(|n| n.role.is_some() || roads.iter().any(|r| r.from == n.id || r.to == n.id));
        let nodes = &self.nodes;
        self.block_links.retain(|l| nodes.iter().any(|n| n.id == l.node));
    }
    /// The node a block is held to, if it has been linked to one.
    pub(crate) fn block_link(&self, solid: SolidId, flitch_base: f64, face: &[Vec<DVec2>]) -> Option<NodeId> {
        self.block_links.iter().find(|l| l.covers(solid, flitch_base, face)).map(|l| l.node)
    }
    pub(crate) fn set_block_link(&mut self, solid: SolidId, flitch_base: f64, face: &[Vec<DVec2>], probe: [f64; 2], node: Option<NodeId>) -> anyhow::Result<()> {
        anyhow::ensure!(node.is_none_or(|id| self.node(id).is_some()), "unknown road node");
        self.block_links.retain(|l| !l.covers(solid, flitch_base, face));
        if let Some(node) = node {
            self.block_links.push(BlockLink { solid, flitch_base, probe, node });
        }
        Ok(())
    }
    /// Give a node a role, taking whatever part of it another node held: a
    /// node that both tipped and loaded a pile keeps the half not moved.
    pub(crate) fn set_role(&mut self, id: NodeId, role: Option<NodeRole>) -> anyhow::Result<()> {
        anyhow::ensure!(self.node(id).is_some(), "unknown road node");
        if let Some(role) = role {
            let destination = role.destination();
            for node in self.nodes.iter_mut().filter(|n| n.id != id) {
                let Some(held) = node.role.filter(|r| r.destination() == destination) else { continue };
                node.role = match (held.dumps() && !role.dumps(), held.reclaims() && !role.reclaims()) {
                    (true, true) => Some(held),
                    (true, false) => Some(NodeRole::Dump(destination)),
                    (false, true) => Some(NodeRole::Reclaim(destination)),
                    (false, false) => None,
                };
            }
        }
        self.nodes.iter_mut().find(|n| n.id == id).expect("checked node").role = role;
        Ok(())
    }
    pub(crate) fn role_point(&self, destination: DestinationId, reclaim: bool) -> Option<DVec3> {
        let serves = |n: &&HaulNode, reclaim: bool| n.role.is_some_and(|r| r.destination() == destination && if reclaim { r.reclaims() } else { r.dumps() });
        // A pile with no reclaim point of its own is loaded where it is tipped.
        self.nodes
            .iter()
            .find(|n| serves(n, reclaim))
            .or_else(|| self.nodes.iter().find(|n| serves(n, false)))
            .map(|n| n.pos)
    }
    /// Where trucks meet a destination: its role node, else `centroid` (a
    /// solid destination's surface), which routing joins to the nearest road.
    /// `None` when the destination is held to its fixed distance or has no
    /// point at all.
    pub(crate) fn destination_point(&self, destination: DestinationId, reclaim: bool, centroid: Option<DVec3>) -> Option<DVec3> {
        if self.fixed_destinations.contains(&destination) {
            return None;
        }
        self.role_point(destination, reclaim).or(centroid)
    }
    /// Split on a specific segment. The original id remains on the first half.
    pub(crate) fn split(&mut self, id: RoadId, segment: usize, pos: DVec3) -> anyhow::Result<NodeId> {
        let road = self.road(id).ok_or_else(|| anyhow::anyhow!("unknown road"))?.clone();
        let points = self.points(&road);
        anyhow::ensure!(segment + 1 < points.len(), "unknown road segment");
        if pos.distance(points[0]) < 1e-6 {
            return Ok(road.from);
        }
        if pos.distance(*points.last().expect("road endpoints")) < 1e-6 {
            return Ok(road.to);
        }
        let node = self.add_node(pos)?;
        let next = self.add_road(road.name.clone(), node, road.to, points[segment + 1..points.len() - 1].to_vec())?;
        let second = self.roads.iter_mut().find(|r| r.id == next).expect("new road");
        second.speed_limit_kph = road.speed_limit_kph;
        if second.verts.first().is_some_and(|p| p.distance(pos) < 1e-6) {
            second.verts.remove(0);
        }
        let first = self.roads.iter_mut().find(|r| r.id == id).expect("original road");
        first.to = node;
        first.verts = points[1..segment + 1].to_vec();
        if first.verts.last().is_some_and(|p| p.distance(pos) < 1e-6) {
            first.verts.pop();
        }
        Ok(node)
    }
    pub(crate) fn join(&mut self, keep: NodeId, remove: NodeId) -> anyhow::Result<()> {
        let removed = self.node(remove).ok_or_else(|| anyhow::anyhow!("unknown road node"))?.clone();
        let kept = self.node(keep).ok_or_else(|| anyhow::anyhow!("unknown road node"))?;
        anyhow::ensure!(keep != remove, "select two nodes");
        anyhow::ensure!(
            kept.role.is_none() || removed.role.is_none() || kept.role == removed.role,
            "nodes have different destination roles"
        );
        if kept.role.is_none() {
            self.set_role(keep, removed.role)?;
        }
        for link in self.block_links.iter_mut().filter(|l| l.node == remove) {
            link.node = keep;
        }
        for road in &mut self.roads {
            if road.from == remove {
                road.from = keep;
            }
            if road.to == remove {
                road.to = keep;
            }
        }
        self.roads.retain(|r| r.from != r.to);
        self.nodes.retain(|n| n.id != remove);
        Ok(())
    }
    /// Join a drawn point to a node, or split an existing road at its projection.
    pub(crate) fn join_point(&mut self, pos: DVec3) -> anyhow::Result<NodeId> {
        let tolerance = self.settings.join_tolerance_m;
        if let Some(node) = self
            .nodes
            .iter()
            .filter(|n| n.pos.distance(pos) <= tolerance)
            .min_by(|a, b| a.pos.distance_squared(pos).total_cmp(&b.pos.distance_squared(pos)))
        {
            return Ok(node.id);
        }
        let closest = self
            .roads
            .iter()
            .flat_map(|r| self.points(r).windows(2).enumerate().map(|(i, p)| (r.id, i, project(pos, p[0], p[1]))).collect::<Vec<_>>())
            .filter(|(_, _, p)| p.distance(pos) <= tolerance)
            .min_by(|a, b| a.2.distance_squared(pos).total_cmp(&b.2.distance_squared(pos)));
        if let Some((road, segment, p)) = closest {
            self.split(road, segment, p)
        } else {
            self.add_node(pos)
        }
    }
    /// Find junctions before adding roads, so an ordinary shape vertex never
    /// becomes a graph node merely because it lies on its own string.
    pub(crate) fn convert(&mut self, strings: &[(String, Vec<DVec3>)]) -> anyhow::Result<()> {
        let strings: Vec<_> = strings
            .iter()
            .filter_map(|(name, points)| {
                let mut points = points.clone();
                points.dedup_by(|a, b| a.distance_squared(*b) < 1e-12);
                (points.len() >= 2).then(|| (name.clone(), points))
            })
            .collect();
        let tolerance = self.settings.join_tolerance_m;
        let mut cuts: Vec<Vec<(f64, DVec3)>> = strings.iter().map(|(_, p)| vec![(0.0, p[0]), ((p.len() - 1) as f64, *p.last().expect("string"))]).collect();
        for (source, (_, points)) in strings.iter().enumerate() {
            // A closed string needs two explicit ends to represent its loop.
            if points[0].distance(*points.last().expect("string")) <= tolerance {
                let mid = points.len() / 2;
                cuts[source].push((mid as f64, points[mid]));
            }
            for (vertex, &point) in points.iter().enumerate() {
                for (target, (_, other)) in strings.iter().enumerate().filter(|(i, _)| *i != source) {
                    for (segment, pair) in other.windows(2).enumerate() {
                        let projected = project(point, pair[0], pair[1]);
                        if projected.distance(point) <= tolerance {
                            let fraction = (projected - pair[0]).length() / pair[0].distance(pair[1]);
                            cuts[source].push((vertex as f64, point));
                            cuts[target].push((segment as f64 + fraction, projected));
                        }
                    }
                }
                if self.nodes.iter().any(|n| n.pos.distance(point) <= tolerance)
                    || self
                        .roads
                        .iter()
                        .any(|r| self.points(r).windows(2).any(|p| project(point, p[0], p[1]).distance(point) <= tolerance))
                {
                    cuts[source].push((vertex as f64, point));
                }
            }
            // Existing endpoints on a newly converted string also join it.
            for node in &self.nodes {
                for (segment, pair) in points.windows(2).enumerate() {
                    let projected = project(node.pos, pair[0], pair[1]);
                    if projected.distance(node.pos) <= tolerance {
                        let fraction = (projected - pair[0]).length() / pair[0].distance(pair[1]);
                        cuts[source].push((segment as f64 + fraction, node.pos));
                    }
                }
            }
        }
        for ((name, points), mut cuts) in strings.into_iter().zip(cuts) {
            cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
            cuts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-8);
            for pair in cuts.windows(2) {
                let from = self.join_point(pair[0].1)?;
                let to = self.join_point(pair[1].1)?;
                if from == to {
                    continue;
                }
                let from_pos = self.node(from).expect("joined node").pos;
                let to_pos = self.node(to).expect("joined node").pos;
                let verts = points
                    .iter()
                    .enumerate()
                    .filter(|(i, p)| (*i as f64) > pair[0].0 + 1e-8 && (*i as f64) < pair[1].0 - 1e-8 && p.distance(from_pos) > 1e-6 && p.distance(to_pos) > 1e-6)
                    .map(|(_, p)| *p)
                    .collect();
                self.add_road(name.clone(), from, to, verts)?;
            }
        }
        Ok(())
    }
    pub(crate) fn is_pristine(&self) -> bool {
        self == &Self::default()
    }

    pub(crate) fn validate(&mut self) -> anyhow::Result<()> {
        for v in [
            self.settings.join_tolerance_m,
            self.settings.auto_join_m,
            self.settings.bench_speed_kph,
            self.settings.acceleration_kph_s,
        ] {
            anyhow::ensure!(v.is_finite() && v > 0.0, "haulage settings must be positive");
        }
        let mut nodes = BTreeSet::new();
        let mut roads = BTreeSet::new();
        let mut roles = BTreeSet::new();
        for n in &self.nodes {
            if let Some(role) = n.role {
                for (serves, reclaim) in [(role.dumps(), false), (role.reclaims(), true)] {
                    anyhow::ensure!(!serves || roles.insert((role.destination(), reclaim)), "duplicate haul destination role");
                }
            }
            anyhow::ensure!(nodes.insert(n.id) && n.pos.is_finite(), "invalid or duplicate road node");
        }
        for r in &self.roads {
            anyhow::ensure!(
                roads.insert(r.id) && nodes.contains(&r.from) && nodes.contains(&r.to) && r.from != r.to,
                "invalid road topology"
            );
            anyhow::ensure!(
                r.verts.iter().all(|p| p.is_finite()) && r.speed_limit_kph.is_none_or(|v| v.is_finite() && v > 0.0),
                "invalid road geometry or speed"
            );
            anyhow::ensure!(self.points(r).windows(2).all(|p| p[0].distance(p[1]) > 1e-6), "zero length road segment");
        }
        for link in &self.block_links {
            anyhow::ensure!(
                nodes.contains(&link.node) && link.flitch_base.is_finite() && link.probe.iter().all(|v| v.is_finite()),
                "invalid block link"
            );
        }
        if let Some(id) = nodes.last() {
            self.next_node_id = self.next_node_id.max(id.0.checked_add(1).ok_or_else(|| anyhow::anyhow!("node ids exhausted"))?);
        }
        if let Some(id) = roads.last() {
            self.next_road_id = self.next_road_id.max(id.0.checked_add(1).ok_or_else(|| anyhow::anyhow!("road ids exhausted"))?);
        }
        Ok(())
    }
    /// `blocks` are the dig blocks' anchors: a road ending among them is the
    /// way in to a working face, not a dead end.
    pub(crate) fn issues(&self, destinations: &[DestinationId], max_grade: f64, blocks: &[DVec3]) -> Vec<NetworkIssue> {
        let mut issues = Vec::new();
        let mut adjacency: BTreeMap<NodeId, Vec<NodeId>> = self.nodes.iter().map(|n| (n.id, Vec::new())).collect();
        for road in &self.roads {
            adjacency.entry(road.from).or_default().push(road.to);
            adjacency.entry(road.to).or_default().push(road.from);
            if self.points(road).windows(2).any(|p| grade(p[0], p[1]).abs() > max_grade) {
                issues.push(NetworkIssue {
                    kind: IssueKind::SteepRoad,
                    pos: self.node(road.from).expect("validated topology").pos,
                    road: Some(road.id),
                    node: None,
                });
            }
        }
        let mut unseen: BTreeSet<_> = adjacency.keys().copied().collect();
        let mut components = Vec::new();
        while let Some(&seed) = unseen.first() {
            let mut stack = vec![seed];
            let mut component = Vec::new();
            while let Some(id) = stack.pop() {
                if unseen.remove(&id) {
                    component.push(id);
                    stack.extend(&adjacency[&id]);
                }
            }
            components.push(component);
        }
        components.sort_by_key(|c| std::cmp::Reverse(c.len()));
        for component in components.iter().skip(1) {
            let id = component[0];
            issues.push(NetworkIssue {
                kind: IssueKind::SeparatePiece,
                pos: self.node(id).expect("node").pos,
                road: None,
                node: Some(id),
            });
        }
        for node in &self.nodes {
            if adjacency[&node.id].len() <= 1 && node.role.is_none() {
                let reach = self.settings.auto_join_m;
                let serves_blocks = self.block_links.iter().any(|l| l.node == node.id) || blocks.iter().any(|b| access_length(*b, node.pos, max_grade) <= reach);
                if !serves_blocks {
                    issues.push(NetworkIssue {
                        kind: IssueKind::DeadEnd,
                        pos: node.pos,
                        road: None,
                        node: Some(node.id),
                    });
                }
                if self.roads.iter().filter(|r| r.from != node.id && r.to != node.id).any(|r| {
                    self.points(r)
                        .windows(2)
                        .any(|p| project(node.pos, p[0], p[1]).distance(node.pos) <= 3.0 * self.settings.join_tolerance_m)
                }) {
                    issues.push(NetworkIssue {
                        kind: IssueKind::NearMiss,
                        pos: node.pos,
                        road: None,
                        node: Some(node.id),
                    });
                }
            }
            if let Some(id) = node.role.map(NodeRole::destination)
                && !destinations.contains(&id)
            {
                issues.push(NetworkIssue {
                    kind: IssueKind::MissingDestination,
                    pos: node.pos,
                    road: None,
                    node: Some(node.id),
                });
            }
        }
        issues
    }
    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        // Serialization has deterministic vector order; counters count as saved
        // content so branching history cannot silently reuse identities.
        let mut copy = self.local_copy();
        copy.next_node_id = 0;
        copy.next_road_id = 0;
        serde_json::to_vec(&copy).expect("finite validated haul network").hash(hasher);
    }
    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self.nodes.len() * size_of::<HaulNode>()
            + self
                .roads
                .iter()
                .map(|r| size_of::<HaulRoad>() + r.name.len() + r.verts.len() * size_of::<DVec3>())
                .sum::<usize>()
    }
}

pub(crate) fn project(p: DVec3, a: DVec3, b: DVec3) -> DVec3 {
    let delta = b - a;
    a + delta * ((p - a).dot(delta) / delta.length_squared().max(1e-12)).clamp(0.0, 1.0)
}
/// A straight drive from `a` to `b` that climbs no steeper than `max_grade`:
/// the distance, or the height change at that grade if longer.
pub(crate) fn access_length(a: DVec3, b: DVec3, max_grade: f64) -> f64 {
    a.distance(b).max((b.z - a.z).abs() / max_grade)
}
pub(crate) fn grade(a: DVec3, b: DVec3) -> f64 {
    (b.z - a.z) / (b - a).truncate().length().max(1e-9)
}

/// One grid built per network snapshot, shared by all block access queries.
pub(crate) struct RoadIndex {
    cells: HashMap<(i64, i64), Vec<usize>>,
    points: Vec<(RoadId, usize, DVec3, DVec3)>,
    cell_m: f64,
    long_segments: Vec<usize>,
    nodes: Vec<(NodeId, (RoadId, usize, DVec3))>,
}
impl RoadIndex {
    pub(crate) fn new(network: &HaulNetwork) -> Self {
        let mut index = Self {
            cells: HashMap::new(),
            points: Vec::new(),
            cell_m: network.settings.auto_join_m.max(10.0),
            long_segments: Vec::new(),
            nodes: Vec::new(),
        };
        let mut indexed_nodes = BTreeSet::new();
        for road in &network.roads {
            let points = network.points(road);
            for (id, segment, point) in [(road.from, 0, points[0]), (road.to, points.len() - 2, *points.last().expect("road endpoints"))] {
                if indexed_nodes.insert(id) {
                    index.nodes.push((id, (road.id, segment, point)));
                }
            }
            for (segment, p) in points.windows(2).enumerate() {
                let slot = index.points.len();
                index.points.push((road.id, segment, p[0], p[1]));
                let min = p[0].min(p[1]) / index.cell_m;
                let max = p[0].max(p[1]) / index.cell_m;
                if (max.x.floor() - min.x.floor() + 1.0) * (max.y.floor() - min.y.floor() + 1.0) > 4096.0 {
                    index.long_segments.push(slot);
                    continue;
                }
                for x in min.x.floor() as i64..=max.x.floor() as i64 {
                    for y in min.y.floor() as i64..=max.y.floor() as i64 {
                        index.cells.entry((x, y)).or_default().push(slot);
                    }
                }
            }
        }
        index
    }
    /// The shortest grade-limited straight drive from `p` onto a road: the
    /// larger of the distance and the height change at `max_grade`. Within
    /// `reach` the point counts as connected. `None` with no roads at all.
    pub(crate) fn access_m(&self, p: DVec3, reach: f64, max_grade: f64) -> Option<f64> {
        self.joins(p, None, reach, max_grade).first().map(|j| access_length(p, j.2, max_grade))
    }
    /// Where a road leaves `node`, for a block held to it.
    pub(crate) fn node_join(&self, node: NodeId) -> Option<(RoadId, usize, DVec3)> {
        self.nodes.iter().find(|(id, _)| *id == node).map(|(_, join)| *join)
    }
    /// Where a block at `p` meets the network: its linked node, else the
    /// nearest road point by grade-limited drive. Points within a metre of
    /// the nearest are all returned - the same junction reached along
    /// different roads - so routing can take the quickest; anything further
    /// would be a short cut through the pit walls.
    pub(crate) fn joins(&self, p: DVec3, link: Option<NodeId>, reach: f64, max_grade: f64) -> Vec<(RoadId, usize, DVec3)> {
        if let Some(join) = link.and_then(|id| self.node_join(id)) {
            return vec![join];
        }
        let mut candidates = self.candidates(p, reach);
        let nearest = candidates.iter().map(|c| access_length(p, c.2, max_grade)).fold(f64::INFINITY, f64::min);
        candidates.retain(|c| access_length(p, c.2, max_grade) <= nearest + 1.0);
        candidates.sort_by(|a, b| access_length(p, a.2, max_grade).total_cmp(&access_length(p, b.2, max_grade)));
        candidates
    }
    pub(crate) fn candidates(&self, p: DVec3, distance: f64) -> Vec<(RoadId, usize, DVec3)> {
        let mut slots = BTreeSet::new();
        let min = (p - DVec3::splat(distance)) / self.cell_m;
        let max = (p + DVec3::splat(distance)) / self.cell_m;
        for x in min.x.floor() as i64..=max.x.floor() as i64 {
            for y in min.y.floor() as i64..=max.y.floor() as i64 {
                if let Some(entries) = self.cells.get(&(x, y)) {
                    slots.extend(entries);
                }
            }
        }
        slots.extend(self.long_segments.iter());
        let near: Vec<_> = slots.into_iter().map(|i: &usize| self.points[*i]).collect();
        let mut candidates: Vec<_> = near
            .iter()
            .map(|(id, i, a, b)| (*id, *i, project(p, *a, *b)))
            .filter(|(_, _, q)| p.distance(*q) <= distance)
            .collect();
        let widened = candidates.is_empty();
        if widened {
            candidates = self.points.iter().map(|(id, i, a, b)| (*id, *i, project(p, *a, *b))).collect();
        }
        candidates.sort_by(|a, b| a.2.distance_squared(p).total_cmp(&b.2.distance_squared(p)));
        let mut roads = BTreeSet::new();
        candidates.retain(|c| roads.insert(c.0));
        candidates.truncate(if widened { 1 } else { 8 });
        if !widened
            && let Some((_, node)) = self.nodes.iter().min_by(|a, b| a.1.2.distance_squared(p).total_cmp(&b.1.2.distance_squared(p)))
            && node.2.distance(p) <= distance
            && !candidates.contains(node)
        {
            candidates.push(*node);
        }
        candidates
    }
}
