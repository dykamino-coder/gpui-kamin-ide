"""Reconstruct the live allocation peak; never call a later root walk a peak walk."""

import json
import math
import pathlib
import re


def allocation(line):
    if not line.startswith(("a ", "z ", "r ", "f ")):
        return None
    parts = line.split(" ", 5)
    return parts[0], parts[1], parts[2], int(parts[3]), parts[4].strip()


def locate(path):
    """First maximum before the first owner record, in an unfiltered allocation stream."""
    blocks = {}
    live = peak = 0.0
    checkpoint = None
    with open(path, encoding="utf-8") as stream:
        header = stream.readline()
        if not re.match(r"# engine_bench alloc stacks v[23] base=[0-9a-f]+", header):
            raise SystemExit("peak reconstruction needs a v2+ pointer allocation file")
        if not re.search(r"\bfilter=\*(?:\s|$)", header):
            raise SystemExit("peak reconstruction needs filter=* (the whole process)")
        match = re.search(r"\bsample=(\d+)", header)
        sample = int(match.group(1)) if match else 0
        owner_line = None
        gpu_seen = False
        dropped = 0
        for number, line in enumerate(stream, 1):
            if line.startswith("# dropped"):
                dropped += int(line.split()[2])
            if line.startswith("o "):
                if gpu_seen:
                    raise SystemExit("peak export needs one owner walk; run one bench mode at a time")
                if owner_line is None:
                    owner_line = number
            if line.startswith("g "):
                gpu_seen = True
            if owner_line is not None:
                continue
            item = allocation(line)
            if item is None:
                continue
            kind, frame, phase, size, pointer = item
            live -= blocks.pop(pointer, 0.0)
            if kind != "f":
                probability = -math.expm1(-size / sample) if sample and size else 1.0
                weight = size / probability
                blocks[pointer] = weight
                live += weight
            if live > peak:
                peak = live
                checkpoint = {"line": number, "frame": int(frame), "phase": phase,
                              "live_bytes": round(peak), "blocks": len(blocks)}
    if checkpoint is None:
        raise SystemExit("no allocations found before the first owner report")
    return {**checkpoint, "sample_interval": sample, "estimated": bool(sample),
            "owner_line": owner_line, "dropped_records": dropped,
            "metric": "recorded live requested heap bytes (not process working set)",
            "scope": "process start through the first owner walk"}


def end_stream(stream):
    """Freeze the heap at the first owner record; never sum different root walks."""
    owners = gpu = False
    for line in stream:
        if line.startswith("o "):
            owners = True
        if line.startswith("g "):
            gpu = True
        if not owners or line.startswith(("o ", "g ", "#")):
            yield line
        elif gpu and line.startswith(("a ", "z ", "r ", "f ")):
            break


def peak_stream(stream, point):
    """Replay the peak, then label only allocations surviving unchanged to the root walk.

    A reused address is never allowed to inherit an old block's owner. Null-address
    estimates and GPU textures describe the later walk and cannot label this peak.
    The remaining peak allocations stay unreported and retain their allocation stacks.
    """
    survivors = set()
    for number, line in enumerate(stream, 1):
        if number <= point["line"]:
            item = allocation(line)
            if item:
                kind, _, _, _, pointer = item
                if kind == "f":
                    survivors.discard(pointer)
                else:
                    survivors.add(pointer)
            yield line
            continue
        if point["owner_line"] is None:
            break
        if number < point["owner_line"]:
            item = allocation(line)
            if item:
                survivors.discard(item[4])
        elif line.startswith("o "):
            _, _, pointer, _ = line.split()
            if pointer in survivors and int(pointer, 16):
                yield line
        elif line.startswith("g "):
            break


def capture(exe, stacks, profiler, depth=8, top=25, symbols=None):
    point = locate(stacks)

    class PeakRun(profiler.Run):
        def _fold(self, handle, live, frames):
            return super()._fold(peak_stream(handle, point), live, frames)

    class EndRun(profiler.Run):
        def _fold(self, handle, live, frames):
            return super()._fold(end_stream(handle), live, frames)

    peak = PeakRun(exe, stacks, True, symbols=symbols)
    steady = EndRun(exe, stacks, True, symbols=peak.symbols)
    peak.end = f"recorded peak frame={point['frame']} phase={point['phase']}"
    if point["owner_line"] is not None:
        steady.end = "heap frozen at the first owner record"
        steady.counter = None  # A later audit counter is not this checkpoint's counter.
    peak.dropped = steady.dropped = point["dropped_records"]
    result = {"schema": "kamin.memprof.peak.v1", "checkpoint": point,
              "peak": profiler.build_report(peak, None, True, depth, top),
              "end": profiler.build_report(steady, None, True, depth, top),
              "limitations": [
                  "Peak is reconstructed from recorded allocations, not RSS or GPU memory.",
                  "Peak owner paths identify blocks surviving unchanged until the later owner walk; "
                  "they do not prove the same root held a block at the peak.",
                  "Transient and unlabelled peak blocks are reported by allocation site, not guessed holders.",
                  "Sampling estimates the peak; dropped records make it incomplete.",
                  "Owner-walk setup before its first record can contribute to the recorded peak.",
              ]}
    return result, steady


def reports(exe, stacks, profiler, depth=8, top=25, symbols=None):
    return capture(exe, stacks, profiler, depth, top, symbols)[0]


def prepare(args):
    if args.peak:
        if args.frames or args.at or args.start:
            raise SystemExit("--peak needs the whole run: no --frames, --at or --start")
        args.owners = True
    return pathlib.Path(args.out)


def fold_run(args, exe, stacks, frames, profiler, name):
    if not args.peak:
        return profiler.Run(exe, stacks, True, frames)
    result, steady = capture(exe, stacks, profiler, args.depth, args.top)
    path = pathlib.Path(args.out) / f"{name}-peak.json"
    path.write_text(json.dumps(result, indent=2), encoding="utf-8")
    return steady


def add_parser(sub):
    parser = sub.add_parser("peak", help="export live heap at its peak and at the end, from saved stacks")
    parser.add_argument("exe")
    parser.add_argument("stacks")
    parser.add_argument("--out", required=True, help="JSON report path")
    parser.add_argument("--depth", type=int, default=8)
    parser.add_argument("--top", type=int, default=25)


def command(args, profiler):
    result = reports(args.exe, args.stacks, profiler, args.depth, args.top)
    pathlib.Path(args.out).write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps(result["checkpoint"], indent=2))
