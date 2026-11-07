---
type: epic
title: "Users edit nodes and connections"
parent: initiative-rust-rewrite
covers: ["R4", "R7", "R15"]
sources: []
companions: []
after: []
refined: true
risk: medium
---

# Users edit nodes and connections

## Description

Add, move, delete, connect, priority and flip tools act on the node or connection under the cursor through undoable commands that respect read-only mode; node warnings follow every edit; the unsaved state is shown and closing asks for confirmation; dragging a node stays within the frame budget. Rotation, and every tool acting on a multi-node selection, belong to epic-selection-clipboard.

## Outcome

A course author edits the FS25 fixture's nodes and connections with the toolbar tools, undoes and redoes each edit, and saves a file that reloads to the same network, with dragging as fluid as R15 requires.

## Requirements

- R1 (parent R4): Users add, move and delete nodes; create and remove regular, reverse and dual connections; swap node priority; and flip direction. Rotating nodes is owned by epic-selection-clipboard.
- R2 (parent R7): Every edit can be undone and redone. Unsaved changes are shown, and the user is asked to confirm before closing.
- R3 (parent R15): Dragging a node on the FS25 fixture renders each frame within 16 ms on the developer's machine, at any zoom.

## Constraints

- C3 (parent): the FS25 fixture is the verification reference.
- C4 (parent): known Java defects are fixed rather than ported (AD-13), here the connection toggle (AD-3) and Redo on Shift+Z.
- C5 (parent): the architecture spine binds, in particular AD-3, AD-4, AD-10, AD-11 and AD-18.

## Done when

- Every R1 tool works on the fixture and each edit undoes and redoes in one step.
- Read-only legacy configs reject all edits.
- The unsaved state is visible and closing asks for confirmation.
- Edits saved from the editor reload identically (the edited golden test covers every command this epic adds).
- The frame-time test measures dragging a node at minimum and maximum zoom within 16 ms.

## Boundaries

- Out: selection, rotate, align, clipboard and every multi-node variant of these tools (epic-selection-clipboard); curves and lines, including connecting with interpolated nodes and creating an end node on empty space (epic-curves-lines); markers (epic-markers-parking); persisting and configuring shortcuts (epic-settings-ui-shell).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md, AD-3, AD-4, AD-10, AD-11, AD-13, AD-18
- research — _bmad-output/initiative-rust-rewrite/research-editing-tools-and-change-history/research-editing-tools-and-change-history.md, §1 and §3 (Java behaviour to port)

## Notes

- Waits on epic 2 because: needs ViewTransform, hit-testing and overlay drawing.
- Decision (2026-11-07): refined from the initiative's lines; no UX or architecture run.
- Decision (2026-11-07): tools act on the node or connection under the cursor; rotate moves to epic-selection-clipboard, which also adds the multi-node variants of move, delete, priority and flip, because Java applies them to the rectangle selection.
- Decision (2026-11-07): a connect tool on an existing link of the same shape removes it with `Disconnect`; on a link of another shape it reshapes it with one transaction of `Disconnect` and `Connect` (no separate SetShape command, as the set check suggested). Network operations stay idempotent (AD-3); the choice is the tool's.
- Decision (2026-11-07): move drags the node directly, with no axis widget; grid snap (S toggles, Shift inverts) belongs to this epic.
- Decision (2026-11-07): shortcuts are one fixed table of the Java defaults in an ad-app module (N, M, Delete, P, F; Ctrl+Z undo, Ctrl+Shift+Z redo); persisting and configuring them is epic-settings-ui-shell (R14).
- Decision (2026-11-07): while a tool is active it owns the left mouse button and panning uses the middle button; with no tool, the left button pans as now. The tracer story checks Java's mapping and records it.
- Decision (2026-11-07): R15's budget while dragging nodes moved here from epic-map-view (its decision of 2026-11-06).
- Decision (2026-11-07): build order 10 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9, one lane. Story 10 is an opening refactor that moves the UI wiring out of main.rs (and renames AddNode to AddNodes, retro P2), so the tracer stays thin. Story 1 is the tracer: Tool trait, toolbar, shortcut table and the node tool through every layer. Story 2 (dragging) comes next as the least certain path, and 3 measures it. The stories share command.rs, the shortcut table and golden_edited.rs, so they run in sequence; 7 also waits on 4 for the edge hit-test rule.
- Note: node warnings today are only "no height" (y == -1); Network keeps that set current on every mutation, so warnings follow each edit without extra work.
