Verifier batch B (refs 27-52) @ e4fe43e: 25 verified, 1 disputed (49).
27 verified MapPanel.java:625-645 pow(1.6,|dir|), clamp.
28 verified MapPanel.java:731-748.
29 verified MapPanel.java:754-771.
30 verified MouseListener.java:99-106.
31 verified MapPanel.java:600 inside roadMap/pdaImage guard.
32 verified :656. 33 verified :877 under bDebugShowHeightMapInfo. 34 verified :884 cursorPosNode != hoveredNode. 35 verified :1034. 36 verified :1046 inside pdaImage guard. 37 verified :177 anonymous ComponentAdapter.
38 verified MapImage.java:86. 39 verified GameXML.java:86. 40 verified RoutesXML.java:108 (enclosing method name unconfirmed). 41 verified ImportManager.java:79. 42 verified MapInfoButton.java:59 unconditional. 43 verified MapImage.java:149.
44 verified CopyPasteManager 6 calls 34, 54, 88, 96, 120, 132.
45 verified MultiSelectManager 83, 244, 318, 334; line 83 is in mouseClicked.
46 verified ScanManager 157, 257.
47 verified EditorXML 768, 782, 812, 827.
48 verified GridDisplayButton.java:57; file has 4 repaint calls 57, 79, 138, 173.
49 disputed BezierCurveButton has 4 repaint calls (100, 120, 128, 135), not 5.
50 verified ColoursTab.java:74; also 172; listPanel.repaint() calls not map.
51 verified CurvePanel 9 calls 136, 159, 217, 250, 257, 264, 273, 309, 320.
52 verified ShowNodeID.java:35.
