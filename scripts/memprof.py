"""Memory profiler of the engine: live heap by stack and by subsystem, candidate against base.

One command per question (docs/memprof.md):

  memprof.py build --out DIR [--worktree WT] [--features allocation-audit]
      builds engine_bench for profiling: release code, function names in the PDB, no identical-code
      folding by the linker (scripts/memprof_build.py). Reports warn about an exe built another way.
  memprof.py run --exe CAND.exe [--base-exe BASE.exe] --at build_elements@2 --out DIR -- PAGE.html --mode relayout --runs 1
      runs engine_bench (allocation-audit or heap-profile build, PDB beside the exe) with the allocation
      probe armed from process start to the end of phase build_elements of measured frame 2, then reports.
  memprof.py report CAND.exe CAND.stacks [BASE.exe BASE.stacks] [--json OUT]
      folds probe files: live heap at the end of the window (default) or every allocation (--allocated).
  memprof.py run --exe CAND.exe [--base-exe BASE.exe] --owners --out DIR -- PAGE.html --mode relayout --runs 1
      who holds the memory: the bench measures the heap its document and window hold by owner path
      (DOM, style, layout, display list, text, gpui) after the last frame of each mode
      (ENGINE_BENCH_MEMORY_REPORT=1, crates/engine-bench/src/holders.rs); the probe records the whole
      run, and the report lists owners, the live heap at that moment and its unreported blocks by site.
  memprof.py diff A.json B.json
      compares two saved reports without symbols.
  memprof.py ws --exe CAND.exe [--base-exe BASE.exe] --out DIR -- PAGE.html --mode both --runs 5
      working set by memory region (image, mapped, private by reservation kind) at its peak, any build
      (timing-only included): what the live-heap reports cannot see (scripts/memprof_ws.py).

The probe (crates/engine-bench/src/allocations/probe.rs) writes one line per allocation with its stack
and one per free. A sampled file (ENGINE_BENCH_ALLOC_STACKS_SAMPLE=T) holds a block of s bytes with
probability 1-exp(-s/T); the report divides by that probability, so sampled bytes are estimates.

Every live block is assigned a site (its first frame that is not allocator or std-container plumbing)
and a subsystem (the first frame from the site up whose module matches SUBSYSTEMS).
"""

import argparse
import collections
import json
import math
import os
import pathlib
import re
import shlex
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from memprof_symbols import Symbols  # noqa: E402

SCHEMA = "kamin.memprof.v1"

# Frames of the allocator itself. Only a leading run of them is dropped, and a frame counts only when
# its name STARTS with one of these: generic arguments such as `Vec<T, alloc::alloc::Global>` inside
# a caller's name must not hide the caller.
ALLOCATOR = (
    "engine_bench::allocations::",
    "engine_bench::_::__rust",  # the #[global_allocator] shim
    "alloc::alloc::",
    "alloc::raw_vec::",
    "alloc::rc::",
    "alloc::sync::",
    "alloc::boxed::",
    "core::alloc::",
    "std::alloc::",
    "std::sys::alloc::",
    "__rust",
    "__rdl",
    "__rg",
    "_mi_",
    "mi_",
    "RtlCaptureStackBackTrace",
)
# Containers and std plumbing: kept in the stack, skipped when choosing the site and the subsystem.
PLUMBING = (
    "alloc::",
    "core::",
    "std::",
    "hashbrown::",
    "smallvec::",
    "indexmap::",
    "arrayvec::",
    "?",
)
# Ordered: the first prefix that matches a frame (from the site toward the root) names the owner.
SUBSYSTEMS = (
    ("kamin_html::layout::reuse", "retained"),
    ("kamin_html::document::retained", "retained"),
    ("kamin_html::text", "text"),
    ("gpui::text_system", "text"),
    ("cosmic_text", "text"),
    ("swash", "text"),
    ("harfrust", "text"),
    ("rustybuzz", "text"),
    ("ttf_parser", "text"),
    ("skrifa", "text"),
    ("read_fonts", "text"),
    ("dwrote", "text"),
    ("kamin_font_facts", "text"),
    ("unicode_", "text"),
    ("icu_", "text"),
    ("hyphenation", "text"),
    ("kamin_style::dom", "dom"),
    ("html5ever", "dom"),
    ("markup5ever", "dom"),
    ("xml5ever", "dom"),
    ("tendril", "dom"),
    ("xml::", "dom"),
    ("kamin_style::resources", "resources"),
    ("kamin_html::document::style", "style"),
    ("kamin_style", "style"),
    ("cssparser", "style"),
    ("selectors", "style"),
    ("kamin_html::paint", "paint"),
    ("gpui::scene", "display-list"),
    ("gpui::window::Window::paint", "display-list"),
    ("gpui::window::Window::insert_primitive", "display-list"),
    ("kamin_html::svg", "svg-images"),
    ("resvg", "svg-images"),
    ("usvg", "svg-images"),
    ("tiny_skia", "svg-images"),
    ("image::", "svg-images"),
    ("png::", "svg-images"),
    ("zune_", "svg-images"),
    ("kamin_html::layout", "layout"),
    ("taffy", "layout"),
    ("gpui::taffy", "layout"),
    ("kamin_html::render", "elements"),
    ("gpui::elements", "elements"),
    ("gpui::element", "elements"),
    ("gpui::arena", "elements"),
    ("gpui_windows", "gpu"),
    ("gpui_pre_windows", "gpu"),
    ("gpui::platform", "gpu"),
    ("reblessive", "recursion-stacks"),
    ("kamin_html::document", "document"),
    ("kamin_html::host", "bench-host"),
    ("engine_bench", "bench-host"),
    ("gpui", "gpui-other"),
    ("kamin_html", "html-other"),
)
HEADER = re.compile(r"# engine_bench alloc stacks v([123]) base=([0-9a-f]+)(.*)")
OWNED = re.compile(r"^alloc::(?:rc::Rc|sync::Arc|boxed::Box)(?:<[^>]*>)?::new\w*<(.+)>$")


def is_allocator(name):
    return name.startswith(ALLOCATOR)


def strip_allocator(names):
    """Drops the leading run of allocator frames; returns (kept, the owned type if a Box/Rc/Arc made it)."""
    cut = 0
    while cut < len(names) and is_allocator(names[cut]):
        cut += 1
    owned = None
    for name in names[:cut]:
        match = OWNED.match(name)
        if match:
            owned = match.group(1)
    return names[cut:], owned


def site_index(names):
    for i, name in enumerate(names):
        if not name.startswith(PLUMBING):
            return i
    return 0 if names else None


def subsystem(names, start):
    for name in names[start:]:
        for prefix, owner in SUBSYSTEMS:
            if name.startswith(prefix):
                return owner
    return "other"


# Holders the reporters cannot reach (captures of type-erased listeners), inferred from the stack of
# an unreported live block: (frame substring, owner path). Shown apart from measured owners.
HELD_BY = (
    ("kamin_html::text::paragraph::paint::selection::", "gpui/listeners/selection-captures"),
    ("kamin_html::interactive::selection::track", "gpui/listeners/selection-captures"),
    # track_selection inlined: the paragraph clone is attributed to the paint of the element.
    ("kamin_html::text::paragraph::element::impl$0::paint", "gpui/listeners/selection-captures"),
)


def held_by(site, stack):
    for frame in (site, *stack):
        for pattern, holder in HELD_BY:
            if frame.startswith(pattern):
                return holder
    return None


def classify(names):
    """(site, subsystem, owned type, stack from the site) of one allocation stack."""
    kept, owned = strip_allocator(names)
    start = site_index(kept)
    if start is None:
        return "?", "other", owned, []
    return kept[start], subsystem(kept, start), owned, kept[start:]


class Run:
    """One probe file folded into groups: {(subsystem, site, owned, stack): [blocks, bytes]}."""

    def __init__(self, exe, stacks, live=True, frames=None, symbols=None):
        self.path = str(stacks)
        self.exe = str(exe)
        with open(stacks, encoding="utf-8") as handle:
            first = handle.readline()
            header = HEADER.match(first.strip())
            if not header:
                raise SystemExit(f"{stacks}: not an engine_bench allocation stack file")
            self.version = int(header.group(1))
            fields = dict(item.split("=", 1) for item in header.group(3).split() if "=" in item)
            self.filter = fields.get("filter")
            self.until = fields.get("until", "-")
            self.sample = int(fields.get("sample", "0"))
            if live and self.version < 2:
                raise SystemExit(f"{stacks}: live heap needs a v2+ file (pointers and frees)")
            self.symbols = symbols or Symbols(exe, int(header.group(2), 16))
            self.end = None
            self.counter = None
            self.dropped = 0
            self.lines = 0
            self.owners = None
            self.estimates = []
            self.frames = frames
            self.groups = self._fold(handle, live, frames)

    def weight(self, size):
        """(blocks, bytes) one record stands for."""
        if not self.sample:
            return 1.0, float(size)
        probability = -math.expm1(-size / self.sample) if size else 0.0
        if probability <= 0:
            return 0.0, 0.0
        return 1 / probability, size / probability

    def _fold(self, handle, live, frames):
        keys = {}  # raw address text -> group key
        groups = collections.defaultdict(lambda: [0.0, 0.0])
        blocks = {}  # pointer -> (key, size)
        pointer_field = self.version >= 2
        snapshot = None  # live blocks when the owner report started
        measured = {}  # pointer -> (path, bytes) of measured blocks
        paths = collections.defaultdict(int)
        for line in handle:
            if line.startswith("o "):
                _, path, pointer, size = line.split()
                if snapshot is None:
                    snapshot = dict(blocks)
                paths[path] += int(size)
                if int(pointer, 16):
                    measured[pointer] = (path, int(size))
                else:
                    self.estimates.append(int(size))
                continue
            if line.startswith("#"):
                if line.startswith("# end"):
                    self.end = line[2:].strip()
                    counter = re.search(r"\blive=(\d+)", line)
                    self.counter = int(counter.group(1)) if counter else None
                elif line.startswith("# dropped"):
                    self.dropped = int(line.split()[2])
                continue
            self.lines += 1
            parts = line.rstrip("\n").split(" ", 5 if pointer_field else 4)
            kind, frame, _phase, size = parts[0], parts[1], parts[2], int(parts[3])
            pointer = parts[4] if pointer_field else None
            if kind == "f":
                # A free counts in every frame: a block of the chosen frames freed later is not live.
                if live:
                    blocks.pop(pointer, None)
                continue
            if frames and not frames[0] <= int(frame) <= frames[1]:
                if live and kind == "r":
                    blocks.pop(pointer, None)  # reallocated outside the frames: no longer theirs
                continue
            raw = parts[5] if pointer_field and len(parts) > 5 else (parts[4] if not pointer_field and len(parts) > 4 else "")
            key = keys.get(raw)
            if key is None:
                names = [self.symbols.name(int(a, 16)) for a in raw.split()]
                site, owner, owned, stack = classify(names)
                key = keys[raw] = (owner, site, owned or "", tuple(stack))
            if live:
                blocks[pointer] = (key, size)
            else:
                count, total = self.weight(size)
                group = groups[key]
                group[0] += count
                group[1] += total
        for key, size in blocks.values():
            count, total = self.weight(size)
            group = groups[key]
            group[0] += count
            group[1] += total
        if snapshot is not None:
            self.owners = self._owners(snapshot, measured, paths)
        return groups

    def _owners(self, snapshot, owned, paths):
        """Owner paths against the live heap at the start of the owner report.

        A measured block matches a live block by address; `estimated` bytes are measured without an
        address (hash tables) and each claims one otherwise unreported live block of the same size;
        `unmatched` ones name an address that is no live block (a reporter's size or address is
        wrong). Live blocks nobody reported are kept by site."""
        unreported = collections.defaultdict(lambda: [0.0, 0.0])
        live = matched = by_size = 0.0
        mismatched = 0
        # An estimate (hash table, no address) claims one unreported live block of exactly its size.
        estimates = collections.Counter(self.estimates)
        inferred = collections.defaultdict(float)
        for pointer, (key, size) in snapshot.items():
            count, total = self.weight(size)
            live += total
            hit = owned.get(pointer)
            if hit:
                matched += total
                mismatched += hit[1] != size
                continue
            if estimates[size] > 0:
                estimates[size] -= 1
                by_size += total
                continue
            holder = held_by(key[1], key[3])
            if holder:
                inferred[holder] += total
            group = unreported[key]
            group[0] += count
            group[1] += total
        reported = sum(paths.values())
        return {"paths": dict(paths), "reported_bytes": reported, "live_bytes": round(live),
                "matched_bytes": round(matched), "matched_by_size_bytes": round(by_size),
                "inferred": {path: round(size) for path, size in inferred.items()},
                "estimated_bytes": reported - sum(size for _, size in owned.values()),
                "unmatched_bytes": sum(size for pointer, (_, size) in owned.items() if pointer not in snapshot),
                "size_mismatches": mismatched, "unreported": unreported}

    def symbol_quality(self):
        """What the PDB can and cannot name: no function records, or functions folded by the linker."""
        scan = getattr(self.symbols, "scan", None)
        if scan is None:
            return {"functions": None, "folded_addresses": 0, "folded_names": 0}, {}
        _addresses, functions, folded = scan()
        groups = {tuple(group) for group in folded.values()}
        return {"functions": functions, "folded_addresses": len(groups), "folded_names": len(folded)}, folded

    def meta(self):
        return {
            "symbols": self.symbol_quality()[0],
            "exe": self.exe, "stacks": self.path, "version": self.version, "filter": self.filter,
            "frames": list(self.frames) if self.frames else None,
            "until": self.until, "sample_mean_bytes": self.sample, "window_end": self.end,
            "dropped_samples": self.dropped, "records": self.lines,
            "audit_live_bytes_at_end": self.counter,
            "blocks": round(sum(v[0] for v in self.groups.values())),
            "bytes": round(sum(v[1] for v in self.groups.values())),
        }


def rollup(groups, depth):
    """Sums groups by subsystem, by site and by stack cut to `depth` frames from the site."""
    by = {"subsystems": collections.defaultdict(lambda: [0.0, 0.0]),
          "sites": collections.defaultdict(lambda: [0.0, 0.0]),
          "stacks": collections.defaultdict(lambda: [0.0, 0.0])}
    for (owner, site, owned, stack), (count, size) in groups.items():
        for name, key in (("subsystems", (owner,)), ("sites", (owner, site, owned)),
                          ("stacks", (owner, site, owned, stack[:depth]))):
            by[name][key][0] += count
            by[name][key][1] += size
    return by


def table(cand, base, top):
    rows = []
    for key in set(cand) | set(base):
        c, b = cand.get(key, [0, 0]), base.get(key, [0, 0])
        rows.append((key, round(c[0]), round(c[1]), round(b[0]), round(b[1])))
    rows.sort(key=lambda r: (-abs(r[2] - r[4]), -abs(r[1] - r[3]), -r[2], r[0]))
    return rows[:top] if top else rows


def build_report(cand, base, live, depth, top):
    one = rollup(cand.groups, depth)
    two = rollup(base.groups, depth) if base else {k: {} for k in one}
    report = {"schema": SCHEMA, "mode": "live" if live else "allocated", "depth": depth,
              "runs": {"cand": cand.meta(), **({"base": base.meta()} if base else {})}}
    if cand.owners:
        report.update(owner_sections(cand.owners, base.owners if base else None, top, depth))
    names = {"subsystems": ("subsystem",), "sites": ("subsystem", "site", "type"),
             "stacks": ("subsystem", "site", "type", "stack")}
    for section, fields in names.items():
        rows = []
        for key, cb, cs, bb, bs in table(one[section], two[section], 0 if section == "subsystems" else top):
            row = dict(zip(fields, key))
            if "stack" in row:
                row["stack"] = list(row["stack"])
            row.update(cand_blocks=cb, cand_bytes=cs)
            if base:
                row.update(base_blocks=bb, base_bytes=bs, delta_blocks=cb - bb, delta_bytes=cs - bs)
            rows.append(row)
        report[section] = rows
    report.update(symbol_notes(report, [cand] + ([base] if base else [])))
    return report


BUILD_HINT = "rebuild both sides with `memprof.py build` (docs/memprof.md)"


def symbol_notes(report, runs):
    """Warnings about what the symbols cannot name, and the folded sites among the reported rows."""
    warnings, folded = [], {}
    for name, run in zip(("cand", "base"), runs):
        quality, names = run.symbol_quality()
        folded.update(names)
        if quality["functions"] == 0:
            warnings.append(f"{name}: the PDB beside the exe has no function records (no PDB, or a build "
                            f"without debug info): sites and subsystems below are not reliable; {BUILD_HINT}")
        elif quality["folded_names"]:
            warnings.append(f"{name}: the exe was linked with identical-code folding (ICF): "
                            f"{quality['folded_names']} functions share {quality['folded_addresses']} bodies, and a "
                            f"frame in such a body carries the name of any one of them. Rows marked {{ICF n}} "
                            f"may belong to another function of that group; {BUILD_HINT}")
    seen = {}
    for section in ("sites", "stacks", "unreported_sites", "unreported_stacks"):
        for row in report.get(section, ()):
            for frame in (row["site"], *row.get("stack", ())):
                if frame in folded:
                    seen[frame] = folded[frame]
    return {"warnings": warnings, "folded_sites": seen}


def owner_sections(cand, base, top, depth=8):
    """Owner paths, their reconciliation with the live heap and unreported live blocks by site."""
    def summary(owners):
        return {k: v for k, v in owners.items() if k not in ("paths", "unreported")}

    out = {"owners_summary": {"cand": summary(cand), **({"base": summary(base)} if base else {})}}
    paths = {(p,): [0, b] for p, b in cand["paths"].items()}
    bases = {(p,): [0, b] for p, b in base["paths"].items()} if base else {}
    rows = []
    for key, _, cs, _, bs in table(paths, bases, 0):
        row = {"owner": key[0], "cand_bytes": cs}
        if base:
            row.update(base_bytes=bs, delta_bytes=cs - bs)
        rows.append(row)
    rows.sort(key=lambda r: (-abs(r.get("delta_bytes", r["cand_bytes"])), r["owner"]))
    unreported = rollup(cand["unreported"], 1)["sites"]
    other = rollup(base["unreported"], 1)["sites"] if base else {}
    sites = []
    for key, cb, cs, bb, bs in table(unreported, other, top):
        row = dict(zip(("subsystem", "site", "type"), key), cand_blocks=cb, cand_bytes=cs)
        if base:
            row.update(base_blocks=bb, base_bytes=bs, delta_blocks=cb - bb, delta_bytes=cs - bs)
        sites.append(row)
    stacks = []
    one = rollup(cand["unreported"], depth)["stacks"]
    two = rollup(base["unreported"], depth)["stacks"] if base else {}
    for key, cb, cs, bb, bs in table(one, two, top):
        row = dict(zip(("subsystem", "site", "type"), key[:3]), stack=list(key[3]), cand_blocks=cb, cand_bytes=cs)
        if base:
            row.update(base_blocks=bb, base_bytes=bs, delta_blocks=cb - bb, delta_bytes=cs - bs)
        stacks.append(row)
    out.update(owners=rows, unreported_sites=sites, unreported_stacks=stacks)
    return out


def render_owners(report, out):
    diff = "base" in report["owners_summary"]
    print("\nwho holds the heap (owner report after the last frame, MB):", file=out)
    for name, s in report["owners_summary"].items():
        live = s["live_bytes"] or 1
        print(f"  {name}: live {s['live_bytes'] / 1e6:.3f}, reported {s['reported_bytes'] / 1e6:.3f}"
              f" ({s['reported_bytes'] / live:.1%}), matched by address {s['matched_bytes'] / 1e6:.3f},"
              f" estimates matched by size {s.get('matched_by_size_bytes', 0) / 1e6:.3f},"
              f" estimated {s['estimated_bytes'] / 1e6:.3f}, unmatched {s['unmatched_bytes'] / 1e6:.3f}"
              f" (size mismatches {s['size_mismatches']}), unreported live"
              f" {(s['live_bytes'] - s['matched_bytes'] - s.get('matched_by_size_bytes', 0)) / 1e6:.3f}",
              file=out)
    for row in report["owners"]:
        tail = f" {row['base_bytes'] / 1e6:9.3f} {mb(row['delta_bytes']):>10}" if diff else ""
        print(f"  {row['cand_bytes'] / 1e6:10.3f}{tail}  {row['owner']}", file=out)
    for name, s in report["owners_summary"].items():
        for path, size in sorted(s.get("inferred", {}).items(), key=lambda item: -item[1]):
            print(f"  {size / 1e6:10.3f}  {path} ({name}, inferred from stacks, not measured)", file=out)
    print("\nunreported live blocks by site:", file=out)
    for row in report["unreported_sites"]:
        size = row["delta_bytes"] if diff else row["cand_bytes"]
        blocks = row["delta_blocks"] if diff else row["cand_blocks"]
        label = marked(report, row["site"]) + (f" [{row['type']}]" if row["type"] else "")
        text = mb(size) if diff else f"{size / 1e6:.3f}"
        print(f"  {text:>10} {blocks:+9d}  {row['subsystem']:<16} {label}", file=out)


def mb(value):
    return f"{value / 1e6:+.3f}" if value else "0"


def marked(report, frame):
    """The frame name, marked when the linker folded that function with others."""
    group = report.get("folded_sites", {}).get(frame)
    return f"{frame} {{ICF {len(group)}}}" if group else frame


def render(report, out=None):
    out = out or sys.stdout
    runs = report["runs"]
    diff = "base" in runs
    c = runs["cand"]
    window = f"filter={c['filter']} until={c['until']} end={c['window_end'] or 'end of file'}"
    if c.get("frames"):
        first, last = c["frames"]
        window += f", only blocks allocated in frames {first}-{last}"
    sampling = f"sampled, mean {c['sample_mean_bytes']} B (estimates)" if c["sample_mean_bytes"] else "exact"
    print(f"memprof {report['mode']} heap, {window}, {sampling}", file=out)
    for name, run in runs.items():
        counter = run.get("audit_live_bytes_at_end")
        # The audit counter holds every frame: a share is meaningful only without --frames.
        seen = "" if run.get("frames") or not counter else f" ({run['bytes'] / counter:.2%} seen)"
        check = f", audit counter {counter / 1e6:.3f} MB{seen}" if counter else ""
        print(f"  {name}: {run['bytes'] / 1e6:.3f} MB in {run['blocks']} blocks{check}"
              f"{', dropped samples ' + str(run['dropped_samples']) if run['dropped_samples'] else ''}"
              f"  ({run['exe']})", file=out)
    if diff:
        b = runs["base"]
        print(f"  delta: {mb(c['bytes'] - b['bytes'])} MB, {c['blocks'] - b['blocks']:+d} blocks", file=out)
        counters = c.get("audit_live_bytes_at_end"), b.get("audit_live_bytes_at_end")
        if all(counters):
            print(f"  audit counter delta: {mb(counters[0] - counters[1])} MB", file=out)
    for warning in report.get("warnings", ()):
        print(f"WARNING {warning}", file=out)
    if report.get("owners"):
        render_owners(report, out)
    key = "delta_bytes" if diff else "cand_bytes"
    print("\nby subsystem (MB):", file=out)
    print(f"  {'delta' if diff else 'bytes':>10} {'blocks':>9}  {'cand':>9} {'base':>9}  subsystem" if diff
          else f"  {'bytes':>10} {'blocks':>9}  subsystem", file=out)
    for row in report["subsystems"]:
        if diff:
            print(f"  {mb(row['delta_bytes']):>10} {row['delta_blocks']:+9d}  {row['cand_bytes'] / 1e6:9.3f} "
                  f"{row['base_bytes'] / 1e6:9.3f}  {row['subsystem']}", file=out)
        else:
            print(f"  {row['cand_bytes'] / 1e6:10.3f} {row['cand_blocks']:9d}  {row['subsystem']}", file=out)
    for section, title in (("sites", "by site"), ("stacks", f"by stack (depth {report['depth']} from the site)")):
        print(f"\n{title}:", file=out)
        for row in report[section]:
            if diff and not row["delta_bytes"] and not row["delta_blocks"]:
                continue
            blocks = row["delta_blocks"] if diff else row["cand_blocks"]
            size = row[key]
            label = marked(report, row["site"]) + (f" [{row['type']}]" if row["type"] else "")
            if section == "stacks":
                label = " < ".join(marked(report, frame) for frame in row["stack"])
                label += f" [{row['type']}]" if row["type"] else ""
            print(f"  {mb(size) if diff else f'{size / 1e6:.3f}':>10} {blocks:+9d}  {row['subsystem']:<16} {label}"
                  if diff else f"  {size / 1e6:10.3f} {blocks:9d}  {row['subsystem']:<16} {label}", file=out)
    folded = report.get("folded_sites")
    if folded:
        print("\nfolded functions ({ICF n}: one body, n names; the frame may be any of them):", file=out)
        for frame, group in sorted(folded.items()):
            others = [name for name in group if name != frame]
            more = f" and {len(others) - 6} more" if len(others) > 6 else ""
            print(f"  {frame}\n    = " + "\n    = ".join(others[:6]) + more, file=out)


def parse_frames(text):
    if not text:
        return None
    first, _, last = text.partition("-")
    return int(first), int(last or first)


def report_command(args):
    live = not args.allocated
    frames = parse_frames(args.frames)
    cand = Run(args.exe, args.stacks, live, frames)
    base = None
    if args.base:
        if len(args.base) != 2:
            raise SystemExit("the base needs both BASE_EXE and BASE_STACKS")
        base = Run(args.base[0], args.base[1], live, frames)
    report = write_report(cand, base, live, args)
    if args.delete_stacks:
        for path in [args.stacks] + ([args.base[1]] if args.base else []):
            drop_stacks(path)
    return report


def write_report(cand, base, live, args):
    report = build_report(cand, base, live, args.depth, args.top)
    if args.json:
        pathlib.Path(args.json).write_text(json.dumps(report, indent=1), encoding="utf-8")
    render(report)
    return report


def drop_stacks(path):
    """Removes a folded probe file: an exact one is gigabytes, and the report no longer needs it."""
    size = os.path.getsize(path)
    os.remove(path)
    print(f"removed {path} ({size / 1e9:.2f} GB; --keep-stacks keeps it)", file=sys.stderr)


def held(patterns):
    """Hold files that exist now: benches must not start while another measurement owns the machine."""
    import glob
    return sorted(path for pattern in patterns.split(os.pathsep) if pattern for path in glob.glob(pattern))


def preflight(args, out, exact):
    """Stops a run that would disturb a measurement in progress or fill the disk."""
    holds = held(os.environ.get("KAMIN_BENCH_HOLD", ""))
    if holds and not args.ignore_hold:
        raise SystemExit(f"hold: {', '.join(holds)} (KAMIN_BENCH_HOLD); not starting engine_bench. "
                         "Run again when it is gone, or pass --ignore-hold if the hold is yours")
    import shutil
    free = shutil.disk_usage(out).free / (1 << 30)
    if exact and free < args.min_free_gb:
        raise SystemExit(f"{free:.0f} GiB free at {out}: an exact probe file is several GB per side; "
                         f"free space, use --sample 16384 or lower --min-free-gb (now {args.min_free_gb})")


def diff_command(args):
    one, two = (json.loads(pathlib.Path(p).read_text(encoding="utf-8")) for p in (args.cand, args.base))
    for report in (one, two):
        if report.get("schema") != SCHEMA:
            raise SystemExit(f"not a {SCHEMA} report")
    out = {"schema": SCHEMA, "mode": one["mode"], "depth": one["depth"],
           "runs": {"cand": one["runs"]["cand"], "base": two["runs"]["cand"]},
           "warnings": [*one.get("warnings", ()), *(w.replace("cand:", "base:", 1) for w in two.get("warnings", ()))],
           "folded_sites": {**two.get("folded_sites", {}), **one.get("folded_sites", {})}}
    for section, fields in (("subsystems", ("subsystem",)), ("sites", ("subsystem", "site", "type")),
                            ("stacks", ("subsystem", "site", "type", "stack"))):
        def index(report):
            return {tuple(tuple(r[f]) if f == "stack" else r[f] for f in fields): [r["cand_blocks"], r["cand_bytes"]]
                    for r in report[section]}
        rows = []
        for key, cb, cs, bb, bs in table(index(one), index(two), 0 if section == "subsystems" else args.top):
            row = dict(zip(fields, key))
            if "stack" in row:
                row["stack"] = list(row["stack"])
            row.update(cand_blocks=cb, cand_bytes=cs, base_blocks=bb, base_bytes=bs,
                       delta_blocks=cb - bb, delta_bytes=cs - bs)
            rows.append(row)
        out[section] = rows
    render(out)


def run_command(args):
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    if args.owners:
        # The owner report follows the last frame of a mode: the probe records the whole run.
        phase, spec = None, "*"
    else:
        if not args.at:
            raise SystemExit("run needs --at <phase>@<frame> or --owners")
        phase, _, frame = args.at.partition("@")
        if not frame:
            raise SystemExit("--at needs <phase>@<frame>, e.g. build_elements@2")
        spec = f"*@{args.start}-{frame}"
    sides = [("cand", args.exe)] + ([("base", args.base_exe)] if args.base_exe else [])
    bench = list(args.bench)
    if bench and bench[0] == "--":
        bench = bench[1:]
    if "--json" not in bench:
        bench.append("--json")
    preflight(args, out, exact=not args.sample)
    frames = parse_frames(args.frames)
    runs = []
    for name, exe in sides:
        stacks = out / f"{name}.stacks"
        env = dict(os.environ, ENGINE_BENCH_ALLOC_STACKS=spec, ENGINE_BENCH_ALLOC_STACKS_OUT=str(stacks))
        for key in ("ENGINE_BENCH_ALLOC_STACKS_UNTIL", "ENGINE_BENCH_MEMORY_REPORT"):
            env.pop(key, None)
        if phase:
            env["ENGINE_BENCH_ALLOC_STACKS_UNTIL"] = phase
        if args.owners:
            env["ENGINE_BENCH_MEMORY_REPORT"] = "1"
        if args.sample:
            env["ENGINE_BENCH_ALLOC_STACKS_SAMPLE"] = str(args.sample)
        else:
            env.pop("ENGINE_BENCH_ALLOC_STACKS_SAMPLE", None)
        prefix = [part.strip('"') for part in shlex.split(args.launcher, posix=False)] if args.launcher else []
        command = [*prefix, exe, *bench]
        with open(out / f"{name}.json", "wb") as stdout, open(out / f"{name}.err", "wb") as stderr:
            code = subprocess.run(command, env=env, stdout=stdout, stderr=stderr,
                                  stdin=subprocess.DEVNULL, timeout=args.timeout).returncode
        print(f"{name}: exit {code}, {stacks}", file=sys.stderr)
        if code:
            if not args.keep_stacks and stacks.exists():
                drop_stacks(stacks)
            raise SystemExit(f"{name} run failed, see {out / (name + '.err')}")
        # Each side is folded and its probe file removed before the next side runs: one raw file
        # on disk at a time.
        runs.append(Run(exe, stacks, True, frames))
        if not args.keep_stacks:
            drop_stacks(stacks)
    args.json = args.json or out / "report.json"
    report = write_report(runs[0], runs[1] if len(runs) > 1 else None, True, args)
    with open(out / "report.txt", "w", encoding="utf-8") as text:
        render(report, text)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--depth", type=int, default=8, help="stack frames kept from the site (default 8)")
    common.add_argument("--top", type=int, default=25, help="rows per site/stack table (default 25)")
    common.add_argument("--json", help="also write the report as JSON")

    run = sub.add_parser("run", parents=[common], help="run engine_bench with the probe, then report")
    run.add_argument("--exe", required=True, help="candidate engine_bench.exe (PDB beside it)")
    run.add_argument("--base-exe", help="base engine_bench.exe")
    run.add_argument("--at", help="<phase>@<frame>: live heap when this phase of this frame ends")
    run.add_argument("--owners", action="store_true",
                     help="who holds the heap: owner report after the last frame against the live heap")
    run.add_argument("--start", type=int, default=0, help="first recorded frame (0 = process start, default)")
    run.add_argument("--sample", type=int, default=0, help="mean sampling interval in bytes (0 = exact)")
    run.add_argument("--out", required=True, help="directory for stacks, bench JSON and reports")
    run.add_argument("--launcher", help="command prefix, e.g. a hidden-desktop launcher")
    run.add_argument("--timeout", type=int, default=1800)
    run.add_argument("--frames", help="A-B: report only blocks allocated in these frames")
    run.add_argument("--keep-stacks", action="store_true",
                     help="keep the raw probe files (GBs in exact mode); by default each is removed once folded")
    run.add_argument("--min-free-gb", type=float, default=60,
                     help="an exact run does not start with less free disk space than this (default 60)")
    run.add_argument("--ignore-hold", action="store_true", help="start even when a KAMIN_BENCH_HOLD file exists")
    run.add_argument("bench", nargs=argparse.REMAINDER, help="-- engine_bench arguments")

    rep = sub.add_parser("report", parents=[common], help="fold probe files")
    rep.add_argument("exe")
    rep.add_argument("stacks")
    rep.add_argument("base", nargs="*", help="BASE_EXE BASE_STACKS")
    rep.add_argument("--allocated", action="store_true", help="every allocation of the window, not the live heap")
    rep.add_argument("--frames", help="A-B: only blocks allocated in these frames (frees count in every frame)")
    rep.add_argument("--delete-stacks", action="store_true", help="remove the probe files once folded")

    dif = sub.add_parser("diff", parents=[common], help="compare two saved JSON reports")
    dif.add_argument("cand")
    dif.add_argument("base")

    wsp = sub.add_parser("ws", help="working set by memory region at its peak (any engine_bench build)")
    wsp.add_argument("--exe", required=True, help="candidate engine_bench.exe")
    wsp.add_argument("--base-exe", help="base engine_bench.exe")
    wsp.add_argument("--out", required=True, help="directory for bench JSON and the report")
    wsp.add_argument("--launcher", help="command prefix that prints 'own child: <pid>' first")
    wsp.add_argument("--interval-ms", type=int, default=1, help="working-set sampling interval")
    wsp.add_argument("--step-mb", type=int, default=2, help="new breakdown when the peak grows by this much")
    wsp.add_argument("--ignore-hold", action="store_true", help="start even when a KAMIN_BENCH_HOLD file exists")
    wsp.add_argument("bench", nargs=argparse.REMAINDER, help="-- engine_bench arguments")

    import memprof_build
    memprof_build.add_parser(sub)

    args = parser.parse_args(argv)
    if args.command == "build":
        return memprof_build.build_command(args)
    if args.command == "ws":
        import memprof_ws
        os.makedirs(args.out, exist_ok=True)
        args.min_free_gb = 0
        preflight(args, args.out, exact=False)
        launcher = [p.strip('"') for p in shlex.split(args.launcher, posix=False)] if args.launcher else []
        return memprof_ws.ws_command(args, launcher)
    {"run": run_command, "report": report_command, "diff": diff_command}[args.command](args)


if __name__ == "__main__":
    main()
