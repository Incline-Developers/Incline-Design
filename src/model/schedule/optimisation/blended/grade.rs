//! Which reserve fields may be used as a blended grade, and how a material's
//! contained quantity is derived from one.
//!
//! # Why this is a gate and not a lookup
//!
//! The accepted optimisation input models material as *identity*: a
//! [`Material`] is an id and a label, because routing and cashflow matching
//! were already resolved during capture. A blended pile needs numbers, so the
//! experimental input builder has to reach back to the project's reserve
//! fields - and those are not interchangeable.
//!
//! [`ReserveAggregation`] distinguishes three things a column can mean:
//!
//! - [`ReserveAggregation::Sum`] - an *extensive* quantity. Tonnes, or
//!   contained metal. Averaging one of these is meaningless.
//! - [`ReserveAggregation::WeightedAverage`] - an *intensive* quantity,
//!   averaged by another field's per-block values. This is a grade.
//! - [`ReserveAggregation::Category`] - a label. Not a number at all.
//!
//! A blended stockpile conserves *contained quantity*, and contained quantity
//! is only `grade x tonnes` when the grade is weighted by the same tonnage
//! basis the schedule itself runs on. A field weighted by volume, or by a
//! different tonnage column, does not compose that way, and a summed field is
//! already a contained quantity rather than a grade. So the rule is exact and
//! checkable, and anything else is rejected with a reason rather than
//! silently coerced.

use std::collections::BTreeMap;

use crate::model::{ReserveAggregation, ReserveField, ReserveFieldId, schedule::optimisation::MaterialId};

/// The unit a grade column is expressed in, and therefore the conversion to a
/// dimensionless mass fraction. Stated explicitly: nothing in the project
/// metadata records whether "Fe" means 0.62 or 62.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GradeBasis {
    /// Already a mass fraction in `0..=1`.
    Fraction,
    /// A percentage in `0..=100`.
    Percent,
}

impl GradeBasis {
    /// Convert a captured value to a dimensionless mass fraction.
    pub(crate) fn to_fraction(self, value: f64) -> f64 {
        match self {
            Self::Fraction => value,
            Self::Percent => value / 100.0,
        }
    }
}

/// Why a reserve field cannot be used as a blended grade.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GradeRejection {
    /// The field id names no field on the document.
    UnknownField(ReserveFieldId),
    /// The field is a grouping label.
    Categorical { field: String },
    /// The field is an extensive quantity. Averaging it would be wrong, and
    /// this is the specific error the brief calls out: do not average summed
    /// quantities as grades.
    Summed { field: String },
    /// The field is a weighted average, but weighted by something other than
    /// the tonnage basis the schedule uses, so `grade x tonnes` is not its
    /// contained quantity.
    ForeignWeight { field: String, weight: String, wanted: String },
    /// The schedule plan has no tonnage field configured, so no weighting can
    /// be checked against it.
    NoTonnageBasis,
    /// A material reaching the blended pile has no value for this field.
    /// Missing is not zero: a blend cannot be computed without it.
    MissingValue { field: String, material: MaterialId },
    /// A captured value is outside the range its basis allows.
    OutOfRange { field: String, material: MaterialId, value: f64 },
}

impl GradeRejection {
    /// A diagnostic a planner can act on.
    pub(crate) fn message(&self) -> String {
        match self {
            Self::UnknownField(id) => format!("reserve field {} is not defined on this project", id.0),
            Self::Categorical { field } => {
                format!("'{field}' is a category field, not a grade; a blended stockpile needs a numeric tonnes-weighted grade")
            }
            Self::Summed { field } => {
                format!("'{field}' is a summed quantity, not a grade; blending it would average a total. Use the weighted-average field it is a total of")
            }
            Self::ForeignWeight { field, weight, wanted } => {
                format!("'{field}' is weighted by '{weight}', but the schedule's tonnage basis is '{wanted}', so grade x tonnes is not its contained quantity")
            }
            Self::NoTonnageBasis => "the schedule plan has no tonnage field configured, so no grade's weighting can be verified".to_string(),
            Self::MissingValue { field, material } => {
                format!("material {} has no '{field}' value; a missing grade cannot be treated as zero in a blend", material.0)
            }
            Self::OutOfRange { field, material, value } => {
                format!("material {}'s '{field}' value {value} is outside the range its declared unit allows", material.0)
            }
        }
    }
}

/// One grade the blended experiment tracks.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GradeField {
    pub(crate) field: ReserveFieldId,
    pub(crate) name: String,
    pub(crate) basis: GradeBasis,
}

impl GradeField {
    /// Check one field against the project's definitions and the schedule's
    /// tonnage basis.
    ///
    /// `tonnage` is the plan's configured tonnage field; a grade qualifies
    /// only when it is a weighted average *by that exact field*.
    pub(crate) fn accept(field: ReserveFieldId, basis: GradeBasis, fields: &[ReserveField], tonnage: Option<ReserveFieldId>) -> Result<Self, GradeRejection> {
        let Some(definition) = fields.iter().find(|entry| entry.id == field) else {
            return Err(GradeRejection::UnknownField(field));
        };
        let name = definition.name.clone();
        let Some(tonnage) = tonnage else {
            return Err(GradeRejection::NoTonnageBasis);
        };
        match definition.aggregation {
            ReserveAggregation::Category => Err(GradeRejection::Categorical { field: name }),
            ReserveAggregation::Sum => Err(GradeRejection::Summed { field: name }),
            ReserveAggregation::WeightedAverage { weight_field } if weight_field == tonnage => Ok(Self { field, name, basis }),
            ReserveAggregation::WeightedAverage { weight_field } => {
                let weight = fields
                    .iter()
                    .find(|entry| entry.id == weight_field)
                    .map(|entry| entry.name.clone())
                    .unwrap_or_else(|| format!("field {}", weight_field.0));
                let wanted = fields
                    .iter()
                    .find(|entry| entry.id == tonnage)
                    .map(|entry| entry.name.clone())
                    .unwrap_or_else(|| format!("field {}", tonnage.0));
                Err(GradeRejection::ForeignWeight { field: name, weight, wanted })
            }
        }
    }

    /// The largest fraction this basis can legitimately represent.
    fn ceiling(&self) -> f64 {
        match self.basis {
            GradeBasis::Fraction => 1.0,
            GradeBasis::Percent => 1.0,
        }
    }
}

/// Per-material grade values, already converted to dimensionless mass
/// fractions, for every grade the experiment tracks.
///
/// Built once by the experimental input builder. Every material that can
/// reach a blended pile must have a value for every tracked grade - the
/// builder refuses rather than defaulting, because a defaulted zero would
/// quietly dilute a blend and a defaulted mean would invent metal.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GradeTable {
    pub(crate) fields: Vec<GradeField>,
    values: BTreeMap<MaterialId, Vec<f64>>,
}

impl GradeTable {
    pub(crate) fn build(fields: Vec<GradeField>, captured: &BTreeMap<MaterialId, Vec<Option<f64>>>) -> Result<Self, GradeRejection> {
        let mut values = BTreeMap::new();
        for (material, raw) in captured {
            let mut row = Vec::with_capacity(fields.len());
            for (position, grade) in fields.iter().enumerate() {
                let Some(Some(value)) = raw.get(position).copied() else {
                    return Err(GradeRejection::MissingValue {
                        field: grade.name.clone(),
                        material: *material,
                    });
                };
                if !value.is_finite() {
                    return Err(GradeRejection::MissingValue {
                        field: grade.name.clone(),
                        material: *material,
                    });
                }
                let fraction = grade.basis.to_fraction(value);
                if !(0.0..=grade.ceiling()).contains(&fraction) {
                    return Err(GradeRejection::OutOfRange {
                        field: grade.name.clone(),
                        material: *material,
                        value,
                    });
                }
                row.push(fraction);
            }
            values.insert(*material, row);
        }
        Ok(Self { fields, values })
    }

    pub(crate) fn count(&self) -> usize {
        self.fields.len()
    }

    /// Mass fraction of `grade` in `material`.
    pub(crate) fn fraction(&self, material: MaterialId, grade: usize) -> Option<f64> {
        self.values.get(&material).and_then(|row| row.get(grade).copied())
    }

    /// The highest fraction any tracked material carries, per grade. Used as
    /// the physically derived upper bound on contained quantity so the
    /// bilinear relaxation stays finite and cannot invent metal.
    pub(crate) fn ceilings(&self) -> Vec<f64> {
        (0..self.count())
            .map(|grade| self.values.values().filter_map(|row| row.get(grade).copied()).fold(0.0_f64, f64::max))
            .collect()
    }
}
