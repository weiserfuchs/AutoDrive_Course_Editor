---
title: 'Parity sign-off: Rust editor 0.9.0 against R1-R16'
created: '2026-11-09'
status: 'signed-off'
signed_off_by: 'Weiserfuchs (user)'
signed_off_on: '2026-11-11'
---

# Parity sign-off: Rust editor 0.9.0 against R1-R16

Walk this list with the release exe (`cargo build --release -p ad-app`, then `target\release\AutoDriveCourseEditor.exe`) on the FS25 fixture.

**Never overwrite the fixtures.** Before you edit, copy `AutoDrive_config.xml` and `vehicles.xml` into a test folder, and open and save only the copies.

Tick each box, or write the gap next to it. A gap becomes a story, or you accept it with a note. When every line is ticked or noted, fill in `signed_off_by` and `signed_off_on` above (or tell the orchestrator "Parität abgenommen").

The **Evidence** lines name the automated tests that already cover the behaviour. The hand step checks what the tests cannot see.

## Initiative requirements

- [x] **R1 Byte-identical round trip.** Open the copy, Save As under a new name without editing, and compare the two files (`fc /b a.xml b.xml`): no differences.
  *Evidence:* ad-io golden and round-trip tests on the fixture.
- [x] **R2 Save after edits.** Add a node and a marker, then save. Reopen: both are there, and every other element is unchanged. Optionally load the result in the game.
  *Evidence:* ad-io save tests (edited network, markers kept, unknown elements kept).
- [x] **R3 FS19/FS22 and legacy configs.** Not required for hand testing (AGENTS.md). Optional: open an FS19 or FS22 config; a pre-1.1.0.6 config opens read-only.
  *Evidence:* ad-io FS19/FS22 round-trip tests, legacy read-only tests.
- [x] **R4 Node and connection tools.** Node and Sub-prio node add nodes. Move, Delete, Connect (regular, Dual, Reverse, L cycles), Priority, Flip and Rotate each work on one node and on a selection.
  *Evidence:* ad-core command tests, ad-app tool tests.
- [x] **R5 Curves.** Make a Bézier curve, an Arc and a straight line. Change the interpolation points and commit each; nodes and connections appear.
  *Evidence:* bezier, arc and line tool tests.
- [x] **R6 Selection and clipboard.** Select with a rectangle, a lasso and a click. Use Align H, Align V and Align depth, and Edit location. Copy, Cut, Paste (to the view centre) and Paste in place (Shift+V).
  *Evidence:* selection, align and clipboard tests.
- [x] **R7 Undo, redo and the unsaved prompt.** Undo and Redo every edit above (Ctrl+Z, Ctrl+Shift+Z). The title shows unsaved changes. Closing asks Save / Discard / Cancel.
  *Evidence:* history tests, prompt tests.
- [x] **R8 Map view.** You see the map image, nodes, connections coloured by kind, markers, parking and warning icons and the grid. Zoom and pan are smooth.
  *Evidence:* view paint tests, frame-time test.
- [x] **R9 Map images and heightmaps.**
  - A known map loads its local image or downloads it when online images are on.
  - Import ▸ Map mod zip…, DDS for FS19… and DDS for FS22/FS25… each import a map image.
  - Import heightmap… loads a heightmap; a new node then takes its height from it.
  *Evidence:* map-image, download, DDS, zip and heightmap tests.
- [x] **R10 Markers, groups and parking.** Add, Edit and Delete marker, and the Groups window. Save next to `vehicles.xml` (the copy): park destinations are kept and updated.
  *Evidence:* marker, group and vehicles tests.
- [x] **R11 Scans.**
  - Scan ▸ Find overlapping nodes…, then Merge overlapping nodes… writes `*_mergeBackup.xml` first.
  - Fix nodes outside the map…, Fix missing node heights… and Check connections… work.
  *Evidence:* scan, merge and backup tests.
- [x] **R12 Autosave.** Turn autosave on with a short interval (Edit ▸ Settings… ▸ AutoSave) and edit. `*_autosave_*.xml` files rotate, while the opened file and its unsaved marker stay unchanged.
  *Evidence:* autosave tests.
- [x] **R13 Route manager.**
  - File ▸ Routes… opens a `routes.xml`; open one route.
  - Edit it, save it and reopen it: the route format and groups are kept.
  *Evidence:* routes_xml round-trip tests (epic 11).
- [x] **R14 Settings and shortcuts.**
  - Change settings, the map scale, the grid and a key binding (Edit ▸ Settings… ▸ Key bindings). Restart: everything is kept in `settings.toml`.
  - Redo is Ctrl+Shift+Z.
  *Evidence:* settings persistence and key-binding tests (epic 9).
- [x] **R15 Frame budget.** Already measured and met (epic 10 story 3, every case under 16 ms up to zoom 250). Hand check: pan, zoom and drag feel fluid on the fixture at minimum and maximum zoom.
- [x] **R16 UI shell.** Every row of the inventory checklist below.

## UI-shell inventory (R16)

The inventory is in `research-ui-shell-inventory.md`. The epic 9 sweep checked every row against the code; here you check them in the exe.

- [x] **File:** Open…, Open Recent (list, Clear list), Routes…, Save (only when there are changes; no dialog), Save As…, Exit. Ctrl+O, Ctrl+S, Ctrl+Shift+S and Ctrl+Q work.
- [x] **Edit:** Undo, Redo, Cut, Copy, Paste, Paste in place, Settings…
- [x] **Display:**
  - Grid; Grid and snap… (presets, spacing, subdivisions); Map scale (24 entries).
  - Marker names (F1), Marker icons (F2), Parking icons (F3), Selected node id.
  - Node size bigger and smaller (+/−), Select hidden (H), Show all hidden, Go to node id… (G), Map info…
- [x] **Scan** and **Import:** as in R9 and R11; also Load map image…, Save map image… and Export heightmap…
- [x] **Help:**
  - About… shows version 0.9.0 and the fork link.
  - Version history opens the fork's releases page.
  - How to create map images opens the map-image help.
  - Open log folder works.
- [x] **Tool bar:**
  - Grouped tools with tooltips naming the current key: Hide node and Hide connection, Continuous, Snap, Rect/Lasso, and Undo/Redo/Cut/Copy/Paste buttons.
  - Shift+F centres on the hovered node; D toggles subdivision snap.
- [x] **Settings window:** the Editor, AutoSave, Curves, Connections and Key bindings tabs. Theme System/Light/Dark switches at once.
- [x] **Update check:** with the setting on, the start shows nothing until a newer fork release exists. This becomes testable after the v0.9.0 release (story 7).
- [x] **Window:** position and size are kept after a restart. The release exe opens no console window.

## Agreed differences (no action)

- **Dropped by the user (2026-11-09):**
  - Debug menu (except Go to node id), Experimental mode, Move widget;
  - the FlatLaf theme catalogue, the ☰ panel toggles, AutoSave Apply Now/Later;
  - Alt mnemonics, colour settings.
- **Out of scope:** localisation, `EditorConfig.xml` import, the message panel, FS19/FS22 hand tests.
- **Not ported; ask for any of them as a new story:**
  - middle-mouse-only panning, interpolated zoom, outline arrowheads;
  - drawn subdivision lines, tool-bar icons;
  - a log window or log level, a manual "Check for updates";
  - the four per-kind connection hide buttons.
- **Changed on purpose:**
  - Redo is Ctrl+Shift+Z (R14).
  - Arc is a normal tool, not experimental.
  - Version history links to the fork's releases page.
  - Save writes in place.

## Gaps found

| # | Where | Gap | Decision (story / accepted) |
|---|---|---|---|

Signed off by the user on 2026-11-11 ("passt soweit alles"): the hand checks of epics 11, 9 and epic 10 story 1 and this checklist found no gap. The update-check line is confirmed with the real release in story 7.

Update check (story 7): the published v0.9.0 is what `releases/latest` returns; a 0.9.0 build shows no update window, as expected.
