---
title: 'PasteNodes links new nodes to existing ones'
type: 'feature'
ticket: '1'
created: '2026-11-08'
status: done
baseline_revision: '1841e1a'
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

**Problem:** Every commit of this epic adds nodes and links them to the existing start and end node in one transaction. `PasteNodes` (story 4.9) links only new nodes to each other, by index, because the new ids are unknown until apply.

**Approach:** A link end becomes `LinkEnd::New(index)` (a node of this paste) or `LinkEnd::Existing(NodeId)`. Apply resolves the ends after `AddNodes`; an existing end that does not exist, or an index out of range, fails with the network unchanged. Revert removes the added links (only what `Connect` added) and the added nodes; redo keeps the ids. Markers stay by index. The clipboard builds `New` ends only, so copy and paste are unchanged. No user-visible change.

## Boundaries & Constraints

**Always:** the network changes only through the command's parts (`AddNodes`, `EditMarker`, `Connect`); a failed apply leaves the network exactly as before.

**Never:** no new command type; no change to the clipboard's behaviour; no UI.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Chain | existing A, B; nodes n0, n1; links A→n0, n0→n1, n1→B one-way | links exist with their shapes; revert leaves A and B with their old links only; redo same ids | none |
| Shapes | dual and reverse links to an existing node | both ends' lists as `Connect` makes them | none |
| Existing already linked | A→n0 where nothing exists yet, plus an existing A→B link untouched | A→B unchanged after apply and revert | none |
| Missing existing end | `Existing(id)` of a deleted node | error, network unchanged | `NetworkError` from `Connect` |
| Bad index | `New(5)` with two nodes | `IndexOutOfRange`, network unchanged | existing error |
| Paste | copy and paste of story 4.9 | unchanged | none |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/command.rs` -- `pub enum LinkEnd { New(usize), Existing(NodeId) }`; `PasteNodes.links: Vec<(LinkEnd, LinkEnd, ConnectionShape)>`; resolution in `apply`; tests.
- `crates/ad-app/src/clipboard.rs` -- build `LinkEnd::New` ends; the clipboard's own index links stay as they are.
- `crates/ad-app/src/document.rs` tests, `crates/ad-io/tests/golden_edited.rs` -- adapt constructors; golden: one pasted chain linked to two untouched existing nodes saves and reloads with those links.

## Tasks & Acceptance

**Execution:**
- [x] `LinkEnd` and resolution -- tests for every matrix row except Paste
- [x] callers adapted -- clipboard and document tests unchanged in behaviour
- [x] `golden_edited.rs` -- chain linked to existing nodes round-trip

**Acceptance Criteria:**
- Given a network with nodes A and B, when a `PasteNodes` adds two nodes linked A → n0 → n1 → B and is undone and redone, then the links and ids match exactly each time and A and B keep their other links.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`

## Implementation Notes

- `LinkEnd { New(usize), Existing(NodeId) }` in `ad-core::command`; `PasteNodes::apply` resolves both kinds after `AddNodes`. A missing existing node fails in `Connect` (`check_pair`), the parts applied so far roll back and the add is reverted, so the network is unchanged. Revert is the parts' revert: each `Connect` removes only the entries it added.
- The clipboard keeps its index links and maps them to `LinkEnd::New` when it builds the paste.
- Tests: `paste_links_new_nodes_to_existing_ones_and_reverts_only_its_links` (chain, dual and reverse to existing ends, existing links untouched, exact round trip), `paste_with_a_missing_existing_end_fails_and_changes_nothing`; golden `pasted_chain_linked_to_existing_nodes_saves_and_reloads`.
- Verified: fmt, clippy `-D warnings`, `cargo test --workspace` (ad-core 64 + 4, ad-app 260 + 2 ignored, ad-io 39, golden_edited 3, golden_roundtrip 1).

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked that the bad-index error still comes before any network change and that the clipboard's links stay index-only.
