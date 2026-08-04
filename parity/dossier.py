# -*- coding: utf-8 -*-
"""Пересборка `DOSSIER.md` — ВСЕ элементы в одном файле.

    python parity/dossier.py

Собирает по каждому элементу: вердикт и историю циклов, оба кадра, полный
текст `original.md` и `ours.md` (включая секции «Дополнение атрибутов»).
Держать вручную нельзя: файл на ~14 тысяч строк расходится с папками после
первого же цикла (было «52 MATCH», в гейте 66).
"""
import io
import os
import re

ZONES = [
    (1, 19, "Титлбар"),
    (20, 37, "Сайдбар — сессии и Customize-нав"),
    (38, 51, "Activity-бар, рейлы, пикеры, стрипы"),
    (52, 91, "Панели, карты, экраны Customize, терминал"),
    (92, 107, "Дерево файлов и его меню"),
    (108, 129, "Редактор, оверлеи, статус-бар, модалки"),
    (130, 159, "Токены дизайна, sample-компоненты, глобальные стили"),
]

HEAD = re.compile(r"^## Цикл (\d+): ([A-ZА-Я/ ()]+)$", re.M)


def read(path: str) -> str:
    return io.open(path, encoding="utf-8").read().strip() if os.path.exists(path) else "—"


def main() -> int:
    root = os.path.dirname(os.path.abspath(__file__))
    dirs = sorted(
        (d for d in os.listdir(root) if os.path.isdir(os.path.join(root, d)) and d[0].isdigit()),
        key=lambda d: int(d.split("-")[0]),
    )
    rows = []
    for d in dirs:
        num = int(d.split("-")[0])
        heads = HEAD.findall(read(os.path.join(d if False else os.path.join(root, d), "verdict.md")))
        verd, cyc = (heads[-1][1].strip(), heads[-1][0]) if heads else ("—", "—")
        rows.append((num, d, verd, cyc, heads))

    match = sum(1 for r in rows if r[2].startswith("MATCH"))
    out = [
        "# ДОСЬЕ визуальной сверки — все 159 элементов в одном файле",
        "",
        "Для каждого элемента: описание и код ОРИГИНАЛА, наш код, оба кадра и вся "
        "история вердиктов по циклам. Источник правды по каждому элементу — "
        "одноимённая папка рядом; здесь всё сведено вместе.",
        "",
        "Генерируется: `python parity/dossier.py`. Краткая таблица — "
        "[INDEX.md](INDEX.md), перечень с файлами и классами — "
        "[INVENTORY.md](INVENTORY.md), гейт — `python parity/gate.py`, полнота "
        "атрибутов — `python parity/attrs.py`.",
        "",
        f"**Сводка: {match} MATCH / {len(rows) - match} DIVERGES из {len(rows)}.**",
        "",
        "## Оглавление по зонам",
        "",
    ]
    for lo, hi, title in ZONES:
        z = [r for r in rows if lo <= r[0] <= hi]
        zm = sum(1 for r in z if r[2].startswith("MATCH"))
        out.append(f"- **{lo}-{hi} {title}** — {len(z)} элементов, {zm} MATCH")
    out.append("")

    for lo, hi, title in ZONES:
        out += [f"# Зона {lo}-{hi} — {title}", ""]
        for num, d, verd, cyc, heads in rows:
            if not (lo <= num <= hi):
                continue
            slug = d.split("-", 1)[1]
            hist = ", ".join(f"ц{c}:{v.strip()}" for c, v in heads) or "—"
            out += [f"## {num}. {slug} — **{verd}** (цикл {cyc})", "", f"*История: {hist}*", ""]
            for f, cap in (("original.png", "оригинал"), ("ours.png", "наш")):
                p = os.path.join(root, d, f)
                out.append(f"![{cap}]({d}/{f})" if os.path.exists(p) else f"*кадр «{cap}» отсутствует*")
            out += [
                "",
                "### Оригинал",
                "",
                read(os.path.join(root, d, "original.md")),
                "",
                "### Наша реализация",
                "",
                read(os.path.join(root, d, "ours.md")),
                "",
                "### Вердикты",
                "",
                read(os.path.join(root, d, "verdict.md")),
                "",
                "---",
                "",
            ]
    io.open(os.path.join(root, "DOSSIER.md"), "w", encoding="utf-8").write("\n".join(out))
    print(f"DOSSIER.md: {match} MATCH / {len(rows) - match} DIVERGES, элементов {len(rows)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
