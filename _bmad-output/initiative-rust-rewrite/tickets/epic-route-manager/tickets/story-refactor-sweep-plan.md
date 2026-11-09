---
title: 'Refactor sweep'
type: 'chore'
ticket: '4'
created: '2026-11-09'
status: done
baseline_revision: 'f43c128'
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

**Problem:** This sweep handles what epic-route-manager's plans and reviews deferred.

**Approach:**
- Collect every item the epic's plans and reviews deferred, and every deferred-work.md entry assigned to this epic; check each against the current code and record the outcome here.
- Scan the crates for TODO/FIXME, for leftover references to later stories and for `#[allow(...)]` this epic added.
- No code changes unless a check finds a real defect owned by this epic.

</frozen-after-approval>

## Implementation Notes

Oneshot: nothing to change.

- **deferred-work.md:** the two 11.1 entries (route autosave and merge backup; the "All is id 1" renumbering) were resolved in 11.2 and recorded there.
- **Code scan:** no TODO, FIXME, `todo!` or `unimplemented!` in `crates/`. No comment still points at story 11.2 or 11.3. The epic added `#![allow(clippy::expect_used)]` to one test module and `#[allow(clippy::expect_used)]` to the routes_xml test module (the convention).
- **Open for the user:** the hand checks of 11.1, 11.2 and 11.3. A real route export from the game should become a fixture when the user has one (epic constraint C3).
- **Verification:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (ad-app 428 + 4 ignored) and the release frame-time test pass; worst best-round max 13.4 ms (drag, min zoom, whole network).

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Review Triage Log

- Quick review (orchestrator, inline): no finding.
