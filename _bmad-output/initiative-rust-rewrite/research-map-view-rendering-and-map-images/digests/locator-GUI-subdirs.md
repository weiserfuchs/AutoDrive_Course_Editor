Digest for area GUI subdirectories except RenderThreads (Buttons, Config, Curves, Menus, RoutesGUI) @ e4fe43e. Paths relative to src/main/java/AutoDriveEditor/GUI/.

Q1 map image
- Menus/ImportMenu/LoadMapImageMenu.java:57 (LoadMapImageMenu) @ e4fe43e: file chooser, ImageIO.read :57, setMapPanelImage(mapImage, true) :59 (in GUI/MapImage.java).
- Menus/ImportMenu/ImportFromZipMenu.java:209-271 (ImportFromZipMenu) @ e4fe43e: thread "DDS Decoder" calls DDSReader.read(fis, DDSReader.ARGB, 0, progressBar) :211; temp DDS file deleted (:214); held in field decodedDDSImage (:46); preview getMapImagePreview (:246); heightmap ImageIO.read :271; export exportMapImageToDisk (:452).
- Menus/ImportMenu/ImportFS19DDSMenu.java:18, ImportFS22DDSMenu.java:18 @ e4fe43e: menu entries only.
- Menus/ImportMenu/SaveMapImageMenu.java:24, ImportHeightMapMenu.java, ExportHeightMapMenu.java @ e4fe43e.
- Menus/EditorMenu.java:119-123 (EditorMenu) @ e4fe43e adds items; 244-246 enable/disable.
- Buttons/ActionBar/ConfigPanel/MapInfoButton.java:46-59 (MapInfoButton) @ e4fe43e: setMapPanelImage(heightMapImage, true), useDefaultMapImage(), forceMapImageRedraw(); labels mapImageLabel, heightmapImageLabel.
- Config/Tabs/EditorTab.java:39-40 (EditorTab) @ e4fe43e: checkbox bUseOnlineMapImages; download logic GUI/MapImage.java:325.

Q2 drawing
- Buttons/BaseButton.java:26 (BaseButton) extends Drawable, empty drawToScreen(Graphics) :88; drawArrowBetween (:251) sized nodeSize * zoomLevel (:299).
- Buttons/ConnectionSelectBaseButton.java:168 overlay connection previews via worldPosToScreenPos.
- Buttons/AddNodeBaseButton.java:146, Buttons/AlignBaseButton.java:66 overlays.
- Buttons/Toolbar/Curves/BezierCurveButton.java:115,142 curve preview overlay.
- Config/ConnectionPreviewPanel.java:114-146 separate preview panel zoomLevel = 1; Config/MoveWidgetPreviewPanel.java:231.
- Menus/DebugMenu/Show*.java (13 files, e.g. ShowFrameInfo.java:28, ShowZoomLevelInfo.java:28), Menus/Display/ShowNodeID.java:35, ShowMarkerNames.java:28, ShowMarkerIcons.java:28, ShowParkingIcons.java:29: overlay toggles followed by repaint.
- No render threads set here; only "DDS Decoder" thread (ImportFromZipMenu.java:209) and EDT Swing code.

Q3 zoom/pan
- Only callers: Buttons/Toolbar/Experimental/ArcSplineButton.java:48 screenPosToWorldPos; Menus/DebugMenu/MoveNodeToCentreMenu.java:40-43 sets zoomLevel = maxZoomLevel then worldPosToScreenPos; Config/Tabs/EditorTab.java:101-107 spinner maxZoomLevel (1-250).
- No pan/zoom implementation in these subdirectories.

Q4 repaint triggers
- getMapPanel().repaint() callers: Buttons/ActionBar/ConfigPanel (GridDisplayButton.java:57 4 hits; SelectHiddenButton.java:68), Buttons/Toolbar/Curves/BezierCurveButton.java:100 (5 hits); Config ColoursTab.java:74, ConnectionsTab.java:52, ExperimentalTab.java:23, MoveWidgetTab.java:85 (~13), MoveWidgetPreviewPanel.java:56; Curves/CurvePanel.java:136 (9); Menus/DebugMenu/Show*.java (13 files), Menus/Display/Show*.java (4 files).
- MapInfoButton.java:59 forceMapImageRedraw().
- ~40 hits, all UI controls/menu toggles after a setting change.

Nothing found: zoom/repaint in RoutesGUI; no download code in these subdirs.
