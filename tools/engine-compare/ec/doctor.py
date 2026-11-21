"""doctor: engines present, CPU topology and lanes, foreign load on the lanes, optional A/A noise."""
import ctypes
import os
import time
from ctypes import wintypes
from pathlib import Path

from .config import CLASSES, LANE_WIDTH

K = ctypes.WinDLL('kernel32', use_last_error=True)
NT = ctypes.WinDLL('ntdll')


def topology():
    """{logical cpu: (efficiency class, core id)} from GetLogicalProcessorInformationEx(RelationProcessorCore)."""
    n = wintypes.DWORD(0)
    K.GetLogicalProcessorInformationEx(0, None, ctypes.byref(n))
    buf = ctypes.create_string_buffer(n.value)
    if not K.GetLogicalProcessorInformationEx(0, buf, ctypes.byref(n)):
        return {}
    out, off, core = {}, 0, 0
    while off < n.value:
        size = int.from_bytes(buf.raw[off + 4:off + 8], 'little'); eff = buf.raw[off + 9]
        mask = int.from_bytes(buf.raw[off + 32:off + 40], 'little')
        for cpu in range(64):
            if mask >> cpu & 1:
                out[cpu] = (eff, core)
        core += 1; off += size
    return out


class SPPI(ctypes.Structure):
    _fields_ = [('Idle', ctypes.c_longlong), ('Kernel', ctypes.c_longlong), ('User', ctypes.c_longlong),
                ('Dpc', ctypes.c_longlong), ('Int', ctypes.c_longlong), ('Count', wintypes.ULONG)]


def cpu_times():
    n = os.cpu_count() or 64
    arr = (SPPI * n)()
    NT.NtQuerySystemInformation(8, arr, ctypes.sizeof(arr), None)  # SystemProcessorPerformanceInformation
    return [(a.Idle, a.Kernel + a.User) for a in arr]


def load(seconds=1.0):
    """Busy share per logical CPU over ``seconds``."""
    a = cpu_times(); time.sleep(seconds); b = cpu_times(); out = []
    for (i0, t0), (i1, t1) in zip(a, b):
        tot = t1 - t0  # kernel time includes idle
        out.append(max(0.0, 1 - (i1 - i0) / tot) if tot else 0.0)
    return out


class PE32(ctypes.Structure):
    _fields_ = [('dwSize', wintypes.DWORD), ('cntUsage', wintypes.DWORD), ('th32ProcessID', wintypes.DWORD),
                ('th32DefaultHeapID', ctypes.c_size_t), ('th32ModuleID', wintypes.DWORD), ('cntThreads', wintypes.DWORD),
                ('th32ParentProcessID', wintypes.DWORD), ('pcPriClassBase', ctypes.c_long), ('dwFlags', wintypes.DWORD),
                ('szExeFile', ctypes.c_wchar * 260)]


# Builds and measured engines must stay off the measuring lanes (the owner's own browser is not listed on purpose).
INTRUDERS = ('cargo.exe', 'sccache.exe', 'rustc.exe', 'link.exe', 'lld-link.exe', 'rust-lld.exe', 'cl.exe', 'clang.exe',
             'servoshell.exe', 'blitz.exe', 'geckodriver.exe')


def processes():
    K.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    snap = K.CreateToolhelp32Snapshot(2, 0); e = PE32(); e.dwSize = ctypes.sizeof(e); out = []
    ok = K.Process32FirstW(snap, ctypes.byref(e))
    while ok:
        out.append((e.th32ProcessID, e.szExeFile.lower()))
        ok = K.Process32NextW(snap, ctypes.byref(e))
    K.CloseHandle(snap)
    return out


def intruders(lanes, own=()):
    """[(pid, name, mask)] of builds/engines whose CPU mask touches a measuring lane."""
    lane = 0
    for base in lanes:
        lane |= ((1 << LANE_WIDTH) - 1) << base
    out = []
    for pid, name in processes():
        if pid in own or not (name in INTRUDERS or name.startswith('engine_bench')):
            continue
        h = K.OpenProcess(0x1000, False, pid)
        if not h:
            continue
        a, s = ctypes.c_size_t(), ctypes.c_size_t()
        if K.GetProcessAffinityMask(wintypes.HANDLE(h), ctypes.byref(a), ctypes.byref(s)) and a.value & lane:
            out.append((pid, name, a.value))
        K.CloseHandle(h)
    return out


def check_lanes(topo, lanes):
    """Problems of the lane layout: every lane = 2 physical cores x 2 threads of the fastest class, disjoint."""
    bad = []; best = max((e for e, _ in topo.values()), default=0); used = set()
    for n, base in enumerate(lanes):
        cpus = list(range(base, base + LANE_WIDTH))
        if any(c not in topo for c in cpus):
            bad.append(f'lane {n}: CPUs {cpus} do not all exist'); continue
        cores = {topo[c][1] for c in cpus}
        if any(topo[c][0] != best for c in cpus):
            bad.append(f'lane {n}: CPUs {cpus} are not all of the fastest class (performance cores)')
        if len(cores) != 2:
            bad.append(f'lane {n}: CPUs {cpus} span {len(cores)} physical cores (need 2 with 2 threads each)')
        if used & set(cpus):
            bad.append(f'lane {n} overlaps another lane')
        used |= set(cpus)
    return bad


def main(cfg, a):
    ok = True
    print('engines:')
    for name, e in cfg.engines.items():
        miss = [k for k in ('exe', 'counters_exe', 'geckodriver') if e.get(k) and not Path(e[k]).exists()]
        if e.get('adapter') == 'kitesurf':
            from .adapters.kitesurf import Kitesurf
            why = Kitesurf(name, cfg, e).ready()
            print(f'  {name:10} {"ok" if not why else why}'); ok &= not why; continue
        print(f'  {name:10} {"ok" if not miss else "missing " + ", ".join(miss)}'); ok &= not miss
    for k, v in (('node', cfg.node), ('playwright', cfg.playwright), ('selenium', cfg.selenium)):
        if v and not Path(v).exists() and k != 'node':
            print(f'  {k}: missing {v}'); ok = False
    print(f'pages: {len(cfg.pages)}; missing files: ' + (', '.join(p.id for p in cfg.pages.values()
          if not all(p.file(f).exists() for f in ('base.html', 'vis.html', 'probe.html'))) or 'none'))
    topo = topology()
    best = max((e for e, _ in topo.values()), default=0)
    pc = sorted(c for c, (e, _) in topo.items() if e == best)
    print(f'topology: {len(topo)} logical CPUs; fastest class CPUs {pc[0]}-{pc[-1]} ({len(pc)})' if pc else 'topology: unknown')
    bad = check_lanes(topo, cfg.lanes)
    for b in bad:
        print('  ' + b)
    ok &= not bad
    print(f'lanes: {cfg.lanes} (classes: ' + ', '.join(f'{c}=0x{int(m, 16):X}' for c, (m, _, _) in CLASSES.items()) + ' inside each lane)')
    ld = load(1.0)
    for n, base in enumerate(cfg.lanes):
        busy = max(ld[base:base + LANE_WIDTH], default=0)
        flag = 'BUSY' if busy > 0.10 else 'idle'
        print(f'  lane {n} CPUs {base}-{base + LANE_WIDTH - 1}: {flag} (max {busy:.0%} over 1 s)')
        ok &= busy <= 0.10
    bad = intruders(cfg.lanes)
    for pid, name, mask in bad:
        print(f'  intruder on the lanes: {name} pid {pid} mask 0x{mask:X}')
    ok &= not bad
    others = [f'{i}:{x:.0%}' for i, x in enumerate(ld) if x > 0.5]
    if others:
        print('  busy CPUs elsewhere: ' + ' '.join(others))
    if a.aa:
        ok &= aa(cfg)
    print('doctor: ' + ('OK' if ok else 'PROBLEMS above'))


def aa(cfg):
    """A/A: the same ours unit twice per lane; relative difference per phase (noise floor of the stand)."""
    from . import adapters as A
    from .scheduler import Lane
    name = next((n for n, e in cfg.engines.items() if e.get('adapter') == 'ours'), None)
    page = next(iter(cfg.pages.values()), None)
    if not name or not page:
        print('aa: needs an ours engine and a page'); return False
    ad = A.load(name, cfg); pol = A.Policy(); part = ad.parts[0]
    print(f'aa: {name} on {page.id}, gpu4, part {part}')
    for n, base in enumerate(cfg.lanes):
        lane = Lane(n, base, cfg.work / f'lane{n}')
        r1 = ad.measure(lane, page, 'gpu4', part, pol); r2 = ad.measure(lane, page, 'gpu4', part, pol)
        d = []
        for ph, x in r1['rows'].items():
            y = r2['rows'].get(ph) or {}
            if x.get('time_ms') and y.get('time_ms'):
                d.append(f'{ph} {100 * (y["time_ms"] / x["time_ms"] - 1):+.0f}%')
        print(f'  lane {n}: ' + ', '.join(d))
    return True

