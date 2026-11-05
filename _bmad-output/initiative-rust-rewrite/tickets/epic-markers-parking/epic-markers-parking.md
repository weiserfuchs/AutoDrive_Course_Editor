---
type: epic
title: "Users manage markers, groups and parking"
parent: initiative-rust-rewrite
covers: ["R10"]
sources: []
companions: []
after: []
refined: false
risk: medium
---

# Users manage markers, groups and parking

## Description

Markers and groups are added, edited and deleted; vehicles.xml park destinations survive and update on save.

## Outcome

Markers and groups are added, edited and deleted.

## Done when

- Markers and groups are added, edited, renamed and deleted undoably.
- vehicles.xml park destinations survive a save and follow marker changes (AD-17).

## Boundaries

- Out: route-manager group list (epic-route-manager).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Waits on epic 3 because: needs the Tool trait and command pattern.
