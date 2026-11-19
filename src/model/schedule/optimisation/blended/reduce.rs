//! The exact model's input with columns that can only ever stand in for
//! others taken out, and the way back.
//!
//! Two kinds go, and neither changes what any schedule can do or earn:
//!
//! - **Duplicate candidates.** Capture emits a block's movements once for
//!   each bar listing it, so a block in two bars has every candidate twice,
//!   identical in every field. One column carries what both would.
//! - **A block's waste, material by material.** A material whose every
//!   movement goes to dumps that no grade limit or target reads, on the same
//!   routes at the same truck hours and value as another such material of
//!   the block, differs from it in nothing the model prices or limits. Its
//!   share of each extraction is fixed (the `portion` rows), so the two are
//!   one material with their fractions added. Most of a block's materials
//!   are often like this.
//!
//! A run works on the reduced input throughout (`app::schedule_solve`), and
//! its schedule maps back by splitting each merged movement over its
//! materials by their fractions, which is exactly the split the `portion`
//! rows ask of the original.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rayon::prelude::*;

use super::{
    input::{BlendInput, ConditionalValue},
    replay::{BlendSolution, MovementRow},
};
use crate::model::schedule::{
    optimisation::{Activity, DestinationId, DestinationKind, MaterialId, MovementCandidate, SourceId},
    trucking::CycleBreakdown,
};

/// A candidate by every field it has, its numbers by their bits: two are
/// the same key exactly when they are identical. With `material` left out,
/// the route the candidate takes. Built by naming every field, so a field
/// added to a candidate is a field added here.
#[derive(Clone, PartialEq, Eq, Hash)]
struct CandidateKey {
    material: Option<MaterialId>,
    loader: crate::model::schedule::optimisation::LoaderId,
    activity: Activity,
    source: SourceId,
    destination: DestinationId,
    truck: crate::model::schedule::optimisation::TruckClassId,
    truck_hours_per_tonne: u64,
    cycle: [u64; 8],
    routing_rule: crate::model::schedule::optimisation::RoutingRuleId,
    routing_preference: u32,
    cashflow: Vec<(crate::model::schedule::optimisation::CashflowRuleId, u64)>,
}

impl CandidateKey {
    fn new(candidate: &MovementCandidate, with_material: bool) -> Self {
        let MovementCandidate {
            loader,
            activity,
            source,
            material,
            destination,
            truck,
            truck_hours_per_tonne,
            cycle,
            routing_rule,
            routing_preference,
            cashflow,
        } = candidate;
        let CycleBreakdown {
            spot_h,
            load_h,
            loaded_h,
            dump_h,
            empty_h,
            loaded_km,
            empty_km,
            rise_m,
        } = *cycle;
        Self {
            material: with_material.then_some(*material),
            loader: *loader,
            activity: *activity,
            source: *source,
            destination: *destination,
            truck: *truck,
            truck_hours_per_tonne: truck_hours_per_tonne.to_bits(),
            cycle: [spot_h, load_h, loaded_h, dump_h, empty_h, loaded_km, empty_km, rise_m].map(f64::to_bits),
            routing_rule: *routing_rule,
            routing_preference: *routing_preference,
            cashflow: cashflow.iter().map(|item| (item.rule, item.value_per_tonne.to_bits())).collect(),
        }
    }
}

pub(crate) struct Reduced {
    pub(crate) input: BlendInput,
    /// Each reduced candidate's original candidates, with each one's share
    /// of its tonnes.
    back: Vec<Vec<(usize, f64)>>,
    /// Each original candidate's reduced one.
    forward: Vec<usize>,
}

impl Reduced {
    pub(crate) fn new(input: &BlendInput) -> Self {
        // Every candidate by its key, built side by side: a large model's
        // candidates run to hundreds of thousands.
        let names: Vec<CandidateKey> = input.movements.par_iter().map(|candidate| CandidateKey::new(candidate, true)).collect();

        // Duplicates: the first of each set of identical candidates stands
        // for all of them.
        let mut first: HashMap<&CandidateKey, usize, foldhash::fast::FixedState> = HashMap::with_capacity_and_hasher(names.len(), Default::default());
        let canonical: Vec<usize> = names.iter().enumerate().map(|(index, name)| *first.entry(name).or_insert(index)).collect();
        drop(first);

        // Dumps whose receipts no grade reads.
        let graded: BTreeSet<DestinationId> = input
            .grade_limits
            .iter()
            .map(|limit| limit.destination)
            .chain(input.grade_targets.iter().map(|target| target.destination))
            .collect();
        let plain_dump = |destination: DestinationId| {
            !graded.contains(&destination)
                && input
                    .destinations
                    .iter()
                    .any(|entry| entry.id == destination && matches!(entry.kind, DestinationKind::Dump))
        };

        // Each block's distinct candidates by material.
        let mut by_block: HashMap<(SourceId, MaterialId), Vec<usize>> = HashMap::new();
        for (index, candidate) in input.movements.iter().enumerate() {
            if candidate.activity == Activity::Dig && canonical[index] == index {
                by_block.entry((candidate.source, candidate.material)).or_default().push(index);
            }
        }
        // The route each of those takes: its key with the material left out,
        // numbered.
        let mut route_of: Vec<Option<u32>> = vec![None; input.movements.len()];
        let mut numbered: HashMap<CandidateKey, u32, foldhash::fast::FixedState> = HashMap::default();
        for &index in by_block.values().flatten() {
            let next = numbered.len() as u32;
            route_of[index] = Some(*numbered.entry(CandidateKey::new(&input.movements[index], false)).or_insert(next));
        }
        drop(numbered);
        let route = |index: usize| route_of[index].expect("every block candidate has its route");

        // For a merged material, the representative it joins; and every
        // candidate's stand-in: itself, or the representative's candidate on
        // the same route.
        let mut stand_in: Vec<usize> = canonical.clone();
        let mut ground = input.ground.clone();
        for source in &mut ground {
            let block = SourceId::Ground(source.id);
            // Mergeable materials grouped by the routes they take.
            let mut groups: BTreeMap<Vec<u32>, Vec<usize>> = BTreeMap::new();
            for (position, share) in source.material.iter().enumerate() {
                let Some(candidates) = by_block.get(&(block, share.material)) else { continue };
                if !candidates.iter().all(|&index| plain_dump(input.movements[index].destination)) {
                    continue;
                }
                let mut routes: Vec<u32> = candidates.iter().map(|&index| route(index)).collect();
                routes.sort_unstable();
                groups.entry(routes).or_default().push(position);
            }
            let mut dropped = BTreeSet::new();
            for positions in groups.values().filter(|positions| positions.len() > 1) {
                let representative = source.material[positions[0]].material;
                let routes: HashMap<u32, usize, foldhash::fast::FixedState> = by_block[&(block, representative)].iter().map(|&index| (route(index), index)).collect();
                for &position in &positions[1..] {
                    for &index in &by_block[&(block, source.material[position].material)] {
                        stand_in[index] = routes[&route(index)];
                    }
                    source.material[positions[0]].fraction += source.material[position].fraction;
                    dropped.insert(position);
                }
            }
            let mut position = 0;
            source.material.retain(|_| {
                position += 1;
                !dropped.contains(&(position - 1))
            });
        }
        // Duplicates of a merged material's candidates follow their first.
        for index in 0..stand_in.len() {
            stand_in[index] = stand_in[canonical[index]];
        }

        let mut renumbered: Vec<usize> = vec![usize::MAX; input.movements.len()];
        let mut movements = Vec::new();
        for (index, candidate) in input.movements.iter().enumerate() {
            if stand_in[index] == index {
                renumbered[index] = movements.len();
                movements.push(candidate.clone());
            }
        }
        let forward: Vec<usize> = stand_in.iter().map(|&kept| renumbered[kept]).collect();

        // Each merged candidate's tonnes split over its materials by their
        // fractions, onto each material's first candidate on that route.
        let blocks: HashMap<_, _> = input.ground.iter().rev().map(|entry| (entry.id, entry)).collect();
        let fraction = |source: SourceId, material: MaterialId| match source {
            SourceId::Ground(ground) => blocks
                .get(&ground)
                .and_then(|entry| entry.material.iter().find(|share| share.material == material))
                .map_or(0.0, |share| share.fraction),
            SourceId::Stockpile(_) => 1.0,
        };
        let mut back: Vec<Vec<(usize, f64)>> = vec![Vec::new(); movements.len()];
        for (index, candidate) in input.movements.iter().enumerate() {
            if canonical[index] == index {
                back[forward[index]].push((index, fraction(candidate.source, candidate.material)));
            }
        }
        for originals in &mut back {
            let total: f64 = originals.iter().map(|(_, share)| share).sum();
            let count = originals.len() as f64;
            for (_, share) in originals.iter_mut() {
                *share = if total > 0.0 { *share / total } else { 1.0 / count };
            }
        }

        let mut reduced = input.clone();
        reduced.ground = ground;
        reduced.movements = movements;
        // Conditional values name their candidate by position, so they follow
        // it to its new one. A dropped duplicate's are its stand-in's own, and
        // go with it; a merged material's candidates are digs, which carry
        // none.
        reduced.conditional_values = input
            .conditional_values
            .iter()
            .filter(|value| stand_in[value.candidate] == value.candidate)
            .map(|value| ConditionalValue {
                candidate: renumbered[value.candidate],
                ..value.clone()
            })
            .collect();
        Self { input: reduced, back, forward }
    }

    /// `solution`, a schedule of the original input, as one of the reduced:
    /// each original candidate's tonnes onto the one standing for it, which
    /// is exact, as the portions split them as the merged column would.
    pub(crate) fn compress(&self, solution: &BlendSolution) -> BlendSolution {
        let mut compressed = solution.clone();
        let mut rows: BTreeMap<(usize, usize, usize), f64> = BTreeMap::new();
        for row in &solution.movements {
            *rows.entry((row.interval, row.segment, self.forward[row.candidate])).or_default() += row.tonnes_t;
        }
        compressed.movements = rows
            .into_iter()
            .map(|((interval, segment, candidate), tonnes_t)| MovementRow {
                candidate,
                interval,
                segment,
                tonnes_t,
            })
            .collect();
        for chunk in &mut compressed.chunks {
            let mut receipts: Vec<(usize, f64)> = Vec::new();
            for &(candidate, tonnes) in &chunk.receipts {
                let candidate = self.forward[candidate];
                match receipts.iter_mut().find(|(kept, _)| *kept == candidate) {
                    Some((_, held)) => *held += tonnes,
                    None => receipts.push((candidate, tonnes)),
                }
            }
            chunk.receipts = receipts;
        }
        compressed
    }

    /// `solution`, a schedule of the reduced input, as one of the original.
    pub(crate) fn expand(&self, solution: BlendSolution) -> BlendSolution {
        let mut expanded = solution;
        expanded.movements = expanded
            .movements
            .iter()
            .flat_map(|row| {
                self.back[row.candidate].iter().map(move |&(candidate, share)| MovementRow {
                    candidate,
                    tonnes_t: row.tonnes_t * share,
                    ..*row
                })
            })
            .collect();
        for chunk in &mut expanded.chunks {
            chunk.receipts = chunk
                .receipts
                .iter()
                .flat_map(|&(candidate, tonnes)| self.back[candidate].iter().map(move |&(original, share)| (original, tonnes * share)))
                .collect();
        }
        expanded
    }
}
