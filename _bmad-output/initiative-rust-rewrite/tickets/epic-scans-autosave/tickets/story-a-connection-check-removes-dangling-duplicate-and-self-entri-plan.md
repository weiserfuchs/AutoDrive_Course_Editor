---
title: 'A connection check removes dangling, duplicate and self entries'
type: 'feature'
ticket: '4'
created: '2026-11-08'
status: 'planned'
baseline_revision: 'c1cefd1'
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

**Problem:** A config edited by other tools can carry broken connection entries the game and the editor read inconsistently. Java has no check for them. Three kinds are agreed (epic decision):
1. **Dangling incoming:** node B lists A in `incoming`, but A does not list B in `outgoing`.
2. **Duplicates:** the same id twice in one node's `outgoing` or `incoming`.
3. **Self-links:** a node listing itself in `outgoing` or `incoming`.

A reverse link (A lists B in `outgoing`, B does not list A in `incoming`) is valid and never touched.

**Approach:**
- First confirm what the loader keeps: if it already drops duplicates or self-links on load, the check covers only what can still exist, and the notes say so (the round trip must stay byte-identical for the fixture).
- Pure check in ad-core: `connection_defects(net) -> Defects { dangling: Vec<(node, value)>, duplicates: Vec<(node, side, value)>, self_links: Vec<(node, side)> }`.
- **Scan menu "Check connections…"** (any open document): a modal lists the counts per kind; with defects and an editable document it offers "Fix", else only "Close". No defects: a notice "Connections are consistent" and no modal.
- **Fix** removes exactly the defective entries (for duplicates, the extra occurrences only, keeping the first) in one transaction, through a small ad-core command if no existing one fits (`RemoveLinkEntries` with exact revert, per AD-4 naming). The touched nodes become the selection; one undo restores every entry at its position.

## Boundaries & Constraints

**Always:** real one-way, dual and reverse links stay exactly as they are; read-only shows the report without Fix.

**Never:** no new links; no node deletion; no change to how the loader parses.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Dangling | B.in = [A], A.out = [] | reported; Fix removes A from B.in | none |
| Reverse | A.out = [B], B.in = [] | not reported | none |
| Duplicate | A.out = [B, C, B], B.in = [A] | reported once; Fix leaves A.out = [B, C] | none |
| Self | A.out = [A, B] | reported; Fix leaves A.out = [B] | none |
| Clean | the fixture | notice "Connections are consistent" (or the real counts, recorded) | none |
| Undo | Fix, Ctrl+Z | every entry back at its index (exact snapshot) | none |
| Read-only | legacy config with defects | report shown, no Fix | none |
| Save | fixed network saved and reloaded | no defects after reload | none |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/scan.rs` -- `connection_defects`; `crates/ad-core/src/command.rs` (+ `network.rs` helpers) -- the removal command with exact revert; tests (use `round_trip`).
- `crates/ad-io/src/config_xml.rs` -- read only: what the loader keeps; a test loading a config with each defect.
- `crates/ad-app/src/document.rs`, `main.rs`, `ui.rs` -- menu item, report modal, Fix through `apply_selecting`, notices.
- `crates/ad-io/tests/golden_edited.rs` -- a fixed network saves and reloads clean.

## Tasks & Acceptance

**Execution:**
- [ ] loader behaviour confirmed and recorded
- [ ] check and removal command -- tests for Dangling, Reverse, Duplicate, Self, Undo
- [ ] app -- tests for Clean, Read-only, selection; golden Save case

**Acceptance Criteria:**
- Given a copy of the fixture with one dangling incoming entry, one duplicate and one self-link planted in the XML, when the user runs "Check connections…", then the report shows one of each, Fix removes them, the saved file reloads clean, and one Ctrl+Z restores them.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
