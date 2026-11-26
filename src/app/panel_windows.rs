//! Panels popped out of the main window into windows of their own: the
//! console and the section view's preview
//! ([`PanelKind`]), on the desktop.
//!
//! Each window is opened and closed to follow
//! [`crate::ui::state::EditorState::wants_panel_window`]. It is frameless like
//! the main window (see [`super::window_frame`]) and its bar is drawn with the
//! same title bar pieces (`ui/elements/panel_window.rs`), so what that bar
//! asks of a window (a move, a resize, Close) is taken here and done to this
//! window rather than sent on to the main one; Close docks the panel again.
//! Everything else its UI asks for is an ordinary [`UiCommand`], handled as
//! if the main window had sent it.
//!
//! A window is drawn again when what it shows changes in the main window: the
//! project, the selection, the console's log, the theme, or (for the section
//! preview) its picture.
//!
//! The browser has one window, so there nothing pops out and none of this
//! runs.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use web_time::Instant;
#[cfg(target_os = "linux")]
use winit::platform::wayland::WindowAttributesExtWayland;
#[cfg(not(target_arch = "wasm32"))]
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::app::App;
#[cfg(not(target_arch = "wasm32"))]
use crate::ui::state::{PanelKind, UiCommand, WindowAction};

/// The size a panel's window opens at, in logical pixels.
#[cfg(not(target_arch = "wasm32"))]
fn open_size(kind: PanelKind) -> winit::dpi::LogicalSize<f64> {
    match kind {
        PanelKind::Console => winit::dpi::LogicalSize::new(900.0, 360.0),
        PanelKind::SectionPreview => winit::dpi::LogicalSize::new(640.0, 640.0),
    }
}

/// The smallest a panel's window can be made, likewise.
#[cfg(not(target_arch = "wasm32"))]
const MIN_SIZE: winit::dpi::LogicalSize<f64> = winit::dpi::LogicalSize::new(260.0, 200.0);

/// What, in the main window, a popped-out panel shows: when it changes, the
/// panel's window is drawn again.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PanelViewKey {
    theme: (bool, u64),
    project: u64,
    console: u64,
}

/// One popped-out panel's window state, kept by `App`.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub(super) struct PanelWindowState {
    /// When its UI asked to be drawn again with nothing else asking (a
    /// caret's blink, a hint's delay).
    repaint_deadline: Option<Instant>,
    /// Its window's state is to be read again for its title bar.
    frame_stale: bool,
}

#[cfg(not(target_arch = "wasm32"))]
fn index(kind: PanelKind) -> usize {
    PanelKind::ALL.iter().position(|listed| *listed == kind).unwrap_or_default()
}

impl App<'_> {
    /// Open the windows the panels want and close the rest, after the main
    /// window's frame, and draw again those whose view of the main window
    /// changed.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn sync_panel_windows(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        for kind in PanelKind::ALL {
            let Some(graphics) = self.graphics.as_mut() else {
                return;
            };
            let exists = graphics.panel_window(kind).is_some();
            let wanted = self.editor.wants_panel_window(kind);
            if exists && !wanted {
                graphics.close_panel_window(kind);
                self.panel_window_states[index(kind)] = PanelWindowState::default();
                if kind == PanelKind::SectionPreview {
                    self.editor.slice_preview_window_texture = None;
                }
            } else if wanted && !exists {
                self.open_panel_window(kind, event_loop);
            }
        }
        let key = self.panel_view_key();
        let preview_rendered = self.graphics.as_mut().is_some_and(|graphics| graphics.take_section_preview_rendered());
        if preview_rendered {
            self.request_panel_redraw(PanelKind::SectionPreview);
        }
        if key != self.panel_view_key {
            self.panel_view_key = key;
            for kind in PanelKind::ALL {
                self.request_panel_redraw(kind);
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn sync_panel_windows(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    #[cfg(not(target_arch = "wasm32"))]
    fn open_panel_window(&mut self, kind: PanelKind, event_loop: &winit::event_loop::ActiveEventLoop) {
        let attributes = super::window_frame::frameless(
            Window::default_attributes()
                .with_title(format!("{}: {}", crate::APP_NAME, kind.title()))
                .with_window_icon(super::window_icon())
                .with_min_inner_size(MIN_SIZE)
                .with_inner_size(open_size(kind)),
            event_loop,
        );
        #[cfg(target_os = "linux")]
        let attributes = attributes.with_name(crate::APP_ID, crate::APP_ID);
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let result = event_loop
            .create_window(attributes)
            .map(Arc::new)
            .map_err(anyhow::Error::from)
            .and_then(|window| graphics.open_panel_window(kind, window));
        match result {
            Ok(()) => self.panel_window_states[index(kind)].frame_stale = true,
            Err(error) => {
                log::error!("The {kind:?} window could not be opened: {error:#}");
                // Docked again, rather than nowhere.
                if let Err(error) = self.set_panel_popped_out(kind, false) {
                    log::error!("{error:#}");
                }
            }
        }
    }

    /// What the panels show of the main window, to notice it changing.
    #[cfg(not(target_arch = "wasm32"))]
    fn panel_view_key(&self) -> PanelViewKey {
        use std::hash::{DefaultHasher, Hash, Hasher};

        let mut project = DefaultHasher::new();
        self.project_view_key().hash(&mut project);
        self.scene_document.revision().hash(&mut project);
        self.editor.selected_handles.len().hash(&mut project);
        for handle in &self.editor.selected_handles {
            handle.hash(&mut project);
        }
        self.editor.hidden_handles.len().hash(&mut project);
        PanelViewKey {
            theme: (self.editor.dark_mode, self.editor.ui_size_percent.to_bits()),
            project: project.finish(),
            console: crate::logging::console_snapshot().0,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn request_panel_redraw(&self, kind: PanelKind) {
        if let Some(window) = self.graphics.as_ref().and_then(|graphics| graphics.panel_window(kind)) {
            window.request_redraw();
        }
    }

    /// The popped-out panel whose window this is, if it is one.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn panel_window_kind(&self, id: winit::window::WindowId) -> Option<PanelKind> {
        self.graphics.as_ref().and_then(|graphics| graphics.panel_window_kind(id))
    }

    /// An event for `kind`'s window.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn handle_panel_window_event(&mut self, kind: PanelKind, event: WindowEvent) {
        match &event {
            WindowEvent::CloseRequested => {
                self.dock_panel(kind);
                return;
            }
            WindowEvent::RedrawRequested => {
                self.render_panel_window(kind);
                return;
            }
            WindowEvent::Resized(size) => {
                if let Some(graphics) = self.graphics.as_mut() {
                    graphics.resize_panel_window(kind, *size);
                }
                self.panel_window_states[index(kind)].frame_stale = true;
            }
            WindowEvent::Focused(focused) => {
                self.panel_window_states[index(kind)].frame_stale = true;
                self.request_panel_redraw(kind);
                if !focused
                    && kind == PanelKind::SectionPreview
                    && let Some(graphics) = self.graphics.as_mut()
                {
                    graphics.release_slice_keys();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.panel_window_states[index(kind)].frame_stale = true;
                self.request_panel_redraw(kind);
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event: key, .. } if kind == PanelKind::SectionPreview && self.section_preview_key(key) => return,
            _ => {}
        }
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.panel_window_event(kind, &event);
        }
    }

    /// The section preview's keys, in its own window: W/S/Q/E move the slice
    /// as in the main window, F or Home frames it again, and Escape docks it.
    /// Whether the key was one of them.
    #[cfg(not(target_arch = "wasm32"))]
    fn section_preview_key(&mut self, key: &KeyEvent) -> bool {
        let PhysicalKey::Code(code) = key.physical_key else {
            return false;
        };
        let pressed = key.state == ElementState::Pressed;
        match code {
            KeyCode::KeyF | KeyCode::Home if pressed => {
                self.editor.slice_preview_navigation.reset();
                self.redraw_requested = true;
                true
            }
            KeyCode::Escape if pressed => {
                self.dock_panel(PanelKind::SectionPreview);
                true
            }
            KeyCode::KeyW | KeyCode::KeyS | KeyCode::KeyQ | KeyCode::KeyE if !self.modifiers.control_key() && !(cfg!(target_os = "macos") && self.modifiers.super_key()) => {
                if self.slice_key(code, key.state) {
                    self.redraw_requested = true;
                }
                true
            }
            _ => false,
        }
    }

    /// Wake for the panels' windows when their UIs asked to be drawn again at
    /// a time, and say when to wake next.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn poll_panel_repaints(&mut self, now: Instant) -> Option<Instant> {
        let mut next: Option<Instant> = None;
        for kind in PanelKind::ALL {
            let state = &mut self.panel_window_states[index(kind)];
            match state.repaint_deadline {
                Some(deadline) if deadline <= now => {
                    state.repaint_deadline = None;
                    self.request_panel_redraw(kind);
                }
                Some(deadline) => next = Some(next.map_or(deadline, |next| next.min(deadline))),
                None => {}
            }
        }
        next
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn dock_panel(&mut self, kind: PanelKind) {
        if let Err(error) = self.set_panel_popped_out(kind, false) {
            log::error!("{error:#}");
        }
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.close_panel_window(kind);
        }
        self.panel_window_states[index(kind)] = PanelWindowState::default();
        self.redraw_requested = true;
    }

    /// Mirror the window's state into the editor for its title bar, as
    /// `sync_window_frame` does the main window's: only after an event that
    /// can have changed it.
    #[cfg(not(target_arch = "wasm32"))]
    fn sync_panel_frame(&mut self, kind: PanelKind) {
        let Some(window) = self.graphics.as_ref().and_then(|graphics| graphics.panel_window(kind)) else {
            return;
        };
        let frame = self.editor.panel_frames.get_mut(kind);
        if std::mem::take(&mut self.panel_window_states[index(kind)].frame_stale) {
            frame.maximized = window.is_maximized();
            frame.fullscreen = window.fullscreen().is_some();
            frame.focused = window.has_focus();
        }
        let fills_screen = frame.maximized || frame.fullscreen;
        frame.border = super::window_frame::DRAWS_OWN_BORDER && !fills_screen;
        // Whole points, so the border the UI strokes and the corners cut
        // follow the same arc: see `sync_window_frame`.
        let scale_factor = window.scale_factor() as f32;
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let pixels_per_point = graphics.panel_pixels_per_point(kind);
        let radius = if graphics.panel_cuts_corners(kind) && !fills_screen {
            (super::window_frame::WINDOW_CORNER_RADIUS * scale_factor / pixels_per_point).round()
        } else {
            0.0
        };
        self.editor.panel_frames.get_mut(kind).corner_radius = radius as u8;
        graphics.set_panel_corner_radius(kind, radius * pixels_per_point);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn render_panel_window(&mut self, kind: PanelKind) {
        self.sync_panel_frame(kind);
        let console = crate::logging::console_snapshot();
        let navigation = (self.editor.slice_preview_navigation, self.editor.slice_preview_size_px);
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let context = crate::ui::elements::panel_window::PanelWindowContext { console: &console };
        let result = graphics.render_panel_window(kind, &mut self.editor, |ui, editor, commands| {
            crate::ui::elements::panel_window::draw_panel_window(ui, kind, editor, &context, commands);
        });
        let frame = match result {
            Ok(frame) => frame,
            Err(error) => {
                log::debug!("The {kind:?} window skipped a frame: {error:?}");
                self.request_panel_redraw(kind);
                return;
            }
        };
        self.panel_window_states[index(kind)].repaint_deadline = match frame.repaint_after {
            Some(delay) if delay.is_zero() => {
                self.request_panel_redraw(kind);
                None
            }
            delay => delay.and_then(|delay| Instant::now().checked_add(delay)),
        };
        // The preview is drawn by the main window's frame: moving or
        // resizing it there needs one.
        if (self.editor.slice_preview_navigation, self.editor.slice_preview_size_px) != navigation {
            self.redraw_requested = true;
        }
        self.place_panel_frame(kind);
        let mut forwarded = Vec::new();
        for command in frame.commands {
            match command {
                UiCommand::Window(action) => self.apply_panel_window_action(kind, action),
                // The bar's Close button docks the panel, not quits Incline.
                UiCommand::RequestExit => self.dock_panel(kind),
                command => forwarded.push(command),
            }
        }
        if !forwarded.is_empty() {
            self.handle_ui_commands(forwarded);
            self.redraw_requested = true;
            self.request_panel_redraw(kind);
        }
    }

    /// After a frame, hand what the bar drew on to the platform: on macOS,
    /// the row the system's own buttons are centred on.
    #[cfg(not(target_arch = "wasm32"))]
    fn place_panel_frame(&mut self, kind: PanelKind) {
        #[cfg(target_os = "macos")]
        if let Some(graphics) = self.graphics.as_ref()
            && let Some(window) = graphics.panel_window(kind)
        {
            let points = f64::from(graphics.panel_pixels_per_point(kind)) / window.scale_factor();
            let frame = self.editor.panel_frames.get_mut(kind);
            let end = crate::mac::place_window_buttons(window, f64::from(frame.buttons_centre_y) * points, super::window_frame::MAC_BUTTONS_LEFT);
            frame.leading_inset = match end {
                Some(end) if !frame.fullscreen => (end / points) as f32,
                _ => 0.0,
            };
        }
        let _ = kind;
    }

    /// Do what a panel window's title bar asked of it, as
    /// `apply_window_action` does for the main window.
    #[cfg(not(target_arch = "wasm32"))]
    fn apply_panel_window_action(&mut self, kind: PanelKind, action: WindowAction) {
        let Some(graphics) = self.graphics.as_ref() else {
            return;
        };
        let Some(window) = graphics.panel_window(kind).cloned() else {
            return;
        };
        let toggle_maximized = || window.set_maximized(!window.is_maximized());
        let mut release_pointer = false;
        match action {
            WindowAction::Drag | WindowAction::Resize(_) => {
                let started = match action {
                    WindowAction::Resize(direction) => window.drag_resize_window(direction),
                    _ => window.drag_window(),
                };
                match started {
                    Ok(()) => release_pointer = true,
                    Err(error) => log::warn!("The {kind:?} window could not be moved or resized: {error}"),
                }
            }
            WindowAction::Minimize => window.set_minimized(true),
            WindowAction::ToggleMaximize => toggle_maximized(),
            WindowAction::TitleDoubleClick => {
                #[cfg(target_os = "macos")]
                match crate::mac::title_double_click() {
                    crate::mac::TitleDoubleClick::Zoom => toggle_maximized(),
                    crate::mac::TitleDoubleClick::Minimize => window.set_minimized(true),
                    crate::mac::TitleDoubleClick::Nothing => {}
                }
                #[cfg(not(target_os = "macos"))]
                toggle_maximized();
            }
            WindowAction::ShowMenu(position) => {
                let pixels_per_point = graphics.panel_pixels_per_point(kind);
                window.show_window_menu(winit::dpi::PhysicalPosition::new(position.x * pixels_per_point, position.y * pixels_per_point));
            }
        }
        self.panel_window_states[index(kind)].frame_stale = true;
        if release_pointer && let Some(graphics) = self.graphics.as_mut() {
            graphics.release_panel_pointer(kind);
        }
    }
}
