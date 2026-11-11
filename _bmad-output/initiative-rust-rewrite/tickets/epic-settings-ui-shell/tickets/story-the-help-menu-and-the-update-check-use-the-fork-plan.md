---
title: 'The help menu and the update check use the fork'
type: 'feature'
ticket: '9'
created: '2026-11-09'
status: done
baseline_revision: 'a3ddffb'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:**
- The Rust editor has no update check and no Help menu.
- Java's update check and version history read upstream's `version.xml` and `history.xml`, which the fork must never use.

**Approach:**
- **ad-io `update`:**
  - `fetch_latest` asks `api.github.com/repos/weiserfuchs/AutoDrive_Course_Editor/releases/latest` with a generic User-Agent and a 20 s timeout, and reads `tag_name`, `html_url` and `body` (serde_json, new dependency).
  - `is_newer` compares version numbers ("v1.2.10" > "1.2.9"; a tag that is no version is never newer).
- **At start:** unless "Look for a newer release at start" (Settings ▸ Editor) is off, the check runs on a background thread (AD-9).
  - A release newer than this build that was not shown before opens "Update available" with its notes, "Open release page" and Close. Closing marks it seen, so each version is shown once.
  - Any failure is only logged.
- **Help menu:**
  - About… shows the version, the fork link and the credits from Java's About.
  - Version history opens the fork's releases page.
  - How to create map images opens the map-image data repository's help page.
  - Open log folder opens the log folder in the file manager.
- The user started this story while away; the plan was not approved separately, and the hand check is pending.

## Boundaries & Constraints

**Always:** no URL at the upstream editor repository (AGENTS.md); the map-image data repository is the one allowed KillBait link.

**Never:** no download or install of a release; no use of the repo-root `version.xml` or `history.xml`.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Compare | v0.2.0 vs 0.1.0, 0.1 vs 0.1.0, nightly | newer, same, never newer | none |
| Answer | JSON with tag, url, body / without body | read | none |
| Bad answer | 404, HTML, nothing listening | error text | logged |
| Newer | v999.0.0 not seen | window; Close marks it seen | none |
| Not shown | seen, same, older, no version, failure | nothing | logged |
| Fork only | every `.rs` and `.toml` under `crates/` | no upstream editor URL | test fails |

</frozen-after-approval>

## Code Map

- `Cargo.toml`, `crates/ad-io/Cargo.toml` -- `serde_json` 1.0.154.
- `crates/ad-io/src/update.rs` (new) -- URLs, `Release`, `version_numbers`, `is_newer`, `fetch_latest`; tests with the local test server.
- `crates/ad-io/src/settings.rs` -- `check_updates`, `update_seen`.
- `crates/ad-app/src/startup.rs` -- `Startup::log_dir`.
- `crates/ad-app/src/jobs.rs` -- `JobResult::UpdateChecked`, `spawn_update_check`.
- `crates/ad-app/src/main.rs` -- `update_url` (set only by `main`), `update_checked`, `close_update`, `open_log_folder`; tests incl. the upstream URL scan.
- `crates/ad-app/src/ui.rs` -- Help menu, `about_window`, `update_window`, the setting; test.

## Tasks & Acceptance

**Execution:**
- [x] ad-io update -- tests Compare, Answer, Bad answer
- [x] app -- tests Newer, Not shown, Fork only
- [x] help menu and windows

**Acceptance Criteria:**
- Given a fork release newer than the running build, when the app starts, then "Update available" shows it once; the Help menu items open their pages and the log folder.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- The build version is ad-app's `CARGO_PKG_VERSION` (0.1.0 today). Epic 10 sets the release version and tags, so until a release exists the check finds none (404, logged).
- `App::new` never sets `update_url`; only `main` does, so no test asks GitHub.
- The Java `VersionManager` URLs at the upstream repository stay in the Java reference code, which epic 10 removes.
- Verification: fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 455 + 4 ignored, ad-io 94); frame time ok; `cargo tree -p ad-core` has no serde or HTTP crate.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: the User-Agent is generic (no user data); a failed or slow check never blocks the UI (background thread, 20 s cap); closing the window by its title-bar button also marks the release seen.
