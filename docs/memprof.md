# Memory profiler (memprof)

One command answers "which subsystem and which stacks hold the heap at point X, candidate against
base". It works on the Rust heap of `engine_bench` (native/D3D memory is not seen; GPU memory is
in the bench JSON).

## In one minute

1. Build each side with the profiler build (one command; the exe and its PDB land in `--out`):

   ```powershell
   python scripts/memprof.py build --out bin/cand            # the checkout in the current directory
   python scripts/memprof.py build --out bin/base --worktree ../base-checkout
   ```

   It builds `engine_bench` in the release profile with the PDB kept (function names and line
   tables) into `<worktree>/target/prof`, the one profiler target directory of a checkout, and
   links it with `/OPT:NOICF`; see
   [The profiler build](#the-profiler-build) for why a plain release build misnames sites.
   `--features` picks the bench build: `allocation-audit` (default) is exact and keeps the audit
   counters, so the report can be checked against `live_requested_bytes`; `heap-profile` has the
   probe only, without the counters, for a build close to the timing build (use it with
   `--sample`). `--jobs`, `--affinity <mask>`, `--below-normal` and `--wrapper "<command prefix>"`
   (for example a build-slot queue) place the build on a shared machine.
   A base and a candidate built with different features or profiles are not compared: `run`,
   `report`, `diff` and `ws` stop and list the differences (`--allow-feature-mismatch` overrides;
   see the build record in [engine-bench.md](engine-bench.md#build-record)).
   `--screen` builds the same thing without link-time optimisation (incremental, 256 codegen
   units) in the same target directory: several times faster, and cargo keeps both variants'
   artifacts, so switching does not rebuild the other. Use it for everyday memory profiles and
   audit counters; final measurements and time profiles need the default build, and both sides
   of a comparison must be built the same way (`build.json` in `--out` records `lto`).

2. Ask the question:

   ```powershell
   python scripts/memprof.py run --exe bin/cand/engine_bench.exe --base-exe bin/base/engine_bench.exe `
     --at build_elements@2 --out out/s0 -- C:/pages/wide-16000.html --viewport 800x600 --mode relayout --runs 1
   ```

   `--at <phase>@<frame>` is the moment: the end of that phase in that measured frame (frames are
   numbered from 1 in process order; 0 is process start). Every block allocated from process start
   (`--start`) and not freed by that moment is counted, so the result is the whole live heap, not
   only what the frame allocated. Add `--sample 16384` for a sampled run (much smaller files and
   faster runs; bytes become estimates). Wrap the bench with `--launcher "<command prefix>"`, for
   example a hidden-desktop launcher.

   The raw probe files (`cand.stacks`, `base.stacks`; gigabytes each in exact mode) are folded and
   removed side by side: the candidate runs, is folded and its file is deleted before the base
   runs, so at most one raw file is on disk. `--keep-stacks` keeps them for a later
   `memprof.py report` (which has `--delete-stacks`). An exact run does not start with less than
   `--min-free-gb` (default 60) of free space at `--out`. `run` and `ws` also refuse to start the
   bench while a hold file exists: `KAMIN_BENCH_HOLD` lists glob patterns separated by `;` (files
   another measurement creates while it needs a quiet machine); `--ignore-hold` is for the owner
   of the hold.

3. Read `out/s0/report.txt` (also `report.json`):

   ```text
   memprof live heap, filter=*@0-2 until=build_elements end=end frame=2 after=build_elements live=..., exact
     cand: 228.934 MB in 1025354 blocks, audit counter 228.934 MB (100.00% seen)
     base: 212.502 MB in 993343 blocks, audit counter 212.502 MB (100.00% seen)
     delta: +16.432 MB, +32011 blocks
   by subsystem (MB):          <- who owns the difference
         +6.292        +7     62.384    56.092  elements
         +6.275    +16003      6.282     0.006  retained
   by site:                    <- first engine frame that allocated it, with [type] for Box/Rc/Arc
         +6.291        +6  elements         gpui::arena::Arena::allocate_next_chunk
         +6.144    +16000  retained         kamin_html::layout::reuse::flow::FlowCx::record_skip
   by stack (depth 8 from the site):
   ```

   With `allocation-audit` the window end also records the audit counter `live_requested_bytes`;
   in exact mode the probe total equals it, which shows nothing was missed. Two builds inline
   differently, so one logical stack can appear as a `+N`/`-N` pair of rows; the subsystem totals
   are not affected.

## The profiler build

A release build misleads a profile in two ways, and `memprof.py build` removes both:

- **No names.** The release profile strips symbols. Without function records in the PDB dbghelp
  falls back to exported or mangled names, the allocator frames are not recognised and everything
  lands in `other`. `build` sets `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` and
  `CARGO_PROFILE_RELEASE_STRIP=false`; nothing in `Cargo.toml` changes, so any commit builds.
- **Folded functions.** The MSVC linker merges functions with identical machine code (`/OPT:ICF`,
  on for optimised builds): one body remains, and a stack frame in it gets the name of any one of
  the merged functions. Generic code makes this common (one fat-LTO build: about 1400 functions in 476
  bodies), and a profile then blames, say, `TaffyTree::with_capacity` for blocks a different
  function allocated. `build` links with `/OPT:NOICF` and keeps the linker flags of
  `.cargo/config.toml` (the 8 MiB stack).

After the build the command checks the image: the stack reserve in the PE header equals the
`/STACK` of the cargo config, the PDB has function records, and no two functions share an address.
`build.json` beside the exe records the commit, features and flags.

Every report checks the same on the exes it is given. For an exe built another way it prints

```text
WARNING cand: the exe was linked with identical-code folding (ICF): 1402 functions share 476 bodies ...
```

marks each folded frame in the tables as `name {ICF n}` and lists, under «folded functions», the
other names that frame may stand for (`folded_sites` in the JSON). A PDB without function records
gets its own warning. Treat a marked site as «one of these functions» until both sides are
rebuilt with `memprof.py build`.

## Choosing the base and the mode

- Base: the merge base of the candidate branch with `main` (`git merge-base origin/main <branch>`),
  not today's `main`, so the difference is the candidate's own change. Build both sides with
  `memprof.py build` and the same `--features` (`--worktree` points at the base checkout; the
  script of a newer checkout builds an older commit as it is).
- Exact (default, `allocation-audit`): every block; totals equal the audit counter. Use it for the
  final answer and for differences under a few MB.
- Sampled (`--sample 16384`, `allocation-audit` or `heap-profile`): first look at a large page or a
  long window, or a build close to the timing build (`heap-profile` has no audit counters).
- Churn instead of live heap: `report --allocated` over a narrow window such as
  `ENGINE_BENCH_ALLOC_STACKS=build_elements@3` (the old `alloc_stacks.py` question).

## Other commands

- `memprof.py report CAND.exe CAND.stacks [BASE.exe BASE.stacks] [--allocated] [--json R.json]`:
  fold existing probe files. `--allocated` counts every allocation of the window (churn) instead of
  the live heap. `--frames A-B` (also on `run`) keeps only blocks allocated in those frames; frees
  count in every frame, so a block freed after them is not live. The header names the frames, and
  the share of the audit counter is left out, since that counter covers every frame.
- `memprof.py diff A.json B.json`: compare two saved reports, no symbols needed (e.g. two days).
- `--depth N` (frames per stack row, default 8) and `--top N` (rows per table, default 25).
- `scripts/alloc_stacks.py` is kept as an alias of `report` (`--live` = live heap).
- `memprof.py ws --exe CAND.exe [--base-exe BASE.exe] --out DIR [--launcher L] -- PAGE.html ...`:
  the working set by memory region at its peak, for a peak working set that the live heap does not
  explain (allocator segments that stay committed, direct OS allocations of large blocks, mapped
  files, images, thread stacks, graphics driver pages). Any build works, timing-only included. The
  process working set is sampled every `--interval-ms` (default 1); each new peak (by `--step-mb`,
  default 2) takes a breakdown: `VirtualQueryEx` over the address space and `QueryWorkingSet` for
  the resident pages, summed by category (`image`, `mapped`, private `stack`, `wc/nocache`,
  `big-res` for reservations of 16 MiB or more such as allocator arenas, `small-res`, and shared
  pages apart) and listing the largest private reservations. `ws.txt`/`ws.json` hold the breakdown
  at each side's sampled peak and the process peak; a peak shorter than one interval shows only in
  the process peak. With the hidden-desktop launcher the child pid is read from its first line, and
  the bench JSON goes to `cand.json`/`base.json`.
- `memprof.py phases --exe CAND.exe [--base-exe BASE.exe] --out DIR [--launcher L] -- PAGE.html ...`
  (or `--from CAND.json [BASE.json]` for bench JSON of earlier runs): memory at the end of every
  phase of the measured frames, layer by layer: `live` (requested bytes of live Rust blocks),
  `allocator` (bytes mimalloc has committed), `held` (allocator minus live: freed pages not yet
  purged, arena commit and rounding), `commit` (private commit of the process), `ws` and its
  sampled `peak` inside the phase, `gpu` (DXGI local memory of the process). Rows are medians over
  the frames of a mode; with a base every column is followed by its delta. It needs an
  `allocation-audit` build for `live`; no probe and no stacks are involved. The allocator column
  comes from `mi_stats_get` and is written only under `ENGINE_BENCH_ALLOCATOR_MEMORY=1` (set by
  this command), so ordinary bench JSON keeps its fields. `phases.txt`/`phases.json` hold the table.

## Probe environment (set by `run`)

| variable | meaning |
| --- | --- |
| `ENGINE_BENCH_ALLOC_STACKS=<phase>[@<frame>[-<frame>]]` | window: phase name, `outside` or `*`, frames |
| `ENGINE_BENCH_ALLOC_STACKS_UNTIL=<phase>` | the window ends when the last frame leaves this phase |
| `ENGINE_BENCH_ALLOC_STACKS_SAMPLE=<bytes>` | Poisson sampling by bytes, mean interval; absent = exact |
| `ENGINE_BENCH_ALLOC_STACKS_OUT=<file>` | probe file (v3 text format) |

File format: header `# engine_bench alloc stacks v3 base=<image base> filter=.. until=.. sample=..`,
then `<a|z|r> <frame> <phase> <bytes> <pointer> <return addresses>` per allocation and
`f <frame> <phase> <bytes> <pointer>` per free, `# end ...` when the window closes and
`# dropped N` (sampled blocks the pointer table could not hold) at process exit.

## How it works and where it comes from

- Live heap by stack is the heaptrack/DMD model: the allocator shim records each allocation with
  its stack and each free by pointer; the report replays them to the chosen moment. Lines go
  through one buffered sink under a lock, so a free is always written before the same address is
  handed out again.
- Sampling is the Chrome sampling heap profiler / tcmalloc scheme: each thread draws exponential
  byte intervals, so a block of `s` bytes is sampled with probability `1-exp(-s/T)` and the report
  divides by it (unbiased). Frees are written only for sampled pointers, found in a fixed lock-free
  table. Disarmed, the allocator pays one relaxed flag load per call in both builds.
- Subsystems: like Chrome's heap-profiler categories and DMD's stack-based reports, every block is
  attributed offline, without annotations in the engine. The allocator frames are cut (only a
  leading run whose names START with `alloc::alloc::`, `alloc::raw_vec::`, `alloc::rc::`,
  `engine_bench::allocations::`, `__rust`...; a `Global` generic argument in a caller's name does
  not count), std containers are skipped to find the site, and the first frame from the site up
  that matches the ordered `SUBSYSTEMS` table in `scripts/memprof.py` names the owner: `retained`,
  `text`, `dom`, `style`, `paint`, `display-list`, `layout`, `elements` (frame element tree and
  arenas), `gpu`, `recursion-stacks`, `svg-images`, `document`, `bench-host`, `*-other`.
  Extend the table when a new module appears in `other`.
- Attribution is by allocation site, not by the current holder: a block allocated by layout and
  later moved into a retained cache is reported under the site that allocated it, with its full
  stack. The question «who holds it now» is answered by `--owners` below.

## Who holds the memory (`--owners`)

```sh
python scripts/memprof.py run --exe CAND.exe [--base-exe BASE.exe] --owners --out DIR \
  -- PAGE.html --viewport 800x600 --mode relayout --runs 1
```

After the last frame of each mode the bench (`ENGINE_BENCH_MEMORY_REPORT=1`,
`crates/engine-bench/src/holders.rs`) walks what the document and the GPUI window hold and writes
`memory_owners` into that mode of the bench JSON: bytes per owner path, their sum, the audit
counter read just before the walk and the difference (`unclassified_bytes`). With the probe each
measured block is also written as `o <path> <pointer> <bytes>`; the report then shows the live heap
at the moment the walk started, how much of it the owners matched by address, what they only
estimated (hash tables without an exposed address; each estimate claims one unreported live block
of exactly its size), what they named wrongly (`unmatched`, a reporter bug), and the live blocks
nobody reported, by allocation site and stack (`unreported_stacks` in the JSON): the list of
reporters still to write. Holders no reporter can reach, such as captures of type-erased mouse
listeners, are inferred from the stacks of unreported blocks by the small `HELD_BY` table in
`scripts/memprof.py` and printed apart, marked «inferred from stacks, not measured». `--owners`
records the whole run exactly (several GB on wide-16000; the file is removed once folded).

Owner paths, about:memory style: `dom/nodes`, `dom/text`, `dom/attributes`, `dom/source-tree` (the
parse tree a revision keeps), `style/computed`, `style/groups`, `style/inherited`,
`style/materialized`, `style/layout-facts`, `layout/intrinsic`, `layout/taffy-nodes`,
`layout/taffy-cache`, `layout/taffy-styles`, `gpui/layout-engine`, `gpui/element-arena`,
`gpui/frame`, `gpui/hitboxes`, `gpui/listeners`, `gpui/element-states`, `display-list/scene`,
`text/line-layouts`, `document/other`.

The mechanism is Servo's `MallocSizeOf` and Gecko's memory reporters, with our own trait:
`kamin_heap::HeapSize` (`crates/heap`) sizes a value's heap blocks through `Ops`, which keeps the
current owner path, counts each `Rc`/`Arc` block once and forwards every block to an optional sink
(the DMD-style cross check). Sizes are requested bytes, like the audit counter, so a report can be
compared byte for byte. Vendor crates (taffy, gpui, rcdom) expose plain `heap_report`/`heap_blocks`
methods with a sink instead of depending on the trait. Nothing is counted in hot structures:
reporters walk existing fields on request, and the walk is compiled only with the
`heap-report` feature (`kamin-html`, `kamin-style`; on in the `allocation-audit` and `heap-profile`
bench builds). Not walked yet: values inside the element arena, captures of listeners and element
states (their boxes are counted), tendril text, retained reuse records, paragraph caches, the
platform text system; they stay in the unreported list.

The same report lists GPU memory apart from the heap, «who holds GPU memory»: every texture the
window's renderer holds, by owner path, with the number of textures and how much of them is in
use. `gpu/atlas/monochrome|polychrome|subpixel` are the glyph and image atlas pages (in use =
allocated tiles; a registered external texture is a polychrome page with none),
`gpu/target/frame|path|path-msaa` the frame and path targets, `gpu/group/stage|atlas|pool|
level-backdrop` the buffers of isolated groups, `gpu/blur/copy|blend-copy|down|gauss` the blur
scratch. Bytes follow the texture descriptions (size, format, mip levels, MSAA samples): what the
pixels take, not what the driver reserved; compare the total with `end_gpu.local_bytes` of the
bench JSON, which is the driver's figure for the whole process. The numbers are also in
`memory_owners.gpu` of the bench JSON. Not walked: the swap chain's other buffers, DirectWrite's
glyph rasterization targets, buffers and shaders. The walk reads existing fields on request
(`Window::gpu_memory`); nothing is counted while rendering.

## Limits

- Exact mode writes one line per allocation: wide-16000 up to the end of the first relayout build
  is 3.5 GB and about 100 s per side. `--sample 16384` gives 45 MB and about 15 s per side, with
  totals within 0.3% of the audit counter; small differences (under a few MB on a 200 MB heap)
  need the exact mode. A later `--start` shortens the file but leaves out blocks allocated before.
- Only the Rust global allocator is seen.
