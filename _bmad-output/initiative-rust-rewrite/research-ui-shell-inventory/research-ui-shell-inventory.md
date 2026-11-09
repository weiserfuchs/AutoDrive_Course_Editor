---
title: 'Codebase research: UI-shell inventory'
type: codebase
ask: |-
  Inventory every user-facing menu item, toolbar and action-bar button, toggle, setting, key binding and the update check of the Java editor, each mapped to its state in the Rust app (input for refining epic-settings-ui-shell, R16).
repository: '.'
commit: '26f78eac78af15203f41e85e169668d2fc08d577'
branch: 'master'
status: complete
created: '2026-11-09'
---

# Codebase research: UI-shell inventory

## 1. Scope, date and sources

Date: 2026-11-09, at commit `26f78ea` (master, after the route-manager epic).

Scope: everything a user can see or trigger in the shell of the Java/Swing editor: the menu bar, the
left toolbar, the top action bar, the config window and its tabs, key bindings, what is persisted
between runs, the update check and the other windows. Each item is mapped to the Rust app. Tool
*behaviour* (how a drag moves a node, how a curve commits) is out of scope; the earlier codebase
research runs cover it. Only the shell entry point of each tool is listed here.

Status values (Rust column):

- **present**: the feature exists in Rust; the Rust place is named.
- **partial**: some of it exists; the cell says what is missing.
- **missing**: nothing in Rust.

Every Rust status was checked by grep and reading in `crates/` (not guessed). Labels are the visible
English texts from `src/main/resources/locale/AutoDriveEditor_en_US.properties` (the only locale
shipped). Paths below are relative to `src/main/java/AutoDriveEditor/` for Java and `crates/` for
Rust.

Sources read:

- Java: `GUI/Menus/EditorMenu.java` and every class it adds under `GUI/Menus/**`; `GUI/Toolbar.java`,
  `GUI/ActionBar.java`, `GUI/Buttons/Toolbar/**`, `GUI/Buttons/ActionBar/**`; `GUI/Config/ConfigGUI.java`
  and `GUI/Config/Tabs/*`; `GUI/Curves/CurvePanel.java`; `GUI/RoutesGUI/RoutesGUI.java`;
  `GUI/TextPanel.java`; `GUI/MapPanel.java` (key map); `Managers/ShortcutManager.java`,
  `Managers/VersionManager.java`, `Managers/ThemeManager.java`; `XMLConfig/EditorXML.java`,
  `XMLConfig/AutoSave.java`; `AutoDriveEditor.java`; `Locale/LocaleManager.java`.
- Rust: `ad-app/src/ui.rs` (top bar, toolbar, all windows and dialogs), `ad-app/src/input.rs`
  (`Action`, `SHORTCUTS`), `ad-app/src/main.rs` (`App`, `run`, `save`, `open`, autosave tick,
  window title), `ad-app/src/startup.rs` (logging, settings load), `ad-app/src/view.rs`
  (`MapView` flags), `ad-app/src/look.rs`, `ad-app/src/overlay.rs`, `ad-app/src/lod.rs`,
  `ad-app/src/tools/*`, `ad-io/src/settings.rs`, `ad-io/src/download.rs`.

Shape of the Rust shell today: there is no menu bar. `ui::top_bar` is one wrapped row with the
buttons Open…, Routes…, Save, the menus Import and Scan, the Grid checkbox, then `ui::toolbar` (19
tool buttons as text, Groups, Continuous, Snap, Rect/Lasso, Undo, Redo) and status labels. Shortcuts
are one fixed table, `input::SHORTCUTS` (23 entries). Settings are read once at start from
`%APPDATA%\AutoDrive Course Editor\config\settings.toml`; the app writes that file only when it does
not exist yet (`startup::load_settings`), so no change made while running is ever persisted.

## 2. Inventory

### 2.1 File menu (Java "File", mnemonic Alt+F)

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Open Config (Alt+L) | GUI/Menus/FileMenu/OpenConfigMenu.java | Asks to save a dirty config, then opens an `.xml` file chooser at the last used folder and loads the config. | **present** (`ui::top_bar` "Open…", `App::request(Pending::Open)`, `App::open`) | Unsaved prompt present. Rust dialog does not start in a remembered folder (no `set_directory` in `App::open`); Alt+L missing (see 2.8). |
| Open Recent ▸ (submenu) | GUI/Menus/FileMenu/RecentFilesMenu.java | Lists up to 10 recent configs and route files (newest first, path cut to 75 chars, icon per type); a click loads it after the unsaved check. | **missing** | Stored in `EditorConfig.xml` `<RecentFiles>` (see 2.10). A path already in the list is not moved to the top. |
| Open Recent ▸ Clear List | GUI/Menus/FileMenu/RecentFilesMenu.java | Confirms, then empties the recent list. | **missing** | |
| Save Config (Alt+S) | GUI/Menus/FileMenu/SaveConfigMenu.java | Saves to the open file without a dialog; enabled only while the config is dirty. | **partial** (`ui::top_bar` "Save", `App::save`) | Rust Save always opens a save dialog prefilled with the path (deferred-work Retro R4); enabled whenever the document is editable, not only when dirty. |
| Save As (Alt+A) | GUI/Menus/FileMenu/SaveAsConfigMenu.java | Save dialog at the last used folder; saves under a new name. | **partial** (`App::save`) | The only Save is effectively Save As; there is no separate item. |
| Exit (Alt+Q) | GUI/Menus/FileMenu/ExitEditorMenu.java | Sends a window-closing event (unsaved prompt, stop threads, write `EditorConfig.xml`). | **partial** (`App::intercept_close`, `ui::unsaved_prompt`) | Closing the window asks Save/Discard/Cancel (better than Java's Yes/No); no Exit menu item and no settings write on exit. |

### 2.2 Routes menu (Java "Routes", Alt+R)

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Load RoutesManager config | GUI/Menus/RoutesMenu/OpenRoutesConfig.java | Picks a `routes.xml`, opens the Routes Manager Config window listing its routes. | **present** (`ui::top_bar` "Routes…", `App::pick_routes_list`, `ui::routes_window`) | |
| Load RoutesManager XML | GUI/Menus/RoutesMenu/OpenRoutesXML.java | Picks a single route export (`*.xml` but not `routes.xml`) and opens it; asks to save a dirty config first (Yes/No). | **present** (`ui::top_bar` "Open…"; `jobs` detects a route via `routes_xml::is_route`) | No separate menu item; the one Open handles both file kinds. |
| Save RoutesManager XML | GUI/Menus/RoutesMenu/SaveRoutesXML.java | Saves the open route export in place; enabled only for a dirty route document. | **partial** (`ui::top_bar` "Save", `jobs` -> `routes_xml::save`) | Same Save-is-Save-As gap as 2.1. |

### 2.3 Edit menu (Java "Edit", Alt+E)

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Undo (Ctrl+Z) | GUI/Menus/EditMenu/UndoMenu.java | Undoes the last change. | **present** (`ui::toolbar` "Undo", `Action::Undo`, Ctrl+Z) | No menu entry; a toolbar button instead. |
| Redo (Shift+Z) | GUI/Menus/EditMenu/RedoMenu.java | Redoes the last undone change. | **present** (`ui::toolbar` "Redo", `Action::Redo`, Ctrl+Shift+Z) | Binding changed on purpose (R14, AD-13). |
| Cut (Ctrl+X) | GUI/Menus/EditMenu/CutMenu.java | Cuts the selection; enabled with a selection. | **partial** (`Action::Cut`, Ctrl+X / clipboard event) | Shortcut only: no menu item, no button. Java hard-codes Ctrl+X here, the `CUT` key bind is never wired. |
| Copy (Ctrl+C, bindable) | GUI/Menus/EditMenu/CopyMenu.java | Copies the selection. | **partial** (`Action::Copy`) | Shortcut only. |
| Paste (Ctrl+V) | GUI/Menus/EditMenu/PasteMenu.java | Pastes the clipboard at the view centre. | **partial** (`Action::Paste`) | Shortcut only. Java hard-codes Ctrl+V; the `PASTE` key bind is never wired. |
| Paste Original Location (Shift+V) | GUI/Menus/EditMenu/PasteOriginalLocationMenu.java | Pastes at the copied place. | **partial** (`Action::PasteInPlace`, Shift+V) | Shortcut only. Java hard-codes Shift+V; the `PASTE_ORIGINAL` key bind is never wired. |

### 2.4 Display menu (Java "Display", Alt+D)

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Set Map Scale ▸ 2km (2.0x) … 48km (24 radio items) | GUI/Menus/Display/MapScaleMenu.java | Sets the map scale 1..24 for the open map; stored per map. | **partial** (`ad-io settings::MapSettings::scale`, read in `jobs` at load) | Only by editing `settings.toml` `[maps.<name>] scale`; no UI and never written back. Items 1-10 also show the factor (2^i). |
| Show Marker Names (F1) | GUI/Menus/Display/ShowMarkerNames.java | Toggles marker name labels; persisted `ShowMarkerNames` (true). | **partial** (`view::MapView::show_marker_names`, default true) | Flag drawn by the view, but no menu item, no F1, not persisted. |
| Show Marker Icons (F2) | GUI/Menus/Display/ShowMarkerIcons.java | Toggles marker icons; persisted `ShowMarkerIcons` (true). | **partial** (`MapView::show_marker_icons`, default true) | Same: no UI, no F2, not persisted. |
| Show Parking Icons (F3) | GUI/Menus/Display/ShowParkingIcons.java | Toggles the parking icons on markers with parked vehicles; persisted `ShowParkingIcons` (true). | **missing** | Rust always draws the parking icon (`view.rs` parking texture); no flag. |
| Show Selected Node ID (F4) | GUI/Menus/Display/ShowNodeID.java | Toggles the node ID in the hover info; persisted `ShowSelectedNodeID` (false). | **partial** (`Action::ToggleNodeId`, F4, `MapView::show_node_id`) | No menu item, not persisted. |

### 2.5 Scan menu (Java "Scan", Alt+S)

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Scan For Overlapping Nodes | GUI/Menus/ScanMenu/ScanNetworkMenu.java | Asks a search distance, marks overlapping nodes. | **present** (`ui::top_bar` Scan ▸ "Find overlapping nodes…", `ui::overlap_dialog`) | |
| Centre Out-Of-Bounds Nodes | GUI/Menus/ScanMenu/OutOfBoundsFixMenu.java | Confirms the bounds, moves nodes outside the map to x=z=0 (undoable) and centres the view on the first. | **present** (Scan ▸ "Fix nodes outside the map…", `Fix::OutOfBounds`, `ui::fix_prompt`) | Rust behaviour per epic 8 (`scan::out_of_bounds_plan`). |
| Fix Invalid Nodes Height | GUI/Menus/ScanMenu/FixNodesHeightMenu.java | Confirms, sets y from the heightmap for nodes with y = -1 (not undoable in Java). | **present** (Scan ▸ "Fix missing node heights…", `Fix::MissingHeights`) | |
| Merge Overlapping Nodes | GUI/Menus/ScanMenu/MergeNodesMenu.java | Merges the marked overlapping nodes. | **present** (Scan ▸ "Merge overlapping nodes…", `ui::merge_prompt`) | |
| (Rust only) Clear overlap marks, Check connections… | – | Rust additions. | **present** (`ui::top_bar`, `ui::connection_report`) | Not in Java; keep. |

### 2.6 Import menu (Java "Import", Alt+I)

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Import Images from .zip (Alt+Z) | GUI/Menus/ImportMenu/ImportFromZipMenu.java | Picks a map mod zip, shows a preview (map image, heightmap, detected name and game version, override), imports. | **present** (Import ▸ "Map mod zip…", `ui::zip_preview_window`) | Alt+Z missing. |
| Map Image ▸ Load Map Image (Alt+M) | GUI/Menus/ImportMenu/LoadMapImageMenu.java | Loads any PNG as the map image (not saved). | **present** (Import ▸ "Load map image…") | Alt+M missing. |
| Map Image ▸ Import FS19 PDA Image (Alt+I) | GUI/Menus/ImportMenu/ImportFS19DDSMenu.java | Imports `pda_map_H.dds`. | **present** (Import ▸ "DDS for FS19…") | Alt+I missing (and Alt+I is also the Import menu mnemonic in Java). |
| Map Image ▸ Import FS22 PDA Image (Alt+P) | GUI/Menus/ImportMenu/ImportFS22DDSMenu.java | Imports `overview.dds`. | **present** (Import ▸ "DDS for FS22/FS25…") | Alt+P missing. |
| Map Image ▸ Save As Default Map Image (Alt+B) | GUI/Menus/ImportMenu/SaveMapImageMenu.java | Save dialog for `mapImages/<map>/<map>.png`. | **present** (Import ▸ "Save map image…") | In Java the actual export is commented out: the dialog writes nothing. Rust works. Alt+B missing. |
| HeightMap ▸ Load Heightmap | GUI/Menus/ImportMenu/ImportHeightMapMenu.java | Loads `terrain.heightmap.png`. | **present** (Import ▸ "Import heightmap…") | |
| HeightMap ▸ Save As Default Heightmap | GUI/Menus/ImportMenu/ExportHeightMapMenu.java | Saves the heightmap as `mapImages/<map>/<map>_HeightMap.png`. | **present** (Import ▸ "Export heightmap…") | |

### 2.7 Help menu (Java "Help", Alt+H) and Debug menu (Java "DEBUG")

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| About Editor | GUI/Menus/HelpMenu/AboutMenu.java | Window with logo, version, build info, "by KillBait!", credits (@rhaetional, @whitevamp, Stephan). | **missing** | Rust version is `ad-app` `0.1.0` (`Cargo.toml`); nothing shows it. Credits text names upstream authors (keep credits, change authorship line). |
| Version History | GUI/Menus/HelpMenu/ShowHistoryMenu.java -> Managers/VersionManager.showVersionHistory | Shows `version_history` HTML from local `history.xml`, else downloads it from KillBait upstream. | **missing** | URL points at upstream (see 2.11). |
| How to create map images | GUI/Menus/HelpMenu/ImagesLinkMenu.java | Dialog with links to the map-images repo and its how-to discussion. | **missing** | Links `github.com/KillBait/AutoDrive_MapImages` and `.../discussions/20`: the allowed data repo. |
| Show Editor Log (checkbox) | GUI/Menus/HelpMenu/LoggingGUIMenu.java | Opens an "Editor Log" window that tails `AutoDriveEditor.log` (1024x600, progress bar on load). | **missing** | Rust logs to `%APPDATA%\AutoDrive Course Editor\data\logs\editor.log.<date>` (`startup::start_logging`); no viewer, no "open log folder". |
| Enable Debug Menu (checkbox) | GUI/Menus/HelpMenu/ShowDEBUGMenu.java | Shows the DEBUG menu and puts "( DEBUG MODE )" in the title. | **missing** | Suggest drop (section 3.2). |
| DEBUG ▸ Centre node in viewport | GUI/Menus/DebugMenu/MoveNodeToCentreMenu.java | Asks a node ID and centres the view on it. | **missing** | Useful as a non-debug "Go to node ID…" (section 3.1). |
| DEBUG ▸ Show All Node ID's (Alt+6) | GUI/Menus/DebugMenu/ShowAllNodeIDMenu.java | Draws every node's ID. | **missing** | Suggest drop. |
| DEBUG ▸ Show Node Height | GUI/Menus/DebugMenu/ShowNodeHeightMenu.java | Draws node heights. | **missing** | Suggest drop (hover info shows y). |
| DEBUG ▸ Show Selected Node Info (Alt+7) | GUI/Menus/DebugMenu/ShowNodeLocationInfo.java | Draws location info of the selected node. | **missing** | Suggest drop (hover info box covers it). |
| DEBUG ▸ Show Curve Debug | GUI/Menus/DebugMenu/ShowCurveManagerInfo.java | Curve manager overlay. | **missing** | Suggest drop. |
| DEBUG ▸ Show Draw Order Info | GUI/Menus/DebugMenu/ShowDrawOrderInfo.java | Draw-order overlay. | **missing** | Suggest drop. |
| DEBUG ▸ Show Frame Info | GUI/Menus/DebugMenu/ShowFrameInfo.java | Frame timing overlay. | **missing** | Suggest drop (Rust has frame-time tests). |
| DEBUG ▸ Show HeightMap Info | GUI/Menus/DebugMenu/ShowHeightMapInfo.java | Heightmap overlay. | **missing** | Suggest drop. |
| DEBUG ▸ Show Render Times | GUI/Menus/DebugMenu/ShowRenderProfileInfo.java | Render profile overlay. | **missing** | Suggest drop. |
| DEBUG ▸ Show Widget Debug | GUI/Menus/DebugMenu/ShowWidgetManagerInfo.java | Widget manager overlay. | **missing** | Suggest drop. |
| DEBUG ▸ Show Zoom Info | GUI/Menus/DebugMenu/ShowZoomLevelInfo.java | Zoom overlay. | **missing** | Suggest drop. |
| DEBUG ▸ Logging ▸ Managers ▸ Log ButtonManager / CopyPasteManager / CurveManager / MultiSelectManager / RouteManager / ScanManager / ThemeManager / WidgetManager Debug (8 checkboxes) | GUI/Menus/DebugMenu/Logging/Log{ButtonManagerInfo,CopyPasteManager,CurveManagerInfo,MultiSelectManagerInfo,RouteManager,ScanManagerInfo,ThemeManagerInfo,WidgetManagerInfo}Menu.java | Each turns on verbose logging for one Java manager. | **missing** | Suggest drop: Rust uses `tracing`; a single log-level setting replaces all 29 log toggles. |
| DEBUG ▸ Logging ▸ Log AutoSave / Config GUI / Connection Selection / Curve Creation / CurveWidget / FileIO / Flip Connection / GUI / Heightmap co-ordinate / LinearLine Creation / Marker / Menu / Merge Nodes / MoveWidget / Shortcut / Snapshot / Undo/Redo / XML Load/Save / XMLReader / ZipUtils / Zoom/Scaling Debug (21 checkboxes) | GUI/Menus/DebugMenu/Logging/Log{AutoSave,ConfigGUIInfo,ConnectSelection,CurveInfo,CurveWidget,FileIO,FlipConnection,GUIInfo,HeightmapInfo,LinearLineInfo,MarkerInfo,MenuDebug,MergeFunction,MoveWidget,ShortcutInfo,SnapShotInfo,UndoRedo,XMLConfig,XMLReader,ZipUtils,ZoomScale}Menu.java | Each turns on verbose logging for one area. | **missing** | Suggest drop (as above). |

### 2.8 Toolbar tools (Java left floating toolbar, `GUI/Toolbar.java`)

The Java toolbar is a column of six collapsible groups (icon buttons; a group header toggles its
panel). Rust shows all tools as text buttons in `ui::toolbar`.

| Java item (visible label = tooltip) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Group headers: Nodes, Curves, Markers, Alignment, Show/Hide, Experimental | GUI/Toolbar.java | Collapse/expand each tool group. | **partial** (`ui::toolbar`) | Flat text buttons, no groups, no icons, no tooltips with the shortcut. |
| Move nodes (M) | GUI/Buttons/Toolbar/Nodes/MoveNodeButton.java | Move tool (with the move widget). | **present** ("Move", `tools::move_node`, M) | Rust has no move widget (see 2.9.6). |
| Create regular connection (L) | GUI/Buttons/Toolbar/Nodes/NormalConnectionButton.java | Connect tool, one-way. L cycles regular, dual, reverse (shortcut group). | **present** ("Connect", `tools::connect`, L cycles) | |
| Add regular node (N) | GUI/Buttons/Toolbar/Nodes/AddPrimaryNodeButton.java | Node tool, regular. N cycles regular/sub-prio. | **present** ("Node", `tools::node`, N cycles) | |
| Create dual connection (L) | GUI/Buttons/Toolbar/Nodes/DualConnectionButton.java | Connect tool, dual. | **present** ("Dual") | |
| Add subprio node (N) | GUI/Buttons/Toolbar/Nodes/AddSecondaryNodeButton.java | Node tool, sub-prio. | **present** ("Sub-prio node") | |
| Create reverse connection (L) | GUI/Buttons/Toolbar/Nodes/ReverseConnectionButton.java | Connect tool, reverse. | **present** ("Reverse") | |
| Swap node priority (P) | GUI/Buttons/Toolbar/Nodes/SwapNodePriorityButton.java | Toggles node priority by click or area. | **present** ("Priority", P) | |
| Flip connections (F) | GUI/Buttons/Toolbar/Nodes/FlipDirectionButton.java | Flips connection direction. | **present** ("Flip", F) | |
| Rotate Nodes (R) | GUI/Buttons/Toolbar/Nodes/RotationButton.java | Rotates the selection; right click sets the snap step. | **present** ("Rotate", R) | Snap step fixed 5° (`tools::rotate::SNAP_STEP_DEG`), see 2.9. |
| Remove Nodes (Delete) | GUI/Buttons/Toolbar/Nodes/DeleteNodeButton.java | Delete tool; Delete key deletes the hovered node. | **present** ("Delete", `Action::DeleteHovered`) | |
| Create a Bézier curve (B) | GUI/Buttons/Toolbar/Curves/BezierCurveButton.java | Bézier tool with the curve panel. | **present** ("Bézier", B, `ui::curve_panel`) | |
| New marker (W) | GUI/Buttons/Toolbar/Markers/AddMarkerButton.java | Add-marker tool. | **present** ("Add marker", W) | |
| Edit Marker (E) | GUI/Buttons/Toolbar/Markers/EditMarkerButton.java | Edit-marker tool. | **present** ("Edit marker", E) | |
| Delete Marker (Shift+D) | GUI/Buttons/Toolbar/Markers/DeleteMarkerButton.java | Delete-marker tool. | **present** ("Delete marker", Shift+D) | Rust adds "Groups" (`ui::groups_window`). |
| Horizontally align nodes (Shift+X) | GUI/Buttons/Toolbar/Alignment/HorizontalAlignButton.java | Align selection z to a clicked node. | **present** ("Align H", Shift+X) | |
| Vertically align nodes (Shift+Y) | GUI/Buttons/Toolbar/Alignment/VerticalAlignButton.java | Align selection x to a clicked node. | **present** ("Align V", Shift+Y) | |
| Depth Align nodes | GUI/Buttons/Toolbar/Alignment/DepthAlignButton.java | Align selection y. | **present** ("Align depth") | No shortcut in either. |
| Edit Node Location | GUI/Buttons/Toolbar/Alignment/EditLocationButton.java | Click a node, edit x/y/z. | **present** ("Edit location", `ui::location_dialog`) | |
| Show/Hide nodes | GUI/Buttons/Toolbar/Display/HideNodeButton.java | Hides nodes; double right click resets all hide toggles. | **missing** | Session state in Java (not persisted). |
| Show/Hide regular connections | GUI/Buttons/Toolbar/Display/HideRegularConnectionsButton.java | Hides regular connections. | **missing** | |
| Show/Hide subprio connections | GUI/Buttons/Toolbar/Display/HideSubprioConnectionsButton.java | Hides sub-prio connections. | **missing** | |
| Show/Hide dual connections | GUI/Buttons/Toolbar/Display/HideDualConnectionsButton.java | Hides dual connections. | **missing** | |
| Show/Hide reverse connections | GUI/Buttons/Toolbar/Display/HideReverseConnectionsButton.java | Hides reverse connections. | **missing** | |
| Create an Arc (Experimental) | GUI/Buttons/Toolbar/Experimental/ArcSplineButton.java | Arc tool, only with `-EXPERIMENTAL`. | **present** ("Arc", `tools::arc`) | Promoted to a normal tool in Rust. |
| Test button (Experimental) | GUI/Buttons/Toolbar/Experimental/TestButton.java | Developer test hook (reuses the EDIT_MARKER shortcut). | **missing** | Suggest drop. |
| Road connection (Experimental) | GUI/Buttons/Toolbar/Experimental/RoadConnection.java | Unfinished road-connection tool. | **missing** | Suggest drop. |
| Scale (Experimental) | GUI/Buttons/Toolbar/Experimental/ScaleButton.java | Unfinished scale-selection tool. | **missing** | Suggest drop (or a later feature request). `TemplateButton.java` exists but is never added. |

### 2.9 Action bar (Java top bar, `GUI/ActionBar.java`)

| Java item (visible label = tooltip) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Menu button ☰ ▸ Show Undo/Redo | GUI/ActionBar.java | Shows/hides the undo/redo panel; persisted `UndoRedoVisible` (true). | **missing** | Suggest drop (Rust buttons are always shown). |
| Menu button ☰ ▸ Show Cut/Copy/Paste | GUI/ActionBar.java | Shows/hides the clipboard panel; persisted `CopyPasteVisible` (true). | **missing** | Suggest drop. |
| Menu button ☰ ▸ Show Text Panel | GUI/ActionBar.java | Shows/hides the bottom text panel; persisted `TextPanelVisible` (false). | **missing** | See 2.12 text panel. |
| Undo the last action | GUI/Buttons/ActionBar/ChangePanel/UndoButton.java | Undo. | **present** (`ui::toolbar` "Undo") | |
| Redo the last action | GUI/Buttons/ActionBar/ChangePanel/RedoButton.java | Redo. | **present** (`ui::toolbar` "Redo") | |
| Selection mode (Rectangular/Freeform; right click swaps) | GUI/Buttons/ActionBar/ChangePanel/MultiSelectButton.java | Shape of the right-drag selection; persisted `UseRectangularSelection` (true). | **partial** (`ui::toolbar` "Rect"/"Lasso", `App::lasso`) | Not persisted (session only). |
| Cut all node in the selected area | GUI/Buttons/ActionBar/ChangePanel/CutButton.java | Cut. | **missing** | Shortcut exists (2.3); button missing. |
| Copy all node in the selected area | GUI/Buttons/ActionBar/ChangePanel/CopyButton.java | Copy. | **missing** | Same. |
| Paste selection into the centre of viewport | GUI/Buttons/ActionBar/ChangePanel/PasteButton.java | Paste. | **missing** | Same. |
| Map (globe) button + popup | GUI/Buttons/ActionBar/ConfigPanel/MapInfoButton.java | Shows map name, map-image status, heightmap status and map scale; the globe icon shows the worst status. | **missing** | Rust shows some image/heightmap notices as top-bar labels (`doc.notice`), not this panel. |
| Autosave toggle + popup "Interval (Minutes)" spinner 5-60 + confirm | GUI/Buttons/ActionBar/ConfigPanel/AutosaveButton.java | Turns autosave on/off and sets the interval. | **partial** (`settings.toml` `global.autosave`, `global.autosave_minutes`; `App::tick_autosave`) | No UI; file edit only. |
| Select Hidden Nodes/Connections (H) | GUI/Buttons/ActionBar/ConfigPanel/SelectHiddenButton.java | Whether hidden nodes/connections can be selected; persisted `SelectHidden` (false). | **missing** | Depends on the hide toggles (2.8). |
| Grid toggle (G) | GUI/Buttons/ActionBar/ConfigPanel/GridDisplayButton.java | Shows the grid; persisted `ShowGrid` (false). | **partial** (`ui::top_bar` "Grid" checkbox, `MapView::show_grid`, start value `global.show_grid`) | No G shortcut; a change is not saved. |
| Grid popup: 1x1x1 / 2x2x1 / 4x4x1 / Custom (X spacing, Y spacing, subdivisions 1-32, Apply) | GUI/Buttons/ActionBar/ConfigPanel/GridDisplayButton.java | Grid spacing preset or custom values, stored per map. | **partial** (`MapSettings::grid_x/grid_y` -> `MapView::grid`) | Spacing only via `settings.toml`; no presets, no subdivisions, no UI. |
| Grid snapping (S; left click toggles) | GUI/Buttons/ActionBar/ConfigPanel/GridSnapButton.java | Snap to grid; persisted `GridSnapEnabled` (false). | **partial** (`ui::toolbar` "Snap", `Action::ToggleSnap`, S; Shift inverts) | Not persisted. |
| Snap mode grid/subdivision (right click; D) | GUI/Buttons/ActionBar/ConfigPanel/GridSnapButton.java | Snap to subdivisions instead of grid lines; persisted `SnapSubDivision` (false). | **missing** | No subdivisions in Rust. |
| Increase node size (numpad +) | GUI/Buttons/ActionBar/ConfigPanel/NodeSizeUpButton.java | Node size +0.1, stored per map. | **missing** | Per-map `node_size` read from `settings.toml` only. |
| Decrease node size (numpad −) | GUI/Buttons/ActionBar/ConfigPanel/NodeSizeDownButton.java | Node size −0.1 (min 0.1), stored per map. | **missing** | |
| Rotation Snapping toggle + popup "Step (Degrees)" 5-90 | GUI/Buttons/ActionBar/ConfigPanel/RotationSnapButton.java | Rotation snap on/off and step; persisted `RotationSnapEnabled` (true), `RotationStep` (5). | **partial** (`tools::rotate::snap_angle`) | Rotation snaps with the shared Snap toggle at a fixed 5°; no own toggle, no step. |
| Continuous connections (;) | GUI/Buttons/ActionBar/ConfigPanel/ContinuousConnectButton.java | After a connection the end node starts the next; persisted `Continuous_Connection` (false). | **partial** (`ui::toolbar` "Continuous", `Action::ToggleContinuous`) | Not persisted. |
| Open Config Window | GUI/Buttons/ActionBar/ConfigPanel/ConfigButton.java | Opens the Editor Config window. | **missing** | See 2.10. |

### 2.10 Config window ("Editor Config", `GUI/Config/ConfigGUI.java`)

A 620x460 window with a list on the left (Editor, AutoSave, Curves, Connections, Colours, Move
Widget, Themes, KeyBinds, and Experimental only with `-EXPERIMENTAL`). Rust has no settings window:
**missing** as a whole. All Java values are stored in `EditorConfig.xml` in the working directory
(element names below); Rust keys are in `settings.toml`.

#### 2.10.1 Editor tab

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Use Online Images | GUI/Config/Tabs/EditorTab.java | Download a missing map image. Default true, key `Check_Online_MapImages`. | **partial** (`global.online_map_images`, `jobs::download_wanted`) | No UI. |
| Middle Mouse Move | GUI/Config/Tabs/EditorTab.java | Pan with the middle button instead of the left. Default false, key `MiddleMouseMove`. | **partial** (`view.rs`: middle drag and primary drag both pan) | No option; both buttons always pan. |
| Interpolate Zoom In/Out | GUI/Config/Tabs/EditorTab.java | Smooth zoom animation. Default true, key `InterpolateZoom`. | **missing** | Rust zoom steps at once (`MapView` zoom). |
| Max Zoom Level (spinner 1-250) | GUI/Config/Tabs/EditorTab.java | Zoom limit. Default 30, key `MaxZoomLevel` (also per map). | **partial** (`lod::MAX_ZOOM` = 30) | Fixed constant. |

#### 2.10.2 AutoSave tab

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| AutoSave Enabled | GUI/Config/Tabs/AutoSaveTab.java | Default true, key `AutoSave_Enabled`. | **partial** (`global.autosave`) | No UI. |
| Autosave Interval (slider 0-60, step 5) | GUI/Config/Tabs/AutoSaveTab.java | Default 10 min, key `AutoSave_Interval`. | **partial** (`global.autosave_minutes`, min 1) | No UI. The action-bar spinner allows 5-60. |
| Max Autosave Slots (slider 1-10) | GUI/Config/Tabs/AutoSaveTab.java | Default 10, key `AutoSave_Slots`. | **partial** (`global.autosave_slots`, min 1) | No UI. |
| Apply Now / Apply On Restart | GUI/Config/Tabs/AutoSaveTab.java | Restart the autosave thread now or keep values for the next start. | **missing** | Not needed if Rust applies settings live (the tick reads settings each frame). |
| (no UI) last used slot | XMLConfig/AutoSave.java, GameXML.autoSaveGameConfigFile | Rotating slot counter, key `AutoSave_Last_Used_Slot`; files `<config>_autosave_<n>.xml`. | **present** (`jobs::next_slot`, `jobs::autosave_path`) | Rust derives the slot from the files; same file names. |

#### 2.10.3 Curves tab

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Maximum Nodes (slider 0-50) | GUI/Config/Tabs/CurvesTab.java | Max of the curve panel's interpolation slider. Default 50, key `CurveSliderMaximum`. | **partial** (`tools::bezier::MAX_POINTS` = 50) | Fixed. |
| Default Nodes (slider 0-50) | GUI/Config/Tabs/CurvesTab.java | Start value of interpolation points. Default 6, key `CurveSliderDefault`. | **partial** (`bezier::DEFAULT_POINTS` = 6) | Fixed. |
| Default Priority (Regular / SubPrio) | GUI/Config/Tabs/CurvesTab.java | New curve's node flag. Default regular, key `CurveNodeDefaultPriority`. | **partial** (`CurveSettings::default`, chosen per curve in `ui::curve_controls`) | Default not configurable. |
| Default Type (Normal / Reverse / Dual) | GUI/Config/Tabs/CurvesTab.java | New curve's connection type. Default REGULAR, key `CurveDefaultConnection`. | **partial** (same) | |
| Control Point Movement Scaler (slider 1-10) | GUI/Config/Tabs/CurvesTab.java | Control point drag factor. Default 3, key `ControlPointMoveScaler`. | **partial** (`bezier::MOVE_SCALER` = 3) | Fixed. |
| (no UI) control points default / max | XMLConfig/EditorXML.java | Keys `NumControlPoints` (1), `MaxControlPoints` (3). | **partial** (`bezier::DEFAULT_CONTROLS` = 1, `MAX_CONTROLS` = 4) | Rust max differs (4). |

#### 2.10.4 Connections tab

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Create End Node | GUI/Config/Tabs/ConnectionsTab.java | A connect click on empty map ends at a new node. Default false, key `CreateLinearLineEndNode`. | **partial** (`global.create_line_end_node`) | No UI. |
| Display Solid Arrows | GUI/Config/Tabs/ConnectionsTab.java | Filled or outline arrowheads. Default true, key `FilledConnectionArrows`. | **missing** | Rust always fills (`view.rs` mesh). |
| Linear Line Spacing (slider 0-50, min 5) | GUI/Config/Tabs/ConnectionsTab.java | Node spacing on connect lines. Default 12 m, key `LinearLineNodeDistance`. | **partial** (`global.line_node_distance`, min 1) | No UI; Rust minimum 1, Java 5. |
| Hidden Connections Transparency (slider 0-100 %) | GUI/Config/Tabs/ConnectionsTab.java | Alpha of hidden items. Default 100 %, key `HiddenNodeTransparency` (1.0). | **missing** | Needs the hide toggles. |

#### 2.10.5 Colours tab

One list of colour entries, a colour wheel, a connection preview and "Reset To Default". Saved under
`<EditorColours>`.

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Nodes: Regular | GUI/Config/Tabs/ColoursTab.java | Red, key `Colour_Node_Regular`. | **partial** (`look::NODE_REGULAR`) | Fixed constant; same colour. |
| Nodes: SubPrio | GUI/Config/Tabs/ColoursTab.java | Orange, key `Colour_Node_Subprio`. | **partial** (`look::NODE_SUBPRIO`) | Fixed. |
| Nodes: Selected | GUI/Config/Tabs/ColoursTab.java | White, key `Colour_Node_Selected`. | **partial** (`view.rs` selection mark `Color32::WHITE`) | Fixed. |
| Nodes: Control Node | GUI/Config/Tabs/ColoursTab.java | Magenta on load (field default 216,0,255), key `Colour_Node_Control`. | **partial** (Bézier handles drawn in fixed white in `tools::bezier`) | Fixed, and a different colour from Java's. |
| Connection: Regular | GUI/Config/Tabs/ColoursTab.java | Green, key `Colour_Connection_Regular`. | **partial** (`look::REGULAR`) | Fixed. |
| Connection: SubPrio | GUI/Config/Tabs/ColoursTab.java | Orange, key `Colour_Connection_Subprio`. | **partial** (`look::SUBPRIO`) | Fixed. |
| Connection: Dual | GUI/Config/Tabs/ColoursTab.java | Blue, key `Colour_Connection_Dual`. | **partial** (`look::DUAL`) | Fixed. |
| Connection: Dual SubPrio | GUI/Config/Tabs/ColoursTab.java | (150,100,50), key `Colour_Connection_Dual_Subprio`. | **partial** (`look::DUAL_SUBPRIO`) | Fixed. |
| Connection: Reverse | GUI/Config/Tabs/ColoursTab.java | Cyan, key `Colour_Connection_Reverse`. | **partial** (`look::REVERSE`) | Fixed. |
| Connection: Reverse SubPrio | GUI/Config/Tabs/ColoursTab.java | Cyan on load (field default 0,140,140), key `Colour_Connection_Reverse_Subprio`. | **partial** (`look::connection_colour` uses `REVERSE`) | Fixed. |
| Grid: Grid Lines | GUI/Config/Tabs/ColoursTab.java | (25,25,25), key `Colour_Grid`. | **partial** (`overlay::GRID_RGB`) | Fixed. |
| Reset To Default | GUI/Config/Tabs/ColoursTab.java | Restores the defaults above. | **missing** | |

#### 2.10.6 Move Widget tab

Saved under `<MoveWidget>`. The Rust move tool drags the node itself and has no widget, so every
row is **missing**.

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Axis Length | GUI/Config/Tabs/MoveWidgetTab.java | Default 30, key `AxisLength`. | **missing** | |
| Axis Width | GUI/Config/Tabs/MoveWidgetTab.java | Default 5, key `AxisWidth`. | **missing** | |
| Arrow Length | GUI/Config/Tabs/MoveWidgetTab.java | Default 24, key `ArrowLength`. | **missing** | |
| Arrow Width | GUI/Config/Tabs/MoveWidgetTab.java | Default 7, key `ArrowWidth`. | **missing** | |
| X Axis Colour / Y Axis Colour / Free Axis Colour (+ Reset) | GUI/Config/Tabs/MoveWidgetTab.java | Red / blue / green, keys `X_AxisColor`, `Y_AxisColor`, `FreeMoveColor`. | **missing** | |
| X Axis Left/Right, Y Axis Up/Down | GUI/Config/Tabs/MoveWidgetTab.java | Defaults RIGHT, UP; keys `X_Direction`, `Y_Direction`. | **missing** | |
| Position Center / Manual (drag in preview) | GUI/Config/Tabs/MoveWidgetTab.java | Default MANUAL, offset (-20, 30); keys `FreeMoveLocation`, `FreeMoveOffsetX`, `FreeMoveOffsetY`. | **missing** | |
| Type Square / Round | GUI/Config/Tabs/MoveWidgetTab.java | Default SQUARE, key `FreeMoveType`. | **missing** | |
| Style Solid / Outline / Pattern | GUI/Config/Tabs/MoveWidgetTab.java | Default PATTERN, key `FreeMoveStyle`. | **missing** | |
| Size Small / Medium / Large | GUI/Config/Tabs/MoveWidgetTab.java | Default SMALL on load (field MEDIUM); sizes 12/16/20 keys `FreeMoveSize`, `FreeMoveDefaultSmall/Medium/Large`. | **missing** | |

#### 2.10.7 Themes tab

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Theme list (FlatLaf, User, IntelliJ, Material groups) | GUI/Config/Tabs/ThemeTab.java, Managers/ThemeManager.java | Applies a FlatLaf theme at once; user themes from a `Themes` folder. Default `com.formdev.flatlaf.FlatLightLaf`, stored with `java.util.prefs` key `currentTheme` (not in EditorConfig.xml). | **missing** | Rust uses eframe's default visuals; no choice. |
| Theme info (Name, Type dark/light, Author) and FormDev link | GUI/Config/Tabs/ThemeTab.java | Shows the selected theme's details. | **missing** | Suggest drop with the theme list. |

#### 2.10.8 KeyBinds tab

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Table Action / Shortcut (29 rows, see 2.11) | GUI/Config/Tabs/KeyBindsTab.java | Lists all bindable actions. | **missing** | Rust `input::SHORTCUTS` is a fixed const table. |
| Per-row Clear / Reset | GUI/Config/Tabs/KeyBindsTab.java, Classes/UI_Components/ShortcutTable/* | Clears or restores one binding. | **missing** | |
| Input New Keybind popup ("Press a key or key combination", Escape cancels, "Key combination in use.. Try again") | GUI/Config/Tabs/KeyBindsTab.java | Captures a new key; refuses one in use. | **missing** | |
| Clear All / Reset All | GUI/Config/Tabs/KeyBindsTab.java | Clears or restores all bindings. | **missing** | |

#### 2.10.9 Experimental tab (only with `-EXPERIMENTAL`)

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Show Selection Bounds | GUI/Config/Tabs/ExperimentalTab.java | Draws selection bounds; not persisted. | **missing** | Suggest drop. |

### 2.11 Key bindings

Bindable defaults from `Managers/ShortcutManager.java` (XML id in `<Shortcuts><KeyBind ID Key
Modifier Name>`), then the fixed menu accelerators and other keys. Rust column checks
`input::SHORTCUTS`.

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Move Node: M (`MOVE_NODE`) | Managers/ShortcutManager.java | Move tool. | **present** (`Action::MoveTool`) | |
| Delete: Delete (`DELTE_NODE`, sic) | Managers/ShortcutManager.java | Delete tool / hovered node. | **present** (`Action::DeleteHovered`) | |
| New LinearLine: L (`NEW_LINE`) | Managers/ShortcutManager.java | Connect tool, cycles the three. | **present** (`Action::ConnectTool`) | |
| New Node: N (`NEW_NODE`) | Managers/ShortcutManager.java | Node tool, cycles regular/sub-prio. | **present** (`Action::NodeTool`) | |
| Node Priority: P (`SWAP_PRIORITY`) | Managers/ShortcutManager.java | Priority tool. | **present** (`Action::PriorityTool`) | |
| Flip Connection: F (`FLIP_CONNECT`) | Managers/ShortcutManager.java | Flip tool. | **present** (`Action::FlipTool`) | |
| Rotate Nodes: R (`ROTATE_NODES`) | Managers/ShortcutManager.java | Rotate tool. | **present** (`Action::RotateTool`) | |
| Create Bezier Curve: B (`BEZIER_CURVE`) | Managers/ShortcutManager.java | Bézier tool. | **present** (`Action::BezierTool`) | |
| Add Marker: W (`ADD_MARKER`) | Managers/ShortcutManager.java | Add marker. | **present** (`Action::AddMarker`) | Rust acts on the hovered node whatever tool. |
| Edit Marker: E (`EDIT_MARKER`) | Managers/ShortcutManager.java | Edit marker. | **present** (`Action::EditMarker`) | |
| Delete Marker: Shift+D (`DELETE_MARKER`) | Managers/ShortcutManager.java | Delete marker. | **present** (`Action::DeleteMarker`) | |
| Align Nodes Horizontally: Shift+X (`ALIGN_HORIZ`) | Managers/ShortcutManager.java | Align H tool. | **present** (`Action::AlignHorizontalTool`) | |
| Align Nodes Vertically: Shift+Y (`ALIGN_VERT`) | Managers/ShortcutManager.java | Align V tool. | **present** (`Action::AlignVerticalTool`) | |
| Focus Node: Shift+F (`FOCUS_NODE`) | Managers/ShortcutManager.java, GUI/MapPanel.java | Centres the view on the hovered node. | **missing** | |
| Cut: Ctrl+X (`CUT`) | Managers/ShortcutManager.java | Listed but never wired in Java (Cut menu hard-codes Ctrl+X). | **present** (`Action::Cut`) | Rebinding it in Java does nothing. |
| Copy: Ctrl+C (`COPY`) | Managers/ShortcutManager.java | Copy (wired to the Copy menu and button). | **present** (`Action::Copy`) | |
| Paste: Ctrl+V (`PASTE`) | Managers/ShortcutManager.java | Listed but never wired (menu hard-codes Ctrl+V). | **present** (`Action::Paste`) | |
| Paste Original Location: Shift+V (`PASTE_ORIGINAL`) | Managers/ShortcutManager.java | Listed but never wired (menu hard-codes Shift+V). | **present** (`Action::PasteInPlace`) | |
| Toggle Select Hidden: H (`SELECT_HIDDEN`) | Managers/ShortcutManager.java | Select-hidden toggle. | **missing** | |
| Toggle Show Grid: G (`SHOW_GRID`) | Managers/ShortcutManager.java | Grid toggle. | **missing** | Checkbox only. |
| Toggle Grid Snapping: S (`GRID_SNAP`) | Managers/ShortcutManager.java | Grid snap toggle. | **present** (`Action::ToggleSnap`) | Rust adds: Shift held inverts snap. |
| Toggle Subdivision Snapping: D (`SUB_SNAP`) | Managers/ShortcutManager.java | Snap mode toggle. | **missing** | |
| Node Size Up: numpad + (`NODE_UP`) | Managers/ShortcutManager.java | Node size up. | **missing** | |
| Node Size Down: numpad − (`NODE_DOWN`) | Managers/ShortcutManager.java | Node size down. | **missing** | |
| Toggle Continuous Connection: ; (`CONTINUOUS_CONNECT`) | Managers/ShortcutManager.java | Continuous toggle. | **present** (`Action::ToggleContinuous`, also Shift+; for German layouts) | |
| Toggle Marker Names: F1 (`MARKER_NAMES`) | Managers/ShortcutManager.java | Marker names. | **missing** | |
| Toggle Marker Icons: F2 (`MARKER_ICONS`) | Managers/ShortcutManager.java | Marker icons. | **missing** | |
| Toggle Parking Icons: F3 (`PARKING_ICONS`) | Managers/ShortcutManager.java | Parking icons. | **missing** | |
| Toggle Show Node ID: F4 (`NODE_ID_HOVER`) | Managers/ShortcutManager.java | Node ID in hover info. | **present** (`Action::ToggleNodeId`) | |
| Undo: Ctrl+Z (menu accelerator, not bindable) | GUI/Menus/EditMenu/UndoMenu.java | Undo. | **present** (`Action::Undo`) | |
| Redo: Shift+Z (menu accelerator, not bindable) | GUI/Menus/EditMenu/RedoMenu.java | Redo. | **present** (`Action::Redo`, Ctrl+Shift+Z) | Changed per R14. |
| Open Config: Alt+L | GUI/Menus/FileMenu/OpenConfigMenu.java | Open. | **missing** | Common choice would be Ctrl+O (decision). |
| Save Config: Alt+S | GUI/Menus/FileMenu/SaveConfigMenu.java | Save. | **missing** | `input.rs` test keeps Ctrl+S free "for saving". |
| Save As: Alt+A | GUI/Menus/FileMenu/SaveAsConfigMenu.java | Save As. | **missing** | |
| Exit: Alt+Q | GUI/Menus/FileMenu/ExitEditorMenu.java | Exit. | **missing** | |
| Import Images from .zip: Alt+Z | GUI/Menus/ImportMenu/ImportFromZipMenu.java | Zip import. | **missing** | |
| Load Map Image: Alt+M | GUI/Menus/ImportMenu/LoadMapImageMenu.java | Load image. | **missing** | |
| Import FS19 PDA Image: Alt+I | GUI/Menus/ImportMenu/ImportFS19DDSMenu.java | FS19 DDS. | **missing** | |
| Import FS22 PDA Image: Alt+P | GUI/Menus/ImportMenu/ImportFS22DDSMenu.java | FS22 DDS. | **missing** | |
| Save As Default Map Image: Alt+B | GUI/Menus/ImportMenu/SaveMapImageMenu.java | Save image. | **missing** | |
| Show All Node ID's: Alt+6; Show Selected Node Info: Alt+7 | GUI/Menus/DebugMenu/*.java | Debug toggles. | **missing** | Drop with the Debug menu. |
| Menu mnemonics Alt+F, R, E, D, S, I, H, D (DEBUG) | GUI/Menus/EditorMenu.java | Open a menu by keyboard (Alt+D is used twice). | **missing** | egui menus have no mnemonics; decision. |
| Shift held (map panel) | GUI/MapPanel.java | Tracks Shift for tools. | **present** (`input::snap`, Shift inverts snap) | |
| F5 / F6 (only `-EXPERIMENTAL`) | AutoDriveEditor.java | FlatLaf UI inspectors. | **missing** | Suggest drop. |

### 2.12 Persistence

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| `EditorConfig.xml` in the working directory | XMLConfig/EditorXML.java | Loaded at start, written on exit (`windowClosing`); holds all settings, shortcuts, recent files and per-map info. | **partial** (`ad-io settings.rs`, `%APPDATA%\AutoDrive Course Editor\config\settings.toml`) | Rust reads at start, writes only a missing file; never saves changes. No import of an old `EditorConfig.xml`. |
| Version element | XMLConfig/EditorXML.java | Writes `Version` = editor version. | **missing** | Only useful for migration. |
| Window position and size | XMLConfig/EditorXML.java | Keys `WindowX`, `WindowY` (-99 = centre), `WindowWidth` (1280), `WindowHeight` (960). | **missing** | `main` uses `eframe::NativeOptions::default()`; eframe persistence is not used. |
| Last used folder | XMLConfig/EditorXML.java | Key `LastUsedLocation`; start folder of every file chooser. | **missing** | Rust Save starts in the document's folder; Open starts in the OS default. |
| Recent files | XMLConfig/EditorXML.java, GUI/Menus/FileMenu/RecentFilesMenu.java | `<RecentFiles><Recent Path Type>` (Type 1 = config, 2 = route), max 10. | **missing** | |
| Key bindings | XMLConfig/EditorXML.java | `<Shortcuts><KeyBind ID Key Modifier Name>`; missing ids get defaults. | **missing** | |
| Per-map info | XMLConfig/EditorXML.java (`MapInfoStore`) | `<KnownMapSettings><Map Name>` with `MapScale` (1), `NodeSize` (2), `GridType` (new map: GRID_2x2), `GridX`/`GridY`/`GridSubDivisions` or `CustomGridX`/`CustomGridY`/`CustomSubDivisions`, `MaxZoomLevel` (30); old `<mapzoomfactor>` format migrated. | **partial** (`settings::MapSettings` `[maps.<name>]` `scale`, `node_size`, `grid_x`, `grid_y`) | Read only; no grid type, subdivisions or max zoom; no entry is created for a new map. |
| Global node size | XMLConfig/EditorXML.java | Key `NodeSizeScale` (2). | **partial** (`MapSettings::node_size` default 2.0) | Rust keeps node size per map only. |
| Toggle states | XMLConfig/EditorXML.java | `ShowGrid`, `GridSnapEnabled`, `SnapSubDivision`, `RotationStep`, `RotationSnapEnabled`, `Continuous_Connection`, `UseRectangularSelection`, `SelectHidden`, `ShowMarkerNames`, `ShowMarkerIcons`, `ShowParkingIcons`, `ShowSelectedNodeID`. | **partial** (`global.show_grid` read at start) | All others are session-only or missing in Rust. |
| Panel visibility | XMLConfig/EditorXML.java | `TextPanelVisible`, `UndoRedoVisible`, `CopyPasteVisible`. | **missing** | Suggest drop with the panels. |
| Config-tab values | XMLConfig/EditorXML.java | Keys listed in 2.10.1-2.10.6 (`<EditorColours>`, `<MoveWidget>` sub-elements). | **partial** (`global.online_map_images`, `autosave`, `autosave_minutes`, `autosave_slots`, `create_line_end_node`, `line_node_distance`) | The rest missing (see each tab). |
| Update-message flag | XMLConfig/EditorXML.java | `ShowUpdateMessage` (true). | **missing** | See 2.13. |
| Theme | Managers/ThemeManager.java | `java.util.prefs` node of `ThemeManager`, key `currentTheme` (Windows registry), not in EditorConfig.xml. | **missing** | |
| Undo history size | – | Not in Java. | **present** (`global.history_cap`, 500) | Rust only. |
| Log file | GUI/Menus/HelpMenu/LoggingGUIMenu.java | `AutoDriveEditor.log` beside the jar. | **present** (`startup::start_logging`, daily `editor.log` in `%APPDATA%\...\data\logs\`) | Different place, no viewer. |
| Locale | Locale/LocaleManager.java | Follows the system locale; only `en_US` exists; external `.properties` searched in a few folders. | **missing** | Rust UI strings are inline English (deferred-work Retro P2: no string table). Decision whether a string table is wanted. |

### 2.13 Update check

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Update check at start | Managers/VersionManager.updateCheck, AutoDriveEditor.java (`windowOpened`) | On window open (skipped with `-EXPERIMENTAL`) GETs `https://github.com/KillBait/AutoDrive_Course_Editor/raw/master/version.xml` (5 s connect timeout, spoofed Firefox User-Agent, on the UI thread), reads `latest_version` and `version_notes` (HTML), compares with `COURSE_EDITOR_VERSION` by semver. | **missing** | **Points at KillBait upstream; must become `weiserfuchs/AutoDrive_Course_Editor`.** A failed request leaves the stream null and the parse then fails outside the caught exception types (likely an uncaught `IllegalArgumentException`). |
| "Version Update" dialog | Managers/VersionManager.updateCheck | If remote is newer and `ShowUpdateMessage` is true: "A new version of AutoDrive Editor is available!", "GitHub Version vX -- you are running vY", release-notes HTML with clickable links. Then `ShowUpdateMessage` = false, so it shows once; it turns true again only when the versions are equal. | **missing** | Proposal: a background `ureq` job (as `ad-io/src/download.rs`) against `https://api.github.com/repos/weiserfuchs/AutoDrive_Course_Editor/releases/latest`, compare `tag_name` with `CARGO_PKG_VERSION`, show the release `body` and a link to the release page; remember the last version shown instead of a bool; a setting to turn the check off. |
| Version history source | Managers/VersionManager.showVersionHistory | Local `history.xml`, else `https://github.com/KillBait/AutoDrive_Course_Editor/raw/master/history.xml`. | **missing** | **Upstream URL.** Repo-root `version.xml` and `history.xml` are upstream content (`version.xml` links `KillBait/AutoDrive_Course_Editor/releases`). Proposal: Help ▸ Version history opens the fork's releases page or lists release notes from the same API. |

Other KillBait references found (for the policy check):

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Map image download URL | GUI/MapImage.java:328 | `raw.githubusercontent.com/KillBait/AutoDrive_MapImages/master/mapImages/<name>/<name>.png` | **present** (`ad-io/src/download.rs:8`) | Allowed: map-image data repo. |
| How to create map images links | GUI/Menus/HelpMenu/ImagesLinkMenu.java:24,27 | `github.com/KillBait/AutoDrive_MapImages` and `.../discussions/20`. | **missing** | Allowed (data repo); PO to confirm the discussion link counts. |
| About "by KillBait!" | GUI/Menus/HelpMenu/AboutMenu.java | Authorship line. | **missing** | Not a URL; rewrite for the fork, keep the credits. |
| Commented upstream link | GUI/Config/Tabs/ThemeTab.java:135 | Dead code, `github.com/KillBait/AutoDrive_Course_Editor`. | **missing** | Nothing to port. |
| Issue links in comments | GUI/Menus/ScanMenu/OutOfBoundsFixMenu.java:23, XMLConfig/RoutesXML.java:65 | Code comments only. | **missing** | Nothing to port. |

### 2.14 Other windows and panels

| Java item (visible label) | Java source file | What it does | Rust status | Notes |
|---|---|---|---|---|
| Routes Manager Config window (list; Load Config, Close) | GUI/RoutesGUI/RoutesGUI.java | Lists the routes of a `routes.xml`; loads the selected one. | **present** (`ui::routes_window`: list, "Open", "Close", Escape) | |
| Curve panel (Interpolation points, Control Points, Node Priority, Connection Type, swap, confirm, cancel) | GUI/Curves/CurvePanel.java | Edits the previewed curve. | **present** (`ui::curve_panel` / `curve_controls`: "Bézier curve" and "Arc" windows, "Swap direction", "Commit", "Cancel") | |
| Zip import "Preview" window | GUI/Menus/ImportMenu/ImportFromZipMenu.java | Map image and heightmap previews, Map Name, Game Version, Override, Import, Cancel. | **present** (`ui::zip_preview_window`) | |
| Marker add/edit dialog | GUI/Buttons/Toolbar/Markers/AddMarkerButton.java, EditMarkerButton.java | Name and group of a marker. | **present** (`ui::marker_dialog`) | |
| Edit location dialog | GUI/Buttons/Toolbar/Alignment/EditLocationButton.java | x/y/z fields. | **present** (`ui::location_dialog`) | |
| Unsaved-changes dialog (Yes/No) | AutoDriveEditor.java, GUI/Menus/FileMenu/OpenConfigMenu.java | "dialog_exit_unsaved" before close/open. | **present** (`ui::unsaved_prompt`: Save, Discard, Cancel) | |
| Text panel (bottom, 3 lines: "Welcome to the AutoDrive Editor…", tool info texts, results) | GUI/TextPanel.java | Shows tool hints and operation messages. | **partial** (`App::tool_notice`, `doc.notice`, labels in `ui::top_bar`) | No message history, no toggle. |
| Editor Log window | GUI/Menus/HelpMenu/LoggingGUIMenu.java | Tails the log file. | **missing** | See 2.7. |
| Map info popup | GUI/Buttons/ActionBar/ConfigPanel/MapInfoButton.java | Map name, image/heightmap status, scale. | **missing** | See 2.9. |
| About window | GUI/Menus/HelpMenu/AboutMenu.java | See 2.7. | **missing** | |
| Version History window (700x600 HTML) | Managers/VersionManager.java | See 2.13. | **missing** | |
| Version Update dialog | Managers/VersionManager.java | See 2.13. | **missing** | |
| Editor Config window | GUI/Config/ConfigGUI.java | See 2.10. | **missing** | |
| Window title "AutoDrive Course Editor - <path> *" (+ "( EXPERIMENTAL MODE )" / "( DEBUG MODE )") | AutoDriveEditor.java `createWindowTitleString` | Title with file and dirty mark. | **present** (`main::window_title`) | No mode suffixes (no such modes). |
| Error dialogs | various `JOptionPane` | Errors and file-type errors. | **present** (`ui::error_window`, panic dialog in `startup::install_panic_hook`) | |
| (Rust only) groups window, connection report, merge/fix prompts, replace-image prompt | – | Rust additions. | **present** (`ui::groups_window`, `connection_report`, `merge_prompt`, `fix_prompt`, `replace_image_prompt`) | |

## 3. Gaps

Counts over all inventory rows above (sections 2.1-2.14, including the Rust-only and KillBait
reference rows): **present 74, partial 51, missing 106** (231 rows). Of the missing rows, 36 are
items this report suggests not porting or that need nothing (3.2), most of them the Debug menu,
the Experimental items and the Move Widget tab.

### 3.1 Candidate story chunks

Flat list of every missing or partial item, grouped. Each heading is a candidate story.

1. **Settings persist and write back.** The base for every chunk below: the app saves
   `settings.toml` when a setting changes or on exit, and creates a `[maps.<name>]` entry for a new
   map. Covers: `EditorConfig.xml` write (2.12), toggle states (grid, snap, continuous, Rect/Lasso,
   marker names/icons/parking icons, node ID, select hidden, rotation snap and step), per-map info
   (scale, node size, grid type, X/Y, subdivisions, max zoom), global node size. Rationale: today no
   change survives a restart.
2. **Menu bar and Save vs Save As.** Replace the top-bar buttons by File (Open, Open Recent, Save,
   Save As, Exit), Edit (Undo, Redo, Cut, Copy, Paste, Paste Original Location), Display, Scan,
   Import, Routes, Help; Save writes in place, Save As asks (deferred-work Retro R4); Save enabled
   only while dirty. Covers 2.1 Save/Save As/Exit, 2.2 Save route, 2.3 Cut/Copy/Paste items.
   Rationale: the Java structure users know, and the Save gap is a known defect.
3. **Recent files and last used folder.** Open Recent (10 entries, config and route types, Clear
   List), file dialogs start in the last used folder. Rationale: daily workflow; small and
   self-contained once chunk 1 exists.
4. **Window position and size remembered.** Restore position/size (eframe persistence or own keys).
   Rationale: small, visible quality gap.
5. **Settings window with tabs.** One window (Config button and a menu entry) with Editor (online
   images, middle mouse mode, max zoom, interpolate zoom), AutoSave (enabled, interval, slots),
   Curves (max/default points, default priority/type, control-point scaler, default/max control
   points), Connections (create end node, solid arrows, line spacing, hidden transparency), and
   optionally Colours (11 colours + reset). Rationale: every value already in `settings.toml`
   needs a UI, and the fixed constants (`MAX_ZOOM`, `MAX_POINTS`, `DEFAULT_POINTS`, `MOVE_SCALER`,
   `look.rs` colours) become settings. Could split into "settings window + Editor/AutoSave tabs"
   and "Curves/Connections/Colours tabs".
6. **Configurable key bindings.** `SHORTCUTS` becomes defaults plus a `[shortcuts]` table in
   `settings.toml`; a KeyBinds tab with capture popup, in-use check, Clear/Reset per row,
   Clear All/Reset All; tooltips show the current binding. Rationale: R14 says shortcuts are
   configurable.
7. **Missing shortcuts and their actions.** G (grid), F1/F2/F3 (marker names, marker icons,
   parking icons), Shift+F (focus hovered node), numpad +/− (node size), H (select hidden), D
   (subdivision snap), plus file shortcuts (Open, Save, Save As, Exit, imports) once the PO picks
   Ctrl- or Alt-based keys. Rationale: Java defaults users rely on; most are one-liners on top of
   chunks 8-10.
8. **Display toggles and map scale.** Display menu: Set Map Scale (2 km-48 km, per map), Show
   Marker Names, Show Marker Icons, Show Parking Icons (new flag), Show Selected Node ID; node size
   up/down buttons. Rationale: the flags partly exist in `MapView` but have no UI.
9. **Visibility filters.** Show/Hide nodes and the four connection kinds (reset by double right
   click), Select Hidden toggle, hidden-transparency setting. Rationale: a whole Java toolbar group
   is missing; it touches view drawing and selection, so it is its own story.
10. **Grid and rotation snap options.** Grid popup (1x1, 2x2, 4x4, custom X/Y/subdivisions,
    per map), subdivision snap mode, a separate rotation-snap toggle with a step of 5-90°.
    Rationale: Rust has only one Snap toggle, a fixed 5° rotation step and file-only spacing.
11. **Map info panel.** Map name, map-image and heightmap status, map scale (globe button popup).
    Rationale: users see why no image shows; today only transient notices.
12. **Update check against weiserfuchs/AutoDrive_Course_Editor releases.** Background check of
    `api.github.com/repos/weiserfuchs/AutoDrive_Course_Editor/releases/latest` at start, dialog
    with the release notes and a link, shown once per new version, a setting to disable it.
    Rationale: R16 names it; the Java URL points at upstream and must not be reused.
13. **Help menu: About, version history, map-images help.** About window (version from
    `CARGO_PKG_VERSION`, fork authorship, credits), Version history (fork releases), How to create
    map images (links to the allowed data repo). Rationale: small, pairs with chunk 12.
14. **Log access.** Help ▸ Show Editor Log (a log window, or simply "Open log folder"), optionally a
    log-level setting replacing the 29 Debug logging toggles. Rationale: users send logs with bug
    reports.
15. **Message panel.** Optional bottom panel with a short history of notices and tool hints
    (Java text panel). Rationale: Rust notices vanish; low priority.
16. **Toolbar presentation.** Grouped, icon tool buttons with tooltips showing the shortcut;
    Cut/Copy/Paste buttons. Rationale: discoverability; Java tooltips doubled as shortcut help.
17. **Theme.** Light / dark / follow system, persisted. Rationale: the Java theme list is large;
    a simple choice covers the need (see 3.2).
18. **Go to node ID.** The Debug "Centre node in viewport" as a normal Edit or Display command.
    Rationale: useful outside debugging; tiny.

### 3.2 Suggested not to port (suggestion, product owner decides)

- **Debug menu and Enable Debug Menu** (11 overlays, 29 logging toggles, Alt+6/Alt+7, "( DEBUG
  MODE )" title): developer aids tied to Java managers; Rust has `tracing` logs and frame-time
  tests. Keep only "Go to node ID" (chunk 18) and maybe one log-level setting (chunk 14).
- **Experimental mode** (`-EXPERIMENTAL` flag, Experimental toolbar group with Test, Road
  connection and Scale buttons, `TemplateButton`, Experimental config tab "Show Selection Bounds",
  F5/F6 FlatLaf inspectors): unfinished or developer-only; Arc is already ported as a normal tool.
- **Move Widget tab** (all 2.10.6 settings): the Rust move tool drags nodes directly and has no
  widget. Porting needs the widget first; decide whether the widget itself is wanted.
- **Action bar ☰ panel toggles** (Show Undo/Redo, Show Cut/Copy/Paste and their persisted flags):
  only hide always-useful buttons.
- **FlatLaf theme catalogue** (IntelliJ/Material themes, user `Themes` folder, theme info and
  FormDev link): replace by light/dark/system (chunk 17).
- **AutoSave "Apply Now / Apply On Restart"**: only needed because Java runs a timer thread; Rust
  reads the settings each tick.
- **Alt-letter menu accelerators and mnemonics** (Alt+L/S/A/Q/Z/M/I/P/B, Alt+F…): egui menus have no
  mnemonics, and Alt+S / Alt+I clash with mnemonics in Java itself. Suggest standard Ctrl
  shortcuts for File actions instead (decision).
- **`EditorConfig.xml` import**: Rust starts from its own defaults; a one-time import is possible
  but probably not worth it (decision).

### 3.3 Decisions for the product owner

1. Which items in 3.2 are dropped (especially the Move Widget, the Debug menu and Experimental mode).
2. File shortcuts: keep Java's Alt+letter keys or use Ctrl+O / Ctrl+S / Ctrl+Shift+S / Ctrl+Q.
3. Menu bar (Java structure) vs the current single top bar.
4. Whether colours are configurable (Colours tab) or stay fixed Java defaults.
5. Theme scope: light/dark/system only, or more.
6. Version history: fork release notes (API) or a link to the releases page; the repo-root
   `version.xml` / `history.xml` are upstream content and should not be reused as is.
7. Whether the "How to create map images" link to `KillBait/AutoDrive_MapImages/discussions/20`
   is allowed under the data-repo exception.
8. Whether a string table / localisation is in scope (Java ships only en_US).
9. Whether old `EditorConfig.xml` settings are imported.
