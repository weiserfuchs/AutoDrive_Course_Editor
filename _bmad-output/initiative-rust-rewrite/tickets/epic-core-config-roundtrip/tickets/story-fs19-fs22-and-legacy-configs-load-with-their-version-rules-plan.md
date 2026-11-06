---
title: 'FS19, FS22 and legacy configs load with their version rules'
type: 'feature'
ticket: '6'
created: '2026-11-06'
status: 'built'
baseline_revision: 'ac28086d324cd66cb00f3eecc34c674dd12fae15'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: ['{project-root}/_bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The loader ignores the game version.
- FS22/FS25 configs keep AutoDrive's spline flags 2 and 4, which the editor should treat as regular.
- Legacy (root `<markerID>`) and unknown-version configs open fully editable and can be saved, which would rewrite a format the editor does not understand.

**Approach:** Implement AD-11 in `ad-io`:
- Detect `GameVersion` once at load from the major of the root `<version>`, and apply the flag rule per version.
- Mark legacy and Unknown configs read-only: save is refused and every command is refused.
- `ad-app` shows a notice and disables Save for read-only configs.

## Boundaries & Constraints

**Always:**
- Major 1 → FS19, 2 → FS22, 3 → FS25. Anything else is Unknown: a missing `<version>`, an unparsable value, or another major.
- Version detection happens once in `ad-io::config_xml::load`. No other crate branches on the version.
- FS22/FS25: flag values 2 and 4 become 0 at load. FS19, Unknown and legacy keep every flag value as read.
- A config is read-only when the root has a `<markerID>` child (start or empty tag), or the version is Unknown. The reason is kept: `Legacy` or `UnknownVersion(text)`.
- `save`/`save_to` on a read-only config return `Err(ConfigError::ReadOnly)` and write nothing.
- The FS25 fixture stays byte-identical (`golden_roundtrip` and `golden_edited` unchanged).
- No unwrap or expect. A malformed `<version>` is never a load error.
- Decision (user, 2026-11-06): commands are refused through a gate on `History`.
  - `History::set_read_only(bool)`. While it is set, `apply` returns `Err(NetworkError::ReadOnly)` and pushes nothing, and `undo`/`redo` return `false`.
  - Story 7's `Document` sets it from `LoadedConfig::read_only()`. The app has no `History` yet, so it only disables Save.

**Never:**
- `vehicles.xml` paths, map-image crop, mod-zip `descVersion` (AD-11 lists them, but later epics own them).
- `Document`, epochs, background load (story 7).
- Converting legacy configs to the current format.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| FS19 | `<version>1.1.0.8</version>`, flags `0,2,4,1` | FS19, editable; flags kept; unchanged save is byte-identical | none |
| FS22 | `<version>2.0.0.5</version>`, flags `0,2,4,1` | FS22, editable; flags `0,0,0,1`; save writes `<flags>0,0,0,1</flags>` | none |
| FS25 | `<version>3.0.1.4</version>`, flags `0,2,4,1` | FS25, as FS22 | none |
| Flag 7 on FS25 | flags `0,7` | kept as 7 (existing test unchanged) | none |
| Unknown major | `<version>4.0</version>` | Unknown, read-only `UnknownVersion("4.0")`, flags kept | save → `Err(ReadOnly)` |
| Missing / garbage version | no `<version>`; `<version>abc</version>` | Unknown, read-only | save → `Err(ReadOnly)` |
| Legacy | root has `<markerID>…</markerID>` or `<markerID/>` | read-only `Legacy`, network loaded for viewing | save → `Err(ReadOnly)` |
| Command refused | read-only config, any transaction | network unchanged, nothing pushed | `Err(ReadOnly)` |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/config_xml.rs`:
  - `load` (≈l.276) walks depth-1 children. Non-modelled ones go through `read_to_end`, and `Event::Empty` at depth 1 hits `_ => {}`.
    - Capture the `<version>` text here, and detect `markerID` from both `Start` and `Empty` events.
  - `build_network` (≈l.202) parses flags as `u8`. Apply the FS22/25 reset to `flags` before `LoadedNode` is built.
  - `LoadedConfig` (≈l.65): add `game_version` and `read_only: Option<ReadOnlyReason>`, with getters.
  - `save` (≈l.495) and `save_to` (≈l.529):
    - `save` returns `Result<Vec<u8>, ConfigError>`.
    - Update its callers: `save_to`, the unit tests' `roundtrip` helper and the other tests, `golden_roundtrip.rs` and `golden_edited.rs`.
  - The test helper `config()` (≈l.569) writes `<version>3</version>` → FS25. The existing tests stay valid.
- `crates/ad-core/src/history.rs`, `network.rs`:
  - The read-only gate goes in `History`, with a new `NetworkError::ReadOnly` variant.
  - `History::apply` returns `Result<EntryId, NetworkError>`.
  - The app holds no `History` yet, and none is added here: a field nobody reads trips `dead_code`. Story 7's `Document` wires the gate.
- `crates/ad-app/src/main.rs`:
  - `save` (l.37) and the top bar (≈l.110): disable Save when `read_only()` is `Some`.
  - Show the reason as a label in the top bar.
  - Legacy text, from Java `GameXML.java:178-179`: "This config format is no longer supported (pre AutoDrive 1.1.0.6). Reload it in the game and save to update it. Viewing only."
- Java reference: `src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:177-200` (detection) and `:345-351` (flag reset).
  - Java leaves the version unset for legacy and keeps a stale one for other majors. Do not port that (AD-13).

## Tasks & Acceptance

**Execution:**
- [ ] `crates/ad-io/src/config_xml.rs`:
  - `pub enum GameVersion { FS19, FS22, FS25, Unknown }` and `pub enum ReadOnlyReason { Legacy, UnknownVersion(String) }`.
  - Detection, the flag reset, the read-only state, and `ConfigError::ReadOnly(ReadOnlyReason)` from `save`/`save_to`.
- [ ] `crates/ad-io/src/config_xml.rs` (unit tests) -- one test per matrix row except "Command refused".
- [ ] `crates/ad-io/tests/golden_roundtrip.rs`, `golden_edited.rs`:
  - Adapt them to `save` returning `Result`.
  - Assert that the fixture is FS25 and editable.
- [ ] `crates/ad-core/src/network.rs`, `history.rs`:
  - Add `NetworkError::ReadOnly`, `History::set_read_only` and `is_read_only`, and gate `apply`, `undo` and `redo`.
  - Test the "Command refused" row: the network is unchanged and `can_undo` is false.
- [ ] `crates/ad-app/src/main.rs`:
  - A read-only label and a disabled Save.

**Acceptance Criteria:**
- Given a legacy config opened in the app, when it is shown, then the network is drawn, a read-only notice names the reason, and Save is disabled.
- Given the FS25 fixture, when it is loaded, then it is FS25, editable, and saves byte-identically.

## Implementation Notes

## Plan Change Log

## Review Triage Log

| # | Finding | Verdict | Route | Evidence |
|---|---------|---------|-------|----------|
| 1 | The FS22/FS25 flag reset also runs on a legacy config whose `<version>` major is 2 or 3 | low | patch | `build_network` matched on the version only, and the frozen rule says legacy keeps every flag as read. The fix applies the reset only when the config is not read-only, and the legacy test now asserts flags `0,2,4,1`. |
| 2 | A `<version>` with an unknown entity fails the load | low | reject | Real: `read_elem` rejects `&foo;`, which goes against "a malformed `<version>` is never a load error". Only a broken file reaches it, and the fix would have to resynchronise the reader after the error. An EOF inside `<version>` makes the whole document malformed, so that error is correct. |
| 3 | `can_undo`/`can_redo` report true while the read-only gate is set | low | patch | `undo`/`redo` returned false, but the queries ignored the gate, so story 7's UI would enable dead buttons. Both now return false while the gate is set, and the test asserts it. |
| 4 | A missing `<version>` shows the notice `Unknown game version "". Viewing only.` | low | reject | Cosmetic. The game always writes `<version>`, and a clearer text needs a branch in the message. |

## Design Notes

- **Major parse:** `text.trim().split('.').next()` parsed as `u32`. `"3"` and `"3.0.1.4"` both give 3.
- **Flag reset on FS22/25 is lossy by design.** A saved FS22 file writes 0 where the file had 2 or 4, exactly like the Java editor. The fixture has only 0 and 1.
- **Legacy `<waypoints>` may carry children the writer does not know.**
  - Today `build_network` rejects them (`UnexpectedElement`), because saving would drop them.
  - A read-only config is never saved, so ignore unknown children when the config is read-only. That way legacy files still open for viewing.
  - The read-only state is known before `build_network` runs, because `<version>` and `<markerID>` precede `<waypoints>`. If they don't, set it after the loop and parse waypoints afterwards: store the `Elem`, call `build_network` after the loop (it already does).
- **`GameVersion` lives in `ad-io`** for now. Story 7 introduces `Document` (AD-11 stores the version there) and may move the type to `ad-core`.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and the new version tests

**Manual checks (if no CLI):**
- Copy the fixture to a temp dir, replace `<version>3.0.1.4</version>` with `<markerID>1</markerID>`, and open it in `cargo run -p ad-app --release`. The network shows, the read-only notice appears, and Save is disabled.
