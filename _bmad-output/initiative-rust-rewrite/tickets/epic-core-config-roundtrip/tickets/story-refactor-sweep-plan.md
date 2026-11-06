---
title: 'Refactor sweep'
type: 'chore'
ticket: '9'
created: '2026-11-06'
status: done
baseline_revision: 'ca267ada1fb447851d1042dab6cfa28d3cecc145'
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

**Problem:** This sweep handles what the epic's plans and reviews deferred.
- AGENTS.md still says the Rust project does not exist yet, so agents miss the crate layout and the check commands.
- The deferred list holds an entry (stale `incoming`-only references after a delete) that story 5 may already have settled.

**Approach:**
- Decision (user, 2026-11-06): make a minimal AGENTS.md edit. Replace the TODO line under "Running and verifying" with the crate layout, the check commands (the same ones CI runs), how to run the app, and where settings and logs live. Nothing else in AGENTS.md changes.
- Check the deferred `incoming` entry against the code, and record the outcome here.
- No code changes unless that check finds a real defect.

</frozen-after-approval>

## Implementation Notes

Oneshot: this is a documentation-level sweep. The code scan found no TODO/FIXME, and only the two commented `#[allow(dead_code)]` on `Document` (kept until the edit stories use them).

- **AGENTS.md:** the TODO line under "Running and verifying" is replaced by four lines: the workspace layout, the checks (the same commands as `.github/workflows/ci.yml`), how to run the app, and where settings and logs live. Nothing else changed.
- **Deferred stale `incoming` references (story 4):** settled by story 5, so no code change was made.
  - `config_xml::join_nodes` drops every list entry that `SaveIdMap` cannot map, and writes `-1` for an emptied list. The unit test `dangling_incoming_reference_is_skipped` covers this.
  - A stale `NodeId` in memory can never point at another node, because ids are never reused (AD-2).
  - Undo of the delete reinserts the node under its old id, which makes the reference consistent again.
  - So a save never writes an invalid id. The deferred-work entry stays as written (append-only); this note closes it.

## Review Triage Log

- **Hand edit inside the managed `bmad:context` block can be lost on refresh** -- low, defer. Real: the block header says a refresh replaces edits. The user chose the minimal edit, and the fix (a refresh through bmad-project-context) edits agent-context. Grouped with the stale stamp below.
- **The "Verified 2026-11-05 against c3b4f8b" stamp is stale** -- low, defer (same root cause as above). The intent forbids other AGENTS.md changes, and a refresh re-stamps it.
- **The Policy merge gate differs from the CI checks** -- low, defer. The mismatch predates this change. Its fix edits agent-context.
- **The closed `incoming` entry in deferred-work.md still looks open** -- low, reject. The file is append-only and the closure is recorded in this plan, which the retrospective reads.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
