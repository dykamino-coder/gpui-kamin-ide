#!/usr/bin/env python3
"""Локальный корпус engine_bench: сырые фазы и целочисленные границы работы."""

import argparse
import hashlib
import json
import subprocess
import sys
import time
from pathlib import Path

from engine_perf_lib import check_bounds, validate, write_table

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "crates/engine-bench/corpus/review4"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--suite", choices=["guard", "all"], default="guard")
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--gpu", default="default")
    parser.add_argument("--record-only", action="store_true")
    args = parser.parse_args()
    if args.runs < 1 or args.timeout <= 0:
        parser.error("runs and timeout must be positive")
    args.exe = args.exe.resolve(strict=True)
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((CORPUS / "manifest.json").read_text(encoding="utf-8"))
    selected = manifest["guard_pages"] if args.suite == "guard" else None
    reports, executions = {}, []
    for page in manifest["pages"]:
        if selected is not None and page["id"] not in selected:
            continue
        path = CORPUS / page["path"]
        if hashlib.sha256(path.read_bytes()).hexdigest() != page["sha256"]:
            raise ValueError(f"corpus hash differs: {page['id']}")
        prefix = args.out / page["id"].replace("/", "--")
        argv = [str(args.exe), str(path), "--runs", str(args.runs), "--mode", "both",
                "--gpu", args.gpu, "--json"]
        execution = {"page": page["id"], "argv": argv, "sha256": page["sha256"]}
        start = time.monotonic()
        with prefix.with_suffix(".stdout.json").open("wb") as stdout, \
                prefix.with_suffix(".stderr.txt").open("wb") as stderr:
            proc = subprocess.Popen(argv, stdout=stdout, stderr=stderr)
            try:
                execution["exit_code"] = proc.wait(timeout=args.timeout)
            except subprocess.TimeoutExpired:
                # Только созданный этим запуском процесс; никакого taskkill по имени.
                proc.kill()
                proc.wait()
                execution["error"] = "timeout"
        execution["elapsed_seconds"] = time.monotonic() - start
        if execution.get("exit_code") == 0:
            try:
                report = json.loads(prefix.with_suffix(".stdout.json").read_text("utf-8"))
                validate(report, args.runs)
                reports[page["id"]] = report
            except (ValueError, KeyError, TypeError) as error:
                execution["error"] = str(error)
        else:
            execution.setdefault("error", "engine_bench rejected the input/frame")
        executions.append(execution)
        prefix.with_suffix(".execution.json").write_text(json.dumps(execution, indent=2), "utf-8")
        print(page["id"], execution.get("error", "OK"), flush=True)
    if selected is not None and (len(selected) != len(set(selected))
                                 or set(selected) != {e["page"] for e in executions}):
        raise ValueError("guard selection contains unknown/duplicate pages")
    bounds = check_bounds(manifest["bounds"], reports)
    write_table(args.out / "phases.tsv", reports)
    result = {"schema_version": 1, "suite": args.suite, "runs": args.runs,
              "executable_sha256": hashlib.sha256(args.exe.read_bytes()).hexdigest(),
              "executions": executions, "bounds": bounds,
              "gate": "not evaluated" if args.record_only else "evaluated"}
    (args.out / "results.json").write_text(json.dumps(result, indent=2) + "\n", "utf-8")
    for bound in bounds:
        print(bound["status"], bound["id"], flush=True)
    if not args.record_only and (any("error" in e for e in executions)
                                or any(b["status"] not in ("PASS", "XFAIL") for b in bounds)):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
