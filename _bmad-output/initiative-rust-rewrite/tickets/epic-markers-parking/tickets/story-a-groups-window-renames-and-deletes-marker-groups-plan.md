---
title: 'A groups window renames and deletes marker groups'
type: 'feature'
ticket: '4'
created: '2026-11-08'
status: 'planned'
baseline_revision: '1acae57'
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

**Problem:** Marker groups can only be changed one marker at a time. AD-17 asks for a group rename as one command; Java has neither rename nor delete for a savegame config.

**Approach (new, no Java counterpart):**
- **Command `RenameGroup { from, to }`** (ad-core): sets the group of every marker in `from` to `to`, remembering which markers it changed, so revert restores exactly those; markers already in `to` are untouched (a rename onto an existing group merges). Names, order and parked vehicles never change.
- **Groups window** (toolbar button "Groups", modal like the marker dialog, so typing never fires shortcuts): one row per group, "All" first, the others sorted case-insensitively as in the marker dialog, each with its marker count.
  - **Rename** opens a text field in the row; OK (or Enter) commits one `RenameGroup` to the trimmed name. An empty name, the same name, or "None" is refused with a red note; "All" as the target is allowed and means delete.
  - **Delete** commits one `RenameGroup` to "All" (the markers stay, without a group).
  - "All" has neither button.
  - "Close" (or Escape) closes the window. The list updates after each commit; each commit undoes in one step after closing.
- Read-only: the button is disabled.

## Boundaries & Constraints

**Always:** one transaction per rename or delete through `Document::apply`; the window is closed by a document replacement; the dialog modal rules of stories 4.8/6.2 apply.

**Never:** no deletion of markers; no group order or group table (groups stay derived from markers, AD-17); no route-manager groups (epic-route-manager).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| List | markers in "b" ×2, "A", "All" ×3 | All (3), A (1), b (2) | none |
| Rename | "b" → "c" | both markers in "c"; one undo restores "b" | none |
| Merge | "b" → "A" | three markers in "A"; undo restores the two to "b", leaves the "A" one | none |
| Delete | delete "b" | both markers in "All", names kept; one undo restores | none |
| Refused | rename to "", "  ", "b", "None" | red note, OK disabled | refused in window |
| All | the "All" row | no rename or delete | none |
| Parked | a renamed marker with parked vehicles | vehicles kept | none |
| Read-only | legacy config | button disabled | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/command.rs` (+ `network/markers.rs` if a setter is needed) -- `RenameGroup` with revert data; tests for Rename, Merge, Delete-as-rename, Parked, exact round trip.
- `crates/ad-app/src/tools/marker.rs` -- pure `group_counts(net) -> Vec<(String, usize)>` (reuse `groups` ordering) and validation of a new name.
- `crates/ad-app/src/ui.rs`, `main.rs` -- the window state in `App`, the toolbar button, `dialog_shown()` includes it, closed on document replacement.
- `crates/ad-io/tests/golden_edited.rs` -- a renamed group saves and reloads.

## Tasks & Acceptance

**Execution:**
- [x] `RenameGroup` -- command and tests
- [x] window -- counts, rename, delete, refusals; tests for List, Refused, All, Read-only, modal blocking
- [x] golden -- renamed group round-trip

**Acceptance Criteria:**
- Given the fixture, when the user opens Groups, renames one group and deletes another, then the markers change groups as shown and each step undoes in one Ctrl+Z.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Command:** `RenameGroup::new(from, to)` keeps `moved: Vec<usize>`, the marker-order indexes of the markers in `from` at apply. Revert sets exactly those back to `from`, so a merge leaves the target group's own markers alone. A redo recomputes the list (same state, same result). Only the group string changes, through the new `Network::set_marker_group(index, group)`; names, order and `parked_vehicles` stay. A `from` without markers applies as a no-op; the app never plans one.
- **Pure logic (`tools::marker`):** `group_counts(net)` gives "All" first (also with count 0), then `groups(net)` order with counts. `GroupsWindow { rename: Option<(from, text)> }`. `start_rename` does nothing for "All". `validate()` gives the trimmed name or `GroupNote::{EmptyName, SameName, Reserved}`; the compares are exact, as in the marker dialog. `GroupNote` is a separate enum, so the marker dialog's `Note` gets no variants it never returns. `rename_plan` and `delete_group_plan` (rename to "All"; `None` for "All" or a group without markers) build one `RenameGroup` each. A rename to "All" plans the same transaction as a delete.
- **App:** `App.groups: Option<GroupsWindow>` is included in `dialog_shown()`, so shortcuts, map clicks and selection are blocked. It is cleared on a document replacement with the other dialogs. `open_groups()`, `finish_group_rename()` and `delete_group()` each check `editable()`; the window stays open after a commit and the rename field closes. The toolbar has a "Groups" button after the marker tools, under the same editable gate.
- **Window (`ui::groups_window`):** an `egui::Modal` with a scroll area of rows "name (count)" plus Rename/Delete (none for "All"). The row being renamed shows a focused text field, the count, OK (disabled while refused), Cancel, and the red note below. Enter commits when valid. Close, Escape or a click beside the window closes the whole window. While a rename is open, Escape closes the window too, not only the field.
- **Tests:** ad-core `command` -- `rename_group_moves_every_marker_of_the_group_and_reverts`, `rename_group_onto_an_existing_group_merges_and_undo_splits_exactly`, `rename_group_to_all_ungroups_and_keeps_names_and_parked_vehicles`, `rename_group_keeps_parked_vehicles_through_undo_and_redo`, `rename_of_a_group_without_markers_changes_nothing` (all through `round_trip`/`snap`). `tools::marker` -- `group_counts_list_all_first_then_the_groups_ignoring_case` (List), `a_rename_to_an_empty_or_the_same_name_or_none_is_refused` (Refused), `a_rename_plans_one_rename_group_to_the_trimmed_name_and_all_means_delete`, `delete_moves_the_group_to_all_keeping_the_markers_and_reverts`, `the_all_row_has_no_rename_or_delete` (All). `main` -- `the_groups_window_renames_and_deletes_each_in_one_undo_step` (acceptance: parked vehicle kept, two undos, redo), `a_refused_group_rename_commits_nothing_and_keeps_the_field`, `read_only_document_does_not_open_the_groups_window_or_change_groups` (Read-only), `shortcuts_and_clicks_do_nothing_while_the_groups_window_is_shown` (modal blocking), `a_new_document_closes_the_groups_window`. `ui` -- `the_groups_window_lists_counts_commits_on_enter_and_closes_on_escape` (painted rows "All (0)", "b (1)|Rename|Delete", the red note, Enter, Escape). Golden -- `renamed_and_deleted_groups_save_reload_and_undo`: renames the fixture's first group and deletes a second, saves to temp, reloads with the model equal and the outside bytes untouched, checks the counts, that names and order are kept, and that two undos restore the list.
- **Verification:** `cargo fmt --all --check` ok. `cargo clippy --workspace --all-targets -- -D warnings` ok. `cargo test --workspace` all pass: ad-app 346 (+2 ignored), ad-core 71, ad-io 44, golden_edited 9, golden_roundtrip 1, vehicles_roundtrip 6. `frame_time` (release): every case ok. The worst best-round max was 9.082 ms (drag, min zoom, west half selected); the worst single round was 12.354 ms (rotate tool idle), under 16 ms.

## Plan Change Log

## Review Triage Log
