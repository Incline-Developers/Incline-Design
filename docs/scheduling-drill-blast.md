# Scheduling: drill and blast

A dig block can only be dug once its blast has been prepared, drilled,
charged and fired. With drill and blast switched on, the schedule works that
chain out hour by hour beside the loaders, and a loader whose next block has
not been blasted waits for it. The plan and the decisions behind it are in
`docs/activity-sequencing-plan.md`.

## The chain

For each blast of the Solids run (Planning → Blasts):

1. **Clear.** Every unmined dig block in a higher bench whose footprint comes
   within the clearance buffer of the blast's footprint, in plan, has been
   dug. A buffer of 0 m means only ground over the blast itself; 100 m keeps
   a 100 m stand-off. Ground above that no dig bar digs never goes, so such a
   blast never clears, and the run's notes name it. Excluding standing ground
   from mining does not count as removing it.
2. **Prep.** A dozer works the blast's top area, in m².
3. **Drill.** Drills drill its holes: the pattern (burden × spacing, square or
   staggered) laid over the blast shape gives the hole count, and each hole is
   the bench height plus subdrill deep, in metres drilled.
4. **Charge.** An MPU loads the product: per hole, the hole's cross-section
   × (depth − stemming) × product density, in tonnes.
5. **Fire.** A charged blast fires in the first daily blast window that ends
   after charging finished, and its ground can be dug from that window's end.
   With a 12:00-15:00 window, a blast charged by 14:59 is dug from 15:00 that
   day; one charged later waits for the next day's.

Clearance is checked at each interval opening, on a grid with at most
one-hour steps and exact machine-window, calendar and delay boundaries.
After clearance, machine steps can follow one another within an interval.
Machines on the same step share a clock and add their rates.

## Machines and bars

Dozers, drills and MPUs are machines like loaders. A machine class has a
**machine type** (Setup → Machine Classes): Loader, Dozer, Drill or MPU, and
its rate is read in that type's unit - m²/h, m/h or t/day. Availability,
utilisation, rate overrides, delay lists, rosters and delay bars apply to
them exactly as to loaders.

Each is a row on the Gantt. Drag the **Blasts** chip onto a dozer, drill or
MPU row (or right-click the row → Add blast bar) and pick the blasts it works,
in order; Edit blasts… on the bar changes them. A machine works the first
blast in its highest-priority open bar that still needs it, and waits there
if that blast is not ready. Machines with the same blast in their bars work
it together, their rates adding. A dig or reclaim bar cannot be put on a
drill and blast machine, nor a blast bar on a loader.

## Setup → Drill & Blast

- **Sequence drill & blast:** off by default, so a project's schedule is
  unchanged until it is switched on. When on, every blast starts Not started.
- The default pattern (burden, spacing, subdrill, staggered rows), hole
  diameter, stemming, product density, clearance buffer and blast window.
- The run's blasts by bench. Right-click a blast, or a whole bench, to set the
  stage it starts the schedule at (Not started, Prepped, Drilled, Charged,
  Fired). Select one to give it its own pattern.

## What you see

- **Gantt:** each blast bar's band shows its machine's work, coloured prep,
  drill or charge, with a diamond where each blast fired; hovering says what
  and how far. A loader waiting on a blast has the amber idle strip, and its
  hover names the blast.
- **Inspector:** at the slider's hour, what each dozer, drill and MPU is on and
  how far, and the blasts under way with when each can be dug.
- **Calendar:** each drill and blast machine's availability, utilisation,
  rate and what it got done each day, in its own unit.
- **Report export:** machine quantities and working hours, grouped by day,
  week or the whole schedule, in a separate drill-and-blast table.
- **Animate:** each blast under way on its bench top: its outline in the
  colour of its stage, its holes appearing as they are drilled and turning red
  as they are charged.

## How it is solved

The hourly dispatch (`optimisation/blended/greedy.rs`) runs the chain
(`optimisation/blended/drill_blast.rs`) at the start of each hour on the
ground standing then, so clearance follows what the loaders have dug. Its
release times are then fixed for Improve and the day-by-day fallback: SCIP
gets no dig columns before a block's release (D7 in the plan). Improve can
therefore not move a release earlier by digging the ground above sooner. The
independent replay rejects any dig before release, checks the machine work
and firing window, and confirms that overlying ground was exhausted before
clearance. Improve also holds those clearance deadlines, so it cannot move
the mining above a blast later than its preparation.

## Validation

Driven on the unsaved DreamLand sample: machine classes of each type, four
machines, blast bars on a dozer, a drill and an MPU over the top two benches,
work windows set by typing, the Inspector and Animate at several hours,
setting a lower blast Fired (its loader then dug all week), and Improve with
the release times fixed (the seed completed and the result passed the
replay). The screenshots below are from that Linux session, before the final
concurrent-machine timing corrections.

Seven temporary focused tests passed during completion: concurrent unequal
machine rates, clearance and priority delays, changing rates and fixed
release times, persistence and incompatible class edits, dispatch through
Improve and independent replay, exact capture boundaries and invalid pattern
refusal (including preparation-only schedules), and footprint distance with
holes. They were removed under the repository's test policy.

Native `cargo clippy -- -D warnings` and a wasm `cargo check` passed (the
wasm target reports unused solver-side code warnings). Not exercised interactively:
the final timing corrections in the Linux UI, the browser, macOS and Windows,
and projects with many blasts (Animate redraws the blasts under way every
frame).

Screenshots: [Setup](drill-blast/setup.png), [Gantt](drill-blast/gantt.png),
[Animate](drill-blast/animate.png).
