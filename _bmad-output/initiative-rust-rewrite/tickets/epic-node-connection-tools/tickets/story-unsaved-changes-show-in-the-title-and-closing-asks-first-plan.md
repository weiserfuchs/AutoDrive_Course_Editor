---
title: 'Unsaved changes show in the title and closing asks first'
type: 'feature'
ticket: '8'
created: '2026-11-07'
status: 'in-progress'
baseline_revision: '8d41deb4859cb64952e406a68256757b03372023'
route: 'full'
route_source: 'auto'
risk: 'low'
review: ''
review_source: ''
lenses_ran: []
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Edits can be lost silently: the window title does not show unsaved changes, and closing the window or opening another file drops a dirty document without asking (R7 of the initiative).

**Approach:** The window title shows the open file's full path and a trailing ` *` while the document is dirty (`History::is_dirty`), as Java does (`AutoDrive Course Editor - <path> *`). Closing the window, or Open, while the document is dirty first asks Save, Discard or Cancel:
- **Save** runs the normal save (its file dialog); the close or open continues only after that save succeeds; a cancelled dialog or a failed save keeps the document and the window.
- **Discard** continues at once.
- **Cancel** keeps everything as it is.
Java offers only Yes/No on close and none on open; Cancel and the open prompt are additions (R7, AD-13 spirit).

## Boundaries & Constraints

**Always:** dirty state comes only from `History` (AD-4/AD-16: `saved_at` vs. cursor); the prompt is modal over the map (no map input while it is shown); a read-only document offers no Save, only Discard and Cancel; the frame-time test still passes.

**Never:** no separate Save/Save As split (retro R4, epic-settings-ui-shell); no autosave (epic-scans-autosave); no change to the save job or `apply_result`.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Title clean | fixture opened, no edits | `AutoDrive Course Editor - C:\...\AutoDrive_config.xml` | none |
| Title dirty | one edit | same title with ` *` | none |
| Back to saved | undo to the save point | ` *` disappears | none |
| After save | save succeeds | ` *` disappears; path is the saved file | none |
| No document | nothing open | `AutoDrive Course Editor` | none |
| Close clean | window close, not dirty | window closes, no prompt | none |
| Close dirty → Cancel | window close while dirty, Cancel | window stays, document unchanged | none |
| Close dirty → Discard | Discard | window closes | none |
| Close dirty → Save ok | Save, file chosen, save succeeds | window closes after the save result | none |
| Close dirty → Save fails/cancelled | file dialog cancelled, or save error | window stays; error window on failure | existing error window |
| Open dirty | Open while dirty | same prompt; Save/Discard then show the open dialog; Cancel does nothing | as above |
| Read-only dirty | legacy config whose flags were reset (dirty at load) | prompt without Save | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/main.rs` -- `App`: add `pending: Option<Pending>` (`enum Pending { Close, Open }`), `prompt: Option<Pending>` (prompt shown), `allow_close: bool`, `title: String` (last sent). `save()` stays; `open()` becomes "request open" (prompt when dirty). In `ui()`: detect `ctx.input(|i| i.viewport().close_requested())`; when dirty and not `allow_close`, send `egui::ViewportCommand::CancelClose` and show the prompt. After `poll_jobs`, when `pending` is set, no save is running and the document is clean, continue the action (Close: `allow_close = true` + `ViewportCommand::Close`; Open: run the file dialog). Send `ViewportCommand::Title` only when the computed title changed.
- Title helper as a pure fn `window_title(doc: Option<&Document>) -> String` (unit-testable): `"AutoDrive Course Editor"` plus ` - {path.display()}` and ` *` when dirty.
- `crates/ad-app/src/ui.rs` -- `unsaved_prompt(app, ctx)`: a modal (`egui::Modal` in 0.37, or a centred non-collapsible `Window` with the map disabled) with the text "The document has unsaved changes." and buttons Save (hidden when read-only), Discard, Cancel; Open in the top bar goes through the request.
- `crates/ad-app/src/document.rs:74` -- drop `#[allow(dead_code)]` on `is_dirty`.
- `crates/ad-app/src/jobs.rs` -- read only: `apply_result` already sets `saved_at` and `path` on success and returns the error message on failure; Save with a cancelled dialog leaves `saving == false`.
- The save-then-continue rule needs "the save of this request finished": clear `pending` when a Save result arrives with an error, or when the user cancels the save dialog (`save()` returns without spawning; make it return `bool`).
- Do not touch: tools, view, `ad-core`.

## Tasks & Acceptance

**Execution:**
- [x] `main.rs` -- `window_title`, pending state, close interception, open request, continuation after save; tests for every matrix row that needs no window (title rows via `window_title`; the prompt flow by driving `App` methods: request close while dirty → prompt shown, Cancel → no close requested, Discard → `allow_close`, Save failure → pending cleared)
- [x] `ui.rs` -- prompt window and Open through the request
- [x] `document.rs` -- `is_dirty` allow removed
- [x] record in Implementation Notes how close interception is done in eframe 0.37 and any limits found

**Acceptance Criteria:**
- Given the fixture with one edit, when the user closes the window, then the prompt appears, Cancel keeps the window, and Discard closes it.
- Given the frame-time test, when run in release, then every case stays under 16 ms.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

**Manual checks (if no CLI):**
- The user edits the fixture, sees ` *`, undoes to see it go, tries close and Open while dirty (Cancel, Discard, and Save to a temp file), and confirms each path.

## Implementation Notes

- Close interception (eframe 0.37): each frame `App::ui` reads `ctx.input(|i| i.viewport().close_requested())`; when the document is dirty and `allow_close` is not set it sends `ViewportCommand::CancelClose` in the same frame, which eframe's `epi_integration` checks for before closing the root viewport, and shows the prompt. Closing after Discard, or after the save has succeeded, sets `allow_close` and sends `ViewportCommand::Close`; the next close request passes through. Limit: this covers the root viewport's close (window X, Alt+F4); a process kill is not intercepted.
- The prompt is an `egui::Modal` (its backdrop takes pointer input over the map and top bar); shortcuts are skipped while it is shown. The id uses `Id::unique` because `Id::new` is deprecated in egui 0.37.
- Save → continue: `save()` now returns whether a save job started. `pending` is set only then; `poll_jobs` clears it on a failed save (the existing error window shows the message); `resume` (after `poll_jobs`, once `saving` is false) continues the action when the document is clean, and shows the prompt again if edits made during the save left it dirty.
- The Save button in the prompt is disabled while a load or save runs, as in the top bar.
- Matrix row "Read-only dirty": a read-only load never resets flags (`build_network` is passed `read_only`), so a read-only document is not dirty in practice. The prompt still hides Save for it; the test forces the dirty state with `History::mark_unsaved`.
- Open with Save or Discard is not unit-tested to the end, since it opens the native file dialog; the Open prompt and Cancel are.

## Plan Change Log

## Review Triage Log
