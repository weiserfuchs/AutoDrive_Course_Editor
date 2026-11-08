---
type: epic
title: "Route-manager files load and save"
parent: initiative-rust-rewrite
covers: ["R13"]
sources: []
companions: []
after: []
refined: true
risk: medium
---

# Route-manager files load and save

## Description

Users open an AutoDrive route-manager export (`routeExport`), edit it with every tool, and save it back in its own format; a routes list (`routes.xml`) shows the exported routes and opens one.

## Outcome

A course author edits a route exported by the AutoDrive route manager and hands it back to the game without a savegame config.

## Requirements

- R1 (parent R13): A `routeExport` file opens into the same model as a config (waypoints, links, flags, markers), with its own group list kept; its map name comes from the `routes.xml` beside its folder, so the map image and heightmap are found.
- R2 (parent R13): Saving writes the `routeExport` format as the AutoDrive mod writes it; autosave and the merge backup write route files for a route document.
- R3 (parent R13): An "Open routes.xml…" list shows each route (name, map, revision, date) and opens the chosen one.

## Constraints

- C3 (parent): no real route file is available yet; the format reference is the AutoDrive mod's own Lua writer and reader (`FS25_AutoDrive` `scripts/XML.lua` `writeGraphToXml` / `readGraphFromXml`, `scripts/Manager/RoutesManager.lua`), cross-checked with the Java editor (`RoutesXML`). A real export becomes a fixture when the user has one.
- C5 (parent): the architecture spine binds, in particular AD-2, AD-6, AD-11, AD-16 and AD-17.

## Done when

- A route file built to the mod's format loads, survives edits and saves back to the same model (save → load equals the edited model; values at the format's 2 decimals).
- The route's group list survives a save, including groups no marker uses.
- The routes list opens a route with its map image.

## Boundaries

- Out: uploading routes to the game, editing `routes.xml` entries (name, revision, date), the mod's `defaultRoutes` export; byte-identical round trip until a real file shows the mod's whitespace.

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md, AD-2, AD-6, AD-11, AD-16, AD-17
- format — FS25_AutoDrive (Stephan-S/FS25_AutoDrive, branch main) `scripts/XML.lua` lines ~367-510, `scripts/Manager/RoutesManager.lua`

## Notes

- Waits on epic 1 because: needs Document, Network and config I/O patterns.
- Waits on epic 6 because: needs marker and group model.
- Decision (2026-11-08): the user has no route file; the format follows the AutoDrive mod's Lua code (user agreed), with a real export to be added as a fixture later.
- Decision (2026-11-08): format facts from the mod: root `routeExport`; `waypoints#c` count; `x`, `y`, `z` as `%.2f` joined by `;`; `out` and `in` as comma lists per node joined by `;`, `-1` for none; `flags` (missing means 0); markers `markers.m(i)#i` (1-based node id), `#n`, `#g`; groups `groups.g(i)#n`, `#i` in no fixed order. `routes.xml` (root `autoDriveRoutesManager`, `routes.route(i)#name/#fileName/#map/#revision/#date` and a `serverId` child) lives in `routesManager/`, the route files in `routesManager/routes/<16 random chars>.xml`.
- Decision (2026-11-08): flags 2 and 4 reset to 0 on load, as for FS22/FS25 configs and Java.
- Decision (2026-11-08): build order 1 → 2 → 3 → 4, one lane.
