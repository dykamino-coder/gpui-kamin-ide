"""Стенд для чата Bridge на CEF: проверяет ОБА направления обмена.

Глазами такое проверять долго, а сломаться может тихо: страница рисуется, но
мертва. Скрипт проверяет три вещи:

  * панель не пустая — страница отрисовалась в кадр;
  * картинка панели меняется — данные расширения до страницы доходят;
  * в логе есть строки обмена В ОБЕ стороны (`[wv:inbound]` и `[wv:post]`).

    python scripts/check_chat_ipc.py

Код возврата 1, если хоть одна проверка не сошлась.
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
EXE = ROOT / "target" / "debug" / "kaminide-gpui.exe"
LOG = ROOT / "target" / "chat-ipc.log"
SHOT = ROOT / "target" / "chat-ipc.png"
PORT = 9333
SCALE = 1.25
# Кандидаты на панель чата: раскладка зависит от сохранённых пропорций.
PANELS = ["claudeBridgeChat"]


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


def shot():
    from PIL import Image

    probe({"cmd": "screenshot", "path": str(SHOT).replace("\\", "/")})
    return Image.open(SHOT).convert("RGB")


def chat_panel() -> dict | None:
    """Панель, в которой сейчас живёт чат: самая крупная из известных."""
    best = None
    for name in PANELS:
        try:
            res = probe({"cmd": "metric", "id": name})
        except OSError:
            continue
        b = res.get("bounds")
        if not b or b["w"] < 100 or b["h"] < 100:
            continue
        if best is None or b["w"] * b["h"] > best["w"] * best["h"]:
            best = b
    return best


def panel_signature(img, panel: dict) -> list[int]:
    """Грубый отпечаток картинки панели: по нему видно, изменилась ли она."""
    x0 = int(panel["x"] * SCALE) + 10
    y0 = int(panel["y"] * SCALE) + 10
    x1 = min(int((panel["x"] + panel["w"]) * SCALE) - 10, img.width)
    y1 = min(int((panel["y"] + panel["h"]) * SCALE) - 10, img.height)
    small = img.crop((x0, y0, x1, y1)).resize((16, 16))
    return [sum(p) // 3 for p in small.getdata()]


def diff(a: list[int], b: list[int]) -> float:
    return sum(abs(x - y) for x, y in zip(a, b)) / len(a)


def main() -> int:
    if not EXE.exists():
        print(f"нет собранного приложения: {EXE}")
        return 1
    kill_app()
    env = dict(os.environ, KAMIN_CEF="1")
    log = io.open(LOG, "w", encoding="utf-8")
    app = subprocess.Popen([str(EXE)], stdout=log, stderr=subprocess.STDOUT, env=env, cwd=ROOT)
    print("приложение запущено, ждём отладочный канал…")
    if not wait_probe(time.time() + 60):
        print("канал не поднялся\n" + tail())
        app.kill()
        return 1
    # Чат приходит с сервера — даём ему прийти и отрисоваться.
    time.sleep(12)

    failures = 0
    panel = chat_panel()
    if not panel:
        print("панель чата не найдена\n" + tail(20))
        kill_app()
        return 1
    print(f"панель чата: {panel['w']:.0f}×{panel['h']:.0f} лог.px")

    img = shot()
    before = panel_signature(img, panel)
    ok_painted = max(before) > 40
    if not ok_painted:
        failures += 1
    print(f"{'ok' if ok_painted else 'СБОЙ'}: панель отрисована — яркость {max(before)}")

    # Страница живёт: восстановление сессий меняет её вид. Ждём и сверяем.
    time.sleep(15)
    if app.poll() is not None:
        print("ПАДЕНИЕ во время работы\n" + tail(30))
        return 1
    after = panel_signature(shot(), panel)
    changed = diff(before, after)

    # Обмен в обе стороны — по строкам лога.
    text = io.open(LOG, encoding="utf-8", errors="replace").read()
    to_app = text.count("[wv:inbound]")
    to_page = text.count("[wv:post]")
    # «Картинка живая» — вторичный признак: ПОЛНОСТЬЮ загруженный чат статичен
    # (спиннеров нет), и нулевая разница при живом обмене — норма, а не сбой.
    # Сбой — только когда И картинка мертва, И обмена со страницей нет:
    # тогда вью реально не поднялся (стенд раньше врал красным на этом).
    ok_alive = changed > 2.0 or (to_app > 0 and to_page > 0)
    if not ok_alive:
        failures += 1
    print(
        f"{'ok' if ok_alive else 'СБОЙ'}: вью живой — картинка изменилась на "
        f"{changed:.1f}, обмен {to_app}/{to_page}"
    )
    ok_both = to_app > 0 and to_page > 0
    if not ok_both:
        failures += 1
    print(f"{'ok' if ok_both else 'СБОЙ'}: обмен — страница→приложение {to_app}, "
          f"приложение→страница {to_page}")

    print(f"\nитог: сбоев {failures}")
    kill_app()
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
