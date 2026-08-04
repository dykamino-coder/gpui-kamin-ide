# -*- coding: utf-8 -*-
"""Раннер цикла сверки: из остатка гейта готовит задание на следующий цикл.

    python parity/cycle.py            # NEXT_CYCLE.md для зоны с худшим счётом
    python parity/cycle.py --all      # задания по ВСЕМ зонам с расхождениями
    python parity/cycle.py --zone 92  # конкретная зона по номеру любого элемента

Что делает:
  1. читает последний вердикт каждого элемента (тот же разбор, что в gate.py);
  2. собирает по зоне список DIVERGES с ТЕКСТОМ последней претензии — чтобы
     следующий ревьюер отчитывался по каждому пункту «закрыто / осталось»,
     а не пересматривал зону заново;
  3. пишет `parity/NEXT_CYCLE.md` — готовый промпт: номер цикла, файлы порта
     и оригинала, свежие кадры, ограничения gpui, формат ответа;
  4. печатает, каких элементов не хватает пары кадров — их вердикт гейт не
     считает подтверждённым.

Спавн ревьюверов остаётся за оператором (агенты — инструмент ассистента, не
скрипта): раннер даёт детерминированное ЗАДАНИЕ, гейт — детерминированную
проверку. Порядок: fix → `gate.py` → `cycle.py` → ревью → вердикты → `gate.py`.
"""
import io
import os
import re
import sys

ZONES = [
    (1, 19, "Титлбар", "ui/titlebar.rs, ui/session_tabs.rs, ui/layout_popover.rs",
     "components/titlebar/*"),
    (20, 37, "Сайдбар", "ui/sessions_list.rs, ui/customize.rs",
     "components/sidebar/*"),
    (38, 51, "Activity, рейлы, стрипы",
     "ui/activity_bar.rs, ui/right_column.rs, ui/slot_panel.rs, ui/tool_picker.rs",
     "components/activity-bar/*"),
    (52, 91, "Панели и экраны",
     "ui/customize.rs, ui/design_panel.rs, ui/logs_panel.rs, ui/extensions_panel.rs, "
     "ui/problems.rs, ui/panel_placeholder.rs, ui/term_toolbar.rs",
     "components/{main,settings,right-panel,main-bottom-panel,problems,terminal}/*"),
    (92, 107, "Дерево файлов", "ui/file_list.rs, ui/file_menu.rs, icon_theme.rs",
     "components/file-tree/*"),
    (108, 129, "Редактор, оверлеи, статус",
     "ui/quick_open.rs, ui/command_palette.rs, ui/find_in_files.rs, ui/workspace_symbols.rs, "
     "ui/modal.rs, ui/quick_pick.rs, ui/toasts.rs, ui/tooltip.rs, ui/status_bar.rs, overlay.rs",
     "components/{overlays,status-bar,webview}/*"),
    (130, 159, "Токены, семплы, глобальные стили",
     "ui/design_panel.rs, ui/shadows.rs, ui/glint.rs, crates/theme/src/lib.rs, "
     "crates/metrics/src/lib.rs",
     "components/main/{DesignPanel,design-sections,component-samples*}, theme/*.css"),
]

HEAD = re.compile(r"^## Цикл (\d+): ([A-ZА-Я/ ()]+)$", re.M)
LIMITS = (
    "Ограничения gpui (НЕ дефект, но упоминать, если влияет): нет "
    "`letter-spacing`, нет `transform`, `svg().text_color()` не реагирует на "
    "hover, ровно "
    "ОДИН `.hover()` на элемент, у `Stateful` нет `self_center()`, `Input` "
    "берёт кегль и паддинги из своего `Size`, а не из обёртки.\n\n"
    "Про эллипсис: `text_ellipsis()` ВЫСТАВЛЯЕТ `TextOverflow::Truncate(\"…\")` "
    "(`vendor/gpui/src/styled.rs:83-88`), но на экране для `.child(SharedString)` "
    "многоточие не появлялось — текст резался родительским `overflow_hidden` "
    "(скрин-сверка, см. шапку `ui/text_fit.rs`). Поэтому «нет …» — валидная "
    "претензия ТОЛЬКО если подтверждена кадром; править её нужно через "
    "`ui::text_fit::fit`, а не добавлением `text_ellipsis()`."
)


def zone_of(num: int):
    for z in ZONES:
        if z[0] <= num <= z[1]:
            return z
    return None


def parse(root: str):
    rows = []
    for d in sorted(
        (d for d in os.listdir(root) if os.path.isdir(os.path.join(root, d)) and d[0].isdigit()),
        key=lambda d: int(d.split("-")[0]),
    ):
        path = os.path.join(root, d, "verdict.md")
        txt = io.open(path, encoding="utf-8").read() if os.path.exists(path) else ""
        heads = list(HEAD.finditer(txt))
        if heads:
            last = heads[-1]
            verdict = last.group(2).strip()
            cycle = int(last.group(1))
            body = txt[last.end():].strip()
        else:
            verdict, cycle, body = "—", 0, ""
        shots = all(
            os.path.exists(os.path.join(root, d, f)) for f in ("original.png", "ours.png")
        )
        rows.append(
            {
                "num": int(d.split("-")[0]),
                "dir": d,
                "verdict": verdict,
                "cycle": cycle,
                "body": body,
                "shots": shots,
            }
        )
    return rows


def task_for(zone, rows, next_cycle: int) -> str:
    lo, hi, title, ours, orig = zone
    bad = [r for r in rows if lo <= r["num"] <= hi and r["verdict"].startswith("DIVERGES")]
    out = [f"## Зона {lo}-{hi} — {title}: цикл {next_cycle}\n"]
    out.append(
        f"Порт: `crates/shell/src/{{{ours}}}`. "
        f"Оригинал: `%PROJECTS%\\kamin-ide\\src\\renderer\\{orig}` + `theme/*.css`.\n"
    )
    out.append(
        "По КАЖДОМУ пункту ниже ответь «закрыто» или «осталось» с числом/строкой "
        "кода; новое — отдельным списком. Мерить пиксели скриптом (PIL/numpy) в "
        "СВОЮ временную папку, в `parity/` не писать. DPR обеих сторон 1.25; в "
        "наших кадрах бывает рамка захвата ~9 физ. px слева и сверху — "
        "калибруйся по ней перед выводами.\n"
    )
    out.append(LIMITS + "\n")
    out.append(f"### Претензии к закрытию ({len(bad)})\n")
    for r in bad:
        first = " ".join(r["body"].split())[:400]
        shots = "" if r["shots"] else "  ⚠ нет пары кадров — вердикт не подтверждён"
        out.append(f"- **{r['num']} {r['dir'][len(str(r['num'])) + 1:]}** (ц.{r['cycle']}){shots}\n  {first}")
    out.append(
        "\nФормат ответа: `NN <имя>: MATCH|DIVERGES` + при DIVERGES 1-3 строки "
        "(оригинал файл:строка/значение → у нас файл:строка/значение → как чинить). "
        "В конце — сводка и список закрытых пунктов.\n"
    )
    return "\n".join(out)


def main() -> int:
    root = os.path.dirname(os.path.abspath(__file__))
    rows = parse(root)
    zones_bad = []
    for z in ZONES:
        bad = [r for r in rows if z[0] <= r["num"] <= z[1] and r["verdict"].startswith("DIVERGES")]
        if bad:
            cyc = max((r["cycle"] for r in rows if z[0] <= r["num"] <= z[1]), default=0)
            zones_bad.append((len(bad), z, cyc))
    if not zones_bad:
        print("Расхождений нет — задание не требуется, проверь `gate.py`.")
        return 0
    zones_bad.sort(key=lambda t: (-t[0], t[1][0]))

    pick = zones_bad
    if "--zone" in sys.argv:
        num = int(sys.argv[sys.argv.index("--zone") + 1])
        z = zone_of(num)
        pick = [t for t in zones_bad if t[1] is z]
    elif "--all" not in sys.argv:
        pick = zones_bad[:1]

    parts = ["# Задание на следующий цикл сверки\n",
             "Сгенерировано `parity/cycle.py` из остатка гейта. "
             "После правок: `python parity/gate.py --list`.\n"]
    for count, z, cyc in pick:
        parts.append(task_for(z, rows, cyc + 1))
    io.open(os.path.join(root, "NEXT_CYCLE.md"), "w", encoding="utf-8", newline="\n").write(
        "\n".join(parts) + "\n"
    )

    for count, z, cyc in zones_bad:
        mark = "→" if any(z is p[1] for p in pick) else " "
        print(f" {mark} {z[0]:>3}-{z[1]:<3} {z[2]:<34} расхождений: {count:<3} последний цикл: {cyc}")
    missing = [r["num"] for r in rows if not r["shots"]]
    if missing:
        print(f"\nбез пары кадров ({len(missing)}): {missing}")
    print("\nзадание: parity/NEXT_CYCLE.md")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
