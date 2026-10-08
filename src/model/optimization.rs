//! Pit optimization scenarios: everything a user types before a run.
//!
//! A scenario is the input half of an optimization - which block model, which
//! attributes carry density, quality and rock type, what mining and processing
//! cost, what the products sell for, and what to produce. Running it (turning
//! each block into one value for the optimizer) and reading the result back
//! are later stages and live elsewhere.
//!
//! Scenarios are not project content: they are kept together in one JSON file
//! beside the application config, see [`crate::app::io::save_optimization_scenarios`].
//! A scenario names its block model and fields rather than holding ids,
//! because ids are handed out afresh each session.
//!
//! Any number a user types may instead be a *constant*: a named value from the
//! scenario's own constants grid. [`FieldValue`] carries both, and keeps the
//! typed value while a constant is chosen so removing the constant gives it
//! back.

use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use crate::{i18n::tr, model::formats::block_model_data::BlockModelData};

/// Bumped whenever the file's shape changes in a way old builds cannot read.
pub(crate) const SCENARIO_FILE_VERSION: u32 = 3;

// ── Constants ──

/// The type of a constant, as the constants grid's Type column offers it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ValueType {
    #[default]
    Number,
    Text,
    YesNo,
}

impl ValueType {
    pub(crate) const ALL: [Self; 3] = [Self::Number, Self::Text, Self::YesNo];

    pub(crate) fn label(self) -> String {
        match self {
            Self::Number => tr!("opt-type-number"),
            Self::Text => tr!("opt-type-text"),
            Self::YesNo => tr!("opt-type-yes-no"),
        }
    }
}

/// A constant's value. Numbers are `f32`, text is a string and Yes/No a bool.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum ConstantValue {
    Number(f32),
    Text(String),
    YesNo(bool),
}

impl ConstantValue {
    pub(crate) fn value_type(&self) -> ValueType {
        match self {
            Self::Number(_) => ValueType::Number,
            Self::Text(_) => ValueType::Text,
            Self::YesNo(_) => ValueType::YesNo,
        }
    }

    /// The empty value of a type, which a constant takes on changing type.
    pub(crate) fn empty(value_type: ValueType) -> Self {
        match value_type {
            ValueType::Number => Self::Number(0.0),
            ValueType::Text => Self::Text(String::new()),
            ValueType::YesNo => Self::YesNo(false),
        }
    }

    pub(crate) fn display(&self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Text(value) => value.clone(),
            Self::YesNo(true) => tr!("opt-yes"),
            Self::YesNo(false) => tr!("opt-no"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Constant {
    pub(crate) name: String,
    pub(crate) value: ConstantValue,
    pub(crate) description: String,
}

impl Constant {
    pub(crate) fn blank() -> Self {
        Self {
            name: String::new(),
            value: ConstantValue::Number(0.0),
            description: String::new(),
        }
    }
}

/// One row of the constants grid. A constant belongs to the nearest group row
/// above it; those above every group row are ungrouped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum ConstantsRow {
    Group { name: String, collapsed: bool },
    Constant(Constant),
}

impl ConstantsRow {
    pub(crate) fn constant(&self) -> Option<&Constant> {
        match self {
            Self::Constant(constant) => Some(constant),
            Self::Group { .. } => None,
        }
    }
}

/// `typed`, or `typed_1`, `typed_2`... if a name in `existing` already is it
/// (compared without regard to case).
pub(crate) fn unique_name<'a>(existing: impl IntoIterator<Item = &'a str>, typed: &str) -> String {
    let taken: HashSet<String> = existing.into_iter().map(str::to_lowercase).collect();
    if !taken.contains(&typed.to_lowercase()) {
        return typed.to_owned();
    }
    (1..)
        .map(|number| format!("{typed}_{number}"))
        .find(|candidate| !taken.contains(&candidate.to_lowercase()))
        .expect("an unbounded range always yields a free name")
}

pub(crate) fn group_names(rows: &[ConstantsRow]) -> impl Iterator<Item = &str> {
    rows.iter().filter_map(|row| match row {
        ConstantsRow::Group { name, .. } => Some(name.as_str()),
        ConstantsRow::Constant(_) => None,
    })
}

/// Move the row at `index` one place up or down the constants grid.
///
/// A constant swaps with its neighbour, so stepping across a group row moves
/// it into the next group. A group row takes its constants with it and swaps
/// places with the neighbouring group (or, at the edge of its block, the
/// ungrouped constants above the first group are left where they are).
pub(crate) fn move_constants_row(rows: &mut [ConstantsRow], index: usize, up: bool) -> Option<usize> {
    let is_group = |row: &ConstantsRow| matches!(row, ConstantsRow::Group { .. });
    if !is_group(rows.get(index)?) {
        let target = if up { index.checked_sub(1)? } else { (index + 1 < rows.len()).then_some(index + 1)? };
        rows.swap(index, target);
        return Some(target);
    }
    if up {
        let previous = rows[..index].iter().rposition(is_group)?;
        let end = block_end(rows, index);
        rows[previous..end].rotate_left(index - previous);
        Some(previous)
    } else {
        let end = block_end(rows, index);
        if end >= rows.len() {
            return None;
        }
        let next_end = block_end(rows, end);
        rows[index..next_end].rotate_left(end - index);
        Some(index + (next_end - end))
    }
}

/// Where the block of the group row at `start` ends: at the next group row.
fn block_end(rows: &[ConstantsRow], start: usize) -> usize {
    rows[start + 1..]
        .iter()
        .position(|row| matches!(row, ConstantsRow::Group { .. }))
        .map_or(rows.len(), |offset| start + 1 + offset)
}

// ── Input/constant field ──

/// What a value field can hold. One impl per type a constant can have.
pub(crate) trait ValueKind: Clone + PartialEq + Default {
    const TYPE: ValueType;

    /// The value of a constant of this type, or `None` for one of another type.
    fn from_constant(value: &ConstantValue) -> Option<Self>;
}

impl ValueKind for f32 {
    const TYPE: ValueType = ValueType::Number;

    fn from_constant(value: &ConstantValue) -> Option<Self> {
        match value {
            ConstantValue::Number(number) => Some(*number),
            _ => None,
        }
    }
}

impl ValueKind for f64 {
    const TYPE: ValueType = ValueType::Number;

    fn from_constant(value: &ConstantValue) -> Option<Self> {
        match value {
            ConstantValue::Number(number) => Some(f64::from(*number)),
            _ => None,
        }
    }
}

impl ValueKind for String {
    const TYPE: ValueType = ValueType::Text;

    fn from_constant(value: &ConstantValue) -> Option<Self> {
        match value {
            ConstantValue::Text(text) => Some(text.clone()),
            _ => None,
        }
    }
}

impl ValueKind for bool {
    const TYPE: ValueType = ValueType::YesNo;

    fn from_constant(value: &ConstantValue) -> Option<Self> {
        match value {
            ConstantValue::YesNo(flag) => Some(*flag),
            _ => None,
        }
    }
}

/// A value the user types, or the name of a constant that stands in for it.
///
/// The typed value is kept while a constant is chosen, so removing the
/// constant hands back what was there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct FieldValue<T> {
    pub(crate) value: T,
    pub(crate) constant: Option<String>,
}

impl<T: Default> Default for FieldValue<T> {
    fn default() -> Self {
        Self {
            value: T::default(),
            constant: None,
        }
    }
}

impl<T: ValueKind> FieldValue<T> {
    pub(crate) fn new(value: T) -> Self {
        Self { value, constant: None }
    }

    /// The value in force: the typed one, or the chosen constant's. `None` when
    /// the constant is gone or is of another type.
    pub(crate) fn resolve(&self, constants: &[ConstantsRow]) -> Option<T> {
        match &self.constant {
            None => Some(self.value.clone()),
            Some(name) => constants
                .iter()
                .filter_map(ConstantsRow::constant)
                .find(|constant| constant.name == *name)
                .and_then(|constant| T::from_constant(&constant.value)),
        }
    }
}

// ── Scenario ──

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum AirMode {
    #[default]
    Topography,
    Rocktype,
}

/// Where a haulage cost comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum HaulageMode {
    #[default]
    Value,
    Field,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct HaulageCost {
    pub(crate) mode: HaulageMode,
    pub(crate) value: FieldValue<f64>,
    /// Numeric block model field carrying the cost, for [`HaulageMode::Field`].
    pub(crate) field: String,
}

impl Default for HaulageCost {
    fn default() -> Self {
        Self {
            mode: HaulageMode::Value,
            value: FieldValue::new(0.0),
            field: String::new(),
        }
    }
}

/// A mining cost for one rock type, in place of the default.
///
/// Scenarios saved when this was a `factor` load with a cost of 0: a factor
/// is not a cost.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct RocktypeCost {
    pub(crate) rocktype: String,
    pub(crate) cost: FieldValue<f32>,
}

impl Default for RocktypeCost {
    fn default() -> Self {
        Self {
            rocktype: String::new(),
            cost: FieldValue::new(0.0),
        }
    }
}

/// What one element costs to recover in a processing method.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ElementCost {
    pub(crate) element: String,
    /// Recovery, in percent.
    pub(crate) recovery: FieldValue<f64>,
    pub(crate) cost: FieldValue<f64>,
}

impl Default for ElementCost {
    fn default() -> Self {
        Self {
            element: String::new(),
            recovery: FieldValue::new(100.0),
            cost: FieldValue::new(0.0),
        }
    }
}

/// A way of processing ore, with the rock it takes, the grades it takes it
/// between and what it costs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ProcessingMethod {
    pub(crate) name: String,
    pub(crate) rocktype: String,
    pub(crate) min_grade: FieldValue<f64>,
    pub(crate) max_grade: FieldValue<f64>,
    pub(crate) threshold: FieldValue<f64>,
    pub(crate) elements: Vec<ElementCost>,
    /// Scales this method's haulage cost; 1 leaves it as it is.
    pub(crate) haulage_factor: FieldValue<f64>,
    pub(crate) ga_cost: FieldValue<f64>,
}

impl Default for ProcessingMethod {
    fn default() -> Self {
        Self {
            name: String::new(),
            rocktype: String::new(),
            min_grade: FieldValue::new(0.0),
            max_grade: FieldValue::new(0.0),
            threshold: FieldValue::new(0.0),
            elements: Vec::new(),
            haulage_factor: FieldValue::new(1.0),
            ga_cost: FieldValue::new(0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct RevenueRow {
    pub(crate) element: String,
    pub(crate) price: FieldValue<f64>,
    pub(crate) selling_cost: FieldValue<f64>,
}

impl Default for RevenueRow {
    fn default() -> Self {
        Self {
            element: String::new(),
            price: FieldValue::new(0.0),
            selling_cost: FieldValue::new(0.0),
        }
    }
}

/// How the overall pit wall angle is given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SlopeMode {
    /// One angle all round.
    #[default]
    Single,
    /// An angle per sector of bearings.
    Rosette,
}

/// One rosette row: from this bearing (degrees clockwise from north) the pit
/// wall stands at this angle (degrees from horizontal), up to the next row's
/// bearing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct RosetteRow {
    /// A plain number, not a constant.
    pub(crate) bearing: f64,
    pub(crate) angle: FieldValue<f64>,
}

impl Default for RosetteRow {
    fn default() -> Self {
        Self {
            bearing: 0.0,
            angle: FieldValue::new(DEFAULT_SLOPE_ANGLE),
        }
    }
}

pub(crate) const DEFAULT_SLOPE_ANGLE: f64 = 45.0;

/// The overall slope: one default angle, or a rosette. The rows are kept while
/// the single angle is in use, and the other way round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct SlopeSettings {
    pub(crate) mode: SlopeMode,
    pub(crate) angle: FieldValue<f64>,
    pub(crate) rosette: Vec<RosetteRow>,
}

impl Default for SlopeSettings {
    fn default() -> Self {
        Self {
            mode: SlopeMode::Single,
            angle: FieldValue::new(DEFAULT_SLOPE_ANGLE),
            rosette: Vec::new(),
        }
    }
}

/// A run of bearings with one wall angle: `from` up to `to` (which may pass 360).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SlopeSector {
    pub(crate) from: f64,
    pub(crate) to: f64,
    pub(crate) angle: f64,
}

/// Why a rosette cannot be turned into sectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RosetteIssue {
    NoRows,
    /// Two rows start at the same bearing.
    SameBearing,
}

impl SlopeSettings {
    /// The rosette as sectors, in bearing order. Each row's angle runs from its
    /// bearing to the next row's, and the last runs round to the first, so one
    /// row is the full circle. Rows whose values cannot be resolved are left out.
    pub(crate) fn sectors(&self, constants: &[ConstantsRow]) -> Result<Vec<SlopeSector>, RosetteIssue> {
        let mut rows: Vec<(f64, f64)> = self
            .rosette
            .iter()
            .filter_map(|row| Some((row.bearing.rem_euclid(360.0), row.angle.resolve(constants)?)))
            .collect();
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        if rows.is_empty() {
            return Err(RosetteIssue::NoRows);
        }
        if rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(RosetteIssue::SameBearing);
        }
        Ok((0..rows.len())
            .map(|index| SlopeSector {
                from: rows[index].0,
                to: rows.get(index + 1).map_or(rows[0].0 + 360.0, |next| next.0),
                angle: rows[index].1,
            })
            .collect())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ShellMode {
    #[default]
    Single,
    /// Several shells at revenue adjustment factors between two ends.
    Multiple,
}

/// The compass direction a directional shell advances in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ShellDirection {
    #[default]
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl ShellDirection {
    pub(crate) const ALL: [Self; 8] = [
        Self::North,
        Self::NorthEast,
        Self::East,
        Self::SouthEast,
        Self::South,
        Self::SouthWest,
        Self::West,
        Self::NorthWest,
    ];

    /// Bearing in degrees clockwise from north. Read by the run (stage 2).
    #[allow(dead_code)]
    pub(crate) fn bearing(self) -> f64 {
        Self::ALL.iter().position(|d| *d == self).unwrap_or(0) as f64 * 45.0
    }

    pub(crate) fn label(self) -> String {
        match self {
            Self::North => tr!("opt-direction-n"),
            Self::NorthEast => tr!("opt-direction-ne"),
            Self::East => tr!("opt-direction-e"),
            Self::SouthEast => tr!("opt-direction-se"),
            Self::South => tr!("opt-direction-s"),
            Self::SouthWest => tr!("opt-direction-sw"),
            Self::West => tr!("opt-direction-w"),
            Self::NorthWest => tr!("opt-direction-nw"),
        }
    }
}

/// Where the shell number is written in the block model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ShellFieldMode {
    /// A field the block model already has.
    #[default]
    Existing,
    /// A new field, named by the user.
    New,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct OutputSettings {
    pub(crate) mode: ShellMode,
    /// The revenue factor range of a multiple-shell run: plain numbers, not
    /// constants.
    pub(crate) factor_from: f64,
    pub(crate) factor_to: f64,
    pub(crate) factor_step: f64,
    pub(crate) shell_count: u32,
    /// Shells that advance in one direction from a starting point picked in the model.
    pub(crate) use_directional_shells: bool,
    pub(crate) shell_direction: ShellDirection,
    /// Easting and northing of the picked starting point.
    pub(crate) shell_start: Option<(f64, f64)>,
    pub(crate) create_reports: bool,
    /// Folder the reports are written to; empty until one is chosen.
    pub(crate) reports_folder: String,
    /// Shells are made as a solid, a surface, both, or - with neither - not at all.
    pub(crate) shell_as_solid: bool,
    pub(crate) shell_as_surface: bool,
    /// Name of the layer the shells are put in.
    pub(crate) shell_layer_name: String,
    pub(crate) write_shell_field: bool,
    pub(crate) shell_field_mode: ShellFieldMode,
    pub(crate) shell_field: String,
    pub(crate) shell_new_field: String,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            mode: ShellMode::Single,
            factor_from: 0.5,
            factor_to: 1.0,
            factor_step: 0.05,
            shell_count: 11,
            use_directional_shells: false,
            shell_direction: ShellDirection::North,
            shell_start: None,
            create_reports: true,
            reports_folder: String::new(),
            shell_as_solid: true,
            shell_as_surface: false,
            shell_layer_name: tr!("opt-default-shell-layer"),
            write_shell_field: false,
            shell_field_mode: ShellFieldMode::Existing,
            shell_field: String::new(),
            shell_new_field: String::new(),
        }
    }
}

/// How many shells a step makes between two ends, ends included.
pub(crate) fn shell_count_for_step(from: f64, to: f64, step: f64) -> Option<u32> {
    (step > 0.0 && to > from).then(|| ((to - from) / step).round() as u32 + 1)
}

/// The step that spreads `count` shells between two ends, ends included.
pub(crate) fn shell_step_for_count(from: f64, to: f64, count: u32) -> Option<f64> {
    (count >= 2 && to > from).then(|| (to - from) / f64::from(count - 1))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct OptimizationScenario {
    /// Identity within the file, so the list can name the one it acts on
    /// whatever the order or names do.
    pub(crate) id: u64,
    pub(crate) name: String,

    // Inputs
    /// Name of the block model the scenario runs on.
    pub(crate) block_model: String,
    pub(crate) density_field: String,
    pub(crate) quality_field: String,
    pub(crate) rocktype_field: String,
    pub(crate) exclude_air: bool,
    pub(crate) air_mode: AirMode,
    /// Name of the topography surface or point cloud, for [`AirMode::Topography`].
    pub(crate) air_topography: String,
    /// The rock type value that means air, for [`AirMode::Rocktype`].
    pub(crate) air_rocktype: String,

    pub(crate) constants: Vec<ConstantsRow>,

    // Mining costs
    pub(crate) mining_cost: FieldValue<f64>,
    /// The text field the mining cost varies by; empty means one cost for all.
    pub(crate) cost_field: String,
    /// Read from files saved before `cost_field`, where it meant "vary by the
    /// rock type field"; `migrate` turns it into `cost_field`. Never written.
    #[serde(skip_serializing)]
    pub(crate) use_rocktype_costs: bool,
    /// One row per value of `cost_field` (the row's `rocktype` is that value).
    pub(crate) rocktype_costs: Vec<RocktypeCost>,
    /// Rehabilitation cost per tonne, applied only to blocks the run finds
    /// economically waste.
    pub(crate) rehab_cost: HaulageCost,
    pub(crate) waste_haulage: HaulageCost,
    pub(crate) ore_haulage: HaulageCost,

    pub(crate) methods: Vec<ProcessingMethod>,
    /// One G&A cost for every method: editing it in one changes all.
    pub(crate) ga_same_for_all: bool,
    pub(crate) revenues: Vec<RevenueRow>,
    pub(crate) slope: SlopeSettings,
    pub(crate) output: OutputSettings,
}

impl Default for OptimizationScenario {
    fn default() -> Self {
        Self::new(0, String::new())
    }
}

impl OptimizationScenario {
    pub(crate) fn new(id: u64, name: String) -> Self {
        Self {
            id,
            name,
            block_model: String::new(),
            density_field: String::new(),
            quality_field: String::new(),
            rocktype_field: String::new(),
            exclude_air: false,
            air_mode: AirMode::Topography,
            air_topography: String::new(),
            air_rocktype: String::new(),
            constants: Vec::new(),
            mining_cost: FieldValue::new(0.0),
            // One row, so switching the option on already asks for something.
            cost_field: String::new(),
            use_rocktype_costs: false,
            rocktype_costs: vec![RocktypeCost::default()],
            rehab_cost: HaulageCost::default(),
            waste_haulage: HaulageCost::default(),
            ore_haulage: HaulageCost::default(),
            // One method, so a user with a single way of processing has
            // nothing to add.
            methods: vec![ProcessingMethod {
                name: tr!("opt-default-method-name"),
                elements: vec![ElementCost::default()],
                ..ProcessingMethod::default()
            }],
            ga_same_for_all: true,
            revenues: vec![RevenueRow::default()],
            slope: SlopeSettings::default(),
            output: OutputSettings::default(),
        }
    }

    /// A key that changes when anything a run depends on changes: everything
    /// but the name.
    pub(crate) fn fingerprint(&self) -> u64 {
        let mut settings = self.clone();
        settings.name.clear();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_string(&settings).unwrap_or_default().hash(&mut hasher);
        hasher.finish()
    }

    /// Choose the main quality field, carrying along every row that still
    /// stood on the previous one (the default element rows).
    pub(crate) fn set_quality_field(&mut self, field: String) {
        let previous = std::mem::replace(&mut self.quality_field, field.clone());
        let follows = |element: &str| element.is_empty() || element == previous;
        for method in &mut self.methods {
            for element in method.elements.iter_mut().filter(|element| follows(&element.element)) {
                element.element = field.clone();
            }
        }
        for revenue in self.revenues.iter_mut().filter(|revenue| follows(&revenue.element)) {
            revenue.element = field.clone();
        }
    }

    /// Settle the field choices against a block model that has just been
    /// picked: preselect density and rock type by name, and drop any choice the
    /// model does not have.
    pub(crate) fn apply_block_model(&mut self, fields: &BlockModelFields) {
        let keep = |value: &mut String, names: &[String]| {
            if !value.is_empty() && !names.contains(value) {
                value.clear();
            }
        };
        keep(&mut self.density_field, &fields.numeric);
        keep(&mut self.rocktype_field, &fields.text);
        if !self.cost_field.is_empty() && !fields.text.contains(&self.cost_field) {
            self.set_cost_field(String::new(), &[]);
        } else if !self.cost_field.is_empty() {
            let field = self.cost_field.clone();
            self.set_cost_field(field.clone(), &fields.rocktype_values(&field));
        }
        if self.density_field.is_empty() {
            self.density_field = first_named(&fields.numeric, &["density", "sg"]).unwrap_or_default();
        }
        if self.rocktype_field.is_empty() {
            self.rocktype_field = first_named(&fields.text, &["rock", "rocktype", "material"]).unwrap_or_default();
        }
        let quality = self.quality_field.clone();
        if !quality.is_empty() && !fields.numeric.contains(&quality) {
            self.set_quality_field(String::new());
        }
        keep(&mut self.rehab_cost.field, &fields.numeric);
        keep(&mut self.waste_haulage.field, &fields.numeric);
        keep(&mut self.ore_haulage.field, &fields.numeric);
        keep(&mut self.output.shell_field, &fields.all);
        for method in &mut self.methods {
            for element in &mut method.elements {
                keep(&mut element.element, &fields.numeric);
            }
        }
        for revenue in &mut self.revenues {
            keep(&mut revenue.element, &fields.numeric);
        }
        self.settle_rocktype_values(&fields.rocktype_values(&self.rocktype_field));
    }

    /// Drop the rock type choices `values` no longer offers, and preselect the
    /// air value (a value named "air" or "-") when none has been chosen.
    /// Bring a scenario saved by an earlier version up to date: mining costs
    /// that varied by rock type now name the rock type field.
    pub(crate) fn migrate(&mut self) {
        if std::mem::take(&mut self.use_rocktype_costs) && self.cost_field.is_empty() {
            self.cost_field = self.rocktype_field.clone();
        }
    }

    /// Choose the field the mining cost varies by (empty for none). Rows whose
    /// value the new field does not have are blanked, and a field with no rows
    /// starts with one.
    pub(crate) fn set_cost_field(&mut self, field: String, values: &[String]) {
        self.cost_field = field;
        if self.cost_field.is_empty() {
            return;
        }
        for cost in &mut self.rocktype_costs {
            if !cost.rocktype.is_empty() && !values.contains(&cost.rocktype) {
                cost.rocktype.clear();
            }
        }
        if self.rocktype_costs.is_empty() {
            self.rocktype_costs.push(RocktypeCost::default());
        }
    }

    pub(crate) fn settle_rocktype_values(&mut self, values: &[String]) {
        let keep = |value: &mut String| {
            if !value.is_empty() && !values.contains(value) {
                value.clear();
            }
        };
        keep(&mut self.air_rocktype);
        for method in &mut self.methods {
            keep(&mut method.rocktype);
        }
        if self.air_rocktype.is_empty() {
            self.air_rocktype = first_named(values, &["air", "-"]).unwrap_or_default();
        }
    }

    /// Every block model field the settings use, which the Outputs section's
    /// shell field must not overwrite.
    pub(crate) fn used_fields(&self) -> HashSet<&str> {
        let mut used: HashSet<&str> = [
            self.density_field.as_str(),
            self.quality_field.as_str(),
            self.rocktype_field.as_str(),
            self.cost_field.as_str(),
        ]
        .into();
        for haulage in [&self.rehab_cost, &self.waste_haulage, &self.ore_haulage] {
            if haulage.mode == HaulageMode::Field {
                used.insert(haulage.field.as_str());
            }
        }
        used.extend(self.methods.iter().flat_map(|method| method.elements.iter().map(|element| element.element.as_str())));
        used.extend(self.revenues.iter().map(|revenue| revenue.element.as_str()));
        used.remove("");
        used
    }

    /// Every element a processing method recovers, once each.
    pub(crate) fn processed_elements(&self) -> Vec<String> {
        let mut elements: Vec<String> = Vec::new();
        for element in self.methods.iter().flat_map(|method| &method.elements).map(|element| &element.element) {
            if !element.is_empty() && !elements.contains(element) {
                elements.push(element.clone());
            }
        }
        elements
    }
}

/// The first of `names` that matches one of `wanted` regardless of case, spaces, underscores and hyphens.
pub(crate) fn first_named(names: &[String], wanted: &[&str]) -> Option<String> {
    let plain = |text: &str| {
        text.chars()
            .filter(|character| !matches!(character, ' ' | '_' | '-'))
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    names.iter().find(|name| wanted.iter().any(|wanted| plain(name) == plain(wanted))).cloned()
}

/// The fields of a block model, as the scenario's pickers offer them.
#[derive(Clone, Debug, Default)]
pub(crate) struct BlockModelFields {
    pub(crate) numeric: Vec<String>,
    /// Categorical fields: those whose values are names.
    pub(crate) text: Vec<String>,
    pub(crate) all: Vec<String>,
    /// Each text field's values, in the order its categories were defined.
    text_values: Vec<(String, Vec<String>)>,
}

impl BlockModelFields {
    pub(crate) fn of(model: &BlockModelData) -> Self {
        let mut fields = Self::default();
        for variable in model.metadata.variables.iter().filter(|variable| !variable.special) {
            fields.all.push(variable.name.clone());
            let is_text = !variable.strings.is_empty() || matches!(variable.physical_type.as_str(), "namedbyte" | "namedshort");
            if is_text {
                fields.text.push(variable.name.clone());
                let mut values: Vec<String> = Vec::new();
                for value in variable.strings.values() {
                    if !values.contains(value) {
                        values.push(value.clone());
                    }
                }
                fields.text_values.push((variable.name.clone(), values));
            }
        }
        fields.numeric = model
            .numeric_variables()
            .into_iter()
            .filter(|variable| !variable.special)
            .map(|variable| variable.name.clone())
            .collect();
        fields
    }

    /// The distinct values of a text field. The categories a block model
    /// defines are what its blocks can hold, so this needs no scan of them.
    pub(crate) fn rocktype_values(&self, field: &str) -> Vec<String> {
        self.text_values
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, values)| values.clone())
            .unwrap_or_default()
    }
}

/// The scenarios file: versioned so a newer build's file is refused rather than
/// misread.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ScenarioFile {
    pub(crate) version: u32,
    pub(crate) scenarios: Vec<OptimizationScenario>,
}

impl ScenarioFile {
    pub(crate) fn new(scenarios: Vec<OptimizationScenario>) -> Self {
        Self {
            version: SCENARIO_FILE_VERSION,
            scenarios,
        }
    }
}
