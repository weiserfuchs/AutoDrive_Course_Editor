---
title: 'Codebase research: editing tools and change history'
type: codebase
ask: |-
  Complete feature inventory of the Java/Swing AutoDrive Course Editor as a baseline; this run covers editing tools and change history.
repository: '.'
commit: 'e4fe43e93f225e8c6d7635d62fb8cfdfd845a971'
branch: 'master'
status: complete
validation: 'normal'
claims: {verified: 36, disputed: 5, corrected_in_text: 5}
created: '2026-11-05'
updated: '2026-11-05'
---

# Codebase research: editing tools and change history

## Questions

1. (how) How does a toolbar tool receive mouse and keyboard input and change the road network (create, move and delete nodes; connections)?
2. (how) How are curves (Bezier, arc spline) created, edited and committed to nodes?
3. (how) How are changes recorded and undone or redone?
4. (how) How do selection and alignment tools work?

## 1. How does a toolbar tool receive mouse and keyboard input and change the road network (create, move and delete nodes; connections)?

**Mouse path.**
- `MapPanel` registers one `MouseListener` object for mouse, motion and wheel events: src/main/java/AutoDriveEditor/GUI/MapPanel.java:135-138 (MapPanel) @ e4fe43e.
- The click, press, release, drag and move handlers store the mouse position and then fan the event out in a fixed order: `multiSelectManager`, `widgetManager`, `buttonManager`, `curveManager`, and finally the `MapPanel` hook: src/main/java/AutoDriveEditor/Listeners/MouseListener.java:33-97 (mouseClicked) @ e4fe43e.
- Enter and exit events go to `buttonManager` only.
- The wheel handler zooms first. It then forwards to `widgetManager`, `buttonManager` and `curveManager`, skipping `multiSelectManager` and the `MapPanel` hook: src/main/java/AutoDriveEditor/Listeners/MouseListener.java:99-106 (mouseWheelMoved) @ e4fe43e.
- `ButtonManager` forwards each event only to the current tool, if there is one: src/main/java/AutoDriveEditor/Managers/ButtonManager.java:510-517 (mouseClicked) @ e4fe43e.
- `WidgetManager` forwards to every enabled widget, whichever tool is current: src/main/java/AutoDriveEditor/Managers/WidgetManager.java:120-154 (mouseClicked) @ e4fe43e.
- Tools extend `BaseButton`, which supplies empty defaults for the mouse hooks, the lifecycle hooks and the behaviour flags (`useMultiSelection`, `usePanelEdgeScrolling` and others): src/main/java/AutoDriveEditor/GUI/Buttons/BaseButton.java:39-123 (BaseButton) @ e4fe43e.

**Active tool.**
- The toolbar registers the tools with `buttonManager.addButton`: src/main/java/AutoDriveEditor/GUI/Toolbar.java:41-50 (Toolbar) @ e4fe43e.
- The active tool is the static `currentButton`. `makeCurrent` deselects the old tool, selects the new one and notifies `curveManager`: src/main/java/AutoDriveEditor/Managers/ButtonManager.java:360-381 (makeCurrent) @ e4fe43e.
- A toolbar click toggles through `makeCurrent` or `deSelectAll`: src/main/java/AutoDriveEditor/GUI/Buttons/BaseButton.java:158-178 (actionPerformed) @ e4fe43e.
- Pressing a shortcut repeatedly cycles through tool groups: add primary/secondary node, and normal/dual/reverse connection.

**Keyboard.**
- Shortcuts are bound on the `MapPanel` InputMap with `WHEN_IN_FOCUSED_WINDOW`: src/main/java/AutoDriveEditor/Managers/ShortcutManager.java:169-185 (registerShortcut) @ e4fe43e.
- The defaults sit in one list: src/main/java/AutoDriveEditor/Managers/ShortcutManager.java:87-116 (ShortcutManager) @ e4fe43e.
  - Tools: M move, Delete delete, L line, N node, P priority, F flip, R rotate, B Bézier.
  - Markers: W add, E edit, Shift+D delete.
  - Alignment: Shift+X horizontal, Shift+Y vertical, and Shift+F focus node.
  - Clipboard: Ctrl+X, Ctrl+C, Ctrl+V, and Shift+V to paste at the original location.
  - Toggles: H select hidden, G grid, S grid snap, D subdivision snap, numpad +/- node size, `;` continuous connection.
  - Display: F1 marker names, F2 marker icons, F3 parking icons, F4 node ID.
- Holding Shift inverts grid snapping. It is hard-coded in `MapPanel`: src/main/java/AutoDriveEditor/GUI/MapPanel.java:144-157 (MapPanel) @ e4fe43e.
- Undo and redo are menu accelerators, not in this list (section 3).

**Create node.**
- On left release, the tool takes the y value from the heightmap and creates the node: src/main/java/AutoDriveEditor/GUI/Buttons/AddNodeBaseButton.java:90-98 (mouseReleased) @ e4fe43e.
- `createNode` suspends autosave, calls `MapNode.createNewNetworkNode`, records an `AddNodeChanger` and sets the stale flag: src/main/java/AutoDriveEditor/GUI/Buttons/AddNodeBaseButton.java:127-138 (createNode) @ e4fe43e.
- The new node gets id `size()+1` and is appended: src/main/java/AutoDriveEditor/RoadNetwork/MapNode.java:189-195 (createNewNetworkNode) @ e4fe43e.

**Move node.**
- The move tool only enables a `MoveWidget`.
- Pressing on a widget axis starts a drag.
- The drag computes its step in `calcMoveOffset`, which clamps to ±1024*mapScale and applies grid snapping. The X step is rounded to 3 decimals and the Z step to 5. The Z bounds check reuses the X bounds: src/main/java/AutoDriveEditor/Classes/Widgets/MoveWidget.java:147-186 (calcMoveOffset) @ e4fe43e.
- Release after a drag records a `MoveWidgetChanger`. A release without a drag edits the selection instead: src/main/java/AutoDriveEditor/Classes/Widgets/MoveWidget.java:72-79 (mouseReleased) @ e4fe43e.

**Delete node.**
- A click, or the Delete key whichever tool is active, creates a `SnapShot`, records a `DeleteNodeChanger` and removes the nodes: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Nodes/DeleteNodeButton.java:92-105 (removeNode) @ e4fe43e.
- Removal strips every list entry that points to the node and decrements every later id: src/main/java/AutoDriveEditor/RoadNetwork/RoadMap.java:45-58 (removeMapNode) @ e4fe43e.

**Connections.**
- The line tools take a start node on the first click.
  - A second click on the same node cancels.
  - A click on another node commits, and continues from that node if continuous connections are on.
  - A click on empty space commits with a new end node, if that option is on: src/main/java/AutoDriveEditor/GUI/Buttons/LinerLineBaseButton.java:83-143 (mousePressed) @ e4fe43e.
- `LinearLine.commit` creates nodes for the interpolated points, reusing any node already at a position, and interpolates y linearly. It connects the nodes and records a `LinearLineChanger` (FULL, MANUAL or CONNECTION_ONLY): src/main/java/AutoDriveEditor/Classes/LinearLine.java:219-296 (commit) @ e4fe43e.
- `createConnectionBetween` is the single place that edits `incoming` and `outgoing`: src/main/java/AutoDriveEditor/GUI/MapPanel.java:781-836 (createConnectionBetween) @ e4fe43e.
  - It rejects a self-link or a node that is not in the list.
  - It adds entries by type, and toggles the link off when it already exists.
  - A comment warns that edge cases can corrupt a config.

**Other tools.**
- Swap priority flips `flag = 1 - flag`: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Nodes/SwapNodePriorityButton.java:74-99 (mousePressed) @ e4fe43e.
- Flip direction re-adds connections in four ordered passes, which is a documented workaround: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Nodes/FlipDirectionButton.java:84-152 (mouseReleased) @ e4fe43e. DUAL and non-selectable connections are filtered out beforehand: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Nodes/FlipDirectionButton.java:154-165 (filterConnections) @ e4fe43e.
- Rotate records a `RotateNodeChanger` with the total angle: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Nodes/RotationButton.java:101-130 (mouseDragged) @ e4fe43e.

## 2. How are curves (Bezier, arc spline) created, edited and committed to nodes?

**Structure.**
- `CurveManager` registers two curve types, Bézier and ArcSpline: src/main/java/AutoDriveEditor/Managers/CurveManager.java:59-60 (CurveManager) @ e4fe43e. ArcSpline is labelled "Work In Progress".
- Both extend `CurveBase`. A curve's preview nodes and preview connections are kept outside the road network until it is committed: src/main/java/AutoDriveEditor/Classes/Curves/CurveBase.java:37-58 (CurveBase) @ e4fe43e.

**Creation.**
- The Bézier tool takes a start node and then an end node. The second click calls `addActiveCurve`: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Curves/BezierCurveButton.java:80-108 (mouseReleased) @ e4fe43e.
- `addActiveCurve` instantiates the curve class by reflection, matching the constructor's parameters: src/main/java/AutoDriveEditor/Managers/CurveManager.java:89-133 (addActiveCurve) @ e4fe43e.
- An ArcSpline is created at the clicked world position.

**Bézier math.**
- For N interpolation points, the parameter step is `t = i/(N+1)` for i = 1..N. The end points are excluded, and spacing is uniform in t, not in arc length: src/main/java/AutoDriveEditor/Classes/Curves/BezierCurve.java:113-128 (generateInterpolationPoints) @ e4fe43e.
- 1 to 4 control points give a quadratic up to a quintic Bernstein polynomial over x and z: src/main/java/AutoDriveEditor/Classes/Curves/BezierCurve.java:158-171 (calcPointsForCurve) @ e4fe43e.
- The curve is shaped by *virtual* control points. Dragging moves the visible control node by the plain mouse difference. When `applyScaling` is true, the virtual point moves `controlPointMoveScaler` times as far (default 3); otherwise it moves by the plain difference: src/main/java/AutoDriveEditor/Classes/Curves/BezierCurve.java:72-87 (moveControlNodeBy) @ e4fe43e. The curve widget's drag passes true.

**ArcSpline math.**
- It produces N+2 points evenly spaced in angle, with both ends included: `x = cx + r·cos`, `z = cz + r·sin`. The default is a half circle with radius 8 and a minimum radius of 2: src/main/java/AutoDriveEditor/Classes/Curves/ArcSpline.java:392-418 (generateArcPoints) @ e4fe43e.
- The widget offers move, rotate (start and end angle, optionally snapped) and radius handles.

**Parameters.**
- The defaults are 6 interpolation points, a slider maximum of 50, 1 control point, a move scaler of 3, regular priority and REGULAR connections: src/main/java/AutoDriveEditor/XMLConfig/EditorXML.java:85-91 (EditorXML) @ e4fe43e.
- The curve panel has these controls:
  - an interpolation slider, 1..max
  - a control-point slider: 1..4 for Bézier, fixed at 4 for ArcSpline
  - priority and connection-type radios (REGULAR, REVERSE, DUAL)
  - swap direction, commit and cancel buttons
- The panel is at src/main/java/AutoDriveEditor/GUI/Curves/CurvePanel.java:127-215 (CurvePanel) @ e4fe43e.
- A new curve hard-codes REGULAR and priority 0. The XML defaults for connection type and priority are not read by the curve classes: src/main/java/AutoDriveEditor/Classes/Curves/CurveBase.java:48-69 (CurveBase) @ e4fe43e.

**Commit.**
- The commit button runs `commitCurve`, removes the curve and activates the previous one: src/main/java/AutoDriveEditor/Managers/CurveManager.java:268-281 (commitActiveCurve) @ e4fe43e.
- A Bézier commit:
  - fills in a missing end y from the other end
  - interpolates y linearly; with N=1 this divides by zero
  - creates N network nodes with the curve priority as their flag
  - connects start → nodes → end with N+1 `createConnectionBetween` calls of the chosen type
  - records a snapshot-based `CurveChanger`

  The code is at src/main/java/AutoDriveEditor/Classes/Curves/BezierCurve.java:302-343 (commitCurve) @ e4fe43e.
- An ArcSpline commit creates N+2 nodes with y = 0 and connects only consecutive arc nodes, so the arc stands alone: src/main/java/AutoDriveEditor/Classes/Curves/ArcSpline.java:449-498 (commitCurve) @ e4fe43e.

**Persistence across tool switches.**
- `CurveManager` draws every curve in `curvesList` on every frame and forwards every mouse event to every curve. Switching tools leaves `curvesList` untouched: src/main/java/AutoDriveEditor/Managers/CurveManager.java:589-623 (mouseClicked) @ e4fe43e.
- Deselecting any tool disables the current curve's widget. The check for the owner tool is commented out: src/main/java/AutoDriveEditor/Managers/CurveManager.java:362-368 (onButtonDeselected) @ e4fe43e.
- Selecting a tool restores the active curve, anchor and selector for each of its active curves that has an active node. The call that re-enables the widget is commented out there: src/main/java/AutoDriveEditor/Managers/CurveManager.java:335-352 (onButtonSelected) @ e4fe43e.
- `isActive` is cleared only for curves of the same type, so a Bézier preview and an ArcSpline preview can be active together.

## 3. How are changes recorded and undone or redone?

**History structure.**
- `ChangeManager` keeps a doubly linked list of `Node{left,right,changeable}` with a cursor and a sentinel: src/main/java/AutoDriveEditor/Managers/ChangeManager.java:163-175 (Node) @ e4fe43e. `Changeable` has only `undo()` and `redo()`.
- `addChangeable` has no size limit. It links the new node to the right of the cursor, which orphans any redo entries. It enables only the Undo menu item, so the Redo item can stay enabled when nothing is left to redo: src/main/java/AutoDriveEditor/Managers/ChangeManager.java:62-69 (addChangeable) @ e4fe43e.
- Undo and redo suspend autosave while they run: src/main/java/AutoDriveEditor/Managers/ChangeManager.java:81-155 (undo) @ e4fe43e.
- The history is cleared only when a config is loaded: src/main/java/AutoDriveEditor/Managers/ChangeManager.java:54-58 (clear) @ e4fe43e, src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:84 (loadGameConfig) @ e4fe43e.

**Two recording styles.**
- Deltas that invert the edit:
  - move: src/main/java/AutoDriveEditor/Classes/Widgets/MoveWidget.java:335-384 (MoveWidgetChanger) @ e4fe43e
  - align: src/main/java/AutoDriveEditor/GUI/Buttons/AlignBaseButton.java:81-125 (AlignmentChanger) @ e4fe43e
  - rotate: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Nodes/RotationButton.java:211-245 (RotateNodeChanger) @ e4fe43e
  - priority: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Nodes/SwapNodePriorityButton.java:129-172 (NodePriorityChanger) @ e4fe43e
  - flip: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Nodes/FlipDirectionButton.java:289-348 (FlipConnectionChanger) @ e4fe43e
  - markers: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Markers/DeleteMarkerButton.java:89-117 (MarkerRemoveChanger) @ e4fe43e
  - the out-of-bounds fix: src/main/java/AutoDriveEditor/GUI/Menus/ScanMenu/OutOfBoundsFixMenu.java:79-125 (CoordinateChanger) @ e4fe43e
- `SnapShot`-based changers, which remove or restore whole node sets:
  - add node: src/main/java/AutoDriveEditor/GUI/Buttons/AddNodeBaseButton.java:161-193 (AddNodeChanger) @ e4fe43e
  - delete: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Nodes/DeleteNodeButton.java:139-177 (DeleteNodeChanger) @ e4fe43e
  - line: src/main/java/AutoDriveEditor/Classes/LinearLine.java:525-582 (LinearLineChanger) @ e4fe43e
  - curve: src/main/java/AutoDriveEditor/Classes/Curves/CurveBase.java:366-404 (CurveChanger) @ e4fe43e
  - cut and paste: src/main/java/AutoDriveEditor/Managers/CopyPasteManager.java:68-136 (CutNodesChanger) @ e4fe43e

**Undo of a delete.**
- The snapshot keeps the live `MapNode` objects with their original ids, sorted by id: src/main/java/AutoDriveEditor/Classes/SnapShot.java:198-205 (createSnapShot) @ e4fe43e.
- It splits connections into internal, outgoing-to-outside and incoming-from-outside: src/main/java/AutoDriveEditor/Classes/SnapShot.java:207-246 (createSnapShot) @ e4fe43e.
- Restore re-inserts each node at `id-1` in ascending id order: src/main/java/AutoDriveEditor/Classes/SnapShot.java:335-358 (restoreOriginalNodes) @ e4fe43e.
  - It increments every later id.
  - It throws `MismatchedIdException` if the left neighbour's id or index does not match: src/main/java/AutoDriveEditor/Classes/SnapShot.java:491-521 (insertMapNode) @ e4fe43e.
- Positions come back because the same objects are re-inserted. All three connection sets are then re-added.

**Stale flag.**
- `setStale` updates the window title (a trailing `*`) and the save menu item: src/main/java/AutoDriveEditor/GUI/MapPanel.java:1084-1092 (setStale) @ e4fe43e.
- Every changer captures `isStale()` when it is created. Undo restores that value and redo sets the flag to true. The flag is never recomputed against the saved file.

**Key bindings.**
- Undo is Ctrl+Z: src/main/java/AutoDriveEditor/GUI/Menus/EditMenu/UndoMenu.java:17 (UndoMenu) @ e4fe43e.
- Redo is **Shift+Z** (`VK_Z` with `SHIFT_DOWN_MASK`), not Ctrl+Y or Ctrl+Shift+Z: src/main/java/AutoDriveEditor/GUI/Menus/EditMenu/RedoMenu.java:17 (RedoMenu) @ e4fe43e.
- Both are menu accelerators and are not in the configurable shortcut list.

Not found: any cap, merging or coalescing of history entries, or a clear on save.

## 4. How do selection and alignment tools work?

**Building a selection.**
- Multi-select uses the right mouse button only, with no modifier keys. It runs only when the current tool's `useMultiSelection()` is true: src/main/java/AutoDriveEditor/Managers/MultiSelectManager.java:90-99 (mousePressed) @ e4fe43e.
- In rectangle mode, each drag tests every node with strict `<`/`>` on x and z, so nodes on the edges are excluded; only nodes passing the selectability check below are added: src/main/java/AutoDriveEditor/Managers/MultiSelectManager.java:156-200 (mouseDragged) @ e4fe43e. Freeform mode tests screen positions against a lasso path instead.
- A node qualifies when the tool's `alwaysSelectHidden()` is true or the node is selectable (`!hidden || bSelectHidden`). Control nodes from an active curve preview are added as well.
- On release, each picked node is toggled in the static `multiSelectList`, if the tool collects selections, and the listeners are notified. Otherwise the manager fires a one-time event: src/main/java/AutoDriveEditor/Managers/MultiSelectManager.java:102-151 (mouseReleased) @ e4fe43e.
- A right click clears the selection unless the tool ignores deselection.

**How tools receive the selection.**
- Tools read the static `multiSelectList` directly.
- Events reach the current tool through `ButtonManager`, only if that tool uses multi-selection: src/main/java/AutoDriveEditor/Managers/ButtonManager.java:525-590 (onMultiSelectStart) @ e4fe43e.
- `WidgetManager` forwards one-time events to widgets as `onMultiSelectRemove`: src/main/java/AutoDriveEditor/Managers/WidgetManager.java:208-210 (onMultiSelectOneTime) @ e4fe43e.

**Alignment.**
- First select nodes, then click a reference node with the align tool: src/main/java/AutoDriveEditor/GUI/Buttons/AlignBaseButton.java:35-51 (mouseClicked) @ e4fe43e.
- Each tool copies one coordinate from the reference to every selected node:
  - Horizontal sets `z`: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Alignment/HorizontalAlignButton.java:57-65 (adjustNodesTo) @ e4fe43e.
  - Vertical sets `x`: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Alignment/VerticalAlignButton.java:58-66 (adjustNodesTo) @ e4fe43e.
  - Depth sets `y`, without an overlap check.
- There is no interpolation, no spacing and no ordering, and connections are not changed.
- `AlignmentChanger` treats a target of exactly 0 as "do not move this axis", so undo after aligning to 0 does not restore the old value: src/main/java/AutoDriveEditor/GUI/Buttons/AlignBaseButton.java:81-146 (AlignmentChanger) @ e4fe43e.
- Edit Location is a dialog for x, y and z. It records no undo entry, runs no overlap check and does not set the stale flag: src/main/java/AutoDriveEditor/GUI/Buttons/Toolbar/Alignment/EditLocationButton.java:40-84 (EditLocationButton) @ e4fe43e.

**Copy, cut and paste.**
- Copy and cut first drop nodes that cannot be deleted: control and rotation nodes, hidden nodes unless hidden selection is on, and the anchors of an uncommitted curve: src/main/java/AutoDriveEditor/Classes/SnapShot.java:62-87 (SnapShot) @ e4fe43e.
- Cut removes the nodes and records a `CutNodesChanger`: src/main/java/AutoDriveEditor/Managers/CopyPasteManager.java:24-35 (cutSelection) @ e4fe43e.
- Paste creates new nodes with ids from `size()+1`. It copies x, z, flag and marker, takes y from the heightmap, and recreates only the connections inside the selection: src/main/java/AutoDriveEditor/Classes/SnapShot.java:403-456 (createNewCopyOfNodes) @ e4fe43e.
- Paste moves the selection centre to the viewport centre. Paste-at-original-location keeps the coordinates.
- The snapshot is kept after pasting, so one copy can be pasted repeatedly.

## Where the answers meet

- `MapPanel.createConnectionBetween` is reached by questions 1 and 2: the line tools, the flip tool and both curve commits. It rejects a self-link and nodes outside `networkNodesList`, and toggles an existing link off: src/main/java/AutoDriveEditor/GUI/MapPanel.java:781-836 (createConnectionBetween) @ e4fe43e.
- `SnapShot.insertMapNode` is reached by questions 3 and 4: undo of delete, undo of cut, and redo of paste. It throws `MismatchedIdException` when ids and list positions disagree: src/main/java/AutoDriveEditor/Classes/SnapShot.java:491-521 (insertMapNode) @ e4fe43e.
- `MapNode.createNewNetworkNode` is reached by questions 1 and 2. Node creation, line commit and curve commit all assign id `size()+1` there: src/main/java/AutoDriveEditor/RoadNetwork/MapNode.java:189-195 (createNewNetworkNode) @ e4fe43e.

## Prior documents

None: no earlier research documents or ADRs exist in this repository.
