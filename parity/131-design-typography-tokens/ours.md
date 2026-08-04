# 131 design-typography-tokens — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\design_panel.rs:562-616 (font-сэмплы + шкала), 427-438 (`token_name`), 440-450 (`token_value`), 888-893 (секция «Typography»); crates\metrics\src\lib.rs:41-47 (FS_*); crates\shell\src\main.rs:81-97 (шрифты вшиты в бинарь); crates\shell\src\root.rs:66 (`UI_FONT = "Bricolage Grotesque"`), design_panel.rs:19 (`MONO = "JetBrains Mono"`)

## Структура/содержание
```
typo div.flex_col.gap(12)
├─ font_sample("--font-sans", «Bricolage Grotesque», FS_LG 16)
│  └─ div.flex_col.gap(2): token_name «--font-sans» + строка
│     «Bricolage Grotesque — quick brown fox 0123456789»
├─ font_sample("--font-mono", «JetBrains Mono», FS_MD 13)
│  └─ «JetBrains Mono — quick brown fox 0123456789»
└─ typo_scale div.flex_col.gap(8).mt(8).pt(12).border_t_1(bg_surface α .5)
   └─ ряд × 5: div.flex.items_baseline.gap(12)
      ├─ token_name «--fs-*» w 90
      ├─ token_value «11px|12px|13px|16px|22px» w 60
      └─ «The five steps» размером шага
```
Шкала берётся ИЗ metrics: `m::FS_XS` 11, `m::FS_SM` 12, `m::FS_MD` 13, `m::FS_LG` 16, `m::FS_XL` 22 (design_panel.rs:587-593) — хардкодов в витрине нет.
Шрифты вшиты в бинарь (main.rs:81-97): `bricolage-latin.ttf`, `bricolage-latin-ext.ttf`, `JetBrainsMono-Variable.ttf`, `JetBrainsMono-Italic-Variable.ttf`.

## Метрики (из кода, точные)
- отступы: у `typo` и font-сэмплов padding/margin нет; шкала mt 8 (SPACE_2) + pt 12 (SPACE_3); колонка имени w 90 (в рядах шкалы) / без ширины в font-сэмплах; колонка значения w 60; внешний отступ даёт `section()` — тело p 16 (SPACE_4), секция mb 24 (SPACE_6)
- гэпы: `typo` gap 12 (SPACE_3); font-сэмпл gap 2; шкала gap 8 (SPACE_2); ряд шкалы gap 12 (SPACE_3)
- цвета: `token_name` p.text_muted #838aa0; `token_value` p.text_disabled #60667b; демо-строки p.text_primary #cfd4e2; разделитель над шкалой — border-top 1px p.bg_surface #3d3f51 α 0.5
- скругления: N/A: скругления — в секции Typography ни одного `rounded` (12 RADIUS_MD есть только у рамки `section()`)
- шрифты: `token_name` / `token_value` — «JetBrains Mono» 11 (FS_XS) weight 400; font-sans-сэмпл — «Bricolage Grotesque» 16 (FS_LG); font-mono-сэмпл — «JetBrains Mono» 13 (FS_MD); шкала — «The five steps» на 11 / 12 / 13 / 16 / 22 из `m::FS_*`; ряд выровнен по `items_baseline`
- ховер: N/A: ховер — витрина статична, ни одного `.hover(...)` в секции Typography

## Отличия от original.md той же папки
1. Значения шкалы совпадают 1:1: fs-xs 11, fs-sm 12, fs-md 13, fs-lg 16, fs-xl 22 — и в metrics, и в витрине (берутся из `m::FS_*`, а не дублируются строками, как `FS_SCALE` в tsx; строки-значения используются только для колонки `tokenValue`).
2. Оба font-сэмпла присутствуют, с теми же демо-фразами и размерами (`--font-sans` на fs-lg, `--font-mono` на fs-md) — совпадают.
3. Разделитель над шкалой (mt 8 + pt 12 + border-top bg-surface 50%) и gap 8 у шкалы — совпадают.
4. Ряд шкалы: CSS-grid `90px 60px 1fr` заменён на flex с фикс-ширинами 90 и 60 + `flex_1`-текст; align-items baseline и gap 12 совпадают. Растяжка последней колонки в gpui-flex отличается от `1fr`.
5. Демо-фраза «The five steps» и подписи `--fs-*` / «NNpx» — совпадают.
6. Шрифтовые стеки: у нас ровно 2 семейства из бинаря («Bricolage Grotesque», «JetBrains Mono»); CSS-фоллбеки (`Bricolage Grotesque Variable`, -apple-system, Fira Code, Cascadia Code, Consolas, monospace) не нужны и отсутствуют. Имя семейства у нас — легаси-вариант «Bricolage Grotesque» (не «… Variable»), name-таблица шрифта под это починена.
7. Легаси fs-алиасы (`--fs-xxs`, `--fs-10`, `--fs-base`, `--fs-15`, `--fs-18`, `--fs-2xl`) не портированы — в оригинальной витрине они тоже не показаны.
8. `--lh-*` токены (`none` 1, `snug` 1.3, `normal` 1.4, `base` 1.5, `relaxed` 1.6) в metrics отсутствуют вовсе: line-height по месту считается как `FS × 1.3` и т.п. В витрине они не показаны и у оригинала.
9. `<code class=tokenName>` → обычный div с моно-семейством: визуально эквивалентно, семантики нет.
