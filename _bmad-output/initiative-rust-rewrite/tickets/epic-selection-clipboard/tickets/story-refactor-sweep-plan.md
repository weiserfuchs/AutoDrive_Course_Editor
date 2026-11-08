---
title: 'Refactor sweep'
type: 'chore'
ticket: '10'
created: '2026-11-08'
status: 'built'
baseline_revision: '1da58ef'
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

**Problem:** This sweep handles what epic-selection-clipboard's plans and reviews deferred.

**Approach:**
- Collect every item the epic's plans and reviews deferred, and every deferred-work.md entry assigned to this epic; check each against the current code and record the outcome here.
- Scan the crates for TODO/FIXME and for `#[allow(...)]` this epic added, and record whether any is a leftover.
- No code changes unless a check finds a real defect owned by this epic.

</frozen-after-approval>

## Implementation Notes

Oneshot: the investigation found nothing left to fix; no code change.

- **deferred-work.md entries from this epic:**
  - Joint ("whole-map") preview query frame-time case (from story 4.3): settled in story 4.6. Rotating the whole network at minimum zoom takes that branch every frame, and tools hand the view their preview bounds (`Preview::place_all`); the cases "rotate: min zoom, whole network selected" and "drag: min zoom, whole network selected" measure it. The entry stays as written (append-only).
  - Ctrl+C/X/V reaching a text widget outside the modal dialogs (from story 4.9): not a defect today, since no non-modal widget holds text a user needs to copy; it stays open for epic-settings-ui-shell, which brings text inputs.
- **Rejected review findings re-checked:** story 4.1's "selected looks like hovered" is a look choice the user accepted in the hand check; story 4.4's untested toolbar switch is two labels on one bool. Neither is a behaviour defect. The other rejections were marked false.
- **Code scan:** no TODO, FIXME, `todo!` or `unimplemented!` in `crates/`.
  - `#[allow(dead_code)]`: only `Icons::parking` and `Icons::warning` (later epics), unchanged.
  - `#[allow(clippy::too_many_arguments)]`: this epic added the one on `Tool::paint_selection` (story 4.7, the hovered node). Intended, as the view's existing ones; grouping the frame inputs into a struct would be a refactor of the view and tool trait, not a deferred cleanup.
  - `#![allow(clippy::expect_used)]` in the new test modules (`clipboard.rs`, `tools/align.rs`, `tools/edit_location.rs`, `tools/rotate.rs`): the test-module convention since epic 1.
- **Frame time:** on this machine under load, "rotate: min zoom, whole network selected" went over 16 ms in about one run in three (best-round max 16.1–17.3 ms; the other runs 9.7–12.7 ms). An alternating comparison with the state after story 4.6 (0a664c6, 3 runs each) gave 19.5 / 17.6 / 11.8 ms there and 17.3 / 11.2 / 12.7 ms on master, so stories 4.7–4.9 did not slow it down; the case sits near the budget under load, as before. Every other case stayed under budget in every run.
- Verification: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` (ad-app 260 + 2 ignored, ad-core 62 + 4, ad-io 39, golden_edited 2, golden_roundtrip 1) pass.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Review Triage Log

- Quick review (orchestrator, inline): no finding; the sweep changed no code.
