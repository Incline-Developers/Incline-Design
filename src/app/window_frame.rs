//! The main window's frame, which Incline draws itself.
//!
//! The window is opened without the system's title bar (see [`frameless`]),
//! and the menu bar stands in for it: `ui/elements/title_bar.rs` draws the
//! name and the buttons and turns gestures on the bar into
//! [`WindowAction`]s, which are carried out here. Around each frame the
//! window's state is mirrored into [`crate::ui::state::WindowFrame`] for the
//! bar to draw from, and what the bar drew is handed on to the platform: on
//! Windows the maximise button's rect, for Snap Layouts
//! (`windows_caption.rs`), and on macOS the row the system's own buttons are
//! centred on (`mac::place_window_buttons`).

use winit::window::WindowAttributes;

use crate::{
    app::App,
    ui::state::{UiProjectView, WindowAction},
};

/// How far in from the window's left edge the system's window buttons sit on
/// macOS, in points: as far as they sit down from its top on a bar of the
/// system's own height.
#[cfg(target_os = "macos")]
const MAC_BUTTONS_LEFT: f64 = 9.0;

/// The window's corner radius where it cuts its own corners, in the window's
/// logical pixels: Breeze's, so the window sits among KDE's own. This is the
/// one radius not `GROUP_CORNER_RADIUS`, being the desktop's rather than the
/// layout's.
const WINDOW_CORNER_RADIUS: f32 = 6.0;

/// Whether nothing but Incline frames the window, so it draws its own border.
/// Windows' compositor and macOS frame theirs.
const DRAWS_OWN_BORDER: bool = cfg!(not(any(target_os = "macos", target_os = "windows", target_arch = "wasm32")));

/// The main window's attributes, without the system's title bar.
///
/// macOS keeps its window: the title bar turns transparent and the content
/// runs up under it, so its buttons, resizing and fullscreen stay the
/// system's. Elsewhere the decorations go, and the bar takes their place;
/// Windows keeps its drop shadow and rounded corners. A Wayland window is
/// transparent, to cut its own corners round
/// (`rendering/graphics/window_corners.rs`); X11 keeps them square.
pub(super) fn frameless(attributes: WindowAttributes, event_loop: &winit::event_loop::ActiveEventLoop) -> WindowAttributes {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;
        let _ = event_loop;
        attributes.with_titlebar_transparent(true).with_fullsize_content_view(true).with_title_hidden(true)
    }
    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};
        let _ = event_loop;
        attributes
            .with_decorations(false)
            .with_undecorated_shadow(true)
            .with_corner_preference(CornerPreference::Round)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = event_loop;
        attributes
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_arch = "wasm32")))]
    {
        use winit::platform::wayland::ActiveEventLoopExtWayland;
        attributes.with_decorations(false).with_transparent(event_loop.is_wayland())
    }
}

/// What the system shows for the window - the taskbar, the window switcher -
/// now that the bar shows it the project's name itself.
fn window_title(project: &UiProjectView) -> String {
    project.titled_project().map_or_else(
        || crate::APP_NAME.to_owned(),
        |entry| {
            let name = entry.name.trim_end_matches(".omf");
            format!("{}: {}{}", crate::APP_NAME, name, if entry.dirty { " *" } else { "" })
        },
    )
}

impl App<'_> {
    /// Before a frame: title the window, and mirror its state into the UI.
    ///
    /// Maximised, fullscreen and focus are read again only after an event
    /// that can change them, as a read can be a round trip to the display
    /// server.
    pub(super) fn sync_window_frame(&mut self, project: &UiProjectView) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let title = window_title(project);
        if title != self.window_title {
            window.set_title(&title);
            self.window_title = title;
        }
        let frame = &mut self.editor.window_frame;
        if std::mem::take(&mut self.window_state_stale) {
            frame.maximized = window.is_maximized();
            frame.fullscreen = window.fullscreen().is_some();
            frame.focused = window.has_focus();
        }
        let fills_screen = frame.maximized || frame.fullscreen;
        frame.border = DRAWS_OWN_BORDER && !fills_screen;
        if let Some(graphics) = self.graphics.as_mut() {
            // Whole points, so the border the UI strokes and the corners cut
            // here follow the same arc at any UI size.
            let pixels_per_point = graphics.ui_pixels_per_point();
            let radius = if graphics.cuts_window_corners() && !fills_screen {
                (WINDOW_CORNER_RADIUS * window.scale_factor() as f32 / pixels_per_point).round()
            } else {
                0.0
            };
            frame.corner_radius = radius as u8;
            graphics.set_window_corner_radius(radius * pixels_per_point);
        }
        #[cfg(target_os = "windows")]
        if let Some(button) = &self.maximize_button {
            frame.maximize_hovered = button.hovered();
            frame.maximize_pressed = button.pressed();
        }
    }

    /// After a frame: hand what the bar drew on to the platform.
    pub(super) fn place_window_frame(&mut self) {
        let (Some(window), Some(graphics)) = (self.window.as_ref(), self.graphics.as_ref()) else {
            return;
        };
        let pixels_per_point = graphics.ui_pixels_per_point();
        let frame = &mut self.editor.window_frame;
        #[cfg(target_os = "windows")]
        if let Some(button) = &self.maximize_button {
            button.set_rect(frame.maximize_button.map(|rect| {
                let rect = rect * pixels_per_point;
                [rect.left(), rect.top(), rect.right(), rect.bottom()].map(|edge| edge.round() as i32)
            }));
        }
        #[cfg(target_os = "macos")]
        {
            // AppKit's points per UI point: the UI has its own size setting.
            let points = f64::from(pixels_per_point) / window.scale_factor();
            let end = crate::mac::place_window_buttons(window, f64::from(frame.buttons_centre_y) * points, MAC_BUTTONS_LEFT);
            frame.leading_inset = match end {
                Some(end) if !frame.fullscreen => (end / points) as f32,
                _ => 0.0,
            };
        }
        let _ = (window, pixels_per_point, frame);
    }

    /// Do what the title bar asked of the window.
    ///
    /// A move or resize the system refuses is logged rather than reported:
    /// the window simply stays where it is, and there is nothing to act on.
    pub(crate) fn apply_window_action(&mut self, action: WindowAction) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let toggle_maximized = || window.set_maximized(!window.is_maximized());
        match action {
            WindowAction::Drag | WindowAction::Resize(_) => {
                let started = match action {
                    WindowAction::Resize(direction) => window.drag_resize_window(direction),
                    _ => window.drag_window(),
                };
                match started {
                    Ok(()) => {
                        if let Some(graphics) = self.graphics.as_mut() {
                            graphics.release_ui_pointer();
                        }
                    }
                    Err(error) => log::warn!("The window could not be moved or resized: {error}"),
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
                let pixels_per_point = self.graphics.as_ref().map_or(1.0, |graphics| graphics.ui_pixels_per_point());
                window.show_window_menu(winit::dpi::PhysicalPosition::new(position.x * pixels_per_point, position.y * pixels_per_point));
            }
        }
        self.window_state_stale = true;
    }
}
