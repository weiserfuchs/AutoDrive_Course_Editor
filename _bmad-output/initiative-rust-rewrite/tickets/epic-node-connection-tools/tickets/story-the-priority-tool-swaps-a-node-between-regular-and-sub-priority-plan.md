---
title: 'The priority tool swaps a node between regular and sub-priority'
type: 'feature'
ticket: '6'
created: '2026-11-07'
status: done
baseline_revision: '1542a5f5ff4f78cbe71e5a5b15b226024da557a2'
route: 'oneshot'
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

**Problem:** Users cannot change a node's priority. Java's priority tool (P) swaps the clicked node between regular and sub-priority, and its `1 - flag` turns any other file value into garbage.

**Approach:** A priority tool (P, toolbar "Priority") sets the clicked node's flag through a new `SetFlag` command that records the old flag, so undo restores any file value. Sub-priority becomes regular; every other value becomes sub-priority. Connection colours follow from the derived kind; read-only disables the tool.

</frozen-after-approval>

## Implementation Notes

Oneshot: about 100 lines with tests, mechanical on top of the existing tool pattern.

- `crates/ad-core/src/command.rs`: `SetFlag { node, flag, old }`; apply reads the old flag first and fails without change on an unknown node; revert sets the old flag. Tests: round trip from a file value 4, unknown node changes nothing.
- `crates/ad-app/src/tools/priority.rs` (new): `PriorityTool` and `swapped(flag)`; tests for regular ↔ sub-priority with the derived kind changing to `SubPrio` and back, file value 4 → sub-priority with undo to 4, click off a node.
- `input.rs` (P, exact modifiers), `main.rs` (`Action::PriorityTool` gated on `editable()`; tests: P plus click swaps and Ctrl+Z restores, read-only ignores tool and click), `ui.rs` (toolbar button after "Delete").
- `crates/ad-io/tests/golden_edited.rs`: swaps the flag of a fixture node no other edit touches; the reloaded node has the new flag.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

**Manual checks (if no CLI):**
- The user presses P, clicks a fixture node, sees its connections recolour, and undoes it.

## Review Triage Log

- Quick review, pass 1: no findings.
