DIGEST Q4 (all @ e4fe43e). Two independent version notions: config `<version>` for AutoDrive configs, modDesc `descVersion` for mod zips.

1. Config <version> -> GameXML.configVersion
- Load-bearing: src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:64 (configVersion) public static FSUtils.GameVersion, no initializer, assigned only at 190, 193, 196.
- Load-bearing: GameXML.java:172-200 (loadGameXMLFile): 172 root not AutoDrive -> log, return null. 177-183 markerID present -> legacy pre-v1.1.0.6, warning dialog, canEditConfig=false, configVersion NOT assigned. 185-199 root <version> via new Semver(version): major 1 FS19, 2 FS22, 3 FS25, canEditConfig=true; other major leaves configVersion unchanged (null on first load or previous value). new Semver throws on missing/malformed version, no catch in block.
- src/main/java/AutoDriveEditor/RoadNetwork/RoadMap.java:23 (fsVersion) declared, never used elsewhere in src.

2. modDesc descVersion -> local fsVersion in zip import
- Load-bearing: src/main/java/AutoDriveEditor/GUI/Menus/ImportMenu/ImportFromZipMenu.java:125-127 (actionPerformed) parses descVersion from modDesc, calls FSUtils.getFSVersion.
- Load-bearing: src/main/java/AutoDriveEditor/Classes/Util_Classes/FSUtils.java:31-44 (getFSVersion): 21-25 NOT_SUPPORTED (FS15); 31-37 NOT_SUPPORTED (FS17); 40-53 FS19; 60-80 FS22; >=90 FS25; else (incl. 54-59, 81-89) NOT_SUPPORTED.
- FSUtils.java:26-28 (isSupported) descVersion >= 40; no callers.
- FSUtils.java:17-24 (getGameVersion) matches description string to enum; used at ImportFromZipMenu.java:419.
- Version passed via setupPreview (ImportFromZipMenu.java:145, 188) to getMapImagePreview (:230), openPreviewWindow (:232).
- ImportFromZipMenu.java:336-359 shows description, combo of GameVersion.values() minus NOT_SUPPORTED, preselects detected.
- ImportFromZipMenu.java:419 selectedVersion = combo if override ticked else detected; to exportImages (:443-455) -> exportMapImageToDisk(..., gameVersion) at :452.
- Never writes GameXML.configVersion.

3. Branches
Config load (configVersion):
- GameXML.java:346-352 (loadGameXMLFile): FS22/FS25 flag 2 or 4 -> NODE_FLAG_REGULAR; FS19 unchanged.
- src/main/java/AutoDriveEditor/XMLConfig/RoutesXML.java:263-269: same reset in routes loader.
- GameXML.java:764-768 (loadVehiclesXMLParking): FS19 XPath //vehicle[./FS19_AutoDrive/AutoDriveVehicleData[@parkDestination]], else //vehicle[./AutoDrive[@parkDestination]].
- GameXML.java:775-780: parkDestination from AutoDriveVehicleData (FS19) or AutoDrive.
- GameXML.java:782-784: vehicle id attr uniqueId FS25 else id.
- GameXML.java:838-842 (saveVehiclesXMLParking): same XPath split to remove parkDestination.
- GameXML.java:853-857: //vehicle[@uniqueId=..] FS25 else @id.
- GameXML.java:861-865: parkDestination set on AutoDriveVehicleData (FS19) or AutoDrive.
- src/main/java/AutoDriveEditor/GUI/MapImage.java:429-435 (setMapPanelImage) and :483-489 (EditorImages.setMapPanelImage): image not 2048x2048 and scaling off -> FS19 hint (dialog_mapimage_incorrect_size_fs19, pda_map_H.dds) or FS22 hint (overview.dds); FS25/null generic. Strings src/main/resources/locale/AutoDriveEditor_en_US.properties:32-33.
- GUI/Menus/ImportMenu/ExportHeightMapMenu.java:93 passes configVersion to exportMapImageToDisk (context not read).
Image conversion:
- Load-bearing: src/main/java/AutoDriveEditor/Managers/ImportManager.java:94-125 (createBufferImage) 2048x2048: FS19 whole image scaled; FS22/FS25 crop centre half (getSubimage w/4, h/4, w/2, h/2) then scale; NOT_SUPPORTED/null blank opaque image.
- ImportManager.java:31-38: importDDSForFS19/importDDSForFS22 hard-code version; menus ImportFS19DDSMenu.java:51, ImportFS22DDSMenu.java:49.
- No FS25 DDS menu item; FS25 images only via zip import.
- src/main/java/AutoDriveEditor/Managers/ExportManager.java:39-54 (exportMapImageToDisk) createBufferImage(image, fsVersion), writes PNG.
- ImportFromZipMenu.java:246-257 (getMapImagePreview): FS19 full image to 256x256; other versions incl. NOT_SUPPORTED crop centre half.
- src/main/java/AutoDriveEditor/GUI/Menus/EditorMenu.java:121-122, 245-246 add/enable FS19/FS22 DDS items; 85-86 commented duplicates.

Not found: no readers of RoadMap.fsVersion; no callers of isSupported; no rejection of NOT_SUPPORTED; nothing resets configVersion between loads besides legacy branch. Not read: rest of GameXML, ExportHeightMapMenu context, VersionManager (Semver :65 is editor version).
