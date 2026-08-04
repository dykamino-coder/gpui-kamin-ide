# 124 quick-open — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\quick_open.rs:93-194 (quick_open), 41-90 (hit_row); инпут/подписка — root.rs:3917-3925

## Структура (gpui-дерево кратко)
```
div (backdrop): absolute.size_full.flex.justify_center.items_start.pt(108).bg(rgba(0,0,0,.35))
├─ input_area() + on_key_down (escape → close; enter → открыть ПЕРВЫЙ)
├─ mouse_down → close
└─ бокс: w(640).max_w(vw−32).rounded(12).bg(bg_mantle).border(bg_surface .6).shadow(0 6 24 .4)
   ├─ input-ряд: px(14).py(12).border_b(bg_surface .5) → Input(appearance false)
   └─ список: py(4).max_h(480), row × ≤50
      row: name (medium) + dir-путь (ellipsis)
```
Изменение текста инпута сразу шлёт `kamin:index:findFile` (root-подписка). Первый ряд подсвечен, Enter открывает его.

## Метрики (из кода, точные)
- Backdrop: rgba(0,0,0,0.35); pt = 0.12×900 = 108 (константа, НЕ от вьюпорта)
- Бокс: w 640, rounded 12 (RADIUS_MD), bg p.bg_mantle #262533, border p.bg_surface #3d3f51 a=.6, shadow 0 6 24 rgba(0,0,0,.4)
- Input-ряд: px 14, py 12, border-b bg_surface a=.5
- Список: max_h 480, MAX_ROWS 50
- Row: baseline, gap 8, px 14, py 6; name fs 12 (FS_SM) weight 500 p.text_primary; путь fs 11 p.text_muted ellipsis
- Первый ряд/hover: bg p.accent_primary #89b4fa a=.14
- Empty «No matches»: px 14, py 12, fs 12, p.text_muted, по центру

## Отличия от original.md той же папки
1. pt 108 — фиксированный (0.12×900), а не 12vh реального вьюпорта: на других высотах позиция уезжает.
2. Навигации стрелками (ArrowUp/Down + mouseenter-active) НЕТ — активен всегда первый ряд, Enter открывает только его.
3. Путь не выровнен вправо (`text-align: right` нет) — идёт сразу за именем.
4. Debounce 80ms отсутствует — запрос на каждый ввод.
5. backdrop-filter: blur(2px) нет (у скрима только альфа).
6. «No matches» показывается и при пустом query (оригинал — только при непустом).
7. Light-темы вариант active-строки (bg accent_primary + fg accent-action-fg) не реализован.
8. w/max-w бокса, паддинги инпута и строк (12/14, 6/14), цвет подсветки 14% — совпадают.
