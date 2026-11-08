---
title: 'A curve preview at fifty interpolation points stays within the frame budget'
type: 'feature'
ticket: '5'
created: '2026-11-08'
status: 'built'
baseline_revision: 'd71b841'
route: 'oneshot'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** R5 of the epic asks that previewing and adjusting a curve at the maximum interpolation points stays within 16 ms; no frame-time case measures it.

**Approach:** Two cases in the headless frame-time test (`view.rs` `frame_time`), built like the rotate cases: the Bézier tool with a curve between two fixture nodes in the dense area, 50 interpolation points and 4 control points, one handle dragged back and forth each frame, at minimum and maximum zoom ("curve: min zoom, 50 points, dragging a handle", "curve: max zoom, …"). Each step asserts the handle moved, so a frozen preview cannot pass. If a case is over budget on the developer's machine, fix the cost in the Bézier tool's paint path without changing what is drawn.

## Boundaries & Constraints

**Always:** the same measurement and report as the other cases (best-round median and max).

**Never:** no change to curve behaviour; no other cases changed.

</frozen-after-approval>

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms, the two curve cases included

## Implementation Notes

- `frame_time` (`crates/ad-app/src/view.rs`) gains `curve_case`, built like `rotate_case`. `begin` centres the view on the curve's midpoint at the case's zoom, creates the Bézier tool, has it `see` the view, clicks the dense node (the drag cases' `dragged`) and then the first node of the dense cell 40 to 80 world units away. It sets the curve's panel fields `points = MAX_POINTS` (50) and `controls = MAX_CONTROLS` (4), spreads the four handles 20 world units apart (`move_handle(i, ((i - 1.5) * 20, 10))`), starts a drag on handle 0 and installs the tool in `app`. `step` moves the pointer by `drag_px(i)` along x and asserts that the handle sits under the pointer and moved since the last step (from the second step after `begin` on). The tool's `paint_selection` draws the preview, so every frame paints the 50 points, 51 segments and 4 handles.
- Cases: "curve: min zoom, 50 points, dragging a handle" and "curve: max zoom, 50 points, dragging a handle". No other case changed, and no Bézier code changed: both cases came in far under budget, so the paint path needed no fix.
- Numbers (best round: median / max ms, release, developer machine):
  - Run 1: min zoom 2.134 / 2.259; max zoom 0.108 / 0.120.
  - Run 2: min zoom 2.169 / 2.482; max zoom 0.103 / 0.110.
  - At min zoom the cost is the network itself, about the same as "drag: min zoom, dense node" (2.2 to 2.3 ms). Every other case was ok in both runs; the worst best-round max was "rotate: min zoom, whole network selected" at 12.280 ms.

## Review Triage Log

- Quick review (orchestrator, inline): no finding; test-only change, the step asserts guard against a frozen preview.
