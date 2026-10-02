//! Haulage layout: network tools, issues, destination points and route checks.
//!
//! The panel reads top to bottom as a planner works: tools, what is selected,
//! what is wrong, the roads, how each destination is reached, then the route
//! check. Figures come from the active project's network and are summarised
//! on the row; the detail behind them is on hover.
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
        state::{HaulEdit, UiCommand},
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup, context_submenu},
            island::{Island, IslandResponse, Side},
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

/// A full-width list row: the label on the left, a weak figure on the right.
fn row(ui: &mut egui::Ui, label: &str, figure: &str, selected: bool, warn: bool) -> egui::Response {
    let height = ui.spacing().interact_size.y + 2.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
    let visuals = ui.visuals();
    if selected {
        ui.painter().rect_filled(rect, crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS, visuals.selection.bg_fill);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS, visuals.widgets.hovered.bg_fill);
    }
    let text = if selected { visuals.selection.stroke.color } else { visuals.text_color() };
    let weak = if selected { text } else { visuals.weak_text_color() };
    let figure_galley = ui.painter().layout_no_wrap(figure.to_owned(), egui::TextStyle::Small.resolve(ui.style()), weak);
    let label_width = (rect.width() - figure_galley.size().x - 24.0).max(0.0);
    let label_galley = egui::WidgetText::from(egui::RichText::new(label).color(if warn { visuals.warn_fg_color } else { text })).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        label_width,
        egui::TextStyle::Body,
    );
    ui.painter()
        .galley(egui::pos2(rect.left() + 6.0, rect.center().y - label_galley.size().y * 0.5), label_galley, text);
    ui.painter().galley(
        egui::pos2(rect.right() - 6.0 - figure_galley.size().x, rect.center().y - figure_galley.size().y * 0.5),
        figure_galley,
        weak,
    );
    response
}

fn section(ui: &mut egui::Ui, id: &str, title: String, open: bool, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(crate::ui::fonts::bold(&title)).id_salt(id).default_open(open).show(ui, add);
}

/// A number edited by dragging or typing, committed once: when the drag or
/// the typing ends. Committing every frame of a drag would flood undo with
/// one step per pixel.
fn committed_number(ui: &mut egui::Ui, id: egui::Id, value: f64, range: std::ops::RangeInclusive<f64>, speed: f64, suffix: &str) -> Option<f64> {
    let mut draft = ui.data(|d| d.get_temp::<f64>(id)).unwrap_or(value);
    let response = ui.add(egui::DragValue::new(&mut draft).range(range).speed(speed).suffix(suffix).max_decimals(2));
    let editing = response.dragged() || response.has_focus();
    if editing {
        ui.data_mut(|d| d.insert_temp(id, draft));
        None
    } else {
        ui.data_mut(|d| d.remove::<f64>(id));
        ((response.drag_stopped() || response.lost_focus() || response.changed()) && draft != value).then_some(draft)
    }
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

pub(crate) fn draw_panel(ui: &mut egui::Ui, editor: &mut EditorState, _document: &Document, project: &UiProjectView, commands: &mut Vec<UiCommand>) -> IslandResponse<()> {
    Island::new("haulage_layout", Side::Right)
        .fill(crate::ui::widgets::tree_row_colors(ui).0)
        .default_width(320.0)
        .min_width(260.0)
        .max_width(520.0)
        .show(ui, |ui, _| {
            let network = &project.haulage;
            let session = project.active_session;
            let destinations = project.haul_destinations.clone();
            tools(ui, editor, network, commands);
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                summary(ui, editor, network);
                selection(ui, editor, network, &destinations, session, commands);
                issues(ui, editor, network, &destinations, commands);
                roads(ui, editor, network, session, commands);
                destination_access(ui, editor, network, project, &destinations, session, commands);
                route_check(ui, editor, network, project, &destinations, commands);
                settings(ui, network, session, commands);
            });
        })
}

fn tools(ui: &mut egui::Ui, editor: &EditorState, network: &HaulNetwork, commands: &mut Vec<UiCommand>) {
    ui.label(crate::ui::fonts::bold(&tr!("haul-network")));
    ui.horizontal_wrapped(|ui| {
        let draw = if editor.haul_draw { tr!("haul-finish") } else { tr!("haul-draw") };
        if ui.add(egui::Button::new(draw).selected(editor.haul_draw)).on_hover_text(tr!("haul-draw-help")).clicked() {
            commands.push(if editor.haul_draw { UiCommand::FinishHaulRoad } else { UiCommand::StartHaulRoad });
        }
        let objects = editor.selected_handles.iter().any(|h| matches!(h, SceneEntityId::Object(_)));
        if ui
            .add_enabled(objects, egui::Button::new(tr!("haul-convert-short")))
            .on_hover_text(tr!("haul-convert-help"))
            .on_disabled_hover_text(tr!("haul-convert-help"))
            .clicked()
        {
            commands.push(UiCommand::ConvertHaulSelection);
        }
        ui.menu_button(tr!("haul-dxf"), |ui| {
            if ui.button(tr!("haul-import")).clicked() {
                commands.push(UiCommand::OpenHaulImport);
                ui.close();
            }
            if ui.add_enabled(!network.roads.is_empty(), egui::Button::new(tr!("haul-export"))).clicked() {
                commands.push(UiCommand::ExportHaulRoads);
                ui.close();
            }
        });
    });
    if editor.haul_draw {
        ui.label(egui::RichText::new(tr!("haul-drawing-hint", points = editor.haul_points.len().to_string())).weak().small());
    }
}

fn summary(ui: &mut egui::Ui, editor: &EditorState, network: &HaulNetwork) {
    if network.roads.is_empty() {
        ui.label(egui::RichText::new(tr!("haul-empty")).weak());
        return;
    }
    let length_km: f64 = network.roads.iter().map(|r| road_stats(network, r).0).sum::<f64>() / 1000.0;
    let mut text = tr!("haul-summary", roads = network.roads.len(), length = format!("{length_km:.1}"));
    if !editor.haul_blocks.is_empty() {
        text = format!(
            "{text} · {}",
            tr!(
                "haul-connected",
                connected = editor.haul_blocks.iter().filter(|(_, c)| *c).count().to_string(),
                total = editor.haul_blocks.len().to_string()
            )
        );
    }
    ui.label(egui::RichText::new(text).weak()).on_hover_text(tr!("haul-connected-help"));
}

fn selection(ui: &mut egui::Ui, editor: &mut EditorState, network: &HaulNetwork, destinations: &[destinations::DestinationView], session: u32, commands: &mut Vec<UiCommand>) {
    if let Some(id) = editor.haul_delete_node {
        ui.group(|ui| {
            ui.label(tr!("haul-delete-role-confirm"));
            ui.horizontal(|ui| {
                if ui.button(tr!("haul-delete")).clicked() {
                    command(commands, session, HaulEdit::DeleteNode(id));
                    editor.haul_delete_node = None;
                }
                if ui.button(tr!("haul-cancel")).clicked() {
                    editor.haul_delete_node = None;
                }
            });
        });
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
    if roads.is_empty() && nodes.is_empty() {
        return;
    }
    ui.add_space(4.0);
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        if let [id] = roads[..] {
            let road = network.road(id).expect("selected road");
            ui.label(crate::ui::fonts::bold(&tr!("haul-road")));
            let draft_id = ui.id().with(("road_name", id));
            let mut name = ui.data(|d| d.get_temp::<String>(draft_id)).unwrap_or_else(|| road.name.clone());
            let response = ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
            if response.lost_focus() {
                if !name.trim().is_empty() && name.trim() != road.name {
                    command(commands, session, HaulEdit::RoadProperties(vec![id], Some(name.clone()), road.speed_limit_kph));
                }
                ui.data_mut(|d| d.remove::<String>(draft_id));
            } else if response.has_focus() {
                ui.data_mut(|d| d.insert_temp(draft_id, name));
            }
            let (length, steepest) = road_stats(network, road);
            ui.label(egui::RichText::new(tr!("haul-road-stats", length = format!("{length:.0}"), grade = format!("{:.1}", steepest * 100.0))).weak());
        } else if !roads.is_empty() {
            ui.label(crate::ui::fonts::bold(&tr!("haul-roads-selected", count = roads.len())));
        }
        if !roads.is_empty() {
            let current = network.road(roads[0]).and_then(|r| r.speed_limit_kph);
            ui.horizontal(|ui| {
                let mut limited = current.is_some();
                if ui.checkbox(&mut limited, tr!("haul-speed-limit")).changed() {
                    command(commands, session, HaulEdit::RoadProperties(roads.clone(), None, limited.then_some(current.unwrap_or(30.0))));
                }
                if let Some(speed) = current
                    && let Some(speed) = committed_number(ui, ui.id().with("speed_limit"), speed, 1.0..=200.0, 0.5, " km/h")
                {
                    command(commands, session, HaulEdit::RoadProperties(roads.clone(), None, Some(speed)));
                }
            });
            if roads.len() == 1 && !network.road(roads[0]).is_some_and(|r| r.verts.is_empty()) {
                ui.label(egui::RichText::new(tr!("haul-shape-hint")).weak().small());
            }
        }
        if let [id] = nodes[..] {
            let node = network.node(id).expect("selected node");
            ui.label(crate::ui::fonts::bold(&tr!("haul-node")));
            egui::ComboBox::from_id_salt("haul_node_role")
                .width(ui.available_width() - 8.0)
                .height(480.0)
                .selected_text(role_label(destinations, node.role))
                .show_ui(ui, |ui| {
                    if ui.selectable_label(node.role.is_none(), tr!("haul-role-none")).clicked() {
                        command(commands, session, HaulEdit::Role(id, None));
                    }
                    for destination in destinations {
                        let role = NodeRole::Dump(destination.id);
                        if ui.selectable_label(node.role == Some(role), role_label(destinations, Some(role))).clicked() {
                            command(commands, session, HaulEdit::Role(id, Some(role)));
                        }
                    }
                    for destination in destinations.iter().filter(|d| d.kind == DestinationKind::Stockpile) {
                        let role = NodeRole::Reclaim(destination.id);
                        if ui.selectable_label(node.role == Some(role), role_label(destinations, Some(role))).clicked() {
                            command(commands, session, HaulEdit::Role(id, Some(role)));
                        }
                    }
                    ui.separator();
                    for (label, kind) in [
                        (tr!("haul-new-stockpile"), DestinationKind::Stockpile),
                        (tr!("haul-new-dump"), DestinationKind::Dump),
                        (tr!("haul-new-crusher"), DestinationKind::Crusher),
                    ] {
                        if ui.selectable_label(false, label).clicked() {
                            commands.push(UiCommand::NewHaulDestination { node: id, kind });
                        }
                    }
                })
                .response
                .on_hover_text(tr!("haul-role-help"));
            let mut pos = node.pos;
            if coordinates(ui, ui.id().with(("node_pos", id)), &mut pos) {
                command(commands, session, HaulEdit::MoveNode(id, pos));
            }
        } else if !nodes.is_empty() {
            ui.label(crate::ui::fonts::bold(&tr!("haul-nodes-selected", count = nodes.len())));
        }
        ui.horizontal(|ui| {
            if let [keep, remove] = nodes[..]
                && ui.button(tr!("haul-join-nodes")).on_hover_text(tr!("haul-join-help")).clicked()
            {
                command(commands, session, HaulEdit::Join(keep, remove));
            }
            if ui.button(tr!("haul-delete")).on_hover_text(tr!("haul-delete-help")).clicked() {
                let mut edits: Vec<_> = roads.iter().map(|id| HaulEdit::DeleteRoad(*id)).collect();
                for id in &nodes {
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
        });
    });
}

fn issues(ui: &mut egui::Ui, editor: &mut EditorState, network: &HaulNetwork, destinations: &[destinations::DestinationView], commands: &mut Vec<UiCommand>) {
    if editor.haul_issues.is_empty() {
        return;
    }
    let list = editor.haul_issues.clone();
    section(ui, "haul_issues", tr!("haul-issues", count = list.len().to_string()), true, |ui| {
        for issue in &list {
            let kind = issue_kind(issue.kind);
            let subject = issue
                .road
                .and_then(|id| network.road(id).map(|r| r.name.clone()))
                .or(issue.node.map(|id| node_label(network, destinations, id)))
                .unwrap_or_default();
            if row(ui, &kind, &subject, false, true).on_hover_text(issue_help(issue.kind)).clicked() {
                editor.selected_handles.clear();
                let entity = issue.road.map(SceneEntityId::HaulRoad).or(issue.node.map(SceneEntityId::HaulNode));
                editor.selected_handles.extend(entity);
                let points = issue.road.and_then(|id| network.road(id)).map(|r| network.points(r)).unwrap_or_else(|| vec![issue.pos]);
                let (min, max) = bounds(&points);
                commands.push(UiCommand::FrameHaul(min, max));
            }
        }
    });
}

fn issue_kind(kind: IssueKind) -> String {
    match kind {
        IssueKind::DeadEnd => tr!("haul-dead-end"),
        IssueKind::NearMiss => tr!("haul-near-miss"),
        IssueKind::SeparatePiece => tr!("haul-separate-piece"),
        IssueKind::SteepRoad => tr!("haul-steep"),
        IssueKind::MissingDestination => tr!("haul-missing-destination"),
    }
}

fn issue_help(kind: IssueKind) -> String {
    match kind {
        IssueKind::DeadEnd => tr!("haul-dead-end-help"),
        IssueKind::NearMiss => tr!("haul-near-miss-help"),
        IssueKind::SeparatePiece => tr!("haul-separate-piece-help"),
        IssueKind::SteepRoad => tr!("haul-steep-help"),
        IssueKind::MissingDestination => tr!("haul-missing-destination-help"),
    }
}

fn roads(ui: &mut egui::Ui, editor: &mut EditorState, network: &HaulNetwork, session: u32, commands: &mut Vec<UiCommand>) {
    if network.roads.is_empty() {
        return;
    }
    section(ui, "haul_roads", tr!("haul-roads-count", count = network.roads.len().to_string()), false, |ui| {
        for road in &network.roads {
            let entity = SceneEntityId::HaulRoad(road.id);
            let (length, steepest) = road_stats(network, road);
            let figure = format!("{length:.0} m · {:.1}%", steepest * 100.0);
            let response = row(ui, &road.name, &figure, editor.selected_handles.contains(&entity), false).on_hover_text(
                road.speed_limit_kph
                    .map(|v| tr!("haul-limit-hover", speed = format!("{v:.0}")))
                    .unwrap_or_else(|| tr!("haul-no-limit")),
            );
            if response.clicked() {
                if !ui.input(|i| i.modifiers.shift || i.modifiers.command) {
                    editor.selected_handles.clear();
                }
                editor.selected_handles.insert(entity);
                let (min, max) = bounds(&network.points(road));
                commands.push(UiCommand::FrameHaul(min, max));
            }
            context_menu_popup(&response, &road.name, |ui| road_menu(ui, editor, road.id, session, commands));
        }
    });
}

/// How each destination meets the network, chosen inline rather than hidden
/// in a menu: its own node, the nearest road, or its fixed distance.
fn destination_access(
    ui: &mut egui::Ui,
    editor: &EditorState,
    network: &HaulNetwork,
    project: &UiProjectView,
    destinations: &[destinations::DestinationView],
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    if destinations.is_empty() {
        return;
    }
    let selected_node = match editor
        .selected_handles
        .iter()
        .filter_map(|h| if let SceneEntityId::HaulNode(id) = h { Some(*id) } else { None })
        .collect::<Vec<_>>()[..]
    {
        [id] => Some(id),
        _ => None,
    };
    section(ui, "haul_destinations", tr!("haul-destinations"), true, |ui| {
        egui::Grid::new("haul_destination_grid").num_columns(2).striped(false).show(ui, |ui| {
            for destination in destinations {
                let fixed = network.fixed_destinations.contains(&destination.id);
                let node = network.nodes.iter().find(|n| n.role == Some(NodeRole::Dump(destination.id)));
                let centroid = project.haul_points.get(&destination.id).copied();
                let on_fixed = fixed || network.roads.is_empty() || (node.is_none() && centroid.is_none());
                let current = if fixed || network.roads.is_empty() {
                    tr!("haul-method-fixed", distance = format!("{:.1}", destination.distance_km))
                } else if node.is_some() {
                    tr!("haul-method-roads")
                } else if centroid.is_some() {
                    tr!("haul-method-nearest")
                } else {
                    tr!("haul-method-fixed", distance = format!("{:.1}", destination.distance_km))
                };
                let label = ui
                    .add(egui::Label::new(&destination.name).sense(egui::Sense::click()))
                    .on_hover_text(tr!("haul-destination-frame"));
                if label.clicked()
                    && let Some(point) = node.map(|n| n.pos).or(centroid)
                {
                    commands.push(UiCommand::FrameHaul(point, point));
                }
                egui::ComboBox::from_id_salt(("haul_destination_method", destination.id))
                    .width(150.0)
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        if let Some(id) = selected_node
                            && ui.selectable_label(node.is_some_and(|n| n.id == id), tr!("haul-use-selected-node")).clicked()
                        {
                            command(
                                commands,
                                session,
                                HaulEdit::Many(vec![HaulEdit::Fixed(destination.id, false), HaulEdit::Role(id, Some(NodeRole::Dump(destination.id)))]),
                            );
                        }
                        if centroid.is_some() {
                            if ui.selectable_label(!fixed && node.is_none(), tr!("haul-method-nearest")).clicked() {
                                let mut edits = vec![HaulEdit::Fixed(destination.id, false)];
                                edits.extend(node.map(|n| HaulEdit::Role(n.id, None)));
                                command(commands, session, HaulEdit::Many(edits));
                            }
                            if let Some(point) = centroid.and_then(|p| RoadIndex::new(network).candidates(p, 0.0).first().map(|c| c.2))
                                && ui.selectable_label(false, tr!("haul-pin-nearest")).on_hover_text(tr!("haul-pin-help")).clicked()
                            {
                                command(commands, session, HaulEdit::Pin(destination.id, point));
                            }
                        }
                        if ui
                            .selectable_label(on_fixed, tr!("haul-method-fixed", distance = format!("{:.1}", destination.distance_km)))
                            .on_hover_text(tr!("haul-fixed-help"))
                            .clicked()
                        {
                            let mut edits = vec![HaulEdit::Fixed(destination.id, true)];
                            edits.extend(
                                network
                                    .nodes
                                    .iter()
                                    .filter(|n| n.role == Some(NodeRole::Dump(destination.id)) || n.role == Some(NodeRole::Reclaim(destination.id)))
                                    .map(|n| HaulEdit::Role(n.id, None)),
                            );
                            command(commands, session, HaulEdit::Many(edits));
                        }
                    })
                    .response
                    .on_hover_text(tr!("haul-method-help"));
                ui.end_row();
            }
        });
        if selected_node.is_none() {
            ui.label(egui::RichText::new(tr!("haul-destinations-hint")).weak().small());
        }
    });
}

fn settings(ui: &mut egui::Ui, network: &HaulNetwork, session: u32, commands: &mut Vec<UiCommand>) {
    section(ui, "haul_settings", tr!("haul-settings"), false, |ui| {
        egui::Grid::new("haul_settings_grid").num_columns(2).show(ui, |ui| {
            let current = network.settings.clone();
            let fields: [(String, String, f64, std::ops::RangeInclusive<f64>, &str); 4] = [
                (tr!("haul-join"), tr!("haul-join-help"), current.join_tolerance_m, 0.01..=100.0, " m"),
                (tr!("haul-auto-join"), tr!("haul-auto-join-help"), current.auto_join_m, 1.0..=100_000.0, " m"),
                (tr!("haul-bench-speed"), tr!("haul-bench-speed-help"), current.bench_speed_kph, 1.0..=200.0, " km/h"),
                (tr!("haul-acceleration"), tr!("haul-acceleration-help"), current.acceleration_kph_s, 0.1..=20.0, " km/h/s"),
            ];
            for (index, (label, help, value, range, suffix)) in fields.into_iter().enumerate() {
                ui.label(label).on_hover_text(help);
                if let Some(value) = committed_number(ui, ui.id().with(("haul_setting", index)), value, range, 0.1, suffix) {
                    let mut settings = current.clone();
                    *[
                        &mut settings.join_tolerance_m,
                        &mut settings.auto_join_m,
                        &mut settings.bench_speed_kph,
                        &mut settings.acceleration_kph_s,
                    ][index] = value;
                    command(commands, session, HaulEdit::Settings(settings));
                }
                ui.end_row();
            }
        });
    });
}

fn road_menu(ui: &mut egui::Ui, editor: &EditorState, id: RoadId, session: u32, commands: &mut Vec<UiCommand>) {
    if let Some(pos) = editor.haul_menu_point
        && ContextMenuAction::new(tr!("haul-split")).show(ui).clicked()
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
    for (title, reclaim) in [(tr!("haul-dump-point"), false), (tr!("haul-reclaim-point"), true)] {
        context_submenu(ui, &title, true, |ui| {
            for destination in destinations.iter().filter(|d| !reclaim || d.kind == DestinationKind::Stockpile) {
                if ContextMenuAction::new(&destination.name).show(ui).clicked() {
                    command(
                        commands,
                        session,
                        HaulEdit::Role(id, Some(if reclaim { NodeRole::Reclaim(destination.id) } else { NodeRole::Dump(destination.id) })),
                    );
                    ui.close();
                }
            }
        });
    }
    for (label, kind) in [
        (tr!("haul-new-stockpile"), DestinationKind::Stockpile),
        (tr!("haul-new-dump"), DestinationKind::Dump),
        (tr!("haul-new-crusher"), DestinationKind::Crusher),
    ] {
        if ContextMenuAction::new(label).show(ui).clicked() {
            commands.push(UiCommand::NewHaulDestination { node: id, kind });
            ui.close();
        }
    }
    if network.node(id).is_some_and(|n| n.role.is_some()) && ContextMenuAction::new(tr!("haul-clear-role")).show(ui).clicked() {
        command(commands, session, HaulEdit::Role(id, None));
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

/// The canvas right-click menu's haulage part. Only on the Haulage page, and
/// for one thing at a time: actions for each of several selected roads would
/// repeat the same entries.
pub(crate) fn canvas_menu(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    if !editor.is_haulage_page() {
        return;
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

fn coordinates(ui: &mut egui::Ui, id: egui::Id, pos: &mut DVec3) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (index, (label, value)) in [("X", &mut pos.x), ("Y", &mut pos.y), ("Z", &mut pos.z)].into_iter().enumerate() {
            ui.label(egui::RichText::new(label).weak());
            if let Some(next) = committed_number(ui, id.with(index), *value, f64::MIN..=f64::MAX, 0.1, "") {
                *value = next;
                changed = true;
            }
        }
    });
    changed
}

#[derive(Clone, Default, PartialEq)]
enum Origin {
    #[default]
    Unset,
    Block(String, DVec3),
    Pile(DestinationId),
}

#[derive(Clone, Default, PartialEq)]
struct Query {
    from: Origin,
    destination: Option<DestinationId>,
    truck: Option<TruckClassId>,
    loader: Option<LoaderAgentId>,
}

/// Picks a sensible starting value for each empty field, so a planner sees a
/// result as soon as they click a block.
fn fill_defaults(q: &mut Query, project: &UiProjectView, destinations: &[destinations::DestinationView]) {
    if q.destination.is_none_or(|id| !destinations.iter().any(|d| d.id == id)) {
        q.destination = destinations.iter().find(|d| d.kind == DestinationKind::Crusher).or(destinations.first()).map(|d| d.id);
    }
    if q.truck.is_none_or(|id| project.schedule.trucks().class(id).is_none()) {
        q.truck = project.schedule.trucks().classes.first().map(|c| c.id);
    }
    if q.loader.is_none_or(|id| project.schedule.agent(id).is_none()) {
        q.loader = project.schedule.agents().first().map(|a| a.id);
    }
}

fn route_check(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    network: &HaulNetwork,
    project: &UiProjectView,
    destinations: &[destinations::DestinationView],
    commands: &mut Vec<UiCommand>,
) {
    section(ui, "haul_route_check", tr!("haul-route-check"), true, |ui| {
        let id = ui.id().with(("haul_query", project.active_session));
        let mut q = ui.data(|d| d.get_temp::<Query>(id)).unwrap_or_default();
        let previous_key = ui.data(|d| d.get_temp::<u64>(id.with("key")));
        if let Some(point) = editor.haul_source_point.take() {
            let name = editor
                .haul_blocks
                .iter()
                .find(|(b, _)| (b.anchor[0] - point.x).abs() < 1e-6 && (b.anchor[1] - point.y).abs() < 1e-6)
                .map(|(b, _)| b.name.clone())
                .unwrap_or_else(|| format!("{:.0}, {:.0}", point.x, point.y));
            q.from = Origin::Block(name, point);
        }
        fill_defaults(&mut q, project, destinations);
        egui::Grid::new("haul_query_grid").num_columns(2).show(ui, |ui| {
            ui.label(tr!("haul-from"));
            let from_text = match &q.from {
                Origin::Unset => tr!("haul-from-pick"),
                Origin::Block(name, _) => name.clone(),
                Origin::Pile(pile) => tr!("haul-from-pile", pile = destination_name(destinations, *pile)),
            };
            egui::ComboBox::from_id_salt("haul_from").width(190.0).selected_text(from_text).show_ui(ui, |ui| {
                for (block, _) in &editor.haul_blocks {
                    let point = DVec3::new(block.anchor[0], block.anchor[1], block.plane);
                    let this = Origin::Block(block.name.clone(), point);
                    if ui.selectable_label(q.from == this, &block.name).clicked() {
                        q.from = this;
                    }
                }
                for d in destinations.iter().filter(|d| d.kind == DestinationKind::Stockpile) {
                    let this = Origin::Pile(d.id);
                    if ui.selectable_label(q.from == this, tr!("haul-from-pile", pile = d.name.clone())).clicked() {
                        q.from = this;
                    }
                }
            });
            ui.end_row();
            ui.label(tr!("haul-to"));
            egui::ComboBox::from_id_salt("haul_destination")
                .width(190.0)
                .selected_text(q.destination.map(|id| destination_name(destinations, id)).unwrap_or_default())
                .show_ui(ui, |ui| {
                    for d in destinations {
                        ui.selectable_value(&mut q.destination, Some(d.id), &d.name);
                    }
                });
            ui.end_row();
            ui.label(tr!("haul-truck"));
            egui::ComboBox::from_id_salt("haul_truck")
                .width(190.0)
                .selected_text(q.truck.and_then(|id| project.schedule.trucks().class(id)).map(|c| c.name.clone()).unwrap_or_default())
                .show_ui(ui, |ui| {
                    for c in &project.schedule.trucks().classes {
                        ui.selectable_value(&mut q.truck, Some(c.id), &c.name);
                    }
                });
            ui.end_row();
            ui.label(tr!("haul-loader"));
            egui::ComboBox::from_id_salt("haul_loader")
                .width(190.0)
                .selected_text(q.loader.and_then(|id| project.schedule.agent(id)).map(|a| a.name.clone()).unwrap_or_default())
                .show_ui(ui, |ui| {
                    for a in project.schedule.agents() {
                        ui.selectable_value(&mut q.loader, Some(a.id), &a.name);
                    }
                });
            ui.end_row();
        });
        // Recalculated whenever the question or the project changes, so the
        // answer is never stale and there is no button to remember.
        let key = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            editor.haul_view_revision.hash(&mut hasher);
            project.active_session.hash(&mut hasher);
            match &q.from {
                Origin::Unset => 0u8.hash(&mut hasher),
                Origin::Block(_, p) => p.to_array().map(f64::to_bits).hash(&mut hasher),
                Origin::Pile(id) => id.hash(&mut hasher),
            }
            q.destination.hash(&mut hasher);
            q.truck.hash(&mut hasher);
            q.loader.hash(&mut hasher);
            hasher.finish()
        };
        if previous_key != Some(key) {
            editor.haul_route = check(network, project, &q);
            commands.push(UiCommand::RefreshHaulOverlay);
        }
        let missing = if project.schedule.trucks().classes.is_empty() {
            Some(tr!("haul-need-truck"))
        } else if project.schedule.agents().is_empty() {
            Some(tr!("haul-need-loader"))
        } else if q.from == Origin::Unset {
            Some(tr!("haul-from-hint"))
        } else {
            None
        };
        if let Some(message) = missing {
            ui.label(egui::RichText::new(message).weak().small());
        }
        if let Some(route) = &editor.haul_route {
            result(ui, route, project, &q);
        }
        ui.data_mut(|d| {
            d.insert_temp(id, q);
            d.insert_temp(id.with("key"), key);
        });
    });
}

fn check(network: &HaulNetwork, project: &UiProjectView, q: &Query) -> Option<RouteCheck> {
    let destination = q.destination?;
    let class = project.schedule.trucks().class(q.truck?)?;
    let loader = project.schedule.agent(q.loader?).and_then(|a| project.schedule.class(a.class_id))?;
    let (source, rate, bench) = match &q.from {
        Origin::Unset => return None,
        Origin::Block(_, point) => (Some(*point), loader.default_dig_rate_tph, true),
        Origin::Pile(pile) => (
            network.destination_point(*pile, true, project.haul_points.get(pile).copied()),
            loader.default_reclaim_rate_tph,
            false,
        ),
    };
    let dump = project.schedule.routing().dump_time_s(destination);
    let index = RoadIndex::new(network);
    let routed = source
        .zip(network.destination_point(destination, false, project.haul_points.get(&destination).copied()))
        .and_then(|(source, target)| DestinationSearch::new(network, &index, class, target)?.route(&index, source, bench, rate, loader.spot_time_s, dump));
    Some(routed.unwrap_or_else(|| RouteCheck {
        cycle: crate::model::schedule::trucking::CycleBreakdown::fixed(
            class,
            project.schedule.routing().distance_km(destination),
            rate,
            loader.spot_time_s,
            dump,
            network.settings.acceleration_kph_s,
        ),
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

fn result(ui: &mut egui::Ui, route: &RouteCheck, project: &UiProjectView, q: &Query) {
    let cycle = route.cycle;
    let minutes = |h: f64| format!("{:.1} min", h * 60.0);
    ui.add_space(6.0);
    ui.label(crate::ui::fonts::bold(&tr!("haul-cycle-minutes", minutes = format!("{:.1}", cycle.total_h() * 60.0))));
    if let Some(class) = q.truck.and_then(|id| project.schedule.trucks().class(id))
        && cycle.total_h() > 0.0
    {
        let per_truck = class.payload_t / cycle.total_h();
        let loader = q.loader.and_then(|id| project.schedule.agent(id)).and_then(|a| project.schedule.class(a.class_id));
        let rate = loader.map(|l| {
            if matches!(q.from, Origin::Pile(_)) {
                l.default_reclaim_rate_tph
            } else {
                l.default_dig_rate_tph
            }
        });
        let mut line = tr!("haul-per-truck", rate = format!("{per_truck:.0}"));
        if let Some(rate) = rate {
            line = format!("{line} · {}", tr!("haul-match", trucks = format!("{:.1}", rate / per_truck)));
        }
        ui.label(egui::RichText::new(line).weak()).on_hover_text(tr!("haul-match-help"));
    }
    if !route.uses_roads {
        ui.colored_label(ui.visuals().warn_fg_color, tr!("haul-no-route"));
    } else if !route.connected {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            tr!("haul-unconnected", length = format!("{:.0}", route.access_m), rise = format!("{:+.0}", route.access_rise_m)),
        );
    } else if route.grade_lengthened {
        ui.label(
            egui::RichText::new(tr!(
                "haul-lengthened",
                length = format!("{:.0}", route.access_m),
                rise = format!("{:+.0}", route.access_rise_m)
            ))
            .weak()
            .small(),
        );
    }
    egui::Grid::new("haul_cycle").num_columns(3).show(ui, |ui| {
        for (label, hours, detail) in [
            (tr!("haul-spot"), cycle.spot_h, String::new()),
            (tr!("haul-load"), cycle.load_h, String::new()),
            (tr!("haul-loaded"), cycle.loaded_h, format!("{:.2} km · ↑{:.0} m", cycle.loaded_km, cycle.rise_m)),
            (tr!("haul-dump"), cycle.dump_h, String::new()),
            (tr!("haul-return"), cycle.empty_h, format!("{:.2} km", cycle.empty_km)),
        ] {
            ui.label(label);
            ui.label(minutes(hours));
            ui.label(egui::RichText::new(detail).weak().small());
            ui.end_row();
        }
    });
    profile(ui, &route.profile);
}

/// Elevation (line) and speed (shaded steps) along the loaded haul, with the
/// figures under the pointer on hover.
fn profile(ui: &mut egui::Ui, points: &[[f64; 3]]) {
    if points.len() < 2 {
        return;
    }
    ui.add_space(4.0);
    ui.label(egui::RichText::new(tr!("haul-profile")).weak().small());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 90.0), egui::Sense::hover());
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
