"""Memory profiler of the engine: live heap by stack and by subsystem, candidate against base.

One command per question (docs/memprof.md):

  memprof.py run --exe CAND.exe [--base-exe BASE.exe] --at build_elements@2 --out DIR -- PAGE.html --mode relayout --runs 1
      runs engine_bench (allocation-audit or heap-profile build, PDB beside the exe) with the allocation
      probe armed from process start to the end of phase build_elements of measured frame 2, then reports.
  memprof.py report CAND.exe CAND.stacks [BASE.exe BASE.stacks] [--json OUT]
      folds probe files: live heap at the end of the window (default) or every allocation (--allocated).
  memprof.py diff A.json B.json
      compares two saved reports without symbols.

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
HASH = re.compile(r"::h[0-9a-f]{16}$")
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


def classify(names):
    """(site, subsystem, owned type, stack from the site) of one allocation stack."""
    kept, owned = strip_allocator(names)
    start = site_index(kept)
    if start is None:
        return "?", "other", owned, []
    return kept[start], subsystem(kept, start), owned, kept[start:]


class Symbols:
    """One image loaded at the base address the process ran it at (dbghelp, PDB beside the exe)."""

    count = 0

    def __init__(self, exe, base):
        import ctypes as c
        import ctypes.wintypes as w

        self.c = c
        self.dbg = c.WinDLL("dbghelp", use_last_error=True)
        self.dbg.SymInitializeW.argtypes = [w.HANDLE, w.LPCWSTR, w.BOOL]
        self.dbg.SymLoadModuleExW.argtypes = [
            w.HANDLE, w.HANDLE, w.LPCWSTR, w.LPCWSTR, c.c_uint64, w.DWORD, c.c_void_p, w.DWORD,
        ]
        self.dbg.SymLoadModuleExW.restype = c.c_uint64
        self.dbg.SymFromAddr.argtypes = [w.HANDLE, c.c_uint64, c.POINTER(c.c_uint64), c.c_void_p]
        Symbols.count += 1
        self.handle = 0x5A0000 + Symbols.count  # any unique value: no live process is attached
        exe = pathlib.Path(exe).resolve()
        if not self.dbg.SymInitializeW(self.handle, str(exe.parent), False):
            raise OSError(f"SymInitialize failed: {c.get_last_error()}")
        if not self.dbg.SymLoadModuleExW(self.handle, None, str(exe), None, base, 0, None, 0):
            raise OSError(f"cannot load symbols of {exe}: {c.get_last_error()}")
        self.names = {}

    def name(self, address):
        if address not in self.names:
            c = self.c
            info = c.create_string_buffer(88 + 2048)  # SYMBOL_INFO followed by the name
            c.c_uint32.from_buffer(info, 0).value = 88
            c.c_uint32.from_buffer(info, 80).value = 2048
            offset = c.c_uint64()
            # A return address belongs to the call before it.
            if self.dbg.SymFromAddr(self.handle, address - 1, c.byref(offset), info):
                length = c.c_uint32.from_buffer(info, 76).value
                text = info.raw[84 : 84 + length].decode("utf-8", "replace")
                self.names[address] = HASH.sub("", text)
            else:
                self.names[address] = "?"
        return self.names[address]


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
        for line in handle:
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
            if frames and not frames[0] <= int(frame) <= frames[1]:
                continue
            pointer = parts[4] if pointer_field else None
            if kind == "f":
                if live:
                    blocks.pop(pointer, None)
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
        return groups

    def meta(self):
        return {
            "exe": self.exe, "stacks": self.path, "version": self.version, "filter": self.filter,
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
    return report


def mb(value):
    return f"{value / 1e6:+.3f}" if value else "0"


def render(report, out=sys.stdout):
    runs = report["runs"]
    diff = "base" in runs
    c = runs["cand"]
    window = f"filter={c['filter']} until={c['until']} end={c['window_end'] or 'end of file'}"
    sampling = f"sampled, mean {c['sample_mean_bytes']} B (estimates)" if c["sample_mean_bytes"] else "exact"
    print(f"memprof {report['mode']} heap, {window}, {sampling}", file=out)
    for name, run in runs.items():
        counter = run.get("audit_live_bytes_at_end")
        check = f", audit counter {counter / 1e6:.3f} MB ({run['bytes'] / counter:.2%} seen)" if counter else ""
        print(f"  {name}: {run['bytes'] / 1e6:.3f} MB in {run['blocks']} blocks{check}"
              f"{', dropped samples ' + str(run['dropped_samples']) if run['dropped_samples'] else ''}"
              f"  ({run['exe']})", file=out)
    if diff:
        b = runs["base"]
        print(f"  delta: {mb(c['bytes'] - b['bytes'])} MB, {c['blocks'] - b['blocks']:+d} blocks", file=out)
        counters = c.get("audit_live_bytes_at_end"), b.get("audit_live_bytes_at_end")
        if all(counters):
            print(f"  audit counter delta: {mb(counters[0] - counters[1])} MB", file=out)
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
            label = row["site"] + (f" [{row['type']}]" if row["type"] else "")
            if section == "stacks":
                label = " < ".join(row["stack"]) + (f" [{row['type']}]" if row["type"] else "")
            print(f"  {mb(size) if diff else f'{size / 1e6:.3f}':>10} {blocks:+9d}  {row['subsystem']:<16} {label}"
                  if diff else f"  {size / 1e6:10.3f} {blocks:9d}  {row['subsystem']:<16} {label}", file=out)


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
    report = build_report(cand, base, live, args.depth, args.top)
    if args.json:
        pathlib.Path(args.json).write_text(json.dumps(report, indent=1), encoding="utf-8")
    render(report)
    return report


def diff_command(args):
    one, two = (json.loads(pathlib.Path(p).read_text(encoding="utf-8")) for p in (args.cand, args.base))
    for report in (one, two):
        if report.get("schema") != SCHEMA:
            raise SystemExit(f"not a {SCHEMA} report")
    out = {"schema": SCHEMA, "mode": one["mode"], "depth": one["depth"],
           "runs": {"cand": one["runs"]["cand"], "base": two["runs"]["cand"]}}
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
    for name, exe in sides:
        env = dict(os.environ, ENGINE_BENCH_ALLOC_STACKS=spec, ENGINE_BENCH_ALLOC_STACKS_UNTIL=phase,
                   ENGINE_BENCH_ALLOC_STACKS_OUT=str(out / f"{name}.stacks"))
        if args.sample:
            env["ENGINE_BENCH_ALLOC_STACKS_SAMPLE"] = str(args.sample)
        else:
            env.pop("ENGINE_BENCH_ALLOC_STACKS_SAMPLE", None)
        prefix = [part.strip('"') for part in shlex.split(args.launcher, posix=False)] if args.launcher else []
        command = [*prefix, exe, *bench]
        with open(out / f"{name}.json", "wb") as stdout, open(out / f"{name}.err", "wb") as stderr:
            code = subprocess.run(command, env=env, stdout=stdout, stderr=stderr,
                                  stdin=subprocess.DEVNULL, timeout=args.timeout).returncode
        print(f"{name}: exit {code}, {out / (name + '.stacks')}", file=sys.stderr)
        if code:
            raise SystemExit(f"{name} run failed, see {out / (name + '.err')}")
    args.stacks = out / "cand.stacks"
    args.base = [args.base_exe, out / "base.stacks"] if args.base_exe else None
    args.allocated = False
    args.frames = None
    args.json = args.json or out / "report.json"
    with open(out / "report.txt", "w", encoding="utf-8") as text:
        report = report_command(args)
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
    run.add_argument("--at", required=True, help="<phase>@<frame>: live heap when this phase of this frame ends")
    run.add_argument("--start", type=int, default=0, help="first recorded frame (0 = process start, default)")
    run.add_argument("--sample", type=int, default=0, help="mean sampling interval in bytes (0 = exact)")
    run.add_argument("--out", required=True, help="directory for stacks, bench JSON and reports")
    run.add_argument("--launcher", help="command prefix, e.g. a hidden-desktop launcher")
    run.add_argument("--timeout", type=int, default=1800)
    run.add_argument("bench", nargs=argparse.REMAINDER, help="-- engine_bench arguments")

    rep = sub.add_parser("report", parents=[common], help="fold probe files")
    rep.add_argument("exe")
    rep.add_argument("stacks")
    rep.add_argument("base", nargs="*", help="BASE_EXE BASE_STACKS")
    rep.add_argument("--allocated", action="store_true", help="every allocation of the window, not the live heap")
    rep.add_argument("--frames", help="A-B: only records of these frames")

    dif = sub.add_parser("diff", parents=[common], help="compare two saved JSON reports")
    dif.add_argument("cand")
    dif.add_argument("base")

    args = parser.parse_args(argv)
    {"run": run_command, "report": report_command, "diff": diff_command}[args.command](args)


if __name__ == "__main__":
    main()
