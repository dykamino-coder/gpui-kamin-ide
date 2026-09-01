"""Разностный разбор двух сводов: флипы порога И числовой дрейф.

`repdiff.py` считает только пересечения порога 0.50 — правка, которая
ухудшила два десятка пар, не сдвинув ни одну через границу, проходит у него
как «0 и 0». Этот разбор показывает и такую деградацию.

    python scripts/repdrift.py старый.txt новый.txt [допуск]

Допуск — в процентных пунктах, по умолчанию 0.05: расхождение меньше него
считается шумом набора. Нечисловые вердикты («красное видно» и родня)
приводятся к 99.0, как в `repdiff.py`.
"""

import io
import sys

THRESHOLD = 0.50
NON_NUMERIC = 99.0


def load(path):
    out = {}
    text = io.open(path, encoding="utf-8", errors="replace", newline="").read()
    for line in text.replace("\r\n", "\n").split("\n"):
        if not line.strip():
            continue
        parts = line.split("|")
        if len(parts) < 3:
            continue
        try:
            value = float(parts[2])
        except ValueError:
            value = NON_NUMERIC
        out[parts[0]] = value
    return out


def name(path):
    return path.replace("\\", "/").rsplit("/", 1)[-1]


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 1
    old = load(sys.argv[1])
    new = load(sys.argv[2])
    tol = float(sys.argv[3]) if len(sys.argv) > 3 else 0.05

    fixed, broken, worse, better = [], [], [], []
    for key, now in new.items():
        was = old.get(key)
        if was is None:
            continue
        if was > THRESHOLD >= now:
            fixed.append((key, was, now))
        elif was <= THRESHOLD < now:
            broken.append((key, was, now))
        elif now - was > tol:
            worse.append((key, was, now))
        elif was - now > tol:
            better.append((key, was, now))

    green_old = sum(1 for v in old.values() if v <= THRESHOLD)
    green_new = sum(1 for v in new.values() if v <= THRESHOLD)
    print(f"зелёных {green_old} -> {green_new}")
    print(f"регрессий {len(broken)} починено {len(fixed)}")
    print(f"дрейф: хуже {len(worse)}, лучше {len(better)} (допуск {tol})")
    for tag, rows in (("REG", broken), ("FIX", fixed)):
        for key, was, now in sorted(rows, key=lambda r: -(r[2] - r[1])):
            print(f"  {tag} {name(key)} {was} -> {now}")
    for key, was, now in sorted(worse, key=lambda r: r[1] - r[2])[:20]:
        print(f"  ХУЖЕ {name(key)} {was} -> {now}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
