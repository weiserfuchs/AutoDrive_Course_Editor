Verifier batch B (refs 13-24) @ e4fe43e: 11 verified, 1 disputed.
13 verified GameXML.java:529 saveGameXMLFile
14 verified GameXML.java:612-669 saveGameXMLFile: removal loop 612-619 up to totalMarkers+100; mapmarker created 626-630; mmK appended 639-669.
15 verified GameXML.java:673-677 saveGameXMLFile
16 verified GameXML.java:681-689 saveGameXMLFile
17 verified GameXML.java:693-705 saveGameXMLFile; repointed only when !isAutoSave && !isBackup.
18 verified GameXML.java:719-721 saveGameXMLFile
19 verified AutoSave.java:28-43 startAutoSaveThread; newSingleThreadScheduledExecutor, scheduleAtFixedRate minutes.
20 verified RoadMap.java:22 RoadMap; constructor line 28 replaces list.
21 verified GameXML.java:360-384 loadGameXMLFile; separate loops, no cross-check.
22 verified RoadMap.java:85-101 getConnection; order as claimed; SUBPRIO/REGULAR require isRegular plus start flag; 60-101 also covers helpers.
23 verified RoadMap.java:45-54 removeMapNode; line 50 `if (mapNode.id > toDelete.id) mapNode.id--`.
24 disputed MapPanel.java:659-679 getNodeAtWorldPosition and 681-~700 getNodeAtScreenPosition: both iterate whole list and break on first node in box (world: nodeSizeWorld; screen: worldPosToScreenPos + nodeSizeScaledHalf). Cited 659-688 cuts off second method. World version returns rotation control node first when RotateButton active and pointer in its box (671-676).
