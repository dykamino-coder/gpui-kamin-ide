"""Стенд РЕАЛЬНОГО ввода в веб-вью: SendInput при окне на переднем плане.

Отличие от `check_web_interact.py`: тот шлёт клавиши напрямую в CEF (probe
`webkey`) и мышь оконными сообщениями — это проверяет мост «наш код → CEF»,
но НЕ путь «Windows → gpui → фокус обёртки → CEF». Юзер поймал именно его:
webkey работал, живой ввод — нет (обёртка не брала gpui-фокус).

Здесь всё по-настоящему: SetForegroundWindow + SendInput (мышь и клавиатура).
НЕ трогать мышь во время прогона.

Маркеры страницы `web_interact.html` — как в check_web_interact.py.

    python scripts/check_web_real.py
"""

import ctypes
import io
import json
import os
import socket
import subprocess
import sys
import time
from ctypes import wintypes
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent
EXE = ROOT / "target" / os.environ.get("KAMIN_EXE", "debug") / "kaminide-gpui.exe"
LOG = ROOT / "target" / "web-real.log"
SHOT = ROOT / "target" / "web-real-{}.png"
PAGE = ROOT / "scripts" / "web_interact.html"
PORT = 9333
SCALE = 1.25
u32 = ctypes.windll.user32


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


def shot(tag: str):
    from PIL import Image

    path = str(SHOT).format(tag).replace("\\", "/")
    probe({"cmd": "screenshot", "path": path})
    return Image.open(path).convert("RGB")


def marker(img, view, my: float):
    x0 = int((view["x"] + view["w"] - 60 + 10) * SCALE)
    y0 = int((view["y"] + my + 10) * SCALE)
    px = [img.getpixel((x0 + dx, y0 + dy)) for dx in range(0, 20, 5) for dy in range(0, 20, 5)]
    return tuple(sum(c[i] for c in px) // len(px) for i in range(3))


def close_to(rgb, want, tol=6) -> bool:
    return all(abs(rgb[i] - want[i]) <= tol for i in range(3))


# ── Реальный ввод: SendInput ─────────────────────────────────────────────
PUL = ctypes.POINTER(ctypes.c_ulong)


class KI(ctypes.Structure):
    _fields_ = [
        ("wVk", ctypes.c_ushort),
        ("wScan", ctypes.c_ushort),
        ("dwFlags", ctypes.c_ulong),
        ("time", ctypes.c_ulong),
        ("dwExtraInfo", PUL),
    ]


class MI(ctypes.Structure):
    _fields_ = [
        ("dx", ctypes.c_long),
        ("dy", ctypes.c_long),
        ("mouseData", ctypes.c_ulong),
        ("dwFlags", ctypes.c_ulong),
        ("time", ctypes.c_ulong),
        ("dwExtraInfo", PUL),
    ]


class INPUT_U(ctypes.Union):
    _fields_ = [("ki", KI), ("mi", MI)]


class INPUT(ctypes.Structure):
    _fields_ = [("type", ctypes.c_ulong), ("u", INPUT_U)]


def send_key(vk: int, up: bool):
    inp = INPUT(type=1)
    inp.u.ki = KI(vk, 0, 2 if up else 0, 0, None)
    u32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))


def press(vk: int):
    send_key(vk, False)
    time.sleep(0.03)
    send_key(vk, True)
    time.sleep(0.05)


def send_unicode(ch: str):
    for code in ch.encode("utf-16-le"):
        pass
    code = ord(ch)
    for up in (False, True):
        inp = INPUT(type=1)
        inp.u.ki = KI(0, code, 0x0004 | (0x0002 if up else 0), 0, None)  # KEYEVENTF_UNICODE
        u32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))
        time.sleep(0.02)


def mouse_click(sx: int, sy: int, right: bool = False, double: bool = False):
    u32.SetCursorPos(sx, sy)
    time.sleep(0.15)
    down, up = (0x0008, 0x0010) if right else (0x0002, 0x0004)
    times = 2 if double else 1
    for _ in range(times):
        for flag in (down, up):
            inp = INPUT(type=0)
            inp.u.mi = MI(0, 0, 0, flag, 0, None)
            u32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))
            time.sleep(0.04)
        time.sleep(0.06)
    time.sleep(0.3)


def screen_pt(hwnd, view, vx, vy):
    pt = wintypes.POINT(int((view["x"] + vx) * SCALE), int((view["y"] + vy) * SCALE))
    u32.ClientToScreen(hwnd, ctypes.byref(pt))
    return pt.x, pt.y


def clipboard() -> str:
    # Юникод честно, без консольных кодировок powershell (кириллица терялась).
    CF_UNICODETEXT = 13
    u32.GetClipboardData.restype = ctypes.c_void_p
    ctypes.windll.kernel32.GlobalLock.restype = ctypes.c_void_p
    ctypes.windll.kernel32.GlobalLock.argtypes = [ctypes.c_void_p]
    ctypes.windll.kernel32.GlobalUnlock.argtypes = [ctypes.c_void_p]
    if not u32.OpenClipboard(0):
        return "<clipboard busy>"
    try:
        h = u32.GetClipboardData(CF_UNICODETEXT)
        if not h:
            return ""
        ptr = ctypes.windll.kernel32.GlobalLock(h)
        text = ctypes.wstring_at(ptr)
        ctypes.windll.kernel32.GlobalUnlock(h)
        return text
    finally:
        u32.CloseClipboard()


def main() -> int:
    if not EXE.exists():
        print(f"нет приложения: {EXE}")
        return 1
    subprocess.run(
        ["powershell", "-c", "Stop-Process -Name kaminide-gpui -Force -ErrorAction SilentlyContinue"],
        capture_output=True,
    )
    time.sleep(2)
    log = io.open(LOG, "w", encoding="utf-8")
    app = subprocess.Popen([str(EXE)], stdout=log, stderr=subprocess.STDOUT, cwd=ROOT)
    print("ждём канал… (мышь не трогать: стенд шлёт РЕАЛЬНЫЙ ввод)")
    deadline = time.time() + 60
    while time.time() < deadline:
        try:
            probe({"cmd": "metric", "id": "titlebar"}, timeout=2.0)
            break
        except OSError:
            time.sleep(0.5)
    time.sleep(6)
    failures = 0

    def check(name, ok, detail=""):
        nonlocal failures
        print(("OK  " if ok else "СБОЙ") + f" {name}" + (f" — {detail}" if detail else ""))
        if not ok:
            failures += 1

    probe({"cmd": "emit", "kind": "fileMode", "name": "web"})
    time.sleep(3)
    view = probe({"cmd": "metric", "id": "browser-viewport"})["bounds"]
    # Навигация по ещё НЕ созданному браузеру молча теряется (создание
    # асинхронное) — шлём, пока на кадре не появится тёмный фон страницы.
    url = "file:///" + str(PAGE).replace("\\", "/")
    for _ in range(12):
        probe({"cmd": "weburl", "id": "browser", "url": url})
        time.sleep(1.5)
        img = shot("nav")
        corner = img.getpixel((int((view["x"] + 8) * SCALE), int((view["y"] + view["h"] - 8) * SCALE)))
        if close_to(corner, (30, 30, 46), tol=12):
            break
    else:
        print("СБОЙ: тест-страница так и не загрузилась")
        app.kill()
        return 1
    hwnd = u32.FindWindowW(None, "KaminIDE")
    # Windows блокирует SetForegroundWindow фоновому процессу — обходим
    # классикой (Alt-тап снимает foreground-lock) и ПРОВЕРЯЕМ результат:
    # иначе SendInput утёк бы в чужое активное окно (первый прогон).
    k32 = ctypes.windll.kernel32
    for _ in range(6):
        if u32.GetForegroundWindow() == hwnd:
            break
        u32.ShowWindow(hwnd, 9)  # SW_RESTORE: окно может быть свёрнуто
        for updown in (0, 2):
            ctypes.windll.user32.keybd_event(0x12, 0, updown, 0)  # Alt
        # AttachThreadInput к текущему foreground-потоку снимает блокировку.
        fg = u32.GetForegroundWindow()
        fg_tid = u32.GetWindowThreadProcessId(fg, None) if fg else 0
        my_tid = k32.GetCurrentThreadId()
        if fg_tid and fg_tid != my_tid:
            u32.AttachThreadInput(my_tid, fg_tid, True)
        u32.SetForegroundWindow(hwnd)
        u32.BringWindowToTop(hwnd)
        if fg_tid and fg_tid != my_tid:
            u32.AttachThreadInput(my_tid, fg_tid, False)
        time.sleep(0.5)
    if u32.GetForegroundWindow() != hwnd:
        print("СБОЙ: окно не удалось поднять на передний план — реальный ввод невозможен")
        app.kill()
        return 1
    time.sleep(0.7)

    # Раскладка окна — EN: прошлый прогон мог оставить RU, и «abc» шло
    # «фис» (набор проверяем ДЛИНОЙ, но буфер обмена сверяем содержимым).
    u32.PostMessageW(hwnd, 0x0050, 0, u32.LoadKeyboardLayoutW("00000409", 1))
    time.sleep(0.5)

    # 1. Реальный клик в инпут → фокус страницы.
    mouse_click(*screen_pt(hwnd, view, 170, 34))
    img = shot("focus")
    m = marker(img, view, 120)
    check("реальный клик → фокус", close_to(m, (0, 255, 0)), f"{m}")

    # 2. Реальный набор a b c (VK) → len 3.
    for vk in (0x41, 0x42, 0x43):
        press(vk)
    time.sleep(0.8)
    m = marker(shot("abc"), view, 20)
    check("реальный набор abc", close_to(m, (30, 42, 255)), f"{m}")

    # 3. Кириллица ЧЕСТНО: русская раскладка окну + физическая клавиша A
    # (ЙЦУКЕН: это «ф») → gpui сам переведёт VK через ToUnicode.
    RU = "00000419"
    EN = "00000409"
    hkl_ru = u32.LoadKeyboardLayoutW(RU, 1)
    u32.PostMessageW(hwnd, 0x0050, 0, hkl_ru)  # WM_INPUTLANGCHANGEREQUEST
    time.sleep(0.5)
    press(0x41)
    time.sleep(0.8)
    hkl_en = u32.LoadKeyboardLayoutW(EN, 1)
    u32.PostMessageW(hwnd, 0x0050, 0, hkl_en)
    time.sleep(0.4)
    m = marker(shot("cyr"), view, 20)
    check("реальная кириллица «ф» (ru-раскладка)", close_to(m, (40, 42, 255)), f"{m}")

    # 4. Backspace + Enter.
    press(0x08)
    time.sleep(0.5)
    m = marker(shot("bs"), view, 20)
    check("реальный backspace", close_to(m, (30, 42, 255)), f"{m}")
    press(0x0D)
    time.sleep(0.5)
    m = marker(shot("enter"), view, 70)
    check("реальный enter (13→91)", close_to(m, (91, 200, 255)), f"{m}")

    # 5. Реальные Ctrl+A / Ctrl+C → буфер.
    subprocess.run(["powershell", "-c", "Set-Clipboard -Value 'sentinel'"], capture_output=True)
    send_key(0x11, False)  # ctrl down
    time.sleep(0.05)
    press(0x41)
    time.sleep(0.3)
    mk_a = marker(shot("ctrl-a"), view, 70)  # 65*7%256=199: дошёл ли ctrl+a
    press(0x43)
    send_key(0x11, True)
    time.sleep(1)
    mk_c = marker(shot("ctrl-c"), view, 70)  # 67*7%256=213: дошёл ли ctrl+c
    got = clipboard()
    check(
        "реальный Ctrl+A/Ctrl+C",
        got == "abc",
        f"буфер {got!r}, ctrl+a {mk_a}, ctrl+c {mk_c}",
    )

    # 6. Реальный правый клик → НАСТОЯЩЕЕ меню (класс #32768).
    mouse_click(*screen_pt(hwnd, view, 170, 34), right=True)
    time.sleep(1)
    menu = u32.FindWindowW("#32768", None)
    if not menu:
        from PIL import ImageGrab
        ImageGrab.grab().convert("RGB").save(str(ROOT / "target" / "web-real-menu-desktop.png"))
    check("контекст-меню (класс #32768)", bool(menu))
    for updown in (0, 2):
        ctypes.windll.user32.keybd_event(0x1B, 0, updown, 0)
    time.sleep(0.7)

    # 7. Внутренний тост — в правом нижнем углу (метрика ov-toasts).
    probe({"cmd": "emit", "kind": "toast", "name": "position check"})
    time.sleep(1.2)
    tb = probe({"cmd": "metric", "id": "ov-toasts"}).get("bounds")
    vp = probe({"cmd": "metric", "id": "titlebar"}).get("bounds")
    if not tb:
        check("тост: метрика ov-toasts", False, "нет bounds")
    else:
        vw = vp["w"] if vp else 0
        right_gap = vw - (tb["x"] + tb["w"])
        # низ стека прижат к статусбару: тост в нижней половине окна
        check(
            "тост в правом нижнем углу",
            abs(right_gap - 16) <= 8 and tb["y"] > 400,
            f"правый зазор {right_gap:.0f}, y {tb['y']:.0f}",
        )

    # 8. Нативный open-диалог (путь shell.showOpenDialog).
    probe({"cmd": "opendialog"})
    t0 = time.time()
    dlg = 0
    while time.time() - t0 < 8 and not dlg:
        dlg = u32.FindWindowW("#32770", None)
        time.sleep(0.5)
    check("rfd open-диалог открылся", bool(dlg))
    if dlg:
        for updown in (0, 2):
            ctypes.windll.user32.keybd_event(0x1B, 0, updown, 0)
        time.sleep(0.7)

    # 9. Реальный клик мимо вью → blur.
    mouse_click(*screen_pt(hwnd, view, 170, 34))
    time.sleep(0.5)
    pt = wintypes.POINT(int(300 * SCALE), int(20 * SCALE))
    u32.ClientToScreen(hwnd, ctypes.byref(pt))
    mouse_click(pt.x, pt.y)
    time.sleep(0.8)
    m = marker(shot("blur"), view, 120)
    check("реальный клик мимо → blur", close_to(m, (255, 0, 0)), f"{m}")

    app.kill()
    print(f"\nитог: сбоев {failures}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
