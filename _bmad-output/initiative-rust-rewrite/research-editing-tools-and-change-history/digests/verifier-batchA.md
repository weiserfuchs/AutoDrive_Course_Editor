Verifier batch A (refs 1-14) @ e4fe43e: 12 verified, 2 disputed (2, 8).
1 verified MapPanel.java:135-138.
2 disputed MouseListener.java:33-97: order holds for clicked/pressed/released/dragged/moved; mouseEntered/Exited (68-75) buttonManager only; mouseWheelMoved (100-106) skips multiSelectManager, no MapPanel hook, zooms first.
3 verified ButtonManager.java:510-517, all eight events. 4 verified 360-381. 5 verified ShortcutManager.java:169-185 (callers pass MapPanel, MapPanel.java:168).
6 verified every key ShortcutManager.java:87-116 (constructor); commented duplicate H at 109.
7 verified AddNodeBaseButton.java:127-138.
8 disputed: clamp ±1024*mapScale (153-156), rounding, grid snap (185) in MoveWidget.java:147-186 (calcMoveOffset); release 72-79 records MoveWidgetChanger; X step 3 decimals (166), Y step 5 decimals (176); Y bounds check at 177 uses minBoundX/maxBoundX.
9 verified DeleteNodeButton.java:92-105 (canDelete guard). 10 verified LinerLineBaseButton.java:83-143; right-click cancel 145-149. 11 verified LinearLine.java:219-296. 12 verified MapPanel.java:781-836; warning comment 775-779.
13 verified FlipDirectionButton.java:84-152; DUAL skip in filterConnections line 159.
14 verified CurveManager.java:89-133.
