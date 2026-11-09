---
title: 'Nodes and each connection kind can be hidden'
type: 'feature'
ticket: '7'
created: '2026-11-09'
status: 'built'
baseline_revision: '0203bca'
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

**Problem:** Java's Show/Hide tool group, Select Hidden (H) and the hidden transparency have no Rust counterpart.

**Approach (follows Java; see the Plan Change Log):**
- **Tools:** two new tool-bar tools.
  - **Hide node** toggles the clicked node, or the whole selection when the clicked node is selected.
  - **Hide connection** toggles the clicked connection, both directions as one.
  - Hiding changes no data and has no undo. It lasts until another document opens, as in Java (session state).
- **Drawing:** hidden nodes and connections are drawn with the "Hidden opacity" setting (Connections tab, 0–1).
- **Picking:** without Select hidden (H, Display menu, persisted), hidden items are neither hovered nor clicked nor right-clicked, and the rectangle and lasso skip hidden nodes. The hide tools always reach them (Java `alwaysSelectHidden`).
- **Show all hidden** (Display menu) shows everything again.
- The user started this story while away; the plan was not approved separately, and the hand check is pending.

## Boundaries & Constraints

**Never:** hidden state is not saved in the config or the settings; no change to the network or history.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Hide click | Hide node on a node / on a selected node | that node / the selection toggled | none |
| Hide link | Hide connection on (b, a) | the pair hidden for both directions | none |
| Hover | hidden node, no tool | not hovered; hide tool or Select hidden: hovered | none |
| Rectangle | rectangle over a hidden node | not selected; with Select hidden: selected | none |
| H | H pressed | Select hidden flips and persists | none |
| Show all | Show all hidden | nothing hidden | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/view.rs` -- `Hidden`, `HiddenView`, `ViewHooks::hidden`; hover, right click and drawing honour it.
- `crates/ad-app/src/tools/hide.rs` (new) -- `HideNodeTool`, `HideLinkTool`; test.
- `crates/ad-app/src/tools/mod.rs` -- `ToolRequest::ToggleHiddenNodes`, `ToggleHiddenLink`, `Tool::picks_hidden`.
- `crates/ad-app/src/document.rs` -- `Document::hidden`.
- `crates/ad-io/src/settings.rs` -- `select_hidden`, `hidden_alpha`.
- `crates/ad-app/src/input.rs`, `main.rs`, `ui.rs` -- H, the requests, the tool-bar tools, the Display items, the opacity slider, `select` skipping hidden nodes; tests.

## Tasks & Acceptance

**Execution:**
- [x] view and tools -- tests Hide click, Hide link, Hover
- [x] selection and menu -- tests Rectangle, H, Show all

**Acceptance Criteria:**
- Given an open map, when the user hides nodes and connections with the hide tools, then they fade to the hidden opacity and no other tool or selection takes them until Select hidden is on.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- Java has four connection hide buttons, one per kind; each only limits which connections a click reaches, and a double right click resets that kind. Here one Hide connection tool reaches any kind, and Show all hidden resets everything.
- The Java default opacity is 1.0, which draws hidden items exactly as shown ones. The default here is 0.25 so hiding is visible; the setting goes from 0 to 1 as in Java.
- Verification: fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 450 + 4 ignored); frame time ok, worst best-round max 9.8 ms.

## Plan Change Log

- 2026-11-09: the ticket said "Show nodes and Show regular, dual, reverse and subprio connections" as display filters per kind. Java's hide works per item (tools), so this story follows Java; the epic's inventory rows 2.8 Show/Hide are covered this way.

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: the hidden lookups run only while something is hidden; a node deleted while hidden leaves a stale id that matches nothing.
