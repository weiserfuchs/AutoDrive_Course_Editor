---
title: 'A Bézier curve previews between two nodes and commits from a curve panel'
type: 'feature'
ticket: '3'
created: '2026-11-08'
status: 'planned'
baseline_revision: '06a89ad'
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

**Problem:** Users cannot draw curves. Java's Bézier tool (`BezierCurveButton`, `BezierCurve`, `CurvePanel`) previews a curve between two nodes, sets it up in a curve panel and commits it as nodes and connections.

**Approach (as Java unless noted):**
- **Tool (B, toolbar "Bézier"):** a click on a start node, then on an end node, creates the curve preview; before the second click a line runs from the start to the hovered node or the cursor. A click on the start again, or a right click, cancels the pending start.
- **Curve:** 4 control points, all placed at the midpoint of start and end (Java `initControlPoints`), of which the first `k` are used (k = 1..4: quadratic to quintic Bernstein over x and z). Points at `t = i / (N + 1)`, `i = 1..N` (N interpolation points), end points excluded.
- **Curve panel** (shown while a curve is previewed): interpolation points 1..50 (default 6), control points 1..4 (default 1), priority (regular / sub-priority, default regular), connection type (regular, dual, reverse, default regular), "Swap direction" (start ↔ end and the used control points reversed, so the shape stays), "Commit" and "Cancel". The preview follows every change live.
- **Commit:** one `PasteNodes` (story 5.1 chain helper): N new nodes with the curve's priority as flag, linked start → nodes → end with the type. Heights per AD-7: `sample_height`, or without a heightmap linear by `t` from the start's y to the end's y (fixes Java's divide-by-zero at N = 1). After the commit the curve is gone and the tool waits for a new start.
- **Unlike Java (AD-10):** one curve at a time; switching tools or files, or starting a new curve, discards the preview. Nothing commits implicitly.
- **Control points (story 5.4 folded in, user decision):** the used control points show as handles. A primary drag that starts on a handle moves the handle by the mouse difference and its virtual control point by that difference times Java's move scaler 3 (`moveControlNodeBy` with scaling); the preview follows live. A drag on a handle never pans or selects. Otherwise, while a curve is previewed, a click on the map does nothing.

## Boundaries & Constraints

**Always:** one transaction per commit through `Document::apply`; read-only disables the tool (existing gates); the panel's settings apply only to the previewed curve; the next curve starts with the defaults (Java's XML defaults are not read).

**Never:** no undo entries for handle drags (they change only the preview); no arc spline (story 5.6); no settings for the defaults.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Preview | B, click start, click end | preview of 6 points on the straight line between them; panel shown | none |
| Points | control points at chosen positions, k = 1..4 | Bernstein points at t = i/(N+1) | none |
| Panel | N = 10, k = 2, sub-priority, dual | preview updates; commit makes 10 sub-priority nodes, dual links start → … → end | none |
| N = 1 | one interpolation point | one node at t = 0.5, height interpolated without division by zero | none |
| Heights | heightmap / none (start y 10, end y 20, N = 4) | sampled / 12, 14, 16, 18 | none |
| Drag handle | drag control point 1 by (10, 0) world | handle moves 10, virtual point 30; preview bends; no history entry | none |
| Hidden handles | k = 1 after dragging point 3 | only handle 1 shown and used; point 3 keeps its place if k grows again | none |
| Swap | swap direction, commit one-way | links run end → … → start, same shape | none |
| Cancel | Cancel button, tool switch, file switch, Escape not required | no commit, panel gone | none |
| Undo | commit, Ctrl+Z | curve nodes and links gone in one step | none |
| Read-only | legacy config | tool disabled | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/bezier.rs` (new) -- `BezierTool` (pending start, optional `Curve` with 4 handles and 4 virtual points); drag via `on_drag_start/on_drag/on_drag_end` when the press is on a handle (the view must offer the drag to the tool on a handle, not only on a node; check how move/rotate get theirs); pure `bernstein(points, t)`, `curve_points(start, controls, end, n)`, `plan(net, map, curve) -> Option<Transaction>` via `connect::chain` (story 5.2; it may need a flag parameter); paint of the pending line, the preview points and segments in the type's colour.
- `crates/ad-app/src/ui.rs` -- the curve panel (a side panel or window, shown while a curve exists) and the toolbar button; the panel edits the tool's curve and returns Commit/Cancel.
- `crates/ad-app/src/input.rs`, `main.rs` -- B shortcut (exact modifiers), action gated on `editable()`, commit through `Document::apply`; document replacement cancels the tool (existing).
- `crates/ad-io/tests/golden_edited.rs` -- a committed curve (sub-priority, dual) saves and reloads.

## Tasks & Acceptance

**Execution:**
- [ ] `tools/bezier.rs` -- math, handle drag and plan; tests for Points, Drag handle, Hidden handles, Panel, N = 1, Heights, Swap
- [ ] tool, panel and wiring -- tests for Preview, Cancel, Undo, Read-only, B shortcut
- [ ] golden -- curve round-trip

**Acceptance Criteria:**
- Given the fixture, when the user presses B, clicks two nodes, bends the curve by dragging a control point, sets interpolation points and type in the panel and commits, then the curve's nodes and links appear and one Ctrl+Z removes them.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
