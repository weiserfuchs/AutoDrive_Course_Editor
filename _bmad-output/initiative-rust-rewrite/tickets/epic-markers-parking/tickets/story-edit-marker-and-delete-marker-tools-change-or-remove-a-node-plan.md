---
title: 'Edit-marker and delete-marker tools change or remove a node''s marker'
type: 'feature'
ticket: '3'
created: '2026-11-08'
status: 'planned'
baseline_revision: 'a3a14d8'
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

**Problem:** Users cannot change or remove destinations. Java's `EditMarkerButton` and `DeleteMarkerButton` do it on a clicked node, without undo for the edit.

**Approach (as Java unless noted):**
- **Tool "Edit marker" (toolbar; E acts at once on the node under the pointer):** a click on a node with a marker opens story 6.2's marker dialog titled "Edit marker", prefilled with its name and group ("All" shows as "None"). The same rules apply: name trimmed, empty or used by another marker refused. OK commits one `EditMarker` with the new name and group; the marker keeps its place in marker order and its parked vehicles. OK without a change commits nothing.
- **Tool "Delete marker" (toolbar; Shift+D acts at once on the node under the pointer):** a click on a node with a marker removes it in one `EditMarker` (no confirmation, as Java). One undo puts it back at its place in marker order with its parked vehicles, so `vehicles.xml` references come back too (story 6.1).
- **On a node without a marker** either tool does nothing and shows "This node has no marker" in the notice line until the next click; E and Shift+D do nothing there.
- The selection is unaffected.

## Boundaries & Constraints

**Always:** one transaction per OK or delete through `Document::apply`; read-only disables both tools and keys (existing gates); dialog modal rules as in story 6.2; E and Shift+D do nothing during a drag or a drawn selection (as W).

**Never:** no group window (story 6.4); no change to parked vehicles other than keeping them; no confirmation dialog.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Edit | marker "Hof" in "A", edit to "Hof 2" in "B" | name and group changed, same marker index, parked vehicles kept; one undo restores | none |
| Prefill | marker in "All" | dialog shows name and "None" | none |
| Own name | OK keeping its own name, new group | accepted | none |
| Taken name | another marker's name | red note, OK disabled | refused in dialog |
| Unchanged | OK without edits | nothing committed | none |
| Delete | delete a marker with parked vehicles | marker gone, later markers move up; one undo restores index and parked vehicles | none |
| No marker | either tool on a plain node | nothing; notice shown | none |
| Keys | E / Shift+D over a marked node, plain node, empty map | dialog or delete / nothing / nothing | none |
| Read-only | legacy config | tools and keys disabled | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/marker.rs` -- `EditMarkerTool`, `DeleteMarkerTool`; the dialog in edit mode (prefill, "Edit marker" title, unchanged → none); `delete_plan(net, node)`.
- `crates/ad-app/src/tools/mod.rs` -- `ToolRequest::EditMarker(NodeId)`; delete commits directly from `on_click` or through a request, whichever fits `App::click` best.
- `crates/ad-app/src/input.rs`, `main.rs`, `ui.rs` -- E and Shift+D (exact modifiers), toolbar buttons, notices, gates.
- `crates/ad-io/tests/golden_edited.rs` -- an edited marker and a deleted marker save and reload; `vehicles_roundtrip.rs` already covers marker deletion for `vehicles.xml`.

## Tasks & Acceptance

**Execution:**
- [ ] tools -- edit and delete; tests for Edit, Prefill, Own name, Taken name, Unchanged, Delete
- [ ] wiring -- tests for No marker, Keys, Read-only
- [ ] golden -- edited and deleted marker round-trip

**Acceptance Criteria:**
- Given the fixture, when the user renames and regroups a marker with Edit marker, deletes another with Delete marker, and undoes both, then each change appears and disappears in one step and the deleted marker returns with its parked vehicles.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
