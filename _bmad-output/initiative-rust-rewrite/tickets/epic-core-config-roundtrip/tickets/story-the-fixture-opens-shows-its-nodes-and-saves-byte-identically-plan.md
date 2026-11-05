---
title: 'The fixture opens, shows its nodes and saves byte-identically'
type: 'feature'
ticket: '2'
created: '2026-11-05'
status: 'built'
baseline_revision: 'ccf8e09f3c842e44e58b958f1fb0c03e531ea934'
route: 'full'
route_source: 'auto'
risk: 'high'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: ['{project-root}/_bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The Rust editor cannot read or write an AutoDrive config yet. Every later story depends on a lossless load/save path, and the byte-identical round trip (initiative R1) is the riskiest part of the rewrite.

**Approach:** This is the tracer.
- `ad-core` gets a minimal `Network`: stable ids, file order, adjacency lists and markers in file order.
- `ad-io` parses `<waypoints>` and `<mapmarker>` with quick-xml, keeps the original bytes, and on save splices regenerated blocks into them (AD-6).
- `ad-app` opens and saves through file dialogs and draws nodes as dots and connections as lines.

## Boundaries & Constraints

**Always:**
- Saving an unedited document writes exactly the original bytes of `AutoDrive_config.xml`, with the BOM and CRLF intact. This is proven by regenerating the two blocks, not by copying the file.
- Regenerated blocks reuse the source's newline sequence and indentation.
- Numbers are written in the game's format:
  - coordinates with exactly 3 decimals
  - marker ids as `N.000000`
  - lists joined with `,`, and `;` between nodes in `out`/`incoming`
  - `-1` for an empty list
- Child elements of `<waypoints>` are written in the order the source has them.
- Each node's `outgoing`/`incoming` entries keep their file order.
- Markers keep their file order (AD-17 as amended).
- Marker text escapes `&`, `<` and `>`.
- Nodes are addressed by `NodeId`. File ids exist only inside `ad-io` (AD-2).
- `ad-core` gains no dependency, and the boundary test stays green.
- `ad-io` returns typed errors (`thiserror`), never panics on input, and keeps the clippy deny lints (AD-12).

**Never:**
- History, commands, the R-tree, connection-kind derivation, version handling, `vehicles.xml`, saving after edits, temp-file plus rename, or background jobs. These belong to stories 3–7.
- A map image or a real map view (epic 2).
- Reformatting anything outside the two regenerated blocks.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Golden round trip | FS25 fixture, load and save unchanged | Output bytes equal the input bytes | No error expected |
| Small LF file | Synthetic config with LF endings, no BOM, 2 nodes, 1 marker | Unchanged save gives identical bytes | No error expected |
| Escaped name | Marker name `A &amp; B` | Loaded as `A & B`, written back as `A &amp; B` | No error expected |
| Empty adjacency | Node with `out` `-1` | Empty outgoing list; written back as `-1` | No error expected |
| Count mismatch | `<x>` has fewer values than `<id>` | No document | `Err` naming the element `x` |
| Bad number | `<y>` contains `abc` | No document | `Err` naming `y` and the value |
| Wrong root | Root element is not `AutoDrive` | No document | `Err` saying the root element is unexpected |
| No mapmarker | Config without `<mapmarker>` | Loads with no markers; unchanged save is identical | No error expected |

</frozen-after-approval>

## Code Map

- `crates/ad-core/src/lib.rs` — currently empty apart from its doc comment. Add `pub mod network;`.
- `crates/ad-core/tests/dependency_boundary.rs` — do not edit. It must keep passing with no new `ad-core` dependency.
- `crates/ad-io/Cargo.toml` — add `quick-xml = "0.42.0"` and `thiserror = "2.0.21"` through `[workspace.dependencies]` in the root `Cargo.toml`.
- `crates/ad-app/src/main.rs` — the empty eframe app from story 1.1 (`App::ui(&mut self, ui, frame)` is the eframe 0.37 signature). Extend it here. Add `rfd = "0.17.2"` through the workspace.
- `AutoDrive_config.xml` (repo root) — the fixture. Facts checked during planning:
  - It starts with a UTF-8 BOM and uses CRLF throughout, with a 4-space indent: `<waypoints>` at 4 spaces, children at 8, `<mmN>` at 8, marker fields at 12.
  - It has 35,652 nodes. Every x, y and z has exactly 3 decimals.
  - Flags are 0 or 1. Ids are 1..N in sequence.
  - `out`/`incoming` groups are separated by `;`, entries by `,`; empty groups are `-1`.
  - There are 223 markers `mm1..mm223`, each with `<id>N.000000</id><name>…</name><group>…</group>`.
  - Marker order is not node order.
  - The only escape used is `&amp;`, three times.
  - The file ends with `</AutoDrive>\r\n`.
- `.gitattributes` keeps the fixture `-text`. Do not change it.
- Java reference, for format only: `src/main/java/AutoDriveEditor/XMLConfig/GameXML.java:248-333` (parse) and `:503-669` (write). Do not port its number formatting (`String.valueOf`) or its marker order.

## Tasks & Acceptance

**Execution:**
- [ ] `Cargo.toml` -- add quick-xml, thiserror and rfd to `[workspace.dependencies]` at the spine's versions (rfd 0.17.2) -- one place pins versions.
- [ ] `crates/ad-core/src/network.rs` -- add the minimal model.
  - `NodeId(u64)`, monotonic.
  - `NodeFlag(u8)`.
  - `Node { x, y, z: f64, flag, outgoing: Vec<NodeId>, incoming: Vec<NodeId> }`.
  - `Marker { node: NodeId, name, group: String }`.
  - `Network { nodes: HashMap<NodeId, Node>, order: Vec<NodeId>, markers: Vec<Marker>, next_id }`, with read accessors and a `push_node` used by the loader.

  Story 3 completes this API -- the core model with stable ids (AD-2).
- [ ] `crates/ad-io/src/config_xml.rs` -- `load(bytes) -> Result<LoadedConfig, ConfigError>` and `save(&LoadedConfig) -> Vec<u8>`.
  - `LoadedConfig` holds the original bytes, the byte ranges of `<waypoints>…</waypoints>` and `<mapmarker>…</mapmarker>`, the detected newline and indent unit, the waypoint child order, and the `Network`.
  - `save` regenerates both blocks and splices them in. A missing `<mapmarker>` stays missing when there are no markers.
  - `ConfigError` names the element and the value.
  - This is the splice writer (AD-6).
- [ ] `crates/ad-io/src/lib.rs` -- `pub mod config_xml;` -- module wiring.
- [ ] `crates/ad-io/tests/golden_roundtrip.rs` -- load the repo-root fixture through `CARGO_MANIFEST_DIR`, save into a unique file under `std::env::temp_dir()`, read it back, assert the bytes are equal and delete the file -- the R1 proof.
- [ ] `crates/ad-io/src/config_xml.rs` (unit tests) -- one test per I/O matrix row except the golden one, on small inline configs -- edge cases.
- [ ] `crates/ad-app/src/main.rs` -- the editor shell.
  - A top bar with "Open…" and "Save" (Save is disabled with nothing loaded), using rfd dialogs, synchronously for now.
  - A central painter that fits all nodes into view and draws connections as lines and nodes as small dots.
  - A load or save error is shown in an `egui::Window`.

**Acceptance Criteria:**
- Given the fixture opened in the app, when the window shows, then the network is visible as dots and lines, fitted to the panel.
- Given the fixture opened in the app, when the user clicks Save, then the file on disk is byte-identical to before. The user checks this, for example with `git status`, which shows no change.
- Given a malformed config opened in the app, when loading fails, then an error window names the problem, and the previously loaded network stays shown.

## Implementation Notes

## Plan Change Log

## Review Triage Log

| # | Finding | Verdict | Route | Evidence |
|---|---------|---------|-------|----------|
| 1 | `save` panics when `<mapmarker>` precedes `<waypoints>` | medium | patch | `load` accepts either order; `save` slices `orig[waypoints.end..mm.start]`, out of range when mapmarker is first. Violates the "never panics on input" rule (AD-12). |
| 2 | `<AutoDrive/>` reports "unexpected root <AutoDrive>, expected <AutoDrive>" | low | reject | The message is real but confusing. Nobody meets it in everyday use, and the fix adds a branch. |
| 3 | Duplicate or non-sequential `<id>` values merge silently or are renumbered | low | reject | Only malformed or hand-edited files reach this; game files are 1..N. The fix adds validation guards. |
| 4 | A missing `<flags>` is generated on save | false | reject | AD-6 mandates generating a missing `<flags>` with zeros. The byte-identity rule targets the fixture, which has `<flags>`. |
| 5 | `&quot;`/`&apos;`/`&#NN;` and a raw `>` are not written back as-is | low | reject | Matches the plan's escape set. The fixture uses only `&amp;`. Fixing it means preserving raw text, which adds complexity. |
| 6 | NaN/inf are accepted, and >3 decimals become 3 | low | reject | `{:.3}` is the specified game format. NaN input is unlikely and needs a new guard. |
| 7 | A marker id like `2.7` maps to node 2 | low | reject | The game writes `N.000000`. Rejecting fractions adds a guard. |
| 8 | Inserting a missing `<mapmarker>` when markers exist is untested | low | reject | No caller can add markers in this story; that path is reachable only from story 3+. |
| 9 | Save always opens a Save-As dialog | false | reject | The plan requires rfd dialogs for Save. The dialog is prefilled with the opened path, so a confirm overwrites in place. |
| 10 | Bidirectional edges are drawn twice, with no culling | low | reject | Cosmetic for the tracer; the real map view is epic 2. |

## Design Notes

- **Splice, not re-serialise.** Use quick-xml only to walk events and record `buffer_position()` at the `<waypoints>`/`</waypoints>` and `<mapmarker>`/`</mapmarker>` boundaries, and to read their text.
  - Strip the BOM before handing bytes to the reader, then add its length back to the offsets.
  - Output is `original[..wp_start] + waypoints_block + original[wp_end..mm_start] + mapmarker_block + original[mm_end..]`.
- **Formatting.**
  - Coordinates: `format!("{:.3}", v)`. Every fixture value has exactly 3 decimals, so parse then format returns the same text.
  - Marker id: `format!("{}.000000", file_index)`, where `file_index` comes from the node's 1-based position in `order`.
- **Indent detection.**
  - The newline is `\r\n` if the bytes before `<waypoints>` end with it, else `\n`.
  - The base indent is the whitespace between that newline and `<waypoints>`.
  - The unit is the base indent when it is non-empty, else 4 spaces.
  - Children use base + unit, and marker fields use base + 2 × unit.
- **Adjacency storage.** It is `Vec<NodeId>` in file order, not a set, because the order is visible in the bytes (for example `2,696`). AD-3's set semantics are enforced by story 3's operations.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all pass, including `golden_roundtrip` and the config_xml unit tests

**Manual checks (if no CLI):**
- `cargo run -p ad-app --release`: open `AutoDrive_config.xml`, see the network, click Save, then confirm `git status` shows the fixture unchanged.
