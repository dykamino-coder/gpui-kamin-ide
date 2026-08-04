# 121 confirm-modal — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\modal.rs:69-214 (render_modal, dialog_button, dialog_button_bg); рендер — в overlay-окне (crates\shell\src\overlay.rs), скрим-затемнение main-окна — root.rs:5384+

## Структура (gpui-дерево кратко)
```
div (скрим): absolute.size_full.flex.center.bg(rgba(0,0,0,.6)) — клик = cancel
└─ div (диалог): stop_propagation + region_area()
   ├─ title
   ├─ body
   ├─ (prompt) input-блок (№122)
   └─ actions: [Cancel] [Confirm]
```
Esc — обрабатывается снаружи (RootView). Danger-вариант красит Confirm в accent_red.

## Метрики (из кода, точные)
- Скрим: rgba(0,0,0,0.6) (= overlay-deep)
- Диалог: min_w 320, max_w 480, p 20 (SPACE_5), rounded 12 (RADIUS_MD), bg p.bg_primary #313240, border 1 p.bg_surface #3d3f51
- Title: fs 13 (FS_MD), weight 600, p.text_primary #cfd4e2, mb 12 (SPACE_3)
- Body: fs 12 (FS_SM), p.text_secondary #adb3c7, line_height 15.6 (fs×1.3), mb 16 (SPACE_4)
- Actions: gap 8, justify_end
- Cancel: px 16 (SPACE_4), py 4 (SPACE_1), rounded 8, border 1 p.bg_overlay #515567, fs 12, p.text_primary; hover bg p.bg_surface
- Confirm: те же паддинги, bg p.accent_action #89b4fa (danger → p.accent_red #f38ba8), fg p.accent_action_fg #313240, weight 600; hover opacity .9

## Отличия от original.md той же папки
1. box-shadow: var(--shadow-modal) отсутствует (тени у диалога нет).
2. fadeIn-анимация 0.12s отсутствует.
3. Body — плоский текст (sanitized-HTML-рендер не реализован).
4. Confirm hover = opacity .9 вместо bg accent-action-hover #74c7ec; danger hover не accent-maroon.
5. Автофокус Confirm (Enter принимает) и восстановление фокуса — нет.
6. Рендер в отдельном overlay-окне; затемнение фона рисует main-окно (двухоконная схема, у оригинала один DOM).
7. Все размеры/цвета покоя (min/max, p 20, radius 12, бордер, палитра кнопок) — совпадают.
