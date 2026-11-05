---
title: 'Network model with stable ids, explicit connections and spatial queries'
type: 'feature'
ticket: '3'
created: '2026-11-05'
status: done
baseline_revision: 'a36ed99178f3ddc32d31ce568ceb61ab85b62d5d'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: ['{project-root}/_bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The `Network` from story 2 is a bag of public fields. Anyone can rewrite adjacency or move a node. There is no connection API, no derived connection kind and no spatial index, and every later tool and command needs all three.

**Approach:** Complete `ad-core::network` to AD-2, AD-3 and AD-5.
- Adjacency changes only through `connect`, `disconnect` and `set_shape`.
- One core function derives the displayed kind.
- `NodeFlag` keeps any file value.
- R-trees of nodes and edges answer every spatial query.
- `ad-io` and `ad-app` switch to the new API, and the golden round trip stays byte-identical.

## Boundaries & Constraints

**Always:**
- `NodeId` stays monotonic and is never reused. `order` is the only node sequence.
- `connect`, `disconnect` and `set_shape` are idempotent and never toggle. Load keeps `out`/`incoming` exactly as read, including duplicates and inconsistencies.
- Decision (user, 2026-11-05):
  - `connect` only adds missing entries.
  - `disconnect(a,b)` removes every link between a and b, in both directions.
  - `set_shape` sets the link to exactly that shape.
- The connection kind is derived, never stored. It follows the Java rules (`RoadMap.getConnection`).
- Positions, flags and adjacency are mutable only through `Network` methods, and every one of them keeps both R-trees in sync.
- Rectangle bounds are inclusive (AD-13). Queries work on the (x, z) plane.
- `ad-core` adds only `rstar` 0.14.0 and `thiserror` 2.0.21, both on the boundary test's `ALLOWED` list. No unwrap or expect.
- `golden_roundtrip` and the existing `config_xml` tests stay green.

**Never:**
- Commands, History, node deletion or reinsertion (story 4).
- Scans that repair inconsistent adjacency.
- Marker model changes (AD-17's per-node marker comes later).
- New UI behaviour in `ad-app`.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Connect one-way | a, b unlinked; `connect(a,b,OneWay)` | a.out=[b], b.in=[a]; kind(a,b)=Regular (flag 0) or SubPrio (flag 1) | none |
| Connect dual | `connect(a,b,Dual)` | a.out∋b, b.in∋a, b.out∋a, a.in∋b; kind = Dual both ways | none |
| Connect reverse | `connect(a,b,Reverse)` | a.out=[b] only; kind = Reverse | none |
| Idempotent | same `connect` twice; `disconnect` twice | no duplicate entries; second call changes nothing | none |
| Connect adds | OneWay a→b exists; `connect(a,b,Dual)` | becomes Dual | none |
| Set shape | Dual a↔b; `set_shape(a,b,OneWay)` | exactly OneWay a→b, no b→a entries | none |
| Disconnect | any link between a and b | all four a/b entries removed; edge index has no a–b edge | none |
| Crossed regular | a.out∋b, b.in∋a, b.out∋a, a.in∌b | CrossedRegular | none |
| Crossed reverse | a.out∋b, b.in∌a, b.out∋a, a.in∋b | CrossedReverse | none |
| Other combos / flag ≥ 2 on regular link | e.g. only a.out∋b and a.in∋b; or regular link from a node with flag 7 | Unknown | none |
| No link | a.out∌b | kind = None | none |
| Flag preserved | config with flags `0,7` | loads as `NodeFlag(7)`, saves `0,7` | none |
| Self or unknown node | `connect(a,a,…)`; any op on a missing id | state unchanged | `Err(NetworkError)` naming the node |
| Spatial queries | random 500-node network with links; moved nodes | nearest, rectangle, polygon, within-distance and edges-near-point equal a brute-force check | none |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/network.rs` -- story 2's minimal model, to be completed. Remove `node_mut` and `push_node`. Keep the marker API (`push_marker`, `markers`) unchanged.
- `crates/ad-core/Cargo.toml` and root `Cargo.toml` -- add `rstar = "0.14.0"` to `[workspace.dependencies]`. `thiserror` is already pinned there.
- `crates/ad-core/tests/dependency_boundary.rs` -- `ALLOWED` is `&[]` and its comment expects later stories to extend it. Add `"rstar"` and `"thiserror"`, and change nothing else.
- `crates/ad-io/src/config_xml.rs` -- `build_network` (≈l.200-260) calls `push_node` and then sets adjacency through `node_mut`. Switch it to `Network::from_loaded`. `join_nodes` and the tests read `node.outgoing`/`incoming`, so switch them to the accessors. Add `ConfigError::Network(#[from] NetworkError)`.
- `crates/ad-app/src/main.rs` -- `paint_network` reads `node.outgoing`. Use the accessor.
- Java reference for the kind rules: `src/main/java/AutoDriveEditor/RoadNetwork/RoadMap.java:60-100`.

## Tasks & Acceptance

**Execution:**
- [ ] `Cargo.toml`, `crates/ad-core/Cargo.toml`, `dependency_boundary.rs` -- add the `rstar` and `thiserror` dependencies and allowlist them.
- [ ] `crates/ad-core/src/network.rs` -- complete the API:
  - `NodeFlag::REGULAR`/`SUB_PRIO`.
  - `Node` with public `x, y, z, flag` and private adjacency behind `outgoing()`/`incoming()` slices.
  - `ConnectionShape`, `ConnectionKind`, `NetworkError`.
  - `from_loaded`, `add_node`, `move_node`, `set_flag`, `connect`, `disconnect`, `set_shape`, `connection_kind`.
  - Queries: `nearest`, `in_rect`, `in_polygon`, `within`, `edges_near`.
- [ ] `crates/ad-core/src/network.rs` (unit tests) -- one test per matrix row, plus every combination of the four membership bits × flags 0/1/7 for `connection_kind`, and a brute-force comparison of each query after random connects and moves.
- [ ] `crates/ad-io/src/config_xml.rs` -- switch to `from_loaded` and the accessors, add a `0,7` flags round-trip test.
- [ ] `crates/ad-app/src/main.rs` -- switch to the accessor.

**Acceptance Criteria:**
- Given the FS25 fixture, when it is loaded and saved, then the bytes are identical (the golden test).
- Given any public `Network` method, when it changes positions or adjacency, then a following spatial query reflects the change.
- Given the workspace, when the AD-1 boundary test runs, then `ad-core` depends only on `rstar` and `thiserror`.

## Implementation Notes

## Plan Change Log

## Review Triage Log

| # | Finding | Verdict | Route | Evidence |
|---|---------|---------|-------|----------|
| 1 | The `in_polygon` brute-force check reuses `point_in_polygon` | low | patch | The expectation uses the same predicate, so only the R-tree prefilter is cross-checked. The fix adds a fixed-shape test (square and concave). |
| 2 | Inclusive distance at exactly `d` is untested for `within`/`edges_near` | low | patch | The random tests offset points and radii to avoid ties. The fix adds an exact-distance test. |
| 3 | NaN or inf coordinates corrupt the R-trees | low | patch | Rust float parsing accepts `NaN`/`inf`. rstar bulk load does not panic, but `move_node` removes entries by value, and NaN never matches, so stale entries stay. The fix rejects non-finite x/y/z at load with `InvalidValue`. |
| 4 | `unusual_flag_value_roundtrips` has a raw line break in a string literal (orchestrator) | low | patch | The test works but is inconsistent with the other tests. The fix writes `"\n"`. |

## Design Notes

- **Shapes:**
  - OneWay(a,b) = a.out+b, b.in+a.
  - Reverse(a,b) = a.out+b only.
  - Dual = OneWay(a,b) + OneWay(b,a).
- **Operations:**
  - `connect` only adds missing entries. It never removes, so OneWay over Dual stays Dual.
  - `disconnect(a,b)` removes every a/b entry in both directions.
  - `set_shape` = `disconnect` + `connect`.
  - Appends keep `Vec` order; `contains` guards against duplicates.
- **Kind** (a.out∋b required, else `None`), with p = b.in∋a, q = b.out∋a, r = a.in∋b, checked in Java's order:
  - (p,q,¬r) → CrossedRegular
  - (¬p,q,r) → CrossedReverse
  - (p,q,r) → Dual
  - (¬p,¬q,¬r) → Reverse
  - (p,¬q,¬r) → SubPrio if flag == 1, Regular if flag == 0, else Unknown
  - anything else → Unknown
- **Indexes:**
  - Nodes: `RTree<GeomWithData<[f64;2], NodeId>>`.
  - Edges: `RTree<GeomWithData<Line<[f64;2]>, (NodeId, NodeId)>>`, with one entry per outgoing entry (Dual gives two).
  - `from_loaded` bulk-loads both trees.
  - `move_node` removes and reinserts the node point and every incident edge, using the old coordinates.
- **`from_loaded`:**
  - Signature: `from_loaded(Vec<LoadedNode { x, y, z, flag, outgoing: Vec<usize>, incoming: Vec<usize> }>)`, where the indexes point into the input.
  - It assigns ids in order and keeps lists verbatim.
  - An out-of-range index gives `Err`.
- **Polygon:** even-odd ray casting over the polygon's bounding-box candidates.
- **`edges_near(p, d)`:** `(from, to)` for each edge whose segment lies within `d` (inclusive).

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `dependency_boundary` and the new network tests
