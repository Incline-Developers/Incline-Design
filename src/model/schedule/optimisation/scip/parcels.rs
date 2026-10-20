//! The schedule's ground in parcels, for a long-term plan the whole horizon
//! can afford: blocks that release with the same blast, lie above the same
//! blasts and are dug by the same loaders become one parcel, and their
//! materials one per routing class (the destinations a material may go
//! to), tonnage weighted.

use std::collections::{BTreeMap, BTreeSet};

use super::super::blended::{
    grade::GradeTable,
    input::{BlendInput, authored_tasks},
};
use crate::model::schedule::optimisation::{
    Activity, CashflowContribution, CashflowRuleId, DestinationId, GroundId, GroundSource, LoaderId, MaterialId, MaterialShare, MovementCandidate, RoutingRuleId, SourceId,
    TaskKind, TruckClassId,
};

/// `input` with its ground in parcels, and each block's parcel.
pub(crate) fn parcels(input: &BlendInput) -> (BlendInput, BTreeMap<GroundId, GroundId>) {
    let mut owner: BTreeMap<GroundId, usize> = BTreeMap::new();
    let mut above: BTreeMap<GroundId, BTreeSet<usize>> = BTreeMap::new();
    if let Some(chain) = &input.drill_blast {
        for (index, blast) in chain.blasts.iter().enumerate() {
            for ground in &blast.releases {
                owner.insert(*ground, index);
            }
            for ground in &blast.above {
                above.entry(*ground).or_default().insert(index);
            }
        }
    }
    let mut loaders: BTreeMap<GroundId, BTreeSet<LoaderId>> = BTreeMap::new();
    let mut routes: BTreeMap<MaterialId, BTreeSet<DestinationId>> = BTreeMap::new();
    for candidate in input.movements.iter().filter(|candidate| candidate.activity == Activity::Dig) {
        if let SourceId::Ground(ground) = candidate.source {
            loaders.entry(ground).or_default().insert(candidate.loader);
            routes.entry(candidate.material).or_default().insert(candidate.destination);
        }
    }
    // Each block's parcel, numbered as first met: blocks alike that follow
    // one another in every bar that lists them, so a parcel's first block
    // left is each of those bars' current one.
    type Alike = (Option<usize>, Vec<usize>, Vec<LoaderId>);
    let alike = |ground: GroundId| -> Alike {
        (
            owner.get(&ground).copied(),
            above.get(&ground).map(|blasts| blasts.iter().copied().collect()).unwrap_or_default(),
            loaders.get(&ground).map(|loaders| loaders.iter().copied().collect()).unwrap_or_default(),
        )
    };
    let mut run_of: BTreeMap<GroundId, Vec<usize>> = BTreeMap::new();
    let mut runs = 0;
    for loader_index in 0..input.loaders.len() {
        for task in authored_tasks(input, loader_index) {
            let TaskKind::Dig { sequence } = &input.tasks[task].kind else { continue };
            let mut last: Option<Alike> = None;
            for &ground in sequence {
                let key = alike(ground);
                if last.as_ref() != Some(&key) {
                    runs += 1;
                    last = Some(key);
                }
                run_of.entry(ground).or_default().push(runs);
            }
        }
    }
    type Key = (Alike, Vec<usize>);
    let mut keys: BTreeMap<Key, GroundId> = BTreeMap::new();
    let mut parcel_of: BTreeMap<GroundId, GroundId> = BTreeMap::new();
    for source in &input.ground {
        let key = (alike(source.id), run_of.get(&source.id).cloned().unwrap_or_default());
        let next = GroundId(keys.len() as u32);
        parcel_of.insert(source.id, *keys.entry(key).or_insert(next));
    }
    // Each parcel's materials, one per routing class.
    let mut classes: BTreeMap<Vec<DestinationId>, usize> = BTreeMap::new();
    let mut class_of: BTreeMap<MaterialId, usize> = BTreeMap::new();
    for (material, destinations) in &routes {
        let next = classes.len();
        class_of.insert(*material, *classes.entry(destinations.iter().copied().collect()).or_insert(next));
    }
    let grades = input.grades.count();
    let mut material_of: BTreeMap<(GroundId, usize), MaterialId> = BTreeMap::new();
    let mut tonnes: BTreeMap<MaterialId, f64> = BTreeMap::new();
    let mut contained: BTreeMap<MaterialId, Vec<f64>> = BTreeMap::new();
    let mut ground: BTreeMap<GroundId, f64> = BTreeMap::new();
    for source in &input.ground {
        let parcel = parcel_of[&source.id];
        *ground.entry(parcel).or_default() += source.tonnes_t;
        for share in &source.material {
            let Some(&class) = class_of.get(&share.material) else { continue };
            let next = MaterialId(material_of.len() as u32);
            let material = *material_of.entry((parcel, class)).or_insert(next);
            let t = source.tonnes_t * share.fraction;
            *tonnes.entry(material).or_default() += t;
            let sums = contained.entry(material).or_insert_with(|| vec![0.0; grades]);
            for (grade, sum) in sums.iter_mut().enumerate() {
                *sum += t * input.grades.fraction(share.material, grade).unwrap_or(0.0);
            }
        }
    }
    let captured: BTreeMap<MaterialId, Vec<Option<f64>>> = contained
        .iter()
        .map(|(material, sums)| {
            let t = tonnes[material];
            (*material, sums.iter().map(|sum| Some(if t > 0.0 { sum / t } else { 0.0 })).collect())
        })
        .collect();
    let mut output = input.clone();
    output.grades = GradeTable::build(input.grades.fields.clone(), &captured).expect("every parcel material has every grade");
    output.ground = ground
        .iter()
        .map(|(parcel, total)| GroundSource {
            id: *parcel,
            tonnes_t: *total,
            material: material_of
                .iter()
                .filter(|((of, _), _)| of == parcel)
                .map(|(_, material)| MaterialShare {
                    material: *material,
                    fraction: if *total > 0.0 { tonnes[material] / total } else { 0.0 },
                })
                .collect(),
        })
        .collect();
    // Dig movements joined per parcel material, loader and route, tonnage
    // weighted; the rest as they were.
    type Route = (MaterialId, LoaderId, DestinationId, TruckClassId, RoutingRuleId, u32);
    let mut joined: BTreeMap<Route, (f64, MovementCandidate, BTreeMap<CashflowRuleId, f64>)> = BTreeMap::new();
    let mut movements = Vec::new();
    let block_tonnes: BTreeMap<(GroundId, MaterialId), f64> = input
        .ground
        .iter()
        .flat_map(|source| source.material.iter().map(move |share| ((source.id, share.material), source.tonnes_t * share.fraction)))
        .collect();
    for candidate in &input.movements {
        let SourceId::Ground(block) = candidate.source else {
            movements.push(candidate.clone());
            continue;
        };
        let (Some(&class), Some(&weight)) = (class_of.get(&candidate.material), block_tonnes.get(&(block, candidate.material))) else {
            continue;
        };
        let parcel = parcel_of[&block];
        let material = material_of[&(parcel, class)];
        let key = (
            material,
            candidate.loader,
            candidate.destination,
            candidate.truck,
            candidate.routing_rule,
            candidate.routing_preference,
        );
        let entry = joined.entry(key).or_insert_with(|| {
            let mut first = candidate.clone();
            first.source = SourceId::Ground(parcel);
            first.material = material;
            first.truck_hours_per_tonne = 0.0;
            first.cycle = Default::default();
            (0.0, first, BTreeMap::new())
        });
        entry.0 += weight;
        let joined = &mut entry.1;
        joined.truck_hours_per_tonne += weight * candidate.truck_hours_per_tonne;
        let (c, from) = (&mut joined.cycle, &candidate.cycle);
        c.spot_h += weight * from.spot_h;
        c.load_h += weight * from.load_h;
        c.loaded_h += weight * from.loaded_h;
        c.dump_h += weight * from.dump_h;
        c.empty_h += weight * from.empty_h;
        c.loaded_km += weight * from.loaded_km;
        c.empty_km += weight * from.empty_km;
        c.rise_m += weight * from.rise_m;
        for contribution in &candidate.cashflow {
            *entry.2.entry(contribution.rule).or_default() += weight * contribution.value_per_tonne;
        }
    }
    for (_, (weight, mut candidate, cashflow)) in joined {
        let share = if weight > 0.0 { 1.0 / weight } else { 0.0 };
        candidate.truck_hours_per_tonne *= share;
        let c = &mut candidate.cycle;
        for value in [
            &mut c.spot_h,
            &mut c.load_h,
            &mut c.loaded_h,
            &mut c.dump_h,
            &mut c.empty_h,
            &mut c.loaded_km,
            &mut c.empty_km,
            &mut c.rise_m,
        ] {
            *value *= share;
        }
        candidate.cashflow = cashflow
            .into_iter()
            .map(|(rule, sum)| CashflowContribution {
                rule,
                value_per_tonne: sum * share,
            })
            .collect();
        movements.push(candidate);
    }
    output.movements = movements;
    let regrouped = |blocks: &[GroundId]| {
        let mut seen = BTreeSet::new();
        blocks
            .iter()
            .filter_map(|block| parcel_of.get(block))
            .copied()
            .filter(|parcel| seen.insert(*parcel))
            .collect::<Vec<_>>()
    };
    for task in &mut output.tasks {
        if let TaskKind::Dig { sequence } = &mut task.kind {
            *sequence = regrouped(sequence);
        }
    }
    // Blasts keep their timeline: it is by blast, not by block.
    if let Some(chain) = output.drill_blast.as_mut() {
        for blast in &mut chain.blasts {
            blast.releases = regrouped(&blast.releases);
            blast.above = regrouped(&blast.above);
        }
    }
    (output, parcel_of)
}
