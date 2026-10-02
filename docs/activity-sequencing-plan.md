# Activity sequencing: implementation plan

Status: implemented 2026-10-02. Replaces "sequencing rules" as the
next roadmap item (the user: few schedulers use precedence rules unless the
schedule is optimised automatically; what they need is the work that has to
happen before a blast can be dug). The decisions are settled in §6 and
are reflected in the design below. Read `AGENTS.md` first.

## 1. Goal

With drill and blast enabled, ground becomes available to a loader only
after preparation, drilling, charging and firing. Each blast follows this
chain:

1. **Clear.** Nothing is still standing on top of the blast: every dig block
   above its footprint has been mined.
2. **Pattern prep.** A dozer clears and levels the blast's top surface, at a
   rate in m²/h.
3. **Drill.** Rigs drill the pattern: hole count from burden × spacing over the
   blast shape, hole depth from the bench height plus subdrill, time from a
   penetration rate in m/h.
4. **Charge.** The holes are loaded at a charging rate.
5. **Fire.** The blast fires in the next blast window (for example 12:00–15:00
   daily).
6. **Dig.** The blast's dig blocks become available to loaders.

The user's priority is UI and workflow: summary at a glance, detail on hover,
no solver figures in always-visible text. The planner should be able to see,
on the Gantt and in Animate, why a loader is waiting and which blast it is
waiting for. Validate by driving the app and taking screenshots.

## 2. What exists today

| Piece | Where | Use here |
| --- | --- | --- |
| Blast shapes | `BlastingPlan` / `BenchBlasts` / `BlastShape` in `model/mod.rs`; faces derived from cuts in `app/commands/blasting.rs` | The unit of activity: one chain per blast. A bench with no cuts is one blast |
| Dig blocks → blast | `DigBlockRecord::blast: Option<BlastShapeRef>` (`app/commands/solids_view.rs`), `GroundContext` in `app/commands/schedule_capture.rs` | Which ground each blast releases |
| Bench and flitch RLs | `BenchSelection` on each block | Bench height for hole depth; what is "above" |
| Drill patterns | `generate_pattern_collars(boundary, burden, spacing, rotation, offset, layout)` in `model/drill_hole.rs` (Square/Staggered) | Hole count, and collar positions for Animate |
| Collar drawing | `rendering/scene/drill_hole_cache.rs` (collar discs), `drill_collar.wgsl` | How holes look in Animate |
| Exclusions and as-mined | `MiningExclusions` on `Solid`; shared topography updates | Where a blast's start status would sit, and the same tree context menus |
| Hourly dispatch | `optimisation/blended/greedy.rs` (`State::work` per interval) | Where the chain is simulated hour by hour |
| Replay | `optimisation/blended/replay.rs` | Must reject digging a block before its blast fired |
| Idle reasons | `IdleReason` in `model/schedule/result.rs`, amber strip on the Gantt | A new "waiting on blast" reason |
| Delays and rosters | `model/schedule/delays.rs` | The pattern for recurring windows (blast windows) |
| Gantt, Inspector, Animate | `ui/elements/schedule_gantt.rs`, `schedule_inspector.rs`, `app/schedule_animation.rs` | Where the chain is shown |

## 3. Design

### 3.1 Blast work

For each blast, capture computes once:

- **Top area** A: the blast face's plan area (faces are already derived for the
  Blasts step).
- **Holes** N: the collar count from `generate_pattern_collars` over the face,
  with the blast's pattern (D3). Keep the collars: Animate draws them.
- **Hole depth** d: bench height (top RL minus base RL) plus subdrill.
- **Drill metres** N × d.
- **Blocks released**: the dig blocks whose `blast` is this one.
- **Blocks above**: dig blocks in any higher bench, of any solid, whose plan
  footprint comes within the configured clearance buffer (D4).

### 3.2 The chain, simulated hour by hour

The hourly dispatch already steps through the horizon an hour at a time,
which is also how mining progress (and so "clear") becomes known. Each hour,
before allocating loaders:

1. A blast whose blocks above are all exhausted becomes **clear**.
2. Each named machine works its highest-priority open bar's next unfinished
   blast. It waits when that blast is not ready. Prep takes m², drilling
   metres, charging tonnes of product. Machines on the same blast and step
   contribute their rates together on a shared clock. Completion and bar
   boundaries are events within an interval, so the next step can start as
   soon as its predecessor finishes.
3. Charging completion determines the next daily window ending strictly
   after it. The blast's ground becomes available at that window's end.

Clearance is checked at interval openings. The grid includes each machine's
bar, calendar and delay boundaries, blast windows and a regular step of at
most one hour.

A block whose blast has not fired is not ready: the loader's dig bar does not
have work (authored order stays mandatory, so the loader waits rather than
skipping ahead), and the loader's idle hours say **waiting on blast**, naming
the blast on hover.

The simulation's result is a **release time per dig block**, plus the timeline
of each blast (clear, prep, drill, charge, fire) for display.

### 3.3 Authored order

The planner lists blasts in each machine's bars. There is no automatic
assignment. Several machines may work the same blast at once. Delay bars
follow the same priority rules as other machine bars; delay lists and rosters
make the machine unavailable.

### 3.4 Solve and replay

- **Hourly dispatch** (the first schedule): simulates the chain as above.
- **Replay**: checks every dig movement against its block's release time, an
  issue if earlier.
- **Improve (SCIP)** and the day-by-day fallback: take the release times from
  the first schedule as fixed data (a block's movement columns before its
  release are not created). Overlying ground must also be mined before the
  fixed clearance time. This is an approximation: SCIP cannot move a
  release earlier by mining the ground above sooner. Document it; it keeps
  the solver model linear in the new data.
- **Fingerprint**: the chain's settings, start statuses and patterns are part
  of the run's inputs, so changing them recalculates.

### 3.5 Starting state

Each blast has a status at the schedule start: Not started, Prepped, Drilled,
Charged or Fired, with "partly drilled" as a percentage later. Set from the
blast list in Schedule → Setup → Drill & Blast, with a bench setting all its
blasts. See D6 for the
default.

### 3.6 Setup

Schedule → Setup has a **Drill & Blast** step with:

- The default burden, spacing, square/staggered layout and subdrill, plus
  overrides for the selected blast.
- Hole diameter, stemming and product density, for charge tonnes.
- Clearance buffer and daily blast window (default 12:00–15:00).
- Starting stages for individual blasts or whole benches, and on/off (D6).

Dozer, drill and MPU classes live in Machine Classes, with rates in m²/h,
m/h and t/day. Named machines live in Machines and use availability,
utilisation, daily rate overrides, rosters and delays like loaders.

### 3.7 Showing it

- **Gantt**: authored blast bars on individual machine rows, work bands and
  firing markers. Hover shows the blast, activity, timing and progress.
  Loaders' amber idle strips name the blast they wait on.
- **Inspector**: each machine's current blast and progress at the slider's
  hour, plus blast stages and release times.
- **Animate**: depth-tested blast outlines coloured by stage, holes appearing
  as drilling progresses and turning red as charging progresses.
- **Calendar**: each machine's daily work and Total in its activity's unit.
- **Report**: daily, weekly or whole-schedule machine work and working hours,
  with quantities and units in a separate drill-and-blast table.

## 4. Steps

Implemented: capture, persisted settings and machine types, authored bars,
dispatch and replay, fixed release and clearance times for Improve, Gantt,
Inspector, Animate, Calendar, report export and documentation. Validation is
recorded in [scheduling-drill-blast.md](scheduling-drill-blast.md).

## 5. Out of scope for v1

Partial starting progress, partial blast release, exclusion zones stopping
nearby loaders during firing, product catalogues and powder factor, rig move
time, additional drill-pattern controls, multiple windows per day and an
optimiser choosing the drill-and-blast programme.

## 6. Decisions (settled 2026-10-02)

- **D1. Named machines with authored bars.** Dozers, drills and MPUs are
  machines like loaders (for example DZ01, DZ02, DR3001-DR3003, MPU01), each
  a Gantt row. The planner drops bars on them listing blasts in order; a
  machine works its bar's blasts in that order and waits if the next is not
  ready. Machines with the same blast in their bars work it together, their
  rates adding. This replaces the shared-capacity fleets and the need order
  originally proposed: there is no automatic assignment.
- **D2. MPU rate in tonnes per day.** A blast's charge is tonnes of product:
  holes × charged length × hole cross-section × product density, where the
  charged length is the hole depth less stemming. Setup carries the hole
  diameter, stemming and product density.
- **D3. Pattern** as proposed: one default in Setup, per-blast overrides.
- **D4. Clearance by distance.** A blast is clear when every unmined dig
  block in a higher bench whose closest point lies within a buffer distance
  of the blast's footprint (in plan) is mined. A buffer of 0 m means "on
  top"; 100 m keeps a 100 m stand-off.
- **D5. Available at the end of the window.** A charged blast fires in the
  first blast window that ends after charging finishes, and its ground is
  available from that window's end (12:00-15:00 window: dig from 15:00).
- **D6.** Off by default; when on, blasts start Not started unless marked.
- **D7.** Improve keeps the first schedule's release times. An optimiser that
  proposes where to drill and blast is a possible later direction; for now
  the schedule is the planner's.
