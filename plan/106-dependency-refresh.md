# Dependency refresh and compatibility

The owner requested updating all libraries alongside the WPT engine work and
checking that the update preserves application behaviour. This is development
work in the isolated recovery branch, without a release-version bump.

## Starting evidence

The existing frozen fragmentation baseline and candidate remain unchanged.
Their comparison must finish before resource-heavy dependency builds or a
second live GUI scenario starts. Dependency metadata resolution can proceed
while the frozen runner is rendering.

Cargo's fresh registry resolution updates 81 compatible package entries.
The lockfile update has been applied, with workspace package versions verified
unchanged. Compilation, library/application tests and the WPT controls remain
pending for this dependency generation. Private before/after lockfile hashes
and package deltas are retained outside the checkout.

The direct registry audit also identifies newer CEF, zstd and gpui-component
generations. The vendored Taffy and html5ever generations have newer upstream
releases. GPUI, resvg and markup5ever_rcdom are already at their current
registry generations; their local functional patches must remain intact.
Updating a version label is not sufficient evidence of migrating a fork.

## Execution and acceptance

1. Inventory every Rust/npm manifest and lockfile, resolved versions, peer
   requirements, toolchain limits, local forks and generated runtime outputs.
2. Update ordinary compatible dependencies with reproducible lockfiles.
3. Migrate supported major versions and adapt their callers. For local forks,
   compare pristine old/new upstream sources and account for every local
   functional patch before replacing implementation files.
4. Run the applicable Rust and Node gates from CONTRIBUTING.md. Native modules,
   CEF and UI/layout changes also require targeted application checks; a build
   alone cannot establish runtime compatibility.
5. Rebuild committed extension/webview artifacts from the updated sources.
   Verify packaged runtime dependencies reproduce the tested lockfile graph.
6. Compare the engine controls before/after the dependency update under the
   same exact comparison and capture settings. Report regressions and retained
   compatibility constraints explicitly; never count oracle or scope changes
   as engine gains.

## Identified migration boundaries

- The latest typescript-eslint peer range excludes TypeScript 7. Keep the
  supported TypeScript generation until a compatible lint stack is available;
  do not force an invalid peer graph.
- xterm 6 and the old CanvasAddon fallback have incompatible peer ranges.
  Migrate the fallback while preserving terminal rendering and interaction.
- The runtime payload cache currently identifies dependency ranges rather
  than the installed lockfile graph. Make its cache and install use the exact
  tested runtime dependency resolution so dependency updates reach the package.

Checks below are recorded only after their commands complete.

## Current checkpoint

All five npm graphs have been installed with lifecycle scripts and without
force/legacy peer overrides. The full root `npm run check`, extension checks,
webview checks (216 tests), server checks (178 tests), dashboard build and
generated extension/webview bundles pass. Five runtime projection tests pass.
The development runtime payload's 40 installed versions match the lockfile;
its cache reuse and `--check` pass. This is not a production package gate.

Native SQLite Unicode transactions and PTY Unicode input/resize assertions
pass. Natural PTY exit leaves a ConPTY worker alive; the previous installed
version reproduces that lifecycle issue. Explicit late cleanup emits a helper
AttachConsole error, so terminal lifecycle acceptance remains open. Live
Windows terminal rendering/input/context-loss checks also remain pending.

The Rust lockfile now also resolves CEF 154 and zstd 0.14. html5ever has been
migrated to 0.40.1 with its processing-instruction patches retained; the rcdom
fork's dependencies are adapted to the 0.40 parser generation. The fork's own
base version is retained, rather than representing an unpublished upstream
rcdom release. Library compilation required four explicit string-atom
comparisons after the new string_cache API introduced a second `AsRef`
implementation. After adapting them, 188 library tests pass; the existing
style-consumer audit still fails on `justify_items_safe` and
`justify_items_last`. `cargo check --workspace --all-targets --locked` passes,
including CEF 154. All three full html5lib suites (tree construction,
tokenization and encoding) pass. Application linking and selected live CEF
controls now pass as detailed below; full application acceptance remains open.

The isolated before/after dependency WPT controls complete all 214 ordered
pairs with identical verdicts: 212 passes and the same two pre-existing geometry
losses. All 852 retained captures have the expected 1000x750 dimensions. The
426 corresponding capture pairs are byte-identical before/after the update.
Engine patch sections differ only in the required dom/counters_scan atom type
adaptation; capture code differs only in its module documentation. This selected
control set does not establish full-corpus or live application compatibility.

The combined updated-dependency and raw-origin geometry revision also completes
the full same-list fragmentation controls: 876 passes, 741 failures and 97
exclusions, with 152 gains and no losses against the guarded baseline. All
3,206 unique captures have the expected dimensions; 28 existing capture-name
collisions limit independent image retention. Dependency-only isolation above,
rather than this combined run, establishes unchanged output for dependency
controls. The full workspace test gate stops on the same HTML consumer audit
(188 passes, one failure); the separate workspace run excluding HTML passes,
including the parser and application unit tests. Application binaries are now
successfully linked with CEF 154. An isolated interactive application control
loads and paints a local browser document, accepts Cyrillic key input through
the application probe, replaces it using Ctrl+A, and preserves input/DOM scroll
across resizing. Closing its main window exits the application and all recorded
direct children. This does not establish native IME, clipboard, mouse-wheel or
terminal acceptance. Duplicate unchanged-value input events also reproduce with
the historical 1.0.61/CEF 150 binary using the same current Node host; its exact
input-event sequence matches the CEF 154 control. That historical comparison is
not a clean isolation of every dependency. The format gate still
reports differences, including inherited HTML source formatting and refreshed
upstream html5ever formatting; it has not passed. Strict workspace Clippy also
fails on current warnings/lints and has not been accepted or suppressed.

Taffy 0.14 remains migration work. Its unresolved private draft has not been
copied into the repository. Preserve each local patch by behaviour rather than
blindly choosing an old or new file wholesale.

The private tree port now retains native 0.14 measurement callbacks returning
LayoutOutput, native containment and cache keys, alongside registered opaque
calc handles, separate first/last x baselines, subgrid track publication and
anonymous-row percentage-basis cache invalidation. Rustfmt parses the resolved
tree modules. Three standalone std calc-registry tests pass, including unknown
handles and pixel-only values with an unknown basis. The native Dimension
tagging test, full Taffy compilation, algorithm callers and GPUI integration
remain unverified; these checks do not accept the migration.

The private grid item, alignment and track-baseline modules are also resolved
and parse with rustfmt. Four axis placement controls and ten actual native
alignment/type controls pass in a lightweight harness using the migrated axis
helper. This harness does not compile the full grid sizing algorithm. The port
keeps first/last row groups separate, applies native auto-margin/cyclic-size
eligibility on independent x/y axes, and clamps scroll-container baseline
coordinates to their border boxes. Linked subgrid axes ignore authored sizing
keywords before native intrinsic-size resolution. The main grid algorithm,
flexbox and block algorithms still contain conflicts; lanes and GPUI callers
also need integration. No unresolved draft is used by the live WPT runner.

The private block module now resolves onto native 0.14 float/BFC handling,
dimension definiteness, deferred align-content placement and signed overflow.
Both first/last y baselines follow the alignment shift; x baselines remain
independent. Six isolated actual helper tests pass for auto margins and
scroll-container baseline exposure. Absolute auto margins require both insets;
negative inline space goes to the logical end, while negative block-axis space
is divided equally, correcting the older local closure. The inline-block
non-visible-overflow fallback is adapted at its own baseline boundary. Full
block compilation and float/containment/align-content runtime verification are
still pending. Only the main grid and flexbox files retain conflict markers;
marker-free modules and lanes still require a complete integration review.

The private shared subgrid placement helper now excludes hidden/absolute
children from both implicit-grid estimates and named-line estimate widening,
retains native auto-repeat counts for track initialization, and applies the
same native track bound to inherited spans, name expansion and initialized
tracks. This avoids converting an oversized inherited u16 span into a negative
i16 placement bound. Rustfmt parses the helper; its three prepared controls
have not run. Main grid callers, detailed layout metadata and native Direction
integration remain pending.

The private main grid now resolves its twelve conflicts and parses with
rustfmt. It consumes shared placement auto-repeat counts and detailed names,
preserves native signed overflow/RTL placement, maps physical margin-trim sides
for RTL, and keeps eligible first/last baseline groups separate. This does not
prove compilation or runtime compatibility: flexbox conflicts, lanes and
subgrid coordinate adaptation, the GPUI safety boundary and the existing
empty-table/container-baseline issue remain outstanding. The draft is not used
by the live WPT runner.

gpui-component 0.5.1 is the latest official generation compatible with our
patched GPUI 0.2.2. Every newer stable 0.6/0.7 generation requires gpui-pre and
gpui-base; 0.7 also changes editor state types and Root. Missing accessibility,
touch/scroll/text APIs and our patched TextRun fields rule out a simple package
alias. A complete coordinated engine migration is required before updating it.
Private semantic maps account for all eleven local component patches.

The macOS cocoa/core-foundation pins cannot be updated independently: upstream
gpui_media also pins core-foundation 0.10.0 exactly, and Cargo rejects the newer
resolution. These pins are retained; macOS runtime compatibility is unverified
on this Windows host.


The private grid-lanes interface port now carries signed scrollable-overflow
rectangles, child containment and native Baselines, and passes the shared
explicit-grid auto-repeat count. Canonical alignment objects replace the old
enum matches and separate safety hooks. Stacking content uses the native
single-subject distribution fallback; 17 isolated actual alignment/helper
checks pass, including three new stacking controls. These checks do not compile
the complete lanes algorithm. Native RTL projection (the existing physical
projection is explicitly retained with Direction::Ltr), intrinsic sizing and
known-axis definiteness, and full container baseline selection remain pending.
No private migration source is adopted into the runner or counted as WPT gain.


The bounded private flexbox port resolves 16 of its 40 conflicting sections;
24 remain. Reviewed sections preserve native Direction, known-axis definiteness,
Contain and signed overflow, alongside independent first/last baseline outputs.
Only a synthetic unit of the reviewed sections parses with rustfmt; whole-file
parsing and compilation remain unproven. Further sizing/baseline sections are
being ported before adoption. The HTML adapter already maps horizontal RTL rows
and vertical/sideways axes to physical flex directions; GPUI currently has no
Direction field. Its horizontal RTL nowrap-column flex_cross_reverse flag is
therefore a physical-axis compatibility contract. A native Direction migration
must coordinate that adapter with the algorithm to avoid applying RTL twice.


The private Taffy port now has no remaining conflict markers. Absolute flex
layout composes the local both-auto ratio/content-floor path before native
keyword final sizing, retains padding-box/signed auto-margin free space and
resolves negative inline auto margins to native RTL/LTR logical end. With no
sharing context, first/last abspos baselines use implicit safe self-start/end
per CSS Align 3. A dead constants field is removed without suppressing warnings.
Grid-lanes now resolves native sizing keywords before normal alignment and
masks linked subgrid axes before probing. Five actual TaffyTree controls cover
masked/no-basis/no-probe/exact stretch, known-axis precedence and measured
border-box size.

The exact current private library passes 197 default-feature tests and 214
with parse/serde. All-feature library checking and the configuration without
content_size pass without warnings. Test APIs use native TaffyView, typed Style,
MaybeResolve and explicit independent local defaults; assertions are preserved.
The upstream memory-size test now exactly accounts for the added inline
payloads and flags: String Style is 824 bytes and Arc<str> Style 792 bytes,
264 bytes over native values. It retains exact nested/total size assertions.
Serialization derives for local lanes/subgrid metadata and error propagation
in the no-content debug writer close the observed feature configuration gaps.
These are library acceptance checks, not WPT gains or acceptance of the whole
engine migration. GPUI boundaries, workspace checks and live behavioral
controls remain pending; the production runner still uses the earlier frozen
engine and the confirmed unique exact WPT gain remains 152.


The verified 56-source Taffy 0.14 port is now adopted into the isolated engine
worktree. GPUI pins the local 0.14 package and maps legacy safety/wrapping/
containment flags to native alignment, Balance/BalanceReverse, flex_line_count
and Contain. Its measurement callback uses native compute_leaf_layout with
content-box measurements and independent first/last baselines. The workspace
lock delta contains only removal of grid 0.18 and replacement of local Taffy
0.9 with 0.14. The actual alignment adapter passes its three isolated tests
against the adopted source. GPUI library type checking is still running;
workspace and live rendering acceptance are pending. The serial WPT candidate
continues with the unchanged frozen earlier engine, so this adoption has no
accepted WPT gains yet.


The vendored package now also carries native 0.14 README/VCS provenance,
the exact independently verified library lock and all ten upstream example
sources. The former examples were compared with pristine 0.9 and have no
local functional delta. All-feature example checking passes with exit 0,
covering both built-in TaffyTree and custom-tree public API consumers. This
supplements the 197/214 library tests; GPUI/workspace/runtime acceptance is
still separate and pending.


The initial adopted GPUI/Taffy integration library check completed with exit 0.
Nine warnings remain in the existing text-system paths; this is not a zero-
warning gate. After that check, justify-items/self safety is carried into native
alignment objects as two additional channels. The CSS adapter projects safety
with values for vertical grid containers and parents, and forwards last-
baseline keywords; three new CSS mapping controls are pending execution.
A workspace/all-targets locked type check is running on this newer source.
It supersedes the initial library check for current-source acceptance. No
Taffy-update WPT gains are accepted before rendering/regression controls.


The current six-channel safety/last-baseline integration passes the locked
workspace all-targets type check with exit 0. Existing compiler warnings still
prevent a zero-warning gate. Full HTML library tests are now running; the
updated runner still needs rebuilding and exact rendering/regression controls.
The separate frozen prior-engine axes run adds 35 independently pixel-proven
gains, so +187/-0 is confirmed before any Taffy 0.14 runtime acceptance.


All 192 HTML library tests pass on the six-channel integration, including
coverage's written-but-unread field audit and the three new CSS axis/keyword
controls. The updated WPT runner builds successfully. Its initial 404-pair
exact rendering/regression cohort contains all prior 187 engine gains plus
baseline controls and targeted alignment/sizing cases; the selected old
baseline is 332 pass/71 fail/1 outside. New indexed capture names prevent
collisions. The control cohort is not an additional engine-gain ledger or a
whole-corpus run; live acceptance is still pending.


The first native rendering cohort is complete but rejects migration acceptance:
six improvements and six regressions preserve the numerical 332/71/1 totals.
All 806 indexed screenshots are valid and collision-free. Five regressions
reproduce independently. Two now pass exact RGB on the repaired runner; the
native library passes 201 tests including real projected/native RTL safe
alignment and wrap-reversal controls, with a verified failing old-code control.
The image helper keeps automatic dimensions for percentage-axis resolution
and shrinks the existing image module. Three remaining rendering failures
(scroll baseline 1a/2 and external-CB safe003), float snapshot stability and
broader regression verification remain adoption gates. Component migration,
full application compatibility and zero-warning checks remain incomplete.


All 192 HTML library tests also pass after these repairs. The frozen repaired
runner is now executing the broader 404-pair exact comparison; its outcome
is pending and cannot yet establish migration acceptance.


The repaired 404-pair native cohort finishes with 333 pass/70 fail/1 outside,
six improvements and five losses against the prior selected engine. Native
baseline fixes then pass 214 default and 231 parse/serde tests, all-features and
without-content-size checks, the actual workspace/all-targets check and runner
build. A frozen 106-pair baseline rendering cohort has five improvements and
zero losses against that repaired native candidate, restoring both scroll
baseline cases and improving three subgrid cases. Migration acceptance remains
pending the larger axes family and unresolved rendering losses. Dependency
expansion is deferred while engine progress and regression acceptance take
priority; component/app/zero-warning gates are still open.

The owner explicitly requested updating GPUI after the nearest vertical-flow
repairs. This supersedes deferring the engine transition indefinitely. First
validate and freeze the repaired current engine; then port the functional GPUI,
Taffy and component changes onto a pinned current engine source with one shared
GPUI type identity. The published gpui package and newer Zed source snapshots
must be distinguished: GPUI Kit/component 0.7 pins the gpui-pre 0.3.7 snapshot
family, including platform and macros. Compare that supported release graph
with current Zed source before selecting the migration target; do not introduce
two engines or remove local functionality to make the update compile. Completion
requires application checks, editor/overlay/CEF runtime validation and exact
affected-family plus accepted-gain retention checks on the migrated engine.

The owner also requires assessing what the new GPUI version actually brings
to this application. The migration audit must map relevant upstream changes
to text shaping/wrapping, Windows rendering and input, scroll/selection,
editor and overlay behavior, and accessibility. For each item record current
behavior, upstream implementation, expected application benefit, porting cost
and the check that would prove the benefit. Identify local patches already
superseded upstream, but remove them only after equivalent behavior is verified.
New APIs or release-note claims alone do not establish faster rendering or
additional exact WPT passes.

The initial read-only source audit preserved the complete local GPUI delta
against pristine 0.2.2, including earlier committed changes. Raw changed-path
counts include formatting and must not be presented as functional patch counts.
The supported newer snapshot splits core, platform dispatch and Windows backend
into separate crates. Its Windows backend implements ClearType/grayscale glyph
analysis and a dedicated subpixel blend pipeline, plus AccessKit integration.
These are concrete capabilities, but application readability, accessibility
and performance still need runtime verification. Subpixel blending requires
special care for the existing transparent overlay surfaces.

The newer core's measured-layout callback still returns size alone and its
TextRun does not provide our per-run font size or CSS inline background geometry.
Our physical baseline metadata and text patches therefore still need explicit
ports. The supported snapshot depends on Taffy 0.13 while the local engine uses
patched 0.14; preserving the native fixes requires adapting that dependency and
its interfaces rather than silently introducing an older parallel layout engine.

The concrete benefit comparison now distinguishes core GPUI from Kit/base
changes. Two high-priority Kit fixes are present in downloaded 0.7 sources and
absent from the current component implementation: tree-sitter callbacks slice
Rope chunks as bytes rather than slicing a string at arbitrary byte offsets,
and cursor pause rejects an unfocused lifecycle while blur clears pending
blink state. These provide specific reliability and repaint checks for the
migration; no application crash reproduction or CPU saving is claimed yet.
Independent edit-tracked range-decoration collections are a useful optional
editor integration for diagnostics/search. Native accessibility, Windows text
rendering and scroll-axis filtering still require actual application consumers
and runtime checks. Upstream tests were inspected but have not been executed
for the new engine. None of these source findings count as WPT gains.

The current source target is pinned to Zed revision
`eec3376016e83b79a574039fb66107d3801741f6`. Its complete in-repository source
dependency closure has been preserved as private migration input and verified
against independently fetched files. The upstream core still labels itself
`gpui 0.2.2`; its source revision, rather than changing that label, identifies
the update. Component/base 0.7 must use the same local current core and platform
graph after their published snapshot aliases are adapted. Adoption, compilation
and application acceptance remain pending.

Comparison with Kit's released snapshot identifies additional current-source
changes: cached element-path hashing, character-input preference passed through
keyboard interception/observation, and GPU-independent Windows font shaping.
These have specific frame/state, text-entry and native font-test acceptance
scenarios; no measured application speedup is claimed. Upstream measured sizes
and authored metrics are pre-snapped, and midpoint rounding differs from our
CSS path. The local fractional measurement, absolute origins and physical
baseline contracts must therefore be ported explicitly.

A private three-way draft preserves every local changed path, including older
committed patches. Its semantic conflicts remain unresolved and it is not an
adopted engine. The local backdrop-blur shader build entry has been relocated
to the new Windows platform crate without dropping its subpixel shader entry
or the core manifest/leak-detection logic. Actual shader, renderer and transparent
overlay checks are still required.

The current private text port now combines editor indentation adjustments with
our separate CSS wrapping policy, including hanging spaces, unbreakable fragments
and shaped intrinsic widths. The line-cache port retains upstream font-generation
invalidation alongside local letter-spacing keys and per-run font sizes; splitting
a shaped line carries each run's size into both halves. These resolved modules
parse, but cross-platform constructors, text painting and complete type/runtime
checks remain pending. They are not adopted by the running application.

The private text entry-point port also retains both new lazy hash shaping and
the local logical-to-visual RTL projection. Optional line-clamp counts use
upstream saturating subtraction while retaining the spacing argument. Both
hash layout/cache probes carry effective per-run font sizes, and single-character
width probes initialize them explicitly. Extracted methods parse; the parent
has now also resolved both shaping-policy conflicts and parses completely.
Default shape/layout calls preserve native decoration boundaries, while explicit
spaced CSS calls retain shaping continuity across paint-only changes even with
zero tracking. Effective run sizes remain part of font-run identity. Actual
ligature, kerning, paint-boundary and editor-selection checks are still pending.

The existing missing no-break-hyphen glyph substitution is shared by ordinary
layout and lazy hash-cache misses, preserving font-run checks and equal UTF-8
byte lengths. Cache hits still avoid materializing text. The complete text
module and extracted helpers parse after a module documentation/order correction;
this does not establish graph compilation, native font/cache fixture results
or runtime compatibility. No WPT gain is claimed for this private port.

The private shaped-line paint module now resolves its five conflicts and parses
as a whole. Native underline ranges, aligned layer bounds, glyph vertical offsets
and background termination compose with local justification, scaled masks,
inline padding/border overflow and continuation edges. Native cursors carry
per-run font sizes, and both cursor extraction and direct splitting preserve
inline decoration geometry. Three prepared native controls cover mixed-size
UTF-8 cursor chunks, split decoration retention and justified paint bounds;
they are syntax checked but have not been compiled or executed. Platform and
renderer integration plus actual editor/HTML rendering remain acceptance gates.

The private Windows DirectWrite port composes ten of its sixteen conflicts.
It retains current GPU-independent shaping state and separated component
ownership alongside local font-data lifetime, face/stretch metadata, effective
first-run size, mutable draw context and the vertical text-renderer interface.
Native nullable-array guards now run before local vertical metrics read glyph
IDs; vmtx advances use the validated slice. Three extracted interface/array/
metric leaves parse, while six main font-selection and mixed-run conflicts
remain. Whole-backend compilation and actual headless-font, mixed-size, vertical
and Windows raster verification are still pending. The production renderer
does not use this unresolved private draft.

The next private Windows port resolves the three font-selection conflicts onto
native collection/COM-face caching and system-font refresh. Requested stretch,
synthetic-form filtering with a real normal-face retry, physical PostScript
identity, vertical/OpenType keys and styled system-family fallback are preserved.
The first-font fallback is used only when a registered font exists. Core/platform
conversion helpers remain explicit across the new crate boundary. The complete
selection leaf parses, leaving three mixed-run/callback conflicts in the backend.
Extracted helper visibility is limited to the enclosing text/platform module
so sibling callers can access shared policies without exporting new public API.
These are source ports, with backend compilation and native fixtures pending.

All sixteen private DirectWrite conflicts are now composed and the complete
backend parses. FontRun carries an explicit paint-boundary flag in its derived
cache identity: default/hash editor paths preserve native separation, while CSS
spaced paths retain continuity independently of tracking. Physical-face folding
does not cross an explicit boundary. UTF-16 size ranges restore the requested
raster size after DirectWrite's shaping-only float adjustment, and the existing
requested-font ascent/descent contract remains intact. Checked native callback
face resolution, vertical metrics, font lifetimes and optional GPU state remain.

Thirty-three reviewed external FontRun constructors now initialize the added
fields. Cosmic clipping and web native-ID remapping copy existing metadata;
original tests/benchmarks inherit their authored line size and retain native
boundary intent. These sources parse, but macOS/Cosmic/web shaping implementations
still need to consume the metadata. The prepared effective-size/boundary control,
complete current graph compilation, native fixtures and real application/exact
WPT checks remain pending. The running renderer still uses the frozen earlier
engine and these private ports have no accepted WPT gains.

For application value, prioritize the Unicode parser and unfocused-caret fixes
first, then verify font-cache invalidation, character-input handling and Windows
text rendering. Edit-tracked decoration collections can serve search and LSP
diagnostics after editor migration. Accessibility and scroll-axis filtering need
explicit shell consumers and runtime checks. Root plugins are an integration
option for component overlays, with the separate transparent native window and
CEF ownership preserved. New form/chart widgets have lower priority for the
current engine and editor work. Neither the migration nor these capabilities
establish an application speedup or additional WPT passes before measurement.

The private GPUI/Taffy adapter now composes all twelve source conflicts and
parses as a complete module. Physical X/Y first/last baselines, shared callback
identity, independent measurement subtrees and fractional content/border sizes
remain in the local Taffy 0.14 contract. Legacy size/Y callbacks adapt without
inventing X baselines; native optional stack guards and timing counters compose.

One native Point<f32> cache now holds unrounded absolute origins, including
fractional root offsets and placed-subtree overrides. Placement and layout
invalidation use that same cache. Exact dimensions remain available for float
bands and inline fitting; painted dimensions come from rounded absolute edges.
The existing ties-away-from-zero policy is retained because current GPUI's
ties-toward-zero policy differs at half-device-pixel coordinates. Four existing
native controls for fractional roots, subtree movement, cache invalidation and
unrounded intrinsic contributions remain prepared, but have not run on this port.

Complete graph compilation, native length/stroke/renderer integration, both
stacker configurations and actual application/exact WPT retention remain pending.
Production must enable stacker to preserve the previous deep-tree guard. This
private source port has not replaced the running renderer and adds no accepted
WPT greens.

The private text-element port composes both remaining conflicts and parses
with an extracted measurement leaf. It preserves current accessible text IDs,
foreground/background paint passes and start/middle/end truncation. The new
native guard prevents cached truncated text from answering an untruncated
intrinsic query; local exact wrap identity, primary-font baselines and isolated
minimum-content probes remain. Failed shaping during a minimum-content probe
also preserves already prepared paint state. This is a concrete useful upstream
fix for labels and intrinsic sizing, but native cache fixtures, full compilation
and actual editor/HTML runtime verification are still pending.

The private core facade also composes both conflicts and parses independently
of its unresolved child modules. Current profiler/debug-overlay and SVG exports
remain alongside local physical-measurement types, HTML raster helpers and the
existing frame/layout/shaping/cache counters. Counters move into a small module
without changing their public path. This does not establish graph compilation
or application adoption.

The private platform/SVG port composes three conflicts and both complete modules
parse independently. Native external drag remains alongside the CEF D3D device,
context and external-texture lifecycle contract. Current parse-once SVG APIs
retain rasterization at different scales. Clipboard SVG now uses the shared
native path, which already performs the local BGRA/unpremultiply correction and
smooth raster density with logical scale metadata. Local HTML SVG nonuniform
scaling, raster caps, image crops, raw BGRA conversion and PNG ICC extraction
remain exported through a small helper module. Native scene/renderer integration,
actual colors/transparency/density fixtures, CEF texture ownership and complete
application verification remain pending; no runtime adoption or WPT gain follows
from these source checks.

The next renderer audit found a concrete unresolved ABI issue in the private
draft: upstream checkerboards and local radial gradients both use background
tag 3. Core background instances carry four stops and a stop count, while the
active WGPU shader still declares two stops and padding. HLSL conflicts and the
Metal gradient path also require composition. Distinct coherent tags, matching
instance layouts and preserved checkerboard/radial/multistop interpolation must
be implemented and verified across actual active backends before adoption.

Core background tags are now composed: native checkerboard retains 3 and radial
gradient uses 4. The complete color module parses. Three Windows shader conflicts
compose exact piecewise sRGB transfers and existing CSS projection, radial
geometry, premultiplied multistop/Oklab interpolation and one ordered RGB dither
policy. Native triangular gradient dithering is not applied in addition to the
existing ordered policy; visual verification of that composed choice is pending.
The separate HLSL shadow-layout conflict still requires the native scene port.

Active WGPU source now declares four stops and stop_count and consumes radial tag
4 while retaining checkerboard tag 3. Two-stop and multistop interpolation weight
colors by alpha; CSS projection and circle/ellipse farthest-corner geometry are
ported into the existing native vertex-preparation/linear-output pipeline.
These shader changes have not been compiled or rendered. Metal gradient consumers,
actual producer strides, native color-space behavior and backend pixel fixtures
remain mandatory before whole-graph/application adoption or any WPT claim.

Metal gradient source now also consumes radial tag 4 and the core four-stop
fields, preserving its native two-color vertex fast path alongside premultiplied
multistop interpolation, CSS geometry, exact transfer curves and one ordered RGB
dither. Apple build source reads the core scene/color files to generate its
header; the real generated header and Metal compilation remain unverified here.

Producer/decoder review found further required integration: core Quad retains
the local CSS transformation tail, but active WGPU Quad has no corresponding
field. Native WebGL ABI fixtures still expect the old 40-word record, and packed
background decoders need the expanded stops/count. Generic producers use Rust
instance sizes, so declaration, vertex/clip/fragment geometry, fixed offsets and
native fixtures must be composed together. Gradient-source agreement alone does
not prove the complete scene ABI or successful rendering.

WGPU/WebGL Quad transport now includes the affine tail. The explicit packed
decoder reads fourteen texels for the 56-word record, with four stop records and
their count; packed path vertices use the expanded 36-word record. Existing native
producer-byte and size fixtures are adapted, including six distinct affine words,
but have not run. WGPU and Metal vertices apply the core matrix, clipping remains
in screen coordinates, and interpolated local coordinates drive gradients and
border/SDF geometry without an inverse matrix. Actual Naga storage/WebGL tests,
generated Metal header/compiler, scene/shadow composition and transformed pixel
fixtures remain pending before application adoption or a WPT gain claim.

All five private scene conflicts and the final HLSL shadow conflict are composed.
The complete scene module parses. Native public/subpixel records and four-byte
grayscale storage retain local group/backdrop and optional ordering diagnostics.
The 32-word shadow record combines native snapped element coverage/radii with
the authored raw border box used to exclude outer shadows beneath transparent
fills. HLSL, WGPU, packed WebGL and Metal consume the same fields. Inset density
is complemented once and clipped to the original element; outer shadows retain
the authored-box exclusion. Actual shader/native pixel checks remain pending.

The extracted window shadow producer also parses independently. Native drop and
inset APIs filter their respective entries; the legacy CSS API retains its
explicit inset override. One producer initializes every shadow field once,
shrinks inset holes and clamps their radii. The window parent still has other
conflicts. Native fixture consumers, group visibility across the new platform
crates, whole compilation and actual CSS/native shadow runtime remain pending.

The private platform group boundary now exposes a borrowed dependency-ordered
group list while core scene storage/ownership stays private. Group render data
and native image ID/bytes support the split Windows renderer's blur, blend,
polygon and mask caches. Its former private-module type reference now names
the core public group type. The complete scene and small group module parse.

Three Windows renderer state conflicts are also composed: native optional
resources, dimensions and device-lost skip state remain alongside local group
targets, mask/blur caches and blend states. Old-device group textures, masks and
blend states are released before native device teardown. The extracted complete
blur-state helper parses; eleven resource/latency/pipeline conflicts remain in
the renderer parent. Whole compilation and actual device-lost/group/filter/mask/
CEF runtime checks are still pending, with no application adoption or WPT gains.

Six further Windows conflicts now compose native optional render targets/views,
release-before-resize lifetime and the local owned frame-latency handle. The
extracted complete resource module parses. Present retains native optional
resource access without adding per-frame composition commits or UI waits.
Native bulk-upload/range sprite batches preserve missing external atlas-slot
skips, and the native dual-source ClearType blend remains. Five renderer conflicts
remain: shared frame/group/headless drawing, intermediate target restoration and
explicit pass/blend binding. Actual compilation, latency/device recovery fixtures
and application/exact WPT verification remain pending.

Two further private Windows pipeline conflicts are composed. Native draw and
texture-draw signatures retain inherited frame bindings; explicit CSS blur and
group blend passes bind their own viewport and frame constant slot zero. Native
range offsets remain in slot one. The blur shader indexes its own instance
buffer without the native batch offset. The extracted complete pass module
parses; three renderer parent conflicts remain. Whole compilation, shader
validation and application runtime are still pending, with no adoption or WPT
gains attributed to this migration.

The private Windows path-MSAA restoration conflict is now composed: after
resolving the intermediate texture, drawing returns to the active CSS group
target or the native frame target. A small parsed module checks optional frame
resources and target views; callers propagate missing-target errors. Two parent
conflicts remain, covering shared visible/group rendering and headless/group
integration. Optional-device/resource callers, whole compilation, shader checks
and actual path/group/filter/device recovery remain unverified.

The final two private Windows renderer conflicts are now composed. Visible and
headless drawing share dependency-ordered group rendering; headless staging
readback retains RowPitch handling and BGRA-to-RGBA conversion. Each frame/group
uploads its own native range buffers and restores viewport, text and batch
constants after blur passes. Visible presentation alone polls the latency handle
without waiting. Group/surface failures propagate instead of producing an
incomplete successful capture. The complete renderer parent and extracted leaves
parse, but this is not a compilation or application/runtime result.

Complete group/surface orchestration now uses checked optional devices/resources,
owned frame-coordinate copies across mutable calls and the native optional
sampler. Local pass constants include the native ClearType/BGR fields and padding.
The downsample helper accepts a sampler slice matching native binding APIs.
Remaining backdrop/composite callers and external-device accessors still need
adaptation, followed by compilation, shaders and actual native/CSS/headless/
device-recovery verification. The application has not adopted these drafts.

Complete private backdrop-blur, color-matrix, group-composite and downsample
callers now use checked native optional resources and owned device/viewport
copies. Backdrop and blend copies read the active parent-group texture when
inside a group; their output returns to that active target. Missing group
compositing slots/textures now return errors. The renderer parent and all
extracted pass modules parse; actual filter/mask/blend pixels remain unverified.

Private external texture transport also follows native optional devices and
window renderer borrowing. Existing COM AddRef/from_raw ownership and atlas
registration/update/unregistration remain; a missing device returns None or
refuses registration/update rather than dereferencing absent devices. Changed
window method blocks parse independently, but the Windows window still has
other conflicts. Source scans find no remaining old direct device/resource or
array-indexed frame-buffer accesses in the migrated renderer. This is a narrow
source audit, not type checking, CEF verification or successful application
adoption. Compilation, shaders, device recovery and exact WPT retention remain
required.

Private native Windows window/event conflicts are now composed. Appearance,
background appearance, scale and minimize callbacks remain Cell-backed; the
local 600ms resize presentation boost uses the same ownership model. Native
deferred visibility reporting, accessibility handling and cross-window reentry
guard remain alongside synchronous resize painting and background-erase
suppression. Native hit-test dispatch matches its current signature. Modal
close already passed HWND correctly and was retained. Both complete modules
and extracted resize/paint handlers parse. Actual focus, resize, minimize,
dialog parent activation, transparent overlay and CEF/device-recovery runtime
remain unverified; core Window migration is still incomplete.

Seven private core Window conflicts now compose native accessibility/input/text
imports with local external textures, forced cache refresh after device recovery,
the current arena clear API and retained frame timing counters. Native auto-sized
root fill precedes prepaint and keeps phase timing. Keyboard traversal preserves
CSS focus-visible modality while calling the native focus API with App context;
its complete extracted module parses. Seven core Window conflicts remain in
asset notification and transformed layer/quad/glyph/image painting. The whole
core Window and dependency graph are not yet validated, and these source changes
do not establish an application or WPT improvement.

Five further private core Window conflicts now compose CEF logical/physical
texture painting, native grayscale/subpixel glyph sprites and CSS transformed
layer/quad painting. Glyphs retain local fractional masks and transformations;
transformed text uses grayscale while native opaque/platform/mode guards remain.
Native scene bool padding is used by external sprites. Layer coverage is rounded
after mapping into screen coordinates, and negative-z layers remain supported.
Quads retain authored fractional geometry; native border strips are retained for
untransformed geometry, while transformed quads keep a complete primitive to
avoid mixing local borders with screen-space scissors. Complete extracted modules
parse. Asset notification and image crop/sampling are the two remaining core
Window conflicts. Actual text/CEF/layer/border pixels, compilation and exact WPT
retention remain unverified; the draft is still unadopted.

The private image conflict is now composed into native visible-image crop,
existing CSS linear/nearest sampling and physical CEF region APIs. Native image
placement keeps visible/image bounds intersection, clamped atlas sub-tiles and
corner radius handling. CSS sampling keeps its fractional mask and floor/ceil
pixel coverage; all paths retain local transforms and native bool transport.
Complete image modules parse. This also removes merged bodies that referenced
image_bounds from a region API without that parameter. One core Window conflict
remains in detached-measurement asset notification. Image element/caller API
auditing, whole compilation and actual crop/nearest/CEF pixels remain pending.

The final core Window asset conflict is composed in the private draft. Native
shared loads retain their task ownership, cached results, eviction weak state
and deduplicated view notification; detached measurement registers a deduplicated
window observer instead of requiring a current view. Completion refreshes live
windows and ignores closed ones. The cache implementation is extracted into a
small documented leaf. The complete Window and cache files parse with rustfmt;
this is not a type check, runtime proof or adoption of the new GPUI graph.

Kit 0.7.0 priorities for this application are Unicode-safe syntax parsing,
unfocused caret repaint lifecycle, visible search highlights, IME/undo and stale
async editing responses, and edit-tracked range decorations. Root-owned overlay
hosting needs adaptation to the existing overlay architecture before adoption.
Toolbar is a possible later UI improvement. These upstream improvements do not
establish a WPT gain or a measured application speedup.

The private GPUI draft now retains the new physical grid-axis and native
block-flow carriers. Five Style conflicts compose checkerboard/pattern and
radial/linear backgrounds, last-baseline conversion and CSS paint ordering.
Native inset flags and legacy forced inset shadows share the padding edge;
borders precede descendants and use the already integrated transform-safe
quad primitive. Style, its small inset-paint leaf and the Taffy adapter parse.
Nine application BoxShadow literal sites need migration when the native inset
field is adopted; current application sources keep their existing GPUI type.

The remaining image-cache conflict now shares native CachedLoad ownership and
completion observers. Image consumers subscribe to their current view, or to
the window during detached measurement, without requiring an active render
view. The cache and shared-load leaf parse. Compilation, async ownership/type
checks, image completion/cancellation, Windows icon loading, detached HTML
image measurement and existing GPU/editor/IME runtime gates remain pending.
These changes are confined to the private draft; the GPUI upgrade is not yet
adopted and no WPT gain or application speedup is claimed.

Four private image-element conflicts now compose native asset decoding and
frame lifecycle with extracted local image-style and intrinsic-size helpers.
Zero-frame painting guards and stale-frame clamping remain; authored aspect
ratios and percentage-opposite automatic sizing retain their local contracts.
Default images use native visible/fitted bounds cropping. Opted-in HTML raster
images use the same crop transport while retaining fractional coverage and
nearest/linear selection. Native SVG rendering already carries smooth scaling
and premultiplied alpha conversion, so it is retained without duplicate manual
rasterization. The complete image parent and both changed small leaves parse.

Eight image call sites in four application files have private migration drafts:
HTML raster layers retain their existing sampling coverage, and icon atlas
warm-up supplies identical native visible/fitted bounds. All caller drafts
parse. These are source adaptations only; whole-graph compilation, actual
object-fit/object-position/corner cropping, animation/loading cancellation,
SVG/icons, existing UI/CEF and exact WPT retention remain required. The new
GPUI graph is not yet adopted.

The private animation, interaction-style and view-cache ports now parse as
part of all 148 GPUI Rust source files, with no remaining conflict markers or
syntax errors. Explicit animation sampling composes with the native shared
clock and reduced-motion policy. Hover/focus text refinements retain existing
font metrics, touch filtering and focus-visible keyboard behavior. Reused-view
accounting follows the native cache reuse path. Type checks and runtime tests
of these contracts remain pending.

The private source closure now has a standalone 29-member workspace: the 27
upstream packages, the upstream scratch patch and an exact copy of the patched
Taffy 0.14 sources. GPUI inherits the local Taffy path with version =0.14.0;
unreachable Zed workspace paths are removed. Offline Cargo metadata without
dependency resolution succeeds and reports the intended local Taffy dependency.
It warns that Taffy's package profiles are ignored in the parent workspace.
This verifies manifest loading, not a fully resolved single dependency graph,
compilation, UI behavior or adoption. Those checks remain required before the
application can use the upgraded GPUI.

Full Cargo dependency resolution subsequently succeeds for this private
workspace. Its 833 resolved packages include exactly one GPUI core and exactly
one Taffy, the local patched 0.14.0; no registry or older Taffy instance appears.
This verifies the private dependency graph, while whole compilation and
application adoption remain pending.

Six remaining Windows source conflicts now compose native atlas ownership and
accelerator dispatch with external CEF textures and child-HWND character input.
External entries retain stable texture identities, are excluded from ordinary
glyph/image tile allocation, and accept updates/removal only through their
external ownership path. Both message drains retain native top-level accelerator
handling while actual child windows receive translated character messages.
Two small leaves hold these adaptations; the changed parents and leaves parse.
Live CEF resize/cancellation, editor and webview keyboard/IME behavior, device
loss, type checking and exact WPT retention remain unverified.


The GPUI migration draft native snapshot has become stale while engine recovery advances: it contains 115 Rust sources versus 125 in the current patched native library, with ten added and ten changed files. A separate exact current-source refresh snapshot is prepared privately, including ratio constraints, standalone-track sizing, full intrinsic contribution cache refresh and bounded preferred ratio contribution. The existing migration workspace remains unchanged; its prior metadata resolution and syntax checks do not establish compatibility of the refreshed native snapshot. Refresh integration, type/build checks and application/WPT retention are still required before GPUI adoption.
