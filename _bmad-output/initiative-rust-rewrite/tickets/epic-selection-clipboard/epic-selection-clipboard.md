---
type: epic
title: "Users select, align, copy and paste"
parent: initiative-rust-rewrite
covers: ["R4", "R6"]
sources: []
companions: []
after: []
refined: false
risk: medium
---

# Users select, align, copy and paste

## Description

Rectangle, lasso and click selection, axis alignment, location editing and copy/cut/paste work on the fixture and undo cleanly.

## Outcome

Rectangle, lasso and click selection, axis alignment, location editing and copy/cut/paste work on the fixture and undo cleanly.

## Done when

- Rectangle, lasso and click selection work with hidden-node rules (AD-15).
- Rotate, and move, delete, priority and flip on a multi-node selection, work and undo in one step.
- Align, edit location, copy, cut and paste work and undo in one step.
- Paste to view centre and to original location both work on the fixture.

## Boundaries

- Out: the single-node tools from epic-node-connection-tools; curves (epic-curves-lines).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Waits on epic 3 because: needs the Tool trait and core edit commands.
- Decision (2026-11-07): rotation and the multi-node variants of move, delete, priority and flip move here from epic-node-connection-tools (its refinement), because Java applies them to the selection.
