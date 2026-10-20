# A long-term plan for Improve, 2026-10-20

Goal (from the user): one horizon-wide plan that sets targets the way the
digger targets already do, so window plans and the dispatch follow a
general long-term intent instead of case-specific incentives. Find out what
targets it can set, what it must leave out to solve quickly, and what it
can afford to include.

Everything below is offline (temporary tests on top of `ae064a2`), not
integrated into Improve.

## What the plan model already covers

`scip/plan.rs` models, per loader, block and period: dug tonnes along each
loader's authored sequence, finished flags and order, reach at full rate,
blast releases, each material's split between destinations, crusher
budgets, dump and pile room, pile inventory, reclaim per bar, pile and
period within reclaim rates and caps, loader hours shared by dig and
reclaim, pooled fleet truck hours, daily grade targets (penalised hinges)
and grade-conditional values. It leaves out: anything inside a day, truck
classes (pooled fleet), per-loader values, FIFO chunks, rests, reclaimed
blends' route qualifications. A reclaim's blend is fixed per plan.

## Building it

- `plan::long_term(input, seed, period_days, settings)`: the whole horizon
  over periods, in target mode (not the bound's optimistic reading), as a
  MIP or its LP. It needed the bounding switch separated from "relaxation
  without a window", and `PlanSolve::inventory` (each pile's tonnes at each
  period end).
- **Blocks are too many.** On the long world, 14-day periods give 209k
  columns, 193k of them route splits (block × material × destination ×
  period); the LP took 180 s and the MIP found nothing in 120 s.
- **Parcels fix the size.** The parcel aggregation from the removed
  continuation experiment (blocks with the same releasing blast, the same
  blasts above and the same loaders become one; materials one per routing
  class) is back as `scip/parcels.rs`, keeping the held blast timeline. The
  long world at 14-day periods: 3.9k columns, LP 0.3 s, MIP 3 s. Targets
  are by loader, destination and day, and piles by id, so they carry over
  to the real blocks unchanged.
- **Bars must be read apart over the whole horizon.** With a loader's bars
  joined into one queue (the window plans' reading), the long world's plan
  is worth *less* than its own first schedule (675M against 692M): a
  waiting bar holds the loader for weeks. Read per bar (as the bound
  does), it is worth 709–716M.

## First results: one dispatch following the plan

The whole horizon dispatched once to the plan's digger targets (each
period's tonnes spread evenly over its days), with the three existing
weightings, held timeline and live chain. Against the first schedule:

| World | Plan | Columns | Solve | Best single dispatch | Improve's 60/120 s search |
|---|---|---|---|---|---|
| Centralia, joined bars | 7-day MIP | 2.9k | 0.7 s | +1.15% | +1.24% |
| Centralia, joined bars | 4-day MIP | 4.4k | 3.1 s | +1.08% | |
| Long world, per bar | 7-day LP | 9.0k | 2.3 s | +0.195% (live) | +0.17% |
| Long world, per bar | 7-day MIP | 9.0k | 11 s | +0.168% (live) | |

One plan solved in seconds and one dispatch match what the whole search
gets today. On the long world the plan promises +2.5% that the dispatch
does not realise: how closely the dispatch can follow it is the next
question. Held-timeline dispatches are mostly rejected (ground standing at
a fixed clearance); live-chain dispatches always replay.

## The other worlds (one dispatch, best of the weightings)

| World | Joined bars | Per bar | Improve's search |
|---|---|---|---|
| 14 days | +2.65% | +3.68% (7-day MIP) | +2.86% |
| Centralia | +1.15% | +1.25% (4-day MIP) | +1.24% |
| No blasts | +0.73% | +0.72% | +1.26% |
| Grades | +0.16% | +0.22% | +1.19% |
| Trucks 60% | +0.36% (live) | +0.37% (live); most −2 to −8% | +2.63% |

The follow-only weighting (no overrun cost) is nearly always best; the
overrun weightings can lose 5–15%. On trucks 60% held-timeline dispatches
are always rejected. Plans solve in 0.1–35 s (MIP), 0.0–5 s (LP).

## Integrated: the long-term plan replaces the background whole-horizon plan

The background whole-horizon MIP (blocks by day) never finished on the long
world or trucks 60%. It is replaced by long-term plans in parcels, read per
bar, as MIPs over 7- then 4-day periods (`LONG_TERM_DAYS`), each tried as it
arrives: its targets for the whole horizon, every weighting, held timeline,
then the chain live; kept like any other improvement, after which its
targets are the base each window replaces locally. A plan arriving mid-pass
interrupts the pass, which carries on from the same window afterwards.

| World | `ae064a2` | Long-term plan |
|---|---|---|
| Centralia | +1.243% | +1.467% |
| 14 days | +2.856% | +3.683% |
| Trucks 60% | +2.628% | +2.628% (arrives, never kept) |
| Grades | +1.190% | +1.158% |
| No blasts | +1.261% | +1.427% |
| Long world | +0.171% | +0.178% (kept at 31 s) |

Re-run after removing the temporary code (uncommitted working tree): the
same to within timing (Centralia +1.503%, the rest identical).

## Why the dispatch falls short of the plan (long world)

Following the 7-day plan, per period: the crusher is full (280 kt/week) in
plan and dispatch alike, so the plan's +2.5% is not crusher tonnage. The
dispatch departs most on loader 1 in days 70–91 (plan 504 kt/week, dispatch
60–263 kt): its bars wait on blasts. The plan takes releases from the held
timeline, which its own different digging no longer clears in time, so the
live chain fires them later. Letting the plan decide blast timing (the
bound's blast rows, earliest releases) made loader 1 follow it, but the
plan (726M) became too optimistic and the dispatch lost 0.3%: removed.

## Further experiments, all removed

| Experiment | Result |
|---|---|
| Windows held to the long-term plan's pile stock at their end (two-sided, priced at the value spread) | Neutral: Centralia +0.08, grades −0.04, no blasts −0.05, long −0.01 points |
| A third, 2-day plan after 7 and 4 | Never arrives in time; identical results |
| The plans' LPs instead of MIPs | Mixed: long world +0.195% (better), 14 days 3.45% and trucks 2.44% (worse) |
| Follow weight ×0.25 to ×8 (overrun 0) | ×1, the existing weight, is best or near it everywhere |
| The plan deciding blast timing (bound's blast rows) | Plan too optimistic (726M); dispatch −0.3% |

## What the plan includes, what it leaves out, and what it costs

| Feature | In the long-term plan | Cost / effect |
|---|---|---|
| Digging along each loader's authored bars | Yes, per bar | Joined bars made the long world's plan worth less than its own first schedule |
| Blocks | As parcels (same releasing blast, blasts above, loaders; one material per routing class) | 209k → 3.9k columns on the long world; targets carry over unchanged |
| Periods | 7 and 4 days | 7-day solves in 1–13 s; 2-day too slow to matter within 60–120 s |
| Blast releases | Held timeline, fixed | Deciding them made the plan optimistic and the dispatch worse |
| Crusher budgets, dump and pile room, pile inventory | Yes | Pile trajectory available (`inventory`), but as window targets it did nothing |
| Reclaim per bar and pile | Yes, fixed blend (optimistic reading over the horizon) | Likely why grades gains nothing |
| Fleet | Pooled hours per period | Every test world has one truck class; fleet is binding on trucks 60% from week 2 |
| Grade targets | Daily targets kept for a period only where every day shares one | |
| Inside a period | Free order, ground released by the period's end counts all period | The main optimism: trucks 60% plans 247 kt crusher feed in week 1 where 171 kt is possible |

Where the dispatch departs from the plan:
- **Long world:** loader 1's bars wait on blasts the plan assumed fired on
  time (its digging differs from the schedule that set the held timeline).
- **Trucks 60%:** the fleet is fully used from day 14 in plan and dispatch
  alike; the dispatch spends its truck hours on waste the authored order
  puts first, the plan on ore. Week-level freedom of order and release is
  what the plan has and the dispatch does not.

Candidate next steps:
1. Finer periods near the start of the horizon (days early, weeks later):
   most decisions that matter now are early, and that is where release
   timing within a week bites.
2. Release within a period pro rata to when the ground is released, not
   all-or-nothing at its end.
3. Reclaim blends per period from the plan's own pile contents (needs a
   blend model; the fixed blend is the plan's known gap on grade worlds).

## Known gaps

- Parcel plans get no seed (the schedule's movements are by block). The
  seed mapping over periods now maps every day of a period, but block
  plans over periods still seed at about half the schedule's value with
  13–90 broken rows; not investigated further, as block plans over periods
  are too slow to use.

## Parcels split by destination rules and grade bands, 2026-10-20 (removed)

Question: should parcels be split further by what the destination rules
and grade targets distinguish?

- **Rules already define the split.** Every world has 3 routing classes
  (waste: dump only; Fe 35–50: any destination; Fe ≥ 50: crusher or
  stockpiles), and within a class every material has the same value at
  every destination. Splitting by rule or by value changes nothing.
- **Grade bands tried.** A class with more than one destination split at
  every grade boundary the targets (lower, target, upper) and rules draw.
  Centralia's ore class (Fe 50–67) splits at 58/60/62; the grades world's
  target band (Fe 28–35) lies below every class with a choice, so nothing
  splits there. Columns +30–35%; long world 7-day MIP 11 → 20 s.
- **The plan gains nothing.** Plan values are identical on five worlds and
  within 0.03% on the long world. Sending sub-58 ore to a stockpile to meet
  the crusher band costs about $100/t in crusher value against a $2–5/t
  grade penalty, so no plan uses the extra choice.
- **Improve, against LT5:** Centralia +1.396% (−0.11), 14 days +3.128%
  (−0.56), trucks 60% and grades unchanged, no blasts +1.450% (+0.02),
  long world +0.620% (+0.44: the 4-day plan was kept at 77 s and the windows
  built on it gained another 2.6M). The plans are worth the same, so these
  differences come from which of several equally valued plans the solver
  returns, not from grade information. Removed; code archived as the
  session file `parcels-grade-bands.rs`.

What it shows instead: equally valued long-term plans dispatch very
differently (the long world from −0.49% to +0.34% in one dispatch). Which
plan the solver returns among ties is a real lever; choosing one the
dispatch can follow (for example, nearest the incumbent's digging) is a
possible next experiment.

## Exact blending in the long-term plan, 2026-10-20 (removed)

Correction to the table above: over the whole horizon the plan does not fix
a reclaim's blend. Each reclaimed flow carries any metal up to its tonnes at
the grade's ceiling, so long as a pile gives up no more metal than it took
in: optimistic, since a plan can reclaim a pile's best first.

Three ways to make blends exact were tried, all against the same parcels
and periods (code archived in the session folder `exact-blend-experiment/`):

1. **Improve's hourly SCIP model on compressed time** (each period one
   interval, rates, fleet hours and crusher budgets summed). It carries the
   dispatch's exact bar semantics (one bar per segment, mandatory
   selection), which over a week are wrong: 14 days is infeasible with grade
   targets and worth 62.5M without (linear plan 74.6M); Centralia found no
   plan in 60 s.
2. **The long-term plan as a SCIP MINLP.** One bilinear row per reclaim
   flow, grade and period (every reclaim carries the blend of all its pile
   has that period: opening stock plus receipts); 4–87 rows. Small worlds
   solve (14 days 0.5 s, no blasts 0.6 s), but on the 52-day worlds SCIP's
   plans are poor (Centralia 70M, grades 48M) or absent (long world) in
   60 s, also when started from the linear plan: SCIP does not accept a
   start that meets rows only to the other solver's tolerance, and with the
   binaries fixed its continuous problem still takes ~60 s.
3. **Linear plans with each pile's blend held at the plan before's,**
   repeated six times, each plan valued exactly by SCIP with its digging
   fixed (instant), the best kept. Not converging (piles enter and leave
   use), but a search:

| World | Linear plan | Its exact value | Best exact (iterated) | Solve |
|---|---|---|---|---|
| Centralia, 7-day | 316.3M | 311.2M (−1.6%) | 313.9M | 11 s (×6) |
| 14 days, 7-day | 74.6M | 73.3M (−1.8%) | 74.3M | 0.2 s |
| Grades, 7-day | 251.2M | 250.0M | 250.1M | 11 s |
| No blasts, 4-day | 330.7M | 329.3M | 330.5M | 0.2 s |
| Trucks 60%, 7-day | 298.4M | 296.6M | 297.0M | 29 s |
| Long world, 7-day | 709.2M | 700.6M (−1.2%) | 704.9M | 60 s |

The linear plan overstates itself 0.3–1.8% through blending, and iterating
finds better exactly blended plans. Improve with them, against LT5:
Centralia +1.392% (−0.11), 14 days +3.673% (−0.01), trucks 60% +2.628%
(=), grades +1.190% (+0.03), no blasts +1.480% (+0.05), long world +0.171%
(−0.007; its plans now arrive at 71 s and are never kept). Neutral, at six
times the solve time: the dispatch's departures from the plan (blast waits,
within-period order) are larger than what blending changes. Removed.

## A full MINLP by horizon (Centralia, 30 min limit each)

Improve's hourly SCIP model, seeded with the dispatch, on Centralia cut to
its first hours:

| Horizon | Variables (binary) | Result | Memory |
|---|---|---|---|
| 1 h | 28k (155) | optimal in 0.4 s, equal to the dispatch | 0.65 GB |
| 12 h | 336k (1,948) | still at the root at 2,797 s, no better than the dispatch | 3.3 GB |
| 48 h | 1.34M (7,816) | time limit at the root, no better; bound meaningless | 10.6 GB |

About 0.27 GB and 28k variables per hour: the 1,256-hour horizon would need
about 35M variables and 340 GB. The first hours are also uninformative (the
dispatch is optimal there): its losses are weeks later.

## Re-forecasting from the actual state (offline, no time budget)

- **Weekly, always locked:** worse everywhere (Centralia +0.81%, long world
  −0.10%): a bad week cannot be undone.
- **Weekly, kept only when better:** long world +0.83% in 405 s (best seen
  then), elsewhere below Improve.
- **Day by day, both ways:** the whole horizon reforecast each day, a 5-day
  plan aiming for its loader and destination totals, the dispatch to both,
  the day locked. Converges where the dispatch follows the plans (Centralia
  +0.91% → +1.40%, grades +0.09% → +0.98%, no blasts +0.46% → +1.42%),
  swings where it cannot (trucks 60% +0.40%, long world −0.14%, 50 min).
  No blast moved in any locked day; the long world's swings were the last
  day's reclaim (±3M), trucks' were the dispatch falling behind the plan's
  pace so late targets met waste.

All removed; what they showed is below.

## Clearance and priority rules in the plans (kept)

- **Clearance rule** (whole-horizon plan, held timeline): a blast's ground is
  dug only if the ground above it is gone by the end of the period the
  timeline clears it in (one binary per blast). Pro rata within the period
  ruled out every blast whose clearance ground is itself released that
  period. Long world +0.18% → +0.51%, the rest unchanged (on the solver
  before the fix below). Interim: to be replaced by blast timing decided in
  the plan, with binary clearing and firing, authored blast order and
  machine hours.
- **Priority rule** (every plan but the bound): bars read apart, each in its
  own order, and a loader works a lower bar in a period only if every higher
  one ran out of released ground in it: unless every block ahead of a bar's
  first block unreleased at the period's start is finished by its end, the
  lower bars get nothing (one binary per bar, period and blast run). A block
  of a blast the plan makes late counts as unreleased. The first schedule
  breaks none of these rows. It replaces the joined queue in the window
  plans and the free bars of the long-term plan. Parcels are cut to stay
  contiguous in every bar listing them. Room at a destination blocking a bar
  is not modelled yet.

## A solver bug: analytic-centre fixing without HiGHS's KKT check

With the priority rule Centralia's long-term plan came out "optimal" at
225.2M; native HiGHS solves the same program to 304.39M. The presolved
models matched bit for bit once five 1e-16 coefficients (which HiGHS drops
on input) were removed; tracing every global bound change against HiGHS's
optimum found the root's analytic-centre fixing. IPX called a non-converged
centre optimal; HiGHS's `optimizeLp` then runs `callLpKktCheck`, finds 571
violations and reports it unknown, so its MIP fixes nothing. The port called
IPX directly and fixed 571 columns, cutting the optimum off. Fixed in
`crates/solver/src/ipx/highs.rs` (`ipxSolutionToHighsSolution`, then the KKT
check). Every dumped plan MIP now matches HiGHS in objective, nodes and LP
iterations.

The wrong fixing had made plans 3-4 times faster (Centralia's 7-day plan
7.3 s → 25 s), so Improve's results fell with the fix; the priority rule
then wins much of it back. With the long-term plan stopping at a 2% gap
(on Centralia the optimum was found at 12 s and proved at 20 s):

| World | Before the fix | Fixed, clearance only | Fixed, both rules, 2% gap |
|---|---|---|---|
| Centralia | +1.471% | +1.291% | +1.492% |
| 14 days | +3.683% | +3.160% | +3.033% |
| Trucks 60% | +2.628% | +1.984% | +2.616% |
| Grades | +1.190% | +0.759% | +0.941% |
| No blasts | +1.427% | +1.453% | +1.617% |
| Long world | +0.511% | +0.240% | +0.516% |

The plans' solve time is now the limit: trucks 60% gets no long-term plan
within 60 s and the long world's takes 28 s. Next: start the long-term plan
from the first schedule (no incumbent for its first 12 s on Centralia), then
blast timing decided in the plan.

## Plan speed after the fix

- **Starting the long-term plan from the first schedule** (mapped to
  parcels, its on-time flags read from its digging): no faster. Completed,
  it lies about 4% below the plan's bound, outside the 2% gap, so the search
  runs on to the same sub-MIP solution (Centralia's 4-day plan 24.7 s either
  way). From the LP relaxation's answer: no faster either. Only a start
  near the optimum helps (3.3 s). Not kept.
- **Trucks 60%:** the bound settles in 9 s, but good plans need a tree:
  native HiGHS on two cores reaches a 2% gap at 65 s (its first plan, at
  15 s, is worth less than the first schedule). No plan arrives within
  Improve's 60 s; its windows still reach +2.62%.
- **One 7-day plan instead of 7- then 4-day:** Centralia +1.49% → +1.51%,
  grades +0.94% → +0.97%, the rest identical. Kept.

## Blast timing decided in the plan, parcel targets and pace caps (removed)

- **Blasts decided by the long-term plan:** binary clear, step and fire
  flags per period; each machine's bars held to dig priority (a lower bar
  gets work only if every higher one ran out of ready work all period); a
  machine with work works at full capacity; a blast charged by a period's
  start fires in it; a minimum clear-to-fire lag from its machines' summed
  rates. The first schedule, with its blast history, breaks none of these
  rows on any blast world. But the plan fired most blasts later than the
  first schedule (its loaders' pace, not its blast rows), dispatches to it
  were worse (Centralia +1.18% → +1.10%, trucks 60% +0.66% → −2.60%) and
  identical with blasts held or live, and it solved up to five times slower
  (long world: no plan in 300 s).
- **Why:** with authored priority enforced, a loader digs only its
  highest-priority ready bar's current block, so the ground order is fixed
  by the bars; the dispatch chooses only pace, routing, stockpiles and work
  under blocked bars. Blast timing follows from pace.
- **Parcel targets** (per loader, parcel and day) in the dispatch: within
  ±0.01 points, worse on trucks 60%. **A weekly cap per loader at the plan's
  pace:** neutral where it does not bind, much worse where it does (trucks
  60% −3.3%, no blasts −1.5%, long world −2.5%): the plan's slower weeks are
  artefacts of pooling a week's trucks and crusher.

## Period length of the long-term plan (one dispatch to its targets)

| World | 7-day | 4-day | 2-day |
|---|---|---|---|
| Centralia | +1.18% (14 s) | +1.27% (28 s) | +1.22% (111 s) |
| 14 days | +2.45% | +1.67% | +1.70% (1-day +2.05%) |
| Trucks 60% | +0.66% (75 s) | −0.06% (72 s) | +0.63% (900 s limit) |
| Grades | −0.28% (9 s) | −0.07% (23 s) | +0.34% (351 s) |
| No blasts | +1.39% | +1.10% | +1.04% (1-day +1.25%) |
| Long world | +0.35% (26 s) | +0.15% (153 s) | +0.09% (363 s) |

Halving the period doubles to quadruples the solve on blast worlds; finer
plans promise less (Centralia +3.9% → +3.1%) but are not better guides.
Seven days stays.
