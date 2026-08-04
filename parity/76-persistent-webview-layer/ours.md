# 76 persistent-webview-layer — наша реализация

НЕ РЕАЛИЗОВАНО как DOM-слой `position:fixed` с iframe'ами. Замена: composition visual hosting + персистентные нативные WebView2-чайлды.

Файлы: `crates/shell/src/wv_visual.rs` (весь файл — CoreWebView2CompositionController в dcomp-underlay визуал), `crates/shell/src/root.rs:3470-3560` (все вебвью создаются на ПЕРВОМ кадре и живут весь ран; дисковый кэш HTML), `root.rs:3386-3404` (clear_zones/hide на кадр), `root.rs:3702-3742` (show/hide wv2-чайлдов по видимости тула), `root.rs:3415-3440` (`overlay::round_webview_children` — скругление углов чайлдов), `crates/shell/src/ui/glint.rs:61-115` (hole_segments — «дыры» в фоне карт под зоны)

## Структура (gpui-дерево кратко)
- Персистентность: `RootView.webviews: HashMap<viewId, Entity<WebView>>` — wry/wv2-чайлды создаются один раз (первый кадр; create_controller пампит event loop → вне первого кадра RefCell-паника) и не уничтожаются; HTML едет `load_url` в любом кадре.
- Позиционирование: не rAF-loop по DOM-якорю, а prepaint-канвас каждого кадра (`sync_zone_view`) — позиция/размер dcomp-визуала обновляются тем же кадром, что и layout (синхронно со сплиттером, без дребезга).
- Видимость: `wv.show()/hide()` по правилу «тул активен в каком-либо слоте ∧ панели видимы ∧ не Customize ∧ alive» (root.rs:3716-3742); скрытый чайлд остаётся смонтирован (буфер/стейт живут) — аналог `display:none`.
- «Шторка» чата: отсутствует (см. 72); z-порядок решается нативным HWND/dcomp, не zIndex 5.
- czShared: ОДИН переиспользуемый вебвью на все contributed Customize-страницы (root.rs:3475-3476).
- Backdrop: одноцветный dcomp-визуал editor_bg ПОД вебвью прикрывает щели, пока Chromium догоняет relayout (wv_visual.rs Host.backdrop).

## Метрики (из кода, точные)
- Скругление зон: dcomp `IDCompositionRectangleClip` (антиалиас) в visual-режиме; в wry-режиме — оконный регион `round_webview_children(zones, scale)`.
- Гистерезис ресайза поверхности: SetBounds только когда размер замер ≥120мс; доводчик 160мс (wv_visual.rs Host.want_since/settle_pending).
- Загрузочный HTML-кэш: `cache/webview-html/{id}.html` — UI рисуется сразу, extension активируется ~8с фоном.

## Отличия от original.md той же папки
1. Нет DOM-слоя (OVERLAY_Z=5), нет копирования rect/borderRadius якоря через getBoundingClientRect/getComputedStyle — синк идёт из layout-движка gpui тем же кадром (жёстче, чем rAF-burst 12 кадров + интервал 500мс оригинала).
2. Нет chatSwitchCovered/шторки с transition 140ms (см. 72).
3. Нет per-frame loop на `body.kamin-dragging`/resize-settle 200мс — не нужен: prepaint синхронен.
4. Contributed Customize-страницы делят один вебвью (czShared) вместо N персистентных iframe.
5. Дополнительно к оригиналу: «дыры» в фонах карт (hole_segments) и backdrop-визуал — артефакты composition-хостинга, у DOM-оригинала не требовались.

## Дополнение атрибутов (цикл 10)

- цвета: слой как таковой прозрачен — composition-вебвью живут в dcomp-underlay ПОД кадром gpui, а в фонах карт под их зоны вырезаются «дыры» (`glint.rs:64,78` — `hole_segments_multi`/`hole_segments`, `glint.rs:122` — `glint_surface_wv_holed`); подложка ровно под вебвью (закрывает разрыв догоняющего ресайза) — editor_bg #1d1c25 dark / #fcfaf6 light (`root.rs:3332`, `palette.rs:59,97`); вокруг дыр канвас заливает bg_sidebar #1d1d28 / #f4f1ea (`root.rs:6060`, `palette.rs:56,94`).
- отступы: инсет вокруг вебвью 8px по бокам и снизу, сверху 0 (`root.rs:3311-3312`; тот же инсет в `visual_wv_body` — `root.rs:3103-3104`); у Customize-обёртки вместо этого pt/pb 8 (`root.rs:6339-6340`); скругление углов чайлдов задаётся не CSS-радиусом, а регионом по зонам (`root.rs:4099` → `overlay.rs:1460`). Оригинал padding/margin не имеет вовсе — геометрия копируется из rect якоря, а инсет принадлежит `.frame`.
