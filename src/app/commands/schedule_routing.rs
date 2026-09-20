//! Turning measured dig blocks into routed material: which destination each
//! portion of a block may go to, for each loader that might dig it.
//!
//! This is where the ordered rules are *resolved* and nowhere else. The
//! evaluator receives, per block and per loader, a list of candidate
//! destinations in priority order and nothing about grades or rock types at
//! all - so capacity can decide between the destinations a rule allowed
//! without ever being able to decide which destinations were allowed.
//!
//! Three rules shape the whole module:
//!
//! - **A condition is tested against the material, not against an average.**
//!   Every value read here is the mapped value on one contributing block-model
//!   row, before spatial proration. A dig block's weighted-average grade, and
//!   the marginal category breakdowns beside it, cannot say whether a category
//!   and a grade occur in the *same* material; the retained rows can.
//! - **A portion with no matching rule refuses the run.** When routing is on,
//!   extracted material has to have somewhere to go, and "nothing matched" is a
//!   configuration error with the loader, the block and the tonnage named. It is
//!   deliberately not the same answer as "every matching destination is full",
//!   which is a valid configuration the evaluator resolves in time.
//! - **The portions of a block always add up to the block.** They come from the
//!   same scan as its total, so a disagreement beyond rounding means the two are
//!   not describing the same measurement, and that refuses the run rather than
//!   routing a number nobody measured.

use super::solids_view::DigBlockRecord;
use crate::{
    i18n::tr,
    model::{
        Document, ReserveFieldId,
        schedule::{
            DestinationId, DestinationKind, DispatchDestination, DispatchPortion, LoaderAgentId, PortionValue, RouteSource, RuleId, SourceScope, destinations,
            destinations::RoutingConfig,
        },
    },
};

/// How far the portions of one block may sum away from its own measured total
/// before the two are treated as different measurements rather than the same one
/// added up in a different order.
///
/// Relative, because the figures are tonnes and a mine's blocks span orders of
/// magnitude; floored, so a block of almost nothing is not held to a tolerance
/// smaller than a float can express.
const RECONCILE_RELATIVE: f64 = 1e-6;
const RECONCILE_ABSOLUTE: f64 = 1e-6;

/// The destinations one run may deliver to, in a fixed order the evaluator
/// indexes into.
///
/// Built from the project rather than stored: a stockpile solid drawn since the
/// last run is in the list, and one deleted since is not - and a rule that named
/// it is refused by name rather than quietly dropped.
pub(crate) fn destination_table(document: &Document) -> Vec<DispatchDestination> {
    let plan = document.schedule();
    let routing = plan.routing();
    destinations::available(document.solids(), routing)
        .into_iter()
        .map(|entry| DispatchDestination {
            id: entry.id,
            kind: entry.kind,
            capacity_t: entry.capacity_t,
            crusher: match (entry.kind, entry.id) {
                (DestinationKind::Crusher, DestinationId::Standalone(id)) => routing.crusher(id).cloned(),
                _ => None,
            },
        })
        .collect()
}

/// One portion of one block as routing prepared it.
pub(crate) struct PreparedPortion {
    pub(crate) tonnes: f64,
    /// Candidate destinations in rule order, each with the rule that offered it,
    /// as indices into the destination table. Several rules may name one
    /// destination; the order is kept as written, because the order is the
    /// resolution.
    pub(crate) routes: Vec<(usize, RuleId)>,
}

/// Why routing could not be prepared for one block.
pub(crate) enum RoutingProblem {
    /// The scan retained nothing about this block's material, so what it is made
    /// of is unknown. Never treated as "made of nothing".
    NoCapture { block: String },
    /// The nominated tonnes field is not among the captured values, so the
    /// portions cannot be weighed.
    TonnageNotCaptured { block: String },
    /// The portions do not add up to the block's own measured total.
    Unreconciled { block: String, portions: f64, total: f64 },
    /// Material this loader would extract that no enabled rule accepts.
    Unmatched { block: String, loader: String, tonnes: f64 },
}

impl RoutingProblem {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::NoCapture { block } => tr!("routing-problem-no-capture", block = block.clone()),
            Self::TonnageNotCaptured { block } => tr!("routing-problem-tonnage", block = block.clone()),
            Self::Unreconciled { block, portions, total } => tr!(
                "routing-problem-unreconciled",
                block = block.clone(),
                portions = format!("{portions:.3}"),
                total = format!("{total:.3}")
            ),
            Self::Unmatched { block, loader, tonnes } => tr!("routing-problem-unmatched", block = block.clone(), loader = loader.clone(), tonnes = format!("{tonnes:.2}")),
        }
    }
}

/// Resolve one block's material into routed portions for one loader.
///
/// `total_t` is the block's own measured tonnage, the figure the schedule digs
/// it for; the portions are checked against it rather than replacing it, so the
/// tonnes the evaluator removes from the ground are the tonnes readiness
/// reported.
pub(crate) fn prepare_block(
    block: &DigBlockRecord,
    total_t: f64,
    loader: LoaderAgentId,
    loader_name: &str,
    tonnage_field: ReserveFieldId,
    routing: &RoutingConfig,
    table: &[DispatchDestination],
) -> Result<Vec<PreparedPortion>, Vec<RoutingProblem>> {
    let Some(held) = block.portions.as_ref() else {
        return Err(vec![RoutingProblem::NoCapture { block: block.name.clone() }]);
    };
    let capture = &held.capture;
    let Some(tonnage_position) = capture.position(tonnage_field) else {
        return Err(vec![RoutingProblem::TonnageNotCaptured { block: block.name.clone() }]);
    };
    let mut problems = Vec::new();
    let mut prepared = Vec::new();
    let mut summed = 0.0;
    for portion in held.portions() {
        // A row that had no value for the nominated field contributed nothing
        // to the block's total either, so it carries no tonnes here. That is an
        // absence, not a zero-tonne portion to be routed.
        let value = portion.values[tonnage_position];
        if !value.is_finite() {
            continue;
        }
        let tonnes = value * portion.fraction;
        if !tonnes.is_finite() {
            problems.push(RoutingProblem::Unreconciled {
                block: block.name.clone(),
                portions: tonnes,
                total: total_t,
            });
            continue;
        }
        summed += tonnes;
        if tonnes <= 0.0 {
            continue;
        }
        let bench = (block.bench.base, block.bench.top);
        let flitch = (block.flitch.base, block.flitch.top);
        let routes: Vec<(usize, RuleId)> = routing
            .rules
            .iter()
            .filter(|rule| {
                rule.accepts(
                    loader,
                    RouteSource::Ground {
                        solid: block.solid,
                        bench,
                        flitch,
                    },
                    |field| {
                        capture.position(field).map(|position| {
                            let value = portion.values[position];
                            if capture.is_categorical(position) {
                                match capture.label(value) {
                                    Some(label) => PortionValue::Category(label.to_owned()),
                                    None => PortionValue::Missing,
                                }
                            } else if value.is_finite() {
                                PortionValue::Number(value)
                            } else {
                                PortionValue::Missing
                            }
                        })
                    },
                )
            })
            // One rule may list several destinations, tried in the order it
            // lists them - the same resolution the rule order itself uses, one
            // level down.
            .flat_map(|rule| {
                rule.destinations
                    .iter()
                    .filter_map(|destination| table.iter().position(|entry| entry.id == *destination).map(|position| (position, rule.id)))
            })
            .collect();
        // Two rules may allow the same destination; the first mention is the
        // one that decides where it sits in the order, and a repeat says
        // nothing more.
        let mut seen = Vec::with_capacity(routes.len());
        let routes: Vec<(usize, RuleId)> = routes
            .into_iter()
            .filter(|(position, _)| {
                let fresh = !seen.contains(position);
                if fresh {
                    seen.push(*position);
                }
                fresh
            })
            .collect();
        if routes.is_empty() {
            problems.push(RoutingProblem::Unmatched {
                block: block.name.clone(),
                loader: loader_name.to_owned(),
                tonnes,
            });
            continue;
        }
        prepared.push(PreparedPortion { tonnes, routes });
    }
    // Checked against the block's own total, and only once every portion has
    // been weighed: a partial sum would report a disagreement the measurement
    // does not have.
    let tolerance = RECONCILE_ABSOLUTE + RECONCILE_RELATIVE * total_t.abs();
    if (summed - total_t).abs() > tolerance {
        problems.push(RoutingProblem::Unreconciled {
            block: block.name.clone(),
            portions: summed,
            total: total_t,
        });
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(prepared)
}

/// The evaluator's own view of one block's portions.
pub(crate) fn dispatch_portions(prepared: Vec<PreparedPortion>) -> Vec<DispatchPortion> {
    prepared
        .into_iter()
        .map(|portion| DispatchPortion {
            tonnes: portion.tonnes,
            routes: portion.routes,
        })
        .collect()
}

/// Whether one scope is placeable against the blocks a run produced.
///
/// A scope that is not is reported against the rule that names it rather than
/// repaired: a scope whose bench has been re-cut is a rule that no longer
/// restricts what it was written to restrict, and silently widening it would
/// route material somewhere nobody chose.
pub(crate) fn scope_is_placeable(scope: SourceScope, blocks: &[DigBlockRecord]) -> bool {
    blocks
        .iter()
        .any(|block| scope.covers(block.solid, (block.bench.base, block.bench.top), (block.flitch.base, block.flitch.top)))
}
