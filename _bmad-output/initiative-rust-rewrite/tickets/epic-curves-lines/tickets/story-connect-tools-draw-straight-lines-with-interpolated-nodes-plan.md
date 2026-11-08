---
title: 'Connect tools draw straight lines with interpolated nodes'
type: 'feature'
ticket: '2'
created: '2026-11-08'
status: 'planned'
baseline_revision: 'dd52d30'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Java's connect tools are line tools (`LinearLine`, `LinerLineBaseButton`): a long connection gets interpolated nodes, and optionally a click on empty map ends the line at a new node. Our connect tools (story 3.5) only link two nodes directly.

**Approach (as Java, `generatePreviewInterpolationPoints`, `commit`):**
- With a start node picked, the line runs from the start to the end point: the hovered node, or else the cursor (grid-snapped as the node tool snaps).
- `segments = round(distance / LinearLineNodeDistance)` in world units. If `segments > 1`, the line gets points at `i / segments` for `i = 1 .. segments - 1`, plus the end point itself when it is not a node.
- **Click on a node** with `segments > 1`: one `PasteNodes` of the interpolated nodes, linked start → nodes → end node with the tool's shape. With `segments <= 1`: today's connect behaviour (link, remove, reshape).
- **Click on empty map:** only when `CreateLinearLineEndNode` is on; then the line's points (end point included) become new nodes in one `PasteNodes`; with `segments <= 1` a single new end node linked to the start. When the option is off, a click on empty map does nothing, as today.
- Heights per AD-7: `sample_height` at each new node, or, without a heightmap, linear interpolation by point index from the start's y to the end node's y (the start's y for a new end point).
- New nodes are regular (Java's default `normalConnectionNodePriority`; per-tool priority comes with the settings UI).
- The preview draws the interpolated points and the segments between them in the tool's look, live with the cursor.
- Continuous mode: the end (node or new end node) starts the next line.
- `settings.toml` gains `line_node_distance` (default 12, at least 1) and `create_line_end_node` (default false) in `[global]`.
- **Unlike Java**, an interpolated point never reuses an existing node that happens to lie under it (Java's `getNodeAtWorldPosition`): a line only ever links its start and end to existing nodes. Recorded in AD-13.

## Boundaries & Constraints

**Always:** one transaction per click through `Document::apply`; read-only disables the tools (existing gates); switching tools or files cancels the pending line (AD-10); right click cancels, as today.

**Never:** no settings dialog (epic-settings-ui-shell); no curve; no change to the selection behaviour.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Long line | start and end node 60 apart, distance 12 | 4 new nodes at 12-unit steps, start → n0..n3 → end with the tool's shape; one undo | none |
| Short link | 15 apart (segments 1) | today's connect (link / remove / reshape) | none |
| Dual, reverse | dual / reverse tool on a long line | every segment dual / reverse | none |
| Heights | heightmap / none (start y 10, end y 20, 4 nodes) | sampled / 12, 14, 16, 18 | none |
| End node off | click on empty map | nothing | none |
| End node on | click on empty map 60 away / 5 away | 5 new nodes (last at the snapped cursor) / 1 new node, linked from the start | none |
| Continuous | continuous on, long line | the end starts the next line | none |
| Node under a point | an unrelated node under an interpolated point | a new node is created anyway; the unrelated node keeps its links | none |
| Cancel | tool switch, file switch, right click | no commit | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/connect.rs` -- pure `line_points(start, end, distance, end_is_node) -> Vec<(f64, f64)>` and `plan_line(...) -> Option<Transaction>` building `PasteNodes` with `LinkEnd::Existing` start/end (story 5.1); `on_click` picks line vs today's `plan`; preview painting of points and segments.
- `crates/ad-app/src/tools/mod.rs` / `ToolContext` -- pass the line options (and the cursor's snapped position if not already available); keep it small.
- `crates/ad-io/src/settings.rs` -- the two global options with serde defaults; tests for defaults, the minimum and an old file without them.
- `crates/ad-app/src/main.rs`, `ui.rs` -- hand the options to the connect tools.
- `crates/ad-io/tests/golden_edited.rs` -- a committed line (start, three new nodes, end) saves and reloads.
- `_bmad-output/.../architecture-rust-rewrite.md` AD-13 -- the deviation line.

## Tasks & Acceptance

**Execution:**
- [ ] `line_points` / `plan_line` -- tests for Long line, Short link, Dual/reverse, Heights, End node on, Node under a point
- [ ] tool wiring and preview -- tests: End node off, Continuous, Cancel, read-only
- [ ] settings -- defaults, minimum, missing keys
- [ ] golden -- line round-trip; AD-13 line

**Acceptance Criteria:**
- Given the fixture, when the user connects two nodes far apart with the connect tool, then nodes appear every 12 units along the line, linked with the tool's shape, and one Ctrl+Z removes them.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
