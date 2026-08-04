# 141 sample-toast-triggers — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_toast_triggers`), design_panel.rs (блок «In-app toasts»)

## Структура/содержание
5 кнопок `ds_btn(Secondary)` в `.compInline`-ряду (flex-wrap, gap 8): Push info / Push success / Push warning / Push error / With actions. Каждая шлёт `ShellEvent::Toast` в стек (`ui/toasts.rs`) с текстами оригинала; «With actions» — actions ["Save","Discard"] + sticky.

## Метрики (из кода, точные)
`ds_btn(Secondary)`: px SPACE_4 16 / py SPACE_1 4, radius RADIUS_SM 8, fs FS_SM 12, фон прозрачный, рамка 1px `--bg-overlay`, hover bg `--bg-surface`.

## Отличия от original.md той же папки
Переходов (`transition 150ms ease`) в gpui нет — общий deviation порта.

## Дополнение атрибутов (цикл 10)

- шрифты: кнопка font-size 12 (FS_SM) (design_samples.rs:129), font-weight 400 (Secondary — ветка `bold = false`, design_samples.rs:100,148-150); семейство UI «Bricolage Grotesque»
