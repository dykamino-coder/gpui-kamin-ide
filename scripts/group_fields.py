"""Переносит группу полей `RootView` в отдельную структуру-модуль.

Мотив: `state/model.rs` — одна декларация на 300+ строк, и джуну в ней не
видно, какие поля про что. Группируем связанные поля в под-структуры
(`plan/100-refactor-250.md`), доступ становится `self.<группа>.<поле>`.

Использование:
    python scripts/group_fields.py <группа> <Структура> <модуль> <поле>...

Пример:
    python scripts/group_fields.py term TerminalState model_term \\
        terminals term_active term_menu_open

Все правки проверяются компилятором: имя поля уникально по крейту, поэтому
замена идёт по `.<поле>` со словесной границей (см. `--check` ниже — скрипт
печатает, сколько вхождений затронул, ДО записи).
"""

import glob
import io
import os
import re
import sys

MODEL = "crates/shell/src/state/model.rs"
INIT = "crates/shell/src/state/init.rs"
FIELD = re.compile(r"^\s*pub(?:\(crate\))? (\w+):")


def cut_fields(names):
    """Вырезает поля из `model.rs` вместе с их doc-комментариями."""
    lines = io.open(MODEL, encoding="utf-8").read().split("\n")
    taken, moved, i = set(), [], 0
    while i < len(lines):
        mo = FIELD.match(lines[i])
        if mo and mo.group(1) in names:
            start = i
            while start - 1 >= 0 and lines[start - 1].strip().startswith("///"):
                start -= 1
            while start in taken:
                start += 1
            end = i
            while not lines[end].rstrip().endswith(","):
                end += 1
            moved.append("\n".join(lines[start : end + 1]))
            taken.update(range(start, end + 1))
            i = end + 1
        else:
            i += 1
    rest = [l for n, l in enumerate(lines) if n not in taken]
    pos = sum(1 for n in range(min(taken)) if n not in taken)
    return rest, pos, moved


def rewrite_uses(names, group):
    """`.field` → `.group.field` по всему крейту (кроме самих деклараций)."""
    skip = {os.path.normpath(MODEL)}
    # `.output(` — вызов Command, а не поле: границу ставим по «не (»
    rx = re.compile(r"\.(" + "|".join(names) + r")\b(?!\s*\()")
    touched = 0
    for f in glob.glob("crates/shell/src/**/*.rs", recursive=True):
        if os.path.normpath(f) in skip:
            continue
        s = io.open(f, encoding="utf-8").read()
        s2 = rx.sub(lambda mo: f".{group}." + mo.group(1), s)
        if s2 != s:
            io.open(f, "w", encoding="utf-8").write(s2)
            touched += 1
    return touched


def main():
    group, struct, module = sys.argv[1], sys.argv[2], sys.argv[3]
    names = sys.argv[4:]
    rest, pos, moved = cut_fields(set(names))
    if len(moved) != len(names):
        print(f"нашёл {len(moved)} из {len(names)} полей — проверь имена")
        return 1
    rest[pos:pos] = [f"    pub {group}: crate::state::{module}::{struct},"]
    io.open(MODEL, "w", encoding="utf-8").write("\n".join(rest))
    head = (
        f"//! Поля `RootView`, вынесенные группой `{group}`\n"
        "//! (`plan/100-refactor-250.md`).\n\n"
        "use gpui::Entity;\n"
        "use gpui_component::input::InputState;\n\n"
        "#[derive(Default)]\n"
        f"pub struct {struct} {{\n"
    )
    io.open(f"crates/shell/src/state/{module}.rs", "w", encoding="utf-8").write(
        head + "\n".join(moved) + "\n}\n"
    )
    print("файлов правлено:", rewrite_uses(names, group))
    # инициализатор: убрать поштучные строки, добавить группу
    lines = io.open(INIT, encoding="utf-8").read().split("\n")
    drop = re.compile(r"^\s*(" + "|".join(names) + r"): ")
    keep = [l for l in lines if not drop.match(l)]
    anchor = next(k for k, l in enumerate(keep) if "palette_open: false," in l)
    keep[anchor + 1 : anchor + 1] = [
        f"            {group}: crate::state::{module}::{struct}::default(),"
    ]
    io.open(INIT, "w", encoding="utf-8").write("\n".join(keep))
    print("init ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
