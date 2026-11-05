DIGEST Q3: zoom, pan, screen-to-world @ e4fe43e. 8 files read. Paths under src/main/java/AutoDriveEditor/ (GUI/ for MapPanel/MapImage).

State
- GUI/MapPanel.java:116-119 (class fields): relativeCentreX/Y = 0.5 (0..1 fraction of map image), zoomLevel = 1F double, mapScale = 1 int.
- GUI/MapImage.java:107-109 (pdaImage load) scales any other size to 2048x2048; "1024" = half of 2048.

mapScale
- Per-map int; map spans 2048*mapScale world units, world -1024*mapScale..+1024*mapScale.
- MapPanel.setMapScale (MapPanel.java:1082) assigns; called only from EditorXML.setNewMapScale (XMLConfig/EditorXML.java:757-770) which sets "N*2 km" label, menu, MapInfoStore.
- Callers: GUI/Menus/Display/MapScaleMenu.java:51-56 actionPerformed, items i=1..9 (1..24 if bShowExtendedMapSizes) labelled i*2 km; EditorXML.checkStoredMapInfoFor (EditorXML.java:725-747) applies stored scale, node size, grid, max zoom; MapScale element EditorXML.java:421 default 1; null map name -> 1 (:745); unknown map default scale 1, node size 2f, GRID_2x2, max zoom 30 (:751).
- Other 1024*mapScale users: MoveWidget.java:153-156, EditLocationButton.java:55-69, RoadConnection.java:176-183.

Pan
- LOAD-BEARING MapPanel.moveMapBy (MapPanel.java:594-602): relativeCentreX -= diffX/(zoomLevel*pdaImage.getWidth()), same Y with height; no-op unless roadMap and pdaImage; widgetManager.updateAllWidgets(), repaint.
- Listeners/MouseListener.java:78-86 (mouseDragged) stores pos, forwards to managers, then getMapPanel().mouseDragged, stores previous.
- MapPanel.java:889-903 (mouseDragged): isDraggingMap -> moveMapBy(dx, dy); else usePanelEdgeScrolling() -> 10 px per event at edge moving outward.
- isDraggingMap set in mouseButtonPressed (MapPanel.java:908-913) for (BUTTON2 && bMiddleMouseMove) or (BUTTON1 && !bMiddleMouseMove); cleared mouseButtonReleased (915-917). bMiddleMouseMove default false (EditorXML.java:69), key MiddleMouseMove (:224), checkbox GUI/Config/Tabs/EditorTab.java:60-62.
- Clamp in getResizedMap (MapPanel.java:571-592) at draw time: halfW = (panelW*0.5)/(zoomLevel*imgW); centre clamped to [halfW, 1-halfW]; transform scale(zoom) then translate(-offsetX,-offsetY), offsetX = relCentreX*imgW - (panelW/zoomLevel)/2.
- centreNodeInMapPanel (MapPanel.java:1028-1035): worldPosToScreenPos(node) then moveMapBy(panel/2 - target).
- GUI/Menus/DebugMenu/MoveNodeToCentreMenu.java:39-47 sets zoomLevel = maxZoomLevel directly, updateNodeScaling, centres.

Zoom
- MouseListener.java:99-106 (mouseWheelMoved) clamps rotation -1..1; !Shift && pdaImage != null -> setNewZoomLevel(num); Shift left for managers.
- LOAD-BEARING getNewZoomLevel (MapPanel.java:625-645): factor 1.6^|direction|; direction<0 zoom in, >0 out; min = max(panelW/imgW, panelH/imgH); max maxZoomLevel; result min(max(new,min),max).
- setNewZoomLevel (MapPanel.java:604-623): bInterpolateZoom && zoomTimer == null -> Swing Timer delay (int)(2/newZoomLevel) ms; then zoomLevel = newZoomLevel unconditionally + updateNodeScaling. Zoom about view centre, not cursor.
- interpolateZoom (MapPanel.java:647-657): zoom += 0.5*(target - zoom); snap within 0.01, stop timer; per tick updateNodeScaling, widgets, repaint.
- maxZoomLevel default 30 (EditorXML.java:71), key MaxZoomLevel (:213), written :519; spinner EditorTab.java:101-107 1..250; per-map MapInfoStore (EditorXML.java:686-720); setNewMaxZoomLevel (EditorXML.java:817-829) resets zoomLevel = 1. bInterpolateZoom default true (:70), key InterpolateZoom (:248).
- forceMapImageRedraw (MapPanel.java:1037-1048): re-clamp zoom to fit, updateNodeScaling, widgets, repaint.
- updateNodeScaling (MapPanel.java:921-930): nodeSizeScaled = (int)(nodeSize*zoomLevel*0.5f), +1 if even; half/quarter derivatives.

Screen to world (LOAD-BEARING) MapPanel.java:731-748 (screenPosToWorldPos)
- topLeftX = relativeCentreX*imgW - (panelW/zoomLevel)/2 (Y same)
- worldX = ((topLeftX + screenX/zoomLevel)*mapScale) - (imgW/2)*mapScale; worldZ likewise (returned as Point2D.y)
- rounded 3 decimals HALF_UP; pdaImage null -> (0,0). Equivalent world = (imagepx - 1024)*mapScale.
- MouseListener.java:119-131 getCurrentMouseWorldPos/X/Y use last stored mouse position.

World to screen (LOAD-BEARING) MapPanel.java:754-771 (worldPosToScreenPos)
- off = 1024*mapScale hard-coded; worldX/Z += off; scaledX = (int)((worldX/mapScale)*zoomLevel); topLeftX = (int)(relCentreX*imgW*zoomLevel) - panelW/2; return (scaledX - topLeftX, ...). Overload MapPanel.java:750-752. int truncation, not exact inverse; 1024 vs imgW/2 equal only because image is 2048.

New node Y
- LOAD-BEARING MapPanel.getYValueFromHeightMap (MapPanel.java:707-729): -1 if heightMapImage null; scale = pda/hm; px = hmW/2 + floor((worldX/mapScale)/scaleX); clamp lower 0, upper `x > hmW` -> hmW-1 (x == hmW not clamped); value = ((red<<8)+green)/256 rounded 3 decimals; / heightMapScale.
- heightMapScale (GUI/MapImage.java:50 default 1) set at MapImage.java:147, 209 from calculateHeightMapScaling (MapImage.java:374-388): averages (heightmapY - node.y)/node.y over nodes, int, min 1.
- Call sites: GUI/Buttons/AddNodeBaseButton.java:90-95 (mouseReleased); Classes/SnapShot.java:410-430 (createNewCopyOfNodes) recomputes y even when inOriginalLocation; Classes/LinearLine.java:228, 245 (228 passes getCurrentMouseWorldY() for both args); GUI/Menus/.../FixNodesHeightMenu.java:31; MapImage.java:380.
- Debug lookup MapPanel.java:846-876 uses 16-bit grey data, shows hValue/256.

Not found: zoom-about-cursor, other zoom limits, other screenPosToWorldPos.
