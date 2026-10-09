//! Turning a saved scenario into the numbers a run works with.
//!
//! Everything here is cheap - no block is visited - so it runs on the UI
//! thread before the job starts, and a scenario that cannot run says why at
//! once. Every constant is resolved, every field looked up, and every rock type
//! name turned into a table indexed by category code, so the per-block pass
//! does array lookups only.

use std::{collections::BTreeMap, sync::Arc};

use glam::DVec2;

use super::grid::Topography;
use crate::{
    i18n::tr,
    model::{
        formats::block_model_data::{BlockModelData, BlockVariable},
        optimization::{
            AirMode, FactorInput, FieldValue, HaulageCost, HaulageMode, OptimizationScenario, RosetteInterpolation, RosetteIssue, ShellMode, SlopeMode, ValueKind, metal_factor,
            parse_factor_list, round_factor, shell_count_for_step,
        },
    },
};

/// The largest number of shells a run makes; shell numbers are kept as `u16`.
pub(crate) const MAX_SHELLS: usize = 1000;

/// A cost per tonne: one value for every block, or a block model field.
#[derive(Clone, Debug)]
pub(crate) enum PerTonne {
    Value(f64),
    /// Blank cells cost nothing.
    Field(Arc<Vec<f64>>),
}

impl PerTonne {
    pub(crate) fn at(&self, block: usize) -> f64 {
        match self {
            Self::Value(value) => *value,
            Self::Field(values) => finite_or_zero(values[block]),
        }
    }
}

pub(crate) fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

/// One element a method recovers.
#[derive(Clone, Debug)]
pub(crate) struct Element {
    /// Index into [`Economics::elements`].
    pub(crate) index: usize,
    pub(crate) values: Arc<Vec<f64>>,
    /// Subtracted from the grade before recovery: the method's threshold for
    /// the main quality field, 0 for the others.
    pub(crate) threshold: f64,
    /// Fraction, 0..=1.
    pub(crate) recovery: f64,
    /// Sales units per tonne of rock per grade unit (see [`metal_factor`]).
    pub(crate) factor: f64,
    /// Price less selling cost, per sales unit; never negative.
    pub(crate) net_price: f64,
    /// Element cost per sales unit contained in the feed.
    pub(crate) cost: f64,
}

/// One processing method (one row of the methods grid).
#[derive(Clone, Debug)]
pub(crate) struct Method {
    pub(crate) name: String,
    /// Whether it takes each rock type code; `None` takes every block.
    pub(crate) accepts: Option<Vec<bool>>,
    /// Main quality grade range, `min <= grade < max`.
    pub(crate) min: f64,
    pub(crate) max: f64,
    /// Processing cost plus G&A, per tonne of feed.
    pub(crate) per_tonne: f64,
    /// The two parts of `per_tonne`, for reports.
    pub(crate) processing: f64,
    pub(crate) ga: f64,
    pub(crate) ore_haulage_factor: f64,
    pub(crate) elements: Vec<Element>,
}

impl Method {
    pub(crate) fn takes(&self, rock_code: Option<f64>, grade: f64) -> bool {
        let rock_ok = match &self.accepts {
            None => true,
            Some(accepts) => rock_code.and_then(|code| code_index(code, accepts.len())).is_some_and(|code| accepts[code]),
        };
        rock_ok && grade >= self.min && grade < self.max
    }
}

/// Which blocks are air: value 0, never valued.
#[derive(Clone)]
pub(crate) enum Air {
    None,
    /// By rock type code.
    Rock(Vec<bool>),
    /// Centroids above this surface.
    Topography(Topography),
}

/// The scenario's economics, resolved.
#[derive(Clone)]
pub(crate) struct Economics {
    /// The density field, when one is chosen.
    pub(crate) density: Option<Arc<Vec<f64>>>,
    /// Used for every block without a field, and for blank, zero or negative
    /// densities in it. Always above 0.
    pub(crate) default_density: f64,
    pub(crate) quality: Arc<Vec<f64>>,
    /// Rock type codes, when a rock type field is set.
    pub(crate) rock: Option<Arc<Vec<f64>>>,
    pub(crate) mining_default: f64,
    /// The field the mining cost varies by and its cost per category code.
    pub(crate) mining_by_code: Option<(Arc<Vec<f64>>, Vec<f64>)>,
    pub(crate) rehab: PerTonne,
    pub(crate) waste_haulage: PerTonne,
    pub(crate) ore_haulage: PerTonne,
    pub(crate) methods: Vec<Method>,
    /// Every element some method recovers, once each, in the order first met.
    pub(crate) elements: Vec<ReportElement>,
    pub(crate) air: Air,
}

/// An element as reports show it: its grade in every block, and the units.
#[derive(Clone, Debug)]
pub(crate) struct ReportElement {
    pub(crate) name: String,
    pub(crate) values: Arc<Vec<f64>>,
    /// Sales units per tonne of rock per grade unit (see [`metal_factor`]).
    pub(crate) factor: f64,
    pub(crate) grade_unit: String,
    pub(crate) sales_unit: String,
}

impl Economics {
    pub(crate) fn mining_cost(&self, block: usize) -> f64 {
        match &self.mining_by_code {
            None => self.mining_default,
            Some((codes, table)) => code_index(codes[block], table.len()).map_or(self.mining_default, |code| table[code]),
        }
    }
}

/// What shells a run makes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ShellPlan {
    /// One shell per revenue factor, ascending (one factor for a single shell).
    Factors(Vec<f64>),
    /// Shells that grow with the revenue factors and advance from `start`
    /// towards `bearing`: shell k uses the k-th factor and lets ore count only
    /// up to k/N of the way across the final pit.
    Directional { factors: Vec<f64>, start: DVec2, front: Front },
}

impl ShellPlan {
    pub(crate) fn count(&self) -> usize {
        match self {
            Self::Factors(factors) => factors.len(),
            Self::Directional { factors, .. } => factors.len(),
        }
    }
}

/// The overall slope a run builds its precedence from.
#[derive(Clone)]
pub(crate) enum SlopeSpec {
    /// One slope for every block (a single angle or a rosette).
    Uniform(mineflow::Slope),
    /// Each block's angle in degrees from a field; blocks without a valid one
    /// (blank, or outside 0..90) take `fallback`.
    Field { values: Arc<Vec<f64>>, fallback: f64 },
}

/// How a directional shell's mining front advances from the starting point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Front {
    /// A straight front at right angles to this bearing (degrees clockwise from north).
    Straight { bearing: f64 },
    /// Rings out from the point.
    Radial,
}

/// A scenario ready to run.
#[derive(Clone)]
pub(crate) struct Prepared {
    pub(crate) economics: Economics,
    pub(crate) slope: SlopeSpec,
    pub(crate) plan: ShellPlan,
    /// Things worth knowing that do not stop the run.
    pub(crate) warnings: Vec<String>,
}

/// The index of a category code stored as a float, if it is one and within `len`.
pub(crate) fn code_index(code: f64, len: usize) -> Option<usize> {
    (code.is_finite() && code >= 0.0 && code.fract() == 0.0 && (code as usize) < len).then_some(code as usize)
}

/// A table over a categorical field's codes: `pick(label)` for each code, `fallback` for gaps.
fn code_table<T: Clone>(variable: &BlockVariable, fallback: T, mut pick: impl FnMut(&str) -> T) -> Vec<T> {
    let len = variable.strings.keys().max().map_or(0, |code| *code as usize + 1);
    let mut table = vec![fallback; len];
    for (code, label) in &variable.strings {
        table[*code as usize] = pick(label);
    }
    table
}

struct Checker<'a> {
    scenario: &'a OptimizationScenario,
    model: &'a BlockModelData,
    issues: Vec<String>,
    warnings: Vec<String>,
}

impl<'a> Checker<'a> {
    fn value<T: ValueKind>(&mut self, field: &FieldValue<T>, what: impl FnOnce() -> String) -> T {
        field.resolve(&self.scenario.constants).unwrap_or_else(|| {
            self.issues
                .push(tr!("opt-run-missing-constant", what = what(), constant = field.constant.clone().unwrap_or_default()));
            T::default()
        })
    }

    fn numeric(&mut self, name: &str, what: impl FnOnce() -> String) -> Option<Arc<Vec<f64>>> {
        if name.is_empty() {
            self.issues.push(tr!("opt-run-field-not-set", what = what()));
            return None;
        }
        let numeric = self.model.numeric_variables().iter().any(|variable| variable.name == name);
        match self.model.shared_numeric_values(name) {
            Some(values) if numeric => Some(values),
            _ => {
                self.issues.push(tr!("opt-run-field-missing", what = what(), field = name.to_owned()));
                None
            }
        }
    }

    fn categorical(&mut self, name: &str, what: impl FnOnce() -> String) -> Option<(&'a BlockVariable, Arc<Vec<f64>>)> {
        if name.is_empty() {
            self.issues.push(tr!("opt-run-field-not-set", what = what()));
            return None;
        }
        let model: &'a BlockModelData = self.model;
        let variable = model.variable(name).filter(|variable| !variable.strings.is_empty());
        match (variable, model.shared_numeric_values(name)) {
            (Some(variable), Some(values)) => Some((variable, values)),
            _ => {
                self.issues.push(tr!("opt-run-field-missing", what = what(), field = name.to_owned()));
                None
            }
        }
    }

    fn per_tonne(&mut self, cost: &HaulageCost, what: impl Fn() -> String) -> PerTonne {
        match cost.mode {
            HaulageMode::Value => PerTonne::Value(self.value(&cost.value, &what)),
            HaulageMode::Field => self.numeric(&cost.field, &what).map_or(PerTonne::Value(0.0), PerTonne::Field),
        }
    }
}

/// Check `scenario` against `model` and resolve it. `topography` is the
/// surface named for air exclusion, when the scenario uses one and it was found.
pub(crate) fn prepare(scenario: &OptimizationScenario, model: &BlockModelData, topography: Option<Topography>) -> Result<Prepared, Vec<String>> {
    let mut check = Checker {
        scenario,
        model,
        issues: Vec::new(),
        warnings: Vec::new(),
    };

    let default_density = check.value(&scenario.default_density, || tr!("opt-default-density"));
    if !(default_density.is_finite() && default_density > 0.0) {
        check.issues.push(tr!("opt-run-default-density"));
    }
    let density = (!scenario.density_field.is_empty())
        .then(|| check.numeric(&scenario.density_field, || tr!("opt-density-field")))
        .flatten();
    let quality = check.numeric(&scenario.quality_field, || tr!("opt-quality-field"));
    let needs_rock = scenario.methods.iter().any(|method| !method.rocktype.is_empty()) || (scenario.exclude_air && scenario.air_mode == AirMode::Rocktype);
    let rock = if needs_rock {
        check
            .categorical(&scenario.rocktype_field, || tr!("opt-rocktype-field"))
            .map(|(variable, values)| (variable.clone(), values))
    } else {
        model
            .variable(&scenario.rocktype_field)
            .filter(|variable| !variable.strings.is_empty())
            .cloned()
            .zip(model.shared_numeric_values(&scenario.rocktype_field))
    };

    // Mining costs.
    let mining_default = check.value(&scenario.mining_cost, || tr!("opt-mining-cost"));
    let mining_by_code = if scenario.cost_field.is_empty() {
        None
    } else {
        let mut costs: BTreeMap<String, f64> = BTreeMap::new();
        for row in &scenario.rocktype_costs {
            if row.rocktype.is_empty() || costs.contains_key(&row.rocktype) {
                continue;
            }
            let cost = check.value(&row.cost, || tr!("opt-run-what-rock-cost", rocktype = row.rocktype.clone()));
            costs.insert(row.rocktype.clone(), f64::from(cost));
        }
        check
            .categorical(&scenario.cost_field, || tr!("opt-cost-by-field"))
            .map(|(variable, values)| (values, code_table(variable, mining_default, |label| costs.get(label).copied().unwrap_or(mining_default))))
    };
    let rehab = check.per_tonne(&scenario.rehab_cost, || tr!("opt-rehab-cost"));
    let waste_haulage = check.per_tonne(&scenario.waste_haulage, || tr!("opt-waste-haulage"));
    let ore_haulage = check.per_tonne(&scenario.ore_haulage, || tr!("opt-ore-haulage"));

    // Revenues: the first row per element counts.
    let mut revenues: BTreeMap<&str, (f64, f64, String, String)> = BTreeMap::new();
    for row in &scenario.revenues {
        if row.element.is_empty() {
            continue;
        }
        if revenues.contains_key(row.element.as_str()) {
            check.warnings.push(tr!("opt-run-duplicate-revenue", element = row.element.clone()));
            continue;
        }
        let price = check.value(&row.price, || tr!("opt-run-what-price", element = row.element.clone()));
        let selling = check.value(&row.selling_cost, || tr!("opt-run-what-selling-cost", element = row.element.clone()));
        if selling > price {
            check.issues.push(tr!("opt-run-selling-above-price", element = row.element.clone()));
        }
        revenues.insert(
            row.element.as_str(),
            (
                (price - selling).max(0.0),
                metal_factor(row.grade_unit, row.sales_unit),
                row.grade_unit.label(),
                row.sales_unit.label(),
            ),
        );
    }

    // Processing methods.
    if scenario.methods.is_empty() {
        check.issues.push(tr!("opt-run-no-methods"));
    }
    let mut methods = Vec::with_capacity(scenario.methods.len());
    let mut report_elements: Vec<ReportElement> = Vec::new();
    for method in &scenario.methods {
        let name = method.name.clone();
        let min = check.value(&method.min_grade, || tr!("opt-run-what-method", method = name.clone(), what = tr!("opt-col-min-grade")));
        let max = check.value(&method.max_grade, || tr!("opt-run-what-method", method = name.clone(), what = tr!("opt-col-max-grade")));
        let threshold = check.value(&method.threshold, || tr!("opt-run-what-method", method = name.clone(), what = tr!("opt-col-threshold")));
        let processing = check.value(&method.processing_cost, || {
            tr!("opt-run-what-method", method = name.clone(), what = tr!("opt-col-processing-cost"))
        });
        let ga = check.value(&method.ga_cost, || tr!("opt-run-what-method", method = name.clone(), what = tr!("opt-ga-costs")));
        let factor = check.value(&method.haulage_factor, || {
            tr!("opt-run-what-method", method = name.clone(), what = tr!("opt-haulage-factor"))
        });
        if max <= min {
            check.issues.push(tr!("opt-run-method-grade-range", method = name.clone()));
        }
        let accepts = if method.rocktype.is_empty() {
            None
        } else {
            rock.as_ref().map(|(variable, _)| code_table(variable, false, |label| label == method.rocktype))
        };
        let mut elements = Vec::with_capacity(method.elements.len());
        for element in &method.elements {
            if element.element.is_empty() {
                continue;
            }
            let what = || tr!("opt-run-what-method", method = name.clone(), what = element.element.clone());
            let recovery = check.value(&element.recovery, what);
            let cost = check.value(&element.cost, what);
            if !(0.0..=100.0).contains(&recovery) {
                check.issues.push(tr!("opt-run-recovery-range", method = name.clone(), element = element.element.clone()));
            }
            let Some(values) = check.numeric(&element.element, what) else {
                continue;
            };
            let (net_price, factor, grade_unit, sales_unit) = revenues.get(element.element.as_str()).cloned().unwrap_or_else(|| {
                check.warnings.push(tr!("opt-run-no-revenue", element = element.element.clone()));
                (0.0, 1.0, String::new(), String::new())
            });
            let index = match report_elements.iter().position(|known| known.name == element.element) {
                Some(index) => index,
                None => {
                    report_elements.push(ReportElement {
                        name: element.element.clone(),
                        values: Arc::clone(&values),
                        factor,
                        grade_unit,
                        sales_unit,
                    });
                    report_elements.len() - 1
                }
            };
            elements.push(Element {
                index,
                values,
                threshold: if element.element == scenario.quality_field { threshold } else { 0.0 },
                recovery: recovery / 100.0,
                factor,
                net_price,
                cost,
            });
        }
        methods.push(Method {
            name: name.trim().to_owned(),
            accepts,
            min,
            max,
            per_tonne: processing + ga,
            processing,
            ga,
            ore_haulage_factor: factor,
            elements,
        });
    }
    method_warnings(scenario, &mut check.warnings);

    // Air.
    let air = if !scenario.exclude_air {
        Air::None
    } else {
        match scenario.air_mode {
            AirMode::Topography => match topography {
                Some(surface) => Air::Topography(surface),
                None => {
                    check.issues.push(tr!("opt-run-topography-missing", name = scenario.air_topography.clone()));
                    Air::None
                }
            },
            AirMode::Rocktype => {
                if scenario.air_rocktype.is_empty() {
                    check.issues.push(tr!("opt-run-air-value-not-set"));
                }
                rock.as_ref()
                    .map_or(Air::None, |(variable, _)| Air::Rock(code_table(variable, false, |label| label == scenario.air_rocktype)))
            }
        }
    };

    let slope = slope(&mut check);
    let plan = shell_plan(&mut check);

    if !check.issues.is_empty() {
        return Err(check.issues);
    }
    let (Some(quality), Some(slope), Some(plan)) = (quality, slope, plan) else {
        return Err(vec![tr!("opt-run-not-ready")]);
    };
    Ok(Prepared {
        economics: Economics {
            density,
            default_density,
            quality,
            rock: rock.map(|(_, values)| values),
            mining_default,
            mining_by_code,
            rehab,
            waste_haulage,
            ore_haulage,
            methods,
            elements: report_elements,
            air,
        },
        slope,
        plan,
        warnings: check.warnings,
    })
}

/// Methods that overlap (they share rock and grades but are different
/// destinations), and rows of one destination that do not agree on costs.
fn method_warnings(scenario: &OptimizationScenario, warnings: &mut Vec<String>) {
    let constants = &scenario.constants;
    let number = |field: &FieldValue<f64>| field.resolve(constants).unwrap_or(f64::NAN);
    for (index, first) in scenario.methods.iter().enumerate() {
        for second in &scenario.methods[index + 1..] {
            let same_rock = first.rocktype.is_empty() || second.rocktype.is_empty() || first.rocktype == second.rocktype;
            if first.name != second.name {
                let overlap = number(&first.min_grade).max(number(&second.min_grade)) < number(&first.max_grade).min(number(&second.max_grade));
                if same_rock && overlap {
                    warnings.push(tr!("opt-run-methods-overlap", first = first.name.clone(), second = second.name.clone()));
                }
            } else {
                let costs = |method: &crate::model::optimization::ProcessingMethod| {
                    let mut costs = vec![number(&method.processing_cost), number(&method.ga_cost), number(&method.haulage_factor)];
                    costs.extend(method.elements.iter().flat_map(|element| [number(&element.recovery), number(&element.cost)]));
                    costs
                };
                if costs(first) != costs(second) {
                    warnings.push(tr!("opt-run-same-name-costs", method = first.name.clone()));
                }
            }
        }
    }
}

/// Small enough that a sector reads as a step, large enough to stay distinct
/// from the next sector's start after mineflow normalises the azimuths.
const STEP_EPSILON_DEGREES: f64 = 1.0e-6;
/// Bearings sampled round the circle for the smooth interpolations.
const INTERPOLATION_SAMPLES: usize = 360;

fn slope(check: &mut Checker) -> Option<SlopeSpec> {
    let settings = &check.scenario.slope;
    let result = match settings.mode {
        SlopeMode::Single | SlopeMode::Field => {
            let angle = check.value(&settings.angle, || tr!("opt-slope-default-angle"));
            if !(angle > 0.0 && angle < 90.0) {
                check.issues.push(tr!("opt-run-angle-range"));
                return None;
            }
            if settings.mode == SlopeMode::Field {
                let values = check.numeric(&settings.field, || tr!("opt-slope-field"))?;
                return Some(SlopeSpec::Field { values, fallback: angle });
            }
            mineflow::Slope::constant(mineflow::degrees(angle))
        }
        SlopeMode::Rosette => {
            let sectors = match settings.sectors(&check.scenario.constants) {
                Ok(sectors) => sectors,
                Err(RosetteIssue::NoRows) => {
                    check.issues.push(tr!("opt-run-rosette-empty"));
                    return None;
                }
                Err(RosetteIssue::SameBearing) => {
                    check.issues.push(tr!("opt-rosette-same-bearing"));
                    return None;
                }
            };
            if sectors.len() != settings.rosette.len() {
                check.issues.push(tr!("opt-run-rosette-constant"));
                return None;
            }
            if sectors.iter().any(|sector| !(sector.angle > 0.0 && sector.angle < 90.0)) {
                check.issues.push(tr!("opt-run-angle-range"));
                return None;
            }
            let rows: Vec<(f64, f64)> = sectors.iter().map(|sector| (sector.from, sector.angle)).collect();
            // The editor offers only what the rows allow; a file saved otherwise
            // falls back to the sectors.
            let interpolation = if rows.len() >= settings.interpolation.min_rows() {
                settings.interpolation
            } else {
                check.warnings.push(tr!("opt-run-interpolation-fallback", interpolation = settings.interpolation.label()));
                RosetteInterpolation::Step
            };
            match interpolation {
                RosetteInterpolation::Step if sectors.len() == 1 => mineflow::Slope::constant(mineflow::degrees(sectors[0].angle)),
                RosetteInterpolation::Step => {
                    let pairs: Vec<(f64, f64)> = sectors
                        .iter()
                        .flat_map(|sector| [(sector.from, sector.angle), (sector.to - STEP_EPSILON_DEGREES, sector.angle)])
                        .collect();
                    mineflow::Slope::from_degree_pairs(&pairs)
                }
                RosetteInterpolation::Linear => mineflow::Slope::from_degree_pairs(&rows),
                RosetteInterpolation::Cosine => mineflow::Slope::from_degree_pairs(&rows).and_then(|slope| slope.cosine_interpolated(INTERPOLATION_SAMPLES)),
                RosetteInterpolation::Cubic => mineflow::Slope::from_degree_pairs(&rows).and_then(|slope| slope.cubic_interpolated(INTERPOLATION_SAMPLES)),
            }
        }
    };
    result
        .map(SlopeSpec::Uniform)
        .map_err(|error| check.issues.push(tr!("opt-run-slope-invalid", error = error.to_string())))
        .ok()
}

fn shell_plan(check: &mut Checker) -> Option<ShellPlan> {
    let output = &check.scenario.output;
    if output.mode == ShellMode::Single {
        return Some(ShellPlan::Factors(vec![1.0]));
    }
    let factors = match output.factor_input {
        FactorInput::Range => {
            let (from, to) = (output.factor_from, output.factor_to);
            let count = shell_count_for_step(from, to, output.factor_step).map(|count| count as usize);
            let Some(count) = count.filter(|count| (2..=MAX_SHELLS).contains(count) && from > 0.0) else {
                check.issues.push(tr!("opt-run-factor-range", max = MAX_SHELLS.to_string()));
                return None;
            };
            let step = (to - from) / (count - 1) as f64;
            // Rounded, so a factor reads 0.9 rather than 0.8999999999999999.
            let factor = |index: usize| round_factor(from + step * index as f64);
            (0..count).map(|index| if index + 1 == count { to } else { factor(index) }).collect()
        }
        FactorInput::List => match parse_factor_list(&output.factor_list) {
            Ok(factors) if factors.len() <= MAX_SHELLS => factors,
            Ok(_) => {
                check.issues.push(tr!("opt-run-factor-range", max = MAX_SHELLS.to_string()));
                return None;
            }
            Err(issue) => {
                check.issues.push(issue.message());
                return None;
            }
        },
    };
    if output.use_directional_shells {
        let Some((x, y)) = output.shell_start else {
            check.issues.push(tr!("opt-run-no-start"));
            return None;
        };
        return Some(ShellPlan::Directional {
            factors,
            start: DVec2::new(x, y),
            front: output.shell_direction.bearing().map_or(Front::Radial, |bearing| Front::Straight { bearing }),
        });
    }
    Some(ShellPlan::Factors(factors))
}
