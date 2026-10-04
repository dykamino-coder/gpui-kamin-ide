# WPT engine recovery and completion

The objective is to pass every supported reftest that does not require
JavaScript, using correct HTML/CSS behaviour rather than test-specific rules or
a relaxed comparison threshold. JavaScript integration is a later milestone.

## Starting snapshot

- Published development base: `a0a21dc` from `origin/main`.
- Recovered local engine history: `8378981`.
- Integration candidate: `bd4791c`, a clean merge of the two histories in an
  isolated worktree. Existing runtime fixes and release versions are retained.
- Historical report `rep-all-v229`: 23,108 pairs, of which 2,991 were marked
  outside scope; 17,226 passed the legacy layout threshold and 2,891 failed.
  These are historical diagnostic numbers, not an exact WPT pass count.

## Execution order

1. Preserve unfinished work and verify the recovered build against a small
   control set. Review each patch independently before applying it.
2. Establish an exact pixel oracle. Keep the old threshold only as an explicit
   layout diagnostic; a low percentage must not imply an exact pass. Validate
   screenshot dimensions and report maximum channel difference and pixel count.
3. Resolve the unfinished text/ruby, vertical layout, flex/grid and paint-order
   mechanisms. Compare base and candidate on the same ordered pair list.
4. Run a fresh corpus sweep, recording source SHA, binary hash, comparison mode,
   pair-list hash and environment. Exclude JavaScript requirements explicitly;
   failed screenshots and unsupported behaviour remain failures.
5. Group failures by shared mechanism, implement bounded changes and verify
   affected families plus independent controls. Repeat until the corpus passes.

## Acceptance and resource limits

- Build the runner with `cargo build --example wptrun`, without a package filter,
  to retain the application's GPUI feature set.
- Do not build while a corpus sweep is running. Start with one runner and avoid
  concurrent workloads that can produce empty screenshots or stale frames.
- Never compare a freshly selected slice directly with a full-sweep baseline;
  render both candidates on that exact slice and in the same order.
- Preserve source patches and private diagnostics outside public checkouts.
- Validate common GPUI/Taffy changes broadly; passing one HTML pair does not
  establish that the application's layout remains correct.
- Follow the repository's applicable checks and PR gates. A candidate, a local
  merge or an unverified patch does not count as an accepted implementation.

The older family maps and scout plans are starting hypotheses. Their failure
counts must be regenerated against the recovered candidate before prioritising
new work. No final corpus improvement is inferred by adding slice improvements.

## First measured checkpoint

The identical ordered tab/ruby control list contains 22 pairs: 20 without
JavaScript and two script-dependent exclusions. Exact passes increased from
8 to 11, with three gained passes and no lost passes. The newly passing pairs
are `tab-size-block-ancestor`, `tab-size-inline-001` and `tab-size-inline-002`.
This is a control-slice result, not a corpus-wide improvement claim.

The changes preserve fractional numeric tab sizes, apply inline tab intervals
from a shared block origin, cancel inherited lengths when overridden by a
number, and use the selected monospace family. `ic` spacing now uses measured
ideographic advances. Some remaining failures have larger pixel differences;
their captures remain available for investigation.

The HTML library run produced 170 passing tests and one failing coverage audit.
That audit's consumer list omits newer engine modules, while some reported
fields also require an actual implementation audit. The failure remains open;
the suite and integration are not declared accepted. The recovered build also
has pre-existing warnings that must be addressed before its applicable gates.

Next: use the actual space advance rather than a zero-glyph approximation for
numeric tabs, check minimum tab advances and text indentation, broaden the
control set, and resolve the coverage audit without suppressing its assertion.

The shaped-space measurement subsequently raised the same control slice to
12/20 exact passes. The fourth gained pair is `tab-size-integer-004`. The HTML
library then had 174 passing tests and the same unresolved coverage-audit failure.

## Scale of the next cycle

The target is approximately 1,000 additional passing pairs in a substantial
engine cycle. Small local fixes are checkpoints, not the throughput target.
Prioritise mechanisms that affect whole layout families and verify their gains
on identical ordered base/candidate lists.

Historical v229 diagnostics identify 590 failures among 1,617 eligible
fragmentation pairs and 807 among 4,047 eligible axes/sizing pairs. These groups
are hypotheses for prioritisation, not current exact results or promised gains.
The next baseline covers fragmentation (`css-break`, `css-multicol`, `css-page`);
the next broad set covers grid, flexbox, sizing, writing modes and alignment.

Neighbouring Blink, Servo and Blitz source checkouts are independent references
for disputed algorithms. Their implementations do not override specifications
or replace verification of this engine's output. Use subagents selectively for
independent research; keep resource-sensitive builds and rendering serial.
