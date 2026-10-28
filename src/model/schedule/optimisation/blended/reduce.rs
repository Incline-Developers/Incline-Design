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

use std::collections::{BTreeMap, BTreeSet};

use super::{
    input::BlendInput,
    replay::{BlendSolution, MovementRow},
};
use crate::model::schedule::optimisation::{Activity, DestinationId, DestinationKind, MaterialId, MovementCandidate, SourceId};

pub(crate) struct Reduced {
    pub(crate) input: BlendInput,
    /// Each reduced candidate's original candidates, with each one's share
    /// of its tonnes.
    back: Vec<Vec<(usize, f64)>>,
}

impl Reduced {
    pub(crate) fn new(input: &BlendInput) -> Self {
        // Duplicates: the first of each set of identical candidates stands
        // for all of them. Debug formatting is exact for every field,
        // floating point included.
        let mut first: BTreeMap<String, usize> = BTreeMap::new();
        let canonical: Vec<usize> = input
            .movements
            .iter()
            .enumerate()
            .map(|(index, candidate)| *first.entry(format!("{candidate:?}")).or_insert(index))
            .collect();

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
        let unmaterialled = |candidate: &MovementCandidate| {
            format!(
                "{:?}",
                MovementCandidate {
                    material: MaterialId(0),
                    ..candidate.clone()
                }
            )
        };

        // Each block's distinct candidates by material.
        let mut by_block: BTreeMap<(SourceId, MaterialId), Vec<usize>> = BTreeMap::new();
        for (index, candidate) in input.movements.iter().enumerate() {
            if candidate.activity == Activity::Dig && canonical[index] == index {
                by_block.entry((candidate.source, candidate.material)).or_default().push(index);
            }
        }
        // For a merged material, the representative it joins; and every
        // candidate's stand-in: itself, or the representative's candidate on
        // the same route.
        let mut stand_in: Vec<usize> = canonical.clone();
        let mut ground = input.ground.clone();
        for source in &mut ground {
            let block = SourceId::Ground(source.id);
            // Mergeable materials grouped by the routes they take.
            let mut groups: BTreeMap<Vec<String>, Vec<usize>> = BTreeMap::new();
            for (position, share) in source.material.iter().enumerate() {
                let Some(candidates) = by_block.get(&(block, share.material)) else { continue };
                if !candidates.iter().all(|&index| plain_dump(input.movements[index].destination)) {
                    continue;
                }
                let mut routes: Vec<String> = candidates.iter().map(|&index| unmaterialled(&input.movements[index])).collect();
                routes.sort();
                groups.entry(routes).or_default().push(position);
            }
            let mut dropped = BTreeSet::new();
            for positions in groups.values().filter(|positions| positions.len() > 1) {
                let representative = source.material[positions[0]].material;
                let routes: BTreeMap<String, usize> = by_block[&(block, representative)]
                    .iter()
                    .map(|&index| (unmaterialled(&input.movements[index]), index))
                    .collect();
                for &position in &positions[1..] {
                    for &index in &by_block[&(block, source.material[position].material)] {
                        stand_in[index] = routes[&unmaterialled(&input.movements[index])];
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

        let mut renumbered: BTreeMap<usize, usize> = BTreeMap::new();
        let mut movements = Vec::new();
        for (index, candidate) in input.movements.iter().enumerate() {
            if stand_in[index] == index {
                renumbered.insert(index, movements.len());
                movements.push(candidate.clone());
            }
        }
        let forward: Vec<usize> = stand_in.iter().map(|kept| renumbered[kept]).collect();

        // Each merged candidate's tonnes split over its materials by their
        // fractions, onto each material's first candidate on that route.
        let fraction = |source: SourceId, material: MaterialId| match source {
            SourceId::Ground(ground) => input
                .ground
                .iter()
                .find(|entry| entry.id == ground)
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
        Self { input: reduced, back }
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
