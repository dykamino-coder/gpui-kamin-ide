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

The recovered `0625c2d` baseline completed all 1,714 ordered fragmentation
pairs: 97 script-dependent exclusions, 724 exact passes and 893 failures among
1,617 eligible pairs. This supersedes the historical tolerance-based family
count for prioritisation. A candidate separating used box size from visible
overflow extent has five passing sizing tests; its corpus result is pending.

A second candidate addresses independent fragment-root pixel rounding. The
captured `multi-line-row-flex-fragmentation-003` failure contains exactly 125
red pixels at device x=103, y=62..186, where the reference is green. The existing
phase correction applies only to parallel flex items; ordinary block copies
still round from their own zero. Generalising that origin correction requires
its own build and identical-list comparison after the sizing sweep finishes.
The implementation uses the existing Taffy `set_root_origin` mechanism through
a separate-root layout API, rather than changing the box tree with padding.
This preserves intrinsic measurement state and applies both physical axes.
The API and its column-stack integration are prepared but not yet compiled or
render-validated. Shared GPUI changes also require application validation.

The sizing sweep exposed `out-of-flow-in-multicolumn-015` as a regression:
reapplying explicit height after measuring positioned descendants discarded
their contribution to balancing before a spanner. Explicit height is already
resolved in the primary measurement pass. The next candidate applies only
min/max in the shared constraint pass and adds a regression test for that
positioned extent. This revision is pending compilation and comparison.

The first sizing comparison completed all 1,714 pairs with identical pair order:
727 exact passes, 890 failures and 97 script exclusions. Gains are
`abspos-in-clipped-overflow-print`, `multi-line-row-flex-fragmentation-063-print`,
`out-of-flow-in-multicolumn-005` and `overflow-clip-010`; the loss is
`out-of-flow-in-multicolumn-015`. The net +3 candidate is not accepted while that
regression remains. Build and check the revised constraints and origin API next.

The revised engine and origin API compile; all six focused sizing tests pass.
GPUI's dependency unit tests cannot run from the root workspace because GPUI
is not a workspace member. A private standalone manifest referencing the same
vendored source and patched Taffy is compiling the new root-origin tests.
Keep its result distinct from application validation and the WPT comparison.

Both GPUI root-origin tests passed. The new API initially fixed six seams but
exposed three nested-column seams because clipping masks retained fractional
edges while boxes rounded to device pixels. Fragment masks now round their
absolute edges with the same rule; both focused mask tests pass. The identical
16-pair smoke list improved from 5 to 13 exact passes (+8/-0), including recovery
of the spanner and nested-column controls. This is not a corpus result. The
combined revision is undergoing the full 1,714-pair fragmentation comparison.

During that sweep, five previously exact flex-fragmentation controls developed
one-device-pixel positioned-child tails (63, 63, 63, 25 and 62 differing pixels).
The next revision preserves unrounded static probes and applies positioned
subtree translation before device-pixel rounding. Its new layout test checks
four display scales, negative origins, descendant-cache invalidation and
unchanged intrinsic/containing-block layout. Compilation and rendering remain
pending until the current frozen-binary sweep finishes.

The independent coverage audit confirms missing consumer registrations for
anchor positioning, motion paths and zoom, and genuine resolution accessors
for logical-side precedence and font-relative lengths. Those registrations
are restored without treating setters as readers. `justify_items_safe` and
`justify_items_last` still have no runtime readers; `font_kerning` is redundant
metadata beside the live `kern` feature path. The audit must continue reporting
these fields until the implementation or redundant state is corrected.

The combined sweep was invalidated by window minimization: captures for rows
793–837 included 181×24 title-bar images. The runner was restored and then
stopped after 925 recorded rows; no complete-corpus gain is accepted from it.
The first eight substantive flex regressions are positioned-child pixel tails;
`float-009` is another substantive rounding regression. A new 138-pair ordered
control list includes discovered gains, losses and the original smoke controls.
Both sides retain all captures so their actual viewport dimensions can be checked.

The runner now disallows ordinary resizing/minimization and its capture helper
rejects minimized frames while requesting restoration without foreground focus.
It preserves the existing pixel oracle and settling thresholds. The positioned
revision builds; all three GPUI geometry tests pass. The HTML library has 182
passing tests and the coverage audit still fails on exactly the three fields
listed above. Build warnings and application validation remain unresolved.

The positioned revision completed the 138 controls at 134 passes versus the
baseline's 29 (+106/-1). Replacing BandFlow's integer-origin/fractional-child
correction with the same absolute-root API restores `float-009`: the revised
control run finishes at 135/138 (+106/-0). Both runs retained 276 captures,
all 1000×750 device pixels, with verified pair order. This selected control
result must not be added to or presented as the complete fragmentation result.

A forced-minimization probe confirmed that the capture helper restores its
window automatically without changing the foreground during restoration.
The next baseline binary keeps the engine at `0625c2d` and applies only this
capture/window protection, so both sides use the same healthy capture path.

The guarded baseline calibration completed all 138 pairs with every verdict
identical to the previous baseline (29/138), and 276 full-size captures.
After the BandFlow change the HTML library again had 182 passes and the same
three-field coverage-audit failure. The guarded baseline is now rerunning the
full ordered 1,714-pair fragmentation list; the frozen BandFlow candidate is
prepared for the matching full run. Neither full result is accepted yet.

While the guarded baseline renders, a separate next revision makes
`font_kerning` the canonical inherited state rather than mixing it into the
font-variant feature vector. It resolves before low-level feature settings,
survives text-only and first-line styling, and resets through the font
shorthand. Five focused tests are prepared but not yet run; avoid resource-heavy
compilation during the live WPT sweep. Horizontal kerning inheritance and
precedence are the immediate scope; upright vertical `vkrn` and the other
font-shorthand reset properties still require separate implementation checks.
This source revision is not in the frozen fragmentation candidate binary.

The whole local corpus inventory scanned 50,080 HTML/XHTML/SVG documents and
found 25,206 declared reference edges. 20,412 positive edges have no declared
script/variant in either document, with 361 edges missing from the legacy list.
276 source documents for those edges are absent as tests from that list, but
some are reference fixtures; WPT filename/path classification remains necessary.
There are also 327 static mismatch-only documents and 31 script-free test
documents with scripted references. The runner currently classifies only the
test source and its primary positive-reference path needs a negative-oracle audit.
Eight reference URLs require query/fragment semantics. These are scope/protocol
gaps, not passing tests or engine gains. The private inventory's projected pair
lists are not yet executable acceptance lists; preserve their full URL metadata
and establish authoritative test classification before expanding the corpus gate.

The vendored upstream WPT manifest classifier in the adjacent Servo checkout
classified all 24,849 source documents with declarations without errors:
24,445 reftests, 388 print reftests and 16 other/support documents. Reference
filenames alone do not exclude a document from being a reftest. Its serialized
items yield 25,450 relations, including 615 negative relations; 20,974 relations
are script-free candidates (20,404 positive and 570 negative). This broader
count includes variants and is not comparable directly to the earlier raw
pair count. Preserve all 305 URL-state relations, 1,050 relations carrying fuzzy
metadata, nine external references and 64 missing references as explicit data.
None of these inventory numbers are execution passes or engine gains.

A separate next runner revision implements exact negative-reference results
through the same validated RGB comparison. Equal RGB images fail a negative
reference; a one-channel difference passes it, while alpha-only differences,
invalid dimensions and invalid buffers cannot produce a negative pass. All
eight standalone comparison/relation tests pass. Missing or unavailable
negative captures now fail explicitly. Runner compilation and live negative
controls remain pending; blank-page heuristics, declared fuzzy ranges and full
URL/reference-graph semantics still need protocol work. The frozen baseline
and fragmentation candidate exclude this revision to keep their comparison
independent of oracle changes.

The guarded full baseline completed all 1,714 rows at 724 exact passes,
893 failures and 97 exclusions, with every verdict identical to the earlier
healthy baseline. All 3,206 unique retained PNG files have the expected
1000×750 device dimensions and every eligible test/reference capture name
is present. Duplicate test stems cause 28 test/reference image-name collisions;
unique files must not be described as independently retained images for every row.
The matching frozen BandFlow candidate has started its full comparison.

The next kerning revision now passes all six focused tests through the real
variable/importance-aware cascade, including source order against `font` and
`!important`. The complete HTML library has 188 passes and one coverage-audit
failure on `justify_items_safe` and `justify_items_last`. Five private live
runner controls also passed their expected positive/negative/mixed/missing
reference verdicts, with all ten captures at 1000×750. These are protocol
checks and do not contribute to engine or corpus gains. The rebuilt runner
still has the inherited warnings, so the quality gates remain unfinished.

The frozen full fragmentation candidate completed all 1,714 rows at 853 exact
passes, 764 failures and 97 exclusions versus the guarded baseline's 724
passes. Pair order and all 3,206 unique capture dimensions were verified.
There are 131 gains and two losses: `tall-line-in-short-fragmentainer-001`
(125 differing pixels) and `multicol-fill-auto-004` (250 differing pixels).
Both losses reproduced in isolated two-pair runs: the frozen baseline passes
both, and the frozen candidate fails both with the same differences. These
are real rounding seams, not failed/minimized captures. The revision is not
accepted while they remain. The net +129 result is a measured family result,
not the all-corpus milestone or the requested order-of-1,000 target.

The owner additionally requested refreshing all libraries and checking
compatibility. Plan 106 tracks that work and its separate dependency generation.
Keep the frozen fragmentation evidence independent of dependency changes.

The next geometry revision retains raw absolute origins and sizes in FlowRow
and horizontal ColumnStack until device rounding, and snaps painted column
rules to the same absolute edge grid as content and masks. FlowRow's element
implementation is extracted into a small module. Compilation passes in the
updated dependency workspace; live verification is pending. A 214-pair control
list includes all 131 frozen gains, both losses and 81 sampled baseline passes.
It completes at 213 exact passes and one remaining loss, with all 426 PNGs at
1000x750 and no new losses relative to the frozen candidate. The column-rule
seam in `multicol-fill-auto-004` is fixed; the tall inline-block remains one
device pixel too wide. The selected set is a regression gate, not a corpus count.

An initial attempt queried raw layout during paint and aborted before captures;
that run is invalid. ColumnStack now stores its raw bounds in PrepaintState.
The next revision gives Paragraph's atom placement the raw paragraph origin,
avoiding another round through its already-rounded parent bounds. It compiles
and passes both isolated loss controls, with all four captures at 1000x750.
The frozen raw-atoms binary completes the full same-list 1,714-pair comparison
at 876 exact passes, 741 failures and 97 exclusions: 152 gains and zero losses
against the guarded baseline. Pair order and all 3,206 unique capture dimensions
are verified. The existing 28 capture-name collisions remain an evidence
retention limitation. Both previous geometry losses now pass. This measured
fragmentation-family gain does not establish the whole-corpus or order-of-1,000
milestone. Dependency-only old/new controls
also complete separately with the same pre-update geometry: all 214 verdicts
and all 426 unique capture pairs are byte-identical, preserving the separation
between dependency effects and geometry fixes.

The next broad baseline is running on 4,309 ordered pairs across css-grid,
css-flexbox, css-writing-modes, css-sizing and css-align. Its frozen guarded
HEAD binary and list identity are retained, together with hashes of all 6,481
listed HTML input documents. This snapshot does not freeze external assets.
The baseline and same-list candidate must finish serially before reporting a
gain. Negative-reference protocol effects must be attributed separately, and
stem-based capture-name collisions remain an explicit retention limitation.

The owner sets the next sequence explicitly: establish 1,000 new unique exact
passes against the original baseline, then run the full eligible corpus.
Focused family baselines and same-list candidate comparisons continue before
that milestone. Do not count repeated passes, overlapping lists, exclusion
changes or reference-protocol corrections as new engine passes. The currently
confirmed engine gain remains 152; the full-corpus run is still outstanding.


The 4,309-pair axes baseline has completed with process exit code 0:
2,594 exact passes, 1,453 failures and 262 exclusions. The complete verifier
checks pair order, binary/list/input-document identities and all 8,034 unique
capture files, including PNG chunk CRCs and 1000x750 dimensions. Sixty
stem-name collisions mean earlier colliding captures are not independently
retained. All 4,309 input pairs are unique and none overlaps the fragmentation
list. The frozen raw-atoms candidate is now running serially on precisely that
list. Initial candidate captures have the expected dimensions; no candidate
or additive gain is established until completion and attribution checks.


The completed axes baseline contains 222 writing-mode failures with the same
`max=26 total=404` signature. A headless Chrome control at the same 1.25 scale
reproduces all 404 fringe pixels with identical colors. The full 400x400
physical-pixel background region matches our capture exactly after translating
the header offset. The source PNG's red/white boundary is linearly sampled
outside the overlaid solid square; this is not evidence of an engine defect.
Keep the exact baseline verdicts and the +152 engine-gain ledger unchanged.
Do not force nearest-neighbor image rendering, modify fixtures or count these
as new passes. The eventual official WPT harness/capture contract must account
for device scale independently of implementation gains.


The CSS adapter now forwards `justify-items`/`justify-self: last baseline`
through the existing first/last converter instead of dropping the last keyword.
Duplicate conversion is removed and the existing apply tests are extracted to
a small module. Two new mapping controls cover explicit first/last overrides
and vertical-container/parent-grid axis projection. These Rust tests are not
yet executed while the separate GPUI integration check is running. Full
last-baseline sharing on physical x, safety propagation on both justify
channels and live WPT acceptance remain pending; no gains are claimed.


Future runner captures use a one-based pair ordinal before the basename,
retaining the `--ref` suffix. This prevents distinct folders/references from
overwriting earlier PNG evidence. Frozen old runs retain legacy names and
their collision limitations. New private run manifests must explicitly record
`capture_naming: pair-index-v1`; verification derives names from ordered input
rows rather than directory enumeration. The 4309-pair axes list yields 8618
distinct indexed filenames. The new runner source is not yet rebuilt or live
verified; capture naming changes neither pixel comparison nor engine gains.


The frozen raw-atoms engine completed all 4309 axes pairs with exit 0:
2629 exact passes, 1418 failures and 262 unchanged out-of-scope entries,
versus baseline 2594/1453/262. Input order, all 6481 HTML identities and all
8034 surviving PNG files passed verification. Sixty historical filename
collisions remain a limitation of these frozen runs, but none affects the 35
new gains. Every gain has old unequal/new equal RGB evidence and is a positive
relation; there is no overlap with the fragmentation family or protocol gain.
One test passes its declared alternate rather than the listed first reference;
a separate old/new frozen-binary control reproduces that alternate's failure
and exact pass. It counts once for the original pair, not as another new pair.
The confirmed unique engine ledger is now +187/-0: prior fragmentation +152
and axes +35. Remaining to the owner's +1000 checkpoint: 813.


Initial native Taffy 0.14 controls completed 404 ordered pairs with exit 0:
332 exact passes, 71 failures and one unchanged outside entry. All 586 HTML
identities and 806 indexed captures verified with zero naming collisions.
Six improvements and six regressions against the selected prior engine mean
this migration is not accepted. Repeating the six regressions reproduces five;
float-009's single-channel one-level pixel difference disappears on repeat,
so capture stability remains under investigation rather than excluded.

Two reproduced regressions now pass exact RGB after keeping a percentage
image's opposite automatic axis unresolved and respecting the adapter's
projected logical cross-start for safe overflow in RTL columns. The native
library passes 201 tests; removing the latter fix makes its real-layout RTL
control fail. The rebuilt runner's six-pair verification passes image014,
RTL003 and float009, with no losses against that repeated cohort. The two
scroll-baseline cases and external-containing-block safe alignment still fail.
An inline-block baseline-boundary correction is only partially verified.
These restore old passes and add nothing to the +187 accepted engine ledger.
A private grid-container baseline draft passes 206 tests, while restoring the
old algorithm makes all three nested-grid semantic controls fail. It has not
been adopted or accepted by WPT. External-CB static alignment must preserve
its flex alignment area but assess safety against actual containing geometry;
blanket removal of safe would break neighboring outer-CB overflow cases.


The combined native baseline implementation now passes 214 default and 231
parse/serde controls; actual workspace/all-targets checking and runner build
finish with exit 0. Grid baseline selection, scroll boundary synthesis and
empty-table suppression have failing old-code semantic controls. The frozen
106-pair rendering cohort completes with 57 exact passes, 48 failures and one
unchanged outside entry: five fail-to-pass transitions and no losses against
the preceding native candidate. Two transitions restore scroll-baseline
regressions; three repair subgrid auto-fill cases. All 190 HTML identities and
210 indexed PNGs verify, and all five gains have identical RGB captures.
Against the original selected engine, this cohort still has three losses
(animation sampling, float-009 capture instability and external-CB safe003).
These controls alone do not accept the migration or advance the global ledger.
A frozen 4309-pair axes-family comparison is running; no whole-corpus run has
started. The next implementation target is orthogonal inline sizing: the old
142-pair sizing/available-space family has 93 failures, of which vertical auto
and percentage sizing account for 30 and 16 respectively. These are potential
coverage counts, not predicted or accepted gains. Used inline-size must retain
separate definite/fallback constraints and min/max-content measurement.


Ordinary in-flow orthogonal blocks now carry a separate nearest-scrollport
record and inline sizing constraint. Percentage basis and auto shrink-to-fit
space remain distinct; viewport/font units resolve before selecting them.
The rotated element probes real min/max-content contributions and lays out
at the resulting used inline size before claiming physical height. Existing
non-block formatting contexts retain their separate contracts. Three formula
controls pass; four integration controls type-check, but execution is pending.
The final workspace/all-targets locked check passes after all source edits.
HTML library test compilation was intentionally deferred when it required a
GPUI rebuild during the live frozen axes run; it is not a passing test gate.
A 147-pair orthogonal/neighbor list is prepared for exact runtime verification
after the current GUI process finishes. No gains are attributed to this new
source before actual rendering and regression verification.


The matching workspace library test run now completes with exit 0: 247 tests
pass, including all 199 HTML tests and the seven new orthogonal sizing
controls. The correct workspace feature union reuses the already-built GPUI
dependency. Rendering acceptance of this source remains pending. The preceding
frozen native axes process has completed its 1086 writing-modes pairs with
eight improvements and no losses. All eight have old unequal/new identical
RGB evidence across 32 valid PNGs. This is a completed family slice inside a
still-running larger process, not final migration or global-ledger acceptance.


The first orthogonal runner is frozen after a successful workspace-union build.
It excludes subsequent fixed-max flex wrapping, content-box percentage-margin,
and grid-lanes geometry repairs; its capture results cannot verify those later
sources. The fixed-max wrap regression now has three actual native-tree controls:
the old implementation fails content-derived known height with an independent
maximum, while definite-height and no-maximum neighbors remain unchanged.
The repair passes 217 default and 234 parse/serde native tests and workspace
checking; rendering acceptance is still pending.

The larger native axes comparison reveals new grid-lanes regressions as native
regular-grid references improve. Absolute containing blocks still included the
preceding gutter, and reverse safe stacking subtracted unclamped negative free
space. Actual tree tests reproduce both mechanisms (three failing controls,
one passing end-edge neighbor); the bounded repairs pass 240 parse/serde native
tests, including first/final grid-line and unsafe-center controls. A separate
geometry helper keeps the existing lanes module smaller. Actual workspace
library testing is running, and the next exact 573-pair candidate cohort combines
orthogonal sizing, migration retention and every observed new loss. The accepted
ledger remains unchanged until rendered gains and zero-loss retention verify.


The native axes comparison is now complete (exit 0): 2659 exact passes,
1388 failures and 262 unchanged outside entries across 4309 pairs. Compared
with the accepted raw-atoms engine it gains 62 pairs and loses 32; migration
acceptance is rejected. All 6481 input HTML identities and 8094 indexed PNGs
validate without capture-name collisions. The repaired orthogonal/flex/lanes
source passes the actual workspace library suite: 247 tests, including 199
HTML tests. Its workspace/all-targets check and runner build are in progress.
The earlier orthogonal-only frozen binary is running the 147-pair sizing
cohort independently; its results cannot verify later flex/lanes repairs.
The next repair cohort now includes every loss from the completed axes run.


The first orthogonal-only cohort completes with 67 exact passes, 79 failures
and one outside entry: +20/-14 against the previous native engine. It is
rejected pending regression repairs. Its 259 input HTML identities and 292
indexed captures validate. Image inspection finds two latent reference-engine
failures exposed by the new correct in-flow sizes: absolute vertical auto boxes
collapse to borders while painting overflowing text; authored display:block
on td is still treated as a real cell and stretched to the row height. Intrinsic
absolute inline claims now have two sizing-contract controls; table cell-role
selection and anonymous-row fixup have two controls. Actual final-source tests,
runner compilation and WPT verification of both repairs remain pending. The
already-running 579-pair flex/lanes candidate excludes these two later changes.


The 579-pair orthogonal/flex/lanes repair cohort completes with exit 0: 420 exact
passes, 157 failures and two unchanged outside entries. It gains 37 pairs and
loses 30 against its selected original baseline, so it is not accepted. All
input identities and indexed PNGs validate. The two lanes geometry repairs and
fixed-max flex wrapping restore 17 losses from the full native axes comparison;
all 17 have old unequal/new identical RGB proof across 68 PNGs. These are
restorations, not additive gains. The earlier orthogonal 20 gains have RGB
proof across 80 PNGs but its 14 losses remain in this frozen cohort. Absolute
inline-claim source tests pass in a 249-test historical suite (201 HTML); that
compilation predates the table-role edits. Final combined-source library tests,
workspace checking and runner build are now running sequentially.


### 2026-10-05: orthogonal absolute and table-role validation

The frozen candidate completed 579 ordered pairs: 446 exact passes, 131 failures,
and two pre-existing script exclusions. Against the preceding candidate it
restored or gained 26 pairs without losing a pass. A separate RGB comparison
validated all 104 before/after images; the capture/input validator checked 895
HTML inputs and 1,154 indexed PNGs with no collisions. Workspace library tests
passed 251 cases (203 HTML), followed by a successful workspace check and runner
build. This cohort still has 16 migration losses against the original engine;
it does not replace the accepted +187 ledger or justify a whole-corpus run.

The next candidate preserves ratio-derived preferred sizes independently from
content-expanded used sizes, and passes positioned intrinsic sizing keywords
through GPUI to native layout. Native regression controls changed from one pass
and three failures to four passes; the complete native serde/parse set passed
247 cases. Workspace integration and pixel verification remain pending.

### 2026-10-05: ratio and baseline checkpoint

The ratio/intrinsic candidate completed 579 controls with exit 0: 450 exact
passes, 127 failures and two unchanged exclusions. It restored four original
passes without losing any relative to the preceding candidate. Before/after RGB
proof covers 16 images; input identities and all 1,154 indexed PNGs validate.
Relative to the original selected baseline it has 49 gains and 12 losses. These
gains remain provisional. Workspace check, runner build and 253 library tests
(205 HTML) passed for this frozen candidate.

Subsequent lanes row-baseline intrinsic sizing and physical fallback repairs
passed 254 native serde/parse tests, workspace checking and 253 workspace library
tests. Their rebuilt runner awaits pixel verification. GPUI is excluded from the
workspace, so its two adapter sizing tests must run in an independent test project;
these tests have not yet executed.

### 2026-10-05: lanes baseline pixel verification

The subsequent baseline candidate completed 579 controls with exit 0: 458 exact
passes, 119 failures and two unchanged exclusions. Eight report transitions have
old unequal/new identical RGB proof across 32 images and no losses relative to
the preceding candidate. One transition is an unstable animation and is not
attributed to these fixes. The seven static transitions restore three original
passes and add four provisional gains. Input identities and all indexed PNGs
validate. Eight original migration losses remain; the accepted ledger is unchanged.

Replaced image used-style normalization passed 256 workspace library tests
(208 HTML) and workspace/all-target checking. Its runner build is in progress.
An expanded 1,856-pair sizing/replaced family baseline is now running on the
original frozen engine. Native positioned fit-content controls also pass for
both block and flex containing blocks, including content overflow and negative
automatic margins. Inspection finds that an old HTML holder intercepts authored
positioned sizing keywords before the native path; retiring that holder for
authored size keywords is prepared but not yet adopted.

### 2026-10-05: native keyword and wrapper contracts

The image-only candidate passed workspace checking, its runner build and 256
library tests (208 HTML). It is frozen before subsequent edits. The positioned
keyword holder has now been retired for authored size keywords; holders for
auto sizes constrained by intrinsic max sizes remain. The native control verifies
fit-content and explicit height produce the same 540px size and 180px offset
under both block and flex containing blocks.

An orthogonal percentage child of an auto-height ratio parent incorrectly used
viewport height; ratio-derived preferred block sizes now supply that basis while
preserving content-expanded used sizes. Sizing wrappers now carry item baseline
orientation and projected self alignment/safety from the effective inherited
style. In grid lanes, item orientation is derived from lane-parent writing mode
as well as ordinary grid-parent metadata. Combined integration tests are running.

The independent GPUI project executed both native sizing adapter tests successfully.
Its full library run passed 75 of 77 tests, including root-origin and alignment
controls; legacy word-character and all-space wrapping assertions failed. The
production line-wrapper source is unchanged from owned HEAD, and its Unicode
line-break dependency version is unchanged. This is evidence of existing test/
behavior disagreement, not a fully green GPUI library suite. Replaying the old
test file against newer Font/TextRun fields fails to compile; the original current
private test source was restored. No assertions were weakened or removed.

### 2026-10-05: expanded sizing baseline and next content-paint fixes

The combined keyword, orthogonal-ratio and wrapper candidate passed 260 workspace
library tests (212 HTML), workspace all-target checking and the runner build.
Its expanded 1856-pair GUI measurement is running against a completed original
baseline of 1226 exact passes, 628 failures and two unchanged exclusions. Partial
reports are not accepted ledger increments. Collision-free indexed candidate
captures and input/resource snapshots are preserved for transition validation.

A subsequent source candidate preserves an SVG's CSS shell when content has zero
area, includes explicit object-fit fill in the shared paint path, and positions
native image content using final fitted bounds rather than requiring both authored
pixel dimensions. Workspace all-target checking passed; three independent native
image-position tests passed, including cover overflow and mixed pixel/percentage
offsets. Workspace library verification and its runner build are in progress.
Pixel acceptance of these subsequent changes is pending. The legacy GPUI full-suite
two failures remain open; no assertions or exact comparison criteria were relaxed.

The expanded run completed with exit zero: 1279 exact passes, 575 failures and two
unchanged exclusions. Ordered validation checked 2482 input documents, all 3708
indexed PNGs and 2633 snapshotted local resources. All 55 gains and two losses
against the original baseline were independently proved by old/new RGB captures;
none had an ambiguous legacy stem. This candidate remains unaccepted because
safe outer-CB placement and one indefinite grid-lanes baseline case regressed.

A subsequent implementation applies abspos safe alignment to the union of actual
flex static-position bounds and the original containing block during prepaint.
Authored cross-axis insets, automatic margins and unresolved parent percentage
padding retain their previous paths. Numeric union-placement controls and parent
writing-mode projection tests are added; integrated tests and WPT acceptance are
pending. This is a bounded correction, not a claim of complete abspos safety.

The subsequent source passed 263 workspace library tests (215 HTML), including
both abspos overflow-limit math controls and writing-mode/inset projection; the
workspace all-target check passed. The runner build remains in progress. The
independent GPUI full serial library suite passed 78 of 80 tests; the two previously
observed line-wrapper assertions still fail. A parallel run also failed the Windows
clipboard test once; it passed in the serial full suite. Its cause is not isolated,
and no assertions were changed. Focused WPT pixel acceptance remains pending.

The runner build completed successfully (session 16477). Frozen binary
`ee0f4a073dc101cb45deb1298743f5ca36b2da3ccca280d90937e96bdff37b2c`
completed the 48-pair replaced-content/safe control slice (session 47816, exit 0):
41 exact passes and 7 failures. All 96 captures, 73 HTML inputs and 82 local
resources were validated. Mapped RGB comparisons against the preceding expanded
candidate show 21 fail-to-pass and 2 pass-to-fail transitions. Sixteen SVG fill
cases, two object-position cases and zero-height SVG containment now pass.
The external-CB safe case 003 also passes; float-009 passes in this shorter
sequence but is not attributed as an engine gain because the frozen original
already showed sequence-dependent one-pixel output.

Two new safe static-position failures have a flex parent that is itself the
original containing block. That geometry is already available to native layout;
the deferred external-CB union adapter is now restricted to distinct containing
blocks. Its projection control covers this boundary. Rebuild and WPT validation
of this correction remain pending. The indefinite grid-lanes case still differs
by 26 pixels. The focused sequence differs from the expanded baseline, so none
of these provisional transitions has been added to the accepted +187 ledger.

The self-CB boundary projection test passed after the correction; the workspace
all-target locked check also passed. Session 18259 is building the next runner.

Session 18259 completed successfully. Frozen corrected binary
`e53d1645974076472810a7bedbddf9a72c8f1e3a526f9f3fd3612d966ccf4e93`
completed the same ordered 48 controls (session 7557, exit 0): 43 passes and 5
failures. Both new safe-static-position regressions were restored with independent
old/new RGB proof; no other control verdict changed. Against the expanded previous
candidate this focused slice has 21 gains and no losses, including float-009's
unattributed sequence effect. A same-ordered 1856-pair expanded run is prepared.

An isolated native Taffy contract suite passed two tests covering eight cases:
block content-height keywords retain indefinite percentage descendants (and fixed
height is a positive control), and intrinsic widths account for padding/border
exactly once under both box-sizing modes. These establish bounded native contracts
before ordinary intrinsic-wrapper migration, not complete native sizing coverage.

The next source implements authored vertical inline intrinsic sizing independently
of the viewport cap: physical height min-content/max-content/fit-content selects
the corresponding horizontal-text contribution before rotation, then reports
that length as the rotated leaf's height. Numeric min/max constraints still apply;
auto/fixed heights and size containment keep their previous paths. Authored
intrinsic measurements use their freshly measured column extent instead of a
cached extent from a previous allocated frame. VerticalText builder methods move
to a separate small module so the oversized interaction module shrinks.
Two new tests cover mode selection, min/max precedence and excluded contexts.
Full workspace tests/check and a runner build are pending in session 22968;
pixel gains are not established. The earlier frozen expanded candidate continues
in session 98477 and is unaffected by these subsequent source edits.

The vertical intrinsic source passed all 265 workspace library tests (217 HTML),
including both new mode/containment controls. The locked workspace all-target
check passed; session 22968 is now building the runner. Its bounded WPT control
list has 98 pairs; no pixel result is available yet.

The isolated native intrinsic contract suite now passes five tests. Additional
controls cover fit-content length/percentage arguments, cyclic inline percentage
contribution versus final used child width, negative and automatic margins, and
explicit second-row/second-column placement of intrinsic grid items. The cyclic
test checks both the unchanged max-content parent width and the child's resolved
percentage width, following CSS Sizing 3 §5.2.1; it does not demand that the used
child width equal its intrinsic contribution. Source:
https://drafts.csswg.org/css-sizing-3/#intrinsic-contribution.

The isolated native contract suite additionally passed first/last text-baseline
export through intrinsic block items in a real two-column grid. Both min-content
and max-content item widths preserve the expected baseline alignment offsets;
all six native contract tests pass. This supports wrapper retirement but does
not prove vertical x-baseline, flex, or grid-lanes migration coverage.

Session 22968 completed the vertical intrinsic runner build with exit 0. Frozen
candidate `c2c5d96e9f2bafdaf452235ee509d9886adb2b6fa8d70a9cbbe10d7f6ec86fdb`
and the previous frozen `e53d1645...` engine are prepared for identical ordered98
control runs, with 163 HTML inputs and 174 referenced resources snapshotted.
Both runs still need execution; the active 1856 GUI run retains serial priority.

The subsequent source retires anonymous intrinsic-sizing grid wrappers for
ordinary non-replaced block boxes, including their flex/grid/grid-lanes item
roles. A single element eligibility predicate controls direct native preferred
size keywords, margin/placement hoisting, percentage pre-resolution and wrapper
creation. Keyword conversion is shared with the existing positioned-box adapter.
Rewritten intrinsic min/max constraints carry an explicit internal wrapper marker;
their existing shim remains. Specialized form/table/list builders, floated boxes,
size containment and calc-size retain their earlier contracts. Two new controls
cover direct eligibility, wrapper preservation and excluded roles. The normal
orthogonal measure path again calls its non-keyword resolver, avoiding the new
unused-method warning without changing its sizing arithmetic.

Workspace tests/check/build are pending in session 1481; no native-migration
pixel gain is established. The bounded migration control list contains 555 unique
pairs selected from the existing sizing/axes/fragmentation inventories, plus the
98 previous controls; textual selection may miss declarations in external CSS.

The previous frozen expanded candidate completed session 98477 with exit 0:
1299 exact passes, 555 failures, 2 unchanged exclusions. Against the original
1856 baseline it has 74 gains and 1 loss; all 75 transitions have independent RGB
proof, no stem ambiguity and no animation-name attribution exclusions. All 2482
HTML inputs, 3708 captures and 2633 referenced resources were validated; three
missing resources remain unchanged. Against the preceding expanded candidate,
20 gains and no losses were separately RGB-proved. The remaining original loss
is indefinite grid-lanes case 002 (26 pixels), so the accepted ledger remains
unchanged. Both same-ordered98 vertical runs are prepared; previous-engine
execution is active in session 93223.

Native wrapper retirement passed all 267 workspace library tests (219 HTML) and
the locked all-target check. Runner build remains active in session 1481. Its
same-ordered555 previous-engine baseline uses frozen vertical candidate c2c5d96e,
with 836 HTML documents and 861 local resources snapshotted; two missing resources
are recorded unchanged. No native migration WPT result is established.

A new isolated vertical x-baseline contract exposed missing last-baseline groups
in grid track sizing: only first-baseline column groups were calculated. An
independent source copy now separates first/last groups by start/end column edge,
projects their opposite physical sides and measures the actual last x baseline.
All seven isolated intrinsic contract tests passed, including both vertical
orientations and first/last baselines. The prototype is not adopted; full native
library verification is pending in session 25128.

The isolated last-x-baseline prototype passed all 237 native library tests in
session 25128 (exit 0), in addition to seven explicit intrinsic contracts. It
remains outside the owned production source until the current native-wrapper
runner is frozen. The same-ordered98 previous engine run completed session 93223
with exit 0: 79 passes and 19 failures, 196 valid captures and unchanged inputs/
resources. Vertical intrinsic candidate execution is active in session 52581.

Same-ordered98 vertical candidate session 52581 completed with exit 0: 79
passes and 19 failures, zero verdict transitions against previous session 93223.
This does not establish equality of all old/new images. Native wrapper runner
session 1481 completed successfully and binary 88e6a442 was frozen before
subsequent source changes. Its same-ordered555 baseline session 40401 completed
with exit 0: 386 passes, 134 failures, 35 exclusions. Validation checked 1040
captures, 836 HTML inputs and 861 resources with two unchanged missing resources.
Frozen native wrapper candidate execution is active in session 16967.

The independently verified last-x-baseline fix was adopted into the owned source
as grid/baseline_x.rs, extracting the existing oversized track-sizing routine.
The regression contract is now an owned native unit test. Production verification
is pending; this change is not part of frozen binary 88e6a442 and has no WPT gain
claim. The accepted unique WPT ledger remains +187/-0.

Native wrapper same-ordered555 candidate session 16967 completed with exit 0:
394 passes, 126 failures, 35 unchanged exclusions; 12 incremental gains and 4
losses against c2. All 16 verdict transitions were RGB-proved without ambiguity.
The four losses are intrinsic-size-007, fit-content-length-percentage-002/005,
and grid-container-scrollbars-sizing-002. No additive acceptance is claimed.

The adopted grid last-x-baseline extraction passed 238 native library tests in
session 59103; workspace test/check/build pipeline 88771 remains active. Full
RGB comparison of the vertical98 run found 188 identical captures and 8 changed
captures; indefinite lanes002 worsened from 26 to 15836 mismatch pixels. Actual
text measures now expose two missing x-baseline paths: column flex and column
lanes. Isolated contracts reproduce both failures. A private column-lanes group
prototype passed eight contracts in session 7429; native full tests 96120 remain
active. It has not been adopted. Column flex repair remains pending, including
orientation metadata currently limited to grid/lanes. The accepted ledger is
unchanged; no full corpus run has started.

Grid last-x-baseline pipeline 88771 completed successfully: 267 workspace library
checks (219 HTML), all-target check and runner build. Frozen binary da03cb0a is
running the same 555 controls in session 9235, before the lane changes below.
The column-lanes prototype passed all 238 native library tests in session 96120.
It was adopted with the row group routine extracted into lanes_baseline.rs
(111 lines); lanes.rs shrank. Its owned column-lanes regression test is registered,
and native validation is active in session 82001. This adoption is not in da03cb0a.
The column flex test failed in session 95440, confirming missing x groups. A block
fit-content contract failed in session 78245: a 50px argument produces 50px despite
100px min-content and 200px max-content. The required clamp is absent on a custom
inline formatting child; its correction remains pending.

Last-x-baseline same-ordered555 run 9235 completed with exit 0: 393 pass, 127 fail,
35 unchanged exclusions, no gains and one RGB-proved loss (column-fill-reverse-
justify-self-001). Its grid reference now uses real last-x sharing while the frozen
candidate still lacks the lane path adopted afterward. No acceptance is claimed.
Owned column-lanes adoption passed all 239 native tests in session 82001.

Block fit-content and column-flex x-baseline prototypes were implemented and
verified independently. Fit-content uses separate min/max probes, adds content-box
edges once for explicit length/percentage arguments and preserves other sizing
keywords. Two contracts and all 241 native library tests passed (74220/46641).
Column flex groups physical baselines, includes their intrinsic cross contribution,
honors auto-margin exclusion and merges compatible opposite-flow/opposite-preference
groups. Four contracts and all 243 native tests passed (97053/30544). Both prototypes
are adopted with the row routine extracted to a separate module; oversized native
files shrink. HTML item/wrapper orientation metadata now includes vertical flex
children without applying grid alignment projection to them. Normal-flow x export
has a new LTR/RTL row/column relative-inset control. Merged native session 4092 and
workspace test/check/build session 48755 are active. No new WPT runner is frozen yet.

The first full prototype checks failed from exhausted disk/PDB output; disposable
completed Cargo caches were cleared through cargo clean, preserving source, logs,
and frozen WPT binaries/captures. Successful retry sessions above are authoritative.
Bare native indefinite-ratio percentage-height contract passed (82206); the remaining
intrinsic-size-007 regression needs HTML metadata/custom-formatting-path isolation.

Merged native session 4092 completed with exit 0: all 246 tests passed, including
8 flow/direction relative-x export cases. Workspace pipeline 48755 remains active.
The intrinsic-size-007 regression was reproduced with frozen native and legacy
runners: native green box is 100 by 784 CSS px, legacy and reference 100 by 100.
Ordinary divs use a column flex formatting node. Early flex-basis measurement
ignores the authored cross intrinsic keyword and supplies an automatic stretch
width; later cross measurement restores width 100 but cannot undo basis height 784.
The indefinite percentage height is skipped in this HTML path, so this is not
evidence of percentage-height resolution against viewport. A bounded native
prototype will constrain cross keywords before basis measurement.

Workspace pipeline 48755 completed with exit 0: 268 library tests (220 HTML),
all-target check and runner build. Combined fit/flex/lanes binary399c3d5e was
frozen with the same ordered555 list, 836 HTML and861 resources (2 missing
unchanged); WPT session87063 is active against frozen da03. This artifact predates
the early cross-keyword repair prototype. No additive acceptance is claimed.
Set comparison found all74 provisional expanded gains outside the187 accepted
paths, making261 a potential union only; regression validation is still required.

Combined fit/flex/lanes WPT session87063 completed with exit0:396pass,124fail,
35 unchanged exclusions. Same-order555 comparison against da03 proves3gains
and0losses, all3RGB transitions unambiguous. This is incremental evidence,
not an additive accepted gain claim against the original engine. Early cross
keyword/fit clamp repair is still private pending final native checks.

Early cross-keyword repair adopted after final merged native session19068:255
tests passed, exit0. All adopted native files match verified private source after
line-ending normalization. Functional fit-content clamps use separate min/max
probes, border-box contributions, once-only content-box edges and definite
percentage basis. Forced stretch excludes intrinsic keywords while retaining
actual auto/stretch and unresolved percentages. Definite-height200 and central
baseline controls passed. flexbox.rs shrinks12 lines; new modules remain below250.
HTML metadata now preserves synthesized physical x-baseline orientation for
horizontal children in vertical flex parents, matching grid rules; real inheritance
control is new and pending. Workspace pipeline34760 is active; runner is not frozen.
Frozen399c3d5e last-x lane diagnostic remains120pixels different only within the
end label; all item background rectangles match. No criterion waiver or fix claim.

Only7 of the32 rejected native migration regressions are in the555 intrinsic
controls. All32 were collected into a distinct ordered witness list with no
legacy capture-name collisions. Frozen accepted2486 baseline32 is active in
session68499, with59 HTML inputs and62 local resources, none missing. Candidate
execution awaits completed workspace34760 and a new frozen runner. These
controls supplement, not replace, the required broader4309 family validation.

Ordered regression baseline32 session68499 completed with exit0:31 exact passes,
1 exact failure (css-flexbox-height-animation-stretch,15000 mismatch pixels).
The animated pair passed in the older broad family, so narrower-sequence evidence
cannot attribute it as a fresh candidate gain; temporal cause remains unresolved.
Validation checked64 captures,59 HTML and62 resources with no missing inputs.
Workspace34760 remains active; no new runner has been frozen.

Early cross-keyword pipeline34760 completed with exit0:269 workspace tests
(221 HTML), all-target check and runner build. Frozen384a55b6 has the same555
inputs as399c3d5e and a separate same32 regression cohort against2486. Candidate32
session93457 is active;555 run is prepared but has not started. All resource
snapshots are preserved. A private two-test animation contract passed in69601:
resizing only wrapper to50 leaves painted child100, while sampling actual node
heights50/75/100 correctly sizes the flex row and every stretched item. HTML live
animation currently applies dimensions to a wrapper around a fixed-style child.
This engine gap remains open; timing differences are also not waived or counted.

Native migration guard32 candidate93457 completed with exit0:31pass/1fail,
exactly matching ordered2486 baseline68499, with no verdict gains/losses. All
previously passing witnesses remain passing; the animated pair remains failing.
Frozen384a55b6 same555 is active in24360. Wider axes4309 is prepared from the
same frozen engine, with6481 HTML and6669 resources (5 missing), not started.
This existing family is not the full eligible corpus and remains necessary for
acceptance. Live animation now samples onto actual Element declarations rather
than an extra formatting wrapper. Frozen frame overlay is shared unchanged.
Infinite alternate uses two iteration durations and triangular direction rather
than GPUI breathing. Four focused controls are new; workspace pipeline30151 is
active. render.rs shrinks115 lines. No animation/WPT gain is claimed, and full
CSS timing-function, delay and iteration-count semantics remain incomplete.

Frozen384 same555 session24360 completed with exit0:399pass,121fail,35
unchanged exclusions. Four incremental gains and one loss against399c3d5e,
all5RGB-proved. Restored intrinsic-size-007 and fit-content002/005; new green
text-in-nested-multicol. New loss multicol intrinsic-size-004 has124physical
green pixels instead of125: four25px contributions are rounded to24.8px
before intrinsic arithmetic, yielding99.2 rather than100. ColumnStack currently
uses rounded layout_as_root().width. Its arithmetic needs explicit unrounded
root measurement, with existing paint rounding retained. No correction yet.
Broader frozen384 axes4309 is active in36022. Live animation pipeline30151
remains active, before any measurement API changes.

- Live animation sampling pipeline completed: 273 library tests (225 HTML), all-target check and runner build passed. Frozen candidate retains the same exact comparison protocol; WPT verification remains pending.
- Multicol intrinsic measurement now reads unrounded layout sizes through an explicit root API before column arithmetic. Paint rounding is unchanged. A scale/origin invariant test covers scales 1, 1.25, 1.5 and 2; native GPUI tests and workspace pipeline are running. No new WPT gain is accepted yet.
- Unrounded intrinsic measurement verification: four native GPUI root-origin tests passed; workspace library tests and all-target check completed successfully. Runner build and exact WPT evidence remain pending.

- Unrounded multicol candidate completed workspace tests/check/runner and was frozen before subsequent changes. WPT evidence remains pending.
- Five new definite vertical child-content tests passed; workspace library total 278. The inherited containing-block size is clamped by parent limits before child edges/margins are deducted, and child limits remain their own. Percentage edges use logical inline CB size. Indefinite parent intrinsic used-size propagation remains open.
- Horizontal orthogonal ordinary auto blocks now pass available space to native functional fit-content, retaining min-content floor and authored limits. Explicit and omitted auto share this policy. Three boundary tests accompany the extracted builder; combined workspace/check/runner pipeline is running. No WPT gain claim yet.
- Current standalone GPUI full-library check: 78 passed, three failed. Two legacy wrapping expectations conflict with existing production CSS wrapping changes; native clipboard validation also failed during an active GUI run. Dependency acceptance remains incomplete.

- Definite-child and horizontal fit changes completed 281 library tests (233 HTML), all-target check and runner build. The combined candidate is frozen. CSS/editor wrapping policies are now separated: seven dedicated native tests passed; native library check passed 82 tests with the unsafe interactive clipboard test filtered. Owned workspace tests/check/runner completed successfully again.
- Clipboard-owner prototype remains private and unaccepted. The isolated station diagnostic fails at OpenClipboard with access denied before data transfer; this does not yet validate or reject the proposed production ownership fix.
- Broad 4309-pair comparison remains active. Intermediate transitions include real table geometry regressions. Paragraph constraint selection now excludes the inherited generic containing-block fallback for parallel table-column intrinsic probes; a boundary test was added. Workspace validation is running. No additional WPT gain is accepted.

- Broad axes completed with authoritative exit zero: 4309 pairs, 2722 pass, 1325 fail, 262 unchanged exclusions. Compared with the accepted raw-atoms baseline, 120 gains/27 losses; RGB proof validates 115 gains and all 27 losses. Five gains are ambiguous due to legacy capture stem collisions. This candidate is not accepted. Fresh same-order 147-transition baseline completed: 26 pass, 121 fail; animation alone drifted between broad and focused sequence.
- Read-only grid investigation found bare fit-content metadata absent on the content-sized anonymous grid wrapper, allowing flex cross stretch to replace intrinsic width with parent width. Functional fit arguments need a separate single-owner contract. Vertical auto-grid zero-height regression requires identifying the original compensation before changing general VerticalText claims.

- Table/content-inline candidate completed 282 library tests and exact 147-transition comparison: 112 gains and seven losses against fresh accepted-engine baseline. Nineteen static broad losses restored; eight new broad gains in horizontal orthogonal sizing were lost. No global acceptance.
- Intrinsic sizing wrapper extracted from giant renderer (net shrink), preserving bare horizontal preferred keyword. Workspace 283 tests/check/runner completed. Same 147 controls remain 112 gains/seven losses, and sizing-002 actual/reference pixels are unchanged. The wrapper native contract passes but does not yet explain the HTML-specific failure; further actual builder evidence is required.
- Horizontal lost gains traced to main-axis flex fit sizing: a wrapped-line width replaces the fit argument. Private native regression reproduced390 instead of400; independent min/max clamp restored400. Sixteen configurations cover rows/columns, content/border-box, minimum floor, maximum cap, percentage, bare fit, and content/keyword flex-basis precedence. Private Taffy full library with permanent boundary tests passed257; resolver and tests adopted, giant flexbox net shrinks.
- Vertical mixed flex empty-item synthesis lacked the central-baseline bit when the item itself is vertical. Native model with flags5 produces150x100; flags7 produces100x100. Metadata repair and boundary test adopted alongside main-fit resolver. Workspace/check/runner pipeline is active; no additional WPT evidence yet.

- Main-fit/central candidate completed 284 library tests (236 HTML), all-target check and runner build; focused147 completed with112 gains/six losses. Central baseline restored, but eight lost new orthogonal gains remain lost. The native main-axis repair does not yet identify their actual HTML cause. No ledger change.
- Actual parsed-HTML headless probe distinguishes native keyword grid100 from native auto grid800. Parsed root includes a float-band host, so its800 width alone is not a nested CSS box measurement. Source tracing found float-band non-float builder bypasses content-sized wrappers by calling element directly. Shared preferred-size wrapping extracted and applied to that path; workspace pipeline running. Exact page validation pending.

- Shared non-float band preferred wrapping completed workspace284 tests/check/build and frozen147 controls with112 gains/six losses. Every146 static pair actual/reference is pixel-identical to the preceding main-fit candidate; only animation phase differs. This change does not explain sizing002. No ledger acceptance.
- Native private block-versus-flex fit fixture passed both400px cases. Fast diagnostic compiler profile changes only kamin-html opt-level to0; initial build133s, incremental trace19s. Same-source147 static parity run active. Temporary opt-in style/wrapper trace source is diagnostic and must be removed before production acceptance. Production remains the acceptance build.

- Diagnostic compiler parity completed: all146 static actual/reference images match the ordinary build exactly; only animation phase differs. Fast builds remain diagnostic evidence and require ordinary-profile confirmation.
- Actual HTML fit sizing traced to a box-model mismatch: the394px content-box argument plus6px borders was treated as a394px border box. The shared native content-box contract restores eight lost new orthogonal greens with zero additional losses in the same147 controls. Workspace285 tests/check/build passed. Ledger unchanged.
- Native scrolling on the actual CSS box restores sizing002, but unrestricted application introduces a percentage-overflow grid loss. This candidate is rejected. Restricting that contract to intrinsic-sized CSS boxes makes both pages exact pass in a joint smoke control; workspace286 tests/check/build passed.
- Empty ratio leaf failure reproduced natively: a fit-content block under a flex parent is0x100 instead of100x100, while standalone and nonempty controls pass. Unresolved intrinsic preferred sizes were fabricated as definite zero, suppressing the ratio minimum transfer. Removing that fabrication passes258 private native library tests, including12 permanent boundary cases preserving explicit0/50px widths. Workspace286 tests/check/build passed; actual aspect025 now exact pass alongside both scroll controls. Same-order147 diagnostic validation is active. Temporary style/bounds traces removed before freezing.
- Column-lane margin regression reproduced against an ordinary grid using nine synthetic boxes for first/last groups. Parallel horizontal first-baseline fallback was ignored by lanes; matching the ordinary-grid policy makes all18 item coordinates agree. Existing258 native library tests and the separate parity fixture pass. The gate and permanent parity test are adopted; ordinary-profile workspace/check/runner validation remains pending. No new accepted gain yet.

- Ordinary-profile ratio/lane candidate completed 286 library tests (238 HTML) with the default test stack, all-target check, and runner build. The exact same-order147 run completed:143 pass/four fail,120 gains/three losses against the accepted-engine witness baseline; all123 transitions have RGB proof. Lane003 is restored. This candidate remains unaccepted while the text-combine and two vertical grid losses are unresolved.
- Same-order old/new residual captures show the vertical grid actual images unchanged while their reference auto-sized boxes begin painting. Intrinsic contributions from auto vertical grid children were missing before stretch. A small extracted helper supplies those contributions;287 library tests/check/build passed in the diagnostic profile. Nine-page smoke keeps six controls passing and reduces both grid discrepancies to106/430 physical pixels, with no tolerance change. Wider147 verification is running; fixed-size RTL text rounding remains under investigation.

- Definite vertical inline constraints now establish the anonymous row percentage basis before intrinsic measurement. Available space alone left <br> percentage separators without a definite basis; geometry traces show the bracket incorrectly sharing the preceding forced line. Exact nine-page controls now pass9/9, restoring text-combine and both grid-scroll losses with RGB proof. Rotated intrinsic contributions and cache use the existing unrounded root measurement API. Diagnostic147 verification is active; ordinary compiler/default-stack tests287, all-target check and runner build passed. Production147 and4309 family snapshots are prepared; no additional gain accepted before broader comparison.

- Definite basis correction completed the147 diagnostic witness family:146 exact pass/one known animation fail,120 RGB-proved fail-to-pass transitions against the same-order accepted-engine baseline, zero losses. The ordinary compiler build repeats exactly the same verdicts, with all292 static actual/reference images identical to the diagnostic build. Workspace287 tests/check/build pass with default stack. Private native library including permanent lane parity and ratio boundaries passes259 with debug info disabled; the prior extra native compile attempt failed writing LLVM output and is not claimed successful. Broader4309 production evidence is prepared; ledger remains unchanged pending that run. A16-case vertical absolute-position family is prepared for the next mechanism investigation.

- The16-case vertical absolute-position family failed uniformly on404 red fringe pixels despite correctly positioned100x100 green squares. An explicit GPUI sampling API now allows CSS natural-size raster backgrounds to preserve source pixel boundaries. Resized/transformed images, generated backgrounds and existing native UI callers retain linear sampling; the reserved sprite padding bit carries the mode without changing the ABI. Windows, Metal and WGSL paths clamp nearest texel access to the atlas tile. Both diagnostic and ordinary/default-stack builds pass288 library tests, all-target check and runner build. The16 cases now pass exactly in both profiles, all16 transitions RGB-proved with no ambiguity; all32 captures match across profiles. Independent48-case resized/rounded/transformed/gradient controls retain25 pass/23 fail with zero gains/losses in both profiles; all94 static captures match across profiles (one animation pair excluded from static parity). Windows runtime verified; Metal/WGSL runtime validation remains unavailable here. Combined ordinary candidate is frozen and its same-order4309 family is running. No new gains are added to the global ledger until that broader comparison and original-baseline deduplication complete; no full eligible-corpus sweep has started.

- Combined ordinary4309 family completed with terminal exit0:3012 exact pass,1035 fail,262 unchanged exclusions;384 distinct fail-to-pass transitions and one pass-to-fail animation transition versus the accepted raw-atoms engine. Both-side script/variant/URL-state and upstream positive-relation audit finds no scope issues among384 gains. Direct RGB proof covers379 gains; the remaining five historical stem collisions are resolved through previously proved same-order147 production witnesses, whose ten actual/reference captures are RGB-identical to the current broad captures. The animation is not waived: both frames contain no red but have different heights; old fresh captures also mismatch, and the old live wrapper differs from the current real-node animation. Shared elapsed-time verification must preserve each engine's own duration/easing and must not count synchronization as an engine gain. No clock adapter has been applied or compiled. The original1714 fragmentation family is prepared for current-engine retention; a fresh frozen accepted-engine baseline is now running before that comparison. A separate720 background family is prepared for both engines, not run. Global ledger remains unchanged; the1000 checkpoint and full eligible sweep are still pending.


### Explicit animation timeline: source prepared, verification pending

An optional GPUI animation elapsed-time global and `WPT_ANIMATION_TIME_MS` runner flag are implemented. Without the explicit override, the live scheduler keeps its previous duration, easing and chain behavior. The historical accepted source has been reconstructed from its frozen patch and all 15 recorded extra sources; its Cargo.lock differed only in line endings, verified before restoring the exact frozen bytes. The same timing seam was applied without changing the historical HTML animation renderer. Neither timing-adapted engine has been compiled or runtime tested yet. Serial frozen fragmentation runs take priority; the accepted ledger remains +187/-0 and the 384 distinct RGB-proved transitions remain pending.

The fresh accepted-engine fragmentation run finished with terminal exit 0: 876 pass, 741 fail, 97 unchanged exclusions. All 1714 report rows exactly match the historical accepted report. The frozen ordinary candidate is now running the same ordered 1714 pairs; acceptance requires terminal completion and capture comparison.

### Definite absolute vertical inline size: unverified source change

Four RTL orthogonal absolute tests show a correctly sized 80px green glyph displaced downward by 240 CSS px. The atomic inline route builds its paragraph without the ordinary element orthogonal-size setup. A source change now gives positioned vertical text its own definite content inline size before RTL alignment, rather than the inherited wrapping fallback. Parent grid/flex and table column sizing keep their existing contracts. A regression unit test covers simultaneous insets and min/max/border-box constraints. Compilation and WPT confirmation remain pending; no gains are counted from this hypothesis.

Frozen ordinary fragmentation retention completed with authoritative exit 0: 881 exact pass, 736 fail, 97 unchanged exclusions; five new fail-to-pass transitions and zero losses against the fresh accepted-engine baseline. All five transitions have direct RGB proof with no capture ambiguity, and all 152 previously accepted fragmentation gains remain green. The ordinary workspace pipeline for the explicit animation clock and definite absolute vertical inline correction is running; neither source change is declared validated yet. The animation remains the one prior accepted axes gain not retained under serial live capture.

The ordinary clock/definite-inline pipeline completed with exit0: 289 library tests (241 HTML), all-target check and runner build. The runner is frozen before compiling historical source. All five additional fragmentation gains pass the eligibility audit; the fifth is an official print-reftest with a positive upstream relation, script-free documents and native WPT_PAGE=1. The audit now recognizes this upstream category and requires native pagination for it. Historical accepted source with the same GPUI clock seam is compiling; standalone native clock tests remain in progress. No animation runtime or definite-inline WPT success is claimed yet.

Both ordinary animation-clock builds completed with terminal exit0. All twelve timed witnesses at 0/125/250/375/500/750 ms completed and their report verdicts were checked against exact RGB captures and trace-confirmed elapsed overrides. Current renderer passes all six phases; historical renderer passes only375ms, with its reference retaining100px height while its test animates. This validates sampled animation behavior without correcting the historical duration/easing or counting synchronization/a previously accepted test again. Native actual GPUI clock tests pass2 (83 other native tests not run). Default-clock48 controls retain25 pass/23 fail,0gains/0losses; all94 static captures are RGB-identical to b832, with the two live animated captures also identical but not used as a timeline proof. The frozen b832 baseline430 for the definite absolute inline correction is now running; the current candidate430 is prepared.

### Natural-size raster img sampling: source prepared, not verified

Decoded HTML raster images now opt into preserving source pixel boundaries when the fitted image dimensions exactly equal the source frame dimensions and the linear transform is identity. Native UI callers default to interpolation; resized/transformed content and vector images retain interpolation. The policy follows fitted content rather than an outer letterbox, and paint dispatch was extracted to a leaf module so img.rs shrinks. Native boundary tests cover contain/none versus fill/cover, the default off state, fractional resizing and nonidentity transforms. These changes are not compiled or WPT-tested yet. A frozen263d baseline family of247 independent raster-img pairs is prepared (419 HTML files present,581 resources snapshotted,5 missing references), not run. The separate430 definite-inline comparison still uses its preexisting frozen engines.

### Measurement checkpoint: 580 distinct demonstrated gains

The definite absolute vertical inline comparison completed with terminal exit0:358 exact pass,71 fail,1 unchanged exclusion across430 pairs, versus354/75/1 in the frozen previous engine. All four new transitions have direct exact RGB proof and pass the upstream positive/script-free eligibility audit; none overlaps earlier gains. The local private evidence ledger now records580 distinct tests improved against the original baseline:187 previously recorded,384 additional axes gains,5 fragmentation gains and4 definite-inline gains. Six equal-elapsed animation witnesses retain the previously counted animation case without adding a synchronization gain. This resolves the earlier pending animation attribution gate. The count is cumulative evidence across immutable compiled engine epochs, not a full-corpus or latest-source global retention result. There are420 gains remaining to the1000 checkpoint; no full eligible-corpus sweep has started. The frozen247 raster-img baseline is now running before compilation of the new IMG sampling source.

The247 raster-img baseline completed with terminal exit0:148 exact pass,99 fail;494 captures verified, no stem collisions. The natural-size IMG sampling source then passed289 workspace library tests (241 HTML), all-target check and runner build in the ordinary profile with default test stack. Two actual GPUI sampling tests pass, with85 other native tests filtered out. Existing compiler warnings remain; no clean-clippy or full native-suite success is claimed. The new runner and complete source identity are frozen before further source edits. Its same ordered247 candidate is now running. Additional48 image controls and retention of every580 counted gain are frozen and prepared; the retention selection has770 HTML and800 resource files with no missing inputs, and uses an explicit125ms clock for the previously counted animation pair. Neither prepared family has a GUI verdict yet, and IMG changes add zero counted gains until terminal pixel/eligibility comparisons complete.

The247 IMG candidate completed:151 exact pass,96 fail, four new gains and one regression. All five transitions have direct RGB proof and all four gains pass eligibility, but zero gains are added because the PNG EXIF pair regresses on619 pixels. Both decoded source/reference rasters are byte-identical after the correct orientation, so the next candidate computes nearest source texels from local sprite geometry before adding the integer atlas origin, avoiding interpolated atlas UV precision at texel boundaries. This is a general rendering correction, with no filename gate or epsilon. Windows/Metal/WGSL sources are updated; the new ordinary validation pipeline is running and no runtime repair success is claimed yet. The independent48 controls completed25 pass/23 fail with zero transitions. A separate legacy clip device-edge experiment and106 ordered clip/mask controls are preserved, uncompiled and not run; its integration is inactive while the IMG regression is repaired. The580-gain retention selection remains prepared, not run; cumulative ledger stays580.

The source-texel correction passed289 workspace library tests (241 HTML), all-target check and runner build in the ordinary/default-stack profile. Its five-case frozen runtime probe passes all five cases, with direct RGB equality checked for all ten captures: the EXIF regression is restored and the four IMG gains retained. These small witnesses do not establish broad retention or add new gains by themselves. The repaired same-order247 comparison is running;48 controls and580-counted-gain retention are frozen for this exact compiled engine. The independent720 background comparison now also has a source-texel production snapshot paired with its prepared historical baseline; both720 runs remain unexecuted. Windows shader compilation/runtime is verified by this build and probe; Metal/WGSL source changes have no native runtime validation on this host.

The repaired247 family completed with terminal exit0:152 exact pass,95 fail, four eligible gains and zero losses versus the pre-IMG frozen engine. All four transitions have direct RGB proof, no ambiguity and no overlap with previously counted tests; the previously regressed EXIF case is restored.

### IMG gains accepted; bulk background comparison started

The current-engine retention and48 sampling controls completed with terminal exit0. Controls retain25 pass/23 fail with zero transitions. Direct RGB inspection retains every counted test:579 match the selected primary reference, and margin-collapse-vrl-022 matches its declared second positive reference. Its first reference is unequal; the runner kept the first reference capture despite reporting the alternate match. A separate frozen same-engine witness proves both exact equality to the second reference and pixel identity of the test capture across runs. This witness adds no gain and does not relax comparison criteria.

The four IMG gains are now accepted in a new immutable private ledger generation:584 distinct improved tests versus the original baseline,416 remaining to the1000 checkpoint. The earlier ledger is preserved. These are test-level results against declared positive references; the selected primary pair is not claimed equal when an alternate wins. The same-order720 background comparison has started with its frozen historical engine. The corresponding source-texel production engine is already frozen and will run next. The full eligible corpus remains unrun.

The runner source now preserves an exact successful alternate capture and its reference path separately from the original primary pair. PNG output was extracted to a small leaf so the runner shrinks. This source-only evidence fix is not compiled yet and is excluded from the frozen720 engines; it changes no engine behavior or pass criterion. Compiler validation and an alternate-reference runtime witness will follow the serial GUI comparison.

The720 historical background baseline completed with terminal exit0:451 exact pass/269 fail, all1440 captures checked with no stem collisions. The frozen source-texel production comparison is now running. No transition from an unfinished prefix is accepted.

The720 production background comparison completed with terminal exit0:552 exact pass/168 fail,101 fail-to-pass transitions and zero losses. Every transition has direct RGB proof with zero ambiguity; both-side script/variant/positive-relation eligibility has zero issues. None overlaps earlier counted tests or pairs. The already retained source-texel engine therefore adds101 accepted gains in another immutable private ledger generation:685 distinct improved tests,315 remaining to the1000 checkpoint. Full eligible-corpus verification remains unrun; new uncompiled baseline/evidence changes are excluded from this result.

Independent baseline investigation confirms that ColumnStack exports size without content baselines, while native column grid-lanes used the first occupied track and exported no last baseline. CSS Grid3 §6.5 and the adjacent Blink stacking accumulator select first/last usable content baselines per track, then the highest first/lowest last across tracks. A native leaf candidate now implements that stacking-axis rule, preserving placement positions before relative/content offsets so dense backfill and relative positioning do not nominate the wrong child. Five source tests cover multiple tracks, dense backfill, relative offsets, unavailable baseline sets and spanning children. The candidate is source-only and uncompiled; it is excluded from both frozen720 engines and adds zero gains. Row-axis baseline behavior is unchanged. Multicolumn baseline propagation remains incomplete and must be corrected rather than forcing lanes to match a broken multicolumn reference. Native tests, ordinary compiler checks and runtime comparison are pending after the serial GUI run.

The native baseline candidate now passes the actual Taffy full library:266 tests, including five track-selection boundaries and two parent-flex placement tests proving first/last export reaches the consumer. The first standalone fixture attempt omitted README required by include_str and failed before tests; that failed artifact is preserved, and a fresh complete source snapshot passed. The ordinary workspace/default-stack pipeline is running, including separate runner oracle tests, all-target check and runner build. No native test is counted as a WPT gain. The candidate remains excluded from the accepted685-gain engine until runtime and retention evidence is obtained.

The ordinary candidate pipeline completed with terminal exit0:289 workspace library tests (241 HTML), eight runner oracle tests, all-target check and runner build. Its exact compiled source and binary are frozen before further edits. A one-case live witness now proves the runner preserves the unequal primary reference and a separate exactly matching declared alternate capture with the correct reference path; all three documents and their resources were frozen before the run. The capture correction adds zero engine gains. Input verification explicitly supports this expanded declared-reference snapshot scope while retaining exact identity checks for older immutable snapshots. The same-order242 baseline comparison has started on the previous frozen source-texel engine; the new candidate and48 controls are prepared. WPT transitions and broad retention for the native baseline change are still unverified.

The242 baseline run completed with terminal exit0:81 pass/158 fail/3 exclusions, with478 valid captures and no missing documents/resources. The frozen candidate production run is now executing. Independent multicolumn integration work adds a native API for the last completed full subtree layout output, including first/last baseline sets; dirty/uncomputed nodes return no output and removed nodes return an error. Two source tests cover inset-adjusted baseline extraction and invalidation/removal. This new API is uncompiled and excluded from the running frozen candidate. A separate native source snapshot is prepared for its validation after GUI completion. ColumnStack will need isolated subtree measurement because Window temporarily removes its shared layout engine during the parent measurement callback; nested calls through the shared Window layout API would panic. No approximate font-derived baseline or test-specific geometry is adopted.

The242 candidate completed with terminal exit0:87 pass/152 fail/3 unchanged exclusions. All six gained tests have direct RGB proof, valid upstream script-free positive relations, zero overlap with counted tests and zero family losses. These gains are not yet accepted:48 controls are running, and685 counted-test retention plus complete835 lane old/new coverage are prepared for this same frozen engine. No full-corpus run has started. The independent completed-output API now passes the actual native library with268 tests, including both new output/invalidation tests; it remains excluded from the frozen runtime candidate and awaits GPUI/custom-column integration and ordinary application compilation.

The complete lane previous-engine run has now finished with authoritative exit0;
ordered reports, frozen documents/resources and collision-free captures are verified.
The same ordered lane comparison is running on the frozen production engine. Its
existing controls have zero transitions. Retention remains the next serial GUI step.

Independent subtree measurement is implemented in GPUI using copied native styles
and separate caches with shared element measurement callbacks. ColumnStack now
requests actual child measurement trees and exports horizontal first/last content
baselines using fragment placement, spanners and relative offsets. Column request
logic was extracted from the oversized flow module into a leaf. Native adapter
tests cover cache/style isolation and callback lifetime; column tests cover flow
order, visible baseline offsets and aggregation. These new tests have not run:
the source is uncompiled and excluded from the frozen lane runtime comparison.
Intermediate text-line baselines in sliced paragraphs, repeated table bands and
vertical content baseline export still need further implementation/verification.
No prototype test or source change is counted as an accepted WPT gain.

A separate strict retention verifier supports the manifest's exact accepted-test
ledger and separately captured winning references. It checks exact RGB, the frozen
reference source, direct positive declarations and upstream URL-state eligibility,
while preserving unequal primary captures. Its already completed alternate-reference
witness passes; the forthcoming retention run itself remains unverified.

The complete same-order lane comparison finished with authoritative exit0 on both
engines. It proves seven exact RGB fail-to-pass transitions, zero losses and zero
capture ambiguity; all seven pass upstream script-free positive-reference eligibility
and have no overlap with previously accepted tests. Accepted-test retention is now
running on this frozen candidate. The seven gains remain pending that existing gate;
no additional acceptance gates are introduced. The independent ColumnStack source
prototype remains uncompiled and excluded from these runtime results.

Accepted-test retention completed with authoritative exit0. Every prior accepted test
has exact RGB proof against a declared positive reference, including its separately
captured winning alternate when the selected primary differs. The seven broad lane
gains are now accepted in an immutable ledger generation; cumulative distinct gains
are692, with308 remaining to the1000 checkpoint. No full eligible-corpus run started.

The independent column implementation now carries intermediate paragraph line
baselines from actual wrapping/paint metrics. Per-node metadata changes only during
performed layout, so intrinsic probes cannot overwrite metadata for a cached full
layout. Visible slices select real intermediate lines with native child/content insets.
Native context lifetime tests exposed retained callbacks after clear/remove; teardown
is corrected and extracted from the oversized native tree module. The full actual
Taffy library passes270 tests, including both new teardown boundaries. Full actual
GPUI runs91 tests:90 pass, including all four new measurement/visible-line boundaries;
the keyboard mapper assertion fails with semicolon versus dollar. The previous GPUI
source reproduces that exact keyboard assertion in a separate run, and keyboard
source identity is unchanged. Full GPUI suite is therefore not claimed passed.

Initial standalone adapter attempts exposed dimension-type/trait-import errors,
fixture generic inference and a missing public doc; each failed snapshot is preserved.
They were fixed before the native lifetime validation. Ordinary application library,
runner oracle, all-target check and runner build are now running on the owned source.
The new column/line-metadata/teardown source remains excluded from accepted WPT gains
until compiled runtime comparisons and retention prove it. Repeated table bands and
vertical content baseline export still require further implementation/verification.

The ordinary intermediate-line application build now passes all library and runner
oracle tests, the all-target check and runner build. Its frozen bounded lane comparison
has exact RGB fail-to-pass transitions with no losses or capture ambiguity; eligibility
is verified and accepted-test retention is running. These are candidate gains pending
validation of the subsequent visibility repairs, rather than accepted gains.

Review found that whole blocks were incorrectly filtering visible overflow baselines
through the fragment mask. Whole blocks now preserve their native baseline pair;
actual clipped slices always select visible performed-layout line metadata. Positioned
descendants and floated block children cannot nominate container content baselines.
Floats in flex/grid remain ordinary blockified items. Updated source tests cover
positioned/floated children and whole-block versus clipped overflow behavior. The paragraph
fixture also uses four unequal-spacing lines to distinguish actual line metadata from
linear interpolation. These repairs are source-only, excluded from the frozen runner,
and await native adapter and ordinary application validation after the serial GUI.

The intermediate-line runner's accepted-test retention completed with terminal exit0:
all accepted tests retain exact RGB matches, including the separately captured declared
alternate. Corrected visibility source passes all four targeted actual GPUI adapter
tests; the other native tests were filtered, so this is not a full-suite claim. The
corrected ordinary application pipeline completed with terminal exit0: all library and
runner oracle tests, all-target check and runner build pass. Its frozen runner retains
the existing controls without transitions, and the bounded lane comparison is running.
Candidate WPT gains remain pending that comparison and corrected-runner retention.

The intrinsic-width investigation now has actual native fixtures for unequal flexible
lane tracks under min-content, max-content and authored min-content constraints. The
full native library passes all273 tests including the three new boundaries. The WPT
geometry mismatch therefore needs a closer model of the HTML intrinsic wrapper. A
fourth source fixture reproduces that wrapper's grid track and start alignment, but is
not compiled yet; it is excluded from the running frozen runner. The failed first
fixture snapshot used an unsupported generic helper for Dimension and is preserved;
the correct native Dimension constructor was used in the passing retry.

Repeated table-band baseline measurement is now implemented separately in source.
It snapshots the actual header/footer copies, uses the source intervals, layout sizes
and origin shifts from painting, and selects header/body/footer baselines in content
order. A source fixture checks translated band order. This extension is uncompiled,
excluded from the frozen runtime comparison and not counted as a WPT gain. It does
not introduce an extra acceptance gate for the already frozen visibility candidate.

The corrected visibility runner completed its bounded lane comparison with exact
eligible fail-to-pass transitions and zero losses. Its accepted-test retention remains
unrun. The new native equivalent of the HTML intrinsic grid wrapper reproduces the
width error: a child expands to100 instead of its min-content width80. Final grid-item
alignment now explicitly resolves fit-content from min/max-content and the actual
margin-reduced grid area, rather than assuming a definite available-space probe already
applies that clamp. The previous fallback probe was removed, shrinking the oversized
alignment module. All274 actual native tests pass, including the new wrapper boundary.
Ordinary application validation of this clamp and the repeated-band extension passed
with terminal exit0: all library tests (including the repeated-band boundary), runner
oracle tests, all-target check and runner build. The latest frozen runner passes the
existing controls without transitions and is running the bounded lane comparison.
Acceptance now targets this latest compiled source with the same exact comparison and
accepted-test retention gates. The older visibility runner's prepared retention is
superseded and remains unrun. The latest bounded lane run completed with terminal
exit0: all nine distinct fail-to-pass transitions have exact RGB and upstream
script-free positive-reference proof, zero family losses and no overlap with accepted
tests. This includes both unequal-flexible-track min-content cases that motivated the
native clamp. These gains are pending ordinary-grid coverage and prior-gain retention.

The shared grid-item alignment change also affects ordinary grids. A separate bounded
script-free positive ordinary-grid family is frozen on both the accepted previous
engine and the latest candidate, with identical documents/resources and no missing
inputs. Its previous-engine run is now executing. This is affected-module coverage,
not the full eligible corpus. Acceptance uses both lane and ordinary-grid evidence plus
the unchanged controls and every previously accepted test; no gain is inferred from
prepared families or a live partial report.

The ordinary-grid previous-engine family completed successfully and its frozen
inputs and captures were verified. The candidate now runs the identical family.
Separately, native measured leaves can now receive content baseline coordinates
on physical X as well as Y, applying the corresponding inset once and preserving
content order for vertical-rl. Three native source tests cover both axes, cache
invalidation, containment and legacy size callbacks. This source is uncompiled
and excluded from the frozen grid comparison; it yields no accepted WPT gains.
GPUI callback and paragraph paint-coordinate integration remain pending.

The bounded ordinary-grid candidate completed with an exact gain and an exact
absolute-item sizing regression, so none of its pending gains were accepted.
An isolated native callback reproduces the same boundary: wrapped longest-line
width was substituted for fit-content only in the absolute path. Applying the
same min/max/available clamp to unresolved absolute widths makes the actual full
native library pass, including both physical-axis measurement fixtures.
The corrected source is undergoing ordinary application validation. Its WPT
witness and identical affected families must be measured on a fresh frozen
runner before acceptance; restoring the old passing case is not a new gain.

The absolute-fit correction passed ordinary application library/oracle tests,
all-target checking and runner build. Its fresh frozen runner exactly restores
the original WPT sizing case; this restoration is not a new gained test.
Existing bounded controls completed with no transitions. The same ordinary-grid
family is now running against the accepted baseline, with complete lane coverage
and all prior-gain retention still prepared. No candidate gains are accepted yet.

Source-only GPUI integration now accepts actual physical X/Y first/last and
complete-line metadata while preserving legacy horizontal callbacks. It scales
coordinates once, keeps copied axis metadata independent and protects completed
layout metadata from intrinsic probes. Adapter scaling and isolation fixtures
are frozen for native GPUI validation after the current serial WPT run. This
source is excluded from the running frozen engine. Actual vertical paragraph
integration remains pending: central/alphabetic choice, reversed-line offsets
and right-origin translation must match final content-box geometry.

The corrected ordinary-grid family completed with an exact eligible new transition
and zero losses; lane coverage and previous-gain retention are still pending.
The next physical-axis contract now retains a right-origin flag through native
measurement and converts offsets using the final content width, including both
insets. All native library tests pass, including the intrinsic-versus-final-width
boundary. The measurement output type moved into a small leaf, shrinking its
former oversized module. GPUI scaling-to-native tests are under validation; an
initial test-only generic type inference error was fixed and a fresh frozen
source retry is running. This source remains separate from the frozen WPT engine.

Actual GPUI native-layout module validation completed successfully, including the
scaling/final-width/inset boundary and existing independent-cache/root-placement
checks. Full GPUI library validation is not claimed. The new physical contract
remains separate from the frozen grid runner and still needs actual paragraph
integration plus ordinary application checks. Complete frozen lane coverage
has now started; its results and prior-gain retention are required for acceptance.

Vertical Paragraph source now exports actual physical-X line positions in content
order through the validated right-origin contract. It distinguishes central
and sideways alphabetic baselines and carries actual reversed-line placement.
The horizontal baseline helper was extracted without changing its captured
line-height behavior. Three source fixtures cover variable advances, reversed
order and alphabetic metrics; these changes are not compiled or counted yet.
Visible-X subtree collection is separately implemented and frozen for GPUI
validation, with final-width/inset/scrollbar and intermediate-line clipping
coverage. Both changes are excluded from the currently running frozen engine.

Complete lane verification finished with exact eligible new transitions and
zero losses; the current candidate now runs all prior gained tests for retention.
A future bounded writing-mode/alignment/baseline selection is prepared on the
frozen pre-vertical engine. Its document inputs are complete; the only missing
resource is explicitly broken-image input shared by test/reference cases.
Neither future family has run, and vertical source gains remain unmeasured.

The absolute-fit candidate completed exact prior-gain retention, including the
separately captured declared alternate reference. Its distinct new transitions
from complete lane and ordinary-grid coverage are accepted with no bounded
losses. Vertical Paragraph and visible-X source validation has now started on
an immutable GPUI snapshot; the ordinary application and future affected WPT
comparison remain pending. The existing native vertical Paragraph render gate
is narrow; extending it requires verification of the legacy rotated text path
and orthogonal constraints, rather than assuming all vertical text uses Paragraph.

The physical-baseline source passed native GPUI module tests and every ordinary
application validation stage. Frozen controls completed without transitions;
the pre-change writing-mode/alignment baseline is now running serially.
That physical-baseline-only candidate remains unaccepted and its broad family
has not run. A newer source implementation replaces the subgrid-only plain-text
gate with parent-time native vertical flow, including sideways-lr rotation and
intrinsic/fixed/minimum inline constraints. Its new fixtures and application
validation are pending serial GUI completion; no gains are inferred from source.
The two affected selections cover complete writing-mode, alignment, grid, flex,
sizing and multicol families without overlap. Supplementary inputs are frozen
on the same accepted pre-change engine, and remain unrun. Whole-corpus validation
still follows the requested distinct-gain milestone.

Native vertical pointer selection now carries physical bounds through painting
and converts screen coordinates back into flat text coordinates. GPUI mouse
listeners do not apply paint transformations automatically. Source fixtures use
the real GPUI matrix at fractional scales and reversed vertical-lr line order.
The selection method moved into a leaf, shrinking the oversized paragraph
module. Compilation and runtime acceptance remain pending; this source is
excluded from the running frozen baseline.

The broader native path also retains parallel table-column min-content probes,
authored intrinsic-height precedence and own size limits. Vertical glyph/span
flags stay attached to the flat text that is rotated for paint. These source
contracts have new fixtures; only leaf syntax/format and whitespace checks have
run so far. The ordinary validation requirement now includes all new fixtures,
and remains separate from the already validated physical-baseline predecessor.

The unified native plain-text source completed every ordinary validation stage:
library fixtures, runner oracle tests, all-target checking and runner build.
Its immutable engine passed existing frozen controls without verdict changes
or losses. The complete primary writing-mode/alignment/baseline comparison is
now running against the validated pre-change family. Whole supplementary layout
families and every accepted-gain retention remain pending. No gains from this
unified source are accepted before those exact comparison and regression gates.

The live primary comparison exposed regressions in orthogonal available-size
and block-flow fixtures. Inspection found two missing links in the native
paragraph path: its layout probe retained the constructor's horizontal flag,
and the paragraph builder did not carry the orthogonal wrapping fallback.
The successor source now forwards the axis and fallback, including the viewport
cap when no inherited cap exists. This successor is not compiled or runtime
validated yet; the running immutable engine predates these edits. Its full
report must finish before compiling and freezing the successor, and all affected
families and accepted-gain retention still require exact comparison.

The complete primary comparison of the first broad native candidate finished
successfully as a process, but its exact rendering result rejects adoption:
five eligible improvements and 67 losses, with all transitions independently
checked against saved RGB captures and no ambiguous capture mapping. None of
its gains are added to the accepted ledger. Further inspection found that
vertical block flow's flex implementation stretched horizontal orthogonal
children's automatic block sizes to parallel siblings; these children now keep
content-based block sizes and the containing block's inline-start placement.
The successor completed ordinary library tests, runner oracle tests, all-target
checking and runner build. Its distinct frozen comparison and controls remain
required; successful compilation does not establish restoration of the losses.

The broader native path now preserves authored parallel inline sizes during
intrinsic measurement and prevents the ordinary block adapter wrapper from
compressing orthogonal content. Clockwise vertical-lr paints its content extent
from the left edge even inside a stretched box. That extent follows absolute
device-edge rounding, with the layout origin captured during prepaint rather
than queried from paint. Seven sizing/direction/origin witnesses now match their
references exactly. The existing 48-control comparison has no status changes.
The complete predecessor comparison proved 13 eligible improvements and three
regressions across 1324 pairs, with no ambiguous capture mapping. Its gains are
not accepted.

Completed diagnostic rows in that comparison expose three table-cell direction
regressions. Their text matches; the differing pixels are entirely missing blue
background. Table cells establish flow-root content, but the parallel containing
block helper omitted them, allowing shorter child content to shrink its inline
box. The successor includes their authored inline size while keeping an
indefinite table-track fallback indefinite. The successor completed ordinary
workspace library/oracle tests, all-target checking and runner build. All ten
direction/sizing/origin witnesses now match exactly, including restoration of
the three cell regressions. Its 48-control comparison has no status changes;
the repeat complete primary comparison has now completed with exit 0 and
858 exact passes / 466 failures. All 13 improvements are independently proved
by RGB captures and pass upstream eligibility checks, with zero losses and
zero ambiguous mappings across all 1324 pairs. They are not yet added to the
accepted ledger. The original-engine supplementary 3085-pair run has now
completed with terminal exit 0 and complete ordered-input/resource/capture
validation, without capture-name collisions. The same frozen 3085-pair candidate
run is active; its RGB/eligibility comparison and strict accepted-gain retention
remain pending before acceptance. Resource/template limitations remain recorded
and do not establish full WPT conformance for affected cases.

While that candidate family is still active, completed capture pairs independently
prove regressions in `orthogonal-positioned-grid-items-014/015`. Both baseline
pairs have identical RGB pixels; each candidate test image is unchanged from
its baseline, while its reference changes (2068 and 4707 pixels respectively).
The visual difference is vertical text wrapping in ordinary grid items with
inline-axis margins; the absolute-positioned test side stays unchanged. This
narrows the next diagnostic to grid-item measurement and final paragraph wrapping,
without establishing a cause yet. It prevents accepting the candidate's gains
until corrected and reverified. This two-pair observation does not replace the
pending complete-family comparison or retention run.

A later partial snapshot independently compares RGB captures for all seven
transitions among 1565 completed rows: two improvements and five regressions.
The additional losses are `column-explicit-placement-002`,
`row-explicit-placement-006` and `grid-lanes-intrinsic-sizing-rows-004-fr`.
For these three, both test and reference change relative to the baseline;
unlike the first two losses, their issue is not confined to the reference side.
Grid-lanes intrinsic measurement and placement must therefore be diagnosed
alongside ordinary orthogonal grid-item wrapping. No gains are accepted from
this still-live family.

The supplementary candidate has now terminated with exit zero. All 3085 rows,
6170 captures and pair order are verified: 2394 pass, 691 fail. Exact RGB
comparison proves three improvements and six regressions with no ambiguous
transitions. The final additional loss is `orthogonal-writing-mode-001`; the
additional improvement is `orthogonal-writing-mode-006`. Eligibility audit
confirms the three gained tests, but the candidate is rejected for acceptance
because of the six losses. The accepted gain ledger remains unchanged. The next
engine step is measurement diagnosis across ordinary grid, grid-lanes and
subgrid regressions before another candidate or retention run.

Both frozen engines have completed a serial nine-pair diagnostic replay with
HTML_MEASURE enabled. All nine report rows and both capture sides reproduce
their respective complete-family runs exactly in RGB. Native Third/Fourth item
traces include known inline heights of 60 and 70 px with wrapping, whereas the
old rotated path includes unconstrained max-content shaping. Final grid-area
constraints, flex available-space deductions and item margins are now the next
isolation target; these traces do not yet establish the root cause. The combined
trace has no test/reference boundary markers, so individual lines cannot alone
prove which capture side produced them. Diagnostic input provenance is inherited
from pre-existing verified complete-family snapshots and checked after replay;
it is not described as a newly created pre-run snapshot. No gain acceptance or
whole-corpus verification is implied by this diagnostic replay.

A native Taffy witness now isolates a double margin deduction at the grid/item
boundary: a 100px area with a 15px margin supplied only 70px to the measured
leaf. Grid alignment's intrinsic probes and final layout now pass the full
available margin-box area; child layout deducts its own margins. Fit-content
border-box clamps still exclude margins. Before the fix the witness fails;
after it, direct leaves and nested flex items retain a single 75px line with
15px and 20px margins and correctly wrap with a 30px margin. A documented
regression fixture is added to the native grid tests. The standalone witness
passes, while full native Taffy tests, ordinary app validation and WPT captures
remain separate pending gates. No WPT restoration or new gain is claimed yet.

The double-margin candidate has now passed all 280 native Taffy tests and the
ordinary application gates: 313 workspace library tests, eight runner tests,
workspace/all-targets check and runner build. Its frozen nine-pair WPT witness
restores `orthogonal-positioned-grid-items-014/015` with exact RGB equality,
preserves the three prior supplementary improvements and leaves the other four
regressions failing. The 48-pair neighboring control family completes with
25 pass/23 fail and zero transitions against the prior candidate. These two
restorations are baseline retention, not new distinct gains. Full affected
families and accepted-gain retention are prepared but not run for this source.
The next isolation target is the intrinsic grid-item probe: its min-content
request sets both indefinite physical axes to MinContent, potentially wrapping
vertical text while measuring its perpendicular block-axis contribution.
That observation is a hypothesis until an independent native witness proves it.

An independent native grid witness now proves the perpendicular-axis issue:
requesting only a minimum width produces a 40px grid instead of a 20px column
because the child receives MinContent in both axes. The contribution probe now
uses MinContent only for its requested axis and MaxContent for an indefinite
perpendicular axis; definite cross sizes and authored sizing-keyword overrides
remain intact. The standalone witness passes after the fix, including the prior
margin fixture. Two native regression tests cover transposed text and definite
cross sizes that must still wrap. Full native/app tests and exact WPT replay
remain pending for this candidate; no restoration or new gain is inferred yet.

The intrinsic-axis candidate now passes all 282 native Taffy tests and the
ordinary app gates (313 workspace library tests, eight runner tests, all-targets
check and runner build). Exact nine-pair replay restores
`column-explicit-placement-002` and
`grid-lanes-intrinsic-sizing-rows-004-fr`, retains the two margin restorations
and all three prior improvements. `row-explicit-placement-006` and
`orthogonal-writing-mode-001` still fail. The row-lanes capture shows a box only
one column wide while constrained text wraps across several columns: the lane
stack-width maximum is measured before final inline track sizing and reused.
This identifies the next measurement dependency to isolate; it is not yet a
verified fix. The accepted distinct-gain ledger remains unchanged.

A native row-lanes witness confirms the stale stack measurement: a 30px final
inline track wraps a synthetic paragraph into a 40px block, but the cached
unconstrained width leaves its box at 20px and advances the next item too early.
Auto stacking widths are now measured with the final track height before
placement; authored widths and explicit stacking overrides retain precedence.
The standalone witness and all 285 native Taffy tests pass. Three regression
tests cover wrapped text, unwrapped text and authored width placement. The
external native snapshot initially lacked its README include; the corrected
compilation and test result are recorded separately. Ordinary application
validation and exact WPT replay remain pending, with no new gains accepted.

The row-stack candidate passes the ordinary application gates (313 library
tests, eight runner tests, all-targets check and runner build), but fails WPT
acceptance. Exact nine-pair replay reduces the row-placement pixel difference
without restoring equality and loses the previously restored fractional-track
intrinsic-sizing pair. Other witness outcomes are unchanged. Re-measurement
alone therefore does not preserve the intrinsic stacking-width contract; this
candidate remains unaccepted and the broad gain ledger is unchanged. Margin
handling and intrinsic-width retention need independent witnesses before a
replacement is accepted.

The 48 neighboring row-stack controls retain their prior outcomes with zero
transitions. A separate native fit-content witness identifies another double
margin subtraction in grid's sizing-keyword width probe: with a 100px area
and 15px margin it receives 70px rather than 85px, giving a 40px box for 20px
content. That probe now supplies the full margin-box cross space. All four
standalone native witnesses pass, and the regression matrix now also covers
fit-content on direct and nested items. Full native/app/WPT validation remains
pending for this additional fix; the prior frozen row-stack candidate remains
rejected, and no gain is accepted.

The sizing-keyword margin candidate passes all 286 native Taffy tests and the
ordinary app gates (313 library tests, eight runner tests, all-targets check and
runner build). Exact replay restores `row-explicit-placement-006`; the fractional
track regression and orthogonal subgrid regression remain. Its 48 neighboring
controls have zero transitions. A further native witness isolates the fractional
track dependency: authored `height: max-content` accepts an external MinContent
probe and yields an 83px container instead of 153px. Lane container measurement
now respects authored intrinsic keywords for unknown axes in inherent sizing;
known dimensions and content-only sizing retain their existing constraints.
The standalone five-test set passes after the change; full native/app/WPT gates
are pending for this source, with no broad acceptance or ledger advancement.

The intrinsic-keyword lane candidate passes all 287 native tests and ordinary
app validation, but leaves the nine-pair WPT outcomes unchanged. Inspection of
the HTML boundary explains why the native fixture is insufficient: intrinsic
grid containers still use a helper grid wrapper rather than passing their
authored keywords to their actual native node. Intrinsic grid/grid-lanes boxes
now use the same direct native path as ordinary blocks, retaining placement,
margins and numeric limits; specialized intrinsic min/max constraints retain
their existing path. Ordinary native grid also respects authored intrinsic
keywords during contribution probes. The boundary regression test and six
standalone native tests pass, including a shared grid/grid-lanes fractional
track witness. Full native/application/WPT validation is pending for this
combined boundary change; none of these local tests advances the green ledger.

The combined boundary source passes all 287 native Taffy tests. The first
ordinary application run catches a separate contract violation: sharing the
expanded native eligibility predicate lets the orthogonal block fallback
rewrite grid auto widths. That fallback now explicitly limits itself to
ordinary block formatting; its existing test is retained and extended to
inline-grid/grid-lanes. The corrected candidate passes all 314 library tests,
eight runner tests, all-targets check and runner build. Exact nine-pair replay
restores the fractional-track intrinsic-sizing pair and retains row-placement
and the other prior restorations: eight pairs pass, with only
`orthogonal-writing-mode-001` still failing. Neighbor controls and broad
acceptance remain separate pending gates; baseline restorations are not gains.

The corrected boundary candidate's 48 neighboring controls complete with
25 pass/23 fail and zero exact transitions against the prior candidate. Its
eight-of-nine witness result remains a focused restoration result, not broad
acceptance. The remaining orthogonal subgrid regression and the full primary,
supplementary and counted-gain retention families are still outstanding.

An independent native witness reproduces the remaining subgrid row contribution
error: the last two marker rows receive 50/50 instead of the required 50/50/40
distribution across the preceding three rows. Subgrid line projection now uses
the relative physical flow of each node and the root grid; inherited line names
and track sizes follow the child's flow, while accumulated edges remain physical.
Reversed track offsets are normalized before extracting inherited sizes. Grid
and grid-lanes areas support independent row and column reversal. Track inheritance
and exported area geometry were extracted into small leaves rather than enlarging
the existing grid/subgrid implementations.

The nested witness and all four asymmetric parent/child column-flow combinations
pass, including unequal track sizes, gutters, borders and parent padding. The
exact external native source copy passes 289 tests without warnings. The new
physical-axis metadata is connected through HTML, GPUI and native Taffy, with
writing-mode projection tests. Ordinary application validation is running; no
WPT restoration, broad transition or retention is claimed for this source yet.
The accepted gain ledger remains 702, and the earlier 16 pending gains remain
unaccepted until broad comparison and counted-gain retention finish.

The physical-flow candidate completes ordinary application validation: 316 library
tests, eight runner tests, all-targets check and runner build pass. Its exact
nine-pair replay restores `subgrid/orthogonal-writing-mode-001` and retains the
other baseline restorations, but loses the previously pending gain
`grid-positioned-children-writing-modes-001`. The focus result is still eight
pass/one fail; its two transitions have exact pixel proof. All 48 neighboring
controls retain 25 pass/23 fail with zero transitions. This candidate is not
accepted, and broad/retention families remain prepared but unrun.

Visual inspection of the absolute-positioning loss shows shifted green boxes
under reversed container flows, while the block reference retains left/top
placement. Source inspection finds native block RTL support without a matching
GPUI direction carrier, and a separate vertical block adapter that excludes
explicit `display: block` because of earlier measured regressions. These are
next-step hypotheses for a shared block/grid writing-direction witness; their
correction is not yet implemented or verified. The subgrid restoration alone
does not justify accepting this candidate or advancing the gain ledger.

An independent synthetic Chromium oracle confirms that block and grid children
must share coordinates in all six horizontal/vertical LTR/RTL combinations with
asymmetric padding and borders. Native block layout now accepts writing-axis
metadata and normalizes its existing BFC algorithm into logical axes. Child
constraints, sizes, edges, ratios, calc-size expressions and output baselines
are projected at the native boundary; final child positions are converted once
the container size is known. Orthogonal children establish independent contexts,
while parallel children retain native margin collapsing and float state. This
does not replace block layout with a flex container.

The new native tests match all six browser coordinate cases and cover vertical
margin collapsing and float packing in both block directions. A separate root
test catches physical-width stretching of an auto vertical root: root constraints
now fill its logical inline height and retain an auto block width, with the
correct viewport start edge. Root constraint calculation was extracted into a
small leaf. Thirteen standalone controls and all 293 native tests pass; the
exact external native copy has 113 verified Rust source files and no warnings.
HTML/GPUI carry the block metadata, while existing vertical flex adapters remain
separate work. Application validation is running; new WPT outcomes and broad
acceptance remain unverified and the accepted ledger is still 702.

The native block-flow candidate completes all ordinary application gates:
316 library tests, eight runner tests, all-targets check and runner build. Exact
focus replay now passes all nine pairs, including the previously lost grid
absolute-positioning gain and the orthogonal subgrid restoration. All 48
neighboring controls retain their prior outcomes, with zero transitions and
separately recorded terminal/pixel evidence. A frozen 1324-pair primary run is
now live against the accepted paragraph baseline; supplementary and timed
counted-gain retention runs are prepared. No broad gain is accepted until these
comparisons, eligibility and retention finish. The ledger remains 702.

The block-flow primary run now completes: 859 pass/465 fail across 1324 pairs,
with 17 exact eligible gains and three baseline losses. All 20 transitions
have pixel proof, with no ambiguous comparisons. The losses are the vertical
grid-lanes baseline pairs `column-grid-lanes-item-baseline-005`,
`grid-lanes-grid-item-content-baseline-001` and
`grid-lanes-grid-item-self-baseline-002b`. This candidate is not accepted;
supplementary and counted-gain retention gates remain outstanding.

The combined 12-pair before-fix replay retains the earlier nine restorations
and reproduces all three losses. Two native coordinate tests also reproduce
missing physical stacking reversal after successful compilation. Final lane
placement is now extracted into a documented leaf, projecting the completed
logical stack through the container's physical writing direction. Projection
uses the final size after stretch and retains physical relative insets and
asymmetric margin/padding edges. Both coordinate tests and all 15 standalone
native controls pass. Complete native/application gates and the after-fix
pixel replay are still pending; the accepted ledger remains 702.

The exact corrected native source copy now passes all 295 library tests without
warnings, with 115 Rust source files matched against the owned worktree. The
ordinary application gates are running; after-fix WPT outcomes and gain
acceptance remain unverified.

The corrected physical lane-flow candidate passes all four application gates:
316 library tests, eight runner tests, all-targets check and runner build.
The 12-pair focus replay passes exactly, restoring the three baseline losses;
these restorations are not additional gains. All 48 neighboring controls retain
their prior outcomes with zero transitions. The completed 1324-pair primary
run has 862 pass/462 fail, 17 exact eligible gains and zero losses against its
original paragraph baseline. All 17 transitions have unambiguous pixel proof,
with input, capture and local-resource provenance checked. The supplementary
3085-pair run is now live; timed retention of the accepted 702 gains remains
required before accepting these gains. The accepted ledger stays 702.

A separate private inline-decoration draft addresses physical sides being read
as flat left/right sides before vertical text rotation. Completed capture
geometry demonstrates a 120-device-pixel excess in a representative line-box
test, matching the two physical cross-axis borders. An independent Chromium
oracle confirms identical glyph geometry for cross-axis decoration in ten
writing-mode/direction cases. The draft projects spacing, run paint sides and
empty-inline decoration through the same paragraph plane. Complete parent and
small-leaf syntax checks pass, and 509 official script-free positive neighboring
pairs are selected with unchanged document hashes. This draft is not applied,
compiled or runtime validated and establishes no additional WPT gain.

The live supplementary run reveals failing transitions in lane absolute
alignment, reverse-fill intrinsic containers and intrinsic row sizing. Pixel
inspection of 13 complete loss rows confirms that the earlier captures matched;
ten references also change under the new engine, so loss classification alone
does not establish which side introduced incorrect geometry. Gains remain
unaccepted while this investigation and the whole run are incomplete.

An independent Chromium geometry oracle for the standard flex reference in a
reverse-fill case confirms that its block flex container fills the available
width. The engine instead shrink-wraps that reference when height is min-content;
its current grid-lanes test fills the width. The previous matching captures
therefore concealed incorrect reference rendering in this case. The next fix
must preserve the correct full-width lane behavior and resolve intrinsic flex
container sizing, rather than restore equality by shrinking both sides.

A private sizing-wrapper draft now preserves automatic horizontal block width
when the wrapper exists for intrinsic height. The existing start alignment is
retained for authored intrinsic widths, inline boxes, floats and vertical
formatting. The unchanged runtime source is hash-guarded; the 203-line parent
and 14-line helper parse. This is not yet a compiled or pixel-verified fix.
The verification selection contains all 2216 eligible positive script-free
neighbors in flexbox, sizing and grid-lanes, with 3489 unchanged documents;
accepted-gain retention and the existing primary/supplementary gates remain
required. No gain or restored result is credited from draft preparation.

Two further private native drafts address independent lane/subgrid contracts.
Absolute lane areas use the correct physical direction, but their item alignment
still hard-codes LTR; the draft uses ordinary grid's physical horizontal direction.
Unrun controls compare row/column lanes with ordinary grid for start/end/center
alignment and include an analytical RTL track-edge expectation.

Standalone subgrid track resolution currently requires a style-resolved container
size before inspecting fixed tracks. Thus a fixed 25px row with auto container
height can be ignored when measuring a flattened orthogonal descendant. The
draft extracts this helper and resolves equal fixed track edges without requiring
container size; flexible and unresolved percentage tracks keep their definite
basis requirement. Three unrun controls cover both fixed-track axes, gap,
fractional/percentage bases and border-box deductions. Both draft parents and
small leaves parse. Native compilation, before/after controls, exact upstream
witnesses and broad/accepted-gain retention remain required; runtime candidate
sources stay unchanged while the supplementary process remains live.

The supplementary process has now completed: 2402 pass/683 fail, with 29 exact
eligible gains and 24 losses. All 53 transitions have unambiguous pixel proof.
Together with the primary run, 46 new transitions remain unaccepted; the ledger
stays at 702 until regressions and accepted-gain retention are resolved.

Independent native before/after controls now reproduce the absolute-alignment
and fixed standalone-track defects. The before source passes 297 tests and fails
three; the corrected source passes all 300 tests without compiler warnings.
An initial shared-target after invocation reused the before executable and is
explicitly excluded from evidence. A separate target directory recompiles the
corrected source and supplies the authoritative after result. Hashes verify the
complete fixture sources and the unchanged original runtime source before the
two fixes are applied locally. Application checks are in progress; exact WPT
restoration is not yet established. All 24 supplementary losses and 29 gains are
selected for a focused diagnostic rerun before the ordered broad gates.

The corrected candidate passes all four application gates: 316 workspace library
tests, eight runner tests, all-target checks and runner build. The terminal
53-pair focused run has 45 pass/eight fail: 16 previously failing pairs are now
exact matches, and all 29 preceding gains remain exact matches. Each mapped
observation is checked against RGB pixels; the changed sequence makes these
restorations diagnostic evidence pending identical ordered broad runs. The
48-pair neighboring control run has zero changed outcomes and no ambiguous
transitions. The remaining eight loss witnesses cover intrinsic flex wrappers,
fractional intrinsic rows, nested standalone fractional tracks and replaced
auto-repeat sizing. An isolated native aspect-ratio constraint probe is now
compiling; its outcome and relation to the remaining losses remain unproven.
No new gains are accepted from these restorations.

The horizontal automatic-width wrapper fix is now applied. Five independent
native layout controls pass: intrinsic height, automatic/fixed width and fixed/
automatic margins. All four application gates pass. The identical ordered
53-pair focused replay improves from 45 pass/eight fail to 48 pass/five fail:
three exact restorations, zero losses and no ambiguous pixel transitions.
These are the standard flex reference cases independently measured in Chromium;
the corrected reference fills available width without shrinking the lane test.
The neighboring 48-pair replay and an identical ordered 2216-pair comparison
remain required, alongside the existing broad and accepted-gain retention gates.

An independent native ratio probe also establishes two defects in the unchanged
leaf/root constraint code: a definite height is overridden by a preferred ratio,
and a transferred maximum reduces definite width. The three-case probe passes
296 tests and fails the two new constraints; a separate Chromium oracle gives
the expected dimensions for all three. CSS Sizing Level 4 specifies that definite
preferred dimensions are unaffected by transferred constraints. The first
probe generation failed to compile because its test argument used the wrong
native type and supplies no reproduction evidence. The corrected generation
is built in an isolated target. This investigation establishes no WPT gain and
does not yet establish that these defects cause any remaining loss witness.

The wrapper's 48 neighboring controls now complete with zero changed outcomes.
The identical ordered 2216-neighbor before generation is live; its frozen after
generation is prepared with the same documents and resources.

A private native ratio correction is frozen separately from runtime sources.
Root and leaf use a common min/max transfer resolver that caps transferred
minimums and floors transferred maximums by definite destination sizes. Leaf
ratio inference preserves known height, applies independent bounds afterwards,
and uses content dimensions for content-box sizing. The leaf parent shrinks by
extracting small helpers. Six transfer controls and four leaf controls accompany
the three reproduced ratio cases and the existing 300 native tests. Small
helpers and the full parent parse; the expected 313-test suite is uncompiled.
Native runtime source hashes are unchanged. A separate four-case nested
standalone-track probe compares fixed and fractional rows under a definite
parent for grid and lanes; it is also unrun. Compilation remains serial with
GUI measurements, and neither draft establishes additional WPT gains.

Further inspection localizes both remaining intrinsic-row witnesses to the first
two containers: exact black-border bounds show shorter test containers than
their fixed-track references. A separate Chromium standard-grid oracle verifies
that min-content height, max-content height and fixed rows all produce the same
100px total with 20/40/20/20px rows in a numeric unequal-fr control. An eight-case
native probe now compares grid/lanes, min/max height and root/nested layout.
It parses but is uncompiled while the specific 2216-pair GUI generation remains
live. This evidence guides the sizing investigation; it does not justify a
blanket keyword rewrite, prove native reproduction or count a WPT restoration.

A loaded-resource Chromium oracle for the remaining replaced auto-repeat
reference verifies natural image dimensions 113x120, used height 100px and used
width 94.15625px, with matching auto-column widths and a 10px gap. An initial
oracle captured incomplete images and provisional 70px columns; that generation
is preserved and explicitly superseded by the load-event oracle. Exact engine
capture runs place the second painted image at different positions in test and
reference, while both share the same first image. The reference spacing also
differs from the independent loaded-resource geometry, so equality-only rollback
would preserve incorrect layout. Four independent leaf image controls now
exercise natural-ratio/max-height sizing in intrinsic and final layout modes,
with and without an incoming track height. Identical caller sources are frozen
against original and private corrected native algorithms; they parse but remain
uncompiled while the specific neighboring GUI generation remains live. No exact
restoration or new WPT gain is claimed from this oracle or probe preparation.

The 2216-pair wrapper-neighbor baseline has now completed with terminal exit 0:
1809 pass and 407 fail. Ordered input documents, local resources and 4432 PNG
captures were verified. These are baseline outcomes, not additional gains.

Native numeric probes reproduce the remaining nested fractional standalone-axis
width defects in ordinary grid and grid-lanes, and the authored min-content
block-height defects in root and nested layouts. Independent standard-grid
controls distinguish stretch from start/auto-margin sizing and verify padding,
border and maximum-height effects; blindly propagating parent size is invalid.
A private writing-mode-aware authored block-keyword candidate passes all eight
numeric grid/lanes min/max-height controls. Its full native suite is still running.

The shared ratio-constraint candidate passes four identical before/after replaced
image controls and all 313 native tests without warnings. All 117 original native
source hashes matched the owned worktree before applying its eight changed files.
The ratio candidate is now present in the owned worktree; application gates and
WPT comparisons on that candidate remain pending. No ledger gain is accepted.

The authored block-keyword candidate completed all 301 native tests without
warnings and was applied after checking each affected baseline source hash.
The combined ratio, block-keyword and definite nested standalone-size candidate
then completed all 315 native tests without warnings. Four fixed/fractional
nested sizing controls now return the independently expected 100px width in
both ordinary grid and grid-lanes.

Eight additional ordinary-grid controls compare identical caller code against
old and new algorithms with independent Chromium expectations. Six candidate
outcomes match, including explicit/max height, padding, border and fixed margin.
Two pre-existing intrinsic-width discrepancies with start alignment or auto
block margins remain unchanged; they are retained as open follow-up controls.
The nested candidate was applied only after preserving these before/after
outcomes and verifying the complete 315-test source hashes. All three fixes now
share the owned native implementation. Application gates are running; WPT
restoration, broad comparison and accepted-gain retention remain unverified.

The combined runtime focus completed 50 pass / 3 fail: two intrinsic-row
regressions are restored with exact RGB proofs, zero ambiguous witnesses and no
new losses. The 48-pair neighboring control run has zero transitions. The
2216-pair neighboring comparison is now running on the frozen combined binary.
The two nested standalone witnesses and one replaced auto-repeat witness still
fail. The numeric nested controls do not establish actual HTML correctness:
the actual native Paragraph versus rotated-wrapper route and its measurement
constraints still need a frozen runtime trace. Premeasurement attribution is
unproven. No additional ledger gain is accepted from these runs.

Follow-up source inspection confirms a native vertical Paragraph route already
exists for plain-text nodes. Attribution of both remaining nested witnesses to
rotated-wrapper premeasurement is therefore unproven. A separate frozen three-
pair diagnostic family is prepared to trace the actual route before changing it.

A separate source audit finds that HTML Len parsing still substitutes stretch
and legacy fill-available spellings with 100 percent, despite native Taffy
having a distinct stretch keyword. A private parser/GPUI boundary draft now
preserves preferred-size keyword identity and parses as Rust, but has not been
type checked or adopted; exhaustive Len consumers and min/max keyword contracts
remain pending. Independent Chromium flow-root controls verify border-box widths
160px versus 230px and heights 70px versus 117px for stretch versus 100 percent
with margins, padding and borders. The initial ordinary-block height control
also involves margin collapse and is preserved separately; it is not equivalent
to the flow-root case. These controls establish semantic distinction, not WPT gains.

The expanded private stretch draft now covers nine source files, including
non-numeric calc, background and clip-radius consumers; all parse as Rust.
All corresponding owned sources remain unchanged. A 34-pair before family is
frozen from 4409 distinct pairs in existing eligible selections. It covers
direct stretch declarations only; neither a completed run nor full-corpus
coverage is claimed.

Eight independent Chromium min/max stretch controls confirm that these bounds
also differ from percentages: border-box widths 160px versus 224px and heights
70px versus 117px with this fixture's margins, padding and borders. Source
audit confirms two missing contracts: GPUI has keyword overrides only for
preferred dimensions. A subsequent type audit corrects the native attribution:
min/max fields use LengthPercentageAuto, which cannot represent stretch, whereas
Dimension is used for preferred sizes. Injecting an unsupported tag into the
bound type would reach unreachable. A separate bound representation/resolution
contract is required; preserving the preferred-size enum alone is insufficient.
The 2216-pair combined runtime comparison remains live; no new gain is accepted.

A direct declaration scan now covers the complete saved official positive
script-free relation graph: 35 distinct stretch pairs, including seven with
min/max bounds. All 34 previous neighboring selections are included, with one
additional native input-color appearance witness. This official before family
is frozen with 50 documents and 51 referenced resources, zero missing/external
files. It remains unrun, excludes URL-state relations and does not cover
stylesheet-only keyword declarations or claim rendering success.

A private stretch-bound contract now retains numeric min/max types and adds
separate default-off physical stretch flags, with CoreStyle default/reference
forwarding. A shared resolver converts containing-block space minus margins
into authored box coordinates before existing ratio transfer. The draft is
connected to leaf and root-block constraint paths and syntax checked in seven
files; its style parent shrinks by extracting custom identifier bounds.
Block/flex/grid/grid-lanes consumers, cached GridItem bounds, GPUI/HTML flags,
type checks and runtime geometry remain required before adoption. Exact native
struct size assertions are preserved pending observed compiler layouts.
All seven corresponding owned sources remain unchanged during the live run.

The unrun aggregate route diagnostic had selected adjacent witnesses because
one-based report row numbers were interpreted as zero-based selection indices.
It and the first individual selections are retained as invalid diagnostic
inputs; the old aggregate launch helper now refuses execution. Three corrected
individual families select exact basenames and verify the remaining failures
against the completed focus report (zero-based indices 35, 39 and 47).
Separate traces avoid attributing repeated text measures by aggregate log order.
No completed focus or broad result is changed by this diagnostic correction.

The private preferred-stretch draft now covers ten source files. Image natural
sizing preserves an automatic axis opposite a stretch keyword for measured
layout, instead of fixing it to the natural size before ratio transfer. Existing
percentage behavior remains in the same deferred path. Syntax parsing passes;
type checks, independent image geometry, outer replaced-holder constraints and
WPT comparison remain unrun. All corresponding owned sources are unchanged.

The 2216-pair combined neighbor run completed with terminal exit zero:
1815 pass / 401 fail versus 1809 / 407 previously. Six fail-to-pass transitions
are proved by exact RGB comparison, with zero losses and zero ambiguous
captures. These include known restored witnesses; no additional gain enters
the ledger before original-baseline attribution and remaining acceptance gates.

Corrected individual diagnostic runs completed for all three remaining focus
failures. Both nested witnesses log native Paragraph measurements of the
vertical X text with a 100px inline constraint, never the expected 25px track
constraint, yielding two 75px text lines. There are no rotated-wrapper events.
This directly proves the native Paragraph route; the remaining investigation
is how HTML construction/native flattening supplies track constraints.
The official 35-pair stretch before family is now running on the frozen
combined binary. No preferred-stretch or bound draft has been adopted.

The official stretch before family subsequently completed with terminal exit
zero: 26 pass / 9 fail, 35 ordered pairs, 70 captures, 50 documents and 51
resources verified, with zero missing resources and zero naming ambiguity.
This immutable baseline is ready for the future candidate comparison.

Two private native structural probes now compare the direct measured leaf
with block/flex containing boxes, percentage max-height and vertical block-flow
flags under fixed/fractional nested tracks in ordinary grid and grid-lanes.
All 16 initial and 24 expanded numeric cases return the expected 100px root
width without compiler warnings. The expanded probe therefore excludes generic
wrapper presence and these isolated style flags as sufficient explanations
for the actual HTML 50px result. Actual constructed native styles and track
placement still need inspection; no runtime fix or WPT gain is claimed.

Optional actual native-tree tracing is now implemented in a small GPUI module,
activated only by GPUI_NATIVE_TREE. Existing dormant diagnostics were extracted
to shrink the parent source. A moved trait import error was repaired before
all ordinary application gates passed: 316 library tests, eight runner tests,
all-target checking and runner build. Native algorithm sources are unchanged.

Both failing nested witnesses completed on the frozen diagnostic binary. Their
four screenshots are RGB-identical to the previous captures. The actual tree
shows both nested subgrid nodes with a single 125-device-pixel row (100 CSS px
at scale 1.25), rather than the inner authored four fractional rows. This
explains the observed 100px text constraint but does not yet locate its source:
raw parsed declarations and pre-layout node construction need inspection to
distinguish initial construction from any layout-time mutation. No gain is
accepted; preferred-stretch and stretch-bound drafts remain unadopted.

A small authored-style inspection example confirms the inner standalone row
list is already replaced before native construction: the DOM retains the
repeat(4, 1fr) attribute but its own and merged computed rows both contain a
single 100px track. The cause is the DOM parent-track copying pass, which
restricted fractional tracks by linked axis but copied fixed tracks on both
axes whenever the child had any subgrid axis.

A new regression witness fails on the old pass with one row instead of four.
The DOM gate is now restricted to the actual linked axis, preserving standalone
tracks and the existing orthogonal fractional-track restriction. A small helper
encodes physical axis mapping; the giant DOM parent source shrinks. Ordinary
application gates are running. The candidate is not yet a completed WPT fix;
focus, controls, broad comparison and accepted-gain retention remain required.


The standalone-axis DOM candidate completed all four ordinary application gates:
318 library tests, eight runner tests, all-target checking and runner build.
Its 53-pair focus comparison is 52 pass / 1 fail: both nested subgrid witnesses
are restored by exact RGB equality, with zero losses or ambiguous captures.
The 48 controls remain 25 pass / 23 fail, with no pixel-proved transitions.
The 2216-pair neighbor comparison is running on the same frozen binary.

The remaining image witness completed a separate actual-tree diagnostic.
Both inner images have the same 94.1667 by 100 CSS-pixel size and natural ratio,
but their outer holders have no ratio and occupy different automatic widths:
119.2778 in grid-lanes and 113 in the reference grid. This isolates the next
sizing contract to the replaced-element holder; a repair is not yet verified.
There are 23 restored prior regressions out of 24. The accepted ledger remains
702; 46 previously observed new greens still await the remaining gates.


An independent headless Chromium oracle loaded the real image resource and
measured 16 cases across both percentage axes, natural/authored ratios,
content/border sizing and fixed padding/borders. It confirms natural ratios
use content coordinates even under authored border-box sizing. The private
holder draft now rebuilds raw bounds and translates fixed border-box edges
into native content coordinates; two construction witnesses and a 16-case
native numerical probe are prepared. Syntax checks pass, but compilation,
geometry and WPT validation remain unrun while the neighbor GUI is active.
Unresolved border-box percentage edges and intrinsic bound keywords retain
the old route pending their native sizing contracts. No gain is accepted.


The image/percentage regression selection now contains 148 distinct official
script-free positive pairs, frozen on the current DOM binary with 220 input
documents and 273 resources; two missing resources are recorded. It is prepared
and unrun. The holder draft retains unresolved block-percentage bounds rather
than reinstating percentages omitted by the common apply pass. Three focused
construction witnesses are prepared; compilation remains unrun.

Actual-tree inspection further shows the image holders are Block nodes with
horizontal BlockFlow and zero flex-shrink. The initial unrun numeric probe's
Flex reconstruction was corrected in a new generation, including the actual
root available width and distinct lane/reference auto-flow. These controls
remain hypotheses until compiled and compared with actual observed geometry.


The two missing image resources in the 148-pair selection have now been traced
to eight input documents and checked against the pinned WPT Git tree. One is
an intentionally broken image URL in writing-mode positioning witnesses; the
other is a declared auto-repeat image URL absent from that tree. These cases
do not establish loaded-image geometry. The remaining intrinsic auto-repeat
regression uses an existing image resource and is unaffected. No corpus input
or frozen resource snapshot was modified during the active GUI comparison.


The fourth image-holder draft is now applied as an unverified candidate in
the owned source tree. It transfers the ratio to the outer box for one-auto
percentage sizing, preserves the existing flex path, and translates natural
ratio bounds into content coordinates. The parent render source shrinks by
19 lines; the new helper is 185 lines and contains three construction tests.
Syntax parsing passes, but type checks, numeric geometry and WPT acceptance
are unrun. The current frozen DOM binary and its active GUI comparison are
unchanged. Do not freeze the shared old binary as a build of this new source.


A completed prefix of the live DOM neighbor run now contains five exact
fail-to-pass capture transitions: four grid-lanes named-line cases and the
known restored nested standalone-axis witness. Independent PNG verification
confirms old test/reference RGB inequality and new equality for all five.
This is partial evidence only: the run remains live, full input/resource
verification and original-baseline attribution are pending, and the accepted
ledger is unchanged. The unbuilt image-holder candidate is a separate epoch.


The 2216-pair DOM comparison completed: 1820 pass / 396 fail versus
1815 / 401 before, with five exact fail-to-pass transitions, zero losses and
zero ambiguous captures. All 4432 captures and frozen inputs/resources are
verified; the same four missing and two unresolved resources remain recorded.
No additional gain has yet entered the original-baseline ledger.

The corrected native image-holder probe now reproduces both actual no-padding
before widths: 119.2778 in grid-lanes and 113 in ordinary grid. The inner image
has no maximum bound; only the holder does. The earlier probe incorrectly
duplicated that bound and is superseded. With the ratio on the holder both
layouts produce the correct 94.1667 by 100 size. Numeric checks retain floating
geometry precision; WPT criteria remain exact RGB equality.

The actual holder candidate completed all ordinary application gates:
321 library tests, eight runner tests, all-target checking and runner build.
Seven comparison families are frozen on the new binary. The focus WPT run
is starting; controls, image/percentage neighbors, broad gates and accepted
gain retention remain unrun on this candidate.


The actual holder candidate completed the focus comparison: all 53 pairs pass
exact RGB equality, with zero losses or ambiguous captures. The last image
witness is restored; all 24 earlier regressions are now restored in this
focus, while its 29 new-green witnesses remain passing. Controls, image/percent
neighbors, combined broad comparisons and all 702 counted gains still require
verification. The accepted ledger remains unchanged pending those gates.


The image-holder candidate completed its 48-pair control run: 25 pass /
23 fail, identical to the DOM baseline, with zero transitions, losses or
ambiguous captures. The prepared 148-pair image/percentage before run now
starts on its older frozen binary; the candidate comparison follows serially.
Broad comparisons and counted-gain retention remain required before acceptance.


The 148-pair image/percentage before baseline completed with terminal exit
zero: 104 pass / 44 fail. All 296 captures, 220 documents and 273 referenced
resources are verified; the two previously audited missing URLs remain
recorded. The candidate now runs the same ordered selection on its frozen
binary. This comparison and subsequent broad/retention gates remain pending.


### Image holder percentage ratio: column regression investigation

The completed 148-pair comparison has 102 exact passes and 46 failures versus 104/44 before: one row restoration and three new column losses (auto-repeat-auto-023/024/025). Candidate acceptance remains withheld; the accepted distinct-gain ledger stays 702. A separate completed four-pair actual native-tree diagnostic reproduces all three losses and retains the row restoration, with eight captures and unchanged local inputs/resources. Independent Chromium geometry after image load shows the ordinary-grid reference first image should be approximately 126.66 CSS pixels wide; the candidate produces approximately 135.78, while grid-lanes produces approximately 126.67. The next investigation targets grid intrinsic/final sizing and ratio transfer rather than accepting two equally incorrect pictures. Full combined comparison and accepted-gain retention remain unrun for this candidate.


### Intrinsic grid contribution cache refresh

A private native phase trace identifies a short-circuiting side-effectful change detector: after the first changed intrinsic contribution, later equal items retain stale caches. Evaluating every eligible item with a fold restores symmetric columns in the isolated loaded-image geometry reproduction: first width changes from approximately 135.78 to 126.67 CSS pixels, consistent with the independent Chromium reference. The native production source now refreshes every eligible contribution in both axes. A six-scenario all-item size/position regression assertion has been prepared for unconstrained, minimum-width and maximum-width images in grid and grid-lanes. Assertion compilation is still live; full native/application and fresh WPT gates are pending, so no candidate acceptance or distinct-gain increment is claimed.


### Cache-refresh application gates and regression witness

The new all-item geometry regression assertion demonstrably fails on the preceding native implementation: the first ratio-bearing percentage-width holder becomes approximately 135.78 CSS pixels wide instead of 126.67. The fixed actual application passes 321 workspace library tests and eight runner tests, all-target checking and runner build under the ordinary profile/default stack. The complete 316-test native library run is still compiling; WPT comparisons for the new executable have not started. The previously accepted distinct-gain ledger remains unchanged.


### Bounded preferred grid contribution

The complete first cache-refresh native run passed all 315 preceding tests but failed the new geometric assertion: max-width images had correct dimensions yet their auto rows retained height transferred from the uncapped percentage width. A private numerical correction applies explicit bounds to the resolved preferred size before transferring its ratio into the minimum track contribution. The actual native source now uses an extracted helper for that rule; the large grid-item parent shrinks. Geometry on the actual repaired code passes all 36 image dimensions and all six expected slot sets across ordinary-grid/grid-lanes and unconstrained/minimum/maximum-width variants. Ordinary-grid source order is additionally asserted; lanes may choose a different equal-looking item order from exact subpixel stack heights but must occupy every expected slot once. Fresh full native and application checks are live. The earlier application gates do not establish validation for this additional functional change, and no WPT or distinct-green acceptance is claimed.


### Transfer-only preferred contribution: completed pre-WPT gates

The bounded source-axis rule now affects only a ratio-transferred automatic destination, retaining the prior direct preferred-axis contribution contract. The full actual native suite passes 316 tests with no warnings; native debug symbols are disabled using the established native test setting, with the default test stack. The ordinary-profile application passes 321 library tests and eight runner tests, all-target checking and runner build. A fresh binary/source/input snapshot has been frozen for seven comparison/retention cohorts. The 148-pair image-percentage exact RGB run is active; no WPT completion or gain acceptance is claimed yet.


### Completed image-percentage comparison after cache/transfer fixes

The fresh exact 148-pair run completes with 107 passes and 41 failures versus the pre-holder baseline 104/44: row-auto-repeat-auto-023/024/025 become exact matches, with zero losses and zero ambiguous transitions. All three column regressions introduced by the preceding holder-only candidate are restored. Compared with that rejected candidate the count rises from 102 to 107 with no losses. All 296 captures, ordered pairs, 220 input documents and 273 local resources are verified; the two pre-existing missing resource paths remain unchanged. The 53-pair retained-fix focus is now running. Broad primary/supplementary comparison and 702 accepted-gain retention remain required before ledger acceptance.


The new candidate also completes the 53-pair retained-fix focus: every pair remains an exact pass, with zero transitions/losses and no ambiguity. Controls are now running. This bounded preservation check does not replace broad comparison, original-baseline attribution or the 702-gain retention run.


The 48-pair control run completes unchanged at 25 passes and 23 existing failures, with zero exact transitions and no ambiguity. The broad 1324-pair primary run is active on the same frozen binary, followed by supplementary comparison, affected native grid neighbors and accepted-gain retention as applicable. Distinct original-baseline attribution and ledger acceptance remain pending.


A read-only priority audit of the last completed physical-flow primary and supplementary runs finds historical failures concentrated in grid, writing modes, multicolumn, flexbox and sizing. The prepared 509-pair inline-decoration projection cohort intersects 82 historical failures. This is an overlap inventory, not a predicted gain count: intervening repairs can already restore historical failures, and the draft only targets rotated paragraph decorations. Current broad results must refresh this ranking before implementation/adoption. The primary comparison is still active, with no completed broad or ledger acceptance claim.


### Styled vertical paragraph boundary preparation

Read-only source inspection confirms that native vertical parent-time paragraph measurement currently requires top-level text nodes; styled inline descendants use the rotated fallback even though the inline text-run path supports text and overlay pieces. Any future expansion must classify actual collected pieces once, preserving atomic/replaced physical orientation and out-of-flow positioning, rather than infer capabilities from tag names. A fresh current-epoch 509-pair baseline is frozen with 777 declared input documents and 810 local resources (none missing) for inline-decoration/styled-paragraph comparison. It is unrun; the main 1324-pair broad run continues on the existing validated binary. No route expansion, predicted gains or ledger acceptance is claimed.


### Original-baseline replay and recovered intrinsic/print regressions

The completed 1324-pair primary cohort has 862 passes and 462 failures; the completed 3085-pair supplementary cohort has 2433 passes and 652 failures. Their neighboring-pair capture join covers all 2216 scheduled neighbors with 1823 passes and 393 failures, three improvements and no losses relative to the preceding completed neighbor cohort. This join does not claim an independent neighbor-order execution. Six equal-RGB failed pairs retain their explicit no-red guard failures.

An immutable original-binary replay of all 272 unaccepted candidate pairs proves 73 original failures/current exact passes and 199 pairs that already passed originally. All 413 declared documents and 437 local resources match the completed current source cohorts; all 544 original replay captures are validated without naming collisions. The 73 candidates remain unaccepted pending current broad preservation; they are not a 272-gain claim.

The completed accepted-702 retention run exposes one print failure. A second apparent capture discrepancy is an allowed alternate-reference match and is proven against that declared reference. A separate original-green flex regression measures a wrapped maximum of 90 CSS pixels below its 100-pixel intrinsic minimum. Its native correction passes 317 tests without warnings and restores the exact flex WPT pair with no loss across 23 controls. A subsequent correction keeps the legacy vertical cross-end default only on its physical X axis: 322 application library tests, eight runner tests, all-target checking and runner build pass. It restores the print pair exactly, with no loss in the same 23 controls.

The current combined 775-pair validation contains the historical 702 accepted gains plus 73 unaccepted genuine candidates. It is running with the explicit 125ms animation setting. The ledger remains unchanged; neither its completion nor broad regression absence is claimed. Styled vertical paragraph/decoration expansion, GPUI migration, and full script-free reference-graph execution remain unfinished.

### Corpus URL routing preparation

A private URL router preserves query/fragment identity independently of the physical corpus path, decodes filename escapes once, and refuses ambiguous Windows paths or device aliases. Its four new unit tests pass alongside the existing sixteen parser/RGB tests. Physical routing checks pass for all 20974 static official relations and all 305 URL-state inventory relations, preserving 602 stateful URL occurrences. This checks file routing only: the entire URL-state inventory has not been asserted script-free, and URL-driven DOM or fragment navigation is not implemented or rendered by this probe. The router is not integrated into the production runner and contributes no accepted WPT gains.

### Current candidate retention completed

The combined 775-pair execution completes with every pair passing. Full input/resource and capture verification confirms all historical 702 accepted gains retained and all 73 originally failing candidates still passing, with no failed pair. One success uses an exact declared alternate reference; its source identity and capture are verified separately. The explicit 125ms animation setting is preserved. Broad preservation on the current final source remains required before accepting the 73 candidates into the ledger. The fresh 509-pair pre-change baseline for styled vertical paragraph/decoration work is now running on this same executable; the private feature draft remains unadopted.

### Border inner-edge geometry preparation

A private numerical draft derives border thickness from the same unrounded absolute edges used by abutting child layout; three checks pass, including a witness that an already rounded origin produces the wrong inner edge. A separate DPI probe checks 320010 device/logical/device conversions and finds 1744 changed rounded edges at several non-integral scales. The prepared GPUI carrier therefore preserves native physical coordinates directly through prepaint, paint and deferred context restoration. Its new modules parse and format, and production source guards remain unchanged. This is an uncompiled, unadopted draft: overflow clipping and real border capture behavior still require validation, and no WPT gain is accepted.

### Styled vertical paragraph candidate applied

The fresh 509-pair before-run completes at 262 passes and 247 failures, with all 1018 captures, 777 input documents and 810 local resources verified and no missing or external resource. The six-file styled-text/native-paragraph and physical-decoration candidate is now applied. Its classification uses actual collected pieces once and keeps atomic or out-of-flow content on the existing physical placement path. All native/GPUI source identities and the preceding physical flex cross-axis correction are preserved. Application gates pass 326 library tests and eight runner tests, all-target checking and runner build. Feature capture comparison and broad preservation remain unrun; no new gain is accepted from compilation or unit tests.

The fresh candidate's same-input 509-pair comparison is now active. Nine frozen feature/control/broad/retention cohorts preserve the build and input identities; their completion is not inferred from freezing. The historical accepted ledger remains unchanged, with the preceding 73 genuine candidates still awaiting final broad preservation.

### Border clipping and migration source refresh

The private border carrier now resolves padding-box overflow masks from the same physical edges as border paint and inset shadows. The public mask API for callers without window/layout context retains its continuous geometry. New leaf modules format and all candidate Rust files parse; production GPUI remains unchanged, so no rendering fix is claimed yet.

A fresh private GPUI migration workspace carries all 127 current native Rust source files, including the verified wrapped flex intrinsic floor. Its offline locked graph resolves 29 workspace members and 833 packages with exactly one local Taffy 0.14 and an unchanged lockfile. This core-only workspace still uses the previously pinned upstream snapshot; latest-source revalidation, core/Windows typechecking, application/component integration, exact WPT and CEF/IME acceptance remain required. The applied HTML candidate is recorded separately and must be preserved during integration. No dependency-upgrade WPT gain is claimed.

### Styled509 exact comparison completed

The candidate completes the same 509 pairs at 273 passes and 236 failures, versus the before-run's 262/247. All eleven transitions are exact RGB improvements, with zero losses and no ambiguous capture naming. Every one of the 273 reported passes is independently checked against its capture pixels, and all input/resource/capture identities are verified. These eleven are improvements against the immediately preceding version; distinct original-baseline attribution is still pending. An original-binary eleven-pair replay is prepared with matching snapshots for all eighteen documents and eighteen resources and no missing or external resources; it is unrun.

The candidate's 775-pair retention execution is now active with the explicit 125ms animation setting. Final broad preservation remains required; neither the eleven feature transitions nor the preceding 73 genuine candidates are newly accepted into the historical ledger yet. A refreshed historical ranking of the completed broad cohorts covers 4409 distinct pairs and 1114 failures, concentrated in grid, writing modes, multicolumn and flexbox; it describes that measured source epoch and does not forecast current gains.

### Original attribution and current geometry experiments

The styled candidate's replacement 775-pair retention execution completes with all pairs passing, including the historical 702 and the 73 original failures. The original-binary eleven-pair replay also completes: all eleven failed originally and pass exactly on the styled candidate, with no overlap with those 775. This establishes 786 distinct observed original-to-green transitions; the historical accepted ledger remains 702 pending broad preservation.

The subsequent vertical float change restores intrinsic container height in the legacy rotated path. A physical projection of inherited inline-block dimensions and edges follows. The combined candidate passes 329 application library tests, all-target checking and runner compilation. Its twelve-pair focused run retains one pass and eleven failures: no new exact greens and no lost passes. Several baseline mismatches decrease, while two increase against the float-only candidate. These geometry experiments do not establish baseline correctness or preservation of the preceding 786 transitions. A 1324-pair primary execution is now running on the combined candidate to measure its wider effects; no result is claimed before completion.


### 2026-10-06: vertical atomic contexts and margin adjacency experiments

The completed shallow-geometry primary1324 run had 869 passes / 455 failures
(+11 already-known styled gains, -4 against completed923; no new unique gains).
The physical-frame candidate passed 124/195. Removing its adapter restored two
baseline controls: 126/195, retaining all 11 styled gains, with one grid-margin
regression against completed923. These experiments do not prove preservation.
Current experimental source adds actual sideways descendant baseline export,
preserves grid/flex item margins, separates margin collapse at inline boxes and
text lines, excludes out-of-flow boxes, and seals formatting-context boundaries.
All 334 library tests, all-target check, and runner build passed. Smoke28 is
14 passes / 14 failures, zero new unique gains. Two baseline controls still fail.
A writing-modes1070 expanded measurement is being prepared; no result yet.
The accepted ledger remains 702; historical distinct exact observations remain
786. All diagnostics and frozen captures are private, outside the checkout.


### 2026-10-06: atom-only paragraphs have no text transform

The writing-modes1070 experiment was intentionally stopped at 232 rows
(runner exit -1; incomplete, not an accepted family). The diagnostic native
trace confirmed that synthesize-vrl-baseline has no VerticalText wrapper.
Its atom sizes therefore must remain physical. Clearing the whole rotated
flow flag restored synthesis but broke wm-propagation-body-051; that approach
was replaced by a scoped, unwind-safe suppression of the dimension adapter.
The final scoped candidate passed 335 library tests, all-target check, and
runner build. Smoke28 passed 15/28: synthesis and the image control are exact,
all eleven styled gains remain exact. Two known grid-baseline controls still
fail; no unique green increase or general preservation claim. Failed writing
modes plus smoke controls form the next 336-pair measurement. Ledger unchanged.


### 2026-10-06: user-requested pause after current baseline correction

The inline-block last-line baseline policy now selects the last physical X
baseline independently of Y. Exact current native127 source passed 317 unit
tests; app library335, all-target check and runner build passed. The frozen
runner d0c88df04a3620d1e4cc09071f39334cfd0f6ab8b38ff1ed23d925ca565e7105
completed focus40: 15 passes / 25 failures, zero gains/losses against the
completed f02 controls. The preceding failures336 run completed 15/336, with
no new unique gains beyond the historical eleven styled observations.
Two grid-baseline controls still fail on the experimental physical-atom path;
current-source full775 preservation and broad acceptance remain unproven.
The user requested stopping after this fix. No further fixes or runs are
scheduled. Accepted ledger remains702; all-WPT goal is incomplete and paused.
Current work is uncommitted in the existing codex/wpt-engine-recovery worktree.
