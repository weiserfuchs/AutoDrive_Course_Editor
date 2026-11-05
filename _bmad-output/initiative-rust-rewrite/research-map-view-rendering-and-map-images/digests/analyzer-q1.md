DIGEST Q1 map image @ e4fe43e. Paths under src/main/java/AutoDriveEditor/. 14 of 15 files.

SUMMARY: static fields in MapImage; two copies pdaImage (coordinate reference) and mapPanelImage (drawn); separate heightmap. Ways in: PNG on disk/downloaded at config load, DDS from user file, manual PNG. Zip import only previews, writes PNGs to mapImages.

1 HELD STATE
- [load-bearing] GUI/MapImage.java:44-55 (MapImage): pdaImage, mapPanelImage, heightMapImage, heightMapImage16bit, heightMapDataBuffer, heightMapPixelData, heightmapType (BYTE/USHORT_GREY), heightMapScale.
- Classes/Util_Classes/ImageUtils.java:16-23 (getNewBufferImage): gc.createCompatibleImage(w,h,transparency), setAccelerationPriority(1); format from screen GraphicsConfiguration.
- MapImage.EditorImages (:454-508) nested holder, no use outside MapImage.java (repo grep).
- TODO :413-415 mapPanelImage to be eliminated.

2 LOCATING (PNG, config load)
- XMLConfig/GameXML.java:77-78 (loadGameConfig) loadMapImage then loadHeightMap on caller thread.
- GUI/MapImage.java:65 (loadMapImage) findImageLocationFor(mapName,"").
- [load-bearing] GUI/MapImage.java:229-291 (findImageLocationFor): probe new SimpleImageInfo(new File); order 1 <cur>/mapImages/<name>/<name><ext>.png (242-246); 2 legacy <cur>/mapImages/<name><ext>.png (255-259); 3 <cur>/src/main/resources/mapImages/<name><ext>.png (268), cur null -> ./src/mapImages/<name>.png drops ext (270); 4 <cur or ./><name><ext>.png (280). name = removePathFromString (237).
- GUI/MapImage.java:293-321: legacy move Files.move from relative "mapImages/<name><ext>.png" to "mapImages/<name>/..." (relative to working dir).
- Download if not found and bUseOnlineMapImages (324-325); cur null -> null (362-365); URL https://raw.githubusercontent.com/KillBait/AutoDrive_MapImages/master/mapImages/<name>/<name><ext>.png (328); save <cur>/mapImages/<name>/<name><ext>.png (340); Classes/Util_Classes/FileUtils.java:65-100 (copyURLToFile) url.openStream, 4096-byte copy, mkdirs, IOException -> null; no timeout, no size check; synchronous on caller thread.

3 DECODING (PNG)
- GUI/MapImage.java:67-82 (loadMapImage): SimpleImageInfo dims; pdaImage = getNewBufferImage(w,h,OPAQUE) (:73); ImageIO.read -> drawn into pdaImage; setMapImageLabel(LOADED); setMapPanelImage(pdaImage,false); IOException silently ignored.
- [load-bearing] GUI/MapImage.java:417-452 (setMapPanelImage): 2048 rule :420; exact -> new mapPanelImage drawn (440-446); other size scale=false -> modal error FS19/FS22 hint, return false (428-438), pdaImage left at wrong size; scale=true -> getScaledInstance(2048,2048) into existing mapPanelImage (421-426), pdaImage untouched, NPE if null.
- GUI/MapImage.java:390-406 (useDefaultMapImage) /mapImages/Blank.png scaled into 2048 pdaImage, setMapPanelImage.
- GUI/MapImage.java:83-93 not-found branch.
- GUI/MapImage.java:98-120 (setPDAImage): buffer at input size, input drawn scaled to 2048.

4 DDS PATH
- Managers/ImportManager.java:31-36 importDDSForFS19/FS22 from menus.
- [load-bearing] Managers/ImportManager.java:38-92 (createBufferImageFromDDS): EDT builds progress JFrame (40-52); thread "DDS Decode" (58, 90-91) DDSReader.read(fis, ARGB, 0, progressBar); OOM -> dialog (63-66); IOException (67-69); finally: invokeLater dispose; createBufferImage, setPDAImage, setMapPanelImage, forceMapImageRedraw, setIsEditorUsingImportedImage, saveImageEnabled, label, showInTextArea all on decode thread (not EDT); failure dialog from decode thread (84-87); null image still success=true (62) -> NPE.
- Managers/ImportManager.java:94-125 (createBufferImage): 2048 OPAQUE, bilinear; FS19 whole scaled (116-117); FS22/FS25 centre half getSubimage (118-121).
- Import/DDSReader.java:104-134 (read): 128-byte header; static DDSReader.image (:43) TYPE_INT_RGB at header size, reused if same (126-135); read return unchecked; null for unknown type (139); mipmap skip (143-167); decode switch 169+: DXT1 decodeDXT1 (299), decodeDXT2 (335), DXT3 (339), DXT4 (363), DXT5 (367), readXxx; setRGB per pixel; returns shared static image.

5 ZIP PATH
- GUI/Menus/ImportMenu/ImportFromZipMenu.java:110-157 (actionPerformed) EDT: openZipFile, modDesc.xml, descVersion, map/@configFilename, map/@imageFilename, .png->.dds (140), dem.png (142), setupPreview.
- [load-bearing] ImportFromZipMenu.java:188-244 (setupPreview): progress frame; DDS to temp file via getFileInputStreamFromZip; thread "DDS Decoder" (209, 238) -> decodedDDSImage (211), temp deleted (213); finally progressFrame.dispose() directly (228); getMapImagePreview (246-263) 256x256; getHeightMapPreview (265-288) ImageIO.read dem.png (271); openPreviewWindow.
- Fields (46-50) decodedDDSImage, decodedHeightmapImage, previews 256x256.
- No setPDAImage/setMapPanelImage; confirm (423) -> exportImages.
- ImportFromZipMenu.java:443-455 (exportImages): name sanitized; createMapImagesFolder; <name>.png via Managers/ExportManager.java:39-54 (exportMapImageToDisk) -> createBufferImage (2048 cropped/scaled, ImageIO.write :54); <name>_HeightMap.png. Picked up at next config load.

6 ZipUtils (Classes/Util_Classes/ZipUtils.java): openZipFile (29-44); getFileFromZip (55-87) rejects absolute/.. (58-62), equals match, single read; getFileStreamFromZip (94-121) "Untested"; [load-bearing] getFileInputStreamFromZip (133-168) temp file AutoDriveEditor_*.tmp, caller deletes; findFileInZip (177+) not read.

7 MANUAL LOAD: GUI/Menus/ImportMenu/LoadMapImageMenu.java:52-69 ImageIO.read, setMapPanelImage(img,true), only mapPanelImage. GUI/Buttons/ActionBar/ConfigPanel/MapInfoButton.java:46-53 toggles heightmap/pda view.

8 HEIGHTMAP
- GUI/MapImage.java:157-226 (loadHeightMap): terrain.heightmap.png beside config (166-167) else findImageLocationFor(mapName,"_HeightMap"); non-USHORT_GRAY -> compatible OPAQUE copy, BYTE; USHORT_GRAY -> copy plus heightMapImage16bit, buffer, pixel data, USHORT_GREY (189-205); IOException ignored (213).
- calculateHeightMapScaling (374-388): averages (heightmapY − node.y)/node.y, int, min 1; node.y == 0 / empty list unhandled.
- manualLoadHeightMap (122-155). Not found (214-224).
- [load-bearing] GUI/MapPanel.java:707-729 (getYValueFromHeightMap) uses 8-bit RGB copy even when 16-bit source exists; ((R<<8)+G)/256 / heightMapScale; -1 if none. Callers LinearLine :228, :245, SnapShot :428, AddNodeBaseButton :93, FixNodesHeightMenu :31, calculateHeightMapScaling. MapPanel :844-869 info display uses 16-bit data.
- EditorMenu:261 enables export-heightmap.

9 DISPLAY READERS: GUI/MapPanel.java:572-590 draws mapPanelImage; coordinates use pdaImage size (MapPanel 576-584, 597-598, 638-639, 734-742, 764-765, 1038-1042; OutOfBoundsFixMenu; MoveNodeToCentreMenu; MultiSelectManager; MouseListener:102; ConnectionDrawThread:85; NodeDrawThread:80). Sizes must match; manual load does not enforce.

10 COPIES: pdaImage (~16 MiB at 2048 INT_RGB) + mapPanelImage (~16 MiB) ≈ 32 MiB (estimate); heightmap up to two copies; DDS path retains static DDSReader.image (can exceed 2048) for process lifetime; zip previews live as long as menu object.

11 THREADS: config load on caller thread (download blocks, no timeout); DDS import: decode + image/UI updates on "DDS Decode" off EDT; zip: decode/previews on "DDS Decoder", exportImages on EDT; rendering reads statics, not volatile, no locks around reassignment.

NOT FOUND: no caching beyond statics; editor never reads game zip/DDS at load time. Not read: SimpleImageInfo, findFileInZip body, openPreviewWindow, exportHeightMapImageToDisk, createMapImagesFolder, forceMapImageRedraw, getCurrentLocation, DXT inner loops.
