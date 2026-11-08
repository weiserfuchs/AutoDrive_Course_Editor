---
title: 'A lasso selects the nodes inside a drawn outline'
type: 'feature'
ticket: '4'
created: '2026-11-08'
status: 'built'
baseline_revision: '6efe7efd0a7212d6932105e2400ef0d5f366042d'
route: 'full'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/initiative-rust-rewrite/tickets/epic-selection-clipboard/tickets/story-a-right-drag-rectangle-selects-nodes-and-the-selection-survi-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Only a rectangle can select by area; curved roads and junctions need a free outline, as Java's "Freeform" selection mode offers.

**Approach:** A toolbar switch picks rectangle or lasso for the right drag (session only, rectangle by default, as Java's MultiSelectButton). In lasso mode the right drag records the pointer path; while dragging it is drawn as a white dashed outline with a faint closing line back to the start (Java). On release the outline, converted to world coordinates, is queried with `Network::in_polygon` (even-odd rule, AD-5) and every node inside is toggled, like the rectangle. A right click behaves as in story 4.1 in both modes.

## Boundaries & Constraints

**Always:** the selection rules of story 4.1 (toggle, clear on an empty right click, works with any tool and read-only); the network never changes; the frame-time test still passes.

**Never:** no persisted mode setting (epic-settings-ui-shell); no change to the rectangle behaviour.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Lasso select | lasso mode, right drag around a curved group | the nodes inside the outline toggle | none |
| Concave outline | a C-shaped outline with a node in the notch | the notch node is not selected | none |
| Degenerate | an outline of fewer than 3 distinct points | nothing toggles | none |
| Switch | toolbar switch to lasso and back | the next right drag uses the chosen shape | none |
| Click | right click on a node / on empty map, either mode | toggle / clear (story 4.1) | none |
| Zoom during drag | wheel while drawing the lasso | recorded points stay on the same map points | none |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/view.rs` -- `SelectEvent` (~156) gains `Lasso { points: Vec<MapPx> }`; the secondary-drag handling (~380-405) records map-px points while dragging in lasso mode (append only when the pointer moved at least ~2 screen px, to keep the list short), draws the dashed outline and the faint closing segment, and reports `Lasso` on release. The view needs the mode: pass it in through `ViewHooks` (e.g. `fn lasso(&self) -> bool { false }`) to keep `show`'s signature.
- `crates/ad-app/src/ui.rs` `select` (~165) -- `Lasso` converts each point with `geometry.to_world` and toggles `net.in_polygon(&pts)`; the toolbar gets a two-state switch "Rect"/"Lasso" (selectable labels) bound to `App.lasso: bool`; `ToolHooks` answers `lasso()`.
- `crates/ad-core/src/network/spatial.rs:177` `in_polygon` -- reuse (even-odd, sorted, empty below 3 points).
- Do not touch: the rectangle path, tools, `ad-core`.

## Tasks & Acceptance

**Execution:**
- [x] `view.rs` -- lasso recording, drawing, event; tests: a right drag in lasso mode reports the recorded points (map px) and no Rect, points survive a zoom step, the outline is drawn only while dragging
- [x] `ui.rs` -- `select` for `Lasso`, the switch, `App.lasso`; tests for Lasso select, Concave outline, Degenerate, Switch, Click in lasso mode

**Acceptance Criteria:**
- Given the fixture, when the user switches to lasso and draws around a curved road, then exactly the enclosed nodes toggle.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- `SelectEvent` lost `Copy` (it now holds a `Vec`); nothing relied on it.
- The mode is read once, at the drag's start, through `ViewHooks::lasso()`; `ToolHooks` carries it as a `pub lasso` field set by `map_panel` from `App.lasso`, so `ToolHooks::new` keeps its signature. A switch mid-drag affects only the next drag.
- `MapView.lasso: Vec<MapPx>` holds the path (empty while no lasso drags). The press origin is the first point; a point is added while dragging, and on release, once the pointer is at least `LASSO_STEP_PX` (2) screen px from the last point, measured with the current transform. The event takes the path; `reset` clears it.
- Drawing: the path plus the pointer as a 1 px white dashed line (the rectangle's 4 px dash/gap), and the closing line pointer -> start dashed in white at 0.25 opacity (Java's `AlphaComposite 0.25`).
- The toolbar switch is two selectable labels "Rect" / "Lasso" after "Snap", always enabled, since selection works on a read-only document.
- The Switch case is tested by setting `App.lasso` and driving `map_panel`, not by clicking the toolbar labels.
- Frame time (release, this machine): every case ok; worst best-round max 7.2 ms ("drag: min zoom, whole network selected").

## Plan Change Log

## Review Triage Log

- Quick review, pass 1: 3 low patched; 1 low and 1 false rejected.
  - low, patch: a long lasso made the release-frame query cost nodes-in-box × path length (unbounded). The outline is thinned evenly to at most `LASSO_MAX_POINTS` (256) for the query.
  - low, patch: the `select` doc line ran past the file's wrap width; rewrapped.
  - low, patch: the `SelectEvent::Lasso` doc promised the release point is always included; it now says the path is sampled every few px.
  - low, reject: the toolbar switch is untested (the test sets `App.lasso`); two selectable labels bound to one bool, covered by the user's hand check.
  - false: "the fixture criterion has no evidence". It is the user's hand check.
