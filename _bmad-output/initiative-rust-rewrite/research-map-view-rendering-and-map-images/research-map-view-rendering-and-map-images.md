---
title: 'Codebase research: map view rendering and map images'
type: codebase
ask: |-
  Complete feature inventory of the Java/Swing AutoDrive Course Editor as a baseline; this run covers map view rendering and map images.
repository: '.'
commit: 'e4fe43e93f225e8c6d7635d62fb8cfdfd845a971'
branch: 'master'
status: complete
validation: 'normal'
claims: {verified: 51, disputed: 1, corrected_in_text: 1}
created: '2026-11-05'
updated: '2026-11-05'
---

# Codebase research: map view rendering and map images

## Questions

1. (how) How is a map image located, loaded and decoded (local file, DDS, zip, download) and held for display?
2. (how) How is the map view drawn (background, nodes, connections, markers, overlays), and on which threads (RenderThreads)?
3. (how) How do zoom, pan and the screen-to-world coordinate conversion work?
4. (what touches) What triggers a repaint of the map view?

## 1. How is a map image located, loaded and decoded (local file, DDS, zip, download) and held for display?

**Held state.** Images live in static fields of `MapImage`: `pdaImage` (the coordinate reference), `mapPanelImage` (what is drawn), and the heightmap fields `heightMapImage`, `heightMapImage16bit` and `heightMapPixelData`, plus `heightMapScale`: src/main/java/AutoDriveEditor/GUI/MapImage.java:44-55 (MapImage) @ e4fe43e. Every in-memory image is created with `createCompatibleImage` for the screen device, so the pixel format follows the display: src/main/java/AutoDriveEditor/Classes/Util_Classes/ImageUtils.java:16-23 (getNewBufferImage) @ e4fe43e. A TODO says `mapPanelImage` is meant to be eliminated.

**Locating a PNG at config load.** `loadGameConfig` calls `loadMapImage` and then `loadHeightMap` on the calling thread: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:77-78 (loadGameConfig) @ e4fe43e. `findImageLocationFor` probes these paths in order: src/main/java/AutoDriveEditor/GUI/MapImage.java:229-291 (findImageLocationFor) @ e4fe43e.
1. `<cur>/mapImages/<name>/<name><ext>.png`
2. The legacy path `<cur>/mapImages/<name><ext>.png`
3. `<cur>/src/main/resources/mapImages/<name><ext>.png`
4. `<name><ext>.png` next to the JAR

A file found in one of the legacy locations is moved into the per-map folder with `Files.move`, using paths relative to the working directory: src/main/java/AutoDriveEditor/GUI/MapImage.java:293-321 (findImageLocationFor) @ e4fe43e.

**Download.** When no file is found and `bUseOnlineMapImages` is set, the image is downloaded from `raw.githubusercontent.com/KillBait/AutoDrive_MapImages/master/mapImages/<name>/<name><ext>.png` into the per-map folder. The download is synchronous on the caller's thread, with no timeout and no size check: src/main/java/AutoDriveEditor/GUI/MapImage.java:324-368 (findImageLocationFor) @ e4fe43e, src/main/java/AutoDriveEditor/Classes/Util_Classes/FileUtils.java:65-100 (copyURLToFile) @ e4fe43e.

**Decoding a PNG.**
- `loadMapImage` reads the size, allocates an OPAQUE `pdaImage` and draws the `ImageIO.read` result into it. It then calls `setMapPanelImage(pdaImage,false)`. An `IOException` is ignored silently: src/main/java/AutoDriveEditor/GUI/MapImage.java:67-82 (loadMapImage) @ e4fe43e.
- `setMapPanelImage` requires 2048x2048. At that size it copies the image into a new `mapPanelImage`.
- At any other size without scaling, it shows an error dialog and returns false. `pdaImage` then keeps the wrong size.
- With scaling, it draws the scaled image into the existing `mapPanelImage` and does not touch `pdaImage`: src/main/java/AutoDriveEditor/GUI/MapImage.java:417-452 (setMapPanelImage) @ e4fe43e.
- The fallback is the classpath resource `/mapImages/Blank.png`, scaled to 2048x2048: src/main/java/AutoDriveEditor/GUI/MapImage.java:390-406 (useDefaultMapImage) @ e4fe43e.

**DDS import (FS19 and FS22 menu items).**
- The menu action on the event thread builds a progress frame. A thread named "DDS Decode" then runs `DDSReader.read`. The same thread scales the result and then updates `pdaImage`, `mapPanelImage`, the labels and the menus. Only the disposal of the progress frame goes through `invokeLater`: src/main/java/AutoDriveEditor/Managers/ImportManager.java:38-92 (createBufferImageFromDDS) @ e4fe43e.
- `createBufferImage` makes a 2048x2048 image. FS19 scales the whole image. FS22 and FS25 crop the centre half first: src/main/java/AutoDriveEditor/Managers/ImportManager.java:94-125 (createBufferImage) @ e4fe43e.
- `DDSReader.read` parses the 128-byte header. It decodes DXT1-5 or uncompressed formats with per-pixel `setRGB` into a static `TYPE_INT_RGB` image that is reused across calls. It returns that shared object, or null for an unknown type: src/main/java/AutoDriveEditor/Import/DDSReader.java:104-134 (read) @ e4fe43e.

**Zip import.**
- On the event thread, the importer opens the mod zip and reads `modDesc.xml`, `descVersion` and the map config's `imageFilename`, replacing `.png` with `.dds`. It also locates `dem.png`.
- On a thread named "DDS Decoder", it extracts the DDS to a temp file, decodes it and builds 256x256 previews of the map and the heightmap: src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/ImportFromZipMenu.java:188-244 (setupPreview) @ e4fe43e.
- The temp-file extraction rejects absolute and `..` paths: src/main/java/AutoDriveEditor/Classes/Util_Classes/ZipUtils.java:133-168 (getFileInputStreamFromZip) @ e4fe43e.
- On confirm, the importer writes `mapImages/<name>/<name>.png` (cropped and scaled by `createBufferImage`) and `<name>_HeightMap.png`. It does not display them. They are picked up at the next config load: src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/ImportFromZipMenu.java:443-455 (exportImages) @ e4fe43e, src/main/java/AutoDriveEditor/Managers/ExportManager.java:39-54 (exportMapImageToDisk) @ e4fe43e.

**Manual load.** A user-picked PNG goes through `setMapPanelImage(img,true)`. Only `mapPanelImage` changes, not `pdaImage`: src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/LoadMapImageMenu.java:52-69 (actionPerformed) @ e4fe43e.

**Heightmap.**
- The loader first looks for `terrain.heightmap.png` beside the config, then for `<name>_HeightMap.png` through the same lookup.
- A 16-bit grey source is kept as `heightMapImage16bit` next to a screen-format copy: src/main/java/AutoDriveEditor/GUI/MapImage.java:157-226 (loadHeightMap) @ e4fe43e.
- The heightmap supplies node elevations. `getYValueFromHeightMap` reads the 8-bit RGB copy, computes `((R<<8)+G)/256` and divides by `heightMapScale`. It returns -1 when there is no heightmap: src/main/java/AutoDriveEditor/GUI/MapPanel.java:707-729 (getYValueFromHeightMap) @ e4fe43e.
- `heightMapScale` is the rounded average of `(heightmapY - node.y)/node.y` over all nodes, at least 1: src/main/java/AutoDriveEditor/GUI/MapImage.java:374-388 (calculateHeightMapScaling) @ e4fe43e.

**Copies in memory.**
- After a PNG load there are two 2048x2048 copies of the map: `pdaImage` and `mapPanelImage`. At 4 bytes per pixel that is about 32 MiB; the analyzer estimated this.
- After a DDS import, the static `DDSReader.image` also stays referenced at the full DDS size.
- Drawing uses `mapPanelImage`, but every coordinate calculation uses `pdaImage`'s size. The two must match, and the manual-load path does not enforce that.

Not read: the `SimpleImageInfo` body, `findFileInZip`, `openPreviewWindow`, and the inner loops of the DXT decoders.

## 2. How is the map view drawn (background, nodes, connections, markers, overlays), and on which threads (RenderThreads)?

**Entry point.** `MapPanel` extends `JLayeredPane`. Its `paintComponent` on the Swing event thread is the only entry point: src/main/java/AutoDriveEditor/GUI/MapPanel.java:241-397 (paintComponent) @ e4fe43e. When a road map is loaded, it draws in this order:
1. The background.
2. The grid, if it is on.
3. On the event thread, it builds the visible-node list.
4. It triggers the two worker threads and waits for them. They draw connections, then nodes, icons and text.
5. The hover tooltip.
6. The `RenderManager` overlays.
7. The debug display.

The order is in src/main/java/AutoDriveEditor/GUI/MapPanel.java:261-312 (paintComponent) @ e4fe43e.

**Background.** Every paint draws the full 2048x2048 `mapPanelImage` through one `AffineTransform` (scale by zoom, translate by the clamped centre). No scaled copy is cached: src/main/java/AutoDriveEditor/GUI/MapPanel.java:571-592 (getResizedMap) @ e4fe43e. The grid is drawn only when zoom is above 2: src/main/java/AutoDriveEditor/GUI/MapPanel.java:216-238 (drawGrid) @ e4fe43e.

**Culling.**
- Every paint scans all nodes linearly on the event thread. The bounds are the panel corners in world coordinates, widened by `24 * mapScale`, and the result goes into a new `ArrayList` each time: src/main/java/AutoDriveEditor/GUI/MapPanel.java:275-288 (paintComponent) @ e4fe43e.
- Connections are culled by their start node only. A connection that comes into the view from an off-screen start node is not drawn: src/main/java/AutoDriveEditor/GUI/RenderThreads/ConnectionDrawThread.java:87-91 (run) @ e4fe43e.

**Worker threads.**
- The `MapPanel` constructor starts two plain threads, "ConnectionDraw Thread" and "NodeDraw Thread": src/main/java/AutoDriveEditor/GUI/MapPanel.java:185-193 (MapPanel) @ e4fe43e.
- On each paint the event thread stores the `Graphics2D` in the static `renderGraphics`, creates `CountDownLatch(2)` and `CountDownLatch(1)`, interrupts both workers, and blocks in `await()`: src/main/java/AutoDriveEditor/GUI/MapPanel.java:297-306 (paintComponent) @ e4fe43e.
- Each worker sits in `wait()`. Its `InterruptedException` catch block is the per-frame work: src/main/java/AutoDriveEditor/GUI/RenderThreads/NodeDrawThread.java:50-66 (run) @ e4fe43e, src/main/java/AutoDriveEditor/GUI/RenderThreads/ConnectionDrawThread.java:63-78 (run) @ e4fe43e.
- The connection worker builds its draw list, takes `drawLock`, draws, and then counts down both latches: src/main/java/AutoDriveEditor/GUI/RenderThreads/ConnectionDrawThread.java:138-149 (run) @ e4fe43e.
- The node worker builds its lists in parallel, waits on `drawOrderLatch`, takes `drawLock`, and draws nodes, then images, then text: src/main/java/AutoDriveEditor/GUI/RenderThreads/NodeDrawThread.java:156-177 (run) @ e4fe43e. Connections are therefore always drawn before nodes.
- No latch has a timeout. The connection worker counts down only on its normal path, so an exception there would leave the event thread blocked.
- The workers read node state without any lock.

**Per-frame work.**
- Nothing is cached across frames except the node sprites and the icons. The icons are rasterised once from SVG: src/main/java/AutoDriveEditor/GUI/MapPanel.java:203-207 (MapPanel) @ e4fe43e.
- Each frame allocates new display-list objects for every visible node and connection, and a new `Point` for every `worldPosToScreenPos` call: src/main/java/AutoDriveEditor/GUI/MapPanel.java:754-771 (worldPosToScreenPos) @ e4fe43e.
- `RenderManager` sorts its draw list on every paint: src/main/java/AutoDriveEditor/Managers/RenderManager.java:46-51 (drawAll) @ e4fe43e.
- Nodes and arrowheads are skipped when the scaled node size is below 2 px. Connections shorter than 1 px are skipped: src/main/java/AutoDriveEditor/GUI/RenderThreads/ConnectionDrawThread.java:177-182 (drawArrowList) @ e4fe43e.

**Connections.**
- Each connection is a line, shortened by half a node at both ends, with an arrowhead at the end node. The arrowhead is a filled polygon or two lines, depending on `bFilledArrows`.
- The colour comes from the connection type and from whether the start node is subprio.
- Hidden connections use a transparency setting: src/main/java/AutoDriveEditor/GUI/RenderThreads/ConnectionDrawThread.java:154-234 (drawArrowList) @ e4fe43e.

**Nodes.** Nodes are blitted sprite images, scaled to the zoomed node size: one for regular nodes, one for subprio nodes, plus a selection overlay. Hidden and preview state come from the node's flags: src/main/java/AutoDriveEditor/GUI/RenderThreads/NodeDrawThread.java:182-198 (drawNodes) @ e4fe43e. The hovered node is added last, so it draws on top.

**Markers and icons.**
- Marker (25x25), parking and warning (20x20) icons are drawn at their native pixel size, centred and not scaled by zoom: src/main/java/AutoDriveEditor/GUI/RenderThreads/NodeDrawThread.java:200-208 (drawImages) @ e4fe43e.
- Icons are chosen per node: an overlap warning, a negative-height warning (y == -1), a parking icon, and a marker icon above the node: src/main/java/AutoDriveEditor/GUI/RenderThreads/NodeDrawThread.java:110-137 (run) @ e4fe43e.
- Marker names and debug text are centred strings in the default font: src/main/java/AutoDriveEditor/GUI/RenderThreads/NodeDrawThread.java:210-228 (drawText) @ e4fe43e.

**Overlays.**
- `RenderManager` holds one static list of `Drawable` objects, sorted by priority from 0 to 256. Its `drawAll` runs on the event thread after the workers finish.
- The `Drawable` subclasses are the tool buttons, `LinearLine`, `MultiSelectManager`, `CurveManager`, `WidgetManager` and `SelectorBase`.
- Previews of lines and curves are drawn at 0.55 alpha: src/main/java/AutoDriveEditor/Classes/LinearLine.java:481-519 (drawToScreen) @ e4fe43e.
- The selection rectangle comes from src/main/java/AutoDriveEditor/Managers/MultiSelectManager.java:459-480 (drawToScreen) @ e4fe43e.

**Shutdown.** On window close, each worker's `stop()` sets a volatile flag and interrupts the thread: src/main/java/AutoDriveEditor/AutoDriveEditor.java:118-126 (AutoDriveEditor) @ e4fe43e.

Not found: frame skipping, dirty-region painting, caching of the node or connection lists across frames, or a thread pool.

## 3. How do zoom, pan and the screen-to-world coordinate conversion work?

**State.** All view state is static on `MapPanel`: `relativeCentreX` and `relativeCentreY` (the view centre as a 0..1 fraction of the map image, starting at 0.5), `zoomLevel` and the integer `mapScale`: src/main/java/AutoDriveEditor/GUI/MapPanel.java:116-119 (MapPanel) @ e4fe43e. The map image is always 2048x2048, so the map covers `2048*mapScale` world units, from `-1024*mapScale` to `+1024*mapScale`.

**`mapScale`.**
- It is a per-map integer. It comes from the Display > Map Scale menu (items labelled `i*2 km`) or from the per-map settings stored in the editor config.
- An unknown map defaults to scale 1, node size 2, a 2x2 grid and a maximum zoom of 30: src/main/java/AutoDriveEditor/XMLConfig/EditorXML.java:725-751 (checkStoredMapInfoFor) @ e4fe43e, src/main/java/AutoDriveEditor/XMLConfig/EditorXML.java:757-770 (setNewMapScale) @ e4fe43e.

**Pan.**
- `moveMapBy(dx,dy)` subtracts `dx/(zoomLevel*imgW)` from `relativeCentreX`, does the same for Y, and repaints: src/main/java/AutoDriveEditor/GUI/MapPanel.java:594-602 (moveMapBy) @ e4fe43e.
- Dragging pans with the left or the middle button, depending on `bMiddleMouseMove`. When the active tool allows it, the view also scrolls by 10 px per drag event at the panel edge: src/main/java/AutoDriveEditor/GUI/MapPanel.java:889-913 (mouseDragged) @ e4fe43e.
- At draw time the centre is clamped, so the view never leaves the image: src/main/java/AutoDriveEditor/GUI/MapPanel.java:571-592 (getResizedMap) @ e4fe43e.

**Zoom.**
- The mouse wheel, without Shift, calls `setNewZoomLevel` with the rotation clamped to -1..1: src/main/java/AutoDriveEditor/Listeners/MouseListener.java:99-106 (mouseWheelMoved) @ e4fe43e.
- The zoom factor is `1.6` per step. The result is clamped between the fit-to-panel minimum, `max(panelW/imgW, panelH/imgH)`, and `maxZoomLevel`: src/main/java/AutoDriveEditor/GUI/MapPanel.java:625-645 (getNewZoomLevel) @ e4fe43e.
- Zoom keeps `relativeCentre`, so it zooms about the view centre, not the cursor.
- With `bInterpolateZoom` (default true), a Swing timer eases the zoom in steps of 0.5 and repaints on each tick: src/main/java/AutoDriveEditor/GUI/MapPanel.java:604-657 (setNewZoomLevel) @ e4fe43e.
- `maxZoomLevel` defaults to 30 and can be set from 1 to 250 in the settings.

**Screen to world.**
- `topLeft = relativeCentre*imgSize - (panelSize/zoomLevel)/2`.
- `world = ((topLeft + screen/zoomLevel)*mapScale) - (imgSize/2)*mapScale`, rounded to 3 decimals. The Y component of the returned point is the world Z axis: src/main/java/AutoDriveEditor/GUI/MapPanel.java:731-748 (screenPosToWorldPos) @ e4fe43e.

**World to screen.**
- This is the inverse, with a hard-coded `1024*mapScale` offset and truncation to int, so it is not an exact inverse: src/main/java/AutoDriveEditor/GUI/MapPanel.java:754-771 (worldPosToScreenPos) @ e4fe43e.
- The node size on screen is `nodeSize*zoomLevel*0.5`, made odd: src/main/java/AutoDriveEditor/GUI/MapPanel.java:921-930 (updateNodeScaling) @ e4fe43e.

**Y values for new nodes.**
- They come from `getYValueFromHeightMap(worldX, worldZ)`, which maps world to heightmap pixels (section 1). Without a heightmap it returns -1.
- Callers include adding a node: src/main/java/AutoDriveEditor/GUI/Buttons/AddNodeBaseButton.java:90-95 (mouseReleased) @ e4fe43e.
- Paste recomputes y for every copied node: src/main/java/AutoDriveEditor/Classes/SnapShot.java:410-430 (createNewCopyOfNodes) @ e4fe43e.

Not found: zoom about the cursor, or any second implementation of the coordinate conversion.

## 4. What triggers a repaint of the map view?

**Inside `MapPanel`.**
- Pan: src/main/java/AutoDriveEditor/GUI/MapPanel.java:600 (moveMapBy) @ e4fe43e.
- Each tick of the zoom animation: src/main/java/AutoDriveEditor/GUI/MapPanel.java:656 (interpolateZoom) @ e4fe43e.
- Mouse move with the heightmap debug info shown: src/main/java/AutoDriveEditor/GUI/MapPanel.java:877 (mouseMoved) @ e4fe43e.
- Mouse move when the hovered node changes: src/main/java/AutoDriveEditor/GUI/MapPanel.java:884 (mouseMoved) @ e4fe43e.
- Centring on a node: src/main/java/AutoDriveEditor/GUI/MapPanel.java:1034 (centreNodeInMapPanel) @ e4fe43e.
- `forceMapImageRedraw`, which also re-clamps the zoom: src/main/java/AutoDriveEditor/GUI/MapPanel.java:1046 (forceMapImageRedraw) @ e4fe43e. It is called on panel resize (src/main/java/AutoDriveEditor/GUI/MapPanel.java:177 (componentResized) @ e4fe43e), when the default image is used (src/main/java/AutoDriveEditor/GUI/MapImage.java:86 (loadMapImage) @ e4fe43e), after a config load (src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:86 (loadGameConfig) @ e4fe43e), after a route-manager load (src/main/java/AutoDriveEditor/XMLConfig/RoutesXML.java:108 (RoutesXML) @ e4fe43e), after a DDS import (src/main/java/AutoDriveEditor/Managers/ImportManager.java:79 (createBufferImageFromDDS) @ e4fe43e), and from the map-info button (src/main/java/AutoDriveEditor/GUI/Buttons/ActionBar/ConfigPanel/MapInfoButton.java:59 (MapInfoButton) @ e4fe43e).
- Heightmap load: src/main/java/AutoDriveEditor/GUI/MapImage.java:149 (manualLoadHeightMap) @ e4fe43e.

**Editing managers** call `getMapPanel().repaint()` after a change:
- Copy and paste, 6 calls, for example src/main/java/AutoDriveEditor/Managers/CopyPasteManager.java:34 (CopyPasteManager) @ e4fe43e.
- Multi-select, 4 calls, for example src/main/java/AutoDriveEditor/Managers/MultiSelectManager.java:83 (mouseClicked) @ e4fe43e.
- The scan manager, 2 calls, for example src/main/java/AutoDriveEditor/Managers/ScanManager.java:157 (ScanManager) @ e4fe43e.
- Editor settings, 4 calls, for example src/main/java/AutoDriveEditor/XMLConfig/EditorXML.java:768 (setNewMapScale) @ e4fe43e.

**UI controls** repaint after a setting changes, about 40 calls in all:
- Grid and hidden-node toggles, for example src/main/java/AutoDriveEditor/GUI/Buttons/ActionBar/ConfigPanel/GridDisplayButton.java:57 (GridDisplayButton) @ e4fe43e.
- The Bézier tool, 4 calls, for example src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Curves/BezierCurveButton.java:100 (BezierCurveButton) @ e4fe43e.
- The settings tabs, about 13 calls, for example src/main/java/AutoDriveEditor/GUI/Config/Tabs/ColoursTab.java:74 (ColoursTab) @ e4fe43e.
- The curve panel, 9 calls, for example src/main/java/AutoDriveEditor/GUI/Curves/CurvePanel.java:136 (CurvePanel) @ e4fe43e.
- The display and debug menu toggles, one call per `Show*` item, for example src/main/java/AutoDriveEditor/GUI/Menus/Display/ShowNodeID.java:35 (ShowNodeID) @ e4fe43e.

**Widget and curve classes** also call `repaint()`: `LinearLine` 7 times, `MoveWidget` 4, `CurveBase` 3, and `ArcSpline`, `CurveWidget` and `ArcSplineWidget` 2 each. The locator did not confirm which component each of these repaints.

**Overall.** A repo-wide grep finds 152 lines matching `getMapPanel().repaint` or `MapPanel.repaint`. Every repaint request ends in `paintComponent`; the worker threads run only inside that call.

Not checked: whether any `javax.swing.Timer` other than the zoom timer triggers repaints.

## Where the answers meet

- `worldPosToScreenPos` is reached by questions 2 and 3. Every node, connection and overlay drawn on each frame passes through it, and each call allocates a new `Point`: src/main/java/AutoDriveEditor/GUI/MapPanel.java:754-771 (worldPosToScreenPos) @ e4fe43e.
- `getResizedMap` is reached by questions 1, 2 and 3. It draws `mapPanelImage` but takes its offsets from `pdaImage`'s size, and it clamps the pan centre: src/main/java/AutoDriveEditor/GUI/MapPanel.java:571-592 (getResizedMap) @ e4fe43e.
- `forceMapImageRedraw` is reached by questions 1, 3 and 4. It re-clamps zoom to the fit-to-panel minimum and repaints: src/main/java/AutoDriveEditor/GUI/MapPanel.java:1037-1048 (forceMapImageRedraw) @ e4fe43e.

## Prior documents

None: no earlier research documents or ADRs exist in this repository.
