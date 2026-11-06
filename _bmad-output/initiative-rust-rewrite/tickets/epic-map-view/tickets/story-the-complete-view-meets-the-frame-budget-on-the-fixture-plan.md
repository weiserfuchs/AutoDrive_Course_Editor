---
title: 'The complete view meets the frame budget on the fixture'
type: 'feature'
ticket: '7'
created: '2026-11-06'
status: 'built'
baseline_revision: '44d21be8f585e1d07943917c4012ddb0aba78693'
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

**Problem:** R3 requires every frame to stay within 16 ms on the fixture, at any zoom, while panning and zooming. The frame-time test so far times static views, one run each. On this noisy machine single runs swing between 7 and 20 ms, so they cannot show the budget either way. The height-warning pass also walks every visible node at minimum zoom, where no node is drawn (deferred from story 2.5).

**Approach:**
- Make the frame-time test measure the complete view: map image, grid, coloured connections with arrows, nodes, icons, names and a hovered node. It covers static views and moving ones (panning and zooming each frame) at minimum, middle and maximum zoom.
- Report robust numbers: the best of several rounds.
- Remove the per-frame walk over all nodes at minimum zoom by keeping the nodes without height in the network.

## Boundaries & Constraints

**Always:**
- **Network** keeps the set of node ids whose `y == -1.0` (no height), in sync through every mutation: `from_loaded`, `add_node`, `move_node`, `remove_node`, `reinsert`. It exposes them read-only. A randomised test checks the set against a full scan after random operations.
- **View:**
  - The node rectangle query runs only when nodes are drawn (`ns >= MIN_NODE_PX`).
  - Height-warning icons come from the no-height set, filtered by the padded visible rectangle.
  - The drawn result is unchanged: same icons and same nodes in every existing view test.
- **Frame-time test cases,** with all overlays on (icons, names, grid where zoom > 2):
  - static: minimum zoom; mid zoom dense; maximum zoom dense, with a hovered node;
  - moving: panning by 40 px per frame at minimum zoom and at mid zoom dense; zooming one wheel step per frame in and out between minimum and maximum zoom about a dense point.
  - Each case runs 3 rounds of 30 timed frames after a warm-up and prints every round's median and max, plus the best round. The budget verdict uses the best round, which filters out outside load. The test stays `#[ignore]` and prints only; no timing assertion runs in CI.
- **Budget:** every case's best-round median and max are under 16 ms on the developer machine. If one is not, fix it in the view or `ad-core` within the epic's rules (no rendering threads, reused buffers), and record what changed.

**Never:**
- Change what is drawn or how it looks.
- Add rendering threads, frame skipping or caching of drawn output across frames (AD-8).
- Treat the user's hands-on smoothness check as replaced. It stays open as the epic's hitl check.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| No-height set | load with y = −1 nodes; add with y = −1; move a node to y = −1 and back; remove and reinsert | the set always equals a full scan | none |
| Min-zoom warnings | a node with y = −1 in view at minimum zoom | its warning icon drawn; no node query needed | none |
| View unchanged | the existing view tests | still pass unchanged | none |
| Budget | the extended frame-time test in release | every case's best-round median and max under 16 ms | if not, optimise and record |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/network.rs`:
  - `Network` (≈l.186) holds `nodes`, `order`, `markers`, the indexes and `next_id`. Add a `no_height` set (`BTreeSet<NodeId>` or a sorted `Vec`).
  - Maintain it in `from_loaded` (≈l.202), `add_node` (≈l.252), `move_node` (≈l.302), `remove_node` (≈l.481) and `reinsert` (≈l.533).
  - Add `pub fn nodes_without_height(&self) -> impl Iterator<Item = NodeId> + '_`.
  - The randomised test sits beside `cached_looks_follow_every_mutation` and reuses its operation generator.
- `crates/ad-app/src/view.rs`:
  - `paint` (≈l.400-460): move `nodes_in_rect_into` inside the `ns >= MIN_NODE_PX` branch.
  - Build the warning-icon mesh from `net.nodes_without_height()`, filtered by the padded rectangle.
  - `overlay::has_height_warning` stays the definition of "no height". Use it in ad-core's test only as the reference, or mirror the `== -1.0` rule with a comment.
- `crates/ad-app/src/view.rs` frame-time test (≈l.1100-1190):
  - `run(name, steps, at, grid, hover)` sets up a static case. Add moving cases: change `transform.center` or call `zoom_at` before each frame.
  - Run 3 rounds per case and print per-round and best-round numbers.
  - Keep the existing static cases.

## Tasks & Acceptance

**Execution:**
- [ ] `crates/ad-core/src/network.rs` -- the no-height set, its accessor, and the randomised test (the "No-height set" row).
- [ ] `crates/ad-app/src/view.rs` -- node query only when nodes are drawn, warnings from the set; a test for the "Min-zoom warnings" row; the existing tests stay green.
- [ ] `crates/ad-app/src/view.rs` frame-time test -- the moving cases and 3 rounds with best-round reporting; record the results in Implementation Notes and fix any case over budget.

**Acceptance Criteria:**
- Given the extended frame-time test in release, when it runs on the developer machine, then every case's best-round median and max are under 16 ms, and the numbers are recorded.
- Given the fixture opened, then the view looks exactly as before this story.

## Implementation Notes

Network keeps `no_height: BTreeSet<NodeId>` (from_loaded, add_node, move_node, remove_node, reinsert); `nodes_without_height()` exposes it. The view queries nodes only when `ns >= MIN_NODE_PX`; warning icons come from the set, filtered by the padded rectangle. Warning icons are now added in id order instead of R-tree order (same set).

Frame-time results (release, 35652 nodes, 1920x1080; overlays on, grid on except where marked; 3 rounds of 30 frames; best round = lowest max; median / max ms):
- static min zoom: 8.865 / 9.411
- static mid zoom dense: 2.166 / 2.498
- static max zoom centre (grid off): 0.109 / 0.117
- static max zoom dense (grid off): 0.163 / 0.194
- static max zoom dense, hovered: 0.172 / 0.190
- pan min zoom (vertical, 40 px/frame; width is pinned at min zoom): 7.335 / 8.519
- pan mid zoom dense (horizontal): 1.724 / 2.457
- zoom in/out about the dense point's screen position: 2.565 / 11.085

All under 16 ms; no optimisation was needed. Single rounds still spike on this machine (up to 16.9 ms max in one min-zoom round), which the best-round rule filters out.

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 1 medium and 5 low patched.
  - medium, patch: at 1920×1080 the minimum zoom shows the full map width, so the horizontal pan at minimum zoom was clamped away and the case timed a static frame. It now pans vertically by 40 px per frame.
  - low, patch: the zoom case drifted to the panel centre once clamped at minimum zoom. Each step now zooms at the dense point's current screen position.
  - low, patch: the static cases "max zoom, centre" and "max zoom, dense" (grid off) had been dropped against the Code Map. They are restored.
  - low, patch: the best round was picked by median but judged by its max. It is now the round with the lowest max, stated in the output.
  - low, patch: `height_warning_shows_where_no_node_is_drawn` would also pass with the old code. It now also asserts that no node circle is drawn and checks the icon position.
  - low, patch: the no-height randomised test reinserted every removed node. Half now stay removed and checks continue; a comment notes the mirrored `y == -1.0` rule.
- Orchestrator verification: in extra runs the zoom case's maximum varied from 10 to 25 ms, while the other cases stayed stable. Per-frame timing showed no frame index that repeatedly ran slow; the spikes are outside load. After the patches, every case's best round is under 16 ms (minimum zoom 9.0/11.1 ms median/max; zoom case 3.2/14.5 ms).

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: no diff
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: no warnings
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case's best-round median and max under 16 ms; numbers recorded

**Manual checks (if no CLI):**
- The user pans and zooms the fixture at minimum and maximum zoom, with the grid on and while hovering, and confirms it stays smooth (the epic's open hitl check).
