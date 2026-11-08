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
- [x] model -- `parked_vehicles` through set, replace, remove, node delete and their reverts; tests
- [x] `vehicles.rs` -- load, attach, patch; tests for every matrix row up to FS19 / FS22
- [x] app wiring -- load, save rule, notices; tests for Save as, No file, Broken file
- [x] parking icon -- drawn for parked markers; frame-time test still green

**Acceptance Criteria:**
- Given copies of both fixtures in one folder, when the user opens the config, deletes a marker before a parking one and saves, then `vehicles.xml` differs from the original only in the shifted marker references.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and the vehicles round trip
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Model:** `Marker.parked_vehicles: Vec<String>`. `set_marker` copies the old list into a replacing marker; a new marker starts empty. Remove, node delete and all reverts already move whole `Marker` values, so they keep the list unchanged. `Network::park_vehicle(index, id)` is the load-time setter, like `push_marker`. Paste builds markers through `EditMarker` on new nodes, so a paste has none.
- **`ad-io::vehicles`:** one `load(bytes, version, &mut Network)` parses, then attaches. The network is touched only after the parse succeeded. A hand-written attribute scanner over the raw start tag, checked against quick-xml's tag bytes, records for each reference attribute the value range and the attribute range with its leading whitespace. The target is `Vehicle(id)` for `parkDestination`, `Node(id)` for first/second, or `Verbatim` for a value that is out of range or not a number (and for a park value of a vehicle without an id). A Verbatim value is never rewritten. An element is a vehicle's AutoDrive element by its parent chain: FS19 `vehicle/FS19_AutoDrive/AutoDriveVehicleData`, otherwise `vehicle/AutoDrive`. The id is `uniqueId` for FS25 and `id` otherwise. `patch` counts positions over markers whose node exists (as `mapmarker_block` writes them). It replaces only values that differ from the loaded number, so unchanged values stay byte-identical even with odd spacing. `save_to` writes atomically (AD-6).
- **App:** `Loaded` gains `Option<Arc<VehiclesFile>>`; `load_file` reads `vehicles.xml` beside the config. A missing file gives none. An unreadable file gives none plus a notice in the document notice, so it is never written. `Document` holds `vehicles` and `opened_path`; a Save As moves `path` but not `opened_path`. `vehicles_to_patch` returns the file only when the target equals `opened_path` (canonicalised when both exist). `spawn_save` writes `vehicles.xml` only after the config write succeeded and only if the file is still there. A vehicles error comes back as `JobResult::Saved { warning }`: the document is marked saved, the message is shown, and a pending close is cancelled so the message is seen.
- **Icon:** the parking icon (20 px) is centred on the node of each marker with parked vehicles, as Java `NodeDrawThread` 120-122 does. It is drawn after the height warning and before the marker icons. Java's F3 toggle is not ported. `frame_time` now also loads the fixture's `vehicles.xml` (read only), so the icons are part of the timing.
- **Not done:** the base for patching is the bytes read at load; a `vehicles.xml` that the game rewrote while the editor was open is overwritten from that base (the config save has the same property).
- **Tests:**
  - `ad-core`: `parked_vehicles_survive_replace_remove_and_reverts` and `parked_vehicles_survive_marker_edits_node_delete_and_undo`.
  - `ad-io` vehicles: `fs22_sample_parks_patches_and_keeps_the_rest`, `fs19_sample_uses_its_own_element`, `fs25_reads_unique_id_and_a_new_park_target_adds_no_attribute` and `broken_file_is_an_error_and_leaves_the_network`.
  - `ad-io/tests/vehicles_roundtrip.rs` (temp copies of both fixtures):
    - Load fixture: 223 markers, parking on 161/163/164/165/167/178, 50 first/second references.
    - Save unchanged: byte-identical.
    - Delete before (marker 100): only the reference values differ, and they differ as expected.
    - Delete referenced (161 and 180): one park removed, its element becomes `<AutoDrive/>`, the five 180s become 1, and later references shift by two.
    - Undo: byte-identical.
    - Node delete: the parked vehicle comes back after undo.
  - `ad-app` jobs: `vehicles_are_patched_only_by_a_save_to_the_opened_path` (Save as, a later save to the new path, a save to the opened path spelled differently), `no_vehicles_file_loads_and_saves_as_before`, `broken_vehicles_file_gives_a_notice_and_is_never_written` and `a_vehicles_warning_is_shown_and_the_config_save_still_counts`.
  - `ad-app` main: `a_vehicles_warning_keeps_the_window_but_the_save_counts`.
  - `ad-app` view: `parking_icon_sits_on_the_node_of_a_marker_with_parked_vehicles`.
- **Verification (2026-11-08):** `cargo fmt --all --check` passes and `cargo clippy --workspace --all-targets -- -D warnings` is clean. `cargo test --workspace` passes: ad-app 306 (2 ignored), ad-core 66 plus 4, ad-io 44, golden_edited 6, golden_roundtrip 1 and vehicles_roundtrip 6. `frame_time` (release): every case is under 16 ms. The worst best-round max was 10.3 ms (rotate: min zoom, whole network selected); static min zoom with everything selected was 7.8 ms, and drag at min zoom with the whole network selected was 6.8 ms. The root `AutoDrive_config.xml` and `vehicles.xml` are unchanged.

## Plan Change Log

## Review Triage Log
