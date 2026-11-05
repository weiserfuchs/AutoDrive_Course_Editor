Digest for area src/main/java/AutoDriveEditor/GUI/ (151 files, all subdirs), commit e4fe43e. This area is UI only. It calls the XML and road-network code but does not contain the parsing, writing, data types or KDTree.

Q1 (read XML)
- No element or attribute parsing of the AutoDrive config lives in this area. The entry points only call `GameXML.loadGameConfig`:
  - src/main/java/AutoDriveEditor/GUI/Menus/FileMenu/OpenConfigMenu.java:53 (actionPerformed) @ e4fe43e. Filter accepts file names containing ".xml". It calls `checkIfConfigIsStaleAndConfirmSave`, then `curveManager.cancelAllCurves`, then `loadGameConfig(File)`.
  - src/main/java/AutoDriveEditor/GUI/Menus/FileMenu/RecentFilesMenu.java:40 (actionPerformed) @ e4fe43e. Calls `loadGameConfig(new File(path))`.
- A different XML read is in the zip importer. It reads modDesc and map config with a stream reader:
  - src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/ImportFromZipMenu.java:125 (ImportFromZipMenu) @ e4fe43e. Reads `modDesc@descVersion` and `map@configFilename`.
  - src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/ImportFromZipMenu.java:138 (ImportFromZipMenu) @ e4fe43e. Reads `map@imageFilename`.

Q2 (write XML)
- No serialisation (element order, number format, encoding) in this area. Call sites of `GameXML.saveGameConfig(path, bool, bool)`:
  - src/main/java/AutoDriveEditor/GUI/Menus/FileMenu/SaveConfigMenu.java:23 (actionPerformed) @ e4fe43e, with `saveGameConfig(null,false,false)`.
  - src/main/java/AutoDriveEditor/GUI/Menus/FileMenu/SaveAsConfigMenu.java:54 (actionPerformed) @ e4fe43e, with the chosen path.
  - src/main/java/AutoDriveEditor/GUI/MapPanel.java:1055 (checkIfConfigIsStaleAndConfirmSave) @ e4fe43e.
  - src/main/java/AutoDriveEditor/GUI/Menus/RoutesMenu/OpenRoutesXML.java:35 @ e4fe43e.
  - src/main/java/AutoDriveEditor/GUI/RoutesGUI/RoutesGUI.java:68 @ e4fe43e.

Q3 (in-memory model)
- No type definitions in this area. It only consumes `RoadMap.networkNodesList` (a `LinkedList<MapNode>`) and `MapNode.outgoing`.
  - src/main/java/AutoDriveEditor/GUI/MapPanel.java:67 (imports `RoadMap.networkNodesList`), used at lines 664, 687 and 790 (createConnectionBetween) @ e4fe43e.
  - src/main/java/AutoDriveEditor/GUI/Buttons/ConnectionSelectBaseButton.java:91 (ConnectionSelectBaseButton) @ e4fe43e. Iterates `RoadMap.networkNodesList` and `mapNode.outgoing`.
  - src/main/java/AutoDriveEditor/GUI/Buttons/BaseButton.java:370 (getConnectionColour) @ e4fe43e. Uses `Connection.ConnectionType`.
  - src/main/java/AutoDriveEditor/GUI/Buttons/AddNodeBaseButton.java:127 (createNode) @ e4fe43e. Calls `createNewNetworkNode(x,y,z,flag,false)`.
  - src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Experimental/TestButton.java:49 @ e4fe43e. Constructs `new MapNode(0,0,0,0,MapNode.NODE_FLAG_REGULAR,false)`.
- Marker UI: src/main/java/AutoDriveEditor/GUI/Buttons/MarkerBaseButton.java and src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Markers/ (3 files, for example AddMarkerButton.java:52 uses `selectedNode.hasMapMarker()`).
- Direct references to a `KDTree` class or a `getRoadMap` function in this area: none found.

Q4 (FS19/FS22/FS25)
- src/main/java/AutoDriveEditor/GUI/MapImage.java:429 and :483 (MapImage) @ e4fe43e. Branches on `FSUtils.GameVersion.FS19` and `FS22` (the lines found show no FS25 branch) to choose the "incorrect map size" message.
- src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/ImportFromZipMenu.java:126 (ImportFromZipMenu) @ e4fe43e. `FSUtils.getFSVersion(descVersion)` derives the version from `modDesc@descVersion`.
- src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/ImportFromZipMenu.java:249 and :252 (getMapImagePreview) @ e4fe43e. FS19 is handled separately; the comment at :252 says "For FS22 and FS25, crop the image to remove the border".
- src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/ImportFromZipMenu.java:341 and :419 @ e4fe43e. A version override uses `GameVersion.values()`.
- Version-specific DDS imports: src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/ImportFS19DDSMenu.java:51 (`importDDSForFS19`, "pda_map_H.dds") and ImportFS22DDSMenu.java:49 (`importDDSForFS22`, "overview.dds") @ e4fe43e. There is no FS25 menu item in this area.
- The code that detects the version from a config XML is not in this area.

Q5 (testing)
- No @Test or junit hits in src/main/java/AutoDriveEditor/GUI @ e4fe43e. One test-like button class exists: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Experimental/TestButton.java (TestButton), an experimental toolbar button and not an automated test.

Searched and found nothing, all under src/main/java/AutoDriveEditor/GUI @ e4fe43e: `DOMSource`, `StreamResult`, `getElementsByTagName`, `setAttribute`, `KDTree`, `@Test`, `junit`.
