//! A minimal WebM (Matroska) writer for one AV1 video track.
//!
//! The header waits for the first frame, because AV1 in Matroska carries the
//! sequence header in the track's CodecPrivate. Blocks are gathered into a
//! cluster in memory and written when a keyframe starts the next one (or the
//! cluster's 16-bit relative timestamps would run out); the cues, the
//! duration and the segment size are filled in when the file is finished.

use std::io::{BufWriter, Seek, SeekFrom, Write};

use anyhow::Result;

use super::encode::{Av1Sink, VideoOutput};

const EBML: u32 = 0x1A45_DFA3;
const EBML_VERSION: u32 = 0x4286;
const EBML_READ_VERSION: u32 = 0x42F7;
const EBML_MAX_ID_LENGTH: u32 = 0x42F2;
const EBML_MAX_SIZE_LENGTH: u32 = 0x42F3;
const DOC_TYPE: u32 = 0x4282;
const DOC_TYPE_VERSION: u32 = 0x4287;
const DOC_TYPE_READ_VERSION: u32 = 0x4285;
const SEGMENT: u32 = 0x1853_8067;
const SEEK_HEAD: u32 = 0x114D_9B74;
const SEEK: u32 = 0x4DBB;
const SEEK_ID: u32 = 0x53AB;
const SEEK_POSITION: u32 = 0x53AC;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2A_D7B1;
const DURATION: u32 = 0x4489;
const MUXING_APP: u32 = 0x4D80;
const WRITING_APP: u32 = 0x5741;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_NUMBER: u32 = 0xD7;
const TRACK_UID: u32 = 0x73C5;
const TRACK_TYPE: u32 = 0x83;
const FLAG_LACING: u32 = 0x9C;
const CODEC_ID: u32 = 0x86;
const CODEC_PRIVATE: u32 = 0x63A2;
const DEFAULT_DURATION: u32 = 0x23_E383;
const VIDEO: u32 = 0xE0;
const PIXEL_WIDTH: u32 = 0xB0;
const PIXEL_HEIGHT: u32 = 0xBA;
const CLUSTER: u32 = 0x1F43_B675;
const TIMESTAMP: u32 = 0xE7;
const SIMPLE_BLOCK: u32 = 0xA3;
const CUES: u32 = 0x1C53_BB6B;
const CUE_POINT: u32 = 0xBB;
const CUE_TIME: u32 = 0xB3;
const CUE_TRACK_POSITIONS: u32 = 0xB7;
const CUE_TRACK: u32 = 0xF7;
const CUE_CLUSTER_POSITION: u32 = 0xF1;

/// Timestamps count milliseconds.
const TIMESTAMP_SCALE_NS: u64 = 1_000_000;
/// A block's timestamp is a signed 16-bit offset from its cluster's.
const MAX_CLUSTER_SPAN_MS: u64 = 30_000;

pub(crate) struct WebmWriter {
    file: BufWriter<Box<dyn VideoOutput>>,
    width: u32,
    height: u32,
    fps: f64,
    /// Where the segment's data starts; positions inside it count from here.
    segment_data: u64,
    /// Where the Duration float and the Cues seek position are patched.
    duration_at: u64,
    cues_seek_at: u64,
    position: u64,
    started: bool,
    frames: u64,
    cluster: Option<(u64, Vec<u8>)>,
    /// (cluster timestamp, cluster position in the segment) of each cluster
    /// that opens on a keyframe.
    cues: Vec<(u64, u64)>,
}

impl WebmWriter {
    pub(crate) fn new(file: BufWriter<Box<dyn VideoOutput>>, width: u32, height: u32, fps: f64) -> Result<Self> {
        Ok(Self {
            file,
            width,
            height,
            fps,
            segment_data: 0,
            duration_at: 0,
            cues_seek_at: 0,
            position: 0,
            started: false,
            frames: 0,
            cluster: None,
            cues: Vec::new(),
        })
    }

    fn emit(&mut self, bytes: &[u8]) -> Result<()> {
        self.file.write_all(bytes)?;
        self.position += bytes.len() as u64;
        Ok(())
    }

    fn start(&mut self, config: &[u8]) -> Result<()> {
        let header = element(
            EBML,
            &[
                uint(EBML_VERSION, 1),
                uint(EBML_READ_VERSION, 1),
                uint(EBML_MAX_ID_LENGTH, 4),
                uint(EBML_MAX_SIZE_LENGTH, 8),
                string(DOC_TYPE, "webm"),
                uint(DOC_TYPE_VERSION, 4),
                uint(DOC_TYPE_READ_VERSION, 2),
            ]
            .concat(),
        );
        self.emit(&header)?;
        // The segment's size is unknown until the end: eight bytes held for it.
        self.emit(&id_bytes(SEGMENT))?;
        self.emit(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])?;
        self.segment_data = self.position;

        let info = element(
            INFO,
            &[
                uint(TIMESTAMP_SCALE, TIMESTAMP_SCALE_NS),
                string(MUXING_APP, crate::APP_NAME),
                string(WRITING_APP, crate::APP_NAME),
                [id_bytes(DURATION), vec![0x88], 0f64.to_be_bytes().to_vec()].concat(),
            ]
            .concat(),
        );
        let tracks = element(
            TRACKS,
            &element(
                TRACK_ENTRY,
                &[
                    uint(TRACK_NUMBER, 1),
                    uint(TRACK_UID, 1),
                    uint(TRACK_TYPE, 1),
                    uint(FLAG_LACING, 0),
                    string(CODEC_ID, "V_AV1"),
                    element(CODEC_PRIVATE, config),
                    uint(DEFAULT_DURATION, (1.0e9 / self.fps).round() as u64),
                    element(VIDEO, &[uint(PIXEL_WIDTH, u64::from(self.width)), uint(PIXEL_HEIGHT, u64::from(self.height))].concat()),
                ]
                .concat(),
            ),
        );

        // The seek head names Info, Tracks and Cues by their place in the
        // segment, each position a fixed eight bytes so Cues' can be patched.
        let seek = |id: u32, at: u64| {
            element(
                SEEK,
                &[element(SEEK_ID, &id_bytes(id)), [id_bytes(SEEK_POSITION), vec![0x88], at.to_be_bytes().to_vec()].concat()].concat(),
            )
        };
        let probe = element(SEEK_HEAD, &[seek(INFO, 0), seek(TRACKS, 0), seek(CUES, 0)].concat());
        let info_at = probe.len() as u64;
        let tracks_at = info_at + info.len() as u64;
        let seek_head = element(SEEK_HEAD, &[seek(INFO, info_at), seek(TRACKS, tracks_at), seek(CUES, 0)].concat());
        // The last eight bytes of the seek head are Cues' position.
        self.cues_seek_at = self.position + seek_head.len() as u64 - 8;
        self.emit(&seek_head)?;
        // Duration's eight bytes end the Info element.
        self.duration_at = self.position + info.len() as u64 - 8;
        self.emit(&info)?;
        self.emit(&tracks)?;
        self.started = true;
        Ok(())
    }

    fn flush_cluster(&mut self) -> Result<()> {
        if let Some((timestamp, blocks)) = self.cluster.take() {
            let cluster = element(CLUSTER, &[uint(TIMESTAMP, timestamp), blocks].concat());
            self.emit(&cluster)?;
        }
        Ok(())
    }

    fn millis(&self, frame: u64) -> u64 {
        (frame as f64 * 1000.0 / self.fps).round() as u64
    }
}

impl Av1Sink for WebmWriter {
    fn write(&mut self, config: &[u8], data: &[u8], keyframe: bool) -> Result<()> {
        if !self.started {
            self.start(config)?;
        }
        let at = self.millis(self.frames);
        self.frames += 1;
        let overflow = self.cluster.as_ref().is_some_and(|(start, _)| at - start > MAX_CLUSTER_SPAN_MS);
        if keyframe || overflow || self.cluster.is_none() {
            self.flush_cluster()?;
            if keyframe {
                self.cues.push((at, self.position - self.segment_data));
            }
            self.cluster = Some((at, Vec::new()));
        }
        let (start, blocks) = self.cluster.as_mut().expect("opened above");
        let relative = (at - *start) as i16;
        let mut block = vec![0x81];
        block.extend_from_slice(&relative.to_be_bytes());
        block.push(if keyframe { 0x80 } else { 0x00 });
        block.extend_from_slice(data);
        blocks.extend(element(SIMPLE_BLOCK, &block));
        Ok(())
    }

    fn finish(mut self: Box<Self>, config: &[u8]) -> Result<()> {
        if !self.started {
            self.start(config)?;
        }
        self.flush_cluster()?;
        let cues_at = self.position - self.segment_data;
        let points: Vec<u8> = self
            .cues
            .iter()
            .flat_map(|(time, cluster)| {
                element(
                    CUE_POINT,
                    &[
                        uint(CUE_TIME, *time),
                        element(CUE_TRACK_POSITIONS, &[uint(CUE_TRACK, 1), uint(CUE_CLUSTER_POSITION, *cluster)].concat()),
                    ]
                    .concat(),
                )
            })
            .collect();
        if !points.is_empty() {
            self.emit(&element(CUES, &points))?;
        }
        let segment_size = self.position - self.segment_data;
        let duration = self.millis(self.frames) as f64;
        self.file.seek(SeekFrom::Start(self.segment_data - 8))?;
        self.file.write_all(&size_bytes8(segment_size))?;
        self.file.seek(SeekFrom::Start(self.duration_at))?;
        self.file.write_all(&duration.to_be_bytes())?;
        if !points.is_empty() {
            self.file.seek(SeekFrom::Start(self.cues_seek_at))?;
            self.file.write_all(&cues_at.to_be_bytes())?;
        }
        self.file.flush()?;
        Ok(())
    }
}

/// An element ID's bytes: IDs carry their own length marker.
fn id_bytes(id: u32) -> Vec<u8> {
    let bytes = id.to_be_bytes();
    let skip = bytes.iter().position(|byte| *byte != 0).unwrap_or(3);
    bytes[skip..].to_vec()
}

/// A size as the shortest variable-length integer that holds it.
fn size_bytes(size: u64) -> Vec<u8> {
    for length in 1..=8u32 {
        // All ones at a length is reserved for "unknown".
        if size < (1u64 << (7 * length)) - 1 {
            let marked = size | (1u64 << (7 * length));
            return marked.to_be_bytes()[8 - length as usize..].to_vec();
        }
    }
    size_bytes8(size).to_vec()
}

/// A size written in the full eight bytes, for one patched later.
fn size_bytes8(size: u64) -> [u8; 8] {
    (size | (1u64 << 56)).to_be_bytes()
}

fn element(id: u32, body: &[u8]) -> Vec<u8> {
    [id_bytes(id), size_bytes(body.len() as u64), body.to_vec()].concat()
}

fn uint(id: u32, value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let skip = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
    element(id, &bytes[skip..])
}

fn string(id: u32, value: &str) -> Vec<u8> {
    element(id, value.as_bytes())
}
