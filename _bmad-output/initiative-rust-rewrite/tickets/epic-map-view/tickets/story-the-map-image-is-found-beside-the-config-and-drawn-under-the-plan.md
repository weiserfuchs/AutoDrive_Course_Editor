---
title: 'The map image is found beside the config and drawn under the network'
type: 'feature'
ticket: '3'
created: '2026-11-06'
status: 'in-progress'
baseline_revision: '6b7b431591d407a4dc908918f30436350007d183'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: ''
review_source: ''
lenses_ran: []
review_loop_iteration: 0
context: ['_bmad-output/initiative-rust-rewrite/tickets/epic-map-view/epic-map-view.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The view draws the network on an empty canvas. Users orient themselves by the map image, and every later story (the look, the icons, the heightmap) needs the map context and the image path in place.

**Approach:** When a config loads:
- the background load job finds `<name>.png` locally, decodes it, and scales it to 2048×2048 if needed (with a notice), or falls back to an embedded Blank image;
- the UI thread uploads it once as a texture and drops the CPU copy;
- the view draws it under the network, aligned through MapGeometry.

Map-level data moves into an `ad-core` `MapContext` (epic decision of 2026-11-06).

## Boundaries & Constraints

**Always:**
- **MapContext** (new, in `ad-core`, no I/O, no egui):
  - It holds the map name (`Option<String>`) and the `MapGeometry`, built from the per-map scale in Settings, with a minimum of 1.
  - It is built once when the document is created (AD-14) and lives on the `Document`. The view takes its geometry from there; `main.rs` stops reading Settings per frame.
  - The heightmap fields come in story 6; leave no placeholder for them.
- **Lookup order (AD-14):**
  1. `<exe dir>/mapImages/<name>/<name>.png`
  2. `<config dir>/mapImages/<name>/<name>.png`

  The first existing file wins. Paths are never relative to the working directory. Without a map name, or with no file found, Blank is used.
- **Decoding and scaling** happen in `ad-io`, inside the existing background load job (AD-9):
  - the result is RGBA8, 2048×2048;
  - any other size is resized to 2048×2048, and the notice names the file and its original size;
  - a file that fails to decode falls back to Blank, with a notice naming the file and the error.
  - The config still loads in every image case. Image problems are never load errors.
- **Blank** is `src/main/resources/mapImages/Blank.png`, copied into the `ad-io` crate and embedded with `include_bytes!`. The single exe needs no resource folder.
- **Texture:**
  - The UI thread uploads it with linear filtering when the new document arrives. It is uploaded once per document, the CPU copy is dropped right after, and it is replaced or dropped together with the document.
- **Drawing:**
  - The image covers map pixels 0..2048 through ViewTransform.
  - It is painted before the connections and nodes.
- **Notices:** a notice is shown as a warning label in the top bar, like the read-only reason, until the next document loads.

**Never:**
- Download, DDS or zip import, or the heightmap (other stories and epics).
- Decode or scale on the UI thread, or keep the CPU copy after upload.
- Write anything into `mapImages/` or beside the config.
- Commit the fixture's real map image. It is gitignored; tests use generated images.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Exe dir wins | the image in both the exe dir and the config dir | the exe-dir path | none |
| Config dir | the image only beside the config | the config-dir path | none |
| Not found / no name | no file, or `map_name()` is `None` | Blank, no notice | none |
| Right size | a 2048×2048 PNG | RGBA 2048×2048, no notice | none |
| Wrong size | a 1000×800 PNG | RGBA 2048×2048; the notice names the file and 1000×800 | none |
| Broken file | a `.png` with invalid bytes | Blank; the notice names the file | not a load error |
| Alignment | world (0,0) at scale 2 | drawn at map px (1024,1024), the image centre | none |

</frozen-after-approval>

## Code Map

- `Cargo.toml` (workspace) and `crates/ad-io/Cargo.toml`: add `image` with `default-features = false` and the `png` feature. Pin an exact version at least two weeks old. Story 6 reuses it for 16-bit heightmaps.
- New `crates/ad-io/src/map_image.rs`:
  - `find_map_image(name, exe_dir: Option<&Path>, config_dir: Option<&Path>) -> Option<PathBuf>`;
  - `load_map_image(path) -> (MapImage, Option<String /*notice*/>)`;
  - `blank() -> MapImage`;
  - `MapImage { pub rgba: Vec<u8> }`, always 2048×2048.
  - Tests use PNGs encoded in memory with the `image` crate into a temp dir.
- New `crates/ad-io/assets/Blank.png`, a copy of `src/main/resources/mapImages/Blank.png`.
- New `crates/ad-core/src/map_context.rs`: `MapContext { pub name: Option<String>, pub geometry: MapGeometry }`, registered in `lib.rs`.
- `crates/ad-app/src/jobs.rs`:
  - `spawn_load`/`load_file` also find and load the image after the config parses, using `config.map_name()`, the config's parent dir and an `exe_dir` passed in.
  - `JobResult::Loaded`'s result carries `(LoadedConfig, MapImage, Option<String>)`.
  - `apply_result` takes `&Settings` in place of `history_cap`. It builds the `MapContext`, scale from `settings.maps[name]` with `.max(1)`, and stores the image and notice on the `Document`.
  - Update the existing tests.
- `crates/ad-app/src/document.rs`:
  - `Document` gains `map: MapContext`, `map_image: Option<MapImage>` (pending upload), `map_texture: Option<egui::TextureHandle>` and `notice: Option<String>`.
  - Document creation goes only through `replace_document`.
- `crates/ad-app/src/main.rs`:
  - Pass `std::env::current_exe()`'s parent into `spawn_load`.
  - On a new epoch, `take()` the pending image, `ctx.load_texture`, and store the handle.
  - Pass `doc.map.geometry` and the texture id to the view, and show `doc.notice` in the top bar.
  - Remove the per-frame Settings lookup in `paint_network`.
- `crates/ad-app/src/view.rs`: `MapView::show`/`paint` take `Option<egui::TextureId>` and paint the image over the screen rect of map px 0..2048 before the mesh. Extend the frame-time test with a 2048×2048 texture, allocated through `ctx` in the test, so the timing includes the image.

## Tasks & Acceptance

**Execution:**
- [ ] Workspace and `ad-io` Cargo files, `assets/Blank.png`, `map_image.rs` -- lookup, decode, scale, Blank and notices, with tests for the matrix rows "Exe dir wins" to "Broken file".
- [ ] `crates/ad-core/src/map_context.rs`, `lib.rs` -- MapContext.
- [ ] `crates/ad-app/src/jobs.rs`, `document.rs` -- load the image in the job, build the MapContext from Settings, and carry the pending image and notice. Tests: the job result carries the image, and scale 2 from Settings reaches `doc.map.geometry`.
- [ ] `crates/ad-app/src/main.rs`, `view.rs` -- upload once, draw under the network, show the notice. Test that the "Alignment" row's image rect matches map px 0..2048 through the transform.

**Acceptance Criteria:**
- Given the fixture opened with `mapImages/NF_Marsch_4fach/NF_Marsch_4fach.png` beside it and `scale = 2`, when the view shows, then the map image appears under the network, and the roads line up with the connections at any zoom (hitl).
- Given a config whose map image is missing, when it opens, then the network shows on Blank and the app works as before.

## Implementation Notes

Frame time (release, 1920x1080, with a 2048x2048 texture, headless so GPU share is not covered), median / max ms: min zoom centre 14.902 / 21.896; mid zoom dense 3.256 / 5.152; max zoom centre 0.078 / 0.092; max zoom dense 0.169 / 0.465. The min-zoom case is near the 16 ms budget (max above it); the image itself adds one shape, the cost is the edge mesh.

## Plan Change Log

## Review Triage Log

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: no diff
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: no warnings
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: each case under 16 ms on a quiet machine; record the numbers in Implementation Notes

**Manual checks (if no CLI):**
- The user runs `cargo run -p ad-app --release` and opens the fixture: the NF_Marsch_4fach image lies under the network, and the roads line up when zoomed in.
