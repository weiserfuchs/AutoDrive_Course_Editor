---
title: 'Buildings with indoor areas show the building filled and the base plate as a thin line'
type: 'feature'
ticket: '4'
created: '2026-11-12'
status: done
baseline_revision: '72984b4'
route: 'full'
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

**Problem:** On many buildings the clear area takes in a base plate much larger than the building, so the map shows the building far too big (the user's hand check after story 2).

**Approach (the user chose "two layers"):**
- **ad-io:** `Placeable::footprint` becomes a `Footprint { outline, indoor }`.
  - The outline is chosen as before: clear areas, else level areas, else test areas.
  - The indoor areas (`indoorArea` start/width/height, the building under its roof) are read beside the outline and placed in the same way.
- **View:**
  - With indoor areas, those are filled (30 %) with the 1.5 px line in the owner's colour. The outline is only a 1 px line at 60 %.
  - Without indoor areas, nothing changes.
  - Hover works on both. The name sits in the middle of the indoor areas.
  - The square still stands in while everything is under 10 px.

## Boundaries & Constraints

**Never:**
- no tip occlusion areas: measured over the fixture's mods, they are often copied template values (1109 m² on many productions, 152 m² on a 1849 m² greenhouse);
- no geometry from the binary `.i3d.shapes`.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Areas | clear + indoor / indoor only / none | both / indoor only, still a footprint / empty | none |
| View | plate with a building inside, zoom 10 | only the building filled; plate thin line; hover on the plate | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/placeables.rs` -- `Footprint { outline, indoor }`, `is_empty`.
- `crates/ad-io/src/footprint.rs` -- `indoorArea` parsing, `local_footprint` and `footprints` return `Footprint`; tests.
- `crates/ad-app/src/view.rs` -- `PlaceableShape::Footprint { outline, indoor, bounds }`, `name_at`, two-layer painting; test.
- `crates/ad-app/src/jobs.rs` -- test reads `footprint.outline`.

## Implementation Notes

- **Measured on the fixture's mods (m², outline / indoor):**

  | Building | Outline | Indoor |
  |---|---|---|
  | Zentrallager_Bauplatz | 36 408 | 463 |
  | Kaeserei | 6 960 | 1 034 |
  | Molkerei | 6 231 | 1 581 |
  | Strom_Halle_XL | 1 674 | 1 250 |
  | Glasfabrik | 8 025 | 228 |

  Glasfabrik's indoor area is one small room, so its building looks too small; its plate line still shows the whole site.
- **Result:** 74 of 117 buildings have a footprint, 36 of them with indoor areas. sellingStation_Baumarkt has indoor areas only and now gets a footprint.
- **Verification:**
  - fmt ok; clippy ok;
  - `cargo test --workspace` all pass (ad-app 462 + 4 ignored, ad-io 104 + 1 ignored);
  - real mods read in 353 ms;
  - frame time: every case ok.

## Verification

**Commands:**
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` -- expected: pass
- `cargo test -p ad-io --release -- --ignored real_mods_footprints --nocapture` -- expected: footprints with indoor areas
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked:
  - a building with indoor areas only is not empty and is drawn filled;
  - bounds take in both layers, so culling and the square threshold see the whole site;
  - an indoor area whose i3d is missing errors like the outline does, so the building keeps its square.
