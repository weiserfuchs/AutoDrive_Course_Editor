---
title: 'Refactor sweep'
type: 'chore'
ticket: '7'
created: '2026-11-08'
status: 'built'
baseline_revision: '552ee3e'
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

**Problem:** This sweep handles what epic-curves-lines' plans and reviews deferred.

**Approach:**
- Collect every item the epic's plans and reviews deferred, and every deferred-work.md entry assigned to this epic; check each against the current code and record the outcome here.
- Scan the crates for TODO/FIXME and for `#[allow(...)]` this epic added, and record whether any is a leftover.
- No code changes unless a check finds a real defect owned by this epic.

</frozen-after-approval>

## Implementation Notes

Oneshot: nothing deferred, no code change; two missing AD-13 lines added.

- **Deferred by this epic's plans and reviews:** none. Every review finding of stories 5.1, 5.2, 5.3, 5.5 and 5.6 was patched or accepted with its reason in the plan's Review Triage Log; story 5.4 was folded into 5.3 (dropped). No deferred-work.md entry names this epic.
- **Hand-check fix:** the curve and arc previews had no direction cue; fixed after story 5.6 with arrowheads (`bezier::paint_chain`).
- **AD-13:** the epic's constraints name two Java deviations the AD-13 list did not carry yet: arc nodes at y = 0, and curve previews surviving a tool switch. Both lines added. The Bézier divide-by-zero was already listed; the line-node reuse was added in story 5.2.
- **Code scan:** no TODO, FIXME, `todo!` or `unimplemented!` in `crates/`.
  - `#[allow(clippy::too_many_arguments)]`: one added on `Tool::paint` (story 5.2, the hovered node's position). Intended, as on `paint_selection` and the view's painters; grouping the frame inputs into a struct would be a refactor of the tool trait, not a deferred cleanup.
  - `#![allow(clippy::expect_used)]` in the new test modules (`tools/bezier.rs`, `tools/arc.rs`): the test-module convention.
- **Noted, not changed:** the `Tool` trait grew per-tool accessors (`curve_mut`, `arc_mut`) and hooks (`committed`, `on_secondary_click`, `on_drag_cancel`) with defaults. Each is small and used; if more tools need panels, one `panel_mut` returning an enum would replace the two accessors.
- Verification: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (ad-app 300 + 2 ignored, ad-core 64 + 4, ad-io 40, golden_edited 6, golden_roundtrip 1) and the release frame-time test pass.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Review Triage Log

- Quick review (orchestrator, inline): no finding; the sweep changed no code.
