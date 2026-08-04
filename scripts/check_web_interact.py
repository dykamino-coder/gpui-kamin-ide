"""Стенд интерактивности OSR: клавиатура, фокус, попап select, меню, скачивание.

Страница `web_interact.html` кодирует состояние цветом маркеров (обратного
канала у probe к странице нет):
  m-len   (справа, y=20)  — rgb(len*10, 42, 255): длина текста в инпуте;
  m-key   (справа, y=70)  — rgb(keyCode, 200, 255): последний keydown;
  m-focus (справа, y=120) — зелёный: фокус у инпута, красный: нет.

Проверки:
  1. клик в инпут → маркер фокуса зелёный;
  2. webkey a/b/c → len 3; кириллица «ф» → len 4; backspace → len 3;
  3. enter → m-key = rgb(13,200,255);
  4. Ctrl+A, Ctrl+C → в буфере обмена текст инпута (клавиши дошли до Chromium);
  5. клик по select → попап рисуется (пиксели ниже select меняются);
  6. клик по download → системный диалог «Сохранить как» (класс #32770) → Esc;
  7. клик мимо вью (в наш UI) → маркер фокуса красный (blur дошёл).

    python scripts/check_web_interact.py
"""

import ctypes
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
EXE = ROOT / "target" / os.environ.get("KAMIN_EXE", "debug") / "kaminide-gpui.exe"
LOG = ROOT / "target" / "web-interact.log"
SHOT = ROOT / "target" / "web-interact-{}.png"
PAGE = ROOT / "scripts" / "web_interact.html"
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


def shot(tag: str):
    from PIL import Image

    path = str(SHOT).format(tag).replace("\\", "/")
    probe({"cmd": "screenshot", "path": path})
    return Image.open(path).convert("RGB")


def marker(img, view, my: float):
    """Средний цвет маркера 40x40 у правого края вью на высоте my (CSS px)."""
    x0 = int((view["x"] + view["w"] - 60 + 10) * SCALE)
    y0 = int((view["y"] + my + 10) * SCALE)
    px = [img.getpixel((x0 + dx, y0 + dy)) for dx in range(0, 20, 5) for dy in range(0, 20, 5)]
    n = len(px)
    return tuple(sum(c[i] for c in px) // n for i in range(3))


def close_to(rgb, want, tol=25) -> bool:
    return all(abs(rgb[i] - want[i]) <= tol for i in range(3))


def click_view(view, vx: float, vy: float):
    """Настоящий клик мышью в точку вью (через click главного окна)."""
    probe({"cmd": "click", "x": view["x"] + vx, "y": view["y"] + vy})


def clipboard() -> str:
    r = subprocess.run(
        ["powershell", "-c", "Get-Clipboard"], capture_output=True, text=True, encoding="utf-8"
    )
    return (r.stdout or "").strip()


def find_dialog() -> int:
    return ctypes.windll.user32.FindWindowW("#32770", None)


def press_esc():
    # Диалог и меню — нативные окна с захватом ввода: Esc только настоящим.
    for updown in (0, 2):
        ctypes.windll.user32.keybd_event(0x1B, 0, updown, 0)
    time.sleep(0.5)


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

    def check(name: str, ok: bool, detail: str = ""):
        nonlocal failures
        print(("OK  " if ok else "СБОЙ") + f" {name}" + (f" — {detail}" if detail else ""))
        if not ok:
            failures += 1

    probe({"cmd": "emit", "kind": "fileMode", "name": "web"})
    time.sleep(3)
    url = "file:///" + str(PAGE).replace("\\", "/")
    probe({"cmd": "weburl", "id": "browser", "url": url})
    time.sleep(3)
    view = probe({"cmd": "metric", "id": "browser-viewport"}).get("bounds")
    if not view:
        print("СБОЙ: нет вью браузера")
        app.kill()
        return 1

    # 1. Клик в инпут → фокус (маркер зелёный).
    click_view(view, 170, 34)
    time.sleep(1)
    img = shot("focus")
    m = marker(img, view, 120)
    check("фокус инпута по клику", close_to(m, (0, 255, 0)), f"маркер {m}")

    # 2. Набор: латиница, кириллица, backspace.
    for ch in ("a", "b", "c"):
        probe({"cmd": "webkey", "id": "browser", "key": ch, "char": ch})
    time.sleep(1)
    img = shot("abc")
    m = marker(img, view, 20)
    check("набор abc (len 3)", close_to(m, (30, 42, 255)), f"маркер {m}")

    probe({"cmd": "webkey", "id": "browser", "key": "ф", "char": "ф"})
    time.sleep(1)
    img = shot("cyr")
    m = marker(img, view, 20)
    check("кириллица «ф» (len 4)", close_to(m, (40, 42, 255)), f"маркер {m}")

    probe({"cmd": "webkey", "id": "browser", "key": "backspace"})
    time.sleep(1)
    img = shot("bs")
    m = marker(img, view, 20)
    check("backspace (len 3)", close_to(m, (30, 42, 255)), f"маркер {m}")

    # 3. Enter приходит как keydown с кодом 13.
    probe({"cmd": "webkey", "id": "browser", "key": "enter"})
    time.sleep(1)
    img = shot("enter")
    m = marker(img, view, 70)
    check("enter (keyCode 13→91)", close_to(m, (91, 200, 255)), f"маркер {m}")

    # 4. Ctrl+A / Ctrl+C → буфер обмена.
    subprocess.run(["powershell", "-c", "Set-Clipboard -Value 'sentinel'"], capture_output=True)
    probe({"cmd": "webkey", "id": "browser", "key": "a", "ctrl": True})
    time.sleep(0.3)
    probe({"cmd": "webkey", "id": "browser", "key": "c", "ctrl": True})
    time.sleep(1)
    got = clipboard()
    check("Ctrl+A/Ctrl+C → буфер", got == "abc", f"буфер {got!r}")

    # 5. Попап select: область под ним до и после клика.
    from PIL import ImageChops

    def below_select(img):
        box = (
            int((view["x"] + 20) * SCALE),
            int((view["y"] + 100) * SCALE),
            int((view["x"] + 320) * SCALE),
            int((view["y"] + 260) * SCALE),
        )
        return img.crop(box)

    before = below_select(shot("sel-before"))
    click_view(view, 170, 84)
    time.sleep(1.5)
    after = below_select(shot("sel-after"))
    diff = ImageChops.difference(before, after)
    changed = sum(1 for pxl in diff.getdata() if sum(pxl) > 30)
    check("попап select рисуется", changed > 500, f"пикселей изменилось {changed}")
    # Закрыть попап Esc'ом В СТРАНИЦУ: нативного окна у попапа нет (наша
    # текстура), системный Esc уходит мимо; клик пришёлся бы в список.
    probe({"cmd": "webkey", "id": "browser", "key": "escape"})
    time.sleep(1)

    # 6. Скачивание: системный диалог «Сохранить как».
    click_view(view, 45, 128)
    t0 = time.time()
    dlg = 0
    while time.time() - t0 < 8 and not dlg:
        dlg = find_dialog()
        time.sleep(0.5)
    check("диалог сохранения открылся", bool(dlg))
    if dlg:
        press_esc()

    # 8. Загрузка файла: клик по input type=file → системный диалог выбора.
    click_view(view, 170, 184)
    t0 = time.time()
    dlg = 0
    while time.time() - t0 < 8 and not dlg:
        dlg = find_dialog()
        time.sleep(0.5)
    check("диалог выбора файла открылся", bool(dlg))
    if dlg:
        press_esc()
        time.sleep(1)

    # 9. Контекст-меню: правый клик в инпут → нативное меню поверх экрана.
    # probe-скриншот (PrintWindow) чужое окно не видит — берём ВЕСЬ экран.
    from PIL import ImageGrab

    desk_before = ImageGrab.grab().convert("RGB")
    probe({"cmd": "click", "x": view["x"] + 170, "y": view["y"] + 34, "button": "right"})
    time.sleep(1.5)
    desk_after = ImageGrab.grab().convert("RGB")
    diff = ImageChops.difference(desk_before, desk_after)
    changed = sum(1 for pxl in diff.getdata() if sum(pxl) > 60)
    check("контекст-меню появилось", changed > 2000, f"пикселей изменилось {changed}")
    press_esc()
    time.sleep(1)

    # 7. Клик мимо вью (титлбар) → blur → маркер красный.
    click_view(view, 170, 34)  # вернуть фокус
    time.sleep(0.5)
    probe({"cmd": "click", "x": 300, "y": 20})
    time.sleep(1)
    img = shot("blur")
    m = marker(img, view, 120)
    check("blur по клику мимо вью", close_to(m, (255, 0, 0)), f"маркер {m}")

    app.kill()
    print(f"\nитог: сбоев {failures}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
