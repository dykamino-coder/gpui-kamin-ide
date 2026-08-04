# -*- coding: utf-8 -*-
"""Симметричность досье по шести семействам атрибутов.

    python parity/attrs.py            # сводка + список пробелов
    python parity/attrs.py --zone 38 51

Требование сверки — «отступы, цвета хекс, гапы, скругления, шрифты, фоны по
ховеру абсолютно всего». Гейт проверяет ВЕРДИКТЫ, этот скрипт — что описания
сопоставимы: КАЖДОЕ семейство, выписанное в `original.md`, обязано быть
разобрано и в `ours.md`. Иначе элемент нельзя честно объявить MATCH — часть
атрибутов просто никто не сверял.

Плюс минимум для `original.md`: цвета и отступы должны быть выписаны всегда
(элемент без единого hex и без единого паддинга описан слишком грубо).
Законный пропуск помечается в файле строкой `N/A: <семейство>`.
"""
import io
import os
import re
import sys

FAMILIES = {
    "отступы": re.compile(r"padding|padding-\w+|\bp[xytblr]?\s|отступ|инсет|margin", re.I),
    "гэпы": re.compile(r"\bgap\b|гэп|зазор", re.I),
    "цвета": re.compile(r"#[0-9a-fA-F]{3,8}\b|--(?:bg|text|accent|editor|glint)-[a-z-]+", re.I),
    "скругления": re.compile(r"radius|скругл|\br[a-z]*-(?:xs|sm|md|lg)\b", re.I),
    "шрифты": re.compile(r"font-size|font-weight|\bfs-(?:xs|sm|md|lg|xl)\b|кегл|weight|моно", re.I),
    "ховер": re.compile(r"hover|ховер", re.I),
}


def gaps(path: str) -> list[str]:
    if not os.path.exists(path):
        return list(FAMILIES)
    text = io.open(path, encoding="utf-8").read()
    # `N/A: <семейство> — пояснение`: ключ — только слово до тире/двоеточия
    na = {
        re.split(r"[\u2014\u2013:-]", m, maxsplit=1)[0].strip().lower()
        for m in re.findall(r"N/A:\s*([^\n]+)", text)
    }
    out = []
    for name, rx in FAMILIES.items():
        if name in na:
            continue
        if not rx.search(text):
            out.append(name)
    return out


def main() -> int:
    root = os.path.dirname(os.path.abspath(__file__))
    lo, hi = 1, 999
    if "--zone" in sys.argv:
        i = sys.argv.index("--zone")
        lo, hi = int(sys.argv[i + 1]), int(sys.argv[i + 2])
    dirs = sorted(
        (d for d in os.listdir(root) if os.path.isdir(os.path.join(root, d)) and d[0].isdigit()),
        key=lambda d: int(d.split("-")[0]),
    )
    bad = 0
    total = 0
    for d in dirs:
        num = int(d.split("-")[0])
        if not (lo <= num <= hi):
            continue
        total += 1
        op = os.path.join(root, d, "original.md")
        up = os.path.join(root, d, "ours.md")
        # какие семейства ЕСТЬ в оригинале — те и обязаны быть у нас
        in_orig = [f for f in FAMILIES if f not in gaps(op)]
        missing_ours = [f for f in in_orig if f in gaps(up)]
        thin_orig = [f for f in ("цвета", "отступы") if f in gaps(op)]
        if missing_ours or thin_orig:
            bad += 1
            parts = []
            if thin_orig:
                parts.append("original слишком грубо: " + ", ".join(thin_orig))
            if missing_ours:
                parts.append("ours не разбирает: " + ", ".join(missing_ours))
            print(f"{d}: {'; '.join(parts)}")
    print()
    print(f"симметрично описано: {total - bad}/{total}")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
