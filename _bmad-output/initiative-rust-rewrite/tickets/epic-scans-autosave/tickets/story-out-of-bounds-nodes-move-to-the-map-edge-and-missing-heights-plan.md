---
title: 'Out-of-bounds nodes move to the map edge and missing heights are filled'
type: 'feature'
ticket: '3'
created: '2026-11-08'
status: 'planned'
baseline_revision: '313eace'
route: 'full'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Nodes outside the map are invisible and unusable in the game; nodes saved without a height (y = -1) float or sink. Java's `OutOfBoundsFixMenu` piles outside nodes at (0, 0) and `FixNodesHeightMenu` fills heights without undo.

**Approach:**
- **Scan menu "Fix nodes outside the map…"** (editable documents): a confirm modal shows the bounds (x and z from -1024·scale to 1024·scale) and the count of nodes outside. OK moves each to the nearest point of the map edge (x and z clamped to the bounds; y kept) in one `MoveNodes` with absolute positions. None outside: a notice "No nodes outside the map" and no modal.
- **Scan menu "Fix missing node heights…"** (editable documents with a heightmap; disabled without): a confirm modal shows the count of nodes with y = -1. OK sets each y to `sample_height` in one `MoveNodes`. None: a notice "No nodes without height".
- After either fix, the moved nodes become the selection (undo restores the earlier selection, as paste does) and a notice gives the count; overlap marks follow as after any edit.
- Pure planning functions in ad-core (no I/O): `out_of_bounds_plan(net, geometry)`, `missing_heights_plan(net, map)`.

## Boundaries & Constraints

**Always:** one transaction per fix through `Document::apply` (selection-aware); read-only disables both items.

**Never:** no change to nodes inside the map or with a height; no link change.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Outside | scale 1; nodes at (1500, 5, 0), (-2000, 5, 300), (100, 5, 1100), (1200, 5, -1300) | moved to (1024, 5, 0), (-1024, 5, 300), (100, 5, 1024), (1024, 5, -1024); selected; one undo | none |
| Scale | scale 2, node at (2000, 0, 0) | stays (inside ±2048) | none |
| Edge | node exactly at 1024 | inside, not moved | none |
| None outside | the fixture | notice, nothing committed | none |
| Heights | heightmap; nodes with y = -1 and others | -1 ones sampled, others unchanged; selected; one undo | none |
| No heightmap | map without heightmap | item disabled | none |
| None missing | no y = -1 | notice, nothing committed | none |
| Read-only | legacy config | both items disabled | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/scan.rs` -- the two plan functions and their tests.
- `crates/ad-app/src/document.rs`, `main.rs`, `ui.rs` -- menu items, confirm modals (as the merge prompt), apply with `apply_selecting`, notices.
- `crates/ad-io/tests/golden_edited.rs` -- a moved-to-edge node and a filled height save and reload (if `MoveNodes` coverage there is not already enough, one short case).

## Tasks & Acceptance

**Execution:**
- [x] plans in ad-core -- tests for Outside, Scale, Edge, Heights
- [x] app -- tests for None outside, No heightmap, None missing, Read-only, selection and undo
- [x] golden case

**Acceptance Criteria:**
- Given a copy of the fixture with a node planted outside the map and one with y = -1 (prepared in the XML, since Edit location refuses both), when the user runs both fixes, then the first sits on the map edge and the second has a terrain height, each selected, each undone by one Ctrl+Z.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **ad-core:** `scan::out_of_bounds_plan(net, geometry)` and `scan::missing_heights_plan(net, map)` return one `MoveNodes` of absolute (before, after) positions in network order, or an empty transaction when nothing needs fixing (as `merge_plan`). Bounds are `scan::map_half_size` = `MAP_PIXELS / 2 · scale`; x and z are clamped, so a node on the edge is inside and y is kept. A missing height is exactly `scan::NO_HEIGHT` (-1, as Java compares); without a heightmap the heights plan is empty.
- **App:** `document::Fix { OutOfBounds, MissingHeights }`; `Document::fix_count`, `apply_fix` (through `apply_selecting`, selecting `placed_nodes` of the entry, so undo restores the earlier selection and overlap marks follow) and `fix_notice`, which replaces the notice line of an earlier fix ("No nodes outside the map", "N nodes moved to the map edge", "No nodes without height", "N node heights filled") instead of piling them up. `App::can_fix` gates both items on `editable()` and the heights item also on a heightmap; `ask_fix` shows the `fix_prompt` modal (one modal for both fixes, as `merge_prompt`; the bounds as "X: -h to h / Z: -h to h") or only the notice when the count is 0; `finish_fix` re-checks `can_fix`. The prompt counts for `dialog_shown` and is closed when a new document loads.
- **Matrix row "None outside / the fixture" does not hold for the fixture:** at its map scale 2 (±2048) two nodes lie far west of the map (file ids 35400 and 35401, x = -9626.506 and -9615.043). The app test therefore fixes those two first (they land on x = -2048), then checks the "None outside" behaviour (notice, nothing committed) on the fixed fixture; "None missing" holds for the fixture as is (no y = -1).
- **Tests:** ad-core `nodes_outside_the_map_move_to_its_edge_and_back_on_revert` (Outside), `the_bounds_grow_with_the_scale_and_the_edge_is_inside` (Scale, Edge), `missing_heights_are_sampled_and_other_heights_stay` (Heights, no heightmap, none missing); ad-app `nodes_outside_the_map_move_to_its_edge_selected_and_one_undo_restores_them`, `missing_heights_are_sampled_selected_and_one_undo_restores_them` (also No heightmap), `the_fixture_has_no_node_without_height_and_after_its_fix_none_outside`, `a_read_only_document_disables_both_fixes`; golden `nodes_fixed_to_the_map_edge_and_a_filled_height_save_and_reload`.
- **Hand-test file:** `target/handtest/AutoDrive_config_defects.xml` (not committed), the fixture with node id 1 at x = 2500.000 (was -70.071; y 100.007, z 65.339) and node id 2 at y = -1 (was 100.000; x -70.372, z 61.963). At scale 2 the bounds fix finds 3 nodes (id 1, 35400, 35401); the heights fix needs the map's heightmap loaded.
- **Verification:** `cargo fmt --all --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test --workspace` all pass (ad-app 405 passed, 3 ignored); `frame_time` (release) every case ok, the slowest best-round max 10.3 ms (drag, min zoom, whole network selected), all others under 8 ms.

## Plan Change Log

## Review Triage Log
