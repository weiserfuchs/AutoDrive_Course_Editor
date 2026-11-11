---
title: 'The Java editor and the Maven build are removed and the readme describes the Rust editor'
type: 'chore'
ticket: '5'
created: '2026-11-11'
status: done
baseline_revision: '0084b5e'
route: 'oneshot'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** parity is signed off (2026-11-11), so the Java editor is no longer needed as a reference. The repository still ships its sources, its Maven build and upstream's update files, and the readme describes the Java editor.

**Approach:**
- **Removed** (user decision, 2026-11-09):
  - `src/` (Java sources and resources, 352 files);
  - `pom.xml`, `AutoDriveCourseEditor.iml`, `Open Editor with Console to see errors.cmd`;
  - `version.xml` and `history.xml`.
- **readme.md** is rewritten for the Rust exe:
  - download, update check, compatibility (FS19/22/25, legacy read-only, routes, vehicles.xml);
  - settings, logs and autosave;
  - map images (data repo links), bug reports at the fork;
  - building from source, credits as in About.
- **AGENTS.md:**
  - no Java guidance; the Java source is reachable in git history at 0084b5e;
  - the planning folder and the release flow are listed.

## Boundaries & Constraints

**Never:** the fixtures `AutoDrive_config.xml` and `vehicles.xml` stay; no Rust code change.

</frozen-after-approval>

## Code Map

- deleted: `src/`, `pom.xml`, `AutoDriveCourseEditor.iml`, `Open Editor with Console to see errors.cmd`, `version.xml`, `history.xml`.
- `readme.md`, `AGENTS.md`.

## Implementation Notes

- No file under `crates/`, `.github/` or the root config refers to the removed files. Java names in Rust doc comments ("Java `MaxZoomLevel`") stay: they record where a behaviour came from.
- The readme's claims were checked against the code:
  - the autosave name `<stem>_autosave_<slot>.xml` next to the config;
  - `vehicles.xml` next to the config;
  - the pre-1.1.0.6 read-only message.
- `_bmad-output` keeps its Java paths as history.
- Verification: fmt ok; build ok; clippy ok; `cargo test --workspace` all pass (ad-app 456 + 4 ignored, ad-io 94).

## Verification

**Commands:**
- `cargo build --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` -- expected: pass

## Review Triage Log

- Quick review (orchestrator, inline): no finding. The upstream-URL test still passes; the readme links only the fork, the AutoDrive mod repos and the map-image data repo.
