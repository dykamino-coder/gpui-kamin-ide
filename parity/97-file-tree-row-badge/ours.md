# 97 file-tree-row-badge — наша реализация
Файлы: `crates/shell/src/ui/file_list.rs:308-326` (рендер в rows()), `:22-45` (`Deco`, `deco_color`), `crates/shell/src/root.rs:1147-1169` (кэш decorations, инвалидация)

## Структура (gpui-дерево кратко)
```
{deco.badge} → div .flex_shrink_0 .text_size(FS_XS)
    .text_color(deco.color → deco_color(id) | text_muted)
    .child(badge)          // "M"/"U"/…
{нет badge} → пустой div
```
Данные: `kamin:decorations` хоста, кэш `tree.deco: path → Option<Deco>` (None = «запрошено, пусто»); свежие пути запрашиваются при листинге (root.rs:701-703).

## Метрики (из кода, точные)
- `FS_XS` = 11px; прижат вправо за счёт `flex_1` у label (аналог margin-left:auto).
- Цвета `deco_color` (COLOR_MAP 1:1): modified→`accent_orange` #fab387, untracked/added/stageModified→`accent_green` #a6e3a1, deleted/conflicting→`accent_red` #f38ba8, ignored→`text_disabled` #60667b, submodule→`accent_blue` #89b4fa, list.error→red, list.warning→`accent_yellow` #f9e2af, fallback→`accent_blue`.
- Без background/border ✓.

## Отличия от original.md той же папки
1. **Нет `font-weight: 600`** — бейдж обычным весом.
2. **Фолбэк-цвет**: badge без deco.color у нас `text_muted`; в оригинале инлайн-color не ставится → наследует цвет строки (text-secondary/при hover primary).
3. **Нет `data-tooltip`** (deco.tooltip) — тултип декорации не показывается (осознанно, см. комментарий file_list.rs:22).
4. padding-left 6px оригинала компенсирован row-gap 6 — эквивалент; при отсутствии badge у нас рендерится пустой div (в оригинале null) — на layout не влияет.
