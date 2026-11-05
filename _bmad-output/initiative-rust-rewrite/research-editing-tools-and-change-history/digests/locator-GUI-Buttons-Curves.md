Digest for area GUI/Buttons/** and GUI/Curves/ @ e4fe43e. G/ = src/main/java/AutoDriveEditor/GUI/.

Q1 tool input and network changes
- G/Buttons/BaseButton.java:26 (BaseButton): base of all tools, implements MouseListener, ActionListener; empty hooks mouseClicked/Pressed/Released/Dragged/Moved 95-103. No key handling besides shortcut registration.
- G/Buttons/Toolbar/Nodes/MoveNodeButton.java:20 (MoveNodeButton): no mouse handlers; onButtonSelect (40) adds MoveWidget via widgetManager.addWidgetClass.
- G/Buttons/AddNodeBaseButton.java:35 (AddNodeBaseButton): mouseReleased (90) creates node, changeManager.addChangeable(new AddNodeChanger) (131); AddNodeChanger (161). Subclasses Toolbar/Nodes/AddPrimaryNodeButton.java:16, AddSecondaryNodeButton.java:16.
- G/Buttons/Toolbar/Nodes/DeleteNodeButton.java:35: mouseReleased (76), removeNode (92), removeSelectedListNodes (107), DeleteNodeChanger (139).
- G/Buttons/LinerLineBaseButton.java:26: mousePressed (83) picks start/end; linearLine.commit() (112, 132). Base for Toolbar/Nodes/NormalConnectionButton.java:24, ReverseConnectionButton.java:24, DualConnectionButton.java:24.
- G/Buttons/ConnectionSelectBaseButton.java:24: mouseMoved (49), mouseDragged (60), mouseReleased (73).
- G/Buttons/Toolbar/Nodes/FlipDirectionButton.java:32: mouseReleased (84), FlipConnectionChanger (289), removeRegularConnections (167), removeReverseConnections (188).
- G/Buttons/Toolbar/Nodes/RotationButton.java:34: handlers 102-125, RotateNodeChanger (211).
- G/Buttons/Toolbar/Nodes/SwapNodePriorityButton.java:27: mousePressed (75), NodePriorityChanger (129).
- G/Buttons/Toolbar/Experimental/RoadConnection.java:28 mousePressed (78); ScaleButton.java:16.
- Markers: G/Buttons/MarkerBaseButton.java:5; Toolbar/Markers/AddMarkerButton.java:41 MarkerAddChanger (191); DeleteMarkerButton.java:27 MarkerRemoveChanger (89); EditMarkerButton.java:39 MarkerEditChanger (205).
- 17 other buttons (ActionBar/ConfigPanel, Toolbar/Display, TemplateButton, TestButton) don't alter network, e.g. Toolbar/Display/HideNodeButton.java:17.

Q2 curves
- G/Buttons/Toolbar/Curves/BezierCurveButton.java:32: mouseReleased (80) curveManager.addActiveCurve(getButtonID(), CURVE_TYPE_BEZIER, CurveWidget.class, curveStartNode, selected) (96); mouseMoved (111), mouseDragged (125), cancelSelection (132), drawToScreen (139).
- G/Buttons/Toolbar/Experimental/ArcSplineButton.java:24: mouseReleased (45) checks getCurrentCurveWidget, getAllActiveControlNodes; createArc (63) addActiveCurve(..., CURVE_TYPE_ARCSPLINE, ArcSplineWidget.class, centrePoint).
- G/Curves/CurvePanel.java:34: BUTTON_COMMIT_CURVE (170), curveManager.commitActiveCurve() (184); updateCurvePanel 260-276, showCurvePanel 246, hideCurvePanel 253, resetCurvePanel 312.
- CurveManager, curve and widget classes outside area.

Q3 undo/redo
- Nested static XxxChanger implements ChangeManager.Changeable with undo()/redo(), pushed via changeManager.addChangeable: AddNodeChanger AddNodeBaseButton.java:161; AlignmentChanger AlignBaseButton.java:81; DeleteNodeChanger DeleteNodeButton.java:139; FlipConnectionChanger FlipDirectionButton.java:289; RotateNodeChanger RotationButton.java:211; NodePriorityChanger SwapNodePriorityButton.java:129; marker changers AddMarkerButton.java:191, DeleteMarkerButton.java:89, EditMarkerButton.java:205.
- G/Buttons/ActionBar/ChangePanel/UndoButton.java:15 actionPerformed (35) changeManager.undo(); RedoButton.java:15.
- ChangeManager outside area.

Q4 selection/alignment
- G/Buttons/ActionBar/ChangePanel/MultiSelectButton.java:19: mouseClicked (52), mousePressed (69) multiSelectList.contains; static MultiSelectManager.multiSelectList, isMultipleSelected.
- G/Buttons/AlignBaseButton.java:19: mouseClicked (36) needs multiselect + clicked node; mouseMoved (54); preview line (63); AlignmentChanger (81); ZStore (121). Subclasses Toolbar/Alignment/DepthAlignButton.java:43 AlignmentChanger(list, 0, toNode.y, 0); HorizontalAlignButton.java:60 (0,0,toNode.z); VerticalAlignButton.java:61 (toNode.x,0,0).
- Toolbar/Alignment/EditLocationButton.java:21 mouseClicked (41).
- Clipboard ActionBar/ChangePanel/CopyButton.java:53, CutButton.java:38, PasteButton.java:14 gated on isMultipleSelected.

Nothing found: keyPressed/KeyListener (only KeyStroke accelerator CopyButton.java:43); move code in MoveNodeButton; curve math/commit under Buttons/Curves.
