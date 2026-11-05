DIGEST Q1: AutoDrive config XML -> memory (commit e4fe43e). Files read: GameXML.java (60-560, 725-800), MapNode.java (135-235), RoadMap.java (15-40, 112-135), XMLUtils.java (20-110), OpenConfigMenu.java, RecentFilesMenu.java, AutoDrive_config.xml. Not read: RoutesXML.java, MapMarker class, rest of GameXML.

CALL PATH
- src/main/java/AutoDriveEditor/GUI/Menus/FileMenu/OpenConfigMenu.java:53 (anonymous open action) @ e4fe43e: sets lastUsedLocation, calls loadGameConfig(lfc.getSelectedFile()).
- src/main/java/AutoDriveEditor/GUI/Menus/FileMenu/RecentFilesMenu.java:39-40 (recentMenuItemListener) @ e4fe43e: loadGameConfig(new File(path)) when configType == CONFIG_SAVEGAME, else loadRouteManagerXML.
- src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:68-108 (loadGameConfig) @ e4fe43e [load-bearing]: calls loadGameXMLFile(file) at :72, then post-load steps.
- GameXML.java:163-483 (loadGameXMLFile) @ e4fe43e [load-bearing]: whole parse. 164-167 plain DocumentBuilderFactory DOM parse, normalize. 234 getElementsByTagName("waypoints"), loops over all matches.

ELEMENTS PARSED
- No XML attributes read from the game config; all data is element text. Attributes only in vehicles.xml.
- Root and version (GameXML.java:172-200, loadGameXMLFile) @ e4fe43e [load-bearing]: root name must be "AutoDrive" else log + return null -> dialog_config_unknown. `<markerID>` present -> pre-v1.1.0.6, warning dialog, canEditConfig=false, loading continues. `<version>` via XMLUtils.getStringValue parsed with Semver: major 1 FS19, 2 FS22, 3 FS25, canEditConfig=true. Sample 3.0.1.4 -> FS25. Other major leaves configVersion unchanged.
- `<waypoints>` children split on "," or ";" (GameXML.java:248-333) @ e4fe43e [load-bearing]: `<id>` count defines N; values only sequence-checked; actual id always i+1 (:339). `<x>`,`<y>`,`<z>` comma-separated doubles, count must equal N. `<out>`,`<incoming>` semicolon-separated groups, one per node, each comma-separated 1-based ids, -1 none; group count must equal N. `<flags>` optional comma-separated ints; absent -> all "0", static hasFlagTag=false; present -> true. FS22/FS25: flag 2 or 4 reset to NODE_FLAG_REGULAR (:346-352).
- `<mapmarker>` (GameXML.java:418-459) @ e4fe43e [load-bearing]: parallel lists of `<id>`,`<name>`,`<group>` inside `<mapmarker>`, not the `<mmN>` wrappers. Marker id Float.parseFloat cast to int (:441-443), 1-based node id. markerId = markerIndex+1 (file order), key into parking map. nodes.get(id-1).createMapMarker(name, group, markerId, parkedVehicles) (:452-453).
- `<MapName>` (GameXML.java:469-476) @ e4fe43e -> RoadMap.mapName; `new RoadMap()` at :464 sets mapName=null first.
- vehicles.xml sibling (GameXML.java:739-799, loadVehiclesXMLParking) @ e4fe43e: same folder; missing -> empty map (:747-750). XPath `//vehicle[./AutoDrive[@parkDestination]]`, FS19 `//vehicle[./FS19_AutoDrive/AutoDriveVehicleData[@parkDestination]]`. Attributes parkDestination (Integer marker id), vehicle `uniqueId` FS25 else `id`. Map markerId -> list of vehicle ids.

OBJECT CONSTRUCTION
- GameXML.java:354 (loadGameXMLFile) @ e4fe43e: MapNode.createMapNode(id,x,y,z,flag,false); MapNode.java:180-187 (createMapNode) rounds coordinates to 3 decimals HALF_UP via limitDoubleToDecimalPlaces, calls constructor (MapNode.java:140-169, MapNode), nodeSelector=MOVE_SELECTOR. Constructor also rounds to 3 decimals via roundUpDoubleToDecimalPlaces.
- MapNode holds id, x, y, z, flag, incoming, outgoing, connections lists; rest editor state.
- Connections (GameXML.java:360-395) @ e4fe43e: out -> mapNode.outgoing, incoming -> mapNode.incoming; entry skipped if -1 or > lastGoodID (:366, :378). Connection(mapNode, outgoing, type) built per outgoing link, type from RoadMap.getConnection(a,b), skipped if UNKNOWN (:387-394).
- MapNode.java:221-224 (createMapMarker) @ e4fe43e: new MapMarker(name, group, id, parkedVehicles) stored in node.mapMarker; 2-arg overload :216.
- GameXML.java:464-465 @ e4fe43e: new RoadMap() then setRoadMapNodes. RoadMap constructor (RoadMap.java:27-33) sets static networkNodesList new LinkedList, mapName null, uuid random. setRoadMapNodes (RoadMap.java:118-120) overwrites static networkNodesList with LinkedList copy of parse ArrayList.

SAMPLE FILE VS CODE
- AutoDrive_config.xml (1175 lines) top-level: version, MapName, ADRouteVersion, ADRouteAuthor, currentDebugChannelMask, mod settings (*_userDefault, Pathfinder, fieldPointOffset, blinkValue, collisionHeigth, driverWages, lookAheadTurning, maxFieldPointDistance, autoConnectStart/End, useFolders, UMRange, enableTrafficDetection, maxTriggerDistance ...), waypoints (id,x,y,z,out,incoming,flags), mapmarker (mm1..mm223 each id,name,group).
- Loader reads only version, MapName, markerID, waypoints/{id,x,y,z,out,incoming,flags}, mapmarker/{id,name,group}. Settings, ADRouteVersion, ADRouteAuthor never read.

UNREAD ELEMENTS PRESERVED
- saveGameXMLFile (GameXML.java:485-) re-parses original file at xmlConfigFile (:489), overwrites waypoints children and mapmarker, writes back; unread elements retained. No `<flags>` -> appended on save (:496-499). No `<mapmarker>` -> created (:621-628).

ERROR HANDLING
- Wrong root -> null + dialog_config_unknown (GameXML.java:172-175, :101-103).
- Count mismatch -> EntryTotalException (:276-316), caught :478-481, dialog naming tag, return null.
- Other exceptions -> loadGameConfig catch (:104-107), log + dialog_config_load_failed: NumberFormatException; NPE for missing elements/empty text (.item(0).getChildNodes().item(0) unchecked); missing/malformed `<version>` (new Semver(null) unguarded); marker id out of range (IndexOutOfBounds); missing marker name/group; SAX/IO.
- id-sequence check inert (:256-268): mismatch sets lastGoodID and breaks, but :268 sets lastGoodID = ids.length unconditionally. TODO at :261 notes no dialog.
- Connection entries > lastGoodID skipped; 0 or negative other than -1 would throw at nodes.get(-1).
- Missing `<flags>` / `<MapName>` handled gracefully.

STATE AFTER LOAD (GameXML.java:73-100, loadGameConfig) @ e4fe43e
- configType=CONFIG_SAVEGAME; xmlConfigFile; configVersion, canEditConfig, hasFlagTag; RoadMap.networkNodesList, mapName, uuid (logged :80).
- getMapPanel().setRoadMap(roadMap); loadMapImage(mapName), loadHeightMap(mapName), checkStoredMapInfoFor(mapName).
- window title; changeManager.clear(); map image redraw; setIsEditorUsingImportedImage(false), saveImageEnabled(false), setStale(false); scanNetworkForOverlapNodes(); clearMultiSelection(); menu enables gameXMLSaveEnabled(true), routesXMLSaveEnabled(false), mapImageMenuEnabled(true), heightmapMenuEnabled(true), scanMenuEnabled(true), menu_OpenRoutesConfig; buttonManager.enableAllButtons(), onConfigChange(); addToRecentFiles(path, configType).
- OpenConfigMenu cancels curves and suspends/resumes autosave; RecentFilesMenu calls checkIfConfigIsStaleAndConfirmSave() and cancels curves.
- On failure no post-load state applied; new RoadMap() side effects happen only if failure after :464.

NOT FOUND / NOT CHECKED
- RoutesXML.loadRoutesXML not read (uses setRoadMapNodes, RoutesXML.java:339). MapMarker class, RoadMap.getConnection, canEditConfig enforcement not read.
- XMLUtils.parseXmlFile (:25, :47) and XMLReader (:243) not used by loadGameXMLFile.
