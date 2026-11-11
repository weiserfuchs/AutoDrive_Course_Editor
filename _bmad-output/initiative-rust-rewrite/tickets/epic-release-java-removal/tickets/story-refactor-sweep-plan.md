---
title: 'Refactor sweep'
type: 'chore'
ticket: '6'
created: '2026-11-11'
status: done
baseline_revision: '4dfb14e'
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

**Problem:** the sweep handles what the plans and reviews of epic-release-java-removal deferred.

**Approach:**
- Collect deferred items from the epic's plans and reviews and from deferred-work.md.
- Scan `crates/` for TODO and FIXME.
- Change code only for a real defect owned by this epic.

</frozen-after-approval>

## Implementation Notes

Oneshot: nothing to change.

- **Deferred items:** none from this epic's plans or reviews. Retro P2 in deferred-work.md (errors as `String`, no string table) is not owned by this epic and stays open, as recorded in the epic 9 sweep.
- **Release workflow:**
  - It has not run yet (story 2), so its first real run is story 7.
  - If it fails there, the fix belongs to story 7.
- **Code scan:** no TODO, FIXME, `todo!` or `unimplemented!` in `crates/`.
- **Verification:** fmt, build, clippy and tests pass on 4dfb14e (story 5); the frame-time test passed in story 3, and no Rust code has changed since.

## Review Triage Log

- Quick review (orchestrator, inline): no finding.
