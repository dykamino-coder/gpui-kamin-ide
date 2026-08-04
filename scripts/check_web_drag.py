"""Стенд «кадр в процессе перетаскивания»: ищет тёмные полосы по краям панели.

Зачем: жалоба «при расширении чёрные полосы сверху и снизу, при сужении справа
и слева» видна ТОЛЬКО пока ручка зажата — обычный стенд смотрит уже устоявшийся
кадр и ничего не находит. Здесь ручка держится (`draghold`), снимается кадр, и
только потом отпускается (`dragrelease`).

    python scripts/check_web_drag.py

Проверка: у каждого края панели берётся полоса пикселей и сравнивается с
серединой. Полоса значительно темнее середины = та самая чёрная кромка.
Код возврата 1, если полосы нашлись.
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
LOG = ROOT / "target" / "web-drag.log"
SHOT = ROOT / "target" / "web-drag-{}.png"
PORT = 9333
SCALE = 1.25
# Куда ведём ручку: сначала расширяем панель, потом сужаем.
STEPS = [("расширение", +260), ("сужение", -260)]


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


def shot(tag: str):
    from PIL import Image

    path = str(SHOT).format(tag).replace("\\", "/")
    probe({"cmd": "screenshot", "path": path})
    return Image.open(path).convert("RGB")


def strip_mean(img, box) -> float:
    px = list(img.crop(box).getdata())
    return sum(sum(p) for p in px) / (len(px) * 3) if px else 0.0


def edges(img, b: dict) -> dict:
    """Яркость четырёх кромок панели и её середины."""
    x0, y0 = int(b["x"] * SCALE), int(b["y"] * SCALE)
    x1, y1 = int((b["x"] + b["w"]) * SCALE), int((b["y"] + b["h"]) * SCALE)
    # Отступ 12 физ.px: у карточки браузера есть своя рамка и паддинг слева,
    # страница начинается внутри. Мерить надо ПОЛЕ СТРАНИЦЫ, иначе рамка сама
    # выглядит «чёрной полосой» (первая версия стенда так и ошибалась).
    pad, band = 12, 6
    return {
        "середина": strip_mean(
            img, (x0 + (x1 - x0) // 3, y0 + (y1 - y0) // 3, x0 + 2 * (x1 - x0) // 3, y0 + 2 * (y1 - y0) // 3)
        ),
        "сверху": strip_mean(img, (x0 + 20, y0 + pad, x1 - 20, y0 + pad + band)),
        "снизу": strip_mean(img, (x0 + 20, y1 - pad - band, x1 - 20, y1 - pad)),
        "слева": strip_mean(img, (x0 + pad, y0 + 20, x0 + pad + band, y1 - 20)),
        "справа": strip_mean(img, (x1 - pad - band, y0 + 20, x1 - pad, y1 - 20)),
    }


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
    probe({"cmd": "emit", "kind": "fileMode", "name": "web"})
    time.sleep(4)

    failures = 0
    for name, delta in STEPS:
        view = probe({"cmd": "metric", "id": "browser-viewport"})["bounds"]
        tree = probe({"cmd": "metric", "id": "file-tree"})["bounds"]
        from_x = round((view["x"] + view["w"] + tree["x"]) / 2)
        y = round(view["y"] + view["h"] / 2)
        to_x = from_x + delta
        probe({"cmd": "draghold", "from": [from_x, y], "to": [to_x, y]})
        # Кадр СНИМАЕМ, пока ручка зажата.
        img = shot(name)
        held = probe({"cmd": "metric", "id": "browser-viewport"})["bounds"]
        probe({"cmd": "dragrelease", "at": [to_x, y]})
        time.sleep(1.5)

        # Профиль яркости по горизонтали через середину панели: видно, где
        # именно кончается страница и начинается чернота.
        y_px = int((held["y"] + held["h"] / 2) * SCALE)
        x0, x1 = int(held["x"] * SCALE), int((held["x"] + held["w"]) * SCALE)
        row = [sum(img.getpixel((x, y_px))) // 3 for x in range(max(0, x0 - 30), min(img.width, x1 + 30))]
        print(f"  панель по X {x0}..{x1} физ.px; яркость от {x0 - 30}: {row[:40]}")
        e = edges(img, held)
        mid = e.pop("середина")
        print(f"{name}: панель {held['w']:.0f}×{held['h']:.0f} @{held['x']:.0f},{held['y']:.0f}, середина {mid:.0f}, кромки " +
              ", ".join(f"{k} {v:.0f}" for k, v in e.items()))
        dark = [k for k, v in e.items() if mid > 25 and v < mid * 0.45]
        if dark:
            print(f"СБОЙ: тёмные кромки при {name}: {', '.join(dark)}")
            failures += 1

    app.kill()
    print(f"\nитог: сбоев {failures}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
