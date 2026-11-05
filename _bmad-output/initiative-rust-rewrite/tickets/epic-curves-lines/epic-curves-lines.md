---
type: epic
title: "Users draw curves and lines"
parent: initiative-rust-rewrite
covers: ["R5"]
sources: []
companions: []
after: []
refined: false
risk: medium
---

# Users draw curves and lines

## Description

Bezier curves, arc splines and straight lines preview, adjust and commit as nodes and connections with heightmap heights.

## Outcome

Bezier curves, arc splines and straight lines preview, adjust and commit as nodes and connections with heightmap heights.

## Done when

- Bezier curves, arc splines and lines preview, adjust and commit as one undoable transaction.
- Committed nodes take heights from the heightmap or interpolate (AD-7).
- Switching tools cancels an uncommitted preview (AD-10).

## Boundaries

- Out: ScaleButton and RoadConnection (out of the initiative).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Waits on epic 3 because: needs the Tool trait, connect commands and overlays.
