---
title: 'The add-marker tool gives a node a named marker in a group'
type: 'feature'
ticket: '2'
created: '2026-11-08'
status: 'planned'
baseline_revision: '72e3038'
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
- [ ] `tools/marker.rs` -- groups, validation, plan; tests for Add, New group, None, Groups list, Empty / duplicate
- [ ] dialog and wiring -- tests for Has marker, W, Cancel, Read-only, modal blocking
- [ ] golden -- added marker round-trip

**Acceptance Criteria:**
- Given the fixture, when the user picks Add marker, clicks a node, types a name and picks a group, then the marker appears with that name and one Ctrl+Z removes it.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
