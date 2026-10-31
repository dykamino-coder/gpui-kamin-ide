# Exact main baselines on Windows

The separate `main baseline artifacts` workflow (`main-baseline.yml`) runs on
every push to `main` and publishes `kamin-main-baseline-<full commit SHA>` for
three days after a successful build. It has no PR or manual trigger. It is
independent of `pull request checks` and its quality gate; `docker.yml` listens
only to `pull request checks`. A slow or failed baseline build cannot delay or
block release publication. Docs-only quality runs keep their existing Rust scope.
No release or version change is involved.

From a Windows checkout with Git, Python 3.8+ and authenticated GitHub CLI:

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
| `shaders.hlsl`, `color_text_raster.hlsl`, `alpha_correction.hlsl` | Exact runtime GPUI shader sources, with sizes and SHA-256 in the manifest |

Dev GPUI compiles HLSL at runtime, using an absolute `CARGO_MANIFEST_DIR` path
embedded in the executable (`vendor/gpui-windows/src/directx_renderer.rs`). An
executable copied out of the default CI checkout cannot find those sources on an
agent machine. The builder's `--ci-layout` option makes a shallow local clone of
the exact detached commit into
`C:/Users/Public/Documents/KaminIDE/main-baseline/<SHA>` before compilation.
The downloader installs the three verified HLSL files into that same per-SHA
`vendor/gpui-windows/src` directory. This small runtime cache is required in
addition to the requested output directory. It needs ordinary write access to
Public Documents; an existing shader is checked and never overwritten. The
manifest cannot choose an arbitrary installation path. Each SHA has its own path,
so refreshing another baseline cannot change an older runner's shaders. Keep this
runtime cache while using the corresponding executable. No renderer code,
compiler flags, binary patching or release-profile substitution is involved.

The runner command, target directory and effective dev profile match the local
gate's `build_shared.sh`. The measured gate machine sets
`profile.dev.debug = "line-tables-only"` in its user Cargo config, alongside a transparent sccache
wrapper. The CI builder explicitly sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only`
for the runner, reproducing that effective value without a `--config` profile
override. Other profile settings come from the same tracked Cargo.toml. Local
sccache changes caching, not the rustc rendering configuration.

`--message-format=json` only identifies Cargo's emitted executable. The separate
workflow has no quality-job debug/incremental overrides; the builder rejects
incoming compiler/profile overrides before applying the measured gate debug
value. The manifest and downloader pin `gate_debug` to `line-tables-only`.
Both test binaries use the repository's
`html-test` alias, including its agent profile and linker; they are not substitutes
for the dev runner in pixel comparisons. Every build uses four Cargo jobs.

The existing quality binaries cannot be reused: their debug/incremental settings
and test profile differ. The workflow restores the quality cache and registry
inputs using only `actions/cache/restore` for `windows-cargo-v2`; it never saves
into the shared quality cache. The differing profiles and per-SHA source layout
require separate compilation. `target/fast` isolates gate output. Exact baseline
compilation has a cold cost on each main push; manifest timings make that cost visible. Only
the three executables, three HLSL files and manifest are uploaded, without PDBs,
logs or targets. The per-SHA source clone and its target are ephemeral runner
storage and are not added to the shared cache.
Build durations in the manifest expose the additional cost on the actual runner.

The downloader queries `actions/workflows/main-baseline.yml/runs` for the exact
SHA and checks a successful main **push**, workflow path, source repository,
run SHA and artifact SHA before checking the downloaded archive against GitHub's
SHA-256 digest. It then checks the manifest commit/target/configurations and every
executable's and shader's SHA-256 and size. Unexpected archive members are rejected; files are
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

## Using a baseline in exact gates

The exact RGB criterion and the approved test list stay unchanged. Load the
baseline executable from this artifact for the exact base SHA when possible and
use it only after the manifest, hashes, profile/features/toolchain and GPU
context match. A missing or incompatible artifact requires a safe local rebuild
or an explicit blocker; comparing against an approximate baseline is forbidden.

Iterations may run the approved target and control families. Before a renderer
change is integrated, the full approved gate runs. Changing the gate scope or
profile remains the lead's decision. Every run manifest records the test-list
version and the pair count; a historical pair count is not assumed current.

An incomparable GPU/platform context is neither a pass nor an RGB loss: restore
compatibility first. A load flake is confirmed only by the current 2/2 repeat.
Lowering the RGB threshold or silently excluding tests is forbidden.
