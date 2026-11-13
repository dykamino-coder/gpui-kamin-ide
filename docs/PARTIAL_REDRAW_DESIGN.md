# Partial redraw design: INC-2026-0064

Source baseline: `origin/main` `81ed673f` (2026-11-13). Design only: no
renderer code changes here. Implementation is the owner's scheduling decision
(card: [INC-2026-0064](../extensions/claude-bridge/runtime-issues/INC-2026-0064.md)).

## 1. Current pipeline (verified in source)

| Stage | Where | What happens every frame |
| --- | --- | --- |
| Invalidation | `vendor/gpui/src/window.rs` `WindowInvalidator::dirty_views` | Only dirty views re-render; others replay through `reuse_prepaint`/`reuse_paint` |
| Scene build | `vendor/gpui/src/scene.rs` `Scene` | All primitives (reused or new) are copied into `next_frame.scene`; `BoundsTree` orders them; `groups` hold subtrees drawn into their own buffers (blur, opacity, blend, masks, 3D planes) |
| Raster | `vendor/gpui-windows/src/directx_renderer.rs` `render` → `pre_draw` | `ClearRenderTargetView` on the whole back buffer; `render_groups` re-renders every group buffer; `draw_scene` submits every batch; rasterizer state has `ScissorEnable: false` |
| Present | `DirectXRenderer::present` | `Present(0, 0)` on a `FLIP_SEQUENTIAL` swap chain with `BUFFER_COUNT = 3`; no dirty rects |
| CEF views | `crates/shell/src/web/render_handler.rs` | `on_accelerated_paint` ignores `_dirty_rects`; `on_paint` counts them for diag only |
| Metrics | `vendor/gpui/src/batch_cost.rs`, `[gpu]` line in `web/diag_why.rs` | Per-kind batches, primitives and `area_px` (painted area) per frame |

So GPUI already avoids CPU rebuilds of unchanged views, but the GPU (and WARP,
which is the CPU) rasterizes the whole window for any change. INC-2026-0001
measured about 12 Mpx painted per frame for a 1.8 Mpx window before #261.

## 2. How the reference engines do it

- **Chromium (cc/viz).** `cc::DamageTracker` computes damage per render
  surface from layer property changes and layer update rects; it expands
  damage for filters that read neighbouring pixels (blur, backdrop filters).
  `viz::SurfaceAggregator` unions child-surface damage from each
  CompositorFrame's `damage_rect` (this is where an embedded web view's dirty
  rect enters). `DirectRenderer` then sets a scissor to the root damage and
  skips quads outside it; on Windows the output presents with dirty rects
  (`Present1`) when partial swap is supported, and falls back to full frames
  on resize, buffer loss and when damage covers most of the output. Buffer
  reuse is tracked per buffer: a back buffer that is N frames old must be
  repainted with the union of the last N damages.
- **Firefox WebRender.** Picture caching splits the scene into tiles and keeps
  per-tile dependency descriptors (primitive ids, transforms, clip, image
  generations). A tile is invalidated only when its descriptor changes, which
  makes invalidation correct by construction rather than by bookkeeping.
  Dirty tiles drive scissored raster and `Present1`/DirectComposition partial
  updates.
- **Zed (upstream GPUI).** No damage tracking: views are cached on the CPU
  (`cached()` views, `reuse_paint`), but each frame resubmits the full scene
  and presents the full drawable. Our vendor copy keeps that model.

Our constraints: one swap chain, no DirectComposition layer tree to delegate
to, CEF frames arriving as shared D3D11 textures, and WARP as the case that
matters most. The approach below takes Chromium's damage/scissor/Present1
pipeline and WebRender's tile-descriptor invalidation; it does not copy code.

## 3. Proposed design

### 3.1 Damage source: tile descriptors over the finished scene

Compute damage after the scene is complete, independent of which view
produced it:

1. Split the window into fixed 256×256 device-pixel tiles.
2. For every primitive (shadows, quads, paths, underlines, sprites, surfaces,
   group composites) compute its clipped device bounds (already needed for
   `BoundsTree`) and fold a 64-bit hash of its draw-relevant fields (kind,
   bounds, content mask, colors/borders/corner radii, atlas tile id + atlas
   generation, sprite transform, surface texture id + content generation) and
   its draw order into every tile it touches.
3. Damage = union of tiles whose hash differs from the previous frame's hash.
   Tiles of disappearing primitives change hash too, so removal is covered.

Why not view-level bookkeeping first: views move without re-rendering
(scroll offsets, `replay_shifted` blocks), hover and focus state reach paint
through several paths, and missed damage produces stale pixels that tests
rarely catch. A scene-level descriptor is correct for anything that reaches
the scene. Cost is O(primitives) hashing on the CPU, which is small next to
rasterizing megapixels on WARP; `[gpu]` already counts primitives per frame
to verify that.

Inputs outside the scene must enter the descriptor explicitly:

- **CEF surfaces.** Each surface keeps a content generation incremented on
  `on_accelerated_paint`/`on_paint`. With a known dirty rect from CEF, only
  the mapped sub-rectangle is damaged; otherwise the whole surface bounds.
- **Atlas.** Re-rasterized or evicted glyph/image tiles bump the atlas
  generation, damaging the tiles that sample them.
- **Paint groups.** A group's composite primitive hashes its own scene's
  combined hash, so any change inside the group damages the group's output
  bounds. Blur and backdrop filters expand damage by the filter radius (the
  same rule as `cc::DamageTracker`), and a backdrop reads what is beneath it,
  so damage under a backdrop region also damages that region.
- **Global frame state.** `window_active`, opaque backdrop color, gamma and
  font settings, scale factor and viewport size are part of a frame header;
  any change forces a full frame.

### 3.2 Buffer age

The swap chain is `FLIP_SEQUENTIAL` with three buffers, so the buffer being
rendered holds the frame from three presents ago. Keep a ring of the last
`BUFFER_COUNT` damages; the region to repaint is the union of the current
damage and the damages since that buffer was last presented. Unknown age
(after `ResizeBuffers`, device loss, `skip_draws`, or a capture frame) means
a full frame.

Frames skipped by the latency gate (`frame_gate` returning `WAIT_TIMEOUT` in
`DirectXRenderer::draw`) are not presented: their damage must accumulate into
the next drawn frame rather than be dropped. This is the main correctness trap
of the current renderer specifically.

### 3.3 Raster

- Reduce damage to at most 4 rectangles (merge tiles; Chromium keeps a single
  root rect, which is too coarse for "caret plus spinner in opposite
  corners"). If the merged area exceeds 50% of the window, draw a full frame.
- Replace the full `ClearRenderTargetView` with `ID3D11DeviceContext1::ClearView`
  restricted to the rectangles (or a clear quad under scissor).
- Create a second rasterizer state with `ScissorEnable: true`; for each damage
  rectangle set `RSSetScissorRects` and draw the batches whose primitives
  intersect it (query `BoundsTree`, keep draw order). Instances outside every
  rectangle are not uploaded.
- `draw_paths_to_intermediate` and the group buffers clear and draw under the
  same rectangles mapped into their space; group buffers whose bounds do not
  intersect damage are neither re-rendered nor composited.
- The same shaders and draw order run inside the scissor as in a full frame,
  so pixels inside damage are identical to a full repaint, and pixels outside
  are the retained previous content.

### 3.4 Present

Use `IDXGISwapChain1::Present1` with `DXGI_PRESENT_PARAMETERS::pDirtyRects` =
the current damage (not the age-accumulated region). For flip model DXGI
requires pixels outside the dirty rects to equal the previous presented frame;
3.2 guarantees that. Dirty rects also let DWM and the RDP encoder process
only the changed area, which is the expected extra win for RDP sessions; it
must be measured, not assumed.

### 3.5 Always-full cases

First frame, resize, DPI/scale change, device lost/recovered, `skip_draws`,
`render_to_image`/WPT frame capture (`self.capture`), window activation change,
theme or font-setting change, atlas reset, damage area above the threshold, and
an explicit kill switch (`KAMIN_PARTIAL_REDRAW=0`).

## 4. Delivery plan (each step a separate Change PR)

1. **Shadow mode.** Compute tile descriptors and damage, report
   `damage_px`, `damage_rects` and full `area_px` in the `[gpu]` line; no
   raster change. Automated tests on synthetic scenes: caret, spinner, hover,
   scroll by `replay_shifted`, primitive removal, group/blur expansion, CEF
   generation bump, skipped-frame accumulation and buffer age.
2. **Verifier.** Debug/test option that renders each frame both partially and
   fully into offscreen WARP targets and compares them byte-for-byte; a
   scripted sequence of scenes runs in Windows CI (WARP needs no GPU).
3. **Scissored raster + Present1** behind `KAMIN_PARTIAL_REDRAW=1`, plus CEF
   dirty-rect plumbing.
4. **Default on** after the Windows runtime gate below; keep the kill switch.

## 5. Acceptance (unchanged from the card, made measurable)

- Windows runtime merge gate on WARP (forced software adapter) and on a
  hardware adapter, on a copy of the owner profile: idle caret blink, a running
  spinner, row hover and a CEF chat update each report `damage_px` within the
  changed region (caret frame ≤ 1% of window area), while the verifier finds no
  pixel differences over the sequence.
- CPU time per frame on WARP for caret blink drops in proportion to the area,
  measured with the existing `[gpu]`/frame timing lines against the current
  main on the same machine.
- No regression in input latency (the `frame_gate` behaviour from #76 is
  kept) and device-loss recovery (`docs/GPUI_DIRECTX_RECOVERY.md`).

## 6. Out of scope

The HTML engine (`crates/html`) rebuilding cascade and layout for the whole
page is a separate engine task; this design only removes whole-window raster
and present cost in the shell renderer.
