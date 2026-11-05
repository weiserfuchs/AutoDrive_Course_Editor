---
title: 'Rust workspace, toolchain and CI run green'
type: 'chore'
ticket: '1'
created: '2026-11-05'
status: 'built'
baseline_revision: 'afbadf83f4147b8b0226e39435d34e9dbaace0a8'
route: 'full'
route_source: 'auto'
risk: 'low'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: ['{project-root}/_bmad-output/initiative-rust-rewrite/architecture-rust-rewrite/architecture-rust-rewrite.md']
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The repository has no Rust code. Every later story needs a workspace with the three crates from the architecture spine, a pinned toolchain, and CI that enforces formatting, lints, tests and the AD-1 dependency rule.

**Approach:** Add a Cargo workspace at the repo root with `ad-core` and `ad-io` (libraries) and `ad-app` (an eframe binary that opens an empty window). Pin Rust 1.99.1. Add a GitHub Actions workflow and an `ad-core` test that fails when `ad-core` gains a forbidden dependency.

## Boundaries & Constraints

**Always:**
- Use edition 2024 and the crate names and directory layout of the spine's Structural Seed (`crates/ad-core`, `crates/ad-io`, `crates/ad-app`).
- Dependency direction is `ad-app → ad-io → ad-core`, plus `ad-app → ad-core`.
- Declare `clippy::unwrap_used` and `clippy::expect_used` as `deny` in `ad-core` and `ad-io` (AD-12).
- Declare eframe at 0.37.0 as a workspace dependency.

**Never:**
- Add dependencies that no code uses yet. Later stories add quick-xml, rstar and the rest.
- Touch `src/main/java`, `pom.xml` or the Java `target/` contents.
- Write a global `static mut` or model state (AD-1).
- Push to the remote. The user pushes.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Clean checkout | `cargo test` | All crates build and the tests pass | No error expected |
| Boundary violation | A dependency such as `eframe` or `quick-xml` is added to `ad-core/Cargo.toml` | The boundary test fails | The message names the forbidden crate |
| App start | `cargo run -p ad-app` | A window titled "AutoDrive Course Editor" opens and closes cleanly | No error expected |

</frozen-after-approval>

## Code Map

- Repo root: nothing Rust-related exists yet. `.gitignore` already lists `target/`. Cargo's `target/` shares the Java Maven output folder name. That is harmless, because the Java build output is untracked and is removed in the final epic.
- `.gitattributes` — keep the `AutoDrive_config.xml -text` line unchanged.
- `AGENTS.md` (Running and verifying) — contains a TODO line for the Rust build commands. A refresh by `bmad-project-context` replaces it after this story. Do not edit it here.
- Local toolchain: `stable-x86_64-pc-windows-msvc`, cargo 1.99.1.

## Tasks & Acceptance

**Execution:**
- [ ] `Cargo.toml` -- create the workspace (`resolver = "3"`, members `crates/*`), `[workspace.package]` (edition 2024, rust-version 1.99) and `[workspace.dependencies]` with eframe 0.37.0 -- one place pins shared versions.
- [ ] `rust-toolchain.toml` -- channel 1.99.1 with the rustfmt and clippy components -- reproducible toolchain (spine Stack).
- [ ] `crates/ad-core/Cargo.toml`, `crates/ad-core/src/lib.rs` -- library with no dependencies, the clippy deny lints and a crate doc comment stating its role -- core layer.
- [ ] `crates/ad-core/tests/dependency_boundary.rs` -- parse `ad-core`'s own `Cargo.toml` (`CARGO_MANIFEST_DIR`) and fail if any `[dependencies]` or `[dev-dependencies]` entry is not on a short allowlist. The allowlist starts empty and later stories extend it with rstar and similar crates. The test explicitly rejects egui, eframe, quick-xml, image, image_dds, zip, ureq, rfd and resvg -- the AD-1 check, run by `cargo test`.
- [ ] `crates/ad-io/Cargo.toml`, `crates/ad-io/src/lib.rs` -- library that depends on ad-core, with the clippy deny lints -- I/O layer.
- [ ] `crates/ad-app/Cargo.toml`, `crates/ad-app/src/main.rs` -- binary that depends on ad-core, ad-io and eframe, and opens an empty `eframe` window titled "AutoDrive Course Editor" -- shell layer.
- [ ] `.github/workflows/ci.yml` -- on push and pull request, on `windows-latest`: install the toolchain from `rust-toolchain.toml`, cache, then run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` -- CI per the spine.

**Acceptance Criteria:**
- Given a fresh clone, when the three CI commands run locally, then all pass.
- Given the workspace, when `cargo tree -p ad-core` runs, then it lists no crates outside the allowlist.
- Given the app built in debug, when the user runs `cargo run -p ad-app`, then an empty window titled "AutoDrive Course Editor" appears. This is checked by the user.

## Implementation Notes

- Review patches were applied directly by the orchestrator; the implementer session had already ended.

- Implemented by subagent: workspace, toolchain, three crates, CI workflow, boundary test. The test returns `io::Result` because `expect_used` is denied.
- Matrix audit: the boundary-violation row was only covered at the parser level. Added `violations()` and `forbidden_dependency_is_reported_by_name` so the failure message naming the crate is tested.
- The CI toolchain step (`rustup show active-toolchain || rustup toolchain install`) is unverified until the first push. The app window is checked manually by the user.

## Plan Change Log

## Review Triage Log

Pass 1 (quick): high 0, medium 1, low 1, false 0, maybe-false 1; rejected 3.
- medium / patch — `dependency_boundary.rs` `dependency_names` missed build, target-specific, renamed (`package =`) and comment-suffixed dependency tables, so AD-1 could be bypassed. Fixed: quote-aware key split, comment strip, `package` rename detection, plus the test `parser_sees_build_target_renamed_and_commented_dependencies`.
- maybe-false / patch — `ci.yml` toolchain step `rustup show active-toolchain || rustup toolchain install` might succeed without the file's components. Replaced by `rustup toolchain install`, which installs the toolchain and components named in `rust-toolchain.toml`. Settled at the first CI run.
- low / rejected — CI runs twice per PR update (push and pull_request). Harmless, and the plan asks for both triggers.
- rejected — unchecked task boxes in the plan. The fix would edit the plan.
- defer — the TODO for Rust commands in AGENTS.md is stale. The fix edits an agent-context file; logged in deferred-work.md.

## Verification

**Commands:**
- `cargo fmt --all --check` -- expected: exit 0
- `cargo clippy --workspace --all-targets -- -D warnings` -- expected: exit 0
- `cargo test --workspace` -- expected: all tests pass, including `dependency_boundary`
- `cargo tree -p ad-core --edges normal` -- expected: only `ad-core` itself

**Manual checks (if no CLI):**
- User runs `cargo run -p ad-app` and sees the empty window titled "AutoDrive Course Editor".
