# AGENTS.md

Incline is a Rust 2024 mine design app — one binary for Windows/macOS/Linux/WebAssembly on `winit` + `wgpu` + `egui`.

## Working economy

Search the relevant subtree, not the repo: `rg -n 'draw_screen_cross' src/rendering`. Exclude `target/` and `dist/`. Read matching functions and their callers, not whole files. Don't re-read a file you just edited or re-run a check that passed until code changed. Run only the checks the change warrants — a UI tweak needs no web build. Keep the table below current when moving code.

### Where things live

Paths relative to `src/`, except `crates/` paths, which are relative to the repository root.

| Task | Start here |
| --- | --- |
| Add a UI action | `ui/state.rs` (`UiCommand`, `console_report_spec`) → UI call site → `app/commands/mod.rs` (match arm, `requires_project`) |
| Change state | `app/mod.rs` (durable), `ui/state.rs` (`EditorState`, transient), `model/project.rs` (projects) |
| Fix rendering | `rendering/graphics/init.rs` (pipelines), `passes.rs` (draw passes), `rendering/scene/` (geometry + caches), `rendering/shaders/` (WGSL) |
| Add or change a side panel | `ui/widgets/island.rs` (width, surface, region, seam), then the panel's own `ui/elements/*.rs` |
| Planning solids | `app/commands/solids.rs` (Setup/shared slabs), `app/planning_pipeline.rs` (Setup steps: Dig Strips checks the strips in plan, Reserving cuts and measures the dig blocks; Run Step reruns only the selected step after any stale ones before it), `app/commands/solids_view.rs` (View cache/jobs), `model/solid_reserves.rs` (volume-prorated totals); Schedule Setup's Solids step (`app/schedule_pipeline.rs::evaluate_schedule_solids`) runs the Solids pipeline and checks each block's tonnes |
| Planning blast cuts | `app/commands/blasting.rs`, `model/arrangement.rs`, `BenchBlasts` in `model/mod.rs`; cut geometry stays in bench-owned planning storage |
| Excluding ground from mining | `MiningExclusions` on `Solid` (`model/mod.rs`, including `min_blast_area`, set on the Blasts panel in `ui/elements/blasting.rs`), toggled from `ui/elements/solids_view.rs` and `dig_strips.rs`, applied in `app/commands/schedule_capture.rs` |
| Planning dig strips | `app/commands/dig_strips.rs`, `ui/elements/dig_strips.rs`; strips live in `BlastingPlan::dig_strips` keyed by flitch, and dig blocks are derived from strip plus blast cuts |
| Schedule setup, Calendar and Gantt | `model/schedule/mod.rs` (authored plan), `calendar.rs` (loader calendars), `result.rs` (shared calculated schedule, Calendar aggregates and cursor inventory) → `app/commands/schedule.rs` → `ui/elements/schedule_setup.rs`, `schedule_calendar.rs`, `schedule_gantt.rs` (with its time slider, shared with Animate as `EditorState::schedule_time_h`), `schedule_inspector.rs` (the Gantt's point-in-time panel), `schedule_charts.rs` (automatic destination charts on the Gantt's timeline; hourly series from `CalculatedSchedule::hourly_receipts`), `schedule_report.rs` (the Calendar's report export and Total column; see `docs/scheduling-inspection.md`); run lifecycle in `app/schedule_run.rs`, publication in `app/schedule_publish.rs`. The plan lives on `Document`, not the scene composite |
| Drill and blast activities | `model/schedule/drill_blast.rs` (machine types, patterns, starting stages and settings) → `app/commands/schedule_capture.rs` (blast quantities and clearance) → `model/schedule/optimisation/blended/drill_blast.rs` (concurrent authored machine work); dispatch in `greedy.rs`, fixed release and clearance times in `formulation.rs`, independent checks in `replay.rs`; Setup in `ui/elements/schedule_drill_blast.rs`, work in Gantt/Inspector/Calendar/report and Animate strokes in `rendering/scene/build.rs`. See `docs/scheduling-drill-blast.md` |
| Destinations and routing | `model/schedule/destinations.rs` → `app/commands/schedule_capture.rs` (rules resolved against retained `MaterialCapture`) → `ui/elements/schedule_destinations.rs`; blended formulation and replay enforce routing |
| Haul roads and cycle times | `model/haulage/` (network, block links, spatial access index, directional routing) → `app/commands/haulage.rs` → `ui/elements/haulage.rs` (Layout panel, and the Setup page's Road network step, laid out by `planning_setup.rs::draw_haulage_details`); Setup's pipeline in `app/haulage_pipeline.rs`, on the step runner in `app/step_pipeline.rs` that the Schedule pipeline shares, and waited on by Schedule's Haulage step (after Solids), whose page lists what reaches the roads from `app/commands/haulage.rs::haul_connections`; Animate's flows are routed in `app/commands/haulage.rs::sync_animation_routes`, projected in `rendering/graphics/projections.rs`, drawn as depth-tested strokes by `rendering/scene/build.rs::rebuild_flow_scene`, with the hover read-out in `ui/mod.rs::draw_haul_flows`; candidate cycles in `app/commands/schedule_capture.rs`, shared delivery breakdowns in `model/schedule/result.rs`. See `docs/scheduling-haulage.md` |
| Truck fleet and trucking rules | `model/schedule/trucking.rs` → `app/commands/schedule.rs` → `ui/elements/schedule_trucking.rs` (truck classes and their Grade Speeds on Haulage → Setup, checked by the Haulage pipeline; `EditorState::open_schedule_step(ScheduleStep::Haulage)` opens them), fleet rows in `schedule_calendar.rs`. Grade speed bands, payload and dump time set cycle costs; loader classes supply spot time. Capture/formulation enforce transport coefficients and fleet hours |
| Drill and blast scheduling | `model/schedule/drill_blast.rs` (machine kinds, patterns, starting stages and recurring/one-off blast windows) → `app/commands/schedule_capture.rs` → `model/schedule/optimisation/blended/drill_blast.rs` (activity chain); Setup in `ui/elements/schedule_drill_blast.rs`, Gantt machine bars and Blasting row in `schedule_gantt.rs`, interactive blast sequence and window editors in `ui/dialogs/schedule.rs`, with preview picking in `app/commands/solids_view.rs`. See `docs/scheduling-drill-blast.md` |
| Schedule dates and period colours | `model/schedule/periods.rs` (`SchedulePeriods`: Day 1's date and per-day colours, display only; dd/mm/yyyy parsing) → `app/commands/schedule.rs` → `ui/elements/schedule_periods.rs` (the Periods step, and `instant_label`/`day_label`, which every schedule page writes times through; the start date is set per frame by `set_clock`), date picker in `ui/widgets/date_picker.rs`; colours drawn in `schedule_gantt.rs` and `schedule_animation.rs` |
| Machine delays | `model/schedule/delays.rs` (types, delay lists, rosters; delay bars are `BarWork::Delay`) → `app/commands/schedule.rs` → `ui/elements/schedule_delays.rs`, Gantt palette and drawing in `schedule_gantt.rs`. Capture zeroes rates for lists/rosters and emits `TaskKind::Delay` for bars |
| Movement value | `model/schedule/cashflow.rs` → `app/commands/schedule.rs` → `ui/elements/schedule_cashflow.rs`; capture, conditional-grade formulation and replay value movements. Matching values add |
| Soft grade targets | `model/schedule/grade_targets.rs` (`CrusherGradeCalendar`: daily crusher bands, Default plus overrides) → `app/commands/schedule_capture.rs` (one target per crusher, grade and day) → crusher grade rows in `ui/elements/schedule_calendar.rs`; blended formulation, hourly dispatch, rolling carry and replay price receipts. See `docs/scheduling-grade-targets.md` |
| Stockpile modes and operating settings | `model/schedule/stockpile_operation.rs` (`StockpileOperation`: daily Build & reclaim / Build only / Reclaim only / Off, Default plus overrides; build-and-reclaim-at-once and rest hours, edited in Setup → Stockpiles via `ui/elements/schedule_destinations.rs`) → `app/commands/schedule_capture.rs` (`BlendPile::modes`) → Mode row in `ui/elements/schedule_calendar.rs`; hourly dispatch, formulation, replay and idle reasons enforce it. See `docs/scheduling-stockpile-modes.md` |
| Schedule optimisation backend | `model/schedule/optimisation/blended/` (contract, `Rows` formulation, independent replay, hourly dispatch LP (the first schedule) in `greedy.rs`, solved by microlp or, with the `highs` feature, HiGHS in `lp.rs`, day-by-day windows as its fallback in `rolling.rs`, HiGHS relaxation bound in `relaxation.rs`), the loader utilisation incentive every one of them prices the same way in `utilisation.rs`, plan targets the dispatch can follow in `plan.rs`, and `optimisation/scip/` (native nonlinear backend; Improve's search over daily plan windows and polished days in `anytime.rs`, its daily plan in `plan.rs`). SCIP (Improve) only with the `scip` feature. See `docs/scheduling-scip-integration.md` |
| Schedule optimisation settings | `model/schedule/experiment.rs` (persisted horizon, limits, event capacity, tracked grade fields, pile representation) → `app/commands/schedule.rs` → `ui/elements/schedule_optimisation.rs` (horizon and grades on Configuration's General table, the rest under Advanced; Improve's limits only with `scip`). The Gantt's bar height sits behind its toolbar gear (`schedule_gantt.rs::draw_settings`); money uses the language's `common-currency-symbol` |
| Schedule calculation from a real project | `app/commands/schedule_capture.rs` → `app/schedule_solve.rs` (validation, hourly dispatch, replay; every build, the browser included) → `app/schedule_publish.rs` → `model/schedule/result.rs`. With the `scip` feature the solve runs in `app/solver_process.rs` (child process; the app replays its answer again) and Improve continues in `app/scip_blend.rs`: the search (`scip/anytime.rs`, polishing in `polish_window`), then the whole-horizon solve where its model fits. `app/schedule_run.rs` owns currentness, jobs and normal run commands. Improve's progress card is `ui/elements/improve_progress.rs`: stages and values (`model/schedule/optimisation/progress.rs`) come from `ScheduleActivity` over the solver process's `Progress` messages into `schedule_run.rs::ImproveFeed`; Stop and keep best writes `finish` to the process's stdin, which raises `ScheduleActivity::finish` |
| Background work | `app/jobs.rs`; normal schedule lifecycle in `app/schedule_run.rs`, solver-process kill and orphan handling in `app/solver_process.rs`, SCIP solver-thread cancellation in `app/scip_blend.rs` |
| Asset loading | `app/commands/residency.rs` owns transitions; `model/asset_residency.rs`, `layer_residency.rs`, `history_storage.rs` move payloads to temporary backing in `asset_storage.rs` |
| Persistence | `model/formats/`, `model/atomic_file.rs` (native), `app/web_storage.rs` (browser) |
| Ultimate pit optimization | `crates/mineflow/src/` (`pseudoflow.rs`, `solver.rs`, `pattern.rs`, `precedence.rs`); see its README; check with `cargo check -p mineflow` |
| Reusable UI widgets | `ui/widgets/` (`menu.rs` buttons and fields, `collapsible_section.rs`, `data_table.rs` (read-only tables), `data_grid.rs` (selectable list panes and key/value property tables), `island.rs` (side panels), `toolbar.rs`) |
| Drill & Blast | `model/drill_hole.rs` (patterns, ties, firing times), `model/blast.rs` (charge library, `charge_hole`, `BlastAnalysis`), `app/tie_in.rs`, `app/blast.rs`, `ui/elements/products.rs` (palette, rules, shot summary), `ui/elements/blast.rs` (contours, relief legend, hole card, timeline) |
| Translations | `src/i18n.rs` (`tr!` macro, loader), `i18n/en/incline_design.ftl` |
| Web shell | `web/` (`index.html`, `web-initializer.js`, `_headers`), built by Trunk via `Trunk.toml` |

`res/` holds embedded assets, `docs/` documentation assets, `vendor/` patched dependencies. `examples/` holds an import-ready project with real data for manual validation; format fixtures live in `src/model/formats/fixtures/`.

## Commands

Toolchain is pinned **nightly**; `.cargo/config.toml` sets `build-std`, so a cold build compiles `std` too. Linux links with `clang` + `mold`.

```bash
cargo check                 # fast loop
cargo clippy
cargo run                   # desktop app (pure Rust; no Improve)
cargo run --features scip-source    # with SCIP built from source, for Improve (`highs`, `scip-system`: see Cargo.toml)
cargo build --release
cargo fmt
cargo test
trunk serve                 # web build on 127.0.0.1:8080 (COOP/COEP headers in Trunk.toml)
trunk build --release       # into dist/
```

`rustfmt.toml` sets `max_width = 180` and `group_imports = "StdExternalCrate"` — long single-line signatures are deliberate, don't hand-wrap them.

No standing test suite. For substantive logic changes add a focused `#[test]` with a behaviour-based name in an adjacent `#[cfg(test)]` module, run it with `cargo test <name>`, then **delete it** before committing. Validate rendering and interaction changes against `examples/` (relevant camera angles, desktop and web), and report validation gaps.

## Architecture

### The command round-trip

Data flows one way; the UI never mutates application state.

```
winit event → App::window_event (app/mod.rs) → app/events.rs (input, redraw)
  → Gui::render (ui/mod.rs) → draw_ui → UiFrameOutput { commands, geometry_dirty }
  → App::handle_ui_commands (app/commands/mod.rs) → per-domain impl in app/commands/*.rs
```

`draw_ui` gets an immutable `UiProjectView`, `&mut EditorState`, and `&Document`; anything else it wants changed it requests by pushing a `UiCommand`.

### Menus exist in three parallel places

`ui/elements/main_menu.rs` (menu bar), egui context menus (`ui/elements/explorer.rs` for tree entries, `ui/dialogs/editing.rs::draw_right_click_context` for the canvas), and `src/mac.rs` (native `NSMenu`: its own `MacMenuAction` enum, a mapping back to `UiCommand` in `app/mod.rs`, and an enable/check sync pass). Adding or removing an item means editing the egui site *and* `mac.rs`.

### State ownership

- **`App`** (`app/mod.rs`) — durable: `ProjectStore` workspace, open entities, undo `History`. Many fields `#[cfg(target_arch = "wasm32")]`-gated.
- **`EditorState`** (`ui/state.rs`) — transient: active tool, `selected_handles`, hidden/frozen sets, dialog flags. Small changes inline; larger transitions via `EditorState::apply_action(EditorAction)`, which returns whether geometry must rebuild — propagate it.
- **`UiProjectView`** — derived from `App` each frame, cached behind an allocation-free fingerprint (`ui_project_view_cache`).

Selection is uniform via `SceneEntityId` (`Object` / `Triangulation` / `BlockModel` / `DrillHole` / `PointCloud`); `rendering/query.rs` and `rendering/pick.rs` return one, so viewport features match on the variant rather than consulting per-kind lists.

Tools that consume one kind of thing are **select first, then act**: `app/commands/scene_selection.rs` counts the selection per kind into `EditorState::selection_counts` each frame, the menu entry enables itself from that count (in `ui/elements/main_menu.rs` *and* `mac.rs`), and the open command snapshots the ids so the dialog reports its input instead of offering a picker. While one is open the viewport stops taking selection (`EditorState::selection_locked_by_tool`). Dialogs taking two surfaces still pick theirs inside the dialog: both inputs are the same kind, so a selection cannot say which is which.

### Invalidation and caching

`App::invalidate_geometry()` is called from ~90 sites, mostly editor-state changes with untouched documents. It stays cheap because the composite `scene_document`, the snap index, and the GPU caches in `rendering/scene/*_cache.rs` rebuild only when `ProjectStore::composite_key()` changes. **Never introduce an unconditional per-frame rebuild of scene or cache data.**

### Rendering

Hand-written wgpu renderer (`rendering/graphics/`, WGSL in `rendering/shaders/`); egui composites on top in the same encoder with `LoadOp::Load`. Vertex positions are chunk-origin-relative so `f32` stays precise far from the world origin — keep domain coordinates in `f64` and rebase before GPU conversion.

The scene covers the whole window; the panels in `ui/chrome.rs` are painted over it, not clipped. A new **top-level** panel must do both halves: take `chrome::region_frame`, *and* hand its fill rect to the single `chrome::paint_regions` call at the end of `draw_ui` as a `chrome::Region`. Nested panels do neither. The explorer column is two regions — tree and properties — with a draggable seam.

### Persistence, jobs, wasm

`ProjectStore` holds `OpenProject`s. Each item's `ProjectItemState` (`model/project.rs`) keeps two counters: `revision` invalidates caches, while `epoch` vs `saved_epoch` drives the `*` dirty markers. Undo restores the content epoch while advancing revision — preserve that distinction. Native format is **OMF**; DXF, CSV, LAS/LAZ, GeoTIFF are import/export only. Writes go through `model/atomic_file.rs`.

`app/jobs.rs` is one generic job queue: compute closure plus owned inputs run on a worker pool, the apply closure runs App-side on the UI thread, and `JobKey` dependencies cancel stale jobs when their source changes; poll cancellation in long loops. Use it instead of another bespoke `pending_*` vec and poll function.

wasm is `panic = "abort"` (the job queue's panic recovery does *not* apply), needs COOP/COEP for shared memory and the `wasm-bindgen-rayon` pool, and persists to IndexedDB via `app/web_storage.rs`. Anything touching files or threads needs a native path *and* a wasm path. The COOP/COEP headers live in `Trunk.toml` (dev server) and `web/_headers` (deployments); keep both.

## Conventions

- **Translate user-facing text, and give every string a hand-written id in `i18n/en/incline_design.ftl`.** Use `tr!("message-id")` or `tr!("greeting", name = value)` (`{ $name }` in the catalog); pass arguments as strings (`.to_string()`). `cargo check` enforces it: an unknown id or a missing argument fails the build. Ids are kebab-case with an area prefix (`menu-`, `tri-`, `common-`). Add the English text first; other languages fall back to it until translated. Fluent trims values, so add any padding spaces at the call site.
- **Build UI from `ui/widgets/`, not bare egui.** Reach for the house widget before the stock one: `menu::MenuButton` over `egui::Button`, `MenuField*` / `MenuFieldCombo` for labelled controls, `properties::read_only_row` for reported values, `CollapsibleSection` / `menu_section` for grouping, `DataTable` for tabular data. When a look or behaviour is needed a second time, or a hand-painted block would help other panels, make it a widget in `ui/widgets/` instead of copying it.
- `userspace_log!` / `userspace_warn!` / `userspace_error!` (`src/logging.rs`) surface messages in the in-app activity console; plain `log::` macros only reach the log file.
- `themed_icon!(ui, "name.svg")` / `unthemed_icon!("name.svg")` embed SVGs from `res/ui/` at compile time; `themed_icon!` picks between `icons_dark/` and `icons_light/`.
- **One corner radius for the whole window.** Panel regions (`chrome::REGION_RADIUS`), floating tiles, toolbar buttons, anything new — all use `widgets::toolbar::GROUP_CORNER_RADIUS`. Never pick a radius by eye.
- Naming: `snake_case` functions/modules, `PascalCase` types, `SCREAMING_SNAKE_CASE` constants.

## Git

- **Never open a PR against `main`.** A push to `main` triggers `.github/workflows/release.yml`, so a merge there can cut a release. PRs go to `staging` or to the integration branch the current line of work uses.
- Short, action-oriented commit subjects; no mandatory prefix scheme. Keep PRs focused: problem, resulting behaviour, validation, platform limitations. Link relevant issues and include screenshots for visual changes.
