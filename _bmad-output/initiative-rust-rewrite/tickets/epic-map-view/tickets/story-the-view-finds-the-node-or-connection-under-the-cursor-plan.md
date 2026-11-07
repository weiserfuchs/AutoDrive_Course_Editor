---
title: 'The view finds the node or connection under the cursor'
type: 'feature'
ticket: '9'
created: '2026-11-06'
status: done
baseline_revision: 'a6477e5c0e3bd47540fb615d6b1d702becec268a'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: ['_bmad-output/initiative-rust-rewrite/tickets/epic-map-view/epic-map-view.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The tools epic needs to know which node or connection is under the cursor, and a place to draw tool overlays above the network. Users need the Java hover feedback (highlighted node, marker group, info box) to see what they are about to act on.

**Approach:**
- Hit-test through ViewTransform, MapGeometry and the spatial index:
  - the hovered node with Java's screen-square rule, made deterministic;
  - the nearest connection within a pixel radius.
- Draw Java's hover feedback: the node on top with a white ring, the marker name with its group, and the info box.
- Add an overlay hook that runs after everything else in the view.

## Boundaries & Constraints

**Always:**
- **Node hit test** (Java `MapPanel.getNodeAtScreenPosition` 681-705):
  - A node is hit when `|cursor − node_screen| < ns/2` on both axes (strict), where `ns` is the odd node size from story 2.4.
  - Candidates come from a node-index rectangle query around the cursor, never a scan of all nodes.
  - When several nodes are hit, the closest to the cursor wins, then the lowest id (Java takes the first in file order; this is a deliberate fix, AD-13).
  - No hit when `ns < 2`, because nodes are not drawn then.
- **Connection hit test** (new; Java has none):
  - The nearest connection whose segment lies within `EDGE_HIT_PX` (4 screen px, in `lod.rs`) of the cursor, via the edge index (`edges_near` or equivalent). Ties go to the lowest `(from, to)`.
  - It is computed and stored on the view for the tools epic, and draws nothing in this story.
- **Hover state** is recomputed every frame from the current pointer position and the view. A pointer outside the panel, or no document, means nothing is hovered.
- **Hover visuals** (Java `NodeDrawThread` 93, 127-151; `MapPanel` 399-569):
  - The hovered node is drawn after all other nodes, in its flag colour, with a white ring: Java's fallback oval, stroke `0.1·ns`, diameter `ns`.
  - If it has a marker, its name is shown even when names are off, with `" ( group )"` appended.
  - **Info box,** only when `ns >= 2`. Its top-left is at the node's screen position + (`ns/2`, 0), clamped inside the panel. Rows, in order:
    - `"Node ID <n>"` (black on white), only while the session toggle "show node ID" is on. F4 switches it, default off. `<n>` is the node's 1-based file id: its position in the order + 1.
    - `"Node Y is invalid ( -1 )"` (black on red) when y == −1.
  - With no rows, no box is drawn.
  - Row geometry: one line of egui's default 12 pt font per row, padded on both sides, all rows the width of the widest, header rows centred.
- **Overlay hook (AD-10):** `MapView::show` takes a callback that receives the painter, the `ViewTransform` and the `MapGeometry`. It runs last, after the info box, so tools draw on top. `main.rs` passes a no-op for now.
- **Frame budget:** with a node hovered, the frame-time test stays under 16 ms on a quiet machine.

**Never:**
- Selection, clicking, dragging nodes, or any tool (tools and selection epics).
- The info box's park, overlap and debug-location rows (data and debug menu come later). The connection hit draws nothing.
- Scanning all nodes or edges per frame for the hit test.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Node hit | ns 11; cursor 5.4 px right of a node; 5.5 px right | hit; not hit (strict) | none |
| Overlap | two nodes 2 px apart, both within ns/2 of the cursor | the closer one; on equal distance the lower id | none |
| Small nodes | zoom where ns < 2 | no node hit | none |
| Zoom levels | the same layout at three zooms | the hit set follows `ns` at each zoom | none |
| Edge hit | cursor 3 px from a segment's middle; 5 px; two segments at 2 and 3 px | that edge; none; the 2 px one | none |
| No pointer | pointer outside the panel or no document | nothing hovered, no ring, no box | none |
| Info rows | toggle off and y = 5; toggle on; y = −1 | no box; "Node ID n"; the red invalid-Y row | none |
| Box clamp | a hovered node at the panel's right or bottom edge | the box lies fully inside the panel | none |
| Marker hover | hovered marker node, names off | "Name ( Group )" shown | none |
| Hook order | a callback that paints a shape | that shape is the last one painted in the frame | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/view.rs`:
  - `MapView::show(ui, net, geometry, node_size, map_texture, …)` handles input and calls `paint`.
  - `paint` draws the image, grid, connection mesh, nodes (circles of diameter `ns`, `look::node_colour`), icons and marker names, with culling through `nodes_in_rect_into` / `edges_in_rect_into`.
  - Add `hovered_node: Option<NodeId>` and `hovered_edge: Option<(NodeId, NodeId)>`, computed in `show` from `response.hover_pos()`.
  - The hovered node is skipped in the main node pass and drawn after it, then the info box, then the overlay callback.
- New `crates/ad-app/src/hover.rs`, pure functions with no painting, unit-tested:
  - `node_at(net, geometry, transform, ns, cursor) -> Option<NodeId>` uses `nodes_in_rect_into` with a reused buffer, or a small query;
  - `edge_at(...)` uses `Network::edges_near` in `crates/ad-core/src/network/spatial.rs`, which takes a world distance, so convert `EDGE_HIT_PX` with `/ zoom * scale`, then pick the nearest by exact segment distance;
  - `info_rows(node, file_id, show_id) -> Vec<Row>`;
  - `box_rect(anchor, size, panel) -> Rect`.
- `crates/ad-app/src/look.rs`: `node_px` and `node_colour`, reuse them. `crates/ad-app/src/overlay.rs`: the marker-name anchor, reuse it, and extend the name with the group when hovered.
- `crates/ad-app/src/lod.rs`: `EDGE_HIT_PX = 4.0`, the ring stroke factor `0.1`.
- `crates/ad-app/src/main.rs`: F4 toggles `show_node_id` for the session (`ctx.input(|i| i.key_pressed(egui::Key::F4))`); pass the no-op overlay callback.
- The file id of a node is its index in `net.order()` + 1, found only for the hovered node, so the cost is one lookup per frame.
- Java reference: `MapPanel.java` 399-569 and 681-705, `NodeDrawThread.java` 93 and 127-151, `RenderManager.java`.

## Tasks & Acceptance

**Execution:**
- [ ] `crates/ad-app/src/hover.rs` (new), `lod.rs` -- node and edge hit tests, info rows and box placement; tests for the "Node hit", "Overlap", "Small nodes", "Zoom levels", "Edge hit", "Info rows" and "Box clamp" rows.
- [ ] `crates/ad-app/src/view.rs`, `overlay.rs` -- hover state, hovered node on top with its ring, the marker name with its group, the info box, and the overlay callback last; tests for the "No pointer", "Marker hover" and "Hook order" rows; extend the frame-time test with a hovered node.
- [ ] `crates/ad-app/src/main.rs` -- the F4 toggle and the no-op hook.

**Acceptance Criteria:**
- Given the fixture opened and zoomed in, when the cursor rests on a node, then it is drawn on top with a white ring. On a marker node the name shows with its group. With F4 on, an info box shows "Node ID n" next to the node (hitl).
- Given the frame-time test on a quiet machine, then every case stays under 16 ms.

## Implementation Notes

- Implemented by a subagent; `hover.rs` is new, and `node_at` takes a reusable buffer. Info box row padding is 2 px.
- `hovered_edge` is written but not read yet, so it carries `#[allow(dead_code)]` until the tools epic.
- Frame time (orchestrator, release): min zoom 11.0–13.3 ms median; hovered node 0.2–1.8 ms.

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 2 low patched; 1 low deferred; 2 low rejected.
  - low, patch: with no document, `show` was not called, so the hover fields kept stale values. `MapView::clear_hover()` is called on the no-document path and from `reset()`; the test also covers `hovered_edge`.
  - low, patch: a frame-time test comment described the wrong pointer choice. It is corrected.
  - low, defer: `edge_at` tests the full centre-to-centre segment, not the drawn, shortened one, and a node hover also yields its edges. The tools epic decides; recorded in deferred-work.md.
  - low, rejected: the 20 px height-warning icon covers the hovered node's ring on y = −1 nodes. Java draws icons after nodes too, so this matches Java's order.
  - low, rejected: the file-id lookup scans `order()` while F4 is on. That is about 35k id comparisons, microseconds, and only for the hovered node.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: no diff
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: no warnings
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms on a quiet machine; numbers recorded

**Manual checks (if no CLI):**
- The user hovers nodes and marker nodes on the fixture and presses F4: the ring, the marker group and the info box appear as described.
