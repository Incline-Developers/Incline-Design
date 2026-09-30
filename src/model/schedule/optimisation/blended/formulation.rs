//! The blended model itself: one builder, written once, used by both
//! experimental solvers.
//!
//! Reading this file is reading the model. The SCIP backend and the iterative
//! HiGHS backend each supply a [`Rows`] implementation and then call
//! [`formulate`]; nothing else about the constraints differs between them.
//! That is what makes the §7 comparison a comparison of *methods* rather than
//! of two independently drifting formulations.

use std::collections::{BTreeMap, BTreeSet};

use super::input::{
    BlendInput, BlendPile, GRADE_CUSHION_T, GRADE_MARGIN, GradeBound, GradeEndpoint, GradeHalfSpace, GradePredicate, GradeQualification, authored_tasks, delivers_to_pile,
    flat_cell, interval_rate, loader_rate, task_active, task_authorises, task_operable,
};
use crate::model::schedule::optimisation::{
    Activity, Destination, DestinationId, DestinationKind, GroundId, Interval, LoaderId, MovementCandidate, ReclaimOrder, SourceId, StockpileId, TaskKind,
};

/// Column handles, kept so the solution can be read back by meaning rather
/// than by index.
pub(crate) struct BlendColumns<V> {
    /// Duration of (interval, segment).
    pub(crate) duration: BTreeMap<(usize, usize), V>,
    /// Tonnes moved by (movement candidate, interval, segment).
    pub(crate) movement: BTreeMap<(usize, usize, usize), V>,
    /// Released opening tonnes of (pile, interval).
    pub(crate) open_t: BTreeMap<(StockpileId, usize), V>,
    /// Released opening contained quantity of (pile, interval, grade).
    pub(crate) open_q: BTreeMap<(StockpileId, usize, usize), V>,
    /// Tonnes reclaimed from (pile, interval).
    pub(crate) recl_t: BTreeMap<(StockpileId, usize), V>,
    /// Contained quantity reclaimed from (pile, interval, grade).
    pub(crate) recl_q: BTreeMap<(StockpileId, usize, usize), V>,
    /// Remaining tonnes of (ground source, interval).
    pub(crate) ground_remaining: BTreeMap<(usize, usize), V>,
    /// Whether (loader, task, interval, segment) is the *selected*
    /// assignment - the bar the loader is actually working in that segment.
    pub(crate) active: BTreeMap<(usize, usize, usize, usize), V>,
    /// Whether (loader, task, interval, segment) *has work available*. The
    /// distinction between this and [`Self::active`] is what makes authored
    /// bar priority mandatory rather than advisory: a ready higher-priority
    /// bar forbids selecting a lower-priority one, and a ready bar with no
    /// ready predecessor must itself be selected.
    pub(crate) ready: BTreeMap<(usize, usize, usize, usize), V>,
    /// Whether ground source `index` is exhausted at the end of flat cell
    /// `cell`. Drives authored dig-block order.
    pub(crate) exhausted: BTreeMap<(usize, usize), V>,
    /// Chunked piles only, keyed (pile, chunk, interval). Published so the
    /// replay can reconstruct chunk eligibility independently instead of
    /// taking the solver's word for it.
    pub(crate) chunk_open_t: BTreeMap<(StockpileId, usize, usize), V>,
    pub(crate) chunk_recl_t: BTreeMap<(StockpileId, usize, usize), V>,
    pub(crate) chunk_closed: BTreeMap<(StockpileId, usize, usize), V>,
    /// Keyed (pile, chunk, interval, movement).
    pub(crate) chunk_recv: BTreeMap<(StockpileId, usize, usize, usize), V>,
    /// Tonnes paid under a grade-conditional value, keyed (position in
    /// [`BlendInput::conditional_values`], interval, segment). Read back only
    /// to value part of a horizon on the model's own terms.
    pub(crate) paid: BTreeMap<(usize, usize, usize), V>,
}

impl<V> BlendColumns<V> {
    pub(crate) fn new() -> Self {
        Self {
            duration: BTreeMap::new(),
            movement: BTreeMap::new(),
            open_t: BTreeMap::new(),
            open_q: BTreeMap::new(),
            recl_t: BTreeMap::new(),
            recl_q: BTreeMap::new(),
            ground_remaining: BTreeMap::new(),
            active: BTreeMap::new(),
            ready: BTreeMap::new(),
            exhausted: BTreeMap::new(),
            chunk_open_t: BTreeMap::new(),
            chunk_recl_t: BTreeMap::new(),
            chunk_closed: BTreeMap::new(),
            chunk_recv: BTreeMap::new(),
            paid: BTreeMap::new(),
        }
    }
}

/// Sizes recorded for the benchmark table.
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct BlendSizes {
    pub(crate) variables: usize,
    pub(crate) binaries: usize,
    pub(crate) linear_constraints: usize,
    pub(crate) nonlinear_constraints: usize,
    /// Nonzero coefficient entries submitted in linear/indicator rows, before
    /// SCIP merges duplicate variables or adds indicator slack variables.
    pub(crate) linear_coefficient_entries: usize,
}

/// The row sink both blended solvers write into.
///
/// This exists so the two methods are not merely *described* as solving the
/// same constraints - they are built by the same function. Everything the
/// blended model needs is here, and the single point where the two methods
/// genuinely differ is [`Rows::mix`].
///
/// Keeping this narrow is deliberate. It is seven operations over an opaque
/// column handle, not a modelling framework: no expression algebra, no solver
/// abstraction, no lifecycle. A backend that cannot express one of these
/// cannot express the blended model at all.
pub(crate) trait Rows {
    /// An opaque column handle. `russcip::Variable` for SCIP, a `good_lp`
    /// variable for the iterative method.
    type Var: Clone;

    fn columns(&mut self) -> &mut BlendColumns<Self::Var>;
    fn sizes(&mut self) -> &mut BlendSizes;

    /// A cheap cooperative checkpoint. The iterative backend uses the
    /// default; a cancellable SCIP worker supplies its atomic signal.
    fn cancelled(&self) -> bool {
        false
    }

    /// A continuous column in `0..=upper`, earning `value` per unit.
    fn valued(&mut self, upper: f64, value: f64, name: &str) -> Self::Var;
    fn binary(&mut self, name: &str) -> Self::Var;
    /// `lhs <= terms . x <= rhs`, as one linear row.
    fn linear(&mut self, terms: Vec<(Self::Var, f64)>, lhs: f64, rhs: f64, name: &str);

    /// Perfect mixing of the released opening blend:
    ///
    /// ```text
    /// Q_recl * T_open - T_recl * Q_open = 0
    /// ```
    ///
    /// **This is the whole difference between the two methods.** SCIP posts
    /// it as the nonconvex bilinear equality it is. The iterative method
    /// cannot, so it posts the linearisation about a fixed grade estimate
    /// `g_hat` for this pile, interval and grade:
    ///
    /// ```text
    /// Q_recl - g_hat * T_recl = 0
    /// ```
    ///
    /// which is exact only where the replayed blend really is `g_hat`, and is
    /// why that method needs replay-and-re-estimate rather than a single
    /// solve. Everything else about the two models is identical, because
    /// everything else is built by the same code.
    #[allow(clippy::too_many_arguments)]
    fn mix(&mut self, pile: StockpileId, interval: usize, grade: usize, recl_q: Self::Var, open_t: Self::Var, recl_t: Self::Var, open_q: Self::Var, name: &str);

    /// `flag = 1  =>  terms . x <= rhs`, with nothing implied when `flag = 0`.
    ///
    /// The default posts the textbook big-M row, `terms . x + M flag <= rhs +
    /// M`, where `big_m` must bound `terms . x - rhs` over the whole model.
    /// That is correct and portable, but a backend checks the row to a
    /// tolerance relative to its own magnitude - which is `M` - so the
    /// implication can be violated by `M x tolerance`. SCIP overrides this
    /// with its native indicator constraint, which enforces the linear part
    /// at its own scale instead.
    fn implies(&mut self, flag: Self::Var, mut terms: Vec<(Self::Var, f64)>, rhs: f64, big_m: f64, name: &str) {
        terms.push((flag, big_m));
        self.leq(terms, rhs + big_m, name);
    }

    fn cont(&mut self, upper: f64, name: &str) -> Self::Var {
        self.valued(upper, 0.0, name)
    }
    fn eq(&mut self, terms: Vec<(Self::Var, f64)>, value: f64, name: &str) {
        self.linear(terms, value, value, name);
    }
    fn leq(&mut self, terms: Vec<(Self::Var, f64)>, value: f64, name: &str) {
        self.linear(terms, f64::NEG_INFINITY, value, name);
    }
    fn geq(&mut self, terms: Vec<(Self::Var, f64)>, value: f64, name: &str) {
        self.linear(terms, value, f64::INFINITY, name);
    }
}

// Several loops below walk a grade / chunk index across *parallel* arrays
// (`open_q`, `recl_q`, `ceilings`, `empty`, ...). Clippy suggests iterating one
// of them, which would only move the indexing to the others and lose the
// symmetry these rows are easiest to read with.
/// Build the blended model.
///
/// The objective is movement value, maximised - the same primary objective
/// the accepted backend uses, so that a blended run and a parcel run at least
/// measure value in the same currency even though their stockpile semantics
/// differ.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FormulationCancelled;

#[allow(clippy::needless_range_loop)]
pub(crate) fn formulate<R: Rows>(rows: &mut R, input: &BlendInput) -> Result<(), FormulationCancelled> {
    let grades = input.grades.count();
    let ceilings = super::input::grade_ceilings(input);
    let segments = input.segments_per_interval.max(1);

    let destination_index: BTreeMap<DestinationId, &Destination> = input.destinations.iter().map(|entry| (entry.id, entry)).collect();

    // ---- shared event clock -------------------------------------------------
    // Every loader agrees on these segment durations, so a source transition
    // inside an interval is a segment boundary for the whole fleet.
    for interval in &input.intervals {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        let mut terms = Vec::with_capacity(segments);
        for segment in 0..segments {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let column = rows.cont(interval.duration_h(), &format!("dur_{}_{segment}", interval.index));
            rows.columns().duration.insert((interval.index, segment), column.clone());
            terms.push((column.clone(), 1.0));
        }
        rows.eq(terms, interval.duration_h(), &format!("clock_{}", interval.index));
    }

    // ---- movement columns ---------------------------------------------------
    // One column per (candidate, interval, segment). A candidate is a
    // resolved destination/truck combination, so routing and cashflow are
    // already decided; only the tonnage is a decision here.
    let reach = DigReach::new(input);
    let held: BTreeSet<GroundId> = input.ground.iter().map(|source| source.id).collect();
    for (index, candidate) in input.movements.iter().enumerate() {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        // A block the input does not hold has no ground balance to draw its
        // tonnes from, so it gets no columns. Capture never names one; a
        // later day of a day-by-day solve names every block that an earlier
        // day finished.
        if let (Activity::Dig, SourceId::Ground(ground)) = (candidate.activity, candidate.source)
            && !held.contains(&ground)
        {
            continue;
        }
        let value = candidate.value_per_tonne().unwrap_or(0.0);
        for interval in &input.intervals {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let Some(rate) = loader_rate(input, candidate, interval.index) else { continue };
            if rate <= 0.0 {
                continue;
            }
            // These columns would be forced to zero by mvsel below. Omit
            // them before building the model, preserving every feasible
            // movement while avoiding inactive-window routing columns.
            if !input.tasks.iter().any(|task| task_active(task, *interval) && task_authorises(task, candidate)) {
                continue;
            }
            // Likewise columns that authored block order forces to zero.
            if !reach.allows(candidate, *interval) {
                continue;
            }
            let bound = rate * interval.duration_h();
            for segment in 0..segments {
                let column = rows.valued(bound, value, &format!("mv_{index}_{}_{segment}", interval.index));
                rows.columns().movement.insert((index, interval.index, segment), column);
            }
        }
    }

    // ---- mandatory authored bar priority (§4) -------------------------------
    // A planner's bar order is an instruction, not a hint. The accepted
    // backend enforces it with a `ready` / `selected` pair per bar and cell,
    // and the blended model reproduces that structure exactly, because
    // without it the solver is free to work whichever bar pays best.
    //
    // Per loader and cell, over the authored order:
    //
    // ```text
    // selected[t] <= ready[t]                            work needs work available
    // selected[t] + ready[e] <= 1      for every e before t    no economic preemption
    // selected[t] >= ready[t] - sum(ready[e] for e before t)   mandatory selection
    // sum over t of selected[t] <= 1                     one bar at a time
    // ```
    //
    // The third row is the one that does the real work. It forbids
    // *voluntary idling*: a loader whose highest-priority ready bar is `t`
    // must select `t`, so it cannot stand down to make a lower-priority bar
    // look like the only option, and it cannot manufacture operational
    // blocking by withholding itself.
    //
    // A bar outside its authored window, or on a loader with no rate for that
    // activity, gets no columns at all, so it is never ready and never blocks.
    // `ready` is *defined* further down, once ground exhaustion and pile
    // inventory exist to define it against.
    for (loader_index, loader) in input.loaders.iter().enumerate() {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        let tasks: Vec<usize> = authored_tasks(input, loader_index);
        for interval in &input.intervals {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            for segment in 0..segments {
                let mut assignment = Vec::new();
                let mut earlier: Vec<R::Var> = Vec::new();
                for &task_index in &tasks {
                    let task = &input.tasks[task_index];
                    if !task_active(task, *interval) || !task_operable(loader, task, interval.index) {
                        continue;
                    }
                    let selected = rows.binary(&format!("act_{loader_index}_{task_index}_{}_{segment}", interval.index));
                    let ready = rows.binary(&format!("rdy_{loader_index}_{task_index}_{}_{segment}", interval.index));
                    rows.columns().active.insert((loader_index, task_index, interval.index, segment), selected.clone());
                    rows.columns().ready.insert((loader_index, task_index, interval.index, segment), ready.clone());

                    rows.leq(
                        vec![(selected.clone(), 1.0), (ready.clone(), -1.0)],
                        0.0,
                        &format!("selready_{loader_index}_{task_index}_{}_{segment}", interval.index),
                    );
                    for (rank, predecessor) in earlier.iter().enumerate() {
                        rows.leq(
                            vec![(selected.clone(), 1.0), (predecessor.clone(), 1.0)],
                            1.0,
                            &format!("nopreempt_{loader_index}_{task_index}_{rank}_{}_{segment}", interval.index),
                        );
                    }
                    let mut forced = vec![(selected.clone(), 1.0), (ready.clone(), -1.0)];
                    for predecessor in &earlier {
                        forced.push((predecessor.clone(), 1.0));
                    }
                    rows.geq(forced, 0.0, &format!("mustwork_{loader_index}_{task_index}_{}_{segment}", interval.index));

                    assignment.push((selected, 1.0));
                    earlier.push(ready);
                }
                if assignment.is_empty() {
                    continue;
                }
                rows.leq(assignment, 1.0, &format!("one_task_{loader_index}_{}_{segment}", interval.index));
            }
        }
    }

    // A loader's movements in a segment cannot exceed rate x that segment's
    // duration, and may only happen under the *selected* bar for the source
    // they draw on. Without the second half the selection binaries would be
    // decorative and priority would not bind on anything.
    let loader_tasks: Vec<Vec<usize>> = (0..input.loaders.len()).map(|loader_index| authored_tasks(input, loader_index)).collect();
    let by_source: Vec<Vec<(SourceId, Vec<usize>)>> = input
        .loaders
        .iter()
        .map(|loader| {
            let mut groups: Vec<(SourceId, Vec<usize>)> = Vec::new();
            for (index, candidate) in input.movements.iter().enumerate().filter(|(_, candidate)| candidate.loader == loader.id) {
                match groups.iter_mut().find(|(source, _)| *source == candidate.source) {
                    Some((_, indices)) => indices.push(index),
                    None => groups.push((candidate.source, vec![index])),
                }
            }
            groups
        })
        .collect();
    for (loader_index, loader) in input.loaders.iter().enumerate() {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        for interval in &input.intervals {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            for segment in 0..segments {
                let duration = rows.columns().duration[&(interval.index, segment)].clone();
                for activity in [Activity::Dig, Activity::Reclaim] {
                    let rate = match activity {
                        Activity::Dig => interval_rate(loader, interval.index).map(|r| r.dig_tph),
                        Activity::Reclaim => interval_rate(loader, interval.index).map(|r| r.reclaim_tph),
                    };
                    let Some(rate) = rate.filter(|value| *value > 0.0) else { continue };
                    let mut terms = vec![(duration.clone(), -rate)];
                    for (index, candidate) in input.movements.iter().enumerate() {
                        if candidate.loader != loader.id || candidate.activity != activity {
                            continue;
                        }
                        if let Some(column) = rows.columns().movement.get(&(index, interval.index, segment)) {
                            terms.push((column.clone(), 1.0));
                        }
                    }
                    if terms.len() > 1 {
                        rows.leq(terms, 0.0, &format!("rate_{loader_index}_{activity:?}_{}_{segment}", interval.index));
                    }
                }

                // Movements from one source <= rate x interval duration x sum
                // of the selection binaries of the bars that authorise that
                // source. Several bars may authorise one source; any of them
                // permits the movement, and `one_task` already forbids
                // selecting more than one.
                //
                // One row per source rather than per candidate: bar
                // authority depends only on (loader, source), and the loader's
                // `rate` row already caps the sum, so whole-number solutions
                // are unchanged while a fractional selection admits the
                // candidates' combined tonnage once instead of once each.
                for (source, indices) in &by_source[loader_index] {
                    let terms: Vec<(R::Var, f64)> = indices
                        .iter()
                        .filter_map(|&index| rows.columns().movement.get(&(index, interval.index, segment)).cloned())
                        .map(|column| (column, 1.0))
                        .collect();
                    if terms.is_empty() {
                        continue;
                    }
                    let exemplar = &input.movements[indices[0]];
                    let Some(rate) = loader_rate(input, exemplar, interval.index).filter(|value| *value > 0.0) else {
                        continue;
                    };
                    let big_m = rate * interval.duration_h();
                    let mut terms = terms;
                    for &task_index in &loader_tasks[loader_index] {
                        if !task_authorises(&input.tasks[task_index], exemplar) {
                            continue;
                        }
                        if let Some(selected) = rows.columns().active.get(&(loader_index, task_index, interval.index, segment)) {
                            terms.push((selected.clone(), -big_m));
                        }
                    }
                    rows.leq(terms, 0.0, &format!("mvsel_{loader_index}_{}_{}_{segment}", source_key(*source), interval.index));
                }
            }
        }
    }

    // ---- ground conservation ------------------------------------------------
    // Shared ground cannot be depleted twice: one balance per source carried
    // across segments, so several loaders digging the same block compete.
    //
    // Before the first interval with a dig column a block is untouched: its
    // remaining tonnage is the authored figure and it is not exhausted. That
    // state is a constant, so it gets no columns; the state begins in the
    // first interval anything can dig it, and a block nothing can dig in the
    // horizon has none at all. Consumers below read a missing state as
    // "untouched". An empty block is exhausted from the start, so it keeps
    // its state from the first cell.
    let mut dig_candidates: Vec<Vec<usize>> = vec![Vec::new(); input.ground.len()];
    for (index, candidate) in input.movements.iter().enumerate() {
        if candidate.activity != Activity::Dig {
            continue;
        }
        let SourceId::Ground(ground) = candidate.source else { continue };
        if let Some(source_index) = input.ground.iter().position(|source| source.id == ground) {
            dig_candidates[source_index].push(index);
        }
    }
    let dug_in = |rows: &mut R, source_index: usize, interval: usize| {
        dig_candidates[source_index]
            .iter()
            .any(|&index| (0..segments).any(|segment| rows.columns().movement.contains_key(&(index, interval, segment))))
    };
    let mut state_from: Vec<Option<usize>> = vec![None; input.ground.len()];
    for (source_index, source) in input.ground.iter().enumerate() {
        state_from[source_index] = if source.tonnes_t <= 0.0 {
            input.intervals.first().map(|interval| interval.index)
        } else {
            input.intervals.iter().map(|interval| interval.index).find(|&interval| dug_in(rows, source_index, interval))
        };
    }

    for (source_index, source) in input.ground.iter().enumerate() {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        let Some(first) = state_from[source_index] else { continue };
        let mut previous: Option<R::Var> = None;
        for interval in input.intervals.iter().filter(|interval| interval.index >= first) {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            for segment in 0..segments {
                let remaining = rows.cont(source.tonnes_t, &format!("grd_{source_index}_{}_{segment}", interval.index));
                rows.columns()
                    .ground_remaining
                    .insert((source_index, flat_cell(interval.index, segment, segments)), remaining.clone());
                let mut terms = vec![(remaining.clone(), 1.0)];
                for (index, candidate) in input.movements.iter().enumerate() {
                    if candidate.activity != Activity::Dig || candidate.source != SourceId::Ground(source.id) {
                        continue;
                    }
                    if let Some(column) = rows.columns().movement.get(&(index, interval.index, segment)) {
                        terms.push((column.clone(), 1.0));
                    }
                }
                match previous.clone() {
                    Some(prior) => {
                        terms.push((prior.clone(), -1.0));
                        rows.eq(terms, 0.0, &format!("grdbal_{source_index}_{}_{segment}", interval.index));
                    }
                    None => {
                        rows.eq(terms, source.tonnes_t, &format!("grdopen_{source_index}_{}_{segment}", interval.index));
                    }
                }
                previous = Some(remaining.clone());
            }
        }
    }

    // ---- proportional extraction of a mixed block ---------------------------
    // A dig block is one physical volume, not a menu. If it measures 60%
    // material A and 40% material B, removing 100 t removes 60 t of A and
    // 40 t of B - in *every* execution segment, not merely over the horizon.
    // The portions may have different eligible destinations, truck classes
    // and cashflow coefficients, so without this the solver would take the
    // profitable portion first and leave a block whose remaining composition
    // it had silently changed.
    //
    // Per source and cell, with `extract` the block tonnes removed in that
    // cell:
    //
    // ```text
    // sum(movements of material m from this source) = fraction[m] * extract
    // ```
    //
    // Summed over `m` this is the block's own completion balance, because the
    // fractions sum to one; stated per material it is the proportionality.
    // A portion with no candidate column in the cell contributes an empty
    // sum, which forces `extract` to zero - a blocked portion prevents the
    // extraction rather than being quietly left behind, unless some other
    // eligible route for it exists.
    for (source_index, source) in input.ground.iter().enumerate() {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        if source.material.len() < 2 {
            // One material: the balance above already says everything, and
            // `fraction = 1` would only restate it.
            continue;
        }
        for interval in &input.intervals {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            // No column can draw on the block here, so every portion sum is
            // empty and `extract` would only be forced to zero.
            if !dug_in(rows, source_index, interval.index) {
                continue;
            }
            for segment in 0..segments {
                let extract = rows.cont(source.tonnes_t, &format!("ext_{source_index}_{}_{segment}", interval.index));
                for share in &source.material {
                    let mut terms = vec![(extract.clone(), -share.fraction)];
                    for (index, candidate) in input.movements.iter().enumerate() {
                        if candidate.activity != Activity::Dig || candidate.source != SourceId::Ground(source.id) || candidate.material != share.material {
                            continue;
                        }
                        if let Some(column) = rows.columns().movement.get(&(index, interval.index, segment)) {
                            terms.push((column.clone(), 1.0));
                        }
                    }
                    rows.eq(terms, 0.0, &format!("portion_{source_index}_{}_{}_{segment}", share.material.0, interval.index));
                }
            }
        }
    }

    // ---- authored dig-block order -------------------------------------------
    // A loader works its task's sequence in the order a planner authored. A
    // later block cannot be touched until the earlier one is exhausted, and
    // that is *not* negotiable for economic reasons - which is exactly what
    // would happen if the solver were free to pick the most valuable block.
    //
    // `exhausted[g, c]` is 1 only when source `g` has nothing left at the end
    // of cell `c`; the implication runs one way (remaining > 0 forces it to
    // 0), which is all the ordering needs, and leaving the converse free
    // costs nothing because nothing rewards setting it.
    //
    // A loader may work through several blocks of its own sequence inside
    // one segment: back to back, in authored order, at its dig rate. So the
    // later block needs the earlier one exhausted by the end of the *same*
    // cell, and a segment boundary is only spent on a change of bar. The
    // loader's `rate` row already makes the blocks fit the segment one after
    // another. The exception is an earlier block another loader can also
    // dig: when that loader finishes it inside the segment cannot be
    // recovered from segment totals, so the later block still waits for the
    // end of the previous cell.
    let cells: Vec<(usize, usize)> = input
        .intervals
        .iter()
        .flat_map(|interval| (0..segments).map(move |segment| (interval.index, segment)))
        .collect();

    for (source_index, source) in input.ground.iter().enumerate() {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        for (position, &(interval, segment)) in cells.iter().enumerate() {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            // An untouched block is not exhausted: no flag.
            let Some(remaining) = rows.columns().ground_remaining.get(&(source_index, flat_cell(interval, segment, segments))).cloned() else {
                continue;
            };
            let flag = rows.binary(&format!("exh_{source_index}_{interval}_{segment}"));
            rows.columns().exhausted.insert((source_index, position), flag.clone());
            // remaining <= tonnes * (1 - flag)
            rows.leq(
                vec![(remaining, 1.0), (flag, source.tonnes_t)],
                source.tonnes_t,
                &format!("exhlink_{source_index}_{interval}_{segment}"),
            );
        }
    }

    for task in &input.tasks {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        let TaskKind::Dig { sequence } = &task.kind else { continue };
        let Some(loader) = input.loaders.iter().find(|entry| entry.id == task.loader) else {
            continue;
        };
        for window in sequence.windows(2) {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let [earlier, later] = window else { continue };
            let Some(earlier_index) = input.ground.iter().position(|entry| entry.id == *earlier) else {
                continue;
            };
            let shared = dig_candidates[earlier_index].iter().any(|&index| input.movements[index].loader != task.loader);
            let later_candidates: Vec<usize> = input
                .movements
                .iter()
                .enumerate()
                .filter(|(_, candidate)| candidate.activity == Activity::Dig && candidate.source == SourceId::Ground(*later) && candidate.loader == task.loader)
                .map(|(index, _)| index)
                .collect();
            if later_candidates.is_empty() {
                continue;
            }
            for (position, &(interval, segment)) in cells.iter().enumerate() {
                let mut terms: Vec<(R::Var, f64)> = later_candidates
                    .iter()
                    .filter_map(|&index| rows.columns().movement.get(&(index, interval, segment)).cloned())
                    .map(|column| (column, 1.0))
                    .collect();
                if terms.is_empty() {
                    continue;
                }
                let name = format!("{}_{}_{}_{}_{interval}_{segment}", task.id.0, earlier.0, later.0, task.loader.0);
                // Digging `later` in this cell requires `earlier` to be
                // exhausted by the end of this cell, or of the previous one
                // when `earlier` is shared. Before the first cell nothing can
                // be exhausted, and an untouched earlier block has no flag
                // because it cannot be: either way the later block is
                // unavailable here.
                let flag = if shared { position.checked_sub(1) } else { Some(position) }.and_then(|cell| rows.columns().exhausted.get(&(earlier_index, cell)).cloned());
                let Some(flag) = flag else {
                    rows.leq(terms, 0.0, &format!("order0_{name}"));
                    continue;
                };
                // One row for all of this loader's candidates on `later`:
                // sum <= rate x interval x flag. The loader's `rate` row caps
                // the sum at that anyway, so this admits the same schedules as
                // a row per candidate, and binds harder on a fractional flag.
                let Some(rate) = interval_rate(loader, interval).map(|rate| rate.dig_tph).filter(|rate| *rate > 0.0) else {
                    continue;
                };
                terms.push((flag, -rate * input.intervals[interval].duration_h()));
                rows.leq(terms, 0.0, &format!("order_{name}"));
            }
        }
    }

    // ---- blended inventory --------------------------------------------------
    let receipt_peaks = piles_with_receipt_peaks(input, &destination_index);
    for pile in &input.piles {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        let receipt_peak = receipt_peaks.contains(&pile.id);
        if !pile.chunks.is_empty() {
            chunked_pile(rows, input, pile, grades, &ceilings, segments, &destination_index, receipt_peak)?;
            continue;
        }
        let capacity = pile.capacity_t;
        let mut open_t_prev: Option<R::Var> = None;
        let mut open_q_prev: Vec<Option<R::Var>> = vec![None; grades];

        for interval in &input.intervals {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let k = interval.index;
            // Released opening state.
            let open_t = rows.cont(capacity, &format!("openT_{}_{k}", pile.id.0));
            rows.columns().open_t.insert((pile.id, k), open_t.clone());
            let mut open_q = Vec::with_capacity(grades);
            for g in 0..grades {
                let column = rows.cont(capacity * ceilings[g], &format!("openQ_{}_{k}_{g}", pile.id.0));
                rows.columns().open_q.insert((pile.id, k, g), column.clone());
                open_q.push(column);
            }

            // Opening state is the authored opening at k = 0, and otherwise
            // the previous interval's balance.
            match open_t_prev.clone() {
                None => rows.eq(vec![(open_t.clone(), 1.0)], pile.opening_t, &format!("openT0_{}", pile.id.0)),
                Some(prior) => {
                    let mut terms = vec![(open_t.clone(), 1.0), (prior.clone(), -1.0)];
                    // + receipts during k-1  - reclaim during k-1
                    for (index, candidate) in input.movements.iter().enumerate() {
                        if delivers_to_pile(candidate, pile.id, &destination_index) {
                            for segment in 0..segments {
                                if let Some(column) = rows.columns().movement.get(&(index, k - 1, segment)) {
                                    terms.push((column.clone(), -1.0));
                                }
                            }
                        }
                    }
                    let recl_prev = rows.columns().recl_t[&(pile.id, k - 1)].clone();
                    terms.push((recl_prev.clone(), 1.0));
                    rows.eq(terms, 0.0, &format!("openTbal_{}_{k}", pile.id.0));
                }
            }
            for g in 0..grades {
                match open_q_prev[g].clone() {
                    None => rows.eq(
                        vec![(open_q[g].clone(), 1.0)],
                        pile.opening_q.get(g).copied().unwrap_or(0.0),
                        &format!("openQ0_{}_{g}", pile.id.0),
                    ),
                    Some(prior) => {
                        let mut terms = vec![(open_q[g].clone(), 1.0), (prior.clone(), -1.0)];
                        for (index, candidate) in input.movements.iter().enumerate() {
                            if !delivers_to_pile(candidate, pile.id, &destination_index) {
                                continue;
                            }
                            // Receipts have identified material, so their
                            // contained quantity is linear in their tonnes.
                            let Some(fraction) = input.grades.fraction(candidate.material, g) else { continue };
                            for segment in 0..segments {
                                if let Some(column) = rows.columns().movement.get(&(index, k - 1, segment)) {
                                    terms.push((column.clone(), -fraction));
                                }
                            }
                        }
                        let recl_prev = rows.columns().recl_q[&(pile.id, k - 1, g)].clone();
                        terms.push((recl_prev.clone(), 1.0));
                        rows.eq(terms, 0.0, &format!("openQbal_{}_{k}_{g}", pile.id.0));
                    }
                }
            }

            // Reclaim totals for this interval.
            let recl_t = rows.cont(capacity, &format!("reclT_{}_{k}", pile.id.0));
            rows.columns().recl_t.insert((pile.id, k), recl_t.clone());
            let mut recl_q = Vec::with_capacity(grades);
            for g in 0..grades {
                let column = rows.cont(capacity * ceilings[g], &format!("reclQ_{}_{k}_{g}", pile.id.0));
                rows.columns().recl_q.insert((pile.id, k, g), column.clone());
                recl_q.push(column);
            }

            // The interval's reclaim total is the sum of its reclaim
            // movements from this pile across every segment.
            let mut terms = vec![(recl_t.clone(), 1.0)];
            for (index, candidate) in input.movements.iter().enumerate() {
                if candidate.activity != Activity::Reclaim || candidate.source != SourceId::Stockpile(pile.id) {
                    continue;
                }
                for segment in 0..segments {
                    if let Some(column) = rows.columns().movement.get(&(index, k, segment)) {
                        terms.push((column.clone(), -1.0));
                    }
                }
            }
            rows.eq(terms, 0.0, &format!("reclTsum_{}_{k}", pile.id.0));

            // Cannot reclaim more than the released opening tonnes.
            rows.leq(vec![(recl_t.clone(), 1.0), (open_t.clone(), -1.0)], 0.0, &format!("reclcap_{}_{k}", pile.id.0));

            // Perfect mixing of the released opening blend, and the grade-box
            // rows that make it sound at zero inventory.
            for g in 0..grades {
                rows.mix(
                    pile.id,
                    k,
                    g,
                    recl_q[g].clone(),
                    open_t.clone(),
                    recl_t.clone(),
                    open_q[g].clone(),
                    &format!("mix_{}_{k}_{g}", pile.id.0),
                );
                rows.leq(
                    vec![(recl_q[g].clone(), 1.0), (recl_t.clone(), -ceilings[g])],
                    0.0,
                    &format!("reclbox_{}_{k}_{g}", pile.id.0),
                );
                rows.leq(
                    vec![(open_q[g].clone(), 1.0), (open_t.clone(), -ceilings[g])],
                    0.0,
                    &format!("openbox_{}_{k}_{g}", pile.id.0),
                );
            }

            // Physical occupancy throughout the interval (§3).
            occupancy_rows(rows, input, pile, k, segments, &destination_index, vec![(open_t.clone(), 1.0)], receipt_peak);

            open_t_prev = Some(open_t.clone());
            for g in 0..grades {
                open_q_prev[g] = Some(open_q[g].clone());
            }
        }
    }

    // ---- what "ready" means (§4) -------------------------------------------
    // Readiness is *defined* here rather than at the assignment rows above,
    // because it is defined against ground exhaustion and pile inventory,
    // which only exist by this point.
    //
    // Both definitions lean on indicator columns that are truthful in the
    // one direction that matters. `exhausted[g, c]` can only be 1 when source
    // `g` really has nothing left, because `remaining > 0` forces it to 0;
    // `stock[p, k]` can only be 0 when pile `p` really is empty. So a bar
    // cannot claim to be unready while it still has work, which is the
    // direction a solver would want to cheat in.

    // Pile stock indicators, one per pile and interval.
    let mut stock: BTreeMap<(StockpileId, usize), R::Var> = BTreeMap::new();
    for pile in &input.piles {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        for interval in &input.intervals {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let k = interval.index;
            let flag = rows.binary(&format!("stock_{}_{k}", pile.id.0));
            let opening = pile_opening_terms(rows, pile, k);
            if opening.is_empty() {
                continue;
            }
            // open_t <= capacity * stock, so open_t > 0 forces stock = 1.
            let mut terms = opening;
            terms.push((flag.clone(), -pile.capacity_t));
            rows.leq(terms, 0.0, &format!("stocklink_{}_{k}", pile.id.0));
            stock.insert((pile.id, k), flag);
        }
    }

    for (loader_index, _loader) in input.loaders.iter().enumerate() {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        for &task_index in &authored_tasks(input, loader_index) {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let task = &input.tasks[task_index];
            for interval in &input.intervals {
                for segment in 0..segments {
                    let key = (loader_index, task_index, interval.index, segment);
                    let Some(ready) = rows.columns().ready.get(&key).cloned() else { continue };
                    let position = flat_cell(interval.index, segment, segments);
                    match &task.kind {
                        TaskKind::Dig { sequence } => {
                            // Ready while any block in the authored sequence
                            // still holds material at the start of this cell.
                            let Some(previous) = position.checked_sub(1) else {
                                // Before the first cell the input's own
                                // tonnages are the state: ready exactly when
                                // some block of the sequence holds material.
                                let holds = sequence.iter().any(|source| input.ground.iter().any(|entry| entry.id == *source && entry.tonnes_t > 0.0));
                                rows.eq(
                                    vec![(ready, 1.0)],
                                    if holds { 1.0 } else { 0.0 },
                                    &format!("rdydig0_{loader_index}_{task_index}_{position}"),
                                );
                                continue;
                            };
                            let blocks: Vec<usize> = sequence.iter().filter_map(|source| input.ground.iter().position(|entry| entry.id == *source)).collect();
                            let flags: Vec<R::Var> = blocks
                                .iter()
                                .filter_map(|&source_index| rows.columns().exhausted.get(&(source_index, previous)).cloned())
                                .collect();
                            // A block without an exhaustion flag is untouched
                            // and still holds material, so the bar has work.
                            if flags.len() < blocks.len() {
                                rows.eq(vec![(ready, 1.0)], 1.0, &format!("rdydigleft_{loader_index}_{task_index}_{position}"));
                                continue;
                            }
                            if flags.is_empty() {
                                rows.eq(vec![(ready, 1.0)], 0.0, &format!("rdydignone_{loader_index}_{task_index}_{position}"));
                                continue;
                            }
                            // ready >= 1 - exhausted[g]  for each block
                            for (rank, flag) in flags.iter().enumerate() {
                                rows.geq(
                                    vec![(ready.clone(), 1.0), (flag.clone(), 1.0)],
                                    1.0,
                                    &format!("rdydiglo_{loader_index}_{task_index}_{rank}_{position}"),
                                );
                            }
                            // ready <= sum(1 - exhausted[g])
                            let mut terms = vec![(ready.clone(), 1.0)];
                            for flag in &flags {
                                terms.push((flag.clone(), 1.0));
                            }
                            rows.leq(terms, flags.len() as f64, &format!("rdydighi_{loader_index}_{task_index}_{position}"));
                        }
                        TaskKind::Reclaim { approved_sources, maximum_t } => {
                            let flags: Vec<R::Var> = approved_sources.iter().filter_map(|pile| stock.get(&(*pile, interval.index)).cloned()).collect();
                            if flags.is_empty() {
                                rows.eq(vec![(ready, 1.0)], 0.0, &format!("rdyrecnone_{loader_index}_{task_index}_{position}"));
                                continue;
                            }
                            // ready >= stock[p] for each approved pile, and
                            // ready <= sum of them, so "some approved pile has
                            // released stock" is exactly readiness.
                            for (rank, flag) in flags.iter().enumerate() {
                                rows.geq(
                                    vec![(ready.clone(), 1.0), (flag.clone(), -1.0)],
                                    0.0,
                                    &format!("rdyreclo_{loader_index}_{task_index}_{rank}_{position}"),
                                );
                            }
                            let mut terms = vec![(ready.clone(), 1.0)];
                            for flag in &flags {
                                terms.push((flag.clone(), -1.0));
                            }
                            rows.leq(terms, 0.0, &format!("rdyrechi_{loader_index}_{task_index}_{position}"));

                            // A bar that has consumed its authored reclaim cap
                            // has no work left. Without this it would stay
                            // "ready" for the rest of the horizon and block
                            // every lower-priority bar on the same loader -
                            // refusing valid schedules rather than admitting
                            // invalid ones, but refusing them all the same.
                            let Some(maximum) = maximum_t else { continue };
                            let spent = rows.binary(&format!("capgone_{loader_index}_{task_index}_{position}"));
                            let mut used: Vec<(R::Var, f64)> = Vec::new();
                            for (index, candidate) in input.movements.iter().enumerate() {
                                if !task_authorises(task, candidate) {
                                    continue;
                                }
                                for earlier in &input.intervals {
                                    for earlier_segment in 0..segments {
                                        if flat_cell(earlier.index, earlier_segment, segments) >= position {
                                            continue;
                                        }
                                        if let Some(column) = rows.columns().movement.get(&(index, earlier.index, earlier_segment)) {
                                            used.push((column.clone(), 1.0));
                                        }
                                    }
                                }
                            }
                            // maximum - used <= maximum * (1 - spent), so any
                            // remaining allowance forces `spent` to 0.
                            let mut terms = used;
                            for term in terms.iter_mut() {
                                term.1 = -1.0;
                            }
                            terms.push((spent.clone(), *maximum));
                            rows.leq(terms, 0.0, &format!("caplink_{loader_index}_{task_index}_{position}"));
                            // ready <= 1 - spent
                            rows.leq(vec![(ready.clone(), 1.0), (spent, 1.0)], 1.0, &format!("rdycap_{loader_index}_{task_index}_{position}"));
                        }
                    }
                }
            }
        }
    }

    // ---- shared truck hours -------------------------------------------------
    // Truck-hours inside a segment cannot exceed the class's share of the
    // interval budget, prorated by that segment's duration.
    for truck in &input.trucks {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        for interval in &input.intervals {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let Some(hours) = truck.hours.get(interval.index).copied() else { continue };
            if interval.duration_h() <= 0.0 {
                continue;
            }
            let fleet = hours / interval.duration_h();
            for segment in 0..segments {
                let duration = rows.columns().duration[&(interval.index, segment)].clone();
                let mut terms = vec![(duration.clone(), -fleet)];
                for (index, candidate) in input.movements.iter().enumerate() {
                    if candidate.truck != truck.id {
                        continue;
                    }
                    if let Some(column) = rows.columns().movement.get(&(index, interval.index, segment)) {
                        terms.push((column.clone(), candidate.truck_hours_per_tonne));
                    }
                }
                if terms.len() > 1 {
                    rows.leq(terms, 0.0, &format!("truck_{}_{}_{segment}", truck.id.0, interval.index));
                }
            }
        }
    }

    // ---- destination limits -------------------------------------------------
    // Direct mining and reclaim share the same crusher budgets: the rows below
    // sum over every movement to the destination regardless of activity.
    for destination in &input.destinations {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        match destination.kind {
            DestinationKind::Crusher => {
                for (day, budget) in destination.crusher_daily_t.iter().enumerate() {
                    let Some(budget) = budget else { continue };
                    let mut terms = Vec::new();
                    for interval in input.intervals.iter().filter(|entry| entry.day() as usize == day) {
                        for (index, candidate) in input.movements.iter().enumerate() {
                            if candidate.destination != destination.id {
                                continue;
                            }
                            for segment in 0..segments {
                                if let Some(column) = rows.columns().movement.get(&(index, interval.index, segment)) {
                                    terms.push((column.clone(), 1.0));
                                }
                            }
                        }
                    }
                    rows.leq(terms, *budget, &format!("crusher_{}_{day}", destination.id.0));
                }
            }
            DestinationKind::Dump => {
                let Some(capacity) = destination.capacity_t else { continue };
                let mut terms = Vec::new();
                for interval in &input.intervals {
                    for (index, candidate) in input.movements.iter().enumerate() {
                        if candidate.destination != destination.id {
                            continue;
                        }
                        for segment in 0..segments {
                            if let Some(column) = rows.columns().movement.get(&(index, interval.index, segment)) {
                                terms.push((column.clone(), 1.0));
                            }
                        }
                    }
                }
                rows.leq(terms, capacity, &format!("dest_{}", destination.id.0));
            }
            // Storage capacity is enforced by occupancy_rows, not by lifetime
            // receipts: reclaim frees space for later deliveries. Chunk slot
            // non-reuse remains a separate, intentional lifecycle constraint.
            DestinationKind::Stockpile(_) => {}
        }
    }

    // ---- reclaim caps -------------------------------------------------------
    for (task_index, task) in input.tasks.iter().enumerate() {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        let TaskKind::Reclaim {
            approved_sources,
            maximum_t: Some(maximum),
        } = &task.kind
        else {
            continue;
        };
        let mut terms = Vec::new();
        for (index, candidate) in input.movements.iter().enumerate() {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            if candidate.loader != input.tasks[task_index].loader || candidate.activity != Activity::Reclaim {
                continue;
            }
            let SourceId::Stockpile(pile) = candidate.source else { continue };
            if !approved_sources.contains(&pile) {
                continue;
            }
            for interval in &input.intervals {
                for segment in 0..segments {
                    if let Some(column) = rows.columns().movement.get(&(index, interval.index, segment)) {
                        terms.push((column.clone(), 1.0));
                    }
                }
            }
        }
        rows.leq(terms, *maximum, &format!("reclmax_{task_index}"));
    }

    // ---- destination eligibility for reclaimed material (§5, §6) -----------
    // OR between rules, AND within a rule.
    //
    // Every reclaim from one pile in one interval sees the same released
    // blend, so a condition on any such movement is a condition on that
    // interval's blend. For each (loader, pile, destination) the routing
    // rules admit under a grade condition, and each interval:
    //
    // ```text
    // flow <= M_flow * used                       did anything take this route
    // sum over alternatives of pick[a] >= used    at least one rule must admit it
    // pick[a] = 1  =>  every bound of a holds     that rule's conditions, ANDed
    // ```
    //
    // The alternatives are never merged. `Fe <= 0.55` and `Fe >= 0.65` into
    // one destination are two rows the solver must choose between, so a blend
    // of 0.60 satisfies neither and the destination is closed - which a single
    // widened interval would have got wrong.
    //
    // Each conditional row is a one-way implication, which is all eligibility
    // needs: a route may be closed on a grade that would have passed, and can
    // never be opened on one that fails.
    let mut qualifications = input.qualifications.clone();
    for limit in &input.grade_limits {
        for candidate in input
            .movements
            .iter()
            .filter(|candidate| candidate.activity == Activity::Reclaim && candidate.destination == limit.destination)
        {
            let SourceId::Stockpile(pile) = candidate.source else { continue };
            if qualifications
                .iter()
                .any(|entry| entry.loader == candidate.loader && entry.pile == pile && entry.destination == limit.destination)
            {
                continue;
            }
            qualifications.push(GradeQualification {
                loader: candidate.loader,
                pile,
                destination: limit.destination,
                alternatives: vec![GradePredicate {
                    rule: candidate.routing_rule,
                    bounds: vec![GradeBound {
                        grade: limit.grade,
                        lower: Some(GradeEndpoint {
                            value: limit.minimum,
                            inclusive: limit.inclusive,
                        }),
                        upper: None,
                    }],
                }],
            });
        }
    }
    for qualification in &qualifications {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        // A rule that admits on identity alone makes the whole disjunction
        // true, so there is nothing to constrain.
        if qualification.unconditional() {
            continue;
        }
        let routed: Vec<usize> = input
            .movements
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                candidate.activity == Activity::Reclaim
                    && candidate.loader == qualification.loader
                    && candidate.source == SourceId::Stockpile(qualification.pile)
                    && candidate.destination == qualification.destination
            })
            .map(|(index, _)| index)
            .collect();
        if routed.is_empty() {
            continue;
        }
        for interval in &input.intervals {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let k = interval.index;
            let Some(recl_t) = rows.columns().recl_t.get(&(qualification.pile, k)).cloned() else {
                continue;
            };
            // Whether any material took this route this interval. The big-M is
            // the *tightest* physical bound on what can - the routed loader's
            // own rate times the interval - not the pile's capacity. A binary
            // may sit up to the integrality tolerance from zero, so a loose M
            // admits `M x tolerance` tonnes through an indicator reading as
            // "unused"; at the pile's capacity that was 1e-4 t of off-spec
            // material, which the replay caught.
            let mut flow = Vec::new();
            let mut big_m_flow = 0.0;
            for &index in &routed {
                for segment in 0..segments {
                    if let Some(column) = rows.columns().movement.get(&(index, k, segment)).cloned() {
                        flow.push((column, 1.0));
                        big_m_flow += loader_rate(input, &input.movements[index], k).unwrap_or(0.0) * interval.duration_h();
                    }
                }
            }
            if flow.is_empty() || big_m_flow <= 0.0 {
                continue;
            }
            let used = rows.binary(&format!("elig_{}_{}_{k}_{}", qualification.loader.0, qualification.pile.0, qualification.destination.0));
            flow.push((used.clone(), -big_m_flow));
            rows.leq(
                flow,
                0.0,
                &format!("eligused_{}_{}_{k}_{}", qualification.loader.0, qualification.pile.0, qualification.destination.0),
            );

            // The tonnes a reclaim from this pile can actually amount to in
            // this interval, which is what every conditional grade row below
            // is scaled against.
            let ceiling_t = reclaim_ceiling(input, qualification.pile, k, segments);
            let mut picks = vec![(used.clone(), -1.0)];
            for (position, alternative) in qualification.alternatives.iter().enumerate() {
                let pick = rows.binary(&format!(
                    "pick_{}_{}_{k}_{}_{position}",
                    qualification.loader.0, qualification.pile.0, qualification.destination.0
                ));
                picks.push((pick.clone(), 1.0));
                for (test_index, test) in alternative.half_spaces().into_iter().enumerate() {
                    let Some(recl_q) = rows.columns().recl_q.get(&(qualification.pile, k, test.grade)).cloned() else {
                        continue;
                    };
                    implies_half_space(
                        rows,
                        pick.clone(),
                        recl_q,
                        recl_t.clone(),
                        test,
                        ceilings.get(test.grade).copied().unwrap_or(1.0),
                        ceiling_t,
                        &format!(
                            "eligbound_{}_{}_{k}_{}_{position}_{test_index}",
                            qualification.loader.0, qualification.pile.0, qualification.destination.0
                        ),
                    );
                }
            }
            rows.geq(
                picks,
                0.0,
                &format!("eligpick_{}_{}_{k}_{}", qualification.loader.0, qualification.pile.0, qualification.destination.0),
            );
        }
    }

    // ---- grade-conditional cashflow on reclaim (§6) -------------------------
    // A value that depends on the blend cannot ride on the movement column,
    // because that column earns its coefficient whatever the grade turns out
    // to be. Each conditional contribution gets its own paid-tonnes column,
    // earning the authored value per tonne, tied to the movement by an
    // indicator whose direction depends on the value's sign:
    //
    // ```text
    // reward (value > 0):  paid <= moved,  paid <= M * earns
    //                      earns = 1  =>  every bound holds, tightened inward
    // cost   (value < 0):  paid <= moved,  paid >= moved - M * (1 - owes)
    //                      owes >= 1 - sum(fails)
    //                      fails[i] = 1  =>  bound i is broken, tightened outward
    // ```
    //
    // A reward can therefore only be claimed on a blend that clears every
    // bound by [`GRADE_MARGIN`], and a cost can only be escaped on a blend
    // that breaks some bound by the same margin - so the optimiser can
    // neither claim a nonmatching positive payment nor evade a matching
    // negative one.
    //
    // No blend is *forbidden* by this. An earlier revision built an exact
    // two-sided truth indicator, which cannot hold at all for a blend within
    // a margin of the boundary; that excluded those blends from every
    // reclaim of the pile in the interval, whether or not the valued
    // movement was used. The one-way form only values such a blend
    // conservatively, and the replay publishes the authored value.
    for (position, conditional) in input.conditional_values.iter().enumerate() {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        let Some(candidate) = input.movements.get(conditional.candidate) else { continue };
        let SourceId::Stockpile(pile) = candidate.source else { continue };
        if candidate.activity != Activity::Reclaim || conditional.value_per_tonne == 0.0 {
            continue;
        }
        let reward = conditional.value_per_tonne > 0.0;
        for interval in &input.intervals {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let k = interval.index;
            let (Some(_recl_t), Some(rate)) = (rows.columns().recl_t.get(&(pile, k)).cloned(), loader_rate(input, candidate, k)) else {
                continue;
            };
            if rate <= 0.0 {
                continue;
            }
            let ceiling_t = reclaim_ceiling(input, pile, k, segments);
            // One indicator per interval: the blend is the interval's, so
            // every segment of it answers the predicate the same way.
            let tests = conditional.half_spaces();
            let name = format!("cv_{position}_{k}");
            let indicator = if reward {
                predicate_earned(rows, pile, k, &tests, &ceilings, ceiling_t, &name)
            } else {
                predicate_owed(rows, pile, k, &tests, &ceilings, ceiling_t, &name)
            };
            let Some(indicator) = indicator else { continue };
            let big_m = rate * interval.duration_h();
            for segment in 0..segments {
                let Some(moved) = rows.columns().movement.get(&(conditional.candidate, k, segment)).cloned() else {
                    continue;
                };
                let paid = rows.valued(big_m, conditional.value_per_tonne, &format!("paid_{position}_{k}_{segment}"));
                rows.columns().paid.insert((position, k, segment), paid.clone());
                rows.leq(vec![(paid.clone(), 1.0), (moved.clone(), -1.0)], 0.0, &format!("paidcap_{position}_{k}_{segment}"));
                if reward {
                    rows.leq(vec![(paid, 1.0), (indicator.clone(), -big_m)], 0.0, &format!("paidoff_{position}_{k}_{segment}"));
                } else {
                    rows.geq(
                        vec![(paid, 1.0), (moved, -1.0), (indicator.clone(), -big_m)],
                        -big_m,
                        &format!("paidon_{position}_{k}_{segment}"),
                    );
                }
            }
        }
    }

    // ---- one stockpile per loader and segment (§8) --------------------------
    // A loader reclaims from one pile at a time. Its authored bar may permit
    // several, and choosing between them is exactly the freedom the bar
    // grants - but drawing on two at once in the same segment is not a
    // schedule any machine can execute, and it would also let one loader
    // average two blends the model never mixed.
    //
    // Switching is permitted at a segment boundary within an interval, which
    // is what makes this a per-segment row rather than a per-interval one.
    // A loader with only one reachable pile gets no rows at all.
    for loader in &input.loaders {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        let mut piles: Vec<StockpileId> = Vec::new();
        for candidate in input
            .movements
            .iter()
            .filter(|candidate| candidate.loader == loader.id && candidate.activity == Activity::Reclaim)
        {
            if let SourceId::Stockpile(pile) = candidate.source
                && !piles.contains(&pile)
            {
                piles.push(pile);
            }
        }
        if piles.len() < 2 {
            continue;
        }
        for interval in &input.intervals {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let k = interval.index;
            let Some(rate) = interval_rate(loader, k).map(|rate| rate.reclaim_tph).filter(|rate| *rate > 0.0) else {
                continue;
            };
            let big_m = rate * interval.duration_h();
            for segment in 0..segments {
                let mut chosen = Vec::with_capacity(piles.len());
                for pile in &piles {
                    let flag = rows.binary(&format!("srcpick_{}_{}_{k}_{segment}", loader.id.0, pile.0));
                    let mut terms = vec![(flag.clone(), -big_m)];
                    for (index, candidate) in input.movements.iter().enumerate() {
                        if candidate.loader != loader.id || candidate.activity != Activity::Reclaim || candidate.source != SourceId::Stockpile(*pile) {
                            continue;
                        }
                        if let Some(column) = rows.columns().movement.get(&(index, k, segment)).cloned() {
                            terms.push((column, 1.0));
                        }
                    }
                    if terms.len() == 1 {
                        continue;
                    }
                    rows.leq(terms, 0.0, &format!("srcused_{}_{}_{k}_{segment}", loader.id.0, pile.0));
                    chosen.push((flag, 1.0));
                }
                if chosen.len() > 1 {
                    rows.leq(chosen, 1.0, &format!("onesrc_{}_{k}_{segment}", loader.id.0));
                }
            }
        }
    }

    // The objective - movement value, maximised - is carried on the movement
    // columns themselves; see [`Rows::valued`].
    Ok(())
}

/// The most one pile can supply in one interval, from physics rather than
/// from a constant.
///
/// The smaller of what it could possibly hold and what every loader routed
/// against it could physically lift in that interval. Used to scale the
/// conditional grade rows below, where a loose bound would let a binary's own
/// integrality tolerance admit material the condition should have excluded.
fn reclaim_ceiling(input: &BlendInput, pile: StockpileId, interval: usize, segments: usize) -> f64 {
    let capacity = input
        .piles
        .iter()
        .find(|entry| entry.id == pile)
        .map(|entry| entry.capacity_t.max(entry.opening_t))
        .unwrap_or(0.0);
    let duration = input.intervals.get(interval).map(|entry| entry.duration_h()).unwrap_or(0.0);
    let reachable: f64 = input
        .movements
        .iter()
        .filter(|candidate| candidate.activity == Activity::Reclaim && candidate.source == SourceId::Stockpile(pile))
        .map(|candidate| loader_rate(input, candidate, interval).unwrap_or(0.0) * duration * segments as f64)
        .sum();
    capacity.min(reachable).max(0.0)
}

/// Which dig columns authored block order already forces to zero.
///
/// Loader `L` may dig block `b` in a cell only once every block authored
/// immediately ahead of `b` in `L`'s sequences is exhausted by the end of the
/// previous cell (the `order` rows). A block cannot be exhausted before the
/// loaders able to dig it could have lifted its tonnes, and each of those
/// loaders can only start on it once *its own* predecessors are exhausted.
/// That recursion gives a lower bound on every block's exhaustion time, and a
/// column in an interval that ends before some predecessor's bound is zero in
/// every feasible schedule. Omitting it therefore removes no schedule.
///
/// The bound is deliberately optimistic wherever it simplifies: bar windows
/// are ignored, a loader's capacity is counted in full for every block it
/// could dig, and the within-interval arrival of capacity is taken at the
/// interval's fastest combined rate. A tolerance covers the solver's own
/// feasibility slack on "exhausted". Each of these only keeps more columns.
struct DigReach {
    /// Keyed (loader, block): the blocks authored immediately ahead of it.
    predecessors: BTreeMap<(LoaderId, GroundId), Vec<GroundId>>,
    /// Lower bound on the hour each block can be exhausted; infinite when no
    /// loader could remove it within the horizon.
    exhausted_from_h: BTreeMap<GroundId, f64>,
}

/// Relative and absolute slack on "exhausted", comfortably above the
/// solver's integrality and feasibility tolerances on the `exhlink` row.
const REACH_RELATIVE_SLACK: f64 = 1e-5;
const REACH_ABSOLUTE_SLACK_T: f64 = 1e-3;
const REACH_TIME_SLACK_H: f64 = 1e-9;

impl DigReach {
    fn new(input: &BlendInput) -> Self {
        let mut predecessors: BTreeMap<(LoaderId, GroundId), Vec<GroundId>> = BTreeMap::new();
        let mut authorised: BTreeSet<(LoaderId, GroundId)> = BTreeSet::new();
        for task in &input.tasks {
            let TaskKind::Dig { sequence } = &task.kind else { continue };
            authorised.extend(sequence.iter().map(|ground| (task.loader, *ground)));
            for pair in sequence.windows(2) {
                // A predecessor the capture did not include has no order row.
                if input.ground.iter().any(|source| source.id == pair[0]) {
                    predecessors.entry((task.loader, pair[1])).or_default().push(pair[0]);
                }
            }
        }
        let mut diggers: BTreeMap<GroundId, Vec<usize>> = BTreeMap::new();
        for candidate in &input.movements {
            let (Activity::Dig, SourceId::Ground(ground)) = (candidate.activity, candidate.source) else {
                continue;
            };
            if !authorised.contains(&(candidate.loader, ground)) {
                continue;
            }
            let Some(loader_index) = input.loaders.iter().position(|loader| loader.id == candidate.loader) else {
                continue;
            };
            let entry = diggers.entry(ground).or_default();
            if !entry.contains(&loader_index) {
                entry.push(loader_index);
            }
        }

        let mut exhausted_from_h: BTreeMap<GroundId, f64> = input.ground.iter().map(|source| (source.id, 0.0)).collect();
        // Every pass keeps each figure a valid lower bound, so stopping early
        // (an authored cycle) is safe; an acyclic order settles well within
        // one pass per block.
        for _ in 0..=input.ground.len() {
            let mut changed = false;
            for source in &input.ground {
                let starts: Vec<(usize, f64)> = diggers
                    .get(&source.id)
                    .map(|loaders| {
                        loaders
                            .iter()
                            .map(|&loader_index| {
                                let loader = input.loaders[loader_index].id;
                                let start = predecessors
                                    .get(&(loader, source.id))
                                    .map(|ahead| ahead.iter().map(|block| exhausted_from_h[block]).fold(0.0, f64::max))
                                    .unwrap_or(0.0);
                                (loader_index, start)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let bound = earliest_removal_h(input, &starts, source.tonnes_t);
                if bound > exhausted_from_h[&source.id] {
                    exhausted_from_h.insert(source.id, bound);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Self { predecessors, exhausted_from_h }
    }

    /// Whether a movement column can be nonzero in this interval.
    fn allows(&self, candidate: &MovementCandidate, interval: Interval) -> bool {
        let (Activity::Dig, SourceId::Ground(ground)) = (candidate.activity, candidate.source) else {
            return true;
        };
        let Some(ahead) = self.predecessors.get(&(candidate.loader, ground)) else { return true };
        ahead
            .iter()
            .all(|block| self.exhausted_from_h.get(block).is_none_or(|&from| from <= interval.end_h + REACH_TIME_SLACK_H))
    }
}

/// A lower bound on when `tonnes` can have been removed by these loaders,
/// each starting at its own hour. Infinite when the horizon is too short.
fn earliest_removal_h(input: &BlendInput, starts: &[(usize, f64)], tonnes: f64) -> f64 {
    let need = tonnes * (1.0 - REACH_RELATIVE_SLACK) - REACH_ABSOLUTE_SLACK_T;
    if need <= 0.0 {
        return 0.0;
    }
    let mut removed = 0.0;
    for interval in &input.intervals {
        let mut through_end = 0.0;
        let mut fastest = 0.0;
        for &(loader_index, start) in starts {
            let rate = interval_rate(&input.loaders[loader_index], interval.index).map(|rate| rate.dig_tph).unwrap_or(0.0);
            if rate <= 0.0 || start >= interval.end_h {
                continue;
            }
            through_end += rate * (interval.end_h - start.max(interval.start_h));
            fastest += rate;
        }
        if removed + through_end >= need && fastest > 0.0 {
            // Before the interval `removed < need`; no loader can do better
            // than every one of them running from the interval's start.
            return (interval.start_h + (need - removed) / fastest).min(interval.end_h);
        }
        removed += through_end;
    }
    f64::INFINITY
}

/// `flag = 1  =>  the interval's blend satisfies this half-space`.
///
/// One-way, and tightened by [`GRADE_MARGIN`] so the solver's own feasibility
/// slack eats the cushion rather than the authored boundary. Inclusive and
/// exclusive endpoints take the same margin: either way the boundary value
/// itself lies inside the cushion, so the operator only decides which side of
/// the authored value the replay counts it on.
///
/// Written on the cleared form - `Q` against `threshold x T` - because the
/// blend itself is a ratio the model must not divide. The cleared form is
/// checked by the solver in *contained tonnes*, not in grade, so a fraction
/// margin alone stops protecting a small reclaim: at `T = 0.5 t` a solver
/// slack of 1e-6 t of component is 2e-6 of grade, twice the margin. Each test
/// therefore also clears the boundary by [`GRADE_CUSHION_T`] of contained
/// quantity, which a solver's slack cannot eat whatever `T` is. The price is a
/// band of `GRADE_CUSHION_T / T` in grade - negligible above a few tonnes -
/// and that no test can be claimed on an interval with no reclaim at all.
#[allow(clippy::too_many_arguments)]
fn implies_half_space<R: Rows>(rows: &mut R, flag: R::Var, recl_q: R::Var, recl_t: R::Var, test: GradeHalfSpace, ceiling: f64, ceiling_t: f64, name: &str) {
    let margin = GRADE_MARGIN;
    if test.above {
        // flag = 1  =>  Q - (v + margin) T >= cushion
        let threshold = test.endpoint.value + margin;
        let big_m = (threshold.abs().max(1.0) * ceiling_t).max(1.0) + GRADE_CUSHION_T;
        rows.implies(flag, vec![(recl_q, -1.0), (recl_t, threshold)], -GRADE_CUSHION_T, big_m, name);
    } else {
        // flag = 1  =>  Q - (v - margin) T <= -cushion
        let threshold = test.endpoint.value - margin;
        let big_m = ((ceiling + threshold.abs()).max(1.0) * ceiling_t).max(1.0) + GRADE_CUSHION_T;
        rows.implies(flag, vec![(recl_q, 1.0), (recl_t, -threshold)], -GRADE_CUSHION_T, big_m, name);
    }
}

/// A binary that may be 1 **only when** every one of these half-spaces holds
/// on the interval's blend, tightened inward by [`GRADE_MARGIN`].
///
/// One-way on purpose: it gates a reward, and a reward the optimiser does
/// not claim is merely value left on the table - the replay publishes what
/// the authored rule actually pays. Returns `None` when the pile has no
/// reclaim columns in this interval.
fn predicate_earned<R: Rows>(rows: &mut R, pile: StockpileId, interval: usize, tests: &[GradeHalfSpace], ceilings: &[f64], ceiling_t: f64, name: &str) -> Option<R::Var> {
    let recl_t = rows.columns().recl_t.get(&(pile, interval)).cloned()?;
    let earned = rows.binary(name);
    for (index, test) in tests.iter().enumerate() {
        let Some(recl_q) = rows.columns().recl_q.get(&(pile, interval, test.grade)).cloned() else {
            continue;
        };
        let ceiling = ceilings.get(test.grade).copied().unwrap_or(1.0);
        implies_half_space(rows, earned.clone(), recl_q, recl_t.clone(), *test, ceiling, ceiling_t, &format!("{name}_on{index}"));
    }
    Some(earned)
}

/// A binary that must be 1 **unless** some half-space is broken on the
/// interval's blend by at least [`GRADE_MARGIN`].
///
/// The mirror of [`predicate_earned`], for a cost: the optimiser can only set
/// it to zero by exhibiting a bound the blend clearly fails, so a matching
/// negative rule cannot be declined. A blend within a margin of every
/// failing boundary owes the cost in the model; the replay publishes what the
/// authored rule actually charges.
fn predicate_owed<R: Rows>(rows: &mut R, pile: StockpileId, interval: usize, tests: &[GradeHalfSpace], ceilings: &[f64], ceiling_t: f64, name: &str) -> Option<R::Var> {
    let recl_t = rows.columns().recl_t.get(&(pile, interval)).cloned()?;
    let owed = rows.binary(name);
    // owes + sum(fails) >= 1: with no failure exhibited the cost applies.
    let mut cover = vec![(owed.clone(), 1.0)];
    for (index, test) in tests.iter().enumerate() {
        let Some(recl_q) = rows.columns().recl_q.get(&(pile, interval, test.grade)).cloned() else {
            continue;
        };
        let ceiling = ceilings.get(test.grade).copied().unwrap_or(1.0);
        let fails = rows.binary(&format!("{name}_f{index}"));
        implies_half_space(
            rows,
            fails.clone(),
            recl_q,
            recl_t.clone(),
            test.negated(),
            ceiling,
            ceiling_t,
            &format!("{name}_off{index}"),
        );
        cover.push((fails, 1.0));
    }
    rows.geq(cover, 1.0, &format!("{name}_owed"));
    Some(owed)
}

/// §8 - the chunked blended pile.
///
/// # Lifecycle
///
/// These are the experimental assumptions, not approved application
/// behaviour:
///
/// - **Capacity** is fixed and authored per chunk.
/// - **Fill order** is sequential: chunk `c + 1` receives nothing until `c`
///   is closed.
/// - A chunk **becomes reclaimable** when it is closed to further receipts.
/// - A **partially filled** chunk may be closed at a calendar boundary; that
///   is what `closed` being a free binary per interval expresses.
/// - A chunk **may not receive while being reclaimed**: receipts require
///   `not closed`, reclaim requires `closed`.
/// - **FIFO/LIFO** applies among released, non-empty chunks.
/// - An **emptied** chunk stays empty, and its **slot is not reused** within
///   the horizon - there is no row that reopens a closed chunk.
///
/// **Opening stock** occupies chunk 0, which is closed from the start, so an
/// authored opening is immediately reclaimable and receipts begin at chunk 1.
/// When a pile opens empty, chunk 0 is an ordinary fill target instead. A
/// day-by-day window opens in the state the day before left: any chunk may
/// hold material, and whether it is closed is stated rather than inferred.
///
/// The throughput consequence of "no slot reuse" is real and worth stating:
/// total material the pile can pass over the whole horizon is bounded by
/// `sum(chunk capacities)` plus opening stock, so a long horizon with few
/// chunks is limited by the model rather than by the equipment.
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
fn chunked_pile<R: Rows>(
    rows: &mut R,
    input: &BlendInput,
    pile: &BlendPile,
    grades: usize,
    ceilings: &[f64],
    segments: usize,
    destinations: &BTreeMap<DestinationId, &Destination>,
    receipt_peak: bool,
) -> Result<(), FormulationCancelled> {
    let count = pile.chunks.len();
    let horizon = input.intervals.len();

    // Opening material per chunk. When the caller authored none, the pile's
    // single opening figure goes into chunk 0, which is the unchunked
    // model's behaviour carried over.
    let opening: Vec<(f64, Vec<f64>)> = if pile.chunk_opening.is_empty() {
        (0..count)
            .map(|c| if c == 0 { (pile.opening_t, pile.opening_q.clone()) } else { (0.0, vec![0.0; grades]) })
            .collect()
    } else {
        pile.chunk_opening.clone()
    };

    // Movements that deliver into this pile, and reclaim candidates from it.
    let delivering: Vec<usize> = input
        .movements
        .iter()
        .enumerate()
        .filter(|(_, candidate)| delivers_to_pile(candidate, pile.id, destinations))
        .map(|(index, _)| index)
        .collect();

    // Per chunk and interval state.
    let mut open_t = vec![vec![None; horizon]; count];
    let mut open_q = vec![vec![vec![None; horizon]; grades]; count];
    let mut recl_t = vec![vec![None; horizon]; count];
    let mut recl_q = vec![vec![vec![None; horizon]; grades]; count];
    let mut closed = vec![vec![None; horizon]; count];
    let mut empty = vec![vec![None; horizon]; count];
    // Receipts of movement m into chunk c during interval k.
    let mut recv: BTreeMap<(usize, usize, usize), R::Var> = BTreeMap::new();

    for c in 0..count {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        let cap = pile.chunks[c];
        for k in 0..horizon {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            open_t[c][k] = Some(rows.cont(cap, &format!("cOpenT_{}_{c}_{k}", pile.id.0)));
            for g in 0..grades {
                open_q[c][g][k] = Some(rows.cont(cap * ceilings[g], &format!("cOpenQ_{}_{c}_{k}_{g}", pile.id.0)));
                recl_q[c][g][k] = Some(rows.cont(cap * ceilings[g], &format!("cReclQ_{}_{c}_{k}_{g}", pile.id.0)));
            }
            recl_t[c][k] = Some(rows.cont(cap, &format!("cReclT_{}_{c}_{k}", pile.id.0)));
            closed[c][k] = Some(rows.binary(&format!("cClosed_{}_{c}_{k}", pile.id.0)));
            empty[c][k] = Some(rows.binary(&format!("cEmpty_{}_{c}_{k}", pile.id.0)));
            rows.columns().chunk_open_t.insert((pile.id, c, k), open_t[c][k].clone().expect("just created"));
            rows.columns().chunk_recl_t.insert((pile.id, c, k), recl_t[c][k].clone().expect("just created"));
            rows.columns().chunk_closed.insert((pile.id, c, k), closed[c][k].clone().expect("just created"));
            for &m in &delivering {
                let column = rows.cont(cap, &format!("cRecv_{}_{c}_{k}_{m}", pile.id.0));
                rows.columns().chunk_recv.insert((pile.id, c, k, m), column.clone());
                recv.insert((m, c, k), column);
            }
        }
    }

    let take = |slot: &Option<R::Var>| slot.clone().expect("chunk column");

    // Tightest physical bounds available for the lifecycle indicator rows.
    //
    // A binary may sit one integrality tolerance away from zero, so every
    // `x <= M * indicator` row admits `M * tolerance` through an indicator
    // that reads as off. Using the chunk's capacity as `M` makes that leak as
    // large as the tolerance allows; using what can physically move in one
    // interval makes it as small as the model can make it. The replay
    // publishes whatever still leaks rather than hiding it - see
    // `replay::ReplayReport::chunk_dust_tonnes_t`.
    let interval_receipt: Vec<f64> = input
        .intervals
        .iter()
        .map(|interval| {
            delivering
                .iter()
                .filter_map(|&m| loader_rate(input, &input.movements[m], interval.index))
                .map(|rate| rate * interval.duration_h())
                .fold(0.0_f64, f64::max)
        })
        .collect();
    let interval_draw: Vec<f64> = input
        .intervals
        .iter()
        .map(|interval| {
            input
                .movements
                .iter()
                .filter(|candidate| candidate.activity == Activity::Reclaim && candidate.source == SourceId::Stockpile(pile.id))
                .filter_map(|candidate| loader_rate(input, candidate, interval.index))
                .map(|rate| rate * interval.duration_h())
                .sum()
        })
        .collect();

    for c in 0..count {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        let cap = pile.chunks[c];
        for k in 0..horizon {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let open = take(&open_t[c][k]);
            let close = take(&closed[c][k]);
            let void = take(&empty[c][k]);
            let draw = take(&recl_t[c][k]);

            // A chunk holding authored opening material is closed from the
            // start, so it is immediately reclaimable and the authored order
            // has something to choose between. A chunk that opens empty is an
            // ordinary fill target. A day-by-day window says instead which
            // chunks the day before left closed; see `BlendPile::chunk_closed`.
            if pile.chunk_starts_closed(c) {
                rows.eq(vec![(close.clone(), 1.0)], 1.0, &format!("cOpenFull_{}_{c}_{k}", pile.id.0));
            }

            // Sequential fill also orders *closing*: a chunk cannot be
            // closed before the one in front of it. Without this the solver
            // could close an empty chunk `c` while `c - 1` was still open
            // and start filling `c + 1`, which passes the per-pair receipt
            // rule below but breaks sequential fill outright. The replay
            // found exactly that on the dynamic LIFO fixture.
            if c > 0 {
                let previous = take(&closed[c - 1][k]);
                rows.leq(vec![(close.clone(), 1.0), (previous, -1.0)], 0.0, &format!("cCloseSeq_{}_{c}_{k}", pile.id.0));
            }

            // Closing is monotone: a closed chunk never reopens, so a slot is
            // never reused.
            if k + 1 < horizon {
                let next = take(&closed[c][k + 1]);
                rows.leq(vec![(close.clone(), 1.0), (next, -1.0)], 0.0, &format!("cMono_{}_{c}_{k}", pile.id.0));
            }

            // Opening state.
            if k == 0 {
                let value = opening.get(c).map(|(tonnes, _)| *tonnes).unwrap_or(0.0);
                rows.eq(vec![(open.clone(), 1.0)], value, &format!("cT0_{}_{c}", pile.id.0));
                for g in 0..grades {
                    let value = opening.get(c).and_then(|(_, contained)| contained.get(g).copied()).unwrap_or(0.0);
                    rows.eq(vec![(take(&open_q[c][g][0]), 1.0)], value, &format!("cQ0_{}_{c}_{g}", pile.id.0));
                }
            } else {
                let prior = take(&open_t[c][k - 1]);
                let prior_draw = take(&recl_t[c][k - 1]);
                let mut terms = vec![(open.clone(), 1.0), (prior, -1.0), (prior_draw, 1.0)];
                for &m in &delivering {
                    terms.push((recv[&(m, c, k - 1)].clone(), -1.0));
                }
                rows.eq(terms, 0.0, &format!("cTbal_{}_{c}_{k}", pile.id.0));

                for g in 0..grades {
                    let prior = take(&open_q[c][g][k - 1]);
                    let prior_draw = take(&recl_q[c][g][k - 1]);
                    let mut terms = vec![(take(&open_q[c][g][k]), 1.0), (prior, -1.0), (prior_draw, 1.0)];
                    for &m in &delivering {
                        let Some(fraction) = input.grades.fraction(input.movements[m].material, g) else {
                            continue;
                        };
                        terms.push((recv[&(m, c, k - 1)].clone(), -fraction));
                    }
                    rows.eq(terms, 0.0, &format!("cQbal_{}_{c}_{k}_{g}", pile.id.0));
                }
            }

            // Receipts require an OPEN chunk, and sequential fill requires the
            // previous one to be closed.
            let receipt_m = cap.min(interval_receipt[k]).max(0.0);
            let draw_m = cap.min(interval_draw[k]).max(0.0);
            for &m in &delivering {
                let column = recv[&(m, c, k)].clone();
                rows.leq(
                    vec![(column.clone(), 1.0), (close.clone(), receipt_m)],
                    receipt_m,
                    &format!("cRecvOpen_{}_{c}_{k}_{m}", pile.id.0),
                );
                if c > 0 {
                    let previous = take(&closed[c - 1][k]);
                    rows.leq(vec![(column, 1.0), (previous, -receipt_m)], 0.0, &format!("cRecvSeq_{}_{c}_{k}_{m}", pile.id.0));
                }
            }

            // Reclaim requires a CLOSED chunk and cannot exceed its released
            // opening tonnes.
            rows.leq(vec![(draw.clone(), 1.0), (close.clone(), -draw_m)], 0.0, &format!("cDrawClosed_{}_{c}_{k}", pile.id.0));
            rows.leq(vec![(draw.clone(), 1.0), (open.clone(), -1.0)], 0.0, &format!("cDrawCap_{}_{c}_{k}", pile.id.0));

            // Emptiness indicator: open tonnes > 0 forces `empty` to 0.
            rows.leq(vec![(open.clone(), 1.0), (void.clone(), cap)], cap, &format!("cEmptyLink_{}_{c}_{k}", pile.id.0));

            // Per-chunk perfect mixing, with the same grade-box rows that make
            // it sound at zero inventory.
            for g in 0..grades {
                let q_open = take(&open_q[c][g][k]);
                let q_draw = take(&recl_q[c][g][k]);
                // Each chunk mixes on its own account: the estimate the
                // iterative method uses is per chunk, keyed by giving the
                // chunk its own grade slot.
                rows.mix(
                    pile.id,
                    chunk_key(c, k, horizon),
                    g,
                    q_draw.clone(),
                    open.clone(),
                    draw.clone(),
                    q_open.clone(),
                    &format!("cMix_{}_{c}_{k}_{g}", pile.id.0),
                );
                rows.leq(vec![(q_draw, 1.0), (draw.clone(), -ceilings[g])], 0.0, &format!("cDrawBox_{}_{c}_{k}_{g}", pile.id.0));
                rows.leq(vec![(q_open, 1.0), (open.clone(), -ceilings[g])], 0.0, &format!("cOpenBox_{}_{c}_{k}_{g}", pile.id.0));
            }

            // Authored order among released, non-empty chunks.
            match pile.order {
                ReclaimOrder::Fifo => {
                    // Every older chunk must be empty first.
                    for j in 0..c {
                        let older = take(&empty[j][k]);
                        rows.leq(vec![(draw.clone(), 1.0), (older, -draw_m)], 0.0, &format!("cFifo_{}_{c}_{j}_{k}", pile.id.0));
                    }
                }
                ReclaimOrder::Lifo => {
                    // Every newer chunk must be empty OR not yet released.
                    for j in (c + 1)..count {
                        let newer = take(&empty[j][k]);
                        let released = take(&closed[j][k]);
                        // draw <= draw_m * (empty_j + 1 - closed_j)
                        rows.leq(
                            vec![(draw.clone(), 1.0), (newer, -draw_m), (released, draw_m)],
                            draw_m,
                            &format!("cLifo_{}_{c}_{j}_{k}", pile.id.0),
                        );
                    }
                }
            }
        }

        // A chunk never receives more than its authored capacity.
        let mut fill = Vec::new();
        for k in 0..horizon {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            for &m in &delivering {
                fill.push((recv[&(m, c, k)].clone(), 1.0));
            }
        }
        if !fill.is_empty() {
            rows.leq(fill, cap, &format!("cFill_{}_{c}", pile.id.0));
        }
    }

    // The pile's own capacity still binds. Per-chunk capacities alone do not
    // imply it: authored chunks can over-subscribe the pile, and without this
    // the model would happily hold more material than the pile can take. The
    // replay found exactly that on a ten-chunk fixture.
    //
    // The occupancy definition is shared with the unchunked pile, with the
    // chunk columns summed to give the interval's opening tonnage. Receipts
    // and reclaim are read off the segment-resolved *movement* columns rather
    // than the per-chunk splits, so chunking costs no extra segment-indexed
    // variables here.
    for k in 0..horizon {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        let opening: Vec<(R::Var, f64)> = (0..count).map(|c| (take(&open_t[c][k]), 1.0)).collect();
        occupancy_rows(rows, input, pile, k, segments, destinations, opening, receipt_peak);
    }

    // Tie each delivering movement's chunk split to its own tonnage, and
    // publish pile-level reclaim aggregates so the rest of the model - the
    // movement sum, reclaim caps and grade limits - is unchanged by chunking.
    for k in 0..horizon {
        if rows.cancelled() {
            return Err(FormulationCancelled);
        }
        for &m in &delivering {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let mut terms: Vec<(R::Var, f64)> = (0..count).map(|c| (recv[&(m, c, k)].clone(), 1.0)).collect();
            for segment in 0..segments {
                if let Some(column) = rows.columns().movement.get(&(m, k, segment)).cloned() {
                    terms.push((column, -1.0));
                }
            }
            rows.eq(terms, 0.0, &format!("cSplit_{}_{k}_{m}", pile.id.0));
        }

        let pile_draw = rows.cont(pile.capacity_t, &format!("reclT_{}_{k}", pile.id.0));
        rows.columns().recl_t.insert((pile.id, k), pile_draw.clone());
        let mut terms = vec![(pile_draw, 1.0)];
        for c in 0..count {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            terms.push((take(&recl_t[c][k]), -1.0));
        }
        rows.eq(terms, 0.0, &format!("cDrawSum_{}_{k}", pile.id.0));

        for g in 0..grades {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            let pile_q = rows.cont(pile.capacity_t * ceilings[g], &format!("reclQ_{}_{k}_{g}", pile.id.0));
            rows.columns().recl_q.insert((pile.id, k, g), pile_q.clone());
            let mut terms = vec![(pile_q, 1.0)];
            for c in 0..count {
                terms.push((take(&recl_q[c][g][k]), -1.0));
            }
            rows.eq(terms, 0.0, &format!("cDrawQSum_{}_{k}_{g}", pile.id.0));
        }

        // The interval's reclaim total is still the sum of its reclaim
        // movements from this pile.
        let pile_draw = rows.columns().recl_t[&(pile.id, k)].clone();
        let mut terms = vec![(pile_draw, 1.0)];
        for (index, candidate) in input.movements.iter().enumerate() {
            if rows.cancelled() {
                return Err(FormulationCancelled);
            }
            if candidate.activity != Activity::Reclaim || candidate.source != SourceId::Stockpile(pile.id) {
                continue;
            }
            for segment in 0..segments {
                if let Some(column) = rows.columns().movement.get(&(index, k, segment)).cloned() {
                    terms.push((column, -1.0));
                }
            }
        }
        rows.eq(terms, 0.0, &format!("cReclTsum_{}_{k}", pile.id.0));
    }
    Ok(())
}

/// Physical occupancy rows for one pile and interval (§3).
///
/// # What occupancy is, and why it is not the released inventory
///
/// Three quantities were previously conflated, and separating them is the
/// correction this function exists for:
///
/// - **Physical occupancy** - everything actually sitting on the pile. New
///   receipts count the moment they land.
/// - **Released inventory** - what a reclaim in this interval may draw on.
///   Under boundary mixing that is the interval's opening tonnage only, and
///   it is enforced separately by the `reclcap` row.
/// - **Blend composition** - the released opening blend, which the bilinear
///   mixing rows carry.
///
/// At every shared execution-segment boundary `s`:
///
/// ```text
/// occupancy(s) = T_open[p, k]
///              + sum of receipts in segments 0..=s
///              - sum of reclaim   in segments 0..=s
/// 0 <= occupancy(s) <= capacity
/// ```
///
/// **Why endpoints are enough.** Within one execution segment every loader
/// runs at a constant rate, so both cumulative receipts and cumulative
/// reclaim are linear in time and occupancy is therefore linear across the
/// segment's interior. A linear function on an interval attains its extremes
/// at the endpoints, so bounding occupancy at every segment boundary bounds
/// it throughout. This is why the segment grid has to be the *shared* event
/// clock: a source transition that is a boundary for one loader but not for
/// another would break the constant-rate premise.
///
/// **Blocks worked back to back.** A loader that works several blocks of its
/// sequence inside one segment delivers each block's tonnes in turn, so its
/// receipts are not at a constant rate and occupancy can peak inside the
/// segment. Where that can happen (`receipt_peak`), a pile that is also
/// reclaimed is checked conservatively as well, with no credit for the
/// segment's own reclaim:
///
/// ```text
/// occupancy(s - 1) + receipts(s) <= capacity
/// ```
///
/// A pile nothing reclaims only fills inside a segment, so its endpoint is
/// already its peak.
///
/// The previous revision omitted the reclaim term. That made the row an upper
/// bound on true occupancy rather than the occupancy itself, and it refused
/// valid schedules: a pile that opens at capacity could not receive anything,
/// however much of its released stock was being reclaimed at the same time.
///
/// `opening` is the pile's opening tonnage expression - one column for an
/// unchunked pile, the sum of the chunk columns for a chunked one - so both
/// variants share exactly one definition of occupancy.
#[allow(clippy::too_many_arguments)]
fn occupancy_rows<R: Rows>(
    rows: &mut R,
    input: &BlendInput,
    pile: &BlendPile,
    interval: usize,
    segments: usize,
    destinations: &BTreeMap<DestinationId, &Destination>,
    opening: Vec<(R::Var, f64)>,
    receipt_peak: bool,
) {
    // Carried as a running occupancy column per segment rather than restating
    // every earlier segment's flows in each row: the column's own bounds are
    // the capacity and nonnegative-occupancy rows, and each balance row holds
    // one segment's flows, so the rows stay linear in segments instead of
    // quadratic.
    let mut previous = opening;
    for segment in 0..segments {
        let occupancy = rows.cont(pile.capacity_t, &format!("occ_{}_{interval}_{segment}", pile.id.0));
        let mut terms = vec![(occupancy.clone(), -1.0)];
        let previous_len = previous.len();
        let mut peak = if receipt_peak { previous.clone() } else { Vec::new() };
        terms.extend(previous);
        for (index, candidate) in input.movements.iter().enumerate() {
            let receipt = delivers_to_pile(candidate, pile.id, destinations);
            let draw = candidate.activity == Activity::Reclaim && candidate.source == SourceId::Stockpile(pile.id);
            if !receipt && !draw {
                continue;
            }
            // A movement cannot be both, but if a scenario ever routed a
            // pile's reclaim back to itself the two coefficients would
            // correctly cancel.
            let mut coefficient = 0.0;
            if receipt {
                coefficient += 1.0;
            }
            if draw {
                coefficient -= 1.0;
            }
            if coefficient == 0.0 {
                continue;
            }
            if let Some(column) = rows.columns().movement.get(&(index, interval, segment)).cloned() {
                if receipt_peak && coefficient > 0.0 {
                    peak.push((column.clone(), coefficient));
                }
                terms.push((column, coefficient));
            }
        }
        if peak.len() > previous_len {
            rows.leq(peak, pile.capacity_t, &format!("occpeak_{}_{interval}_{segment}", pile.id.0));
        }
        // occupancy(s) = occupancy(s - 1) + receipts(s) - reclaim(s), with
        // 0 <= occupancy(s) <= capacity carried by the column bounds.
        // Nonnegativity is also implied by `reclcap` plus nonnegative
        // receipts, but the brief asks for it explicitly.
        rows.eq(terms, 0.0, &format!("occbal_{}_{interval}_{segment}", pile.id.0));
        previous = vec![(occupancy, 1.0)];
    }
}

/// Piles whose occupancy can peak inside a segment: they are reclaimed, and
/// they receive from a loader that can work two blocks of one sequence back
/// to back within a segment.
fn piles_with_receipt_peaks(input: &BlendInput, destinations: &BTreeMap<DestinationId, &Destination>) -> BTreeSet<StockpileId> {
    input
        .piles
        .iter()
        .filter(|pile| {
            let reclaimed = input
                .movements
                .iter()
                .any(|candidate| candidate.activity == Activity::Reclaim && candidate.source == SourceId::Stockpile(pile.id));
            reclaimed
                && input.movements.iter().any(|candidate| {
                    let SourceId::Ground(ground) = candidate.source else { return false };
                    candidate.activity == Activity::Dig
                        && delivers_to_pile(candidate, pile.id, destinations)
                        && input
                            .tasks
                            .iter()
                            .any(|task| task.loader == candidate.loader && matches!(&task.kind, TaskKind::Dig { sequence } if sequence.len() > 1 && sequence.contains(&ground)))
                })
        })
        .map(|pile| pile.id)
        .collect()
}

/// The columns that make up a pile's opening tonnage in an interval: one
/// column for an unchunked pile, one per chunk for a chunked one.
fn pile_opening_terms<R: Rows>(rows: &mut R, pile: &BlendPile, interval: usize) -> Vec<(R::Var, f64)> {
    if pile.chunks.is_empty() {
        return rows
            .columns()
            .open_t
            .get(&(pile.id, interval))
            .map(|column| vec![(column.clone(), 1.0)])
            .unwrap_or_default();
    }
    (0..pile.chunks.len())
        .filter_map(|chunk| rows.columns().chunk_open_t.get(&(pile.id, chunk, interval)).cloned())
        .map(|column| (column, 1.0))
        .collect()
}

/// Model size of one formulation family: every column or row whose name
/// starts with the same prefix before its first `_`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FamilySize {
    pub(crate) columns: usize,
    pub(crate) binaries: usize,
    pub(crate) rows: usize,
    pub(crate) entries: usize,
}

/// A sink that builds no solver model and only tallies what [`formulate`]
/// posts, by family. It exists to answer "which part of the model is large"
/// on a real project without paying for a solver build.
struct FamilyRows {
    columns: BlendColumns<()>,
    sizes: BlendSizes,
    families: BTreeMap<String, FamilySize>,
}

impl FamilyRows {
    fn family(&mut self, name: &str) -> &mut FamilySize {
        let prefix = name.split('_').next().unwrap_or(name);
        if !self.families.contains_key(prefix) {
            self.families.insert(prefix.to_owned(), FamilySize::default());
        }
        self.families.get_mut(prefix).expect("just inserted")
    }
}

impl Rows for FamilyRows {
    type Var = ();

    fn columns(&mut self) -> &mut BlendColumns<()> {
        &mut self.columns
    }

    fn sizes(&mut self) -> &mut BlendSizes {
        &mut self.sizes
    }

    fn valued(&mut self, _upper: f64, _value: f64, name: &str) {
        self.family(name).columns += 1;
    }

    fn binary(&mut self, name: &str) {
        let family = self.family(name);
        family.columns += 1;
        family.binaries += 1;
    }

    fn linear(&mut self, terms: Vec<((), f64)>, _lhs: f64, _rhs: f64, name: &str) {
        if terms.is_empty() {
            return;
        }
        let family = self.family(name);
        family.rows += 1;
        family.entries += terms.iter().filter(|(_, coefficient)| *coefficient != 0.0).count();
    }

    fn implies(&mut self, _flag: (), terms: Vec<((), f64)>, _rhs: f64, _big_m: f64, name: &str) {
        self.linear(terms, 0.0, 0.0, name);
    }

    fn mix(&mut self, _pile: StockpileId, _interval: usize, _grade: usize, _recl_q: (), _open_t: (), _recl_t: (), _open_q: (), name: &str) {
        let family = self.family(name);
        family.rows += 1;
        family.entries += 4;
    }
}

/// Per-family model sizes, largest first by columns plus rows.
pub(crate) fn family_sizes(input: &BlendInput) -> Vec<(String, FamilySize)> {
    let mut rows = FamilyRows {
        columns: BlendColumns::new(),
        sizes: BlendSizes::default(),
        families: BTreeMap::new(),
    };
    formulate(&mut rows, input).expect("the counting sink never cancels");
    let mut families: Vec<(String, FamilySize)> = rows.families.into_iter().collect();
    families.sort_by_key(|(_, size)| std::cmp::Reverse(size.columns + size.rows));
    families
}

/// A source as it appears in a row name.
fn source_key(source: SourceId) -> String {
    match source {
        SourceId::Ground(ground) => format!("g{}", ground.0),
        SourceId::Stockpile(pile) => format!("p{}", pile.0),
    }
}

/// A chunked pile needs one grade estimate per *chunk* and interval, not just
/// per interval, so [`Rows::mix`] is keyed by a flattened `(chunk, interval)`
/// index. Unchunked piles pass the interval directly, and the two never
/// collide because a pile is either chunked or not.
pub(crate) fn chunk_key(chunk: usize, interval: usize, horizon: usize) -> usize {
    chunk * horizon.max(1) + interval
}
