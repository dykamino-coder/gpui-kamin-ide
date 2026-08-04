# -*- coding: utf-8 -*-
"""Перенос куска файла в новый модуль — механически и одинаково.

Разбор больших файлов (`plan/100-refactor-250.md`) состоит из одной и той же
операции: «взять строки A..B, положить в новый модуль с шапкой, оставить в
исходнике вызов/ничего». Руками это делается с ошибками (разрезанный
doc-комментарий, забытый `use`), поэтому операция вынесена сюда.

    python scripts/split_module.py <src.rs> <A> <B> <dst.rs> "<док-строка>"

A и B — 1-based, включительно. Скрипт:
  * не даёт разрезать элемент по живому: если строка A начинается с `///`
    или строка B+1 начинается с `///`, он падает с объяснением;
  * копирует блок `use` исходного файла в новый модуль (лишние уберёт
    `cargo build` предупреждениями, недостающие — ошибками);
  * печатает, сколько строк перенесено и сколько осталось.
"""
from __future__ import annotations

import io
import os
import sys


def collect_uses(lines: list[str]) -> str:
    """Все `use`-объявления файла, включая многострочные (`use a::{
...}`)."""
    out: list[str] = []
    depth = 0
    inside = False
    for line in lines:
        if not inside and (line.startswith("use ") or line.startswith("pub use ")):
            inside = True
            depth = 0
        if inside:
            out.append(line)
            depth += line.count("{") - line.count("}")
            if depth <= 0 and line.rstrip().endswith(";"):
                inside = False
    return "".join(out)


def main() -> int:
    if len(sys.argv) < 6:
        print(__doc__)
        return 2
    src, a, b, dst, doc = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4], sys.argv[5]
    lines = io.open(src, encoding="utf-8").read().splitlines(keepends=True)

    # Разрез внутри doc-блока: начало куска продолжает чужой комментарий…
    if a > 1 and lines[a - 2].lstrip().startswith("///"):
        print(f"строка {a - 1} — doc-комментарий: кусок начинается внутри блока")
        return 1
    # …или кусок кончается «висячим» doc-комментарием без своего элемента.
    if lines[b - 1].lstrip().startswith("///"):
        print(f"строка {b} — doc-комментарий: элемент остался за границей")
        return 1

    block = "".join(lines[a - 1 : b])
    uses = collect_uses(lines)
    header = f"//! {doc}\n//!\n//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).\n\n{uses}\n"
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    io.open(dst, "w", encoding="utf-8").write(header + block)
    rest = "".join(lines[: a - 1] + lines[b:])
    io.open(src, "w", encoding="utf-8").write(rest)
    print(
        f"перенесено {b - a + 1} строк → {dst}; в {src} осталось {rest.count(chr(10))}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
