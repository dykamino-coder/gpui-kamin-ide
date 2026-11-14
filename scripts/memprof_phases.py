"""Memory at the end of every phase: live heap, what the allocator holds, the process, the GPU.

  memprof.py phases --exe CAND.exe [--base-exe BASE.exe] --out DIR -- PAGE.html --mode relayout --runs 5
  memprof.py phases --from CAND.json [BASE.json]          (bench JSON files of earlier runs)

The live-heap reports say which code holds requested bytes. A working set also contains what the
allocator keeps above them: pages freed but not yet purged, arena commit and rounding. This table puts
the layers side by side at the end of each phase of each measured frame, from the bench's own JSON
(`process_resources`): no probe, no stacks, any `allocation-audit` build.

  live        requested bytes of live Rust blocks (audit counter `live_requested_bytes`)
  allocator   bytes mimalloc has committed (ENGINE_BENCH_ALLOCATOR_MEMORY=1, set by this command)
  held        allocator - live: committed by the allocator and not requested by anyone
  commit      private commit of the process (allocator, stacks, graphics driver, everything)
  ws / peak   working set at the phase end, and its sampled peak inside the phase
  gpu         DXGI local memory of the process on the adapter in use (textures, atlases, targets)

Rows are the median over the measured frames of a mode; with a base, each column is followed by its
delta. A column the build does not report (no audit counters, an exe without allocator memory) is `-`.
"""

import json
import os
import pathlib
import statistics
import subprocess
import sys

COLUMNS = ("live", "allocator", "held", "commit", "ws", "peak", "gpu")


def load(path):
    text = pathlib.Path(path).read_text(encoding="utf-8", errors="replace")
    return json.loads(text[text.index("{"):])


def phase_row(phase):
    """The layers of one phase of one frame, bytes (None where the build does not report one)."""
    end = phase.get("end") or {}
    live = (end.get("rust_allocations") or {}).get("live_requested_bytes")
    allocator = (end.get("allocator") or {}).get("committed_bytes")
    gpu = (phase.get("end_gpu") or {}).get("local_bytes")
    return {"live": live, "allocator": allocator,
            "held": allocator - live if live is not None and allocator is not None else None,
            "commit": end.get("private_commit_bytes"), "ws": end.get("rss_bytes"),
            "peak": phase.get("sampled_peak_rss_bytes"), "gpu": gpu}


def table(bench):
    """{mode: [(phase, {column: median bytes or None}, frames)]} in pipeline order."""
    out = {}
    for mode, data in bench.get("modes", {}).items():
        frames = (data.get("process_resources") or {}).get("samples") or []
        rows = []
        for phase in (frames[0].get("phases") or {}) if frames else ():
            seen = [phase_row(frame["phases"][phase]) for frame in frames if (frame.get("phases") or {}).get(phase)]
            if not seen:
                continue
            row = {}
            for column in COLUMNS:
                values = [one[column] for one in seen if one[column] is not None]
                row[column] = round(statistics.median(values)) if values else None
            rows.append((phase, row, len(seen)))
        out[mode] = rows
    return out


def build(cand, base):
    one, two = table(cand), table(base) if base else {}
    modes = {}
    for mode, rows in one.items():
        other = {phase: row for phase, row, _ in two.get(mode, ())}
        modes[mode] = [{"phase": phase, "frames": frames,
                        **{f"cand_{c}_bytes": row[c] for c in COLUMNS},
                        **({f"base_{c}_bytes": other.get(phase, {}).get(c) for c in COLUMNS} if base else {})}
                       for phase, row, frames in rows]
    return {"schema": "kamin.memprof.phases.v1", "modes": modes,
            "process_peak_working_set_bytes": {"cand": cand.get("peak_working_set_bytes"),
                                               **({"base": base.get("peak_working_set_bytes")} if base else {})}}


def cell(value, base, diff):
    if value is None:
        return f"{'-':>9}" + (f"{'':>9}" if diff else "")
    text = f"{value / 1e6:9.2f}"
    if diff:
        text += f"{(value - base) / 1e6:+9.2f}" if base is not None else f"{'':>9}"
    return text


def render(report, out=None):
    out = out or sys.stdout
    diff = any("base_live_bytes" in row for rows in report["modes"].values() for row in rows)
    peaks = report["process_peak_working_set_bytes"]
    print("memory at the end of each phase, MB (median over frames" + ("; each column: cand, delta to base)" if diff else ")"),
          file=out)
    for name, value in peaks.items():
        if value:
            print(f"  {name}: process peak working set {value / 1e6:.2f}", file=out)
    width = 18 if diff else 9
    for mode, rows in report["modes"].items():
        print(f"\n{mode}:", file=out)
        print("  " + "".join(f"{column:>{width}}" for column in COLUMNS) + "  frames  phase", file=out)
        for row in rows:
            cells = "".join(cell(row[f"cand_{c}_bytes"], row.get(f"base_{c}_bytes"), diff) for c in COLUMNS)
            print(f"  {cells}  {row['frames']:>6}  {row['phase']}", file=out)


def add_parser(sub):
    parser = sub.add_parser("phases", help="live heap, allocator commit, process and GPU memory by phase end")
    parser.add_argument("--exe", help="candidate engine_bench.exe (allocation-audit build)")
    parser.add_argument("--base-exe", help="base engine_bench.exe")
    parser.add_argument("--out", help="directory for bench JSON and the report")
    parser.add_argument("--from", dest="files", nargs="+", metavar="JSON", help="CAND.json [BASE.json] of earlier runs")
    parser.add_argument("--launcher", help="command prefix, e.g. a hidden-desktop launcher")
    parser.add_argument("--timeout", type=int, default=1800)
    parser.add_argument("--ignore-hold", action="store_true", help="start even when a KAMIN_BENCH_HOLD file exists")
    parser.add_argument("bench", nargs="*", help="-- engine_bench arguments")


def phases_command(args, launcher, preflight):
    if args.files:
        cand, base = load(args.files[0]), load(args.files[1]) if len(args.files) > 1 else None
        return render(build(cand, base))
    if not args.exe or not args.out:
        raise SystemExit("phases needs --exe and --out, or --from CAND.json [BASE.json]")
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    args.min_free_gb = 0
    preflight(args, out, exact=False)
    bench = [arg for arg in args.bench if arg != "--"]
    if "--json" not in bench:
        bench.append("--json")
    loaded = []
    for name, exe in [("cand", args.exe)] + ([("base", args.base_exe)] if args.base_exe else []):
        env = dict(os.environ, ENGINE_BENCH_ALLOCATOR_MEMORY="1")
        for key in ("ENGINE_BENCH_ALLOC_STACKS", "ENGINE_BENCH_MEMORY_REPORT"):
            env.pop(key, None)
        with open(out / f"{name}.json", "wb") as stdout, open(out / f"{name}.err", "wb") as stderr:
            code = subprocess.run([*launcher, exe, *bench], env=env, stdout=stdout, stderr=stderr,
                                  stdin=subprocess.DEVNULL, timeout=args.timeout).returncode
        if code:
            raise SystemExit(f"{name} run failed, see {out / (name + '.err')}")
        loaded.append(load(out / f"{name}.json"))
    report = build(loaded[0], loaded[1] if len(loaded) > 1 else None)
    (out / "phases.json").write_text(json.dumps(report, indent=1), encoding="utf-8")
    with open(out / "phases.txt", "w", encoding="utf-8") as text:
        render(report, text)
    render(report)
