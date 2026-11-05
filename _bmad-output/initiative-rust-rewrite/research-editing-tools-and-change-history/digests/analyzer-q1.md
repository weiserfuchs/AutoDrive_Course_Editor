DIGEST Q1 toolbar tool input and network edits @ e4fe43e. SRC = src/main/java/AutoDriveEditor/. ~20 files read (budget exceeded).

1 Mouse path
- [LOAD-BEARING] SRC/GUI/MapPanel.java:135-138 (MapPanel) one MouseListener as mouse, motion, wheel listener.
- [LOAD-BEARING] SRC/Listeners/MouseListener.java:33-97 (mouseClicked..mouseMoved): store position, fan-out multiSelectManager, widgetManager, buttonManager, curveManager, MapPanel hook, store previous.
- MouseListener.java:67-75 enter/exit -> buttonManager only.
- MouseListener.java:99-106 (mouseWheelMoved): zoom if !Shift && image; then widget, button, curve managers (not multiSelect).
- [LOAD-BEARING] SRC/Managers/ButtonManager.java:510-517 forward only to currentButton if non-null.
- SRC/Managers/WidgetManager.java:120-154 forward to every enabled widget regardless of tool.
- SRC/Managers/ButtonManager.java:41, 226-240 (ButtonInterface) mouse hooks + onMultiSelectStart/Stop; SRC/GUI/Buttons/BaseButton.java:26, 39-123 empty defaults; lifecycle onButtonCreation/Select/Deselect, drawToScreen; flags useMultiSelection, usePanelEdgeScrolling, ignoreButtonDeselect, ignoreDeselect.
- SRC/GUI/MapPanel.java:889-917 panning separate (isDraggingMap BUTTON2 if bMiddleMouseMove else BUTTON1; edge scroll 10 px if usePanelEdgeScrolling).
- SRC/GUI/MapPanel.java:881-885 (mouseMoved) hoveredNode via getNodeAtScreenPosition.

2 Tool registration/switching
- SRC/GUI/Toolbar.java:41-50 node panel order MoveNode, NormalConnection, AddPrimaryNode, DualConnection, AddSecondaryNode, ReverseConnection, SwapNodePriority, FlipDirection, Rotation, DeleteNode; BezierCurveButton :57.
- SRC/Managers/ButtonManager.java:295-311 (addButton) duplicate ID rejected; ButtonNode, setNode, onButtonCreation.
- ButtonManager.java:282 static currentButton; getCurrentButton 411-417.
- [LOAD-BEARING] ButtonManager.java:360-381 (makeCurrent): onButtonDeselect old, set current, setSelected/onButtonSelect new, deselect others, notify curveManager.
- ButtonManager.java:387-396 (deSelectAll) current = null.
- [LOAD-BEARING] SRC/GUI/Buttons/BaseButton.java:158-178 (actionPerformed) selected -> makeCurrent, unselected -> deSelectAll, unless ignoreButtonDeselect.
- SRC/GUI/Buttons/Toolbar/Nodes/MoveNodeButton.java:58-68 shortcut action makeCurrent or deSelectAll.
- ShortcutGroup cycling: AddNodeBaseButton.java:44-65 (primary/secondary), LinerLineBaseButton.java:41-66 (normal/dual/reverse); repeated press -> getNextButton().

3 Keyboard
- SRC/Managers/ShortcutManager.java:169-185 (registerShortcut) WHEN_IN_FOCUSED_WINDOW InputMap + ActionMap on MapPanel; :188-223 registerMenuShortcut accelerators; :232-260 updateShortcut runtime rebind.
- [LOAD-BEARING] SRC/Managers/ShortcutManager.java:87-116 (ShortcutManager, defaultShortcutList); ShortcutID enum 30-60. User overrides loading not traced.
- Defaults: M move; Delete delete; L linear line (cycles); N new node (cycles); P swap priority; F flip; R rotate; B bezier; W add marker; E edit marker; Shift+D delete marker; Shift+X align horizontal; Shift+Y align vertical; Shift+F focus node; Ctrl+X cut; Ctrl+C copy; Ctrl+V paste; Shift+V paste original; H select-hidden; G grid; S grid snap; D subdivision snap; numpad + / - node size; ; continuous connection; F1 marker names; F2 marker icons; F3 parking icons; F4 node ID on hover.
- Elsewhere: SRC/GUI/MapPanel.java:144-157 Shift press/release sets bIsShiftPressed (inverts grid snap); UndoMenu.java:17 Ctrl+Z; RedoMenu.java:17 Shift+Z; CutMenu.java:17, PasteMenu.java:17, PasteOriginalLocationMenu.java:17 hard-coded accelerators; CopyMenu.java:22 registerMenuShortcut; Alt+key in File/Debug menus (SaveConfigMenu.java:17, ExitEditorMenu.java:15, ShowAllNodeIDMenu.java:16); mnemonics EditorMenu.java:54-141.

4 Tools
- Create: SRC/GUI/Buttons/AddNodeBaseButton.java:85-125 calcPosition grid snap bGridSnapEnabled XOR bIsShiftPressed. [LOAD-BEARING] :90-98 (mouseReleased) heightmap Y, createNode, checkAreaForNodeOverlap; AddPrimaryNodeButton.java:42 NODE_FLAG_REGULAR. [LOAD-BEARING] :127-138 (createNode) roadMap != null, suspend autosave, MapNode.createNewNetworkNode, AddNodeChanger, setStale(true), resume, repaint. SRC/RoadNetwork/MapNode.java:189-195 id size()+1, append, checkNodeOverlap. AddNodeChanger 161-175 undo removeOriginalNodes.
- Move: SRC/GUI/Buttons/Toolbar/Nodes/MoveNodeButton.java:40-48 widget on select, disable on deselect; useMultiSelection true (36-37). [LOAD-BEARING] SRC/Classes/Widgets/MoveWidget.java:52-145 press on axis starts drag; drag calcMoveOffset clamp ±1024*mapScale + grid snap -> moveNodesBy; release MoveWidgetChanger(multiSelectList, dx, dy) + setStale; release without drag manages selection/anchor. MoveWidget.java:245-262 moveNodesBy rounds 3 decimals, skips control/rotation nodes, updates curve previews.
- Delete: SRC/GUI/Buttons/Toolbar/Nodes/DeleteNodeButton.java:75-85 click; [LOAD-BEARING] :92-105 (removeNode) SnapShot, DeleteNodeChanger, removeOriginalNodes, clear hoveredNode, setStale. SRC/Classes/SnapShot.java:308-328 removeOriginalNodes connections then RoadMap.removeMapNode. SRC/RoadNetwork/RoadMap.java:45-58 scrub lists, decrement ids, remove. DeleteNodeButton.java:87-127 multi-select delete; :56-72 Delete key works regardless of current tool. Undo SnapShot.java:335-358 restoreOriginalNodes original ids + connections.
- Connections: NormalConnectionButton.java:47 REGULAR; priority toggle 52-74. [LOAD-BEARING] SRC/GUI/Buttons/LinerLineBaseButton.java:83-143 (mousePressed) first press start node; press start again cancels; press other node commit (continue if bContinuousConnections); empty space commit with end node if bCreateLinearLineEndNode. BUTTON3 cancels (145-149). Deselect resets unless shortcutChanged (70-73).
- [LOAD-BEARING] SRC/Classes/LinearLine.java:219-296 (commit): nodes per interpolated preview point via createNewNetworkNode, reusing existing at position; single end node if none; Y interpolated linearly; connectNodes -> MapPanel.createConnectionBetween (302-356); LinearLineChanger FULL/MANUAL/CONNECTION_ONLY; setStale; autosave suspended.
- [LOAD-BEARING] SRC/GUI/MapPanel.java:781-836 (createConnectionBetween): rejects start == target or node not in list; if not present adds per type (REGULAR/CROSSED_REGULAR/SUBPRIO add incoming; REVERSE/CROSSED_REVERSE remove incoming/outgoing entries; DUAL four entries); if present toggles off; comment 775-779 warns edge cases can corrupt config; also used by FlipDirection, BezierCurve, ArcSpline.
- SRC/RoadNetwork/MapNode.java:242-313 addConnection/removeConnection used by SnapShot.
- SwapNodePriority SRC/GUI/Buttons/Toolbar/Nodes/SwapNodePriorityButton.java:74-99 flag = 1 - flag, NodePriorityChanger; multiselect 86-110.
- FlipDirection SRC/GUI/Buttons/Toolbar/Nodes/FlipDirectionButton.java:84-152 four passes remove regular, remove reverse, add flipped reverse, add flipped regular (workaround documented 86-103); DUAL filtered (154-165).
- Rotation SRC/GUI/Buttons/Toolbar/Nodes/RotationButton.java:101-130 drag rotates control node, release RotateNodeChanger if angle > 0; BUTTON3 updateSelection.

Not covered: Shortcut.java, MultiSelectManager, ChangeManager, checkAreaForNodeOverlap, getNodeAtScreenPosition, curve/marker/alignment tools, LinearLine before 200, RotationButton 1-95.
