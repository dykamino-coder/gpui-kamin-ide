"""How an engine_bench exe was built, and the refusal to compare two that were built differently.

A base and a candidate with different cargo features or profiles differ by more than the change
under test: `timing-only` against `engine-bench` adds the work counters, `allocation-audit` adds
the allocation probe, a build without LTO has other machine code. The exe reports its build as
`engine_bench --build-info` and in the `build` object of its JSON; an older exe has neither, and
then only what its JSON shows (work counters, allocation audit) is compared.

    python scripts/bench_build.py BASE.exe|BASE.json CAND.exe|CAND.json   # exit 1 and the differences
"""

import json
import subprocess
import sys

FIELDS = ("features", "profile", "opt_level", "debug_assertions", "profile_overrides")
ALLOW = "--allow-feature-mismatch"


def of_exe(exe):
    """The exe's own build record, or None for an exe that predates `--build-info`."""
    try:
        with subprocess.Popen([str(exe), "--build-info"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                              stdin=subprocess.DEVNULL) as process:
            try:
                text, _ = process.communicate(timeout=60)
            except subprocess.TimeoutExpired:
                process.kill()
                return None
        build = json.loads(text.decode("utf-8")) if process.returncode == 0 else None
    except (OSError, ValueError):
        return None
    return build if isinstance(build, dict) and "features" in build else None


def of_report(report):
    """The build record of a bench JSON; for an older JSON, the features its content shows."""
    build = report.get("build")
    if isinstance(build, dict) and "features" in build:
        return build
    shown = {}
    if "work_counters_enabled" in report:
        shown["work-counters"] = bool(report["work_counters_enabled"])
    for mode in (report.get("modes") or {}).values():
        samples = ((mode or {}).get("process_resources") or {}).get("samples") or []
        if samples and "rust_allocation_audit_enabled" in samples[0]:
            shown["allocation-audit"] = bool(samples[0]["rust_allocation_audit_enabled"])
            break
    return {"shown_features": shown} if shown else None


def shown(build):
    """Feature flags a build record answers for, whichever form it has."""
    if "shown_features" in build:
        return build["shown_features"]
    return {name: name in build["features"] for name in ("work-counters", "allocation-audit")}


def differences(base, cand):
    """Human-readable differences between two build records; empty when they match."""
    if "features" in base and "features" in cand:
        out = []
        only_base = sorted(set(base["features"]) - set(cand["features"]))
        only_cand = sorted(set(cand["features"]) - set(base["features"]))
        if only_base or only_cand:
            out.append(f"features: base only {only_base or '-'}, candidate only {only_cand or '-'}")
        for field in FIELDS[1:]:
            if base.get(field) != cand.get(field):
                out.append(f"{field}: base {base.get(field)!r}, candidate {cand.get(field)!r}")
        return out
    one, two = shown(base), shown(cand)
    return [f"{name}: base {one[name]}, candidate {two[name]}"
            for name in sorted(set(one) & set(two)) if one[name] != two[name]]


def require_same(base, cand, allow=False, out=sys.stderr):
    """Stops a comparison of differently built sides. An unknown side is reported, not refused."""
    if base is None or cand is None:
        side = "both sides" if base is None and cand is None else "the base" if base is None else "the candidate"
        print(f"build check: {side} does not report its build (older engine_bench); features not compared", file=out)
        return []
    found = differences(base, cand)
    if found and not allow:
        raise SystemExit("the base and the candidate were built differently, the comparison would measure the build:\n  "
                         + "\n  ".join(found) + f"\nbuild both the same way, or pass {ALLOW} if this is intended")
    if found:
        print("build check: comparing differently built sides (" + "; ".join(found) + ")", file=out)
    return found


def load(path):
    """Build record of an exe or of a bench JSON file."""
    if str(path).lower().endswith(".json"):
        with open(path, "rb") as handle:
            text = handle.read().decode("utf-8-sig")
        return of_report(json.loads(text[text.index("{"):]))
    return of_exe(path)


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    allow = ALLOW in argv
    paths = [a for a in argv if a != ALLOW]
    if len(paths) != 2:
        raise SystemExit(__doc__)
    require_same(load(paths[0]), load(paths[1]), allow)


if __name__ == "__main__":
    main()
