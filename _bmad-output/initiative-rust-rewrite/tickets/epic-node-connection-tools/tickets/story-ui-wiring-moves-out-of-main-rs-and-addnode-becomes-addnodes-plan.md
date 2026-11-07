---
title: 'UI wiring moves out of main.rs and AddNode becomes AddNodes'
type: 'refactor'
ticket: '10'
created: '2026-11-07'
status: 'built'
baseline_revision: 'c395e2c0fd94f1eedbbcc4e017d15ca19f2c890e'
route: 'full'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** All UI wiring of the shell (top bar, keyboard input, error window, the overlay hook) sits in `crates/ad-app/src/main.rs`, so every tool story of this epic would edit the same function. The add command is named `AddNode`, where the command-naming convention says `AddNodes` (retro P2).

**Approach:** Move the top bar and the error window into a `ui` module and the keyboard handling into an `input` module, leaving `main.rs` with the eframe entry, the `App` struct, job polling and the open/save actions. Rename the add command to `AddNodes`. No behaviour changes.
- Decision (user, 2026-11-07): `AddNodes` takes a list of nodes and adds them in order, with their ids readable after apply; a redo reinserts each under its first id. One command then serves the node tool now and paste, lines and curves later.

## Boundaries & Constraints

**Always:** behaviour, rendering and test results stay identical; AD-1 crate boundaries hold; `AutoDrive_config.xml` is never written.

**Never:** no Tool trait, toolbar or new shortcuts (story 3.1); no change to `view.rs` drawing or `jobs.rs` behaviour; no new dependencies.

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/main.rs` -- `App` (fields, `new`, `poll_jobs`, `open`, `save`, `paint_network`), `eframe::App::ui` (F4 toggle, top bar with Open/Save/Grid/status labels, central panel, error window), `main`. Keep `App`, `poll_jobs`, `open`, `save`, `paint_network`, `main` here.
- `crates/ad-app/src/ui.rs` (new) -- `top_bar(app, ui, ctx)` and `error_window(app, ctx)`, moved verbatim from `ui()`.
- `crates/ad-app/src/input.rs` (new) -- `handle_shortcuts(app, ctx)`: the F4 node-id toggle, the place 3.1's shortcut table plugs in.
- `crates/ad-core/src/command.rs:14-41,121,137,195` -- `AddNode` struct, `Command::AddNode` variant, apply/revert arms; tests at :367, :484, :512.
- Rename call sites: `crates/ad-core/src/history.rs:202,206,295`, `crates/ad-app/src/jobs.rs:228,264`, `crates/ad-io/tests/golden_edited.rs:4,110`.
- Do not touch: `view.rs`, `hover.rs`, `overlay.rs`, `document.rs` (its `apply` allow stays until 3.1).

## Tasks & Acceptance

**Execution:**
- [x] `crates/ad-app/src/ui.rs`, `crates/ad-app/src/input.rs` -- create with the moved code; the functions take `&mut App` (fields made `pub(crate)` only where needed) -- one place per concern for the tool stories
- [x] `crates/ad-app/src/main.rs` -- declare the modules; `ui()` calls `input::handle_shortcuts`, `ui::top_bar`, the central panel and `ui::error_window` -- main.rs shrinks to entry and app wiring
- [x] `crates/ad-core/src/command.rs` and call sites -- replace `AddNode` by `AddNodes` holding a list of `(x, y, z, flag)` with `ids()` after apply; apply adds or reinserts all in order, revert removes them in reverse; a test covers apply, revert and redo of several nodes keeping ids and order -- convention, and one command for later multi-node adds
- [x] verify -- run the Verification commands

**Acceptance Criteria:**
- Given the refactored app, when the fixture is opened, then the top bar, Grid toggle, F4 toggle, read-only label, notice and error window behave as before.
- Given the workspace, when searched for `AddNode\b`, then no match remains.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- `ui` and `input` are child modules of `main.rs`, so they reach `App`'s private fields and `open`/`save` without any `pub(crate)` on the fields.
- `AddNodes` uses a non-empty `removed` list to tell a redo from the first apply; revert removes the nodes last first, and redo pops them back in order. `ids()` stays the same across undo and redo.
- `AddNode` still shows up in older planning docs under `_bmad-output/`. Those are history and were left as they are. No code matches remain.

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 1 low patched; 1 low rejected.
  - low, patch: the overlay hook (`paint_network` with the empty `view.show` closure) stayed in main.rs although the Intent names it as wiring to move out, so tool stories would still edit main.rs. `paint_network` moved verbatim to `ui::map_panel`; the Code Map line that kept it in main.rs was wrong. Verification re-run: all green.
  - low, reject: the AC grep for `AddNode\b` matches planning docs, and the Retro P2 entry in deferred-work.md still names `AddNode`. In code there is no match (checked); the docs are history, deferred-work.md is append-only, and the fix would only edit this plan. Retro P2's rename part is closed by this story.
