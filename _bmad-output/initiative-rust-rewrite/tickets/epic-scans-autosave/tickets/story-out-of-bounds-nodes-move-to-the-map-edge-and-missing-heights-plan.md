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
- [ ] plans in ad-core -- tests for Outside, Scale, Edge, Heights
- [ ] app -- tests for None outside, No heightmap, None missing, Read-only, selection and undo
- [ ] golden case

**Acceptance Criteria:**
- Given a copy of the fixture with a node planted outside the map and one with y = -1 (prepared in the XML, since Edit location refuses both), when the user runs both fixes, then the first sits on the map edge and the second has a terrain height, each selected, each undone by one Ctrl+Z.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
