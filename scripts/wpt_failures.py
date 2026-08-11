"""Выборка провалов из отчёта — список пар для повторного прогона.

Зачем: полный прогон раздела идёт десятки минут, а после правки интересны
ровно те пары, что не сошлись. Гонять их одних — минуты, и цикл «правка →
замер» становится рабочим. Полный регресс нужен реже: он ловит обратное —
что правка сломала уже зелёное.

    python scripts/wpt_failures.py target/wpt-report-text-all-h1.txt \
        > target/wpt-pairs-fail.txt

Порог тот же, что у счёта. Строки, где вместо числа стоит ошибка стенда
(«пустая страница», «снимок не получен»), тоже попадают в выборку: их надо
пересматривать в первую очередь.
"""

import sys
from pathlib import Path

THRESHOLD = 0.5


def failures(report: Path) -> list[str]:
    out = []
    for line in report.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        parts = line.split("|")
        if len(parts) < 3:
            continue
        try:
            green = float(parts[-1]) <= THRESHOLD
        except ValueError:
            green = False
        if not green:
            out.append(f"{parts[0]}|{parts[1]}")
    return out


def main() -> None:
    if len(sys.argv) < 2:
        print("нужен путь к отчёту", file=sys.stderr)
        raise SystemExit(1)
    rows = []
    for name in sys.argv[1:]:
        rows.extend(failures(Path(name)))
    print("\n".join(rows))
    print(f"провалов: {len(rows)}", file=sys.stderr)


if __name__ == "__main__":
    main()
