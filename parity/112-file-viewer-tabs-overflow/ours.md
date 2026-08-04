# 112 file-viewer-tabs-overflow — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\editor_tabs.rs:229-343; закрытие по Escape — crates\shell\src\root.rs:5857-5858, закрытие кликом-мимо — root.rs:6020-6042; состояние — root.rs:224, 1988-1989

## Структура/содержание
```
div#ftabs-overflow (relative, flex_shrink_0, h 24): «{N}» + codicon chevron-down 12
└─ (overflow_open) deferred(menu, priority 60)
   div.occlude.absolute.top(28).right(0).min_w(200).max_w(360).max_h(400).overflow_hidden
   └─ div#ftov-{i} × скрытые табы:
      ├─ file_img 14×14 (Catppuccin-иконка)
      ├─ label: flex_1.min_w(0).overflow_hidden.text_ellipsis.whitespace_nowrap
      └─ (dirty) круг 6×6 rounded_full accent_orange
```
Клик по кнопке → `ToggleFileTabsOverflow`; клик по пункту → `SelectEditorTab(i)` + `ToggleFileTabsOverflow`. Escape (`CloseOverlay`, root.rs:5857) и mouse-down мимо меню (root.rs:6027, 6038) тоже закрывают.

## Метрики (из кода, точные)
- отступы: кнопка px 6 при h 24 (py нет); меню p 4 (SPACE_1); пункт px 8 (SPACE_2) / py 5
- гэпы: кнопка gap 2; меню gap 1 между пунктами; пункт gap 6
- цвета: кнопка p.text_secondary #adb3c7; меню bg p.bg_surface #3d3f51, border 1px p.text_primary #cfd4e2 α 0.06, shadow 0 6 24 rgba(0,0,0,0.30); пункт p.text_secondary #adb3c7; dirty-круг p.accent_orange #fab387
- скругления: кнопка 8 (RADIUS_SM); меню 12 (RADIUS_MD); пункт 8 (RADIUS_SM); dirty — rounded_full
- шрифты: кнопка font-size 12 (FS_SM), weight 400; chevron — codicon 12; пункт font-size 12 (FS_SM), weight 400; собственных шрифтовых правил у меню нет
- фоны по ховеру: кнопка — p.text_primary α 0.08 + текст p.text_primary #cfd4e2; пункт — p.bg_surface_hover #3b3b52 (сплошной), цвет текста НЕ меняется

## Отличия от original.md той же папки
1. Кнопка показывает СЧЁТЧИК скрытых («N ▾») и имеет размер по содержимому (h 24, px 6); у оригинала — квадрат 24×24 только с chevron.
2. Кнопка: hover-фон text-primary 8% вместо `--bg-surface-hover`, и это единственное расхождение в самой кнопке; тултипа «More open files» и aria-label нет.
3. Меню: min-w 200 / max-w 360 / p 4 / radius-md / bg-surface / shadow 0 6 24 30% — совпадают с оригиналом 1:1. Бордер `text_primary α .06` = `--divider-soft` (color-mix text-primary 6%) — тоже совпадает.
4. max-height 400px фикс вместо `60vh`, и `overflow-y: auto` не портирован: у нас `overflow_hidden` — лишние пункты обрезаются, прокрутки в меню НЕТ.
5. `top: 28` фикс вместо `calc(100% + 2px)`; z-index 30 → `deferred(priority 60)`.
6. Пункт: gap 6, px 8, py 5, radius-sm, fs 12, text-secondary — совпадают 1:1; hover bg-surface-hover совпадает, но цвет текста на hover НЕ поднимается до `--text-primary`.
7. Активный пункт не подсвечен: `.overflowItemActive` (accent-primary 16% + text-primary) не портирован.
8. Pin-иконка в пункте отсутствует (оригинал показывает `codicon-pinned`).
9. `title={путь}` у пункта нет (у нас тултипа на пунктах меню нет вовсе).
10. Dirty-точка — accent-orange, как у оригинала и как у нашего таба №111 (прежнее расхождение с accent-primary устранено).
11. `.overflow { padding-right: var(--space-1) }` у контейнера кнопки не портирован.
12. Escape и mousedown-вне закрывают меню — совпадает с оригиналом; scrollIntoView выбранного таба не нужен (активный таб принудительно видим, №110).
