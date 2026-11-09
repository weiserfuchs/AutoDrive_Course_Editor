---
type: epic
title: "The Rust editor ships and Java is gone"
parent: initiative-rust-rewrite
covers: ["R15", "R16"]
sources: []
companions: []
after: []
refined: true
risk: medium
---

# The Rust editor ships and Java is gone

## Description

A tagged release publishes the Windows executable, parity is signed off against R1-R16, the frame budget is re-measured, and src/main/java with the Maven build is removed.

## Outcome

A tag `v0.9.0` publishes `AutoDriveCourseEditor.exe` on the fork's releases page; the repository holds only the Rust editor.

## Requirements

- R1: the release build is a windowed `AutoDriveCourseEditor.exe` (no console window) with version 0.9.0.
- R2: pushing a `v*` tag builds the exe on CI and attaches it to a draft release on weiserfuchs/AutoDrive_Course_Editor; the user publishes it.
- R3 (parent R15): the frame budget is re-measured on the fixture at minimum and maximum zoom and recorded.
- R4 (parent R1-R16): parity is signed off by the user against a checklist built from the requirements and the UI-shell inventory.
- R5: the Java sources, resources, Maven build, IntelliJ file, start script and upstream `version.xml`/`history.xml` are removed; the readme describes the Rust editor.

## Constraints

- C1: fork independence (AGENTS.md); the release, readme and links name only weiserfuchs/AutoDrive_Course_Editor, except the map-image data repo.
- C2: the repo-root fixtures `AutoDrive_config.xml` and `vehicles.xml` stay.
- C3: no tag is pushed and no release is published without the user's go-ahead.

## Done when

- A tagged release publishes the Windows x64 executable.
- Parity with R1-R16 is signed off against the fixture and the UI-shell inventory.
- R15 is re-measured and met.
- src/main/java and the Maven build are removed.

## Boundaries

- Out: new features; an exe icon, installer or code signing; Linux and macOS builds.

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md
- inventory — _bmad-output/initiative-rust-rewrite/research-ui-shell-inventory/research-ui-shell-inventory.md

## Notes

- Waits on epic 9 because: needs feature parity across all menus and tools.
- Decision (user, 2026-11-09): first release v0.9.0; a GitHub Action on tag builds it; exe name `AutoDriveCourseEditor.exe`; remove src/, pom.xml, the .iml, the .cmd, version.xml and history.xml; rewrite the readme.
- Finding: the exe opens a console window (no `windows_subsystem`), and there is no release profile.
- Parity sign-off comes before the Java removal, so the Java reference is still there while the user checks.
- Build order 1 → 2 → … → 7, one lane.
