---
type: epic
title: "The network stays healthy and recoverable"
parent: initiative-rust-rewrite
covers: ["R11", "R12"]
sources: []
companions: []
after: []
refined: true
risk: medium
---

# The network stays healthy and recoverable

## Description

Users find and fix overlapping nodes, nodes outside the map, nodes without a height and broken connection entries, each fix in one undoable step; a merge writes a backup first; autosave writes rotating side files without touching the document.

## Outcome

A course author keeps the FS25 fixture's network clean with a few menu actions and never loses more than a few minutes of work to a crash.

## Requirements

- R1 (parent R11): An overlap scan with a chosen distance (3D, default 0.1) marks overlapping nodes with the warning icon and reports their count; the marks follow later edits.
- R2 (parent R11): Merging the overlaps writes `<config>_mergeBackup.xml`, then merges each group into one node keeping every connection, in one undoable step.
- R3 (parent R11): Nodes outside the map move to the nearest map edge, and nodes without a height take it from the heightmap, each in one undoable step; the fixed nodes become the selection.
- R4 (parent R11): A connection check finds incoming entries without a matching outgoing one, duplicate entries and self-links, and removes them in one undoable step.
- R5 (parent R12): Autosave writes `<config>_autosave_<n>.xml` in rotating slots on an interval while the document has unsaved changes and no tool preview is active, never changing the path, the saved state or `vehicles.xml` (AD-16).

## Constraints

- C3 (parent): the FS25 fixture is the verification reference.
- C4 (parent): known Java defects are fixed rather than ported (AD-13): the ignored scan distance, the merge and height fix without undo, out-of-bounds nodes piled at the map centre.
- C5 (parent): the architecture spine binds, in particular AD-4, AD-6, AD-9 and AD-16.

## Done when

- Each scan and fix works on the fixture (or a copy with planted defects) and undoes in one step.
- A merge leaves `<config>_mergeBackup.xml` with the state before the merge.
- Autosave writes its slots on the interval and leaves path, saved state and `vehicles.xml` untouched.
- The golden tests cover every command this epic adds; the frame-time test still passes with overlap marks shown.

## Boundaries

- Out: route-manager files (epic-route-manager); a settings dialog for autosave (epic-settings-ui-shell; until then `settings.toml`); restoring from an autosave beyond opening the file.

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md, AD-4, AD-6, AD-9, AD-13, AD-16
- research — _bmad-output/initiative-rust-rewrite/research-editing-tools-and-change-history/research-editing-tools-and-change-history.md, §3 (change history)

## Notes

- Waits on epic 3 because: needs edit commands and the job runner.
- Decision (2026-11-08): refined from the initiative's lines and the Java source (`ScanManager`, the Scan menu items, `EditorXML` autosave options); no UX or architecture run.
- Decision (2026-11-08): out-of-bounds nodes move to the nearest map edge instead of Java's (0, 0) (user decision; AD-13).
- Decision (2026-11-08): the connection check covers incoming entries without a matching outgoing one, duplicate entries and self-links (Java has no such scan; user agreed to these three for now).
- Decision (2026-11-08): `*_autosave_*.xml` and `*_mergeBackup.xml` are gitignored, so side files beside the repo-root fixture are never committed.
- Decision (2026-11-08): autosave settings live in `settings.toml` (on, 10 min, 10 slots, as Java).
- Decision (2026-11-08): build order 1 → 2 → 3 → 4 → 5 → 6, one lane: the stories share the Scan menu, the warning marks and the golden tests.
