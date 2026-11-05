---
type: epic
title: "FS25 configs load, display and save losslessly"
parent: initiative-rust-rewrite
covers: ["R1", "R2", "R3", "R7"]
sources: []
companions: []
after: []
refined: false
risk: high
---

# FS25 configs load, display and save losslessly

## Description

The Rust workspace, CI, core model, History (undo/redo, saved_at), job runner and Settings skeleton exist; the editor opens the FS25 fixture in the background, draws its nodes and connections plainly, honours read-only legacy configs, and saves byte-identically.

## Outcome

The Rust workspace, CI, core model, History (undo/redo, saved_at), job runner and Settings skeleton exist.

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
