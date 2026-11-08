---
title: 'An arc spline previews with handles and commits with heightmap heights'
type: 'feature'
ticket: '6'
created: '2026-11-08'
status: 'built'
baseline_revision: '58c61fa'
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

**Problem:** Java's experimental arc tool (`ArcSplineButton`, `ArcSpline`, `ArcSplineWidget`) lays out a circular arc as a standalone chain of nodes; users have no such tool.

**Approach (as Java unless noted):**
- **Tool (toolbar "Arc", no shortcut, as Java):** a primary click on the map with no arc previewed places an arc centred there: radius 8, start angle -90°, end angle 90° (a half circle). Angles follow Java: a point at angle `a` sits at `centre + r·(cos(a − 90°), sin(a − 90°))`, angles normalised to [0, 360), an end angle below the start gets +360.
- **Points:** N + 2 points (N interpolation points from the curve panel plus both ends), evenly spaced in angle (Java `generateArcPoints`).
- **Handles:** centre (drag moves the whole arc), start and end (drag turns that angle to the pointer's angle about the centre, in whole degrees), radius (sits midway between centre and start; a drag moves it along that line and sets the radius to twice its distance from the centre, at least 2). Handle drags change only the preview, never pan or select, and a lost pointer ends only the drag (story 5.3's `on_drag_cancel`).
- **Curve panel** (story 5.3's panel without the control-point setting): interpolation points 1..50 (default 6), priority, connection type, "Swap direction" (links run the other way round the arc), "Commit", "Cancel".
- **Commit:** one `PasteNodes` of the N + 2 nodes with the curve's priority as flag, linked in order with the connection type, not linked to any existing node (standalone, as Java). Heights per AD-7: `sample_height` at each node; without a heightmap the "no height" sentinel −1, as the node tool does. **Unlike Java**, which commits every arc node at y = 0 (AD-13 already names it in the epic's constraints).
- One curve or arc at a time; switching tools or files discards it (AD-10).

## Boundaries & Constraints

**Always:** one transaction per commit through `Document::apply`; read-only disables the tool (existing gates); a new arc starts with the defaults.

**Never:** no rotation-snap setting (whole degrees only, until the settings UI); no link to existing nodes; no undo entries for handle drags.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Place | Arc tool, click at (100, 50) | half circle radius 8 from (92, 50) round to (108, 50); 8 points; panel shown | none |
| Points | angles 0°..180°, N = 2 | 4 points evenly in angle, both ends included | none |
| Wrap | start 270°, end 90° | end treated as 450°: arc over 180° through 0° | none |
| Centre handle | drag by (10, 5) | every point and handle moves by (10, 5) | none |
| Angle handle | drag start handle to the pointer's angle 30.4° | start angle 30°; points follow | none |
| Radius handle | drag to 6 from the centre / to 0.5 | radius 12 / 2 | clamped |
| Swap | swap, commit one-way | links run last → first | none |
| Heights | heightmap / none | sampled / −1 sentinel | none |
| Commit / undo | commit, Ctrl+Z | N + 2 nodes and N + 1 links, one undo removes them | none |
| Cancel | Cancel, tool switch, file switch | no commit | none |
| Read-only | legacy config | tool disabled | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/arc.rs` (new) -- `ArcTool` with an optional `Arc { centre, radius, start_deg, end_deg, panel fields }`; pure `arc_points`, handle positions and moves; drag via the same mechanism as the Bézier handles; `plan` via `connect::chain`-style `PasteNodes` without existing ends (extend the helper or build the `PasteNodes` directly, whichever is smaller).
- `crates/ad-app/src/ui.rs` -- toolbar button; the curve panel shared with the Bézier tool (`Tool::curve_mut()` or a sibling accessor), hiding the control-point setting for the arc.
- `crates/ad-app/src/main.rs` -- action gated on `editable()`, commit through `App::finish_curve` (or its sibling).
- `crates/ad-io/tests/golden_edited.rs` -- a committed arc saves and reloads.

## Tasks & Acceptance

**Execution:**
- [x] `tools/arc.rs` -- math, handles, plan; tests for Place, Points, Wrap, Centre/Angle/Radius handle, Swap, Heights
- [x] tool, panel and wiring -- tests for Commit/undo, Cancel, Read-only
- [x] golden -- arc round-trip

**Acceptance Criteria:**
- Given the fixture, when the user places an arc, shapes it with its handles, commits it and presses Ctrl+Z, then the arc's nodes and links appear and disappear in one step.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- Shared panel: `bezier::CurveSettings { points, flag, shape }` (with `Default`: 6, regular, one-way) replaces the three fields on `Curve`; `arc::Arc` holds one too. `ui::curve_controls(ui, settings, controls: Option<&mut usize>, swap: &mut bool)` draws the panel; the control-point slider shows only when `controls` is given (Bézier). `curve_panel` picks the window ("Bézier curve" or "Arc") from `Tool::curve_mut()` or the new sibling `Tool::arc_mut()`, and calls the matching `swap()` after the window. The Bézier tests and the frame-time case now set `curve.settings.points` etc.; no behaviour change.
- `tools/arc.rs`: pure `span` (Java normalisation, end below start +360), `on_circle` (Java `angle - 90°`), `arc_points` (N + 2, even in angle), `angle_to` (Java `atan2(c - p) - 90°`, wrapped to [0, 360), rounded to whole degrees). `Arc { centre, radius, start_deg, end_deg, reversed, settings }`; handles are derived (`Arc::handle`), not stored: the radius handle is `on_circle(centre, r / 2, start)`. The centre and radius handles move by the pointer delta (as Bézier/Java); the radius move projects onto the centre-start line, clamped to [0, r] as Java's `clampNodeToLine` (so one move can at most double the radius) and to at least `MIN_RADIUS / 2`, radius = 2 x distance, at least 2. The angle handles take the pointer's absolute angle.
- Commit: `arc::plan(map, arc)` builds the `PasteNodes` directly with `LinkEnd::New` links (smaller than extending `connect::chain`, which also could not reverse). Swapped links run last -> first in Java's order. Heights `sample_height(..).unwrap_or(-1.0)`. `App::finish_curve` plans with `bezier::plan` when the tool has a curve, else `arc::plan`; still one `Document::apply`.
- Toolbar: "Arc" button after "Bézier", gated by `editable()` like every toolbar tool. No `Action`/shortcut was added (as Java, no shortcut); the read-only test therefore follows the toolbar-only tools' pattern (tool left active: a click places nothing, a held arc does not commit).
- A click while an arc is previewed does nothing (Java creates only when no arc is active). Placement does not grid-snap (not in the spec).
- Tests: `tools::arc::tests::{a_click_places_a_half_circle_of_eight_points, points_run_evenly_in_angle_with_both_ends, an_end_below_the_start_wraps_through_zero, the_centre_handle_moves_the_whole_arc, an_angle_handle_turns_to_the_pointers_whole_degree, the_radius_handle_sets_twice_its_distance_at_least_two, a_press_off_the_handles_is_not_claimed, the_commit_links_the_points_in_order_or_reversed_after_swap, heights_are_sampled_or_the_no_height_sentinel, cancel_drops_the_arc_and_the_next_starts_with_the_defaults}`; `tests::{an_arc_is_placed_shaped_by_its_handles_committed_and_undone_in_one_step, the_arc_is_dropped_by_cancel_a_tool_switch_and_a_new_file, read_only_document_ignores_the_arc_tool}` in `main.rs`; `committed_arc_saves_and_reloads` in `golden_edited.rs`.
- Verification: `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` all pass (ad-app 299, golden_edited 6). `frame_time` (release) all ok in two runs; worst best-round max "rotate: min zoom, whole network selected" 8.708 / 10.039 ms; curve cases 2.900 / 2.998 ms (min zoom) and 0.155 / 0.119 ms (max zoom). No frame-time case for the arc (not required).

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator), pass 1: no patch.
  - checked: a lost pointer ends only the handle drag (`on_drag_cancel`); the radius handle's clamp to the start follows Java's `clampNodeToLine` and still lets a continuous drag grow the radius.
  - accepted: no grid snap on placement and no shortcut, as Java; the read-only gate is the toolbar's, as for the other toolbar-only tools.
- Verification (orchestrator re-run): fmt, clippy and workspace tests pass (ad-app 299, golden_edited 6).
