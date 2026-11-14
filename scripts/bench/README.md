# Paired benchmark failure artifacts

`pairs.py` is the portable runner source. The deployment adapter supplies `hb.py`
(hidden desktop launch, FIFO scheduling and corpus paths), `seqstop.py` and
`verdict.py`. Pass its directory explicitly:

```text
python scripts/bench/pairs.py OUT BASE_EXE CAND_EXE PAGES 1 both,relayout,edit 0 1 --adapter-dir=DEPLOYMENT_ADAPTER
```

Each process attempt gets its own `attempts/<stem>/<number>/` directory with
`stdout.json`, `stderr.err`, `frames/` and `fresh/`. The first canonical `<stem>.err`
is never replaced. A successful retry supplies the canonical `<stem>.json`, while
`failures.json` continues to identify the failed attempts and their artifact paths.
Resuming a missing sample allocates a new attempt directory.

The engine saves the baseline, first differing frame and absolute RGB difference
when `--frame-diff-dir` is supplied. A refused final frame check also writes
`failed-frame.json` with its original reason and report before the process exits.
No retry erases that attempt's pixels or refusal. Capture I/O errors are printed
alongside the original refusal without replacing it.

Run the synthetic runner regression without a GPU or deployment adapter:

```text
python -m unittest discover -s scripts/bench -p test_pairs.py
```

The adapter's scheduling, audit counters, retry limits, acceptance policy and
verdict implementation remain authoritative. A successful retry does not clear
the recorded failure.
