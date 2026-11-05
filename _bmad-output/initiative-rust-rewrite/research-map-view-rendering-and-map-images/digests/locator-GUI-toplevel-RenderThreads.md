Digest for GUI top-level files (MapPanel, MapImage; ActionBar, TextPanel, Toolbar not read) and GUI/RenderThreads (2 files) @ e4fe43e. Paths under src/main/java/AutoDriveEditor/GUI/.

Q1 map image
- MapImage.java:44-53 (MapImage): static pdaImage, mapPanelImage, heightMapImage, heightMapImage16bit, heightMapPixelData; pdaImage drawn by MapPanel.
- MapImage.java:57 (loadMapImage): findImageLocationFor(mapName,""), SimpleImageInfo size, ImageIO.read(new File) into OPAQUE buffer (getNewBufferImage), pdaImage, setMapPanelImage(pdaImage,false). Not found -> useDefaultMapImage, forceMapImageRedraw, dialog, NOT_FOUND label.
- MapImage.java:~225 (findImageLocationFor): order <currentLocation>/mapImages/<name>/<name>.png; legacy mapImages/<name>.png (moved into per-map folder with Files.move); src/main/resources/mapImages/<name>.png; <name>.png beside JAR; if bUseOnlineMapImages download https://raw.githubusercontent.com/KillBait/AutoDrive_MapImages/master/mapImages/<name>/<name>.png (~325-360). Same function with suffix for heightmap.
- MapImage.java:390 (useDefaultMapImage): classpath /mapImages/Blank.png scaled to 2048x2048.
- MapImage.java:417 (setMapPanelImage): requires 2048x2048, else error dialog (FS19/FS22 variants) and false unless scaleToCorrectSize; copies into mapPanelImage. TODO ~413 says mapPanelImage should be eliminated.
- MapImage.java:123-210 heightmap: ImageIO.read; TYPE_USHORT_GRAY kept as heightMapImage16bit, else drawn into heightMapImage.
- MapImage.java:455-505 nested class with setMapPanelImage/getMapPanelImage and private field copies; use undetermined.
- No DDS/zip in MapImage; PNG via ImageIO only.

Q2 drawing/threads
- MapPanel.java:74 (MapPanel) extends JLayeredPane.
- MapPanel.java:185-193 (constructor) starts Threads "ConnectionDraw Thread" and "NodeDraw Thread"; autosave started :198.
- MapPanel.java:241 (paintComponent) EDT: hints; getResizedMap if roadMap; drawGrid if bShowGrid; visible-node list via screenPosToWorldPos corners + 24*mapScale margin; NodeDrawThread/ConnectionDrawThread.setVisibleNodes; static renderGraphics=g2, threadCountLatch=CountDownLatch(2), drawOrderLatch=CountDownLatch(1), interrupt() both, await threadCountLatch; drawNodeInfo; RenderManager.drawAll(g2); profiling/FPS. roadMap null -> SVG logo (340-357).
- MapPanel.java:571 (getResizedMap) draws mapPanelImage with AffineTransform scale zoomLevel, translate from relativeCentre (576-590).
- MapPanel.java:216 (drawGrid) only zoomLevel > 2. MapPanel.java:399 (drawNodeInfo) text overlays.
- RenderThreads/NodeDrawThread.java:24 (NodeDrawThread) Runnable, wait() loop woken by InterruptedException; builds nodes/images/text lists (worldPosToScreenPos, flag, selection, visibility, preview; warning/parking/marker icons; marker names, debug ids; hover node); waits drawOrderLatch, takes drawLock, drawNodes (182), drawImages (200), drawText (210); threadCountLatch.countDown() (~176); volatile isStopped (36).
- RenderThreads/ConnectionDrawThread.java:20 (ConnectionDrawThread) same loop; ConnectionDrawList (37) with colour flags; drawLock; drawArrowList (154); threadCountLatch.countDown() and drawOrderLatch.countDown() (~148-149). Connections drawn before nodes.
- EDT blocks on threadCountLatch; drawing on workers into the paintComponent Graphics2D.

Q3 zoom/pan
- MapPanel.java:118 public static double zoomLevel; pan state relativeCentreX/Y fraction of pdaImage size (576-598, 735, 764).
- MapPanel.java:138 addMouseWheelListener.
- MapPanel.java:604 (setNewZoomLevel) sets zoomLevel, updateNodeScaling; bInterpolateZoom -> Swing Timer zoomTimer running interpolateZoom (647): 0.5 lerp, widgets, repaint().
- MapPanel.java:625 (getNewZoomLevel) factor 1.6^|direction|, negative zooms in; clamp between max(panelW/pdaW, panelH/pdaH) and maxZoomLevel.
- MapPanel.java:594 (moveMapBy) relativeCentre -= diff/(zoomLevel*pdaImage size), repaint() (600).
- MapPanel.java:889 (mouseDragged) isDraggingMap -> moveMapBy; else edge scroll ±10 px.
- MapPanel.java:~1050 (mouseButtonPressed) isDraggingMap on middle or left per bMiddleMouseMove.
- MapPanel.java:731 (screenPosToWorldPos) topLeft = relativeCentre*pdaSize - (panelSize/zoomLevel)/2; + screen/zoomLevel; mapScale and 1024 centre offset (735-745 not read).
- MapPanel.java:753 (worldPosToScreenPos) inverse.
- MapPanel.java:1037 (forceMapImageRedraw) re-clamps zoom, updateNodeScaling, widgetManager.updateAllWidgets, repaint.

Q4 repaint
- MapPanel.java:600 moveMapBy; :656 interpolateZoom; :877 mouseMoved heightmap debug; :884 mouseMoved hover change; :1034 centreNodeInMapPanel; :1046 forceMapImageRedraw (called from MapPanel.java:177 componentResized, MapImage.java:86 default image, others).
- MapImage.java:149 heightmap load repaint.
- repaint() in 40+ other files; getMapPanel().repaint|MapPanel.repaint 152 lines repo-wide.
- Worker threads run only inside paintComponent via interrupt().

Nothing found: DDS/zip/ZipFile in MapImage/MapPanel; no SwingWorker/invokeLater/LockSupport in MapPanel, MapImage, RenderThreads.
