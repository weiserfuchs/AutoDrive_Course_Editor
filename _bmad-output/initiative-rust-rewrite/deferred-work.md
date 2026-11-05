- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/tickets/story-rust-workspace-toolchain-and-ci-run-green-plan.md`
  summary: Refresh the AGENTS.md block (bmad-project-context) so "Running and verifying" names the Rust workspace and its commands instead of the TODO.
  evidence: The Rust workspace now exists (crates/ad-core, ad-io, ad-app; cargo fmt, clippy -D warnings, test); AGENTS.md still says it is not yet created.

- source_plan: `_bmad-output/initiative-rust-rewrite/tickets/epic-core-config-roundtrip/tickets/story-edits-run-as-undoable-commands-with-saved-state-plan.md`
  summary: Deleting a node leaves stale `incoming`-only references (c.in∋d without d.out∋c) dangling, which a save would write as an invalid id.
  evidence: `remove_node` finds referrers through the node's own lists and the edge index (outgoing edges only). The FS25 fixture has 0 such entries (script check, 2026-11-05). It would settle with a scan of FS19/FS22 configs, or by deciding whether the planned consistency scan should repair them before save.
