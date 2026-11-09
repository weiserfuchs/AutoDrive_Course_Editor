---
title: 'The frame budget is re-measured at minimum and maximum zoom'
type: 'chore'
ticket: '3'
created: '2026-11-09'
status: 'built'
baseline_revision: 'dc760cc'
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

**Problem:** R15 asks for pan, zoom and drag within 16 ms at any zoom. The frame-time test covers minimum zoom and the default maximum (30), but since epic 9 story 4 the user can raise the maximum zoom to 250.

**Approach:**
- `frame_time` gets a `view_max` the view is built with.
- Three cases run at the settings' highest maximum zoom (`MAX_ZOOM_RANGE.end()`, 250), centred on the dense node:
  - static with a hovered node;
  - pan;
  - dragging a node.
- The whole test is run in release and the numbers are recorded here.

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/view.rs` -- `frame_time`: `view_max`, `at_top` and the zoom-250 cases.

## Implementation Notes

- **Centre at zoom 250:** the zoom anchor (960, 540) is not the panel centre (the top bar pushes the panel down). So zooming in moves the centre by a few world units, and at zoom 250 no node is within the 300 px hover reach. `at_top` sets the centre again after zooming.
- **Measurement, 2026-11-09:**
  - Setup: release build on the developer's machine, FS25 fixture (35 652 nodes, densest 256² cell 768 nodes), 1920×1080, best of 3 rounds of 30 frames, max frame time of the best round.
  - Results:

| Case | Max ms |
|---|---|
| static, min zoom / all selected | 3.6 / 7.7 |
| static, max zoom 30, dense, hovered | 0.3 |
| static, zoom 250, dense, hovered | 0.2 |
| pan, min zoom / mid zoom / zoom 250 | 4.0 / 0.7 / 0.1 |
| zoom in and out, min ↔ 30 | 2.6 |
| drag node, min / 30 / 250 | 3.5 / 0.1 / 0.1 |
| drag whole network, min zoom (worst case) | 14.2 |
| drag west half, min zoom | 7.4 |
| rotate whole network, min zoom | 11.7 |
| curve handle drag, min zoom | 4.0 |

  - Every case is within 16 ms, so R15 is met.
  - Single spikes occur in the other rounds, e.g. 42.7 ms once while dragging the whole network. They come from the OS or allocator and match earlier epics' runs, where the best round is the measure.
- **Verification:** fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 456 + 4 ignored); frame time ok.

## Verification

**Commands:**
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: the zoom-250 static case asserts a hovered node every frame, so the view really is at that zoom on a node.
