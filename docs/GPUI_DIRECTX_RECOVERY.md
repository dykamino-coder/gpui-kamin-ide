# DirectX recovery: INC-2026-0023 / INC-2026-0025

Source baseline: `c07b0a9` on `wpt/fix-gpui-recovery`, GPUI 0.3.8.
The old `vendor/gpui/src/platform/windows/` files now live under
`vendor/gpui-windows/src/`. Incident cards were read from `origin/main` and
are not modified by this change.

## INC-2026-0023: already fixed by the upgrade

Import commit `f726565` added the reset at the end of
`DirectXRenderer::handle_device_lost_impl`: `BlurScratch::default()`, both
group target fields, and both lazy blend states. `BlurScratch` owns the copy,
downsample/group targets, blur constants, mask cache and group metadata.
Consequently the dimension-only reuse checks cannot return a previous-device
effect after successful recovery. This patch moves that same reset before
fallible allocation so failed recovery also releases the effect caches.

The headless generation test populates these resources on device A, recovers
to B without resizing, alternates devices with changed dimensions, and covers
an empty effects scene. It verifies cache invalidation and queries actual
COM resources with `GetDevice` after lazy reallocation.

## INC-2026-0025: upgrade fixed unsafe release, but not the whole failure path

At the baseline, devices/resources and the back buffer use `Option::take()`;
there is no `ManuallyDrop` ownership left in the renderer. Replacement objects
are locals until all fallible construction succeeds. The platform recovery
uses `?` before logging success or dispatching new devices. These parts of
the old incident are already corrected upstream.

Two source-level counterexamples remain at that baseline:

1. `resize` stores the requested width/height, takes the target/view, and then
   propagates `ResizeBuffers` or `recreate_resources` failure. Retrying the
   same size immediately returns `Ok(())` with both slots absent.
2. Recovery derives its composition policy from `direct_composition.is_none()`,
   then takes that field before allocation. After a failed attempt the next
   attempt therefore switches from a composition swap chain to an HWND swap
   chain. `skip_draws` is only set on success; raw device access and `draw`
   can also reach `expect` on the now-empty device/resource slots.

The fix retains the constructor policy, disables drawing before recovery,
guards missing draw resources/raw device access, and lets a same-size resize
repair missing targets. Tests inject failed replacement stages and failed
resize/partial allocation, exercise the five-attempt helper, retry from empty
owners, and drop unavailable renderers. Native composition allocation and
driver removal themselves are not simulated by these injected boundaries.
Globals, pipelines and the atlas can retain live COM owners during failure;
they are inaccessible to drawing until the replacement is installed. They are
not manually released slots. Effect caches are released at recovery entry.

No test creates a window or presents. Sleep/RDP, visual/input acceptance and
driver diagnostics in a disposable Windows instance remain unrun; automated
ownership tests do not constitute the incident cards' Windows runtime gate.

## Local verification

| Exact command | Observed result |
| --- | --- |
| `cargo fmt --all -- --check` | Failed on pre-existing formatting outside this fix, including `crates/html`; no formatting applied there. |
| `rustfmt --check --edition 2024 vendor/gpui-windows/src/directx_renderer.rs vendor/gpui-windows/src/window.rs` | Passed, including the renderer's child modules. The vendored package is not a workspace member, so direct `cargo fmt -p gpui-pre-windows` cannot select it. |
| `cargo clippy -p gpui-pre-windows --all-targets -j 2 -- -D warnings` | Exit 0; no Windows-crate warnings. Dependency `gpui-pre` emits 11 existing warnings. |
| `cargo test -p gpui-pre-windows --lib -j 2 directx_renderer::recovery_tests` | 4 passed, 0 failed. |
| `cargo test -p gpui-pre-windows --lib -j 2 -- --skip platform::tests::test_clipboard` | 21 passed, 1 failed: unchanged `keyboard::tests::test_keyboard_mapper` expects US `Shift+4` to produce `$`, but native mapping produces `;` with this environment's layout. Clipboard test skipped to preserve the user's clipboard. |
| `cargo build --example wptrun -p kamin-html` | Exit 0, dev/debug profile; existing GPUI/HTML warnings remain. `CARGO_BUILD_JOBS=2` was set for this process. The runner was not launched. |
| `git diff --check` | Passed. |

Neither `crates/html` nor the incident cards were changed. No GUI, push, PR or
merge was performed. The full library test and workspace formatting check are
not reported as passing.
