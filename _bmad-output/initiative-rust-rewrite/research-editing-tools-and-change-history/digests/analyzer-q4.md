DIGEST Q4 selection and alignment @ e4fe43e. Paths under src/main/java/AutoDriveEditor/. 11 files.

1 Selection (Managers/MultiSelectManager.java)
- :90-99 (mousePressed), :278-299 (startMultiSelect): BUTTON3 only, tool useMultiSelection(), pdaImage loaded; no modifier keys. Stores selectStart world pos, resets rect, onMultiSelectStart; freeform path start.
- [load-bearing] :156-197 (mouseDragged) rectangle: clears previews and selectedNodes, rebuild normalised rect, tests all networkNodesList with strict < > on x and z (edges excluded).
- :211-226, :311 freeform: worldPosToScreenPos in freeformSelectionPath.contains, lineTo per drag, closePath in stopMultiSelect.
- GUI/Buttons/ActionBar/ChangePanel/MultiSelectButton.java:50-65, :39-47 right-click toggles freeform/rectangular.
- [load-bearing] :188, :218 and RoadNetwork/MapNode.java:411: node qualifies if tool alwaysSelectHidden() or isSelectable() (= !isNodeHidden || bSelectHidden). bSelectHidden persisted default false (GUI/Buttons/ActionBar/ConfigPanel/SelectHiddenButton.java:31-39, :56-73; XMLConfig/EditorXML.java:159, :245, :548). BaseButton.java:53 default false; HideNodeButton.java:46 true. TODO :187 "Fix node visibility check". No node-type filter in main loop.
- :199-210, :227-239 curve preview control nodes added (skip rotation nodes, no hidden check).
- :191-193 preview flags per tool.
- [load-bearing] :102-147 (mouseReleased): clear previews, stopMultiSelect; if selectedNodes non-empty and addSelectedToMultiSelectList(): toggle each (remove + setSelected(false) if present; else add, setSelected(true) only with previewNodeSelectionChange); :142-147 isMultipleSelected, Edit menu, onMultiSelectAdd/Remove/Change. :148-151 else onMultiSelectOneTime only.
- :74-88, :321-335 BUTTON3 click clears selection unless ignoreDeselect(); fires after drag release too.
- :33-36, :258-276 static multiSelectList, selectedNodes, isMultipleSelected; removing last node clears.
- MultiSelectButton.java:68-83 single click toggles membership.
- :388-423 getSelectionBounds; :459-492 drawToScreen white rect if bShowSelectionBounds.
- getAllNodesInArea/getAllNodesInRectangle private, no caller found in file.

2 Delivery to tools
- Static multiSelectList (:33) read directly by align, copy/cut/paste.
- :58-72, :425-434 MultiSelectEventListener seven methods.
- Managers/ButtonManager.java:525-590 forwards only to current button with useMultiSelection(); BaseButton.java:111-123 defaults.
- Overrides: DeleteNodeButton.java:88 onMultiSelectChange; SwapNodePriorityButton.java:80-86 onMultiSelectOneTime.
- Managers/WidgetManager.java:172-216; :208-210 OneTime calls widget.onMultiSelectRemove (slip).
- Classes/Widgets/MoveWidget.java:300-329 anchor management.
- ButtonManager.java:102-133 flag docs.

3 Alignment
- [load-bearing] GUI/Buttons/AlignBaseButton.java:35-51 (mouseClicked): needs non-empty multiSelectList and isMultipleSelected and click on node (reference via getNodeAtScreenPosition); suspendAutoSaving, adjustNodesTo, updateAllCurves if preview, setStale(true), clearMultiSelection, repaint, resume.
- AlignBaseButton.java:53-79 dashed preview line (horizontal at screen y / vertical at screen x).
- [load-bearing] GUI/Buttons/Toolbar/Alignment/HorizontalAlignButton.java:57-65 node.z = toNode.z, checkNodeOverlap, AlignmentChanger(list,0,0,toNode.z).
- [load-bearing] GUI/Buttons/Toolbar/Alignment/VerticalAlignButton.java:58-66 node.x = toNode.x, checkNodeOverlap, AlignmentChanger(list,toNode.x,0,0).
- GUI/Buttons/Toolbar/Alignment/DepthAlignButton.java:19-51 node.y = toNode.y, no checkNodeOverlap, ALIGN_NONE no preview.
- One axis each; no interpolation/spacing/ordering; connections untouched.
- AlignBaseButton.java:81-146 AlignmentChanger: ZStore diff = node.axis - arg for non-zero arg else 0; undo +=, redo -=, checkNodeOverlap. Quirk: target 0 treated as no-move, undo won't restore.
- GUI/Buttons/Toolbar/Alignment/EditLocationButton.java:40-84 dialog X/Y/Z (x,z ±1024*mapScale, y 0..1024*mapScale), rounded 3 decimals; no undo, no checkNodeOverlap, no stale flag.

4 Copy/cut/paste
- CopyButton.java:53-54, CutButton.java:38-39 need isMultipleSelected && non-empty.
- Managers/CopyPasteManager.java:37-44 (copySelection) SnapShot + clear selection.
- CopyPasteManager.java:24-35 (cutSelection) SnapShot, CutNodesChanger, removeOriginalNodes, clear, stale.
- [load-bearing] Classes/SnapShot.java:62-87, RoadNetwork/MapNode.java:375-392: drops canDelete() false (control, rotation, hidden when !bSelectHidden, uncommitted curve anchors).
- SnapShot.java:85, :198-205, :432 bounds/centre, NodeBackup.
- SnapShot.java:207-246 connectionList (DUAL once), otherOutgoing, otherIncoming.
- SnapShot.java:308-328 removeOriginalNodes.
- [load-bearing] SnapShot.java:335-358, :491-522 restoreOriginalNodes insertMapNode, shifts later ids, MismatchedIdException.
- CopyPasteManager.java:46-56 (pasteSelection) null snapshot no-op; createNewCopyOfNodes; PasteSelectionChanger.
- [load-bearing] SnapShot.java:403-456 (createNewCopyOfNodes): ids from size()+1; copies x, z, y recomputed from heightmap, flag, marker; only internal connections recreated with same types; external not copied.
- SnapShot.java:410-418, :426-428: inOriginalLocation keeps x/z; else shift so centre lands at viewport centre; 3 decimals.
- PasteButton.java:40 false; PasteMenu.java:24 false; PasteOriginalLocationMenu.java:24 true.
- CopyPasteManager.java:68-136 undo/redo of paste and cut.
- Snapshot not cleared after paste: repeatable.

Not found: modifiers in selection; spacing/distribution; callers of getAllNodesInArea/Rectangle (file only).
