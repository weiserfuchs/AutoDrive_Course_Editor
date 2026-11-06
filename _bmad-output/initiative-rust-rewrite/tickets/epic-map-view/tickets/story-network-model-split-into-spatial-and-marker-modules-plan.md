---
title: 'Network model split into spatial and marker modules'
type: 'refactor'
ticket: '1'
created: '2026-11-06'
status: done
baseline_revision: '98a867894a541a00a050090adbbff06c912a393f'
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

**Problem:** `crates/ad-core/src/network.rs` has about 1,320 lines. Besides the model, it holds the R-tree index, the spatial queries and the marker list (retro S1). The map view epic is about to add more queries to this file.

**Approach:** Move the spatial code and the marker code into two child modules of `network`, as a pure move. Behaviour and the public API stay unchanged; nothing outside `network` changes.

## Boundaries & Constraints

**Always:**
- Every public path stays valid: `ad_core::network::{Marker, MarkerDelta, Network, …}` and every `Network` method keep their names and signatures.
- Tests move with the code they test, unchanged except for `use` lines.
- Comments move along.

**Never:**
- Change logic, add queries or rename anything.
- Touch `command.rs`, `history.rs`, `ad-io` or `ad-app`. Their source and test files must compile unchanged.
- Make `Network` fields `pub` or `pub(crate)` to enable the split. Child modules already see the parent's private items.

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/network.rs`:
  - stays the parent module (no `mod.rs` rename) and gains `mod markers; mod spatial;`;
  - re-exports `pub use markers::{Marker, MarkerDelta};`.
- New `crates/ad-core/src/network/spatial.rs` takes:
  - the `NodeEntry`/`EdgeEntry` aliases (≈l.171-172) and `edge_entry` (≈l.184);
  - `point_in_polygon` (≈l.189);
  - `drop_edge` and `put_edge` (≈l.360-373), which become `pub(super)` methods in an `impl Network` block;
  - the queries `nearest`, `in_rect`, `in_polygon`, `within` and `edges_near` (≈l.656-716);
  - the tests `polygon_literal_shapes`, `distance_bounds_are_inclusive`, `seg_dist2`, `check_queries` and `queries_match_brute_force_after_connects_and_moves` (≈l.995-1039, 1179-1320).
- New `crates/ad-core/src/network/markers.rs` takes:
  - `Marker` (≈l.107) and `MarkerDelta` (≈l.163);
  - `push_marker`, `markers` (≈l.277, 290), `set_marker`, `remove_marker` and `revert_marker` (≈l.579-630);
  - the test `marker_deltas_revert` (≈l.947).
- **Stay in `network.rs`:**
  - `NodeId`, `NodeFlag`, `Node`, `LoadedNode`, the shapes and kinds, `NetworkError`, the link deltas, `RemovedNode`;
  - the `Network` struct and its fields;
  - `from_loaded`, `add_node`, `move_node` and `remove_node`/`reinsert`. These insert into or remove from `node_index` directly; keep that, and use the aliases from `spatial` (`pub(super)`).
  - `connection_kind` and adjacency;
  - all other tests.
- **Shared test helpers:** `Lcg` (≈l.726) is used only by the query tests, so it moves to `spatial`'s tests. `two`, `out` and `inc` stay. If a moved test needs a helper that stays, make it `pub(super)` in `network`'s test module and import it.

## Tasks & Acceptance

**Execution:**
- [ ] `crates/ad-core/src/network/spatial.rs` -- create it with the spatial items and tests listed in the Code Map -- the queries get their own module before the map view adds more.
- [ ] `crates/ad-core/src/network/markers.rs` -- create it with the marker items and test -- the markers epic changes them on their own.
- [ ] `crates/ad-core/src/network.rs` -- remove the moved items, declare the modules, re-export `Marker`/`MarkerDelta`, and fix the imports.

**Acceptance Criteria:**
- Given the workspace after the move, when `cargo test --workspace` runs, then the same number of tests pass as at the baseline (ad-core 46), with both golden tests included.
- Given `git diff --stat` against the baseline, then only the three `network` files changed.
- Given `network.rs`, then it holds no `RTree`, `AABB`, `locate_*` or marker method code.

## Implementation Notes

- Implemented by a subagent as a pure move. `NodeEntry`, `EdgeEntry`, `edge_entry`, `drop_edge`, `put_edge` and `edges_at` are `pub(super)` in `spatial.rs`. The test helpers `two` and `HUGE` became `pub(super)` in `network.rs`'s test module. All 20 network tests kept: 16 in `network.rs`, 1 in `markers.rs`, 3 in `spatial.rs`.

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 1 low, 1 false.
  - low, patch: `move_node` and `remove_node` still searched the edge R-tree directly (`AABB`, `locate_in_envelope_intersecting`), failing AC3. Extracted `pub(super) fn edges_at` into `spatial.rs`; both callers keep their filters; `network.rs` now has no `AABB` or `locate_*`.
  - false: "AC2 fails because the diff includes the plan file". The plan is this build's own record; the source diff touches only the three `network` files.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: no diff
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: no warnings
- `cargo test --workspace` -- expected: all pass; ad-core still runs 46 tests
- `grep -nE "RTree|AABB|locate_|fn (set|remove|revert)_marker" crates/ad-core/src/network.rs` -- expected: only the struct field types and the `node_index` updates in the model methods
