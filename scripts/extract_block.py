# -*- coding: utf-8 -*-
"""Вынести блок `let ИМЯ: AnyElement = { … };` из `render` в метод.

Вторая половина `render` — это цепочка колонок: каждая собирается в своём
блоке и потом кладётся в дерево. Блок уезжает в метод, возвращающий тот же
`AnyElement`, а на его месте остаётся вызов — порядок сборки не меняется.

    python scripts/extract_block.py <src.rs> <A> <B> <dst.rs> <имя> "<док>" [параметр…]

A — строка с `let ИМЯ`, B — строка с закрывающей `};`. Параметры пишутся
как `имя: тип` и подставляются и в сигнатуру, и в вызов.
"""
from __future__ import annotations

import io
import os
import re
import sys

sys.path.insert(0, os.path.dirname(__file__))
from extract_items import collect_uses  # noqa: E402


def main() -> int:
    if len(sys.argv) < 7:
        print(__doc__)
        return 2
    src, a, b, dst, name, doc = (
        sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4], sys.argv[5], sys.argv[6]
    )
    extra = sys.argv[7:]
    lines = io.open(src, encoding="utf-8").read().splitlines(keepends=True)

    first = lines[a - 1]
    m = re.match(r"^\s*let (?:mut )?(\w+)(: [^=]+)? = ", first)
    if not m:
        print(f"строка {a} — не начало блока `let`: {first.strip()[:60]}")
        return 1
    var = m.group(1)
    tail = lines[b - 1].rstrip()
    if not tail.endswith(";"):
        print(f"строка {b} — не конец блока (нет `;`): {tail[-40:]}")
        return 1

    # тело = всё между `= ` и завершающей `;`
    body = first[m.end():] + "".join(lines[a : b - 1]) + lines[b - 1].rstrip().rstrip(";") + "\n"
    body = "".join("    " + l if l.strip() else l for l in body.splitlines(keepends=True))
    params = "".join(", " + p for p in extra)
    args = "".join(", " + p.split(":")[0].strip() for p in extra)
    sig = (
        f"    pub(crate) fn {name}(\n        &mut self,\n        window: &mut Window,\n"
        f"        cx: &mut Context<Self>{params},\n    ) -> AnyElement {{\n"
    )
    method = sig + body + "    }\n"

    if os.path.exists(dst):
        old = io.open(dst, encoding="utf-8").read().rstrip("\n")
        assert old.endswith("}"), dst
        text = old[:-1].rstrip("\n") + "\n\n" + method + "}\n"
    else:
        header = (
            f"//! {doc}\n//!\n//! Блок `render` вынесен как есть "
            "(`plan/100-refactor-250.md`).\n\n" + collect_uses(lines) + "\n"
        )
        text = header + "impl RootView {\n" + method + "}\n"
    os.makedirs(os.path.dirname(dst) or ".", exist_ok=True)
    io.open(dst, "w", encoding="utf-8").write(text)

    call = f"        let {var}: AnyElement = self.{name}(window, cx{args});\n"
    rest = "".join(lines[: a - 1] + [call] + lines[b:])
    io.open(src, "w", encoding="utf-8").write(rest)
    print(f"{name}: {b - a + 1} строк → {dst}; в {src} осталось {rest.count(chr(10))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
