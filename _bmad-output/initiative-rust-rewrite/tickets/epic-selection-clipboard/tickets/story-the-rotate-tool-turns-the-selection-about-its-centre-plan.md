---
title: 'The rotate tool turns the selection about its centre'
type: 'feature'
ticket: '6'
created: '2026-11-08'
status: 'in-progress'
baseline_revision: 'dd0bf9f70a14b5c7f45884b4ef979b06968cb86e'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: ''
review_source: ''
lenses_ran: []
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/initiative-rust-rewrite/tickets/epic-selection-clipboard/tickets/story-the-move-tool-drags-the-whole-selection-in-one-undo-step-plan.md'
  - '{project-root}/_bmad-output/initiative-rust-rewrite/tickets/epic-selection-clipboard/tickets/story-dragging-the-whole-network-selected-stays-within-the-frame-b-plan.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Users cannot rotate a selection (R4 part moved to this epic). Java's rotate tool (R) turns the selection about its centre with a handle.

**Approach:** As Java (`RotationButton`, `RotationUtils`): the rotate tool (R, toolbar "Rotate") shows, for a non-empty selection, a small dot at the centre of the selection's bounding box and a circle through a handle placed straight above the centre at the radius of the furthest selected node plus half a node size (screen px). Dragging the handle (a press within a few px of it) rotates the selection: the angle is the pointer's angle about the centre relative to the handle's start, rounded to whole degrees, or to 5° steps while grid snap applies (Java's default rotation step). The rotation is previewed through the view's preview set (story 4.2) and the handle follows the angle; release commits one `MoveNodes` with absolute positions (y kept); release at 0° commits nothing. An empty selection shows nothing and does nothing. A frame-time case rotates the whole network inside the map at minimum and maximum zoom (R4).

## Boundaries & Constraints

**Always:** the network changes only on release through `Document::apply`; Undo/Redo and tool switches cancel a running rotation (existing path); a press not on the handle behaves as today (left drag pans); positions stay f64 (rounded only at write, AD-7; Java rounds to 3 decimals per step, not ported); the frame-time test stays under 16 ms.

**Never:** no configurable rotation step (epic-settings-ui-shell); no rotation about another pivot; no change to selection input.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Quarter turn | handle dragged 90° | every selected node at its position turned 90° about the bounding-box centre; one MoveNodes | none |
| Whole degrees | pointer at 37.4° | preview and commit at 37° | none |
| Snap steps | grid snap applies, pointer at 37.4° | 35° | none |
| No turn | release at 0° | nothing committed | none |
| Undo | Ctrl+Z after a rotation | exact start positions | none |
| Empty selection | rotate tool, nothing selected | no handle drawn; drags pan | none |
| Off-handle press | press away from the handle | no rotation; left drag pans | none |
| Read-only | legacy config | tool disabled | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-app/src/tools/rotate.rs` (new) -- `RotateTool`: pure helpers `centre(net, selection)` (bounding box centre), `turned((x, z), centre, degrees)`, `snap_angle(deg, snap) -> f64` (round to 1° or 5°); `on_drag_start` claims only a press near the handle (handle position needs the view transform: compute the handle in world units from the screen radius, or let `paint` cache it per frame); `on_drag` updates a `Preview` map (story 4.2 type) for all selected nodes; `on_drag_end` commits one `MoveNodes`; `paint` draws centre dot, circle and handle (white, as Java's control node ring).
- `crates/ad-app/src/tools/mod.rs`, `input.rs` (R), `main.rs` (`Action::RotateTool`, editable gate), `ui.rs` (toolbar "Rotate") -- wiring as the other tools.
- `crates/ad-app/src/view.rs` -- preview set already supports arbitrary per-node positions (story 4.2/4.3); the press node offered at drag start may be none (handle is not a node), so the tool claims on position.
- `crates/ad-app/src/view.rs` frame_time -- add "rotate: min zoom, whole network selected" and "rotate: max zoom, whole network selected" (same in-map set as story 4.3, angle sweeping a few degrees per frame), asserting every selected node is previewed.
- `crates/ad-io/tests/golden_edited.rs` -- one rotated group (a `MoveNodes` from `turned`) round-trips.
- Do not touch: move tool, selection input, `ad-core`.

## Tasks & Acceptance

**Execution:**
- [x] `tools/rotate.rs` -- tool and helpers; tests for Quarter turn, Whole degrees, Snap steps, No turn, Undo, Empty selection, Off-handle press
- [x] wiring (`mod.rs`, `input.rs`, `main.rs`, `ui.rs`) -- R and toolbar; read-only test
- [x] frame-time cases for rotation
- [x] `golden_edited.rs` -- rotated group round-trip

**Acceptance Criteria:**
- Given the fixture with a selected junction, when the user drags the rotate handle and releases, then the junction turns about its centre in whole degrees (5° with snap) and one Ctrl+Z restores it.
- Given the frame-time test, when run in release, then the rotate cases stay under 16 ms.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms, rotate cases included

**Manual checks (if no CLI):**
- The user rotates a fixture junction with and without snap and undoes it.

## Implementation Notes

- **Handle and the view.** The handle's radius is in screen px, so the tool needs the zoom. A drag gets only world positions, and `Tool::paint` gets neither the network nor the selection. So `Tool` has a new default method `paint_selection(painter, t, map, net, selection)`, which `ToolHooks::paint` calls before `paint`. Only the rotate tool implements it. Each frame it stores the view's world units per screen px (`scale / zoom`) and half the node size in px (`look::node_px / 2`) in a `Cell` (`RotateTool::see`). Then it draws a white centre dot (3 px), a circle (1 px, half alpha; Java's is dashed) and the handle (filled white, at least 4 px). `on_drag_start` tests the press against the last seen view. The circle (bounding-box centre and radius) is recomputed there from the context's network and selection, so a stale selection cannot be used. Before any frame is painted, no drag is claimed.
- **Angle.** It is the pointer's angle about the centre, clockwise on screen from straight above (`atan2(dx, -dz)`, −180..180). It is rounded to 1°, or to 5° (`SNAP_STEP_DEG`) while `ctx.snap`. Positive angles turn clockwise, as Java's `AffineTransform.rotate` with z down. The preview is rebuilt only when the rounded angle changes. During a drag the circle is fixed and the handle is drawn at `turned(start, centre, angle)`.
- **Hit test.** A press within 8 px (`HANDLE_HIT_PX`) of the handle claims the drag, even on a node. Any other press is not claimed, so the view pans.
- **Commit.** The release commits one `MoveNodes` of every selected node in the network, with absolute positions and y kept. A release at 0° commits nothing. Rotated nodes are not clamped to the map (as in Java).
- **Wiring.** `Action::RotateTool` (R, editable gate), the toolbar button "Rotate" after "Move".
- **Tests.** rotate.rs has tests for the helpers, the handle position, Quarter turn, Whole degrees and Snap steps, preview and handle following the angle, No turn (0°, 0.4°, 2° with snap), Undo, Empty selection, Off-handle press (8 px boundary, on a selected node), no view seen, and cancel. main.rs has a rotation through `ToolHooks` that undoes in one step, and the read-only gate. golden_edited turns three untouched fixture nodes 37° about their bounding-box centre (the same formula as `turned`; ad-io cannot depend on the ad-app binary). Each result is taken as the writer's three decimals read back, so the exact model comparison after reload holds.
- **Frame time (release, this machine, one run, best round).** "rotate: min zoom, whole network selected": median 6.6 ms, max 10.1 ms. "rotate: max zoom, whole network selected": median 0.61 ms, max 1.08 ms. Every other case is unchanged and under 16 ms. `RotateTool::on_drag` with every in-map node selected took 0.21 ms median and 0.33 ms max per event (timed around the call in the test, then removed). Not measured: `paint_selection` with the rotate tool active, a large selection and no drag. It walks the selection twice per frame for the centre and radius.

## Plan Change Log

## Review Triage Log
