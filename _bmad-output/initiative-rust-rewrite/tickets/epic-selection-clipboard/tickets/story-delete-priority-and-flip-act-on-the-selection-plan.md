---
title: 'Delete, priority and flip act on the selection'
type: 'feature'
ticket: '5'
created: '2026-11-08'
status: 'built'
baseline_revision: '80206b3914b399b966665c1523ebd925869cc077'
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

**Problem:** Delete, priority and flip act on one node or connection only; users need them on a whole selection (epic decision: only on a click, never while a selection is drawn).

**Approach:** A click with the delete, priority or flip tool on a **selected** node acts on the whole selection in one transaction:
- delete: one `DeleteNodes` of all selected nodes (in id order);
- priority: one `SetFlag` per selected node, each swapped by `priority::swapped` (sub-priority ↔ regular, other values → sub-priority);
- flip: one `Disconnect` + `Connect` pair per connection whose both ends are selected and that is exactly one-way or reverse (`flip::plan`); dual and other links unchanged; no such connection → nothing.
The Delete key deletes the selection when it is not empty, else the hovered node (story 3.4). A click on an unselected node, or on a connection, acts on that alone, as before. Undo restores everything in one step; the selection is pruned as usual (story 4.1).

## Boundaries & Constraints

**Always:** one transaction per click or key; every change through `Document::apply`; read-only disables tools and key (existing gates); any active tool preview is cancelled before the Delete key acts (existing rule).

**Never:** no tool acting while a selection is drawn; no change to the single-target behaviour; no new commands.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Delete selection | three nodes selected, delete tool click on one of them | all three deleted with their links; one undo restores ids, order, links | none |
| Priority selection | mixed flags selected, priority click on a selected node | each selected node swapped; one undo restores each old flag | none |
| Flip selection | selection with one-way, reverse and dual links inside, plus a link to an outside node | inner one-way and reverse flipped, dual and outside link unchanged; one undo | none |
| Flip nothing | selection without an inner one-way/reverse link | nothing committed | none |
| Unselected target | click on a node outside the selection, or on a connection | acts on that alone (stories 3.4, 3.6, 3.7) | none |
| Delete key | selection not empty / empty with a hovered node / empty and nothing hovered | deletes the selection / the hovered node / nothing | none |
| Read-only | legacy config | nothing | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/delete.rs`, `priority.rs`, `flip.rs` `on_click` -- when `ctx.hovered_node` is in `ctx.selection` (added in story 4.2), build the selection transaction; otherwise keep today's code. Put the selection rule in one small helper per tool or a shared `fn acts_on_selection(ctx) -> Option<&BTreeSet<NodeId>>` in `tools/mod.rs`.
- `crates/ad-app/src/tools/flip.rs` `plan(net, a, b)` -- reuse per inner connection; collect inner links from each selected node's `outgoing()` whose target is selected (each link once).
- `crates/ad-app/src/tools/priority.rs` `swapped` -- reuse.
- `crates/ad-app/src/main.rs` `delete_hovered` (~282) -- if `doc.selection` is not empty, `DeleteNodes` of the selection (sorted), else today's hovered path.
- `crates/ad-io/tests/golden_edited.rs` -- one `DeleteNodes` of a group of untouched nodes (several ids in one command) and its reload check, if not already covered by the existing multi-id delete.
- Do not touch: view, selection input, `ad-core`.

## Tasks & Acceptance

**Execution:**
- [x] tools -- selection paths with tests for Delete selection, Priority selection, Flip selection, Flip nothing, Unselected target (each with one undo restoring everything)
- [x] `main.rs` -- Delete key; tests for the three Delete key rows and Read-only
- [x] `golden_edited.rs` -- group delete round-trip if not covered

**Acceptance Criteria:**
- Given the fixture with a selected group, when the user clicks one of its nodes with delete, priority or flip, then the whole group changes and one Ctrl+Z restores it.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- Shared `tools::acts_on_selection(ctx)`: `Some(selection)` when the hovered node is selected. Flip collects inner links from each selected node's `outgoing()` (an exact one-way or reverse link is listed only in its start's `outgoing`, so each is planned once) and concatenates the `plan` commands into one transaction.
- The Delete key deletes the (pruned) selection without the hover-epoch check, which only guards the hovered path.
- `golden_edited.rs`: the existing deletes were single-id commands, so a three-id `DeleteNodes` of unmarked untouched nodes was added after the group move; the flag and flip order indexes are now taken after it.

- Review fixes: the hover uses the same selection-mark fallback as the drag start (`view::node_or_mark_at`, one reach), so a tool click on a mark works zoomed out; `MapView::selecting()` blocks the Delete key while a rectangle or lasso is drawn; the golden group delete excludes the added node.

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 1 medium and 2 low patched; 1 false rejected.
  - medium, patch: zoomed far out, a delete/priority/flip click on a selection mark did nothing (bug 4.11 fixed only the drag start). `node_or_mark_at` now serves the drag start and the per-frame hover; test `zoomed_out_a_delete_click_on_a_selection_mark_deletes_the_selection`.
  - low, patch: the Delete key deleted the old selection while a rectangle or lasso was being drawn. `MapView::selecting()` blocks it; test `the_delete_key_does_nothing_while_a_selection_is_drawn`.
  - low, patch: the golden group delete did not exclude the added node explicitly; it does now.
  - false: "the fixture criterion rests on automated tests only partly". It is the user's hand check.
- Verification after the patches: fmt, clippy, workspace tests pass. Frame time: one run showed the whole-network drag at 17.7 ms max; a back-to-back comparison then gave branch 9.7–11.0 ms and master 8.6–11.0 ms (3 runs each), so the spike was machine load, not this change.
