---
title: 'Recent files, the last folder and the window place are remembered'
type: 'feature'
ticket: '3'
created: '2026-11-09'
status: done
baseline_revision: 'b557a2a'
route: 'full'
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

**Problem:** Every start begins from scratch: no recent files, dialogs open in the system default folder, the window opens at its default size.

**Approach:**
- `settings.toml` keeps `recent_files` (newest first, at most 10, no duplicates), `last_folder` and `window` (outer position, inner size, maximized); they persist through story 1's write-back.
- A file goes into the recent files when it loads or saves successfully (configs and routes).
- File ▸ Open Recent lists the files by name with the folder, the full path as hover text, a missing file shown off, and "Clear list". Opening one goes through the unsaved-changes prompt; a file gone since fails the load and keeps the document (AD-12).
- Open, Routes… and the import dialogs (DDS, zip, map image, heightmap) start in the last used folder and remember the folder of the picked file. Save As keeps starting in the document's folder; the export dialogs keep their prefilled target.
- The window place is taken when a close is requested and restored at start. While maximized, the size from before is kept. A remembered size under 400×300 is ignored.
- The user started this story while away; the plan was not approved separately, and the hand check is pending.

## Boundaries & Constraints

**Never:** no recent list for map images or zips.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Order | 12 files, then one again | that one first, 10 kept | none |
| Load / save | successful load, failed load, successful save | loaded and saved files listed, the failed one not | none |
| Gone | recent file deleted | menu entry off; opening it anyway: error, document kept | error |
| Folder | picked `C:\maps\x.xml` | next dialog starts in `C:\maps` | none |
| Window | close while normal / maximized | place taken / previous size kept, maximized set | none |
| Tiny | remembered 100 px wide | default window | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/settings.rs` -- `recent_files`, `last_folder`, `window`, `WindowPlace`, `RECENT_MAX`, `add_recent`; tests.
- `crates/ad-app/src/main.rs` -- `pick`, `remember_folder`, `open_recent`, recent entries in `poll_jobs`, `window_place`, `start_viewport`, `Pending::OpenPath` (was `OpenRoute`); tests.
- `crates/ad-app/src/ui.rs` -- File ▸ Open Recent.

## Tasks & Acceptance

**Execution:**
- [x] settings -- tests Order, round trip
- [x] app -- tests Load / save, Gone, Folder, Window, Tiny

**Acceptance Criteria:**
- Given files opened and the window moved, when the user restarts, then Open Recent lists the files, the window is where it was, and Open… starts in the last folder.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass

## Implementation Notes

- Paths compare exactly, so the same file opened once as `C:\a.xml` and once as `c:\A.xml` gives two entries; acceptable for now.
- A window remembered on a monitor that is gone may open off screen; the OS usually moves it back. Not handled further (no monitor list in egui).
- Verification: fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 436 + 4 ignored, ad-io 92).

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: a cancelled close still records the place (harmless, the next close overwrites it); the recent menu checks files only while it is open.
