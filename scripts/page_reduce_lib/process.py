"""Bounded launches. Only handles/PIDs obtained from this launch are terminated."""

import ctypes
import json
import os
from pathlib import Path
import re
import shlex
import signal
import subprocess
import time


def prefix(value):
    # Match memprof's quoted launcher-prefix interface, preserving Windows paths.
    return [part.strip('"') for part in shlex.split(value, posix=os.name != "nt")]


def run(command, directory, timeout, env=None, ready=None, ready_grace=0):
    """Wait for exit or an explicit completed capture, then reap owned processes."""
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    stdout = directory / "stdout.txt"
    stderr = directory / "stderr.txt"
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    child_handle, job = None, None
    kernel = None
    with stdout.open("wb") as output, stderr.open("wb") as errors:
        proc = subprocess.Popen(command, cwd=directory, env=env, stdout=output,
                                stderr=errors, stdin=subprocess.DEVNULL,
                                creationflags=flags, start_new_session=os.name != "nt")
        ownership = {"pid": proc.pid, "command": command, "child": None}
        pidlog = directory / "owned-process.json"
        pidlog.write_text(json.dumps(ownership), encoding="utf-8")
        deadline = time.monotonic() + timeout
        timed_out, captured, completed_at = False, False, None
        try:
            if os.name == "nt":
                from .windows_job import Job
                job = Job(proc)
            while proc.poll() is None:
                if ready is not None and ready():
                    completed_at = completed_at or time.monotonic()
                    if time.monotonic() - completed_at >= ready_grace:
                        captured = True
                        break
                if ownership["child"] is None:
                    header = stdout.read_bytes().split(b"\n", 1)[0]
                    match = re.fullmatch(rb"Hidden desktop: perfcorpus-bench-\d+ own child: (\d+)\r?", header)
                    if match:
                        ownership["child"] = int(match[1])
                        pidlog.write_text(json.dumps(ownership), encoding="utf-8")
                        if os.name == "nt":
                            kernel = ctypes.WinDLL("kernel32", use_last_error=True)
                            kernel.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
                            kernel.OpenProcess.restype = ctypes.c_void_p
                            kernel.TerminateProcess.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
                            kernel.CloseHandle.argtypes = [ctypes.c_void_p]
                            # Hold the handle now, so a reused PID cannot be killed later.
                            child_handle = kernel.OpenProcess(0x100001, False, ownership["child"])
                if time.monotonic() >= deadline:
                    timed_out = True
                    break
                time.sleep(0.02)
        finally:
            # Closing our anonymous job terminates only descendants of the
            # recorded root, including a memprof-launched engine or Chrome child.
            try:
                if job:
                    job.close(pidlog)
            finally:
                if proc.poll() is None:
                    if child_handle:
                        kernel.TerminateProcess(child_handle, 124)
                    if os.name == "nt":
                        proc.kill()
                    else:
                        os.killpg(proc.pid, signal.SIGKILL)
                proc.wait(timeout=10)
                if os.name == "nt":
                    # Releasing the process object also releases inherited log
                    # handles retained during Windows process rundown.
                    proc._handle.Close()
                if child_handle:
                    kernel.CloseHandle(child_handle)
    code = None if timed_out else (0 if captured else proc.returncode)
    return code, stdout.read_bytes(), stderr.read_bytes()
