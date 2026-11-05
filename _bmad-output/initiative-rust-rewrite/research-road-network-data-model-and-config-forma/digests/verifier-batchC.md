Verifier batch C (refs 25-36) @ e4fe43e: 11 verified, 1 disputed.
25 verified ScanManager.java:58-85; local KDTree3D from networkNodesList (58-62); withinDistance(mapNode, 0.1) line 74; never stored.
26 verified ScanManager.java:93-137; linear loop line 110; TODO 134-137 "horribly inefficient" above checkNodeOverlap (139).
27 verified GameXML.java:64.
28 verified FSUtils.java:31-44; also 21-25, 31-37 explicit NOT_SUPPORTED.
29 verified ImportFromZipMenu.java:125-127.
30 disputed ImportFromZipMenu.java:336-359: overrideVersionComboBox from GameVersion minus NOT_SUPPORTED, starts disabled, preselected detected (341-359); enabled by override checkbox per comment at 349, checkbox outside range; where chosen value applied not opened.
31 verified GameXML.java:346-352.
32 verified RoutesXML.java:263-269.
33 verified ImportManager.java:94-125; OPAQUE 2048 (104); FS19 117; FS22/FS25 118-121; other versions undrawn blank image (inference, no colour set).
34 verified pom.xml:84-89; no src/test.
35 verified TestButton.java:43-62 (TestButton); line 49 inside actionPerformed; logs calcScreenPos distances, asserts nothing.
36 verified AutoDrive_config.xml:2 (AutoDrive); declaration line 1, root line 2; no other sample/fixture (other xml: history.xml, version.xml, pom.xml, log4j2.xml); no zip/dds/test dir; file has version 3.0.1.4, map NF_Marsch_4fach; not referenced by src/pom/readme.
