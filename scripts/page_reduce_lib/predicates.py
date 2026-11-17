"""External engine oracles. A failed/absent measurement is unresolved, never true."""

import json
import os
from pathlib import Path
import re
import sys

from .png import different
from .process import prefix, run


class Oracle:
    def __init__(self, args):
        self.args = args
        self.launcher = prefix(args.launcher) if args.launcher else []
        self.runner = args.runner
        if self.runner == "auto":
            self.runner = "wptrun" if "wptrun" in Path(args.exe).stem.lower() else "page"
        if os.name == "nt" and not self.launcher:
            raise ValueError("a hidden-desktop --launcher is required for engine launches on Windows")
        if args.predicate == "memory" and self.runner == "wptrun":
            raise ValueError("memory uses memprof ws with engine-bench/page arguments, not wptrun pairs")

    def engine(self, page):
        args = self.args
        (page.parent / "target").mkdir(exist_ok=True)
        env = dict(os.environ, KAMIN_FORCE_SCALE="1", WPT_OFFSCREEN="1", WPT_SURFACE="offscreen", WPT_DUMP="all",
                   WPT_PAGE="1", WPT_ANIMATION_TIME_MS="125", WPT_REPORT=str(page.parent / "report.txt"),
                   WPT_GPU_CONTEXT=str(page.parent / "gpu.json"))
        if self.runner == "wptrun":
            pairs = page.parent / "pairs.txt"
            pairs.write_text(f"{page}|{page}\n", encoding="utf-8")
            tail = [str(pairs), str(args.width), str(args.height)]
        else:
            tail = [str(page)]
        tail.extend(arg.replace("{page}", str(page)) for arg in args.exe_arg)
        return run([*self.launcher, args.exe, *tail], page.parent, args.timeout_s, env)

    def __call__(self, page):
        args = self.args
        if args.require_regex:
            source = page.read_text(encoding="utf-8")
            if any(not re.search(pattern, source, re.S) for pattern in args.require_regex):
                return False
        if args.predicate == "memory":
            return self.memory(page)
        result, stdout, stderr = self.engine(page)
        if result is None:
            return None
        if args.predicate == "panic":
            return args.match in stderr.decode("utf-8", errors="replace")
        if result != 0:
            return None
        shots = [path for path in (page.parent / "target" / "wpt-shots").glob("*.png")
                 if "--ref" not in path.name]
        if args.engine_png:
            shots = [page.parent / args.engine_png]
        if len(shots) != 1 or not shots[0].is_file():
            return None
        chrome_dir = page.parent / "chrome"
        chrome_dir.mkdir()
        dest = chrome_dir / "shot.png"
        # Keep Chrome on the same hidden desktop as the engine. On Windows,
        # headless Chrome on the interactive desktop can retain its browser
        # process after capture and stall process rundown during job cleanup.
        command = [*self.launcher, args.chrome, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                   "--disable-extensions", "--disable-background-networking", "--disable-component-update",
                   "--no-first-run", "--no-default-browser-check",
                   f"--user-data-dir={chrome_dir / 'profile'}", "--virtual-time-budget=1000",
                   "--force-device-scale-factor=1", f"--window-size={args.width},{args.height}",
                   f"--screenshot={dest}", page.as_uri()]
        # Headless Chrome can remain alive after writing its capture (for
        # example while remote fonts are pending). A complete IEND marks capture
        # readiness; different() still validates all chunks and CRCs below.
        def captured():
            try:
                return dest.read_bytes().endswith(b"\x00\x00\x00\x00IEND\xaeB`\x82")
            except (FileNotFoundError, PermissionError):
                # Chrome may still hold the newly created PNG exclusively.
                return False
        # Allow normal browser shutdown before terminating the owned job. Killing
        # during screenshot shutdown can delay Windows child-process rundown.
        code, _, _ = run(command, chrome_dir, args.timeout_s, ready=captured, ready_grace=2)
        if code != 0 or not dest.is_file():
            return None
        return different(shots[0], dest, args.rect, args.tol)

    def memory(self, page):
        args = self.args
        out = page.parent / "ws"
        script = Path(__file__).resolve().parents[1] / "memprof.py"
        command = [sys.executable, "-I", str(script), "ws", "--exe", args.exe,
                   "--out", str(out)]
        if args.launcher:
            command.extend(["--launcher", args.launcher])
        command.extend(["--", str(page), *(arg.replace("{page}", str(page)) for arg in args.exe_arg)])
        code, _, _ = run(command, page.parent, args.timeout_s)
        report = out / "ws.json"
        if code != 0 or not report.is_file():
            return None
        cand = json.loads(report.read_text(encoding="utf-8")).get("cand")
        if not cand or "process_peak_working_set" not in cand:
            return None
        return cand["process_peak_working_set"] > args.ws_mb * (1 << 20)
