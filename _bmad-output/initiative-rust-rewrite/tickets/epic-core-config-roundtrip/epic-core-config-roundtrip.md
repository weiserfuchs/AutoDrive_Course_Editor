---
type: epic
title: "FS25 configs load, display and save losslessly"
parent: initiative-rust-rewrite
covers: ["R1", "R2", "R3", "R7"]
sources: []
companions: [_bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md]
after: []
refined: true
risk: high
---

# FS25 configs load, display and save losslessly

## Description

The Rust workspace, CI, core model, History (undo/redo, saved_at), job runner and Settings skeleton exist; the editor opens the FS25 fixture in the background, draws its nodes and connections plainly, honours read-only legacy configs, and saves byte-identically.

## Outcome

The developer can open the FS25 fixture in the Rust editor and save it without losing a byte, on a tested core that every later epic builds on.

## Requirements

- R1 (parent R1): Opening the FS25 fixture `AutoDrive_config.xml` and saving it unchanged yields a byte-identical file.
- R2 (parent R2): After edits, save writes waypoints and markers in the game's format and keeps every other element of the config.
- R3 (parent R3): FS19 and FS22 configs load and save with their version-specific handling, and legacy pre-1.1.0.6 configs open read-only.
- R4 (parent R7): Every edit can be undone and redone, and the document knows whether it has unsaved changes. The undo/redo controls and close confirmation belong to epic-node-connection-tools.

## Constraints

- Parent C1, C3 and C5 bind. The architecture spine AD-1, AD-2, AD-3, AD-4, AD-5, AD-6, AD-9, AD-11, AD-12, AD-16 (save path only) and AD-18 apply directly.

## Done when

- The fixture loads, renders plainly and saves byte-identically in CI (AD-6 golden tests).
- The edited-document round-trip test passes.
- FS19/FS22 unit tests and read-only legacy handling pass.
- Undo/redo history and saved state work through History (AD-4).
- CI (fmt, clippy, test, AD-1 dependency check) is green on master.

## Boundaries

- Out: map image, real rendering (epic-map-view); editing tools (epic-node-connection-tools); user-facing settings UI (epic-settings-ui-shell).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Decision (2026-11-05): refined from the initiative's lines alone; the architecture spine settles the shared decisions, so no spike is needed.
- Decision (2026-11-05): plain node and connection drawing (dots and lines on an empty canvas) is in scope here so the tracer is visible; the real map view is epic-map-view.
- Decision (2026-11-05): tracer bullet is story 2 (fixture opens, shows, saves byte-identically); story 1 is its hitl setup. Next comes the least certain work: the full model (3) and edited saves (5).
- Decision (2026-11-05): stories 6 and 8 are chained after 5 and 7 because they touch the same config I/O and app startup code.
- Decision (2026-11-05): a config with an unknown game version opens read-only with a notice (architecture AD-11 amended).
- Decision (2026-11-05): story 1 (setup) and story 2 (tracer) stay separate.
