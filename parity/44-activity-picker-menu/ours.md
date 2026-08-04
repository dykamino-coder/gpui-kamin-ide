# 44 activity-picker-menu — наша реализация

Файлы: `crates/shell/src/ui/tool_picker.rs` (`tool_picker()`); рендер в overlay-окне
`crates/shell/src/overlay.rs` (ветка `r.tool_picker`); состояние и закрытие —
`root.rs` (`OpenToolPicker`/`CloseToolPicker`/`PinTool`/`UnpinTool`, ветка Escape);
модель — `crates/shell/src/activity.rs` (`BUILTIN_ACTIVITIES`, `dyn_tools_list`).

> Досье переписано в цикле 13: прежний текст описывал код первого цикла
> (py 6, иконка 15, галка 13 accent, `est_h = 40 + 34·N`, фикс-ширина 220,
> «Escape не реализован») — ничего из этого в коде уже нет.

## Структура (gpui-дерево)
```
div#tool-picker .occlude .absolute (left = px_x, top = px_y)
  min_w 220, max_w vw−16, max_h vh−16, overflow_y_scroll,
  flex col, gap 1, p SPACE_1, rounded RADIUS_MD, bg bg-surface,
  border 1px text_primary@0.06, shadow dropdown_shadow()
  ├ hit_area()                                   // hit-регион overlay-окна
  ├ div "TOOLS"   px SPACE_3, py SPACE_1, FS_XS, text_muted
  └ строка × (BUILTIN_ACTIVITIES + dyn_tools_list):
      div#tp-<id> flex items_center gap SPACE_2, px SPACE_3, py SPACE_2,
                  rounded RADIUS_SM, FS_SM, text_primary, cursor_pointer,
                  hover bg text_primary@0.10
        ├ tool_glyph_split(icon, 18, 16)  // svg 18 / codicon 16 (база)
        ├ div .flex_1 label
        └ [pinned] codicon check 16px (цвет наследуется от строки)
```

## Метрики (из кода, точные)
- Контейнер: `min-w` **220** (`PICKER_W`), `max-w` **vw − 16**, `max-h` **vh − 16**,
  `overflow-y: auto`; `p` SPACE_1 **4**, `gap` **1**, `rounded` RADIUS_MD **12**,
  bg `bg_surface` **#3d3f51**, border 1px `text_primary`@**0.06**,
  тень `dropdown_shadow()`.
- Заголовок «TOOLS»: px SPACE_3 **12** / py SPACE_1 **4**, FS_XS **11**,
  `text_muted` **#838aa0**.
- Строка: gap SPACE_2 **8**, px SPACE_3 **12**, py SPACE_2 **8**,
  rounded RADIUS_SM **8**, FS_SM **12**, цвет `text_primary` **#cfd4e2**;
  hover — подложка `text_primary`@**0.10**.
- Иконка тула: svg **18** (`DEFAULT_SIZE_PX`), codicon **16** (база
  `.codicon`); галка codicon-check **16**, цвет наследуется от строки.
- Геометрия считается ПО СОДЕРЖИМОМУ (ревью ц.13 забраковало прежние
  константы 35.2 и «40»): шапка **31.9** = p 4×2 + рамка 2 + (py 4×2 +
  FS_XS·1.169) + gap 1; строка = py 8×2 + max(глиф, FS_SM·1.169, 16) →
  **34** у svg-иконки и **32** у codicon; `est_h` = шапка + сумма строк.
  Ширина — **измеренная шейпером** самая длинная строка (px 12×2 + глиф +
  gap 8 + label [+ 8 + 16]) плюс p 4×2 и рамка, но НЕ поверх `min-w` 220:
  при `box-sizing: border-box` паддинг уже внутри. Кламп:
  `x ∈ [8, vw − menu_w − 8]`, `y = up ? y − est_h − 6 : y + 6` в
  `[8, vh − est_h − 8]`.
- Клик по строке: pinned → `UnpinTool`, иначе `PinTool` (+ активация);
  оба хендлера гасят пикер. Escape закрывает (`root.rs`, цепочка приоритетов
  оверлеев). Клик-мимо — скрим main-окна (per-pixel hit-test).

## Отличия от original.md той же папки
1. `letter-spacing: 0.04em` у заголовка — свойства в gpui нет; текст задан
   заглавными, кернинг оригинала не воспроизводится.
2. Позиционирование одноходовое (кламп по расчётной высоте и измеренной
   ширине) — без двухпроходного `visibility:hidden` + re-measure и без flip
   стороны при resize уже открытого меню.
3. Ролей `listbox`/`option` нет — доступность в gpui-порте не реализована.

## Дополнение атрибутов (цикл 10, переписано в 13)

- отступы: контейнер p SPACE_1 4; строка px SPACE_3 12 / py SPACE_2 8;
  заголовок px 12 / py 4; поле от краёв вьюпорта 8
- цвета: bg-surface #3d3f51, рамка text-primary@6 %, строка text-primary #cfd4e2,
  ховер-подложка text-primary@10 %, заголовок text-muted #838aa0
- шрифты: строка — кегль fs-sm 12; глиф тула 18, галка codicon 16; заголовок FS_XS 11
- скругления: контейнер RADIUS_MD 12, строка RADIUS_SM 8
- гэпы: между строками 1, внутри строки 8
- ховер: только подложка text-primary@10 %; цвет текста и иконки не меняется —
  как `.menuItem` оригинала, который держит `--text-primary` в обоих состояниях
