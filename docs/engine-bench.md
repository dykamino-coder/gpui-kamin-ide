# Engine phase benchmark

`engine_bench` measures one local document in one offscreen Windows GPUI window.
It requires the independent `engine-bench` feature; it neither enables nor calls
the `wpt-harness` runner, and refuses measurements if that feature is also enabled.
It changes no engine algorithms. The standalone `crates/engine-bench` package
has no dev-dependencies. Its binary requires the explicit package feature
`engine-bench`; the engine dependency is optional, so ordinary workspace builds
do not activate the timers. Its normal `gpui` and `gpui-platform` dependencies inherit
the same workspace defaults as `crates/shell`; the installer build does not add
Cargo features. Resolved GPUI features are `default`, `font-kit`, `wayland`,
`windows-manifest`, `x11`; platform features are `default`, and the Windows
backend has no enabled features. Neither `gpui/test-support` nor GPUI
`bench-support`/`profiler` is enabled. The engine enables only `engine-bench`,
not `wpt-harness`. The global allocator is `mimalloc`, as in the application.

Build only this package: building workspace tests/examples can unify test
features and is not a measurement build. You can inspect the normal graph with
`cargo tree -p engine-bench --features engine-bench --edges normal,build --format "{p} FEATURES={f}"`.
The binary also checks the engine's resolved WPT feature and refuses to measure
if another selected package has enabled it.

```powershell
cargo build -p engine-bench --bin engine_bench --features engine-bench --profile release --target-dir target/bench
target/bench/release/engine_bench.exe C:/pages/heavy.html --viewport 800x600 --runs 100 --mode both
target/bench/release/engine_bench.exe C:/pages/heavy.html --runs 100 --json --snapshot C:/results/first.png
```

Use the release executable directly for repeated measurements. Debug builds
refuse to measure. Defaults: 800x600 CSS pixels, device scale factor 1, 30 samples
per mode, both modes. `--mode cold|warm|both` selects the samples. `--snapshot`
writes the first measured frame after timing; its directory must already exist.
Errors produce a nonzero exit status. A run is rejected if its capture has the
wrong dimensions, document traversal degrades, pixels change between samples,
or more than one frame crosses the sample boundary.

## What is measured

All durations are wall time in milliseconds, including work or waiting within
that boundary. Schema version 2 reports a **non-overlapping partition of each
sample's total** (`intervals_overlap: false`). The parse/style children are subtracted from
document construction before conversion to floating-point milliseconds. All subtraction uses checked
integer `Duration` arithmetic; inconsistent nesting fails the entire invocation,
without clamping negative values or dropping samples. Individual medians or p90s
need not sum to the total's median or p90.

| JSON phase | Boundary and interpretation |
| --- | --- |
| `host_resources` | Read/decode HTML, host resource discovery, linked CSS and imports, URL rebasing and optional serialization. Includes a separate host HTML parse; it is not the engine's `parse`. Cold only. |
| `parse` | Existing `Document::build` HTML/XML admission into the engine DOM. Cold only. |
| `style` | Existing `dom::convert`: stylesheet processing, cascade/computed values and normalized styled box-tree construction. This includes more than selector matching. Cold only. |
| `document_other` | Complete `Document::build` minus parse and style: key/base URL handling, resource/font preparation and viewport preparation. Cold only. |
| `build_elements` | Root `Render::render`, including `Document::render` and GPUI element-tree construction. Engine layout preparation can occur here. |
| `layout_request` | Root element's `request_layout`, including descendants. |
| `layout` | Wall interval from completion of root `request_layout` to entry into root `prepaint`. GPUI computes root layout between these boundaries (`Drawable::layout_as_root`); this includes its ancestor/wrapper transitions. Nested computations during this interval count once. Custom/nested layout performed during building or prepaint stays in that calling phase, rather than being counted again. |
| `prepaint` | Root element's `prepaint`, including descendants and any layout work deferred into this callback. |
| `paint` | Root element's `paint`: CPU scene-command generation, including descendants. This is not GPU raster time. |
| `raster_gpu_readback` | Existing GPUI `PRESENT_US` delta: complete `window.present`, including GPU submission/raster, synchronous readback and platform present/commit. A combined wall-time scope, **not an isolated GPU timestamp measurement**. |
| `pipeline_other` | Total minus the disjoint phases above: frame scheduling, previous-document disposal, GPUI work outside root callbacks (including deferred work), orchestration and counter quantization. Kept explicitly so time outside measured boundaries is never silently lost or attributed to an engine algorithm. |
| `first_frame_total` | Budget total, not an additional phase: from host loading (cold) or unchanged-frame request (warm), through completed first GPU readback and return to the async task. Excludes application/window startup, statistics, fingerprinting and PNG encoding. |

`modes.<mode>.diagnostic_scopes` retains inclusive `document_build_total`,
`cpu_draw_total`, `gpu_readback` and `taffy_inclusive`, with raw samples and
statistics. The existing `TAFFY_US` counter includes nested layout calls and may
count the same wall interval more than once; it is diagnostic only, never used
to derive the additive layout phase. Its resolution is one microsecond per
increment. These diagnostics **do overlap**
(`diagnostic_intervals_overlap: true`) and must not be added to the phase budget.
Readback is contained in `raster_gpu_readback`; draw contains the root lifecycle.
No GPU timestamp or pure style/layout algorithm time is inferred from these labels.

The root adapter forwards the same element layout ID and introduces no layout
node. The only engine hooks are `Instant` measurements at existing parse/style
boundaries, with storage owned by `Document`. Their module, fields, arguments,
clock reads and public API are all `#[cfg(feature = "engine-bench")]`; ordinary
builds compile them out. The existing GPUI counters/capture are reused without
changing vendor code.

## Cold, warm and statistics

The application, font system and GPU are initialized using an empty frame.
**Cold means a fresh document, not a fresh process or empty machine caches.**
Each cold sample rereads resources and builds a new document. The OS file cache,
process image/font caches, allocator and GPU remain shared across samples.
In `both` mode, warm samples redraw the last cold document with no mutations.
Warm-only mode prepares that document with one unreported cold frame first.
An explicit redraw executes the lifecycle; it does not measure an idle window.
Host resource, parse/style and document scopes are `null` for warm samples, rather than fictitious zeroes.

Samples are not trimmed, rescaled or filtered. The median averages the two
middle values for even sample counts. p90 uses nearest rank `ceil(0.9*N)`.
JSON includes count, minimum, maximum, median, p90 and the individual samples.
The BGR FNV-1a fingerprint detects changing sample output; use saved PNGs and
an exact RGB comparison for independent pixel validation.

`peak_working_set_bytes` is Windows `PeakWorkingSetSize` for the whole process
since launch, including startup, all samples and captures. It is resident memory,
not private committed bytes, GPU allocation size or a per-document allocation
peak. `startup_peak_working_set_bytes` reports the high-water mark after startup;
subtracting the two does not yield a document's peak memory. Restart the process
to reset the high-water mark.

## Inputs and limits

The host supports `.html`/`.htm`, local stylesheet links, ordinary CSS imports
(including media qualifiers), local CSS `url()` resources, images and the
engine's local font loading. Relative stylesheet URLs keep their original base.
Host resource preparation is reported separately. Inline CSS is otherwise left
to the engine. Linked CSS is inserted at the link's DOM position, so discovery
can serialize the input; this can affect selectors targeting `link`/`style` and
is a limitation of this host. Prefer inline styles for strict cross-runner
pixel checks. CSS import `layer`/`supports` qualifiers, escaped function names,
network resources and nested documents are unsupported. Missing discovered
resources fail the run. Resources referenced only by unsupported engine features
may still be ignored by the engine. JavaScript is not executed.

The host uses a white viewport, grayscale text, Times New Roman at 16px and
normal line-height 1.31 as inherited defaults; document CSS can override text
styles. It freezes scrollbar interaction/animation for deterministic static
capture, matching wptrun's static rendering policy. Scrollbar painting remains;
interactive hitboxes, hover/fade updates and input handling are not benchmarked.

This is an offscreen **rendered window**, not a headless browser navigation.
It uses Windows graphics APIs and the currently selected adapter. On the review
machine that is an Intel iGPU: JSON includes adapter/driver data when GPUI can
provide it (otherwise `gpu` is null). Record power plan, temperature,
display configuration and competing load beside results. There is one window,
one document and one tab-equivalent; there is no browser UI, multi-tab contention,
network stack timing or compositor isolation. Readback and CPU capture copies
have measurable overhead. The total also includes refresh scheduling and may
track display cadence. There are no shader GPU timestamps or subphase claims
for otherwise inseparable scopes. Reuse the same fonts, viewport and build.

For stability acceptance, use **N=100 samples per mode and at least three fresh
processes per fixture** on a quiet machine, with other agents/builds stopped.
Compare each mode/phase's process medians using `(max - min) / min`; the target is
below 10%. Report small intervals near timer resolution explicitly. Never discard
slow samples to satisfy a threshold. A contended-machine series is an example
under load, not stability acceptance; repeat it in a quiet window before review.

Record CPU load **beside each executable invocation**, together with its timestamp,
build revision, power plan and raw JSON. For example, collect the OS CPU estimate
immediately before and after each fresh process, outside its timed intervals:

```powershell
$cpuBefore = Get-CimInstance Win32_Processor | Measure-Object LoadPercentage -Average
$startedUtc = [DateTime]::UtcNow.ToString("o")
& target/bench/release/engine_bench.exe C:/pages/heavy.html --runs 100 --json > C:/results/run-1.json
$benchExit = $LASTEXITCODE
$cpuAfter = Get-CimInstance Win32_Processor | Measure-Object LoadPercentage -Average
@{ started_utc = $startedUtc; cpu_before_percent = $cpuBefore.Average;
   cpu_after_percent = $cpuAfter.Average; exit_code = $benchExit } |
    ConvertTo-Json > C:/results/run-1.environment.json
```

These CPU readings are OS load estimates near the run, not per-phase utilization.
Save evidence outside the checkout. Compare the first-frame PNG against the same
document rendered by `wptrun` at the same viewport, scale and fonts, using GPU
first-frame capture and **exact RGB equality**. Use one fresh process per fixture
in each host so their prior document/cache histories match.

## Relation to existing profiler designs

Servo's servoshell `--profile` configures a time-profiler channel. Category and
document metadata accompany duration events; its time-profiler thread aggregates
per-category samples and can emit tabular data or an HTML trace. See
`ports/servoshell/prefs.rs`, `components/shared/profile/time.rs` and
`components/profile/time.rs` in Servo. This harness similarly preserves raw
scope samples, but specifies arithmetic even medians and p90 explicitly.

Blink emits nested tracing events in categories such as `blink,blink_style`;
`StyleEngine::UpdateStyleAndLayoutTree` records `Document::recalcStyle` and
`Document::rebuildLayoutTree` in `third_party/blink/renderer/core/css/style_engine.cc`.
Lighthouse's [task groups](https://github.com/GoogleChrome/lighthouse/blob/main/core/lib/tracehouse/task-groups.js)
map `ParseHTML`/`ParseAuthorStyleSheet`, `UpdateLayoutTree`/`Layout`, and
`PrePaint`/`Paint`/`RasterTask` into broader reporting groups. Its
[main-thread task processor](https://github.com/GoogleChrome/lighthouse/blob/main/core/lib/tracehouse/main-thread-tasks.js)
reconstructs nesting and computes self time by subtracting child durations.
This harness subtracts only children with known boundaries (document parse/style),
retaining inclusive counters separately. It does not reconstruct a general tracing timeline. Category labels do not imply pure GPU time.

Gecko places profiler labels/markers at style, layout and paint boundaries, for
example `Styles` in `layout/style/RestyleManager.cpp`, `LAYOUT_StyleComputation`
in `layout/style/ServoStyleSet.cpp`, and paint scopes in
`layout/base/nsLayoutUtils.cpp` and `layout/painting/nsDisplayList.cpp`. Those
markers contextualize profiler timelines; they are not interchangeable with an
offscreen first-frame stopwatch. This harness uses existing engine boundaries
and capture completion rather than inventing matching category names.

These are comparisons of instrumentation mechanisms, not performance results
or claims of equivalent browser workloads.
