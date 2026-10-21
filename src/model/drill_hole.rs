use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Arc,
};

use glam::{DQuat, DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::{
    i18n::tr,
    model::{formats::csv_drill_hole::CsvDrillFileMapping, project::ProjectItemState},
};

/// How much wider than the hole itself the collar marker is drawn.
pub(crate) const COLLAR_MARKER_RADIUS_SCALE: f64 = 5.0;
/// Smallest on-screen diameter used to draw a drill-hole trace.
pub(crate) const MIN_RENDER_PIXEL_DIAMETER: f32 = 2.0;
/// Independent screen-space floor: collars shrink to dots in overview views
/// instead of magnifying the trace's minimum diameter by the marker scale.
pub(crate) const COLLAR_MARKER_MIN_PIXEL_DIAMETER: f32 = 3.0;
/// World-space collar radius for datasets that do not provide a physical hole
/// diameter. Roughly matches a 240 mm production hole after the marker scale.
pub(crate) const COLLAR_MARKER_FALLBACK_RADIUS: f64 = 0.6;
pub(crate) const COLLAR_MARKER_OUTLINE_COLOR: [f32; 3] = [0.086, 0.376, 0.851];
pub(crate) const COLLAR_MARKER_FILL_COLOR: [f32; 3] = [1.0, 1.0, 1.0];
/// How many stops a numeric ramp may carry. A limit on the ramp only: a
/// categorical field colours every code it has.
pub(crate) const MAX_DRILL_COLOR_STOPS: usize = 12;
/// Distinct codes above which a categorical column is named on load as
/// likely free text. Nothing is dropped at any count.
pub(crate) const WIDE_CATEGORY_FIELD_HINT: usize = 256;
/// Upper bound for an interactively generated blast pattern. It keeps a bad
/// unit/spacing entry from building millions of preview primitives on the UI
/// thread while remaining comfortably above ordinary production rounds.
pub(crate) const MAX_PATTERN_HOLES: usize = 25_000;
/// How thick a tie-in connector is drawn, as a multiple of the radius of the
/// holes it joins. Its physical width follows the pattern, while the renderer
/// applies the same screen-space floor as a drill-hole trace.
pub(crate) const TIE_RADIUS_SCALE: f64 = 1.5;
/// A disc's diameter, in metres, where a set has not chosen one: ten times
/// an HQ core hole, so a disc never reads as the hole itself, and small
/// beside the closest drill spacings, so neighbouring discs never touch.
/// Far out the pixel floor below sets the size, not this.
pub(crate) const DEFAULT_DISC_DIAMETER: f64 = 1.0;
/// What the disc diameter control accepts, in metres.
pub(crate) const DISC_DIAMETER_RANGE: std::ops::RangeInclusive<f64> = 0.1..=10.0;
/// The string's on-screen width where a set has not chosen one.
pub(crate) const DEFAULT_STRING_PIXEL_WIDTH: f32 = 2.0;
/// What the string width control accepts, in pixels.
pub(crate) const STRING_PIXEL_WIDTH_RANGE: std::ops::RangeInclusive<f32> = 1.0..=8.0;
/// How much wider than the string a disc is always drawn, in pixels: two
/// pixels of colour either side of it however far the eye is.
pub(crate) const DISC_PIXEL_MARGIN: f32 = 4.0;
/// The least a disc is drawn along the string, in pixels, so a ply a few
/// centimetres thick still shows from kilometres away.
pub(crate) const DISC_MIN_PIXEL_LENGTH: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct DrillHoleId(pub(crate) u64);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DrillHoleSource {
    /// Deserialization-only compatibility for sessions created before DHD
    /// support was removed. These sources are never restored or opened.
    #[serde(rename = "Dhd")]
    LegacyDhd { path: PathBuf },
    Csv {
        name: String,
        files: Vec<CsvDrillFileMapping>,
        /// Filename-only browser identity. Native imports use their first
        /// mapped file path while the project retains the decoded dataset.
        #[serde(default)]
        browser_path: Option<PathBuf>,
    },
    /// A dataset decoded from an Open Mining Format container. It stays
    /// in-memory until exported or explicitly saved in another format.
    Omf { name: String, path: PathBuf },
}

impl DrillHoleSource {
    pub(crate) fn display_name(&self) -> String {
        match self {
            Self::LegacyDhd { path } => path
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| tr!("drill-hole-unsupported-drillhole-source")),
            Self::Csv { name, .. } => name.clone(),
            Self::Omf { name, .. } => name.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TraceStation {
    pub(crate) depth: f64,
    pub(crate) position: DVec3,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum DrillValue {
    Numeric(#[serde(with = "stored::number")] f64),
    Category(String),
}

/// One interval as the site logged it, and its identity within its hole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct LoggedInterval {
    #[serde(with = "stored::number")]
    pub(crate) from: f64,
    #[serde(with = "stored::number")]
    pub(crate) to: f64,
    pub(crate) values: BTreeMap<String, DrillValue>,
    /// The site logged nothing here: the interval was inserted by a
    /// correction, `from` and `to` are the depths it went in at and stay its
    /// identity, and `values` is empty.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) never_logged: bool,
}

/// One interval as interpreted: what every reader, colour and section sees.
/// `logged` is what the site sent, or `None` when nothing has parted them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillInterval {
    #[serde(with = "stored::number")]
    pub(crate) from: f64,
    #[serde(with = "stored::number")]
    pub(crate) to: f64,
    pub(crate) values: BTreeMap<String, DrillValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) logged: Option<LoggedInterval>,
}

impl DrillInterval {
    pub(crate) fn logged(&self) -> (f64, f64, &BTreeMap<String, DrillValue>) {
        match &self.logged {
            Some(logged) => (logged.from, logged.to, &logged.values),
            None => (self.from, self.to, &self.values),
        }
    }

    pub(crate) fn is_corrected(&self) -> bool {
        self.logged
            .as_ref()
            .is_some_and(|logged| logged.from != self.from || logged.to != self.to || logged.values != self.values)
    }

    /// Whether a correction inserted the interval where the site logged
    /// nothing.
    pub(crate) fn is_never_logged(&self) -> bool {
        self.logged.as_ref().is_some_and(|logged| logged.never_logged)
    }

    /// Set the as-logged interval aside, the first time a correction is
    /// about to part the two.
    fn keep_logged(&mut self) {
        if self.logged.is_none() {
            self.logged = Some(LoggedInterval {
                from: self.from,
                to: self.to,
                values: self.values.clone(),
                never_logged: false,
            });
        }
    }
}

/// What a correction does to an interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum CorrectionKind {
    /// A code or a name changes.
    Value,
    /// A from or a to moves.
    Boundary,
    /// An interval is added where nothing was logged.
    Insertion,
    /// An interval, logged or inserted, leaves the interpreted data; its
    /// ground reads as not logged.
    Deletion,
}

/// Where a correction stands on its way back to the site's database.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum CorrectionStatus {
    Proposed,
    Approved,
    Denied,
    Sent,
    Confirmed,
    Flagged,
}

/// One change to one interval of one hole, kept whatever is decided about it.
/// The interval is named by its hole and its as-logged from and to, which a
/// correction never moves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Correction {
    pub(crate) hole: String,
    #[serde(with = "stored::number")]
    pub(crate) logged_from: f64,
    #[serde(with = "stored::number")]
    pub(crate) logged_to: f64,
    pub(crate) kind: CorrectionKind,
    pub(crate) field: String,
    #[serde(default)]
    pub(crate) before: Option<DrillValue>,
    #[serde(default)]
    pub(crate) after: Option<DrillValue>,
    pub(crate) author: String,
    pub(crate) date: String,
    pub(crate) reason: String,
    pub(crate) status: CorrectionStatus,
    /// Filled when a decision is made; empty until then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) approver: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) decision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) decided: Option<String>,
    /// For an insertion that puts its row in, the whole row: every value it
    /// holds, fixed when the insertion is proposed. Other records name none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) row: Option<BTreeMap<String, DrillValue>>,
    /// For a deletion, the interval it takes out, whole: its depths, every
    /// value it holds and its logged side, so undo puts it back exactly and
    /// what was logged stays in the record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) removed: Option<DrillInterval>,
}

/// Numbers of drill-hole values and depths as a file holds them. JSON has no
/// NaN or infinity, so such a number is written as its text and read back
/// from it, and a null an earlier save left reads as NaN: a row survives a
/// save whatever it holds. Finite values keep the shape they always had.
mod stored {
    pub(super) mod number {
        use std::fmt;

        use serde::{Deserializer, Serializer, de};

        pub(crate) fn serialize<S: Serializer>(number: &f64, serializer: S) -> Result<S::Ok, S::Error> {
            match *number {
                number if number.is_finite() => serializer.serialize_f64(number),
                number if number.is_nan() => serializer.serialize_str("NaN"),
                number if number > 0.0 => serializer.serialize_str("inf"),
                _ => serializer.serialize_str("-inf"),
            }
        }

        pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
            deserializer.deserialize_any(Number)
        }

        /// Reads a number as [`serialize`] writes it, and a null as NaN. Any
        /// other text is no number, and the record holding it is not read.
        struct Number;

        impl de::Visitor<'_> for Number {
            type Value = f64;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number, \"NaN\", \"inf\", \"-inf\" or null")
            }

            fn visit_f64<E: de::Error>(self, number: f64) -> Result<f64, E> {
                Ok(number)
            }

            fn visit_i64<E: de::Error>(self, number: i64) -> Result<f64, E> {
                Ok(number as f64)
            }

            fn visit_u64<E: de::Error>(self, number: u64) -> Result<f64, E> {
                Ok(number as f64)
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<f64, E> {
                match text {
                    "NaN" => Ok(f64::NAN),
                    "inf" => Ok(f64::INFINITY),
                    "-inf" => Ok(f64::NEG_INFINITY),
                    _ => Err(E::custom(format!("not a number: {text:?}"))),
                }
            }

            fn visit_unit<E: de::Error>(self) -> Result<f64, E> {
                Ok(f64::NAN)
            }

            fn visit_none<E: de::Error>(self) -> Result<f64, E> {
                Ok(f64::NAN)
            }
        }
    }
}

impl Correction {
    /// Roughly how much memory the record holds: its text, the codes before
    /// and after, the decision fields and any row it holds.
    pub(crate) fn estimated_bytes(&self) -> usize {
        let decided = [&self.approver, &self.decision, &self.decided].into_iter().flatten().map(String::len);
        [
            size_of::<Self>(),
            self.hole.len(),
            self.field.len(),
            self.before.as_ref().map_or(0, value_bytes),
            self.after.as_ref().map_or(0, value_bytes),
            self.author.len(),
            self.date.len(),
            self.reason.len(),
            self.row.as_ref().map_or(0, values_bytes),
            self.removed.as_ref().map_or(0, |removed| {
                values_bytes(&removed.values) + removed.logged.as_ref().map_or(0, |logged| values_bytes(&logged.values))
            }),
        ]
        .into_iter()
        .chain(decided)
        .fold(0usize, usize::saturating_add)
    }

    /// A proposed correction to the interval of hole `hole` logged from
    /// `logged.0` to `logged.1`, as `note` sets it out.
    pub(crate) fn proposed(hole: &str, logged: (f64, f64), kind: CorrectionKind, field: &str, before: DrillValue, after: DrillValue, note: &CorrectionNote) -> Self {
        Self {
            hole: hole.to_owned(),
            logged_from: logged.0,
            logged_to: logged.1,
            kind,
            field: field.to_owned(),
            before: Some(before),
            after: Some(after),
            author: note.author.clone(),
            date: note.date.clone(),
            reason: note.reason.clone(),
            status: CorrectionStatus::Proposed,
            approver: None,
            decision: None,
            decided: None,
            row: None,
            removed: None,
        }
    }
}

/// Records that met one outcome in an edit, counted for a single warning that
/// names the first of them.
#[derive(Default)]
struct Tally<'a> {
    count: usize,
    first: Option<&'a Correction>,
}

impl<'a> Tally<'a> {
    fn add(&mut self, record: &'a Correction) {
        self.count += 1;
        self.first.get_or_insert(record);
    }

    /// The count, and the first record's hole and logged depths as a message
    /// names them; `None` when nothing was counted.
    fn first(&self) -> Option<(usize, String, String, String)> {
        self.first
            .map(|record| (self.count, record.hole.clone(), format!("{:.2}", record.logged_from), format!("{:.2}", record.logged_to)))
    }
}

/// Which intervals a seam rename reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenameScope {
    /// One horizon of one hole: the block the log draws around interval
    /// `interval` of hole `hole`, that interval and the touching intervals
    /// either side holding the same name. Same-named horizons elsewhere in
    /// the hole are not reached.
    Horizon { hole: usize, interval: usize },
    /// Every hole of the set.
    Set,
}

/// Who proposed a correction, when and why.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CorrectionNote {
    pub(crate) author: String,
    pub(crate) date: String,
    pub(crate) reason: String,
}

impl CorrectionNote {
    /// A note dated now giving `reason`. The author stays empty until the
    /// project names a current author.
    pub(crate) fn now(reason: String) -> Self {
        Self {
            author: String::new(),
            date: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            reason,
        }
    }
}

/// The field a floor move names: the to of the interval it is keyed on.
/// Its before and after are depths.
pub(crate) const FLOOR_FIELD: &str = "to";
/// The field a roof moved alone names: the from of the interval it is
/// keyed on.
pub(crate) const ROOF_FIELD: &str = "from";

/// Which edge of an interval a boundary move takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoundarySide {
    Roof,
    Floor,
}

/// A roof or floor move asked for in one hole: the hole by index and by
/// name, the interval and side picked, the boundary's depth when it was
/// picked, and the depth it moves to.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BoundaryMove {
    pub(crate) hole: usize,
    pub(crate) dhid: String,
    pub(crate) interval: usize,
    pub(crate) side: BoundarySide,
    pub(crate) at: f64,
    pub(crate) to: f64,
}

/// Where a correction is written: its hole and the interval its record is
/// keyed on, by index, and for a boundary move the touching interval below,
/// whose from follows the moved to. An insertion's interval is where the row
/// goes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CorrectionTarget {
    pub(crate) hole: usize,
    pub(crate) interval: usize,
    pub(crate) below: Option<usize>,
}

impl CorrectionTarget {
    /// A correction written to one interval alone.
    pub(crate) fn interval(hole: usize, interval: usize) -> Self {
        Self { hole, interval, below: None }
    }
}

/// An interval asked to go into one hole: the hole by index and by name,
/// the field it is logged in, its depths and its code, and a code for any
/// other field of that field's table, blank where it has none.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IntervalInsert {
    pub(crate) hole: usize,
    pub(crate) dhid: String,
    pub(crate) field: String,
    pub(crate) from: f64,
    pub(crate) to: f64,
    pub(crate) code: String,
    pub(crate) siblings: Vec<(String, String)>,
}

impl IntervalInsert {
    /// Every code given, trimmed, each with its field: the field picked
    /// first, then the others, blanks and any repeat of the picked field
    /// left out.
    pub(crate) fn codes(&self) -> Vec<(&str, &str)> {
        let code = self.code.trim();
        std::iter::once((self.field.as_str(), code))
            .filter(|(_, code)| !code.is_empty())
            .chain(
                self.siblings
                    .iter()
                    .filter(|(field, code)| *field != self.field && !code.trim().is_empty())
                    .map(|(field, code)| (field.as_str(), code.trim())),
            )
            .collect()
    }
}

/// The rows of a hole an insertion of one field deals with, see
/// [`DrillHole::insertion_rows`].
#[derive(Clone, Debug, Default)]
struct InsertionRows<'a> {
    holding: Vec<(usize, &'a DrillInterval)>,
    blank: Vec<(usize, &'a DrillInterval)>,
}

/// An interval asked to be deleted: the hole by index and by name, the
/// interval by index and the field it was picked in, and the row as it was
/// picked, which must still be there.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IntervalDelete {
    pub(crate) hole: usize,
    pub(crate) dhid: String,
    pub(crate) interval: usize,
    pub(crate) field: String,
    pub(crate) row: DrillInterval,
}

/// What an insertion would do: its corrections, in the order they apply,
/// and which of the intervals it trims it splits, by index in the hole as
/// it stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct InsertionPlan {
    pub(crate) corrections: Vec<(CorrectionTarget, Correction)>,
    pub(crate) split: Vec<usize>,
}

/// Why an interval cannot be inserted where asked.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum InsertionProblem {
    /// The hole is not in the set.
    Missing,
    /// No code was given.
    NoCode,
    /// The from is not above the to.
    NoThickness,
    /// A depth lies outside 0 to the hole's end.
    OutsideHole { end: f64 },
    /// The interval would cover one of the field's intervals whole, which
    /// is a deletion rather than an insertion.
    Covers { from: f64, to: f64, name: String },
    /// The interval would overlap a row of the field's table that holds no
    /// value in the field, which is never trimmed.
    OverlapsUnnamed { from: f64, to: f64 },
}

impl InsertionProblem {
    pub(crate) fn message(&self) -> String {
        let depth = |depth: &f64| format!("{depth:.2}");
        match self {
            Self::Missing => tr!("drill-hole-insertion-hole-missing"),
            Self::NoCode => tr!("drill-hole-insertion-no-code"),
            Self::NoThickness => tr!("drill-hole-insertion-no-thickness"),
            Self::OutsideHole { end } => tr!("drill-hole-boundary-outside-hole", end = shown_depth(*end)),
            Self::Covers { from, to, name } => tr!("drill-hole-insertion-covers", from = depth(from), to = depth(to), name = name.clone()),
            Self::OverlapsUnnamed { from, to } => tr!("drill-hole-insertion-overlaps-unnamed", from = depth(from), to = depth(to)),
        }
    }
}

/// Why a boundary cannot be moved to the depth asked.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BoundaryProblem {
    /// The interval is not in the hole, or has no depths.
    Missing,
    /// The depth is not a number, or is where the boundary already is.
    Unmoved,
    /// The depth lies outside 0 to the hole's end.
    OutsideHole { end: f64 },
    /// An interval either side would be left with its from at its to.
    NoThickness { from: f64, to: f64 },
    /// The depth lies past the far edge of an interval either side.
    PastEdge { from: f64, to: f64 },
    /// With no interval touching below, the depth runs into the next one of
    /// the table, which starts at `from`.
    Overlap { from: f64 },
    /// With no interval touching above, the depth runs into the one before
    /// it in the table, which ends at `to`.
    OverlapAbove { to: f64 },
}

impl BoundaryProblem {
    pub(crate) fn message(&self) -> String {
        let depth = |depth: &f64| format!("{depth:.2}");
        match self {
            Self::Missing => tr!("drill-hole-boundary-interval-missing"),
            Self::Unmoved => tr!("drill-hole-boundary-unmoved"),
            Self::OutsideHole { end } => tr!("drill-hole-boundary-outside-hole", end = shown_depth(*end)),
            Self::NoThickness { from, to } => tr!("drill-hole-boundary-no-thickness", from = depth(from), to = depth(to)),
            Self::PastEdge { from, to } => tr!("drill-hole-boundary-past-edge", from = depth(from), to = depth(to)),
            Self::Overlap { from } => tr!("drill-hole-boundary-overlap", from = depth(from)),
            Self::OverlapAbove { to } => tr!("drill-hole-boundary-overlap-above", to = depth(to)),
        }
    }
}

/// One surface connector: the delay laid between two holes.
///
/// Undirected: the signal crosses it from whichever end it reaches first, so
/// the way a row was drawn has no bearing on how it fires. In a tie-up that
/// is a tree off its initiation points - nearly every shot - that is exactly
/// the round the connectors laid outward would give, and it spares the user
/// from having to draw every leg in firing order. See
/// [`DrillHoleDataset::tie_flows`] for which way each one was crossed.
///
/// The product is carried by value rather than by [`crate::ui::state::DelayProductId`].
/// The palette is application configuration - its ids are handed out afresh
/// each run and its order is by delay - so an id stored here would repoint at
/// whatever product took its place. A tie is a record of what was actually
/// laid, which is why editing the palette leaves a tied round alone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TieIn {
    /// The two holes it joins, in no particular order.
    pub(crate) a: usize,
    pub(crate) b: usize,
    pub(crate) delay_ms: u32,
    pub(crate) product: String,
    pub(crate) color: [f32; 3],
}

impl TieIn {
    /// Whether this connector joins the same two holes as `other`, whichever
    /// way round either runs. Two holes are joined by one connector or none -
    /// there is nowhere to put a second - so this is the identity a new tie
    /// overwrites on.
    pub(crate) fn joins(&self, a: usize, b: usize) -> bool {
        (self.a == a && self.b == b) || (self.a == b && self.b == a)
    }
}

/// How the round crossed one tie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TieFlow {
    /// The first signal to reach `b` came over it from `a`.
    AToB,
    /// The first signal to reach `a` came over it from `b`.
    BToA,
    /// Both ends were reached first by other routes: a loop, a backup, or
    /// two fronts meeting. It times nothing.
    Redundant,
    /// No signal reaches either end.
    Unreached,
}

/// Where a round starts, and how long after the shot is fired it goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Initiation {
    pub(crate) hole: usize,
    pub(crate) delay_ms: u32,
}

/// Ties as a file holds them: keyed by hole name, because a hole's index is
/// only stable for as long as the dataset stays loaded - see [`DrillHoleRef`] -
/// and ties outlive that.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct StoredTieIns {
    #[serde(default)]
    pub(crate) ties: Vec<StoredTieIn>,
    /// Every hole that can start the round. New files use this collection;
    /// `initiation` below is retained only so projects written by the first
    /// tie-in implementation still open without losing their start point.
    #[serde(default)]
    pub(crate) initiations: Vec<StoredInitiation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) initiation: Option<StoredInitiation>,
}

impl StoredTieIns {
    pub(crate) fn is_empty(&self) -> bool {
        self.ties.is_empty() && self.initiations.is_empty() && self.initiation.is_none()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StoredTieIn {
    pub(crate) a: String,
    pub(crate) b: String,
    pub(crate) delay_ms: u32,
    pub(crate) product: String,
    pub(crate) color: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StoredInitiation {
    pub(crate) hole: String,
    pub(crate) delay_ms: u32,
}

/// Where a hole's orientation came from: measured, assumed, or unknown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum OrientationSource {
    /// A survey record placed or steered the trace.
    Measured,
    /// No survey; the direction was set by hand or by design.
    Assumed,
    /// Nothing recorded; the trace is only a projection.
    #[default]
    Unknown,
}

impl OrientationSource {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Measured => tr!("drill-hole-measured"),
            Self::Assumed => tr!("drill-hole-assumed"),
            Self::Unknown => tr!("common-unknown"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillHole {
    pub(crate) dhid: String,
    pub(crate) collar: DVec3,
    /// Source diameter. The 2 px visual floor is applied by the trace shader,
    /// without changing this physical value.
    pub(crate) diameter: Option<f64>,
    pub(crate) trace: Vec<TraceStation>,
    /// Explicit geometry coverage. Empty means the complete measured-depth
    /// trace is continuous.
    pub(crate) render_ranges: Vec<(f64, f64)>,
    pub(crate) intervals: Vec<DrillInterval>,
    /// Defaults to `Unknown` for a project saved before this field existed.
    #[serde(default)]
    pub(crate) orientation_source: OrientationSource,
    /// The driller's depth from the collar file, stored as recorded: the
    /// logs may stop short of it, or run past it.
    #[serde(default)]
    pub(crate) total_depth: Option<f64>,
}

/// Row arrangement used when filling a blast boundary with collars.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DrillPatternLayout {
    #[default]
    Square,
    Staggered,
}

impl DrillPatternLayout {
    pub(crate) const ALL: [Self; 2] = [Self::Square, Self::Staggered];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Square => tr!("drill-hole-square"),
            Self::Staggered => tr!("drill-hole-staggered"),
        }
    }
}

/// Fill an XY polygon with a drill grid rotated counter-clockwise from global
/// X. `spacing` runs within a row and `burden` separates rows; staggered rows
/// move half a spacing. Collars sit half a cell inside the rotated polygon
/// bounds and take their Z from the boundary's polygon plane.
pub(crate) fn generate_pattern_collars(
    boundary: &[DVec3],
    burden: f64,
    spacing: f64,
    rotation_degrees: f64,
    offset: DVec2,
    layout: DrillPatternLayout,
) -> Result<Vec<DVec3>, String> {
    if boundary.len() < 3 || boundary.iter().any(|point| !point.is_finite()) {
        return Err(tr!("drill-hole-choose-valid-closed-polyline"));
    }
    if !burden.is_finite() || !spacing.is_finite() || burden <= 0.0 || spacing <= 0.0 {
        return Err(tr!("drill-hole-burden-spacing-must-greater-than"));
    }
    if !rotation_degrees.is_finite() || !offset.is_finite() {
        return Err(tr!("drill-hole-rotation-offsets-must-contain-valid"));
    }

    let centroid = boundary.iter().copied().sum::<DVec3>() / boundary.len() as f64;
    let (sin_rotation, cos_rotation) = rotation_degrees.to_radians().sin_cos();
    let grid_offset = DVec2::new(offset.x * cos_rotation + offset.y * sin_rotation, -offset.x * sin_rotation + offset.y * cos_rotation);
    let grid_boundary = boundary
        .iter()
        .map(|point| {
            let offset = point.truncate() - centroid.truncate();
            DVec3::new(
                offset.x * cos_rotation + offset.y * sin_rotation,
                -offset.x * sin_rotation + offset.y * cos_rotation,
                point.z,
            )
        })
        .collect::<Vec<_>>();
    let min_x = grid_boundary.iter().map(|point| point.x).fold(f64::INFINITY, f64::min);
    let max_x = grid_boundary.iter().map(|point| point.x).fold(f64::NEG_INFINITY, f64::max);
    let min_y = grid_boundary.iter().map(|point| point.y).fold(f64::INFINITY, f64::min);
    let max_y = grid_boundary.iter().map(|point| point.y).fold(f64::NEG_INFINITY, f64::max);
    let width = max_x - min_x;
    let height = max_y - min_y;
    let columns = ((width / spacing).ceil() as usize).max(1);
    let rows = ((height / burden).ceil() as usize).max(1);
    let cells = columns.saturating_mul(rows);
    if cells > MAX_PATTERN_HOLES.saturating_mul(20) {
        return Err(crate::i18n::tr!("drill-hole-spacing-would-scan-too-many", maximum = MAX_PATTERN_HOLES.to_string()));
    }

    // Newell's method gives a stable normal for either winding and polygons
    // with more than three vertices. A near-vertical/degenerate ring falls
    // back to the mean boundary elevation below.
    let mut normal = DVec3::ZERO;
    for index in 0..boundary.len() {
        let current = boundary[index];
        let next = boundary[(index + 1) % boundary.len()];
        normal.x += (current.y - next.y) * (current.z + next.z);
        normal.y += (current.z - next.z) * (current.x + next.x);
        normal.z += (current.x - next.x) * (current.y + next.y);
    }
    if normal.z.abs() <= (width * height).abs().max(1.0) * 1.0e-12 {
        return Err(tr!("drill-hole-selected-polyline-has-no-usable"));
    }
    let elevation = |x: f64, y: f64| {
        if normal.z.abs() > 1.0e-12 {
            centroid.z - (normal.x * (x - centroid.x) + normal.y * (y - centroid.y)) / normal.z
        } else {
            centroid.z
        }
    };

    let mut collars = Vec::with_capacity(cells.min(MAX_PATTERN_HOLES));
    let base_x = if width < spacing { (min_x + max_x) * 0.5 } else { min_x + spacing * 0.5 };
    let base_y = if height < burden { (min_y + max_y) * 0.5 } else { min_y + burden * 0.5 };
    // Reduce arbitrary offsets to one lattice period and start at the first
    // candidate inside the rotated bounds. This keeps large coordinate entry
    // values cheap while preserving their exact pattern phase.
    let first_x = base_x + grid_offset.x + ((min_x - base_x - grid_offset.x) / spacing).ceil() * spacing;
    let first_y = base_y + grid_offset.y + ((min_y - base_y - grid_offset.y) / burden).ceil() * burden;
    // Rows run in ascending Y, so a sweep keeps only the edges a row can
    // touch: the ones it crosses, plus any close enough to fall under the
    // on-boundary tolerance. A dense ring - a tessellated circle, say - then
    // costs one edge pass per row instead of one per candidate hole.
    struct BoundaryEdge {
        start: DVec2,
        end: DVec2,
        min_y: f64,
        max_y: f64,
    }
    const ON_EDGE_TOLERANCE: f64 = 1.0e-8;
    let mut edges = grid_boundary
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let start = point.truncate();
            let end = grid_boundary[(index + 1) % grid_boundary.len()].truncate();
            BoundaryEdge {
                start,
                end,
                min_y: start.y.min(end.y),
                max_y: start.y.max(end.y),
            }
        })
        .collect::<Vec<_>>();
    edges.sort_unstable_by(|left, right| left.min_y.total_cmp(&right.min_y));

    let mut pending_edge = 0_usize;
    let mut active_edges: Vec<usize> = Vec::new();
    let mut crossings: Vec<f64> = Vec::new();
    let mut on_boundary: Vec<usize> = Vec::new();
    for row in 0..rows {
        let y = first_y + row as f64 * burden;
        if y >= max_y {
            break;
        }
        let stagger = if layout == DrillPatternLayout::Staggered && row % 2 == 1 { spacing * 0.5 } else { 0.0 };

        while pending_edge < edges.len() && edges[pending_edge].min_y <= y + ON_EDGE_TOLERANCE {
            active_edges.push(pending_edge);
            pending_edge += 1;
        }
        active_edges.retain(|&index| edges[index].max_y >= y - ON_EDGE_TOLERANCE);

        crossings.clear();
        on_boundary.clear();
        for &index in &active_edges {
            let BoundaryEdge { start, end, .. } = edges[index];
            if (start.y > y) != (end.y > y) {
                crossings.push((end.x - start.x) * (y - start.y) / (end.y - start.y) + start.x);
            }
            // The tolerance is a hair over an exact hit, so an edge can only
            // put a handful of columns on the boundary - walk those instead of
            // testing every column against every edge.
            let span = end - start;
            let length_squared = span.length_squared();
            if length_squared > 0.0 {
                let low = start.x.min(end.x) - ON_EDGE_TOLERANCE;
                let high = start.x.max(end.x) + ON_EDGE_TOLERANCE;
                let first_column = (((low - first_x - stagger) / spacing).ceil()).max(0.0) as usize;
                let last_column = (((high - first_x - stagger) / spacing).floor()).max(0.0) as usize;
                for column in first_column..=last_column.min(columns.saturating_sub(1)) {
                    let point = DVec2::new(first_x + column as f64 * spacing + stagger, y);
                    let along = ((point - start).dot(span) / length_squared).clamp(0.0, 1.0);
                    if point.distance_squared(start + span * along) <= ON_EDGE_TOLERANCE * ON_EDGE_TOLERANCE {
                        on_boundary.push(column);
                    }
                }
            }
        }
        crossings.sort_unstable_by(f64::total_cmp);
        on_boundary.sort_unstable();
        on_boundary.dedup();

        // Even-odd fill: a column is inside when an odd number of crossings
        // lie to its right, which one pointer walk resolves for the whole row.
        let mut crossing_index = 0_usize;
        let mut boundary_index = 0_usize;
        for column in 0..columns {
            let x = first_x + column as f64 * spacing + stagger;
            if x >= max_x {
                break;
            }
            while crossing_index < crossings.len() && crossings[crossing_index] <= x {
                crossing_index += 1;
            }
            while boundary_index < on_boundary.len() && on_boundary[boundary_index] < column {
                boundary_index += 1;
            }
            let on_edge = on_boundary.get(boundary_index).is_some_and(|&hit| hit == column);
            if on_edge || (crossings.len() - crossing_index) % 2 == 1 {
                let world_x = centroid.x + x * cos_rotation - y * sin_rotation;
                let world_y = centroid.y + x * sin_rotation + y * cos_rotation;
                collars.push(DVec3::new(world_x, world_y, elevation(world_x, world_y)));
                if collars.len() > MAX_PATTERN_HOLES {
                    return Err(crate::i18n::tr!("drill-hole-pattern-too-many-holes", maximum = MAX_PATTERN_HOLES.to_string()));
                }
            }
        }
    }
    if collars.is_empty() {
        return Err(tr!("drill-hole-no-holes-fit-inside-boundary"));
    }
    Ok(collars)
}

/// Everything a move rewrites in a hole: its collar and its trace. Nothing
/// else is touched - the intervals above all, whose per-interval value maps
/// are what makes a whole [`DrillHole`] expensive to copy - so a live preview
/// captures and rewrites only this.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct HolePlacement {
    pub(crate) collar: DVec3,
    pub(crate) trace: Vec<TraceStation>,
    pub(crate) orientation_source: OrientationSource,
}

/// Where a hole points, in the terms a drill plan is written in: `azimuth`
/// degrees clockwise from grid north, `dip` degrees from horizontal with down
/// negative. This is the convention [`project_tangent`] resolves a survey in,
/// so a hole read out of a file and a hole turned here describe themselves the
/// same way.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct HoleOrientation {
    pub(crate) azimuth: f64,
    pub(crate) dip: f64,
}

/// Limit for canonical dip readouts and typed drill-plan orientations.
/// Ring gestures retain unwrapped dip angles so they can pass through vertical.
pub(crate) const MAX_HOLE_DIP: f64 = 90.0;

/// How a Rotate Collar edit turns the holes it was handed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum CollarRotation {
    /// Point every hole the same way, whatever each was pointing before - what
    /// the panel applies, a round being drilled at one angle.
    Absolute(HoleOrientation),
    /// Turn each hole from where it stood, so a pattern that was not uniform
    /// to begin with keeps its spread - what a ring drag produces.
    Delta { azimuth: f64, dip: f64 },
}

impl CollarRotation {
    /// The turn that leaves every hole exactly where it was, which is what
    /// reverting a Rotate Collar command writes.
    pub(crate) const IDENTITY: Self = Self::Delta { azimuth: 0.0, dip: 0.0 };

    pub(crate) fn is_identity(self) -> bool {
        matches!(self, Self::Delta { azimuth, dip } if azimuth == 0.0 && dip == 0.0)
    }
}

/// Which way a collar was set up, read off the first surveyed length below
/// it - the piece of the hole the rig actually aimed, rather than the
/// collar-to-toe chord, which on a hole that bends is neither.
///
/// A trace with no length below the collar has no direction to report.
fn trace_orientation(collar: DVec3, trace: &[TraceStation]) -> Option<HoleOrientation> {
    let direction = trace.iter().find_map(|station| {
        let offset = station.position - collar;
        (offset.length_squared() > 1.0e-12).then_some(offset)
    })?;
    Some(direction_orientation(direction))
}

/// Azimuth and dip of a direction, in the drill-plan convention.
///
/// A dead-vertical direction has no bearing to report and comes back as north,
/// the same stand-in [`resolve_trace`] falls back on for a survey that never
/// gave one.
pub(crate) fn direction_orientation(direction: DVec3) -> HoleOrientation {
    HoleOrientation {
        azimuth: direction.x.atan2(direction.y).to_degrees().rem_euclid(360.0),
        dip: direction.z.atan2(direction.x.hypot(direction.y)).to_degrees(),
    }
}

/// The world rotation taking a hole pointing `from` to one pointing `to`: a
/// swing about the vertical, then a tilt within the vertical plane the swing
/// left it in.
///
/// Decomposed that way rather than as a shortest-arc rotation because the two
/// halves are the two numbers on the drill plan: each gizmo ring drives one of
/// them, and neither disturbs the other.
fn orientation_rotation(from: HoleOrientation, to: HoleOrientation) -> DQuat {
    // Azimuth runs clockwise seen from above, which is the negative direction
    // about +Z.
    let swing = DQuat::from_rotation_z(-(to.azimuth - from.azimuth).to_radians());
    // Horizontal axis square to the destination bearing: turning about it is
    // what raises or drops the toe.
    let bearing = to.azimuth.to_radians();
    let tilt_axis = DVec3::new(bearing.cos(), -bearing.sin(), 0.0);
    DQuat::from_axis_angle(tilt_axis, (to.dip - from.dip).to_radians()) * swing
}

impl HolePlacement {
    /// Where the collar stands. Mirrors [`DrillHole::collar_position`]: the
    /// trace's first station is it, and `collar` only stands in for a hole
    /// that arrived without a trace.
    pub(crate) fn collar_position(&self) -> DVec3 {
        self.trace.first().map_or(self.collar, |station| station.position)
    }

    /// Which way the collar was set up. See [`trace_orientation`].
    pub(crate) fn orientation(&self) -> Option<HoleOrientation> {
        trace_orientation(self.collar_position(), &self.trace)
    }

    /// What `rotation` would leave this hole pointing at. `None` where the
    /// hole has no direction of its own to turn from.
    pub(crate) fn rotated_orientation(&self, rotation: CollarRotation) -> Option<HoleOrientation> {
        let from = self.orientation()?;
        Some(match rotation {
            CollarRotation::Absolute(target) => target,
            CollarRotation::Delta { azimuth, dip } => HoleOrientation {
                azimuth: (from.azimuth + azimuth).rem_euclid(360.0),
                dip: from.dip + dip,
            },
        })
    }
}

/// The values exports write for a total depth not recorded. Exact values,
/// not a range, so a deep hole in feet is still a depth; -99 and -999, the
/// other usual ones, are below zero and out already.
const TOTAL_DEPTH_NO_DATA: [f64; 2] = [9_999.0, 99_999.0];

/// A total depth as recorded, if it is one: positive, finite and not a
/// no-data value. Anything else is not recorded.
pub(crate) fn recorded_total_depth(depth: f64) -> Option<f64> {
    let no_data = TOTAL_DEPTH_NO_DATA.iter().any(|sentinel| (depth - sentinel).abs() < 1.0e-9);
    (depth.is_finite() && depth > 0.0 && !no_data).then_some(depth)
}

impl DrillHole {
    /// Where the hole ends: its trace's last depth, or deeper where an
    /// interval or its total depth runs on past the trace.
    pub(crate) fn end_depth(&self) -> f64 {
        self.intervals
            .iter()
            .map(|interval| interval.to)
            .chain(self.total_depth)
            .filter(|to| to.is_finite())
            .fold(self.trace.last().map_or(0.0, |station| station.depth), f64::max)
    }

    /// The stretch around `depth` an insertion of `field` has: from the
    /// deepest floor of a row bounding it at or above the depth, or the
    /// collar, to the shallowest roof of one below it, or the hole's end.
    /// Bounding rows are those of [`Self::insertion_rows`]; a row holding no
    /// data is as if it were not there. `None` where a bounding row holds the
    /// depth, or it is outside the hole. `derived` are the set's derived
    /// columns, which hold no data of their own; `end` is where the hole
    /// ends, as [`DrillHoleDataset::end_of`] has it.
    pub(crate) fn unlogged_stretch(&self, field: &str, depth: f64, derived: &BTreeSet<String>, end: f64) -> Option<(f64, f64)> {
        if !(0.0..end).contains(&depth) {
            return None;
        }
        let rows = self.insertion_rows(field, derived);
        let bounds = || rows.holding.iter().chain(&rows.blank).map(|(_, interval)| *interval);
        if bounds().any(|interval| interval.from <= depth && depth < interval.to) {
            return None;
        }
        let from = bounds().map(|interval| interval.to).filter(|to| *to <= depth).fold(0.0, f64::max);
        let to = bounds().map(|interval| interval.from).filter(|from| *from > depth).fold(end, f64::min);
        Some((from, to))
    }

    /// The rows an insertion of `field` deals with, by index, split in one
    /// pass: those holding a value of the field, which it trims, and those
    /// of the field's table blank in it but holding other data, such as a
    /// sample, which it may not overlap. A row holding no data, nothing but
    /// blanks and `derived` columns, is in neither: it is as if it were not
    /// there.
    fn insertion_rows(&self, field: &str, derived: &BTreeSet<String>) -> InsertionRows<'_> {
        let columns = field_table(self, field, derived);
        let mut rows = InsertionRows::default();
        for (index, interval) in self.intervals.iter().enumerate() {
            if holds_value(interval, field) {
                rows.holding.push((index, interval));
            } else if holds_data(interval, derived) && shares_table(interval, &columns) {
                rows.blank.push((index, interval));
            }
        }
        rows
    }

    /// Capture where the hole stands, for a preview to rewrite it from.
    pub(crate) fn placement(&self) -> HolePlacement {
        HolePlacement {
            collar: self.collar,
            trace: self.trace.clone(),
            orientation_source: self.orientation_source,
        }
    }

    /// Put the hole back at `placement` translated by `delta` - a zero delta
    /// therefore restores it exactly. The trace is copied into the existing
    /// allocation rather than replacing it. Interval geometry is measured down
    /// the trace rather than in world space, so it needs no adjustment.
    pub(crate) fn set_placement(&mut self, placement: &HolePlacement, delta: DVec3) {
        self.collar = placement.collar + delta;
        self.trace.clone_from(&placement.trace);
        self.orientation_source = placement.orientation_source;
        if delta != DVec3::ZERO {
            for station in &mut self.trace {
                station.position += delta;
            }
        }
    }

    /// Put the hole back at `placement` turned by `rotation` about its own
    /// collar - [`CollarRotation::IDENTITY`] therefore restores it exactly.
    ///
    /// The collar never moves: the whole trace swings rigidly about it, survey
    /// curvature and all, so a turned hole is the same hole aimed elsewhere
    /// rather than a straightened one. Interval geometry is measured down the
    /// trace rather than in world space, so it needs no adjustment - the same
    /// reason [`Self::set_placement`] leaves it alone.
    pub(crate) fn set_rotated_placement(&mut self, placement: &HolePlacement, rotation: CollarRotation) {
        self.collar = placement.collar;
        self.trace.clone_from(&placement.trace);
        self.orientation_source = placement.orientation_source;
        if rotation.is_identity() {
            // The restore path, taken on every rollback: the trace copy above
            // is already the whole of it.
            return;
        }
        let Some(from) = placement.orientation() else {
            // Nothing below the collar to aim, so there is nothing to turn.
            return;
        };
        let Some(to) = placement.rotated_orientation(rotation) else {
            return;
        };
        if to == from {
            return;
        }
        let quat = orientation_rotation(from, to);
        let pivot = placement.collar_position();
        for station in &mut self.trace {
            station.position = pivot + quat * (station.position - pivot);
        }
        self.orientation_source = OrientationSource::Assumed;
    }

    /// The physical world radius of the hole. A dataset without diameters uses
    /// a nominal radius for collars and ties; traces receive their independent
    /// screen-space floor in the renderer.
    pub(crate) fn render_radius(&self) -> f64 {
        self.diameter.map_or(COLLAR_MARKER_FALLBACK_RADIUS / COLLAR_MARKER_RADIUS_SCALE, |diameter| diameter * 0.5)
    }

    /// Where the collar stands. The trace's first station is it; `collar`
    /// only stands in for a dataset that arrived without a trace.
    pub(crate) fn collar_position(&self) -> DVec3 {
        self.trace.first().map_or(self.collar, |station| station.position)
    }

    /// Which way the hole was set up. See [`trace_orientation`]. Asked of the
    /// hole rather than of a captured placement so a per-frame readout costs
    /// no trace copy.
    pub(crate) fn orientation(&self) -> Option<HoleOrientation> {
        trace_orientation(self.collar_position(), &self.trace)
    }

    /// Where the trace stands at `depth`. Binary search, not a walk from the
    /// collar, so building a hole stops being quadratic in its station count.
    pub(crate) fn position_at_depth(&self, depth: f64) -> Option<DVec3> {
        let first = *self.trace.first()?;
        if depth <= first.depth {
            return Some(first.position);
        }
        let index = self.trace.partition_point(|station| station.depth < depth);
        // Past the toe there is no bracketing pair, and a depth that is not a
        // number lands at zero: both fall through to the last station.
        let Some(&[a, b]) = index.checked_sub(1).and_then(|above| self.trace.get(above..=index)) else {
            return self.trace.last().map(|station| station.position);
        };
        let span = b.depth - a.depth;
        let t = if span > 0.0 { ((depth - a.depth) / span).clamp(0.0, 1.0) } else { 0.0 };
        Some(a.position.lerp(b.position, t))
    }

    /// The straight pieces the trace is drawn in between depths `from` and
    /// `to`, clamped to the trace: cut at every station, at the ends of
    /// `render_ranges`, and at `cuts`. A piece whose middle lies outside
    /// every render range is not drawn and is left out, as is one with no
    /// length. Every builder of a hole's geometry and the pick walk the
    /// trace through this, so what is clickable is what is drawn.
    pub(crate) fn trace_pieces(&self, from: f64, to: f64, cuts: &[f64]) -> Vec<TracePiece> {
        let (Some(first), Some(last)) = (self.trace.first(), self.trace.last()) else {
            return Vec::new();
        };
        let (from, to) = (from.max(first.depth), to.min(last.depth));
        if !from.is_finite() || !to.is_finite() || to <= from {
            return Vec::new();
        }
        let inside = |depth: &f64| *depth > from && *depth < to;
        let mut boundaries: Vec<f64> = self
            .trace
            .iter()
            .map(|station| station.depth)
            .filter(inside)
            .chain(self.render_ranges.iter().flat_map(|&(start, end)| [start, end]).filter(inside))
            .chain(cuts.iter().copied().filter(inside))
            .chain([from, to])
            .collect();
        boundaries.sort_by(f64::total_cmp);
        boundaries.dedup_by(|a, b| (*a - *b).abs() <= 1.0e-9);

        let mut pieces = Vec::with_capacity(boundaries.len().saturating_sub(1));
        let mut previous = self.position_at_depth(boundaries[0]);
        for pair in boundaries.windows(2) {
            let start = previous;
            let end = self.position_at_depth(pair[1]);
            previous = end;
            if pair[1] <= pair[0] + 1.0e-9 {
                continue;
            }
            let (Some(start), Some(end)) = (start, end) else {
                continue;
            };
            if start.distance_squared(end) <= 1.0e-18 {
                continue;
            }
            let midpoint = (pair[0] + pair[1]) * 0.5;
            if !self.render_ranges.is_empty() && !self.render_ranges.iter().any(|(start, end)| *start <= midpoint && midpoint < *end) {
                continue;
            }
            pieces.push(TracePiece {
                from: pair[0],
                to: pair[1],
                start,
                end,
            });
        }
        pieces
    }
}

/// One straight piece of a drawn trace: its depths and where they stand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TracePiece {
    pub(crate) from: f64,
    pub(crate) to: f64,
    pub(crate) start: DVec3,
    pub(crate) end: DVec3,
}

fn trace_box(collar: DVec3, trace: &[TraceStation]) -> WorldBox {
    let mut min = collar;
    let mut max = collar;
    for station in trace {
        min = min.min(station.position);
        max = max.max(station.position);
    }
    (min, max)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DrillFieldKind {
    Numeric { min: f64, max: f64 },
    Categorical { categories: Vec<String> },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DrillField {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) kind: DrillFieldKind,
}

#[derive(Clone, Debug)]
pub(crate) struct DrillHoleDataset {
    pub(crate) holes: Vec<DrillHole>,
    pub(crate) fields: Vec<DrillField>,
    pub(crate) bounds: Option<(DVec3, DVec3)>,
    /// One box per hole, in `holes` order, worked out in the same pass as
    /// `bounds` so a pick can test a hole before walking it. Not saved.
    pub(crate) hole_boxes: Vec<WorldBox>,
    /// The surface connectors tying the pattern in. Content rather than
    /// styling: they dirty the project and are undone with everything else.
    pub(crate) ties: Vec<TieIn>,
    /// Holes the round can start at. One initiation per collar, with any
    /// number of collars participating in the same firing graph.
    pub(crate) initiations: Vec<Initiation>,
    /// Every correction proposed against the set, oldest first. Append-only:
    /// only undoing the edit that made one takes it back out.
    pub(crate) corrections: Vec<Correction>,
    /// What each loaded hole is charged with, by hole index. Content, like
    /// the ties: undone with everything else and written with the dataset.
    pub(crate) charges: BTreeMap<usize, crate::model::blast::HoleCharge>,
    /// Interval columns an Incline tool wrote and may write again. A column
    /// of the same name that came in with the data is never written over.
    pub(crate) derived_columns: BTreeSet<String>,
    /// The deletion records of each hole, by index into `corrections`, worked
    /// out on first use after an edit. Not saved.
    deletions: std::sync::OnceLock<HashMap<String, Vec<usize>>>,
}

impl DrillHoleDataset {
    pub(crate) fn new(mut holes: Vec<DrillHole>) -> Self {
        holes.sort_by(|a, b| crate::natural_sort::natural_cmp(&a.dhid, &b.dhid));
        // position_at_depth binary-searches on depth, so a trace must ascend.
        for hole in &mut holes {
            hole.trace.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        }
        let fields = collect_fields(&holes);
        let mut dataset = Self {
            holes,
            fields,
            bounds: None,
            hole_boxes: Vec::new(),
            ties: Vec::new(),
            initiations: Vec::new(),
            corrections: Vec::new(),
            charges: BTreeMap::new(),
            derived_columns: BTreeSet::new(),
            deletions: std::sync::OnceLock::new(),
        };
        // The one gate every importer and project load passes through, so the
        // boxes cannot fall out of step with the traces.
        dataset.refresh_bounds();
        dataset
    }

    /// An absent box is safe: the caller walks the hole. A stale one is not:
    /// the gate says no, so hole geometry changes must end in `refresh_bounds`.
    pub(crate) fn hole_box(&self, index: usize) -> Option<WorldBox> {
        self.hole_boxes.get(index).copied()
    }

    /// The connector between two holes.
    pub(crate) fn tie_between(&self, a: usize, b: usize) -> Option<&TieIn> {
        self.ties.iter().find(|tie| tie.joins(a, b))
    }

    /// When each hole fires, in milliseconds from the shot going off, or
    /// `None` for a hole no signal reaches.
    ///
    /// A hole fires on the *first* signal to arrive, so this is a multi-source
    /// shortest path from the initiation points rather than a walk of the graph: a round
    /// tied in a loop is well defined, and a connector that arrives after its
    /// hole has already gone simply does nothing. Ties carry the signal
    /// either way - see [`TieIn`].
    pub(crate) fn firing_times(&self) -> Vec<Option<u32>> {
        let mut times = vec![None; self.holes.len()];
        let mut adjacent: Vec<Vec<(usize, u32)>> = vec![Vec::new(); self.holes.len()];
        for tie in &self.ties {
            if tie.a < self.holes.len() && tie.b < self.holes.len() {
                adjacent[tie.a].push((tie.b, tie.delay_ms));
                adjacent[tie.b].push((tie.a, tie.delay_ms));
            }
        }
        let mut queue = std::collections::BinaryHeap::new();
        for initiation in self.initiations.iter().filter(|initiation| initiation.hole < self.holes.len()) {
            if times[initiation.hole].is_none_or(|existing| initiation.delay_ms < existing) {
                times[initiation.hole] = Some(initiation.delay_ms);
                queue.push(std::cmp::Reverse((initiation.delay_ms, initiation.hole)));
            }
        }
        while let Some(std::cmp::Reverse((time, hole))) = queue.pop() {
            if times[hole].is_some_and(|settled| settled < time) {
                continue;
            }
            for &(next, delay_ms) in &adjacent[hole] {
                let candidate = time.saturating_add(delay_ms);
                if times[next].is_none_or(|existing| candidate < existing) {
                    times[next] = Some(candidate);
                    queue.push(std::cmp::Reverse((candidate, next)));
                }
            }
        }
        times
    }

    /// Every interval holding `name` in `field` within `scope`, with its hole
    /// and interval index. An interval missing a depth is left out: it is
    /// never drawn, and its record could not be saved.
    pub(crate) fn intervals_named<'a>(&'a self, field: &'a str, name: &str, scope: RenameScope) -> impl Iterator<Item = ((usize, usize), &'a DrillHole, &'a DrillInterval)> + 'a {
        let holes: Box<dyn Iterator<Item = (usize, &DrillHole)>> = match scope {
            RenameScope::Horizon { hole, .. } => Box::new(self.holes.get(hole).map(|drilled| (hole, drilled)).into_iter()),
            RenameScope::Set => Box::new(self.holes.iter().enumerate()),
        };
        let horizon = match scope {
            RenameScope::Horizon { hole, interval } => Some(self.horizon(hole, field, interval)),
            RenameScope::Set => None,
        };
        let wanted = DrillValue::Category(name.to_owned());
        holes.flat_map(move |(hole_index, hole)| {
            let wanted = wanted.clone();
            let horizon = horizon.clone();
            hole.intervals
                .iter()
                .enumerate()
                .filter(move |(index, interval)| {
                    interval.from.is_finite()
                        && interval.to.is_finite()
                        && interval.values.get(field) == Some(&wanted)
                        && horizon.as_ref().is_none_or(|members| members.contains(index))
                })
                .map(move |(interval_index, interval)| ((hole_index, interval_index), hole, interval))
        })
    }

    /// The horizon around interval `interval` of hole `hole` in `field`, as
    /// the log draws it: that interval and, in depth order, the touching
    /// intervals either side holding the same name, by interval index. A gap
    /// or an interval of another name ends it; a row holding no value of the
    /// field, blank, of another table or empty, is passed over, as the strat
    /// column passes it. Empty when the interval holds no name or has no
    /// depths.
    pub(crate) fn horizon(&self, hole: usize, field: &str, interval: usize) -> Vec<usize> {
        let Some(drilled) = self.holes.get(hole) else {
            return Vec::new();
        };
        let name_of = |index: usize| match drilled.intervals[index].values.get(field) {
            Some(DrillValue::Category(name)) if !name.trim().is_empty() => Some(name),
            _ => None,
        };
        let mut order: Vec<usize> = (0..drilled.intervals.len()).filter(|&index| holds_value(&drilled.intervals[index], field)).collect();
        order.sort_by(|&a, &b| drilled.intervals[a].from.total_cmp(&drilled.intervals[b].from).then(a.cmp(&b)));
        let Some(at) = order.iter().position(|&index| index == interval) else {
            return Vec::new();
        };
        let Some(name) = name_of(interval) else {
            return Vec::new();
        };
        let joins = |upper: usize, lower: usize| name_of(lower) == Some(name) && drilled.intervals[lower].from <= drilled.intervals[upper].to + 1.0e-9;
        let mut start = at;
        while start > 0 && joins(order[start - 1], order[start]) && name_of(order[start - 1]) == Some(name) {
            start -= 1;
        }
        let mut end = at;
        while end + 1 < order.len() && joins(order[end], order[end + 1]) {
            end += 1;
        }
        order[start..=end].to_vec()
    }

    /// The value corrections renaming `from` to `to` in `field`, one per
    /// interval carrying `from` within `scope`, each paired with its hole and
    /// interval index. Nothing is changed here; the edit is the caller's.
    pub(crate) fn rename_corrections(&self, field: &str, from: &str, to: &str, scope: RenameScope, note: &CorrectionNote) -> Vec<(CorrectionTarget, Correction)> {
        if from == to {
            return Vec::new();
        }
        self.intervals_named(field, from, scope)
            .map(|((hole_index, interval_index), hole, interval)| {
                let (logged_from, logged_to, _) = interval.logged();
                let record = Correction::proposed(
                    &hole.dhid,
                    (logged_from, logged_to),
                    CorrectionKind::Value,
                    field,
                    DrillValue::Category(from.to_owned()),
                    DrillValue::Category(to.to_owned()),
                    note,
                );
                (CorrectionTarget::interval(hole_index, interval_index), record)
            })
            .collect()
    }

    /// The other intervals of hole `hole` from the interval table of
    /// interval `interval`, with depths. Two intervals are of one table when
    /// they share a column, columns an Incline tool derives aside: a header
    /// two tables share is told apart at import by the table's name, so a
    /// lithology row and a quality row never share one. A row with every
    /// cell blank, bar derived ones, could be of any table, so it counts in
    /// each.
    fn table_neighbours(&self, hole: usize, interval: usize) -> impl Iterator<Item = (usize, &DrillInterval)> + '_ {
        let intervals = self.holes.get(hole).map_or(&[][..], |hole| &hole.intervals[..]);
        let own = intervals.get(interval);
        let columns: BTreeSet<&String> = own
            .map(|own| own.values.keys().filter(|key| !self.derived_columns.contains(*key)).collect())
            .unwrap_or_default();
        intervals
            .iter()
            .enumerate()
            .filter(move |(index, other)| *index != interval && own.is_some() && in_table(other, &columns, &self.derived_columns))
    }

    /// The interval of the same table that starts exactly where interval
    /// `interval` of hole `hole` ends, by depth, never by index: the one a
    /// moved floor carries along. A row holding data goes before one holding
    /// none, blank or only derived, at the same depth.
    pub(crate) fn interval_below(&self, hole: usize, interval: usize) -> Option<usize> {
        let to = self.holes.get(hole)?.intervals.get(interval)?.to;
        self.table_neighbours(hole, interval)
            .filter(|(_, other)| other.from == to)
            .min_by_key(|(_, other)| !holds_data(other, &self.derived_columns))
            .map(|(index, _)| index)
    }

    /// The interval of the same table that ends exactly where interval
    /// `interval` of hole `hole` starts, as [`Self::interval_below`] picks.
    pub(crate) fn interval_above(&self, hole: usize, interval: usize) -> Option<usize> {
        let from = self.holes.get(hole)?.intervals.get(interval)?.from;
        self.table_neighbours(hole, interval)
            .filter(|(_, other)| other.to == from)
            .min_by_key(|(_, other)| !holds_data(other, &self.derived_columns))
            .map(|(index, _)| index)
    }

    /// The boundary correction moving the roof or floor of interval
    /// `interval` of hole `hole` to `depth`. A boundary two touching
    /// intervals of one table share is the upper one's floor: one record
    /// keyed on the upper interval, field [`FLOOR_FIELD`], its target
    /// carrying the lower one, which follows, so no gap or overlap opens
    /// and the hole keeps its length. A floor or roof nothing touches moves
    /// alone, a roof's record keyed on its own interval with field
    /// [`ROOF_FIELD`]. Refused, with the reason, when the depth is outside
    /// the hole, at or past the far edge of either interval, or across a
    /// gap into the next interval of the table. Nothing is changed here; the
    /// edit is the caller's.
    pub(crate) fn boundary_corrections(
        &self,
        hole: usize,
        interval: usize,
        side: BoundarySide,
        depth: f64,
        note: &CorrectionNote,
    ) -> Result<Vec<(CorrectionTarget, Correction)>, BoundaryProblem> {
        let drilled = self.holes.get(hole).ok_or(BoundaryProblem::Missing)?;
        // A roof shared with the interval above is that interval's floor,
        // and the clicked interval is the one that follows it: taken as is,
        // never looked up again, as a blank row above shares no column to
        // find it by.
        let (interval, side, follower) = match side {
            BoundarySide::Roof => match self.interval_above(hole, interval) {
                Some(above) => (above, BoundarySide::Floor, Some(interval)),
                None => (interval, BoundarySide::Roof, None),
            },
            BoundarySide::Floor => (interval, BoundarySide::Floor, None),
        };
        let moved = drilled
            .intervals
            .get(interval)
            .filter(|moved| moved.from.is_finite() && moved.to.is_finite())
            .ok_or(BoundaryProblem::Missing)?;
        let at = if side == BoundarySide::Roof { moved.from } else { moved.to };
        if !depth.is_finite() || depth == at {
            return Err(BoundaryProblem::Unmoved);
        }
        let end = drilled.end_depth();
        if !(0.0..=end).contains(&depth) {
            return Err(BoundaryProblem::OutsideHole { end });
        }
        let edge = |from: f64, to: f64, past: bool| {
            if past {
                BoundaryProblem::PastEdge { from, to }
            } else {
                BoundaryProblem::NoThickness { from, to }
            }
        };
        let neighbours = || self.table_neighbours(hole, interval).map(|(_, other)| other);
        let below = match side {
            BoundarySide::Roof => {
                if depth >= moved.to {
                    return Err(edge(moved.from, moved.to, depth > moved.to));
                }
                let before = neighbours().map(|other| other.to).filter(|to| *to <= moved.from).max_by(f64::total_cmp);
                if let Some(to) = before.filter(|to| depth < *to) {
                    return Err(BoundaryProblem::OverlapAbove { to });
                }
                None
            }
            BoundarySide::Floor => {
                if depth <= moved.from {
                    return Err(edge(moved.from, moved.to, depth < moved.from));
                }
                let below = follower.or_else(|| self.interval_below(hole, interval));
                match below.map(|index| &drilled.intervals[index]) {
                    Some(lower) if depth >= lower.to => return Err(edge(lower.from, lower.to, depth > lower.to)),
                    Some(_) => {}
                    None => {
                        let next = neighbours().map(|other| other.from).filter(|from| *from >= moved.to).min_by(f64::total_cmp);
                        if let Some(from) = next.filter(|from| depth > *from) {
                            return Err(BoundaryProblem::Overlap { from });
                        }
                    }
                }
                below
            }
        };
        let (logged_from, logged_to, _) = moved.logged();
        let field = if side == BoundarySide::Roof { ROOF_FIELD } else { FLOOR_FIELD };
        let record = Correction::proposed(
            &drilled.dhid,
            (logged_from, logged_to),
            CorrectionKind::Boundary,
            field,
            DrillValue::Numeric(at),
            DrillValue::Numeric(depth),
            note,
        );
        Ok(vec![(CorrectionTarget { hole, interval, below }, record)])
    }

    /// The hole `edit` names as it would read once moved, for a preview
    /// drawn while the move is still being typed. `None` when the move would
    /// be refused, or the hole at that index is no longer the one named.
    /// Nothing is changed or recorded here.
    pub(crate) fn boundary_preview(&self, edit: &BoundaryMove) -> Option<DrillHole> {
        let drilled = self.holes.get(edit.hole).filter(|hole| hole.dhid == edit.dhid)?;
        let (target, record) = self
            .boundary_corrections(edit.hole, edit.interval, edit.side, edit.to, &CorrectionNote::default())
            .ok()?
            .into_iter()
            .next()?;
        let mut previewed = drilled.clone();
        write_correction(&mut previewed, target, &record, true).then_some(previewed)
    }

    /// How many holes renaming `from` to `to` in `field` within `scope` would
    /// leave with `to` out of `column`'s order: in such a hole, once renamed
    /// and with touching runs of one name joined, a column name above a
    /// renamed run comes after `to` in the column, or one below it comes no
    /// later. Names outside the column and UNK are passed over, and a `to`
    /// the column lacks is never out of order. Disorder elsewhere in a hole
    /// does not count; only the renamed runs are judged. Nothing is changed.
    pub(crate) fn rename_out_of_sequence(&self, field: &str, from: &str, to: &str, scope: RenameScope, column: &[String]) -> usize {
        let mut renamed: Vec<(usize, usize)> = self.intervals_named(field, from, scope).map(|(at, _, _)| at).collect();
        renamed.sort_unstable();
        let mut out = 0;
        let mut start = 0;
        while start < renamed.len() {
            let hole = renamed[start].0;
            let end = start + renamed[start..].iter().take_while(|at| at.0 == hole).count();
            let members: Vec<usize> = renamed[start..end].iter().map(|at| at.1).collect();
            start = end;
            let rows: Vec<(f64, Option<&str>, bool)> = self.holes[hole]
                .intervals
                .iter()
                .enumerate()
                .filter(|(_, interval)| interval.from.is_finite() && interval.to.is_finite())
                .map(|(index, interval)| {
                    let name = match interval.values.get(field) {
                        Some(DrillValue::Category(name)) => Some(name.as_str()),
                        _ => None,
                    };
                    (interval.from, name, members.contains(&index))
                })
                .collect();
            if out_of_sequence(&rows, to, column) {
                out += 1;
            }
        }
        out
    }

    /// The corrections inserting the interval `edit` asks for. One insertion
    /// record keyed on the new interval's depths, carrying the whole row it
    /// puts in, and one more, carrying none, for each other field of its
    /// table given a code. Each interval holding a value of the field that it
    /// overlaps loses the overlap, its floor or roof moving alone with a
    /// boundary record. Where it falls inside one, that interval is split:
    /// its floor rises to the new from, and its part below the new to goes in
    /// as an insertion carrying its values, derived columns left for their
    /// tool to write again. Trims come first, then the inserted rows in depth
    /// order, each target written against the hole as the ones before it
    /// leave it. Refused, with the reason, when the hole at `edit.hole` is no
    /// longer the one named, the code is blank, the from is not above the to,
    /// a depth is outside the hole, the interval would cover one of the
    /// field's intervals whole, or it would overlap a row of the field's
    /// table blank in the field that holds other data, such as a sample,
    /// which is never trimmed. A row holding no data, nothing but blanks and
    /// derived columns, is left as it is and refuses nothing. Nothing is
    /// changed here; the edit is the caller's.
    pub(crate) fn insertion_corrections(&self, edit: &IntervalInsert, note: &CorrectionNote) -> Result<InsertionPlan, InsertionProblem> {
        let drilled = self.holes.get(edit.hole).filter(|hole| hole.dhid == edit.dhid).ok_or(InsertionProblem::Missing)?;
        let code = edit.code.trim();
        if code.is_empty() {
            return Err(InsertionProblem::NoCode);
        }
        let (from, to) = (edit.from, edit.to);
        if !from.is_finite() || !to.is_finite() || from >= to {
            return Err(InsertionProblem::NoThickness);
        }
        let end = self.end_of(drilled);
        if from < 0.0 || to > end {
            return Err(InsertionProblem::OutsideHole { end });
        }
        let field = edit.field.as_str();
        let rows = drilled.insertion_rows(field, &self.derived_columns);
        let holding = || rows.holding.iter().copied();
        let mut overlapped: Vec<(usize, &DrillInterval)> = holding().filter(|(_, other)| other.from < to && other.to > from).collect();
        overlapped.sort_by(|a, b| a.1.from.total_cmp(&b.1.from).then(a.0.cmp(&b.0)));
        if let Some((_, covered)) = overlapped.iter().find(|(_, other)| from <= other.from && other.to <= to) {
            return Err(InsertionProblem::Covers {
                from: covered.from,
                to: covered.to,
                name: value_name(covered.values.get(field)),
            });
        }
        if let Some((_, blank)) = rows.blank.iter().find(|(_, other)| other.from < to && other.to > from) {
            return Err(InsertionProblem::OverlapsUnnamed { from: blank.from, to: blank.to });
        }
        let mut plan = InsertionPlan::default();
        for &(index, other) in &overlapped {
            let (logged_from, logged_to, _) = other.logged();
            let (side, before, after) = if other.from < from {
                (FLOOR_FIELD, other.to, from)
            } else {
                (ROOF_FIELD, other.from, to)
            };
            let record = Correction::proposed(
                &drilled.dhid,
                (logged_from, logged_to),
                CorrectionKind::Boundary,
                side,
                DrillValue::Numeric(before),
                DrillValue::Numeric(after),
                note,
            );
            plan.corrections.push((CorrectionTarget::interval(edit.hole, index), record));
            if other.from < from && other.to > to {
                plan.split.push(index);
            }
        }
        // After the deepest of the field's intervals starting above it, so
        // the field's rows stay in depth order.
        let position = holding()
            .filter(|(_, other)| other.from < from)
            .max_by(|a, b| a.1.from.total_cmp(&b.1.from).then(a.0.cmp(&b.0)))
            .map(|(index, _)| index + 1)
            .or_else(|| holding().map(|(index, _)| index).next())
            .unwrap_or(drilled.intervals.len());
        let inserted = |key: (f64, f64), field: &str, name: DrillValue, row: Option<BTreeMap<String, DrillValue>>| Correction {
            before: None,
            row,
            ..Correction::proposed(&drilled.dhid, key, CorrectionKind::Insertion, field, name.clone(), name, note)
        };
        let codes = edit.codes();
        let row: BTreeMap<String, DrillValue> = codes.iter().map(|(key, value)| ((*key).to_owned(), DrillValue::Category((*value).to_owned()))).collect();
        // The row goes in with the first record; the other fields' records
        // name what it holds.
        for (index, (key, value)) in codes.iter().enumerate() {
            let record = inserted((from, to), key, DrillValue::Category((*value).to_owned()), (index == 0).then(|| row.clone()));
            plan.corrections.push((CorrectionTarget::interval(edit.hole, position), record));
        }
        for (offset, &index) in plan.split.clone().iter().enumerate() {
            let parent = &drilled.intervals[index];
            let Some(name) = parent.values.get(field).cloned() else { continue };
            let values = parent
                .values
                .iter()
                .filter(|(key, _)| !self.derived_columns.contains(*key))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            plan.corrections.push((
                CorrectionTarget::interval(edit.hole, position + 1 + offset),
                inserted((to, parent.to), field, name, Some(values)),
            ));
        }
        Ok(plan)
    }

    /// The other code fields of `field`'s table in hole `hole`, in the set's
    /// field order: categorical columns its rows hold that no Incline tool
    /// derives. An insertion can give each a code.
    pub(crate) fn table_fields(&self, hole: usize, field: &str) -> Vec<&DrillField> {
        let Some(drilled) = self.holes.get(hole) else {
            return Vec::new();
        };
        let columns = field_table(drilled, field, &self.derived_columns);
        self.fields
            .iter()
            .filter(|other| other.key != field && columns.contains(&other.key) && matches!(other.kind, DrillFieldKind::Categorical { .. }))
            .collect()
    }

    /// The codes inserting `edit` as `plan` sets out would put out of their
    /// field's column order in its hole, each field given a code judged
    /// against its own `column`, as [`Self::rename_out_of_sequence`] judges a
    /// renamed run, against the intervals around it once trimmed. An
    /// interval it splits is left out: a parting sits inside its seam, which
    /// is no question of order. Nothing is changed.
    pub(crate) fn insertion_out_of_sequence<'c>(&self, edit: &IntervalInsert, plan: &InsertionPlan, column: impl Fn(&str) -> &'c [String]) -> Vec<String> {
        let Some(drilled) = self.holes.get(edit.hole) else {
            return Vec::new();
        };
        // Where each interval starts once trimmed: a roof trim moves it.
        let mut starts: Vec<f64> = drilled.intervals.iter().map(|interval| interval.from).collect();
        for (target, record) in &plan.corrections {
            if record.kind == CorrectionKind::Boundary
                && record.field == ROOF_FIELD
                && let (Some(start), Some(DrillValue::Numeric(depth))) = (starts.get_mut(target.interval), record.after.as_ref())
            {
                *start = *depth;
            }
        }
        edit.codes()
            .into_iter()
            .filter(|(field, code)| {
                let mut runs: Vec<(f64, Option<&str>, bool)> = drilled
                    .intervals
                    .iter()
                    .enumerate()
                    .filter(|(index, interval)| !plan.split.contains(index) && interval.from.is_finite() && interval.to.is_finite())
                    .map(|(index, interval)| {
                        let name = match interval.values.get(*field) {
                            Some(DrillValue::Category(name)) => Some(name.as_str()),
                            _ => None,
                        };
                        (starts[index], name, false)
                    })
                    .collect();
                runs.push((edit.from, Some(*code), true));
                out_of_sequence(&runs, code, column(field))
            })
            .map(|(_, code)| code.to_owned())
            .collect()
    }

    /// The deletion taking interval `interval` of hole `hole`, named `dhid`,
    /// out of the interpreted data: one record keyed on its logged depths,
    /// holding the whole row so that undo puts it back exactly and what was
    /// logged stays. It names `field` and the code the row held there; where
    /// `field` is a column an Incline tool derives, the row's first other
    /// column holding a value instead, in column-name order, or no field at
    /// all. Every field the row holds goes with it. Deleting an inserted row
    /// is a deletion like any other, the insertion's record standing. `None`
    /// when the hole at that index is no longer the one named, or the row
    /// there is no longer `expected`, the row as it was picked. Nothing is
    /// changed here; the edit is the caller's.
    pub(crate) fn deletion_corrections(
        &self,
        hole: usize,
        dhid: &str,
        interval: usize,
        field: &str,
        expected: &DrillInterval,
        note: &CorrectionNote,
    ) -> Option<(CorrectionTarget, Correction)> {
        let drilled = self.holes.get(hole).filter(|drilled| drilled.dhid == dhid)?;
        let row = drilled.intervals.get(interval).filter(|row| same_interval(row, expected))?;
        let (logged_from, logged_to, _) = row.logged();
        let named = if self.derived_columns.contains(field) {
            row.values
                .iter()
                .find(|(key, value)| is_data(key, value, &self.derived_columns))
                .map_or("", |(key, _)| key.as_str())
        } else {
            field
        };
        let blank = DrillValue::Category(String::new());
        let record = Correction {
            before: row.values.get(named).cloned(),
            after: None,
            removed: Some(row.clone()),
            ..Correction::proposed(dhid, (logged_from, logged_to), CorrectionKind::Deletion, named, blank.clone(), blank, note)
        };
        Some((CorrectionTarget::interval(hole, interval), record))
    }

    /// Whether any hole of the set has a total depth: only then does a hole
    /// without one say so. A blast pattern, typed to a design depth, has none.
    pub(crate) fn records_total_depth(&self) -> bool {
        self.holes.iter().any(|hole| hole.total_depth.is_some())
    }

    /// Where `drilled` ends for a correction: its own end, or deeper where a
    /// row a deletion took out ran on past it, so the ground it freed reads
    /// as not logged and takes an insertion.
    pub(crate) fn end_of(&self, drilled: &DrillHole) -> f64 {
        self.deleted_rows(&drilled.dhid)
            .map(|row| row.to)
            .filter(|to| to.is_finite())
            .fold(drilled.end_depth(), f64::max)
    }

    /// The rows deletions have taken out of hole `hole` that the site had
    /// logged, for an as-logged view to keep showing them. An inserted row
    /// deleted again was never logged and is not among them.
    pub(crate) fn deleted_logged_rows(&self, hole: usize) -> Vec<&DrillInterval> {
        let Some(drilled) = self.holes.get(hole) else {
            return Vec::new();
        };
        self.deleted_rows(&drilled.dhid).filter(|row| !row.is_never_logged()).collect()
    }

    /// Every row deletions have taken out of the hole named `dhid`, oldest
    /// first, read through the index rather than every record.
    fn deleted_rows<'a>(&'a self, dhid: &'a str) -> impl Iterator<Item = &'a DrillInterval> + 'a {
        let deletions = self.deletions.get_or_init(|| {
            let mut deletions: HashMap<String, Vec<usize>> = HashMap::new();
            for (index, record) in self.corrections.iter().enumerate() {
                if record.kind == CorrectionKind::Deletion && record.removed.is_some() {
                    deletions.entry(record.hole.clone()).or_default().push(index);
                }
            }
            deletions
        });
        deletions
            .get(dhid)
            .into_iter()
            .flatten()
            .filter_map(|&index| self.corrections.get(index))
            .filter(move |record| record.kind == CorrectionKind::Deletion && record.hole == dhid)
            .filter_map(|record| record.removed.as_ref())
    }

    /// Work out again what is read from the records after they changed: the
    /// deletions index, and the fields.
    pub(crate) fn records_changed(&mut self) {
        self.deletions = std::sync::OnceLock::new();
        self.refresh_fields();
    }

    /// Collect the fields again. A field only logged rows deletions took out
    /// still hold, as logged, is kept, so a deletion never takes a column
    /// away; fields the holes' rows hold are theirs alone, and a column a tool
    /// derives is not logged data.
    fn refresh_fields(&mut self) {
        let mut fields = collect_fields(&self.holes);
        let kept: Vec<DrillField> = fields_of([self.deleted_logged_values()])
            .into_iter()
            .filter(|field| !self.derived_columns.contains(&field.key) && fields.iter().all(|live| live.key != field.key))
            .collect();
        if !kept.is_empty() {
            fields.extend(kept);
            fields.sort_by(|a, b| crate::natural_sort::natural_cmp(&a.label, &b.label));
        }
        self.fields = fields;
    }

    /// The values of each logged row deletions took out, as logged.
    fn deleted_logged_values(&self) -> impl Iterator<Item = &BTreeMap<String, DrillValue>> {
        self.corrections
            .iter()
            .filter(|record| record.kind == CorrectionKind::Deletion)
            .filter_map(|record| record.removed.as_ref())
            .filter(|row| !row.is_never_logged())
            .map(|row| row.logged().2)
    }

    /// Take the derived columns a file names, keeping those some row still
    /// holds: one of the holes', or any row a deletion took out, logged or
    /// inserted, either side, which keeps the column the tool's for an undo.
    /// A name no row holds is dropped, since a later import of that name
    /// would otherwise be written over.
    pub(crate) fn restore_derived_columns(&mut self, stored: BTreeSet<String>) {
        let deleted = || {
            self.corrections
                .iter()
                .filter(|record| record.kind == CorrectionKind::Deletion)
                .filter_map(|record| record.removed.as_ref())
        };
        let held = |column: &String| {
            self.holes.iter().any(|hole| hole.intervals.iter().any(|interval| interval.values.contains_key(column)))
                || deleted().any(|row| row.values.contains_key(column) || row.logged().2.contains_key(column))
        };
        self.derived_columns = stored.into_iter().filter(held).collect();
        self.records_changed();
    }

    /// Which way the round crossed each tie, given [`Self::firing_times`],
    /// in the order of [`Self::ties`].
    pub(crate) fn tie_flows(&self, times: &[Option<u32>]) -> Vec<TieFlow> {
        let at = |hole: usize| times.get(hole).copied().flatten();
        self.ties
            .iter()
            .map(|tie| match (at(tie.a), at(tie.b)) {
                (None, None) => TieFlow::Unreached,
                (Some(a), Some(b)) if a.saturating_add(tie.delay_ms) == b => TieFlow::AToB,
                (Some(a), Some(b)) if b.saturating_add(tie.delay_ms) == a => TieFlow::BToA,
                _ => TieFlow::Redundant,
            })
            .collect()
    }

    /// The value corrections sliding the names of `field` in hole `hole` one
    /// run along the hole. A run is one name over consecutive intervals that
    /// name a code of `column`; intervals with no name, a name the column
    /// lacks, or [`UNKNOWN_NAME`] keep theirs, are counted, and are not part
    /// of the slide. Up, each run takes the name of the run below it and the
    /// bottom run the column's name after the hole's last; down is the
    /// mirror. With no such column name the end run is named
    /// [`UNKNOWN_NAME`].
    ///
    /// With `from`, an interval index, only its horizon ([`Self::horizon`]),
    /// a run of its own as the log draws it, and the runs on the side the
    /// names move to slide: up, the runs above it each take the name of the
    /// run below, and the horizon is named [`UNKNOWN_NAME`], opened for a
    /// name to be put in; the runs below are untouched. Down is the mirror.
    /// A `from` in no run slides nothing.
    ///
    /// Only intervals whose name changes get a record; depths never move,
    /// and an interval missing a depth is left out as a rename leaves it
    /// out. Nothing is changed here; the edit is the caller's.
    pub(crate) fn column_shift(&self, hole: usize, field: &str, column: &[String], direction: ShiftDirection, from: Option<usize>, note: &CorrectionNote) -> ColumnShift {
        let mut shift = ColumnShift::default();
        let Some(drilled) = self.holes.get(hole) else {
            return shift;
        };
        let mut named: Vec<(usize, &String)> = Vec::new();
        for (index, interval) in drilled.intervals.iter().enumerate() {
            if !(interval.from.is_finite() && interval.to.is_finite()) {
                continue;
            }
            let Some(DrillValue::Category(code)) = interval.values.get(field) else {
                continue;
            };
            if code != UNKNOWN_NAME && column.contains(code) {
                named.push((index, code));
            } else {
                shift.untouched += 1;
            }
        }
        named.sort_by(|a, b| drilled.intervals[a.0].from.total_cmp(&drilled.intervals[b.0].from).then(a.0.cmp(&b.0)));
        let horizon = from.map(|interval| self.horizon(hole, field, interval)).unwrap_or_default();
        let mut runs: Vec<(&str, Vec<usize>)> = Vec::new();
        let mut last_inside = false;
        for (index, code) in named {
            let inside = horizon.contains(&index);
            match runs.last_mut() {
                Some((name, members)) if *name == code.as_str() && inside == last_inside => members.push(index),
                _ => runs.push((code.as_str(), vec![index])),
            }
            last_inside = inside;
        }
        let clicked = match from {
            Some(interval) => match runs.iter().position(|(_, members)| members.contains(&interval)) {
                Some(at) => Some(at),
                None => return shift,
            },
            None => None,
        };
        let place = |code: &str| column.iter().position(|held| held == code);
        let last = runs.len().saturating_sub(1);
        for (at, (old, members)) in runs.iter().enumerate() {
            let new: Option<&str> = match (direction, clicked) {
                (ShiftDirection::Up, Some(clicked)) if at > clicked => continue,
                (ShiftDirection::Down, Some(clicked)) if at < clicked => continue,
                (_, Some(clicked)) if at == clicked => None,
                (ShiftDirection::Up, _) if at < last => Some(runs[at + 1].0),
                (ShiftDirection::Up, _) => place(old).and_then(|place| column.get(place + 1)).map(String::as_str),
                (ShiftDirection::Down, _) if at > 0 => Some(runs[at - 1].0),
                (ShiftDirection::Down, _) => place(old).and_then(|place| place.checked_sub(1)).and_then(|place| column.get(place)).map(String::as_str),
            };
            if new == Some(*old) {
                continue;
            }
            let new = new.unwrap_or_else(|| {
                shift.unknown += members.len();
                UNKNOWN_NAME
            });
            for &index in members {
                let (logged_from, logged_to, _) = drilled.intervals[index].logged();
                let record = Correction::proposed(
                    &drilled.dhid,
                    (logged_from, logged_to),
                    CorrectionKind::Value,
                    field,
                    DrillValue::Category((*old).to_owned()),
                    DrillValue::Category(new.to_owned()),
                    note,
                );
                shift.corrections.push((CorrectionTarget::interval(hole, index), record));
            }
        }
        shift.corrections.sort_by_key(|(target, _)| target.interval);
        shift
    }

    /// Write each correction's `after` and append its record: a value
    /// correction's as its interval's interpreted value, a boundary's as its
    /// interval's from or to and its follower's from, an insertion's as a new
    /// row marked never logged. The as-logged interval is kept aside the
    /// first time anything parts the two, and is never written after that.
    pub(crate) fn apply_corrections(&mut self, targets: &[CorrectionTarget], records: &[Correction]) {
        let mut not_applied = Tally::default();
        for (&target, record) in targets.iter().zip(records) {
            // An insertion or deletion whose record is still in the trail is
            // still in place, its undo having failed: writing it again would
            // put in a second row or refuse, and add a second record.
            if matches!(record.kind, CorrectionKind::Insertion | CorrectionKind::Deletion) && self.corrections.iter().any(|kept| same_record(kept, record)) {
                continue;
            }
            let written = self.holes.get_mut(target.hole).is_some_and(|hole| write_correction(hole, target, record, true));
            if written {
                self.corrections.push(record.clone());
            } else if record.kind == CorrectionKind::Deletion {
                not_applied.add(record);
            }
        }
        if let Some((count, hole, from, to)) = not_applied.first() {
            crate::userspace_warn!("{}", tr!("drill-hole-deletion-not-applied", count = count, hole = hole, from = from, to = to));
        }
        self.records_changed();
    }

    /// Write cells of a derived `column`, each `(hole, interval, value)`, and
    /// mark the column derived or not. A derived value is no correction: it is
    /// written beside an interval's logged values too, so it never makes the
    /// interval read as corrected.
    pub(crate) fn write_column(&mut self, column: &str, cells: &[(usize, usize, Option<DrillValue>)], derived: bool) {
        for (hole, interval, value) in cells {
            let Some(interval) = self.holes.get_mut(*hole).and_then(|hole| hole.intervals.get_mut(*interval)) else {
                continue;
            };
            set_value(&mut interval.values, column, value.as_ref());
            if let Some(logged) = interval.logged.as_mut().filter(|logged| !logged.never_logged) {
                set_value(&mut logged.values, column, value.as_ref());
            }
        }
        if derived {
            self.derived_columns.insert(column.to_owned());
        } else {
            self.derived_columns.remove(column);
        }
        self.refresh_fields();
    }

    /// Undo [`Self::apply_corrections`]: the `before` values and depths go
    /// back and the records are withdrawn, latest first. A record whose undo
    /// could not be written stays, as what it changed is still there. What
    /// could not be undone as asked is said once per outcome, with a count.
    pub(crate) fn withdraw_corrections(&mut self, targets: &[CorrectionTarget], records: &[Correction]) {
        let (mut rows_kept, mut kept, mut lost_back, mut lost) = (Tally::default(), Tally::default(), Tally::default(), Tally::default());
        for (&target, record) in targets.iter().zip(records).rev() {
            let position = self.corrections.iter().rposition(|kept| same_record(kept, record));
            if record.kind == CorrectionKind::Deletion && position.is_none() {
                // A deletion with no record never took its row out, which was
                // said when it was applied, or its record was left out when
                // the file was read, which the read said: nothing to put back.
                continue;
            }
            let written = self.holes.get_mut(target.hole).is_some_and(|hole| write_correction(hole, target, record, false));
            if written
                && !matches!(record.kind, CorrectionKind::Insertion | CorrectionKind::Deletion)
                && let Some(hole) = self.holes.get_mut(target.hole)
            {
                for index in [Some(target.interval), target.below].into_iter().flatten() {
                    let interval = &mut hole.intervals[index];
                    if !interval.is_corrected() {
                        interval.logged = None;
                    }
                }
            }
            match (position, written) {
                (Some(position), true) => {
                    self.corrections.remove(position);
                }
                // A deleted row that could not go back keeps its record, so
                // the row it holds is not lost.
                (Some(_), false) if record.kind == CorrectionKind::Deletion => rows_kept.add(record),
                (Some(_), false) => kept.add(record),
                (None, true) => lost_back.add(record),
                (None, false) => lost.add(record),
            }
        }
        if let Some((count, hole, from, to)) = rows_kept.first() {
            crate::userspace_warn!("{}", tr!("drill-hole-deletion-not-undone", count = count, hole = hole, from = from, to = to));
        }
        if let Some((count, hole, from, to)) = kept.first() {
            crate::userspace_warn!("{}", tr!("drill-hole-correction-not-undone", count = count, hole = hole, from = from, to = to));
        }
        if let Some((count, hole, from, to)) = lost_back.first() {
            crate::userspace_warn!("{}", tr!("drill-hole-undo-no-record", count = count, hole = hole, from = from, to = to));
        }
        if let Some((count, hole, from, to)) = lost.first() {
            crate::userspace_warn!("{}", tr!("drill-hole-undo-no-record-not-back", count = count, hole = hole, from = from, to = to));
        }
        self.records_changed();
    }

    /// The ties as a file holds them, keyed by hole name.
    pub(crate) fn stored_ties(&self) -> StoredTieIns {
        let name = |index: usize| self.holes.get(index).map(|hole| hole.dhid.clone());
        StoredTieIns {
            ties: self
                .ties
                .iter()
                .filter_map(|tie| {
                    Some(StoredTieIn {
                        a: name(tie.a)?,
                        b: name(tie.b)?,
                        delay_ms: tie.delay_ms,
                        product: tie.product.clone(),
                        color: tie.color,
                    })
                })
                .collect(),
            initiations: self
                .initiations
                .iter()
                .filter_map(|initiation| {
                    Some(StoredInitiation {
                        hole: name(initiation.hole)?,
                        delay_ms: initiation.delay_ms,
                    })
                })
                .collect(),
            initiation: None,
        }
    }

    /// Resolve stored ties back onto this dataset's holes, reporting how many
    /// were dropped because a hole they named is no longer here. Called after
    /// construction, since it is [`Self::new`] that fixes the hole order the
    /// indices are against.
    pub(crate) fn apply_stored_ties(&mut self, stored: StoredTieIns) -> usize {
        let index_of = |name: &str| self.holes.iter().position(|hole| hole.dhid == name);
        let mut dropped = 0;
        self.ties = stored
            .ties
            .into_iter()
            .filter_map(|tie| {
                let (Some(a), Some(b)) = (index_of(&tie.a), index_of(&tie.b)) else {
                    dropped += 1;
                    return None;
                };
                Some(TieIn {
                    a,
                    b,
                    delay_ms: tie.delay_ms,
                    product: tie.product,
                    color: tie.color,
                })
            })
            .collect();
        let stored_initiations = stored.initiations.into_iter().chain(stored.initiation);
        self.initiations = stored_initiations
            .filter_map(|initiation| {
                let Some(hole) = index_of(&initiation.hole) else {
                    dropped += 1;
                    return None;
                };
                Some(Initiation {
                    hole,
                    delay_ms: initiation.delay_ms,
                })
            })
            .collect();
        // A collar has one editable initiation card. If a malformed file
        // names it twice, keep the last value rather than drawing stacked
        // cards or feeding duplicate sources into the firing graph.
        self.initiations.reverse();
        let mut seen = std::collections::HashSet::new();
        self.initiations.retain(|initiation| seen.insert(initiation.hole));
        self.initiations.reverse();
        dropped
    }

    /// The charges as a file holds them, keyed by hole name.
    pub(crate) fn stored_charges(&self) -> crate::model::blast::StoredCharges {
        crate::model::blast::StoredCharges {
            holes: self
                .charges
                .iter()
                .filter_map(|(index, charge)| {
                    Some(crate::model::blast::StoredCharge {
                        hole: self.holes.get(*index)?.dhid.clone(),
                        charge: charge.clone(),
                    })
                })
                .collect(),
        }
    }

    /// Resolve stored charges back onto this dataset's holes, reporting how
    /// many named a hole that is no longer here.
    pub(crate) fn apply_stored_charges(&mut self, stored: crate::model::blast::StoredCharges) -> usize {
        let index: std::collections::HashMap<&str, usize> = self.holes.iter().enumerate().map(|(index, hole)| (hole.dhid.as_str(), index)).collect();
        let mut dropped = 0;
        let mut charges = BTreeMap::new();
        for entry in stored.holes {
            match index.get(entry.hole.as_str()) {
                Some(hole) => {
                    charges.insert(*hole, entry.charge);
                }
                None => dropped += 1,
            }
        }
        self.charges = charges;
        dropped
    }

    /// Approximate retained size, used by the undo history's memory budget.
    pub(crate) fn estimated_bytes(&self) -> usize {
        size_of::<Self>()
            + self
                .holes
                .iter()
                .map(|hole| {
                    size_of::<DrillHole>()
                        + hole.dhid.len()
                        + hole.trace.len() * size_of::<TraceStation>()
                        + hole.render_ranges.len() * size_of::<(f64, f64)>()
                        + hole
                            .intervals
                            .iter()
                            .map(|interval| size_of::<DrillInterval>() + values_bytes(&interval.values) + interval.logged.as_ref().map_or(0, |logged| values_bytes(&logged.values)))
                            .fold(0usize, usize::saturating_add)
                })
                .fold(0usize, usize::saturating_add)
            + self.hole_boxes.len() * size_of::<WorldBox>()
            + self.ties.iter().map(|tie| size_of::<TieIn>() + tie.product.len()).fold(0usize, usize::saturating_add)
            + self.initiations.len() * size_of::<Initiation>()
            + self.corrections.iter().map(Correction::estimated_bytes).fold(0usize, usize::saturating_add)
            + self
                .fields
                .iter()
                .map(|field| {
                    size_of::<DrillField>()
                        + field.key.len()
                        + field.label.len()
                        + match &field.kind {
                            DrillFieldKind::Categorical { categories } => categories.iter().map(|value| size_of::<String>() + value.len()).fold(0usize, usize::saturating_add),
                            DrillFieldKind::Numeric { .. } => 0,
                        }
                })
                .fold(0usize, usize::saturating_add)
            + self
                .charges
                .values()
                .map(|charge| size_of::<usize>() + charge.estimated_bytes())
                .fold(0usize, usize::saturating_add)
    }

    pub(crate) fn field(&self, key: &str) -> Option<&DrillField> {
        self.fields.iter().find(|field| field.key == key)
    }

    pub(crate) fn refresh_bounds(&mut self) {
        let (bounds, hole_boxes) = drill_extents(&self.holes);
        self.bounds = bounds;
        self.hole_boxes = hole_boxes;
        debug_assert_eq!(self.hole_boxes.len(), self.holes.len());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum DrillColorPreset {
    Rainbow,
    Grayscale,
    Heat,
    GreenYellowRed,
}

impl DrillColorPreset {
    pub(crate) const ALL: [Self; 4] = [Self::Rainbow, Self::Grayscale, Self::Heat, Self::GreenYellowRed];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Rainbow => crate::i18n::tr!("drill-hole-rainbow"),
            Self::Grayscale => crate::i18n::tr!("drill-hole-grayscale"),
            Self::Heat => crate::i18n::tr!("drill-hole-heat"),
            Self::GreenYellowRed => crate::i18n::tr!("drill-hole-green-yellow-red"),
        }
    }

    pub(crate) fn smooth(self) -> bool {
        matches!(self, Self::Rainbow | Self::Grayscale | Self::Heat)
    }

    pub(crate) fn stops(self) -> Vec<DrillColorStop> {
        let colors: &[[f32; 3]] = match self {
            Self::Rainbow => &[[0.05, 0.20, 0.95], [0.00, 0.85, 0.95], [0.05, 0.82, 0.20], [1.00, 0.85, 0.00], [0.92, 0.02, 0.02]],
            Self::Grayscale => &[[0.05, 0.05, 0.05], [0.95, 0.95, 0.95]],
            Self::Heat => &[[0.02, 0.02, 0.02], [0.85, 0.00, 0.00], [1.00, 0.78, 0.00], [1.00, 1.00, 0.92]],
            Self::GreenYellowRed => &[[0.00, 0.82, 0.20], [1.00, 0.85, 0.00], [0.92, 0.00, 0.00]],
        };
        let denom = if self.smooth() { colors.len().saturating_sub(1).max(1) } else { colors.len().max(1) } as f32;
        colors
            .iter()
            .enumerate()
            .map(|(index, color)| DrillColorStop {
                t: index as f32 / denom,
                color: *color,
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillColorStop {
    pub(crate) t: f32,
    pub(crate) color: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillCategoryColor {
    pub(crate) value: String,
    pub(crate) color: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec<DrillCategoryColor>", into = "Vec<DrillCategoryColor>")]
pub(crate) struct CategoryTable {
    entries: Vec<DrillCategoryColor>,
    hash: u64,
}

impl CategoryTable {
    fn new(mut entries: Vec<DrillCategoryColor>) -> Self {
        entries.sort_by(|a, b| a.value.cmp(&b.value));
        entries.dedup_by(|a, b| a.value == b.value);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for entry in &entries {
            entry.value.hash(&mut hasher);
            for channel in entry.color {
                channel.to_bits().hash(&mut hasher);
            }
        }
        Self { entries, hash: hasher.finish() }
    }

    pub(crate) fn content_hash(&self) -> u64 {
        self.hash
    }
}

impl From<Vec<DrillCategoryColor>> for CategoryTable {
    fn from(entries: Vec<DrillCategoryColor>) -> Self {
        Self::new(entries)
    }
}

impl From<CategoryTable> for Vec<DrillCategoryColor> {
    fn from(table: CategoryTable) -> Self {
        table.entries
    }
}

impl Default for CategoryTable {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl std::ops::Deref for CategoryTable {
    type Target = [DrillCategoryColor];
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DrillColorState {
    pub(crate) active_field: Option<String>,
    pub(crate) preset: DrillColorPreset,
    pub(crate) smooth: bool,
    pub(crate) stops: Vec<DrillColorStop>,
    pub(crate) categories: CategoryTable,
    /// Multiplies the drilled diameter where a hole is drawn: at true width a
    /// hole reads as a pipe beside the geology and a set of thousands as a
    /// mat, so how wide a set draws is the set's to choose.
    #[serde(default = "default_radius_scale", deserialize_with = "clamped_radius_scale")]
    pub(crate) radius_scale: f64,
    /// Narrowest a hole is drawn, whatever the scale above and however far
    /// the eye is: below this it would flicker out rather than thin.
    #[serde(default = "default_min_pixel_diameter", deserialize_with = "clamped_min_pixel_diameter")]
    pub(crate) min_pixel_diameter: f32,
    /// Named groups of codes mined together, kept per source field. They
    /// travel with the colour state so a project written before them loads
    /// with none.
    #[serde(default, deserialize_with = "lenient_working_sections")]
    pub(crate) working_sections: Vec<WorkingSection>,
    /// Colour the active field by working section: a code in a group takes
    /// the group's colour, any other code keeps its own.
    #[serde(default)]
    pub(crate) by_working_section: bool,
    /// The order of each categorical field's codes down the deposit, kept
    /// beside the working sections and read as leniently, so a project
    /// written before there was one loads with none.
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "lenient_strat_columns")]
    pub(crate) strat_columns: Vec<StratColumn>,
    /// The field the strat column reads until one is picked by hand: the
    /// parent (seam) field an import found, else the field it ordered.
    /// Written by import only and read as leniently as the columns, so a
    /// project written before it loads with none.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient_strat_field")]
    pub(crate) strat_field: Option<String>,
    /// How the set is drawn. A set saved before there was a choice reads
    /// back at its true diameter, the look it was saved with; an imported
    /// set starts as string and discs (see `Self::for_logged_holes`).
    #[serde(default = "saved_before_styles_hole_style", deserialize_with = "lenient_hole_style")]
    pub(crate) hole_style: DrillHoleStyle,
    /// The discs' diameter in metres, drawn string and discs.
    #[serde(default = "default_disc_diameter", deserialize_with = "clamped_disc_diameter")]
    pub(crate) disc_diameter: f64,
    /// The string's width on screen in pixels, drawn string and discs: it
    /// neither thins nor thickens with zoom.
    #[serde(default = "default_string_pixel_width", deserialize_with = "clamped_string_pixel_width")]
    pub(crate) string_pixel_width: f32,
}

/// How a drill hole set is drawn in the 3D view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum DrillHoleStyle {
    /// A cylinder at the drilled diameter times the set's width scale.
    TrueDiameter,
    /// A thin string of constant screen width along the trace, and a disc of
    /// a chosen diameter on every interval with a value in the colour field.
    StringAndDiscs,
}

impl DrillHoleStyle {
    pub(crate) const ALL: [Self; 2] = [Self::TrueDiameter, Self::StringAndDiscs];

    pub(crate) fn label(self) -> String {
        match self {
            Self::TrueDiameter => tr!("drill-hole-true-diameter"),
            Self::StringAndDiscs => tr!("drill-hole-string-discs"),
        }
    }
}

/// Seams or plies of one categorical field that are mined as one unit.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct WorkingSection {
    pub(crate) name: String,
    pub(crate) field: String,
    pub(crate) codes: Vec<String>,
}

/// One categorical field's codes in order down the deposit, seams, plies and
/// splits alike, as the geologist placed them. Nothing here is inferred.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct StratColumn {
    pub(crate) field: String,
    pub(crate) codes: Vec<String>,
}

/// The name a shift gives a run it leaves with no name: unknown, until it
/// is renamed. Never a strat column entry.
pub(crate) const UNKNOWN_NAME: &str = "UNK";

/// Which way a whole-column shift moves a hole's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShiftDirection {
    /// The hole's names slide up one run: each run takes the name of the run
    /// below it, the bottom run the column's next name.
    Up,
    /// The mirror: each run takes the name of the run above it, the top run
    /// the column's name before the hole's first.
    Down,
}

/// What a whole-column shift of one hole would do, worked out from its names
/// as they stand, so no name is shifted twice.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ColumnShift {
    /// One value correction per interval whose name moves, with its hole and
    /// interval index.
    pub(crate) corrections: Vec<(CorrectionTarget, Correction)>,
    /// Of those, the intervals named [`UNKNOWN_NAME`]: the run a shift
    /// from a horizon opens, or the end run when the column has no name past
    /// the hole's last (or first).
    pub(crate) unknown: usize,
    /// Intervals naming a code the column does not hold, left alone.
    pub(crate) untouched: usize,
}

impl ColumnShift {
    /// Each distinct rename the shift makes, old name to new, a name
    /// replaced by [`UNKNOWN_NAME`] left out: no section or colour follows it.
    pub(crate) fn renames(&self) -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for (_, record) in &self.corrections {
            if let (Some(DrillValue::Category(from)), Some(DrillValue::Category(to))) = (&record.before, &record.after)
                && to != UNKNOWN_NAME
                && !pairs.iter().any(|(known, _)| known == from)
            {
                pairs.push((from.clone(), to.clone()));
            }
        }
        pairs
    }
}

/// `sections` after the renames of a shift, each judged against the
/// sections as they stood, as [`sections_after_rename`] judges one: a
/// section holding an old name takes the new one in its place, or beside it
/// while `remains` says an interval still holds the old one, unless some
/// section of the field already holds the new name. Judged together so no
/// section gains a name by way of another rename.
pub(crate) fn sections_after_shift(sections: &[WorkingSection], field: &str, renames: &[(String, String)], remains: impl Fn(&str) -> bool) -> Vec<WorkingSection> {
    let held = |code: &str| sections.iter().any(|section| section.field == field && section.codes.iter().any(|member| member == code));
    let mut after = sections.to_vec();
    for section in after.iter_mut().filter(|section| section.field == field) {
        let mut codes = Vec::with_capacity(section.codes.len());
        for code in &section.codes {
            match renames.iter().find(|(from, _)| from == code).map(|(_, to)| to) {
                Some(to) if !held(to) => {
                    if remains(code) {
                        codes.push(code.clone());
                    }
                    codes.push(to.clone());
                }
                _ => codes.push(code.clone()),
            }
        }
        section.codes = codes;
    }
    after
}

/// Why a working section cannot be kept as named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SectionProblem {
    /// Blank once trimmed.
    NoName,
    /// A code of the field outside the section has the name, ignoring
    /// case: a pick under it could mean either. A code inside it may share
    /// the name, as a seam's section is named after the seam.
    NameIsCode,
    /// As [`Self::NameIsCode`], where the code of that name is outside
    /// because another section already holds it.
    CodeClaimed { code: String, section: String },
    /// Another section of the field has the name, ignoring case.
    DuplicateName,
    /// Every code it lists is blank or already in an earlier section of the
    /// field.
    NoCodes,
}

impl SectionProblem {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::NoName => tr!("drill-hole-working-section-needs-name"),
            Self::NameIsCode => tr!("drill-hole-code-outside-section-has-name"),
            Self::CodeClaimed { code, section } => tr!("drill-hole-code-already-in-section", code = code.clone().to_string(), section = section.clone().to_string()),
            Self::DuplicateName => tr!("drill-hole-another-working-section-field-has"),
            Self::NoCodes => tr!("drill-hole-every-code-lists-already-another"),
        }
    }
}

/// A section [`tidy_working_sections`] left out, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DroppedSection {
    pub(crate) name: String,
    pub(crate) problem: SectionProblem,
}

/// Case-blind comparison of two names, without allocating.
fn same_name(a: &str, b: &str) -> bool {
    a.chars().flat_map(char::to_lowercase).eq(b.chars().flat_map(char::to_lowercase))
}

/// The working section of `field` named like `code` without holding it: a
/// code renamed to `code` would cost that section its name on the next open.
pub(crate) fn section_named_apart<'a>(sections: &'a [WorkingSection], field: &str, code: &str) -> Option<&'a WorkingSection> {
    sections
        .iter()
        .find(|section| section.field == field && same_name(&section.name, code) && !section.codes.iter().any(|held| held == code))
}

/// The working section an insertion's codes would cost its name, with the
/// code that would, as [`section_named_apart`] finds for a rename: checked
/// for the field picked and for every other field given a code.
pub(crate) fn insertion_named_apart<'a, 'e>(sections: &'a [WorkingSection], edit: &'e IntervalInsert) -> Option<(&'a WorkingSection, &'e str)> {
    edit.codes()
        .into_iter()
        .find_map(|(field, code)| section_named_apart(sections, field, code).map(|section| (section, code)))
}

/// What is said when `code` is named like `section` without being one of
/// its codes: the code, its field and the section. The insert dialog shows
/// it before anything is sent, and a rename, a shift or an insertion
/// reaching the App is refused with it.
pub(crate) fn name_is_code_message(section: &WorkingSection, code: &str) -> String {
    tr!(
        "drill-hole-section-name-is-code",
        code = code.to_owned(),
        field = section.field.clone(),
        name = section.name.clone()
    )
}

/// `sections` after `from` is renamed `to` in `field`: a section holding
/// `from` takes `to` in its place, or beside it while an interval still holds
/// `from`, so the renamed intervals keep their pick. A `to` some section of
/// the field already holds stays where it is.
pub(crate) fn sections_after_rename(sections: &[WorkingSection], field: &str, from: &str, to: &str, from_remains: bool) -> Vec<WorkingSection> {
    let mut sections = sections.to_vec();
    if sections.iter().any(|section| section.field == field && section.codes.iter().any(|code| code == to)) {
        return sections;
    }
    for section in sections.iter_mut().filter(|section| section.field == field) {
        let Some(at) = section.codes.iter().position(|code| code == from) else {
            continue;
        };
        if from_remains {
            section.codes.insert(at + 1, to.to_owned());
        } else {
            section.codes[at] = to.to_owned();
        }
    }
    sections
}

/// What stops `name` naming a new working section of `field` holding
/// `members`. The editor asks before offering to add one and
/// [`tidy_working_sections`] before keeping one, so the two agree.
pub(crate) fn working_section_name_problem(name: &str, field: &str, codes: &[String], members: &[String], sections: &[WorkingSection]) -> Option<SectionProblem> {
    let name = name.trim();
    if name.is_empty() {
        return Some(SectionProblem::NoName);
    }
    if let Some(code) = codes.iter().find(|code| same_name(code, name) && !members.contains(code)) {
        // Say where the code went when another section has it: that is the
        // cause, not the name.
        return Some(
            sections
                .iter()
                .find(|section| section.field == field && section.codes.contains(code))
                .map_or(SectionProblem::NameIsCode, |section| SectionProblem::CodeClaimed {
                    code: code.clone(),
                    section: section.name.clone(),
                }),
        );
    }
    if sections.iter().any(|section| section.field == field && same_name(&section.name, name)) {
        Some(SectionProblem::DuplicateName)
    } else {
        None
    }
}

/// The codes `fields` lists for `key`, or none for a field that is not
/// categorical or not there.
pub(crate) fn field_codes<'a>(fields: &'a [DrillField], key: &str) -> &'a [String] {
    match fields.iter().find(|field| field.key == key).map(|field| &field.kind) {
        Some(DrillFieldKind::Categorical { categories }) => categories,
        _ => &[],
    }
}

/// The codes a field offers to be typed, in the order a geologist reads them:
/// its strat `column` first, in the column's order, then any other of its
/// `codes` the column lacks, in their own order. Every code is offered.
pub(crate) fn offered_codes(column: &[String], codes: &[String]) -> Vec<String> {
    column.iter().chain(codes.iter().filter(|code| !column.contains(code))).cloned().collect()
}

/// A list of working sections as the app keeps it: names trimmed, each code
/// in at most one section per field (the first that claims it), and no
/// section kept whose name is blank, is a code of its field it does not
/// hold, or repeats another's, ignoring case. What is left out comes back
/// with the reason. A section of a field `fields` does not hold is kept as
/// it is, as a colour for a code a shorter extract lacks is.
pub(crate) fn tidy_working_sections(sections: Vec<WorkingSection>, fields: &[DrillField]) -> (Vec<WorkingSection>, Vec<DroppedSection>) {
    let mut kept: Vec<WorkingSection> = Vec::new();
    let mut dropped = Vec::new();
    for mut section in sections {
        section.name = section.name.trim().to_owned();
        // The codes first: whether the name may be one of them depends on
        // which of them this section still holds once earlier ones claimed
        // theirs.
        let mut codes: Vec<String> = Vec::new();
        for code in std::mem::take(&mut section.codes) {
            let claimed = kept.iter().any(|other| other.field == section.field && other.codes.contains(&code));
            if !code.trim().is_empty() && !claimed && !codes.contains(&code) {
                codes.push(code);
            }
        }
        if let Some(problem) = working_section_name_problem(&section.name, &section.field, field_codes(fields, &section.field), &codes, &kept) {
            dropped.push(DroppedSection { name: section.name, problem });
            continue;
        }
        if codes.is_empty() {
            dropped.push(DroppedSection {
                name: section.name,
                problem: SectionProblem::NoCodes,
            });
            continue;
        }
        section.codes = codes;
        kept.push(section);
    }
    (kept, dropped)
}

/// Working sections the names of `field`'s codes suggest, for the editor to
/// offer, never applied unasked. A root is a code of two or more characters
/// that other codes begin with; only the shortest roots count, so no two
/// suggestions share a code. Each holds its root and every code beginning
/// with it. A common beginning that is not itself a code (KAL1A and KAL4B,
/// no KAL) or a code of digits alone is no root, and a code carrying on a
/// root's number (P10 after P1) is not under it. Codes already in a section
/// take no part, and what is offered is what [`tidy_working_sections`] keeps.
pub(crate) fn suggested_working_sections(field: &str, codes: &[String], sections: &[WorkingSection]) -> Vec<WorkingSection> {
    let taken = |code: &String| sections.iter().any(|section| section.field == field && section.codes.contains(code));
    let mut free: Vec<&String> = codes.iter().filter(|code| !code.trim().is_empty() && !taken(code)).collect();
    free.sort_unstable();
    free.dedup();
    // A code that carries on a root's number is not under it: P10 is not a
    // ply of P1.
    let extends = |root: &str, code: &str| {
        code.starts_with(root) && !(root.ends_with(|character: char| character.is_ascii_digit()) && code[root.len()..].starts_with(|character: char| character.is_ascii_digit()))
    };
    // Sorted, every code beginning with a root follows the root directly, so
    // one pass finds each shortest root and the codes under it.
    let mut suggested = Vec::new();
    let mut consumed = vec![false; free.len()];
    for index in 0..free.len() {
        if consumed[index] {
            continue;
        }
        let root = free[index];
        let span = free[index + 1..].iter().take_while(|code| code.starts_with(root.as_str())).count();
        let numeric = root.trim().chars().all(|character| character.is_ascii_digit() || character == '.');
        if span == 0 || root.trim().chars().count() < 2 || numeric {
            continue;
        }
        let under: Vec<usize> = (index + 1..=index + span).filter(|&other| !consumed[other] && extends(root, free[other])).collect();
        if under.is_empty() {
            continue;
        }
        consumed[index] = true;
        for &other in &under {
            consumed[other] = true;
        }
        let mut section_codes: Vec<String> = Vec::new();
        for code in codes {
            let member = code == root || under.iter().any(|&other| free[other] == code);
            if member && !section_codes.contains(code) {
                section_codes.push(code.clone());
            }
        }
        suggested.push((root, section_codes));
    }
    suggested.sort_by_key(|(root, _)| codes.iter().position(|code| code == *root));
    let mut offered: Vec<WorkingSection> = Vec::new();
    for (root, section_codes) in suggested {
        let name = root.trim();
        if working_section_name_problem(name, field, codes, &section_codes, sections).is_none() && !offered.iter().any(|other| same_name(&other.name, name)) {
            offered.push(WorkingSection {
                name: name.to_owned(),
                field: field.to_owned(),
                codes: section_codes,
            });
        }
    }
    offered
}

/// Reads the working sections an entry at a time, so one malformed entry,
/// or a value that is not a list at all, costs only itself and not the
/// colours saved beside it. [`skipped_working_sections`] counts what this
/// passes over, for the loader to report.
fn lenient_working_sections<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<WorkingSection>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        Section(WorkingSection),
        Unreadable(serde::de::IgnoredAny),
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum List {
        Entries(Vec<Entry>),
        Unreadable(serde::de::IgnoredAny),
    }
    Ok(match List::deserialize(deserializer)? {
        List::Entries(entries) => entries
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Section(section) => Some(section),
                Entry::Unreadable(_) => None,
            })
            .collect(),
        List::Unreadable(_) => Vec::new(),
    })
}

/// Reads the strat field; anything but a non-blank string reads as none.
fn lenient_strat_field<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Field {
        Key(String),
        Unreadable(serde::de::IgnoredAny),
    }
    Ok(match Field::deserialize(deserializer)? {
        Field::Key(key) if !key.trim().is_empty() => Some(key),
        _ => None,
    })
}

/// Reads the strat columns a field at a time, so one malformed entry costs
/// only itself and not the colours saved beside it.
fn lenient_strat_columns<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<StratColumn>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        Column(StratColumn),
        Unreadable(serde::de::IgnoredAny),
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum List {
        Entries(Vec<Entry>),
        Unreadable(serde::de::IgnoredAny),
    }
    Ok(match List::deserialize(deserializer)? {
        List::Entries(entries) => entries
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Column(column) => Some(column),
                Entry::Unreadable(_) => None,
            })
            .collect(),
        List::Unreadable(_) => Vec::new(),
    })
}

/// How many saved working sections the reader passes over in a colour
/// state's JSON: each list entry that is not a section, or one for a value
/// that is not a list. Nothing saved counts nothing.
pub(crate) fn skipped_working_sections(color: &serde_json::Value) -> usize {
    match color.get("working_sections") {
        None | Some(serde_json::Value::Null) => 0,
        Some(serde_json::Value::Array(entries)) => entries.iter().filter(|entry| WorkingSection::deserialize(*entry).is_err()).count(),
        Some(_) => 1,
    }
}

/// A dataset is drawn at its drilled width until someone says otherwise.
pub(crate) fn default_radius_scale() -> f64 {
    1.0
}

fn default_min_pixel_diameter() -> f32 {
    MIN_RENDER_PIXEL_DIAMETER
}

/// What the width controls accept: a hole thinner than a twentieth of its
/// drilled width is a line, and one twice it is a shaft.
pub(crate) const RADIUS_SCALE_RANGE: std::ops::RangeInclusive<f64> = 0.05..=2.0;
/// One pixel is the thinnest a hole can be and still be drawn at all.
pub(crate) const MIN_PIXEL_DIAMETER_RANGE: std::ops::RangeInclusive<f32> = 1.0..=8.0;

// Clamped where a value comes in, not only where the dialog sets one: a file
// edited by hand must not draw a hole at nothing and then save that back.
fn clamped_radius_scale<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    let value = f64::deserialize(deserializer)?;
    Ok(if value.is_finite() {
        value.clamp(*RADIUS_SCALE_RANGE.start(), *RADIUS_SCALE_RANGE.end())
    } else {
        default_radius_scale()
    })
}

fn clamped_min_pixel_diameter<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    Ok(if value.is_finite() {
        value.clamp(*MIN_PIXEL_DIAMETER_RANGE.start(), *MIN_PIXEL_DIAMETER_RANGE.end())
    } else {
        default_min_pixel_diameter()
    })
}

/// The style of a set saved before there was one: drawn at its true
/// diameter, as it was when saved. Deliberately not what a new set gets.
fn saved_before_styles_hole_style() -> DrillHoleStyle {
    DrillHoleStyle::TrueDiameter
}

/// A style this build does not know, from a newer one, reads as the saved
/// look rather than costing the set every colour saved beside it.
fn lenient_hole_style<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<DrillHoleStyle, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Style {
        Known(DrillHoleStyle),
        Unknown(serde::de::IgnoredAny),
    }
    Ok(match Style::deserialize(deserializer)? {
        Style::Known(style) => style,
        Style::Unknown(_) => saved_before_styles_hole_style(),
    })
}

pub(crate) fn default_disc_diameter() -> f64 {
    DEFAULT_DISC_DIAMETER
}

pub(crate) fn default_string_pixel_width() -> f32 {
    DEFAULT_STRING_PIXEL_WIDTH
}

/// The disc diameter as the controls and a saved file may set it: a value
/// that is not a number is the default, anything else is held in range.
pub(crate) fn clamp_disc_diameter(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(*DISC_DIAMETER_RANGE.start(), *DISC_DIAMETER_RANGE.end())
    } else {
        default_disc_diameter()
    }
}

/// The string width as the controls and a saved file may set it.
pub(crate) fn clamp_string_pixel_width(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(*STRING_PIXEL_WIDTH_RANGE.start(), *STRING_PIXEL_WIDTH_RANGE.end())
    } else {
        default_string_pixel_width()
    }
}

fn clamped_disc_diameter<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(clamp_disc_diameter(f64::deserialize(deserializer)?))
}

fn clamped_string_pixel_width<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    Ok(clamp_string_pixel_width(f32::deserialize(deserializer)?))
}

impl Default for DrillColorState {
    fn default() -> Self {
        let preset = DrillColorPreset::Rainbow;
        Self {
            active_field: None,
            preset,
            smooth: preset.smooth(),
            stops: preset.stops(),
            categories: CategoryTable::default(),
            radius_scale: default_radius_scale(),
            min_pixel_diameter: default_min_pixel_diameter(),
            working_sections: Vec::new(),
            by_working_section: false,
            strat_columns: Vec::new(),
            strat_field: None,
            // The look a set had before there was a choice. A new set takes
            // the constructor for where it came from instead.
            hole_style: saved_before_styles_hole_style(),
            disc_diameter: default_disc_diameter(),
            string_pixel_width: default_string_pixel_width(),
        }
    }
}

impl DrillColorState {
    /// A set of logged holes, imported: drawn as string and discs, the
    /// picture a modeller reads intervals by.
    pub(crate) fn for_logged_holes() -> Self {
        Self {
            hole_style: DrillHoleStyle::StringAndDiscs,
            ..Self::default()
        }
    }

    /// A set of planned holes, laid out as a pattern: drawn at their true
    /// diameter. They carry no intervals to put discs on, and their design
    /// diameter, and the tie-ins sized from it, must read as drawn.
    pub(crate) fn for_planned_holes() -> Self {
        Self {
            hole_style: DrillHoleStyle::TrueDiameter,
            ..Self::default()
        }
    }

    /// The narrowest a disc is drawn on screen, in pixels: always wider than
    /// the string it sits on, so its colour shows either side.
    pub(crate) fn disc_min_pixel_diameter(&self) -> f32 {
        self.string_pixel_width + DISC_PIXEL_MARGIN
    }

    /// The colour kept under a code or a section's [`section_color_key`]. A
    /// section with no entry yet shows the colour under its bare name until
    /// [`Self::reconcile_categories`] gives it one.
    pub(crate) fn category_color(&self, value: &str) -> Option<[f32; 3]> {
        let find = |value: &str| {
            self.categories
                .binary_search_by(|entry| entry.value.as_str().cmp(value))
                .ok()
                .map(|index| self.categories[index].color)
        };
        find(value).or_else(|| value.strip_prefix(SECTION_COLOR_PREFIX).and_then(find))
    }

    /// The working section of `field` that holds `code`, if any.
    pub(crate) fn working_section_of(&self, field: &str, code: &str) -> Option<&WorkingSection> {
        self.working_sections
            .iter()
            .find(|section| section.field == field && section.codes.iter().any(|member| member == code))
    }

    /// `field`'s strat column, top first; empty when it has none.
    pub(crate) fn strat_column(&self, field: &str) -> &[String] {
        self.strat_columns.iter().find(|column| column.field == field).map_or(&[], |column| column.codes.as_slice())
    }

    /// Replace `field`'s strat column with `codes`, blanks, UNK and
    /// duplicates left out, keeping the field's place among the others. An empty
    /// column is kept as none.
    pub(crate) fn set_strat_column(&mut self, field: &str, codes: Vec<String>) {
        let mut kept: Vec<String> = Vec::with_capacity(codes.len());
        for code in codes {
            if !code.trim().is_empty() && code != UNKNOWN_NAME && !kept.contains(&code) {
                kept.push(code);
            }
        }
        match self.strat_columns.iter().position(|column| column.field == field) {
            Some(at) if kept.is_empty() => {
                self.strat_columns.remove(at);
            }
            Some(at) => self.strat_columns[at].codes = kept,
            None if kept.is_empty() => {}
            None => self.strat_columns.push(StratColumn {
                field: field.to_owned(),
                codes: kept,
            }),
        }
    }

    /// The working section of `field` called `name`, if any.
    pub(crate) fn working_section_named(&self, field: &str, name: &str) -> Option<&WorkingSection> {
        self.working_sections.iter().find(|section| section.field == field && section.name == name)
    }

    /// The colour key of each code's section, gathered once for a rebuild;
    /// empty unless colouring the active field by working section.
    pub(crate) fn section_lookup(&self) -> SectionLookup<'_> {
        let mut keys = Vec::new();
        let mut index = HashMap::new();
        if self.by_working_section
            && let Some(field) = self.active_field.as_deref()
        {
            for section in self.working_sections.iter().filter(|section| section.field == field) {
                keys.push(section_color_key(&section.name));
                for code in &section.codes {
                    index.entry(code.as_str()).or_insert(keys.len() - 1);
                }
            }
        }
        SectionLookup { keys, index }
    }

    /// The colour table key an interval of `code` is coloured by: its
    /// working section's key when colouring by section and the code is in
    /// one, otherwise the code. This walks the section list; a pass over
    /// many intervals takes a [`Self::section_lookup`] instead.
    pub(crate) fn color_key<'a>(&'a self, code: &'a str) -> std::borrow::Cow<'a, str> {
        if !self.by_working_section {
            return code.into();
        }
        self.active_field
            .as_deref()
            .and_then(|field| self.working_section_of(field, code))
            .map_or(code.into(), |section| section_color_key(&section.name).into())
    }

    /// What the colour table lists for `field` coloured by section: every
    /// section, then each code no section holds, in the field's order.
    pub(crate) fn working_section_view<'a>(&'a self, field: &str, categories: &'a [String]) -> Vec<ColorRow<'a>> {
        let mut view: Vec<ColorRow<'a>> = self
            .working_sections
            .iter()
            .filter(|section| section.field == field)
            .map(|section| ColorRow {
                label: &section.name,
                key: section_color_key(&section.name).into(),
                section: Some(section),
            })
            .collect();
        for code in categories {
            if self.working_section_of(field, code).is_none() && !view.iter().any(|row| row.key == code.as_str()) {
                view.push(ColorRow::code(code));
            }
        }
        view
    }

    /// Every write goes through here so the order the lookup searches holds.
    pub(crate) fn set_categories(&mut self, categories: Vec<DrillCategoryColor>) {
        self.categories = CategoryTable::new(categories);
    }

    /// Give `to` the colour `from` has in `field`, when `field` is the one
    /// coloured by and `to` has none of its own, so a renamed code keeps its
    /// look. Returns whether anything changed.
    pub(crate) fn carry_category_color(&mut self, field: &str, from: &str, to: &str) -> bool {
        if self.active_field.as_deref() != Some(field) || self.category_color(to).is_some() {
            return false;
        }
        let Some(color) = self.category_color(from) else {
            return false;
        };
        let mut table = Vec::from(std::mem::take(&mut self.categories));
        table.push(DrillCategoryColor { value: to.to_owned(), color });
        self.set_categories(table);
        true
    }

    /// [`Self::carry_category_color`] for several renames made at once: each
    /// new name without a colour takes its old name's colour as it stood, so
    /// no colour is passed along by way of another rename.
    pub(crate) fn carry_category_colors(&mut self, field: &str, renames: &[(String, String)]) -> bool {
        if self.active_field.as_deref() != Some(field) {
            return false;
        }
        let carried = renames
            .iter()
            .filter(|(_, to)| self.category_color(to).is_none())
            .filter_map(|(from, to)| self.category_color(from).map(|color| DrillCategoryColor { value: to.clone(), color }))
            .collect::<Vec<_>>();
        if carried.is_empty() {
            return false;
        }
        let mut table = Vec::from(std::mem::take(&mut self.categories));
        table.extend(carried);
        self.set_categories(table);
        true
    }

    /// Give every code in `field`, and every working section of it, a
    /// colour, keeping every colour already chosen, and return how many of
    /// the field's codes were filled in; section names are not counted.
    /// Entries for codes absent from `field` stay: a shorter extract must not
    /// lose the full one's picks.
    pub(crate) fn reconcile_categories(&mut self, field: &DrillField) -> usize {
        let DrillFieldKind::Categorical { categories } = &field.kind else {
            return 0;
        };
        let added = self.fill_category_colors(categories);
        if self.working_sections.iter().any(|section| section.field == field.key) {
            // A section saved before sections had keys keeps the colour under
            // its bare name, and one named after a code it holds starts in it.
            let adopted: Vec<DrillCategoryColor> = self
                .working_sections
                .iter()
                .filter(|section| section.field == field.key)
                .filter_map(|section| {
                    let key = section_color_key(&section.name);
                    let own = self.categories.binary_search_by(|entry| entry.value.as_str().cmp(&key)).is_ok();
                    let bare = self.category_color(&section.name);
                    bare.filter(|_| !own).map(|color| DrillCategoryColor { value: key, color })
                })
                .collect();
            if !adopted.is_empty() {
                let mut table = Vec::from(std::mem::take(&mut self.categories));
                table.extend(adopted);
                self.set_categories(table);
            }
            let keys: Vec<String> = self.working_section_view(&field.key, categories).into_iter().map(|row| row.key.into_owned()).collect();
            self.fill_category_colors(&keys);
        }
        added
    }

    /// Give each of `categories` a colour where it has none, avoiding the
    /// colours the others already hold, and return how many were filled in.
    pub(crate) fn fill_category_colors(&mut self, categories: &[String]) -> usize {
        let mut table = Vec::from(std::mem::take(&mut self.categories));
        let mut used: Vec<[f32; 3]> = Vec::new();
        for value in categories {
            if let Ok(index) = table.binary_search_by(|entry| entry.value.as_str().cmp(value)) {
                used.push(table[index].color);
            }
        }
        let mut filled = Vec::new();
        for (index, value) in categories.iter().enumerate() {
            if table.binary_search_by(|entry| entry.value.as_str().cmp(value)).is_err() {
                let mut color = None;
                for offset in 0..=used.len() {
                    let candidate = default_color(categories, index, offset);
                    if !used.contains(&candidate) {
                        color = Some(candidate);
                        break;
                    }
                }
                let color = color.unwrap_or_else(|| default_color(categories, index, 0));
                used.push(color);
                filled.push(DrillCategoryColor { value: value.clone(), color });
            }
        }
        let added = filled.len();
        table.append(&mut filled);
        self.set_categories(table);
        added
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LoadedDrillHoleDataset {
    pub(crate) name: String,
    pub(crate) source: DrillHoleSource,
    pub(crate) dataset: Arc<DrillHoleDataset>,
}

#[derive(Clone, Debug)]
pub(crate) struct OpenDrillHoleDataset {
    pub(crate) id: DrillHoleId,
    pub(crate) state: ProjectItemState,
    pub(crate) name: String,
    pub(crate) dataset: Arc<DrillHoleDataset>,
    pub(crate) color: DrillColorState,
    /// Downhole geophysics files linked to the dataset, saved with it.
    /// Relinking replaces the link rather than editing it, so a reader can
    /// tell a new link from the one it checked by pointer.
    pub(crate) geophysics: Option<Arc<crate::model::geophysics::GeophysicsLink>>,
}

impl OpenDrillHoleDataset {
    pub(crate) fn entity_id(&self) -> crate::model::SceneEntityId {
        crate::model::SceneEntityId::DrillHole(self.id)
    }
}

/// One hole inside a dataset.
///
/// A dataset is a scene entity - it is what the explorer lists, what is
/// hidden, frozen and coloured - so [`crate::model::SceneEntityId`] names the
/// dataset and stops there. A single hole is a part of one, the way a vertex
/// is part of a polyline, and this is how the Drill & Blast workspace points
/// at it: the dataset's id and the hole's index in
/// [`DrillHoleDataset::holes`], which is fixed for as long as the dataset is
/// loaded ([`DrillHoleDataset::new`] sorts the holes once, on import).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DrillHoleRef {
    pub(crate) dataset: DrillHoleId,
    pub(crate) hole: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SurveyObservation {
    pub(crate) depth: f64,
    pub(crate) azimuth: Option<f64>,
    pub(crate) dip: Option<f64>,
    pub(crate) position: Option<DVec3>,
}

/// A resolved trace and whether an observation steered it past the collar.
pub(crate) struct ResolvedTrace {
    pub(crate) stations: Vec<TraceStation>,
    pub(crate) steered: bool,
}

pub(crate) fn resolve_trace(collar: DVec3, observations: &mut [SurveyObservation], target_depth: f64) -> ResolvedTrace {
    observations.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    let mut trace = vec![TraceStation { depth: 0.0, position: collar }];
    let mut last_orientation = observations.iter().find_map(SurveyObservation::orientation);
    let mut steered = false;
    for observation in observations.iter().copied() {
        if !observation.depth.is_finite() || observation.depth < 0.0 {
            continue;
        }
        let previous = *trace.last().expect("collar station exists");
        if observation.depth + 1.0e-9 < previous.depth {
            continue;
        }
        let stored_position = observation.position.filter(|position| position.is_finite());
        steered |= observation.orientation().is_some();
        let position = stored_position.unwrap_or_else(|| {
            let (azimuth, dip) = last_orientation.unwrap_or((0.0, -90.0));
            project_tangent(previous.position, observation.depth - previous.depth, azimuth, dip)
        });
        if observation.depth > previous.depth + 1.0e-9 {
            steered |= stored_position.is_some() || last_orientation.is_some();
            trace.push(TraceStation {
                depth: observation.depth,
                position,
            });
        } else if let Some(first) = trace.first_mut() {
            first.position = collar;
        }
        last_orientation = observation
            .orientation()
            .or_else(|| stored_position.and_then(|_| orientation_from_delta(position - previous.position)))
            .or(last_orientation);
    }
    let previous = *trace.last().expect("collar station exists");
    if target_depth.is_finite() && target_depth > previous.depth + 1.0e-9 {
        let (azimuth, dip) = last_orientation.unwrap_or((0.0, -90.0));
        steered |= last_orientation.is_some();
        trace.push(TraceStation {
            depth: target_depth,
            position: project_tangent(previous.position, target_depth - previous.depth, azimuth, dip),
        });
    }
    ResolvedTrace { stations: trace, steered }
}

impl SurveyObservation {
    fn orientation(&self) -> Option<(f64, f64)> {
        match (self.azimuth, self.dip) {
            (Some(azimuth), Some(dip)) if azimuth.is_finite() && dip.is_finite() => Some((azimuth, dip)),
            _ => None,
        }
    }
}

fn orientation_from_delta(delta: DVec3) -> Option<(f64, f64)> {
    let horizontal = delta.x.hypot(delta.y);
    let length = horizontal.hypot(delta.z);
    (length > 1.0e-12).then(|| (delta.x.atan2(delta.y).to_degrees().rem_euclid(360.0), delta.z.atan2(horizontal).to_degrees()))
}

fn project_tangent(origin: DVec3, distance: f64, azimuth_degrees: f64, dip_degrees: f64) -> DVec3 {
    let azimuth = azimuth_degrees.to_radians();
    let dip = dip_degrees.to_radians();
    let horizontal = distance * dip.cos();
    origin + DVec3::new(horizontal * azimuth.sin(), horizontal * azimuth.cos(), distance * dip.sin())
}

/// Write one side of a correction to its hole: its after when `apply`, its
/// before when undone. A value goes into the record's field, a roof's depth
/// is the interval's from, a floor's its to and its follower's from. An
/// insertion puts its row in, or takes it back out; a deletion takes its row
/// out, or puts it back where it was. False, with nothing written, when the
/// target is not in the hole.
fn write_correction(hole: &mut DrillHole, target: CorrectionTarget, record: &Correction, apply: bool) -> bool {
    match record.kind {
        CorrectionKind::Insertion => return write_insertion(hole, target, record, apply),
        CorrectionKind::Deletion => return write_deletion(hole, target, record, apply),
        CorrectionKind::Value | CorrectionKind::Boundary => {}
    }
    let value = if apply { record.after.as_ref() } else { record.before.as_ref() };
    let touched = [Some(target.interval), target.below];
    if touched.iter().flatten().any(|&index| index >= hole.intervals.len()) {
        return false;
    }
    if record.kind == CorrectionKind::Boundary {
        let Some(&DrillValue::Numeric(depth)) = value else {
            return false;
        };
        for index in touched.into_iter().flatten() {
            hole.intervals[index].keep_logged();
        }
        if record.field == ROOF_FIELD {
            hole.intervals[target.interval].from = depth;
        } else {
            hole.intervals[target.interval].to = depth;
        }
        if let Some(below) = target.below {
            hole.intervals[below].from = depth;
        }
    } else {
        let interval = &mut hole.intervals[target.interval];
        interval.keep_logged();
        set_value(&mut interval.values, &record.field, value);
    }
    true
}

/// [`write_correction`] for an insertion. A record carrying its row puts it
/// in at the target, its depths and key the record's, and when undone takes
/// it back out, if it is still the one put in. A record carrying no row
/// names a value of the row before it and only checks that row is there.
fn write_insertion(hole: &mut DrillHole, target: CorrectionTarget, record: &Correction, apply: bool) -> bool {
    let ours = hole
        .intervals
        .get(target.interval)
        .and_then(|interval| interval.logged.as_ref())
        .is_some_and(|logged| logged.never_logged && logged.from == record.logged_from && logged.to == record.logged_to);
    match (&record.row, apply) {
        (None, _) => ours,
        (Some(_), false) => {
            if ours {
                hole.intervals.remove(target.interval);
            }
            ours
        }
        (Some(row), true) => {
            if target.interval > hole.intervals.len() {
                return false;
            }
            hole.intervals.insert(
                target.interval,
                DrillInterval {
                    from: record.logged_from,
                    to: record.logged_to,
                    values: row.clone(),
                    logged: Some(LoggedInterval {
                        from: record.logged_from,
                        to: record.logged_to,
                        values: BTreeMap::new(),
                        never_logged: true,
                    }),
                },
            );
            true
        }
    }
}

/// [`write_correction`] for a deletion: the row the record holds leaves the
/// hole at the target, if it is still that row, and when undone goes back in
/// at the same place, exactly as it was.
fn write_deletion(hole: &mut DrillHole, target: CorrectionTarget, record: &Correction, apply: bool) -> bool {
    let Some(removed) = &record.removed else {
        return false;
    };
    if apply {
        let ours = hole.intervals.get(target.interval).is_some_and(|row| same_interval_as_saved(row, removed));
        if ours {
            hole.intervals.remove(target.interval);
        }
        ours
    } else if hole.dhid == record.hole && target.interval <= hole.intervals.len() {
        hole.intervals.insert(target.interval, removed.clone());
        true
    } else {
        false
    }
}

/// Whether two values are the same, a NaN the same as a NaN, as a row read
/// back from a file must match the one it was.
fn same_value(a: &DrillValue, b: &DrillValue) -> bool {
    match (a, b) {
        (DrillValue::Numeric(a), DrillValue::Numeric(b)) => a == b || (a.is_nan() && b.is_nan()),
        (DrillValue::Category(a), DrillValue::Category(b)) => a == b,
        _ => false,
    }
}

fn same_option_value(a: Option<&DrillValue>, b: Option<&DrillValue>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => same_value(a, b),
        (None, None) => true,
        _ => false,
    }
}

fn same_values(a: &BTreeMap<String, DrillValue>, b: &BTreeMap<String, DrillValue>) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|((key_a, a), (key_b, b))| key_a == key_b && same_value(a, b))
}

fn same_depth(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

/// Whether two intervals are the same row, depths, values and logged side,
/// a NaN matching a NaN.
pub(crate) fn same_interval(a: &DrillInterval, b: &DrillInterval) -> bool {
    same_depth(a.from, b.from)
        && same_depth(a.to, b.to)
        && same_values(&a.values, &b.values)
        && match (&a.logged, &b.logged) {
            (Some(a), Some(b)) => same_depth(a.from, b.from) && same_depth(a.to, b.to) && same_values(&a.values, &b.values) && a.never_logged == b.never_logged,
            (None, None) => true,
            _ => false,
        }
}

/// Whether `row` is `held`, either as held or as a save reads it back: the
/// interval columns of a file hold no NaN or infinite number, so such a value
/// comes back as none. The logged side keeps it.
fn same_interval_as_saved(row: &DrillInterval, held: &DrillInterval) -> bool {
    same_interval(row, held) || {
        let mut saved = held.clone();
        saved.values.retain(|_, value| !matches!(value, DrillValue::Numeric(number) if !number.is_finite()));
        same_interval(row, &saved)
    }
}

/// Whether two records are the same record, as [`same_interval`] matches rows.
fn same_record(a: &Correction, b: &Correction) -> bool {
    a.hole == b.hole
        && same_depth(a.logged_from, b.logged_from)
        && same_depth(a.logged_to, b.logged_to)
        && a.kind == b.kind
        && a.field == b.field
        && same_option_value(a.before.as_ref(), b.before.as_ref())
        && same_option_value(a.after.as_ref(), b.after.as_ref())
        && a.author == b.author
        && a.date == b.date
        && a.reason == b.reason
        && a.status == b.status
        && a.approver == b.approver
        && a.decision == b.decision
        && a.decided == b.decided
        && match (&a.row, &b.row) {
            (Some(a), Some(b)) => same_values(a, b),
            (None, None) => true,
            _ => false,
        }
        && match (&a.removed, &b.removed) {
            (Some(a), Some(b)) => same_interval(a, b),
            (None, None) => true,
            _ => false,
        }
}

/// Whether the member rows of `rows`, each its start, its name in the field
/// and whether a member, read as `name`, sit out of `column`'s order: once
/// touching runs of one name are joined, a column name above one of their
/// runs comes after `name` in the column, or one below it comes no later.
/// Rows starting together keep their order. Names outside the column and UNK
/// are passed over, and a `name` the column lacks is never out of order.
fn out_of_sequence(rows: &[(f64, Option<&str>, bool)], name: &str, column: &[String]) -> bool {
    let rank = |name: &str| (name != UNKNOWN_NAME).then(|| column.iter().position(|code| code == name)).flatten();
    let Some(target) = rank(name) else {
        return false;
    };
    let mut order: Vec<&(f64, Option<&str>, bool)> = rows.iter().collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Runs top to bottom: place in the column, and whether a member.
    let mut runs: Vec<(usize, bool)> = Vec::new();
    for &&(_, named, is_member) in &order {
        let place = if is_member { Some(target) } else { named.and_then(rank) };
        let Some(place) = place else { continue };
        match runs.last_mut() {
            Some(run) if run.0 == place => run.1 |= is_member,
            _ => runs.push((place, is_member)),
        }
    }
    runs.iter()
        .enumerate()
        .filter(|(_, run)| run.1)
        .any(|(at, _)| runs[..at].iter().any(|run| run.0 >= target) || runs[at + 1..].iter().any(|run| run.0 <= target))
}

/// Whether `interval` has depths and a value of `field`: one the strat
/// column draws, and an insertion trims.
pub(crate) fn holds_value(interval: &DrillInterval, field: &str) -> bool {
    interval.from.is_finite()
        && interval.to.is_finite()
        && match interval.values.get(field) {
            Some(DrillValue::Category(name)) => !name.trim().is_empty(),
            Some(DrillValue::Numeric(_)) => true,
            None => false,
        }
}

/// Whether `interval` holds any data: a value neither blank nor in one of
/// the `derived` columns an Incline tool writes. A row without is as if it
/// were not there.
fn holds_data(interval: &DrillInterval, derived: &BTreeSet<String>) -> bool {
    interval.values.iter().any(|(key, value)| is_data(key, value, derived))
}

/// Whether `value`, in column `key`, is data: not blank, and not in one of
/// the `derived` columns an Incline tool writes.
fn is_data(key: &str, value: &DrillValue, derived: &BTreeSet<String>) -> bool {
    !derived.contains(key)
        && match value {
            DrillValue::Category(text) => !text.trim().is_empty(),
            DrillValue::Numeric(_) => true,
        }
}

/// The columns of `field`'s table in `hole`: those of the rows holding a
/// value of the field, bar the `derived` ones, which tell no table.
fn field_table<'a>(hole: &'a DrillHole, field: &str, derived: &BTreeSet<String>) -> BTreeSet<&'a String> {
    hole.intervals
        .iter()
        .filter(|interval| holds_value(interval, field))
        .flat_map(|interval| interval.values.keys())
        .filter(|key| !derived.contains(*key))
        .collect()
}

/// Whether `interval` has depths and shares one of `columns`: a row of the
/// table they are the columns of.
fn shares_table(interval: &DrillInterval, columns: &BTreeSet<&String>) -> bool {
    interval.from.is_finite() && interval.to.is_finite() && interval.values.keys().any(|key| columns.contains(key))
}

/// Whether `interval` is a row of the table whose columns are `columns`: it
/// has depths and either shares one of them or holds nothing but `derived`
/// columns, and so could be of any table.
fn in_table(interval: &DrillInterval, columns: &BTreeSet<&String>, derived: &BTreeSet<String>) -> bool {
    shares_table(interval, columns) || (interval.from.is_finite() && interval.to.is_finite() && interval.values.keys().all(|key| derived.contains(key)))
}

/// Roughly how much memory `values` holds: per entry its key, its header
/// and the value.
fn values_bytes(values: &BTreeMap<String, DrillValue>) -> usize {
    values
        .iter()
        .map(|(key, value)| size_of::<String>() + key.len() + size_of::<DrillValue>() + value_bytes(value))
        .fold(0usize, usize::saturating_add)
}

/// The text a value holds beyond its own size.
fn value_bytes(value: &DrillValue) -> usize {
    match value {
        DrillValue::Category(text) => text.len(),
        DrillValue::Numeric(_) => 0,
    }
}

/// A depth as a dialog shows it: rounded to the millimetre, trailing zeros
/// left off, and never "-0". The insert dialog's hint and every refusal
/// quoting a hole's end show it alike.
pub(crate) fn shown_depth(depth: f64) -> String {
    let text = format!("{depth:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".to_owned() } else { text.to_owned() }
}

/// A value as the strat column names it: a code trimmed, a number as written.
pub(crate) fn value_name(value: Option<&DrillValue>) -> String {
    match value {
        Some(DrillValue::Category(name)) => name.trim().to_owned(),
        Some(DrillValue::Numeric(number)) => format!("{number}"),
        None => String::new(),
    }
}

/// Set `key` to `value`, or leave it unrecorded when there is none.
fn set_value(values: &mut BTreeMap<String, DrillValue>, key: &str, value: Option<&DrillValue>) {
    match value {
        Some(value) => {
            values.insert(key.to_owned(), value.clone());
        }
        None => {
            values.remove(key);
        }
    }
}

fn collect_fields(holes: &[DrillHole]) -> Vec<DrillField> {
    fields_of(holes.iter().map(|hole| hole.intervals.iter().map(|interval| &interval.values)))
}

/// [`collect_fields`] over any groups of rows' values, such as those
/// deletions hold.
fn fields_of<'a, Rows: IntoIterator<Item = &'a BTreeMap<String, DrillValue>>>(holes: impl IntoIterator<Item = Rows>) -> Vec<DrillField> {
    let mut numeric: BTreeMap<String, (String, f64, f64, bool)> = BTreeMap::new();
    let mut categorical: BTreeMap<String, (String, BTreeSet<String>)> = BTreeMap::new();
    for hole in holes {
        for values in hole {
            for (key, value) in values {
                let label = key.clone();
                match value {
                    // Sentinels stay out of the range; the field still shows.
                    DrillValue::Numeric(value) if value.is_finite() => {
                        let sentinel = crate::model::block_model::is_no_data_sentinel(*value);
                        numeric
                            .entry(key.clone())
                            .and_modify(|(_, min, max, has_real)| {
                                if sentinel {
                                    return;
                                }
                                if *has_real {
                                    *min = min.min(*value);
                                    *max = max.max(*value);
                                } else {
                                    *min = *value;
                                    *max = *value;
                                    *has_real = true;
                                }
                            })
                            .or_insert_with(|| if sentinel { (label, 0.0, 0.0, false) } else { (label, *value, *value, true) });
                    }
                    DrillValue::Category(value) if !value.trim().is_empty() => {
                        let values = &mut categorical.entry(key.clone()).or_insert_with(|| (label, BTreeSet::new())).1;
                        if !values.contains(value.as_str()) {
                            values.insert(value.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    let mut fields = Vec::new();
    fields.extend(numeric.into_iter().map(|(key, (label, min, max, _))| DrillField {
        key,
        label,
        kind: DrillFieldKind::Numeric { min, max },
    }));
    fields.extend(categorical.into_iter().map(|(key, (label, categories))| {
        // Every distinct code reaches the model; the ramp limit is not theirs.
        let mut categories = categories.into_iter().collect::<Vec<_>>();
        categories.sort_by(|a, b| crate::natural_sort::natural_cmp(a, b));
        DrillField {
            key,
            label,
            kind: DrillFieldKind::Categorical { categories },
        }
    }));
    fields.sort_by(|a, b| crate::natural_sort::natural_cmp(&a.label, &b.label));
    fields
}

pub(crate) type WorldBox = (DVec3, DVec3);

/// One walk of the stations, so the dataset bounds and the per-hole boxes
/// cannot come from different station sets.
fn drill_extents(holes: &[DrillHole]) -> (Option<WorldBox>, Vec<WorldBox>) {
    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = DVec3::splat(f64::NEG_INFINITY);
    let mut any = false;
    let mut boxes = Vec::with_capacity(holes.len());
    for hole in holes {
        let hole_box = trace_box(hole.collar_position(), &hole.trace);
        boxes.push(hole_box);
        // The bounds cover the collar marker; the hole's box is geometry alone.
        let radius = hole.render_radius() * COLLAR_MARKER_RADIUS_SCALE;
        min = min.min(hole_box.0 - DVec3::splat(radius));
        max = max.max(hole_box.1 + DVec3::splat(radius));
        any = true;
    }
    (any.then_some((min, max)), boxes)
}

/// The default colour for the code at `index` in a field's code list,
/// generated so no code can fall past the end of a palette: hues advance by
/// the golden angle through nine saturation and lightness bands, so near
/// hues still differ in tone, and no band reaches white, the no-value colour.
pub(crate) fn generated_category_color(index: usize) -> [f32; 3] {
    const GOLDEN_STEP: f64 = 0.618_033_988_749_895;
    const HUE_ORIGIN: f64 = 0.58;
    const BANDS: [(f32, f32); 9] = [
        (0.60, 0.52),
        (0.90, 0.70),
        (0.45, 0.32),
        (0.75, 0.60),
        (0.98, 0.42),
        (0.55, 0.46),
        (0.80, 0.36),
        (0.70, 0.66),
        (0.50, 0.40),
    ];
    let hue = (HUE_ORIGIN + index as f64 * GOLDEN_STEP).rem_euclid(1.0) as f32;
    let (saturation, lightness) = BANDS[index % BANDS.len()];
    hsl_to_rgb(hue, saturation, lightness)
}

/// Plain HSL to sRGB, in the component convention the shader takes.
fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> [f32; 3] {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue.rem_euclid(1.0) * 6.0;
    let second = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (red, green, blue) = match sector as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let base = lightness - chroma * 0.5;
    [red + base, green + base, blue + base]
}

/// Working sections take full-strength hues, the most contrasting first, so
/// a handful of seams read apart at a glance; codes keep the generated tones.
const SECTION_COLORS: [[f32; 3]; 12] = [
    [1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 1.0, 0.0],
    [1.0, 1.0, 0.0],
    [1.0, 0.0, 1.0],
    [0.0, 1.0, 1.0],
    [1.0, 0.5, 0.0],
    [0.5, 0.0, 1.0],
    [0.5, 1.0, 0.0],
    [0.0, 0.5, 1.0],
    [1.0, 0.0, 0.5],
    [0.0, 1.0, 0.5],
];

/// The default colour for `keys[index]`, `offset` steps along its palette: a
/// section key counts only the sections before it, so the first section is
/// red wherever it sits among the codes.
fn default_color(keys: &[String], index: usize, offset: usize) -> [f32; 3] {
    let is_section = |key: &String| key.starts_with(SECTION_COLOR_PREFIX);
    if !is_section(&keys[index]) {
        return generated_category_color(index + offset);
    }
    let rank = keys[..index].iter().filter(|key| is_section(key)).count() + offset;
    SECTION_COLORS.get(rank).copied().unwrap_or_else(|| generated_category_color(rank))
}

/// A colour for every code in `categories`, sorted by code for the lookup.
pub(crate) fn default_category_colors(categories: &[String]) -> Vec<DrillCategoryColor> {
    let colors = categories
        .iter()
        .enumerate()
        .map(|(index, value)| DrillCategoryColor {
            value: value.clone(),
            color: default_color(categories, index, 0),
        })
        .collect::<Vec<_>>();
    CategoryTable::new(colors).into()
}

/// Colour table keys of working sections begin with this Unicode
/// noncharacter, reserved for internal use, so a section's colour does not
/// share an entry with a code of its name.
const SECTION_COLOR_PREFIX: char = '\u{FDD0}';

/// The colour table key a working section called `name` is coloured by.
pub(crate) fn section_color_key(name: &str) -> String {
    let mut key = String::with_capacity(name.len() + SECTION_COLOR_PREFIX.len_utf8());
    key.push(SECTION_COLOR_PREFIX);
    key.push_str(name);
    key
}

/// One row of the colour list: what it is called, the colour table key it
/// sets, and the section it stands for, if it is one.
pub(crate) struct ColorRow<'a> {
    pub(crate) label: &'a str,
    pub(crate) key: std::borrow::Cow<'a, str>,
    pub(crate) section: Option<&'a WorkingSection>,
}

impl<'a> ColorRow<'a> {
    /// A code's own row, coloured under the code.
    pub(crate) fn code(code: &'a str) -> Self {
        Self {
            label: code,
            key: code.into(),
            section: None,
        }
    }
}

/// The section colour key of each code of the active field; see
/// [`DrillColorState::section_lookup`].
pub(crate) struct SectionLookup<'a> {
    keys: Vec<String>,
    index: HashMap<&'a str, usize>,
}

impl SectionLookup<'_> {
    /// The key `code` is coloured by: its section's, or `code` where it is
    /// in none.
    pub(crate) fn color_key<'b>(&'b self, code: &'b str) -> &'b str {
        self.index.get(code).map_or(code, |&section| self.keys[section].as_str())
    }
}

/// What a reference pick is made on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceTarget {
    /// One logged code.
    Code(String),
    /// Every code of the working section of this name.
    Section(String),
}

impl ReferenceTarget {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Code(name) | Self::Section(name) => name,
        }
    }

    pub(crate) fn label(&self) -> String {
        match self {
            Self::Code(code) => code.clone(),
            Self::Section(name) => tr!("drill-hole-name-working-section", name = name.clone().to_string()),
        }
    }
}

/// Which boundary of a working section a reference surface is built from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ReferenceSide {
    #[default]
    Roof,
    Floor,
}

impl ReferenceSide {
    pub(crate) const ALL: [Self; 2] = [Self::Roof, Self::Floor];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Roof => tr!("drill-hole-roof"),
            Self::Floor => tr!("drill-hole-floor"),
        }
    }
}

/// What one hole contributes to a reference surface for one working section.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ReferencePick {
    /// Down-hole depth of the chosen boundary, or `None` where the hole never
    /// holds the value.
    pub(crate) depth: Option<f64>,
    /// How many separate runs of the value the hole holds. Two or more is a
    /// possible fault repeat: the uppermost run is used and the hole flagged.
    pub(crate) runs: usize,
}

/// The pick a hole gives for `value` in `field`, on `side`.
///
/// Intervals carrying the value form runs. An interval with no value in that
/// field sits inside a run without breaking it, since a parting is still the
/// same working section, while a different value ends it. The roof is the
/// top of a run's first interval, the floor its deepest base; with more
/// than one run the uppermost is taken and `runs` says so.
///
/// `codes` is one code, or every code of a working section: consecutive
/// intervals holding any of them form a single run.
pub(crate) fn reference_pick(hole: &DrillHole, field: &str, codes: &[String], side: ReferenceSide) -> ReferencePick {
    let (runs, _) = value_runs(hole, field, codes);
    let depth = runs.first().map(|&(top, base)| match side {
        ReferenceSide::Roof => top,
        ReferenceSide::Floor => base,
    });
    ReferencePick { depth, runs: runs.len() }
}

/// The uppermost run of a working section in one hole, whole.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SectionRun {
    /// Down-hole depth of the roof: the top of the run's first interval.
    pub(crate) top: f64,
    /// Down-hole depth of the floor: the deepest base in the run.
    pub(crate) base: f64,
    /// How many separate runs the hole holds, as for [`ReferencePick`].
    pub(crate) runs: usize,
    /// The hole was logged past the run: a different value follows it. False
    /// where the intervals ran out with the run open, so the base is only
    /// where logging stopped.
    pub(crate) closed: bool,
}

/// The uppermost run of intervals carrying any of `codes` in `field`, read by
/// the same rules as [`reference_pick`], or `None` where the hole never holds
/// them.
pub(crate) fn section_run(hole: &DrillHole, field: &str, codes: &[String]) -> Option<SectionRun> {
    let (runs, last_open) = value_runs(hole, field, codes);
    let &(top, base) = runs.first()?;
    Some(SectionRun {
        top,
        base,
        runs: runs.len(),
        closed: runs.len() > 1 || !last_open,
    })
}

/// The `(top, base)` of every run of `codes` in `field`, uppermost first, and
/// whether the last run was still open when the intervals ran out.
fn value_runs(hole: &DrillHole, field: &str, codes: &[String]) -> (Vec<(f64, f64)>, bool) {
    let mut intervals: Vec<&DrillInterval> = hole.intervals.iter().collect();
    intervals.sort_by(|a, b| a.from.total_cmp(&b.from));
    let mut runs: Vec<(f64, f64)> = Vec::new();
    let mut open: Option<(f64, f64)> = None;
    for interval in intervals {
        match interval.values.get(field) {
            Some(DrillValue::Category(code)) if codes.contains(code) => {
                // A nested or duplicated row sorts after the interval that
                // contains it, so the base is the deepest `to` in the run.
                open = Some(match open {
                    Some((top, base)) => (top, base.max(interval.to)),
                    None => (interval.from, interval.to),
                });
            }
            Some(DrillValue::Category(code)) if code.trim().is_empty() => {}
            None => {}
            Some(_) => {
                if let Some(run) = open.take() {
                    runs.push(run);
                }
            }
        }
    }
    let last_open = open.is_some();
    if let Some(run) = open {
        runs.push(run);
    }
    (runs, last_open)
}

/// One disc of a hole drawn as string and discs: a stretch of the trace and
/// the interval whose value colours it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HoleDisc {
    pub(crate) from: f64,
    pub(crate) to: f64,
    /// Index into the hole's `intervals`.
    pub(crate) interval: usize,
}

/// Whether a value earns its interval a disc. A blank code, or a number that
/// is not one or is a no-data marker, is nothing at all - the colour path
/// draws it white for the same reason - so that interval shows only string.
pub(crate) fn disc_value(value: &DrillValue) -> bool {
    match value {
        DrillValue::Category(code) => !code.trim().is_empty(),
        DrillValue::Numeric(number) => number.is_finite() && !crate::model::block_model::is_no_data_sentinel(*number),
    }
}

/// The discs `hole` shows for the colour field `field`, shallowest first,
/// clamped to the trace and to `render_ranges`.
///
/// Where intervals holding a value overlap - a seam logged whole and again
/// as its plies - the shortest one covering a depth is drawn there, ties to
/// the first logged: the plies show, and the whole seam fills only what
/// they leave. Discs therefore never overlap one another, so no two of them
/// fight for the same pixels.
pub(crate) fn hole_discs(hole: &DrillHole, field: &str) -> Vec<HoleDisc> {
    let (Some(first), Some(last)) = (hole.trace.first(), hole.trace.last()) else {
        return Vec::new();
    };
    let (min_depth, max_depth) = (first.depth, last.depth);
    // Also refuses a depth that is not a number.
    if min_depth.partial_cmp(&max_depth) != Some(std::cmp::Ordering::Less) {
        return Vec::new();
    }
    let mut candidates: Vec<usize> = hole
        .intervals
        .iter()
        .enumerate()
        .filter(|(_, interval)| interval.from.is_finite() && interval.to.is_finite() && interval.from < interval.to)
        .filter(|(_, interval)| interval.values.get(field).is_some_and(disc_value))
        .map(|(index, _)| index)
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    let mut boundaries: Vec<f64> = candidates
        .iter()
        .flat_map(|&index| [hole.intervals[index].from, hole.intervals[index].to])
        .chain(hole.render_ranges.iter().flat_map(|&(from, to)| [from, to]))
        .filter(|depth| depth.is_finite())
        .map(|depth| depth.clamp(min_depth, max_depth))
        .collect();
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|a, b| (*a - *b).abs() <= 1.0e-9);

    candidates.sort_by(|&a, &b| hole.intervals[a].from.total_cmp(&hole.intervals[b].from).then(a.cmp(&b)));
    let length = |index: usize| hole.intervals[index].to - hole.intervals[index].from;
    let mut next = 0usize;
    let mut active: Vec<usize> = Vec::new();
    let mut discs: Vec<HoleDisc> = Vec::new();
    for pair in boundaries.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if to <= from + 1.0e-9 {
            continue;
        }
        let midpoint = (from + to) * 0.5;
        while next < candidates.len() && hole.intervals[candidates[next]].from <= midpoint {
            active.push(candidates[next]);
            next += 1;
        }
        active.retain(|&index| midpoint < hole.intervals[index].to);
        if !hole.render_ranges.is_empty() && !hole.render_ranges.iter().any(|(start, end)| *start <= midpoint && midpoint < *end) {
            continue;
        }
        let Some(winner) = active.iter().copied().min_by(|&a, &b| length(a).total_cmp(&length(b)).then(a.cmp(&b))) else {
            continue;
        };
        match discs.last_mut() {
            Some(disc) if disc.interval == winner && (disc.to - from).abs() <= 1.0e-9 => disc.to = to,
            _ => discs.push(HoleDisc { from, to, interval: winner }),
        }
    }
    discs
}
