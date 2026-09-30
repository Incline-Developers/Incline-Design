# SCIP exploration: blended stockpile optimisation

Status: SCIP is now the normal desktop scheduling backend. This document
preserves the investigation and historical benchmarks; statements below about
experimental availability and two-sided boundary indicators describe the old
implementation. Current run semantics and restrictions are documented in
[Scheduling SCIP integration](scheduling-scip-integration.md).

`cargo check` / `cargo run` use bundled SCIP 10.0.2 by default. The iterative
HiGHS comparison is optional (`--features blend-experiment`). System SCIP
requires `--no-default-features --features scip-system` and `SCIPOPTDIR`.
Native solver dependencies are excluded from WASM.

## 1. Why a second backend was investigated at all

The accepted model treats stockpile material as **ordered parcels of fixed
composition**. A reclaim draws a known material share off a known position,
so every relationship stays linear and HiGHS can solve it.

A **blended** pile has no positions. What comes off it is the pile's own
average at the moment of the draw, and that average is a ratio of two
decision variables. Cleared of the division it is a nonconvex bilinear
equality. No linear backend can express it - `good_lp`'s interface cannot
even describe it.

## 2. Backend capability findings

Dependency path: [`russcip 0.10.0`](https://docs.rs/crate/russcip/0.10.0) with
`default-features = false, features = ["bundled"]`, which pulls
`scip-sys 0.1.28`.

`bundled` downloads a prebuilt SCIP from `scipoptsuite-deploy v0.12.0` and
uses scip-sys's *pregenerated* Linux bindings, so the build needs neither a
system SCIP nor libclang/bindgen. The linked build identifies itself as:

```
SCIP 10.0.2 [LP solver: SoPlex 8.0.2]
```

The same deploy bundle ships GCG 4.0.2 and IPOPT 3.14.19. SCIP itself is
Apache-2.0 from version 9 onward, which is why this path needs no academic
licence; IPOPT is EPL-2.0. **Distribution consequence:** a shipped build
would carry a dynamically linked `libscip` downloaded at build time, which is
a supply-chain and packaging decision well beyond an experiment. An
alternative exists and is now **verified working**: Arch packages
`scip 10.1.0` (with `soplex 8.1.0`, `papilo 3.0.2`, `onetbb`), and scip-sys
links a system install via its `bindgen` feature and `$SCIPOPTDIR` - the
`scip-system` cargo feature. That is the path a real integration should
take, and the verification-stage measurements in §7-§8 were produced on it.

| Requirement | Finding |
|---|---|
| Nonconvex bilinear equality | Yes. `add_cons_quadratic` maps to `SCIPcreateConsBasicQuadraticNonlinear`. |
| Integers alongside nonlinear | Yes. A binary selecting tonnage across a bilinear equality branches correctly. |
| Feasible solution retrieval | Yes. `best_sol()`, `n_sols()`, `Solution::val`. |
| Objective bound, gap, status | Bound and status yes; **gap is not exposed** by russcip and needs raw `SCIPgetGap`. |
| Time limits | Yes. `limits/time` as a real parameter (russcip's own setter takes whole seconds only). |
| Cancellation / interruption | **Not in the safe API.** `SCIPinterruptSolve` exists in scip-sys and is reached through `Model::scip_ptr()`. |
| Feasible starting solutions | Yes, but unsafe at default settings - see below. |
| WASM | Excluded by construction. |

Everything needing a raw call is wrapped once in `scip/adapter.rs`; there is
no general binding layer.

### 2.1 A warm start silently corrupts the answer

This is the most important finding in the investigation.

Supplying a feasible starting solution to a model containing nonlinear
constraints makes SCIP 10.0.2 terminate at the **seeded** objective, report it
as `Optimal`, and report a dual bound equal to that same value, having
explored zero nodes. The better solution is never found, and because the gap
is zero the result is indistinguishable from a proven optimum.

Reproducer - three variables, two constraints:

```
maximise c   subject to   c * t = m,   t = 10 + 10 b,   m = 6,   b binary
```

True optimum `c = 0.6` at `b = 0`. Seed the feasible `b = 1` point
(`c = 0.3`):

| configuration | result |
|---|---|
| no seed | 0.6, correct |
| seeded, defaults | **0.3 reported optimal, bound 0.3, 0 nodes** |
| seeded, `presolving/maxrounds = 0` | 0.6, correct |
| seeded, `misc/allowweakdualreds = false` | 0.6, correct |
| seeded, `misc/allowstrongdualreds = false` | still 0.3 |

It is sign-independent (identical minimising a negated objective), and it does
**not** occur on a purely linear MILP seeded the same way - so the interaction
belongs to the nonlinear presolve path, which is exactly the path a blended
formulation depends on.

Mitigation: `misc/allowweakdualreds = false` whenever a seed is supplied.
Weak dual reductions are a genuine presolve strength, so this is a real cost,
not a free fix.

The wider conclusion is the one that shapes the rest of this document: **a
backend's own verdict is evidence, not proof.** Independent replay is not
optional here.

#### 2.1.1 Verified against SCIP 10.1.0

The anomaly was re-verified on **SCIP 10.1.0** ([release notes](https://github.com/scipopt/scip/releases/tag/v10.1.0),
2026-09-18), because its release notes do not mention a fix and the conclusion
must not rest on one build:

- Path: Arch's `scip 10.1.0` package (with its `soplex 8.1.0`, `papilo 3.0.2`
  and `onetbb` dependencies), extracted locally and linked through
  `russcip 0.10.0`'s `bindgen` feature with `$SCIPOPTDIR` - exactly the
  "system SCIP" path a real integration would take. `russcip 0.10.0` remains
  the newest crate release, so no newer crate pairing exists to try.
- Result, via the retained `seeded_nonlinear_solution_behaviour_on_this_build`
  probe: **identical behaviour on both versions.**

| configuration | SCIP 10.0.2 (bundled) | SCIP 10.1.0 (system) |
|---|---|---|
| unseeded, defaults | 0.6, correct | 0.6, correct |
| seeded, defaults | **0.3 reported optimal, bound 0.3, 0 nodes** | **0.3 reported optimal, bound 0.3, 0 nodes** |
| seeded, `misc/allowweakdualreds = false` | 0.6, correct | 0.6, correct |
| seeded, `presolving/maxrounds = 0` | 0.6, correct | 0.6, correct |

One load-bearing detail for anyone re-checking: in the reproducer, the
contained quantity must be a **fixed column** (`contained ∈ [6, 6]`), not a
constant right-hand side on the bilinear row. With a constant RHS the anomaly
does not appear on either version - the weakened form is a different problem
to SCIP's presolve. The `scip/scenarios.rs` probe encodes the fixed-column
form.

The finding is reproduced on two SCIP releases with the same Rust crate and
reproducer. It remains a configuration-specific finding; a separate
implementation of the reproducer has not independently confirmed the cause.

### 2.2 Numerical convention for grade boundaries

SCIP satisfies constraints to `numerics/feastol`, 1e-6 by default. The smoke
solve returned a grade of 0.5999999999 with a bilinear residual of 1e-9, and
the scenario runs returned objectives up to 9e-5 above their own dual bound.

A bare `>=` on a blended grade can therefore be met with up to ~1e-6 of
violation. Minimum-grade conditions are tightened by an explicit documented
margin (`blended::formulation::GRADE_MARGIN`) so a delivery that reports as
qualifying
genuinely does, and the replay applies the same margin rather than inventing
a second convention.

## 3. Blended stockpile semantics

The experiment retains the receipt-release approximation:

- Opening stock is available immediately.
- A reclaim during interval `k` draws from `k`'s **released opening** blend.
- Receipts during `k` occupy capacity on arrival but join the reclaimable
  blend only at the boundary into `k + 1`.
- Every reclaim inside `k` sees the same blend, whatever order it happens in.
- A pile cannot reclaim more than its released opening tonnes.
- Execution segments continue to govern loaders, trucks and capacity; only
  inventory state lives at interval granularity.

This is **discrete boundary mixing**, not continuously changing perfect
mixing during simultaneous receipt and reclaim. For one blended pile FIFO and
LIFO have no meaning, so the authored `ReclaimOrder` is deliberately not
consulted; opening lots are combined explicitly by tonnes and contained
quantity by the input builder.

### 3.1 Equations and units

`T` is tonnes; `Q` is contained quantity in tonnes of the graded component
(a dimensionless mass fraction times tonnes).

```
T_open[p,k+1]   = T_open[p,k]   + T_recv[p,k]   - T_recl[p,k]
Q_open[p,k+1,g] = Q_open[p,k,g] + Q_recv[p,k,g] - Q_recl[p,k,g]
```

Receipts carry identified material, so `Q_recv` is linear in movement tonnes.
The pile's own draw is not:

```
Q_recl[p,k,g] * T_open[p,k] - T_recl[p,k] * Q_open[p,k,g] = 0
```

one nonconvex bilinear equality per pile, interval and grade.

**Why that equality alone is unsound.** At `T_open = 0` it degenerates to
`0 = 0` and stops constraining `Q_recl`, so an empty pile could supply
contained metal from nothing. The model therefore also carries grade-box
rows, which are physically true independently:

```
0 <= Q_recl[p,k,g] <= gmax[g] * T_recl[p,k]
0 <= Q_open[p,k,g] <= gmax[g] * T_open[p,k]
             T_recl[p,k] <= T_open[p,k]
```

`gmax[g]` is the highest fraction any material in the problem carries - a
finite physically derived bound, not a big-M. These make "zero inventory
supplies nothing" structural rather than something the solver must discover,
and they tighten the bilinear relaxation.

**Physical occupancy, not released inventory (corrected).** An earlier
revision checked capacity as `released opening + receipts so far <= capacity`
at each segment boundary. That conflates two different quantities: *physical
occupancy* (everything sitting on the pile, with new receipts counting on
arrival and reclaim freeing space as it happens) and *released inventory*
(what a reclaim in this interval may draw on, which under boundary mixing is
the interval's opening only). The conflation had a real cost: a pile that
opened full could not receive anything while it was being drawn down, so
production was refused purely because the pile began full.

Both the model (`formulation::occupancy_rows`, shared by the chunked and
unchunked paths) and the replay now carry physical occupancy:

```text
opening[k] + sum(receipts in segments 0..=s) - sum(reclaims in segments 0..=s) <= capacity
```

checked at every segment boundary, with a matching non-negativity row. Rates
are constant inside a segment, so occupancy is linear across a segment's
interior and the endpoint values bound it throughout. The release-timing rule
(a reclaim draws only on the released opening) is unchanged and checked as its
own separate quantity. The regression scenario
`a_full_pile_can_receive_while_it_is_reclaimed` pins the correction: a
1,000 t pile at capacity receives 500 t while reclaiming 500 t, holds exactly
1,000 t throughout, and earns both streams' value - the earlier formulation
refused the receipt and lost the dig.

### 3.2 Which reserve fields may be used as a grade

`ReserveAggregation` distinguishes three incompatible meanings, and the
experiment refuses anything it cannot justify:

| aggregation | treatment |
|---|---|
| `WeightedAverage { weight_field }` where `weight_field` is the plan's tonnage field | **accepted** - `grade x tonnes` is its contained quantity |
| `WeightedAverage` by any other field | rejected: weighted by volume or another tonnage column, `grade x tonnes` is not contained quantity |
| `Sum` | rejected: an extensive quantity. Averaging a total is the specific error to avoid |
| `Category` | rejected: a label, not a number |

Also rejected: a missing value (**not** treated as zero - it would quietly
dilute a blend), a non-finite value, and a value outside the range its
declared unit allows. The unit basis (fraction vs percent) is declared
explicitly because nothing in project metadata records whether "Fe" means
0.62 or 62.

Category-dependent reclaim rules are **not** supported for blended reclaim:
categories are discarded when material enters a blended pile, so such a rule
cannot be evaluated and must be flagged rather than silently treated as
unrestricted.

## 4. Scheduling constraints retained

Reproduced in the blended model: the shared event clock (segment durations
summing to the interval), per-loader single assignment per segment, loader
rate limits, shared ground balances (no double depletion), **authored
dig-block order** (a later block is unavailable until the earlier one is
exhausted - enforced by exhaustion indicators, not left to economics), authored
task windows, shared continuous truck-hours per segment, several destinations
per segment, crusher daily budgets shared by direct mining and reclaim,
destination capacities, and reclaim caps.

**Mandatory authored bar priority** (added in the verification stage): a
planner's bar order is an instruction, not a hint. Per loader and cell, over
the authored order, the model carries a `ready`/`selected` binary pair and
four rows:

```text
selected[t] <= ready[t]                             work needs work available
selected[t] + ready[e] <= 1     for every e before t  no economic preemption
selected[t] >= ready[t] - sum(ready[e] before t)      mandatory selection
sum over t of selected[t] <= 1                        one bar at a time
```

The third row does the real work: it forbids *voluntary idling*, so a loader
cannot stand down on its highest-priority ready bar to make a lower-priority
bar look like the only option. `ready` is defined against ground exhaustion
and pile inventory through one-directional indicators (a bar cannot claim to
be unready while it still has work), and movements are gated on the
*selected* bar for the source they draw on, so the priority binds on tonnes
rather than decorating the model. Window containment matches the accepted
backend: a bar covering only part of a calendar interval is not worked in
that interval.

Not reproduced: the parcel model's FIFO/LIFO ordering machinery for
*unchunked* piles, where it has no meaning.

## 5. Independent physical replay

`blended/replay.rs` (solver-independent since the verification stage; it takes
a `BlendSolution` and knows nothing about how it was produced) reconstructs
the timeline from the published movement rows alone and recomputes every
quantity from first principles. It never reads a solver variable other than
the movement tonnes, durations and published chunk state, and never consults
a solver's constraint-status report.

Notably, the replay recomputes the blended grade by **division** (`Q / T`) -
exactly the operation the formulation has to avoid. The model's cleared
bilinear form and the replay's direct ratio are independent routes to the same
number, so a disagreement is detectable.

Checked: tonnes and contained-quantity conservation, actual blended grades,
grade eligibility, inventory release timing, loader and truck capacity per
segment, destination and crusher limits, authored windows, reclaim caps,
physical pile occupancy throughout (opening plus receipts minus reclaim at
every segment boundary), and that contained quantity never exceeds its own
tonnage. Absolute and relative residuals are reported whether or not they
breach tolerance.

Two report channels are kept deliberately apart:

- `issues` - **physical** rule violations. A candidate with any of these is
  not a schedule at all, whatever a solver reported.
- `grade_issues` - grade-dependent conditions the timeline fails. Equally
  fatal for the nonlinear method, but the *iterative* method needs the
  distinction: "this timeline is impossible" and "this timeline is possible
  but my grade estimate was wrong" differ in exactly whether the next
  iteration can learn from it.

For chunked piles the replay also recomputes each chunk's composition from
its **own published receipts** (`ChunkRow::receipts`, per movement candidate)
rather than accepting a solver-asserted grade, checks the chunk lifecycle
(sequential fill and close, no receive-while-reclaiming, FIFO/LIFO frontier)
independently, and evaluates grade limits against **what was actually drawn**
- the blend of the chunks the reclaim took - which for a chunked pile is not
the pile-wide average. An earlier revision checked the pile average on
chunked piles and reported breaches on schedules that were in fact correct;
the physically right quantity is the drawn blend.

The replay caught four real defects during development, which is the point of
it: a grade-boundary margin applied in the wrong direction (deliveries at
0.619999 against a 0.62 minimum); a big-M leak passing ~1e-4 t of off-spec
material through an "unused" indicator; per-chunk capacity enforced without
the pile's own capacity; and a replay/formulation disagreement about a chunked
pile's opening stock.

## 6. Chunk lifecycle (§8)

The brief requires this lifecycle to be written down before any chunk code.
It was, and the implementation follows it exactly
(`blended::formulation::chunked_pile`).

| aspect | policy |
|---|---|
| Capacity | Fixed, authored per chunk. |
| Fill order | Sequential: chunk `i + 1` receives nothing until `i` is closed. |
| Becomes reclaimable | When closed to further receipts. |
| Partially filled | May be closed at a calendar interval boundary. |
| Receive while reclaiming | Not permitted. A chunk is filling **or** closed. |
| FIFO/LIFO | Applies among released, non-empty chunks. |
| When emptied | Stays empty. |
| Slot reuse | None within the experimental horizon. |

**Opening stock**: a chunk holding authored opening material is closed from
the start, so it is immediately reclaimable and the authored order has
something to choose between. A pile that opens empty starts filling at
chunk 0.

Three lifecycle details were added or tightened during the verification
stage, each after the replay caught a schedule the rows admitted but the
lifecycle forbids:

- **Closing is sequential too** (`cCloseSeq`): a chunk cannot close before
  the chunk in front of it. Without this the solver could close an empty
  chunk `c` while `c - 1` was still open and start filling `c + 1`, which
  passes the per-pair receipt rule but breaks sequential fill outright.
- **The pile's own capacity binds** even when its chunks do not over-subscribe
  it jointly at one instant: occupancy rows are shared with the unchunked
  path, summing chunk opening columns against the pile capacity.
- **Lifecycle indicator rows use the tightest physical big-M** (what can
  actually move in one interval: the routed loaders' rate x duration, not the
  chunk's capacity). A binary sitting one integrality tolerance away from
  zero admits `M x tolerance` through an "off" indicator, so the tighter M is
  the difference between a leak the replay tolerates and one it reports;
  whatever still leaks is published as `chunk_dust_tonnes_t` rather than
  hidden.

These are **experimental assumptions, not approved application behaviour**.
Their consequences are real: no slot reuse bounds total throughput at
`chunks x chunk capacity` plus opening stock for the whole horizon, so a long
horizon with few chunks is throughput-limited by the model rather than by the
equipment - a *product approximation*, not a solver limitation. Each chunk's
composition must arise from its own receipts through the same nonlinear mass
balance - substituting fixed grades per chunk would not be the exact
experiment.

## 7. Measurements

Release build, single machine, 60 s budget per solve. All rows below are from
the **verification stage** formulation (physical occupancy, mandatory bar
priority, chunk lifecycle corrections) running on **SCIP 10.1.0** linked as a
system library; the pre-verification numbers on bundled 10.0.2 were smaller
and easier (most rows solved at the root) because the model then lacked the
priority machinery that makes the search honest.

### 7.1 Experiment 1 - the same parcel MILP, both backends

The accepted backend writes the model it is about to solve to MPS; SCIP reads
that file. Same columns, rows, bounds, integrality and objective.

| horizon | HiGHS status | HiGHS obj | SCIP status | SCIP obj | SCIP bound | SCIP gap | SCIP nodes | MPS |
|---|---|---|---|---|---|---|---|---|
| 4 h | Optimal | 900.0 | Optimal | 900.0 | 900.0 | 0 | 38 | 630 KB |
| 8 h | Optimal (17.1 s) | 1120.0 | TimeLimit | 720.0 | 1120.0 | 55.6% | 21 | 1.9 MB |
| 12 h | FeasibleLimit | 960.0 | TimeLimit | **1140.0** | 1140.0 | 3.1e-7 | 1 | 3.8 MB |
| 24 h | **LimitNoIncumbent** | - | TimeLimit | **1140.0** | 812160 | 711x | 1 | 13 MB |

Read carefully, because the ranking reverses with horizon:

- At 4 h both prove optimality; HiGHS is faster (1.4 s against 4.4 s).
- At 8 h HiGHS proves 1120 while SCIP is still at 720 with a 56% gap.
- At 12 h the order reverses hard: SCIP reaches 1140 with an essentially
  closed gap, where HiGHS returns 960 against the same 1140 bound.
- At 24 h HiGHS returns **no incumbent at all**, while SCIP returns a
  schedule worth 1140. SCIP's *bound* there is uninformative (812160 - the trivial
  bound; it never finished the root), so the run proves nothing about
  optimality, but "a schedule" beats "nothing".

That 12 h/24 h reversal is the single most useful measurement in this
investigation, because the 24 h no-incumbent wall is precisely the limitation
recorded for the accepted backend. It is not evidence that SCIP is a better
MILP solver in general - at 8 h it is plainly worse on the same model - but it
does show the wall is a solver-strategy artefact rather than something
inherent to the formulation.

**Important limitation.** These SCIP figures are **solver-reported only**.
The MPS round-trip preserves the mathematics but not the column-to-meaning
mapping, so the SCIP parcel solutions were *not* put through
`validate_solution`. Given §2.1, they should not be treated as validated
schedules. The agreement between SCIP's 12 h objective and HiGHS's 12 h dual
bound is corroborating, not proof.

### 7.2 Experiment 2 - the blended model

Three dig loaders feeding one blended pile plus one reclaim loader, one-hour
intervals, two segments per interval. "graded" adds a 62% Fe minimum on the
reclaim route.

| horizon | variant | objective | vars | binaries | linear | nonlinear | nodes | wall | replay |
|---|---|---|---:|---:|---:|---:|---:|---|---|
| 4 h | plain | 800.0 | 172 | 92 | 327 | 4 | 198 | <0.1 s | clean |
| 8 h | plain | 1560.0 | 344 | 184 | 655 | 8 | 1544 | 0.5 s | clean |
| 12 h | plain | 1700.0 | 516 | 276 | 983 | 12 | 3759 | 3.6 s | clean |
| 24 h | plain | 1700.0 | 1032 | 552 | 1967 | 24 | 1712 | 7.3 s | clean |
| 48 h | plain | **no incumbent** | 2064 | 1104 | 3936 | 48 | 4175 | 60 s limit | - |
| 4 h | graded | 714.28 | 176 | 96 | 335 | 4 | 291 | 0.1 s | clean |
| 8 h | graded | 1514.28 | 352 | 192 | 671 | 8 | 2029 | 0.7 s | clean |
| 12 h | graded | 1700.0 | 528 | 288 | 1007 | 12 | 1960 | 1.9 s | clean |
| 24 h | graded | 1700.0 | 1056 | 576 | 2015 | 24 | 811 | 5.5 s | clean |
| 48 h | graded | 2600.0 | 2112 | 1152 | 4032 | 48 | 828 | 24.3 s | clean |

Every solved row has a solver-reported zero gap and a replay-clean incumbent;
the replay checks feasibility, while the reported gap is the solver's
optimality certificate for the encoded model. The one failure - plain 48 h,
no incumbent in 60 s - is a search outcome, not an infeasibility: it has a
finite bound of 2600. The graded 48 h variant solves to optimality, but these
measurements do not establish why the plain variant stalled.

### 7.3 Experiment 3 - chunked blended piles

Alternating lean (40% Fe) and rich (80% Fe) chunks, all released at the
start, reclaim paid per tonne.

| horizon | chunks | vars | binaries | linear | nonlinear | wall (FIFO / LIFO) |
|---|---|---|---:|---:|---:|---|
| 4 h | 2 | 104 | 40 | 191 | 8 | 0.01 / 0.01 s |
| 12 h | 5 | 564 | 192 | 1115 | 60 | 0.02 / 0.02 s |
| 12 h | 10 | 984 | 312 | 2255 | 120 | 0.11 / 0.10 s |
| 24 h | 10 | 1968 | 624 | 4511 | 240 | 0.16 / 5.75 s |
| 48 h | 10 | 3936 | 1248 | 9023 | 480 | 26.4 / 7.3 s |

Every run has a solver-reported zero gap and a replay-clean incumbent; FIFO and LIFO produce genuinely
different draws on the same fixture.

### 7.4 Scale comparison

Same horizon, the two stockpile models:

| horizon | parcel model | blended, 10 chunks | ratio |
|---|---|---|---|
| 24 h | 37,824 vars / 16,320 binaries | 1,968 / 624 | ~19x / ~26x |
| 48 h | 121,248 vars / 50,592 binaries | 3,936 / 1,248 | ~31x / ~41x |

The ratios moved from the earlier revision because the blended model gained
the priority and occupancy rows it was missing, not because the parcel model
changed.

### 7.5 What these measurements do and do not establish

They are a fair measure of **model size**: the blended formulation replaces
the parcel model's per-unit ordering chains with a handful of bilinear rows
per pile, interval and grade, and that is a structural reduction of more than
an order of magnitude, not a fixture artefact.

They are **not** a fair measure of problem difficulty. Three honest caveats:

1. **Branching is now exercised but not bounded.** An earlier revision of
   these fixtures solved everything at the root, which measured nothing about
   worst case. The corrected formulation genuinely branches (198-4,175 nodes
   in Experiment 2), and its one failure - plain 48 h returning no incumbent
   in 60 s - is a real search failure on a real instance. But no instance was
   constructed to be *deliberately* hard, so the worst case remains unmeasured
   in both directions.
2. The blended fixtures carry fewer loaders and routes than the parcel audit
   fixture, so the two are not the same scheduling problem. Only the
   Experiment 1 comparison holds the mathematics fixed.
3. These objectives are **not comparable** with the previous revision's
   numbers: the occupancy correction admitted schedules the old model refused
   (a full pile receiving while reclaimed), and the priority machinery
   forbids schedules the old model allowed (economic preemption of authored
   bars). Both changes move the optimum for reasons that have nothing to do
   with the solver.

Anyone quoting these numbers should quote all three caveats with them.

## 8. The iterative fixed-grade HiGHS method, and the comparison

Implemented in the verification stage. `blended::input::BlendInput` remains
deliberately a **separate contract** from the accepted `OptimisationInput` -
the two describe different stockpile physics (ordered parcels of fixed
composition against a blended average), so sharing a type would invite
comparing objectives that do not measure the same thing. The parcel HiGHS
model stays a separate baseline and is not equivalent to either blended
method. The blended experiment now has three parts, deliberately separated:

| part | file | what it is |
|---|---|---|
| scenario contract, semantics, predicates | `blended/input.rs` | solver-independent |
| grade field gating | `blended/grade.rs` | solver-independent |
| independent replay | `blended/replay.rs` | solver-independent |
| **the model, written once** | `blended/formulation.rs` | builds columns and rows through the `Rows` trait; the mixing relationship is the one operation left to the backend |
| SCIP backend | `scip/blend.rs` | implements `Rows::mix` as the nonconvex bilinear equality |
| iterative HiGHS backend | `blended/iterative.rs` | implements `Rows::mix` as `Q_recl = g_hat * T_recl` at the current estimates |

Because both backends consume `formulation::formulate`, the two methods are
not merely *described* as solving the same constraints - they are built from
one source, and the only difference in what they solve is the mixing row.

### 8.1 The method

The blended model's only nonlinearity is the mixing equality. Fix the blend
to an estimate `g_hat` per pile (or chunk), interval and grade and that row
becomes linear; the whole model becomes a MILP HiGHS solves directly. But
`g_hat` is an assumption: the schedule it returns changes what the pile
contains. One iteration is therefore: build the MILP at the current
estimates, solve, **replay** the candidate physically (recomputing real
blends by division), then update the estimates from what the piles actually
held.

Three outcomes per candidate, kept apart because conflating them is how an
iterative scheme quietly reports nonsense:

- **Physically invalid** - not a schedule; no re-estimation fixes it.
- **Grade mismatch** - replayable, but the resulting blends differed from the
  estimates, or a grade-dependent grant was not earned. This is the case the
  iteration exists to resolve.
- **Publishable** - every physical, grade and valuation check passes on the
  *replayed* quantities. A candidate is never publishable merely because the
  fixed-grade MILP declared it feasible.

Estimates update with damping (half the distance to the replayed blend);
undamped updates oscillate on threshold scenarios where the fixed point sits
exactly on a grade boundary. A cycle detector (fingerprinted at 1e-4
resolution) ends a start that revisits a region. Each run tries three
documented deterministic starts - opening-then-supply, physical floor,
physical ceiling - none of which may look at a SCIP result, or the comparison
would be biased.

What the method **cannot** claim: HiGHS's status and dual bound describe one
fixed-grade subproblem, not the blended problem. An infeasible subproblem
does not prove the blended problem infeasible (a wrong estimate can make a
feasible schedule unreachable), and a proven-optimal subproblem does not
bound the blended optimum. The method reports no gap at all; the best it can
say honestly is "here is an independently valid schedule worth X".

### 8.2 Comparison results

Same scenarios, same 60 s budget per method, SCIP 10.1.0. "obj" is the
objective of the published schedule; for SCIP that is replay-validated, for
the iterative method it is the replayed objective of the best valid
candidate. The two objectives are comparable **with each other** (identical
stockpile semantics and constraints) and with neither the parcel model nor
any earlier revision of these fixtures.

The developer harness also prints each backend's **raw solver objective**,
the **objective recomputed from published movement rows**, their difference,
and the count, total absolute tonnes and maximum tonnes of tiny movement and
chunk-receipt columns omitted during extraction. These aggregate adjustments
are reported even when the rounded objectives agree. The known-answer rerun
after this reporting change had zero omitted columns and a zero raw/published
difference on both backends. On dynamic FIFO, iterative HiGHS omitted three
movement columns and three chunk-receipt columns, totalling 5.283e-13 t in
each set, with no objective difference at the displayed precision. On dynamic
LIFO its best schedule omitted 17 movement columns (4.931e-11 t total) and
13 chunk-receipt columns (1.050e-11 t total); the raw/published objective
difference was -2.092e-11. These are extraction aggregates, not extra
allowances. No feasibility tolerance was changed.

<!-- COMPARISON TABLE -->

| world | SCIP obj | SCIP bound / gap | SCIP wall | ITER obj | ITER first valid | ITER total | ITER end |
|---|---:|---|---:|---:|---:|---:|---|
| known-answer (3 h) | **6000** | 6000 / 0 | 0.015 s | **0** | 1 ms | 0.11 s | cycled |
| threshold-trap (8 h) | 900 | 900 / 0 | 0.007 s | 900 | 7 ms | 0.22 s | cycled |
| dynamic chunks FIFO (12 h) | 5600 | 5600 / 0 | 3.3 s | 5600 | 0.54 s | 1.1 s | converged |
| dynamic chunks LIFO (12 h) | 5600 | 5600 / 0 | 4.2 s | 5600 | 0.83 s | 23.2 s | iteration limit |
| resource competition (12 h) | 4950 | 8100 / 64% | 60 s limit | **6900** | 0.06 s | 0.9 s | iteration limit |
| resource competition (24 h) | 6300 | 8100 / 29% | 60 s limit | **6900** | 0.02 s | 1.1 s | cycled |
| resource competition (48 h) | 9600 | 13800 / 44% | 60 s limit | **11400** | 0.08 s | 2.2 s | cycled |

Reading the table:

- Every published schedule from either method is replay-clean; there is no
  row where a solver's claim survived and its replay did not.
- **Neither method dominates.** On the four worlds SCIP closes, it certifies
  the optimum quickly (15 ms to 4 s), and the iterative method matches it on
  three - but on *known-answer* it returns **0**: the optimum there requires
  disciplining the schedule across intervals (dig the rich supply first, let
  the blend rise above 62%, then reclaim), and a fixed-point iteration whose
  estimates move after each myopic solve never discovers the wait. Its
  failure is honest - it cycles rather than converging, and the trivial
  schedule it keeps is genuinely valid - but a valid zero is still a zero.
- On the three resource-competition worlds SCIP cannot close, the order
  reverses sharply: its 60 s incumbents (4950/6300/9600) sit far below its
  own bounds, while the iterative method publishes strictly better schedules
  (6900/6900/11400) in one to two seconds. Better incumbents are not bounds -
  on those worlds the true optimum is unestablished by both - but a strictly
  better schedule for a fraction of the compute is the operative difference.
- The iterative method's "first valid" times are milliseconds throughout: a
  usable schedule is essentially immediate, and iteration only improves on
  it. Its terminations ("cycled", "iteration limit") are reported as what
  they are, never dressed up as convergence.

### 8.3 What the comparison does not settle

Neither method bounds the blended optimum on the harder worlds, so "iterative
found more value" means *found a better feasible point*, not *is closer to
optimal*. The threshold-trap world exists precisely because a fixed point can
sit on a grade boundary; there the iterative method's honest outcome is a
cycle, and a cycle is reported as one rather than dressed up as convergence.
Chunk-slot limits and the receipt-release boundary remain product
approximations of the *scenario*, not costs of either backend.

## 9. Status and recommendation

**Neither backend alone is the answer; the evidence points at a hybrid, and
at a specific division of labour.**

What the verification stage established:

1. **The seeded-solution hazard is current, not historical.** It reproduces
   identically on SCIP 10.1.0 (§2.1.1), through the crate and linking path a
   real build would use. Any seeded SCIP solve must disable weak dual
   reductions and must be replay-gated. This is a managed hazard, not a
   blocker.
2. **The nonlinear model closes the small worlds.** On the four closed worlds
   SCIP reports a zero gap in milliseconds to seconds, and its incumbents
   pass independent replay. Replay establishes feasibility, while the gap is
   the solver's certificate for the encoded model.
3. **The nonlinear model stops certifying as the world grows.** At 12-48 h of
   resource competition it returns incumbents 29-64% below its own bounds in
   a 60 s budget, and plain 48 h in the horizon sweep found no incumbent at
   all. "No incumbent" remains distinct from infeasible (a finite dual bound
   is reported throughout).
4. **The iterative method is the stronger incumbent generator and the weaker
   optimiser.** It beats SCIP's 60 s incumbents on every large world at a
   fraction of the time, produces a valid schedule within milliseconds, and
   reports no bound at all - and it can miss an optimum that requires
   cross-interval blending discipline (known-answer: 0 against SCIP's
   zero-gap 6000).

Recommendation: **proceed toward SCIP integration, with the iterative HiGHS
method as the incumbent generator inside the same contract.** The natural
shape is: `solve_iterative` produces a valid starting schedule fast; the
nonlinear SCIP model then searches and, where it finishes, certifies;
everything published passes `replay` regardless of which side produced it.
Seeding SCIP from the iterative result is exactly the hazardous path of
§2.1.1. The `allowweakdualreds = false` mitigation and replay gate are
available, but a seed must first be translated into an accepted SCIP starting
solution and measured under a shared budget. Replay cannot detect a false
optimality certificate. The Stage 5A worker runs SCIP unseeded.

The remaining difficulty is **not** blending, sequencing or grade-dependent
rules. It is backend integration and trust:

1. A warm start silently corrupts the answer on nonlinear models (§2.1,
   §2.1.1) - a correctness hazard with a known mitigation, so no SCIP result
   should ever be published without replay.
2. Cancellation uses a same-thread SCIP event handler and is now wired into
   the background worker (Stage 5A below). Event spacing limits response
   time during the solve; no cross-thread SCIP pointer is shared.
3. `bundled` downloads a prebuilt SCIP at build time. Acceptable for an
   experiment; a shipped build should link a system or vendored SCIP, and the
   `scip-system` path (verified here against Arch's 10.1.0 package) is what
   that looks like.

Required production work, explicitly out of scope here: full lifecycle/UI wiring,
packaging across desktop platforms (SCIP + SoPlex + PaPILO + TBB must ship or
link), project capture into `BlendInput`, publication into the schedule
timeline, and UI integration. Nothing is reachable from Run Schedule.

### Distinguishing the four claims

| claim | status here |
|---|---|
| A feasible schedule | Yes, for every scenario and both methods, confirmed by independent replay. |
| A solver-reported gap for the encoded model | SCIP reports zero on four closed worlds and 29-64% on the large resource worlds; the latter is an unclosed gap. Its 24 h parcel bound is valid but uninformative. The iterative method reports no blended-model bound or gap. |
| Proven optimality within tolerances | Yes for the closed worlds, including the branching instances of §7.2. Not established on any large world, by either method. |
| Remaining approximations | Receipt-release boundary mixing rather than continuous mixing; inventory at interval rather than segment granularity; the §6 chunk lifecycle assumptions, including no slot reuse; grade boundaries carry a documented margin; category-dependent reclaim rules unsupported. These are product approximations of the scenario, not solver limitations. |

### What is not done

- The parcel-via-MPS SCIP solutions are solver-reported only, not replayed
  (§7.1).
- No deliberately hard blended instance was constructed, so worst-case
  hardness is unmeasured in both directions (§7.5).
- Peak memory was not recorded.
- The hybrid pipeline (iterative incumbent feeding a seeded, mitigated SCIP
  certification) is the recommendation, not an implemented artefact: the
  comparison ran the two methods side by side, never chained. Stage 5A also
  keeps them separate.

## 10. Reproducing

### Stage 5A: in-process background execution (2026-09-21)

`app/scip_blend.rs` exposes `App::start_experimental_scip_blend` for an owned,
already captured `BlendInput`, `ScheduleRunInputs`, plan revision, and typed
`ScipSolveOptions`. It queues one worker on the existing bounded compute pool.
The worker validates input, builds the model, solves unseeded, extracts owned
rows, and runs the independent blended replay **once** before returning a
typed `ScipCompletion`. The application thread polls the existing job queue;
the `JobKey::Project` revision gate and the pending run identity plus schedule
input/plan revision gate decide whether it may retain the result. Replacing a
request cancels its predecessor; cancelling clears the pending request while
leaving the last validated result untouched. Project teardown and shutdown
use the same cancellation path. This developer entry is not called by Run
Schedule and does not capture project data; Stage 5B must provide that capture
and its complete semantic fingerprint.

The cancellation flag is shared with a russcip event handler. The handler
checks it on `PRESOLVE_ROUND`, `NODE_FOCUSED`, `LP_SOLVED`, and
`BEST_SOL_FOUND`, then calls `SCIPinterruptSolve` on the solver worker thread.
The callback borrows a live `Model<Solving>`; no SCIP pointer crosses threads
or survives model teardown. The local SCIP 10.1.0 header allows interruption
in presolving and solving, and [SCIP's event-handler documentation](https://www.scipopt.org/doc/html/EVENT.php)
describes invoking it in a callback. Preparation, extraction, and replay poll
the flag at loop boundaries. An event-free stretch inside SCIP remains
uninterruptible until its next callback, so this is cooperative cancellation,
not a strict deadline.

Focused developer checks against **russcip 0.10.0 / SCIP 10.1.0** measured:

| check | observed result |
|---|---|
| cancel before formulation | no model and no solve |
| cancel during a 1,000-interval formulation | stopped after six checkpoints; 0.55 ms from request to completion |
| cancel during an active 48-hour solve | `UserInterrupt`, one successful `SCIPinterruptSolve` call; 5.8 ms request-to-completion |
| 1 ms SCIP time limit on a 24-hour world | `TimeLimit`, no incumbent, `LimitNoIncumbent`; solver returned in 4.37 ms (3.37 ms over limit) |
| 250 ms SCIP time limit on the same world | `TimeLimit`, replay-valid 5,100 objective, `FeasibleLimit`; 0.10 ms observed overshoot |

These are measurements of this build and workload, not general latency
guarantees. `limits/time` covers SCIP solving; formulation and replay have
separate timings. The completion retains raw backend status, raw and published
objectives, primary bound/gap, numerical adjustments, model sizes, backend
versions, and event/approximation metadata. A replay breach or published
objective above the reported maximisation bound returns `ValidationFailure`;
the result is not usable. Replay checks feasibility and cashflow, while SCIP
alone supplies a bound and a solver termination status. The worker does not
clamp an inconsistent gap or infer optimality from a valid incumbent.

The tested native build linked the extracted Arch SCIP 10.1.0 package,
SoPlex 8.1, PaPILO, TBB, Bliss, Clusol, GMP/MPFR, zlib and readline through
`scip-system`; `SCIPOPTDIR` and the runtime library search path must point to
that installation. `scip-experiment` still uses bundled SCIP 10.0.2. Normal
builds and WASM exclude SCIP. macOS/Windows packaging and runtime dependency
validation remain Stage 5B or later work; no machine-specific library path is
embedded in source.

The small reproducible developer entry is the `app::scip_blend::developer_checks`
test module. Run it against an installed SCIP 10.1.0 as follows:

```sh
SCIPOPTDIR=<scip-prefix> LD_LIBRARY_PATH=<scip-prefix>/lib \
  cargo test --features scip-system -- app::scip_blend::developer_checks --nocapture
```

The test module remains because it is the requested developer execution and
cancellation check, including an actual in-solve interruption. Its scenarios
are synthetic and do not change the schedule dispatcher.

The scenarios and benchmarks live in the developer-only `#[cfg(test)]` module
`scip/scenarios.rs`. It is retained as the reproducible scenario surface for
this investigation. It is compiled only with a SCIP experiment feature and
never reached from Run Schedule. Benchmarks are marked
`#[ignore = "benchmark"]`; run a named comparison single-threaded in release:

```sh
cargo test --release --features scip-experiment -- scip::scenarios::compare_known_answer --ignored --nocapture --test-threads=1
```

For the system-SCIP path used in the verification stage (SCIPOPTDIR pointing
at an unpacked Arch `scip 10.1.0` package tree, `LD_LIBRARY_PATH` at its
`lib`):

```sh
SCIPOPTDIR=<scip-usr> LD_LIBRARY_PATH=<scip-usr>/lib \
  cargo test --release --features scip-system -- scip::scenarios::compare_known_answer --ignored --nocapture --test-threads=1
```

### Stage 5B: real-project capture and experimental runs (2026-09-21)

A project can now be captured, solved, independently replayed and retained
without `Run Schedule`, the dispatcher, the Gantt, the Calendar production
rows or the animation changing in any way.

**Entry point and data sources.** `Run experimental optimisation`, in the
Schedule Setup **Configuration** page's feature-gated Optimisation section,
calls `App::start_experimental_project_blend`. That takes an owned
`CaptureSnapshot` on the UI thread - a plan clone, the reserve-field list, the
destination views, and `Arc`s on the *same* cached planning snapshot and bar
readiness reports the ordinary run preparation reads - and does nothing else
there. The worker turns that into a `BlendInput` through
`app/commands/schedule_capture.rs`, polling cancellation in each substantial
loop, and then runs the Stage 5A solve, extraction and replay unchanged.
Nothing re-measures geometry, runs Solids, or reads a scene composite.

**Supported semantics.** Conditions on *dug*
material are fully supported: the contributing block-model rows' own values
are known at capture, so `DestinationRule::accepts` and
`CashflowRule::matches` are called exactly as routing calls them today.
Reclaim is different, because a blended pile's grade is a decision variable.
`DestinationRule::accepts_identity` and `CashflowRule::matches_identity` were
split out of the existing predicates (pure refactors: `accepts` is now
`accepts_identity && conditions`) so capture can ask the identity question
without pre-evaluating a grade it does not know. On that basis:

| Project construct applied to reclaim | Capture |
| --- | --- |
| No conditions | Supported |
| Lower, upper or two-sided bound on a mapped grade | Becomes a bound in a `GradePredicate`, with each endpoint's inclusivity retained |
| Category condition | **Refused**, naming the rule: categories are discarded by the blend |
| Condition on a field that is not a tracked grade | **Refused**, naming the field |
| Two rules admitting one destination under different conditions | Supported as separate OR alternatives; the gap between disjoint ranges stays closed |
| Conditional cashflow rule on a mapped grade | Supported as a signed additive contribution whenever its predicate holds |

No rule is ever silently widened, narrowed or substituted.

**Mixed dig blocks are proportional, not approximated.** A movement candidate
names one material, but a dig block is one physical volume with one completion
balance. A block whose captured rows disagree stays a single `GroundSource`
carrying one `MaterialShare` per distinct captured material, and the
formulation adds, per block and per execution segment:

```text
sum(movements of material m out of this block) = fraction[m] x extract
```

so removing 100 t of a 60/40 block removes 60 t and 40 t, in every segment
rather than merely over the horizon. The portions may have different eligible
destinations, truck classes and cashflow coefficients; a portion with no
candidate column contributes an empty sum and therefore forces `extract` to
zero, which is the intended "a blocked portion prevents that extraction unless
another eligible route exists". A portion no enabled rule accepts is refused at
capture, naming the block and the portion, because the rest of the block cannot
be dug without it.

The independent replay recomputes the same fact from the published movement
tonnes alone, cell by cell, against the *measured* block composition - not
against the model's own rows. Removing the formulation's proportion rows makes
the acceptance case below fail in the replay, which is how that independence
was checked.

`CaptureStats::mixed_blocks` counts the blocks that hold more than one
material, and the result summary states that each was dug in its measured
proportions.

An earlier revision of this stage captured such a block as *ordered
sub-blocks*, one per material, dug in capture order. That was wrong: capture
order is not an authored mining sequence, and it let a loader take one material
out first, changing early crusher feed, stockpile grades, cashflow and
potentially the optimum. It is recorded here because the proportion rows exist
to prevent exactly that.

**Acceptance case: a mixed block that only partly pays.** One block of 2,000 t
holds 1,000 t at 62% Fe, which a rule sends straight to the crusher at +60/t,
and 1,000 t at 50% Fe, which only the dump accepts at -20/t. The loader is cut
to 50 t/h so a 24 h horizon stops it partway. The solve mines 1,200 t - exactly
the rate limit - as 600 t of each material, and the 800 t left in the block is
still half and half. The solver cannot take the attractive portion alone, and
it cannot change what remains.

**Stockpile representation.** Chosen per pile on its Setup page, defaulting to
*Not configured*, which blocks capture of a pile the run uses and blocks
nothing else. *Blended pile* combines the authored opening lots by tonnes and
contained quantity and states that lot order no longer applies; *Ordered
blended chunks* turns each authored opening lot into a closed, immediately
reclaimable chunk of its own composition and fills the configured receiving
chunks behind them in order. Authored lots are never rewritten or deleted, no
chunk count is inferred from a truck payload, and the non-reuse of emptied
slots is stated in the summary rather than worked around.

**Grade units.** Required explicitly per grade field, fraction or percent,
never inferred from a name or a magnitude. Only fields a run needs are
required; an unrelated unmapped field blocks nothing. `GradeField::accept`
still enforces that a grade is a weighted average *by the schedule's own
tonnage field*.

**Run identity.** `App::experimental_blend_key` replaces Stage 5A's provisional
identity. It folds in the run options, the Setup gate's inputs, the bar
revision, the reserve-field *definitions*, the reclaim half of the fleet, every
destination's kind/capacity/distance/crusher budget/opening inventory, the
routing, trucking and cashflow rule contents, and the whole experiment
configuration - all by stable id, and with no presentation name. It is built
from the project rather than by re-running capture, because a currentness check
has to be affordable; the capture's own `fingerprint` walks the *built* model
and is reported as the model identity. A verified consequence: renaming a
stockpile, a loader, a bar or the currency leaves both unchanged, while a
cashflow coefficient or the calendar resolution changes both.

**Measured end to end.** The developer fixture (ordered two-block dig
sequence, one reclaim bar capped at 800 t, one truck class, a stockpile with
1,000 t of 58% Fe opening stock, a crusher on a 6,000 t/day budget, a dump,
and a 60% Fe grade-dependent reclaim route) captures and solves to proven
optimality in ~2 s wall time at a 24-hour horizon and 4-hour resolution.
Reconciliation: 3,500 t mined (exactly the measured ground), 800 t reclaimed
(exactly the authored cap), 800 t processed, closing pile 1,792.3 t inside its
5,000 t capacity, objective 39,400.00 - which is `800 x (60 - 2) - 3,500 x 2`
to the cent. The replay reports no physical and no grade issue.

**Practical limits.** Capture refuses rather than allocates when the horizon
and resolution would build more than 4,000,000 movement columns, and reports
the estimate either way. The per-interval execution-event budget is derived by
the same union-bound rule the accepted backend uses and is held to the same
`SEGMENT_CEILING`; when the clamp binds, the summary says so.

**Remaining Stage 5C work.** Application cutover: feeding a validated
experimental result into the Gantt, the Calendar production rows and the
animation; a publication path from `BlendSolution` to the dispatcher's
`Movement`/`ExecutionSegment` vocabulary; and desktop packaging of the SCIP
dependency.

Multi-stockpile reclaim authoring, upper and two-sided grade bounds, and
conditional reclaim cashflow are implemented in the experimental path. The
older benchmark tables above predate these additions. Category conditions on
stockpile or mixed source selections remain unavailable because the blend has
no category inventory. The conditional cashflow formulation excludes a narrow
band around each grade boundary to make negative contributions mandatory;
`GRADE_MARGIN` in `blended/input.rs` documents its width and replay checks the
authored endpoints. These are experimental semantics until the Stage 5C
application cutover.

## 11. Scenario definitions

Focused scenarios (all replay-gated):

| # | scenario | what it pins down |
|---|---|---|
| 1 | 1,000 t at 60% Fe receiving 500 t at 50% and 500 t at 70% | the blend stays 60% |
| 2 | pile opens empty, reclaim pays 100x dig; dig into an empty pile with reclaim available same interval | no phantom reclaim or contained metal; receipts cannot be reclaimed before release |
| 3 | 1,000 t at 60% Fe / 5% SiO2, reclaim capped at 400 t, two grades | the remaining 600 t holds both grades exactly |
| 4 | 1,000 t at capacity, dig pays 10/t in, reclaim pays 1/t out, one interval | **a full pile can receive while it is reclaimed** (the §3 occupancy regression) |
| 5 | one loader, two bars covering one interval: cheap block priority 0, rich block priority 1 (100x value) | authored bar priority is mandatory (worth 500, not 50,000) |
| 6 | a bar whose window covers only part of a calendar interval | containment convention: partial windows do not win the interval |
| 7 | crusher minimum 60% Fe, pile opens at 50%, only 80% supply available | the optimiser must choose enough high-grade supply for later reclaim to qualify |
| 8 | dig and reclaim both routed to one 600 t/day crusher | the shared budget binds, not the two loaders' own capacity |
| 9 | authored sequence puts a worthless block before a 10x valuable one | economics cannot reorder authored work |
| 10 | seeded-solution probe (§2.1.1) | the warm-start anomaly, per configuration, on the linked build |

Comparison worlds (§8.2; both blended methods receive them unchanged):

| world | shape | what it forces |
|---|---|---|
| known-answer | 3 h, pile opens 400 t at 50% Fe, crusher needs 62%, only 80% supply, dig and reclaim at 300 t/h | the schedule must *dig first and wait* for the blend to rise before reclaim pays |
| threshold-trap | 8 h, pile opens exactly at the 62% minimum, only supply is 55%, dig pays a little on its own | every received tonne pushes the blend under the boundary; the solver is torn between filling and keeping the paid route |
| dynamic chunks (FIFO and LIFO) | 12 h, empty four-chunk pile (300 t each), 45% and 85% supply, shared 700 t/day crusher, tight trucks | the solver chooses what enters each chunk, when to close it, and which closed chunk to draw |
| resource competition | 12/24/48 h, two piles (opening 48% and 65%), four loaders, both reclaims capped at 600 t, one 900 t/day crusher, 260 truck-hours/day | direct mining and two reclaims compete for one fleet and one budget across horizons |

Horizon sweep (§7.2): three dig loaders feeding one blended pile plus one
reclaim loader, one-hour intervals, two segments per interval, 4/8/12/24/48
hours, plain and graded (62% Fe minimum) variants, 60 s budget, release
build. Chunk sweep (§7.3): alternating lean/rich chunks, 2/5/10 chunks,
FIFO and LIFO, same horizons.
