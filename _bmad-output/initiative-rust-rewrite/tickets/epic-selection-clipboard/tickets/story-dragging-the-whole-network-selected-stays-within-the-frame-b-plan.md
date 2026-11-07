---
title: 'Dragging the whole network selected stays within the frame budget'
type: 'feature'
ticket: '3'
created: '2026-11-08'
status: 'built'
baseline_revision: '5acae5990e74f606526140dfc77f93d07ddd3799'
route: 'full'
route_source: 'auto'
risk: 'high'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/initiative-rust-rewrite/tickets/epic-selection-clipboard/tickets/story-the-move-tool-drags-the-whole-selection-in-one-undo-step-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** R4 of epic-selection-clipboard requires every frame within 16 ms while a selection is dragged on the FS25 fixture, up to the whole network selected, at any zoom. Story 4.2 made the preview a set; it has not been measured with a large set.

**Approach:** Add frame-time cases that drag every fixture node inside the map (the one node far outside, x = -9626, pins the group's offset per the story 4.2 rule and would freeze the drag) with the move tool at minimum and maximum zoom, rings and preview drawn, asserting each frame that every selected node is previewed and the grabbed one follows the pointer. Fix whatever the measurement shows is over budget, without changing what is drawn.

## Boundaries & Constraints

**Always:** the drawn result is unchanged (moved nodes and every connection touching them at the preview, nothing at the old places, a moved node entering the view drawn); existing tests and frame-time cases stay green and no slower; AD-8 (UI thread, reused buffers, R-tree viewport queries).

**Never:** no change to tool semantics, selection or commands; no threads; no GPU instancing (architecture Deferred).

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/view.rs` frame-time test (end of `frame_time`) -- the two cases "drag: min zoom, whole network selected" and "drag: max zoom, whole network selected" are in place (written before the fix).
- `crates/ad-app/src/view.rs` paint path for the preview (`place()`, `preview_edges`, the loop that draws previewed nodes and their connections; ~460-760) -- today it runs one `edges_in_rect_into` per previewed node and draws every previewed node, whatever is in view. Cost scales with the selection size, not the view.
- `crates/ad-app/src/tools/move_node.rs` -- the tool updates its preview map on each drag event (outside the timed frame in the test, inside a real frame in the app); measure it too and keep it cheap.
- `crates/ad-core/src/network.rs` `Node::outgoing()`/`incoming()` -- adjacency per node without an R-tree query.

## Implementation Notes

- First measurement (release, 2026-11-08, before any fix): drag at min zoom, whole network selected: best-round median 45.2 ms, max 65.8 ms; at max zoom: median 51.1 ms, max 97.8 ms. Both over budget. The cost is about the same at both zooms, so it does not come from what is visible: the preview path does work per previewed node (≈35,000).
- Re-measured at the start of the fix (same machine, same code): min zoom median 26.1 ms, max 45.7 ms; max zoom median 39.0 ms, max 51.8 ms. Still over budget; the spread against the first numbers is machine noise.
- **Cause.** Two loops in `MapView::paint` ran over the whole preview, whatever was in view: (1) one R-tree point query (`edges_in_rect_into`) at each previewed node's stored position, ≈35,000 queries per frame, each pushing every found connection into the mesh, visible or not; (2) one `circle_filled` (and ring) per previewed node, ≈35,000 circle shapes per frame to tessellate, nearly all off screen at max zoom.
- **Fix (view.rs only).** (1) One edge query over the bounding box of the previewed nodes' stored positions replaces the per-node queries. Every connection of a previewed node has that stored position as an end, so the box finds all of them; each index entry comes back once, so a connection between two previewed nodes is drawn once without the old from-end rule. Entries with no previewed end are skipped. A connection is pushed only when the box of its placed segment meets the view rectangle, the same rule the R-tree applies in the batched pass. With one previewed node the box is its point, as before. (2) A previewed node's dot and ring are drawn only when its preview position lies in the padded rectangle the other nodes are queried in, so a node entering the view is drawn and one outside is not. Off-screen geometry was invisible, so what is drawn is unchanged; the existing preview tests pass unchanged.
- **Move tool.** `MoveTool::on_drag` with every fixture node inside the map selected took median 0.26 ms, max 0.74 ms per event (timed around the call in the test, then removed). It updates the preview in place and needs no change.
- **After the first fix (release, this machine, best round).** Whole network selected: min zoom median 7.7–8.1 ms, max 8.7–9.4 ms; max zoom median 2.2–2.3 ms, max 2.6–3.1 ms. The coordinator's runs of the same code gave min zoom median 9.6–11.6 ms, max 15.1–16.5 ms: two of four over budget, too little margin. Target set: best-round max ≤ 12 ms on repeated runs.
- **Second pass: where the min-zoom frame went** (timers in `paint`, then removed; this machine, ms per frame). Whole-network drag: bounding box over the preview 0.65, edge query 1.1 (55,291 entries: the stored box is the whole map, the view at minimum zoom only ≈55 % of it vertically, so it was the second query of two), edge loop with lookups and mesh building 1.9, selection marks 0.9, tessellation 1.8; total ≈6.5. The static "min zoom, all selected" frame: query 0.7 (36,490 entries), edge loop 1.0, marks 1.4, tessellation 1.9; total ≈5.2. So the drag's extra cost was the second, whole-map query and the lookups on edges outside the view, not missing level of detail: both passes already shared `push_edge` (the `MIN_EDGE_PX` skip in `look::line_ends`, the arrow rule in `look::arrow`) and one mesh. Measured against "static: min zoom" (no selection), most of the gap is the 35,000 selection marks, which the static all-selected case pays too.
- **Second fix (view.rs only).**
  - One walk over the preview gives the stored box and `shift`, the largest per-axis distance between a previewed node's stored and placed position. Every connection with a previewed end then has its stored box within the view widened by `shift`. The edge query takes the smallest of three: the view widened by `shift` (a group moved a little, the whole network at minimum zoom included); the joint box of view and stored box; or the view and the stored box as two queries (one node dragged far from where it is stored). With no preview it is the view, as before.
  - One loop over the result does two preview lookups per connection. Connections with no previewed end are drawn at their stored ends, connections with one at their placed ends, both under the batched pass's rule (the segment's box meets the view), so a wider query draws nothing extra. The two-query case keeps the separate pass over the stored box.
  - Selection marks take a previewed node's place from the preview without a node lookup, and the marks mesh reserves its capacity (this also helps "static: min zoom, all selected").
  - New test `a_preview_draws_exactly_what_the_moved_network_draws`: three previews, one per query choice (checked with a temporary print), each drawn over the stored network. The sorted mesh vertices (position and colour) and circle centres must equal those of the same network with the nodes really moved and no preview. Covers a node entering the view, a connection from an unpreviewed node outside the view, and a reverse link the target does not list.
- **After the second fix (release, this machine, 9 runs, best round).** Whole network selected, min zoom: median 5.7–6.1 ms, max 5.8–9.9 ms, one run at 11.7 ms (all three rounds of that run had outlier frames; "static: min zoom, all selected" reached 9.8 ms best-round max in another run). Max zoom: median 0.45–0.81 ms, max 0.49–1.09 ms (was 2.2 ms median). Static all selected: median 4.7–5.2 ms (was 5.3–5.5). Single-node drags unchanged: min zoom median 2.3–2.6 ms, max 2.5–3.5 ms; max zoom 0.07–0.08 ms. No case over budget in any run. The whole-network drag now costs ≈0.9 ms more than the static all-selected frame. The rest is edge and mark output that the static frame draws too.
- **Review fixes.**
  - The whole-network drag now uses the same pointer triangle as the single-node drags: ±320 px converted with the zoom, not ±10 world units. At min zoom that is about ±680 world units. The group's edge clamp (story 4.2) still holds the grabbed node to an x offset of −17.3 … +50.9 world units, because the in-map nodes span nearly the whole map. So the whole-network case still takes the widened query. The joint whole-map query can only arise for a large selection that is not the whole network dragged far at min zoom; no frame-time case covers that. The step asserts the grabbed node at `start + clamp(dx, range)`.
  - The view now gets the same `inside` selection as the tool for both whole-network cases (new `inside_selected`; `inside` moved up beside the other selections). Before, it got `every_node`, so the node at x = −9626 was drawn selected but not previewed.
  - Selection marks are back to `net.node` first and then `place`, so a preview id missing from the network gets no mark, as before. The marks mesh keeps its reserved capacity.
  - With no preview, the edge loop pushes every connection the view query returns, with no lookups or `in_view` check.
  - `a_preview_draws_exactly_what_the_moved_network_draws` now runs at zoom 10 (dots and rings) and zoom 1 (selection marks). Every node is selected, plus an id not in the network that is previewed in view. A sixth node f at x = 900 stays out of view at both zooms in two cases and moves into view in the third. Reverting the ring culling, or the `net.node`-first marks, makes the test fail (checked, then restored).
- **After the review fixes (release, this machine, 6 runs, best round).** Whole network, min zoom: median 5.9–6.9 ms, max 6.5–9.8 ms, one run at 11.8 ms (rounds of that run reached 16.1 and 23.1 ms; the static all-selected case had a 20.7 ms round in another run, so outlier frames are machine noise). Whole network, max zoom: median 0.47–0.81 ms, max 0.64–1.03 ms. Static all selected: median 4.7–5.5 ms. Single-node drags: min zoom median 2.4–2.9 ms, max 2.5–4.2 ms; max zoom 0.07–0.08 ms. No case over budget in any run. Restoring the node lookup in the marks loop costs about 0.3 ms against the second fix.
- **West half case.** New frame-time case "drag: min zoom, west half selected": every in-map node with x < 0 selected and dragged with the move tool by its lowest-id node, using the same ±320 px triangle; the view gets the tool's selection. The three group cases now share one `group_case` helper in `frame_time`, and `group_selected` (a `Cell<Option<&BTreeSet>>`) replaces `inside_selected`. The group may move −17.3 … +2048.1 world units in x, so it follows the pointer up to ≈ ±680 units.
  - **Query branch.** A temporary print showed this case takes the *widened* query, not the joint one: the shift is per axis, and a pure x drag widens only x, (4096 + 2·680) × 2240 ≈ 12.2 M against ≈ 16.7 M for the joint whole-map box. So the expectation that it reaches the joint branch did not hold.
  - **Joint branch measured.** To measure that branch anyway, the widened choice was disabled temporarily for previews of more than 1,000 nodes (3 runs, then restored). The west half then took 4.9–5.3 ms median and 5.5–6.2 ms max, and the whole network 6.6–7.2 ms median and 9.7–12.1 ms max. Even this worst case stays under budget, so no uniform-offset query was added.
  - **Numbers (release, this machine, 6 runs with the final code, best round).** West half: median 4.4–5.8 ms, max 4.9–6.3 ms. Whole network min zoom: median 6.0–7.9 ms, max 7.2–9.9 ms; max zoom: median 0.49–0.86 ms, max 0.52–1.09 ms. Single-node drags: min zoom 2.3–2.9 ms median, 2.5–3.1 ms max; max zoom 0.07–0.09 ms. No case over budget.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms, the two whole-network drag cases included

**Manual checks (if no CLI):**
- The user selects a large part of the fixture, drags it at both zoom ends and confirms it stays smooth.

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 1 medium and 3 low patched.
  - medium, patch: the whole-network drag moved only ±10 world units (≈ ±5 px at min zoom), so the measured frames took only the cheap widened query. The group drag now uses the ±320 px triangle converted with the zoom, like the single-node drag; a "west half selected" case (group free to move ~±680 world units) was added. Neither reaches the whole-map (joint) query, which needs a large selection dragged ~900+ world units on both axes at min zoom; forced on, it measured ≤ 12.1 ms max on the implementer's machine. Deferred (see deferred-work.md).
  - low, patch: the view drew the far-off node (x = -9626) as selected while the tool did not preview it; the view now gets the tool's selection in these cases.
  - low, patch: the equivalence test did not cover ring culling or the selection-mark path; it now runs at zoom 10 and zoom 1 with a selection, a node entering the view, and a preview id missing from the network (no mark, as before).
  - low, patch: static frames ran a per-edge preview lookup and in_view check with no preview; skipped when there is none.
- Final verification on the developer's machine (2 runs): fmt, clippy, workspace tests pass; frame_time every case under 16 ms (whole network min zoom max 7.3–8.2 ms, max zoom ≤ 1.0 ms, west half 5.6–5.8 ms).
