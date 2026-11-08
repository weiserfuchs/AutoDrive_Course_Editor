---
title: 'Copy, cut and paste to the view centre or the original place'
type: 'feature'
ticket: '9'
created: '2026-11-08'
status: done
baseline_revision: '56f7bea'
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
- [x] `PasteNodes` -- apply, revert, redo with the same ids, markers and links; tests in ad-core
- [x] history accessor and document selection records -- tests: undo of paste restores the prior selection, redo selects the pasted nodes, undo of cut restores the cut selection
- [x] `clipboard.rs` -- copy and paste; tests for every matrix row up to Repeat, inner links only, shapes kept
- [x] wiring -- shortcuts (with the egui event caveat), gates; tests for Cut, Other file, Nothing, Read-only, blocked while drawing a selection
- [x] `golden_edited.rs` -- pasted group round-trip

**Acceptance Criteria:**
- Given the fixture, when the user selects a junction, presses Ctrl+C and pastes it twice with Ctrl+V, then two linked copies appear, the last one selected, and Ctrl+Z twice removes them.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and `golden_edited`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **egui clipboard events (checked in egui-winit 0.37.0 `src/lib.rs`).** With `clipboard_shortcuts` on (eframe default), a key press that `is_cut_command`/`is_copy_command`/`is_paste_command` matches is replaced by `Event::Cut`/`Event::Copy`/`Event::Paste(text)` and no `Event::Key` is sent. Matched: Cmd/Ctrl alone (`command_only`) + X/C/V, the dedicated Cut/Copy/Paste keys, and on Windows Shift+Delete (cut), Ctrl+Insert (copy), Shift+Insert (paste). `Paste` is pushed only when the OS clipboard holds non-empty text; with no text but an image it pushes `PasteImage`; otherwise nothing. Ctrl+Shift+V and Shift+V stay key events.
- **Handling.** `input::pressed` consumes, after the key table, `Event::Copy` -> Copy, `Event::Cut` -> Cut, `Event::Paste`/`PasteImage` -> Paste, so the Windows aliases work too. The key forms Ctrl+C/X/V stay in the table for a backend that passes keys through (and the tests). Copy and cut set `App.copied`, and `handle_shortcuts` puts that note ("AutoDrive Course Editor: N nodes copied") on the OS clipboard with `ctx.copy_text`, so a later Ctrl+V arrives as a paste event. The existing modal gate returns before anything is consumed, so the Edit location text fields keep their own copy and paste. Known limit: if another app later puts only a non-text, non-image item (e.g. files) on the OS clipboard, Ctrl+V sends no event and does nothing; Shift+V still works.
- **Select-after.** `History` got `undo_target()`/`redo_target()` (the entry the next undo/redo acts on) and `transaction(id)`. `Document::apply_selecting(tx, select)` applies, stores `(before, select(&applied tx))` per `EntryId` in a `BTreeMap`, sets the selection to `after` and prunes. `undo`/`redo` look up the target first and, on success, restore `before`/`after` for a recorded entry; other entries keep the old rule (prune only). Records of entries no longer in the history are dropped at the next `apply_selecting`, so the map stays within the history cap plus one. Cut selects nothing after; paste selects `clipboard::pasted(tx)`.
- **PasteNodes.** `Command::PasteNodes { nodes, markers (index, name, group), links (from, to, shape) }` composes the existing commands: the first apply runs an `AddNodes`, maps indexes to its ids (`IndexOutOfRange` otherwise) and applies one `EditMarker` per marker and one `Connect` per link; the parts are kept as an inner `Transaction`, so revert is its revert and redo reapplies it (AddNodes reinserts the same ids). A failure on the first apply reverts the add, leaving the network unchanged.
- **Clipboard.** `clipboard::copy` keeps selected nodes in id order with x/z relative to the bounding-box centre, y and flag; markers in network marker order; links with both ends selected, from each node's `outgoing`: exact dual once (lower index), else one-way when the target lists the start as incoming, else reverse. `paste` rounds x/z with `edit_location::round3` (now `pub(crate)`), samples y or keeps the copied y, and renames taken marker names to the first free `_n` against the network and the paste's own earlier names.
- **View centre.** `MapView::centre()` (read-only accessor, the only change in `view.rs`) gives the map pixel at the view centre; paste converts it with `geometry.to_world`. Before the first frame of a newly loaded document it uses the map centre.
- **Verification (2026-11-08).** `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` all pass (ad-app 260, ad-core 62, ad-io 39 + golden_edited 2 + golden_roundtrip 1); `frame_time` release: every case ok, worst best-round max 9.5 ms (min zoom, all selected).

## Plan Change Log

- `view.rs`: added the read-only `MapView::centre()` accessor despite "Do not touch: view"; paste needs the view centre and the transform is private.

## Review Triage Log

- Quick review, pass 1: no patch; 1 low deferred.
  - low, defer: Ctrl+C/X/V events are consumed before the UI draws, so Ctrl+C on text highlighted in a non-modal window (the error window) copies nodes instead of the text. No such text the user needs to copy exists today; revisit with the settings UI.
  - checked, no finding: inner links rebuild their shape exactly, since `Network::connect` only adds links; select-after records are bounded by the history.
- Verification: fmt, clippy and workspace tests pass (re-run by the orchestrator).
