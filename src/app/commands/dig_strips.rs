//! Flitch-owned strip drawings and the dig blocks formed jointly with blasts.
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

use glam::{DVec2, DVec3};

use super::solids_view::{FlitchFaceCache, ViewSolid};
use crate::{
    model::{Object, arrangement},
    ui::state::{BenchSelection, BlastOutline},
};

/// Plan faces, each with a point inside it.
pub(crate) type AnchoredFaces = Vec<(arrangement::Face, DVec2)>;

/// The plan faces one flitch's ground is divided into, north-west first.
///
/// A flitch is cut by its own strips *and* by the blast boundaries of the bench
/// holding it, so a strip crossing a blast boundary yields one block per blast.
/// The bench's own cut lines stand in for those boundaries: a blast boundary is
/// made of bench cuts and stretches of the bench footprint, and the bench
/// footprint lies outside this flitch's, the pit narrowing as it goes down.
///
/// Shared by the derivation the panel reads and the geometry job that cuts the
/// flitch solid apart, so the blocks drawn and the blocks measured are the same
/// blocks.
pub(crate) fn dig_block_faces(footprint: &[Vec<DVec2>], strips: &[Object], bench_cuts: &[Object]) -> AnchoredFaces {
    let mut faces: Vec<_> = arrangement::subdivide(footprint, &dig_block_cuts(strips, bench_cuts))
        .into_iter()
        .filter_map(|face| arrangement::representative_point(&face).map(|anchor| (face, anchor)))
        .collect();
    // Numbered by where they sit, so a block keeps its number while its
    // neighbours are redrawn.
    faces.sort_by(|a, b| b.1.y.total_cmp(&a.1.y).then_with(|| a.1.x.total_cmp(&b.1.x)));
    faces
}

/// [`dig_block_faces`] for the committed body `cache` holds, worked out once
/// for the lines given and kept with the body: opening Dig Strips again, or
/// editing one flitch's strips, derives nothing for the other flitches.
pub(crate) fn cached_dig_block_faces(cache: &FlitchFaceCache, flitch: f64, footprint: &[Vec<DVec2>], strips: &[Object], bench_cuts: &[Object]) -> Arc<AnchoredFaces> {
    cache.faces(flitch, true, lines_key(&[strips, bench_cuts]), || dig_block_faces(footprint, strips, bench_cuts))
}

/// A flitch's ground divided by its bench's cuts alone, so its part of each
/// blast, with a point inside each; kept like [`cached_dig_block_faces`].
pub(crate) fn cached_blast_faces(cache: &FlitchFaceCache, flitch: f64, footprint: &[Vec<DVec2>], bench_cuts: &[Object]) -> Arc<AnchoredFaces> {
    cache.faces(flitch, false, lines_key(&[bench_cuts]), || {
        arrangement::subdivide(footprint, &arrangement::cut_lines(bench_cuts))
            .into_iter()
            .filter_map(|face| arrangement::representative_point(&face).map(|anchor| (face, anchor)))
            .collect()
    })
}

/// Stands for the geometry of each list of lines, in order.
pub(crate) fn lines_key(lines: &[&[Object]]) -> u64 {
    let mut key = DefaultHasher::new();
    for (index, cuts) in lines.iter().enumerate() {
        index.hash(&mut key);
        cuts.len().hash(&mut key);
        cuts.iter().for_each(|cut| cut.geometry_hash().hash(&mut key));
    }
    key.finish()
}

/// The lines a flitch's ground is divided along: its own strips and its
/// bench's cuts.
pub(crate) fn dig_block_cuts(strips: &[Object], bench_cuts: &[Object]) -> Vec<Vec<DVec2>> {
    let mut cuts = arrangement::cut_lines(strips);
    cuts.extend(arrangement::cut_lines(bench_cuts));
    cuts
}

/// What each block of `faces` is cut out of its flitch with: the cell the
/// cut lines alone make around it, where no other block shares that cell,
/// and its own outline where one does.
///
/// A flitch has no ground outside its footprint, so a cell holding one
/// block cuts out exactly that block, without following the footprint round
/// every bend. Each bend would be one more cut through the body, with a cap
/// of its own for every later cut to split again.
pub(crate) fn dig_block_plans(faces: &[(arrangement::Face, DVec2)], cuts: &[Vec<DVec2>]) -> Vec<arrangement::Face> {
    let Some((min, max)) = faces
        .iter()
        .flat_map(|(face, _)| face.iter().flatten())
        .fold(None, |bounds: Option<(DVec2, DVec2)>, point| {
            Some(bounds.map_or((*point, *point), |(min, max)| (min.min(*point), max.max(*point))))
        })
    else {
        return Vec::new();
    };
    let (min, max) = (min - DVec2::ONE, max + DVec2::ONE);
    let cells = arrangement::subdivide(&[vec![min, DVec2::new(max.x, min.y), max, DVec2::new(min.x, max.y)]], cuts);
    let holder: Vec<Option<usize>> = faces
        .iter()
        .map(|(_, anchor)| cells.iter().position(|cell| arrangement::point_in_face(cell, *anchor)))
        .collect();
    faces
        .iter()
        .zip(&holder)
        .map(|((face, _), cell)| match cell {
            Some(cell) if holder.iter().filter(|other| *other == &Some(*cell)).count() == 1 => cells[*cell].clone(),
            _ => face.clone(),
        })
        .collect()
}

/// What the Dig Strips step counts its blocks from, one solid at a time,
/// owned so the count runs as a job.
pub(crate) struct DigCountSolid {
    solid: crate::model::Solid,
    faces: FlitchFaceCache,
    body: Arc<super::solids_view::SolidBody>,
    blasting: Option<Arc<super::solids_view::SolidBlasting>>,
}

/// Stands for everything the dig block count reads: each solid's committed
/// body and the lines dividing each of its flitches.
fn dig_count_key(solids: &[crate::model::Solid], caches: &std::collections::HashMap<crate::model::SolidId, ViewSolid>) -> u64 {
    let mut key = DefaultHasher::new();
    for solid in solids {
        let Some(cache) = caches.get(&solid.id) else { continue };
        let Some(footprints) = cache.flitch_footprints() else { continue };
        (solid.id, cache.body_attempt(), cache.blasting_product().is_some()).hash(&mut key);
        for bench in solid.benching.benches() {
            let bench_cuts = solid.blasting.bench(bench.base).map(|entry| entry.cuts.as_slice()).unwrap_or_default();
            for flitch in bench.flitches.iter().filter(|flitch| footprints.contains_key(&flitch.base.to_bits())) {
                let strips = solid.blasting.drawing(flitch.base, true).map_or(&[][..], |drawing| &drawing.cuts);
                (flitch.base.to_bits(), lines_key(&[strips, bench_cuts])).hash(&mut key);
            }
        }
    }
    key.finish()
}

/// How many dig blocks the strips divide every flitch into, worked out in
/// plan from the flitch ground Benching committed.
///
/// The whole of the Dig Strips step's own work: the same faces the step draws
/// and Reserving later cuts, counted without cutting anything, and held, so
/// a strip edit settles at once and the step opens on blocks already worked
/// out. A flitch whose bench has blasts out of mining has its blast pieces
/// worked out too, which the step reads to leave out a flitch with nothing
/// left to dig.
fn count_dig_blocks(solids: &[DigCountSolid], cancel: &crate::app::jobs::CancelFlag) -> anyhow::Result<usize> {
    use rayon::prelude::*;
    // A survey's flitch ground runs to thousands of points a ring, and finding
    // a point inside each face is most of the work: each flitch is divided
    // once for its ground and lines, and the flitches side by side.
    let mut flitches = Vec::new();
    for item in solids {
        let solid = &item.solid;
        for bench in solid.benching.benches() {
            let bench_cuts = solid.blasting.bench(bench.base).map(|entry| entry.cuts.as_slice()).unwrap_or_default();
            let excluded_blasts = item.blasting.as_ref().is_some_and(|blasting| {
                blasting
                    .blast_faces
                    .iter()
                    .any(|face| (face.bench.base - bench.base).abs() < 1e-6 && solid.exclusions.blast_face_excluded(face.bench.base, &face.face))
            });
            for flitch in &bench.flitches {
                let Some(footprint) = item.body.flitch_footprints.get(&flitch.base.to_bits()) else {
                    continue;
                };
                let strips = solid.blasting.drawing(flitch.base, true).map_or(&[][..], |drawing| &drawing.cuts);
                flitches.push((&item.faces, flitch.base, footprint, strips, bench_cuts, excluded_blasts));
            }
        }
    }
    let count = flitches
        .into_par_iter()
        .map(|(faces, flitch, footprint, strips, bench_cuts, excluded_blasts)| {
            if cancel.is_cancelled() {
                return 0;
            }
            if excluded_blasts {
                cached_blast_faces(faces, flitch, footprint, bench_cuts);
            }
            cached_dig_block_faces(faces, flitch, footprint, strips, bench_cuts).len()
        })
        .sum();
    anyhow::ensure!(!cancel.is_cancelled(), crate::app::jobs::Cancelled);
    Ok(count)
}

impl crate::app::App<'_> {
    /// How many dig blocks the strips divide every flitch into, once the
    /// background count of the current ground and lines has landed; until
    /// then `None`, with the count started.
    pub(crate) fn dig_block_count(&mut self, solids: &[crate::model::Solid]) -> Option<usize> {
        let runtime = self.workspace.active_project().map(|project| project.runtime_id)?;
        let mut key = DefaultHasher::new();
        runtime.hash(&mut key);
        dig_count_key(solids, &self.solid_view_cache).hash(&mut key);
        let key = key.finish();
        if let Some((held, count)) = self.dig_block_count
            && held == key
        {
            return Some(count);
        }
        let job = crate::app::jobs::JobKey::DigBlockCount { key };
        if !self.job_pending(&job) {
            self.cancel_jobs(|pending| matches!(pending, crate::app::jobs::JobKey::DigBlockCount { .. }));
            let inputs: Vec<DigCountSolid> = solids
                .iter()
                .filter_map(|solid| {
                    let cache = self.solid_view_cache.get(&solid.id)?;
                    Some(DigCountSolid {
                        solid: solid.clone(),
                        faces: cache.face_cache(),
                        body: cache.body_product()?,
                        blasting: cache.blasting_product(),
                    })
                })
                .collect();
            self.spawn_job_quietly(
                crate::i18n::tr!("stage-counting-dig-blocks"),
                vec![job],
                move |cancel| count_dig_blocks(&inputs, cancel),
                move |app, result| {
                    if let Ok(count) = result {
                        app.dig_block_count = Some((key, count));
                    }
                },
            );
        }
        None
    }

    pub(crate) fn sync_dig_blocks(&mut self) {
        // Only the Dig Strips step draws and lists these. View shows the blocks
        // as cut-apart geometry instead, and Blasting comes before the strips
        // exist at all - a later step's subdivision must not appear over an
        // earlier step's benches.
        if !self.editor.is_dig_strips_step() {
            if !self.editor.dig_outlines.is_empty() {
                // Put aside rather than dropped: coming back to the step
                // unchanged shows them again without deriving anything.
                let outlines = std::mem::take(&mut self.editor.dig_outlines);
                self.editor.dig_outlines_parked = self.editor.dig_outlines_key.take().map(|key| (key, outlines));
                self.invalidate_overlay();
            }
            return;
        }
        let Some(document) = self.workspace.active_document() else { return };
        let mut hash = DefaultHasher::new();
        self.workspace.active_project().map(|project| project.runtime_id).hash(&mut hash);
        document.revision().hash(&mut hash);
        self.editor.selected_blast.hash(&mut hash);
        // Blast boundaries cut the strips, so a derivation that ran before the
        // blast outlines settled has to be redone once they change.
        self.editor.blasting_outlines_key.hash(&mut hash);
        for row in &self.editor.solids_view_selection {
            row.solid.hash(&mut hash);
            row.band.map(|b| (b.base.to_bits(), b.top.to_bits(), b.is_flitch)).hash(&mut hash);
        }
        for cache in self.solid_view_cache.values() {
            cache.key.hash(&mut hash);
            cache.built_through(crate::app::planning_pipeline::GeometryDemand::Body).hash(&mut hash);
        }
        let key = hash.finish();
        if self.editor.dig_outlines_key == Some(key) {
            return;
        }
        if let Some((parked, outlines)) = self.editor.dig_outlines_parked.take()
            && parked == key
        {
            self.editor.dig_outlines = outlines;
            self.editor.dig_outlines_key = Some(key);
            self.invalidate_overlay();
            return;
        }
        // Deriving a flitch's blocks finds a point inside each one, and a
        // survey's flitch ground runs to thousands of points a ring: the
        // flitches are derived side by side, then listed in order.
        struct Flitch<'a> {
            solid: &'a crate::model::Solid,
            faces: FlitchFaceCache,
            bench: f64,
            flitch: f64,
            top: f64,
            footprint: Option<&'a Vec<Vec<DVec2>>>,
            bench_cuts: &'a [Object],
            excluded_blasts: std::sync::Arc<Vec<Vec<Vec<DVec2>>>>,
        }
        let mut flitches = Vec::new();
        for solid in document.solids() {
            // The flitch ground, which Benching committed: the strips drawn on
            // it are this step's own input, not the blocks cut out of them.
            let Some((cache, footprints)) = self.solid_view_cache.get(&solid.id).and_then(|cache| Some((cache, cache.flitch_footprints()?))) else {
                continue;
            };
            for bench in solid.benching.benches() {
                let bench_cuts = solid.blasting.bench(bench.base).map(|entry| entry.cuts.as_slice()).unwrap_or_default();
                let excluded_blasts: std::sync::Arc<Vec<Vec<Vec<DVec2>>>> = std::sync::Arc::new(
                    self.editor
                        .blasting_outlines
                        .iter()
                        .filter(|outline| outline.solid == solid.id && (outline.bench_base - bench.base).abs() < 1e-6 && outline.excluded)
                        .map(|outline| outline.rings.iter().map(|ring| ring.iter().map(|p| p.truncate()).collect()).collect())
                        .collect(),
                );
                for flitch in &bench.flitches {
                    flitches.push(Flitch {
                        solid,
                        faces: cache.face_cache(),
                        bench: bench.base,
                        flitch: flitch.base,
                        top: flitch.top(),
                        footprint: footprints.get(&flitch.base.to_bits()),
                        bench_cuts,
                        excluded_blasts: excluded_blasts.clone(),
                    });
                }
            }
        }
        // Dig Strips works on one flitch at a time: its blocks are the only
        // ones worked out and listed, and with none picked there are none.
        let target = self.editor.planning_cut_target();
        let derived: Vec<Option<Arc<AnchoredFaces>>> = {
            use rayon::prelude::*;
            flitches
                .par_iter()
                .map(|item| {
                    // A flitch every piece of which lies in excluded blasts has
                    // nothing left to dig, so the tree leaves it out.
                    let gone = item.solid.exclusions.bench_excluded(item.bench)
                        || (!item.excluded_blasts.is_empty()
                            && item.footprint.is_some_and(|footprint| {
                                cached_blast_faces(&item.faces, item.flitch, footprint, item.bench_cuts)
                                    .iter()
                                    .all(|(_, anchor)| item.excluded_blasts.iter().any(|rings| arrangement::point_in_face(rings, *anchor)))
                            }));
                    if gone {
                        return None;
                    }
                    let faces = match item.footprint {
                        Some(footprint) if target.is_some_and(|(solid, picked)| solid == item.solid.id && (picked.base - item.flitch).abs() < 1e-6) => {
                            let drawing = item.solid.blasting.drawing(item.flitch, true);
                            cached_dig_block_faces(&item.faces, item.flitch, footprint, drawing.map_or(&[][..], |drawing| &drawing.cuts), item.bench_cuts)
                        }
                        _ => Arc::default(),
                    };
                    Some(faces)
                })
                .collect()
        };
        let mut outlines = Vec::new();
        let mut excluded_flitches = std::collections::HashSet::new();
        for (item, faces) in flitches.iter().zip(derived) {
            let Some(faces) = faces else {
                excluded_flitches.insert((item.solid.id, item.flitch.to_bits()));
                continue;
            };
            let solid = item.solid;
            // The bench's blasts, as Blasting named them, so each block
            // can be listed under the blast it lies in.
            let blasts: Vec<(&str, bool, Vec<Vec<DVec2>>)> = self
                .editor
                .blasting_outlines
                .iter()
                .filter(|outline| outline.solid == solid.id && (outline.bench_base - item.bench).abs() < 1e-6)
                .map(|outline| {
                    (
                        outline.name.as_str(),
                        outline.excluded,
                        outline.rings.iter().map(|ring| ring.iter().map(|p| p.truncate()).collect()).collect(),
                    )
                })
                .collect();
            let first = outlines.len();
            for (index, (face, anchor)) in faces.iter().enumerate() {
                let anchor = *anchor;
                let blast = blasts.iter().find(|(_, _, rings)| arrangement::point_in_face(rings, anchor));
                // Ground Blasting took out of mining is gone here, as
                // its solids are; only a block excluded on its own
                // still shows, so it can be put back.
                if solid.exclusions.bench_excluded(item.bench) || blast.is_some_and(|(_, excluded, _)| *excluded) {
                    continue;
                }
                let excluded = solid.exclusions.block_excluded(item.flitch, face);
                let blast = blast.map(|(name, _, _)| (*name).to_owned());
                let area = arrangement::face_area(face);
                outlines.push(BlastOutline {
                    solid: solid.id,
                    bench_base: item.flitch,
                    plane: item.top,
                    name: (index + 1).to_string(),
                    anchor: anchor.to_array(),
                    area,
                    blast,
                    excluded,
                    rings: face.iter().map(|ring| ring.iter().map(|p| DVec3::new(p.x, p.y, item.top)).collect()).collect(),
                });
            }
            // Grouped by blast, numbered blasts in number order, as the
            // panel nests them.
            let number = |name: Option<&str>| name.and_then(|name| name.parse::<u64>().ok()).unwrap_or(u64::MAX);
            outlines[first..].sort_by(|a, b| {
                number(a.blast.as_deref())
                    .cmp(&number(b.blast.as_deref()))
                    .then_with(|| a.blast.cmp(&b.blast))
                    .then_with(|| number(Some(&a.name)).cmp(&number(Some(&b.name))))
            });
        }
        self.editor.dig_outlines = outlines;
        // A flitch the tree no longer lists can't stay picked in it. The pick
        // moves to the nearest flitch of its solid the tree still lists - the
        // step opens on a bench's top flitch, which on a pit's top benches is
        // often wholly in blasts under the minimum size - rather than leaving
        // nothing picked and every flitch drawn at once.
        if let [row] = self.editor.solids_view_selection.as_slice()
            && let Some(band) = row.band.filter(|band| band.is_flitch && excluded_flitches.contains(&(row.solid, band.base.to_bits())))
        {
            let nearest = flitches
                .iter()
                .filter(|item| item.solid.id == row.solid && item.footprint.is_some() && !excluded_flitches.contains(&(item.solid.id, item.flitch.to_bits())))
                .min_by(|a, b| (a.flitch - band.base).abs().total_cmp(&(b.flitch - band.base).abs()).then(a.flitch.total_cmp(&b.flitch)))
                .map(|item| crate::ui::state::SolidsViewRow {
                    solid: item.solid.id,
                    band: Some(BenchSelection {
                        base: item.flitch,
                        top: item.top,
                        is_flitch: true,
                    }),
                });
            self.editor.solids_tree_reveal |= nearest.is_some();
            self.editor.solids_view_selection = nearest.into_iter().collect();
        }
        self.editor.dig_excluded_flitches = excluded_flitches;
        self.editor.dig_outlines_key = Some(key);
        self.invalidate_overlay();
    }
}
