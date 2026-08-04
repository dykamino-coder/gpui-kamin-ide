"""Выносит группу вариантов `ShellEvent` во вложенный enum.

`host/events.rs` — одна декларация на 200 вариантов: джуну там не видно, какие
события про что. Группируем по домену (`plan/100-refactor-250.md`):

    ShellEvent::SetPref(k, v)  →  ShellEvent::Cz(CzEvent::SetPref(k, v))

Использование:
    python scripts/split_enum.py <Обёртка> <SubEnum> <модуль> <Вариант>...

Переписывание идёт СО СЧЁТОМ СКОБОК: у варианта с полями закрывающая скобка
ищется парой к открывающей, а не регуляркой (правило «regex только после
проверки, что он затронет»). Полноту гарантирует компилятор: пропущенный сайт
не соберётся.
"""

import glob
import io
import os
import re
import sys

EVENTS = "crates/shell/src/host/events.rs"
PAIR = {"(": ")", "{": "}", "[": "]"}


def match_close(text, i):
    """Индекс скобки, парной открывающей в `text[i]` (учитывает строки)."""
    stack = [text[i]]
    j = i + 1
    while j < len(text) and stack:
        c = text[j]
        if c == '"':
            j += 1
            while j < len(text) and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
        elif c in PAIR:
            stack.append(c)
        elif c in PAIR.values():
            stack.pop()
        j += 1
    return j - 1


def cut_variants(names):
    """Вырезает варианты из `ShellEvent` вместе с их doc-комментариями."""
    lines = io.open(EVENTS, encoding="utf-8").read().split("\n")
    start = next(k for k, l in enumerate(lines) if l.startswith("pub enum ShellEvent"))
    head = re.compile(r"^    ([A-Z]\w*)\b")
    taken, moved, i = set(), [], start + 1
    while i < len(lines) and lines[i] != "}":
        mo = head.match(lines[i])
        if mo and mo.group(1) in names:
            top = i
            while top - 1 > start and lines[top - 1].strip().startswith("///"):
                top -= 1
            while top in taken:
                top += 1
            depth, end = 0, i
            while True:
                depth += sum(lines[end].count(c) - lines[end].count(PAIR[c]) for c in "({[")
                if depth <= 0 and lines[end].rstrip().endswith(","):
                    break
                end += 1
            moved.append("\n".join(lines[top : end + 1]))
            taken.update(range(top, end + 1))
            i = end + 1
        else:
            i += 1
    rest = [l for n, l in enumerate(lines) if n not in taken]
    return rest, sum(1 for n in range(min(taken)) if n not in taken), moved


def rewrite(names, wrapper, sub):
    """`ShellEvent::Name(..)` → `ShellEvent::Wrapper(Sub::Name(..))`."""
    rx = re.compile(r"\bShellEvent::(" + "|".join(names) + r")\b")
    touched = 0
    for f in glob.glob("crates/shell/src/**/*.rs", recursive=True):
        if os.path.normpath(f) == os.path.normpath(EVENTS):
            continue
        s = io.open(f, encoding="utf-8").read()
        if not rx.search(s):
            continue
        out, pos = [], 0
        while True:
            mo = rx.search(s, pos)
            if not mo:
                out.append(s[pos:])
                break
            out.append(s[pos : mo.start()])
            name = mo.group(1)
            after = mo.end()
            nxt = after
            while nxt < len(s) and s[nxt] in " \n\t":
                nxt += 1
            if nxt < len(s) and s[nxt] in "({":
                close = match_close(s, nxt)
                out.append(f"ShellEvent::{wrapper}({sub}::{name}")
                out.append(s[after : close + 1])
                out.append(")")
                pos = close + 1
            else:
                out.append(f"ShellEvent::{wrapper}({sub}::{name})")
                pos = after
        s2 = "".join(out)
        if f"use crate::host::events_" not in s2 and f"{sub}" in s2:
            s2 = add_import(s2, sub)
        io.open(f, "w", encoding="utf-8").write(s2)
        touched += 1
    return touched


def add_import(src, sub):
    """Добавляет `use` рядом с первым существующим `use`."""
    lines = src.split("\n")
    at = next((k for k, l in enumerate(lines) if l.startswith("use ")), None)
    imp = f"use crate::host::events::{sub};"
    if at is None or imp in src:
        return src
    lines.insert(at, imp)
    return "\n".join(lines)


def main():
    wrapper, sub, module = sys.argv[1], sys.argv[2], sys.argv[3]
    names = sys.argv[4:]
    rest, pos, moved = cut_variants(set(names))
    if len(moved) != len(names):
        got = {m.strip().split("(")[0].split(" ")[-1] for m in moved}
        print("не найдены:", sorted(set(names) - got))
        return 1
    rest[pos:pos] = [
        f"    /// События домена `{module}` — вложенный enum, чтобы корневой",
        "    /// список оставался читаемым.",
        f"    {wrapper}({sub}),",
    ]
    io.open(EVENTS, "w", encoding="utf-8").write("\n".join(rest))
    head = (
        f"//! Вариантты `ShellEvent` домена `{module}`\n"
        "//! (`plan/100-refactor-250.md`).\n\n"
        "use super::events::*;\n"
        "use serde_json::Value;\n\n"
        "#[derive(Clone)]\n"
        f"pub enum {sub} {{\n"
    )
    io.open(f"crates/shell/src/host/{module}.rs", "w", encoding="utf-8").write(
        head + "\n".join(moved) + "\n}\n"
    )
    print("файлов правлено:", rewrite(names, wrapper, sub))
    return 0


if __name__ == "__main__":
    sys.exit(main())
