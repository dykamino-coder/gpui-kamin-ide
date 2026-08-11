"""Снимки страниц НАСТОЯЩИМ браузером — честный эталон для ручного разбора.

Стенд сравнивает нас с нами же: и тест, и эталон рисует наш движок. Поэтому
пара, где обе стороны сломаны одинаково, выглядит зелёной. Здесь те же
страницы снимает Chrome, и человеку есть с чем сверяться.

    python scripts/wpt_chrome_shots.py target/pairs-text-nojs.txt
    python scripts/wpt_chrome_shots.py target/rep-text-n4.txt --only-failed

Страницы отдаются по HTTP из корня набора, а НЕ файловым адресом: тесты
подключают шрифт Ahem как `/fonts/ahem.css`, и от `file:///` такой адрес
уезжает в корень диска. Без него Chrome рисует настоящие буквы там, где по
тесту обязаны быть чёрные квадраты Ahem, — и третья колонка стенда врёт.

Снимки ложатся в `target/wpt-chrome/<имя>.png` рядом с нашими из
`target/wpt-shots`. Размер окна и масштаб — как у стенда (800×600 при 1.25),
иначе картинки не совместить.

    --only-failed  снимать только непройденные пары
    --force        переснять даже то, что уже снято
"""

import functools
import http.server
import socketserver
import subprocess
import sys
import threading
from pathlib import Path

ROOT = Path("vendor/wpt-parsing").resolve()

OUT = Path("target/wpt-chrome")
# Стенд просит окно 800×600 логических точек при масштабе 1.25 — снимок
# выходит 1000×750. Браузеру задаём то же самое, иначе перенос строк
# случится в другом месте и сравнивать будет нечего.
WIDTH, HEIGHT, SCALE = 800, 600, 1.25
CHROME = [
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
]


def browser() -> str:
    for path in CHROME:
        if Path(path).is_file():
            return path
    raise SystemExit("не нашёл ни Chrome, ни Edge — правь список CHROME")


def pages(report: Path, only_failed: bool) -> list[Path]:
    """Страницы из списка пар или из отчёта: и тест, и его эталон."""
    out: list[Path] = []
    for line in report.read_text(encoding="utf-8", errors="ignore").splitlines():
        parts = line.split("|")
        if len(parts) < 2:
            continue
        if only_failed and len(parts) >= 3:
            try:
                if float(parts[2]) <= 0.5:
                    continue
            except ValueError:
                pass
        for side in parts[:2]:
            page = Path(side.strip())
            if page.is_file() and page not in out:
                out.append(page)
    return out


def serve() -> int:
    """Отдать корень набора по HTTP и вернуть порт."""
    handler = functools.partial(
        http.server.SimpleHTTPRequestHandler, directory=str(ROOT)
    )
    # Тихий обработчик: иначе на каждый запрос сыплется строка в вывод.
    handler.log_message = lambda *_: None
    httpd = socketserver.ThreadingTCPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    return httpd.server_address[1]


def shoot(exe: str, page: Path, dest: Path, port: int) -> bool:
    dest.parent.mkdir(parents=True, exist_ok=True)
    # Путь снимка — ОБЫЧНЫЙ путь Windows. Адрес страницы — HTTP из корня
    # набора: только так `/fonts/ahem.css` из теста находит свой шрифт.
    try:
        rel = page.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        # Страница вне набора: снимать её остаётся файловым адресом.
        rel = None
    cmd = [
        exe,
        "--headless=new",
        "--disable-gpu",
        "--hide-scrollbars",
        "--virtual-time-budget=3000",
        f"--force-device-scale-factor={SCALE}",
        f"--window-size={WIDTH},{HEIGHT}",
        f"--screenshot={dest.resolve()}",
        (
            f"http://127.0.0.1:{port}/{rel}"
            if rel is not None
            else "file:///" + str(page.resolve()).replace("\\", "/")
        ),
    ]
    try:
        subprocess.run(cmd, capture_output=True, timeout=60)
    except subprocess.TimeoutExpired:
        return False
    return dest.is_file()


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    only_failed = "--only-failed" in sys.argv
    if not args:
        raise SystemExit(__doc__)
    force = "--force" in sys.argv
    exe = browser()
    port = serve()
    todo = pages(Path(args[0]), only_failed)
    print(f"страниц: {len(todo)}, браузер: {Path(exe).name}, корень на порту {port}")
    done = skipped = 0
    for i, page in enumerate(todo, 1):
        dest = OUT / f"{page.stem}.png"
        if dest.is_file() and not force:
            skipped += 1
            continue
        if shoot(exe, page, dest, port):
            done += 1
        if i % 25 == 0:
            print(f"  {i}/{len(todo)}")
    print(f"снято {done}, уже было {skipped}, папка {OUT}")


if __name__ == "__main__":
    main()
