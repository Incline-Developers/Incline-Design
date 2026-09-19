//! Schedule Animate's derived scene.
//!
//! The source dig-block meshes remain immutable.  Each source block holds one
//! derived cut at a time, and moving the time cursor re-solves only the blocks
//! whose depletion actually moved - at any instant that is the handful being
//! dug, not the whole pit.  The work runs on the bounded worker pool, and an
//! addressed result is accepted only while its project, calculation and
//! request still match.

use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::{Arc, OnceLock},
};

use rayon::prelude::*;

use crate::{
    app::{commands::solids::preview_triangulation, jobs::JobKey},
    model::{DigBlockId, SolidId, arrangement::Face, formats::mesh_data::Triangulation, schedule::animation::AnimationIndex, triangulation::OpenTriangulation},
    ui::state::{BenchSelection, BlastShapeRef, SolidsViewRow},
};

/// How finely a cut is placed. The depleted fraction is rounded to a multiple
/// of this before any geometry is asked for, so two cursor positions that
/// round together produce the identical mesh.
///
/// The drawn fraction is then within half a quantum - a tenth of a percent of
/// the block - of the fraction the schedule says, everywhere except beside the
/// two endpoints: a block barely started is drawn at one whole quantum dug
/// rather than rounded back to untouched, and one all but finished is drawn
/// with one quantum standing rather than rounded away to nothing. Those two
/// steps are worth up to a full quantum of error each, and they are
/// deliberate: whole and gone are the two states a viewer reads as facts about
/// the run, and neither may be reached by rounding. Add the inversion's
/// [`VOLUME_TOLERANCE`] for the total.
///
/// Rounded rather than compared against the cut already on screen: a
/// comparison that tolerates each step tolerates every step, and a slow drag
/// would then walk the cursor arbitrarily far from the geometry without ever
/// redrawing it.
const FRACTION_QUANTUM: f64 = 0.002;

/// How many cut positions a block's retained-volume profile is sampled at.
///
/// Each sample is one scan of the block's triangles, against the fourteen a
/// bisection of the whole block costs - so the profile has paid for itself by
/// the third cut, and a block the cursor crosses is cut at dozens of
/// fractions.
const PROFILE_SAMPLES: usize = 33;

/// How close in volume an inverted cut has to land, as a fraction of the whole
/// block. A tenth of the quantum the fraction is rounded to, so the inversion
/// is not what limits the accuracy.
const VOLUME_TOLERANCE: f64 = FRACTION_QUANTUM / 10.0;

/// How many probes the inversion may spend reaching [`VOLUME_TOLERANCE`]
/// before it reports that it could not.
///
/// Every second probe bisects, so the bracket is at worst halved every two of
/// them: twenty-four is a bracket four thousand times narrower than the span
/// between two profile samples, which is itself a thirty-second of the block.
/// Reaching the end of that budget means the retained volume is not behaving
/// like the volume of a solid, and a wrong cut is worse than a reported one.
const CUT_REFINEMENTS: usize = 24;

/// How close to untouched, or to emptied, a block has to be to count as
/// either.
///
/// Not one ulp. The depleted fraction is arithmetic over a dig that can run to
/// dozens of intervals, and it accumulates tens of ulps doing it; a block the
/// ledger has emptied has to read as emptied even so, or the last two tenths
/// of a percent of it stay on screen for the rest of the run. A billionth of a
/// block is far below anything that could be drawn, so nothing visible is
/// rounded away here.
const ENDPOINT_TOLERANCE: f64 = 1.0e-9;

/// Which cut a block is showing: whole, gone, or one of the quantised
/// fractions between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CutKey {
    Whole,
    Partial(u32),
    Gone,
}

impl CutKey {
    /// Full and empty are exact and are never rounded into a neighbouring
    /// fraction: the difference between a block that is nearly gone and one
    /// that is gone is the whole block.
    fn of(fraction: f64) -> Self {
        if fraction <= ENDPOINT_TOLERANCE {
            return Self::Whole;
        }
        if fraction >= 1.0 - ENDPOINT_TOLERANCE {
            return Self::Gone;
        }
        let last = (1.0 / FRACTION_QUANTUM) as u32;
        Self::Partial(((fraction / FRACTION_QUANTUM).round() as u32).clamp(1, last - 1))
    }

    /// The fraction to cut at, which is the key's own and not the cursor's:
    /// the geometry is then a function of the key alone, so the same key
    /// reached from either direction is the same mesh.
    fn fraction(self) -> f64 {
        match self {
            Self::Whole => 0.0,
            Self::Partial(steps) => f64::from(steps) * FRACTION_QUANTUM,
            Self::Gone => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity {
    runtime: u32,
    run: u64,
    solids_generation: u64,
}

struct SourceBlock {
    id: DigBlockId,
    solid: SolidId,
    bench: BenchSelection,
    flitch: BenchSelection,
    blast: Option<BlastShapeRef>,
    face: Arc<Face>,
    direction: glam::DVec2,
    volume: Option<f64>,
    /// How much of this block survives a cut at a given position, measured
    /// once by whichever worker first needs it and reused by every later cut
    /// of the same immutable geometry.
    profile: Arc<OnceLock<VolumeProfile>>,
    mesh: OpenTriangulation,
}

/// One source block's derived geometry, as the last solved cut left it.
#[derive(Default)]
struct Slot {
    /// The cut this block was last asked for, or `None` while it has never
    /// been asked for at all.  Asked for, not necessarily achieved: see
    /// `stale`.
    cut: Option<CutKey>,
    /// Set when the attempt at `cut` failed.  The mesh below is then an
    /// earlier cut of this block, and no later frame will replace it while the
    /// cursor stays where it is - the attempt is not repeated, or a block that
    /// cannot be cut would be retried sixty times a second for ever.
    ///
    /// So the slot is not evidence that its instant is drawn, and the view
    /// says as much for as long as one of these stands.
    stale: bool,
    /// What to draw, or `None` once the block has been dug out entirely.
    mesh: Option<OpenTriangulation>,
}

/// Everything a solved cut of one block depends on.
///
/// Two source blocks that agree here are the same block, cut the same way, in
/// the same colours - so a cut solved for one is a cut of the other, and a
/// rebuild of the source can keep it rather than starting the block whole
/// again. The mesh and the plan face are compared by address: both are
/// rebuilt wholesale by the Dig Strips stage and never edited in place, so a
/// shared pointer is a shared geometry.
#[derive(Clone, Copy, PartialEq)]
struct CutInputs {
    mesh: usize,
    face: usize,
    direction: glam::DVec2,
    volume: Option<f64>,
    color: [f32; 4],
    line_color: [f32; 4],
    line_weight: Option<f32>,
    flitch: Option<crate::model::FlitchStyle>,
}

impl CutInputs {
    fn of(block: &SourceBlock) -> Self {
        Self {
            mesh: Arc::as_ptr(&block.mesh.mesh) as usize,
            face: Arc::as_ptr(&block.face) as usize,
            direction: block.direction,
            volume: block.volume,
            color: block.mesh.color,
            line_color: block.mesh.line_color,
            line_weight: block.mesh.line_weight,
            flitch: block.mesh.flitch_style,
        }
    }
}

/// One block handed to a worker: which slot it fills, the cut to place, and
/// the immutable geometry to place it in.
struct AnimationWork {
    slot: usize,
    cut: CutKey,
    face: Arc<Face>,
    direction: glam::DVec2,
    volume: Option<f64>,
    profile: Arc<OnceLock<VolumeProfile>>,
    mesh: OpenTriangulation,
}

/// What came back for one slot.
enum Cut {
    /// Solved: the mesh to draw, or nothing once the block is entirely mined.
    Solved(Option<Box<OpenTriangulation>>),
    /// Could not be solved.  The slot keeps whatever it was showing, and is
    /// marked stale so that the view stops claiming to be the instant it
    /// names.  The key is stamped anyway, so a block whose geometry cannot be
    /// cut is reported once instead of being retried on every frame for ever.
    Failed(String),
}

#[derive(Clone, Default, PartialEq)]
struct Visibility {
    solids: HashSet<SolidId>,
    rows: Vec<SolidsViewRow>,
    blasts: HashSet<BlastShapeRef>,
}

impl Visibility {
    fn from_editor(editor: &crate::ui::state::EditorState) -> Self {
        Self {
            solids: editor.schedule_animation_hidden_solids.clone(),
            rows: editor.schedule_animation_hidden_rows.clone(),
            blasts: editor.schedule_animation_hidden_blasts.clone(),
        }
    }

    fn hides(&self, block: &SourceBlock) -> bool {
        self.solids.contains(&block.solid)
            || self.rows.contains(&SolidsViewRow {
                solid: block.solid,
                band: Some(block.bench),
            })
            || self.rows.contains(&SolidsViewRow {
                solid: block.solid,
                band: Some(block.flitch),
            })
            || block.blast.is_some_and(|blast| self.blasts.contains(&blast))
    }
}

/// Session-only state.  Nothing here is persisted or written back to reserves.
#[derive(Default)]
pub(crate) struct ScheduleAnimation {
    identity: Option<Identity>,
    /// The run the time cursor was last rewound for.
    ///
    /// Tracked apart from `identity` because it is maintained whether or not
    /// the Animate page is open: the Gantt sets the same cursor, so arriving
    /// here must not rewind what was set there. Only a new result does that.
    cursor_identity: Option<Identity>,
    index: Option<AnimationIndex>,
    source: Vec<SourceBlock>,
    /// Fingerprint of the calculated solids `source` was taken from, so it is
    /// re-collected when they move rather than on every frame.
    source_key: Option<u64>,
    /// One derived cut per source block, by the same index.
    slots: Vec<Slot>,
    /// The cursor position and visibility every block has already been solved
    /// for, so a view nobody is touching costs nothing: no depletion sampling,
    /// no work list, no scan of the pit.  Cleared whenever a batch lands,
    /// because what it landed may not be what is being asked for now.
    settled: Option<(f64, u64)>,
    /// Advanced whenever the hidden sets change.
    hidden_generation: u64,
    /// Whether the scene draws the solved cuts or the authored blocks whole.
    /// A run that is missing or stale shows the ground undug.
    depleted: bool,
    /// Advanced whenever the drawn body changes, which is what the assembled
    /// scene is keyed on.
    body_generation: u64,
    scene: Vec<OpenTriangulation>,
    /// What `scene` was last assembled from.  Assembling it clones every mesh
    /// in it, so it is rebuilt when its inputs move and not once a frame.
    scene_key: Option<u64>,
    request: u64,
    pending: bool,
    /// How many slots are holding a cut older than the one they were asked
    /// for, because the ask failed.  Counted rather than searched for, so
    /// reporting it costs nothing on a frame where nothing moved.
    stale_slots: usize,
    hidden: Visibility,
    error: Option<String>,
}

impl ScheduleAnimation {
    pub(crate) fn accepts(&self, runtime: u32, run: u64, request: u64) -> bool {
        self.identity.is_some_and(|identity| identity.runtime == runtime && identity.run == run) && self.request == request
    }

    pub(crate) fn scene(&self) -> &[OpenTriangulation] {
        &self.scene
    }

    /// The meshes the viewport should show: the solved cuts, or the authored
    /// blocks whole while there is no run to deplete them by.
    fn body(&self) -> impl Iterator<Item = &OpenTriangulation> {
        let cuts = self.depleted.then(|| self.slots.iter().filter_map(|slot| slot.mesh.as_ref())).into_iter().flatten();
        let whole = (!self.depleted).then(|| self.source.iter().map(|block| &block.mesh)).into_iter().flatten();
        cuts.chain(whole)
    }

    fn set_depleted(&mut self, depleted: bool) {
        if self.depleted != depleted {
            self.depleted = depleted;
            self.body_generation = self.body_generation.wrapping_add(1);
        }
    }
}

impl crate::app::App<'_> {
    /// Stop any animation geometry in flight and retire the address it was
    /// computed for.
    ///
    /// Cancelling alone is not enough to keep a superseded result off the
    /// screen: a worker that has already finished sends its result anyway, and
    /// the apply closure only checks that the project, run and request still
    /// match - which, after a bare cancel, they all still do. Advancing the
    /// serial is what makes that check fail, so nothing computed against
    /// ground the view has since stopped showing can land on top of what
    /// replaced it.
    fn abandon_animation_geometry(&mut self) {
        self.cancel_jobs(|key| matches!(key, JobKey::ScheduleAnimation { .. }));
        self.schedule_animation.request = self.schedule_animation.request.wrapping_add(1).max(1);
        self.schedule_animation.settled = None;
        self.schedule_animation.pending = false;
        self.editor.schedule_animation_pending = false;
    }

    /// Refresh the animation view without starting either planning pipeline.
    pub(crate) fn sync_schedule_animation(&mut self) {
        let animate = self.editor.is_schedule_animation();
        let runtime = self.workspace.active_project().map(|project| project.runtime_id);
        if self.schedule_animation.identity.is_some_and(|identity| Some(identity.runtime) != runtime) {
            self.abandon_animation_geometry();
            self.schedule_animation = ScheduleAnimation::default();
        }
        // Read from the held calculation rather than copying it. It owns the
        // execution log, the idle log and the ledger; cloning all three once a
        // frame to look at a run number is the largest allocation on this path
        // and it is made whether or not anything moved.
        let current = self.schedule_calculation_is_current();
        let identity = self.schedule_calculation.as_ref().zip(runtime).map(|(calculation, runtime)| Identity {
            runtime,
            run: calculation.run,
            solids_generation: calculation.schedule.generation,
        });

        // Ahead of the page gate, because the cursor is not this page's: the
        // Gantt's playhead is the same instant, and it is set there while this
        // page is closed. A new result rewinds it; opening the page does not.
        if current && self.schedule_animation.cursor_identity != identity {
            self.schedule_animation.cursor_identity = identity;
            self.editor.schedule_animation_time_h = 0.0;
        }

        if !animate {
            // Geometry for a page nobody is looking at, which would publish
            // into the scene and invalidate every cache in it when it landed.
            if self.schedule_animation.pending {
                self.abandon_animation_geometry();
            }
            return;
        }

        if current && self.schedule_animation.identity != identity {
            let cursor_identity = self.schedule_animation.cursor_identity;
            self.abandon_animation_geometry();
            self.schedule_animation = ScheduleAnimation::default();
            self.schedule_animation.identity = identity;
            self.schedule_animation.cursor_identity = cursor_identity;
            // Built before the result is stored, so the borrow of the held
            // calculation ends before the animation state is written.
            let index = self.schedule_calculation.as_ref().map(|calculation| AnimationIndex::build(&calculation.schedule));
            match index {
                Some(Ok(index)) => self.schedule_animation.index = Some(index),
                Some(Err(error)) => self.schedule_animation.error = Some(error.to_string()),
                None => {}
            }
        }

        // The source is immutable for as long as the calculated solids behind
        // it are, so it is collected when they move rather than on every
        // frame. Stale ground is rewalked the same way, so the disabled view
        // still shows current authored solids rather than an obsolete
        // snapshot, without rebuilding them sixty times a second to do it.
        let source_key = self.animation_source_key();
        if self.schedule_animation.source_key != Some(source_key) {
            // Cuts in flight are addressed by source index, and a fresh source
            // renumbers those - so work started against the old one would land
            // its geometry on whichever blocks now hold its slots.
            self.abandon_animation_geometry();
            self.rebuild_animation_source();
            self.schedule_animation.source_key = Some(source_key);
        }
        let solids_ready = self.workspace.active_document().is_some_and(|document| {
            document.solids().iter().all(|solid| {
                self.solid_view_cache
                    .get(&solid.id)
                    .and_then(super::commands::solids_view::ViewSolid::finished_partition)
                    .is_some()
            })
        });
        self.editor.schedule_animation_horizon_h = self
            .schedule_calculation
            .as_ref()
            .filter(|_| current)
            .map_or(0.0, |calculation| calculation.schedule.horizon_h.max(0.0));
        self.editor.schedule_animation_enabled = current && solids_ready && self.schedule_animation.index.is_some() && self.editor.schedule_animation_horizon_h > 0.0;

        // Ahead of the branch below, because the navigator's eyes work whether
        // or not there is a run to deplete by: a view showing authored solids
        // whole still has to hide the ones the user has hidden, and it is the
        // same hidden set that filters both.
        let visibility = Visibility::from_editor(&self.editor);
        if self.schedule_animation.hidden != visibility {
            self.schedule_animation.hidden = visibility;
            self.schedule_animation.hidden_generation = self.schedule_animation.hidden_generation.wrapping_add(1);
            self.schedule_animation.body_generation = self.schedule_animation.body_generation.wrapping_add(1);
        }

        if !current || !solids_ready || self.schedule_animation.index.is_none() {
            if self.schedule_animation.pending {
                self.abandon_animation_geometry();
            }
            self.schedule_animation.set_depleted(false);
            self.editor.schedule_animation_time_h = 0.0;
            self.editor.schedule_animation_shown_h = 0.0;
            self.editor.schedule_animation_degraded = false;
            // Why the slider is off and what turns it back on. What the
            // viewport is showing while it is off is on the screen already.
            self.editor.schedule_animation_status = if let Some(error) = &self.schedule_animation.error {
                error.clone()
            } else if !solids_ready {
                crate::i18n::tr!(literal = "Calculated solids are unavailable. Run Solids through Dig Strips.")
            } else if self.schedule_calculation.is_some() {
                crate::i18n::tr!(literal = "The schedule result is stale. Run Schedule again.")
            } else {
                crate::i18n::tr!(literal = "Run Schedule to enable time scrubbing.")
            };
            self.refresh_animation_scene();
            return;
        }

        let horizon = self.editor.schedule_animation_horizon_h;
        let time_h = self.editor.schedule_animation_time_h.clamp(0.0, horizon);
        self.editor.schedule_animation_time_h = time_h;
        self.schedule_animation.set_depleted(true);

        // One batch in flight at a time, and the cursor's latest position
        // picked up when it lands.
        //
        // Cancelling and respawning at every cursor position is what starves a
        // slow clip: each batch is abandoned before any of it finishes, so a
        // drag that never pauses draws nothing at all. An overtaken batch is
        // let finish and published instead. What it publishes is a whole
        // instant, not a mixture: every block it worked lands on the cut its
        // own time asked for, and every block it left alone was already
        // standing there - so the view is always some one moment of the run,
        // trailing the slider rather than blending two positions. The readout
        // names that moment rather than the slider's, dimmed while the two
        // differ, and the frame after a batch lands re-solves whatever the
        // cursor has since moved past.
        //
        // The exception is the batch nobody needs any more: a scrub out and
        // back leaves the cursor's own cut already on screen, and that result
        // is dropped where it lands rather than drawn and immediately undone.
        //
        // Once every block is solved for where the cursor is, the position is
        // recorded settled and the whole path costs nothing until something
        // moves: no depletion sampling and no walk of the pit per frame.
        let asking = (time_h, self.schedule_animation.hidden_generation);
        if !self.schedule_animation.pending && self.schedule_animation.settled != Some(asking) {
            let work = self.animation_work(time_h);
            if work.is_empty() {
                self.schedule_animation.settled = Some(asking);
                // Every visible block stands at the cut this instant asks for,
                // so this instant is what the viewport is showing.
                self.editor.schedule_animation_shown_h = time_h;
                self.editor.schedule_animation_pending = false;
            } else {
                self.request_animation_geometry(work, time_h);
            }
        }
        self.editor.schedule_animation_degraded = self.schedule_animation.stale_slots > 0;
        self.refresh_animation_scene();
    }

    /// What the derived source is taken from: the calculated solids, and the
    /// authored order that gives each block its successor.
    ///
    /// Pointer-and-count rather than content, because the partitions behind it
    /// are rebuilt wholesale by the Dig Strips stage and never edited in place.
    fn animation_source_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let Some(document) = self.workspace.active_document() else {
            return hasher.finish();
        };
        for solid in document.solids() {
            solid.id.0.hash(&mut hasher);
            let parts = self.solid_view_cache.get(&solid.id).and_then(super::commands::solids_view::ViewSolid::finished_partition);
            match parts {
                Some(parts) => {
                    parts.len().hash(&mut hasher);
                    for part in parts {
                        (Arc::as_ptr(&part.mesh.mesh) as usize).hash(&mut hasher);
                    }
                }
                None => 0usize.hash(&mut hasher),
            }
            solid.color.map(f32::to_bits).hash(&mut hasher);
        }
        document.revision().hash(&mut hasher);
        self.schedule_calculation.as_ref().map(|calculation| calculation.run).hash(&mut hasher);
        hasher.finish()
    }

    /// Re-collect the derived source from the calculated solids.
    ///
    /// Whatever is still the same block, cut the same way, keeps the cut it
    /// had and the volume profile that placed it. The source is re-collected
    /// whenever anything in the document moves - a string imported, a name
    /// typed - and starting every block whole again for that would drop the
    /// whole pit back to undug and re-cut it, which is a flash of untouched
    /// ground across the viewport for as long as the workers take.
    fn rebuild_animation_source(&mut self) {
        let mut previous: HashMap<DigBlockId, (CutInputs, Arc<OnceLock<VolumeProfile>>, Slot)> = std::mem::take(&mut self.schedule_animation.source)
            .into_iter()
            .zip(std::mem::take(&mut self.schedule_animation.slots))
            .map(|(block, slot)| (block.id, (CutInputs::of(&block), block.profile, slot)))
            .collect();
        let mut source = Vec::new();
        let Some(document) = self.workspace.active_document() else {
            // Both are already empty, taken above; what is left is to say the
            // drawn body has changed, because it has - to nothing.
            self.schedule_animation.stale_slots = 0;
            self.schedule_animation.body_generation = self.schedule_animation.body_generation.wrapping_add(1);
            return;
        };
        for solid in document.solids() {
            let Some(parts) = self.solid_view_cache.get(&solid.id).and_then(super::commands::solids_view::ViewSolid::finished_partition) else {
                continue;
            };
            for part in parts {
                let Some(block) = &part.block else { continue };
                let mut mesh = part.mesh.clone();
                mesh.flitch_style = solid
                    .benching
                    .intervals
                    .get(part.band.interval)
                    .map(|interval| interval.style(part.band.position, solid.color));
                mesh.color = mesh.flitch_style.map_or(solid.color, |style| style.color);
                source.push(SourceBlock {
                    id: block.id,
                    solid: solid.id,
                    bench: part.bench,
                    flitch: part.band.selection,
                    blast: part.blast,
                    face: block.face.clone(),
                    direction: glam::DVec2::Y,
                    volume: part.volume,
                    profile: Arc::default(),
                    mesh,
                });
            }
        }
        if let Some(calculation) = &self.schedule_calculation {
            let centers: HashMap<_, _> = source
                .iter()
                .filter_map(|block| crate::model::arrangement::representative_point(&block.face).map(|center| (block.id, center)))
                .collect();
            // One lookup per authored member rather than a walk of every
            // balance in the run for each of them.
            let resolved: HashMap<_, _> = calculation
                .schedule
                .balances
                .iter()
                .map(|balance| (balance.block.ground_identity().key(), balance.resolved))
                .collect();
            // Which block each one is dug toward. `None` marks ground the bars
            // disagree about: a block two of them name has no one successor,
            // and the answer must not be whichever bar the document happens to
            // list first. Those keep the +Y fallback, which is what the
            // fallback is for.
            let mut successors: HashMap<DigBlockId, Option<DigBlockId>> = HashMap::new();
            for bar in document.schedule().bars() {
                let mut ordered: Vec<_> = bar.members().iter().filter_map(|member| resolved.get(&member.ground_identity().key()).copied()).collect();
                ordered.dedup();
                for pair in ordered.windows(2) {
                    successors
                        .entry(pair[0])
                        .and_modify(|held| {
                            if *held != Some(pair[1]) {
                                *held = None;
                            }
                        })
                        .or_insert(Some(pair[1]));
                }
            }
            for block in &mut source {
                let Some(next) = successors.get(&block.id).copied().flatten() else { continue };
                let Some((here, there)) = centers.get(&block.id).zip(centers.get(&next)) else {
                    continue;
                };
                let toward_next = *there - *here;
                if toward_next.length_squared() > 1e-12 {
                    block.direction = toward_next.normalize();
                }
            }
        }
        // Slots are addressed by source index, so a fresh source renumbers
        // them: they are matched back to their blocks here, by everything the
        // cut was solved from, and a block that has moved in any of it starts
        // over. Starting over means holding the block whole, and saying so:
        // that is what the ground looks like before anything has been dug, so
        // entering the page at the origin has nothing to solve rather than
        // sending the entire pit through a worker to be handed back the meshes
        // it went in with.
        let mut stale_slots = 0;
        let mut slots = Vec::with_capacity(source.len());
        for block in &mut source {
            let inputs = CutInputs::of(block);
            match previous.remove(&block.id) {
                Some((held, profile, slot)) if held == inputs => {
                    // The profile measures this same immutable geometry along
                    // this same direction, so it survives with the cut.
                    block.profile = profile;
                    stale_slots += usize::from(slot.stale);
                    slots.push(slot);
                }
                _ => slots.push(Slot {
                    cut: Some(CutKey::Whole),
                    stale: false,
                    mesh: Some(block.mesh.clone()),
                }),
            }
        }
        self.schedule_animation.slots = slots;
        self.schedule_animation.stale_slots = stale_slots;
        self.schedule_animation.source = source;
        self.schedule_animation.body_generation = self.schedule_animation.body_generation.wrapping_add(1);
    }

    /// Whether every visible block already stands at the cut `time_h` asks
    /// for.
    ///
    /// The same judgement [`Self::animation_work`] makes, without building the
    /// work to make it: this one is asked on the frame a batch lands, to find
    /// out whether the batch is still wanted, and building a work list to
    /// answer that would clone every mesh in it.
    fn animation_is_current(&self, time_h: f64) -> bool {
        let Some(index) = self.schedule_animation.index.as_ref() else { return true };
        let hidden = &self.schedule_animation.hidden;
        self.schedule_animation
            .source
            .iter()
            .enumerate()
            .filter(|(_, block)| !hidden.hides(block))
            .all(|(slot, block)| self.schedule_animation.slots.get(slot).and_then(|slot| slot.cut) == Some(CutKey::of(index.depleted_fraction(block.id, time_h))))
    }

    /// The blocks whose cut no longer stands at `time_h`.
    fn animation_work(&self, time_h: f64) -> Vec<AnimationWork> {
        let Some(index) = self.schedule_animation.index.as_ref() else { return Vec::new() };
        let hidden = &self.schedule_animation.hidden;
        self.schedule_animation
            .source
            .iter()
            .enumerate()
            .filter(|(_, block)| !hidden.hides(block))
            .filter_map(|(slot, block)| {
                let cut = CutKey::of(index.depleted_fraction(block.id, time_h));
                let standing = self.schedule_animation.slots.get(slot).and_then(|slot| slot.cut) == Some(cut);
                (!standing).then(|| AnimationWork {
                    slot,
                    cut,
                    face: block.face.clone(),
                    direction: block.direction,
                    volume: block.volume,
                    profile: block.profile.clone(),
                    mesh: block.mesh.clone(),
                })
            })
            .collect()
    }

    fn request_animation_geometry(&mut self, work: Vec<AnimationWork>, time_h: f64) {
        let Some(identity) = self.schedule_animation.identity else { return };
        // A fresh address for this batch, so a result that outlives the ground
        // it was cut from - a source rebuild, a new run, the page being left -
        // fails the check when it lands instead of publishing over whatever
        // replaced it.
        self.schedule_animation.request = self.schedule_animation.request.wrapping_add(1).max(1);
        let request = self.schedule_animation.request;
        self.schedule_animation.pending = true;
        self.schedule_animation.settled = None;
        // Dimming the clock is the whole progress report. A scrub fires one of
        // these per cursor position, so a readout in the toolbar and a busy
        // pointer would blink several times a second while the slider moved,
        // and the status bar holds the frame open for as long as it is showing
        // something - a full scene redraw per frame for a task nobody is
        // waiting on.
        self.editor.schedule_animation_pending = true;

        self.spawn_job_quietly(
            crate::i18n::tr!(literal = "Updating schedule animation"),
            vec![JobKey::ScheduleAnimation {
                runtime: identity.runtime,
                run: identity.run,
                request,
            }],
            move |cancel| build_animation_cuts(work, cancel),
            move |app, result| {
                if !app.schedule_animation.accepts(identity.runtime, identity.run, request) || !app.schedule_calculation_is_current() {
                    return;
                }
                app.schedule_animation.pending = false;
                // Whether this is the cut the cursor is now asking for is the
                // next frame's judgement, not this one's - and it is the frame
                // that clears the dimmed clock, once it finds nothing left to
                // solve. `invalidate_geometry` below asks for that frame.
                app.schedule_animation.settled = None;
                // The only way out of `build_animation_cuts` is cancellation:
                // a block that will not cut comes back as `Cut::Failed`
                // alongside the ones that did. A cancelled result has nothing
                // to say and nothing to apply.
                let Ok(cuts) = result else { return };
                // Scrubbed away and back while this was in flight: the cursor
                // is asking for the cut already on screen, so this batch would
                // draw an instant nobody wants and be undone on the next
                // frame - two scene rebuilds and two rounds of upload to end
                // where the view already was. Dropped instead, and the frame
                // asked for below finds the position settled.
                if app.animation_is_current(app.editor.schedule_animation_time_h) {
                    app.redraw_requested = true;
                    return;
                }
                let mut newly_stale: isize = 0;
                for (slot, key, cut) in cuts {
                    let Some(held) = app.schedule_animation.slots.get_mut(slot) else { continue };
                    // Stamped whether or not the cut was solved, so a block
                    // that cannot be cut is not re-attempted on every frame
                    // until the cursor moves off it.
                    held.cut = Some(key);
                    let failed = matches!(cut, Cut::Failed(_));
                    let turned = held.stale != failed;
                    if turned {
                        held.stale = failed;
                        newly_stale += if failed { 1 } else { -1 };
                    }
                    match cut {
                        Cut::Solved(mesh) => held.mesh = mesh.map(|mesh| *mesh),
                        // Reported to the activity console rather than to the
                        // timeline's status line, which is only drawn while
                        // the page is disabled - and this is a failure that
                        // happens with everything working. Said once as the
                        // block goes stale rather than at every cut it then
                        // fails, which during a drag is all of them.
                        Cut::Failed(error) => {
                            if turned {
                                crate::userspace_warn!("{}", error);
                            }
                        }
                    }
                }
                app.schedule_animation.stale_slots = app.schedule_animation.stale_slots.saturating_add_signed(newly_stale);
                // What the viewport now shows is this batch's instant, whole:
                // the blocks it cut are at this time's fractions and the ones
                // it skipped were already there.
                app.editor.schedule_animation_shown_h = time_h;
                app.schedule_animation.body_generation = app.schedule_animation.body_generation.wrapping_add(1);
                app.refresh_animation_scene();
                app.invalidate_geometry();
            },
        );
    }

    /// What the assembled scene is made of, so that assembling it - which
    /// clones every mesh in it, project surfaces included - happens when one
    /// of those things moves rather than once a frame.
    fn animation_scene_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for mesh in &self.triangulations {
            mesh.id.0.hash(&mut hasher);
            // Every edit and every style change advances the item's revision,
            // which is what the drawn copy has to follow.
            mesh.state.revision().hash(&mut hasher);
            (Arc::as_ptr(&mesh.mesh) as usize).hash(&mut hasher);
        }
        // Anything the workspace can change about what is open to be drawn.
        self.workspace.composite_key().hash(&mut hasher);
        self.schedule_animation.body_generation.hash(&mut hasher);
        hasher.finish()
    }

    fn refresh_animation_scene(&mut self) {
        let key = self.animation_scene_key();
        if self.schedule_animation.scene_key == Some(key) {
            return;
        }
        let hidden = &self.schedule_animation.hidden;
        let drawn: HashSet<_> = self
            .schedule_animation
            .source
            .iter()
            .filter(|block| !hidden.hides(block))
            .map(|block| block.mesh.id)
            .collect();
        // Every open surface, on the project's own visibility.
        //
        // The solids' source sheets were once dropped here, on the grounds
        // that the ground they describe is what the blocks are drawn from.
        // That is a second visibility rule with no switch on it: a topography
        // or surface opened while this page is up would never appear, however
        // its eye was set, and nothing said why. Suppressing a source sheet is
        // the solid inspector's business, where it is scoped to the one solid
        // being inspected and the viewport is its own; this page paints into
        // the main viewport, where what is shown is the project's answer.
        self.schedule_animation.scene = self
            .triangulations
            .iter()
            .cloned()
            .chain(self.schedule_animation.body().filter(|mesh| drawn.contains(&mesh.id)).cloned())
            .collect();
        self.schedule_animation.scene_key = Some(key);
    }

    /// Frame one authored solid from its immutable full block geometry. The
    /// time cursor can remove most of it, but navigation continues to mean the
    /// solid itself rather than whichever small remnant happens to remain.
    pub(crate) fn focus_schedule_animation_solid(&mut self, solid: SolidId) {
        if !self.editor.is_schedule_animation() {
            return;
        }
        let mut bounds: Option<(glam::DVec3, glam::DVec3)> = None;
        for block in self.schedule_animation.source.iter().filter(|block| block.solid == solid) {
            let box_ = block.mesh.mesh.bounds();
            let low = glam::DVec3::new(box_.min.x, box_.min.y, box_.min.z);
            let high = glam::DVec3::new(box_.max.x, box_.max.y, box_.max.z);
            bounds = Some(bounds.map_or((low, high), |(min, max)| (min.min(low), max.max(high))));
        }
        let Some((min, max)) = bounds else { return };
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.frame_bounds(
                min,
                max,
                &self.scene_document,
                &self.schedule_animation.scene,
                &self.block_models,
                &self.drill_holes,
                &self.point_clouds,
                &self.editor.hidden_handles,
            );
            self.redraw_requested = true;
        }
    }
}

/// Solve every block in the work list, in parallel: they share nothing but
/// the cancel flag, and one heavy clip no longer holds up the others.
fn build_animation_cuts(work: Vec<AnimationWork>, cancel: &crate::app::jobs::CancelFlag) -> anyhow::Result<Vec<(usize, CutKey, Cut)>> {
    work.into_par_iter()
        .map(|work| {
            anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
            let unmeasured = work.volume.is_none() || work.volume == Some(0.0);
            let cut = if unmeasured || work.cut == CutKey::Whole {
                Cut::Solved(Some(Box::new(work.mesh)))
            } else if work.cut == CutKey::Gone {
                Cut::Solved(None)
            } else {
                match clip_in_direction(&work.mesh, &work.face, work.direction, &work.profile, work.cut.fraction(), cancel) {
                    Ok(clipped) => Cut::Solved(clipped.map(Box::new)),
                    // A cancellation is the whole job going away, not this one
                    // block failing to cut.
                    Err(error) if cancel.is_cancelled() => return Err(error),
                    Err(error) => Cut::Failed(format!("{error:#}")),
                }
            };
            Ok((work.slot, work.cut, cut))
        })
        .collect()
}

/// Retain the high-projection side. The low side is mined first, so the
/// excavation front advances in `direction` toward the next authored block.
fn clip_in_direction(
    source: &OpenTriangulation,
    face: &Face,
    direction: glam::DVec2,
    profile: &OnceLock<VolumeProfile>,
    removed_fraction: f64,
    cancel: &crate::app::jobs::CancelFlag,
) -> anyhow::Result<Option<OpenTriangulation>> {
    let (low, high) = face
        .iter()
        .flatten()
        .map(|point| point.dot(direction))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), projection| (low.min(projection), high.max(projection)));
    if !low.is_finite() || high <= low {
        return Ok(Some(source.clone()));
    }
    let profile = match profile.get() {
        Some(profile) => profile,
        None => {
            let built = VolumeProfile::build(&source.mesh, direction, low, high, cancel)?;
            // Two workers reach this together only if one block is in two
            // batches at once, which one batch in flight rules out. Either
            // measurement would do in any case: the geometry is immutable and
            // the direction is the block's own.
            let _ = profile.set(built);
            profile.get().expect("the profile was just set")
        }
    };
    if profile.whole() <= f64::EPSILON {
        return Ok(Some(source.clone()));
    }
    let cut = profile.cut_retaining(&source.mesh, direction, profile.whole() * (1.0 - removed_fraction), cancel)?;
    let (vertices, faces, _, boundary_wall) = clip_at_cut(&source.mesh, face, direction, cut, cancel)?;
    if faces.is_empty() {
        return Ok(None);
    }
    // Every retained component is independently capped.  The clip records
    // its boundary walls, so only their rim is drawn: no ghost edge survives
    // on the removed side and no internal triangulation clutters the surface.
    let slab = (vertices, faces);
    let edges = super::commands::triangulation::solid_between::boundary_wall_outline(&slab, &boundary_wall);
    let (vertices, faces) = slab;
    let mesh = Arc::new(Triangulation::from_vertices_and_faces(vertices, faces)?);
    let spatial = Arc::new(crate::model::spatial::TriangleBvh::build(&mesh));
    let order = Arc::new(crate::model::triangulation::morton_surface_face_order(&mesh));
    let mut result = preview_triangulation(source.name.clone(), mesh, spatial, edges, order, source.color, source.line_color);
    result.id = source.id;
    result.flitch_style = source.flitch_style;
    result.cull_back_faces = true;
    result.always_show_edges = true;
    result.line_weight = source.line_weight.or(Some(1.5));
    Ok(Some(result))
}

/// How much of one immutable block survives a cut, against where the cut is
/// placed - measured once, inverted as often as the cursor asks.
///
/// The face advances by moving a cut plane, and finding where to put it for a
/// given depletion means inverting this. Bisecting the block from scratch
/// costs fourteen full scans of its triangles for every cut, on geometry that
/// never changes; sampling the curve once and searching inside one span of it
/// costs two or three, and holds for every later cut of the same block.
///
/// Retained volume falls monotonically as the cut advances: what is retained
/// past a further cut is a subset of what is retained past a nearer one.
struct VolumeProfile {
    /// The cut behind the whole block, and the cut past all of it.
    low: f64,
    high: f64,
    /// Retained volume at [`PROFILE_SAMPLES`] evenly spaced cuts from `low` to
    /// `high`, descending from the whole block to nothing.
    samples: Vec<f64>,
}

impl VolumeProfile {
    fn build(mesh: &Triangulation, direction: glam::DVec2, low: f64, high: f64, cancel: &crate::app::jobs::CancelFlag) -> anyhow::Result<Self> {
        let span = (PROFILE_SAMPLES - 1) as f64;
        let mut samples = (0..PROFILE_SAMPLES)
            .map(|step| retained_volume_at_cut(mesh, direction, low + (high - low) * step as f64 / span, cancel))
            .collect::<anyhow::Result<Vec<_>>>()?;
        for sample in &samples {
            anyhow::ensure!(
                sample.is_finite() && *sample >= 0.0,
                "{}",
                crate::i18n::tr!(literal = "A block's volume could not be measured.")
            );
        }
        // The search below brackets by descent, so the samples have to descend.
        // Summing thousands of triangles at two nearby cuts can put a few ulps
        // the wrong way round, and that much is flattened; anything larger is
        // not rounding, it is geometry that does not behave like a solid, and
        // inverting it would place the cut confidently in the wrong place.
        let slack = VOLUME_TOLERANCE * samples[0];
        for index in 1..samples.len() {
            anyhow::ensure!(
                samples[index] <= samples[index - 1] + slack,
                "{}",
                crate::i18n::tr!(literal = "A block retains more material the further it is cut back.")
            );
            samples[index] = samples[index].min(samples[index - 1]);
        }
        Ok(Self { low, high, samples })
    }

    /// The whole block, which is what is retained by a cut behind all of it.
    fn whole(&self) -> f64 {
        self.samples.first().copied().unwrap_or(0.0)
    }

    /// Where to put the cut that leaves `target` volume standing, within
    /// [`VOLUME_TOLERANCE`] of the whole block - or a report that it could not
    /// be found.
    ///
    /// The samples bracket the answer; the search then refines inside that one
    /// span by interpolation, which on a curve this smooth lands within
    /// tolerance in a couple of probes. Interpolation alone is not enough to
    /// promise anything, though: on a lopsided curve it can creep toward one
    /// end and leave the other where it started, so the bracket never closes.
    /// Every second probe therefore bisects, which halves the bracket whatever
    /// the curve is doing.
    ///
    /// What is returned is measured, never assumed. Either a probe landed
    /// within tolerance of the target, or the bracket's two measured ends came
    /// within tolerance of each other - and since retained volume descends
    /// with the cut, everything between two such ends, the target included, is
    /// within tolerance of everything else between them. A cut that meets
    /// neither test is an error rather than a midpoint offered as though it
    /// did.
    fn cut_retaining(&self, mesh: &Triangulation, direction: glam::DVec2, target: f64, cancel: &crate::app::jobs::CancelFlag) -> anyhow::Result<f64> {
        let step = (self.high - self.low) / (self.samples.len() - 1) as f64;
        let index = self.samples.partition_point(|retained| *retained > target).clamp(1, self.samples.len() - 1);
        let (mut near, mut far) = (self.low + step * (index - 1) as f64, self.low + step * index as f64);
        let (mut near_volume, mut far_volume) = (self.samples[index - 1], self.samples[index]);
        let tolerance = VOLUME_TOLERANCE * self.whole();
        // The bracket's own ends are already measured, and a cut asked for at
        // one of them - which is where a round fraction of an evenly sampled
        // block lands - is answered without probing anything.
        if (near_volume - target).abs() <= tolerance {
            return Ok(near);
        }
        if (far_volume - target).abs() <= tolerance {
            return Ok(far);
        }
        for probe in 0..CUT_REFINEMENTS {
            anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
            if near_volume - far_volume <= tolerance {
                return Ok((near + far) * 0.5);
            }
            let mut cut = if probe % 2 == 1 {
                (near + far) * 0.5
            } else {
                near + (far - near) * (target - near_volume) / (far_volume - near_volume)
            };
            if !(cut > near && cut < far) {
                cut = (near + far) * 0.5;
            }
            // Adjacent floats, with nothing left between them to probe. No
            // further work can narrow this, so the loop stops here and the
            // check below decides whether what it has is good enough.
            if !(cut > near && cut < far) {
                break;
            }
            let retained = retained_volume_at_cut(mesh, direction, cut, cancel)?;
            if (retained - target).abs() <= tolerance {
                return Ok(cut);
            }
            if retained > target {
                (near, near_volume) = (cut, retained);
            } else {
                (far, far_volume) = (cut, retained);
            }
        }
        anyhow::ensure!(
            near_volume - far_volume <= tolerance,
            "{}",
            crate::i18n::tr!(literal = "A block's shape could not be cut to the depletion the schedule reports.")
        );
        Ok((near + far) * 0.5)
    }
}

/// Volume of the portion at `dot(xy, direction) >= cut`, without constructing
/// its cap.
///
/// The source is the consistently wound cell soup emitted by the solids
/// clipper. Vertical faces project to zero; clipping and integrating the
/// remaining floor/roof triangles therefore measures the same vertical
/// prism as `clip_solid_to_plan`, while avoiding all topology construction.
fn retained_volume_at_cut(mesh: &Triangulation, direction: glam::DVec2, cut: f64, cancel: &crate::app::jobs::CancelFlag) -> anyhow::Result<f64> {
    let bounds = mesh.bounds();
    let origin_x = bounds.min.x;
    let origin_y = bounds.min.y;
    let floor = bounds.min.z;
    let mut signed = 0.0;
    for (face_index, indices) in mesh.face_vertex_indices_iter().enumerate() {
        if face_index.is_multiple_of(2048) {
            anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
        }
        let triangle = indices.map(|index| mesh.vertices()[index]);
        let (polygon, count) = clip_triangle_at_cut(triangle, direction, cut);
        for index in 1..count.saturating_sub(1) {
            let [a, b, c] = [polygon[0], polygon[index], polygon[index + 1]].map(|vertex| [vertex.x - origin_x, vertex.y - origin_y, vertex.z - floor]);
            let projected = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            signed += projected * (a[2] + b[2] + c[2]) / 6.0;
        }
    }
    Ok(signed.abs())
}

fn clip_triangle_at_cut(triangle: [crate::model::formats::mesh_data::Vertex; 3], direction: glam::DVec2, cut: f64) -> ([crate::model::formats::mesh_data::Vertex; 4], usize) {
    use crate::model::formats::mesh_data::Vertex;

    let mut output = [Vertex::new(0.0, 0.0, 0.0); 4];
    let mut count = 0;
    let mut previous = triangle[2];
    let mut previous_projection = glam::DVec2::new(previous.x, previous.y).dot(direction);
    let mut previous_inside = previous_projection >= cut;
    for current in triangle {
        let current_projection = glam::DVec2::new(current.x, current.y).dot(direction);
        let current_inside = current_projection >= cut;
        if current_inside != previous_inside {
            let amount = ((cut - previous_projection) / (current_projection - previous_projection)).clamp(0.0, 1.0);
            output[count] = Vertex::new(
                previous.x + (current.x - previous.x) * amount,
                previous.y + (current.y - previous.y) * amount,
                previous.z + (current.z - previous.z) * amount,
            );
            count += 1;
        }
        if current_inside {
            output[count] = current;
            count += 1;
        }
        previous = current;
        previous_projection = current_projection;
        previous_inside = current_inside;
    }
    (output, count)
}

type ClippedAtY = (Vec<crate::model::formats::mesh_data::Vertex>, Vec<[u32; 3]>, f64, Vec<bool>);

fn clip_at_cut(mesh: &Triangulation, face: &Face, direction: glam::DVec2, cut: f64, cancel: &crate::app::jobs::CancelFlag) -> anyhow::Result<ClippedAtY> {
    let tangent = glam::DVec2::new(-direction.y, direction.x);
    let (min_tangent, max_tangent) = face
        .iter()
        .flatten()
        .map(|point| point.dot(tangent))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), projection| (low.min(projection), high.max(projection)));
    let pad = (max_tangent - min_tangent).abs().max(1.0);
    let cut_line = vec![tangent * (min_tangent - pad) + direction * cut, tangent * (max_tangent + pad) + direction * cut];
    let retained = crate::model::arrangement::subdivide(face, &[cut_line])
        .into_iter()
        .filter(|piece| crate::model::arrangement::representative_point(piece).is_some_and(|point| point.dot(direction) >= cut))
        .collect::<Vec<_>>();
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    let mut volume = 0.0;
    let mut boundary_wall = Vec::new();
    for piece in retained {
        anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
        let clipped = super::commands::triangulation::solid_between::clip_solid_to_plan(mesh, &piece, cancel)?;
        let offset = vertices.len() as u32;
        vertices.extend(clipped.slab.0);
        faces.extend(clipped.slab.1.into_iter().map(|face| face.map(|index| index + offset)));
        volume += clipped.volume;
        boundary_wall.extend(clipped.boundary_wall);
    }
    Ok((vertices, faces, volume, boundary_wall))
}
