---
title: 'Merging overlaps writes a backup, then merges in one undoable step'
type: 'feature'
ticket: '2'
created: '2026-11-08'
status: 'planned'
baseline_revision: '3a64ade'
route: 'full'
route_source: 'auto'
risk: 'high'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Overlapping nodes found by the scan (story 8.1) must be merged by hand. Java's "Merge nodes" (`ScanManager.mergeOverlappingNodes`) writes `<config>_mergeBackup.xml`, then rewires and deletes the overlaps, without undo.

**Approach:**
- **Scan menu "Merge overlapping nodes…"**, enabled while overlap marks exist and the document is editable. A confirm modal says "Merge N marked nodes into M nodes? A backup is written first."
- **Groups:** the marked nodes form groups: nodes overlapping each other (the scan's rule: within the scan distance in x/z and 2 m in height), joined transitively. Each group keeps one node: the one with a marker if exactly one has a marker, else the first in network order. If several nodes of a group have markers, the first marked one is kept, and a notice names the markers removed with the other nodes.
- **Links:** every link of a removed node to a node outside its group is recreated on the kept node with the same direction and shape (one-way, reverse, dual; by the incoming/outgoing entries as the clipboard reads them), unless the kept node already has that link. Links inside a group are dropped (no self-links). Then the removed nodes are deleted.
- **One transaction** built from existing commands (`Connect` adds only; `DeleteNodes` removes the merged nodes with their links and markers); one undo restores every node, link, marker and parked vehicle. No new command type unless one proves necessary.
- **Backup first (AD-16):** a background job writes `<config>_mergeBackup.xml` beside the config from a snapshot of the current state (unsaved edits included), atomically, without touching path, saved state or `vehicles.xml`. Only after it succeeds is the merge applied, and only if the document and its revision are unchanged; otherwise nothing is applied and a notice says why. A failed backup aborts the merge with the reason.
- After the merge the marks are recomputed as after any edit (the kept nodes no longer overlap); the merged nodes become the selection.

## Boundaries & Constraints

**Always:** read-only disables the item (existing gates); the merge never runs without a written backup.

**Never:** no move of the kept node (its position stays); no merge of unmarked nodes; no marker moved between nodes.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Pasted copy | a chain pasted in place (copy linked a→b→c over original A→B→C, plus A linked to an outside X) | A, B, C kept with their links; a, b, c gone; no duplicate links; X untouched | none |
| Shapes | removed node with one-way in, one-way out, reverse and dual links to outside nodes | each recreated on the kept node with its direction and shape | none |
| Inner links | links between members of one group | dropped, no self-links | none |
| Markers | one member has a marker / two do | that node kept / first kept, notice names the other | none |
| Group of three | three nodes overlapping transitively | one kept, two removed | none |
| Backup | merge on a document with unsaved edits | `<config>_mergeBackup.xml` holds the state before the merge, edits included; path and saved state unchanged | none |
| Backup fails | target folder read-only | no merge, error with the reason | error |
| Changed meanwhile | an edit lands between backup start and result | merge not applied, notice | notice |
| Undo | Ctrl+Z after the merge | the network exactly as before (golden check) | none |
| Read-only | legacy config | item disabled | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/scan.rs` -- `overlap_groups(net, distance, marks) -> Vec<Group { keep, removed }>` and `merge_plan(net, groups) -> (Transaction, removed_markers)`; tests for Pasted copy, Shapes, Inner links, Markers, Group of three, exact revert.
- `crates/ad-app/src/jobs.rs` -- the backup job (`config_xml::save_to` to the backup path, beside the config); `main.rs`, `ui.rs` -- the menu item, confirm modal, apply on the backup result with epoch and revision checks, notices, selection.
- `crates/ad-io/tests/golden_edited.rs` -- a merged network saves and reloads; undo returns the original model.

## Tasks & Acceptance

**Execution:**
- [x] groups and merge plan in ad-core -- tests for the network rows
- [x] backup job and app flow -- tests for Backup, Backup fails, Changed meanwhile, Read-only, selection and marks after the merge
- [x] golden -- merged network round trip and undo

**Acceptance Criteria:**
- Given the user's hand-test config with a pasted copy over the original, when the user scans at 0.1 and merges, then the backup file appears, the copies are gone with every original link intact, no marks remain, and one Ctrl+Z brings the copies back.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Groups** (`scan::overlap_groups`): union-find over the marked nodes, joined by the scan's rule (R-tree `within` the distance in x/z, `HEIGHT_TOLERANCE` in height); only marked nodes join, so an unmarked node next to a group stays. Keep = first member in network order with a marker, else the first member. Groups come in network order; a lone marked node is a group with nothing removed (counted in the modal's M).
- **Plan** (`scan::merge_plan`): one pass over every node's `out` list (a reverse link into a removed node is listed only at its source). For each link with a removed end, both ends map to their group's kept node; same kept node = inner link, dropped. The shape comes from the new `Network::link_shape`, which is the clipboard's rule moved into ad-core (clipboard now calls it); duals are taken once (lower id). A `Connect` is planned only when not all of its entries exist already and not planned before. Then one `DeleteNodes` of all removed nodes in network order, which takes their links and markers. No new command type. Returns the names of the deleted markers for the notice.
- **App flow:** Scan menu "Merge overlapping nodes…" is enabled by `App::can_merge` (editable, marks non-empty, no backup running). The modal shows "Merge N marked nodes into M nodes? A backup is written first." plus the backup path; Merge / Cancel / Escape. Merge starts `jobs::spawn_merge_backup` (a `config.clone()` snapshot to `<dir>/<stem>_mergeBackup.xml` through `config_xml::save_to`, atomic; history, path and `vehicles.xml` untouched) with `MergeStart { epoch, revision, overlap }`. On the result: a failed write shows "Merge cancelled: cannot write the backup <path>: <reason>"; a replaced document drops it; a changed revision or changed/cleared marks adds the notice "Merge not applied: the document changed while the backup was written"; otherwise `Document::merge_overlaps` recomputes groups and plan and applies them with `apply_selecting` (kept nodes selected; undo restores the selection before). Marks follow as after any edit; a notice "Markers deleted with merged nodes: …" names removed markers. The top bar shows "Writing merge backup…" while it runs.
- **Revision caveat:** `revision` is also bumped by a selection gesture, so a click on the map while the backup is written cancels the merge (with the notice). The backup takes well under a second, so this was left as the spec's rule.
- **Timing** (release, fixture 35652 nodes, first 650 nodes pasted in place = 1300 marks, best of 20): groups + plan 2.4 ms, apply 6.4 ms, a full rescan after 7.8 ms (the document's follow uses `around` instead, as 650 < N/8, so this is an upper bound). About 17 ms worst case, one frame: the plan stays on the UI thread; only the backup runs in the background.
- **Tests:** ad-core `scan::tests::{a_chain_pasted_in_place_merges_into_the_original_without_new_links, every_link_shape_is_recreated_on_the_kept_node_and_reverts_exactly, links_inside_a_group_are_dropped_without_self_links, the_node_with_the_marker_is_kept_and_of_two_the_first, three_nodes_overlapping_transitively_are_one_group, links_between_two_groups_join_their_kept_nodes_and_unmarked_nodes_stay, an_empty_marking_plans_nothing}` (all through `round_trip`, which became `pub(crate)`; the shapes case has markers with parked vehicles); ad-app `tests::{a_merge_writes_the_backup_then_merges_selects_the_kept_nodes_and_undoes_in_one_step, a_failed_backup_merges_nothing_and_names_the_reason, an_edit_while_the_backup_is_written_cancels_the_merge_with_a_notice, cancel_merges_nothing_and_writes_no_backup, a_read_only_document_or_no_marks_disable_the_merge, backup_path_is_beside_the_config_with_the_merge_suffix}` (temp dirs only); golden `merged_pasted_copy_saves_and_reloads_as_the_original_and_undo_brings_it_back` (650 fixture nodes pasted in place, merged: model equals the original, saves and reloads, bytes outside the modelled blocks unchanged, one undo returns the pasted model). "Backup fails" is simulated with a deleted target folder, as a read-only folder attribute does not stop file creation on Windows.
- **Verification:** `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` all pass (ad-app 401 + 3 ignored, ad-core 84 + 4, ad-io 76, golden_edited 10, golden_roundtrip 1, vehicles_roundtrip 6); `frame_time` every case ok, best-round max between 0.08 ms and 9.6 ms (drag of the whole network at min zoom). The acceptance check in the GUI (hand-test config, scan at 0.1, merge, Ctrl+Z) needs the user.

## Plan Change Log

## Review Triage Log
