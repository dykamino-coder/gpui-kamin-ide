"""Снимки страниц НЕСКОЛЬКИМИ настоящими браузерами — честный эталон.

Стенд сравнивает нас с нами же: и тест, и эталон рисует наш движок, поэтому
пара, где обе стороны сломаны одинаково, выглядит зелёной. Здесь те же
страницы снимают Chrome, Edge и Firefox — если два движка согласны между
собой, а мы нет, дефект наш и место видно на глаз.

    python scripts/wpt_browser_shots.py target/L-grid.txt --only-failed
    python scripts/wpt_browser_shots.py target/rep.txt --browsers=chrome,firefox

Снимки: `target/wpt-browsers/<браузер>/<имя>.png`. Chrome и Edge снимают в
том же масштабе, что стенд (800×600 при 1.25 → 1000×750). Firefox масштаб
окна не принимает: он снимает 800×600, и для наложения картинку надо
растянуть в 1.25 раза (сравнивать удобнее по ГЕОМЕТРИИ блоков, а не по
точкам).

    --only-failed  снимать только непройденные пары (третья колонка отчёта)
    --force        переснять даже то, что уже снято
    --browsers=    список через запятую: chrome, edge, firefox (по умолчанию все)
"""

import functools
import http.server
import socketserver
import subprocess
import sys
import threading
from pathlib import Path

ROOT = Path("vendor/wpt-parsing").resolve()
OUT = Path("target/wpt-browsers")
WIDTH, HEIGHT, SCALE = 800, 600, 1.25

EXES = {
    "chrome": [r"C:\Program Files\Google\Chrome\Application\chrome.exe"],
    "edge": [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ],
    "firefox": [r"C:\Program Files\Mozilla Firefox\firefox.exe"],
}


def found(name: str) -> str | None:
    for path in EXES.get(name, []):
        if Path(path).is_file():
            return path
    return None


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
    """Отдать корень набора по HTTP и вернуть порт: тесты просят /fonts/ahem.css."""
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(ROOT))
    handler.log_message = lambda *_: None
    httpd = socketserver.ThreadingTCPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    return httpd.server_address[1]


def url_of(page: Path, port: int) -> str:
    try:
        rel = page.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return "file:///" + str(page.resolve()).replace("\\", "/")
    return f"http://127.0.0.1:{port}/{rel}"


def shoot(name: str, exe: str, page: Path, dest: Path, port: int) -> bool:
    dest.parent.mkdir(parents=True, exist_ok=True)
    url = url_of(page, port)
    if name == "firefox":
        # У Firefox свой набор ключей: снимок по `--screenshot <файл>`, окно
        # логическими точками. Масштаб окна он не принимает — картинка выходит
        # 800×600 и при наложении растягивается.
        cmd = [
            exe,
            "--headless",
            f"--window-size={WIDTH},{HEIGHT}",
            "--screenshot",
            str(dest.resolve()),
            url,
        ]
    else:
        cmd = [
            exe,
            "--headless=new",
            "--disable-gpu",
            "--hide-scrollbars",
            "--virtual-time-budget=3000",
            f"--force-device-scale-factor={SCALE}",
            f"--window-size={WIDTH},{HEIGHT}",
            f"--screenshot={dest.resolve()}",
            url,
        ]
    try:
        subprocess.run(cmd, capture_output=True, timeout=90)
    except subprocess.TimeoutExpired:
        return False
    return dest.is_file()


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if not args:
        raise SystemExit(__doc__)
    only_failed = "--only-failed" in sys.argv
    force = "--force" in sys.argv
    want = ["chrome", "edge", "firefox"]
    for a in sys.argv[1:]:
        if a.startswith("--browsers="):
            want = [w.strip() for w in a.split("=", 1)[1].split(",") if w.strip()]
    port = serve()
    todo = pages(Path(args[0]), only_failed)
    print(f"страниц: {len(todo)}, корень на порту {port}")
    for name in want:
        exe = found(name)
        if not exe:
            print(f"  {name}: не найден, пропускаю")
            continue
        done = skipped = 0
        for i, page in enumerate(todo, 1):
            dest = OUT / name / f"{page.stem}.png"
            if dest.is_file() and not force:
                skipped += 1
                continue
            if shoot(name, exe, page, dest, port):
                done += 1
            if i % 25 == 0:
                print(f"  {name}: {i}/{len(todo)}")
        print(f"  {name}: snyato {done}, bylo {skipped}, papka {OUT / name}")


if __name__ == "__main__":
    main()
