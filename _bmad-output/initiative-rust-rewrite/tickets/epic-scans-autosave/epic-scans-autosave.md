---
type: epic
title: "The network stays healthy and recoverable"
parent: initiative-rust-rewrite
covers: ["R11", "R12"]
sources: []
companions: []
after: []
refined: false
risk: medium
---

# The network stays healthy and recoverable

## Description

Overlap, out-of-bounds and connection-consistency scans fix problems undoably; merge backups and rotating autosave write side files.

## Outcome

Overlap, out-of-bounds and connection-consistency scans fix problems undoably.

## Done when

- Overlap, out-of-bounds and connection-consistency scans report and fix undoably.
- A merge writes <config>_mergeBackup.xml first.
- Autosave writes rotating slots without touching path or saved state (AD-16).

## Boundaries

- Out: route-manager files (epic-route-manager).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Waits on epic 3 because: needs edit commands and the job runner.
