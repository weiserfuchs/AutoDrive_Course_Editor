---
title: 'A menu bar carries the file, edit, scan, import and routes commands'
type: 'feature'
ticket: '2'
created: '2026-11-09'
status: done
baseline_revision: 'cba0b79'
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

**Problem:** File commands are buttons in the tool bar, Save always opens a dialog (deferred Retro R4, against AD-16), and there are no file shortcuts.

**Approach:**
- An egui menu bar above the tool bar:
  - File: Open…, Routes…, Save, Save As…, Exit.
  - Edit: Undo, Redo, Cut, Copy, Paste, Paste in place.
  - Display: Grid, Selected node id. Story 6 extends it.
  - Scan and Import: moved unchanged from the tool bar.
- Help comes with story 9.
- The tool bar keeps the tools and the status labels.
- Save writes to the document's own path without a dialog and is enabled only while the document is editable and has unsaved changes. Save As asks for the target. The unsaved-changes prompt's Save also saves in place, as Java does.
- Shortcuts (user decision): Ctrl+O, Ctrl+S, Ctrl+Shift+S, Ctrl+Q. Menu items show their shortcut.
- The user started this story while away; the plan was not approved separately, and the hand check is pending.

## Boundaries & Constraints

**Never:** no Help menu items yet (story 9); no new display toggles (story 6); no configurable keys (story 5).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Save clean | no unsaved changes | Save off, does nothing | none |
| Save dirty | edited document | written to its path at once, clean afterwards | save error as before |
| Read-only | read-only document | Save and Save As off | none |
| Exit, Open dirty | unsaved changes | the unsaved prompt | none |
| Keys | Ctrl+O/S/Shift+S/Q | Open, Save, Save As, Exit; plain S still toggles snap | none |
| Narrow window | 800 px | menus and tools inside the window | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/input.rs` -- actions Open, Save, SaveAs, Exit with their shortcuts; `perform` (shared by shortcuts and menus); `shortcut_text`; tests.
- `crates/ad-app/src/main.rs` -- `run_file`, `can_save`, `can_save_as`, `save` (in place), `save_as`, `start_save`; the prompt's Save saves in place; `menu_bar` before `top_bar`; tests.
- `crates/ad-app/src/ui.rs` -- `menu_bar` with the File, Edit, Display, Scan and Import menus; the tool bar without the moved commands; tests.
- `crates/ad-app/src/view.rs` -- the frame-time test draws the menu bar too.

## Tasks & Acceptance

**Execution:**
- [x] actions and shortcuts -- test Keys
- [x] save in place, save as -- tests Save clean, Save dirty, Read-only, Exit/Open dirty
- [x] menu bar -- test Narrow window

**Acceptance Criteria:**
- Given an opened config, when the user edits it and presses Ctrl+S, then it is saved to its file without a dialog; every menu item runs its command.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- File actions need the egui context (dialogs, the window), so `run_file` handles them and `run` ignores them; `input::perform` calls one or the other.
- Save on a route writes the route format (story 11.2), in place as for a config.
- Retro R4 (deferred-work) is resolved.
- Verification: fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 433 + 4 ignored); frame time every case ok.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: the old test that kept Ctrl+S free now expects Save; the prompt's Save on a read-only document still starts nothing.
