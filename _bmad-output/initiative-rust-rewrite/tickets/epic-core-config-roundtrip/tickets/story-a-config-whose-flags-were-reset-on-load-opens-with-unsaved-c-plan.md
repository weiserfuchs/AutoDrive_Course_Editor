---
title: 'A config whose flags were reset on load opens with unsaved changes'
type: 'feature'
ticket: '10'
created: '2026-11-06'
status: 'built'
baseline_revision: '0b09627570c67d4cc579922704a1e5c9443b87da'
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

**Problem:** An FS22/FS25 load resets flags 2 and 4 to 0, so Save writes different bytes, yet the document starts clean. A close prompt or autosave would treat it as unchanged (retro finding B2).

**Approach:** `ad-io` reports whether the load reset any flag. When it did, the new `Document` starts with its saved state lost, so it is dirty until a successful save. The reset is not a History entry, so undo cannot restore the old flags.

</frozen-after-approval>

## Implementation Notes

Oneshot: about 60 lines in three files plus tests.

- `crates/ad-io/src/config_xml.rs`: `build_network` also returns whether it changed a flag. `LoadedConfig` stores it, and a getter `flags_reset()` exposes it. The `config_xml` unit test for FS22/FS25 asserts true; FS19, read-only and flag-free configs assert false.
- `crates/ad-core/src/history.rs`: `History::mark_unsaved()` sets the saved state to lost. A unit test shows that `mark_saved_at` with a later `Base` capture makes it clean again.
- `crates/ad-app/src/document.rs`: `Document::new` calls `mark_unsaved()` when `config.flags_reset()`.
- `crates/ad-app/src/jobs.rs` tests, through `apply_result`:
  - FS25 with flag 2 is dirty and cannot undo;
  - FS25 without 2 or 4 is clean;
  - FS19 with flag 2 is clean;
  - a save of the reset document makes it clean.
  - The legacy and unknown-version cases stay clean and read-only: the existing read-only test also asserts `!is_dirty()`. Add an unknown-version variant to it.

- Built as planned. The legacy and unknown-version test now loads configs with flags 2 and 4, so it shows that read-only configs never report a reset.

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 1 low, 0 false.
  - low, patch: no `config_xml` test asserted that a flag-free FS25 config reports no reset, although these notes promised one. Added an FS25 case with flags 0/1 to `fs22_and_fs25_reset_spline_flags` that asserts `!flags_reset()`.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: no diff
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: no warnings
- `cargo test --workspace` -- expected: all pass, including both golden tests (the fixture has no 2/4 flags, so it stays clean and byte-identical)
