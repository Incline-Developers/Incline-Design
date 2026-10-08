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
common-currency-symbol = $
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
planning-page-setup = Setup
planning-page-schedule = Schedule

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
planning-subpage-view = View
planning-subpage-layout = Layout
planning-subpage-animate = Animate
planning-subpage-calendar = Calendar

## Solids Reserves setup

planning-field-list = Field List
planning-block-models = Block Models
planning-solids = Solids
planning-benching = Benching
planning-blasting = Blasting
planning-benches = Benches
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
planning-computing-reserves = Computing selected reserves…
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
planning-reserve-categories = { $count } categories
planning-reserve-unavailable = Block model data is unavailable; reload the model to compute reserves
planning-volume-unavailable = Preview available · volume requires closed meshes

## Schedule setup: the loader fleet, and the Gantt it draws rows for

schedule-loader-classes = Machine Classes
schedule-loader-agents = Machines
schedule-class = Class
schedule-dig-rate = Default dig rate (t/h)
schedule-effective-rate = Class rate
schedule-tph = tph
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
schedule-calendar-empty-override = Empty calendar overrides must not be stored
schedule-calendar-read-only = The class default rate is edited in Schedule Setup
schedule-calendar-duplicate-cell = A calendar edit contains the same cell more than once
schedule-calendar-period-overflow = The calendar period is too large
schedule-calendar-effective-rate = The effective production rate is too small or large to represent
schedule-calendar-edit = Edit loader calendar
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
report-dug-t = Dug (t)
report-dig-h = Digging (h)
report-reclaim-h = Reclaiming (h)
report-delay-h = Delayed (h)
report-idle-h = Idle (h)
report-idle-reason-h = Idle: { $reason } (h)
schedule-calendar-total-help = The whole calculated schedule: tonnes, truck-hours and value summed; closing stock and deposited tonnes at its end; grades weighted by tonnes.
schedule-calendar-day = Day { $day }
schedule-calendar-hours = { $start }–{ $end } h
schedule-calendar-loaders = Loaders
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
schedule-error-unknown-agent = That loader agent is no longer in this project
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
schedule-readiness-state = State
schedule-readiness-last-run = Last run
schedule-readiness-blocks = Dig blocks offered
schedule-readiness-tonnage-field = Tonnage field
schedule-readiness-never-run = Not run yet
schedule-readiness-result-stale = Retired by an edit since; shown as it stood
schedule-readiness-result-current = Current
schedule-stage-no-tonnage-field = Choose the reserve field read as tonnes in Configuration
schedule-stage-tonnage-field-not-summed = This field is not summed, so it cannot be read as tonnes
schedule-stage-no-classes = Add a loader class before scheduling
schedule-stage-no-agents = Add a loader agent before scheduling
schedule-stage-agent-no-class = This machine's loader class is no longer in this project
schedule-stage-waiting-solids = Waiting on the Solids pipeline: { $reason }
schedule-stage-duplicate-solid = { $solid } is cut from the same design, topography and block model as { $other }, so both would mine the same ground. Delete one, or change its inputs.
schedule-stage-blocks-unmeasured = { $count } dig blocks have no tonnage on the chosen field. Fix the block models, exclude the ground, or count these blocks as 0 t.
schedule-stage-blocks-negative-tonnes = { $count } dig blocks hold negative values on the tonnage field, such as -99 for a blank. Those values are counted as 0 t.
schedule-not-ready-not-run = { $step } has not been run
schedule-not-ready-stale = { $step } is out of date; run Schedule Setup again
schedule-not-ready-running = { $step } is still running
schedule-not-ready-failed = { $step }: { $message }
schedule-run-stopped-by-edit = Schedule Setup run stopped: the { $step } step's inputs changed while it was running
schedule-result-superseded = A Schedule Setup result arrived from a run that had already been superseded, and was discarded
schedule-calculation-ready = Ready to calculate
schedule-calculation-blocked = Calculation unavailable — { $reason }
schedule-bar-height = Bar height (px)
gantt-settings = Gantt Settings
schedule-tonnage-field = Tonnage field
schedule-tonnage-field-none = Not chosen
schedule-tonnage-field-no-fields = This project has no reserve fields yet. Add them in Solids → Setup → Field List
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
destination-stage-no-rules = Destination routing is on but no rule is enabled
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
destination-rule-no-loaders = No loaders in the fleet yet.
destination-no-destinations = No destinations yet.
destination-select-all = Select all
destination-rule-sources = Sources
destination-rule-all-sources = All sources
destination-rule-conditions = Conditions
destination-no-rules = No rules. Every rule names one destination and the material allowed to reach it. Right-click to add one.
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
haul-draw-help = Click points in the viewport; each becomes a road node. They sit at the Z level, or on what a snap mode finds. Click a road or node to join it. Backspace removes the last point; Enter, Escape or a double-click finishes.
haul-convert = Convert selection to roads
haul-import = Import DXF as roads…
haul-import-heading = Import DXF as Haul Roads
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
haul-join-nodes-help = Merge the two selected nodes into one, joining their roads.
haul-delete-help = Delete the selection (Delete key). Roads left without a road at an end lose that end too.
haul-delete-road = Delete road
haul-dead-end-help = A road end with no other road and no destination. Fine at a pit floor; otherwise the road may be meant to join another.
haul-near-miss-help = A road end close to another road but not joined to it. Trucks cannot pass between them.
haul-separate-piece-help = Roads not connected to the main network. Destinations on the main network cannot be reached from them.
haul-steep-help = Part of this road is steeper than a truck class's maximum grade. It is still used; check the design or the class setting.
haul-missing-destination-help = This node is a point for a destination that no longer exists.
haul-use-selected-node = The selected node
haul-method-help = How trucks reach this destination: a road node you chose, or the road nearest its surface.
haul-from = From
haul-to = To
haul-from-pile = Reclaim from { $pile }
haul-need-truck = Add a truck class in Haulage Setup to check routes.
haul-need-loader = Add a loader in Schedule Setup to check routes.
haul-match-help = Theoretical matching: trucks are assumed never to queue at the loader or the destination.
haul-profile-legend = elevation · speed
haul-profile-hover = { $distance } m · { $elevation } m RL · { $speed } km/h
haul-unconnected-note =
    { $areas ->
        [one] 1 dig block has
       *[other] { $areas } dig blocks have
    } no road node within the auto-join distance. Their trucks are assumed to drive straight to the nearest node (up to { $longest } m), never steeper than the truck's maximum grade. Haulage → Layout shows these blocks in red.
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
drill-blast-missing = { $bar }: { $count } blast(s) in this bar are not in the planning run
drill-blast-unworked = Loaders wait on these blasts all horizon: they are dug by a bar, but no machine bar works the step named, and they do not start Fired. Add them to a blast bar on that kind of machine, or set their starting stage in Setup → Drill & Blast: { $blasts }
drill-blast-unworked-entry = { $blast } ({ $step })
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
haul-grade-speeds = Grade Speeds
haul-grade-band = Grade band
haul-default-speeds = Use default grade speeds
haul-error-grade = Enter a grade above 0 and up to 100 %.
haul-error-seconds = Enter a number of seconds, 0 or more.
haul-remove-band = Delete Grade Band
haul-speeds-help = Speeds for a large haul truck on a road with 2% rolling resistance: rimpull uphill, retarder downhill. A band runs up to the next grade.
haul-open-layout = Open Haulage layout
haul-dump-override = Override dump time (s)
haul-unlimited = Default
haul-convert-help = Turn the selected design polylines into haul roads. Ends and crossings within the join tolerance become shared nodes; the polylines stay as they are.
haul-empty = No roads yet. Draw one, or convert or import pit design strings.
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
haul-block-joined = Chosen nodes · { $length } m
haul-block-nearest = Nearest node · { $length } m
haul-block-far = Nearest node is { $length } m away, beyond the auto-join distance
haul-block-no-roads = No roads yet
haul-link-pick = Join to road nodes…
haul-link-help = Join the selected blocks to nodes you choose instead of the nearest road. Pick one or more; each block uses whichever gives the quickest cycle.
haul-link-clear = Use nearest node
haul-link-clear-help = Let these blocks join whichever node is nearest again.
haul-link-picking = Click nodes or road points, then Join.
haul-step-network = Road Network
haul-join-title = Join to Road
haul-blocks = Blocks
haul-nodes = Nodes
haul-join-blocks = Join
haul-join-blocks-help = Join the selected blocks to the picked nodes. A point on a road becomes a node.
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
haul-flow-hover = { $rate } t/h loaded
haul-flow-legend = Loaded hauls now · stripes run faster and wider with tonnage · busiest { $rate } t/h
haul-role-help = Trucks deliver to a dump point and load from a stockpile's reclaim point. A stockpile with no reclaim point is loaded at its dump point.
haul-node-on = End of { $road }
haul-selection = Selection
haul-selection-empty = Click a road, node or block; shift-click for more.
haul-new-road = New Road
haul-points = Points
haul-drawing-keys = Backspace undoes · Enter or double-click finishes
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
haul-link-no-roads-note = There are no roads, so trucks cannot haul anything. Draw them on the Haulage page's Layout.
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
haul-connections-open = Open the Haulage page's Layout to fix it
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
schedule-capture-bar-unassigned = this bar is not assigned to a machine
schedule-capture-bar-block-unmeasured = a dig block in this bar has no measured tonnage
schedule-capture-block-no-tonnes = this block measured no tonnes of the nominated field
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
cmd-split-polyline-into-two = Split source polyline into two open polylines
cmd-text-edit-finished = Finished text edit for object { $object_id }
cmd-text-updated = Updated text on object { $object_id }
cmd-view-centre-rotation-not-available-flying = The centre of rotation is not available in flying mode
cmd-view-fixed-centre-rotation-x-y = Fixed the centre of rotation at { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = No point under the cursor to fix the centre of rotation on
cmd-view-released-centre-rotation = Released the centre of rotation
cmd-view-reset-view-fit-extents = Reset view (fit to extents)
cmd-view-reset-view-plan-same-distance = Reset view (plan at the same distance; click again to fit to extents)
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
pile-rest-help = Hours new material must rest before it can be reclaimed: since the last delivery to the pile, or to the chunk for a chunked one. While the chunk next in the reclaim order rests, the pile is not reclaimed. Opening stock is already rested. 0 lets the pile be tipped on and reclaimed at once.
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
pile-chunk-size-missing = A chunked stockpile needs a chunk size.
pile-chunk-over-capacity = The chunk size is larger than the stockpile's maximum tonnes, so no chunk could ever fill.
pile-chunk-grade-missing = { $chunk } has no { $grade }, which the schedule tracks.
pile-chunk-grade-negative = { $chunk } has a negative { $grade }.
pile-grade-needed = The schedule tracks this grade, so it needs a value.
pile-not-checked = Not checked yet
pile-opening-stock = Opening stock
pile-opening-empty = Starts empty
pile-opening-differs = The chunks differ, so these are their combined values.
pile-opening-split-again = Split into chunks again
pile-opening-combine = Combine into one blend
pile-opening-not-held = —
inventory-blend-name = Opening stock
inventory-chunk-name = Chunk { $number }
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
schedule-run-period-note = Run Period: schedule from hour zero through one more day than the current result (day 1 when there is none), hour by hour.
schedule-run-whole-note = Run All Periods: schedule from hour zero through the planning end day, hour by hour.
schedule-improve = Improve
schedule-improving = Improving…
schedule-improve-note = Improve: start from the hourly schedule and spend up to the solve time limit looking for a better one over the whole horizon. Stop keeps the best found so far.
schedule-improving-note = Looking for a better schedule. Stop keeps the best found so far.
schedule-auto = Auto
schedule-auto-note = Recalculate the hourly schedule on its own whenever the schedule's inputs change. It takes a fraction of a second; Improve still has to be asked for.
schedule-run-cancel-note = Stop the run. What was last calculated stays as it is.
schedule-run-stop-early-note = Stop optimising and keep the schedule on screen.
schedule-run-job = Calculating schedule
schedule-run-never = Not run yet. Press Run All Periods, or switch on Auto.
schedule-run-working = Calculating through day { $day }…
schedule-haulage-waiting = Waiting for Haulage
schedule-haulage-open = Open on the Haulage page
schedule-solids-open = Open on the Solids page
schedule-solids-tonnage = Tonnage
schedule-unmeasured-as-zero = Count unmeasured blocks as 0 t
schedule-blocks-zero = Blocks counted as 0 t
schedule-unmeasured-as-zero-help = Blocks no block model reaches are dug at once and move nothing.
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
idle-not-worth-it-note = Ground, room and trucks were all there, but moving the material was worth less than leaving it. Check the Cashflow values for this material and destination.
schedule-run-needs-solids = Cannot run until the Solids pipeline has been run: the schedule digs the blocks it makes.
schedule-run-at-end = The schedule already reaches the planning end day (day { $day }). Use Run All Periods to recalculate it, or move the planning end.
schedule-run-cancelled = Run { $run } was cancelled. The last calculated schedule is unchanged.
schedule-run-superseded = The schedule changed while it was being calculated. That run was discarded; run it again.
schedule-run-refused-plain = Run { $run } could not start.
schedule-run-refused = Run { $run } could not start: { $count } problems to fix. Hover for the list.
schedule-run-refused-because = Run { $run } could not start. { $reason }
schedule-run-no-solution = Run { $run } reached its time limit without finding a schedule. Nothing was published.
schedule-run-infeasible = Run { $run }: no schedule satisfies the configured rules over this horizon. Nothing was published.
schedule-run-failed = Run { $run } did not produce a valid schedule. Nothing was published.
schedule-run-publication-failed = The validated answer could not be published: { $reason }
schedule-solver-not-started = The solver could not be started: { $reason }
schedule-solver-crashed = The solver stopped unexpectedly ({ $status }). The project and any schedule already shown are unaffected; the details are in the log.
schedule-run-finished = Schedule run { $run } calculated through day { $day }
schedule-run-kept-better = Schedule run { $run } found nothing worth more than run { $held }, which is kept
schedule-run-first = Scheduled through day { $day }
schedule-run-optimal = Scheduled through day { $day } · optimal
schedule-run-limited-no-gap = Scheduled through day { $day }
schedule-run-suffix-event-budget = {" "}· event budget capped
schedule-run-suffix-chunks-full = {" "}· chunk slots full
schedule-run-suffix-unworked-blasts =
    { $count ->
        [one] {" "}· 1 blast has no machine to work it
       *[other] {" "}· { $count } blasts have no machine to work them
    }
schedule-run-no-bars = Drag Dig from the palette onto a loader to start the schedule.
schedule-run-empty-bars = Add dig blocks to a bar to start the schedule: right-click it, then Edit Sequence.
schedule-run-stale = Run { $run } is out of date. The calculated work is hidden until the schedule is run again.
schedule-result-summary = Unique ground: { $started } t · extracted: { $extracted } t · remaining: { $remaining } t
schedule-bar-reclaimed = Reclaimed: { $tonnes } t
schedule-bar-reclaimed-of = Reclaimed: { $tonnes } t of the { $cap } t cap
schedule-dispatch-reclaim = Reclaiming
schedule-dispatch-heading = { $kind } — { $source }
schedule-span-hours = { $from } → { $to } ({ $hours } h)
schedule-tonnes-per-day = { $value } t/day
schedule-dispatch-tonnes-rate = { $tonnes } t at { $rate } t/h
schedule-dispatch-shared-with = Also on this block: { $agents }
schedule-dispatch-delivered = → { $destination }: { $tonnes } t
schedule-dispatch-chunks = Chunks drawn in this interval: { $chunks }
schedule-dispatch-pile-holds = { $stockpile } holds { $tonnes } t at the end of this execution
schedule-calendar-schedule = Schedule
schedule-animation-stale = The schedule result is stale. Run Period or Run All Periods again.
schedule-animation-never = Run Period or Run All Periods to enable time scrubbing.
schedule-animation-inventory = { $stockpile }: { $tonnes } t at this instant
schedule-calendar-value = Movement value ({ $currency })
schedule-calendar-dig-tonnes = Dug (t)
schedule-calendar-reclaim-tonnes = Reclaimed (t)
schedule-calendar-closing-grades = Closing grades: { $grades }
truck-calendar-hours-used = Truck-hours used
destination-calendar-reclaimed = Reclaimed (t)
destination-calendar-closing = Closing stock (t)
schedule-note-event-budget = The derived execution-event budget was capped at { $positions } positions per interval, so some source transitions inside an interval may have been unavailable.
schedule-detail-value = Movement value: { $currency }{ $value }
schedule-detail-bound = Bound: { $bound } · relative gap { $gap }%
schedule-detail-bound-no-gap = Bound: { $bound } · relative gap not reported
schedule-detail-bound-relaxation = Bound: { $bound } · relative gap { $gap }% · proved by the linear relaxation (HiGHS), not by SCIP
schedule-detail-no-bound = Bound not reported by the solver.
schedule-detail-bound-weak = No useful bound on the best possible schedule was reached in the time allowed.
schedule-detail-fixed-blasts = Blast times are the hourly schedule's: Improve reorders the mining around them but does not move a blast, and the bound covers only schedules with these blast times.
schedule-detail-start-day-by-day = Day-by-day schedule ({ $windows } windows)
schedule-detail-start-hourly = Hourly dispatch schedule ({ $intervals } intervals)
schedule-detail-start-kept = { $start } in { $seconds } s, worth { $value }. The whole-horizon solve found nothing better in its time, so this is the published schedule.
schedule-detail-start-early = { $start } in { $seconds } s, worth { $value }. The whole-horizon solve is still looking for a better one.
schedule-detail-start-proven = { $start } in { $seconds } s, worth { $value }. The linear relaxation proved it within the gap target, so the whole-horizon solve was not needed.
schedule-detail-start-stopped = { $start } in { $seconds } s, worth { $value }. The whole-horizon solve was stopped, so this is the published schedule.
schedule-detail-start-only = { $start } in { $seconds } s, worth { $value }. Improve searches the whole horizon for a better one.
schedule-first-schedule-failed = The hourly schedule could not be made ({ $reason }). Improve can still search for one with the full optimiser.
schedule-detail-start-improved = { $start } in { $seconds } s, worth { $value }. The published schedule is the whole-horizon solve's, which started from it.
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
schedule-detail-chunks = { $slots } chunk slot(s) in use; an emptied slot is not reused within the horizon, which can limit total receipts.
schedule-detail-grade-margin = Grade boundaries: a route is closed within { $fraction } (fraction) or { $percent } percentage points inside each bound. Conditional values within that band are valued conservatively by the optimiser; published values use the authored boundary.
schedule-detail-grade-band = { $deliveries } conditional-value deliveries ({ $tonnes } t) fell within that band; they could shift the value by at most { $value }.
schedule-detail-indicator-leak = Conditional-value indicators were solved to the backend's integrality tolerance; the optimiser's own figure may differ from the published value by up to { $value }.
schedule-detail-omitted = { $rows } solver rows totalling { $tonnes } t were too small to time and are not shown.
schedule-gantt-window = Window
schedule-gantt-window-open = { $from } onwards
schedule-gantt-window-span = { $from } → { $to }
schedule-bar-set-window = Set Work Window…
schedule-window-dialog = Work Window
schedule-window-start = Start (hours)
schedule-window-end = End (hours)
schedule-window-end-hint = Blank for open-ended
schedule-window-invalid = The end must be a number after the start, or blank
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

sequence-editor-title = Edit Sequence — ⁨{ $bar }⁩
sequence-editor-no-run = ⁨{ $reason }⁩ Run Solids → Dig Strips to pick dig blocks for this bar.
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
sequence-objects = Objects

## Schedule → Gantt

planning-subpage-gantt = Gantt
planning-subpage-charts = Charts
gantt-empty-fleet = No agent rows yet. Add loader classes and loader agents in Setup → Site Data
schedule-add-work = Add work
schedule-open-setup = Open Schedule Setup
schedule-open-solids-setup = Open Solids Setup
gantt-reset-view = Reset View
gantt-zoom-in = Zoom in
gantt-zoom-out = Zoom out
gantt-day = Day { $day }
gantt-day-time = Day { $day }, { $time }
gantt-range = { $from } → { $to }
gantt-slider-away = Bring the time slider into view
gantt-inspector = Inspector
gantt-inspector-help = What every machine, stockpile, crusher and truck fleet is doing at the time slider. Click the ruler or drag the slider to move it; ← and → step an hour, with Shift a day.
inspector-next-hour = Next hour (→)
inspector-previous-hour = Previous hour (←)
inspector-no-schedule = Nothing is calculated yet. This fills in once the schedule is.
inspector-beyond = The schedule is calculated to { $end }. Move the slider back to inspect it.
inspector-loaders = LOADERS
inspector-stockpiles = STOCKPILES
inspector-crushers = CRUSHERS
inspector-dumps = DUMPS
inspector-trucks = TRUCKS
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
charts-feed-help = Tonnes fed to the crusher, hour by hour. Gaps are hours it was not fed - often because the day's limit was reached. The figure is at the time slider.
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
planning-stat-avg = Avg
planning-stat-min = Min
planning-stat-max = Max
planning-blast-name-taken = { $blast } already exists
planning-blasts = Blasts
planning-min-blast-area = Minimum blast
planning-min-blast-area-help = Blasts smaller than this are left out of mining and get no name.
planning-blasts-empty = Select a bench to see its blasts
planning-blasts-one-bench = Select one bench to draw cuts
planning-dig-one-flitch = Select one flitch to draw strips
planning-blast-count =
    { $count ->
        [one] { $count } blast
       *[other] { $count } blasts
    }
planning-dig-strips = Dig Strips
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
stage-stale-hint = Edited since its last run. Run Step or Run All brings it up to date, or tick Auto to have it rerun on its own; the steps after it keep their earlier results until then.
planning-auto = Auto
planning-auto-note = Rerun out-of-date steps half a second after your last edit.
schedule-setup-auto-note = Rerun out-of-date steps and recalculate the schedule after your last edit. The same switch as the Gantt's Auto.
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
stage-waiting-solids = Waiting on { $count } solids
stage-no-occupied-bands = The benching plan reaches no material in this solid
stage-run-step = Run
stage-progress-short = { $done }/{ $total }
stage-run-all = Run All
stage-cancel = Cancel Run
stage-diagnostics = Diagnostics
planning-reserve-coverage = Model coverage
planning-reserve-coverage-note = Block-model volume measured into the shown geometry, as a share of its geometric volume
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
planning-snapshot-ready = Ready · { $blocks } dig blocks
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
truck-open-calendar-help = Set the fleet day by day on this class's Calendar row.
truck-new-rule = New Trucking Rule
truck-duplicate-rule = Duplicate Trucking Rule
truck-delete-rule = Delete Trucking Rule
truck-no-rules = No trucking rules yet. Right-click to add one.
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
truck-stage-no-rules = No trucking rules are configured, so no truck class is permitted anywhere.
schedule-calendar-trucks = Trucks
truck-calendar-units = Units
truck-calendar-availability = Availability (%)
truck-calendar-utilisation = Utilisation (%)

## Cashflow

cashflow = Cashflow
cashflow-error-invalid-value = Value must be a number.
cashflow-error-unknown-rule = That cashflow rule is no longer in this project.
cashflow-error-unrepresentable = Those values produce a figure no number can express.
cashflow-activity-all = Any activity
cashflow-activity-dig = Dig
cashflow-activity-reclaim = Reclaim
cashflow-activity = Activity
cashflow-rule = Cashflow rule
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
cashflow-new-rule = New Cashflow Rule
cashflow-duplicate-rule = Duplicate Cashflow Rule
cashflow-delete-rule = Delete Cashflow Rule
cashflow-no-rules = No cashflow rules yet. Right-click to add one.
cashflow-select-rule = Select a cashflow rule.
cashflow-default-rule-name = Cashflow rule
cashflow-help = Matching rules add together. Values guide optimisation; they do not guarantee movement priority.
cashflow-stage-no-rules = No cashflow rules are configured. Movements are worth nothing.
cashflow-stage-zero-value = { $rule } describes movements but pays nothing.

## Stockpile inventory and reclaim

inventory-order-fifo = FIFO (oldest first)
inventory-order-lifo = LIFO (newest first)
inventory-error-invalid-tonnes = Tonnes must be a number above zero.
inventory-error-invalid-value = That value is not a number, or the category is blank.
inventory-error-duplicate-value = That field already has a value on this portion.
inventory-error-unknown-lot = That chunk is no longer in this stockpile.
inventory-error-unknown-portion = That portion is no longer in this chunk.
inventory-error-last-portion = A chunk needs at least one portion. Delete the chunk instead.
inventory-error-lot-at-end = That chunk is already at the end of the order.
inventory-error-over-capacity = Opening inventory would exceed this stockpile's capacity.
inventory-error-stockpile-full = This stockpile is at its capacity.
inventory-error-overdrawn = That chunk does not hold that many tonnes.
inventory-error-invalid-interval = That is not a model interval.
inventory-reclaim-order = Reclaim order
inventory-opening = Opening inventory
inventory-no-lots = The stockpile starts empty.
inventory-new-lot = New chunk
inventory-duplicate-lot = Duplicate Chunk
inventory-delete-lot = Delete Chunk
inventory-move-lot-older = Move Towards Oldest
inventory-move-lot-newer = Move Towards Newest
inventory-default-lot-name = Chunk
inventory-lot = Opening chunk
inventory-chunk-column = Chunk (oldest first)
inventory-lot-name = Name
inventory-lot-tonnes = Tonnes
inventory-portion = Portion
inventory-portions = Portions
inventory-new-portion = New Portion
inventory-delete-portion = Delete Portion
inventory-portion-missing = —
inventory-select-lot = Select an opening chunk.
inventory-stage-over-capacity = { $stockpile } opens with more than its capacity.
inventory-stage-invalid-fields = { $count } opening-inventory properties refer to missing fields or incompatible field types.
inventory-field-missing = Missing field #{ $id }
inventory-field-incompatible = { $field } (incompatible type)
inventory-field-incompatible-note = The saved value is preserved, but its field type has changed.

## Reclaim bars

destination-error-category-unsupported = Categories are not retained in blended stockpiles, so this rule cannot test one.
destination-condition-category-blocked = Categories are not retained in blended stockpiles, and this rule's sources can include stockpiles (All covers any added later). Tick only pits, benches or flitches in Sources to test a category.
destination-rule-category-conflict = This rule tests a category on material it may reclaim: { $fields }.
reclaim-error-not-a-stockpile = A reclaim bar's source has to be a stockpile.
reclaim-error-invalid-limit = Maximum tonnes must be a number above zero, or blank.
reclaim-error-wrong-activity = That edit does not apply to this kind of bar.
delay-error-unknown = That delay type, delay list or roster is no longer in this project.
delay-error-type-in-use = This delay type is still used by { $users }. Change those first.
delay-bar-unnamed = a delay bar
date-picker-previous = Previous month
date-picker-next = Next month
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
delay-types-row = Delay Types · { $count }
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
delay-no-lists = No delay lists yet.
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
delay-delete-entry = Delete this delay
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
delay-hours-hint = A number of hours greater than zero.
delay-rosters = Rosters
delay-roster-default = Shift change
delay-no-rosters = No rosters yet.
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
planning-delete-field = Delete Field
planning-no-block-models-project = No block models in this project
planning-extents = Extents: { $lower } → { $upper }
planning-constant = Constant
planning-extents-heading = Extents
planning-blocks = Blocks
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
gantt-delay-heading = { $kind } — { $source }
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
schedule-class-reclaim-rate-help = How fast this class loads trucks from a stockpile
schedule-spot-time-invalid = Enter zero or more seconds

# Schedule optimisation settings
experiment-error-invalid-setting = That value is not one this setting accepts.
experiment-representation-blended = Blended pile
experiment-representation-chunks = Ordered blended chunks
experiment-advanced = Advanced
schedule-capture-event-capacity = Event capacity must be between 1 and 24, or left blank for the derived capacity.
experiment-end-day = Horizon (days)
experiment-end-day-help = Run All Periods solves from hour zero to the end of this day, and Run Period never extends past it. Open-ended bars stop here.
experiment-interval = Calendar interval (h)
experiment-interval-help = The interval sets how finely the calendar is split and when stockpile receipts become reclaimable. It is not a limit of one dig block per interval: a loader may work through several sources inside one interval.
experiment-solve-seconds = Solve time limit (s)
experiment-relative-gap = Relative gap target
experiment-grades = Grades carried
experiment-grades-none = None
experiment-no-grade-fields = No tonnes-weighted average field is defined, so no blend grade can be tracked.
experiment-representation = Representation
experiment-blended-help = Reclaim uses the pile's tonnes-weighted average grades. Opening chunks are combined into one blend, so there is no reclaim order.
experiment-chunks-help = Chunks of the chunk size are filled in order, opening stock first, and reclaimed in the pile's FIFO or LIFO order, each with its own composition. A partly filled chunk keeps receiving until it is full, even across days the pile is not building. Material delivered after a full chunk is emptied goes into the next chunk, so the pile refills all run; its maximum tonnes limit what it holds at once.
experiment-chunk-size = Chunk size
experiment-receiving-chunks = Receiving chunks
experiment-receiving-unlimited = { $size } each, as many as the run needs

# Schedule calculation: capture refusals
schedule-capture-horizon = The requested horizon is not a finite, positive number of hours.
schedule-capture-interval = The calendar interval is not finite and positive.
schedule-capture-block-not-in-run = A dig block in this bar is not in the Solids run the schedule reads.
schedule-capture-opening-grade-missing = '{ $grade }' is missing on an opening portion; a missing grade cannot be read as zero.
schedule-capture-grade-negative = '{ $grade }' is { $value }, which must be a finite, non-negative numeric grade.
schedule-capture-opening-over-capacity = Opening stock of { $opening } t exceeds the pile capacity of { $capacity } t.
schedule-capture-lots-combined = { $stockpile }: { $count } opening chunks combined into one blend; reclaim order does not apply.
schedule-capture-block-uncaptured = Nothing was captured about this block's material, so its blend cannot be computed.
schedule-capture-block-no-tonnes = The tonnage field is not among this block's captured values.
schedule-capture-grade-uncaptured = '{ $grade }' was not captured for this block, and a blend cannot be computed without it.
schedule-capture-grade-missing = '{ $grade }' is missing on some of this block's material; a missing grade cannot be read as zero.
schedule-capture-calendar-non-finite = { $field } is not a finite number
schedule-capture-calendar-non-positive = { $field } must be greater than zero
schedule-capture-calendar-window = a work window ends before it starts
schedule-capture-calendar-intervals = its time steps do not line up
schedule-capture-field-horizon = the horizon
schedule-capture-field-interval = the calendar interval
schedule-capture-field-cashflow = the cashflow total
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
schedule-capture-too-many-columns = Improve needs { $columns } million columns here; its limit is { $ceiling } million. Bring the planning end day in or widen the interval.
schedule-capture-no-work = No assigned bar has work inside the requested horizon. Add dig blocks to a bar (right-click it, then Edit Sequence), or move a bar into the horizon.
schedule-capture-no-movement = No movement is permitted by the current rules, so there is nothing to schedule.
schedule-capture-rehandle = { $rule } permits moving reclaimed material into a stockpile. Rehandling between stockpiles is not modelled, so those movements were left out.
schedule-chunk-opening = Opening chunk { $lot }
schedule-chunk-receiving = Receiving chunk { $number }
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
state-plan-view-then-fit-extents = Plan view at the same distance, then fit to extents
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
tri-solid-role-design = design
tri-solid-role-topography = topography
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
tri-name-assigned-solid = Name assigned to the solid.
tri-north-pit-solid = e.g. north_pit_solid
tri-build = Build

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
viewport-bar-reset-view-plan-over-centre = Reset View: plan over the centre of rotation, click again to fit all
viewport-bar-reset-view-plan-same-distance = Reset View: plan at the same distance, click again to fit all
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
solids-no-solids-yet-add = No solids yet - add one on the Setup page
solids-multiple = Multiple…
solids-selection = Selection
solids-select-solid-bench-flitch = Select a solid, bench or flitch
solids-blast = Blast
solids-plan-area = Plan area
solids-dig-block = Dig block
solids-block-id = Block ID
solids-bench = In bench
solids-flitch = In flitch
solids-block-volume = Block volume
solids-bench-rl = Bench RL
solids-flitch-rl = Flitch RL
solids-contents = Contents

## Animate strings
animate-calculated-solids-unavailable-run = Calculated solids are unavailable. Run Solids through Dig Strips.
animate-updating-schedule-animation = Updating schedule animation
animate-block-volume-could-not = A block's volume could not be measured.
animate-block-retains-more-material = A block retains more material the further it is cut back.
animate-block-shape-could-not = A block's shape could not be cut to the depletion the schedule reports.
animate-day = Day { $day } { $clock }
