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
| Planning solids | `app/commands/solids.rs` (Setup/shared slabs), `app/commands/solids_view.rs` (View cache/jobs), `model/solid_reserves.rs` (volume-prorated totals) |
| Planning blast cuts | `app/commands/blasting.rs`, `model/arrangement.rs`, `BenchBlasts` in `model/mod.rs`; cut geometry stays in bench-owned planning storage |
| Excluding ground from mining | `MiningExclusions` on `Solid` (`model/mod.rs`), toggled from `ui/elements/solids_view.rs` and `dig_strips.rs`, applied in `app/commands/schedule_capture.rs` |
| Planning dig strips | `app/commands/dig_strips.rs`, `ui/elements/dig_strips.rs`; strips live in `BlastingPlan::dig_strips` keyed by flitch, and dig blocks are derived from strip plus blast cuts |
| Schedule setup, Calendar and Gantt | `model/schedule/mod.rs` (authored plan), `calendar.rs` (loader calendars), `result.rs` (shared calculated schedule, Calendar aggregates and cursor inventory) → `app/commands/schedule.rs` → `ui/elements/schedule_setup.rs`, `schedule_calendar.rs`, `schedule_gantt.rs` (with its time slider, shared with Animate as `EditorState::schedule_time_h`), `schedule_inspector.rs` (the Gantt's point-in-time panel), `schedule_charts.rs` (automatic destination charts on the Gantt's timeline; hourly series from `CalculatedSchedule::hourly_receipts`); run lifecycle in `app/schedule_run.rs`, publication in `app/schedule_publish.rs`. The plan lives on `Document`, not the scene composite |
| Destinations and routing | `model/schedule/destinations.rs` → `app/commands/schedule_capture.rs` (rules resolved against retained `MaterialCapture`) → `ui/elements/schedule_destinations.rs`; blended formulation and replay enforce routing |
| Truck fleet and trucking rules | `model/schedule/trucking.rs` → `app/commands/schedule.rs` → `ui/elements/schedule_trucking.rs`, fleet rows in `schedule_calendar.rs`. Capture/formulation enforce transport coefficients and fleet hours |
| Machine delays | `model/schedule/delays.rs` (types, delay lists, rosters; delay bars are `BarWork::Delay`) → `app/commands/schedule.rs` → `ui/elements/schedule_delays.rs`, Gantt palette and drawing in `schedule_gantt.rs`. Capture zeroes rates for lists/rosters and emits `TaskKind::Delay` for bars |
| Movement value | `model/schedule/cashflow.rs` → `app/commands/schedule.rs` → `ui/elements/schedule_cashflow.rs`; capture, conditional-grade formulation and replay value movements. Matching values add |
| Soft grade targets | `model/schedule/grade_targets.rs` (`CrusherGradeCalendar`: daily crusher bands, Default plus overrides) → `app/commands/schedule_capture.rs` (one target per crusher, grade and day) → crusher grade rows in `ui/elements/schedule_calendar.rs`; blended formulation, hourly dispatch, rolling carry and replay price receipts. See `docs/scheduling-grade-targets.md` |
| Stockpile modes and operating settings | `model/schedule/stockpile_operation.rs` (`StockpileOperation`: daily Build & reclaim / Build only / Reclaim only / Off, Default plus overrides; build-and-reclaim-at-once and rest hours, edited in Setup → Stockpiles via `ui/elements/schedule_destinations.rs`) → `app/commands/schedule_capture.rs` (`BlendPile::modes`) → Mode row in `ui/elements/schedule_calendar.rs`; hourly dispatch, formulation, replay and idle reasons enforce it. See `docs/scheduling-stockpile-modes.md` |
| Schedule optimisation backend | `model/schedule/optimisation/blended/` (contract, `Rows` formulation, independent replay, hourly dispatch LP (the first schedule) in `greedy.rs`, day-by-day windows as its fallback in `rolling.rs`, HiGHS relaxation bound in `relaxation.rs`) and `optimisation/scip/` (native nonlinear backend). Normal desktop SCIP; optional `blend-experiment` iterative HiGHS comparison. `optimisation/highs.rs` is the historical parcel baseline. Current integration: `docs/scheduling-scip-integration.md`; historical evidence: `docs/scip-blended-stockpile-experiment.md`, `docs/scheduling-optimisation-backend.md` |
| Schedule optimisation settings | `model/schedule/experiment.rs` (persisted horizon, limits, event capacity, tracked grade fields, pile representation) → `app/commands/schedule.rs` → `ui/elements/schedule_optimisation.rs`. Editable in native and WASM builds; solver dependencies native only |
| Schedule calculation from a real project | `app/commands/schedule_capture.rs` → `app/solver_process.rs` (child process; the app replays its answer again) → `app/scip_blend.rs` (solve/extract/replay, inside that process) → `app/schedule_publish.rs` → `model/schedule/result.rs`. `app/schedule_run.rs` owns currentness, jobs and normal run commands |
| Background work | `app/jobs.rs`; normal schedule lifecycle in `app/schedule_run.rs`, solver-process kill and orphan handling in `app/solver_process.rs`, SCIP solver-thread cancellation in `app/scip_blend.rs` |
| Asset loading | `app/commands/residency.rs` owns transitions; `model/asset_residency.rs`, `layer_residency.rs`, `history_storage.rs` move payloads to temporary backing in `asset_storage.rs` |
| Persistence | `model/formats/`, `model/atomic_file.rs` (native), `app/web_storage.rs` (browser) |
| Ultimate pit optimization | `crates/mineflow/src/` (`pseudoflow.rs`, `solver.rs`, `pattern.rs`, `precedence.rs`); see its README; check with `cargo check -p mineflow` |
| Translations | `src/i18n.rs`, `i18n/en/incline_design.ftl` |
| Web shell | `web/` (`index.html`, `web-initializer.js`, `_headers`), built by Trunk via `Trunk.toml` |

`res/` holds embedded assets, `docs/` documentation assets, `vendor/` patched dependencies. `examples/` holds an import-ready project with real data for manual validation; format fixtures live in `src/model/formats/fixtures/`.

## Commands

Toolchain is pinned **nightly**; `.cargo/config.toml` sets `build-std`, so a cold build compiles `std` too. Linux links with `clang` + `mold`.

```bash
cargo check                 # fast loop
cargo clippy
cargo run                   # desktop app
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

- **Translate user-facing text.** `tr!("message-id")`, `tr!("greeting", name = who)`, or the literal forms `tr!(literal = "Apply")` / `tr_format!` (see `src/i18n.rs`). New keys go in `i18n/en/incline_design.ftl`.
- `userspace_log!` / `userspace_warn!` / `userspace_error!` (`src/logging.rs`) surface messages in the in-app activity console; plain `log::` macros only reach the log file.
- `themed_icon!(ui, "name.svg")` / `unthemed_icon!("name.svg")` embed SVGs from `res/ui/` at compile time; `themed_icon!` picks between `icons_dark/` and `icons_light/`.
- **One corner radius for the whole window.** Panel regions (`chrome::REGION_RADIUS`), floating tiles, toolbar buttons, anything new — all use `widgets::toolbar::GROUP_CORNER_RADIUS`. Never pick a radius by eye.
- Naming: `snake_case` functions/modules, `PascalCase` types, `SCREAMING_SNAKE_CASE` constants.

## Git

- Short, action-oriented commit subjects; no mandatory prefix scheme. Keep PRs focused: problem, resulting behaviour, validation, platform limitations. Link relevant issues and include screenshots for visual changes.
