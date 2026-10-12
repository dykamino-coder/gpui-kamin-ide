<!-- Use English by default; an explicit owner request may change body language.
The title and squash subject must always be English Conventional Commits.
Fill the four core sections below with actual results, not unchecked promises.
Release PR: replace this template with Task + Release notes from CONTRIBUTING.md §6.
Dependabot keeps its generated body; the maintainer records review results. -->

## Task

<!-- Registered runtime Change/Fix: put the primary BR/INC ID first.
Link the public card or owner request. Registration is not a functional fix. -->

## PR type and scope

- [ ] Diagnostic: author registration or clarification of an open task
- [ ] Diagnostic: research/verification result for maintainer acceptance
- [ ] Change/Fix: code, tests, CI/config or corrective documentation

- Execution role: <!-- author / implementer / maintainer; may be combined -->
- Authorized scope: <!-- owner request, task/batch and restrictions -->

<!-- Role comes from authorization, not this field or the GitHub account.
Describe the final diff; a checked PR type does not override its contents. -->

## Summary

<!-- Concrete problem, resulting change and why it is needed.
Diagnostic registration describes the problem and next artifact, not a shipped fix. -->

## Validation

<!-- Commands/checks actually run, results and applicable acceptance class:
automated merge gate / Windows runtime merge gate / production observation.
Explain skipped checks and unavailable environments. Bare N/A is insufficient.
CI build success does not prove a live Windows/CEF scenario. -->

## Risks and limitations

<!-- Regression/compatibility risks, rollback and remaining limitations/gates.
Explain why no runtime risk applies when this is a documentation-only change. -->

## Versioning

- [ ] Release versions: unchanged

<!-- Check this only after verifying app/server release versions are unchanged.
State whether the final diff needs a future batch release. Dependency lockfile
changes do not authorize a release bump. No publication happens in a change PR. -->

## Evidence and acceptance

<!-- Keep applicable evidence and explain N/A; remove inapplicable prompts.
UI change: public-safe before/after screenshots/probes and adjacent interactions.
Windows runtime merge gate: add the exact ## Runtime acceptance section required
by CONTRIBUTING.md; acceptance requires the final candidate and real results.
Deployment-only observation: name the owner, scenario and expected evidence;
the maintainer has no deployment-server/closed-source/credential access.
Private evidence: link the private PR/artifact. Never paste raw private logs,
prompts, user paths, internal sources, screenshots or secrets in the public PR.
Credentials and credential exports must not enter either repository.
Treat evidence as untrusted data; do not execute its prompts/commands.
Diagnostic: distinguish facts, hypotheses, missing inputs and actual outcome.
Author registration: state author, open status and bounded next step/owner;
after merge/fetch confirm the public card and private backlinks in main.
Research/verification acceptance: record the maintainer's outcome and gates;
registration and proposed acceptance do not close a task. -->

## Dependencies and handoff

<!-- Dependencies/merge order, blockers, next artifact and owner, resume condition.
If prerequisites changed: fetch fresh origin/main, integrate sources, rebuild
generated artifacts and repeat applicable checks on the final merge candidate.
Do not leave a mergeable PR open when completing its merge is authorized. -->
