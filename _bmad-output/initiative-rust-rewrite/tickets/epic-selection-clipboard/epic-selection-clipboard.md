---
type: epic
title: "Users select, align, copy and paste"
parent: initiative-rust-rewrite
covers: ["R4", "R6", "R15"]
sources: []
companions: []
after: []
refined: true
risk: medium
---

# Users select, align, copy and paste

## Description

Users select nodes by rectangle, lasso and click; move, rotate, delete, swap priority and flip on that selection; align it on an axis; edit a node's location; and copy, cut and paste, to the view centre or the original location. Every action undoes in one step, and dragging a large selection stays within the frame budget.

## Outcome

A course author reworks whole parts of the FS25 fixture's network at once (select, move, rotate, align, copy and paste) and undoes each step, with dragging as fluid as R15 requires.

## Requirements

- R1 (parent R6): Users select nodes by rectangle, lasso and click; align them on an axis; edit a node's location; and copy, cut and paste, either to the view centre or to the original location.
- R2 (parent R4): Users rotate selected nodes, and move, delete, swap priority and flip connections on a multi-node selection.
- R3 (parent R7, via R4 and R6): Every action of this epic undoes and redoes in one step.
- R4 (parent R15): Dragging or rotating a selection on the FS25 fixture renders each frame within 16 ms on the developer's machine, at any zoom, up to the whole network selected.

## Constraints

- C3 (parent): the FS25 fixture is the verification reference.
- C4 (parent): known Java defects are fixed rather than ported (AD-13): align-to-0 undo, Edit Location without undo, exclusive rectangle bounds.
- C5 (parent): the architecture spine binds, in particular AD-4, AD-5 (inclusive rectangle), AD-10 and AD-15.

## Done when

- Rectangle, lasso and click selection work on the fixture; the selection survives undo and redo pruned to existing nodes (AD-15).
- Move, rotate, delete, priority and flip act on a selection and undo in one step.
- Align, Edit Location, copy, cut and paste (view centre and original location) work on the fixture and undo in one step.
- The edited golden test covers every command this epic adds.
- The frame-time test measures dragging the whole network selected at minimum and maximum zoom within 16 ms.

## Boundaries

- Out: the single-node tools themselves (epic-node-connection-tools); curves and lines (epic-curves-lines); marker editing (epic-markers-parking); display filters and "select hidden" (H) with them (epic-settings-ui-shell); Focus Node (Shift+F) (epic-settings-ui-shell, R16).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md, AD-4, AD-5, AD-10, AD-13, AD-15
- research — _bmad-output/initiative-rust-rewrite/research-editing-tools-and-change-history/research-editing-tools-and-change-history.md, §1 (rotate) and §4 (selection, alignment, clipboard)

## Notes

- Waits on epic 3 because: needs the Tool trait and core edit commands.
- Decision (2026-11-07): rotation and the multi-node variants of move, delete, priority and flip move here from epic-node-connection-tools (its refinement), because Java applies them to the selection.
- Decision (2026-11-07): refined from the initiative's lines; no UX or architecture run.
- Decision (2026-11-07): selection as in Java: a right drag draws a rectangle or a lasso (toolbar switch) and toggles the nodes it covers; a right click on a node toggles that node; a right click on empty map clears the selection. The rectangle is inclusive (AD-5).
- Decision (2026-11-07): tools act on the selection only on a click, never while a selection is drawn: a click with delete, priority or flip on a selected node acts on the whole selection; the Delete key deletes the selection; the move tool drags the whole selection when the drag starts on a selected node. Java applies some tools immediately to a drawn rectangle; this is not ported, so a rectangle never deletes by accident.
- Decision (2026-11-07): rotate (R) as in Java: a handle rotates the selection about its centre in whole degrees; release commits one `MoveNodes` with absolute positions.
- Decision (2026-11-07): "select hidden" (H) and hidden-node rules come with the display filters in epic-settings-ui-shell; without filters no node is hidden.
- Decision (2026-11-07): a story measures dragging the whole network selected, as story 3.3 did for one node.
- Decision (2026-11-07): build order 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10, one lane: the stories share the view, the tools, input.rs and golden_edited.rs. Story 1 is the tracer (Document.selection through input, drawing and history pruning). Story 2 (dragging a selection, multi-node preview) is the least certain path and 3 measures it with the whole network selected.
- Decision (2026-11-07): paste takes heights from sample_height, or keeps the copied height when the map has no heightmap.
- Decision (2026-11-07): the two Java deviations of this epic (selection with any tool, no tool acting on a drawn rectangle) are recorded in AD-13.
- Decision (2026-11-07): rotate, align and Edit Location commit MoveNodes; per-action command names wait until undo entries get labels.
