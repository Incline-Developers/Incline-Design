# Scheduling optimisation backend — formulation and performance notes

Historical parcel-HiGHS formulation notes. Normal desktop runs now use the
SCIP blended model, not this parcel baseline. See
[Scheduling SCIP integration](scheduling-scip-integration.md) for current
GUI integration, settings and limitations.

Developer notes for `src/model/schedule/optimisation/`. The module docs in
`highs.rs` carry the semantics; this file carries the *reasoning* behind the
performance work and the measurements it rests on, so a later reader can tell
which choices were forced by evidence and which are open.

Scope of this document: Stage 4D, "make reclaim practical over a scheduling
horizon". The formulation semantics accepted in Stage 4C are unchanged.

## 1. What the model is

Calendar intervals carry input constancy. Execution happens in *segments*:
ordered event positions inside an interval, shared by every loader, whose
durations are decision variables summing to the interval. Stockpile inventory
is held as discrete **units** — opening lots and receipt parcels — and
FIFO/LIFO is an exact linear conjunction chain over those units, evaluated at
every segment boundary.

That last sentence is the whole performance story. Writing `cell` for one
(interval, segment) pair, the inventory block costs

    units × cells   where units ≈ positions-per-interval × intervals
                    and cells = intervals × segments

so it grows with the **square of the horizon**, while the dig-only model grows
linearly. Four column families scale that way — the unit balance, the
emptiness indicator, the ordering chain, and the per-unit draw — and each
reclaim draw needs a destination movement as well.

## 2. What changed in Stage 4D, and why

### 2.1 The tie-preference objective (the largest single win)

Routing preference, shorter haul cycles and "fill earlier receipt positions"
used to be added to the objective with a coefficient small enough that their
total possible effect stayed below the documented objective tolerance. The
bound was a sum over every movement and parcel column, so the coefficient fell
to about **1e-13** beside primary costs of 1 to 2.

That is not a tie-break in double precision; it is noise on thousands of
columns. HiGHS warns about it ("excessively small costs"), and the root
relaxation becomes so degenerate that the cut loop never terminates. On the
eight-hour audit fixture the solver spent a full sixty seconds at the root,
generated 5,792 cuts, explored **zero** nodes, ran **zero** heuristic
iterations and returned no incumbent at all. Removing the term entirely —
purely as an experiment — produced a validated incumbent in the same budget.

Preferences are now a **second lexicographic stage**: stage one maximises
movement value; stage two re-costs the same model with the preference
coefficients at their natural scale, under a row holding the primary objective
at what stage one achieved (to `tolerances.objective`), warm-started from
stage one's solution. The stage is bounded in wall time and skipped when the
budget is spent, which is why `SolveSummary::tie_preferences_applied` exists:
a false flag means the rows are primary-optimal but the preference ordering
among equally valuable alternatives was not applied. That is a time-budget
outcome, never a correctness one.

This is strictly stronger than the epsilon it replaces: the epsilon could in
principle move the primary objective by up to the tolerance, the staged form
cannot move it at all.

### 2.2 Interior-point root relaxation

With the simplex root LP the cut loop still does not terminate on longer
horizons: the dual bound is reached by the first relaxation and never moves,
yet HiGHS keeps separating — ten thousand cuts, zero nodes, zero heuristics.
The degeneracy is structural (thousands of zero-cost indicator columns over a
shared event clock), not a scaling accident.

`mip_lp_solver = ipm` steps around it. The twelve-hour fixture goes from *no
incumbent in sixty seconds* to a validated solution inside 2% of the bound —
proven optimal in 28 seconds on some runs, since a time-limited MIP solve is
not reproducible run to run. This is the
one solver option chosen on behavioural evidence rather than formulation
reasoning, and the evidence is in the log: rounds of separation with a
constant bound.

For the record, options that made **no** difference and are not set:
`presolve=off`, `mip_heuristic_effort=0.5`, `mip_detect_symmetry=false`,
`run_crossover=off`, `ipm_optimality_tolerance=1e-4`. Relaxing the pinned
`primal_feasibility_tolerance` from 1e-9 to the HiGHS default also made no
difference, so the pinned value costs nothing and stays.

### 2.3 Warm start from a restricted solve of the same model

The pinned `highs` wrapper exposes neither re-costing nor a starting solution,
but `highs-sys` does (`Highs_changeColsCostByRange`, `Highs_addRow`,
`Highs_changeRowBounds`, `Highs_setSolution`), and the good_lp column order is
the HiGHS column order. So: **yes, the backend accepts a feasible starting
solution**, and no second scheduling engine was needed to produce one.

The seed is the same model with one block of rows holding every interval to
its first execution segment — the conservative schedule with no within-interval
source transitions. Its solution is feasible for the unrestricted model by
construction (same columns, strictly more constraints), so no repair or
validation of the seed is required. After the seed solve those rows are
relaxed in place and the solution is handed back as a starting point.

Measured on the eight-hour fixture: without the seed, 3.7% gap at the limit;
with it, proven optimal in 22 seconds.

Two seeds that did **not** work are worth recording. Forcing reclaim off as
well produced a much easier seed and got the twelve-hour case its first
incumbent before IPM was found — but it misleads the search once IPM is in
play (eight hours regressed from optimal to a 33% gap). Forcing *all* activity
off solves in 0.11 seconds and is useless: HiGHS never improves on it.

### 2.4 Recursive state instead of cumulative rows

Three families were written as "state at this boundary = initial value less
everything before it": ground remaining, receiving capacity, and stockpile
inventory. Each wrote a row whose length grows with the cell index, so the
matrix grew with the square of the horizon. At twenty-four hours these were
most of the nonzeros in the model.

They now carry state from the previous boundary. The two forms are
algebraically identical. Nonzeros at twenty-four hours: **776k → 354k**.

### 2.5 Smaller structural reductions

- A draw that takes its whole unit-material share down exactly one route *is*
  that route: the movement reuses the draw's column instead of a second column
  tied to it by an equality. On the audit fixture that is every receipt draw.
- The per-cell closing balance column is gone; a unit's carried balance is an
  expression. Its implicit lower bound of zero is restated as one row on the
  last cell, which is the only cell with no successor equality to enforce it.
- Receipt positions are sized from the loaders that can actually reach the
  pile — a loader with no dig task, or no authored route into that stockpile,
  no longer inflates the bound. This sizes every inventory unit the pile
  carries for the rest of the horizon.
- A stockpile no task can reclaim from carries no ordering state at all. Only
  its inventory total is needed, for capacity.

Combined effect at twenty-four hours: **67,008 → 37,824 columns**, 134,768 →
95,176 rows, 776k → 354k nonzeros.

### 2.6 Publication: the negligible-draw rule

Two defects in the same rule surfaced once the solver started exploring
different corners of the feasible region. Both are now decided by the balance
a draw *leaves*, not the one it finds:

- A unit holding twice the dust scale can be retired by a draw of exactly the
  dust scale, leaving a residual the model calls empty. Dropping that row as
  "negligible" left the published unit holding its whole parcel, and the
  replayed FIFO/LIFO order then demanded the phantom be reclaimed before
  anything newer.
- Conversely a selection binary may sit an integrality tolerance away from
  zero, so the model can carry a sub-picogram draw against a unit the order
  has *not* reached. Publishing that row claimed a loader dug out of turn.

A third, separate defect: the snapping pass would inflate *any* row that came
within three dust of its unit's balance up to that whole balance — including a
1e-15 row against a 1e-4 unit. Rows that do not publish no longer consume
published balance either, and every dropped tonne is recorded in
`PublicationAudit`.

## 3. Formulation note: receipt positions per interval (Stage 4D §1)

This was investigated and **not implemented**. The reasoning and the numbers
are below so the decision can be revisited.

### 3.1 The exact construction

Receipt positions are provisioned per (stockpile, interval, **segment**), and
each group is sized for the whole interval's production, because segment
durations are decision variables. So `S` groups each hold `P` positions and
the pile carries `S × P` inventory units per interval.

An exact per-interval mapping exists. Keep the parcel groups and every one of
their constraints untouched, and add one ordered **slot list** per (stockpile,
interval) of length `C`:

- offset selectors `o[g][k]`, binary, one per segment `g > 0` and candidate
  offset `k ∈ 0..=C`, with `Σ_k o[g][k] = 1` and `Σ_k k·o[g][k] = Σ_{g'<g} c_g'`
  where `c_g'` is the number of assigned positions in group `g'`;
- transfers `x[g,p,k] ≥ 0` for each group position `p` and slot `k = offset+p`,
  with `x ≤ target·o[g][k−p]`, `x ≤ quantity[g,p]`, and
  `x ≥ quantity[g,p] − target·(1 − o[g][k−p])`;
- slot quantity `Q[k] = Σ x[g,p,k]`, and slot composition by material likewise.

Why it preserves the semantics: group `g` occupies the half-open slot range
`[o_g, o_g + c_g)`, and those ranges tile in segment order, so the map is
order-preserving and injective on assigned positions — the FIFO/LIFO sequence
over slots is exactly the old segment-major, position-minor sequence.
Unassigned positions carry zero quantity, so slots they collide with receive
nothing. Every parcel constraint is untouched, so fixed composition, one
remainder per stream per group, delivery causality and bounded-discrepancy
interleaving among overlapping supplies are unchanged. Release is unchanged:
an interval's slots release at the end of that interval, as its groups did.

`C` must bound the total assigned positions of an interval:
`ceil(interval receipt bound / parcel target) + one remainder per (stream,
segment)`.

### 3.2 The conflict in the simpler variant

The obvious simpler design — a single interval-wide position list whose slots
are tagged with the supplying segment, with no per-segment group — **cannot**
preserve the interleaving rule, and this is the specific conflict to report.

Interleaving spreads each stream's tonnage proportionally across the *group's*
positions, within one parcel of its share at every prefix. Causality confines
a segment's parcels to a contiguous block of the interval. With two streams of
equal total the two demands meet exactly at the allowance; with three streams
supplying in different segments they are jointly infeasible — fully segregated
blocks put a stream's whole tonnage in the first third of the order, a
discrepancy of two thirds of its total against an allowance of one parcel.

Making the block boundaries decision variables does not rescue it: the
discrepancy rows multiply the block's occupied count by the stream's prefix
tonnage, and both become variables, so the rows turn bilinear. Linearising
them costs more than the units they save.

### 3.3 Why it was not implemented

The saving is exactly the duplicated *tonnage* capacity,
`(S − 1) × ceil(bound / target)` units per interval. The remainder allowance
is identical in both forms. On the audit fixture (S = 2, bound 120 t, target
50 t) that is 12 units per interval against 9 — about **25%**.

Against that, the measured ceiling is not 25% away. At twenty-four hours with
a 200 t parcel target the model is 25,824 columns / 11,232 binaries / 62,964
rows — **smaller than the twelve-hour model that solves to a 1.8% gap** — and
it still returns no incumbent. Horizon length, not unit count, is what the
solver is failing on. A 25% unit reduction cannot close that, and it would be
a large, numerically delicate change (big-M transfers at parcel scale) to a
part of the model that is currently correct.

Recommendation: revisit only together with a decision on §5 below.

## 4. Benchmarks

Audit fixture: three dig loaders at 40 t/h feeding one 1,000 t FIFO stockpile,
one reclaim bar at 40 t/h, one-hour intervals, crusher budget 300 t/day,
60-second solve budget, single-threaded. "Issues" is the count from the
independent `validate_solution` replay.

| Horizon | Parcel | Order | Status | Objective | Bound | Gap | Cols | Binaries | Rows | Formulate | Solve | Issues |
| ---: | ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 4 h | 50 t | FIFO | Optimal | 900 | 900 | 0 | 1,504 | 800 | 4,356 | 3 ms | 1.2 s | 0 |
| 4 h | 100 t | FIFO | Optimal | 900 | 900 | 0 | 1,304 | 696 | 3,554 | 2 ms | 1.1 s | 0 |
| 8 h | 50 t | FIFO | Optimal | 1,120 | 1,120 | 0 | 4,928 | 2,368 | 13,304 | 10 ms | 21.9 s | 0 |
| 8 h | 100 t | FIFO | Optimal | 1,120 | 1,120 | 0 | 4,208 | 2,032 | 10,934 | 7 ms | 13.4 s | 0 |
| 12 h | 50 t | FIFO | Feasible | 1,120 | 1,140 | 1.8% | 10,272 | 4,704 | 26,860 | 30 ms | 60.2 s | 0 |
| 12 h | 100 t | FIFO | Feasible | 1,120 | 1,140 | 1.8% | 8,712 | 4,008 | 22,154 | 22 ms | 60.0 s | 0 |
| 24 h | 50 t | FIFO | No incumbent | — | — | — | 37,824 | 16,320 | 95,176 | 382 ms | 93.6 s | — |
| 24 h | 100 t | FIFO | No incumbent | — | — | — | 31,824 | 13,776 | 78,854 | 270 ms | 61.8 s | — |
| 48 h | 100 t | FIFO | No incumbent | — | — | — | 121,248 | 50,592 | 295,886 | 3.8 s | 60.1 s | — |
| 4 h | 50 t | LIFO | Optimal | 840 | 840 | 0 | 1,504 | 800 | 4,356 | 2 ms | 3.6 s | 0 |
| 8 h | 50 t | LIFO | Optimal | 840 | 840 | 0 | 4,928 | 2,368 | 13,304 | 10 ms | 10.4 s | 0 |

Reading it:

- Everything that returns a solution passes the independent replay. There is
  no row where the model solved and the published timeline did not reconcile.
- The acceptance target — a validated feasible twenty-four hour result inside
  sixty seconds — is **not met**. Twelve hours is met, within 2% of the bound.
- Formulation time is never the constraint: 382 ms at twenty-four hours,
  3.8 s at forty-eight.
- Parcel target moves size by about 15% and does not change any verdict. That
  matters: it means the discretisation knob is not quietly deciding these
  results.
- LIFO is cheaper here than FIFO and reaches a lower objective, which is the
  fixture's own arithmetic (LIFO strands the opening lot behind later
  receipts), not a modelling asymmetry.
- The twelve-hour rows show `tie_preferences_applied` false: the budget was
  spent on stage one, so those answers are primary-optimal without the
  preference ordering applied. See §2.1.
- HiGHS checks its time limit between root separation rounds, so the
  twenty-four hour row overshoots its budget by about half again. Up to twelve
  hours the overshoot is nil.

Before Stage 4D, for comparison, the same fixture returned **no incumbent at
eight hours and beyond**, and eight hours is now solved to proven optimality.

<!-- Produced by a temporary #[test] (removed per the repository's
     no-standing-suite rule): build the fixture described above at each
     horizon and parcel target, solve with a 60 s limit, and print the
     summary statistics plus validate_solution's issue count. Times are
     single-threaded on the development machine, and a time-limited MIP
     solve is not reproducible run to run, so treat the status column as the
     signal and the seconds as indicative. -->

## 5. Remaining limit, and the smallest product approximation

**Dominant remaining structure.** HiGHS's root separation loop. It is bounded
by `2·sqrt(maxTreeSizeLog2)` rounds — roughly the square root of the integer
column count — and every round re-solves the root LP. At twenty-four hours
that is around 250 rounds against an LP of 82k rows, so the root alone
outlasts the budget: zero nodes are ever explored and no heuristic ever runs.
Integer columns therefore cost twice, once in the LP and once in the round
count.

**Are useful incumbents available?** Up to twelve hours, yes, and validated.
At twenty-four hours, no — and note that "no incumbent" here is genuinely
distinct from infeasibility: the dual bound is finite and correct (1,140 on
the audit fixture), the primal side simply never starts.

**The one relaxation tried and rejected.** The ordering chain column is
implied integral by its own AND rows, so it need not be declared binary; doing
that removes two fifths of the model's integer columns. It is not worth it:
the branching it supports is worth more than the smaller search space, and the
twelve-hour fixture went from a 1.8% gap to 78% in the same budget. Recorded
here because it looks like an obvious win and is not.

**Smallest explicit product approximation that would help.** Allow **one
remainder per receipt stream per interval** instead of one per stream per
segment. Today a stream delivering 10 t in each of two segments makes two
part-filled parcels; the change merges them into one 20 t parcel, placed by
the later of the two segments so nothing supplied late moves earlier in the
receipt order.

- Effect on FIFO/LIFO and ordering: none. Composition stays fixed to one
  source and material, release timing is unchanged, and the order is still
  segment-major.
- Effect on optimality: it is a *restriction*, so the achievable objective can
  fall. It cannot rise.
- Effect on size: it removes the remainder allowance from the slot count, so
  combined with §3.1 the audit fixture goes from 12 units per interval to 6 —
  a halving, rather than 25%.

It is a product decision, not a modelling one, and has not been made.

**What would help more than any of this** is a horizon strategy: the backend
is proven correct and tractable at twelve hours with reclaim, and a Run All
Periods pass that solves a shift at a time with carried-forward inventory
would stay inside that envelope. That is a scheduling-product design question
and explicitly out of scope here.
