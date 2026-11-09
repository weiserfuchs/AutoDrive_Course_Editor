---
title: 'A settings window edits the editor, autosave, curve and connection settings and the theme'
type: 'feature'
ticket: '4'
created: '2026-11-09'
status: 'built'
baseline_revision: '2e84e04'
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

**Problem:** Every setting in `settings.toml` can only be changed by editing the file, and the Java config values the inventory lists for the Editor, AutoSave, Curves and Connections tabs are partly fixed constants (`lod::MAX_ZOOM`, `bezier::MAX_POINTS`, `DEFAULT_POINTS`, `MOVE_SCALER`, the curve panel's default priority and type). The theme cannot be chosen.

**Approach:**
- Edit ▸ Settings… opens a modal window with four tabs:
  - **Editor:** online map images, max zoom (1–250, Java default 30), and the theme (system, light or dark; user decision).
  - **AutoSave:** enabled, interval (1–60 minutes), number of files (1–10).
  - **Curves:**
    - max interpolation points (2–50, default 50);
    - default interpolation points (1 to the max, default 6);
    - default priority (Regular or SubPrio) and default type (Regular, Dual or Reverse);
    - control point movement (1–10, default 3).
  - **Connections:** create end node, line node spacing (1–50 m).
- Changes apply at once and persist through story 1. Shortcuts wait while the window is shown, as for the other dialogs.
- New Bézier curves and arcs start with the curve defaults, and Bézier handle drags use the movement factor. The curve panel's slider stops at the max points. The map view's zoom stops at the max zoom; a view zoomed in further zooms out to it.
- The user started this story while away; the plan was not approved separately, and the hand check is pending.

## Boundaries & Constraints

**Never:** no colour settings (user decision: fixed); no hidden transparency (story 7); no key bindings tab (story 5).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Tabs | window on each tab | its settings shown | none |
| Curve defaults | 20 points with max 8, SubPrio, Reverse, scaler 40 | new curves: 8 points, SubPrio, Reverse; scaler 10 | clamped |
| Drag scale | scaler 5, handle moved (1, 2) | control point moved (5, 10) | none |
| Arc | arc tool with defaults | new arc has them | none |
| Max zoom | 500 / 5 with the view zoomed further | 250 / view zooms out to 5 | clamped |
| Theme | dark, then light | egui theme preference follows | none |
| Default above max | max 4, default 9 on the Curves tab | default 4 | clamped |
| Escape | window shown | closed | none |

</frozen-after-approval>

## Code Map

- `crates/ad-io/src/settings.rs` -- `max_zoom`, `theme` (`Theme`), `curve_max_points`, `curve_default_points`, `curve_default_subprio`, `curve_default_kind` (`CurveKind`), `control_point_scaler`, the ranges.
- `crates/ad-app/src/tools/bezier.rs` -- `BezierTool::with_defaults`, `Curve::scaler`; test.
- `crates/ad-app/src/tools/arc.rs` -- `ArcTool::with_defaults`; test.
- `crates/ad-app/src/view.rs` -- `ViewTransform::set_max_zoom`, `MapView::max_zoom`.
- `crates/ad-app/src/main.rs` -- `apply_settings` (max zoom, theme), `curve_defaults`, `curve_scaler`, the settings window state; tests.
- `crates/ad-app/src/ui.rs` -- Edit ▸ Settings…, `SettingsTab`, `settings_window`, the curve panel's max; test.

## Tasks & Acceptance

**Execution:**
- [x] settings and ranges
- [x] tools, view and app -- tests Curve defaults, Drag scale, Arc, Max zoom, Theme
- [x] window -- tests Tabs, Default above max, Escape

**Acceptance Criteria:**
- Given the settings window, when the user changes a setting, then it acts at once and is kept after a restart.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Not ported from the Java Editor and Connections tabs:**
  - "Middle Mouse Move": the Rust view pans with both buttons.
  - "Interpolate Zoom": Rust zooms in steps.
  - "Display Solid Arrows": Rust always fills arrowheads.
  - These are recorded for the sweep's inventory check. Hidden transparency comes with story 7.
- The Java max zoom is also per map; here it is global.
- Verification: fmt ok; clippy ok; `cargo test --workspace` all pass (ad-app 441 + 4 ignored); frame time ok.

## Plan Change Log

## Review Triage Log

- Quick review (orchestrator, inline): no finding. Checked: the tool bar's Bézier and Arc buttons and the B key all build the tools with the defaults; a curve already previewed keeps its own settings.
