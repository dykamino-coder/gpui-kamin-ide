# 35 customize-mode-nav — наша реализация
Файлы: `crates\shell\src\ui\customize.rs:18-25` (PANELS), `:32-160` (customize_nav), `crates\shell\src\root.rs:5253-5263` (монтаж: плоский сайдбар без карточки)

## Структура (gpui-дерево кратко)
```
div .flex_col .size_full .gap(2) .px(SPACE_2=8) .py(SPACE_3=12)
├─ header: div .px(8) .pb(8) .text(FS_XS=11, text_muted) "CUSTOMIZE"
├─ PANELS.map(nav-item)                 ← элемент 36; 5 пунктов:
│    Settings(settings-gear) / Design(symbol-color) / Extensions(extensions)
│    / Logs(output) / System(pulse)     — набор и иконки 1:1
└─ contributed-узел + страницы          ← элемент 37
```

## Метрики (из кода, точные)
- Колонка: gap 2, px 8, py 12
- Header: px 8 (итог слева 8+8=16), pb 8, fs 11, `text_muted` #838aa0

## Отличия от original.md той же папки
1. Header-инсет: у нас 16px слева (px колонки 8 + px хедера 8) vs оригинальные 12 (`padding: 8px 12px` при list-инсете 8).
2. **`font-weight: 500`, `letter-spacing: 0.08em`, `font-feature-settings: "ss01"` у титула НЕ ПЕРЕНЕСЕНЫ** — обычный regular без разрядки.
3. Вертикальный ритм: оригинал `.root { padding: 12px 0; gap: 8px }` + `.header { padding: 8px 12px }`; у нас py 12 + pb 8 у хедера — близко, но `gap: 8` между хедером и списком заменён паддингом, а строки идут с общим gap 2 колонки.
4. Список — не `<ul>/<li>` (в gpui нет семантики), стили эквивалентны (`padding: 0 8; gap: 2` → px 8 / gap 2 на колонке).
