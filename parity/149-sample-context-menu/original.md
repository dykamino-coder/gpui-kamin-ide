# 149 sample-context-menu — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:143-159, components/activity-bar/ActivityContextMenu.module.css

## Содержание/структура
`ContextMenuRow()` в Block «Context menu», hint: «ActivityContextMenu recipe — right-click in the live UI; here a static preview of the same surface.»
Статичное превью (`style="position: static; boxShadow: none"`):
- `<div class=menu role=menu>`:
  1. `<button role=menuitem class=item>`: `codicon-eye-closed` + `<span class=itemLabel>Hide</span>`
  2. `<button role=menuitem aria-haspopup=menu class="item itemMoveTo">`: `codicon-arrow-right` + `<span class=itemLabel>Move to</span>` + `codicon-chevron-right` c классом `.chevron`

## Метрики
- `.menu`, `.submenu`: position fixed (превью — static); z-index `--z-dropdown` (100); min-width 180px; background `--bg-surface` (#3d3f51); border `1px solid var(--divider-soft)`; border-radius `--radius-md` (12px); box-shadow `--shadow-dropdown` (0 4px 16px rgba(0,0,0,0.5)); list-style none; margin 0; padding 4px; flex column; gap 1px; max-height `calc(100vh - 16px)`; max-width `calc(100vw - 16px)`; overflow-y auto
- `.item`, `.subItem`: flex; align-items center; gap 8px; width 100%; padding 8px 12px; background transparent; border none; border-radius 8px; color `--text-primary`; font inherit, `--fs-sm` (12px); text-align left; cursor pointer
- `.itemLabel`, `.subItemLabel`: flex 1
- `.chevron`: font-size 12px; color `--text-muted`
- `.subItemIcon`: inline-flex, центрирование, color `--text-muted`

## Состояния/варианты
- hover (`.item:hover`, `.subItem:hover`): background `color-mix(in srgb, var(--text-primary) 10%, transparent)`
- открытый сабменю (`.itemMoveTo[aria-expanded="true"]`): background `color-mix(in srgb, var(--accent-primary) 16%, transparent)`; color `--text-primary` — строка «Move to» остаётся подсвеченной как breadcrumb
- в живом UI меню и сабменю рендерятся порталом в `<body>` с position: fixed
