# -*- coding: utf-8 -*-
"""Заменить ТЕЛО досье (`ours.md`/`original.md`) из черновика, сохранив
секцию «## Дополнение атрибутов (цикл N)».

    python parity/merge_body.py <draft.md> [--side ours|original]

Черновик — блоки:

    ### 113-monaco-editor-host
    # 113 monaco-editor-host — наша реализация
    ... (полный markdown тела) ...

Нужен, когда описание не «неполное», а УСТАРЕВШЕЕ: элемент давно реализован,
а `ours.md` всё ещё пишет «НЕ РЕАЛИЗОВАНО» — из-за такой лжи ревьюеры
тратят круг на несуществующее расхождение.
"""
import io
import os
import re
import sys


def keep_supplement(path: str) -> str:
    if not os.path.exists(path):
        return ""
    text = io.open(path, encoding="utf-8").read()
    m = re.search(r"^## Дополнение атрибутов \(цикл \d+\)\n.*?(?=^## |\Z)", text, re.M | re.S)
    return ("\n\n" + m.group(0).rstrip() + "\n") if m else ""


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    draft = sys.argv[1]
    side = "ours"
    if "--side" in sys.argv:
        side = sys.argv[sys.argv.index("--side") + 1]
    fname = "ours.md" if side == "ours" else "original.md"
    root = os.path.dirname(os.path.abspath(__file__))

    blocks: dict[str, list[str]] = {}
    elem = None
    for line in io.open(draft, encoding="utf-8").read().splitlines():
        if line.startswith("### ") and re.match(r"### \d", line):
            elem = line[4:].strip()
            blocks[elem] = []
        elif elem is not None:
            blocks[elem].append(line)

    done, missing = 0, []
    for elem, lines in blocks.items():
        d = os.path.join(root, elem)
        if not os.path.isdir(d):
            missing.append(elem)
            continue
        path = os.path.join(d, fname)
        body = "\n".join(lines).strip() + "\n"
        io.open(path, "w", encoding="utf-8").write(body + keep_supplement(path))
        done += 1
    print(f"переписано тел: {done}")
    if missing:
        print("нет таких папок: " + ", ".join(missing))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
