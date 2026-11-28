#!/usr/bin/env python3
"""Regenerate and round-trip a complete crate patch against a clean upstream tree."""

import argparse
import base64
import difflib
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import zlib

ROOT = Path(__file__).resolve().parent.parent
PINS = {
    "gpui": "gpui-pre 0.3.8 (zed 279fe070bb389b79652e52065b2f001edcc0b11b)",
    "gpui-windows": "gpui-pre-windows 0.3.8 (zed 279fe070bb389b79652e52065b2f001edcc0b11b)",
    "gpui-base": "gpui-base 0.7.1 (longbridge/gpui-kit 87d10ae5e1299d1be18670a37570c5402f952e34)",
    "gpui-component": "gpui-component 0.7.1 (longbridge/gpui-kit 87d10ae5e1299d1be18670a37570c5402f952e34)",
    "taffy": "taffy 0.14.0 (DioxusLabs/taffy 2d936b7701f64b0fe95e50f1bfdcfc65a9f88f6f, cargo package)",
    "html5ever": "html5ever 0.40.1",
    "tendril": "tendril 0.5.1",
    "xml-rs": "xml-rs 0.8.29",
    "markup5ever_rcdom": "markup5ever_rcdom 0.39.0+unofficial",
    "resvg": "resvg 0.48.1 + src at linebender/resvg 77b12f4951a7b75d759c42e8c9497cff7c839c3c",
    "usvg": "usvg 0.48.1 + src at linebender/resvg 77b12f4951a7b75d759c42e8c9497cff7c839c3c",
}
PACKAGE_REVISIONS = {
    "gpui-base": "87d10ae5e1299d1be18670a37570c5402f952e34",
    "gpui-component": "87d10ae5e1299d1be18670a37570c5402f952e34",
    "taffy": "2d936b7701f64b0fe95e50f1bfdcfc65a9f88f6f",
}

FIXES = {
    "tendril": ["Admission: fallible reservation before tendril growth; a storage-owned lease survives shared views until the last allocation owner is dropped."],
    "xml-rs": ["Admission: canonical reader storage and leased events grow fallibly before mutation; a typed terminal driver releases partial state and stops reads, entity expansion and callbacks immediately.", "XML namespace constraints: reject duplicate declarations and duplicate expanded attribute names; recursive entity references remain syntax errors independently of work limits."],
    "gpui": [
        "Font fallback normalization: carry a typed raw metric target in the shared fallback configuration, including font identity and shaping cache keys, without growing Font or FontRun.",
        "Effect lifetime diagnostics: opt-in batch journal reports immutable mask uploads and texture owners released when the complete scene stops using their effect kind.",
        "Moved frames: a frame whose largest scroll node moved by whole device pixels is compared, cell by cell and with retained layers expanded, against the last frame moved by that step (both cut by the moved window); where they agree a renderer may copy the kept pixels, and the rest becomes at most four damage rectangles (`Scene::frame_shift`). Layer sources stay held from recording to present so the comparison reads what the last frame drew.",
        "Analytic gradients: immutable geometry and arbitrary sRGB stops belong to retained commands; replay, splicing, damage comparison and group batches preserve content while remapping scene-local descriptor indices.",
        "Measured float overflow: independently measured hosts return scrollable extents separately from normal-flow size; child contributions use native clipping, containment and relative offsets.",
        "Scroll replay: rebase transformed primitive bounds together with the matrix so gradient-local coordinates match a fresh paint without large cancelling translations.",
        "Fixed-background replay: stationary viewport content uses an independently scrolling clip; canvas clips stay anchored. Position-dependent complex clips rebuild instead of translating.",
        "Embedded document steps: snapshot the current element ancestry and rendering view so preparation outside draw uses the same IDs as nested hosts. No external publication authorized.",
        "Shared render image: a validated immutable dense BGRA frame borrows its Arc CPU owner without a pixel clone; CPU consumers can recover that owner, keeping its resource lease alive with the backing.",
        "Lazy render image: an owner may produce the dense BGRA frame only when the atlas uploads it, so the image keeps no resident CPU pixel copy; CPU readers decode a transient frame on demand.",
        "Raster image lifetime: each window tracks uploads with weak CPU-image owners and shared scene leases, including replay, clipping, transforms and retained scene copies. CPU cache eviction and the final scene release remove the atlas tile; natural resampling uses the same upload owner.",
        "Measured subtrees: retain an independent layout engine and identity ancestry through prepaint/paint, restoring caller ownership on unwind; deferred native draws and paint snapshots retain the arena owning their layout IDs, including reentry into an outer arena. Deferred construction restores its original identity, inherited text refinements and rem size without replaying geometry or paint state; immutable ancestry/text snapshots share exact-length slices rather than retaining traversal capacity.",
        "Intrinsic measurement: expose size-only axis queries and their purpose, and the axes the parent fixed for a final layout pass, to measured leaves (MeasuredBox) without replacing completed layout geometry or metadata.",
        "Settled subtree skipping: ink sinks collect the placed bounds of every primitive a scope offers (culled included, deferred paint snapshots feed the sinks open at capture) and are tainted by input, element state, groups or callbacks unless a scope declares them inert while unseen (counted also where no sink is open and carried out of an isolated scene, for the ink probes of the paint around it); exact layout leaves stand in for inflexible column items whose descendant margins do not adjoin their edges, and are plain leaves of the retained frame tree.",
        "Frame diagnostics: opt-in host logging correlates incoming/deferred force, dirty state and completed frame callbacks without changing scheduling.",
        "Element lifetime: isolate transient probe allocation with a scoped arena; invalidate handles before destruction and return overflow chunks after the outer draw scope.",
        "Container units: publish completed content-box dimensions once per persistent source per frame, settle nested style/layout inputs top-down before paint, and retain persistent element state.",
        "Measured static-position rectangles: retain host-local geometry with performed native leaf layout and expose the completed horizontal flex line of a zero-sized marker; completed-pass access rejects dirty/intrinsic results and independent measurement copies cannot replace it. Keep measurement callable with the stacker feature disabled. Upstream proposal is recorded privately for P07; no external publication authorized.",
        "Expose the first actual in-flow content baseline from completed layout without remeasurement; list-marker ownership and fragment policy remain in HTML.",
        "Upstream #66070: expose completed unrounded normal geometry with tree/pass identity, full root constraint space and device scale, independently of relative/paint placement.",
        "Native stack depth: keep Div phase styles on the heap and borrow Style while requesting layout, releasing large construction temporaries before descending into children. Drain atomic paint contexts in a separate frame after child painting returns; finish background, shadow and border temporaries before recursive paint. Isolate accessibility-node construction and completion from recursive prepaint frames. Borrow phase payloads in place while traversing children and move them only after traversal returns. Borrow text refinements in shared child-drawing scopes, finish element-ID temporaries before recursion, and isolate anonymous element-state dispatch from the stateful branch in prepaint and paint.",
        "Optional stack growth: measurement/baseline helpers and measured-layout storage compile with the stacker feature disabled, so native-stack depth can be tested without hidden stack switching.",
        "Box painting: expose final used parent border/padding read-only so background clips share layout's percentage basis and scale.",
        "Expose final Taffy flex line/item geometry and semantic item order and parent identity, typed block-axis/reverse fragmentation growth constraints and caller-owned monolithic classification (CSS Flexbox 1 section 10; Writing Modes 3 logical-to-physical mappings).",
        "Grid fragmentation: expose each item's final placement lines on either physical axis and pass block space inserted after grid tracks to Taffy (CSS Grid 2 section 12).",
        "Measured hosts: expose the native containing-block size and resolved edges, preserving CSS percentage padding and content-box sizing in probe and final builds.",
        "CSS layout: pass measured baselines and intrinsic image sizes into Taffy so flex/grid sizing and alignment use text/image metrics.",
        "Host painting: generic child visitors borrow box geometry and opaque metadata inside existing clip/effect scopes; an explicit host callback selects fill bounds. CSS order and text-background/fill policy belong to HTML; no CSS collector, flags or TLS. Device snapping is an explicit geometry mechanism.",
        "Text: retain per-run size, spacing, decorations and vertical layout in shaping/cache keys to prevent reuse across incompatible metrics.",
        "Tests: retain bundled upstream fonts for SVG regression coverage, import the moved image-fit type explicitly, and preserve the randomized property-test seed/context lifecycle with the supported proptest attribute.",
        "Wrapping: keep the upstream editor continuation-state contract separate from CSS previous-character tracking across wrap boundaries.",
        "Image fit: retain the visible content box and atlas UV crop for default linear sampling, including cover and positioned images.",
        "Resolved path scenes: share markup rasterization and exact BGRA conversion with typed tiny-skia geometry, avoiding generated SVG text.",
        "Frame counters (BR-31A): time inside the swap chain Present call and frames the platform dropped instead of waiting are counted beside the present time.",
        "Host integration: a unique per-window readback endpoint delivers pixels and copy/map duration; capture sessions, completion subscriptions and first/latest policy belong to the host. Endpoint replacement/drop closes host subscriptions independently of production offscreen scenes. Typed application-owned instrumentation replaces environment switches and global prepaint/batch measurements. Retain animation clock and the identity-transformed profiler overlay.",
        "Window traversal: typed host context scopes replace the need for paint/prepaint TLS and follow canonical paint/deferred snapshots.",
        "Paint contract: checked window/pass/live-composition snapshots share one guarded replay. RasterFrame retains glyph offset, actual/exact fill transforms, rounded/exact bounds and opaque typed host data with Rc identity; typed scopes replace only their own slot without cloning the map. General replay inherits the caller frame; raster/full replay retain it. Geometry and isolated scenes restore on unwind.",
        "Immutable paint commands: capture source paint and host completion once, retain same-draw resources, reopen explicit mechanical ancestor groups, and place ordered commands with a final destination clip through the canonical Scene backend.",
        "Segmented paint commands: admit nodes by source slices and unique segment/ordinal placements, preserve shared ancestor effects and scope-owned clip chains, and project hitboxes from one source prepaint with the same placements. A mechanical child-prepaint observer exposes the active clip scope without repeating traversal.",
        "calc-size: carry sizing keyword bases and sizing-box calculations into native intrinsic layout.",
        "Grid placement: expose completed item row spans from Taffy metadata, preserving implicit lines, source order and dirty-state absence.",
        "Grid geometry: read completed tracks by container ID so empty grids use native layout without a synthetic item.",
        "Transformed hit testing: prepaint scopes record the inverse affine map and parent clip of a transformed subtree; frame hit testing maps the pointer through the scope chain, singular matrices are unhittable. Upstream proposal is recorded privately; no external publication authorized.",
        "Standalone targets: restore the upstream dev-dependencies the crates.io package dropped (published equivalents of the same snapshot) so every example builds; the two network image examples need the rustls/aws-lc-sys reqwest client and require an opt-in feature instead; three examples follow the fork's extended TextRun and calc() lengths and load the bundled upstream Lilex font; copied-layout measurement enters the stack-safe context with the stacker feature; measurement test modules import the measure helper.",
        "Text layout: a retained text measurement is reused only at the same device-snapped line height, so a scale-factor or text-style change measures again.",
        "Paint presentation scope: Window::with_paint_presentation replays immutable records under an explicit matrix: an exact axis-preserving similarity moves vector commands, masks and flat groups exactly; any other non-singular affine matrix has live-paint parity (primitive matrices compose like with_transformation, masks move only when the axes are kept); without the scope caller transforms are not reapplied.",
        "Segment hit scopes: Window::paint_hit_scope and with_prepaint_segment_hit_scopes test single segment placements through the hit scope of the copy that replays them (per-entry scopes in the frame, overriding the hitbox id scope).",
        "Segment source clips: Window::with_paint_source_clips re-applies, inside a nested segment recording over the same source coordinates, the clip chain a containing block adds over a base chain.",
        "Exact content mask: Window::with_content_mask_exact restores a mask read by content_mask (final window coordinates) without intersection or page-stack rescale; continued roots replayed in a sliced fragment copy are cut by their fragmentainer.",
        "LCD text: honor explicit Subpixel independently of system preference only on opaque, pixel-aligned root surfaces; use grayscale for exact glyphs, isolated/recorded scenes, transparent/Mica backdrops and transformed composition.",
        "Retained subtrees: preservation of nested window slots across whole-subtree replay for hosts tracking scene versions and line-cache dependencies; public record/replay of prepaint and paint index ranges (the cached-view reuse) for hosts with their own invalidation, a replayability check (no composition group in the subtree or its deferred draws, root paint environment), the next-frame callback count, and an interaction epoch counting dispatched inputs and focus moves. Upstream proposal is recorded privately; no external publication authorized.",
        "Frame layout storage: when the window's frame tree is full with at least 4096 nodes, grow it once to the frame's element-arena allocation count instead of doubling 2 KiB nodes; smaller trees keep doubling and independent measurement subtrees size to their own nodes.",
        "Shared node styles: equal taffy styles of one layout tree share one allocation through a FIFO of the 8 most recent distinct styles (full equality on hit), cleared with the tree; subtree copies share the source style.",
        "Retained frame tree: the window's Taffy tree and its caches survive frames; a request with the inputs of a previous node (parent by first child, children and style; plain leaf by request position and style; measured leaf by a host layout key and style) claims it and takes the pass's shared equal style, unclaimed nodes are recycled or dropped in one sweep, and kept leaves drop their measure closures between frames. Sharing candidates live as long as the kept tree.",
        "Lazy listener lists: the twenty listener and drag-style lists of Interactivity live in one block allocated by the first registration; a box without listeners keeps one null pointer (has_pinch_listeners folded into the hitbox predicate).",
        "Transparent host wrappers: an ink scope is opened with the host's own object (InkTarget) instead of a sink allocated per scope, the open scopes of a paint context live inline (two before the heap), and an element may decline its dispatch-tree node (Element::dispatch_node, default true) when it sets no key context, focus, handler or deferred draw; thousands of recording wrappers a frame then cost no allocation and no node. Behavior of every other element is unchanged.",
        "GPU memory report: a window forwards an on-request walk of the textures its platform renderer holds (owner path, bytes, bytes in use); platforms that do not report it keep the empty default.",
        "Update the rendering: host steps registered with Window::on_update_rendering run at the start of every draw before its paint pass; an ElementPlace (element id stack and rendering view) lets a pass outside of the draw build elements with the frame's ids and view. with_layout_outside_draw lays out for the following frame (app frame arena cleared by that draw, the window's engine, the frame line segment); with_layout_read lays out between frames with a private arena, a nested engine, sealed line caches and element states set aside for the next frame. A Div without id lays out in any phase; subtree_layout_bounds lists every placed node of a subtree. The frame tree reserves the request sequence of a pass once, at its first request, for as many nodes as the previous pass requested.",
        "Frame damage: the window compares the new scene with the one it drew last, cell by cell (64 device px) over the primitives that can write each cell in raster order, exactly (values without draw order; a group mark equals another only if the groups paint the same, recursively); backdrop filters make the damage unknown. The scene also matches every group with the group of the compared frame that paints the same image, by content and not by index (bucketed by bounds, blur and primitive counts, checked exactly), so inserted, removed and reordered groups keep the others' match.",
        "Isolated views: a view drawn in a box its parent lays out (isolated, optional sizer) is a deferred draw that notifying does not propagate to its ancestors; a frame whose only dirty views lie inside isolated views replays the previous root tree and redraws just those draws in their recorded boxes, keeping window control hitboxes and observed entities.",
        "Scroll-owned deferred draws: a draw deferred with Deferred::in_scroll_node keeps the scroll node open where it was deferred and prepaints and paints under it in the deferred pass (marks, clip, interest margin, nested nodes); a scrolled replay plans the draw's nodes and paint range as parts of the record, nested draws too, and replays them moved in their own place of the deferred pass. Plain defer_draw keeps refusing a scrolled replay.",
        "Layout-decided scrollbars: Style::kamin_scrollbars passes the overflow:auto axes, the viewport, a scroll range proxy and boxes built for a root gutter to Taffy; after layout a div takes the bars layout decided (overflow and its overflow clip offset), and Window::layout_scrollbars reads them.",
        "Batch counters: the `[gpu]` line reports batches per frame for every kind and, for paths, the intermediate area the batches used.",
        "Transform pivots: a painter that draws through a non-identity matrix keeps the matrix inputs (TransformPivot: linear part, translation, origin from the box, quarter-turn snap, the outer pivot, or the painter's own TransformBuild); a scrolled replay builds the moved matrix from the moved inputs by the function a fresh paint runs, bit for bit, and a node whose such primitive has no pivot refuses a moved replay (ScrollRefusal::Transform).",
    ],
    "gpui-windows": [
        "Font-size-adjust: each fallback face that a normalized run selects is pinned before shaping as its own format range at its raw-metric used size, with CSS-run boundaries and native vertical design advances.",
        "Effect lifetime: complete scenes, including kept frames, retain only their needed scratch and immutable mask uploads; embedded captures have independent scratch. Mask uploads are reported as GPU owners and scenes with more than 64 masks reuse their uploads without a count cliff.",
        "Frame canvas: the frame is drawn into a kept canvas texture and each back buffer receives its accumulated damage by copy before present; a scroll step planned as a move (`Scene::frame_shift`) copies the canvas by the step through a scratch texture and rasterizes only the planned rectangles, from instances uploaded once; capture reads the canvas and reports the plan, rectangles and GPU wait.",
        "Analytic gradients: D3D and WARP evaluate radial, repeating and arbitrary-stop sRGB fills directly in the destination, with retained descriptor/stop buffers, shared group uploads and explicit memory accounting; no intermediate raster tile. Analytic quads draw as order-preserving runs with a pixel program of their own, so the plain quad and path programs carry neither the stop loop nor its buffers.",
        "Atlas lifetime: keep only live texture slots with monotonic checked ids; reclaim empty pages without tombstones or stale-scene id reuse.",
        "Debug shaders: embed both HLSL sources and their shared include, compile with rooted virtual filenames and the existing flags, and keep dev/agent binaries independent of the build worktree. The scoped include callback borrows static bytes until synchronous D3DCompile returns; its raw out-pointers are supplied by the compiler. Release keeps its existing embedded bytecode.",
        "Frame demand: wake idle Windows windows asynchronously through the existing GPUI invalidator, coalescing WM_PAINT and retaining paced vsync retry during native draws; weak window ownership prevents stale HWND requests.",
        "Settled subtree skipping: ink sinks collect the placed bounds of every primitive a scope offers (culled included, deferred paint snapshots feed the sinks open at capture) and are tainted by input, element state, groups or callbacks unless a scope declares them inert while unseen; exact layout leaves stand in for inflexible column items whose descendant margins do not adjoin their edges, and are plain leaves of the retained frame tree.",
        "Frame diagnostics: opt-in host logging exposes native draw force and pending force without changing requests.",
        "Deferred frames: consume the pending forced request even when the incoming draw is already forced, preventing a stale extra presentation.",
        "DirectX groups: one frame-domain staging surface preserves GPU interpolation exactly; scissored writes, dirty-rectangle clears, bounded pooled retained surfaces with blur margins, and rectangle-only blend backdrop copies.",
        "Adapter policy: instance-owned typed DXGI preference, unchanged default enumeration, device recovery and live adapter LUID/driver identity.",
        "DirectWrite: CSS font selection, shaping, vertical glyphs and fallback metrics.",
        "Zero em size: a run at font-size 0 is shaped blank (no glyph, no advance, zero extent); a line of such runs is not shaped, a mixed line shapes them at a stand-in size no neighbour uses and drops their glyph runs.",
        "DirectX: CSS primitives, images, filters, masks, gradients and per-window frame readback with copy/map duration delivered to the explicit host endpoint.",
        "Window lifecycle: per-window rendering/resize policy and deterministic WPT scale.",
        "Process shutdown: drain deferred native window destruction for at most 5 s (logged if a window is never released), stop and join owned vsync threads while pumping messages, then release GPU/text devices before leaving the platform loop. A window revokes its OLE drop target in WM_DESTROY, while the HWND is alive, so a window destroyed by the system releases it.",
        "Diagnostics: application-owned GPU batch measurements and typed startup configuration survive device recovery; environment parsing belongs to the host.",
        "Tests: initialize unfragmented backdrop fixtures against the paired GPUI surface fields.",
        "Unsafe invariants: added unsafe blocks are windows-rs COM/Win32 calls on interfaces owned by the live DirectX/DirectWrite state,"
        " called on the thread that owns that state, with HRESULTs and out-parameters checked before use. Raw memory outside embedded debug shader compilation is touched in three places only: the frame-capture"
        " readback copies w*4 bytes per row of a w x h BGRA8 staging texture between Map and Unmap (RowPitch >= w*4), GPU timing reads"
        " a query result through the raw GetData vtable entry into a plain-data out-value whose size is passed with it, and"
        " external-texture registration adopts a non-null ID3D11Texture2D pointer already AddRef'd by the caller (from_raw takes that"
        " reference). Release builds never select the debug-only WARP/D3D11On12 device overrides.",
        "Offscreen windows: a platform option creates message-only windows whose renderer draws into a render-target texture of the"
        " same device, format and shaders instead of a swap chain, with no DirectComposition target, Present, DWM attributes or"
        " resize presentation boost; coalesced posted paint messages replace WM_PAINT for frame demand and vsync pacing, and device"
        " recovery rebuilds the offscreen target. Measurement and pixel-gate hosts read frames back without loading the desktop compositor.",
        "Presentation never blocks the UI thread (BR-31A): Present uses DXGI_PRESENT_DO_NOT_WAIT; a frame dropped by it or by the frame-latency"
        " gate keeps the gate count it took and makes the next paint present the current scene; Present time and dropped frames feed the frame counters.",
        "GPU memory report: the DirectX renderer names every texture it holds (atlas pages with their allocated share, frame and path targets, group and blur buffers) by owner path, on request only; the software rasterizer is an explicit adapter preference that no selector string parses to, so optimized benchmark builds can measure software rendering while application selection is unchanged.",
        "Damage redraw: the target keeps the last frame and every buffer (offscreen texture or flip-model back buffer) carries the damage accumulated since it was drawn; a frame is drawn only within that rectangle under a scissor, groups whose composite cannot reach it are skipped, and an unchanged frame is neither drawn nor presented. A different size, background, opaque backdrop, atlas epoch (removed tiles, replaced or forgotten external textures, device loss), a sampled external texture, a failed pass or GPUI_DAMAGE_REDRAW=0 draw the whole frame. Frame capture keeps one staging texture per size. Group images stay in the group atlas between frames: a group matched with a group of the last frame (Scene::group_base) keeps that group's atlas rectangle and image and is not drawn again; the other groups take free space on the shelves of the last full layout, and the atlas is packed again when they do not fit.",
        "Retained group pixel auditing is an explicit GPUI_GROUP_SCRUB=1 diagnostic; normal frames validate content and resource identity without staging readbacks or audit redraws. Only a busy GPU defers an audit readback; device failures propagate to renderer recovery.",
        "Unknown cursor: when GetCursorPos fails (a desktop that takes no input, a locked session) the pointer is reported outside the client area, not at the screen origin, so it hovers no hitbox.",
        "Path batches: a batch clears and rasterizes its 4x multisampled intermediate only within the whole pixels its paths' clipped bounds reach inside the target and the bound scissor, draws nothing when that area is empty, and its sprites average the samples in integers themselves, so the intermediate has no resolved copy and no whole-window clear or resolve.",
        "Transformed instances in partial redraws: a frame drawn within rectangles draws an instance a matrix moves once, with its own content mask, under each rectangle's scissor; cutting its mask to a rectangle would clip its triangles anew (SV_ClipDistance, clipped as geometry on WARP) and move its edge pixels away from the whole frame's.",
    ],
    "gpui-base": [
        "Editor input: minimap/scroll coordinates, incremental highlighting and run traversal.",
        "Expose editor integration hooks without duplicating Base's input implementation; use explicit application-owned prepaint instrumentation.",
        "Tests: use crate-local markdown and initialize extended GPUI font/run fields so standalone regressions compile.",
        "Standalone targets: the benches use GPUI's criterion 0.8 bench harness; the unused network client dev-dependency (native aws-lc-sys) is not declared.",
        "Inline flows: the retained flow layout is keyed by the scale factor its device-snapped line heights depend on.",
        "Motion: a full-length transition lasts exactly its policy duration (f64 instead of f32 duration arithmetic), so it finishes and stops requesting frames on time.",
    ],
    "gpui-component": [
        "Highlighter: share immutable captures on hits; bound retained capture bytes and entry count (including empty results), evict FIFO without sorting and bypass oversize entries.",
        "Highlighter: resolve styles afresh so mutable/reused theme resolvers cannot return stale colors; invalidate captures after text/tree changes.",
        "Tests: retain exact upstream fixtures so published-crate tests do not depend on the original monorepo layout.",
        "Editor integration: root overlays and WebView hosting; expose a scrollbar policy hook so product geometry belongs to the application.",
        "Tests: modal text-selection tests wait out the wall-clock modal animation as well as stepping the test clock.",
    ],
    "taffy": [
        "Measured float overflow: expose the shared child contribution calculation so independently measured hosts use the same clipping and containment rule as native layout.",
        "Intrinsic queries: expose the existing cached ComputeSize traversal for a requested axis without performing final root layout.",
        "Upstream #1353: retain the used physical relative-position vector separately from Layout.location in block, flex, grid and lanes, including vertical block-flow conversion.",
        "Native stack depth: construct already-boxed per-container flex constants in a separate non-inlined preparation frame before descending into children, and keep the non-inlined logical-axis adapter out of the identity fast path when parent and children already use the physical horizontal LTR coordinate system. Complete output adaptation, out-of-flow placement and cache cloning after recursive container layout returns, in separate native frames. Intrinsic measurements retain margin-collapse metadata so empty deep blocks remain cacheable.",
        "Export final flex lines and item geometry; preserve flex sizing while applying fragmentation consumed space on either physical block axis and in either direction, retaining used automatic minima for scroll/monolithic items (CSS Flexbox 1 section 10; Writing Modes 3 logical-to-physical mappings). Line membership fixed before fragmentation is addressed by the exported line indices in both wrap directions. Negative row line growth suppresses the gap at a break after alignment: later lines move towards block-start and only a content-sized container shrinks (css-gaps-1 section 2).",
        "Grid fragmentation: block space inserted after the tracks of one physical axis keeps unfragmented track sizing and content alignment, moves following tracks, grows spanning areas and a content-sized container, and reaches subgrids inside their span (CSS Grid 2 sections 9 and 12).",
        "Leaf measurement returns baselines so flex/grid baseline alignment uses text metrics.",
        "CSS sizing: intrinsic constraints, aspect ratios, box sizing and percentage resolution.",
        "Flex/grid/block layout: carry CSS margins, baselines and track sizing through layout.",
        "Grid/subgrid/lanes: retain local track sizing, masonry and physical baseline algorithms with their regression fixtures.",
        "Intrinsic limits and out-of-flow layout: preserve containing sizes and CSS min/max constraints across measurement passes.",
        "Upstream #1293: borrow OOF candidate slices while retaining lazy ownership and hoisted-child lifecycle.",
        "Upstream #1322/#1323: export final border-box baselines independently of grouping margins and relative paint offsets; preserve first/last physical X/Y channels and scroll clamping.",
        "Upstream #1242/#1303/#1305/#1304: stretch edge-adjoining margins, float identity/readback, relative painting and complete exclusion bands.",
        "Upstream #1320/#1335: accept length-percentage calc in auto-repeat tracks and count outer flex margins once; retain compressible controls per CSS Sizing 3 instead of #1351.",
        "Upstream #1327/#1328/#1333/#1337/#1336/#1332: avoid temporary allocation and unused stretch probes while preserving final metadata, baseline groups and percentage bases.",
        "Packaging: retain the selected source/examples; omit the upstream generated test module.",
        "calc-size: resolve retained keyword bases separately for intrinsic contributions and used sizes; preserve CSS automatic minima.",
        "Node storage: node_storage_full and reserve_nodes let a host grow the node, children and parent storage once to an expected node count.",
        "Shared node styles: a node holds its Style behind SharedStyle (Rc, as Style is not Send) so equal styles of a tree may share one allocation (new_leaf/new_with_children/set_style accept Style or SharedStyle, shared_style hands it out); subgrid tracks are boxed.",
        "Retained trees: retain_nodes drops unkept nodes in one sweep through a key list it keeps between sweeps (a small per-frame sweep allocates nothing), reset_node reuses a node's storage for a fresh node, children_slice/has_node_context read without allocating, and style_equals/adopt_equal_style compare and swap in an equal shared style without clearing the node's cache.",
        "Lazy node storage: the size-only measure entries of a node's cache are allocated by its first measurement and kept by clear; the final (rounded) layout is allocated only by rounding, a tree without rounding reads the unrounded layout.",
        "Cache modes: an entry's sizing mode and collapsible margins are part of its key, so a node measured in more than one mode in one pass (a grid in a flex item) keeps the entries of each mode instead of dropping all of them on every mode change.",
        "Grid contribution memo: a grid of 64 or more items without subgrids or margin-trim keeps, for one layout pass only, the last min-content, max-content and minimum contribution of each item per axis, keyed bit for bit by its grid area size (and the container inner size for the minimum); it is read only where the item's per-query cache is empty and fills that cache as the measurement would, so repeated size queries of one pass (a flex column asking min- and max-content heights) give identical results without reaching the items (css-grid-1 section 11.5).",
        "Layout-decided scrollbars (CSS Overflow 3 section 3): a box whose overflow:auto bars layout decides is laid out with its guessed bars, and when its scrollable overflow (a scroll range proxy excluded) asks for others, only that box is laid out again in the same pass, at most twice, a bar that appeared staying (Blink AddScrollbarFreeze); the viewport publishes its gutter first, and out-of-flow boxes built for another root gutter resolve their insets against the decided one; a parent records the decided scrollbar size. Descendant gutter changes invalidate intrinsic ancestor measurements and repeat their size selection with transaction-frozen bars; empty grid and lane tracks contribute their in-flow overflow area.",
    ],
    "html5ever": [
        "Admission: opt-in fallible driver and sink operation hooks stop canonical tokenization/tree repair immediately, including EOF recovery; preflight growable token and tree-builder buffers.",
        "Parse processing instructions into DOM nodes instead of bogus comments.",
        "Preserve the tokenizer/tree-builder state and insertion rules for those tokens.",
    ],
    "markup5ever_rcdom": [
        "Revision writes: expose a document-owned text factory and swap admitted sibling storage together with its lease for reversible child-list edits.",
        "Identity: document-scoped POD slot/generation keys are assigned at node creation; weak lookup and allocation-free retirement on actual Drop preserve detached JS-owned nodes without ABA.",
        "Admission: fallible canonical sink mutations retain storage leases, bound payload/work and release partial trees iteratively, including selectedcontent and weak parent owners.",
        "Compatibility: use the shared html5ever/markup5ever/xml5ever 0.40 tree interfaces.",
        "Populate selectedcontent from the selected option during parsing and at finish.",
        "Resolve select options/targets in tree order; static reset respects disabled, multiple and display size instead of selecting an arbitrary BFS result.",
    ],
    "resvg": [
        "feDisplacementMap: demultiply the displacement image and apply scale once; otherwise displacement escapes the filter region (upstream issue linebender/resvg#1019).",
        "feConvolveMatrix: multiply bias by bounded output alpha, as required by Filter Effects 1.",
        "feColorMatrix: round normalized channels to the nearest byte (CSS Color 4 section 5.1) instead of truncating.",
        "Patterns: use nearest sampling for device-aligned 1:1 tiles so their edges do not blur or acquire seams.",
        "feDropShadow: convert shadow colors only for linearRGB; an extra sRGB conversion incorrectly lightens the shadow.",
        "Keep rendering compatible with the paired usvg parser/tree changes; apply usvg first (docs/VENDORING.md).",
    ],
    "usvg": [
        "CSS root compositing: accept a typed computed opacity before resolving explicit inheritance, then leave only the root group to its external paint owner; nested SVG resources retain their own context.",
        "Typed path-tree construction: resolve bounds, ordered luminance masks, clipping and linearRGB blur through the existing renderer without XML serialization.",
        "Resource cycles: traverse the cross-resource graph, use chains and gradient href chains; discard closing edges instead of recursing indefinitely.",
        "CSS drop-shadow(): expand the region for the offset and blur support instead of clipping to the default SVG filter region.",
        "Empty geometry: do not create filtered groups for zero-size/empty shapes or images (SVG 2 sections 9-12).",
        "SVG 2 geometry properties: applicable CSS cx/cy/r/rx/ry/x/y/width/height/d override presentation attributes; pathLength scales strokes.",
        "Zero-sized viewBox disables rendering of svg, symbol, marker and pattern (SVG 2 section 8.6).",
        "Paint inheritance: empty gradient/pattern transforms preserve href inheritance; singular transforms use the fallback paint.",
        "Symbol instances: display:none suppresses rendering; use dimensions override symbol dimensions (SVG 2 section 5.6.2).",
        "Paint keywords are ASCII case-insensitive (CSS Values 4 section 2.1).",
        "External use resources: resolve URLs/fragments through an authoritative injectable loader, preserving standalone bounded local reads and shared cycle/depth/allocation limits.",
        "External use follows the document resource policy and secure static processing, including nested SVG images; imported references retain their source URL bases.",
        "Preserve snapshots of referenced definitions and their resource bases, including imported images.",
    ],
}


def normalized(path):
    data = path.read_bytes()
    if b"\0" in data:
        return data
    try:
        data.decode("utf-8")
    except UnicodeDecodeError:
        return data
    return data.replace(b"\r\n", b"\n")


def upstream_files(directory):
    return {p.relative_to(directory).as_posix(): normalized(p)
            for p in directory.rglob("*") if p.is_file()}


def vendor_files(name):
    prefix = f"vendor/{name}/"
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", prefix],
        cwd=ROOT).decode().split("\0")
    return {p[len(prefix):]: normalized(ROOT / p)
            for p in paths if p and (ROOT / p).is_file()}


def blob_id(data):
    return hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()


def binary_literal(data):
    """Git binary-patch literal: zlib chunks prefixed with decoded byte length."""
    compressed = zlib.compress(data)
    lines = [f"literal {len(data)}\n"]
    for offset in range(0, len(compressed), 52):
        chunk = compressed[offset:offset + 52]
        count = len(chunk)
        prefix = chr(ord("A") + count - 1) if count <= 26 else chr(ord("a") + count - 27)
        lines.append(prefix + base64.b85encode(chunk, pad=True).decode("ascii") + "\n")
    return "".join(lines) + "\n"


def is_binary(data):
    try:
        data.decode("utf-8")
        return b"\0" in data
    except UnicodeDecodeError:
        return True


def has_trailing_whitespace(data):
    # Preserve upstream bytes without introducing whitespace errors into the patch file.
    return any(line.endswith((b" ", b"\t")) for line in data.splitlines())


def render(name, before, after):
    lines = [f"# KaminIDE patches — {PINS[name]}\n", "#\n"]
    lines += [f"# {i}. {reason}\n" for i, reason in enumerate(FIXES[name], 1)]
    lines += ["#\n", "# Generated by vendor/regenerate-patch.py; see docs/VENDORING.md.\n",
              "# Apply from the repository root: git apply --unidiff-zero this.patch\n\n"]
    for rel in sorted(before.keys() | after.keys()):
        old, new = before.get(rel, b""), after.get(rel, b"")
        if old == new and (rel in before) == (rel in after):
            continue
        path = f"vendor/{name}/{rel}"
        lines.append(f"diff --git a/{path} b/{path}\n")
        if rel not in before:
            lines.append("new file mode 100644\n")
        elif rel not in after:
            lines.append("deleted file mode 100644\n")
        old_id = blob_id(old) if rel in before else "0" * 40
        new_id = blob_id(new) if rel in after else "0" * 40
        lines.append(f"index {old_id}..{new_id}\n")
        if is_binary(old) or is_binary(new) or has_trailing_whitespace(old) or has_trailing_whitespace(new):
            lines += ["GIT binary patch\n", binary_literal(new), binary_literal(old)]
            continue
        delta = difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{path}" if rel in before else "/dev/null",
            tofile=f"b/{path}" if rel in after else "/dev/null", n=0)
        for line in delta:
            lines.append(line if line.endswith("\n") else line + "\n\\ No newline at end of file\n")
    result = "".join(lines).encode()
    return result if b"\ndiff --git " in result else result.rstrip(b"\n") + b"\n"


def round_trip(name, before, after, patch, scratch):
    with tempfile.TemporaryDirectory(prefix="vendor-patch-", dir=scratch) as temp:
        root = Path(temp)
        crate = root / "vendor" / name
        crate.mkdir(parents=True)
        for rel, data in before.items():
            target = crate / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        # A clean import has a header-only patch; git apply rejects an empty diff.
        if b"\ndiff --git " in patch:
            subprocess.run(["git", "apply", "--unidiff-zero", "--whitespace=nowarn", "-"],
                           input=patch, cwd=root, check=True)
        actual = upstream_files(crate)
        if actual != after:
            changed = sorted(k for k in actual.keys() | after.keys() if actual.get(k) != after.get(k))
            raise RuntimeError(f"Round-trip mismatch: {changed}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("crate", choices=PINS)
    parser.add_argument("upstream", type=Path, help="clean, unpacked upstream crate directory")
    parser.add_argument("--scratch", type=Path, help="parent for disposable verification directory")
    parser.add_argument("--check", action="store_true", help="verify without overwriting the patch")
    args = parser.parse_args()
    revision = PACKAGE_REVISIONS.get(args.crate)
    if revision:
        info = args.upstream / ".cargo_vcs_info.json"
        actual = json.loads(info.read_text(encoding="utf-8"))["git"]["sha1"]
        if actual != revision:
            raise SystemExit(f"Wrong upstream revision: expected {revision}, got {actual}")
    before, after = upstream_files(args.upstream), vendor_files(args.crate)
    patch = render(args.crate, before, after)
    round_trip(args.crate, before, after, patch, args.scratch)
    dest = ROOT / "vendor" / f"{args.crate}-kaminide.patch"
    if args.check:
        if normalized(dest) != patch:
            raise SystemExit(f"Outdated patch: {dest}")
    else:
        dest.write_bytes(patch)
    print(f"{args.crate}: round-trip OK ({len(after)} files, {len(patch)} patch bytes)")


if __name__ == "__main__":
    main()
