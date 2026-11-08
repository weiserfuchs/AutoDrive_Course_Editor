---
type: epic
title: "Users manage markers, groups and parking"
parent: initiative-rust-rewrite
covers: ["R10"]
sources: []
companions: []
after: []
refined: true
risk: medium
---

# Users manage markers, groups and parking

## Description

Users add, edit and delete markers, and rename and delete groups, each in one undoable step. Vehicles parked at a marker in the savegame's `vehicles.xml` stay attached to it, show on the map, and their marker references follow marker changes on save.

## Outcome

A course author keeps the FS25 fixture's destinations tidy (names, groups, removals) without breaking the vehicles in the savegame that park at or drive to them.

## Requirements

- R1 (parent R10): Users add a marker to a node (name and group), edit its name and group, and delete it; each undoes in one step.
- R2 (parent R10): Users rename a group (all its markers at once) and delete a group (its markers move to "All"), each in one undoable step.
- R3 (parent R10): The vehicles parked at a marker are loaded from `vehicles.xml` beside the config, survive every edit and undo, and show as the parking icon on the marker's node.
- R4 (parent R10, R2): On Save, `vehicles.xml` keeps every byte except its marker references (`parkDestination`, `firstMarker`, `secondMarker`), which follow marker order; a reference to a deleted marker becomes `1`, a `parkDestination` to one is removed.

## Constraints

- C3 (parent): the FS25 fixtures `AutoDrive_config.xml` and `vehicles.xml` are the verification reference; neither is ever overwritten.
- C4 (parent): known Java defects are fixed rather than ported (AD-13): the whole-file reformat and stale references of `vehicles.xml`, duplicate marker names.
- C5 (parent): the architecture spine binds, in particular AD-4, AD-6 and AD-17.

## Done when

- Markers are added, edited and deleted, and groups renamed and deleted, on the fixture, each undoing in one step.
- Opening the fixture with its `vehicles.xml`, editing markers and saving writes a `vehicles.xml` that differs from the original only in the marker reference attributes, each pointing at the same marker as before (or `1` / removed for a deleted one).
- The golden tests cover every command this epic adds and the `vehicles.xml` round trip.

## Boundaries

- Out: the route-manager group list (epic-route-manager); assigning vehicles to markers (the game does that); a marker list or search UI (epic-settings-ui-shell).

## References

- parent — _bmad-output/initiative-rust-rewrite/initiative-rust-rewrite.md, section Requirements
- architecture — _bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md, AD-4, AD-6, AD-13, AD-17
- research — _bmad-output/initiative-rust-rewrite/research-road-network-data-model-and-config-forma/research-road-network-data-model-and-config-forma.md, §1–§3 (markers, groups, vehicles.xml)

## Notes

- Waits on epic 3 because: needs the Tool trait and command pattern.
- Decision (2026-11-08): refined from the initiative's lines and the Java source (`AddMarkerButton`, `EditMarkerButton`, `DeleteMarkerButton`, `GameXML` vehicles code); no UX or architecture run.
- Decision (2026-11-08): marker tools as in Java: add (W), edit (E), delete (Shift+D); a toolbar click picks the tool, which acts on a clicked node; the shortcut acts at once on the node under the pointer, as Java's shortcut does. The group field is an editable list of the existing groups, sorted; "None" means "All".
- Decision (2026-11-08): a marker name already used is refused in the add and edit dialogs (AD-13).
- Decision (2026-11-08): groups get a small window listing each group and its marker count, with rename (`RenameGroup`, AD-17; renaming onto an existing group merges) and delete (markers move to "All"); new, Java has neither.
- Decision (2026-11-08): `vehicles.xml` beside the config is read on load (FS19 `FS19_AutoDrive/AutoDriveVehicleData` and `id`, FS22/FS25 `AutoDrive`, FS25 `uniqueId`) and, on every real Save, patched in place: only `parkDestination`, `firstMarker` and `secondMarker` values change, following marker order; a missing file is fine (AD-13, AD-17).
- Decision (2026-11-08): the user's savegame `vehicles.xml` becomes the second repo fixture (`-text`, never overwritten).
- Decision (2026-11-08): build order 1 → 2 → 3 → 4 → 5, one lane: the stories share the marker model, the marker tools and the golden tests.
