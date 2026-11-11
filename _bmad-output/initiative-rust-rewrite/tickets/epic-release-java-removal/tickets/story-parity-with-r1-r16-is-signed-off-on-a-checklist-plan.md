---
title: 'Parity with R1-R16 is signed off on a checklist'
type: 'chore'
ticket: '4'
created: '2026-11-09'
status: done
baseline_revision: '4114df4'
route: 'oneshot'
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

**Problem:** the epic is done only when the user signs off parity with R1-R16 against the fixture and the UI-shell inventory.

**Approach:**
- `_bmad-output/initiative-rust-rewrite/parity-signoff/parity-signoff.md` lists:
  - each requirement R1-R16 with a hand step in the release exe and its automated evidence;
  - the inventory groups of the Rust UI;
  - the agreed differences;
  - a gaps table.
- The user walks it and signs it off. Gaps become stories or are accepted.
- Story 5 (Java removal) waits for the sign-off, so the Java reference is still there during the check.

</frozen-after-approval>

## Code Map

- `_bmad-output/initiative-rust-rewrite/parity-signoff/parity-signoff.md` (new).

## Implementation Notes

- The checklist warns to edit only copies of the fixtures (AGENTS.md).
- The update check can be seen only after the v0.9.0 release (story 7); the checklist says so.
- Status stays open until the user signs off; the story becomes done then.

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Every R1-R16 and every kept inventory group has a line; the drop list matches the epic 9 decisions.
