---
title: 'Codebase research: road network data model and config format'
type: codebase
ask: |-
  Complete feature inventory of the Java/Swing AutoDrive Course Editor as a baseline; this run covers road network data model and config format.
repository: '.'
commit: 'e4fe43e93f225e8c6d7635d62fb8cfdfd845a971'
branch: 'master'
status: complete
validation: 'normal'
claims: {verified: 33, disputed: 3, corrected_in_text: 3}
created: '2026-11-05'
updated: '2026-11-05'
---

# Codebase research: road network data model and config format

## Questions

1. (how) How is an AutoDrive config XML file read into memory: which elements and attributes are parsed, and how do they become objects?
2. (how) How is the in-memory road network written back to a config XML file (element order, number formatting, encoding)?
3. (how) How is the road network represented in memory: node, connection, marker, group and flag types, and the structures that hold and index them (incl. KDTree)?
4. (how) How does the code tell FS19, FS22 and FS25 config files apart, and what differs in their handling?
5. (how) How is this code tested?

## 1. How is an AutoDrive config XML file read into memory: which elements and attributes are parsed, and how do they become objects?

The menu actions call `loadGameConfig`: src/main/java/AutoDriveEditor/GUI/Menus/FileMenu/OpenConfigMenu.java:53 (actionPerformed) @ e4fe43e and src/main/java/AutoDriveEditor/GUI/Menus/FileMenu/RecentFilesMenu.java:39-40 (recentMenuItemListener) @ e4fe43e. `loadGameConfig` calls `loadGameXMLFile` and then applies the post-load steps: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:68-108 (loadGameConfig) @ e4fe43e. `loadGameXMLFile` does the whole parse with a plain DOM `DocumentBuilder`: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:163-167 (loadGameXMLFile) @ e4fe43e. The `XMLUtils.parseXmlFile` helpers are not used on this path.

**Root and version.** The root element must be `AutoDrive`. Otherwise the load returns null and the "config unknown" dialog appears. A `<markerID>` element marks a config older than v1.1.0.6: a warning appears, `canEditConfig` is set to false, `configVersion` is left unset, and the load continues. The earlier wording "opens view-only" was not borne out by these lines; where `canEditConfig` is enforced was not read. Otherwise the root `<version>` is parsed with Semver. Major 1 maps to FS19, 2 to FS22 and 3 to FS25: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:172-200 (loadGameXMLFile) @ e4fe43e.

**No attributes.** The loader reads no XML attributes from the game config. All data is element text.

**`<waypoints>`.** It holds parallel lists inside `<id>`, `<x>`, `<y>`, `<z>`, `<out>`, `<incoming>` and `<flags>`: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:248-333 (loadGameXMLFile) @ e4fe43e.
- The number of `<id>` values defines the node count N. Every list must have the same count; a mismatch throws `EntryTotalException`, which shows a dialog naming the tag.
- x, y and z are comma-separated doubles.
- `<out>` and `<incoming>` hold one `;`-separated group per node. Each group is a comma-separated list of 1-based node ids, and `-1` means no connection.
- `<flags>` is optional. When it is missing, every flag is 0 and the static `hasFlagTag` is set to false.
- The `<id>` values are only sequence-checked, and that check has no effect. The node id is always `i+1`: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:256-268 (loadGameXMLFile) @ e4fe43e, src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:337-339 (loadGameXMLFile) @ e4fe43e.

**Node objects.** Each node is built with `MapNode.createMapNode(id,x,y,z,flag,false)`: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:354 (loadGameXMLFile) @ e4fe43e. Both that factory and the constructor round the coordinates to 3 decimals: src/main/java/AutoDriveEditor/RoadNetwork/MapNode.java:180-187 (createMapNode) @ e4fe43e, src/main/java/AutoDriveEditor/RoadNetwork/MapNode.java:140-169 (MapNode) @ e4fe43e.

**Connections.** `<out>` entries fill `mapNode.outgoing` and `<incoming>` entries fill `mapNode.incoming`, each resolved by `nodes.get(n-1)`. A `Connection` object is then built for each outgoing link, with its type from `RoadMap.getConnection`; links of type UNKNOWN are skipped: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:360-395 (loadGameXMLFile) @ e4fe43e.

**`<mapmarker>`.** The loader reads the parallel `<id>`, `<name>` and `<group>` lists inside it, not the `<mmN>` wrappers. The marker id is parsed with `Float.parseFloat` and cast to int, so values such as `194.000000` are accepted. The marker is attached with `nodes.get(id-1).createMapMarker(name, group, markerId, parkedVehicles)`, where `markerId` is the marker's position in the file: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:418-459 (loadGameXMLFile) @ e4fe43e.

**`<MapName>`.** It goes into `RoadMap.mapName`. `new RoadMap()` resets the static node list, mapName and uuid, and `setRoadMapNodes` stores a `LinkedList` copy of the parsed nodes: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:464-476 (loadGameXMLFile) @ e4fe43e, src/main/java/AutoDriveEditor/RoadNetwork/RoadMap.java:27-33 (RoadMap) @ e4fe43e.

**Parking data from `vehicles.xml`.** Park destinations come from a `vehicles.xml` in the same folder, read with XPath. The result maps a marker id to a list of vehicle ids; a missing file gives an empty map: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:739-799 (loadVehiclesXMLParking) @ e4fe43e.

**Elements the loader ignores.** The fixture has many top-level elements that are never read: `ADRouteVersion`, `ADRouteAuthor`, `currentDebugChannelMask` and the mod settings (`*_userDefault`, `Pathfinder`, `driverWages` and others). They survive a save because the save re-parses the original file (section 2).

**Error handling.**
- A wrong root, or a list-count mismatch, gives a specific dialog.
- Everything else falls into the catch in `loadGameConfig` and shows "config load failed": src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:104-107 (loadGameConfig) @ e4fe43e. That covers number-format errors, null pointers from missing elements, an unguarded `new Semver(null)`, and marker ids outside the node list.

**State after a successful load.** `loadGameConfig` sets `configType`, `xmlConfigFile`, the RoadMap on the map panel and the session uuid. It loads the map image and heightmap by map name, clears the undo history, runs the overlap scan, clears the multi-selection, enables menus and buttons, and adds the file to the recent-files list: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:73-100 (loadGameConfig) @ e4fe43e.

Not read for this question: `RoutesXML.loadRoutesXML` (the separate routes format), the `MapMarker` class body, `RoadMap.getConnection`, and where `canEditConfig` is enforced.

## 2. How is the in-memory road network written back to a config XML file (element order, number formatting, encoding)?

`saveGameConfig` wraps the write. It returns silently when no file is loaded, and shows a success or failure dialog: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:110-131 (saveGameConfig) @ e4fe43e. Callers include src/main/java/AutoDriveEditor/GUI/Menus/FileMenu/SaveConfigMenu.java:23 (actionPerformed) @ e4fe43e and src/main/java/AutoDriveEditor/GUI/Menus/FileMenu/SaveAsConfigMenu.java:54 (actionPerformed) @ e4fe43e.

**In-place edit of the original file.** `saveGameXMLFile` re-parses the existing config file from disk and edits that DOM: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:485-491 (saveGameXMLFile) @ e4fe43e.
- Element order is the order already on disk. The children of `<waypoints>` get new text by name (`id`, `x`, `y`, `z`, `incoming`, `out`, `flags`).
- The only element created is `<flags>`, which is appended when the source had none: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:494-499 (saveGameXMLFile) @ e4fe43e, src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:503-608 (saveGameXMLFile) @ e4fe43e.
- All other root content (version, MapName, settings) is kept as it was.

**Ids.** There is no renumbering step. Each node's `id` is written in list order; a mismatch is only logged: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:511-521 (saveGameXMLFile) @ e4fe43e.

**Separators.**
- `id`, `x`, `y`, `z` and `flags` are comma-joined.
- `incoming` and `out` use `,` within a node and `;` between nodes, and write `-1` for a node without connections: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:558-572 (saveGameXMLFile) @ e4fe43e.

**Number formatting.** x, y and z are written with `String.valueOf(double)`, Java's shortest round-trip form: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:529 (saveGameXMLFile) @ e4fe43e. The fixture stores three decimals (`100.000`), which comes back as `100.0`. Values below 1e-3 or at least 1e7 switch to E-notation.

**Markers.**
- The children of every existing `mm<N>` element's parent are removed. A `<mapmarker>` element is created when missing.
- `mm1..mmK` are appended in node-list order, each with `id`, `name` and `group`: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:612-669 (saveGameXMLFile) @ e4fe43e.
- Marker ids are written as plain ints, so the fixture's `194.000000` becomes `194` on the first save.

**Encoding and layout.**
- The Transformer uses UTF-8 encoding, `INDENT=yes` and an indent of 2: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:673-677 (saveGameXMLFile) @ e4fe43e.
- Before the write, every whitespace-only text node is removed with XPath: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:681-689 (saveGameXMLFile) @ e4fe43e. The fixture's 4-space indent therefore becomes a 2-space indent.
- The writer sets no BOM, no `standalone` flag and no line separator. The fixture starts with a UTF-8 BOM and uses LF line endings. The analyzer did not run the code, so the JDK's actual declaration and line endings were not checked.

**Target file.** A null `newName` writes to `xmlConfigFile`. Otherwise the output goes to `newName`, and only a real Save As (not autosave or backup) repoints `xmlConfigFile`: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:693-705 (saveGameXMLFile) @ e4fe43e.

**`vehicles.xml`.** Only a real save writes it. The writer re-parses `vehicles.xml` and rewrites the `parkDestination` attributes: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:719-721 (saveGameXMLFile) @ e4fe43e, src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:812-886 (saveVehiclesXMLParking) @ e4fe43e.

**Autosave.**
- A single-thread scheduler runs at a fixed rate, by default every 10 minutes: src/main/java/AutoDriveEditor/XMLConfig/AutoSave.java:28-43 (startAutoSaveThread) @ e4fe43e.
- The defaults come from src/main/java/AutoDriveEditor/XMLConfig/EditorXML.java:78-79 (EditorXML) @ e4fe43e: 10 minutes and 10 slots.
- The file name is `<config>_autosave_<slot>.xml`, and the slot number wraps around: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:133-156 (autoSaveGameConfigFile) @ e4fe43e.

**Merge backup.** Before a node merge, the editor writes `<config>_mergeBackup.xml`: src/main/java/AutoDriveEditor/Managers/ScanManager.java:272-279 (saveMergeBackupConfigFile) @ e4fe43e.

**Route-manager format.** It is written from scratch with root `routeExport`, using attributes and `;`-separated lists and an indent of 4: src/main/java/AutoDriveEditor/XMLConfig/RoutesXML.java:343-516 (saveRouteXML) @ e4fe43e.

Not found: no atomic write (no temp file and rename) and no `.bak` of the original before a save.

## 3. How is the road network represented in memory: node, connection, marker, group and flag types, and the structures that hold and index them (incl. KDTree)?

**Container.** The whole network is one static `LinkedList<MapNode> networkNodesList`: src/main/java/AutoDriveEditor/RoadNetwork/RoadMap.java:22 (RoadMap) @ e4fe43e. There is no spatial index and no map from id to node.

**`MapNode`.** Its persistent fields are `double x,y,z`, `int id`, `int flag`, the `ArrayList<MapNode>` lists `incoming` and `outgoing`, an `ArrayList<Connection> connections` and a private `MapMarker`: src/main/java/AutoDriveEditor/RoadNetwork/MapNode.java:114-123 (MapNode) @ e4fe43e. The other fields are editor state: selection, hidden, warnings, preview and the hidden-connection lists.

**Flags.** They are `int` constants: REGULAR=0 and SUBPRIO=1 come from the mod; SPECIAL_NODE=97, ROTATION_NODE=98 and CONTROL_NODE=99 are editor-only: src/main/java/AutoDriveEditor/RoadNetwork/MapNode.java:67-73 (MapNode) @ e4fe43e.

**Node warnings.** `NodeWarning` has the values NONE, OVERLAP, NEGATIVE_Y and OVERLAP_Y: src/main/java/AutoDriveEditor/RoadNetwork/MapNode.java:75-87 (NodeWarning) @ e4fe43e.

**Node factories.** `createNewNetworkNode` gives a new node `id = size()+1`, appends it and runs the overlap check: src/main/java/AutoDriveEditor/RoadNetwork/MapNode.java:189-195 (createNewNetworkNode) @ e4fe43e. Control, rotation and special nodes are made with negative ids and are never added to the list: src/main/java/AutoDriveEditor/RoadNetwork/MapNode.java:197-213 (createControlNode) @ e4fe43e.

**Connections.**
- The primary data is the per-node adjacency lists. Every edge appears in the source node's `outgoing` and in the target node's `incoming`.
- Load fills the two lists independently, without checking that they agree: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:360-384 (loadGameXMLFile) @ e4fe43e.
- The connection type is derived from membership in those lists, in this order: CROSSED_REGULAR, CROSSED_REVERSE, DUAL, REVERSE, SUBPRIO, REGULAR, and UNKNOWN otherwise: src/main/java/AutoDriveEditor/RoadNetwork/RoadMap.java:60-101 (getConnection) @ e4fe43e.
- `ConnectionType` has ten values: src/main/java/AutoDriveEditor/RoadNetwork/Connection.java:15-25 (ConnectionType) @ e4fe43e. Two separate colour mappings exist: src/main/java/AutoDriveEditor/RoadNetwork/Connection.java:35-67 (getColor) @ e4fe43e and src/main/java/AutoDriveEditor/RoadNetwork/Connection.java:95-127 (getColor) @ e4fe43e.
- `MapNode.connections` is filled only at load. The analyzer found no other add or remove of it, so it can go stale after edits. Rendering and selection use the adjacency lists instead.

**Keeping both lists consistent.** Each edit site does this by hand. The sites are src/main/java/AutoDriveEditor/RoadNetwork/MapNode.java:242-313 (removeConnection) @ e4fe43e, src/main/java/AutoDriveEditor/GUI/MapPanel.java:781-840 (createConnectionBetween) @ e4fe43e, src/main/java/AutoDriveEditor/RoadNetwork/RoadMap.java:45-58 (removeMapNode) @ e4fe43e and src/main/java/AutoDriveEditor/Managers/ScanManager.java:182-200 (mergeOverlappingNodes) @ e4fe43e.

**Markers and groups.**
- A marker is a private nested class on its node, with name, group (a string), id and a list of parked vehicles: src/main/java/AutoDriveEditor/RoadNetwork/MapNode.java:580-596 (MapMarker) @ e4fe43e.
- Groups exist as objects only in the static `RoutesXML.markerGroup` list: src/main/java/AutoDriveEditor/XMLConfig/RoutesXML.java:54 (RoutesXML) @ e4fe43e.
- `MarkerGroup` holds an index and a name. Its `compareTo` returns 0 or 1: src/main/java/AutoDriveEditor/RoadNetwork/MarkerGroup.java:8-29 (MarkerGroup) @ e4fe43e.

**Ids and list positions.**
- Ids are dense and 1-based, with `id == index+1`.
- Deleting a node decrements every later id: src/main/java/AutoDriveEditor/RoadNetwork/RoadMap.java:45-54 (removeMapNode) @ e4fe43e.
- Undo re-inserts a node at `id-1`, increments every later id, and throws `MismatchedIdException` on a mismatch: src/main/java/AutoDriveEditor/Classes/SnapShot.java:495-515 (insertMapNode) @ e4fe43e.
- `getIndexPositionOfNode` is `indexOf`, a linear scan: src/main/java/AutoDriveEditor/RoadNetwork/RoadMap.java:103 (getIndexPositionOfNode) @ e4fe43e.

**Lookup by position.** Two methods iterate the whole list and stop at the first node inside the node-size box, not the nearest one.
- `getNodeAtWorldPosition` uses a box in world units. When the rotate tool is active and the pointer is inside the rotation control node's box, it returns that control node first: src/main/java/AutoDriveEditor/GUI/MapPanel.java:659-679 (getNodeAtWorldPosition) @ e4fe43e.
- `getNodeAtScreenPosition` projects each node with `worldPosToScreenPos` and uses a scaled box: src/main/java/AutoDriveEditor/GUI/MapPanel.java:681-700 (getNodeAtScreenPosition) @ e4fe43e. The connection hit test also scans all nodes: src/main/java/AutoDriveEditor/GUI/Buttons/ConnectionSelectBaseButton.java:85-107 (getConnectionsAroundPoint) @ e4fe43e.

**KDTree.**
- `KDTree3D` is an unbalanced 3-D tree over `MapNode`: src/main/java/AutoDriveEditor/Classes/KDTree/KDTree3D.java:15 (KDTree3D) @ e4fe43e.
- Its only user rebuilds it from the full list on every overlap scan and queries `withinDistance(node, 0.1)`. The tree is never stored: src/main/java/AutoDriveEditor/Managers/ScanManager.java:58-85 (scanNetworkForOverlapNodes) @ e4fe43e.
- The overlap check that runs on each edit is a linear scan instead. A TODO calls it "horribly inefficient": src/main/java/AutoDriveEditor/Managers/ScanManager.java:93-137 (checkAreaForNodeOverlap) @ e4fe43e.

**Cost shape, as the code shows it.**
- Node by id: O(n), via LinkedList `get`.
- Node by position: O(n).
- `indexOf`: O(n).
- Edge-type tests: O(degree).
- Node delete: several O(n) passes.
- Overlap check per edit: O(n).

Not read for this question: the `CopyPasteManager` and `MultiSelectManager` bodies, and `ConnectionDrawThread`.

## 4. How does the code tell FS19, FS22 and FS25 config files apart, and what differs in their handling?

The code has two independent version notions.

**Config `<version>`.**
- The static `GameXML.configVersion` has no initializer: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:64 (GameXML) @ e4fe43e. It is set from the Semver major of the root `<version>`: 1 maps to FS19, 2 to FS22 and 3 to FS25: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:185-199 (loadGameXMLFile) @ e4fe43e.
- A legacy `markerID` config, or any other major version, leaves the previous value in place.
- `RoadMap.fsVersion` is declared but never used: src/main/java/AutoDriveEditor/RoadNetwork/RoadMap.java:23 (RoadMap) @ e4fe43e.

**`descVersion` from a mod zip's `modDesc`.**
- It is mapped by ranges: 40-53 is FS19, 60-80 is FS22, 90 and above is FS25, and everything else is NOT_SUPPORTED: src/main/java/AutoDriveEditor/Classes/Util_Classes/FSUtils.java:31-44 (getFSVersion) @ e4fe43e.
- It is read only in the zip import: src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/ImportFromZipMenu.java:125-127 (actionPerformed) @ e4fe43e. The dialog also builds a combo box of the game versions without NOT_SUPPORTED. It starts disabled and preselects the detected version, and a comment says an "override" checkbox enables it: src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/ImportFromZipMenu.java:341-359 (ImportFromZipMenu) @ e4fe43e. Where the chosen value is applied was not verified.
- It never writes `configVersion`.

**Branches on `configVersion`.**
- For FS22 and FS25, node flags 2 and 4 (nodes generated from map splines) are reset to REGULAR at load: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:346-352 (loadGameXMLFile) @ e4fe43e. The route-manager loader does the same: src/main/java/AutoDriveEditor/XMLConfig/RoutesXML.java:263-269 (loadRoutesXML) @ e4fe43e.
- In `vehicles.xml`, FS19 uses the `FS19_AutoDrive/AutoDriveVehicleData` element and FS22 and FS25 use `AutoDrive`. FS25 matches vehicles by `uniqueId`, the others by `id`. This applies on load and on save: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:764-784 (loadVehiclesXMLParking) @ e4fe43e, src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:838-865 (saveVehiclesXMLParking) @ e4fe43e.
- The wrong-image-size dialog adds an FS19 hint (`pda_map_H.dds`) or an FS22 hint (`overview.dds`). FS25 gets only the generic message: src/main/java/AutoDriveEditor/GUI/MapImage.java:429-435 (setMapPanelImage) @ e4fe43e.

**Branches in image conversion.**
- `createBufferImage` makes a 2048x2048 image. FS19 scales the whole image. FS22 and FS25 first crop the centre half and then scale. A NOT_SUPPORTED or null version gives a blank image: src/main/java/AutoDriveEditor/Managers/ImportManager.java:94-125 (createBufferImage) @ e4fe43e.
- The DDS menu entries hard-code FS19 or FS22: src/main/java/AutoDriveEditor/Managers/ImportManager.java:31-38 (importDDSForFS19) @ e4fe43e. There is no FS25 DDS menu item; FS25 images come in only through the zip import.

Not found: any reader of `RoadMap.fsVersion`, any caller of `FSUtils.isSupported`, and any rejection of NOT_SUPPORTED.

## 5. How is this code tested?

There are no automated tests at this commit.
- No `src/test` tree exists, and `@Test` and `junit` appear nowhere in the Java sources. This comes from the locators' searches at e4fe43e.
- `pom.xml` declares JUnit 4.13.2 with test scope but configures no surefire plugin: pom.xml:84-89 (dependencies) @ e4fe43e.
- `TestButton` is an experimental toolbar button, not a test. It logs screen-position distances and asserts nothing: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Experimental/TestButton.java:43-62 (TestButton) @ e4fe43e.
- The only sample data in the repository is `AutoDrive_config.xml` at the root. Its `<version>` is 3.0.1.4, which the version rule in section 4 maps to FS25: AutoDrive_config.xml:2 (AutoDrive) @ e4fe43e. No code, `pom.xml` or `readme.md` references it.

## Where the answers meet

- `loadGameXMLFile` is reached by questions 1, 3 and 4. It assigns ids `i+1` (src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:337-339 (loadGameXMLFile) @ e4fe43e), resets flags 2 and 4 for FS22 and FS25 (src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:346-352 (loadGameXMLFile) @ e4fe43e), and throws `EntryTotalException` on a list-count mismatch (src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:248-333 (loadGameXMLFile) @ e4fe43e).
- `RoadMap.removeMapNode` is reached by questions 2 and 3. It keeps ids dense by decrementing every later id, which is the property the save relies on when it writes `node.id` in list order: src/main/java/AutoDriveEditor/RoadNetwork/RoadMap.java:45-54 (removeMapNode) @ e4fe43e.

## Prior documents

None: no earlier research documents or ADRs exist in this repository.
