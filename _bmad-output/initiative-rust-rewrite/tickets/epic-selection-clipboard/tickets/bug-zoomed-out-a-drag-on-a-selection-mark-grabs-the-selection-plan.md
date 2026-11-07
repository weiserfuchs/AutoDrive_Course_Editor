---
title: 'Zoomed out, a drag on a selection mark grabs the selection'
type: 'bugfix'
ticket: '11'
created: '2026-11-08'
status: 'built'
baseline_revision: 'f7633b3aa740b652ef4d2474f30a57f6daf66a29'
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

**Problem:** Found by the user in the story 4.3 hand check: zoomed far out, nodes are too small to hit (story 2.9 rule, node size below `MIN_NODE_PX`), so a move drag never starts on a node and a selection cannot be dragged there, although its marks (story 4.1) are visible.

**Approach:** When the press of a primary drag hits no node and nodes are too small to hit, a selected node whose mark lies within half the mark size plus 2 px of the press is offered to the tool as the drag's node; the move tool then drags the whole selection (story 4.2). Unselected nodes stay unhittable at that size, and nothing changes at zoom levels where nodes are drawn.

</frozen-after-approval>

## Implementation Notes

Oneshot: about 50 lines with tests.

- `crates/ad-app/src/hover.rs`: `selected_mark_at` finds the nearest selected node within `reach` screen px (square test, lowest id on a tie), querying the R-tree around the press.
- `crates/ad-app/src/view.rs`: at a primary drag start, `node_at` first; if it finds nothing, the node size is below `MIN_NODE_PX` and there is a non-empty selection, `selected_mark_at` with reach `SELECTED_MARK_PX / 2 + MARK_HIT_SLACK_PX` (1.5 + 2 px). Test `zoomed_out_a_press_on_a_selection_mark_grabs_that_node` (zoom 0.5): a press on the mark and 3 px beside it grabs the selected node; 5 px beside it, on an unselected node, or with no selection grabs nothing. Test helper `pointer_frames_zoomed` added beside `pointer_frames_on`.
- Hover, clicks and the selection gestures are unchanged: only the drag start uses the mark.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

**Manual checks (if no CLI):**
- The user zooms fully out, selects part of the fixture, and drags it with the move tool by one of its marks.

## Review Triage Log

- Quick review, pass 1: 2 low patched.
  - low, patch: `selected_mark_at` queried exactly ±reach while its test is inclusive, so rounding could drop a node at the edge; the query now has 1 px slack, as `node_at`.
  - low, patch: the new `std` import split the `ad_core` import group; moved to the top.
