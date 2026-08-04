# -*- coding: utf-8 -*-
"""Перенос ЦЕЛЫХ элементов (`fn`, `struct`, `const`, …) в новый модуль.

Резать по номерам строк оказалось опасно: граница то и дело падала между
doc-комментарием и его элементом, а однажды унесла закрывающую скобку
`impl`. Здесь границы считает сам скрипт — по началам элементов, поэтому
кусок всегда целый вместе со своим `///` и атрибутами.

    python scripts/extract_items.py <src.rs> <dst.rs> "<док-строка>" имя1 имя2…

Работает и со свободными элементами (отступ 0), и с методами внутри
`impl Тип {` (отступ 4): методы кладутся в новый файл, обёрнутые в
`impl Тип {}` — тела не меняются. Блок `use` исходника копируется целиком;
лишнее уберёт `cargo fix`.
"""
from __future__ import annotations

import io
import os
import re
import sys

ITEM = re.compile(
    r"^(?P<indent>[ ]*)(pub(\([a-z]+\))? )?"
    r"(async )?(unsafe )?(extern \"[A-Za-z]+\" )?"
    r"(fn|struct|enum|union|trait|impl|const|static|type|mod|macro_rules!)\b"
)
LEAD = re.compile(r"^\s*(///|//!|#\[|//)")


def item_spans(lines: list[str], indent: int) -> list[tuple[str, int, int]]:
    """(имя, первая строка, последняя строка) для элементов заданного отступа."""
    starts: list[tuple[str, int]] = []
    for i, line in enumerate(lines):
        m = ITEM.match(line)
        if not m or len(m.group("indent")) != indent:
            continue
        name = re.sub(r"[<(:{].*$", "", line[m.end() :].strip()).strip()
        starts.append((name or line.strip(), i))
    spans = []
    for k, (name, i) in enumerate(starts):
        head = i
        while head > 0 and LEAD.match(lines[head - 1]):
            head -= 1
        end = starts[k + 1][1] - 1 if k + 1 < len(starts) else len(lines) - 1
        # хвостовые пустые строки и «висячие» комментарии оставляем соседу
        while end > i and (lines[end].strip() == "" or LEAD.match(lines[end])):
            end -= 1
        spans.append((name, head, end))
    return spans


def collect_uses(lines: list[str]) -> str:
    out: list[str] = []
    depth = 0
    inside = False
    for line in lines:
        if not inside and re.match(r"^(pub(\([a-z]+\))? )?use ", line):
            inside = True
            depth = 0
        if inside:
            out.append(re.sub(r"^pub(\([a-z]+\))? use ", "use ", line))
            depth += line.count("{") - line.count("}")
            if depth <= 0 and line.rstrip().endswith(";"):
                inside = False
    return "".join(out)


def main() -> int:
    if len(sys.argv) < 5:
        print(__doc__)
        return 2
    src, dst, doc, names = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
    lines = io.open(src, encoding="utf-8").read().splitlines(keepends=True)

    # Имя может встречаться дважды (`struct X` и `impl X`) — берём ВСЕ куски,
    # иначе половина элемента останется на месте (поймано на `TreeNodeDto`).
    spans: dict[str, list[tuple[int, int]]] = {}
    for n, a, b in item_spans(lines, 0):
        spans.setdefault(n, []).append((a, b))
    impl_header = None
    for name, a, _ in item_spans(lines, 0):
        if lines[a].lstrip().startswith("impl ") or " impl " in lines[a]:
            for n, ia, ib in item_spans(lines, 4):
                spans.setdefault(n, [(ia, ib)])
            impl_header = lines[a].rstrip("\n")
            break

    missing = [n for n in names if n not in spans]
    if missing:
        print("не нашёл элементы:", ", ".join(missing))
        print("есть:", ", ".join(sorted(spans)[:40]))
        return 1

    picked = sorted(span for n in names for span in spans[n])
    методы = any(lines[a].startswith("    ") or lines[a].startswith("    #") for a, _ in picked)
    body = "\n".join("".join(lines[a : b + 1]).rstrip("\n") for a, b in picked) + "\n"
    if методы:
        body = f"{impl_header}\n{body}}}\n"

    header = (
        f"//! {doc}\n//!\n//! Перенесено без изменения поведения "
        "(`plan/100-refactor-250.md`).\n\n" + collect_uses(lines) + "\n"
    )
    os.makedirs(os.path.dirname(dst) or ".", exist_ok=True)
    io.open(dst, "w", encoding="utf-8").write(header + body)

    drop = set()
    for a, b in picked:
        drop.update(range(a, b + 1))
    rest = "".join(l for i, l in enumerate(lines) if i not in drop)
    io.open(src, "w", encoding="utf-8").write(rest)
    print(f"перенесено {len(picked)} элементов в {dst}; в исходнике {rest.count(chr(10))} строк")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
