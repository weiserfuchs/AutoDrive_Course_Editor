---
title: 'Refactor sweep'
type: 'chore'
ticket: '8'
created: '2026-11-07'
status: 'built'
baseline_revision: 'dd4d79d137acc04583e40d56c3d9358044c74c6e'
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

**Problem:** This sweep handles what epic-map-view's plans and reviews deferred. Its plans deferred two items to deferred-work.md:
- The height-warning check walked every visible node each frame (from story 2.5, meant for story 2.7).
- `hovered_edge` tests the full centre-to-centre segment, and a node hover also yields its edges (from story 2.9, for the tools epic).

**Approach:**
- Check each item against the current code and record the outcome here.
- Scan the crates for TODO/FIXME and `#[allow(...)]` added by this epic, and record whether any is a leftover.
- No code changes unless a check finds a real defect owned by this epic. Items deferred by epic 1 (AGENTS.md refresh, merge gate, retro B4–B8, A2, R4, P2) stay with their target epics.

</frozen-after-approval>

## Implementation Notes

Oneshot: the investigation found no code to change, so this is a record-only sweep.

- **Height-warning walk (story 2.5):** settled by story 2.7 (f709d0d). `Network` keeps a `no_height` set, updated on add, move and remove (`crates/ad-core/src/network.rs:191`, accessor `nodes_without_height` at :283). The view reads only that set, filtered by the padded visible rectangle (`crates/ad-app/src/view.rs:447`). The randomised test `no_height_set_follows_every_mutation` (`crates/ad-core/src/network/spatial.rs:607`) checks it against a full scan. No change.
- **`hovered_edge` segment (story 2.9):** still open as designed. Nothing reads `hovered_edge` yet, and the deferred entry assigns the decision to epic-node-connection-tools. No change.
- **Code scan:** no TODO, FIXME, `todo!` or `unimplemented!` in `crates/`. None of the allows below is a leftover.
  - Five `#[allow(dead_code)]`, each with a comment naming its later user. This epic added `Icons::parking` and `Icons::warning` (later epics) and `MapView::hovered_edge` (tools epic). Epic 1 added `Document::apply` (edit stories) and `Document::is_dirty` (dirty-state UI).
  - Two `#[allow(clippy::too_many_arguments)]` from story 2.9: `paint_info_box` (`crates/ad-app/src/view.rs:247`) and the test helper `frame_hover` (`view.rs:604`). They are intended; a parameter struct would be a refactor beyond this sweep.
  - `#![allow(clippy::expect_used)]` heads the test modules of the files this epic created or touched (`hover.rs`, `icons.rs`, `look.rs`, `view.rs`, `heightmap.rs`, `network/spatial.rs`, `network/markers.rs`). This is the workspace convention for tests, used by every crate since epic 1.
- deferred-work.md is append-only, so its entries stay as written; this plan closes the height-warning entry.
- Baseline before the sweep: fmt, clippy `-D warnings` and `cargo test --workspace` pass (ad-app 58 + 2 ignored, ad-core 56 + 4, ad-io 39 + golden_edited + golden_roundtrip).

## Review Triage Log

- Quick review, pass 1: 2 low patched.
  - low, patch: the allow scan left out the `clippy::too_many_arguments` and test-module `clippy::expect_used` allows this epic added. Checked with grep; the Code scan note now covers them.
  - low, patch: the `Document::is_dirty` allow was labelled "edit stories", but its comment names the dirty-state UI. The note is corrected.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case's best-round median and max under 16 ms
