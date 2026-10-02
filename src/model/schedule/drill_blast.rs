//! Drill and blast: the work between ground that is standing and ground a
//! loader can dig.
//!
//! A blast goes through a fixed chain. It is *clear* once nothing above it,
//! within the buffer distance, is left standing; a dozer then preps its top
//! surface, drills drill its pattern, an MPU charges the holes, and it fires
//! in the next blast window. Its dig blocks are available from that window's
//! end. Dozers, drills and MPUs are machines like loaders - a machine class
//! has a [`MachineKind`] - and work the blasts their Gantt bars list, in
//! order. See `docs/activity-sequencing-plan.md`.

use serde::{Deserialize, Serialize};

use crate::{i18n::tr, model::SolidId};

/// What a machine class does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MachineKind {
    /// Digs and reclaims, in tonnes per hour.
    #[default]
    Loader,
    /// Preps a blast's top surface, in square metres per hour.
    Dozer,
    /// Drills a blast's pattern, in metres drilled per hour.
    Drill,
    /// A mobile processing unit charging holes, in tonnes of product per day.
    Mpu,
}

impl MachineKind {
    pub(crate) const ALL: [Self; 4] = [Self::Loader, Self::Dozer, Self::Drill, Self::Mpu];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Loader => tr!("machine-kind-loader"),
            Self::Dozer => tr!("machine-kind-dozer"),
            Self::Drill => tr!("machine-kind-drill"),
            Self::Mpu => tr!("machine-kind-mpu"),
        }
    }

    /// The unit its class rate is authored in.
    pub(crate) fn rate_unit(self) -> &'static str {
        match self {
            Self::Loader => "t/h",
            Self::Dozer => "m²/h",
            Self::Drill => "m/h",
            Self::Mpu => "t/day",
        }
    }

    /// Its authored rate as work per hour. An MPU's is per day.
    pub(crate) fn hourly(self, rate: f64) -> f64 {
        match self {
            Self::Mpu => rate / 24.0,
            _ => rate,
        }
    }

    pub(crate) fn is_drill_blast(self) -> bool {
        self != Self::Loader
    }

    /// The activity a drill & blast machine does, or `None` for a loader.
    pub(crate) fn activity(self) -> Option<BlastActivity> {
        match self {
            Self::Loader => None,
            Self::Dozer => Some(BlastActivity::Prep),
            Self::Drill => Some(BlastActivity::Drill),
            Self::Mpu => Some(BlastActivity::Charge),
        }
    }
}

/// One step of a blast's chain that a machine does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlastActivity {
    Prep,
    Drill,
    Charge,
}

impl BlastActivity {
    pub(crate) const ALL: [Self; 3] = [Self::Prep, Self::Drill, Self::Charge];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Prep => tr!("blast-activity-prep"),
            Self::Drill => tr!("blast-activity-drill"),
            Self::Charge => tr!("blast-activity-charge"),
        }
    }

    /// The unit its quantity is measured in.
    pub(crate) fn unit(self) -> &'static str {
        match self {
            Self::Prep => "m²",
            Self::Drill => "m",
            Self::Charge => "t",
        }
    }

    /// The stage a blast reaches when this is done.
    pub(crate) fn done(self) -> BlastStage {
        match self {
            Self::Prep => BlastStage::Prepped,
            Self::Drill => BlastStage::Drilled,
            Self::Charge => BlastStage::Charged,
        }
    }
}

/// How far a blast has come, at the start of the schedule or at any moment
/// of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlastStage {
    #[default]
    NotStarted,
    Prepped,
    Drilled,
    Charged,
    Fired,
}

impl BlastStage {
    pub(crate) const ALL: [Self; 5] = [Self::NotStarted, Self::Prepped, Self::Drilled, Self::Charged, Self::Fired];

    pub(crate) fn label(self) -> String {
        match self {
            Self::NotStarted => tr!("blast-stage-not-started"),
            Self::Prepped => tr!("blast-stage-prepped"),
            Self::Drilled => tr!("blast-stage-drilled"),
            Self::Charged => tr!("blast-stage-charged"),
            Self::Fired => tr!("blast-stage-fired"),
        }
    }

    /// Whether `activity` is already done at this stage.
    pub(crate) fn has_done(self, activity: BlastActivity) -> bool {
        self >= activity.done()
    }
}

/// A blast as the project remembers it: its solid, its bench and a point
/// inside it, like an excluded blast. Resolved against the blast faces of the
/// run being read, so a reference follows its ground when cuts move.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlastRef {
    pub(crate) solid: SolidId,
    /// Base RL of the bench.
    pub(crate) bench: f64,
    pub(crate) anchor: [f64; 2],
}

impl BlastRef {
    pub(crate) fn is_valid(&self) -> bool {
        self.bench.is_finite() && self.anchor.iter().all(|value| value.is_finite())
    }

    /// Whether this names the same stored blast as `other`: one solid and
    /// bench, and the very same anchor.
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.solid == other.solid && self.bench.to_bits() == other.bench.to_bits() && self.anchor.map(f64::to_bits) == other.anchor.map(f64::to_bits)
    }

    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.solid.hash(hasher);
        self.bench.to_bits().hash(hasher);
        self.anchor.map(f64::to_bits).hash(hasher);
    }
}

/// The blasts a dozer, drill or MPU bar works, in order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlastOrder {
    #[serde(default)]
    pub(crate) members: Vec<BlastRef>,
}

/// Burden and spacing, and how holes go below the floor.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DrillPattern {
    pub(crate) burden_m: f64,
    pub(crate) spacing_m: f64,
    #[serde(default)]
    pub(crate) staggered: bool,
    pub(crate) subdrill_m: f64,
}

impl Default for DrillPattern {
    fn default() -> Self {
        Self {
            burden_m: 5.0,
            spacing_m: 6.0,
            staggered: false,
            subdrill_m: 1.0,
        }
    }
}

impl DrillPattern {
    fn is_valid(&self) -> bool {
        positive(self.burden_m) && positive(self.spacing_m) && self.subdrill_m.is_finite() && self.subdrill_m >= 0.0
    }
}

/// A blast's starting stage, where the planner set one.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlastStatus {
    pub(crate) blast: BlastRef,
    pub(crate) stage: BlastStage,
}

/// A blast's own pattern, where it differs from the default.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlastPattern {
    pub(crate) blast: BlastRef,
    pub(crate) pattern: DrillPattern,
}

/// The project's drill and blast settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub(crate) struct DrillBlastConfig {
    /// Off by default: until a planner turns it on, every dig block is
    /// available from the start, as before drill and blast existed.
    pub(crate) enabled: bool,
    pub(crate) pattern: DrillPattern,
    pub(crate) hole_diameter_mm: f64,
    /// Unloaded collar length of each hole.
    pub(crate) stemming_m: f64,
    /// Density of the product, as loaded.
    pub(crate) product_density_t_m3: f64,
    /// How far, in plan, standing ground in a higher bench keeps a blast from
    /// being clear. Zero: only ground over the blast itself.
    pub(crate) buffer_m: f64,
    /// The daily blast window, in hours of the day. Ground is available from
    /// the end of the window a blast fires in.
    pub(crate) window_start_h: f64,
    pub(crate) window_end_h: f64,
    /// None preserves the legacy daily window; an empty list means no windows.
    #[serde(default)]
    pub(crate) windows: Option<Vec<BlastWindow>>,
    pub(crate) statuses: Vec<BlastStatus>,
    pub(crate) patterns: Vec<BlastPattern>,
}

impl Default for DrillBlastConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            pattern: DrillPattern::default(),
            hole_diameter_mm: 229.0,
            stemming_m: 3.5,
            product_density_t_m3: 1.2,
            buffer_m: 0.0,
            window_start_h: 12.0,
            window_end_h: 15.0,
            windows: None,
            statuses: Vec::new(),
            patterns: Vec::new(),
        }
    }
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

/// One recurring daily window or one window at elapsed project hours.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BlastWindow {
    pub(crate) id: u64,
    pub(crate) start_h: f64,
    pub(crate) end_h: f64,
    pub(crate) daily: bool,
}

impl BlastWindow {
    pub(crate) fn valid(&self) -> bool {
        self.start_h.is_finite() && self.end_h.is_finite() && self.start_h >= 0.0 && self.end_h > self.start_h && (!self.daily || self.end_h <= 24.0)
    }

    pub(crate) fn next_end(&self, charged_h: f64) -> Option<f64> {
        if self.daily {
            let end = (charged_h / 24.0).floor() * 24.0 + self.end_h;
            Some(if end > charged_h { end } else { end + 24.0 })
        } else {
            (self.end_h > charged_h).then_some(self.end_h)
        }
    }
}

/// The settings a Setup form edits, without the per-blast lists.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillBlastSettings {
    pub(crate) enabled: bool,
    pub(crate) pattern: DrillPattern,
    pub(crate) hole_diameter_mm: f64,
    pub(crate) stemming_m: f64,
    pub(crate) product_density_t_m3: f64,
    pub(crate) buffer_m: f64,
    pub(crate) window_start_h: f64,
    pub(crate) window_end_h: f64,
}

impl DrillBlastConfig {
    pub(crate) fn effective_windows(&self) -> Vec<BlastWindow> {
        self.windows.clone().unwrap_or_else(|| {
            vec![BlastWindow {
                id: 0,
                start_h: self.window_start_h,
                end_h: self.window_end_h,
                daily: true,
            }]
        })
    }

    /// Expand recurring windows only across the requested timeline range.
    pub(crate) fn window_spans(&self, from_h: f64, to_h: f64) -> Vec<(BlastWindow, f64, f64)> {
        let mut spans = Vec::new();
        for window in self.effective_windows() {
            if window.daily {
                let mut day = (from_h.max(0.0) / 24.0).floor() * 24.0;
                while day < to_h {
                    if day + window.end_h > from_h {
                        spans.push((window, day + window.start_h, day + window.end_h));
                    }
                    day += 24.0;
                }
            } else if window.start_h < to_h && window.end_h > from_h {
                spans.push((window, window.start_h, window.end_h));
            }
        }
        spans
    }

    pub(crate) fn set_windows(&mut self, windows: Vec<BlastWindow>) -> Result<(), DrillBlastError> {
        let mut candidate = self.clone();
        candidate.windows = Some(windows);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub(crate) fn is_pristine(&self) -> bool {
        *self == Self::default()
    }

    pub(crate) fn settings(&self) -> DrillBlastSettings {
        DrillBlastSettings {
            enabled: self.enabled,
            pattern: self.pattern,
            hole_diameter_mm: self.hole_diameter_mm,
            stemming_m: self.stemming_m,
            product_density_t_m3: self.product_density_t_m3,
            buffer_m: self.buffer_m,
            window_start_h: self.window_start_h,
            window_end_h: self.window_end_h,
        }
    }

    /// Replace the settings, or refuse them whole.
    pub(crate) fn set_settings(&mut self, settings: DrillBlastSettings) -> Result<(), DrillBlastError> {
        let candidate = Self {
            enabled: settings.enabled,
            pattern: settings.pattern,
            hole_diameter_mm: settings.hole_diameter_mm,
            stemming_m: settings.stemming_m,
            product_density_t_m3: settings.product_density_t_m3,
            buffer_m: settings.buffer_m,
            window_start_h: settings.window_start_h,
            window_end_h: settings.window_end_h,
            windows: self.windows.clone(),
            statuses: self.statuses.clone(),
            patterns: self.patterns.clone(),
        };
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<(), DrillBlastError> {
        if !self.pattern.is_valid() || self.patterns.iter().any(|entry| !entry.pattern.is_valid()) {
            return Err(DrillBlastError::Pattern);
        }
        if !positive(self.hole_diameter_mm) || !positive(self.product_density_t_m3) || !(self.stemming_m.is_finite() && self.stemming_m >= 0.0) {
            return Err(DrillBlastError::Hole);
        }
        if !(self.buffer_m.is_finite() && self.buffer_m >= 0.0) {
            return Err(DrillBlastError::Buffer);
        }
        if !(self.window_start_h.is_finite() && self.window_end_h.is_finite() && 0.0 <= self.window_start_h && self.window_start_h < self.window_end_h && self.window_end_h <= 24.0)
        {
            return Err(DrillBlastError::Window);
        }
        if let Some(windows) = &self.windows {
            let mut ids = std::collections::BTreeSet::new();
            if windows.iter().any(|window| !window.valid() || !ids.insert(window.id)) {
                return Err(DrillBlastError::Window);
            }
        }
        if self.statuses.iter().any(|entry| !entry.blast.is_valid()) || self.patterns.iter().any(|entry| !entry.blast.is_valid()) {
            return Err(DrillBlastError::Reference);
        }
        Ok(())
    }

    /// Set the starting stage of each of `blasts`; Not started removes the
    /// entry.
    pub(crate) fn set_status(&mut self, blasts: &[BlastRef], stage: BlastStage) -> Result<(), DrillBlastError> {
        if blasts.iter().any(|blast| !blast.is_valid()) {
            return Err(DrillBlastError::Reference);
        }
        self.statuses.retain(|entry| !blasts.iter().any(|blast| blast.same(&entry.blast)));
        if stage != BlastStage::NotStarted {
            self.statuses.extend(blasts.iter().map(|blast| BlastStatus { blast: *blast, stage }));
        }
        Ok(())
    }

    /// Give a blast its own pattern, or `None` to return it to the default.
    pub(crate) fn set_pattern(&mut self, blast: BlastRef, pattern: Option<DrillPattern>) -> Result<(), DrillBlastError> {
        if !blast.is_valid() || pattern.is_some_and(|pattern| !pattern.is_valid()) {
            return Err(DrillBlastError::Pattern);
        }
        self.patterns.retain(|entry| !entry.blast.same(&blast));
        if let Some(pattern) = pattern {
            self.patterns.push(BlastPattern { blast, pattern });
        }
        Ok(())
    }

    /// Tonnes of product one hole of `depth_m` takes.
    pub(crate) fn charge_per_hole_t(&self, depth_m: f64) -> f64 {
        let radius = self.hole_diameter_mm / 2000.0;
        std::f64::consts::PI * radius * radius * (depth_m - self.stemming_m).max(0.0) * self.product_density_t_m3
    }

    pub(crate) fn hash_content<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        self.enabled.hash(hasher);
        for value in [
            self.pattern.burden_m,
            self.pattern.spacing_m,
            self.pattern.subdrill_m,
            self.hole_diameter_mm,
            self.stemming_m,
            self.product_density_t_m3,
            self.buffer_m,
            self.window_start_h,
            self.window_end_h,
        ] {
            value.to_bits().hash(hasher);
        }
        self.windows.is_some().hash(hasher);
        for window in self.windows.iter().flatten() {
            window.id.hash(hasher);
            window.start_h.to_bits().hash(hasher);
            window.end_h.to_bits().hash(hasher);
            window.daily.hash(hasher);
        }
        self.pattern.staggered.hash(hasher);
        for entry in &self.statuses {
            entry.blast.hash_content(hasher);
            entry.stage.hash(hasher);
        }
        for entry in &self.patterns {
            entry.blast.hash_content(hasher);
            for value in [entry.pattern.burden_m, entry.pattern.spacing_m, entry.pattern.subdrill_m] {
                value.to_bits().hash(hasher);
            }
            entry.pattern.staggered.hash(hasher);
        }
    }
}

/// Why drill and blast settings were refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DrillBlastError {
    Pattern,
    Hole,
    Buffer,
    Window,
    Reference,
}

impl DrillBlastError {
    pub(crate) fn message(self) -> String {
        match self {
            Self::Pattern => tr!("drill-blast-error-pattern"),
            Self::Hole => tr!("drill-blast-error-hole"),
            Self::Buffer => tr!("drill-blast-error-buffer"),
            Self::Window => tr!("drill-blast-error-window"),
            Self::Reference => tr!("drill-blast-error-reference"),
        }
    }
}
