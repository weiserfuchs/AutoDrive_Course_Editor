---
title: 'The display menu sets the map scale, labels, icons, node size and finds a node'
type: 'feature'
ticket: '6'
created: '2026-11-09'
status: 'built'
baseline_revision: '774fd0c'
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

**Problem:** Java's Display menu, its F1–F3, G, numpad +/− and Shift+F shortcuts, the map info popup and the debug "centre node" have no Rust counterpart. The map scale and node size can only be changed in `settings.toml`, and the parking icons cannot be hidden.

**Approach:**
- The Display menu holds:
  - **Map scale** with 24 widths: scale k is 2k km. It is enabled for a map with a name, since the scale is kept per map.
  - The on/off items **Grid** (G), **Marker names** (F1), **Marker icons** (F2), **Parking icons** (F3, a new view flag) and **Selected node id** (F4), each marked while on and showing its shortcut.
  - **Node size bigger / smaller** (+ / −, 0.1 steps, at least 0.1).
  - **Go to node id…**: a 1-based file id, as saved. It centres the view, or shows why the id was refused.
  - **Map info…**: map name, scale, node size, and whether a map image and a heightmap are shown.
- Shift+F centres the view on the hovered node.
- All new shortcuts are configurable (story 5).
- The flags persist (story 1). Scale and node size go into the map's `[maps.<name>]` entry. They are view settings, not edits, so they never make the document dirty (AD-14).
- The user started this story while away; the plan was not approved separately, and the hand check is pending.

## Boundaries & Constraints

**Never:** no hide filters (story 7); no grid presets (story 8).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Toggles | F1, F2, F3, G, F4 | flags flip, kept after a restart | none |
| Node size | + twice on map "Neu" (2.0) | 2.2 in the view and the entry | none |
| Limits | − 40 times, scale 99 | 0.1, 24 | clamped |
| Clean | any display change | document stays clean | none |
| Go to | "x", "0", "2" with one node / " 1 " | refused with the reason / view centred on node 1 | note in the dialog |
| Keys | F1–F3, G, +, −, Shift+F, F | the display actions; plain F still flips | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/settings.rs` -- `show_marker_names`, `show_marker_icons`, `show_parking_icons`, `show_node_id`.
- `crates/ad-app/src/input.rs` -- seven actions with Java's keys, configurable; test.
- `crates/ad-app/src/view.rs` -- `show_parking_icons`, `MapView::centre_on`.
- `crates/ad-app/src/document.rs`, `jobs.rs` -- `Document::has_image` (from the load's found flag; any shown image sets it).
- `crates/ad-app/src/main.rs` -- `store_map_settings`, `set_map_scale`, `change_node_size`, `centre_on_node`, `go_to_node`, `has_map_name`, the dialog state; tests.
- `crates/ad-app/src/ui.rs` -- the Display menu, `goto_dialog`, `map_info_window`; test.

## Tasks & Acceptance

**Execution:**
- [x] actions and flags -- tests Toggles, Keys
- [x] per-map values -- tests Node size, Limits, Clean
- [x] go to node and map info -- test Go to

**Acceptance Criteria:**
- Given an open map, when the user changes its scale and node size and toggles the labels, then the view follows at once and keeps them after a restart.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- egui does not tell the numpad keys apart: + and − on the main row act too. On a German layout + is its own key and − is the minus key.
- Java's Map scale items 1–10 also show the factor 2^i; here every item shows "2k km (×k)".
- Verification: fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 447 + 4 ignored); frame time ok.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: shortcuts do not act while the Go to dialog is shown (it is in `dialog_shown`); the non-modal Map info window has no text field, so + and − there still act on the map.
