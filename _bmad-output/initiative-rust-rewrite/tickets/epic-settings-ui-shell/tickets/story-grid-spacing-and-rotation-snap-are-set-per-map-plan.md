---
title: 'Grid spacing and rotation snap are set per map'
type: 'feature'
ticket: '8'
created: '2026-11-09'
status: 'built'
baseline_revision: 'd2cac59'
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

**Problem:**
- The grid spacing can only be set in `settings.toml`.
- There are no subdivisions and no subdivision snap (Java D).
- The rotate tool snaps only with the shared grid snap and at a fixed 5°, while Java has its own rotation snap with a step of 5–90°.

**Approach:**
- **Display ▸ Grid and snap…:**
  - presets 1×1, 2×2, 4×4 and own X/Z spacing (from 0.1 m);
  - subdivisions 1–32;
  - Apply keeps them in the map's entry (AD-14);
  - the rotation step (5–90°) applies at once.
- **Display menu:** Snap to grid (S), Snap to subdivisions (D) and Rotation snap.
- **Grid snap:** rounds to the grid, or to grid / subdivisions while subdivision snap is on (`MapContext::snap_grid`). The node, connect and move tools and the snap preview use it.
- **Rotate tool:** turns in the rotation step while rotation snap is on (Java default on, 5°), else in whole degrees. Grid snap and Shift no longer affect it.
- Persisted: subdivisions per map; subdivision snap, rotation snap and step globally.
- The user started this story while away; the plan was not approved separately, and the hand check is pending.

## Boundaries & Constraints

**Never:** no change to how the grid is drawn.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Store | grid 4×4, 4 subdivisions on map "Neu" | entry has them; a reload restores them | none |
| Limits | spacing 0, 99 subdivisions | 0.1, 32 | clamped |
| D | D pressed | subdivision snap flips; snap grid 1×1 for 4×4 / 4 | none |
| Snap | node tool with snap at (1.3, 2.6) | node at (1, 3) | none |
| Rotation | step None / 5 / 15 | 37.4° → 37 / 35 / 30 | none |
| Dialog | Escape | closed, nothing applied | none |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/map_context.rs` -- `subdivisions`, `snap_subdivisions`, `snap_grid`, `MAX_SUBDIVISIONS`; test.
- `crates/ad-io/src/settings.rs` -- `MapSettings::grid_subdivisions`; `snap_subdivisions`, `rotation_snap`, `rotation_step`.
- `crates/ad-app/src/tools/rotate.rs`, `tools/mod.rs` -- `Tool::set_rotation_step`; `snap_angle` takes the step; tests.
- `crates/ad-app/src/tools/node.rs`, `connect.rs`, `move_node.rs` -- snap to `snap_grid()`.
- `crates/ad-app/src/jobs.rs`, `main.rs`, `input.rs`, `ui.rs` -- load and store the subdivisions, `set_grid`, D, the grid dialog, the menu items; tests.

## Tasks & Acceptance

**Execution:**
- [x] model and settings -- tests Store, Limits
- [x] snapping and rotation -- tests D, Snap, Rotation
- [x] dialog and menu -- test Dialog

**Acceptance Criteria:**
- Given an open map, when the user sets a 4×4 grid with 4 subdivisions and turns on subdivision snap, then new nodes snap to 1 m steps, and after a restart the map has the same grid.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Not drawn:** subdivision lines are not drawn; the grid shows its main lines only.
- **Behaviour change:** rotation now snaps in 5° steps by default, even with grid snap off (Java's default rotation snap). Before, it snapped only with grid snap, and Shift inverted it.
- **D:** D now toggles subdivision snap. Before, a plain D did nothing (the old test said so); Shift+D still deletes a marker.
- **Verification:** fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 452 + 4 ignored); frame time ok.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: the app hands the rotation step to the active tool every frame, so a tool picked from the tool bar or by R gets it before its first drag.
