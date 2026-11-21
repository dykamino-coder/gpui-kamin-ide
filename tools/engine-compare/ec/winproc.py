"""Windows process control with the standard library only (ctypes).

Every measured process tree runs:
- on a hidden, never-switched desktop (``STARTUPINFO.lpDesktop``), so no window reaches the user's screen;
- inside a Job Object that pins the whole tree to one CPU mask (``JOB_OBJECT_LIMIT_AFFINITY``), caps committed
  memory and kills the tree when the job handle closes;
- started suspended and resumed only after the job is in place, so no instruction runs on other cores.

``Tree`` samples the working set and CPU time of every process of the job (except driver helpers such as
node.exe) and answers exact CPU/WS snapshots at marks.
"""
import ctypes
import os
import subprocess
import threading
import time
from ctypes import wintypes

K = ctypes.WinDLL('kernel32', use_last_error=True)
U = ctypes.WinDLL('user32', use_last_error=True)
PS = ctypes.WinDLL('psapi', use_last_error=True)

CREATE_SUSPENDED = 0x4
CREATE_NO_WINDOW = 0x08000000
CREATE_UNICODE_ENVIRONMENT = 0x400
STARTF_USESTDHANDLES = 0x100
JOB_LIMIT_AFFINITY = 0x10
JOB_LIMIT_JOB_MEMORY = 0x200
JOB_LIMIT_KILL_ON_CLOSE = 0x2000
JOB_EXTENDED_LIMITS = 9
JOB_PID_LIST = 3
PROCESS_QUERY_LIMITED = 0x1000
SYNCHRONIZE = 0x00100000
GENERIC_ALL = 0x10000000

# Helpers that drive a browser but are not the browser: never counted in CPU or memory.
HELPERS = {'node.exe', 'geckodriver.exe', 'conhost.exe', 'cmd.exe', 'python.exe', 'msedgedriver.exe', 'chromedriver.exe'}


class STARTUPINFOW(ctypes.Structure):
    _fields_ = [('cb', wintypes.DWORD), ('lpReserved', wintypes.LPWSTR), ('lpDesktop', wintypes.LPWSTR),
                ('lpTitle', wintypes.LPWSTR), ('dwX', wintypes.DWORD), ('dwY', wintypes.DWORD),
                ('dwXSize', wintypes.DWORD), ('dwYSize', wintypes.DWORD), ('dwXCountChars', wintypes.DWORD),
                ('dwYCountChars', wintypes.DWORD), ('dwFillAttribute', wintypes.DWORD), ('dwFlags', wintypes.DWORD),
                ('wShowWindow', wintypes.WORD), ('cbReserved2', wintypes.WORD), ('lpReserved2', ctypes.c_void_p),
                ('hStdInput', wintypes.HANDLE), ('hStdOutput', wintypes.HANDLE), ('hStdError', wintypes.HANDLE)]


class PROCESS_INFORMATION(ctypes.Structure):
    _fields_ = [('hProcess', wintypes.HANDLE), ('hThread', wintypes.HANDLE),
                ('dwProcessId', wintypes.DWORD), ('dwThreadId', wintypes.DWORD)]


class IO_COUNTERS(ctypes.Structure):
    _fields_ = [(n, ctypes.c_ulonglong) for n in ('r', 'w', 'o', 'rb', 'wb', 'ob')]


class BASIC_LIMIT(ctypes.Structure):
    _fields_ = [('PerProcessUserTimeLimit', ctypes.c_longlong), ('PerJobUserTimeLimit', ctypes.c_longlong),
                ('LimitFlags', wintypes.DWORD), ('MinimumWorkingSetSize', ctypes.c_size_t),
                ('MaximumWorkingSetSize', ctypes.c_size_t), ('ActiveProcessLimit', wintypes.DWORD),
                ('Affinity', ctypes.c_size_t), ('PriorityClass', wintypes.DWORD), ('SchedulingClass', wintypes.DWORD)]


class EXTENDED_LIMIT(ctypes.Structure):
    _fields_ = [('Basic', BASIC_LIMIT), ('Io', IO_COUNTERS), ('ProcessMemoryLimit', ctypes.c_size_t),
                ('JobMemoryLimit', ctypes.c_size_t), ('PeakProcessMemoryUsed', ctypes.c_size_t),
                ('PeakJobMemoryUsed', ctypes.c_size_t)]


class PMC(ctypes.Structure):
    _fields_ = [('cb', wintypes.DWORD), ('PageFaultCount', wintypes.DWORD)] + [
        (n, ctypes.c_size_t) for n in ('PeakWorkingSetSize', 'WorkingSetSize', 'QuotaPeakPagedPoolUsage',
                                       'QuotaPagedPoolUsage', 'QuotaPeakNonPagedPoolUsage', 'QuotaNonPagedPoolUsage',
                                       'PagefileUsage', 'PeakPagefileUsage', 'PrivateUsage')]


for fn, res, args in [
    ('CreateProcessW', wintypes.BOOL, [wintypes.LPCWSTR, wintypes.LPWSTR, ctypes.c_void_p, ctypes.c_void_p, wintypes.BOOL,
                                        wintypes.DWORD, ctypes.c_void_p, wintypes.LPCWSTR, ctypes.c_void_p, ctypes.c_void_p]),
    ('CreateJobObjectW', wintypes.HANDLE, [ctypes.c_void_p, wintypes.LPCWSTR]),
    ('SetInformationJobObject', wintypes.BOOL, [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]),
    ('QueryInformationJobObject', wintypes.BOOL, [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p]),
    ('AssignProcessToJobObject', wintypes.BOOL, [wintypes.HANDLE, wintypes.HANDLE]),
    ('TerminateJobObject', wintypes.BOOL, [wintypes.HANDLE, wintypes.UINT]),
    ('ResumeThread', wintypes.DWORD, [wintypes.HANDLE]),
    ('CloseHandle', wintypes.BOOL, [wintypes.HANDLE]),
    ('OpenProcess', wintypes.HANDLE, [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]),
    ('WaitForSingleObject', wintypes.DWORD, [wintypes.HANDLE, wintypes.DWORD]),
    ('GetExitCodeProcess', wintypes.BOOL, [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]),
    ('GetProcessTimes', wintypes.BOOL, [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4),
    ('QueryFullProcessImageNameW', wintypes.BOOL, [wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]),
    ('QueryProcessCycleTime', wintypes.BOOL, [wintypes.HANDLE, ctypes.POINTER(ctypes.c_ulonglong)]),
    ('GetProcessAffinityMask', wintypes.BOOL, [wintypes.HANDLE, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]),
]:
    f = getattr(K, fn); f.restype = res; f.argtypes = args
PS.GetProcessMemoryInfo.restype = wintypes.BOOL
PS.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(PMC), wintypes.DWORD]
U.OpenDesktopW.restype = wintypes.HANDLE
U.OpenDesktopW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
U.CreateDesktopW.restype = wintypes.HANDLE
U.CreateDesktopW.argtypes = [wintypes.LPCWSTR, ctypes.c_void_p, ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p]

_DESK = {}
_LAUNCH = threading.Lock()  # inheritable pipe ends of one launch must not leak into a concurrent one


def desktop(name):
    """Open (or create once per tool process) the hidden desktop ``name``; the handle lives as long as the tool.

    Reusing one desktop matters: the desktop compositor keeps state per desktop ever created."""
    if name not in _DESK:
        h = U.OpenDesktopW(name, 0, False, GENERIC_ALL) or U.CreateDesktopW(name, None, None, 0, GENERIC_ALL, None)
        if not h:
            raise OSError(f'hidden desktop {name!r} cannot be opened: {ctypes.get_last_error()}')
        _DESK[name] = h
    return 'winsta0\\' + name


def _ft(f):
    return ((f.dwHighDateTime << 32) | f.dwLowDateTime) / 1e7


def proc_cpu_s(h):
    ft = [wintypes.FILETIME() for _ in range(4)]
    if not K.GetProcessTimes(h, *[ctypes.byref(x) for x in ft]):
        return None
    return _ft(ft[2]) + _ft(ft[3])


def proc_cycles(h):
    c = ctypes.c_ulonglong()
    return c.value if K.QueryProcessCycleTime(h, ctypes.byref(c)) else 0


def proc_mem(h):
    m = PMC(); m.cb = ctypes.sizeof(PMC)
    if not PS.GetProcessMemoryInfo(h, ctypes.byref(m), m.cb):
        return None
    return m


def proc_name(h):
    n = wintypes.DWORD(1024); buf = ctypes.create_unicode_buffer(1024)
    if not K.QueryFullProcessImageNameW(h, 0, buf, ctypes.byref(n)):
        return '?'
    return os.path.basename(buf.value).lower()


def own_affinity():
    a, s = ctypes.c_size_t(), ctypes.c_size_t()
    K.GetProcessAffinityMask(wintypes.HANDLE(-1), ctypes.byref(a), ctypes.byref(s))
    return a.value, s.value


def _env_block(env):
    s = ''.join(f'{k}={v}\0' for k, v in sorted(env.items(), key=lambda kv: kv[0].upper())) + '\0'
    return ctypes.create_unicode_buffer(s, len(s))


def job_pids(buf, n):
    """PIDs of a JOBOBJECT_BASIC_PROCESS_ID_LIST read as ULONG_PTR slots: two DWORD counts share slot 0 (assigned
    low, listed high), the PIDs follow from slot 1. (Reading the count from slot 1 dropped the job's first process -
    the engine itself for one-shot engines - and padded the list with zeros.)"""
    listed = (buf[0] >> 32) & 0xFFFFFFFF
    return [buf[1 + i] for i in range(min(listed, n))]


class Proc:
    """One launched tree. ``stdout``/``stdin`` are binary pipe files when requested."""

    def __init__(self, argv, *, mask, desk=None, env=None, cwd=None, mem_cap=8 << 30, stdin=False, stdout=True, stderr_path=None):
        self.argv = list(argv); self.mask = mask; self.started = time.time(); self.t0 = time.perf_counter()
        self.job = K.CreateJobObjectW(None, None)
        lim = EXTENDED_LIMIT()
        lim.Basic.LimitFlags = JOB_LIMIT_KILL_ON_CLOSE | JOB_LIMIT_AFFINITY | (JOB_LIMIT_JOB_MEMORY if mem_cap else 0)
        lim.Basic.Affinity = mask; lim.JobMemoryLimit = mem_cap or 0
        if not K.SetInformationJobObject(self.job, JOB_EXTENDED_LIMITS, ctypes.byref(lim), ctypes.sizeof(lim)):
            raise OSError(f'job limits: {ctypes.get_last_error()}')
        with _LAUNCH:
            self._spawn(env, cwd, desk, stdin, stdout, stderr_path)

    def _spawn(self, env, cwd, desk, stdin, stdout, stderr_path):
        import msvcrt
        si = STARTUPINFOW(); si.cb = ctypes.sizeof(si); si.dwFlags = STARTF_USESTDHANDLES
        if desk:
            si.lpDesktop = desktop(desk)
        close_after = []
        def child_end(fd):
            h = msvcrt.get_osfhandle(fd); os.set_handle_inheritable(h, True); close_after.append(fd); return h
        self.stdin = self.stdout = None
        if stdin:
            r, w = os.pipe(); si.hStdInput = child_end(r); self.stdin = os.fdopen(w, 'wb', buffering=0)
        else:
            nul = os.open(os.devnull, os.O_RDONLY); si.hStdInput = child_end(nul)
        if stdout:
            r, w = os.pipe(); si.hStdOutput = child_end(w); self.stdout = os.fdopen(r, 'rb')
        else:
            nul = os.open(os.devnull, os.O_WRONLY); si.hStdOutput = child_end(nul)
        if stderr_path:
            fd = os.open(str(stderr_path), os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_BINARY); si.hStdError = child_end(fd)
        else:
            nul = os.open(os.devnull, os.O_WRONLY); si.hStdError = child_end(nul)
        pi = PROCESS_INFORMATION()
        cmd = ctypes.create_unicode_buffer(subprocess.list2cmdline(self.argv))
        envb = _env_block(env) if env is not None else None
        ok = K.CreateProcessW(self.argv[0], cmd, None, None, True, CREATE_SUSPENDED | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
                              envb, str(cwd) if cwd else None, ctypes.byref(si), ctypes.byref(pi))
        err = ctypes.get_last_error()
        for fd in close_after:
            os.close(fd)
        if not ok:
            K.CloseHandle(self.job)
            raise OSError(f'CreateProcess {self.argv[0]}: error {err}')
        if not K.AssignProcessToJobObject(self.job, pi.hProcess):
            err = ctypes.get_last_error(); K.TerminateJobObject(self.job, 1); K.CloseHandle(self.job)
            raise OSError(f'job assignment: {err}')
        K.ResumeThread(pi.hThread); K.CloseHandle(pi.hThread)
        self.handle = pi.hProcess; self.pid = pi.dwProcessId

    def pids(self):
        n = 1024
        buf = (ctypes.c_size_t * (n + 2))()
        if not K.QueryInformationJobObject(self.job, JOB_PID_LIST, buf, ctypes.sizeof(buf), None):
            return []
        return job_pids(buf, n)

    def peak_commit(self):
        lim = EXTENDED_LIMIT()
        K.QueryInformationJobObject(self.job, JOB_EXTENDED_LIMITS, ctypes.byref(lim), ctypes.sizeof(lim), None)
        return lim.PeakJobMemoryUsed

    def poll(self):
        if K.WaitForSingleObject(self.handle, 0) != 0:
            return None
        c = wintypes.DWORD(); K.GetExitCodeProcess(self.handle, ctypes.byref(c)); return c.value

    def wait(self, timeout=None):
        end = None if timeout is None else time.perf_counter() + timeout
        while True:
            c = self.poll()
            if c is not None:
                return c
            if end is not None and time.perf_counter() > end:
                return None
            K.WaitForSingleObject(self.handle, 200)

    def kill(self):
        """Terminate the whole tree (only this job's processes)."""
        K.TerminateJobObject(self.job, 1)

    def close(self):
        for f in (self.stdin, self.stdout):
            try:
                f and f.close()
            except OSError:
                pass
        if self.job:
            K.TerminateJobObject(self.job, 1); K.CloseHandle(self.job); self.job = None
        if self.handle:
            K.CloseHandle(self.handle); self.handle = None


class Tree(threading.Thread):
    """Samples WS (sum over measured processes) every ``period`` s; exact CPU/WS snapshots via ``snap()``."""

    def __init__(self, proc, period=0.1, exclude=HELPERS):
        super().__init__(daemon=True)
        self.p = proc; self.period = period; self.exclude = exclude; self.h = {}; self.names = {}
        self.samples = []; self.stop = threading.Event(); self.lock = threading.Lock(); self.marks = {}

    def _refresh(self):
        for pid in self.p.pids():
            if pid in self.names:
                continue
            h = K.OpenProcess(PROCESS_QUERY_LIMITED | SYNCHRONIZE, False, pid)
            self.names[pid] = proc_name(h) if h else '?'
            if h and self.names[pid] not in self.exclude:
                self.h[pid] = h
            elif h:
                K.CloseHandle(h)

    def snap(self):
        """(t, cpu_s, ws, private, cycles) summed over measured processes; exited ones keep their final CPU."""
        with self.lock:
            self._refresh(); t = time.perf_counter(); cpu = 0.0; ws = pv = cyc = 0
            for pid, h in self.h.items():
                c = proc_cpu_s(h); cpu += c or 0; cyc += proc_cycles(h)
                if K.WaitForSingleObject(h, 0) != 0:
                    m = proc_mem(h)
                    if m:
                        ws += m.WorkingSetSize; pv += m.PrivateUsage
            return t, cpu, ws, pv, cyc

    def run(self):
        while not self.stop.is_set():
            self.samples.append(self.snap())
            self.stop.wait(self.period)

    def mark(self, name):
        s = self.snap(); self.marks[name] = s; self.samples.append(s); return s

    def finish(self):
        self.stop.set(); self.join(2)
        with self.lock:
            peak = {pid: (self.names.get(pid), (proc_mem(h).PeakWorkingSetSize if proc_mem(h) else None)) for pid, h in self.h.items()}
            for h in self.h.values():
                K.CloseHandle(h)
            self.h = {}
        return peak
