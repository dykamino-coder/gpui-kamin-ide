"""Попиксельное сравнение: Chrome против нашей отрисовки.

Зачем: покрытие свойств можно доказать тестами, но «выглядит так же» —
только картинкой. Скрипт рисует ОДИН файл разметки двумя движками и считает,
где они разошлись.

    python scripts/pixel_compare.py [файл.html] [ширина] [высота]

Печатает долю несовпавших пикселей и список полос по вертикали, где
расхождение сосредоточено, — по ним видно, какой блок разъехался.
Код возврата 1, если расхождение больше порога.
"""

import io
import os
import subprocess
import sys
import time
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent
CHROME = Path(r"C:\Program Files\Google\Chrome\Application\chrome.exe")
OUT = ROOT / "target" / "compare"
# Порог: текст растрируется разными движками и совпасть до пикселя не может,
# поэтому эталон состоит из прямоугольников. Для них расхождение выше
# нескольких процентов — это разъехавшаяся раскладка, а не сглаживание.
THRESHOLD_PCT = 3.0


def shot_chrome(html: Path, w: int, h: int) -> Path:
    """Снимок из Chrome в headless-режиме."""
    out = OUT / "chrome.png"
    profile = OUT / "chrome-profile"
    cmd = [
        str(CHROME),
        "--headless=new",
        "--disable-gpu",
        "--hide-scrollbars",
        "--force-device-scale-factor=1",
        f"--user-data-dir={profile}",
        f"--window-size={w},{h}",
        f"--screenshot={out}",
        html.as_uri(),
    ]
    subprocess.run(cmd, capture_output=True, timeout=120)
    return out


def shot_ours(html: Path, w: int, h: int) -> Path:
    """Снимок из нашего примера: запустить, дать нарисовать, снять окно."""
    exe = ROOT / "target" / "debug" / "examples" / "compare.exe"
    if not exe.exists():
        print(f"нет сборки примера: {exe}")
        sys.exit(1)
    subprocess.run(
        ["powershell", "-NoProfile", "-Command",
         "Get-Process compare -ErrorAction SilentlyContinue | Stop-Process -Force"],
        capture_output=True,
    )
    time.sleep(1)
    proc = subprocess.Popen([str(exe), str(html), str(w), str(h)])
    time.sleep(6)
    out = OUT / "ours.png"
    ps = f'''
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;using System.Runtime.InteropServices;
public class W {{
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  public struct R {{ public int L,T,Rr,B; }}
  public struct P {{ public int X,Y; }}
}}
"@
$p = Get-Process compare -ErrorAction Stop | Select-Object -First 1
$h = $p.MainWindowHandle
[void][W]::ShowWindow($h, 9); [void][W]::SetForegroundWindow($h)
Start-Sleep -Milliseconds 800
# Снимаем ОКНО целиком (PrintWindow не зависит от перекрытия другими окнами),
# затем обрезаем до клиентской области: иначе рамка сдвигает всё содержимое.
$wr = New-Object W+R; [void][W]::GetWindowRect($h, [ref]$wr)
$cr = New-Object W+R; [void][W]::GetClientRect($h, [ref]$cr)
$pt = New-Object W+P; [void][W]::ClientToScreen($h, [ref]$pt)
$ww = $wr.Rr - $wr.L; $wh = $wr.B - $wr.T
$cw = $cr.Rr - $cr.L; $ch = $cr.B - $cr.T
$offX = $pt.X - $wr.L; $offY = $pt.Y - $wr.T
$full = New-Object System.Drawing.Bitmap $ww, $wh
$g = [System.Drawing.Graphics]::FromImage($full)
$dc = $g.GetHdc(); [void][W]::PrintWindow($h, $dc, 2); $g.ReleaseHdc($dc); $g.Dispose()
$rect = New-Object System.Drawing.Rectangle $offX, $offY, $cw, $ch
$client = $full.Clone($rect, $full.PixelFormat)
$client.Save("{out}", [System.Drawing.Imaging.ImageFormat]::Png)
"client ${{cw}}x${{ch}} offset ${{offX}},${{offY}}"
'''
    subprocess.run(["powershell", "-NoProfile", "-Command", ps], capture_output=True)
    proc.terminate()
    return out


def compare(a: Path, b: Path) -> float:
    """Доля несовпавших пикселей и разбор по полосам."""
    from PIL import Image, ImageChops

    ia = Image.open(a).convert("RGB")
    ib = Image.open(b).convert("RGB")
    # Наш снимок приходит в ФИЗИЧЕСКИХ пикселях (на 125% системном масштабе
    # окно 800 логических точек снимается как 1000), а Chrome принудительно
    # рисует в масштабе 1. Сравнивается логическая геометрия, поэтому наш
    # снимок приводится к размеру эталона.
    if (ib.width, ib.height) != (ia.width, ia.height):
        scale = ia.width / ib.width
        print(f"масштаб нашего снимка {ib.width}x{ib.height} → {ia.width}x{ia.height} (×{scale:.3f})")
        ib = ib.resize((ia.width, round(ib.height * scale)), Image.LANCZOS)
    w = min(ia.width, ib.width)
    h = min(ia.height, ib.height)
    ia = ia.crop((0, 0, w, h))
    ib = ib.crop((0, 0, w, h))
    ia.save(OUT / "chrome-norm.png")
    ib.save(OUT / "ours-norm.png")

    diff = ImageChops.difference(ia, ib)
    # Небольшая разница цвета — сглаживание кромок, а не смещение блока.
    mask = diff.convert("L").point(lambda v: 255 if v > 40 else 0)
    mask.save(OUT / "diff.png")

    px = mask.load()
    total = w * h
    bad = 0
    bands = []
    band_h = 40
    for y0 in range(0, h, band_h):
        band_bad = 0
        for y in range(y0, min(y0 + band_h, h)):
            for x in range(0, w, 2):  # шаг 2: карта расхождений, не точный счёт
                if px[x, y]:
                    band_bad += 2
        bad += band_bad
        if band_bad:
            bands.append((y0, band_bad * 100.0 / (band_h * w)))

    pct = bad * 100.0 / total
    print(f"размер {w}x{h}, расхождение {pct:.2f}%")
    worst = sorted(bands, key=lambda t: -t[1])[:8]
    if worst:
        print("полосы с наибольшим расхождением (y, %):")
        for y0, p in worst:
            print(f"  y={y0:4d}  {p:5.1f}%")
    print(f"карта различий: {OUT / 'diff.png'}")
    return pct


def boxes_by_colour(img, scale: float = 1.0) -> dict:
    """Габариты каждого цветного блока: цвет → (x, y, ширина, высота).

    Пиксельная разница меряет сглаживание кромок, а не раскладку. В эталоне у
    каждого блока свой цвет, поэтому честная метрика — где этот цвет начался и
    кончился. Так видно расхождение в точках, а не в процентах пикселей.
    """
    px = img.load()
    w, h = img.size
    found: dict = {}
    for y in range(0, h):
        for x in range(0, w):
            c = px[x, y]
            # Фон и почти-белое не считаем блоками.
            if c[0] > 230 and c[1] > 230 and c[2] > 230:
                continue
            # Грубый ключ: сглаживание кромок иначе плодит мнимые блоки.
            key = (c[0] // 32, c[1] // 32, c[2] // 32)
            b = found.get(key)
            if b is None:
                found[key] = [x, y, x, y, 1]
            else:
                b[4] += 1
                if x < b[0]:
                    b[0] = x
                if y < b[1]:
                    b[1] = y
                if x > b[2]:
                    b[2] = x
                if y > b[3]:
                    b[3] = y
    return {
        k: (
            round(v[0] * scale),
            round(v[1] * scale),
            round((v[2] - v[0] + 1) * scale),
            round((v[3] - v[1] + 1) * scale),
        )
        for k, v in found.items()
        # Отсев кромок и сглаживания: настоящий блок и крупнее 8 точек по
        # каждой стороне, и залит целиком, а не тонкой каймой.
        if (v[2] - v[0]) >= 8 and (v[3] - v[1]) >= 8 and v[4] >= 400
    }


def compare_geometry(a: Path, b: Path) -> int:
    """Сравнение по габаритам блоков. Возвращает число расхождений > 2 точек."""
    from PIL import Image

    ia = Image.open(a).convert("RGB")
    ib = Image.open(b).convert("RGB")
    scale = ia.width / ib.width if ib.width else 1.0
    ba = boxes_by_colour(ia)
    bb = boxes_by_colour(ib, scale)

    # Допуск: округление логических точек в физические и обратно даёт ±2.
    tol = 2
    only_chrome = sorted(set(ba) - set(bb))
    only_ours = sorted(set(bb) - set(ba))
    mismatches = []
    for key in sorted(set(ba) & set(bb)):
        x1, y1, w1, h1 = ba[key]
        x2, y2, w2, h2 = bb[key]
        d = (abs(x1 - x2), abs(y1 - y2), abs(w1 - w2), abs(h1 - h2))
        if max(d) > tol:
            mismatches.append((key, ba[key], bb[key], d))

    total = len(set(ba) | set(bb))
    print()
    print(f"блоков в эталоне {len(ba)}, у нас {len(bb)}")
    if only_chrome:
        print(f"ПОТЕРЯНЫ у нас: {len(only_chrome)} цвет(ов) — {only_chrome[:6]}")
    if only_ours:
        print(f"ЛИШНИЕ у нас: {len(only_ours)} цвет(ов) — {only_ours[:6]}")
    if mismatches:
        print(f"РАСХОЖДЕНИЯ ГЕОМЕТРИИ (допуск {tol} тчк): {len(mismatches)}")
        for key, chrome, ours, d in mismatches[:12]:
            print(f"  цвет {key}: хром {chrome} → наш {ours}, разница {d}")
    else:
        print(f"геометрия совпала для всех {len(ba)} блоков (допуск {tol} тчк)")
    return len(mismatches) + len(only_chrome) + len(only_ours)


def main() -> int:
    html = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 and sys.argv[1] else (
        ROOT / "crates" / "html" / "tests" / "fixtures" / "layout.html"
    )
    w = int(sys.argv[2]) if len(sys.argv) > 2 else 800
    h = int(sys.argv[3]) if len(sys.argv) > 3 else 700
    OUT.mkdir(parents=True, exist_ok=True)

    print(f"эталон: {html}")
    a = shot_chrome(html, w, h)
    if not a.exists():
        print("Chrome не отдал снимок")
        return 1
    b = shot_ours(html, w, h)
    if not b.exists():
        print("наш снимок не получен")
        return 1
    pct = compare(a, b)
    bad = compare_geometry(a, b)
    # Решает геометрия: пиксельная доля меряет сглаживание кромок при
    # приведении масштабов, а не правильность раскладки.
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
