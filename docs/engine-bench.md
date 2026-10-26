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

All durations are wall time in milliseconds on the calling thread, including
any work or waiting within the stated boundary. These are **nested scopes, not
an additive breakdown**. JSON sets `intervals_overlap: true` and retains every
raw sample. Do not derive an unexplained remainder by subtracting these scopes.

| JSON field | Boundary and interpretation |
| --- | --- |
| `host_resources` | Read/decode HTML, host resource discovery, linked CSS and imports, URL rebasing and optional serialization. Includes a separate host HTML parse; it is not the engine's `parse`. Cold only. |
| `parse` | Existing `Document::build` HTML/XML admission into the engine DOM. Cold only. |
| `style` | Existing `dom::convert`: stylesheet processing, cascade/computed values and normalized styled box-tree construction. This includes more than selector matching. Cold only. |
| `document_build_total` | Complete `Document::build`, including parse/style and preparation of viewport/resources. Excludes host loading and replacement/drop of the previous document. Cold only. |
| `build_elements` | Root `Render::render`, including `Document::render` and GPUI element-tree construction. Engine layout preparation can occur here. |
| `layout_request` | Root element's `request_layout`, including descendant requests and their work. |
| `layout` | Delta of the existing GPUI `TAFFY_US` counter: calls to Taffy layout computation/geometry completion, including text measurement. Not all engine layout work lives here; nested calls may overlap the other scopes. Resolution is one microsecond per counter increment. |
| `prepaint` | Root element's `prepaint`, including descendants. Work deferred outside that root is covered by `cpu_draw_total`. |
| `paint` | Root element's `paint`: CPU scene-command generation, including descendants. Deferred draws outside the root are covered by `cpu_draw_total`. This is not GPU raster time. |
| `cpu_draw_total` | Existing GPUI `DRAW_US` delta for the complete window draw, including lifecycle overhead and deferred work. One-microsecond counter resolution. |
| `raster_gpu_readback` | Existing GPUI `PRESENT_US` delta: complete `window.present`, including GPU submission/raster, synchronous readback and platform present/commit. This is a combined wall-time scope, **not an isolated GPU timestamp measurement**. |
| `gpu_readback` | Existing capture counter delta for texture copy/map and CPU row copy; nested inside `raster_gpu_readback`. |
| `first_frame_total` | From the start of host loading (cold) or unchanged-frame request (warm), through a completed first GPU readback and return to the async task. Includes frame scheduling, previous-document disposal and orchestration. Excludes application/window startup, statistics, fingerprinting and PNG encoding. |

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
Parse/style/build scopes are `null` for warm samples, rather than fictitious zeroes.

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

For repeatability, run several fresh processes on an idle machine and compare
each mode/scope's medians using `(max - min) / min`. Report small intervals near
timer resolution and unstable scopes explicitly; never discard slow samples
to satisfy a threshold. Save raw JSON, build revision, environment and optional
first-frame PNG outside the checkout. Compare that PNG against the same document
rendered by `wptrun` at the same viewport, scale and font configuration, using
GPU first-frame capture and exact RGB equality. Use one fresh process per
fixture in each host so their prior document/cache histories match.

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
This harness reports inclusive scopes directly; it does not reconstruct a trace
or compute exclusive self times. Category labels do not imply pure GPU time.

Gecko places profiler labels/markers at style, layout and paint boundaries, for
example `Styles` in `layout/style/RestyleManager.cpp`, `LAYOUT_StyleComputation`
in `layout/style/ServoStyleSet.cpp`, and paint scopes in
`layout/base/nsLayoutUtils.cpp` and `layout/painting/nsDisplayList.cpp`. Those
markers contextualize profiler timelines; they are not interchangeable with an
offscreen first-frame stopwatch. This harness uses existing engine boundaries
and capture completion rather than inventing matching category names.

These are comparisons of instrumentation mechanisms, not performance results
or claims of equivalent browser workloads.
