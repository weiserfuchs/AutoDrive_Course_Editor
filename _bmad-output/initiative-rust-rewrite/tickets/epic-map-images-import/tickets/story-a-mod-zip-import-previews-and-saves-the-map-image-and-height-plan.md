---
title: 'A mod-zip import previews and saves the map image and heightmap'
type: 'feature'
ticket: '3'
created: '2026-11-08'
status: 'built'
baseline_revision: '3641a05'
route: 'full'
route_source: 'auto'
risk: 'high'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** A map mod's zip holds the map's PDA image and heightmap, but users must extract and convert them by hand. Java's `ImportFromZipMenu` reads the zip, previews both and exports them under `mapImages/`.

**Approach (as Java unless noted):**
- **Import menu "Map mod zip…"**: pick a `.zip`; a background job (AD-9) reads it:
  - `modDesc.xml` at the root: `descVersion` gives the game (40–53 FS19, 60–80 FS22, ≥ 90 FS25, else refused, AD-11); the `<map configFilename>`; the `<map><title>` texts (one per language).
  - the map config file: `<map imageFilename>`, with `.png` replaced by `.dds`.
  - `dem.png` anywhere in the zip (Java's `findFileInZip`).
  - decodes the DDS (story 7.1), crops and scales it for the game (story 7.2) and decodes `dem.png`.
- **Preview window** (modal): 256×256 previews of map and heightmap, the detected game (changeable: FS19 / FS22 / FS25, as Java's override), and a map name field. The name is prefilled with the open config's map name when a document is open, else the English title (else the first title); unlike Java's locale combo, it is a free text field. "Import" saves; "Cancel" discards.
- **Import** saves `<exe dir>/mapImages/<name>/<name>.png` and `<name>_HeightMap.png` (name with spaces and dots replaced by `_`, as Java), asking before replacing existing files. When the open config's map name equals the name, the image and heightmap are attached at once (heightmap scale recomputed as at load); otherwise they are found at the next load.
- **Safety (AD-12):** entries are read by name inside the zip, never extracted to paths; absolute and `..` names are refused; each read entry is capped (DDS 128 MiB, PNG and XML 64 MiB); a malformed zip or XML, a missing entry or an unsupported version shows an error and changes nothing.
- The heightmap is missing (no `dem.png`): import the map image alone, with a notice.

## Boundaries & Constraints

**Always:** reading, decoding and saving run off the UI thread; a document replaced meanwhile ignores the result (AD-18); the config and the history never change.

**Never:** no extraction to disk; no other zip contents; no download.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| FS25 zip | generated zip: modDesc descVersion 92, map xml, the FS25 DDS, dem.png | previews; game FS25; name = open config's map name; Import saves both PNGs and attaches them | none |
| Versions | descVersion 45 / 70 / 95 / 30 | FS19 / FS22 / FS25 / refused | error for 30 |
| Override | detected FS25, user picks FS19 | whole-image crop on Import | none |
| Name | no document open; titles en "Foo Bar", de "Foo" | name "Foo Bar"; saved as `Foo_Bar` | none |
| No dem | zip without dem.png | image only, notice | notice |
| Hostile | entry `../x.dds`, absolute path, oversized entry | refused | error |
| Missing | no modDesc / no map config / no DDS | error naming what is missing | error |
| Exists | target PNGs present | asks before replacing | none |
| Other map | open config's map differs from the name | saved, not attached | none |

</frozen-after-approval>

## Code Map

- `Cargo.toml` / `crates/ad-io/Cargo.toml` -- `zip` 9.4.0 (`default-features = false`, `deflate`), per the Stack.
- `crates/ad-io/src/mod_zip.rs` (new) -- `read(bytes or path) -> Result<ModZip { version, titles, map_image: RgbaImage (cropped 2048²), heightmap: Option<RgbaImage or the 16-bit source> }>`; name sanitising; entry caps; tests with zips built in the test (the `zip` writer is fine in dev-dependencies, or write stored entries by hand).
- `crates/ad-app/src/jobs.rs`, `main.rs`, `ui.rs` -- the job, the preview modal (textures for the previews), Import/Cancel, replace question (reuse story 7.2's), attach when the names match (heightmap through the same path a load uses).

## Tasks & Acceptance

**Execution:**
- [x] `mod_zip.rs` -- tests for Versions, Name, No dem, Hostile, Missing
- [x] job and preview -- tests for FS25 zip (with the FS25 sample, skipped when absent), Override, Exists, Other map, Replaced document
- [x] dependency added per the Stack

**Acceptance Criteria:**
- Given the fixture open and a zip built from the FS25 sample files, when the user imports it and confirms, then the preview shows both images, and after Import the map image and heightmap are attached and found again at the next load.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Dependency:** `zip = { version = "9.4.0", default-features = false, features = ["deflate"] }` in the workspace, used by `ad-io`; `ad-app` has it as a dev-dependency only (tests build zips with `ZipWriter`). `image` moved from dev-dependency to dependency of `ad-app` (preview `RgbaImage`s).
- **`ad-io::mod_zip`:** `read(reader)` opens the archive and reads `modDesc.xml`, the `<map configFilename>` file and the DDS by name into memory, each capped (`MAX_DDS` 128 MiB, `MAX_OTHER` 64 MiB, checked against the header and again while reading with `take`). Requested names have `\` turned into `/`; absolute names, drive letters and any `..` component are refused (`Unsafe`). `dem.png` is the first entry whose last path component is `dem.png` (Java matched `endsWith`, which also hits `xdem.png`); its name gets the same check. The DDS is kept uncropped in `ModZip.image`, so the game picked in the preview decides the crop at Import.
- **Heightmap format:** `dem.png` is saved byte for byte as `<name>_HeightMap.png` (Java re-encodes the same image). It is decoded once with the loader's own code (`map_image::decode_heightmap`, split out of `load_heightmap`), so only a heightmap the next load accepts (16-bit grey or 8-bit RGB(A)) is imported; the FS25 sample is 16-bit grey 4097². An unreadable `dem.png` is treated like a missing one: image only, with a notice naming it.
- **Name:** `default_name` (map name of the open config, else `en` title, else first title); `folder_name` trims, replaces spaces and dots with `_` and refuses empty names and `/ \ : * ? " < > |` and control characters (Import is disabled with a red note). Attach compares the folder name with the config's map name.
- **App flow:** `zip_import.rs` holds the read job (previews: 256² of the whole image, of the centre half and of the heightmap stretched min..max as grey), the `ZipPreview` state, the save job and `attach`. The preview modal is `ui::zip_preview_window`. Import checks on the UI thread whether the target files exist (a stat only); if so the modal asks "Replace the existing files?" — Yes saves, No returns to the preview. Reading, decoding, encoding and writing run on background threads.
- **Epochs (AD-18):** both jobs capture the open document's epoch (`None` without one). A read result for another document is dropped; a new document also closes the preview. A save result always leaves the files written, but attaches only to the same epoch and only when the names match. Attach sets `map_image` (uploaded next frame) and, with a heightmap, `heightmap` and `height_map_scale = calibrate(...)` with the document's geometry, as a load does; the config and history are untouched.
- **Menu:** "Map mod zip…" is enabled without a document; the DDS items still need one.
- **Hand check:** `target/handtest/FS25_TestMap.zip` (modDesc descVersion 92 with titles `NF Marsch 4fach`, `maps/map.xml` with `imageFilename="maps/overview.png"`, `maps/overview.dds` = `pda_map_H.dds`, `maps/data/dem.png` = `dem.png`), built with .NET `ZipFile`; read back through `mod_zip::read` as FS25, 4096² image, dem present.
- **Tests:** `mod_zip::tests::{a_complete_zip_gives_version_titles_image_and_dem, versions_map_to_games_and_old_ones_are_refused, the_name_defaults_to_the_map_name_then_english_then_first_title, no_dem_gives_the_image_alone_with_a_notice, hostile_names_and_oversized_entries_are_refused, missing_parts_are_errors_naming_them}`; `map_image::tests::previews_follow_the_crop_and_stretch_the_heightmap`; `zip_tests::{the_fs25_sample_zip_previews_saves_and_attaches, the_chosen_game_sets_the_crop, existing_files_are_replaced_only_after_yes, another_maps_name_or_no_document_saves_without_attaching, results_for_a_replaced_document_are_ignored, a_bad_zip_shows_an_error_and_changes_nothing}`.
- **Verification (2026-11-08):** `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` ok (ad-app 361 passed, 2 ignored; ad-io 64 + integration tests); the FS25 sample test ran (not skipped). `frame_time` (release) passed twice; worst best-round max 7.8 ms (rotate, min zoom, whole network), all cases under 16 ms.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator), pass 1: 1 low patched.
  - low, patch: images saved for another map (no attach) gave no feedback at all. The open document now shows a notice naming the saved folder; with no document open, the message shows in the message window. Covered in `another_maps_name_or_no_document_saves_without_attaching`.
  - accepted: only the entries actually read are checked for hostile names; `dem.png` must be named exactly (Java's `endsWith` also matched e.g. `xdem.png`).
- Verification after the patch: fmt, clippy and workspace tests pass (ad-app 361).
- Hand check (user), pass 1: after Import the top bar still said "No heightmap found …", so the heightmap looked inactive, although it was saved and attached. Fixed: attaching a heightmap drops the load's no-heightmap notice line (other lines stay); test `an_attached_heightmap_drops_the_loads_no_heightmap_notice`. Verified: fmt, clippy, workspace tests (ad-app 362).
