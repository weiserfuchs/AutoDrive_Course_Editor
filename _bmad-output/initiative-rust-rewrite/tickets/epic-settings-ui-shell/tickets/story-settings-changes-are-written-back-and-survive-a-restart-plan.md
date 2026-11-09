---
title: 'Settings changes are written back and survive a restart'
type: 'feature'
ticket: '1'
created: '2026-11-09'
status: 'built'
baseline_revision: '4cd51f1'
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

**Problem:** `settings.toml` is written only when it is missing (`startup::load_settings`), so no change survives a restart; the grid, snap, continuous and lasso toggles are session-only, and a newly opened map gets no `[maps.<name>]` entry.

**Approach:**
- `GlobalSettings` gains `snap`, `continuous` and `lasso` (default off, as Java); the app starts with them and with `show_grid`.
- At the end of every frame the app copies these toggles into its settings and gives a map seen for the first time its entry with the values it opened with (scale, node size, grid). When the settings differ from the last written state, `settings.toml` is written atomically. Writing at the end of the changing frame also covers exit, so there is no separate exit hook.
- A settings file that could not be read is never written back (startup hands no write path); a missing one gets the defaults written, as before.
- Later stories put more values into `Settings`; they persist through the same path without extra code.
- The user started this story while away; the plan was not approved separately, and the hand check is pending.

## Boundaries & Constraints

**Always:** atomic write (AD-16 pattern); per-map values keyed by map name (AD-14).

**Never:** no settings UI (story 4); no change of an existing map entry by opening it.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Unchanged | no toggle changed | no write | none |
| Toggle | snap, grid, lasso switched on | file holds them after the frame | write error logged |
| New map | map "Neu" opened, no entry | entry with the opened values | none |
| Known map | map "Known" with scale 4 | entry unchanged | none |
| Restart | the written file | toggles restored | none |
| Broken file | unparsable `settings.toml` | defaults used, file never written | warning in the log |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/settings.rs` -- `snap`, `continuous`, `lasso` in `GlobalSettings`.
- `crates/ad-app/src/startup.rs` -- `Startup::settings_path`; `load_settings` returns `None` for a broken file; test.
- `crates/ad-app/src/main.rs` -- `settings_path`, `settings_written`, `sync_settings`, `persist_settings` at the end of `ui`; toggles start from the settings; test.

## Tasks & Acceptance

**Execution:**
- [x] settings fields and startup path -- test Broken file
- [x] write back and restore -- tests Unchanged, Toggle, New map, Known map, Restart

**Acceptance Criteria:**
- Given the app, when the user switches grid, snap, continuous or lasso and opens a map, then after a restart the toggles are as left and `settings.toml` has an entry for that map.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass

## Implementation Notes

- The write runs on the UI thread: the file is a few hundred bytes and changes only on a user action, and a background write could finish after a newer one. AD-9 reserves background threads for long work.
- A failed write is logged once per change; the next change tries again.
- Verification: fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 430 + 4 ignored).

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: tests construct `App` without a path, so no test writes the user's settings; the first frame writes nothing (the written state starts as the loaded one).
