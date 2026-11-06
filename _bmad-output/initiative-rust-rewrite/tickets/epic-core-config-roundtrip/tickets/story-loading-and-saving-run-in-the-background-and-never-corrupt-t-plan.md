---
title: 'Loading and saving run in the background and never corrupt the open document'
type: 'feature'
ticket: '7'
created: '2026-11-06'
status: done
baseline_revision: '496320d535e02d823ba5ccff19688e9d0714c488'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: ['{project-root}/_bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `ad-app` reads, parses and saves the config synchronously on the UI thread, so the window freezes. There is no `Document`, no epoch and no saved state, so nothing stops a late result from landing on the wrong document, or a save from marking newer edits as saved.

**Approach:**
- Add a `Document` that owns the loaded config, its `History`, path and epoch, and a job runner in `ad-app`. Load and save run on background threads and report through a channel that the UI polls each frame.
- Results apply only through `replace_document` (load) or the saved-state update (save), and only for a matching epoch (AD-9, AD-16, AD-18).

## Boundaries & Constraints

**Always:**
- Only the UI thread holds and mutates the `Document`. Jobs take owned inputs and return owned results over `std::sync::mpsc`. The UI drains the channel with `try_recv` each frame, and a finished job calls `ctx.request_repaint()`.
- **Load:**
  - A job reads and parses the file.
  - `Ok` → `replace_document`: new `Document`, new `History`, `epoch + 1`, clean, and the read-only gate set from `LoadedConfig::read_only()`.
  - `Err` → error window with a message naming the file and the `ConfigError` (which names element and value). The current document is unchanged.
- **Save:**
  - The request captures the epoch, the history position, a clone of the `LoadedConfig` and the target path. The job runs `save_to`.
  - `Ok` with a matching epoch → the saved position becomes the captured one, and `path` becomes the target (Save As). A non-matching epoch is dropped.
  - `Err` → error window. Saved state is unchanged.
- **History:**
  - `History` gains a way to capture the current position and to mark a captured position as saved.
  - A captured position that no longer names the same state (its entry was evicted or discarded, or entries were evicted since a base capture) leaves the document dirty.
- Open is disabled while a load job is running. Read-only documents keep Save disabled (story 6).
- No unwrap or expect. No new dependencies.
- Decision (user, 2026-11-06): `Document` lives in `ad-app` and holds `LoadedConfig` plus `History`. This deviates from the structural seed (`ad-core/document.rs`), and `ad-io`'s API stays unchanged.

**Never:**
- Autosave, backups, the settings file, logging or the panic hook (story 8).
- Edit UI, tools, selection, map context.
- Cancelling running jobs, or progress bars.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Load ok | job result `Ok(config)` at the current epoch | document replaced, epoch + 1, clean, read-only gate from the config | none |
| Load fails | malformed file while a document is open | document, epoch and history unchanged | error message names the path and element |
| Save ok | save captured at position P, no edit since | clean, and `path` = target | none |
| Save after edit | save captured at P, then an edit, then the result arrives | still dirty | none |
| Stale epoch | save result whose epoch is older than the current document | dropped; the new document's saved state is unchanged | none |
| Save fails | `save_to` error | saved state unchanged | error message |
| Saved base evicted | cap 1: capture at base, apply 2 entries, mark the capture saved | dirty | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/main.rs` -- story 2's synchronous `open`/`save` (l.16-58), the `App { config, path, error }` struct and the read-only label. Replace them with document and job handling. Keep the painter and the error window.
- `crates/ad-core/src/history.rs` -- `Position { Base, After(EntryId) }` is private.
  - `saved: Option<Position>`. `mark_saved()` and `is_dirty()` use `current()`.
  - Eviction happens in `apply`.
  - Add the capture/mark API here, and keep `mark_saved()` working as before.
- `crates/ad-io/src/config_xml.rs` -- `LoadedConfig` is `Clone`, `load(&[u8])`, `save_to(&LoadedConfig, &Path)` and `ConfigError`. No change is expected.
- New `crates/ad-app/src/document.rs` and `jobs.rs`. Put the testable logic there, not in the egui code.
- Structural seed: AD lists `document.rs` under `ad-core`. The user decided it lives in `ad-app` (see Boundaries).

## Tasks & Acceptance

**Execution:**
- [ ] `crates/ad-core/src/history.rs`:
  - `pub struct SavePoint` (opaque, `Copy`), `History::save_point()` and `mark_saved_at(SavePoint)`, following the invalidation rule above.
  - Tests for "Save after edit" and "Saved base evicted" at the `History` level.
- [ ] `crates/ad-app/src/document.rs`:
  - `Document { config: LoadedConfig, history: History, path: PathBuf, epoch: u64 }`.
  - `Document::apply(Transaction)`, a thin pass-through so tests can edit.
  - `is_dirty`.
- [ ] `crates/ad-app/src/jobs.rs`:
  - `JobResult::{Loaded { path, result }, Saved { epoch, point, target, result }}` and spawn helpers.
  - An `apply_result(&mut Option<Document>, &mut epoch_counter, JobResult) -> Option<String error>` free of egui, so it can be unit-tested.
- [ ] `crates/ad-app/src/main.rs` -- wire the dialogs to the jobs, poll each frame, disable Open while loading, and show a "Loading…"/"Saving…" label.
- [ ] Tests in `document.rs`/`jobs.rs`: one per matrix row. Use small inline configs, and real threads only for one end-to-end load through the channel.

**Acceptance Criteria:**
- Given the fixture opened, when it loads, then the window stays responsive and the network appears when the job finishes.
- Given a broken XML file opened while the fixture is shown, when loading fails, then an error window names the file and element, and the fixture stays shown.

## Implementation Notes

- 2026-11-06: The user approved the merge without running the manual GUI checks (responsiveness while loading, the error window for a broken file, a clean `git status` after Save). They remain unverified on screen.

## Plan Change Log

## Review Triage Log

| # | Finding | Verdict | Route | Evidence |
|---|---------|---------|-------|----------|
| 1 | No `replace_document`: `apply_result` replaces the document inline | low | patch | It deviates from the plan and AD-18's single replacement path, which later stories need to clear tool and selection state. The fix extracts `jobs::replace_document`, and the Loaded/Ok arm calls it. |
| 2 | A panicking job leaves `loading`/`saving` set, so Open or Save stays disabled | low | reject | It needs a panic in `ad-io`, which AD-12 forbids: no unwrap/expect, and bad input returns errors. Recovering would need `catch_unwind` or a drop guard. The panic hook comes in story 8. |
| 3 | The matrix tests parse the multi-MB fixture instead of small inline configs, and `document.rs` has no tests | low | patch | It deviates from the plan's test instructions and slows the suite. The tests now use an inline FS25 config, and the fixture stays only in the channel test. The `Document` behaviour is covered through the `jobs` tests, so no separate `document.rs` tests were added. |
| 4 | The load-failure test does not check that the message names the element | low | patch | The acceptance criterion requires path and element. The test now uses a bad `<x>` value and asserts both `broken.xml` and `<x>`. |
| 5 | The save error repeats the target path ("Cannot save X: cannot write X: …") | low | reject | Cosmetic. A fix needs a branch per error kind. |
| 6 | The broken-XML test leaves a file in the system temp directory | low | patch | Fixed by removing the temp dir in the test. |

## Design Notes

- **Epoch:**
  - A counter on `App` gives each new `Document` the next value. A save result applies only when `doc.epoch == result.epoch`.
  - Loads need no epoch check, because only one runs at a time (Open is disabled).
- **SavePoint:**
  - It holds the current `Position`, plus the count of evicted entries when it is `Base`.
  - `mark_saved_at` accepts `After(id)` only while `id` is still in `entries[..cursor]` or the redo tail, and `Base` only while the evicted count is unchanged. Otherwise the saved state becomes lost.
  - `is_dirty` keeps its rule: the saved state differs from the current position.
- **Save snapshot** is `LoadedConfig::clone()`. That is a few MB for the fixture and cheap next to the fsync.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including the golden tests and the new history, document and jobs tests

**Manual checks (if no CLI):**
- `cargo run -p ad-app --release`: open the fixture. The window stays responsive while it loads.
- Then open a broken XML file. The error names the file, and the fixture stays shown.
- Save over the fixture. `git status` is clean afterwards.
