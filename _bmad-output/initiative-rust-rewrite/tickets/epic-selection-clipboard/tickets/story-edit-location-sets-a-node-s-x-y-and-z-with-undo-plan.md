---
title: 'Edit Location sets a node''s x, y and z with undo'
type: 'feature'
ticket: '8'
created: '2026-11-08'
status: done
baseline_revision: '22d1172'
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
- [x] `tools/edit_location.rs` -- parse and plan; tests for Edit (rounding), Unchanged, Invalid text, Out of range, Comma
- [x] dialog and wiring -- tests: a click opens the dialog for the hovered node, a click off a node does not, OK commits and one undo restores, Cancel commits nothing, read-only does nothing, a shortcut pressed while the dialog is open does nothing
- [x] `golden_edited.rs` -- edited node round-trip

**Acceptance Criteria:**
- Given the fixture, when the user picks Edit location, clicks a node, types new x, y and z and presses OK, then the node moves there and one Ctrl+Z restores it.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- Opening the dialog: `Tool` got one default method, `take_location_request() -> Option<NodeId>` (always `None`). `EditLocationTool::on_click` stores `ctx.hovered_node` and returns no transaction; `App::click` takes the request right after `on_click` and opens `LocationDialog` (node id and the three field strings) in `App::location`. `App::finish_location(ok)` closes it and, with OK, commits the one `MoveNodes` through `Document::apply`.
- Rounding: half-up (away from zero on a tie) to 3 decimals on the decimal text, not on the binary value. The parsed f64 is printed with Rust's shortest round-trip `Display` (never an exponent), the first three decimals plus the fourth digit decide, and the result is `milli / 1000.0` (exact operands, so the nearest f64). A typed `0.0005` or `1.0005` therefore gives `0.001` / `1.001`, where Java's `new BigDecimal(double)` sees the binary value just below the tie and would round down. A tiny negative that rounds to 0 gives `+0.0`, not `-0.000`. The range is checked on the unrounded value; the bounds are whole numbers, so rounding cannot leave the range.
- Parsing: trimmed, commas read as the decimal point, `str::parse::<f64>`; empty, non-numeric and non-finite (`1e999`, `NaN`, `inf`) give "Not a number", out of range gives "Must be between min and max". Ranges x, z within ±1024·scale, y 0..=1024·scale (`MapContext.geometry.scale`).
- Dialog: `ui::location_dialog`, an `egui::Modal` drawn before the unsaved prompt (so a prompt from a window close stays on top). OK and Enter commit only with every field valid; Cancel, Escape and a backdrop click (`ModalResponse::should_close`) commit nothing.
- Input blocking while the dialog is shown: `input::handle_shortcuts` returns early (the unsaved-prompt check moved in there too), so no shortcut fires, Ctrl+Z / Ctrl+Shift+Z included, and no key is consumed; `map_panel` hands the view no tool and drops selection gestures; `App::click` ignores clicks. The modal backdrop covers the toolbar in the GUI. A new document (finished load) closes the dialog with the tool.
- Tests: `tools::edit_location::tests` (`values_round_half_up_to_three_decimals`, `a_comma_is_read_as_the_decimal_point`, `invalid_text_is_refused`, `values_out_of_range_are_refused`, `the_dialog_prefills_three_decimals_and_parses_each_field_against_its_range`, `an_edit_moves_the_node_in_one_move_nodes_and_reverts`, `an_unchanged_position_or_a_missing_node_commits_nothing`, `a_click_asks_for_the_dialog_only_on_a_node`); main.rs (`an_edit_location_click_opens_the_dialog_for_the_hovered_node_only`, `edit_location_ok_commits_one_move_and_one_undo_restores_it`, `edit_location_cancel_unchanged_or_invalid_commits_nothing`, `read_only_document_ignores_the_edit_location_tool`, `shortcuts_and_clicks_do_nothing_while_the_edit_location_dialog_is_shown`, `a_new_document_closes_the_edit_location_dialog`); ui.rs (`enter_commits_a_valid_dialog_and_escape_cancels_it`, `a_click_with_edit_location_opens_the_dialog_and_the_map_then_ignores_selection`); `golden_edited` sets one more untouched node to (123.457, 45.25, -678.901) and checks it reloads exactly.
- Verified: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` (incl. `golden_roundtrip`, `golden_edited`) pass. `frame_time` (release): the first two runs had the min-zoom whole/west-half selected drag and rotate cases at 17-23 ms; the same cases were over budget on the baseline without this change (machine load), and the third run had every case under 16 ms (worst: west half selected drag max 13.4 ms, rotate min zoom max 10.8 ms). The GUI hand check of the acceptance criterion is still for a human.

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 1 medium patched.
  - medium, patch: OK without an edit moved the node, since the prefill shows three decimals and fixture coordinates have more; it also refused a node whose y was already below 0. A field still showing its prefilled text now keeps the exact coordinate (`LocationDialog::values`, used by the dialog and the commit); test `untouched_fields_keep_the_exact_coordinate`.
- Verification after the patch: fmt, clippy and workspace tests pass.
