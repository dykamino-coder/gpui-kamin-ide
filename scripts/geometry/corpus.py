"""Run exact-path geometry comparisons on a private page manifest (Windows)."""

import argparse
import collections
import json
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare import compare, nonnegative


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--input-root", type=Path, required=True)
    parser.add_argument("--coordination-root", type=Path,
                        help="shared host pause-marker root (defaults to input root)")
    parser.add_argument("--private", type=Path, required=True)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--hidden-launcher", type=Path, required=True)
    parser.add_argument("--podman", type=Path, required=True)
    parser.add_argument("--image", default="mcr.microsoft.com/playwright:v1.65.0-noble")
    parser.add_argument("--widths", type=int, nargs="+", default=[360, 768, 1280])
    parser.add_argument("--height", type=int, default=600)
    parser.add_argument("--absolute", type=nonnegative, default=0.5)
    parser.add_argument("--relative", type=nonnegative, default=0)
    parser.add_argument("--stage", choices=("all", "engine", "chromium", "compare"), default="all")
    args = parser.parse_args()
    if sys.platform != "win32":
        parser.error("this launcher uses the shared Windows hidden desktop")
    if any(value < 1 or value > 8192 for value in [*args.widths, args.height]):
        parser.error("viewport dimensions must be in 1..8192")
    root = args.input_root.resolve(strict=True)
    coordination = (args.coordination_root or root).resolve(strict=True)
    private = args.private.resolve(strict=True)
    checkout = Path(__file__).resolve().parents[2]
    if private.is_relative_to(checkout):
        parser.error("diagnostics must be outside the checkout")
    pages = json.loads(args.manifest.read_text(encoding="utf-8"))
    names = set()
    for page in pages:
        name = page["name"]
        if not name or any(c not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-" for c in name):
            parser.error("page names must be safe filenames")
        if name in names:
            parser.error("duplicate page name")
        names.add(name)
        source = (root / page["input"]).resolve(strict=True)
        if not source.is_relative_to(root):
            parser.error("page input escapes input root")
        page["source"] = source

    def run(command, log):
        # The shared host reserves measurement windows through these markers.
        while ((coordination / "BENCH-PAUSE.txt").exists()
               or any(coordination.glob("agents/*/HOLD-PAIRS"))):
            print("waiting for shared measurement window", flush=True)
            time.sleep(10)
        with log.open("w", encoding="utf-8") as output:
            subprocess.run([sys.executable, str(args.hidden_launcher), *map(str, command)],
                           stdout=output, stderr=subprocess.STDOUT, check=True)

    reports = []
    totals = collections.Counter()
    counts = collections.Counter()
    for page in pages:
        for width in args.widths:
            stem = f"{page['name']}-{width}"
            engine = private / f"{stem}-engine.json"
            chromium = private / f"{stem}-chromium.json"
            if args.stage in ("all", "engine"):
                run([args.engine.resolve(), "--geometry", page["source"], engine, width, args.height],
                    private / f"{stem}-engine.log")
            source = page["source"].relative_to(root).as_posix()
            if args.stage in ("all", "chromium"):
                run([args.podman, "run", "--rm", "--name", f"geomdump-{stem}",
                     "-v", f"{checkout}:/src:ro", "-v", f"{root}:/pages:ro",
                     "-v", f"{private}:/private", args.image,
                     "node", "/src/scripts/geometry/chromium.cjs",
                     "--input", f"/pages/{source}", "--output", f"/private/{chromium.name}",
                     "--width", width, "--height", args.height,
                     "--browsers-path", "/private/browsers",
                     "--playwright-module", "/private/playwright/node_modules/playwright-core"],
                    private / f"{stem}-chromium.log")
            if args.stage not in ("all", "compare"):
                print(f"{stem}: {args.stage} snapshot written", flush=True)
                continue
            result = compare(json.loads(engine.read_text(encoding="utf-8")),
                             json.loads(chromium.read_text(encoding="utf-8")),
                             args.absolute, args.relative)
            (private / f"{stem}-comparison.json").write_text(
                json.dumps(result, indent=2, allow_nan=False) + "\n", encoding="utf-8")
            totals.update(result["totals"])
            counts.update(result["mismatched_counts_by_property"])
            sanitized = {key: value for key, value in result.items()
                         if key not in ("schema_version", "elements", "worst_elements")}
            sanitized["worst_elements"] = [
                {"path": element["path"], "tag": element["tag"],
                 "max_abs_delta": element["max_abs_delta"],
                 "differences": [difference if difference["delta"] is not None
                                 else {"property": difference["property"], "mismatch": True}
                                 for difference in element["differences"]]}
                for element in result["worst_elements"]]
            reports.append({"page": page["name"], "width": width, **sanitized})
            print(f"{stem}: {result['totals']['mismatched_elements']} mismatched elements", flush=True)
    if args.stage not in ("all", "compare"):
        return
    summary = {"schema_version": 1, "purpose": "diagnostic alongside the exact-RGB gate",
               "pages": len(pages), "comparisons": len(reports), "widths": args.widths,
               "height": args.height, "device_scale_factor": 1,
               "tolerances": {"absolute": args.absolute, "relative": args.relative},
               "totals": dict(totals), "mismatched_counts_by_property": dict(sorted(counts.items())),
               "per_page": reports}
    (private / "first-report.json").write_text(
        json.dumps(summary, indent=2, allow_nan=False) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
