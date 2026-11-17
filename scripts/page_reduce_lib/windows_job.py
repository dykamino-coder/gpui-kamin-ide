"""Anonymous job containing only a reducer-owned process and its descendants."""

import ctypes
from ctypes import wintypes
import json
import time


class BasicLimits(ctypes.Structure):
    _fields_ = [("process_time", ctypes.c_int64), ("job_time", ctypes.c_int64),
                ("flags", wintypes.DWORD), ("min_ws", ctypes.c_size_t),
                ("max_ws", ctypes.c_size_t), ("active", wintypes.DWORD),
                ("affinity", ctypes.c_size_t), ("priority", wintypes.DWORD),
                ("scheduling", wintypes.DWORD)]


class IoCounters(ctypes.Structure):
    _fields_ = [(name, ctypes.c_uint64) for name in
                ("read_ops", "write_ops", "other_ops", "read_bytes", "write_bytes", "other_bytes")]


class ExtendedLimits(ctypes.Structure):
    _fields_ = [("basic", BasicLimits), ("io", IoCounters),
                ("process_memory", ctypes.c_size_t), ("job_memory", ctypes.c_size_t),
                ("peak_process", ctypes.c_size_t), ("peak_job", ctypes.c_size_t)]


class Accounting(ctypes.Structure):
    _fields_ = [(name, ctypes.c_int64) for name in
                ("user_time", "kernel_time", "period_user", "period_kernel")] + [
                    (name, wintypes.DWORD) for name in ("faults", "total", "active", "terminated")]


class Job:
    def __init__(self, proc):
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        k = self.kernel
        k.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
        k.CreateJobObjectW.restype = wintypes.HANDLE
        k.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
        k.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
        k.TerminateJobObject.argtypes = [wintypes.HANDLE, wintypes.UINT]
        k.QueryInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int,
                                               ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p]
        k.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        k.OpenProcess.restype = wintypes.HANDLE
        k.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
        k.CloseHandle.argtypes = [wintypes.HANDLE]
        self.handle = k.CreateJobObjectW(None, None)
        limits = ExtendedLimits()
        limits.basic.flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not self.handle or not k.SetInformationJobObject(self.handle, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            error = ctypes.get_last_error()
            self.close()
            raise OSError(error, "cannot establish owned process job")
        if not k.AssignProcessToJobObject(self.handle, int(proc._handle)):
            error = ctypes.get_last_error()
            self.close()
            # It may already have exited; no descendants were launched by our
            # Python timeout test in that case. Other failures are infrastructure.
            if proc.poll() is None:
                raise OSError(error, "cannot assign owned process to job")

    def close(self, owned_log=None):
        if self.handle:
            handles = []
            try:
                capacity = 64
                while True:
                    buffer = ctypes.create_string_buffer(8 + capacity * ctypes.sizeof(ctypes.c_size_t))
                    if self.kernel.QueryInformationJobObject(self.handle, 3, buffer, len(buffer), None):
                        count = ctypes.c_uint32.from_buffer(buffer, 4).value
                        pids = (ctypes.c_size_t * count).from_buffer(buffer, 8)
                        handles = [self.kernel.OpenProcess(0x100000, False, pid) for pid in pids]
                        if owned_log:
                            ownership = json.loads(owned_log.read_text(encoding="utf-8"))
                            ownership["job_descendants"] = list(pids)
                            owned_log.write_text(json.dumps(ownership), encoding="utf-8")
                        break
                    if ctypes.get_last_error() != 234:  # ERROR_MORE_DATA
                        raise ctypes.WinError(ctypes.get_last_error())
                    capacity *= 2
                # Termination is asynchronous. Wait for descendants to release
                # inherited log handles before the caller reads/removes files.
                self.kernel.TerminateJobObject(self.handle, 124)
                deadline = time.monotonic() + 10
                while True:
                    counts = Accounting()
                    if not self.kernel.QueryInformationJobObject(
                            self.handle, 1, ctypes.byref(counts), ctypes.sizeof(counts), None):
                        raise ctypes.WinError(ctypes.get_last_error())
                    if not counts.active:
                        break
                    if time.monotonic() >= deadline:
                        raise TimeoutError("owned process job did not terminate")
                    time.sleep(.01)
                for handle in handles:
                    if handle and self.kernel.WaitForSingleObject(handle, 10000) != 0:
                        raise TimeoutError("owned descendant did not finish process rundown")
            finally:
                for handle in handles:
                    if handle:
                        self.kernel.CloseHandle(handle)
                self.kernel.CloseHandle(self.handle)
                self.handle = None
