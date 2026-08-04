# 45 activity-context-menu — наша реализация
Файлы: `crates/shell/src/overlay.rs:1121-1238` (`tool_tab_menu()` — корневое меню), рендер в overlay-окне (overlay.rs:974 передаёт `tool_menu_sub`); события `root.rs:1185-1198` (`OpenToolTabMenu`/`CloseToolTabMenu`/`ToolMenuSub`); триггеры: RMB по табу стрипа `slot_panel.rs:72-84`, RMB по плитке рейла `right_column.rs:105-116`.

## Структура (gpui-дерево кратко)
```
div#tool-tab-menu .occlude .absolute (x,y кламп; est_h 92)
  min_w 180, flex col, gap 1, p SPACE_1, rounded RADIUS_MD,
  bg-surface, border 1px text_primary@0.06, hit_area()
  ├ div#ttm-hide:   codicon eye-closed 14px muted + "Hide"
  │     клик → UnpinTool(slot,id) + CloseToolTabMenu
  └ div#ttm-moveto: codicon arrow-right 14px muted + "Move to" + chevron-right 12px
        on_hover(true) → ToolMenuSub(true); открытое → bg accent@0.16
```
- Закрытие: скрим main-окна (клик-мимо через per-pixel hit-test overlay) + `close_popovers_except("ttab")` при открытии других поповеров.

## Метрики (из кода, точные)
- `MENU_W` (min-w) **180px**, кламп-маржин **8px**, оценка высоты **92px**.
- Контейнер: `p(SPACE_1)` 4, `gap 1px`, rounded `RADIUS_MD` **12px**, bg `p.bg_surface` #3d3f51, border 1px `p.text_primary`@0.06.
- Пункт: gap `SPACE_2` **8**, px `SPACE_3` **12**, py `SPACE_2` **8**, rounded `RADIUS_SM` **8px**, `FS_SM` **12px**, текст `p.text_primary` #cfd4e2.
- Hover: bg `p.text_primary`@**0.10**.
- Иконки пунктов: codicon **14px** `p.text_muted` #838aa0; шеврон **12px** muted.
- «Move to» при открытом сабменю: bg `p.accent_primary` #89b4fa @**0.16**.

## Отличия от original.md той же папки
1. **Нет box-shadow**: `box_style` не вызывает `.shadow()` (`--shadow-dropdown` оригинала отсутствует) — меню без тени.
2. **Иконки muted**, в оригинале `<i>` наследует цвет пункта (`--text-primary`).
3. «Move to» открывается ТОЛЬКО hover'ом; клик не тогглит (оригинал: onClick=toggle + onMouseEnter). Ховер «Hide» НЕ закрывает сабменю (оригинал закрывает).
4. Закрытие: нет Escape, нет закрытия по scroll (capture) и window blur — только клик-мимо.
5. Триггеры покрывают стрип-табы и рейлы правых карт, но НЕ плитки сайдбарного бара (у `tile()` нет RMB — см. 39 п.1); оригинал вешает меню на все плитки.
6. Нет `max-height/max-width + overflow-y`; est_h 92 — только для клампа позиции.
7. Нет ролей `menu`/`menuitem`, `aria-haspopup`/`aria-expanded`.
8. Паддинги/gap/радиусы/hover 10% — совпадают с оригиналом 1:1.

## Дополнение атрибутов (цикл 10)

- шрифты: пункты — кегль FS_SM 12, глифы codicon 16 (базовый `.codicon`), chevron 12 (`overlay.rs`, блок tool-меню)
