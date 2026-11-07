//! Flitch-owned strip drawings and the dig blocks formed jointly with blasts.
use std::hash::{DefaultHasher, Hash, Hasher};

use glam::{DVec2, DVec3};

use crate::{
    model::{Object, arrangement},
    ui::state::{BenchSelection, BlastOutline},
};

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
pub(crate) fn dig_block_faces(footprint: &[Vec<DVec2>], strips: &[Object], bench_cuts: &[Object]) -> Vec<(arrangement::Face, DVec2)> {
    let mut faces: Vec<_> = arrangement::subdivide(footprint, &dig_block_cuts(strips, bench_cuts))
        .into_iter()
        .filter_map(|face| arrangement::representative_point(&face).map(|anchor| (face, anchor)))
        .collect();
    // Numbered by where they sit, so a block keeps its number while its
    // neighbours are redrawn.
    faces.sort_by(|a, b| b.1.y.total_cmp(&a.1.y).then_with(|| a.1.x.total_cmp(&b.1.x)));
    faces
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

/// How many dig blocks the strips divide every flitch into, worked out in
/// plan from the flitch ground Benching committed.
///
/// The whole of the Dig Strips step's own work: the same faces the step draws
/// and Reserving later cuts, counted without cutting anything, so a strip
/// edit settles at once.
pub(crate) fn count_dig_block_faces(solids: &[crate::model::Solid], caches: &std::collections::HashMap<crate::model::SolidId, super::solids_view::ViewSolid>) -> usize {
    use rayon::prelude::*;
    // A survey's flitch ground runs to thousands of points a ring, and finding
    // a point inside each face is most of the work: each flitch is counted
    // once for its ground and lines, and the flitches side by side.
    let mut flitches = Vec::new();
    for solid in solids {
        let Some(cache) = caches.get(&solid.id) else { continue };
        let (Some(footprints), Some(attempt)) = (cache.flitch_footprints(), cache.body_attempt()) else {
            continue;
        };
        for bench in solid.benching.benches() {
            let bench_cuts = solid.blasting.bench(bench.base).map(|entry| entry.cuts.as_slice()).unwrap_or_default();
            for flitch in &bench.flitches {
                let Some(footprint) = footprints.get(&flitch.base.to_bits()) else {
                    continue;
                };
                let strips = solid.blasting.drawing(flitch.base, true).map_or(&[][..], |drawing| &drawing.cuts);
                let mut key = DefaultHasher::new();
                attempt.hash(&mut key);
                flitch.base.to_bits().hash(&mut key);
                for (lines, cuts) in [(0u8, strips), (1u8, bench_cuts)] {
                    lines.hash(&mut key);
                    cuts.len().hash(&mut key);
                    cuts.iter().for_each(|cut| cut.geometry_hash().hash(&mut key));
                }
                flitches.push((cache, key.finish(), footprint, strips, bench_cuts));
            }
        }
    }
    flitches
        .into_par_iter()
        .map(|(cache, key, footprint, strips, bench_cuts)| cache.dig_face_count(key, || dig_block_faces(footprint, strips, bench_cuts).len()))
        .sum()
}

impl crate::app::App<'_> {
    pub(crate) fn sync_dig_blocks(&mut self) {
        // Only the Dig Strips step draws and lists these. View shows the blocks
        // as cut-apart geometry instead, and Blasting comes before the strips
        // exist at all - a later step's subdivision must not appear over an
        // earlier step's benches.
        if !self.editor.is_dig_strips_step() {
            if !self.editor.dig_outlines.is_empty() {
                self.editor.dig_outlines.clear();
                self.editor.dig_outlines_key = None;
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
        // Deriving a flitch's blocks finds a point inside each one, and a
        // survey's flitch ground runs to thousands of points a ring: the
        // flitches are derived side by side, then listed in order.
        struct Flitch<'a> {
            solid: &'a crate::model::Solid,
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
            let Some(footprints) = self.solid_view_cache.get(&solid.id).and_then(super::solids_view::ViewSolid::flitch_footprints) else {
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
        let selection = &self.editor.solids_view_selection;
        let derived: Vec<Option<Vec<(arrangement::Face, DVec2)>>> = {
            use rayon::prelude::*;
            flitches
                .par_iter()
                .map(|item| {
                    // A flitch every piece of which lies in excluded blasts has
                    // nothing left to dig, so the tree leaves it out.
                    let gone = item.solid.exclusions.bench_excluded(item.bench)
                        || (!item.excluded_blasts.is_empty()
                            && item.footprint.is_some_and(|footprint| {
                                arrangement::subdivide(footprint, &arrangement::cut_lines(item.bench_cuts))
                                    .iter()
                                    .filter_map(|face| arrangement::representative_point(face))
                                    .all(|anchor| item.excluded_blasts.iter().any(|rings| arrangement::point_in_face(rings, anchor)))
                            }));
                    if gone {
                        return None;
                    }
                    let band = BenchSelection {
                        base: item.flitch,
                        top: item.top,
                        is_flitch: true,
                    };
                    let faces = match item.footprint {
                        Some(footprint) if super::solids_view::selected(selection, item.solid.id, Some(band)) => {
                            let drawing = item.solid.blasting.drawing(item.flitch, true);
                            dig_block_faces(footprint, drawing.map_or(&[][..], |drawing| &drawing.cuts), item.bench_cuts)
                        }
                        _ => Vec::new(),
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
            for (index, (face, anchor)) in faces.into_iter().enumerate() {
                let blast = blasts.iter().find(|(_, _, rings)| arrangement::point_in_face(rings, anchor));
                // Ground Blasting took out of mining is gone here, as
                // its solids are; only a block excluded on its own
                // still shows, so it can be put back.
                if solid.exclusions.bench_excluded(item.bench) || blast.is_some_and(|(_, excluded, _)| *excluded) {
                    continue;
                }
                let excluded = solid.exclusions.block_excluded(item.flitch, &face);
                let blast = blast.map(|(name, _, _)| (*name).to_owned());
                let area = arrangement::face_area(&face);
                outlines.push(BlastOutline {
                    solid: solid.id,
                    bench_base: item.flitch,
                    plane: item.top,
                    name: (index + 1).to_string(),
                    anchor: anchor.to_array(),
                    area,
                    blast,
                    excluded,
                    rings: face.into_iter().map(|ring| ring.into_iter().map(|p| DVec3::new(p.x, p.y, item.top)).collect()).collect(),
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
        // A flitch the tree no longer lists can't stay picked in it.
        if let [row] = self.editor.solids_view_selection.as_slice()
            && row.band.is_some_and(|band| band.is_flitch && excluded_flitches.contains(&(row.solid, band.base.to_bits())))
        {
            self.editor.solids_view_selection.clear();
        }
        self.editor.dig_excluded_flitches = excluded_flitches;
        self.editor.dig_outlines_key = Some(key);
        self.invalidate_overlay();
    }
}
