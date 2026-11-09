---
title: 'Pushing a v* tag builds the exe and attaches it to a draft release'
type: 'chore'
ticket: '2'
created: '2026-11-09'
status: 'built'
baseline_revision: '900a2e0'
route: 'oneshot'
route_source: 'auto'
risk: 'medium'
review: 'quick'
review_source: 'pinned'
lenses_ran: ['quick']
review_loop_iteration: 0
context: []
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** nothing builds or publishes a release; the user chose a GitHub Action on tag (2026-11-09).

**Approach:**
- `.github/workflows/release.yml` runs on `v*` tags on windows-latest with `contents: write`:
  - installs the pinned toolchain;
  - checks that the tag names the crate version (`.github/scripts/check-tag.sh`);
  - runs fmt, clippy and the tests like CI;
  - builds the release exe;
  - `gh release create <tag> AutoDriveCourseEditor.exe --draft --verify-tag --generate-notes`.
- The release is a draft and not marked pre-release. The user checks it and publishes it; the update check reads `releases/latest`, which skips drafts and pre-releases.
- `.gitattributes`: `*.sh` is checked out with LF, so bash on the Windows runner can run the script.

## Boundaries & Constraints

**Never:** no tag is pushed in this story (story 7, with the user's go-ahead); no publish without a person.

</frozen-after-approval>

## Code Map

- `.github/workflows/release.yml` (new).
- `.github/scripts/check-tag.sh` (new).
- `.gitattributes` -- LF for `*.sh`.

## Implementation Notes

- The first real run is story 7. Until then the workflow is checked by reading it and by running the version check locally:
  - `v0.9.0` passes;
  - `v0.9.1` and `0.9.0` fail with a message.
- `GITHUB_REF_NAME` is the tag name. The runner's preinstalled `gh` uses the job token.

## Verification

**Commands:**
- `bash .github/scripts/check-tag.sh v0.9.0` -- expected: exit 0; `v0.9.1` -- expected: exit 1
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` -- expected: pass (no Rust change)

## Review Triage Log

- Quick review (orchestrator, inline): one finding, fixed. The version check first ran before the toolchain install; it now runs after, so `cargo pkgid` uses the pinned toolchain.
- Quick review: one finding, fixed. A CRLF checkout would break the script; LF is now forced for `*.sh`.
