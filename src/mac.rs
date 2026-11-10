//! macOS system menu bar, and the window buttons over Incline's title bar.
//!
//! Incline Design's other targets draw their menu inside the egui window. On macOS,
//! AppKit owns the equivalent commands so they appear in the global menu bar
//! and participate in standard Command-key handling.
//!
//! The title bar is Incline's own on every platform (`ui/elements/title_bar.rs`),
//! but macOS keeps its red, yellow and green buttons: [`place_window_buttons`]
//! moves them onto the bar's row.

use std::{path::PathBuf, sync::Mutex};

use objc2::{
    AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, Sel},
    sel,
};
use objc2_app_kit::{NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuItem, NSStrikethroughStyleAttributeName, NSUnderlineStyle};
use objc2_foundation::{NSAttributedString, NSDictionary, NSNumber, NSObject, NSObjectProtocol, NSString, NSUserDefaults};

use crate::{
    i18n::tr,
    ui::{
        elements::main_menu::{MenuRow, workspace_menus},
        state::{EditorState, UiProjectView, ViewToggle, Workspace},
    },
};

static PENDING_ACTIONS: Mutex<Vec<MacMenuAction>> = Mutex::new(Vec::new());
static LAST_MENU_STATE: Mutex<Option<MenuState>> = Mutex::new(None);

/// Not `Copy`: the Open Recent rows are part of what the menu is showing, so
/// they belong in the state the sync pass compares against.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MenuState {
    can_save: bool,
    has_project: bool,
    /// Whether each workspace menu row can run, in [`MenuRow::all`] order.
    menu_rows: Vec<bool>,
    /// Whether the active project is a file that can be shown in Finder.
    has_project_file: bool,
    /// The workspace decides which discipline menus are visible and active.
    active_workspace: Workspace,
    /// The View menu's switches, in the order [`VIEW_TOGGLES`] lists them.
    view_toggles: [bool; VIEW_TOGGLES.len()],
    /// The File > Open Recent rows.
    recent: Vec<RecentRow>,
}

/// One File > Open Recent row: its name, the project it opens, and whether
/// that project's file was found at the last look.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RecentRow {
    name: String,
    path: PathBuf,
    missing: bool,
}

/// Actions emitted by AppKit and consumed by the winit application loop.
///
/// The variants without an index are each one fixed menu item;
/// [`MacMenuAction::OpenRecent`] names a row of the File > Open Recent submenu, which is rebuilt
/// from the recent-project list rather than written out here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MacMenuAction {
    SaveProject,
    SaveProjectAs,
    NewProject,
    OpenProject,
    OpenImport,
    OpenExport,
    ExportViewportImage,
    RequestExit,
    OpenPreferences,
    OpenAbout,
    ShowProjectInFileManager,
    /// One row of File > Open Recent, by its index in the recent list the menu
    /// was last built from.
    OpenRecent(usize),
    /// One View menu switch, by its position in [`VIEW_TOGGLES`].
    ToggleView(usize),
    /// One workspace menu row, by its position in [`MenuRow::all`].
    MenuRow(usize),
}

/// The View menu's rows, in the order they are drawn. The egui menu bar draws
/// the same two - see [`crate::ui::elements::main_menu`].
pub(crate) const VIEW_TOGGLES: [ViewToggle; 2] = [ViewToggle::Console, ViewToggle::DarkMode];

/// A workspace's root menus all carry the tag [`workspace_menu_tag`] gives
/// it, so [`set_workspace_menus`] finds them without matching on a
/// translated title. Every other root item is left at 0 or above.
const WORKSPACE_MENU_TAG_BASE: isize = -1;

fn workspace_menu_tag(workspace: Workspace) -> isize {
    let index = Workspace::ALL.iter().position(|listed| *listed == workspace).unwrap_or_default();
    WORKSPACE_MENU_TAG_BASE - index as isize
}

/// Tags at or above this carry a recent-project index rather than naming a
/// fixed action, leaving room for the fixed list to grow.
const RECENT_TAG_BASE: isize = 1000;
/// Tags at or above this, and below [`RECENT_SUBMENU_TAG`], carry a View menu
/// switch by its index in [`VIEW_TOGGLES`].
const VIEW_TAG_BASE: isize = 500;
/// Tags at or above this, and below [`MENU_ROW_TAG_END`], carry a workspace
/// menu row by its position in [`MenuRow::all`].
const MENU_ROW_TAG_BASE: isize = 300;
const MENU_ROW_TAG_END: isize = 400;
/// Tag of the Open Recent item itself. It opens a submenu rather than
/// performing anything, so [`MacMenuAction::from_tag`] rejects it: no fixed
/// action sits this far up the range.
const RECENT_SUBMENU_TAG: isize = RECENT_TAG_BASE - 1;

impl MacMenuAction {
    /// Every action that is one fixed menu item, in tag order.
    ///
    /// A variant's position here is its `NSMenuItem` tag, offset by one so
    /// that 0 - the tag an item that was never given one carries - names
    /// nothing.
    const FIXED: &'static [Self] = &[
        Self::SaveProject,
        Self::SaveProjectAs,
        Self::NewProject,
        Self::OpenProject,
        Self::OpenImport,
        Self::OpenExport,
        Self::ExportViewportImage,
        Self::RequestExit,
        Self::OpenPreferences,
        Self::OpenAbout,
        Self::ShowProjectInFileManager,
    ];

    /// The `NSMenuItem` tag this action is carried by.
    fn tag(self) -> isize {
        match self {
            Self::OpenRecent(index) => RECENT_TAG_BASE + index as isize,
            Self::ToggleView(index) => VIEW_TAG_BASE + index as isize,
            Self::MenuRow(index) => MENU_ROW_TAG_BASE + index as isize,
            action => Self::FIXED.iter().position(|fixed| *fixed == action).map_or(0, |index| index as isize + 1),
        }
    }

    fn from_tag(tag: isize) -> Option<Self> {
        if tag >= RECENT_TAG_BASE {
            return Some(Self::OpenRecent(usize::try_from(tag - RECENT_TAG_BASE).ok()?));
        }
        if (VIEW_TAG_BASE..RECENT_SUBMENU_TAG).contains(&tag) {
            let index = usize::try_from(tag - VIEW_TAG_BASE).ok()?;
            return (index < VIEW_TOGGLES.len()).then_some(Self::ToggleView(index));
        }
        if (MENU_ROW_TAG_BASE..MENU_ROW_TAG_END).contains(&tag) {
            let index = usize::try_from(tag - MENU_ROW_TAG_BASE).ok()?;
            return (index < MenuRow::all().count()).then_some(Self::MenuRow(index));
        }
        Self::FIXED.get(usize::try_from(tag - 1).ok()?).copied()
    }
}

define_class!(
    // SAFETY:
    // - NSObject has no subclassing requirements.
    // - MenuTarget has no Drop implementation.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    struct MenuTarget;

    impl MenuTarget {
        // SAFETY: This signature matches the AppKit target/action selector.
        #[unsafe(method(performInclineDesignMenuAction:))]
        fn perform_action(&self, sender: &NSMenuItem) {
            if let Some(action) = MacMenuAction::from_tag(sender.tag()) {
                action_queue().push(action);
            }
        }
    }

    // SAFETY: NSObjectProtocol has no additional safety requirements.
    unsafe impl NSObjectProtocol for MenuTarget {}
);

impl MenuTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's `init` has the declared signature.
        unsafe { msg_send![super(this), init] }
    }
}

fn action_queue() -> std::sync::MutexGuard<'static, Vec<MacMenuAction>> {
    PENDING_ACTIONS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn last_menu_state() -> std::sync::MutexGuard<'static, Option<MenuState>> {
    LAST_MENU_STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Take every menu action queued since the last application-loop iteration.
pub(crate) fn drain_actions() -> Vec<MacMenuAction> {
    std::mem::take(&mut *action_queue())
}

fn menu(title: &str, mtm: MainThreadMarker) -> Retained<NSMenu> {
    NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title))
}

fn menu_item(title: &str, selector: Option<Sel>, key: &str, mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    // SAFETY: Each selector supplied by this module is implemented either by
    // MenuTarget or by AppKit's standard responder chain.
    unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(mtm), &NSString::from_str(title), selector, &NSString::from_str(key)) }
}

fn add_separator(menu: &NSMenu, mtm: MainThreadMarker) {
    menu.addItem(&NSMenuItem::separatorItem(mtm));
}

fn add_submenu(root: &NSMenu, title: &str, submenu: &NSMenu, mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let item = menu_item(title, None, "", mtm);
    item.setSubmenu(Some(submenu));
    root.addItem(&item);
    item
}

/// Add a placeholder top-level menu with no actions wired up yet.
///
/// Unused while every menu has content - kept as the counterpart to
/// `MenuBarMenu::enabled` on the egui side.
#[allow(dead_code)]
fn add_disabled_submenu(root: &NSMenu, title: &str, mtm: MainThreadMarker) {
    let empty_menu = menu(title, mtm);
    let item = add_submenu(root, title, &empty_menu, mtm);
    item.setEnabled(false);
}

fn add_action(menu: &NSMenu, title: &str, key: &str, action: MacMenuAction, target: &MenuTarget, mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let item = menu_item(title, Some(sel!(performInclineDesignMenuAction:)), key, mtm);
    item.setTag(action.tag());
    // NSMenuItem's target is weak. representedObject retains the target for
    // exactly as long as the item remains installed in the application menu.
    unsafe {
        item.setTarget(Some(target as &AnyObject));
        item.setRepresentedObject(Some(target as &AnyObject));
    }
    menu.addItem(&item);
    item
}

/// Replace winit's fallback macOS menu with Incline Design's native menu bar.
pub(crate) fn install_menu_bar() {
    let Some(mtm) = MainThreadMarker::new() else {
        log::error!("{}", crate::i18n::tr!("mac-cannot-install-macos-menu-bar"));
        return;
    };

    let app = NSApplication::sharedApplication(mtm);
    let target = MenuTarget::new(mtm);
    let root = menu(crate::APP_NAME, mtm);
    root.setAutoenablesItems(false);
    *last_menu_state() = None;

    let application_menu = menu(crate::APP_NAME, mtm);
    application_menu.setAutoenablesItems(false);
    add_action(
        &application_menu,
        &tr!("menu-file-about", app = crate::APP_NAME),
        "",
        MacMenuAction::OpenAbout,
        &target,
        mtm,
    );
    add_separator(&application_menu, mtm);
    add_action(
        &application_menu,
        &tr!("mac-quit-app", app = crate::APP_NAME.to_string()),
        "q",
        MacMenuAction::RequestExit,
        &target,
        mtm,
    );
    add_submenu(&root, crate::APP_NAME, &application_menu, mtm);

    let file_menu = menu(&tr!("menu-file"), mtm);
    file_menu.setAutoenablesItems(false);
    add_action(&file_menu, &tr!("menu-file-new-project"), "n", MacMenuAction::NewProject, &target, mtm);
    add_action(&file_menu, &tr!("menu-file-open-project"), "o", MacMenuAction::OpenProject, &target, mtm);
    // Empty until the first sync pass fills it - see `rebuild_recent_menu`.
    let recent_menu = menu(&tr!("menu-file-open-recent"), mtm);
    recent_menu.setAutoenablesItems(false);
    let recent_item = add_submenu(&file_menu, &tr!("menu-file-open-recent"), &recent_menu, mtm);
    recent_item.setTag(RECENT_SUBMENU_TAG);
    recent_item.setEnabled(false);
    add_action(&file_menu, &tr!("common-reveal-finder"), "", MacMenuAction::ShowProjectInFileManager, &target, mtm);
    add_separator(&file_menu, mtm);
    add_action(&file_menu, &tr!("menu-file-save-project"), "s", MacMenuAction::SaveProject, &target, mtm);
    add_action(&file_menu, &tr!("menu-file-save-project-as"), "S", MacMenuAction::SaveProjectAs, &target, mtm);
    add_separator(&file_menu, mtm);
    add_action(&file_menu, &tr!("menu-file-import"), "", MacMenuAction::OpenImport, &target, mtm);
    add_action(&file_menu, &tr!("menu-file-export"), "", MacMenuAction::OpenExport, &target, mtm);
    add_action(&file_menu, &tr!("menu-file-export-viewport-image"), "", MacMenuAction::ExportViewportImage, &target, mtm);
    add_separator(&file_menu, mtm);
    add_action(&file_menu, &tr!("preferences-title"), ",", MacMenuAction::OpenPreferences, &target, mtm);
    add_action(&file_menu, &tr!("menu-file-about", app = crate::APP_NAME), "", MacMenuAction::OpenAbout, &target, mtm);
    add_submenu(&root, &tr!("menu-file"), &file_menu, mtm);

    // Switches onto the same settings the Interface preferences tab holds;
    // their checkmarks are kept current by `sync_menu_state`.
    let view_menu = menu(&tr!("menu-view"), mtm);
    view_menu.setAutoenablesItems(false);
    for (index, toggle) in VIEW_TOGGLES.iter().enumerate() {
        add_action(&view_menu, &toggle.label(), "", MacMenuAction::ToggleView(index), &target, mtm);
    }
    add_submenu(&root, &tr!("menu-view"), &view_menu, mtm);

    // Every workspace's menus, from the table the egui bar draws;
    // `set_workspace_menus` shows the active workspace its own.
    debug_assert!(MenuRow::all().count() as isize <= MENU_ROW_TAG_END - MENU_ROW_TAG_BASE);
    let mut row_index = 0;
    for workspace in Workspace::ALL {
        for workspace_menu in workspace_menus(workspace) {
            let title = workspace_menu.title.label();
            let root_menu = menu(&title, mtm);
            root_menu.setAutoenablesItems(false);
            for (index, group) in workspace_menu.groups.iter().enumerate() {
                if index > 0 {
                    add_separator(&root_menu, mtm);
                }
                let submenu = group.submenu.map(|title| menu(&title.label(), mtm));
                if let Some(submenu) = &submenu {
                    submenu.setAutoenablesItems(false);
                }
                for row in group.rows {
                    add_action(submenu.as_deref().unwrap_or(&*root_menu), &row.label(), "", MacMenuAction::MenuRow(row_index), &target, mtm);
                    row_index += 1;
                }
                if let (Some(title), Some(submenu)) = (group.submenu, &submenu) {
                    add_submenu(&root_menu, &title.label(), submenu, mtm);
                }
            }
            add_submenu(&root, &title, &root_menu, mtm).setTag(workspace_menu_tag(workspace));
        }
    }

    app.setMainMenu(Some(&root));
    NSMenu::setMenuBarVisible(true, mtm);
}

fn find_item(menu: &NSMenu, tag: isize) -> Option<Retained<NSMenuItem>> {
    for item in menu.itemArray().iter() {
        if item.tag() == tag {
            return Some(item);
        }
        if let Some(submenu) = item.submenu()
            && let Some(found) = find_item(&submenu, tag)
        {
            return Some(found);
        }
    }
    None
}

fn set_enabled(root: &NSMenu, action: MacMenuAction, enabled: bool) {
    if let Some(item) = find_item(root, action.tag()) {
        item.setEnabled(enabled);
    }
}

/// Match the native discipline menus to the active workspace.
///
/// A workspace shows the menus it has and nothing else: a discipline whose
/// menu belongs elsewhere leaves a shorter bar rather than an empty dropdown,
/// so Drill & Blast and Planning show none at all. The egui bar does the same -
/// see `main_menu::draw_workspace_menus`.
fn set_workspace_menus(root: &NSMenu, workspace: Workspace) {
    let shown = workspace_menu_tag(workspace);
    for item in root.itemArray().iter() {
        if item.tag() <= WORKSPACE_MENU_TAG_BASE {
            item.setHidden(item.tag() != shown);
        }
    }
}

fn set_checked(root: &NSMenu, action: MacMenuAction, checked: bool) {
    if let Some(item) = find_item(root, action.tag()) {
        item.setState(if checked { NSControlStateValueOn } else { NSControlStateValueOff });
    }
}

/// Refill File > Open Recent from the recent-project list.
///
/// The rows are rebuilt rather than enabled and disabled in place: what the
/// list holds changes with every project opened, and a row's tag is its index
/// in it. Each new item retains its own target, exactly as the fixed ones
/// installed at startup do.
///
/// A project whose file was not found is disabled and struck through, as the
/// egui menu shows it.
fn rebuild_recent_menu(root: &NSMenu, recent: &[RecentRow], mtm: MainThreadMarker) {
    let Some(item) = find_item(root, RECENT_SUBMENU_TAG) else {
        return;
    };
    let Some(submenu) = item.submenu() else {
        return;
    };
    submenu.removeAllItems();
    let target = MenuTarget::new(mtm);
    for (index, row) in recent.iter().enumerate() {
        let item = add_action(&submenu, &row.name, "", MacMenuAction::OpenRecent(index), &target, mtm);
        if row.missing {
            item.setEnabled(false);
            item.setAttributedTitle(Some(&struck_through(&row.name)));
            item.setToolTip(Some(&NSString::from_str(&tr!("menu-file-recent-project-missing", path = row.path.display().to_string()))));
        }
    }
    // Greyed out with nothing to offer rather than hidden, so the File menu
    // keeps its shape.
    item.setEnabled(!recent.is_empty());
}

/// `text` with a single line struck through it.
fn struck_through(text: &str) -> Retained<NSAttributedString> {
    let style = NSNumber::new_isize(NSUnderlineStyle::Single.0);
    // SAFETY: the strikethrough attribute takes an NSNumber holding an
    // NSUnderlineStyle.
    let attributes = NSDictionary::<objc2_foundation::NSAttributedStringKey, AnyObject>::from_slices(&[unsafe { NSStrikethroughStyleAttributeName }], &[&*style as &AnyObject]);
    unsafe { NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &NSString::from_str(text), Some(&attributes)) }
}

/// The project behind one File > Open Recent row, as the menu last showed it.
///
/// The click carries the row's index alone, so it is resolved against the
/// same list the row was built from rather than against the current one.
pub(crate) fn recent_project_path(index: usize) -> Option<PathBuf> {
    let state = last_menu_state();
    state.as_ref()?.recent.get(index).map(|row| row.path.clone())
}

/// Keep native checkmarks and availability aligned with the current editor.
pub(crate) fn sync_menu_state(editor: &EditorState, project: &UiProjectView) {
    let state = MenuState {
        can_save: project.projects.iter().any(crate::ui::state::UiProjectEntry::needs_save),
        has_project: project.projects.iter().any(|entry| entry.is_active),
        menu_rows: MenuRow::all().map(|row| row.enabled(project)).collect(),
        has_project_file: project.active_path.is_some(),
        active_workspace: editor.active_workspace,
        view_toggles: VIEW_TOGGLES.map(|toggle| toggle.get(editor)),
        recent: project
            .recent_projects()
            .map(|entry| RecentRow {
                name: entry.name.clone(),
                path: entry.path.clone(),
                missing: entry.missing,
            })
            .collect(),
    };
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(root) = NSApplication::sharedApplication(mtm).mainMenu() else {
        return;
    };
    let mut previous = last_menu_state();
    if previous.as_ref() == Some(&state) {
        return;
    }
    *previous = Some(state.clone());
    drop(previous);

    rebuild_recent_menu(&root, &state.recent, mtm);
    set_workspace_menus(&root, state.active_workspace);

    set_enabled(&root, MacMenuAction::SaveProject, state.can_save);
    set_enabled(&root, MacMenuAction::SaveProjectAs, state.has_project);
    // A never-saved project is nowhere to be shown.
    set_enabled(&root, MacMenuAction::ShowProjectInFileManager, state.has_project_file);
    for (index, enabled) in state.menu_rows.iter().enumerate() {
        set_enabled(&root, MacMenuAction::MenuRow(index), *enabled);
    }
    for (index, checked) in state.view_toggles.iter().enumerate() {
        set_checked(&root, MacMenuAction::ToggleView(index), *checked);
    }
}

/// What a double-click on a title bar does, as set in System Settings >
/// Desktop & Dock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TitleDoubleClick {
    Zoom,
    Minimize,
    Nothing,
}

/// The double-click action the user has chosen for title bars.
pub(crate) fn title_double_click() -> TitleDoubleClick {
    let defaults = NSUserDefaults::standardUserDefaults();
    match defaults
        .stringForKey(&NSString::from_str("AppleActionOnDoubleClick"))
        .map(|action| action.to_string())
        .as_deref()
    {
        Some("Minimize") => TitleDoubleClick::Minimize,
        Some("None") => TitleDoubleClick::Nothing,
        // "Maximize" and "Fill" both zoom, as does an unset preference.
        _ => TitleDoubleClick::Zoom,
    }
}

/// Place the window's red, yellow and green buttons over Incline's own title
/// bar: `left` points in from the window's left edge, centred `centre_y`
/// points down from its top. Returns where they end, from the window's left.
///
/// AppKit lays its title bar out again whenever it sees fit - a resize, a
/// change of key window, a new title - so this is called every frame, and
/// only moves what has moved. In fullscreen the buttons belong to the menu
/// bar that slides down, and are left alone.
pub(crate) fn place_window_buttons(window: &winit::window::Window, centre_y: f64, left: f64) -> Option<f64> {
    use objc2_app_kit::{NSView, NSWindowButton};
    use objc2_foundation::{NSPoint, NSRect};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let RawWindowHandle::AppKit(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    // winit hands out its content view, alive as long as the window is.
    let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let ns_window = view.window()?;
    let close = ns_window.standardWindowButton(NSWindowButton::CloseButton)?;
    let minimize = ns_window.standardWindowButton(NSWindowButton::MiniaturizeButton)?;
    let zoom = ns_window.standardWindowButton(NSWindowButton::ZoomButton)?;
    let spacing = minimize.frame().origin.x - close.frame().origin.x;
    let end = left + 2.0 * spacing + zoom.frame().size.width;
    if window.fullscreen().is_some() {
        return Some(end);
    }
    // The buttons sit in the title bar view, inside a container pinned to
    // the window's top; a bar taller than the system's grows the container to
    // keep them in it.
    let titlebar = unsafe { close.superview() }?;
    let container = unsafe { titlebar.superview() }?;
    let height = close.frame().size.height;
    let needed = (centre_y + height / 2.0).ceil();
    let frame = container.frame();
    if frame.size.height < needed {
        let window_height = ns_window.frame().size.height;
        container.setFrame(NSRect::new(
            NSPoint::new(frame.origin.x, window_height - needed),
            objc2_foundation::NSSize::new(frame.size.width, needed),
        ));
    }
    let titlebar_height = titlebar.frame().size.height;
    let y = if titlebar.isFlipped() {
        centre_y - height / 2.0
    } else {
        titlebar_height - centre_y - height / 2.0
    };
    for (index, button) in [&close, &minimize, &zoom].into_iter().enumerate() {
        let origin = NSPoint::new(left + index as f64 * spacing, y.round());
        if button.frame().origin != origin {
            button.setFrameOrigin(origin);
        }
    }
    Some(end)
}
