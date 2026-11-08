---
title: 'A connection check removes dangling, duplicate and self entries'
type: 'feature'
ticket: '4'
created: '2026-11-08'
status: 'planned'
baseline_revision: 'c1cefd1'
route: 'full'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: []
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** A config edited by other tools can carry broken connection entries the game and the editor read inconsistently. Java has no check for them. Three kinds are agreed (epic decision):
1. **Dangling incoming:** node B lists A in `incoming`, but A does not list B in `outgoing`.
2. **Duplicates:** the same id twice in one node's `outgoing` or `incoming`.
3. **Self-links:** a node listing itself in `outgoing` or `incoming`.

A reverse link (A lists B in `outgoing`, B does not list A in `incoming`) is valid and never touched.

**Approach:**
- First confirm what the loader keeps: if it already drops duplicates or self-links on load, the check covers only what can still exist, and the notes say so (the round trip must stay byte-identical for the fixture).
- Pure check in ad-core: `connection_defects(net) -> Defects { dangling: Vec<(node, value)>, duplicates: Vec<(node, side, value)>, self_links: Vec<(node, side)> }`.
- **Scan menu "Check connections…"** (any open document): a modal lists the counts per kind; with defects and an editable document it offers "Fix", else only "Close". No defects: a notice "Connections are consistent" and no modal.
- **Fix** removes exactly the defective entries (for duplicates, the extra occurrences only, keeping the first) in one transaction, through a small ad-core command if no existing one fits (`RemoveLinkEntries` with exact revert, per AD-4 naming). The touched nodes become the selection; one undo restores every entry at its position.

## Boundaries & Constraints

**Always:** real one-way, dual and reverse links stay exactly as they are; read-only shows the report without Fix.

**Never:** no new links; no node deletion; no change to how the loader parses.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Dangling | B.in = [A], A.out = [] | reported; Fix removes A from B.in | none |
| Reverse | A.out = [B], B.in = [] | not reported | none |
| Duplicate | A.out = [B, C, B], B.in = [A] | reported once; Fix leaves A.out = [B, C] | none |
| Self | A.out = [A, B] | reported; Fix leaves A.out = [B] | none |
| Clean | the fixture | notice "Connections are consistent" (or the real counts, recorded) | none |
| Undo | Fix, Ctrl+Z | every entry back at its index (exact snapshot) | none |
| Read-only | legacy config with defects | report shown, no Fix | none |
| Save | fixed network saved and reloaded | no defects after reload | none |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/scan.rs` -- `connection_defects`; `crates/ad-core/src/command.rs` (+ `network.rs` helpers) -- the removal command with exact revert; tests (use `round_trip`).
- `crates/ad-io/src/config_xml.rs` -- read only: what the loader keeps; a test loading a config with each defect.
- `crates/ad-app/src/document.rs`, `main.rs`, `ui.rs` -- menu item, report modal, Fix through `apply_selecting`, notices.
- `crates/ad-io/tests/golden_edited.rs` -- a fixed network saves and reloads clean.

## Tasks & Acceptance

**Execution:**
- [x] loader behaviour confirmed and recorded
- [x] check and removal command -- tests for Dangling, Reverse, Duplicate, Self, Undo
- [x] app -- tests for Clean, Read-only, selection; golden Save case

**Acceptance Criteria:**
- Given a copy of the fixture with one dangling incoming entry, one duplicate and one self-link planted in the XML, when the user runs "Check connections…", then the report shows one of each, Fix removes them, the saved file reloads clean, and one Ctrl+Z restores them.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip`, `golden_edited` and `vehicles_roundtrip`
- `cargo test -p ad-app --release -- --ignored frame_time --nocapture` -- expected: every case under 16 ms

## Implementation Notes

- **Loader (read only, unchanged):** `parse_adjacency` keeps every id of a `<out>`/`<incoming>` group in file order, duplicates and the node's own id included, and `Network::from_loaded` maps them verbatim (one edge-index entry per outgoing occurrence, so duplicates and self-links are in the edge index too). An entry whose partner does not list back (dangling incoming) is kept as well. Only an id naming no node of the file is an error (`UnknownNode`). So all three kinds can exist after a load and the check covers all of them; `config_xml` test `dangling_duplicate_and_self_entries_are_kept_as_read_and_roundtrip` loads one of each and saves it byte-identically.
- **Fixture counts:** the fixture (35652 nodes) has 0 dangling, 0 duplicate and 0 self entries, so the "Clean" row holds as written: the check shows the notice "Connections are consistent" and no modal. The check costs about 0.9 ms on it (release).
- **ad-core:** `scan::connection_defects(net) -> Defects { dangling: Vec<(b, a)>, duplicates: Vec<(node, Side, value)>, self_links: Vec<(node, Side)> }`, in network order. One walker classifies each entry once: the node's own id is a self-link (every occurrence removed, reported once per list), a repeat of another id a duplicate (reported once per value at its second occurrence, repeats removed, the first kept), and the first occurrence of an incoming id whose node does not list back, or no longer exists, is dangling. So `b.in = [a, a]` with `a.out` lacking `b` counts as one dangling and one duplicate, and both entries go. Outgoing entries are never dangling (a reverse link).
- **Command:** new `Command::RemoveLinkEntries { entries: Vec<(NodeId, Side, usize)> }` over a new `Network::remove_entries`, which validates every entry first (missing node or index out of range: error, nothing changed), removes highest index first per list and records each removal in a `LinkDelta`; revert is the existing `revert_links`, which replays backwards and puts every entry back at its index, edge-index entries included. `scan::connection_fix_plan` builds one such command (empty transaction when clean); `scan::relinked_nodes(tx)` gives the touched nodes. `placed_nodes` treats the command as moving no node (no overlap remark needed).
- **App:** Scan menu "Check connections…" (below a separator, enabled with any open document) calls `App::check_connections` -> `Document::check_connections`: with defects, `connection_report` holds them and `ui::connection_report` shows the three counts with "Fix" (editable only) and "Close", else "The config is read-only." and only "Close"; without defects only the notice line (replaced, not repeated, on a rerun). `finish_connections(true)` re-checks `editable()` and runs `Document::fix_connections` through `apply_selecting` (touched nodes selected, undo restores the earlier selection). The report counts for `dialog_shown` and is dropped when a new document loads. No notice after a fix (the spec asks for none).
- **Tests:** ad-core `a_dangling_incoming_entry_is_reported_and_removed` (Dangling, also an entry naming a deleted node), `reverse_one_way_and_dual_links_are_not_reported` (Reverse), `a_duplicate_is_reported_once_and_only_its_repeats_are_removed` (Duplicate), `a_self_link_is_reported_and_removed` (Self), `one_undo_restores_every_removed_entry_at_its_index` (Undo), `removing_entries_that_are_not_there_fails_and_changes_nothing`; ad-io `dangling_duplicate_and_self_entries_are_kept_as_read_and_roundtrip` (loader); ad-app `the_fixture_s_connections_are_consistent` (Clean), `a_read_only_document_shows_the_connection_report_without_a_fix` (Read-only), `the_connection_fix_selects_the_relinked_nodes_and_one_undo_restores_every_entry` (selection, Undo); golden `fixed_connection_defects_save_as_the_fixture_and_reload_clean` (Save: the fixture with the three planted entries, fixed and saved, is the fixture byte for byte and reloads clean; one undo restores the planted lists).
- **Hand-test file:** `target/handtest/AutoDrive_config_links.xml` (not committed), the fixture plus 6 bytes: `<incoming>` of node id 6 `5` -> `5,8` (dangling: node 8's out is `9`), `<out>` of node id 3 `4` -> `4,4` (duplicate), `<out>` of node id 7 `8` -> `8,7` (self-link). It loads editable, saves back byte-identically, and the check reports dangling (6, 8), duplicate (3, out, 4), self-link (7, out). After Fix and save it equals the fixture.
- **Verification:** `cargo fmt --all --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test --workspace` all pass (ad-app 408 passed, 3 ignored; ad-core 93; ad-io 77; golden_edited 12, golden_roundtrip 1, vehicles_roundtrip 6); `frame_time` (release) every case ok, the slowest best-round max 7.5 ms (drag, min zoom, whole network selected).

## Plan Change Log

## Review Triage Log
