# Stockpile modes and operating settings

Each stockpile has a **Mode** row in the Calendar, at the top of its group,
saying what the pile may do each day:

| Mode | Deliveries | Reclaim |
| --- | --- | --- |
| Build & reclaim | yes | yes |
| Build only | yes | no |
| Reclaim only | no | yes |
| Off | no | no |

The Default column applies to every day; a day cell overrides it, and a blank
day cell inherits it. Double-click a cell (or press Enter) to choose from a
list; a day's list also offers its Default back. Typing `both`, `build`,
`reclaim` or `off`, rectangular paste and Delete use the normal Calendar
controls. Delete on a day returns it to the Default; on the Default it
restores Build & reclaim. A pasted rectangle is one undo step.

Days are aligned to project hour zero, like crusher limits and grade targets.
Old projects open with every pile on Build & reclaim, which is how every pile
behaved before modes.

## The Stockpiles page

Setup's Stockpiles step lists each pile with its maximum tonnes, and shows
the selected one as one table in sections: **General** (name and maximum
tonnes, blank for unlimited), **Operation** (build and reclaim at once, rest
before reclaim, and the Default mode with how many days differ, with **Open
in Calendar** to edit them), **Optimiser** (blended or ordered chunks; a
chunked pile adds its reclaim order and chunk size), **Opening stock** and
**Haulage** (whether its dump and reclaim points reach the roads, in the
Haulage step's words).

Opening stock is entered as one tonnage and one set of values. A blended pile
holds it as one blend. A chunked pile splits it into opening chunks of its
chunk size, the last smaller, and shows them beside the table, where one
chunk can be given values of its own. Once the chunks differ, Opening stock
shows their combined values (tonnes-weighted, or summed for a field that
adds up) and offers to split them again; a blended pile holding several
chunks offers to combine them. Changing the representation or the chunk size
re-splits stock that is still one blend, and leaves chunks edited one by one
as they are.

Deliveries fill chunks of the chunk size in order, each closing when full to
be reclaimed. A chunk is filled once: material delivered after a reclaim
frees room goes into the next chunk, so a chunked pile refills all run, its
maximum tonnes limiting what it holds at once, and FIFO and LIFO keep
following age. A chunk size larger than the pile's maximum tonnes is an
error, since no chunk could fill.

What would stop a calculation is reported on the step, as an error when the
pile is used and a warning otherwise: a chunked pile without a chunk size,
and opening stock without a value for a grade the schedule tracks. A missing grade is also marked where it is typed and on its chunk.

## What the schedule does

- **Not building.** Routing passes the pile over as if it were full: material
  goes to the next destination its rules allow. If there is none, the
  machine stands.
- **Not reclaiming.** A Reclaim bar on the pile has no work that day, however
  much the pile holds, so its loader moves on to its next bar, exactly as it
  would from an empty pile. Bar priority is judged the same way.

Idle time a mode causes is explained on the Gantt's idle strip as
**stockpile settings**, naming the piles. A pile its mode keeps from building is
named before any destination that is merely full.

The hourly dispatch, the whole-horizon SCIP model, the day-by-day fallback
and the independent replay all apply the same rule. In the model, movement
columns into a pile on a day it is not building, and out of a pile on a day
it is not reclaiming, are never created, and a reclaim bar's readiness counts
only piles reclaiming that interval. The replay rejects receipts or reclaim
on a forbidden day.

## Closing chunks

A chunk of an ordered chunked pile closes - stops receiving and becomes
reclaimable - in two cases only, the same in the hourly dispatch, the model
and the replay:

- when it is **full** (within 1e-3 t of its capacity); it closes at the
  start of the next hour and the next chunk starts receiving;
- at the start of an hour its pile's **Mode** keeps from building (Reclaim
  only or Off); the chunk receiving closes if it holds anything. This is how
  a planner closes a chunk early on purpose: set the pile to Reclaim only for
  the day it should start being reclaimed.

Before this the model could close a partly filled chunk at any hour while the
dispatch never did, so Improve could release material the first schedule
could not. Now neither does: a chunk left partly filled when deliveries stop
stays unreclaimable until its pile's Mode stops building. A Reclaim bar
standing beside such a pile is explained as **stockpile settings**.

A Reclaim bar on a chunked pile has work only while a chunk is released -
closed, rested and holding material - not merely while the pile holds
something. Material in the chunk still filling cannot be drawn, so it does
not hold the loader: a Reclaim bar ranked above a Dig bar on the same
machine lets the digging fill the next chunk, draws it once it closes, and
the two alternate. Before, the Reclaim bar held the machine as soon as the
filling chunk held anything, and neither bar could work again.

A Dig bar has work only while its current block - the first of its
sequence with ground left - can go somewhere: each material in it has a
destination with room as the hour starts (a stockpile building that day and
more than 1 t below its maximum tonnes, a dump more than 1 t below its
capacity, a crusher more than 1 t below that day's limit; unlimited ones
always have room). Otherwise the machine works its next bar with work, and
returns to the Dig bar the first hour there is room, digging as fast as the
room allows - with a Reclaim bar below it on the same machine, as fast as the
pile is reclaimed. A bar still never starts before its place on the Gantt.
The hourly dispatch, the whole-horizon model and the replay judge it the
same way.

An idle hour is explained by the bar holding the machine - the highest one
with work - rather than by any bar open at the time: a Dig bar whose
material has nowhere to go reads as its destinations being full, even with
a Reclaim bar below it that could have worked.

## Per-pile settings

Two settings sit with the pile in **Setup → Stockpiles** and hold every day:

- **Build and reclaim at once** (on by default, which is how every pile
  behaved before). Off, the pile takes no deliveries in an hour it is
  reclaimed in. The hourly dispatch gives the hour to the Reclaim bar: while
  the bar draws the pile, deliveries go to the next destination their rules
  allow; if the bar then draws nothing, the hour is solved again with the
  pile open to deliveries. SCIP may choose either way each hour.
- **Rest before reclaim (h)** (0 by default). New material rests this long
  before it can be reclaimed. For a blended pile the clock restarts with
  every hour it receives anything, and the pile cannot be reclaimed until
  it has received nothing for the whole rest; while it rests a Reclaim bar on
  it has no work, so its loader moves to its next bar. For a chunked pile
  each chunk rests from the hour it closed, and a resting chunk counts as not
  yet released for FIFO and LIFO; a Reclaim bar on a chunked pile still
  counts as having work while the pile holds anything, as it does while
  chunks are filling. Opening stock is already rested.

An hour's deliveries restart a rest only above 1e-4 t, in the dispatch, the
model and the replay alike. Day-by-day windows carry when each pile last
received and when each chunk closed, so a rest runs across window
boundaries.

In the model, one `build` binary per pile and interval links the hour's
deliveries to the pile. Not building and reclaiming at once bounds reclaim by
its complement; a rest bounds reclaim by the complement of every `build`
still inside the rest, and, because rest decides whether a Reclaim bar has
work, the binary is held truthful in both directions (it is 1 only for a
real delivery). A chunk's release under a rest reads its `closed` column at
the last interval starting at least the rest earlier.

Idle time either setting causes is explained with the Mode's as
**stockpile settings**.

## Storage

`SchedulePlan::stockpile_operations` holds one `StockpileOperation` per pile
whose calendar or settings differ from the defaults
(`model/schedule/stockpile_operation.rs`). Capture copies the settings onto
`BlendPile::exclusive` and `BlendPile::rest_h`.
Capture writes each used pile's daily modes onto `BlendPile::modes`, trimmed
of trailing Build & reclaim days; a day past the end builds and reclaims.
Deleting a standalone stockpile removes its calendar in the same undo step; a
solid-backed pile keeps it, like its other settings, so undoing the solid
change brings it back.
