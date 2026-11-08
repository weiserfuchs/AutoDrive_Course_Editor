---
title: 'Map image and heightmap load and save by hand'
type: 'feature'
ticket: '5'
created: '2026-11-08'
status: done
baseline_revision: 'e0a118a'
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

**Problem:** Users cannot pick a map image or heightmap file themselves, nor write the shown ones somewhere. Java has four Import-menu items for this (`LoadMapImageMenu`, `SaveMapImageMenu`, `ImportHeightMapMenu`, `ExportHeightMapMenu`).

**Approach (as Java: loading only shows, saving persists):**
- **"Load map image…"**: pick a PNG; a background job decodes it through the same loader as a local image (a wrong size is scaled to 2048² with the usual notice, AD-14) and shows it for the open document. Not saved: a notice says "Map image loaded, not saved — use Save map image to keep it".
- **"Save map image…"**: a save dialog prefilled with `<exe dir>/mapImages/<name>/<name>.png` (`unknown.png` without a map name); writes the shown image as PNG atomically. To have the image available, the document keeps the shown image's PNG bytes (encoded once when it is shown) instead of a full RGBA copy (AD-8).
- **"Import heightmap…"**: pick a PNG (any name, Java filters for `terrain.heightmap.png`); decode with the heightmap loader (16-bit grey or 8-bit RGB), set `MapContext.heightmap`, recompute `height_map_scale` as a load does, drop the load's "no heightmap" notice (story 7.3). Not saved.
- **"Export heightmap…"**: a save dialog prefilled with `<exe dir>/mapImages/<name>/<name>_HeightMap.png`; writes the heightmap as a 16-bit grey PNG that the loader reads back to the same heights. Disabled without a heightmap.
- All four need an open document (read-only too); decoding and encoding run in the background; a failure shows the reason and changes nothing. The config and history never change.

## Boundaries & Constraints

**Always:** AD-9 background work; AD-18 epochs for results; atomic writes; paths chosen in the dialog, never relative to the working directory.

**Never:** no automatic save on load or import of a heightmap; no image editing.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Load 2048 | a 2048² PNG | shown; notice "not saved" | none |
| Load other size | a 1024² PNG | scaled to 2048², shown, scaling notice | none |
| Load bad | not a PNG | error, image unchanged | error window |
| Save | an image shown | PNG written where chosen; byte content decodes to the shown image | none |
| Save none | blank image | saves the blank 2048² image (as Java) | none |
| Import heightmap | the FS25 `dem.png` | heightmap set, scale recomputed, notice line dropped | none |
| Import bad | a PNG the heightmap loader refuses | error, heightmap unchanged | error window |
| Export | heightmap present | 16-bit PNG; reloads to the same heights | none |
| Export none | no heightmap | item disabled | none |
| Replaced | another config opened during a job | result ignored | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/map_image.rs` -- `encode_heightmap(&Heightmap) -> Vec<u8>` (16-bit grey); reuse `decode_map_image`, `decode_heightmap`, `encode_png`, `save_map_image`.
- `crates/ad-app/src/document.rs` -- keep the shown image's PNG bytes (`Arc<[u8]>`), set wherever `map_image` is set (load, DDS, zip, download, manual load).
- `crates/ad-app/src/jobs.rs`, `main.rs`, `ui.rs` -- the four menu items, jobs, results with epochs, notices.

## Tasks & Acceptance

**Execution:**
- [x] ad-io heightmap encoding -- round-trip test (the FS25 `dem.png` when present, and a generated one)
- [x] app -- tests for every matrix row
- [x] PNG bytes kept for every way an image is shown

**Acceptance Criteria:**
- Given the fixture open, when the user loads a PNG as map image, saves it, imports `dem.png` as heightmap and exports it, then each step shows its result, and the saved files load back to the same image and heights.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **PNG bytes live in `MapImage`.** `MapImage` gained `png: Arc<[u8]>`, so no path can show an image without bytes to save it. `decode` keeps the source bytes when the PNG is already 2048² (they decode to the shown image) and encodes once only when it scaled; `to_2048` (DDS, zip) encodes once and returns `Result`; Blank keeps the embedded `Blank.png`. `encode_png` became the private `encode_rgba`; DDS import and zip save use `image.png` instead of encoding again. `ImageSave` holds `Arc<[u8]>`, so the replace prompt clones no bytes.
- **Document keeps only the PNG after upload.** `Document::map_png` is set by `Document::new` and `Document::show(image)`, which every result that shows an image now calls (DDS, download, zip attach, manual load). The upload still takes `map_image`, so the RGBA copy is dropped as before (AD-8); `map_png` holds 0.1–a few MB.
- **Four Import-menu items** (after a separator): Load map image…, Save map image…, Import heightmap…, Export heightmap…. All need a document, read-only too; Export is enabled by `ui::can_export_heightmap` (a heightmap is present). They sit inside the existing `can_import` gate (not while loading, importing or downloading for the document).
- **Jobs:** `spawn_image_load` → `JobResult::ImageLoaded { epoch }`, `spawn_heightmap_load` → `HeightmapLoaded { epoch }` (both set `importing`), `spawn_export(Export::MapImage | Export::Heightmap)` → `Exported` (sets a new `exporting` flag, label "Saving image…"). Results for another epoch are dropped (AD-18). Writes go through `save_map_image` (atomic, folders created). The heightmap is encoded in the job by the new `map_image::encode_heightmap` (16-bit grey, `Heightmap::raw()` added in ad-core).
- **Heightmap attach reused:** `zip_import::attach` was split; `attach_heightmap` (calibrate as a load does, drop the `NO_HEIGHTMAP` line) serves both the zip and the manual import. The notice line filter moved to `Document::drop_notice_lines`, with `Document::add_notice` for the download's append. Calibration runs on the UI thread, as the zip attach already did (O(nodes)).
- **Notices:** a manual load drops earlier lines starting with "Map image " (the previous image's scaling, failed load, or not-saved line), then adds the scaling notice if any and `NOT_SAVED`. A successful save of the PNG that is still shown (`Arc::ptr_eq` with `map_png`) drops `NOT_SAVED`; the scaling line stays because it still describes the shown image.
- **Dialogs:** save dialogs are prefilled with `jobs::export_target` (Java names, `mapImages/unknown.png` / `unknown_HeightMap.png` without a map name) and start in the nearest existing folder of that target under the exe dir, so no folder is created before the user saves. Without an exe dir only the file name is prefilled. `.png` is appended when the chosen name lacks it. The OS dialog asks before it overwrites. The open dialogs filter `*.png` (any name, not only `terrain.heightmap.png`).
- **Tests:** ad-io `every_image_carries_png_bytes_that_decode_to_it`, `an_encoded_heightmap_reads_back_to_the_same_heights`, `the_fs25_dem_round_trips` (skips without `dem.png`). ad-app `manual_image_tests`: `a_2048_png_is_shown_not_saved`, `another_size_is_scaled_with_the_notice`, `a_bad_file_shows_an_error_and_keeps_the_image`, `save_writes_the_shown_image_and_drops_the_not_saved_line`, `save_without_an_image_writes_the_blank_one`, `a_failed_save_shows_the_reason`, `an_imported_heightmap_is_set_with_its_scale_and_drops_the_notice`, `a_refused_heightmap_shows_an_error_and_keeps_the_old_one`, `export_writes_a_16_bit_png_that_reloads_to_the_same_heights` (also the disabled item), `results_for_a_replaced_document_are_ignored`, `export_targets_and_the_png_extension`, `every_way_an_image_is_shown_keeps_png_bytes_of_it` (Blank load, scaled local load, DDS, zip attach, download), and the acceptance run `the_fixture_takes_a_loaded_image_and_the_fs25_dem_and_saves_both` (skips without `dem.png`).
- **Verification (2026-11-08):** `cargo fmt --all --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test --workspace` all pass (ad-app 384 passed, 3 ignored; ad-core 71; ad-io 76 + integration tests). `frame_time` (release): every case ok, best-round maxima ≤ 9.9 ms (worst single-round max 13.4 ms, drag at min zoom with the whole network selected). The dialogs themselves need a hand check in the GUI.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator), pass 1: no patch.
  - checked: `MapImage` carries its PNG bytes, so every way an image is shown can be saved without a full RGBA copy after upload (AD-8).
  - accepted: without a map name the targets are Java's `mapImages/unknown.png` and `unknown_HeightMap.png`; a manual load drops earlier "Map image …" notice lines.
- Verification (orchestrator re-run): fmt, clippy and workspace tests pass (ad-app 384).
