Title: docs(html): record unoptimized browser-depth frame measurements

## Task

INC-2026-0084; owner decision on 2026-10-30 requires the #430 depth contract on
8 MiB stacks at both opt-level 0 and opt-level 2. This diagnostic checkpoint
does not complete the engine fix or close the incident.

## PR type and scope

Diagnostic documentation checkpoint on the assigned implementation branch.
Only the incident and investigation/handoff notes change.

## Summary

Reproduce the original-main overflow and identify multicol prepaint as the
first failing phase. Record seven-kind native measurements, the 8256-byte
recurring prepaint chain and compiler-confirmed owners. Compare browser
mechanisms and trace the product shell's CEF path. Preserve baseline artifacts
for continuation. No engine implementation is included.

## Validation

Original-main agent binary compiled; the exact depth regression overflows.
Existing isolated probes passed 33/35 kind/phase endpoints at depth 1026;
multicol prepaint/paint endpoints overflow. Eight private original-main wptrun
self-pairs passed. Formatting, conventions, layer-import, transitional,
whitespace and local PR metadata checks passed for this documentation
checkpoint; details are in progress.md.
Full agent/dev suites, clippy and before/after RGB acceptance remain outstanding
because no engine fix has been implemented. Diagnostics are not acceptance.

## Risks and limitations

No product behavior changes. The driver required ending the current cluster
at tool-call 360/400. No opt-level 2 per-level result or release before/after
timing is claimed at this checkpoint. Already-running dev/release baseline
continuations need final inspection. Shell findings are source/dependency
analysis, not a live opt-level 0 test. The original overflow remains unfixed.

## Versioning

Release versions: unchanged

No batch release is needed for this documentation checkpoint.

## Evidence and acceptance

Frame methodology, browser comparison and measurements: `agents/deepframes/design.md`.
Run record: `agents/deepframes/progress.md`.
The incident remains open; no READY marker or zero-loss RGB claim is justified.

## Dependencies and handoff

Follows #430 and incident registration #557. The lead opens and merges the PR;
the executor commits and pushes this checkpoint with plain git. The lead owns
continuation of implementation and gates. Fix PR remains for the lead.
