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
- [x] `tools/bezier.rs` -- math, handle drag and plan; tests for Points, Drag handle, Hidden handles, Panel, N = 1, Heights, Swap
- [x] tool, panel and wiring -- tests for Preview, Cancel, Undo, Read-only, B shortcut
- [x] golden -- curve round-trip

**Acceptance Criteria:**
- Given the fixture, when the user presses B, clicks two nodes, bends the curve by dragging a control point, sets interpolation points and type in the panel and commits, then the curve's nodes and links appear and one Ctrl+Z removes them.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Math:** `bezier::bernstein` is the general Bernstein form over start, the used control points and end; a test checks it against Java's written-out quadratic to quintic formulas for k = 1..4. `curve_points` samples t = i/(N+1), i = 1..N.
- **Commit:** `bezier::plan` reuses `connect::plan_line` (heights per AD-7: `sample_height`, else linear by t, so N = 1 gives the mean height with no division). `plan_line` and `chain` got a `flag` parameter; the connect tools pass `NodeFlag::REGULAR`.
- **Curve state:** `Curve` keeps the start/end ids (positions are read from the network when painted or planned), 4 handles and 4 virtual control points, all at the midpoint, plus the panel fields. `controls` picks the first k; hidden ones keep their place. Swap reverses only the used k (Java `swapCurveDirection`).
- **Handle drag:** same mechanism as the rotate handle: `paint_selection` records world units per px (`see`), `on_drag_start` claims a press within 8 px of the nearest used handle, so the view neither pans nor offers it elsewhere; `on_drag` moves the handle by the pointer delta and its control point by 3x (Java `controlPointMoveScaler` default); `on_drag_end` returns no transaction, so no history entry.
- **Panel access:** a default trait method `Tool::curve_mut() -> Option<&mut bezier::Curve>`, overridden only by the Bézier tool. Chosen over downcasting because the panel needs only the curve, and it keeps `ui.rs` free of `Any`. The panel is a non-modal `egui::Window` drawn after the map panel while a curve exists; it edits the curve in place (the preview follows the next frame), and its Commit/Cancel go through `App::finish_curve` (Commit: `Document::apply`, gated on `editable()`; both then cancel the tool so it waits for a new start).
- **Right click:** a new default trait method `Tool::on_secondary_click()`, called from `ToolHooks::select` for a right click (which still selects, AD-15); the Bézier tool clears its pending start there. A previewed curve stays.
- **Clicks while previewing** do nothing. A new curve needs Commit, Cancel, or picking the tool again (B or the toolbar), which drops the preview (AD-10).
- **Shortcut:** B, exact modifiers; `Action::BezierTool` gated on `editable()` like the other tools.
- **Not done / open:** `DragEvent::Cancel` (a claimed drag released where no pointer position is known) calls `Tool::cancel`, which drops the whole curve and not only the drag; the same path exists for the other tools. The panel's widgets have no UI-level test; its logic is covered through `finish_curve` and the `Curve` methods.

**Tests:** `tools::bezier::tests::{bernstein_matches_javas_quadratic_to_quintic_formulas, curve_points_sit_at_i_over_n_plus_one_without_the_ends, two_clicks_preview_six_points_on_the_straight_line, a_click_on_the_start_or_a_right_click_cancels_the_pending_start, dragging_a_handle_moves_it_and_its_control_point_three_times_as_far, a_press_off_the_handles_is_not_claimed, hidden_handles_keep_their_place_and_are_not_used, the_panels_settings_shape_the_commit, one_point_sits_half_way_at_the_mean_height, heights_are_sampled_or_run_linearly_by_t, swap_reverses_the_links_and_keeps_the_shape, cancel_drops_the_curve_and_the_pending_start}`; `tests::{b_previews_a_curve_that_bends_commits_from_the_panel_and_undoes_in_one_step, the_curve_is_dropped_by_cancel_a_tool_switch_b_and_a_new_file, a_right_click_cancels_the_pending_curve_start, read_only_document_ignores_the_bezier_tool}` in `main.rs`; B added to `input::tests`; `golden_edited::committed_curve_saves_and_reloads`.

**Verification:** `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` all pass (ad-app 286, golden_edited 5, golden_roundtrip ok). Frame time (release): every case under 16 ms; worst maxima of the best rounds: rotate min zoom whole network 11.6 ms, drag min zoom whole network 8.4 ms, rotate tool idle 7.8 ms, static min zoom all selected 6.8 ms.

## Plan Change Log

## Review Triage Log
