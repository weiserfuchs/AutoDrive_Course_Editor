---
title: 'Refactor sweep'
type: 'chore'
ticket: '11'
created: '2026-11-09'
status: done
baseline_revision: 'b1da10f'
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

**Problem:** This sweep handles what epic-settings-ui-shell's plans and reviews deferred and checks the UI-shell inventory against the result.

**Approach:**
- Collect every item the epic's plans and reviews deferred, and every deferred-work.md entry assigned to this epic; check each against the current code and record the outcome here.
- Check each gap group and drop item of the inventory (research-ui-shell-inventory, sections 3.1 and 3.2) against the code.
- Scan the crates for TODO/FIXME and for `#[allow(...)]` this epic added.
- No code changes unless a check finds a real defect owned by this epic.

</frozen-after-approval>

## Implementation Notes

Oneshot: nothing to change.

### deferred-work.md entries for this epic

- **Retro R4 (Save is always Save As):** resolved by story 2, recorded there.
- **Retro P2:**
  - `AddNode` → `AddNodes` was already done in an earlier epic.
  - Errors as `String` and no string table stay. Localisation is out of this epic by the user's decision (2026-11-09), and replacing the error type is a refactor across ad-app, not cleanup.
  - Accepted; it is for epic 10 or later if wanted.

### Inventory gap groups (section 3.1)

| # | Group | Outcome |
|---|---|---|
| 1 | Settings persist and write back | done (story 1); Java's max zoom per map is global here (story 4) |
| 2 | Menu bar, Save vs Save As | done (story 2); Routes is a File item, not its own menu |
| 3 | Recent files, last folder | done (story 3) |
| 4 | Window place | done (story 3) |
| 5 | Settings window | done (story 4); see "not ported" |
| 6 | Configurable key bindings | done (story 5); Copy, Cut and Paste fixed (OS clipboard keys) |
| 7 | Missing shortcuts | done: G, F1–F3, Shift+F, +/− (story 6), H (story 7), D (story 8), file keys Ctrl+O/S/Shift+S/Q (story 2); no keys for the import items (no Java key kept, the user chose Ctrl keys only for File) |
| 8 | Display toggles, map scale | done (story 6) |
| 9 | Visibility filters | done per item as in Java (story 7, see its change log) |
| 10 | Grid and rotation snap | done (story 8); subdivision lines are not drawn |
| 11 | Map info | done as a window (story 6) |
| 12 | Update check | done (story 9); no "check now" item |
| 13 | Help menu | done (story 9) |
| 14 | Log access | "Open log folder" (story 9); no log window and no log-level setting |
| 15 | Message panel | out (epic boundary) |
| 16 | Tool-bar presentation | groups, tooltips with keys, Cut/Copy/Paste (story 10); no icons |
| 17 | Theme | done: system, light, dark (story 4) |
| 18 | Go to node id | done (story 6) |

### Drop list (section 3.2)

All dropped by the user (2026-11-09): Debug menu, Experimental mode, Move Widget tab, the FlatLaf theme catalogue, the ☰ panel toggles, AutoSave Apply Now/Later and Alt mnemonics. Colours stay fixed. `EditorConfig.xml` import and localisation are out.

### Not ported, left for the user to ask for

- Middle-mouse-only panning: Rust pans with both buttons.
- Interpolated zoom.
- Outline arrowheads.
- Subdivision grid lines.
- Tool-bar icons.
- A log window or log level.
- A manual "Check for updates".
- Java's four per-kind connection hide buttons; here one Hide connection tool.

### Code scan

- No TODO, FIXME, `todo!` or `unimplemented!` in `crates/`.
- The epic added `#![allow(clippy::expect_used)]` only in test modules (the convention).

### Open for the user

The hand checks of stories 1–10 (and of epic 11).

### Verification

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (ad-app 456 + 4 ignored) and the release frame-time test pass.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass

## Review Triage Log

- Quick review (orchestrator, inline): no finding.
