---
title: 'Autosave writes rotating side files without touching the document'
type: 'feature'
ticket: '5'
created: '2026-11-08'
status: done
baseline_revision: 'e8806c5'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** A crash or a closed window without saving loses all unsaved edits. Java autosaves every 10 minutes into 10 rotating slots (`AutoSave`, `GameXML.saveGameConfig(isAutoSave)`), remembering the last slot in a static field.

**Approach (AD-16):**
- `settings.toml` `[global]` gains `autosave` (default `true`), `autosave_minutes` (default 10, at least 1) and `autosave_slots` (default 10, at least 1).
- The app keeps a timer per open document (restarted when a document is loaded or saved). When the interval has passed and the document is **dirty, editable, not being saved or loaded, and no tool preview is active** (no drag, no pending connect/curve/arc, no selection being drawn, no modal dialog), a background job (AD-9) writes a snapshot of the config to `<config dir>/<config stem>_autosave_<n>.xml` atomically (AD-6). If a gate blocks, it retries on the next frame where the gates pass; the timer then restarts.
- **Slot:** the next slot after the newest existing autosave file of this config (by modification time), wrapping after `autosave_slots`; slot 1 when none exists. Slot numbers start at 1.
- The job never changes `Document.path`, the saved state (`saved_at`, the dirty title) or `vehicles.xml`, and never touches the config file itself.
- A failure adds a notice ("Autosave failed: <reason>"); a success shows "Autosaved to <file name>" briefly in the top bar (until the next notice or 10 s).
- The autosave files are gitignored (epic decision).

## Boundaries & Constraints

**Always:** the snapshot is taken on the UI thread at the moment of the write, so it holds exactly the current network; the timer uses `std::time::Instant` behind a small clock seam so tests can drive it.

**Never:** no restore UI (opening an autosave file with Open works as for any config); no autosave of read-only documents; no write when nothing changed since the last autosave (dirty and unchanged revision since the last autosave counts as nothing to do).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Due, dirty | interval passed, unsaved edit | `<stem>_autosave_1.xml` written with the current network; title still dirty; path unchanged | none |
| Clean | interval passed, no unsaved edit | nothing written | none |
| Unchanged | autosaved, no edit since | nothing written at the next interval | none |
| Preview | a drag or a pending connect in progress | waits; writes once the preview ends | none |
| Modal | a dialog shown | waits | none |
| Rotation | slots 1..3 exist, 3 newest, 10 slots | slot 4 | none |
| Wrap | slot 10 newest, 10 slots | slot 1 (overwritten) | none |
| Read-only | legacy config | never | none |
| Off | `autosave = false` | never | none |
| Fails | folder not writable / removed | notice with the reason | notice |
| Vehicles | config with vehicles.xml beside it | vehicles.xml untouched | none |
| Saved after | user saves after an autosave | normal save; autosave files stay | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/settings.rs` -- the three settings with defaults and minimums.
- `crates/ad-app/src/jobs.rs` -- `autosave_path(config, slot)`, `next_slot(dir, stem, slots)`, the write job (reuse `config_xml::save_to`), its `JobResult`.
- `crates/ad-app/src/main.rs` / `document.rs` -- the timer and gates (a clock seam), the last-autosaved revision, notices; `ui.rs` -- the brief label.

## Tasks & Acceptance

**Execution:**
- [x] settings -- defaults, minimums, missing keys
- [x] slot choice and paths -- tests for Rotation, Wrap
- [x] timer, gates and job -- tests for every other matrix row with a fake clock

**Acceptance Criteria:**
- Given a copy of the fixture with `autosave_minutes = 1`, when the user edits it and waits a minute, then `AutoDrive_config_autosave_1.xml` appears beside it with the edit, the title still shows unsaved changes, and `vehicles.xml` is unchanged.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Settings:** `autosave`, `autosave_minutes`, `autosave_slots` in `[global]`, the two numbers raised to 1 by the existing `at_least_one` deserializer.
- **Paths and slot:** `jobs::autosave_path(config, slot)` and `jobs::next_slot(config, slots)` (takes the config path instead of dir + stem; same result). `next_slot` looks only at slots 1..=`slots`, so files of a larger earlier slot count are ignored. It runs in the background job, so the UI thread does no file I/O.
- **Timer state per document:** `Document.autosave_since: Option<Instant>` (`None` starts the timer at the next check; a new document starts with `None`, a successful `Saved` resets it to `None`) and `Document.autosaved: Option<SavePoint>`. "Unchanged since the last autosave" compares the history `SavePoint`, not `revision`: `revision` also bumps on selection gestures, which change nothing that is written.
- **Clock seam:** `App::tick_autosave(now, notify)` takes the time as a parameter; `ui()` passes `Instant::now()`, tests pass `t0 + n minutes`. It returns the wait until the next needed check, and `ui()` hands that to `ctx.request_repaint_after`, so the due time fires while the user is idle. While blocked it asks for no repaint: whatever ends the block (input, a job's notify) brings a frame.
- **Busy predicate:** `App::busy()` = a claimed drag (`view.dragging()`), a selection being drawn, `Tool::in_progress()`, `dialog_shown()` or the unsaved-changes prompt. The autosave gate adds `saving`, `loading` and a running autosave. `Tool::in_progress()` is a new trait method defaulting to `preview().is_some()`; connect (pending start), Bézier (start, curve, handle drag) and arc (arc, handle drag) override it. The error window does not block.
- **Order:** the tick runs at the end of `ui()`, after this frame's input and dialogs. The timer restarts when a write starts and when it finds nothing to write (clean or unchanged); a blocked due check keeps retrying each frame. A failure keeps `autosaved`, so the next interval retries.
- **Results:** `JobResult::Autosaved { epoch, point, result }`; `apply_result` records the point or adds the notice "Autosave failed: <reason>" only for the matching epoch. `poll_jobs` sets the top-bar line "Autosaved to <file name>" (`App.autosave_label`), which drops after 10 s, when the document's notice changes, or with a new document; dropping it asks for one more repaint.
- **Verification:** `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` all pass (ad-app 419 + 3 ignored). `frame_time` (release): every case ok, worst best-round max 11.6 ms (rotate, min zoom, whole network selected); the view benchmark does not include the autosave tick, which is a few comparisons per frame. The config clone for the snapshot happens on the UI thread once per write, as the plan requires (same as the merge backup); not separately timed.
- **Tests:** settings `autosave_options_default_and_keep_the_minimum`; jobs `autosave_path_is_beside_the_config_with_the_slot`, `the_next_slot_follows_the_newest_autosave` (Rotation), `the_next_slot_wraps_after_the_last` (Wrap); app `a_due_autosave_writes_slot_1_and_keeps_the_title_path_config_and_vehicles` (Due, Vehicles, idle repaint, 10 s line), `a_notice_ends_the_autosaved_line`, `a_clean_or_unchanged_document_is_not_autosaved` (Clean, Unchanged), `a_due_autosave_waits_for_previews_selections_dialogs_and_saves` (Preview, Modal), `a_read_only_document_or_autosave_off_is_never_autosaved` (Read-only, Off), `a_failed_autosave_adds_a_notice_and_retries_after_the_interval` (Fails), `a_save_after_an_autosave_keeps_the_files_and_restarts_the_timer` (Saved after), `a_new_document_restarts_the_autosave_timer`; `in_progress` asserts in the Bézier and arc cancel tests.
- **Acceptance criterion** (a real minute in the GUI) needs a human hand check.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator), pass 1: no patch.
  - accepted: "unchanged" is compared by history save point instead of revision (a selection gesture changes nothing that is written); the snapshot clone runs on the UI thread once per write, as for the merge backup.
- Verification (orchestrator re-run): fmt, clippy and workspace tests pass (ad-app 419).
