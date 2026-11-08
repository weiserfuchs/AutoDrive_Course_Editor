---
title: 'Autosave writes rotating side files without touching the document'
type: 'feature'
ticket: '5'
created: '2026-11-08'
status: 'planned'
baseline_revision: 'e8806c5'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
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
- [ ] settings -- defaults, minimums, missing keys
- [ ] slot choice and paths -- tests for Rotation, Wrap
- [ ] timer, gates and job -- tests for every other matrix row with a fake clock

**Acceptance Criteria:**
- Given a copy of the fixture with `autosave_minutes = 1`, when the user edits it and waits a minute, then `AutoDrive_config_autosave_1.xml` appears beside it with the edit, the title still shows unsaved changes, and `vehicles.xml` is unchanged.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
