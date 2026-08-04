# -*- coding: utf-8 -*-
"""Влить черновик дозаполнения атрибутов в досье.

    python parity/merge_fill.py <draft.md> [--cycle 10]

Черновик — плоский файл блоков:

    ### 02-titlebar-left-cluster
    #### original
    - гэпы: gap 8 (`Titlebar.module.css:31`)
    #### ours
    - цвета: text-secondary #adb3c7 (`palette.rs:24`)

Каждый блок дописывается в `original.md` / `ours.md` элемента отдельной
секцией «## Дополнение атрибутов (цикл N)». Повторный прогон ЗАМЕНЯЕТ ранее
влитую секцию того же цикла, а не плодит копии.
"""
import io
import os
import re
import sys


def parse(path: str) -> dict[str, dict[str, list[str]]]:
    out: dict[str, dict[str, list[str]]] = {}
    elem = side = None
    for line in io.open(path, encoding="utf-8").read().splitlines():
        if line.startswith("### "):
            elem = line[4:].strip()
            side = None
            out.setdefault(elem, {})
        elif line.startswith("#### ") and elem:
            side = line[5:].strip().lower()
            out[elem].setdefault(side, [])
        elif elem and side and line.strip():
            out[elem][side].append(line.rstrip())
    return {e: {s: v for s, v in sides.items() if v} for e, sides in out.items() if sides}


def family(line: str) -> str:
    """«- цвета: …» → «цвета» (ключ дедупликации внутри секции)."""
    m = re.match(r"^-\s*([^:]+):", line)
    return m.group(1).strip().lower() if m else line.strip().lower()


def apply(path: str, lines: list[str], cycle: int) -> None:
    head = f"## Дополнение атрибутов (цикл {cycle})"
    text = io.open(path, encoding="utf-8").read() if os.path.exists(path) else ""
    rx = re.compile(rf"^{re.escape(head)}\n(.*?)(?=^## |\Z)", re.M | re.S)
    old = rx.search(text)
    # Секция ДОПОЛНЯЕТСЯ, а не переписывается: второй черновик того же цикла
    # (добивка N/A) иначе стирал строки первого.
    merged: list[str] = []
    seen: set[str] = set()
    for src in ((old.group(1).splitlines() if old else []), lines):
        for ln in src:
            if not ln.strip():
                continue
            key = family(ln)
            if key in seen:
                continue
            seen.add(key)
            merged.append(ln)
    body = head + "\n\n" + "\n".join(merged) + "\n"
    text = rx.sub("", text).rstrip() + "\n\n" + body
    io.open(path, "w", encoding="utf-8").write(text)


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    draft = sys.argv[1]
    cycle = 10
    if "--cycle" in sys.argv:
        cycle = int(sys.argv[sys.argv.index("--cycle") + 1])
    root = os.path.dirname(os.path.abspath(__file__))
    blocks = parse(draft)
    done = 0
    missing = []
    for elem, sides in blocks.items():
        d = os.path.join(root, elem)
        if not os.path.isdir(d):
            missing.append(elem)
            continue
        for side, lines in sides.items():
            fname = "original.md" if side.startswith("orig") else "ours.md"
            apply(os.path.join(d, fname), lines, cycle)
            done += 1
    print(f"влито блоков: {done} в {len(blocks) - len(missing)} элементов")
    if missing:
        print("нет таких папок: " + ", ".join(missing))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
