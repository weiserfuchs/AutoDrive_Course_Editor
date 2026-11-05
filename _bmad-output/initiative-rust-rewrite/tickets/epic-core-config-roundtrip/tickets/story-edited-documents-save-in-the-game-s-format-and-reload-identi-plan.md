---
title: "Edited documents save in the game's format and reload identically"
type: 'feature'
ticket: '5'
created: '2026-11-05'
status: 'built'
baseline_revision: '29e753c44d29c6c0b7d31d21dd2c8b97426eedc4'
route: 'full'
route_source: 'auto'
risk: 'high'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: ['{project-root}/_bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Story 2 proved the unedited round trip only. Nothing yet shows that an edited network (delete, add, connect, marker change) saves in the game's format and reloads to the same model. Also, file ids are assigned ad hoc inside `save`, and a crash during a save can leave the target file half-written.

**Approach:**
- Make the save path in `ad-io` edit-safe: a public `SaveIdMap` built from `order` is the only source of file ids (AD-2).
- Cover the missing `<flags>` and missing `<mapmarker>` paths with tests.
- Add a `save_to(path)` that writes a temp file in the target directory and renames it into place (AD-6).
- Add the second golden test: edit the fixture through `History`, save to a temp dir, reload, compare models, and check that untouched bytes are identical.

## Boundaries & Constraints

**Always:**
- Saving the unedited fixture stays byte-identical (`golden_roundtrip` unchanged).
- File ids are 1..N in `order`. Every `out`/`incoming` entry and marker id comes from `SaveIdMap`.
- Coordinates are written with exactly 3 decimals, marker ids as `N.000000`, `,` and `;` as in story 2, and `-1` for an empty list.
- A missing `<flags>` element is generated with all zeros. A missing `<mapmarker>` is inserted before `</AutoDrive>` when markers exist, using the source's indent and newline.
- All bytes outside the `<waypoints>` and `<mapmarker>` blocks are written unchanged.
- `save_to` writes `<target>.tmp…` in the target's directory and renames it over the target. On any error the target is untouched and the temp file is removed. It returns `ConfigError::Io` naming the path.
- `ad-app`'s Save uses `save_to`.
- Edits in tests go through `History`/`Transaction`, never through raw `Network` methods (AD-4).
- Decision (user, 2026-11-05):
  - A list entry or marker that refers to a node no longer in the network is skipped at save, so an emptied list is written as `-1`.
  - Save never writes `0` or any other invalid id.

**Never:**
- Background or async save, `saved_at` wiring in the app, autosave and backups (stories 7 and 8).
- Version-specific handling (story 6) and `vehicles.xml`.
- Repairing `out`/`incoming` inconsistencies.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Edited fixture | Fixture edited in one transaction: delete a Reverse-link target, delete a marked node, add a node, connect it Dual and OneWay to existing nodes, rename one marker and add one | saved file reloads to an equal model (same positions, flags, adjacency in order, markers in order); bytes before `<waypoints>`, between the blocks and after `<mapmarker>` equal the original | none |
| Renumbering | delete node 2 of 3 | node 3 is written as id 2 everywhere (`id`, `out`, `incoming`, marker id `2.000000`) | none |
| Missing flags | small config without `<flags>`, unchanged save | `<flags>0,…</flags>` generated after the last child; reload has all flags 0 | none |
| Missing mapmarker | small config without `<mapmarker>`, add a marker, save | block inserted before `</AutoDrive>` with source indent and newline; reloads with the marker | none |
| All markers removed | config with one marker, remove it | `<mapmarker>` block written empty (open tag, newline, indented close tag) and reloads with no markers | none |
| Dangling reference | node A lists only `incoming` from B; delete B; save | A's `incoming` written without B (`-1` if empty); file reloads | none |
| Atomic write ok | `save_to` into a temp dir with an existing target | target replaced with the new bytes; no temp file left | none |
| Atomic write fails | target directory does not exist | no file created | `Err(ConfigError::Io)` naming the path |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/config_xml.rs`:
  - `save` (≈l.456) builds a local `file_ids: HashMap<NodeId, usize>` and passes it to `waypoints_block`, `mapmarker_block` and `join_nodes`.
  - `join_nodes` and `mapmarker_block` fall back to `unwrap_or(0)` for unknown ids. With edits, that means dangling references; see Open Questions.
  - The missing-`<mapmarker>` insertion branch and the flags generation exist but are untested.
- `crates/ad-io/tests/golden_roundtrip.rs` -- keep it as is. Add `crates/ad-io/tests/golden_edited.rs` for the edited test.
- `crates/ad-core/src/command.rs`, `history.rs` -- story 4's API for the edits (`Transaction`, `Command::*`, `History::apply`).
  - `ad-io`'s `LoadedConfig.network` is `pub`, so tests apply edits to it through `History`.
- `crates/ad-app/src/main.rs` -- `save` calls `std::fs::write`. Switch it to `config_xml::save_to`.
- `crates/ad-core` -- no change expected.

## Tasks & Acceptance

**Execution:**
- [ ] `crates/ad-io/src/config_xml.rs` -- add `pub struct SaveIdMap` with `from_network(&Network)` and `get(NodeId) -> Option<usize>`, and use it in `save`. Apply the dangling-reference decision. Add `ConfigError::Io { path, source }` and `save_to(&LoadedConfig, &Path) -> Result<(), ConfigError>` with temp file and rename.
- [ ] `crates/ad-io/src/config_xml.rs` (unit tests) -- one test each for the Renumbering, Missing flags, Missing mapmarker, All markers removed, Dangling reference, Atomic write ok and Atomic write fails rows.
- [ ] `crates/ad-io/tests/golden_edited.rs` -- the Edited fixture row.
  - A model-equality helper compares by order index: x, y, z, flag, `outgoing`/`incoming` as order indexes, and markers as (order index, name, group) in order.
  - Added coordinates use values with at most 3 decimals.
- [ ] `crates/ad-app/src/main.rs` -- Save writes through `save_to`.

**Acceptance Criteria:**
- Given the edited fixture saved and reloaded, when the models are compared, then they are equal, and the untouched byte ranges are identical.
- Given the app with the fixture opened, when the user saves over the file, then it is written through temp file plus rename and is still byte-identical.

## Implementation Notes

- Deviation accepted by the user (2026-11-06):
  - The edited golden test applies `AddNode` in its own transaction and the remaining edits in a second one.
  - `Connect` needs the new node's id, which exists only after `AddNode` applies.
  - Same-transaction add-and-connect is left to the tools epic.

## Plan Change Log

## Review Triage Log

| # | Finding | Verdict | Route | Evidence |
|---|---------|---------|-------|----------|
| 1 | The dangling-reference test never reaches the skip logic | false | reject | In the test, node 1 has `out -1` and node 2 lists `incoming 1`. `remove_node(1)` finds no referrers (its own lists are empty, and no edge ends at it), so node 2 keeps the stale entry until save. With `unwrap_or(0)` the assert on `<incoming>-1</incoming>` would fail. |
| 2 | The renumbering test never checks list entries | low | patch | After the delete no surviving list references node 3. The fix adds a 1–3 link and asserts it is written as `2`. |
| 3 | The edited golden test is two transactions, not one | — | accepted | The user accepted the deviation; see Implementation Notes. |
| 4 | The edited golden test does not assert the edits themselves | low | patch | Only model equality and `written != original` are checked. The fix asserts markers and the new node's link kinds. |
| 5 | `save_to` does not fsync before the rename | medium | patch | After a power loss the rename can be persisted before the data, leaving an empty config. The fix calls `sync_all` on the temp file. |
| 6 | `save_to` drops the target's permissions and replaces a symlink | low | reject | The game config is a plain user file. Preserving metadata would add platform-specific code. |
| 7 | The atomic-write test assumes a fresh pid-named directory | low | reject | It breaks only on a leftover directory with the same pid from a crashed run. |
| 8 | The unedited fixture through `save_to` has no automated test | low | reject | `save_to` writes `save`'s bytes, which `golden_roundtrip` covers, and the atomic path has its own unit test. The manual check covers the app. |

## Design Notes

- **Untouched-bytes check in the golden test.**
  - Locate `<waypoints>`…`</waypoints>` and `<mapmarker>`…`</mapmarker>` by byte search in both files.
  - Assert that the prefix, the middle and the suffix are equal.
  - The fixture has both blocks, waypoints first.
- **Temp name:** `.{file_name}.{process id}.tmp` in the same directory. Rename replaces the target on Windows and Unix.
- **Empty mapmarker block:** `"<mapmarker>" + nl + base_indent + "</mapmarker>"`, which is what `mapmarker_block` already produces for zero markers.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and the new `config_xml` tests

**Manual checks (if no CLI):**
- `cargo run -p ad-app --release`: open `AutoDrive_config.xml`, Save over it, then `git status` shows the fixture unchanged and no `.tmp` file is left.
