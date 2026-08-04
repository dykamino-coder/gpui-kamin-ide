# 125 find-in-files — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\find_in_files.rs:127-234 (find_in_files), 45-123 (hit_row, split3); инпут/подписка — root.rs:3927-3944

## Структура (gpui-дерево кратко)
```
div (backdrop): absolute.size_full.pt(0.12×vh).bg(rgba(0,0,0,.35)) — клик/Esc close, Enter → первый хит
└─ бокс: w(720).max_w(vw−32).max_h(0.76×vh).rounded(12).bg(bg_mantle).shadow(0 6 24 .4)
   ├─ input-ряд: px(14).py(12).border_b
   ├─ status: «Searching…» | «Type at least 2 chars» | «{N} hits»
   └─ список (скролл), row × ≤200:
      ├─ header: rel (ellipsis) + «:{line}»
      └─ snippet: mono, [до][match: bg accent_orange .35][после]
```
Запрос при len≥2 (root-подписка, busy-флаг); клик/Enter → `OpenFileAt(abs, line)`.

## Метрики (из кода, точные)
- Backdrop: rgba(0,0,0,.35); pt = 12% высоты вьюпорта (min 600)
- Бокс: w 720, max_h 76% vh, rounded 12, bg p.bg_mantle #262533, border p.bg_surface a=.6, shadow 0 6 24 rgba(0,0,0,.4)
- Input: px 14, py 12, border-b bg_surface a=.5
- Status: px 14, py 6, fs 11 (FS_XS), p.text_muted
- Row: px 14, py 6, rounded 4 (RADIUS_XS), gap 2, flex-col; header fs 11 p.text_muted; snippet «JetBrains Mono» fs 11 p.text_secondary
- Match: bg p.accent_orange #fab387 a=.35, rounded 2, text p.text_primary
- Первый ряд/hover: bg p.accent_primary a=.14; MAX_ROWS 200

## Отличия от original.md той же папки
1. pt 12% vs 10vh у оригинала (окно ниже).
2. Debounce 220ms отсутствует — запрос на каждый ввод при len≥2 (порог 2 символа совпадает).
3. Стрелочной навигации нет; активен всегда первый хит.
4. backdrop-filter: blur(2px) нет.
5. `font-variant-numeric: tabular-nums` на «:{line}» нет.
6. box-shadow свой (0 6 24 .4) vs var(--shadow-dropdown).
7. Совпадает: w 720 / max-h 76vh, паддинги 12/14 и 6/14, статус-тексты дословно, подсветка матча accent-orange 35% rounded 2, кап 200.

## Дополнение атрибутов (цикл 10)

- шрифты: header ряда (rel + `:line`) font-size 11 (FS_XS), font-weight 400 (find_in_files.rs:83); snippet — font-family «JetBrains Mono» (моно), font-size 11 (FS_XS) (find_in_files.rs:102-103); status-строка font-size 11 (FS_XS) (find_in_files.rs:221); input-ряд собственного font-size не задаёт — наследует базовый размер окна (find_in_files.rs:206-215)
