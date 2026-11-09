---
title: 'Key bindings are configurable'
type: 'feature'
ticket: '5'
created: '2026-11-09'
status: 'built'
baseline_revision: 'e9147c9'
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

**Problem:** The shortcut table is fixed in code (`input::SHORTCUTS`); R14 says shortcuts are configurable, as in Java's KeyBinds tab.

**Approach:**
- `settings.toml` gains a `[shortcuts]` table holding only the bindings that differ from the defaults: action id → key text, e.g. `node_tool = "Shift+N"`.
  - An empty text means no key.
  - A text that names no key, or an unknown id, is ignored with a log line, so the default stays.
- A rebound action loses all of its default keys; Continuous connections, for example, has two.
- The app rebuilds its shortcut table whenever the overrides change. Shortcuts, menus and tooltips all use that table.
- The settings window gets a "Key bindings" tab. It lists each configurable action with its key and three buttons:
  - **Set…** waits for a key. Escape alone cancels. A key already bound to another action is refused with "… is used by …".
  - **Clear** removes the key.
  - **Reset** restores the default.
  - **Reset all** drops every change.
- Copy, Cut and Paste are not configurable: Ctrl+C/X/V reach the app as OS clipboard events, not as keys.
- The user started this story while away; the plan was not approved separately, and the hand check is pending.

## Boundaries & Constraints

**Never:** no change to the defaults; no tool-bar tooltips (story 10).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Text round trip | every default | `parse(format(s)) == s` | none |
| Rebind | `node_tool = "Shift+N"` | Shift+N selects the node tool, N does nothing | none |
| Clear | `toggle_continuous = ""` | `;` and Shift+`;` do nothing | none |
| Bad text | `node_tool = "Ctrl+Nope"`, unknown id | defaults stay | logged |
| Capture | Set… on Node tool, Shift+N | stored, table rebuilt | none |
| In use | Set… on Move tool, Shift+N | refused with the note | none |
| Escape | while waiting | capture cancelled, window open | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/settings.rs` -- `Settings::shortcuts`.
- `crates/ad-app/src/input.rs` -- `CONFIGURABLE` (id, label), `bindings`, `parse_shortcut`, `shortcut_to_text`, `captured`, `bound_elsewhere`, `action_id`, `action_label`; `pressed` and `shortcut_text` take the table; tests.
- `crates/ad-app/src/main.rs` -- `bindings`, `bindings_from`, `capturing`; rebuilt in `apply_settings`.
- `crates/ad-app/src/ui.rs` -- `Capture`, `take_captured_key`, `key_bindings_tab`; menus use the table; test.

## Tasks & Acceptance

**Execution:**
- [x] table from settings -- tests Text round trip, Rebind, Clear, Bad text
- [x] Key bindings tab -- tests Capture, In use, Escape

**Acceptance Criteria:**
- Given the Key bindings tab, when the user sets a new key for a tool and restarts, then the tool answers to the new key and the menus show it.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- Ctrl in the settings text stands for egui's command modifier, as in the default table; a captured Ctrl press is stored as Ctrl.
- While the tab waits for a key, every key press of that frame is consumed, so neither a shortcut nor the window's Escape acts.
- Verification: fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 444 + 4 ignored); frame time ok.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: shortcuts do not act while the settings window is shown (`dialog_shown`), so a captured key never also runs its old action; leaving the tab or closing the window stops a capture.
