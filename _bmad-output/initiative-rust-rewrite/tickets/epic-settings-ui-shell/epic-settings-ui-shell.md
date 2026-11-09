---
type: epic
title: "Settings, shortcuts and the full UI shell"
parent: initiative-rust-rewrite
covers: ["R14", "R16"]
sources: ["_bmad-output/initiative-rust-rewrite/research-ui-shell-inventory/research-ui-shell-inventory.md"]
companions: []
after: []
refined: true
risk: medium
---

# Settings, shortcuts and the full UI shell

## Description

Settings, per-map info and shortcuts persist in TOML and are configurable in a settings window; a menu bar carries every Java menu the user keeps, and the update check reads the fork's releases. The UI-shell inventory (research-ui-shell-inventory) is the checklist.

## Outcome

Settings, per-map info and shortcuts persist in TOML; every kept Java menu, toggle and the update check exist.

## Requirements

- R1 (parent R14): every setting change and per-map value is written back to `settings.toml` and survives a restart.
- R2 (parent R16): a menu bar (File, Edit, Display, Scan, Import, Routes, Help) above the tool bar; Save writes in place, Save As asks.
- R3 (parent R14): a settings window edits the settings; key bindings are configurable.
- R4 (parent R16): the Java display, visibility, grid and snap toggles kept by the inventory exist.
- R5 (parent R16): the update check and the Help menu use weiserfuchs/AutoDrive_Course_Editor.

## Constraints

- C1: fork independence: no URL at KillBait/AutoDrive_Course_Editor; KillBait/AutoDrive_MapImages only as the map-image data repo (its "how to create map images" discussion link counts as data repo).
- C2: the architecture spine binds, in particular AD-1, AD-9 and AD-14 (per-map settings keyed by map name).

## Done when

- Every inventory row is present in Rust or listed as dropped below.
- Settings, per-map info and shortcuts persist in TOML and are configurable.
- The update check reads the fork's latest release.

## Boundaries

- Out: release packaging (epic-release-java-removal); localisation (Java ships only en_US); importing an old `EditorConfig.xml`; the message panel.
- Dropped (user, 2026-11-09): Debug menu (only "go to node id" becomes a normal command), Experimental mode and its buttons and tab, the Move Widget and its tab, the FlatLaf theme catalogue, the action bar ☰ panel toggles, AutoSave Apply Now/Later, Alt-letter mnemonics; map colours stay fixed at the Java defaults.

## References

- inventory — _bmad-output/initiative-rust-rewrite/research-ui-shell-inventory/research-ui-shell-inventory.md
- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md

## Notes

- Decision (user, 2026-11-09): menu bar plus tool bar; file shortcuts Ctrl+O, Ctrl+S, Ctrl+Shift+S, Ctrl+Q (configurable); theme light/dark/system, colours fixed; drop list above.
- Decision (orchestrator, 2026-11-09, open to change): version history is a link to the fork's releases page; the repo-root `version.xml` and `history.xml` are upstream content and are not used.
- Finding: `settings.toml` is written only when missing (`startup::load_settings`), so no change survives a restart today; story 1 fixes it first.
- Build order 1 → 2 → … → 11, one lane.
