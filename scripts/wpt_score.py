"""Счёт по разделам WPT: сколько пар сходится и почему остальные — нет.

    python scripts/wpt_score.py                    # свод по последним отчётам
    python scripts/wpt_score.py --diff стар нов    # что позеленело и что сломалось

Отчёты кладёт прогон `wptrun` (`target/wpt-report-*.txt`), формат строки —
`тест|эталон|процент`. Порог тот же, что у стенда: 0.5%.

Провалы делятся на три корзины, потому что чинятся они по-разному: за JS
(тест сам считает раскладку скриптом), черновые фичи сетки (`masonry`,
`subgrid`, `grid-lanes` — их нет ни в одном движке) и всё остальное — то, что
действительно наше.
"""

import re
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent
THRESHOLD = 0.5
DRAFT = ("grid-lanes", "masonry", "subgrid")


def load(path: Path) -> dict[str, float]:
    out: dict[str, float] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line:
            continue
        parts = line.split("|")
        try:
            out[parts[0]] = float(parts[-1])
        except ValueError:
            # Не число — это «пустая страница» или «снимок не получен».
            # Пропускать такую строку нельзя: она уйдёт из знаменателя и
            # завысит долю зелёных. Считаем полным расхождением.
            out[parts[0]] = 100.0
    return out


def bucket(test: str) -> str:
    try:
        src = Path(test).read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return "наше"
    # Скрипт бывает не только тегом. `<body onload="…scrollTop = 200">`
    # выставляет прокрутку до снимка, и без выполнения обработчика тест
    # заведомо показывает не то — правкой раскладки это не лечится.
    if re.search(r"<script", src, re.I) or re.search(r"\son[a-z]{3,}\s*=", src, re.I):
        return "за JS"
    if any(word in src for word in DRAFT):
        return "черновик"
    return "наше"


def report(path: Path) -> None:
    rows = load(path)
    green = sum(1 for v in rows.values() if v <= THRESHOLD)
    kinds = {"за JS": 0, "черновик": 0, "наше": 0}
    for test, value in rows.items():
        if value > THRESHOLD:
            kinds[bucket(test)] += 1
    tail = ", ".join(f"{k}: {n}" for k, n in kinds.items() if n)
    print(f"{path.stem:32} {green:5}/{len(rows):<5} провалы — {tail}")


def diff(old: Path, new: Path) -> None:
    a, b = load(old), load(new)
    keys = [k for k in b if k in a]
    fixed = [(a[k], b[k], k) for k in keys if a[k] > THRESHOLD >= b[k]]
    broken = [(a[k], b[k], k) for k in keys if a[k] <= THRESHOLD < b[k]]
    print(
        f"зелёных {sum(1 for k in keys if a[k] <= THRESHOLD)} ->"
        f" {sum(1 for k in keys if b[k] <= THRESHOLD)} из {len(keys)};"
        f" позеленело {len(fixed)}, сломалось {len(broken)}"
    )
    for was, now, test in sorted(broken, key=lambda t: -t[1])[:20]:
        print(f"  СЛОМАНО {was:6.2f} -> {now:6.2f}  {Path(test).name}")
    for was, now, test in sorted(fixed, key=lambda t: -t[0])[:20]:
        print(f"  ПОЧИНЕНО {was:6.2f} -> {now:6.2f}  {Path(test).name}")


def main() -> None:
    if "--diff" in sys.argv:
        at = sys.argv.index("--diff")
        diff(Path(sys.argv[at + 1]), Path(sys.argv[at + 2]))
        return
    if len(sys.argv) > 1:
        for name in sys.argv[1:]:
            report(Path(name))
        return
    reports = sorted((ROOT / "target").glob("wpt-report-*.txt"))
    if not reports:
        print("нет отчётов: сначала прогон wptrun")
        return
    for path in reports:
        report(path)


main()
