<!-- bmad:context -->
<!-- Verified 2026-11-05 against c3b4f8b. Managed by bmad-project-context; edits inside this block are replaced on refresh. Keep anything you want preserved outside the markers. -->

## AutoDrive Course Editor

Editor for AutoDrive route networks (AutoDrive_config.xml) of Farming Simulator. Currently Java 11 / Swing (`src/main/java`), being fully rewritten in Rust with egui/eframe for performance. Planning artifacts live in `_bmad-output/`.

## Policy

- This fork is independent of KillBait/AutoDrive_Course_Editor: never point update checks, version/history URLs or links at upstream; use weiserfuchs/AutoDrive_Course_Editor.
- The Java code is reference only: read it to port behavior, delete or change it freely; no Java build needs to keep working.
- Work on a branch per story (`rust/<story-slug>`); commit there, merge to `master` only when `cargo build`, `cargo test` and `cargo clippy` pass.
- Never overwrite `AutoDrive_config.xml` in the repo root; it is the FS25 reference fixture. Write round-trip output to a temp dir and compare.
- Never commit build output (`target/`); it is gitignored.
- Code, comments and commit messages in English.

## Where things are

- Java entry point: `src/main/java/AutoDriveEditor/AutoDriveEditor.java`; road network model in `src/main/java/AutoDriveEditor/RoadNetwork/`, XML config in `src/main/java/AutoDriveEditor/XMLConfig/`.
- Resources to carry over (locales, themes, map images, icons): `src/main/resources/`.

## Running and verifying

- Maven is not installed here: do not try to build or run the Java editor; port from reading the source.
- TODO (Rust/egui, not yet created): record the crate location and any non-obvious build/run command here on the first refresh after the Rust project exists.
- Verify load/save with `AutoDrive_config.xml` (FS25); FS19/FS22 compatibility is not required for testing.

<!-- /bmad:context -->
