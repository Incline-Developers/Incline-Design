# Incline — English message catalog (canonical source).
#
# Every `tr!(...)` call in the code is checked against THIS file at compile time:
# an unknown id or a missing argument fails the build. Other languages
# (`i18n/<lang>/incline_design.ftl`) may be incomplete and fall back here.
#
# Ids are kebab-case, grouped by area with a prefix (`menu-`, `settings-`,
# `tri-`, `common-`, ...). Keep this file grouped and roughly sorted.

## Shared

common-cancel = Cancel
common-clear = Clear
common-close = Close
common-fill = Fill
common-set = Set

## Status bar

# Title of the status bar's language menu. The languages themselves are never
# translated: each names itself in its own script, from `LanguageChoice`.
status-language = Language

## Menu bar — File

menu-file = File
menu-file-save-project = Save Project
menu-file-save-project-as = Save Project As...
menu-file-new-project = New Project...
menu-file-open-project = Open Project...
menu-file-open-recent = Open Recent
menu-file-show-in-explorer = Show in Explorer
menu-file-show-in-folder = Open Containing Folder
menu-file-import = Import...
menu-file-export = Export...
menu-file-export-viewport-image = Export Viewport Image...
menu-file-export-engineering-drawing = Export Engineering Drawing...
menu-file-about = About { $app }...
menu-file-exit = Exit Application

## Menu bar — View

menu-view = View

## Workspaces

ws-production = Production
ws-drill-and-blast = Drill & Blast
ws-geology = Geology
ws-planning = Planning

## Menubars

ws-menubar-design = Design
ws-menubar-triangulation = Triangulation
ws-menubar-raster = Raster
ws-menubar-point-cloud = Point Cloud
ws-menubar-block-model = Block Model
ws-menubar-drillholes = Drill Holes
ws-menubar-active-layer = Layer:

## Menubars functions

ws-menubar-design-insert-point = Insert Point
ws-menubar-design-insert-point-at-intersection = At intersection
ws-menubar-design-insert-point-at-elevation = At elevation
ws-menubar-design-move-to = Move to
ws-menubar-design-create-triangulation = Create Triangulation

## Rename / delete item dialogs

# { $kind } is a workspace noun from the ws-production-* set above.
dialog-rename-title = Rename { $kind }
dialog-rename-field = New name
dialog-rename-field-hint = Required
dialog-rename-submit = Rename
dialog-delete-title = Delete { $kind }
dialog-delete-confirm =
    Delete '{ $name }' from the project?
    This cannot be undone.
confirm-delete-product =
    Delete product '{ $name }' from the palette?
    This cannot be undone.

## Create Triangulation dialog

tri-create-title = Create Triangulation
tri-create-help = Triangulates the objects selected when this dialog opened. Close it to change the selection.
tri-create-type-label = Triangulation type
tri-create-type-help =
    Open surface creates a terrain-style sheet. Solid creates a fully enclosed
    mesh and requires input that can form a watertight boundary.
tri-create-output-name = Output name
tri-create-output-name-help = Name assigned to the generated triangulation.
tri-create-output-name-hint = triangulation name
tri-create-run = Triangulate

tri-selection-none = The selected objects are no longer available.
tri-selection-selected = { $summary } selected

tri-type-open-surface = Surface
tri-type-solid-closed = Solid

# Selection summary pieces, e.g. "3 polylines, 1 point". Each noun is pluralised
# by its own count so languages with more than two plural forms read correctly.
tri-count-polylines =
    { $count ->
        [one] { $count } polyline
       *[other] { $count } polylines
    }
tri-count-strings =
    { $count ->
        [one] { $count } string
       *[other] { $count } strings
    }
tri-count-circles =
    { $count ->
        [one] { $count } circle
       *[other] { $count } circles
    }
tri-count-points =
    { $count ->
        [one] { $count } point
       *[other] { $count } points
    }
tri-count-texts =
    { $count ->
        [one] { $count } text object
       *[other] { $count } text objects
    }
tri-count-objects =
    { $count ->
        [one] { $count } object
       *[other] { $count } objects
    }

about-read-full-licence = Read the full licence ↗
about-source-code = Source Code
about-website = Website
about-title = About { $app }
drill-hole-colour-stop = Stop { $index }
properties-restore-defaults-tooltip = Reset the { $heading } settings to their defaults

## Dynamic UI messages

ui-selected-count = { $count } selected
ui-selected-objects = { $count } object(s) selected
ui-selected-polylines = { $count } polyline(s) selected
ui-invalid-axis-value = Enter a valid { $axis } value.
ui-selection-spans = Selection spans { $min } to { $max }.
confirm-delete-count = Are you sure you want to delete { $count } selected item(s)?
confirm-delete-layer = Delete layer '{ $name }' and all objects on it?
    This cannot be undone.
plot-preview-pixels = { $width } × { $height } px at { $dpi } dpi
tri-estimated-memory = Estimated peak memory ~{ $estimate }. { $detail }
block-grid-summary = Grid: { $x } × { $y } × { $z } = { $count } blocks
status-selected = Selected: { $count }
status-faces = Faces: { $drawn } / { $total } ({ $drawn_chunks }/{ $total_chunks } chunks)
status-clip = Clip near/far/Δ: { $near } / { $far } / { $delta } m
status-points = Points: { $drawn } / { $target } of { $total } ({ $drawn_chunks }/{ $total_chunks } chunks)

explorer-no-rasters = No rasters
slice-viewport-gestures = middle-drag pan · right-drag orbit · Shift+wheel walk · W/S move slab · Q/E rotate · Esc exit

## Startup environment details

## Renderer startup diagnostics

## Edit Object dialog

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Index
color-rgb = RGB
color-opacity = Opacity
color-edit = Click to edit colour
color-saturation-value = Saturation and brightness
color-hue = Hue

asset-loading = Loading asset data
asset-unloading = Unloading asset data
asset-load-failed = Could not load asset data
asset-unload-failed = Could not unload asset data

preferences-title = Preferences

context-text-colour = Text colour

context-polylines = Polylines
context-points = Points

# Coordinate systems
crs-unknown-ellipsoid = Unrecognised earth model "{ $name }" in this coordinate system definition.
crs-no-ellipsoid = This coordinate system definition does not say what earth model it uses.
crs-unknown-code = EPSG:{ $code } is not in the coordinate system registry.
crs-transform-failed = A coordinate could not be converted; the result was not a finite position.
crs-no-datum-path = No published transformation is available between the reference frames of { $from } and { $to } (EPSG datums { $source } and { $target }). Converting anyway would be wrong by an unknown amount, so nothing was changed.
crs-unknown-datum = The reference frame of { $from } or { $to } cannot be identified, and the two use different earth models. Converting between them would be wrong by an unknown amount.

# Survey workspace
ws-survey = Survey
survey-count-designs = { $count } { $count ->
    [one] design
   *[other] designs
  }
survey-count-meshes = { $count } { $count ->
    [one] triangulation
   *[other] triangulations
  }
survey-count-models = { $count } { $count ->
    [one] block model
   *[other] block models
  }
survey-count-clouds = { $count } { $count ->
    [one] point cloud
   *[other] point clouds
  }
survey-count-holes = { $count } { $count ->
    [one] drillhole dataset
   *[other] drillhole datasets
  }
survey-count-rasters = { $count } { $count ->
    [one] raster
   *[other] rasters
  }
survey-angle = Rotation about Z (counterclockwise)
survey-scale = Uniform XYZ scale factor
survey-invalid-transform = Origins, angle and resulting coordinates must be finite.
survey-invalid-scale = Scale must be a finite positive number with a finite reciprocal.
survey-empty-selection = Select at least one supported item to transform.
survey-unavailable = A selected item is missing or unloaded. Load it before transforming.
survey-wrong-project = Select designs from the active project only.
survey-name-required = Enter a coordinate system name.
survey-working = Transforming selected data…
survey-completed = Converted { $items } in place. Undo restores them.
survey-failed = Transformation failed: { $error }
survey-stale = Transformation discarded because the active project or source data changed. Select the source data and try again.
survey-coordinates-menu = Coordinates
survey-definitions-action = Definitions…
survey-transform-action = Transform…
survey-definitions-title = Coordinate Definitions
survey-transform-title = Transform Coordinates
survey-new-system = New Coordinate System
survey-new-system-name = Coordinate system
survey-set-local = Set as Mine Coordinate System
survey-delete-system = Delete Coordinate System
survey-systems-empty = No coordinate systems
survey-system-name = Name
survey-system-origin = Same point — system coordinates
survey-angle-help = Counterclockwise from reference X toward reference Y, viewed from above.
survey-scale-help = Uniform XYZ scale from the reference frame to this system. Use 1 to preserve dimensions.
survey-close = Close
survey-from = From
survey-to = To
survey-transform-button = Transform
survey-swap = Swap
survey-drape-note = Draped imagery is dropped from converted surfaces and must be re-draped.
survey-needs-grid-block-model = A block model is a regular grid of cells, and a change of projection or reference frame does not keep it regular. Converting it would mean resampling every cell into a new grid and losing the values it carries, so it was left alone.
survey-needs-grid-raster = A raster is placed by an affine map onto the world, which a change of projection or reference frame cannot preserve. Converting it would mean resampling the image, so it was left alone.
survey-conversion-exact = Exact: grid change only, no reprojection.
survey-conversion-accuracy = Stated accuracy { $accuracy } m.
survey-kind = Kind
survey-axis-names = Axis names
survey-kind-registry-short = Registry system
survey-kind-grid-short = Grid over another system
survey-registry-search = Search
survey-registry-hint = Name or EPSG code, e.g. "mga zone 56"
survey-registry-none = Nothing in the registry matches every word.
survey-parent = Defined against
survey-parent-origin = Known point — parent coordinates
survey-pick-registry = Search for the system and choose it from the results.
survey-pick-parent = Choose the system this grid is defined against.
survey-pick-system = Choose a system
survey-pick-systems = Choose the system to convert from and the one to convert to.
survey-no-selection = Choose a coordinate system on the left, or right-click to add one.
survey-kind-grid = Grid over { $parent }
survey-system-in-use = "{ $name }" cannot be deleted: { $dependants } { $dependants ->
    [one] is
   *[other] are
  } defined against it. Point them elsewhere first.
survey-system-cycle = "{ $name }" is defined against itself, directly or through its parents.
survey-system-missing = That coordinate system no longer exists. Select another definition.
survey-same-system = Choose different source and destination systems.
survey-name-exists = A coordinate system with that name already exists. Select it to edit, or choose another name.

preferences-ui-size = UI size
preferences-ui-size-help = Adjusts text and controls relative to your device’s normal display scaling. 100% uses the default size. Screen resolution and window size do not shrink the interface.

relimit-select-boundary = Select polyline or circle to relimit to

relimit-click-boundary = Click the polyline or circle to intersect with…

relimit-mode-help = Intersect moves one endpoint to a polyline or circle. Absolute sets the final line length. Relative adds or subtracts length.

browser-graphics-device-lost = The browser lost its graphics device. Reopen this page in a new tab. GPU details: { $message }

# Join Point Clouds dialog

# Borehole log sideways scale

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins, and Incline Design contributors. Permission is hereby granted, free of charge, to any person obtaining a copy of this software to deal in it without restriction, subject to the conditions of the MIT License.

    Incline Design is provided "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, including but not limited to the warranties of MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE and NONINFRINGEMENT.
about-free-open-source-mine-design = Free Open Source Mine Design
about-licensed-under-mit-license = Licensed under the MIT License

## App strings

app-activated-browser-project-name = Activated browser project '{ $name }'.
app-browser-project-delete-failed = Browser project deletion failed: { $error }
app-browser-project-no-longer-exists = That browser project no longer exists
app-browser-save-failed-error = Browser save failed: { $error }
app-could-not-activate-browser-project = Could not activate browser project: { $error }
app-could-not-delete-browser-project = Could not delete browser project: { $error }
app-could-not-load-browser-project = Could not load the browser project: { $error }
app-could-not-restore-browser-project = Could not restore the browser project: { $error }
app-deleted-browser-project = Deleted browser project
app-failed-create-window-error = Failed to create window: { $error }
app-failed-create-window-icon-error = Failed to create window icon: { $error }
app-failed-detach-top-down-preview = Failed to detach top-down preview: { $error }
app-failed-initialize-graphics-error = Failed to initialize graphics: { $error }
app-browser-preferences-load-failed = Failed to load browser preferences: { $error }
app-failed-load-config-file-error = Failed to load config file: { $error }
app-failed-load-session-file-error = Failed to load session file: { $error }
app-failed-rasterize-window-icon-error = Failed to rasterize window icon: { $error }
app-failed-save-browser-session-error = Failed to save browser session: { $error }
app-failed-save-session-error = Failed to save session: { $error }
app-saved-name-browser-storage = Saved '{ $name }' to browser storage

## Block strings

block-model-between = Between
block-model-block-grid = Block grid
block-model-block-size = Block size
block-model-choose-numeric-variable = Choose a numeric variable
block-model-choose-numeric-variables = Choose numeric variables
block-model-count-variables-selected = { $count } variables selected
block-model-estimate-variables = Estimate variables
block-model-full-x-y-z-dimensions = Full X, Y and Z dimensions of each block. Smaller blocks increase detail, computation time and memory use.
block-model-grid-bounds-block-sizes-invalid = Grid bounds or block sizes are invalid.
block-model-lower-x-y-z-edges = Lower X, Y and Z edges of the block-model volume. Block centres begin half a block inside these limits.
block-model-maximum = Maximum
block-model-maximum-nearest-samples-used-each = Maximum nearest samples used for each block. Lower values run faster; higher values can smooth estimates and increase computation time.
block-model-maximum-samples = Maximum samples
block-model-minimum = Minimum
block-model-min-samples-help = Minimum nearby samples required to estimate a block. Blocks with fewer samples inside the search radius are left empty.
block-model-minimum-samples = Minimum samples
block-model-no-block-model-selected = No block model selected
block-model-no-drill-holes-selected = No drill holes selected
block-model-nugget = Nugget
block-model-numeric-interval-fields-interpolate = Numeric interval fields to interpolate. Each selected field becomes one block-model variable.
block-model-kriging-help = Ordinary Kriging estimates numeric drill-hole intervals at each block centre using a spherical variogram.
block-model-partial-sill = Partial sill
block-model-range-search-radius = Range / search radius
block-model-range-help = Samples farther than this distance are excluded; covariance reaches zero at this range.
block-model-select-all = Select all
block-model-selected-block-model-whose-blocks = The selected block model, whose blocks are thresholded into a solid. Close the dialog to threshold a different one.
block-model-source-drill-holes-help = The selected Drill Holes collection, whose numeric intervals are estimated into blocks. Close the dialog to estimate from a different one.
block-model-sill-help = Spatially correlated variance contributed by the spherical model. Together with the nugget, it sets covariance at zero distance.
block-model-spherical-variogram-search = Spherical variogram and search
block-model-threshold-at-most = <= threshold
block-model-threshold-at-least = >= threshold
block-model-threshold-min = Threshold / min
block-model-upper-x-y-z-extent = Upper X, Y and Z extent to cover. The last block may extend past this extent when the span is not an exact multiple of block size.
block-model-variable = Variable
block-model-variance-effectively-zero-separation = Variance at effectively zero separation caused by measurement error or variation below the sampling scale. Use zero when no nugget effect is intended.
block-model-volume-feedback-disconnected = Block-volume usage feedback readback disconnected
block-model-volume-feedback-failed = Block-volume usage feedback readback failed: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Borehole strings

borehole-inspector-checking-linked-geophysics-file = Checking the linked geophysics file...
borehole-inspector-close-inspector = Close the inspector
borehole-inspector-data = Data
borehole-inspector-display = Display
borehole-inspector-file-not-where-was-linked = { $file } is not where it was linked from.
borehole-inspector-guessed-name = Guessed by name
borehole-inspector-hold-hole-while-you-work = Hold this hole while you work on the ones around it.
borehole-inspector-holding-hole-click-follow-selection = Holding this hole. Click to follow the selection again.
borehole-inspector-log = Log
borehole-inspector-no-hole-inspected = No hole inspected
borehole-inspector-pick-file = Pick { $file }...
borehole-inspector-pick-file-again-show-its = Pick { $file } again to show its geophysics: a browser page cannot reopen a file by itself.
borehole-inspector-reading-geophysics-file-its-index = Reading the geophysics file for its index; the status bar shows progress.
borehole-inspector-strat = Strat
borehole-inspector-summary = Summary

## Canvas strings

canvas-circle-summary = Circle | Layer: { $layer } | radius { $radius }
canvas-not-selectable-closed-polyline = Not selectable | Choose a closed polyline
canvas-polyline-summary = Polyline | Layer: { $layer } | { $count } vertices
canvas-surface-name = Surface | { $name }
canvas-trimmed = Trimmed

## Cinematic strings

cinematic-shadows-method = Cinematic view shadows: { $method }

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = Created batter berm from object { $object_id }
cmd-bezier-replaced-polyline-span-first-last = Replaced polyline span { $first }→{ $last } with { $count } sampled intermediate points
cmd-bezier-vertices-first-last = Vertices { $first } to { $last }
cmd-block-model-block-model-loader-disconnected-path = Block model loader disconnected for { $path }
cmd-block-model-block-model-path-has-count = Block model { $path } has { $count } variable(s) of an unsupported type that won't be readable: { $names }
cmd-block-model-building-ore-mesh = Building ore mesh…
cmd-block-model-could-not-create-block-model = Could not create block model: { $error }
cmd-block-model-could-not-decode-block-model = Could not decode block-model colour variable '{ $variable }': { $error }
cmd-block-model-created-block-model-name-ordinary = Created block model '{ $name }' by Ordinary Kriging
cmd-block-model-failed-load-block-model-error = Failed to load block model: { $error }
cmd-block-model-generated-ore-mesh-from-block = Generated ore mesh from block model '{ $name }'
cmd-block-model-imported-block-model-source-path = Imported block model source { $path }
cmd-block-model-loaded-block-model-name-blocks = Loaded block model '{ $name }': { $blocks } blocks ({ $renderable } renderable), grid { $dimx }x{ $dimy }x{ $dimz }, { $variables } variables
cmd-block-model-loading-name = Loading { $name }
cmd-block-model-loading-name-ellipsis = Loading { $name }…
cmd-chamfer-applied = Chamfered corner { $corner } with radius { $radius } and { $segments } segments
cmd-chamfer-radius = Radius { $radius }
cmd-commands-clipped = Clipped
cmd-commands-command-failed-error = Command failed: { $error }
cmd-commands-count-control-string-s = { $count } control string(s)
cmd-commands-count-control-string-s-layer = { $count } control string(s) on '{ $layer }'
cmd-commands-count-point-s-across-layers = { $count } point(s) across { $layers } layers
cmd-commands-count-point-s-layer = { $count } point(s) on '{ $layer }'
cmd-commands-kind-layer = { $kind } on '{ $layer }'
cmd-commands-no-control-strings = No control strings
cmd-commands-no-extent = No extent
cmd-commands-select-holes-place-reference-points = Select the holes to place reference points on
cmd-triangulate-needs-selection = Select the objects to triangulate before running Create Triangulation
cmd-commands-select-one-loaded-block-model = Select one loaded block model before creating an ore triangulation from it
cmd-commands-select-one-loaded-drill-hole = Select one loaded drill hole collection before creating a block model from it
cmd-commands-select-one-loaded-point-cloud = Select one loaded point cloud before creating a triangulation from it
cmd-contours-needs-triangulation = Select one loaded triangulation before generating contours from it
cmd-slice-needs-triangulation = Select one loaded triangulation before slicing it by Z range
cmd-commands-select-one-loaded-triangulation-one = Select one loaded triangulation and one closed polyline before clipping
cmd-commands-select-one-more-objects-before = Select one or more objects before setting { $axis }
cmd-commands-sliced = Sliced
cmd-contours-contour-generation-failed-error = Contour generation failed: { $error }
cmd-contours-discarded-layer-exists = Contours for '{ $name }' were discarded: layer '{ $layer_name }' now exists
cmd-contours-discarded-project-closed = Contours for '{ $name }' were discarded: the project was closed
cmd-contours-discarded-layer-deleted = Contours for '{ $name }' were discarded: the selected output layer was deleted
cmd-contours-generated = Generated { $line_count } contour polyline(s) for triangulation '{ $name }' in layer '{ $layer_name }'
cmd-creation-assembled-boundary-rings = Assembled { $assembled_count } closed boundary ring(s) from fragmented open strings
cmd-creation-created-triangulation-from-boundary = Created triangulation from { $boundary_count } boundary ring(s) and { $constraint_count } open constraint(s), surface type { $surface_type }
cmd-creation-creating-triangulation = Creating triangulation…
cmd-creation-generate-upper-surface-ignored-count = Generate upper surface: ignored { $count } lower conflicting breakline segment(s); source objects are unchanged
cmd-creation-ignored-objects = Ignored { $rejected } non-polyline or degenerate object(s) during triangulation
cmd-creation-weld-retry-moved-coarse-welded = Weld & retry: moved { $coarse_welded } vertex/vertices onto shared positions (up to { $coarse_weld_tol } m); source objects are unchanged
cmd-creation-welded-breakline-vertices = Welded { $welded } breakline vertex/vertices that coincided within tolerance
cmd-cuts-clipped-surface-name-polyline-mode = Clipped surface '{ $name }' by polyline ({ $mode })
cmd-cuts-clipping-surface-polyline = Clipping surface by polyline…
cmd-cuts-cut-topology-name-pit-shell = Cut topology '{ $name }' to pit shell
cmd-cuts-cut-triangulation-name-z-band = Cut triangulation '{ $name }' by Z band [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = Cutting topology by pit shell…
cmd-cuts-cutting-triangulation-z = Cutting triangulation by Z…
cmd-cuts-ignored-vertical-faces = Ignored { $count } vertical or degenerate reference topology face(s) with no XY area
cmd-cuts-site-skipped-constraint-from-x = { $site }: skipped constraint ({ $from_x }, { $from_y }) -> ({ $to_x }, { $to_y }) the triangulator could not split
cmd-cuts-skipped-degenerate-edges = { $site }: skipped { $skipped } near-degenerate constraint edge(s); the cut boundary may be off by a hairline near them
cmd-cuts-trimmed-surface = Trimmed surface '{ $surface }' to topology '{ $topology }' ({ $mode })
cmd-cuts-trimming-surface-topology = Trimming surface to topology…
cmd-drape-draped-intersected-vertices-changed = Draped { $intersected } vertices; { $changed } changed elevation
cmd-drape-no-intersections = None of the selected design vertices intersect the selected topologies
cmd-drape-objects-changed-object-s-changed = { $objects } changed object(s) · { $changed } of { $intersected } intersecting vertices moved
cmd-drape-select-one-more-design-objects = Select one or more design objects to drape
cmd-drape-select-one-more-topologies-drape = Select one or more topologies to drape onto
cmd-drape-selected-topologies-no-longer-loaded = The selected topologies are no longer loaded
cmd-drill-hole-choose-drillhole-source-files-again = Choose the drillhole source files again
cmd-drill-hole-drill-pattern-too-large-contains = The drill pattern is too large or contains invalid collar coordinates
cmd-drill-hole-drillhole-field-label-has-count = Drillhole field '{ $label }' has { $count } distinct codes, more than a coded field would typically have; it looks like free text rather than a categorical field, but every code is kept and coloured
cmd-drill-hole-enter-name-drill-pattern = Enter a name for the drill pattern
cmd-drill-hole-failed-load-drillholes-error = Failed to load drillholes: { $error }
cmd-drill-hole-depth-must-be-positive = Hole depth must be greater than zero
cmd-drill-hole-diameter-must-be-positive = Hole diameter must be greater than zero
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Loaded drillhole dataset '{ $name }': { $holes } holes, { $fields } colour fields
cmd-drill-hole-name-already-loading = '{ $name }' is already loading
cmd-drill-hole-name-reason = '{ $name }': { $reason }
cmd-drill-hole-name-side = { $name } { $side }
cmd-drill-hole-name-working-section-side = { $name } working section { $side }
cmd-drill-hole-no-hole-holds-value-field = No hole holds '{ $value }' in that field
cmd-drill-hole-only-mapped-csv-bundles-imported = Only mapped CSV bundles are imported in the browser
cmd-drill-hole-pattern-contains-no-holes = The pattern contains no holes
cmd-drill-hole-reading-name = Reading { $name }
cmd-drill-hole-reference-points-used-holes-placed = Reference points: { $used } holes placed, { $absent } without '{ $value }', { $flagged } flagged as possible fault repeats
cmd-drill-hole-uppermost-run-used-flagged-holes = Uppermost run used, flagged: { $holes }
cmd-drill-hole-working-section-name-not-same = Working section '{ $name }' is not the same in every selected dataset; each dataset's own was used.
cmd-drill-hole-working-sections-not-kept-dataset = Working sections not kept in '{ $dataset }'. { $reasons }
cmd-explode-count-line-s = { $count } line(s)
cmd-explode-polyline = Explode Polyline
cmd-explode-exploded-polyline-into-count-line = Exploded polyline into { $count } line segments
cmd-file-block-model-csv-encoding-failed = Block-model CSV encoding failed: { $error }
cmd-file-block-model-csv-export-failed = Block-model CSV export failed: { $error }
cmd-file-browser-recovery-unavailable = Browser recovery files are unavailable; saved projects remain in IndexedDB
cmd-file-closed-project-runtime-id-runtime = Closed project runtime id { $runtime_id }
cmd-file-could-not-create-new-project = Could not create a new project: { $error }
cmd-file-could-not-finish-pending-project = Could not finish the pending project action: { $error }
cmd-file-could-not-finish-saving-before = Could not finish saving before exit: { $error }
cmd-file-could-not-open-browser-project = Could not open browser project: { $error }
cmd-file-could-not-open-path-error = Could not open { $path }: { $error }
cmd-file-could-not-read-selected-file = Could not read selected file: { $error }
cmd-file-could-not-reload-layer-from = Could not reload layer from disk: { $error }
cmd-file-could-not-reload-project-from = Could not reload project from disk: { $error }
cmd-file-could-not-remove-browser-project = Could not remove browser project: { $error }
cmd-file-could-not-restore-layer-from = Could not restore layer from project: { $error }
cmd-file-could-not-snapshot-dirty-project = Could not snapshot the dirty project for recovery: { $error }
cmd-file-could-not-start-browser-export = Could not start browser export: { $error }
cmd-file-could-not-write-recovery-copies = Could not write recovery copies: { $error }
cmd-file-created-new-browser-project = Created new browser project
cmd-file-created-new-project = Created new project
cmd-file-description-download-failed-error = { $description } download failed: { $error }
cmd-file-discard-cancelled-project-changed = Discard was cancelled because the project changed while the OMF was reloading
cmd-file-discarded-changes-layer-target-name = Discarded changes to layer '{ $target_name }'
cmd-file-discarded-changes-reloaded-path = Discarded changes: reloaded { $path }
cmd-file-downhole-geophysics-csv = Downhole geophysics CSV
cmd-file-downloaded-description-file-name = Downloaded { $description }: { $file_name }
cmd-file-drillhole-csv-export-failed-error = Drillhole CSV export failed: { $error }
cmd-file-dxf-download-encoding-failed-error = DXF download encoding failed: { $error }
cmd-file-dxf-import-failed-error = DXF import failed: { $error }
cmd-file-encoding-block-model-csv-download = Encoding block-model CSV download…
cmd-file-encoding-dxf-download = Encoding DXF download…
cmd-file-encoding-triangulation-download = Encoding triangulation download…
cmd-file-exit-deferred-exports = Exit deferred until background exports finish
cmd-file-exit-requested-no-unsaved-changes = Exit requested with no unsaved changes
cmd-file-exported-block-model-csv-path = Exported block-model CSV to { $path }
cmd-file-exported-description-dxf-path = Exported { $description } to DXF: { $path }
cmd-file-exported-three-drillhole-csvs-path = Exported three drillhole CSVs to { $path }
cmd-file-exported-triangulation-name-path = Exported triangulation '{ $name }' to { $path }
cmd-file-exporting-name = Exporting { $name }…
cmd-file-exporting-triangulation-name-path = Exporting triangulation '{ $name }' to { $path }
cmd-file-fatal-renderer-failure-reason = Fatal renderer failure: { $reason }
cmd-file-dialog-action-failed = File dialog action failed: { $msg }
cmd-file-imported-added-object-s-from = Imported { $added } object(s) from { $name }
cmd-file-imported-total-dxf-object-s = Imported { $total } DXF object(s)
cmd-file-layer-discard-was-cancelled-because = Layer discard was cancelled because the project changed while the project was reloading
cmd-file-no-recovery-directory = No recovery directory available: { $error }
cmd-file-no-unsaved-project-content-nothing = No unsaved project content; nothing to recover
cmd-file-parsing-browser-dxf-import = Parsing browser DXF import…
cmd-file-parsing-dxf-import = Parsing DXF import…
cmd-file-project-closes-after-save = Project will close after its current save finishes
cmd-file-the-project-closes-after-save = The project will close after its current save finishes
cmd-file-queued-count-triangulation-file-s = Queued { $count } triangulation file(s) for import
cmd-file-recovery-copies-path-reopen-them = Recovery copies are in { $path }; reopen them after restarting
cmd-file-recovery-copy-failed-error = Recovery copy failed: { $error }
cmd-file-recovery-copy-failed-failure = Recovery copy failed: { $failure }
cmd-file-recovery-copy-written-path = Recovery copy written: { $path }
cmd-file-reverting-layer = Reverting layer…
cmd-file-reverting-project = Reverting project…
cmd-file-save-failed-message = Save failed: { $message }
cmd-file-save-project-already-running-save = A save of this project is already running; save again when it finishes
cmd-file-save-worker-ended-without-result = Save worker ended without a result
cmd-file-saved-project-as = Saved project as: { $path }
cmd-file-saved-project = Saved project: { $path }
cmd-file-saving-browser-storage = Saving to browser storage…
cmd-file-selected-block-model-no-longer = The selected block model is no longer loaded
cmd-file-selected-drillhole-dataset-no-longer = The selected drillhole dataset is no longer loaded
cmd-file-switching-project = Switching project…
cmd-file-triangulation-download-encoding-failed = Triangulation download encoding failed: { $error }
cmd-file-user-chose-exit-without-saving = User chose to exit without saving
cmd-file-user-requested-exit-project-export = User requested exit (project export or unsaved-work confirmation required)
cmd-file-viewport = Viewport
cmd-file-wait-current-project-save-finish = Wait for the current project save to finish
cmd-file-wait-current-project-switch-finish = Wait for the current project switch to finish
cmd-file-wait-project-operation-finish-before = Wait for the project operation to finish before discarding changes
cmd-file-wait-project-revert-finish-before = Wait for the project revert to finish before saving
cmd-folder-collection-named-name-already-exists = A collection named '{ $name }' already exists
cmd-folder-collection-no-longer-exists = That collection no longer exists
cmd-folder-created-collection-name = Created collection '{ $name }'
cmd-folder-deleted-collection-name = Deleted collection '{ $name }'
cmd-folder-moved-item-into-collection-name = Moved item into collection '{ $name }'
cmd-folder-moved-item-root-section = Moved item to the root of { $section }
cmd-folder-renamed-collection-before-after = Renamed collection '{ $before }' to '{ $after }'
cmd-folder-section-cannot-hold-item = That section cannot hold this item
cmd-fuse-closed-polyline = Closed polyline
cmd-fuse-count-source-line-s = { $count } source line(s)
cmd-fuse-created-shape-object-id-vertices = Created { $shape } { $object_id } with { $vertices } vertices from { $sources } source line(s)
cmd-fuse-click-missed = Fuse: click did not hit any object (nothing under cursor)
cmd-fuse-click-not-near-endpoint = Fuse: click was not close enough to either endpoint of the selected line
cmd-fuse-clicked-closed-polyline = Fuse: clicked object { $object_id } is a closed polyline, fuse only works on open polylines
cmd-fuse-clicked-not-open-polyline = Fuse: clicked object { $object_id } is not an open polyline (it's a { $kind })
cmd-fuse-clicked-object-missing = Fuse: clicked object { $object_id } no longer exists
cmd-fuse-clicked-too-few-vertices = Fuse: clicked polyline { $object_id } has only { $count } vertex/vertices, need at least 2
cmd-fuse-endpoint-marker-missing = Fuse: endpoint marker { $marker_index } no longer exists
cmd-fuse-close-needs-three-vertices = Fuse: line needs at least 3 distinct vertices to close into a polyline (has { $count })
cmd-fuse-lines = Fuse Lines
cmd-fuse-needs-two-segments = Fuse: need at least 2 segments to commit (have { $count })
cmd-fuse-no-active-layer = Fuse: no active layer to place the fused line on
cmd-fuse-no-active-project = Fuse: no active project, cannot commit
cmd-fuse-no-source-line = Fuse: no source line to close into a polyline
cmd-fuse-awaiting-object-invalid = Fuse: object { $awaiting_id } is no longer a valid polyline
cmd-fuse-object-already-in-chain = Fuse: object { $object_id } is already part of the fuse chain, click a different line
cmd-fuse-result-too-few-vertices = Fuse: result has too few vertices ({ $count }), aborting
cmd-fuse-segment-object-invalid = Fuse: segment object { $object_id } is no longer a valid polyline, aborting
cmd-fuse-source-object-invalid = Fuse: source object { $object_id } is no longer a valid open polyline
cmd-fuse-source-object-missing = Fuse: source object { $object_id } no longer exists
cmd-fuse-open-polyline = Open polyline
cmd-include-failed = Include failed: { $message }
cmd-include-included-solid-shape-name-topology = Included solid '{ $shape_name }' in topology '{ $topology_name }' (retained { $retained } topology faces, skipped { $skipped } closure-cap faces)
cmd-include-including-pit-stockpile-solid = Including pit/stockpile solid…
cmd-insert-point-count-operation-point-s = { $count } { $operation } point(s)
cmd-insert-point-elevation-must-be-finite = Insert Point at Elevation requires a finite elevation
cmd-insert-point-insert-points = Insert Points
cmd-insert-point-inserted-count-operation-point-s = Inserted { $count } { $operation } point(s)
cmd-insert-point-intersection = Intersection
cmd-insert-point-no-new-operation-points-were = No new { $operation } points were found
cmd-insert-point-select-least-two-polylines-before = Select at least two polylines before inserting intersection points
cmd-insert-point-select-one-more-polylines-before = Select one or more polylines before inserting a point at elevation
cmd-layer-created-layer-name = Created layer '{ $name }'
cmd-layer-deleted-with-objects = Deleted layer { $layer_id } (and all objects on it)
cmd-layer-duplicated-layer-duplicate-name = Duplicated layer '{ $duplicate_name }'
cmd-layer-locked = Locked
cmd-layer-name-copy = { $name } copy
cmd-layer-selected-count-object-s-layer = Selected { $count } object(s) in layer { $layer_id }
cmd-layer-state-layer-name = { $state } layer '{ $name }'
cmd-layer-unlocked = Unlocked
cmd-move-tool-moved-collars = Applied move delta ({ $delta }) to { $count } drillhole collar(s)
cmd-move-tool-moved-objects = Applied move delta ({ $delta }) to { $count } object(s)
cmd-move-tool-count-hole-s = { $count } hole(s)
cmd-object-edit-edited-kind = Edited { $kind }
cmd-object-edit-edited-kind-count-vertices = Edited { $kind } ({ $count } vertices)
cmd-object-edit-no-changes-apply = No changes to apply
cmd-object-edit-object-changed-since-editor-opened = This object changed since the editor opened; reopen it to edit the current version
cmd-object-edit-target-changed = Object edit target changed; discarding the edit
cmd-object-edit-object-no-longer-exists-document = That object no longer exists in the document
cmd-object-edit-select-single-design-object-edit = Select a single design object to edit
cmd-object-edit-unassigned = Unassigned
cmd-offset-create-offset = Create Offset
cmd-offset-created-offset-count-object-s = Created offset of { $count } object(s)
cmd-offset-distance-must-be-positive = Offset distance must be greater than zero
cmd-offset-skipped-count-circle-s-offset = Skipped { $count } circle(s): the offset distance is larger than the radius
cmd-omf-could-not-open-project-source = Could not open project { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Create or open a project before merging data
cmd-omf-dataset-name-count-working-section = Dataset '{ $name }': { $count } working section(s) could not be restored: { $details }
cmd-omf-field-codes-partly-coloured = Dataset '{ $name }': field '{ $field }' was saved with { $saved } of { $total } codes coloured; the rest were given generated colours.
cmd-omf-encoding-project = Encoding project…
cmd-omf-exported-project-path = Exported project to { $path }
cmd-omf-imported-project = Imported project '{ $project_name }' from { $source_name }: { $count } top-level dataset(s)
cmd-omf-importing-project = Importing project…
cmd-omf-export-failed = OMF export failed: { $error }
cmd-omf-import-failed = OMF import failed: { $error }
cmd-omf-opened-project = Opened project '{ $project_name }' from { $source_name }
cmd-omf-project-source-name-contains-no = Project '{ $source_name }' contains no supported data elements
cmd-omf-source-name-applied-project-origin = { $source_name }: applied project origin { $origin } before merge
cmd-omf-crs-differs = { $source_name }: coordinate reference system '{ $source_crs }' differs from project CRS '{ $target_crs }'; coordinates were merged without reprojection
cmd-omf-source-name-units-source-units = { $source_name }: units '{ $source_units }' differ from project units '{ $target_units }'; coordinates were merged without conversion
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = There is no open Incline Design data to export
cmd-placement-2-vertices = 2 vertices
cmd-placement-count-vertices = { $count } vertices
cmd-placement-created-circle = Created circle with radius { $radius } m
cmd-placement-created-closed-polyline = Created closed polyline with { $count } vertices
cmd-placement-created-line-segment-2-vertices = Created line segment with 2 vertices
cmd-placement-created-open-polyline-count-vertices = Created open polyline with { $count } vertices
cmd-placement-placed-point-x-y-z = Placed point at { $x }, { $y }, { $z }
cmd-placement-radius = Radius { $radius } m
cmd-plot-composing-engineering-drawing = Composing engineering drawing…
cmd-plot-could-not-write-engineering-drawing = Could not write the engineering drawing: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Drawing scale fitted to visible data: 1:{ $scale }
cmd-plot = Plot
cmd-plot-saved-drawing = Saved engineering drawing: { $description } ({ $width } × { $height } px at { $dpi } dpi)
cmd-point-cloud-classified = Classified { $name }: { $ground } ground, { $vegetation } vegetation and { $noise } noise of { $count } points
cmd-point-cloud-classifying-point-clouds = Classifying point clouds
cmd-point-cloud-join-dropped-classifications = Dropped point classifications: some of the joined clouds are unclassified, and a partly classified cloud cannot be filtered to ground.
cmd-point-cloud-failed-classify-point-clouds-error = Failed to classify point clouds: { $error }
cmd-point-cloud-failed-join-point-clouds-error = Failed to join point clouds: { $error }
cmd-point-cloud-failed-load-point-cloud-error = Failed to load point cloud: { $error }
cmd-point-cloud-joined-count-clouds-into-name = Joined { $count } clouds into { $name } ({ $points } points)
cmd-point-cloud-joining-name = Joining { $name }
cmd-point-cloud-loaded-point-cloud-name-count = Loaded point cloud { $name } ({ $count } points)
cmd-point-cloud-point-cloud-classification-discarded = Point cloud classification discarded: a cloud changed while it ran. Run it again.
cmd-point-cloud-point-cloud-loader-disconnected-path = Point-cloud loader disconnected for { $path }
cmd-point-cloud-select-one-more-loaded-point = Select one or more loaded point clouds before classifying them
cmd-point-cloud-select-two-more-loaded-point = Select two or more loaded point clouds before joining them
cmd-point-cloud-tin-max-edge-disabled = (max edge disabled)
cmd-point-cloud-tin-max-edge-max-edge = (max edge { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = Point cloud TIN failed: { $error }
cmd-point-cloud-tin-filtered-ground = Terrain TIN: filtered to { $ground } ground points of { $total }
cmd-point-cloud-tin-subsampled = Terrain TIN: spatially subsampled { $sampled } of { $total } points
cmd-point-cloud-tin-triangulated = Terrain TIN: triangulated { $vertex_count } unique XY points into { $face_count } faces{ $suffix }
cmd-products-added-product-delay-ms-ms = Added product { $delay_ms } ms { $name }
cmd-products-deleted-product-delay-ms-ms = Deleted product { $delay_ms } ms { $name }
cmd-products-failed-save-products-error = Failed to save products: { $error }
cmd-products-product-no-longer-palette = That product is no longer in the palette
cmd-property-action-count-object-s-layer = { $action } { $count } object(s) to layer { $layer }
cmd-property-batch-set-axis-value-count = Batch-set { $axis } value on { $count } object(s)
cmd-property-batch-set-closed-count-polyline = Batch-set closed on { $count } polyline(s)
cmd-property-batch-set-color-count-object = Batch-set color on { $count } object(s)
cmd-property-batch-set-fill-style-count = Batch-set fill style on { $count } object(s)
cmd-property-batch-set-line-weight-count = Batch-set line weight on { $count } polyline(s)
cmd-property-copied = Copied
cmd-property-moved = Moved
cmd-raster-draped = Draped raster { $raster } over triangulation { $triangulation } (overlapping extents)
cmd-raster-failed-load-raster-name-error = Failed to load raster { $name }: { $error }
cmd-raster-failed-load-raster-path-error = Failed to load raster { $path }: { $error }
cmd-raster-loaded-raster-name-via-driver = Loaded raster { $name } via { $driver } ({ $srcx }x{ $srcy }, preview { $prevx }x{ $prevy })
cmd-raster-no-overlapping-triangulation = No loaded triangulation overlaps the extents of { $name }
cmd-raster-loader-disconnected = Raster loader disconnected for { $path }
cmd-raster-undraped = Undraped rasters from { $count } triangulation(s)
cmd-reference-surface-build-surface-failed-error = Build Surface failed: { $error }
cmd-reference-surface-building-surface = Building surface…
cmd-reference-surface-built-surface-name-from-vertex = Built surface { $name } from { $vertex_count } point(s) into { $face_count } face(s), box z { $low } to { $high }{ $support }{ $coincident }{ $controls }
cmd-reference-surface-control-string-index-could-not = Control string { $index } could not be added to the mesh
cmd-reference-surface-control-string-index-crosses-itself = Control string { $index } crosses itself in plan
cmd-reference-surface-control-string-index-doubles-back = Control string { $index } doubles back on itself in plan
cmd-reference-surface-control-string-index-ends-where = Control string { $index } ends where it starts; close it to use it as a mask
cmd-reference-surface-control-string-index-has-count = Control string { $index } has { $count } distinct vertex(es); a control needs at least { $minimum }
cmd-reference-surface-control-string-index-has-non = Control string { $index } has non-finite coordinates
cmd-reference-surface-control-string-index-no-longer = Control string { $index } is no longer available
cmd-reference-surface-control-string-index-runs-along = Control string { $index } runs along the extent string in plan; that is not supported yet
cmd-reference-surface-control-string-overrides-pick-x = Control string overrides the pick at ({ $x }, { $y }): pick { $pick } m, control { $control } m, difference { $difference } m
cmd-reference-surface-control-strings-b-disagree-x = Control strings { $a } and { $b } disagree at ({ $x }, { $y }): { $za } m against { $zb } m, { $difference } m apart
cmd-reference-surface-control-strings-b-run-along = Control strings { $a } and { $b } run along each other in plan; that is not supported yet
cmd-reference-surface-count-control-string-s-vertices = ; { $count } control string(s) with { $vertices } vertex(es){ $crossings }
cmd-reference-surface-count-point-s-inside-extent = { $count } point(s) inside the extent; a surface needs at least { $minimum }
cmd-reference-surface-count-point-s-outside-extent = ; { $count } point(s) outside the extent shaped it as support
cmd-reference-surface-count-point-s-selected-surface = { $count } point(s) selected; a surface needs at least { $minimum }
cmd-reference-surface-count-point-s-shared-plan = ; { $count } point(s) shared a plan position and were kept once
cmd-reference-surface-delaunay-insert-failed-error = Delaunay insert failed: { $error }
cmd-reference-surface-extent-must-closed-string = The extent must be a closed string
cmd-reference-surface-extent-string-crosses-itself-plan = The extent string crosses itself in plan
cmd-reference-surface-extent-string-has-non-finite = The extent string has non-finite coordinates
cmd-reference-surface-extent-string-needs-least-three = The extent string needs at least three distinct vertices
cmd-reference-surface-extent-string-no-longer-available = The extent string is no longer available
cmd-reference-surface-meeting-count-crossing-s = meeting at { $count } crossing(s)
cmd-reference-surface-and-more = , … and { $more } more
cmd-reference-surface-no-mask-selected-surface-unclipped = No mask selected; the surface is unclipped
cmd-reference-surface-no-part-surface-falls-inside = No part of the surface falls inside the extent
cmd-reference-surface-open-project-before-building-surface = Open a project before building a surface
cmd-reference-surface-points-collinear-plan-surface-needs = The points are collinear in plan; a surface needs three that are not
cmd-reference-surface-select-exactly-one-closed-string = Select exactly one closed string to clip the surface to
cmd-reference-surface-selected-point-has-non-finite = A selected point has non-finite coordinates
cmd-reference-surface-selected-points-span-count-layers = The selected points span { $count } layers; the surface is placed under { $section }
cmd-relimit-click-missed = Relimit: click did not hit any object (nothing under cursor)
cmd-relimit-click-ignored = Relimit: click ignored, tool is not currently waiting for a target pick
cmd-relimit-clicked-source-line = Relimit: clicked the source line itself, pick a different line
cmd-relimit-no-source-line = Relimit: no source line is set, aborting pick
cmd-relimit-relimited-line-source-id-selected = Relimited line { $source_id } to the selected target
cmd-relimit-resized-line-source-id-using = Resized line { $source_id } using { $mode } value { $value }
cmd-rename-item-no-longer-belongs-active = That item no longer belongs to the active project
cmd-rename-renamed-before-name = Renamed '{ $before }' to '{ $name }'
cmd-rename-renamed-name-taken = Renamed '{ $before }' to '{ $name }' ('{ $requested }' is already taken)
cmd-rotate-collar-turned-count-drillhole-collar-s = Turned { $count } drillhole collar(s) { $rotation }
cmd-section-verb-count-item-s-section = { $verb } { $count } item(s) in { $section }
cmd-selection-delete-vertex = Delete Vertex
cmd-selection-deleted-count-selected-object-s = Deleted { $count } selected object(s)
cmd-selection-deleted-vertex = Deleted vertex { $vertex } from polyline { $object_id }
cmd-selection-duplicate-selection = Duplicate Selection
cmd-selection-duplicated-count-object-s = Duplicated { $count } object(s)
cmd-session-created-triangulation = Created triangulation '{ $name }' ({ $vertex_count } vertices, { $face_count } faces) from surface type { $surface_type }
cmd-session-deleted-triangulation = Deleted triangulation '{ $name }' from project
cmd-session-failed-load-triangulation-error = Failed to load triangulation: { $error }
cmd-session-failed-load-triangulation-message = Failed to load triangulation: { $message }
cmd-session-loaded-triangulation = Loaded triangulation '{ $name }' ({ $path }, { $vertex_count } vertices, { $face_count } faces)
cmd-session-set-triangulation-tri-id-color = Set triangulation { $tri_id } color to { $color }
cmd-session-triangulation-load-no-result = Triangulation load for { $path } ended without a result
cmd-session-triangulation-failed = Triangulation operation failed: { $message }
cmd-session-unloaded-triangulation-name = Unloaded triangulation '{ $name }'
cmd-slice-entered-slice-view-cx-cy = Entered slice view @ { $cx }, { $cy }, { $cz } along { $dx }, { $dy } ({ $length }m line)
cmd-slice-exited-slice-view = Exited slice view
cmd-slice-reset-section-view-fit-extents = Reset the section view (fit to extents)
cmd-slice-set-section-grid-enabled = Set section grid = { $enabled }
cmd-split-created-2-open-polylines = Created 2 open polylines
cmd-split-line = Split Line
cmd-split-points-needs-interior-vertex = Split At Points: choose an interior vertex of the open line
cmd-split-points-needs-non-adjacent-vertices = Split At Points: choose two non-adjacent polyline vertices
cmd-split-polyline-into-two = Split source polyline into two open polylines
cmd-text-edit-finished = Finished text edit for object { $object_id }
cmd-text-updated = Updated text on object { $object_id }
cmd-view-centre-rotation-not-available-flying = The centre of rotation is not available in flying mode
cmd-view-fixed-centre-rotation-x-y = Fixed the centre of rotation at { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = No point under the cursor to fix the centre of rotation on
cmd-view-released-centre-rotation = Released the centre of rotation
cmd-view-reset-view-fit-extents = Reset view (fit to extents)
cmd-view-set-cinematic-view-enabled = Set cinematic view = { $enabled }
cmd-view-set-topology-wireframes-enabled = Set topology wireframes = { $enabled }
cmd-view-set-view-points-enabled = Set view points = { $enabled }
cmd-view-set-xy-grid-enabled = Set XY grid = { $enabled }
cmd-view-zoom-extents-preserving-angle = Zoom to extents (preserving angle)

## Common strings

common-add-product = Add Product
common-appearance = Appearance...
common-background = Background
common-block-model = Block model
common-block-models = Block Models
common-borehole-inspector = Borehole Inspector
common-build-surface = Build Surface
common-build-surface-ellipsis = Build Surface...
common-cancelled = Cancelled
common-chamfer = Chamfer
common-choose = Choose...
common-circle = Circle
common-classify = Classify
common-classify-point-clouds = Classify Point Clouds
common-click-point-fix-centre-rotation = Click a point to fix the centre of rotation
common-clip-surface-polyline = Clip Surface by Polyline...
common-closed = Closed
common-collection = Collection
common-colour = Colour
common-confirm-omf-rewrite = Confirm OMF Rewrite
common-could-not-replace-current-project = Could not replace the current project: { $error }
common-count-object-s = { $count } object(s)
common-create = Create
common-create-batter-berm = Create Batter Berm
common-create-bezier-curve = Create Bezier Curve
common-create-block-model = Create Block Model
common-create-block-model-ellipsis = Create Block Model...
common-create-circle = Create Circle
common-create-drill-pattern = Create Drill Pattern
common-create-layer = Create Layer
common-create-line = Create Line
common-create-ore-triangulation = Create Ore Triangulation
common-create-ore-triangulation-ellipsis = Create Ore Triangulation...
common-create-point = Create Point
common-create-polyline = Create Polyline
common-create-triangulation = Create Triangulation...
common-crosses = Crosses
common-cut = Cut
common-cut-topology-pit-shell = Cut Topology with Pit Shell...
common-delete-collection = Delete Collection
common-delete-layer = Delete Layer
common-delete-product = Delete Product
common-delete-selection = Delete Selection
common-designs = Designs
common-discard-layer-changes = Discard Layer Changes
common-down = Down
common-drape-topology = Drape to Topology
common-easting = Easting
common-edit-object = Edit Object
common-edit-text = Edit Text
common-elevation = Elevation
common-exit-without-saving = Exit Without Saving
common-export-engineering-drawing = Export Engineering Drawing
common-file-was-left-out-downhole = { $file } was left out of the downhole geophysics: { $error }
common-filter = Filter
common-fly-mode = Fly Mode
common-generate-contour-lines = Generate Contour Lines...
common-hide-all = Hide All
common-hide-selection = Hide Selection
common-hole-id = Hole ID
common-ignore = Ignore
common-import-csv-block-model = Import CSV Block Model
common-import-dxf = Import DXF
common-incline-design-project = Incline Design project
common-join = Join...
common-join-point-clouds = Join Point Clouds
common-joined-cloud = Joined Cloud
common-layer = Layer
common-legend = Legend
common-line = Line
common-line-weight = Line weight
common-link-geophysics = Link Geophysics...
common-load-drillholes-before-linking-geophysics = Load the drillhole dataset before linking geophysics to it
common-lock-all = Lock All
common-lock-selection = Lock Selection
common-m = m
common-max = Max
common-merge-shell-into-topology = Merge Shell into Topology
common-merge-shell-into-topology-ellipsis = Merge Shell into Topology...
common-modelling = Modelling
common-move-collar = Move Collar
common-move-collection = Move to Collection
common-move-design = Move Design
common-move-selection = Move Selection
common-name-has-no-readable-size = { $name } has no readable size
common-new-product = New Product
common-no-block-models = No block models
common-no-design-layers = No design layers
common-no-drill-holes = No drill holes
common-no-file-chosen = No file chosen
common-no-open-project = No open project
common-no-point-clouds = No point clouds
common-no-triangulations = No triangulations
common-none = None
common-northing = Northing
common-offset = Offset
common-open = Open
common-orientation = Orientation
common-point = Point
common-point-cloud = Point cloud
common-point-clouds = Point Clouds
common-polyline = Polyline
common-polyline-layer = Polyline on '{ $layer }'
common-project = Project
common-rasters = Rasters
common-redo = Redo
common-reference-points = Reference Points...
common-relimit-line = Relimit Line
common-remove-project = Remove Project
common-reset-view = Reset View
common-reveal-all = Reveal All
common-reveal-finder = Reveal in Finder
common-rotate-collar = Rotate Collar
common-save-exit = Save and Exit
common-scale-bar = Scale bar
common-set-initiation-point = Set Initiation Point
common-shape = Shape
common-shell = With Shell
common-slashes = Slashes
common-slice = Slice
common-slice-triangulation-z-range = Slice Triangulation by Z Range...
common-surface-contours = Surface Contours
common-text = Text
common-degree-suffix = °
common-tie-holes = Tie Holes
common-triangulations = Triangulations
common-trim-topology = Trim to Topology...
common-undo = Undo
common-undrape-all = Undrape All
common-uniform-white = Uniform white
common-unknown = Unknown
common-unlock-all = Unlock All
common-untitled = Untitled
common-up = Up
common-vertical-exaggeration = Vertical Exaggeration
common-x = x
common-zoom-extents = Zoom to Extents

## Confirmations strings

confirmations-close-project-unsaved-changes = Close Project: Unsaved Changes
confirmations-close-without-saving = Close Without Saving
confirmations-delete = Delete
confirmations-delete-objects = Delete Objects
confirmations-discard = Discard
confirmations-discard-all-unsaved-changes-layer =
    Discard all unsaved changes to layer '{ $name }'?
    The saved layer is reloaded from disk while changes to other layers are kept. This cannot be undone.
confirmations-discard-all-unsaved-changes-name =
    Discard all unsaved changes to '{ $name }'?
    The last saved version is reloaded from disk. This cannot be undone.
confirmations-discard-changes = Discard Changes
confirmations-exit-unsaved-changes = Exit: Unsaved Changes
confirmations-incline-design-cannot-reproduce-all = Incline Design cannot reproduce all content from the original OMF. Saving will omit the following content:
confirmations-product = Product
confirmations-project = this project
confirmations-remove-name-delete-its-browser = Remove '{ $name }' and delete its browser-stored copy? Unsaved changes will be lost.
confirmations-remove-project-unsaved-changes = Remove Project: Unsaved Changes
confirmations-remove-without-saving = Remove Without Saving
confirmations-replace-project-unsaved-changes = Replace Project: Unsaved Changes
confirmations-save = Save
confirmations-save-anyway = Save Anyway
confirmations-save-changes-current-project-before = Save changes to the current project before replacing it?
confirmations-save-changes-name-before-closing = Save changes to '{ $name }' before closing it?
confirmations-save-changes-name-before-removing = Save changes to '{ $name }' before removing it from Incline Design?
confirmations-save-close = Save and Close
confirmations-save-modified-project-before-exiting = Save the modified project before exiting?
confirmations-save-to-browser-before-exit = Save the modified project to browser storage before exiting?
confirmations-save-remove = Save and Remove

## Console strings

console-copy-all = Copy all
console-copy-message = Copy message
console-error = ERROR
console-info = INFO
console-no-console-activity-yet = No console activity yet
console-pending = PENDING
console-progress-summary = In progress · { $summary }
console-success = SUCCESS
console-warn = WARN

## Csv strings

csv-block-model-category = Category
csv-block-model-value = Value
csv-drill-hole-rows-for-undefined-holes = { $count } rows were for a hole the bundle's geometry does not define
csv-drill-hole-count-rows-were-skipped-total = { $count } rows were skipped in total
csv-drill-hole-csv-file-empty = CSV file is empty
csv-drill-hole-csv-has-too-many-unreadable = CSV has too many unreadable bytes to repair; it is probably in a legacy encoding, so save it as UTF-8 and import it again
csv-drill-hole-csv-header-has-no-columns = CSV header has no columns
csv-drill-hole-csv-headers-must-nonblank-unique = CSV headers must be nonblank and unique
csv-drill-hole-geophysics-needs-geometry = Downhole geophysics needs a collar or explicit-segment file in the bundle, whose holes it attaches to
csv-drill-hole-azimuth-out-of-range = { $file } holds { $count } rows whose azimuth is not between 0 and 360
csv-drill-hole-dip-out-of-range = { $file } holds { $count } rows whose dip is not between -90 and 90; those rows were read without a direction
csv-drill-hole-file-inclination-values-could-angle = { $file } inclination values that could be an angle are all at or below zero, so the column was read as dip, negative downward
csv-drill-hole-file-maps-gamma-density-column = { $file } maps a gamma or density column twice
csv-drill-hole-invalid-utf8 = { $file } is not valid UTF-8; { $count } unreadable byte(s) were replaced in { $cells } cell(s); a damaged cell is not read as data
csv-drill-hole-file-requires-gamma-density-column = { $file } requires a gamma or density column
csv-drill-hole-row-undefined-hole = { $file } row { $row } is for DHID '{ $dhid }', a hole the bundle's geometry does not define
csv-drill-hole-holes-hole-s-carry-overlapping = { $holes } hole(s) carry overlapping intervals, such as a seam logged alongside its splits: { $summary }
csv-drill-hole-skipped-row-reason = Skipped a row: { $reason }
csv-geophysics-above-5 = above 5
csv-geophysics-below-0-5 = below 0.5
csv-geophysics-count-more = (+{ $count } more)
csv-geophysics-count-rows-were-skipped-total = { $count } rows were skipped in total in { $file }
csv-geophysics-csv-has-record-longer-than = CSV has a record longer than { $limit } MiB: the file has no line breaks where a CSV has them, or is not text
csv-geophysics-csv-has-unterminated-quoted-field = CSV has an unterminated quoted field
csv-geophysics-curve-file-was-left-out = { $curve } in { $file } was left out: most of its readings are { $side }, so its median is outside 0.5 to 5 g/cc and its unit looks wrong (g/cc expected). Incline converts no units; correct the export and link it again
csv-geophysics-file-empty = { $file } is empty
csv-geophysics-file-has-no-curve-no = { $file } has no curve: no column besides the hole id and depth holds numbers
csv-geophysics-file-mapping-has-mapped-columns = { $file } mapping has { $mapped } columns, CSV has { $found }
csv-geophysics-file-no-longer-matches-its = { $file } no longer matches its index: link it again
csv-geophysics-file-not-grouped-hole-its = { $file } is not grouped by hole: its holes' rows are split across too many runs. Sort it by hole id, then depth, and link it again
csv-geophysics-file-requires-one-dhid-one = { $file } requires one DHID and one depth column
csv-geophysics-row-blank-hole-id = { $file } row { $row } has a blank hole id
csv-geophysics-row-column-count = { $file } row { $row } has { $found } columns; expected { $expected }
csv-geophysics-row-negative-depth = { $file } row { $row } has a negative depth
csv-geophysics-row-no-depth = { $file } row { $row } has no readable depth
csv-geophysics-file-s-path-not-valid = the file's path is not valid UTF-8, which a project cannot save: rename the file or its folder and link it again
csv-geophysics-rows-skipped = { $file }: { $skipped } of { $rows } rows could not be read; the reasons are in the console
csv-geophysics-runs-not-grouped = Geophysics for { $count } hole(s) comes in more than one run, not grouped by hole; each later run adds only depths its hole has no reading at: { $holes }
csv-geophysics-linked-downhole-geophysics-from-file = Linked downhole geophysics from { $file }: { $holes } hole(s), curves { $curves }; { $rows } row(s) read, { $skipped } skipped. The readings stay in the file and are read a hole at a time
csv-geophysics-no-readings = no readings
csv-geophysics-no-usable-depth-step = no usable depth step
csv-geophysics-run-count-mismatch = Read { $read } run(s) of { $hole }, the link has { $runs }
csv-geophysics-rows-geophysics-row-s-count = { $rows } geophysics row(s) for { $count } hole(s) the dataset does not define are not linked: { $holes }
csv-geophysics-rows-readings-would-need-samples = { $rows } readings would need { $samples } samples
csv-geophysics-run-hole-curve-was-not = A run of { $hole } { $curve } was not kept ({ $reason })

data-table-copy-selection = Copy selection
data-table-copy-table = Copy table

## Drill strings

drill-hole-add = Add
drill-hole-add-all = Add all
drill-hole-add-stop = Add stop
drill-hole-add-working-section = Add working section
drill-hole-all-rendered-intervals-opaque-white = All rendered intervals are opaque white.
drill-hole-another-working-section-field-has = Another working section of this field has that name.
drill-hole-assumed = Assumed
drill-hole-burden-spacing-must-greater-than = Burden and spacing must be greater than zero
drill-hole-cache-drill-hole-set-name-has = Drill hole set { $name } has { $count } holes and tie-ins, past the { $capacity } the selection highlight can carry: selecting the set as a whole still highlights it, selecting single holes will not
drill-hole-cache-drill-hole-set-name-stations = Drill hole set { $name }: { $stations } stations, { $before } segments merged to { $after }, { $cells } cells
drill-hole-choose-valid-closed-polyline = Choose a valid closed polyline
drill-hole-clear-filter = Clear the filter
drill-hole-code-already-in-section = { $code } is already in working section { $section }.
drill-hole-code-outside-section-has-name = A code outside this section has that name. A section may share its name only with a code it holds.
drill-hole-colour-scale = Colour scale
drill-hole-count-codes = { $count } codes
drill-hole-count-codes-interval-no-logged = { $count } codes. An interval with no logged value stays white.
drill-hole-disc-diameter = Disc diameter
drill-hole-appearance-title = Drill Hole Appearance: { $name }
drill-hole-drilled-diameter = Of drilled diameter
drill-hole-every-code-lists-already-another = Every code it lists is already in another working section.
drill-hole-every-interval-value-colour-field = Every interval with a value in the colour field is drawn as a disc this wide on the string. Far away it is never narrower than a few pixels.
drill-hole-field = Field
drill-hole-field-working-section = { $field } by working section
drill-hole-floor = Floor
drill-hole-grayscale = Grayscale
drill-hole-green-yellow-red = Green–Yellow–Red
drill-hole-heat = Heat
drill-hole-drilled-width-help = A hole at its drilled width reads as a pipe beside the geology; a set of thousands reads as a mat.
drill-hole-line-width-help = The hole itself is drawn as a line this wide at every zoom.
drill-hole-however-far-eye-hole-drawn = However far the eye is, a hole is drawn at least this wide.
drill-hole-measured = Measured
drill-hole-name-working-section = { $name } (working section)
drill-hole-never-thinner-than = Never thinner than
drill-hole-new-section-name = New section name
drill-hole-new-working-section = New working section
drill-hole-no-holes-fit-inside-boundary = No holes fit inside this boundary at the current burden and spacing
drill-hole-part-code = Part of a code
drill-hole-pattern-too-many-holes = Pattern exceeds the maximum of { $maximum } holes; increase burden or spacing
drill-hole-preset = Preset
drill-hole-px = px
drill-hole-rainbow = Rainbow
drill-hole-reset-colours = Reset colours
drill-hole-reset-preset = Reset preset
drill-hole-reset-shown-colours = Reset shown colours
drill-hole-roof = Roof
drill-hole-rotation-offsets-must-contain-valid = Rotation and offsets must contain valid numbers
drill-hole-selected-polyline-has-no-usable = The selected polyline has no usable XY area
drill-hole-shown-total-codes-shown = { $shown } of { $total } codes shown
drill-hole-shown-total-rows-shown = { $shown } of { $total } rows shown
drill-hole-smooth-interpolation = Smooth interpolation
drill-hole-spacing-would-scan-too-many = This spacing would scan too many grid cells; increase burden or spacing (maximum { $maximum } holes)
drill-hole-square = Square
drill-hole-staggered = Staggered
drill-hole-stepped-bands = Stepped bands
drill-hole-string-discs = String and discs
drill-hole-string-discs-where-intervals-overlap = As string and discs, where intervals overlap the shortest one is drawn as the disc.
drill-hole-string-width = String width
drill-hole-style = Style
drill-hole-suggested-from-code-names-count = Suggested from code names ({ $count })
common-times-sign = ×
common-minus-sign = −
drill-hole-ticked-but-hidden-filter-count = Ticked but hidden by the filter: { $count }
drill-hole-true-diameter = True diameter
drill-hole-unsupported-drillhole-source = Unsupported drillhole source
drill-hole-width = Width
drill-hole-working-section-needs-name = A working section needs a name.
drill-hole-working-section-set-seams-plies = A working section is a set of seams or plies mined as one unit. Colouring by it gives the whole set one colour.
drill-hole-working-sections = Working sections
drill-pattern-arrangement = Arrangement
drill-pattern-axis-offset = { $axis } offset
drill-pattern-blast-shape = Blast shape
drill-pattern-burden = Burden
drill-pattern-choose-closed-blast-boundary-then = Choose a closed blast boundary, then tune the grid. The drill holes update live in the viewport.
drill-pattern-closed-design-polyline-whose-xy = The closed design polyline whose XY footprint will be filled with holes.
drill-pattern-rotation-help = Counter-clockwise pattern rotation from the global { $axis } axis.
drill-pattern-distance-between-holes-along-each = Distance between holes along each pattern row.
drill-pattern-name-hint = e.g. West Cut 03
drill-pattern-diameter-help = Finished hole diameter. Entered in millimetres and stored with every generated hole.
drill-pattern-hole-depth = Hole depth
drill-pattern-hole-diameter = Hole diameter
drill-pattern-move-over-closed-polyline-then = Move over a closed polyline, then click it in the viewport. Esc cancels the pick.
drill-pattern-name-help = Name of the drillhole dataset created in the project.
drill-pattern-none-picked = None picked
drill-pattern-pattern-name = Pattern name
drill-pattern-spacing-help = Perpendicular distance between pattern rows.
drill-pattern-pick = Pick
drill-pattern-preview-count-hole-s-diameter = Preview: { $count } hole(s) · { $diameter } mm diameter · { $depth } m deep
drill-pattern-rotation = Rotation
drill-pattern-shift-pattern-grid-along-global = Shift the pattern grid along the global { $axis } axis while keeping it clipped to the blast shape.
drill-pattern-spacing = Spacing
drill-pattern-staggered-offsets-every-second-row = Staggered offsets every second row by half the spacing.
drill-pattern-vertical-depth-below-each-collar = Vertical depth below each collar.

## Dxf strings

dxf-block-nesting-too-deep = DXF block nesting exceeds maximum depth ({ $depth }), skipping '{ $name }'
dxf-circular-block-reference = DXF circular block reference detected: '{ $name }'
dxf-undefined-layer = DXF entity referenced undefined layer '{ $name }', imported as '{ $fallback }'
dxf-import-budget-exceeded = DXF import exceeds the { $what } budget ({ $limit }); remaining geometry is skipped
dxf-insert-unknown-block = DXF INSERT references unknown block '{ $name }'

## Edit strings

edit-absolute-length = Absolute length
edit-absolute-rl = Absolute RL
edit-action = Action
edit-angle = Angle
edit-dip-help = Angle from horizontal, negative downwards: -90 is a vertical hole.
edit-app-web-not-recommended-production = { $app } Web is not recommended for production use. Only use it as a demo.
edit-application = Application
edit-apply = Apply
edit-apply-pick-target = Apply and Pick Target
edit-axis-value = { $axis } value
edit-azimuth = Azimuth
edit-batter-angle = Batter angle (°)
edit-azimuth-help = Bearing the holes are drilled on, in degrees clockwise from grid north.
edit-bench-height = Bench height
edit-benches = Benches
edit-berm-width = Berm width
edit-bezier-curve = Bezier Curve
edit-choose-layer = Choose a layer
edit-measure-help = Choose whether the entered value is distance along the slope, horizontal width, or vertical height.
edit-choose-which-two-polyline-paths = Choose which of the two polyline paths between the selected vertices will be replaced. Length includes elevation and curved edges.
edit-click-corner-closed-polyline = Click a corner on a closed polyline.
edit-click-open-closed-polyline-begin = Click an open or closed polyline to begin.
edit-click-second-vertex-replacement-span = Click the second vertex of the replacement span.
edit-click-vertex-start-replacement-span = Click a vertex to start the replacement span.
edit-collide-triangulation = Collide with Triangulation
edit-confirm-selection = Confirm Selection
edit-control-point-1 = Control point 1
edit-control-point-2 = Control point 2
edit-copy = Copy
edit-corner-radius-limited-so-replacement = Corner radius, limited so the replacement cannot pass adjacent vertices.
edit-create-new-layer = Create a new layer
edit-create-new-project = Create a new project
edit-create-project = Create project
edit-delta-length-m-use = Delta length (m, use + or -)
edit-dip = Dip
edit-direction = Direction
edit-distance = Distance
edit-distance-along-slope = Distance along slope
edit-download-free-native-version-our = Download the free native version at our website ↗
edit-drill-hole = Drill Hole
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = End
edit-enter-valid-elevation = Enter a valid elevation.
edit-exit-slice = Exit slice
edit-finish-polyline = Finish Polyline
edit-generate-batter-berms = Generate Batter-Berms
edit-height = Height
edit-height-change = Height change
edit-height-mode = Height mode
edit-horizontal-distance = Horizontal distance
edit-horizontal-width-each-flat-berm = Horizontal width of each flat berm between successive batters.
edit-hover-choose-which-end-move = Hover to choose which end to move, then click to confirm.
edit-insert-point-elevation = Insert Point at Elevation
edit-intersect = Intersect
edit-kind-properties = { $kind } { $properties }
edit-layer-name = Layer name
edit-load-project = Load Project
edit-longest = Longest
edit-m-s = m/s
edit-measure = Measure
edit-mit-license = MIT License
edit-mode = Mode
edit-move = Move
edit-move-layer = Move to Layer
edit-move-which-end = Move which end
edit-movement-speed-slice-when-using = Movement speed of the slice when using the navigation keys.
edit-moving-end-endpoint = Moving: End endpoint
edit-moving-start-endpoint = Moving: Start endpoint
edit-new-length-m = New length (m)
edit-new-project = New Project
edit-number-complete-batter-berm-levels = Number of complete batter-and-berm levels. The maximum is limited to the deepest level that preserves the specified geometry.
edit-bezier-segments-help = Number of line segments used to approximate the curve between the two selected vertices.
edit-chamfer-segments-help = Number of straight segments used to approximate the rounded corner. Use 1 for a straight chamfer.
edit-object = Object
edit-offset-element = Offset Element
edit-pick-side = Pick Side
edit-pit = Pit
edit-project-name = Project name
edit-properties = Properties
edit-radius = Radius
edit-recent = Recent
edit-relative = Relative (+/-)
edit-elevation-mode-help = Relative applies a vertical change to every point. Absolute RL projects every point onto one target elevation.
edit-remove-from-list = Remove from List
edit-replace-path = Replace path
edit-rotate = Rotate
edit-rotation-speed-slice-when-using = Rotation speed of the slice when using Q and E.
edit-s = °/s
edit-segments = Segments
edit-segments-lying-elevation-ignored = Segments lying at this elevation are ignored.
edit-endpoint-help = Select the endpoint that changes; the other endpoint remains fixed.
edit-selected-holes-point-different-ways = Selected holes point different ways. Apply sets them all to these angles.
edit-selected-start-end-point-moves = The selected start or end point moves along the line direction; the opposite endpoint stays fixed.
edit-set-axis = Set { $axis }
edit-shortest = Shortest
edit-slice-view = Slice View
edit-slope-angle-each-batter-face = Slope angle of each batter face, measured from horizontal.
edit-slope-angle-offset-positive-negative = Slope angle of the offset. Positive and negative angles move the copy above or below the source as it moves sideways.
edit-speed = Speed
edit-start = Start
edit-stockpile = Stockpile
edit-stop-generated-offset-where-its = Stop the generated offset where its path first meets a visible triangulation.
edit-target-rl = Target RL
edit-text-colour-opacity = Text colour and opacity.
edit-thickness-visible-slice-slab-centred = Thickness of the visible slice slab centred on the overview indicator.
edit-translation-axis-help = Translation distance along the world { $axis } axis.
edit-type = Type
edit-type-direction-together-set-offset = Type and Direction together set the offset side. Pit + Up and Stockpile + Down step outward; Pit + Down and Stockpile + Up step inward.
edit-bench-direction-help = Up raises each bench by the bench height; Down lowers it. This also flips the offset side - see Type.
edit-value-help = The value is interpreted using the selected Measure and Height mode.
edit-vertical-rise-fall-each-bench = Vertical rise or fall of each bench before the next berm is created.
edit-bezier-control-point-1-help = World X, Y and Z coordinates of the first Bezier control point.
edit-bezier-control-point-2-help = World X, Y and Z coordinates of the second Bezier control point.

## Events strings

events-couldn-t-exit-error = Couldn't exit: { $error }
events-couldn-t-save-error = Couldn't save: { $error }
events-set-elevation = Set Elevation
events-set-elevation-from-cursor-hit = Set elevation from cursor hit to Z { $z }
events-tool-not-available-section-view = That tool is not available in the section view

## Explorer strings

explorer-clear-active-triangulation-texture = Clear Active Triangulation Texture
explorer-delete-from-project = Delete from Project
explorer-discard-changes = Discard Changes...
explorer-download = Download
explorer-drape-over-surface = Drape Over Surface
explorer-draped-over-surface = Draped over a surface
explorer-duplicate = Duplicate
explorer-empty-collection = Empty collection
explorer-face-colour = Face colour
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    { $count } colour variable(s)
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    { $holes } hole(s)
    { $fields } colour field(s)
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    { $count } point(s)
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Load
explorer-lock = Lock
explorer-new-collection = New Collection
explorer-no-collection = No Collection
explorer-select-all-objects = Select All Objects
explorer-source-name = Source: { $name }
explorer-unload = Unload
explorer-unlock = Unlock

## Files strings

files-automatic-colour = Automatic colour
files-automatic-rl-spacing = Automatic RL spacing
files-axis-scale-ratio = { $axis } scale ratio
files-ok = OK
files-reset-scale = Reset to 1×
files-rl-grid-options = RL Grid Options
files-rl-spacing = RL spacing
files-scales-z-distances-visually-without = Scales Z distances visually without changing stored coordinates.
files-thickness = Thickness
files-xy-grid-options = XY Grid Options

## Geophysics strings

geophysics-checking-geophysics-files = Checking geophysics files
geophysics-downhole-geophysics-name-could-not = Downhole geophysics for '{ $name }' could not be linked: { $error }
geophysics-file-changed = The geophysics file changed since it was indexed
geophysics-file-unreadable = The geophysics file linked to '{ $name }' cannot be read at { $path } ({ $error }); link it again from the dataset's right-click menu
geophysics-linked-changed-rereading = The geophysics linked to '{ $name }' changed since it was indexed; reading it again
geophysics-hole-has-size-mib-geophysics = { $hole } has { $size } MiB of geophysics rows, more than a hole is read at
geophysics-hole-needs-size-mib-its = { $hole } needs { $size } MiB for its geophysics, more than the browser has left: unload other items, then unload and load this dataset again
geophysics-linking-geophysics-name = Linking geophysics to { $name }
geophysics-reading-geophysics-hole = Reading geophysics for { $hole }
geophysics-web-could-not-read-name-error = Could not read '{ $name }': { $error }
geophysics-web-name-used-session-s-downhole = '{ $name }' is used for this session's downhole geophysics

## Gpu strings

gpu-cache-block-model-surface-build-failed = Block-model surface build failed: { $error }
gpu-cache-block-model-surface-build-worker = Block-model surface build worker disconnected
gpu-cache-block-model-surface-chunk-rejected = Block model surface chunk rejected before GPU allocation: instances={ $instances } bytes, limit={ $limit } bytes
gpu-cache-block-volume-worker-disconnected = Block-volume preparation worker disconnected
gpu-cache-translucent-volume-could-not-built = Translucent volume could not be built ({ $error }); showing this block model as cubes instead.
gpu-cache-edge-chunk-rejected = Triangulation edge chunk rejected before GPU allocation: instances={ $instances } bytes, limit={ $limit } bytes
gpu-cache-triangulation-chunk-rejected = Triangulation GPU chunk rejected before allocation: vertices={ $vertices } bytes, indices={ $indices } bytes, limit={ $limit } bytes
gpu-cache-triangulation-too-many-vertices = Triangulation '{ $name }' has { $count } vertices (> u32::MAX); cannot chunk for GPU
gpu-cache-triangulation-uploaded = Triangulation '{ $name }' uploaded in { $chunks } spatial chunks ({ $faces } faces)

## I18n strings

i18n-active-language = Active language is { $language } (bundled: { $bundled })
i18n-could-not-select-language-error = Could not select a language: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = GPU adapter: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = GPU driver: { $driver } { $driver_info }
init-gpu-limits-max-buffer-size = GPU limits: max_buffer_size={ $max_buffer_size } MiB, max_storage_buffer_binding_size={ $max_storage_buffer_binding_size } MiB, max_storage_buffers_per_shader_stage={ $max_storage_buffers_per_shader_stage }, max_uniform_buffer_binding_size={ $max_uniform_buffer_binding_size } KiB, max_texture_dimension_2d={ $max_texture_dimension_2d }, max_bind_groups={ $max_bind_groups }
init-gpu-supports-maximum-buffer-size = GPU supports a maximum buffer size of { $size } MiB; large scenes may not display fully
init-surface-present-mode = Surface presentation mode: { $mode }
init-wgpu-error-continuing-error = wgpu error (continuing): { $error }

## Input strings

input-could-not-read-name-error = could not read { $name }: { $error }
input-could-not-slice-name-error = could not slice { $name }: { $error }

## Io strings

io-add-collar-file-explicit-segments = Add the collar file (or an explicit-segments file): downhole geophysics attaches to the holes it defines.
io-ascii-points-xyz-pts = ASCII Points (.xyz, .pts)
io-attribute = Attribute
io-blank-header = (blank header)
io-block-model = Block model:
io-choose-file-purpose-map-its = Choose a file purpose to map its columns.
io-choose-loaded-block-model = Choose a loaded block model
io-choose-loaded-dataset = Choose a loaded dataset
io-choose-loaded-layer = Choose a loaded layer
io-choose-loaded-triangulation = Choose a loaded triangulation
io-choose-purpose = Choose purpose…
io-choose-source-file-files-import = Choose the source file or files to import.
io-collar = Collar
io-column-mapping = Column mapping
io-comma-separated-values-csv = Comma-Separated Values (.csv)
io-csv-files = CSV files
io-dataset = Dataset:
io-default = Default
io-density-read-g-cc-exported = Density, read as g/cc, as exported. A curve whose median is not between 0.5 and 5 g/cc is left out of the import with a warning, its unit looking wrong.
io-depth = Depth
io-diameter = Diameter
io-downhole-geophysics = Downhole geophysics
io-drawing-exchange-format-dxf = Drawing Exchange Format (.dxf)
io-drill-holes = Drill holes
io-east-x = East / X
io-elevation-z = Elevation / Z
io-end-x = End X
io-end-y = End Y
io-end-z = End Z
io-explicit-segments = Explicit segments
io-export = Export
io-export-csv-block-model = Export CSV Block Model
io-export-csv-drillholes = Export CSV Drillholes
io-export-dxf = Export DXF
io-export-one-layer = Export one layer
io-export-open-mining-format-2 = Export Open Mining Format 2
io-export-ply = Export PLY
io-export-stl = Export STL
io-export-wavefront-obj = Export Wavefront OBJ
io-gamma-api = Gamma (API)
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Import
io-import-ascii-point-cloud = Import ASCII Point Cloud
io-import-drillhole-csv-bundle = Import Drillhole CSV Bundle
io-import-geotiff = Import GeoTIFF
io-import-las-laz-point-cloud = Import LAS/LAZ Point Cloud
io-import-open-mining-format-2 = Import Open Mining Format 2
io-import-pcd-point-cloud = Import PCD Point Cloud
io-import-ply = Import PLY
io-import-stl = Import STL
io-import-wavefront-obj = Import Wavefront OBJ
io-inclination = Inclination
io-interval = Interval
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-long-spaced-density-g-cc = Long-spaced density (g/cc)
io-mapped-csv-bundle-csv = Mapped CSV bundle (.csv)
io-measured-depth-down-hole-read = Measured depth down the hole, read as metres. Incline converts no units: the database that exported the file sets them.
io-model-file = Model file
io-name-count-files = { $name } + { $count } files
io-natural-gamma-read-api-units = Natural gamma, read as API units, as exported.
io-no-csv-chosen = No .csv chosen
io-no-csv-files-chosen = No CSV files chosen
io-no-dxf-chosen = No .dxf chosen
io-no-omf-chosen = No .omf chosen
io-north-y = North / Y
io-open-mining-format-2-omf = Open Mining Format 2 (.omf)
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-projects = Projects
io-short-spaced-density-g-cc = Short-spaced density (g/cc)
io-source-file = Source file
io-start-x = Start X
io-start-y = Start Y
io-start-z = Start Z
io-stl = STL (.stl)
io-triangulation = Triangulation:
io-unmapped = Unmapped
io-wavefront-obj = Wavefront OBJ (.obj)
io-writes-three-files-beside-name = Writes three files beside the name you choose: collars, survey and intervals, in the columns this dialog imports.

## Jobs strings

jobs-background-task-poll-label-ended = Background task '{ $poll_label }' ended without a result
jobs-cancelled-label-its-project-no = Cancelled '{ $label }': its project is no longer active
jobs-discarded-stale-result = Discarded stale background result for '{ $poll_label }' because a source changed or closed
jobs-drillhole-import = a drillhole import

## Log strings

log-traces-auto-from-hole = Auto, from this hole
log-traces-curve-no-reading = { $curve }: no reading
log-traces-curve-value-unit = { $curve }: { $value } { $unit }
log-traces-custom-range = Custom range
log-traces-default-colour = Default colour
log-traces-density-scale = Density scale
log-traces-depth-m = { $depth } m
log-traces-gamma = Gamma
log-traces-gamma-colour = Gamma colour
log-traces-gamma-scale = Gamma scale
log-traces-percentile-range-no-data = The hole's 1st to 99th percentile, rounded outward. This hole has no data for it yet.
log-traces-percentile-range = The hole's 1st to 99th percentile, rounded outward: { $range }.
log-traces-long-density = Long density
log-traces-long-density-colour = Long density colour
log-traces-min-max-unit = { $min } to { $max } { $unit }
log-traces-reading = Reading...
log-traces-short-density = Short density
log-traces-short-density-colour = Short density colour

## Logging strings

logging-activity-completed = Activity completed
logging-activity-started = Activity started
logging-application-id-id = Application ID: { $id }
logging-application-name = Application name: { $name }
logging-application-startup = Application Startup
logging-build-target-os-architecture = Build target: { $os }-{ $architecture }
logging-completed = Completed
logging-count-messages = { $count } messages
logging-desktop-session-xdg-session-type = Desktop session: XDG_SESSION_TYPE={ $session }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Initialising Incline Design
logging-locale-environment = Locale environment: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = macOS session: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Operating system: GNU / Linux
logging-operating-system-macos = Operating system: macOS
logging-operating-system-microsoft-windows = Operating system: Microsoft Windows
logging-pointer-width = Pointer width: { $width }-bit
logging-process-id-id = Process ID: { $id }
logging-release-version = Release version: { $version }
logging-renderer = Renderer
logging-rust-compiler-host = Rust compiler host: { $host }
logging-system = System
logging-system-error = System Error
logging-unknown = unknown
logging-windows-session-sessionname-session = Windows session: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = Working…

## Mac strings

mac-cannot-install-macos-menu-bar = Cannot install the macOS menu bar away from the main thread
mac-quit-app = Quit { $app }

## Main strings

main-incline-design-web-startup-failed = Incline Design Web startup failed: { $error }

## Menu strings

menu-count-files-selected = { $count } files selected

## Modelling strings


## Object strings

object-edit-appearance = Appearance
object-edit-arc-circle = Arc & Circle
object-edit-arc-segments = Arc segments
object-edit-bulge = Bulge
object-edit-bulge-arcs-horizontal-data-model = Bulge arcs are horizontal by data model: the arc turns in plan and the elevation runs straight from one vertex to the next.
object-edit-centre-x = Centre X
object-edit-centre-y = Centre Y
object-edit-centre-z = Centre Z
object-edit-chord = Chord
object-edit-colour-layer = Colour by layer
object-edit-enter-number = Enter a number
object-edit-follow-owning-layer-s-colour = Follow the owning layer's colour instead of a colour pinned to this object.
object-edit-id = ID
object-edit-identity = Identity
object-edit-insert-after = Insert after
object-edit-join-last-vertex-back-first = Join the last vertex back to the first.
object-edit-length = Length { $length } m
object-edit-move-down = Move down
object-edit-move-up = Move up
object-edit-object-has-no-arc-segments = This object has no arc segments.
object-edit-object-has-single-position = This object has a single position.
object-edit-object-needs-least-required-vertices = This object needs at least { $required } vertices
object-edit-one-more-properties-not-valid = One or more properties is not a valid number
object-edit-perimeter-area = Perimeter { $length } m, area { $area } m²
object-edit-reverse = Reverse
object-edit-row-invalid-number = Row { $row }: position or bulge is not a valid number
object-edit-sweep = Sweep
object-edit-text-not-number = "{ $text }" is not a number
object-edit-vertices = Vertices

## Omf strings

omf-element-name-has-count-tie = Element '{ $name }' has { $count } tie-in(s) naming holes it no longer contains
omf-element-name-has-count-unreadable = Element '{ $name }' has { $count } unreadable working section(s); they were left out
omf-element-unsupported-section = Element '{ $name }' names section '{ $section }' which cannot show this kind of item in this build
omf-element-name-names-unknown-section = Element '{ $name }' names an unknown section '{ $section }'
omf-ignoring-colour-map-omf-attribute = Ignoring the colour map on OMF attribute '{ $attribute }': { $error }
omf-mining-data-exported-incline = Mining data exported by Incline
omf-import = OMF import
omf-texture = OMF texture
omf-validation-warnings = OMF validation warnings: { $warnings }
omf-application-metadata-dropped = Project application metadata '{ $application }' is not retained
omf-project-author-not-retained = Project author is not retained
omf-project-description-not-retained = Project description is not retained
omf-unsupported-metadata-keys = Project has unsupported metadata keys: { $keys }
omf-skipped-drillhole-data-saved-older = Skipped drillhole data saved in an older layout ({ $names }); import it again from its source files

## Plot strings

plot-1-1000-one-millimetre-sheet = At 1:1000, one millimetre on the sheet is one metre on the ground.
plot-1-scale-covers-width-height = 1:{ $scale } · covers { $width } × { $height } m
plot-all-visible-data = All visible data
plot-automatic-grid-interval = Automatic grid interval
plot-border = Border
plot-centre = Centre on
plot-fit-scale-help = Choose the smallest conventional scale that fits everything visible onto the sheet.
plot-coordinate-grid = Coordinate grid
plot-current-view-centre = Current view centre
plot-date-caps = DATE
plot-date = Date
plot-dots-per-inch-paper-size = Dots per inch. This paper size can be rasterised up to { $max_dpi } dpi; 300 dpi is a normal print quality.
plot-dpi = dpi
plot-drawing-no = DRAWING No.
plot-drawing-number = Drawing number
plot-drawn-by-caps = DRAWN BY
plot-drawn-by = Drawn by
plot-e-g-example-gold-project = e.g. Example Gold Project
plot-entered-coordinates = Entered coordinates
plot-export-png = Export PNG...
plot-fit-scale-visible-data = Fit scale to visible data
plot-grid-interval = Grid interval
plot-landscape = Landscape
plot-lists-visible-surfaces-design-layers = Lists the visible surfaces and design layers with their colours.
plot-margin = Margin
plot-margins-leave-no-room-map = The margins leave no room for the map
plot-metres-scale-1-scale = metres    Scale 1:{ $scale }
plot-mm = mm
plot-north-arrow = North arrow
plot-nothing-visible-draw = Nothing visible to draw
plot-paper = Paper
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } mm
plot-paper-size = Paper size
plot-pick-interval-reads-roughly-every = Pick an interval that reads roughly every 50 mm on the printed sheet.
plot-plan = Plan
plot-scale-must-be-positive = The plot scale must be a positive number
plot-png-written-sheet-s-exact = The PNG is written at the sheet's exact paper size and records its DPI, so it prints at true scale.
plot-portrait = Portrait
plot-resolution = Resolution
plot-rev = REV
plot-revision = Revision
plot-scale = SCALE
plot-scale-ratio = Scale  1:
plot-scale-framing = Scale and framing
plot-sheet-furniture = Sheet furniture
plot-size-width-height-mm = { $size } ({ $width } × { $height } mm)
plot-subtitle = Subtitle
plot-title = Title
plot-title-block = Title block
plot-today = today

## Point strings

point-cloud-classify = Classify
point-cloud-classify-vegetation = Classify vegetation
point-cloud-cloth-resolution = Cloth resolution
point-cloud-cloth-resolution-about-one-half = A cloth resolution about one and a half times the spacing of the sparsest selected cloud's points, so every particle has returns under it.
point-cloud-combine-selected-point-clouds-into = Combine the selected point clouds into one new cloud, so a single triangulation can be built across all of them. Per-point colours are kept; a cloud without them contributes its display colour.
point-cloud-selected-count = { $count } selected · { $points } points
point-cloud-delete-selected-clouds-from-project = Delete the selected clouds from the project once the join completes, freeing the memory their duplicate copy would otherwise hold.
point-cloud-flat-pads-structures = Flat (pads, structures)
point-cloud-ground-cloud-covers-steep-follows = The ground the cloud covers. Steep follows walls down from their crests; Flat uses a stiffer cloth that bridges large buildings and plant but rounds off sharp breaks.
point-cloud-ground-threshold = Ground threshold
point-cloud-how-far-around-each-point = How far around each point to count neighbours.
point-cloud-join = Join
point-cloud-let-cloth-follow-walls-down = Let the cloth follow walls down from their crests, where its stiffness would otherwise hold it off the face. Turn off only on gentle ground crowded with plant.
point-cloud-mark-each-point-ground-noise = Mark each point as ground, noise or unclassified. A cloth is pressed up under the cloud and settles on the ground surface; points within the ground threshold of it are ground. Any existing classes are replaced; undo restores them.
point-cloud-mark-isolated-returns-birds-dust = Mark isolated returns - birds, dust, multipath blunders - as noise before the ground is found, so a stray low point cannot drag the cloth down.
point-cloud-mark-noise = Mark noise
point-cloud-minimum-neighbours = Minimum neighbours
point-cloud-name-assigned-joined-point-cloud = Name assigned to the joined point cloud.
point-cloud-name-count-points = { $name } ({ $count } points)
point-cloud-noise-radius = Noise radius
point-cloud-point-clouds = Point clouds
point-cloud-points-closer-than-settled-cloth = Points closer than this to the settled cloth, measured across its surface, are ground.
point-cloud-points-fewer-neighbours-than-within = Points with fewer neighbours than this within the noise radius are noise.
point-cloud-raise-cloth-resolution-if-your = Raise the cloth resolution if your machine has less RAM.
point-cloud-recommended = Recommended
point-cloud-recover-steep-slopes = Recover steep slopes
point-cloud-relief-dumps-rolling-ground = Relief (dumps, rolling ground)
point-cloud-remove-sources = Remove sources
point-cloud-resolution-m-points-spacing-m = { $resolution } m (points ~{ $spacing } m apart)
point-cloud-selected-clouds-copied-into-joined = The selected clouds, copied into the joined cloud. Close the dialog to join a different set.
point-cloud-selected-clouds-each-classified-its = The selected clouds, each classified on its own. Close the dialog to classify a different set.
point-cloud-classify-help = Sort the returns with a trained classifier that reads the shape of the points around each one: ground, vegetation - banded low (under 1 m), medium (under 3 m) or high by height - and everything else, such as buildings and plant, left unclassified. Turn this off to use the cloth alone.
point-cloud-spacing-cloth-s-particles-around = Spacing of the cloth's particles. Around the cloud's point spacing is a good start; finer follows the ground more closely but needs denser points.
point-cloud-steep-pit-walls-benches = Steep (pit walls, benches)
point-cloud-terrain = Terrain
point-cloud-use = Use

## Products strings

products-add-initiation = Add Initiation
products-delay = Delay
products-delay-palette = Delay Palette
products-how-long-after-shot-fired = How long after the shot is fired this collar initiates the round.
products-initiation-name = Initiation · { $name }
products-milliseconds-between-one-hole-firing = Milliseconds between one hole firing and the next.
products-ms = ms
products-no-products = No products
products-remove = Remove
products-update = Update

## Progress strings

progress-percent-done-total = { $percent } ({ $done } of { $total })
progress-task-finished = { $task }: Finished

## Project strings

project-item = Item

## Properties strings

properties-adds-view-dependent-rim-highlight = Adds a view-dependent rim highlight at block and material boundaries. Leaving this off slightly reduces volume-rendering work.
properties-block-model-downscale = Block model downscale
properties-camera = Camera
properties-camera-clip-planes = Camera clip planes
properties-cap-while-resizing = Cap while resizing
properties-colours-each-point-cloud-chunk = Colours each point-cloud chunk, outlines the box it is frustum-culled by, and shows the points drawn last frame against the level-of-detail target and the visible total in the status bar.
properties-colours-each-surface-chunk-outlines = Colours each surface chunk, outlines the box it is frustum-culled by, and shows the faces drawn last frame against the visible total in the status bar.
properties-dark-mode = Dark mode
properties-dataset = Dataset
properties-developer = Developer
properties-downscale-rasters = Downscale rasters
properties-drillholes = Drillholes
properties-edit-object = Edit Object...
properties-field-view = Field of view
properties-fps = FPS
properties-frame-counter = Frame counter
properties-frame-rate-cap = Frame rate cap
properties-hz = Hz
properties-interface = Interface
properties-invert-horizontal = Invert horizontal
properties-invert-vertical = Invert vertical
properties-limits-newly-loaded-geotiff-previews = Limits newly loaded GeoTIFF previews to 4096 pixels on their longest side. Disable to use full resolution up to the GPU's texture limit, which uses more memory.
properties-line-colour = Line colour
properties-look-sensitivity = Look sensitivity
properties-max-clip-span = Max clip span
properties-move-layer = Move to Layer...
properties-near-clip-limit = Near clip limit
properties-no-drillhole-datasets-open = No drillhole datasets are open.
properties-orbit-sensitivity = Orbit sensitivity
properties-panel-chrome = Panel chrome
properties-performance = Performance
properties-plan-mode = Plan Mode
properties-point-cloud-chunk-debug-view = Point cloud chunk debug view
properties-presents-step-display-no-tearing = Presents in step with the display: no tearing, and the display sets the frame rate. Off, frames present as soon as they are drawn and the cap below applies.
properties-reflective-block-edges = Reflective block edges
properties-restore-defaults = Restore Defaults
properties-show-console = Show console
properties-shows-live-near-far-projection = Shows the live near and far projection distances in the status bar.
properties-snap-polling = Snap polling
properties-surface-chunk-debug-view = Surface chunk debug view
properties-vertical-sync = Vertical sync
properties-world-axis-gizmo = World axis gizmo
properties-zoom-cursor = Zoom to cursor
properties-zoom-sensitivity = Zoom sensitivity

## Reference strings

reference-points-count-holes-from-dataset = { $count } holes from '{ $dataset }'
reference-points-holes-from-datasets = { $count } holes from { $datasets } datasets
reference-points-holes = Holes
reference-points-holes-points-placed-selected-when = The holes the points are placed on, as selected when the dialog opened. Close the dialog to select different ones.
reference-points-make = Make
reference-points-no-categorical-field = No categorical field
reference-points-no-values = No values
reference-points-one-point-per-hole-boundary = One point per hole at that boundary, as a new layer. A hole holding the section twice gives its uppermost and is flagged.
reference-points-reference-points = Reference Points
reference-points-side = Side
reference-points-working-section = Working section
reference-points-working-section-field = Working section field
reference-surface-controls = Controls
reference-surface-extent = Extent
reference-surface-points-outside-extent-still-shape = Points outside the extent still shape the surface; only the surface is clipped to it.
reference-surface-points-surface-built-from-selected = The points the surface is built from, as selected when the dialog opened. Close the dialog to select different ones.
reference-surface-extent-help = The selected closed string the finished surface is clipped to; points outside it still shape the surface.
reference-surface-selected-open-strings-surface-made = The selected open strings the surface is made to pass through, as selected when the dialog opened. Close the dialog to select different ones.
reference-surface-triangulates-selected-points-plan-in = Triangulates the selected points in plan into a new surface. Each build adds a surface.

## Screenshot strings

screenshot-could-not-encode-viewport-image = Could not encode viewport image: { $error }
screenshot-could-not-map-viewport-screenshot = Could not map viewport screenshot: { $error }
screenshot-could-not-save-viewport-image = Could not save viewport image { $path }: { $error }
screenshot-downloaded-viewport-image-file-name = Downloaded viewport image: { $file_name }
screenshot-saved-viewport-image-path = Saved viewport image: { $path }
screenshot-viewport-image-download-failed-error = Viewport image download failed: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = BVH face index { $index } out of range for mesh; substituting degenerate triangle

## State strings

state-above = at or above
state-activate-project = Activate Project
state-all-open-incline-design-data = All open Incline Design data
state-apply-generated-rings = Apply generated rings
state-apply-selection = Apply to selection
state-rotate-by-azimuth-dip = by azimuth { $azimuth }°, dip { $dip }°
state-rotate-to-azimuth-dip = to azimuth { $azimuth }°, dip { $dip }°
state-below = at or below
state-build-reference-points = Build Reference Points
state-centre-rotation = Centre of Rotation
state-checking-unsaved-work = Checking unsaved work
state-choose-destination = Choose a destination
state-choose-one-more-files = Choose one or more files
state-clear-raster = Clear Raster
state-click-pit-shell-viewport = Click the pit shell in the viewport.
state-click-pit-stockpile-solid-viewport = Click the pit or stockpile solid in the viewport.
state-click-surface-viewport = Click the surface in the viewport.
state-click-topology-viewport = Click the topology in the viewport.
state-close-project = Close Project
state-colour-drillholes = Colour Drillholes
state-colour-drillholes-working-section = Colour Drillholes by Working Section
state-colour-points-classification = Colour Points by Classification
state-copy-objects-layer = Copy Objects to Layer
state-count-cloud-s = { $count } cloud(s)
state-count-file-s = { $count } file(s)
state-count-object-s-axis-value = { $count } object(s) · { $axis } { $value }
state-count-object-s-closed = { $count } object(s) · { $closed }
state-count-object-s-layer = { $count } object(s) · { $layer }
state-count-object-s-weight = { $count } object(s) · { $weight }
state-count-object-s-z-elevation = { $count } object(s) · Z { $elevation }
state-points-controls-clipped = { $count } point(s) · { $controls } control string(s) · clipped to the extent string
state-points-controls-unclipped = { $count } point(s) · { $controls } control string(s) · unclipped
state-create-collection = Create Collection
state-create-point-cloud-tin = Create Point Cloud TIN
state-create-project = Create Project
state-current-project = Current project
state-cut-topology-pit-shell = Cut Topology to Pit Shell
state-cut-triangulation-polyline = Cut Triangulation by Polyline
state-cut-triangulation-z = Cut Triangulation by Z
state-dark-mode = Dark Mode
state-data-ticked-export-checklist = The data ticked in the export checklist
state-detached = Detached
state-disabled = Disabled
state-discard-project-changes = Discard Project Changes
state-discard-replace-project = Discard and Replace Project
state-discarding-unsaved-changes = Discarding unsaved changes
state-docked = Docked
state-drape-raster = Drape Raster
state-drill-pattern = Drill Pattern
state-duplicate-layer = Duplicate Layer
state-east = East
state-enabled = Enabled
state-exit-incline-design = Exit Incline Design
state-export-block-model-csv = Export Block Model CSV
state-export-drillhole-csv = Export Drillhole CSV
state-export-layer-dxf = Export Layer to DXF
state-export-omf = Export OMF
state-export-project-dxf = Export Project to DXF
state-export-triangulation = Export Triangulation
state-export-viewport-image = Export Viewport Image
state-finish-closed-polyline = Finish closed polyline
state-finish-open-polyline = Finish open polyline
state-fit-extents = Fit to extents
state-fix-release-centre-both-views = Fix or release the centre both views orbit about
state-folder-section = { $folder } in { $section }
state-generate-contours = Generate Contours
state-hidden = Hidden
state-import-drillholes = Import Drillholes
state-import-omf = Import OMF
state-import-point-cloud = Import Point Cloud
state-import-raster = Import Raster
state-import-triangulation = Import Triangulation
state-insert-intersection-points = Insert Intersection Points
state-insert-points-elevation = Insert Points at Elevation
state-keep-inside = Keep inside
state-keep-outside = Keep outside
state-kriged-block-model = Kriged Block Model
state-load-block-model = Load Block Model
state-load-drillholes = Load Drillholes
state-load-layer = Load Layer
state-load-point-cloud = Load Point Cloud
state-load-raster = Load Raster
state-load-triangulation = Load Triangulation
state-locked-count-object-s = Locked { $count } object(s)
state-major-minor = Major { $major } · minor { $minor }
state-member-into-folder-section = { $member } into { $folder } in { $section }
state-member-root-section = { $member } to the root of { $section }
state-move-axis-value = Move to Axis Value
state-move-objects-layer = Move Objects to Layer
state-name-count-cloud-s = { $name } · { $count } cloud(s)
state-name-count-holes = { $name } · { $count } holes
state-name-count-object-s = { $name } · { $count } object(s)
state-name-z-min-z-max = { $name } · { $z_min } to { $z_max }
state-new-collection-under-section = New collection under { $section }
state-next-edit = Next edit
state-north = North
state-off = Off
state-on = On
state-open-containing-folder = Open the containing folder
state-open-project = Open Project
state-preserve-view-angle = Preserve view angle
state-previous-edit = Previous edit
state-project-id = Project { $id }
state-remove-block-model = Remove Block Model
state-remove-drillholes = Remove Drillholes
state-remove-point-cloud = Remove Point Cloud
state-remove-raster = Remove Raster
state-remove-triangulation = Remove Triangulation
state-removed-from-active-triangulation = Removed from active triangulation
state-removed-from-every-triangulation = Removed from every triangulation
state-rename-kind = Rename { $kind }
state-save-close-project = Save and Close Project
state-save-despite-unsupported-content = Save despite unsupported content
state-save-project = Save Project As
state-save-replace-project = Save and Replace Project
state-saving-current-project = Saving the current project
state-section-name = { $section } section
state-select-layer-objects = Select Layer Objects
state-selected-objects = Selected objects
state-selected-polylines = Selected polylines
state-selected-scene-elements = Selected scene elements
state-set-block-model-variable = Set Block Model Variable
state-set-cinematic-view = Set Cinematic View
state-set-drillhole-colour-preset = Set Drillhole Colour Preset
state-set-drillhole-discs = Set Drillhole Discs
state-set-drillhole-style = Set Drillhole Style
state-set-drillhole-width = Set Drillhole Width
state-set-entity-lock = Set Entity Lock
state-set-grid = Set Grid
state-set-layer-lock = Set Layer Lock
state-set-line-weight = Set Line Weight
state-set-object-colour = Set Object Colour
state-set-object-fill = Set Object Fill
state-set-point-visibility = Set Point Visibility
state-set-polyline-closed = Set Polyline Closed
state-set-raster-lock = Set Raster Lock
state-set-standard-view = Set Standard View
state-set-topology-wireframes = Set Topology Wireframes
state-set-triangulation-colour = Set Triangulation Colour
state-show-console = Show Console
state-show-project = Show Project
state-shown = Shown
state-slice-mode = Slice Mode
state-slice-preview = Slice Preview
state-south = South
state-stem-contours = { $stem } Contours
state-target-new-name = { $target } to “{ $new_name }”
state-trim-above = Trim above
state-trim-below = Trim below
state-trim-triangulation-surface = Trim Triangulation to Surface
state-undrape-raster = Undrape Raster
state-undrape-rasters = Undrape Rasters
state-unload-block-model = Unload Block Model
state-unload-drillholes = Unload Drillholes
state-unload-layer = Unload Layer
state-unload-point-cloud = Unload Point Cloud
state-unload-raster = Unload Raster
state-unload-triangulation = Unload Triangulation
state-untitled-project = Untitled project
state-use-typed-radius = Use typed radius
state-west = West

## Status strings

status-clip-near-far = Clip near/far/Δ: -- / -- / --
status-faces-chunks = Faces: -- / -- (--/-- chunks)
status-frame-rate = Frame rate
status-points-chunks = Points: -- / -- of -- (--/-- chunks)

## Text strings

text-could-not-build-vector-mesh = Could not build vector mesh for font { $font }, glyph { $glyph }: { $error }
text-document-text-mesh-exceeded-its = Document text mesh exceeded its u32 index range

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Choose the drillhole dataset to tie in first
tie-in-count-connector-s = { $count } connector(s)
tie-in-delete-tie-ins = Delete Tie-Ins
tie-in-deleted-count-selected-tie-connector = Deleted { $count } selected tie-in connector(s)
tie-in-hole = hole
tie-in-initiation-point-lifted-from-name = Initiation point lifted from { $name }
tie-in-initiation-point-set-name-delay = Initiation point set on { $name } at { $delay } ms
tie-in-select-delay-product-palette-before = Select a delay product in the palette before tying holes in
tie-in-tied-connectors = Tied { $count } connector(s) at { $delay } ms with { $product }
tie-in-tied-connectors-replacing = Tied { $count } connector(s) at { $delay } ms with { $product }, replacing { $replaced }

## Toolbar strings

toolbar-fill-type = Fill type

## Toolbars strings

toolbars-auto-bench = Auto-Bench
toolbars-bezier-polyline = Bezier Polyline
toolbars-chamfer-polyline-corners = Chamfer Polyline Corners
toolbars-create-text = Create Text
toolbars-cursor-regular = Cursor: Regular
toolbars-cursor-snap-line = Cursor: Snap to Line
toolbars-cursor-snap-point = Cursor: Snap to Point
toolbars-cursor-snap-surface = Cursor: Snap to Surface
toolbars-delete-points = Delete Points
toolbars-explode-polyline-lines = Explode Polyline to Lines
toolbars-fuse-polylines = Fuse Polylines
toolbars-measure-distance = Measure Distance
toolbars-new-layer = New Layer
toolbars-split-polyline-points = Split Polyline At Points
toolbars-strike-dip = Strike and Dip
toolbars-tool-not-available-section-view = { $tool } - not available in the section view

## Tri strings

tri-sampling-method-help = Adaptive concentrates vertices on complex terrain via plane-fit error; uniform spreads them evenly. More methods may be added in future.
tri-adaptive-quadtree = Adaptive (quadtree)
tri-axis-range = { $axis } range
tri-base-topology-will-receive-pit = The base topology that will receive the pit or stockpile shape.
tri-boundary-polyline = Boundary polyline
tri-bridge-gaps-help = Bridge gaps and boundary concavities narrower than this across the surface. 0 still bridges gaps up to roughly the sampling cell size; larger values fill bigger holes and erode boundary concavities.
tri-budget = Budget by
tri-cancel-pick = Cancel Pick
tri-candidate-detail = Candidate detail
tri-candidate-fine-cells-per-budgeted = Candidate fine cells per budgeted vertex. Higher gives the adaptive sampler more freedom to place detail, but is slower to build.
tri-cap-surface-share-source-points = Cap the surface by a share of the source points or by an exact vertex count.
tri-choose-input-clicking-loaded-surface = Choose this input by clicking a loaded surface in the viewport
tri-choose-which-side-reference-topology = Choose which side of the reference topology to remove from the surface within their shared XY area.
tri-clip = Clip
tri-clip-creates-new-triangulation-name = The clip creates a new triangulation with this name; the source surface is not modified.
tri-clip-surface-polyline = Clip Surface by Polyline
tri-closed-pit-stockpile-solid-whose = A closed pit or stockpile solid whose exposed boundary will be included in the result.
tri-cloud-carries-no-classifications-so = This cloud carries no classifications, so every point is surfaced. Import a LAS/LAZ file that has been through a ground filter to reconstruct bare earth.
tri-create-new-layer-contours-append = Create a new layer for the contours or append them to an existing layer in the active project.
tri-cut-topology-pit-shell = Cut Topology with Pit Shell
tri-e-g-design-trimmed = e.g. design_trimmed
tri-e-g-mysurf-cut = e.g. mysurf_cut
tri-e-g-mysurf-slice = e.g. mysurf_slice
tri-e-g-surface-contour = e.g. surface_contour
tri-e-g-topo-cut = e.g. topo_cut
tri-e-g-topo-pit = e.g. topo_with_pit
tri-exact-number-surface-vertices-target = Exact number of surface vertices to target. Very large values build slowly and use significant memory.
tri-existing-ground-topology-will-cut = The existing ground topology that will be cut by the pit shell.
tri-fill-holes-up = Fill holes up to
tri-generate = Generate
tri-generate-contour-lines = Generate Contour Lines
tri-generate-upper-surface = Generate Upper Surface
tri-ground-points-only = Ground points only
tri-hide-unload-sources = Hide and unload sources
tri-higher-edge-will-enforced-each = The higher edge will be enforced at each conflict. Lower conflicting segments will be ignored as breaklines and the surface will interpolate through those areas. The source polylines are unchanged.
tri-breaklines-cross = The highlighted breakline edges cross or overlap in plan at different elevations. One terrain surface cannot follow both.
tri-intervals-colours = Intervals & colours
tri-keep-clipped-topology-included-shape = Keep the clipped topology and included shape as separate triangulations instead of combining them into one entity.
tri-keep-inside-discards-surface-outside = Keep inside discards surface outside the polyline. Keep outside cuts a polyline-shaped hole from the surface.
tri-keeps-only-surface-within-polyline = Keeps only the surface within the polyline boundary.
tri-keep-surface-relation-help = Keeps the surface { $relation } the topology within its XY coverage.
tri-layer-already-exists-select-above = That layer already exists; select it above or choose another name.
tri-limit-z-range = Limit Z range
tri-major = Major
tri-max-edge-length = Max edge length
tri-merge = Merge
tri-method = Method
tri-min = Min
tri-minimum-maximum-elevations-retained = Minimum and maximum elevations retained in the output surface. The minimum must be below the maximum.
tri-minor = Minor
tri-contour-interval-help = Minor controls ordinary contours. Major controls emphasized contours and must use an interval at least as large as Minor.
tri-move-cursor-over-loaded-surface = Move the cursor over a loaded surface.
tri-slice-output-name-help = Name assigned to the elevation-clipped output surface.
tri-name-assigned-merged-topology-pit = Name assigned to the merged topology and pit/stockpile result.
tri-name-assigned-newly-created-contour = Name assigned to the newly created contour layer.
tri-reconstruct-output-name-help = Name assigned to the reconstructed triangulation.
tri-name-assigned-topology-after-pit = Name assigned to the topology after the pit shell is cut from it.
tri-name-assigned-trimmed-output-surface = Name assigned to the trimmed output surface.
tri-nearby-breakline-vertices-do-not = Nearby breakline vertices do not meet at exactly the same position, so the surface cannot be triangulated.
tri-new-layer = New layer
tri-new-layer-name = New layer name
tri-no-boundary-selected = No boundary selected
tri-no-point-cloud-selected = No point cloud selected
tri-no-surface-selected = No surface selected
tri-once-clip-succeeds-unload-source = Once the clip succeeds, unload the source surface so only the clipped result stays in the scene.
tri-once-cut-succeeds-unload-original = Once the cut succeeds, unload the original topology so only the cut result stays in the scene. The pit shell stays loaded.
tri-once-merge-succeeds-unload-source = Once the merge succeeds, unload the source topology and solid so only the merged result stays in the scene.
tri-once-slice-succeeds-unload-source = Once the slice succeeds, unload the source surface so only the sliced result stays in the scene.
tri-once-trim-succeeds-unload-surface = Once the trim succeeds, unload the surface that was trimmed so only the result stays in the scene. The topology stays loaded.
tri-only-loaded-pickable = Only loaded triangulations can be picked.
tri-operation = Operation
tri-output-layer = Output layer
tri-percentage = Percentage
tri-percentage-cloud = Percentage of cloud
tri-pick-from-view = Pick from View
tri-pit-design-surface-only-areas = The pit design surface. Only areas where it excavates below the topology are used for the cut.
tri-pit-shell = Pit shell
tri-pit-stockpile-solid = Pit/stockpile solid
tri-recommended-weld-retry = Recommended: Weld & Retry
tri-reconstruct-ground-only-help = Reconstruct from the points classified as bare earth, discarding vegetation, buildings, plant and noise. Turn this off to surface every point in the cloud.
tri-reconstruct-help = Reconstruct a triangulated terrain surface from a point cloud. The adaptive sampler spends the vertex budget where the ground is most complex and keeps planar areas sparse.
tri-reduce-budget-candidate-detail-if = Reduce the budget or candidate detail if your machine has less RAM.
tri-reference-topology-help = The reference topology that defines where the other surface is trimmed.
tri-reject-reconstructed-triangle-edges = Reject reconstructed triangle edges longer than this distance. Use 0 for no edge-length limit.
tri-remove-inside-help = Removes the surface within the polyline boundary and keeps the rest.
tri-removes-topology-where-pit-shell = Removes the topology where the pit shell excavates below it so the shell fills the hole. The seam follows the true 3D contact line between the surfaces; topology under parts of the shell that stand above the ground is kept.
tri-result = Result
tri-save-two-entities = Save as two entities
tri-select = Select…
tri-selected-closed-polyline-whose-xy = The selected closed polyline, whose XY boundary defines the clipping area.
tri-selected-point-cloud-whose-points = The selected point cloud, whose points will be reconstructed into a terrain surface. Close the dialog to reconstruct a different one.
tri-selected-surface-from-which-contour = The selected surface, from which contour lines will be generated. Close the dialog to contour a different one.
tri-selected-surface-which-will-clipped = The selected surface, which will be clipped. Close the dialog to clip a different one.
tri-slice-source-help = The selected surface, whose elevation range will be clipped. Close the dialog to slice a different one.
tri-share-source-points-keep-fractions = Share of source points to keep. Fractions such as 0.125% are allowed.
tri-slice-triangulation-z-range = Slice Triangulation by Z Range
tri-solution-generate-upper-surface = Solution: Generate Upper Surface
tri-surface-trim = Surface to Trim
tri-target-surface-help = The surface that will be changed; the selected topology is left intact.
common-percent-suffix = %
tri-topology = Topology
tri-triangulation-failed = Triangulation Failed
tri-trim = Trim
tri-trim-topology = Trim to Topology
tri-uniform-grid = Uniform grid
tri-unload-source-surface = Unload source surface
tri-unload-source-topology = Unload source topology
tri-up-target-point-count-points = Up to { $target } of { $point_count } points will become surface vertices ({ $percent }%).
tri-use-full-surface-elevation-range = Use the full surface elevation range
tri-vertex-count = Vertex count
tri-vertices-within-5-cm-xy = Vertices within 5 cm in XY and Z will share one position for this triangulation. This can shift the generated surface locally by up to 5 cm; the source polylines are unchanged.
tri-weld-retry = Weld & Retry
tri-when-enabled-generate-contours-only = When enabled, generate contours only between the specified minimum and maximum elevations.

## Ui strings

ui-choose-offset-side = Choose offset side
ui-choose-relimit-side = Choose relimit side
ui-click-circle-centre = Click the circle centre
ui-click-closed-polyline-use-blast = Click a closed polyline to use as the blast shape
ui-click-collar-add-edit-initiation = Click a collar to add or edit an initiation point
ui-click-first-point-slice-line = Click the first point of the slice line
ui-click-first-vertex = Click first vertex
ui-click-perimeter-point-type-radius = Click a perimeter point or type a radius
ui-click-second-point-slice-line = Click the second point of the slice line
ui-click-second-vertex = Click second vertex
ui-click-use-pointer-radius = or click to use the pointer radius
ui-could-not-copy-text-browser = Could not copy text to the browser clipboard: { $error }
ui-dip-horizontal-no-strike = { $dip } (horizontal, no strike)
ui-distance-meters = { $distance } meters
ui-drag-ring-type-azimuth-dip = Drag a ring, or type an azimuth and dip
ui-each-hole-turns-about-its = each hole turns about its own collar
ui-enter-positive-decimal-radius = Enter a positive decimal radius
ui-esc-cancels = Esc cancels
ui-no-delay-product-tie = No delay product to tie with
ui-press-enter-use-typed-radius = Press Enter to use the typed radius
ui-right-click-delay-palette-heading = right-click the Delay Palette heading to add one
ui-select-designs = Select designs
ui-select-drill-hole = Select a drill hole
ui-select-endpoint-join = Select the endpoint to join
ui-select-first-crest-toe-point = Select first crest/toe point
ui-select-item = Select an item
ui-select-line-fuse = Select a line to fuse
ui-select-line-polyline = Select a line or polyline
ui-select-line-relimit = Select line to relimit
ui-select-next-line-fuse = Select the next line to fuse
ui-select-opposite-berm-point = Select opposite berm point
ui-select-point = Select a point
ui-select-polyline = Select a polyline
ui-select-polyline-open-line = Select a polyline or open line
ui-select-polyline-vertex = Select a polyline vertex
ui-select-second-crest-toe-point = Select second crest/toe point
ui-select-second-split-point = Select second split point
ui-select-split-point = Select a split point
ui-select-topologies = Select topologies
ui-slice-view = Slice view
ui-strike-dip = { $strike }° strike · { $dip }
ui-value-dip = { $value }° dip

## Viewport strings

viewport-1-1-true-shape = 1:1, true shape
viewport-1-ratio = 1:{ $ratio }
viewport-all-total-categories-keep-their = All { $total } categories keep their colour; only the first { $shown } are drawn distinctly
viewport-axis-maximum = { $axis } maximum
viewport-axis-minimum = { $axis } minimum
viewport-azimuth-dip = Azimuth { $azimuth }, dip { $dip }
viewport-back-whole-log = Back to the whole log.
viewport-bar-blast-timeline-placeholder = Blast Timeline [PLACEHOLDER]
viewport-bar-burden-relief-heatmap-placeholder = Burden Relief Heatmap [PLACEHOLDER]
viewport-bar-cinematic-view = Cinematic View
viewport-bar-color = Color:
viewport-bar-contours-equal-time-placeholder = Contours of Equal Time [PLACEHOLDER]
viewport-bar-disable-cinematic-view = Disable Cinematic View
viewport-bar-disable-flying-mode = Disable Flying Mode
viewport-bar-disable-x-ray-vision = Disable X-Ray Vision
viewport-bar-drill-holes = Drill Holes:
viewport-bar-enable-flying-mode = Enable Flying Mode
viewport-bar-enable-x-ray-vision = Enable X-Ray Vision
viewport-bar-exit-slice-view = Exit Slice View
viewport-bar-fill = Fill:
viewport-bar-fix-centre-rotation = Fix Centre of Rotation
viewport-bar-hide-borehole-inspector = Hide Borehole Inspector
viewport-bar-hide-classification = Hide Classification
viewport-bar-hide-points = Hide Points
viewport-bar-hide-rl-grid = Hide RL Grid
viewport-bar-hide-wireframes = Hide Wireframes
viewport-bar-hide-xy-grid = Hide XY Grid
viewport-bar-release-centre-rotation = Release Centre of Rotation
viewport-bar-show-borehole-inspector = Show Borehole Inspector
viewport-bar-show-classification = Show Classification
viewport-bar-show-points = Show Points
viewport-bar-show-rl-grid = Show RL Grid
viewport-bar-show-wireframes = Show Wireframes
viewport-bar-show-xy-grid = Show XY Grid
viewport-bar-vertical-slice-view = Vertical Slice View
viewport-blank = (blank)
viewport-choose-active-block-model-variable = Choose the active block model variable
viewport-choose-variable = Choose a variable
viewport-click-edit-color-right-click = Click to edit color; right-click to remove
viewport-click-type-boundary-s-value = Click to type this boundary's value
viewport-colour-mapping = Colour mapping
viewport-count-categories = { $count } categories
viewport-count-category = { $count } category
viewport-depth-m-hole-end = { $depth } m hole end
viewport-double-click-add-boundary-here = Double-click to add a boundary here
viewport-drag-move-middle-click-toggles = Drag to move · Middle-click toggles ≤
viewport-drag-move-right-click-remove = Drag to move · Right-click to remove · Middle-click toggles ≤
viewport-drag-spin-view-around-hole = Drag to spin the view around the hole. Double-click to face north.
viewport-e = E
viewport-edit-category-colour = Edit this category colour
viewport-edit-colour-used-empty-values = Edit the colour used for empty values
viewport-empty = (empty)
viewport-empty-hidden = (empty · hidden)
viewport-filter-variables = Filter variables
viewport-fit-hole-track = Fit the hole to the track
viewport-from = { $from } to { $to }
viewport-from-m = { $from } to { $to } m
viewport-h-1-ratio = H 1:{ $ratio }
viewport-hole-has-no-trace-draw = This hole has no trace to draw.
viewport-interval-data = Interval data
viewport-intervals = Intervals
viewport-m-from-collar-toward-bearing = m from collar, toward { $bearing }°
viewport-navigation-hint = Middle-drag to pan · Scroll to zoom
viewport-navigation-hint-detach = Middle-drag to pan · Scroll to zoom · Click to detach
viewport-n = N
viewport-no-data-variable = No data for this variable
viewport-no-density-log-hole = No density log for this hole
viewport-no-downhole-geophysics-hole = No downhole geophysics for this hole
viewport-no-gamma-log-hole = No gamma log for this hole
viewport-no-matches = No matches
viewport-no-trace = No trace
viewport-no-usable-range = (no usable range)
viewport-not-logged = Not logged
viewport-orientation-source = Orientation source
viewport-rebuild-variable-s-colours-from = Rebuild this variable's colours from its data
viewport-reset = Reset
viewport-restore-full-model-range = Restore the full model range
viewport-roll-wheel-over-log-zoom = Roll the wheel over the log to zoom in on a seam. Drag the log to spin the hole and to walk down it.
viewport-s = S
viewport-sideways-scale = Sideways scale
viewport-squeeze-sideways-just-enough-keep = Squeeze sideways just enough to keep the hole in view. Never stretches.
viewport-trace-extent = Trace extent
viewport-w = W
viewport-widen-panel-show-density = Widen the panel to show density
viewport-widen-panel-show-density-gamma = Widen the panel to show density and gamma
viewport-widen-panel-show-gamma = Widen the panel to show gamma

## Charging dialogs

charging-edit-charge-product = Edit Charge Product
charging-new-charge-product = New Charge Product
charging-explosive-decks-add-mass-primed-stemming = Explosive decks add mass and are primed; stemming and air decks take length only.
charging-density = Density
charging-density-hint = In-hole density. Mass per metre is this times the hole's cross-section.
charging-another-product-already-has-name = Another product already has this name
charging-edit-charge-rule = Edit Charge Rule
charging-new-charge-rule = New Charge Rule
charging-decks-collar-toe = Decks, collar to toe
charging-priming = Priming
charging-preview = Preview
charging-preview-on-a = On a
charging-preview-hole-of = hole of
charging-preview-use-pattern = use the pattern's
charging-preview-active-pattern-median-hole = Preview on the active pattern's median hole
charging-fixed-decks-longer-than-hole = The fixed decks are longer than this hole
charging-mass-kg-explosive = { $mass } kg explosive
charging-rate-kg-m = { $rate } kg/m
charging-count-primer = { $count } primer(s)
charging-another-rule-already-has-name = Another rule already has this name
charging-save-reload-count-hole = Save and Reload { $count } Hole(s)
charging-length = Length
charging-rest-length-m = rest · { $length } m
charging-rest = rest
charging-deck-takes-whatever-length-fixed-decks = This deck takes whatever length the fixed decks leave. One deck per rule fills.
charging-remove-deck = Remove deck
charging-add-deck = Add Deck
charging-downhole-delay = Downhole delay
charging-hole-detonator-hole-fires-long-after = The in-hole detonator. A hole fires this long after its surface signal arrives.
charging-primer-height = Primer height
charging-how-far-above-base-each-explosive = How far above the base of each explosive deck its primer sits.
charging-booster = Booster
charging-cast-booster-mass-each-primer = Cast booster mass in each primer.
charging-count-rule-load-product-will-need = { $count } rule(s) load this product and will need another chosen.
charging-rule = Rule
charging-holes-already-loaded-keep-their-charge = Holes already loaded with it keep their charge.

## Blast review overlays

blast-burden-relief = Burden relief
blast-ms-per-metre-last-neighbour-fire = ms per metre to the last neighbour to fire
blast-below-hole-fires-before-rock-front = Below this a hole fires before the rock in front of it has moved: tight.
blast-above-rock-front-has-long-gone = Above this the rock in front has long gone: slack, with cut-off and flyrock risk.
blast-tight = tight
blast-good = good
blast-slack = slack
blast-free-face = free face
blast-fires-at = Fires at
blast-empty-won-t-detonate = empty, won't detonate
blast-not-reached = not reached
blast-value-ms-m-from-hole = { $value } ms/m from { $hole }
blast-fires-first-free-face = fires first: free face
blast-relief = Relief
blast-explosive = Explosive
blast-powder-factor = Powder factor
blast-not-loaded = Not loaded
blast-count-primer-delay-ms-downhole = { $count } primer(s) · { $delay } ms downhole
blast-set-initiation-point-tie-holes-play = Set an initiation point and tie the holes in to play the round
blast-pause = Pause
blast-play = Play
blast-back-start = Back to the start
blast-duration-ms = of { $duration } ms
blast-real-time = Real time
blast-mic-limit = MIC limit
blast-most-explosive-allowed-detonate-any-8 = The most explosive allowed to detonate in any 8 ms at this site. Windows over it are flagged.
blast-no-holes-loaded-surface-signal-plays = No holes are loaded: the surface signal plays, but nothing detonates. Load holes with the Charge Holes tool.
blast-now-holes-hole = Now: { $holes } hole(s)
blast-in-8-ms = in 8 ms
blast-peak-mass-kg-time-ms = Peak { $mass } kg at { $time } ms
blast-peak-holes-hole-time-ms = Peak { $holes } hole(s) at { $time } ms
blast-peak-over-limit = , { $over } kg over
blast-peak-within-limit = , within limit
blast-top-surface-signal-lighting-each-downline = Top: the surface signal lighting each downline. Below: detonations. Click or drag to move the playhead.

## Products palette and charge rules

products-charge-rules = Charge Rules
products-new-rule = New Rule
products-charge-products = Charge Products
products-new-rule-default-name = New rule
products-no-rules = No rules
products-load-selected-holes-count = Load Selected Holes ({ $count })
products-unload-selected-holes-count = Unload Selected Holes ({ $count })
products-edit-rule = Edit Rule
products-duplicate-rule = Duplicate Rule
products-delete-rule = Delete Rule
products-fill-product = fill  { $product }
products-primer-offset-m-off-each-explosive = Primer { $offset } m off each explosive deck's base, { $booster } kg booster, { $delay } ms downhole
products-double-click-edit = Double-click to edit
products-edit-product = Edit Product

## Charging activity log

blast-log-updated-charge-product-name = Updated charge product { $name }
blast-log-added-charge-product-name = Added charge product { $name }
blast-log-updated-charge-rule-name = Updated charge rule { $name }
blast-log-added-charge-rule-name = Added charge rule { $name }
blast-log-entry-no-longer-charge-library = That entry is no longer in the charge library
blast-log-deleted-name-from-charge-library = Deleted { $name } from the charge library
blast-log-failed-save-charge-library-error = Failed to save the charge library: { $error }
blast-log-cannot-load-rule-problem = Cannot load with this rule: { $problem }
blast-log-count-hole-too-short-fixed-decks = { $count } hole(s) are too short for the fixed decks of this rule and were left as they were
blast-log-count-hole-have-no-depth-load = { $count } hole(s) have no depth to load
blast-log-count-loaded-hole-have-no-diameter = { $count } loaded hole(s) have no diameter, so their explosive mass is unknown
common-charge-holes = Charge Holes
blast-log-loaded-count-hole-rule = Loaded { $count } hole(s) with { $rule }
blast-log-unload-holes = Unload Holes
blast-log-unloaded-count-hole = Unloaded { $count } hole(s)
blast-log-select-holes-active-pattern-first = Select holes of the active pattern first
blast-log-rule-no-longer-charge-library = That rule is no longer in the charge library
blast-log-there-no-charge-rule-load-add = There is no charge rule to load with: add one in the products panel

## Charge rule problems

blast-rule-stemming = Stemming
blast-rule-air-deck = Air deck
blast-rule-give-rule-name = Give the rule a name
blast-rule-add-least-one-deck = Add at least one deck
blast-rule-only-one-deck-can-fill-rest = Only one deck can fill the rest of the hole
blast-rule-deck-lengths-must-greater-than-zero = Deck lengths must be greater than zero
blast-rule-no-product-named-name = No product named '{ $name }'
blast-rule-rule-needs-least-one-explosive-deck = A rule needs at least one explosive deck

## Console reports (Drill & Blast)

state-save-charge-product = Save Charge Product
state-save-charge-rule = Save Charge Rule
state-delete-charge-library-entry = Delete Charge Library Entry

## Viewport messages (Drill & Blast)

ui-click-drag-over-holes-load-them = Click or drag over holes to load them with { $rule }
ui-hold-shift-unload = hold Shift to unload
ui-no-charge-rule-load = No charge rule to load with
ui-right-click-charge-rules-heading-add = right-click the Charge Rules heading to add one

## OMF warnings (Drill & Blast)

omf-element-name-has-count-charge-naming = Element '{ $name }' has { $count } charge(s) naming holes it no longer contains
