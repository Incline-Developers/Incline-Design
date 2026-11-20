//! What a pit design takes out of the ground (or a dump puts on it), bench
//! by bench: the volume between the design and the topography, and where a
//! block model is chosen, what it holds by the project's reserve fields.
//!
//! Worked out in the background once the design is, and kept on the object
//! with what it was worked out from, so it shows again without the block
//! model loaded and pit designs compare side by side.

use serde::{Deserialize, Serialize};

use crate::model::{
    ReserveAggregation, ReserveField, ReserveFieldId,
    block_model::BlockModelId,
    solid_reserves::{ReserveGroup, ReserveTotals},
    triangulation::TriangulationId,
};

/// What a design is measured against: chosen in its settings, but no part
/// of what it is designed from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct EvaluationSettings {
    /// The block model its ground is measured in, by the project's reserve
    /// fields.
    pub(crate) block_model: Option<BlockModelId>,
    /// The optimisation shell it is set against.
    pub(crate) shell: Option<TriangulationId>,
}

/// Why a design's measure is not what it comes to now, for the panel to
/// say.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Waiting {
    /// Its design is being worked out.
    Design,
    /// No topography to close it against is chosen.
    NoTopography,
    /// The topography or block model chosen is not loaded.
    NotLoaded(String),
    /// It is being measured.
    Measuring,
}

/// What a design came to, measured.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Evaluation {
    /// A fingerprint of what it was measured from: the design, the
    /// topography, and the block model with its fields and mapping.
    pub(crate) key: u64,
    /// Why it could not be measured, where it could not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) problem: Option<String>,
    /// The whole design.
    #[serde(default)]
    pub(crate) total: Figures,
    /// Each bench, bottom up: the ground between two of its RLs.
    #[serde(default)]
    pub(crate) benches: Vec<BenchFigures>,
}

impl Evaluation {
    /// One that could not be measured, and why.
    pub(crate) fn failed(key: u64, problem: String) -> Self {
        Self {
            key,
            problem: Some(problem),
            total: Figures::default(),
            benches: Vec::new(),
        }
    }
}

/// One bench's ground.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BenchFigures {
    pub(crate) base: f64,
    pub(crate) top: f64,
    pub(crate) figures: Figures,
}

/// What a piece of ground comes to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Figures {
    /// Its volume, from its shape.
    pub(crate) volume: f64,
    /// What the block model holds of it, where one was chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) measured: Option<Measured>,
}

/// What a block model holds of a piece of ground.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Measured {
    /// How much of it the model's blocks cover.
    pub(crate) covered: f64,
    /// Each numeric reserve field: its sum or average, or none where
    /// nothing gave it a value.
    pub(crate) values: Vec<FieldValue>,
    /// The same, for each category of each category field: by material.
    pub(crate) categories: Vec<CategoryFigures>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FieldValue {
    pub(crate) field: ReserveFieldId,
    pub(crate) value: Option<f64>,
}

/// One category field's breakdown: each of its categories' figures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CategoryFigures {
    pub(crate) field: ReserveFieldId,
    pub(crate) groups: Vec<GroupFigures>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GroupFigures {
    pub(crate) name: String,
    pub(crate) covered: f64,
    pub(crate) values: Vec<FieldValue>,
}

impl Measured {
    /// A reserve scan's totals for one piece of ground, read by `fields`.
    pub(crate) fn of(totals: &ReserveTotals, fields: &[ReserveField]) -> Self {
        let values = |group: &ReserveGroup| -> Vec<FieldValue> {
            fields
                .iter()
                .filter(|field| field.aggregation != ReserveAggregation::Category)
                .map(|field| FieldValue {
                    field: field.id,
                    value: group.numeric.get(&field.id).and_then(|total| total.value(field.aggregation)),
                })
                .collect()
        };
        let mut categories: Vec<CategoryFigures> = fields
            .iter()
            .filter_map(|field| {
                let groups = totals.categories.get(&field.id)?;
                Some(CategoryFigures {
                    field: field.id,
                    groups: groups
                        .iter()
                        .map(|(name, group)| GroupFigures {
                            name: name.clone(),
                            covered: group.covered_volume,
                            values: values(group),
                        })
                        .collect(),
                })
            })
            .collect();
        // The most of each first, as a material breakdown reads.
        for category in &mut categories {
            category.groups.sort_by(|a, b| b.covered.total_cmp(&a.covered).then_with(|| a.name.cmp(&b.name)));
        }
        Self {
            covered: totals.all.covered_volume,
            values: values(&totals.all),
            categories,
        }
    }

    /// The value of `field`, if it has one.
    pub(crate) fn value(&self, field: ReserveFieldId) -> Option<f64> {
        self.values.iter().find(|value| value.field == field)?.value
    }
}

impl GroupFigures {
    /// The value of `field` in this group, if it has one.
    pub(crate) fn value(&self, field: ReserveFieldId) -> Option<f64> {
        self.values.iter().find(|value| value.field == field)?.value
    }
}
