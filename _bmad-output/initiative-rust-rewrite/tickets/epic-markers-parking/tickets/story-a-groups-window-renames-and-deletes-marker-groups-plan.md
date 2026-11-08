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
- [ ] `RenameGroup` -- command and tests
- [ ] window -- counts, rename, delete, refusals; tests for List, Refused, All, Read-only, modal blocking
- [ ] golden -- renamed group round-trip

**Acceptance Criteria:**
- Given the fixture, when the user opens Groups, renames one group and deletes another, then the markers change groups as shown and each step undoes in one Ctrl+Z.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
