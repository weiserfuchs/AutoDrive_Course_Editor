DIGEST Q2 curves @ e4fe43e. Paths under src/main/java/AutoDriveEditor/. 12 of 15 files. Not opened: RadiusSelector, CurveInterface, ArcSplineInterface, LinearLine.

1 Structure
- Managers/CurveManager.java:59-60 (CurveManager) registers CURVE_TYPE_BEZIER -> BezierCurve, CURVE_TYPE_ARCSPLINE -> ArcSpline; map :47; registerCurveType :73. ArcSpline type string "ArcSpline (Work In Progress)" (Classes/Curves/ArcSpline.java:37).
- Classes/Curves/CurveBase.java:37 abstract; initControlPoints, generateInterpolationPoints, generateConnections (56-58); updateCurve -> generatePreview -> repaint (110-113, 167-170); curvePreviewNodesList, curvePreviewConnectionList (42-43) not in network; controlPointsMap, virtualControlPointMap (39-40).

2 Creation
- GUI/Buttons/Toolbar/Curves/BezierCurveButton.java:80-108 (mouseReleased): click start node, rubber band; same node cancels (89-92); right-click cancels (102-107); other node -> addActiveCurve(getButtonID(), CURVE_TYPE_BEZIER, CurveWidget.class, curveStartNode, selected) (96).
- GUI/Buttons/Toolbar/Experimental/ArcSplineButton.java:45-66 creates arc at click world position (49-58, 65).
- Managers/CurveManager.java:89-133 (addActiveCurve): no-op while isDraggingWidget (90); reflection constructor by param count and isInstance (103-118); none -> log, null (126).
- CurveManager.java:142-166 (createNewCurve): widget; previous same-type curve inactive, previous widget disabled (144-150); CurveInfo, active node, currentCurve, curvesList (152-155); anchor, updateAllCurves, CurvePanel lazy (158-165).

3 Math (LOAD-BEARING)
- Classes/Curves/BezierCurve.java:113-128 (generateInterpolationPoints): step 1/(N+1) float (117); i = 1..N, t = step*i (120-121); endpoints excluded; uniform in t not arc length; preview y = -1, priority curvePriority (124).
- BezierCurve.java:158-171 (calcPointsForCurve): 1 control point quadratic (181-194); 2 cubic (203-219); 3 quartic 1,4,6,4,1 (232-256); 4 quintic 1,5,10,10,5,1 (265-294); uses virtualControlPointMap.
- BezierCurve.java:72-87 (moveControlNodeBy): visible node moves by diff; virtual by diff*controlPointMoveScaler when applyScaling (CurveWidget.java:80 always true); controlPointMoveScaler default 3 (XMLConfig/EditorXML.java:87).
- BezierCurve.java:96-102 (initControlPoints) 4 control points at midpoint.
- Classes/Curves/ArcSpline.java:392-418 (generateArcPoints), numPoints = N+2 (378-382): angles normalised [0,360), end += 360 if end < start (397-403); startRad = toRadians(start-90), step (end-start)/(numPoints-1) (405-407); x = cx + r cos, z = cz + r sin (409-412); endpoints included, uniform in angle.
- ArcSpline defaults startNodeAngle -90, endNodeAngle 90, initialRadius 8, MIN_RADIUS 2 (40, 52-56); setRadius (532-535).
- ArcSpline.java:110-122 (initControlPoints): centre control node, start rotation node (cx-8), end rotation node (cx+8), radius special node (cx-4).

4 Parameters
- XMLConfig/EditorXML.java:85-91: curveSliderMax 50; curveIterationsDefault 6; curveControlPointDefault 1; controlPointMoveScaler 3; maxControlPoints 3; curveNodeDefaultPriority REGULAR; curveDefaultConnection REGULAR. Load 225-232 (CurveSliderMaximum, CurveSliderDefault, NumControlPoints, CurveNodeDefaultPriority, CurveDefaultConnection; iterations clamped to max 227); save 528-534.
- GUI/Curves/CurvePanel.java: interpolation slider 1..curveSliderMax (127-129), updates only while getValueIsAdjusting (131-137); control point slider set from curve min/max (149-161, 232-244, 282-285), disabled if equal; Bezier 1..4 (BezierCurve.java:55, 62); ArcSpline 4..4 (ArcSpline.java:82-103); priority radios (201-206); connection radios REGULAR, REVERSE, DUAL (207-215); swap direction (84); commit (169-170), cancel (173-174); dispatch 183-190.
- CurveBase.java:48, 68-69: numInterpolations = curveIterationsDefault, curveType REGULAR hard-coded, curvePriority 0, numCurrentControlPoints 1; EditorXML curveDefaultConnection/curveNodeDefaultPriority not read by curves (only resetCurvePanel CurvePanel.java:312-320). BezierCurve.java:47 setNumCurrentControlPoints(curveControlPointDefault). Setters call updateCurve (CurveBase.java:247-295).
- Swap: BezierCurve.java:352-378 swaps start/end, mirrors control points ("not fully tested"); ArcSpline.java:138-141 toggles bClockwiseConnections.

5 Editing
- CurveManager.java:589-613 forwards every mouse event to every curve; CurveBase.java:100-103 hover.
- Classes/Widgets/CurveWidget.java (extends MoveWidget): click makes curve current, sets anchor (32-47); right-click clears (52-54); drag moveNodesBy (67) -> moveControlNodeBy(..., true), updateAllCurves (77-83).
- Classes/Widgets/ArcSplineWidget.java: Move, Rotation, Radius selectors (29-35); centre drag moves all 4 (69-71, 90-98); rotation nodes (72-79); special node (80-83).
- ArcSpline.java:196-258 rotateStart/EndNode atan2, snap rotationStep if bRotationSnapEnabled else 1 degree; AffineTransform round 3 decimals (267-275); radius handle moved (212, 280-285).
- ArcSpline.java:179-183, 291-309, 321-361 radius clamp, newRadius = max(2*|handle-centre|, 2).
- ArcSpline.java:145-155 mouseReleased sets arcActive (never read).
- CurveManager.java:674-690 CurveInfo ownerName, curveType, curve, widget, isActive, activeNode.

6 Commit (LOAD-BEARING)
- GUI/Curves/CurvePanel.java:183-186 -> CurveManager.java:268-281 (commitActiveCurve): commitCurve, remove from curvesList, activate last, hide panel if none.
- Classes/Curves/BezierCurve.java:302-329 (commitCurve): suspend autosave (304); copy y if anchor y == -1 (307-312); yInterpolation = (end.y-start.y)/(previewCount-1) (314) divides by zero for N=1; createNewNetworkNode(x, start.y + yInterp*j, z, curvePriority, false) (316-321); connectNodes (335-343) start->first, chain, last->end via createConnectionBetween(getCurveType()), N+1 connections; CurveChanger(new SnapShot(networkNodes)) (324); anchor null (326).
- Classes/Curves/ArcSpline.java:449-477 (commitCurve): y from rotation nodes = 0, yInterp 0 (454-465); N+2 nodes; connectNodes (484-498) consecutive only; standalone arc.
- RoadNetwork/MapNode.java:189-195 id size()+1, append, checkNodeOverlap; flag = curve priority.
- GUI/MapPanel.java:781- createConnectionBetween rejects start == target / missing node; toggle if exists (818-); REGULAR add outgoing+incoming (799-802); REVERSE add outgoing then remove incoming entries (797, 803-806); DUAL both (807-816).
- CurveBase.java:366-403 (CurveChanger): undo removeOriginalNodes, checkAreaForNodeOverlap, restore stale; redo restoreOriginalNodes, stale true.

7 Active across tool switches (LOAD-BEARING)
- CurveManager.java:39, :57 Drawable PRIORITY_HIGH; draws all curvesList each frame (620-623); curvesList changes only via removeActiveCurve and cancelAllCurves (317-325).
- ButtonManager.java:364, 372, 391 call curveManager hooks. CurveManager.java:362-368 (onButtonDeselected) disables widget only. CurveManager.java:335-352 (onButtonSelected) reactivates curves owned by the button with activeNode.
- isActive cleared only for same curveType (145-148, 174-177): Bezier and ArcSpline previews can be active together.
- cancelActiveCurve (287-305) via CurveBase.cancelCurve (156-161); cancelAllCurves hides panel; callers not located.
- BezierCurveButton.java:53-56 onButtonDeselect -> cancelSelection resets first-click state only.

Not checked: cancelAllCurves callers, selectors, MoveWidget internals; CurveBase.isControlNode (304-311) ignores its argument.
