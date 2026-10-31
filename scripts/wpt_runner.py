"""Строгий Windows exact-runner: один ByLuid на gate, baseline обязателен."""

import argparse
from contextlib import contextmanager
import ctypes
import json
import os
import sys
from pathlib import Path
import subprocess
import time

# Поддерживается и embedded Python, который не добавляет каталог скрипта в sys.path.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from wpt_context import InfrastructureError, compare, load, require, sha, sidecar

ROOT = Path(__file__).resolve().parent.parent
LOCKS = Path(os.environ.get("WPT_SLOT_ROOT", r"C:\Users\MSI\Projects\gpui-kamin-ide\target\locks"))


def boot_session():
    # SYSTEM_TIMEOFDAY_INFORMATION.BootTime: точный boot marker, без округления uptime.
    data = ctypes.create_string_buffer(48)
    status = ctypes.windll.ntdll.NtQuerySystemInformation(3, data, len(data), None)
    require(status == 0, f"cannot query boot session: NTSTATUS {status}")
    return f"{os.environ.get('COMPUTERNAME')}:{int.from_bytes(data.raw[:8], 'little')}"


def resolve(exe):
    policy = os.environ.get("WPT_GPU", "minimum-power")
    result = subprocess.run([str(exe), "--gpu-catalog", f"--gpu={policy}"],
                            capture_output=True, text=True, encoding="utf-8", timeout=30)
    require(result.returncode == 0, f"GPU resolution failed: {result.stderr}")
    rows = json.loads(result.stdout)["adapters"]
    require(bool(rows), "no matching hardware GPU")
    return rows[0]["selector"]


@contextmanager
def slot():
    LOCKS.mkdir(parents=True, exist_ok=True)
    while True:
        owned = None
        for index in range(1, 9):
            path = LOCKS / f"slice.off{index}"
            try:
                path.mkdir()
                owned = path
                break
            except FileExistsError:
                continue
        if owned:
            break
        time.sleep(5)
    marker = owned / "pid"
    marker.write_text(str(os.getpid()), encoding="ascii")
    try:
        yield
    finally:
        marker.unlink()
        owned.rmdir()


def atomic_json(path, value):
    temporary = path.with_suffix(path.suffix + f".{os.getpid()}.tmp")
    temporary.write_text(json.dumps(value, indent=2), encoding="utf-8")
    temporary.replace(path)


def run(exe, pair_list, report, selector, session, resume=False):
    exe, pair_list, report = (Path(p).resolve() for p in (exe, pair_list, report))
    if resume and report.exists():
        try:
            context = load(report, exe, pair_list)
            require(context["boot_session"] == session, "resume from another boot")
            high, low = selector[5:].split(":")
            require(context["device"]["luid_high_part"] & 0xFFFFFFFF == int(high, 16) and
                    context["device"]["luid_low_part"] == int(low, 16), "resume GPU differs")
            require(set(pairs(pair_list)) == set(verdicts(report)), "resume omitted/added pairs")
            return
        except InfrastructureError:
            # Невалидный resume не читается как готовый: новый child переснимает весь список.
            pass
    report.parent.mkdir(parents=True, exist_ok=True)
    work = report.parent / (report.name + ".workers")
    work.mkdir(exist_ok=True)
    vendor = work / "vendor"
    if not vendor.exists():
        source = Path(os.environ.get("WPT_VENDOR_ROOT", r"C:\Users\MSI\Projects\gpui-kamin-ide\vendor"))
        result = subprocess.run(["cmd", "/c", "mklink", "/J", str(vendor), str(source)], capture_output=True)
        require(result.returncode == 0, "cannot create own corpus junction")
    env = dict(os.environ, WPT_GPU=selector, WPT_GPU_SESSION=session,
               WPT_OFFSCREEN="1", WPT_PAGE="1", WPT_COMPARE="exact",
               WPT_ANIMATION_TIME_MS="125", WPT_REPORT=str(report),
               WPT_GPU_CONTEXT=str(sidecar(report)), WPT_WIN_POS="-4000,60", KAMIN_FORCE_SCALE="1")
    with slot(), (report.parent / (report.name + ".log")).open("wb") as log:
        if b"WPT_GPU_CONTEXT" in exe.read_bytes():
            result = subprocess.run([str(exe), str(pair_list)], cwd=work, env=env,
                                    stdout=log, stderr=subprocess.STDOUT, timeout=900)
        else:
            observer = Path(__file__).with_name("wpt_legacy_observer.py")
            require(observer.exists(), "baseline has no capture attestation and observer is unavailable")
            result = subprocess.run([os.sys.executable, str(observer), str(exe), str(pair_list)],
                                    cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=900)
    require(result.returncode == 0, f"worker failed ({result.returncode}): {report.name}.log")
    load(report, exe, pair_list)
    wanted = pairs(pair_list)
    got = verdicts(report)
    require(set(wanted) == set(got), "worker omitted/added pairs")


def pairs(path):
    return ["|".join(line.split("|")[:2]) for line in Path(path).read_text(encoding="utf-8-sig").splitlines() if "|" in line]


def verdicts(path):
    return {"|".join(parts[:2]): "|".join(parts[2:])
            for line in Path(path).read_text(encoding="utf-8-sig").splitlines()
            if len(parts := line.split("|")) >= 3}


def gate(candidate, base, out, lists):
    out = Path(out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    selector, session = resolve(candidate), boot_session()
    all_pairs = sorted(set(pair for path in lists for pair in pairs(path)))
    require(bool(all_pairs), "empty gate list")
    atomic_json(out / "gate-request.json", {"selector": selector, "boot_session": session,
                "candidate_sha256": sha(candidate), "base_sha256": sha(base)})
    losses, flakes = [], []
    for index in range(0, len(all_pairs), 150):
        pair_list = out / f"pairs-{index // 150:03}.txt"
        pair_list.write_text("\n".join(all_pairs[index:index + 150]) + "\n", encoding="utf-8")
        baseline, new = (out / f"{name}-{index // 150:03}.rep" for name in ("base", "candidate"))
        for exe, report in ((base, baseline), (candidate, new)):
            run(exe, pair_list, report, selector, session, os.environ.get("GATE_RESUME") == "1")
        compare(baseline, new)
        a, b = verdicts(baseline), verdicts(new)
        suspects = [pair for pair in a if a[pair] == "0.00" and b[pair] != "0.00" and not b[pair].startswith("вне цели")]
        for number, pair in enumerate(suspects):
            isolated = out / f"isolated-{index}-{number}.txt"
            isolated.write_text(pair + "\n", encoding="utf-8")
            checks = []
            for attempt in range(2):
                report = out / f"isolated-{index}-{number}-{attempt}.rep"
                run(candidate, isolated, report, selector, session)
                compare(baseline, report)
                checks.append(verdicts(report)[pair] == "0.00")
            (flakes if all(checks) else losses).append(pair)
    if flakes:
        print(f"GATE FLAKES (green 2/2 alone): {len(flakes)}")
    print(f"GATE {'LOSSES ' + str(len(losses)) if losses else 'OK'} n={len(all_pairs)} selector={selector}")
    for pair in losses:
        print(pair)
    return int(bool(losses))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    g = sub.add_parser("gate")
    g.add_argument("candidate", type=Path)
    g.add_argument("base", type=Path)
    g.add_argument("out", type=Path)
    g.add_argument("lists", nargs="+", type=Path)
    s = sub.add_parser("slice")
    s.add_argument("exe", type=Path)
    s.add_argument("pair_list", type=Path)
    s.add_argument("report", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "gate":
            return gate(args.candidate.resolve(), args.base.resolve(), args.out, args.lists)
        require(len(pairs(args.pair_list)) <= 150, "slice exceeds 150 pairs")
        selector = os.environ.get("WPT_GPU")
        if not selector or not selector.startswith("luid:"):
            selector = resolve(args.exe.resolve())
        run(args.exe, args.pair_list, args.report, selector, boot_session())
        rows = verdicts(args.report)
        print(f"pairs={len(rows)} green={sum(v == '0.00' for v in rows.values())} selector={selector}")
        return 0
    except (InfrastructureError, OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"INCOMPARABLE_GPU_CONTEXT: {error}", file=os.sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
