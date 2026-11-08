---
title: 'A DDS import shows the map image and saves it for the next load'
type: 'feature'
ticket: '2'
created: '2026-11-08'
status: 'planned'
baseline_revision: '82e97cf'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
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
- [ ] ad-io crop, scale, save -- tests for FS25, FS19, Odd size, the path
- [ ] job and UI -- tests for Exists (both answers), Bad file, No map name, Replaced, read-only allowed
- [ ] the FS25 sample end to end in a test (skipped when the gitignored file is absent): decode, crop, 2048²

**Acceptance Criteria:**
- Given the fixture open, when the user imports `pda_map_H.dds` as FS22/FS25, then the map background appears at once, and after reopening the fixture it is still there.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
