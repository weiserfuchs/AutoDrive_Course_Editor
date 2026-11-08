---
title: 'Refactor sweep'
type: 'chore'
ticket: '6'
created: '2026-11-08'
status: done
baseline_revision: '236b02d'
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

**Problem:** This sweep handles what epic-scans-autosave's plans and reviews deferred, including the CI AD-1 dependency check noted in deferred-work.md.

**Approach:**
- Collect every item the epic's plans and reviews deferred, and every deferred-work.md entry assigned to this epic; check each against the current code and record the outcome here.
- Scan the crates for TODO/FIXME and for `#[allow(...)]` this epic added, and record whether any is a leftover.
- No code changes unless a check finds a real defect owned by this epic; the CI check is in scope by the ticket.

</frozen-after-approval>

## Implementation Notes

Oneshot: the AD-1 CI check added; nothing else deferred.

- **CI AD-1 check** (deferred-work.md entry from the 7.6 sweep, assigned to this sweep by the ticket): `.github/workflows/ci.yml` gains a step "ad-core dependency check (AD-1)". It fails when `cargo tree -p ad-core -e normal` lists a UI, I/O, HTTP, threading or settings crate (egui, eframe, epaint, winit, wgpu, quick-xml, image, image_dds, zip, ureq, rfd, tokio, reqwest, rayon, crossbeam, directories, toml, serde), or when `crates/ad-core/src` uses `std::fs`, `std::net`, `std::thread` or `std::process`. Run locally: passes (ad-core depends only on rstar, thiserror and their dependencies). The step uses bash on the Windows runner.
- **Deferred by this epic's plans and reviews:** none. Accepted behaviours are recorded in the stories (count notice follows edits; a revision bump cancels a pending merge; "unchanged" for autosave by save point; no notice after a connection fix).
- **Hand-check decisions during the epic:** the overlap distance applies in (x, z) with a 2 m height tolerance (8.1); the move clamp for a selection spanning the map stays (8.3).
- **Finding about the fixture:** at scale 2 two fixture nodes (file ids 35400, 35401) lie far west of the map (x ≈ -9620); the out-of-bounds fix moves them to the edge. The fixture itself stays unchanged.
- **Code scan:** no TODO, FIXME, `todo!` or `unimplemented!` in `crates/`; the epic added one `#![allow(clippy::expect_used)]` in a test module (the convention).
- Verification: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (ad-app 419 + 3 ignored) and the release frame-time test pass.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- the AD-1 step's commands run locally -- expected: pass

## Review Triage Log

- Quick review (orchestrator, inline): the CI step was dry-run locally and passes; no finding.
