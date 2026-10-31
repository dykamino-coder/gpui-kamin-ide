"""Параллельные exact workers с одним LUID и проверяемым merged sidecar."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import os
import sys
import subprocess
from pathlib import Path

# Поддерживается и embedded Python, который не добавляет каталог скрипта в sys.path.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from wpt_context import InfrastructureError, SCHEMA, equal, load, require, sha, sidecar
from wpt_runner import atomic_json, boot_session, pairs, resolve, run, verdicts


def sharded(exe, pair_list, report, workers):
    exe, pair_list, report = (Path(p).resolve() for p in (exe, pair_list, report))
    requested = pairs(pair_list)
    require(requested and 1 <= workers <= 8, "empty list or invalid worker count")
    selector = os.environ.get("WPT_GPU")
    if not selector or not selector.startswith("luid:"):
        selector = resolve(exe)
    session = boot_session()
    work = Path(str(report) + ".shards")
    work.mkdir(parents=True, exist_ok=True)
    jobs = []
    for index in range(0, len(requested), 150):
        chunk = work / f"list{index // 150}.txt"
        chunk.write_text("\n".join(requested[index:index + 150]) + "\n", encoding="utf-8")
        jobs.append((chunk, work / f"rep{index // 150}.txt"))
    atomic_json(sidecar(report), {"schema": SCHEMA, "complete": False})

    def worker(job):
        chunk, output = job
        run(exe, chunk, output, selector, session, os.environ.get("GATE_RESUME") == "1")
        return load(output, exe, chunk)

    with ThreadPoolExecutor(max_workers=workers) as pool:
        contexts = list(pool.map(worker, jobs))
    for context in contexts[1:]:
        equal(contexts[0], context)
    rows = {}
    for _, output in jobs:
        rows.update(verdicts(output))
    require(set(rows) == set(requested), "merged report has missing/extra pairs")
    report.write_text("".join(f"{pair}|{rows[pair]}\n" for pair in requested), encoding="utf-8")
    atomic_json(sidecar(report), {"schema": SCHEMA, "complete": True, "kind": "aggregate",
                "exe": {"sha256": sha(exe)}, "list_sha256": sha(pair_list),
                "boot_session": session, "report_sha256": sha(report),
                "children": [str(output) for _, output in jobs]})
    load(report, exe, pair_list)
    green = sum(rows[pair] == "0.00" for pair in requested)
    print(f"pairs={len(requested)} green={green} selector={selector} workers={workers}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exe")
    parser.add_argument("pair_list")
    parser.add_argument("report")
    parser.add_argument("workers", nargs="?", type=int, default=2)
    args = parser.parse_args()
    try:
        sharded(args.exe, args.pair_list, args.report, args.workers)
    except (InfrastructureError, OSError, ValueError, subprocess.TimeoutExpired) as error:
        parser.exit(2, f"INCOMPARABLE_GPU_CONTEXT: {error}\n")
