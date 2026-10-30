# INC-2026-0084 execution

Current checkpoint: **BLOCKER — engine fix and acceptance are incomplete**.
At tool-call 360/400 the driver required finishing the current cluster without
new pilots. This branch is not READY and does not complete the engine fix.

- Branch: `fix/inc84-deep-paint-frames`; exact starting main: `d9dcc67728d92ef1c75e521438f17c0181f2db0e`.
- Owner decision 2026-10-30: the depth contract must hold without optimization.
- Read repository rules, architecture, incident, vendor workflow and shared machine rules.
- Baseline wptrun build queued through the unchanged shared build script, `JOBS=4`, own target directory.
- All three global build slots were occupied at the start. No other process or file is modified.
- Product shell depends on CEF and not on kamin-html; a shell overflow on this HTML path is not established.
- Implementation, frame measurements, benchmarks and acceptance gates are pending.
- The shared script changed to four slots during the queue. Restarted only my
  waiting bash process (PID 58908 and child 26144); baseline compilation began
  in the fourth slot with JOBS=4. No shared script was edited.
- Prepared isolated address probes at N=16/32 and a COFF-unwind extractor;
  instrumentation is temporary and will be removed before acceptance.
- Updated my queued cargo launcher to the current four-slot protocol; stopped only my waiting childless PIDs 32744 and 47224 and restarted both queues. Shared scripts and other agents were untouched.
- Added temporary Paragraph phase probes beside the common Hooked adapter,
  because Paragraph implements GPUI Element directly.
- Saved all temporary probes outside the checkout and restored the original
  source bytes before any HTML compilation. `git diff --stat` is empty for
  tracked files. The first agent test, wptrun and release engine-bench builds
  therefore compile exact starting-main sources; the native debugger now avoids needing
  to reapply measurement instrumentation.
- Prepared isolated 1026-level build/request/layout/prepaint/paint invocations
  for all seven kinds. COFF unwind parsing was checked on the completed Taffy
  archive; generic recursive layout frames are instantiated in GPUI/HTML.
- Baseline wptrun and agent tests compile dependencies in separate targets;
  release engine-bench also entered its slot (cargo process started at 20:40).
  No baseline binary or frame result was available. Free C: space remained
  above 300 GiB.
- Prepared a native entry-RSP sampler using local DbgHelp symbols and Windows
  debug events. Its breakpoint/restore/single-step cycle passed a smoke check
  on our own Python child. This avoids source instrumentation and captures
  the recursive Taffy layout path too. No symbol server or token is used.
- At 20:59 all three original builds remain in progress; no engine fix or
  acceptance result is claimed. Baseline source files remain unchanged.
- The linked-PE frame decoder agrees with llvm-readobj on all 2954
  symbol-matched frames in an existing Rust DLL. These are tool validation
  results, not INC-2026-0084 measurements.
- Updated the incident with the owner's fixed cross-profile depth contract;
  Fix PR remains for the lead. Engine sources and tests remain unchanged.
- Expanded Servo search across available components: the script thread explicitly
  selects 8 MiB, with no additional layout/paint depth counter found. The local
  Blink reference is sparse, so negative search results are scoped accordingly.
- Prepared event-driven continuations on our own original build PIDs: preserve
  agent EXE/PDB and run native/PE/phase probes after success; after baseline
  wptrun, build unchanged dev-profile tests through a shared slot and repeat
  the measurements. No log polling or early engine source change is required.
- Original-main wptrun completed in 98m30s with zero warnings; retained
  `base/main.exe` and its matching local PDB outside the checkout. Eight private
  fixture self-pairs completed 8/8 green with the unchanged offscreen shared
  runner. Saved all sixteen test/reference PNGs for benchmark image validation.
- Baseline wptrun PE unwind data already shows 3792-4160-byte frames in generic
  paint-context replay wrappers, plus 3424/3456-byte Paragraph paint/request
  frames. These are individual compiler frames at opt-level 2, not yet the
  measured recurring cost of the test paths. Original agent/dev test and release
  benchmark builds remain in progress.
- Detected displaced ownership of our dev-build slot immediately after the
  baseline handoff. Paused only our cargo PID 10708 and its compiler PID 53856;
  an own foreground continuation will resume them only after acquiring a real
  free slot. Other owners, lock files and the shared script were untouched.
  Future own launchers publish the Windows liveness marker first.

- Original-main agent tests compiled (2084 listed tests, not a passing suite).
  Exact special-depth regression overflows in multicol prepaint. Existing
  7-kind by 5-phase probes at depth 1026 passed 33/35 endpoints; multicol
  prepaint/paint endpoints overflow. Each invocation ran one actual probe.
- Completed all-seven-kind N=16/32 native measurements, full multicol chains
  and 209,808 original COFF unwind records. Recurring prepaint: 8256 bytes/level;
  `prepaint_frags`: 1520 bytes, `paint_frags`: 2080 bytes, column Hooks paint:
  928 bytes. Tables and measurement limitations are in design.md.
- Own helper 60132 acquired slot 3 and resumed only our paused cargo/compiler.
  Foreign processes/locks were untouched; free C: space remained over 300 GB.
- Existing baseline pipelines at checkpoint: cargo 10708 (dev tests,
  `after-dev.py`) and cargo 47012 (release engine-bench, `after-bench.py`).
  Wrappers release only their own slot markers; continuations retain outputs
  outside the checkout. Inspect final results before starting another build.
- Remaining: general frame reduction, both-profile measurements, actual release
  before/after timings on seven deep documents and normal page, full agent/dev
  suites, fmt/clippy/repository checks and exact-main zero-loss RGB gate using
  build_shared.sh main.exe binaries. No fix or acceptance claim is made.
- Documentation checkpoint checks: `cargo fmt --all -- --check` passed;
  `check_layer_imports.py --base d9dcc677` passed (0 errors, 0 exceptions);
  `check_transitional.py --base d9dcc677` passed (0 errors, 18 registered
  markers); `git diff --check` passed. Committed-tree conventions passed
  (`SUMMARY rules=0 dups=0`); local PR metadata validation passed. These checks
  do not accept the engine fix.
