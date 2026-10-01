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
The solve itself runs in a child process of the app; see
[Solver process](#solver-process).

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

1. **Windows.** Each window is one kept day, solved alone; only if that
   fails is the run solved again with two days of look-ahead per window (see
   "Days alone first" below; `blended/rolling.rs`). Only the kept day is kept; its end state opens the
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
better publishes the stitched schedule, with the tighter of the whole-horizon
bound and the relaxation bound (below) when either exists, and no bound
otherwise. The run details state which schedule was published and which
solve proved its bound.

Chunked piles are solved this way too. Each chunk's tonnes and contained
quantity cross the boundary from the window's replay, and whether it was
closed in the last kept interval crosses from the window's schedule: a closed
chunk opens the next window closed (`BlendPile::chunk_closed`), so an emptied
slot is never reused, and an open one may still close at the boundary or keep
filling. Authored piles are unaffected: without the flags, a chunk holding
opening material starts closed as before. The replay now also reports each
chunk's closing state and checks that a chunk required to start closed does.
On the four chunk fixtures over 96 h (dynamic FIFO/LIFO, released FIFO/LIFO,
40 s each) every seeded run reached a proven optimum; the dynamic FIFO one
unseeded had ended at its limit with nothing better than 0.

Seeded solves, and the seed's completion, run with SCIP's MPEC heuristic
switched off (`heuristics/mpec/freq = -1`). On the dynamic FIFO fixture the
seeded solve hung four runs out of four: MPEC passed Ipopt an NLP whose MUMPS
ordering (METIS) corrupted the heap, glibc aborted inside `malloc`, and the
solver thread was left waiting forever. With MPEC off the same runs finished
three times out of three. Unseeded solves keep SCIP's default. A day-by-day
window's derived input is also validated like a captured one before SCIP
sees it.

The fault is not MPEC's. Any Ipopt solve can reach it: MUMPS calls the
bundled METIS through `mumps_metis_nodend_mixedto32`, which corrupts the heap,
and glibc then aborts the whole app (`free(): invalid size` in
`gk_malloc_cleanup`) or hangs the solver thread. SCIP's sub-NLP heuristic hit
it in an unseeded solve of the competition fixture over 168 h, two runs out of
two. Every SCIP solve now gives Ipopt an options file,
`$TMPDIR/incline-ipopt.opt`, holding `mumps_pivot_order 0`, so MUMPS orders
with AMD instead. The same fixture then completed five runs out of five.
With the option set to METIS explicitly, it hung. The four chunk fixtures over
96 h reached the same proven optima as before. The ordering affects only how
Ipopt factorises, not what any solve means. MPEC stays off in seeded solves.
A fault of this kind is why the solve now runs in its own process: the next
one ends a run, not the app.

Two exact changes came with this:

- A dig candidate whose block the input does not hold gets no columns.
  Capture never produces one, because validation refuses it; a later window
  does, for every finished block.
- A dig bar's readiness in the first cell is now whether any of its blocks
  holds material, as the replay already defined it. It was forced to 1, which
  could make a bar with no work block lower-priority bars for one cell.

Two ideas from the literature were tried on the windows and not kept. Each
was measured on six fixtures at 40 s, against the unchanged code:

- **Relaxed look-ahead** (Ankem et al. 2026: binaries only in the kept
  period, continuous beyond it; only the kept day replayed). Worse. SCIP found
  no schedule at all in the competition fixture's first window, and the
  dynamic LIFO chunk fixture ended at 0 instead of its proven 9,600. Dynamic
  FIFO ended at 6,400 instead of 9,600. The graded fixture was unchanged. The
  fractional look-ahead makes the kept day's choices look better than they
  are, and it weakens SCIP's primal heuristics.
- **No exhaustion flag before a block could be finished** (Bley et al. 2010,
  early-start fixing, applied to `exh` as reach pruning already is to dig
  columns). Neutral, and graded's first window was slower (2.2 s against
  0.26 s). SCIP's presolve evidently derives these fixings from the column
  bounds itself.

The patch for both is not in the tree. DreamLand was not re-measured, so a
look-ahead that stays integer but shorter remains untested.

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

## Dispatch starts

At the default 60 s limit the day-by-day windows still failed on DreamLand's
week. Each window gets 6 s, and SCIP could not finish a three-day window's
first LP in that time ("gap None" in the log). The best schedule it held by
then had the loaders working about one hour in ten. Every window ended at its
limit, and the run published 5.86M against a 74.7M bound, with mostly idle
Gantt rows. SCIP's heuristic emphasis (aggressive, or feasibility) changed
nothing, and a shorter look-ahead helped later days but not the first.
Ten times the time (600 s) still left the first three windows poor.

Yet on that project digging flat out is nearly optimal, and it is cheap to
construct. `blended/greedy.rs` does so without a solver:
- Each interval, every loader works its highest-priority ready bar at its
  rate, with readiness defined exactly as the replay defines it.
- A dig bar works its blocks in authored order, back to back where the
  formulation's `order` rows allow it.
- Each block's materials are split in proportion and sent to their
  best-paying destinations first, within crusher days, dump and pile
  capacity, and truck hours. Loaders take shared room in order of what
  their bar pays best.

On DreamLand's week it builds a schedule worth 74.50M in 22 ms, which the
replay accepts.

The dispatch schedule is only ever a start:
- **Windows:** every day-by-day window is offered its own dispatch schedule.
  It is completed and checked by SCIP exactly as the stitched seed is, so the
  seeded-mode workaround applies to the window too.
- **Single solves:** a horizon solved in one piece, or one whose day-by-day
  start failed, is offered the dispatch schedule for the whole horizon.
- **Fallback:** a dispatch schedule the replay rejects, or SCIP will not
  complete, is logged and the solve runs as before. Chunked piles are not
  dispatched.

The dispatcher's limits on reclaim:
- It reclaims into a destination whose admission depends on the blend only
  when the pile's released blend clears the boundary by the formulation's
  margin.
- It never reclaims where a grade-conditional value applies.

DreamLand, from the app's captured request, 60 s limit:

| horizon | before | with dispatch starts |
|---|---|---|
| 1 day | 11.2M optimal, 4.3 s | 11.2M optimal, 0.6 s |
| 3 days | FeasibleLimit at 60 s | 33.6M optimal, 6.0 s |
| 4 days | 6.10M at 60 s | 44.8M, proven by the relaxation bound, 10 s |
| 7 days | 5.86M at 60 s | 74.50M at 60 s, 0.29 % below the 74.71M bound |

Fixtures through `execute_scip_blend`, 40 s:

| fixture | before | with dispatch starts |
|---|---|---|
| competition 24 h | 5,700 | 5,700 |
| competition 72 h | 13,500 | 15,900 |
| competition 168 h | 31,500 | 33,900 |
| graded 24 / 72 / 168 h | optimal in 0.08 / 0.96 / 2.7 s | optimal in 0.04 / 0.10 / 0.34 s |

On the competition fixture's 24 h the first dispatcher was worse than the
unseeded solve (4,500 against 5,700):
- It refused every reclaim into a crusher with a minimum grade.
- It let direct feed fill the crusher ahead of reclaim worth more.

Fixing both brought it to 5,700. A started solve can stop at its gap target
(`GapLimit`) rather than close the gap exactly. The developer check that
required `Optimal` now accepts either, and still requires an optimal
termination.

## Relaxation bound from HiGHS

SCIP's own bound on a long horizon arrives late, because its first LP is
slow: SoPlex has only simplex methods. SCIP cannot be pointed at HiGHS's
interior-point method instead. Its HiGHS LP interface (`lpi_highs.cpp`)
answers a barrier request with dual simplex, so building SCIP against HiGHS
would swap one simplex for another and change packaging on every platform.

Instead, a run solved day by day also solves the model's linear relaxation
with HiGHS's interior-point method, on its own thread, from the start of the
run (`blended/relaxation.rs`, `RelaxationJob` in `app/scip_blend.rs`). The
relaxation is the model `formulate` builds with three changes. Each one only
enlarges the feasible set, so the optimum is an upper bound on every schedule
the model allows:

- binaries may take any value in 0..=1;
- indicators are posted as the formulation's big-M rows, valid at any flag
  value;
- the perfect-mixing equality is left out. The grade-box rows stay, so a
  reclaim's grade is held to the pile's grade ceilings, but not to its blend.

The last change makes the bound weaker on models whose value depends on the
blend, not wrong. HiGHS's optimum is loosened by `max(1e-4, 1e-6 x |value|)`
to cover its solve tolerances before it is used. A replayed schedule worth
more than the loosened bound would mean the relaxation is wrong. The bound is
then logged and dropped, and the schedule, which was replayed on its own, is
kept. The run never waits for the relaxation: it is polled when the
day-by-day schedule is ready and when the run ends. Cancelling the run, or
the run finishing, stops it at HiGHS's next interior-point iteration.

The bound is used in three places:

- **The early day-by-day schedule** is shown with it, so its gap is known
  while the whole-horizon solve runs.
- **The final result** carries the tighter of SCIP's bound and the
  relaxation's. The run details say when the relaxation proved it.
- **The gap target.** If the day-by-day schedule is already within the
  run's gap target of the relaxation bound, the whole-horizon solve is
  skipped. The schedule is published as optimal within that target
  (`DayByDayRole::Proven`), the same claim SCIP makes when it stops at its
  gap limit. If the bound only arrives once the whole-horizon solve has
  started, the solve is stopped when the bound proves the day-by-day
  schedule, with the same claim. A SCIP result that the relaxation bound
  brings within the target is reported as optimal in the same way.

Bounds on the fixtures, relaxation against SCIP, with SCIP given 40 s:

| fixture | relaxation | SCIP |
|---|---|---|
| known answer, 3 h | 6,000 in 2 ms | 6,000, optimal |
| graded, 96 h | 24,000 in 30 ms | 24,000, optimal |
| dynamic chunks FIFO / LIFO, 96 h | 9,600 in 0.24 / 0.29 s | 9,600, optimal |
| released chunks FIFO, 96 h | 800 in 55 ms | 800, optimal |
| competition, 72 h | 18,300 in 66 ms | 18,300 at the limit |
| competition, 168 h | 36,300 in 0.24 s | 36,300 at the limit |
| threshold trap | 11,200 in 3 ms | 900, optimal |

The threshold trap is the weak case: its value depends on whether a reclaim's
blend clears a grade threshold, which the relaxation cannot see. Through a
whole run, 40 s budget:

- The graded 96 h fixture is proven within the gap target by the
  day-by-day schedule. It finishes in 0.39 s with no whole-horizon solve.
- The dynamic chunk fixtures show their early schedules with a 9,600 bound
  and finish as before, optimal at 9,600.
- The competition week shows its early schedule at 31,500 against 36,300
  after 20 s, and ends the same.

DreamLand's week has not yet been run with the relaxation. Its earlier LP
probe, the whole-horizon model written by SCIP and relaxed by HiGHS, took
36 s by interior point, which would put a bound on the early schedule about
two minutes sooner than SCIP's root LP. Its day-by-day schedule sat 0.11%
below the final bound, above the default 0.01% gap target, so it would still
run the whole-horizon solve.

## Days alone first

With dispatch starts every window began from a good schedule, and the
look-ahead became most of each window's cost: a window solved 72 hours to
keep 24. Solved alone, each of DreamLand's days was optimal in under half a
second, and the week came out better than with the look-ahead. A window
never looks past the run's horizon either way, so the look-ahead never
planned days the user did not ask for; it only made each day's decision see
the next two.

That sight matters when a day can trap the next. On the dynamic LIFO chunk
fixture, days solved alone closed every chunk by the end of the second day,
because an open chunk was worth nothing within that day. The third day's
diggers then had nowhere to put material, and its window was infeasible. So
the run now solves the days alone first and, if that fails, solves them
again from the start with the 48-hour look-ahead, within what is left of the
day-by-day share of the budget. If that fails too, the whole-horizon solve
runs from a dispatch start as before.

Three fixes came out of measuring this:

- **Carried pile state is clamped.** An emptied pile could close a day with
  3e-14 contained against 0 t, which the next window's input check refused,
  ending the day-by-day stage. Piles are now clamped as chunks already were:
  tonnes to `0..=capacity`, contained to `0..=tonnes`.
- **`stocklink` is written per tonne of capacity.** SCIP checks a column
  against its bound relative to the bound, but a row against a zero
  right-hand side in absolute terms. Days that each filled DreamLand's
  200,000 t pile left it 0.06 t over capacity: inside the bound's tolerance,
  but 0.06 over the unscaled `open_t - capacity * stock <= 0`, so SCIP
  refused the completed seed. Dividing the row by the capacity changes no
  schedule it allows.
- **The relaxation can stop the whole-horizon solve.** Days alone are often
  done before the relaxation bound is, so the check made when the
  day-by-day schedule is ready found no bound yet, and SCIP then ran to its
  time limit on a schedule that was already optimal. A watcher thread now
  passes the run's cancellation to SCIP and also interrupts it once the
  bound proves the seed (`watch_solve`). SCIP only acts on an interrupt at
  its next event, so during a long root LP the stop can lag the bound by
  several seconds; the user's Stop button has the same lag.

DreamLand, from the app's captured request, 60 s limit (each column measured
on its own):

| horizon | 48 h look-ahead | days alone first |
|---|---|---|
| 1 day | 11.2M optimal, 0.6 s | 11.2M optimal, 0.5 s (one window, unchanged) |
| 3 days | 33.6M optimal, 6.0 s | 33.6M proven, 2.9 s |
| 4 days | 44.8M proven, 10 s | 44.8M early at about 2 s, proven at 16 s |
| 7 days | 74.50M at 60 s, early result at 30 s | 74.58M at 60 s, early result at 3 s, 0.18 % below the 74.71M bound |

On the week the whole-horizon solve did not improve on the day-by-day
schedule in the remaining time. Its incumbent, the completed seed, was then
refused by the replay because pile 0 sat 0.06 t over capacity, and the
day-by-day schedule was kept. The replay checks capacity to 1e-5 t, while
SCIP's bound tolerance on a 200,000 t pile is 0.2 t; that mismatch is older
than this change and is still open.

Fixtures through `execute_scip_blend`, 40 s:

| fixture | 48 h look-ahead | days alone first |
|---|---|---|
| competition 96 h | 19,200 | 20,400 |
| competition 168 h | 33,900 | 33,900 |
| graded 168 h | optimal in 0.3 s | optimal in 0.1 s |
| dynamic chunks FIFO 96 h | 9,600 in 29 s | 9,600 in 24 s |
| dynamic chunks LIFO 96 h | 9,600 in 38 s | 9,600 in 38 s, after the fallback |

## Solver process

SCIP, SoPlex, Ipopt, MUMPS and HiGHS are native code. A fault in one of them,
such as the METIS heap corruption above, is not a panic the job queue can
catch. Inside the app it took the whole app down, unsaved edits included. So
Run Period and Run All Periods now solve in a second copy of the app's own
binary, started as `incline-design --schedule-solver`
(`src/app/solver_process.rs`). Capture and publication stay in the app.

- The app writes the captured input, run identity and options to the
  process's stdin as one line of JSON. The process runs the same solve as
  before, day-by-day windows and relaxation bound included. It writes back
  log records, the day-by-day schedule when there is one, and the finished
  run, one framed JSON line each. JSON carries every `f64` exactly
  (`serde_json`'s `float_roundtrip`), and maps with tuple keys travel as
  lists of pairs.
- The app replays every schedule the process sends, early or final, itself.
  It publishes only if that replay passes, agrees with the process's value
  to 1e-9 relative, and stays under the reported bound. A process whose
  memory a fault had corrupted cannot publish what it did not solve. The
  process's own replay findings travel only to explain an unpublished run.
- Anything a native library prints to stdout, such as SCIP's log under
  `INCLINE_SCIP_LOG`, is told apart from messages by the frame and logged.
  stderr is logged too, and its last lines are logged again when the process
  dies.
- Cancel or Stop kills the process. On the competition week, a cancel
  4 s in returned 11 ms after the request. A process whose app has gone
  sees its stdin close, stops its solve, and aborts if that has not finished
  within 30 s.
- A process that ends without an answer fails the run with "The solver
  stopped unexpectedly (signal …)". A schedule already shown early stays,
  as it does when the whole-horizon solve fails.

Checked on the fixtures against the same solve run in the app, 40 s budget:

| fixture | solver process | in the app |
|---|---|---|
| graded, 96 h | optimal, 24,000, relaxation bound, 0.42 s | same, 0.40 s |
| competition, 72 h (30 s) | 13,500 against 18,300 at the limit | same |
| competition, 168 h | early 31,500 at 20 s, final 31,500 against 36,300 | same |

A stand-in process that aborted after reading its request was reported as
signal 6 with its stderr, and the app carried on. The process adds tens of
milliseconds for starting up and moving data. Not yet checked: the app itself
running DreamLand through the process, and Windows and macOS. There,
`current_exe` and pipe handling are standard library, and a debug Windows build
starts the process with `CREATE_NO_WINDOW`.

