# Implementation contract: period calendar for loader units

This replaces the earlier working-interval proposal. Implement this specification directly; do not delegate product or scheduling semantics back to the implementation model. The next deliverable is an editable Calendar subpage, with periods across columns and loader settings down hierarchical rows, integrated into the existing dispatcher.

## 1. Scope and settled decisions

- Schedule navigation becomes **Setup | Calendar | Gantt | Animate**.
- Calendar is a full-width panel, without a separate explorer or 3D viewport. Keep the existing optional console.
- The hierarchy is **Loaders → loader unit → Availability, Utilisation, Rate**. Use existing fleet order and names; no new organizational model.
- Periods are 24-hour elapsed project days in this first increment. Period 0 is labelled Day 1 and covers [0,24); Day 2 covers [24,48). There are no civil dates, time zones, rosters, or configurable period lengths in this increment.
- Availability and utilisation are percentages. Rate is tonnes per productive hour, using the schedule's nominated tonnes field consistently.
- Period settings are averages over that period, not the placement of downtime within it. Do not create artificial shift gaps or randomly timed stops.
- A blank period rate inherits the loader class's current default rate. It does not copy that rate or carry forward the previous period's override.
- Availability and utilisation each have an editable per-loader default, initially 100%. Blank period percentage cells inherit that default.
- Calendar edits are authored content, undoable and persisted. They invalidate calculated schedules, but do not run calculations or rebuild Solids automatically.
- Do not implement dependencies, destinations, haulage, stockpiles, AI, or an optimizer in this change.

## 2. Exact UI

Illustrative layout (empty entries below are genuinely blank cells):

| Setting | Default | Day 1 | Day 2 | Day 3 |
| --- | ---: | ---: | ---: | ---: |
| ▾ Loaders | | | | |
|   ▾ EX01 | | | | |
|     Availability (%) | 90 | | 0 | |
|     Utilisation (%) | 80 | | | 85 |
|     Rate (t/h) | 1,000 | | | 1,200 |
|   ▾ EX02 | | | | |
|     Availability (%) | 100 | | | |
|     Utilisation (%) | 100 | | | |
|     Rate (t/h) | 800 | | 900 | |

The Default column is necessary to explain inheritance without repeating values across every empty period cell. Its availability/utilisation values are editable. Its rate is read-only, reflecting the class default already edited in Setup; do not introduce a second rate-default editor. Hovering that value identifies the class and says it is edited in Setup. Loader and Loaders header rows contain no numeric aggregates.

Layout rules:

- Freeze the left hierarchy column (240 logical pixels), Default column (100), and period header. Period columns are 112 pixels; body rows use the application's existing grid row height/theme.
- Horizontal scroll affects period columns only; vertical scroll affects all body rows together. Paint only visible rows and columns plus a small overscan. Do not create widgets for thousands of off-screen periods.
- All loader groups start expanded. Collapse state, selection, scroll, and typed drafts are transient editor state, keyed by project runtime and stable loader IDs, never names or row indices.
- Initially expose 14 days. Show at least through the latest override and the latest finite authored bar boundary (ceil of end_h / 24; for an open window include its starting day), plus one subsequent day. Extending visible columns changes no saved content and never limits dispatch. A small trailing “+14 days” action extends the view. An explicit day-number jump control scrolls to a distant period without traversing intervening columns; do not enumerate the columns to implement it.
- Header hover shows elapsed hours, e.g. “24–48 h”. Blank cell hover shows the resolved value and its source, e.g. “90% · loader default” or “1,000 t/h · class default”. No placeholder text in every empty cell.
- Use existing selection styling for selection, ordinary text for explicit overrides, and no heatmap, permanent calculated-rate row, daily tonnage row, or readiness badges.
- No loaders: show one “Add loader” action opening Schedule Setup → Loader Agents. Use a short empty-state sentence only here.
- Add one on-demand help entry: “Blank cells use defaults. Production rate = rate × availability × utilisation. Percentages apply across the whole day.” No standing explanatory paragraph.

### Cell editing and navigation

Single click selects a cell; double click, Enter, or typing starts editing. Enter commits and moves down to the next editable field cell; Tab commits and moves right, Shift-Tab left. Escape abandons the draft. Clicking another cell commits a valid draft before moving. Invalid input stays in the editor with one local error; do not silently clamp it. Navigating away discards only the uncommitted draft; committed cells are already saved in the plan.

Accept percentages as `90` or `90%`, storing 0.9; accept decimal rates without units. Trim whitespace. Reject NaN/infinity. Percentage range is inclusive 0–100. Rate overrides must be finite and strictly positive. Use zero availability or utilisation for no production, not a zero rate. Clearing a period cell means inheritance. Clearing a Default percentage restores 100%; the Default rate cannot be edited.

Arrow keys move cell selection when not editing. Delete/Backspace clears selected period overrides; it resets selected editable Default percentages to 100%. Header rows and Default rate cells are not editable and are skipped by keyboard editing navigation.

Support rectangular selection with Shift-click and tab/newline clipboard paste into editable numeric cells. Clipboard parsing is all-or-nothing: reject headers, read-only Default rate cells, out-of-grid rows, or any invalid value without applying a partial rectangle. Empty clipboard fields clear period overrides/reset editable defaults. Preserve trailing empty fields. Copy returns explicit cell text, so inherited cells copy as blanks; do not materialize defaults. Use the existing egui clipboard path so desktop and browser share behavior. One paste or clear rectangle is one undo step. Do not add fill handles or a spreadsheet formula engine.

## 3. Domain model and invariants

Create `src/model/schedule/calendar.rs`, exported from `schedule/mod.rs`. Define:

```rust
pub(crate) const SCHEDULE_PERIOD_H: f64 = 24.0;
pub(crate) struct CalendarPeriod(pub u32); // zero-based, ordered, serde

pub(crate) struct LoaderPeriodOverride {
    pub availability: Option<f64>, // fractions, 0..=1
    pub utilisation: Option<f64>,  // fractions, 0..=1
    pub rate_tph: Option<f64>,     // finite > 0
}

pub(crate) struct LoaderCalendar {
    pub default_availability: f64, // default 1
    pub default_utilisation: f64,  // default 1
    pub periods: BTreeMap<CalendarPeriod, LoaderPeriodOverride>,
}
```

Use crate-private fields/accessors consistently with neighboring code; the sketch specifies shape rather than requiring public mutation. Add `#[serde(default)] calendar: LoaderCalendar` to `LoaderAgent`, with `Default`, clone/equality, serialization, and strict field validation. Missing calendar data loads as continuous 100%/100% with inherited class rate. Do not add calendar data to the scene composite.

A period override is independent of all other periods. Remove entries whose three options are None. Preserve an explicit value equal to today's default: it is an intentional override that must survive a future default change. Distinguish None from zero. Canonicalize negative zero to zero for percentages. Use ordered iteration for hashes and serialization.

Keep schedule metadata version 7 for this additive defaulted field so current saved projects remain readable in the new build. Do not promise old binaries can read newly saved calendar data. Validate the new fields in `SchedulePlan::validate_loaded`; extend `hash_content` and `estimated_bytes`. Agent deletion deletes its owned calendar, and undo restores both through the existing whole-plan command. Agent class reassignment updates inherited rates and retains explicit period rates.

Resolve each setting independently:

```
a = period.availability.unwrap_or(loader.default_availability)
u = period.utilisation.unwrap_or(loader.default_utilisation)
r = period.rate_tph.unwrap_or(class.default_dig_rate_tph)
effective_rate_tph = r * a * u
```

Defaults outside all overrides remain active indefinitely. Zero defaults with a future positive override must still allow that future production. Empty calendar is equivalent to existing behavior. Positive values whose multiplication underflows to zero must be refused as an unrepresentable effective rate; intentional zero a/u is valid.

Put pure validation/resolution methods in calendar.rs: `validate`, `values_at(period, class_rate)`, and `compile(class_rate)`. Do not duplicate the formula in UI and dispatch.

## 4. Commands, persistence, and invalidation

Add one `ScheduleEdit::SetCalendarCells { edits: Vec<CalendarCellEdit> }`. Each edit addresses a loader ID, `Default` or `Period(CalendarPeriod)`, a typed field enum, and `Option<f64>` in domain units. Reject Default + Rate. None for a Default percentage resets it to 1; None for a period setting removes that override. Reject duplicate cell addresses within one batch.

Route through `UiCommand::schedule(active_session, ...)` and `apply_schedule_edit`. Validate all targets/values on the cloned plan before executing one `Command::SetSchedulePlan`. No-op batches produce no history entry. Commands never use visible row numbers as durable addresses. Project switches invalidate queued commands using the existing runtime check.

Include every default and explicit override in the Loader Agents setup fingerprint (`schedule_fingerprints`). Run publication must therefore reject a job started before a calendar edit. Calendar edits retire the held result; rename edits still preserve it. Retain the existing document-revision-based report-cache invalidation for this increment; do not refactor reserve caching as part of calendar work.

Update Loader Agents setup validation to accept zero availability/utilisation as valid non-working capacity. Calendar input errors belong to that step if malformed persisted content reaches validation. Extend command console reporting with one concise batch-edit report, not one message per cell. Add translations for all new UI/error text.

## 5. Dispatcher integration — sparse piecewise rates

Compile each loader calendar once during run-input preparation into immutable sorted effective-rate changes. Keep `DispatchAgent.rate_tph` as the class/base rate for validation/context and add a compiled rate calendar containing `initial_rate_tph` plus `Vec<RateChange { at_h, rate_tph }>`. The dispatcher uses the compiled rate, not the base rate, for assignments. Update all constructors. Calendar compilation creates breakpoints at each explicitly overridden day's start and end, evaluates defaults/overrides there, merges adjacent equal effective rates, and folds a change at hour zero into the initial rate. It does not generate every day between sparse overrides. Use the shared period constant; remove the app-layer ownership of the 24-hour constant and update Run Period/new-bar creation to import it.

Dispatch changes in `src/model/schedule/dispatch.rs`:

1. Validate finite nonnegative compiled rates, finite strictly increasing positive change times, unique agents, and existing rate/window/tonnage rules. A zero compiled rate is valid; a nonpositive base rate is still invalid.
2. Resolve each agent's current rate from its sorted curve (binary search or a maintained cursor). Agents with zero effective rate receive no assignment. Preserve priority, sequence order, and shared depletion rules.
3. The next event is the earliest block depletion, bar-window edge, effective-rate change, or requested horizon. All loaders advance together to that event. Rate changes are start-inclusive; the old rate applies only to the interval ending there.
4. When no loader is working, still advance to a relevant future rate/window event. A zero-rate day must not terminate a run that has future capacity. Do not emit zero-tonnage execution segments for unavailable loaders; idle remains the complement of execution.
5. Expand the iteration cap to include compiled rate-change events: use checked arithmetic for `16 + 8 * (2 * bar_count + distinct_block_count + total_rate_change_count)`. Refuse overflow. Retain worker cancellation polling.
6. For Limited versus Stranded, require both positive remaining ground and some positive-duration intersection of that bar's future window with a positive effective-rate interval. Implement a pure `has_positive_rate_between(start_h, end_h)` query on the compiled calendar. An open window with permanent zero capacity is Stranded, not Limited. At a finite horizon, future capacity must exist after that horizon to report Limited.
7. Add the contributing loader's `rate_tph` to `ExecutionSegment`, populated from the assignment. Merge contiguous segments only when this rate AND the existing combined-rate/sharer/ground/bar fields match. Otherwise opposing rate changes in two loaders could preserve their combined rate and erase the change in each loader's contribution.
8. Preserve the worker input/currentness contract, Arc result ownership, and shared-ledger accounting. Animation continues to consume execution segments and their combined rate; it must not apply a/u factors again. Audit its segment consumers for the added field.

Example: class rate 1,000, default availability 90%, utilisation 80% gives 720 t/h. Day 2 rate override 1,200 gives 864 t/h; Day 3 blank rate returns to 720 t/h. Over a six-hour bar inside Day 1, production is 4,320 t if sufficient ground exists. Do not assume the bar receives all 17,280 daily tonnes in those six hours.

## 6. UI integration map

- `src/ui/state.rs`: add Calendar to `PlanningSubpage`, its label, and the Schedule subpage list. Add `is_schedule_calendar()`. Include Calendar in the planning-panel routing predicates while excluding it from viewport/geometry interaction predicates. Add `ScheduleCalendarView` for scroll, visible extent, collapsed loader IDs, selected rectangle, and one cell draft. Reset it on project replacement/close. Keep Gantt state independent.
- `src/ui/mod.rs`: route Calendar through the planning full-panel branch, omit explorer as for Gantt, invoke calendar draw function, and register its single region with `chrome::paint_regions`.
- New `src/ui/elements/schedule_calendar.rs`: owns grid layout, cell formatting/parsing, selection, paste, and typed commands. Register in elements/mod.rs. Use `chrome::region_frame`; nested frozen panes are not top-level regions.
- `src/ui/widgets/data_grid.rs`: reuse typography/colors/row-height conventions. Do not force its fixed property-table layout into a period matrix or rewrite all existing grids. Build frozen panes and virtualized cells locally; extract a helper only if directly shared.
- `src/ui/elements/viewport_bar.rs`, menus and `src/mac.rs`: inspect Schedule subpage selection paths; add Calendar wherever those paths explicitly list Schedule subpages. Use existing navigation commands/actions so leaving a sequence preview retains the existing cleanup behavior.
- `src/app/commands/schedule.rs`, `src/model/schedule/mod.rs`, and `src/app/commands/schedule_readiness.rs`: wire batch calendar edits, owned agent calendar data, and compilation into DispatchAgent respectively.
- `src/app/schedule_pipeline.rs`, `src/app/schedule_run.rs`: fingerprint calendars and use the model period constant.
- `i18n/en/incline_design.ftl`: add Calendar, field labels, help, empty action, and focused validation messages.
- Update AGENTS.md navigation pointers with calendar.rs and schedule_calendar.rs after implementation.

## 7. Required acceptance cases

Use a small focused temporary harness according to AGENTS.md; do not generate a test per getter or widget.

1. **Backward behavior and persistence:** existing metadata without calendar loads at 100%/100%; save/reopen retains defaults, explicit zero, blank inheritance, and an override equal to its default. Calendar-free dispatch agrees with the prior behavior.
2. **Rate math and exact boundary:** enough ground, 1,000 t/h × 0.9 × 0.8 removes 17,280 t in Day 1. Day 2 rate 1,200 removes 20,736 t; Day 3 reverts to the default. A run ending exactly at hour 24 uses Day 1 only. Partial six-hour window removes 4,320 t.
3. **No capacity and sparse future capacity:** default a=0, positive Day 100 override, open bar: Whole Run waits then works without enumerating all intervening days. A Day 1 run is Limited if Day 100 is reachable, but Stranded if its bar closes before then. Permanent zero capacity is Stranded.
4. **Shared ground and segmentation:** two loaders share a block; one goes to zero at a boundary, the other continues without duplicate depletion. Also swap their 100/200 rates to 200/100 across a boundary: retain distinct per-loader segments although combined rate stays 300.
5. **Atomic editing and lifetime:** one invalid pasted cell refuses the whole batch; successful paste/clear undoes once. Calendar edit during a job prevents stale publication. Rename retains output; class-rate edit updates blank rate periods and leaves explicit overrides intact.

Run formatting/native compilation and WASM compilation once after the relevant code settles. Use `web_time` for any timing. Manually inspect Calendar on desktop/browser for frozen panes, horizontal scrolling, collapsed loaders, blank-cell editing, paste/clear, undo, and page navigation. One end-to-end run verifies the calendar changes execution and animation. If a browser cannot be exercised, state the gap instead of treating a bundle build as interactive validation.

## 8. Execution order for the smaller model

Implement these as sequential reviewable commits, without another open-ended planning pass:

1. **Persist loader calendars:** calendar types/validation/defaulting, agent ownership, batch commands, hashing/memory, and setup fingerprint. Complete persistence/atomicity portions of acceptance cases 1 and 5.
2. **Apply period rates to dispatch:** compilation, event boundaries, zero-capacity outcomes, cap update, per-loader segment rate, shared period constant. Complete cases 2–4 and stale-publication checks.
3. **Add the Calendar grid:** navigation, frozen hierarchy/default/period panes, inherited blanks, edits, selection/paste, translations, and UI lifecycle. Complete manual acceptance.

Copy-ready instruction:

> Implement SCHEDULING_NEXT_MILESTONE.md in its stated order. It is the approved implementation contract; do not replace it with a proposal or ask another model to choose the semantics. Preserve unrelated working-tree changes. Start with commit-sized step 1 and continue through steps 2 and 3, running only the focused acceptance checks specified. Preserve the quiet UI, shared-ground ledger, undo, persistence, cancellation and browser support. Do not add interval rosters, arbitrary period lengths, mining dependencies, optimization, or extra calculated rows. Report any actual conflict with the current source before changing the contract; otherwise execute it. Do not commit unless the user has authorized committing.
