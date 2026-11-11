---
title: 'Refactor sweep'
type: 'chore'
ticket: '3'
created: '2026-11-11'
status: done
baseline_revision: '09f3437'
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

**Problem:** the sweep handles what the plans and reviews of epic-placeables deferred.

**Approach:**
- Collect deferred items from the epic's plans and reviews.
- Scan `crates/` for TODO and FIXME.
- Change code only for a real defect owned by this epic.

</frozen-after-approval>

## Implementation Notes

Oneshot: nothing to change.

- **Hand check:** the user checked footprints and rotation against the map image (2026-11-11: "sieht erstmal richtig aus"). Stories 1 and 2 are done.
- **Deferred, accepted:**
  - Two mods-folder changes within one read can apply out of order (story 2). This is rare, and the next folder change or reopening fixes it.
  - Base-game (`$data`) models get no footprint, because they are in the game's archives (an epic boundary).
  - Only the rotation about the vertical axis is applied.
- **Code scan:** no TODO, FIXME, `todo!` or `unimplemented!` in `crates/`.
- **Verification:** fmt, clippy and tests pass on 09f3437 (story 2); the frame time held there with 73 footprints.

## Review Triage Log

- Quick review (orchestrator, inline): no finding.
