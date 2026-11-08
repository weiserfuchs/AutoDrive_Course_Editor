---
title: 'A missing map image downloads in the background'
type: 'feature'
ticket: '4'
created: '2026-11-08'
status: 'planned'
baseline_revision: '07a263d'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** A config whose map has no local image opens on a blank background. Java downloads the image from `KillBait/AutoDrive_MapImages`, but on the UI thread, with no timeout and no size check (`MapImage.findImageLocationFor`, `FileUtils.copyURLToFile`).

**Approach:**
- `settings.toml` gains `online_map_images` in `[global]` (default `true`, as Java's `Check_Online_MapImages`).
- After a config loads and no local image is found (AD-14 steps 1 and 2), and the setting is on and the config names a map, a background job (AD-9) fetches `https://raw.githubusercontent.com/KillBait/AutoDrive_MapImages/master/mapImages/<name>/<name>.png` with `ureq` (connect timeout 10 s, overall timeout 60 s, body cap 32 MiB).
- The body must decode as a PNG (through the same loader as a local image, so a wrong size is scaled with the usual notice). It is then saved as `<exe dir>/mapImages/<name>/<name>.png` (atomic) and attached to the document if its epoch still matches (AD-18).
- While it runs, the top bar shows "Downloading map image…". On 404: notice "No map image online for <name>". On a timeout, a network error, an oversize or non-PNG body: notice naming the reason. The blank image stays and nothing is saved.
- The base URL is a constant in ad-io; tests point the job at a local test server.
- No heightmap download (the data repository holds images only).

## Boundaries & Constraints

**Always:** the UI never waits on the network; at most one download per load; the document data and history never change.

**Never:** no retries, no download when a local image exists, no settings dialog (epic-settings-ui-shell), no upstream URLs other than the data repository (AGENTS.md allows it as the data source).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Success | no local image, server returns a PNG | image attached and saved under `<exe>/mapImages/<name>/` | none |
| Local image | image found locally | no request | none |
| Setting off | `online_map_images = false` | no request | none |
| No map name | config without `<MapName>` | no request | none |
| 404 | server 404 | notice "No map image online for <name>"; blank stays | notice |
| Timeout | server never answers | notice after the timeout; UI kept running | notice |
| Oversize | body above 32 MiB | aborted while reading; notice | notice |
| Not a PNG | HTML body | notice; nothing saved | notice |
| Replaced | another config opened before the end | result not attached (file still saved for its map) | none |

</frozen-after-approval>

## Code Map

- `Cargo.toml` / `crates/ad-io/Cargo.toml` -- `ureq` 3.5.1 per the Stack (only the features needed for HTTPS GET; check the default TLS choice builds on Windows without extra tools).
- `crates/ad-io/src/download.rs` (new) -- `fetch_map_image(base_url, name, limits) -> Result<Vec<u8>, DownloadError>` with timeouts, the size cap and status handling; tests against a `std::net::TcpListener` test server (200, 404, slow, oversize, HTML).
- `crates/ad-io/src/settings.rs` -- `online_map_images`.
- `crates/ad-app/src/jobs.rs`, `main.rs`, `ui.rs` -- start after a load without an image, the job result with the epoch, the label and notices; reuse the save and attach paths of story 7.2.

## Tasks & Acceptance

**Execution:**
- [ ] `download.rs` and dependency -- tests for 404, Timeout, Oversize, Not a PNG, Success
- [ ] setting and app wiring -- tests for Local image, Setting off, No map name, Replaced, the label
- [ ] one ignored test that fetches the real `NF_Marsch_4fach` image (run by hand only)

**Acceptance Criteria:**
- Given no local image for the fixture's map, when the user opens the fixture, then the map image downloads without blocking the UI, appears, and is found locally at the next load.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass (no network in the default tests)
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
