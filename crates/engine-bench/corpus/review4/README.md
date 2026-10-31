# Review 4 work-scaling corpus

These synthetic HTML inputs preserve the heavy, scaling and reproduction pages
from both review-4 authors. `manifest.json` identifies each input by SHA-256;
the runner verifies those hashes before execution. JavaScript probes retained
in the source are inert in the engine benchmark host.

Build the release host with `--features engine-bench`, using the repository's
shared build-slot procedure. Run from the repository root:

```text
python scripts/engine_perf.py --exe <engine_bench.exe> --out <outside-repo-dir> --suite guard
python scripts/engine_perf.py --exe <engine_bench.exe> --out <outside-repo-dir> --suite all --record-only
```

Each page gets its own process. Both cold and warm samples retain all exclusive
phase timings, integer work counters, all-thread process CPU, RSS/private commit
and available DXGI process memory. `phases.tsv` accompanies raw JSON and stderr.
Memory peaks are sampled every 5 ms and at phase boundaries; the JSON separately
labels lifetime high-water marks and end-of-phase observations. GPU observation
failure is explicit, never interpreted as zero usage.

The gate compares integer work counts between adjacent input sizes using
cross multiplication. Wall time, CPU time and memory are observations, not gate
thresholds. Expected failures identify an exact mode/scaling edge and the
package that must remove it. A new failed edge, stale expected failure, timeout,
missing sample or disabled counters fails the guard. `--record-only` records
those conditions without asserting the guard.

The full corpus includes known admission failures and expensive pages. It is a
local diagnostic suite; it is not on the pull-request release critical path.
