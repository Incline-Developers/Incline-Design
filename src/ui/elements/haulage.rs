//! Haulage layout: what is selected in the network, a route check while the
//! selection names a haul, and the network's issues, as a column of panes
//! beside the viewport.
//!
//! Drawing, converting and DXF exchange are tools in the strip down the
//! viewport's left edge. Figures come from the active project's network and
//! are summarised in each row; the detail behind them is on hover.
use glam::DVec3;

use crate::{
    i18n::tr,
    model::{
        Document, SceneEntityId,
        haulage::{
            HaulNetwork, NodeId, NodeRole, RoadId,
            network::{IssueKind, RoadIndex, grade},
            routing::{DestinationSearch, RouteCheck},
        },
        schedule::{DestinationId, DestinationKind, LoaderAgentId, TruckClassId, destinations},
    },
    ui::{
        EditorState, UiProjectView,
        elements::planning_setup::{central_pane_of, stacked_lower_share},
        state::{HaulEdit, HaulPromote, HaulPromotePoint, HaulPromoteTarget, UiCommand},
        widgets::{
            context_menu::ContextMenuAction,
            data_grid::{DataGrid, PropertyRows, PropertyTable, grid_cell_entry, grid_cell_warning, grid_columns_row, grid_empty_state, property_table_height},
            island::{Island, Side},
            menu,
        },
    },
};

fn command(commands: &mut Vec<UiCommand>, session: u32, edit: HaulEdit) {
    commands.push(UiCommand::Haulage { project: session, edit });
}

fn bounds(points: &[DVec3]) -> (DVec3, DVec3) {
    points
        .iter()
        .fold((DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)), |(lo, hi), p| (lo.min(*p), hi.max(*p)))
}

fn road_stats(network: &HaulNetwork, road: &crate::model::haulage::HaulRoad) -> (f64, f64) {
    let points = network.points(road);
    let length = points.windows(2).map(|p| p[0].distance(p[1])).sum();
    let steepest = points.windows(2).map(|p| grade(p[0], p[1]).abs()).fold(0.0, f64::max);
    (length, steepest)
}

fn destination_name(destinations: &[destinations::DestinationView], id: DestinationId) -> String {
    destinations
        .iter()
        .find(|d| d.id == id)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| tr!("haul-missing-destination"))
}

pub(crate) fn role_label(destinations: &[destinations::DestinationView], role: Option<NodeRole>) -> String {
    match role {
        None => tr!("haul-role-none"),
        Some(NodeRole::Dump(id)) => tr!("haul-role-dump", destination = destination_name(destinations, id)),
        Some(NodeRole::Reclaim(id)) => tr!("haul-role-reclaim", destination = destination_name(destinations, id)),
        Some(NodeRole::DumpAndReclaim(id)) => tr!("haul-role-both", destination = destination_name(destinations, id)),
    }
}

/// The roles a node can take for one destination: a stockpile is also
/// loaded from, at its own point or where it is tipped.
fn roles_for(destination: &destinations::DestinationView) -> Vec<NodeRole> {
    if destination.kind == DestinationKind::Stockpile {
        vec![NodeRole::DumpAndReclaim(destination.id), NodeRole::Dump(destination.id), NodeRole::Reclaim(destination.id)]
    } else {
        vec![NodeRole::Dump(destination.id)]
    }
}

/// The role a node takes when it becomes a destination's way in: a pile is
/// loaded there too, unless it already has a reclaim point of its own.
fn arrival_role(network: &HaulNetwork, destination: &destinations::DestinationView) -> NodeRole {
    let separate_reclaim = network.nodes.iter().any(|n| n.role == Some(NodeRole::Reclaim(destination.id)));
    if destination.kind == DestinationKind::Stockpile && !separate_reclaim {
        NodeRole::DumpAndReclaim(destination.id)
    } else {
        NodeRole::Dump(destination.id)
    }
}

/// A node's name in lists and issues: its role, else the road it ends.
fn node_label(network: &HaulNetwork, destinations: &[destinations::DestinationView], id: NodeId) -> String {
    let Some(node) = network.node(id) else { return tr!("haul-node") };
    if node.role.is_some() {
        return role_label(destinations, node.role);
    }
    network
        .roads
        .iter()
        .find(|r| r.from == id || r.to == id)
        .map(|r| tr!("haul-node-on", road = r.name.clone()))
        .unwrap_or_else(|| tr!("haul-node"))
}

/// Shares of the Issues grid's columns: what is wrong, then where.
const ISSUE_FRACTIONS: [f32; 2] = [0.45, 0.55];
/// Height of the loaded-haul profile under the route check's figures.
const PROFILE_HEIGHT: f32 = 100.0;

/// What the Layout column claimed: its panes, for the chrome to round off,
/// and the seams that resize them.
pub(crate) struct LayoutColumn {
    pub(crate) regions: Vec<egui::Rect>,
    pub(crate) grips: Vec<crate::ui::chrome::Grip>,
}

/// The Layout page's column down the right edge: what is selected in the
/// viewport, a route check while a block and a destination are, and the
/// network's issues.
pub(crate) fn draw_panel(ui: &mut egui::Ui, editor: &mut EditorState, _document: &Document, project: &UiProjectView, commands: &mut Vec<UiCommand>) -> LayoutColumn {
    let network = &project.haulage;
    let session = project.active_session;
    let destinations = project.haul_destinations.clone();
    let column = Island::new("haulage_layout", Side::Right)
        .default_width(360.0)
        .min_width(260.0)
        .max_width(560.0)
        .bare()
        .show(ui, |ui, _| {
            // Claimed from the bottom up; the selection takes what is left.
            let mut regions = Vec::new();
            let mut grips = Vec::new();
            let picked = Picked::of(editor, network);
            let reserve = property_table_height(ui, picked.rows()) + crate::ui::chrome::region_frame(ui).total_margin().sum().y;
            let (rect, seam) = stacked_lower_share(ui, "haul_issues_pane", 0.25, reserve, |ui, rect| issues(ui, rect, editor, network, &destinations, commands));
            regions.push(rect);
            grips.push(seam);
            match route_question(editor, network, &destinations) {
                Some(question) => {
                    let (rect, seam) = stacked_lower_share(ui, "haul_route_pane", 0.55, reserve, |ui, rect| {
                        route_check(ui, rect, editor, network, project, &destinations, &question, commands);
                    });
                    regions.push(rect);
                    grips.push(seam);
                }
                None if editor.haul_route.take().is_some() => commands.push(UiCommand::RefreshHaulOverlay),
                None => {}
            }
            regions.push(central_pane_of(ui, |ui, rect| {
                selection(ui, rect, picked, editor, network, project, &destinations, session, commands);
            }));
            (regions, grips)
        });
    let (regions, mut grips) = column.inner;
    grips.push(column.grip);
    LayoutColumn { regions, grips }
}

/// What the Selection pane is describing this frame.
enum Picked {
    /// A road being drawn, with the points clicked so far.
    Drawing(usize),
    /// A destination's node about to be deleted, waiting for a yes.
    ConfirmDelete(NodeId),
    /// Nodes being picked for the selected blocks: how many blocks, and how
    /// many nodes so far.
    Linking(usize, usize),
    /// Dug blocks, which can be held to nodes.
    Blocks(Vec<crate::ui::state::HaulBlock>),
    /// A block of a dump or stockpile: that destination.
    Destination(DestinationId),
    Roads(Vec<RoadId>),
    Nodes(Vec<NodeId>),
    /// Roads and nodes together: all that is shared is deleting them.
    Mixed(Vec<RoadId>, Vec<NodeId>),
    Nothing,
}

impl Picked {
    fn of(editor: &mut EditorState, network: &HaulNetwork) -> Self {
        if editor.haul_draw {
            return Self::Drawing(editor.haul_points.len());
        }
        if let Some(id) = editor.haul_delete_node {
            return Self::ConfirmDelete(id);
        }
        let roads: Vec<RoadId> = editor
            .selected_handles
            .iter()
            .filter_map(|h| if let SceneEntityId::HaulRoad(id) = h { network.road(*id).map(|r| r.id) } else { None })
            .collect();
        let nodes: Vec<NodeId> = editor
            .selected_handles
            .iter()
            .filter_map(|h| if let SceneEntityId::HaulNode(id) = h { network.node(*id).map(|n| n.id) } else { None })
            .collect();
        let blocks: Vec<_> = editor
            .haul_selected_blocks
            .iter()
            .filter_map(|id| editor.haul_blocks.iter().find(|b| b.id == *id))
            .cloned()
            .collect();
        let dug: Vec<_> = blocks.iter().filter(|b| b.dug).cloned().collect();
        if dug.is_empty() {
            editor.haul_link_pick = false;
            editor.haul_link_points.clear();
        }
        if editor.haul_link_pick {
            return Self::Linking(dug.len(), editor.haul_link_points.len());
        }
        // Blocks come first: a node selected beside one is the destination
        // the route check asks about, not something to edit.
        if !dug.is_empty() {
            return Self::Blocks(dug);
        }
        if let [block] = &blocks[..]
            && roads.is_empty()
            && nodes.is_empty()
        {
            return Self::Destination(DestinationId::Solid(block.solid));
        }
        match (roads.is_empty(), nodes.is_empty()) {
            (false, true) => Self::Roads(roads),
            (true, false) => Self::Nodes(nodes),
            (false, false) => Self::Mixed(roads, nodes),
            (true, true) => Self::Nothing,
        }
    }

    /// Rows the pane draws, so the panes under it leave room for them.
    fn rows(&self) -> usize {
        match self {
            Self::Drawing(_) | Self::ConfirmDelete(_) => 3,
            Self::Linking(..) => 4,
            Self::Blocks(blocks) if blocks.len() == 1 => 3 + blocks[0].links.len().max(1),
            Self::Blocks(_) => 6,
            Self::Destination(_) => 2,
            Self::Roads(roads) if roads.len() == 1 => 5,
            Self::Roads(_) => 2,
            Self::Nodes(nodes) if nodes.len() == 1 => 5,
            Self::Nodes(nodes) if nodes.len() == 2 => 2,
            Self::Nodes(_) | Self::Mixed(..) => 1,
            Self::Nothing => 1,
        }
    }
}

/// How the access to a destination is chosen: its own node, the node
/// selected beside it, or the road nearest its surface.
#[derive(Clone, Copy, PartialEq)]
enum Access {
    Node,
    SelectedNode,
    Nearest,
    PinNearest,
    /// Neither: no node of its own, and no surface or no road to reach it
    /// by, so nothing can be hauled there.
    Unreached,
}

/// A node's role as its combo offers it: one it can take, or a new
/// destination made at the node.
#[derive(Clone, Copy, PartialEq)]
enum RoleChoice {
    Role(Option<NodeRole>),
    New(DestinationKind),
}

/// Block counts by how each meets the roads, for a selection of several.
fn access_counts(blocks: &[crate::ui::state::HaulBlock]) -> (usize, usize, usize) {
    let manual = blocks.iter().filter(|b| !b.links.is_empty()).count();
    let far = blocks.iter().filter(|b| !b.connected).count();
    (manual, blocks.len() - manual - far, far)
}

#[allow(clippy::too_many_arguments)]
fn selection(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    picked: Picked,
    editor: &mut EditorState,
    network: &HaulNetwork,
    project: &UiProjectView,
    destinations: &[destinations::DestinationView],
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let title = match &picked {
        Picked::Drawing(_) => tr!("haul-new-road"),
        Picked::ConfirmDelete(_) => tr!("haul-node"),
        Picked::Linking(..) => tr!("haul-join-title"),
        Picked::Blocks(blocks) if blocks.len() == 1 => blocks[0].name.clone(),
        Picked::Blocks(blocks) => tr!("haul-blocks-selected", count = blocks.len()),
        Picked::Destination(id) => destination_name(destinations, *id),
        Picked::Roads(roads) if roads.len() == 1 => tr!("haul-road"),
        Picked::Roads(roads) => tr!("haul-roads-selected", count = roads.len()),
        Picked::Nodes(nodes) if nodes.len() == 1 => tr!("haul-node"),
        Picked::Nodes(nodes) => tr!("haul-nodes-selected", count = nodes.len()),
        Picked::Mixed(..) | Picked::Nothing => tr!("haul-selection"),
    };
    PropertyTable::new("haul_selection", rect, &title).show(ui, |rows| match picked {
        Picked::Drawing(points) => {
            rows.readonly(&tr!("haul-points"), &points.to_string(), None, None).on_hover_text(tr!("haul-draw-help"));
            rows.note(&tr!("haul-drawing-keys"));
            if rows.action("", &tr!("haul-finish")).clicked() {
                commands.push(UiCommand::FinishHaulRoad);
            }
        }
        Picked::ConfirmDelete(id) => {
            rows.note(&tr!("haul-delete-role-confirm"));
            if rows.action("", &tr!("haul-delete")).clicked() {
                command(commands, session, HaulEdit::DeleteNode(id));
                editor.haul_delete_node = None;
            }
            if rows.action("", &tr!("haul-cancel")).clicked() {
                editor.haul_delete_node = None;
            }
        }
        Picked::Linking(blocks, points) => {
            rows.readonly(&tr!("haul-blocks"), &blocks.to_string(), None, None);
            rows.readonly(&tr!("haul-nodes"), &points.to_string(), None, None);
            rows.note(&tr!("haul-link-picking"));
            if points > 0 && rows.action("", &tr!("haul-join-blocks")).on_hover_text(tr!("haul-join-blocks-help")).clicked() {
                commands.push(UiCommand::LinkHaulBlocks(std::mem::take(&mut editor.haul_link_points)));
            }
            if rows.action("", &tr!("haul-cancel")).clicked() {
                editor.haul_link_pick = false;
                editor.haul_link_points.clear();
                commands.push(UiCommand::RefreshHaulOverlay);
            }
        }
        Picked::Blocks(blocks) => {
            if let [block] = &blocks[..] {
                let length = format!("{:.0}", block.access_m);
                let (access, warning) = match (block.links.is_empty(), block.join) {
                    (false, _) => (tr!("haul-block-joined", length = length), None),
                    (true, Some(_)) if block.connected => (tr!("haul-block-nearest", length = length), None),
                    (true, Some(_)) => (tr!("haul-access-far", length = length.clone()), Some(tr!("haul-block-far", length = length))),
                    (true, None) => (tr!("haul-block-no-roads"), Some(tr!("haul-block-no-roads"))),
                };
                rows.readonly_warning(&tr!("haul-access"), &access, None, warning.as_deref())
                    .on_hover_text(tr!("haul-connected-help"));
                for (index, (node, _)) in block.links.iter().enumerate() {
                    let key = if index == 0 { tr!("haul-joined-to") } else { String::new() };
                    rows.readonly(&key, &node_label(network, destinations, *node), None, None);
                }
            } else {
                let (manual, auto, far) = access_counts(&blocks);
                rows.readonly(&tr!("haul-joined-manually"), &manual.to_string(), None, None);
                rows.readonly(&tr!("haul-joined-nearest"), &auto.to_string(), None, None);
                rows.readonly_warning(
                    &tr!("haul-out-of-reach"),
                    &far.to_string(),
                    None,
                    (far > 0).then(|| tr!("haul-out-of-reach-help")).as_deref(),
                );
            }
            if !network.roads.is_empty() && rows.action("", &tr!("haul-link-pick")).on_hover_text(tr!("haul-link-help")).clicked() {
                editor.haul_link_pick = true;
                editor.haul_link_points.clear();
            }
            if blocks.iter().any(|b| !b.links.is_empty()) && rows.action("", &tr!("haul-link-clear")).on_hover_text(tr!("haul-link-clear-help")).clicked() {
                commands.push(UiCommand::LinkHaulBlocks(Vec::new()));
            }
            if blocks.len() == 1 && route_question(editor, network, destinations).is_none() {
                rows.note(&tr!("haul-route-hint"));
            }
        }
        Picked::Destination(id) => {
            if let Some(destination) = destinations.iter().find(|d| d.id == id) {
                destination_access(rows, editor, network, project, destination, session, commands);
            }
        }
        Picked::Roads(roads) => {
            let current = network.road(roads[0]).and_then(|r| r.speed_limit_kph);
            if let [id] = roads[..] {
                let road = network.road(id).expect("selected road");
                let (_, name) = rows.committed_entry(("haul_road_name", id), &tr!("haul-name"), &road.name, None);
                if let Some(name) = name.filter(|name| !name.trim().is_empty()) {
                    command(commands, session, HaulEdit::RoadProperties(vec![id], Some(name.trim().to_owned()), road.speed_limit_kph));
                }
                let (length, steepest) = road_stats(network, road);
                rows.readonly(&tr!("haul-length"), &format!("{length:.0}"), Some("m"), None);
                rows.readonly(&tr!("haul-steepest"), &format!("{:.1}", steepest * 100.0), Some("%"), None);
            }
            // Empty means no limit, so a limit is cleared the way it is typed.
            let shown = current.map(|v| format!("{v:.0}")).unwrap_or_default();
            let (response, typed) = rows.committed_entry(("haul_speed_limit", roads[0]), &tr!("haul-speed-limit-short"), &shown, Some("km/h"));
            response.on_hover_text(tr!("haul-speed-limit-help"));
            if let Some(typed) = typed {
                let limit = match typed.trim() {
                    "" => Some(None),
                    text => text.parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0).map(|v| Some(v.min(200.0))),
                };
                if let Some(limit) = limit {
                    command(commands, session, HaulEdit::RoadProperties(roads.clone(), None, limit));
                }
            }
            if rows.action("", &tr!("haul-delete")).on_hover_text(tr!("haul-delete-help")).clicked() {
                command(commands, session, HaulEdit::Many(roads.iter().map(|id| HaulEdit::DeleteRoad(*id)).collect()));
            }
        }
        Picked::Nodes(nodes) => {
            if let [id] = nodes[..] {
                let node = network.node(id).expect("selected node");
                let mut choice = RoleChoice::Role(node.role);
                let options = std::iter::once((RoleChoice::Role(None), tr!("haul-role-none")))
                    .chain(
                        destinations
                            .iter()
                            .flat_map(roles_for)
                            .map(|role| (RoleChoice::Role(Some(role)), role_label(destinations, Some(role)))),
                    )
                    .chain([
                        (RoleChoice::New(DestinationKind::Stockpile), tr!("haul-new-stockpile")),
                        (RoleChoice::New(DestinationKind::Dump), tr!("haul-new-dump")),
                        (RoleChoice::New(DestinationKind::Crusher), tr!("haul-new-crusher")),
                    ]);
                let response = rows.combo(("haul_node_role", id), &tr!("haul-role"), &mut choice, &role_label(destinations, node.role), options);
                if response.changed() {
                    match choice {
                        RoleChoice::Role(role) => command(commands, session, HaulEdit::Role(id, role)),
                        RoleChoice::New(kind) => commands.push(UiCommand::NewHaulDestination { node: id, kind }),
                    }
                }
                response.on_hover_text(tr!("haul-role-help"));
                for (axis, label) in ["X", "Y", "Z"].into_iter().enumerate() {
                    let current = format!("{:.2}", node.pos[axis]);
                    let (_, typed) = rows.committed_entry(("haul_node_pos", id, axis), label, &current, Some("m"));
                    if let Some(value) = typed.and_then(|t| t.trim().parse::<f64>().ok()).filter(|v| v.is_finite()) {
                        let mut pos = node.pos;
                        pos[axis] = value;
                        command(commands, session, HaulEdit::MoveNode(id, pos));
                    }
                }
            }
            if let [keep, remove] = nodes[..]
                && rows.action("", &tr!("haul-join-nodes")).on_hover_text(tr!("haul-join-nodes-help")).clicked()
            {
                command(commands, session, HaulEdit::Join(keep, remove));
            }
            if rows.action("", &tr!("haul-delete")).on_hover_text(tr!("haul-delete-help")).clicked() {
                delete(editor, network, &[], &nodes, session, commands);
            }
        }
        Picked::Mixed(roads, nodes) => {
            if rows.action("", &tr!("haul-delete")).on_hover_text(tr!("haul-delete-help")).clicked() {
                delete(editor, network, &roads, &nodes, session, commands);
            }
        }
        Picked::Nothing if network.roads.is_empty() => rows.note(&tr!("haul-empty")),
        Picked::Nothing => rows.note(&tr!("haul-selection-empty")),
    });
}

/// Delete roads and nodes, asking first about a node that is a
/// destination's point.
fn delete(editor: &mut EditorState, network: &HaulNetwork, roads: &[RoadId], nodes: &[NodeId], session: u32, commands: &mut Vec<UiCommand>) {
    let mut edits: Vec<_> = roads.iter().map(|id| HaulEdit::DeleteRoad(*id)).collect();
    for id in nodes {
        if network.node(*id).is_some_and(|n| n.role.is_some()) {
            editor.haul_delete_node = Some(*id);
        } else {
            edits.push(HaulEdit::DeleteNode(*id));
        }
    }
    if !edits.is_empty() {
        command(commands, session, HaulEdit::Many(edits));
    }
}

/// How a dump or stockpile meets the network, chosen in its row: its own
/// node, a node selected beside it, or the road nearest its surface.
fn destination_access(
    rows: &mut PropertyRows<'_>,
    editor: &EditorState,
    network: &HaulNetwork,
    project: &UiProjectView,
    destination: &destinations::DestinationView,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let selected_node = match editor
        .selected_handles
        .iter()
        .filter_map(|h| if let SceneEntityId::HaulNode(id) = h { Some(*id) } else { None })
        .collect::<Vec<_>>()[..]
    {
        [id] => Some(id),
        _ => None,
    };
    let node = network.nodes.iter().find(|n| n.role.is_some_and(|r| r.destination() == destination.id && r.dumps()));
    let centroid = project.haul_points.get(&destination.id).copied();
    let (mut access, current) = if node.is_some() {
        (Access::Node, tr!("haul-method-roads"))
    } else if centroid.is_some() && !network.roads.is_empty() {
        (Access::Nearest, tr!("haul-method-nearest"))
    } else {
        (Access::Unreached, tr!("haul-method-none"))
    };
    let mut options = Vec::new();
    if node.is_some() {
        options.push((Access::Node, tr!("haul-method-roads")));
    }
    if selected_node.is_some_and(|id| node.is_none_or(|n| n.id != id)) {
        options.push((Access::SelectedNode, tr!("haul-use-selected-node")));
    }
    let pin = centroid.and_then(|p| RoadIndex::new(network).candidates(p, 0.0).first().map(|c| c.2));
    if centroid.is_some() && !network.roads.is_empty() {
        options.push((Access::Nearest, tr!("haul-method-nearest")));
    }
    if pin.is_some() {
        options.push((Access::PinNearest, tr!("haul-pin-nearest")));
    }
    let response = rows.combo(("haul_destination_access", destination.id), &tr!("haul-dump-method"), &mut access, &current, options);
    let changed = response.changed();
    response.on_hover_text(tr!("haul-method-help"));
    rows.note(&tr!("haul-route-hint-destination"));
    if !changed {
        return;
    }
    let edit = match access {
        Access::Node | Access::Unreached => None,
        Access::SelectedNode => selected_node.map(|id| HaulEdit::Role(id, Some(arrival_role(network, destination)))),
        Access::Nearest => node.map(|n| HaulEdit::Role(n.id, None)),
        Access::PinNearest => pin.map(|point| HaulEdit::Pin(arrival_role(network, destination), point)),
    };
    if let Some(edit) = edit {
        command(commands, session, edit);
    }
}

fn issues(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, network: &HaulNetwork, destinations: &[destinations::DestinationView], commands: &mut Vec<UiCommand>) {
    let list = editor.haul_issues.clone();
    let count = list.len().to_string();
    let columns = [(tr!("haul-issue"), ISSUE_FRACTIONS[0]), (tr!("haul-where"), ISSUE_FRACTIONS[1])];
    DataGrid::new("haul_issues_grid", rect, &tr!("haul-issues-title"))
        .title_detail(&count)
        .columns(&columns)
        .show(ui, |ui| {
            if list.is_empty() {
                grid_empty_state(ui, &tr!("haul-no-issues"), None);
                return;
            }
            for (index, issue) in list.iter().enumerate() {
                let subject = if issue.blocks.is_empty() {
                    issue
                        .road
                        .and_then(|id| network.road(id).map(|r| r.name.clone()))
                        .or(issue.node.map(|id| node_label(network, destinations, id)))
                        .unwrap_or_default()
                } else {
                    tr!("haul-blocks-in", area = issue.area.clone(), count = issue.blocks.len())
                };
                let entity = issue.road.map(SceneEntityId::HaulRoad).or(issue.node.map(SceneEntityId::HaulNode));
                let selected = if issue.blocks.is_empty() {
                    entity.is_some_and(|e| editor.selected_handles.len() == 1 && editor.selected_handles.contains(&e))
                } else {
                    editor.haul_selected_blocks == issue.blocks
                };
                let help = issue_help(issue.kind);
                let (response, cells) = grid_columns_row(ui, &ISSUE_FRACTIONS, &[&issue.kind.label(), &subject], selected);
                grid_cell_warning(ui, ("haul_issue_mark", index), cells[1], &help);
                if response.on_hover_text(help).clicked() {
                    editor.selected_handles.clear();
                    editor.selected_handles.extend(entity);
                    editor.haul_selected_blocks = issue.blocks.clone();
                    let points = if issue.blocks.is_empty() {
                        issue.road.and_then(|id| network.road(id)).map(|r| network.points(r)).unwrap_or_else(|| vec![issue.pos])
                    } else {
                        editor
                            .haul_blocks
                            .iter()
                            .filter(|b| issue.blocks.contains(&b.id))
                            .flat_map(|b| b.rings.iter().flatten().copied())
                            .collect()
                    };
                    let (min, max) = bounds(&points);
                    commands.push(UiCommand::FrameHaul(min, max));
                }
            }
        });
}

fn issue_help(kind: IssueKind) -> String {
    match kind {
        IssueKind::DeadEnd => tr!("haul-dead-end-help"),
        IssueKind::NearMiss => tr!("haul-near-miss-help"),
        IssueKind::SeparatePiece => tr!("haul-separate-piece-help"),
        IssueKind::SteepRoad => tr!("haul-steep-help"),
        IssueKind::MissingDestination => tr!("haul-missing-destination-help"),
        IssueKind::OutOfReach => tr!("haul-out-of-reach-help"),
    }
}

/// Shares of the Road Network grid's columns: the setting, then its value.
const SETTING_FRACTIONS: [f32; 2] = [0.6, 0.4];

/// Haulage Setup's Road Network step: how roads join and how blocks reach
/// them, one row each, with what a setting does on hover.
pub(crate) fn draw_network_settings(ui: &mut egui::Ui, rect: egui::Rect, network: &HaulNetwork, session: u32, commands: &mut Vec<UiCommand>) {
    let current = network.settings.clone();
    let columns = [(tr!("haul-setting"), SETTING_FRACTIONS[0]), (tr!("planning-value"), SETTING_FRACTIONS[1])];
    let fields: [(String, String, f64, std::ops::RangeInclusive<f64>, &str); 4] = [
        (tr!("haul-join"), tr!("haul-join-help"), current.join_tolerance_m, 0.01..=100.0, "m"),
        (tr!("haul-auto-join"), tr!("haul-auto-join-help"), current.auto_join_m, 1.0..=100_000.0, "m"),
        (tr!("haul-bench-speed"), tr!("haul-bench-speed-help"), current.bench_speed_kph, 1.0..=200.0, "km/h"),
        (tr!("haul-acceleration"), tr!("haul-acceleration-help"), current.acceleration_kph_s, 0.1..=20.0, "km/h/s"),
    ];
    DataGrid::new("haul_network_settings", rect, &tr!("haul-step-network")).columns(&columns).show(ui, |ui| {
        for (index, (label, help, mut value, range, suffix)) in fields.into_iter().enumerate() {
            let (response, cells) = grid_columns_row(ui, &SETTING_FRACTIONS, &[&label, ""], false);
            response.on_hover_text(&help);
            if grid_cell_entry(ui, ("haul_setting", index), cells[1], &mut value, suffix) {
                let mut settings = current.clone();
                *[
                    &mut settings.join_tolerance_m,
                    &mut settings.auto_join_m,
                    &mut settings.bench_speed_kph,
                    &mut settings.acceleration_kph_s,
                ][index] = value.clamp(*range.start(), *range.end());
                command(commands, session, HaulEdit::Settings(settings));
            }
        }
    });
}

fn road_menu(ui: &mut egui::Ui, editor: &EditorState, id: RoadId, session: u32, commands: &mut Vec<UiCommand>) {
    // Near a bend the road already has a point there to make a node of;
    // elsewhere it is cut where it was clicked.
    let (label, at) = match editor.haul_menu_bend {
        Some((road, pos)) if road == id => (tr!("haul-promote-bend"), Some(pos)),
        _ => (tr!("haul-split"), editor.haul_menu_point),
    };
    if let Some(pos) = at
        && ContextMenuAction::new(label).show(ui).clicked()
    {
        command(commands, session, HaulEdit::Split(id, pos));
        ui.close();
    }
    if ContextMenuAction::new(tr!("haul-delete-road")).show(ui).clicked() {
        command(commands, session, HaulEdit::DeleteRoad(id));
        ui.close();
    }
}

fn node_menu(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    network: &HaulNetwork,
    id: NodeId,
    destinations: &[destinations::DestinationView],
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    if ContextMenuAction::new(tr!("haul-promote")).show(ui).clicked() {
        editor.haul_promote = Some(promotion(network, id, destinations));
        editor.canvas_context_menu_open = false;
        ui.close();
    }
    if network.node(id).is_some_and(|n| n.role.is_some()) && ContextMenuAction::new(tr!("haul-clear-role")).show(ui).clicked() {
        command(commands, session, HaulEdit::Role(id, None));
        ui.close();
    }
    let blocker = network.merge_blocker(id);
    let remove = ContextMenuAction::new(tr!("haul-remove-node")).enabled(blocker.is_none()).show(ui);
    let remove = match blocker {
        Some(reason) => remove.on_disabled_hover_text(reason),
        None => remove.on_hover_text(tr!("haul-remove-node-help")),
    };
    if remove.clicked() {
        command(commands, session, HaulEdit::RemoveNode(id));
        ui.close();
    }
    if ContextMenuAction::new(tr!("haul-delete-node")).show(ui).clicked() {
        if network.node(id).is_some_and(|n| n.role.is_some()) {
            editor.haul_delete_node = Some(id);
        } else {
            command(commands, session, HaulEdit::DeleteNode(id));
        }
        ui.close();
    }
}

/// The dialog's starting choice for a node: what it already serves, else
/// the first destination, else a new stockpile.
fn promotion(network: &HaulNetwork, node: NodeId, destinations: &[destinations::DestinationView]) -> HaulPromote {
    let (target, point) = match network.node(node).and_then(|n| n.role) {
        Some(NodeRole::Dump(id)) => (HaulPromoteTarget::Existing(id), HaulPromotePoint::Dump),
        Some(NodeRole::Reclaim(id)) => (HaulPromoteTarget::Existing(id), HaulPromotePoint::Reclaim),
        Some(NodeRole::DumpAndReclaim(id)) => (HaulPromoteTarget::Existing(id), HaulPromotePoint::DumpAndReclaim),
        None => (
            destinations
                .first()
                .map_or(HaulPromoteTarget::New(DestinationKind::Stockpile), |d| HaulPromoteTarget::Existing(d.id)),
            HaulPromotePoint::DumpAndReclaim,
        ),
    };
    HaulPromote { node, target, point }
}

/// Promote to Destination: make a node the point trucks tip at, or load
/// from, for a destination chosen here, or for a new one made at it.
pub(crate) fn draw_promote_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    let Some(mut promote) = editor.haul_promote else { return };
    let destinations = &project.haul_destinations;
    let kind_of = |target: HaulPromoteTarget| match target {
        HaulPromoteTarget::Existing(id) => destinations.iter().find(|d| d.id == id).map(|d| d.kind),
        HaulPromoteTarget::New(kind) => Some(kind),
    };
    let target_label = |target: HaulPromoteTarget| match target {
        HaulPromoteTarget::Existing(id) => destination_name(destinations, id),
        HaulPromoteTarget::New(DestinationKind::Stockpile) => tr!("haul-new-stockpile"),
        HaulPromoteTarget::New(DestinationKind::Dump) => tr!("haul-new-dump"),
        HaulPromoteTarget::New(DestinationKind::Crusher) => tr!("haul-new-crusher"),
    };
    let point_label = |point: HaulPromotePoint| match point {
        HaulPromotePoint::DumpAndReclaim => tr!("haul-dump-and-reclaim"),
        HaulPromotePoint::Dump => tr!("haul-dump"),
        HaulPromotePoint::Reclaim => tr!("haul-reclaim"),
    };
    let mut open = true;
    let mut close = false;
    menu::DragableMenu::new("haul_promote_dialog", tr!("haul-promote-title"))
        .open(&mut open)
        .min_width(340.0)
        .show(ui.ctx(), |ui| {
            let targets = destinations
                .iter()
                .map(|d| HaulPromoteTarget::Existing(d.id))
                .chain([DestinationKind::Stockpile, DestinationKind::Dump, DestinationKind::Crusher].map(HaulPromoteTarget::New));
            let options: Vec<_> = targets.map(|t| (t, egui::WidgetText::from(target_label(t)))).collect();
            let shown = target_label(promote.target);
            menu::MenuFieldCombo::new("haul_promote_target", tr!("haul-destination"), &mut promote.target, shown, options).show(ui);
            // A stockpile is loaded as well as tipped at: which of its points
            // this is. A new one starts as both, until it has a reclaim point.
            let pile = kind_of(promote.target) == Some(DestinationKind::Stockpile);
            if pile && matches!(promote.target, HaulPromoteTarget::Existing(_)) {
                let options: Vec<_> = [HaulPromotePoint::DumpAndReclaim, HaulPromotePoint::Dump, HaulPromotePoint::Reclaim]
                    .map(|p| (p, egui::WidgetText::from(point_label(p))))
                    .into();
                let shown = point_label(promote.point);
                menu::MenuFieldCombo::new("haul_promote_point", tr!("haul-point"), &mut promote.point, shown, options).show(ui);
            }
            menu::menu_actions(ui, |ui| {
                if ui.add(menu::MenuButton::new(tr!("haul-promote-apply")).primary()).clicked() || menu::dialog_confirm_pressed(ui.ctx()) {
                    match promote.target {
                        HaulPromoteTarget::New(kind) => commands.push(UiCommand::NewHaulDestination { node: promote.node, kind }),
                        HaulPromoteTarget::Existing(id) => {
                            let role = match (pile, promote.point) {
                                (true, HaulPromotePoint::DumpAndReclaim) => NodeRole::DumpAndReclaim(id),
                                (true, HaulPromotePoint::Reclaim) => NodeRole::Reclaim(id),
                                _ => NodeRole::Dump(id),
                            };
                            command(commands, project.active_session, HaulEdit::Role(promote.node, Some(role)));
                        }
                    }
                    close = true;
                }
                if ui.add(menu::MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    editor.haul_promote = (open && !close).then_some(promote);
}

/// The canvas right-click menu's haulage part. Only on the Haulage page, and
/// for one thing at a time: actions for each of several selected roads would
/// repeat the same entries.
pub(crate) fn canvas_menu(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.is_haulage_page() {
        return;
    }
    let blocks: Vec<_> = editor
        .haul_selected_blocks
        .iter()
        .filter_map(|id| editor.haul_blocks.iter().find(|b| b.id == *id && b.dug))
        .collect();
    let linked = blocks.iter().any(|b| !b.links.is_empty());
    if !blocks.is_empty() && !project.haulage.roads.is_empty() && ContextMenuAction::new(tr!("haul-link-pick")).show(ui).clicked() {
        editor.haul_link_pick = true;
        editor.haul_link_points.clear();
        editor.canvas_context_menu_open = false;
        ui.close();
    }
    if linked && ContextMenuAction::new(tr!("haul-link-clear")).show(ui).clicked() {
        commands.push(UiCommand::LinkHaulBlocks(Vec::new()));
        ui.close();
    }
    if ContextMenuAction::new(tr!("haul-draw")).show(ui).clicked() {
        commands.push(UiCommand::StartHaulRoad);
        ui.close();
    }
    if editor.selected_handles.iter().any(|h| matches!(h, SceneEntityId::Object(_))) && ContextMenuAction::new(tr!("haul-convert")).show(ui).clicked() {
        commands.push(UiCommand::ConvertHaulSelection);
        ui.close();
    }
    let haul: Vec<_> = editor
        .selected_handles
        .iter()
        .copied()
        .filter(|h| matches!(h, SceneEntityId::HaulRoad(_) | SceneEntityId::HaulNode(_)))
        .collect();
    match haul[..] {
        [SceneEntityId::HaulRoad(id)] => road_menu(ui, editor, id, project.active_session, commands),
        [SceneEntityId::HaulNode(id)] => node_menu(ui, editor, &project.haulage, id, &project.haul_destinations, project.active_session, commands),
        [SceneEntityId::HaulNode(keep), SceneEntityId::HaulNode(remove)] if ContextMenuAction::new(tr!("haul-join-nodes")).show(ui).clicked() => {
            command(commands, project.active_session, HaulEdit::Join(keep, remove));
            ui.close();
        }
        _ => {}
    }
}

/// Where a checked haul starts.
#[derive(Clone, PartialEq)]
enum Origin {
    /// A dig block: its name, loading point and the nodes it is held to.
    Block(String, DVec3, Vec<NodeId>),
    Pile(DestinationId),
}

/// What the selection asks the route check: a haul from a block, or from a
/// stockpile, to a destination.
struct RouteQuestion {
    from: Origin,
    to: DestinationId,
}

/// The haul the selection describes, if it describes one: a dug block and a
/// destination, or a stockpile and somewhere else to take its material. A
/// destination is selected by its node, or by a block of its dump or pile.
fn route_question(editor: &EditorState, network: &HaulNetwork, destinations: &[destinations::DestinationView]) -> Option<RouteQuestion> {
    let blocks: Vec<_> = editor
        .haul_selected_blocks
        .iter()
        .filter_map(|id| editor.haul_blocks.iter().find(|b| b.id == *id))
        .collect();
    let mut named: Vec<DestinationId> = Vec::new();
    let from_nodes = editor.selected_handles.iter().filter_map(|h| match h {
        SceneEntityId::HaulNode(id) => network.node(*id).and_then(|n| n.role).map(NodeRole::destination),
        _ => None,
    });
    let from_blocks = blocks.iter().filter(|b| !b.dug).map(|b| DestinationId::Solid(b.solid));
    for id in from_nodes.chain(from_blocks) {
        if !named.contains(&id) && destinations.iter().any(|d| d.id == id) {
            named.push(id);
        }
    }
    let dug: Vec<_> = blocks.iter().filter(|b| b.dug).collect();
    let pile = |id: DestinationId| destinations.iter().any(|d| d.id == id && d.kind == DestinationKind::Stockpile);
    match (&dug[..], &named[..]) {
        ([block], [to]) => Some(RouteQuestion {
            from: Origin::Block(block.name.clone(), block.point(), block.links.iter().map(|l| l.0).collect()),
            to: *to,
        }),
        ([], [a, b]) if pile(*a) != pile(*b) => {
            let (from, to) = if pile(*a) { (*a, *b) } else { (*b, *a) };
            Some(RouteQuestion { from: Origin::Pile(from), to })
        }
        _ => None,
    }
}

/// The truck class and loader a route is checked with, kept between frames.
#[derive(Clone, Default, PartialEq)]
struct Fleet {
    truck: Option<TruckClassId>,
    loader: Option<LoaderAgentId>,
}

#[allow(clippy::too_many_arguments)]
fn route_check(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    network: &HaulNetwork,
    project: &UiProjectView,
    destinations: &[destinations::DestinationView],
    question: &RouteQuestion,
    commands: &mut Vec<UiCommand>,
) {
    let id = egui::Id::new(("haul_query", project.active_session));
    let mut fleet = ui.data(|d| d.get_temp::<Fleet>(id)).unwrap_or_default();
    let previous_key = ui.data(|d| d.get_temp::<u64>(id.with("key")));
    if fleet.truck.is_none_or(|id| project.schedule.trucks().class(id).is_none()) {
        fleet.truck = project.schedule.trucks().classes.first().map(|c| c.id);
    }
    if fleet.loader.is_none_or(|id| project.schedule.agent(id).is_none()) {
        fleet.loader = project.schedule.agents().first().map(|a| a.id);
    }
    // Recalculated whenever the question or the project changes, so the
    // answer is never stale and there is no button to remember.
    let key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        editor.haul_view_revision.hash(&mut hasher);
        project.active_session.hash(&mut hasher);
        match &question.from {
            Origin::Block(name, p, links) => {
                name.hash(&mut hasher);
                p.to_array().map(f64::to_bits).hash(&mut hasher);
                links.hash(&mut hasher);
            }
            Origin::Pile(id) => id.hash(&mut hasher),
        }
        question.to.hash(&mut hasher);
        fleet.truck.hash(&mut hasher);
        fleet.loader.hash(&mut hasher);
        hasher.finish()
    };
    if previous_key != Some(key) || editor.haul_route.is_none() {
        editor.haul_route = check(network, project, question, &fleet);
        commands.push(UiCommand::RefreshHaulOverlay);
    }
    PropertyTable::new("haul_route_check", rect, &tr!("haul-route-check")).show(ui, |rows| {
        let from = match &question.from {
            Origin::Block(name, ..) => name.clone(),
            Origin::Pile(pile) => tr!("haul-from-pile", pile = destination_name(destinations, *pile)),
        };
        rows.readonly(&tr!("haul-from"), &from, None, None);
        rows.readonly(&tr!("haul-to"), &destination_name(destinations, question.to), None, None);
        let trucks = project.schedule.trucks();
        let truck_text = fleet.truck.and_then(|id| trucks.class(id)).map(|c| c.name.clone()).unwrap_or_default();
        rows.combo(
            "haul_truck",
            &tr!("haul-truck"),
            &mut fleet.truck,
            &truck_text,
            trucks.classes.iter().map(|c| (Some(c.id), c.name.clone())),
        );
        let loader_text = fleet.loader.and_then(|id| project.schedule.agent(id)).map(|a| a.name.clone()).unwrap_or_default();
        rows.combo(
            "haul_loader",
            &tr!("haul-loader"),
            &mut fleet.loader,
            &loader_text,
            project.schedule.agents().iter().map(|a| (Some(a.id), a.name.clone())),
        );
        if trucks.classes.is_empty() {
            rows.note(&tr!("haul-need-truck"));
        } else if project.schedule.agents().is_empty() {
            rows.note(&tr!("haul-need-loader"));
        }
        if let Some(route) = &editor.haul_route {
            result(rows, route, project, question, &fleet);
        }
    });
    ui.data_mut(|d| {
        d.insert_temp(id, fleet);
        d.insert_temp(id.with("key"), key);
    });
}

fn result(rows: &mut PropertyRows<'_>, route: &RouteCheck, project: &UiProjectView, question: &RouteQuestion, fleet: &Fleet) {
    let cycle = route.cycle;
    let minutes = |h: f64| format!("{:.1}", h * 60.0);
    if !route.uses_roads {
        rows.readonly_warning(&tr!("haul-cycle-time"), &tr!("haul-no-route-short"), None, Some(&tr!("haul-no-route")));
        return;
    }
    let warning = if !route.connected {
        Some(tr!(
            "haul-unconnected",
            length = format!("{:.0}", route.access_m),
            rise = format!("{:+.0}", route.access_rise_m)
        ))
    } else {
        None
    };
    rows.readonly_warning(&tr!("haul-cycle-time"), &minutes(cycle.total_h()), Some("min"), warning.as_deref());
    if let Some(class) = fleet.truck.and_then(|id| project.schedule.trucks().class(id))
        && cycle.total_h() > 0.0
    {
        let per_truck = class.payload_t / cycle.total_h();
        rows.readonly(&tr!("haul-per-truck-short"), &format!("{per_truck:.0}"), Some("t/h"), None);
        let loader = fleet.loader.and_then(|id| project.schedule.agent(id)).and_then(|a| project.schedule.class(a.class_id));
        let rate = loader.map(|l| {
            if matches!(question.from, Origin::Pile(_)) {
                l.default_reclaim_rate_tph
            } else {
                l.default_dig_rate_tph
            }
        });
        if let Some(rate) = rate {
            rows.readonly(&tr!("haul-match-short"), &format!("{:.1}", rate / per_truck), None, None)
                .on_hover_text(tr!("haul-match-help"));
        }
    }
    for (label, hours) in [
        (tr!("haul-spot"), cycle.spot_h),
        (tr!("haul-load"), cycle.load_h),
        (tr!("haul-loaded"), cycle.loaded_h),
        (tr!("haul-dump"), cycle.dump_h),
        (tr!("haul-return"), cycle.empty_h),
    ] {
        rows.readonly(&label, &minutes(hours), Some("min"), None);
    }
    rows.readonly(&tr!("haul-loaded-distance"), &format!("{:.2}", cycle.loaded_km), Some("km"), None);
    rows.readonly(&tr!("haul-return-distance"), &format!("{:.2}", cycle.empty_km), Some("km"), None);
    rows.readonly(&tr!("haul-rise-short"), &format!("{:.0}", cycle.rise_m), Some("m"), None);
    if route.uses_roads && route.connected && route.grade_lengthened {
        rows.note(&tr!(
            "haul-lengthened",
            length = format!("{:.0}", route.access_m),
            rise = format!("{:+.0}", route.access_rise_m)
        ));
    }
    if route.profile.len() >= 2 {
        rows.wide(PROFILE_HEIGHT, |ui, rect| profile(ui, rect, &route.profile));
    }
}

fn check(network: &HaulNetwork, project: &UiProjectView, question: &RouteQuestion, fleet: &Fleet) -> Option<RouteCheck> {
    let destination = question.to;
    let class = project.schedule.trucks().class(fleet.truck?)?;
    let loader = project.schedule.agent(fleet.loader?).and_then(|a| project.schedule.class(a.class_id))?;
    let (source, link, rate, bench) = match &question.from {
        Origin::Block(_, point, link) => (Some(*point), link.as_slice(), loader.default_dig_rate_tph, true),
        Origin::Pile(pile) => (
            network.destination_point(*pile, true, project.haul_points.get(pile).copied()),
            &[][..],
            loader.default_reclaim_rate_tph,
            false,
        ),
    };
    let dump = project.schedule.routing().dump_time_s(destination);
    let index = RoadIndex::new(network);
    let routed = source
        .zip(network.destination_point(destination, false, project.haul_points.get(&destination).copied()))
        .and_then(|(source, target)| DestinationSearch::new(network, &index, class, target)?.route(&index, source, link, bench, rate, loader.spot_time_s, dump));
    // No route is an answer too: the haul cannot be made, and the check
    // says so rather than inventing a distance.
    Some(routed.unwrap_or_else(|| RouteCheck {
        cycle: crate::model::schedule::trucking::CycleBreakdown::default(),
        loaded_path: Vec::new(),
        empty_path: Vec::new(),
        connected: false,
        access_m: 0.0,
        access_rise_m: 0.0,
        grade_lengthened: false,
        profile: Vec::new(),
        uses_roads: false,
    }))
}

/// Elevation (line) and speed (shaded steps) along the loaded haul, with the
/// figures under the pointer on hover.
fn profile(ui: &mut egui::Ui, rect: egui::Rect, points: &[[f64; 3]]) {
    let response = ui.interact(rect, ui.id().with("haul_profile"), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    painter.rect_filled(rect, crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS, visuals.extreme_bg_color);
    let plot = rect.shrink2(egui::vec2(4.0, 14.0));
    let length = points.last().map_or(0.0, |p| p[0]).max(1e-6);
    let (z_lo, z_hi) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
    let z_span = (z_hi - z_lo).max(1.0);
    let v_hi = points.iter().map(|p| p[2]).fold(1.0, f64::max);
    let x = |d: f64| plot.left() + (d / length) as f32 * plot.width();
    let speed_color = visuals.selection.bg_fill.gamma_multiply(0.45);
    for pair in points.windows(2) {
        let top = plot.bottom() - (pair[1][2] / v_hi) as f32 * plot.height();
        let span = egui::Rect::from_min_max(egui::pos2(x(pair[0][0]), top), egui::pos2(x(pair[1][0]).max(x(pair[0][0]) + 0.5), plot.bottom()));
        painter.rect_filled(span, 0.0, speed_color);
    }
    let line: Vec<_> = points
        .iter()
        .map(|p| egui::pos2(x(p[0]), plot.bottom() - ((p[1] - z_lo) / z_span) as f32 * plot.height()))
        .collect();
    painter.add(egui::Shape::line(line, egui::Stroke::new(2.0, egui::Color32::from_rgb(230, 120, 70))));
    let small = egui::TextStyle::Small.resolve(ui.style());
    let weak = visuals.weak_text_color();
    painter.text(
        rect.left_top() + egui::vec2(4.0, 1.0),
        egui::Align2::LEFT_TOP,
        format!("{z_hi:.0} m RL"),
        small.clone(),
        weak,
    );
    painter.text(
        rect.left_bottom() + egui::vec2(4.0, -1.0),
        egui::Align2::LEFT_BOTTOM,
        format!("{z_lo:.0} m RL"),
        small.clone(),
        weak,
    );
    painter.text(
        rect.right_bottom() + egui::vec2(-4.0, -1.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{:.2} km", length / 1000.0),
        small.clone(),
        weak,
    );
    painter.text(rect.right_top() + egui::vec2(-4.0, 1.0), egui::Align2::RIGHT_TOP, tr!("haul-profile-legend"), small, weak);
    if let Some(pointer) = response.hover_pos() {
        let distance = f64::from((pointer.x - plot.left()) / plot.width()).clamp(0.0, 1.0) * length;
        if let Some(pair) = points.windows(2).find(|p| p[1][0] >= distance) {
            let t = if pair[1][0] > pair[0][0] {
                (distance - pair[0][0]) / (pair[1][0] - pair[0][0])
            } else {
                0.0
            };
            let z = pair[0][1] + (pair[1][1] - pair[0][1]) * t;
            painter.line_segment([egui::pos2(pointer.x, plot.top()), egui::pos2(pointer.x, plot.bottom())], egui::Stroke::new(1.0, weak));
            response.on_hover_text(tr!(
                "haul-profile-hover",
                distance = format!("{distance:.0}"),
                elevation = format!("{z:.1}"),
                speed = format!("{:.0}", pair[1][2])
            ));
        }
    }
}
