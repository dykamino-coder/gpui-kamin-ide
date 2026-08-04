# -*- coding: utf-8 -*-
"""Пересборка `INDEX.md` — сводной таблицы всех 159 элементов.

    python parity/index.py

Читает вердикты (`verdict.md`), наличие файлов досье и кадров и печатает
единую таблицу по зонам: элемент → вердикт → цикл → ссылки на original.md,
ours.md, кадры. Держать вручную бессмысленно: после каждого цикла счётчики
меняются, а расхождение таблицы с гейтом уже один раз выдавало «51 MATCH»,
когда в гейте было 65.
"""
import io
import os
import re

ZONES = [
    (1, 19, "Титлбар"),
    (20, 37, "Сайдбар"),
    (38, 51, "Activity, рейлы, стрипы"),
    (52, 91, "Панели и экраны"),
    (92, 107, "Дерево файлов"),
    (108, 129, "Редактор, оверлеи, статус"),
    (130, 159, "Токены, семплы, глобальные стили"),
]

HEAD = re.compile(r"^## Цикл (\d+): ([A-ZА-Я/ ()]+)$", re.M)


def verdict(path: str) -> tuple[str, str]:
    if not os.path.exists(path):
        return "—", "—"
    heads = HEAD.findall(io.open(path, encoding="utf-8").read())
    if not heads:
        return "—", "—"
    cyc, verd = heads[-1]
    return verd.strip(), cyc


def main() -> int:
    root = os.path.dirname(os.path.abspath(__file__))
    dirs = sorted(
        (d for d in os.listdir(root) if os.path.isdir(os.path.join(root, d)) and d[0].isdigit()),
        key=lambda d: int(d.split("-")[0]),
    )
    rows = {}
    for d in dirs:
        num = int(d.split("-")[0])
        verd, cyc = verdict(os.path.join(root, d, "verdict.md"))
        have = {
            f: os.path.exists(os.path.join(root, d, f))
            for f in ("original.md", "ours.md", "original.png", "ours.png")
        }
        # Пара одинаковых кадров — не пара (общий кроп зоны вместо элемента)
        if have["original.png"] and have["ours.png"]:
            blobs = [
                io.open(os.path.join(root, d, f), "rb").read()
                for f in ("original.png", "ours.png")
            ]
            if blobs[0] == blobs[1]:
                have["original.png"] = have["ours.png"] = False
        rows[num] = (d, verd, cyc, have)

    total_match = sum(1 for _, v, _, _ in rows.values() if v.startswith("MATCH"))
    out = [
        "# Инвентарь визуальной сверки — 159 элементов",
        "",
        "Одна папка на элемент: `original.md` (описание + код оригинала), `ours.md` "
        "(наша сторона), `original.png` / `ours.png` (кадры обеих сторон), "
        "`verdict.md` (история циклов ревью).",
        "",
        "Таблица генерируется: `python parity/index.py`. Гейт: `python parity/gate.py`. "
        "Задание на следующий цикл: `python parity/cycle.py`. Полнота описаний "
        "(отступы/цвета/гэпы/скругления/шрифты/ховер): `python parity/attrs.py`.",
        "",
        f"**Статус: {total_match} MATCH / {len(rows) - total_match} DIVERGES из {len(rows)}.**",
        "",
    ]
    for lo, hi, title in ZONES:
        zone = [rows[n] for n in sorted(rows) if lo <= n <= hi]
        m = sum(1 for _, v, _, _ in zone if v.startswith("MATCH"))
        out += [
            f"## {lo}-{hi} — {title}  ({m} MATCH / {len(zone) - m} DIVERGES)",
            "",
            "| # | элемент | вердикт | цикл | original.md | ours.md | original.png | ours.png |",
            "|---|---|---|---|---|---|---|---|",
        ]
        for n in sorted(rows):
            if not (lo <= n <= hi):
                continue
            d, verd, cyc, have = rows[n]
            slug = d.split("-", 1)[1]
            mark = lambda ok: "✔" if ok else "—"  # noqa: E731
            out.append(
                f"| {n} | [{slug}]({d}/) | {verd} | {cyc} | "
                f"[{mark(have['original.md'])}]({d}/original.md) | "
                f"[{mark(have['ours.md'])}]({d}/ours.md) | "
                f"{mark(have['original.png'])} | {mark(have['ours.png'])} |"
            )
        out.append("")
    io.open(os.path.join(root, "INDEX.md"), "w", encoding="utf-8").write("\n".join(out))
    print(f"INDEX.md: {total_match} MATCH / {len(rows) - total_match} DIVERGES")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
