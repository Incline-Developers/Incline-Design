//! The UI of a window besides the main one: a popped-out panel's
//! (`app/panel_windows.rs`).
//!
//! A context, winit bridge and renderer of its own, set up as the main
//! window's are ([`super::configure_context`]) and following the same theme
//! and UI size, so the two windows look alike. What it draws is handed in by
//! the caller, and what it asks for comes back as [`UiCommand`]s, as from the
//! main window.

use std::time::Duration;

use egui_wgpu::ScreenDescriptor;
use winit::{event::WindowEvent, window::Window};

use super::{SELECTION_COLOR, UiPaint, configure_context, paint_ui, scaling, state::EditorState, theme_visuals};
use crate::ui::state::UiCommand;

pub(crate) struct SecondaryGui {
    ctx: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    /// The system has taken the held button to move or resize the window.
    /// See [`Self::release_pointer`].
    release_pointer: bool,
}

/// One frame of a secondary window's UI, laid out and waiting to be painted.
pub(crate) struct SecondaryFrame {
    pub(crate) commands: Vec<UiCommand>,
    pub(crate) paint: UiPaint,
    /// When the UI wants drawing again with nothing else asking, if it does.
    pub(crate) repaint_after: Option<Duration>,
}

impl SecondaryGui {
    pub(crate) fn new(window: &Window, device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        configure_context(&ctx);
        let state = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, window, Some(window.scale_factor() as f32), window.theme(), None);
        let renderer = egui_wgpu::Renderer::new(device, surface_format, egui_wgpu::RendererOptions::default());
        Self {
            ctx,
            state,
            renderer,
            release_pointer: false,
        }
    }

    pub(crate) fn handle_event(&mut self, window: &Window, event: &WindowEvent) -> egui_winit::EventResponse {
        self.state.on_window_event(window, event)
    }

    /// Let go of the primary button at the next frame, unless it has been let
    /// go of by then: once the system is moving or resizing the window, the
    /// release may never reach it. See `Gui::release_pointer`.
    pub(crate) fn release_pointer(&mut self) {
        self.release_pointer = true;
    }

    pub(crate) fn register_native_texture(&mut self, device: &wgpu::Device, view: &wgpu::TextureView) -> egui::TextureId {
        self.renderer.register_native_texture(device, view, wgpu::FilterMode::Linear)
    }

    pub(crate) fn update_native_texture(&mut self, device: &wgpu::Device, id: egui::TextureId, view: &wgpu::TextureView) {
        self.renderer.update_egui_texture_from_wgpu_texture(device, view, wgpu::FilterMode::Linear, id);
    }

    /// Points to physical pixels in the window, as the last frame drew.
    pub(crate) fn pixels_per_point(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    /// Lay the window's UI out with `draw`, in the editor's theme and UI size.
    pub(crate) fn layout(
        &mut self,
        window: &Window,
        editor: &mut EditorState,
        screen_size: [u32; 2],
        mut draw: impl FnMut(&mut egui::Ui, &mut EditorState, &mut Vec<UiCommand>),
    ) -> SecondaryFrame {
        let zoom = scaling::zoom_factor(editor.ui_size_percent);
        let old_zoom = self.ctx.zoom_factor();
        if zoom != old_zoom {
            // Before the input is taken, as the main window does: see
            // `Gui::update_scale`.
            self.ctx.options_mut(|options| options.zoom_factor = zoom);
            scaling::rescale_events(&mut self.state.egui_input_mut().events, old_zoom / zoom);
        }
        if self.ctx.global_style().visuals.dark_mode != editor.dark_mode {
            self.ctx.set_visuals(theme_visuals(editor.dark_mode, SELECTION_COLOR));
        }
        let mut raw_input = self.state.take_egui_input(window);
        if std::mem::take(&mut self.release_pointer) {
            let released = raw_input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::PointerButton {
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    }
                )
            });
            let (down, pos, modifiers) = self.ctx.input(|input| (input.pointer.primary_down(), input.pointer.latest_pos(), input.modifiers));
            if down
                && !released
                && let Some(pos) = pos
            {
                raw_input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers,
                });
            }
        }
        let mut commands = Vec::new();
        let full_output = self.ctx.run_ui(raw_input, |ui| draw(ui, editor, &mut commands));
        let repaint_after = full_output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|output| output.repaint_delay)
            .filter(|delay| *delay != Duration::MAX);
        self.state.handle_platform_output(window, full_output.platform_output);
        let pixels_per_point = full_output.pixels_per_point;
        let paint = UiPaint {
            jobs: self.ctx.tessellate(full_output.shapes, pixels_per_point),
            textures: full_output.textures_delta,
            screen: ScreenDescriptor {
                size_in_pixels: screen_size,
                pixels_per_point,
            },
        };
        SecondaryFrame { commands, paint, repaint_after }
    }

    /// Paint a laid-out frame over `target_view`, cleared to the window's
    /// background first: nothing is drawn under this window's UI.
    pub(crate) fn paint(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, target_view: &wgpu::TextureView, paint: UiPaint) {
        // Raw bytes, as egui paints them: the view is not sRGB.
        let fill = super::chrome::window_fill(&self.ctx.global_style().visuals);
        let channel = |value: u8| f64::from(value) / 255.0;
        let clear = wgpu::Color {
            r: channel(fill.r()),
            g: channel(fill.g()),
            b: channel(fill.b()),
            a: 1.0,
        };
        paint_ui(&mut self.renderer, device, queue, encoder, target_view, paint, wgpu::LoadOp::Clear(clear));
    }
}
