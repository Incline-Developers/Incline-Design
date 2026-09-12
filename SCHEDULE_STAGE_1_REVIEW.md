# Schedule stage 1 review

## Decision

Proceed to stage 2 after the corrections in this review. The fleet model, active-project UI projection, command-based history and explicit OMF payload form a useful foundation. Keep undo support. The user's visual inspection is positive; visual consistency and Gantt workflow polish can accompany the next stage.

This is approval of the foundation, not a claim that production scheduling or all desktop/browser interactions are validated.

## Corrections made

1. **Text edits were submitted on every keystroke.** Schedule Setup used the shared `properties::committed` helper, which treats a changed, non-dragged response as committed. `PropertyTable::field` returns a plain TextEdit response, so each typed character could generate another history entry and refresh the draft. Schedule text fields now submit on focus loss (including Enter for single-line fields). Name validation is re-evaluated against the final draft before submission. Combo choices remain immediate commits.
2. **Rate display discarded precision.** `rate_text` rounded to two decimals and also seeded the editable draft. Small positive rates could display as zero and fractional rates were not faithfully editable. Display and drafts now preserve the stored floating-point value using round-trippable text.
3. **Empty fleets lost allocation history on reopen.** OMF omitted schedules whenever the name and entity lists were empty, even if deleted entities had advanced the counters. The import merge had the same omission. Both now distinguish a pristine default plan from a previously used, currently empty plan. Retired IDs and the serialized content survive that round trip.

## Stage 2 prerequisites

- **Project-scoped commands:** Schedule UiCommands currently carry entity IDs but no project/session identity. Handlers edit whichever project is active when dispatched. Resetting dialogs on project changes is useful but is not a stale-command rejection contract. Add a session token that does not restart on each open before introducing asynchronous picks, editors or calculations; reject messages with a different token.
- **Identity through undo branches:** Whole-plan undo restores the allocation counters too. Add class A, undo its addition, then add class B: B can receive A's ID. This is harmless for the current fully synchronous fleet references when restored together, but the “never reused” claim is too broad. Define safe identity semantics before persistent assignments and delayed jobs rely on them. Keep allocator bookkeeping separate from semantic dirty tracking if using a high-water mark.
- **Persistent dig-block references:** Stage 1 does not resolve Solids' session-bound references. A saved sequence must become explicitly unresolved when its source cannot be identified; never silently match a block by name or a reused numeric ID.
- **Explicit tonne source and calculated execution output:** Select a Sum reserve field with the stated tonne-unit assumption. Gate on a current snapshot and valid complete measurements. Keep authored sequences separate from execution segments, including interrupted/resumed segments, so Gantt and animation consume the same result.
- **Dispatch interpretation remains proposed:** Priority lanes belong to each agent. Lower lanes supply fallback work; higher-priority work can interrupt when available. Confirm availability rules and whether interruption happens immediately or only at a block boundary before implementing the evaluator. Do not interpret moving down as handing work to the next loader.

## Nonblocking polish

Align Schedule Setup's add/edit workflows and status icons with Solids where appropriate; the current permanent pending icons imply a runnable step that does not exist. Improve Gantt navigation alongside actual sequence manipulation, rather than polishing a blank timeline in isolation. Unicode name comparison is currently ASCII case-insensitive only; either document that rule or centralize Unicode-aware matching across UI and domain if broader names are required.

Do not spend this stage removing unrelated Solids test helpers. They do not block scheduling.

## Validation

See the appended review section in SCHEDULE_STAGE_1_VALIDATION.md for the exact extra regression sources and checks. The original tests drive commands directly; they did not catch the per-keystroke UI behaviour. The additional headless egui check covers draft initialization and rate fidelity, not a full physical keyboard interaction or a desktop/browser session. Those interaction checks remain necessary.
