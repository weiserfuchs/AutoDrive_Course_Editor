---
type: epic
title: "Users select, align, copy and paste"
parent: initiative-rust-rewrite
covers: ["R6"]
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
- Align, edit location, copy, cut and paste work and undo in one step.
- Paste to view centre and to original location both work on the fixture.

## Boundaries

- Out: tools from epic-node-connection-tools; curves (epic-curves-lines).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Waits on epic 3 because: needs the Tool trait and core edit commands.
