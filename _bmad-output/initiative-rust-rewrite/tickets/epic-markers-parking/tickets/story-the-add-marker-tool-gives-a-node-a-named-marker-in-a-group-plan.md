---
title: 'The add-marker tool gives a node a named marker in a group'
type: 'feature'
ticket: '2'
created: '2026-11-08'
status: done
baseline_revision: '72e3038'
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

**Problem:** Users cannot add destinations. Java's `AddMarkerButton` opens a dialog for a name and a group on a clicked node.

**Approach (as Java unless noted):**
- **Tool "Add marker" (toolbar):** a primary click on a node without a marker opens the marker dialog for it. **W** opens the dialog at once for the node under the pointer, whichever tool is active (Java's shortcut action); no node, or a node with a marker, does nothing for W.
- **On a node with a marker** the tool opens nothing and shows "This node already has a marker" in the toolbar's notice line until the next click (Java's info text).
- **Marker dialog** (modal, like Edit location; story 6.3 reuses it for editing): a name field and a group field. The group field is a text field with a drop-down of the existing groups (from the network's markers, without "All"), sorted case-insensitively, plus "None" first; picking one fills the field. "None" or an empty group means "All". The name is trimmed. An empty name, or one another marker already uses (exact match), shows a red note and disables OK (AD-13). OK or Enter commits; Cancel, Escape or a click beside it commits nothing.
- **Commit:** one `EditMarker` setting the marker (appended to the marker order, AD-17), no parked vehicles. One undo removes it. The selection is unaffected.
- While the dialog is open, map input and shortcuts do nothing (the existing modal rule).

## Boundaries & Constraints

**Always:** one transaction per OK through `Document::apply`; read-only disables the tool and W (existing gates); a document replacement closes the dialog.

**Never:** no editing or deleting of markers (story 6.3); no group window (story 6.4); no marker icons beyond today's drawing.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Add | click a node, name "Hof", group "Felder" (existing) | marker "Hof" in "Felder", appended; one undo removes it | none |
| New group | group "Neu" typed | marker in group "Neu" | none |
| None | group "None" or empty | group "All" | none |
| Groups list | markers in "b", "A", "All", "a2" | list: None, A, a2, b | none |
| Empty / duplicate | name "  " or an existing name | red note, OK disabled | refused in dialog |
| Has marker | click a node with a marker | no dialog; notice shown | none |
| W | W over a node / over empty map / over a marked node | dialog / nothing / nothing | none |
| Cancel | Cancel, Escape, click beside | nothing committed | none |
| Read-only | legacy config | tool and W disabled | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/marker.rs` (new) -- `AddMarkerTool` (click asks the app for the dialog, like `EditLocationTool`'s request); pure `groups(net) -> Vec<String>` (sorted, without "All"), `MarkerDialog { node, name, group }` with `validate(net) -> Result<(String, String), Note>` and `plan(...) -> Transaction`.
- `crates/ad-app/src/ui.rs` -- `marker_dialog` modal (text field + drop-down), toolbar button, the notice line.
- `crates/ad-app/src/input.rs`, `main.rs` -- W (exact modifiers) → open the dialog for the hovered node; gates; modal blocking as for Edit location; dialog closed on document replacement.
- `crates/ad-io/tests/golden_edited.rs` -- an added marker in a new group saves and reloads (if not already covered by an existing `EditMarker` case, extend it).

## Tasks & Acceptance

**Execution:**
- [x] `tools/marker.rs` -- groups, validation, plan; tests for Add, New group, None, Groups list, Empty / duplicate
- [x] dialog and wiring -- tests for Has marker, W, Cancel, Read-only, modal blocking
- [x] golden -- added marker round-trip

**Acceptance Criteria:**
- Given the fixture, when the user picks Add marker, clicks a node, types a name and picks a group, then the marker appears with that name and one Ctrl+Z removes it.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Tool request:** `Tool::take_location_request` became `take_request() -> Option<ToolRequest>` with `ToolRequest::{EditLocation(NodeId), AddMarker(NodeId)}`; story 6.3 adds its edit request there. `AddMarkerTool` asks for every clicked node; `App::click` decides: a marked node sets the notice, an unmarked one opens `MarkerDialog`.
- **Notice:** `App::tool_notice` (a `&'static str`), shown in the top bar's notice line after the load notice; cleared at the start of the next map click and on document replacement.
- **Dialog:** `MarkerDialog { node, name, group, choices }`; `choices` ("None" plus `groups(net)`) is taken at open, since the network cannot change while the modal is shown. It opens with group "None". `validate` trims the name and refuses it empty or used by another node's marker (exact match; the dialog's own node is excluded, for 6.3). The group is trimmed too, so a whitespace-only group means "All" like an empty one (the spec only names trimming for the name). `groups` sorts by lowercase, then by exact text, and drops duplicates; Java's locale collator also ignores accents, which this does not.
- **UI:** `ui::marker_dialog` is an `egui::Modal` like Edit location: name field with the red note, group text field with a "Groups" `ComboBox` beside it whose entries fill the field; OK disabled while the name is refused.
- **Modal rule:** `App::dialog_shown()` (location or marker) replaces `location.is_some()` in `click`, `map_panel` and `handle_shortcuts`.
- **W:** `Action::AddMarker`, `Key::W` with exact no modifiers, gated by `editable()`. It uses the hover of the document the view last showed (as Delete). It does nothing while a tool-claimed drag (new `MapView::dragging()`) or a selection drag is in progress, so the modal never cuts a drag whose end the tool would then miss; it does not cancel the active tool.
- **Tests:** `tools::marker::tests` (add_appends_the_marker_in_one_edit_marker_and_reverts, a_typed_group_is_kept_as_a_new_group, none_or_an_empty_group_means_all, groups_are_sorted_ignoring_case_without_all_after_none, an_empty_or_taken_name_is_refused, no_dialog_for_a_marked_or_missing_node, a_click_asks_for_the_dialog_only_on_a_node); `main.rs` (add_marker_ok_commits_one_marker_and_one_undo_removes_it, add_marker_on_a_marked_node_shows_the_notice_until_the_next_click, add_marker_cancel_or_a_refused_name_commits_nothing, w_opens_the_marker_dialog_for_a_hovered_unmarked_node_whichever_tool_is_active, read_only_document_ignores_the_add_marker_tool_and_w, shortcuts_and_clicks_do_nothing_while_the_marker_dialog_is_shown, a_new_document_closes_the_marker_dialog_and_the_notice); `ui.rs` the_marker_dialog_commits_on_enter_and_escape_or_a_click_beside_cancels; `input.rs` W in the table and Shift/Ctrl/Alt+W ignored; `golden_edited.rs` added_marker_in_a_new_group_saves_and_reloads.
- **Verification:** `cargo fmt --all --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test --workspace` all pass (ad-app 321, golden_edited 7, others unchanged). `frame_time` (release): every case ok, worst best-round max 9.616 ms (rotate, min zoom, whole network selected); min zoom static 2.280 ms, drag whole network 8.451 ms.
- **Not checked:** the dialog in the running app (needs a human): combo popup placement inside the modal, and whether Escape with the combo popup open closes the whole dialog.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator), pass 1: no patch.
  - accepted: the group is trimmed too; a name counts as taken only on another node (story 6.3 keeps a marker's own name); W does nothing during a drag or a drawn selection.
  - for the hand check: where the group drop-down opens and whether Escape with it open closes the dialog.
- Verification (orchestrator re-run): fmt, clippy and workspace tests pass (ad-app 321, golden_edited 7).
