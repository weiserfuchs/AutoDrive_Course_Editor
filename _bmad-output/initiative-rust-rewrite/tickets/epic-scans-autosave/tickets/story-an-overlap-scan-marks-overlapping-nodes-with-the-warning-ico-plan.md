---
title: 'An overlap scan marks overlapping nodes with the warning icon'
type: 'feature'
ticket: '1'
created: '2026-11-08'
status: done
baseline_revision: '076a0a4'
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
- [x] `overlaps` -- tests for Pair, Height, Distance, Group
- [x] marks, following edits, drawing -- tests for Follow, Clear, Read-only, a frame-time case with the fixture's marks
- [x] dialog -- tests for Invalid; the Clean count recorded in the notes

**Acceptance Criteria:**
- Given a copy of the fixture with two nodes pasted onto existing ones (Shift+V), when the user runs "Find overlapping nodes…" with 0.1, then those nodes show the warning icon, and moving one away removes its mark.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Scan:** `ad_core::scan::overlaps(net, d)` (new `scan.rs`, pure): for each node, `within(x, z, d)` candidates kept when the 3D distance is at most `d` (inclusive).
- **Timing (release, fixture, 35 652 nodes):** a full scan costs about 8.4-9.4 ms at d = 0.1 and 9-12 ms at d = 1.0, over the ~2 ms budget, so marks are not rescanned after each edit. Instead `scan::placed_nodes(tx)` (the nodes a transaction adds, pastes, deletes or moves; an exhaustive match, so a new command must decide) and `scan::around` give the nodes within `d` of them before and after the apply/undo/redo, and `scan::remark` recomputes only those: about 1-2 µs for one moved node. Remarking around every node would cost about 43-50 ms, so an entry placing more than 64 nodes and more than an eighth of the network gets one full rescan instead (`around_or_rescan` in `document.rs`). A randomized ad-core test checks that the kept marks equal a full scan after every apply and revert.
- **Clean:** the fixture has **0** overlapping nodes at d = 0.1 (1 313 at d = 1.0). Because d = 0.1 marks nothing, the frame-time cases draw the d = 1.0 marks.
- **Marks and notice:** `Document.overlap: Option<(f64, BTreeSet<NodeId>)>`, set by `scan_overlaps`, cleared by `clear_overlaps`; a new document starts with none, so opening another file clears them. The notice line ("N overlapping nodes found" / "No overlapping nodes") is replaced after every rescan or follow, so it shows the current count; clearing drops it and keeps other notice lines. Nothing touches the history, so read-only configs scan and stay clean.
- **Drawing:** `ViewHooks::overlap()` (ToolHooks gets `overlap` from the document) hands the marks to `paint`, which draws `Icons::warning` (20 px, centred on the node at its preview position, culled to the view, one mesh) at every zoom, like the parking icon. As in Java `NodeDrawThread` 110-117, a marked node shows the overlap icon instead of the height-warning icon. The `dead_code` allow on `Icons::warning` is gone.
- **UI:** a "Scan" menu beside "Import" (enabled with a document): "Find overlapping nodes…" opens a modal (`App.overlap_dialog`, part of `dialog_shown`, closed when a new document loads) with a distance field prefilled "0.1". It reuses `edit_location::parse(text, 0.001, 100)` (comma read as the decimal point, rounded to three decimals) with its red notes; OK is disabled and Enter ignored while the text is refused. "Clear overlap marks" is enabled while marks are shown.
- **Tests:** ad-core `scan::tests` (pair, height, distance incl. inclusive bound, group, empty, marks kept around placed nodes equal a full scan); ad-app `document::tests` (follow a move with undo/redo and a delete, a new node marked, an edit placing many nodes rescans, clear, read-only); `ui::tests::the_overlap_dialog_refuses_an_invalid_distance_and_scans_a_valid_one`; `tests::nodes_pasted_in_place_are_marked_and_a_moved_one_loses_its_mark` (the acceptance criterion, with Copy and Paste in place), `tests::a_new_document_clears_the_overlap_marks_and_closes_the_scan_dialog`, `tests::a_read_only_document_can_be_scanned`; `view::tests::overlap_marks_draw_the_warning_icon_in_place_of_the_height_warning`; frame-time cases "static: min zoom, overlap marks (d = 1.0)" and "static: mid zoom, dense, overlap marks (d = 1.0)", plus printed scan and follow timings.
- **Verification:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass. `frame_time` (release): every case under 16 ms; overlap-mark cases best-round median/max 2.26/2.35 ms (min zoom) and 0.57/0.59 ms (mid zoom, dense), against 2.32/2.82 and 0.59/0.60 ms for the same views without marks; highest case max: rotate at min zoom, whole network, 12.5 ms.
- **Not verified:** the GUI by hand (dialogs need a human).

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator), pass 1: no patch.
  - checked: marks follow edits by rechecking only around placed nodes (µs per moved node), with one full rescan (about 9 ms) only for edits touching more than 64 nodes and an eighth of the network; a randomized test keeps the kept marks equal to a full scan.
  - accepted: the count notice follows every edit; a marked node shows the overlap icon instead of the height warning (as Java); the dialog always opens with 0.1.
- Verification (orchestrator re-run): fmt, clippy and workspace tests pass (ad-app 395).
- Hand check (user), pass 1: after copying a large group and pasting it in place, d = 0.1 marked only 4 nodes. The saved config had 659 nodes on an original's x/z, but their heights (from the heightmap at paste) were 0.38 m off on average, 1.65 m at most, so the 3D distance missed them. User decision: the distance applies in (x, z), with a separate height tolerance of 2 m (`scan::HEIGHT_TOLERANCE`); two levels of a bridge stay apart. Test `height_counts_only_beyond_the_tolerance`; AD-13 updated. The fixture still has 0 overlaps at 0.1 (1315 at 1.0). Verified: fmt, clippy, workspace tests, frame time.
