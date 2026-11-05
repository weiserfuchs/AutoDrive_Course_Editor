Digest for area top-level files in src/main/java/AutoDriveEditor/, RoadNetwork, XMLConfig, Managers, Handlers, repo top level @ e4fe43e. Paths under src/main/java/AutoDriveEditor/ unless noted.

Q1 Reading a config XML
- XMLConfig/GameXML.java:68 (loadGameConfig) @ e4fe43e entry point; DOM parse and root check at 172 (root must be `AutoDrive`).
- XMLConfig/GameXML.java:234-345 (loadGameConfig) @ e4fe43e reads `<waypoints>` children id, x, y, z, out, incoming, flags (320). Comma-separated lists split on `,` (older `;` split commented out). x,y,z via Double.parseDouble, flag via Integer.parseInt. Nodes built with createMapNode(id,x,y,z,flag,false) (MapNode.java:171) into a list; out/incoming resolved by 1-based index at 360-380 `mapNode.outgoing.add(nodes.get(n-1))`. `-1` = no connection. Missing `<flags>` -> hasFlagTag false, save creates tag (497).
- XMLConfig/GameXML.java:418-460 (loadGameConfig) @ e4fe43e reads `<mapmarker>` children id, name, group per marker; marker id via Float.parseFloat cast to int (comment "AD 6.0.0.4 config fix"); attached via nodes.get(id-1).createMapMarker(name, group, markerId, parkedVehicles) (MapNode.java:221).
- XMLConfig/GameXML.java:469 (loadGameConfig) @ e4fe43e reads `<MapName>`.
- XMLConfig/GameXML.java:739 (loadVehiclesXMLParking) @ e4fe43e reads vehicles.xml park destinations via XPath -> HashMap<Integer,List<String>> markerId->vehicle ids.
- XMLConfig/RoutesXML.java:301-335 (loadRoutesXML) @ e4fe43e reads separate routes format; markerGroup -> MarkerGroup(id,name); marker attributes `g` and name.
- RoadNetwork/MapNode.java:140 (MapNode constructor), RoadNetwork/RoadMap.java:22 (networkNodesList) @ e4fe43e.
- XMLUtils used at GameXML.java:176 getStringValue(root,"version").

Q2 Writing
- XMLConfig/GameXML.java:110 (saveGameConfig), :485 (saveGameXMLFile) @ e4fe43e. Existing file re-parsed and edited in place.
- Element order kept from existing file; waypoints children rewritten by name (508-598: id, x, y, z, incoming, out, flags). Missing flags appended (497).
- Number formatting String.valueOf(double) for x,y,z (e.g. 525), String.valueOf(int) for ids/flags; no DecimalFormat; joined with `,`.
- Empty incoming/out written as `-1` (558-572).
- Markers (613-660): existing mm<N> removed, mapmarker rebuilt, each `<mm{N}>` with id, name, group (644-654).
- saveVehiclesXMLParking (:812) sets parkDestination attribute (866).
- Encoding (673-700): Transformer ENCODING UTF-8, INDENT yes, indent-amount 2; whitespace-only text nodes stripped via XPath `//text()[normalize-space(.)='']`; same at 880-882 for vehicles.xml.
- Target xmlConfigFile or newName; autosave/backup: saveGameConfig(newName,isAutoSave,isBackup), GameXML.java:133 (autoSaveGameConfigFile), XMLConfig/AutoSave.java.
- Routes writer: XMLConfig/RoutesXML.java:468, :480 @ e4fe43e.

Q3 In-memory representation
- RoadNetwork/MapNode.java:24 (MapNode) @ e4fe43e: id, x, y, z, flag int, incoming/outgoing MapNode lists, connections List<Connection>, hiddenConnectionsList, private MapMarker (class :580 name, group, mapMarkerID, parkedVehiclesList), warning and selection state. Flag constants 67-73: REGULAR=0, SUBPRIO=1, SPECIAL_NODE=97, ROTATION_NODE=98, CONTROL_NODE=99 (26-61 commented-out NodeFlag enum). NodeWarning (:75). MapNodeStore (:500). Factories createMapNode (:171), createNewNetworkNode (:189), createControlNode (:197), createRotationNode (:203), createSpecialNode (:209). addConnection :284, removeConnection :242.
- RoadNetwork/Connection.java:13 (Connection) @ e4fe43e: startNode, endNode, enum ConnectionType (REGULAR, REVERSE, DUAL, CROSSED, CROSSED_REGULAR, CROSSED_REVERSE); getColor :95. Types derived from in/out lists (RoadMap.java ~90-100, uses NODE_FLAG_SUBPRIO).
- RoadNetwork/RoadMap.java:18 (RoadMap) @ e4fe43e static holder: LinkedList<MapNode> networkNodesList (:22), mapName, FSUtils.GameVersion fsVersion (:23), uuid. getIndexPositionOfNode (:103) = indexOf.
- RoadNetwork/MarkerGroup.java:8 (MarkerGroup) @ e4fe43e index + name, Comparable; list RoutesXML.markerGroup LinkedList (RoutesXML.java:54).
- Managers/ScanManager.java:58 (checkNodeOverlap) @ e4fe43e builds new KDTree3D() from all nodes for overlap detection (:88); built on demand, not stored.
- Names only: Managers/MultiSelectManager.java, CopyPasteManager.java, ChangeManager.java.

Q4 FS versions
- Classes/Util_Classes/FSUtils.java:5 (GameVersion), getGameVersion(String) :17.
- Detection: XMLConfig/GameXML.java:172-196 (loadGameConfig) @ e4fe43e: root `version` parsed with Semver; major 1 FS19, 2 FS22, 3 FS25; stored in static configVersion (:64).
- Pre-1.1.0.6 configs recognised by `markerID` element on root (:176); opened view-only canEditConfig=false.
- GameXML.java:346: for FS22/FS25 flags 2 and 4 reset to NODE_FLAG_REGULAR at load.
- GameXML.java:764-784 (loadVehiclesXMLParking): FS19 XPath `//vehicle[./FS19_AutoDrive/AutoDriveVehicleData[@parkDestination]]`; FS22/FS25 `AutoDrive` element; vehicle id attr `uniqueId` FS25 else `id` (:783).
- GameXML.java:838-866 (saveVehiclesXMLParking) same split (:853-856).
- XMLConfig/RoutesXML.java:263 branches on FS22/FS25.
- Managers/ImportManager.java:31-38, :116-118 (createBufferImage): FS19 vs FS22/FS25 DDS import; importDDSForFS19, importDDSForFS22; no FS25 entry.
- Managers/ExportManager.java:39 (exportMapImageToDisk) takes GameVersion.
- Handlers/ holds only GlobalExceptionHandler.java.

Q5 Testing
- No test sources at commit; no src/test.
- pom.xml:85-86 @ e4fe43e junit dependency; compiler 11 (15-16); no surefire (only maven-compiler-plugin 3.8.1, maven-assembly-plugin 3.2.0).
- Fixture: AutoDrive_config.xml at repo root (FS25 sample, added in e4fe43e).

Searched, nothing found: DecimalFormat or Locale number formatting in XMLConfig; KDTree in RoadNetwork/ or XMLConfig/.

[Parent note: this locator also read AGENTS.md, which states project intent; that content was not copied here. AGENTS.md and CLAUDE.md are added to the excluded paths from here on.]
