# Schedule stage 1 — loader setup and Gantt foundation

## Assignment and boundary

Implement this stage only. Deliver persistent loader classes and loader agents, and a real navigable Gantt canvas with an agent row for every configured loader. This is the foundation for the minimum viable scheduling test, not yet the complete test: sequence bars, 3D block assignment, calculated production and automatic dispatch follow in stage 2. Do not imply empty Gantt rows are a calculated schedule.

The user's target workflow is: define Liebherr 9400 at 3000 tph; assign EX7001 to that class; drag a sequence onto EX7001's Gantt; edit its ordered dig blocks in a 3D window; see execution duration from tonnes/rate; automatically fall back to lower-priority work and return to higher-priority work when available. Subsequent animation must consume the same calculated execution output.

Stage 1 is intentionally small enough to validate persistence, editing and time navigation before introducing geometry picking and dispatch. Preserve existing Solids behaviour and all unrelated working-tree changes.

## Current code findings

- `src/ui/state.rs`: `PlanningPage::Schedule` offers Setup and Animate, but no Gantt. Navigation is driven by `PlanningSubpage` and workspace predicates; inspect all exhaustive matches and layout predicates when adding Gantt.
- `src/ui/elements/planning_setup.rs`: Schedule uses placeholder Configuration / Site Data / Content navigation, disabled item creation and temporary egui storage for the schedule name. These are scaffolding, not durable domain data.
- `src/ui/mod.rs`: planning setup and Solids View have dedicated layout routing. Gantt needs deliberate routing; changing the subpage enum alone will not render it.
- `src/model/mod.rs`: `Document` owns serializable project planning data. `src/model/formats/omf.rs` explicitly writes and restores planning metadata; adding a serde field to Document alone is insufficient for OMF saves.
- `src/model/project.rs::scene_document` builds a composite across open projects and copies planning fields into it. Schedule must read and edit the active project's data explicitly, not whichever project was copied last.
- `src/app/commands/solids_view.rs::planning_snapshot()` supplies a readiness-gated `PlanningSnapshot` with runtime, generation and `DigBlockRecord`s. Records expose IDs, parentage and `MaterialState`, but do not supply mesh handles. This is the future scheduling boundary; do not recalculate reserves in Schedule.
- Reserve fields have IDs and aggregation types, but no explicit units or dedicated tonnage role. A future duration calculation must require an explicit summed field interpreted as tonnes; never choose a field by its display name.

## Stage 1 behaviour

### Setup → Site Data

Replace the Schedule placeholder Content selection with explicit **Loader Classes** and **Loader Agents** entries. Preserve Configuration, using it for a persistent schedule name. Leave unrelated Haulage and Solids navigation unchanged.

Loader Classes is an editable list with Name and Default dig rate (tph). Support add, rename, rate edit and delete. Example: `Liebherr 9400`, `3000`. Store no example data automatically. New rows can be transient drafts until valid; a valid committed class must have a trimmed nonempty name and a finite rate strictly above zero. Names must be unique within classes after trimming and case-insensitive comparison. Show actionable inline validation and keep the last valid committed value when input is invalid.

Loader Agents is an editable list with Name, Class and a read-only Effective dig rate (tph). Class selection refers to a class ID and displays its name. Example: `EX7001` assigned to `Liebherr 9400`. Enforce trimmed nonempty, case-insensitively unique agent names and an existing class. When no classes exist, explain that a class must be added first. Class renames and rate edits immediately appear for all linked agents. Stage 1 has no per-agent rate override.

Reject deletion of an in-use class with an explanation identifying its agents; users can reassign or delete agents first. Do not silently cascade deletions or orphan references. Agent deletion removes its Gantt row. Names and class IDs remain independent: renaming never changes identity.

Use normal application command dispatch, translation and undo conventions. One committed cell edit, addition or deletion is one undo action; avoid recording every keystroke. Cancelled drafts and no-op edits do not dirty the project.

### Schedule → Gantt

Add Gantt between Setup and Animate. Retain Animate's existing behaviour without inventing animation functionality.

Render a fixed left header with agent names and optionally their class, and a time ruler above the scrollable timeline. Agent order follows persisted list order; users need not reorder rows in this stage. The header and time grid remain aligned while scrolling vertically. Long names truncate with a tooltip. An empty project shows an actionable message directing users to Loader Classes and Loader Agents.

Use elapsed project time for this stage: origin is `Day 1, 00:00`, not the computer clock. No time zones, shifts or calendar dates yet. All time calculations use double-precision elapsed seconds; pixel coordinates are presentation only. Initialize to a seven-day visible span and provide Reset View. The ruler adapts through hour, day and week scales as users zoom. For example choose minor intervals from 1h, 3h, 6h, 12h, 1d, 2d, 7d and 14d, with labels only where they fit and optional coarser grouping above. Constrain the visible span to 1 hour–365 days and disallow panning before zero.

Provide visible zoom-in/out controls, Ctrl+wheel zoom anchored at the pointer, Shift+wheel horizontal time pan, and ordinary wheel vertical row scroll. Middle-drag can pan time. Scope input consumption to this canvas so keyboard shortcuts and other panes still work. Compute visible ticks/rows only; never iterate every hour from zero to the current scroll position. Keep row headers fixed during horizontal pan. Resize and DPI changes must preserve correct alignment and readable labels.

Show a clear empty-state message such as “Dig sequences will be added in the next stage.” Do not draw fabricated production bars, offer nonfunctional drop targets, or treat manual rectangles as scheduled work.

## Domain and ownership

Suggested new module: `src/model/schedule.rs`, exported from `model/mod.rs`.

- `SchedulePlan`: persistent name, ordered loader classes, ordered loader agents and ID allocation state.
- `LoaderClassId`, `LoaderAgentId`: distinct serializable ID types. Use the repository's ID conventions; IDs must survive save/reopen and never alias live entities. Undo/redo must restore the same references. Check duplicate IDs and exhausted allocation rather than wrapping.
- `LoaderClass`: ID, name, `default_dig_rate_tph: f64`.
- `LoaderAgent`: ID, name, `class_id: LoaderClassId`.

Attach one default-empty SchedulePlan to each project Document. Keep UI drafts, current table selection, zoom, scroll and hover in EditorState, scoped/reset on active-project change. Domain code must not depend on egui. Resolve effective rate through the class; do not persist a second rate on each agent.

Expose a read-only active-project schedule view to UI through the existing UI projection pattern. All writes target the captured active project and validated IDs, through new UiCommands and `src/app/commands/schedule.rs`. Reject stale commands if their project has changed. Scheduling configuration changes must not invalidate Solids products or trigger reserve computation.

Integrate with the existing history machinery after tracing a comparable document edit from dispatch through undo/redo. Ensure undo restoration also refreshes the UI projection. Avoid a second independent undo stack.

## Persistence and project lifecycle

Implement both the normal document serialization path and explicit OMF planning metadata read/write. Use a named, versioned schedule metadata payload consistent with nearby planning metadata. Missing data in older projects means an empty schedule. Unsupported versions and malformed rates, IDs or class references produce a clear load diagnostic; do not silently substitute a valid-looking rate. Follow existing import error handling so unrelated data is not discarded unnecessarily.

Trace save, Save As, native reopen, browser storage, active-project switching and document reconstruction. Preserve schedule data through those paths; opening project B must not change or display A's schedule. Keep schedule ownership on the active project rather than merging independent schedules across open projects. Save/reopen does not require a Solids run. Draft editor text and Gantt view state need not persist between application sessions.

## Suggested implementation order

1. Trace project ownership, commands, history and OMF serialization. Add the domain model, validation and persistent defaults.
2. Add commands and undo/redo, then serialization round-trip tests before building UI.
3. Replace Schedule's placeholder setup UI with the two working editors and persistent schedule name.
4. Add Gantt navigation and routing, then a pure time-to-screen / tick-selection helper and the agent timeline view. Use `chrome::region_frame` and region reporting for top-level panels.
5. Validate lifecycle, input behaviour and layout. Update task navigation pointers if modules move, and record exact checks and remaining gaps.

Keep schedule setup and Gantt rendering in focused modules such as `ui/elements/schedule_setup.rs` and `schedule_gantt.rs`; avoid growing the shared Solids setup module with another large implementation.

## Acceptance checks

- Create Liebherr 9400 at 3000 tph and EX7001 linked to it. EX7001 appears on Gantt immediately. Add a second agent of the same class; both show the effective rate.
- Change the class rate to 3500 and rename the class: both agents update without changing IDs. Reject empty/duplicate names, zero/negative/nonfinite rates and unknown class IDs in the command layer as well as UI.
- In-use class deletion is rejected; reassigning agents permits deletion. Undo/redo of add, rename, rate edit, reassignment and deletion restores valid data and row state.
- Save/reopen and Save As preserve names, rates, IDs, assignments and order. An old file with no schedule opens successfully. Switching between two projects with different schedules shows only the active one, including when both contain identical numeric IDs.
- Zoom remains anchored at the pointer except when constrained by the time origin; tick intervals grow/shrink sensibly, labels do not overlap, and headers/grid stay aligned under resize, DPI scaling and scrolling. Exercise narrow panes and several hundred agents.
- Opening Setup/Gantt or editing loader data never launches Solids jobs, changes its completion markers or creates unsaved changes merely by viewing.
- Run focused domain, serialization, undo and time-axis tests; native compilation, clippy, formatting and wasm compilation. Exercise desktop and browser interaction where available, reporting untested behaviour explicitly. Follow AGENTS.md's instruction to remove passing temporary test modules, but archive their exact source and rerun instructions in a repository validation document.

Deliver a short implementation report stating what works, commands/check results and limitations. Do not mark this stage complete with nonfunctional persistence, unimplemented commands or placeholder agent rows.

## Next stages and contracts to preserve

### Stage 2: complete the minimum viable scheduling test

Add named sequence definitions, ordered block selection in a 3D editor, drag/drop assignments, tonnage-based durations and a deterministic dispatch evaluator. Reuse the Solids preview renderer with editor-specific selection state; inspect its render/pick boundary before deciding the extraction. Opening the sequence editor must not run geometry calculations. A stale/unready snapshot blocks calculation with a reason and a route to run Solids.

Require explicit selection of a Sum reserve field as tonnes, with a visible statement of the assumed units. Use complete measured totals from one current snapshot. Missing, partial, unavailable or capacity-only material must not silently become zero tonnes. For the MVP assume continuous operation at class tph, with no travel, haulage, availability factors or shifts. Example: 6000 tonnes at 3000 tph requires two operating hours.

Keep sequence definitions, assignments and calculated execution segments distinct. One assignment can yield several segments when interrupted; one long painted bar cannot represent this correctly. Each execution segment should eventually identify agent, assignment, block, start/end and processed tonnes so animation reads the same output as Gantt.

Proposed interpretation of “flow down” and “jump higher”: each agent has its own stack of priority lanes; higher lanes win, and the next ready lower lane supplies fallback work. Do not transfer work automatically to another agent's row. Availability is an explicit earliest-start time for the MVP, not an inferred mining-access rule. At availability of higher-priority work, interrupt lower-priority work, retain its remaining tonnes, and resume it later. Use deterministic tie-breaking. Confirm this interpretation before implementing dispatch; stage 1 does not depend on it.

An acceptance scenario for that proposed policy: low-priority A contains 6000 tonnes and starts at hour 0; high-priority B contains 3000 tonnes and becomes available at hour 1; rate is 3000 tph. Execute A at 0–1h, B at 1–2h, resume A at 2–3h. Without B, A finishes at 2h. Persist sequence membership order and reject duplicate allocation of the same material across active assignments for this MVP.

Before persisting block assignments, resolve the current session-bound identity/geometry-handle limitations. A reopened project or topology-changing rerun must not bind an old ID to unrelated material. Preserve unresolved references with diagnostics until explicitly reconciled; never silently rematch by a block's display name or use `replaces` as proof of identical tonnage. Carry project/runtime and generation provenance through preview picks and calculated outputs. Stage 1 adds no block references, so it need not solve that issue prematurely.

### Stage 3: animation and production realism

Add a playback slider driven by execution segments, then calendars, downtime, travel/haulage constraints and mining precedence as separate extensions. Cross-solid overlap and safe physical access remain separate from priority-lane availability. The MVP must not imply those constraints have been modelled.
