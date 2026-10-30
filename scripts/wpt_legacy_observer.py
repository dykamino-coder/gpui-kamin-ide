"""Аттестация старого baseline через Frida: только собственный suspended child."""

import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import struct
import sys
import threading
import time

# Поддерживается и embedded Python, который не добавляет каталог скрипта в sys.path.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from wpt_context import InfrastructureError, require, sha, sidecar
from wpt_runner import atomic_json


def version(path):
    api = ctypes.windll.version
    size = api.GetFileVersionInfoSizeW(str(path), None)
    require(size > 0, f"UMD file version missing: {path}")
    data = ctypes.create_string_buffer(size)
    require(api.GetFileVersionInfoW(str(path), 0, size, data), "GetFileVersionInfo failed")
    pointer, length = ctypes.c_void_p(), wintypes.UINT()
    require(api.VerQueryValueW(data, "\\", ctypes.byref(pointer), ctypes.byref(length)), "VerQueryValue failed")
    words = struct.unpack("13I", ctypes.string_at(pointer, 52))
    return f"{words[2] >> 16}.{words[2] & 65535}.{words[3] >> 16}.{words[3] & 65535}"


def drivers(modules):
    rows = []
    for module in modules:
        name = module["name"]
        if name.startswith(("igd", "nvwgf", "atid", "amdxx")) or name == "d3d10warp.dll":
            rows.append(dict(module, sha256=sha(module["path"]), version=version(module["path"])))
    require(bool(rows), "No loaded GPU UMD")
    return sorted(rows, key=lambda row: row["name"])


def observe(exe, pair_list):
    import frida  # Необязательный инструмент аттестации старых бинарников.

    exe, pair_list = Path(exe).resolve(), Path(pair_list).resolve()
    report = Path(os.environ["WPT_REPORT"])
    high, low = os.environ["WPT_GPU"][5:].split(":")
    high = int(high, 16)
    resolved = {"high_part": high if high < 2**31 else high - 2**32, "low_part": int(low, 16)}
    started = int(time.time() * 1000)
    frames, errors, modules, generations, selections = [], [], [], {}, []
    done = threading.Event()
    device = frida.get_local_device()
    pid = device.spawn([str(exe), str(pair_list)], env=dict(os.environ), cwd=os.getcwd(), stdio="pipe")
    api = ctypes.windll.kernel32
    api.OpenProcess.restype = wintypes.HANDLE
    api.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
    api.CloseHandle.argtypes = [wintypes.HANDLE]
    handle = api.OpenProcess(0x100000 | 0x1000, False, pid)
    if not handle:
        device.kill(pid)
        require(False, "cannot observe own child exit")
    session = None
    manifest = {"schema": 1, "complete": False, "run_id": f"{pid}-{started}", "pid": pid,
                "process_start_ms": started, "exe": {"path": str(exe), "basename": exe.name, "sha256": sha(exe)},
                "source_revision": os.environ.get("WPT_BASE_REVISION", "external-baseline"),
                "boot_session": os.environ["WPT_GPU_SESSION"], "list_sha256": sha(pair_list),
                "requested": {"selector": os.environ["WPT_GPU"], "strict": True,
                              "fallback_reason": None, "resolved": resolved},
                "capture": {"width": 800, "height": 600, "scale_factor": 1., "path": "gpu-readback",
                            "stage": "before-present", "text_mode": "grayscale", "color_space": "raw-unorm-no-colorspace-transform",
                            "animation_time_ms": int(os.environ["WPT_ANIMATION_TIME_MS"]),
                            "animation_frames": os.environ.get("WPT_ANIMATION_FRAMES")},
                "shader_policy": {"debug_assertions": True, "backend": "d3dcompiler"},
                "frames": frames, "generations": generations, "pngs": [], "report_sha256": None}
    atomic_json(sidecar(report), manifest)
    device.on("output", lambda child, fd, data: (sys.stdout.buffer.write(data), sys.stdout.buffer.flush()) if child == pid else None)

    def message(value, data):
        try:
            if value["type"] == "error":
                errors.append(value.get("stack", value)); return
            event = value["payload"]
            if event["event"] == "strict-selector":
                require(event["resolved"] == resolved, "legacy selector changed")
                selections.append(event["resolved"])
            elif event["event"] == "device":
                # Модули читаются, пока child жив; snapshots связываются с его поколениями.
                modules[:] = event["modules"]
            elif event["event"] == "capture":
                event.pop("event")
                event["sequence"] = len(frames) + 1
                frames.append(event)
                key = str(event["context"]["device"]["generation"])
                if key not in generations:
                    generations[key] = {"device": event["context"]["device"], "driver_dlls": drivers(modules),
                                        "shader_executables": event["context"]["shader_executables"]}
        except Exception as error:
            errors.append(str(error))

    try:
        session = device.attach(pid)
        session.on("detached", lambda *args: done.set())
        source = "const requested=" + json.dumps(resolved) + ";\n" + Path(__file__).with_suffix(".js").read_text(encoding="utf-8")
        script = session.create_script(source)
        script.on("message", message)
        script.load()
        device.resume(pid)
        require(done.wait(850), "legacy child timed out")
        status = wintypes.DWORD()
        require(api.GetExitCodeProcess(handle, ctypes.byref(status)) and status.value == 0,
                f"legacy child did not exit successfully: {status.value}")
        require(not errors, f"legacy observer errors: {errors}")
        require(bool(selections), "legacy strict selector did not run")
        require(bool(frames), "no observed GPU readback")
        manifest.update(driver_dlls=drivers(modules), report_sha256=sha(report), complete=True)
        atomic_json(sidecar(report), manifest)
    finally:
        if not done.is_set():
            device.kill(pid)  # Только созданный здесь PID.
        api.CloseHandle(handle)


if __name__ == "__main__":
    try:
        observe(*sys.argv[1:])
    except (InfrastructureError, OSError, ValueError, ImportError) as error:
        print(f"INCOMPARABLE_GPU_CONTEXT: {error}", file=sys.stderr)
        sys.exit(2)
