//! The windows of panels popped out of the main one: a surface each on the
//! main window's device, with nothing on it but its UI
//! ([`crate::ui::secondary::SecondaryGui`]).
//!
//! `App` opens and closes them (`app/panel_windows.rs`); this holds what
//! drawing them takes. The section preview's picture is still rendered by
//! the main window's frame, into the texture the docked preview shows
//! (`slice_preview.rs`); its window shows the same texture, registered with
//! its own renderer.

use std::{sync::Arc, time::Duration};

use winit::{
    dpi::PhysicalSize,
    event::WindowEvent,
    window::{Window, WindowId},
};

use super::{Graphics, RenderSurfaceError, window_corners::WindowCorners};
use crate::ui::{
    secondary::SecondaryGui,
    state::{EditorState, PanelKind, UiCommand},
};

pub(super) struct PanelWindow {
    kind: PanelKind,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gui: SecondaryGui,
    /// Cuts the window's corners round where it has to do so itself, as the
    /// main window does (`window_corners`).
    corners: Option<WindowCorners>,
    /// The section preview's picture as this window's renderer knows it, and
    /// the generation of the texture it was registered from.
    preview_texture: Option<(u64, egui::TextureId)>,
}

/// What one frame of a panel's window asked for.
pub(crate) struct PanelFrame {
    pub(crate) commands: Vec<UiCommand>,
    pub(crate) repaint_after: Option<Duration>,
}

impl PanelWindow {
    fn new(graphics: &Graphics<'_>, kind: PanelKind, window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let surface = graphics.instance.create_surface(window.clone())?;
        let capabilities = surface.get_capabilities(&graphics.adapter);
        if !capabilities.formats.contains(&graphics.config.format) {
            anyhow::bail!(crate::i18n::tr!("err-gfx-preview-format"));
        }
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Fifo)
            .or_else(|| capabilities.present_modes.first().copied())
            .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr!("err-gfx-preview-present-mode")))?;
        // Transparent on Wayland, to cut its own corners as the main window
        // does; opaque elsewhere where there is a choice.
        let corners_alpha_mode = WindowCorners::alpha_mode(&window, &capabilities);
        let alpha_mode = match corners_alpha_mode {
            Some(mode) => mode,
            None if capabilities.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) => wgpu::CompositeAlphaMode::Opaque,
            None => capabilities
                .alpha_modes
                .first()
                .copied()
                .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr!("err-gfx-preview-alpha-mode")))?,
        };
        let format = graphics.config.format;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![format.remove_srgb_suffix()],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&graphics.device, &config);
        let gui = SecondaryGui::new(&window, &graphics.device, format.remove_srgb_suffix());
        let corners = corners_alpha_mode.map(|_| WindowCorners::new(&graphics.device, format.remove_srgb_suffix()));
        Ok(Self {
            kind,
            window,
            surface,
            config,
            gui,
            corners,
            preview_texture: None,
        })
    }
}

impl Graphics<'_> {
    fn panel(&self, kind: PanelKind) -> Option<&PanelWindow> {
        self.panel_windows.iter().find(|panel| panel.kind == kind)
    }

    fn panel_mut(&mut self, kind: PanelKind) -> Option<&mut PanelWindow> {
        self.panel_windows.iter_mut().find(|panel| panel.kind == kind)
    }

    pub(crate) fn open_panel_window(&mut self, kind: PanelKind, window: Arc<Window>) -> anyhow::Result<()> {
        let panel = PanelWindow::new(self, kind, window)?;
        panel.window.request_redraw();
        self.panel_windows.retain(|open| open.kind != kind);
        self.panel_windows.push(panel);
        Ok(())
    }

    pub(crate) fn close_panel_window(&mut self, kind: PanelKind) {
        self.panel_windows.retain(|panel| panel.kind != kind);
    }

    /// The panel whose window this is, if it is one.
    pub(crate) fn panel_window_kind(&self, id: WindowId) -> Option<PanelKind> {
        self.panel_windows.iter().find(|panel| panel.window.id() == id).map(|panel| panel.kind)
    }

    pub(crate) fn panel_window(&self, kind: PanelKind) -> Option<&Arc<Window>> {
        self.panel(kind).map(|panel| &panel.window)
    }

    /// Points to physical pixels in `kind`'s window, as it last drew.
    pub(crate) fn panel_pixels_per_point(&self, kind: PanelKind) -> f32 {
        self.panel(kind).map_or(1.0, |panel| panel.gui.pixels_per_point())
    }

    /// Whether `kind`'s window cuts its own corners round.
    pub(crate) fn panel_cuts_corners(&self, kind: PanelKind) -> bool {
        self.panel(kind).is_some_and(|panel| panel.corners.is_some())
    }

    /// The radius of `kind`'s window's corners, in physical pixels.
    pub(crate) fn set_panel_corner_radius(&mut self, kind: PanelKind, radius: f32) {
        if let Some(corners) = self.panel_mut(kind).and_then(|panel| panel.corners.as_mut()) {
            corners.set_radius(radius);
        }
    }

    /// See [`SecondaryGui::release_pointer`].
    pub(crate) fn release_panel_pointer(&mut self, kind: PanelKind) {
        if let Some(panel) = self.panel_mut(kind) {
            panel.gui.release_pointer();
        }
    }

    /// Hand an event to `kind`'s UI, and draw it again if the UI asks.
    pub(crate) fn panel_window_event(&mut self, kind: PanelKind, event: &WindowEvent) {
        if let Some(panel) = self.panel_mut(kind)
            && panel.gui.handle_event(&panel.window, event).repaint
        {
            panel.window.request_redraw();
        }
    }

    pub(crate) fn resize_panel_window(&mut self, kind: PanelKind, size: PhysicalSize<u32>) {
        let device = self.device.clone();
        let Some(panel) = self.panel_mut(kind) else {
            return;
        };
        if size.width > 0 && size.height > 0 {
            panel.config.width = size.width;
            panel.config.height = size.height;
            panel.surface.configure(&device, &panel.config);
            panel.window.request_redraw();
        }
    }

    /// Lay `kind`'s window out with `draw` and present it.
    pub(crate) fn render_panel_window(
        &mut self,
        kind: PanelKind,
        editor: &mut EditorState,
        draw: impl FnMut(&mut egui::Ui, &mut EditorState, &mut Vec<UiCommand>),
    ) -> Result<PanelFrame, RenderSurfaceError> {
        let preview = (kind == PanelKind::SectionPreview).then(|| self.section_preview_view()).flatten();
        let Some(index) = self.panel_windows.iter().position(|panel| panel.kind == kind) else {
            return Ok(PanelFrame {
                commands: Vec::new(),
                repaint_after: None,
            });
        };
        let panel = &mut self.panel_windows[index];
        if kind == PanelKind::SectionPreview {
            editor.slice_preview_window_texture = preview.map(|(generation, view)| match panel.preview_texture {
                Some((registered, id)) if registered == generation => id,
                Some((_, id)) => {
                    panel.gui.update_native_texture(&self.device, id, &view);
                    panel.preview_texture = Some((generation, id));
                    id
                }
                None => {
                    let id = panel.gui.register_native_texture(&self.device, &view);
                    panel.preview_texture = Some((generation, id));
                    id
                }
            });
        }
        let output = match panel.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(output) | wgpu::CurrentSurfaceTexture::Suboptimal(output) => output,
            wgpu::CurrentSurfaceTexture::Timeout => return Err(RenderSurfaceError::Timeout),
            wgpu::CurrentSurfaceTexture::Occluded => return Err(RenderSurfaceError::Occluded),
            wgpu::CurrentSurfaceTexture::Outdated => {
                panel.surface.configure(&self.device, &panel.config);
                return Err(RenderSurfaceError::Outdated);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                panel.surface.configure(&self.device, &panel.config);
                return Err(RenderSurfaceError::Lost);
            }
            wgpu::CurrentSurfaceTexture::Validation => return Err(RenderSurfaceError::Validation),
        };
        // Raw byte writes, as for the main window's UI: egui applies gamma
        // itself.
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(panel.config.format.remove_srgb_suffix()),
            ..Default::default()
        });
        let frame = panel.gui.layout(&panel.window, editor, [panel.config.width, panel.config.height], draw);
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Panel window") });
        panel.gui.paint(&self.device, &self.queue, &mut encoder, &view, frame.paint);
        // Last, over the UI too: see `window_corners`.
        if let Some(corners) = &panel.corners {
            corners.draw(&self.queue, &mut encoder, &view, [panel.config.width, panel.config.height]);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        panel.window.pre_present_notify();
        self.queue.present(output);
        Ok(PanelFrame {
            commands: frame.commands,
            repaint_after: frame.repaint_after,
        })
    }
}
