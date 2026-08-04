# 88 terminal-view — наша реализация
Файлы: `crates/shell/src/root.rs:3962+` (ветка "terminal" в tool_body), `root.rs:137-138` (TERM_CELL_W=7.8, TERM_CELL_H=17.0), `crates/shell/src/term.rs` (TermSession — alacritty_terminal поверх PTY)

## Структура (gpui-дерево кратко)
```
div#terminal-body (track_focus, key_context "Terminal", flex-col, size_full, min_h 0,
                   overflow_hidden, p 8, font «JetBrains Mono» 12px, line-height 16,
                   text_secondary #adb3c7)
├─ term_toolbar(...)   (см. 89)
└─ #terminal-grid (relative, flex-col, flex_1, min_h 0, overflow_hidden)
   ├─ probe_area("terminal") — bounds для resize (cols = w/7.2, rows = h/16)
   ├─ строки грида: div flex h 16, раны с fg-цветом; выделение bg accent_primary 30%
   └─ block-курсор absolute: ячейка 7.2×16, bg accent_primary #89b4fa, символ цветом bg_primary #313240
терминалов нет → центр «Starting shell…» text_muted
```
Ввод: keystroke→PTY, Ctrl+C с выделением = copy (не SIGINT), Ctrl+V = paste (CRLF→CR); scroll-wheel = скроллбэк ×3 строки; drop файла/узла дерева → путь (с кавычками при пробеле) в PTY.

## Метрики (из кода, точные)
- Ячейка 7.2×16; шрифт моно 12px; паддинг тела 8 (SPACE_2).
- Selection: #89b4fa@0.3; курсор — инверсия accent/bg.

## Отличия от original.md той же папки
1. Рендер — собственный грид на alacritty_terminal, не xterm.js: нет `.root` (margin 0 6 6, bg mantle, radius 12) и нет `.body` с фоном `--editor-bg` — терминал рисуется прямо на фоне карты слота (bg_mantle #262533), отдельной editor-bg (#1d1c25) поверхности НЕТ.
2. Шрифт 12px (оригинал xterm fontSize 13).
3. Empty-state: «Starting shell…» без иконки; оригинал — codicon-terminal 28px + «No terminal yet — pick a shell from the “+” menu.» (и сессия у нас автосоздаётся, empty почти не виден).
4. Per-slot состояния нет: один список terminals на приложение (оригинал — сессии на слот).
5. Неактивные сессии не рендерятся вовсе (см. 91) — у оригинала все смонтированы с display:none.
6. Курсор — block accent_primary (оригинал xterm cursor `--editor-cursor` #a0a0d0, blink).
7. text_secondary #adb3c7 как базовый fg (оригинал `--editor-fg` #dcdce4).

## Дополнение атрибутов (цикл 10)

- гэпы: flex-`gap` у карты терминала нет — тулбар (min-h 30) и тело идут встык (`root.rs:3948-3980`), как `.root` оригинала (`TerminalView.module.css:11-23`, gap не объявлен). Внутренние gap принадлежат тулбару (элемент 89: `.bar` gap 4, `.tabs` gap 2 — `term_toolbar.rs:359,130`). Пустое состояние `.empty { gap: var(--space-2) 8 }` (`TerminalView.module.css:63`) не портировано: пустого состояния нет — при показе панели сессия спавнится автоматически (`root.rs:4503`).
- ховер: N/A: ховер — у карты терминала и у сетки ячеек hover-правил нет (`root.rs:3948-3980`, рендер рун `:3834-3860`), как и у `.root`/`.body`/`.session` оригинала; ховеры принадлежат табам и кнопкам тулбара (89/90).
