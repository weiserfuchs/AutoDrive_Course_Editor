---
title: 'A mod-zip import previews and saves the map image and heightmap'
type: 'feature'
ticket: '3'
created: '2026-11-08'
status: 'planned'
baseline_revision: '3641a05'
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
- [ ] `mod_zip.rs` -- tests for Versions, Name, No dem, Hostile, Missing
- [ ] job and preview -- tests for FS25 zip (with the FS25 sample, skipped when absent), Override, Exists, Other map, Replaced document
- [ ] dependency added per the Stack

**Acceptance Criteria:**
- Given the fixture open and a zip built from the FS25 sample files, when the user imports it and confirms, then the preview shows both images, and after Import the map image and heightmap are attached and found again at the next load.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
