---
title: 'A routes list opens an exported route'
type: 'feature'
ticket: '3'
created: '2026-11-09'
status: 'built'
baseline_revision: '89e6880'
route: 'full'
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

**Problem:** A route can only be opened by finding its random 16-character file name in `routesManager/routes/`; the route manager's `routes.xml` names each route, but nothing shows it.

**Approach:**
- A top-bar button "Routes…" (the editor has no File menu; it sits beside "Open…") picks a `routes.xml`. It is read on a background thread (AD-9). A read or parse error is shown in the error window, and no list window opens.
- A modal window lists its routes (name, map, revision, date), sorted by name ignoring case. A click selects a row. "Open" or a double click opens it; "Close", Escape or a click beside the window closes it.
- Opening loads `<routes.xml folder>/routes/<fileName>` through the normal load. Unsaved changes ask Save, Discard or Cancel first, as Open does. The map name comes from that same `routes.xml` (story 11.1's lookup two folders up).
- A missing route file fails the load with the load error naming the file and keeps the current document (AD-12).
- The user started this story while away; the plan was not approved separately, and the hand check is pending.

## Boundaries & Constraints

**Never:** no editing of `routes.xml` entries, no delete or upload (epic boundary).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| List | routes "c route", "b route", "A route" | rows A, b, c with map, revision, date; serverId read | none |
| Open | "b route" selected, clean document | route loaded, path `routes/<fileName>`, map name from the list | none |
| No row | Open without a selection | nothing | none |
| Unsaved | dirty document | prompt for `OpenRoute(path)`; Discard loads | none |
| Missing | listed file absent | error naming the file, document unchanged and still dirty | error |
| Broken list | malformed or missing `routes.xml` | error naming it, no window | error |
| Escape | window shown | closes, opens nothing | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/jobs.rs` -- `JobResult::RoutesRead`, `read_routes_list` (sorted), `spawn_routes_read`, `route_file`.
- `crates/ad-app/src/main.rs` -- `Pending::OpenRoute(PathBuf)` (Pending is no longer `Copy`), `App::load` split out of `open`, `pick_routes_list`, `start_routes_read`, `open_route`, `routes`/`reading_routes` state, `dialog_shown` includes the window; tests.
- `crates/ad-app/src/ui.rs` -- the "Routes…" button, `RoutesWindow` and `routes_window`; tests.

## Tasks & Acceptance

**Execution:**
- [x] list read and sort -- tests List, Broken list
- [x] window and open -- tests Open, No row, Unsaved, Missing, Escape, top bar wrap includes "Routes…"

**Acceptance Criteria:**
- Given the generated `target/handtest/routesManager/routes.xml`, when the user picks it with "Routes…" and opens "Test route", then the route opens on the NF_Marsch_4fach map.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass

## Implementation Notes

- The ticket says "File item"; the editor's top bar has no File menu, so "Routes…" is a button beside "Open…" (tooltip "Open a route from an AutoDrive routes.xml").
- The missing-file case uses the normal load error rather than a separate check, so it also covers a file removed between listing and opening.
- Verification: fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 428 + 4 ignored).

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline) of the diff: no finding. Checked: the window is part of `dialog_shown`, so shortcuts and map clicks wait while it is shown; Open is disabled while loading or saving; `open_route` keeps the window when no row is selected.
