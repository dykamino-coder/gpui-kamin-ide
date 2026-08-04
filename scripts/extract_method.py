# -*- coding: utf-8 -*-
"""Вынести кусок тела `render` в метод — без изменения порядка выполнения.

`render` у `RootView` начинается длинным прологом: подготовка кадра, ленивое
создание инпутов, бутстрап вебвью. Это последовательность независимых
блоков, поэтому каждый уезжает в свой метод, а на его месте остаётся вызов.

    python scripts/extract_method.py <src.rs> <A> <B> <dst.rs> <имя> "<док>" [доп-параметр…]

A и B — 1-based, включительно. Доп-параметры пишутся как `имя: тип`
(например `visual_bridge: bool`) и подставляются и в сигнатуру, и в вызов.
"""
from __future__ import annotations

import io
import os
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

    body = "".join(lines[a - 1 : b])
    params = "".join(", " + p for p in extra)
    args = "".join(", " + p.split(":")[0].strip() for p in extra)
    sig = (
        f"    pub(crate) fn {name}(\n        &mut self,\n"
        f"        window: &mut Window,\n        cx: &mut Context<Self>{params},\n    ) {{\n"
    )
    header = (
        f"//! {doc}\n//!\n//! Кусок `render` вынесен как есть "
        "(`plan/100-refactor-250.md`): порядок вызовов в кадре прежний.\n\n"
        + collect_uses(lines)
        + "\nuse crate::root::RootView;\n\n"
    )
    if os.path.exists(dst):
        old = io.open(dst, encoding="utf-8").read().rstrip("\n")
        assert old.endswith("}"), dst
        src_text = old[:-1].rstrip("\n") + "\n\n" + sig + body + "    }\n}\n"
    else:
        src_text = header + "impl RootView {\n" + sig + body + "    }\n}\n"
    os.makedirs(os.path.dirname(dst) or ".", exist_ok=True)
    io.open(dst, "w", encoding="utf-8").write(src_text)

    call = f"        self.{name}(window, cx{args});\n"
    rest = "".join(lines[: a - 1] + [call] + lines[b:])
    io.open(src, "w", encoding="utf-8").write(rest)
    print(f"{name}: {b - a + 1} строк → {dst}; в {src} осталось {rest.count(chr(10))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
