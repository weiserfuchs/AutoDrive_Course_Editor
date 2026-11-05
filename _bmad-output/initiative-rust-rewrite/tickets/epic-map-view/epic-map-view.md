---
type: epic
title: "The map view is fast and complete"
parent: initiative-rust-rewrite
covers: ["R8", "R9", "R15"]
sources: []
companions: []
after: []
refined: false
risk: high
---

# The map view is fast and complete

## Description

Local map-image and heightmap lookup, PNG load, MapContext attach and height sampling exist; map image, nodes, connections, markers, icons and grid render with cursor-centred zoom and pan within the frame budget on the fixture.

## Outcome

Local map-image and heightmap lookup, PNG load, MapContext attach and height sampling exist.

## Done when

- A rendering spike on the fixture confirms the frame budget before the design is locked (AD-8).
- Map image, heightmap, nodes, connections, markers, icons and grid render from local files.
- Cursor-centred zoom and pan meet R15's budget on the fixture.
- Height sampling for new nodes is available from ad-core (AD-7).

## Boundaries

- Out: download, DDS and zip import (epic-map-images-import); editing tools (epic-node-connection-tools).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Waits on epic 1 because: needs Document, Network queries and a loaded fixture.
