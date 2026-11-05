DIGEST Q2: writing the in-memory road network back to config XML @ e4fe43e. Paths under src/main/java/AutoDriveEditor/ unless noted. ~9 files read.

Overview: saveGameXMLFile re-parses the existing file, overwrites only waypoint and map-marker data in that DOM, serialises the whole DOM.

1. Entry points
- XMLConfig/GameXML.java:110-131 (saveGameConfig) @ e4fe43e: returns silently if xmlConfigFile == null; calls saveGameXMLFile; plain save -> success dialog + setStale(false); exception -> log + save-fail dialog.
- [load-bearing] GameXML.java:485-491 (saveGameXMLFile) @ e4fe43e: docBuilder.parse(file) on existing config; waypoints = getElementsByTagName("waypoints").item(0); root = doc.getFirstChild().
- Callers (null,false,false): GUI/Menus/FileMenu/SaveConfigMenu.java:23; GUI/MapPanel.java:1055 (checkIfConfigIsStaleAndConfirmSave), route variant :1057; AutoDriveEditor.java:115; GUI/Menus/RoutesMenu/OpenRoutesXML.java:35; GUI/RoutesGUI/RoutesGUI.java:68. Save As GUI/Menus/FileMenu/SaveAsConfigMenu.java:54.
- Target: GameXML.java:693-705 @ e4fe43e: newName null -> xmlConfigFile; else new File(newName), and on non-autosave non-backup xmlConfigFile repointed + window title reset.

2. Element order
- [load-bearing] GameXML.java:494-499, 503-608 (saveGameXMLFile) @ e4fe43e: order is the on-disk order; loops waypoints children, setTextContent by name id, x, y, z, incoming, out, flags; never reorders/creates except `<flags>` appended if !hasFlagTag.
- Sample order AutoDrive_config.xml:49-58 @ e4fe43e: waypoints id,x,y,z,out,incoming,flags, then mapmarker.
- Separators GameXML.java:511-607: id,x,y,z,flags comma-joined; incoming/out `,` within node, `;` between nodes; none -> `-1`.
- Markers: GameXML.java:612-619 removes all children of the parent of each existing mm<N>, N from 1 to totalMarkers+99. GameXML.java:624-630 creates `<mapmarker>` under root if missing. GameXML.java:639-669 appends mm1..mmK in networkNodesList order for hasMapMarker() nodes, children id ("" + mapNode.id), name, group.
- [load-bearing] Marker ids written as plain ints (194); sample has 194.000000 (AutoDrive_config.xml:60), first save changes formatting.
- Other root content (version, MapName, *_userDefault ...) untouched, preserved via re-parsed DOM.
- vehicles.xml: only on real save (GameXML.java:719-721); saveVehiclesXMLParking (GameXML.java:812-886) re-parses vehicles.xml, skips if missing or map empty (:818-824), removes parkDestination attrs, sets parkDestination=String.valueOf(markerIndex), matches @uniqueId FS25 else @id; FS19 AutoDriveVehicleData path; same transformer settings.

3. Ids and ordering
- [load-bearing] GameXML.java:511-521 (saveGameXMLFile) @ e4fe43e: no renumbering on save; id written as node.id in list order; only check `if (node.id != a) LOG.info("Found")`.
- RoadNetwork/RoadMap.java:45-54 (removeMapNode) decrements ids above deleted node.
- mmN is a running counter in node-list order, not node id.
- Load: GameXML.java:248-268 (loadGameXMLFile) checks id list vs 1..N, mismatch only logged, lastGoodID overwritten with ids.length.

4. Number formatting
- [load-bearing] x,y,z doubles (RoadNetwork/MapNode.java:115-117) written via String.valueOf (GameXML.java:529, 540, 551) = Double.toString shortest round-trip: 100.000 -> 100.0; E-notation below 1e-3 or >= 1e7.
- Sample fixed at 3 decimals (AutoDrive_config.xml:51-53), 35,651 values at 3 decimals, one z value with 7 decimals.
- flags, ids int.
- Route export (XMLConfig/RoutesXML.java:343-516, saveRouteXML) @ e4fe43e: new doc root routeExport; waypoints attr c=<count>, children x,y,z,out,in,flags; `;`-separated, connections `,` within node, -1 empty; markers `m i= n= g=`; groups `g i= n=`.

5. Encoding
- [load-bearing] GameXML.java:673-677 @ e4fe43e: Transformer ENCODING=UTF-8, INDENT=yes, indent-amount=2, StreamResult(File); RoutesXML.java:487-491 indent 4.
- No standalone/OMIT_XML_DECLARATION/DOCTYPE set; sample declares standalone="no"; JDK output not verified.
- Sample starts with UTF-8 BOM (EF BB BF); writer sets no BOM.
- Sample LF only; no line separator set.
- GameXML.java:681-689: whitespace-only text nodes deleted via XPath `//text()[normalize-space(.)='']` then re-indented; sample uses 4-space indent, saved files 2-space.

6. Backup and autosave
- [load-bearing] XMLConfig/AutoSave.java:28-43 (startAutoSaveThread) scheduleAtFixedRate(autoSaveInterval, MINUTES), single thread "AutoSave"; fires if getRoadMap() != null; autoSaveGameConfigFile() or autoSaveRouteManagerXML() by config type. Defaults interval 10 min, 10 slots (XMLConfig/EditorXML.java:78-79); keys AutoSave_Interval, AutoSave_Slots, AutoSave_Last_Used_Slot (EditorXML.java:202-205, 513).
- GameXML.java:133-143; AutoSave.java:74-89: busy-waits 5 s while !canAutoSave(); suspendAutoSaving plain boolean used by CurveBase, LinearLine.
- GameXML.java:144-156: filename removeExtension(config path) + "_autosave_" + slot + ".xml", same folder; slot wraps after maxAutoSaveSlots; incremented after success.
- Autosave/backup do not retarget xmlConfigFile, no saveVehiclesXMLParking.
- Managers/ScanManager.java:272-279 (saveMergeBackupConfigFile): `<config>_mergeBackup.xml` before node merge via saveGameConfig(filename,false,true) or saveRouteManagerXML.
- RoutesXML.java:134-155, 157-185, 502: success dialog when `!isAutoSave || !isBackup`; backup Save As still retargets xmlConfigFile (check only !isAutoSave).

Not found: no atomic write (temp/rename), no pre-save .bak; no BOM/line-separator/standalone handling. XMLUtils.java:196-232 setters not read (appear to belong to EditorXML, unconfirmed). Not run.
