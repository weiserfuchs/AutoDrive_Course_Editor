---
title: 'Connect tools create regular, dual and reverse connections'
type: 'feature'
ticket: '5'
created: '2026-11-07'
status: done
baseline_revision: '05beeaf1399af66329db10576d9754745ffefbe4'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/initiative-rust-rewrite/tickets/epic-node-connection-tools/tickets/story-the-delete-tool-removes-the-node-or-connection-under-the-cursor-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Users cannot create connections. They need three connect tools (regular, dual, reverse) that link two existing nodes, change an existing link's shape, or remove it, each undone in one step.

**Approach:** Each connect tool takes a start node on the first click and an end node on the second. Clicking the start node again cancels. On the end click the tool compares the pair's current links with its own shape from start to end (AD-3: Network operations stay idempotent, the tool decides):
- no link between the two → `Connect` with the tool's shape;
- the links are exactly the tool's shape from start to end → `Disconnect` (removed);
- any other links between the two → one transaction of `Disconnect` then `Connect`, so the pair ends up with exactly the tool's shape from start to end.
The pending link is drawn from the start node to the cursor. L selects the regular tool and cycles regular → dual → reverse (Java's line group); `;` toggles continuous connections (Java default off): after a commit the end node becomes the next start node.

## Boundaries & Constraints

**Always:** every change goes through `Document::apply` (AD-4) as one transaction per end click; hits use the story 3.4 rule (`ctx.hovered_node`); Undo/Redo and tool switches cancel a pending start (existing cancel path); read-only disables the tools.

**Never:** no interpolated nodes and no end node on empty space (epic-curves-lines; Java's `linearLineNodeDistance` and `CreateLinearLineEndNode`); no sub-priority node choice on the tool button (only matters with created nodes); no change to `Network` semantics.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Create | regular tool, click A then B, no links between them | A→B one-way created (`A.out∋B`, `B.in∋A`); one history entry | none |
| Create dual / reverse | dual or reverse tool, A then B, no links | dual: both directions; reverse: `A.out∋B` only | none |
| Remove same | links between A and B are exactly the tool's shape from A to B | all links between A and B removed | none |
| Reshape | A→B one-way exists, dual tool A then B (or B→A one-way exists, regular tool A then B) | one transaction; afterwards exactly the tool's shape from A to B; one undo restores the old links | none |
| Cancel | second click on the start node | pending start cleared, nothing committed | none |
| Empty click | click with no hovered node | nothing; a pending start stays | none |
| Continuous | `;` on, A then B, then C | A→B committed, then B→C; B became the start node | none |
| L cycles | L pressed repeatedly | regular → dual → reverse → regular | none |
| Read-only | legacy config | tools disabled | History rejects too |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/connect.rs` (new) -- `ConnectTool { shape: ConnectionShape, start: Option<NodeId>, continuous: bool }`; `on_click` uses `ctx.hovered_node`; a pure `fn plan(net, a, b, shape) -> Option<Transaction>` encodes the three-way rule (unit tests per matrix rows Create, Create dual/reverse, Remove same, Reshape); `paint` draws a line from the start node's screen position to the cursor and a ring on the start node; `cancel` clears `start`; `cycled(active)` like `node::cycled`.
- "Exactly the tool's shape from A to B": OneWay = `A.out∋B ∧ B.in∋A ∧ B.out∌A ∧ A.in∌B`; Dual = both directions with incoming; Reverse = `A.out∋B ∧ B.in∌A ∧ B.out∌A ∧ A.in∌B`. Read `Node::outgoing()`/`incoming()` (network.rs:58-66).
- `crates/ad-core/src/command.rs` `Connect { a, b, shape }`, `Disconnect { a, b }` -- reuse; `Disconnect` removes every link both ways (network.rs:478).
- `crates/ad-app/src/tools/mod.rs` -- register the module; `ToolContext` already has `hovered_node`, `net`.
- `crates/ad-app/src/input.rs` -- `Action::ConnectTool` on L, `Action::ToggleContinuous` on `;` (egui `Key::Semicolon`), exact modifiers.
- `crates/ad-app/src/main.rs` `App::run` -- L cycles via `connect::cycled`; `;` flips `App.continuous` (session only) and the active connect tool's flag.
- `crates/ad-app/src/ui.rs` -- toolbar buttons "Connect", "Dual", "Reverse" and a "Continuous" checkbox.
- `crates/ad-io/tests/golden_edited.rs` -- add a reverse `Connect` and a reshape transaction (`Disconnect` + `Connect`) on fixture pairs no other edit touches; the reload check covers them.
- Do not touch: `hover.rs` rule, `view.rs` drawing beyond what the overlay needs.

## Tasks & Acceptance

**Execution:**
- [x] `tools/connect.rs` -- tool, `plan`, `cycled`, with tests for every matrix row that needs no UI (Create, dual/reverse, Remove same, Reshape incl. undo, Cancel, Empty click, Continuous)
- [x] `input.rs`, `main.rs`, `ui.rs` -- L, `;`, toolbar; tests: L cycles, `;` toggles, read-only does nothing
- [x] `golden_edited.rs` -- reverse link and reshape round-trip

**Acceptance Criteria:**
- Given the fixture, when the user connects two nodes with each tool, reshapes one link and removes one, then the colours and arrows change as expected and each step undoes in one Ctrl+Z.
- Given the frame-time test, when run in release, then every case stays under 16 ms.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Tool.** `tools/connect.rs`: `ConnectTool { shape, start, continuous }`, names "Connect", "Dual", "Reverse". `start` keeps the start node's id and its world (x, z) from the click, so `paint` needs no network: the ring (8 px) and the line to the cursor are drawn in the shape's connection colour (`look::REGULAR`/`DUAL`/`REVERSE`). The stored position cannot go stale: Undo, Redo, the Delete key and tool switches all cancel first. An empty click returns early and keeps the start; a click on the start clears it.
- **Rule.** `plan(net, a, b, shape)` returns `None` for a missing node or `a == b`; otherwise `[Connect]` when no entry of either node's `out`/`in` names the other, `[Disconnect]` when `is_exactly` holds (the Code Map's four-flag rule), else `[Disconnect, Connect]`. A one-way link the other way is therefore reshaped, not removed. With `continuous`, the end node becomes the next start after every end click, whether it created, removed or reshaped.
- **Continuous.** `Tool::set_continuous` (default no-op) lets `;` reach the active connect tool; `App.continuous` (session only, default off) seeds new connect tools from L and the toolbar. The "Continuous" checkbox runs `Action::ToggleContinuous`. The toggle works in read-only documents too, like Snap; it changes no document.
- **Keys.** L (no modifiers) runs `connect::cycled`: regular from any non-connect tool, then dual, reverse, regular. `;` maps to `Key::Semicolon` with no modifiers *and* with Shift alone (see Plan Change Log).
- **Toolbar.** "Connect", "Dual", "Reverse" after "Delete", then "Continuous" before "Snap"; the tool buttons are disabled for read-only documents.
- **Tests.** connect: create per shape, remove exactly-same per shape, reshape the other way, reshape one-way to dual with revert restoring it, dual to reverse, missing/same node, cancel by clicking the start, empty click keeps the start, continuous, cancel, L cycle. main: L cycles, `;` toggles the session flag and the active tool (continuous chain, then a non-continuous commit), read-only ignores L and clicks, connect → reshape → remove with one Undo each and Undo dropping a pending start. input: L, `;`, Shift+`;`; Shift+L, Ctrl+L, Ctrl+`;` do nothing. golden_edited: a reshape transaction (`Disconnect` + `Connect`, to Dual or, for a dual link, OneWay) on an untouched fixture link and a reverse `Connect` between two untouched unlinked nodes reload with the same kinds.
- **Frame-time (release).** Every case under 16 ms; worst best-round max 3.05 ms, worst single round 5.13 ms.

## Plan Change Log

- `;` also fires with Shift held. On layouts such as German, `;` is Shift+`,`, and egui reports the typed logical key `Semicolon` with Shift set, so an exact no-modifier match would never fire there. On US layouts Shift+`;` types `:` (`Key::Colon`), so the extra entry cannot collide. Modifiers still match exactly per entry.

## Review Triage Log

- Quick review, pass 1: no findings. Observations only: the colour/arrow check is the user's hand check; the release frame-time run was repeated on review (every case under 16 ms); reverse is covered by the tool unit tests and golden_edited, not by the app-level undo test.
