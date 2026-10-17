//! How an Improve run is going, as its progress card shows it: the stage it
//! is in, and the value of the best schedule it holds over time, with the
//! bound on every schedule once one is proved.
//!
//! The solve reports [`ImproveEvent`]s - from the solver process, across its
//! pipe - and the app folds them into an [`ImproveProgress`], timed by its
//! own clock from the moment the run started.

/// A stage of an Improve run, in the order a run passes through them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ImproveStage {
    /// Reading the project into the model.
    Capture,
    /// The hourly dispatch schedule every run starts from.
    FirstSchedule,
    /// The search from it (`scip::anytime`).
    Search,
    /// The whole-horizon SCIP solve, where the model is small enough.
    WholeHorizon,
    /// The app replaying the answer and publishing it.
    Publish,
}

/// One thing a run has to report.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum ImproveEvent {
    Stage(ImproveStage),
    /// The best schedule's value, and the tightest bound proved on every
    /// schedule.
    Point {
        value: f64,
        bound: Option<f64>,
    },
}

/// A value the run held, `at_s` seconds after it started.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ImprovePoint {
    pub(crate) at_s: f64,
    pub(crate) value: f64,
    pub(crate) bound: Option<f64>,
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImproveEnd {
    Published,
    /// Stopped and nothing kept, or failed: the schedule shown before stays.
    NotPublished,
}

/// An Improve run, as far as it has got.
#[derive(Clone, Debug)]
pub(crate) struct ImproveProgress {
    pub(crate) started: web_time::Instant,
    pub(crate) time_limit_s: Option<f64>,
    /// Each stage the run has reached and when, in seconds from its start.
    pub(crate) stages: Vec<(ImproveStage, f64)>,
    /// Every value reported, in order.
    pub(crate) points: Vec<ImprovePoint>,
    /// When the value last rose, in seconds from the start.
    pub(crate) improved_at_s: Option<f64>,
    /// Asked to stop and keep the best schedule it has.
    pub(crate) finishing: bool,
    /// When it ended and how; `None` while it runs.
    pub(crate) ended: Option<(f64, ImproveEnd)>,
}

impl ImproveProgress {
    pub(crate) fn new(time_limit_s: Option<f64>) -> Self {
        Self {
            started: web_time::Instant::now(),
            time_limit_s,
            stages: vec![(ImproveStage::Capture, 0.0)],
            points: Vec::new(),
            improved_at_s: None,
            finishing: false,
            ended: None,
        }
    }

    /// Seconds since the run started, or until it ended.
    pub(crate) fn elapsed_s(&self) -> f64 {
        self.ended.map_or_else(|| self.started.elapsed().as_secs_f64(), |(at_s, _)| at_s)
    }

    pub(crate) fn apply(&mut self, event: ImproveEvent) {
        let at_s = self.started.elapsed().as_secs_f64();
        match event {
            ImproveEvent::Stage(stage) => {
                if self.stages.last().is_none_or(|(last, _)| *last != stage) {
                    self.stages.push((stage, at_s));
                }
            }
            ImproveEvent::Point { value, bound } => {
                if self.points.last().is_some_and(|last| value > last.value) {
                    self.improved_at_s = Some(at_s);
                }
                self.points.push(ImprovePoint { at_s, value, bound });
            }
        }
    }

    pub(crate) fn end(&mut self, how: ImproveEnd) {
        if self.ended.is_none() {
            self.ended = Some((self.started.elapsed().as_secs_f64(), how));
        }
    }

    pub(crate) fn stage(&self) -> ImproveStage {
        self.stages.last().map_or(ImproveStage::Capture, |(stage, _)| *stage)
    }

    /// How long each stage reached took, the last one up to now.
    pub(crate) fn stage_durations(&self) -> Vec<(ImproveStage, f64)> {
        let now = self.elapsed_s();
        self.stages
            .iter()
            .enumerate()
            .map(|(index, (stage, from))| (*stage, self.stages.get(index + 1).map_or(now, |(_, to)| *to) - from))
            .collect()
    }

    pub(crate) fn first_value(&self) -> Option<f64> {
        self.points.first().map(|point| point.value)
    }

    pub(crate) fn best(&self) -> Option<&ImprovePoint> {
        self.points.last()
    }

    /// The best schedule's distance below the bound, as a share of the bound.
    pub(crate) fn gap(&self) -> Option<f64> {
        let best = self.best()?;
        best.bound.filter(|bound| *bound > 0.0).map(|bound| (bound - best.value).max(0.0) / bound)
    }
}
