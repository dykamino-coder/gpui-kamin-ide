"""Fold the allocation stacks written by an allocation-audit engine_bench.

The bench writes them when ENGINE_BENCH_ALLOC_STACKS=<phase>[@<frame>[-<frame>]] and
ENGINE_BENCH_ALLOC_STACKS_OUT=<file> are set (crates/engine-bench/src/allocations/probe.rs).
Return addresses are resolved through dbghelp with the PDB of that exact build, which must
lie next to the executable (cargo leaves it in <target>/release/engine_bench.pdb).

  alloc_stacks.py EXE STACKS                         calls and bytes per stack
  alloc_stacks.py EXE STACKS BASE_EXE BASE_STACKS    only the stacks whose totals differ

Options: --depth N (frames kept per stack, default 12), --frames A-B (frame numbers kept).
"""

import argparse
import collections
import ctypes as c
import ctypes.wintypes as w
import pathlib
import re
import sys

# Frames of the allocator itself: everything up to the last of these is dropped.
ALLOCATOR = re.compile(
    r"probe::|Counted|GlobalAlloc|__rust_|__rustc|__rdl_|__rg_|alloc::alloc::|RtlCaptureStackBackTrace"
)
HASH = re.compile(r"::h[0-9a-f]{16}$")


class Symbols:
    """One image loaded at the base address the process ran it at."""

    count = 0

    def __init__(self, exe, base):
        self.dbg = c.WinDLL("dbghelp", use_last_error=True)
        self.dbg.SymInitializeW.argtypes = [w.HANDLE, w.LPCWSTR, w.BOOL]
        self.dbg.SymLoadModuleExW.argtypes = [
            w.HANDLE, w.HANDLE, w.LPCWSTR, w.LPCWSTR, c.c_uint64, w.DWORD, c.c_void_p, w.DWORD,
        ]
        self.dbg.SymLoadModuleExW.restype = c.c_uint64
        self.dbg.SymFromAddr.argtypes = [w.HANDLE, c.c_uint64, c.POINTER(c.c_uint64), c.c_void_p]
        Symbols.count += 1
        self.handle = 0x5A0000 + Symbols.count  # any unique value: no live process is attached
        exe = pathlib.Path(exe).resolve()
        if not self.dbg.SymInitializeW(self.handle, str(exe.parent), False):
            raise OSError(f"SymInitialize failed: {c.get_last_error()}")
        if not self.dbg.SymLoadModuleExW(self.handle, None, str(exe), None, base, 0, None, 0):
            raise OSError(f"cannot load symbols of {exe}: {c.get_last_error()}")
        self.names = {}

    def name(self, address):
        if address not in self.names:
            info = c.create_string_buffer(88 + 2048)  # SYMBOL_INFO followed by the name
            c.c_uint32.from_buffer(info, 0).value = 88
            c.c_uint32.from_buffer(info, 80).value = 2048
            offset = c.c_uint64()
            # A return address belongs to the call before it.
            if self.dbg.SymFromAddr(self.handle, address - 1, c.byref(offset), info):
                length = c.c_uint32.from_buffer(info, 76).value
                text = info.raw[84 : 84 + length].decode("utf-8", "replace")
                self.names[address] = HASH.sub("", text)
            else:
                self.names[address] = "?"
        return self.names[address]


def fold(exe, stacks, depth, frames):
    """{stack: [calls, bytes]} for one run."""
    lines = pathlib.Path(stacks).read_text(encoding="utf-8").splitlines()
    header = re.match(r"# engine_bench alloc stacks v1 base=([0-9a-f]+)", lines[0] if lines else "")
    if not header:
        raise SystemExit(f"{stacks}: not an engine_bench allocation stack file")
    symbols = Symbols(exe, int(header.group(1), 16))
    totals = collections.defaultdict(lambda: [0, 0])
    for line in lines[1:]:
        kind, frame, phase, size, *addresses = line.split()
        if frames and not frames[0] <= int(frame) <= frames[1]:
            continue
        names = [symbols.name(int(address, 16)) for address in addresses]
        own = [i for i, name in enumerate(names[:12]) if ALLOCATOR.search(name)]
        names = names[own[-1] + 1 if own else 0 :][:depth]
        total = totals[(phase, kind, " < ".join(names))]
        total[0] += 1
        total[1] += int(size)
    return totals


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    parser.add_argument("exe")
    parser.add_argument("stacks")
    parser.add_argument("base", nargs="*", help="BASE_EXE BASE_STACKS")
    parser.add_argument("--depth", type=int, default=12)
    parser.add_argument("--frames")
    args = parser.parse_args()
    if len(args.base) not in (0, 2):
        parser.error("the base needs both BASE_EXE and BASE_STACKS")
    frames = None
    if args.frames:
        first, _, last = args.frames.partition("-")
        frames = (int(first), int(last or first))
    rows = fold(args.exe, args.stacks, args.depth, frames)
    if args.base:
        base = fold(args.base[0], args.base[1], args.depth, frames)
        keys = set(rows) | set(base)
        rows = {
            key: [rows.get(key, [0, 0])[i] - base.get(key, [0, 0])[i] for i in (0, 1)]
            for key in keys
        }
        rows = {key: delta for key, delta in rows.items() if delta != [0, 0]}
    sign = "+" if args.base else ""
    print(f"{'calls':>9} {'bytes':>12}  phase kind stack")
    for (phase, kind, stack), (calls, size) in sorted(
        rows.items(), key=lambda row: (-abs(row[1][1]), -abs(row[1][0]), row[0])
    ):
        print(f"{calls:{sign}9d} {size:{sign}12d}  {phase} {kind} {stack}")
    print(
        f"{sum(v[0] for v in rows.values()):{sign}9d} {sum(v[1] for v in rows.values()):{sign}12d}  total",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
