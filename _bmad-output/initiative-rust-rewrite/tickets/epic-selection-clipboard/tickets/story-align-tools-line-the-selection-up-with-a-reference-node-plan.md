---
title: 'Align tools line the selection up with a reference node'
type: 'feature'
ticket: '7'
created: '2026-11-08'
status: 'in-progress'
baseline_revision: '0a664c607a904d0cabcea5125a4a3b41c7ff2e2c'
route: 'full'
route_source: 'auto'
risk: 'low'
review: ''
review_source: ''
lenses_ran: []
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Users cannot line nodes up. Java's align tools copy one coordinate of a clicked reference node to every selected node; its undo breaks for a target of exactly 0 (AD-13).

**Approach:** Three align tools as in Java (`AlignBaseButton`, `HorizontalAlignButton`, `VerticalAlignButton`, `DepthAlignButton`): with a non-empty selection, a primary click on a node (the reference, selected or not) sets every selected node's z (horizontal, Shift+X), x (vertical, Shift+Y) or y (depth, toolbar only) to the reference's value, in one `MoveNodes` with absolute before and after positions, so aligning to 0 undoes correctly. Nodes already at the value are left out; if none changes, nothing is committed. While the pointer hovers a reference node with a non-empty selection, a guide line through the reference along the aligned axis is drawn (Java's `drawLine`). Unlike Java, the selection is kept after aligning, as the other selection tools keep it.

## Boundaries & Constraints

**Always:** one transaction per click through `Document::apply`; read-only disables the tools (existing gates); a click on empty map or with an empty selection does nothing.

**Never:** no spacing or ordering of nodes; no connection changes; no overlap check (epic-scans-autosave).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Horizontal | three selected, Shift+X tool, click reference | every selected z = reference z; x, y kept; one MoveNodes | none |
| Vertical | Shift+Y tool | every selected x = reference x | none |
| Depth | depth tool | every selected y = reference y | none |
| Align to 0 | reference z = 0 | undo restores the old non-zero values (AD-13 fix) | none |
| Reference selected | the reference is part of the selection | it stays put; the others align | none |
| Nothing changes | all selected already at the value | nothing committed | none |
| Empty selection / empty click | no selection, or click off a node | nothing | none |
| Read-only | legacy config | tools disabled | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/align.rs` (new) -- `AlignTool { axis: Axis { Horizontal, Vertical, Depth } }`; pure `plan(net, selection, reference, axis) -> Option<Transaction>`; `on_click` uses `ctx.hovered_node` and `ctx.selection`; the guide line in `paint_selection` or `paint` (screen line through the reference, horizontal for z, vertical for x; depth draws none or a short marker). Names "Align H", "Align V", "Align depth".
- `crates/ad-app/src/tools/mod.rs`, `input.rs` (Shift+X, Shift+Y; exact modifiers), `main.rs` (actions gated on `editable()`), `ui.rs` (three toolbar buttons) -- wiring as the other tools.
- `crates/ad-core/src/command.rs` `MoveNodes` -- reuse.
- `crates/ad-io/tests/golden_edited.rs` -- one aligned group (z set to a reference's z) round-trips.
- Do not touch: view, selection input, `ad-core`.

## Tasks & Acceptance

**Execution:**
- [x] `tools/align.rs` -- tool and `plan`; tests for every matrix row except Read-only
- [x] wiring -- keys, toolbar, actions; tests: Shift+X/Shift+Y select the tools (and plain X/Y do nothing), read-only does nothing
- [x] `golden_edited.rs` -- aligned group round-trip

**Acceptance Criteria:**
- Given the fixture with a few selected nodes, when the user picks Shift+X and clicks a reference node, then the selected nodes line up horizontally with it and one Ctrl+Z restores them.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- The guide line needs the hovered node at paint time, which no hook carried. Without touching the view, `Tool::paint_selection` gained a `hovered: Option<NodeId>` parameter (rotate ignores it; `#[allow(clippy::too_many_arguments)]` as in `view.rs`), and `ToolHooks` hands it `app.view.hovered_node` from before `view.show`, so the guide follows the hover one frame late. Clicks still use the current frame's hover.
- Depth draws no guide, as in Java. The guide is a white dashed 3 px line at half alpha across the painter's clip rect.
- Verified: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (incl. `golden_edited` with the aligned group) and `frame_time` (all cases under 16 ms) pass. The GUI hand check (acceptance criterion on the fixture) is still for a human; `shift_x_aligns_the_selection_to_a_clicked_node_and_undo_restores_it` covers it headless.

## Plan Change Log

## Review Triage Log
