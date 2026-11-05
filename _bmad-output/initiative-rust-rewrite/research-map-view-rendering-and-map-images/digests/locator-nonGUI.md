Digest for area top-level, Classes, Import, Listeners, Managers, Handlers, Locale, RoadNetwork, XMLConfig, resources @ e4fe43e. Paths under src/main/java/AutoDriveEditor/.

Q1 map image
- XMLConfig/GameXML.java:77 calls MapImage.loadMapImage(mapName), forceMapImageRedraw() :86, loadHeightMap, setIsEditorUsingImportedImage.
- XMLConfig/RoutesXML.java:99 loadMapImage(RoadMap.mapName), forceMapImageRedraw() :108, loadHeightMap.
- Managers/ImportManager.java:31, :35 importDDSForFS19/FS22 -> createBufferImageFromDDS :38.
- Managers/ImportManager.java:58 (createBufferImageFromDDS) plain Thread "DDS Decode" (:90); FileInputStream + DDSReader.read(fis, DDSReader.ARGB, 0, progressBar) :60; progress JFrame disposed via invokeLater :71; result scaled (createBufferImage ~:119) -> setPDAImage(scaledImage), setMapPanelImage(scaledImage, false), forceMapImageRedraw() :79, setMapImageLabel(IMPORTED) :82. Image held in GUI.MapImage (pdaImage) and MapPanel.
- Import/DDSReader.java:104, :108 (DDSReader.read) MIT from npedotnet; static image field (:43) BufferedImage TYPE_INT_RGB created/reused :128-134; decodeDXT1 :299, DXT3 :339, DXT5 :367, readA1R5G5B5 :394 (DXT3, DXT5, A1R5G5B5 logged untested); progress via invokeLater :327.
- Classes/Util_Classes/ZipUtils.java:29 openZipFile, :55 getFileFromZip, :94 getFileStreamFromZip, :133 getFileInputStreamFromZip (temp File), :177 findFileInZip.
- Classes/SimpleImageInfo.java:42, :49, :54 header info; not referenced elsewhere in area.
- Classes/Util_Classes/ImageUtils.java:16 getNewBufferImage, :25 loadImage, :57 ImageIO.read(url).
- Classes/Util_Classes/FileUtils.java:68 url.openStream() download path, not visibly tied to map image.
- resources: SVG icons only (globe_map_*), no map image.

Q2 drawing
- Managers/RenderManager.java:46 (RenderManager.drawAll) sorts static drawList by priority, calls drawToScreen(g) on each Drawable; called only from GUI/MapPanel.java:312 (EDT paint). Drawable :66, constructor adds to drawList :71. Priorities :21-25 (NONE 0, LOW 1, MEDIUM 64, HIGH 128, MAX 256). addToRenderQueue(BaseButton) :32, drawQueue() :59 debug.
- Drawable subclasses: Classes/LinearLine.java:43 (drawToScreen ~:492), Managers/MultiSelectManager.java:26 (selection rectangle); Curves, Widgets, WidgetManager, CurveManager, DebugDisplayManager not opened.
- Classes/Curves/ArcSpline.java:550-607, Classes/Curves/CurveBase.java:327-339 draw curve/control nodes via g2d and worldPosToScreenPos.
- Classes/Widgets/Selectors/RadiusSelector.java:76, RotationSelector.java:119 AffineTransform.
- RenderThreads: AutoDriveEditor.java:6-7 imports GUI.RenderThreads.ConnectionDrawThread, NodeDrawThread; :119 ConnectionDrawThread.stop(), :124 NodeDrawThread.stop() in shutdown handler.
- Classes/NameableThread.java:5 ThreadFactory used by XMLConfig/AutoSave.java:31.
- AutoDriveEditor.java:228 invokeLater for GUI startup.

Q3 zoom/pan
- Conversions and static zoomLevel in GUI/MapPanel.
- Listeners/MouseListener.java:100 (mouseWheelMoved) clamps rotation -1..1, getMapPanel().setNewZoomLevel(num) when !bIsShiftPressed && pdaImage != null; forwards to widgetManager, buttonManager, curveManager.
- Listeners/MouseListener.java:78 (mouseDragged) forwards to managers then getMapPanel().mouseDragged(x,y) (panning); storePreviousMousePos :94.
- Listeners/MouseListener.java:~117 getCurrentMouseWorldPos -> screenPosToWorldPos.
- Callers: Classes/Widgets/WidgetBase.java:72,81,91,143; Classes/Util_Classes/RotationUtils.java:67,101,114,125 (zoomLevel scales radius); Classes/LinearLine.java:77,98,109,244; Classes/Curves/ArcSpline.java:199,237; Selectors MoveSelector.java:36,58,82, RotationSelector.java:41,57,71, RadiusSelector.java:54,114; Classes/Widgets/MoveWidget.java:148.
- Classes/SnapShot.java:411 panel centre to world; imports getYValueFromHeightMap.
- XMLConfig/EditorXML.java:819 zoomLevel = 1 in reset path.

Q4 repaint triggers (getMapPanel().repaint())
- Managers/CopyPasteManager.java:34,54,88,96,120,132 (6).
- Managers/MultiSelectManager.java:83,244,318,334 (4).
- Managers/ScanManager.java:157,257 (2).
- XMLConfig/EditorXML.java:768,782,812,827 (4).
- Other repaint counts (target unchecked): LinearLine 7, CurveBase 3, ArcSpline 2, MoveWidget 4, CurveWidget 2, ArcSplineWidget 2, RotationUtils 1, UI_Components 19 across 8 files.
- forceMapImageRedraw(): ImportManager.java:79, GameXML.java:86, RoutesXML.java:108.
- MouseListener handlers delegate to MapPanel.

Nothing found: RenderThread/ExecutorService/new Thread only ImportManager.java:58 and AutoSave; no HttpURLConnection/map download code (only FileUtils.java:68 openStream); map-image locate likely in GUI/MapImage.loadMapImage (unconfirmed).
