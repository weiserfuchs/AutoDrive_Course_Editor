---
title: 'The flip tool reverses the direction of a connection'
type: 'feature'
ticket: '7'
created: '2026-11-07'
status: 'built'
baseline_revision: 'a937bd7ca59090682318ae66f544ed5d582a76f5'
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

**Problem:** Users cannot reverse a connection's direction. Java's flip tool (F) does it with a documented four-pass workaround and skips dual links.

**Approach:** A flip tool (F, toolbar "Flip") reverses the clicked one-way or reverse connection in one transaction of `Disconnect` and `Connect` with the same shape from the other end. Dual links, and links that are not exactly one-way or reverse, are left unchanged. The click uses the edge hit rule of story 3.4 (a hovered node wins; only the drawn part counts), and the tool highlights the connection it would flip. Read-only disables the tool.

</frozen-after-approval>

## Implementation Notes

Oneshot: about 100 lines with tests on top of the delete and connect tools.

- `crates/ad-app/src/tools/flip.rs` (new): `flip::plan(net, a, b)` finds the exact shape (one-way or reverse) through `connect::is_exactly` (now `pub(crate)`) and returns `Disconnect(a, b)` + `Connect(b, a, shape)`; `FlipTool` ignores node clicks and paints the hovered edge like the delete tool. Tests: one-way and reverse flip and undo to identical link lists, dual unchanged from either side, node and empty clicks do nothing.
- `input.rs` (F, exact modifiers), `main.rs` (`Action::FlipTool` gated on `editable()`; tests: F plus a click on a connection flips it and Ctrl+Z restores, read-only ignores F), `ui.rs` (toolbar button after "Priority").
- `crates/ad-io/tests/golden_edited.rs`: flips a fixture one-way link between nodes no other edit touches; the reload shows it from the other end only.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

**Manual checks (if no CLI):**
- The user presses F, clicks a one-way connection on the fixture, sees the arrow turn, clicks a dual one with no effect, and undoes the flip.

## Review Triage Log

- Quick review, pass 1: 1 low patched; 3 low rejected.
  - low, patch: the flip tool highlighted every hovered connection, dual ones included, although a click on those does nothing. New `Tool::acts_on_edge` (default true) and `ViewHooks::highlights_edge`; the view hands only edges the tool acts on to `paint`, and the flip tool answers with `flip::plan`. Test extended (dual not highlighted, one-way highlighted).
  - low, reject: the app test calls `Action::FlipTool` instead of pressing F; the key mapping has its own table test, as for every other tool.
  - low, reject: the read-only flip test does not force the tool in and click; the click gate is shared (`App::click` returns early when not editable) and tested for the other tools.
  - low, reject: the golden test builds the flip transaction by hand; ad-io cannot depend on ad-app, and `flip::plan` is unit-tested to produce exactly that transaction.
- Verification re-run after the patch: fmt, clippy, workspace tests (ad-app 136) and the release frame-time test pass.
