<!-- Title and squash subject: English Conventional Commits. The body may use
the team's working language. Fill the three core sections with actual results,
not unchecked promises. Release intent is derived from the diff: do not add a
manual "versions unchanged" line. Release PR: replace this template with Task +
Release notes from CONTRIBUTING.md §6. Dependabot keeps its generated body; the
maintainer records review results. Delete conditional sections that do not apply. -->

## Summary

<!-- Concrete problem, resulting behavior and why it is needed.
Registered task: put its BR/INC ID and card link first.
Diagnostic registration describes the problem and next artifact, not a shipped fix. -->

## Validation

<!-- Commands/checks actually run, results and exact evidence links; the
applicable acceptance class (automated merge gate / Windows runtime merge gate /
production observation). Name skipped checks and unavailable environments.
CI build success does not prove a live Windows/CEF scenario. -->

## Risks and limitations

<!-- Regression/compatibility risks, rollback, checks not run and remaining gates. -->

<!-- Conditional sections (keep only those that apply):

## Task
Primary BR/INC ID first for a registered runtime task (CI reads it from here,
the title or the start of the body).

## Runtime acceptance
Required when the task card names a Windows runtime merge gate; exact fields in
CONTRIBUTING.md "Класс приёмки". Removing it makes the metadata check red.

## Dependencies and handoff
Merge order, prerequisites, next artifact and owner, resume condition.

Private evidence: link the private PR/artifact only. Never paste raw private
logs, prompts, user paths, internal sources, screenshots or secrets here.
Credentials must not enter either repository. -->
