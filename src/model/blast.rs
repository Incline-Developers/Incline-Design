//! Drill & Blast charging and timing: what goes down a hole, and what the
//! fired pattern adds up to.
//!
//! Two halves. The *library* - charge products and the rules that stack them
//! into a loaded column - is application configuration, kept with the delay
//! palette in the config file because the same products load every pattern a
//! user opens. A *charge* is project content: what one hole was loaded with,
//! carried by value so that editing the library later leaves a loaded round
//! exactly as it was designed, the same reason a [`crate::model::drill_hole::TieIn`]
//! carries its delay rather than a palette id.
//!
//! [`BlastAnalysis`] reads a tied, charged pattern back: when each hole
//! detonates, how much relief it has, lines of equal firing time, how much
//! explosive goes off in any 8 ms window, and the powder factor.

use std::{collections::HashMap, f64::consts::PI};

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::{
    i18n::tr,
    model::drill_hole::{DrillHole, DrillHoleDataset},
};

/// The window ground vibration is judged over: charges detonating within it
/// are treated as one, so the most explosive in any such window - the
/// maximum instantaneous charge - is what a site's vibration limit caps.
pub(crate) const VIBRATION_WINDOW_MS: f64 = 8.0;

/// What a deck of a loaded column is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum DeckKind {
    /// Bulk or packaged explosive. Its density turns a deck length into mass.
    Explosive,
    /// Inert fill - crushed rock or drill cuttings - that confines the gas.
    Stemming,
    /// A void held open by a gas bag or plug. Weighs nothing, takes length.
    Air,
}

impl DeckKind {
    pub(crate) const ALL: [Self; 3] = [Self::Explosive, Self::Stemming, Self::Air];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Explosive => tr!(literal = "Explosive"),
            Self::Stemming => tr!(literal = "Stemming"),
            Self::Air => tr!(literal = "Air deck"),
        }
    }
}

/// One product a deck can be loaded with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ChargeProduct {
    /// Unique within the library: rules name their products by it.
    pub(crate) name: String,
    pub(crate) kind: DeckKind,
    /// In-hole density in g/cm³. Only explosives use it.
    pub(crate) density: f64,
    /// sRGB, the colour the deck is drawn down the hole in.
    pub(crate) color: [u8; 3],
}

impl ChargeProduct {
    pub(crate) fn color_f32(&self) -> [f32; 3] {
        self.color.map(|channel| f32::from(channel) / 255.0)
    }
}

/// How much of the hole one deck of a rule takes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum DeckLength {
    /// A fixed length in metres.
    Fixed(f64),
    /// Whatever the fixed decks above and below leave. A rule has at most one.
    Fill,
}

/// One deck of a rule, from the collar down.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct RuleDeck {
    pub(crate) product: String,
    pub(crate) length: DeckLength,
}

/// A loading rule: the stack of decks from the collar down, and how each
/// explosive deck in it is primed.
///
/// The decks above the fill deck are laid from the collar down and the decks
/// below it from the toe up, so the same rule loads a 9 m hole and a 15 m one
/// alike: the stemming stays the stemming and the toe charge stays at the
/// toe, and only the main column changes length.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ChargeRule {
    /// Unique within the library.
    pub(crate) name: String,
    pub(crate) decks: Vec<RuleDeck>,
    /// Downhole detonator delay in each primer, in milliseconds.
    pub(crate) downhole_delay_ms: u32,
    /// How far above the base of each explosive deck its primer sits.
    pub(crate) primer_offset: f64,
    /// Cast booster mass in each primer, in kilograms.
    pub(crate) booster_kg: f64,
}

impl ChargeRule {
    /// Why the rule cannot load anything, or `None` when it is sound.
    pub(crate) fn problem(&self, products: &[ChargeProduct]) -> Option<String> {
        if self.name.trim().is_empty() {
            return Some(tr!(literal = "Give the rule a name"));
        }
        if self.decks.is_empty() {
            return Some(tr!(literal = "Add at least one deck"));
        }
        if self.decks.iter().filter(|deck| deck.length == DeckLength::Fill).count() > 1 {
            return Some(tr!(literal = "Only one deck can fill the rest of the hole"));
        }
        if self
            .decks
            .iter()
            .any(|deck| matches!(deck.length, DeckLength::Fixed(length) if !(length.is_finite() && length > 0.0)))
        {
            return Some(tr!(literal = "Deck lengths must be greater than zero"));
        }
        if let Some(deck) = self.decks.iter().find(|deck| !products.iter().any(|product| product.name == deck.product)) {
            return Some(crate::i18n::tr_format!(literal = "No product named '%name%'", name = &deck.product));
        }
        let explosive = |deck: &RuleDeck| products.iter().any(|product| product.name == deck.product && product.kind == DeckKind::Explosive);
        if !self.decks.iter().any(explosive) {
            return Some(tr!(literal = "A rule needs at least one explosive deck"));
        }
        None
    }
}

/// The charge products and rules a fresh installation starts with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BlastLibrary {
    #[serde(default)]
    pub(crate) products: Vec<ChargeProduct>,
    #[serde(default)]
    pub(crate) rules: Vec<ChargeRule>,
}

impl Default for BlastLibrary {
    fn default() -> Self {
        let product = |name: &str, kind, density, color| ChargeProduct {
            name: name.to_owned(),
            kind,
            density,
            color,
        };
        let deck = |product: &str, length| RuleDeck {
            product: product.to_owned(),
            length,
        };
        Self {
            products: vec![
                product("ANFO", DeckKind::Explosive, 0.82, [0xF2, 0xB1, 0x34]),
                product("Emulsion", DeckKind::Explosive, 1.20, [0xE0, 0x4F, 0x8C]),
                product("Heavy ANFO 70/30", DeckKind::Explosive, 1.25, [0xE8, 0x6A, 0x3A]),
                product("Stemming", DeckKind::Stemming, 0.0, [0x9A, 0x8C, 0x78]),
                product("Air Deck", DeckKind::Air, 0.0, [0x8E, 0xD1, 0xF5]),
            ],
            rules: vec![
                ChargeRule {
                    name: "Emulsion column".to_owned(),
                    decks: vec![deck("Stemming", DeckLength::Fixed(3.5)), deck("Emulsion", DeckLength::Fill)],
                    downhole_delay_ms: 500,
                    primer_offset: 0.5,
                    booster_kg: 0.4,
                },
                ChargeRule {
                    name: "ANFO, emulsion toe".to_owned(),
                    decks: vec![
                        deck("Stemming", DeckLength::Fixed(3.5)),
                        deck("ANFO", DeckLength::Fill),
                        deck("Emulsion", DeckLength::Fixed(2.0)),
                    ],
                    downhole_delay_ms: 500,
                    primer_offset: 0.5,
                    booster_kg: 0.4,
                },
                ChargeRule {
                    name: "Air-decked".to_owned(),
                    decks: vec![
                        deck("Stemming", DeckLength::Fixed(3.0)),
                        deck("Air Deck", DeckLength::Fixed(1.0)),
                        deck("Heavy ANFO 70/30", DeckLength::Fill),
                    ],
                    downhole_delay_ms: 500,
                    primer_offset: 0.5,
                    booster_kg: 0.4,
                },
            ],
        }
    }
}

/// One deck as it was loaded, in measured depth down the hole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ChargeDeck {
    pub(crate) from: f64,
    pub(crate) to: f64,
    pub(crate) product: String,
    pub(crate) kind: DeckKind,
    pub(crate) density: f64,
    pub(crate) color: [f32; 3],
}

impl ChargeDeck {
    pub(crate) fn length(&self) -> f64 {
        (self.to - self.from).max(0.0)
    }

    /// Explosive mass in kilograms for a hole of `diameter` metres.
    pub(crate) fn mass_kg(&self, diameter: f64) -> f64 {
        match self.kind {
            DeckKind::Explosive => PI * (diameter * 0.5).powi(2) * self.length() * self.density * 1_000.0,
            DeckKind::Stemming | DeckKind::Air => 0.0,
        }
    }
}

/// A booster and its detonator, at a measured depth.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Primer {
    pub(crate) depth: f64,
    pub(crate) delay_ms: u32,
    pub(crate) booster_kg: f64,
}

/// What one hole is loaded with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct HoleCharge {
    /// The rule it was loaded by, as a record: renaming or deleting the rule
    /// afterwards leaves the charge alone.
    pub(crate) rule: String,
    pub(crate) decks: Vec<ChargeDeck>,
    pub(crate) primers: Vec<Primer>,
}

impl HoleCharge {
    /// Explosive mass in the hole, boosters included.
    pub(crate) fn mass_kg(&self, diameter: Option<f64>) -> f64 {
        let column = diameter.map_or(0.0, |diameter| self.decks.iter().map(|deck| deck.mass_kg(diameter)).sum());
        column + self.primers.iter().map(|primer| primer.booster_kg).sum::<f64>()
    }

    /// When the hole goes after its surface signal arrives: the first of its
    /// detonators to fire.
    pub(crate) fn downhole_delay_ms(&self) -> u32 {
        self.primers.iter().map(|primer| primer.delay_ms).min().unwrap_or(0)
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>() + self.rule.len() + self.decks.iter().map(|deck| size_of::<ChargeDeck>() + deck.product.len()).sum::<usize>() + self.primers.len() * size_of::<Primer>()
    }
}

/// Why a rule could not load one particular hole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChargeFailure {
    /// The hole has no depth below its collar.
    NoDepth,
    /// The fixed decks alone are longer than the hole.
    TooShort,
}

/// Lay `rule` down `hole`.
pub(crate) fn charge_hole(rule: &ChargeRule, products: &[ChargeProduct], hole: &DrillHole) -> Result<HoleCharge, ChargeFailure> {
    let (Some(first), Some(last)) = (hole.trace.first(), hole.trace.last()) else {
        return Err(ChargeFailure::NoDepth);
    };
    let (top, bottom) = (first.depth, last.depth);
    if bottom <= top + 1.0e-6 {
        return Err(ChargeFailure::NoDepth);
    }
    let record = |deck: &RuleDeck, from: f64, to: f64| {
        let product = products.iter().find(|product| product.name == deck.product);
        ChargeDeck {
            from,
            to,
            product: deck.product.clone(),
            kind: product.map_or(DeckKind::Stemming, |product| product.kind),
            density: product.map_or(0.0, |product| product.density),
            color: product.map_or([0.6; 3], ChargeProduct::color_f32),
        }
    };
    let fixed = |deck: &RuleDeck| match deck.length {
        DeckLength::Fixed(length) => length,
        DeckLength::Fill => 0.0,
    };

    let mut decks = Vec::with_capacity(rule.decks.len());
    match rule.decks.iter().position(|deck| deck.length == DeckLength::Fill) {
        Some(fill) => {
            let above: f64 = rule.decks[..fill].iter().map(fixed).sum();
            let below: f64 = rule.decks[fill + 1..].iter().map(fixed).sum();
            // The fill deck has to be left something worth loading, or the
            // rule is not describing this hole.
            if above + below > bottom - top - 0.1 {
                return Err(ChargeFailure::TooShort);
            }
            let mut depth = top;
            for deck in &rule.decks[..fill] {
                let to = depth + fixed(deck);
                decks.push(record(deck, depth, to));
                depth = to;
            }
            let fill_bottom = bottom - below;
            decks.push(record(&rule.decks[fill], depth, fill_bottom));
            let mut depth = fill_bottom;
            for deck in &rule.decks[fill + 1..] {
                let to = depth + fixed(deck);
                decks.push(record(deck, depth, to));
                depth = to;
            }
        }
        None => {
            // No fill: laid from the collar until the hole runs out.
            let mut depth = top;
            for deck in &rule.decks {
                if depth >= bottom - 1.0e-6 {
                    break;
                }
                let to = (depth + fixed(deck)).min(bottom);
                decks.push(record(deck, depth, to));
                depth = to;
            }
        }
    }

    let primers = decks
        .iter()
        .filter(|deck| deck.kind == DeckKind::Explosive && deck.length() > 1.0e-6)
        .map(|deck| Primer {
            depth: (deck.to - rule.primer_offset.max(0.0)).clamp(deck.from, deck.to),
            delay_ms: rule.downhole_delay_ms,
            booster_kg: rule.booster_kg.max(0.0),
        })
        .collect::<Vec<_>>();
    if primers.is_empty() {
        return Err(ChargeFailure::TooShort);
    }
    Ok(HoleCharge {
        rule: rule.name.clone(),
        decks,
        primers,
    })
}

/// Charges as a file holds them: keyed by hole name, for the same reason
/// [`crate::model::drill_hole::StoredTieIns`] is.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct StoredCharges {
    #[serde(default)]
    pub(crate) holes: Vec<StoredCharge>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StoredCharge {
    pub(crate) hole: String,
    pub(crate) charge: HoleCharge,
}

/// The bands burden relief is read in, in ms per metre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ReliefLimits {
    /// Below this the burden is still confined when the hole goes.
    pub(crate) low: f64,
    /// Above this the rock in front has moved off: cut-off and flyrock risk.
    pub(crate) high: f64,
}

impl Default for ReliefLimits {
    fn default() -> Self {
        Self { low: 4.0, high: 12.0 }
    }
}

/// Where one hole's relief stands against the limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReliefBand {
    /// Fires first among its neighbours: it breaks to a free face.
    Free,
    Tight,
    Good,
    Slack,
    /// No signal reaches the hole.
    Unfired,
}

pub(crate) const RELIEF_TIGHT_COLOR: [f32; 3] = [0.87, 0.20, 0.22];
pub(crate) const RELIEF_GOOD_COLOR: [f32; 3] = [0.22, 0.74, 0.38];
pub(crate) const RELIEF_SLACK_COLOR: [f32; 3] = [0.24, 0.52, 0.96];
pub(crate) const RELIEF_FREE_COLOR: [f32; 3] = [0.98, 0.98, 0.98];
pub(crate) const RELIEF_UNFIRED_COLOR: [f32; 3] = [0.35, 0.35, 0.38];

impl ReliefBand {
    pub(crate) fn color(self) -> [f32; 3] {
        match self {
            Self::Free => RELIEF_FREE_COLOR,
            Self::Tight => RELIEF_TIGHT_COLOR,
            Self::Good => RELIEF_GOOD_COLOR,
            Self::Slack => RELIEF_SLACK_COLOR,
            Self::Unfired => RELIEF_UNFIRED_COLOR,
        }
    }
}

/// One line of equal firing time, as a run of world points.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TimeContour {
    pub(crate) time_ms: f64,
    pub(crate) points: Vec<DVec3>,
    /// Whether the run closes on itself.
    pub(crate) closed: bool,
}

/// The surface signal crossing one tie: it leaves `from` when that hole's
/// surface signal arrives, and reaches `to` the tie's delay later.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SignalPath {
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) leaves_ms: f64,
    pub(crate) arrives_ms: f64,
}

/// The busiest 8 ms of the round by one measure.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct WindowPeak {
    pub(crate) start_ms: f64,
    pub(crate) value: f64,
}

/// A tied - and perhaps charged - pattern read back.
#[derive(Clone, Debug, Default)]
pub(crate) struct BlastAnalysis {
    /// When each hole detonates: surface arrival plus its downhole delay.
    /// `None` for a hole no signal reaches, and for an empty hole in a
    /// pattern that is being loaded - see [`Self::is_empty_hole`].
    pub(crate) times: Vec<Option<f64>>,
    /// Holes left unloaded in a pattern that has some loaded: they pass the
    /// signal on but never detonate.
    pub(crate) empty: Vec<bool>,
    /// Holes with a charge in them: the only ones the timeline shows going off.
    pub(crate) loaded: Vec<bool>,
    /// When the last connector finishes carrying the surface signal.
    pub(crate) signal_end_ms: Option<f64>,
    /// When the surface signal reaches each hole and lights its downline.
    pub(crate) surface_times: Vec<Option<f64>>,
    /// Milliseconds per metre to the most recent neighbour that fired at or
    /// before the hole. `None` for an unfired hole or one that fires first.
    pub(crate) relief: Vec<Option<f64>>,
    /// That neighbour.
    pub(crate) relieved_by: Vec<Option<usize>>,
    /// Explosive mass loaded in each hole, kilograms.
    pub(crate) mass_kg: Vec<f64>,
    /// Rock each hole is responsible for, cubic metres.
    pub(crate) volume: Vec<f64>,
    /// Reached holes in detonation order.
    pub(crate) firing_order: Vec<usize>,
    /// Every tie that carried a first signal, in the direction it carried it.
    pub(crate) signal_paths: Vec<SignalPath>,
    pub(crate) contours: Vec<TimeContour>,
    pub(crate) duration_ms: Option<f64>,
    pub(crate) charged_holes: usize,
    pub(crate) total_mass_kg: f64,
    pub(crate) total_volume: f64,
    pub(crate) peak_holes: WindowPeak,
    pub(crate) peak_mass: WindowPeak,
}

impl BlastAnalysis {
    pub(crate) fn compute(dataset: &DrillHoleDataset) -> Self {
        let count = dataset.holes.len();
        let surface = dataset.firing_times();
        // Until anything is loaded every reached hole is read as firing, so a
        // round can be timed before it is charged. Once loading starts only
        // loaded holes detonate: an empty hole passes the surface signal on
        // and does nothing else - no burst, no relief, no share of the 8 ms.
        let detonates = |index: usize| dataset.charges.is_empty() || dataset.charges.contains_key(&index);
        let times: Vec<Option<f64>> = (0..count)
            .map(|index| {
                if !detonates(index) {
                    return None;
                }
                let surface = surface.get(index).copied().flatten()?;
                let downhole = dataset.charges.get(&index).map_or(0, HoleCharge::downhole_delay_ms);
                Some(f64::from(surface) + f64::from(downhole))
            })
            .collect();
        let signal_paths = dataset
            .ties
            .iter()
            .zip(dataset.tie_flows(&surface))
            .filter_map(|(tie, flow)| {
                let (from, to) = match flow {
                    crate::model::drill_hole::TieFlow::AToB => (tie.a, tie.b),
                    crate::model::drill_hole::TieFlow::BToA => (tie.b, tie.a),
                    _ => return None,
                };
                let leaves_ms = f64::from(surface[from]?);
                Some(SignalPath {
                    from,
                    to,
                    leaves_ms,
                    arrives_ms: leaves_ms + f64::from(tie.delay_ms),
                })
            })
            .collect();
        let signal_paths: Vec<SignalPath> = signal_paths;
        let signal_end_ms = signal_paths
            .iter()
            .map(|path| path.arrives_ms)
            .chain(surface.iter().flatten().map(|time| f64::from(*time)))
            .reduce(f64::max);
        let empty = (0..count).map(|index| !detonates(index)).collect();
        let loaded = (0..count).map(|index| dataset.charges.contains_key(&index)).collect();
        let collars: Vec<DVec3> = dataset.holes.iter().map(DrillHole::collar_position).collect();
        let mesh = CollarMesh::build(&collars);

        let mut relief = vec![None; count];
        let mut relieved_by = vec![None; count];
        for hole in 0..count {
            let Some(time) = times[hole] else {
                continue;
            };
            let mut best: Option<(f64, usize)> = None;
            for &neighbour in &mesh.neighbours[hole] {
                let Some(other) = times[neighbour] else {
                    continue;
                };
                if other > time {
                    continue;
                }
                let distance = collars[hole].truncate().distance(collars[neighbour].truncate()).max(1.0e-6);
                let value = (time - other) / distance;
                if best.is_none_or(|(best, _)| value < best) {
                    best = Some((value, neighbour));
                }
            }
            if let Some((value, neighbour)) = best {
                relief[hole] = Some(value);
                relieved_by[hole] = Some(neighbour);
            }
        }

        let mass_kg: Vec<f64> = (0..count)
            .map(|index| dataset.charges.get(&index).map_or(0.0, |charge| charge.mass_kg(dataset.holes[index].diameter)))
            .collect();
        let volume: Vec<f64> = (0..count)
            .map(|index| {
                let hole = &dataset.holes[index];
                let height = match (hole.trace.first(), hole.trace.last()) {
                    (Some(first), Some(last)) => (first.position.z - last.position.z).abs(),
                    _ => 0.0,
                };
                mesh.area[index] * height
            })
            .collect();

        let mut firing_order: Vec<usize> = (0..count).filter(|index| times[*index].is_some()).collect();
        firing_order.sort_by(|a, b| times[*a].unwrap_or(0.0).total_cmp(&times[*b].unwrap_or(0.0)).then(a.cmp(b)));
        let duration_ms = firing_order.last().and_then(|last| times[*last]);

        let (peak_holes, peak_mass) = window_peaks(&firing_order, &times, &mass_kg);
        let contour_step_ms = duration_ms.map_or(0.0, |duration| nice_step(duration / 12.0));
        let contours = if contour_step_ms > 0.0 {
            mesh.contours(&collars, &times, contour_step_ms)
        } else {
            Vec::new()
        };

        Self {
            charged_holes: dataset.charges.len(),
            total_mass_kg: mass_kg.iter().sum(),
            total_volume: (0..count).filter(|index| dataset.charges.contains_key(index)).map(|index| volume[index]).sum(),
            surface_times: surface.iter().map(|time| time.map(f64::from)).collect(),
            empty,
            loaded,
            signal_end_ms,
            times,
            relief,
            relieved_by,
            mass_kg,
            volume,
            firing_order,
            signal_paths,
            contours,
            duration_ms,
            peak_holes,
            peak_mass,
        }
    }

    /// Kilograms of explosive per cubic metre of the charged holes' rock.
    pub(crate) fn powder_factor(&self) -> Option<f64> {
        (self.total_volume > 1.0e-9 && self.total_mass_kg > 0.0).then(|| self.total_mass_kg / self.total_volume)
    }

    pub(crate) fn hole_powder_factor(&self, hole: usize) -> Option<f64> {
        let volume = *self.volume.get(hole)?;
        let mass = *self.mass_kg.get(hole)?;
        (volume > 1.0e-9 && mass > 0.0).then(|| mass / volume)
    }

    /// Whether `hole` is left unloaded in a pattern being loaded.
    pub(crate) fn is_empty_hole(&self, hole: usize) -> bool {
        self.empty.get(hole).copied().unwrap_or(false)
    }

    /// Whether `hole` holds a charge.
    pub(crate) fn is_loaded(&self, hole: usize) -> bool {
        self.loaded.get(hole).copied().unwrap_or(false)
    }

    /// How far the timeline runs: to the last detonation's 8 ms window, or to
    /// the end of the surface signal if that runs on past it.
    pub(crate) fn timeline_end_ms(&self) -> Option<f64> {
        let detonations = self.duration_ms.map(|duration| duration + VIBRATION_WINDOW_MS);
        match (detonations, self.signal_end_ms) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }

    pub(crate) fn band(&self, hole: usize, limits: ReliefLimits) -> ReliefBand {
        if self.times.get(hole).copied().flatten().is_none() {
            return ReliefBand::Unfired;
        }
        match self.relief.get(hole).copied().flatten() {
            None => ReliefBand::Free,
            Some(value) if value < limits.low => ReliefBand::Tight,
            Some(value) if value > limits.high => ReliefBand::Slack,
            Some(_) => ReliefBand::Good,
        }
    }

    /// How many reached holes fall in each band.
    pub(crate) fn band_counts(&self, limits: ReliefLimits) -> [usize; 3] {
        let mut counts = [0; 3];
        for hole in 0..self.times.len() {
            match self.band(hole, limits) {
                ReliefBand::Tight => counts[0] += 1,
                ReliefBand::Good => counts[1] += 1,
                ReliefBand::Slack => counts[2] += 1,
                ReliefBand::Free | ReliefBand::Unfired => {}
            }
        }
        counts
    }

    /// Holes and kilograms detonating in `[start, start + 8 ms)`.
    pub(crate) fn window_at(&self, start: f64) -> (usize, f64) {
        let first = self.firing_order.partition_point(|hole| self.times[*hole].unwrap_or(0.0) < start);
        self.firing_order[first..]
            .iter()
            .take_while(|hole| self.times[**hole].unwrap_or(0.0) < start + VIBRATION_WINDOW_MS)
            .fold((0, 0.0), |(holes, mass), hole| (holes + 1, mass + self.mass_kg[*hole]))
    }
}

/// The busiest `VIBRATION_WINDOW_MS` by hole count and by mass, found with a
/// sliding window over the holes in firing order. Every window that matters
/// starts on a detonation, so only those starts are tried.
fn window_peaks(order: &[usize], times: &[Option<f64>], mass: &[f64]) -> (WindowPeak, WindowPeak) {
    let mut peak_holes = WindowPeak::default();
    let mut peak_mass = WindowPeak::default();
    let mut end = 0;
    let mut running = 0.0;
    for start in 0..order.len() {
        let opens = times[order[start]].unwrap_or(0.0);
        while end < order.len() && times[order[end]].unwrap_or(0.0) < opens + VIBRATION_WINDOW_MS {
            running += mass[order[end]];
            end += 1;
        }
        let holes = (end - start) as f64;
        if holes > peak_holes.value {
            peak_holes = WindowPeak { start_ms: opens, value: holes };
        }
        if running > peak_mass.value {
            peak_mass = WindowPeak { start_ms: opens, value: running };
        }
        running -= mass[order[start]];
    }
    (peak_holes, peak_mass)
}

/// A round 1-2-5 step near `raw`.
pub(crate) fn nice_step(raw: f64) -> f64 {
    if !raw.is_finite() || raw <= 0.0 {
        return 1.0;
    }
    let magnitude = 10f64.powf(raw.log10().floor());
    let normalized = raw / magnitude;
    let step = if normalized < 1.5 {
        1.0
    } else if normalized < 3.5 {
        2.0
    } else if normalized < 7.5 {
        5.0
    } else {
        10.0
    };
    (step * magnitude).max(1.0)
}

#[derive(Clone, Copy)]
struct CollarVertex {
    position: spade::Point2<f64>,
    hole: usize,
}

impl spade::HasPosition for CollarVertex {
    type Scalar = f64;

    fn position(&self) -> spade::Point2<f64> {
        self.position
    }
}

/// The pattern as a mesh of collars in plan: who neighbours whom, and how
/// much ground each hole stands for.
///
/// A Delaunay triangulation of the collars, with the long thin triangles a
/// concave or ragged pattern edge leaves across open ground dropped - they
/// join holes that do not relieve each other.
struct CollarMesh {
    triangles: Vec<[usize; 3]>,
    neighbours: Vec<Vec<usize>>,
    /// Plan area per hole: its angle-weighted share of every triangle around it,
    /// scaled up where the triangles do not go all the way round so an edge
    /// hole is credited with a full burden-by-spacing rather than half one.
    area: Vec<f64>,
}

impl CollarMesh {
    fn build(collars: &[DVec3]) -> Self {
        use spade::{DelaunayTriangulation, Triangulation as _};

        let count = collars.len();
        let mut mesh = Self {
            triangles: Vec::new(),
            neighbours: vec![Vec::new(); count],
            area: vec![0.0; count],
        };
        let vertices: Vec<CollarVertex> = collars
            .iter()
            .enumerate()
            .filter(|(_, collar)| collar.is_finite())
            .map(|(hole, collar)| CollarVertex {
                position: spade::Point2::new(collar.x, collar.y),
                hole,
            })
            .collect();
        if vertices.len() < 3 {
            return mesh;
        }
        let Ok(tin) = DelaunayTriangulation::<CollarVertex>::bulk_load(vertices) else {
            return mesh;
        };

        let mut edges: Vec<f64> = tin
            .undirected_edges()
            .map(|edge| {
                let [a, b] = edge.vertices().map(|vertex| vertex.position());
                (a.x - b.x).hypot(a.y - b.y)
            })
            .collect();
        if edges.is_empty() {
            return mesh;
        }
        edges.sort_by(f64::total_cmp);
        let limit = edges[edges.len() / 2] * 2.2;

        let mut angle = vec![0.0f64; count];
        for face in tin.inner_faces() {
            let vertices = face.vertices().map(|vertex| *vertex.data());
            let points = vertices.map(|vertex| DVec2::new(vertex.position.x, vertex.position.y));
            if (0..3).any(|index| points[index].distance(points[(index + 1) % 3]) > limit) {
                continue;
            }
            let area = 0.5 * (points[1] - points[0]).perp_dot(points[2] - points[0]).abs();
            if area <= 1.0e-9 {
                continue;
            }
            let holes = vertices.map(|vertex| vertex.hole);
            for index in 0..3 {
                let (hole, a, b) = (holes[index], points[(index + 1) % 3] - points[index], points[(index + 2) % 3] - points[index]);
                // Shared in proportion to the angle at each corner, which -
                // unlike an even third - gives every hole of a regular grid
                // exactly one cell whichever way the diagonals fall.
                let corner = a.angle_to(b).abs();
                mesh.area[hole] += area * corner / PI;
                angle[hole] += corner;
                for other in [holes[(index + 1) % 3], holes[(index + 2) % 3]] {
                    if !mesh.neighbours[hole].contains(&other) {
                        mesh.neighbours[hole].push(other);
                    }
                }
            }
            mesh.triangles.push(holes);
        }
        for hole in 0..count {
            if angle[hole] > 1.0e-6 {
                mesh.area[hole] *= (std::f64::consts::TAU / angle[hole]).clamp(1.0, 4.0);
            }
        }
        mesh
    }

    /// Lines of equal time across the mesh, every `step` ms, stitched from
    /// triangle crossings into runs.
    fn contours(&self, collars: &[DVec3], times: &[Option<f64>], step: f64) -> Vec<TimeContour> {
        let Some(max) = times.iter().flatten().copied().reduce(f64::max) else {
            return Vec::new();
        };
        let mut contours = Vec::new();
        let mut level = step;
        while level < max && contours.len() < 10_000 {
            // Each crossing segment joins two triangle edges, keyed by their
            // vertex pair, so runs are stitched by walking shared edges.
            let mut segments: Vec<[(usize, usize); 2]> = Vec::new();
            let mut points: HashMap<(usize, usize), DVec3> = HashMap::new();
            for triangle in &self.triangles {
                let Some(values) = triangle.iter().map(|hole| times[*hole]).collect::<Option<Vec<f64>>>() else {
                    continue;
                };
                let mut crossing = Vec::with_capacity(2);
                for index in 0..3 {
                    let (a, b) = (index, (index + 1) % 3);
                    if (values[a] >= level) != (values[b] >= level) {
                        let (ha, hb) = (triangle[a], triangle[b]);
                        let key = (ha.min(hb), ha.max(hb));
                        let t = (level - values[a]) / (values[b] - values[a]);
                        points.entry(key).or_insert_with(|| collars[ha].lerp(collars[hb], t));
                        crossing.push(key);
                    }
                }
                if let [from, to] = crossing[..] {
                    segments.push([from, to]);
                }
            }
            contours.extend(
                stitch(&segments, &points)
                    .into_iter()
                    .map(|(points, closed)| TimeContour { time_ms: level, points, closed }),
            );
            level += step;
        }
        contours
    }
}

/// Join crossing segments that share an edge into the longest runs they make.
fn stitch(segments: &[[(usize, usize); 2]], points: &HashMap<(usize, usize), DVec3>) -> Vec<(Vec<DVec3>, bool)> {
    let mut at: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (index, segment) in segments.iter().enumerate() {
        for key in segment {
            at.entry(*key).or_default().push(index);
        }
    }
    let mut used = vec![false; segments.len()];
    let mut runs = Vec::new();
    let next = |key: (usize, usize), used: &[bool]| at.get(&key).and_then(|list| list.iter().copied().find(|segment| !used[*segment]));
    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut keys = std::collections::VecDeque::from([segments[start][0], segments[start][1]]);
        // Extend forwards, then backwards, from the seed segment.
        for forwards in [true, false] {
            loop {
                let end = if forwards { *keys.back().unwrap() } else { *keys.front().unwrap() };
                let Some(segment) = next(end, &used) else {
                    break;
                };
                used[segment] = true;
                let [a, b] = segments[segment];
                let far = if a == end { b } else { a };
                if forwards {
                    keys.push_back(far);
                } else {
                    keys.push_front(far);
                }
            }
        }
        let closed = keys.len() > 3 && keys.front() == keys.back();
        if closed {
            keys.pop_back();
        }
        let run: Vec<DVec3> = keys.iter().filter_map(|key| points.get(key).copied()).collect();
        if run.len() >= 2 {
            runs.push((run, closed));
        }
    }
    runs
}
