---
title: 'Refactor sweep'
type: 'chore'
ticket: '6'
created: '2026-11-08'
status: done
baseline_revision: '5354147'
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

**Problem:** This sweep handles what epic-map-images-import's plans and reviews deferred.

**Approach:**
- Collect every item the epic's plans and reviews deferred, and every deferred-work.md entry assigned to this epic; check each against the current code and record the outcome here.
- Scan the crates for TODO/FIXME and for `#[allow(...)]` this epic added, and record whether any is a leftover.
- No code changes unless a check finds a real defect owned by this epic.

</frozen-after-approval>

## Implementation Notes

Oneshot: one deferred defect fixed; nothing else left.

- **deferred-work.md entry from story 7.4** (a download finishing after another config was opened could overwrite an image imported for the same map meanwhile): a real defect owned by this epic, fixed. `jobs::download_map_image` saves only when the target does not exist yet; a download starts only when no local image existed, so an existing file is newer. Test `a_download_never_overwrites_an_image_saved_meanwhile`. The entry stays as written (append-only).
- **Accepted behaviours recorded in the stories:** the DDS crop is the centre rectangle w/2 × h/2 (7.2); only the zip entries actually read are checked for hostile names, and `dem.png` must be named exactly (7.3); without a map name the save targets are Java's `unknown.png` / `unknown_HeightMap.png` (7.5).
- **Hand-check fixes during the epic:** the stale "No heightmap found" notice after a zip import (7.3).
- **Code scan:** no TODO, FIXME, `todo!` or `unimplemented!` in `crates/`. The epic added only `#![allow(clippy::expect_used)]` in test modules (the convention) and ad-io's `test-util` feature for the download test server, used only by ad-app's dev-dependencies.
- **Noted for CI:** the architecture says CI runs an AD-1 dependency check; `.github/workflows/ci.yml` runs only fmt, clippy and test. ad-core gained no dependency in this epic, so nothing broke, but the check is missing (story 7.1 noticed it).
- Verification: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (ad-app 385 + 3 ignored) and the release frame-time test pass.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Review Triage Log

- Quick review (orchestrator, inline): the one code change is small and tested; no finding.
