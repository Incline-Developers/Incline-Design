//! Haulage layout: network tools, issues, destination points and route checks.
use glam::DVec3;

use crate::{
    i18n::tr,
    model::{
        Document, SceneEntityId,
        haulage::{
            HaulNetwork, NodeId, NodeRole,
            network::{IssueKind, RoadIndex},
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

pub(crate) fn draw_panel(ui: &mut egui::Ui, editor: &mut EditorState, _document: &Document, project: &UiProjectView, commands: &mut Vec<UiCommand>) -> IslandResponse<()> {
    Island::new("haulage_layout", Side::Right)
        .fill(crate::ui::widgets::tree_row_colors(ui).0)
        .default_width(310.0)
        .min_width(240.0)
        .max_width(500.0)
        .show(ui, |ui, _| {
            let network = &project.haulage;
            let session = project.active_session;
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(crate::ui::fonts::bold(&tr!("planning-page-haulage")));
                if ui
                    .button(if editor.haul_draw { tr!("haul-finish") } else { tr!("haul-draw") })
                    .on_hover_text(tr!("haul-draw-help"))
                    .clicked()
                {
                    commands.push(if editor.haul_draw { UiCommand::FinishHaulRoad } else { UiCommand::StartHaulRoad });
                }
                if ui
                    .add_enabled(
                        editor.selected_handles.iter().any(|h| matches!(h, SceneEntityId::Object(_))),
                        egui::Button::new(tr!("haul-convert")),
                    )
                    .clicked()
                {
                    commands.push(UiCommand::ConvertHaulSelection);
                }
                ui.horizontal(|ui| {
                    if ui.button(tr!("haul-import")).clicked() {
                        editor.show_import = true;
                        editor.data_menu = crate::ui::state::DataMenu::Dxf;
                        editor.import_as_haul_roads = true;
                    }
                    if ui.add_enabled(!network.roads.is_empty(), egui::Button::new(tr!("haul-export"))).clicked() {
                        commands.push(UiCommand::ExportHaulRoads);
                    }
                });
                ui.separator();
                let destinations = project.haul_destinations.clone();
                let ids: Vec<_> = destinations.iter().map(|d| d.id).collect();
                let max_grade = project.schedule.trucks().classes.iter().map(|c| c.maximum_grade).reduce(f64::min).unwrap_or(0.1);
                // Issues are a network edit product, never a per-frame graph scan.
                let cache_id = ui.id().with("haul_issue_cache");
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                use std::hash::{Hash, Hasher};
                network.hash_content(&mut hasher);
                ids.hash(&mut hasher);
                max_grade.to_bits().hash(&mut hasher);
                let key = hasher.finish();
                let index_id = ui.id().with("haul_index_cache");
                let mut index_cache = ui.data(|d| d.get_temp::<(u64, std::sync::Arc<RoadIndex>)>(index_id));
                if index_cache.as_ref().is_none_or(|c| c.0 != key) {
                    index_cache = Some((key, std::sync::Arc::new(RoadIndex::new(network))));
                }
                let index_cache = index_cache.expect("road index cache");
                ui.data_mut(|d| d.insert_temp(index_id, index_cache.clone()));
                let index = &index_cache.1;
                let mut cache = ui.data(|d| d.get_temp::<(u64, Vec<crate::model::haulage::network::NetworkIssue>)>(cache_id));
                if cache.as_ref().is_none_or(|c| c.0 != key) {
                    cache = Some((key, network.issues(&ids, max_grade)));
                }
                let cache = cache.expect("issues cache");
                ui.label(tr!("haul-summary", roads = network.roads.len().to_string(), issues = cache.1.len().to_string()));
                if !editor.haul_blocks.is_empty() {
                    ui.label(tr!(
                        "haul-connected",
                        connected = editor.haul_blocks.iter().filter(|(_, c)| *c).count().to_string(),
                        total = editor.haul_blocks.len().to_string()
                    ));
                }
                ui.collapsing(tr!("haul-issues"), |ui| {
                    for issue in &cache.1 {
                        let label = match issue.kind {
                            IssueKind::DeadEnd => tr!("haul-dead-end"),
                            IssueKind::NearMiss => tr!("haul-near-miss"),
                            IssueKind::SeparatePiece => tr!("haul-separate-piece"),
                            IssueKind::SteepRoad => tr!("haul-steep"),
                            IssueKind::MissingDestination => tr!("haul-missing-destination"),
                        };
                        let label = format!(
                            "{} · {}",
                            label,
                            issue
                                .road
                                .map(|id| id.0 & u64::from(u32::MAX))
                                .or(issue.node.map(|id| id.0 & u64::from(u32::MAX)))
                                .unwrap_or(0)
                        );
                        if ui.small_button(label).clicked() {
                            commands.push(UiCommand::FrameHaulPoint(issue.pos));
                        }
                    }
                });
                ui.data_mut(|d| d.insert_temp(cache_id, cache));
                ui.collapsing(tr!("haul-roads"), |ui| {
                    for road in &network.roads {
                        let entity = SceneEntityId::HaulRoad(road.id);
                        let points = network.points(road);
                        let length: f64 = points.windows(2).map(|p| p[0].distance(p[1])).sum();
                        let grade = points.windows(2).map(|p| crate::model::haulage::network::grade(p[0], p[1]).abs()).fold(0.0, f64::max) * 100.0;
                        let response = ui.selectable_label(editor.selected_handles.contains(&entity), &road.name).on_hover_text(format!(
                            "{}: {:.0} m\n{}: {:.1}%\n{}: {}",
                            tr!("haul-length"),
                            length,
                            tr!("haul-grade"),
                            grade,
                            tr!("haul-speed-limit"),
                            road.speed_limit_kph.map(|v| format!("{v:.0}")).unwrap_or_else(|| tr!("haul-no-limit"))
                        ));
                        if response.clicked() {
                            editor.selected_handles.clear();
                            editor.selected_handles.insert(entity);
                            commands.push(UiCommand::FrameHaulPoint(points[0]));
                        }
                        context_menu_popup(&response, &road.name, |ui| road_menu(ui, editor, road.id, session, commands));
                    }
                });
                ui.collapsing(tr!("haul-nodes"), |ui| {
                    for node in &network.nodes {
                        let entity = SceneEntityId::HaulNode(node.id);
                        let label = node
                            .role
                            .map(|r| {
                                let (id, reclaim) = match r {
                                    NodeRole::Dump(id) => (id, false),
                                    NodeRole::Reclaim(id) => (id, true),
                                };
                                format!(
                                    "{} · {}",
                                    destinations
                                        .iter()
                                        .find(|d| d.id == id)
                                        .map(|d| d.name.clone())
                                        .unwrap_or_else(|| tr!("haul-missing-destination")),
                                    if reclaim { tr!("haul-reclaim") } else { tr!("haul-dump") }
                                )
                            })
                            .unwrap_or_else(|| format!("{} {}", tr!("haul-node"), node.id.0 & u64::from(u32::MAX)));
                        let response = ui
                            .selectable_label(editor.selected_handles.contains(&entity), &label)
                            .on_hover_text(format!("{:.1}, {:.1}, {:.1}", node.pos.x, node.pos.y, node.pos.z));
                        if response.clicked() {
                            if !ui.input(|i| i.modifiers.shift) {
                                editor.selected_handles.clear();
                            }
                            editor.selected_handles.insert(entity);
                            commands.push(UiCommand::FrameHaulPoint(node.pos));
                        }
                        context_menu_popup(&response, &label, |ui| node_menu(ui, editor, network, node.id, &destinations, session, commands));
                    }
                });
                selection_properties(ui, editor, network, session, commands);
                ui.collapsing(tr!("haul-destinations"), |ui| {
                    for destination in &destinations {
                        let nearest = project.haul_points.get(&destination.id).and_then(|p| index.candidates(*p, 0.0).first().map(|c| c.2));
                        let point = if network.fixed_destinations.contains(&destination.id) {
                            None
                        } else {
                            network.role_point(destination.id, false).or(nearest)
                        };
                        let method = if point.is_some() && network.role_point(destination.id, false).is_some() {
                            tr!("haul-method-roads")
                        } else if point.is_some() {
                            tr!("haul-method-nearest")
                        } else {
                            tr!("haul-method-fixed", distance = format!("{:.1}", destination.distance_km))
                        };
                        let response = ui.selectable_label(false, format!("{} · {method}", destination.name));
                        if response.clicked()
                            && let Some(point) = point
                        {
                            commands.push(UiCommand::FrameHaulPoint(point));
                        }
                        context_menu_popup(&response, &destination.name, |ui| {
                            if let Some(node) = editor
                                .selected_handles
                                .iter()
                                .find_map(|h| if let SceneEntityId::HaulNode(id) = h { Some(*id) } else { None })
                                && ContextMenuAction::new(tr!("haul-pick-node")).show(ui).clicked()
                            {
                                command(commands, session, HaulEdit::Role(node, Some(NodeRole::Dump(destination.id))));
                                command(commands, session, HaulEdit::Fixed(destination.id, false));
                                ui.close();
                            }
                            if let Some(nearest) = nearest
                                && ContextMenuAction::new(tr!("haul-pin-nearest")).show(ui).clicked()
                            {
                                command(commands, session, HaulEdit::Pin(destination.id, nearest));
                                ui.close();
                            }
                            if ContextMenuAction::new(tr!("haul-use-fixed")).show(ui).clicked() {
                                command(commands, session, HaulEdit::Fixed(destination.id, true));
                                for node in network
                                    .nodes
                                    .iter()
                                    .filter(|n| n.role == Some(NodeRole::Dump(destination.id)) || n.role == Some(NodeRole::Reclaim(destination.id)))
                                {
                                    command(commands, session, HaulEdit::Role(node.id, None));
                                }
                                ui.close();
                            }
                        });
                    }
                });
                route_check(ui, editor, network, project, &destinations, commands);
                ui.collapsing(tr!("haul-settings"), |ui| {
                    let mut settings = network.settings.clone();
                    let mut commit = false;
                    for (label, value) in [
                        (tr!("haul-join"), &mut settings.join_tolerance_m),
                        (tr!("haul-auto-join"), &mut settings.auto_join_m),
                        (tr!("haul-bench-speed"), &mut settings.bench_speed_kph),
                        (tr!("haul-acceleration"), &mut settings.acceleration_kph_s),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(label);
                            let response = ui.add(egui::DragValue::new(value).range(0.01..=100_000.0));
                            commit |= response.changed();
                        });
                    }
                    if commit {
                        command(commands, session, HaulEdit::Settings(settings));
                    }
                });
            });
        })
}

fn road_menu(ui: &mut egui::Ui, editor: &EditorState, id: crate::model::haulage::RoadId, session: u32, commands: &mut Vec<UiCommand>) {
    for label in [tr!("haul-rename"), tr!("haul-speed-limit")] {
        if ContextMenuAction::new(label).show(ui).clicked() {
            commands.push(UiCommand::EditHaulProperties);
            ui.close();
        }
    }
    if let Some(pos) = editor.cursor_world
        && ContextMenuAction::new(tr!("haul-split")).show(ui).clicked()
    {
        command(commands, session, HaulEdit::Split(id, pos));
        ui.close();
    }
    if ContextMenuAction::new(tr!("haul-delete")).show(ui).clicked() {
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
    if ContextMenuAction::new(tr!("haul-move-node")).show(ui).clicked() {
        editor.canvas_context_menu_open = false;
        editor.haul_move_node = Some(id);
        ui.close();
    }
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
    if ContextMenuAction::new(tr!("haul-clear-role")).show(ui).clicked() {
        command(commands, session, HaulEdit::Role(id, None));
        ui.close();
    }
    let selected: Vec<_> = editor
        .selected_handles
        .iter()
        .filter_map(|h| if let SceneEntityId::HaulNode(id) = h { Some(*id) } else { None })
        .collect();
    if selected.len() == 2 && ContextMenuAction::new(tr!("haul-join-nodes")).show(ui).clicked() {
        command(commands, session, HaulEdit::Join(selected[0], selected[1]));
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

pub(crate) fn canvas_menu(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, document: &Document, commands: &mut Vec<UiCommand>) {
    if ContextMenuAction::new(tr!("haul-draw")).show(ui).clicked() {
        commands.push(UiCommand::StartHaulRoad);
        ui.close();
    }
    if ContextMenuAction::new(tr!("haul-convert"))
        .enabled(editor.selected_handles.iter().any(|h| matches!(h, SceneEntityId::Object(_))))
        .show(ui)
        .clicked()
    {
        commands.push(UiCommand::ConvertHaulSelection);
        ui.close();
    }
    let selected: Vec<_> = editor.selected_handles.iter().copied().collect();
    let destinations = destinations::available(document.solids(), project.schedule.routing());
    for h in selected {
        match h {
            SceneEntityId::HaulRoad(id) => road_menu(ui, editor, id, project.active_session, commands),
            SceneEntityId::HaulNode(id) => node_menu(ui, editor, &project.haulage, id, &destinations, project.active_session, commands),
            _ => {}
        }
    }
}

fn selection_properties(ui: &mut egui::Ui, editor: &mut EditorState, network: &HaulNetwork, session: u32, commands: &mut Vec<UiCommand>) {
    if let Some(id) = editor.haul_delete_node {
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
    }
    let selected: Vec<_> = editor.selected_handles.iter().copied().collect();
    for handle in &selected {
        match handle {
            SceneEntityId::HaulNode(id) if network.node(*id).is_some() => {
                let node = network.node(*id).expect("selected node");
                egui::CollapsingHeader::new(format!("{} {}", tr!("haul-node"), id.0 & u64::from(u32::MAX)))
                    .default_open(true)
                    .show(ui, |ui| {
                        let mut pos = node.pos;
                        if coordinates(ui, &mut pos) {
                            command(commands, session, HaulEdit::MoveNode(*id, pos));
                        }
                        if ui.button(tr!("haul-pick-position")).clicked() {
                            editor.haul_move_node = Some(*id);
                        }
                    });
            }
            SceneEntityId::HaulRoad(id) if network.road(*id).is_some() => {
                let road = network.road(*id).expect("selected road");
                egui::CollapsingHeader::new(&road.name).default_open(true).show(ui, |ui| {
                    let draft_id = ui.id().with(("road_name", id));
                    let mut name = ui.data(|d| d.get_temp::<String>(draft_id)).unwrap_or_else(|| road.name.clone());
                    if ui.text_edit_singleline(&mut name).lost_focus() && name != road.name {
                        command(commands, session, HaulEdit::RoadProperties(vec![*id], Some(name.clone()), road.speed_limit_kph));
                    }
                    ui.data_mut(|d| d.insert_temp(draft_id, name));
                    let mut limited = road.speed_limit_kph.is_some();
                    let mut speed = road.speed_limit_kph.unwrap_or(30.0);
                    let mut changed = ui.checkbox(&mut limited, tr!("haul-speed-limit")).changed();
                    if limited {
                        changed |= ui.add(egui::DragValue::new(&mut speed).range(0.1..=200.0)).changed();
                    }
                    if changed {
                        let roads = selected.iter().filter_map(|h| if let SceneEntityId::HaulRoad(id) = h { Some(*id) } else { None }).collect();
                        command(commands, session, HaulEdit::RoadProperties(roads, None, limited.then_some(speed)));
                    }
                    for (index, &point) in road.verts.iter().enumerate() {
                        ui.push_id(index, |ui| {
                            let mut pos = point;
                            if coordinates(ui, &mut pos) {
                                command(commands, session, HaulEdit::MoveShape(*id, index, pos));
                            }
                            if ui.small_button(tr!("haul-pick-position")).clicked() {
                                editor.haul_move_shape = Some((*id, index));
                            }
                        });
                    }
                });
            }
            _ => {}
        }
    }
}
fn coordinates(ui: &mut egui::Ui, pos: &mut DVec3) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (label, value) in [("X", &mut pos.x), ("Y", &mut pos.y), ("Z", &mut pos.z)] {
            ui.label(label);
            changed |= ui.add(egui::DragValue::new(value).speed(0.1)).changed();
        }
    });
    changed
}

#[derive(Clone, Default, PartialEq)]
struct Query {
    source: DVec3,
    destination: Option<DestinationId>,
    truck: Option<TruckClassId>,
    loader: Option<LoaderAgentId>,
    pile: Option<DestinationId>,
}
fn route_check(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    network: &HaulNetwork,
    project: &UiProjectView,
    destinations: &[destinations::DestinationView],
    commands: &mut Vec<UiCommand>,
) {
    ui.collapsing(tr!("haul-route-check"), |ui| {
        let id = ui.id().with(("haul_query", project.active_session));
        let mut q = ui.data(|d| d.get_temp::<Query>(id)).unwrap_or_default();
        let original = q.clone();
        if let Some(point) = editor.haul_source_point.take() {
            q.source = point;
            q.pile = None;
        }
        egui::ComboBox::from_id_salt("haul_block_source").selected_text(tr!("haul-ground")).show_ui(ui, |ui| {
            for (block, _) in &editor.haul_blocks {
                if ui.selectable_label(false, &block.name).clicked() {
                    q.source = DVec3::new(block.anchor[0], block.anchor[1], block.plane);
                    q.pile = None;
                }
            }
        });
        if ui.button(tr!("haul-cursor")).clicked()
            && let Some(pos) = editor.cursor_world
        {
            q.source = pos;
            q.pile = None;
        }
        ui.label(tr!("haul-source"));
        coordinates(ui, &mut q.source);
        egui::ComboBox::from_id_salt("haul_pile")
            .selected_text(
                q.pile
                    .and_then(|id| destinations.iter().find(|d| d.id == id))
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| tr!("haul-ground")),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut q.pile, None, tr!("haul-ground"));
                for d in destinations.iter().filter(|d| d.kind == DestinationKind::Stockpile) {
                    ui.selectable_value(&mut q.pile, Some(d.id), &d.name);
                }
            });
        egui::ComboBox::from_id_salt("haul_destination")
            .selected_text(
                q.destination
                    .and_then(|id| destinations.iter().find(|d| d.id == id))
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| tr!("haul-destination")),
            )
            .show_ui(ui, |ui| {
                for d in destinations {
                    ui.selectable_value(&mut q.destination, Some(d.id), &d.name);
                }
            });
        egui::ComboBox::from_id_salt("haul_truck")
            .selected_text(
                q.truck
                    .and_then(|id| project.schedule.trucks().class(id))
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| tr!("haul-truck")),
            )
            .show_ui(ui, |ui| {
                for c in &project.schedule.trucks().classes {
                    ui.selectable_value(&mut q.truck, Some(c.id), &c.name);
                }
            });
        egui::ComboBox::from_id_salt("haul_loader")
            .selected_text(
                q.loader
                    .and_then(|id| project.schedule.agent(id))
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| tr!("haul-loader")),
            )
            .show_ui(ui, |ui| {
                for a in project.schedule.agents() {
                    ui.selectable_value(&mut q.loader, Some(a.id), &a.name);
                }
            });
        let mut failed = ui.data(|d| d.get_temp::<bool>(id.with("failed"))).unwrap_or(false);
        let check_requested = ui.button(tr!("haul-check")).clicked();
        if !check_requested && original != q {
            editor.haul_route = None;
            failed = false;
        }
        if check_requested {
            editor.haul_route = None;
            let loader = q.loader.and_then(|id| project.schedule.agent(id)).and_then(|a| project.schedule.class(a.class_id));
            if let (Some(destination), Some(class), Some(loader)) = (q.destination, q.truck.and_then(|id| project.schedule.trucks().class(id)), loader) {
                let index = RoadIndex::new(network);
                let point = |id, reclaim| {
                    if network.fixed_destinations.contains(&id) {
                        return None;
                    }
                    network
                        .role_point(id, reclaim)
                        .or_else(|| project.haul_points.get(&id).and_then(|p| index.candidates(*p, 0.0).first().map(|c| c.2)))
                };
                let source = q.pile.map_or(Some(q.source), |id| point(id, true));
                let rate = if q.pile.is_some() {
                    loader.default_reclaim_rate_tph
                } else {
                    loader.default_dig_rate_tph
                };
                let dump = project.schedule.routing().dump_time_s(destination);
                if let (Some(source), Some(target)) = (source, point(destination, false))
                    && let Some(search) = crate::model::haulage::routing::DestinationSearch::new(network, &index, class, target)
                {
                    editor.haul_route = search.route(&index, source, q.pile.is_none(), rate, loader.spot_time_s, dump);
                }
                if editor.haul_route.is_none() {
                    let cycle = crate::model::schedule::trucking::CycleBreakdown::fixed(
                        class,
                        project.schedule.routing().distance_km(destination),
                        rate,
                        loader.spot_time_s,
                        dump,
                        network.settings.acceleration_kph_s,
                    );
                    editor.haul_route = Some(crate::model::haulage::routing::RouteCheck {
                        cycle,
                        loaded_path: Vec::new(),
                        empty_path: Vec::new(),
                        connected: false,
                        access_m: 0.0,
                        access_rise_m: 0.0,
                        grade_lengthened: false,
                        profile: Vec::new(),
                        uses_roads: false,
                    });
                }
            }
            failed = editor.haul_route.is_none();
        }
        if check_requested || original != q {
            commands.push(UiCommand::RefreshHaulOverlay);
        }
        if failed {
            ui.label(tr!("haul-no-loader"));
        }
        if let Some(route) = &editor.haul_route {
            let cycle = route.cycle;
            if !route.uses_roads {
                ui.label(tr!("haul-no-route"));
            }
            ui.label(crate::ui::fonts::bold(&tr!("haul-cycle-minutes", minutes = format!("{:.1}", cycle.total_h() * 60.0))));
            egui::Grid::new("haul_cycle").num_columns(2).show(ui, |ui| {
                for (label, value) in [
                    (tr!("haul-spot"), cycle.spot_h * 60.0),
                    (tr!("haul-load"), cycle.load_h * 60.0),
                    (tr!("haul-loaded"), cycle.loaded_h * 60.0),
                    (tr!("haul-dump"), cycle.dump_h * 60.0),
                    (tr!("haul-return"), cycle.empty_h * 60.0),
                    (tr!("haul-distance"), cycle.loaded_km),
                    (tr!("haul-rise"), cycle.rise_m),
                ] {
                    ui.label(label);
                    ui.label(format!("{value:.2}"));
                    ui.end_row();
                }
                if let Some(class) = q.truck.and_then(|id| project.schedule.trucks().class(id)) {
                    let rate = class.payload_t / cycle.total_h();
                    ui.label(tr!("haul-rate"));
                    ui.label(format!("{rate:.0}"));
                    ui.end_row();
                    if let Some(loader) = q.loader.and_then(|id| project.schedule.agent(id)).and_then(|a| project.schedule.class(a.class_id)) {
                        ui.label(tr!("haul-match"));
                        ui.label(format!(
                            "{:.1}",
                            if q.pile.is_some() {
                                loader.default_reclaim_rate_tph / rate
                            } else {
                                loader.default_dig_rate_tph / rate
                            }
                        ));
                        ui.end_row();
                    }
                }
            });
            if route.uses_roads && !route.connected {
                ui.label(tr!(
                    "haul-unconnected",
                    length = format!("{:.0}", route.access_m),
                    rise = format!("{:.0}", route.access_rise_m)
                ));
            }
            profile(ui, &route.profile, 1, &tr!("haul-profile"));
            profile(ui, &route.profile, 2, &tr!("haul-profile-speed"));
        }
        ui.data_mut(|d| {
            d.insert_temp(id, q);
            d.insert_temp(id.with("failed"), failed);
        });
    });
}
fn profile(ui: &mut egui::Ui, points: &[[f64; 3]], field: usize, label: &str) {
    if points.len() < 2 {
        return;
    }
    ui.label(label);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 80.0), egui::Sense::hover());
    let min = points.iter().map(|p| p[field]).fold(f64::INFINITY, f64::min);
    let max = points.iter().map(|p| p[field]).fold(f64::NEG_INFINITY, f64::max);
    let length = points.last().map_or(0.0, |p| p[0]);
    let mut prev = None;
    for p in points {
        let screen = egui::pos2(
            rect.left() + (p[0] / length.max(1e-6)) as f32 * rect.width(),
            rect.bottom() - ((p[field] - min) / (max - min).max(1.0)) as f32 * rect.height(),
        );
        if let Some(prev) = prev {
            ui.painter().line_segment([prev, screen], egui::Stroke::new(2.0, egui::Color32::from_rgb(230, 100, 70)));
        }
        prev = Some(screen);
    }
}
