Digest for area Managers, Listeners, Handlers, RoadNetwork, top-level AutoDriveEditor/ and GUI/ files @ e4fe43e. Paths under src/main/java/AutoDriveEditor/.

Q1
- GUI/MapPanel.java:135 creates Listeners MouseListener, registers as mouse, motion, wheel (136-138); :142 key bindings InputMap (Shift 144-145); :843 mouseMoved, :889 mouseDragged; 893-894 buttonManager.getCurrentButton().
- Listeners/MouseListener.java:34 mouseClicked, :46 mousePressed, :57 mouseReleased, :78 mouseDragged, :89 mouseMoved: fan-out order multiSelectManager, widgetManager, buttonManager, curveManager; drag/move also MapPanel.
- Managers/ButtonManager.java:41 (ButtonInterface) mouse methods 226-230; :295 addButton; :366, :393 currentButton select/deselect; 510-517 forward to currentButton.buttonInterface.
- GUI/Toolbar.java:41-46 addButton MoveNodeButton, NormalConnectionButton, AddPrimaryNodeButton, DualConnectionButton, AddSecondaryNodeButton, ReverseConnectionButton.
- AutoDriveEditor.java:47-48, :93, :95 static changeManager, buttonManager.
- Managers/WidgetManager.java:120-146 forwards to enabled widgets.
- RoadNetwork/RoadMap.java:22, :36 addNodeToNetwork, :45 removeMapNode; MapNode.java:171, :180, :189 factories; :242 removeConnection, :284 addConnection; Connection.java:13.

Q2
- Managers/CurveManager.java:39 (CurveManager) curvesList; :73 registerCurveType; :89 addActiveCurve (reflection); :172 setActiveCurve; :195 setCurrentCurveFromNode; :268 commitActiveCurve; :287 cancelActiveCurve; :310 updateAllCurves; :317 cancelAllCurves; :335 onButtonSelected; :362 onButtonDeselected; :409 isControlNode; :421 isAnchorNode; :503 getControlNodeAt; 555-576 setters; 589-608 forward events; :674 CurveInfo.
- GUI/Curves/CurvePanel.java:169-184 commit -> commitActiveCurve.
- RoadNetwork/MapNode.java:197, :203, :209 control/rotation/special factories.

Q3
- Managers/ChangeManager.java:23 (Changeable) undo :25, redo :28; :62 addChangeable; :81 undo; :136 redo; :163 Node (doubly linked); 11-12 describe queue; 60-73 menu enable state.
- Managers/CopyPasteManager.java:27, :51 addChangeable; :68 CutNodesChanger; :106 PasteSelectionChanger.
- GUI/Menus/EditMenu/UndoMenu.java:17, :23; RedoMenu.java:17, :23 (Ctrl+Shift+Z per this locator).

Q4
- Managers/MultiSelectManager.java:26: static multiSelectList (:33), selectedNodes (:35), isMultipleSelected (:36), multiSelectRect (:44), listeners (:58); :75 mouseClicked, :91 mousePressed, :102 mouseReleased, :157 mouseDragged; rectangle built while dragging, applied on release; :114 currentButton.addSelectedToMultiSelectList(); :258 addToMultiSelectList, :268 remove, :278 startMultiSelect, :301 stopMultiSelect, :321 clearMultiSelection, :388 getSelectionBounds, :425/:432 listeners; :64 MultiSelectEventListener implemented by ButtonManager:22, WidgetManager:22.

Not found: Handlers only GlobalExceptionHandler; no keyboard listener class; key handling MapPanel.java:142 InputMap and Managers/ShortcutManager.java:174-209 (registerShortcut, registerMenuShortcut); no node edit logic in Managers/Listeners.
