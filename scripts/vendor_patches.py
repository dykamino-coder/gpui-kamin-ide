# -*- coding: utf-8 -*-

"""Реестр правок vendored gpui / gpui-component — чтобы они не потерялись
при обновлении версии вендора.

Каждая наша правка помечена в коде комментарием `KaminIDE patch`. Скрипт:

    python scripts/vendor_patches.py           # проверить, что все на месте
    python scripts/vendor_patches.py --export  # выгрузить дифф в vendor/patches/

Порядок при апгрейде вендора:
 1. `--export` на СТАРОМ дереве (сохранит текущие диффы);
 2. заменить `vendor/<crate>` новой версией;
 3. `git apply --3way vendor/patches/*.patch` (или разложить руками);
 4. `python scripts/vendor_patches.py` — счётчики должны сойтись;
 5. `cargo build` + прогон probe-замеров парити.

Счётчики намеренно точные: если правку случайно снесли (или продублировали),
проверка падает и говорит, где именно.
"""

from __future__ import annotations
import sys
sys.stdout.reconfigure(encoding='utf-8', errors='replace')  # cp1251-консоль давилась на юникоде в выводе

import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MARKER = "KaminIDE patch"

# файл → сколько маркеров должно быть + зачем правка
EXPECTED: dict[str, tuple[int, str]] = {
    "vendor/gpui/src/elements/div.rs": (
        5,
        "hover/group_hover применяются и на LAYOUT (иначе цвет текста и "
        "кодиконов не меняется: он запекается до paint); текстовый "
        "под-рефайнмент сливается поле-в-поле, а не заменяется целиком "
        "(иначе hover с одним text_color стирал font_size/font_family)",
    ),
    "vendor/gpui/src/platform.rs": (
        6,
        "чужие текстуры (кадры CEF) в атласе; SVG из байтов: RGBA→BGRA + "
        "SMOOTH_SVG_SCALE_FACTOR (underlay/dcomp-методы удалены в 1.0.5 — "
        "мёртвая WebView2-эпоха)",
    ),
    "vendor/gpui/src/window.rs": (
        17,
        "отрисовка чужих текстур (`paint_external_texture`); `paint_image_region` "
        "— вырезка региона software-кадра CEF без растяжения (RDP); `paint_external_texture_px` — спрайт по физическим px (нет ресемпла на дробном DPI); счётчики "
        "стоимости кадра (`frame_perf`); `focus_visible` — клавиатурная "
        "модальность фокуса (`:focus-visible`)",
    ),
    "vendor/gpui/src/platform/windows/directx_renderer.rs": (
        23,
        "чужие текстуры в атласе; present без dcomp-Commit (пер-кадровый "
        "WaitForCommitCompletion = 30-90мс/кадр под RDP — удалён в 1.0.5 "
        "вместе с underlay-цепочкой)",
    ),
    "vendor/gpui/src/gpui.rs": (
        3,
        "модуль `frame_perf`: счётчики стоимости кадра + счётчики шейпинга и "
        "реэкспорт prepaint_prof (#76)",
    ),
    "vendor/gpui/src/scene.rs": (
        6,
        "ExternalTexture: кадры CEF в атласе — cropped/with_size/size/region "
        "(region — вырезка content_rect со смещением, план 101 Ф7)",
    ),
    "vendor/gpui/src/taffy.rs": (
        2,
        "счётчики раскладки (узлы, проходы, замеры, время) — план 101, Ф6",
    ),
    "vendor/gpui/src/text_system.rs": (
        3,
        "letter-spacing: `shape_line_spaced` / `layout_line_spaced` — обёртки с трекингом, старые вызовы не тронуты (план 99)",
    ),
    "vendor/gpui/src/text_system/line_layout.rs": (
        7,
        "letter-spacing: поле в ключах кэша строк + сдвиг глифов рядом с проходом "
        "force_width; счётчики промахов/попаданий кэша шейпинга (#76)",
    ),
    "vendor/gpui/src/platform/windows/platform.rs": (1, "WM_CHAR для child-HWND сторонних вью"),
    "vendor/gpui/src/platform/windows/events.rs": (
        3,
        "синхронный кадр внутри WM_SIZE: без него разворот/восстановление ~200 мс показывали растянутый старый кадр (план 101, Ф12)",
    ),
    "vendor/gpui/src/platform/windows/window.rs": (
        3,
        "env перечитывается при каждом создании окна; resize_boost_until — принудительные кадры после WM_SIZE (Ф12)",
    ),
    "vendor/gpui/src/elements/image_cache.rs": (
        1,
        "notify сразу по завершении загрузки картинки: on_next_frame ждал кадра окна, при точечной перерисовке новые иконки не показывались никогда",
    ),
    "vendor/gpui-component/src/input/state.rs": (7, "Input: поведение под наши сценарии"),
    "vendor/gpui-component/src/input/input.rs": (4, "Input: рендер под наши метрики"),
    "vendor/gpui-component/src/input/element.rs": (1, "Input: отрисовка выделения"),
    "vendor/gpui-component/src/input/search.rs": (
        2,
        "панель поиска редактора: `items_center` в колонке не давал строке "
        "растянуться, а `flex_1` без минимума схлопывал поле ввода до суффикса "
        "«Aa» — искать было негде, кнопки уезжали за бар",
    ),
    "vendor/gpui/src/element.rs": (
        2,
        "prepaint_prof: self-time профиль prepaint по типам элементов (KAMIN_PREPAINT_PROF=1, #76)",
    ),
    "vendor/gpui-component/src/input/element.rs": (
        8,
        "спаны prepaint_prof + append вместо O(n^2) combine_highlights + "
        "инкрементальный run-курсор в layout_lines (O(строки×runs) → O(runs)) + "
        "кэш склейки styles видимого диапазона (#76: 19->60fps)",
    ),
    "vendor/gpui-component/src/highlighter/highlighter.rs": (
        7,
        "межкадровый кэш styles() + cached_styles/store_styles для склеек "
        "видимого диапазона + потолок STYLES_CACHE_CAP (#76)",
    ),
    "vendor/gpui-component/src/scroll/scrollable.rs": (4, "overflow_y_scrollbar_with(external handle) + scrollbar_inset_right"),
    "vendor/gpui-component/src/scroll/scrollbar.rs": (
        1,
        "метрики ползунка под `::-webkit-scrollbar` оригинала: дорожка 8, "
        "thumb на всю ширину, радиус 4, без инсетов",
    ),
    "vendor/gpui-component/src/webview.rs": (3, "WebView2: наши правки хостинга"),
    "vendor/gpui-component/src/root.rs": (1, "Root: наш слой оверлеев"),
}


def count(path: pathlib.Path) -> int:
    if not path.exists():
        return -1
    return path.read_text(encoding="utf-8", errors="replace").count(MARKER)


def check() -> int:
    bad = 0
    for rel, (want, why) in sorted(EXPECTED.items()):
        got = count(ROOT / rel)
        if got == want:
            continue
        bad += 1
        state = "ФАЙЛА НЕТ" if got < 0 else f"маркеров {got}, ожидалось {want}"
        print(f"[!] {rel}: {state}\n    зачем: {why}")
    total = sum(w for w, _ in EXPECTED.values())
    if bad:
        print(f"\nНЕ СОШЛОСЬ: {bad} файл(ов). Всего маркеров должно быть {total}.")
        return 1
    print(f"OK: {len(EXPECTED)} файлов, {total} маркеров правок на месте.")
    return 0


def export() -> int:
    out = ROOT / "vendor" / "patches"
    out.mkdir(parents=True, exist_ok=True)
    for rel in sorted(EXPECTED):
        name = rel.replace("/", "__") + ".patch"
        diff = subprocess.run(
            ["git", "diff", "--no-color", "--", rel],
            cwd=ROOT,
            capture_output=True,
            text=True,
            encoding="utf-8",
        ).stdout
        (out / name).write_text(diff, encoding="utf-8")
        print(f"{'пусто' if not diff.strip() else 'дифф'} → vendor/patches/{name}")
    print("\nВНИМАНИЕ: пустой дифф значит, что правка уже закоммичена в vendor/ —")
    print("тогда при апгрейде переносить её надо ВРУЧНУЮ по маркерам.")
    return 0


if __name__ == "__main__":
    sys.exit(export() if "--export" in sys.argv else check())
