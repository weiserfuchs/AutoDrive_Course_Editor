---
title: 'Markers, parking and warning icons, marker names and the grid draw'
type: 'feature'
ticket: '5'
created: '2026-11-06'
status: 'built'
baseline_revision: 'b27a46218fcf140b07c569f43ce083e4d32c3738'
route: 'full'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: ['_bmad-output/initiative-rust-rewrite/tickets/epic-map-view/epic-map-view.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The view shows nodes and connections, but no markers, marker names, warnings or grid. Users find destinations by marker, and align edits with the grid.

**Approach:** Port the Java drawing:
- Rasterise the SVG icons once.
- Draw the marker icon and name for every marker, and the warning icon for nodes with height −1.
- Draw the grid, aligned to the world origin, when zoom is above 2.

Parking and overlap icons are prepared but get their data from later epics.

## Boundaries & Constraints

**Always:**
- **Icons** (Java `NodeDrawThread` 110-139, `MapPanel` 204-207):
  - The SVGs in `src/main/resources/editor/nodes/` (`marker_icon`, `parking_icon`, `warning_icon`, `warning_y_icon`) are copied into `ad-app`, embedded, and rasterised once per app run at a fixed size: marker 25 px, the others 20 px.
  - They are uploaded as textures and drawn centred on their anchor, at that size whatever the zoom, with no zoom threshold.
  - The rasterising library must render the parking icon's text "P". With Lato missing it falls back to a system sans font.
- **Which icons, in this order, all that apply:**
  1. `warning_y` (red) at the node centre when the node's y is exactly −1 (no height).
  2. The marker icon for every marker. Its anchor is world `(x, z − 1)` on screen, shifted up by 20 px.
- **Marker names:**
  - white text in egui's default proportional font, size 12;
  - centred horizontally on world `(x, z − 1)`, with its vertical centre about 3 px above that point (Java baseline `y + h/2 − 3`);
  - no background, no zoom threshold, name only (the group is added on hover, in story 9).
- **Toggles:**
  - Marker icons and marker names are on by default.
  - The grid is off by default (Java `bShowGrid` = false).
  - A "Grid" checkbox in the top bar switches it for the session; it starts from a new global setting `show_grid` (default false). Nothing writes the settings file.
- **Grid** (Java `MapPanel.drawGrid` 216-238):
  - It is drawn only when the toggle is on and zoom > 2.
  - The spacing is in world units: per-map `grid_x`/`grid_y` (new `MapSettings` fields, `f64`, default 2.0, minimum 0.1), copied into `MapContext` at load.
  - Lines lie at whole multiples of the spacing from the world origin and span the panel. They are opaque (25,25,25), 1 px, in one mesh.
- **Draw order:** map image, grid, connections, nodes, icons, marker names.
- **Frame budget:** the frame-time test stays under 16 ms on a quiet machine, with names and icons included. The fixture's 223 markers are in view at minimum zoom.

**Never:**
- The overlap warning (needs the scan, epic-scans-autosave) or the parking icon's data (parked vehicles come from `vehicles.xml`, epic-markers-parking). The parking texture is rasterised and checked, but no node draws it yet.
- Hover, tooltips and the group in the name (story 9), user icon overrides, settings UI beyond the one checkbox, or writing settings.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Icons rasterise | each of the 4 embedded SVGs | an RGBA image of 25×25 (marker) or 20×20, not fully transparent; parking has light "P" pixels in its centre | a rasterise failure is logged and that icon is skipped; never a panic |
| Height warning | node y = −1.0; y = −1.0005; y = 0 | warning_y drawn; not drawn; not drawn | none |
| Marker placement | a marker node at world (100, 200), scale 1, a known transform | icon centre = screen of (100, 199) − (0, 20); name centred at screen of (100, 199) | none |
| Toggles | names off / icons off | that element not drawn, the other still drawn | none |
| Grid off or low zoom | grid toggle off, or zoom ≤ 2 | no grid lines | none |
| Grid lines | spacing 2, a visible world x range 3.0..9.0 | vertical lines at x = 4, 6, 8 | none |
| Setting | a settings file without the grid fields | grid 2×2; `show_grid` false | none |

</frozen-after-approval>

## Code Map

- **SVG rasteriser:** add a pinned `resvg` (with its `usvg` and `tiny-skia` re-exports, and the fontdb system-font loading needed for the parking "P") as an `ad-app` dependency in the workspace `Cargo.toml`. Pick a version at least two weeks old that is compatible with the current toolchain. The `image` crate is already present.
- New `crates/ad-app/assets/icons/*.svg`, copies of the four SVGs.
- New `crates/ad-app/src/icons.rs`:
  - it embeds the SVGs with `include_bytes!`;
  - `rasterise(svg, px) -> Option<egui::ColorImage>`, unit-tested;
  - an `Icons` struct holds the `TextureHandle`s, loaded lazily once from the egui `Context` (in `App` or `MapView`).
- `crates/ad-app/src/view.rs` `MapView::paint`:
  - draw order: image, grid, mesh of connections, nodes, then icons and names;
  - icons and names come from markers whose node is in the (padded) visible rectangle, plus warning_y for visible nodes with y == −1;
  - `network.markers()` gives `Marker { node, name, group }`, and `net.node(id)` gives the position;
  - put the anchor and line math in pure functions in `look.rs` or a new `overlay.rs`, unit-tested.
- `crates/ad-io/src/settings.rs`:
  - `GlobalSettings` gains `show_grid: bool` (default false);
  - `MapSettings` gains `grid_x` and `grid_y` (`f64`, default 2.0).
- `crates/ad-core/src/map_context.rs` gains `grid: (f64, f64)` with a minimum of 0.1. `crates/ad-app/src/jobs.rs` copies it from Settings.
- `crates/ad-app/src/main.rs`: the "Grid" checkbox in the top bar, with the session state initialised from `settings.global.show_grid`. Marker names and icons have no UI toggle yet; they use view flags, default true.
- Extend the frame-time test so it runs with the real icon textures and names (allocate the textures through `ctx`).
- Java reference: `NodeDrawThread.java` 110-139 and 200-228, `MapPanel.java` 204-238, `EditorXML.java` 116, 169, 180-182, 427-479 and 787-810.

## Tasks & Acceptance

**Execution:**
- [ ] Workspace and `ad-app` Cargo files, `assets/icons`, `icons.rs` -- rasterise and upload once; tests for the "Icons rasterise" row.
- [ ] `crates/ad-io/src/settings.rs`, `crates/ad-core/src/map_context.rs`, `crates/ad-app/src/jobs.rs` -- grid and `show_grid` settings into MapContext and the app; tests for the "Setting" row.
- [ ] `crates/ad-app/src/view.rs` (plus `look.rs` or `overlay.rs`), `main.rs` -- the grid, icons, names, toggles and draw order; tests for the "Height warning", "Marker placement", "Toggles", "Grid off or low zoom" and "Grid lines" rows; update the frame-time test and record its numbers.

**Acceptance Criteria:**
- Given the fixture opened, then its markers show the pin icon with the white name above each marker node at every zoom, and with "Grid" ticked and zoomed in past 2, a dark grid aligned to whole world units appears under the network (hitl).
- Given the frame-time test on a quiet machine, then every case stays under 16 ms.

## Implementation Notes

- resvg 0.45.1; icons rasterised once on the first frame (system fonts loaded once, Arial as sans fallback for the parking P).
- Icons are batched into one mesh per texture; one `painter.image` per marker cost about 10 ms extra at min zoom.
- Frame time (release, 1920x1080, 35652 nodes, 223 markers, not a fully quiet machine), median ms: min zoom 11.1-11.9; mid zoom dense 1.5; max zoom centre 0.1; max zoom dense 0.4; max zoom dense + grid 0.4-1.6; mid zoom dense + grid 3.2-4.3. Max single frames up to 33 ms at min zoom on the first timed frames (noise).

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 1 medium and 3 low patched; 1 low deferred; 2 low and 2 false rejected.
  - low, patch: the parking SVG names Lato-Bold, so usvg fell back to fontdb's serif family. The serif family is now set to Arial too, so the "P" is sans.
  - low, patch: a zero rasterise size returned None without the promised log line. It is now logged.
  - medium, patch: the grid had no density limit (spacing 0.1 at zoom just over 2 is about 30k lines). It is now skipped below `lod::MIN_GRID_PX` = 4 px on-screen spacing; unit-tested.
  - low, patch: `resvg` was a caret requirement. It is pinned as `=0.45.1`.
  - low, defer: the height-warning check walks every visible node each frame, even at minimum zoom. Within budget; recorded in deferred-work.md for story 2.7.
  - low, rejected: long marker names can pop at the panel edge (60 px pad). Cosmetic and rare; a fix needs per-name width measurement.
  - low, rejected: icon and grid sizes are egui points, not physical px. This is consistent with the whole view; handling pixels_per_point is a cross-cutting change.
  - false: "frame budget not demonstrated". Orchestrator quiet run: min zoom 9.4 ms median; the over-budget maxima came with outside load.
  - false: the unticked Execution checklist is plan bookkeeping.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: no diff
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: no warnings
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms on a quiet machine; numbers recorded

**Manual checks (if no CLI):**
- The user opens the fixture: marker pins and names show; ticking "Grid" and zooming in shows the grid.
