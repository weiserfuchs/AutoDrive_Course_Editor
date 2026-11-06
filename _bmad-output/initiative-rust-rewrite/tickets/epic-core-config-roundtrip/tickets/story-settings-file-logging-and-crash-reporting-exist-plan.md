---
title: 'Settings file, logging and crash reporting exist'
type: 'feature'
ticket: '8'
created: '2026-11-06'
status: done
baseline_revision: 'c3cdf50b363ba6a69523435a25553878abc28972'
route: 'full'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: ['{project-root}/_bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The editor has no persistent settings, writes no log, and a panic just kills the window without a trace. Later epics need one `Settings` home (history cap, per-map scale, shortcuts), and AD-12 requires a panic hook that logs and shows a dialog.

**Approach:**
- Add a serde/TOML `Settings` with load/save in `ad-io::settings`.
- In `ad-app`:
  - At startup, resolve the OS config and data dirs with `directories`.
  - Start a daily rolling `tracing` log file.
  - Install a panic hook that logs and shows a message box.
  - Wire the history cap from settings into new documents.

## Boundaries & Constraints

**Always:**
- **`Settings` structure:**
  - `Settings { global: GlobalSettings, maps: BTreeMap<String, MapSettings> }`, with serde `default` on every struct and field, so a partial or older file loads.
  - `GlobalSettings { history_cap: usize }` (default `DEFAULT_CAP` = 500).
  - `MapSettings { scale: u32 }` (default 1).
  - Unknown keys are ignored.
- **`ad-io::settings::load(path)`:**
  - A missing file gives `Ok(Settings::default())`.
  - An unreadable or malformed file gives `Err(SettingsError)` naming the path and the TOML error.
- **`ad-io::settings::save(&Settings, path)`:**
  - Writes atomically: temp file in the same dir, `sync_all`, rename. On error the temp file is removed.
  - It creates the parent dir if it is missing.
- **App startup:**
  - Paths come from `directories::ProjectDirs::from("", "", "AutoDrive Course Editor")`: config dir `settings.toml`, and logs in the data dir under `logs/`.
  - Settings load once. A missing file is written with defaults.
  - A malformed file is logged as a warning and the defaults are used, but the file is not overwritten.
  - New documents use `History::new(settings.global.history_cap)`.
- **Logging:**
  - `tracing` + `tracing-subscriber` (fmt) writing through a `tracing-appender` daily rolling file `editor.log`.
  - Load and save start, success and failure are logged at info/warn.
  - No `println!`/`eprintln!`.
- **Panic hook:**
  - It is installed before anything else that can panic.
  - It logs the message and location at `error`, shows `rfd::MessageDialog` (error level) with the message and the log dir, and then calls the previous hook.
- **Debug panic:** in a debug build (`cfg!(debug_assertions)`), `AD_DEBUG_PANIC=1` panics right after the hook and logging are set up.
- **Dependencies:**
  - Versions from the architecture's Stack table, through `[workspace.dependencies]`.
  - If a pinned version does not resolve, use the nearest available version and note it in Implementation Notes.
  - `ad-core` gets nothing new.
- No unwrap or expect in `ad-io`.
- If no project dir can be resolved, the app still starts without a log or settings file and uses the defaults.

**Never:**
- A settings UI, shortcuts table, autosave, themes, or importing Java `EditorConfig.xml`.
- Per-map settings applied to any view (there is no `MapContext` yet).
- `anyhow`.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Round trip | settings with cap 200 and maps `{"Marsch": scale 4}` saved to a temp dir | reloads equal | none |
| Missing file | path that does not exist | `Settings::default()` (cap 500, no maps) | none |
| Partial file | `[global]` only, or `[maps.X]` without `scale` | missing fields take defaults | none |
| Malformed | `global = 5` / invalid TOML | no settings | `Err(SettingsError)` naming the path |
| Save into a missing dir | parent dir absent | dir created, file written, no temp file left | none |
| Save fails | parent is a file | nothing written | `Err(SettingsError)` naming the path |

</frozen-after-approval>

## Code Map

- `Cargo.toml` (root) -- add these to `[workspace.dependencies]`:
  - `serde = { version = "1.0.229", features = ["derive"] }` and `toml = "1.2.0"`.
  - `tracing = "0.1.44"`, `tracing-subscriber = "0.3.23"`, `tracing-appender = "0.2.5"` and `directories = "6.0.0"`.
- `crates/ad-io/Cargo.toml`, `src/lib.rs` -- add `serde` and `toml`, and `pub mod settings;`.
  - Copy the temp-file pattern from `config_xml::save_to` (≈l.640). Don't share code across modules for this.
- `crates/ad-app/Cargo.toml` -- add the tracing crates and `directories`.
- `crates/ad-app/src/main.rs`:
  - `main()` builds `App::default()`. `App` gains `settings: Settings`, and `App::default` becomes `App::new(settings)`.
  - Add the startup sequence here, or in a small `startup.rs` with the hook and logging.
- `crates/ad-app/src/jobs.rs` -- `replace_document` → `Document::new` uses `History::default()`. Thread the cap through: add a `history_cap` parameter to `Document::new` and `replace_document`.
  - `jobs::apply_result` is called from `App::poll_jobs`. Pass the cap in, and update the jobs tests (any cap is fine, use 500).
- `crates/ad-app/src/document.rs` -- `Document::new(config, path, epoch)`.
- `crates/ad-core/src/history.rs` -- `DEFAULT_CAP`. Reuse it for the settings default. `ad-io` already depends on `ad-core`.

## Tasks & Acceptance

**Execution:**
- [ ] `Cargo.toml`, `crates/ad-io/Cargo.toml`, `crates/ad-app/Cargo.toml` -- add the dependencies.
- [ ] `crates/ad-io/src/settings.rs` -- `Settings`, `GlobalSettings`, `MapSettings`, `SettingsError` (thiserror), `load` and `save`, plus unit tests for every matrix row in a unique temp dir per test, removed afterwards.
- [ ] `crates/ad-app/src/main.rs` (+ optional `startup.rs`) -- dirs, logging, panic hook, `AD_DEBUG_PANIC`, loading the settings and writing the defaults.
- [ ] `crates/ad-app/src/document.rs`, `jobs.rs`, `main.rs` -- the history cap from settings, and info/warn logs for load and save.

**Acceptance Criteria:**
- Given a first start, when the app opens, then `settings.toml` with defaults and a dated `editor.log` exist in the OS config and data dirs.
- Given a debug build with `AD_DEBUG_PANIC=1`, when the app starts, then a message box shows the panic, and the log file contains it at error level.

## Implementation Notes

- 2026-11-06: The agent ran the debug app once. It wrote `settings.toml` (cap 500) and `editor.log.2026-11-06` under `%APPDATA%\AutoDrive Course Editor`. The `AD_DEBUG_PANIC` message-box check was not run; it blocks on a dialog.

## Plan Change Log

## Review Triage Log

| # | Finding | Verdict | Route | Evidence |
|---|---------|---------|-------|----------|
| 1 | The crash dialog names a log dir even when logging failed to start | low | patch | `install_panic_hook` received `log_dir` before `start_logging`, which can return `None`. Logging now starts first, and the hook gets the dir only when logging is running; otherwise the dialog says no log is available. The hook is still installed before `AD_DEBUG_PANIC` and the settings load. `start_logging` cannot panic, because it returns `Option`. |
| 2 | The panic hook is installed after `ProjectDirs::from` | false | reject | `ProjectDirs::from` returns `Option` and does not panic, so nothing that can panic runs before the hook. |
| 3 | No test proves the history cap reaches `History::new` | low | patch | Every test passed 500 (= `DEFAULT_CAP`). Added `history_cap_reaches_new_document` with cap 1. |

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including the new settings tests and `dependency_boundary`

**Manual checks (if no CLI):**
- `cargo run -p ad-app`: `%APPDATA%\AutoDrive Course Editor\config\settings.toml` and `%APPDATA%\AutoDrive Course Editor\data\logs\editor.log.<date>` exist.
- `AD_DEBUG_PANIC=1 cargo run -p ad-app`: a message box appears, and the log has an ERROR line with the panic.
