//! The video export's frames, rendered offscreen at the video's own size.
//!
//! Each frame draws the scene from the viewport's camera into a target of
//! exactly the requested size - framing the largest centred part of what the
//! viewport shows that has the video's shape - then paints the parts of
//! Animate the viewport draws through egui (destination labels, machine
//! icons) over it with an egui context of its own. The window's size, shape
//! and chrome never reach the video.
//!
//! Readback does not wait: each frame's copy is mapped in the background and
//! collected on a later frame, a few in flight at once, so the GPU and the
//! app overlap rather than taking turns. Nothing here blocks, which the
//! browser's main thread cannot.

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

#[cfg(not(target_arch = "wasm32"))]
use super::offscreen::ViewAim;
use super::{offscreen::OffscreenTarget, readback::RgbaReadback, *};

/// Frames copied back but not yet collected before the export stops
/// drawing new ones; see [`Graphics::video_readback_full`].
const MAX_IN_FLIGHT: usize = 3;

const MAP_PENDING: u8 = 0;
const MAP_DONE: u8 = 1;
const MAP_FAILED: u8 = 2;

/// Which names are drawn over the video's scene.
#[derive(Clone, Copy, Debug)]
pub(crate) struct VideoLabels {
    pub(crate) destinations: bool,
    pub(crate) machines: bool,
}

/// What one video frame is drawn from.
pub(crate) struct VideoFrameInput<'frame> {
    pub(crate) editor: &'frame mut EditorState,
    pub(crate) document: &'frame Document,
    pub(crate) triangulations: &'frame [OpenTriangulation],
    pub(crate) block_models: &'frame [OpenBlockModel],
    pub(crate) drill_holes: &'frame [OpenDrillHoleDataset],
    pub(crate) point_clouds: &'frame [OpenPointCloud],
    pub(crate) rasters: &'frame [OpenRasterTexture],
    pub(crate) project: &'frame crate::ui::state::UiProjectView,
    /// The video's size in pixels.
    pub(crate) size: (u32, u32),
    /// Seconds into the video, which sets how far the flows' stripes have
    /// travelled, so they move at the video's pace rather than the window's.
    pub(crate) clock_s: f64,
    pub(crate) labels: VideoLabels,
    /// The view to record from; the window's camera when `None`. Set by
    /// the MCP server only, which the browser has none of.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) view: Option<VideoView>,
    /// Bring the GPU's copy of the scene up to date first, as there is no
    /// window frame doing it.
    pub(crate) sync: bool,
}

/// A video's own view of the scene, in place of the window's.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct VideoView {
    pub(crate) aim: ViewAim,
    pub(crate) perspective: bool,
}

struct InFlight {
    readback: RgbaReadback,
    state: Arc<AtomicU8>,
}

pub(crate) struct VideoTarget {
    target: OffscreenTarget,
    overlay: crate::ui::VideoOverlay,
    in_flight: VecDeque<InFlight>,
    /// A video's own view as placed on its first frame and held after, so
    /// the shot stays put while the ground is cut under it.
    #[cfg(not(target_arch = "wasm32"))]
    held: Option<ViewAim>,
}

impl VideoTarget {
    fn new(graphics: &Graphics<'_>, width: u32, height: u32) -> Self {
        let target = OffscreenTarget::new(graphics, width, height, "Video frame");
        let overlay = crate::ui::VideoOverlay::new(
            &graphics.device,
            target.config.format.remove_srgb_suffix(),
            graphics.gui.as_ref().map_or_else(crate::ui::default_visuals, Gui::visuals),
        );
        Self {
            target,
            overlay,
            in_flight: VecDeque::new(),
            #[cfg(not(target_arch = "wasm32"))]
            held: None,
        }
    }
}

impl Graphics<'_> {
    /// Whether as many frames as are let be in flight are still being
    /// copied back, so the next should wait.
    pub(crate) fn video_readback_full(&self) -> bool {
        self.video_target.as_ref().is_some_and(|target| target.in_flight.len() >= MAX_IN_FLIGHT)
    }

    /// Render one video frame and start copying it back; collect it with
    /// [`Self::take_video_frames`].
    pub(crate) fn render_video_frame(&mut self, input: VideoFrameInput<'_>) {
        let VideoFrameInput {
            editor,
            document,
            triangulations,
            block_models,
            drill_holes,
            point_clouds,
            rasters,
            project,
            size: (width, height),
            clock_s,
            labels: shown_labels,
            #[cfg(not(target_arch = "wasm32"))]
            view,
            sync,
        } = input;
        let mut target = match self.video_target.take() {
            Some(target) if target.target.size.width == width && target.target.size.height == height => target,
            _ => VideoTarget::new(self, width, height),
        };
        // The window's projection this frame - its clip planes fitted to the
        // scene - reshaped to the video.
        target.target.projection = self.projection.cropped_to(width, height);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(view) = view {
            let mut projection = self.projection.clone();
            projection.resize(width, height);
            projection.set_perspective(view.perspective);
            target.target.projection = projection;
        }
        // Labels and icons as large against the frame as they are against the
        // viewport: the video is as many points tall as the viewport is.
        let window_ppp = self.ui_pixels_per_point().max(0.1);
        let viewport_points = (self.viewport_rect.height as f32 / window_ppp).max(1.0);
        let shrink = ((self.viewport_rect.width as f32 / self.viewport_rect.height.max(1) as f32) / (width as f32 / height as f32)).min(1.0);
        let pixels_per_point = height as f32 / (viewport_points * shrink);

        // The window's own projections of the flows, pins and icons; put back
        // once this frame is drawn, so the window's next frame is untouched.
        let window_flows = std::mem::take(&mut editor.animation_flows);
        let window_pins = std::mem::take(&mut editor.haul_pins);
        let window_icons = std::mem::take(&mut editor.animation_machine_icons);
        let window_labels = std::mem::take(&mut editor.animation_machine_labels);

        // A video's own view takes the camera, and leaves out the window's
        // section, for the frame; both are put back after.
        #[cfg(not(target_arch = "wasm32"))]
        let window_view = view.map(|_| (self.camera.clone(), self.slice_view.take()));
        target.target.swap_into(self);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(view) = view {
            let held = match target.held {
                Some(held) => held,
                None => {
                    let held = match view.aim {
                        ViewAim::Frame { forward, up, bounds: None } => ViewAim::Frame {
                            forward,
                            up,
                            bounds: self.scene_bounds_now(document, triangulations, block_models, drill_holes, point_clouds, &editor.hidden_handles),
                        },
                        aim => aim,
                    };
                    target.held = Some(held);
                    held
                }
            };
            if let Err(error) = self.place_camera(held, document, triangulations, block_models, drill_holes, point_clouds, &editor.hidden_handles) {
                log::warn!("The video's view could not be placed: {error}");
            }
        }
        self.camera_uniform
            .update_view_proj(&self.camera, &self.projection, self.scene_origin, self.vertical_exaggeration);
        self.camera_uniform.set_section_slab(self.section_slab(), self.scene_origin);
        self.camera_uniform.update_viewport(width, height);
        self.camera_uniform.set_viewport_origin(0, 0);
        self.camera_uniform.set_interaction_quality(1.0, 1.0);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&self.camera_uniform));

        self.project_haul_flows(editor);
        let view_proj = self.view_proj();
        editor.haul_pins = if editor.shows_haul_network() {
            document
                .haulage()
                .nodes
                .iter()
                .filter_map(|node| Some((self.world_to_window_px(&view_proj, node.pos)?, node.role?)))
                .collect()
        } else {
            Vec::new()
        };
        self.upload_flow_scene(editor, pixels_per_point, clock_s);
        let (near, icons, labels) = self.split_machines(&editor.animation_machines, pixels_per_point);
        editor.animation_machine_icons = icons;
        editor.animation_machine_labels = labels;
        if !near.is_empty() && self.machines.is_none() {
            self.machines = Some(crate::rendering::machines::MachineGpu::new(&self.device).map_err(|error| {
                crate::userspace_error!("{error:#}");
            }));
        }
        if let Some(Ok(machines)) = &mut self.machines {
            machines.place(&self.device, &self.queue, &near, self.scene_origin);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Video frame encoder"),
        });
        if sync {
            let retained = std::collections::HashSet::new();
            let scale_factor = self.scale_factor() as f32;
            self.sync_scene_content(
                &mut encoder,
                frame::SceneContent {
                    editor,
                    document,
                    triangulations,
                    block_models,
                    drill_holes,
                    point_clouds,
                    rasters,
                    solid_preview: &[],
                    retained_surfaces: &retained,
                },
                scale_factor,
            );
        }
        let full = ViewportRect::full(width, height);
        // The viewport's view, overlays included, as the image export draws it.
        self.render_scene_pass(
            &mut encoder,
            &target.target.view,
            full,
            editor,
            triangulations,
            block_models,
            drill_holes,
            point_clouds,
            rasters,
            true,
        );
        self.render_editor_overlay_pass(&mut encoder, &target.target.view, full, editor, false);
        target.overlay.paint(
            &self.device,
            &self.queue,
            &mut encoder,
            &target.target.gui_view,
            [width, height],
            pixels_per_point,
            editor,
            document,
            project,
            shown_labels,
        );
        let readback = RgbaReadback::record(&self.device, &mut encoder, &target.target.texture, "Video frame readback");
        self.queue.submit(std::iter::once(encoder.finish()));
        target.target.swap_into(self);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some((camera, section)) = window_view {
            self.camera = camera;
            self.slice_view = section;
            self.scene_cache_key = None;
        }

        let state = Arc::new(AtomicU8::new(MAP_PENDING));
        let signal = Arc::clone(&state);
        readback.buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            signal.store(if result.is_ok() { MAP_DONE } else { MAP_FAILED }, Ordering::Release);
        });
        target.in_flight.push_back(InFlight { readback, state });

        editor.animation_flows = window_flows;
        editor.haul_pins = window_pins;
        editor.animation_machine_icons = window_icons;
        editor.animation_machine_labels = window_labels;
        self.video_target = Some(target);
    }

    /// The frames whose copies have arrived, oldest first. `Err` when a copy
    /// failed.
    pub(crate) fn take_video_frames(&mut self) -> anyhow::Result<Vec<Vec<u8>>> {
        let Some(target) = self.video_target.as_mut() else {
            return Ok(Vec::new());
        };
        let _ = self.device.poll(wgpu::PollType::Poll);
        let mut frames = Vec::new();
        while let Some(front) = target.in_flight.front() {
            match front.state.load(Ordering::Acquire) {
                MAP_PENDING => break,
                MAP_FAILED => {
                    target.in_flight.clear();
                    anyhow::bail!(crate::i18n::tr!("err-gfx-video-frame-read"));
                }
                _ => {
                    let done = target.in_flight.pop_front().expect("checked above");
                    let pixels = done.readback.unpack();
                    done.readback.buffer.unmap();
                    frames.push(pixels?);
                }
            }
        }
        Ok(frames)
    }

    /// Drop the video target and whatever it still has in flight.
    pub(crate) fn release_video_target(&mut self) {
        self.video_target = None;
    }
}
