//! Top application menu bar: the mark, the application-wide menus (File,
//! Project) and the workspace tabs.
//!
//! Everything that acts on the scene rather than on the application has moved
//! down to the viewport bar - see [`crate::ui::elements::viewport_bar`] - so
//! this bar carries only what is true whichever workspace is open, the way
//! Blender's topbar does.

// The dropdowns below are all in the system menu bar on macOS (`mac.rs`), and
// nothing this module still draws there needs any of this.
#[cfg(not(target_os = "macos"))]
use crate::ui::{
    state::{UiProjectEntry, ViewToggle},
    widgets::context_menu::{ContextMenuAction, MenuBarMenu, context_menu_heading, context_menu_separator, context_submenu},
};
use crate::{
    i18n::tr,
    model::{Axis, SceneEntityId},
    ui::{
        EditorState, UiCommand, UiProjectView,
        state::{ActiveTool, Workspace, string_rework_live},
        themed_icon,
        widgets::toolbar::GROUP_CORNER_RADIUS,
    },
};

/// Extra space above the bar's contents, on top of the panel frame's own
/// margin, so the menus and tabs are not pinned to the window's top edge.
const TOP_PADDING: i8 = 3;
/// Side of the application mark at the left of the bar.
const LOGO_SIZE: f32 = 18.0;
/// Space either side of the mark, so it reads as a mark rather than as the
/// first menu.
const LOGO_MARGIN: f32 = 6.0;
/// Height of the rule parting the menus from the workspace tabs.
#[cfg(not(target_os = "macos"))]
const SEPARATOR_HEIGHT: f32 = 16.0;
/// Space either side of that rule.
#[cfg(not(target_os = "macos"))]
const SEPARATOR_MARGIN: f32 = 8.0;
/// Space either side of a workspace tab's label.
const TAB_PADDING: f32 = 10.0;
/// Vertical space a workspace tab claims in the bar.
const TAB_HEIGHT: f32 = 20.0;
/// Height of a workspace tab's fill, sat against the top of that space. Shorter
/// than [`TAB_HEIGHT`] so the group box behind Planning keeps air below it
/// instead of bleeding into the region under the bar.
const TAB_FILL_HEIGHT: f32 = 17.0;
/// Space either side of a dropdown's label in the viewport bar.
///
/// `egui`'s `menu_style` packs bar labels to 2 points, which is right for a
/// menu bar where the labels are the whole row. In the viewport bar they share
/// the row with three clusters of icons, and at that padding the six of them
/// ran together into one band of text.
///
/// Unused on macOS, where the production menus live in the system menu bar.
#[cfg_attr(target_os = "macos", allow(dead_code))]
const MENU_LABEL_PADDING: f32 = 7.0;
/// Gap between two dropdowns in that run, on top of their padding.
#[cfg_attr(target_os = "macos", allow(dead_code))]
const MENU_LABEL_GAP: f32 = 2.0;
/// What the platform calls showing a file in its file manager. On macOS the
/// File menu's row lives in the system menu instead - see `mac.rs` - but the
/// splash's Recent list still asks for the name here.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn show_in_file_manager_label() -> String {
    #[cfg(target_os = "windows")]
    return tr!("menu-file-show-in-explorer");
    #[cfg(target_os = "macos")]
    return crate::i18n::tr!("common-reveal-finder");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return tr!("menu-file-show-in-folder");
}

/// Draw the top menu bar panel.
///
/// Dropdowns are the same widget as the right-click menus - see
/// [`crate::ui::widgets::context_menu`] - so rows, submenus and separators all
/// come from there rather than from egui's default menu style.
///
/// On macOS the File and Project menus live in the system menu bar (`mac.rs`)
/// instead, but the bar itself stays: the mark and the workspace tabs are not
/// things an `NSMenu` can carry.
///
/// Returns the panel's bounding rect.
pub(crate) fn draw_main_menu(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) -> egui::Rect {
    // Not a region: the bar spans the window and carries the gap's own colour,
    // so it reads as the backdrop the workspace is laid on rather than as a
    // panel of its own. See `chrome::window_bar_frame`.
    let bar_fill = crate::ui::chrome::window_bar_fill(ui);
    let frame = crate::ui::chrome::window_bar_frame(ui);
    let frame = frame.inner_margin(egui::Margin {
        top: frame.inner_margin.top + TOP_PADDING,
        ..frame.inner_margin
    });
    egui::Panel::top("main_menu")
        .show_separator_line(crate::ui::chrome::show_separator_line(ui))
        .frame(frame)
        .show(ui, |ui| {
            // Too narrow a window scrolls the row rather than clipping the
            // workspace tabs off the end of it. See `elements::bar_strip`.
            // The row is as tall as `MenuBar` makes it, which is this unless
            // something on it is taller; the strip grows with the contents.
            crate::ui::elements::bar_strip(ui, "main_menu_strip", ui.spacing().interact_size.y, |ui, strip| {
                let mut used = 0.0;
                ui.scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
                    egui::MenuBar::new().ui(ui, |ui| {
                        draw_logo(ui);

                        #[cfg(not(target_os = "macos"))]
                        {
                            draw_file_menu(ui, editor, project, commands);
                            draw_view_menu(ui, editor, commands);
                        }
                        // The parameter is only read by the menus above.
                        #[cfg(target_os = "macos")]
                        let _ = project;

                        #[cfg(not(target_os = "macos"))]
                        draw_separator(ui);

                        draw_workspace_tabs(ui, editor, commands, bar_fill);

                        // `MenuBar` sizes itself to the whole width it is given,
                        // so its rect says nothing about how much of the row was
                        // used; where the next item would go does.
                        used = ui.cursor().left() - strip.left();
                    });
                });
                used
            });
        })
        .response
        .rect
}

/// Paint the application mark at the head of the bar.
///
/// Flat and one-coloured: beside the menu labels it is a glyph, not artwork.
fn draw_logo(ui: &mut egui::Ui) {
    ui.add_space(LOGO_MARGIN);
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(LOGO_SIZE), egui::Sense::hover());
    egui::Image::new(themed_icon!(ui, "logo.svg")).fit_to_exact_size(rect.size()).paint_at(ui, rect);
    ui.add_space(LOGO_MARGIN);
}

/// Part the menus from the workspace tabs with a short vertical rule.
#[cfg(not(target_os = "macos"))]
fn draw_separator(ui: &mut egui::Ui) {
    ui.add_space(SEPARATOR_MARGIN);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, SEPARATOR_HEIGHT), egui::Sense::hover());
    let color = ui.visuals().widgets.noninteractive.bg_stroke.color;
    ui.painter().line_segment([rect.center_top(), rect.center_bottom()], egui::Stroke::new(1.0, color));
    ui.add_space(SEPARATOR_MARGIN);
}

/// A drag previews the order locally; only releasing commits it to settings.
#[derive(Clone)]
struct WorkspaceTabDrag {
    workspace: Workspace,
    order: [Workspace; 5],
    grab_offset: f32,
}

/// Draw the run of workspace tabs, sliding neighbours out of the dragged tab's way.
fn draw_workspace_tabs(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>, bar_fill: egui::Color32) {
    let drag_id = ui.scope_id().with("workspace-tab-drag");
    let mut drag = ui.ctx().data_mut(|data| data.get_temp::<WorkspaceTabDrag>(drag_id));
    let font = egui::TextStyle::Button.resolve(ui.style());
    let widths = Workspace::ALL.map(|workspace| ui.painter().layout_no_wrap(workspace.label(), font.clone(), egui::Color32::PLACEHOLDER).size().x + TAB_PADDING * 2.0);
    let tab_width = |workspace| widths[Workspace::ALL.iter().position(|item| *item == workspace).unwrap()];
    let spacing = ui.spacing().item_spacing.x;
    let total_width = widths.iter().sum::<f32>() + spacing * (Workspace::ALL.len() - 1) as f32;
    let (strip, _) = ui.allocate_exact_size(egui::vec2(total_width, TAB_HEIGHT), egui::Sense::hover());
    let pointer = ui.input(|input| input.pointer.interact_pos());
    let cancelled = ui.input(|input| input.key_pressed(egui::Key::Escape));
    if cancelled {
        drag = None;
    }
    if let Some(drag) = &mut drag
        && let Some(pointer) = pointer
    {
        let center = pointer.x - drag.grab_offset + tab_width(drag.workspace) / 2.0;
        let mut index = drag.order.iter().position(|item| *item == drag.workspace).unwrap();
        let mut x = strip.left();
        let centers = drag.order.map(|workspace| {
            let center = x + tab_width(workspace) / 2.0;
            x += tab_width(workspace) + spacing;
            center
        });
        while index > 0 && center < centers[index - 1] {
            drag.order.swap(index, index - 1);
            index -= 1;
        }
        while index + 1 < drag.order.len() && center > centers[index + 1] {
            drag.order.swap(index, index + 1);
            index += 1;
        }
    }
    let order = drag.as_ref().map_or(editor.workspace_order, |drag| drag.order);
    let mut x = strip.left();
    let mut tabs = Vec::new();
    for workspace in order {
        let id = ui.scope_id().with(("workspace-tab", workspace));
        let animated_x = ui.ctx().animate_value_with_time(id.with("position"), x, 0.12);
        let left = if let Some(drag) = &drag
            && drag.workspace == workspace
            && let Some(pointer) = pointer
        {
            (pointer.x - drag.grab_offset).clamp(strip.left(), strip.right() - tab_width(workspace))
        } else {
            animated_x
        };
        let rect = egui::Rect::from_min_size(egui::pos2(left, strip.top()), egui::vec2(tab_width(workspace), TAB_FILL_HEIGHT));
        tabs.push((workspace, id, rect));
        x += tab_width(workspace) + spacing;
    }
    // Paint the held tab last so it travels above its sliding neighbours.
    tabs.sort_by_key(|(workspace, _, _)| drag.as_ref().is_some_and(|drag| drag.workspace == *workspace));
    for (workspace, id, rect) in tabs {
        let response = draw_workspace_tab(ui, editor, commands, workspace, bar_fill, id, rect);
        if response.drag_started() && !cancelled {
            let press = ui.input(|input| input.pointer.press_origin()).unwrap_or(rect.center());
            drag = Some(WorkspaceTabDrag {
                workspace,
                order,
                grab_offset: press.x - rect.left(),
            });
        }
    }
    if ui.input(|input| input.pointer.any_released()) {
        if let Some(drag) = drag.take()
            && drag.order != editor.workspace_order
        {
            let index = drag.order.iter().position(|item| *item == drag.workspace).unwrap();
            commands.push(UiCommand::ReorderWorkspace {
                workspace: drag.workspace,
                before: drag.order.get(index + 1).copied(),
            });
        }
    } else if !ui.input(|input| input.pointer.primary_down()) {
        drag = None;
    }
    ui.ctx().data_mut(|data| {
        if let Some(drag) = drag {
            data.insert_temp(drag_id, drag);
        } else {
            data.remove::<WorkspaceTabDrag>(drag_id);
        }
    });
}

/// Open `workspace`, putting down anything the tools it does not carry had
/// picked up.
///
/// The drawing tools leave the window with the workspaces that draw - see
/// [`Workspace::has_drawing_tools`] - so a tool still armed after the switch
/// would have nothing left to put it down. Entering a workspace without them
/// hands the pointer back to plain picking first. Flying and the slice view are
/// not among them: their buttons sit in the run every workspace carries - see
/// `viewport_bar::draw_scene_modes` - so a mode is still there to be turned off
/// after the switch, and one being run over a blast pattern or a geological
/// model is the point of it being universal. Arming the slice line is the slice
/// view's own first step rather than a drawing tool, so it survives too. The
/// cursor mode is left as the user set it: it does nothing without a drawing
/// tool to use it, and it is theirs again the moment production is back.
fn select_workspace(editor: &mut EditorState, commands: &mut Vec<UiCommand>, workspace: Workspace) {
    if editor.active_workspace == workspace {
        return;
    }
    let previous_workspace = editor.active_workspace;
    editor.active_workspace = workspace;
    editor.vertex_edit_run = None;
    // Only Geology reads a segment as Measure Distance hovers; the renderer
    // sees the switch and takes the highlight down.
    editor.measure_segment_hover = None;
    // The recent functions list is the workspace's own.
    if editor.canvas_context_menu_recent {
        editor.canvas_context_menu_open = false;
    }
    if workspace != Workspace::DrillAndBlast && editor.drill_pattern_open {
        editor.close_drill_pattern();
    }
    // A tie-in is Drill & Blast's own run; it does not wait on the other side
    // of a trip through production.
    editor.end_tie_chain();
    editor.initiation_dialog = None;
    editor.cancel_haul_edit();
    // Survey consumes the same entity selection as the design workspaces.
    // Keep it when entering/leaving Survey so users can select data first.
    // Drill & Blast's individual-hole selection still has different semantics.
    let survey_transition = workspace == Workspace::Survey || previous_workspace == Workspace::Survey;
    if !survey_transition || workspace == Workspace::DrillAndBlast || previous_workspace == Workspace::DrillAndBlast {
        commands.push(UiCommand::ClearSelection);
    }
    // A tool belongs to the discipline whose cell arms it, both ways round:
    // Drill & Blast's Move Collar is put down on the way out just as the
    // drawing tools are on the way in.
    let survives = match editor.active_tool {
        ActiveTool::None | ActiveTool::VerticalSlice | ActiveTool::PickRotationCentre | ActiveTool::MeasureDistance | ActiveTool::MeasureBatterAngle | ActiveTool::Hide => true,
        ActiveTool::MoveCollar | ActiveTool::RotateCollar | ActiveTool::TieHoles | ActiveTool::SetInitiationPoint | ActiveTool::ChargeHoles => {
            workspace == Workspace::DrillAndBlast
        }
        // Planning's Blasting step carries a run of the drawing tools of its
        // own, so one armed there is still armed when the user comes back to
        // it - the same rule, read against the page rather than the tab.
        tool if tool.designs_pit() => workspace.has_production_tools() || editor.is_planning_cut_step(),
        ActiveTool::InsertPoint | ActiveTool::FillSegment | ActiveTool::SplitAtCrossings | ActiveTool::RemoveBetweenCrossings | ActiveTool::ExplodePolylineToPoints => {
            workspace.has_drawing_aids() || editor.is_planning_cut_step()
        }
        _ => workspace.has_drawing_tools() || editor.is_planning_cut_step(),
    };
    // A tool one workspace edits with on a section, another may refuse there.
    let survives = survives && !(editor.slice_mode_enabled && editor.active_tool.section_refuses(workspace));
    if !survives {
        commands.push(UiCommand::SetActiveTool(ActiveTool::None));
    }
}

/// One workspace tab: a text cell filled from [`tab_fill`].
///
/// A tab for a workspace that has nothing behind it yet is drawn as bare text
/// on the bar rather than left off it, so the shape of the application is
/// visible before all of it is built.
fn draw_workspace_tab(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    commands: &mut Vec<UiCommand>,
    workspace: Workspace,
    bar_fill: egui::Color32,
    id: egui::Id,
    rect: egui::Rect,
) -> egui::Response {
    let enabled = workspace.implemented();
    let selected = editor.active_workspace == workspace;
    let state = if !enabled {
        TabState::Disabled
    } else if selected {
        TabState::Active
    } else {
        TabState::Inactive
    };

    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(workspace.label(), font, egui::Color32::PLACEHOLDER);
    // Not `add_enabled_ui`, which fades everything painted inside it toward the
    // background: the weak text colour below is the whole of what a disabled tab
    // says about itself, and a fade would wash it out further. The colours below
    // say what state a tab is in; the sense is what stops it being clicked.
    let sense = if enabled { egui::Sense::click_and_drag() } else { egui::Sense::hover() };
    let response = ui.interact(rect, id, sense);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, selected, workspace.label()));

    let visuals = ui.visuals();
    ui.painter().rect_filled(rect, GROUP_CORNER_RADIUS, tab_fill(visuals, bar_fill, state, response.hovered()));
    let text = match state {
        TabState::Active => visuals.strong_text_color(),
        TabState::Inactive => visuals.text_color(),
        TabState::Disabled => visuals.weak_text_color(),
    };
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, text);

    if response.clicked() {
        select_workspace(editor, commands, workspace);
    }
    response
}

/// What a workspace tab is showing about itself.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TabState {
    Active,
    Inactive,
    Disabled,
}

/// Fill for a workspace tab, stepped off `bar_fill` the way Blender steps its
/// workspace tabs.
///
/// The open workspace's tab takes the panel surface itself - it reads as the
/// workspace below coming up through the bar, which is exactly what Blender's
/// active tab does. Every other tab takes the bar's own colour, so no cell
/// outline shows around it and only its label is left on the backdrop; what
/// separates an available workspace from one with nothing behind it yet is the
/// text colour alone. An available tab still lights up under the pointer, and
/// that step is signed by theme so "lighter" means away from the background in
/// both.
fn tab_fill(visuals: &egui::Visuals, bar_fill: egui::Color32, state: TabState, hovered: bool) -> egui::Color32 {
    let away = if visuals.dark_mode { 1 } else { -1 };
    let hover = i16::from(hovered && state != TabState::Disabled) * 6;
    match state {
        TabState::Active => crate::ui::widgets::shifted(visuals.panel_fill, away * hover),
        TabState::Inactive => crate::ui::widgets::shifted(bar_fill, away * hover),
        TabState::Disabled => bar_fill,
    }
}

/// The File menu: everything about the project as a file on disk.
#[cfg(not(target_os = "macos"))]
fn draw_file_menu(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    let file_menu = tr!("menu-file");
    MenuBarMenu::new(&file_menu).show(ui, |ui| {
        let has_unsaved = project.projects.iter().any(UiProjectEntry::needs_save);
        let active_project = project.projects.iter().find(|entry| entry.is_active);
        if ContextMenuAction::new(tr!("menu-file-save-project")).enabled(has_unsaved).show(ui).clicked() {
            commands.push(UiCommand::SaveProject);
            ui.close();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if ContextMenuAction::new(tr!("menu-file-save-project-as"))
            .enabled(active_project.is_some())
            .show(ui)
            .clicked()
        {
            if let Some(project) = active_project {
                commands.push(UiCommand::SaveProjectAs(project.runtime_id));
            }
            ui.close();
        }
        context_menu_separator(ui);
        if ContextMenuAction::new(tr!("menu-file-new-project")).show(ui).clicked() {
            commands.push(UiCommand::NewProject);
            ui.close();
        }
        if ContextMenuAction::new(tr!("menu-file-open-project")).show(ui).clicked() {
            commands.push(UiCommand::OpenProject);
            ui.close();
        }
        draw_open_recent(ui, project, commands);
        // Disabled until the project is a file: a never-saved one is nowhere
        // to be shown.
        #[cfg(not(target_arch = "wasm32"))]
        if ContextMenuAction::new(show_in_file_manager_label())
            .enabled(project.active_path.is_some())
            .show(ui)
            .clicked()
        {
            commands.push(UiCommand::ShowProjectInFileManager);
            ui.close();
        }
        context_menu_separator(ui);
        if ContextMenuAction::new(tr!("menu-file-import")).enabled(active_project.is_some()).show(ui).clicked() {
            editor.show_import = true;
            editor.show_export = false;
            editor.import_as_haul_roads = false;
            ui.close();
        }
        if ContextMenuAction::new(tr!("menu-file-export")).enabled(active_project.is_some()).show(ui).clicked() {
            editor.show_import = false;
            editor.show_export = true;
            ui.close();
        }
        if ContextMenuAction::new(tr!("menu-file-export-viewport-image")).show(ui).clicked() {
            commands.push(UiCommand::ExportViewportImage);
            ui.close();
        }
        if ContextMenuAction::new(tr!("menu-file-export-engineering-drawing")).show(ui).clicked() {
            commands.push(UiCommand::OpenPlotDialog);
            ui.close();
        }
        context_menu_separator(ui);
        if ContextMenuAction::new(tr!("preferences-title")).show(ui).clicked() {
            commands.push(UiCommand::OpenPreferences);
            ui.close();
        }
        if ContextMenuAction::new(tr!("menu-file-about", app = crate::APP_NAME)).show(ui).clicked() {
            editor.show_about = true;
            ui.close();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if ContextMenuAction::new(tr!("menu-file-exit")).show(ui).clicked() {
            commands.push(UiCommand::RequestExit);
            ui.close();
        }
    });
}

/// The View menu, beside File: the view preferences reached often enough to
/// want a row of their own.
///
/// Each row is a switch onto the same setting the Interface preferences tab
/// holds - see [`crate::ui::elements::properties`] - rather than a second
/// piece of state, so a change here shows in that tab and is saved with it.
#[cfg(not(target_os = "macos"))]
fn draw_view_menu(ui: &mut egui::Ui, editor: &EditorState, commands: &mut Vec<UiCommand>) {
    let view_menu = tr!("menu-view");
    MenuBarMenu::new(&view_menu).show(ui, |ui| {
        for toggle in [ViewToggle::Console, ViewToggle::DarkMode] {
            if ContextMenuAction::new(toggle.label()).checked(toggle.get(editor)).show(ui).clicked() {
                commands.push(UiCommand::ToggleViewOption(toggle));
                ui.close();
            }
        }
    });
}

/// The File menu's Open Recent submenu: the same remembered projects the
/// welcome splash lists, so a project can be reopened without the splash or
/// the file chooser.
///
/// Greyed out with nothing to offer rather than hidden, so the menu keeps its
/// shape. The open project is not among the rows - see
/// [`UiProjectView::recent_projects`] - so none of them is ever the dirty one
/// and no `*` marker is needed.
#[cfg(not(target_os = "macos"))]
fn draw_open_recent(ui: &mut egui::Ui, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    let recent: Vec<_> = project.recent_projects().collect();
    let open_recent = tr!("menu-file-open-recent");
    context_submenu(ui, &open_recent, !recent.is_empty(), |ui| {
        for entry in recent {
            let row = ContextMenuAction::new(entry.name.as_str()).show(ui);
            // A row is a file stem, so two remembered projects can read alike;
            // the full path tells them apart.
            #[cfg(not(target_arch = "wasm32"))]
            let row = row.on_hover_text(entry.path.display().to_string());
            if row.clicked() {
                #[cfg(not(target_arch = "wasm32"))]
                commands.push(UiCommand::ActivateTrackedProject(entry.path.clone()));
                #[cfg(target_arch = "wasm32")]
                commands.push(UiCommand::ActivateTrackedProject(entry.id));
                ui.close();
            }
        }
    });
}

/// One entry of the Production Design menu. The egui menu, the macOS menu and
/// the recent functions list all read [`DESIGN_ROWS`], so they cannot drift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DesignRow {
    InsertAtIntersection,
    InsertAtElevation,
    CleanStrings,
    SetAxis(Axis),
    CreateTriangulation,
}

/// The Design menu's entries in menu order; [`design_groups`] cuts it up.
pub(crate) const DESIGN_ROWS: [DesignRow; 7] = [
    DesignRow::InsertAtIntersection,
    DesignRow::InsertAtElevation,
    DesignRow::CleanStrings,
    DesignRow::SetAxis(Axis::X),
    DesignRow::SetAxis(Axis::Y),
    DesignRow::SetAxis(Axis::Z),
    DesignRow::CreateTriangulation,
];

/// The Design menu's runs of [`DESIGN_ROWS`]: a submenu title, or `None` for
/// rows standing on the menu itself, and whether a separator goes above it.
pub(crate) fn design_groups() -> [(Option<String>, std::ops::Range<usize>, bool); 4] {
    [
        (Some(tr!("ws-menubar-design-insert-point")), 0..2, false),
        (None, 2..3, false),
        (Some(tr!("ws-menubar-design-move-to")), 3..6, true),
        (None, 6..7, true),
    ]
}

impl DesignRow {
    pub(crate) fn label(self) -> String {
        match self {
            Self::InsertAtIntersection => tr!("ws-menubar-design-insert-point-at-intersection"),
            Self::InsertAtElevation => tr!("ws-menubar-design-insert-point-at-elevation"),
            Self::CleanStrings => tr!("cmd-string-clean-clean-strings"),
            Self::SetAxis(axis) => tr!("common-set") + &format!(" {}...", axis.label()),
            Self::CreateTriangulation => tr!("ws-menubar-design-create-triangulation"),
        }
    }

    pub(crate) fn command(self) -> UiCommand {
        match self {
            Self::InsertAtIntersection => UiCommand::InsertPointsAtIntersections,
            Self::InsertAtElevation => UiCommand::OpenInsertPointAtElevationDialog,
            Self::CleanStrings => UiCommand::CleanStrings,
            Self::SetAxis(axis) => UiCommand::OpenMoveToAxisDialog(axis),
            Self::CreateTriangulation => UiCommand::OpenCreateTriangulation,
        }
    }

    pub(crate) fn enabled(self, editor: &EditorState) -> bool {
        // Every entry here acts on the current design selection.
        match self {
            // Needs two or more crossing polylines to insert anything.
            Self::InsertAtIntersection => editor.selection_has_intersections,
            Self::InsertAtElevation => editor.selection_has_polylines,
            Self::CleanStrings => editor.selection_counts.open_strings > 0,
            Self::SetAxis(_) => editor.selected_handles.iter().any(|handle| matches!(handle, SceneEntityId::Object(_))),
            // Only objects that can contribute an edge count towards it.
            Self::CreateTriangulation => editor.selection_counts.triangulation_sources > 0,
        }
    }

    /// The row's name in a saved recent list, made from its menu words'
    /// translation ids (plus the axis for Set X/Y/Z) rather than their text,
    /// so a language switch relabels the list without losing it. Not itself
    /// a translation id.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::InsertAtIntersection => "ws-menubar-design-insert-point-at-intersection",
            Self::InsertAtElevation => "ws-menubar-design-insert-point-at-elevation",
            Self::CleanStrings => "cmd-string-clean-clean-strings",
            Self::SetAxis(Axis::X) => "ws-menubar-design-move-to-x",
            Self::SetAxis(Axis::Y) => "ws-menubar-design-move-to-y",
            Self::SetAxis(Axis::Z) => "ws-menubar-design-move-to-z",
            Self::CreateTriangulation => "ws-menubar-design-create-triangulation",
        }
    }

    /// `None` for a key no row carries any more.
    pub(crate) fn from_key(key: &str) -> Option<Self> {
        DESIGN_ROWS.into_iter().find(|row| row.key() == key)
    }

    /// The way to the row through the menu bar, as the recent list shows it:
    /// "Design / Insert Point / At intersection".
    pub(crate) fn path(self) -> String {
        let index = DESIGN_ROWS.iter().position(|row| *row == self).unwrap_or_default();
        let submenu = design_groups().into_iter().find(|(_, run, _)| run.contains(&index)).and_then(|(title, _, _)| title);
        let mut parts = vec![tr!("ws-menubar-design")];
        parts.extend(submenu);
        parts.push(self.label());
        parts.join(" / ")
    }
}

/// The Design menu's entries, shared by production's Design and the top
/// of Geology Design.
#[cfg(not(target_os = "macos"))]
fn draw_design_menu_entries(ui: &mut egui::Ui, editor: &EditorState, commands: &mut Vec<UiCommand>) {
    let draw_row = |ui: &mut egui::Ui, row: DesignRow, commands: &mut Vec<UiCommand>| {
        if ContextMenuAction::new(row.label()).enabled(row.enabled(editor)).show(ui).clicked() {
            commands.push(row.command());
            commands.push(UiCommand::NoteRecentFunction(row.key()));
            ui.close();
        }
    };
    for (title, run, separated) in design_groups() {
        if separated {
            context_menu_separator(ui);
        }
        let rows = &DESIGN_ROWS[run];
        match title {
            Some(title) => {
                let enabled = rows.iter().any(|entry| entry.enabled(editor));
                context_submenu(ui, &title, enabled, |ui| rows.iter().for_each(|row| draw_row(ui, *row, commands)));
            }
            None => rows.iter().for_each(|row| draw_row(ui, *row, commands)),
        }
    }
}

/// One row of Geology Design's lower half: a tool it arms, or a command it
/// pushes. Leo's Design functions sit here too, so the half is a superset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeologyRow {
    Tool(ActiveTool),
    /// One of the Design menu's rows, run and enabled as that menu runs it.
    Design(DesignRow),
    InsertAtSpacing,
    Reverse,
    DrapeAlongTriangles,
    Thin,
    Smooth,
    RemoveLastVertex,
    NextVertex,
    CopyAlongString,
    CarryOnString,
    RedrawRun,
    DragVertex,
    NudgeRl,
    SurfaceLines,
    SaveSurfaceLines,
}

/// Geology Design's lower half in menu order; [`geology_design_groups`] cuts
/// it into submenus and the macOS menu tags rows by index into it.
pub(crate) const GEOLOGY_DESIGN_ROWS: [GeologyRow; 44] = [
    GeologyRow::Tool(ActiveTool::MakePoint),
    GeologyRow::Tool(ActiveTool::MakeLine),
    GeologyRow::Tool(ActiveTool::MakePoly),
    GeologyRow::Tool(ActiveTool::MakeCircle),
    GeologyRow::Tool(ActiveTool::MakeText),
    GeologyRow::RemoveLastVertex,
    GeologyRow::NextVertex,
    GeologyRow::CopyAlongString,
    GeologyRow::CarryOnString,
    GeologyRow::RedrawRun,
    GeologyRow::Tool(ActiveTool::Move),
    GeologyRow::Design(DesignRow::SetAxis(Axis::X)),
    GeologyRow::Design(DesignRow::SetAxis(Axis::Y)),
    GeologyRow::Design(DesignRow::SetAxis(Axis::Z)),
    GeologyRow::Tool(ActiveTool::OffsetElement),
    GeologyRow::Tool(ActiveTool::RelimitLine),
    GeologyRow::Tool(ActiveTool::Bezier),
    GeologyRow::Tool(ActiveTool::Chamfer),
    GeologyRow::Reverse,
    GeologyRow::Design(DesignRow::CleanStrings),
    GeologyRow::Tool(ActiveTool::FuseIntoPolyline),
    GeologyRow::Tool(ActiveTool::SplitAtPoints),
    GeologyRow::Tool(ActiveTool::SplitAtCrossings),
    GeologyRow::Tool(ActiveTool::RemoveBetweenCrossings),
    GeologyRow::Tool(ActiveTool::ExplodePolyline),
    GeologyRow::Tool(ActiveTool::ExplodePolylineToPoints),
    GeologyRow::Tool(ActiveTool::EditVertex),
    GeologyRow::Tool(ActiveTool::InsertPoint),
    GeologyRow::Tool(ActiveTool::FillSegment),
    GeologyRow::InsertAtSpacing,
    GeologyRow::DragVertex,
    GeologyRow::NudgeRl,
    GeologyRow::Tool(ActiveTool::DeletePoints),
    GeologyRow::Design(DesignRow::InsertAtIntersection),
    GeologyRow::Design(DesignRow::InsertAtElevation),
    GeologyRow::Thin,
    GeologyRow::Smooth,
    GeologyRow::Tool(ActiveTool::DrapeToTopology),
    GeologyRow::DrapeAlongTriangles,
    GeologyRow::Design(DesignRow::CreateTriangulation),
    GeologyRow::Tool(ActiveTool::MeasureBatterAngle),
    GeologyRow::Tool(ActiveTool::MeasureDistance),
    GeologyRow::SurfaceLines,
    GeologyRow::SaveSurfaceLines,
];

/// Geology Design's submenus: title and run of [`GEOLOGY_DESIGN_ROWS`]. The
/// rows after the last run stand on the menu itself.
pub(crate) fn geology_design_groups() -> [(String, std::ops::Range<usize>); 5] {
    [
        (tr!("ws-menubar-geology-draw"), 0..10),
        (tr!("ws-menubar-geology-edit"), 10..20),
        (tr!("ws-menubar-geology-join-split"), 20..26),
        (tr!("ws-menubar-geology-vertices"), 26..37),
        (tr!("ws-menubar-geology-surface"), 37..40),
    ]
}

impl GeologyRow {
    /// The row's words, its toolbar cell's or Design entry's where it has one.
    pub(crate) fn label(self) -> String {
        match self {
            Self::Tool(tool) => match tool {
                ActiveTool::MakePoint => tr!("common-create-point"),
                ActiveTool::MakeLine => tr!("common-create-line"),
                ActiveTool::MakePoly => tr!("common-create-polyline"),
                ActiveTool::MakeCircle => tr!("common-create-circle"),
                ActiveTool::MakeText => tr!("toolbars-create-text"),
                ActiveTool::Move => tr!("common-move-design"),
                ActiveTool::OffsetElement => tr!("common-offset"),
                ActiveTool::RelimitLine => tr!("common-relimit-line"),
                ActiveTool::Bezier => tr!("toolbars-bezier-polyline"),
                ActiveTool::Chamfer => tr!("toolbars-chamfer-polyline-corners"),
                ActiveTool::FuseIntoPolyline => tr!("toolbars-fuse-polylines"),
                ActiveTool::SplitAtPoints => tr!("toolbars-split-polyline-points"),
                ActiveTool::SplitAtCrossings => tr!("ws-menubar-geology-split-at-crossings"),
                ActiveTool::RemoveBetweenCrossings => tr!("ws-menubar-geology-remove-between-crossings"),
                ActiveTool::ExplodePolyline => tr!("toolbars-explode-polyline-lines"),
                ActiveTool::ExplodePolylineToPoints => tr!("ws-menubar-geology-explode-to-points"),
                ActiveTool::DeletePoints => tr!("toolbars-delete-points"),
                ActiveTool::EditVertex => tr!("toolbars-edit-vertex"),
                ActiveTool::InsertPoint => tr!("ws-menubar-geology-insert-point"),
                ActiveTool::FillSegment => tr!("ws-menubar-geology-fill-segment"),
                ActiveTool::DrapeToTopology => tr!("common-drape-topology"),
                ActiveTool::MeasureDistance => tr!("toolbars-measure-distance"),
                _ => tr!("toolbars-strike-dip"),
            },
            Self::Design(DesignRow::InsertAtIntersection) => tr!("toolbars-insert-points-crossings"),
            Self::Design(DesignRow::InsertAtElevation) => tr!("ws-menubar-geology-insert-at-elevation"),
            Self::Design(row) => row.label(),
            Self::InsertAtSpacing => tr!("ws-menubar-geology-insert-at-spacing"),
            Self::Reverse => tr!("toolbars-reverse-strings"),
            Self::DrapeAlongTriangles => tr!("ws-menubar-geology-drape-along-triangles"),
            Self::Thin => tr!("ws-menubar-geology-thin"),
            Self::Smooth => tr!("ws-menubar-geology-smooth"),
            Self::RemoveLastVertex => tr!("ws-menubar-geology-remove-last-vertex"),
            Self::NextVertex => tr!("ws-menubar-geology-next-vertex"),
            Self::CopyAlongString => tr!("ws-menubar-geology-copy-along-string"),
            Self::CarryOnString => tr!("ws-menubar-geology-carry-on-string"),
            Self::RedrawRun => tr!("ws-menubar-geology-redraw-run"),
            Self::DragVertex => tr!("ws-menubar-geology-drag-vertex"),
            Self::NudgeRl => tr!("ws-menubar-geology-nudge-rl"),
            Self::SurfaceLines => tr!("ws-menubar-geology-surface-lines"),
            Self::SaveSurfaceLines => tr!("ws-menubar-geology-save-surface-lines"),
        }
    }

    pub(crate) fn command(self) -> UiCommand {
        match self {
            Self::Tool(tool) => UiCommand::SetActiveTool(tool),
            Self::Design(row) => row.command(),
            Self::InsertAtSpacing => UiCommand::OpenInsertPointAtSpacingDialog,
            Self::Reverse => UiCommand::ReverseSelectedStrings,
            Self::DrapeAlongTriangles => UiCommand::ArmDrapeAlongTriangles,
            Self::Thin => UiCommand::OpenThinStringsDialog,
            Self::Smooth => UiCommand::OpenSmoothStringsDialog,
            Self::RemoveLastVertex => UiCommand::RemoveLastStrokeVertex,
            Self::NextVertex => UiCommand::OpenNextVertexPanel,
            Self::CopyAlongString => UiCommand::ArmCopyAlongString,
            Self::CarryOnString => UiCommand::ArmCarryOnString,
            Self::RedrawRun => UiCommand::ArmRedrawRun,
            Self::DragVertex => UiCommand::ArmDragVertex,
            Self::NudgeRl => UiCommand::OpenNudgePanel,
            Self::SurfaceLines => UiCommand::ToggleSectionSurfaceLines,
            Self::SaveSurfaceLines => UiCommand::SaveSectionSurfaceLines,
        }
    }

    /// Whether the row can run now: Leo's rows on the selection his Design
    /// entries ask for, a tool as its toolbar cell is (layer, fly, section).
    pub(crate) fn enabled(self, editor: &EditorState) -> bool {
        match self {
            Self::Tool(tool) => {
                !editor.fly_mode_enabled
                    && (!tool.requires_active_layer() || editor.active_layer.is_some())
                    && !(editor.slice_mode_enabled && tool.section_refuses(editor.active_workspace))
            }
            Self::Design(row) => row.enabled(editor),
            Self::InsertAtSpacing | Self::Reverse | Self::Thin | Self::Smooth => editor.selection_has_polylines,
            Self::DrapeAlongTriangles => true,
            // Carrying on, the string's end is not the stroke's to give back.
            Self::RemoveLastVertex => {
                !editor.pending_stroke.is_empty() && !editor.poly_finish_dialog && !(editor.pending_stroke.len() == 1 && string_rework_live(editor).is_some())
            }
            Self::NextVertex => matches!(editor.active_tool, ActiveTool::MakeLine | ActiveTool::MakePoly),
            Self::CopyAlongString => !editor.poly_finish_dialog,
            Self::CarryOnString | Self::RedrawRun => editor.pending_stroke.is_empty() && !editor.poly_finish_dialog,
            // Move works on a section in Geology and needs no layer.
            Self::DragVertex | Self::NudgeRl => !editor.fly_mode_enabled,
            Self::SurfaceLines => true,
            // Saved only while drawn: the switch on and a section up.
            Self::SaveSurfaceLines => editor.slice_mode_enabled && editor.section_surface_lines_on(),
        }
    }

    /// A switch row's tick; `None` for a row that is not a switch.
    pub(crate) fn checked(self, editor: &EditorState) -> Option<bool> {
        (self == Self::SurfaceLines).then_some(editor.section_surface_lines)
    }

    /// The row's name in a saved recent list, from its words' translation id
    /// as [`DesignRow::key`]; a Design menu row keeps its own key, so a use
    /// from either half of Geology Design is the one line.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Tool(tool) => match tool {
                ActiveTool::MakePoint => "common-create-point",
                ActiveTool::MakeLine => "common-create-line",
                ActiveTool::MakePoly => "common-create-polyline",
                ActiveTool::MakeCircle => "common-create-circle",
                ActiveTool::MakeText => "toolbars-create-text",
                ActiveTool::Move => "common-move-design",
                ActiveTool::OffsetElement => "common-offset",
                ActiveTool::RelimitLine => "common-relimit-line",
                ActiveTool::Bezier => "toolbars-bezier-polyline",
                ActiveTool::Chamfer => "toolbars-chamfer-polyline-corners",
                ActiveTool::FuseIntoPolyline => "toolbars-fuse-polylines",
                ActiveTool::SplitAtPoints => "toolbars-split-polyline-points",
                ActiveTool::SplitAtCrossings => "ws-menubar-geology-split-at-crossings",
                ActiveTool::RemoveBetweenCrossings => "ws-menubar-geology-remove-between-crossings",
                ActiveTool::ExplodePolyline => "toolbars-explode-polyline-lines",
                ActiveTool::ExplodePolylineToPoints => "ws-menubar-geology-explode-to-points",
                ActiveTool::DeletePoints => "toolbars-delete-points",
                ActiveTool::EditVertex => "toolbars-edit-vertex",
                ActiveTool::InsertPoint => "ws-menubar-geology-insert-point",
                ActiveTool::FillSegment => "ws-menubar-geology-fill-segment",
                ActiveTool::DrapeToTopology => "common-drape-topology",
                ActiveTool::MeasureDistance => "toolbars-measure-distance",
                ActiveTool::MeasureBatterAngle => "toolbars-strike-dip",
                // No row arms these; a tool joining the table needs its own
                // key here, so the match names every tool.
                ActiveTool::None
                | ActiveTool::Hide
                | ActiveTool::MoveCollar
                | ActiveTool::RotateCollar
                | ActiveTool::TieHoles
                | ActiveTool::SetInitiationPoint
                | ActiveTool::ChargeHoles
                | ActiveTool::PickRotationCentre
                | ActiveTool::BatterBermOffset
                | ActiveTool::VerticalSlice => "",
            },
            Self::Design(row) => row.key(),
            Self::InsertAtSpacing => "ws-menubar-geology-insert-at-spacing",
            Self::Reverse => "toolbars-reverse-strings",
            Self::DrapeAlongTriangles => "ws-menubar-geology-drape-along-triangles",
            Self::Thin => "ws-menubar-geology-thin",
            Self::Smooth => "ws-menubar-geology-smooth",
            Self::RemoveLastVertex => "ws-menubar-geology-remove-last-vertex",
            Self::NextVertex => "ws-menubar-geology-next-vertex",
            Self::CopyAlongString => "ws-menubar-geology-copy-along-string",
            Self::CarryOnString => "ws-menubar-geology-carry-on-string",
            Self::RedrawRun => "ws-menubar-geology-redraw-run",
            Self::DragVertex => "ws-menubar-geology-drag-vertex",
            Self::NudgeRl => "ws-menubar-geology-nudge-rl",
            Self::SurfaceLines => "ws-menubar-geology-surface-lines",
            Self::SaveSurfaceLines => "ws-menubar-geology-save-surface-lines",
        }
    }

    /// `None` for a key no row carries any more.
    pub(crate) fn from_key(key: &str) -> Option<Self> {
        GEOLOGY_DESIGN_ROWS.into_iter().find(|row| row.key() == key)
    }

    /// The way to the row through the menu bar: "Geology Design / Draw /
    /// Create Point".
    pub(crate) fn path(self) -> String {
        let index = GEOLOGY_DESIGN_ROWS.iter().position(|row| *row == self).unwrap_or_default();
        let submenu = geology_design_groups().into_iter().find(|(_, run)| run.contains(&index)).map(|(title, _)| title);
        let mut parts = vec![tr!("ws-menubar-geology-design")];
        parts.extend(submenu);
        parts.push(self.label());
        parts.join(" / ")
    }
}

/// A line of a recent functions list: a row of the Design menu of the
/// workspace that keeps the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecentRow {
    Design(DesignRow),
    Geology(GeologyRow),
}

impl RecentRow {
    /// The row `workspace` keeps under `key`; `None` where it keeps no list
    /// or no row carries the key.
    pub(crate) fn from_key(workspace: Workspace, key: &str) -> Option<Self> {
        match workspace {
            Workspace::Production => DesignRow::from_key(key).map(Self::Design),
            Workspace::Geology => GeologyRow::from_key(key).map(Self::Geology),
            _ => None,
        }
    }

    /// The row in `workspace` that pushes `command`, so a toolbar cell
    /// running the same function as a menu row counts as a use of it.
    pub(crate) fn running(workspace: Workspace, command: &UiCommand) -> Option<Self> {
        match workspace {
            Workspace::Production => DESIGN_ROWS.into_iter().find(|row| row.command() == *command).map(Self::Design),
            Workspace::Geology => GEOLOGY_DESIGN_ROWS.into_iter().find(|row| row.command() == *command).map(Self::Geology),
            _ => None,
        }
    }

    /// Whether the row, just run, was used: a tool row's command toggles, so
    /// a click that put its tool down, or that the tool refused, is not.
    pub(crate) fn was_used(self, editor: &EditorState) -> bool {
        match self {
            Self::Geology(GeologyRow::Tool(tool)) => editor.active_tool == tool,
            _ => true,
        }
    }

    pub(crate) fn path(self) -> String {
        match self {
            Self::Design(row) => row.path(),
            Self::Geology(row) => row.path(),
        }
    }

    pub(crate) fn command(self) -> UiCommand {
        match self {
            Self::Design(row) => row.command(),
            Self::Geology(row) => row.command(),
        }
    }

    pub(crate) fn enabled(self, editor: &EditorState) -> bool {
        match self {
            Self::Design(row) => row.enabled(editor),
            Self::Geology(row) => row.enabled(editor),
        }
    }

    pub(crate) fn checked(self, editor: &EditorState) -> Option<bool> {
        match self {
            Self::Design(_) => None,
            Self::Geology(row) => row.checked(editor),
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Design(row) => row.key(),
            Self::Geology(row) => row.key(),
        }
    }
}

/// Geology Design's lower half: Leo's Design functions and every R116 tool,
/// grouped by what each does to a string.
#[cfg(not(target_os = "macos"))]
fn draw_geology_design_entries(ui: &mut egui::Ui, editor: &EditorState, commands: &mut Vec<UiCommand>) {
    let row = |ui: &mut egui::Ui, row: GeologyRow, commands: &mut Vec<UiCommand>| {
        let mut action = ContextMenuAction::new(row.label()).enabled(row.enabled(editor));
        if let Some(checked) = row.checked(editor) {
            action = action.checked(checked);
        }
        if action.show(ui).clicked() {
            commands.push(row.command());
            commands.push(UiCommand::NoteRecentFunction(row.key()));
            ui.close();
        }
    };
    let groups = geology_design_groups();
    for (title, run) in &groups {
        context_submenu(ui, title, true, |ui| {
            for &each in &GEOLOGY_DESIGN_ROWS[run.clone()] {
                row(ui, each, commands);
            }
        });
    }
    for &each in &GEOLOGY_DESIGN_ROWS[groups[groups.len() - 1].1.end..] {
        row(ui, each, commands);
    }
}

/// Draw the menus belonging to the active workspace, for the viewport bar to
/// place at the head of its own row.
///
/// These act on what is in the scene rather than on the application, so they
/// follow the workspace rather than sitting in the menu bar above it. On macOS
/// they are in the system menu bar instead and this draws nothing.
///
/// `row_height` is how tall a label's hover fill is drawn: the bar is taller
/// than the labels need, so that fill sits inside it rather than reaching its
/// edges.
///
/// Deliberately not wrapped in an `egui::MenuBar`, which claims the whole
/// width it is given: these menus are one cluster in a row with two more, so
/// they take only what they use. Every dropdown here is a `MenuBarMenu`, which
/// carries its own menu config rather than reading the bar's, so the only
/// thing the wrapper would have added is the flat button style below.
pub(crate) fn draw_workspace_menus(ui: &mut egui::Ui, editor: &EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>, row_height: f32) {
    #[cfg(target_os = "macos")]
    {
        let _ = (ui, editor, project, commands, row_height);
    }

    #[cfg(not(target_os = "macos"))]
    ui.scope(|ui| {
        egui::containers::menu::menu_style(ui.style_mut());
        // After `menu_style`, which sets the padding this overrides.
        let spacing = &mut ui.style_mut().spacing;
        spacing.interact_size.y = row_height;
        spacing.button_padding = egui::vec2(MENU_LABEL_PADDING, 0.0);
        spacing.item_spacing.x = MENU_LABEL_GAP;

        if editor.active_workspace == Workspace::Planning {
            MenuBarMenu::new(&tr!("haul-roads")).enabled(project.has_active_project).show(ui, |ui| {
                for (label, command) in [
                    (tr!("haul-draw"), UiCommand::StartHaulRoad),
                    (tr!("haul-convert"), UiCommand::ConvertHaulSelection),
                    (tr!("haul-import"), UiCommand::OpenHaulImport),
                    (tr!("haul-export"), UiCommand::ExportHaulRoads),
                    (tr!("haul-open-layout"), UiCommand::EditHaulProperties),
                ] {
                    if ContextMenuAction::new(label).show(ui).clicked() {
                        commands.push(command);
                        ui.close();
                    }
                }
            });
        }
        // Drill & Blast and Planning carry no discipline menus of their own
        // yet, and the run is what the workspace has rather than a fixed set of
        // titles: a menu that opens on nothing is left off it.
        if matches!(editor.active_workspace, Workspace::DrillAndBlast | Workspace::Planning) {
            return;
        }

        if editor.active_workspace == Workspace::Survey {
            MenuBarMenu::new(&tr!("survey-coordinates-menu")).show(ui, |ui| {
                if ContextMenuAction::new(tr!("survey-definitions-action")).show(ui).clicked() {
                    commands.push(UiCommand::OpenSurveyDefinitions);
                    ui.close();
                }
                if ContextMenuAction::new(tr!("survey-transform-action"))
                    .enabled(project.has_active_project)
                    .show(ui)
                    .clicked()
                {
                    commands.push(UiCommand::OpenSurveyTransform);
                    ui.close();
                }
            });

            MenuBarMenu::new(&tr!("ws-menubar-point-cloud")).show(ui, |ui| {
                // Every entry runs on the selected clouds: one is reconstructed
                // at a time, a join needs two or more to join, and classifying
                // takes any number.
                let selected_clouds = editor.selection_counts.point_clouds;
                if ContextMenuAction::new(tr!("common-create-triangulation")).enabled(selected_clouds == 1).show(ui).clicked() {
                    commands.push(UiCommand::OpenPointCloudTin);
                    ui.close();
                }
                if ContextMenuAction::new(tr!("common-join")).enabled(selected_clouds >= 2).show(ui).clicked() {
                    commands.push(UiCommand::OpenPointCloudJoin);
                    ui.close();
                }
                if ContextMenuAction::new(tr!("common-classify")).enabled(selected_clouds >= 1).show(ui).clicked() {
                    commands.push(UiCommand::OpenPointCloudClassify);
                    ui.close();
                }
            });
            return;
        }

        if editor.active_workspace == Workspace::Geology {
            MenuBarMenu::new(&tr!("ws-menubar-geology-design")).show(ui, |ui| {
                context_menu_heading(ui, &tr!("ws-menubar-production-design"));
                draw_design_menu_entries(ui, editor, commands);
                context_menu_separator(ui);
                context_menu_heading(ui, &tr!("ws-menubar-geology-design"));
                draw_geology_design_entries(ui, editor, commands);
            });
            // Thresholding runs on the block model, so - like the entry below
            // it - the menu is the input's own and the one selected model is
            // what the tool opens on.
            MenuBarMenu::new(&tr!("ws-menubar-block-model")).show(ui, |ui| {
                if ContextMenuAction::new(tr!("common-create-ore-triangulation-ellipsis"))
                    .enabled(editor.selection_counts.block_models == 1)
                    .show(ui)
                    .clicked()
                {
                    commands.push(UiCommand::OpenCreateOreTriangulation);
                    ui.close();
                }
            });

            // Estimation runs on the drill holes, so the entry lives under
            // them and takes the one selected collection as its input, the
            // way the other select-first tools do.
            MenuBarMenu::new(&tr!("ws-menubar-drillholes")).show(ui, |ui| {
                if ContextMenuAction::new(tr!("common-create-block-model-ellipsis"))
                    .enabled(editor.selection_counts.drill_holes == 1)
                    .show(ui)
                    .clicked()
                {
                    commands.push(UiCommand::OpenCreateBlockModel);
                    ui.close();
                }
            });

            MenuBarMenu::new(&tr!("ws-menubar-modelling")).show(ui, |ui| {
                // Select first, then act: placed on the holes selected at open.
                let can_build_points = editor.selection_counts.reference_holes > 0;
                if ContextMenuAction::new(tr!("common-reference-points"))
                    .enabled(can_build_points)
                    .disabled_hover_text(tr!("ws-menubar-modelling-select-holes"))
                    .show(ui)
                    .clicked()
                {
                    commands.push(UiCommand::OpenReferencePoints);
                    ui.close();
                }
                // Select first, then act: the surface is built from the points
                // that are selected when it opens, not from a layer picked
                // inside the dialog.
                let can_build_surface = editor.selection_counts.surface_points >= crate::app::commands::triangulation::reference_surface::MINIMUM_POINTS;
                if ContextMenuAction::new(tr!("common-build-surface-ellipsis"))
                    .enabled(can_build_surface)
                    .disabled_hover_text(tr!(
                        "ws-menubar-modelling-select-points",
                        count = crate::app::commands::triangulation::reference_surface::MINIMUM_POINTS.to_string()
                    ))
                    .show(ui)
                    .clicked()
                {
                    commands.push(UiCommand::OpenReferenceSurface);
                    ui.close();
                }
                context_menu_separator(ui);
                // Select first, then act: measured against the one surface
                // selected, for the seam it was built from.
                let can_make_thickness = editor.selection_counts.triangulations == 1;
                if ContextMenuAction::new(tr!("common-thickness-points-ellipsis"))
                    .enabled(can_make_thickness)
                    .disabled_hover_text(tr!("ws-menubar-modelling-select-surface"))
                    .show(ui)
                    .clicked()
                {
                    commands.push(UiCommand::OpenThicknessPoints);
                    ui.close();
                }
                // Select first, then act: the same one surface, once its
                // thickness points are made.
                if ContextMenuAction::new(tr!("common-thickness-surfaces-ellipsis"))
                    .enabled(can_make_thickness)
                    .disabled_hover_text(tr!("ws-menubar-modelling-select-surface"))
                    .show(ui)
                    .clicked()
                {
                    commands.push(UiCommand::OpenSeamSurface);
                    ui.close();
                }
                context_menu_separator(ui);
                // Select first, then act: the seam's roof and floor selected are
                // clipped; the limits, surfaces of the same kind, are picked in
                // the dialog.
                if ContextMenuAction::new(tr!("common-clip-to-surface-ellipsis"))
                    .enabled(editor.selection_counts.triangulations == 2)
                    .disabled_hover_text(tr!("ws-menubar-modelling-select-surfaces"))
                    .show(ui)
                    .clicked()
                {
                    commands.push(UiCommand::OpenCutTriangulationToSurface);
                    ui.close();
                }
            });
            return;
        }

        MenuBarMenu::new(&tr!("ws-menubar-design")).show(ui, |ui| draw_design_menu_entries(ui, editor, commands));

        MenuBarMenu::new(&tr!("ws-menubar-triangulation")).show(ui, |ui| {
            // The tools below whose inputs the selection can name take them
            // from it; the tools taking two surfaces cannot tell one selected
            // surface from the other, so they still pick theirs in the dialog.
            let one_surface_selected = editor.selection_counts.triangulations == 1;
            let can_clip = one_surface_selected && editor.selection_counts.clip_boundaries == 1;
            if ContextMenuAction::new(tr!("common-clip-surface-polyline")).enabled(can_clip).show(ui).clicked() {
                commands.push(UiCommand::OpenCutTriangulationByPolyline);
                ui.close();
            }
            if ContextMenuAction::new(tr!("common-slice-triangulation-z-range"))
                .enabled(one_surface_selected)
                .show(ui)
                .clicked()
            {
                commands.push(UiCommand::OpenCutTriangulationByZ);
                ui.close();
            }
            if ContextMenuAction::new(tr!("common-trim-topology")).show(ui).clicked() {
                commands.push(UiCommand::OpenCutTriangulationBySurface);
                ui.close();
            }
            if ContextMenuAction::new(tr!("common-clip-to-surface-ellipsis"))
                .enabled(editor.selection_counts.triangulations == 2)
                .disabled_hover_text(tr!("ws-menubar-modelling-select-surfaces"))
                .show(ui)
                .clicked()
            {
                commands.push(UiCommand::OpenCutTriangulationToSurface);
                ui.close();
            }
            context_menu_separator(ui);
            if ContextMenuAction::new(tr!("common-cut-topology-pit-shell")).show(ui).clicked() {
                commands.push(UiCommand::OpenCutTopologyByPitShell);
                ui.close();
            }
            if ContextMenuAction::new(tr!("common-merge-shell-into-topology-ellipsis")).show(ui).clicked() {
                commands.push(UiCommand::OpenIncludeSolidInTopology);
                ui.close();
            }
            if ContextMenuAction::new(tr!("menu-build-solid-surfaces")).show(ui).clicked() {
                commands.push(UiCommand::OpenBuildSolidFromSurfaces);
                ui.close();
            }
            context_menu_separator(ui);
            if ContextMenuAction::new(tr!("common-generate-contour-lines"))
                .enabled(one_surface_selected)
                .show(ui)
                .clicked()
            {
                commands.push(UiCommand::OpenContourTriangulation);
                ui.close();
            }
        });

        // Pit optimization works over a block model, but it opens on its
        // scenarios rather than on a selection: each scenario names its own.
        MenuBarMenu::new(&tr!("ws-menubar-block-model")).show(ui, |ui| {
            if ContextMenuAction::new(tr!("menu-pit-optimization")).show(ui).clicked() {
                commands.push(UiCommand::OpenOptimizationScenarios);
                ui.close();
            }
        });

        MenuBarMenu::new(&tr!("ws-menubar-raster")).show(ui, |ui| {
            let any_draped = project.raster_textures.iter().any(|raster| raster.is_draped);
            if ContextMenuAction::new(tr!("common-undrape-all")).enabled(any_draped).show(ui).clicked() {
                commands.push(UiCommand::UndrapeAllRasters);
                ui.close();
            }
        });
    });
}
