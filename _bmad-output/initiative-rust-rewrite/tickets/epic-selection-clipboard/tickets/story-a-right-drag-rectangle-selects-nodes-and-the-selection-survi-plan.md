---
title: 'A right-drag rectangle selects nodes and the selection survives undo'
type: 'feature'
ticket: '1'
created: '2026-11-07'
status: 'in-progress'
baseline_revision: '676660f3115afebccbd99221992c750bf374374a'
route: 'full'
route_source: 'auto'
risk: 'high'
review: ''
review_source: ''
lenses_ran: []
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Users cannot select nodes, so nothing can act on a group. The tracer of epic-selection-clipboard proves the selection path through every layer: input, document state, drawing, and its life across undo and document replacement.

**Approach:** Add `Document.selection`, a set of `NodeId` kept outside the history (AD-15), pruned to existing ids after every apply, undo and redo, and cleared when a document replaces the old one (AD-18). Selection uses the right mouse button whichever tool is active (epic decision, AD-13): a right drag draws an inclusive rectangle (AD-5) and toggles every node it covers; a right click on a node toggles that node; a right click on empty map clears the selection. Selected nodes are drawn with Java's selection ring (white); the rectangle is drawn dashed white while dragging.

## Boundaries & Constraints

**Always:** the network never changes through selection; commands still get explicit id lists (AD-15); a right drag never pans; left and middle buttons keep their current meaning; the frame-time test still passes.

**Never:** no lasso (story 4.4); no tool acting on the selection (stories 4.2, 4.5–4.9); no "select hidden" rule (epic-settings-ui-shell); no `select_after` yet (story 4.9).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Rectangle select | right drag over nodes, empty selection | those nodes selected, drawn with the ring | none |
| Inclusive edges | a node exactly on the rectangle's edge | selected | none |
| Toggle | right drag over a mix of selected and unselected nodes | each covered node flips its state | none |
| Click toggle | right click on a node | that node flips its state | none |
| Clear | right click on empty map | selection empty | none |
| Tiny drag | right press and release within egui's click tolerance | treated as a click | none |
| Prune on undo | a selected node is added by an edit, then undone | the node leaves the selection | none |
| Prune on delete | a selected node is deleted | it leaves the selection; undo does not re-select it | none |
| New document | load replaces the document | selection empty | none |
| Read-only | legacy config | selection works (it changes no data) | none |
| Any tool | node, move, connect … active | right drag and click select as above; left-button behaviour unchanged | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/document.rs` -- `pub selection: BTreeSet<NodeId>` (or `HashSet`; sorted iteration is easier to test); `apply`/`undo`/`redo` call `prune_selection()` (retain ids where `network.node(id).is_some()`). A new `Document` starts empty, which covers replacement (AD-18).
- `crates/ad-app/src/view.rs` `show` (~185-360) -- secondary button: on `drag_started_by(Secondary)` record the start (screen px), while dragging draw the dashed rectangle, on `drag_stopped` report `SelectEvent::Rect { a, b }` in map px; `clicked_by(Secondary)` reports `SelectEvent::Click { node: hovered_node }`. Panning stays primary/middle only. Return the event alongside the primary click (extend the return type or add a `ViewHooks` method `select(SelectEvent)`; follow the hooks pattern from story 3.2).
- `crates/ad-app/src/view.rs` paint -- draw the ring for every selected node in the visible set (reuse the hover ring drawing from story 2.9, white); take the selection as `&BTreeSet<NodeId>` (or a closure) so the paint loop checks membership per visible node, O(visible · log n).
- `crates/ad-app/src/ui.rs` `map_panel` / `ToolHooks` -- turn `SelectEvent` into selection changes: rectangle → `nodes_in_rect_into` with the corners converted to world (normalise min/max), toggle each; click on a node → toggle; click on nothing → clear.
- Pure helper `fn toggle(selection, ids)` with unit tests.
- `crates/ad-core/src/network/spatial.rs:143` `nodes_in_rect_into` -- inclusive, reuse.
- `crates/ad-app/src/view.rs` frame-time test -- add one static case with every fixture node selected (Notes: the check beyond criteria).
- Do not touch: tools, commands, `hover.rs` rules, `jobs.rs` replace logic (a new Document is already empty).

## Tasks & Acceptance

**Execution:**
- [x] `document.rs` -- selection field and pruning; tests for Prune on undo, Prune on delete, New document
- [x] `view.rs` -- secondary drag/click events, rectangle drawing, selection ring; tests: right drag reports a rect and does not pan, right click reports a click, primary drag still pans
- [x] `ui.rs` (+ pure helper) -- apply events to the selection; tests for Rectangle select, Inclusive edges, Toggle, Click toggle, Clear, Read-only, Any tool
- [x] frame-time test -- static case "min zoom, all selected"

**Acceptance Criteria:**
- Given the fixture, when the user right-drags over a road and right-clicks single nodes, then exactly those nodes show the selection ring, and a right click on empty map clears them.
- Given the frame-time test with the whole network selected, when run in release, then every case stays under 16 ms.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms, the all-selected case included

**Manual checks (if no CLI):**
- The user selects on the fixture by rectangle and click, clears, deletes a selected node and undoes it, and sees the selection behave as in the matrix.

## Implementation Notes

- The hooks pattern was extended: `ViewHooks::select(SelectEvent)` records the gesture and `ViewHooks::selection()` hands the view the set to ring. `ToolHooks` takes `&doc.selection` and keeps the event in `select_event`; `map_panel` applies it after `show` (after any commit), through the pure `ui::select` and `ui::toggle`.
- Deviation from the Code Map: the rectangle's press corner is stored in map px, not screen px, so a wheel zoom during the drag keeps the corner on the same map point. The drawn rectangle converts it back each frame.
- The right-click node is hit-tested at the click position (as the primary drag start does), not taken from the previous frame's hover, which a tool preview suppresses.
- The selection ring reuses the hover ring (white, `HOVER_RING_FACTOR`), drawn on visible selected nodes and on a selected previewed node. The rectangle is a 1 px white dashed line (4 px dash and gap).
- Frame time (release, this machine): "static: min zoom, all selected" best round median 2.2 ms, max 3.5 ms. A throwaway "mid zoom, dense, all selected" run (not kept) gave max 1.9 ms.

## Plan Change Log

## Review Triage Log
