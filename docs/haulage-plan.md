# Haulage: implementation plan

Status: agreed roadmap item, decisions settled (section 6), not started. Roadmap order is inspection views
(done), **haulage**, sequencing rules, scenarios. This document is the brief
for whoever implements it. Read `AGENTS.md` first.

Revised after reviewing industry haulage documentation and five research
papers (see section 8). Don't name any commercial product in the repo.

## 1. Goal

Replace the travel-only truck coefficient with a haul model a planner
recognises:

- Trucks drive real roads.
- They climb ramps slower than they run on the flat, and obey road speed
  limits.
- They lose time pulling away and stopping.
- They spend time spotting, being loaded and dumping.

Every movement candidate's truck-hours per tonne should then come from its
own route. A deep bench then costs more trucks than a shallow one, and the
schedule shows where trucks, not loaders, limit output.

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
| Capture | `app/commands/schedule_capture.rs` ≈ line 1439–1500 | One `MovementCandidate` per (block or pile, material, destination, truck class), each with its own `truck_hours_per_tonne`, so per-block coefficients need **no formulation change** |
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
- **Solid destinations** (stockpile and dump solids) have geometry.
  **Standalone destinations** (typically crushers) have none.

## 3. Design

### 3.1 Haul roads are their own objects

Roads are a dedicated **haul network**, not plain polylines (decision D4):
they need actions of their own, such as promoting a road node to a stockpile,
dump or crusher, setting a speed limit on a road, and later opening and
closing roads over time. DXF interoperability comes from converting in both
directions.

- **Model:** a `HaulNetwork` on the `Document`, beside `solids` and
  `schedule`, with its own id counters, allocated and protected like the
  others: never reused, never rewound by an undo.
  - `HaulNode { id, pos: DVec3, role: Option<NodeRole> }`: an explicit
    junction or end. Roads meet only where they share a node, so topology is
    authored, not guessed every run.
  - `HaulRoad { id, name, from: NodeId, to: NodeId, verts: Vec<DVec3>,
    speed_limit_kph: Option<f64> }`: the shape between two nodes.
    Intermediate vertices are shape points, not junctions.
  - `NodeRole`: dump point or reclaim point of a destination
    (`DestinationId`). Later: bench exit, waypoint.
  - Undo goes through the normal edit path. Persisted in OMF with
    `#[serde(default)]`, so older projects open with an empty network.
- **Scene:**
  - Add `SceneEntityId::HaulRoad(RoadId)` and `SceneEntityId::HaulNode(NodeId)`
    so roads and nodes pick, select, hover and highlight like everything else
    (`rendering/query.rs`, `rendering/pick.rs`).
  - Draw them from the scene build (`rendering/scene/`) keyed by the
    network's revision, like design strings. Never rebuild per frame.
  - Roads are drawn thicker than design strings, with nodes as dots and
    role nodes as labelled pins.
  - They're visible in every Planning page and hidden in Design unless
    toggled.
- **Tools** (Haulage → Layout; also in the canvas right-click menu and
  `mac.rs`, per `AGENTS.md` → Menus):
  - **Draw road:** click points; snapping to existing nodes joins them,
    snapping to the middle of a road splits it with a new node, and the
    existing snap index covers surfaces and design objects. Esc or
    double-click ends it.
  - **Move node, move shape point, delete road or node** (a node is
    deleted with its roads, after a confirmation when it has a role),
    **split road here**, **join two nodes**.
  - **Node right-click menu:**
    - *Make dump point for ▸* an existing destination;
    - *Make reclaim point for ▸* an existing stockpile;
    - *New stockpile here*, *New dump here*, *New crusher here*: these
      create a standalone destination with this node as its dump point,
      then select it in Schedule Setup for naming;
    - *Clear role*.
  - **Road right-click menu:** *Set speed limit…* (applies to the
    selection), *Rename*.
- **DXF and polylines:**
  - **Convert to roads:** select polylines, run the command. Their vertices
    become shape points. Ends, and any vertex within the **join tolerance**
    (default 2 m, 3D) of another road, become shared nodes. A vertex that
    lands on another road's segment, in plan and within tolerance in
    elevation, splits it. Plan crossings at different elevations don't join,
    so an overpass stays separate.
  - The original polylines are kept, and the user can hide or delete them.
  - **Import DXF** gets an option "As haul roads", which imports then
    converts.
  - **Export roads** writes them as polylines to DXF, one layer for the
    network.
- **Per-edge figures** for routing: 3D length, signed grade, and speed
  limit.
  - Bulged input is flattened on conversion, so roads are straight
    segments.
  - Chains of shape points are already single roads, so the path graph is
    exactly nodes and roads.
- **Issues** (a list in the Layout panel and markers in the view; clicking
  one frames the camera on it):
  - dead ends that have no role;
  - near misses (an end within 3× the join tolerance of another road, not
    joined);
  - separate pieces (components beyond the largest);
  - roads steeper than a truck class's maximum grade (see 3.3);
  - roles whose destination no longer exists.

### 3.2 Where trucks join the network

**Dig blocks.**
- A truck runs on the **bench floor**, so the block's travel point is its
  anchor at the flitch's bench-floor RL.
- **Access leg** from a block to a candidate road point (a node, or the
  closest point on a road):
  - plan distance `d`, height difference `Δz`;
  - class maximum grade `g` (see 3.3);
  - **leg length = max(√(d² + Δz²), |Δz| / g)**.

  The leg can't be steeper than the class can climb or descend. A road 12 m
  above a block at 10 % costs at least 120 m of travel, even if it is
  directly overhead (the user's rule). The leg's effective grade is then
  `Δz / leg length`, so it is travelled at the class's grade speed for that
  grade (3.3), capped at the **bench speed** (setting, default 15 km/h).
- **Choosing the join:**
  - Don't just take the nearest point. Consider the road points within the
    **auto-join distance** (setting, default 300 m; widen to the nearest
    road if there are none) and take the one with the least total time,
    access leg plus network path.
  - This falls out of the routing for free: per block, the minimum over
    candidate access points of (leg time + the destination search's time at
    that point). Use a handful of candidates (the nearest node, and the
    closest point on each nearby road, up to about 8).
- **Connected and unconnected:**
  - A block is *connected* when its best access leg is within the auto-join
    distance and needed no added length for grade, meaning there's a road on
    or near its bench.
  - In the Layout view, connected blocks are tinted green and unconnected
    ones red. This is the quickest way to see where a ramp or bench road is
    missing.
  - An unconnected block still schedules on its grade-limited straight leg.
    Capture warns naming the area and bench ("No road on this bench; joined a
    road 12 m above, assuming 120 m at 10 %"). Readiness lists unconnected
    blocks as warnings, not errors (D5).

**Destinations.**
- **Dump point:** each destination has one dump point, a node with the
  dump-point role.
  - Solid destinations with no node assigned default to the road point
    nearest the solid's plan centroid at its top surface. They're shown in
    the UI as "Nearest road", with a one-click "Make it a node" to pin it.
  - Standalone destinations need a node.
- **Reclaim point:** stockpiles may have a separate one, defaulting to the
  dump point. Real piles are often built from one side and reclaimed from the
  other.
- **Reclaim** runs from the pile's reclaim point to the receiving
  destination's dump point.
- **Fallback:**
  - A destination with no dump point and no usable network keeps today's
    fixed one-way distance.
  - The UI shows the method ("Roads" or "Fixed 2.0 km").
  - A project with no roads behaves as today, apart from the cycle additions
    in 3.3.

### 3.3 Cycle time per class

This follows the standard truck cycle used by industry haul tools:

**Cycle = spot at loader + load + loaded travel + spot at dump + dump +
empty travel.**

- **Spot at loader:** a new **loader class** field, in seconds (default 45).
  Industry tools set spot time per loader type, because it depends on the
  machine and how trucks back in.
- **Load time:** payload ÷ the loader's nominal rate.
  - Use the dig rate for dig work and the reclaim rate for reclaim, taken
    from the class or agent default, not per-day overrides.
  - A loader's t/h rate already includes the swing and truck exchange, so
    this is the loader's time per truck. It is also the truck's time under
    the loader, near enough. The bucket-by-bucket formula (first-pass delay +
    passes × bucket cycle) is a later refinement and needs bucket data that
    isn't in the model.
- **Spot at dump plus dump:** one class field, **dump time** in seconds
  (default 60). Plus an optional per-destination override for crushers,
  which often queue at a single tip.
- **Travel time:** a shortest-time path over the graph. The loaded and empty
  legs are separate searches, because uphill loaded is not downhill empty and
  the two legs may take different roads. On each edge:
  - **Speed** = the lowest of three limits:
    - the class's **grade speed** for that edge's grade and load state (see
      below);
    - the road's speed limit;
    - the class's maximum speed.

    Industry tools always take the lowest applicable limit.
  - **Start and stop:** each leg starts and ends at rest, so add a fixed
    acceleration loss at each end: `v / (2a)` for the first and last edge's
    speed `v`, with one network **acceleration** setting (default
    1.5 km/h/s, folded away). The papers show that ignoring this
    underestimates short hauls badly. Published speed factors for a unit
    starting from rest on a short section run as low as 0.25–0.5 of the
    maximum. This one term captures most of that without a
    velocity-profile simulation.
- **Maximum grade per class** (default 10 %): the steepest grade the class
  may climb or descend. It shapes the access leg (3.2). Roads steeper than it
  are flagged as issues but still used, because the road is the design and
  the flag tells the planner to check it.
- **Grade speed table per class** (replaces the two speed fields). Rows are
  grade bands, each with a loaded and an empty speed. This mirrors how sites
  set loaded and unloaded speed limits by grade, and stands in for rimpull
  and retarder curves:

  | Grade from | Loaded km/h | Empty km/h |
  | --- | --- | --- |
  | −100 % (steep down) | 15 | 25 |
  | −6 % | 25 | 35 |
  | −2 % (flat) | old loaded speed | old unloaded speed |
  | 2 % | 20 | 40 |
  | 6 % (ramp) | 11 | 22 |

  Each row applies from its grade up to the next row's. Users can add or
  remove rows. The defaults are generic for a large rigid truck and are
  documented as starting values to replace with site figures.
- **Queuing is excluded.**
  - For scheduling, industry practice is "theoretical" truck matching with
    zero minimum queue: fractional trucks, no queue, no loader hang. That is
    exactly what the truck-hour budget does.
  - The research is split on which way reality deviates. Bunching on shared
    roads lengthens cycles; good dispatching meets targets with fewer trucks
    than a deterministic estimate.
  - Planners who want an allowance already have the class's utilisation
    calendar row, and wet-season slowdowns go there too. Say so in the docs
    rather than adding a parameter.
- `truck_hours_per_tonne = cycle_h / payload`.

**Semantic change (must be documented, not silent):** spot, load and dump
time now count, so every route uses more truck-hours than before, even in
projects without roads.
- Old files deserialize with spot and dump time 0.
- Each class's grade table is built from its old flat speeds: the old loaded
  and unloaded speeds on every row.
- So an old project without roads differs from before only by load time.
- Record the change in the new doc (section 5, phase 4) and in the module
  docs of `trucking.rs`. See decision D1.

### 3.4 Where the computation lives

- **`model/haulage/`** (new module; pure model code, wasm-safe):
  - `network.rs`: the `HaulNetwork` entity and its edits (draw, split, join,
    move, delete, convert polylines, roles), the issue list, and a
    nearest-point spatial index (uniform grid over road bounding boxes) for
    access candidates.
  - `routing.rs`: Dijkstra over road time for a given class and load state.
    - Run one search from each dump and reclaim point per class per direction
      (reverse graph for the loaded leg into a destination).
    - Each block then takes the minimum over its access candidates (3.2).
    - That gives points × classes × 2 searches, not one per candidate. Edge
      times depend on the class, so the graph is shared and the weights are
      computed per class.
  - Haulage settings on the network: join tolerance, auto-join distance,
    bench speed, acceleration.
- **One formula.** `trucking::coefficients` gains a variant that takes the
  route's computed breakdown:
  `CycleBreakdown { spot_h, load_h, loaded_h, dump_h, empty_h, loaded_km,
  empty_km, rise_m }`. The fixed-distance path builds the same breakdown.
- **Capture.**
  - Capture runs off the UI thread, so it takes an owned clone of the
    network, which is small.
  - Compute breakdowns and attach them to `MovementCandidate`.
  - Keep `truck_hours_per_tonne` as the field the solvers read, and add the
    breakdown beside it for publishing. The solvers must not change.
- **Fingerprint.**
  - Hash into the run fingerprint (`schedule_run.rs`):
    - the network's content and settings;
    - the new class fields, loader spot times and the loader rates used for
      load time.
  - Editing a road then recalculates; other object edits don't.
- **Caching:** cache the search results by that hash if they're slow on real
  data. Measure first.

### 3.5 Publishing

- Each delivery in the result gets the cycle breakdown of the candidate that
  produced it, or an index to it. The views can then show the average cycle,
  haul distance, rise and tonne-km per class, loader and day without
  recomputing.
- Keep the result's size modest: store breakdowns per candidate, not per
  delivery, if deliveries can reference their candidate.

## 4. UI

Keep the Haulage tab about **roads and where trucks join them**. Truck
classes and trucking rules stay in Schedule Setup, where the fleet calendars
are (decision D2).

### 4.1 Haulage → Layout (the 3D viewport)

Replace the scaffold step list with a small side panel, in the style of the
Schedule Setup lists:

- **Tools:** Draw road, Convert selection to roads, Import DXF as roads,
  Export roads.
- **Network:** one summary line, for example "42 roads · 3 issues · 118 of
  120 blocks connected". The issues are listed below on expansion, and
  clicking one frames the camera on it.
- **Destinations:** one row per destination with its method ("Roads ·
  node", "Nearest road", or "Fixed 2.0 km"). Clicking a row frames its dump
  point. Its menu offers Pick node, Make nearest point a node, and Use fixed
  distance.
- **Settings** (folded): join tolerance, auto-join distance, bench speed,
  acceleration.

**On this page:**
- roads are tinted by grade (flat neutral, uphill warm, steeper than some
  class's maximum red);
- dig blocks are tinted faintly, green when connected and red when not.

Hovering a road shows its name, length, grade and speed limit. Hovering a
node shows its role.

**Route check** (the haul query). Pick a dig block, or a stockpile for
reclaim, then a destination and a truck class.
- The view highlights the loaded path in red and the empty path in yellow,
  including the access leg. When grade lengthened the access leg, it is drawn
  dashed.
- A card shows:
  - the cycle breakdown (spot, load, haul, dump, return, total minutes);
  - one-way distance and rise;
  - t/h per truck, and **trucks to keep the loader busy** (loader rate ÷
    rate per truck, a fractional number);
  - a small **elevation and speed profile** along the route, reusing the
    Charts page's drawing code.

This is how a planner learns to trust the model, so it's worth getting right.

Block tints may need the existing solid or block highlight path; check how
`solids_view.rs` and `dig_strips.rs` tint blocks.

### 4.2 Schedule Setup

- **Truck Classes:**
  - Replace the two speed fields with the grade speed table, plus
    **maximum speed**, **maximum grade** and **dump time (s)**.
  - Show a greyed example line under the table once a schedule exists, for
    example "Busiest route: 23.4 min cycle". Optional.
- **Loader Classes:** add **spot time (s)**.
- **Stockpiles, Dumps and Crushers:**
  - The distance field stays as the fixed fallback, labelled "Fixed haul
    distance".
  - Show the dump-point method beside it, read-only, with a link to Haulage.
  - Crushers get an optional dump time override.

### 4.3 Inspection (reuse the inspection views)

- **Inspector → Trucks:** keep "X of Y in use". On hover, show the average
  cycle in minutes and the haul distance for the hour.
- **Inspector → Loaders:** on hover, add "Trucks to match: 4.3" for the
  current route.
- **Gantt bar hover:** add the average cycle for the bar's deliveries, one
  line.
- **Calendar → truck rows:**
  - add "Avg cycle (min)" and "Tonne-km" rows per class;
  - give each a Total column entry (tonne-weighted cycle, summed tonne-km).
- **Report → Trucks table:** add avg cycle, loaded km, rise and tonne-km.
- **Report → new Haulage table:** one row per period, loader, source and
  destination, with the cycle breakdown. This is what haulage audits export,
  and what cost models consume.
- **Charts:** a "Trucks in use" chart per class against the fleet line,
  reusing the stockpile chart's capacity-line drawing.
- **Idle reason** `NoTrucks`: its hover can name the class and its average
  cycle that day.

## 5. Phases

Each phase is committed separately and checked with screenshots.

**Phase 1: the haul network entity and its tools (no schedule effect).**
- `HaulNetwork` model, edits, undo and persistence, with focused tests:
  - draw, split and join keep topology;
  - convert polylines: join tolerance, T-junction split, overpass not
    joined;
  - deleting a node deletes its roads;
  - issues: dead ends, near misses, pieces;
  - grade sign.
- `SceneEntityId` variants, scene drawing, picking and hover.
- Tools: draw road, edit nodes and shape points, split, join, delete, set
  speed limit, rename. Convert selection, DXF import "as haul roads", and
  export.
- Menus in all three places (`AGENTS.md`).
- Layout side panel: tools, network summary and issues.
- Wire `SetPlanningSubpage` for Haulage, which is ignored today.

**Phase 2: roles, block connection and route check.**
- **Node roles:** promote a node to a dump or reclaim point, or create a
  new stockpile, dump or crusher on it. Nearest-road defaults for solid
  destinations.
- **Block access** with the grade-limited leg, and the connected/unconnected
  tint.
- `routing.rs` with tests:
  - a straight flat road matches the analytic cycle, including the
    acceleration loss;
  - a ramp is slower uphill loaded;
  - a speed limit caps speed;
  - directionality;
  - a road 12 m above a block costs a 120 m leg at 10 %;
  - the best join beats the nearest when the nearest is up a cliff;
  - a disconnected network falls back.
- The Route check card, path highlight and profile.

**Phase 3: cycle model into the schedule.**
- Class fields (grade table, maximum speed, maximum grade, dump time) and loader spot time,
  with serde defaults and migration from the old speeds.
- Breakdown-based `coefficients`.
- Capture builds the network and per-block breakdowns, with warnings for
  unconnected blocks; extend the fingerprint.
- Check equivalence: a project with no roads, and with spot, load and
  dump forced to zero, must produce the same schedule as before.
- Publish breakdowns.
- Update `trucking.rs` module docs and remove the
  `#[allow(dead_code, reason = "...later stage")]` attributes that this
  consumes.

**Phase 4: inspection and docs.**
- The Inspector, Gantt, Calendar, report and chart additions from 4.3.
- Docs: write `docs/scheduling-haulage.md`, covering:
  - what the model counts and excludes;
  - the default speeds;
  - the fallback rules;
  - the semantic change.
- Update the `AGENTS.md` table: add a "Haul roads and cycle times" row and
  amend the truck row.
- Delete this plan.

**Later (ask before starting).** Ordered by likely value:
1. **Roads that open and close over time,** such as staged ramps: open from
   or until a day, per road or group of roads. This needs per-period coefficients, meaning
   candidates split at the change days, which does touch the formulation.
2. **Cornering speeds:** speed by deflection angle at switchbacks and
   intersections. This needs an edge-based path search.
3. **Slow points, stop signs and one-way roads** on roads and nodes. Per-road speed limits are already in phase 1.
4. **In-pit and ex-pit split:** waypoints, or a road tag, so reports
   subtotal haulage by area.
5. **Exports:**
   - each scheduled haul as a 3D polyline (DXF);
   - fuel burn per cycle, feeding Cashflow.
6. **A power-limited uphill speed** from engine power, efficiency and
   gross weight: `v = 366.97 × kW × η / (TR × kg)`. This is the alternative
   to entering grade speeds by hand.
7. **Truck animation** along routes in Animate.

Out of scope entirely: tyre heat limits, battery and trolley trucks, and
traffic micro-simulation.

## 6. Decisions (agreed 2026-10-02)

- **D1.** Spot, load and dump time count for every project, including ones
  without roads, and the change is documented. **Agreed.**
- **D2.** Truck classes and rules stay in Schedule Setup, with Haulage
  holding roads, block connection and dump points. **Agreed.**
- **D3.** Speeds come from a grade speed table (loaded and empty) per class,
  plus road speed limits and one acceleration loss per stop. **Agreed.**
- **D4.** Roads are **dedicated haul-network objects** with explicit nodes,
  so nodes can be promoted to stockpiles, dumps and crushers, and roads can
  carry speed limits now and other rules later. DXF interoperability comes
  through "Convert to roads", "Import DXF as roads" and "Export roads".
  **Agreed, changed from plain polylines on ticked layers.**
- **D5.** Unconnected blocks schedule with a warning, joining a road in a
  straight line. The leg can't be steeper than the class's **maximum
  grade**: a road Δz above or below needs at least |Δz| ÷ max grade of
  travel (12 m at 10 % is 120 m). The join chosen is the quickest overall,
  not merely the nearest. **Agreed, with the user's maximum-grade rule.**

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
  - Never name any commercial mining or scheduling product in the repo.
- **Checks:**
  - Run `cargo clippy`.
  - Run `trunk build` once at the end, because the model code must compile
    for wasm (the solver and capture paths are native-only; the network and
    Layout view are not).
  - Translate all new UI text into `i18n/en/incline_design.ftl`.
- **Known open issue, unrelated:** with a Reclaim bar on DreamLand,
  Improve's seed completion is infeasible. Don't chase it as part of this
  work.

## 8. Background reading

These are available locally to the user; the implementer doesn't need them.

- **Industry haul-tool documentation.** Source of:
  - the cycle definition and theoretical truck matching;
  - bench access (auto-join, bench-floor travel, tiered bench speeds);
  - separate stockpile dump and reclaim points;
  - "lowest applicable speed limit wins";
  - grade speed limits by load state;
  - an issue list for network errors;
  - the haul query with loaded and empty routes.
- **Choi & Nieto 2011,** *Automation in Construction* 20: least-cost haul
  routing.
  - Supports Dijkstra with a **directional** cost, since uphill and downhill
    differ, and a curve penalty.
- **Upadhyay & Askari-Nasab 2012,** micro-simulation of haulage.
  - Speed from total resistance (grade plus rolling resistance) and power.
  - Speed factors for acceleration on short sections.
  - Bunching on shared roads lengthens cycles.
- **Tabesh, Upadhyay & Askari-Nasab 2016,** discrete event simulation of
  truck and shovel operations.
  - Road network built from exported polylines merged by gradient and
    length tolerance.
  - Segment speeds from gradient and rolling resistance.
- **Moradi Afrapoli, Tabesh & Askari-Nasab 2019,** *EJOR* 276: dispatching.
  - Good dispatching meets targets with about 85 % of the deterministically
    calculated fleet. Deterministic truck-hours are a planning figure, not a
    prediction.
- **Baek & Choi 2017,** *Applied Sciences* 7: haul road design by least-cost
  path, with Douglas-Peucker simplification. Road design itself is out of
  scope.
