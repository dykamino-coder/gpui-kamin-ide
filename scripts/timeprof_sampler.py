"""Stack sampler of a running engine_bench, from outside the process (timeprof.py run).

No administrator rights and no change to the measured code. Kernel sampling through ETW (xperf, WPR,
samply) needs the system-profile privilege, which a normal user does not have; suspending a thread
of one's own process and reading it does not.

Every `interval` the sampler suspends a thread of the target, copies its registers (GetThreadContext)
and the used part of its stack (one ReadProcessMemory), and resumes it: about 0.1 ms stopped. The
stack is then walked on the copy (scripts/timeprof_unwind.py) while the target runs.

What one sample means:
  - the main thread (the first thread of the process: window, layout, paint) is sampled at every tick,
    running or waiting, so its samples add up to wall time: a wait for the GPU or for the present shows
    as time in the waiting call;
  - other threads are sampled only when they used the CPU since the previous tick;
  - a sample weighs the time the target ran since the previous tick (the time it stood suspended under
    the sampler is not counted), and carries the CPU time its thread used in that interval, from the
    thread's cycle counter (QueryThreadCycleTime). The run takes longer under the sampler, its shape
    stays the same.

The bench publishes the measured frame and phase in two counters; their address comes from the file
named by ENGINE_BENCH_PROFILE_STATE, and they are read while the main thread is suspended, so a sample
never straddles a phase boundary.

Output, one text file (leaf frame first, hex addresses):
  # engine_bench cpu samples v1 base=<image base> interval_us=<n> phases=<a,b,..>
  s <thread id> <frame> <phase index or -> <weight us> <cpu us> <pc> <return address>...
  m <base> <size> <image path>          modules, for the frames outside the exe
  # end samples=<n> run_us=<n> stopped_us=<n> main=<thread id> truncated=<n>
"""

import ctypes as c
import ctypes.wintypes as w
import time

from timeprof_unwind import Stack, Unwinder

PROCESS_ACCESS = 0x0400 | 0x0010 | 0x100000  # QUERY_INFORMATION | VM_READ | SYNCHRONIZE
THREAD_ACCESS = 0x0002 | 0x0008 | 0x0040  # SUSPEND_RESUME | GET_CONTEXT | QUERY_INFORMATION
CONTEXT_SIZE, CONTEXT_FLAGS_AT, CONTEXT_CONTROL_INTEGER = 1232, 0x30, 0x100003
RAX_AT, RSP_AT, RIP_AT = 0x78, 0x98, 0xF8
IDLE_MICROSECONDS = 30

k32 = c.WinDLL("kernel32", use_last_error=True)
ntdll = c.WinDLL("ntdll")
psapi = c.WinDLL("psapi", use_last_error=True)
k32.OpenProcess.restype = w.HANDLE
k32.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
k32.CloseHandle.argtypes = [w.HANDLE]
k32.WaitForSingleObject.argtypes = [w.HANDLE, w.DWORD]
k32.SuspendThread.argtypes = [w.HANDLE]
k32.SuspendThread.restype = w.DWORD
k32.ResumeThread.argtypes = [w.HANDLE]
k32.GetThreadContext.argtypes = [w.HANDLE, c.c_void_p]
k32.GetThreadId.argtypes = [w.HANDLE]
k32.GetThreadId.restype = w.DWORD
k32.QueryThreadCycleTime.argtypes = [w.HANDLE, c.POINTER(c.c_uint64)]
k32.GetCurrentThread.restype = w.HANDLE
k32.ReadProcessMemory.argtypes = [w.HANDLE, c.c_void_p, c.c_void_p, c.c_size_t, c.POINTER(c.c_size_t)]
ntdll.NtGetNextThread.argtypes = [w.HANDLE, w.HANDLE, w.DWORD, w.ULONG, w.ULONG, c.POINTER(w.HANDLE)]
ntdll.NtQueryInformationThread.argtypes = [w.HANDLE, c.c_int, c.c_void_p, w.ULONG, c.c_void_p]
psapi.EnumProcessModulesEx.argtypes = [w.HANDLE, c.c_void_p, w.DWORD, c.POINTER(w.DWORD), w.DWORD]
psapi.GetModuleInformation.argtypes = [w.HANDLE, c.c_void_p, c.c_void_p, w.DWORD]
psapi.GetModuleFileNameExW.argtypes = [w.HANDLE, c.c_void_p, w.LPWSTR, w.DWORD]


def cycles_per_microsecond():
    """Rate of the thread cycle counter, measured on this thread over a short spin."""
    cycles, thread = c.c_uint64(), k32.GetCurrentThread()
    k32.QueryThreadCycleTime(thread, c.byref(cycles))
    first, started = cycles.value, time.perf_counter()
    while time.perf_counter() - started < 0.02:
        pass
    elapsed = time.perf_counter() - started
    k32.QueryThreadCycleTime(thread, c.byref(cycles))
    return max((cycles.value - first) / (elapsed * 1e6), 1.0)


def aligned(size, alignment=16):
    """A zeroed buffer of `size` bytes at an address aligned as CONTEXT requires; (owner, address)."""
    owner = c.create_string_buffer(size + alignment)
    address = (c.addressof(owner) + alignment - 1) & ~(alignment - 1)
    return owner, address


class Sampler:
    def __init__(self, pid, interval, state=0, all_threads=True, stack_bytes=256 << 10):
        self.interval, self.state, self.all_threads = interval, state, all_threads
        self.process = k32.OpenProcess(PROCESS_ACCESS, False, pid)
        if not self.process:
            raise OSError(c.get_last_error(), f"OpenProcess {pid}")
        self.threads = {}  # thread id -> [handle, cycles at the previous tick, stack base]
        self.main = None
        self.modules = {}
        self.unwinder = Unwinder()
        self.context_owner, self.context = aligned(CONTEXT_SIZE)
        self.registers = (c.c_uint64 * 16).from_address(self.context + RAX_AT)
        self.copy = c.create_string_buffer(stack_bytes)
        self.words = (c.c_uint64 * 2)()
        self.got = c.c_size_t()
        self.samples = self.truncated = 0
        self.rate = cycles_per_microsecond()
        self.stopped = 0.0

    def close(self):
        for entry in self.threads.values():
            k32.CloseHandle(entry[0])
        k32.CloseHandle(self.process)

    def alive(self):
        return k32.WaitForSingleObject(self.process, 0) == 0x102  # WAIT_TIMEOUT

    def read(self, address, buffer, size):
        """Bytes read from the target; a read that crosses into unmapped memory fails as a whole."""
        if k32.ReadProcessMemory(self.process, address, buffer, size, c.byref(self.got)):
            return self.got.value
        return 0

    def stack_base(self, thread):
        """The high end of a thread's stack (TEB.StackBase)."""
        basic = (c.c_uint64 * 6)()  # THREAD_BASIC_INFORMATION; TebBaseAddress is the second word
        if ntdll.NtQueryInformationThread(thread, 0, basic, 48, None) or not basic[1]:
            return 0
        return self.words[0] if self.read(basic[1] + 8, self.words, 8) == 8 else 0

    def refresh_threads(self):
        handle, seen, current = w.HANDLE(), set(), None
        while ntdll.NtGetNextThread(self.process, current, THREAD_ACCESS, 0, 0, c.byref(handle)) == 0:
            current = handle.value
            thread = k32.GetThreadId(current)
            seen.add(thread)
            if thread in self.threads:
                previous = self.threads[thread][0]
                self.threads[thread][0] = current
                k32.CloseHandle(previous)
            else:
                self.threads[thread] = [current, 0, self.stack_base(current)]
                if self.main is None:
                    self.main = thread  # threads are listed in creation order
            handle = w.HANDLE()
        for thread in set(self.threads) - seen:
            k32.CloseHandle(self.threads.pop(thread)[0])

    def refresh_modules(self):
        handles, needed = (c.c_void_p * 1024)(), w.DWORD()
        if not psapi.EnumProcessModulesEx(self.process, handles, c.sizeof(handles), c.byref(needed), 3):
            return
        info, name = (c.c_void_p * 3)(), c.create_unicode_buffer(1024)
        for module in handles[:needed.value // c.sizeof(c.c_void_p)]:
            if module in self.modules or not psapi.GetModuleInformation(self.process, module, info, c.sizeof(info)):
                continue
            size = (info[1] or 0) & 0xFFFFFFFF  # MODULEINFO: base, SizeOfImage (32 bits), entry point
            psapi.GetModuleFileNameExW(self.process, module, name, 1024)
            self.modules[module] = (size, name.value)
            self.unwinder.add(name.value, module, size)

    def position(self):
        """(frame, phase) the bench is in; phase None outside measured frames or without the counters."""
        if not self.state or self.read(self.state, self.words, 16) != 16:
            return 0, None
        return self.words[0], (None if self.words[1] >= 1 << 32 else self.words[1])

    def capture(self, entry, first):
        """Registers, program counter and stack copy of one thread, taken while it is suspended."""
        handle, _, base = entry
        c.c_uint32.from_address(self.context + CONTEXT_FLAGS_AT).value = CONTEXT_CONTROL_INTEGER
        if k32.SuspendThread(handle) == 0xFFFFFFFF:
            return None
        started = time.perf_counter()
        try:
            if not k32.GetThreadContext(handle, self.context):
                return None
            position = self.position() if first else None
            rsp = self.registers[4]
            size = min(base - rsp, len(self.copy)) if base > rsp else 4096
            size = self.read(rsp, self.copy, size) or self.read(rsp, self.copy, min(size, 4096 - rsp % 4096))
        finally:
            k32.ResumeThread(handle)
            self.stopped += time.perf_counter() - started
        cut = base > rsp and size < base - rsp
        pc = c.c_uint64.from_address(self.context + RIP_AT).value
        return list(self.registers), pc, Stack(rsp, self.copy.raw[:size]), position, cut

    def tick(self, weight, out):
        """One sample of the main thread and of every other thread that ran since the previous tick."""
        cycles = c.c_uint64()
        position = (0, None)
        order = [self.main] + ([t for t in self.threads if t != self.main] if self.all_threads else [])
        for thread in order:
            entry = self.threads.get(thread)
            if entry is None or not k32.QueryThreadCycleTime(entry[0], c.byref(cycles)):
                continue
            # A thread that only served the previous suspension used a few microseconds: not running.
            cpu = min(round((cycles.value - entry[1]) / self.rate), weight) if entry[1] else 0
            entry[1] = cycles.value
            if cpu < IDLE_MICROSECONDS and thread != self.main:
                continue
            taken = self.capture(entry, thread == self.main)
            if taken is None:
                continue
            registers, pc, stack, seen, cut = taken
            if seen is not None:
                position = seen
            pcs = self.unwinder.walk(registers, pc, stack)
            if self.unwinder.image(pcs[-1]) is None and len(pcs) < 3:
                self.refresh_modules()  # an image loaded since the last look
                pcs = self.unwinder.walk(registers, pc, stack)
            self.truncated += cut
            phase = "-" if position[1] is None else position[1]
            out.write(f"s {thread} {position[0]} {phase} {weight} {cpu} "
                      + " ".join(format(pc, "x") for pc in pcs) + "\n")
            self.samples += 1

    def run(self, out):
        """Samples until the process exits; returns (samples, microseconds the target ran)."""
        self.refresh_threads()
        self.refresh_modules()
        total = 0
        resumed = time.perf_counter()
        due = resumed + self.interval
        refreshed = resumed
        while self.alive():
            delay = due - time.perf_counter()
            if delay > 0:
                time.sleep(delay)
            now = time.perf_counter()
            due = max(due + self.interval, now + self.interval / 2)
            before = self.stopped
            self.tick(max(round((now - resumed) * 1e6), 1), out)
            total += max(round((now - resumed) * 1e6), 1)
            # The target ran while the stacks were walked: only the suspensions are taken out.
            resumed = now + (self.stopped - before)
            if now - refreshed > 0.25:
                self.refresh_threads()
                self.refresh_modules()
                refreshed = now
        for base, (size, name) in sorted(self.modules.items()):
            out.write(f"m {base:x} {size:x} {name}\n")
        out.write(f"# end samples={self.samples} run_us={total} stopped_us={round(self.stopped * 1e6)} "
                  f"main={self.main} truncated={self.truncated}\n")
        return self.samples, total
