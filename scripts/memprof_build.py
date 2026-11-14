"""Builds engine_bench for the profilers (memprof.py build): release code, names for every function.

  memprof.py build --out DIR [--worktree WT] [--features allocation-audit]
                   [--jobs N] [--affinity MASK] [--below-normal] [--wrapper "command prefix"]

The build is the release profile with the PDB kept (CARGO_PROFILE_RELEASE_STRIP=false,
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only: function names, inlined frames and line tables), so it
works on any commit, a base included. The linker is told not to fold identical functions (/OPT:NOICF):
folded, several functions share one body and a stack frame in it carries the name of any one of them,
so a profile blames the wrong function. The linker flags of the worktree's .cargo/config.toml (the
8 MiB stack) are kept. The result is checked: the stack reserve in the PE header, function records in
the PDB, no folded functions.

One target directory per checkout, always <worktree>/target/prof: every profiler build of a seat
reuses it, so a rebuild after an edit is incremental.

DIR receives engine_bench.exe, engine_bench.pdb and build.json (commit, features, flags).
"""

import json
import os
import pathlib
import re
import shlex
import shutil
import struct
import subprocess
import sys
import tomllib

TARGET = "x86_64-pc-windows-msvc"
NOICF = ["-C", "link-arg=/OPT:NOICF"]


def add_parser(sub):
    parser = sub.add_parser("build", help="build engine_bench with symbols and without identical-code folding")
    parser.add_argument("--out", required=True, help="directory for engine_bench.exe, its PDB and build.json")
    parser.add_argument("--worktree", default=".", help="repository checkout to build (default: current directory)")
    parser.add_argument("--features", default="allocation-audit",
                        help="engine-bench features: allocation-audit (exact, audit counters; default), "
                             "heap-profile (probe only), timing-only (time profiles)")
    parser.add_argument("--jobs", type=int, help="cargo build jobs")
    parser.add_argument("--affinity", help="core bit mask of the build, e.g. 0xFFF00")
    parser.add_argument("--below-normal", action="store_true", help="run the build at below-normal priority")
    parser.add_argument("--wrapper", help="command prefix, e.g. a build-slot queue script")


def rustflags(worktree):
    """Linker and compiler flags of the worktree's cargo config for this target, plus /OPT:NOICF."""
    config = pathlib.Path(worktree) / ".cargo" / "config.toml"
    flags = []
    if config.exists():
        data = tomllib.loads(config.read_text(encoding="utf-8"))
        for table in (data.get("build", {}), data.get("target", {}).get(TARGET, {})):
            value = table.get("rustflags", [])
            flags += value.split() if isinstance(value, str) else value
    return flags + NOICF


def stack_reserve(exe):
    """SizeOfStackReserve of a PE32+ image."""
    with open(exe, "rb") as image:
        image.seek(0x3C)
        (pe,) = struct.unpack("<I", image.read(4))
        image.seek(pe + 24)
        optional = image.read(80)
    if optional[:2] != b"\x0b\x02":
        raise SystemExit(f"{exe}: not a PE32+ image")
    return struct.unpack_from("<Q", optional, 72)[0]


def expected_stack(flags):
    sizes = [int(size) for flag in flags for size in re.findall(r"/STACK:(\d+)", flag)]
    return sizes[-1] if sizes else None


def pin(process, affinity, below_normal):
    """Core mask and priority of a started process; its children inherit both."""
    import ctypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    kernel.SetPriorityClass.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    handle = int(process._handle)
    if below_normal:
        kernel.SetPriorityClass(handle, 0x4000)  # BELOW_NORMAL_PRIORITY_CLASS
    if affinity and not kernel.SetProcessAffinityMask(handle, affinity):
        raise OSError(ctypes.get_last_error(), f"SetProcessAffinityMask {affinity:#x}")


def git(worktree, *args):
    done = subprocess.run(["git", "-C", str(worktree), *args], capture_output=True, text=True)
    return done.stdout.strip() if done.returncode == 0 else None


def check(exe, flags):
    """Facts of the built image a profile depends on; raises when one does not hold."""
    from memprof_symbols import Symbols
    reserve, wanted = stack_reserve(exe), expected_stack(flags)
    if wanted and reserve != wanted:
        raise SystemExit(f"{exe}: stack reserve {reserve}, the cargo config asks for {wanted}")
    _addresses, functions, folded = Symbols(exe, 0x140000000).scan()
    if not functions:
        raise SystemExit(f"{exe}: its PDB has no function records")
    if folded:
        raise SystemExit(f"{exe}: {len(folded)} functions are still folded (/OPT:NOICF did not reach the linker)")
    return {"stack_reserve": reserve, "functions": functions, "folded_functions": 0}


def build_command(args):
    worktree = pathlib.Path(args.worktree).resolve()
    target = worktree / "target" / "prof"
    out = pathlib.Path(args.out).resolve()
    flags = rustflags(worktree)
    env = dict(os.environ)
    for name in ("RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS", f"CARGO_TARGET_{TARGET.upper().replace('-', '_')}_RUSTFLAGS"):
        if env.pop(name, None) is not None:
            print(f"note: {name} is ignored; the flags come from {worktree / '.cargo' / 'config.toml'}", file=sys.stderr)
    # The encoded form has the highest precedence in cargo and survives any shell in --wrapper.
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
    env.update(CARGO_PROFILE_RELEASE_STRIP="false", CARGO_PROFILE_RELEASE_DEBUG="line-tables-only")
    if args.jobs:
        env["CARGO_BUILD_JOBS"] = str(args.jobs)
    prefix = [part.strip('"') for part in shlex.split(args.wrapper, posix=False)] if args.wrapper else []
    command = [*prefix, "cargo", "build", "-p", "engine-bench", "--bin", "engine_bench", "--features", args.features,
               "--release", "--target-dir", str(target)]
    print(" ".join(command), file=sys.stderr)
    process = subprocess.Popen(command, cwd=worktree, env=env, stdin=subprocess.DEVNULL)
    if args.affinity or args.below_normal:
        pin(process, int(args.affinity, 0) if args.affinity else 0, args.below_normal)
    code = process.wait()
    if code:
        raise SystemExit(f"cargo build failed ({code})")
    built = target / "release" / "engine_bench.exe"
    facts = check(built, flags)
    out.mkdir(parents=True, exist_ok=True)
    for name in ("engine_bench.exe", "engine_bench.pdb"):
        shutil.copy2(built.with_name(name), out / name)
    facts.update(commit=git(worktree, "rev-parse", "HEAD"), dirty=bool(git(worktree, "status", "--porcelain")),
                 features=args.features, profile="release + debug line tables", rustflags=flags)
    (out / "build.json").write_text(json.dumps(facts, indent=1), encoding="utf-8")
    print(f"{out / 'engine_bench.exe'}: {facts['functions']} functions named, none folded, "
          f"stack {facts['stack_reserve'] >> 20} MiB, commit {facts['commit']}{' (dirty)' if facts['dirty'] else ''}")
