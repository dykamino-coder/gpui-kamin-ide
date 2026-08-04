# 126 workspace-symbols — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\workspace_symbols.rs:118-220 (workspace_symbols), 40-115 (kind_glyph, symbol_row); инпут/подписка — root.rs:3946-3961

## Структура (gpui-дерево кратко)
Тот же бокс, что QuickOpen (№124):
```
backdrop (.35, pt 108) → бокс w(640).bg(bg_mantle).rounded(12).shadow(0 6 24 .4)
├─ input-ряд px(14).py(12).border_b
└─ список py(4).max_h(480), row × ≤100:
   [SymbolKind-codicon 14 accent_blue] [name medium] [«container · basename» ellipsis]
```
Запрос при len≥1 (`kamin:lang:workspaceSymbol`); Enter/клик → `OpenFile(uri)`.

## Метрики (из кода, точные)
- Как №124: backdrop rgba(0,0,0,.35), pt 108 (0.12×900 фикс), бокс w 640, rounded 12, bg #262533, border bg_surface a=.6, shadow 0 6 24 .4; input px 14 py 12; row px 14 py 6 gap 8
- Иконка kind: codicon 14px, p.accent_blue #89b4fa
- name: fs 12 weight 500 p.text_primary; path: fs 11 p.text_muted ellipsis
- Первый ряд/hover: p.accent_primary a=.14; MAX_ROWS 100; empty «No symbols» при query>0
- Kind-карта: 1..3 namespace, 4 class, 5|8|11|12 method/fn, 6 property, 7 field, 9 enum, 10 interface, 13 variable, 14 constant, 22 struct, 23..24 event, иначе symbol-misc

## Отличия от original.md той же папки
1. Открытие БЕЗ reveal диапазона: `OpenFile(uri)` вместо `openFileAt(uri, range)` — курсор не прыгает к символу.
2. Иконка kind окрашена accent_blue (у оригинала цвет по умолчанию codicon = текущий text-цвет).
3. pt фиксированный 108 (не 12vh вьюпорта); blur нет; стрелок нет; light-тема active — нет (наследие №124).
4. Debounce 120ms отсутствует (запрос на каждый ввод, min 1 символ — совпадает).
5. Kind-карта чуть шире оригинала (2..3 → namespace-глиф; 8/11/12 слиты в method/fn) — визуально совместимо, но constructor(8) у оригинала отдельный глиф.

## Дополнение атрибутов (цикл 10)

- скругления: бокс border-radius 12 (RADIUS_MD) (workspace_symbols.rs:209); у строк списка радиуса нет (`symbol_row` без `.rounded`, workspace_symbols.rs:82-89) — совпадает с оригиналом (`.item` без border-radius)
