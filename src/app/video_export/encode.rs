//! Turning captured viewport frames into a video file.
//!
//! Frames arrive as opaque RGBA8, one per call, at a fixed frame rate. AV1
//! (rav1e) is written into MP4, MOV or WebM by the small muxers beside this
//! module; GIF is written by the `gif` crate. Nothing here knows about the
//! schedule: it is handed pixels and somewhere to write them.

use std::{
    io::{BufWriter, Seek, Write},
    path::PathBuf,
};

use anyhow::{Result, anyhow};

/// What a video can be saved as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VideoFormat {
    Mp4,
    Mov,
    WebM,
    Gif,
}

impl VideoFormat {
    /// Every format, in the order the save dialog offers them: the first is
    /// what a name with no extension is saved as, and what the browser
    /// downloads.
    pub(crate) const ALL: [Self; 4] = [Self::Mp4, Self::WebM, Self::Mov, Self::Gif];

    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mov => "mov",
            Self::WebM => "webm",
            Self::Gif => "gif",
        }
    }

    /// The save dialog's name for the format.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn filter_name(self) -> &'static str {
        match self {
            Self::Mp4 => "MP4 video (AV1)",
            Self::Mov => "QuickTime movie (AV1)",
            Self::WebM => "WebM video (AV1)",
            Self::Gif => "GIF animation",
        }
    }

    /// What the browser's download is labelled as.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn mime_type(self) -> &'static str {
        match self {
            Self::Mp4 => "video/mp4",
            Self::Mov => "video/quicktime",
            Self::WebM => "video/webm",
            Self::Gif => "image/gif",
        }
    }

    /// The format a saved path names by its extension.
    pub(crate) fn of_path(path: &std::path::Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        Self::ALL.into_iter().find(|format| format.extension() == extension)
    }
}

/// Where an encoder writes. The muxers seek back to fill in sizes once the
/// frames are in.
pub(crate) trait VideoOutput: Write + Seek + Send {}

impl<T: Write + Seek + Send> VideoOutput for T {}

/// Where a finished video ends up: the file chosen on desktop; in the
/// browser, memory, downloaded once the video is finished.
pub(crate) struct Destination {
    path: PathBuf,
    #[cfg(target_arch = "wasm32")]
    memory: memory::MemoryFile,
}

impl Destination {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            #[cfg(target_arch = "wasm32")]
            memory: memory::MemoryFile::default(),
        }
    }

    fn output(&self) -> Result<Box<dyn VideoOutput>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use anyhow::Context as _;
            let file = std::fs::File::create(&self.path).with_context(|| format!("Could not create {}", self.path.display()))?;
            Ok(Box::new(file))
        }
        #[cfg(target_arch = "wasm32")]
        Ok(Box::new(self.memory.clone()))
    }

    /// The finished video: its path, and in the browser its bytes too.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn finished(self) -> PathBuf {
        self.path
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn finished(self) -> (PathBuf, Vec<u8>) {
        (self.path, self.memory.take())
    }

    /// Remove the output if a run is abandoned, so a cancelled export leaves
    /// no half-written file behind.
    pub(crate) fn discard(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(target_arch = "wasm32")]
mod memory {
    use std::{
        io::{Cursor, Result, Seek, SeekFrom, Write},
        sync::{Arc, Mutex},
    };

    /// A growable in-memory file, shared so the export can take its bytes
    /// back once the encoder that owns a handle has finished with it.
    #[derive(Clone, Default)]
    pub(super) struct MemoryFile(Arc<Mutex<Cursor<Vec<u8>>>>);

    impl MemoryFile {
        pub(super) fn take(&self) -> Vec<u8> {
            std::mem::take(self.0.lock().expect("video memory poisoned").get_mut())
        }
    }

    impl Write for MemoryFile {
        fn write(&mut self, buf: &[u8]) -> Result<usize> {
            self.0.lock().expect("video memory poisoned").write(buf)
        }

        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }

    impl Seek for MemoryFile {
        fn seek(&mut self, position: SeekFrom) -> Result<u64> {
            self.0.lock().expect("video memory poisoned").seek(position)
        }
    }
}

/// Something that takes frames and writes a file when finished.
pub(crate) trait FrameEncoder: Send {
    /// Encode one frame: `width * height * 4` bytes of RGBA, top row first.
    fn push(&mut self, rgba: &[u8]) -> Result<()>;
    /// Flush what is buffered and close the file.
    fn finish(self: Box<Self>) -> Result<()>;
}

/// Open an encoder writing `format` to `destination`.
pub(crate) fn open(destination: &Destination, format: VideoFormat, width: u32, height: u32, fps: f64) -> Result<Box<dyn FrameEncoder>> {
    let output = BufWriter::new(destination.output()?);
    match format {
        VideoFormat::Gif => Ok(Box::new(GifEncoder::new(output, width, height, fps)?)),
        VideoFormat::Mp4 | VideoFormat::Mov => {
            let flavour = if format == VideoFormat::Mov {
                super::iso::Flavour::QuickTime
            } else {
                super::iso::Flavour::Mp4
            };
            let sink = super::iso::IsoWriter::new(output, flavour, width, height, fps)?;
            Ok(Box::new(av1::Av1Encoder::new(Box::new(sink), width, height, fps)?))
        }
        VideoFormat::WebM => {
            let sink = super::webm::WebmWriter::new(output, width, height, fps)?;
            Ok(Box::new(av1::Av1Encoder::new(Box::new(sink), width, height, fps)?))
        }
    }
}

/// Where an AV1 encoder puts its frames: a container writer.
pub(crate) trait Av1Sink: Send {
    /// One temporal unit, temporal delimiters removed, in presentation
    /// order. `config` is the av1C record, given with every call so a writer
    /// that needs it up front (WebM) can write its header on the first.
    fn write(&mut self, config: &[u8], data: &[u8], keyframe: bool) -> Result<()>;
    fn finish(self: Box<Self>, config: &[u8]) -> Result<()>;
}

mod av1 {
    use rav1e::prelude::*;

    use super::*;

    /// The encoder's speed preset, 0 (slowest, smallest) to 10 (fastest).
    /// A viewport recording is mostly flat colour and slow change, which
    /// the fastest preset carries well: at 1080p it encodes half again as
    /// fast as 9 for a file 7% larger.
    const SPEED_PRESET: u8 = 10;
    /// The most tiles a frame is split into. rav1e spreads a frame's work
    /// across the rayon pool's threads by tile, so one per thread is what
    /// keeps them busy: at 1080p, eight tiles encode nearly three times as
    /// fast as rav1e's own choice, with files within a few percent of the
    /// size.
    const MAX_TILES: usize = 16;
    /// Constant quantiser, 0-255: lower is better and larger.
    const QUANTIZER: usize = 90;
    /// A keyframe at least every this many seconds, so a player can seek.
    const KEYFRAME_SECONDS: f64 = 5.0;

    pub(super) struct Av1Encoder {
        context: Context<u8>,
        sink: Option<Box<dyn Av1Sink>>,
        /// The av1C record: rav1e's four bytes, then the sequence header OBU
        /// once the first keyframe has carried one.
        config: Vec<u8>,
        width: usize,
        height: usize,
        luma: Vec<u8>,
        chroma_u: Vec<u8>,
        chroma_v: Vec<u8>,
    }

    impl Av1Encoder {
        pub(super) fn new(sink: Box<dyn Av1Sink>, width: u32, height: u32, fps: f64) -> Result<Self> {
            let (width, height) = (width as usize, height as usize);
            // A millisecond time base, so any typed frame rate is honoured
            // to three decimals.
            let time_base = Rational::new(1000, (fps * 1000.0).round().max(1.0) as u64);
            let encoder = EncoderConfig {
                width,
                height,
                time_base,
                bit_depth: 8,
                chroma_sampling: ChromaSampling::Cs420,
                pixel_range: PixelRange::Limited,
                color_description: Some(ColorDescription {
                    color_primaries: ColorPrimaries::BT709,
                    transfer_characteristics: TransferCharacteristics::BT709,
                    matrix_coefficients: MatrixCoefficients::BT709,
                }),
                low_latency: true,
                quantizer: QUANTIZER,
                max_key_frame_interval: (fps * KEYFRAME_SECONDS).round().max(1.0) as u64,
                speed_settings: SpeedSettings::from_preset(SPEED_PRESET),
                tiles: rayon::current_num_threads().clamp(1, MAX_TILES),
                ..Default::default()
            };
            let context: Context<u8> = Config::new()
                .with_encoder_config(encoder)
                .new_context()
                .map_err(|error| anyhow!("The AV1 encoder refused the settings: {error}"))?;
            let config = context.container_sequence_header();
            let chroma = width.div_ceil(2) * height.div_ceil(2);
            Ok(Self {
                context,
                sink: Some(sink),
                config,
                width,
                height,
                luma: vec![0; width * height],
                chroma_u: vec![0; chroma],
                chroma_v: vec![0; chroma],
            })
        }

        /// Hand every packet the encoder has ready to the container.
        fn drain(&mut self) -> Result<()> {
            loop {
                match self.context.receive_packet() {
                    Ok(packet) => self.write_packet(&packet.data, packet.frame_type == FrameType::KEY)?,
                    Err(EncoderStatus::Encoded) => continue,
                    Err(EncoderStatus::NeedMoreData | EncoderStatus::LimitReached) => return Ok(()),
                    Err(error) => return Err(anyhow!("AV1 encoding failed: {error}")),
                }
            }
        }

        fn write_packet(&mut self, data: &[u8], keyframe: bool) -> Result<()> {
            let (unit, sequence_header) = strip_temporal_delimiters(data)?;
            // The four-byte record leaves the sequence header to the
            // container; MP4 recommends and WebM requires it there.
            if self.config.len() == 4
                && let Some(header) = sequence_header
            {
                self.config.extend_from_slice(header);
            }
            let sink = self.sink.as_mut().expect("the sink lives until finish");
            sink.write(&self.config, &unit, keyframe)
        }
    }

    impl FrameEncoder for Av1Encoder {
        fn push(&mut self, rgba: &[u8]) -> Result<()> {
            rgba_to_yuv420(rgba, self.width, self.height, &mut self.luma, &mut self.chroma_u, &mut self.chroma_v);
            let mut frame = self.context.new_frame();
            let chroma_stride = self.width.div_ceil(2);
            frame.planes[0].copy_from_raw_u8(&self.luma, self.width, 1);
            frame.planes[1].copy_from_raw_u8(&self.chroma_u, chroma_stride, 1);
            frame.planes[2].copy_from_raw_u8(&self.chroma_v, chroma_stride, 1);
            self.context.send_frame(frame).map_err(|error| anyhow!("AV1 encoding failed: {error}"))?;
            self.drain()
        }

        fn finish(mut self: Box<Self>) -> Result<()> {
            self.context.flush();
            self.drain()?;
            let sink = self.sink.take().expect("finished once");
            sink.finish(&self.config)
        }
    }

    /// The temporal unit without its temporal delimiter OBUs, which neither
    /// container stores, and the sequence header OBU in it, if there is one.
    fn strip_temporal_delimiters(data: &[u8]) -> Result<(Vec<u8>, Option<&[u8]>)> {
        const SEQUENCE_HEADER: u8 = 1;
        const TEMPORAL_DELIMITER: u8 = 2;
        let mut out = Vec::with_capacity(data.len());
        let mut sequence_header = None;
        let mut at = 0;
        while at < data.len() {
            let header = data[at];
            let kind = (header >> 3) & 0x0F;
            let extension = header & 0x04 != 0;
            let has_size = header & 0x02 != 0;
            let mut cursor = at + 1 + usize::from(extension);
            let payload = if has_size {
                let (size, read) = leb128(&data[cursor.min(data.len())..]).ok_or_else(|| anyhow!("A malformed OBU came out of the encoder"))?;
                cursor += read;
                size
            } else {
                data.len() - cursor
            };
            let end = cursor
                .checked_add(payload)
                .filter(|end| *end <= data.len())
                .ok_or_else(|| anyhow!("A truncated OBU came out of the encoder"))?;
            match kind {
                TEMPORAL_DELIMITER => {}
                SEQUENCE_HEADER => {
                    sequence_header.get_or_insert(&data[at..end]);
                    out.extend_from_slice(&data[at..end]);
                }
                _ => out.extend_from_slice(&data[at..end]),
            }
            at = end;
        }
        Ok((out, sequence_header))
    }

    /// An unsigned LEB128 value and the bytes it took.
    fn leb128(bytes: &[u8]) -> Option<(usize, usize)> {
        let mut value = 0usize;
        for (index, byte) in bytes.iter().take(8).enumerate() {
            value |= usize::from(byte & 0x7F) << (7 * index);
            if byte & 0x80 == 0 {
                return Some((value, index + 1));
            }
        }
        None
    }

    /// Opaque sRGB RGBA to BT.709 limited-range 4:2:0, chroma averaged over
    /// each 2x2 block.
    fn rgba_to_yuv420(rgba: &[u8], width: usize, height: usize, luma: &mut [u8], u: &mut [u8], v: &mut [u8]) {
        use rayon::prelude::*;
        const KR: f32 = 0.2126;
        const KB: f32 = 0.0722;
        const KG: f32 = 1.0 - KR - KB;
        let pixel = |x: usize, y: usize| {
            let at = (y * width + x) * 4;
            (f32::from(rgba[at]) / 255.0, f32::from(rgba[at + 1]) / 255.0, f32::from(rgba[at + 2]) / 255.0)
        };
        // A row at a time across the cores: one thread spends longer on this
        // than the encoder spends on the frame.
        luma.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
            for (x, out) in row.iter_mut().enumerate() {
                let (r, g, b) = pixel(x, y);
                *out = (16.0 + 219.0 * (KR * r + KG * g + KB * b)).round().clamp(0.0, 255.0) as u8;
            }
        });
        let chroma_width = width.div_ceil(2);
        u.par_chunks_mut(chroma_width)
            .zip(v.par_chunks_mut(chroma_width))
            .enumerate()
            .for_each(|(cy, (u_row, v_row))| {
                for cx in 0..chroma_width {
                    let (mut cb, mut cr, mut count) = (0.0, 0.0, 0.0);
                    for (x, y) in [(2 * cx, 2 * cy), (2 * cx + 1, 2 * cy), (2 * cx, 2 * cy + 1), (2 * cx + 1, 2 * cy + 1)] {
                        if x < width && y < height {
                            let (r, g, b) = pixel(x, y);
                            let y_ = KR * r + KG * g + KB * b;
                            cb += (b - y_) / (2.0 * (1.0 - KB));
                            cr += (r - y_) / (2.0 * (1.0 - KR));
                            count += 1.0;
                        }
                    }
                    u_row[cx] = (128.0 + 224.0 * cb / count).round().clamp(0.0, 255.0) as u8;
                    v_row[cx] = (128.0 + 224.0 * cr / count).round().clamp(0.0, 255.0) as u8;
                }
            });
    }
}

/// The widest a GIF is written: past this the file grows far faster than
/// it reads better, so a larger viewport is scaled down by a whole factor.
const GIF_MAX_WIDTH: u32 = 960;
/// GIF counts delays in hundredths of a second, and browsers draw anything
/// under two as ten, so frames closer than this are dropped rather than
/// shown slow.
const GIF_MIN_DELAY_CS: u64 = 2;

struct GifEncoder {
    encoder: gif::Encoder<BufWriter<Box<dyn VideoOutput>>>,
    width: u32,
    height: u32,
    factor: u32,
    fps: f64,
    /// Frames pushed so far.
    count: u64,
    /// A frame waits until the next is kept, which is when its delay is
    /// known: (its start in centiseconds, its pixels).
    held: Option<(u64, Vec<u8>)>,
}

impl GifEncoder {
    fn new(file: BufWriter<Box<dyn VideoOutput>>, width: u32, height: u32, fps: f64) -> Result<Self> {
        let factor = width.div_ceil(GIF_MAX_WIDTH).max(1);
        let (out_width, out_height) = ((width / factor).max(1), (height / factor).max(1));
        let size = |value: u32| u16::try_from(value).map_err(|_| anyhow!("The viewport is too large for a GIF"));
        let mut encoder = gif::Encoder::new(file, size(out_width)?, size(out_height)?, &[])?;
        encoder.set_repeat(gif::Repeat::Infinite)?;
        Ok(Self {
            encoder,
            width,
            height,
            factor,
            fps,
            count: 0,
            held: None,
        })
    }

    fn write_held(&mut self, until_cs: u64) -> Result<()> {
        if let Some((start, mut pixels)) = self.held.take() {
            let (width, height) = ((self.width / self.factor).max(1) as u16, (self.height / self.factor).max(1) as u16);
            let mut frame = gif::Frame::from_rgba_speed(width, height, &mut pixels, 10);
            frame.delay = (until_cs.saturating_sub(start)).max(GIF_MIN_DELAY_CS).min(u64::from(u16::MAX)) as u16;
            self.encoder.write_frame(&frame)?;
        }
        Ok(())
    }

    fn scaled(&self, rgba: &[u8]) -> Vec<u8> {
        downscale(rgba, self.width, (self.width / self.factor).max(1), (self.height / self.factor).max(1), self.factor)
    }
}

/// `rgba`, `width` pixels wide, shrunk by a whole `factor` to `out_width` x
/// `out_height` (no more than it divides into), each output pixel the
/// average of the block it covers.
pub(crate) fn downscale(rgba: &[u8], width: u32, out_width: u32, out_height: u32, factor: u32) -> Vec<u8> {
    let (width, out_width, out_height, factor) = (width as usize, out_width as usize, out_height as usize, factor.max(1) as usize);
    let mut out = Vec::with_capacity(out_width * out_height * 4);
    for oy in 0..out_height {
        for ox in 0..out_width {
            let mut sum = [0u32; 3];
            for y in oy * factor..(oy + 1) * factor {
                for x in ox * factor..(ox + 1) * factor {
                    let at = (y * width + x) * 4;
                    for channel in 0..3 {
                        sum[channel] += u32::from(rgba[at + channel]);
                    }
                }
            }
            let area = (factor * factor) as u32;
            out.extend_from_slice(&[(sum[0] / area) as u8, (sum[1] / area) as u8, (sum[2] / area) as u8, 255]);
        }
    }
    out
}

impl FrameEncoder for GifEncoder {
    fn push(&mut self, rgba: &[u8]) -> Result<()> {
        let start = (self.count as f64 * 100.0 / self.fps).round() as u64;
        self.count += 1;
        if let Some((held_start, _)) = &self.held
            && start < held_start + GIF_MIN_DELAY_CS
        {
            return Ok(());
        }
        self.write_held(start)?;
        self.held = Some((start, self.scaled(rgba)));
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> Result<()> {
        let end = (self.count as f64 * 100.0 / self.fps).round() as u64;
        self.write_held(end)?;
        let mut file = self.encoder.into_inner()?;
        file.flush()?;
        Ok(())
    }
}
