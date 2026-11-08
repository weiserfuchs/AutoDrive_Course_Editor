---
type: epic
title: "Users draw curves and lines"
parent: initiative-rust-rewrite
covers: ["R5"]
sources: []
companions: []
after: []
refined: true
risk: medium
---

# Users draw curves and lines

## Description

Users draw straight lines with interpolated nodes, Bézier curves and arc splines. Each previews before it commits, can be adjusted while previewed, and commits as nodes and connections with heights from the heightmap, in one undoable step.

## Outcome

A course author lays out long roads and smooth bends on the FS25 fixture without placing every node by hand, and undoes each one in one step.

## Requirements

- R1 (parent R5): The connect tools draw straight lines: a link longer than the line node distance gets interpolated nodes at that spacing, and optionally a click on empty map ends the line at a new node.
- R2 (parent R5): Users create a Bézier curve between two nodes, adjust its interpolation points, control points, priority, connection type and direction in a curve panel and by dragging control points, and commit or cancel it.
- R3 (parent R5): Users create an arc spline, adjust its position, angles and radius with handles, and commit it.
- R4 (parent R7, via R5): Every commit is one transaction that undoes and redoes in one step; created nodes take heights per AD-7.
- R5 (parent R15): Previewing and adjusting a curve with the maximum interpolation points renders each frame within 16 ms on the developer's machine.

## Constraints

- C3 (parent): the FS25 fixture is the verification reference.
- C4 (parent): known Java defects are fixed rather than ported (AD-13): the Bézier divide-by-zero at one interpolation point, arc nodes committed at y = 0, curves surviving a tool switch.
- C5 (parent): the architecture spine binds, in particular AD-4, AD-7 and AD-10.

## Done when

- Lines, Bézier curves and arc splines preview, adjust and commit as one undoable transaction on the fixture.
- Committed nodes take heights from `sample_height`, or interpolate linearly between the end nodes by point index (AD-7).
- Switching tools or replacing the document cancels an uncommitted preview (AD-10).
- The edited golden test covers every command this epic adds or extends.
- The frame-time test measures a previewed curve at the maximum interpolation points within 16 ms.

## Boundaries

- Out: ScaleButton and RoadConnection (out of the initiative); a settings dialog for the line options (epic-settings-ui-shell; until then `settings.toml`); several curves previewed at once.

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md, AD-4, AD-7, AD-10, AD-13
- research — _bmad-output/initiative-rust-rewrite/research-editing-tools-and-change-history/research-editing-tools-and-change-history.md, §2 (curves)

## Notes

- Waits on epic 3 because: needs the Tool trait, connect commands and overlays.
- Waits on epic 4 because: needs PasteNodes and the selection-aware Document.
- Decision (2026-11-08): refined from the initiative's lines and the Java source; no UX or architecture run.
- Decision (2026-11-08): lines as in Java (`LinearLine`, `LinerLineBaseButton`): the connect tools (L) interpolate nodes every `LinearLineNodeDistance` (default 12) and, with `CreateLinearLineEndNode` (default off), end on a new node at an empty-map click; both options live in `settings.toml` until the settings UI; the preview shows the interpolated nodes.
- Decision (2026-11-08): Bézier (B) as in Java: start node, then end node; a curve panel with interpolation points 1..50 (default 6), control points 1..4, priority, connection type, swap direction, commit and cancel; control points drag with Java's move scaler 3.
- Decision (2026-11-08): arc spline as in Java's experimental tool (move, angle and radius handles, standalone arc), but with heights per AD-7 instead of y = 0; its own story near the end, so it can be dropped without touching the rest.
- Decision (2026-11-08): one curve at a time; switching tools or files discards it (AD-10), unlike Java.
- Decision (2026-11-08): commits extend `PasteNodes` (story 4.9) so a link end may be an existing node, instead of adding a near-duplicate command.
- Decision (2026-11-08): build order 1 → 2 → 3 → 4 → 5 → 6 → 7, one lane: the stories share the tools, the command and golden_edited.rs.
