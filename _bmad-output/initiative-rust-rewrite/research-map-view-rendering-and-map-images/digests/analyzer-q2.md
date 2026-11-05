DIGEST Q2 map view drawing and render threads @ e4fe43e. Paths under src/main/java/AutoDriveEditor/. 12 files. Not run.

Frame (EDT)
- [load-bearing] GUI/MapPanel.java:74 (MapPanel) extends JLayeredPane; :241-397 (paintComponent) only entry point, EDT.
- GUI/MapPanel.java:250-259: Graphics2D copy, hints AA on, text AA off, nearest-neighbour, render-speed.
- GUI/MapPanel.java:335-359: roadMap null -> logo SVG rasterised every paint.
- [load-bearing] GUI/MapPanel.java:261-312, 394: order background getResizedMap; drawGrid if bShowGrid; visible list on EDT; trigger workers, await threadCountLatch; workers draw connections, nodes, images, text into shared renderGraphics; drawNodeInfo (hover tooltip); RenderManager.drawAll; debugDisplayManager.drawDebug.

Background
- [load-bearing] GUI/MapPanel.java:571-592 (getResizedMap): full 2048 mapPanelImage every paint via one AffineTransform scale(zoom) translate(-offset); clamps relativeCentre; no cached scaled copy.
- GUI/MapImage.java:417-446 mapPanelImage OPAQUE; TODO 413-415.
- GUI/MapPanel.java:216-238 (drawGrid) zoom > 2; worldPosToScreenPos per line (new Point); synchronized.

Culling
- [load-bearing] GUI/MapPanel.java:275-288 (paintComponent): linear scan of all networkNodesList each paint; corners to world + 24*mapScale; node.x/z in bounds; new ArrayList per paint; no spatial index.
- Handed to workers via static setters, no copy/sync: MapPanel.java:291-292; NodeDrawThread.java:38-42; ConnectionDrawThread.java:26, 53-55.
- GUI/RenderThreads/ConnectionDrawThread.java:87-91 (run): connections culled by start node only; off-screen start into view not drawn.

Threads and sync
- GUI/MapPanel.java:185-193: plain Threads "ConnectionDraw Thread", "NodeDraw Thread".
- [load-bearing] GUI/MapPanel.java:297-302 (paintComponent): trigger via interrupt() per paint; workers synchronized run() loop on wait(), InterruptedException catch is work body (NodeDrawThread.java:50-66; ConnectionDrawThread.java:63-78); isStopped checked first.
- Statics renderGraphics, threadCountLatch, drawOrderLatch, drawLock (MapPanel.java:85-87, 128).
- MapPanel.java:297-306: renderGraphics = g2; new CountDownLatch(2) and (1); interrupt both; await.
- ConnectionDrawThread: build ConnectionDrawList (85-133); drawLock + drawArrowList (138-145); countDown both latches (148-149).
- NodeDrawThread: build lists (80-152); drawOrderLatch.await (156); drawLock, nodes, images, text (157-166); countDown (177); lock() inside try so unlock in finally can run without lock.
- No timeouts. ConnectionDrawThread comment "finishes last in almost all cases" (22-24); NodeDrawThread 2-3x quicker, curves/lines placed there (26-27). ConnectionDrawThread countDown only on normal path (147-149), no try/finally; RuntimeException there -> EDT blocks forever. NodeDrawThread InterruptedException rethrown as RuntimeException (167-169).
- Workers read visibleNodes and node state without lock.

Per-frame allocations (nothing cached across frames)
- EDT: five ProfileUtil objects even when profiling off (MapPanel.java:244-248); new AffineTransform (587); new visibleNodes (275); new latches (298-299); Point2D corners; g.create(); drawNodeInfo infoList (399-400); Managers/RenderManager.java:46-48 Collections.sort(drawList) every paint.
- Workers: worldPosToScreenPos new Point each call (MapPanel.java:754-771); NodeDrawThread allocates NodeDisplayList/ImageDisplayList/TextDisplayStore per item (97, 102, 112-136), clears lists (61-63); ConnectionDrawThread two Points + ConnectionDrawList per connection (89-121), clears drawList (71), RoadMap.isRegular/isDual/isReverse/isCrossedReverse per connection, contains() on ignore/preview lists.
- drawArrowList Graphics copy and Polygon once per frame, reset() (157-158, 223); drawNodes Graphics copy + two Composites (184-186).
- Cached: node sprites static BufferedImages (MapPanel.java:93-104); icons rasterised once from SVG in constructor (203-207); mapPanelImage; nodeSizeScaled statics.
- LOD: nodes/arrowheads skipped when nodeSizeScaled < 2 (NodeDrawThread.java:92, 183; ConnectionDrawThread.java:192; MapPanel.java:79, 402); connections < 1 px skipped (ConnectionDrawThread.java:177-182).

Connections
- [load-bearing] GUI/RenderThreads/ConnectionDrawThread.java:154-234 (drawArrowList): line shortened by nodeSizeScaledHalf; arrowhead when nodeSizeScaled >= 2 and length >= nodeSizeScaled, length nodeSize*zoomLevel*0.7 minus diff/1.15 (192-229); filled polygon if bFilledArrows else two lines (218-228); alpha hiddenNodesTransparencyLevel for hidden (189); colours per type at compute (95-122); preview flags; dual drawn from one side (ignore list check, unverified); old commented version 125-131.

Nodes
- [load-bearing] GUI/RenderThreads/NodeDrawThread.java:90-98: NodeDisplayList(x,y,flag,selected,visible) per visible node; added when nodeSizeScaled >= 2 and not hovered-special (92-93); select/visible XOR preview (94-96).
- Hovered node added last (144-151).
- NodeDrawThread.java:182-198 (drawNodes): sprite blit subprioNodeImage for non-regular flag else regularNodeImage; hidden alpha; selected overlay; drawImage args at 190 not seen.

Markers and text
- [load-bearing] GUI/RenderThreads/NodeDrawThread.java:200-208 (drawImages): native pixel size centred, not scaled by zoom; marker 25x25, others 20x20 (MapPanel.java:204-207).
- NodeDrawThread.java:110-137: overlap warning (110-113); negative-height warning if y == -1 (114-117); parking if isParkDestination && bShowParkingIcons (120-122); marker at worldPosToScreenPos(x, z-1) offset -20 px if bShowMarkerIcons (127-138).
- NodeDrawThread.java:210-228 (drawText): default font, centred via getStringBounds, vertical rect.height/2 - 3; background off at node call sites.
- Marker name on hover or bShowMarkerNames, hovered appends " ( group )"; debug node ID, Y height (101-108, 128-134).
- GUI/MapPanel.java:399-568 (drawNodeInfo) hover tooltip on EDT, rebuilt every paint.

Overlays
- [load-bearing] Managers/RenderManager.java:46-51 (drawAll): static drawList (27) sorted by priority, drawToScreen on EDT after nodes/connections; priorities NONE 0, LOW 1, MEDIUM 64, HIGH 128, MAX 256 (21-25); setRenderPriority rejects <0 or >256 (84-89); Drawable constructor registers (70-73); addToRenderQueue(BaseButton) counter priority if absent (32-44).
- Drawable subclasses: BaseButton, LinearLine, MultiSelectManager, CurveManager, WidgetManager, SelectorBase.
- GUI/Buttons/BaseButton.java:251-299 (drawArrowBetween) Graphics copy + Polygon per call; hidden alpha commented (278-280).
- Classes/LinearLine.java:481-519 (drawToScreen) 0.55 alpha, fillArc preview nodes, arrows; Classes/Curves/CurveBase.java:320-345 preview, control points, "CP_n" labels; Managers/MultiSelectManager.java:459-480 selection bounds rect, drag rect/lasso, new BasicStroke per paint.

Stop
- AutoDriveEditor.java:118-126 stop() sets volatile isStopped, interrupt; NodeDrawThread.java:67-70, ConnectionDrawThread.java:74-77 return. Non-daemon (MapPanel 185-193 only checked). Stopped worker never counts down -> in-flight paint could block.

Not found: frame skipping, dirty regions, cross-frame list caching, thread pool, latch timeouts (files read only).
