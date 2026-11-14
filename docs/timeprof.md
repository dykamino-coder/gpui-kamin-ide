# Time profiler (timeprof)

One command answers "where does the time of each phase go, candidate against base": by phase, by
subsystem, by function and by stack, and for one function by its callees. It replaces one-off timing
probes compiled into the engine (a debug branch per question, a rebuild per probe).

## In one minute

1. Build each side with the profiler build (see [memprof](memprof.md#the-profiler-build)):

   ```powershell
   python scripts/memprof.py build --out bin/cand
   python scripts/memprof.py build --out bin/base --worktree ../base-checkout
   ```

   Any `--features` with work counters publishes the phase (`allocation-audit`, the default, or
   `engine-bench`). Use the same features on both sides.

2. Ask the question:

   ```powershell
   python scripts/timeprof.py run --exe bin/cand/engine_bench.exe --base-exe bin/base/engine_bench.exe `
     --out out/p07 -- C:/pages/p07_table.html --viewport 800x600 --mode relayout --runs 20
   ```

   A frame is short and a sample is one millisecond, so give the bench enough frames (`--runs`):
   a phase needs a few hundred samples before a 5% line in it means anything. Wrap the bench with
   `--launcher "<command prefix>"`, for example a hidden-desktop launcher. The run obeys
   `KAMIN_BENCH_HOLD` like memprof.

3. Read `out/p07/report.txt` (also `report.json`):

   ```text
   timeprof, every phase; main thread wall time by samples, ms
     cand: 5435 samples, target ran 4.67 s and stood suspended 7.6% more, interval 1000 us
   by phase:
          wall    on cpu   waiting   threads frames per frame     bench  phase
       2039.19    198.97   1840.22     16.55     14      2.39         -  outside
       1104.59   1085.00     19.59      3.58     13     40.35     42.53  layout
        742.28    725.23     17.05      2.78     13     31.02     32.09  build_elements
        330.86    326.05      4.81      1.60     13      2.24      1.79  prepaint
         88.16     51.72     36.44      4.08     13      6.01      4.28  raster_gpu_readback
   by subsystem (who the time belongs to):
   by where the thread was:          <- engine code, allocator, a wait, or a system DLL
   by site (first engine frame from the leaf):
   by function, inclusive (time with the function anywhere on the stack):
   by stack (depth 8 from the site, leaf first):
   ```

   (A run of a many-paragraph page, `--mode relayout --runs 10`, on a build without LTO; with a
   base, `base` and `delta` columns follow.) `wall` is the main thread's time in the phase by
   samples, summed over the frames it saw: 13 here, the ten measured ones plus the cold frame before
   them and the fresh-document checks after. `per frame` is its median over those frames and `bench`
   the median of the same phase as the bench timed its measured frames itself, so the two columns
   show how much the sampler saw; the sampled figure leaves out the time the target stood suspended.
   `on cpu` and `waiting` split the wall time by the thread's own CPU counter: a phase that waits
   for the GPU or for a present has little CPU and its stacks end in the waiting call. `threads` is
   CPU time of other threads during the phase. `outside` is the time between frames (settling).

4. Narrow it without running again (`cand.samples` and `base.samples` stay in `--out`; they are small):

   ```powershell
   python scripts/timeprof.py report bin/cand/engine_bench.exe out/p07/cand.samples `
     bin/base/engine_bench.exe out/p07/base.samples --phase layout --focus compute_grid_layout
   ```

   `--phase` and `--frames A-B` keep the samples of one phase or of some measured frames (numbered
   from 1 in process order, as in memprof). `--focus NAME` keeps the samples that have a function
   whose name contains NAME on the stack and adds the table of its callees, inclusive, four levels
   deep: the split of one function into its stages (grid: placement, contributions, track sizing)
   that used to need a timing probe. `timeprof.py diff A.json B.json` compares two saved reports.

## What a number means

- Sampling, not instrumentation: a row is `samples × weight`. With N samples in a row the error is
  about `1/√N` of it (100 samples: ±10%). Rows of a few samples are noise; use more `--runs`.
- A sample weighs the time the target ran since the previous tick. The time the target stood
  suspended under the sampler is taken out, so sums are comparable with an unprofiled run; the header
  says how much longer the run took.
- Inlined functions are restored from the PDB line tables, so a stack shows the functions of the
  source even where the optimiser merged them into one body, and two builds that inline differently
  still compare by name. Identical functions folded by the linker cannot be told apart: use
  `memprof.py build` (a report on another exe warns and marks the rows `{ICF n}`).
- A site is the first frame from the leaf that is not allocator or std plumbing; the subsystem is
  the first frame from the site up that matches memprof's `SUBSYSTEMS` table. Time inside the
  allocator, `memcpy` or a system DLL therefore belongs to the engine function that called it;
  «by where the thread was» shows that split separately.
- The profile answers «where is the time»; it does not count events. «How many times was X called»
  needs a work counter in the engine (`kamin_html::work`), which the bench already reports per phase.

## How it works and where it comes from

- The sampler (`scripts/timeprof_sampler.py`) is a separate process and needs no administrator
  rights: it suspends a thread of the bench, copies its registers and the used part of its stack
  (one `ReadProcessMemory`, 256 KiB at most by default, `--stack-kb`), and resumes it: about 0.1 ms
  stopped per sample. Kernel sampling through ETW (xperf, WPR, samply) needs the system-profile
  privilege, which a normal user does not have. This is the approach of the Gecko profiler on
  Windows (suspend, read registers, walk), moved out of the process so the measured binary carries no
  sampler and cannot deadlock with it.
- The stack is walked afterwards on the copy (`scripts/timeprof_unwind.py`) from the same data the
  system unwinder uses: `.pdata` function tables and unwind codes of each loaded image, with prologs
  undone step by step, chained records followed and epilogs simulated. dbghelp's `StackWalk64` on
  the live thread gives the same frames but keeps it stopped for about a millisecond per sample.
- The main thread is sampled at every tick, running or not (wall time); other threads only while
  they use the CPU, weighted by the CPU time they used (`QueryThreadCycleTime`). `--main-only`
  leaves them out.
- The phase comes from the bench: `ENGINE_BENCH_PROFILE_STATE=<file>` makes it write the address of
  two counters (measured frame, phase index) and the phase names; the sampler reads the counters
  while the main thread is suspended, so a sample belongs to exactly one phase, as in Chromium's
  tracing merged with its sampling profiler. Phase boundaries cost the bench two stores.
- Reports reuse memprof's folding: sites, subsystems, candidate against base, JSON for `diff`.

## Files

| file | content |
| --- | --- |
| `cand.samples`, `base.samples` | one line per sample: thread, frame, phase, weight, CPU time, program counters; then the module list |
| `cand.json`, `base.json` | the bench's own JSON of that run |
| `report.txt`, `report.json` | the folded report (`kamin.timeprof.v1`) |

## Limits

- Windows x64 only. The bench must come from a commit that publishes the profile state, built with
  work counters; an older exe is refused with a message.
- A stack deeper than the copied part is cut at the root side (counted as `truncated stacks`); its
  leaf side, which sites and subsystems use, is intact.
- GPU work is seen as the CPU time and the waits of the calls that submit it, not as time on the
  GPU itself.
- A few stacks per thousand do not reach the thread's start (in the run above 8 of 4856 main-thread
  samples ended inside a graphics driver DLL without unwind data); their leaf side is still used.
- Shares by subsystem hold on a `memprof.py build --screen` exe, but its milliseconds are not the
  release ones: time a final comparison on the default build of both sides.
