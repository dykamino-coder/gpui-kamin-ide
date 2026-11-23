# Exporting a recorded heap peak

`memprof.py run --peak` records the whole allocation stream, reconstructs its
largest live heap, and writes `cand-peak.json` (and `base-peak.json` for a pair)
before deleting the raw stacks. It implies `--owners`:

```powershell
python scripts/memprof.py run --exe bin/profile/engine_bench.exe --peak --out out/peak `
  -- C:/pages/example.html --mode cold --runs 1
```

Use one mode per run. The checkpoint is the first maximum before the first owner
record. It contains the recorded line, frame, phase, live requested bytes, block
count, sample interval and dropped-record count. The JSON has separate `peak`
and `end` reports; `end` freezes the heap at the owner report, before teardown.
The ordinary `report.json` and text output remain available.

Existing saved pointer streams can be exported without running the page again:

```powershell
python scripts/memprof.py peak bin/profile/engine_bench.exe out/cand.stacks --out out/peak.json
```

Phase-filtered and pre-v2 streams are rejected: they cannot reconstruct a whole
live heap. Sampling estimates the peak; use `--sample 0` for exact allocation
records when disk space and runtime allow it. Dropped records make either form
incomplete. Existing timeout, file-size, free-space and hold checks still apply.

## What the owner labels prove

An allocation site says where a block was allocated. An owner path says which
document/window field reported it during the later owner walk. These are separate
tables. The peak report accepts a later owner label only if the same block
survived unchanged from the peak to that walk. A free, realloc, or new allocation
at the same address invalidates that label. Null-address estimates and later GPU
texture totals are excluded from the peak owners.

This is **not a root walk taken at the peak**: the root may have changed in the
meantime. Transient peak allocations remain unlabelled and retain their allocation
stacks. Setup allocations before the first owner record can contribute to the
recorded peak. GPU/native allocations and process working set are different
metrics; use the bench GPU report and `memprof.py ws` for those. The JSON repeats
these limits so an exported file cannot silently lose them.

## Building an audited source export

The default profiler build still uses `<worktree>/target/prof`. A source export
without `.git` must name its repository and commit explicitly:

```powershell
python scripts/memprof.py build --worktree C:/exports/source --source-repo C:/repo `
  --source-commit FULL_SHA --target-dir C:/repo/target/prof --out bin/profile
```

`--target-dir` must already exist; it does not waive local build policy. The tool
checks every tracked blob byte for byte, rejects missing/extra/linked inputs,
and verifies the same identity after cargo completes. Only the export's root
`target` output is excluded. Use a byte-exact export, rather than a checkout with
newline conversion. A nested directory cannot borrow its parent checkout's HEAD.

`build.json` records the verified commit/tree, blob count and listing digest,
requested/resolved target paths, and SHA-256 of the copied exe and PDB. Existing
PE stack-reserve, PDB function and identical-code-folding checks remain mandatory.
