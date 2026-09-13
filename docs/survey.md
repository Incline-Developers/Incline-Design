# Survey coordinates

Open the **Survey** workspace, then **Coordinates**:

- **Definitions…** opens the coordinate-system editor.
- **Transform…** opens a draggable From/To transformation window.

Both windows use the application's `DragableMenu`, `MenuField*`, `MenuButton`
and menu layout helpers. The Coordinates menu uses `MenuBarMenu` and
`ContextMenuAction`; macOS provides the equivalent native menu.

## Definitions

Laid out like Preferences: the systems down the left, the selected one's fields
on the right. Every row is a coordinate system; there is no anonymous frame
behind them.

Right-click the empty space below the list to add a system, or a row to delete
it or make it the **mine coordinate system** - the system the project's numbers
are already in, marked with an arrowhead in its gutter. A system is one of:

- **Registry system** - an entry in the embedded EPSG registry, found by typing
  a name or code ("mga zone 56"). Every word has to match, so a search narrows
  rather than listing every system sharing a word. New systems start here, on
  WGS84, so a freshly added row is always a valid one.
- **Grid over another system** - a named grid defined by a known point in its
  parent's coordinates, the same point in its own, a rotation about Z and a
  uniform scale. The parent is named, not embedded: renaming a system carries
  its children with it, and a system other grids are defined against cannot be
  deleted until they are pointed elsewhere.

Every chain therefore bottoms out at a registry system, which is how a mine grid
is defined on paper: a relationship to a national grid. Grids stack - a pit grid
over a mine grid over a registry system is three systems and two transforms,
collapsed to one before any data moves, angles adding and scales multiplying. A
grid whose parent chain leads back to itself is refused when saved.

### Axis names

A system can say what it calls its axes. A mine grid writing easting, northing
and reduced level puts `E`, `N`, `RL` in its **Axis names** fields, and every
coordinate readout in the application follows: the status bar, the object
editor, block-model slice ranges and the definition fields themselves.

The names belong to the system, not to the application, so they apply only while
that system is the mine coordinate system - choosing a different one changes the
labels with it. Name all three or none; a blank falls back to its letter, so a
half-filled set cannot leave one readout labelled with nothing.

Where there is only room for a couple of characters - the orientation gizmo's
arms, the viewport bar's elevation field - the name is cut to its first two
characters rather than shrunk to fit, so "UPWARDS" reads "UP".

Edits write through to the config as each field's edit lands, the way
Preferences do.

## Transform

Select project data, then open **Coordinates → Transform…**. Choose its current
system under **From** and its destination under **To**. From starts on the mine
coordinate system, because that is where the project's numbers already are. Both
selectors offer the saved systems and nothing else. **Swap** exchanges them;
source and destination must differ.

The window states which published operations the conversion will go through and
their stated accuracy before it runs, and refuses any pair of systems it has no
published route between.

The selection is converted **in place**, in one undo step: the same designs on
the same layers, the same meshes under the same names and ids. Nothing is
duplicated and nothing is renamed, so anything else pointing at an item - a
drape, a style, an explorer position - still points at it afterwards. A design
layer's elevation moves with the objects on it. Undo puts every converted item
back as it was.

Supported data: design points, polylines (including arcs/circles), text,
triangulations, block models, point clouds and drillhole datasets. Collars and
trace stations move with the grid; depths, diameters and interval ranges are
measured along the hole, so they follow the scale factor only and are untouched
at a scale of 1. Ties and initiations name holes by index and survive
unchanged. Mesh connectivity and block-model
attribute columns are preserved. Block origins, orientations and local cell
dimensions transform together; point colours retain their association with
points. Spatial indexes, render preparation and bounds are rebuilt as needed.

Converted items are rewritten under the same identity, which is a case the
scene caches were not originally built for: a surface's geometry, a raster's
placement and a block model's origin could previously only arrive or leave, so
the caches watched style and slicing and took the rest as fixed. They now key on
the geometry itself - a surface on its mesh allocation, a raster on its
world-to-texture map, a block model on its placement and block source - so a
conversion redraws instead of leaving the old picture on screen.

Work runs in the background. Invalid or unloaded selections are rejected; a
failed transformation changes nothing. Changing source data or switching the
active project can discard a pending result. Design objects must belong to the
active project. Completion and cancellation are reported in the activity console.

Scale affects all three axes. Geoid/AHD height conversions are not supported:
heights pass through a reprojection untouched. Converting a surface drops its raster drape,
which has to be re-draped afterwards.

Rasters are selected from their explorer row, or alongside a surface they are
draped over when that surface is clicked in the viewport - the drape has no
geometry of its own, so it selects and deselects with the surface wearing it. A
marquee never takes one. Converting a raster composes the transform into its
world-to-texture map and reads no pixels, so the image stays exactly as sharp as
it was.

## Grid changes and reprojection

A conversion is one of two things, and the difference decides what can come
along. A **grid change** is affine: straight lines stay straight and every
regular structure survives, because its parameters can simply be restated. A
**reprojection** - a change of projection or of reference frame - is not. A
regular block grid in one projection is not regular in another, and a raster's
affine placement has no counterpart on the far side.

So block models and rasters require a grid change and are refused, with a
reason, under a reprojection; converting them would mean resampling and losing
what they carry. Designs, triangulations, point clouds and drillhole positions
are made of independent points and convert exactly either way. Depths, diameters
and interval ranges follow a grid's scale factor only: a hole drilled 30 m deep
is 30 m deep in any coordinate system.

The Transform window states which published operations a conversion will go
through and their stated accuracy before it runs, and refuses any pair of
reference frames it has no published transformation for.

## Validation

Focused temporary tests cover conversion between two named systems and the
reference frame, local destination defaults, isolation from unsaved definition
edits, native/browser config round trips and old-config defaults, and keyboard
confirmation/closing of both custom dialogs. They are removed after passing as
required by the repository instructions. Earlier checks covered transformed
geometry, block attributes, and mixed-item undo/redo.

No QGIS code or assets are copied. The implementation adds no dependencies.
