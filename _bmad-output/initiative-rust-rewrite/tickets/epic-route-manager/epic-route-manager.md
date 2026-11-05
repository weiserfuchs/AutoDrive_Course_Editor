---
type: epic
title: "Route-manager files load and save"
parent: initiative-rust-rewrite
covers: ["R13"]
sources: []
companions: []
after: []
refined: false
risk: medium
---

# Route-manager files load and save

## Description

Route-manager XML (routeExport with markers and groups) loads into the same model and saves back in its own format.

## Outcome

Route-manager XML (routeExport with markers and groups) loads into the same model and saves back in its own format.

## Done when

- A route-manager XML loads, edits and saves back in its own format.
- Marker groups from route files are preserved.

## Boundaries

- Out: AutoDrive config files (epic-core-config-roundtrip).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Waits on epic 1 because: needs Document, Network and config I/O patterns.
- Waits on epic 6 because: needs marker and group model.
