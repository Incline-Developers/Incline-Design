# Scheduling: drill and blast

A dig block can only be dug once its blast has been prepared, drilled,
charged and fired. With drill and blast switched on, the schedule works that
chain out hour by hour beside the loaders, and a loader whose next block has
not been blasted waits for it.

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
5. **Fire.** A charged blast fires in the first available blast window that ends
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
MPU row (or right-click the row → Add blast bar). In the interactive sequence
window, narrow the view with Solids Navigation and click blasts in the 3D
preview to add them to the order. Sequence Preview fires the blasts before
its position, taking them off the view to uncover the bench below, and a
click inserts at that position. Selected blasts are highlighted; the list on
the right allows reordering and removal. Apply saves the sequence, and Cancel
discards it.
Edit blasts… on the bar reopens the same window. A machine works the first
blast in its highest-priority open bar that still needs it, and waits there
if that blast is not ready. Machines with the same blast in their bars work
it together, their rates adding. A dig or reclaim bar cannot be put on a
drill and blast machine, nor a blast bar on a loader.

A **Follow** bar (the Follow chip, then choose the machine) has no blasts of
its own: while it is the machine's highest-priority open bar, the machine
works whatever its leader's own open blast bar has next, adding its rate
there. Two dozers of one rate then prep the leader's sequence in half the
time. Only machines of the same type can be followed; a leader standing for
a delay still leads. Change the leader from the bar's right-click menu.

## Blasting windows

The **Blasting** row on the Gantt shows windows and firing diamonds with each
blast's name. Double-click empty space, or right-click → Add blasting window,
to create a window. Windows can repeat daily or occur once. Daily window
hours are hours of the day; one-off hours are elapsed from Day 1 00:00
(for example, Day 3 12:00 is hour 60). Double-click a window to edit it;
its context menu also offers Delete. Editing or deleting a daily window
changes all its occurrences.

Existing projects retain their daily window until windows are edited on the
Gantt. Removing every window leaves charged blasts waiting indefinitely.
A blast already marked Fired at the start remains available immediately.

## Setup → Drill & Blast

- **Sequence drill & blast:** off by default, so a project's schedule is
  unchanged until it is switched on. When on, every blast starts Not started.
- The default pattern (burden, spacing, subdrill, staggered rows), hole
  diameter, stemming, product density and clearance buffer. The legacy daily
  window is editable here until windows are managed on the Gantt.
- The run's blasts by bench. Right-click a blast, or a whole bench's header,
  to set the stage it starts the schedule at (Not started, Prepped, Drilled,
  Charged, Fired). Select one to give it its own pattern. Ground is named by
  the app's one path, solid/bench RL/blast/flitch RL/dig block: blast 1 of
  Pit A's bench from 336 to 348 is "Pit A/336/1" everywhere it is shown, and a
  dig block in it "Pit A/336/1/344/3".

## What you see

- **Status line:** a run in which a dig bar needs a blast that no machine bar
  works - its next step (prep, drill or charge) is in no bar of that kind of
  machine, and it does not start Fired - still publishes, but the status adds
  "· N blasts have no machine to work them" in the warning colour. Its hover
  names each blast and the missing step; the loader waits on it all horizon.
- **Gantt:** each blast bar's band shows its machine's work, coloured prep,
  drill or charge; hovering says what and how far. The remainder of a machine
  bar greys out when all its blasts finish that machine's step. Firing diamonds
  and blast names appear on the separate Blasting row. A loader waiting on a blast has the amber idle strip, and its
  hover names the blast and what the blast itself is waiting for: ground above
  still being dug, ground above that no bar digs (it never clears), a step no
  machine bar works, a machine busy elsewhere, or a blast window.
- **Inspector:** at the slider's hour, what each dozer, drill and MPU is on and
  how far, and the blasts under way with when each can be dug. A loader
  waiting on a blast, and an idle dozer, drill or MPU, names the blast and what
  holds it (for example "Pit A/336/1 · never clears").
- **Calendar:** each drill and blast machine's availability, utilisation,
  rate and what it got done each day, in its own unit.
- **Report export:** machine quantities and working hours, grouped by day,
  week or the whole schedule, in a separate drill-and-blast table.
- **Animate:** a blast is drawn whole until it fires, cross-hatched with
  benching's crosses until prepped (prep clears the hatching from one end
  along its longer side). Laid over its top, its outline is in the colour of
  its stage and its holes appear as they are drilled and turn red as they are
  charged. Once it fires, its dig blocks replace it and its marks go.

## How it is solved

The hourly dispatch (`optimisation/blended/greedy.rs`) runs the chain
(`optimisation/blended/drill_blast.rs`) at the start of each hour on the
ground standing then, so clearance follows what the loaders have dug. Its
release times are then fixed for Improve and the day-by-day fallback: SCIP
gets no dig columns before a block's release. Improve can therefore not move
a release earlier by digging the ground above sooner. The
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

The subsequent Gantt window and sequence-editor changes passed temporary
behaviour tests for mixed daily/one-off windows, exact window-end boundaries,
no remaining windows, legacy persistence, machine-specific completion shading,
simultaneous firing label layout and stale preview clicks. Those tests were
removed under the repository policy. Native Clippy, the WebAssembly compile
check and the desktop build passed; desktop startup and GPU initialization
were smoke-checked. Full interaction with the new controls remains unverified
in desktop and browser GUIs; the earlier screenshots show the previous layout.
