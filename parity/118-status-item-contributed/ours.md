# 118 status-item-contributed — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\status_bar.rs:31-63 (`ContribItem`), 66-89 (`rich_text`), 92-131 (`contrib`), 209-214 и 263-269 (размещение слева/справа по alignment+priority)

## Структура/содержание
```
div#sbi-{id}: flex.items_center.gap(4).px(8).rounded(4)
└─ children(rich_text(text))   — «$(icon)» → codicon 12px + текст-куски
(tooltip)  .tooltip(KaminTooltip)
(command)  .cursor_pointer.hover(...).on_mouse_down → поток → host RPC
           «kamin:command:execute» с аргументом command
```
`ContribItem` — `StatusBarItemState` 1:1 (id / alignment / priority / text / tooltip / command / color / visible), парсится из JSON (`from_value`, status_bar.rs:46-62). Цвет `#hex` → `parse_hex`, иначе `p.text_muted`. Alignment 1 → левая группа (сортировка priority DESC), 2 → правая (priority ASC).

## Метрики (из кода, точные)
- отступы: px 8 (SPACE_2), py НЕТ (как `.item { padding: 0 var(--space-2) }`); высота 24 — растяжка по бару (`align-items: stretch`, у ряда нет `items_center`)
- гэпы: внутри пилюли gap 4 (глиф ↔ текст); между пилюлями gap 2 у левой и правой групп, gap 4 (SPACE_1) у корня бара
- цвета: fg = `item.color` (`#hex`) либо p.text_muted #838aa0; глифы красятся тем же fg; текст на hover — p.text_primary #cfd4e2
- скругления: rounded 4 (RADIUS_XS)
- шрифты: собственного размера пилюля не задаёт → наследует 11 (FS_XS) от корня бара (status_bar.rs:253); font-weight 400; глифы `$(icon)` — codicon font-size 12 (status_bar.rs:77)
- фоны по ховеру: p.bg_surface #3d3f51 α 0.6 — ТОЛЬКО у пилюль с `command`

## Отличия от original.md той же папки
1. Совпадают 1:1: gap 4, padding 0×8, radius-xs, fs-xs 11, codicon 12, hover `bg-surface 60%` + `text-primary`.
2. Hover только у пилюль с `command` — поведенчески совпадает с оригиналом (`disabled={!command}`, `.item:disabled:hover` прозрачный + text-muted), но у нас это не `<button disabled>`: нет `cursor: default`-семантики и aria-состояния.
3. `item.color` применяется только если строка начинается с `#`; идентификаторы ThemeColor (`statusBarItem.warningBackground`, `charts.red`) молча падают в text-muted.
4. Парсер `$(name)`: имя ищется в нашей `codicon_map`; нераспознанное имя молча ВЫПАДАЕТ из вывода (оригинальный `renderCodiconText` — общий с QuickPick — оставляет спан класса codicon).
5. Тултип — наш `KaminTooltip` (№129) вместо `data-tooltip` + `aria-label`.
6. Инлайновый `style={{color}}` оригинала у нас применяется к контейнеру и к глифам одинаково — совпадает.
