DIGEST Q3 undo/redo @ e4fe43e. …/ = src/main/java/AutoDriveEditor/. 18 files (3 over budget).

1 History
- [load-bearing] …/Managers/ChangeManager.java:31-34, 163-175 (ChangeManager, Node): doubly linked list Node{left,right,changeable}, cursor currentIndex, sentinel parentNode.
- …/Managers/ChangeManager.java:23-29 (Changeable) undo(), redo().
- [load-bearing] …/Managers/ChangeManager.java:62-69 (addChangeable): no size limit; new change sets currentIndex.right = node, orphaning redo entries; only enables menu_Undo (68), Redo menu can stay enabled; redo() then logs and disables (136-142).
- canUndo currentIndex != parentNode; canRedo currentIndex.right != null (73, 77).
- …/Managers/ChangeManager.java:81-100, 107-115 (undo, moveLeft): suspendAutoSaving, undo, moveLeft (menu states), resumeAutoSaving.
- …/Managers/ChangeManager.java:136-155 (redo): suspend, moveRight, redo, resume.
- [load-bearing] …/Managers/ChangeManager.java:54-58 (clear): currentIndex = parentNode, disable menus; callers only …/XMLConfig/GameXML.java:84 and …/XMLConfig/RoutesXML.java:106 (config load).
- Created once …/AutoDriveEditor.java:95, field :47; currentIndex null until first clear; copy constructor 46-49.
- Toolbar Undo/Redo buttons never enabled/disabled by manager.

2 Recording styles
- Style A deltas [load-bearing]: MoveWidgetChanger …/Classes/Widgets/MoveWidget.java:335-384 (added MoveWidget.java:77, CurveWidget.java:28) moveNodesBy(∓diff); AlignmentChanger …/GUI/Buttons/AlignBaseButton.java:81-125 ZStore diffs (DepthAlignButton:43, HorizontalAlignButton:60, VerticalAlignButton:61); RotateNodeChanger …/GUI/Buttons/Toolbar/Nodes/RotationButton.java:211-245 ±angle, isStale from RotationButton.java:119; NodePriorityChanger …/GUI/Buttons/Toolbar/Nodes/SwapNodePriorityButton.java:129-172 toggle flag; FlipConnectionChanger …/GUI/Buttons/Toolbar/Nodes/FlipDirectionButton.java:289-348; MarkerRemoveChanger …/GUI/Buttons/Toolbar/Markers/DeleteMarkerButton.java:89-117 (Add/Edit at AddMarkerButton.java:191, EditMarkerButton.java:205 not read); CoordinateChanger …/GUI/Menus/ScanMenu/OutOfBoundsFixMenu.java:79-125.
- Style B snapshots [load-bearing]: AddNodeChanger …/GUI/Buttons/AddNodeBaseButton.java:161-193; DeleteNodeChanger …/GUI/Buttons/Toolbar/Nodes/DeleteNodeButton.java:92-127, 139-177 (undo restore, checkAreaForNodeOverlap, setStale(saved)); LinearLineChanger …/Classes/LinearLine.java:276-289, 525-582 FULL/MANUAL/CONNECTION_ONLY, reused nodes removed from snapshot (283); CurveChanger …/Classes/Curves/CurveBase.java:366-404 (ArcSpline.java:471, BezierCurve.java:324); CutNodesChanger, PasteSelectionChanger …/Managers/CopyPasteManager.java:68-136, paste createNewCopyOfNodes then SnapShot (50-51), copy not recorded (37-44).
- …/RoadNetwork/MapNode.java:500-565 MapNodeStore unused outside MapNode.java.

3 Undo of delete
- [load-bearing] …/Classes/SnapShot.java:198-205, 578-619, 632-634 (createSnapShot, NodeBackup, compareTo): NodeBackup holds live MapNode ref, mapNodeID, x/y/z, flag, marker data; TreeMap sorted by mapNodeID. canDelete false skipped (76-84).
- …/Classes/SnapShot.java:207-246 connectionList (DUAL de-duplicated), otherOutgoing, otherIncoming.
- …/Classes/SnapShot.java:308-328 (removeOriginalNodes) remove connections then RoadMap.removeMapNode; …/RoadNetwork/RoadMap.java:45-58 decrement higher ids.
- [load-bearing] …/Classes/SnapShot.java:335-342 (restoreOriginalNodes) ascending id, set id, insertMapNode; …/Classes/SnapShot.java:491-508 (insertMapNode) checks left neighbour id/index, throws MismatchedIdException before change; :513-521 id++ right nodes, add at id-1. Same MapNode object re-inserted; stored coords not copied back (used for paste only). Connections re-added …/Classes/SnapShot.java:346-357, 376-380 order connectionList, otherOutgoing, otherIncoming, skip null ends; setSelected(false) (342).
- Errors: DeleteNodeChanger.undo catches MismatchedIdException -> showMismatchedIDError; others wrap in RuntimeException.

4 Stale flag
- …/GUI/MapPanel.java:1068, 1084-1092 (isStale, setStale) static boolean; title change; save menu enabled = stale. Title " *" …/AutoDriveEditor.java:244; close prompt 112-117.
- [load-bearing] Pattern: constructor captures isStale(), undo setStale(saved), redo setStale(true). Examples …/GUI/Buttons/Toolbar/Nodes/DeleteNodeButton.java:142-147, 160, 173; …/Classes/Widgets/MoveWidget.java:350, 368, 381; …/Classes/LinearLine.java:535, 561, 580; …/Managers/CopyPasteManager.java:75, 89, 97. Not recomputed from saved state. Changers built before setStale(true) (MoveWidget 77-78; AddNodeBaseButton 130-132; CopyPasteManager 50-52; LinearLine :293).
- undo/redo wrapped in suspend/resume autosave; RotateNodeChanger and LinearLineChanger don't wrap own bodies.

5 Key bindings
- …/GUI/Menus/EditMenu/UndoMenu.java:17 makeMenuItem VK_Z CTRL_DOWN_MASK (Ctrl+Z), disabled initially; :23 undo().
- [load-bearing] …/GUI/Menus/EditMenu/RedoMenu.java:17 VK_Z SHIFT_DOWN_MASK (Shift+Z, not Ctrl+Shift+Z or Ctrl+Y); :23 redo().
- …/GUI/Menus/JMenuItemBase.java:31-39 setAccelerator; EditorMenu.java:71-72 adds items.
- Undo/redo not in ShortcutManager list. UndoButton/RedoButton …/GUI/Buttons/ActionBar/ChangePanel/ call at :38, no accelerator; UndoButton ignoreButtonDeselect true (:32).

Not found: cap/merge/coalescing; clear on save. Not read: AddMarker/EditMarker changers, addNodeToNetwork, addConnection bodies.
