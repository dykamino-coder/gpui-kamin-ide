# 75 webview-view-anchor — наша реализация
Файлы: `crates/shell/src/root.rs:3422-3490` (webview_body, статические вью), `root.rs:3492-3560` (webview_body_dyn), `root.rs:3281-3420` (visual_wv_body: canvas-prepaint = синк зоны + клип `RADIUS_LG`), `crates/shell/src/ui/glint.rs:194` (вырез фона тем же радиусом), `crates/shell/src/probe_registry.rs` (probe_area — реестр bounds по id)

## Структура (gpui-дерево кратко)
Роль «якоря» играют два механизма:
1. wry-режим: `div#id.relative.size_full` → `probe_area(id)` (записывает bounds кадра в реестр) → `wv.clone()` (wry-чайлд сам позиционируется по bounds элемента).
2. visual hosting (`KAMIN_VISUAL_WV=1`): `visual_wv_body(id)` — `div` px 8 / pb 8 → probe_area + `gpui::canvas` prepaint, который тем же кадром зовёт `wv_visual::sync_zone_view` (позиция+размер dcomp-визуала) и `set_zone_view` («дыра» в фоне карты).

Динамический вариант (webview_body_dyn, contributed-тулы): вокруг вебвью px 8 / pb 8 (top 0), под ним подложка `rounded(RADIUS_MD)` bg editor_bg `#1d1c25` (закрывает разрыв ресайза).

Состояния до готовности:
- нет HTML (`!has_html`) → `panel_placeholder(label, "Open new tool or drag-n-drop tool from other panels", slot)` (см. 68);
- HTML есть, скрипт не жив (`!alive`) → load-cover: codicon `\u{eb19}` 22px accent_primary + «Loading…» FS_MD text_secondary.

## Метрики (из кода, точные)
- Инсет вокруг вебвью: left/right 8, bottom 8, top 0 (= margin `0 var(--space-2) var(--space-2)` оригинала).
- Радиус подложки дин-вью: RADIUS_MD = 12; статические вью в visual-режиме — клип углов делает dcomp-clip / `overlay::round_webview_children` (root.rs:3415-3440), радиус по зонам.
- Подложка: p.editor_bg `#1d1c25` (диаг-режим KAMIN_VWV_PAINTDBG=1 — оранжевый).
- Ретрай resolve: не чаще 1 раза в 5с (root.rs:3024-3033), без лимита попыток.

## Отличия от original.md той же папки
1. Радиус: оригинал `.frame` — `radius-lg` 16; у нас дин-подложка 12 (RADIUS_MD); статические вью клипуются по зоне отдельным механизмом.
2. Вместо WebviewLoadingSkeleton (шиммер, attempts) и WebviewLoadError (Retry) — единый load-cover «Loading…» без счётчика попыток и без ручного Retry.
3. Ретрай-политика: оригинал 45 попыток, backoff 350ms×1.5 до 3000, exhausted-состояние; у нас фикс-интервал 5с без исчерпания.
4. `.frameFlush` (Customize) — эквивалент есть: czShared-вебвью рендерится без инсетов внутри glint-карты (root.rs:5304-5333).
5. Якорь-механика иная по сути: не DOM rect + слой поверх, а probe-реестр bounds + канвас-prepaint (visual) либо позиционирование wry-чайлда.

## Дополнение атрибутов (цикл 10)

- ховер: N/A: ховер — зона-якорь (`visual_wv_body`, `root.rs:3099-3140`) hover-подсветки не имеет: мышь форвардится в composition-вебвью (`send_mouse_view`), меняется только курсор (`wv_cursor`, `root.rs:3105`); у `.frame` оригинала hover-правил тоже нет.
