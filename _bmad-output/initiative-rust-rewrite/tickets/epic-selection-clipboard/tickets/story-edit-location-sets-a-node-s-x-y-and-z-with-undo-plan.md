---
title: 'Edit Location sets a node''s x, y and z with undo'
type: 'feature'
ticket: '8'
created: '2026-11-08'
status: 'planned'
baseline_revision: '22d1172'
route: 'full'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Users cannot type a node's exact position. Java's Edit Location tool (`EditLocationButton`) opens a dialog for x, y and z but writes the node directly, so it cannot be undone (AD-13).

**Approach:** An Edit Location tool (toolbar only, no shortcut, as in Java). A primary click on a node opens a modal dialog with three text fields x, y, z, prefilled with the node's position at 3 decimals. Each field is parsed live; an invalid field shows a red note under it and disables OK. Valid ranges as in Java: x and z within the map (±1024 × map scale), y from 0 to 1024 × map scale. OK (or Enter) rounds each value half-up to 3 decimals and commits one `MoveNodes` with the absolute before and after position; if nothing changed, nothing is committed. Cancel (or Escape, or closing the dialog) commits nothing. A comma is accepted as the decimal separator. The selection is unaffected.

## Boundaries & Constraints

**Always:** one transaction per OK through `Document::apply`; read-only disables the tool (existing gates); a click on empty map does nothing; while the dialog is open, map input and shortcuts do not act; if the node is gone when OK is pressed (cannot happen while modal, but undo is blocked anyway), nothing is committed.

**Never:** no editing of several nodes at once; no flag or marker editing; no heightmap snap of the typed y; no new command.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Edit | click node, type x 12.3456, OK | x = 12.346 (half-up), y, z kept; one MoveNodes; one undo restores | none |
| Cancel | change fields, Cancel / Escape / close | nothing committed | none |
| Unchanged | OK without edits | nothing committed | none |
| Invalid text | "abc", empty, "1e999", NaN | red note, OK disabled | refused in dialog |
| Out of range | x beyond ±1024·scale, y < 0 | red note, OK disabled | refused in dialog |
| Comma | "12,5" | accepted as 12.5 | none |
| Empty click | click off a node | no dialog | none |
| Read-only | legacy config | tool disabled | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/edit_location.rs` (new) -- `EditLocationTool`; pure `parse(text, min, max) -> Result<f64, Note>` and `plan(net, node, [x, y, z]) -> Option<Transaction>`; `on_click` on `ctx.hovered_node` asks the app to open the dialog (e.g. via a pending-request field the app takes after the click). Name "Edit location".
- `crates/ad-app/src/ui.rs` -- the dialog as an `egui::Modal` (like the unsaved prompt), its state (node, three strings) in `App`; toolbar button.
- `crates/ad-app/src/tools/mod.rs`, `main.rs` -- wiring as the other tools; tool and dialog gated on `editable()`; shortcuts skipped while the dialog is open.
- `crates/ad-io/tests/golden_edited.rs` -- one node set to typed values (3 decimals) round-trips.
- Do not touch: view, selection input, `ad-core`.

## Tasks & Acceptance

**Execution:**
- [ ] `tools/edit_location.rs` -- parse and plan; tests for Edit (rounding), Unchanged, Invalid text, Out of range, Comma
- [ ] dialog and wiring -- tests: a click opens the dialog for the hovered node, a click off a node does not, OK commits and one undo restores, Cancel commits nothing, read-only does nothing, a shortcut pressed while the dialog is open does nothing
- [ ] `golden_edited.rs` -- edited node round-trip

**Acceptance Criteria:**
- Given the fixture, when the user picks Edit location, clicks a node, types new x, y and z and presses OK, then the node moves there and one Ctrl+Z restores it.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
