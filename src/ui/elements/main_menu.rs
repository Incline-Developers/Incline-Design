//! Top application menu bar: the mark, the application-wide menus (File,
//! Project) and the workspace tabs. It is the window's title bar as well, and
//! [`title_bar`] adds the project's name and the window's buttons to it.
//!
//! Everything that acts on the scene rather than on the application has moved
//! down to the viewport bar - see [`crate::ui::elements::viewport_bar`] - so
//! this bar carries only what is true whichever workspace is open, the way
//! Blender's topbar does.

// The dropdowns below are all in the system menu bar on macOS (`mac.rs`), and
// the mark gives way to the system's window buttons there, so nothing this
// module still draws there needs any of this.
#[cfg(not(target_os = "macos"))]
use crate::ui::{
    state::{UiProjectEntry, ViewToggle},
    themed_icon,
    widgets::context_menu::{ContextMenuAction, MenuBarMenu, context_menu_separator, context_submenu},
};
use crate::{
    i18n::tr,
    model::{Axis, SceneEntityId},
    ui::{
        EditorState, UiCommand, UiProjectView,
        elements::title_bar,
        state::{ActiveTool, Workspace},
        widgets::page_tabs::{PageTab, PageTabsPaint},
    },
};

/// Extra space above the bar's contents, on top of the panel frame's own
/// margin, so the menus and tabs are not pinned to the window's top edge.
const TOP_PADDING: i8 = 3;
/// Side of the application mark at the left of the bar. Not on macOS, where
/// the system's window buttons stand at the head of the bar instead.
#[cfg(not(target_os = "macos"))]
const LOGO_SIZE: f32 = 18.0;
/// Space either side of the mark, so it reads as a mark rather than as the
/// first menu.
#[cfg(not(target_os = "macos"))]
const LOGO_MARGIN: f32 = 6.0;
/// Space between the run of menus and tabs and the window's buttons, at
/// either end of the bar.
const WINDOW_BUTTONS_GAP: f32 = 8.0;
/// Space between the last menu and the workspace tabs, on top of the bar's
/// item spacing, so the two runs read as two rather than one.
#[cfg(not(target_os = "macos"))]
const MENUS_TABS_GAP: f32 = 2.0;
/// How far below the row's centre the bar's contents sit - the mark, the
/// menus and the workspace tabs' names. Centred on the
/// row they ride high, the bar's top padding being only the window's edge.
/// See [`lowered`].
const LABEL_DROP: f32 = 2.0;
/// Space either side of a workspace tab's label.
const TAB_PADDING: f32 = 10.0;
/// Vertical space a workspace tab claims in the bar.
const TAB_HEIGHT: f32 = 20.0;
/// Height of a workspace tab's cell, sat against the top of that space, which
/// its label is centred in. The open tab's surface runs on below it, down to
/// the viewport bar it stands on.
const TAB_CELL_HEIGHT: f32 = 17.0;
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
/// Returns the panel's bounding rect, and the workspace tabs to paint once the
/// regions under the bar are done: the open one stands on the viewport bar.
pub(crate) fn draw_main_menu(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) -> (egui::Rect, Option<PageTabsPaint>) {
    // Not a region: the bar spans the window and carries the gap's own colour,
    // so it reads as the backdrop the workspace is laid on rather than as a
    // panel of its own. See `chrome::window_bar_frame`.
    let frame = crate::ui::chrome::window_bar_frame(ui);
    let mut tabs = None;
    let frame = frame.inner_margin(egui::Margin {
        top: frame.inner_margin.top + TOP_PADDING,
        ..frame.inner_margin
    });
    let rect = egui::Panel::top("main_menu")
        .show_separator_line(false)
        .frame(frame)
        .show(ui, |ui| {
            // The bar is the window's title bar too: see `elements::title_bar`.
            let inner = ui.max_rect();
            let bar = inner + frame.inner_margin;
            let row = egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), ui.spacing().interact_size.y));
            title_bar::interact_background(ui, bar, commands);
            let controls_left = title_bar::draw_window_controls(ui, bar, row, &mut editor.window_frame, commands);
            let strip_rect = inner.with_max_x(inner.right().min(controls_left - WINDOW_BUTTONS_GAP));
            // Room for the system's window buttons, where they are over the
            // bar, measured from the strip's own left so a scrolled strip
            // keeps the same space.
            let leading_space = (editor.window_frame.leading_inset > 0.0).then(|| (editor.window_frame.leading_inset + WINDOW_BUTTONS_GAP - strip_rect.left()).max(0.0));
            let mut tabs_right = strip_rect.right();
            ui.scope_builder(egui::UiBuilder::new().max_rect(strip_rect), |ui| {
                // Too narrow a window scrolls the row rather than clipping the
                // workspace tabs off the end of it. See `elements::bar_strip`.
                // The row is as tall as `MenuBar` makes it, which is this unless
                // something on it is taller; the strip grows with the contents.
                crate::ui::elements::bar_strip(ui, "main_menu_strip", ui.spacing().interact_size.y, |ui, strip| {
                    let mut used = 0.0;
                    ui.scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
                        egui::MenuBar::new().ui(ui, |ui| {
                            if let Some(space) = leading_space {
                                ui.add_space(space);
                            }

                            #[cfg(not(target_os = "macos"))]
                            lowered(ui, |ui| {
                                draw_logo(ui);
                                draw_file_menu(ui, editor, project, commands);
                                draw_view_menu(ui, editor, commands);
                                ui.add_space(MENUS_TABS_GAP);
                            });

                            tabs = Some(draw_workspace_tabs(ui, editor, commands));

                            // `MenuBar` sizes itself to the whole width it is given,
                            // so its rect says nothing about how much of the row was
                            // used; where the next item would go does.
                            used = ui.cursor().left() - strip.left();
                        });
                    });
                    tabs_right = (strip.left() + used).min(strip_rect.right());
                    used
                });
            });
            let title_span = egui::Rangef::new(tabs_right, controls_left);
            title_bar::draw_title(ui, bar, title_span, row.center().y + LABEL_DROP, project);
        })
        .response
        .rect;
    (rect, tabs)
}

/// Lay `add_contents` out along the bar [`LABEL_DROP`] lower than the row
/// centres it, taking the same room in the row as if it had not moved.
///
/// Moving the row itself down would move the bar's height with it, so its
/// contents are lowered within it instead.
#[cfg(not(target_os = "macos"))]
fn lowered(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let drop = egui::vec2(0.0, LABEL_DROP);
    let row = ui.available_rect_before_wrap();
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row.translate(drop)).layout(*ui.layout()));
    add_contents(&mut child);
    ui.advance_cursor_after_rect(child.min_rect().translate(-drop));
}

/// Paint the application mark at the head of the bar.
///
/// Flat and one-coloured: beside the menu labels it is a glyph, not artwork.
#[cfg(not(target_os = "macos"))]
fn draw_logo(ui: &mut egui::Ui) {
    ui.add_space(LOGO_MARGIN);
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(LOGO_SIZE), egui::Sense::hover());
    egui::Image::new(themed_icon!(ui, "logo.svg")).fit_to_exact_size(rect.size()).paint_at(ui, rect);
    ui.add_space(LOGO_MARGIN);
}

/// Draw the run of workspace tabs, always in [`Workspace::ALL`]'s order.
///
/// The open workspace stands up out of the bar onto the viewport bar under it,
/// a browser's open tab (`widgets::page_tabs`), sliding to the next one picked.
/// The other tabs are labels on the bar itself. They are laid out and take
/// their clicks here, and are painted once the regions are, since the open one
/// reaches down to them.
fn draw_workspace_tabs(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) -> PageTabsPaint {
    let spacing = ui.spacing().item_spacing.x;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let widths = Workspace::ALL.map(|workspace| ui.painter().layout_no_wrap(workspace.label(), font.clone(), egui::Color32::PLACEHOLDER).size().x + TAB_PADDING * 2.0);
    let total_width = widths.iter().sum::<f32>() + spacing * (Workspace::ALL.len() - 1) as f32;
    let (strip, _) = ui.allocate_exact_size(egui::vec2(total_width, TAB_HEIGHT), egui::Sense::hover());
    // The line the tabs' names are centred on, which the system's window
    // buttons on macOS are centred on too. See `PageTabsPaint::label_offset`.
    editor.window_frame.buttons_centre_y = strip.top() + TAB_CELL_HEIGHT / 2.0 + LABEL_DROP;
    let mut x = strip.left();
    let mut painted = Vec::with_capacity(Workspace::ALL.len());
    for (workspace, width) in Workspace::ALL.into_iter().zip(widths) {
        let id = ui.scope_id().with(("workspace-tab", workspace));
        let rect = egui::Rect::from_min_size(egui::pos2(x, strip.top()), egui::vec2(width, TAB_CELL_HEIGHT));
        painted.push(draw_workspace_tab(ui, editor, commands, workspace, id, rect));
        x += width + spacing;
    }
    let selected = Workspace::ALL.iter().position(|workspace| *workspace == editor.active_workspace);
    PageTabsPaint::new(ui, ui.scope_id().with("workspace-tab-tile"), painted, selected).label_offset(LABEL_DROP)
}

/// Open `workspace`, putting down anything the tools it does not carry had
/// picked up.
///
/// The drawing tools leave the window with production - see
/// [`Workspace::has_production_tools`] - so a tool still armed after the switch
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
    // The finish menu belongs to the stroke, which stays behind on its page.
    editor.poly_finish_dialog = false;
    editor.poly_finish_dialog_px = None;
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
        _ => workspace.has_production_tools() || editor.is_planning_cut_step(),
    };
    if !survives {
        commands.push(UiCommand::SetActiveTool(ActiveTool::None));
    }
}

/// One workspace tab, taking its click, and the tab to paint.
///
/// A tab for a workspace that has nothing behind it yet is drawn as bare text
/// on the bar rather than left off it, so the shape of the application is
/// visible before all of it is built.
fn draw_workspace_tab(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>, workspace: Workspace, id: egui::Id, rect: egui::Rect) -> PageTab {
    let enabled = workspace.implemented();
    let selected = editor.active_workspace == workspace;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(workspace.label(), font, egui::Color32::PLACEHOLDER);
    // The weak text colour is the whole of what a disabled tab says about
    // itself; the sense is what stops it being clicked.
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let response = ui.interact(rect, id, sense);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, enabled, selected, workspace.label()));
    if response.clicked() {
        select_workspace(editor, commands, workspace);
    }
    PageTab {
        rect,
        galley,
        hovered: response.hovered(),
        enabled,
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
            let missing = entry.is_missing();
            let row = ContextMenuAction::new(entry.name.as_str()).enabled(!missing).strikethrough(missing).show(ui);
            // A row is a file stem, so two remembered projects can read alike;
            // the full path tells them apart.
            #[cfg(not(target_arch = "wasm32"))]
            let row = if missing {
                row.on_disabled_hover_text(tr!("menu-file-recent-project-missing", path = entry.path.display().to_string()))
            } else {
                row.on_hover_text(entry.path.display().to_string())
            };
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
    SetAxis(Axis),
    CreateTriangulation,
}

/// The Design menu's entries in menu order; [`design_groups`] cuts it up.
pub(crate) const DESIGN_ROWS: [DesignRow; 6] = [
    DesignRow::InsertAtIntersection,
    DesignRow::InsertAtElevation,
    DesignRow::SetAxis(Axis::X),
    DesignRow::SetAxis(Axis::Y),
    DesignRow::SetAxis(Axis::Z),
    DesignRow::CreateTriangulation,
];

/// The Design menu's runs of [`DESIGN_ROWS`], parted by separators: a submenu
/// title, or `None` for rows standing on the menu itself.
pub(crate) fn design_groups() -> [(Option<String>, std::ops::Range<usize>); 3] {
    [
        (Some(tr!("ws-menubar-design-insert-point")), 0..2),
        (Some(tr!("ws-menubar-design-move-to")), 2..5),
        (None, 5..6),
    ]
}

impl DesignRow {
    pub(crate) fn label(self) -> String {
        match self {
            Self::InsertAtIntersection => tr!("ws-menubar-design-insert-point-at-intersection"),
            Self::InsertAtElevation => tr!("ws-menubar-design-insert-point-at-elevation"),
            Self::SetAxis(axis) => tr!("common-set") + &format!(" {}...", axis.label()),
            Self::CreateTriangulation => tr!("ws-menubar-design-create-triangulation"),
        }
    }

    pub(crate) fn command(self) -> UiCommand {
        match self {
            Self::InsertAtIntersection => UiCommand::InsertPointsAtIntersections,
            Self::InsertAtElevation => UiCommand::OpenInsertPointAtElevationDialog,
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
        let submenu = design_groups().into_iter().find(|(_, run)| run.contains(&index)).and_then(|(title, _)| title);
        let mut parts = vec![tr!("ws-menubar-design")];
        parts.extend(submenu);
        parts.push(self.label());
        parts.join(" / ")
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
                context_menu_separator(ui);
                // Select first, then act: placed on the holes selected at open.
                let can_build_points = editor.selection_counts.reference_holes > 0;
                if ContextMenuAction::new(tr!("common-reference-points")).enabled(can_build_points).show(ui).clicked() {
                    commands.push(UiCommand::OpenReferencePoints);
                    ui.close();
                }
                // Select first, then act: the surface is built from the points
                // that are selected when it opens, not from a layer picked
                // inside the dialog.
                let can_build_surface = editor.selection_counts.surface_points >= crate::app::commands::triangulation::reference_surface::MINIMUM_POINTS;
                if ContextMenuAction::new(tr!("common-build-surface-ellipsis")).enabled(can_build_surface).show(ui).clicked() {
                    commands.push(UiCommand::OpenReferenceSurface);
                    ui.close();
                }
            });
            return;
        }

        MenuBarMenu::new(&tr!("ws-menubar-design")).show(ui, |ui| {
            let draw_row = |ui: &mut egui::Ui, row: DesignRow, commands: &mut Vec<UiCommand>| {
                if ContextMenuAction::new(row.label()).enabled(row.enabled(editor)).show(ui).clicked() {
                    commands.push(row.command());
                    commands.push(UiCommand::NoteRecentFunction(row.key()));
                    ui.close();
                }
            };
            for (index, (title, run)) in design_groups().into_iter().enumerate() {
                if index > 0 {
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
        });

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
