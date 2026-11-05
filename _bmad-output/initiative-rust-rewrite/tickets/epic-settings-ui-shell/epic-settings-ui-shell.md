---
type: epic
title: "Settings, shortcuts and the full UI shell"
parent: initiative-rust-rewrite
covers: ["R14", "R16"]
sources: []
companions: []
after: []
refined: false
risk: medium
---

# Settings, shortcuts and the full UI shell

## Description

Settings, per-map info and shortcuts persist in TOML; every remaining Java menu, toggle and the update check exist, checked against the UI-shell inventory.

## Outcome

Settings, per-map info and shortcuts persist in TOML.

## Done when

- A UI-shell inventory (codebase research run 4) exists before the epic is refined.
- Settings, per-map info and shortcuts persist in TOML and are configurable.
- Every inventoried Java menu and toggle exists, and the update check reads the fork's releases.

## Boundaries

- Out: release packaging (epic-release-java-removal).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Waits on epic 4 because: needs selection and align tools.
- Waits on epic 5 because: needs curve tools.
- Waits on epic 6 because: needs marker tools.
- Waits on epic 7 because: needs import menus.
- Waits on epic 8 because: needs scan menus.
- Waits on epic 11 because: needs route-manager menus.
