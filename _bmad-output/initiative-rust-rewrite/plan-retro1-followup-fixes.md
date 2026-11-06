---
title: 'Epic 1 retro follow-up fixes'
type: 'bugfix'
ticket: ''
created: '2026-11-06'
status: 'built'
baseline_revision: '9a05402a951784f654afb66b6f39fbdb507824d1'
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

**Problem:** The epic 1 retrospective (action item 1) found four small defects across stories that the per-story reviews could not see.
- **B1:** Open is gated only on `loading` and Save only on `saving`. So opening a file while it is being saved can show the old content as clean, and a later Save overwrites the edits that were just saved.
- **B3:** No test proves that a read-only config gets a gated `History` through `apply_result`/`Document::new`.
- **P1:** AD-12's `unwrap_used`/`expect_used` deny lints are missing in `ad-app`.
- **D1:** `config_xml::save_to` and `settings::save` each carry a near-identical temp-file-plus-rename writer.

**Approach:**
- Disable Open and Save while any job (load or save) runs.
- Add one `jobs` test that loads a legacy config through `apply_result` and checks that `apply` is refused and `can_undo` is false.
- Add the deny lints to `ad-app`'s `Cargo.toml`.
- Move the writer into one `pub(crate)` function in `ad-io` that both callers use.
  - Error types, messages and behaviour stay as they are.
  - `settings::save` still creates the parent dir first.

The user approved bundling these four fixes into one story (2026-11-06, after the retro).

</frozen-after-approval>

## Implementation Notes

Oneshot: about 80 changed lines across five files, all mechanical.

- New `crates/ad-io/src/atomic.rs` (`pub(crate) fn write`). `config_xml::save_to` and `settings::save` map its `io::Error` into their existing error types. `settings::save` creates the parent dir only when the path has one; before, `create_dir_all(".")` ran, which had no effect.
- `main.rs`: Open needs `!loading && !saving`, and Save needs the same plus an editable document.
- `ad-app/Cargo.toml`: `[lints.clippy]` denies `unwrap_used`/`expect_used`. No production code had to change, and the existing test-module allow now takes effect.
- `jobs.rs`: new test `read_only_config_gets_a_gated_history`.

## Review Triage Log

- **low, patched:** `settings::save` ran `create_dir_all` before the file-name check, so a path without a file name (`a/b/..`) created `a/b` and then failed. The dir is now created only when the path has a file name.
- **low, patched:** the new read-only test checked only `read_only().is_some()` and `is_err()`. It now asserts `ReadOnlyReason::Legacy` and `Err(NetworkError::ReadOnly)`.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including the existing atomic-write tests in `config_xml` and `settings` and the new read-only `jobs` test
