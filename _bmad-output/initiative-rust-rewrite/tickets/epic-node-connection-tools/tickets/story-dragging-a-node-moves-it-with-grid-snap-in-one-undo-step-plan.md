---
title: 'Dragging a node moves it with grid snap in one undo step'
type: 'feature'
ticket: '2'
created: '2026-11-07'
status: 'built'
baseline_revision: '5d8bcc79dfa9da6e4dbb40d19f9db864c12691cc'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/initiative-rust-rewrite/tickets/epic-node-connection-tools/tickets/story-a-node-added-with-the-node-tool-undoes-redoes-and-saves-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Nodes cannot be moved. Users need to drag a node to a new place, optionally snapped to the grid, and undo the whole drag in one step.

**Approach:** A move tool (M) drags the node under the cursor directly, with no axis widget. The drag is a preview in tool state; release commits one `MoveNodes` with the absolute before and after positions (AD-4, AD-10). Grid snap is a toggle (S, and a toolbar checkbox) that Shift inverts while held, for the move tool and the node tool alike.
- Decision (user, 2026-11-07): while dragging, the view draws the dragged node and its connections at the preview position and not at the old place, as if the node moved live (as in Java); the network itself changes only on release.

## Boundaries & Constraints

**Always:** the network changes only on release, through `Document::apply`; the node keeps its y (Java moves x/z only); the drag claims the left button only when it starts on a node, otherwise a left drag pans (story 3.1 decision); the frame-time test still passes.

**Never:** no selection or multi-node move (epic-selection-clipboard); no subdivision snap (Java's D toggle); no persisted snap setting; no overlap check after the move (epic-scans-autosave); no axis widget.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Plain drag | move tool, left drag starting on a node, snap off | node previewed under the cursor offset as grabbed; release commits one MoveNodes (x, z changed, y kept) | none |
| Snapped drag | snap on (or off with Shift held) | target x/z rounded to the nearest multiple of the map's grid spacing | none |
| Shift inverts | snap on and Shift held | target follows the cursor unsnapped | none |
| Drag on empty map | left drag not starting on a node | the map pans; nothing commits | none |
| No movement | press and release on a node without moving, or a drag that ends at the start position | nothing commits (no history entry) | none |
| Out of bounds | cursor beyond ±1024·map scale | the target stops at its last in-bounds position, as Java stops movement | none |
| Tool switch mid-drag | M/N or a toolbar button while dragging | preview dropped, network unchanged | none |
| Undo | Ctrl+Z after a drag | node back at the exact start position | none |
| Read-only | legacy config | move tool disabled | History rejects too |
| Node tool snap | node tool, snap on | the new node lands on the nearest grid point | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/mod.rs` -- extend `Tool` with drag hooks: `on_drag_start(&mut self, ctx, hovered: Option<NodeId>) -> bool` (true claims the drag), `on_drag(&mut self, ctx)`, `on_drag_end(&mut self, ctx, net) -> Option<Transaction>`; defaults return false/None so `NodeTool` is unchanged. `ToolContext` gains `snap: bool` (already resolved with Shift) and `&Network` where needed.
- `crates/ad-app/src/tools/move_node.rs` (new) -- `MoveTool { drag: Option<Drag { id, before: Position, grab_offset: (f64, f64), target: (f64, f64) }> }`; snap and bounds helpers as pure functions with unit tests (matrix rows Plain, Snapped, Shift, Out of bounds, No movement).
- `crates/ad-app/src/tools/node.rs` -- apply the same snap helper to the click position.
- `crates/ad-app/src/view.rs:185-254` -- `show` must (a) not pan a primary drag the tool claimed, (b) report drag start/move/end in map px with the hovered node at drag start, and (c) take an optional position override `(NodeId, x, z)` and draw that node and its connections there instead of at the stored position (Intent decision). The node may be off-screen at its stored place but on-screen at the preview, so include it in the visible set explicitly. Keep the batched mesh path; story 3.3 measures it.
- `crates/ad-app/src/input.rs` -- table entries: M → MoveTool, S → toggle snap; Shift state read each frame for inversion.
- `crates/ad-app/src/ui.rs` -- toolbar: "Move" tool button and a "Snap" checkbox; map panel routes drag events to the tool and applies the transaction.
- `crates/ad-app/src/main.rs` -- `App.snap: bool` (session only); `set_tool` already cancels.
- `crates/ad-core/src/command.rs` `MoveNodes` (`Vec<(NodeId, before, after)>`) -- reuse, no change.
- `crates/ad-core/src/map_context.rs` `MapContext.grid`, `MapGeometry.scale` -- grid spacing and bounds.
- `crates/ad-io/tests/golden_edited.rs` -- add a `MoveNodes` of one existing node to the edited transaction; the reload check covers it.
- Do not touch: hover rules (`hover.rs`), drawing look (`look.rs`), `jobs.rs`.

## Tasks & Acceptance

**Execution:**
- [x] `tools/mod.rs`, `tools/move_node.rs` -- drag hooks and move tool with pure snap/bounds helpers and tests for every matrix row that needs no UI
- [x] `tools/node.rs` -- snapped click position; test
- [x] `view.rs` -- drag claim, drag events, preview drawing; tests: a claimed drag does not pan, an unclaimed drag pans, a drag starting off a node is not offered as claimed
- [x] `input.rs`, `ui.rs`, `main.rs` -- M, S, Shift, toolbar; tests: tool switch mid-drag leaves the network unchanged, a drag commits one history entry and undo restores the start
- [x] `golden_edited.rs` -- a moved node round-trips

**Acceptance Criteria:**
- Given the fixture, when the user drags a node with snap off and with snap on, then it lands under the cursor or on the nearest grid point, and one Ctrl+Z puts it back.
- Given the frame-time test, when run in release, then every case stays under 16 ms.

## Design Notes

Java (MoveWidget.java:147-230): the move offset is clamped to ±1024·mapScale and movement stops once the cursor leaves that range; snap rounds the anchor's target to `gridSpacing` multiples (`Math.round(pos / grid) * grid`). Java rounds the X step to 3 and the Z step to 5 decimals and checks Z bounds with the X bounds; both are defects not ported (AD-13: world stays f64, rounded only at write). Java's move snap ignores Shift; the epic decided Shift inverts snap for every tool.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

**Manual checks (if no CLI):**
- The user drags a fixture node, snapped and unsnapped, and undoes it (hitl).

## Implementation Notes

- **Claim in the same frame.** `MapView::show` now takes `&mut impl ViewHooks` in place of the overlay closure. The trait has three default methods: `drag(DragEvent) -> bool`, `preview()` and `paint()`. A closure for the claim and a second one for the overlay would both borrow the tool, so they cannot exist together. With the trait, the claim is decided before this frame pans and the preview before this frame paints. There is no frame of lag. `()` is the no-op hooks. `ui::ToolHooks` adapts the active tool. It converts map px to world, builds the `ToolContext` and keeps the transaction of a released drag in `commit`. `map_panel` applies that transaction after `show` through `Document::apply`.
- **Drag events.** On `drag_started_by(Primary)` the view hit-tests the *press origin* with `hover::node_at`. It offers `Start { at, node }` at that position, not at the point where egui recognised a drag. A claimed drag reports `Move` while dragged and `End` on `drag_stopped`, and never pans. A middle drag always pans and is never offered. `reset()` clears the claim.
- **Trait.** `on_drag_end(&mut self, ctx)` takes no separate `net`, because `ToolContext` now carries `net` and `snap`. `Tool::preview() -> Option<(NodeId, x, z)>` feeds the view's position override. `MoveTool::on_drag_end` first applies the release position, then commits only if the position changed.
- **Snap.** `tools::snap_to_grid` uses `floor(v / g + 0.5) * g`, which is Java's `Math.round`: halves round up, also for negative values. `input::snap` resolves the toggle against Shift once per frame. Clicks and drags both receive the resolved value. `drag_target` works per axis. It clamps the pointer plus the grab offset to ±1024·scale, snaps and clamps again. If the pointer itself is outside the bounds on an axis, that axis keeps its last target, as Java does.
- **Preview drawing.** The batched edge mesh skips edges that touch the previewed node. Those edges come from a point query at the node's stored position (every edge of the node ends there, whether or not it is in view) and are drawn with that end at the preview. The previewed node's dot is drawn outside the culled set. Its height-warning icon, marker icon and name, and the info box also follow the preview, so nothing is left behind at the old place. The edge mesh code moved unchanged into a `push_edge` closure.
- **Toolbar.** A "Move" button sits after the two node buttons, and a "Snap" checkbox after it (tooltip "S; hold Shift to invert"). M selects a fresh move tool, which cancels a drag in progress. S toggles snap and works without a document. Both match their modifiers exactly, so Ctrl+S stays free.
- **Tests.** move_node: plain drag, snapped drag, preview, drag off a node, drag back to start, cancel, out of bounds (unit and through a drag). node: snapped click. view: claimed drag does not pan and reports Start/Move/End, unclaimed drag pans, drag off a node is offered with `node: None`, middle drag never offered, preview draws node and connection at the preview when both stored ends are out of view. input: M, S, Shift inversion. main: drag commits one undo step and undo restores the start, snapped drag lands on the grid, tool switch (M or N) mid-drag leaves the network unchanged, press and release commits nothing, S toggles. golden_edited: a `MoveNodes` of an existing node to (100.25, y, -200.5) reloads at that position.
- **Frame-time (release).** Every case is under 16 ms. The worst max is 2.87 ms at minimum zoom in the best round, and 6.76 ms in a single round.

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 1 medium and 4 low patched; 1 false rejected.
  - medium, patch: Ctrl+Z / Ctrl+Shift+Z during a claimed drag changed the network under it, so the release committed a MoveNodes with a stale `before` or on a removed node (UnknownNode). Undo and Redo now cancel the tool's preview first; test `undo_or_redo_mid_drag_drops_the_drag`.
  - low, patch: a drag stopped without a pointer position sent neither End nor cancel, leaving a stuck preview. The view now sends `DragEvent::Cancel`; untested, egui offers no way to produce that event in a test.
  - low, patch: snap-then-clamp put a snapped target at the map edge off the grid (1023.9, grid 5 → 1024). It now takes the nearest in-bounds grid multiple (1020); test expectation corrected.
  - low, patch: the node tool drew its preview at the raw cursor while the click landed on the snapped point. `Tool::paint` gets the resolved snap grid; test `the_snapped_preview_sits_where_the_click_lands`.
  - low, patch: hover was hit-tested at stored positions during a drag, so other nodes or the dragged node's old spot showed the ring and info box. No hover while a preview is active; test `nothing_is_hovered_while_a_preview_is_shown`.
  - false: "the node tool snap is not clamped to the map bounds". The frozen matrix keeps Java's behaviour: a click outside the map adds the node at the clicked position.
- Verification re-run after the patches: fmt, clippy, workspace tests (ad-app 98) and the release frame-time test (worst max 4.3 ms) pass.
