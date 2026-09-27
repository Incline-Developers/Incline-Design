//! Offscreen colour targets copied back to the CPU as RGBA8, shared by the
//! viewport image export and the plot map render.

use std::sync::Arc;

use anyhow::{Result, anyhow};

/// A `width` x `height` colour target in `format` that can be rendered into and
/// copied out of.
pub(super) fn offscreen_target(device: &wgpu::Device, label: &str, format: wgpu::TextureFormat, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// A texture copied into a mappable buffer, awaiting readback.
pub(super) struct RgbaReadback {
    /// Shared so a browser map callback can unmap the buffer it was handed.
    pub(super) buffer: Arc<wgpu::Buffer>,
    padded_bytes_per_row: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    format: wgpu::TextureFormat,
}

impl RgbaReadback {
    /// Record a copy of all of `texture` into a new mappable buffer.
    pub(super) fn record(device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, texture: &wgpu::Texture, label: &str) -> Self {
        let (width, height) = (texture.width(), texture.height());
        let padded_bytes_per_row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        // On wasm `wgpu::Buffer` is not Send+Sync; the Arc never crosses threads
        // there, but the native readback path requires Arc over Rc.
        #[cfg_attr(target_arch = "wasm32", allow(clippy::arc_with_non_send_sync))]
        let buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: u64::from(padded_bytes_per_row) * u64::from(height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        Self {
            buffer,
            padded_bytes_per_row,
            width,
            height,
            format: texture.format(),
        }
    }

    /// Map the buffer, blocking until the GPU has finished the copy.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn map_blocking(&self, device: &wgpu::Device) -> Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = tx.send(result);
        });
        device.poll(wgpu::PollType::wait_indefinitely()).map_err(|error| anyhow!("GPU poll failed: {error}"))?;
        rx.recv()
            .map_err(|_| anyhow!("Readback callback dropped"))?
            .map_err(|error| anyhow!("Buffer map failed: {error}"))
    }

    /// The mapped pixels with wgpu's row padding stripped and the surface
    /// format normalised to opaque RGBA8.
    pub(super) fn unpack(&self) -> Result<Vec<u8>> {
        let swap_bgra = match self.format.remove_srgb_suffix() {
            wgpu::TextureFormat::Bgra8Unorm => true,
            wgpu::TextureFormat::Rgba8Unorm => false,
            other => return Err(anyhow!("Unsupported surface format for image export: {other:?}")),
        };
        let padded = self.buffer.get_mapped_range(..)?;
        let row_bytes = self.width as usize * 4;
        let mut rgba = Vec::with_capacity(row_bytes * self.height as usize);
        for row in padded.chunks_exact(self.padded_bytes_per_row as usize) {
            rgba.extend_from_slice(&row[..row_bytes]);
        }
        drop(padded);
        if swap_bgra {
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel[3] = 255;
        }
        Ok(rgba)
    }
}
