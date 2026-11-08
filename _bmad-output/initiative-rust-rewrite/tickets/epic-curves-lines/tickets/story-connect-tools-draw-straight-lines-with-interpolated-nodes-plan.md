---
title: 'Connect tools draw straight lines with interpolated nodes'
type: 'feature'
ticket: '2'
created: '2026-11-08'
status: done
baseline_revision: 'dd52d30'
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
- [x] `line_points` / `plan_line` -- tests for Long line, Short link, Dual/reverse, Heights, End node on, Node under a point
- [x] tool wiring and preview -- tests: End node off, Continuous, Cancel, read-only
- [x] settings -- defaults, minimum, missing keys
- [x] golden -- line round-trip; AD-13 line

**Acceptance Criteria:**
- Given the fixture, when the user connects two nodes far apart with the connect tool, then nodes appear every 12 units along the line, linked with the tool's shape, and one Ctrl+Z removes them.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- `line_points(start, end, node_distance, end_is_node)` returns `(x, z, fraction)`: the inner points at `i / segments` when `segments > 1`, then the end unless it is a node (exactly `end`, fraction 1). With `segments <= 1` it is empty for an end node (today's `plan`) or just the new end point. The fraction drives the no-heightmap height (`y0 + (y1 - y0) * t`), so 60 units at y 10 → 20 give 12, 14, 16, 18.
- `chain(start, nodes, end, shape)` builds the one `PasteNodes` (regular nodes, `LinkEnd::Existing` start/end, `shape` on every segment); `plan_line` adds the heights. The Bézier story can reuse `chain`.
- Line options live on the tool (`LineOptions { node_distance, end_node }`, from `GlobalSettings` via `App::line()`), like `continuous`, because `paint` needs them too and gets no `ToolContext`.
- `Tool::paint` gains `hovered_node: Option<(f64, f64)>` (world position, looked up in `ToolHooks::paint`) so the preview ends exactly at the hovered node; the other tools ignore it.
- Continuous after a new end node: the new node's id exists only after the apply, so `Tool::committed(&Transaction)` (default no-op) is called by `App::click` after a successful apply; the connect tool takes the last pasted id as the next start. `cancel` drops that pending hand-over.
- The end point on empty map snaps like the node tool (`ctx.snap`, `map.grid`); the preview snaps the same way through `snap_grid`. As in Java the preview shows the line to the cursor even when the end node option is off.
- `line_node_distance` is a `u32` (Java int) with a deserializer raising 0 to 1; `LineOptions::from_settings` also applies `max(1)`.
- Existing test `semicolon_toggles_continuous_...` had a ~20-unit c → a link, which is now a line through one new node; the assertion follows.
- Golden: `committed_line_saves_and_reloads` builds the chain the dual tool commits (three nodes, positions kept to 3 decimals) since ad-io cannot call ad-app; saves, reloads, undo restores.
- Verification: `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` all pass (ad-app 270, golden_edited 4); `frame_time` all cases ok, worst best-round max 9.1 ms (min zoom, all selected), rotate min zoom whole network 8.5 ms.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator), pass 1: no patch.
  - checked, no finding: the start position is stored when picked; Undo, Redo, Delete, cut and paste cancel the tool first, so it cannot go stale before a commit, and a vanished start makes `plan_line` return nothing.
  - accepted as Java: with the end-node option off, the preview still runs to the cursor on empty map; the user judges it in the hand check.
- Verification (orchestrator re-run): fmt, clippy, workspace tests and the release frame-time test pass (no case over budget).
