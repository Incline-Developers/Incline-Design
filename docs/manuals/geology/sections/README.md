# Sections in Incline Design

Written for Incline Design **v0.4.0-dev**.

This page covers working in a vertical section through the drill holes: seeing where a surface cuts the section, saving those cut lines, and editing strings in the plane of the section. It is for geologists working in the Geology workspace.

## Contents

1. [Concepts](#1-concepts)
2. [Surface Lines on Section](#2-surface-lines-on-section)
3. [Save Surface Lines as Strings](#3-save-surface-lines-as-strings)
4. [Offset, Chamfer and Trim/Extend/Relimit on a Section](#4-offset-chamfer-and-trimextendrelimit-on-a-section)

---

## 1. Concepts

Where this fits: these tools serve step 4 of [Modelling](../README.md#modelling), in the Publish Model stage of the geologist's life cycle. A section is where the first strings are drawn, and where the reference surface is checked against the holes. Next in the flow: [Seam Thickness](../thickness/README.md).

A vertical section is a cut through the ground along a line you choose, seen from the side. Drill holes that lie near the line show as vertical traces with their seam intervals. Height is up the screen, distance along the line is across it.

A section has two jobs. At the start, before any surface exists, it is where the geologist draws: cut a section along a line of holes and draw the roof, the floor and the faults on it. A roof, a floor and a fault are easiest to see and to draw from the side, one section at a time, and each line drawn there is a control string. The section is the geologist's sheet of paper, and the tools on this page keep the pen on it. Every time after, it is where the built surface is checked against the holes, and where the strings are redrawn when new evidence disagrees.

A geologist checks the reference surface, roof or floor, against the holes in section because a surface can look right in plan and still miss the picks. In section you see at once whether the surface passes through each logged seam interval, or rides above or below it. Where it misses, **Save Surface Lines as Strings** keeps the lines the shown surfaces cut on the section as strings on a new layer. Move each line to its own surface's layer, redraw it in the plane of the section to your interpretation, and build again.

How a section is opened (this is the generic slice view, not geology work):

- Click **Vertical Section View** on the viewport bar, then click two points for the line of the section. The same button reads **Exit Section View** while a section is up.
- Drill holes, strings, points and block models show within the width of the slab, with **3D view** ticked or not. Surfaces show as the line where they cross the plane, or as a slab when **3D view** is ticked in the **Section View** panel.
- Clicking a hole in a section picks only a part of a hole the section draws.

General drawing tools (**Create Point**, **Create Line**, **Create Polyline**, **Measure Distance**, **Move Design** and the rest) work on a section as they do in plan and are not repeated here. **Drag a Vertex** and **Nudge RL...**, which work on a section too, are in [String Vertices](../string-vertices/README.md).

Where this page says the console, it means the activity console. It is closed at first; **View > Show Console** shows and hides it and remembers your choice. A warning or an error also opens an alert with its words.

## 2. Surface Lines on Section

### What it does
Draws, on the section, the line where each shown surface crosses the section plane, in the surface's own colour.
### Why a geologist uses it
To see, hole by hole, whether the surface runs through the logged seam interval, above it or below it, as Concepts says.
### Where it is
Geology workspace > **Geology Design > Surface Lines on Section**
### Before you start
The Geology workspace, with a section up, as in [Concepts](#1-concepts), and a surface loaded and shown, such as one made with [Build Surface](../surfaces/README.md#3-build-surface).
### Steps
1. Choose **Geology Design > Surface Lines on Section**. The entry takes a tick, and the console reads "Set surface lines on section = true".
2. Look at the section. Each shown surface that crosses it is drawn as a line in its own colour, along the whole crossing: past the two points that set the section, to the surface's edge. A crossing broken by a gap in the surface gives more than one line.
3. Compare each line with the holes. Where the line meets a hole's logged roof or floor, the surface passes through the pick; where it runs above or below the logged interval, it misses. Where a control string overrode a pick, the line follows the string's height, not the pick's.
4. Choose the entry again to switch the lines off; the console reads "Set surface lines on section = false".

![Geology workspace, section view looking north: the part of the surface A Roof between S1 and S2, a line from RL 79.5 to RL 76, across the seam A roof picks](images/01-surface-lines-section.svg)
### Options and defaults
| Option | Default | What it does |
|---|---|---|
| **Surface Lines on Section** | Off | Ticked: the lines are drawn whenever a section is up. Not saved with the project, so it is off again next session. |

### What it refuses and why
- Nothing is drawn with no section up, or outside the Geology workspace. An unloaded or hidden surface, or one that misses the plane, has no line.
### Related tools
[Save Surface Lines as Strings](#3-save-surface-lines-as-strings), [Build Surface](../surfaces/README.md#3-build-surface).

## 3. Save Surface Lines as Strings

### What it does
Writes the lines the section draws into a new layer as ordinary strings, one string for each line, each in its surface's colour.
### Why a geologist uses it
A drawn line can only be looked at. A string can be edited, offset, plotted and checked against other strings, so the cut becomes something you work with. This is the section check of Modelling step 4, as [Concepts](#1-concepts) describes.
### Where it is
Geology workspace > **Geology Design > Save Surface Lines as Strings**
### Before you start
The Geology workspace, a section up and **Surface Lines on Section** ticked, as in [section 2](#2-surface-lines-on-section), and a project open. The entry stays grey until the section and the lines are on.
### Steps
1. With the surface lines drawn on the section, choose **Geology Design > Save Surface Lines as Strings**.
2. A new layer `Section Lines` appears and becomes the active layer. If that name is taken the layer is `Section Lines (2)`, and so on. It holds one string for each line drawn, each in its surface's colour; a line that closes on itself is saved as a closed string.
3. The console reads "Saved", the number of strings, "string(s) from", the number of surfaces, "surface(s) to" and the layer's name, for example "Saved 1 string(s) from 1 surface(s) to Section Lines". Each string runs the whole crossing, on past the holes to the edge of the surface. Ctrl+Z removes the layer in one step.

![Geology workspace, section view looking north: the surface line of A Roof saved as an ordinary string in the layer Section Lines, the part between S1 and S2 running from RL 79.5 to RL 76](images/02-save-surface-lines-string.svg)
### Options and defaults
None.
### What it refuses and why
- Entry greyed with no section up or the lines off. The warning for that case is "Turn on Surface Lines on Section with a section up before saving them".
- "Open a project before saving surface lines".
- "No shown surface crosses the section; nothing saved". No layer is made.
### Related tools
[Surface Lines on Section](#2-surface-lines-on-section), [Offset, Chamfer and Trim/Extend/Relimit on a Section](#4-offset-chamfer-and-trimextendrelimit-on-a-section).

## 4. Offset, Chamfer and Trim/Extend/Relimit on a Section

### What it does
**Offset** copies a string parallel to itself, **Chamfer** rounds a corner, and **Trim/Extend Line** ends a string on another or sets its length. On a section each works in the section's plane, so the result stays on it.
### Why a geologist uses it
A floor is often drawn from a roof and a thickness, and a roof is run out to a fault line. Working in the plane keeps the result where the section shows it.
### Where it is
Geology workspace > **Geology Design > Edit > Offset**, **Trim/Extend Line** and **Chamfer Polyline Corners**. In other workspaces they do not work on a section: in Production their toolbar buttons stay greyed, with the tool's name in the hover, for example *Offset - not available in the section view*.
### Before you start
The Geology workspace, a section up and the string lying on it, such as one saved with [Save Surface Lines as Strings](#3-save-surface-lines-as-strings).
### Steps
1. **Offset**. Choose **Geology Design > Edit > Offset** and click the string. The panel **Offset Element** shows only **Distance square to the string** and **Collide with Triangulation**. Type the distance and press **Pick Side**. At "Choose offset side" click on the side the copy goes. The copy lies that far away, measured square to the string in the plane of the section, so under a sloping string it moves a little more than that distance in height. The console reads "Created offset of", the count, "object(s)".
2. **Chamfer** and **Trim/Extend Line**. **Chamfer Polyline Corners**: the panel **Chamfer** reads "Click a corner of a string on the section." Click a corner and press **Apply**, with **Segments** 8 and **Radius** 1 m unless you change them. On a section an inner corner of an open string rounds too. **Trim/Extend Line**: click the string, press **Apply and Pick Target**, click the target at "Select the polyline or circle to trim or extend to", then at "Choose the side to keep" click the piece that changes: the gap the string should grow into, or the part to cut off. It lands where the two meet in the plane.

![Geology workspace, section view looking north: the saved A Roof string as a grey dashed line and its offset, 2 m below it square to the string, in orange](images/03-offset-section-string.svg)
### Options and defaults
| Option | Default | What it does |
|---|---|---|
| **Distance square to the string** | 0 m | How far the copy lies from the string. **Pick Side** stays grey at 0. |

### What it refuses and why
- **Offset**: "A circle cannot be offset on a section", since a circle is drawn in plan. A string off the plane gives "A string that does not lie on the section cannot be offset on it".
- **Chamfer**: "This corner cannot be chamfered on the section: the string must lie on it and bend at the corner". End vertices are not corners.
### Related tools
[Save Surface Lines as Strings](#3-save-surface-lines-as-strings), [Control Strings](../control-strings/README.md), [String Vertices](../string-vertices/README.md).
