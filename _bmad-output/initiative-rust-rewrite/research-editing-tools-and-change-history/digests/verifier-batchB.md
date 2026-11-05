Verifier batch B (refs 15-28) @ e4fe43e: 12 verified, 2 disputed (17, 24).
15 verified. 16 verified (polynomial bodies not read). 17 disputed: virtual moves diff*scaler only when applyScaling, else diff; visible always plain diff. 18 verified (caller line 380). 19 verified (also maxControlPoints=3). 20 verified (curvePriority Java default 0, line 45). 21 verified (float division -> Infinity/NaN). 22 verified (createRotationNode y=0, MapNode.java:203-204). 23 verified.
24 disputed: onButtonDeselected (362-368) disables current curve widget, owner check commented out; onButtonSelected (335-352) restores active curve/anchor/selector, setWidgetEnabled(true) commented out (346).
25 verified. 26 verified. 27 verified (sort via NodeBackup.compareTo 632-634). 28 verified (502-508, 513-516).
