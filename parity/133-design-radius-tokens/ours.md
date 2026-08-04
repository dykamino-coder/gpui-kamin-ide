# 133 design-radius-tokens — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\design_panel.rs:652-679 (секция Radius), 427-450 (`token_name` / `token_value`), 900-905 (вызов `section`); crates\metrics\src\lib.rs:35-39 (RADIUS_*)

## Структура/содержание
```
radius div.flex.flex_wrap.gap(12)
└─ колонка × 4: div.w(120).flex_col.items_center.gap(4)
   ├─ бокс div 80×80.rounded(токен).border_1(accent_primary α .5).bg(bg_surface)
   ├─ token_name «--radius-xs» / «--radius-sm» / «--radius-md» / «--radius-lg»
   └─ token_value «4px» / «8px» / «12px» / «16px»
```
Сабтайтл секции: «4-step concentric scale anchored at 16px outer».

## Метрики (из кода, точные)
- отступы: у колонки и бокса собственных padding/margin нет; внешний даёт `section()` — тело p 16 (SPACE_4), секция mb 24 (SPACE_6)
- гэпы: ряд колонок gap 12 (SPACE_3); внутри колонки gap 4 (SPACE_1)
- цвета: бокс bg p.bg_surface #3d3f51, border 1px p.accent_primary #89b4fa α 0.5; `token_name` p.text_muted #838aa0; `token_value` p.text_disabled #60667b
- скругления: демонстрируемые токены — 4 (RADIUS_XS), 8 (RADIUS_SM), 12 (RADIUS_MD), 16 (RADIUS_LG); у колонок скруглений нет
- шрифты: обе подписи — «JetBrains Mono» 11 (FS_XS) weight 400; `token_value` в фикс-колонке 60px
- ховер: N/A: ховер — витрина статична, ни одного `.hover(...)` в секции Radius

## Отличия от original.md той же папки
1. Значения 4 токенов идентичны (4 / 8 / 12 / 16). Алиас `--radius-xl` (= 16, legacy) не портирован — вызовов нет, в оригинальной витрине он тоже не показан.
2. Бокс 80×80, bg `bg-surface`, border `accent-primary 50%`, колонка gap 4 + items-center, подписи `--radius-*` + «Npx» — совпадают с оригиналом.
3. Layout: CSS-grid `repeat(auto-fill, minmax(120px, 1fr))` заменён на flex-wrap с фикс-колонкой 120px (в gpui нет grid) — колонки не растягиваются по остатку строки.
4. `token_value` у нас в фикс-боксе шириной 60px внутри колонки, выровненной по центру: при коротком «4px» текст остаётся в 60px-боксе, из-за чего центрирование подписи чуть отличается от оригинала (там `<span>` по контенту).
5. Правило шкалы «outer = inner + padding» вынесено в сабтайтл секции — в оригинале это комментарий в CSS.
