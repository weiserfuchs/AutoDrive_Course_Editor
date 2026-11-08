---
title: 'A missing map image downloads in the background'
type: 'feature'
ticket: '4'
created: '2026-11-08'
status: 'built'
baseline_revision: '07a263d'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
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
- [x] `download.rs` and dependency -- tests for 404, Timeout, Oversize, Not a PNG, Success
- [x] setting and app wiring -- tests for Local image, Setting off, No map name, Replaced, the label
- [x] one ignored test that fetches the real `NF_Marsch_4fach` image (run by hand only)

**Acceptance Criteria:**
- Given no local image for the fixture's map, when the user opens the fixture, then the map image downloads without blocking the UI, appears, and is found locally at the next load.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass (no network in the default tests)
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Dependency:** `ureq = "=3.5.1"`, `default-features = false, features = ["rustls"]` (rustls with ring and webpki-roots; no gzip, a PNG does not compress). Builds on Windows with the MSVC toolchain alone. New in Cargo.lock: base64, getrandom (second version), http, httparse, itoa, ring, rustls, rustls-pki-types, rustls-webpki, subtle, untrusted, ureq, ureq-proto, utf8-zero, wasi, webpki-roots, zeroize.
- **ad-io `download`:** `MAP_IMAGES_URL` (the data repository), `Limits { connect, global, max_bytes }` (default 10 s, 60 s, 32 MiB), `map_image_url` (name percent-encoded), `fetch_map_image(base_url, name, limits) -> Result<Vec<u8>, DownloadError>`. One agent per call with `timeout_connect`, `timeout_global` and `http_status_as_error(false)`; 404 is `NotFound`, other non-200 `Status`. A `Content-Length` above the cap is refused before reading; otherwise the body is read through `with_config().limit(cap)`, whose overflow is `BodyExceedsLimit` → `TooLarge` (an error, never a truncated body). `Timeout` and an I/O `TimedOut` both map to `Timeout`.
- **Test server:** `ad_io::download::test_server` (one-shot `TcpListener` on 127.0.0.1:0 answering raw HTTP/1.1), compiled for ad-io tests and, through the new `test-util` feature, for ad-app's dev-dependency only.
- **Decode:** `map_image::decode_map_image(bytes, source)` is what `load_map_image` now uses, so a download decodes and scales exactly as a local image (wrong size: scaled with the usual notice). The fetched bytes, not a re-encode, are saved atomically to `<exe dir>/mapImages/<name>/<name>.png`; a failed save is a notice, the image still attaches.
- **Starting the job:** `jobs::Loaded` gains a sixth element, `bool` "a local map image was found". `jobs::download_wanted(loaded, settings)` gives the map name when not found, the setting is on and the config names a map. `App::poll_jobs` reads it from the `Loaded` result and starts `spawn_download` right after the new document is in place, with its epoch; so once per load, never blocking the load. The repaint context is kept from the first frame (`App::repaint`) because `poll_jobs` has no context.
- **Result:** `JobResult::Downloaded { epoch, result }`. On a matching epoch the image replaces `map_image` (uploaded by the view as for an import) and any notice is appended to `doc.notice` (404: "No map image online for <name>"; others: "Map image for <name> could not be downloaded (<reason>)", reasons "timed out", "larger than N bytes", "not a PNG: …", "HTTP status N", or the network error). A stale epoch drops it; the file was saved by the job already. Network, history and dirty state never change.
- **UI:** "Downloading map image…" in the top bar while a download runs for the open document (`App::downloading: Option<u64>`, an epoch; `ui::download_label`). The Import menu is disabled meanwhile so a download cannot overwrite an image imported during it.
- **Setting:** `[global] online_map_images` (default `true`, serde default keeps old files loading).
- **Tests:** ad-io `download::tests` (success with path and encoding, 404, 500, silent server timeout, body stalling midway, oversize Content-Length, oversize chunked body, URL layout), `settings::tests::online_map_images_defaults_on_and_loads_off`; ad-app `jobs::tests` (`a_download_is_wanted_only_without_a_local_image_with_the_setting_and_a_name`, `a_downloaded_image_is_attached_saved_and_found_at_the_next_load`, `a_404_gives_the_no_image_online_notice_and_keeps_the_blank`, `a_timeout_oversize_or_html_body_names_the_reason_and_saves_nothing`, `a_download_for_a_replaced_document_is_not_attached`, `without_the_program_folder_nothing_is_requested`, ignored `real_map_image_download`), `download_tests` in main.rs (`a_load_without_an_image_downloads_it_once_with_the_label`, `no_download_with_a_local_image_the_setting_off_or_no_map_name`, `a_download_finishing_after_another_open_is_saved_but_not_attached`).
- **Dropped test:** a refused-connection test; on Windows a connect to a closed local port retries for about 2 s, so it hit the short test timeout instead of a network error.
- **Verification (2026-11-08):** `cargo fmt --all --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test --workspace` all pass (371 ad-app, 3 ignored; 73 ad-io; no network). `frame_time` release: every case ok, highest best-round max 8.953 ms (rotate, min zoom, whole network). The ignored `real_map_image_download` passed: the 2048x2048 image of `NF_Marsch_4fach` downloaded, decoded without a scaling notice and saved to a temp folder (about 1.3 s).
- **Open for the hand check:** opening the fixture with no `mapImages/NF_Marsch_4fach/` beside the exe shows the label, then the image; the next open finds it locally.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator), pass 1: no patch; 1 low deferred.
  - low, defer: a download still running after another config was opened can, on finishing, overwrite an image the user imported for the same map in the meantime (Import is enabled again for the new document). Narrow; appended to deferred-work.md.
  - checked: an oversize body is an error, never truncated; no network in the default tests; the real download passed by hand (2048², about 1.3 s).
- Verification (orchestrator re-run): fmt, clippy and workspace tests pass (ad-app 371).
