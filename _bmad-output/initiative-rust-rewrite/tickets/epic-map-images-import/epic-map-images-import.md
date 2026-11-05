---
type: epic
title: "Map images arrive from anywhere"
parent: initiative-rust-rewrite
covers: ["R9"]
sources: []
companions: []
after: []
refined: false
risk: medium
---

# Map images arrive from anywhere

## Description

Map images and heightmaps are downloaded on demand and imported from DDS files and mod zips, proven by a DDS format spike; local lookup and attach come from epic 2.

## Outcome

Map images and heightmaps are downloaded on demand and imported from DDS files and mod zips, proven by a DDS format spike.

## Done when

- A DDS spike with real FS19, FS22 and FS25 samples settles format coverage first.
- DDS and mod-zip import produce 2048x2048 map images and heightmaps with the version crop rule.
- Map images download from KillBait/AutoDrive_MapImages with timeouts and size limits.

## Boundaries

- Out: local lookup and attach (epic-map-view).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Waits on epic 2 because: needs MapContext attach and map texture upload.
