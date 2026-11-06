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
