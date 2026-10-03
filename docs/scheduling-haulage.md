# Haul roads and truck cycle times

Haulage has two pages. **Setup** holds the road network settings (join
tolerance, auto-join distance, bench speed, acceleration) and the truck
classes, each class's figures above its Grade Speeds. Setup has its own
pipeline (`app/haulage_pipeline.rs`) with Run Step, Run All and Auto: Road
Network reports the Layout's issues as warnings, Truck Classes checks each
class. Schedule's pipeline has a Haulage step that runs this pipeline when it
is not current and waits for it; its repair links open this page. **Layout** holds the road network,
destination dump and reclaim points, block connections and the Route check.
Loader classes stay in Schedule Setup. A road is a dedicated planning object: its ends are junction
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
blocks connected); the selected block; the selected road or node; the route
check; issues; destinations; roads. The explorer column carries a Solids
Navigation tree of its own, like Animate's: hide solids, benches, blasts or
flitches to see and click the blocks underneath. Hidden blocks are neither
drawn nor clickable, and a click takes the highest visible block under the
cursor. The selected block is filled orange, with a line to where it meets the
roads. Drag a node or bend point in the viewport to move it, with its roads
previewed until release; a still click only selects. Selecting a road shows its
name, length, steepest grade and speed limit; selecting a node shows its role
and coordinates. The role list also creates a new stockpile, dump or crusher at
the node and opens its setup. Delete (key or button) removes selected roads and
nodes; a node left with no road and no role goes with them, and deleting a
destination's node asks first. Issues name the road or node concerned, explain
themselves on hover, and frame and select it on click.

Each destination row has a choice of how trucks reach it: a selected node, the
nearest road to a solid's surface, a node added at that nearest point, or its
fixed distance. A node's role is a dump point, a reclaim point, or for a
stockpile both at once (**Dump & reclaim point**). Giving a node one half of a
role another node holds moves only that half; a stockpile made from a node, or
pointed at one, starts as dump & reclaim unless it already has a separate
reclaim point. The route check needs only a click on a dig block: destination,
truck class and loader start filled in, and the result recalculates whenever
the question or the project changes.

A block joins the network at the nearest point on any road, interior points
included. To choose instead, select the block and press **Choose node…**, then
click a node, or a point on a road, which splits the road there. The block is
then held to that node whatever is nearer; **Use nearest road** releases it. A
link is stored with the network by the block's solid, flitch base RL and a
point inside it, so a rerun that redraws strips keeps it with whichever block
covers that point. Linked blocks always count as connected, still pay their
grade-limited drive to the node, and show a blue line to it on the Layout.

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

New classes start with default grade speeds, from a large haul truck's
published curves at 398 t loaded and 165 t empty, with 2% rolling resistance,
each band taken at its starting grade. Uphill and level bands come from the
rimpull curve. Downhill bands are the top speed of the highest gear whose
standard retarding holds the effective grade (grade less rolling resistance,
scaled by weight when empty); the steepest band is taken at 15%:

| Grade from | Loaded km/h | Empty km/h |
| --- | ---: | ---: |
| −100% | 12.9 | 23.8 |
| −10% | 17.4 | 32.1 |
| −8% | 23.8 | 43.6 |
| −6% | 32.1 | 43.6 |
| −4% | 43.6 | 60 |
| −2% | 60 | 60 |
| 0% | 57 | 60 |
| 1% | 41 | 60 |
| 2% | 32 | 60 |
| 4% | 22 | 48 |
| 6% | 17 | 38 |
| 8% | 14 | 31 |
| 10% | 10 | 26 |

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

A block joins at the nearest road point by grade-limited access length. Only
points within a metre of that nearest one - the same junction reached along
different roads - compete on total cycle time; a farther point is never chosen
for being quicker overall, because the straight leg to it would cut through
the pit walls. (The first implementation took the least total cycle among all
nearby roads, which joined blocks to distant junctions.) The
initial auto-join distance is 300 m and join tolerance is 2 m. If no road is
nearby, the nearest road is considered. A block is connected when its
grade-limited access fits within the auto-join distance; a road 12 m above is
connected at 120 m, and the route check says the leg was lengthened. Unconnected
blocks still schedule; Readiness reports how many in one line, and Layout tints
them red. Route check draws a lengthened access leg dashed. A road end is
reported as a dead end only when no dig block is within the auto-join
distance of it and no block is linked to it: the end of a road into a working
face is where its blocks join.

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
roads: loading now counts. **Use default grade speeds** on a truck class's
Grade Speeds pane replaces migrated flat speeds with the grade-dependent defaults. Old truck classes migrate their two speeds to every
grade band and retain zero dump time; old loader classes retain zero spot time.
With spot, load and dump zero, fixed-distance coefficients match the previous
travel-only calculation. New classes receive the default settings above. Review class speeds and times before calculating production schedules.

## Animate

While Schedule → Animate shows a current result, every loaded haul under way at
the shown instant is drawn along its route. Each delivery adds its tonnes per
hour to each road segment it uses, so a road shared by several blocks shows
their sum. Stripes run from the loader towards the destination; they move
faster and the line is wider and warmer (blue to amber) the more tonnes per
hour cross it. Hovering a segment gives its rate, and a key in the corner gives
the busiest. Routes are worked out once per schedule and network, with the same
searches capture uses; only the projection and the stripes are redone each
frame, and only while flows are on screen.

The flows are scene strokes (`rendering/scene/build.rs::rebuild_flow_scene`),
depth-tested like any other line, so a solid still standing in front of a road
hides its band and stripes. A faint line along the same route is drawn first
without the depth test, so a haul behind a solid still shows where it runs.
The hover read-out and key are painted over the scene by
`ui/mod.rs::draw_haul_flows`, and hovering finds a road whether or not it is
hidden.

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

The second rework (Setup page, Solids Navigation, nearest-point joins, block
links, dump & reclaim roles and Animate flows) was driven on the same unsaved
sample: hiding benches and clicking a block beneath, linking a block to a ramp
end and seeing the route check follow, the Road network and Truck Classes
steps, setting ROM B's node to dump & reclaim, and scrubbing Animate with flows
moving to ROM A and OSA A. Temporary tests covered the nearest interior join,
a linked block's longer route, link persistence and the split dump/reclaim
roles, and were removed. Native clippy and a wasm `cargo check` passed (the
latter with its existing unused-trucking warnings).

Depth-tested flows were checked on the same sample from a low view across the
pit: the band and stripes stop where the ramp passes under unmined blocks and
the faint line continues through them, and the stripes still move.

Screenshots: [Layout with a selected block and route check](haulage/layout.png),
[Setup](haulage/setup.png), [Animate flows](haulage/animate-flows.png),
[truck settings](haulage/truck-settings.png),
[Calendar](haulage/calendar.png), [Inspector](haulage/inspector.png).
