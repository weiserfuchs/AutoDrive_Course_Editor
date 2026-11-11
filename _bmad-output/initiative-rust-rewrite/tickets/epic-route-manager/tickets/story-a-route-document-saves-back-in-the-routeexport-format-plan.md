---
title: 'A route document saves back in the routeExport format'
type: 'feature'
ticket: '2'
created: '2026-11-09'
status: done
baseline_revision: 'c8676ef'
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

**Problem:** Story 11.1 opens route-manager files but cannot write them: Save shows a notice, autosave skips routes and a merge on a route is refused for lack of a backup.

**Approach:**
- **ad-io `routes_xml::save(network, groups)`** writes `routeExport` as the mod's `writeGraphToXml` does: `waypoints#c`; `x`, `y`, `z` as `%.2f`, `out` and `in` as comma lists (`-1` for none) and `flags`, each joined by `;`; `markers/m` with `i` (1-based node id from `SaveIdMap`, AD-2), `n`, `g`; `groups/g` with `n`, `i`. Attribute values are escaped.
- **Groups:** the document's group list as read (names and ids kept, also groups no marker uses), then each marker group not in it, in marker order, with fresh ids after the highest one.
- **One writer entry:** `config_xml::save` writes a route document with the route writer, so Save, Save As, autosave and the merge backup all write route files for a route (atomically, through `save_to`); `vehicles.xml` is never patched for a route (a route loads without one).
- The 11.1 guards go: the Save notice, the autosave skip and the merge refusal.
- The user started this story while away ("Mach bitte weiter, ich kann grad nicht testen"); the plan was not approved separately, and the hand check of 11.1 and 11.2 is pending.

## Boundaries & Constraints

**Always:** atomic writes (AD-16); file ids only in ad-io (AD-2).

**Never:** no `routes.xml` edits (name, revision, date: epic boundary); no routes list window (11.3); no change to config saving.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Round trip | route loaded, node moved, node deleted, node added and linked, marker with quotes and `&` | load(save) equals the edited model at 2 decimals | none |
| Groups kept | list has a group no marker uses | written with its id | none |
| New group | a marker uses a group not in the list | appended with max id + 1 | none |
| Empty | no nodes, no groups | loads back empty | none |
| Save | Save on a route | route file written, document clean, `routes.xml` untouched, no `vehicles.xml` | as for configs |
| Autosave | due autosave of a dirty route | `<stem>_autosave_<n>.xml` in route format | as for configs |
| Merge | merge on a route | `<stem>_mergeBackup.xml` in route format, then the merge | as for configs |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/routes_xml.rs` -- `save`, `groups_to_write`, attribute escaping; tests.
- `crates/ad-io/src/config_xml.rs` -- `save` hands a route document to the route writer; `ConfigError::Route` removed.
- `crates/ad-app/src/main.rs` -- the three 11.1 guards removed; tests for save, autosave and merge on a route.
- `crates/ad-app/src/jobs.rs` -- the 11.1 notices removed; the test writer `test_route_text` now calls the real writer.
- `crates/ad-app/src/document.rs` -- `Document::is_route` removed (no caller left).

## Tasks & Acceptance

**Execution:**
- [x] route writer and group rule -- tests Round trip, Groups kept, New group, Empty
- [x] app -- tests Save, Autosave, Merge
- [x] hand-test files regenerated with the real writer

**Acceptance Criteria:**
- Given the generated `routesManager` folder, when the user opens `routes/TestRoute0000001.xml`, edits it, saves and reopens it, then the edits are there.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass

## Implementation Notes

- **"All is id 1":** the writer keeps the ids as read and does not apply the mod's renumbering. The mod renumbers on its own read (`readGraphFromXml`), so a file without "All" at id 1 still loads in the game the same way; renumbering in the editor would only change ids the game rewrites anyway.
- **Renamed groups:** `RenameGroup` changes the markers only; the old name stays in the route's group list (an empty group, as the epic keeps groups no marker uses) and the new name is appended with a fresh id.
- **Save As** writes the route format to any path; it does not add the file to a `routes.xml` (epic boundary).
- **Output layout:** 4-space indentation and LF, as the 11.1 test writer; byte-identical output to the game's writer is out of scope until a real export exists.
- **Verification:** fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 424 + 4 ignored, ad-io 90). The hand-test route was regenerated with the real writer (`jobs::route_tests::write_handtest_route_manager`).

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline) of the diff: no finding. Checked: every caller of `config_xml::save_to` (save, autosave, merge backup) now writes a route for a route document; `vehicles_to_patch` is `None` for a route (no vehicles loaded); markers whose node is gone are skipped as in the config writer.
