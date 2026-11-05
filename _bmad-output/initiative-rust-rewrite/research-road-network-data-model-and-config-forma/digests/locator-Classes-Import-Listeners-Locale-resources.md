Digest for area Classes, Import, Listeners, Locale, resources, src/test at e4fe43e. XML load/save code and the road-network model are not in this area (RoadNetwork/, Managers/, XMLConfig/).

Q1 (read XML)
- src/main/java/AutoDriveEditor/Classes/Util_Classes/XMLUtils.java:25 (XMLUtils.parseXmlFile) @ e4fe43e: DOM parse from byte[] (line 25) or path (line 47), using DocumentBuilderFactory.
- XMLUtils.java:243 (XMLUtils.XMLReader) @ e4fe43e: wrapper with getElementByTag, getChildNode, getNodeValue, getAttributeValue, getChildren, getElementByAttribute.
- XMLUtils.java:66-181 (XMLUtils) @ e4fe43e: typed readers getStringValue, getBooleanValue, getIntegerValue, getFloatValue, getEnumValue, getColorValue, each with a default-value overload.
- AutoDrive_config.xml:1 @ e4fe43e: sample config at repo root; starts with a BOM and `<?xml version="1.0" encoding="utf-8" standalone="no"?>`, root `<AutoDrive>`, children `<version>`, `<MapName>`, `<ADRouteVersion>`, `<ADRouteAuthor>` ...
- Not found in area: git grep "waypoints", "mapmarker" under Classes, Import, Listeners, Locale.

Q2 (write XML)
- XMLUtils.java:196-232 (XMLUtils) @ e4fe43e: setTextValue, setBooleanValue, setByteValue, setIntegerValue, setFloatValue, setEnumValue, setColorValue add child elements to a DOM Document.
- Config writer not in this area. XMLUtils.java has no Transformer, OutputKeys, setOutputProperty, UTF.
- Named but not read: src/main/java/AutoDriveEditor/GUI/Menus/RoutesMenu/SaveRoutesXML.java, src/main/java/AutoDriveEditor/XMLConfig/RoutesXML.java @ e4fe43e.

Q3 (in-memory model)
- src/main/java/AutoDriveEditor/Classes/KDTree/KDTree3D.java:15 (KDTree3D) @ e4fe43e: 3D KD-tree over MapNode. insert (27), remove (59), findMin (124), withinRadiusExcludingY (156), withinRadius (193), withinDistance (225), withinRectangle (258), withinWorldCoordinateRectangle (306), getAllNodesSortedById (346), getTreeSize (360), clear (371).
- KDTree3DNode.java:5 (KDTree3DNode) @ e4fe43e: MapNode plus left/right children.
- KDTree3D used at src/main/java/AutoDriveEditor/Managers/ScanManager.java:58 @ e4fe43e.
- Model types (git grep only): RoadNetwork/MapNode.java:24 (MapNode) — inner NodeWarning enum (75), MapNodeStore class (500), private MapMarker class (580), NODE_FLAG_REGULAR; RoadNetwork/Connection.java:15 (Connection.ConnectionType); RoadNetwork/RoadMap.java:18 (RoadMap) @ e4fe43e.
- Classes/ files referencing MapNode by count: SnapShot.java 90, BezierCurve.java 43, ArcSpline.java 35, LinearLine.java 34, CurveBase.java 32, KDTree3D.java 28. Curves/CurveBase.java:39 (CurveBase) @ e4fe43e: `Map<Integer, MapNode> controlPointsMap`, `LinkedList<MapNode> curvePreviewNodesList`.
- No group or flag type definitions in area.

Q4 (FS19/FS22/FS25)
- src/main/java/AutoDriveEditor/Classes/Util_Classes/FSUtils.java:5 (FSUtils.GameVersion) @ e4fe43e: enum NOT_SUPPORTED, FS19, FS22, FS25; isSupported(descVersion) returns descVersion >= 40.
- FSUtils.java:29 (FSUtils.getFSVersion) @ e4fe43e: descVersion 21-25 NOT_SUPPORTED (FS15); 31-37 NOT_SUPPORTED (FS17); 40-53 FS19; 60-80 FS22; >=90 FS25; else NOT_SUPPORTED.
- descVersion comes from the map mod's modDesc, not the AutoDrive config; called from GUI/Menus/ImportMenu/ImportFromZipMenu.java:126 @ e4fe43e.
- Version-dependent handling elsewhere: GUI/MapImage.java:429, 483; ImportFromZipMenu.java:249, 341, 419; Managers/ExportManager.java:39 (exportMapImageToDisk) takes GameVersion; ImportFS22DDSMenu.java and ImportManager.importDDSForFS22 @ e4fe43e.
- src/main/java/AutoDriveEditor/Import/DDSReader.java:1 (DDSReader) @ e4fe43e: DDS reader, not version-specific.
- Not found: git grep -i FS19/FS22/FS25 in Classes, Import, Listeners, Locale hits only FSUtils.

Q5 (testing)
- git ls-tree at e4fe43e: no src/test, no Java test classes.
- pom.xml:85 @ e4fe43e declares junit:junit:4.13.2 scope test; no surefire config.
- GUI/Buttons/Toolbar/Experimental/TestButton.java is a toolbar button, not a test.

Other
- Listeners/MouseListener.java:1 (MouseListener), Locale/LocaleManager.java: nothing on the questions.
- src/main/resources: locale/AutoDriveEditor_en_US.properties, log4j2.xml, themes/*.properties, META-INF/MANIFEST.MF, SVG icons. Nothing defines the config schema.
