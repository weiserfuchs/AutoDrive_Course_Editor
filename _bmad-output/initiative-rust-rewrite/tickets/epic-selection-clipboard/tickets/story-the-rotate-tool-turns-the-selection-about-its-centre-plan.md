---
title: 'The rotate tool turns the selection about its centre'
type: 'feature'
ticket: '6'
created: '2026-11-08'
status: 'in-review'
baseline_revision: 'dd0bf9f70a14b5c7f45884b4ef979b06968cb86e'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
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

- **Handle and the view.** The handle's radius is in screen px, so the tool needs the zoom. A drag gets only world positions, and `Tool::paint` gets neither the network nor the selection. So `Tool` has a new default method `paint_selection(painter, t, map, net, selection, revision)`, which `ToolHooks::paint` calls before `paint`. Only the rotate tool implements it. Each frame it stores the view's world units per screen px (`scale / zoom`) and half the node size in px (`look::node_px / 2`) in a `Cell` (`RotateTool::see`). Then it draws a white centre dot (3 px), a circle (1 px, half alpha; Java's is dashed) and the handle (filled white, at least 4 px). `on_drag_start` tests the press against the last seen view. The circle (bounding-box centre and radius) is recomputed there from the context's network and selection, so a stale selection cannot be used. Before any frame is painted, no drag is claimed.
- **Angle.** The turn is the pointer's angle about the centre (`atan2(dx, -dz)`) minus its angle at the press, wrapped into −180..180. So a press anywhere on the handle starts at 0° and never jumps. It is rounded to 1°, or to 5° (`SNAP_STEP_DEG`) while `ctx.snap`. Positive angles turn clockwise, as Java's `AffineTransform.rotate` with z down. The preview is rebuilt only when the rounded angle changes. During a drag the circle is fixed and the handle is drawn at `turned(start, centre, angle)`.
- **Hit test.** The circle's radius is at least 24 px (`MIN_RADIUS_PX`), so with one node or a tight group the handle's 8 px hit area (`HANDLE_HIT_PX`) never covers the centre. A press within 8 px of the handle claims the drag, even on a node. Any other press is not claimed, so the view pans.
- **Commit.** The release commits one `MoveNodes` of every selected node in the network, with absolute positions and y kept, but only if some node changed. So 0°, or a turn of one node (it sits at the centre), commits nothing. Rotated nodes are not clamped to the map (as in Java).
- **Wiring.** `Action::RotateTool` (R, editable gate), the toolbar button "Rotate" after "Move".
- **Idle cache.** With the tool active and no drag, `paint_selection` drew the handle from `Circle::of` every frame: two walks over the selection with node lookups. With every node selected that cost 0.68 ms median and 1.4 ms max per frame (timed around the call, then removed). Idle frames took 5.4 ms median against 4.8 ms for "static: min zoom, all selected". Now the tool caches the selection's centre and reach, keyed on (`Document::revision`, selection length, network length). Only the radius in px is recomputed per frame. `Document::revision` is bumped by every apply, undo and redo (in `prune_selection`) and by `map_panel` after a selection gesture. `ToolHooks.revision` hands it to `paint_selection`, which gained a `revision` parameter. `on_drag_start` still recomputes the circle fresh.
- **Tests.** rotate.rs has tests for the helpers, the handle position, Quarter turn, Whole degrees and Snap steps, preview and handle following the angle, No turn (0°, 0.4°, 2° with snap), Undo, Empty selection, Off-handle press (8 px boundary, on a selected node), no view seen, and cancel. It also has a single node: the handle sits 24 px above it, a press on the node is not claimed, and a 90° turn commits nothing. A press 6 px beside the handle starts at 0°, follows the turn relative to the press, and wraps past ±180°. The idle cache is checked too: a moved node is ignored until the revision changes, and a change of the selection length takes effect at once. main.rs has a rotation through `ToolHooks` that undoes in one step, and the read-only gate. golden_edited turns three untouched fixture nodes 37° about their bounding-box centre at full precision (the same formula as `turned`; ad-io cannot depend on the ad-app binary). It checks that they reload within the writer's rounding (|diff| ≤ 0.0005, y exact). Every other node must reload exactly.
- **Frame time (release, this machine, 3 runs after the review fixes, best round).** "rotate: min zoom, whole network selected": median 6.3–6.6 ms, max 6.8–9.8 ms. "rotate: max zoom, whole network selected": median 0.57–0.63 ms, max 1.07–1.22 ms. "rotate tool idle: min zoom, whole network selected": median 4.9–5.2 ms, max 5.7–8.8 ms, against "static: min zoom, all selected" at median 4.8–5.0 ms and max 5.4–5.8 ms. Every case is under 16 ms. `RotateTool::on_drag` with every in-map node selected took 0.21 ms median and 0.33 ms max per event (timed around the call in the test, then removed).
- **Rotate preview budget (coordinator follow-up, deferred from story 4.3).** On the developer's machine under load, "rotate: min zoom, whole network selected" gave best-round max 12.4 and 16.6 ms in 2 runs. Target: best-round max ≤ 12 ms over repeated runs, with nothing drawn differently.
  - **Before (release, this machine, 3 runs, best round).** Rotate min zoom: median 6.1–7.5 ms, max 8.6–9.4 ms. Rotate max zoom: median 0.59–0.93 ms, max 1.2–1.7 ms. Whole-network drag min zoom: median 6.1–6.7 ms.
  - **Where it went** (timers in `paint`, then removed; ms per rotate frame at min zoom). Walk over the preview for the stored box and shift: 0.8–1.0 (one random `net.node` lookup per previewed node). Edge query: 0.8–1.25 (37,000–55,291 entries; the joint whole-map box at most angles). Edge loop: 1.8–2.35. Selection marks: 1.0–1.1. Tessellation: 1.9. Total 6.4–7.7.
  - **Measured and dropped: adjacency.** I timed walking each previewed node's `outgoing()` with `pv.get`/`net.node` for the far end and `net.link_look` for the colour, with the cull and no mesh building. That alone took 3.3–4.0 ms a frame, against about 3 ms for query plus loop including the mesh. Adjacency is also incomplete: a reverse link from an unpreviewed node is listed by neither end, so only the edge index finds it.
  - **Measured and dropped: a dense array.** I tried an id-indexed array of places filled during the walk, read instead of hash probes. The edge loop barely changed (1.8 ms) and the walk grew to 1.0 ms, so there was no net gain.
  - **Fix.** `Preview` is now a struct around the places map (read through `Deref`). `Preview::place_all(id, stored, placed)` fills it and keeps `PreviewBounds`: the stored box and the per-axis shift. The bounds are dropped by `insert` or when `place_all` does not cover every entry. The view uses them and skips the walk; without them (previews built by `collect`, as in the view tests) it walks as before. The move and rotate tools fill their previews with `place_all` from their `(id, before)` lists, a sequential pass over data they already iterate. Their `on_drag` with the whole network went from median 0.27 to 0.40 ms (rotate, timed around the call, then removed). That runs inside the app's frame but not inside the test's timed frame.
  - **Test.** `a_preview_draws_exactly_what_the_moved_network_draws` gains a fourth preview, every node turned 30° about (0, 0). Every preview is now drawn twice: with view-computed bounds (plus the ghost id) and with `place_all` bounds. Both must equal the network with the nodes really moved. Zeroing the handed-over shift makes it fail (checked, then restored).
  - **After (release, this machine, 5 runs, best round; the machine was busier than for the before runs: static min zoom 2.3–2.8 ms against 2.3–2.5).** Rotate min zoom: median 5.8–7.0 ms, max 6.6–7.7 ms. Rotate max zoom: median 0.24–0.29 ms, max 0.68–1.14 ms. Whole-network drag: min zoom median 6.1–6.7 ms, max 6.7–7.0 ms; max zoom median 0.18–0.26 ms (was 0.48–0.88). Rotate tool idle: median 5.4–5.8 ms. Single-node drags unchanged within noise (min zoom 2.6–3.1 ms median, max zoom 0.08–0.09 ms). No case over budget.
  - **A/B run, interleaved: 3 rounds of baseline, then new, under the same load.** Rotate min zoom: baseline median 6.2–7.7 ms, max 8.8–9.1 ms; new median 6.6–6.9 ms, max 7.6–10.2 ms. Whole-network drag min zoom: baseline median 7.1–7.6 ms; new 6.5–7.0 ms. Single-node drag min zoom: baseline 2.8–3.2 ms; new 2.9–3.4 ms (noise).
  - **Left.** The rest of the rotate frame is the joint query over up to 55,000 entries with two lookups each, about 1 ms more than the idle frame. A tighter query would need the preview to carry its transform (the view turned back about the pivot), which the general `Preview` does not.

## Plan Change Log

## Review Triage Log

- Coordinator review, pass 1: 5 findings, all patched.
  - A turn that moves no node committed an empty-effect `MoveNodes`; the release now commits only if some node changed.
  - With one node or a tight group the handle covered the centre, and the absolute angle jumped on the first move. The radius is now at least 24 px, and the angle is measured from the press.
  - The idle `paint_selection` cost was unmeasured: 0.68 ms per frame with the whole network selected. A frame-time case was added, and the centre and reach are now cached per document revision.
  - golden_edited pre-rounded the rotated values. It now applies full precision and checks the reload within ±0.0005.
  - golden_edited listed `group` twice in the delete section's `busy`; the duplicate was dropped.
