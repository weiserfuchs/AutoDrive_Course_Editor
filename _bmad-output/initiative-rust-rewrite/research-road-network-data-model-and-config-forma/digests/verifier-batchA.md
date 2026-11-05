Verifier batch A (refs 1-12) @ e4fe43e: 11 verified, 1 disputed.
1 verified GameXML.java:68-108 loadGameConfig; loadGameXMLFile at 72, post-load 74-100.
2 disputed GameXML.java:172-200 loadGameXMLFile: root check 172-175; markerID 177-183 warning + canEditConfig=false; Semver 185-197. "opened view-only" not shown here: code only sets canEditConfig=false, enforcement elsewhere; configVersion not set in markerID branch.
3 verified 248-333; EntryTotalException x 276, y 285, z 295, out 305, incoming 315; flags optional 320-333.
4 verified 256-268; line 268 overwrites lastGoodID with ids.length.
5 verified 337-339; id i+1, ids[i] parse commented out.
6 verified 354.
7 verified 360-395; skips -1 and ids above lastGoodID; Connection unless UNKNOWN 387-395.
8 verified 418-459.
9 verified 464-476; RoadMap.java:27-32; MapName written at 475.
10 verified 485-491.
11 verified 503-608.
12 verified 511-521; mismatch only LOG.info("Found") (519).
