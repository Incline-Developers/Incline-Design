# Haul roads and truck cycle times

Haulage → Layout holds the road network, destination dump and reclaim points,
connection checks and the Route check. Truck and loader classes stay in
Schedule Setup. A road is a dedicated planning object: its ends are junction
nodes and its intermediate vertices only shape the road. Moving a node moves
all incident roads. Undo preserves the network's allocated IDs. OMF stores the
network, settings and roles; older projects start with an empty network.

Draw a road by clicking points and finish with Enter, Escape or a
double-click; Backspace removes the last point. Points land on the surface
under the cursor, so a road drawn in plan drapes onto the pit, and fall back to
the previous point's level where there is no surface. A snap mode chosen on the
viewport toolbar still applies. Clicking an existing road or node joins it.
Convert selected design polylines, or choose **DXF → Import DXF as roads**.
Conversion keeps the original design objects, flattens bulges, joins
T-junctions within the 3D tolerance and keeps elevated crossings separate.
Export writes one DXF polyline per road.

The Layout panel reads top to bottom: tools; a one-line summary (roads, length,
blocks connected); the selection; issues; roads; destinations; the route check;
settings. Drag a node or bend point in the viewport to move it, with its roads
previewed until release; a still click only selects. Selecting a road shows its
name, length, steepest grade and speed limit; selecting a node shows its role
and coordinates. The role list also creates a new stockpile, dump or crusher at
the node and opens its setup. Delete (key or button) removes selected roads and
nodes; a node left with no road and no role goes with them, and deleting a
destination's node asks first. Issues name the road or node concerned, explain
themselves on hover, and frame and select it on click.

Each destination row has a choice of how trucks reach it: a selected node, the
nearest road to a solid's surface, a node added at that nearest point, or its
fixed distance. The route check needs only a click on a dig block: destination,
truck class and loader start filled in, and the result recalculates whenever
the question or the project changes.

## Cycle model

Each candidate movement has its own cycle:

`spot + load + loaded travel + dump + empty return`

Loading is truck payload divided by the loader class's nominal dig or reclaim
rate. Loader availability and utilisation affect scheduled loader hours, not
this loading time. Spot time belongs to the loader class; dump time belongs to
the truck class, with an optional crusher override. New loader classes start
with 45 seconds spotting, and new truck classes with 60 seconds dumping.

Road travel uses 3D segment length and signed grade. Grade is elevation change
divided by plan length. Each truck class supplies loaded and empty speed bands;
a band's lower grade bound applies up to the next band. The lowest bound also
covers grades below it. The selected speed is capped by the class maximum and
any road speed limit. The return traverses the same topology in the opposite
direction, with its own least-time path and signed grades.

Generic starting speeds for new classes are placeholders for site figures:

| Grade from | Loaded km/h | Empty km/h |
| --- | ---: | ---: |
| −100% | 15 | 25 |
| −6% | 25 | 35 |
| −2% | 20 | 50 |
| 2% | 20 | 40 |
| 6% | 11 | 22 |

The initial maximum speed is 50 km/h and maximum grade 10%. Acceleration is
1.5 km/h/s. A road journey counts the acceleration/deceleration loss at the
loading and dump ends: `speed / (2 × acceleration)` for each end of each leg.
Passing a junction adds no stop. Road grades above the maximum are flagged;
the maximum grade lengthens access legs rather than banning authored roads.

## Access and fallback

A dig block's source point is its plan anchor at the flitch floor. Its access
length is the larger of direct 3D distance and `abs(height change) / maximum
grade`. A road 12 m above a block at 10% therefore requires at least 120 m of
access. Its effective grade is height change divided by that access length.
Dig access is capped at the bench speed, initially 15 km/h.

The query compares projections onto nearby roads plus the nearest junction,
choosing the least total cycle rather than simply the nearest road. The
initial auto-join distance is 300 m and join tolerance is 2 m. If no road is
nearby, the nearest road is considered. A block is connected when its
grade-limited access fits within the auto-join distance; a road 12 m above is
connected at 120 m, and the route check says the leg was lengthened. Unconnected
blocks still schedule; Readiness reports how many in one line, and Layout tints
them red. Route check draws a lengthened access leg dashed.

Explicit dump/reclaim roles take priority. Solid destinations can use the
nearest road to their surface centroid; trucks then also drive the straight,
grade-limited leg from that road to the centroid, both ways. Standalone
destinations need an explicit node to use roads. Reclaim uses the reclaim node when present and
otherwise the dump point. A destination can be forced to its fixed distance.
If a source or destination point is unavailable, or the selected road
component cannot reach the destination, capture uses the destination's fixed
one-way distance and the class's flat speeds. Spot, load and dump still count.
The fixed-distance fallback has no road endpoint acceleration correction.

The model excludes queuing, traffic, cornering, stops at junctions, one-way
restrictions, fuel, time-dependent road availability and truck animation.

## Schedule and inspection

Truck-hours per tonne are total cycle hours divided by payload. Candidate
cycles are captured once and shared by their published deliveries. Haulage
edits and class, loader or destination timing edits retire calculated results.
The Inspector and Gantt show cycle detail on hover; the Calendar adds average
cycle and tonne-km per class. Reports include truck haul summaries and a
Haulage table grouped by period, loader, activity, source and destination.
Cycle, distance and rise averages are tonne-weighted; tonne-km is summed.
Truck charts compare simultaneous use with the available fleet.

This changes the previous travel-only coefficient even for projects without
roads: loading now counts. **Use generic starting speeds** on a truck class
replaces migrated flat speeds with the grade-dependent defaults. Old truck classes migrate their two speeds to every
grade band and retain zero dump time; old loader classes retain zero spot time.
With spot, load and dump zero, fixed-distance coefficients match the previous
travel-only calculation. New classes receive the generic starting settings
above. Review class speeds and times before calculating production schedules.

## Validation

Validated on Linux desktop against the unsaved DreamLand sample: road drawing,
endpoint picking, context actions, destination creation, dig-block connection
cues, route checking and profiles, and normal schedule calculation through day
7. The Calendar and Inspector displayed published cycle figures. The sample
was closed without saving.

Ten temporary focused tests passed for topology, conversion, overpasses,
directional routes, acceleration, grade-limited access, fastest joins,
disconnected fallback, migration, candidate capture and OMF persistence. Six
existing project capture checks also passed, including solve/replay and report
construction. Temporary haulage tests were removed as required by AGENTS.md.
Native `cargo clippy` and `trunk build` passed. The web build reports native-only
trucking code as unused. Browser interaction and macOS/Windows interaction
have not been exercised; DXF file-dialog round trips remain a manual check.

The Layout rework was driven on the same unsaved sample: draping drawn roads,
dragging nodes, assigning a crusher node from the destination list, the
auto-filled route check, the single Readiness line, the truck band table and
a schedule calculation. Temporary tests covered the destination access leg, the
120 m grade-limited access counting as connected, and loose-node pruning, and
were removed. Native clippy and a wasm `cargo check` passed.

Screenshots: [Layout and route check](haulage/layout.png),
[truck settings](haulage/truck-settings.png),
[Calendar](haulage/calendar.png), [Inspector](haulage/inspector.png).
