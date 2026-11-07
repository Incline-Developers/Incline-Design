# Incline: English message catalog (canonical source).
#
# Every `tr!(...)` call in the code is checked against THIS file at compile time:
# an unknown id or a missing argument fails the build. Other languages
# (`i18n/<lang>/incline_design.ftl`) may be incomplete and fall back here.
#
# Ids are kebab-case, grouped by area with a prefix (`menu-`, `settings-`,
# `tri-`, `common-`, ...). Keep this file grouped and roughly sorted.

## Shared

common-cancel = Cancel
common-currency-symbol = $
common-clear = Clear
common-close = Close
common-fill = Fill
common-set = Set

## Status bar

# Title of the status bar's language menu. The languages themselves are never
# translated: each names itself in its own script, from `LanguageChoice`.
status-fps = { $fps } fps
status-cpu = cpu
status-memory = memory
status-language = Language

## Menu bar: File

menu-file = File
menu-file-save-project = Save Project
menu-file-save-project-as = Save Project As...
menu-file-new-project = New Project...
menu-file-open-project = Open Project...
menu-file-open-recent = Open Recent
menu-file-recent-project-missing = Not found: { $path }
menu-file-show-in-explorer = Show in Explorer
menu-file-show-in-folder = Open Containing Folder
menu-file-import = Import...
menu-file-export = Export...
menu-file-export-viewport-image = Export Viewport Image...
menu-file-about = About { $app }...
menu-file-exit = Exit Application

## Menu bar: View

menu-view = View

## Workspaces

ws-production = Production
ws-drill-and-blast = Drill & Blast
ws-geology = Geology
ws-planning = Planning
planning-page-setup = Setup
planning-page-schedule = Schedule

## Menubars

ws-menubar-design = Design
ws-menubar-triangulation = Triangulation
ws-menubar-raster = Raster
ws-menubar-point-cloud = Point Cloud
ws-menubar-block-model = Block Model
ws-menubar-drillholes = Drillholes
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
dialog-delete-rows-confirm =
    Delete { $count } items from the project?
dialog-delete-collection-confirm =
    Delete collection '{ $name }' and everything in it from the project?
confirm-delete-product =
    Delete product '{ $name }' from the palette?
    This cannot be undone.

## Create Triangulation dialog

tri-create-title = Create Triangulation
tri-create-type-label = Triangulation type
tri-create-type-help =
    Open surface creates a terrain-style sheet. Solid creates a fully enclosed
    mesh and requires input that can form a watertight boundary.
tri-create-output-name = Output name
tri-create-output-name-hint = triangulation name
tri-create-run = Triangulate

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
        [one] { $count } polyline
       *[other] { $count } polylines
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

## Dynamic UI messages

ui-selected-count = { $count } selected
ui-selected-objects = { $count } object(s) selected
ui-selected-polylines = { $count } polyline(s) selected
pick-selection-done = Done
pick-selection-how = Click to add or drop one, or drag a box to add several.
pick-selection-objects-prompt = Select the objects to move.
pick-selection-polylines-prompt = Select the polylines to insert points on.
pick-selection-crossing-prompt = Select two or more polylines that cross.
pick-selection-edges-prompt = Select the polylines, circles and points to triangulate.
pick-selection-holes-prompt = Select the drillholes to place reference points on.
pick-selection-surface-prompt = Select the points to build from, with any open polylines the surface must pass through and one closed polyline to clip it to.
pick-selection-holes-selected = { $count } hole(s) selected
pick-selection-points-selected = { $count } point(s) selected
pick-selection-objects-help = The objects that are moved. Pick selects them in the viewport.
pick-selection-polylines-help = The polylines the points are inserted on. Pick selects them in the viewport.
pick-selection-edges-help = The objects that are triangulated. Pick selects them in the viewport.
tri-create-objects-help = Triangulates the objects below into one surface.
reference-points-no-holes = No holes selected
reference-points-holes-help = The holes the points are placed on. Pick selects them in the viewport.
reference-surface-no-points = No points selected
reference-surface-count-closed-polylines = { $count } closed polylines
reference-surface-points-help = The points the surface is built from. Pick selects them in the viewport, along with the controls and the extent.
reference-surface-controls-help = The open polylines the surface is made to pass through, picked with the points.
ui-invalid-axis-value = Enter a valid { $axis } value.
ui-selection-spans = Selection spans { $min } to { $max }.
confirm-delete-count = Are you sure you want to delete { $count } selected item(s)?
confirm-delete-layer = Delete layer '{ $name }' and all objects on it?
    This cannot be undone.
tri-estimated-memory = Estimated peak memory ~{ $estimate }. { $detail }
block-grid-summary = Grid: { $x } × { $y } × { $z } = { $count } blocks
status-selected = Selected: { $count }
status-faces = Faces: { $drawn } / { $total } ({ $drawn_chunks }/{ $total_chunks } chunks)
status-clip = Clip near/far/Δ: { $near } / { $far } / { $delta } m
status-points = Points: { $drawn } / { $target } of { $total } ({ $drawn_chunks }/{ $total_chunks } chunks)

explorer-no-rasters = No rasters
slice-viewport-gestures = middle-drag pan · right-drag orbit · Shift+wheel walk · W/S step section · Q/E rotate · Esc exit

## Startup environment details

## Renderer startup diagnostics

## Edit Object dialog

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Index
color-rgb = RGB

asset-loading = Loading asset data
asset-unloading = Unloading asset data
asset-load-failed = Could not load asset data
asset-unload-failed = Could not unload asset data

# Planning → Set Up. Shared labels; Schedule's own are under "Schedule setup"
# below, Haulage's under the haul- keys.
planning-configuration = Configuration
schedule-general = General
planning-dumps = Dumps
planning-stockpiles = Stockpiles
planning-name = Name
planning-new-dump = New Dump
planning-new-stockpile = New Stockpile
planning-properties = Properties
planning-property = Property
planning-step = Step
planning-status = Status
planning-value = Value
planning-schedule-name = Schedule Name
planning-page-solids = Solids
planning-page-haulage = Haulage
planning-view-reserves = Reserves
planning-subpage-animate = Animate
planning-subpage-calendar = Calendar

## Solids Reserves setup

planning-field-list = Field List
planning-block-models = Block Models
planning-solids = Solids
planning-benching = Benching
planning-blasting = Blasting
preferences-title = Preferences

context-text-colour = Text colour

context-polylines = Polylines
context-points = Points
planning-building-slabs = Building bench slabs…
planning-too-many-flitches = A bench can contain at most 64 flitches
planning-solid-needs-surfaces = Load or assign both the design surface and topography to inspect this solid
planning-building-view = Building solids inspector…
planning-solid-geometry-pending = Preparing occupied benches…
planning-empty-category = (Empty)
planning-computing-reserves = Computing reserves…
planning-mapping-per-volume = { $column } × block m³
planning-no-fields = No fields yet
planning-model-columns = Block Model Columns
planning-add-column = Add to the Field List
planning-average-by = Average by { $field }
planning-combines-as = Combines As
planning-no-solids = No solids yet
triangulation-picker-loaded-only = Only loaded surfaces are listed. Load one in the explorer to offer it here.
planning-reserve-needs-model = Assign a block model to compute reserve fields
planning-reserve-open-piece = { $solid }: the piece from RL { $base } to { $top } is open along { $count } edge(s), { $length } long in all; the longest is near { $x }, { $y }, RL { $z }. Its volume cannot be measured, so reserves cannot be taken from it.
planning-reserve-open-solid = Reserve fields need a closed solid, and this one is open: its surfaces do not meet all the way round. Rebuild it from a design that meets the topography everywhere; Build Solid from Surfaces reports how many edges are open.
planning-reserve-method = Geometric overlap fractions · uniform values within blocks · totals added per solid
planning-reserve-blocks = Equivalent blocks
planning-reserve-status = Reserves
planning-reserve-scope = Method
planning-reserve-categories =
    { $count ->
        [one] { $count } category
       *[other] { $count } categories
    }
planning-reserve-unavailable = Block model data is unavailable; reload the model to compute reserves
planning-volume-unavailable = Preview available · volume requires closed meshes

## Schedule setup: the loader fleet, and the Gantt it draws rows for

schedule-loader-classes = Machine Classes
schedule-loader-agents = Machines
schedule-class = Class
schedule-dig-rate = Default dig rate (t/h)
schedule-effective-rate = Class rate
schedule-tph = t/h
schedule-new-class = New Machine Class
schedule-new-agent = New Machine
schedule-delete-agent-title = Delete { $name }?
schedule-delete-agent-bars = { $count ->
    [one] Its bar moves to Unassigned and won't be scheduled until it's given a machine.
   *[other] Its { $count } bars move to Unassigned and won't be scheduled until they're given a machine.
}
schedule-delete-agent-follows = { $count ->
    [one] One follow bar loses its leader.
   *[other] { $count } follow bars lose their leader.
}
schedule-delete-class = Delete Class
schedule-delete-agent = Delete Machine
schedule-add-class = Add Class
schedule-add-agent = Add Machine
schedule-no-classes = Add a machine class before adding machines
schedule-no-agents = No machines yet
schedule-no-class-list = No machine classes yet
schedule-select-class = Select a machine class from the list
schedule-select-agent = Select a machine from the list
schedule-rate-not-a-number = Enter a rate greater than zero
schedule-loader-class-default = Machine Class
schedule-loader-agent-default = Loader
schedule-error-empty-name = Enter a name
schedule-error-duplicate-name = ⁨{ $name }⁩ is already in use
schedule-error-invalid-rate = The rate must be a finite number greater than zero
schedule-calendar-invalid-percentage = Availability and utilisation must be between 0 and 100%
schedule-calendar-invalid-incentive = The utilisation incentive must be a number that is not negative
schedule-calendar-empty-override = Empty calendar overrides must not be stored
schedule-calendar-read-only = The class default rate is edited in Schedule Setup
schedule-calendar-duplicate-cell = A calendar edit contains the same cell more than once
schedule-calendar-period-overflow = The calendar period is too large
schedule-calendar-effective-rate = The effective production rate is too small or large to represent
schedule-calendar-edit = Edit loader calendar
schedule-truck-calendar-edit = Edit truck calendar
schedule-calendar-cells-updated =
    { $count ->
        [one] 1 cell updated
       *[other] { $count } cells updated
    }
schedule-calendar-setting = Setting
schedule-calendar-default = Default
schedule-calendar-total = Total
report-export = Export report
report-export-title = Schedule report
report-export-help = The calculated schedule as tables - movements, loader time, stockpiles, crushers, dumps and trucks - grouped by day, week or the whole schedule, to copy into a spreadsheet or save as CSV.
report-export-unavailable = Calculate the schedule first.
report-group-by = Group rows by
report-group-day = Day
report-group-week = Week
report-group-whole = Whole schedule
report-copy = Copy tables
report-copy-help = Copies the tables as tab-separated text, which a spreadsheet pastes straight into columns.
report-save = Save as CSV…
report-copied = Schedule report copied to the clipboard.
report-saved = Schedule report saved to { $path }
report-file-name = Schedule report - { $grouping }.csv
report-heading = Schedule report, grouped by { $grouping }, calculated to { $end }
report-day = Day { $day }
report-week = Week { $week } (days { $first }-{ $last })
report-days = Days { $first }-{ $last }
report-period = Period
report-movements = Movements
report-loaders = Loader time
report-stockpiles = Stockpiles
report-crushers = Crushers
report-dumps = Dumps
report-trucks = Trucks
report-loader = Loader
report-activity = Activity
report-dig = Dig
report-reclaim = Reclaim
report-source = Source
report-destination = Destination
report-tonnes = Tonnes (t)
report-grade = { $grade }
report-closing-grade = Closing { $grade }
report-value = Movement value ({ $currency })
report-stockpile = Stockpile
report-opening-t = Opening (t)
report-received-t = Received (t)
report-reclaimed-t = Reclaimed (t)
report-closing-t = Closing (t)
report-crusher = Crusher
report-processed-t = Processed (t)
report-limit-t = Limit (t)
report-penalty = Grade target penalty ({ $currency })
report-dump = Dump
report-to-date-t = Received to date (t)
report-truck-class = Truck class
report-truck-hours-used = Truck-hours used
report-truck-hours-available = Truck-hours available
report-dug-t = Mined (t)
report-dig-h = Digging (h)
report-reclaim-h = Reclaiming (h)
report-delay-h = Delayed (h)
report-idle-h = Idle (h)
report-idle-reason-h = Idle: { $reason } (h)
schedule-calendar-total-help = The whole calculated schedule: tonnes, truck-hours and value summed; closing stock and deposited tonnes at its end; grades weighted by tonnes.
schedule-calendar-day = Day { $day }
schedule-calendar-hours = { $start }–{ $end } h
schedule-calendar-weekday-date = { $weekday } { $date }
weekday-short-mon = Mon
weekday-short-tue = Tue
weekday-short-wed = Wed
weekday-short-thu = Thu
weekday-short-fri = Fri
weekday-short-sat = Sat
weekday-short-sun = Sun
schedule-calendar-availability = Availability (%)
schedule-calendar-utilisation = Utilisation (%)
schedule-calendar-empty = Add a loader to author its production calendar.
schedule-calendar-add-loader = Add loader
schedule-calendar-loader-default = loader default
schedule-calendar-class-source = class default
schedule-calendar-explicit = explicit override
schedule-calendar-class-default = { $value } · class { $class }; edited in Setup
schedule-calendar-resolved = { $value } · { $source }
schedule-calendar-invalid = This loader calendar is invalid
schedule-calendar-invalid-number = Enter a number
schedule-calendar-paste-outside = The pasted rectangle extends outside the loader grid
schedule-calendar-paste-read-only = The pasted rectangle includes a read-only class rate
schedule-calendar-calculated-selection = Selection includes calculated cells
schedule-calendar-tonnes-partial = Calculated through { $hours } h · day partially covered
schedule-error-unknown-class = That loader class is no longer in this project
schedule-error-unknown-agent = That machine is no longer in this project
schedule-error-class-in-use = Still assigned to ⁨{ $agents }⁩. Reassign or delete those machines first
schedule-error-ids-exhausted = This project cannot hold any more loader classes or agents
schedule-error-duplicate-id = Two entries share one identity
schedule-stale-edit = That schedule edit was discarded: it was made in a project that is no longer open

## Schedule → Sequences

schedule-new-bar = New Bar
schedule-rename-bar = Rename Bar
schedule-rename-bar-action = Rename…
schedule-copy-bar = Copy Bar
schedule-delete-bar = Delete Bar

# Used to name a copied bar, so it becomes project data: no bidi isolation
# marks here, which would be stored in the name itself.
schedule-bar-copy-name = { $name } copy
schedule-bar-edit-sequence = Edit Sequence…
schedule-bar-raise-priority = Raise Priority
schedule-bar-lower-priority = Lower Priority
schedule-bar-unassign = Unassign
schedule-bar-unassigned = Unassigned
schedule-bar-unassigned-note = Bars with no machine. Drag one onto a loader row, or use its Assign To menu
schedule-bar-lane = Lane ⁨{ $lane }⁩
schedule-bar-execution-note = The marker is the authored earliest start; the coloured spans are calculated execution.
schedule-dispatch-generation-changed = The Solids run changed while the schedule was being measured. Recalculate against the current run
schedule-dispatch-idle = Idle
schedule-dispatch-execution = Digging

# Schedule Setup pipeline
schedule-readiness-step = Scheduling Readiness
schedule-step-block-tonnes = Blocks
schedule-step-road-access = Road Access
schedule-readiness-blocks = Dig blocks available
schedule-readiness-calculation = Calculation
schedule-readiness-ready = Ready
schedule-readiness-not-ready = Not ready yet
schedule-readiness-open-gantt = Open the Gantt
schedule-readiness-tonnage-field = Tonnage field
schedule-stage-no-tonnage-field = Choose the reserve field read as tonnes in Configuration
schedule-stage-tonnage-field-not-summed = This field is not summed, so it cannot be read as tonnes
schedule-stage-no-classes = Add a loader class before scheduling
schedule-stage-no-agents = Add a loader on the Machines step before scheduling
schedule-stage-agent-no-class = This machine's loader class is no longer in this project
schedule-stage-waiting-solids = Waiting on the Solids pipeline: { $reason }
schedule-stage-duplicate-solid = { $solid } is cut from the same design, topography and block model as { $other }, so both would mine the same ground. Delete one, or change its inputs.
schedule-stage-blocks-unmeasured = { $count } dig blocks have no tonnage on the chosen field. Fix the block models, exclude the ground, or count unreserved blocks as 0 t.
schedule-stage-blocks-negative-tonnes = { $count } dig blocks hold negative values on the tonnage field, such as -99 for a blank. Those values are counted as 0 t.
schedule-stage-blocks-negative-grade = { $count } dig blocks hold negative values of { $grade }, such as -99 for a blank. Those values are counted as a zero grade.
schedule-not-ready-not-run = { $step } has not been run
schedule-not-ready-stale = { $step } is out of date; run Schedule Setup again
schedule-not-ready-running = { $step } is still running
schedule-not-ready-failed = { $step }: { $message }
schedule-run-stopped-by-edit = Schedule Setup run stopped: the { $step } step's inputs changed while it was running
schedule-result-superseded = A Schedule Setup result arrived from a run that had already been superseded, and was discarded
gantt-row-resize-help = Drag to make every row taller or shorter. Double-click to reset.
schedule-tonnage-field = Tonnage field
schedule-tonnage-field-none = Not chosen
schedule-tonnage-field-no-fields = This project has no reserve fields yet. Add them in Setup → Solids → Field List.
schedule-error-unknown-bar = That bar is no longer in this project
schedule-error-unknown-member = That position is no longer in this bar's dig order
schedule-error-duplicate-member = That dig block is already in this bar's dig order
schedule-error-invalid-window = A work window needs a start at or after the schedule origin and an end after it
schedule-error-lane-overlap = That would overlap another bar in the same lane. Bars in one lane cannot overlap: drop it on the edge between lanes to give it a lane of its own.
schedule-error-invalid-bar-height = Bar height must be a number from 20 to 160 pixels.

# Destinations and routing: where calculated production goes.
# Capacity is always in the schedule's nominated tonnes field. Blank means
# unlimited; zero is a real capacity that can receive nothing.
destination-stage-retained = { $count ->
        [one] 1 capacity is kept for a solid that is no longer a stockpile or dump
       *[other] { $count } capacities are kept for solids that are no longer stockpiles or dumps
    }
destination-stage-no-rules = No enabled rule has a destination yet
destination-stage-rule-loader-missing = Names a loader that is no longer in the fleet
destination-stage-field-missing = Names a field that is no longer in the Field List
destination-stage-field-kind = ⁨{ $field }⁩ no longer aggregates the way this condition reads it
destination-unlimited = Unlimited
destination-select = Select a destination
destination-select-rule = Select a rule
destination-edit-in-solids = Renamed and deleted in Solids
destination-linked-solid = Name and type
destination-linked-note = From the solid, edited in Solids
destination-type = Type
destination-type-fixed = Type: { $kind }
destination-capacity = Maximum tonnes
destination-daily-limit = Maximum tonnes per day
destination-new-crusher = New Crusher
destination-default-stockpile = Stockpile
destination-default-dump = Dump
destination-default-crusher = Crusher
destination-no-stockpiles = No stockpiles. Draw one in Solids, or add one here.
destination-rule = Rule
destination-rule-order = Order
destination-rule-default = Rule
destination-rule-enabled = Enabled
destination-rule-target = Delivers to
destination-rule-disabled = Disabled: this rule takes no part in routing
destination-rule-loaders = Loaders
destination-rule-all-loaders = All loaders
destination-rule-none = None chosen
destination-rule-no-target = No destination
destination-rule-no-target-warning = This rule has no destination, so it moves nothing.
destination-rule-no-loaders = No loaders in the fleet yet.
destination-no-destinations = No destinations yet.
destination-select-all = Select all
destination-rule-sources = Sources
destination-rule-all-sources = All sources
destination-rule-conditions = Conditions
destination-no-rules = No rules. Every rule names one destination and the material allowed to reach it.
destination-no-sources = The last Solids run produced no ground to choose from.
destination-source-unplaced = One source is not in the last Solids run
destination-no-conditions = No conditions: any material this rule's loaders dig from its sources. Right-click to add one.
destination-unresolved = Unresolved destination
destination-add-condition = Add Condition…
destination-edit-condition = Edit Condition…
destination-delete-condition = Delete Condition
destination-condition = Condition
destination-condition-field = Field
destination-condition-pick-field = Choose a field
destination-condition-no-values = No values measured for this field yet
destination-condition-absent = not measured
destination-condition-values = Values
destination-condition-range = Range
destination-condition-open = Open
destination-condition-lower-inclusive = Lower bound is inclusive
destination-condition-upper-inclusive = Upper bound is inclusive
destination-condition-lower = Lower bound
destination-condition-upper = Upper bound

# Said on demand rather than on the page: what a condition is compared against
# is the contributing block-model row's own value, never the dig block's average
# of it and never the tonnes that row contributed.
destination-condition-note-sum = Compared against the contributing block's own mapped value, not the tonnes it contributes
destination-condition-note-average = Compared against the contributing block's own value, not the dig block's weighted average
destination-condition-note-category = Matches when the contributing block's mapped value is one of the chosen values
destination-move-rule-up = Move Up
destination-move-rule-down = Move Down
common-apply = Apply

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
survey-system-origin = Same point (system coordinates)
survey-angle-help = Counterclockwise from reference X toward reference Y, viewed from above.
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
survey-registry-hint = Name or EPSG code
survey-parent = Defined against
survey-parent-origin = Known point (parent coordinates)
survey-pick-registry = Search for the system and choose it from the results.
survey-pick-parent = Choose the system this grid is defined against.
survey-choose-system = Choose a system
survey-no-system = No system chosen
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
preferences-recent-functions-shown = Recent functions shown

relimit-select-boundary = Select the polyline or circle to trim or extend to

relimit-click-boundary = Click the polyline or circle to intersect with…

relimit-mode-help = Intersect moves one endpoint to a polyline or circle. Absolute sets the final line length. Relative adds or subtracts length.

browser-graphics-device-lost = The browser lost its graphics device. Reopen this page in a new tab. GPU details: { $message }

# Soft grade targets
grade-target-invalid = Use a finite, non-negative target and penalty; lower must be below target and upper above it. Set at least one limit and an outside multiplier of at least 1.
grade-target-missing-destination = A grade target names a missing destination. Remove the target or restore the destination.
grade-target-lower = Lower limit
grade-target-value = Target
grade-target-upper = Upper limit
grade-target-content-penalty = Content penalty
grade-calendar-penalty-row = Penalty ({ $currency }/t)
grade-calendar-none = None
grade-calendar-invalid = Enter a finite, non-negative grade or penalty per tonne.
grade-calendar-expand-help = Grade received each day. Click to show its target, limits and penalty.
grade-calendar-input-help = Resolved: { $value }. Blank inherits Default; None clears it for the day. Penalty is per tonne at a limit and doubles beyond it.
grade-calendar-actual-penalty = Penalty { $currency }{ $penalty } ({ $rate }/t)
grade-calendar-actual-band = Limits { $lower } – { $upper }, target { $target }
experiment-grade-units-help = Track a grade to blend it, target it and report it. Its stored numbers are used as they are, so enter targets on the same scale as the data.

# Haul roads and complete truck cycles
haul-roads = Haul roads
haul-road = Road
haul-node = Node
haul-invalid-position = Enter a finite road position.
haul-missing-road = This road or shape point no longer exists.
haul-survey-selection = Select design geometry for coordinate transformation; haul roads are edited in Haulage.
haul-new-stockpile = New stockpile here
haul-new-dump = New dump here
haul-new-crusher = New crusher here
haul-draw = Draw road
haul-finish = Finish road
haul-draw-help = Click points in the viewport; each becomes a road node. They sit at the Z level, or on what a snap mode finds. Click a road or node to join it. Backspace removes the last point; Enter ends the road and starts another; Escape or a double-click finishes.
haul-convert = Convert selection to roads
haul-import = Import DXF as roads…
haul-import-heading = Import DXF as Haul Roads
haul-import-done =
    { $added ->
        [one] Imported 1 road
       *[other] Imported { $added } roads
    }{ $skipped ->
        [0] .
        [one] ; 1 was already on the network.
       *[other] ; { $skipped } were already on the network.
    }
haul-import-nothing =
    { $total ->
        [0] The file has no polylines to import as roads.
        [one] Its 1 polyline is already on the network.
       *[other] All { $total } polylines are already on the network.
    }
haul-export = Export roads as DXF…
haul-dead-end = Dead end
haul-near-miss = Near miss
haul-separate-piece = Separate piece
haul-steep = Too steep
haul-missing-destination = Missing destination
haul-join = Join tolerance
haul-setting = Setting
haul-stage-settings = A road network setting is not a positive number.
haul-join-help = Road ends this close join into one node.
haul-delete-help = Delete key. Roads left without a road at an end lose that end too.
haul-delete-road = Delete road
haul-dead-end-help = A road end with no other road and no destination. Fine at a pit floor; otherwise the road may be meant to join another.
haul-near-miss-help = A road end close to another road but not joined to it. Trucks cannot pass between them.
haul-separate-piece-help = Roads not connected to the main network. Destinations on the main network cannot be reached from them.
haul-steep-help = Part of this road is steeper than a truck class's maximum grade. It is still used; check the design or the class setting.
haul-missing-destination-help = This node is a point for a destination that no longer exists.
haul-use-selected-node = The selected node
haul-from = From
haul-to = To
haul-from-pile = Reclaim from { $pile }
haul-need-truck = Add a truck class in Setup → Truck Classes to check routes.
haul-need-loader = Add a loader on Setup's Machines step to check routes.
haul-match-help = Theoretical matching: trucks are assumed never to queue at the loader or the destination.
haul-profile-legend = elevation · speed
haul-profile-hover = { $distance } m · { $elevation } m RL · { $speed } km/h
haul-unconnected-note =
    { $areas ->
        [one] 1 dig block has
       *[other] { $areas } dig blocks have
    } no road node within the auto-join distance. Their trucks are assumed to drive straight to the nearest node (up to { $longest } m), never steeper than the truck's maximum grade. Setup → Road Network shows these blocks in red.
haul-lengthened = Reaching the road means a { $rise } m change in height, so the drive from the block is taken as { $length } m at the maximum grade.
haul-drag-hint = drag to move
haul-spot-min = Spot (min)
haul-load-min = Load (min)
haul-loaded-min = Haul (min)
haul-dump-min = Dump (min)
haul-return-min = Return (min)
haul-dump-method = Trucks reach it by

## Drill and blast
machine-kind-loader = Loader
machine-kind-dozer = Dozer
machine-kind-drill = Drill
machine-kind-mpu = MPU
machine-group-loader = Loaders
machine-group-dozer = Dozers
machine-group-drill = Drills
machine-group-mpu = MPUs
stage-state-empty = Nothing set up yet
stage-empty-hint = This step ran, but there was nothing for it to work on.
calendar-rows = Rows
calendar-view = View
calendar-view-everything = Everything
calendar-view-default-name = View
calendar-view-name = Name
calendar-view-save = Save as a view
calendar-view-delete = Delete view
calendar-view-unsaved = { $view } (changed)
calendar-rows-section-rows = Rows
calendar-rows-of = { $kind } rows
calendar-rows-grades = Grades
calendar-rows-value = Schedule value
charts-trucks-scale = { $trucks } trucks
blast-activity-prep = Prep
blast-activity-drill = Drill
blast-activity-charge = Charge
blast-stage-not-started = Not started
blast-stage-prepped = Prepped
blast-stage-drilled = Drilled
blast-stage-charged = Charged
blast-stage-fired = Fired
drill-blast-error-pattern = Burden and spacing must be greater than zero, and subdrill zero or more.
drill-blast-error-hole = Hole diameter and product density must be greater than zero, and stemming zero or more.
drill-blast-error-buffer = The buffer must be zero or more metres.
drill-blast-error-window = The blast window must start before it ends, within the day.
drill-blast-error-reference = A blast reference could not describe any blast.
schedule-error-wrong-machine = That machine cannot do this work: loaders dig and reclaim, dozers, drills and MPUs work blasts.
schedule-error-kind-in-use = Machines of this class hold bars the new kind cannot work: { $bars }
drill-blast-unclearable = Never clear: ground above these blasts, within the buffer, is in no dig bar: { $blasts }
drill-blast-unclearable-more = { $blasts } and { $more } more
drill-blast-missing = { $bar }: { $count } of its blasts could not be found in the current run
drill-blast-unworked-entry = { $blast } ({ $step })
schedule-bar-unworked-blasts = { $count ->
    [one] Its ground in { $blasts } is never dug: no machine bar works that step. Add the blast to a blast bar on that kind of machine, or start it further on in Setup → Drill & Blast.
   *[other] Its ground in { $blasts } is never dug: no machine bar works those steps. Add the blasts to blast bars on those kinds of machine, or start them further on in Setup → Drill & Blast.
}
idle-waiting-on-blast = Waiting on blast
idle-waiting-on-blast-note = The bar's next block is in a blast that has not fired yet.
idle-waiting-on-blast-named = Waiting on blast { $blast }
blast-hold-never-clears = never clears
blast-hold-never-clears-note = Ground above this blast, within the clearance buffer, is in no dig bar, so it is never dug and the blast never clears. Add that ground to a dig bar, or set this blast's starting stage in Setup → Drill & Blast.
blast-hold-above = ground above not dug
blast-hold-above-note = Ground above this blast, within the clearance buffer, is still being dug.
blast-hold-above-named = Ground above this blast, within the clearance buffer, is still being dug: { $blasts }
blast-hold-no-machine = no { $step } machine
blast-hold-no-machine-note = No { $step } machine has a bar that works this blast. Add it to a blast bar on a { $step } machine, or set its starting stage in Setup → Drill & Blast.
blast-hold-working = { $step } under way
blast-hold-queued = waiting for a { $step } machine
blast-hold-queued-note = Ready for { $step }, but the machines whose bars work it are busy on other blasts, delayed, or their bars have not started.
blast-hold-window = waiting for a blast window
blast-hold-window-note = Charged, and waiting for the next blast window to fire.
blast-add-bar = Add blast bar
drill-blast-step = Drill & Blast
drill-blast-blasts = Blasts
drill-blast-no-blasts = No blasts yet: run Solids through Blasting.
blast-set-stage = Starts { $stage }
drill-blast-enabled = Sequence drill & blast
drill-blast-burden = Burden (m)
drill-blast-spacing = Spacing (m)
drill-blast-subdrill = Subdrill (m)
drill-blast-diameter = Hole diameter (mm)
drill-blast-stemming = Stemming (m)
drill-blast-density = Product density (t/m³)
drill-blast-buffer = Clearance buffer (m)
drill-blast-window-start = Opens at
drill-blast-window-end = Closes at
drill-blast-default-window = Default daily blast window
drill-blast-staggered = Staggered rows
drill-blast-charge-per-hole = Charge per hole, { $bench } m bench
drill-blast-number = Enter a number
drill-blast-clock = Enter a time of day, such as 06:30
drill-blast-off-note = Off: every dig block can be dug from the start.
drill-blast-pattern = Pattern
drill-blast-holes = Holes & charge
drill-blast-timing = Timing
drill-blast-group-hint = Right-click to set where every blast in it starts.
drill-blast-select-hint = Select a blast to see its area or give it its own pattern.
drill-blast-no-windows = No blasting windows: only blasts that start Fired are ever dug. Turn on the default window, or add windows on the Gantt's Blasting row.
drill-blast-stage = Starts at
drill-blast-area = Area
drill-blast-own-pattern = Its own pattern
blast-edit-bar = Edit blasts…
blast-machine = Machine
blast-machine-choose = Choose a dozer, drill or MPU
blast-bar-order = Worked in this order
blast-bar-empty-order = No blasts yet. Click one in the view to add it.
blast-sequence-preview-at = { $fired } of { $total } blasts fired
blast-sequence-all-fired = Everything in view is fired. Move Sequence Preview back, or show more in Solids Navigation.
blast-bar-missing = (not in this Solids run)
gantt-palette-blast = Blasts
gantt-palette-follow = Follow
gantt-follow-title = Follow which machine?
gantt-follow-none = No other drill and blast machine to follow.
follow-bar-default-name = Follow { $machine }
follow-bar-no-machine = Follow (no machine)
follow-bar-no-leader = The machine this bar follows was removed. Choose another from the bar's right-click menu.
follow-bar-leader = Follows
follow-bar-help = While open, this machine works the blasts its leader's blast bar has next, adding its rate to the leader's.
schedule-add-follow-bar = Add Follow Bar
schedule-delete-bars = Delete { $count } Bars
blast-work-heading = { $activity } · { $blast }
blast-work-progress = { $done } of { $total } { $unit } done by now
blast-fired-at = Dig from { $at }
machine-kind = Machine type
loader-type = Loader type
loader-type-help = How this class's loaders are drawn in Animate. It changes nothing the schedule is calculated from.
loader-type-excavator = Excavator
loader-type-wheel-loader = Wheel loader
machine-work-rate = Rate ({ $unit })
blast-bar-default-empty = Blasts
blast-bar-default-name = { $first } (+{ $more })
inspector-drill-blast = DRILL & BLAST
inspector-drill-blast-idle = Not working
inspector-blast-clear = Clear
inspector-blasts-fired = { $fired } of { $total } blasts fired
schedule-calendar-blast-work = Worked ({ $unit })
blast-edit-title = Edit blasts
report-blast-activity = Activity
report-blast-unit = Unit
report-blast-quantity = Work completed
report-blast-hours = Working hours
gantt-blasting-row = Blasting
gantt-blasting-row-note = Windows and fired blasts
blast-window-add = Add blasting window…
blast-window-edit = Edit blasting window
blast-window-new = Add blasting window
blast-window-daily = Repeat daily
blast-window-once = One-off window
blast-window-hour-note = Start and end are hours of the day (0–24).
blast-window-instant-note = Write a time like 17/04/2026 06:00, Day 3 06:00, or a number of hours from the start.
blast-window-opens = Opens
blast-window-closes = Closes
blast-window-default = Default daily window, set in Setup → Drill & Blast
blast-window-edit-default = Edit the default window in Setup
blast-window-changed = Windows changed while this editor was open. Close and reopen it.
blast-window-invalid = Enter a start and end with the end after the start. Daily windows must fit within 0–24 hours.
blast-sequence-changed = The bar or Solids run changed. Close and reopen this sequence editor.
blast-windows-label = Windows added on the Gantt
blast-windows-hint = Add a one-off window by right-clicking the Gantt's Blasting row.
haul-auto-join = Auto-join distance
haul-auto-join-help = How far a dig block looks for a road node.
haul-bench-speed = Bench speed
haul-bench-speed-help = Top speed from a dig block to its road.
haul-acceleration = Acceleration
haul-acceleration-help = Sets the time lost starting and stopping.
haul-clear-role = Clear role
haul-delete-node = Delete node and its roads
haul-remove-node = Remove node
haul-remove-node-help = Join the two roads that meet here into one road, which keeps a bend where the node was. Blocks joined only to this node go back to the nearest node.
haul-remove-node-role = A destination's node can't be removed. Clear its destination first.
haul-remove-node-two = Only a node where exactly two roads meet can be removed.
haul-remove-node-loop = These two roads already meet at both ends, so joining them would make a loop.
haul-delete-role-confirm = Delete this destination node and all its roads?
haul-delete = Delete
haul-cancel = Cancel
haul-split = Split road here
haul-promote-bend = Promote to road node
haul-promote = Promote to destination…
haul-promote-title = Promote to Destination
haul-promote-apply = Promote
haul-destination = Destination
haul-point = Point
haul-join-nodes = Join nodes
haul-method-roads = Its road node
haul-method-nearest = Nearest road
haul-method-none = No road access
haul-no-route-short = No route
haul-pin-nearest = Add a node at the nearest road
haul-route-check = Route Check
haul-reclaim = Reclaim
haul-loader = Loader
haul-truck = Truck class
haul-no-route = No road route reaches this destination from here, so nothing can be hauled on it.
haul-unconnected = No road node within the auto-join distance: trucks are assumed to drive { $length } m straight to the nearest node ({ $rise } m).
haul-spot = Spot
haul-load = Load
haul-loaded = Loaded haul
haul-dump = Dump
haul-return = Return
haul-cycle = Avg cycle (min)
haul-cycle-minutes = Cycle: { $minutes } min
haul-distance = Loaded haul (km)
haul-rise = Rise (m)
haul-tonne-km = Tonne-km
haul-match = { $trucks } trucks to match the loader
haul-table = Haulage
haul-maximum-speed = Maximum speed
haul-maximum-grade = Maximum grade
haul-dump-time = Dump time
haul-spot-time = Spot time (s)
haul-grade-from = Grade from
haul-grade-to = To
haul-speed-capped = (capped to { $speed } km/h)
haul-loaded-speed = Loaded
haul-empty-speed = Empty
haul-add-band = Add Grade Band
haul-grade-speeds = Road Grade Speeds
haul-grade-band = Grade band
haul-default-speeds = Use default grade speeds
haul-error-grade = Enter a grade above 0 and up to 100 %.
haul-error-seconds = Enter a number of seconds, 0 or more.
haul-remove-band = Delete Grade Band
haul-speeds-help = Speeds for a large haul truck on a road with 2% rolling resistance: rimpull uphill, retarder downhill. A band runs up to the next grade.
haul-open-layout = Open the Road Network
haul-dump-override = Override dump time (s)
haul-unlimited = Default
haul-convert-help = Turn the selected design polylines into haul roads. Ends and crossings within the join tolerance become shared nodes; the polylines stay as they are.
haul-empty = No roads yet. Draw one, or convert or import pit design polylines.
haul-connected-help = A block is connected when it can reach a road node within the auto-join distance, climbing or descending no steeper than the truck's maximum grade. Green blocks join the nearest node, blue ones the nodes chosen for them; red ones are out of reach and still scheduled, with a longer straight drive to the nearest node.
haul-roads-selected =
    { $count ->
        [one] 1 road selected
       *[other] { $count } roads selected
    }
haul-nodes-selected =
    { $count ->
        [one] 1 node selected
       *[other] { $count } nodes selected
    }
haul-role-none = Junction (no destination)
haul-role-dump = Dump point · { $destination }
haul-role-reclaim = Reclaim point · { $destination }
haul-role-both = Dump & reclaim point · { $destination }
haul-dump-and-reclaim = Dump & reclaim
haul-pin-stockpile-dump = Stockpile · dump point
haul-pin-stockpile-reclaim = Stockpile · reclaim point
haul-link-missed = Click a node or a road to join the blocks to it.
haul-point-no-snap = Nothing to snap to here. Items stay locked on this page until you unlock them with their padlock.
haul-block-joined = Chosen nodes · { $length } m
haul-block-nearest = Nearest node · { $length } m
haul-block-far = Nearest node is { $length } m away, beyond the auto-join distance
haul-block-no-roads = No roads yet
haul-link-pick = Join to road nodes…
haul-link-help = Pick one or more; each block uses whichever gives the quickest cycle.
haul-link-clear = Use nearest node
haul-link-picking = Click nodes or road points, then Join.
haul-step-network = Road Network
haul-network-settings = Network Settings
haul-join-title = Join to Road
haul-blocks = Blocks
haul-nodes = Nodes
haul-join-blocks = Join
haul-join-blocks-help = A point on a road becomes a node.
haul-joined-to = Joined to
haul-joined-manually = Joined manually
haul-joined-nearest = Nearest node
haul-out-of-reach = Out of reach
haul-out-of-reach-help = No road node within the auto-join distance. Trucks are assumed to drive straight to the nearest node; draw a road or join these blocks to a node.
haul-blocks-in = { $area } · { $count ->
        [one] 1 block
       *[other] { $count } blocks
    }
haul-route-hint = Shift-click a destination to check a route.
haul-route-hint-destination = Shift-click a block to check a route.
haul-blocks-selected =
    { $count ->
        [one] 1 block selected
       *[other] { $count } blocks selected
    }
haul-flow-hover = Loaded haul flow: { $rate } t/h on this route
haul-role-help = Trucks deliver to a dump point and load from a stockpile's reclaim point. A stockpile with no reclaim point is loaded at its dump point.
haul-node-on = End of { $road }
haul-selection = Selection
haul-selection-empty = Click a road, node or block; shift-click for more.
haul-new-road = New Road
haul-points = Points
haul-drawing-keys = Backspace undoes · Enter next road · Esc finishes
haul-access = Access
haul-access-far = { $length } m from a node
haul-name = Name
haul-length = Length
haul-steepest = Steepest grade
haul-speed-limit-short = Speed limit
haul-link-connected = Connected
haul-link-surface = None: trucks dump on its surface from the nearest road
haul-link-at-dump = Same as its dump point
haul-link-no-point = None: give a road node this role
haul-link-off-road = Not on any road
haul-link-separate = On a road that does not join the rest
haul-link-no-roads = No roads
haul-link-no-roads-note = There are no roads, so trucks cannot haul anything. Draw them in Setup's Road Network step.
haul-link-dump-problem = Trucks cannot dump here: { $status }
haul-link-reclaim-problem = Trucks cannot reclaim from here: { $status }
haul-link-pit-problem = { $missed } of { $total } dig blocks reach no road
haul-link-dump-column = Dump point
haul-link-reclaim-column = Reclaim point
haul-link-pit-column = Pit
haul-link-blocks-column = Dig blocks
haul-link-blocks = { $reached } of { $total } reach the roads
haul-connections = Connections
haul-connections-pits = Pits
haul-connections-not-run = Run this step to check what reaches the roads.
haul-connections-no-destinations = No stockpiles, dumps or crushers yet.
haul-connections-no-blocks = No dig blocks yet: run Solids.
haul-connections-open = Open the Road Network to fix it
haul-speed-limit-help = Leave empty to drive at each truck class's own maximum speed.
haul-speed-limit-truck = Truck class limit
haul-role = Role
haul-issue = Issue
haul-where = Where
haul-issues-title = Issues
haul-no-issues = No issues found.
haul-cycle-time = Cycle
haul-per-truck-short = Per truck
haul-match-short = Trucks to match loader
haul-loaded-distance = Loaded distance
haul-return-distance = Return distance
haul-rise-short = Rise
schedule-capture-bar-block-unmeasured = a dig block in this bar has no measured tonnage
schedule-capture-bar-machine-missing = this bar's machine is no longer in the project
schedule-capture-machine-class-missing = this machine's class is no longer in the project
schedule-capture-unroutable = { $destination }: no road route from { $count } sources it is offered
schedule-capture-stranded = { $count } sources have no road route to any destination they are offered

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
app-failed-initialize-graphics-error = Failed to initialise graphics: { $error }
app-browser-preferences-load-failed = Failed to load browser preferences: { $error }
app-failed-load-config-file-error = Failed to load config file: { $error }
app-failed-load-session-file-error = Failed to load session file: { $error }
app-failed-rasterize-window-icon-error = Failed to rasterise window icon: { $error }
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
block-model-source-some-holes = { $name } ({ $count } holes)
block-model-nugget = Nugget
block-model-numeric-interval-fields-interpolate = Numeric interval fields to interpolate. Each selected field becomes one block-model variable.
block-model-kriging-help = Ordinary Kriging estimates numeric drill-hole intervals at each block centre using a spherical variogram.
block-model-partial-sill = Partial sill
block-model-range-search-radius = Range / search radius
block-model-range-help = Samples farther than this distance are excluded; covariance reaches zero at this range.
block-model-select-all = Select all
block-model-source-model-help = The block model whose blocks are thresholded into a solid.
block-model-source-drillholes-help = The drillhole collection whose numeric intervals are estimated into blocks. Holes picked in the viewport before opening limit the estimate to those holes.
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
cmd-block-model-loaded-block-model-name-blocks = Loaded block model '{ $name }': { $blocks } blocks, { $variables } variables
cmd-block-model-loading-name = Loading { $name }
cmd-block-model-loading-name-ellipsis = Loading { $name }…
cmd-chamfer-applied = Chamfered corner { $corner } with radius { $radius } and { $segments } segments
cmd-chamfer-radius = Radius { $radius }
cmd-commands-clipped = Clipped
cmd-commands-command-failed-error = Command failed: { $error }
cmd-commands-count-control-string-s = { $count } control polyline(s)
cmd-commands-count-control-string-s-layer = { $count } control polyline(s) on '{ $layer }'
cmd-commands-count-point-s-across-layers = { $count } point(s) across { $layers } layers
cmd-commands-count-point-s-layer = { $count } point(s) on '{ $layer }'
cmd-commands-kind-layer = { $kind } on '{ $layer }'
cmd-commands-no-control-strings = No control polylines
cmd-commands-no-extent = No extent
cmd-commands-sliced = Clipped
cmd-contours-contour-generation-failed-error = Contour generation failed: { $error }
cmd-contours-discarded-layer-exists = Contours for '{ $name }' were discarded: layer '{ $layer_name }' now exists
cmd-contours-discarded-project-closed = Contours for '{ $name }' were discarded: the project was closed
cmd-contours-discarded-layer-deleted = Contours for '{ $name }' were discarded: the selected output layer was deleted
cmd-contours-generated = Generated { $line_count } contour polyline(s) for triangulation '{ $name }' in layer '{ $layer_name }'
cmd-creation-assembled-boundary-rings = Assembled { $assembled_count } closed boundary ring(s) from fragmented open polylines
cmd-creation-created-triangulation-from-boundary = Created triangulation from { $boundary_count } boundary ring(s) and { $constraint_count } open constraint(s), surface type { $surface_type }
cmd-creation-creating-triangulation = Creating triangulation…
cmd-creation-generate-upper-surface-ignored-count = Generate upper surface: ignored { $count } lower conflicting breakline segment(s); source objects are unchanged
cmd-creation-ignored-objects = Ignored { $rejected } non-polyline or degenerate object(s) during triangulation
cmd-creation-weld-retry-moved-coarse-welded = Weld & retry: moved { $coarse_welded } vertex/vertices onto shared positions (up to { $coarse_weld_tol } m); source objects are unchanged
cmd-creation-welded-breakline-vertices = Welded { $welded } breakline vertex/vertices that coincided within tolerance
cmd-cuts-clipped-surface-name-polyline-mode = Clipped surface '{ $name }' by polyline ({ $mode })
cmd-cuts-clipping-surface-polyline = Clipping surface by polyline…
cmd-cuts-cut-topology-name-pit-shell = Cut topography '{ $name }' to pit shell
cmd-cuts-cut-triangulation-name-z-band = Clipped triangulation '{ $name }' to elevations [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = Cutting topography by pit shell…
cmd-cuts-cutting-triangulation-z = Clipping triangulation by elevation…
cmd-cuts-ignored-vertical-faces = Ignored { $count } vertical or degenerate reference topography face(s) with no XY area
cmd-cuts-site-skipped-constraint-from-x = { $site }: skipped constraint ({ $from_x }, { $from_y }) -> ({ $to_x }, { $to_y }) the triangulator could not split
cmd-cuts-skipped-degenerate-edges = { $site }: skipped { $skipped } near-degenerate constraint edge(s); the cut boundary may be off by a hairline near them
cmd-cuts-trimmed-surface = Trimmed surface '{ $surface }' to topography '{ $topology }' ({ $mode })
cmd-cuts-trimming-surface-topology = Trimming surface to topography…
cmd-drape-draped-intersected-vertices-changed = Draped { $intersected } vertices; { $changed } changed elevation
cmd-drape-no-intersections = None of the selected design vertices intersect the selected topographies
cmd-drape-objects-changed-object-s-changed = { $objects } changed object(s) · { $changed } of { $intersected } intersecting vertices moved
cmd-drape-select-one-more-design-objects = Select one or more design objects to drape
cmd-drape-select-one-more-topologies-drape = Select one or more topographies to drape onto
cmd-drape-selected-topologies-no-longer-loaded = The selected topographies are no longer loaded
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
cmd-file-wait-browser-project-finish-opening = Wait for the browser project to finish opening
cmd-file-wait-browser-project-removal-finish = Wait for the browser project removal to finish
cmd-file-wait-current-project-switch-finish = Wait for the current project switch to finish
cmd-file-wait-project-operation-finish-before = Wait for the project operation to finish before discarding changes
cmd-file-wait-project-finish-opening = Wait for the project to finish opening
cmd-file-wait-project-revert-finish-before = Wait for the project revert to finish before saving
cmd-folder-collection-named-name-already-exists = A collection named '{ $name }' already exists
cmd-folder-collection-no-longer-exists = That collection no longer exists
cmd-folder-created-collection-name = Created collection '{ $name }'
cmd-folder-deleted-collection-name = Deleted collection '{ $name }'
cmd-folder-deleted-collection-contents = Deleted collection '{ $name }' and its contents
cmd-folder-moved-item-into-collection-name = Moved item into collection '{ $name }'
cmd-folder-moved-item-root-section = Moved item to the root of { $section }
cmd-folder-renamed-collection-before-after = Renamed collection '{ $before }' to '{ $after }'
cmd-folder-section-cannot-hold-item = That section cannot hold this item
cmd-fuse-closed-polyline = Closed polyline
cmd-fuse-count-source-line-s = { $count } source line(s)
cmd-fuse-created-shape-object-id-vertices = Created { $shape } { $object_id } with { $vertices } vertices from { $sources } source line(s)
cmd-fuse-click-missed = Join: click did not hit any object (nothing under cursor)
cmd-fuse-click-not-near-endpoint = Join: click was not close enough to either endpoint of the selected line
cmd-fuse-clicked-closed-polyline = Join: clicked object { $object_id } is a closed polyline; only open polylines can be joined
cmd-fuse-clicked-not-open-polyline = Join: clicked object { $object_id } is not an open polyline (it's a { $kind })
cmd-fuse-clicked-object-missing = Join: clicked object { $object_id } no longer exists
cmd-fuse-clicked-too-few-vertices = Join: clicked polyline { $object_id } has only { $count } vertex/vertices, need at least 2
cmd-fuse-endpoint-marker-missing = Join: endpoint marker { $marker_index } no longer exists
cmd-fuse-close-needs-three-vertices = Join: line needs at least 3 distinct vertices to close into a polyline (has { $count })
cmd-fuse-lines = Join Polylines
cmd-fuse-needs-two-segments = Join: need at least 2 segments to commit (have { $count })
cmd-fuse-no-active-layer = Join: no active layer to place the joined line on
cmd-fuse-no-active-project = Join: no active project, cannot commit
cmd-fuse-no-source-line = Join: no source line to close into a polyline
cmd-fuse-awaiting-object-invalid = Join: object { $awaiting_id } is no longer a valid polyline
cmd-fuse-object-already-in-chain = Join: object { $object_id } is already part of the join chain, click a different line
cmd-fuse-result-too-few-vertices = Join: result has too few vertices ({ $count }), aborting
cmd-fuse-segment-object-invalid = Join: segment object { $object_id } is no longer a valid polyline, aborting
cmd-fuse-source-object-invalid = Join: source object { $object_id } is no longer a valid open polyline
cmd-fuse-source-object-missing = Join: source object { $object_id } no longer exists
cmd-fuse-open-polyline = Open polyline
cmd-include-failed = Include failed: { $message }
cmd-include-included-solid-shape-name-topology = Included solid '{ $shape_name }' in topography '{ $topology_name }' (retained { $retained } topography faces, skipped { $skipped } closure-cap faces)
cmd-include-including-pit-stockpile-solid = Including pit/stockpile solid…
cmd-insert-point-count-operation-point-s = { $count } { $operation } point(s)
cmd-insert-point-elevation-must-be-finite = Insert Point at Elevation requires a finite elevation
cmd-insert-point-insert-points = Insert Points
cmd-insert-point-inserted-count-operation-point-s = Inserted { $count } { $operation } point(s)
cmd-insert-point-intersection = Intersection
cmd-insert-point-no-new-operation-points-were = No new { $operation } points were found
cmd-layer-created-layer-name = Created layer '{ $name }'
cmd-layer-deleted-with-objects = Deleted layer { $name } and everything on it
cmd-layer-duplicated-layer-duplicate-name = Duplicated layer '{ $duplicate_name }'
cmd-layer-locked = Locked
cmd-layer-name-copy = { $name } copy
cmd-layer-selected-count-object-s-layer = Selected { $count } in { $name }
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
cmd-omf-crs-not-set = { $source_name } uses the coordinate reference system '{ $source_crs }'; the project's is not set, so it was left unset
cmd-omf-units-not-set = { $source_name } uses the units '{ $source_units }'; the project's are not set, so they were left unset
cmd-omf-there-no-open-incline-design = There is no open Incline Design data to export
cmd-placement-2-vertices = 2 vertices
cmd-placement-count-vertices = { $count } vertices
cmd-placement-created-circle = Created circle with radius { $radius } m
cmd-placement-created-closed-polyline = Created closed polyline with { $count } vertices
cmd-placement-created-line-segment-2-vertices = Created line segment with 2 vertices
cmd-placement-created-open-polyline-count-vertices = Created open polyline with { $count } vertices
cmd-placement-placed-point-x-y-z = Placed point at { $x }, { $y }, { $z }
cmd-placement-radius = Radius { $radius } m
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
cmd-property-batch-set-color-count-object = Batch-set colour on { $count } object(s)
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
cmd-reference-surface-control-string-index-could-not = Control polyline { $index } could not be added to the mesh
cmd-reference-surface-control-string-index-crosses-itself = Control polyline { $index } crosses itself in plan
cmd-reference-surface-control-string-index-doubles-back = Control polyline { $index } doubles back on itself in plan
cmd-reference-surface-control-string-index-ends-where = Control polyline { $index } ends where it starts; close it to use it as a mask
cmd-reference-surface-control-string-index-has-count = Control polyline { $index } has { $count } distinct vertex(es); a control needs at least { $minimum }
cmd-reference-surface-control-string-index-has-non = Control polyline { $index } has non-finite coordinates
cmd-reference-surface-control-string-index-no-longer = Control polyline { $index } is no longer available
cmd-reference-surface-control-string-index-runs-along = Control polyline { $index } runs along the extent polyline in plan; that is not supported yet
cmd-reference-surface-control-string-overrides-pick-x = Control polyline overrides the pick at ({ $x }, { $y }): pick { $pick } m, control { $control } m, difference { $difference } m
cmd-reference-surface-control-strings-b-disagree-x = Control polylines { $a } and { $b } disagree at ({ $x }, { $y }): { $za } m against { $zb } m, { $difference } m apart
cmd-reference-surface-control-strings-b-run-along = Control polylines { $a } and { $b } run along each other in plan; that is not supported yet
cmd-reference-surface-count-control-string-s-vertices = ; { $count } control polyline(s) with { $vertices } vertex(es){ $crossings }
cmd-reference-surface-count-point-s-inside-extent = { $count } point(s) inside the extent; a surface needs at least { $minimum }
cmd-reference-surface-count-point-s-outside-extent = ; { $count } point(s) outside the extent shaped it as support
cmd-reference-surface-count-point-s-selected-surface = { $count } point(s) selected; a surface needs at least { $minimum }
cmd-reference-surface-count-point-s-shared-plan = ; { $count } point(s) shared a plan position and were kept once
cmd-reference-surface-delaunay-insert-failed-error = Delaunay insert failed: { $error }
cmd-reference-surface-extent-must-closed-string = The extent must be a closed polyline
cmd-reference-surface-extent-string-crosses-itself-plan = The extent polyline crosses itself in plan
cmd-reference-surface-extent-string-has-non-finite = The extent polyline has non-finite coordinates
cmd-reference-surface-extent-string-needs-least-three = The extent polyline needs at least three distinct vertices
cmd-reference-surface-extent-string-no-longer-available = The extent polyline is no longer available
cmd-reference-surface-meeting-count-crossing-s = meeting at { $count } crossing(s)
cmd-reference-surface-and-more = , … and { $more } more
cmd-reference-surface-no-mask-selected-surface-unclipped = No mask selected; the surface is unclipped
cmd-reference-surface-no-part-surface-falls-inside = No part of the surface falls inside the extent
cmd-reference-surface-open-project-before-building-surface = Open a project before building a surface
cmd-reference-surface-points-collinear-plan-surface-needs = The points are collinear in plan; a surface needs three that are not
cmd-reference-surface-select-exactly-one-closed-string = Select exactly one closed polyline to clip the surface to
cmd-reference-surface-selected-point-has-non-finite = A selected point has non-finite coordinates
cmd-reference-surface-selected-points-span-count-layers = The selected points span { $count } layers; the surface is placed under { $section }
cmd-relimit-click-missed = Trim/Extend: click did not hit any object (nothing under cursor)
cmd-relimit-click-ignored = Trim/Extend: click ignored, tool is not currently waiting for a boundary pick
cmd-relimit-clicked-source-line = Trim/Extend: clicked the source line itself, pick a different line
cmd-relimit-no-source-line = Trim/Extend: no source line is set, aborting pick
cmd-relimit-relimited-line-source-id-selected = Trimmed or extended line { $source_id } to the selected boundary
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
cmd-session-updated-triangulation = Updated triangulation '{ $name }' ({ $face_count } faces)
cmd-session-failed-load-triangulation-error = Failed to load triangulation: { $error }
cmd-session-failed-load-triangulation-message = Failed to load triangulation: { $message }
cmd-session-loaded-triangulation = Loaded triangulation '{ $name }' ({ $path }, { $vertex_count } vertices, { $face_count } faces)
cmd-session-set-triangulation-tri-id-color = Set triangulation { $tri_id } colour to { $color }
cmd-session-triangulation-load-no-result = Triangulation load for { $path } ended without a result
cmd-session-triangulation-failed = Triangulation operation failed: { $message }
cmd-session-unloaded-triangulation-name = Unloaded triangulation '{ $name }'
cmd-slice-entered-slice-view-cx-cy = Entered section view @ { $cx }, { $cy }, { $cz } along { $dx }, { $dy } ({ $length }m line)
cmd-slice-exited-slice-view = Exited section view
cmd-slice-reset-section-view-fit-extents = Reset the section view (fit to extents)
cmd-slice-set-section-grid-enabled = Set section grid = { $enabled }
cmd-split-created-2-open-polylines = Created 2 open polylines
cmd-split-line = Split Line
cmd-split-points-needs-interior-vertex = Split At Points: choose an interior vertex of the open line
cmd-split-polyline-into-two = Split source polyline into two open polylines
cmd-text-edit-finished = Finished text edit for object { $object_id }
cmd-text-updated = Updated text on object { $object_id }
cmd-view-centre-rotation-not-available-flying = The centre of rotation is not available in fly mode
cmd-view-fixed-centre-rotation-x-y = Fixed the centre of rotation at { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = No point under the cursor to fix the centre of rotation on
cmd-view-released-centre-rotation = Released the centre of rotation
cmd-view-reset-view-fit-extents = Reset view (fit to extents)
cmd-view-set-cinematic-view-enabled = Set cinematic view = { $enabled }
cmd-view-set-topology-contours-enabled = Set topography contours = { $enabled }
cmd-view-set-topology-wireframes-enabled = Set topography wireframes = { $enabled }
cmd-view-set-view-points-enabled = Set view points = { $enabled }
cmd-view-set-xy-grid-enabled = Set XY grid = { $enabled }
cmd-view-zoom-extents-preserving-angle = Zoom to extents (preserving angle)

## Common strings

common-add-product = Add Product
common-appearance = Appearance...
common-background = Background
common-block-model = Block model
common-block-models = Block Models
common-borehole-inspector = Drillhole Inspector
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
routing-problem-unreconciled = ⁨{ $block }⁩: its material adds up to { $portions } t but it was measured at { $total } t
routing-problem-scope-unplaced = ⁨{ $rule }⁩ names ground the last Solids run did not produce

# Why a run stopped with work left that it could otherwise have done. The
# destination is named by the caller, which knows the project's own names.

# Calendar rows for destinations. "Scheduled inventory" rather than stock on
# hand: nothing is reclaimed in this increment, and no opening inventory is
# modelled, so the figure is what this schedule put there and nothing else.
destination-calendar-limit = Maximum tonnes
destination-calendar-received = Received
destination-calendar-processed = Processed
destination-calendar-deposited = Deposited
destination-calendar-inherited = the default
destination-calendar-default-hover = Periods with no figure of their own use this. Type a number of tonnes, or Unlimited.
destination-new = New Destination
destination-delete = Delete Destination
destination-new-rule = New Rule
destination-duplicate-rule = Duplicate Rule
destination-delete-rule = Delete Rule
destination-crusher-edit = Edit crusher limits
pile-mode-edit = Edit stockpile modes
pile-operating-edit = Edit stockpile operation
pile-simultaneous = Build and reclaim at once
pile-simultaneous-help = When off, the stockpile takes no deliveries in an hour it is reclaimed in. A Reclaim bar working the pile has the hour; deliveries go to the next destination their rules allow.
pile-rest = Rest before reclaim
pile-rest-help = Hours new material must rest before it can be reclaimed: since the last delivery to the pile, or to the parcel for a stockpile kept in parcels. While the parcel next in the reclaim order rests, the pile is not reclaimed. Opening stock is already rested. 0 lets the pile be tipped on and reclaimed at once.
pile-mode-row = Mode
pile-general = General
pile-operation = Operation
pile-optimiser = Optimiser
pile-haulage = Haulage
pile-capacity-unlimited = Unlimited
pile-daily-mode = Daily mode
pile-daily-mode-overrides = { $mode } · { $days ->
    [one] 1 day differs
   *[other] { $days } days differ
}
pile-open-calendar = Open in Calendar
pile-chunk-size-missing = A stockpile kept in parcels needs a parcel size.
pile-chunk-over-capacity = The parcel size is larger than the stockpile's maximum tonnes, so no parcel could ever fill.
pile-chunk-grade-missing = { $chunk } has no { $grade }, which the schedule tracks.
pile-chunk-grade-negative = { $chunk } has a negative { $grade }.
pile-grade-needed = The schedule tracks this grade, so it needs a value.
pile-not-checked = Not checked yet
pile-opening-stock = Opening stock
pile-opening-empty = Starts empty
pile-opening-differs = The parcels differ, so these are their combined values.
pile-opening-split-again = Split into parcels again
pile-opening-combine = Combine into one blend
pile-opening-not-held = —
inventory-blend-name = Opening stock
inventory-chunk-name = Parcel { $number }
destination-capacity-column = Capacity
destination-limit-column = Daily limit
pile-mode-both = Build & reclaim
pile-mode-build = Build only
pile-mode-reclaim = Reclaim only
pile-mode-off = Off
pile-mode-default-hover = What this stockpile may do on any day without its own mode. Double-click to choose.
pile-mode-day-hover = { $mode } ({ $source }). Double-click to choose; Delete returns the day to the Default.
pile-mode-invalid = Type Build & reclaim, Build only, Reclaim only or Off.
pile-mode-inherit = Default ({ $mode })
destination-crushers = Crushers
destination-destinations = Destinations
destination-kind-stockpile = Stockpile
destination-kind-dump = Dump
destination-kind-crusher = Crusher
destination-error-invalid-capacity = Capacity must be a number of tonnes that is not negative. Leave it blank for unlimited.
destination-error-unknown = That destination is no longer in the schedule
destination-error-unknown-rule = That routing rule is no longer in the schedule
destination-error-in-use = Still used by ⁨{ $rules }⁩. Change or delete those rules first
destination-error-not-a-crusher = Only a crusher has a daily tonnage limit
destination-error-empty-selection = Select at least one entry, or choose All
destination-error-invalid-scope = A source must name a finite, non-empty elevation range
destination-error-empty-condition = A condition needs at least one value or bound
destination-error-invalid-bound = A bound must be a finite number
destination-error-empty-interval = Those bounds describe a range no value can be in
destination-error-duplicate-condition = One field can carry only one condition in a rule
destination-error-duplicate-value = ⁨{ $value }⁩ is listed twice
destination-error-rule-at-end = That rule is already at the end of the order
destination-problem-solid-missing = ⁨{ $rule }⁩ delivers to a solid that is no longer in the project
destination-problem-solid-kind = ⁨{ $rule }⁩ delivers to a solid that is no longer a stockpile or dump
destination-problem-missing = ⁨{ $rule }⁩ delivers to a destination that is no longer in the schedule
destination-rule-matches-all = All material
destination-rule-sources-count = { $count ->
        [one] 1 source
       *[other] { $count } sources
    }

# Run Period and Run All Periods - the schedule optimiser's explicit runs
schedule-run-period-note = Run Period: schedule one more day.
schedule-run-whole-note = Run All Periods: schedule through the planning end day.
schedule-improve-note = Improve: search for a better schedule, for up to the solve time limit.
schedule-improving-note = Looking for a better schedule.
schedule-improve-finishing = Run { $run }: finishing early with the best schedule found so far.
schedule-improve-stalled = No better schedule in { $minutes } min; finishing the run.
improve-card-title = Improving schedule
improve-card-title-finishing = Finishing…
improve-card-title-published = Improve finished
improve-card-title-stopped = Improve stopped early, best kept
improve-card-title-not-published = Improve stopped
improve-card-limit = of { $limit }
improve-card-best = Best found
improve-card-gain = +{ $gain } ({ $percent })
improve-card-no-gain = No better schedule yet
improve-card-gap = { $gap } below the bound
improve-card-gap-note = The bound is the most any schedule could be worth. The gap closing to zero proves the schedule is the best there is.
improve-card-bound-optimistic = Bounds are optimistic: a real schedule may never reach one.
improve-card-no-bound = No bound proved yet
improve-card-no-bound-ended = No bound proved
improve-card-last-gain = Last better schedule { $ago } ago
improve-card-chart-empty = The value appears here once the search starts.
improve-card-chart-bound = Bound
improve-stage-capture = Reading the project
improve-stage-first = First schedule
improve-stage-search = Search
improve-stage-whole = Whole-horizon solve
improve-stage-publish = Publishing
improve-card-stop = Stop and keep best
improve-card-discard = Discard
improve-card-stall = Stop after no gain for
improve-card-stall-note = Finish the run on its own once the search has gone this long without finding a better schedule.
improve-card-stall-never = Never
improve-card-stall-minutes = { $minutes } min
improve-card-published = Published at { $value }.
improve-card-not-published = Nothing published; the schedule shown before stays.
schedule-run-cancel-note = Stop the run and keep the last result.
schedule-run-stop-early-note = Stop and keep the best schedule found so far.
schedule-run-job = Calculating schedule
schedule-run-never = Not run yet. Press Run All Periods, or switch on Auto.
schedule-run-working = Calculating through day { $day }…
schedule-haulage-waiting = Waiting for Haulage
schedule-solids-tonnage = Tonnage
schedule-unmeasured-as-zero = Count unreserved blocks as 0 t
schedule-blocks-zero = Blocks counted as 0 t
schedule-run-improving = Showing a first schedule through day { $day }. Looking for a better one over the whole horizon…
schedule-run-early = Run { $run }: a first schedule through day { $day } is ready and shown. The whole-horizon solve continues; stop it to keep this schedule.
schedule-run-stopped-early = Run { $run } stopped. Its first schedule is kept.
schedule-run-blocked = Cannot run: { $reason }
schedule-run-updating = Updating the schedule…
idle-title = Idle · { $reason }
idle-delayed = delayed
idle-delayed-note = A delay list, roster or delay bar takes this machine out here.
idle-unavailable = not available
idle-unavailable-note = The calendar gives this machine no rate here: its availability, utilisation or dig rate is zero.
idle-no-work = no bar open
idle-no-bars = No bars
idle-next-bar = Its next bar starts { $at }
idle-no-work-note = None of this machine's bars is open here. Drag a bar over this time, or widen one's window.
idle-work-finished = work finished
idle-work-finished-note = Everything in this machine's open bars has been dug, or the stockpiles it reclaims are empty. Add blocks to its sequence, or give it another bar.
idle-no-route = no destination rule
idle-no-route-note = Ground is left, but no destination rule sends its material anywhere this machine can haul to. Check the Destinations rules for this material.
idle-destinations-full = destinations full
idle-destinations-full-note = Every destination this machine's material may go to is full, or its crusher has used its budget for the day. Add capacity, raise the budget, or route the material somewhere else as well.
idle-full-list = No room at: { $destinations }
idle-pile-mode = stockpile settings
idle-pile-mode-note = A stockpile's settings stop this machine here: its Mode in the Calendar for the day, building and reclaiming not being allowed at once, or new material still resting. Change the setting, or route the material somewhere else as well.
idle-pile-list = Held by: { $destinations }
idle-no-trucks = no trucks
idle-no-trucks-note = Every truck class that can haul for this machine is fully used in this hour. Add trucks, or give this machine priority over the others.
idle-not-worth-it = not worth moving
idle-not-worth-it-note = Ground, room and trucks were all there, but moving the material was worth less than leaving it. Check the Cash flow values for this material and destination.
schedule-run-needs-solids = Cannot run until the Solids pipeline has been run: the schedule digs the blocks it makes.
schedule-run-at-end = The schedule already reaches the planning end day (day { $day }). Use Run All Periods to recalculate it, or move the planning end.
schedule-run-cancelled = Run { $run } was cancelled. The last calculated schedule is unchanged.
schedule-run-superseded = The schedule changed while it was being calculated. That run was discarded; run it again.
schedule-run-refused-plain = Run { $run } could not start.
schedule-run-refused = Run { $run } could not start: { $count } problems to fix:
schedule-run-refused-because = Run { $run } could not start. { $reason }
schedule-run-no-solution = Run { $run } reached its time limit without finding a schedule. Nothing was published.
schedule-run-infeasible = Run { $run }: no schedule satisfies the configured rules over this horizon. Nothing was published.
schedule-run-failed = Run { $run } did not produce a valid schedule. Nothing was published.
schedule-run-publication-failed = The validated answer could not be published: { $reason }
schedule-solver-crashed = The solver stopped unexpectedly ({ $status }). The project and any schedule already shown are unaffected; the details are in the log.
schedule-run-finished = Schedule run { $run } calculated through day { $day }
schedule-run-kept-better = Schedule run { $run } found nothing worth more than run { $held }, which is kept
schedule-run-first = Scheduled through day { $day }
schedule-run-restored = Opened the schedule saved with this project, worth { $value }, and checked it against the project.
schedule-run-optimal = Scheduled through day { $day } · optimal
schedule-run-limited-no-gap = Scheduled through day { $day }
schedule-run-no-bars = Drag Dig from the palette onto a loader to start the schedule.
schedule-run-empty-bars = Add dig blocks to a bar to start the schedule: right-click it, then Edit Sequence.
schedule-run-stale = Run { $run } is out of date. The calculated work is hidden until the schedule is run again.
schedule-result-summary = Unique ground: { $started } t · extracted: { $extracted } t · remaining: { $remaining } t
schedule-run-still-showing = Still showing run { $run }: { $status }
schedule-bar-reclaimed = Reclaimed: { $tonnes } t
schedule-bar-reclaimed-of = Reclaimed: { $tonnes } t of the { $cap } t cap
schedule-dispatch-reclaim = Reclaiming
schedule-dispatch-heading = { $kind }: { $source }
schedule-span-hours = { $from } → { $to } ({ $hours } h)
schedule-tonnes-per-day = { $value } t/day
schedule-dispatch-tonnes-rate = { $tonnes } t at { $rate } t/h
schedule-dispatch-shared-with = Also on this block: { $agents }
schedule-dispatch-delivered = → { $destination }: { $tonnes } t
schedule-dispatch-pile-holds = { $stockpile } holds { $tonnes } t at the end of this execution
schedule-calendar-schedule = Schedule
schedule-animation-stale = The schedule result is stale. Run Period or Run All Periods again.
schedule-animation-never = Run Period or Run All Periods to enable time scrubbing.
schedule-animation-inventory = { $stockpile }: { $tonnes } t at this instant
animate-span-minute = 1 min
animate-span-hour = 1 hr
animate-span-day = 1 day
animate-span-week = 1 week
animate-speed-note = How long playback takes over this much of the schedule
video-export-tool = Record video
video-export-tool-needs-schedule = Calculate the schedule to record it
video-export-tool-recording = A video is recording
video-export-title = Record Video
video-export-range = What to record
video-export-range-periods = Periods
video-export-range-custom = From and to
video-export-select-all = Select all
video-export-shift-click = Shift-click ticks or clears every day since the last one clicked.
video-export-month = { $month } { $year }
video-export-week = Week { $week }
video-export-week-range = { $week } · { $from } – { $to }
video-export-start = Start
video-export-end = End
video-export-speed = Speed
video-export-speed-playback = Playback speed
video-export-speed-rate = Plays
video-export-speed-duration = Video length
video-export-duration = Length
video-export-duration-help = How long the video runs, as minutes and seconds (mm:ss). What is recorded is played through in this time.
video-export-output = Video
video-export-frame-rate = Frame rate
video-export-summary = { $length } long · { $frames } frames
video-export-size = Size
video-export-size-named = { $name } · { $width } × { $height }
video-export-size-viewport = Viewport · { $width } × { $height }
video-export-size-custom = Custom
video-export-size-help = The video is drawn at this size from the viewport's camera, framing the largest part of the view with its shape - marked on the viewport while this is open.
video-export-width = Width
video-export-display = Display options
video-export-destination-labels = Destination labels
video-export-machine-labels = Machine labels
video-export-height = Height
video-export-too-small = The video must be at least 16 pixels wide and high.
video-export-nothing-selected = Tick at least one period to record.
video-export-invalid-range = The start must come before the end, inside the calculated schedule.
video-export-invalid-speed = Set a playback speed or a video length.
video-export-format-note = The file type is chosen when saving: MP4, WebM or MOV (AV1), or GIF.
video-export-record = Record…
video-export-file-name = Schedule
video-export-job = Recording { $file }
video-export-recording = Recording frame { $done } of { $total }
video-export-saved = Saved the video to { $path }
video-export-failed = The video could not be saved: { $error }
video-export-cancelled = Video recording cancelled
video-export-already-recording = A video is already recording.
video-export-needs-animate = Open Animate with a calculated schedule to record a video.
video-export-left-animate = Video recording stopped: Animate was left or its schedule changed.
schedule-calendar-value = Movement value ({ $currency })
schedule-calendar-grade-penalty = Grade penalties ({ $currency })
schedule-calendar-dig-tonnes = Mined (t)
schedule-calendar-reclaim-tonnes = Reclaimed (t)
schedule-calendar-scheduled-utilisation = Scheduled utilisation (%)
schedule-calendar-scheduled-utilisation-help = Hours digging and reclaiming at this machine's rates, as a share of the hours its availability and utilisation leave it.
schedule-calendar-utilisation-hours = { $worked } h worked of { $available } h available
schedule-calendar-utilisation-target = Utilisation target (%)
schedule-calendar-utilisation-incentive = Utilisation incentive ({ $currency }/t/%)
schedule-calendar-utilisation-incentive-help = Each dug tonne is worth this much more to the schedule for every point the loader's scheduled utilisation stands below its target, and nothing once it reaches it: the hourly schedule measures the day so far, Improve the whole day. It steers the schedule towards keeping loaders busy and is never counted as money.
schedule-calendar-closing-grades = Closing grades: { $grades }
truck-calendar-hours-used = Truck-hours used
destination-calendar-reclaimed = Reclaimed (t)
destination-calendar-closing = Closing stock (t)
schedule-note-event-budget = The derived execution-event budget was capped at { $positions } positions per interval, so some source transitions inside an interval may have been unavailable.
schedule-detail-value = Movement value: { $value }
schedule-detail-bound = Bound: { $bound } · relative gap { $gap }%
schedule-detail-bound-no-gap = Bound: { $bound } · relative gap not reported
schedule-detail-bound-relaxation = Bound: { $bound } · relative gap { $gap }% · proved by the linear relaxation (HiGHS), not by SCIP
schedule-detail-no-bound = Bound not reported by the solver.
schedule-detail-bound-weak = No useful bound on the best possible schedule was reached in the time allowed.
schedule-detail-fixed-blasts = Blast times are the hourly schedule's: Improve reorders the mining around them but does not move a blast, and the bound covers only schedules with these blast times.
schedule-detail-start-day-by-day = Day-by-day schedule ({ $windows } windows)
schedule-detail-start-hourly = Hourly dispatch schedule ({ $intervals } intervals)
schedule-detail-start-search = Search from the first schedule ({ $kept } improvements kept)
schedule-detail-start-kept = { $start } in { $seconds } s, worth { $value }. The whole-horizon solve found nothing better in its time, so this is the published schedule.
schedule-detail-start-early = { $start } in { $seconds } s, worth { $value }. The whole-horizon solve is still looking for a better one.
schedule-detail-start-proven = { $start } in { $seconds } s, worth { $value }. A bound proves it within the gap target, so the whole-horizon solve was not needed.
schedule-detail-start-stopped = { $start } in { $seconds } s, worth { $value }. The whole-horizon solve was stopped, so this is the published schedule.
schedule-detail-start-only = { $start } in { $seconds } s, worth { $value }. Improve searches the whole horizon for a better one.
schedule-first-schedule-failed = The hourly schedule could not be made ({ $reason }). Improve can still search for one with the full optimiser.
schedule-detail-start-improved = { $start } in { $seconds } s, worth { $value }. The published schedule is the whole-horizon solve's, which started from it.
schedule-detail-start-searched = { $start } in { $seconds } s, worth { $value }. The whole horizon is too large to solve at once, so this is the published schedule.
schedule-detail-day-by-day-failed = Day-by-day start abandoned ({ $reason }); the whole horizon was solved without it.
schedule-detail-timings = Solve { $solve } s · capture { $capture } s · model build { $formulate } s · validation and publication { $replay } s
schedule-detail-model = Model: { $variables } variables ({ $binaries } binary), { $constraints } constraints, { $intervals } intervals × { $positions } event positions
schedule-detail-backend = Backend: { $backend }
schedule-detail-proof-progress = Presolve { $presolve } s · { $nodes } nodes · { $iterations } LP iterations · { $entries } posted linear coefficient entries
schedule-detail-first-incumbent = First solver incumbent at { $seconds } s (independent validation performed after solving).
schedule-detail-root-unfinished = The initial root LP completed, but root-node processing did not finish.
schedule-detail-root-lp-unobserved = Root-node processing did not finish; no completed initial root LP was observed.
schedule-detail-capture = Captured { $candidates } movement candidates over { $blocks } dig blocks · model identity { $identity }
schedule-detail-optimal-scope = Optimality is for the model as encoded, within the gap target; the approximations below still apply.
schedule-detail-release = Stockpile receipts occupy capacity on arrival and join the reclaimable blend at the next interval boundary.
schedule-detail-grade-margin = Grade boundaries: a route is closed within { $fraction } (fraction) or { $percent } percentage points inside each bound. Conditional values within that band are valued conservatively by the optimiser; published values use the authored boundary.
schedule-detail-grade-band = { $deliveries } conditional-value deliveries ({ $tonnes } t) fell within that band; they could shift the value by at most { $value }.
schedule-detail-indicator-leak = Conditional-value indicators were solved to the backend's integrality tolerance; the optimiser's own figure may differ from the published value by up to { $value }.
schedule-detail-omitted = { $rows } solver rows totalling { $tonnes } t were too small to time and are not shown.
schedule-gantt-window = Window
schedule-gantt-window-open = { $from } onwards
schedule-gantt-window-span = { $from } → { $to }
schedule-bar-set-window = Set Work Window…
schedule-window-dialog = Work Window
schedule-window-start = Start
schedule-window-end = End
schedule-window-end-hint = Blank for open-ended
schedule-window-invalid = Write times like 17/04/2026 06:00 or Day 3 06:00. The end must be after the start, or blank.
schedule-window-apply = Set Window
schedule-dispatch-block = Block { $block }
schedule-dispatch-remaining = { $tonnes } t left in this block
schedule-dispatch-emptied = This block is finished
schedule-bar-worked = { $tonnes } t worked
schedule-bar-left-behind = { $tonnes } t stays in the ground for a later bar
schedule-bar-finished-early = Finished before the window closed
schedule-error-malformed-reference = That dig block reference could not be read
sequence-unresolved-solid = Its solid is no longer in this project
sequence-unresolved-flitch = No flitch sits at that level any more
sequence-unresolved-flitch-top = The flitch at that level now runs to ⁨{$now}⁩ m: the ground was re-flitched, and this is not the band it was planned against
sequence-unresolved-ground = No dig block covers that ground any more
sequence-unresolved-volume = That ground has kept its outline but its volume changed: it was { $was } m³ and is now { $now } m³. Reselect the block to plan against what is there now
sequence-unresolved-source = The surface this ground was cut from has changed since it was picked. Reselect or reconfirm the block to plan against what is there now
sequence-unresolved-ambiguous = ⁨{ $count }⁩ dig blocks cover that ground, so which one was meant cannot be decided
sequence-unresolved-changed = That ground has changed: it covered ⁨{ $was }⁩ m² and now covers ⁨{ $now }⁩ m². Reselect the block to plan against what is there now
sequence-empty = Add dig blocks to this bar
sequence-not-ready = The dig blocks have not been calculated: ⁨{ $reason }⁩
sequence-no-tonnage-field = Choose the reserve field that holds tonnes, in Configuration
sequence-tonnage-field-missing = The chosen tonnage field is no longer in this project's Field List
sequence-tonnage-field-not-sum = ⁨{ $field }⁩ is not a summed field, so it cannot be read as tonnes
sequence-unresolved-at = { $block }: { $reason }
blasting-recut-since-saved = This version cuts the blasts in { $solids } differently from the one that saved the project, so their outlines were updated. Save to keep them.
inspector-bar-section = Selected bar
inspector-bar-blocks =
    { $count ->
        [one] 1 block
       *[other] { $count } blocks
    }
inspector-bar-blocks-tonnes =
    { $count ->
        [one] 1 block · { $tonnes } t
       *[other] { $count } blocks · { $tonnes } t
    }
inspector-bar-blasts =
    { $count ->
        [one] 1 blast
       *[other] { $count } blasts
    }
schedule-blocks-show-unmeasured = Show them in 3D
sequence-carried-over =
    { $count ->
        [one] The Solids run cut 1 dig order block differently, so it now uses the block there.
       *[other] The Solids run cut { $count } dig order blocks differently, so they now use the blocks there.
    }
sequence-unresolved-count = ⁨{ $count }⁩ of its dig blocks could not be found in the current run
sequence-block-unmeasured = ⁨{ $block }⁩ has no measured tonnage: ⁨{ $reason }⁩
sequence-block-partial = ⁨{ $block }⁩ was only partly measured, so its tonnage is incomplete
sequence-duplicate-ground = Positions ⁨{$first}⁩ and ⁨{$second}⁩ are the same dig block (⁨{$block}⁩), so it would be dug twice. Remove one of them
sequence-invalid-tonnes = ⁨{$block}⁩ measures ⁨{$value}⁩ on the tonnage field, which cannot be tonnes. Check the field mapping or the block model
sequence-total-not-finite = This bar's total tonnage is not a finite number, so it cannot be executed
sequence-pick-stale = That pick was made against an older Solids run, so it was discarded. Pick the block again
sequence-pick-unknown-block = That dig block is not in the current Solids run, so the pick was discarded. Pick the block again
sequence-material-capacity-only = it has no block model, so only its capacity is known
sequence-material-no-schema = this project defines no reserve fields
sequence-material-unavailable = its reserves have not been measured
sequence-material-unmapped = no block model maps a value onto the chosen field

## Schedule → the floating sequence editor

sequence-editor-title = Edit Sequence: ⁨{ $bar }⁩
sequence-editor-no-run = ⁨{ $reason }⁩ Run Solids → Reserving to pick dig blocks for this bar.
sequence-editor-order = Dig order
sequence-editor-empty-order = No dig blocks yet
sequence-editor-unresolved-kept = ⁨{ $count }⁩ of these references could not be found in the current run. They stay in the order, and stay removable, until you decide what to do with them.
sequence-editor-stale-picks = ⁨{ $count }⁩ of these blocks were picked against an earlier Solids run. Remove and re-pick them; Apply will refuse them as they stand.
sequence-editor-bar-gone = This bar is no longer in the schedule. Nothing was applied.
sequence-editor-target-changed = This bar's dig order has changed since the editor was opened. Applying would overwrite that newer order with this older one, so it is refused. Reload to start again from what the bar holds now.
sequence-editor-reload = Reload
sequence-editor-preview = Sequence Preview
sequence-editor-preview-at = ⁨{ $dug }⁩ of ⁨{ $total }⁩ blocks
sequence-editor-apply = Apply
sequence-editor-discard-title = Discard these changes?
sequence-editor-discard-body = This editing session has changes that have not been applied to ⁨{ $bar }⁩.
sequence-editor-discard = Discard
sequence-editor-keep-editing = Keep Editing
sequence-editor-unknown-block = Unidentified ground
sequence-applied-blocks = ⁨{ $count }⁩ dig blocks
sequence-pick-superseded = Picked against an earlier Solids run
sequence-target-changed = This bar's dig order changed while the sequence editor was open, so the older draft was refused rather than written over it
sequence-solids-navigation = Solids Navigation
sequence-add-to-dig-order = Add to Dig Order
sequence-add-to-blast-order = Add to Blast Order
sequence-objects = Objects

## Schedule → Gantt

planning-subpage-gantt = Gantt
planning-subpage-charts = Charts
gantt-empty-fleet = No machines yet. Their rows appear here once machines are added in Setup.
gantt-add-loaders = Add Machines
schedule-add-work = Add work
gantt-reset-view = Reset View
gantt-reset-view-help = Fit the view to the scheduled work
gantt-zoom-in = Zoom in
gantt-zoom-out = Zoom out
gantt-day = Day { $day }
gantt-day-time = Day { $day }, { $time }
gantt-week = Week { $week }
gantt-week-date = Week { $week } · { $date }
gantt-week-short = W{ $week }
gantt-month = Month { $month }
gantt-month-short = M{ $month }
gantt-year = Year { $year }
gantt-range = { $from } → { $to }
gantt-slider-away = Bring the time slider into view
gantt-inspector-help = Click the ruler or drag the slider to move the time; ← and → step an hour, with Shift a day.
inspector-next-hour = Next hour (→)
inspector-previous-hour = Previous hour (←)
inspector-no-schedule = Nothing to show until the schedule is calculated.
inspector-beyond = The schedule is calculated to { $end }. Move the slider back to inspect it.
inspector-loaders = LOADERS
inspector-stockpiles = STOCKPILES
inspector-crushers = CRUSHERS
inspector-dumps = DUMPS
inspector-trucks = TRUCKS
charts-trucks = Trucks
inspector-rate = { $rate } t/h
inspector-delay = Delay
inspector-delay-of = Delay · { $kind }
inspector-tonnes = { $tonnes } t
inspector-pile-building = Building +{ $rate } t/h
inspector-pile-reclaiming = Reclaiming −{ $rate } t/h
inspector-pile-standing = Standing
inspector-pile-full = Full
inspector-pile-resting = Resting until { $until }
inspector-pile-capacity = { $percent }% of { $capacity } t capacity
inspector-not-fed = Not fed
inspector-crusher-at-limit = Daily limit reached
inspector-not-receiving = Not receiving
inspector-crusher-today = Today { $tonnes } t
inspector-crusher-today-of = Today { $tonnes } of { $limit } t
inspector-crusher-outside = { $grade } today, outside its band
inspector-crusher-feed-grade = This hour: { $grades }
inspector-crusher-day-grade = Day: { $grade } (band { $lower } – { $upper }, target { $target })
inspector-dump-to-date = { $tonnes } t received so far
charts-no-schedule = Charts appear once the schedule is calculated.
charts-no-destinations = There are no stockpiles, crushers or dumps to chart. Add destinations in Setup.
charts-inventory = Inventory
charts-feed = Feed
charts-grade = { $grade } grade
charts-received = Received to date
charts-nothing-received = Nothing received in this schedule
charts-grade-day = { $grade } for the day
charts-inventory-help = What the stockpile holds, against its capacity (dashed). Days its Mode stops building or reclaiming are tinted: blue build only, green reclaim only, grey off. The figure is at the time slider.
charts-feed-help = Tonnes fed to the crusher, hour by hour. The dashed line is the rate that would meet each day's limit. Gaps are hours it was not fed - often because the day's limit was reached. The figure is at the time slider.
charts-grade-help = The feed's grade hour by hour (line), the day's target band (green) and the day's blend (bar) - white inside the band, orange outside. The band prices the day's blend, so single hours may stray. The figure is the day's blend at the time slider.
charts-received-help = Everything the dump has received from the start of the schedule. The figure is at the time slider.
charts-capacity = Capacity { $tonnes } t
charts-mode = Mode: { $mode }
charts-feed-hour = This hour: { $rate } t
charts-feed-day = Day: { $tonnes } t
charts-feed-day-of = Day: { $tonnes } of { $limit } t
charts-grade-hour = This hour: { $grade }
inspector-trucks-in-use = { $busy } of { $fleet } in use
inspector-trucks-help = Trucks hauling at this instant, of the fleet available today after availability and utilisation.
planning-stat-sum = Sum
planning-stat-min = Min
planning-stat-max = Max
planning-blast-name-taken = { $blast } already exists
planning-blasts = Blasts
planning-min-blast-area = Minimum blast
planning-min-blast-area-help = Blasts smaller than this are left out of mining and get no name.
planning-min-strip-area = Minimum strip
planning-min-strip-area-help = Dig blocks smaller than this are left out of mining. They are still cut from the solid, so the blocks add up to their benches.
planning-blasts-empty = Select a bench to see its blasts
planning-blasts-one-bench = Select one bench to draw cuts
planning-dig-one-flitch = Select one flitch to draw strips
planning-blast-count-under =
    { $count ->
        [one] { $count } blast
       *[other] { $count } blasts
    } · { $under } too small
planning-copy-cuts-button =
    { $count ->
        [0] Copy lines to…
        [one] Copy { $count } line to…
       *[other] Copy { $count } lines to…
    }
planning-copy-cuts-title-benches = Copy lines to benches
planning-copy-cuts-title-flitches = Copy lines to flitches
planning-copy-cuts-confirm = Copy to { $count }
planning-copy-cuts-done-benches =
    { $targets ->
        [one] Copied { $lines } lines to { $targets } bench
       *[other] Copied { $lines } lines to { $targets } benches
    }
planning-copy-cuts-done-flitches =
    { $targets ->
        [one] Copied { $lines } lines to { $targets } flitch
       *[other] Copied { $lines } lines to { $targets } flitches
    }
planning-blast-count =
    { $count ->
        [one] { $count } blast
       *[other] { $count } blasts
    }
planning-dig-strips = Dig Strips
planning-reserving = Reserving
planning-reserving-item = Item
planning-reserving-finding = Finding
planning-reserving-not-run = Run this step to cut the dig blocks out of the solids and measure their reserves.
planning-reserving-stale = The strips or the ground have changed since the blocks were cut. Run this step again to cut and measure them.
planning-reserving-empty = No dig blocks yet.
planning-reserving-complete = { $blocks } dig blocks cut and measured. Inspect them in Reserves.
planning-dig-blocks = Dig Blocks
planning-dig-blocks-empty = Select a flitch to see its dig blocks
planning-dig-block = Block
planning-dig-blast-group = Blast { $name }
planning-dig-block-title = Block { $name }
planning-dig-block-count =
    { $count ->
        [one] { $count } block
       *[other] { $count } blocks
    }

## Reserve field diagnostics

reserve-issue-unmapped = Not mapped to a column or constant on this block model
reserve-issue-constant = The constant mapped onto this field is not a finite number
reserve-issue-missing-column = Column "{ $column }" is not in this block model
reserve-issue-wants-category = Column "{ $column }" is numeric; a category field needs a categorical column
reserve-issue-wants-numeric = Column "{ $column }" is categorical; this field needs a numeric column
reserve-issue-not-resident = Column "{ $column }" is listed but its values are not loaded
reserve-issue-length = Column "{ $column }" has { $values } values for { $blocks } blocks
reserve-issue-weight-missing = The weighting field is no longer in the Field List
reserve-issue-weight-not-summed = Weighting field "{ $name }" must be a summed field
reserve-issue-weight-unresolved = The weighting field could not be resolved on this block model
reserve-issue-all-missing = Every block in range was missing a value for this field
reserve-issue-model-excluded = This block model is excluded from the project's reserves. Tick Used for reserving for it in Planning → Block Models.
reserve-average = Average
reserve-average-by-volume = Average by volume
reserve-block-volume = Block volume
reserve-weights-others = Other fields average by this one, so it stays a Sum.
reserve-new-field = New Field
reserve-field-name-hint = e.g. Fe
reserve-kind-sum-note = Added up across blocks: tonnes, volume.
reserve-kind-average-note = Averaged over blocks by volume or a Sum field: grades, density.
reserve-kind-category-note = A label per block: rock type.
reserve-weighted = Weighted by
reserve-add-field = Add Field
planning-reserve-capacity-only = Geometry only · no block model assigned, so there is no measured content

## Solids pipeline stages

stage-state-not-run = Not run
stage-stale-hint = Edited since its last run. The steps after it keep their earlier results until it reruns.
planning-auto = Auto
setup-auto-note = Reruns out-of-date steps and the schedule after each edit.
stage-auto-holds = Auto is on. Switch it off to run by hand.
schedule-auto-holds = Auto is on. Switch it off to run by hand.
stage-state-stale = Stale
stage-state-blocked = Blocked
stage-state-queued = Queued
stage-state-running = Running
stage-state-complete = Complete
stage-state-failed = Failed
stage-state-cancelled = Cancelled
stage-blocked-by = Run { $stage } first
stage-run-stopped-by-edit = Run stopped: the { $stage } step's inputs changed while it was running
stage-failed-count =
    { $count ->
        [one] { $count } problem to resolve
       *[other] { $count } problems to resolve
    }
stage-duplicate-field = Two fields share this name
stage-model-data-gaps = { $missing } blocks missing a value · { $weights } with no usable weight
stage-waiting-models = Waiting on { $count } block models
stage-counting-dig-blocks = Counting dig blocks…
stage-waiting-solids = Waiting on { $count } solids
stage-no-occupied-bands = The benching plan reaches no material in this solid
stage-run-step = Run
stage-run-all = Run All
stage-cancel = Cancel Run
stage-diagnostics = Diagnostics
planning-reserve-coverage = Model coverage
planning-reserve-coverage-note = Block-model volume measured into the shown geometry, as a share of its geometric volume
planning-reserve-coverage-partial = The block model reaches only this share of the shown ground, so the tonnes and grades below are for that part.
stage-block-not-closed = Dig block { $block } ({ $area } m²) did not come out closed, so it has no volume
stage-volume-mismatch = { $blocks } dig blocks total { $children } m³ against the bench's own { $parent } m³
stage-duplicate-block-id = Two dig blocks share identity { $id }
stage-block-no-blast = Dig block { $block } lies in no blast of its bench
planning-stat-not-scanned = Not scanned yet · run the Block Models stage
planning-stat-scanning = Scanning…
planning-stat-loading = Loading block model…
planning-stat-scan-failed = Scan failed · { $error } · right-click the model to recompute
planning-recompute-stats = Recompute Statistics
planning-snapshot-incomplete = { $solid } has no completed geometry for the current inputs
planning-snapshot-failed = { $solid } failed: { $message }
stage-bench-no-children = A bench holding { $volume } m³ produced no dig blocks
stage-child-unmeasured = Dig block { $block } has no reserve measurement
stage-blocks-mismatch = Dig blocks total { $children } equivalent blocks against the bench's own { $parent }
stage-coverage-mismatch = Dig blocks cover { $children } m³ against the bench's own { $parent } m³
stage-field-lost = A reserve field measured on the bench is absent from its dig blocks
stage-field-mismatch = { $field }: dig blocks total { $children } against the bench's own { $parent }
stage-category-mismatch = Category { $category }: dig blocks hold { $children } equivalent blocks against the bench's own { $parent }
planning-not-run = Not run · use Run All to build this solid's benches and dig blocks
planning-reserve-partial = Partial · { $contributed } blocks contributed, { $missing } could not
planning-reserve-none-contributed = No block in this ground carried a value ({ $missing } missing)
planning-block-replaces = Replaces { $ids }
planning-snapshot-no-project = No project is open
planning-snapshot-not-run = { $stage } has not been run
planning-snapshot-stale = { $stage } is out of date; run it again
planning-snapshot-running = { $stage } is still running
planning-snapshot-unmeasured = Dig block { $block } has no reserve measurement
stage-missing-reference = The bench has no independent reserve measurement to reconcile against
stage-field-invented = The dig blocks report a reserve field the bench does not measure

## Trucks and trucking rules

truck-error-invalid-payload = Payload must be a positive number of tonnes.
truck-error-invalid-speed = Speed must be a positive number of km/h.
truck-error-invalid-units = Units must be a whole number of trucks.
truck-error-unknown-class = That truck class is no longer in this project.
truck-error-unknown-rule = That trucking rule is no longer in this project.
truck-error-class-in-use = Still used by { $rules }.
truck-error-unrepresentable = Those settings produce a haulage figure no number can express.
truck-classes = Truck Classes
truck-rules = Trucking Rules
truck-rule = Trucking rule
truck-new-class = New Truck Class
truck-duplicate-class = Duplicate Truck Class
truck-delete-class = Delete Truck Class
truck-no-classes = No truck classes yet
truck-payload-column = Payload
truck-default-class-name = HT
truck-payload = Payload
truck-default-fleet = Default fleet size
truck-units-suffix = trucks
truck-default-fleet-help = How many trucks the class has on any day the Calendar does not set otherwise.
truck-open-calendar = Open in Calendar
truck-new-rule = New Trucking Rule
truck-duplicate-rule = Duplicate Trucking Rule
truck-delete-rule = Delete Trucking Rule
truck-select-rule = Select a trucking rule.
truck-default-rule-name = Trucking rule
truck-rule-enabled = Enabled
truck-rule-disabled = Disabled
truck-rule-loaders = Loaders
truck-rule-sources = Sources
truck-rule-destinations = Destinations
truck-rule-classes = Truck classes
truck-rule-all-destinations = All destinations
truck-rule-stockpiles = Stockpiles
truck-rule-ground = Pit ground
truck-rule-no-classes = Add a truck class first.
truck-rule-class-missing = A truck class this rule names is gone.
truck-rule-destination-missing = A destination this rule names is gone.
truck-stage-no-classes = No truck classes are configured. Trucks do not constrain this schedule.
truck-rule-no-fleet = This rule's truck classes have no trucks, so nothing can be hauled under it.
truck-stage-no-rules = No trucking rules are configured, so no truck class is permitted anywhere.
schedule-calendar-trucks = Trucks
truck-calendar-units = Units
truck-calendar-availability = Availability (%)
truck-calendar-utilisation = Utilisation (%)

## Cashflow

cashflow = Cash flow
cashflow-error-invalid-value = Value must be a number.
cashflow-error-unknown-rule = That cash flow rule is no longer in this project.
cashflow-error-unrepresentable = Those values produce a figure no number can express.
cashflow-activity-all = Any activity
cashflow-activity-dig = Dig
cashflow-activity-reclaim = Reclaim
cashflow-activity = Activity
cashflow-rule = Cash flow rule
cashflow-rule-matches-all = Every movement
cashflow-rule-destinations-count = { $count } destinations
cashflow-rule-enabled = Enabled
cashflow-rule-disabled = Disabled
cashflow-rule-value = Value per tonne
cashflow-rule-loaders = Loaders
cashflow-rule-sources = Sources
cashflow-rule-destinations = Destinations
cashflow-rule-conditions = Conditions
cashflow-rule-all-destinations = All destinations
cashflow-new-rule = New Cash Flow Rule
cashflow-duplicate-rule = Duplicate Cash Flow Rule
cashflow-delete-rule = Delete Cash Flow Rule
cashflow-select-rule = Select a cash flow rule.
cashflow-default-rule-name = Cash flow rule
cashflow-help = Matching rules add together. Values guide optimisation; they do not guarantee movement priority.
cashflow-stage-no-rules = No cash flow rules are configured. Movements are worth nothing.
cashflow-stage-zero-value = { $rule } describes movements but pays nothing.

## Stockpile inventory and reclaim

inventory-order-fifo = FIFO (oldest first)
inventory-order-lifo = LIFO (newest first)
inventory-error-invalid-tonnes = Tonnes must be a number above zero.
inventory-error-invalid-value = That value is not a number, or the category is blank.
inventory-error-duplicate-value = That field already has a value on this portion.
inventory-error-unknown-lot = That parcel is no longer in this stockpile.
inventory-error-unknown-portion = That portion is no longer in this parcel.
inventory-error-last-portion = A parcel needs at least one portion. Delete the parcel instead.
inventory-error-lot-at-end = That parcel is already at the end of the order.
inventory-error-over-capacity = Opening inventory would exceed this stockpile's capacity.
inventory-error-stockpile-full = This stockpile is at its capacity.
inventory-error-overdrawn = That parcel does not hold that many tonnes.
inventory-error-invalid-interval = That is not a model interval.
inventory-reclaim-order = Reclaim order
inventory-opening = Opening inventory
inventory-no-lots = The stockpile starts empty.
inventory-new-lot = New parcel
inventory-duplicate-lot = Duplicate Parcel
inventory-delete-lot = Delete Parcel
inventory-move-lot-older = Move Towards Oldest
inventory-move-lot-newer = Move Towards Newest
inventory-default-lot-name = Parcel
inventory-lot = Opening parcel
inventory-chunk-column = Parcel (oldest first)
inventory-lot-name = Name
inventory-lot-tonnes = Tonnes
inventory-portion = Portion
inventory-portions = Portions
inventory-new-portion = New Portion
inventory-delete-portion = Delete Portion
inventory-portion-missing = —
inventory-select-lot = Select an opening parcel.
inventory-stage-over-capacity = { $stockpile } opens with more than its capacity.
inventory-stage-invalid-fields = { $count } opening-inventory properties refer to missing fields or incompatible field types.
inventory-field-missing = Missing field #{ $id }
inventory-field-incompatible = { $field } (incompatible type)
inventory-field-incompatible-note = The saved value is preserved, but its field type has changed.

## Reclaim bars

destination-error-category-unsupported = Categories are not retained in blended stockpiles, so this rule cannot test one.
destination-condition-category-blocked = Stockpiles don't keep categories, and this rule only takes from stockpiles. Add a pit, bench or flitch to its Sources to test a category.
destination-condition-category-narrows = Stockpiles don't keep categories, so this rule will take from the pits only.
destination-sources-category-note = Stockpiles don't keep categories, so they can't be sources of a rule that tests one.
destination-rule-category-conflict = This rule tests a category on material it may reclaim: { $fields }.
reclaim-error-not-a-stockpile = A reclaim bar's source has to be a stockpile.
reclaim-error-invalid-limit = Maximum tonnes must be a number above zero, or blank.
reclaim-error-wrong-activity = That edit does not apply to this kind of bar.
delay-error-unknown = That delay type, delay list or roster is no longer in this project.
delay-error-type-in-use = This delay type is still used by { $users }. Change those first.
delay-bar-unnamed = a delay bar
date-picker-month-year = { $month } { $year }
date-picker-mon = Mo
date-picker-tue = Tu
date-picker-wed = We
date-picker-thu = Th
date-picker-fri = Fr
date-picker-sat = Sa
date-picker-sun = Su
date-picker-january = January
date-picker-february = February
date-picker-march = March
date-picker-april = April
date-picker-may = May
date-picker-june = June
date-picker-july = July
date-picker-august = August
date-picker-september = September
date-picker-october = October
date-picker-november = November
date-picker-december = December
periods-step = Periods
periods-dates = Dates
periods-start-date = Start date
periods-end-date = End date
periods-choose-date = Choose a date…
periods-needs-start = Choose a start date first
periods-end-before-start = The end date must be on or after the start date
periods-no-start-note = Without a start date the schedule reads in day numbers only.
periods-list = Periods
periods-period = Period
periods-date = Date
periods-colour = Colour
periods-selected = { $count } selected
periods-clear-colour = Clear Colour
periods-day-date = Day { $day } · { $date }
delay-step = Delays
delay-types = Delay Types
delay-types-note = What kinds of delay there are, and the colour the Gantt draws each in.
delay-no-types-note = No delay types yet. Add one under Delay Types on the left.
delay-type = Delay Type
delay-type-default = Delay type
delay-untyped = No type
delay-colour = Colour
delay-new-type = New Delay Type
delay-delete-type = Delete Delay Type
delay-lists = Delay Lists
delay-list-default = Delays
delay-list-row = { $title } · { $count }
delay-new-list = New Delay List
delay-delete-list = Delete Delay List
delay-list-title = Title
delay-entries = Delays
delay-machine = Machine
delay-start = Start
delay-end = End
delay-hours = Hours
delay-list-empty = No delays in this list yet.
delay-add-entry = Add Delay
delay-paste-rows = Paste Rows…
delay-paste-hint = Paste machine, start and end columns from a spreadsheet (Ctrl+V over the table). Times read like 17/04/2026 06:00, Day 3 06:00, or a number of hours.
delay-paste-example = EX7001, Day 2 06:00, Day 2 18:00
delay-paste-add = Add Rows
delay-paste-cancel = Cancel
delay-paste-line = Line { $line }: { $problem }
delay-paste-columns = needs machine, start and end columns
delay-paste-machine = no machine called { $name }
delay-paste-start = cannot read the start { $text }
delay-paste-end = cannot read the end { $text }
delay-paste-order = the end is not after the start
delay-instant-hint = Write a time like 17/04/2026 06:00, Day 3 06:00, or a number of hours from the start. Dates need a start date on the Periods step.
delay-rosters = Rosters
delay-roster-default = Shift change
delay-new-roster = New Roster
delay-delete-roster = Delete Roster
delay-roster-note = A delay that repeats, such as a daily shift change.
delay-roster-first = First starts
delay-roster-duration = Lasts (h)
delay-roster-every = Repeats every (h)
delay-roster-until = Until
delay-roster-duration-hint = A number of hours, at least one minute (0.017 h).
delay-roster-every-hint = A number of hours, at least 1.
delay-roster-until-hint = the end of the schedule
delay-roster-machines = Machines
delay-roster-all = All machines
delay-roster-invalid = A roster's delay must last at least a minute, repeat at most hourly and be shorter than its repeat, and its end must come after its first start.
delay-add-bar = Add Delay Bar
planning-excluded-label = { $name } · excluded
planning-exclude-from-mining = Exclude from Mining
planning-topography-update = Update Topography
planning-topography-update-action = Update Topography…
planning-topography-update-note = Measure several solids against a new topography at once, such as this week's as-mined survey.
planning-topography-update-solids = Solids
planning-topography-update-row = { $name } (now { $current })
planning-topography-update-apply = Apply
planning-blast-cuts = Blast cuts
planning-double-click-rename = Double-click to rename
planning-rename-blast = Rename Blast
planning-reset-blast-name = Reset Blast Name
planning-rename-field = Rename Field
planning-delete-solid-title = Delete { $name }?
planning-delete-solid-note = Its benches, blasts, cut lines and dig strips go with it, and bars and rules that use its ground stop working. This can't be undone.
reserve-field-in-use = Still used by { $users }. Change those first.
reserve-field-used-tonnage = the Tonnage field in Configuration
reserve-field-used-grades = Grades carried in Configuration
reserve-field-delete-title = Delete { $name }?
reserve-field-delete-note = Block models stop mapping a column to it, and grade targets and opening stock values on it are removed. This can't be undone.
planning-delete-field = Delete Field
planning-no-block-models-project = No block models in this project
planning-import-block-model = Import Block Model
planning-extents = Extents: { $lower } → { $upper }
planning-constant = Constant
planning-used-reserving = Used for reserving
planning-source = Source
planning-stat-sum-avg = Sum / Avg
planning-block-count = { $blocks } blocks
planning-columns-heading = Columns
planning-per-m3-heading = Per m³ × block volume
planning-rename-solid = Rename Solid
planning-delete-solid = Delete Solid
planning-set-block-model-reserve = Set a block model to reserve this pit
planning-save-solid-project = Save Solid to Project
planning-set-surface-inspect-solid = Set a surface to inspect this solid
planning-load-solid-surfaces-inspect = Load this solid's surfaces to inspect it
planning-loading-solid-surfaces = Loading this solid's surfaces…
planning-rebuilding-solid = Rebuilding solid…
planning-building-solid = Building solid…
planning-solid-volume = { $volume } m³
planning-surface-only-other-surface = Surface only · the other surface is not loaded
planning-surface-only = Surface only
planning-no-solid-selected = No solid selected
planning-top-rl = Top RL
planning-base-rl = Base RL
planning-bench-column = Bench
planning-flitch-column = Flitch
planning-no-ranges = No ranges yet
planning-flitch-styles = Flitch Styles
planning-fill = Fill
planning-height = Height
planning-insert-range-below = Insert Range Below
planning-delete-range = Delete Range
planning-add-range = Add Range
planning-top-flitch = Top
planning-bottom-flitch = Bottom
# A flitch position between the top and bottom one, counted from the top.
# English ordinals: st/nd/rd listed by number, th for everything else.
planning-flitch =
    { $index ->
        [2] { $index }nd
        [3] { $index }rd
        [21] { $index }st
        [22] { $index }nd
        [23] { $index }rd
        [31] { $index }st
        [32] { $index }nd
        [33] { $index }rd
        [41] { $index }st
        [42] { $index }nd
        [43] { $index }rd
        [51] { $index }st
        [52] { $index }nd
        [53] { $index }rd
        [61] { $index }st
        [62] { $index }nd
        [63] { $index }rd
       *[other] { $index }th
    }
planning-bench-not-whole-number = A { $bench } m bench is not a whole number of { $flitch } m flitches
planning-pattern = Pattern
planning-results = Results
planning-no-bench-holds-any = No bench holds any of this solid
schedule-capture-excluded = { $bar }: { $count } excluded blocks skipped
schedule-capture-empty = { $bar }: { $count } blocks of 0 t dug at once
gantt-palette-dig = Dig
gantt-palette-reclaim = Reclaim
gantt-palette-delay = Delay
gantt-palette-hint = Drag onto a machine's row to add a { $item } bar there.
gantt-delay-type-title = Delay type
gantt-delay-types-setup = Set up delay types…
gantt-delay-heading = { $kind }: { $source }
gantt-delay-calendar-note = The schedule gives this machine no work here.
gantt-delay-bar-note = Holds the machine while it is the highest-priority bar open. Bars in higher lanes still work through it.
gantt-delay-change-type = Delay Type
reclaim-bar-default-name = Reclaim { $stockpile }
reclaim-bar-default-name-several = Reclaim ({ $count } stockpiles)
reclaim-add-bar = Add Reclaim Bar
reclaim-add-dig-bar = Add Dig Sequence
reclaim-edit-bar = Edit Reclaim
reclaim-source = Permitted stockpiles
reclaim-source-choose = Choose stockpiles
reclaim-source-required = Choose at least one stockpile.
reclaim-sources-summary = { $count } stockpiles
reclaim-sources-help = The optimiser may take from any of these. Their order is not a preference.
reclaim-sources-tooltip = Permitted: { $stockpiles }
reclaim-loader = Loader
reclaim-loader-choose = Choose a loader
reclaim-maximum = Maximum tonnes
reclaim-maximum-none = No limit
reclaim-maximum-value = Maximum: { $tonnes } t
reclaim-maximum-unlimited = Maximum: no limit
reclaim-source-unresolved = That stockpile is no longer in this project.
reclaim-no-stockpiles = No stockpiles to reclaim from.
schedule-calendar-reclaim-rate = Reclaim rate (t/h)
schedule-calendar-dig-rate = Dig rate (t/h)
schedule-class-reclaim-rate = Reclaim rate (t/h)
schedule-spot-time-invalid = Enter zero or more seconds

# Schedule optimisation settings
experiment-error-invalid-setting = That value is not one this setting accepts.
experiment-representation-blended = Blended pile
experiment-representation-chunks = Ordered blended parcels
experiment-advanced = Advanced
schedule-capture-event-capacity = Event capacity must be between 1 and 24, or left blank for the derived capacity.
experiment-end-day = Horizon (days)
experiment-end-day-help = Run All Periods solves from hour zero to the end of this day, and Run Period never extends past it. Open-ended bars stop here.
experiment-interval = Calendar interval (h)
experiment-interval-help = The interval sets how finely the calendar is split and when stockpile receipts become reclaimable. It is not a limit of one dig block per interval: a loader may work through several sources inside one interval.
experiment-solve-seconds = Solve time limit (s)
experiment-relative-gap = Relative gap target (%)
experiment-grades = Grades carried
experiment-grades-none = None
experiment-no-grade-fields = No tonnes-weighted average field is defined, so no blend grade can be tracked.
experiment-grades-choose-tonnage = Choose a Tonnage field first
experiment-representation = Representation
experiment-blended-help = Reclaim uses the pile's tonnes-weighted average grades. Opening parcels are combined into one blend, so there is no reclaim order.
experiment-chunks-help = The pile is divided into parcels of the parcel size, filled one at a time, opening stock first, each with its own composition. A partly filled parcel keeps receiving until it is full, even across days the pile is not building. Under FIFO the oldest parcel is reclaimed first, and under LIFO the newest, so a partly reclaimed newest parcel is topped up when building resumes. An emptied parcel's place is reused, and the pile's maximum tonnes limit what it holds at once.
experiment-chunk-size = Parcel size

# Schedule calculation: capture refusals
schedule-capture-horizon = The requested horizon is not a finite, positive number of hours.
schedule-capture-interval = The calendar interval is not finite and positive.
schedule-capture-block-not-in-run = A dig block in this bar is not in the Solids run the schedule reads.
schedule-capture-opening-grade-missing = '{ $grade }' is missing on an opening portion; a missing grade cannot be read as zero.
schedule-capture-grade-negative = '{ $grade }' is { $value }, which must be a finite, non-negative numeric grade.
schedule-capture-opening-over-capacity = Opening stock of { $opening } t exceeds the pile capacity of { $capacity } t.
schedule-capture-lots-combined = { $stockpile }: { $count } opening parcels combined into one blend; reclaim order does not apply.
schedule-capture-block-uncaptured = Nothing was captured about this block's material, so its blend cannot be computed.
schedule-capture-block-no-tonnage-field = The tonnage field is not among this block's captured values.
schedule-capture-block-no-tonnes = This block measured no tonnes of the nominated field.
schedule-capture-grade-uncaptured = '{ $grade }' was not captured for this block, and a blend cannot be computed without it.
schedule-capture-grade-missing = '{ $grade }' is missing on some of this block's material; a missing grade cannot be read as zero.
schedule-capture-calendar-non-finite = { $field } is not a finite number
schedule-capture-calendar-non-positive = { $field } must be greater than zero
schedule-capture-calendar-window = a work window ends before it starts
schedule-capture-calendar-intervals = its time steps do not line up
schedule-capture-field-horizon = the horizon
schedule-capture-field-interval = the calendar interval
schedule-capture-field-cashflow = the cash flow total
schedule-capture-field-other = a calendar setting
schedule-capture-calendar = The schedule calendar could not be built: { $reason }
schedule-capture-reclaim-source-gone = A permitted reclaim source is no longer a stockpile in this project.
schedule-capture-portion-unrouted = No enabled destination rule accepts this part of the block, and the rest of the block cannot be dug without it.
schedule-capture-reclaim-unrouted = No enabled destination rule accepts material reclaimed from this stockpile.
schedule-capture-no-truck = No compatible truck class serves this route, so nothing can be hauled on it.
schedule-capture-condition-category = Category conditions on '{ $field }' cannot be tested once material is blended.
schedule-capture-condition-untracked = '{ $field }' is not one of the grades carried, so it cannot be tested on reclaimed material.
schedule-capture-condition-unbounded = A condition on '{ $field }' with neither bound is not a condition.
schedule-capture-condition-bound = The '{ $field }' bound { $value } must be finite and non-negative.
schedule-capture-block-fallback = block { $id }
schedule-capture-portion = { $block } (material { $number })
schedule-capture-field-fallback = field { $id }
grade-rejection-unknown-field = Reserve field { $id } is not defined on this project.
grade-rejection-categorical = '{ $field }' is a category field, not a grade; a blended stockpile needs a numeric tonnes-weighted grade.
grade-rejection-summed = '{ $field }' is a summed quantity, not a grade; blending it would average a total. Use the weighted-average field it is a total of.
grade-rejection-foreign-weight = '{ $field }' is weighted by '{ $weight }', but the schedule's tonnage field is '{ $wanted }', so grade × tonnes is not its contained quantity.
grade-rejection-no-tonnage = The schedule has no tonnage field, so no grade's weighting can be checked.
grade-rejection-missing = Material { $material } has no '{ $field }' value; a missing grade cannot be treated as zero in a blend.
grade-rejection-out-of-range = Material { $material }'s '{ $field }' value { $value } is outside the range its unit allows.
schedule-capture-too-many-intervals = The horizon splits into more than { $limit } calendar intervals. Shorten the horizon or widen the calendar interval.
schedule-capture-no-bars = There is no work yet. On the Gantt, use Add work to make a bar, then add dig blocks to it (right-click it, then Edit Sequence).
schedule-capture-no-work = No bar has work inside the horizon. On the Gantt, add dig blocks to a bar (right-click it, then Edit Sequence), or move a bar into the horizon.
schedule-capture-no-movement = No movement is permitted by the current rules, so there is nothing to schedule.
schedule-capture-rehandle = { $rule } permits moving reclaimed material into a stockpile. Rehandling between stockpiles is not modelled, so those movements were left out.
schedule-unblasted = Unblasted
schedule-dig-sequence = Dig sequence
schedule-leave-blank-use-ground = Leave blank to use the ground-derived name
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
common-cut-topology-pit-shell = Cut Topography with Pit Shell...
common-delete-collection = Delete Collection
common-delete-layer = Delete Layer
common-delete-product = Delete Product
common-delete-selection = Delete Selection
common-designs = Designs
common-discard-layer-changes = Discard Layer Changes
common-down = Down
common-drape-topology = Drape to Topography
common-easting = Easting
common-edit-object = Edit Object
common-edit-text = Edit Text
common-elevation = Elevation
common-exit-without-saving = Exit Without Saving
common-file-was-left-out-downhole = { $file } was left out of the downhole geophysics: { $error }
common-filter = Filter
common-fly-mode = Fly Mode
common-generate-contour-lines = Generate Contour Lines...
common-hide-all = Hide All
common-hide-selection = Hide Selection
common-ignore = Ignore
common-import-csv-block-model = Import CSV Block Model
common-import-dxf = Import DXF
common-incline-design-project = Incline Design project
common-join = Join...
common-join-point-clouds = Join Point Clouds
common-joined-cloud = Joined Cloud
common-layer = Layer
common-line = Line
common-line-weight = Line weight
common-link-geophysics = Link Geophysics...
common-load-drillholes-before-linking-geophysics = Load the drillhole dataset before linking geophysics to it
common-lock-all = Lock All
common-lock-selection = Lock Selection
common-m = m
common-max = Max
common-merge-shell-into-topology = Merge Shell into Topography
common-merge-shell-into-topology-ellipsis = Merge Shell into Topography...
common-move-collar = Move Collar
common-move-collection = Move to Collection
common-move-design = Move Design
common-move-selection = Move Selection
common-name-has-no-readable-size = { $name } has no readable size
common-new-product = New Product
common-no-block-models = No block models
common-no-design-layers = No design layers
common-no-drill-holes = No drillholes
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
common-relimit-line = Trim/Extend Line
common-remove-project = Remove Project
common-reset-view = Reset View
common-reveal-all = Reveal All
common-reveal-finder = Reveal in Finder
common-rotate-collar = Rotate Collar
common-save-exit = Save and Exit
common-set-initiation-point = Set Initiation Point
common-shape = Shape
common-shell = With Shell
common-slashes = Slashes
common-slice = Clip
common-slice-triangulation-z-range = Clip Triangulation by Elevation...
common-surface-contours = Surface Contours
common-text = Text
common-degree-suffix = °
common-tie-holes = Tie Holes
common-triangulations = Triangulations
common-trim-topology = Trim to Topography...
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

## Warning and error alerts

alerts-copied = Message copied to the clipboard
alerts-copy-details = Copy details
alerts-error = Error:
alerts-time-more-waiting =
    { $count ->
        [one] { $time } · { $count } more message waiting
       *[other] { $time } · { $count } more messages waiting
    }
alerts-warning = Warning:

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
drill-hole-cache-drill-hole-set-name-has = Drillhole set { $name } has { $count } holes and tie-ins, past the { $capacity } the selection highlight can carry: selecting the set as a whole still highlights it, selecting single holes will not
drill-hole-cache-drill-hole-set-name-stations = Drillhole set { $name }: { $stations } stations, { $before } segments merged to { $after }, { $cells } cells
drill-hole-choose-valid-closed-polyline = Choose a valid closed polyline
drill-hole-code-already-in-section = { $code } is already in working section { $section }.
drill-hole-code-outside-section-has-name = A code outside this section has that name. A section may share its name only with a code it holds.
drill-hole-colour-scale = Colour scale
drill-hole-count-codes = { $count } codes
drill-hole-count-codes-interval-no-logged = { $count } codes. An interval with no logged value stays white.
drill-hole-disc-diameter = Disc diameter
drill-hole-appearance-title = Drillhole Appearance: { $name }
drill-hole-drilled-diameter = Of drilled diameter
drill-hole-every-code-lists-already-another = Every code it lists is already in another working section.
drill-hole-every-interval-value-colour-field = Every interval with a value in the colour field is drawn as a disc this wide on the trace. Far away it is never narrower than a few pixels.
drill-hole-field = Field
drill-hole-dataset = Dataset
drill-hole-field-working-section = { $field } by working section
drill-hole-floor = Floor
drill-hole-grayscale = Greyscale
drill-hole-green-yellow-red = Green–Yellow–Red
drill-hole-heat = Heat
drill-hole-drilled-width-help = A hole at its drilled width reads as a pipe beside the geology; a set of thousands reads as a mat.
drill-hole-line-width-help = The hole itself is drawn as a line this wide at every zoom.
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
drill-hole-string-discs = Trace and discs
drill-hole-string-discs-where-intervals-overlap = As trace and discs, where intervals overlap the shortest one is drawn as the disc.
drill-hole-string-width = Trace width
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
drill-pattern-choose-closed-blast-boundary-then = Choose a closed blast boundary, then tune the grid. The drillholes update live in the viewport.
drill-pattern-rotation-help = Counter-clockwise pattern rotation from the global { $axis } axis.
drill-pattern-distance-between-holes-along-each = Distance between holes along each pattern row.
drill-pattern-name-hint = e.g. West Cut 03
drill-pattern-diameter-help = Finished hole diameter. Entered in millimetres and stored with every generated hole.
drill-pattern-hole-depth = Hole depth
drill-pattern-hole-diameter = Hole diameter
drill-pattern-move-over-closed-polyline-then = Move over a closed polyline, then click it in the viewport. Esc cancels the pick.
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
edit-save-project = Save
edit-delta-length-m-use = Delta length (m, use + or -)
edit-dip = Dip
edit-direction = Direction
edit-distance = Distance
edit-distance-along-slope = Distance along slope
edit-download-free-native-version-our = Download the free native version at our website ↗
edit-drill-hole = Drillhole
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = End
edit-enter-valid-elevation = Enter a valid elevation.
edit-exit-slice = Exit section
edit-finish-polyline = Finish Polyline
edit-generate-batter-berms = Generate Batter Berms
edit-height = Height
edit-height-change = Height change
edit-height-mode = Height mode
edit-horizontal-distance = Horizontal distance
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
edit-recent-functions = Recent functions
edit-no-recent-functions = No recent functions
edit-relative = Relative (+/-)
edit-elevation-mode-help = Relative applies a vertical change to every point. Absolute RL projects every point onto one target elevation.
edit-remove-from-list = Remove from List
edit-replace-path = Replace path
edit-rotate = Rotate
edit-rotation-speed-slice-when-using = Rotation speed of the section when using Q and E.
edit-s = °/s
edit-segments = Segments
edit-segments-lying-elevation-ignored = Segments lying at this elevation are ignored.
edit-selected-holes-point-different-ways = Selected holes point different ways. Apply sets them all to these angles.
edit-selected-start-end-point-moves = The selected start or end point moves along the line direction; the opposite endpoint stays fixed.
edit-set-axis = Set { $axis }
edit-shortest = Shortest
edit-slice-view = Section View
edit-slice-3d = 3D view
edit-slice-3d-help = Draw surfaces in 3D across the section width instead of as the line where they cross the section plane, and allow orbiting. Drillholes, polylines, points and block models show within the section width either way.
edit-slope-angle-each-batter-face = Measured from horizontal.
edit-slope-angle-offset-positive-negative = Slope angle of the offset. Positive and negative angles move the copy above or below the source as it moves sideways.
edit-speed = Speed
edit-start = Start
edit-stockpile = Stockpile
edit-stop-generated-offset-where-its = Stop the generated offset where its path first meets a visible triangulation.
edit-target-rl = Target RL
edit-thickness-visible-slice-slab-centred = Section width: the thickness of ground shown, centred on the section line.
edit-type = Type
edit-type-direction-together-set-offset = Type and Direction together set the offset side. Pit + Up and Stockpile + Down step outward; Pit + Down and Stockpile + Up step inward.
edit-bench-direction-help = Up raises each bench by the bench height; Down lowers it. This also flips the offset side - see Type.
edit-bezier-control-point-1-help = World X, Y and Z coordinates of the first Bezier control point.
edit-bezier-control-point-2-help = World X, Y and Z coordinates of the second Bezier control point.
edit-offset-cut = Offset Cut
edit-distance-between-cuts-picking = Distance between cuts. Picking a side fills the ground between the source and the cursor with cuts this far apart.

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
explorer-hide = Hide
explorer-load = Load
explorer-lock = Lock
explorer-new-collection = New Collection
explorer-no-collection = No Collection
explorer-delete-selected = Delete { $count } Items
explorer-remove-collection = Remove Collection
explorer-select-all-objects = Select All Objects
explorer-selected-count = { $count } Selected
explorer-show = Show
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
input-could-not-slice-name-error = could not clip { $name }: { $error }

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
io-drill-holes = Drillholes
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
io-fixed-block-size = Fixed block size
io-no-column-mapped-these = No column is mapped to these axes - every block uses this size instead.

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
menu-build-solid-surfaces = Build Solid from Surfaces...

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
object-edit-id = ID
object-edit-identity = Identity
object-edit-insert-after = Insert after
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
omf-damaged = This file is damaged or is not an Incline project
omf-mining-data-exported-incline = Mining data exported by Incline
omf-import = OMF import
omf-texture = OMF texture
omf-validation-warnings = OMF validation warnings: { $warnings }
omf-application-metadata-dropped = Project application metadata '{ $application }' is not retained
omf-project-author-not-retained = Project author is not retained
omf-project-description-not-retained = Project description is not retained
omf-unsupported-metadata-keys = Project has unsupported metadata keys: { $keys }
omf-optimization-scenarios-unreadable = The project's optimisation scenarios could not be read and were left out: { $error }
omf-skipped-drillhole-data-saved-older = Skipped drillhole data saved in an older layout ({ $names }); import it again from its source files

## Plot strings


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
point-cloud-name-count-points = { $name } ({ $count } points)
point-cloud-none-loaded = No point clouds are loaded.
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
point-cloud-join-sources-help = Tick the clouds to copy into the joined cloud. Two or more are needed.
point-cloud-classify-sources-help = Tick the clouds to classify. Each is classified on its own.
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
properties-developer = Developer
properties-downscale-rasters = Downscale rasters
properties-edit-object = Edit Object...
properties-field-view = Field of view
properties-fps = FPS
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
properties-orbit-sensitivity = Orbit sensitivity
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
properties-zoom-sensitivity = Zoom sensitivity

## Reference strings

reference-points-count-holes-from-dataset = { $count } holes from '{ $dataset }'
reference-points-holes-from-datasets = { $count } holes from { $datasets } datasets
reference-points-holes = Holes
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
reference-surface-extent-help = The selected closed polyline the finished surface is clipped to; points outside it still shape the surface.
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
state-click-closed-polyline-viewport = Click the closed polyline in the viewport.
state-click-surface-viewport = Click the surface in the viewport.
state-click-topology-viewport = Click the topography in the viewport.
state-close-project = Close Project
state-colour-drillholes = Colour Drillholes
state-colour-drillholes-working-section = Colour Drillholes by Working Section
state-copy-objects-layer = Copy Objects to Layer
state-count-cloud-s = { $count } cloud(s)
state-count-file-s = { $count } file(s)
state-count-object-s-axis-value = { $count } object(s) · { $axis } { $value }
state-count-object-s-closed = { $count } object(s) · { $closed }
state-count-objects = { $count } object(s)
state-count-object-s-weight = { $count } object(s) · { $weight }
state-count-object-s-z-elevation = { $count } object(s) · Z { $elevation }
state-points-controls-clipped = { $count } point(s) · { $controls } control polyline(s) · clipped to the extent polyline
state-points-controls-unclipped = { $count } point(s) · { $controls } control polyline(s) · unclipped
state-create-collection = Create Collection
state-create-point-cloud-tin = Create Point Cloud TIN
state-create-project = Create Project
state-current-project = Current project
state-cut-topology-pit-shell = Cut Topography to Pit Shell
state-cut-triangulation-polyline = Cut Triangulation by Polyline
state-cut-triangulation-z = Clip Triangulation by Elevation
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
state-move-axis-value = Move to Axis Value
state-move-objects-layer = Move Objects to Layer
state-name-count-cloud-s = { $name } · { $count } cloud(s)
state-name-count-holes = { $name } · { $count } holes
state-name-count-object-s = { $name } · { $count } object(s)
state-name-z-min-z-max = { $name } · { $z_min } to { $z_max }
state-new-collection-under-section = New collection under { $section }
state-next-edit = Next edit
state-north = North
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
state-individually-hidden-designs = Individually hidden designs
state-set-block-model-variable = Set Block Model Variable
state-set-cinematic-view = Set Cinematic View
state-set-drillhole-colour-preset = Set Drillhole Colour Preset
state-set-drillhole-discs = Set Drillhole Discs
state-set-drillhole-style = Set Drillhole Style
state-set-drillhole-width = Set Drillhole Width
state-set-entity-lock = Set Lock
state-set-grid = Set Grid
state-set-visibility = Set Visibility
state-set-layer-lock = Set Layer Lock
state-set-line-weight = Set Line Weight
state-set-object-colour = Set Object Colour
state-set-object-fill = Set Object Fill
state-set-point-visibility = Set Point Visibility
state-set-polyline-closed = Set Polyline Closed
state-set-raster-lock = Set Raster Lock
state-set-standard-view = Set Standard View
state-set-topology-contours = Set Topography Contours
state-set-topology-wireframes = Set Topography Wireframes
state-set-triangulation-colour = Set Triangulation Colour
state-show-console = Show Console
state-show-project = Show Project
state-shown = Shown
state-slice-mode = Section View
state-slice-preview = Section Preview
state-south = South
state-stem-contours = { $stem } Contours
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
status-points-chunks = Points: -- / -- of -- (--/-- chunks)

## Text strings

text-cjk-fonts-fetch-failed = Could not download the Chinese, Japanese and Korean fonts, so that text may show as boxes: { $error }
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
toolbars-fuse-polylines = Join Polylines
toolbars-measure-distance = Measure Distance
toolbars-new-layer = New Layer
toolbars-split-polyline-points = Split Polyline At Points
toolbars-strike-dip = Strike and Dip
toolbars-hide = Hide
toolbars-show-all-hidden = Show All Hidden
toolbars-tool-not-available-section-view = { $tool } - not available in the section view
toolbars-tool-needs-layer = { $tool } - make a layer first
web-startup-no-graphics = This browser could not use your graphics card through WebGPU. Try a current version of Chrome or Edge with hardware acceleration on, or the desktop app from inclinedesign.net.

## Tri strings

tri-sampling-method-help = Adaptive concentrates vertices on complex terrain via plane-fit error; uniform spreads them evenly. More methods may be added in future.
tri-adaptive-quadtree = Adaptive (quadtree)
tri-axis-range = { $axis } range
tri-base-topology-will-receive-pit = The base topography that will receive the pit or stockpile shape.
tri-boundary-polyline = Boundary polyline
tri-boundary-polyline-help = The closed polyline whose XY boundary defines the clipping area. Pick it in the viewport.
tri-bridge-gaps-help = Bridge gaps and boundary concavities narrower than this across the surface. 0 still bridges gaps up to roughly the sampling cell size; larger values fill bigger holes and erode boundary concavities.
tri-budget = Budget by
tri-cancel-pick = Cancel Pick
tri-candidate-detail = Candidate detail
tri-candidate-fine-cells-per-budgeted = Candidate fine cells per budgeted vertex. Higher gives the adaptive sampler more freedom to place detail, but is slower to build.
tri-choose-input-clicking-loaded-surface = Choose this input by clicking a loaded surface in the viewport
tri-choose-which-side-reference-topology = Choose which side of the reference topography to remove from the surface within their shared XY area.
tri-clip = Clip
tri-clip-creates-new-triangulation-name = The clip creates a new triangulation with this name; the source surface is not modified.
tri-clip-surface-polyline = Clip Surface by Polyline
tri-closed-pit-stockpile-solid-whose = A closed pit or stockpile solid whose exposed boundary will be included in the result.
tri-cloud-carries-no-classifications-so = This cloud carries no classifications, so every point is surfaced. Import a LAS/LAZ file that has been through a ground filter to reconstruct bare earth.
tri-cut-topology-pit-shell = Cut Topography with Pit Shell
tri-e-g-design-trimmed = e.g. design_trimmed
tri-e-g-mysurf-cut = e.g. mysurf_cut
tri-e-g-mysurf-slice = e.g. mysurf_clip
tri-e-g-surface-contour = e.g. surface_contour
tri-e-g-topo-cut = e.g. topo_cut
tri-e-g-topo-pit = e.g. topo_with_pit
tri-exact-number-surface-vertices-target = Exact number of surface vertices to target. Very large values build slowly and use significant memory.
tri-existing-ground-topology-will-cut = The existing topography that will be cut by the pit shell.
tri-fill-holes-up = Fill holes up to
tri-generate = Generate
tri-generate-contour-lines = Generate Contour Lines
tri-generate-upper-surface = Generate Upper Surface
tri-ground-points-only = Ground points only
tri-hide-unload-sources = Hide and unload sources
tri-higher-edge-will-enforced-each = The higher edge will be enforced at each conflict. Lower conflicting segments will be ignored as breaklines and the surface will interpolate through those areas. The source polylines are unchanged.
tri-breaklines-cross = The highlighted breakline edges cross or overlap in plan at different elevations. One terrain surface cannot follow both.
tri-intervals-colours = Intervals & colours
tri-keep-clipped-topology-included-shape = Keep the clipped topography and included shape as separate triangulations instead of combining them into one entity.
tri-keep-inside-discards-surface-outside = Keep inside discards surface outside the polyline. Keep outside cuts a polyline-shaped hole from the surface.
tri-keeps-only-surface-within-polyline = Keeps only the surface within the polyline boundary.
tri-keep-surface-relation-help = Keeps the surface { $relation } the topography within its XY coverage.
tri-layer-already-exists-select-above = That layer already exists; select it above or choose another name.
tri-limit-z-range = Limit elevation range
tri-major = Major
tri-max-edge-length = Max edge length
tri-merge = Merge
tri-method = Method
tri-minor = Minor
tri-contour-interval-help = Minor controls ordinary contours. Major controls emphasised contours and must use an interval at least as large as Minor.
tri-contour-labels = Contour labels
tri-contour-labels-help = Writes each major line's height on it as text, in the contour layer. A line too short for its label gets none.
tri-move-cursor-over-closed-polyline = Move the cursor over a closed polyline.
tri-move-cursor-over-loaded-surface = Move the cursor over a loaded surface.
tri-nearby-breakline-vertices-do-not = Nearby breakline vertices do not meet at exactly the same position, so the surface cannot be triangulated.
tri-new-layer = New layer
tri-new-layer-name = New layer name
tri-no-boundary-selected = No boundary selected
tri-only-loaded-pickable = Only loaded triangulations can be picked.
tri-operation = Operation
tri-output-layer = Output layer
tri-percentage = Percentage
tri-percentage-cloud = Percentage of cloud
tri-pick-from-view = Pick from View
tri-pit-design-surface-only-areas = The pit design surface. Only areas where it excavates below the topography are used for the cut.
tri-pit-shell = Pit shell
tri-pit-stockpile-solid = Pit/stockpile solid
tri-recommended-weld-retry = Recommended: Weld & Retry
tri-reconstruct-ground-only-help = Reconstruct from the points classified as bare earth, discarding vegetation, buildings, plant and noise. Turn this off to surface every point in the cloud.
tri-reconstruct-help = Reconstruct a triangulated terrain surface from a point cloud. The adaptive sampler spends the vertex budget where the ground is most complex and keeps planar areas sparse.
tri-reduce-budget-candidate-detail-if = Reduce the budget or candidate detail if your machine has less RAM.
tri-reference-topology-help = The reference topography that defines where the other surface is trimmed.
tri-reject-reconstructed-triangle-edges = Reject reconstructed triangle edges longer than this distance. Use 0 for no edge-length limit.
tri-remove-inside-help = Removes the surface within the polyline boundary and keeps the rest.
tri-removes-topology-where-pit-shell = Removes the topography where the pit shell excavates below it so the shell fills the hole. The seam follows the true 3D contact line between the surfaces; topography under parts of the shell that stand above the ground is kept.
tri-result = Result
tri-save-two-entities = Save as two entities
tri-select = Select…
tri-point-cloud-source-help = The point cloud whose points will be reconstructed into a terrain surface.
tri-contour-surface-help = The surface contour lines are generated from. Choose it from the list, or press Pick and click it in the viewport.
tri-clip-surface-help = The surface to clip. Choose it from the list, or press Pick and click it in the viewport.
tri-slice-surface-help = The surface whose elevation range will be clipped. Choose it from the list, or press Pick and click it in the viewport.
tri-share-source-points-keep-fractions = Share of source points to keep. Fractions such as 0.125% are allowed.
tri-slice-triangulation-z-range = Clip Triangulation by Elevation
tri-solution-generate-upper-surface = Solution: Generate Upper Surface
tri-surface-trim = Surface to Trim
tri-target-surface-help = The surface that will be changed; the selected topography is left intact.
common-percent-suffix = %
tri-topology = Topography
tri-triangulation-failed = Triangulation Failed
tri-trim = Trim
tri-trim-topology = Trim to Topography
tri-uniform-grid = Uniform grid
tri-unload-source-surface = Unload source surface
tri-unload-source-topology = Unload source topography
tri-up-target-point-count-points = Up to { $target } of { $point_count } points will become surface vertices ({ $percent }%).
tri-vertex-count = Vertex count
tri-vertices-within-5-cm-xy = Vertices within 5 cm in XY and Z will share one position for this triangulation. This can shift the generated surface locally by up to 5 cm; the source polylines are unchanged.
tri-weld-retry = Weld & Retry
tri-solid-role-design = design
tri-solid-role-topography = topography
tri-solid-no-volume-cut = These surfaces enclose nothing to dig: the design surface is nowhere below the topography. Check the topography has ground over the design surface; one with a hole cut out there leaves nothing above it.
tri-solid-no-volume-fill = These surfaces enclose nothing to fill: the design surface is nowhere above the topography. Check the topography has ground under the design surface; one with a hole cut out there leaves nothing below it.
solid-build-failed = { $solid } could not be built. { $reason }
tri-solid-repaired-surface = The { $role } surface folds over itself in plan or has hairline cracks between its faces, so the solid was built from a repaired copy of it: points less than 0.001 apart in plan were joined, edges were split where points of the surface lie on them, and wherever it folds the sheet on the outside of the solid was kept.
tri-solid-open-along-edge = Solid is open along { $count } edge(s): the two surfaces do not meet all the way round, so this is a shell between them rather than a closed solid. Its volume is still exact.
tri-built-solid-between = Built { $region } solid between '{ $design }' and '{ $topography }' · { $volume } m³
tri-building-solid-surfaces = Building solid from surfaces…
tri-build-solid-surfaces = Build Solid from Surfaces
tri-design-surface = Design Surface
tri-pit-shell-dump-design = The pit shell, dump design or stockpile design bounding the volume.
tri-ground-design-measured-against = The ground the design is measured against. Both surfaces are left intact.
tri-volume = Volume
tri-which-two-volumes-surfaces = Which of the two volumes the surfaces bound: the ground cut away below the design, or the material placed above it.
tri-encloses-over-area-two = Encloses { $region }, over the area the two surfaces share and closing along the line where they cross.
tri-north-pit-solid = e.g. north_pit_solid
tri-build = Build

## Ui strings

ui-choose-offset-side = Choose offset side
ui-choose-relimit-side = Choose the side to keep
ui-click-circle-centre = Click the circle centre
ui-click-closed-polyline-use-blast = Click a closed polyline to use as the blast shape
ui-click-collar-add-edit-initiation = Click a collar to add or edit an initiation point
ui-click-first-point-slice-line = Click the first point of the section line
ui-click-first-vertex = Click first vertex
ui-click-perimeter-point-type-radius = Click a perimeter point or type a radius
ui-click-second-point-slice-line = Click the second point of the section line
ui-click-second-vertex = Click second vertex
ui-click-use-pointer-radius = or click to use the pointer radius
ui-could-not-copy-text-browser = Could not copy text to the browser clipboard: { $error }
ui-dip-horizontal-no-strike = { $dip } (horizontal, no strike)
ui-distance-meters = { $distance } metres
ui-drag-ring-type-azimuth-dip = Drag a ring, or type an azimuth and dip
ui-each-hole-turns-about-its = each hole turns about its own collar
ui-enter-positive-decimal-radius = Enter a positive decimal radius
ui-esc-cancels = Esc cancels
ui-no-delay-product-tie = No delay product to tie with
ui-press-enter-use-typed-radius = Press Enter to use the typed radius
ui-right-click-delay-palette-heading = right-click the Delay Palette heading to add one
ui-select-designs = Select designs
ui-select-drill-hole = Select a drillhole
ui-select-endpoint-join = Select the endpoint to join
ui-select-first-crest-toe-point = Select first crest/toe point
ui-select-item = Select an item
ui-select-line-fuse = Select a line to join
ui-select-line-polyline = Select a line or polyline
ui-select-line-relimit = Select the line to trim or extend
ui-select-next-line-fuse = Select the next line to join
ui-select-opposite-berm-point = Select opposite berm point
ui-select-point = Select a point
ui-select-designs-to-hide = Click or drag over designs to hide them
ui-select-polyline = Select a polyline
ui-select-polyline-open-line = Select a polyline or open line
ui-select-polyline-vertex = Select a polyline vertex
ui-select-second-crest-toe-point = Select second crest/toe point
ui-select-second-split-point = Select second split point
ui-select-split-point = Select a split point
ui-select-topologies = Select topographies
ui-slice-view = Section view
ui-strike-dip = { $strike }° strike · { $dip }
ui-value-dip = { $value }° dip

## Viewport strings

viewport-1-1-true-shape = 1:1, true shape
viewport-1-ratio = 1:{ $ratio }
viewport-all-total-categories-keep-their = All { $total } categories keep their colour; only the first { $shown } are drawn distinctly
viewport-azimuth-dip = Azimuth { $azimuth }, dip { $dip }
viewport-back-whole-log = Back to the whole log.
viewport-bar-blast-timeline = Blast Timeline
viewport-bar-burden-relief-heatmap = Burden Relief Heatmap
viewport-bar-cinematic-view = Cinematic View
viewport-bar-color = Colour:
viewport-bar-contours-equal-time = Timing Contours
viewport-bar-disable-cinematic-view = Disable Cinematic View
viewport-bar-disable-flying-mode = Disable Fly Mode
viewport-bar-disable-x-ray-vision = Disable X-Ray Vision
viewport-bar-drill-holes = Drillholes:
viewport-bar-enable-flying-mode = Enable Fly Mode
viewport-bar-enable-x-ray-vision = Enable X-Ray Vision
viewport-bar-exit-slice-view = Exit Section View
viewport-bar-fill = Fill:
viewport-bar-fix-centre-rotation = Fix Centre of Rotation
viewport-bar-hide-borehole-inspector = Hide Drillhole Inspector
viewport-bar-hide-contours = Hide Contours
viewport-bar-hide-points = Hide Points
viewport-bar-hide-rl-grid = Hide RL Grid
viewport-bar-hide-wireframes = Hide Wireframes
viewport-bar-hide-xy-grid = Hide XY Grid
viewport-bar-release-centre-rotation = Release Centre of Rotation
viewport-bar-show-borehole-inspector = Show Drillhole Inspector
viewport-bar-show-contours = Show Contours
viewport-bar-show-points = Show Points
viewport-bar-show-rl-grid = Show RL Grid
viewport-bar-show-wireframes = Show Wireframes
viewport-bar-show-xy-grid = Show XY Grid
viewport-bar-vertical-slice-view = Vertical Section View
viewport-blank = (blank)
viewport-choose-variable = Choose a variable
viewport-click-edit-color-right-click = Right-click to remove
viewport-colour-mapping = Colour mapping
viewport-count-categories = { $count } categories
viewport-count-category = { $count } category
viewport-depth-m-hole-end = { $depth } m hole end
viewport-drag-spin-view-around-hole = Drag to spin the view around the hole. Double-click to face north.
viewport-e = E
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
viewport-bench = Bench:
viewport-flitch = Flitch:
viewport-labels = Labels

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
charging-preview-use-pattern-hole = Use the pattern's median hole
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
blast-tight-below = Tight below
blast-slack-above = Slack above
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
blast-duration-ms = of { $duration } ms
blast-real-time = Real time
blast-times-slower = { $factor }× slower
blast-mic-limit = MIC limit
blast-most-explosive-allowed-detonate-any-8 = The most explosive allowed to detonate in any 8 ms at this site. Windows over it are flagged.
blast-no-holes-loaded-surface-signal-plays = No holes are loaded: the surface signal plays, but nothing detonates. Load holes with the Charge Holes tool.
blast-now-holes-hole = Now: { $holes } hole(s)
blast-in-8-ms = in 8 ms
blast-peak-mass-kg-time-ms = Peak { $mass } kg at { $time } ms
blast-peak-holes-hole-time-ms = Peak { $holes } hole(s) at { $time } ms
blast-peak-over-limit = , { $over } kg over
blast-peak-within-limit = , within limit

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
state-volume-below-design-surface = the volume below the design surface and above the topography - a pit or cut
state-volume-above-design-surface = the volume above the design surface and below the topography - a dump or stockpile
state-click-design-surface-viewport = Click the design surface in the viewport.
state-click-topography-viewport = Click the topography in the viewport.
state-solids-preview = From the Solids preview

## Viewport messages (Drill & Blast)

ui-click-drag-over-holes-load-them = Click or drag over holes to load them with { $rule }
ui-hold-shift-unload = hold Shift to unload
ui-no-charge-rule-load = No charge rule to load with
ui-right-click-charge-rules-heading-add = right-click the Charge Rules heading to add one

## OMF warnings (Drill & Blast)

omf-element-name-has-count-charge-naming = Element '{ $name }' has { $count } charge(s) naming holes it no longer contains

## Solids strings
solids-building-solid-preview = Building solid preview…
solids-saved-solid-project = Saved solid '{ $name }' to the project · { $volume } m³
solids-missing-surface = Missing surface
solids-missing-block-model = Missing block model
solids-new-solid = New Solid
solids-pit-dump-design-itself = The pit or dump design itself
solids-topography = Topography
solids-surface-design-measured-against = The surface the design is measured against
solids-reserved-against-model = Reserved against this model
solids-optional-dumps-stockpiles = Optional for dumps and stockpiles
solids-add-solid = Add Solid
solids-no-solids-yet-add = No solids yet - add one in Setup
solids-multiple = Multiple…
solids-selection = Selection
solids-select-solid-bench-flitch = Select a solid, bench or flitch
solids-showing-unmeasured =
    { $count ->
        [one] Showing the 1 block with no tonnage
       *[other] Showing the { $count } blocks with no tonnage
    }
solids-blast = Blast
solids-plan-area = Plan area
solids-dig-block = Dig block
solids-dig-blocks = Dig blocks
solids-dig-blocks-selected = { $count } selected
planning-dig-blocks-title = { $count } dig blocks
solids-block-id = Block ID
solids-bench = In bench
solids-flitch = In flitch
solids-block-volume = Block volume
solids-bench-rl = Bench RL
solids-flitch-rl = Flitch RL
solids-contents = Contents
solids-view-geometry = Geometry

## Animate strings
animate-calculated-solids-unavailable-run = Calculated solids are unavailable. Run Solids through Reserving.
animate-updating-schedule-animation = Updating schedule animation
animate-block-volume-could-not = A block's volume could not be measured.
animate-block-retains-more-material = A block retains more material the further it is cut back.
animate-block-shape-could-not = A block's shape could not be cut to the depletion the schedule reports.

## Pit optimization

opt-scenarios-title = Optimisation scenarios
menu-pit-optimization = Pit Optimisation...
opt-scenarios-empty = No scenarios yet. Add one to set up a pit optimisation run, or import some from a JSON file.
opt-scenario-title = Optimisation scenario
opt-scenario-default-name = Scenario { $number }
opt-copy-suffix = copy
opt-add-scenario = Add scenario
opt-save-scenario = Save scenario
opt-delete-scenario = Delete scenario
opt-duplicate-scenario = Duplicate scenario
opt-scenarios-loaded = Loaded { $count } optimisation scenario(s)
opt-scenarios-save-failed = Could not save the optimisation scenarios: { $error }
opt-run = Run
opt-edit = Edit
opt-duplicate = Duplicate
opt-delete = Delete
opt-discard = Discard
opt-select = Select
opt-yes = Yes
opt-no = No
opt-unsaved-title = Unsaved changes
opt-unsaved-message = "{ $name }" has changes that are not saved. Save them before closing?

opt-status-never-run = Not run yet
opt-status-up-to-date = Run, and unchanged since
opt-status-stale = Changed since the last run - run it again
opt-status-running = Running
opt-status-running-percent = Running: { $percent }%

opt-section-inputs = Inputs
opt-constants-open = Constants
opt-constants-title = Constants
opt-constants-save = Save constants
opt-constants-summary = { $count } constants
opt-constants-saved = { $count } constants saved to the project. Every scenario and schedule uses them.
opt-constants-unsaved-message = The constants have changes that are not saved. Save them before closing?
opt-constants-note = One list for every scenario and schedule of the project. Changing a constant makes only the runs that use it stale.
opt-constants-merged-clash = Constants with the same name but different values were merged; the first value is kept: { $names }
opt-section-mining-costs = Mining costs
opt-section-processing-costs = Processing costs
opt-section-revenues = Revenues
opt-section-constraints = Constraints
opt-section-outputs = Outputs

opt-density-field = Density field
opt-quality-field = Main quality field
opt-rocktype-field = Rock type field
opt-none-selected = None
opt-block-model-not-loaded = This block model is not in the open project, so its fields cannot be listed.
opt-block-model-unloaded = This block model is not loaded. Load it (double-click it in the explorer) to check that it can be optimised.
opt-block-model-unloaded-hint = An unloaded block model keeps its field list, but its blocks are not read yet, so Incline cannot tell whether it is one regular, unrotated grid. Running the scenario asks to load it first and checks then.
opt-exclude-air = Exclude air blocks
opt-air-exclusion = Air exclusion
opt-air-use-topography = Topography surface
opt-air-use-rocktype = Rock type value
opt-air-source = Air from
opt-air-topography = Topography
opt-air-rocktype = Air rock type
opt-cost-source = Cost from
opt-cost-field = Cost field
opt-slope-source = Slope from
opt-slope-angle = Overall angle
opt-slope-angle-field = Angle field
opt-slope-fallback-angle = Fallback angle
opt-factor-input = Factors from
opt-group-directional = Directional shells
opt-reports-folder = Reports folder
opt-create-shell-solids = Create shell solids
opt-create-shell-surfaces = Create shell surfaces
opt-shell-field-value = Value written
opt-shell-field-target = Write into
opt-shell-field-name = Field
opt-any-rocktype = Any rock type

opt-col-name = Name
opt-col-type = Type
opt-col-value = Value
opt-col-description = Description
opt-col-rocktype = Rock type
opt-col-cost = Cost
opt-col-min-grade = Minimum grade
opt-col-max-grade = Maximum grade
opt-col-threshold = Threshold
opt-col-element = Element
opt-col-recovery = Recovery
opt-col-element-cost = Element cost
opt-col-price = Price
opt-col-selling-costs = Selling costs
opt-type-number = Number
opt-type-text = Text
opt-type-yes-no = Yes/No

opt-grid-add = Add
opt-grid-delete = Delete
opt-grid-up = Up
opt-grid-down = Down
opt-grid-add-all = Add all
opt-grid-add-group = Add group
opt-grid-group-name = Group name

opt-pick-constant = Use a constant
opt-clear-constant = Remove the constant and type a value
opt-pick-constant-title = Select a constant
opt-no-constants-of-type = There are no constants of this type. Add them in the Constants section.
opt-constant-missing = The constant "{ $name }" no longer exists
opt-constant-wrong-type = The constant "{ $name }" is not of this field's type

opt-mining-cost = Default mining cost
opt-currency-agnostic = Any currency: costs are plain numbers
opt-cost-by-field = Use different costs based on BM field
opt-rehab-cost = Rehabilitation costs
opt-waste-haulage = Waste haulage costs
opt-ore-haulage = Ore haulage costs
opt-haulage-value = Value or constant
opt-haulage-field = Block model field

opt-default-method-name = Default
opt-elements-of = Elements of { $name }
opt-select-method = Select a processing method above to edit its elements.
opt-ga-costs = G&A costs
opt-group-other-costs = Other costs
opt-haulage-factor = Haulage cost adjustment factor
opt-ga-same-for-all = Same G&A for every method

opt-select-mode = Shells
opt-mode-single = Single shell
opt-mode-multiple = Multiple shells using revenue factors
opt-directional-shells = Use directional shells
opt-direction-n = North
opt-direction-ne = North - East
opt-direction-e = East
opt-direction-se = South - East
opt-direction-s = South
opt-direction-sw = South - West
opt-direction-w = West
opt-direction-nw = North - West
opt-pick-start = Pick
opt-start-point-label = Starting point
opt-mining-direction = Mining direction
opt-pick-start-hint = Click a block of the model to set the starting point
opt-start-point = X { $x }, Y { $y }
opt-picked = Picked
opt-pick-prompt = Select a block of the model
opt-pick-needs-block-model = Choose a block model in Inputs before picking a starting point
opt-pick-done = Directional shell starting point set to X { $x }, Y { $y }
opt-factor-from = Revenue factor from
opt-factor-to = Revenue factor to
opt-factor-step = Step
opt-shell-count = Number of shells
opt-factor-range-invalid = The factor range must end above where it starts.
opt-create-reports = Create reports
opt-shell-name-prefix = Shell name prefix
opt-shell-name-prefix-hint = Shells are named <prefix>_RF 0.85, inside a collection named after the scenario. Each scenario needs its own prefix, so its shells never clash with another scenario's.
opt-shell-prefix-taken = Another scenario already uses this prefix; shells must have a prefix of their own
opt-shell-prefix-empty = Type a prefix for the shells' names
opt-run-prefix-taken = The shell name prefix “{ $prefix }” is empty or used by another scenario; give this scenario its own prefix in Outputs
opt-layer-name = Layer name
opt-default-shell-layer = Pit shell
opt-reports-downloaded = The browser downloads the report
opt-create-reports-hint = Writes optimization_results.csv into a folder named after the scenario inside the chosen folder, replacing the earlier report of that name; nothing else in the folder is touched.
opt-reports-folder-hint = Choose a folder
opt-browse = Browse...
opt-reports-browser-unavailable = Reports cannot be written to a folder from the browser
opt-shell-field-existing = Existing field
opt-shell-field-new = New field
opt-shell-new-field-hint = New field name
opt-shell-field-value-sequence = Sequence number
opt-shell-field-value-factor = Revenue factor
opt-shell-field-value-hint = Written as a text (category) field, so each shell gets its own colour, matching its shell triangulations. Each block gets the smallest shell that holds it, so the categories ring outwards from the smallest shell. Blocks outside every shell, and air, are left blank. A field that already exists is overwritten.
opt-shell-field-missing = Choose the block model field the shell value is written into
opt-shell-field-taken = Field '{ $field }' is used by the settings or holds numbers; choose a text field for the shell value
opt-run-field-written = Wrote { $count } shell(s) into field '{ $field }' of block model '{ $model }'
opt-run-field-failed = Could not write field '{ $field }' of block model '{ $model }': { $error }
opt-write-shell-field = Write shell value into BM

opt-overall-slope = Overall slope angle
opt-slope-default-angle = One angle all round
opt-slope-use-rosettes = Use rosettes
opt-slope-rosettes-hint = An angle for each bearing sector
opt-col-bearing = Bearing
opt-col-angle = Angle
opt-rosette-same-bearing = Two rows have the same bearing. Give each row its own bearing to see the rosette.
opt-rosette-no-rows = Add a row to see the rosette.
opt-rosette-diagram-hover = Plan view of the pit: each sector shows the wall angle that applies there
opt-compass-north = N
opt-compass-east = E
opt-compass-south = S
opt-compass-west = W
opt-factors-title = Revenue factors
opt-group-block-model = Block model
opt-group-mining-cost = Mining cost
opt-group-methods = Processing methods
opt-group-prices = Prices and selling costs
opt-group-rosette = Rosettes
opt-group-run-mode = Run mode
opt-group-files = Files
opt-group-write-back = Block model update

opt-import-json = Import scenarios from JSON
opt-export-json = Export scenarios to JSON
opt-import-summary = Add the scenarios in a JSON file
opt-export-summary = Write every scenario to a JSON file
opt-scenarios-saved-to-project = Saved { $count } optimisation scenario(s) to the project; save the project to keep them
opt-scenarios-no-project = no project is open
opt-scenarios-exported = Exported { $count } optimisation scenario(s) to { $path }
opt-scenarios-imported = Imported { $count } optimisation scenario(s)
opt-scenarios-import-failed = Could not import the optimisation scenarios: { $error }

## Optimization: units, interpolation, block model checks
opt-unit-percent = %
opt-unit-per-tonne = /t
opt-unit-density = t/m³
opt-unit-grams-per-tonne = g/t
opt-unit-ppm = ppm
opt-unit-ppb = ppb
opt-unit-unit = Unit
opt-unit-tonne = t
opt-unit-kilogram = kg
opt-unit-gram = g
opt-unit-ounce = oz
opt-unit-pound = lb
opt-col-grade-unit = Grade unit
opt-col-sales-unit = Sales unit
opt-col-processing-cost = Processing cost
opt-units-hint = Grade unit: how the block model field is stored. Sales unit: what price, selling cost and every method's element cost are per.
opt-element-units = Grade { $grade }, priced per { $sales }
opt-element-cost-hint = Element cost is per sales unit contained in the feed
opt-interpolation = Between rows
opt-interpolation-step = Step (sectors)
opt-interpolation-linear = Linear
opt-interpolation-cosine = Cosine
opt-interpolation-cubic = Cubic
opt-bm-irregular = This block model is sub-blocked or its blocks are not on one regular grid: it cannot be optimised yet.
opt-bm-rotated = This block model is rotated: it cannot be optimised yet.

## Optimization: run
opt-run-missing-constant = { $what }: the constant "{ $constant }" is missing or of another type
opt-run-field-not-set = { $what } is not chosen
opt-run-field-missing = { $what }: the block model has no field "{ $field }" of the right kind
opt-run-what-rock-cost = Mining cost for { $rocktype }
opt-run-what-price = Price of { $element }
opt-run-what-selling-cost = Selling cost of { $element }
opt-run-what-method = Method { $method }, { $what }
opt-run-duplicate-revenue = { $element } has more than one price row; the first is used
opt-run-selling-above-price = { $element } costs more to sell than it sells for
opt-run-no-methods = No processing method is defined
opt-run-method-grade-range = Method { $method }: the maximum grade must be above the minimum
opt-run-recovery-range = Method { $method }: the recovery of { $element } must be between 0 and 100 %
opt-run-no-revenue = { $element } has no price, so it earns nothing
opt-run-methods-overlap = Methods { $first } and { $second } take the same material; each block goes where it brings the better cash flow
opt-run-same-name-costs = Rows of method { $method } do not agree on their costs; each row is valued as it is
opt-run-topography-missing = The topography "{ $name }" is not an open triangulation; a point cloud has to be triangulated first
opt-run-air-value-not-set = The rock type value that means air is not chosen
opt-run-not-ready = The scenario is not complete
opt-run-angle-range = Slope angles must be between 0 and 90°
opt-run-rosette-empty = The rosette has no rows
opt-run-rosette-constant = A rosette angle uses a constant that is missing
opt-run-slope-invalid = The slope cannot be used: { $error }
opt-run-factor-range = The revenue factors must start above 0 and give between 2 and { $max } shells
opt-run-no-start = Directional shells need a starting point
opt-run-pattern = Slope pattern: { $offsets } precedence offsets over { $benches } benches
opt-run-label = Optimising { $name }
opt-run-cannot-start = Scenario { $name } cannot run:
opt-run-issue = • { $issue }
opt-run-needs-block-model = Scenario { $name }: the block model "{ $model }" is not in the project
opt-run-started = Running scenario { $name } on { $cells } cells
opt-run-queued = Scenario { $name } is queued until a running scenario finishes
opt-run-cancelled = Scenario { $name } was cancelled
opt-run-failed = Scenario { $name } failed: { $error }
opt-run-finished = Scenario { $name } finished: { $count } shell(s) in { $seconds } s
opt-run-shell-summary = Shell { $shell } (factor { $factor }): { $blocks } blocks, { $tonnes } t, ore { $ore } t, waste { $waste } t, cash flow at base price { $value }
opt-missing-title = This scenario names things the project no longer has:
opt-missing-block-model = Block model “{ $name }” is not in the project
opt-missing-field = Field “{ $field }” ({ $what }) is not in the block model, or is of the wrong kind
opt-missing-topography = Topography “{ $name }” is not in the project
opt-missing-air-value = Air value “{ $value }” is not a value of { $field }
opt-missing-choice = “{ $name }” is no longer there to choose; pick another
opt-load-title = Load for optimisation
opt-load-question = { $count ->
    [one] { $names } is unloaded. Load it to go on?
   *[other] { $names } are unloaded. Load them to go on?
}
opt-load-note = It stays loaded, hidden, so later runs need no loading. Cancel leaves it unloaded and does not go on.
opt-load = Load
opt-load-declined = Not loaded; the optimisation did not go on
opt-run-unloaded = { $names } was unloaded while the run waited; run it again to load it
opt-results-title = Results: { $name }
opt-results-title-empty = Results
opt-results-none = The run's results are no longer here; run the scenario again or open its report file.
opt-results-basis-note = Every shell is valued at the base price (revenue factor 1), with each block sent where it pays best at that price.
opt-results-cumulative = Cumulative pit
opt-results-cumulative-hint = Each shell's whole pit
opt-results-incremental = Incremental pushback
opt-results-incremental-hint = What each shell adds to the one before it
opt-results-destination = Destination
opt-results-all-destinations = All destinations
opt-results-waste = Waste
opt-results-ore-tonnes = Ore tonnes
opt-results-waste-tonnes = Waste tonnes
opt-results-cash-flow = Cash flow
opt-results-hover-shell = Shell { $shell } (RF { $factor })
opt-results-hover-tonnes = Ore { $ore } t, waste { $waste } t
opt-results-hover-strip = Strip ratio { $ratio }
opt-results-hover-money = Revenue { $revenue }, cost { $cost }, cash flow { $cash }
opt-results-col-shell = Shell
opt-results-col-raf = RF
opt-results-col-dir = Direction
opt-results-col-blocks = Blocks
opt-results-col-tonnes = Tonnes
opt-results-col-ore = Ore t
opt-results-col-waste = Waste t
opt-results-col-strip = Strip ratio
opt-results-col-revenue = Revenue
opt-results-col-cost = Total cost
opt-results-col-cash-flow = Cash flow
opt-results-columns = Choose the table's columns
opt-results-columns-default = Default columns
opt-results-main-grade = Main grade, { $element }
opt-results-col-contained = { $element } contained
opt-results-col-recovered = { $element } recovered
opt-results-col-element-revenue = { $element } revenue
opt-results-col-element-cost = { $element } cost
opt-results-col-mining = Mining cost
opt-results-col-waste-haulage = Waste haulage cost
opt-results-col-rehab = Rehabilitation cost
opt-results-col-ore-haulage = Ore haulage cost
opt-results-col-processing = Processing cost
opt-results-col-ga = G&A cost
opt-results-col-element-costs = Element costs
opt-results-filter = Show chosen shells only: click shells in the chart, then Apply
opt-results-filter-choosing = Stop choosing shells
opt-results-filter-applied = Showing { $count } chosen shell(s); click to show every shell
opt-results-choosing-hint = Click shells in the chart to choose them; click a chosen one again to take it out.
opt-results-chosen-count = { $count } chosen
opt-results-apply = Apply
opt-results-basis-note-chosen = Every shell is valued at the base price (revenue factor 1). Each increment is measured from the chosen shell before it: the pushback between them.
opt-results-copy-shells = Copy filtered shells into a new optimisation scenario
opt-shells-copied = Scenario “{ $name }” created with the { $count } chosen shell(s) as its revenue factors
opt-results-export = Export CSV
opt-results-open = Open CSV…
opt-results-exported = Saved the optimisation report to { $path }
opt-results-export-failed = Could not save the optimisation report: { $error }
opt-results-opened = Opened the optimisation report { $name } ({ $count } shell(s))
opt-results-open-failed = Could not read { $name } as an optimisation report: { $error }
opt-run-report-written = Wrote the optimisation report to { $path }
opt-run-report-failed = Could not write the optimisation report to { $path }: { $error }
opt-run-reports-folder-missing = Choose a reports folder in Outputs, or untick Create reports
opt-shell-surface-suffix = surface
opt-status-queued = Queued
opt-cancel-run = Cancel the run
opt-default-density = Default density
opt-default-density-help = Used for every block when no density field is chosen, and for blank, zero or negative values in the field. Must be above 0.
opt-run-default-density = The default density must be above 0
opt-run-no-density = { $count } block(s) had a blank, zero or negative density; the default density was used
opt-slope-field = Read from block model field
opt-slope-field-hint = Each block's own overall slope angle, the same in every direction. Blocks with a blank value or one outside 0-90° use the default angle.
opt-run-slope-angles = Slope angles from the block model: { $count } distinct angle(s), { $from }° to { $to }°
opt-run-default-angle = { $count } block(s) had a blank or out-of-range slope angle; the default angle was used
opt-interpolation-hint = How the wall angle changes between rows. Bearings are degrees clockwise from north.
    • Step: each angle holds from its bearing to the next row's (any number of rows)
    • Linear: a straight blend from row to row (2 rows or more)
    • Cosine: a smooth blend from row to row (2 rows or more)
    • Cubic: one smooth curve through every row (4 rows or more)
    Options needing more rows than the rosette has are not offered. The diagram always shows sectors.
opt-run-interpolation-fallback = { $interpolation } interpolation needs more rosette rows; the sectors (step) were used
opt-factor-input-range = From, to and step
opt-factor-input-list = Type factors manually
opt-factor-list = Revenue factors
opt-factor-list-hint = Numbers above 0, separated by commas or spaces, for example 0.5, 0.75, 1, 1.2. They are sorted and repeats dropped; each makes one shell.
opt-factor-list-empty = Type at least one revenue factor
opt-factor-list-invalid = "{ $piece }" is not a revenue factor above 0
opt-directional-shells-hint = Shell k uses the k-th revenue factor and lets ore count only up to k/N of the way across the final pit as the mining front advances from the starting point: a straight front towards the chosen direction, or rings out from the point for Radial. Shells are named like "RF 0.85 DIR 50%".
opt-direction-radial = Radial (out from the point)

## Scheduling (schedules under optimization scenarios)
sched-title = Schedule
sched-add = Add schedule
sched-save = Save schedule
sched-delete = Delete schedule
sched-duplicate = Duplicate schedule
sched-default-name = Schedule
sched-default-period-field = period
sched-default-stockpile = Stockpile { $number }
sched-unsaved-message = Schedule "{ $name }" has changes that are not saved. Save them before closing?
sched-show-schedules = Show schedules
sched-hide-schedules = Hide schedules
sched-drag-hint = Drag onto another scenario to move or duplicate this schedule there
sched-drop-title = Into "{ $scenario }":
sched-move-here = Move here
sched-duplicate-here = Duplicate here
sched-moved-to = Schedule "{ $name }" moved to "{ $scenario }"
sched-copied-to = Schedule "{ $name }" duplicated into "{ $scenario }"
sched-moved-issues = { $count } setting(s) do not match the new scenario and are marked red
sched-name-taken = Another schedule of this scenario has this name
sched-parent-gone = The scenario of this schedule no longer exists
sched-blocked = Run the scenario first: a schedule plans the shells of its scenario's run in this session
sched-issues-title = This schedule cannot run yet:
sched-run-issues = Schedule "{ $name }" cannot run: { $issues }
sched-run-label = Schedule { $name }
sched-run-started = Schedule "{ $name }" started: { $pits } pushbacks, each taken as the final pit and scheduled worst, fixed lead and best
sched-run-finished = Schedule "{ $name }" finished in { $seconds } s
sched-run-outside-bounds = Schedule "{ $name }": the fixed-lead NPV ({ $lead }) is not between worst { $worst } and best { $best }
sched-run-field-pit = Periods of the schedule ({ $case }) go into the block model
sched-run-cancelled = Schedule "{ $name }" was cancelled
sched-run-failed = Schedule "{ $name }" failed: { $error }
sched-run-cannot-start = Schedule "{ $name }" cannot start
sched-run-grid-changed = The block model's grid is not the one the scenario was run on: run the scenario again
sched-run-initial-grade = Stockpile "{ $stockpile }": its initial grade is in no grade range of method "{ $method }", the first it feeds
sched-npv-hover-peak = The NPV peaks earlier, at pushback { $pushback }
sched-run-schedule = Schedule "{ $name }": { $pushbacks } pushbacks in { $periods } periods, NPV { $npv } (best { $best }, worst { $worst })
sched-run-unfinished-schedule = Schedule "{ $name }": its pushbacks are not all mined within { $max } periods; the NPV is of what is mined
sched-run-peak-advice = Schedule "{ $name }": the NPV peaks at pushback { $pushback } ({ $npv }); the pushbacks after it lower it
sched-run-no-pushbacks = Schedule "{ $name }" has no pushback to mine
sched-res-no-reports-folder = Schedule "{ $name }": no reports are written, as the scenario has no reports folder (Outputs)
sched-res-export-failed = The schedule report could not be written: { $error }
sched-res-title = Schedule results: { $scenario } › { $schedule }
sched-res-title-file = Schedule results: { $name }
sched-res-title-empty = Schedule results
sched-res-none = Nothing to show: run the schedule, or open a schedule's periods CSV.
sched-res-periods = Period by period
sched-res-pits = Pit by pit
sched-res-mining = Mining
sched-res-processing = Processing
sched-res-export = Export CSV
sched-res-open = Open CSV
sched-res-opened = Schedule report { $name } opened: { $count } pushbacks
sched-res-open-failed = Could not read the schedule report { $name }: { $error }
sched-res-new-schedule = Schedule "{ $name }" added with the picked pushbacks
sched-res-pick = Pick pushbacks for a new schedule
sched-res-pick-stop = Stop picking pushbacks
sched-res-ore-to = Mine → { $method }
sched-res-ore-stocked = Mine → stockpiles
sched-res-processed-by = { $method } feed
sched-res-feed-grade = Head grade { $element }
sched-res-recovered = { $element } recovered
sched-res-npv-case = NPV, { $case }
sched-res-pit-label = PB { $pushback } · RAF { $factor }
sched-res-period-notes = { $periods } periods, NPV { $npv }; left on stockpiles at the end: { $abandoned } t. Period 0 in the table is the initial capital.
sched-res-unfinished = Not every pushback is mined within the most periods: the NPV is of what is mined.
sched-res-unfinished-mark = (unfinished)
sched-res-gap = Best and worst differ by { $percent } % of the best case at the last pushback: the sequence matters.
sched-res-gap-small = Best and worst differ by only { $percent } % at the last pushback: under 3 % the mining sequence barely changes the NPV.
sched-res-outside-bounds = The fixed-lead NPV of the schedule is not between worst and best: check the lead and the capacities.
sched-res-peak = The fixed-lead NPV peaks at pushback { $pushback }: the pushbacks after it lower the NPV.
sched-res-hover-period = Period { $period }
sched-res-hover-mined = Mined { $mined } t, of which waste { $waste } t
sched-res-hover-method = { $method }: mine → { $direct } t; feed { $processed } t (direct and reclaimed)
sched-res-hover-stock = Mine → stockpiles { $stocked } t; stockpiles → process { $reclaimed } t
sched-res-hover-balance = { $stockpile }: { $balance } t on it
sched-res-hover-dcf = Discounted { $dcf }, to date { $cumulative }
sched-res-hover-case = { $case }: { $npv } in { $periods } periods
sched-res-hover-abandoned = Left on stockpiles (fixed lead): { $tonnes } t
sched-res-held-no-stockpile = Mining held back: the methods were full and the next ore was below every stockpile's min grade
sched-res-held-stockpiles-full = Mining held back: the methods and the stockpiles taking the next ore were full
sched-res-held-short-no-stockpile = no stockpile
sched-res-held-short-full = stockpiles full
sched-res-col-period = Period
sched-res-col-held = Held back
sched-res-col-mined = Mined (t)
sched-res-col-waste = Waste (t)
sched-res-col-processed = Process feed (t)
sched-res-col-stocked = Mine → stockpiles (t)
sched-res-col-reclaimed = Stockpiles → process (t)
sched-res-col-direct = Mine → { $method } (t)
sched-res-col-method-processed = { $method } feed (t)
sched-res-col-stockpile-stocked = Mine → { $stockpile } (t)
sched-res-col-stockpile-reclaimed = { $stockpile } → process (t)
sched-res-col-balance = { $stockpile } balance (t)
sched-res-col-feed-grade = Head grade { $element }
sched-res-col-recovered = { $element } recovered
sched-res-col-mining-cost = Mining cost
sched-res-col-processing-cost = Processing cost
sched-res-col-rehandle-cost = Rehandling
sched-res-col-fixed-cost = Fixed cost
sched-res-col-capex = Capital
sched-res-col-dcf = Discounted cash flow
sched-res-col-cumulative = Cumulative discounted
sched-res-col-pushback = Pushback
sched-res-col-npv = NPV, { $case }
sched-res-col-periods = Periods, { $case }
sched-res-col-abandoned = Left on stockpiles, { $case } (t)
sched-res-case-lead = Fixed lead = { $lead }
sched-res-all-cases = Worst and best
sched-res-all-cases-hint = Show the worst and best case NPV beside fixed lead: the bounds it should fall between
sched-res-stockpile-balance = Stockpile balance
sched-res-npv = NPV
sched-res-col-balance-total = Stockpile balance (t)
sched-res-col-stockpile-stocked-grade = Mine → { $stockpile } grade
sched-res-col-stockpile-reclaimed-grade = { $stockpile } → process grade
sched-res-col-balance-grade = { $stockpile } balance grade
sched-res-col-bench = Bench (elevation)
sched-res-col-bench-ore = Bench ore (t)
sched-res-col-bench-waste = Bench waste (t)
sched-res-col-bench-grade = Bench grade { $element }
sched-res-col-bench-period = Mined in period
sched-run-held-no-stockpile = Mining held back in periods { $periods }: the methods were full and the ore next in line was below every stockpile's min grade (or no stockpile feeds its method), so it had nowhere to go
sched-run-held-stockpiles-full = Mining held back in periods { $periods }: the methods were full and the stockpiles that take the ore next in line were full too
sched-run-parent-first = Schedule "{ $name }": scenario "{ $scenario }" has no up-to-date run, so it runs first; the schedule follows
sched-run-parent-first-hint = The scenario has no up-to-date run: Run runs it first, then this schedule
sched-run-parent-failed = The scenario's run did not finish, so { $count } schedule(s) waiting for it do not run
sched-anim-button = Play the mining on the block model
sched-anim-needs-run = Run the schedule first: the mining played is its last run's
sched-anim-no-run = Schedule "{ $name }" has no mining to play: run it, and check a pit of its case is mined out
sched-anim-title = { $name }: { $case }, pit { $shell }
sched-anim-no-model = Block model "{ $model }" is not open
sched-anim-unloaded = Block model "{ $model }" is not loaded: load it to play the mining
sched-anim-failed = The mining could not be played: { $error }
sched-anim-step = Pushback { $pushback } · bench at { $elevation } · period { $period }
sched-anim-opened = Playing the mining: the block model is shown alone until the deck is closed
anim-step-of = { $step } / { $count }
anim-steps-per-second = benches/s
anim-speed-hint = Benches mined per second while playing; drag to change
anim-keep-outer = Outer blocks
anim-keep-outer-hint = Show the rock around the pit as ground until the end. Off, only the blocks the schedule mines are shown
anim-strip-hint = Click or drag to go to a bench: benches vanish to the right of the start, and come back dragging left
sched-npv-none = Fixed-lead NPV of the last run. Not run yet, or changed since.
sched-npv-hover = Fixed lead, every pushback: { $lead } in { $periods } periods
    Best case: { $best }
    Worst case: { $worst }
sched-section-pushbacks = Pushbacks
sched-section-periods = Periods & discounting
sched-section-capacities = Capacities
sched-section-stockpiles = Stockpiles
sched-section-costs = Costs & events
sched-section-outputs = Outputs
sched-group-pushbacks = Pushbacks and cases
sched-pushbacks = Pushbacks
sched-pushbacks-hint = Which shells the fixed-lead case mines as pushbacks. Every shell is also tried as the final pit.
sched-pushbacks-every-shell = Every shell
sched-pushbacks-factors = Chosen revenue factors
sched-pushback-factors = Pushback factors
sched-pushback-factors-hint = The scenario's revenue factors; tick those whose shells are pushbacks. A factor the scenario no longer runs is red.
sched-no-parent-factors = The scenario's revenue factors cannot be read: check its Outputs section
sched-lead-benches = Fixed lead
sched-lead-benches-hint = Benches (block layers) each pushback stays ahead of the next one, never more: with 3, pushback 2 takes its first bench before pushback 1 takes its fourth. 0 mines the pit bench by bench (worst case); a lead as deep as the pit mines the pushbacks one after another (best case).
sched-cases-note = Best case (shells one after another) and worst case (the pit bench by bench) always run, as the bounds the fixed-lead NPV should fall between.
sched-group-periods = Periods
sched-periods-note = A period is whatever you take it to be (a year, a quarter): use the same one for every capacity, cost and rate.
sched-max-periods = Most periods
sched-max-periods-hint = A schedule stops after this many periods, so a pit the capacities never finish does not run forever
sched-discount-rate = Discount rate per period
sched-group-mining = Mining
sched-mining-capacity = Mining capacity per period
sched-group-processing = Processing
sched-method = Method
sched-col-max = Most per period
sched-total-limit-hint = One limit on the tonnes every processing method takes together, beside each method's own
sched-total-process-limit = All methods per period
sched-group-selling = Selling
sched-element = Element
sched-col-max-sold = Most sold per period
sched-selling-note = In each element's sales unit (Revenues). An element without a row is not limited.
sched-unit-tonnes = t
sched-group-stockpiles = Stockpiles
sched-stockpiles-note = Ore a method cannot take in a period goes to a stockpile feeding that method if it is at or above the stockpile's min grade (the highest min grade wins); ore no stockpile takes holds mining back. A stockpile keeps the ore of each method it feeds apart, at its average grade, and reclaims it to that method only, highest min grade first, when the method has room. The initial balance belongs to the first method it feeds.
sched-col-name = Name
sched-col-feeds = Feeds
sched-col-capacity = Capacity
sched-col-rehandle = Rehandling
sched-col-grade = Grade
sched-group-costs = Costs
sched-capex-initial = Initial capital
sched-capex-initial-hint = Spent in period 0, before mining starts
sched-fixed-cost = Fixed cost per period
sched-fixed-cost-hint = Paid every period the mine runs, whatever it moves
sched-group-events = Events
sched-events-note = Each row is a dated change from its period on: capital spent in that period, a change of one capacity, or both. Mill +1,000,000 t from period 3 for 15,000,000: period 3, capital 15,000,000, changes Method: Mill, +1,000,000. Capital a year before the capacity arrives: two rows (period 2 capital only; period 3 the change). Rows on one capacity add up.
sched-col-period = Period
sched-col-description = Description
sched-col-capex = Capital
sched-col-target = Changes
sched-col-change = Change per period
sched-event-target-none = Nothing (cost only)
sched-event-target-mining = Mining capacity
sched-event-target-total = All methods together
sched-event-target-method = Method: { $method }
sched-event-target-selling = Selling: { $element }
sched-event-in-period = Event in period { $period }
sched-group-write-back = Block model
sched-write-period-field = Write periods into the block model
sched-write-period-field-hint = Each block gets the period it is mined in, in a text field, so every period takes its own colour
sched-period-case = Case
sched-case-fixed-lead = Fixed lead
sched-case-best = Best case
sched-case-worst = Worst case
sched-period-field-name = Period field
sched-period-field-missing = Choose or name the field the periods are written into
sched-period-field-taken = "{ $field }" is used by the scenario, or holds numbers
sched-create-reports = Create reports
sched-create-reports-hint = Written into the scenario's reports folder
sched-group-issues = Not ready to run
sched-issue-constant = { $what }: constant "{ $constant }" is not in the scenario's constants
sched-issue-negative = { $what } cannot be negative
sched-issue-unknown = { $what } "{ $name }" is not in the scenario
sched-issue-factor = Revenue factor { $factor } is not one of the scenario's
sched-issue-no-pushbacks = Choose at least one revenue factor as a pushback
sched-issue-no-periods = At least one period
sched-issue-event-period = { $what } is past the last period ({ $max })
sched-zero-limit-hint = A limit of 0 lets nothing through: tick No limit instead, or type the capacity
sched-discount-note = Each period's cash flow is discounted at the end of the period: NPV = sum of cash flow / (1 + rate) ^ period. Initial capital is not discounted.
sched-col-min-grade = Min grade
sched-feeds-none = Choose the method(s) this stockpile takes ore for and is reclaimed to
sched-feeds-unknown = A method ticked here is not in the scenario
sched-group-initial-balance = Initial balance of "{ $name }"
sched-initial-tonnes = Tonnes
sched-initial-grade = Grade of { $field }
sched-initial-balance-hint = Ore already on the stockpile when the schedule starts (an operating mine or a re-plan); 0 for an empty stockpile
sched-stockpile-min-grade-hint = Lowest grade of { $field } sent to this stockpile: ore at or above it that a method it feeds cannot take in a period goes here
sched-issue-no-limits = Every capacity has no limit: set at least one limit for mining, processing or selling
sched-issue-zero-limit = { $what }: a limit of 0 lets nothing through
sched-issue-event-unlimited = { $what } changes { $target }, which has no limit
sched-issue-event-below-zero = The events take { $target } below 0 from period { $period }
sched-issue-no-feeds = Stockpile "{ $name }" feeds no method
sched-issue-no-quality = Stockpile grades are of the scenario's main quality field, which is not chosen
chart-legend-hint = Click to hide or show; hover to bring it to the front
opt-list-edit = Edit the picked scenario or schedule
opt-list-duplicate = Duplicate the picked scenario or schedule
opt-list-delete = Delete the picked scenario or schedule
opt-list-results = Results of the picked scenario or schedule
sched-no-limit-hint = No limit: switch it on (in the box, left) to set one
opt-rename-hint = Double-click to rename
sched-run-all = Running { $count } schedules of "{ $scenario }" side by side
sched-run-all-button = Run all its schedules, side by side
sched-run-all-none = The scenario has no schedules to run
opt-results-columns-all = Show all
opt-run-inputs-changed = Scenario "{ $name }": its block model inputs changed since its run, so the run (and its schedules) are out of date
## Error messages raised by the code, grouped by the file that raises them

# src/app/commands/file.rs
err-file-export-project-closed = The project selected for export is no longer open
err-file-triangulation-not-loaded = The selected triangulation is no longer loaded
err-file-project-closed = The selected project is no longer open
err-file-drillhole-not-loaded = The selected drillhole dataset is no longer loaded
err-file-block-model-not-loaded = The selected block model is no longer loaded
err-file-renderer-not-ready = The renderer is not initialised yet
err-file-no-active-project-import = No project is open to import into
err-file-no-active-project-save = No project is open to save
err-file-browser-loop-unavailable = The browser event loop is unavailable
err-file-no-omf-selected = No OMF file was selected
err-file-mesh-extension = Choose a filename ending in .obj, .stl or .ply
err-file-not-text = The file is not text
err-file-choose-sources = Choose source files before importing
err-file-choose-csv = Choose a .csv file before importing
err-file-revert-never-saved = This project has never been saved, so there is nothing to revert to
err-file-layer-closed = The selected layer is no longer open
err-file-layers-never-saved = This project has never been saved, so its layers cannot be restored
err-file-browser-project-closed = The selected browser project is no longer open
err-file-desc-dxf = DXF file
err-file-desc-triangulation = triangulation file
err-file-desc-drillhole-csv = drillhole CSV
err-file-desc-block-model-csv = block model CSV
err-file-desc-haul-roads = haul roads file
err-file-desc-schedule-report = schedule report
err-file-create-folder = Could not create { $path }
err-file-read-failed = Could not read { $path }
err-file-save-project-failed = Could not save the project to { $path }
err-file-show-in-manager = Could not show { $path } in the file manager
err-file-wait-switch = Wait for the current project switch to finish
err-file-wait-revert-save = Wait for the project revert to finish before saving
err-file-save-before-show = Save the project before showing where it is
err-file-choose-sources-again = Choose the source files again before importing
err-file-choose-triangulations = Choose triangulation files before importing
err-file-choose-point-clouds = Choose point cloud files before importing
err-file-one-csv = Choose one CSV block model file at a time
err-file-text-edit-save-project = Apply or discard the current text edit before saving the project
err-file-wait-save = Wait for the current project save to finish
err-file-wait-save-discard = Wait for the project save to finish before discarding changes
err-file-wait-operation-discard = Wait for the project operation to finish before discarding changes
err-file-text-edit-save-this = Apply or discard the current text edit before saving this project
err-file-import-in-progress = A triangulation import from { $path } is still in progress
err-file-save-in-progress = A save to { $path } is already in progress
err-file-path-in-use = Another open project already uses { $path }
err-file-operation-in-progress = A file operation involving { $path } is already in progress
err-file-choose-csv-block-model = Choose a CSV block model file
err-file-encode-triangulation = Could not encode the triangulation: { $error }
err-file-write-failed = Could not write { $path }: { $error }
err-file-recovery-failure = Project '{ $stem }' to { $path }: { $error }
err-file-label-exporting = Exporting { $name }
err-file-label-saving = Saving { $name }
err-file-opening-project = Opening project…
err-file-opening-browser-project = Opening browser project…
err-file-layer-description = layer { $layer }

# src/app/mod.rs
err-file-section-preview-title = { $app } | Section preview (wheel to zoom, middle-drag to pan, F to fit)
err-file-browser-window-failed = Could not create the browser window: { $error }

# src/app/jobs.rs
err-file-unknown-panic = unknown error
err-file-task-panicked = A background task failed unexpectedly: { $message }

# src/app/web_download.rs
err-file-download-name-empty = The download file name is empty
err-file-download-start-failed = Could not start the browser download: { $error }

# src/app/web_storage.rs
err-file-storage-error = Browser storage error: { $error }
err-file-storage-non-text = Browser storage returned project data that is not text
err-file-storage-invalid-record = The browser project record is not valid: { $error }
err-file-storage-no-details = Browser storage returned a project without its details

# src/model/atomic_file.rs
err-file-temp-create = Could not create a temporary file beside { $path }
err-file-permissions = Could not keep the permissions of { $path }
err-file-sync = Could not flush { $path } to disk
err-file-replace = Could not replace { $path }

# src/main.rs
err-file-worker-pool-failed = Could not start the browser worker pool: { $error }
err-file-event-loop-failed = Could not create the browser event loop: { $error }
err-file-app-state-failed = Could not create the application state: { $error }


# src/model/formats/point_cloud.rs
err-cloud-unsupported-extension = Unsupported point cloud file type: { $name }
err-cloud-open-failed = Could not open the point cloud file { $name }
err-cloud-map-failed = Could not read the point cloud file { $name } into memory
err-cloud-las-range-overflow = LAS data range is too large
err-cloud-las-header-truncated = The LAS header is cut short
err-cloud-las-no-signature = Not a LAS or LAZ file (the LASF signature is missing)
err-cloud-las-header-small = The LAS header size ({ $header_size } bytes) is smaller than the minimum of { $minimum } bytes
err-cloud-las14-header-small = The LAS 1.4 header is too small to hold the extended point count
err-cloud-las-record-small = The LAS point record length ({ $length } bytes) is too small
err-cloud-laz-vlr-range = The LAZ variable length record range is too large
err-cloud-laz-vlr-overlap = A LAZ variable length record runs into the point data
err-cloud-laz-vlr-truncated = A LAZ variable length record is cut short
err-cloud-laz-vlr-data-range = The LAZ variable length record data range is too large
err-cloud-laz-vlr-data-overlap = LAZ variable length record data runs into the point data
err-cloud-laz-no-laszip = The LAZ file has no laszip record, so it cannot be decompressed
err-cloud-las-offset-before-header = The LAS point data starts before the end of the header
err-cloud-las-offset-past-end = The LAS point data starts past the end of the file
err-cloud-las-count-overflow = The LAS point count is too large
err-cloud-las-size-overflow = The LAS point data is too large for the available memory
err-cloud-laszip-invalid = The laszip record is not valid
err-cloud-laszip-item-overflow = The laszip item size is too large
err-cloud-laszip-size-mismatch = The laszip point size ({ $item_size } bytes) does not match the LAS record length ({ $record_length } bytes)
err-cloud-las-data-truncated = The LAS point data is cut short
err-cloud-laz-offset-too-large = The LAZ point data offset is too large
err-cloud-laz-init-failed = Could not start the LAZ decompressor
err-cloud-laz-batch-size = The LAZ decode batch is too large for the available memory
err-cloud-laz-decompress-failed = Could not decompress the LAZ point records
err-cloud-alloc-overflow = The memory needed for { $label } is too large
err-cloud-alloc-failed = Could not allocate { $bytes } bytes for { $label }
err-cloud-label-las-points = LAS points
err-cloud-label-las-colours = LAS colours
err-cloud-label-las-classes = LAS classifications
err-cloud-label-laz-batch = the LAZ decode batch
err-cloud-label-las-packed = packed LAS colours
err-cloud-ascii-utf8 = The text point cloud file is not valid UTF-8
err-cloud-ascii-no-points = No points were found in the text point cloud file
err-cloud-ascii-bad-row = Line { $line } of the text point cloud needs at least x, y and z values
err-cloud-ascii-bad-number = Line { $line } of the text point cloud has an invalid number: { $token }
err-cloud-ascii-non-finite = Line { $line } of the text point cloud has a coordinate that is not a finite number
err-cloud-pcd-header-truncated = The PCD header is cut short
err-cloud-pcd-header-utf8 = The PCD header is not valid UTF-8
err-cloud-pcd-bad-size = The PCD SIZE line is not valid
err-cloud-pcd-size-count = The PCD SIZE line does not have one entry per field
err-cloud-pcd-size-zero = Every PCD field SIZE must be greater than zero
err-cloud-pcd-type-count = The PCD TYPE line does not have one entry per field
err-cloud-pcd-bad-type-line = The PCD TYPE line is not valid
err-cloud-pcd-bad-type = The PCD field type { $token } is not valid
err-cloud-pcd-bad-count = The PCD COUNT line is not valid
err-cloud-pcd-count-count = The PCD COUNT line does not have one entry per field
err-cloud-pcd-count-zero = Every PCD field COUNT must be greater than zero
err-cloud-pcd-bad-points = The PCD POINTS line is not valid
err-cloud-pcd-bad-data = The PCD DATA line is not valid
err-cloud-pcd-no-data = The PCD file has no DATA line
err-cloud-pcd-no-points = The PCD file has no POINTS line
err-cloud-pcd-no-fields = The PCD file has no FIELDS line
err-cloud-pcd-size-count-zero = Every PCD field SIZE and COUNT must be greater than zero
err-cloud-pcd-colour-field = The PCD { $name } field must be a single 4-byte F, I or U value (found { $kind }{ $size } with COUNT { $count })
err-cloud-pcd-encoding = The PCD data encoding { $encoding } is not supported (only ascii and binary are)
err-cloud-pcd-no-field = The PCD file has no { $name } field
err-cloud-pcd-ascii-utf8 = The PCD text data is not valid UTF-8
err-cloud-pcd-row-width = The PCD row width is too large
err-cloud-pcd-bad-coordinate = A PCD coordinate is not a valid number
err-cloud-pcd-bad-rgb = A PCD colour value is not valid
err-cloud-pcd-data-truncated = The PCD data is cut short
err-cloud-pcd-record-size = The PCD binary record size is not valid
err-cloud-pcd-binary-size = The PCD binary data is too large for the available memory
err-cloud-pcd-binary-truncated = The PCD binary data is cut short
err-cloud-pcd-coordinate-type = The PCD coordinate type { $kind }{ $size } is not supported

# src/model/formats/dxf.rs
err-dxf-read-failed = Could not read { $name }
err-dxf-parse-failed = Could not parse { $name } as a DXF file
err-dxf-validate-failed = The imported DXF file { $name } is not valid
err-dxf-write-failed = Could not write { $name }

# src/model/input.rs
err-input-too-large = { $name } is too large (browser imports are limited to 2 GiB)
err-input-label-file = the file { $name }
err-input-label-head = the start of the file { $name }
err-input-allocate = Could not allocate { $bytes } bytes for { $name }: { $error }

# src/model/ground_filter.rs
err-cloud-cloth-resolution = The cloth resolution must be greater than zero
err-cloud-ground-threshold = The ground threshold must be greater than zero
err-cloud-noise-radius = The noise radius must be greater than zero
err-cloud-classifier-features = The classifier model expects { $expected } features but { $computed } are computed
err-cloud-noise-radius-small = The noise radius of { $radius } m is too small for a point cloud this large, so raise it
err-cloud-too-many-points = The point cloud has too many points to classify in one pass
err-cloud-no-ground-points = The point cloud has no points left to find the ground in

# src/app/commands/point_cloud.rs
err-cloud-no-points = The point cloud { $name } contains no points
err-cloud-no-finite-points = The point cloud { $name } contains no points with finite coordinates
err-cloud-unsupported-file = Unsupported point cloud file: { $name }
err-cloud-file-missing = The point cloud file does not exist: { $name }
err-cloud-read-failed = Could not read the point cloud { $name }
err-cloud-join-select-two = Select at least two point clouds to join
err-cloud-selected-unloaded = A selected point cloud is no longer loaded
err-cloud-classify-no-project = Open a project before classifying point clouds
err-cloud-classify-select-one = Select at least one point cloud to classify
err-cloud-classify-failed = Could not classify { $name }
err-cloud-join-no-points = The selected point clouds contain no points
err-cloud-join-no-finite-points = The selected point clouds contain no points with finite coordinates
err-cloud-label-joined = the joined point cloud
err-cloud-label-joined-colours = the joined point cloud colours
err-cloud-label-joined-classes = the joined point cloud classifications

# src/model/formats/omf.rs
err-omf-nothing-to-export = There is no open Incline Design data to export
err-omf-create-writer = Could not create the OMF writer
err-omf-encode-optimisation = Could not encode the optimisation scenarios
err-omf-finish-project = Could not finish writing the project
err-omf-schedule-format-unsupported = The schedule format { $found } is not read by this version, which reads { $supported }. Scheduling is in development and keeps no backwards compatibility
err-omf-schedule-format-unstated = unstated
err-omf-polyline-too-few-vertices = Polyline { $id } on layer '{ $layer }' has fewer than two vertices
err-omf-class-name = Class { $code }
err-omf-continuous-colour-map-empty = The continuous colour map has no colours
err-omf-discrete-colour-map-no-boundaries = The discrete colour map has no boundaries
err-omf-discrete-colour-map-mismatch = The discrete colour map has { $colours } colours for { $boundaries } boundaries
err-omf-block-model-invalid-bounds = Block model '{ $name }' has invalid bounds
err-omf-block-model-no-blocks = Block model '{ $name }' has no blocks to display
err-omf-block-model-missing-block = Block model '{ $name }' is missing block { $index }
err-omf-raster-singular-transform = Raster '{ $name }' has a world transform that cannot be inverted
err-omf-raster-too-large = The raster dimensions are too large for memory
err-omf-raster-buffer-size = The raster pixel buffer has { $actual } bytes, but { $expected } were expected
err-omf-asset-missing-from-backing = The unloaded asset is missing from its backing archive
err-omf-read-container = Could not read the OMF container { $source }
err-omf-open-archive = Could not open the OMF archive
err-omf-read-project-index = Could not read the project index
err-omf-version-1 = This is an OMF 1 file{ $detail }. Incline Design reads OMF 2 only, so re-export it as OMF 2 from the application that wrote it
err-omf-validate-line-designs = The OMF line set designs are not valid
err-omf-missing-asset-backing = The asset has no backing data
err-omf-block-count-overflow = The block count is too large
err-omf-element-unsupported-metadata = Element '{ $name }' has unsupported metadata keys: { $keys }
err-omf-element-description-dropped = Element '{ $name }' has a description that Incline Design does not keep
err-omf-element-unsupported-attributes = Element '{ $name }' has unsupported attributes that will be left out: { $attributes }
err-omf-attribute-metadata-dropped = Attribute '{ $attribute }' on element '{ $name }' has descriptive metadata that Incline Design does not keep
err-omf-designs-not-composite = The Incline Design designs element '{ $name }' is not an OMF composite
err-omf-element-part-unreadable = Element '{ $name }' has { $what } that could not be read and was left out: { $reason }
err-omf-part-reserves = a reserves field list
err-omf-part-solids = planning solids
err-omf-part-haulage = a haul road network
err-omf-read-layer-objects = Could not read the objects of design layer '{ $name }'
err-omf-read-layer = Could not read design layer '{ $name }'
err-omf-validate-designs = The designs '{ $name }' are not valid
err-omf-design-lines-no-objects = The design lines have no object column
err-omf-design-points-no-objects = The design points have no object column
err-omf-design-lines-length-mismatch = The design lines have columns of different lengths
err-omf-design-points-length-mismatch = The design points have columns of different lengths
err-omf-design-object-no-point = Design object { $id } has no point
err-omf-design-object-no-segments = Design object { $id } has no segments
err-omf-design-polyline-not-continuous = Design polyline { $id } has segments that do not form one continuous polyline
err-omf-design-layer-orphan-geometry = The design layer has geometry that no object record claims
err-omf-designs-nested-unsupported = Skipped unsupported '{ $name }' geometry nested inside the Designs composite
err-omf-grid-vertex-overflow = The grid surface has too many vertices
err-omf-grid-heights-mismatch = Grid surface '{ $name }' has { $heights } heights for { $vertices } vertices
err-omf-grid-face-overflow = The grid surface has too many faces
err-omf-build-surface = Could not build the surface '{ $name }'
err-omf-point-set-empty = Skipped the empty point set '{ $name }'
err-omf-classification-undecodable = Element '{ $name }' has a classification attribute that could not be decoded and will be left out
err-omf-point-set-no-finite = The point set '{ $name }' contains no finite points
err-omf-block-count-overflows = The block model has too many blocks
err-omf-block-model-empty = Skipped the empty block model '{ $name }'
err-omf-drillhole-missing-parts = Drillhole dataset '{ $name }' is missing its collars, traces or intervals
err-omf-drillhole-read-part = Could not read the { $part } of drillhole dataset '{ $name }'
err-omf-part-collars = collars
err-omf-part-traces = traces
err-omf-part-intervals = intervals
err-omf-drillhole-collars-not-points = The drillhole collars of '{ $name }' are not a point set
err-omf-drillhole-traces-not-lines = The drillhole traces of '{ $name }' are not a line set
err-omf-hole-name = Hole { $number }
err-omf-drillhole-no-depths = The drillhole traces have no measured depths
err-omf-drillhole-traces-length-mismatch = The drillhole traces of '{ $name }' have columns of different lengths
err-omf-drillhole-no-from = The drillhole intervals have no From depths
err-omf-drillhole-no-to = The drillhole intervals have no To depths
err-omf-drillhole-intervals-length-mismatch = The drillhole intervals of '{ $name }' have columns of different lengths
err-omf-drillhole-field-length = Drillhole interval field '{ $field }' of '{ $name }' has the wrong length
err-omf-drillhole-category-missing = The '{ $attribute }' category is missing
err-omf-texture-not-surface = Skipped mapped texture '{ $texture }' on '{ $name }': only surface textures can become Incline Design rasters
err-omf-decode-texture = Could not decode texture '{ $name }'
err-omf-texture-not-affine = Skipped texture '{ $texture }' on '{ $name }': its UV mapping is not affine in world XY
err-omf-raster-size-mismatch = Raster '{ $name }' declared a source size of { $declared_width } × { $declared_height }, but its embedded full-resolution image is { $width } × { $height }. The image dimensions were used
err-omf-items-over-limit = The OMF { $description } count { $count } exceeds the import limit of { $limit } in Incline Design
err-omf-items-over-memory = The OMF { $description } count is too large for memory
err-omf-sub-block-parent-outside = The sub-block parent { $parent } is outside the grid { $count }
err-omf-parent-index-overflow = The block parent index is too large
err-omf-sub-block-parent-range = The sub-block parent is out of range
err-omf-parent-attribute-missing = The parent block attribute is missing value { $parent }
err-omf-items-grid-vertices = grid surface vertices
err-omf-items-grid-faces = grid surface faces
err-omf-items-colours = colours
err-omf-items-classifications = classifications
err-omf-items-blocks = blocks
err-omf-items-free-subblocks = free-form sub-blocks
err-omf-items-regular-subblocks = regular sub-blocks
err-omf-items-texcoords = texture coordinates
err-omf-items-vertices = vertices
err-omf-items-number-values = number values

# src/model/project.rs
err-store-project-format-version = Unsupported legacy design format version { $found } (expected { $expected })
err-store-project-document-invalid = The project document is not valid
err-store-project-folders-invalid = The project folders are not valid

# src/model/folders.rs
err-store-folder-blank-name = A { $section } folder has a blank name
err-store-folder-duplicate-name = There is more than one { $section } folder named '{ $name }'
err-store-folder-duplicate-id = More than one folder has the id { $id } ('{ $name }')

# src/model/asset_storage.rs
err-store-backing-create = Could not create the asset backing file
err-store-backing-flush = Could not finish writing the asset backing file
err-store-read-path = Could not read { $path }
err-store-backing-write = Could not write the asset backing file
err-store-browser-store = Could not store the unloaded asset: { $error }
err-store-asset-open = Could not open the unloaded asset
err-store-asset-read = Could not read the unloaded asset
err-store-browser-read = Could not read the unloaded asset: { $error }
err-store-cache-locate = Could not find the asset cache folder
err-store-cache-create = Could not create the asset cache folder
err-store-cache-lock-open = Could not open the asset cache lock
err-store-session-create = Could not create the asset session folder
err-store-session-lock = Could not create the asset session lock

# src/model/asset_residency.rs
err-store-backing-no-triangulation = The backing data contains no triangulation
err-store-backing-no-block-model = The backing data contains no block model
err-store-backing-no-drillholes = The backing data contains no drillholes
err-store-backing-no-point-cloud = The backing data contains no point cloud
err-store-backing-no-raster = The backing data contains no raster

# src/app/commands/layer.rs
err-store-layer-position-missing = The layer position is no longer available

# src/app/commands/triangulation/cuts.rs
err-tri-triangulation-not-found = Triangulation not found
err-tri-not-closed-polyline-or-circle = The selected object is not a closed polyline or circle
err-tri-polyline-fewer-than-three-points = The selected polyline has fewer than 3 boundary points
err-tri-no-surface-inside-polyline = No surface geometry falls inside the selected polyline
err-tri-no-surface-outside-polyline = No surface geometry falls outside the selected polyline
err-tri-topography-and-pit-shell-same = The topography and the pit shell must be different triangulations
err-tri-topography-not-found = Topography not found
err-tri-pit-shell-not-found = Pit shell not found
err-tri-no-topography-outside-pit-shell = No topography geometry falls outside the pit shell
err-tri-z-min-below-z-max = The minimum elevation must be less than the maximum elevation
err-tri-no-geometry-in-elevation-range = No triangulation geometry lies within the specified elevation range
err-tri-trim-surface-and-topography-same = The surface to trim and the topography must be different triangulations
err-tri-trim-surface-not-found = Surface to trim not found
err-tri-reference-topography-not-found = Reference topography not found
err-tri-no-surface-relative-topography = No surface geometry lies { $retained } the topography within its XY coverage
err-tri-topography-overlaps-itself = The reference topography overlaps itself in plan and is not single-valued (triangles { $first } and { $second } overlap by { $area } square units with up to { $z_delta } elevation difference)
err-tri-name-repaired-surface = Repaired surface
err-tri-repaired-surface-no-footprint = The repaired surface has no plan footprint left to rebuild
err-tri-pit-shell-no-faces = The pit shell contains no faces
err-tri-pit-shell-no-usable-faces = The pit shell contains no usable (non-degenerate) faces
err-tri-cdt-bulk-load-failed = Could not build the { $name } triangulation: { $error }
err-tri-cdt-dropped-vertices = The { $name } triangulation dropped vertices unexpectedly
err-tri-pit-shell-non-finite = The pit shell contains non-finite coordinates
err-tri-name-pit-shell = Pit shell
err-tri-name-pit-shell-envelope = Pit shell envelope
err-tri-pit-shell-no-footprint = The pit shell has no plan footprint to build a lower envelope from
err-tri-topography-no-faces = The reference topography contains no faces
err-tri-topography-needs-non-vertical-face = The reference topography must contain at least one non-vertical face

# src/app/commands/triangulation/creation.rs
err-tri-open-boundary-degenerate = Open polylines form a closed boundary, but that boundary is degenerate or collinear
err-tri-no-objects-selected = No objects selected for triangulation
err-tri-open-project-first = Open a project before creating a triangulation
err-tri-no-usable-polylines = No usable polylines selected. Triangulation needs a closed boundary, or open polylines whose endpoints form one
err-tri-no-faces-produced = The triangulation produced no faces. The polylines may be collinear or degenerate
err-tri-no-closed-boundary = The triangulation has no closed boundary
err-tri-boundary-cycle-ended = The open polyline boundary ended unexpectedly
err-tri-open-strings-ambiguous-boundary = The selected open polylines do not define a single closed boundary ({ $unmatched } unmatched endpoints, { $junctions } branching junctions)
err-tri-breakline-edge-vs-edge = breakline { $path_index } edge { $edge_index } ({ $a } to { $b }) and breakline { $other_path_index } edge { $other_edge_index } ({ $c } to { $d }): { $detail }
err-tri-breakline-edge-no-conflict-found = breakline { $path_index } edge { $edge_index } ({ $a } to { $b }): no conflicting edge found by a direct geometric check (likely a near-degenerate numerical case)
err-tri-relation-cross = cross
err-tri-relation-touch = touch
err-tri-edges-collinear-overlap = collinear and overlapping along the same line for parameter range [{ $t0 }, { $t1 }] of edge A (these two breaklines run along the same wall without sharing vertices, so they cannot both be inserted without splitting them)
err-tri-crossing-different-elevations = different elevations: not representable by a single-valued terrain surface
err-tri-crossing-same-elevation = same elevation: could be split at this point
err-tri-edges-crossing-detail = { $relation } in plan at ({ $x }, { $y }); edge A's elevation there is { $z_a }, edge B's elevation there is { $z_c } ({ $representable })
err-tri-endpoints-start-start = start of edge A and start of edge B
err-tri-endpoints-start-end = start of edge A and end of edge B
err-tri-endpoints-end-start = end of edge A and start of edge B
err-tri-endpoints-end-end = end of edge A and end of edge B
err-tri-nearest-endpoints-gap = nearest endpoints ({ $label }) are { $gap } apart in plan (elevation differs by { $dz }), likely meant to be the same shared vertex but digitised independently
err-tri-collinear-overlap-conflicting-elevations = collinear overlap has conflicting elevations near ({ $x }, { $y }); edge A's elevation is { $z_a }, edge B's elevation is { $z_c }
err-tri-breakline-edges-conflicting-elevations = Selected breakline edges intersect in plan at conflicting elevations and cannot form a single-valued terrain surface (breakline { $path_index } edge { $edge_index } ({ $a } to { $b }) and breakline { $other_path_index } edge { $other_edge_index } ({ $c } to { $d }): { $detail })
err-tri-no-closed-breakline-boundary = No closed breakline boundary supplied
err-tri-breakline-too-few-vertices = A selected breakline has too few vertices
err-tri-breakline-non-finite = A selected breakline contains non-finite coordinates
err-tri-cdt-insert-failed = Could not insert a point into the triangulation: { $error }
err-tri-breaklines-same-point-conflicting-elevations = Selected breaklines contain the same plan point at conflicting elevations ({ $first } and { $second })
err-tri-breakline-edges-intersect = Selected breakline edges intersect or overlap in plan and cannot form a terrain surface ({ $detail })
err-tri-cdt-vertex-without-elevation = The triangulation introduced a vertex without an elevation while resolving breaklines
err-tri-constrained-no-faces = The constrained surface triangulation produced no faces
err-tri-solid-boundary-constant-elevation = A closed solid needs each outer boundary to have one constant elevation
err-tri-solid-above-and-below = A solid design cannot extend both above and below its outer boundary elevation
err-tri-solid-needs-more-elevations = Each closed solid group needs breaklines at more than one elevation
err-tri-outer-vertex-missing = An outer breakline vertex is missing from the constrained surface
err-tri-solid-closure-degenerate = Could not triangulate the solid closure: the boundary ring is degenerate
err-tri-polyline-fewer-than-three-vertices = A selected polyline has fewer than 3 vertices
err-tri-polyline-non-finite = A selected polyline contains non-finite coordinates
err-tri-polyline-same-point-conflicting-elevations = The selected polyline contains the same plan point at conflicting elevations ({ $first } and { $second })
err-tri-polyline-edges-too-close = The selected polyline edges cross or overlap themselves too closely in plan to triangulate
err-tri-polyline-edges-cross-conflicting-elevations = The selected polyline edges cross in plan at conflicting elevations ({ $first } and { $second })
err-tri-polyline-triangulation-failed = Could not triangulate the polyline (it may be degenerate or collinear)
err-tri-polyline-edges-conflicting-elevations = The selected polyline edges cross in plan at conflicting elevations (edge { $edge_index } and edge { $other_edge_index }: { $detail })

## app/commands/triangulation/include.rs
err-surf-include-same-surface = The topography and the pit or stockpile solid must be different triangulations
err-surf-include-topography-missing = Topography triangulation not found
err-surf-include-solid-missing = Pit or stockpile solid triangulation not found
err-surf-include-combined-no-faces = The combined triangulation has no faces
err-surf-include-topography-no-faces = The topography has no faces
err-surf-include-solid-no-faces = The pit or stockpile solid has no faces
err-surf-include-clip-failed = Clipping the pit or stockpile failed after extending its rim vertically: { $error }
err-surf-include-no-intersection-boundary = The pit or stockpile solid does not form a boundary where it meets the topography
err-surf-include-cut-ring-non-finite = The cut boundary has coordinates that are not finite numbers
err-surf-include-cut-rings-empty = The cut boundary is empty
err-surf-include-cut-ring = Cut boundary
err-surf-include-no-cap-faces = The pit or stockpile solid has no cap faces
err-surf-include-no-cap-no-overlap = The pit or stockpile solid has no flat closing cap and does not overlap the topography
err-surf-include-no-closed-boundary = Could not form a closed boundary on the topography from the clipped pit or stockpile
err-surf-include-no-retained-side = The pit or stockpile solid has no geometry on the kept side of the topography
err-surf-include-no-intersection = The pit or stockpile solid does not intersect the topography surface
err-surf-include-boundary-non-finite = The topography boundary has coordinates that are not finite numbers
err-surf-include-no-constraint-geometry = The shape coverage has no constraint geometry
err-surf-include-boundary = Boundary
err-surf-include-cut-ring-coverage = Cut boundary coverage
err-surf-include-shape-coverage = Shape coverage
## app/commands/triangulation/point_cloud_tin.rs
err-surf-tin-cloud-unloaded = The selected point cloud is no longer loaded
err-surf-tin-progress = Triangulating point cloud...
err-surf-tin-classification-mismatch = The point cloud's classifications do not match its points
err-surf-tin-too-few-ground = Fewer than 3 points in the point cloud are classified as ground. Turn off 'Ground points only' to use every point
err-surf-tin-too-few-points = The point cloud has too few points to triangulate a topography surface
err-surf-tin-cancelled = Topography triangulation cancelled
err-surf-tin-bulk-load-failed = Triangulating the topography failed: { $error }
err-surf-tin-too-few-unique = The point cloud has fewer than 3 unique XY positions
err-surf-tin-no-faces = No faces were created. Increase Max edge or use more points
err-surf-tin-too-few-finite = The point cloud has fewer than 3 points with finite coordinates
err-surf-tin-count-overflow = The point cloud has too many points in one cell to triangulate
## app/commands/triangulation/contours.rs
err-surf-contour-intervals-invalid = Intervals must be positive numbers
err-surf-contour-minor-too-small = The minor contour interval is too small (minimum 0.000001); it would create an unbounded number of contour levels
err-surf-contour-major-below-minor = The major interval must be at least the minor interval
err-surf-contour-triangulation-missing = Triangulation not found
err-surf-contour-range-invalid = The contour elevation range must have a minimum below its maximum
err-surf-contour-range-no-overlap = The contour elevation range does not overlap the triangulation
err-surf-contour-no-elevation-extent = The triangulation has no elevation range to contour
err-surf-contour-too-many-levels = These settings would create about { $levels } contour levels over an elevation range of { $extent } m. Increase the minor interval, or restrict the elevation range to contour part of the triangulation
err-surf-contour-no-project = No active project to store the contours in
err-surf-contour-name-required = Enter a name for the new contour layer
err-surf-contour-layer-exists = A layer named '{ $name }' already exists. Select it as the output layer or choose a different name
err-surf-contour-layer-missing = The selected contour output layer no longer exists
err-surf-contour-progress = Generating contours...
err-surf-contour-none = No contours were created for the selected intervals
## app/commands/triangulation/solid_between.rs
err-surf-solid-same-surface = A solid needs two different surfaces
err-surf-solid-design-missing = Design surface not found
err-surf-solid-topography-missing = Topography surface not found
err-surf-solid-bench-bands = Bench bands must be finite numbers, in ascending order and not overlapping
err-surf-solid-no-faces = The solid between the two surfaces has no faces
## app/commands/triangulation/session.rs
err-surf-read-failed = Failed to read { $name }: { $error }
err-surf-no-faces = The triangulation has no faces
## app/commands/mod.rs
err-surf-project-required = Create or open a project before importing, drawing or generating data
err-surf-ore-name = { $stem } Ore
## model/raster.rs
err-raster-zero-size = The raster has no pixels (its width or height is zero)
err-raster-too-large = The raster is too large to fit in memory
err-raster-buffer-size = The raster pixel data is { $actual } bytes but { $expected } bytes were expected
err-raster-preview-too-large = The raster preview is too large to fit in memory
err-raster-singular-geotransform = The raster's georeferencing cannot be inverted (its pixel size is zero)
err-raster-missing-geotags = The GeoTIFF has no ModelTransformation or PixelScale and Tiepoint tags
err-raster-unsupported-integer = The raster uses an unsupported integer sample format
err-raster-truncated-band = The raster has a truncated band chunk
err-raster-unsupported-float = The raster uses an unsupported floating-point sample format
err-raster-open-failed = Failed to open raster { $path }
err-raster-read-failed = Failed to read raster { $path }
err-raster-no-pixels = Raster { $path } has no pixels
err-raster-no-geotransform = Raster { $path } has no usable georeferencing
err-raster-invalid-geotransform = Raster { $path } has invalid georeferencing
err-raster-unsupported-format = Raster { $path } has an unsupported pixel format ({ $format })
err-raster-invalid-chunks = Raster { $path } has an invalid chunk layout
err-raster-decode-failed = Failed to decode raster { $path }
err-raster-truncated-chunk = Raster { $path } has a truncated chunk
## app/commands/raster.rs
err-raster-unsupported-file = Unsupported raster file: { $path }
err-raster-file-missing = Raster file does not exist: { $path }
err-raster-select-triangulation = Select a triangulation first
err-raster-triangulation-unloaded = The active triangulation is no longer loaded
err-raster-load-texture-first = Load the texture before draping it

# src/app/schedule_solve.rs
err-sched-cancelled-dispatch = Cancelled during the hourly dispatch
err-sched-replay-rejected = The replay rejected the schedule: { $issue }
err-sched-exceeds-bound = The published objective { $published } exceeds the SCIP bound { $bound } by more than { $tolerance }
err-sched-time-limit = The SCIP time limit must be positive and finite
err-sched-gap-range = The SCIP relative gap must lie between 0 and 1
err-sched-grid-empty = The schedule's event grid is empty
err-sched-invalid-interval = Invalid interval at position { $position }
err-sched-invalid-drill-blast = Invalid drill and blast input
err-sched-invalid-pile = Invalid or duplicate stockpile { $id }
err-sched-invalid-pile-opening-data = Invalid opening data on stockpile { $id }
err-sched-pile-opening-no-parcels = Parcel opening without parcels on stockpile { $id }
err-sched-pile-receiving-parcel = The receiving parcel is not one of the parcels on stockpile { $id }
err-sched-invalid-pile-parcels = Invalid parcels on stockpile { $id }
err-sched-invalid-parcel-opening = Invalid opening on stockpile { $id }, parcel { $position }
err-sched-invalid-pile-opening = Invalid opening on stockpile { $id }
err-sched-invalid-ground = Invalid or duplicate ground source { $id }
err-sched-invalid-material-shares = Invalid material shares on ground source { $id }
err-sched-duplicate-loader = Duplicate loader
err-sched-invalid-loader-rates = Invalid rates for loader { $id }
err-sched-duplicate-destination = Duplicate destination
err-sched-invalid-destination = Invalid destination { $id }
err-sched-invalid-grade-target = Invalid grade target { $index }
err-sched-target-period-boundary = An interval crosses the period boundary of grade target { $index }
err-sched-target-marginal-cost = The marginal costs of the grade targets cannot be represented
err-sched-invalid-carried-receipts = Invalid carried grade target receipts
err-sched-duplicate-truck = Duplicate truck class
err-sched-invalid-truck = Invalid truck class { $id }
err-sched-invalid-task = Invalid task { $id }
err-sched-invalid-dig-sequence = Invalid dig sequence on task { $id }
err-sched-invalid-reclaim-sources = Invalid reclaim sources or cap on task { $id }
err-sched-invalid-movement = Invalid movement candidate { $index }
err-sched-invalid-grade-limit = Invalid grade limit
err-sched-invalid-qualification = Invalid reclaim grade qualification
err-sched-invalid-conditional-value = Invalid conditional reclaim cash flow
err-sched-replay-failed = The schedule did not replay on the captured input: { $issue }

# src/app/scip_blend.rs
err-sched-cancelled-search = Cancelled during the search
err-sched-cancelled-seed = Cancelled while completing the first schedule into a starting point
err-sched-cancelled-formulation = Cancelled while building the model
err-sched-cancelled-before-solve = Cancelled before the solve
err-sched-cancelled-solve = Cancelled during the solve (solver status: { $status })
err-sched-cancelled-extraction = Cancelled while reading the solution
err-sched-cancelled-replay = Cancelled during the replay
err-sched-cancelled-window = Cancelled during a day-by-day window
err-sched-cancelled-scoring = Cancelled while scoring the grade targets
err-sched-solver-stopped = SCIP stopped with { $error }
err-sched-window-label = window { $position } of { $total }
err-sched-window-no-time = { $label }: no time left
err-sched-window-invalid-input = { $label }: invalid window input: { $problem }
err-sched-window-no-schedule = { $label }: no schedule within { $limit } ({ $status })
err-sched-window-replay-rejected = { $label }: the replay rejected the window: { $issue }
err-sched-stitched-replay-failed = The combined schedule failed the replay over the whole horizon: { $issue }
err-sched-set-pricing = Could not set the SCIP pricing rule: { $error }
err-sched-set-linear-check = Could not set the SCIP linear check: { $error }
err-sched-set-time-limit = Could not set the SCIP time limit: { $error }
err-sched-set-gap = Could not set the SCIP gap: { $error }
err-sched-dispatch-rejected = The replay rejected the dispatch schedule: { $issue }
err-sched-seed-retry = { $problem }; with the movements alone, { $again }
err-sched-seed-missing-cells = The starting point names { $missing } movement cells that the whole-horizon model does not have
err-sched-seed-no-time = No time left to complete the starting point
err-sched-seed-configure = Could not set up the starting point's completion: { $error }
err-sched-seed-stopped = SCIP stopped completing the starting point with { $error }
err-sched-seed-none = SCIP found no completion ({ $status })
err-sched-seed-offer = Could not offer the starting point to SCIP: { $error }
err-sched-seed-solve-configure = Could not set up the solve from the starting point: { $error }

# src/app/schedule_publish.rs
err-sched-pub-dig-outside-sequence = The published digging names a block outside its authored sequence
err-sched-pub-unknown-candidate = The published movement names an unknown candidate
err-sched-pub-dig-no-bar = The published digging has no authored bar
err-sched-pub-dig-unknown-bar = The published digging names an unknown bar
err-sched-pub-dig-non-dig-bar = The published digging belongs to a bar that is not a dig
err-sched-pub-dig-unknown-cell = The published digging names an unknown cell
err-sched-pub-unattributed = The replay did not attribute every published movement to a bar
err-sched-cancelled-publication = Cancelled during publication
err-sched-pub-row-unknown-candidate = A movement row names an unknown candidate { $candidate }
err-sched-pub-row-unknown-cell = A movement row names an unknown cell ({ $interval }, { $segment })
err-sched-pub-row-no-bar = Movement row { $index } was worked under no authored bar
err-sched-pub-no-bar = A movement names a bar that was not recorded when the schedule was captured
err-sched-pub-no-loader = A movement names a loader that was not recorded when the schedule was captured
err-sched-pub-no-source = A movement names a source that was not recorded when the schedule was captured
err-sched-pub-no-destination = A movement names a destination that was not recorded when the schedule was captured
err-sched-pub-no-truck = A movement names a truck class that was not recorded when the schedule was captured
err-sched-pub-value-not-finite = A movement value is not finite
err-sched-pub-depleted = Dig block { $block } was depleted past its measured tonnes

# src/app/schedule_solve.rs
err-sched-cancelled-validation = Cancelled before the input was checked
err-sched-cancelled-input-validation = Cancelled while the input was being checked

# src/app/scip_blend.rs
err-sched-interrupt-rejected = The solver rejected a request to interrupt its search
err-sched-incumbent-rejected = The independent replay rejected the best schedule found

# src/app/schedule_run.rs
err-sched-no-replayed-schedule = The run holds no replayed schedule

# src/model/schedule/optimisation/blended/lp.rs
err-sched-solver-ended = The solver ended with status { $status }
err-sched-bad-coefficient = A constraint has an invalid coefficient { $coefficient } on column { $column }
err-sched-empty-constraint = A constraint with no terms needs a value from { $lower } to { $upper }
err-sched-conflicting-constraints = Two constraints over the same terms need at least { $lower } and at most { $upper }

# src/model/schedule/optimisation/blended/relaxation.rs
err-sched-relaxation-stopped = Stopped while building the relaxation
err-sched-relaxation-ended = The relaxation solve ended with status { $status }
err-sched-relaxation-non-finite = The relaxation reported an optimum that is not finite ({ $objective })

# src/model/schedule/optimisation/blended/greedy.rs
err-sched-clearance-fleet-hours = Blast clearance needs more fleet hours than remain
err-sched-interval-cancelled = Interval { $interval }: cancelled
err-sched-block-undiggable = Interval { $interval }: block { $block } must be finished but no loader digs it
err-sched-interval-failed = Interval { $interval }: { $reason }

# src/model/schedule/optimisation/blended/routing.rs
err-sched-no-destination-left = No destination is left for dig group { $group } in interval { $interval }, segment { $segment }

# src/model/schedule/optimisation/scip/plan.rs
err-sched-no-plan = No plan was found (solver status { $status })

# src/app/commands/solids.rs
err-solids-no-preview = No solid is being previewed
err-solids-still-building = The solid is still being built
err-solids-no-topography = This solid has no topography yet, so its preview is the design surface itself

# src/model/solid_reserves.rs
err-solids-band-range = Reserve band { $base } to { $top } is not a finite, non-empty elevation range
err-solids-piece-slot = A reserve piece names a result slot outside the partition
err-solids-bands-order = Reserve bands must be ordered from the bottom up and must not overlap
err-solids-block-counts = The block geometry and attribute counts disagree
err-solids-missing-geometry = Missing block geometry at block { $index }
err-solids-piece-context = In the piece from RL { $base } to { $top }, against the block centred at ({ $x }, { $y }, RL { $z }) of { $size_x } x { $size_y } x { $size_z }
err-solids-overlap = A block's overlap exceeds its volume across the planning pieces, so the pieces overlap each other


# src/model/mod.rs
err-bm-polyline-min-vertices = { $what } needs at least 2 vertices
err-bm-polyline-bad-vertex = { $what } contains an invalid vertex
err-bm-polyline-bad-bulge = { $what } contains an invalid bulge
err-bm-object-bad-colour = Invalid colour
err-bm-point-bad-position = Invalid point position
err-bm-closed-polyline-min-vertices = A closed polyline needs at least 3 vertices unless it contains an arc
err-bm-polyline-bad-line-weight = Invalid polyline line weight
err-bm-circle-bad-centre = Invalid circle centre
err-bm-circle-bad-radius = The circle radius must be greater than zero
err-bm-circle-bad-line-weight = Invalid circle line weight
err-bm-text-bad-position = Invalid text position
err-bm-text-bad-height = Invalid text height
err-bm-text-bad-rotation = Invalid text rotation
err-bm-layer-id-range = Layer "{ $name }" has id { $id }, outside the 32-bit project id range
err-bm-layer-id-duplicate = Duplicate layer id { $id } ("{ $name }")
err-bm-layer-bad-elevation = Layer "{ $name }" has an invalid elevation
err-bm-layer-bad-colour = Layer "{ $name }" has an invalid colour
err-bm-object-id-range = { $kind } object has id { $id }, outside the 32-bit project id range
err-bm-object-id-duplicate = Duplicate object id { $id }
err-bm-object-missing-layer = { $kind } object { $id } references missing layer { $layer }
err-bm-object-invalid = Object { $id }: { $error }
err-bm-hidden-object-missing = Hidden object id { $id } does not reference a design object

# src/model/mod.rs
err-bm-haul-id-range = Haul road ids are outside the project id range

# src/model/forest.rs
err-bm-classifier-truncated = The classifier model file is truncated
err-bm-classifier-unknown-format = The classifier model has an unknown format
err-bm-classifier-class-count = The classifier model has an unsupported number of classes ({ $class_count })
err-bm-classifier-leaf-range = The classifier model has a leaf out of range
err-bm-classifier-missing-feature = The classifier model splits on a feature it does not have
err-bm-classifier-branch-range = The classifier model has a branch out of range
err-bm-classifier-empty-tree = The classifier model has an empty tree
err-bm-classifier-trailing-data = The classifier model has extra data after its last tree

# src/model/kriging.rs
err-bm-kriging-bad-bounds = Block model bounds and cell sizes must be finite and positive
err-bm-kriging-dimension-range = The block grid dimension is outside the supported range
err-bm-kriging-cell-overflow = The block grid has too many cells
err-bm-kriging-no-samples = Ordinary kriging needs at least one finite sample
err-bm-kriging-bad-variogram = The variogram range and sill must be positive and the nugget must not be negative
err-bm-kriging-bad-neighbours = Neighbour counts must satisfy 1 <= minimum <= maximum <= 64
err-bm-kriging-out-of-memory = Not enough memory to hold { $count } kriging estimates

# src/model/block_model.rs
err-bm-grid-dimensions-overflow = The regular block grid dimensions are too large
err-bm-grid-count-mismatch = The regular block grid dimensions imply { $len } blocks, but the block model reports { $expected_len }
err-bm-grid-voxel-zero = The regular block grid voxel dimensions must not be zero
err-bm-grid-bounds-invalid = The regular block grid bounds are invalid
err-bm-scaled-grid-invalid = The scaled block grid is invalid
err-bm-scaled-bounds-invalid = The scaled block bounds are invalid

# src/rendering/scene/block_model_volume_cache.rs
err-bm-volume-cell-bytes-overflow = The block volume has too many cells
err-bm-volume-cells-alloc = Could not allocate { $bytes } bytes for the block volume cells: { $error }
err-bm-volume-cells-browser-limit = The block volume cells need { $bytes } bytes, which is over the 1 GiB browser limit
err-bm-volume-temp-size = Could not size the temporary cell file to { $bytes } bytes: { $error }
err-bm-volume-temp-map = Could not map the temporary cell file: { $error }
err-bm-volume-temp-flush = Could not flush the temporary cell file: { $error }
err-bm-volume-temp-seal = Could not seal the temporary cell file: { $error }
err-bm-volume-temp-create = Could not create the temporary cell file: { $error }
err-bm-volume-temp-permissions = Could not restrict the temporary cell file permissions: { $error }
err-bm-volume-metadata-overflow = The block volume brick metadata is too large
err-bm-volume-metadata-too-large = The block volume brick metadata would need at least { $mib } MiB
err-bm-volume-brick-grid-overflow = The block volume brick grid is too large
err-bm-volume-occupancy-alloc = Could not allocate the brick occupancy table: { $error }
err-bm-volume-sparse-count-overflow = The sparse block volume has too many cells
err-bm-volume-sparse-bytes-overflow = The sparse block volume cells are too large
err-bm-volume-sparse-too-large = The sparse block volume would need { $mib } MiB for its cells
err-bm-volume-address-alloc = Could not allocate the brick address table: { $error }
err-bm-volume-x-cells-range = The block volume has too many cells along X
err-bm-volume-y-cells-range = The block volume has too many cells along Y
err-bm-volume-z-cells-range = The block volume has too many cells along Z

# src/app/commands/block_model.rs
err-bm-csv-only = Only CSV block model sources are supported
err-bm-source-missing = The block model source does not exist: { $path }
err-bm-csv-read-failed = Could not read the CSV block model { $path }
err-bm-csv-parse-failed = Could not parse the CSV block model { $name }
err-bm-csv-inspect-failed = Could not inspect the CSV block model { $name }
err-bm-drillholes-not-loaded = The selected drillholes are no longer loaded
err-bm-no-numeric-variable = Select at least one numeric drillhole variable
err-bm-variable-duplicate = The drillhole variable "{ $variable }" was selected more than once
err-bm-variable-not-numeric = The drillhole variable "{ $variable }" is not numeric
err-bm-variable-too-few-samples = The variable "{ $variable }" has only { $count } usable interval samples, but at least { $min_samples } are required
err-bm-variable-no-estimates = The variable "{ $variable }" has no block centres with at least { $min_samples } samples inside the search radius
err-bm-kriging-no-variables = No variables were selected for kriging
err-bm-model-not-loaded = The selected block model is no longer loaded
err-bm-ore-filter-empty = No blocks match the ore filter
err-bm-ore-index-range = Ore block index { $index } is out of range
err-bm-ore-bad-bounds = Ore block { $index } has invalid bounds
err-bm-ore-too-many-vertices = The ore triangulation has too many vertices
err-bm-job-decoding-colour = Decoding block model colour variable…
err-bm-job-kriging = Ordinary kriging…

# src/app/commands/drill_hole.rs
err-drill-browser-loop-unavailable = The browser event loop is unavailable
err-drill-open-project-first = Open a project before importing drillholes
err-drill-dhd-unsupported = DHD drillhole sources are no longer supported
err-drill-csv-sources-missing = One or more drillhole CSV files no longer exist
err-drill-csv-bundle-parse-failed = Could not parse the mapped drillhole CSV files
err-drill-omf-via-importer = OMF drillhole data is loaded through the project importer

# src/ui/dialogs/drill_pattern.rs
err-drill-pattern-pick-boundary = Pick a closed polyline or circle to define the blast shape

# src/model/haulage/network.rs
err-haul-invalid-position = Enter a finite road position
err-haul-node-ids-exhausted = No more road node numbers are available
err-haul-road-ids-exhausted = No more road numbers are available
err-haul-road-needs-two-nodes = A road needs two different nodes
err-haul-invalid-road-shape = The road's shape has an invalid point
err-haul-unknown-node = That road node no longer exists
err-haul-unknown-road = That road no longer exists
err-haul-unknown-segment = That road segment no longer exists
err-haul-select-two-nodes = Select two different nodes
err-haul-node-roles-differ = The nodes have different destination roles
err-haul-settings-positive = Haulage settings must be positive numbers
err-haul-duplicate-role = Two nodes serve the same destination role
err-haul-invalid-node = A road node is duplicated or has an invalid position
err-haul-invalid-topology = A road joins missing or identical nodes, or is duplicated
err-haul-invalid-geometry = A road has an invalid shape or speed limit
err-haul-zero-length-segment = A road has a segment of zero length
err-haul-invalid-block-link = A block link has no nodes or an invalid position

# src/model/optimization_run/values.rs
err-opt-too-many-routes = There are too many processing routes

# src/app/commands/survey.rs
err-survey-invalid-model-extent = The block model's extent is invalid after scaling
err-survey-missing-block-bounds = A block has no bounds
err-survey-invalid-block-coordinate = A block coordinate is invalid after the transform
err-survey-empty-point-cloud = The point cloud has no valid points
err-survey-invalid-point-size = The point size is invalid after the transform

# src/model/optimization_run/report.rs
err-opt-report-finish = Could not finish the report
err-opt-report-missing-column = The file has no '{ $name }' column
err-opt-report-element-columns = The element columns of the file are not in groups of five
err-opt-report-element-group = An element column group does not end in revenue and cost
err-opt-report-grade-column = An element grade column is not named as expected
err-opt-report-contained-column = An element contained column is not named as expected
err-opt-report-not-a-number = '{ $text }' is not a number
err-opt-report-shell-number = A shell number is not a whole number
err-opt-report-no-incremental = The file has no incremental rows

# src/app/video_export/mod.rs
err-video-renderer-not-ready = The renderer is not initialised yet

# src/app/video_export/encode.rs
err-video-create-file = Could not create { $path }
err-video-encoder-settings = The AV1 encoder refused the settings: { $error }
err-video-encode-failed = AV1 encoding failed: { $error }
err-video-malformed-output = The AV1 encoder produced malformed output
err-video-truncated-output = The AV1 encoder produced truncated output
err-video-gif-too-large = The viewport is too large for a GIF

# src/rendering/graphics/video_capture.rs
err-gfx-video-frame-read = A video frame could not be read from the graphics card

# src/rendering/graphics/readback.rs
err-gfx-poll-failed = Waiting for the graphics card failed: { $error }
err-gfx-readback-dropped = The image could not be read from the graphics card
err-gfx-buffer-map = The image could not be read from the graphics card: { $error }
err-gfx-image-format = Image export does not support the display format { $format }

# src/rendering/graphics/slice_preview.rs
err-gfx-preview-format = The plan preview window does not support the main viewport format
err-gfx-preview-present-mode = The plan preview window has no supported present mode
err-gfx-preview-alpha-mode = The plan preview window has no supported alpha mode

# src/rendering/graphics/init.rs
err-gfx-no-adapter = No compatible graphics adapter found: { $error }
err-gfx-no-surface-format = The display reports no supported 8-bit sRGB format
err-gfx-no-present-modes = The display reports no supported present modes
err-gfx-no-alpha-modes = The display reports no supported alpha modes

# src/rendering/text.rs
err-gfx-font-parse = Could not read font face { $face }: { $error }
err-gfx-font-missing = The font face is no longer available

# src/fonts.rs
err-font-no-window = { $url }: no browser window
err-font-not-response = { $url }: not a valid response
err-font-http = { $url }: HTTP { $status }
err-font-fetch = { $url }: { $error }

# src/rendering/machines.rs
err-gfx-model-truncated = The machine model file is truncated
err-gfx-model-chunk = The machine model file has a truncated section
err-gfx-model-no-view = The machine model has data without a buffer
err-gfx-model-view-missing = The machine model is missing a buffer
err-gfx-model-view-range = The machine model data lies outside its buffer
err-gfx-model-accessor = The machine model is missing data
err-gfx-model-node = The machine model is missing a part
err-gfx-model-mesh = The machine model is missing a mesh
err-gfx-model-positions = The machine model has a mesh without positions
err-gfx-model-no-json = The machine model file has no description section
err-gfx-model-read = Could not read the { $model } machine model
err-gfx-model-float-vectors = The machine model has an unsupported vector format
err-gfx-model-scalar-indices = The machine model has an unsupported index format
err-gfx-model-embedded = The machine model must keep its data in the file itself
err-gfx-model-component = The machine model uses an unsupported data type ({ $kind })

# src/rendering/machines.rs (continued)
err-gfx-model-not-glb = The machine model is not a binary glTF file
err-gfx-model-version = The machine model uses glTF version { $version }, not 2
err-gfx-model-accessor-range = The machine model data lies outside its buffer
err-gfx-model-too-deep = The machine model has too many nested parts
err-gfx-model-index-range = The machine model has a face that points past its vertices
err-gfx-model-empty = The machine model has no faces


# src/app/memory.rs and its callers (browser builds)
err-memory-estimate-overflows = The memory estimate for the { $what } is too large
err-memory-over-budget = The { $what } needs { $needed } MiB, but only { $remaining } MiB is left of the browser's 3 GiB working memory
err-memory-what-block-model = block model
err-memory-what-block-volume = block model volume view
err-cloud-too-many-to-classify = The point cloud has too many points to classify in one pass
err-solids-invalid-block-bounds = A block has invalid bounds
err-solids-invalid-block-transform = A block has an invalid position or scale
err-solids-flitch-no-footprint = The flitch at RL { $rl } has no valid dig block outline
video-filter-mp4 = MP4 video (AV1)
video-filter-mov = QuickTime movie (AV1)
video-filter-webm = WebM video (AV1)
video-filter-gif = GIF animation
err-file-desc-video = video
err-file-desc-project = project
err-file-desc-opt-scenarios = optimisation scenarios
err-file-desc-opt-report = optimisation report
