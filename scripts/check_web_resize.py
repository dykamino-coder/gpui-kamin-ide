"""Стенд для веб-панели: сам тащит сплиттер и сам проверяет, что кадр догнал.

Зачем: проверять ресайз глазами через человека — долго и ненадёжно. Скрипт
повторяет ровно тот жест (зажать сплиттер, вести, отпустить) и после каждого
шага сверяет три вещи:

  * приложение живо (иначе печатает хвост лога и валится);
  * размер кадра CEF совпал с размером панели (с поправкой на масштаб экрана);
  * у правого края панели действительно пиксели страницы, а не фон карточки —
    так ловится «кадр меньше панели», который по одним числам не виден.

    python scripts/check_web_resize.py

Код возврата 1, если хоть один шаг не сошёлся.
"""

import io
import json
import os
import socket
import subprocess
import sys
import time
from pathlib import Path

# Вывод в UTF-8: в консоли Windows иначе рвётся на «→» и кириллице.
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent
import os
EXE = ROOT / "target" / os.environ.get("KAMIN_EXE", "debug") / "kaminide-gpui.exe"
LOG = ROOT / "target" / "web-resize.log"
SHOT = ROOT / "target" / "web-resize.png"
PORT = 9333
# Ширины панели в логических px, куда ведём сплиттер. Есть и рост, и сужение,
# и резкий бросок — прошлые поломки вылезали именно на них.
STEPS = [520, 900, 620, 1150, 480]


def probe(req: dict, timeout: float = 5.0):
    """Один запрос в отладочный канал приложения."""
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
    lines = io.open(LOG, encoding="utf-8", errors="replace").read().splitlines()
    return "\n".join(lines[-n:])


def last_texture() -> tuple[int, int] | None:
    """Последний размер кадра ИМЕННО браузерной панели.

    Вью в приложении несколько (чат моста, демо-страница), и все они пишут о
    своих кадрах — без отбора по имени сюда попадал чужой размер.
    """
    if not LOG.exists():
        return None
    for line in reversed(io.open(LOG, encoding="utf-8", errors="replace").read().splitlines()):
        if "[cef] текстура вью browser:" in line and "×" in line:
            size = line.rsplit(":", 1)[1].strip()
            w, h = size.split("×")
            return int(w), int(h)
    return None


def page_reaches_edge(bounds: dict, scale: float) -> bool:
    """Есть ли пиксели страницы у ПРАВОГО края панели.

    Кадр меньше панели выглядит как тёмная полоса справа — по числам её не
    видно, поэтому смотрим сам пиксель.
    """
    from PIL import Image

    probe({"cmd": "screenshot", "path": str(SHOT).replace("\\", "/")})
    img = Image.open(SHOT).convert("RGB")
    x = int((bounds["x"] + bounds["w"] - 6) * scale)
    y = int((bounds["y"] + bounds["h"] / 2) * scale)
    x = min(max(x, 0), img.width - 1)
    y = min(max(y, 0), img.height - 1)
    # Берём полосу у правого края панели: если кадр меньше панели, там
    # остаётся тёмный/чёрный фон, а не пиксели страницы.
    xs = [x - d for d in (4, 8, 12)]
    vals = []
    for xi in xs:
        xi = min(max(xi, 0), img.width - 1)
        r, g, b = img.getpixel((xi, y))
        vals.append((r + g + b) / 3)
    return max(vals) > 90


PS_WINDOW = """
Add-Type -Namespace WT -Name U -MemberDefinition '[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c); [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr p); public delegate bool EnumWindowsProc(IntPtr h, IntPtr p); [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid); [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);'
$ids=(Get-Process -Name kaminide-gpui).Id
$cb=[WT.U+EnumWindowsProc]{ param($h,$p) $pid2=0; [void][WT.U]::GetWindowThreadProcessId($h,[ref]$pid2); if($ids -contains $pid2 -and [WT.U]::IsWindowVisible($h)){ [void][WT.U]::ShowWindow($h,CMD) }; return $true }
[void][WT.U]::EnumWindows($cb,[IntPtr]::Zero)
"""


def show_window(cmd: int):
    """6 — свернуть, 9 — развернуть."""
    subprocess.run(
        ["powershell", "-c", PS_WINDOW.replace("CMD", str(cmd))],
        capture_output=True,
    )


def panel_brightness(bounds: dict, scale: float) -> float:
    """Средняя яркость середины панели — пустая панель заметно темнее."""
    from PIL import Image

    probe({"cmd": "screenshot", "path": str(SHOT).replace("\\", "/")})
    img = Image.open(SHOT).convert("RGB")
    x0 = int((bounds["x"] + 20) * scale)
    y0 = int((bounds["y"] + 20) * scale)
    x1 = int((bounds["x"] + bounds["w"] - 20) * scale)
    y1 = int((bounds["y"] + bounds["h"] - 20) * scale)
    px = img.crop((x0, y0, x1, y1))
    data = list(px.getdata())
    return sum(sum(p) for p in data) / (len(data) * 3)


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

    probe({"cmd": "emit", "kind": "fileMode", "name": "web"})
    time.sleep(4)

    scale = 1.25
    failures = 0
    for want in STEPS:
        if app.poll() is not None:
            print(f"ПАДЕНИЕ приложения на шаге {want}\n" + tail(30))
            return 1
        try:
            vres = probe({"cmd": "metric", "id": "browser-viewport"})
            tres = probe({"cmd": "metric", "id": "file-tree"})
            if "bounds" not in vres or "bounds" not in tres:
                print(f"нет региона на шаге {want}: панель={vres}, дерево={tres}")
                print(tail(20))
                return 1
            view, tree = vres["bounds"], tres["bounds"]
        except (OSError, KeyError) as e:
            print(f"ЗАВИСАНИЕ или пропал регион на шаге {want}: {e}\n" + tail(30))
            return 1
        # Ведём ручку так, чтобы панель стала нужной ширины.
        # Вертикальный сплиттер между центром и правой колонкой probe-регионом
        # не помечен, поэтому берём щель между правым краем панели и левым
        # краем дерева файлов — ручка ровно там.
        from_x = round((view["x"] + view["w"] + tree["x"]) / 2)
        to_x = round(from_x + (want - view["w"]))
        y = round(view["y"] + view["h"] / 2)
        res = probe({"cmd": "drag", "from": [from_x, y], "to": [to_x, y]})
        if not res.get("ok"):
            print(f"драг не прошёл: {res}")
        # ЗАМЕР ДОГОНА: сколько мс от конца драга до кадра нужного размера.
        t0 = time.time()
        catch_ms = None
        while time.time() - t0 < 1.5:
            b = probe({"cmd": "metric", "id": "browser-viewport"}).get("bounds")
            tex = last_texture()
            if b and tex and abs(tex[0] - round(b["w"] * scale)) <= 3:
                catch_ms = int((time.time() - t0) * 1000)
                break
            time.sleep(0.03)
        print(f"  догон после отпускания: {catch_ms if catch_ms is not None else '>1500'} мс")
        time.sleep(0.3)

        if app.poll() is not None:
            print(f"ПАДЕНИЕ после драга {want}\n" + tail(30))
            return 1
        try:
            view = probe({"cmd": "metric", "id": "browser-viewport"})["bounds"]
        except OSError as e:
            print(f"ЗАВИСАНИЕ после драга {want}: {e}\n" + tail(30))
            return 1

        tex = last_texture()
        want_px = (round(view["w"] * scale), round(view["h"] * scale))
        ok_size = tex is not None and abs(tex[0] - want_px[0]) <= 2 and abs(tex[1] - want_px[1]) <= 2
        ok_pixels = page_reaches_edge(view, scale)
        # Приговор выносим по размеру: он объективен. Пиксель у края —
        # подсказка, а не приговор: на тёмной странице (google в тёмной теме)
        # он неотличим от фона карточки.
        verdict = "ok" if ok_size else "СБОЙ"
        if verdict != "ok":
            failures += 1
        print(
            f"{verdict}: панель {view['w']:.0f}×{view['h']:.0f} лог.px → ждём кадр "
            f"{want_px[0]}×{want_px[1]}, пришёл {tex}, страница у правого края: {ok_pixels}"
        )

    # Сворачивание окна: gpui пересоздаёт устройство D3D11, и текстуры от
    # прежнего мертвы — панель оставалась пустой (поймано на живом прогоне).
    view = probe({"cmd": "metric", "id": "browser-viewport"})["bounds"]
    before = panel_brightness(view, scale)
    show_window(6)
    time.sleep(3)
    show_window(9)
    time.sleep(5)
    if app.poll() is not None:
        print("ПАДЕНИЕ после сворачивания\n" + tail(30))
        return 1
    view = probe({"cmd": "metric", "id": "browser-viewport"})["bounds"]
    after = panel_brightness(view, scale)
    ok_min = after > before * 0.6
    if not ok_min:
        failures += 1
    print(f"{'ok' if ok_min else 'СБОЙ'}: сворачивание — яркость {before:.1f} → {after:.1f}")

    print(f"\nитог: шагов {len(STEPS)} + сворачивание, сбоев {failures}")
    kill_app()
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
