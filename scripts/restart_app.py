"""Штатный рестарт gpui-kamin-ide для стендов.

ОБЯЗАТЕЛЬНЫЙ путь вместо Stop-Process -Force: сначала probe flushLayout
(отложенный дебаунс-патч лейаута доезжает на диск), затем graceful WM_CLOSE
(приложение делает свой flush + web::shutdown), фолбэк — kill. Force-kill
между дебаунсами терял хвост изменений лейаута юзера.

    python scripts/restart_app.py            # рестарт
    python scripts/restart_app.py --stop     # только остановить
"""
import json
import socket
import subprocess
import sys
import time
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = Path(__file__).resolve().parent.parent
EXE = ROOT / "target" / "debug" / "kaminide-gpui.exe"


def probe(req, timeout=3.0):
    with socket.create_connection(("127.0.0.1", 9333), timeout=timeout) as s:
        s.settimeout(timeout)
        s.sendall((json.dumps(req) + "\n").encode())
        buf = b""
        while b"\n" not in buf:
            c = s.recv(65536)
            if not c:
                break
            buf += c
    return json.loads(buf.decode().strip())


PS_CLOSE = r"""
Add-Type -Namespace WT -Name U -MemberDefinition '[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l); [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr p); public delegate bool EnumWindowsProc(IntPtr h, IntPtr p); [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid); [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h); [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);'
$ids=(Get-Process -Name kaminide-gpui -ErrorAction SilentlyContinue).Id
$cb=[WT.U+EnumWindowsProc]{ param($h,$p) $pid2=0; [void][WT.U]::GetWindowThreadProcessId($h,[ref]$pid2); if($ids -contains $pid2 -and [WT.U]::IsWindowVisible($h) -and [WT.U]::GetWindowTextLength($h) -gt 0){ [void][WT.U]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }; return $true }
[void][WT.U]::EnumWindows($cb,[IntPtr]::Zero)
"""


def stop():
    try:
        probe({"cmd": "flushLayout"}, timeout=3.0)
        print("layout flushed")
    except OSError:
        print("probe недоступен (приложение не запущено?)")
    subprocess.run(["powershell", "-c", PS_CLOSE], capture_output=True)
    # ждём штатного выхода до 8с, потом добиваем
    for _ in range(40):
        r = subprocess.run(
            ["powershell", "-c", "(Get-Process kaminide-gpui -ErrorAction SilentlyContinue | Measure-Object).Count"],
            capture_output=True, text=True)
        if r.stdout.strip() == "0":
            print("закрылось штатно")
            return
        time.sleep(0.2)
    subprocess.run(["powershell", "-c", "Stop-Process -Name kaminide-gpui -Force -ErrorAction SilentlyContinue"], capture_output=True)
    print("добито force (после flush — потерь лейаута нет)")


def start():
    # DETACHED_PROCESS: приложение НЕ наследует консоль вызывающего шелла.
    # Прежний Start-Process оставлял унаследованные хэндлы — bash-сессия
    # агента висела до таймаута после каждого рестарта («токсичные билды
    # на 6 часов» — жалоба юзера).
    detached = 0x00000008  # DETACHED_PROCESS
    new_group = 0x00000200  # CREATE_NEW_PROCESS_GROUP
    with open(ROOT / "user-run.log", "ab") as out, open(ROOT / "user-run-err.log", "ab") as err:
        subprocess.Popen(
            [str(EXE)],
            cwd=str(ROOT),
            stdout=out,
            stderr=err,
            stdin=subprocess.DEVNULL,
            creationflags=detached | new_group,
        )
    print("запущено")


if __name__ == "__main__":
    stop()
    if "--stop" not in sys.argv:
        start()
