//! The video export's frames: the finished viewport - scene, overlay and the
//! egui labels and icons drawn over it - copied back from the window's own
//! surface after the frame is drawn.
//!
//! Unlike the image export, which renders the scene again into a texture of
//! its own, this reads what the user sees, so Animate's machine icons and
//! labels are in the video.

use super::{readback::RgbaReadback, *};

impl Graphics<'_> {
    /// Whether frames can be read back from this window at all.
    pub(crate) fn can_capture_video(&self) -> bool {
        self.surface_copyable
    }

    /// The viewport's size in physical pixels.
    pub(crate) fn viewport_size(&self) -> (u32, u32) {
        (self.viewport_rect.width, self.viewport_rect.height)
    }

    /// Have the next render copy the viewport's top-left `width` x `height`
    /// back; [`Self::take_video_frame`] then has it.
    pub(crate) fn request_video_frame(&mut self, width: u32, height: u32) {
        self.pending_video_frame = Some((width, height));
        self.window.request_redraw();
    }

    /// The frame the last render copied back, once: `Some(None)` when the
    /// viewport was no longer the size the recording started at.
    pub(crate) fn take_video_frame(&mut self) -> Option<Option<Vec<u8>>> {
        self.video_frame.take()
    }

    /// Record the copy on the frame's encoder, after egui has drawn into the
    /// surface. `None` when the viewport has changed size, which ends the
    /// recording rather than stretching or cropping what follows.
    pub(super) fn encode_video_capture(&mut self, encoder: &mut wgpu::CommandEncoder, surface: &wgpu::Texture) -> Option<Option<RgbaReadback>> {
        let (width, height) = self.pending_video_frame.take()?;
        let rect = self.viewport_rect;
        if rect.width & !1 != width || rect.height & !1 != height || rect.x + width > surface.width() || rect.y + height > surface.height() {
            return Some(None);
        }
        Some(Some(RgbaReadback::record_region(
            &self.device,
            encoder,
            surface,
            (rect.x, rect.y),
            (width, height),
            "Video Frame Readback Buffer",
        )))
    }

    /// Map the copy and keep its pixels for the app to collect. Runs after
    /// `queue.submit`; blocks on the GPU, which a recording that already
    /// waits on every frame can afford.
    pub(super) fn finish_video_capture(&mut self, capture: Option<RgbaReadback>) {
        let pixels = capture.and_then(|readback| {
            let mapped = readback.map_blocking(&self.device).and_then(|()| {
                let pixels = readback.unpack();
                readback.buffer.unmap();
                pixels
            });
            match mapped {
                Ok(pixels) => Some(pixels),
                Err(error) => {
                    log::warn!("Video frame readback failed: {error:#}");
                    None
                }
            }
        });
        self.video_frame = Some(pixels);
    }
}
