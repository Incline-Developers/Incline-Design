# Haulage: implementation plan

Status: agreed roadmap item, not started. Roadmap order is inspection views
(done), **haulage**, sequencing rules, scenarios. This document is the brief
for whoever implements it. Read `AGENTS.md` first.

## 1. Goal

Replace the travel-only truck coefficient with a haul model a planner
recognises: trucks drive real roads, climb ramps slower than they run on the
flat, and spend time being loaded and dumping. Every movement candidate's
truck-hours per tonne should then come from its own route, so a deep bench
costs more trucks than a shallow one and the schedule shows where trucks,
not loaders, limit output.

The user's priority is UI and workflow: intuitive, simple, uncluttered.
Summary at a glance, detail on hover, no solver figures in always-visible
text. Validate by driving the app and taking screenshots.

## 2. What exists today

| Piece | Where | What it does |
| --- | --- | --- |
| Truck classes | `model/schedule/trucking.rs` (`TruckClass`, `TruckCalendar`) → Schedule Setup → Truck Classes (`ui/elements/schedule_trucking.rs`) | Payload, loaded and unloaded speed (km/h), units, availability and utilisation per day |
| Trucking rules | same file (`TruckingRule`, `allowed_classes`) | Which classes may serve which loader, source and destination; a union of matching rules |
| One-way distance | `model/schedule/destinations.rs` (`distance_km` on `StandaloneDestination` and `SolidDestination`, default `DEFAULT_DISTANCE_KM`), edited in Setup → Stockpiles/Dumps/Crushers (`ui/elements/schedule_destinations.rs`) | One figure per **destination**, used for every source and for reclaim from it |
| Coefficient | `trucking::coefficients(class, route, one_way_km)` | `travel_cycle_h = d/loaded + d/unloaded`; `truck_hours_per_tonne = cycle / payload`. Already takes a whole `RouteContext` so a haul model can replace the distance without changing callers |
| Capture | `app/commands/schedule_capture.rs` ≈ line 1439–1500 | One `MovementCandidate` per (block or pile, material, destination, truck class), each with its own `truck_hours_per_tonne` — so per-block coefficients need **no formulation change** |
| Solving | `optimisation/blended/greedy.rs` (≈ line 452), `rolling.rs`, SCIP | `Σ tonnes × truck_hours_per_tonne ≤ class truck-hours` per interval |
| Publishing | `app/schedule_publish.rs:219` | `truck_hours = tonnes × truck_hours_per_tonne` on each delivery |
| Explaining | `IdleReason::NoTrucks` (`model/schedule/result.rs`), Inspector Trucks section (`schedule_inspector.rs::trucks`), truck rows in the Calendar, Trucks table in the report (`schedule_report.rs`) | Trucks in use against the fleet |
| Run fingerprint | `app/schedule_run.rs` ≈ line 287–297, `app/schedule_pipeline.rs:515` | Distances are hashed so a change recalculates |
| Haulage tab | `PlanningPage::Haulage` with one subpage `PlanningSubpage::Layout` (`ui/state.rs` ≈ 5620–5645). Layout shows the 3D viewport (`is_planning_viewport`). Its step list is a scaffold (`planning_setup.rs::draw_steps`, the `PlanningPage::Haulage => {}` arm): "Configuration" and "Site data → Content" rows with nothing behind them. `app/commands/mod.rs:862` ignores Haulage subpage changes | Empty |

Useful geometry already available:

- **Polylines** are `Object::Polyline { layer, verts: Vec<PolyVertex>, .. }`
  with 3D `DVec3` positions and DXF bulges (`model/mod.rs` ≈ 826–880).
  Imported road strings from DXF arrive as these.
- **Dig blocks** carry a plan anchor point inside their footprint and flitch
  base/top RL (`DigBlockRef` in `model/schedule/sequence.rs` ≈ 302; capture's
  `GroundContext` has `solid`, `bench`, `flitch`). That gives a 3D point per
  block.
- **Solid destinations** (stockpile and dump solids) have geometry. Standalone
  destinations (typically crushers) have none.

## 3. Design

### 3.1 Road network: polylines on chosen layers

Planners already have road and ramp strings, usually from DXF. Don't invent a
road editor. The network is **every polyline on the layers the user ticks as
road layers**, drawn and edited with the existing CAD tools.

- Build an undirected graph from those polylines: nodes at vertices, edges
  along segments. Bulged segments are flattened (reuse the tessellation in
  `Object::string_geometry`).
- **Joining:** vertices from different polylines join when they are within a
  **join tolerance** in 3D (default 2 m). A vertex that lands on another
  polyline's segment, in plan and within tolerance in elevation, splits that
  segment and joins it (T-junctions are the common case). Plan crossings at
  different elevations do **not** join, so a ramp passing over a bench road
  stays separate.
- **Per-edge figures:** 3D length and grade (rise over plan run, signed by
  direction).
- **Diagnostics,** shown in the Layout panel and as markers in the view:
  - dead ends (degree-1 nodes);
  - near misses (ends within 3× tolerance of another road but not joined);
  - separate pieces (connected components beyond the largest);
  - edges steeper than a sanity limit (default 15 %).

Not in scope: one-way roads, per-road speed limits, intersections with
priority, traffic congestion. Leave room for per-road attributes later (key
attributes by `ObjectId`), but don't build them.

### 3.2 Where trucks join the network

- **Dig block → network:** from the block's anchor at the flitch mid-RL to the
  nearest network point, preferring points within the bench's elevation band.
  The connecting leg is counted as flat travel at its 3D length. If the
  nearest point is far away (default over 500 m) or outside the bench band,
  still use it, but raise a capture diagnostic naming the area and bench
  ("No road reaches this bench; using the nearest road 640 m away").
- **Destination → network:** each destination gets a **dump point**.
  - Solid destinations default to the network point nearest the solid's plan
    centroid at its top surface. The user can override it by picking in the
    view.
  - Standalone destinations need a picked point.
  - Store dump points on the destination settings in `destinations.rs`, next
    to `distance_km`, as `Option<[f64; 3]>`.
- **Reclaim** runs from the pile's dump point to the receiving destination's
  dump point.
- **Fallback, per destination:** a destination with no dump point, or no
  usable network, keeps today's fixed one-way distance. The UI shows which
  method each destination uses ("Roads · 3.4 km avg" or "Fixed 2.0 km"). A
  project with no road layers behaves exactly as today, apart from the cycle
  additions in 3.3.

### 3.3 Cycle time per class

Cycle = load + travel loaded + dump + travel empty.

- **Load time:** payload ÷ the loader's nominal rate (dig rate for dig work,
  reclaim rate for reclaim; the class or agent default, not per-day
  overrides). A loader's t/h rate already includes truck exchange, so no
  separate spot time is needed. A truck is busy while it is being loaded, so
  it belongs in truck-hours.
- **Dump time:** a new class field, minutes per load (default 1.0).
- **Travel:** a shortest-time path over the graph. Each edge's time is its
  length at the class's speed for that edge's grade band and load state. The
  loaded and empty legs are separate searches, because uphill loaded is not
  the same as downhill empty, and they may take different roads.
- **Speed table per class**, replacing the two speed fields in the UI:

  |  | Loaded km/h | Empty km/h |
  | --- | --- | --- |
  | Flat | (old loaded speed) | (old unloaded speed) |
  | Uphill | 11 | 25 |
  | Downhill | 20 | 30 |

  An edge is a ramp when its grade magnitude is over a **ramp threshold**
  (network setting, default 3 %). Keep it to three rows; a full rimpull curve
  is out of scope.
- **Queuing** is excluded and documented as excluded. The module docs must
  say so, as they say "travel only" today.
- `truck_hours_per_tonne = cycle_h / payload`.

**Semantic change (must be documented, not silent):** load and dump time now
count, so every route uses more truck-hours than before, even in projects
without roads. Old files deserialize with dump time 0 and the new speed rows
defaulted from the old flat speeds. Load time still applies. Record the change
in the new doc (section 6) and in the module docs of `trucking.rs`. See
decision D1.

### 3.4 Where the computation lives

- `model/haulage/` (new module; pure, no UI, wasm-safe):
  - `network.rs`: graph build, joining, diagnostics, a nearest-point spatial
    index (uniform grid over segment bounding boxes).
  - `routing.rs`: Dijkstra over edge time for a given class and load state.
    Run one search from each dump point per class per direction (reverse
    graph for the loaded leg into a destination), then look up every block's
    access node. That gives destinations × classes × 2 searches, not one per
    candidate.
  - `HaulageSettings` (persisted on the `Document` with the schedule plan):
    road `LayerId` set, join tolerance, ramp threshold.
  - The road polylines themselves stay ordinary objects.
- `trucking::coefficients` gains a variant that takes the route's computed
  breakdown (`CycleBreakdown { load_h, loaded_h, dump_h, empty_h,
  loaded_km, empty_km }`). The fixed-distance path builds the same breakdown,
  so there is one formula.
- Capture builds the network once per run from a snapshot of the road
  objects (an owned copy, because capture runs off the UI thread), computes
  breakdowns, and attaches them to `MovementCandidate`. Keep
  `truck_hours_per_tonne` as the field the solvers read; add the breakdown
  beside it for publishing. The solvers must not change.
- **Fingerprint:** hash road-layer object content (ids, vertices), the
  haulage settings, dump points, the new class fields and the loader rates
  used for load time into the run fingerprint (`schedule_run.rs`), so editing
  a road recalculates. Make sure unrelated object edits on other layers do
  **not** recalculate.
- **Caching:** the graph can be cached by that hash (job queue, `app/jobs.rs`,
  if it's slow on real data; measure first).

### 3.5 Publishing

Each delivery in the result gets the tonne-weighted cycle breakdown of the
candidate that produced it (or an index to it), so the views can show the
average cycle, haul distance and tonne-km per class, loader and day without
recomputing. Keep the result's size modest: store per candidate, not per
delivery, if deliveries can reference their candidate.

## 4. UI

Keep the Haulage tab about **roads**. Truck classes and trucking rules stay in
Schedule Setup, where the fleet calendars are (decision D2).

### 4.1 Haulage → Layout (the 3D viewport)

Replace the scaffold step list with a small side panel, in the style of the
Schedule Setup lists:

- **Road layers:** a checklist of drawing layers (use `checklist_popup` or an
  inline list). Ticking one immediately rebuilds the network overlay.
- **Network:** one summary line, for example "42 roads · 3 junction warnings",
  with the warnings listed below on hover or expansion. Clicking a warning
  frames the camera on it.
- **Dump points:** one row per destination, showing its method
  ("Roads"/"Fixed 2.0 km"). Its menu offers Pick in view, Use nearest road,
  and Use fixed distance.
  - Picking follows the existing select-then-act and snapping patterns; while
    picking, the viewport stops taking selection.
- **Settings** (folded): join tolerance, ramp threshold.

Viewport overlay while on this page:

- road edges tinted by grade: flat neutral, uphill warm, steep red;
- junction dots, plus warning markers for dead ends and near misses;
- dump points as labelled pins.

Hovering a road shows its length and grade.

**Route check:** pick a dig block (or a destination) and the overlay
highlights the loaded and empty paths to a destination you choose. A small
card shows the cycle breakdown: load, haul, dump, return, total minutes,
distance, and t/h per truck. This is how a planner trusts the model, so it's
worth getting right.

The overlay is a viewport overlay, not scene geometry. Never rebuild scene
caches per frame (`AGENTS.md` → Invalidation and caching). Drawing it with
egui's painter from projected points, the way existing screen-space markers
do, is likely simplest.

### 4.2 Schedule Setup → Truck Classes

- Replace the two speed fields with the 3×2 speed table.
- Add **Dump time (min)**.
- Under the table, show a greyed line with the class's example cycle on its
  busiest route once a schedule exists. Optional, but useful.

### 4.3 Inspection (reuse the inspection views)

- **Inspector → Trucks:** keep "X of Y in use". On hover, show the average
  cycle in minutes and the haul distance for the hour.
- **Gantt bar hover:** add the average cycle for the bar's deliveries, one
  line.
- **Calendar → truck rows:** add "Avg cycle (min)" and "Tonne-km" rows per
  class. They're aggregates like the existing rows and need a Total column
  entry (tonne-weighted cycle, summed tonne-km).
- **Report → Trucks table:** add avg cycle, loaded km, tonne-km.
- **Charts:** a "Trucks in use" chart per class against the fleet line,
  reusing the stockpile chart's capacity-line drawing. Cheap and clear.
- **Idle reason** `NoTrucks` text can stay; its hover can name the class and
  its average cycle that day.

## 5. Phases

Each phase is committed separately and checked with screenshots.

**Phase 1: road network and Layout view (no schedule effect).**
- `model/haulage/network.rs` with focused tests: join tolerance, T-junction
  split, overpass not joined, dead end and near-miss detection, grade sign.
- Haulage settings persisted, with undo through the normal edit path.
- Layout side panel: road layers, network summary and warnings, overlay.
- Wire `SetPlanningSubpage` for Haulage, which is ignored today.

**Phase 2: dump points and route check.**
- Dump points on destinations, with a picker and nearest-road defaults.
- `routing.rs` with tests: a straight flat road matches the analytic cycle;
  a ramp is slower uphill loaded; directionality; a disconnected network
  falls back.
- The Route check card and path highlight.

**Phase 3: cycle model into the schedule.**
- Class fields (speed table, dump time) with serde defaults and migration
  from the old speeds.
- Breakdown-based `coefficients`; capture builds the network and per-block
  breakdowns; diagnostics for far or unreachable access; fingerprint.
- Confirm a project with no road layers produces the same schedule as
  before, apart from the documented load and dump addition. Test with load
  and dump forced to zero.
- Publish breakdowns.
- Update `trucking.rs` module docs and remove the
  `#[allow(dead_code, reason = "...later stage")]` attributes that this
  consumes.

**Phase 4: inspection.**
- The Inspector, Gantt, Calendar, report and chart additions from 4.3.
- Docs: `docs/scheduling-haulage.md` (what the model counts and excludes,
  fallback rules, the semantic change); update the `AGENTS.md` table rows
  (new "Haul roads and cycle times" row; amend the truck row).

**Possible later items (ask before starting):**
- traffic colouring of roads at the slider time;
- per-road speed limits or one-way roads;
- queuing or match factor;
- trucks animated along routes in Animate.

## 6. Decisions for the user (recommended defaults in bold)

- **D1.** Load and dump time count for every project, including ones without
  roads: **yes, documented**. The alternative is keeping travel-only until
  roads are set up.
- **D2.** Truck classes and rules: **stay in Schedule Setup**, with Haulage
  holding roads and dump points. The alternative is moving all truck setup
  into the Haulage tab.
- **D3.** Speed model: **flat, uphill and downhill × loaded and empty**. The
  alternative is grade bands or rimpull curves.
- **D4.** The road network: **polylines on ticked layers**. The alternative
  is a dedicated road object type.

## 7. Working rules for the implementer

- **Read-only sample:** the DreamLand sample project used for manual testing
  is read-only. Never save it; close the app without saving. If it has no
  road strings, draw a test network in the running app and discard it.
  `examples/` holds another import-ready project.
- **GUI driving:**
  - Only send input when the focused window's title starts with
    "Incline Design".
  - Never use `pkill -f` with a pattern that matches your own shell command.
    Use `pkill -x incline-design`.
- **Solver settings:** keep `misc/allowweakdualreds=false` and MPEC off in
  seeded SCIP solves.
- **Tests:** add focused `#[test]`s while developing and delete them before
  committing (`AGENTS.md`).
- **Commits:**
  - Leave the untracked file "Stage 5C TODO" out of commits.
  - Never name any external scheduling product in the repo.
- **Checks:**
  - Run `cargo clippy`.
  - Run `trunk build` once at the end, because the model code must compile
    for wasm (the solver and capture paths are native-only; the network and
    Layout view are not).
  - Translate all new UI text into `i18n/en/incline_design.ftl`.
- **Known open issue, unrelated:** with a Reclaim bar on DreamLand,
  Improve's seed completion is infeasible. Don't chase it as part of this
  work.
