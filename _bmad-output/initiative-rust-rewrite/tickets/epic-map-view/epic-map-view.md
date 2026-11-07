---
type: epic
title: "The map view is fast and complete"
parent: initiative-rust-rewrite
covers: ["R8", "R9", "R15"]
sources: []
companions: []
after: []
refined: true
risk: high
status: done
---

# The map view is fast and complete

## Description

Local map-image and heightmap lookup, PNG load, MapContext attach and height sampling exist; map image, nodes, connections, markers, icons and grid render with cursor-centred zoom and pan within the frame budget on the fixture.

## Outcome

Opening the FS25 fixture shows the NF_Marsch_4fach map image with the whole network drawn on top in the Java editor's look, and zoom and pan stay within R15's frame budget.

## Requirements

- R1 (parent R8): The map view shows the map image, nodes, connections coloured by kind, markers, parking and warning icons and the grid, with zoom and pan.
- R2 (parent R9): Map images and heightmaps are found locally, downloaded on demand, or imported from DDS files and mod zips. New nodes take their height from the heightmap.
- R3 (parent R15): Editing the FS25 fixture stays fluid. At any zoom, pan, zoom and drag each render a frame within 16 ms on the developer's machine.

## Done when

- A rendering spike on the fixture confirms the frame budget before the design is locked (AD-8).
- Map image, heightmap, nodes, connections, markers, icons and grid render from local files.
- Cursor-centred zoom and pan meet R15's budget on the fixture.
- Height sampling for new nodes is available from ad-core (AD-7).

## Boundaries

- Out: download, DDS and zip import (epic-map-images-import; R2's local part only here); editing tools (epic-node-connection-tools); warnings that need a scan, such as overlapping nodes (epic-scans-autosave); a settings UI for per-map values (epic-settings-ui-shell); the frame budget while dragging nodes (epic-node-connection-tools).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Waits on epic 1 because: needs Document, Network queries and a loaded fixture.
- Decision (2026-11-06): refined from the initiative's lines; no UX or architecture run. The Java look (colours, node sizes, SVG icons in src/main/resources/editor) is ported as is, fixing only known defects (AD-13).
- Decision (2026-11-06): the frame budget is measured headlessly (egui without a window: viewport query, shapes, tessellation) on the fixture at minimum and maximum zoom; the user confirms smoothness by hand once. The GPU share is covered only by that check.
- Decision (2026-11-06): warning icons drawn here come from node and marker data alone (parking, negative height); scan-based warnings come with epic-scans-autosave.
- Decision (2026-11-06): the opening story splits network.rs (retro S1) before the view adds queries.
- Decision (2026-11-06): the user supplied the fixture's map image (2048x2048 RGBA) and heightmap (2049x2049, 16-bit grey); they live beside the fixture as mapImages/NF_Marsch_4fach/NF_Marsch_4fach.png and terrain.heightmap.png, gitignored as third-party content. Automated tests use generated images.
- Note: the fixture spans about ±2000 m, so NF_Marsch_4fach needs map scale 2; one node lies at x = -9626, far outside the map.
- Decision (2026-11-06): build order 1 → 2 → 3 → 4 → 5 → 9 → 7 → 8, with 6 after 3 beside 4–5. Story 2 is the tracer and rendering spike; it is deliberately thinner than every layer because AD-8 requires the frame budget before the image and the look are built on it. The refactor (1) goes first because it splits the code the view queries. Story 3 precedes 6 because 6 reuses its MapContext and load path.
- Decision (2026-11-06): R3's budget while dragging nodes moves to epic-node-connection-tools, which owns dragging; this epic measures pan and zoom.
- Decision (2026-11-06): for the hand checks, NF_Marsch_4fach gets scale = 2 by editing settings.toml once; no special code.
- Decision (2026-11-06): MapContext lives in ad-core with map name, scale, heightmap and height_map_scale; the map texture stays on ad-app's Document, because ad-core must not depend on egui (AD-1 over AD-14). The architecture spine reconciliation (retro action item 4) records it.
- Decision (2026-11-06): a 16-bit grey heightmap is read directly as value/256, the Java scale at full precision (AD-13).
- Decision (2026-11-06): story 9 (hit-testing, hover, overlay hook) added from the set check, because epic-node-connection-tools needs it from this epic.
- Decision (2026-11-07): epic closed. The user checked the fixture with the real NF_Marsch_4fach image and heightmap: map alignment, the Java look, markers, grid, hover highlight, info box and node-id toggle look right, and pan and zoom stay smooth. Download, DDS and zip import remain with epic-map-images-import; the drag budget and the edge hit-test rule remain with epic-node-connection-tools.
