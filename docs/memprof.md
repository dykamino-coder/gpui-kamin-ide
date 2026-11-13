# Memory profiler (memprof)

One command answers "which subsystem and which stacks hold the heap at point X, candidate against
base". It works on the Rust heap of `engine_bench` (native/D3D memory is not seen; GPU memory is
in the bench JSON).

## In one minute

1. Build each side with symbols (the PDB must lie beside the exe):

   ```powershell
   $env:CARGO_PROFILE_RELEASE_STRIP="false"; $env:CARGO_PROFILE_RELEASE_DEBUG="line-tables-only"
   cargo build -p engine-bench --bin engine_bench --features allocation-audit --profile release --target-dir target/bench
   # copy target/bench/release/engine_bench.exe and engine_bench.pdb to e.g. bin/cand/
   ```

   `allocation-audit` is exact and keeps the audit counters, so the report can be checked against
   `live_requested_bytes`. `heap-profile` has the probe only, without the counters, for a build
   close to the timing build; use it with `--sample`.

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

## Choosing the base and the mode

- Base: the merge base of the candidate branch with `main` (`git merge-base origin/main <branch>`),
  not today's `main`, so the difference is the candidate's own change. Build both sides with the
  same features and the same `CARGO_PROFILE_RELEASE_*` settings; if a side predates this tool,
  cherry-pick the memprof commit onto it (only `crates/engine-bench` and `scripts` change).
- Exact (default, `allocation-audit`): every block; totals equal the audit counter. Use it for the
  final answer and for differences under a few MB.
- Sampled (`--sample 16384`, `allocation-audit` or `heap-profile`): first look at a large page or a
  long window, or a build close to the timing build (`heap-profile` has no audit counters).
- Churn instead of live heap: `report --allocated` over a narrow window such as
  `ENGINE_BENCH_ALLOC_STACKS=build_elements@3` (the old `alloc_stacks.py` question).

## Other commands

- `memprof.py report CAND.exe CAND.stacks [BASE.exe BASE.stacks] [--allocated] [--json R.json]`:
  fold existing probe files. `--allocated` counts every allocation of the window (churn) instead of
  the live heap. `--frames A-B` keeps only records of those frames.
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
`scripts/memprof.py` and printed apart, marked «inferred from stacks, not measured». `--owners` records the whole run exactly (several GB on wide-16000; the
helper scripts delete the `.stacks` file after folding).

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

## Limits

- Exact mode writes one line per allocation: wide-16000 up to the end of the first relayout build
  is 3.5 GB and about 100 s per side. `--sample 16384` gives 45 MB and about 15 s per side, with
  totals within 0.3% of the audit counter; small differences (under a few MB on a 200 MB heap)
  need the exact mode. A later `--start` shortens the file but leaves out blocks allocated before.
- Only the Rust global allocator is seen.
