# -*- coding: utf-8 -*-
"""Убрать одиночные `use path::name;`, если имя уже пришло групповым `use`.

`wire_imports.py` дописывает импорты по одному, а разбор файла часто уже
принёс те же имена пачкой (`use a::{b, c}`) — компилятор ругается на
повторный импорт. Скрипт оставляет ПЕРВОЕ вхождение имени и убирает
последующие одиночные строки.
"""
from __future__ import annotations

import glob
import io
import re
import sys

SINGLE = re.compile(r"^(pub(\([a-z]+\))? )?use ([\w:]+)::(\w+);\s*$")
GROUP = re.compile(r"^(pub(\([a-z]+\))? )?use ([\w:]+)::\{([^}]*)\}", re.S)


def names_of(line: str) -> set[str]:
    m = GROUP.match(line)
    if m:
        return {p.strip().split(" as ")[0] for p in m.group(4).split(",") if p.strip()}
    m = SINGLE.match(line)
    return {m.group(4)} if m else set()


def dedupe(path: str) -> int:
    lines = io.open(path, encoding="utf-8").read().splitlines(keepends=True)
    seen: set[str] = set()
    out: list[str] = []
    removed = 0
    buf = ""
    for line in lines:
        chunk = buf + line
        if chunk.lstrip().startswith(("use ", "pub use ")) and chunk.count("{") > chunk.count("}"):
            buf = chunk
            continue
        buf = ""
        got = names_of(chunk)
        if got and SINGLE.match(chunk) and got & seen:
            removed += 1
            continue
        seen |= got
        out.append(chunk)
    if removed:
        io.open(path, "w", encoding="utf-8").write("".join(out))
    return removed


def main() -> int:
    total = 0
    for path in glob.glob("crates/shell/src/**/*.rs", recursive=True):
        n = dedupe(path)
        if n:
            print(f"{path}: убрано {n}")
            total += n
    print("итого", total)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
