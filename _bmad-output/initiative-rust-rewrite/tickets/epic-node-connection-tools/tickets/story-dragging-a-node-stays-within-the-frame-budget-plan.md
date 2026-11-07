---
title: 'Dragging a node stays within the frame budget'
type: 'feature'
ticket: '3'
created: '2026-11-07'
status: done
baseline_revision: '3ea8268072a80a246560d9f9ddeebf545865fa46'
route: 'oneshot'
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

**Problem:** R3 of epic-node-connection-tools requires every frame to stay within 16 ms on the FS25 fixture while a node is dragged, at any zoom. The headless frame-time test measures static views, pan and zoom, but not a drag.

**Approach:** Add two cases to the ignored `frame_time` test in `crates/ad-app/src/view.rs`: a node in the densest cell is dragged back and forth with the move tool at minimum and at maximum zoom, grid on, with the view drawing the drag preview, measured like the existing cases. Fix whatever the measurement shows is over budget; no other code changes.

</frozen-after-approval>

## Implementation Notes

Oneshot: a test-only change of about 40 lines, unless the measurement shows a budget breach.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case, the drag cases included, under 16 ms

**Manual checks (if no CLI):**
- The user drags a node on the fixture at both zoom ends and confirms it stays smooth.

- Measured (release, 2026-11-07, after the review patch): drag at minimum zoom, best round median 2.29 ms, max 2.84 ms; drag at maximum zoom, median 0.055 ms, max 0.079 ms. Every other case stayed under 16 ms. Nothing was over budget, so no code changed beyond the test.

## Review Triage Log

- Quick review, pass 1: 1 medium and 1 low patched; 1 low and 1 false rejected.
  - medium, patch: the drag offset was accumulated and reset only per round, so the warm-up's +400 px carried into the timed frames; the node drifted off screen at max zoom and could freeze at the map edge at min zoom. The offset is now a triangle of the frame index (0 → +320 → -280 px), the same in warm-up and timed frames.
  - low, patch: the final assert proved only that some preview existed. Each drag frame now asserts that the preview sits at the dragged position, so a frozen or ignored drag fails the test.
  - low, reject: the drag bypasses the view's press handling (hit test, claim). That path adds one R-tree query on the press frame, not per drag frame; routing pointer events through the test is more than a direct correction. Noted in the test comment.
  - false: "no evidence of the numbers". The release run was made; the numbers are recorded above.
