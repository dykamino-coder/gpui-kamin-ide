# 155 glint-surface-card-ring — наша реализация
Файлы: crates/shell/src/ui/glint.rs:28-59 (`glint_surface`), 64-233 (`hole_segments*`, `glint_surface_wv_holed`); токены — crates/theme/src/palette.rs:48-49, 86-87, 124-125 (glint_edge/glint_mid)

## Структура/содержание
`glint_surface(p, content)`: 4 слоя div — (1) сплошной mid (glint_mid), (2) linear_gradient 135° edge(α)→edge(0) на 0%→22%, (3) linear_gradient 135° edge(0)→edge(α) на 78%→100%, (4) внутренний rect p(1px) с заливкой bg_mantle radius 15. gpui 0.2.2 даёт максимум 2 стопа на градиент — 4-стоповый CSS-глинт собран из двух наложенных 2-стоповых слоёв (за пределами стопов градиент клампится → между 22% и 78% чистый mid). Пиксельно эквивалентно оригиналу.
Вариант `glint_surface_wv_holed`: те же 4 слоя paint_quad-ами через content-mask сегментов вокруг «дыр» composition-вебвью + антиалиасные угловые маски radius 12 (полилиния 12 сегментов) — зона остаётся прозрачной для underlay-вебвью.
Используется карточками MainContent / FilePanel / RightPanel / MainBottomPanel (те же потребители, что в оригинале).

## Метрики (из кода, точные)
- Кромка 1px (внутренний rect inset p(1)); внешний радиус RADIUS_LG 16, внутренний 15 (concentric).
- Стопы: 0% edge α.18 → 22% α0; 78% α0 → 100% α.18; угол 135°.
- dark: edge rgba(255,255,255,.18), mid #262533 (bg_mantle); light: edge rgba(60,40,20,.18), mid #e6e1d4 (bg_surface). Закрыто тестом glint_mid_matches_panel_fill.

## Отличия от original.md той же папки
- Значения совпадают полностью: угол, стопы 0/22/78/100, α .18, fill bg-mantle, кромка 1px.
- Расхождение в light glint-mid: оригинальный токен --glint-border формально всегда ставит mid = var(--bg-mantle) (light bg-mantle #fbf7f4), у нас light mid = bg_surface #e6e1d4. Требует сверки с light-theme.css оригинала (наш комментарий утверждает «light glint mid = bg_surface» — вероятно оригинальная light-тема переопределяет токен; original.md фиксирует только dark).
- Механика: два 2-стоповых слоя вместо одного 4-стопового + inner-rect вместо padding-box/border-box трюка — визуально идентично, но проверяется скриншотом, не кодом.
- Бонус против оригинала: вариант с «дырой» под нативный вебвью (в DOM-оригинале не нужен).
