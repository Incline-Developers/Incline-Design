# Schedule stage 1 — validation record

What was checked for the loader-fleet setup and the Gantt foundation, and the
exact temporary tests that checked it. The tests are archived here rather than
kept in the tree, per `CLAUDE.md`: *"No standing test suite. For substantive
logic changes add a focused `#[test]` in an adjacent `#[cfg(test)]` module,
run it, then delete it before committing."*

## Result

33 tests, all passing, across five temporary modules:

| Module | Tests | Covers |
| --- | --- | --- |
| `src/model/schedule.rs` | 9 | Validation, uniqueness, id allocation and its high-water mark, in-use deletion, load checks |
| `src/model/formats/omf.rs` | 6 | Real OMF save/reopen round-trip, old files, retired ids, load diagnostics |
| `src/app/commands/schedule.rs` | 10 | Command dispatch, session-token rejection, undo/redo, dirty tracking, project scoping |
| `src/ui/state.rs` | 7 | Time axis: zoom anchoring, limits, tick ladder, visible ticks |
| `src/ui/elements/schedule_setup.rs` | 1 | Headless draft initialization and rate fidelity |

The last module and the OMF retired-id case came from the stage 1 review; the
three command-layer cases named below came from its stage 2 prerequisites.

## Commands run

```
cargo fmt
cargo check --offline
cargo clippy --offline
cargo test --offline
cargo check --offline --target wasm32-unknown-unknown
cargo run --offline          # started, held 20 s, exited on timeout with no output
```

`cargo clippy` and both `cargo check` targets are clean apart from the
pre-existing `unused dependency 'mineflow'` (native) and `unused dependency
'toml'` (wasm) manifest warnings, which predate this work. `cargo test` also
reports dead-code warnings from the `#[cfg(test)]` helpers committed in
`src/app/commands/solids_view.rs`, which are unrelated to the schedule.

## Not covered

- **No GUI interaction was exercised.** The application was started and ran
  without panicking, but nothing navigated to Schedule → Setup or Schedule →
  Gantt, so the panes, the two editors, the dialogs, the ruler and every input
  gesture (Ctrl+wheel zoom, Shift+wheel pan, wheel row scroll, middle-drag)
  are **unverified on screen**. So are the layout requirements that only a
  running window can settle: narrow panes, several hundred agents, DPI
  scaling, resize behaviour and label overlap. The headless
  `schedule_setup.rs` case draws one property table into an off-screen
  context; it checks what the draft is seeded with, not keyboard focus, so
  the focus-loss commit rule itself is still unverified interactively.
- **No browser run.** `wasm32-unknown-unknown` compiles; `trunk serve` was not
  run and IndexedDB persistence of a schedule was not exercised. The browser
  stores whole OMF bytes (`BrowserProjectRecord::omf_bytes`), which is the
  path the round-trip test covers, but that inference is not a test.
- **Save As and native reopen from disk** were not run through the file
  dialogs. The round-trip test drives `omf::to_bytes` → `omf::from_bytes`,
  which is what both paths use, but it does not touch `atomic_file`.
- **Two projects open at once** cannot be tested, because this build holds one
  project at a time: `ProjectStore::add_and_activate` clears the list. The
  scoping test uses that to its advantage - both projects number their first
  loader class 0, so their schedule ids genuinely collide and only the active
  project's fleet may be shown. Runtime ids no longer collide: they are handed
  out by the process (`model/project.rs::next_runtime_namespace`), which is
  what makes the stale-edit rejection possible.
- **Name comparison is ASCII case-insensitive** (`str::eq_ignore_ascii_case`).
  Two names differing only outside ASCII - `Löffel` and `LÖFFEL` - count as
  different names in both the UI and the domain, consistently, but that is a
  documented limit rather than a tested Unicode rule.

## Restoring the tests

Each block below goes back at the end of the file it names. One extra piece of
scaffolding is needed by the `src/app/commands/schedule.rs` block - add this to
`impl App` in `src/app/mod.rs`, immediately above `fn project_view`:

```rust
#[cfg(test)]
pub(crate) fn project_view_for_test(&self) -> Arc<UiProjectView> {
    self.project_view()
}
```

Then `cargo test --offline`.

### `src/model/schedule.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn fleet() -> (SchedulePlan, LoaderClassId, LoaderAgentId) {
        let mut plan = SchedulePlan::default();
        let class = plan.add_class("Liebherr 9400", 3000.0).expect("valid class");
        let agent = plan.add_agent("EX7001", class).expect("valid agent");
        (plan, class, agent)
    }

    #[test]
    fn an_agent_takes_its_rate_from_its_class_and_keeps_it_through_edits() {
        let (mut plan, class, agent) = fleet();
        let second = plan.add_agent("EX7002", class).expect("valid agent");
        assert_eq!(plan.effective_rate_tph(agent), Some(3000.0));
        assert_eq!(plan.effective_rate_tph(second), Some(3000.0));

        plan.set_class_rate(class, 3500.0).expect("a positive rate");
        plan.rename_class(class, "Liebherr 9400 XE").expect("a free name");
        assert_eq!(plan.effective_rate_tph(agent), Some(3500.0));
        assert_eq!(plan.effective_rate_tph(second), Some(3500.0));
        // Renaming is not re-identifying: both machines still name the same
        // class, and the class still has the id it was allocated.
        assert_eq!(plan.agent(agent).map(|agent| agent.class_id), Some(class));
        assert_eq!(plan.class(class).map(|class| class.name.as_str()), Some("Liebherr 9400 XE"));
    }

    #[test]
    fn names_are_trimmed_unique_and_required() {
        let (mut plan, class, agent) = fleet();
        assert_eq!(plan.add_class("   ", 100.0), Err(ScheduleError::EmptyName));
        assert_eq!(plan.rename_agent(agent, ""), Err(ScheduleError::EmptyName));
        assert_eq!(plan.add_class(" liebherr 9400 ", 100.0), Err(ScheduleError::DuplicateName("liebherr 9400".to_owned())));
        assert_eq!(plan.add_agent("ex7001", class), Err(ScheduleError::DuplicateName("ex7001".to_owned())));
        // A class and an agent may share a name: they are different things.
        assert!(plan.add_class("EX7001", 100.0).is_ok());
        // Renaming to your own name, differently cased, is not a collision.
        assert!(plan.rename_agent(agent, " ex7001 ").is_ok());
        assert_eq!(plan.agent(agent).map(|agent| agent.name.as_str()), Some("ex7001"));
    }

    #[test]
    fn rates_must_be_finite_and_above_zero() {
        let (mut plan, class, _) = fleet();
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(plan.set_class_rate(class, bad), Err(ScheduleError::InvalidRate), "{bad}");
            assert_eq!(plan.add_class("Другой", bad), Err(ScheduleError::InvalidRate), "{bad}");
        }
        // The last valid value stands after every refusal.
        assert_eq!(plan.class(class).map(|class| class.default_dig_rate_tph), Some(3000.0));
    }

    #[test]
    fn an_in_use_class_cannot_be_deleted_until_its_agents_move_or_go() {
        let (mut plan, class, agent) = fleet();
        let spare = plan.add_class("Cat 6060", 2200.0).expect("valid class");
        assert_eq!(plan.remove_class(class), Err(ScheduleError::ClassInUse(vec!["EX7001".to_owned()])));
        assert!(plan.class(class).is_some(), "a refused deletion changes nothing");

        plan.set_agent_class(agent, spare).expect("an existing class");
        assert!(plan.remove_class(class).is_ok());
        assert_eq!(plan.effective_rate_tph(agent), Some(2200.0));

        assert_eq!(plan.remove_class(spare), Err(ScheduleError::ClassInUse(vec!["EX7001".to_owned()])));
        plan.remove_agent(agent).expect("an existing agent");
        assert!(plan.remove_class(spare).is_ok());
        assert!(plan.agents().is_empty() && plan.classes().is_empty());
    }

    #[test]
    fn unknown_ids_are_refused_rather_than_acted_on() {
        let (mut plan, class, agent) = fleet();
        let ghost_class = LoaderClassId(999);
        let ghost_agent = LoaderAgentId(999);
        assert_eq!(plan.rename_class(ghost_class, "x"), Err(ScheduleError::UnknownClass));
        assert_eq!(plan.set_class_rate(ghost_class, 1.0), Err(ScheduleError::UnknownClass));
        assert_eq!(plan.remove_class(ghost_class), Err(ScheduleError::UnknownClass));
        assert_eq!(plan.rename_agent(ghost_agent, "x"), Err(ScheduleError::UnknownAgent));
        assert_eq!(plan.remove_agent(ghost_agent), Err(ScheduleError::UnknownAgent));
        assert_eq!(plan.set_agent_class(ghost_agent, class), Err(ScheduleError::UnknownAgent));
        assert_eq!(plan.set_agent_class(agent, ghost_class), Err(ScheduleError::UnknownClass));
        assert_eq!(plan.add_agent("EX7003", ghost_class), Err(ScheduleError::UnknownClass));
    }

    #[test]
    fn ids_are_never_reused_by_a_later_addition() {
        let mut plan = SchedulePlan::default();
        let first = plan.add_class("A", 1.0).expect("valid");
        plan.remove_class(first).expect("unused");
        let second = plan.add_class("B", 1.0).expect("valid");
        assert_ne!(first, second, "a deleted id must not come back on the next addition");
    }

    #[test]
    fn an_allocator_only_ever_rises_and_is_not_part_of_the_content_fingerprint() {
        let (plan, class, agent) = fleet();

        // What an undo hands back: the plan as it was before those additions,
        // carrying counters that have since moved on.
        let mut restored = SchedulePlan::default();
        restored.raise_allocator_to(&plan);
        assert_eq!(restored.add_class("Cat 6060", 2200.0).expect("valid"), LoaderClassId(class.0 + 1));
        let spare = restored.classes()[0].id;
        assert_eq!(restored.add_agent("EX7002", spare).expect("valid"), LoaderAgentId(agent.0 + 1));

        // Lowering is not possible, in either direction of an undo.
        let mut high = plan.clone();
        high.raise_allocator_to(&SchedulePlan::default());
        assert_eq!(high, plan);

        // And a counter left standing is not a content change: a project
        // whose fleet is back exactly as saved is not unsaved work.
        let fingerprint = |plan: &SchedulePlan| {
            let mut hasher = std::hash::DefaultHasher::new();
            plan.hash_content(&mut hasher);
            std::hash::Hasher::finish(&hasher)
        };
        let mut advanced = SchedulePlan::default();
        advanced.raise_allocator_to(&plan);
        assert_eq!(fingerprint(&advanced), fingerprint(&SchedulePlan::default()));
        assert_ne!(fingerprint(&plan), fingerprint(&advanced));
    }

    #[test]
    fn a_loaded_plan_is_checked_before_it_is_trusted() {
        let (plan, class, _) = fleet();

        let mut good = plan.clone();
        assert!(good.validate_loaded().is_ok());
        assert_eq!(good, plan, "a valid plan is not rewritten by validation");

        // A class whose rate did not survive the file.
        let mut bad_rate = plan.clone();
        bad_rate.classes[0].default_dig_rate_tph = 0.0;
        assert_eq!(bad_rate.validate_loaded(), Err(ScheduleError::InvalidRate));

        // An agent whose class is not in the file.
        let mut dangling = plan.clone();
        dangling.classes.clear();
        assert_eq!(dangling.validate_loaded(), Err(ScheduleError::UnknownClass));

        // Two classes sharing one identity.
        let mut twins = plan.clone();
        let mut copy = twins.classes[0].clone();
        copy.name = "Elsewhere".to_owned();
        twins.classes.push(copy);
        assert_eq!(twins.validate_loaded(), Err(ScheduleError::DuplicateId));

        // Counters behind the ids they must not collide with are raised.
        let mut stale_counter = plan.clone();
        stale_counter.next_class_id = 0;
        stale_counter.next_agent_id = 0;
        stale_counter.validate_loaded().expect("otherwise valid");
        let fresh = stale_counter.add_class("Cat 6060", 2200.0).expect("valid");
        assert_ne!(fresh, class, "a reloaded plan must not hand out an id it already holds");
    }

    #[test]
    fn a_suggested_name_steps_past_the_names_already_taken() {
        let (plan, _, _) = fleet();
        let taken = || plan.classes().iter().map(|class| class.name.clone());
        assert_eq!(suggested_name("Loader Class", taken()), "Loader Class");
        assert_eq!(suggested_name("liebherr 9400", taken()), "liebherr 9400 2");
    }
}
```

### `src/model/formats/omf.rs`

```rust
#[cfg(test)]
mod schedule_tests {
    use super::*;
    use crate::model::schedule::{LoaderClassId, ScheduleError, SchedulePlan};

    fn fleet() -> SchedulePlan {
        let mut plan = SchedulePlan::default();
        plan.set_name("Q1 mine plan");
        let liebherr = plan.add_class("Liebherr 9400", 3000.0).expect("valid class");
        let cat = plan.add_class("Cat 6060", 2200.0).expect("valid class");
        plan.add_agent("EX7001", liebherr).expect("valid agent");
        plan.add_agent("EX7002", liebherr).expect("valid agent");
        plan.add_agent("EX8001", cat).expect("valid agent");
        plan
    }

    fn payload(plan: &SchedulePlan, version: u64) -> serde_json::Value {
        serde_json::json!({ "version": version, "plan": serde_json::to_value(plan).expect("plan serializes") })
    }

    /// The bytes an actual save writes, read back the way an actual open
    /// reads them. Not a serde round-trip: the payload has to survive the
    /// element metadata it is carried in.
    fn round_trip(document: Document) -> ImportBundle {
        let progress = crate::model::progress::Progress::new();
        let mut designs = crate::model::project::new_empty(None);
        designs.document = document;
        let snapshot = ProjectSnapshot {
            name: "round trip".to_owned(),
            designs: Some(designs),
            ..ProjectSnapshot::default()
        };
        let bytes = to_bytes(snapshot, &progress.phase(0.0, 1.0)).expect("writes");
        from_bytes("round trip.omf", bytes, &progress.phase(0.0, 1.0)).expect("reads")
    }

    #[test]
    fn a_saved_fleet_comes_back_with_its_names_rates_ids_assignments_and_order() {
        let plan = fleet();
        let mut document = Document::new();
        document.set_schedule(plan.clone());
        let bundle = round_trip(document);

        assert!(bundle.warnings.is_empty(), "{:?}", bundle.warnings);
        let restored = bundle.designs.first().expect("one designs element").document.schedule();
        assert_eq!(restored, &plan, "every field, id and list position survives the file");
        // Spelled out, so a future change to `PartialEq` cannot quietly weaken
        // what this test is checking.
        assert_eq!(restored.name, "Q1 mine plan");
        assert_eq!(
            restored.classes().iter().map(|class| class.name.as_str()).collect::<Vec<_>>(),
            ["Liebherr 9400", "Cat 6060"]
        );
        assert_eq!(
            restored.agents().iter().map(|agent| agent.name.as_str()).collect::<Vec<_>>(),
            ["EX7001", "EX7002", "EX8001"]
        );
        let agents: Vec<_> = restored.agents().iter().map(|agent| restored.effective_rate_tph(agent.id)).collect();
        assert_eq!(agents, [Some(3000.0), Some(3000.0), Some(2200.0)]);
    }

    #[test]
    fn a_project_saved_before_scheduling_existed_opens_with_an_empty_schedule() {
        // No schedule is written at all when there is nothing to write, which
        // is exactly the shape of every file saved by an earlier build.
        let bundle = round_trip(Document::new());
        assert!(bundle.warnings.is_empty(), "{:?}", bundle.warnings);
        let restored = bundle.designs.first().expect("one designs element").document.schedule();
        assert!(restored.is_empty());
        assert_eq!(restored, &SchedulePlan::default());
    }

    #[test]
    fn an_unreadable_schedule_is_reported_rather_than_guessed_at() {
        let plan = fleet();

        // A file from a newer build.
        let newer = read_schedule(payload(&plan, SCHEDULE_METADATA_VERSION + 1)).expect_err("refused");
        assert!(newer.contains("newer version"), "{newer}");

        // A rate that did not survive whatever wrote it. The point is that no
        // plausible-looking default takes its place.
        let mut broken = serde_json::to_value(&plan).expect("serializes");
        broken["classes"][0]["default_dig_rate_tph"] = serde_json::json!(0.0);
        let reason = read_schedule(serde_json::json!({ "version": SCHEDULE_METADATA_VERSION, "plan": broken })).expect_err("refused");
        assert_eq!(reason, ScheduleError::InvalidRate.message());

        // An agent whose class is not in the file.
        let mut orphaned = plan.clone();
        orphaned.remove_agent(orphaned.agents()[2].id).expect("exists");
        orphaned.remove_agent(orphaned.agents()[1].id).expect("exists");
        orphaned.remove_agent(orphaned.agents()[0].id).expect("exists");
        let mut value = serde_json::to_value(&orphaned).expect("serializes");
        value["agents"] = serde_json::json!([{ "id": 7, "name": "EX9999", "class_id": 404 }]);
        let reason = read_schedule(serde_json::json!({ "version": SCHEDULE_METADATA_VERSION, "plan": value })).expect_err("refused");
        assert_eq!(reason, ScheduleError::UnknownClass.message());

        // Structural nonsense.
        assert!(read_schedule(serde_json::json!({ "version": SCHEDULE_METADATA_VERSION, "plan": 3 })).is_err());
        assert!(read_schedule(serde_json::json!("not a payload at all")).is_err());
    }

    #[test]
    fn design_content_and_a_schedule_travel_in_the_same_file_without_disturbing_each_other() {
        let mut document = Document::new();
        let layer = document.add_layer("Pit designs".to_owned(), None, [1.0; 4], true, 0.0);
        document.insert_object(crate::model::Object::Point {
            id: crate::model::ObjectId(1),
            layer,
            pos: glam::DVec3::new(1.0, 2.0, 3.0),
            color: crate::model::ObjectColor::ByLayer,
        });
        document.set_schedule(fleet());

        let bundle = round_trip(document);
        assert!(bundle.warnings.is_empty(), "{:?}", bundle.warnings);
        let restored = &bundle.designs.first().expect("one designs element").document;
        assert_eq!(restored.layers().len(), 1, "the design layer is still there");
        assert_eq!(restored.objects().len(), 1, "and the object on it");
        assert_eq!(restored.schedule(), &fleet(), "and the schedule beside them");
    }

    #[test]
    fn the_payload_carries_its_own_version() {
        let plan = fleet();
        let written = payload(&plan, SCHEDULE_METADATA_VERSION);
        assert_eq!(written["version"], serde_json::json!(SCHEDULE_METADATA_VERSION));
        assert_eq!(read_schedule(written).expect("reads"), plan);
        // An id that a hand-edited file left colliding is refused, not merged.
        let mut twins = serde_json::to_value(&plan).expect("serializes");
        twins["classes"][1]["id"] = twins["classes"][0]["id"].clone();
        let reason = read_schedule(serde_json::json!({ "version": SCHEDULE_METADATA_VERSION, "plan": twins })).expect_err("refused");
        assert_eq!(reason, ScheduleError::DuplicateId.message());
        let _ = LoaderClassId(0);
    }

    #[test]
    fn empty_fleet_preserves_retired_ids_on_reopen() {
        let mut plan = SchedulePlan::default();
        let id = plan.add_class("Retired", 3000.0).unwrap();
        plan.remove_class(id).unwrap();
        let mut document = Document::new();
        document.set_schedule(plan.clone());
        let bundle = round_trip(document);
        let mut restored = bundle.designs[0].document.schedule().clone();
        assert_eq!(restored, plan);
        let mut merged = Document::new();
        merged.merge_schedule_from(&bundle.designs[0].document);
        assert_eq!(merged.schedule(), &plan);
        assert_ne!(restored.add_class("Replacement", 3000.0).unwrap(), id);
    }
}
```

### `src/app/commands/schedule.rs`

```rust
#[cfg(test)]
mod tests {
    use crate::{
        model::schedule::{LoaderAgentId, LoaderClassId, SchedulePlan},
        ui::state::{ScheduleEdit, UiCommand},
    };

    /// A headless app on one untitled project, the way the application itself
    /// starts before anything is drawn.
    fn app() -> crate::app::App<'static> {
        let mut app = crate::app::App::default();
        app.start_untitled_project().expect("an empty project always opens");
        app
    }

    fn plan(app: &crate::app::App<'_>) -> SchedulePlan {
        app.workspace.active_document().expect("a project is open").schedule().clone()
    }

    fn command(app: &mut crate::app::App<'_>, command: UiCommand) {
        app.handle_ui_command(command).expect("schedule commands never fail the dispatcher");
    }

    /// One fleet edit addressed to the project that is open, the way the UI
    /// addresses it from the projection it drew the plan from.
    fn run(app: &mut crate::app::App<'_>, edit: ScheduleEdit) {
        let session = app.workspace.active_project().expect("a project is open").runtime_id;
        command(app, UiCommand::schedule(session, edit));
    }

    fn fleet(app: &mut crate::app::App<'_>) -> (LoaderClassId, LoaderAgentId) {
        run(
            app,
            ScheduleEdit::AddClass {
                name: "Liebherr 9400".to_owned(),
                rate_tph: 3000.0,
            },
        );
        let class = plan(app).classes()[0].id;
        run(app, ScheduleEdit::AddAgent { name: "EX7001".to_owned(), class });
        let agent = plan(app).agents()[0].id;
        (class, agent)
    }

    fn undo(app: &mut crate::app::App<'_>) {
        command(app, UiCommand::Undo);
    }

    fn redo(app: &mut crate::app::App<'_>) {
        command(app, UiCommand::Redo);
    }

    #[test]
    fn the_target_workflow_reaches_the_fleet_the_gantt_draws_rows_from() {
        let mut app = app();
        let (class, agent) = fleet(&mut app);
        run(&mut app, ScheduleEdit::AddAgent { name: "EX7002".to_owned(), class });
        let fleet = plan(&app);
        assert_eq!(fleet.agents().len(), 2, "both machines are on the fleet, and so on the Gantt");
        assert!(fleet.agents().iter().all(|agent| fleet.effective_rate_tph(agent.id) == Some(3000.0)));

        run(&mut app, ScheduleEdit::SetClassRate { class, rate_tph: 3500.0 });
        run(
            &mut app,
            ScheduleEdit::RenameClass {
                class,
                name: "Liebherr 9400 XE".to_owned(),
            },
        );
        let fleet = plan(&app);
        assert!(fleet.agents().iter().all(|agent| fleet.effective_rate_tph(agent.id) == Some(3500.0)));
        assert_eq!(fleet.agents()[0].id, agent, "an agent's identity is untouched by its class being edited");
        assert_eq!(fleet.agents()[0].class_id, class);
    }

    #[test]
    fn every_kind_of_edit_is_exactly_one_undo_step_and_comes_back_on_redo() {
        let mut app = app();
        let (class, agent) = fleet(&mut app);
        let spare = {
            run(
                &mut app,
                ScheduleEdit::AddClass {
                    name: "Cat 6060".to_owned(),
                    rate_tph: 2200.0,
                },
            );
            plan(&app).classes()[1].id
        };
        let baseline = plan(&app);

        let edits = [
            ScheduleEdit::RenameClass {
                class,
                name: "Liebherr 9400 XE".to_owned(),
            },
            ScheduleEdit::SetClassRate { class, rate_tph: 3500.0 },
            ScheduleEdit::RenameAgent { agent, name: "EX7009".to_owned() },
            ScheduleEdit::SetAgentClass { agent, class: spare },
            ScheduleEdit::DeleteAgent(agent),
        ];
        for edit in edits {
            let before = plan(&app);
            run(&mut app, edit.clone());
            let after = plan(&app);
            assert_ne!(before, after, "{edit:?} changed nothing");

            undo(&mut app);
            assert_eq!(plan(&app), before, "one undo did not take back exactly {edit:?}");
            redo(&mut app);
            assert_eq!(plan(&app), after, "redo did not put {edit:?} back");
        }

        // And all the way back to where the sequence started, one step each.
        for _ in 0..edits_len() {
            undo(&mut app);
        }
        assert_eq!(plan(&app), baseline);
        assert_eq!(
            plan(&app).agent(agent).map(|agent| agent.id),
            Some(agent),
            "undo restores the same ids, not equivalent new ones"
        );
    }

    fn edits_len() -> usize {
        5
    }

    #[test]
    fn an_addition_and_a_deletion_each_undo_to_the_state_before_them() {
        let mut app = app();
        let (class, agent) = fleet(&mut app);
        let full = plan(&app);

        undo(&mut app);
        assert_eq!(plan(&app).agents().len(), 0, "the agent is gone");
        assert_eq!(plan(&app).classes().len(), 1, "its class is not");
        undo(&mut app);
        // The fleet, not the whole plan: the id allocator deliberately does
        // not rewind with an undo, so that an undone id is never reissued.
        assert!(plan(&app).is_empty());

        redo(&mut app);
        redo(&mut app);
        assert_eq!(plan(&app), full, "redo restores both, with their ids");
        assert_eq!(plan(&app).agent(agent).map(|agent| agent.class_id), Some(class));

        run(&mut app, ScheduleEdit::DeleteAgent(agent));
        run(&mut app, ScheduleEdit::DeleteClass(class));
        assert!(plan(&app).is_empty());
        undo(&mut app);
        undo(&mut app);
        assert_eq!(plan(&app), full);
    }

    #[test]
    fn an_undone_addition_never_lends_its_id_to_the_next_one() {
        let mut app = app();
        let (class, agent) = fleet(&mut app);
        let runtime = app.workspace.active_project().expect("open").runtime_id;
        undo(&mut app);
        undo(&mut app);
        assert!(plan(&app).is_empty(), "the fleet is back to empty");
        assert!(!app.project_content_is_dirty(runtime), "and an id counter left standing is not unsaved work");

        // The undone ids are spent: a class and a machine added on the new
        // branch are different equipment and must be told apart from the ones
        // a redo could bring back.
        let (other_class, other_agent) = fleet(&mut app);
        assert_ne!(other_class, class, "a new class took the undone class's id");
        assert_ne!(other_agent, agent, "a new machine took the undone machine's id");
    }

    #[test]
    fn an_edit_drawn_against_a_project_that_is_gone_is_refused() {
        let mut app = app();
        let (class, _) = fleet(&mut app);
        let stale = UiCommand::schedule(
            app.workspace.active_project().expect("open").runtime_id,
            ScheduleEdit::SetClassRate { class, rate_tph: 3500.0 },
        );

        // The same rate edit, queued in one frame and handled after the
        // project it was drawn from has been replaced. Its class id resolves
        // in the replacement too - a fresh project numbers its first class 0
        // as well - so only the session token can tell that it is stale.
        app.start_untitled_project().expect("opens");
        run(
            &mut app,
            ScheduleEdit::AddClass {
                name: "Cat 6060".to_owned(),
                rate_tph: 2200.0,
            },
        );
        let before = plan(&app);
        assert_eq!(before.classes()[0].id, class, "the ids really do collide across projects");

        command(&mut app, stale);
        assert_eq!(plan(&app), before, "a stale edit reached the new project's fleet");

        // Nor did it push a history entry: the one undo available is still
        // the class this project genuinely added.
        undo(&mut app);
        assert!(plan(&app).is_empty(), "the stale edit left an undo step behind");
    }

    #[test]
    fn a_refused_edit_changes_nothing_and_leaves_no_undo_step_behind() {
        let mut app = app();
        let (class, agent) = fleet(&mut app);
        let before = plan(&app);

        let refused = [
            // A class still in use.
            ScheduleEdit::DeleteClass(class),
            // Names that break the rules.
            ScheduleEdit::RenameAgent { agent, name: "   ".to_owned() },
            ScheduleEdit::RenameClass { class, name: String::new() },
            // Rates that are not rates.
            ScheduleEdit::SetClassRate { class, rate_tph: 0.0 },
            ScheduleEdit::SetClassRate { class, rate_tph: -5.0 },
            ScheduleEdit::SetClassRate { class, rate_tph: f64::NAN },
            // Ids from a stale view.
            ScheduleEdit::RenameClass {
                class: LoaderClassId(404),
                name: "Ghost".to_owned(),
            },
            ScheduleEdit::DeleteAgent(LoaderAgentId(404)),
            ScheduleEdit::SetAgentClass { agent, class: LoaderClassId(404) },
            ScheduleEdit::AddAgent {
                name: "EX7003".to_owned(),
                class: LoaderClassId(404),
            },
            // A duplicate, differently cased and padded.
            ScheduleEdit::AddAgent {
                name: " ex7001 ".to_owned(),
                class,
            },
        ];
        for command in refused {
            run(&mut app, command.clone());
            assert_eq!(plan(&app), before, "{command:?} was not refused cleanly");
        }

        // An edit that hands back the value already held is not an edit.
        run(
            &mut app,
            ScheduleEdit::RenameClass {
                class,
                name: "Liebherr 9400".to_owned(),
            },
        );
        run(&mut app, ScheduleEdit::SetClassRate { class, rate_tph: 3000.0 });
        assert_eq!(plan(&app), before);

        // Nothing above pushed a history entry, so the one undo available is
        // still the agent that was genuinely added.
        undo(&mut app);
        assert_eq!(plan(&app).agents().len(), 0);
        assert_eq!(plan(&app).classes().len(), 1);
    }

    #[test]
    fn editing_the_fleet_dirties_the_project_and_undoing_back_to_the_save_cleans_it() {
        let mut app = app();
        let runtime = app.workspace.active_project().expect("open").runtime_id;
        assert!(!app.project_content_is_dirty(runtime), "an untouched new project is not unsaved work");

        let (class, _) = fleet(&mut app);
        assert!(app.project_content_is_dirty(runtime), "an added machine is unsaved work");

        undo(&mut app);
        undo(&mut app);
        assert!(!app.project_content_is_dirty(runtime), "undoing back to the save clears the marker");

        redo(&mut app);
        assert!(app.project_content_is_dirty(runtime));
        // And a refused edit never dirties anything.
        undo(&mut app);
        assert!(!app.project_content_is_dirty(runtime));
        run(&mut app, ScheduleEdit::SetClassRate { class, rate_tph: -1.0 });
        assert!(!app.project_content_is_dirty(runtime));
    }

    #[test]
    fn opening_a_second_project_shows_only_its_own_fleet_even_with_identical_ids() {
        let mut app = app();
        let (first_class, _) = fleet(&mut app);
        let first = app.workspace.active_project().expect("open").runtime_id;

        // A second project, whose schedule ids necessarily collide with the
        // first's: this build holds one project at a time, so both number
        // their first loader class 0. Only the runtime id separates them,
        // which is why it is handed out by the process rather than by a store
        // that is thrown away with the project.
        app.start_untitled_project().expect("opens");
        let second = app.workspace.active_project().expect("open").runtime_id;
        assert_ne!(first, second, "the replacement project reused the closed project's session token");
        assert!(plan(&app).is_empty(), "a new project starts with no fleet");

        run(
            &mut app,
            ScheduleEdit::AddClass {
                name: "Cat 6060".to_owned(),
                rate_tph: 2200.0,
            },
        );
        let second_class = plan(&app).classes()[0].id;
        assert_eq!(second_class, first_class, "the ids really do collide across projects");
        assert_eq!(plan(&app).classes()[0].name, "Cat 6060", "and the active project's own fleet is what is shown");
        assert_eq!(plan(&app).agents().len(), 0, "the first project's machine is not here");

        // The UI projection reads the active project, not whichever document
        // the render-scene composite copied last.
        let view = app.project_view_for_test();
        assert_eq!(view.schedule.classes().len(), 1);
        assert_eq!(view.schedule.classes()[0].name, "Cat 6060");
        assert!(view.schedule.agents().is_empty());
    }

    #[test]
    fn switching_projects_clears_the_selections_and_drafts_that_named_the_old_one() {
        let mut app = app();
        let (class, agent) = fleet(&mut app);
        app.editor.schedule_selected_class = Some(class);
        app.editor.schedule_selected_agent = Some(agent);
        app.editor.schedule_name_draft = Some(crate::ui::state::ScheduleNameDraft {
            source: String::new(),
            text: "half typed".to_owned(),
        });
        app.editor.gantt.pan(5.0 * 86_400.0);

        app.start_untitled_project().expect("opens");
        assert_eq!(app.editor.schedule_selected_class, None);
        assert_eq!(app.editor.schedule_selected_agent, None);
        assert_eq!(app.editor.schedule_name_draft, None);
        assert_eq!(app.editor.gantt.start_seconds, 0.0, "the Gantt goes back to the start of the new project's time");
    }

    #[test]
    fn schedule_edits_do_not_disturb_the_solids_pipeline() {
        let mut app = app();
        // The fingerprints the Solids stages are marked stale against must be
        // blind to the fleet: configuring a schedule is not a reason to rerun
        // a reserve scan.
        let before = app.planning_fingerprints();
        let (class, _) = fleet(&mut app);
        run(&mut app, ScheduleEdit::SetClassRate { class, rate_tph: 3500.0 });
        run(&mut app, ScheduleEdit::SetName("Q1 mine plan".to_owned()));
        assert_eq!(app.planning_fingerprints(), before, "a schedule edit moved a Solids stage fingerprint");
    }
}
```

### `src/ui/state.rs`

```rust
#[cfg(test)]
mod gantt_view_tests {
    use super::GanttView;

    const HOUR: f64 = GanttView::HOUR;
    const DAY: f64 = GanttView::DAY;

    #[test]
    fn a_gantt_opens_on_a_week_of_elapsed_project_time() {
        let view = GanttView::default();
        assert_eq!(view.start_seconds, 0.0, "time starts at Day 1, 00:00, not at the clock");
        assert_eq!(view.span_seconds, 7.0 * DAY);
        assert_eq!(view.end_seconds(), 7.0 * DAY);
    }

    #[test]
    fn screen_positions_follow_the_rect_they_are_asked_about() {
        let view = GanttView::default();
        assert_eq!(view.x_of(0.0, 100.0, 700.0), 100.0);
        assert_eq!(view.x_of(7.0 * DAY, 100.0, 700.0), 800.0);
        assert_eq!(view.x_of(3.5 * DAY, 100.0, 700.0), 450.0);
        // The same instants against a pane half the width, which is all a
        // resize or a DPI change amounts to here.
        assert_eq!(view.x_of(3.5 * DAY, 0.0, 350.0), 175.0);
        // Off-screen instants still map, so a bar starting before the window
        // can be clipped rather than dropped.
        assert!(view.x_of(-DAY, 0.0, 700.0) < 0.0);
    }

    #[test]
    fn zoom_holds_the_instant_under_the_pointer() {
        let mut view = GanttView::default();
        view.pan(10.0 * DAY);
        let anchor = 0.25;
        let pinned = view.start_seconds + anchor * view.span_seconds;
        for factor in [1.5, 1.5, 1.5, 1.0 / 1.5, 0.5] {
            view.zoom_at(factor, anchor);
            let now = view.start_seconds + anchor * view.span_seconds;
            assert!((now - pinned).abs() < 1e-6, "the anchored instant moved by {}", now - pinned);
        }
    }

    #[test]
    fn the_window_never_leaves_its_limits() {
        let mut view = GanttView::default();
        // Zooming in past an hour stops at an hour.
        for _ in 0..40 {
            view.zoom_at(2.0, 0.5);
        }
        assert_eq!(view.span_seconds, GanttView::MIN_SPAN_SECONDS);
        // And out past a year stops at a year.
        for _ in 0..40 {
            view.zoom_at(0.5, 0.5);
        }
        assert_eq!(view.span_seconds, GanttView::MAX_SPAN_SECONDS);

        // Time before the start of the project is not somewhere to pan to.
        view.reset();
        view.pan(-100.0 * DAY);
        assert_eq!(view.start_seconds, 0.0);
        // Zooming out at the left edge widens to the right instead of pinning.
        view.zoom_at(0.5, 0.0);
        assert_eq!(view.start_seconds, 0.0);
        assert_eq!(view.span_seconds, 14.0 * DAY);

        // Nonsense is refused rather than propagated into the view.
        let steady = view;
        view.zoom_at(f64::NAN, 0.5);
        view.zoom_at(0.0, 0.5);
        view.pan(f64::INFINITY);
        assert_eq!(view, steady);
    }

    #[test]
    fn reset_returns_the_view_to_the_start_of_the_project() {
        let mut view = GanttView::default();
        view.pan(30.0 * DAY);
        view.zoom_at(20.0, 0.5);
        view.row_scroll = 120.0;
        view.reset();
        assert_eq!(view.start_seconds, 0.0);
        assert_eq!(view.span_seconds, GanttView::DEFAULT_SPAN_SECONDS);
        assert_eq!(view.row_scroll, 120.0, "which row is on screen is not part of the time view");
    }

    #[test]
    fn the_tick_interval_steps_up_as_the_labels_run_out_of_room() {
        let mut view = GanttView::default();
        // A week across a wide pane: half-days fit, under a band of days.
        assert_eq!(view.minor_interval(1400.0, 72.0), 12.0 * HOUR);
        // The same week in a narrow column steps up rather than overlapping.
        assert_eq!(view.minor_interval(300.0, 72.0), 2.0 * DAY);
        assert_eq!(view.minor_interval(120.0, 72.0), 7.0 * DAY);

        // Zoomed to a day, the ruler is in hours.
        view.span_seconds = DAY;
        assert_eq!(view.minor_interval(1400.0, 72.0), 3.0 * HOUR);
        assert_eq!(view.minor_interval(400.0, 72.0), 6.0 * HOUR);
        assert_eq!(view.minor_interval(2400.0, 72.0), HOUR, "a very wide pane reaches the finest interval");
        // And at the closest zoom, in single hours.
        view.span_seconds = GanttView::MIN_SPAN_SECONDS;
        assert_eq!(view.minor_interval(1400.0, 72.0), HOUR);

        // Every interval offered is one from the ladder.
        for width in [80.0_f32, 300.0, 900.0, 2400.0] {
            for span in [HOUR, 6.0 * HOUR, DAY, 7.0 * DAY, 90.0 * DAY, 365.0 * DAY] {
                view.span_seconds = span;
                let interval = view.minor_interval(width, 72.0);
                assert!(
                    GanttView::TICK_LADDER.contains(&interval) || interval == GanttView::MAX_SPAN_SECONDS,
                    "{interval} is not a tick interval"
                );
                // And it always leaves at least the requested spacing.
                let spacing = f64::from(width) * interval / span;
                assert!(
                    spacing >= 72.0 || interval == GanttView::MAX_SPAN_SECONDS,
                    "ticks {spacing} apart at width {width}, span {span}"
                );
            }
        }
    }

    #[test]
    fn only_the_ticks_on_screen_are_visited() {
        let mut view = GanttView::default();
        let ticks: Vec<_> = view.visible_ticks(DAY).collect();
        assert_eq!(ticks, vec![0.0, DAY, 2.0 * DAY, 3.0 * DAY, 4.0 * DAY, 5.0 * DAY, 6.0 * DAY, 7.0 * DAY]);

        // Scrolled to day 300, the same eight ticks - not three hundred.
        view.pan(300.0 * DAY);
        let ticks: Vec<_> = view.visible_ticks(DAY).collect();
        assert_eq!(ticks.len(), 8);
        assert_eq!(ticks.first().copied(), Some(300.0 * DAY));
        assert!(ticks.iter().all(|tick| *tick >= view.start_seconds && *tick <= view.end_seconds()));

        // A window that starts part way through an interval begins at the
        // next whole one rather than at its own left edge.
        view.reset();
        view.pan(HOUR * 5.0);
        let ticks: Vec<_> = view.visible_ticks(DAY).collect();
        assert_eq!(ticks.first().copied(), Some(DAY));

        // The finest interval across the widest window is still bounded by
        // what fits, so nothing here can iterate unboundedly.
        view.span_seconds = GanttView::MAX_SPAN_SECONDS;
        assert_eq!(view.visible_ticks(GanttView::MAX_SPAN_SECONDS).count(), 1);
    }
}
```

### `src/ui/elements/schedule_setup.rs`

```rust
#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn opening_class_properties_preserves_full_rate_precision() {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .families
            .insert(egui::FontFamily::Name("noto_sans_bold".into()), fonts.families[&egui::FontFamily::Proportional].clone());
        ctx.set_fonts(fonts);
        let mut editor = EditorState::new();
        let mut plan = SchedulePlan::default();
        let id = plan.add_class("Loader", 3000.123456).unwrap();
        editor.schedule_selected_class = Some(id);
        let mut commands = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                draw_class_properties(ui, ui.available_rect_before_wrap(), &mut editor, &plan, 1, &mut commands);
            });
        });
        output.textures_delta.clear();
        assert!(commands.is_empty());
        assert_eq!(editor.schedule_class_draft.as_ref().unwrap().rate.parse::<f64>().unwrap(), 3000.123456);
        for rate in [0.0001, 3000.123456, f64::MAX] {
            assert_eq!(rate_text(rate).parse::<f64>().unwrap(), rate);
        }
    }
}
```
