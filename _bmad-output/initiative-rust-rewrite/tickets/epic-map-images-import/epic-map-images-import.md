---
type: epic
title: "Map images arrive from anywhere"
parent: initiative-rust-rewrite
covers: ["R9"]
sources: []
companions: []
after: []
refined: true
risk: medium
status: done
---

# Map images arrive from anywhere

## Description

Users get a map image and heightmap for any map: imported from a DDS file or a map mod's zip, downloaded on demand from the map-image data repository, or loaded and saved by hand. Imports run in the background, crop and scale per game version, and are saved where the next load finds them.

## Outcome

A course author opens a config of a map that has no local image and sees the right background, from a download, a DDS or the map's mod zip, without leaving the editor.

## Requirements

- R1 (parent R9): DDS files in the formats the games use (settled by a spike) decode to a 2048×2048 map image with the version crop rule (FS19 whole image; FS22/FS25 centre half).
- R2 (parent R9): A DDS import shows the image at once and saves it as `mapImages/<name>/<name>.png` next to the executable.
- R3 (parent R9): A mod-zip import reads the game version and the map's image and heightmap from the zip, previews both, and on confirm saves and attaches them; hostile archives are refused (AD-12).
- R4 (parent R9): When no local image exists and online images are enabled, the image downloads in the background from `KillBait/AutoDrive_MapImages` with a timeout and a size limit, is saved and attached.
- R5 (parent R9, R16): Users load a map-image PNG by hand, save the current map image, and import and export a heightmap.
- R6 (parent R15): No import or download blocks the UI; the frame budget holds while one runs.

## Constraints

- C3 (parent): the FS25 fixture and the user's FS25 `pda_map_H.dds` / `dem.png` are the verification reference; FS19 and FS22 are covered by generated samples only.
- C4 (parent): known Java defects are fixed rather than ported (AD-13): the blocking download without timeout or size check; imports that never reach disk.
- C5 (parent): the architecture spine binds, in particular AD-9, AD-11, AD-12 and AD-14.

## Done when

- The DDS spike records which formats decode and closes the architecture's deferred DDS item.
- DDS, zip and download each give the fixture's map its 2048×2048 image (zip and download verified with the FS25 data available), saved under `mapImages/` next to the executable and found at the next load.
- A failed import or download leaves the document unchanged and tells the user why.

## Boundaries

- Out: a settings dialog for the online option (epic-settings-ui-shell; until then `settings.toml`); map-image editing; FS19/FS22 verification with real files.

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md, AD-9, AD-11, AD-12, AD-14, Deferred (DDS coverage)
- research — _bmad-output/initiative-rust-rewrite/research-map-view-rendering-and-map-images/research-map-view-rendering-and-map-images.md, §1

## Notes

- Waits on epic 2 because: needs MapContext attach and map texture upload.
- Decision (2026-11-08): refined from the initiative's lines, the research and the Java source (`ImportManager`, `DDSReader`, `ImportFromZipMenu`, `MapImage`, the import menu items); no UX or architecture run.
- Decision (2026-11-08): the user has FS25 data only; the spike decodes the FS25 `pda_map_H.dds` (DXT1, 4096²) and generated samples of the other formats Java's `DDSReader` reads; FS19 and FS22 rules are unit-tested.
- Decision (2026-11-08): imported and downloaded images are written to `<exe dir>/mapImages/<name>/`, the first place the lookup searches (AD-14); during development that is `target/release/mapImages`.
- Decision (2026-11-08): unlike Java, a DDS import is also saved to disk at once; the download runs in the background with a timeout and size limit; `online_map_images` in `settings.toml` (default on, as Java).
- Decision (2026-11-08): the small map-image menu items (load and save PNG, import and export heightmap) belong here, beside the imports.
- Decision (2026-11-08): build order 1 → 2 → 3 → 4 → 5 → 6, one lane: the stories share ad-io images and the Import menu.
