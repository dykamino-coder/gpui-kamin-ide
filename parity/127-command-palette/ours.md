# 127 command-palette — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\command_palette.rs:118-251 (command_palette), 35-115 (filter, command_row); инпут — root.rs:3909-3915; константы crates\metrics\src\lib.rs:23-24

## Структура (gpui-дерево кратко)
```
div (скрим): absolute.size_full.pt(84).bg(rgba(0,0,0,.5)) — клик = close, Esc/Enter на key_down
└─ панель: w(640).max_h((vh−84−48)×0.75).rounded(12).bg(bg_mantle).shadow(0 8 32 .5)
   ├─ input-ряд: search-codicon 16 + Input + kbd «Esc»
   ├─ список p(4).gap(1), row × ≤50:
   │  row: [category: ][title] … [id mono]
   └─ футер «{N} command(s) · Enter to run»
```
Фильтр: substring по title/id/category, внутренние `_`-команды скрыты. Enter запускает первый ряд.

## Метрики (из кода, точные)
- Скрим rgba(0,0,0,.5); top 84 (PALETTE_TOP_OFFSET), w 640 (PALETTE_WIDTH)
- Панель: rounded 12, bg p.bg_mantle #262533, border 1 p.bg_surface a=.8, shadow 0 8 32 rgba(0,0,0,.5)
- Input-ряд: px 16 (SPACE_4), py 6, gap 8, border-b bg_surface a=.6; search 16px p.text_muted
- kbd: «JetBrains Mono» fs 11, bg p.bg_overlay #515567 a=.5, px 6, py 2, rounded 4
- Row: px 12 (SPACE_3), py 8 (SPACE_2), rounded 8, fs 13 (FS_MD), baseline, justify_between, gap 12; category p.text_muted weight 500; title p.text_primary ellipsis; id mono fs 11 p.text_muted
- Первый ряд: bg p.accent_primary a=.12; hover a=.18
- Футер: px 16, py 8, border-t bg_surface a=.6, fs 11, p.text_muted
- Empty: px 16, py 12, italic, p.text_muted

## Отличия от original.md той же папки
1. Input-ряд py 6 вместо space-3 (12) — осознанная компенсация: gpui-Input несёт собственную высоту ~30px (комментарий в коде).
2. max-h = (vh−84−48)×0.75 — аппроксимация 60vh, не точное значение.
3. Empty-текст «No commands match» без кавычек-query (оригинал: `No commands match "{query}"`).
4. Скрим — div, не `<button aria-label>`; клавиатурная навигация стрелками отсутствует (Enter = первый).
5. Первый ряд 12% + hover 18% — совпадают; футер/kbd/цвета — совпадают.
6. MAX_ROWS 50 — кап как PALETTE_MAX_ROWS оригинала.

## Дополнение атрибутов (цикл 10)

- скругления: панель border-radius 12 (RADIUS_MD) (command_palette.rs:191); строка row border-radius 8 (RADIUS_SM) (command_palette.rs:96); kbd «Esc» border-radius 4 (RADIUS_XS) (command_palette.rs:235); у скрима, input-ряда и футера скруглений нет
