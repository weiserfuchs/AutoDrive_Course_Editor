---
title: 'A curve preview at fifty interpolation points stays within the frame budget'
type: 'feature'
ticket: '5'
created: '2026-11-08'
status: 'planned'
baseline_revision: 'd71b841'
route: 'oneshot'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
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

## Review Triage Log
