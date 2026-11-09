---
title: 'The release build is a windowed AutoDriveCourseEditor.exe, version 0.9.0'
type: 'chore'
ticket: '1'
created: '2026-11-09'
status: 'built'
baseline_revision: '5609684'
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

**Problem:** the release exe is `ad-app.exe`, opens a console window next to the editor, has version 0.1.0 and no release profile.

**Approach:**
- `[[bin]] name = "AutoDriveCourseEditor"` in ad-app; the package stays `ad-app`, so `cargo run -p ad-app` and `cargo test -p ad-app` are unchanged.
- `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`: release builds run without a console; debug builds keep it for development.
- `[workspace.package] version = "0.9.0"`, used by all three crates; About and the update check read it (user decision).
- `[profile.release]`: thin LTO, one codegen unit, stripped symbols.
- The user started this epic while away; the plan was not approved separately, and the hand check is pending.

</frozen-after-approval>

## Code Map

- `Cargo.toml` -- workspace version, release profile.
- `crates/*/Cargo.toml` -- `version.workspace = true`; ad-app's `[[bin]]`.
- `crates/ad-app/src/main.rs` -- `windows_subsystem`.

## Implementation Notes

- Without a console nothing is lost: logging already writes only to the log folder, and the panic hook shows a message box (startup.rs).
- Thin rather than fat LTO keeps `cargo test --release` (frame-time test) and `cargo run --release` builds acceptable.
- Verification: fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 456 + 4 ignored); `cargo build --release -p ad-app` yields `target/release/AutoDriveCourseEditor.exe` (21 MB) whose PE subsystem is 2 (Windows GUI).

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo build --release -p ad-app` -- expected: `target/release/AutoDriveCourseEditor.exe`

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: no code reads the binary name; `CARGO_PKG_VERSION` now yields 0.9.0, and the update-check tests build their versions from it.
