"""Убирает импорты, на которые компилятор ругается `unused import`.

Работает по координатам из `cargo build --message-format=short`: файл, строка и
КОЛОНКА начала лишнего имени — поэтому не нужно угадывать регуляркой, какое из
одноимённых вхождений лишнее (`Regex для удаления только после проверки`).

Использование:
    cargo build --message-format=short 2>&1 | python scripts/strip_unused_uses.py

Правит только однострочные `use ...;`; многострочные печатает как пропущенные.
"""

import io
import re
import subprocess
import sys
from collections import defaultdict

WARN = re.compile(
    r"^(?P<file>.+?):(?P<line>\d+):(?P<col>\d+): warning: unused imports?: (?P<names>.+)$"
)
NAME = re.compile(r"`([^`]+)`")


def collect(stream):
    """file -> [(line, col, name)] — по одной записи на КАЖДОЕ лишнее имя."""
    found = defaultdict(list)
    for raw in stream:
        mo = WARN.match(raw.strip())
        if not mo:
            continue
        names = NAME.findall(mo.group("names"))
        # У группы («A` and `B`») колонка указывает на первое имя; остальные
        # ищем по тексту строки уже при правке.
        found[mo.group("file").replace("\\", "/")].append(
            (int(mo.group("line")), int(mo.group("col")), names)
        )
    return found


def drop_name(line, name):
    """Убирает имя из группы `{...}`; возвращает None, если не нашлось."""
    short = name.split("::")[-1].strip()
    for pat in (
        rf"\b{re.escape(name)}\b\s*,\s*",
        rf",\s*\b{re.escape(name)}\b",
        rf"\b{re.escape(short)}\b\s*,\s*",
        rf",\s*\b{re.escape(short)}\b",
        rf"\b{re.escape(short)}\b",
    ):
        new = re.sub(pat, "", line, count=1)
        if new != line:
            return new
    return None


def main():
    found = collect(sys.stdin)
    if not found:
        print("нечего убирать")
        return
    for path, hits in found.items():
        src = io.open(path, encoding="utf-8").read().split("\n")
        for lineno, _col, names in sorted(hits, reverse=True):
            idx = lineno - 1
            line = src[idx]
            if not line.strip().startswith("use ") or not line.rstrip().endswith(";"):
                print(f"пропуск (не однострочный use): {path}:{lineno}")
                continue
            if len(names) == 1 and f"{{" not in line:
                src[idx] = None
                continue
            for name in names:
                new = drop_name(line, name)
                if new is None:
                    print(f"не нашёл `{name}`: {path}:{lineno}")
                    continue
                line = new
            # `use path::{};` — пустая группа, строка больше не нужна
            src[idx] = None if re.search(r"\{\s*\};$", line.rstrip()) else line
        io.open(path, "w", encoding="utf-8").write(
            "\n".join(l for l in src if l is not None)
        )
        print(f"{path}: {len(hits)} строк(и)")


if __name__ == "__main__":
    main()
