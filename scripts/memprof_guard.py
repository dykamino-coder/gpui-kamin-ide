"""Runs one engine_bench side of memprof.py so that it can neither outlive its limits nor leave orphans.

The command (usually a launcher chain such as `hlaunch.cmd -> python hidden-desktop-launch.py ->
engine_bench`) starts suspended inside a Job Object of its own with KILL_ON_JOB_CLOSE, and is resumed only
once it is in the job: every process it creates joins the job (a launcher's own job nests inside it).
Killing only the direct child is not enough: a killed cmd.exe leaves its python launcher and the bench
running on the benchmark cores. So on any stop the whole job is terminated, the ids of the processes it
held are named, and the guard waits until the job has no live process.

A side is stopped when
  * it runs longer than `timeout` seconds;
  * a watched file (the allocation stack file) grows past `max_bytes`: a bench that keeps working after its
    report, or an exact probe of a large page, writes stacks without bound;
  * the disk holding the watched file has less than `min_free` bytes free.
A stop raises `Stopped` with a message that names the cause, the limit and the processes terminated.
When the command exits on its own, processes it left behind in the job are terminated as well and named.

On other systems the command runs in its own process group, which is killed the same way.
"""

import ctypes
import os
import shutil
import subprocess
import sys
import time

POLL_SECONDS = 0.25


class Stopped(Exception):
    """A side was stopped by the guard; the message says why and which processes were terminated."""


if sys.platform == "win32":
    import ctypes.wintypes as w

    class _IoCounters(ctypes.Structure):
        _fields_ = [(name, ctypes.c_ulonglong) for name in (
            "ReadOperationCount", "WriteOperationCount", "OtherOperationCount",
            "ReadTransferCount", "WriteTransferCount", "OtherTransferCount")]

    class _BasicLimit(ctypes.Structure):
        _fields_ = [("PerProcessUserTimeLimit", ctypes.c_int64), ("PerJobUserTimeLimit", ctypes.c_int64),
                    ("LimitFlags", w.DWORD), ("MinimumWorkingSetSize", ctypes.c_size_t),
                    ("MaximumWorkingSetSize", ctypes.c_size_t), ("ActiveProcessLimit", w.DWORD),
                    ("Affinity", ctypes.c_size_t), ("PriorityClass", w.DWORD), ("SchedulingClass", w.DWORD)]

    class _ExtendedLimit(ctypes.Structure):
        _fields_ = [("BasicLimitInformation", _BasicLimit), ("IoInfo", _IoCounters),
                    ("ProcessMemoryLimit", ctypes.c_size_t), ("JobMemoryLimit", ctypes.c_size_t),
                    ("PeakProcessMemoryUsed", ctypes.c_size_t), ("PeakJobMemoryUsed", ctypes.c_size_t)]

    class _Accounting(ctypes.Structure):
        _fields_ = [("TotalUserTime", ctypes.c_int64), ("TotalKernelTime", ctypes.c_int64),
                    ("ThisPeriodTotalUserTime", ctypes.c_int64), ("ThisPeriodTotalKernelTime", ctypes.c_int64),
                    ("TotalPageFaultCount", w.DWORD), ("TotalProcesses", w.DWORD),
                    ("ActiveProcesses", w.DWORD), ("TotalTerminatedProcesses", w.DWORD)]

    _k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    _ntdll = ctypes.WinDLL("ntdll")
    _k32.CreateJobObjectW.restype = w.HANDLE
    _k32.CreateJobObjectW.argtypes = [ctypes.c_void_p, w.LPCWSTR]
    _k32.SetInformationJobObject.argtypes = [w.HANDLE, ctypes.c_int, ctypes.c_void_p, w.DWORD]
    _k32.QueryInformationJobObject.argtypes = [w.HANDLE, ctypes.c_int, ctypes.c_void_p, w.DWORD, ctypes.c_void_p]
    _k32.AssignProcessToJobObject.argtypes = [w.HANDLE, w.HANDLE]
    _k32.TerminateJobObject.argtypes = [w.HANDLE, w.UINT]
    _k32.TerminateProcess.argtypes = [w.HANDLE, w.UINT]
    _k32.CloseHandle.argtypes = [w.HANDLE]
    _ntdll.NtResumeProcess.argtypes = [w.HANDLE]
    _ntdll.NtResumeProcess.restype = ctypes.c_long

    _BASIC_ACCOUNTING, _PROCESS_ID_LIST, _EXTENDED_LIMIT = 1, 3, 9
    _KILL_ON_JOB_CLOSE = 0x2000
    _CREATE_SUSPENDED = 0x4

    def _check(ok, what):
        if not ok:
            raise OSError(ctypes.get_last_error(), what)

    class _Tree:
        """The command and everything it starts, in one job."""

        def __init__(self, command, **popen):
            self.job = _k32.CreateJobObjectW(None, None)
            _check(self.job, "CreateJobObject")
            info = _ExtendedLimit()
            info.BasicLimitInformation.LimitFlags = _KILL_ON_JOB_CLOSE
            _check(_k32.SetInformationJobObject(self.job, _EXTENDED_LIMIT, ctypes.byref(info),
                                                ctypes.sizeof(info)), "SetInformationJobObject")
            flags = popen.pop("creationflags", 0) | _CREATE_SUSPENDED
            self.proc = subprocess.Popen(command, creationflags=flags, **popen)
            handle = int(self.proc._handle)
            if not _k32.AssignProcessToJobObject(self.job, handle):
                error = ctypes.get_last_error()
                _k32.TerminateProcess(handle, 1)
                self.proc.wait()
                self.close()
                raise OSError(error, "AssignProcessToJobObject")
            if _ntdll.NtResumeProcess(handle) < 0:
                self.kill()
                raise OSError(0, "NtResumeProcess")

        def pids(self):
            """Ids of the processes in the job now."""
            for _ in range(4):
                count = max(self.active(), 1) + 16
                buf = (ctypes.c_size_t * (count + 1))()
                if _k32.QueryInformationJobObject(self.job, _PROCESS_ID_LIST, buf, ctypes.sizeof(buf), None):
                    listed = ctypes.cast(buf, ctypes.POINTER(w.DWORD))[1]
                    return [int(buf[1 + i]) for i in range(listed)]
            return []

        def active(self):
            info = _Accounting()
            if not _k32.QueryInformationJobObject(self.job, _BASIC_ACCOUNTING, ctypes.byref(info),
                                                  ctypes.sizeof(info), None):
                return 0
            return info.ActiveProcesses

        def kill(self):
            """Terminates every process of the job; returns their ids once none is alive."""
            pids = self.pids()
            if pids:
                _k32.TerminateJobObject(self.job, 1)
            deadline = time.monotonic() + 10
            while self.active() and time.monotonic() < deadline:
                time.sleep(0.05)
            if self.active():
                raise Stopped(f"processes {self.pids()} are still alive 10 s after their job was terminated")
            return pids

        def close(self):
            if self.job:
                _k32.CloseHandle(self.job)  # KILL_ON_JOB_CLOSE: nothing of the job survives this
                self.job = None
else:
    import signal

    class _Tree:
        def __init__(self, command, **popen):
            self.proc = subprocess.Popen(command, start_new_session=True, **popen)

        def pids(self):
            return [self.proc.pid]

        def active(self):
            try:
                os.killpg(self.proc.pid, 0)
                return 1
            except OSError:
                return 0

        def kill(self):
            pids = [self.proc.pid] if self.active() else []
            try:
                os.killpg(self.proc.pid, signal.SIGKILL)
            except OSError:
                pass
            return pids

        def close(self):
            pass


def run(command, *, timeout, watch=None, max_bytes=None, min_free=None, poll=POLL_SECONDS, started=None,
        **popen):
    """Runs `command` under the limits of the module docstring; returns its exit code.

    `started(proc)`, if given, is called once the tree runs (memprof_ws samples the bench from it); the
    limits are checked again as soon as it returns."""
    deadline = time.monotonic() + timeout
    tree = _Tree(command, **popen)
    try:
        if started:
            started(tree.proc)
        while True:
            try:
                code = tree.proc.wait(timeout=poll)
                break
            except subprocess.TimeoutExpired:
                pass
            cause = _over(watch, max_bytes, min_free)
            if cause is None and time.monotonic() >= deadline:
                cause = f"it ran longer than --timeout {timeout} s"
            if cause:
                pids = tree.kill()
                tree.proc.wait()
                raise Stopped(f"{cause}; terminated its process tree (pids {_names(pids)})")
        left = tree.pids() if tree.active() else []
        if left:
            tree.kill()
            print(f"terminated processes left behind by the command (pids {_names(left)})", file=sys.stderr)
        return code
    finally:
        tree.close()


def _over(watch, max_bytes, min_free):
    if not watch:
        return None
    try:
        size = os.path.getsize(watch)
    except OSError:
        size = 0
    if max_bytes is not None and size > max_bytes:
        return (f"{watch} reached {size / 1e9:.2f} GB, over the limit of {max_bytes / 1e9:.2f} GB "
                "(--max-stacks-gb)")
    if min_free is not None:
        free = shutil.disk_usage(os.path.dirname(os.path.abspath(watch))).free
        if free < min_free:
            return (f"free disk space fell to {free / (1 << 30):.1f} GiB, under the floor of "
                    f"{min_free / (1 << 30):.0f} GiB (--min-free-gb), while {watch} was {size / 1e9:.2f} GB")
    return None


def _names(pids):
    return ", ".join(str(pid) for pid in pids) or "none alive"
