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
- [x] tools -- edit and delete; tests for Edit, Prefill, Own name, Taken name, Unchanged, Delete
- [x] wiring -- tests for No marker, Keys, Read-only
- [x] golden -- edited and deleted marker round-trip

**Acceptance Criteria:**
- Given the fixture, when the user renames and regroups a marker with Edit marker, deletes another with Delete marker, and undoes both, then each change appears and disappears in one step and the deleted marker returns with its parked vehicles.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Dialog edit mode:** `MarkerDialog` gained `editing: Option<(name, group)>`, set by `MarkerDialog::edit(net, node)` (prefill; "All" shows as "None"; `None` for a node without a marker). `title()` gives "Add marker" / "Edit marker" for the modal heading. `plan` returns `None` when the validated (trimmed, "None"/empty → "All") result equals the original, so "Unchanged" commits nothing even after only trimming or picking "None" for "All". Validation is shared with add: a name is taken only by a marker on another node, so the own name passes.
- **Delete:** `delete_plan(net, node)` builds the one `EditMarker::new(node, None)`. `DeleteMarkerTool::on_click` returns it directly (so `App::click` commits it through the existing `doc.apply` path); on a plain node it leaves `ToolRequest::NoMarker(node)` for the notice. `EditMarkerTool` asks with `ToolRequest::EditMarker(node)`; `App::click` opens `MarkerDialog::edit` or shows `NO_MARKER` ("This node has no marker"), cleared by the next click as before.
- **Keys:** E (`Action::EditMarker`) and Shift+D (`Action::DeleteMarker`), exact modifiers; neither was in the table (Shift+Delete is the eframe Cut alias, unrelated). W, E and Shift+D share `App::marker_target()` (the old `add_marker_hovered` hover/epoch/drag/selecting checks), so all three do nothing during a drag or a drawn selection, over empty map, or on a read-only document (`editable()` gate). Keys show no notice, as W.
- **Toolbar:** "Edit marker" and "Delete marker" after "Add marker", under the existing editable gate. Neither key nor tool touches the selection or cancels the active tool (markers do not affect previews).
- **Tests:** `tools::marker` -- `edit_replaces_name_and_group_in_place_keeping_parked_vehicles_and_reverts`, `edit_prefills_name_and_group_with_all_as_none`, `edit_keeping_its_own_name_with_a_new_group_is_accepted`, `edit_to_another_markers_name_is_refused`, `edit_without_a_change_plans_nothing`, `delete_removes_the_marker_and_one_revert_restores_its_place_and_vehicles`, `edit_and_delete_marker_clicks_act_only_on_a_node`. `main` -- `edit_marker_ok_commits_one_edit_and_one_undo_restores_it`, `delete_marker_removes_it_in_one_step_and_undo_brings_back_its_vehicles`, `edit_or_delete_marker_on_a_plain_node_shows_the_notice_until_the_next_click`, `e_and_shift_d_act_on_a_hovered_marked_node_whichever_tool_is_active`, `read_only_document_ignores_the_edit_and_delete_marker_tools_and_keys`, `e_and_shift_d_do_nothing_during_a_drawn_selection`. `input` -- E / Shift+D in `table_maps_the_java_defaults`, extra-modifier cases in `extra_modifiers_do_not_fire_n_or_f4`. Golden -- `edited_and_deleted_markers_save_reload_and_undo` (fixture: rename + regroup marker 0, delete marker 1 with a parked vehicle, save to temp, reload model equal, later markers move up, two undos restore the list with the vehicle).
- **Verification:** `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` all pass (ad-app 334, golden_edited 8, golden_roundtrip and vehicles_roundtrip pass). `frame_time` (release): every case ok, worst best-round max 9.201 ms (rotate, min zoom, whole network selected); worst single round 14.390 ms, under 16 ms.
- **Open point:** if the fixture or a file has two markers with the same name, editing one of them only to change its group is refused as "Another marker has this name" (frozen rule: "used by another marker refused"); Java's edit dialog checks no names and would accept it.

## Plan Change Log

## Review Triage Log
