# -*- coding: utf-8 -*-
"""Дописать импорты после разбора файла на модули.

После `extract_items.py` компилятор жалуется «cannot find function `X`» —
элемент уехал в соседний модуль. Скрипт читает эти жалобы, ищет, где `X`
теперь объявлен, и дописывает `use` в пострадавший файл. При нужде делает
элемент `pub(crate)`.

    python scripts/wire_imports.py [число проходов]

Ничего не угадывает: если имя объявлено в двух местах — печатает оба и
оставляет решение человеку.
"""
from __future__ import annotations

import glob
import io
import re
import subprocess
import sys

SRC = "crates/shell/src"
MISSING = re.compile(
    r"(crates[\\/]shell[\\/]src[\\/][^:]+):\d+:\d+: error\[E04(?:25|22|12)\]: "
    r"cannot find (?:function|value|type|struct, variant or union type) `(\w+)`"
)
PRIVATE = re.compile(r"error\[E060[34]\]: (?:function|constant|static|struct) `?(\w+)`? is private")
DECL = r"^(pub(\([a-z]+\))? )?(fn|struct|enum|const|static|type) {}\b"


def declarations(name: str) -> list[str]:
    pat = re.compile(DECL.format(re.escape(name)), re.M)
    return [p for p in glob.glob(f"{SRC}/**/*.rs", recursive=True) if pat.search(io.open(p, encoding="utf-8").read())]


def module_path(path: str) -> str:
    rel = path.replace("\\", "/").split(SRC + "/")[1][:-3]
    parts = [p for p in rel.split("/") if p != "mod"]
    return "crate::" + "::".join(parts)


def add_use(path: str, line: str) -> None:
    s = io.open(path, encoding="utf-8").read()
    if line in s:
        return
    i = s.index("\n\n", s.index("//!")) + 2 if s.startswith("//!") else 0
    io.open(path, "w", encoding="utf-8").write(s[:i] + line + "\n" + s[i:])


def publicize(path: str, name: str) -> None:
    s = io.open(path, encoding="utf-8").read()
    s2 = re.sub(
        r"^(fn|struct|enum|const|static|type) " + re.escape(name) + r"\b",
        lambda m: "pub(crate) " + m.group(0),
        s,
        count=1,
        flags=re.M,
    )
    if s2 != s:
        io.open(path, "w", encoding="utf-8").write(s2)


def main() -> int:
    rounds = int(sys.argv[1]) if len(sys.argv) > 1 else 8
    for it in range(rounds):
        err = subprocess.run(
            ["cargo", "build", "--message-format=short"],
            capture_output=True, text=True, encoding="utf-8", errors="replace",
        ).stderr
        pairs = {(f.replace("\\", "/"), n) for f, n in MISSING.findall(err)}
        privs = set(PRIVATE.findall(err))
        for name in privs:
            for d in declarations(name):
                publicize(d, name)
        if not pairs and not privs:
            print("импорты сошлись, проход", it)
            return 0
        for path, name in pairs:
            decls = [d for d in declarations(name) if d.replace("\\", "/") != path]
            if len(decls) != 1:
                print(f"{name}: объявлений {len(decls)} — {decls}; пропускаю")
                continue
            publicize(decls[0], name)
            add_use(path, f"use {module_path(decls[0])}::{name};")
        print("проход", it, "имён", len(pairs), "приватных", len(privs))
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
