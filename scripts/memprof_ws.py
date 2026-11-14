"""Working set of a running engine_bench by memory region, at the moment of its peak (memprof.py ws).

The live-heap reports (memprof.py run) see only Rust allocations. A peak working set can also come from
allocator segments that stay committed after the heap shrank, from mapped files, images, thread stacks or
the graphics driver. This samples the process's working set every --interval-ms; whenever it reaches a
new peak by at least --step-mb it takes one breakdown:

  VirtualQueryEx over the address space -> every region with its type (image, mapped, private), protection
  and reservation (AllocationBase); QueryWorkingSet -> every resident page. Resident pages are summed by
  category and by reservation.

Categories of private memory (the process's own commit):
  stack        reservations holding a guard page (thread stacks)
  wc/nocache   write-combined or uncached pages (graphics driver mappings of the GPU heap)
  big-res      private reservations of 16 MiB or more (allocator arenas and segments, driver heaps)
  small-res    other private reservations
Shared pages (resident in another process too) are counted apart.

The breakdown at the peak is the report; the base and candidate are compared by category, and the largest
private reservations of each side are listed (their addresses differ between processes, their sizes and
protections do not).
"""

import ctypes
import ctypes.wintypes as w
import json
import subprocess
import sys
import time

MEM_COMMIT, MEM_IMAGE, MEM_MAPPED, MEM_PRIVATE = 0x1000, 0x1000000, 0x40000, 0x20000
PAGE_GUARD, PAGE_NOCACHE, PAGE_WRITECOMBINE = 0x100, 0x200, 0x400
PROCESS_QUERY_INFORMATION, PROCESS_VM_READ, SYNCHRONIZE = 0x400, 0x10, 0x100000
PAGE = 4096
BIG_RESERVATION = 16 << 20


class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p),
                ("AllocationProtect", w.DWORD), ("PartitionId", w.WORD), ("RegionSize", ctypes.c_size_t),
                ("State", w.DWORD), ("Protect", w.DWORD), ("Type", w.DWORD)]


class PMC(ctypes.Structure):
    _fields_ = [("cb", w.DWORD), ("PageFaultCount", w.DWORD), ("PeakWorkingSetSize", ctypes.c_size_t),
                ("WorkingSetSize", ctypes.c_size_t), ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPagedPoolUsage", ctypes.c_size_t), ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                ("QuotaNonPagedPoolUsage", ctypes.c_size_t), ("PagefileUsage", ctypes.c_size_t),
                ("PeakPagefileUsage", ctypes.c_size_t), ("PrivateUsage", ctypes.c_size_t)]


k32 = ctypes.WinDLL("kernel32", use_last_error=True)
psapi = ctypes.WinDLL("psapi", use_last_error=True)
k32.OpenProcess.restype = w.HANDLE
k32.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
k32.VirtualQueryEx.restype = ctypes.c_size_t
k32.VirtualQueryEx.argtypes = [w.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
k32.CloseHandle.argtypes = [w.HANDLE]
k32.WaitForSingleObject.argtypes = [w.HANDLE, w.DWORD]
k32.WaitForSingleObject.restype = w.DWORD
psapi.GetProcessMemoryInfo.argtypes = [w.HANDLE, ctypes.POINTER(PMC), w.DWORD]
psapi.QueryWorkingSet.argtypes = [w.HANDLE, ctypes.c_void_p, w.DWORD]


def regions(handle):
    """Every committed region: (base, size, type, protect, allocation base)."""
    out, addr, mbi = [], 0, MBI()
    while k32.VirtualQueryEx(handle, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
        base, size = mbi.BaseAddress or 0, mbi.RegionSize
        if mbi.State == MEM_COMMIT:
            out.append((base, size, mbi.Type, mbi.Protect, mbi.AllocationBase or 0))
        addr = base + size
        if addr >= 1 << 47:
            break
    return out


def resident_pages(handle):
    """(virtual page number, shared) of every resident page (QueryWorkingSet)."""
    count = 1 << 16
    while True:
        buf = (ctypes.c_size_t * (count + 1))()
        if psapi.QueryWorkingSet(handle, buf, ctypes.sizeof(buf)):
            n = buf[0]
            return [(e >> 12, bool(e & 0x100)) for e in buf[1:n + 1]]
        if ctypes.get_last_error() != 24:  # ERROR_BAD_LENGTH
            raise OSError(ctypes.get_last_error(), "QueryWorkingSet")
        count = int(buf[0]) + 4096 if buf[0] else count * 2


def category(typ, reservation):
    """Category of a page by its region type and its reservation (size, guard and write-combine flags)."""
    if typ == MEM_IMAGE:
        return "image"
    if typ == MEM_MAPPED:
        return "mapped"
    if reservation["guard"]:
        return "stack"
    if reservation["wc"]:
        return "wc/nocache"
    return "big-res" if reservation["size"] >= BIG_RESERVATION else "small-res"


def breakdown(handle):
    regs = regions(handle)
    reservations = {}
    for base, size, typ, prot, alloc in regs:
        r = reservations.setdefault((typ, alloc), {"size": 0, "guard": False, "wc": False, "prot": set()})
        r["size"] = max(r["size"], base + size - alloc)
        r["guard"] |= bool(prot & PAGE_GUARD)
        r["wc"] |= bool(prot & (PAGE_NOCACHE | PAGE_WRITECOMBINE))
        r["prot"].add(prot & 0xFF)
    starts = sorted((base, base + size, typ, alloc) for base, size, typ, prot, alloc in regs)
    import bisect
    keys = [s[0] for s in starts]
    cats, by_res = {}, {}
    for vpn, shared in resident_pages(handle):
        addr = vpn << 12
        i = bisect.bisect_right(keys, addr) - 1
        if i < 0 or addr >= starts[i][1]:
            cat, res = "unknown", None
        else:
            typ, alloc = starts[i][2], starts[i][3]
            res = reservations[(typ, alloc)]
            cat = category(typ, res)
            if typ == MEM_PRIVATE:
                key = (alloc, res["size"])
                by_res[key] = by_res.get(key, 0) + PAGE
        if shared:
            cat += " (shared)"
        cats[cat] = cats.get(cat, 0) + PAGE
    top = sorted(by_res.items(), key=lambda kv: -kv[1])[:40]
    return {"categories": cats,
            "private_reservations": [{"reserved": size, "resident": res} for (alloc, size), res in top]}


def sample(pid, interval_ms, step_mb):
    """Follow `pid` until it exits; the breakdown at its working-set peak."""
    h = k32.OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | SYNCHRONIZE, False, pid)
    if not h:
        raise OSError(ctypes.get_last_error(), f"OpenProcess {pid}")
    best, best_ws, peak, pmc = None, 0, 0, PMC()
    pmc.cb = ctypes.sizeof(pmc)
    deadline = time.monotonic() + 1800
    try:
        while time.monotonic() < deadline:
            if not psapi.GetProcessMemoryInfo(h, ctypes.byref(pmc), pmc.cb):
                break
            ws = pmc.WorkingSetSize
            if ws >= best_ws + (step_mb << 20):
                try:
                    b = breakdown(h)
                except OSError:
                    break
                b["working_set"], b["private_usage"] = ws, pmc.PrivateUsage
                b["at_s"] = time.monotonic()
                best, best_ws = b, ws
            peak = pmc.PeakWorkingSetSize
            # WAIT_OBJECT_0: exited; anything but WAIT_TIMEOUT ends the watch too.
            if k32.WaitForSingleObject(h, interval_ms) != 0x102:
                break
        if best is not None:
            best["process_peak_working_set"] = peak
    finally:
        k32.CloseHandle(h)
    return best


def run_side(exe, bench, launcher, interval_ms, step_mb, out_json):
    """Launch one engine_bench (through the hidden-desktop launcher, which prints its child pid first)."""
    # stdout goes to a file, never a pipe: the bench writes its JSON while it is sampled, and an unread
    # pipe would block it. The launcher's first line names its child.
    command = [*launcher, exe, *bench]
    with open(out_json, "wb") as stdout:
        proc = subprocess.Popen(command, stdout=stdout, stderr=subprocess.DEVNULL, stdin=subprocess.DEVNULL)
    pid = proc.pid
    if launcher:
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            with open(out_json, "rb") as f:
                line = f.readline().decode(errors="replace")
            if line.endswith("\n"):
                break
            time.sleep(0.005)
        pid = int(line.rsplit(":", 1)[1])
    best = sample(pid, interval_ms, step_mb)
    proc.wait()
    return best


def mb(v):
    return v / (1 << 20)


def render(cand, base, out=sys.stdout):
    sides = [("cand", cand)] + ([("base", base)] if base else [])
    for name, b in sides:
        print(f"{name}: working set at peak {mb(b['working_set']):.1f} MB, process peak "
              f"{mb(b['process_peak_working_set']):.1f} MB, private usage {mb(b['private_usage']):.1f} MB", file=out)
    print("\nresident at the peak by category (MB):", file=out)
    keys = sorted(set(cand["categories"]) | set(base["categories"] if base else {}),
                  key=lambda k: -cand["categories"].get(k, 0))
    print(f"{'delta':>10} {'cand':>10} {'base':>10}  category", file=out)
    for k in keys:
        c = cand["categories"].get(k, 0)
        b = base["categories"].get(k, 0) if base else 0
        print(f"{mb(c - b):>+10.2f} {mb(c):>10.2f} {mb(b):>10.2f}  {k}", file=out)
    for name, b in sides:
        print(f"\n{name}: largest private reservations by resident pages (MB):", file=out)
        for r in b["private_reservations"][:15]:
            print(f"  resident {mb(r['resident']):8.2f}  reserved {mb(r['reserved']):9.2f}", file=out)


def ws_command(args, launcher):
    import os
    os.makedirs(args.out, exist_ok=True)
    bench = list(args.bench)
    if bench and bench[0] == "--":
        bench = bench[1:]
    if "--json" not in bench:
        bench.append("--json")
    cand = run_side(args.exe, bench, launcher, args.interval_ms, args.step_mb, os.path.join(args.out, "cand.json"))
    base = run_side(args.base_exe, bench, launcher, args.interval_ms, args.step_mb,
                    os.path.join(args.out, "base.json")) if args.base_exe else None
    report = {"cand": cand, "base": base}
    with open(os.path.join(args.out, "ws.json"), "w") as f:
        json.dump(report, f, indent=1)
    with open(os.path.join(args.out, "ws.txt"), "w", encoding="utf-8") as f:
        render(cand, base, f)
    render(cand, base)
