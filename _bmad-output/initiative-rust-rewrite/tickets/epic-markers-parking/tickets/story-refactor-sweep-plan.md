---
title: 'Refactor sweep'
type: 'chore'
ticket: '5'
created: '2026-11-08'
status: done
baseline_revision: 'd4ed8b3'
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

**Problem:** This sweep handles what epic-markers-parking's plans and reviews deferred.

**Approach:**
- Collect every item the epic's plans and reviews deferred, and every deferred-work.md entry assigned to this epic; check each against the current code and record the outcome here.
- Scan the crates for TODO/FIXME and for `#[allow(...)]` this epic added, and record whether any is a leftover.
- No code changes unless a check finds a real defect owned by this epic.

</frozen-after-approval>

## Implementation Notes

Oneshot: nothing deferred, no code change.

- **Deferred by this epic's plans and reviews:** none. Every review finding of stories 6.1–6.4 was patched or accepted with its reason in the plan's Review Triage Log; no deferred-work.md entry names this epic. Two hand-check fixes landed during the epic: duplicate names from older files can be kept when editing (story 6.3) and the top bar wraps so no button is cut off (after story 6.4).
- **Accepted behaviours worth knowing later:** a Save overwrites a `vehicles.xml` the game changed while the editor was open (as the config save does); Escape closes the whole Groups window even with a rename field open; "None" and same-name checks are case-sensitive.
- **Code scan:** no TODO, FIXME, `todo!` or `unimplemented!` in `crates/`.
  - `#[allow(dead_code)]`: story 6.1 removed the one on `Icons::parking`; only `Icons::warning` remains (later epic).
  - `#![allow(clippy::expect_used)]` in the new test modules (`tools/marker.rs`, `ad-io` `vehicles.rs`): the test-module convention.
- Verification: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (ad-app 347 + 2 ignored, ad-core 71 + 4, ad-io 44, golden_edited 9, golden_roundtrip 1, vehicles_roundtrip 6) and the release frame-time test pass.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Review Triage Log

- Quick review (orchestrator, inline): no finding; the sweep changed no code.
