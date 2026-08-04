# 144 sample-tooltip — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_tooltip`), ui/tooltip.rs

## Структура/содержание
Одна кнопка `ds_btn(Ghost)` «Hover me» с `.tooltip(...)` и тем же текстом, что в `data-tooltip` оригинала. Рисует общий механизм порта (gpui-ховер + overlay-копия) — аналог document-level listener'а.

## Метрики (из кода, точные)
`.btnGhost`: px 16 / py 4, radius 8, fs 12, фон прозрачный, цвет text-secondary, рамка 1px transparent (резерв ширины), hover bg-surface + text-primary.

## Отличия от original.md той же папки
Нет CSS-перехода фона 150ms (deviation порта).

## Дополнение атрибутов (цикл 10)

- цвета: кнопка Ghost — фон прозрачный, text p.text_secondary #adb3c7, border 1px rgba(0,0,0,0) (design_samples.rs:110-122); hover — bg p.bg_surface #3d3f51 + text p.text_primary #cfd4e2 (design_samples.rs:119-120); сам бокс тултипа — bg p.bg_surface #3d3f51, text p.text_primary #cfd4e2, shadow rgba(0,0,0,.3) (tooltip.rs:29-31,67-68)
- шрифты: кнопка font-size 12 (FS_SM), font-weight 400 (design_samples.rs:129); бокс тултипа font-size 11 (FS_XS), line-height 14.3 (tooltip.rs:72-73)
