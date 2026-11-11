---
title: 'Buildings from placeables.xml show as squares coloured by owner'
type: 'feature'
ticket: '1'
created: '2026-11-11'
status: done
baseline_revision: 'b361bb3'
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

**Problem:** the user wants to see built buildings on the map (2026-11-11). The savegame's `placeables.xml` names each building's position, rotation, XML file and owner.

**Approach:**
- **ad-io `placeables`:** `load(bytes) -> Vec<Placeable>` reads each top-level `<placeable>` with a `position`:
  - name from the file stem, else the unique id;
  - unique id, mod name, file, x/z, rotation y, farm id;
  - preplaced entries without a position are skipped;
  - another root or broken XML is an error.
- **Load job:** the `placeables.xml` beside a config is read like `vehicles.xml`, never written. A missing file gives none; a broken file gives a notice and none. Routes have none.
- **View:**
  - a 10 px square per building in view, under the network, in the owner's colour:
    - map (farm 0) light grey;
    - own farm (1) magenta;
    - other farms violet;
    - none of these clash with a node or connection colour.
  - Names show from zoom 2.
  - The hovered square, when no node or connection is hovered, shows "name ( id, owner )".
- **Display ▸ Placeables (F5):** configurable, persisted as `show_placeables` (default on).
- **Fixture:** `placeables.xml` is committed as an FS25 fixture (`-text`, AGENTS.md protection) and drawn in the frame-time test.
- Rotation is not drawn yet (story 2 checks its sign against real footprints).

## Boundaries & Constraints

**Never:** no write of `placeables.xml`; the repo-root fixtures are never overwritten.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Parse | fixture | 117 buildings, 61 of farm 1, TankeNord = gasStation01 | none |
| Skip | preplaced without position, "1 2" position | skipped | none |
| Bad file | `<vehicles/>`, unclosed | error | notice, config opens |
| Save | save over the config | `placeables.xml` unchanged | none |
| Draw | zoom 1 / zoom 10 | squares by owner / plus names | none |
| Hover | pointer on a square / on a node over a square | name with id and owner / the node | none |
| Toggle | F5 off | nothing drawn or hovered; persisted | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/placeables.rs` (new), `lib.rs`; `settings.rs` -- `show_placeables`.
- `crates/ad-app/src/jobs.rs` -- `Loaded` gains the placeables, `load_placeables`, tests.
- `crates/ad-app/src/document.rs` -- `Document::placeables`.
- `crates/ad-app/src/view.rs` -- `ViewHooks::placeables`, `show_placeables`, `hovered_placeable`, `placeable_screen`, `placeable_at`, `paint_placeables`; test; frame-time test draws the fixture.
- `crates/ad-app/src/look.rs` -- owner colours.
- `crates/ad-app/src/input.rs`, `main.rs`, `ui.rs` -- `TogglePlaceables` (F5), the Display item, persistence.
- `placeables.xml`, `.gitattributes`, `AGENTS.md` -- the fixture.

## Implementation Notes

- **Startup:** the user approved the approach (symbol first, footprint later, all buildings by owner, the fixture).
- **Owners:** farm 1 is "own" as in a single-player game; in multiplayer it is the first farm.
- **Verification:**
  - fmt ok; clippy ok;
  - `cargo test --workspace` all pass (ad-app 458 + 4 ignored, ad-io 97);
  - frame time: every case ok with the fixture's buildings drawn, e.g. static min zoom max 4.1 ms.

## Verification

**Commands:**
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` -- expected: pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked:
  - `hovered_placeable` is an index into the hooks' list and is recomputed every frame, so a new document cannot show a stale one;
  - it is also cleared by `clear_hover`;
  - the label of a hovered building is drawn even below zoom 2.
