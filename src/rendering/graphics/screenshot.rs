//! One-shot viewport image export (File > Export Viewport Image...).
//!
//! The scene pass already resolves into an arbitrary target, so the capture
//! renders one extra frame into an offscreen texture (no egui chrome), copies
//! it into a mappable buffer alongside the normal frame's command encoder, and
//! encodes a PNG after the queue submit.

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use super::{
    readback::{self, RgbaReadback},
    *,
};
use crate::{userspace_log, userspace_warn};

pub(crate) enum ScreenshotTarget {
    #[cfg(not(target_arch = "wasm32"))]
    Native(PathBuf),
    #[cfg(target_arch = "wasm32")]
    Browser(String),
}

/// GPU-side capture state produced before submit and consumed after present.
pub(super) struct PendingScreenshot {
    readback: RgbaReadback,
    /// The visible-viewport sub-rect of the (full-window-sized) capture to
    /// keep when encoding the PNG - see `encode_mapped_png`.
    crop: ViewportRect,
    target: ScreenshotTarget,
}

impl<'a> Graphics<'a> {
    /// Queue a viewport export; the next rendered frame writes the PNG.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn request_screenshot(&mut self, path: PathBuf) {
        self.pending_screenshot = Some(ScreenshotTarget::Native(path));
        self.window.request_redraw();
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn request_browser_screenshot(&mut self, file_name: String) {
        self.pending_screenshot = Some(ScreenshotTarget::Browser(file_name));
        self.window.request_redraw();
    }

    /// Render the scene into an offscreen texture and record a copy of it into
    /// a mappable buffer on the frame's encoder. Runs before `queue.submit`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode_screenshot_capture(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        editor: &EditorState,
        triangulations: &[OpenTriangulation],
        block_models: &[OpenBlockModel],
        drill_holes: &[OpenDrillHoleDataset],
        point_clouds: &[OpenPointCloud],
        rasters: &[OpenRasterTexture],
        target: ScreenshotTarget,
    ) -> PendingScreenshot {
        let width = self.config.width.max(1);
        let height = self.config.height.max(1);
        let (texture, view) = readback::offscreen_target(&self.device, "Screenshot Target", self.config.format.add_srgb_suffix(), width, height);
        // An export is one of the places the cinematic view earns its keep.
        // Reuse the graded scene, including the ungraded designs and drill
        // holes, then add the live overlay exactly as the viewport does.
        #[cfg(not(target_arch = "wasm32"))]
        let from_cache = editor.cinematic_enabled && self.scene_cache_key.is_some();
        #[cfg(target_arch = "wasm32")]
        let from_cache = false;
        if !from_cache {
            self.render_scene_pass(
                encoder,
                &view,
                self.viewport_rect,
                editor,
                triangulations,
                block_models,
                drill_holes,
                point_clouds,
                rasters,
                true,
            );
        }
        // The export is the viewport as the user sees it, so it carries the
        // live overlay too. Without the cinematic view the scene was just
        // rendered into the multisample target and there is nothing to restore
        // from the cache; with it, the cache is where the finished image is.
        self.render_editor_overlay_pass(encoder, &view, self.viewport_rect, editor, from_cache);

        PendingScreenshot {
            readback: RgbaReadback::record(&self.device, encoder, &texture, "Screenshot Readback Buffer"),
            crop: self.viewport_rect,
            target,
        }
    }

    /// Map the readback buffer and write the PNG. Runs after `queue.submit`;
    /// blocks on the GPU, which is fine for a one-shot export.
    pub(super) fn finish_screenshot_capture(&self, capture: PendingScreenshot) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = match &capture.target {
                ScreenshotTarget::Native(path) => path.clone(),
            };
            if let Err(error) = self.write_screenshot_png(&capture) {
                userspace_warn!(
                    "{}",
                    crate::i18n::tr_format!(
                        literal = "Could not save viewport image %path%: %error%",
                        path = path.display(),
                        error = format!("{error:#}")
                    )
                );
            } else {
                userspace_log!("{}", crate::i18n::tr_format!(literal = "Saved viewport image: %path%", path = path.display()));
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let buffer = std::sync::Arc::clone(&capture.readback.buffer);
            let callback_buffer = std::sync::Arc::clone(&buffer);
            buffer.map_async(wgpu::MapMode::Read, .., move |result| {
                if let Err(error) = result {
                    userspace_warn!("{}", crate::i18n::tr_format!(literal = "Could not map viewport screenshot: %error%", error = error));
                    return;
                }
                let encoded = encode_mapped_png(&capture);
                callback_buffer.unmap();
                match (capture.target, encoded) {
                    (ScreenshotTarget::Browser(file_name), Ok(bytes)) => match crate::app::web_download::download(&file_name, &bytes, "image/png") {
                        Ok(()) => userspace_log!("{}", crate::i18n::tr_format!(literal = "Downloaded viewport image: %file_name%", file_name = file_name)),
                        Err(error) => {
                            userspace_warn!("{}", crate::i18n::tr_format!(literal = "Viewport image download failed: %error%", error = error))
                        }
                    },
                    (_, Err(error)) => {
                        userspace_warn!(
                            "{}",
                            crate::i18n::tr_format!(literal = "Could not encode viewport image: %error%", error = format!("{error:#}"))
                        )
                    }
                }
            });
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn write_screenshot_png(&self, capture: &PendingScreenshot) -> Result<()> {
        let ScreenshotTarget::Native(path) = &capture.target;
        capture.readback.map_blocking(&self.device)?;
        let png = encode_mapped_png(capture)?;
        capture.readback.buffer.unmap();
        std::fs::write(path, png)?;
        Ok(())
    }
}

fn encode_mapped_png(capture: &PendingScreenshot) -> Result<Vec<u8>> {
    let rgba = capture.readback.unpack()?;
    let (width, height) = (capture.readback.width, capture.readback.height);

    // The capture covers the full window; keep only the visible-viewport
    // sub-rect (`crop`), clamped defensively in case it's a frame stale
    // relative to a resize that just landed - see `Graphics::apply_canvas_rect`.
    let crop_x = capture.crop.x.min(width.saturating_sub(1)) as usize;
    let crop_y = capture.crop.y.min(height.saturating_sub(1)) as usize;
    let crop_width = (capture.crop.width.min(width - crop_x as u32)).max(1) as usize;
    let crop_height = (capture.crop.height.min(height - crop_y as u32)).max(1) as usize;
    let full_frame = crop_x == 0 && crop_y == 0 && crop_width == width as usize && crop_height == height as usize;
    let cropped = if full_frame {
        rgba
    } else {
        let mut out = Vec::with_capacity(crop_width * crop_height * 4);
        for row in 0..crop_height {
            let start = ((crop_y + row) * width as usize + crop_x) * 4;
            out.extend_from_slice(&rgba[start..start + crop_width * 4]);
        }
        out
    };

    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, crop_width as u32, crop_height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&cropped)?;
        writer.finish()?;
    }
    Ok(bytes)
}
