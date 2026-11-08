---
title: 'PasteNodes links new nodes to existing ones'
type: 'feature'
ticket: '1'
created: '2026-11-08'
status: 'planned'
baseline_revision: '1841e1a'
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
- [ ] `LinkEnd` and resolution -- tests for every matrix row except Paste
- [ ] callers adapted -- clipboard and document tests unchanged in behaviour
- [ ] `golden_edited.rs` -- chain linked to existing nodes round-trip

**Acceptance Criteria:**
- Given a network with nodes A and B, when a `PasteNodes` adds two nodes linked A → n0 → n1 → B and is undone and redone, then the links and ids match exactly each time and A and B keep their other links.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`

## Implementation Notes

## Plan Change Log

## Review Triage Log
