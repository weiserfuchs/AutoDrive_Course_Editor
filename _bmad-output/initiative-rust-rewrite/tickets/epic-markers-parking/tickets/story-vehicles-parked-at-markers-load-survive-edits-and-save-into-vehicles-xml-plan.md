---
title: 'Vehicles parked at markers load, survive edits and save into vehicles.xml'
type: 'feature'
ticket: '1'
created: '2026-11-08'
status: 'planned'
baseline_revision: '73c4e39'
route: 'full'
route_source: 'auto'
risk: 'high'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The savegame's `vehicles.xml` refers to markers by their 1-based position in the config's marker list: `parkDestination` (where a vehicle parks), `firstMarker` and `secondMarker` (its destinations). The editor ignores the file, so deleting a marker silently shifts every later reference to the wrong marker. Java (`GameXML.loadVehiclesXMLParking`, `saveVehiclesXMLParking`) updates only `parkDestination`, reformats the whole file, and skips the update when no vehicle parks.

**Approach (AD-17, AD-13):**
- **Model:** `Marker` gains `parked_vehicles: Vec<String>` (vehicle ids). `EditMarker` keeps a marker's parked vehicles when it changes name or group, and a revert restores a removed marker with them; `DeleteNodes` and its revert do the same. A new marker has none; paste copies none.
- **Load:** `ad-io::vehicles` reads `vehicles.xml` from the config's folder, if present. Vehicle id: FS25 `uniqueId`, FS19/FS22 `id`; AutoDrive element: FS19 `FS19_AutoDrive/AutoDriveVehicleData`, FS22/FS25 `AutoDrive`. Each `parkDestination = k` adds the vehicle to marker k's `parked_vehicles`. Each `firstMarker`/`secondMarker = k` is kept as a reference to the node of marker k. An out-of-range value is kept as it is and never rewritten. A missing or unparsable file loads as none, with a notice for an unparsable one; the config still opens.
- **Save:** only when the target is the opened config's own path (a real Save, not a save to another file), `vehicles.xml` beside it is patched in place and written atomically (AD-6):
  - Only the values of `parkDestination`, `firstMarker` and `secondMarker` change; every other byte stays.
  - A vehicle's `parkDestination` is the 1-based index, in marker order, of the marker that lists it. A vehicle no marker lists loses the attribute (the attribute text with its leading space is removed). A vehicle whose marker lists it but that had no attribute gets none added: only the game assigns new parking.
  - `firstMarker`/`secondMarker` become the index of the marker now on the referenced node, or `1` when that node has no marker any more.
  - No file beside the config: nothing is written.
- **Display:** a marker whose `parked_vehicles` is not empty draws the parking icon (prepared in story 2.5) on its node.

## Boundaries & Constraints

**Always:** the repo fixtures are never written; tests copy them to a temp dir. Config and `vehicles.xml` save in one job; if the config write fails, `vehicles.xml` is not touched. A `vehicles.xml` error after a successful config write is reported, and the config save still counts.

**Never:** no parking assignment UI; no new vehicles or AutoDrive elements; no change of any other attribute; no write on a save to another file.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Load fixture | fixture + its vehicles.xml | 6 vehicles parked on markers 161, 163, 164, 165, 167, 178; 25 first/second references kept | none |
| Save unchanged | open, Save to the same path (temp copy) | vehicles.xml byte-identical | none |
| Delete before | delete marker 100, Save | every reference above 100 drops by one; nothing else changes | none |
| Delete referenced | delete marker 161 (a park target) and marker 180 (five `firstMarker` targets), Save | 161's vehicle loses `parkDestination`; the five references become `1`; later references shift by two | none |
| Undo | delete marker 161, undo, Save | identical to Save unchanged | none |
| Node delete | delete the node of a parking marker, undo | marker and parked vehicles back | none |
| Save as | Save to another file name | vehicles.xml untouched | none |
| No file | config without vehicles.xml | loads and saves as today | none |
| Broken file | unparsable vehicles.xml | config opens, notice shown, file never written | notice |
| FS19 / FS22 | small samples with each element and id rule | parsed and patched like FS25 | none |
| Icon | marker with parked vehicles | parking icon drawn | none |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/network/markers.rs`, `command.rs` -- `Marker.parked_vehicles`; `set_marker` keeps the old list when replacing; revert paths restore it. Update every `Marker { .. }` construction (about 11 sites).
- `crates/ad-io/src/vehicles.rs` (new) -- `VehiclesFile { original, attrs: Vec<(byte range, kind, vehicle id, value)> }`; `load(bytes, version) -> Result<…>`; `attach(&mut Network, &VehiclesFile)` (park lists, first/second as node refs); `patch(&VehiclesFile, &Network) -> Vec<u8>`. Parse with the same XML reader `config_xml` uses, keeping byte positions.
- `crates/ad-io/src/config_xml.rs` -- only if the version and marker order are needed from `LoadedConfig`; keep it small.
- `crates/ad-app/src/document.rs`, `jobs.rs`, `main.rs` -- the document holds the vehicles file and its path; `load_file` reads it; `spawn_save` patches it after the config, only for the opened path; notices.
- `crates/ad-app/src/view.rs` / `icons.rs` -- draw the parking icon; remove its `dead_code` allow.
- `crates/ad-io/tests/` -- a vehicles round-trip test on copies of both fixtures (Load fixture, Save unchanged, Delete before, Delete referenced, Undo) and FS19/FS22 samples.

## Tasks & Acceptance

**Execution:**
- [ ] model -- `parked_vehicles` through set, replace, remove, node delete and their reverts; tests
- [ ] `vehicles.rs` -- load, attach, patch; tests for every matrix row up to FS19 / FS22
- [ ] app wiring -- load, save rule, notices; tests for Save as, No file, Broken file
- [ ] parking icon -- drawn for parked markers; frame-time test still green

**Acceptance Criteria:**
- Given copies of both fixtures in one folder, when the user opens the config, deletes a marker before a parking one and saves, then `vehicles.xml` differs from the original only in the shifted marker references.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and the vehicles round trip
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
