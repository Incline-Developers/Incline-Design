//! Animate's video export: the schedule played through, frame by frame, into
//! a video file.
//!
//! Recording does not run in real time. Each frame moves the schedule's time
//! slider to the next instant and waits until Animate has cut the ground and
//! placed the machines for exactly that instant. The window's own frame then
//! uploads that geometry, and right after it the video's frame is drawn
//! offscreen at the video's size from the viewport's camera (see
//! `rendering/graphics/video_capture.rs`). Its pixels arrive a frame or two
//! later and go to an encoder running as a background job.

mod encode;
#[cfg(feature = "video-av1")]
mod iso;
#[cfg(feature = "video-av1")]
mod webm;

use std::{path::PathBuf, sync::mpsc};

pub(crate) use encode::VideoFormat;

use super::{App, jobs::JobKey};
use crate::{i18n::tr, ui::state::VideoExportRequest, userspace_log, userspace_warn};

/// Frames queued for the encoder before collecting them waits for it.
const QUEUED_FRAMES: usize = 4;

/// A recording in progress.
pub(crate) struct Recording {
    /// The schedule instant of every frame, in hours.
    times: Vec<f64>,
    /// The next frame to render.
    next: usize,
    /// Frames whose pixels have reached the encoder.
    collected: usize,
    width: u32,
    height: u32,
    fps: f64,
    /// Frames to the encoder; dropped to tell it the last has been sent.
    frames: mpsc::SyncSender<Vec<u8>>,
    /// Whether Animate shows the next frame's instant, so the video frame is
    /// drawn after this frame's window render.
    render_requested: bool,
    /// Where the slider was, to put it back afterwards.
    restore_time_h: f64,
}

impl Recording {
    /// The frame to draw after this window frame, if one is due: its size
    /// and its time into the video in seconds.
    /// The instant the next frame shows, once this one is drawn: the slider
    /// moves there straight away, so the next frame's ground is cut while
    /// this one is still being encoded.
    pub(crate) fn upcoming_time(&self) -> Option<f64> {
        self.times.get(self.next).copied()
    }

    pub(crate) fn take_render_request(&mut self) -> Option<((u32, u32), f64)> {
        std::mem::take(&mut self.render_requested).then(|| {
            let clock = self.next as f64 / self.fps;
            self.next += 1;
            ((self.width, self.height), clock)
        })
    }
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
        use anyhow::bail;

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
        if self.graphics.is_none() {
            bail!("The renderer is not initialised yet");
        }
        // Encoders work on whole 2x2 blocks of chroma.
        let (width, height) = (request.width & !1, request.height & !1);
        if width < 16 || height < 16 {
            bail!("{}", tr!("video-export-too-small"));
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
                    let mut encoder = encode::open(&target, format, width, height, fps)?;
                    let mut done = 0u64;
                    for frame in receiver {
                        if cancel.is_cancelled() {
                            bail!("{}", tr!("video-export-cancelled"));
                        }
                        encoder.push(&frame)?;
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
            collected: 0,
            width,
            height,
            fps,
            frames: sender,
            render_requested: false,
            restore_time_h: self.editor.schedule_time_h,
        });
        self.editor.video_export_frame = Some((width, height));
        self.editor.video_export_dialog = None;
        self.editor.schedule_animation_playing = false;
        self.editor.video_export_progress = Some((0, total));
        self.redraw_requested = true;
        Ok(())
    }

    /// Move the slider to the next frame to record, and once Animate shows
    /// exactly that instant, have the video frame drawn after this frame's
    /// window render. Runs after Animate's own sync, before the render.
    pub(crate) fn drive_video_recording(&mut self) {
        let Some(recording) = self.video_recording.as_ref() else {
            return;
        };
        self.redraw_requested = true;
        self.editor.schedule_animation_playing = false;
        if !self.editor.is_schedule_animation() || !self.editor.schedule_animation_enabled {
            userspace_warn!("{}", tr!("video-export-left-animate"));
            self.cancel_video_export();
            return;
        }
        // Every frame drawn: wait for the last copies to arrive.
        let Some(&target) = recording.times.get(recording.next) else {
            return;
        };
        if self.editor.schedule_time_h != target {
            self.editor.schedule_time_h = target;
            return;
        }
        if self.animation_shows(target)
            && let Some(recording) = self.video_recording.as_mut()
        {
            recording.render_requested = true;
        }
    }

    /// Hand the frames whose pixels have arrived to the encoder, and finish
    /// once the last has. Runs after the render.
    pub(crate) fn collect_video_frames(&mut self) {
        let Some(recording) = self.video_recording.as_ref() else {
            return;
        };
        let rendered_all = recording.next == recording.times.len();
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let frames = match graphics.take_video_frames(rendered_all) {
            Ok(frames) => frames,
            Err(error) => {
                userspace_warn!("{}", tr!("video-export-failed", error = format!("{error:#}")));
                self.cancel_video_export();
                return;
            }
        };
        let recording = self.video_recording.as_mut().expect("checked above");
        for frame in frames {
            if recording.frames.send(frame).is_err() {
                // The encoder has stopped; its job reports why.
                self.stop_video_recording();
                return;
            }
            recording.collected += 1;
        }
        let total = recording.times.len();
        self.editor.video_export_progress = Some((recording.collected, total));
        if recording.collected == total {
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
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.release_video_target();
        }
        self.editor.video_export_progress = None;
        self.editor.video_export_frame = None;
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
