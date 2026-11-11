---
type: epic
title: "Buildings from placeables.xml are shown on the map"
parent: initiative-rust-rewrite
covers: []
sources: []
companions: []
after: []
refined: true
risk: medium
---

# Buildings from placeables.xml are shown on the map

## Description

The user asked (2026-11-11) to see built buildings on the map, e.g. from the savegame's `placeables.xml`. The file holds each building's position, rotation, XML file and owner, but no size; the footprint is only in the placeable's own XML inside its mod zip.

## Outcome

With a `placeables.xml` beside the opened config, every building is visible on the map, coloured by owner, named, and later drawn with its real footprint.

## Requirements

- R1: `placeables.xml` beside the config is read for display only and never written; a missing file shows nothing, a broken one a notice.
- R2: each building with a position is a square in its owner's colour (map, own farm, other farm) under the network, named from a zoom on, with id and owner on hover; Display ▸ Placeables (F5) toggles it and persists.
- R3: with a mods folder set, a building's rotated footprint is read from its placeable XML in the mod zip and drawn instead of the square.

## Constraints

- C1: `placeables.xml` in the repo root is an FS25 fixture like `vehicles.xml`; never overwritten.
- C2: the frame budget (R15 of the initiative) still holds with the fixture's buildings drawn.

## Done when

- The fixture's 117 placed buildings show on the map, coloured by owner.
- With the mods folder, the footprints match the buildings on the map image.

## Boundaries

- Out: editing or moving buildings; base-game (`$data`) footprints unless they are readable without the game archives; heights.

## Notes

- Decision (user, 2026-11-11): symbol first, footprint later; all buildings coloured by owner; `placeables.xml` committed as a fixture.
- Finding: the fixture has 118 placeables, 117 with a position, all from mods (NF Marsch, Fed packs); farmId 0 = 35, 1 = 61, 15 = 22.
- Rotation is not drawn in story 1; its sign is checked against real footprints in story 2.
- Build order 1 → 2 → 3, one lane.
