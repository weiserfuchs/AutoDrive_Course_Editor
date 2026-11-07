---
title: 'Refactor sweep'
type: 'chore'
ticket: '9'
created: '2026-11-07'
status: done
baseline_revision: '5756f93d4ab5a91d41ba7f54089a9aea0dd3d36d'
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

**Problem:** This sweep handles what epic-node-connection-tools' plans and reviews deferred.

**Approach:**
- Collect every item the epic's plans and reviews deferred, and every deferred-work.md entry assigned to this epic; check each against the current code and record the outcome here.
- Scan the crates for TODO/FIXME and for `#[allow(...)]` this epic added, and record whether any is a leftover.
- No code changes unless a check finds a real defect owned by this epic.

</frozen-after-approval>

## Implementation Notes

Oneshot: the investigation found nothing deferred and one rejected defect worth fixing (below); one small code change.

- **Deferred by this epic's plans and reviews:** none. Every review finding of stories 3.10 and 3.1–3.8 was patched or rejected with its reason in the plan's Review Triage Log; no story appended to deferred-work.md.
- **Re-checked rejected defect (story 3.4):** Ctrl+Z and Delete in one frame could delete a node the undone move took away from the cursor, because the hover came from the previous frame's network. Fixed here: Undo and Redo clear the view's hover (`App::run`), the next frame recomputes it. Test `undo_and_delete_in_one_frame_delete_nothing`. The other rejected findings of the epic are test-coverage or evidence points, not behaviour defects.
- **deferred-work.md entries assigned to this epic:**
  - `hovered_edge` drawn segment and node suppression (from story 2.9): settled by story 3.4 (`hover::edge_at`: a hovered node wins; only the drawn part counts, no end caps). The entry stays as written (append-only).
  - Retro P2, its `AddNode` → `AddNodes` part: settled by story 3.10. The rest of P2 (errors as `String` in ad-app, no string table) stays with epic-settings-ui-shell.
- **Code scan:** no TODO, FIXME, `todo!` or `unimplemented!` in `crates/`.
  - `#[allow(dead_code)]`: only `Icons::parking` and `Icons::warning` remain (later epics). This epic removed the allows on `Document::apply`, `Document::is_dirty` and `MapView::hovered_edge`, which are now used.
  - `#![allow(clippy::expect_used)]`: one in each new test module (`document.rs`, `main.rs`, `tools/connect.rs`, `delete.rs`, `flip.rs`, `move_node.rs`, `node.rs`, `priority.rs`). Intended: the crates deny `expect_used`, and every test module follows this convention since epic 1.
  - `#[allow(clippy::too_many_arguments)]`: this epic added two, on `MapView::show` (the hooks parameter, story 3.2) and `MapView::paint` (the preview override, story 3.2), beside the existing `paint_info_box` and test helper from story 2.9. They are intended; grouping the view's frame inputs into a struct would be a refactor of the view, not a deferred cleanup.
- Baseline before the sweep: fmt, clippy `-D warnings`, `cargo test --workspace` (ad-app 146 + 2 ignored, ad-core 59 + 4, ad-io 39 + golden_edited + golden_roundtrip) and the release frame-time test pass.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Review Triage Log

- Quick review, pass 1: 2 low patched.
  - low, patch: the allow scan left out the test modules' `#![allow(clippy::expect_used)]`. Recorded above as intended.
  - low, patch: story 3.4 rejected a real defect (Ctrl+Z and Delete in one frame) only for its fix cost; re-checked, the fix is one line per action. Fixed and tested here.
- Verification after the fix: fmt, clippy, workspace tests (ad-app 147) and the release frame-time test pass.
