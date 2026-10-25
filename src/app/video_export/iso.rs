//! A minimal ISO base media file writer for one AV1 video track: MP4, or
//! the same boxes branded as a QuickTime movie.
//!
//! Samples are streamed into `mdat` as they arrive and the index (`moov`)
//! follows it, so memory stays flat however long the recording. Every
//! sample is its own chunk at a constant duration, which keeps the sample
//! tables to a line each apart from sizes, offsets and keyframes.

use std::io::{BufWriter, Seek, SeekFrom, Write};

use anyhow::Result;

use super::encode::{Av1Sink, VideoOutput};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flavour {
    Mp4,
    QuickTime,
}

/// Ticks per frame in the media time scale; the scale is the frame rate
/// times this, so a typed rate keeps three decimals.
const TICKS_PER_FRAME: u32 = 1000;
/// The movie header's time scale.
const MOVIE_TIMESCALE: u32 = 1000;

pub(crate) struct IsoWriter {
    file: BufWriter<Box<dyn VideoOutput>>,
    flavour: Flavour,
    width: u32,
    height: u32,
    timescale: u32,
    /// Where `mdat`'s 64-bit size is patched once the samples are in.
    mdat_start: u64,
    position: u64,
    sizes: Vec<u32>,
    offsets: Vec<u64>,
    /// 1-based numbers of the samples that are keyframes.
    keyframes: Vec<u32>,
}

impl IsoWriter {
    pub(crate) fn new(mut file: BufWriter<Box<dyn VideoOutput>>, flavour: Flavour, width: u32, height: u32, fps: f64) -> Result<Self> {
        let ftyp = match flavour {
            Flavour::Mp4 => plain(b"ftyp", |body| {
                body.extend_from_slice(b"isom");
                body.extend_from_slice(&0x200u32.to_be_bytes());
                for brand in [b"isom", b"iso6", b"av01", b"mp41"] {
                    body.extend_from_slice(brand);
                }
            }),
            Flavour::QuickTime => plain(b"ftyp", |body| {
                body.extend_from_slice(b"qt  ");
                body.extend_from_slice(&0x200u32.to_be_bytes());
                body.extend_from_slice(b"qt  ");
            }),
        };
        file.write_all(&ftyp)?;
        let mdat_start = ftyp.len() as u64;
        // A 64-bit size, so a long recording past 4 GiB still indexes.
        file.write_all(&1u32.to_be_bytes())?;
        file.write_all(b"mdat")?;
        file.write_all(&0u64.to_be_bytes())?;
        Ok(Self {
            file,
            flavour,
            width,
            height,
            timescale: (fps * f64::from(TICKS_PER_FRAME)).round().max(1.0) as u32,
            mdat_start,
            position: mdat_start + 16,
            sizes: Vec::new(),
            offsets: Vec::new(),
            keyframes: Vec::new(),
        })
    }

    fn moov(&self, config: &[u8]) -> Vec<u8> {
        let samples = self.sizes.len() as u64;
        let media_duration = samples * u64::from(TICKS_PER_FRAME);
        let movie_duration = media_duration * u64::from(MOVIE_TIMESCALE) / u64::from(self.timescale);
        let (width, height) = (self.width, self.height);
        let quicktime = self.flavour == Flavour::QuickTime;

        let mvhd = full(b"mvhd", 1, 0, |b| {
            b.extend_from_slice(&0u64.to_be_bytes()); // creation
            b.extend_from_slice(&0u64.to_be_bytes()); // modification
            b.extend_from_slice(&MOVIE_TIMESCALE.to_be_bytes());
            b.extend_from_slice(&movie_duration.to_be_bytes());
            b.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // rate 1.0
            b.extend_from_slice(&0x0100u16.to_be_bytes()); // volume 1.0
            b.extend_from_slice(&[0; 10]);
            matrix(b);
            b.extend_from_slice(&[0; 24]); // pre-defined
            b.extend_from_slice(&2u32.to_be_bytes()); // next track id
        });
        let tkhd = full(b"tkhd", 1, 0x3, |b| {
            b.extend_from_slice(&0u64.to_be_bytes());
            b.extend_from_slice(&0u64.to_be_bytes());
            b.extend_from_slice(&1u32.to_be_bytes()); // track id
            b.extend_from_slice(&0u32.to_be_bytes());
            b.extend_from_slice(&movie_duration.to_be_bytes());
            b.extend_from_slice(&[0; 8]);
            b.extend_from_slice(&0u16.to_be_bytes()); // layer
            b.extend_from_slice(&0u16.to_be_bytes()); // alternate group
            b.extend_from_slice(&0u16.to_be_bytes()); // volume
            b.extend_from_slice(&0u16.to_be_bytes());
            matrix(b);
            b.extend_from_slice(&(width << 16).to_be_bytes());
            b.extend_from_slice(&(height << 16).to_be_bytes());
        });
        let mdhd = full(b"mdhd", 1, 0, |b| {
            b.extend_from_slice(&0u64.to_be_bytes());
            b.extend_from_slice(&0u64.to_be_bytes());
            b.extend_from_slice(&self.timescale.to_be_bytes());
            b.extend_from_slice(&media_duration.to_be_bytes());
            b.extend_from_slice(&0x55C4u16.to_be_bytes()); // "und"
            b.extend_from_slice(&0u16.to_be_bytes());
        });
        let hdlr = full(b"hdlr", 0, 0, |b| {
            b.extend_from_slice(if quicktime { b"mhlr" } else { &[0; 4] });
            b.extend_from_slice(b"vide");
            b.extend_from_slice(&[0; 12]);
            b.extend_from_slice(b"VideoHandler\0");
        });
        let vmhd = full(b"vmhd", 0, 1, |b| b.extend_from_slice(&[0; 8]));
        let dinf = plain(b"dinf", |b| {
            b.extend(full(b"dref", 0, 0, |b| {
                b.extend_from_slice(&1u32.to_be_bytes());
                b.extend(full(b"url ", 0, 1, |_| {}));
            }));
        });
        let av01 = plain(b"av01", |b| {
            b.extend_from_slice(&[0; 6]);
            b.extend_from_slice(&1u16.to_be_bytes()); // data reference index
            b.extend_from_slice(&[0; 16]);
            b.extend_from_slice(&(width as u16).to_be_bytes());
            b.extend_from_slice(&(height as u16).to_be_bytes());
            b.extend_from_slice(&0x0048_0000u32.to_be_bytes()); // 72 dpi
            b.extend_from_slice(&0x0048_0000u32.to_be_bytes());
            b.extend_from_slice(&0u32.to_be_bytes());
            b.extend_from_slice(&1u16.to_be_bytes()); // frame count
            let mut name = [0u8; 32];
            let label = b"AOM Coding";
            name[0] = label.len() as u8;
            name[1..=label.len()].copy_from_slice(label);
            b.extend_from_slice(&name);
            b.extend_from_slice(&0x0018u16.to_be_bytes()); // depth
            b.extend_from_slice(&0xFFFFu16.to_be_bytes());
            b.extend(plain(b"av1C", |b| b.extend_from_slice(config)));
            // BT.709 in limited range, as the encoder tags the stream.
            b.extend(plain(b"colr", |b| {
                b.extend_from_slice(if quicktime { b"nclc" } else { b"nclx" });
                for code in [1u16, 1, 1] {
                    b.extend_from_slice(&code.to_be_bytes());
                }
                if !quicktime {
                    b.push(0);
                }
            }));
        });
        let stsd = full(b"stsd", 0, 0, |b| {
            b.extend_from_slice(&1u32.to_be_bytes());
            b.extend(av01);
        });
        let stts = full(b"stts", 0, 0, |b| {
            b.extend_from_slice(&1u32.to_be_bytes());
            b.extend_from_slice(&(samples as u32).to_be_bytes());
            b.extend_from_slice(&TICKS_PER_FRAME.to_be_bytes());
        });
        let stss = full(b"stss", 0, 0, |b| {
            b.extend_from_slice(&(self.keyframes.len() as u32).to_be_bytes());
            for keyframe in &self.keyframes {
                b.extend_from_slice(&keyframe.to_be_bytes());
            }
        });
        let stsc = full(b"stsc", 0, 0, |b| {
            b.extend_from_slice(&1u32.to_be_bytes());
            for value in [1u32, 1, 1] {
                b.extend_from_slice(&value.to_be_bytes());
            }
        });
        let stsz = full(b"stsz", 0, 0, |b| {
            b.extend_from_slice(&0u32.to_be_bytes());
            b.extend_from_slice(&(samples as u32).to_be_bytes());
            for size in &self.sizes {
                b.extend_from_slice(&size.to_be_bytes());
            }
        });
        let co64 = full(b"co64", 0, 0, |b| {
            b.extend_from_slice(&(samples as u32).to_be_bytes());
            for offset in &self.offsets {
                b.extend_from_slice(&offset.to_be_bytes());
            }
        });
        let stbl = plain(b"stbl", |b| {
            for part in [stsd, stts, stss, stsc, stsz, co64] {
                b.extend(part);
            }
        });
        let minf = plain(b"minf", |b| {
            for part in [vmhd, dinf, stbl] {
                b.extend(part);
            }
        });
        let mdia = plain(b"mdia", |b| {
            for part in [mdhd, hdlr, minf] {
                b.extend(part);
            }
        });
        let trak = plain(b"trak", |b| {
            b.extend(tkhd);
            b.extend(mdia);
        });
        plain(b"moov", |b| {
            b.extend(mvhd);
            b.extend(trak);
        })
    }
}

impl Av1Sink for IsoWriter {
    fn write(&mut self, _config: &[u8], data: &[u8], keyframe: bool) -> Result<()> {
        self.file.write_all(data)?;
        self.offsets.push(self.position);
        self.sizes.push(data.len() as u32);
        if keyframe {
            self.keyframes.push(self.sizes.len() as u32);
        }
        self.position += data.len() as u64;
        Ok(())
    }

    fn finish(mut self: Box<Self>, config: &[u8]) -> Result<()> {
        let mdat_size = self.position - self.mdat_start;
        let moov = self.moov(config);
        self.file.write_all(&moov)?;
        self.file.seek(SeekFrom::Start(self.mdat_start + 8))?;
        self.file.write_all(&mdat_size.to_be_bytes())?;
        self.file.flush()?;
        Ok(())
    }
}

/// A box: its size, its type, then what `body` writes.
fn plain(kind: &[u8; 4], body: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut out = vec![0; 4];
    out.extend_from_slice(kind);
    body(&mut out);
    let size = out.len() as u32;
    out[..4].copy_from_slice(&size.to_be_bytes());
    out
}

/// A full box: a box whose body starts with a version and flags.
fn full(kind: &[u8; 4], version: u8, flags: u32, body: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    plain(kind, |out| {
        out.push(version);
        out.extend_from_slice(&flags.to_be_bytes()[1..]);
        body(out);
    })
}

/// The identity transform.
fn matrix(out: &mut Vec<u8>) {
    for value in [0x0001_0000u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
        out.extend_from_slice(&value.to_be_bytes());
    }
}
