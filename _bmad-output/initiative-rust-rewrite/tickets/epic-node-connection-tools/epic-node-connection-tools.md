---
type: epic
title: "Users edit nodes and connections"
parent: initiative-rust-rewrite
covers: ["R4", "R7"]
sources: []
companions: []
after: []
refined: false
risk: medium
---

# Users edit nodes and connections

## Description

Add, move, delete, connect, priority, flip and rotate tools work through undoable commands that respect read-only mode; per-edit node warnings update; the unsaved state is shown and closing asks for confirmation.

## Outcome

Add, move, delete, connect, priority, flip and rotate tools work through undoable commands that respect read-only mode.

## Done when

- Every R4 tool works on the fixture and each edit undoes and redoes.
- Read-only legacy configs reject all edits.
- The unsaved state is visible and closing asks for confirmation.
- Edits saved from the editor reload identically.

## Boundaries

- Out: selection, align, clipboard (epic-selection-clipboard); curves and lines (epic-curves-lines); markers (epic-markers-parking).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Waits on epic 2 because: needs ViewTransform, hit-testing and overlay drawing.
