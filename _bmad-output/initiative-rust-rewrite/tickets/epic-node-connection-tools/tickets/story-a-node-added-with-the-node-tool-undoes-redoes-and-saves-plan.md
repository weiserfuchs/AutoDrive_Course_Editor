---
title: 'A node added with the node tool undoes, redoes and saves'
type: 'feature'
ticket: '1'
created: '2026-11-07'
status: 'in-progress'
baseline_revision: '1352f7866c887fc26155e1199d3ef8a3a2778659'
route: 'full'
route_source: 'auto'
risk: 'high'
review: ''
review_source: ''
lenses_ran: []
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The editor cannot change the network: there is no tool, no toolbar, no undo or redo in the UI, and no shortcut table. The tracer of epic-node-connection-tools proves the whole edit path on the FS25 fixture: pick a tool, add a node, undo, redo, save, reload.

**Approach:** Add the Tool trait (AD-10), a toolbar with exactly one active tool, one fixed shortcut table with the Java defaults, Undo and Redo, and a node tool that adds a node on a left click through `AddNodes`, its height from `sample_height`. Read-only documents disable tools, Undo and Redo; a newly loaded document cancels the active tool (AD-18).
- Decision (user, 2026-11-07): mouse buttons as in Java: a left drag always pans and a left click goes to the active tool; the middle button pans too. A later tool claims the left drag only when it starts on its target (story 3.2: a node).
- Decision (user, 2026-11-07): the plan stays whole above the token target; the tracer spans every layer on purpose.

## Boundaries & Constraints

**Always:** every network change goes through `Document::apply` / `History` (AD-4); height only from `ad_core::heightmap::sample_height`, `None` written as y = -1 (AD-7); shortcuts live in one table in `input.rs`; Redo is Ctrl+Shift+Z (AD-13); the frame-time test still passes.

**Never:** no grid snap (story 3.2 adds it, for the node tool too); no move, delete, connect, priority or flip tools; no title star or close prompt (story 3.8); no configurable or persisted shortcuts (epic-settings-ui-shell); no overlap check on the new node (epic-scans-autosave).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Add with heightmap | node tool active, left click on the map | one node at the clicked world x/z, y = sampled height, flag REGULAR; one history entry | none |
| Add without heightmap | no heightmap loaded | node with y = -1; the existing height-warning icon shows | none |
| Sub-priority node | N pressed while the node tool is active | the tool switches to sub-priority; the next node has flag SUB_PRIO (Java cycles the add-node group) | none |
| Undo / redo | Ctrl+Z, then Ctrl+Shift+Z (or the buttons) | the node disappears, then returns under the same id | buttons disabled when nothing to undo or redo |
| Read-only config | legacy or unknown-version document | node tool, Undo and Redo disabled; a click adds nothing | History also rejects (`NetworkError::ReadOnly`) |
| New document | a load result replaces the document while a tool is active | the active tool is cancelled (none active) | none |
| Click outside the map image | click beyond ±1024·scale | node added at the clicked position, as Java does | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/mod.rs` (new) -- `Tool` trait: `on_click(&mut self, ctx: &ToolContext) -> Option<Transaction>`, `paint(&self, painter, &ViewTransform, MapGeometry, cursor)`, `cancel(&mut self)`, `name()`. `ToolContext` carries the clicked world x/z and `&MapContext`. Keep the trait minimal; stories 3.2–3.7 extend it (drag, hover).
- `crates/ad-app/src/tools/node.rs` (new) -- `NodeTool { flag }`: on click builds `Transaction::new(vec![Command::AddNodes(AddNodes::new(vec![(x, y, z, flag)]))])` with `y = sample_height(map, x, z).unwrap_or(-1.0)`; paints a preview dot at the cursor.
- `crates/ad-app/src/input.rs` -- turn into the shortcut table: an `Action` enum (NodeTool, Undo, Redo, ToggleNodeId) and one `const` list of `(Action, egui::KeyboardShortcut)` with N, Ctrl+Z, Ctrl+Shift+Z, F4; `handle_shortcuts` dispatches. Check Ctrl+Shift+Z before Ctrl+Z (egui modifier matching).
- `crates/ad-app/src/ui.rs` -- `top_bar`: toolbar (node tool toggle showing regular/sub-prio, Undo, Redo, enabled per `can_undo`/`can_redo` and not read-only); `map_panel`: pass the active tool's paint as the overlay hook and hand a primary click to the tool, then `doc.apply(tx)`; an apply error goes to `app.error`.
- `crates/ad-app/src/view.rs:182-244` -- `show` must report a primary click position (e.g. return `Option<(f64, f64)>` screen pos of `response.clicked_by(Primary)`) keep panning on primary and middle drags; do not change drawing.
- `crates/ad-app/src/main.rs` -- `App` gets `tool: Option<Box<dyn Tool>>` (or an enum); `poll_jobs` cancels the tool when a load result replaced the document (compare `doc.epoch` before/after `apply_result`).
- `crates/ad-app/src/document.rs:55-63` -- `apply` loses its `#[allow(dead_code)]`; add `undo()`/`redo()` delegating to `History` with `&mut config.network`.
- `crates/ad-core/src/geometry.rs:24` `MapGeometry::to_world`, `crates/ad-app/src/view.rs:80` `ViewTransform::screen_to_map` -- screen → map px → world.
- `crates/ad-core/src/heightmap.rs:79` `sample_height` -- reuse.
- `crates/ad-io/tests/golden_edited.rs:107-117` -- the added node's y comes from `sample_height` on a small generated heightmap `MapContext` instead of the literal 3.25.
- Do not touch: `hover.rs`, `overlay.rs`, `look.rs`, `jobs.rs` load/save logic, `ad-io` config writing.

## Tasks & Acceptance

**Execution:**
- [x] `crates/ad-app/src/tools/mod.rs`, `tools/node.rs` -- Tool trait and node tool, with unit tests for the matrix rows Add with/without heightmap and Sub-priority (tool output without a UI) -- AD-10 core of the tracer
- [x] `crates/ad-app/src/document.rs` -- `undo`/`redo`, drop the `apply` allow; tests: read-only document rejects the node tool's transaction, undo/redo keep the id -- AD-4, AD-11
- [x] `crates/ad-app/src/input.rs` -- shortcut table and dispatch; test that the table maps N, Ctrl+Z, Ctrl+Shift+Z, F4 and that Ctrl+Shift+Z resolves to Redo, not Undo -- AD-13
- [x] `crates/ad-app/src/view.rs` -- click reporting and pan-button rule -- input plumbing
- [x] `crates/ad-app/src/ui.rs`, `crates/ad-app/src/main.rs` -- toolbar, overlay, click → apply, tool cancel on new document (test via the epoch check) -- tracer wiring
- [x] `crates/ad-io/tests/golden_edited.rs` -- sampled height for the added node -- round-trip proof
- [x] record in Implementation Notes the Java mouse mapping found and the rule applied

**Acceptance Criteria:**
- Given the fixture is open with its heightmap, when the user picks the node tool, clicks the map, presses Ctrl+Z then Ctrl+Shift+Z and saves to a new file, then reopening that file shows the node at the clicked place with a terrain height.
- Given the frame-time test, when run in release, then every case stays under 16 ms with the toolbar and node-tool overlay in place.

## Design Notes

Java (MapPanel.java:908-917, EditorXML.java:69): by default the **left** button drags the map whatever tool is active; the setting MiddleMouseMove switches panning to the middle button. Tools act on a left click/release (AddNodeBaseButton.java:90-98), so a click adds a node and a drag pans. The user chose Java's mapping (Intent decision): `show` keeps panning on a primary or middle drag and reports a primary click (not a drag) to the caller.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

**Manual checks (if no CLI):**
- The user runs the acceptance flow on the fixture (hitl) and checks a legacy config shows the tools disabled, if one is at hand.

## Implementation Notes

- **Java mouse mapping found.** `MapPanel.mouseButtonPressed` starts a map drag on BUTTON1 unless `MiddleMouseMove` is set, then on BUTTON2. `AddNodeBaseButton.mouseReleased` adds a node on every BUTTON1 release while the tool is active, so in Java a node also lands at the end of a left-drag pan.
- **Rule applied.** `MapView::show` pans on a primary or middle drag. It returns the map pixel of a primary *click* (egui `clicked_by(Primary)`: press and release with no drag) and nothing for a drag or a secondary click. A pan therefore never adds a node. The release-after-drag node is a Java defect we do not port. Tests: `primary_click_reports_the_map_pixel`, `drags_pan_and_never_click`, `secondary_click_is_not_reported`.
- `show` returns the clicked **map pixel**, not the screen position. That keeps `ViewTransform` private to the view. `App::click` converts it with `MapGeometry::to_world`. The overlay hook now also gets the pointer's screen position, which `Tool::paint` uses for its preview.
- **Toolbar.** There are two selectable buttons, "Node" and "Sub-prio node" (one per flag, as Java's add-node group has), instead of a single toggle with a mode label. Clicking the active button turns the tool off. N cycles: no tool or another tool → regular, regular → sub-prio, sub-prio → regular (`tools::node::cycled`).
- **Tool identity** is `Tool::name()`, compared by the toolbar and `cycled`. No downcast and no enum.
- **Gating.** `App::editable()` (a document exists and `config.read_only()` is `None`) gates N, Undo, Redo, the tool buttons and `App::click`. History rejects as well (`NetworkError::ReadOnly`).
- **Tool cancel on a new document.** `poll_jobs` compares `doc.epoch` before and after `apply_result`, and calls `set_tool(None)` when it changed. A failed load leaves the tool active. Tests: `a_new_document_cancels_the_active_tool`, `a_failed_load_keeps_the_tool`.
- **Frame-time test.** Each timed frame now runs `ui::top_bar` with an `App` whose node tool is active, then the view with the tool's paint hook. The app has no document, so the buttons render disabled. Release run: the worst case is 2.65 ms max, at minimum zoom.
- **golden_edited.** The added node's y is `sample_height` on a generated 16×16 `MapContext` heightmap. The test asserts that the reloaded node carries that y.

## Plan Change Log

## Review Triage Log
