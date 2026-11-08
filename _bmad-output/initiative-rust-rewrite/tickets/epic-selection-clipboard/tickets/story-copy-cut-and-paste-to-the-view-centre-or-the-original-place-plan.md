---
title: 'Copy, cut and paste to the view centre or the original place'
type: 'feature'
ticket: '9'
created: '2026-11-08'
status: 'planned'
baseline_revision: '56f7bea'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Users cannot duplicate or move parts of a network between places or configs. Java's `CopyPasteManager`/`SnapShot` copies the selection and pastes it at the view centre (Ctrl+V) or the original place (Shift+V).

**Approach:**
- **Copy (Ctrl+C):** with a non-empty selection, store an app-level clipboard of detached data (AD-15): per node its position relative to the selection centre (centre of the selection's x/z bounding box, as Java), y, flag and marker (name, group); the centre itself; and every link whose both ends are selected, by node index, with its shape (one-way, dual, reverse). Links to outside nodes are dropped. Unlike Java, copy keeps the selection.
- **Cut (Ctrl+X):** copy, then one `DeleteNodes` of the selection.
- **Paste (Ctrl+V / Shift+V):** one transaction creating the nodes, their markers and inner links. Ctrl+V centres the group on the view centre, Shift+V uses the stored centre (original place). x and z are rounded half-up to 3 decimals (as Java); y comes from `sample_height`, or the copied y when the map has no heightmap. Markers keep their group; a marker name already used in the network gets the first free suffix `_2`, `_3`, … (unlike Java, which duplicates the name; user decision), checked against existing markers and the ones of the same paste. A name that is free (e.g. after a cut) is kept. Paste can repeat from the same clipboard.
- **Selection:** the pasted nodes become the selection. Undo of a paste or a cut restores the selection from before it; redo restores the selection after it.
- The clipboard survives opening another file, so a group can be pasted into another config. Empty clipboard or empty selection: nothing.

## Boundaries & Constraints

**Always:** one transaction per cut and per paste through `Document::apply`; read-only disables cut and paste (copy still works, it changes no data); shortcuts are blocked while a modal dialog is shown (existing rule) and while a rectangle or lasso is drawn; an active tool preview is cancelled before cut or paste.

**Never:** no OS clipboard format for the network data (app-level only); no paste of links to nodes outside the group; no renaming beyond the `_n` suffix; no overlap check (epic-scans-autosave).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Copy and paste | linked group with a marker and a link to an outside node, Ctrl+C, Ctrl+V | new nodes centred on the view, same flags, marker copied, inner links with their shapes, no outside link; pasted nodes selected | none |
| Original place | Shift+V | new nodes at the copied x/z | none |
| Heights | map with heightmap / without | y = sample_height at the new x/z / copied y | none |
| Repeat | Ctrl+V twice | two independent copies | none |
| Marker names | copy marker "Feld", paste twice; cut "Hof", paste | "Feld_2", then "Feld_3"; "Hof" kept | none |
| Cut | Ctrl+X on a group | group deleted in one step; clipboard holds it; one undo restores nodes, links, markers and the selection | none |
| Undo paste | Ctrl+V, Ctrl+Z | pasted nodes gone, prior selection restored; redo re-pastes with the same ids and selects them | none |
| Other file | copy, open another config, Ctrl+V | group pasted into the new config | none |
| Nothing | empty selection on copy/cut, empty clipboard on paste | nothing | none |
| Read-only | legacy config | cut and paste do nothing; copy works | existing gates |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/command.rs` -- new `PasteNodes` command: nodes `(x, y, z, flag)`, markers `(index, name, group)`, links `(from, to, ConnectionShape)` by index. Apply adds the nodes (redo reuses the first ids, as `AddNodes`), then sets markers and links; revert undoes in reverse. Prefer composing the existing `AddNodes`, `EditMarker` and `Connect` internally. `ids()` after the first apply.
- `crates/ad-core/src/history.rs` -- a small accessor for the entry an undo or redo just acted on (e.g. the `EntryId` at the cursor), so the document can map entries to selections.
- `crates/ad-app/src/document.rs` -- `apply_selecting(tx, after)`-style path: stores, per history entry, the selection before and after; `undo`/`redo` restore them for those entries, then prune as usual. Entries evicted from the history may keep their record (bounded by the cap) or be dropped; keep it simple.
- `crates/ad-app/src/clipboard.rs` (new) -- detached `Clipboard` type; pure `copy(net, selection) -> Option<Clipboard>` and `paste(clip, centre: Option<(f64, f64)>, map) -> Transaction`.
- `crates/ad-app/src/input.rs`, `main.rs` -- actions Copy, Cut, Paste, PasteInPlace; `App.clipboard` outside the document. Note: eframe turns Ctrl+C/X/V into `egui::Event::Copy/Cut/Paste` instead of key events, and emits `Paste` only when the OS clipboard holds text; verify in the egui-winit source and handle it (e.g. copy also puts a short text on the OS clipboard).
- `crates/ad-io/tests/golden_edited.rs` -- a pasted group (with an inner link and a marker) saves and reloads.
- Do not touch: view, selection input.

## Tasks & Acceptance

**Execution:**
- [ ] `PasteNodes` -- apply, revert, redo with the same ids, markers and links; tests in ad-core
- [ ] history accessor and document selection records -- tests: undo of paste restores the prior selection, redo selects the pasted nodes, undo of cut restores the cut selection
- [ ] `clipboard.rs` -- copy and paste; tests for every matrix row up to Repeat, inner links only, shapes kept
- [ ] wiring -- shortcuts (with the egui event caveat), gates; tests for Cut, Other file, Nothing, Read-only, blocked while drawing a selection
- [ ] `golden_edited.rs` -- pasted group round-trip

**Acceptance Criteria:**
- Given the fixture, when the user selects a junction, presses Ctrl+C and pastes it twice with Ctrl+V, then two linked copies appear, the last one selected, and Ctrl+Z twice removes them.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

## Plan Change Log

## Review Triage Log
