- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/tickets/story-rust-workspace-toolchain-and-ci-run-green-plan.md`
  summary: Refresh the AGENTS.md block (bmad-project-context) so "Running and verifying" names the Rust workspace and its commands instead of the TODO.
  evidence: The Rust workspace now exists (crates/ad-core, ad-io, ad-app; cargo fmt, clippy -D warnings, test); AGENTS.md still says it is not yet created.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/tickets/story-edits-run-as-undoable-commands-with-saved-state-plan.md`
  summary: Deleting a node leaves stale `incoming`-only references (c.in∋d without d.out∋c) dangling, which a save would write as an invalid id.
  evidence: `remove_node` finds referrers through the node's own lists and the edge index (outgoing edges only). The FS25 fixture has 0 such entries (script check, 2026-11-05). It would settle with a scan of FS19/FS22 configs, or by deciding whether the planned consistency scan should repair them before save.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/tickets/story-refactor-sweep-plan.md`
  summary: Run bmad-project-context to refresh the AGENTS.md block, so that the hand-edited Rust lines (story 1.9) and the "Verified … against c3b4f8b" stamp are re-derived and re-stamped by the skill.
  evidence: Story 1.9 edited inside `<!-- bmad:context -->` by hand, at the user's choice. The block header says a refresh replaces such edits, and the stamp still names a commit from before the Rust workspace existed.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/tickets/story-refactor-sweep-plan.md`
  summary: Align the AGENTS.md Policy merge gate ("cargo build, cargo test and cargo clippy") with CI, which also runs `cargo fmt --all --check` and clippy with `-D warnings`.
  evidence: The gate predates this sweep. An agent that follows Policy alone can merge code that CI then fails on fmt or on clippy warnings. The fix edits agent-context, so it is deferred.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/epic-core-config-roundtrip-retrospective.md`
  summary: (Retro B4) The in-memory network keeps dangling `incoming` ids after a delete, and the edited golden test cannot catch a regression because save and its `model()` both drop unknown ids; the consistency scan (epic-scans-autosave) should find or repair them.
  evidence: `golden_edited.rs:22,41`, `config_xml.rs:506`, `network.rs:514-519`; verified in the epic retrospective.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/epic-core-config-roundtrip-retrospective.md`
  summary: (Retro B5) A panic inside a load or save job leaves Open or Save disabled for good, behind a "crashed" dialog while the window is still alive.
  evidence: `jobs.rs` spawn helpers never send a result after a panic, so `loading`/`saving` stay true; 1.7 triage deferred it to story 8, which did not solve it.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/epic-core-config-roundtrip-retrospective.md`
  summary: (Retro B6) Deeply nested XML inside `<waypoints>` or `<mapmarker>` overflows the job thread's stack in the recursive `read_elem`, aborting with no log and no dialog; add a depth limit that returns a typed error (AD-12).
  evidence: Recursion verified at `config_xml.rs` `read_elem`; a stack overflow is not caught by the panic hook.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/epic-core-config-roundtrip-retrospective.md`
  summary: (Retro B7) A self-closing `<mapmarker/>` counts as missing, so Save inserts a second mapmarker block.
  evidence: Verified in the retrospective; whether the game ever writes that form is unknown (only the FS25 fixture is available).

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/epic-core-config-roundtrip-retrospective.md`
  summary: (Retro B8) Several markers on one node load, but `EditMarker` changes only the first; AD-17 says at most one marker per node. The markers epic decides.
  evidence: `network.rs` marker lookup and `EditMarker` apply; seen in the retrospective's boundary review.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/epic-core-config-roundtrip-retrospective.md`
  summary: (Retro S1) Split `network.rs` (about 1,320 lines) into spatial and markers modules before epic-map-view adds queries.
  evidence: `crates/ad-core/src/network.rs` holds id types, kind derivation, markers, undo deltas, adjacency, two R-trees, five queries and a polygon helper.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/epic-core-config-roundtrip-retrospective.md`
  summary: (Retro A2) Extend the AD-1 dependency check so `ad-io` cannot pick up UI crates, and consider a separate CI step for it.
  evidence: `crates/ad-core/tests/dependency_boundary.rs` checks only `ad-core`, and runs inside `cargo test`.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/epic-core-config-roundtrip-retrospective.md`
  summary: (Retro R4) Every Save is a Save As (a dialog prefilled with the path); AD-16 distinguishes Save and Save As. For epic-settings-ui-shell.
  evidence: `crates/ad-app/src/main.rs` `save` always opens the file dialog.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/epic-core-config-roundtrip-retrospective.md`
  summary: (Retro P2) `ad-app` carries errors as `String`, there is no string table for UI text, and the command is `AddNode` where the conventions say `AddNodes`. For epic-settings-ui-shell or the tools epic.
  evidence: `jobs.rs` `JobResult`, `main.rs` `error`, `command.rs` `AddNode`.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-map-view/tickets/story-markers-parking-and-warning-icons-marker-names-and-the-grid-plan.md`
  summary: The height-warning check (y == -1) walks every visible node each frame, even at minimum zoom where no dots are drawn; keep the set of nodes without height in the Network (updated by add/move/remove) so the view looks up only those.
  evidence: view.rs queries all ~35,652 fixture nodes per frame at minimum zoom for this check; min-zoom median rose ~1-2 ms to about 9.4 ms on a quiet run, still under the 16 ms budget. Story 2.7 (complete-view budget) is the natural place.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-map-view/tickets/story-the-view-finds-the-node-or-connection-under-the-cursor-plan.md`
  summary: Decide in epic-node-connection-tools whether `hovered_edge` should test the drawn segment (shortened by ns/2 at each end, skipped below MIN_EDGE_PX) instead of the full centre-to-centre segment, and whether a hovered node suppresses the edge hit.
  evidence: hover.rs `edge_at` uses the edge index's full segment, so a cursor on a node also reports that node's connections, and connections too short to be drawn can be hit; nothing reads `hovered_edge` yet.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-selection-clipboard/tickets/story-dragging-the-whole-network-selected-stays-within-the-frame-b-plan.md`
  summary: Add a frame-time case that drags a large selection far on both axes at minimum zoom (the view's whole-map "joint" preview query), and, if it is near the budget on the developer's machine, a uniform-offset query (view shifted back into stored coordinates plus the view) that keeps the drawn result identical.
  evidence: No frame-time case reaches that branch; forced on, it measured up to 12.1 ms best-round max on the implementer's machine, which ran about 1.4x faster than the developer's on earlier code, so it is unverified against 16 ms there (medium if true).

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-selection-clipboard/tickets/story-copy-cut-and-paste-to-the-view-centre-or-the-original-place-plan.md`
  summary: Let Ctrl+C/X/V reach a focused or highlighted text widget outside the modal dialogs before the editor's clipboard shortcuts take them.
  evidence: input.rs consumes egui's Copy/Cut/Paste events before the UI is drawn; today only the error window shows selectable text.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-map-images-import/tickets/story-a-missing-map-image-downloads-in-the-background-plan.md`
  summary: A map-image download that finishes after another config was opened may overwrite an image the user imported for the same map meanwhile; skip the save when the target appeared after the download started.
  evidence: jobs.rs saves the fetched bytes unconditionally; Import is enabled again as soon as the open document changes.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-map-images-import/tickets/story-refactor-sweep-plan.md`
  summary: Add the AD-1 dependency check (ad-core may not depend on I/O or UI crates) to CI, as the architecture's deployment section says it runs.
  evidence: .github/workflows/ci.yml runs only cargo fmt, clippy and test.
