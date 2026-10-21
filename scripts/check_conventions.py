"""Check added convention debt; duplicate warnings are advisory.

Usage: python scripts/check_conventions.py [base=origin/main] [--head HEAD]
Checks committed trees, using their merge base (the same range as CI).
Clippy owns environment access, debug macros and console output checks.
"""

import argparse
from collections import defaultdict
from pathlib import Path
import re
import subprocess
import sys


def git(*args):
    return subprocess.check_output(["git", "-C", str(REPO), *args])


def tree_sources(revision):
    """Read blobs in one batch rather than starting Git for every Rust file."""
    files = git("ls-tree", "-r", "--name-only", revision, "crates/html/src").decode().splitlines()
    files = [f for f in files if f.endswith(".rs")]
    if not files:
        return {}
    queries = "".join(f"{revision}:{f}\n" for f in files).encode()
    data = subprocess.check_output(
        ["git", "-C", str(REPO), "cat-file", "--batch"], input=queries
    )
    sources, offset = {}, 0
    for filename in files:
        end = data.index(b"\n", offset)
        size = int(data[offset:end].split()[-1])
        offset = end + 1
        sources[filename] = data[offset:offset + size].decode("utf-8", "replace")
        offset += size + 1
    return sources


RULES = [
    ("1 explicit state: new thread_local!", r"\bthread_local!\s*\{", r"^crates/html/src/"),
    ("1 explicit state: new static Mutex/RwLock", r"\bstatic\s+\w+\s*:\s*[^=]*\b(Mutex|RwLock|OnceLock<\s*Mutex)", r"^crates/html/src/"),
    ("3 side channel: kamin-* attribute/tag", r'"kamin-[a-z-]+"', r"^crates/html/src/(?!.*tests?\.rs)(?!svg/clip/ids\.rs)"),
    ("4 multi-frame layout: window.refresh()", r"\bwindow\.refresh\(\)", r"^crates/html/src/(layout|render)/"),
    ("5 journal in comment", r"//.*(ЗАМЕРЕН|ОТКАЧЕН|ОТКАТИЛ|ПРОБОВАЛИ|\bv\d{2,3}\b|scout-|\(\d\d\.\d\d)", r"\.rs$"),
    ("5 placeholder header", r"//!.*split out to keep", r"\.rs$"),
    ("5 out-of-repo reference", r"//.*(target/scout|agents[\\/]|kaminide-private)", r"\.rs$"),
    # A method ban cannot distinguish valid partial comparisons from unwraps.
    ("7 panic on NaN", r"partial_cmp\([^)]*\)\s*\.unwrap\(\)", r"\.rs$"),
]

# Rule 7 for float ordering: `partial_cmp(..).unwrap_or(Equal)` is not a total
# order once a NaN appears (`sort_by` may panic, `max_by` picks arbitrarily);
# `f32::total_cmp` is. Matched over whole files because rustfmt splits the
# chain across lines; a changed file may not add occurrences.
PARTIAL_FALLBACK = re.compile(
    r"partial_cmp\s*\((?:[^()]|\([^()]*\))*\)\s*\.unwrap_or(?:_else|_default)?\s*\("
)


def windows(src):
    lines = [re.sub(r"\s+", "", line) for line in src.splitlines()]
    lines = [line for line in lines if line and not line.startswith("//")
             and line not in ("{", "}", "};", ")", "),")]
    return {"\n".join(lines[i:i + 7]) for i in range(max(0, len(lines) - 6))}


def shared_counts(sources, changed):
    index = defaultdict(set)
    for filename, src in sources.items():
        if not filename.endswith("fields.rs"):
            for window in windows(src):
                index[window].add(filename)
    result = {}
    for filename in changed:
        shared = defaultdict(int)
        for window in windows(sources.get(filename, "")):
            for other in index[window] - {filename}:
                shared[other] += 1
        result[filename] = shared
    return result


def check(args):
    head = git("rev-parse", "--verify", f"{args.head}^{{commit}}").decode().strip()
    base = git("merge-base", args.base, head).decode().strip()
    diff_range = f"{base}..{head}"
    bad, current = [], None
    in_test = False
    diff = git("diff", "-U0", diff_range, "--", "crates", "vendor").decode("utf-8", "replace")
    for line in diff.splitlines():
        if line.startswith("+++ b/"):
            current = line[6:]
            in_test = False
        elif line.startswith("+") and not line.startswith("+++") and current:
            # Preserve the existing added-test exemption; shared state is still
            # checked even inside a new test module.
            if "#[cfg(test)]" in line:
                in_test = True
            if in_test and not line[1:].lstrip().startswith(("thread_local", "static")):
                continue
            for name, pattern, path in RULES:
                if re.search(path, current) and re.search(pattern, line[1:]):
                    bad.append((name, current, line[1:].strip()[:110]))

    changed = git("diff", "--name-only", "--diff-filter=AM", diff_range,
                  "--", "crates/html/src").decode().splitlines()
    changed = [f for f in changed if f.endswith(".rs")]
    dups = []
    if changed:
        sources, old_sources = tree_sources(head), tree_sources(base)
        for filename in changed:
            if filename.endswith("fields.rs"):
                continue
            src = sources[filename]
            size, old_size = len(src.splitlines()), len(old_sources.get(filename, "").splitlines())
            if size > 250 and size > old_size:
                bad.append(("5 file over 250 lines (grew)", filename, f"{old_size} -> {size}"))
            if not src.lstrip("\ufeff").startswith("//!"):
                bad.append(("5 missing //! header", filename, ""))
            found = len(PARTIAL_FALLBACK.findall(src))
            old_found = len(PARTIAL_FALLBACK.findall(old_sources.get(filename, "")))
            if found > old_found:
                bad.append(("7 float order: partial_cmp(..).unwrap_or(..), use total_cmp",
                            filename, f"{old_found} -> {found}"))
        now, before = shared_counts(sources, changed), shared_counts(old_sources, changed)
        for filename in changed:
            for other, count in sorted(now[filename].items(), key=lambda pair: -pair[1]):
                old_count = before[filename].get(other, 0)
                if count >= args.dup_min and count > old_count:
                    dups.append((filename, other, f"{old_count}->{count}"))

    for name, filename, detail in bad:
        print(f"RULE {name}: {filename}: {detail}")
    for filename, other, count in dups[:60]:
        print(f"DUP {count} shared 7-line windows: {filename} <-> {other}")
    print(f"SUMMARY rules={len(bad)} dups={len(dups)}")
    return 1 if bad else 0


REPO = Path(__file__).resolve().parents[1]

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base", nargs="?", default="origin/main", help="diff base revision")
    parser.add_argument("--head", default="HEAD", help="committed candidate revision")
    parser.add_argument("--dup-min", type=int, default=4, help="advisory duplicate threshold")
    try:
        sys.exit(check(parser.parse_args()))
    except subprocess.CalledProcessError as error:
        print(f"ERROR Git check failed (exit {error.returncode})", file=sys.stderr)
        sys.exit(2)
