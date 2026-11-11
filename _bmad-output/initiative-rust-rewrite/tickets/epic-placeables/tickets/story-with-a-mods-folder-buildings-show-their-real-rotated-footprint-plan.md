---
title: 'With a mods folder, buildings show their real rotated footprint'
type: 'feature'
ticket: '2'
created: '2026-11-11'
status: 'built'
baseline_revision: 'ac3c12b'
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

**Problem:** `placeables.xml` has no building size. The footprint is in each placeable's own XML, and only as node references into its `.i3d` scene (`i3dMapping node="0>1|0"`).

**Approach:**
- **ad-io `footprint`:**
  - Parses the placeable XML: `<base><filename>` names the i3d; it lists `clearArea`, `levelArea` and `testArea` and the `i3dMapping`s.
  - Parses the i3d's node tree under `<Scene>`, with translation, rotation y and scale per node.
  - Resolves each area's node paths to ground-plane positions.
  - Clear areas win, then level areas, then test areas:
    - a clear or level area is the parallelogram start/width/height;
    - a test area is the box start→end along the start node's axes.
  - Places the footprint with the savegame's position and rotation y.
  - `ModFiles` reads from `<mods>/<mod>.zip` (names matched ignoring case) or an unpacked `<mods>/<mod>/`, at most 32 MB per file, and refuses `..`.
  - Each placeable file is read once.
- **`Placeable::footprint`:** world quads; empty draws the square.
- **Settings:** `mods_folder` (Edit ▸ Settings… ▸ Editor: Browse…, Default). The default `Documents\My Games\FarmingSimulator2025\mods` is resolved only in `main`, so tests read no real mods.
- **Job:** `spawn_footprints` reads them in the background once per document and mods folder; a result for an older document is dropped.
- **View:**
  - A footprint is drawn as translucent quads (30 %) with a 1.5 px outline in the owner's colour.
  - While it is smaller than the 10 px square on screen, the square stands in.
  - Hover works anywhere inside the footprint; the name sits in its middle.

## Boundaries & Constraints

**Never:** no write to the mods; no base-game (`$data`) i3d. Those are in the game's archives, so such buildings keep their square.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Paths | "0>", "0>1\|0", scaled parent, bad index | resolved / none | none |
| Areas | clear + test / level by direct path / test only | clear wins; same rectangle | none |
| None | no areas, missing i3d, unknown mapping id | empty (square) | none |
| Rotation | savegame 90° | local x → −z, local z → +x | none |
| Files | zip (case differs), folder, missing, `..` | read / none | none |
| Job | mods folder with one mod, a `$data` building, stale epoch | footprint set / empty / dropped | none |
| View | zoom 10 / minimum zoom | quad, hover inside / square | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/footprint.rs` (new) -- `Affine`, scene and placeable XML parsing, `local_footprint`, `place`, `mod_path`, `ModFiles`, `footprints`; tests, an ignored real-mods test.
- `crates/ad-io/src/placeables.rs` -- `Quad`, `Placeable::footprint`.
- `crates/ad-io/src/settings.rs` -- `mods_folder`.
- `crates/ad-app/src/jobs.rs` -- `JobResult::Footprints`, `spawn_footprints`, applying it; test.
- `crates/ad-app/src/main.rs` -- `mods_default` (main only), `footprints_for`, `mods_dir`, `start_footprints` (from `apply_settings`).
- `crates/ad-app/src/ui.rs` -- the Mods folder row.
- `crates/ad-app/src/view.rs` -- `PlaceableShape`, `in_quad`, footprint painting and hover; tests; frame-time test with the real footprints.

## Implementation Notes

- **Real mods (fixture):**
  - 73 of 117 buildings get a footprint, read in 250 ms (release).
  - The other 44 are map selling stations without areas, or buildings that use base-game models (`$data/...i3d`, e.g. easyShed01, farma1000), or the two mods not installed (StaffedWorkshop, Hirschfeld charging station).
- **Rotation convention:** x' = x cos θ + z sin θ, z' = −x sin θ + z cos θ (right-handed, y up), for i3d nodes and the savegame rotation alike.
  - Only rotation y is applied; x/z tilts of area nodes are ignored.
  - **The user's hand check confirms the sign against the map image.**
- **Folder change:** a result of an earlier folder can arrive after a newer one for the same document. This is rare (two folder changes within the read time); the last one to arrive wins.
- **Verification:**
  - fmt ok; clippy ok;
  - `cargo test --workspace` all pass (ad-app 461 + 4 ignored, ad-io 103 + 1 ignored);
  - frame time: every case ok with 73 footprints drawn.

## Verification

**Commands:**
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` -- expected: pass
- `cargo test -p ad-io --release -- --ignored real_mods_footprints --nocapture` -- expected: footprints for the fixture's mod buildings
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked:
  - `start_footprints` runs each frame from `apply_settings` but spawns only once per (epoch, folder);
  - `ModFiles` cannot leave the mods folder (`..` refused; zip names only looked up, not written);
  - the per-file cap stops a huge entry.
