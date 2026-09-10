"""Сверяет маршруты событий: кого `dispatch` отправляет — тот и обрабатывает.

Зачем: у каждого `apply_*` последняя ветка — `_ => {}`. Если диспетчер отправит
событие модулю, который его больше не разбирает, компилятор промолчит, а кнопка
в интерфейсе просто перестанет работать. Скрипт ловит это до запуска.

    python scripts/check_event_routing.py

Печатает три вида расхождений и возвращает код 1, если они есть:
  * «маршрут в никуда» — dispatch шлёт вариант модулю, где нет его ветки;
  * «обработчик-сирота» — модуль разбирает вариант, который ему не шлют;
  * «двойной дом» — вариант разбирают сразу в нескольких модулях.
"""

import io
import re
import sys

# Windows-runner печатает в cp1252 — русские сообщения иначе роняют скрипт.
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pathlib import Path

EVENTS = Path("crates/shell/src/state/events")
# `ShellEvent::Cz(CzEvent::Foo` / `ShellEvent::Bar` → ключ "Cz::Foo" / "Bar"
PAT = re.compile(
    r"ShellEvent::(?P<outer>[A-Z]\w+)(?:\((?P<inner>[A-Z]\w+)::(?P<var>[A-Z]\w+))?"
)


def key(mo):
    if mo.group("inner"):
        return f"{mo.group('outer')}::{mo.group('var')}"
    return mo.group("outer")


def dispatch_routes():
    """{вариант: модуль} по вызовам `self.apply_<модуль>(event, cx)`."""
    text = io.open(EVENTS / "dispatch.rs", encoding="utf-8").read()
    body = text[text.index("match &event {") :]
    routes, pending = {}, []
    for line in body.split("\n"):
        for mo in PAT.finditer(line):
            pending.append(key(mo))
        call = re.search(r"self\.apply_(\w+)\(event, cx\)", line)
        if call:
            for k in pending:
                routes[k] = call.group(1)
            pending = []
    return routes


def module_arms():
    """{вариант: {модули}} по веткам верхнего уровня в каждом `apply_*`."""
    arms = {}
    for f in sorted(EVENTS.glob("*.rs")):
        if f.name in ("mod.rs", "dispatch.rs"):
            continue
        text = io.open(f, encoding="utf-8").read()
        mo = re.search(r"pub\(crate\) fn apply_(\w+)\(", text)
        if not mo:
            continue
        module = mo.group(1)
        for line in text[mo.end() :].split("\n"):
            # ветки верхнего уровня идут с отступом 12 пробелов
            if not line.startswith(" " * 12) or line.startswith(" " * 13):
                continue
            for m2 in PAT.finditer(line):
                arms.setdefault(key(m2), set()).add(module)
    return arms


def main():
    routes, arms = dispatch_routes(), module_arms()
    bad = []
    for var, module in sorted(routes.items()):
        homes = arms.get(var, set())
        if module not in homes:
            bad.append(f"маршрут в никуда: {var} → apply_{module}, а ветки там нет")
    for var, homes in sorted(arms.items()):
        if var not in routes:
            bad.append(f"обработчик-сирота: {var} разбирают в {sorted(homes)}, но не шлют")
        elif len(homes) > 1:
            bad.append(f"двойной дом: {var} разбирают в {sorted(homes)}")
    print(f"вариантов в диспетчере: {len(routes)}, с обработчиком: {len(arms)}")
    for b in bad:
        print("  " + b)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
