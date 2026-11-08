---
title: 'A DDS import shows the map image and saves it for the next load'
type: 'feature'
ticket: '2'
created: '2026-11-08'
status: done
baseline_revision: '82e97cf'
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

**Problem:** A map without a local PNG shows a blank background. Java's "Import FS19 DDS" / "Import FS22 DDS" menu items (`ImportManager.createBufferImageFromDDS`, `createBufferImage`) decode a DDS, crop and scale it and show it, but never save it.

**Approach:**
- **Import menu** in the top bar (an egui menu button "Import") with "DDS for FS19…" and "DDS for FS22/FS25…", enabled when a document is open (read-only configs too: the image changes no data).
- Pick a `.dds` with the file dialog; a background job (AD-9) decodes it with `ad_io::dds::decode` (story 7.1), crops it (FS19: whole image; FS22/FS25: the centre half, i.e. the square from w/4 to 3w/4, as Java and AD-11) and scales it to 2048×2048 (a high-quality filter; no scaling when already 2048²).
- On success the image becomes the open document's map image (texture replaced) and is saved as `<exe dir>/mapImages/<map name>/<map name>.png` (AD-14, atomic write). If that file exists, a modal asks "Replace the existing map image?" first; No keeps the image shown but does not save it.
- A document replaced while the job runs ignores the result (AD-18 epoch). A failure (decode error, no map name in the config, write error) shows the reason in the error window and leaves the document's image unchanged.
- The UI keeps running while the job works; a "Importing…" label shows in the top bar.

## Boundaries & Constraints

**Always:** the decode, crop and scale run off the UI thread; no network or zip (stories 7.3, 7.4); the full-size CPU copy is dropped after the texture upload as today (AD-8).

**Never:** no change to the config or the history (an image is not an edit); no heightmap from a DDS.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| FS25 | the fixture open, `pda_map_H.dds` (4096²) via FS22/FS25 | centre 2048² shown; saved to `<exe>/mapImages/NF_Marsch_4fach/NF_Marsch_4fach.png`; found at the next load | none |
| FS19 | a 1024² DDS via FS19 | whole image scaled to 2048² | none |
| Odd size | a 3000×2000 DDS via FS22/FS25 | centre half (1500×1000) scaled to 2048² | none |
| Exists | target PNG present | asks; Yes replaces; No shows without saving | none |
| Bad file | a truncated DDS | error shown, image unchanged | error window |
| No map name | config without `<MapName>` | error "the config names no map", nothing saved | error window |
| Replaced | another file opened before the job ends | result ignored | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/map_image.rs` (or a new `import.rs`) -- pure `crop_for(version, image) -> RgbaImage`, `to_2048(image)`, `save_map_image(exe_dir, name, image)` (atomic, creates folders); tests for the crops, scaling and the saved path.
- `crates/ad-app/src/jobs.rs` -- an import job and its `JobResult` with the epoch; `main.rs`/`ui.rs` -- the Import menu, the replace question, attaching the image (replace `map_image`/`map_texture` the same way a load does), the label and errors.
- Use the existing exe-dir resolution from `startup.rs`/`jobs.rs` for `<exe dir>`.

## Tasks & Acceptance

**Execution:**
- [x] ad-io crop, scale, save -- tests for FS25, FS19, Odd size, the path
- [x] job and UI -- tests for Exists (both answers), Bad file, No map name, Replaced, read-only allowed
- [x] the FS25 sample end to end in a test (skipped when the gitignored file is absent): decode, crop, 2048²

**Acceptance Criteria:**
- Given the fixture open, when the user imports `pda_map_H.dds` as FS22/FS25, then the map background appears at once, and after reopening the fixture it is still there.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **ad-io (`map_image.rs`):** `map_image_target`, `crop_for(version, image)` (FS19 whole; anything else the rectangle `w/4, h/4, w/2 × h/2`, as Java and the Odd-size row, so 3000×2000 gives 1500×1000, not a square), `to_2048` (Lanczos3, untouched when already 2048²), `encode_png`, `save_map_image(target, png)` (creates the folders, `atomic::write`). The existing PNG load path keeps its Triangle filter.
- **Job (`jobs.rs`):** `import_dds` reads, decodes, crops, scales and encodes the PNG off the UI thread. When the target is absent it writes it in the job, so a write error fails the import and the image stays unchanged. When it exists the PNG comes back in `Imported.replace` and nothing is written. `JobResult::Imported { epoch, .. }` is dropped, errors too, when the document's epoch changed (AD-18). `JobResult::ImageSaved` carries the write after a Yes.
- **Target and version:** `import_target` needs the config's map name (error "the config names no map") and the exe dir, and it is checked before the file dialog opens. The exe-dir lookup moved from `App::open` to `jobs::exe_dir()`. The version comes from the menu item: "DDS for FS19…" passes FS19, "DDS for FS22/FS25…" passes FS25 (same crop as FS22).
- **UI:** an "Import" menu in the top bar, enabled with a document (read-only too) while no load or import runs, and an "Importing…" label. A successful result sets `doc.map_image`, which the map panel uploads and then drops, as on a load. The pending replace (`Document.image_to_replace`) lives on the document, so a new document drops it. The modal "Replace the existing map image?": Yes spawns the write, No or Escape keeps the image shown without saving. While it is up it counts as a dialog (`dialog_shown`), so shortcuts and map input are off.
- **Known gaps:** a write error after Yes shows the error while the image stays shown, like No. A job for a replaced document that found no target has already written the file, and the new document ignores the result.
- **Tests:** ad-io `fs22_and_fs25_crop_the_centre_half`, `fs19_keeps_the_whole_image_and_scales_it`, `an_odd_size_crops_the_centre_half_and_scales_to_2048`, `a_saved_image_is_found_and_loaded_at_the_next_load`, `fs25_sample_decodes_crops_and_scales` (ran here: 4096² → 2048², first pixel = source (1024, 1024)). jobs `the_import_target_needs_a_map_name`, `an_import_saves_a_new_target_and_keeps_an_existing_one`, `a_bad_dds_is_an_error_naming_it_and_saves_nothing`. main `import_tests::{an_import_shows_the_image_and_saves_it, an_existing_image_is_replaced_only_on_yes, a_bad_file_shows_an_error_and_keeps_the_image, a_result_for_a_replaced_document_is_ignored, a_read_only_document_takes_an_imported_image}`.
- **Verification:** `cargo fmt --all --check` exit 0. `cargo clippy --workspace --all-targets -- -D warnings` exit 0. `cargo test --workspace` all pass (ad-app 355 + 2 ignored, ad-io 57, ad-core 71, the rest). In the release frame_time run every case was under 16 ms; the worst best-round max was 9.2 ms (rotate tool idle, min zoom, all selected). The GUI acceptance (background shows at once, still there after reopening) needs a human run.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator), pass 1: no patch.
  - accepted: the crop is the centre rectangle w/2 × h/2 (Java and the Odd-size row; the Approach's "square" holds for square DDS files); a write failing after Yes leaves the shown image as with No; a job that finishes after the document was replaced still writes its PNG for its own map, which the next load of that map uses.
- Verification (orchestrator re-run): fmt, clippy and workspace tests pass (ad-app 355).
