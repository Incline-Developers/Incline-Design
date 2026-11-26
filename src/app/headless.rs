//! Incline with no window: `incline-design mcp --headless`.
//!
//! The process is one MCP client's own Incline, spoken to over standard
//! input and output. It opens, edits, builds, saves and draws projects as the
//! window does, through the same tools and commands, but nothing is shown:
//! the scene is only ever drawn offscreen, for `render_view`. It needs a GPU
//! for that, but no display, and it ends when the client closes its end.
//!
//! Without a window there is no event loop to drive it, so it drives itself:
//! each turn takes the client's requests and what the background work has
//! finished, takes the project a step on as the window's frame would, and
//! sleeps until the next request or the soonest thing that is due.

use std::{
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use anyhow::Result;

use super::App;
use crate::rendering::graphics::Graphics;

/// How often a turn comes round while background work is under way, to take
/// in what it has finished.
const BUSY_TURN: Duration = Duration::from_millis(16);
/// How often a turn comes round with nothing to do, for the clocks that keep
/// the project's own runs current.
const IDLE_TURN: Duration = Duration::from_secs(1);

/// Rouses the loop when the client sends a request.
#[derive(Default)]
struct Waker {
    woken: Mutex<bool>,
    signal: Condvar,
}

impl Waker {
    fn wake(&self) {
        if let Ok(mut woken) = self.woken.lock() {
            *woken = true;
            self.signal.notify_one();
        }
    }

    /// Sleep until woken or `deadline`, whichever is first.
    fn sleep_until(&self, deadline: Instant) {
        let Ok(mut woken) = self.woken.lock() else {
            return;
        };
        while !*woken {
            let Some(left) = deadline.checked_duration_since(Instant::now()).filter(|left| !left.is_zero()) else {
                break;
            };
            match self.signal.wait_timeout(woken, left) {
                Ok((guard, _)) => woken = guard,
                Err(_) => return,
            }
        }
        *woken = false;
    }
}

/// Serve the MCP client on standard input and output until it goes.
pub(crate) fn run() -> Result<()> {
    // The person's settings are read, for their libraries and coordinate
    // systems, but this Incline is the client's: it leaves them, and the
    // projects it opens out of their recent list.
    super::io::keep_settings_read_only();
    let waker = Arc::new(Waker::default());
    let wake = {
        let waker = Arc::clone(&waker);
        Arc::new(move || waker.wake())
    };
    let server = crate::mcp::Server::on_standard_io(wake)?;
    let mut app = App::new()?;
    crate::logging::startup_log();
    app.start_headless(server);
    loop {
        app.headless_turn();
        if app.headless_finished() {
            break;
        }
        waker.sleep_until(app.headless_deadline(Instant::now()));
    }
    // A run in flight is told to stop rather than holding the exit.
    app.cancel_schedule_run_calculation_quietly();
    app.graphics = None;
    Ok(())
}

impl App<'_> {
    /// Take `server` as the MCP server and start a renderer with no window.
    /// Without a GPU the project can still be worked on; only pictures of
    /// it cannot be drawn.
    fn start_headless(&mut self, server: crate::mcp::Server) {
        self.headless = true;
        self.mcp_server = Some(server);
        match pollster::block_on(Graphics::new_headless()) {
            Ok(graphics) => self.graphics = Some(graphics),
            Err(error) => log::warn!("No renderer could be started, so render_view will not work: {error:#}"),
        }
    }

    /// One turn: the client's requests, the background work's results, and
    /// the project taken a step on.
    fn headless_turn(&mut self) {
        self.poll_saves();
        self.poll_file_dialogs();
        self.sync_geophysics();
        self.serve_mcp();
        self.poll_mcp_waits();
        self.poll_background_work();
        self.advance_project();
        self.draw_due_video_frame();
        let now = Instant::now();
        if self.scanned_model_kept_until.is_some_and(|until| until <= now) {
            self.scanned_model_kept_until = None;
            self.evict_unloaded_items();
        }
    }

    /// Draw the video frame a recording has due, as the window does after
    /// its own frame, and pass on the frames that have come back.
    fn draw_due_video_frame(&mut self) {
        if let Some(frame) = self.video_recording.as_mut().and_then(super::video_export::Recording::take_render_request) {
            let project = self.project_view();
            let drawn = super::events::drawn_surfaces(
                &self.editor,
                false,
                crate::app::commands::prismoid_grid::with_prismoid_scene(&self.triangulations, &self.prismoid_scene),
                &self.solid_view_body,
                &self.reserves_scene,
                &self.schedule_animation,
                &self.water,
                None,
            );
            if let Some(graphics) = self.graphics.as_mut() {
                graphics.render_video_frame(crate::rendering::graphics::video_capture::VideoFrameInput {
                    editor: &mut self.editor,
                    document: &self.scene_document,
                    triangulations: drawn.0,
                    block_models: &self.block_models,
                    drill_holes: &self.drill_holes,
                    point_clouds: &self.point_clouds,
                    rasters: &self.raster_textures,
                    project: &project,
                    size: frame.size,
                    clock_s: frame.clock_s,
                    labels: frame.labels,
                    view: frame.view,
                    hidden: frame.hidden,
                    sync: true,
                });
            }
            if let Some(next) = self.video_recording.as_ref().and_then(super::video_export::Recording::upcoming_time) {
                self.editor.schedule_time_h = next;
            }
        }
        self.collect_video_frames();
    }

    /// Whether the client has gone and nothing being written is left to
    /// finish.
    fn headless_finished(&self) -> bool {
        self.mcp_server.as_ref().is_none_or(crate::mcp::Server::closed) && self.pending_saves.is_empty()
    }

    /// When the next turn is due unless a request comes first.
    fn headless_deadline(&self, now: Instant) -> Instant {
        let busy = self.video_recording.is_some()
            || !self.pending_jobs.is_empty()
            || !self.pending_saves.is_empty()
            || !self.pending_file_dialogs.is_empty()
            || !self.mcp_waits.is_empty();
        let turn = now + if busy { BUSY_TURN } else { IDLE_TURN };
        [
            self.schedule_auto_deadline,
            self.planning_auto_deadline,
            self.haulage_auto_deadline,
            self.scanned_model_kept_until,
        ]
        .into_iter()
        .flatten()
        .fold(turn, Instant::min)
    }
}
