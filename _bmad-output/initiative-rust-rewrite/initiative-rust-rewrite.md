---
type: initiative
title: "AutoDrive course editing is fast: the editor runs natively in Rust"
parent: none
sources:
  - _bmad-output/initiative-rust-rewrite/research-road-network-data-model-and-config-forma/research-road-network-data-model-and-config-forma.md
  - _bmad-output/initiative-rust-rewrite/research-map-view-rendering-and-map-images/research-map-view-rendering-and-map-images.md
  - _bmad-output/initiative-rust-rewrite/research-editing-tools-and-change-history/research-editing-tools-and-change-history.md
companions:
  - _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md
after: []
risk: high
---

# AutoDrive course editing is fast: the editor runs natively in Rust

## Description

The Java/Swing AutoDrive Course Editor (about 36,000 lines in `src/main/java`) is fully rewritten as a native Rust/egui application, because the Java editor is too slow on large route networks. When this initiative is done, every user-facing feature of the Java editor works in the Rust editor. FS25 configs round-trip without loss. The Java code is removed. The three codebase research documents are the feature baseline; the architecture spine fixes how the parts fit together.

## Outcome

Course authors editing large AutoDrive networks get a fluid editor, measured by R15's frame budget on the FS25 fixture, with no loss of features or data compared with the Java editor.

## Requirements

- R1 (research-road-network §1, §2): Opening the FS25 fixture `AutoDrive_config.xml` and saving it unchanged yields a byte-identical file.
- R2 (research-road-network §1, §2): After edits, save writes waypoints and markers in the game's format and keeps every other element of the config.
- R3 (research-road-network §4): FS19 and FS22 configs load and save with their version-specific handling, and legacy pre-1.1.0.6 configs open read-only.
- R4 (research-editing-tools §1): Users add, move and delete nodes; create and remove regular, reverse and dual connections; swap node priority; flip direction; and rotate nodes.
- R5 (research-editing-tools §2): Users create Bézier curves, arc splines and straight lines with adjustable interpolation, and commit them as nodes and connections.
- R6 (research-editing-tools §4): Users select nodes by rectangle, lasso and click; align them on an axis; edit a node's location; and copy, cut and paste, either to the view centre or to the original location.
- R7 (research-editing-tools §3): Every edit can be undone and redone. Unsaved changes are shown, and the user is asked to confirm before closing.
- R8 (research-map-view §2, §3, §4): The map view shows the map image, nodes, connections coloured by kind, markers, parking and warning icons and the grid, with zoom and pan.
- R9 (research-map-view §1, §3): Map images and heightmaps are found locally, downloaded on demand, or imported from DDS files and mod zips. New nodes take their height from the heightmap.
- R10 (research-road-network §1, §3): Users add, edit and delete markers and groups. Park destinations in `vehicles.xml` are kept and updated on save.
- R11 (research-road-network §3, research-editing-tools §3): Scans find and fix overlapping nodes (merging them after writing a backup file), out-of-bounds nodes and inconsistent connections.
- R12 (research-road-network §2): Autosave writes rotating backup files without changing the opened file or its saved state.
- R13 (research-road-network §2): Route-manager XML files load and save.
- R14 (research-editing-tools §1; user 2026-11-05): Settings, per-map info and keyboard shortcuts persist in a TOML file and are configurable. Redo defaults to Ctrl+Shift+Z.
- R15 (user goal, 2026-11-05): Editing the FS25 fixture stays fluid. At any zoom, pan, zoom and drag each render a frame within 16 ms on the developer's machine.
- R16 (user scope, 2026-11-05): Every remaining user-facing feature of the Java editor is available, judged against an inventory of the UI shell. This covers menus, display and debug toggles, and the update check against the fork.

## Constraints

- C1 (user, 2026-11-05): The editor is built in Rust with egui/eframe and ships as a single Windows x64 executable. No parallel runnable Java version is kept.
- C2 (user, 2026-11-05): The fork is independent of the KillBait editor repository. The update check targets `weiserfuchs/AutoDrive_Course_Editor`. Map images come from `KillBait/AutoDrive_MapImages`.
- C3 (user, 2026-11-05): The FS25 fixture is the verification reference. FS19 and FS22 are covered by unit tests only.
- C4 (user, 2026-11-05): Known Java defects are fixed rather than ported (architecture AD-13). Settings start fresh; `EditorConfig.xml` is not imported.
- C5: The architecture spine binds every epic.

## Done when

- The release build loads the FS25 fixture and saves it byte-identically, and the edited-document round-trip test passes in CI.
- Every requirement R1–R16 is demonstrated in the released executable against the fixture. For R16, this is checked against the UI-shell inventory.
- R15's frame budget is measured and met on the fixture at minimum and maximum zoom.
- A tagged release of the fork publishes the Windows x64 executable, and CI is green on `master`.
- `src/main/java` and the Maven build are removed from the repository.

## Boundaries

- Out: platforms other than Windows x64, translations beyond English, importing Java editor settings, telemetry, an installer, keeping the Java editor runnable.
- Out: contributing changes back to the KillBait upstream repository.
- Out: the experimental Java tools ScaleButton and RoadConnection.
- Touch point: `KillBait/AutoDrive_MapImages` — map images downloaded read-only; owner: epic-map-images-import (download, DDS, zip); local lookup: epic-map-view
- Touch point: GitHub Releases on the fork — update-check source; owner: epic-release-java-removal
- Tracer path: open the FS25 fixture, see the map and network, add a node, undo it, save, and get an identical file.

## References

- research — _bmad-output/initiative-rust-rewrite/research-road-network-data-model-and-config-forma/research-road-network-data-model-and-config-forma.md, §1–§5
- research — _bmad-output/initiative-rust-rewrite/research-map-view-rendering-and-map-images/research-map-view-rendering-and-map-images.md, §1–§4
- research — _bmad-output/initiative-rust-rewrite/research-editing-tools-and-change-history/research-editing-tools-and-change-history.md, §1–§4
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Assumption: R15's 16 ms frame budget (60 fps) on the developer's machine is the performance target.
- Open question: R16 needs a UI-shell inventory (codebase research run 4); it runs as the first step of refining epic-settings-ui-shell.
- Decision (2026-11-05): epics cut by capability; 11 epics in the order of tickets/tickets.toml; route-manager split from scans into its own epic.
- Decision (2026-11-05): tracer path spans epics 1–3 (load fixture, see network, add node, undo, save identically).
- Decision (2026-11-05): ArcSpline stays in scope (epic-curves-lines); the experimental ScaleButton and RoadConnection tools are out of scope.
- Decision (2026-11-05): the UI-shell inventory runs as the first step of refining epic-settings-ui-shell.
- Decision (2026-11-05): epics 4–8 may run in parallel after epic 3; building them sequentially is recommended because they share the toolbar and shortcut table.
