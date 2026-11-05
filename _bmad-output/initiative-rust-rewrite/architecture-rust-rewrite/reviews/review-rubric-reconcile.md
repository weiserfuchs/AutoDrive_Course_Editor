# Review: architecture-rust-rewrite.md (rubric + reconcile against research)

Verdict: The spine fixes the big structural divergence points well (crates, ids, adjacency, history, index, lossless XML), but it silently drops several file-level and lifecycle behaviours the research establishes (autosave, merge backup, unknown flag values, marker/vehicle linkage, legacy/unknown config versions, heightmap and map-image locating rules) and leaves a few Rules ambiguous enough that two epics could still diverge.

## Task A: rubric

### A1. Divergence points for feature epics
Covered: crate layering, NodeId vs file id, adjacency API, command/history, R-tree, config round-trip, coordinates, rendering thread model, job runner, tool trait, version handling, errors, defect list.

Missed (no AD, not in Deferred):
- Autosave and merge backup (F1). Both write config files and interact with AD-6 and AD-9.
- Hidden nodes / hidden connections and node warnings (OVERLAP, NEGATIVE_Y, OVERLAP_Y): stored on Node, derived by a query, or tool state? Java runs an overlap check per edit; AD-5 gives queries but no owner or trigger for warning state. The scan, render and add-node epics will each decide differently.
- Selection semantics: it is a Document field, but is a selection change an undoable command? Which mouse button builds it (Java: right button only, toggling)? Does it survive undo of a delete? Unspecified.
- Shortcut registry: Settings holds shortcuts, but there is no owner/format (action id to key), no default table, and no rule that tool-group cycling (repeat key cycles primary/secondary node, normal/dual/reverse connection) lives in the registry rather than per tool.
- Clipboard: the snapshot is kept after paste, paste goes to viewport centre or original location, markers are copied, y is recomputed from the heightmap. No rule says where the clipboard lives or that it is outside History.
- Map-image storage location (F4).

### A2. Is each AD's Rule enforceable, and does it prevent its divergence?
- AD-1: enforceable via the cargo dependency graph. Add a CI check (cargo-deny or a test on `cargo metadata`) so "no egui in ad-core" is mechanical. The "no global Mutex" part is review-only.
- AD-2: enforceable, but "no code outside ad-io reads or stores a file id" collides with marker ids and vehicles.xml (F3) and with the Java "show node ID" display toggle. Decide whether the displayed id is NodeId or a derived file-order number; otherwise two epics pick differently.
- AD-3: enforceable by API. Gap: load keeps disagreeing lists, but derived kind is then undefined for those pairs (Java: UNKNOWN, skipped). State that derive returns Unknown, renderers skip it, and save emits the lists as held.
- AD-4: mostly enforceable (Network mutators private to the command module). Gap: "dirty = cursor differs from saved cursor" breaks when the history cap evicts the saved state. Rule must say eviction of the saved cursor makes the document permanently dirty. Autosave interaction with an in-progress transaction is unstated.
- AD-6: "regenerates only `<waypoints>` and `<mapmarker>`" is not precise enough. Java also appends `<flags>` when absent and rewrites the children `id/x/y/z/out/incoming/flags` and `mm1..mmK` (each with id, name, group; `-1` for empty adjacency). State that structure. The only test is the unedited fixture; add a second golden test with an edited document compared to a hand-checked expected file, otherwise the regenerate path for new content is untested. The fixture in the repo has a BOM and CRLF line endings (checked), while the research says LF; the "follow source" rule is right, the research claim is wrong.
- AD-7: enforceable. Gap: heightmap formula and scale (F4).
- AD-8: "no rendering work on other threads" is fine. "Frame budget" is referenced under Deferred but never defined.
- AD-9: enforceable. Gap: autosave on a background thread needs a Document snapshot; the Rule says only the UI thread holds the Document, so say the serialization input is an owned clone made on the UI thread.
- AD-10: "cancels the preview, or the tool commits it explicitly first" lets two tools choose differently. Pick one default (cancel) plus an explicit exception list. Java's curve previews persisting across tool switches may be relied on by users; AD-13 does not list it as a fixed defect.
- AD-11: "detected once at load" leaves unknown and legacy major undefined (F2).
- AD-12: enforceable; add the clippy `unwrap_used`/`expect_used` lint on ad-core and ad-io to make it mechanical.
- AD-13: good but incomplete. Unlisted Java quirks that each epic would handle on its own: ArcSpline commit sets y = 0 (use heightmap?), Edit Location has no overlap check, the Z bounds check reuses X bounds in move clamp, move step rounding 3 vs 5 decimals, Redo menu enabled-state bug, DDS decoder shared static image, download without size check, manual map-image load not updating the coordinate reference image, async-vs-sync map-image import not displaying until next load.

### A3. Does anything under Deferred allow divergence?
- "Route-manager XML parity details": load/save semantics (replace network vs merge, flag reset for FS22/25, `routeExport` root with attributes, `;` lists, indent 4) decide whether routes share the ad-io reader and the Document-replacement path (AD-4, AD-12). Defer the format only; fix now that route import is either a whole-Document replace or a single Transaction.
- "Undo cap": dirty-state semantics depend on eviction behaviour (see AD-4 note).
- "Settings migration" is fine, but per-map settings (map scale, node size, grid, max zoom) drive AD-7 coordinates. Fix the defaults now: scale 1, node size 2, grid 2x2, max zoom 30 (range 1..250).
- "Platforms other than Windows": fine, but then working-directory-relative `mapImages/` paths (F4) must be decided now.
- "GPU-instanced rendering if profiling shows below the frame budget": no budget, no fixture size, no profiling story owner. Define the budget or list it as an open question.
- Experimental tools: acceptable since AD-3, AD-4 and AD-10 bind them.

### A4. Capability coverage versus research
Covered: load/save, model, undo, view, tools, curves, selection and clipboard, image import, scans, settings, update check.
Missing from the capability map: autosave; merge backup; Save As target semantics; recent-files list; vehicles.xml park destinations and parking icons; map-info dialog; marker group management; align tools (horizontal, vertical, depth, Edit Location); display toggles (marker names/icons, parking icons, node ID, hidden nodes, grid, subdivision snap); manual PNG and manual heightmap load; out-of-bounds fix scan; legacy-config warning/read-only state; DDS decode progress UI.

### A5. Dimensions at initiative altitude
- Deployment: Windows x64 exe via GitHub Releases is decided, but CI lists only fmt, clippy and test. No release workflow, no version source (Cargo version vs tag), no signing decision. Decide or list as open.
- Environments: nothing on toolchain pinning (`rust-toolchain.toml`) or committing `Cargo.lock`. Stack versions are not marked verified.
- Operations: logging is decided. Panic hook writing to the log is not (AD-12 says no panics on bad input, but image/egui can still panic). Update-check frequency and offline behaviour not decided.
- Testing: only unit tests plus one golden test. Missing: headless tool tests (Tool trait fed synthetic events against a Document), apply/revert property test (restores identical Network including order and R-tree), performance benchmark, and the fact that Java has zero tests (research Q5), so there is no behavioural oracle. FS19/FS22 paths are "unit tests only" but only an FS25 fixture exists.
- Security: zip import rejects absolute and `..` paths in Java; AD-9 covers timeouts only. Add zip-slip rule and a download size cap.

## Task B: things the research establishes that the spine drops or contradicts

### F1 [high] Autosave and merge backup are absent
Research: autosave every 10 min by default with 10 rotating slots, file `<config>_autosave_<slot>.xml`, slot wraps; autosave and backups never repoint the current file path and do not write vehicles.xml; autosave is suspended during undo/redo and node creation; merge backup `<config>_mergeBackup.xml` is written before a node merge.
Spine: no mention, not deferred, and a background autosave is in tension with AD-9.
Fix: add an AD (or extend AD-6 and AD-9): autosave and merge backup are save-to-copy operations; the UI thread clones the serialization input, the job runner writes via ad-io (temp+rename) with the naming pattern above; they never change the document's path or dirty cursor and never write vehicles.xml. Settings: interval and slot count (defaults 10 min, 10). Add to the capability map.

### F2 [high] Version handling incomplete; non-0/1 flag values would be lost
- A `<markerID>` (pre-1.1.0.6) config sets `canEditConfig=false` with a warning; an unknown major leaves the previous version in place (latent Java bug). Spine gives no behaviour. Fix: `GameVersion { Fs19, Fs22, Fs25, Unknown }` on Document; legacy config loads with a warning and the Document is read-only (no commands accepted, save disabled); unknown major loads as Unknown with a warning; never inherit the prior Document's version.
- Flags 2 and 4 are reset to REGULAR only for FS22 and FS25; FS19 keeps them. The spine's `enum NodeFlag { Regular, SubPrio }` cannot hold them, so FS19 files would be silently altered on save, contradicting AD-6. Fix: flag is an integer newtype with `REGULAR`/`SUBPRIO` constants that preserves any value on round trip; the FS22/25 reset lives in ad-io only.
- `descVersion` ranges (40-53 FS19, 60-80 FS22, 90 and above FS25, else unsupported) and the user-overridable combo are not recorded; AD-11 only says "used in image import". Record them.
- Missing optional `<flags>`: Java sets all flags 0 and appends `<flags>` on save. Spine silent; add to AD-6.
- FS25 has no DDS menu item (zip import only) and the wrong-size dialog has FS19/FS22 hints; carry into the image-import epic.

### F3 [high] Markers, groups and parking/vehicles.xml underspecified and in tension with AD-2
- The marker id is the marker's position in the file, `<mapmarker>` refers to nodes by file id, and vehicles.xml `parkDestination` values refer to marker ids. The ER diagram has no parked vehicles on MARKER, and vehicles.xml is mentioned only in AD-6 and AD-11.
- Fix: core Marker carries `parked_vehicles`; ad-io derives marker file ids from node order at save and rewrites `parkDestination` per vehicle from the model; missing vehicles.xml means an empty map; vehicles.xml is written only on a real Save/Save As (not autosave or backup); FS25 matches by `uniqueId`, others by `id`, and the element path differs by version. State that paste copies name and group but not parked vehicles.
- Groups exist in the file only as names on markers. State that groups are derived from marker group strings (plus an optional ordered list on Document) and what happens when the last marker of a group is removed or renamed.
- Marker ids are parsed as float then int (`194.000000`); markers pointing outside the node list are a load error.

### F4 [medium] Map image and heightmap rules absent
- Lookup order: `mapImages/<name>/<name><ext>.png`, legacy `mapImages/<name><ext>.png` (moved into the per-map folder), `src/main/resources/mapImages/...`, next to the jar. Download URL `raw.githubusercontent.com/KillBait/AutoDrive_MapImages/master/mapImages/<name>/<name><ext>.png`. All are working-directory relative in Java, which is meaningless for a distributed exe. Decide the base directory (exe dir or OS data dir via `directories`), and whether the legacy move and the `src/main/resources` path are dropped.
- Image must be 2048x2048 or an error dialog and no change; Blank.png fallback scaled to 2048; FS19 scales the whole image, FS22/25 crop the centre half then scale; zip import writes `<name>.png` and `<name>_HeightMap.png` and shows nothing until next load (decide whether to display immediately and list as a deviation).
- Heightmap: first `terrain.heightmap.png` beside the config, else `<name>_HeightMap.png`; 16-bit grey kept; y = ((R<<8)+G)/256 / heightMapScale, where heightMapScale is the rounded average of (heightmapY - node.y)/node.y over all nodes, minimum 1; returns -1 when absent and y == -1 triggers the negative-height warning. AD-7 specifies none of this. Put the formula, the sentinel (or Option mapped to -1) and the scale calculation in AD-7, and decide 16-bit use versus reproducing the 8-bit path.
- AD-8 drops the CPU copy after upload; clarify that this applies to the map image only, since heightmap sampling needs its pixels.
- DDS: Java handles DXT1-5 plus uncompressed; confirm `image_dds` covers the uncompressed variants. Port the zip path-traversal rejection as a rule.

### F5 [medium] Shortcuts, input mapping and view behaviour
- The default shortcut list (M, Delete, L, N, P, F, R, B; W/E/Shift+D; Shift+X/Y/F; Ctrl+X/C/V, Shift+V; H, G, S, D, numpad +/-, `;`; F1-F4) is not carried over. Add the table or name the owning epic, plus a single action-id registry in ad-app/ui. Also state whether Ctrl+Y is bound to redo.
- Shift inverts grid snap; right mouse builds selection; left or middle drag pans (setting `bMiddleMouseMove`); edge scroll 10 px; wheel zoom factor 1.6 with animated interpolation; fit-to-panel minimum zoom; max zoom 1..250 default 30; grid only above zoom 2; node screen size nodeSize*zoom*0.5; marker/parking/warning icons drawn at native pixel size, not zoom-scaled; connections drawn before nodes; overlays sorted by priority; previews at 0.55 alpha; pan clamped so the view never leaves the image. AD-8 names only LOD constants. Fix: one `view/constants.rs` named in AD-8 and a "carried over unchanged" list stating which of animated zoom and edge scrolling are ported.

### F6 [medium] Save semantics beyond the config body
- Java saves in place (re-parsing the original from disk) and Save As repoints the path; no atomic write and no .bak. Temp+rename is a good improvement but should be listed in AD-13 as a deviation. The Document should retain the opaque event sequence from load rather than re-read the source at save (Java fails if the source changed or vanished).
- Save is a silent no-op when no file is loaded; success and failure dialogs exist.
- Post-load steps (clear history, run overlap scan, clear selection, add to recent files, load image and heightmap) should be stated as the definition of a load in AD-4 and AD-9.
- Verify the XML declaration (`encoding="utf-8" standalone="no"`) is reproduced for edited documents, not only the unedited fixture.

### F7 [low] Tool and curve parameters
Defaults: 6 interpolation points, slider max 50, 1 control point, move scaler 3, arc half circle radius 8 with minimum 2. Curves hard-code REGULAR and priority 0 while XML defaults exist but are unused: say whether the new code honours them (an AD-13 line). Flip direction needs four ordered passes in Java only because of the toggle; with idempotent AD-3 it should be one command. Bezier t spacing is uniform in t, not arc length; keep or change explicitly.

### F8 [low] Misc
- The research says the fixture has LF line endings; the repo file has a BOM and CRLF.
- "Coordinates with exactly 3 decimals" is an assumption: add a test that parses and re-formats every number in the fixture, and decide behaviour for values Java would write in E-notation (below 1e-3 or at least 1e7).
- Stack versions are not verifiable from the research; confirm and pin through `Cargo.lock`.
- AD-5 lists "edges near a point" but the R-tree indexes nodes only; specify the algorithm (expanded-box node query then segment test, or a separate edge index).

## Top findings (summary)
1. [high] F1: autosave and merge backup missing and in tension with AD-9. Add an AD defining snapshot-then-background save, naming, no path or dirty side effects.
2. [high] F2: a two-value NodeFlag enum loses FS19 flags 2/4, and legacy or unknown version behaviour is undefined. Use a preserving flag newtype; define `GameVersion::Unknown` and read-only legacy mode.
3. [high] F3: marker file ids and vehicles.xml linkage contradict "file ids only in ad-io" unless the model carries `parked_vehicles` and ad-io derives marker ids from order. Fix the ER model and AD-6.
4. [medium] F4: map-image and heightmap rules (paths and base directory for a distributed exe, formula, -1 sentinel, 16-bit decision) absent from AD-7, AD-9 and AD-11.
5. [medium] AD-10 ("cancels ... or commits") is ambiguous and AD-4's dirty cursor breaks on history eviction. Choose one behaviour in each Rule.
