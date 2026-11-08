---
title: 'Map image and heightmap load and save by hand'
type: 'feature'
ticket: '5'
created: '2026-11-08'
status: 'planned'
baseline_revision: 'e0a118a'
route: 'full'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
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
- [ ] ad-io heightmap encoding -- round-trip test (the FS25 `dem.png` when present, and a generated one)
- [ ] app -- tests for every matrix row
- [ ] PNG bytes kept for every way an image is shown

**Acceptance Criteria:**
- Given the fixture open, when the user loads a PNG as map image, saves it, imports `dem.png` as heightmap and exports it, then each step shows its result, and the saved files load back to the same image and heights.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
