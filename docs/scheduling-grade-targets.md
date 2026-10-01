# Scheduling grade targets

Soft grade targets price the tonnes-weighted blend a crusher receives each
day. They steer routing and mixing while leaving deliveries outside the band
feasible. Routing grade conditions remain hard admission rules; a stockpile's
or dump's grade is controlled through routing, so only crushers take targets.

## Setup

Tick **Track {grade}** in **Configuration → Optimisation** for each grade to
blend, target and report. The schedule uses each field's stored numbers
directly, with no percentage or fraction setting: enter targets on the same
scale as the data (`62` for data stored as `62`, `0.62` for `0.62`).
Multiplying a field and its targets by the same factor gives the same
schedule. Untracked fields stay in the project.

Under each **crusher in the Calendar**, each tracked grade has one row: the
grade received that day. Click its label to unfold Target, Lower limit, Upper
limit and Penalty rows beneath it; they start folded. The Default column applies
to every day; fill a day cell to override it. Blank day cells inherit Default;
`None` removes a value for that day. Typing, rectangular paste and clearing use
the normal Calendar controls. A pasted band is validated as a whole and applied
in one undo step; an invalid paste changes nothing.

Penalties are currency per delivered tonne at a limit, and the slope doubles
beyond it. A missing target or both missing limits leaves the day inactive, so
cells can be filled one at a time. A missing penalty means zero. Lower must be
below target and upper above it. A blank lower or upper limit makes a one-sided
band.

Days are aligned to project hour zero; the final day may be shortened by the
horizon. Targets measure receipts, direct mining and reclaim together.

Each edit is undoable, saved with the project and changes the calculation's
identity, so the schedule recalculates automatically. Old projects load with no
targets. Untracking a grade keeps its targets but stops pricing and showing
them until it is tracked again. Deleting a crusher removes its targets in the
same undo step.

## Penalty

For delivered tonnes `T`, contained quantity `Q`, target `g`, band limit `l`,
penalty at the limit `P`, and outside multiplier `m`, each enabled side adds:

```
lower side: P / (g - l) * [max(g*T - Q, 0) + (m - 1)*max(l*T - Q, 0)]
upper side: P / (l - g) * [max(Q - g*T, 0) + (m - 1)*max(Q - l*T, 0)]
```

Capture keeps numeric grades unchanged and carries tonnes × grade through
blending; the money remains on the schedule's tonnage basis. The curve is continuous. A 60 / 62 / 66 percent target
with a $5/t penalty and multiplier 2 gives:

| Delivered Fe | Penalty per tonne |
| --- | ---: |
| 58% | $15.00 |
| 59% | $10.00 |
| 60% | $5.00 |
| 61% | $2.50 |
| 62% | $0.00 |
| 64% | $2.50 |
| 66% | $5.00 |
| 70% | $15.00 |

Costs apply to the day's combined blend, not to individual parcels. No
receipts means no penalty. Multiple grade specifications add their costs.
Beyond a limit the cost continues increasing at the configured steeper slope.
There is no discontinuous jump at a limit.

## Calculation and verification

Hourly dispatch prices the running blend through the current interval. Its
production credit covers a bound on the marginal grade cost so a target alone
cannot make it park a loader. It has no look-ahead, so early receipts can miss a
target even when later receipts balance the complete day.

Improve maximises movement value less the full daily penalties. Without
sufficient positive movement value, it can prefer less production; configure
cashflow rewards when production has economic value. The production credit used
by hourly dispatch is never included in the reported objective.

The common formulation uses linear positive-part deviation columns. Reclaim
content is allocated to each routed movement at the pile's delivered blend;
this allocation uses the existing nonlinear mixing relationship in SCIP and
the estimated blend in the optional fixed-grade backend. Chunked piles use the
blend actually drawn from eligible chunks, rather than the whole pile average.
The relaxation retains the linear deviation and content-conservation rows,
while dropping mixing as it already does for stockpile physics.

Day-by-day solving carries tonnes and contained quantity for each target day. Window objectives charge the change in penalty since their opening,
allowing a later window to recover a cost paid earlier. Look-ahead receipts are
discarded along with the look-ahead schedule. The stitched schedule is scored
again over the complete horizon, so each day is charged once.

Independent replay derives content from source material and actual pile/chunk
draws, aggregates receipts and recomputes the penalties. The published objective
uses this replayed cost. Objective reconciliation prices the physical
contained-tonne feasibility tolerance at the target slopes; this numerical
allowance is recorded separately and included in bound comparisons.

## Results

A crusher's grade row shows each day's received tonnes-weighted grade, direct
mining and reclaim together, even without a target. Zero receipts, uncovered
days and a stale result stay blank; a partly covered final day carries the
Calendar's partial marker. With a target, the figure is green inside the limits
and in the warning colour outside, and its hover leads with the day's penalty
and cost per tonne, then the band.

The Calendar's existing movement-value row remains gross movement cashflow;
target costs are reported in grade hovers and subtracted from the schedule's
published optimisation value.

## Validation

Temporary checks covered Calendar defaults and daily overrides, atomic paste
rejection, persistence, capture, hourly dispatch, full SCIP and rolling carry,
and scale equivalence: the same blend and target stored as 0.62, 62 and 620
gave the same penalty and solver value. They were removed under repository
conventions.
