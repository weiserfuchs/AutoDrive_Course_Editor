---
title: 'The move tool drags the whole selection in one undo step'
type: 'feature'
ticket: '2'
created: '2026-11-07'
status: 'built'
baseline_revision: '0e3bb8ed42c4f485072d97faf06132766ae6ac60'
route: 'full'
route_source: 'auto'
risk: 'high'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/initiative-rust-rewrite/tickets/epic-node-connection-tools/tickets/story-dragging-a-node-moves-it-with-grid-snap-in-one-undo-step-plan.md'
  - '{project-root}/_bmad-output/initiative-rust-rewrite/tickets/epic-selection-clipboard/tickets/story-a-right-drag-rectangle-selects-nodes-and-the-selection-survi-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The move tool drags one node only. Users need to move a whole selection at once and undo it in one step.

**Approach:** A move drag that starts on a selected node moves every selected node by the same offset. Grid snap applies to the grabbed node; the shared offset is clamped so no selected node leaves ±1024·map scale, so the group keeps its shape. Release commits one `MoveNodes` with absolute before and after positions (y kept). The view's single-node preview override becomes a set of moved nodes, drawn with their connections at the preview, as the single node is today; story 4.6 (rotate) reuses that set. A drag that starts on an unselected node moves that node alone, exactly as before.

## Boundaries & Constraints

**Always:** the network changes only on release, through `Document::apply`; the selection is unchanged by a move; Undo/Redo and tool switches cancel a running drag (existing path); the frame-time test still passes, the single-node drag cases no slower.

**Never:** no rotate (story 4.6); no frame-time case for the whole network selected (story 4.3 adds it); no change to how a selection is made.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Group drag | three nodes selected, drag starts on one of them | all three previewed at the same offset; release commits one MoveNodes of all three | none |
| Snap | snap on, group drag | the grabbed node lands on the grid; the others keep their offsets to it | none |
| Edge clamp | a selected node would pass ±1024·scale | the offset stops so that node sits at the edge; the group keeps its shape | none |
| Unselected start | drag starts on a node outside the selection | only that node moves (story 3.2 behaviour) | none |
| No movement | release at the start position | nothing committed | none |
| Undo | Ctrl+Z after a group move | every node back at its exact start position; selection unchanged | none |
| Preview drawing | group drag | moved nodes and every connection touching them drawn at the preview, nothing left at the old places; a moved node that enters the view from outside is drawn | none |
| Read-only | legacy config | move tool disabled (existing) | History rejects too |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/move_node.rs` -- `Drag` becomes a group: grabbed id, grab offset, and the `(id, before)` list of moved nodes (the selection when the start node is selected, else the start node alone). `drag_target` keeps computing the grabbed node's target; derive the shared offset from it, then clamp the offset by the group's extremes (min/max x and z of the befores) against ±half. The single-node path must give identical results (existing tests stay green). `on_drag_end` commits one `MoveNodes` of all moved nodes.
- `crates/ad-app/src/tools/mod.rs` -- `ToolContext` needs the selection (`&BTreeSet<NodeId>`); `Tool::preview` changes from `Option<(NodeId, f64, f64)>` to something that serves many nodes with O(1) lookup, e.g. `fn preview(&self) -> Option<&Preview>` where `Preview` maps `NodeId -> (x, z)` (a `HashMap`, updated on drag, not rebuilt per frame). Keep it general enough for rotate (per-node positions, not only an offset).
- `crates/ad-app/src/view.rs` -- `ViewHooks::preview`, `place()` (~203), the paint path (~460-730) and `preview_edges` (~215, 623-630) work on the set: every edge with an end in the set is drawn from placed positions and skipped in the batched pass; moved nodes are drawn even when their stored position is out of view (today only the single previewed node is). Hover stays off while a preview is active (story 3.2 rule).
- `crates/ad-app/src/ui.rs` `ToolHooks` -- pass the selection into `ToolContext`; forward the new preview type.
- `crates/ad-core/src/command.rs` `MoveNodes` -- reuse unchanged.
- `crates/ad-io/tests/golden_edited.rs` -- add one group `MoveNodes` (two or three untouched nodes) and check them after reload.
- Do not touch: selection input (`ui::select`, story 4.1), hover rules, `ad-core`.

## Tasks & Acceptance

**Execution:**
- [x] `tools/move_node.rs` -- group drag with offset clamp; tests for Group drag, Snap, Edge clamp, Unselected start, No movement, Undo
- [x] `tools/mod.rs`, `ui.rs` -- selection in the context, preview set type; adapt the other tools' signatures only as needed
- [x] `view.rs` -- draw the preview set; tests: moved nodes and their connections at the preview, none at the old place, a node entering the view from outside drawn, hover off during the preview
- [x] `golden_edited.rs` -- group move round-trip

**Acceptance Criteria:**
- Given the fixture with a selected road section, when the user drags one of its nodes with the move tool and releases, then the whole section moves together and one Ctrl+Z puts it back.
- Given the frame-time test, when run in release, then every existing case stays under 16 ms and the single-node drag cases are no slower than before.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

**Manual checks (if no CLI):**
- The user selects part of the fixture, drags it with the move tool (snap on and off), and undoes it.

## Implementation Notes

- **Preview type.** `view::Preview` is a `HashMap<NodeId, (f64, f64)>` of absolute world positions, so rotate (story 4.6) can fill it the same way. `Tool::preview` and `ViewHooks::preview` return `Option<&Preview>`. The tool updates it in place on every drag event; the view never rebuilds it.
- **Hasher.** With SipHash, the two lookups per drawn connection made the single-node "drag: min zoom" case about 15 % slower (median 2.6 → 3.0 ms). `Preview` uses `IdHasher`, an FxHash-style multiply in `view.rs`. With it, that case is 2.4 ms against a 2.6 ms baseline.
- **Group drag.** `Drag` holds the grabbed id, its origin, the grab offset, its target, the `(id, before)` list and the preview. `drag_target` still computes the grabbed node's target: grab offset, ±half clamp, snap, and the last value on an axis whose pointer is outside the map. Then the target is clamped to `range`, which is ±half narrowed by the group's extent around the grabbed node. Every other node gets `before + (target − origin)`, and the grabbed node gets the target exactly, so a snapped grab lands on the grid. With one node the range is ±half, so the result matches story 3.2 bit for bit. The clamp uses `max`/`min`, not `clamp`, so a group wider than the map (bad data) cannot panic. When the edge clamp applies with snap on, the edge wins and the grabbed node may sit off the grid (matrix row Edge clamp).
- **Commit.** Release commits one `MoveNodes` of every moved node (y kept), but only if some node changed.
- **Context.** `ToolContext.selection: &BTreeSet<NodeId>` comes from `ToolHooks` (drags) and from `App::click` (clicks, `&doc.selection`). The other tools ignore it. In their tests it is an empty set.
- **View.** Two passes change. The batched edge pass skips every edge with an end in the preview. For each previewed node, the point query at its stored position draws its edges with both ends placed. An edge between two previewed nodes is drawn once, from its `from` end. Every previewed node's dot (and ring if selected) is drawn outside the culled set. Hover stays off while a preview is shown (unchanged).
- **Tests.** move_node covers group drag with one offset, snapped group, edge clamp (right, left, and z with snap), unselected start, back to start, and undo of a group move. view covers a previewed group drawn only at the preview: each connection once, nothing left at the old row, and a node entering from outside drawn. main covers a group drag through `ToolHooks` that undoes in one step and leaves the selection unchanged. golden_edited adds a three-node `MoveNodes` that reloads at its positions.
- **Frame time (release, this machine).** Every case is under 16 ms. Best-round max values: drag min zoom 2.5–2.8 ms (baseline 3.5), drag max zoom 0.07–0.13 ms (baseline 0.08), static min zoom all selected 5.8–6.2 ms.

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 1 medium patched; 1 false rejected.
  - medium, patch: with a selected node stored outside ±half (the fixture has one at x = -9626), the group's offset range excluded 0, so the first drag event shifted the group without pointer movement and a release at the start committed a MoveNodes. The range now always contains the start position per axis; a node outside can only move back toward the map. Test `a_node_stored_past_the_edge_never_moves_the_group_on_its_own`.
  - false: "AC 1 has no automated test on the fixture". It is the user's hand check; the app test covers the tool path and golden_edited the saved group move.
- Verification re-run after the patch: fmt, clippy, workspace tests and the release frame-time test pass.
