DIGEST Q3: road network in-memory representation @ e4fe43e. Paths under src/main/java/AutoDriveEditor/.

1. Core containers
- One static `LinkedList<MapNode> networkNodesList` on RoadMap [LOAD-BEARING] RoadNetwork/RoadMap.java:22 (RoadMap.networkNodesList) @ e4fe43e. No spatial index, no id-to-node map. RoadMap.fsVersion (:23) static GameVersion; no assignment/read found (grep capped). uuid :24, mapName :20.
- RoadMap() (:27-33) resets networkNodesList. Loaders end with setRoadMapNodes (RoutesXML.java:338-339).

2. MapNode (RoadNetwork/MapNode.java:24)
- Persistent fields (:114-122): double x,y,z; int id; ArrayList<MapNode> incoming, outgoing; ArrayList<Connection> connections; int flag; private MapMarker mapMarker (:123).
- Editor state (:125-138): selection, hidden, warning flag/type, warningNodes, preview flags, ignoreDrawingConnectionsList, hiddenConnectionsList, previewHiddenConnectionsList, scheduledToBeDeleted, SelectorInterface nodeSelector.
- Coordinates rounded to 3 decimals in constructor (:144-146) and createMapNode (:171-187). createMapNode(..., isSelected) (:180-187) ignores isSelected, passes false.
- Flags (:67-73) int: REGULAR=0, SUBPRIO=1 (mod flags); SPECIAL_NODE=97, ROTATION_NODE=98, CONTROL_NODE=99 editor-only; predicates :404-407. Commented-out NodeFlag enum :26-63. FS22/FS25 load resets flags 2,4 to 0 (GameXML.java:346-352). Save writes node.flag verbatim (GameXML.java:598-607).
- Factories: createMapNode (:171, :180) assigns move selector; createNewNetworkNode (:189-195) id = networkNodesList.size()+1, adds to list, calls checkNodeOverlap; createControlNode (id -99, flag 99), createRotationNode (id -98, flag 98), createSpecialNode (id -98, flag 97) at :197-213 not added to list.
- NodeWarning (:75-87): NONE, OVERLAP, NEGATIVE_Y, OVERLAP_Y; scan only assigns OVERLAP.

3. Markers and groups
- MapMarker private static nested class (MapNode.java:580-596): name, group (String), Integer mapMarkerID, List<String> parkedVehiclesList (last two "EXPERIMENTAL"). Hangs off one node (createMapMarker :216-224, removeMapMarker :226). Markers found by iterating networkNodesList + hasMapMarker() (e.g. GameXML.java:640).
- isParkDestination (:414); hasVehicleParking/getParkedVehiclesList (:398, :427) dereference mapMarker without null check.
- RoadNetwork/MarkerGroup.java:8-29 {final int groupIndex; final String groupName}; compareTo returns 0 only if both equal else 1.
- Groups in static LinkedList<MarkerGroup> RoutesXML.markerGroup (XMLConfig/RoutesXML.java:54), not in RoadMap; filled from `<g i= n=>` (RoutesXML.java:300-317); markers `<m i= n= g=>` attached via nodes.get(markerNodeId-1).createMapMarker(name, group) (RoutesXML.java:326-336).
- New groups: new MarkerGroup(size()+1, name) in GUI/Buttons/Toolbar/Markers/AddMarkerButton.java:173, EditMarkerButton.java:90. Marker refers to group by name string.

4. Connections: adjacency lists per node, Connection objects derived [LOAD-BEARING]
- Primary data: per-node ArrayList<MapNode> outgoing and incoming; each edge stored in both endpoints (mirrored), following `<out>`/`<incoming>`.
- Load (XMLConfig/GameXML.java:360-384) parses out and incoming independently via nodes.get(parseInt(x)-1); -1 none; no consistency check between the two.
- Save (GameXML.java:560-596) writes n.id from networkNodesList and each node's lists.
- Connection (RoadNetwork/Connection.java:13): {startNode, startNodeID, endNode, endNodeID, connectionType, isHighlighted}; constructor snapshots ids (:77-93); equals start/end/type; hashCode sum of node hashes (:160-173).
- MapNode.connections populated only at load (GameXML.java:387-395) via RoadMap.getConnection unless UNKNOWN; no other .connections.add/remove outside GameXML.java:391 (commented code ConnectionDrawThread.java:125-127, MapNode.java:480). getConnectionList() :425. Editor builds temporary Connection lists on demand (GUI/Buttons/ConnectionSelectBaseButton.java:99, 144, 174; Classes/SnapShot.java:217, 241, 260, 453; curves/LinearLine previews). connections can go stale after edits; rendering/selection use incoming/outgoing.
- ConnectionType (Connection.java:15-25): NONE, UNKNOWN, MULTIPLE, REGULAR, SUBPRIO, DUAL, REVERSE, CROSSED, CROSSED_REGULAR, CROSSED_REVERSE. ConnectionType.getColor(startNode) (:35-67) and Connection.getColor() (:95-127) map to colours; DUAL subprio colour differs between the two (start SUBPRIO vs either end).
- Type derivation from list membership (RoadMap.java:60-101), order: CROSSED_REGULAR, CROSSED_REVERSE, DUAL, REVERSE, SUBPRIO (regular && start flag 1), REGULAR (regular && start flag 0), else UNKNOWN; start flag 97/98/99 -> UNKNOWN. isDual all four memberships; isRegular start.out∋t, t.in∋start, no reverse; isReverse start.out∋t only; isCrossedRegular start.out∋t, t.in∋start, t.out∋start, start.in∌t; isCrossedReverse start.out∋t, t.in∌start, t.out∋start, start.in∋t. Each uses ArrayList.contains O(deg).
- Consistency by hand per edit site: MapNode.addConnection/removeConnection (MapNode.java:242-313, used by SnapShot; add without duplicate check); GUI/MapPanel.java:781-840 (createConnectionBetween) toggles per type, checks networkNodesList.contains O(n); RoadMap.removeMapNode (RoadMap.java:45-58) removes node from all other lists; Managers/ScanManager.java:182-200 (mergeOverlappingNodes) edits lists directly.
- MapNode.MapNodeStore (MapNode.java:500-565) undo helper: node ref, id backup, de-duplicated copies of incoming/outgoing; restores id (getMapNode :514-517); copyList (:558-564) O(n²).
- hiddenConnectionsList (:136) only accepts targets in outgoing (:485-491).

5. Ids vs list positions [LOAD-BEARING]
- Invariant id == index+1, dense, 1-based.
- Load assigns id = i+1, ignores file ids (GameXML.java:337-339).
- createNewNetworkNode uses size()+1 (MapNode.java:190).
- RoadMap.removeMapNode decrements id of every later node (RoadMap.java:50) then removes; ~3 linear passes per deleted node.
- Classes/SnapShot.java:495-515 (insertMapNode) inserts at id-1, increments later ids (:513-515); checks neighbour id vs getIndexPositionOfNode, throws MismatchedIdException (:502-507); RoadMap.showMismatchedIDError (RoadMap.java:122).
- getIndexPositionOfNode = networkNodesList.indexOf (RoadMap.java:103) O(n); LinkedList get(id-1) O(n), e.g. GUI/Menus/.../MoveNodeToCentreMenu.java:42.
- Connection.startNodeID/endNodeID snapshots not updated on renumbering.

6. KDTree (Classes/KDTree/KDTree3D.java:15; KDTree3DNode.java:5)
- Unbalanced 3-D tree over MapNode, axes x,y,z by depth; node {mapNode, left, right}. Insert left if less else right (:27-53), no rebalancing.
- Queries: withinRadius(double x4) (:138, ignores y, first hit only), withinRadius(MapNode,double) (:149), withinDistance (:218), withinRectangle (:252), withinWorldCoordinateRectangle (:300, x/z), getAllNodesSortedById (:340, in-order by tree not id), size() (:356 O(n)), empty() (:373).
- remove (:55-122) uses findMin(node.right,0) walking left only regardless of axis; no caller found.
- Only use: Managers/ScanManager.java:58-85 (scanNetworkForOverlapNodes): rebuilt from networkNodesList each scan, withinDistance(mapNode, 0.1) per node (hard-coded 0.1 at :74). Never stored or synced.
- Per-edit overlap check checkNodeOverlap -> checkAreaForNodeOverlap (ScanManager.java:93-131, 139-159) linear scan per call; TODO ScanManager.java:134-137 "horribly inefficient", suggests KDTree. Called from addNodeToNetwork (RoadMap.java:36-43) and createNewNetworkNode.

7. Position lookup: linear scan [LOAD-BEARING]
- GUI/MapPanel.java:659-679 (getNodeAtWorldPosition) and :681- (getNodeAtScreenPosition) iterate whole networkNodesList, return first node within node-size box; screen variant converts each via worldPosToScreenPos (:688). O(n) per mouse query, first match not nearest. Rotation control node :671-675; curveManager.getControlNodeAt :698.
- GUI/Buttons/ConnectionSelectBaseButton.java:85-107 (getConnectionsAroundPoint) iterates all nodes, x/z window, pointToLineDistance (:109-129) per outgoing edge, hits < 0.5.
- Managers/MultiSelectManager.java:184, 215, 343, 367 iterate networkNodesList (not read).
- Rendering and saving (GameXML.java:560-607, :640) walk the full list.

8. Cost shape: node by id O(n); by position O(n); indexOf O(n); edge tests O(deg); delete O(n)+O(n)+O(sum deg); per-edit overlap O(n); full overlap scan tree build O(n log n) avg / O(n²) worst.

NOT FOUND / limits: no KDTree3D.remove caller, no second KDTree user, no HashMap<Integer,MapNode> in files read; RoadMap.fsVersion use unconfirmed; CopyPasteManager, MultiSelectManager body, ConnectionDrawThread not read. ~10 of 15 files.
