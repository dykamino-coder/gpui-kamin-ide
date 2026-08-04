"""Стенд «кадры без мыши»: идёт ли анимация страницы, когда ввода нет.

Зачем: жалоба «после клика анимация двигается только если шевелить мышью» по
логу не видна — надо посчитать кадры и отрисовки за секунды простоя. Страница
своя: секундная CSS-анимация во всю панель, так что каждый кадр отличается от
предыдущего.

    python scripts/check_web_anim.py

Проверяем три вещи, все — с НЕТРОНУТОЙ мышью:
  * Chromium присылает кадры (иначе виноват насос CEF);
  * мы их рисуем (иначе рвётся заказ перерисовки);
  * картинка на экране действительно меняется (иначе рисуем один и тот же).

Код возврата 1, если что-то из этого не сошлось.
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
# По умолчанию отладочная сборка; KAMIN_EXE=release — та, что уходит людям.
EXE = ROOT / "target" / os.environ.get("KAMIN_EXE", "debug") / "kaminide-gpui.exe"
LOG = ROOT / "target" / "web-anim.log"
PAGE = ROOT / "target" / "web-anim.html"
SHOT = ROOT / "target" / "web-anim-{}.png"
PORT = 9333

# Полоса едет через всю панель за секунду — любой соседний кадр отличается.
PAGE_HTML = """<!doctype html><meta charset=utf-8><title>анимация</title>
<style>
  html,body{margin:0;height:100%;background:#101014;overflow:hidden}
  .bar{position:absolute;top:0;bottom:0;width:35%;
       background:linear-gradient(90deg,#ff0080,#00e0ff);
       animation:go 1s linear infinite}
  @keyframes go{from{left:-35%}to{left:100%}}
</style><div class=bar></div>
"""


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
    time.sleep(2)


def tail(n: int = 25) -> str:
    if not LOG.exists():
        return "(лога нет)"
    return "\n".join(io.open(LOG, encoding="utf-8", errors="replace").read().splitlines()[-n:])


def counters() -> list[dict]:
    """Разобрать секундные сводки из лога: кадры, отрисовки, заказы."""
    out = []
    if not LOG.exists():
        return out
    for line in io.open(LOG, encoding="utf-8", errors="replace").read().splitlines():
        if "[cef] за секунду:" not in line:
            continue
        body = line.split("за секунду:", 1)[1]
        row = {}
        for part in body.split(","):
            part = part.strip()
            num = part.rsplit(" ", 1)[-1]
            name = part[: len(part) - len(num)].strip()
            if num.isdigit():
                row[name] = int(num)
        out.append(row)
    return out


def shot(tag: str):
    from PIL import Image

    path = str(SHOT).format(tag).replace("\\", "/")
    probe({"cmd": "screenshot", "path": path})
    return Image.open(path).convert("RGB")


def diff(a, b, bounds: dict, scale: float) -> float:
    """Средняя разница пикселей середины панели между двумя кадрами."""
    box = (
        int((bounds["x"] + 10) * scale),
        int((bounds["y"] + 10) * scale),
        int((bounds["x"] + bounds["w"] - 10) * scale),
        int((bounds["y"] + bounds["h"] - 10) * scale),
    )
    pa, pb = list(a.crop(box).getdata()), list(b.crop(box).getdata())
    n = min(len(pa), len(pb))
    if n == 0:
        return 0.0
    total = sum(abs(pa[i][c] - pb[i][c]) for i in range(0, n, 7) for c in range(3))
    return total / (len(range(0, n, 7)) * 3)


def main() -> int:
    if not EXE.exists():
        print(f"нет собранного приложения: {EXE}")
        return 1
    PAGE.write_text(PAGE_HTML, encoding="utf-8")
    kill_app()
    log = io.open(LOG, "w", encoding="utf-8")
    env = dict(os.environ)
    app = subprocess.Popen([str(EXE)], stdout=log, stderr=subprocess.STDOUT, cwd=ROOT, env=env)
    print("приложение запущено, ждём отладочный канал…")
    if not wait_probe(time.time() + 60):
        print("канал не поднялся\n" + tail())
        app.kill()
        return 1

    probe({"cmd": "emit", "kind": "fileMode", "name": "web"})
    time.sleep(3)
    url = "file:///" + str(PAGE).replace("\\", "/")
    print(f"открываем {url}")
    probe({"cmd": "weburl", "id": "browser", "url": url})
    time.sleep(3)

    view = probe({"cmd": "metric", "id": "browser-viewport"}).get("bounds")
    if not view:
        print("нет региона панели\n" + tail())
        app.kill()
        return 1
    scale = 1.25

    failures = 0
    # Простой: 4 секунды НИ ОДНОГО события ввода.
    before = counters()
    a = shot("a")
    time.sleep(4)
    b = shot("b")
    rows = counters()[len(before) :]
    if not rows:
        print("СБОЙ: приложение не напечатало ни одной секундной сводки")
        failures += 1
    else:
        frames = [r.get("кадров", 0) for r in rows]
        paints = [r.get("отрисовок", 0) for r in rows]
        print(f"за простой: кадров {frames}, отрисовок {paints}")
        if min(frames) < 20:
            print(f"СБОЙ: Chromium присылает мало кадров при простое: {frames}")
            failures += 1
        # По СРЕДНЕМУ: в software-режиме (RDP) загрузка текстуры каждый кадр
        # дороже, единичные провалы секунды не делают анимацию мёртвой.
        if sum(paints) / max(len(paints), 1) < 20:
            print(f"СБОЙ: кадры приходят, а рисуем редко: {paints}")
            failures += 1

    d = diff(a, b, view, scale)
    print(f"картинка за 4 с простоя изменилась на {d:.1f}")
    if d < 3.0:
        print("СБОЙ: на экране одна и та же картинка — анимация не идёт без мыши")
        failures += 1

    busy = sum(r.get("занято", 0) for r in rows)
    mismatch = sum(r.get("не тот размер", 0) for r in rows)
    if busy or mismatch:
        print(f"внимание: кадров занято {busy}, не того размера {mismatch}")

    app.kill()
    print(f"\nитог: сбоев {failures}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
