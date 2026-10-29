//! SCIP's clocks (`clock.c`): nested-start stopwatches. Only wall-clock
//! time is kept (SCIP's default `timing/clocktype`); what a clock reads
//! never feeds back into the search unless a time limit is hit.

use web_time::Instant;

/// `SCIP_CLOCK`.
#[derive(Clone, Debug)]
pub struct Clock {
    /// Seconds accumulated over finished runs.
    elapsed: f64,
    started: Option<Instant>,
    pub lasttime: f64,
    pub nruns: i32,
    pub enabled: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock {
    /// `SCIPclockCreate`/`SCIPclockInit`.
    pub fn new() -> Self {
        Self {
            elapsed: 0.0,
            started: None,
            lasttime: 0.0,
            nruns: 0,
            enabled: true,
        }
    }

    /// `SCIPclockReset`.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.started = None;
        self.nruns = 0;
    }

    /// `SCIPclockEnable`.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// `SCIPclockDisable`: resets as well.
    pub fn disable(&mut self) {
        self.enabled = false;
        self.reset();
    }

    /// `SCIPclockEnableOrDisable`.
    pub fn enable_or_disable(&mut self, enable: bool) {
        if enable { self.enable() } else { self.disable() }
    }

    /// `SCIPclockStart`; `time_enabled` is `timing/enabled`.
    pub fn start(&mut self, time_enabled: bool) {
        if time_enabled && self.enabled {
            if self.nruns == 0 {
                self.started = Some(Instant::now());
                self.lasttime = self.elapsed;
            }
            self.nruns += 1;
        }
    }

    /// `SCIPclockStop`.
    pub fn stop(&mut self, time_enabled: bool) {
        if time_enabled && self.enabled {
            self.nruns -= 1;
            if self.nruns == 0
                && let Some(started) = self.started.take()
            {
                self.elapsed += started.elapsed().as_secs_f64();
            }
        }
    }

    /// `SCIPclockIsRunning`.
    pub fn is_running(&self) -> bool {
        self.nruns > 0
    }

    /// `SCIPclockGetTime`.
    pub fn get_time(&self) -> f64 {
        if !self.enabled {
            return 0.0;
        }
        match self.started {
            Some(started) if self.nruns > 0 => self.elapsed + started.elapsed().as_secs_f64(),
            _ => self.elapsed,
        }
    }

    /// `SCIPclockGetLastTime`.
    pub fn get_last_time(&self) -> f64 {
        self.lasttime
    }

    /// `SCIPclockSetTime`.
    pub fn set_time(&mut self, sec: f64) {
        self.elapsed = sec;
        if self.nruns > 0 {
            self.started = Some(Instant::now());
        }
    }
}
