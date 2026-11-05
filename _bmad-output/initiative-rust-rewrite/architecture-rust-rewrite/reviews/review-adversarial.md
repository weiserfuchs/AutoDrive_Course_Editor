---
type: review
lens: adversarial (two compliant units that still build incompatibly)
target: architecture-rust-rewrite.md (draft, 2026-11-05)
---

# Adversarial review: architecture spine

**Verdict:** The spine settles crate boundaries and the command rule well, but it specifies the shared Document data (map info, markers and groups, selection and clipboard, saved-cursor, node order) only as nouns. Several epic pairs can each follow every AD and still collide. Two ADs are also not implementable as written (AD-2 with `slotmap`, AD-4 dirty state with a capped history).

Format per finding: Unit A and Unit B, how each obeys the ADs, where they clash, fix.

---

## F1 (High) Map info has no owner, no shape, and three possible homes

- **Unit A: map-image import / heightmap (ad-io + job runner).** Obeys AD-9 (decode off-thread, return owned result), AD-7 (heightmap "as data"), AD-8 (CPU copy dropped after texture upload). It returns `{image, heightmap, scale, map_name}` to the app and stores the scale in `Settings` under "per-map settings keyed by map name" (Conventions: Settings).
- **Unit B: map view + node tools (ad-app, ad-core).** Obeys AD-7 (`MapGeometry` converts world to pixel) and the structural seed (`Document = Network + History + selection + game version + map info`). It builds `MapGeometry` from the Document's map info.
- **Clash.**
  - Scale lives in Settings (A) and in Document (B). Changing scale in the settings dialog changes the view but not the Document, so out-of-bounds scans (ad-core) use a different scale from the renderer.
  - The heightmap is needed on the CPU by node creation, curve commit and paste (AD-7 sampler), but AD-8 drops the CPU image. Nothing says the heightmap is exempt.
  - A background result (image, heightmap) is neither "a command" nor "a whole new Document" (AD-9 offers only those two), so there is no legal way to attach it to the Document.
  - The map-name key for per-map settings has no defined source (config path, zip name, user input), so two units key differently.
- **Fix: new AD-14 "MapContext".**
  - `MapContext { name: MapName, scale: MapScale(u32), heightmap: Option<Arc<Heightmap>> }` is a field of `Document`, defined in ad-core. The map image texture and its CPU bytes are app state, not Document.
  - `MapName` is derived once, by one ad-io function, from the config path or import source. Settings are keyed only by it.
  - Settings stores only the last-chosen scale as a default. On Document load it is copied into `MapContext`; afterwards `MapContext` is the only source for `MapGeometry`.
  - Changing scale or attaching a heightmap is a `SetMapContext` command (in the history). State whether it marks the Document dirty (it should only if scale is persisted in the config).
  - Tighten AD-8: the heightmap is exempt from "CPU copy dropped" and is held as `Arc<Heightmap>`.
  - Tighten AD-9: results are classified as (a) new Document, (b) command, (c) `MapAttach` (texture plus heightmap), all applied by the UI thread.

## F2 (High) Heightmap sampling: two height policies for created nodes

- **Unit A: node and line tools.** AD-7: call `ad-core::sample_height(&heightmap, x, z)` for each new node.
- **Unit B: curve tools (Bézier, arc, spline).** AD-13: "Height is interpolated by index over all points", i.e. linear between endpoint heights.
- **Clash.** Both comply. A curve over a hill gets interpolated heights while a line over the same hill gets sampled heights. Paste, merge and move each choose differently again. A move command that re-samples y on release and a rotate/align command that keeps y produce different undo data.
- **Fix: tighten AD-7.**
  - Height is decided in ad-core, not by tools: every tool-produced position goes through `Network::place(x, z, hint_y)`, which samples the heightmap when present and otherwise uses the hint.
  - Amend AD-13's Bézier line to "interpolated by index only when no heightmap exists".
  - Move/rotate/align keep y unless the command explicitly names `ResampleHeight`.

## F3 (High) AD-2 is unimplementable with `slotmap`: "restore under the original NodeId"

- **Unit A: DeleteNodes command (ad-core).** AD-2: "Undo restores a deleted node under its original `NodeId`". `slotmap` cannot re-insert at a removed key; the implementer must tombstone or invent a second id.
- **Unit B: selection, clipboard, scans, curve tools holding `NodeId`s in tool or Document state.** They keep ids across a delete/undo cycle.
- **Clash.** If A tombstones, B's iteration APIs see or skip ghosts depending on implementer. If A re-inserts with a new key, every stored id from B dangles. Neither violates the AD text.
- **Fix: tighten AD-2.**
  - `NodeId` is allocated by a monotonic counter in `Network` and never reused (own map keyed by it, or slotmap with soft delete; name one in the AD).
  - `Network::reinsert(NodeId, Node, order_pos)` is the only restore path.
  - Any `NodeId` stored outside `Network` may dangle and must be resolved with `get()`, never indexed.

## F4 (High) Selection: outside the history but entangled with commands; clipboard has no owner

- **Unit A: selection + clipboard epic.** AD-10: selection is a Document field changed "only through selection operations" (not undoable). Copy stores `Vec<NodeId>`; paste clones those nodes.
- **Unit B: delete / merge / scan fixes.** AD-4: each is a Transaction that removes nodes, targeting `doc.selection`; apply/revert do not touch selection.
- **Clash.**
  - After delete, selection holds dead ids. After undo they are live again. One epic clears selection on delete, another leaves it, so "undo delete, then copy" copies different sets.
  - Cut = copy then delete: the clipboard holds `NodeId`s that are then deleted.
  - Clipboard in the Document (lost on load) or the app (survives load, points into the old Document)?
  - Paste "selects the new nodes": inside the command's apply, or a separate selection op? This decides whether undo of paste leaves dead ids selected.
- **Fix: new AD-15 "Selection and clipboard".**
  - Commands never read `doc.selection`; tools pass explicit `Vec<NodeId>` into commands.
  - After every apply/revert, `Document` prunes selection to live ids. A command may declare `select_after`, which History applies on apply and reverts to the prior selection on revert.
  - The clipboard is app-level, holds detached data (`ClipNode{rel_pos, flag, marker, internal links by index}`, no `NodeId`s), survives Document load, and paste works into any Document.

## F5 (High) Dirty state vs capped history vs async save vs autosave

- **Unit A: save / save-as (ad-io + job runner).** AD-6/AD-9: serialise on a background thread. On result arrival it sets `saved_cursor = history.cursor()` (AD-4).
- **Unit B: autosave/backup.** AD-9: a timer launches a background write. It skips work when "unsaved changes" is false and, on success, also updates the saved cursor because "a file was written".
- **Clash.**
  1. Save is async. If the user edits during the save, the cursor at completion is not the one serialised, so the Document is wrongly marked clean. The cursor must be captured at request time.
  2. Autosave to a backup path must not move the saved cursor. Nothing says so.
  3. Both jobs need owned input (AD-9), but nothing says who clones or serialises, or on which thread.
  4. History has a cap (AD-4). If the entry at the saved cursor is evicted, "cursor equals saved cursor" is false forever or, with index arithmetic, falsely true after wrap.
  5. A failed save must leave the saved cursor unchanged. Only the Document is protected by AD-12.
- **Fix: tighten AD-4, add AD-16 "Persistence jobs".**
  - Every history entry has a never-reused `EntryId`. `saved_at: Option<EntryId>`; eviction of that entry sets it to `None` (dirty until next save). "Unsaved" = current entry id != `saved_at`.
  - `SaveRequest` captures `entry_id_at_snapshot` and the serialised bytes (serialised on the UI thread from `&Network`, or via an ad-core `snapshot()`; pick one). On `Ok` the UI sets `saved_at` to the request's id. On `Err` nothing changes.
  - Autosave is a distinct job kind: writes only the backup path, never touches `saved_at`, runs only when the history revision differs from the last autosaved revision.

## F6 (Medium-High) Background load replaces Document while a tool has a preview

- **Unit A: config load (ad-io job).** AD-9: "a whole new Document for a load". The app swaps `self.doc` on poll; AD-4 replaces the history with it.
- **Unit B: a curve/line tool mid-preview, a drag in progress, or an open scan result dialog.** AD-10: preview lives in tool state, anchored on `NodeId`s of the old Document.
- **Clash.** The swap is legal but the tool keeps ids of a dead network. AD-10 cancels a preview only when the tool is switched; a Document swap is not a tool switch. The next commit refers to foreign ids (a panic, banned by AD-12, or a silent hit on the wrong node if ids are small counters). Related: the dirty prompt (before the job starts or when the result arrives?), and a late result of an older job (two loads, or an image import resolving after a new load) landing on the wrong Document.
- **Fix: tighten AD-9 and AD-10.**
  - Every Document has a `doc_epoch`. Jobs capture it at launch and results with a mismatched epoch are dropped.
  - Replacing the Document goes through one `App::replace_document()` that calls `tool.cancel()`, clears hover/drag state and selection, and resets history.
  - The unsaved-changes prompt is decided on the UI thread before the job starts; while a load is pending, further load/save requests are rejected.

## F7 (Medium-High) Node order: slotmap iteration vs the "stable order" AD-2 promises

- **Unit A: ad-io writer.** AD-2: file ids from "the network's stable node order: loaded order, new nodes appended in creation order". It iterates `network.nodes()`; with `slotmap` that is slot order, and freed slots are reused.
- **Unit B: delete-undo, merge scan, paste.** Restore "in its original order position"; merge removes one of two nodes; paste appends.
- **Clash.** The writer's order and Unit B's order are different structures. A delete-then-add-then-save file differs from the equivalent session with undo. The unedited golden test cannot catch it. Merge may keep the earlier or later node, giving different file ids per implementer.
- **Fix: tighten AD-2.** `Network` owns `order: Vec<NodeId>` as the only definition of file sequence; `nodes()` iterates it. DeleteNodes records the order index. Merge keeps the earlier node. Paste and curve commits append in creation order. Add an edited round-trip test (edit, save, reload, compare order and adjacency).

## F8 (Medium-High) File ids leak into other files: markers and vehicles.xml

- **Unit A: config writer.** Renumbers dense file ids at save (AD-2) and writes marker references with them.
- **Unit B: `vehicles.xml` / route-manager writer.** AD-6: "only `parkDestination` attributes change". It keeps the destination values it read at load.
- **Clash.** After a delete shifts file ids (or markers are reordered), `parkDestination` and route entries still hold old values. Both units obey AD-2 and AD-6, yet the saved file set is inconsistent. No AD says who owns the old-to-new map across files, or whether those values are node ids or marker indices (verify against the game).
- **Fix: tighten AD-2/AD-6.** Cross-file references are held in the Document as `NodeId`/marker handles (translated at load) and re-emitted through a `SaveIdMap` returned by the config writer, which the other writers consume. State which values are node ids and which are marker indices.

## F9 (Medium-High) Marker and group data shape

- **Unit A: ad-io config reader/writer.** Reads `<mapmarker>` entries in file order into a `Vec<Marker{node, name, group}>`; groups are just strings (Conventions).
- **Unit B: marker tools / group UI.** Treats a marker as `node.marker: Option<Marker>` (ER diagram: "NODE may carry MARKER") and keeps a group list that can contain empty groups; renames and deletes groups.
- **Clash.**
  - Markers on node vs markers as a Network collection: both match the text ("Network (nodes, adjacency, markers, groups)").
  - The file cannot persist an empty group; the UI wants one.
  - Rename group: one command or N?
  - Marker order matters for the game (marker index); a per-node field loses it.
  - Duplicates (two markers per node, same name in a group) and who restores a node's marker when DeleteNodes is undone are unspecified.
- **Fix: new AD-17 "Markers and groups".**
  - `Network.markers: Vec<Marker>` is order-preserving; `Marker { node: NodeId, name, group: GroupName }`; at most one per node, enforced by `Network::set_marker`.
  - Groups are derived (set of marker group names plus the fixed default), never stored separately. Empty groups do not exist after save; the UI may hold them as view state.
  - `RenameGroup` is one command listing the affected markers. DeleteNodes captures and restores the node's marker with its index.
  - Position in the vec is the marker index written to files.

## F10 (Medium) Command granularity for drags and live tools

- **Unit A: move-selection tool.** AD-10 says a tool returns "an optional Command" per input event. It emits `MoveNodes{delta}` per drag frame so the R-tree and overlay follow the mouse.
- **Unit B: rotate/align/curve-control widgets.** Preview in tool state, one transaction on release with absolute positions.
- **Clash.** Under A one drag consumes ~100 history entries and the cap, needs 100 undos, and deltas accumulate float error so revert is not exact. Under B the recorded old positions can be stale if anything intervened. "One user action = one Transaction" (AD-4) is ambiguous about what an action is.
- **Fix: tighten AD-10.** Drags and slider-style gestures are preview in tool state only; `Network` and the R-tree are untouched until release or Enter. One command per gesture. Move-type commands carry `Vec<(NodeId, old: Pos3, new: Pos3)>` with absolute positions, never deltas. History never coalesces. Esc, tool switch, or Document swap discards the preview without a command.

## F11 (Medium) Per-node flag vs per-connection kind: two mutation paths for one fact

- **Unit A: connection tools.** AD-3: `set_kind(a, b, kind)`.
- **Unit B: priority tool.** `SetFlag(node)` (AD command-naming list).
- **Clash.** The kind is derived from the two sets and the start node's flag, so `set_kind(a, b, SubPrio)` must flip `a.flag`, which changes every other outgoing edge of `a`. A's command stores one edge for revert; B's stores node flags. Undo of A leaves the other edges' kinds wrong.
- **Fix: tighten AD-3.** `set_kind` returns the full `ChangeSet` (adjacency and every flag it touched) and commands built on it store that set for revert; or restrict priority changes to `SetFlag` and limit `set_kind` to direction variants.

## F12 (Low-Medium) Scans and fixes operate on stale results

- **Unit: scans (overlap, out-of-bounds, merge).** AD-9 puts the scan in the background with owned input; AD-4 makes fixes commands. The result is a `Vec<NodeId>` in a dialog; the user edits, then clicks "fix".
- **Clash.** The fix hits nodes that moved or died. The F6 epoch only covers Document swaps, not edits.
- **Fix.** Scan results carry the history `EntryId` at scan time; applying a fix is refused (or rescanned) if it changed.

---

## Additional gaps

- AD-4 does not say whether non-network state (settings, map attach) is a command; F1 decides it.
- The golden test (AD-6) covers only the unedited fixture; add the edited round-trip test (F7, F8).
- `MapScale` should be a single ad-core type; Settings and `MapGeometry` currently describe it independently.

## Summary of fixes

| Id | Severity | Fix |
| --- | --- | --- |
| F1 | High | New AD-14 MapContext in Document; tighten AD-8, AD-9 |
| F2 | High | `Network::place` is the single height policy; amend AD-7, AD-13 |
| F3 | High | AD-2: monotonic never-reused `NodeId`, `reinsert` |
| F4 | High | New AD-15: explicit ids in commands, pruned selection, detached app-level clipboard |
| F5 | High | AD-4 `EntryId`/`saved_at` plus new AD-16 persistence jobs |
| F6 | Med-High | `doc_epoch` + `replace_document()` cancels tools (AD-9, AD-10) |
| F7 | Med-High | AD-2: `order` list owned by Network |
| F8 | Med-High | `SaveIdMap` for cross-file references (AD-2/AD-6) |
| F9 | Med-High | New AD-17 markers and groups |
| F10 | Medium | AD-10: one absolute-position command per gesture |
| F11 | Medium | AD-3: `set_kind` returns full ChangeSet |
| F12 | Low-Med | Scan results tagged with history entry id |
