# Desktop scheduling with SCIP

Run Period and Run All Periods use one path: owned project capture → SCIP →
extraction → independent physical/grade/value replay → immutable calculated
schedule. Gantt, Calendar and animation consume that same result. There is no
dispatcher or iterative-HiGHS fallback. WASM edits and saves settings, but does
not calculate schedules.

## Horizons and lifecycle

Every run starts at hour zero with authored opening inventory. Run Period
requests Day 1 when no current result exists, otherwise extends the current
horizon by one day, capped at Planning end day. It reoptimises earlier days.
Run All Periods requests the complete configured horizon. Empty days inside
that requested horizon are calculated, not missing data.

Semantic edits retire results and cancel pending work. Publication repeats
the currentness check. Cancellation, refusal, no incumbent and replay failure
publish nothing and preserve the prior result. Renames and solve-limit edits
do not change model semantics; shortening the horizon past a held result does.
The existing solver-thread interrupt handler and seeded-mode workaround remain.
A horizon longer than one day-by-day window is first solved a day at a time;
see [Day-by-day start](#day-by-day-start-for-long-horizons).

## Resolution and event capacity

Calendar interval sets input constancy and stockpile receipt release. Event
positions are shared, ordered segments inside each interval; their durations
are decision variables. Event capacity is an optional saved setting, 1–24.
Blank preserves the existing derived capacity, capped at 24. A lower explicit
cap can restrict source switches and change the best attainable schedule: it
is a modelling choice, not a solver-only speed knob. The run details state
when the derived requirement exceeds the selected cap. Changing it retires a
result. No automatic capacity reduction or gap relaxation is performed.

Movement columns outside all authorising task windows are omitted instead of
created and constrained to zero. This reduces model size without changing the
feasible schedules.

## Grade thresholds

Grades are internal mass fractions. Percent input divides values and
thresholds by 100; fraction input is unchanged. Authored bounds are preserved.
Replay uses only arithmetic slack δ = 1e-9 fraction (1e-7 percentage points).

| Authored comparison | Replay test, internal units | Conditional reward permission |
| --- | --- | --- |
| x ≥ b | x ≥ b − δ | Q − (b + m)T ≥ c |
| x > b | x > b + δ | Q − (b + m)T ≥ c |
| x ≤ b | x ≤ b + δ | Q − (b − m)T ≤ −c |
| x < b | x < b − δ | Q − (b − m)T ≤ −c |
| Range | Both authored endpoints, with their inclusivity | Both inward tests |

Here T is interval reclaim tonnes, Q contained quantity, m = 1e-6 fraction
(0.0001 percentage points), c = 1e-5 contained tonnes. The effective band is
m + c/T. A cost can be escaped only by proving a bound fails outward by that
same cushion; negation reverses direction and inclusivity. SCIP uses native
linear indicator constraints for these grade tests. Equality/unsupported
predicates are refused where capture cannot encode them.

Conditional cashflow has no forbidden boundary band: rewards can be
underclaimed and costs overcharged in the solver near thresholds. Published
cashflow uses the authored replay predicate. Details show affected tonnes,
the conservative value allowance, and the derived indicator leakage allowance.
Routing is different: destination admission still requires a conservatively
satisfied bound, so its safety band can restrict feasible delivery. Optimality
and gap describe the encoded conservative model, not a proof of optimality for
an exact discontinuous authored-value problem.

## Physical and presentation limitations

- Receipts occupy pile capacity immediately and become reclaimable at the next
  interval boundary. Unchunked reclaim carries that released opening blend.
- Stockpile capacity limits instantaneous occupancy, including opening stock;
  it does not limit lifetime receipts. Reclaim permits refill. Dump capacity
  still limits cumulative receipts, and crusher budgets limit daily throughput.
- Blended representation combines opening lots; FIFO/LIFO intent is not applied
  to a homogeneous blend. Ordered chunks preserve order, but emptied receiving
  slots are not reused during a horizon.
- Stockpile-to-stockpile rehandle is excluded with a capture note rather than
  silently assigning zero grades.
- Mixed dig blocks move in their measured proportions; opening stock is included
  in grade upper bounds, including reclaim-only projects.
- Gantt delivery merging preserves grade, value and truck-hour rates. Calendar
  splits movements at day boundaries. Cursor inventory integrates actual solved
  receipt/reclaim spans, including simultaneous flows. Only digging depletes
  ground geometry in animation.
- The objective is movement cashflow, without invented terminal stock values
  or production targets. Idle loaders can therefore be economically optimal.

## Build and distribution

`cargo run` uses exact russcip 0.10.0 and scip-sys 0.1.28 with bundled SCIP
10.0.2 / SoPlex 8.0.2 from scipoptsuite-deploy v0.12.0. Native system builds use
`--no-default-features --features scip-system` and `SCIPOPTDIR`; those versions
must be validated separately. `blend-experiment` enables developer comparisons.

The executable embeds the link directory plus `$ORIGIN` (Linux) or
`@executable_path` (macOS). Linux direct launch was checked outside cargo.
Ship the matching shared library beside a packaged executable; Windows needs
the DLL beside it or on PATH. The Linux bundle also requires libgfortran.so.5,
libquadmath, libstdc++, zlib and platform runtime libraries. Windows/macOS
installers and runtime dependency packaging are not verified.

See [backend notices and provenance](scip-backend-notices.md). A distribution
must carry all applicable dependency notices, not only SCIP's Apache licence.

## Verification

Native comparison/capture/worker tests and temporary publication tests passed:
27 passed, 8 ignored. Temporary tests covered a two-loader project through
Day 1 and Day 2, persisted event capacity and legacy defaults, dig-only
depletion, Calendar/value/truck/crusher reconciliation, and overlapping cursor
inventory across midnight. They are removed after validation under repo policy.
Native clippy and WASM checking passed. On DreamLand, the default 24-position
model (1,770,192 variables, 3,949,179 linear constraints) reached 60 seconds
without an incumbent. Explicit capacity 2 reduced it to 147,648 variables and
329,211 linear constraints, found an incumbent near 24 seconds, and published
a replay-valid schedule at the 60-second limit. Value: 1,092,454.41; reported
gap: 648,301.96%. This is evidence of successful integration, not acceptable
schedule quality or near-optimality. Gantt and Calendar displayed that result;
Day 1 dug 109.9 t / 7,661.1 t / 0 t across the three loaders, and later days
were blank. Animation scrubbing advanced the displayed solved time. Desktop
cancellation published nothing and retained the previously published Run 3
execution bands. The visual ground-depletion check is limited
by the overlaid surfaces in the example; dig-only depletion was checked in the
publication/animation-index test. No validation edits were
saved into DreamLand.omf.

The root relaxation/proof remains the next performance target. Investigate
tighter bounds and material/candidate aggregation that preserves rule/grade
equivalence before changing economic semantics, resolution or defaults. A
looser gap target alone cannot fix this enormous first-root bound. Terminal
values or production targets are separate planner decisions, not inferred here.

## First-stage optimiser audit and diagnostics

The first-stage correction removes a second, unintended stockpile constraint:
the destination loop formerly bounded all receipts over the horizon by storage
capacity. Both formulation and independent replay now leave that check to the
physical occupancy timeline. This changes the feasible set deliberately; it
does not alter release timing, homogeneous mixing, chunk non-reuse, economic
coefficients, resolution, or event capacity.

Every normal solve and developer blended scenario records `SolveDiagnostics`:

- First solver solution event and number of best-solution updates. These are
  SCIP incumbents, **not** independently validated schedules. Replay remains
  after solving and is still required for publication.
- Presolve time and rounds; transformed variable/constraint counts at first
  root focus, and separately final transformed counts/nonzeros.
- First completed initial-root-LP and cut-and-price LP events, and whether a
  root-node completion event (or subsequent non-root focus) was observed.
  Time/iteration-limited LP statuses do not count as completed LPs. A completed
  initial LP is not a completed root node. Models solved in presolve or without
  an LP can legitimately have absent root observations.
- Rows, columns and actual nonzeros in the first completed root LP snapshot;
  absent when there was no such snapshot. Original posted nonzero coefficient
  **entries** are counted separately: SCIP may merge repeated variables or
  introduce indicator slacks, so this is not claimed to be a canonical matrix
  nonzero count.
- Processed nodes and total/root LP iterations. Final status, objective, bound,
  gap, formulation/solve/extraction/replay timings remain in the run log.

No telemetry call constructs an LP or changes solver settings. LP-only getters
run synchronously in solving callbacks; post-solve statistics guard SCIP's
actual stage because a limit can return while presolve is still active.
Successful-result tooltips show a short progress summary, explicitly labelled
first *solver* incumbent, and limited runs describe unfinished root processing.
No-incumbent limited attempts also retain the root explanation in their details.

### Repeatable benchmark procedure

1. Build once with the same profile/backend/features for both revisions. Do not
   include compilation time in solver comparisons. Record compiler and SCIP
   versions, machine, model identity, horizon, interval, event capacity, time
   limit, gap target, pile representation and all authored coefficients.
2. Run individual developer fixtures serially (not concurrently). For example:

   ```sh
   cargo test --features blend-experiment known_blend_is_preserved -- --nocapture --test-threads=1
   cargo test --features blend-experiment empty_pile_and_release_timing -- --nocapture --test-threads=1
   cargo test --features blend-experiment chunks_fill_close_and_reclaim_from_empty -- --nocapture --test-threads=1
   ```

   Output includes `DIAGNOSTICS`, `SOLVE`, replay issues and residuals. Existing
   ignored comparison benchmarks are opt-in; they may run several long solves.
3. For a real-project run, start the already-built desktop binary with
   `INCLINE_SCIP_LOG=1`, open the same project, run Solids, and apply identical
   schedule settings without saving over the original project. Compare Run
   Period against Run Period, not against a differently covered horizon.
   `/usr/bin/time -v` can record process peak memory, but that includes rendering,
   project loading and Solids; it is not isolated solver memory. SCIP's progress
   output supplies its own memory/proof observations.
4. Repeat each timing case three times, serially. Report median and range,
   first-incumbent time, end-of-solve replay validation, objective/bound/gap,
   matrix sizes, LP iterations and root observations. Keep failures/no incumbent
   as results, rather than dropping them from averages. Count full wall time
   separately from SCIP's clock (presolve is inside its solve time).
5. The capacity correction changes feasible schedules. Use no-refill cases for
   unchanged-model performance comparisons. Treat refill cases as correctness
   comparisons, not as proof of a speed improvement. No new DreamLand timing
   result is claimed by this first-stage audit.

### Focused first-stage validation

Temporary adjacent checks (removed after running, per repository policy) used:

- A 1,000 t homogeneous pile, four one-hour intervals, alternating 1,000 t/h
  fill/reclaim windows, 2,000 t available ground, mining value -1/t and reclaim
  value +10/t. Reintroducing the former horizon receipts cap gives objective
  9,000; the corrected model gives 18,000 and ends empty. The corrected case
  had 44 variables, 79 linear rows, four nonlinear rows and 152 posted linear
  coefficient entries. Its observed root snapshot had 16 columns, 16 rows and
  36 nonzeros; these small-fixture numbers are not a DreamLand benchmark.
- A 1,500 t dump cap reduces that fixture's value to 13,500. A deliberately
  overfilled solution is independently rejected for physical occupancy.
- Reclaim-only Fe minimum and arsenic maximum: clean opening stock yields
  10,000; excessive arsenic allows no valued reclaim. Replaying the clean
  answer against contaminated stock raises grade issues.
- Multiple loaders sharing a one-hour truck budget per interval respect that
  shared budget and independently replay successfully.
- A small binary triangle model with presolve disabled exercises root LP
  observations and returns the known optimum 1. A zero-second limit records
  no completed root LP and does not perform invalid-stage statistics calls.

Existing scenarios also cover opening blend conservation, empty-pile release,
multi-grade partial reclaim, simultaneous receipt/reclaim, mandatory bar
priority, shared crusher budgets, and ordered chunk lifecycle. Capture checks
cover mixed-ground proportions and reconciliation against project figures.
Desktop visual verification and a new real-project benchmark remain separate
validation tasks; numerical checks do not stand in for those checks.

## Second-stage exact reductions

Four formulation changes remove columns and rows without changing which
schedules are feasible:

- **Reach pruning.** A lower bound on each block's exhaustion time is derived
  from authored order and the dig rates of every loader that could lift it
  (windows ignored, capacity counted optimistically, solver slack allowed
  for). Dig columns in intervals ending before a predecessor's bound are
  omitted, because the `order` rows force them to zero.
- **Ground state from first touch.** Remaining-tonnes, exhaustion and
  extraction columns begin in a block's first diggable interval; before that
  the block is a constant, untouched state, which readiness reads as "has
  work". Blocks no loader can reach in the horizon have no state.
- **Grouped linking rows.** `mvsel` and `order` post one row per (loader,
  source, cell) instead of one per candidate. The loader's `rate` row already
  caps the grouped sum, so integer solutions are unchanged and the LP
  relaxation is tighter.
- **Running occupancy.** One occupancy column per pile and segment, carrying
  the capacity and floor as bounds, replaces prefix sums that grew with
  segments squared.

With `INCLINE_SCIP_LOG=1` the worker also logs the captured structure (block
tonnage spread, sequence lengths, rates) and a per-family size table from a
counting pass that builds no solver model.

DreamLand, Run Period Day 1, 60 s, same machine and settings (one run each,
not the three-run protocol above):

| Event capacity | Model | Variables | Linear rows | Coefficient entries | Presolve | Outcome |
| --- | --- | --- | --- | --- | --- | --- |
| 24 (default) | before | 1,770,192 | 3,949,179 | ≈36.7M | 51.0 s | no incumbent |
| 24 (default) | after | 157,464 | 96,914 | ≈0.99M | 11.5 s | incumbent 26.5 s, value 1,092,454.41, root LP unfinished |
| 2 | before | 147,648 | 329,211 | — | — | value 1,092,454.41, gap 648,301.96% |
| 2 | after | 13,254 | 8,276 | 82,698 | 0.8 s | **optimal** 10,702,846.60 in 4.2 s, replay valid |

At 24 positions the remaining obstacle is the root LP itself (about 60k
iterations without completion), which points at the degeneracy of many
interchangeable free-duration segments rather than model size.

Grade limits and routing qualifications apply to a chunked pile's *interval
aggregate*, in both the formulation and the replay, by design: a small dirty
chunk may be blended out with the richer chunks drawn in the same interval,
as it would be over a train, crusher stockpile or day. The reductions led SCIP
to an equal-valued optimum that does this, so
`chunks_fill_close_and_reclaim_from_empty` now asserts the interval blend
rather than each chunk's.

## Blocks worked back to back within a segment

A loader may work several blocks of one bar's sequence inside a single
execution segment, back to back in authored order at its dig rate. The order
row for a later block now reads the earlier block's exhaustion at the end of
the *same* cell, so a segment boundary is spent only on a change of bar, pile
or priority. The loader's `rate` row already makes the blocks fit the segment
one after another.

- **Shared blocks.** When another loader can also dig the earlier block, the
  later block still waits for the end of the previous cell: who finished the
  earlier block first cannot be recovered from segment totals.
- **Pile peaks.** Back-to-back blocks deliver at an uneven rate, so a pile
  that is also reclaimed gets a conservative `occpeak` row: opening occupancy
  plus the segment's receipts, with no credit for that segment's reclaim, stays
  within capacity. This is the one place the change can refuse a schedule the
  previous model accepted, and only for a pile close to full.
- **Truck hours** stay averaged over the segment.
- **Replay** now checks authored dig order cell by cell (it did not before),
  with the same shared-block rule, and applies the peak check to segments in
  which a loader delivering to the pile worked more than one block.
- **Derived event budget.** A dig bar now needs one segment, plus one per
  sequence step whose earlier block is shared; a reclaim bar keeps one per
  approved pile.

DreamLand Day 1 (two loaders, one bar each, no shared blocks) now derives one
event position per interval: 6,699 variables and 4,247 rows, proven optimal at
11,200,000.00 in 0.45 s with a valid replay, both loaders working the whole
day. The previous best was 10,702,846.60 at capacity 2, with idle gaps at block
changes.

## Day-by-day start for long horizons

A week of hourly intervals is one large model: a block's columns stay in it
from the hour it becomes reachable to the end of the horizon. On DreamLand's
week (7 days x 1 h, both ROM piles blended) the first root LP alone took
226.5 s, and a 300 s run published 1.09M against a 74.7M bound.

A horizon that needs more than one window is now solved in three steps,
inside the run's one time limit:

1. **Windows.** Each window is one kept day plus two days of look-ahead
   (`blended/rolling.rs`). Only the kept day is kept; its end state opens the
   next window: block remaining tonnes (finished blocks leave the input, which
   retires their columns), pile tonnes and contained quantity from the
   window's own replay, and what is left of reclaim caps, dump capacity and
   crusher days. The last window keeps everything it solves. Each window's
   schedule must pass the independent replay, and the stitched schedule must
   pass it again against the whole horizon. The windows share half the time
   limit.
2. **Completing the seed.** A second copy of the whole-horizon model is solved
   with every movement and segment duration held to the stitched values, which
   fills in the state and indicator columns. A value may move by 1e-3 t or
   1e-3 of itself, whichever is larger; a movement the seed does not make is
   held at zero. Pinned exactly, a real week's seed had no completion: a
   window's answer, mapped back from SCIP's presolved problem, can miss an
   original row by more than SCIP's tolerance. One loader was over its rate row
   by a 0.0002 t tail from the previous block, and each hour's dig from a mixed
   block was split between materials only to within the tolerance, so no one
   extraction total satisfied every `portion` row. A relative band of 1e-5
   still failed and 1e-4 completed. The band moves only the start SCIP is given;
   what is published is always a replayed schedule. SCIP's own `completesol` heuristic
   was tried with the stitched movements as a partial solution and is not
   used: it searched a neighbourhood of them instead, spent the whole budget,
   and returned 1.09M (with `boundwidening = 0` as well).
3. **Whole horizon, seeded.** The completed solution is added before solving;
   SCIP checks it against the original model before storing it. The
   seeded-mode workaround (`misc/allowweakdualreds = false`) applies. The
   published schedule is whichever replayed schedule is worth more, with the
   whole-horizon dual bound when there is one.

The stitched schedule is published as soon as it has passed the
whole-horizon replay, before the whole-horizon solve starts, under the same
currentness checks as any other answer. The status says the day-by-day
schedule is shown while a better one is sought, and the Stop button keeps
it. The run's final answer replaces it and is never worth less, because it
is the better of the two. If the whole-horizon solve fails, the early
schedule stays, alongside the failure.

The stitched schedule has no bound of its own: each window is optimal at best
given the days already kept. A run whose whole-horizon solve found nothing
better publishes the stitched schedule, with the whole-horizon bound when that
solve produced one and no bound otherwise. The run details state which of
the two was published.

Chunked piles are not solved this way. A chunk's open, closed and emptied
state would have to cross the boundary, and the model has no opening form for
a partly filled open chunk.

Two exact changes came with this:

- A dig candidate whose block the input does not hold gets no columns.
  Capture never produces one, because validation refuses it; a later window
  does, for every finished block.
- A dig bar's readiness in the first cell is now whether any of its blocks
  holds material, as the replay already defined it. It was forced to 1, which
  could make a bar with no work block lower-priority bars for one cell.

SCIP's LP now starts with primal simplex and devex pricing
(`lp/initalgorithm = p`, `lp/pricing = d`) in every solve. Only the LP
algorithm changes, not the model:

| model | default | primal + devex |
|---|---|---|
| DreamLand week, first LP alone | 226.5 s | 103.0 s |
| DreamLand week, seeded run | root LP unfinished in 208 s | root LP at 112 s |
| DreamLand Day 1 | 0.45 s, optimal | 0.48 s, optimal |
| known-answer fixture, 24 h / 72 h | 0.35 s / 2.04 s | 0.05 s / 0.19 s |
| competition fixture, 24 / 48 / 72 h, 60 s | same bounds | same bounds, one better incumbent |

DreamLand week, 300 s limit, after both changes: five windows in 73 s, the
first four proven optimal. The stitched week replays at 74,628,305.71. The
seed completes in 1.4 s and SCIP accepts it. The whole-horizon root LP
finishes at 112 s, and at the limit the bound is 74,708,782, a gap of 0.11%.

Run to run, window times vary. Windows are solved optimally but not
identically, and on other runs the last window reached its limit, which made
the windows take up to 150 s. The whole-horizon solve then had too little
time left for its root LP, and published the day-by-day schedule with SCIP's
pseudo-solution bound only. The whole-horizon root LP needs 110-120 s here.
SCIP's bundled LP solver, SoPlex 8.0.2, has no barrier method. On this week's
LP, HiGHS's interior-point method took 36 s against 93 s for its simplex, with
the same optimum.

A fault this exposed is also fixed. SCIP reports "no dual bound" as its
infinity, 1e20, and a solve stopped before its first root LP used to publish
1e20 as the bound, with a matching gap. Both are now reported as absent.
