"""External raw process CPU checkpoints for the engine's schema-1 idle journal."""

import json
import os
from pathlib import Path
import sys
import time
import uuid


def add_parser(sub):
    parser = sub.add_parser("idle", help="observe raw CPU without waking a --time --idle viewer")
    parser.add_argument("--pid", type=int, required=True, help="exact viewer child PID")
    parser.add_argument("--journal", type=Path, required=True, help="the viewer's --why directory")
    parser.add_argument("--seconds", type=float, default=5, help="quiet span, at least five seconds")
    parser.add_argument("--settle", type=float, default=3, help="external settle before CPU checkpoints")
    parser.add_argument("--timeout", type=float, default=120, help="seconds allowed for each handshake")
    parser.add_argument("--quit", action="store_true", help="close the observed viewer afterwards")


def command(args):
    if sys.platform != "win32":
        raise SystemExit("idle requires Windows process counters")
    if args.seconds < 5 or args.settle < 0 or args.timeout <= 0:
        raise SystemExit("idle needs seconds >= 5, settle >= 0 and timeout > 0")
    import ctypes as c
    import ctypes.wintypes as w

    k32 = c.WinDLL("kernel32", use_last_error=True)
    k32.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
    k32.OpenProcess.restype = w.HANDLE
    k32.CloseHandle.argtypes = [w.HANDLE]
    k32.GetProcessTimes.argtypes = [w.HANDLE] + [c.POINTER(w.FILETIME)] * 4
    k32.QueryProcessCycleTime.argtypes = [w.HANDLE, c.POINTER(c.c_ulonglong)]
    k32.QueryPerformanceCounter.argtypes = [c.POINTER(c.c_longlong)]
    k32.QueryPerformanceFrequency.argtypes = [c.POINTER(c.c_longlong)]
    k32.WaitForSingleObject.argtypes = [w.HANDLE, w.DWORD]
    k32.WaitForSingleObject.restype = w.DWORD

    def checked(ok):
        if not ok:
            raise c.WinError(c.get_last_error())

    def ticks(value):
        return (value.dwHighDateTime << 32) | value.dwLowDateTime

    handle = k32.OpenProcess(0x1000 | 0x100000, False, args.pid)
    checked(handle)
    try:
        frequency = c.c_longlong()
        checked(k32.QueryPerformanceFrequency(c.byref(frequency)))

        def sample():
            if k32.WaitForSingleObject(handle, 0) != 0x102:
                raise RuntimeError("observed process exited")
            creation, exit_time, kernel, user = (w.FILETIME() for _ in range(4))
            checked(k32.GetProcessTimes(handle, c.byref(creation), c.byref(exit_time),
                                        c.byref(kernel), c.byref(user)))
            cycles, qpc = c.c_ulonglong(), c.c_longlong()
            checked(k32.QueryProcessCycleTime(handle, c.byref(cycles)))
            checked(k32.QueryPerformanceCounter(c.byref(qpc)))
            return ticks(creation), {"cycles": cycles.value, "qpc": qpc.value,
                                     "kernel_100ns": ticks(kernel), "user_100ns": ticks(user)}

        creation, _ = sample()
        reply = args.journal / "idle-reply.json"
        control = args.journal / "idle-control.json"
        identity = uuid.uuid4().hex

        def wait_reply(state, request_id=None):
            deadline = time.monotonic() + args.timeout
            while time.monotonic() < deadline:
                try:
                    value = json.loads(reply.read_text(encoding="utf-8"))
                except (FileNotFoundError, json.JSONDecodeError):
                    value = {}
                if value.get("pid") != args.pid or value.get("creation_100ns") != creation:
                    if value:
                        raise RuntimeError("journal belongs to a different process")
                elif value.get("state") == state and (request_id is None or value.get("id") == request_id):
                    return
                if k32.WaitForSingleObject(handle, 50) != 0x102:
                    raise RuntimeError("observed process exited before journal reply")
            raise TimeoutError(f"no journal {state} reply")

        def send(op, **fields):
            temporary = control.with_name(f"idle-control-{identity}.tmp")
            temporary.write_text(json.dumps({"id": identity, "op": op, **fields}), encoding="utf-8")
            os.replace(temporary, control)

        wait_reply("ready")
        time.sleep(args.settle)
        send("begin")
        wait_reply("observing", identity)
        time.sleep(args.settle)
        _, start = sample()
        time.sleep(args.seconds)
        _, end = sample()
        send("finish", cpu_raw={"pid": args.pid, "creation_100ns": creation,
                               "qpc_frequency": frequency.value, "start": start, "end": end})
        wait_reply("complete", identity)
        report = json.loads((args.journal / "why.json").read_text(encoding="utf-8"))
        observation = report["idle_samples"][-1]
        print(json.dumps(observation))
        if args.quit:
            send("quit")
        if not observation["quiet"]:
            raise SystemExit("engine journal did not observe zero CPU and zero frame activity")
    finally:
        k32.CloseHandle(handle)
