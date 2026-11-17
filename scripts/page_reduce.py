"""Reduce a downloaded HTML/CSS page while preserving an external failure.

Example (existing offscreen wptrun; no engine build):
  python -I scripts/page_reduce.py --page page.html --css-inline --predicate panic \
    --match "positioned child layout" --exe C:/private/wptrun.exe \
    --launcher 'python C:/private/hidden-launch.py' --out C:/private/reduced

All variants, logs and captures stay below --out. Use private storage for real
pages. RGB means any channel difference in --rect; inspect the final captures
to establish that it still demonstrates the intended defect (fonts also differ).
"""

import argparse
import os
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
from page_reduce_lib.predicates import Oracle
from page_reduce_lib.reduce import Reducer
from page_reduce_lib.resources import prepare


def rectangle(value):
    try:
        parts = tuple(map(int, value.split(",")))
        if len(parts) != 4 or min(parts[:2]) < 0 or min(parts[2:]) <= 0:
            raise ValueError()
        return parts
    except ValueError:
        raise argparse.ArgumentTypeError("expected x,y,width,height with positive dimensions")


def arguments():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--page", type=Path, required=True)
    parser.add_argument("--css-inline", action="store_true")
    parser.add_argument("--predicate", choices=("panic", "rgb", "memory"), required=True)
    parser.add_argument("--exe", required=True, help="existing engine executable; never builds")
    parser.add_argument("--runner", choices=("auto", "page", "wptrun"), default="auto")
    parser.add_argument("--exe-arg", action="append", default=[], help="extra engine arg; {page} expands")
    parser.add_argument("--launcher", default="", help="quoted hidden-desktop launch prefix, as in memprof")
    parser.add_argument("--match", help="required stderr substring for panic")
    parser.add_argument("--rect", type=rectangle, help="RGB rectangle in pixels, at DPR 1")
    parser.add_argument("--tol", type=int, default=0, help="RGB channel tolerance, 0..255")
    parser.add_argument("--ws-mb", type=float, help="process peak WS threshold; delegated to memprof ws")
    parser.add_argument("--chrome", help="Chrome/Edge executable for rgb")
    parser.add_argument("--engine-png", help="relative PNG output path for a non-wptrun engine")
    parser.add_argument("--width", type=int, default=800)
    parser.add_argument("--height", type=int, default=600)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=3)
    parser.add_argument("--timeout-s", type=float, default=60)
    parser.add_argument("--max-seconds", type=float, help="stop reducing after this budget, then recheck")
    parser.add_argument("--require-regex", action="append", default=[],
                        help="optional source feature guard (repeatable); combined with the oracle")
    args = parser.parse_args()
    if min(args.jobs, args.width, args.height, args.timeout_s) <= 0 or not 0 <= args.tol <= 255:
        parser.error("jobs, dimensions and timeout must be positive; tol must be 0..255")
    if args.max_seconds is not None and args.max_seconds <= 0:
        parser.error("max-seconds must be positive")
    import re
    for pattern in args.require_regex:
        try:
            re.compile(pattern)
        except re.error as error:
            parser.error(f"invalid require-regex: {error}")
    if args.predicate == "panic" and not args.match:
        parser.error("panic requires a nonempty --match")
    if args.predicate == "rgb":
        if not args.rect:
            parser.error("rgb requires --rect")
        if not args.chrome:
            candidates = [Path(os.environ.get("PROGRAMFILES", "C:/Program Files")) / "Google/Chrome/Application/chrome.exe",
                          Path("C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe")]
            args.chrome = next((str(path) for path in candidates if path.is_file()), None)
        if not args.chrome:
            parser.error("rgb requires an installed Chrome/Edge or --chrome")
    if args.predicate == "memory" and (args.ws_mb is None or args.ws_mb <= 0):
        parser.error("memory requires positive --ws-mb")
    args.page, args.out = args.page.resolve(), args.out.resolve()
    args.exe = str(Path(args.exe).resolve())
    if not Path(args.exe).is_file():
        parser.error("--exe must name an existing executable")
    return args


def main():
    args = arguments()
    if args.out.exists() and any(args.out.iterdir()):
        raise SystemExit("--out must be empty: do not overwrite earlier evidence")
    original = args.page.read_bytes()
    source = prepare(original.decode("utf-8-sig"), args.page, args.css_inline)
    oracle = Oracle(args)
    reducer = Reducer(oracle, args.out, args.jobs, args.max_seconds)
    status, final = "failed", source
    try:
        final = reducer.run(source)
        status = "budget reached; partial, freshly verified" if time.monotonic() >= reducer.deadline else "fixed point; freshly verified"
    finally:
        reducer.close()
        if (args.out / "min.html").is_file():
            final = (args.out / "min.html").read_text(encoding="utf-8")
        replay = list(sys.argv[1:])
        replay[replay.index("--page") + 1] = str(args.out / "min.html")
        replay[replay.index("--out") + 1] = str(args.out / "verify")
        if "--max-seconds" in replay:
            replay[replay.index("--max-seconds") + 1] = "1"
        else:
            replay += ["--max-seconds", "1"]
        command = [sys.executable, "-I", str(Path(__file__).resolve()), *replay]
        summary = (f"Status: {status}\nOriginal bytes: {len(original)}\nPrepared bytes: {len(source.encode('utf-8'))}\n"
                   f"Final bytes: {len(final.encode('utf-8'))}\nRuns: {reducer.runs}\nCache hits: {reducer.hits}\n"
                   f"Seconds: {time.monotonic() - reducer.started:.3f}\n"
                   f"Reproduce (fresh --out required): {subprocess.list2cmdline(command)}\n")
        (args.out / "summary.txt").write_text(summary, encoding="utf-8")
        print(summary, flush=True)


if __name__ == "__main__":
    main()
