# Stockpile modes

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

## What the schedule does

- **Not building.** Routing passes the pile over as if it were full: material
  goes to the next destination its rules allow. If there is none, the
  machine stands.
- **Not reclaiming.** A Reclaim bar on the pile has no work that day, however
  much the pile holds, so its loader moves on to its next bar, exactly as it
  would from an empty pile. Bar priority is judged the same way.

Idle time a mode causes is explained on the Gantt's idle strip as
**stockpile mode**, naming the piles. A pile its mode keeps from building is
named before any destination that is merely full.

The hourly dispatch, the whole-horizon SCIP model, the day-by-day fallback
and the independent replay all apply the same rule. In the model, movement
columns into a pile on a day it is not building, and out of a pile on a day
it is not reclaiming, are never created, and a reclaim bar's readiness counts
only piles reclaiming that interval. The replay rejects receipts or reclaim
on a forbidden day.

## Storage

`SchedulePlan::stockpile_operations` holds one `StockpileOperation` per pile
that has a non-default calendar (`model/schedule/stockpile_operation.rs`).
Capture writes each used pile's daily modes onto `BlendPile::modes`, trimmed
of trailing Build & reclaim days; a day past the end builds and reclaims.
Deleting a standalone stockpile removes its calendar in the same undo step; a
solid-backed pile keeps it, like its other settings, so undoing the solid
change brings it back.
