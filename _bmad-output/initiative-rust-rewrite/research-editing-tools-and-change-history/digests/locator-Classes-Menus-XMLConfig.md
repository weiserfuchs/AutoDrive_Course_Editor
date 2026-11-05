Digest for area Classes/, GUI/Menus/, XMLConfig/, resources @ e4fe43e. Paths under src/main/java/AutoDriveEditor/.

Q1
- Classes/Interfaces/WidgetInterface.java:10 (WidgetInterface): mousePressed/Released/Dragged + onMultiSelect* (19-40).
- Classes/Interfaces/SelectorInterface.java:10 (SelectorInterface).
- Classes/Widgets/MoveWidget.java:53, :73, :132 (mousePressed, mouseReleased, mouseDragged): drag-move; release records MoveWidgetChanger at :77; anchor node 89-120.
- Classes/Widgets/WidgetBase.java (WidgetBase) not read.
- Classes/Widgets/Selectors/MoveSelector.java:34, :56 (checkWidgetSelection, checkSelectedAxis).
- Classes/LinearLine.java:43 (LinearLine): MouseMotionListener, MouseWheelListener; creates nodes/connections; records at :279, :285, :289 (LinearLineChanger FULL, MANUAL, CONNECTION_ONLY).
- Classes/SnapShot.java:30 (SnapShot): createNewConnection (:452), insertMapNode (:520).
- Classes/KeyBinds/Shortcut.java:16 (Shortcut), :40 (ShortcutEventInterface); ShortcutGroup.java:13.
- Classes/UI_Components/ToolbarButton.java (20+ files, not read).
- GUI/Menus/EditMenu: Copy, Cut, Paste, PasteOriginalLocation (4 files), e.g. CopyMenu.java.

Q2
- Classes/Interfaces/CurveInterface.java:16 (mouse :23-25); ArcSplineInterface.java:11 (rotateStartNode, rotateEndNode, rotateControlNode :19-21).
- Classes/Curves/CurveBase.java:37 (CurveBase): mouse stubs 84-96, mouseMoved 101, cancelCurve 156, updateCurve 167, setCurveInterpolations 268, setNumCurrentControlPoints 281, preview lists 202, 208, drawToScreen 320, CurveChanger 366.
- Classes/Curves/BezierCurve.java:23 (BezierCurve): curveControlPointDefault from EditorXML (:21); commitCurve :302, undo registration :324.
- Classes/Curves/ArcSpline.java:29 (ArcSpline): mouseReleased :145 initial control point; commitCurve :449; undo :471.
- Classes/Widgets/CurveWidget.java:18, :25, :60, :77 (mouseReleased, mouseDragged, moveNodesBy).
- Classes/Widgets/ArcSplineWidget.java:21, :38, :61, :90.
- Classes/Widgets/Selectors/RadiusSelector.java:18; RotationSelector.java, SelectorBase.java not read.
- XMLConfig/EditorXML.java:42 curve defaults; loadEditorXMLConfig :189, saveEditorXMLConfig :501.

Q3
- ChangeManager.Changeable implemented everywhere; ChangeManager outside area.
- GUI/Menus/EditMenu/UndoMenu.java:12, :23 Ctrl+Z changeManager.undo(); RedoMenu.java:12, :23 VK_Z with SHIFT_DOWN_MASK changeManager.redo().
- Classes/SnapShot.java:30: restoreOriginalNodes ~340-355, removeNewCopyOfNodes ~388-392, createNewCopyOfNodes ~414-439, NodeBackup :578.
- Classes/Curves/CurveBase.java:366-398 (CurveChanger) snapshot-based.
- Classes/Widgets/MoveWidget.java:335-372 (MoveWidgetChanger).
- Classes/LinearLine.java:525-576 (LinearLineChanger), full/connection-only :555, :558.
- GUI/Menus/DebugMenu/Logging/LogUndoRedoMenu.java bDebugLogUndoRedoInfo.

Q4
- Classes/Widgets/MoveWidget.java:301-326 onMultiSelectChange/Add/Remove/Cleared; uses MultiSelectManager.multiSelectList.
- WidgetInterface.java:34-40 callbacks.
- RotationSelector.java, Classes/Util_Classes/RotationUtils.java, MathUtils.java not read.
- Classes/KDTree/KDTree3D.java:15 role in selection unverified.
- No align/distribute classes in area (grep for 'align' not run).

Resources: locale/AutoDriveEditor_en_US.properties, log4j2.xml, themes/FlatLaf*.properties, editor/actionbar/*.svg, META-INF/MANIFEST.MF.
Nothing found: ChangeManager definition in area; MouseListener registration in area.
