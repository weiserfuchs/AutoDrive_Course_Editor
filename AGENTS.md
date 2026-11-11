<!-- bmad:context -->
<!-- Verified 2026-11-05 against c3b4f8b. Managed by bmad-project-context; edits inside this block are replaced on refresh. Keep anything you want preserved outside the markers. -->

## AutoDrive Course Editor

Editor for AutoDrive route networks (AutoDrive_config.xml) of Farming Simulator. A Rust rewrite (egui/eframe) of the former Java/Swing editor. Planning artifacts live in `_bmad-output/`.

## Policy

- This fork is independent of KillBait/AutoDrive_Course_Editor: never point update checks, version/history URLs or links at upstream; use weiserfuchs/AutoDrive_Course_Editor.
- The Java editor was removed in epic 10; its source is in git history up to commit 0084b5e (`git show 0084b5e:src/main/java/...`) if old behaviour must be looked up.
- Work on a branch per story (`rust/<story-slug>`); commit there, merge to `master` only when `cargo build`, `cargo test` and `cargo clippy` pass.
- Never overwrite `AutoDrive_config.xml`, `vehicles.xml` or `placeables.xml` in the repo root; they are the FS25 reference fixtures (config and its savegame vehicles and placeables). Write round-trip output to a temp dir and compare.
- Never commit build output (`target/`); it is gitignored.
- Code, comments and commit messages in English.

## Where things are

- Planning: `_bmad-output/initiative-rust-rewrite/` (initiative, architecture, epics and story plans, UI-shell inventory, parity sign-off).
- Release: pushing a `v<version>` tag that matches the workspace version runs `.github/workflows/release.yml`, which attaches `AutoDriveCourseEditor.exe` to a draft release; a person publishes it.

## Running and verifying

- **Rust workspace:** `crates/ad-core` holds the model, commands and history, with no I/O and no UI. `crates/ad-io` holds the config XML and the settings. `crates/ad-app` is the eframe app. `rust-toolchain.toml` pins the toolchain.
- **Checks (CI runs the same):** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- **The app:** `cargo run -p ad-app --release` (binary `AutoDriveCourseEditor.exe`, no console window in release). It opens and saves only through file dialogs, so checks in the GUI need a human.
- **Settings and logs:** `%APPDATA%\AutoDrive Course Editor\config\settings.toml` and `...\data\logs\`.
- Verify load/save with `AutoDrive_config.xml` (FS25); FS19/FS22 compatibility is not required for testing.

<!-- /bmad:context -->
