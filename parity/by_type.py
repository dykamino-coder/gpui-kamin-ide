# -*- coding: utf-8 -*-
"""Второй разрез инвентаря: по ТИПУ элемента, а не по зоне экрана.

`INDEX.md` группирует 159 досье по зонам (титлбар, сайдбар, панели…). Здесь —
тот же набор, но по видам: кнопки, иконки, инпуты и селекторы, плейсхолдеры,
меню и поповеры, строки списков, панели и карты, оверлеи, токены. Один
элемент может попасть в несколько разделов (кнопка-иконка — и туда, и туда).

    python parity/by_type.py     # перегенерировать BY_TYPE.md

Правило отнесения — по слагу папки; неопознанное падает в «Прочее», чтобы
пропажа была видна, а не растворялась.
"""
import io
import os
import re

ROOT = os.path.dirname(os.path.abspath(__file__))
HEAD = re.compile(r"^## Цикл (\d+): ([A-ZА-Я/ ()]+)$", re.M)

# (заголовок раздела, подстроки слага). Порядок важен: элемент попадает во ВСЕ
# подходящие разделы, поэтому специфичное идёт раньше общего.
TYPES = [
    ("Кнопки", ("button", "btn", "trigger", "toggle", "action", "control")),
    ("Иконки и глифы", ("icon", "glyph", "logo", "badge", "spinner", "dot")),
    ("Инпуты и селекторы", ("input", "search", "filter", "select", "dropdown",
                            "checkbox", "picker", "prompt")),
    ("Плейсхолдеры и пустые состояния", ("placeholder", "empty", "welcome",
                                         "skeleton", "loading", "error")),
    ("Меню и поповеры", ("menu", "popover", "submenu", "tooltip", "overflow")),
    ("Строки списков и деревьев", ("row", "item", "node", "tab", "chip",
                                   "entry", "tile")),
    ("Панели, карты и колонки", ("panel", "card", "column", "body", "view",
                                 "bar", "strip", "rail", "container", "wrapper",
                                 "header", "toolbar", "footer")),
    ("Оверлеи и модалки", ("modal", "overlay", "toast", "scrim", "palette",
                           "quick", "dialog")),
    ("Токены, семплы, глобальные стили", ("token", "sample", "typography",
                                          "color", "shadow", "radius",
                                          "spacing", "focus", "global")),
]


def last_verdict(path: str) -> tuple[str, str]:
    if not os.path.isfile(path):
        return ("?", "-")
    text = io.open(path, encoding="utf-8").read()
    hits = HEAD.findall(text)
    if not hits:
        return ("?", "-")
    cycle, state = hits[-1]
    return (state.strip(), cycle)


def main() -> None:
    dirs = sorted(
        (d for d in os.listdir(ROOT) if re.match(r"^\d+-", d)),
        key=lambda d: int(d.split("-", 1)[0]),
    )
    rows = []
    for d in dirs:
        num = int(d.split("-", 1)[0])
        slug = d.split("-", 1)[1]
        state, cycle = last_verdict(os.path.join(ROOT, d, "verdict.md"))
        rows.append((num, slug, d, state, cycle))

    used: set[int] = set()
    out = [
        "# Инвентарь по ТИПАМ элементов",
        "",
        "Тот же набор досье, что в `INDEX.md`, но разложенный по видам, а не по "
        "зонам экрана. В каждой папке: `original.md` (код и метрики оригинала), "
        "`ours.md` (наша реализация), `original.png` / `ours.png` (кадры), "
        "`verdict.md` (история циклов).",
        "",
        "Генерируется: `python parity/by_type.py`.",
        "",
    ]
    for title, keys in TYPES:
        picked = [r for r in rows if any(k in r[1] for k in keys)]
        if not picked:
            continue
        used.update(r[0] for r in picked)
        match = sum(1 for r in picked if r[3].startswith("MATCH"))
        out.append(f"## {title} — {match}/{len(picked)} MATCH")
        out.append("")
        out.append("| # | элемент | вердикт | цикл | досье |")
        out.append("|---|---|---|---|---|")
        for num, slug, d, state, cycle in picked:
            out.append(f"| {num} | {slug} | {state} | {cycle} | [{d}/]({d}/) |")
        out.append("")

    rest = [r for r in rows if r[0] not in used]
    if rest:
        out.append(f"## Прочее — {len(rest)}")
        out.append("")
        out.append("| # | элемент | вердикт | цикл | досье |")
        out.append("|---|---|---|---|---|")
        for num, slug, d, state, cycle in rest:
            out.append(f"| {num} | {slug} | {state} | {cycle} | [{d}/]({d}/) |")
        out.append("")

    io.open(os.path.join(ROOT, "BY_TYPE.md"), "w", encoding="utf-8").write(
        "\n".join(out)
    )
    print(f"BY_TYPE.md: {len(rows)} элементов, вне разделов {len(rest)}")


main()
