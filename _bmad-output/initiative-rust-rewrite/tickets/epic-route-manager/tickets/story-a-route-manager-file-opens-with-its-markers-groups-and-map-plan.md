---
title: 'A route-manager file opens with its markers, groups and map'
type: 'feature'
ticket: '1'
created: '2026-11-08'
status: done
baseline_revision: '81c3ffc'
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

**Problem:** The editor opens only AutoDrive configs. AutoDrive's route manager exports routes as `routeExport` files; Java opens them (`RoutesXML.loadRouteManagerXML`), and the mod's own `readGraphFromXml` defines the format.

**Approach:**
- **ad-io `routes_xml`**: parse a `routeExport` file (format per the epic's Decision line): `waypoints#c`; `x`, `y`, `z` split by `;`; `out`, `in` per node split by `;`, then `,`, `-1` for none; `flags` (missing or empty → all 0; 2 and 4 reset to 0 as Java and AD-11 for FS22/FS25); markers `markers/m` with `i` (1-based node id), `n`, `g`; groups `groups/g` with `n`, `i`. Fewer values than `c`, an id out of range or a non-number is an error naming the element (AD-12). The result is a `LoadedRoute { network (with markers), groups: Vec<(name, id)> }`.
- **Open…** detects the root element: `AutoDrive` (config, as today) or `routeExport` (route). A route document is editable (no version rules apply; the mod reads what it writes) and has no `vehicles.xml`.
- **Map name:** from `routes.xml` two folders up (`routesManager/routes.xml` for `routesManager/routes/<file>.xml`), the `route` whose `fileName` equals the file name with an `_autosave_<n>` or `_mergeBackup` suffix stripped (Java). No match or no `routes.xml`: no map name, a notice "No map name for this route (routes.xml not found)"; the map stays blank. With a map name, the map image, download and heightmap work exactly as for a config (AD-14).
- The title shows the route file name. Saving a route is story 11.2: until then Save on a route document shows "Saving routes comes with story 11.2" instead of writing (no data loss: the config writer must never be used on a route).
- For the user's hand check, create `target/handtest/routesManager/routes.xml` and `target/handtest/routesManager/routes/TestRoute0000001.xml`: the route file holds the whole fixture network and its markers written to the mod's format (2 decimals), with groups from the fixture's markers; `routes.xml` lists it with `map="NF_Marsch_4fach"`.

## Boundaries & Constraints

**Always:** a failed route load leaves the current document unchanged (AD-12); file ids exist only in ad-io (AD-2).

**Never:** no route save (11.2); no routes list window (11.3); no change to config loading.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Parse | 3 nodes, links incl. -1, flags 0;1;0, one marker, groups "All" and "Felder" | network, marker on node 2 with group, group list [("All", 1), ("Felder", 2)] | none |
| No flags | `flags` missing | all flags 0 | none |
| Flag reset | flags 2 and 4 | 0 | none |
| Short | `c` = 3, two x values | error naming `x` | error |
| Bad id | out entry 7 with c = 3 | error naming `out` | error |
| Detect | Open on a config / a route | config path as before / route path | none |
| Map name | routes.xml lists the file with map M | map name M, image lookup as for configs | none |
| Suffix | `<file>_autosave_2.xml` | matched to `<file>.xml` | none |
| No routes.xml | route file elsewhere | notice, blank map | notice |
| Save guard | Save on a route | notice, nothing written | none |
| Fixture route | the generated route of the whole fixture | same node count, links and markers as the fixture (heights/positions at 2 decimals) | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/routes_xml.rs` (new) -- parse, `routes.xml` reading (`name`, `fileName`, `map`, `revision`, `date`, `serverId`), map-name lookup with the suffix rule; tests.
- `crates/ad-app/src/jobs.rs` (`load_file`) and `document.rs` -- a document kind (config or route) and the route's group list; `main.rs`/`ui.rs` -- Open detection, the Save guard, title, notices.
- A small test (or ignored generator test) that writes the hand-test files from the fixture.

## Tasks & Acceptance

**Execution:**
- [x] `routes_xml` parse and `routes.xml` -- tests for Parse, No flags, Flag reset, Short, Bad id, Map name, Suffix
- [x] app -- tests for Detect, No routes.xml, Save guard, Fixture route
- [x] hand-test files generated under `target/handtest/routesManager/`

**Acceptance Criteria:**
- Given the generated `routesManager` folder, when the user opens `routes/TestRoute0000001.xml`, then the fixture's network and markers appear on the NF_Marsch_4fach map image, and every tool works on it.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Document kind:** a route document is a `LoadedConfig` built by `LoadedConfig::for_route(route, map_name)` (no original bytes, `GameVersion::Unknown`, not read-only, `flags_reset` from the route). It carries the route's group list (`route_groups()`); `is_route()` is the kind, and `Document::is_route()` asks it. This keeps `doc.config.network` and the `Loaded` tuple unchanged and puts the group list in every snapshot, ready for the 11.2 writer.
- **Write guards:** `config_xml::save` refuses a route (`ConfigError::Route`), so the config writer can never write one, whatever the caller. In the app: Save on a route shows "Saving routes comes with story 11.2" (once) before any dialog and returns false; `tick_autosave` skips route documents; `finish_merge` on a route skips the backup (sends `MergeBackup Ok` without writing) and adds "Merged without a backup: route backups come with story 11.2", so the merge tool still works. `spawn_save`, `spawn_autosave` and `spawn_merge_backup` are the only callers of `config_xml::save_to`.
- **Detection:** `routes_xml::is_route` looks only at the root element; anything else goes to `config_xml::load` unchanged, so config errors stay as before.
- **Parsing:** `x`, `y`, `z`, `out`, `in` and a non-empty `flags` must have exactly `c` values (more is an error too, not ignored as in Lua: the mod never writes more); ids out of `1..=c` are errors naming `<out>`, `<in>` or `<m i>`; marker `n` and `g` are required; a group without `i` gets index + 1 (the mod's `getNoNil`). The mod's "All is id 1" renumbering is not applied (deferred to 11.2).
- **Map name:** `routes.xml` two folders up; notice "No map name for this route (…)" with "routes.xml not found", "not listed in routes.xml" or "routes.xml could not be read: …". A `vehicles.xml` beside a route is not read.
- **Hand-test files:** `target/handtest/routesManager/routes.xml` and `routes/TestRoute0000001.xml` (35652 nodes, fixture markers, 17 groups with "All" = 1), written by the ignored test `jobs::route_tests::write_handtest_route_manager` with the test-only writer `jobs::test_route_text`; that test also loads the file back through `load_file` with map name NF_Marsch_4fach.
- **Verification:** `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` all pass (ad-app 424 + 4 ignored, ad-io 88); `frame_time` every case ok, worst best-round max 9.9 ms (rotate, min zoom, whole network).

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator), pass 1: 1 medium patched.
  - medium, patch: a merge on a route ran without a backup, against AD-16 and story 8.2 ("the merge never runs without a written backup"). It is now refused on a route with the notice "Not merged: a merge needs a backup, and route backups come with story 11.2"; story 11.2 adds route backups. Test `a_merge_on_a_route_is_refused_until_routes_have_backups`.
  - accepted: more values than `c` is an error (the mod never writes more); the group list is kept as read (the mod's "All is id 1" renumbering is for 11.2).
- Verification after the patch: fmt, clippy and workspace tests pass (ad-app 424).
