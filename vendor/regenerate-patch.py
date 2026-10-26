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
    "gpui": [
        "Container units: publish completed content-box dimensions once per persistent source per frame, settle nested style/layout inputs top-down before paint, and retain persistent element state.",
        "Upstream #66070: expose completed unrounded normal geometry with tree/pass identity, full root constraint space and device scale, independently of relative/paint placement.",
        "Native stack depth: keep Div phase styles on the heap and borrow Style while requesting layout, releasing large construction temporaries before descending into children. Drain atomic paint contexts in a separate frame after child painting returns; finish background, shadow and border temporaries before recursive paint. Isolate accessibility-node construction and completion from recursive prepaint frames. Borrow phase payloads in place while traversing children and move them only after traversal returns. Borrow text refinements in shared child-drawing scopes, finish element-ID temporaries before recursion, and isolate anonymous element-state dispatch from the stateful branch in prepaint and paint.",
        "Optional stack growth: measurement/baseline helpers and measured-layout storage compile with the stacker feature disabled, so native-stack depth can be tested without hidden stack switching.",
        "Box painting: expose final used parent border/padding read-only so background clips share layout's percentage basis and scale.",
        "Expose final Taffy flex line/item geometry and semantic item order and parent identity, typed block-axis/reverse fragmentation growth constraints and caller-owned monolithic classification (CSS Flexbox 1 section 10; Writing Modes 3 logical-to-physical mappings).",
        "Measured hosts: expose the native containing-block size and resolved edges, preserving CSS percentage padding and content-box sizing in probe and final builds.",
        "CSS layout: pass measured baselines and intrinsic image sizes into Taffy so flex/grid sizing and alignment use text/image metrics.",
        "Host painting: generic child visitors borrow box geometry and opaque metadata inside existing clip/effect scopes; an explicit host callback selects fill bounds. CSS order and text-background/fill policy belong to HTML; no CSS collector, flags or TLS. Device snapping is an explicit geometry mechanism.",
        "Text: retain per-run size, spacing, decorations and vertical layout in shaping/cache keys to prevent reuse across incompatible metrics.",
        "Tests: retain bundled upstream fonts for SVG regression coverage, import the moved image-fit type explicitly, and preserve the randomized property-test seed/context lifecycle with the supported proptest attribute.",
        "Wrapping: keep the upstream editor continuation-state contract separate from CSS previous-character tracking across wrap boundaries.",
        "Image fit: retain the visible content box and atlas UV crop for default linear sampling, including cover and positioned images.",
        "Resolved path scenes: share markup rasterization and exact BGRA conversion with typed tiny-skia geometry, avoiding generated SVG text.",
        "Host integration: frame capture with an opt-in first-frame latch, completion events and readback timing for WPT readiness/convergence diagnostics, animation clock, opt-in prepaint profiling and identity-transformed profiler overlay.",
        "Window traversal: typed host context scopes replace the need for paint/prepaint TLS and follow canonical paint/deferred snapshots.",
        "Paint contract: checked window/pass/live-composition snapshots share one guarded replay. RasterFrame retains glyph offset, actual/exact fill transforms, rounded/exact bounds and opaque typed host data with Rc identity; typed scopes replace only their own slot without cloning the map. General replay inherits the caller frame; raster/full replay retain it. Geometry and isolated scenes restore on unwind.",
        "Immutable paint commands: capture source paint and host completion once, retain same-draw resources, reopen explicit mechanical ancestor groups, and place ordered commands with a final destination clip through the canonical Scene backend.",
        "calc-size: carry sizing keyword bases and sizing-box calculations into native intrinsic layout.",
        "Grid placement: expose completed item row spans from Taffy metadata, preserving implicit lines, source order and dirty-state absence.",
        "Grid geometry: read completed tracks by container ID so empty grids use native layout without a synthetic item.",
    ],
    "gpui-windows": [
        "DirectWrite: CSS font selection, shaping, vertical glyphs and fallback metrics.",
        "DirectX: CSS primitives, images, filters, masks, gradients and frame capture with CPU copy/map timing for WPT phase measurements.",
        "Window lifecycle: per-window rendering/resize policy and deterministic WPT scale.",
        "Diagnostics: GPU batch measurements consumed by the application's diagnostic panel.",
        "Tests: initialize unfragmented backdrop fixtures against the paired GPUI surface fields.",
        "Unsafe invariants: added unsafe blocks are windows-rs COM/Win32 calls on interfaces owned by the live DirectX/DirectWrite state,"
        " called on the thread that owns that state, with HRESULTs and out-parameters checked before use. Raw memory is touched in three places only: the frame-capture"
        " readback copies w*4 bytes per row of a w x h BGRA8 staging texture between Map and Unmap (RowPitch >= w*4), GPU timing reads"
        " a query result through the raw GetData vtable entry into a plain-data out-value whose size is passed with it, and"
        " external-texture registration adopts a non-null ID3D11Texture2D pointer already AddRef'd by the caller (from_raw takes that"
        " reference). Release builds never select the debug-only WARP/D3D11On12 device overrides.",
    ],
    "gpui-base": [
        "Editor input: minimap/scroll coordinates, incremental highlighting and run traversal.",
        "Expose editor integration hooks without duplicating Base's input implementation.",
        "Tests: use crate-local markdown and initialize extended GPUI font/run fields so standalone regressions compile.",
    ],
    "gpui-component": [
        "Highlighter: share immutable captures on hits; bound retained capture bytes and entry count (including empty results), evict FIFO without sorting and bypass oversize entries.",
        "Highlighter: resolve styles afresh so mutable/reused theme resolvers cannot return stale colors; invalidate captures after text/tree changes.",
        "Tests: retain exact upstream fixtures so published-crate tests do not depend on the original monorepo layout.",
        "Editor integration: root overlays and WebView hosting; expose a scrollbar policy hook so product geometry belongs to the application.",
    ],
    "taffy": [
        "Upstream #1353: retain the used physical relative-position vector separately from Layout.location in block, flex, grid and lanes, including vertical block-flow conversion.",
        "Native stack depth: keep per-container flex constants on the heap and skip the logical-axis adapter when parent and children already use the physical horizontal LTR coordinate system. Complete output adaptation, out-of-flow placement and cache cloning after recursive container layout returns, in separate native frames. Intrinsic measurements retain margin-collapse metadata so empty deep blocks remain cacheable.",
        "Export final flex lines and item geometry; preserve flex sizing while applying fragmentation consumed space on either physical block axis and in either direction, retaining used automatic minima for scroll/monolithic items (CSS Flexbox 1 section 10; Writing Modes 3 logical-to-physical mappings).",
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
    ],
    "html5ever": [
        "Parse processing instructions into DOM nodes instead of bogus comments.",
        "Preserve the tokenizer/tree-builder state and insertion rules for those tokens.",
    ],
    "markup5ever_rcdom": [
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
    return "".join(lines).encode()


def round_trip(name, before, after, patch, scratch):
    with tempfile.TemporaryDirectory(prefix="vendor-patch-", dir=scratch) as temp:
        root = Path(temp)
        crate = root / "vendor" / name
        crate.mkdir(parents=True)
        for rel, data in before.items():
            target = crate / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
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
