"""Стенд скролла CEF-вью: ловит «прыжки» контента при прокрутке.

Жалоба: при колесе и таскании скроллбара контент прыгает. По логу это не
видно — нужен замер позиции. Страница сама кодирует свой `scrollY` цветом
фиксированного маркера в углу (R = позиция % 256, G = позиция / 256), стенд
крутит колесо через probe и после каждого шага читает маркер со скриншота.

    python scripts/check_web_scroll.py

Сбой = позиция пошла НАЗАД между шагами вниз (прыжок) или маркер не читается.
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
LOG = ROOT / "target" / "web-scroll.log"
PAGE = ROOT / "target" / "web-scroll.html"
SHOT = ROOT / "target" / "web-scroll.png"
PORT = 9333
SCALE = 1.25
STEPS = 14

# Маркер 60×24 в левом верхнем углу: цвет = scrollY (R младший байт, G старший).
# Синий канал 255 — отличаем маркер от случайного фона.
PAGE_HTML = """<!doctype html><meta charset=utf-8><title>скролл</title>
<style>
  body{margin:0;height:6000px;background:repeating-linear-gradient(
    #202030 0 60px, #3a3a52 60px 120px)}
  #m{position:fixed;left:0;top:0;width:60px;height:24px}
</style>
<div id=m></div>
<script>
  const m = document.getElementById('m');
  function paint(){
    const y = Math.round(window.scrollY);
    m.style.background = `rgb(${y % 256}, ${Math.floor(y / 256)}, 255)`;
  }
  addEventListener('scroll', paint, {passive: true});
  paint();
</script>
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
    time.sleep(3)


def marker(view: dict) -> int | None:
    """Прочитать scrollY из цвета маркера. None — маркер не найден.

    До трёх попыток: скриншот, снятый в момент смены кадра, изредка ловит
    рамку/фон вместо маркера — это шум стенда, не приложение.
    """
    from PIL import Image

    for _ in range(3):
        probe({"cmd": "screenshot", "path": str(SHOT).replace("\\", "/")})
        img = Image.open(SHOT).convert("RGB")
        x = min(int((view["x"] + 18) * SCALE), img.width - 1)
        y = min(int((view["y"] + 8) * SCALE), img.height - 1)
        r, g, b = img.getpixel((x, y))
        if b >= 200:
            return g * 256 + r
        time.sleep(0.15)
    return None


def main() -> int:
    if not EXE.exists():
        print(f"нет собранного приложения: {EXE}")
        return 1
    PAGE.write_text(PAGE_HTML, encoding="utf-8")
    kill_app()
    log = io.open(LOG, "w", encoding="utf-8")
    app = subprocess.Popen([str(EXE)], stdout=log, stderr=subprocess.STDOUT, cwd=ROOT)
    print("приложение запущено, ждём отладочный канал…")
    if not wait_probe(time.time() + 60):
        print("канал не поднялся")
        app.kill()
        return 1
    probe({"cmd": "emit", "kind": "fileMode", "name": "web"})
    time.sleep(3)
    url = "file:///" + str(PAGE).replace("\\", "/")
    view = probe({"cmd": "metric", "id": "browser-viewport"}).get("bounds")
    if not view:
        print("нет региона панели")
        app.kill()
        return 1
    # Навигация с проверкой: пока маркер не синий (scrollY=0), страница не наша.
    for _ in range(4):
        probe({"cmd": "weburl", "id": "browser", "url": url})
        time.sleep(2.5)
        if marker(view) == 0:
            break
    else:
        print(f"страница не загрузилась: маркер {marker(view)}")
        app.kill()
        return 1
    cx_ = round(view["x"] + view["w"] / 2)
    cy_ = round(view["y"] + view["h"] / 2)

    failures = 0
    positions: list[int] = []
    # ВНИЗ: позиция обязана не убывать между шагами.
    for i in range(STEPS):
        probe({"cmd": "scroll", "x": cx_, "y": cy_, "lines": -4})
        time.sleep(0.35)
        pos = marker(view)
        if pos is None:
            print(f"шаг {i}: маркер не читается")
            failures += 1
            continue
        if positions and pos + 4 < positions[-1]:
            print(f"СБОЙ: прыжок НАЗАД на шаге {i}: {positions[-1]} → {pos}")
            failures += 1
        positions.append(pos)
    print(f"позиции вниз: {positions}")
    if positions and positions[-1] <= positions[0]:
        print("СБОЙ: скролл вниз не двигает страницу")
        failures += 1

    app.kill()
    print(f"\nитог: сбоев {failures}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
