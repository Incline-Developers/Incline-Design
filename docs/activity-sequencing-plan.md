# Activity sequencing: implementation plan

Status: proposed, not started. Replaces "sequencing rules" as the next
roadmap item (the user, 2026-10-02: few schedulers use precedence rules unless
the schedule is optimised automatically; what they need is the work that has
to happen before a blast can be dug). Decisions D1–D7 in §6 need the user's
answer first. Read `AGENTS.md` first.

## 1. Goal

Ground is not available to a loader until it has been prepared, drilled,
charged and fired. Today every dig block is available from hour 0, so a
schedule can dig a bench that nobody has drilled. The goal is that each blast
goes through the real chain, and its dig blocks become available only once it
has fired:

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
  footprint overlaps this blast's by more than a sliver (D4).

### 3.2 The chain, simulated hour by hour

The hourly dispatch already steps through the horizon an hour at a time,
which is also how mining progress (and so "clear") becomes known. Each hour,
before allocating loaders:

1. A blast whose blocks above are all exhausted becomes **clear**.
2. Each fleet's capacity this hour (count × rate × its availability) goes to
   the clear blasts that still need that activity, in **need order** (§3.3).
   Prep takes m², drilling metres, charging holes. A blast moves to the next
   activity when the previous one is complete, and can start it in the same
   hour with what capacity is left.
3. A charged blast **fires** at the first moment inside a blast window: the
   window's start, or immediately if charging finishes inside one (D5).
4. Its blocks become available after the re-entry delay (default 0 h).

A block whose blast has not fired is not ready: the loader's dig bar does not
have work (authored order stays mandatory, so the loader waits rather than
skipping ahead), and the loader's idle hours say **waiting on blast**, naming
the blast on hover.

The simulation's result is a **release time per dig block**, plus the timeline
of each blast (clear, prep, drill, charge, fire) for display.

### 3.3 Need order

Fleets work blasts in the order loaders need them: by the authored priority
of the bar that first digs a block of the blast, then by that block's
position in the bar's sequence. Blasts no bar digs are worked last, in bench
order top down. (An explicit drill order is a later option, not v1.)

### 3.4 Solve and replay

- **Hourly dispatch** (the first schedule): simulates the chain as above.
- **Replay**: checks every dig movement against its block's release time, an
  issue if earlier.
- **Improve (SCIP)** and the day-by-day fallback: take the release times from
  the first schedule as fixed data (a block's movement columns before its
  release are not created). This is an approximation: SCIP cannot move a
  release earlier by mining the ground above sooner. Document it; it keeps
  the solver model linear in the new data.
- **Fingerprint**: the chain's settings, start statuses and patterns are part
  of the run's inputs, so changing them recalculates.

### 3.5 Starting state

Each blast has a status at the schedule start: Not started, Prepped, Drilled,
Charged or Fired, with "partly drilled" as a percentage later. Set from the
Solids Navigation tree (right-click a bench or blast → Status) like the
existing exclusions, with a bench setting all its blasts. See D6 for the
default.

### 3.6 Setup

Schedule → Setup gains a **Drill & Blast** step, laid out as cards like
Haulage → Setup:

- **Fleets**: dozers (count, m²/h), drill rigs (count, penetration m/h, and a
  per-hole move time in minutes), charge crews (count, rate per D2). Each with
  an availability % (v1: one figure; daily calendars like trucks later).
- **Pattern**: burden, spacing, square/staggered, subdrill. Per-blast
  overrides from the blast's properties in Planning → Blasts.
- **Blast windows**: a daily window (default 12:00–15:00) and the days it
  applies; re-entry delay.
- **On/off** (D6).

### 3.7 Showing it

- **Gantt**: one read-only row per fleet (Dozers, Drills, Charging), with a bar
  per blast per activity, and a Blasts row with a marker at each firing. Hover
  a bar: blast, activity, start and end, quantity (m², m, holes). Loaders'
  amber idle strip carries the new reason.
- **Inspector**: at the slider's hour, each fleet's current blast and
  progress, and the next blasts to fire.
- **Animate**: each blast's outline tinted by its state at the shown hour:
  waiting for clearance, prepped, drilling (collars appear in proportion to
  metres drilled), charged (collars change colour), fired (the muck tint until
  dug). Hover a blast for its status and times. Collars reuse the drill-hole
  collar look; the same depth-tested approach as the haul flows.
- **Calendar**: a Drill & Blast group with metres drilled, holes charged and
  blasts fired per day, and the Total column.

## 4. Steps

Each step ends working, validated in the app, and committed.

1. **Blast work in capture**: area, holes, depth, released and overlying
   blocks, need order; a focused test on a two-bench fixture. No behaviour
   change yet.
2. **Setup and status**: the Drill & Blast step, the start statuses and tree
   menu, persistence in the schedule plan, fingerprint. Off by default.
3. **Dispatch and replay**: the hour-by-hour chain, release times, the
   waiting-on-blast idle reason, replay check.
4. **Improve**: release times as fixed data in the formulation and windows.
5. **Gantt and Inspector**: fleet rows, firing markers, hovers.
6. **Animate**: blast tints and collars.
7. **Calendar and report**: daily D&B totals.
8. **Docs**: `docs/scheduling-drill-blast.md`, AGENTS.md table row.

## 5. Out of scope for v1

Explicit drill orders and drill bars authored by hand, machine calendars per
rig, partial blasts (digging part of a blast before the rest is drilled),
blast clearance zones that stop nearby loaders, explosive products and
powder factor, and drill-pattern design beyond burden, spacing and layout.

## 6. Decisions for the user

- **D1. Fleets as capacity or as machines?** Proposed: capacity per activity
  (count × rate), shared across blasts in need order, so two rigs on one
  pattern finish it twice as fast. The alternative, one machine per blast at
  a time with its own Gantt row, is more literal but slower to build and to
  read.
- **D2. Charging rate unit.** Proposed: holes per hour per crew. Alternatives:
  metres per hour, or kilograms per hour (which needs a powder factor).
- **D3. Pattern per blast.** Proposed: one default pattern in Setup, with
  per-blast overrides in Planning → Blasts. Alternative: per bench.
- **D4. What counts as "on top".** Proposed: any dig block in a higher bench
  whose footprint overlaps the blast's by more than 1 m², fully mined.
  Alternative: a percentage of the blast's area cleared.
- **D5. When a charged blast fires.** Proposed: at the first moment inside a
  window - the window's start, or straight away if charging finishes inside
  one - with every blast charged by then firing together, and a re-entry
  delay (default 0 h) before digging. Alternative: always at a fixed firing
  time.
- **D6. Default status and switching it on.** Proposed: off by default, so
  existing schedules do not change; when switched on, every blast starts Not
  started except those the planner marks (with a bench-wide action).
  Alternative: on for new projects, with the top bench Fired.
- **D7. Improve keeps the first schedule's release times.** Proposed as above
  (§3.4). Alternative: model the chain inside SCIP, which is far larger and
  slower.
