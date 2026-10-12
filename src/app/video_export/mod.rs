//! Animate's video export: the schedule played through the viewport, frame by
//! frame, into a video file.
//!
//! Recording does not run in real time. Each frame moves the schedule's time
//! slider to the next instant, waits until Animate has cut the ground, placed
//! the machines and drawn them for exactly that instant, and only then copies
//! the finished viewport - the scene and the labels and icons drawn over it -
//! back from the GPU. The pixels go to an encoder running as a background
//! job, so a slow encode holds the recording back rather than the window.

mod encode;
#[cfg(feature = "video-av1")]
mod iso;
#[cfg(feature = "video-av1")]
mod webm;

use std::{path::PathBuf, sync::mpsc};

pub(crate) use encode::VideoFormat;

use super::{App, jobs::JobKey};
use crate::{i18n::tr, ui::state::VideoExportRequest, userspace_log, userspace_warn};

/// Frames queued for the encoder before capture waits for it.
const QUEUED_FRAMES: usize = 3;

/// A recording in progress.
pub(crate) struct Recording {
    /// The schedule instant of every frame, in hours.
    times: Vec<f64>,
    /// The next frame to capture.
    next: usize,
    /// The viewport region copied back each frame, in physical pixels.
    width: u32,
    height: u32,
    /// Frames to the encoder; dropped to tell it the last has been sent.
    frames: mpsc::SyncSender<Vec<u8>>,
    /// Whether this frame's render was asked to copy the viewport back.
    capture_requested: bool,
    /// Where the slider was, to put it back afterwards.
    restore_time_h: f64,
}

impl<'a> App<'a> {
    /// Ask where to save the video, then record it.
    pub(crate) fn choose_video_export(&mut self, request: VideoExportRequest) {
        use rfd::AsyncFileDialog;
        let formats = VideoFormat::available();
        self.spawn_file_dialog(async move {
            let mut dialog = AsyncFileDialog::new().set_file_name(format!("{}.{}", tr!("video-export-file-name"), formats[0].extension()));
            for format in &formats {
                dialog = dialog.add_filter(format.filter_name(), &[format.extension()]);
            }
            let path = dialog.save_file().await?.path().to_path_buf();
            Some(super::commands::file::FileDialogAction::ExportVideo(path, request))
        });
    }

    /// Start recording `request` into `path`.
    pub(crate) fn start_video_export(&mut self, mut path: PathBuf, request: VideoExportRequest) -> anyhow::Result<()> {
        use anyhow::{Context as _, bail};

        if self.video_recording.is_some() {
            bail!("{}", tr!("video-export-already-recording"));
        }
        if !self.editor.is_schedule_animation() || !self.editor.schedule_animation_enabled {
            bail!("{}", tr!("video-export-needs-animate"));
        }
        let format = match VideoFormat::of_path(&path) {
            Some(format) => format,
            None => {
                // A name typed without a known extension is saved as the
                // first format offered, and says so in its name.
                let format = VideoFormat::available()[0];
                let mut name = path.file_name().map(|name| name.to_os_string()).unwrap_or_default();
                name.push(".");
                name.push(format.extension());
                path.set_file_name(name);
                format
            }
        };
        let graphics = self.graphics.as_ref().context("The renderer is not initialised yet")?;
        if !graphics.can_capture_video() {
            bail!("{}", tr!("video-export-unsupported-display"));
        }
        // Encoders work on whole 2x2 blocks of chroma, so an odd edge is
        // trimmed rather than padded.
        let (viewport_width, viewport_height) = graphics.viewport_size();
        let (width, height) = (viewport_width & !1, viewport_height & !1);
        let scale = request.scale.max(1);
        // Shrunk by the factor, then trimmed to even again.
        let (video_width, video_height) = ((width / scale) & !1, (height / scale) & !1);
        if video_width < 16 || video_height < 16 {
            bail!("{}", tr!("video-export-viewport-too-small"));
        }
        let times = request.frame_times();
        if times.is_empty() {
            bail!("{}", tr!("video-export-nothing-selected"));
        }

        let total = times.len();
        let fps = request.fps;
        let (sender, receiver) = mpsc::sync_channel::<Vec<u8>>(QUEUED_FRAMES);
        let target = path.clone();
        let label = tr!(
            "video-export-job",
            file = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
        );
        self.spawn_job_reporting_progress(
            label,
            vec![JobKey::VideoExport],
            move |cancel, progress| {
                let result = (|| {
                    let mut encoder = encode::open(&target, format, video_width, video_height, fps)?;
                    let mut done = 0u64;
                    for frame in receiver {
                        if cancel.is_cancelled() {
                            bail!("{}", tr!("video-export-cancelled"));
                        }
                        if scale == 1 {
                            encoder.push(&frame)?;
                        } else {
                            encoder.push(&encode::downscale(&frame, width, video_width, video_height, scale))?;
                        }
                        done += 1;
                        progress.set_items(done, total as u64);
                    }
                    if cancel.is_cancelled() || done < total as u64 {
                        bail!("{}", tr!("video-export-cancelled"));
                    }
                    encoder.finish()?;
                    Ok(target.clone())
                })();
                if result.is_err() {
                    encode::discard(&target);
                }
                result
            },
            move |app, result: anyhow::Result<PathBuf>| match result {
                Ok(path) => userspace_log!("{}", tr!("video-export-saved", path = path.display().to_string())),
                Err(error) => {
                    // An encoder that failed leaves the capture with nowhere
                    // to send its frames.
                    app.stop_video_recording();
                    userspace_warn!("{}", tr!("video-export-failed", error = format!("{error:#}")));
                }
            },
        );

        self.video_recording = Some(Recording {
            times,
            next: 0,
            width,
            height,
            frames: sender,
            capture_requested: false,
            restore_time_h: self.editor.schedule_time_h,
        });
        self.editor.video_export_dialog = None;
        self.editor.schedule_animation_playing = false;
        self.editor.video_export_progress = Some((0, total));
        self.redraw_requested = true;
        Ok(())
    }

    /// Move the slider to the frame being recorded, and once Animate shows
    /// exactly that instant, have this frame's render copy the viewport.
    /// Runs after Animate's own sync, before the render.
    pub(crate) fn drive_video_recording(&mut self) {
        let Some(recording) = self.video_recording.as_ref() else {
            return;
        };
        let (target, width, height) = (recording.times[recording.next], recording.width, recording.height);
        self.redraw_requested = true;
        self.editor.schedule_animation_playing = false;
        if !self.editor.is_schedule_animation() || !self.editor.schedule_animation_enabled {
            userspace_warn!("{}", tr!("video-export-left-animate"));
            self.cancel_video_export();
            return;
        }
        if self.editor.schedule_time_h != target {
            self.editor.schedule_time_h = target;
            return;
        }
        if !self.animation_shows(target) {
            return;
        }
        if let Some(recording) = self.video_recording.as_mut() {
            recording.capture_requested = true;
        }
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.request_video_frame(width, height);
        }
    }

    /// Hand the frame this render copied back to the encoder. Runs after the
    /// render.
    pub(crate) fn collect_video_frame(&mut self) {
        let Some(recording) = self.video_recording.as_mut() else {
            return;
        };
        if !std::mem::take(&mut recording.capture_requested) {
            return;
        }
        let Some(capture) = self.graphics.as_mut().and_then(|graphics| graphics.take_video_frame()) else {
            return;
        };
        let Some(pixels) = capture else {
            userspace_warn!("{}", tr!("video-export-viewport-resized"));
            self.cancel_video_export();
            return;
        };
        if recording.frames.send(pixels).is_err() {
            // The encoder has stopped; its job reports why.
            self.stop_video_recording();
            return;
        }
        recording.next += 1;
        let total = recording.times.len();
        self.editor.video_export_progress = Some((recording.next, total));
        if recording.next == total {
            // Dropping the sender is what tells the encoder it has them all.
            self.stop_video_recording();
        }
    }

    /// Stop capturing and put the slider back. The encoder finishes or
    /// fails on its own.
    fn stop_video_recording(&mut self) {
        if let Some(recording) = self.video_recording.take() {
            self.editor.schedule_time_h = recording.restore_time_h;
        }
        self.editor.video_export_progress = None;
    }

    /// Abandon the recording: the encoder stops and removes its file.
    pub(crate) fn cancel_video_export(&mut self) {
        self.cancel_jobs(|key| matches!(key, JobKey::VideoExport));
        if self.video_recording.is_some() {
            userspace_log!("{}", tr!("video-export-cancelled"));
        }
        self.stop_video_recording();
    }
}
