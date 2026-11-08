---
title: 'An overlap scan marks overlapping nodes with the warning icon'
type: 'feature'
ticket: '1'
created: '2026-11-08'
status: 'planned'
baseline_revision: '076a0a4'
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

**Problem:** Overlapping nodes (two nodes in the same place, often from a careless paste or an old import) break routes in the game and are invisible in the editor. Java's "Scan network" marks them, but ignores the distance the user types (`ScanManager.scanNetworkForOverlapNodes` always uses 0.1).

**Approach:**
- **Scan menu** in the top bar (an egui menu button "Scan", beside "Import"), item "Find overlapping nodes…": a modal asks for the distance (default 0.1, accepted 0.001 to 100, comma accepted; invalid shows a red note and disables OK).
- **The scan:** for every node, the R-tree's `within(x, z, d)` gives candidates, kept when the 3D distance (with y) is at most `d`. Every node that has at least one such partner is marked. Pure function in ad-core (no I/O, no UI): `overlaps(net, d) -> BTreeSet<NodeId>`.
- **Marks:** the document holds `overlap: Option<(f64, BTreeSet<NodeId>)>` (distance and marked nodes). Marked nodes draw the warning icon (`Icons::warning`, prepared in story 2.5), centred on the node as the parking icon is, at every zoom where icons draw. A notice says "N overlapping nodes found" or "No overlapping nodes".
- **Following edits:** while marks exist, they are recomputed with the same distance after every apply, undo and redo, so a moved, merged or deleted node loses its mark at once.
- **"Clear overlap marks"** in the Scan menu removes them. Opening another file clears them.
- The scan changes no data and works on read-only configs too.

## Boundaries & Constraints

**Always:** the scan of the fixture runs within a frame budget-friendly time (measure it; if it exceeds a few ms on edits, recompute only around the changed nodes); the frame-time test passes with marks shown.

**Never:** no merging (story 8.2); no change to the network or history.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Pair | two nodes 0.05 apart, d = 0.1 | both marked; notice "2 overlapping nodes found" | none |
| Height | same x/z, y 0 and 5, d = 0.1 | not marked | none |
| Distance | nodes 0.5 apart, d = 0.1 / 1.0 | unmarked / marked | none |
| Group | three nodes within d of each other | all three marked | none |
| Follow | move one of a pair away; undo | marks gone; back after undo | none |
| Clean | the fixture, d = 0.1 | count as found (record it) | none |
| Invalid | "abc", 0, 1000 | red note, OK disabled | refused in dialog |
| Clear | Clear overlap marks | icons gone | none |
| Read-only | legacy config | scan works | none |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/network/spatial.rs` (or a new `scan.rs` in ad-core) -- `overlaps(net, d)`; tests.
- `crates/ad-app/src/document.rs` -- the marks and their recompute after apply/undo/redo.
- `crates/ad-app/src/view.rs`, `icons.rs` -- draw the warning icon for marked nodes (remove the `dead_code` allow); the frame-time test with marks.
- `crates/ad-app/src/ui.rs`, `main.rs` -- the Scan menu, the distance dialog (modal rules as the other dialogs), notices.

## Tasks & Acceptance

**Execution:**
- [ ] `overlaps` -- tests for Pair, Height, Distance, Group
- [ ] marks, following edits, drawing -- tests for Follow, Clear, Read-only, a frame-time case with the fixture's marks
- [ ] dialog -- tests for Invalid; the Clean count recorded in the notes

**Acceptance Criteria:**
- Given a copy of the fixture with two nodes pasted onto existing ones (Shift+V), when the user runs "Find overlapping nodes…" with 0.1, then those nodes show the warning icon, and moving one away removes its mark.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
