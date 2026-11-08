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
- [ ] groups and merge plan in ad-core -- tests for the network rows
- [ ] backup job and app flow -- tests for Backup, Backup fails, Changed meanwhile, Read-only, selection and marks after the merge
- [ ] golden -- merged network round trip and undo

**Acceptance Criteria:**
- Given the user's hand-test config with a pasted copy over the original, when the user scans at 0.1 and merges, then the backup file appears, the copies are gone with every original link intact, no marks remain, and one Ctrl+Z brings the copies back.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
