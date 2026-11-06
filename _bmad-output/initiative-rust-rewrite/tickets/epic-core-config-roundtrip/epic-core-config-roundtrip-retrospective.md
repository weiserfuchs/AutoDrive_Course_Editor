---
epic: epic-core-config-roundtrip
date: 2026-11-06
verdict: accepted-with-open-items
criteria: declared
headless: false
---

# Retrospective: FS25 configs load, display and save losslessly

## Epic summary

- **Epic:** `epic-core-config-roundtrip`. Criteria are declared in the epic file: Requirements R1–R4 and Done when.
- **Tickets:** all nine are `done` (1.1–1.9); `pending_tickets` is empty, and none is left at `built`.
- **Going-in concerns:** the user had none to add (2026-11-06).

**Plan ranges** (each from the plan's `baseline_revision` to the next baseline; 1.9's end is the epic head, inferred):

| Ticket | Range | Commits |
|--------|-------|---------|
| 1.1 | afbadf8..ccf8e09 | 7706926 chore, 15fb211 done, ccf8e09 plan 1.2 |
| 1.2 | ccf8e09..a36ed99 | 93363f7 feat, a36ed99 done |
| 1.3 | a36ed99..04b453f | 492f3c1 feat, 04b453f done |
| 1.4 | 04b453f..29e753c | 0adf743 feat, 29e753c done |
| 1.5 | 29e753c..ac28086 | de27175 feat, ac28086 done |
| 1.6 | ac28086..496320d | b747c0f feat, 496320d done |
| 1.7 | 496320d..c3cdf50 | 9cf8f34 feat, c3cdf50 done |
| 1.8 | c3cdf50..ca267ad | 5a84732 feat, ca267ad done |
| 1.9 | ca267ad..f6d89eb | bc29f12 docs, f6d89eb done |

No merge commits: every story was fast-forwarded into `master`.

**Evidence inventory:**
- **Available:**
  - the epic file and the nine plans, each with frozen intent, Implementation Notes and a Review Triage Log;
  - `git_evidence.py` output per range;
  - the full epic diff (`afbadf8..f6d89eb`, about 4,860 lines; it is in the session scratchpad, which is temporary);
  - the session transcript for stories 1.2–1.9 (`0215c868-31d1-4e1d-9f23-3cfe97331187.jsonl` in the Claude project folder);
  - `deferred-work.md`.
- **Missing:**
  - story files: no ticket was pulled, so the entries in `tickets.toml` are the spec;
  - run folders in the epic folder: none;
  - a previous retrospective: this is the first epic;
  - a session log for story 1.1, which ran in an earlier session that this one only knows through a summary;
  - any CI run before this retrospective: no story had been pushed. `master` was pushed during the retro (see Acceptance verdict).

## Findings

**Scope:**
- Aggregate views were derived by a subagent from the full diff and the current code. Builds and tests were not run in that pass; the last full run on `master` was green, see Behavior verification.
- The diff review ran the adversarial, edge-case and verification-gap lenses inline through a subagent, weighted on the boundaries between stories. `bmad-review` itself was not invoked, so this is narrower than a full lens run.
- I re-checked the claims marked "verified" myself.

### Architecture delta

| # | Finding | Source | Disposition |
|---|---------|--------|-------------|
| A1 | The crate graph keeps to AD-1 (app → io → core). `ad-core` depends only on rstar and thiserror. | `crates/*/Cargo.toml`, `crates/ad-core/tests/dependency_boundary.rs` | accept |
| A2 | The AD-1 check covers only `ad-core`, nothing stops `ad-io` from picking up UI crates, and CI has no separate dependency step (the check runs inside `cargo test`). | `dependency_boundary.rs`, `.github/workflows/ci.yml` | defer |
| A3 | `Document` lives in `ad-app`, not in `ad-core` as the seed says. `replace_document` is a free function in `jobs.rs`, not `App::replace_document`. The saved state is `History` + `SavePoint`, not `Document.saved_at`. `GameVersion` sits on `LoadedConfig`. | `crates/ad-app/src/document.rs`, `jobs.rs:75`, `history.rs:22-28`, `config_xml.rs:113`; user decision in the 1.7 plan | accept (recorded deviation); reconcile the spine, see Action items |
| A4 | AD-4 is enforced only by convention (verified). The network can be changed without going through `History`, which also bypasses the read-only gate and the dirty state. Story 1.4 rejected three findings because "AD-4 forbids it". | `config_xml.rs:116` (`pub network`), `document.rs:11-12` (`pub config`/`history`), every `pub` mutator in `network.rs` | fix before the tools epic |

### Duplication map

| # | Finding | Source | Disposition |
|---|---------|--------|-------------|
| D1 | The atomic writer (temp file, `sync_all`, rename, removal on error) exists twice, almost line for line. The 1.9 sweep did not catch it. | `config_xml.rs:624-656`, `settings.rs:82-116` | fix now (small) |
| D2 | The fixture path is built three times, and `scratch_dir` and the temp-dir helpers exist twice in the tests. | `jobs.rs:134`, `golden_*.rs`, `config_xml.rs:1054`, `settings.rs:123` | accept |
| D3 | The rollback-after-partial-apply loop is written three times. | `command.rs:146-156`, `:167-176`, `:240-252` | accept |

### Size growth

| # | Finding | Source | Disposition |
|---|---------|--------|-------------|
| S1 | `network.rs` has 1,320 lines (about 718 production). It holds the id types, the kind derivation, markers, undo deltas, adjacency, two R-trees, five queries and a polygon helper. | `crates/ad-core/src/network.rs` | defer: split out `spatial` and `markers` before the map-view epic adds queries |
| S2 | `config_xml.rs` has 1,088 lines (about 656 production): reader, version detection, builder, `SaveIdMap`, writer and atomic file write. | `crates/ad-io/src/config_xml.rs` | accept for now |

### Pattern divergence

| # | Finding | Source | Disposition |
|---|---------|--------|-------------|
| P1 | AD-12's deny lints (`unwrap_used`/`expect_used`) are set only in `ad-core` and `ad-io` (verified). `ad-app` is not covered, and there is no `[workspace.lints]`. | `crates/*/Cargo.toml` | fix now |
| P2 | `ad-app` carries errors as `String`, there is no string table (conventions), and the command is named `AddNode` where the convention says `AddNodes`. | `jobs.rs:15,21`, `main.rs:23`, `command.rs:16` | defer to epic-settings-ui-shell / tools |
| P3 | Two styles of the test-module `expect` allow (inner and outer attribute), and the one in `jobs.rs` has no effect. | network/command/history vs config_xml/settings | accept |

### Spec reconciliation

| # | Finding | Source | Disposition |
|---|---------|--------|-------------|
| R1 | **The Done-when "CI green on master" was not met before the retro.** Nothing has been pushed: local `master` is 23 commits ahead of `origin/master` (`c3b4f8b`), so CI has never run, and the 1.1 toolchain step was left "unverified until the first push". | `git rev-list --left-right --count master...origin/master` → `23 0`; 1.1 plan, Implementation Notes | resolved: pushed during the retro, run 42198541244 green |
| R2 | R1–R4 are met by tests: the byte-identical golden test, the edited golden test, the FS19/22/25/legacy unit tests, and the History tests. | `golden_roundtrip.rs`, `golden_edited.rs`, `config_xml.rs:749-813`, `history.rs` tests | accept |
| R3 | R4's dirty state is not visible in the app yet: `Document::apply`/`is_dirty` are `dead_code`. The epic leaves the UI to epic-node-connection-tools. | `document.rs:31,36` | accept |
| R4 | Every Save is a Save As (a dialog prefilled with the path). AD-16 knows Save and Save As. | `main.rs:79-88` | defer to epic-settings-ui-shell |

### Boundary lenses (diff review)

| # | Finding | Boundary | Source | Disposition |
|---|---------|----------|--------|-------------|
| B1 | **Open during a save** (or Save during a load) of the same file can show the old content as a clean document, and a later Save overwrites the edits that were just saved (verified: Open is gated only on `loading`, Save only on `saving`). | 1.7 / AD-18 | `main.rs:159-168` | fix now (gate both buttons on both flags) |
| B2 | An FS22/FS25 file with flags 2/4 is clean after loading, yet Save writes different bytes (the flag reset). A close prompt or autosave would treat it as unchanged. | 1.6 / 1.5 / 1.7 | `config_xml.rs:301-305`, `history.rs:66` | fix: the user decided the document should be dirty after a reset (Action item 2) |
| B3 | No test checks that a read-only config gets a gated `History` through `Document::new`/`apply_result`; removing `document.rs:21` breaks nothing. | 1.6 / 1.7 | `document.rs:21`, `jobs.rs` tests | fix now (one test) |
| B4 | The edited golden test cannot detect a regression of the 1.4 Reverse-target fix, because both the save and its `model()` drop unknown ids. The in-memory network keeps dangling `incoming` ids, so the "settled" in 1.9 holds only for the bytes on disk. | 1.4 / 1.5 / 1.9 | `golden_edited.rs:22,41`, `config_xml.rs:506`, `network.rs:514-519` | defer (consistency scan, epic-scans-autosave) |
| B5 | A panic in a job leaves Open or Save permanently disabled, now behind a "crashed" dialog for a window that is still alive. 1.7 triage #2 deferred this to story 8, which did not solve it. | 1.7 / 1.8 | `jobs.rs:32-71`, `startup.rs:28-43` | defer |
| B6 | Deeply nested XML in a modelled block overflows the job's stack (`read_elem` recurses). That aborts the process with no log and no dialog. | 1.2 / AD-12 / 1.8 | `config_xml.rs:142-182` (verified recursion at `:163`) | defer |
| B7 | A self-closing `<mapmarker/>` counts as missing (verified), so Save adds a second block. Whether the game ever writes that form is unknown. | 1.2 / 1.5 | `config_xml.rs:374-378`, `:599-608` | defer |
| B8 | Several markers on one node load, but `EditMarker` handles only the first (AD-17 says at most one). | 1.2 / 1.4 | `network.rs:277`, `:580-609` | defer (markers epic) |
| B9 | A `history_cap = 0` setting silently means 1, and the first start pins the default 500 in the user file. | 1.4 / 1.8 | `history.rs:58-63`, `startup.rs:89-94` | accept (document in the settings UI epic) |
| B10 | On read-only configs, links from nodes with flags 2/4 are classified `Unknown`. | 1.3 / 1.6 | `network.rs:632-654`, `config_xml.rs:303` | accept |

**Checked and clean:**
- `SavePoint`/`mark_saved_at` vs the read-only gate;
- epoch handling vs load errors;
- `mark_saved_at` with an id from the redo tail.

### Process lessons (from the plans and the session transcript)

- **L1 Plan size:** six of the nine plans were above the 1,600-token guideline (about 1,700–2,000). The user kept each one whole.
- **L2 Unverified manual checks:** the GUI checks for 1.7 (responsiveness, error window, `git status` after Save) and the 1.8 panic dialog never ran. 1.6 and 1.8 were checked only headless by the agent. Recorded in the plans' Implementation Notes.
- **L3 Deferred to stories that did not deliver:** a reviewer rejection deferred to a later story was not carried there. 1.7 #2 pointed at story 8 (B5), and the 1.9 sweep closed the stale-`incoming` entry only for the bytes on disk (B4).
- **L4 Same agent for implementation and patches:** the per-story quick review caught real defects every time (1.2 #1, 1.4 #1 high, 1.5 #5). The cross-story defects B1–B3 still show up only here.

## Behavior verification

Exercised:
- **1.2 and 1.5:** the user opened the fixture in the app and saved it; `git status` was clean (plan Implementation Notes).
- **1.6:** the agent loaded a legacy copy of the fixture through `config_xml::load`. It was read-only `Legacy` with 35,652 nodes and 223 markers, and save was refused.
- **1.8:** the agent started the debug app. It wrote `settings.toml` (cap 500) and `editor.log.2026-11-06`, with the settings lines in the log.
- **All of `cargo test --workspace` passed on `ca267ad`/`f6d89eb`:** 90 tests, including both golden tests and one real background load.

Not exercised:
- in the GUI: responsiveness during loading, the error window for a broken file, Save over the fixture after 1.7, the `AD_DEBUG_PANIC` dialog;
- CI was not exercised before the retro; it was run during the retro: run 42198541244 is green.

## Previous-retro follow-through

There was nothing to follow through on: this is the first epic, so no previous retrospective file exists.

## Action items

These are proposed, not applied. Code fixes go through the normal build loop, and spec changes go through `bmad-agent-po`.

| # | Action | From | Owner | Kind |
|---|--------|------|-------|------|
| 1 | **Follow-up fixes, as one small story before the next epic:** (a) gate Open and Save on both `loading` and `saving`; (b) add a test that a legacy or unknown-version config gives a gated `History` through `apply_result`; (c) add `[lints.clippy] unwrap_used/expect_used = "deny"` to `ad-app`, or move them to `[workspace.lints]`; (d) use one shared atomic writer for `config_xml::save_to` and `settings::save`. | B1, B3, P1, D1 | dev (`bmad-build`) | remediation |
| 2 | Make a document dirty right after loading when the FS22/FS25 flag reset changed any flag (user decision, 2026-11-06). | B2 | PO (`bmad-agent-po`) → dev | new story |
| 3 | Enforce AD-4 by types before epic-node-connection-tools: `Network` mutators not reachable from outside `History`/`Command`, and `Document`/`LoadedConfig` network access read-only. | A4 | PO → architect | story in the tools epic |
| 4 | Reconcile the architecture spine with as-built: `Document` in `ad-app` (see ADR), saved state as `History`+`SavePoint` instead of `Document.saved_at`, `GameVersion` on `LoadedConfig`, `replace_document` as a free function. | A3 | architect (`bmad-architecture`) | spec reconciliation |
| 5 | Carry these deferred findings into the tree or deferred-work so they are not lost: B4 (dangling `incoming` in memory → consistency scan), B5 (a job panic leaves the buttons stuck), B6 (nesting depth limit in `read_elem`), B7 (`<mapmarker/>`), B8 (several markers per node), S1 (split `network.rs` before map view), A2 (AD-1 check for `ad-io`), R4 (Save vs Save As), P2 (string table, typed app errors, `AddNodes` naming). | B4–B8, S1, A2, R4, P2 | PO | backlog |
| 6 | Run the open GUI checks: responsiveness while loading, the error window for a broken file, Save over the fixture with `git status` clean, and the `AD_DEBUG_PANIC=1` dialog plus the ERROR line in the log. | L2 | user | verification |
| 7 | CI warns that `actions/checkout@v4` targets Node.js 20; bump it to a Node 24 major. | CI run 42198541244 annotation | dev | chore |
| 8 | **Process:** when a reviewer finding is rejected because "a later story covers it", write a deferred-work entry naming that story, so its plan picks it up. | L3 | build workflow user | process |
| 9 | **Process:** end each epic with a cross-story boundary review. It found B1–B3, which no per-story review could see. | L4 | build workflow user | process |
| 10 | **Process:** the plans ran at about 1,700–2,000 tokens six times, and the user kept each one whole. Either raise the target for this project or trim Design Notes earlier. | L1 | user | process |

## Decision records

- **Proposed:** "The Document lives in ad-app and owns the loaded config". Source: the 1.7 plan (Open Questions and the frozen decision) and commit `9cf8f34`. **The user approved it,** and it was written to `docs/decisions/adr-document-lives-in-ad-app.md`.
- **Considered but not proposed:**
  - The FS22/FS25 flag reset: AD-11 already records it.
  - The read-only gate on `History`: a temporary placement that the 1.6 plan records as handed to `Document`.
  - Skipping dangling references at save: the 1.5 plan records it, and it is easy to reverse.

## Acceptance verdict

**accepted-with-open-items** (criteria declared; the user decided this before CI ran: "decide after CI", accept with open items if CI is green).

- **R1–R4:** met by tests.
  - `golden_roundtrip` (the byte-identical FS25 fixture) and `golden_edited`;
  - the FS19/FS22/FS25/unknown/legacy unit tests and the read-only gate;
  - the History tests for undo, redo, the cap and saved state.
- **Done when:**
  - The CI run after pushing `master` (`c3b4f8b..f6d89eb`, 2026-11-06) is green: run 42198541244, job `check` (fmt, clippy `-D warnings`, test including the AD-1 boundary test and both golden tests).
  - The 1.1 toolchain step is settled by this run.
- **All nine tickets** are done.
- **Open items:** Action items 1–10. Of these, B1 (Open during a save can lose just-saved edits) and B2 (dirty after the flag reset) are the most user-visible.

## Open questions

- Does the game ever write `<mapmarker/>` in self-closing form, or several markers on one node (B7, B8)? A real FS19/FS22 config would settle it; the repo has only the FS25 fixture.
- Should `history_cap = 0` mean "unlimited", or be rejected (B9)? The settings UI epic decides.
