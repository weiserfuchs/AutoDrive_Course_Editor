---
title: 'v0.9.0 is tagged and released'
type: 'chore'
ticket: '7'
created: '2026-11-11'
status: 'done'
baseline_revision: 'b361bb3'
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

**Problem:** the epic is done when a tagged release publishes the Windows exe.

**Approach:**
- With the user's go-ahead (2026-11-11), push master and the annotated tag `v0.9.0`.
- The release workflow builds the draft; the orchestrator checks the attached exe.
- The user publishes the release.

</frozen-after-approval>

## Implementation Notes

- **Workflow:**
  - `check-tag.sh v0.9.0` passed locally before the push.
  - Run 43094795891 passed every step in about 15 minutes.
- **Downloaded asset** (`AutoDriveCourseEditor.exe`, 21 MB):
  - PE x64, subsystem 2 (GUI);
  - contains "0.9.0" and the fork URL.
- **Release notes:** the orchestrator set them on the draft.
- **Publishing:** the user published the release; Claude Code's permission check blocked the orchestrator's publish, as a public action.
- **Result:**
  - https://github.com/weiserfuchs/AutoDrive_Course_Editor/releases/tag/v0.9.0, not a draft, not a pre-release.
  - `releases/latest` (the update check's endpoint) returns it, so builds older than 0.9.0 would be offered it, and 0.9.0 itself shows nothing.

## Review Triage Log

- Quick review (orchestrator, inline): no finding.
