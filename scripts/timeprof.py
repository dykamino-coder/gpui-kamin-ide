"""Time profiler of the engine: where the time of each phase goes, candidate against base.

One command per question (docs/timeprof.md):

  timeprof.py run --exe CAND.exe [--base-exe BASE.exe] --out DIR -- PAGE.html --mode relayout --runs 20
      runs engine_bench (a build of `memprof.py build`: PDB beside the exe) under a stack sampler and
      reports time by phase, by subsystem, by function and by stack.
  timeprof.py report CAND.exe CAND.samples [BASE.exe BASE.samples] [--phase layout] [--frames A-B]
                     [--focus NAME] [--json OUT]
      folds sample files again: one phase, some frames, or the callees of one function (--focus).
  timeprof.py diff A.json B.json
      compares two saved reports without symbols.

The sampler (scripts/timeprof_sampler.py) works from outside the process, without administrator
rights: it suspends a thread, copies its registers and stack, resumes it and walks the copy. The main
thread is sampled at every tick, so its samples add up to wall time (a wait shows as time in the
waiting call); other threads are sampled while they use the CPU. A sample weighs the time the target
ran since the previous tick.

Every sample gets the phase the bench was in (ENGINE_BENCH_PROFILE_STATE), a stack with inlined
functions restored from the PDB, a site (its first frame that is not allocator or std plumbing) and a
subsystem (the first frame from the site up whose module matches memprof's SUBSYSTEMS).
"""

import argparse
import collections
import json
import os
import pathlib
import shlex
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import memprof  # noqa: E402
from memprof_symbols import Symbols  # noqa: E402

SCHEMA = "kamin.timeprof.v1"
NO_PHASE = "outside"
WAITS = ("NtWait", "ZwWait", "NtDelayExecution", "NtUserMsgWaitForMultipleObjects", "NtUserGetMessage",
         "NtRemoveIoCompletion", "NtSignalAndWait", "NtYieldExecution")


class Profile:
    """One sample file folded into groups: {(phase, thread, subsystem, site, stack): [microseconds, samples]}."""

    def __init__(self, exe, samples, frames=None, phase=None, focus=None, symbols=None):
        self.exe, self.path, self.frames, self.only, self.focus = str(exe), str(samples), frames, phase, focus
        self.phases, self.end, self.interval = [], {}, 0
        self.groups = collections.defaultdict(lambda: [0, 0])
        self.inclusive = collections.defaultdict(lambda: [0, 0])  # (phase, function) -> [us, samples]
        self.callees = collections.defaultdict(lambda: [0, 0])  # under --focus: path below the focus frame
        self.phase_us = collections.defaultdict(lambda: [0, 0, 0, set()])  # wall, on cpu, other threads, frames
        self.waiting = collections.defaultdict(int)
        self.frame_us = collections.defaultdict(int)  # (phase, frame) -> main thread wall
        self.leaves = collections.defaultdict(int)
        self.focus_us = 0
        self.symbols = symbols
        self.keys, self.lost = {}, 0
        lines = pathlib.Path(samples).read_text(encoding="utf-8").splitlines()
        modules = [line.split(" ", 3) for line in lines if line.startswith("m ")]
        self.modules = sorted((int(base, 16), int(size, 16), path) for _, base, size, path in modules)
        if not lines or not lines[0].startswith("# engine_bench cpu samples v1"):
            raise SystemExit(f"{samples}: not an engine_bench sample file")
        fields = dict(item.split("=", 1) for item in lines[0].split() if "=" in item)
        self.phases = [name for name in fields.get("phases", "").split(",") if name]
        self.interval = int(fields.get("interval_us", "0"))
        for line in lines:
            if line.startswith("# end"):
                self.end = dict(item.split("=", 1) for item in line.split() if "=" in item)
        self.main = self.end.get("main")
        if self.symbols is None:
            self.symbols = self._load(exe)
        for line in lines:
            if line.startswith("s "):
                self._sample(line)

    def _load(self, exe):
        name = pathlib.Path(exe).name.lower()
        base = next((base for base, _, path in self.modules if pathlib.Path(path).name.lower() == name), None)
        if base is None:
            raise SystemExit(f"{self.path}: {name} is not among the sampled process's modules")
        symbols = Symbols(exe, base)
        self.exe_range = next((base, base + size) for base, size, path in self.modules if base == symbols.base)
        for base, _size, path in self.modules:
            if base != symbols.base:
                symbols.add_module(path, base)
        return symbols

    def _module(self, address):
        for base, size, path in self.modules:
            if base <= address < base + size:
                return pathlib.Path(path).stem.lower()
        return "?"

    def _names(self, raw):
        """Frame names of one stack, leaf first, with inlined functions restored."""
        names = []
        exe = getattr(self, "exe_range", (0, 1 << 64))
        for index, text in enumerate(raw):
            address = int(text, 16)
            # The symbol lookup steps one byte back, as for a return address; the sampled pc is exact.
            chain = self.symbols.frames(address + 1 if index == 0 else address)
            if exe[0] <= address < exe[1]:
                names.extend(chain)
            else:
                module = self._module(address)
                names.extend(f"{module}!{name}" for name in chain)
        return names

    def _sample(self, line):
        _, thread, frame, phase, weight, cpu, *raw = line.split()
        weight, frame, cpu = int(weight), int(frame), int(cpu)
        phase = NO_PHASE if phase == "-" else (self.phases[int(phase)] if int(phase) < len(self.phases) else phase)
        if self.frames and not self.frames[0] <= frame <= self.frames[1]:
            return
        if self.only and phase != self.only:
            return
        main = thread == self.main or self.main is None
        key = self.keys.get((main, tuple(raw)))
        if key is None:
            names = self._names(raw)
            site, owner, _owned, stack = memprof.classify(names)
            leaf = names[0] if names else "?"
            kind = ("wait" if any(mark in leaf for mark in WAITS) else
                    "allocator" if memprof.is_allocator(leaf) or "!mi_" in leaf else
                    leaf.split("!", 1)[0] if "!" in leaf else "engine")
            key = self.keys[(main, tuple(raw))] = (owner, site, tuple(stack), kind, tuple(names))
        owner, site, stack, kind, names = key
        if not main:
            weight = cpu  # another thread counts by the CPU time it used: it is sampled only while it runs
        if self.focus:
            at = max((i for i, name in enumerate(names) if self.focus in name), default=None)
            if at is None:
                return
            # The path from the focus function down to the leaf, root first.
            below = tuple(reversed(names[:at + 1]))
            for depth in range(1, min(len(below), 4) + 1):
                entry = self.callees[below[:depth]]
                entry[0] += weight
                entry[1] += 1
            self.focus_us += weight
        totals = self.phase_us[phase]
        if main:
            totals[0] += weight
            totals[1] += cpu
            totals[3].add(frame)
            self.frame_us[(phase, frame)] += weight
            self.waiting[phase] += weight - cpu
        else:
            totals[2] += weight
        self.leaves[(phase, kind if main else "other threads")] += weight
        group = self.groups[(phase, "main" if main else "other", owner, site, stack)]
        group[0] += weight
        group[1] += 1
        for name in set(names):
            entry = self.inclusive[(phase, name)]
            entry[0] += weight
            entry[1] += 1

    symbol_quality = memprof.Run.symbol_quality

    def meta(self):
        samples = sum(group[1] for group in self.groups.values())
        return {"exe": self.exe, "samples_file": self.path, "interval_us": self.interval,
                "frames": list(self.frames) if self.frames else None, "phase": self.only, "focus": self.focus,
                "samples": samples, "run_us": int(self.end.get("run_us", 0)),
                "stopped_us": int(self.end.get("stopped_us", 0)), "truncated_stacks": int(self.end.get("truncated", 0)),
                "symbols": memprof.Run.symbol_quality(self)[0]}


def bench_phases(path):
    """Median phase wall time per frame the bench measured itself (ms); None without the JSON.

    The bench reports its measured frames only; the samples also cover the cold frame before them
    and the fresh-document checks after, so totals differ and the per-frame medians are compared."""
    try:
        text = pathlib.Path(path).read_text(encoding="utf-8", errors="replace")
        data = json.loads(text[text.index("{"):])
    except (OSError, ValueError):
        return None
    frames = collections.defaultdict(list)
    for mode in data.get("modes", {}).values():
        for sample in mode.get("samples_ms", ()):
            for phase, value in sample.items():
                if isinstance(value, (int, float)) and phase != "first_frame_total":
                    frames[phase].append(value)
    return {phase: statistics.median(values) for phase, values in frames.items()}


def side_tables(profile, depth):
    phases, subsystems, sites, stacks = {}, {}, {}, {}
    for (phase, thread, owner, site, stack), (weight, count) in profile.groups.items():
        if thread != "main":
            owner, site, stack = "other-threads/" + owner, site, stack
        for table, key in ((subsystems, (phase, owner)), (sites, (phase, owner, site)),
                           (stacks, (phase, owner, site, stack[:depth]))):
            entry = table.setdefault(key, [0, 0])
            entry[0] += count
            entry[1] += weight
    for phase, (wall, cpu, other, frames) in profile.phase_us.items():
        per_frame = [profile.frame_us[(phase, frame)] for frame in frames]
        phases[(phase,)] = [len(frames), wall, cpu, other, profile.waiting.get(phase, 0),
                            statistics.median(per_frame) if per_frame else 0]
    return {"phases": phases, "subsystems": subsystems, "sites": sites, "stacks": stacks,
            "functions": {key: [value[1], value[0]] for key, value in profile.inclusive.items()},
            "callees": {(" > ".join(key),): [value[1], value[0]] for key, value in profile.callees.items()},
            "leaves": {key: [0, value] for key, value in profile.leaves.items()}}


FIELDS = {"subsystems": ("phase", "subsystem"), "sites": ("phase", "subsystem", "site"),
          "stacks": ("phase", "subsystem", "site", "stack"), "functions": ("phase", "function"),
          "callees": ("path",), "leaves": ("phase", "where")}


def build_report(cand, base, depth, top, bench=None):
    one = side_tables(cand, depth)
    two = side_tables(base, depth) if base else {name: {} for name in one}
    report = {"schema": SCHEMA, "depth": depth,
              "runs": {"cand": cand.meta(), **({"base": base.meta()} if base else {})}}
    rows = []
    for key in sorted(set(one["phases"]) | set(two["phases"]), key=lambda k: -one["phases"].get(k, [0, 0])[1]):
        c, b = one["phases"].get(key, [0] * 6), two["phases"].get(key, [0] * 6)
        row = {"phase": key[0], "cand_frames": c[0], "cand_us": c[1], "cand_cpu_us": c[2],
               "cand_other_threads_us": c[3], "cand_wait_us": c[4], "cand_frame_median_us": c[5]}
        if base:
            row.update(base_frames=b[0], base_us=b[1], base_cpu_us=b[2], base_other_threads_us=b[3],
                       base_wait_us=b[4], base_frame_median_us=b[5], delta_us=c[1] - b[1])
        for name, measured in (("cand", (bench or {}).get("cand")), ("base", (bench or {}).get("base"))):
            if measured and key[0] in measured:
                row[f"{name}_bench_ms"] = round(measured[key[0]], 3)
        rows.append(row)
    report["phases"] = rows
    for section, fields in FIELDS.items():
        rows = []
        limit = 0 if section in ("subsystems", "leaves") else top
        for key, cb, cs, bb, bs in memprof.table(one[section], two[section], limit):
            row = dict(zip(fields, key))
            if "stack" in row:
                row["stack"] = list(row["stack"])
            row.update(cand_samples=cb, cand_us=cs)
            if base:
                row.update(base_samples=bb, base_us=bs, delta_us=cs - bs)
            rows.append(row)
        report[section] = rows
    if cand.focus:
        report["focus"] = {"name": cand.focus, "cand_us": cand.focus_us, **({"base_us": base.focus_us} if base else {})}
    notes = memprof.symbol_notes(report, [cand] + ([base] if base else []))
    report.update(notes)
    return report


def ms(value, signed=False):
    return f"{value / 1000:+.2f}" if signed else f"{value / 1000:.2f}"


def render(report, out=None):
    out = out or sys.stdout
    runs = report["runs"]
    diff = "base" in runs
    c = runs["cand"]
    scope = [f"phase {c['phase']}" if c.get("phase") else "every phase"]
    if c.get("frames"):
        scope.append(f"frames {c['frames'][0]}-{c['frames'][1]}")
    if c.get("focus"):
        scope.append(f"under {c['focus']}")
    print(f"timeprof, {', '.join(scope)}; main thread wall time by samples, ms", file=out)
    for name, run in runs.items():
        ran = run["run_us"] or 1
        print(f"  {name}: {run['samples']} samples, target ran {ran / 1e6:.2f} s and stood suspended "
              f"{run['stopped_us'] / ran:.1%} more, interval {run['interval_us']} us"
              f"{', truncated stacks ' + str(run['truncated_stacks']) if run['truncated_stacks'] else ''}"
              f"  ({run['exe']})", file=out)
    for warning in report.get("warnings", ()):
        print(f"WARNING {warning}", file=out)
    if report.get("focus"):
        focus = report["focus"]
        tail = f", base {ms(focus['base_us'])}, delta {ms(focus['cand_us'] - focus['base_us'], True)}" if diff else ""
        print(f"\ntime under {focus['name']}: cand {ms(focus['cand_us'])}{tail}", file=out)
        print("callees (inclusive, path from the focus function):", file=out)
        for row in report["callees"]:
            tail = f" {ms(row['base_us']):>9} {ms(row['delta_us'], True):>9}" if diff else ""
            print(f"  {ms(row['cand_us']):>9}{tail}  {row['path']}", file=out)
    print("\nby phase:", file=out)
    print("  (wall..threads: totals over the frames; per frame: median over the frames, beside the bench's own"
          " median of its measured frames)", file=out)
    head = f"  {'wall':>9} {'on cpu':>9} {'waiting':>9} {'threads':>9} {'frames':>6} {'per frame':>9} {'bench':>9}"
    print(head + (f" {'base':>9} {'delta':>9}" if diff else "") + "  phase", file=out)
    for row in report["phases"]:
        bench = f"{row['cand_bench_ms']:.2f}" if "cand_bench_ms" in row else "-"
        tail = f" {ms(row['base_us']):>9} {ms(row['delta_us'], True):>9}" if diff else ""
        print(f"  {ms(row['cand_us']):>9} {ms(row['cand_cpu_us']):>9} {ms(row['cand_wait_us']):>9} "
              f"{ms(row['cand_other_threads_us']):>9} {row['cand_frames']:>6} {ms(row['cand_frame_median_us']):>9} "
              f"{bench:>9}{tail}  {row['phase']}", file=out)
    titles = (("subsystems", "by subsystem (who the time belongs to)"), ("leaves", "by where the thread was"),
              ("sites", "by site (first engine frame from the leaf)"),
              ("functions", "by function, inclusive (time with the function anywhere on the stack)"),
              ("stacks", f"by stack (depth {report['depth']} from the site, leaf first)"))
    for section, title in titles:
        print(f"\n{title}:", file=out)
        for row in report[section]:
            if diff and not row["delta_us"]:
                continue
            label = (row.get("subsystem") or row.get("where") or "")
            if section == "sites":
                label = f"{label:<16} {memprof.marked(report, row['site'])}"
            elif section == "functions":
                label = row["function"]
            elif section == "stacks":
                label = f"{label:<16} " + " < ".join(memprof.marked(report, frame) for frame in row["stack"])
            tail = f" {ms(row['base_us']):>9} {ms(row['delta_us'], True):>9}" if diff else ""
            print(f"  {ms(row['cand_us']):>9}{tail}  {row['phase']:<20} {label}", file=out)


def parse_frames(text):
    return memprof.parse_frames(text)


def fold(args, cand_exe, cand_samples, base=None, bench=None):
    frames = parse_frames(args.frames)
    cand = Profile(cand_exe, cand_samples, frames, args.phase, args.focus)
    other = Profile(base[0], base[1], frames, args.phase, args.focus) if base else None
    report = build_report(cand, other, args.depth, args.top, bench)
    if args.json:
        pathlib.Path(args.json).write_text(json.dumps(report, indent=1), encoding="utf-8")
    render(report)
    return report


def report_command(args):
    if args.base and len(args.base) != 2:
        raise SystemExit("the base needs both BASE_EXE and BASE_SAMPLES")
    bench = {name: bench_phases(pathlib.Path(path).with_suffix(".json"))
             for name, path in (("cand", args.samples), *((("base", args.base[1]),) if args.base else ()))}
    return fold(args, args.exe, args.samples, args.base or None, bench)


def diff_command(args):
    one, two = (json.loads(pathlib.Path(p).read_text(encoding="utf-8")) for p in (args.cand, args.base))
    for report in (one, two):
        if report.get("schema") != SCHEMA:
            raise SystemExit(f"not a {SCHEMA} report")
    out = {"schema": SCHEMA, "depth": one["depth"], "runs": {"cand": one["runs"]["cand"], "base": two["runs"]["cand"]},
           "warnings": [*one.get("warnings", ()), *(w.replace("cand:", "base:", 1) for w in two.get("warnings", ()))],
           "folded_sites": {**two.get("folded_sites", {}), **one.get("folded_sites", {})}}
    base_phases = {row["phase"]: row for row in two["phases"]}
    out["phases"] = []
    for row in one["phases"]:
        other = base_phases.get(row["phase"], {})
        out["phases"].append({**row, "base_us": other.get("cand_us", 0),
                              "delta_us": row["cand_us"] - other.get("cand_us", 0)})
    for section, fields in FIELDS.items():
        def index(report):
            return {tuple(tuple(r[f]) if f == "stack" else r[f] for f in fields): [r["cand_samples"], r["cand_us"]]
                    for r in report.get(section, ())}
        rows = []
        for key, cb, cs, bb, bs in memprof.table(index(one), index(two), 0 if section == "subsystems" else args.top):
            row = dict(zip(fields, key))
            if "stack" in row:
                row["stack"] = list(row["stack"])
            row.update(cand_samples=cb, cand_us=cs, base_samples=bb, base_us=bs, delta_us=cs - bs)
            rows.append(row)
        out[section] = rows
    render(out)


def read_state(path, process, deadline):
    """The bench's `v1 pid=.. state=.. phases=..` line, or None when the exe does not publish one."""
    while time.monotonic() < deadline and process.poll() is None:
        try:
            text = pathlib.Path(path).read_text(encoding="utf-8")
        except OSError:
            text = ""
        if text.endswith("\n"):
            return dict(item.split("=", 1) for item in text.split() if "=" in item)
        time.sleep(0.002)
    return None


def run_side(name, exe, bench, launcher, out, args):
    import timeprof_sampler
    state_file = out / f"{name}.state"
    state_file.unlink(missing_ok=True)
    env = dict(os.environ, ENGINE_BENCH_PROFILE_STATE=str(state_file))
    for key in ("ENGINE_BENCH_ALLOC_STACKS", "ENGINE_BENCH_MEMORY_REPORT"):
        env.pop(key, None)
    with open(out / f"{name}.json", "wb") as stdout, open(out / f"{name}.err", "wb") as stderr:
        process = subprocess.Popen([*launcher, exe, *bench], env=env, stdout=stdout, stderr=stderr,
                                   stdin=subprocess.DEVNULL)
    state = read_state(state_file, process, time.monotonic() + args.start_timeout)
    if state is None:
        process.wait()
        raise SystemExit(f"{name}: {exe} did not publish its profile state (ENGINE_BENCH_PROFILE_STATE); it "
                         f"needs a build with work counters from a commit that has timeprof. See {out / (name + '.err')}")
    sampler = timeprof_sampler.Sampler(int(state["pid"]), args.interval_ms / 1000, int(state["state"], 16),
                                       not args.main_only, args.stack_kb << 10)
    samples = out / f"{name}.samples"
    try:
        with open(samples, "w", encoding="utf-8") as sink:
            sink.write(f"# engine_bench cpu samples v1 interval_us={round(args.interval_ms * 1000)} "
                       f"phases={state.get('phases', '')}\n")
            count, ran = sampler.run(sink)
    finally:
        sampler.close()
    code = process.wait()
    print(f"{name}: exit {code}, {count} samples over {ran / 1e6:.2f} s, {samples}", file=sys.stderr)
    if code:
        raise SystemExit(f"{name} run failed, see {out / (name + '.err')}")
    return samples


def run_command(args):
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    args.min_free_gb = 0
    memprof.preflight(args, out, exact=False)
    bench = list(args.bench)
    if bench and bench[0] == "--":
        bench = bench[1:]
    if "--json" not in bench:
        bench.append("--json")
    launcher = [part.strip('"') for part in shlex.split(args.launcher, posix=False)] if args.launcher else []
    sides = [("cand", args.exe)] + ([("base", args.base_exe)] if args.base_exe else [])
    files = {name: run_side(name, exe, bench, launcher, out, args) for name, exe in sides}
    measured = {name: bench_phases(out / f"{name}.json") for name, _ in sides}
    args.json = args.json or out / "report.json"
    base = (args.base_exe, files["base"]) if args.base_exe else None
    report = fold(args, args.exe, files["cand"], base, measured)
    with open(out / "report.txt", "w", encoding="utf-8") as text:
        render(report, text)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--depth", type=int, default=8, help="stack frames kept from the site (default 8)")
    common.add_argument("--top", type=int, default=25, help="rows per table (default 25)")
    common.add_argument("--json", help="also write the report as JSON")
    scope = argparse.ArgumentParser(add_help=False)
    scope.add_argument("--frames", help="A-B: only samples of these measured frames (numbered from 1)")
    scope.add_argument("--phase", help="only samples of this phase, e.g. layout")
    scope.add_argument("--focus", help="only samples with this function on the stack (substring of its name); "
                                       "adds the time of its callees")

    run = sub.add_parser("run", parents=[common, scope], help="run engine_bench under the sampler, then report")
    run.add_argument("--exe", required=True, help="candidate engine_bench.exe (PDB beside it)")
    run.add_argument("--base-exe", help="base engine_bench.exe")
    run.add_argument("--out", required=True, help="directory for samples, bench JSON and reports")
    run.add_argument("--launcher", help="command prefix, e.g. a hidden-desktop launcher")
    run.add_argument("--interval-ms", type=float, default=1.0, help="sampling interval (default 1 ms)")
    run.add_argument("--main-only", action="store_true", help="sample only the main thread")
    run.add_argument("--stack-kb", type=int, default=256, help="stack bytes copied per sample (default 256 KiB)")
    run.add_argument("--start-timeout", type=int, default=120, help="seconds to wait for the bench to start")
    run.add_argument("--ignore-hold", action="store_true", help="start even when a KAMIN_BENCH_HOLD file exists")
    run.add_argument("bench", nargs=argparse.REMAINDER, help="-- engine_bench arguments")

    rep = sub.add_parser("report", parents=[common, scope], help="fold sample files")
    rep.add_argument("exe")
    rep.add_argument("samples")
    rep.add_argument("base", nargs="*", help="BASE_EXE BASE_SAMPLES")

    dif = sub.add_parser("diff", parents=[common], help="compare two saved JSON reports")
    dif.add_argument("cand")
    dif.add_argument("base")

    args = parser.parse_args(argv)
    {"run": run_command, "report": report_command, "diff": diff_command}[args.command](args)


if __name__ == "__main__":
    main()
