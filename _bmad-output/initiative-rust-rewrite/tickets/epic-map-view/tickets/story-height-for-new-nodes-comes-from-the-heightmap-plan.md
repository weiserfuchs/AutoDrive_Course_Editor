---
title: 'Height for new nodes comes from the heightmap'
type: 'feature'
ticket: '6'
created: '2026-11-06'
status: done
baseline_revision: '13047d9b7c8c2f3bf7c017dd6934810f2ee1b444'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: ['_bmad-output/initiative-rust-rewrite/tickets/epic-map-view/epic-map-view.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** New nodes need a terrain height (R2, AD-7). Without a heightmap they get −1, which AutoDrive treats as invalid. The editing tools in the next epic need a single function that returns a height for a world position.

**Approach:**
- The background load job finds the heightmap beside the config or among the map images, decodes 8-bit RGB or 16-bit grey into one sample format, and calibrates Java's height scale against the loaded nodes.
- It attaches the heightmap to the `MapContext`.
- `ad-core` gains the one `sample_height(map_context, x, z) -> Option<f64>`.

## Boundaries & Constraints

**Always:**
- **Lookup (AD-14, Java `MapImage.loadHeightMap` 157-226):**
  1. `terrain.heightmap.png` in the config's directory;
  2. `mapImages/<name>/<name>_HeightMap.png` in the exe dir, then in the config dir, through the same lookup as the map image.
  - No map name → only step 1.
- **Decoding** happens in `ad-io` on the load job. Each pixel becomes one `u16` raw value:
  - 16-bit grey: the grey value itself (epic decision 2026-11-06);
  - 8-bit RGB(A): `(R << 8) + G`, as Java.
  - Any size is kept as is; the fixture's heightmap is 2049×2049.
  - Other formats, or a file that fails to decode, give no heightmap plus a notice naming the file.
- **Sampling** (Java `MapPanel.getYValueFromHeightMap` 707-729), with W and H the heightmap size:
  - `px = W/2 + floor((x / scale) / (2048 / W))` and `py = H/2 + floor((z / scale) / (2048 / H))`, clamped to `0..=W−1` and `0..=H−1`. Java misses the `== W` case; this clamp fixes it (AD-13).
  - height = `raw / 256`, rounded to 3 decimals half-up, then divided by `height_map_scale`.
  - `None` when there is no heightmap. The `−1` sentinel stays in ad-io only.
- **Calibration** (Java `calculateHeightMapScaling` 374-388):
  - Once at attach: `height_map_scale = round(mean((h − y) / y))` over the loaded nodes, where `h` is the sample at scale 1. The minimum is 1.
  - Nodes with `y <= 0` (including −1) are skipped, because they would divide by zero. No usable nodes → 1.
- **MapContext** gains `heightmap: Option<Arc<Heightmap>>` and `height_map_scale: f64`. `Heightmap` lives in `ad-core` (no I/O): width, height and the raw samples.
- **Missing heightmap:** a notice joins the map-image notice: "No heightmap found for <name>; new nodes will have no height". Image and heightmap problems never fail the load.

**Never:**
- Change the height of existing nodes, or draw the heightmap. Importing, exporting or downloading heightmaps belongs to epic-map-images-import.
- Keep a second copy of the heightmap, or sample it on the UI thread outside `sample_height`.
- Commit the fixture's heightmap. It is gitignored; tests use generated PNGs.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Lookup order | `terrain.heightmap.png` beside the config and `<name>_HeightMap.png` in mapImages | the `terrain.heightmap.png` | none |
| Fallback | only `mapImages/<name>/<name>_HeightMap.png` in the exe dir, or in the config dir | that file | none |
| Decode 16-bit | a 4×4 16-bit grey PNG holding 25600 | raw 25600 → height 100.000 at scale 1 | none |
| Decode 8-bit | a 4×4 RGB PNG with R=100, G=128 | raw 25728 → height 100.5 | none |
| Broken | invalid bytes, or a palette/other format | no heightmap; notice names the file | not a load error |
| Pixel mapping | W=2049, scale 2, world (0,0), (−4096,−4096), (4095.9, 4095.9) | px (1024,1024), (0,0), (2048,2048) after the clamp | none |
| Rounding | raw 12345 | 48.223 (12345/256 = 48.22265…) | none |
| Calibration | node heights equal the samples; samples double the heights; all y ≤ 0; no nodes | scale 1; round(1.0) = 1; 1; 1 | none |
| No heightmap | `MapContext` without heightmap | `sample_height` → None | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/map_image.rs`:
  - `find_map_image(name, exe_dir, config_dir)` probes `mapImages/<name>/<name>.png`. Generalise it with a suffix (`""` or `"_HeightMap"`), or add `find_heightmap(name, exe_dir, config_dir)` that tries `config_dir/terrain.heightmap.png` first.
  - Add `load_heightmap(path) -> Result<ad_core::Heightmap, String>` here or in a new `heightmap.rs`.
  - Use `image` 0.25: `DynamicImage::ImageLuma16` gives grey16, `into_rgb8` gives RGB. Inspect `color()` to tell them apart, and treat other formats as unsupported.
- New `crates/ad-core/src/heightmap.rs`:
  - `Heightmap { width, height, raw: Vec<u16> }` with `new` (checks the length);
  - `sample_raw(px, py)`, the pixel mapping and `calibrate(&Network, MapGeometry) -> f64`;
  - `pub fn sample_height(ctx: &MapContext, x, z) -> Option<f64>`, the only height function (AD-7).
  - Register it in `lib.rs`. `Network::order()` and `node()` give x, y and z.
- `crates/ad-core/src/map_context.rs`: add `heightmap` and `height_map_scale` (default 1.0). Keep `MapContext::new`'s callers compiling; prefer a setter or `with_heightmap` over a fifth positional argument.
- `crates/ad-app/src/jobs.rs`:
  - `load_file` already finds and loads the image after the config parses; do the heightmap the same way and carry it in `Loaded`.
  - `replace_document`/`apply_result` attach it to `doc.map` and run `calibrate`. The calibration is about 35k samples, cheap, but do it in the job if that keeps the UI thread free; the job has the network.
  - Join the notices.
- `crates/ad-app/src/document.rs`: nothing new beyond `map` and `notice`.
- `#[ignore]` fixture test in `crates/ad-app/src/jobs.rs` (or `ad-io` tests):
  - it loads the fixture through `load_file` with the real `terrain.heightmap.png` beside it (gitignored; the test skips with a message if the file is missing);
  - it prints `height_map_scale` and the median/max of `|sample_height − y|` over nodes with y > 0, and asserts the median is under 1.0;
  - run it with `cargo test -p ad-app -- --ignored heightmap --nocapture`.

## Tasks & Acceptance

**Execution:**
- [ ] `crates/ad-core/src/heightmap.rs`, `map_context.rs`, `lib.rs` -- Heightmap, sampling, calibration, `sample_height`; tests for the "Pixel mapping", "Rounding", "Calibration" and "No heightmap" rows.
- [ ] `crates/ad-io/src/map_image.rs` (or `heightmap.rs`) -- lookup and decoding; tests for the "Lookup order", "Fallback", "Decode 16-bit", "Decode 8-bit" and "Broken" rows, with PNGs generated in temp dirs.
- [ ] `crates/ad-app/src/jobs.rs` -- load, attach, calibrate and join the notices; a job-level test that a generated heightmap beside an inline config reaches `doc.map` and `sample_height` returns its value; the `#[ignore]` fixture test with its numbers recorded in Implementation Notes.

**Acceptance Criteria:**
- Given the fixture with its `terrain.heightmap.png`, when it loads, then the heights sampled at existing nodes match their y values closely: the median deviation is under 1 m, shown by the ignored fixture test.
- Given a config without any heightmap, when it loads, then the notice says new nodes will have no height and `sample_height` returns None.

## Implementation Notes

- Fixture run (`cargo test -p ad-app -- --ignored heightmap --nocapture`, map scale 2): `height_map_scale` 1, 35652 nodes with y > 0, median |sample - y| 0.391 m, max 11.285 m.
- `MapContext::attach_heightmap(heightmap, &network)` attaches and calibrates; `replace_document` calls it on the UI thread. `Loaded` is now a 4-tuple (config, image, heightmap, notice).

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 1 medium and 3 low patched; 1 confirmation (no finding).
  - medium, patch: calibration (about 35k samples) ran on the UI thread in `replace_document`, which the Never list forbids. It now runs in the load job, which receives a Settings clone for the map scale; `replace_document` only stores the result. `MapContext::attach_heightmap` was removed as unused.
  - low, patch: the decoder expands palette PNGs to RGB, so they were accepted against the "Broken" row. `load_heightmap` now rejects IHDR colour type 3 before decoding; tested.
  - low, patch: the notice join used a raw newline in a multi-line literal. It is now `"{a}
{b}"`.
  - low, patch: the ignored fixture test indexed the median without an emptiness check. It now asserts first.
  - no finding: the reviewer confirmed the "Calibration" row is covered.
- Fixture after the patches (orchestrator): height_map_scale 1, median |sample − y| 0.391 m, max 11.285 m.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: no diff
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: no warnings
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app -- --ignored heightmap --nocapture` -- expected: median |sample − y| under 1.0 on the fixture; numbers recorded
