---
title: 'The delete tool removes the node or connection under the cursor'
type: 'feature'
ticket: '4'
created: '2026-11-07'
status: 'in-review'
baseline_revision: '65daebe026896c27dd3722c5670c6e23947aa62f'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/initiative-rust-rewrite/tickets/epic-node-connection-tools/tickets/story-dragging-a-node-moves-it-with-grid-snap-in-one-undo-step-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Nodes and connections cannot be removed. Users need a delete tool for the node or connection under the cursor, and the Delete key for the hovered node, each undone in one step.

**Approach:** A delete tool deletes the clicked node through `DeleteNodes`, or, when no node is under the cursor, removes the clicked connection through `Disconnect`. The Delete key deletes the hovered node whichever tool is active, as in Java. This story settles the edge hit-test rule that story 3.7 (flip) reuses:
- A node under the cursor wins: no connection is hit while a node is hovered.
- A connection is hit only on its drawn part: the segment shortened by half the node size at each end, and only when it is drawn at all (screen length of at least `MIN_EDGE_PX`).
- The delete tool highlights the connection it would remove.

## Boundaries & Constraints

**Always:** every change goes through `Document::apply` (AD-4); a delete or Delete key cancels any active tool preview first, as Undo does; read-only documents disable the tool and the key.

**Never:** no multi-node delete (epic-selection-clipboard); no marker deletion as its own action (epic-markers-parking; a deleted node takes its marker with it, as `DeleteNodes` already does); no change to `Network` or `DeleteNodes`/`Disconnect` semantics.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Delete node | delete tool, click on a node | node, its links and its marker removed; one history entry; undo restores id, order position, links and marker | none |
| Delete connection | delete tool, click on a drawn connection, no node under the cursor | that link removed in both directions (`Disconnect`), nodes kept; undo restores it with its shape | none |
| Node wins | cursor over a node that a connection also passes | the node is hit, not the connection | none |
| Hidden end | cursor on the part of a connection covered by its end node | the node is hit | none |
| Too short | connection drawn shorter than `MIN_EDGE_PX` | not hit | none |
| Empty click | click on empty map | nothing | none |
| Delete key | Delete with a hovered node, any tool | that node deleted; any tool preview cancelled first | nothing without a hovered node |
| Read-only | legacy config | tool and key do nothing | History rejects too |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/hover.rs:60-95` `edge_at` -- apply the rule: return `None` when `node_at` found a node (caller passes it), shorten each segment by `ns/2` screen px (converted to world) at both ends, skip segments whose screen length is below `MIN_EDGE_PX` (`lod.rs:6`). Unit tests per matrix rows Node wins, Hidden end, Too short.
- `crates/ad-app/src/view.rs` `show` -- compute `hovered_edge` with the new rule (after `hovered_node`), drop the `#[allow(dead_code)]` on `hovered_edge` (view.rs ~146) once read; pass the hovered node and edge to the tools (`ToolContext` or the hooks).
- `crates/ad-app/src/tools/delete.rs` (new) -- `DeleteTool`: `on_click` returns `DeleteNodes(vec![node])` when a node is hovered, else `Disconnect { a, b }` for the hovered edge, else `None`; `paint` draws the hovered edge highlighted (a thicker line in the warning colour) when no node is hovered.
- `crates/ad-app/src/tools/mod.rs` -- `ToolContext` gains `hovered_node: Option<NodeId>` and `hovered_edge: Option<(NodeId, NodeId)>`; `paint` needs the hovered edge and the network for the highlight (extend minimally).
- `crates/ad-app/src/input.rs` -- `Action::DeleteTool` has no Java default key (Java's Delete key is the node action); add `Action::DeleteHovered` on `Key::Delete`.
- `crates/ad-app/src/main.rs` `App::run` -- `DeleteHovered` if editable and `view.hovered_node` is some: cancel the tool, then `doc.apply(DeleteNodes)`.
- `crates/ad-app/src/ui.rs` -- toolbar button "Delete".
- `crates/ad-core/src/command.rs` `DeleteNodes`, `Disconnect` -- reuse unchanged.
- `crates/ad-io/tests/golden_edited.rs` -- already deletes nodes and has `Disconnect`? Add one `Disconnect` of an existing link if none is there, so a saved connection removal reloads identically.
- Deferred entry this closes: deferred-work.md "Decide in epic-node-connection-tools whether `hovered_edge` should test the drawn segment…".

## Tasks & Acceptance

**Execution:**
- [x] `hover.rs` -- edge rule with tests
- [x] `tools/mod.rs`, `tools/delete.rs` -- delete tool with tests for Delete node, Delete connection, Empty click
- [x] `view.rs`, `ui.rs`, `input.rs`, `main.rs` -- wiring, Delete key, toolbar; tests: Delete key deletes the hovered node and undo restores it, read-only does nothing, Delete key mid-drag cancels the drag
- [x] `golden_edited.rs` -- connection removal round-trips
- [x] record the edge rule in Implementation Notes

**Acceptance Criteria:**
- Given the fixture, when the user deletes a node and a connection with the delete tool and presses Ctrl+Z twice, then both come back exactly.
- Given the frame-time test, when run in release, then every case stays under 16 ms.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Edge hit-test rule (story 3.7 reuses it).** `hover::edge_at(net, geometry, t, ns, hovered, cursor)` returns `None` when `hovered` (the result of `node_at`) is set. Otherwise it tests each candidate against its *drawn* screen segment, `hover::drawn_segment`. That function is `look::line_ends` on the two node centres: each end is pulled in by `ns / 2`, and the result is `None` below `MIN_EDGE_PX`. So it is exactly what the view draws. A connection is hit when the cursor projects onto that segment (parameter in [0, 1], no end caps) within `EDGE_HIT_PX` (inclusive) of it. The nearest one wins, and ties go to the lowest `(from, to)`. `edges_near` on the full world segment still preselects candidates; it is a superset because the drawn part lies inside the full segment. When nodes are too small to hit (`ns < 2`), the segment is still shortened by `ns / 2`. A dual link hovers as either direction; `Disconnect` removes both.
- **Hover to tools.** `ToolContext` gained `hovered_node` and `hovered_edge`. `App::click` fills them from the view's hover of the same frame, and drag contexts leave them `None` (a drag gets its node in `on_drag_start`). `Tool::paint` and `ViewHooks::paint` gained `hovered_edge: Option<(Pos2, Pos2)>`. It is the drawn screen segment, which the view computes, so tools need neither the network nor `ns` to paint it. `DeleteTool::paint` draws it 3 px wide in `visuals.warn_fg_color`. The delete tool claims no drags, so a left drag still pans.
- **Delete key.** `Action::DeleteHovered` sits on Delete with no modifiers; the delete tool has no key. `App::delete_hovered` uses `view.hovered_node` only when the view last showed this document (`view_epoch == doc.epoch`) and the node still exists. A just-loaded document, or an Undo in the same frame, therefore cannot delete a stale id. It cancels the tool's preview first, then applies `DeleteNodes`. In the running app no node is hovered while a move preview is shown (story 3.2), so Delete does nothing mid-drag. The mid-drag test sets the hover by hand to cover the cancel path.
- **Toolbar.** A "Delete" button sits after "Move"; like the other tools, it is disabled for read-only documents.
- **Tests.** hover: node wins, hidden end, only the drawn part (large ns), too short (and hit once zoomed in). delete: node with its links and marker (revert restores the snapshot), node wins over edge, connection both ways with nodes kept (revert restores Dual), empty click. input: Delete, Ctrl+Delete does nothing. main: Delete key and undo, a hover from another document is ignored, read-only ignores key and tool, Delete mid-drag cancels the drag, the delete tool removes a connection and a node and two undos restore the snapshot (acceptance 1). golden_edited: a `Disconnect` of an untouched fixture link reloads with no link either way.
- **Frame-time (release).** Every case is under 16 ms. The worst max is 3.05 ms in the best round and 5.56 ms in a single round.
- Settles deferred-work.md entry "Decide in epic-node-connection-tools whether `hovered_edge` should test the drawn segment…"; the entry stays (the file is append-only).

## Plan Change Log

## Review Triage Log
