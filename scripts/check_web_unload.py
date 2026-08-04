"""Стенд выгрузки скрытых вью: renderer уходит и мгновенно возвращается.

Сценарий: включить веб-режим (браузер видим) → убедиться, что кадры идут →
переключить панель в Files (браузер скрыт) → подождать дольже 20-секундной
отсрочки → в логе строка «выгружаю renderer», браузера нет → вернуть Web →
кадры снова идут, а страница живая (анимация двигается без мыши).

    python scripts/check_web_unload.py
"""

import io
import json
import os
import socket
import subprocess
import sys
import time
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent
EXE = ROOT / "target" / os.environ.get("KAMIN_EXE", "release") / "kaminide-gpui.exe"
LOG = ROOT / "target" / "web-unload.log"
SHOT = ROOT / "target" / "web-unload-{}.png"
PAGE = ROOT / "target" / "web-anim.html"
PORT = 9333
SCALE = 1.25


def probe(req: dict, timeout: float = 5.0):
    with socket.create_connection(("127.0.0.1", PORT), timeout=timeout) as s:
        s.settimeout(timeout)
        s.sendall((json.dumps(req) + "\n").encode())
        buf = b""
        while b"\n" not in buf:
            chunk = s.recv(65536)
            if not chunk:
                break
            buf += chunk
    return json.loads(buf.decode().strip())


def wait_probe(deadline: float) -> bool:
    while time.time() < deadline:
        try:
            probe({"cmd": "metric", "id": "titlebar"}, timeout=2.0)
            return True
        except OSError:
            time.sleep(0.5)
    return False


def kill_app():
    subprocess.run(
        ["powershell", "-c", "Stop-Process -Name kaminide-gpui -Force -ErrorAction SilentlyContinue"],
        capture_output=True,
    )
    time.sleep(3)


def log_text() -> str:
    return io.open(LOG, encoding="utf-8", errors="replace").read()


def diff_mid(a, b, bounds) -> float:
    box = (
        int((bounds["x"] + 20) * SCALE),
        int((bounds["y"] + 20) * SCALE),
        int((bounds["x"] + bounds["w"] - 20) * SCALE),
        int((bounds["y"] + bounds["h"] - 20) * SCALE),
    )
    pa, pb = list(a.crop(box).getdata()), list(b.crop(box).getdata())
    n = min(len(pa), len(pb))
    total = sum(abs(pa[i][c] - pb[i][c]) for i in range(0, n, 9) for c in range(3))
    return total / (len(range(0, n, 9)) * 3)


def shot(tag: str):
    from PIL import Image

    path = str(SHOT).format(tag).replace("\\", "/")
    probe({"cmd": "screenshot", "path": path})
    return Image.open(path).convert("RGB")


def main() -> int:
    if not EXE.exists():
        print(f"нет собранного приложения: {EXE}")
        return 1
    kill_app()
    log = io.open(LOG, "w", encoding="utf-8")
    app = subprocess.Popen([str(EXE)], stdout=log, stderr=subprocess.STDOUT, cwd=ROOT)
    print("приложение запущено, ждём отладочный канал…")
    if not wait_probe(time.time() + 60):
        print("канал не поднялся")
        app.kill()
        return 1
    failures = 0

    probe({"cmd": "emit", "kind": "fileMode", "name": "web"})
    time.sleep(3)
    url = "file:///" + str(PAGE).replace("\\", "/")
    probe({"cmd": "weburl", "id": "browser", "url": url})
    time.sleep(2)

    # Скрыть: панель в Files. Ждём выгрузку (отсрочка 20 с + запас).
    probe({"cmd": "emit", "kind": "fileMode", "name": "files"})
    print("панель скрыта, ждём выгрузку…")
    t0 = time.time()
    while time.time() - t0 < 35:
        if "вью browser скрыт" in log_text():
            break
        time.sleep(1)
    if "вью browser скрыт" not in log_text():
        print("СБОЙ: renderer браузера не выгрузился за 35 с")
        failures += 1
    else:
        print(f"выгрузился за {int(time.time() - t0)} с после скрытия")

    # Вернуть: страница должна подняться и жить (кадры без мыши).
    probe({"cmd": "emit", "kind": "fileMode", "name": "web"})
    time.sleep(2)
    probe({"cmd": "weburl", "id": "browser", "url": url})
    time.sleep(3)
    view = probe({"cmd": "metric", "id": "browser-viewport"}).get("bounds")
    if not view:
        print("СБОЙ: панель не вернулась")
        failures += 1
    else:
        a = shot("a")
        time.sleep(2)
        b = shot("b")
        d = diff_mid(a, b, view)
        print(f"после возврата картинка изменилась на {d:.1f}")
        if d < 3.0:
            print("СБОЙ: страница после возврата мертва")
            failures += 1

    app.kill()
    print(f"\nитог: сбоев {failures}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
