---
title: 'The tool bar is grouped and shows each tool''s shortcut'
type: 'feature'
ticket: '10'
created: '2026-11-09'
status: done
baseline_revision: 'acf1e57'
route: 'full'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:**
- The tool bar is one flat row of text buttons, and most have no tooltip.
- Undo and Redo show fixed key texts that ignore rebound keys (story 5).
- There are no Cut, Copy and Paste buttons, which Java's action bar has.

**Approach:**
- **Groups:** the tools are grouped as in Java, with separators:
  - nodes (Node, Sub-prio node, Move, Rotate, Delete, Priority, Flip, Edit location);
  - connections;
  - curves;
  - markers, with Groups;
  - alignment;
  - hide.
- **Rest of the bar:** then the toggles (Continuous, Snap, Rect/Lasso), then Undo, Redo, Cut, Copy and Paste.
- **Tooltips:** each button's tooltip says in a few words what it does, followed by its current key from the bindings in use (story 5). An action without a key shows the hint alone.
- **Enabled when:** Cut and Copy need a selection, Paste the app clipboard; tools and edits need an editable document (AD-11).
- **No icons:** the buttons stay text buttons.
- The user started this story while away; the plan was not approved separately, and the hand check is pending.

## Boundaries & Constraints

**Never:** no change to what a tool does; no collapsible groups.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Hints | every tool of the bar | a non-empty hint | none |
| Key | Move tool | "Drag nodes or the selection (M)" | none |
| Rebound | `move_tool = "Shift+M"` / `""` | the tooltip names Shift+M / no key | none |
| Narrow | 800 px window | menus, groups, Hide connection and the edit buttons inside | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/ui.rs` -- `tool_hint`, `hint_text`, `action_button`, the grouped `toolbar`; tests.

## Tasks & Acceptance

**Execution:**
- [x] groups, hints and edit buttons -- tests Hints, Key, Rebound, Narrow

**Acceptance Criteria:**
- Given the tool bar, when the user hovers a tool, then its tooltip says what it does and names its current key.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Icons:** Java's icons are not ported; the inventory lists them under tool-bar presentation, and egui text buttons keep the bar readable at any width.
- **Marker tools:** their keys (W, E, Shift+D) act on the hovered node whatever tool is active; the tooltip names them because Java did.
- **Verification:** fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 456 + 4 ignored); frame time ok.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: Cut, Copy and Paste keys come from the shortcut table, so no tooltip names a key twice.
