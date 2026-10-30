# Exact main baselines on Windows

After a successful `pull request checks` push run on `main`, the Windows Rust
job publishes `kamin-main-baseline-<full commit SHA>` for three days. Pull request
and manual runs do not publish baselines. Main pushes always run this Windows job,
even when changed-file scope would otherwise skip Rust, so every main SHA gets a
baseline. No release or version change is involved.

From a checkout with Git, Python 3.8+ and authenticated GitHub CLI:

```powershell
git fetch origin
python scripts/fetch_main_baseline.py --output C:\baselines
# Or select an exact main push commit:
python scripts/fetch_main_baseline.py <full-SHA> --output C:\baselines
```

The script creates `OUTPUT/<full-SHA>` containing:

| File | Build configuration |
| --- | --- |
| `main.exe` | `cargo build --example wptrun --features kamin-html/wpt-harness`, dev profile, implicit `x86_64-pc-windows-msvc` host, `target/fast` |
| `html-tests.exe` | `cargo html-test --locked --no-run`, no additional features |
| `html-tests-wpt.exe` | `cargo html-test --locked --no-run --features wpt-harness` |
| `manifest.json` | Commit, source tree, rustc, target, configuration input hashes, executable sizes/hashes and build timings |

The runner command, target directory and effective dev profile match the local
gate's `build_shared.sh`. The measured gate machine sets
`profile.dev.debug = "line-tables-only"` in its user Cargo config, alongside a transparent sccache
wrapper. The CI builder explicitly sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only`
for the runner, reproducing that effective value without a `--config` profile
override. Other profile settings come from the same tracked Cargo.toml. Local
sccache changes caching, not the rustc rendering configuration.

`--message-format=json` only identifies Cargo's emitted executable. The workflow
clears the quality job's debug/incremental overrides first; the builder rejects
incoming compiler/profile overrides before applying the measured gate debug
value. The manifest and downloader pin `gate_debug` to `line-tables-only`.
Both test binaries use the repository's
`html-test` alias, including its agent profile and linker; they are not substitutes
for the dev runner in pixel comparisons. Every build uses four Cargo jobs.

The existing quality binaries cannot be reused: their debug/incremental settings
and test profile differ. Cargo restores the existing quality cache and registry
inputs; the differing profiles require separate compilation. `target/fast` isolates gate output. The
quality cache is saved before baseline compilation so gate output, incremental state
and agent output do not enlarge it. Exact baseline compilation therefore has a
cold cost on each main push; manifest timings make that cost visible. Only
the three executables and manifest are uploaded, without PDBs, logs or targets.
Build durations in the manifest expose the additional cost on the actual runner.

The downloader checks a successful main **push**, workflow path, source repository,
run SHA and artifact SHA before checking the downloaded archive against GitHub's
SHA-256 digest. It then checks the manifest commit/target/configurations and every
executable's SHA-256 and size. Unexpected archive members are rejected; files are
written only after all checks pass. An existing output directory is never replaced.
The SHA-256 field comes from the [GitHub artifact API](https://docs.github.com/en/rest/actions/artifacts).

These executables are trusted repository build products, not signed releases.
The manifest provides integrity and provenance within that trust boundary. The
script reads no tokens itself and uses `gh`'s normal authentication. A failed,
pending or expired run has no downloadable baseline; use the persistent local
main slot in that case. Updating `origin/main` remains the caller's responsibility.

The html test executables need the same test corpora and working directory as
`cargo html-test`. Direct execution must also set the test-thread stack environment
from `.cargo/config.toml`. Run from the matching source checkout, for example:

```powershell
$env:RUST_MIN_STACK = "8388608"
& C:\baselines\<full-SHA>\html-tests-wpt.exe --list
& C:\baselines\<full-SHA>\html-tests-wpt.exe <test-name> --exact
```

Pixel equivalence must be checked on the same Windows GPU, with both runner files
named `main.exe`, the same WPT pairs, fonts, environment and exact RGB comparison.
CI does not attest pixels on a local GPU. A locally built gate runner from the
same commit is the reference for accepting the first downloaded artifact.
