---
title: 'Edits run as undoable commands with saved state'
type: 'feature'
ticket: '4'
created: '2026-11-05'
status: done
baseline_revision: '04b453fad66ecdf3795942c5350463c1537f86ae'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: ['{project-root}/_bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `Network` can be changed, but no change can be undone, and nothing knows whether the document has unsaved changes. Every editing tool in later epics needs both (AD-4).

**Approach:** Add `Command`, `Transaction` and `History` to `ad-core`.
- Commands cover add, delete, connect, disconnect, move and marker edit.
- Each command records what it needs to revert itself exactly.
- `History` keeps a capped linear stack with never-reused `EntryId`s, discards the redo tail, and tracks the saved entry.
- `Network` gains the remove and reinsert primitives that the delete command needs.

## Boundaries & Constraints

**Always:**
- Undo then redo, and redo then undo, give identical networks. That covers node ids, `order`, every `outgoing`/`incoming` list including entry positions, flags, positions, markers and their order, and both spatial indexes.
- Delete records each node's order position. Undo restores the node under its original `NodeId` at that position (`Network::reinsert`, AD-2). It also restores neighbour list entries at their original positions and the node's marker at its original index.
- Redo of an add reuses the original `NodeId`. Ids are never reused for anything else.
- Move commands hold absolute before and after positions.
- One `Transaction` is one history entry and is undone in one step. If a command fails during apply, the commands already applied are reverted, the network is unchanged, and the error is returned.
- A new transaction discards the redo tail. Past the cap, the oldest entry is evicted.
- The document is dirty exactly when the current history position differs from the saved one. If the saved entry is evicted, or is discarded with the redo tail, the document stays dirty until the next `mark_saved`.
- `ad-core` gains no new dependency. No unwrap or expect.
- Decision (user, 2026-11-05): `DEFAULT_CAP` is 500.

**Never:**
- UI for undo/redo, shortcuts, close confirmation (epic-node-connection-tools).
- Settings file for the cap (story 8). The cap is a `History::new(cap)` argument with a `DEFAULT_CAP` constant.
- Save after edits (story 5), selection, `Document`/epochs (AD-18), `SetFlag` and other commands not listed.
- Changes to `ad-io` or `ad-app` beyond compiling.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Round trip per command | Each command (add, delete, connect, disconnect, move, marker edit) applied to a small linked network | undo → identical to before; redo → identical to after | none |
| Delete restores order | Delete a middle node with incoming, outgoing and a marker | undo restores id, order position, neighbour list positions, marker index | none |
| Multi-node delete | Delete two adjacent linked nodes in one transaction | one undo restores both and their shared links | none |
| Add redo keeps id | add → undo → redo | same `NodeId`; next new node gets a fresh id | none |
| Marker edit | create, rename, remove a marker | each undoes to the previous name, group and index | none |
| Transaction failure | Transaction whose second command names an unknown node | network unchanged, no entry pushed | `Err(NetworkError)` |
| Redo tail discard | apply A, B; undo B; apply C | redo not possible; B gone | none |
| Dirty tracking | fresh history; apply; mark_saved; undo; redo | clean, dirty, clean, dirty, clean | none |
| Saved entry evicted | cap 2; apply A; mark_saved; apply B, C, D | dirty, and undoing as far as possible stays dirty | none |
| Saved entry discarded | apply A, B; mark_saved; undo B; apply C | dirty, including after undoing C | none |
| Nothing to undo/redo | undo on empty history; redo at tip | returns false, no change | none |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/network.rs` -- story 3's API. Reuse it: `connect`, `disconnect`, `set_shape`, `move_node`, `add_node`, `push_marker`, `markers()`.
  - `Network` is not `PartialEq` (`RTree`). Tests compare `nodes`, `order`, `markers`, `next_id` and `edges_near(0,0,HUGE)`/`in_rect(full)` through a test helper.
  - Story 3's tests already show this pattern (`idempotent`).
- `crates/ad-core/src/lib.rs` -- add `pub mod command; pub mod history;`, the AD structural seed's file names.
- `crates/ad-core/tests/dependency_boundary.rs` -- do not edit.
- AD-4 in the architecture doc is the contract. `src/main/java/AutoDriveEditor/Managers/ChangeManager.java` has no cap and no saved state; port nothing from it.

## Tasks & Acceptance

**Execution:**
- [ ] `crates/ad-core/src/network.rs` -- add the primitives that return revert records.
  - `remove_node(id) -> Result<RemovedNode>`, `reinsert(RemovedNode)`. The record holds the node, order position, neighbour list entries with indexes and the marker with its index.
  - Link revert: `connect`/`disconnect`/`set_shape` return a `LinkDelta` of added and removed list entries with positions, and `revert_links(&LinkDelta)` undoes it.
  - Marker edits: `set_marker(node, name, group)` replaces in place or appends, `remove_marker(node)`, and a `MarkerDelta` with revert.
  - All of them keep both R-trees in sync.
- [ ] `crates/ad-core/src/command.rs` -- `Command` enum with `AddNode`, `DeleteNodes`, `Connect`, `Disconnect`, `MoveNodes`, `EditMarker` (`Some((name, group))` sets, `None` removes). Each has apply and revert against `&mut Network`. `Transaction(Vec<Command>)` applies in order and reverts in reverse.
- [ ] `crates/ad-core/src/history.rs` -- `EntryId`, `History`:
  - `new(cap)` and `DEFAULT_CAP`
  - `apply(&mut Network, Transaction) -> Result<EntryId, NetworkError>`
  - `undo`, `redo -> bool`
  - `can_undo`, `can_redo`
  - `mark_saved`, `is_dirty`
- [ ] tests in `network.rs`, `command.rs`, `history.rs` -- one test per matrix row. The round-trip helper compares full state, including the indexes.

**Acceptance Criteria:**
- Given any listed command applied through `History`, when it is undone and redone any number of times, then the network alternates between exactly two states.
- Given the workspace, when `cargo test` runs, then `golden_roundtrip` and the dependency-boundary test still pass.

## Implementation Notes

## Plan Change Log

## Review Triage Log

| # | Finding | Verdict | Route | Evidence |
|---|---------|---------|-------|----------|
| 1 | `remove_node` leaves a dangling `outgoing` entry and a stale edge for Reverse links into the deleted node | high | patch | Neighbours come only from the node's own lists, and Reverse adds a.out∋d without d.in∋a. The fixture has 9 Reverse links (checked with a script), so delete-then-save would write a bogus id. The fix looks up the incoming edges in the edge index, with tests. |
| 2 | A failed undo leaves the network partly reverted while the cursor stays put | false | reject | Revert fails only on a network changed outside History. AD-4 forbids that, and no reachable path produces it. |
| 3 | Rollback errors are swallowed in `Transaction::apply`, and `MoveNodes` loses the original error | low | reject | Only reachable when the rollback itself fails, the same unreachable state as #2. |
| 4 | `order_pos` falls back to `unwrap_or(0)` when the id is missing from `order` | false | reject | A node present in `nodes` is always in `order`, and every path maintains both. |
| 5 | `reinsert` and the revert clamps do not report misuse | low | reject | Only misuse outside History hits this. Making them return errors adds error paths for unreachable states. |
| 6 | Test gaps: deleting a Reverse target, `next_id`, and fresh id between undo and redo | low | reject | The Reverse-target test is part of patch #1. `next_id` is correctly excluded, because ids are never reused. The add-redo row is covered by the existing assertions. |

Deferred: stale `incoming`-only references (c.in∋d without d.out∋c) still dangle after deleting d. The fixture has none, so this is unverified for other configs.

## Design Notes

- **Revert records come from the `Network` primitive.** A command does not snapshot whole lists. A `LinkDelta` is a list of `(node, Out|In, index, value, Added|Removed)` records. Revert walks it backwards, removing added entries and inserting removed ones at their index.
- **First apply versus redo.**
  - `AddNode` stores its `NodeId` after the first apply. Undo calls `remove_node` and keeps the `RemovedNode`, and redo calls `reinsert` with it.
  - `DeleteNodes` removes its nodes in the given order, keeping one `RemovedNode` each. Undo reinserts them in reverse order, so each record's indexes are valid again.
- **Saved state:** `saved: Option<Position>`, where `Position` is "base" or an `EntryId`. `None` means lost.
  - Base is the position below the oldest entry still in the history.
  - Eviction of the saved entry, or discarding it with the redo tail, sets the saved state to lost.
  - Saved at base and then evicting the oldest entry also sets it to lost.
  - `is_dirty() = saved != Some(current)`.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `dependency_boundary`, and the new command/history tests
