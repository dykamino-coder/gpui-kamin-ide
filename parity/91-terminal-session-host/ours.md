# 91 terminal-session-host — наша реализация

НЕ РЕАЛИЗОВАНО как DOM-хост с `display:none`-персистентностью (xterm-инстансов нет).

Замена: буфер сессии живёт в данных, не в дереве.
Файлы: `crates/shell/src/term.rs:101-...` (TermSession: alacritty_terminal Term + PTY + Proxy), `crates/shell/src/root.rs:187` (`terminals: Vec<TermSession>`), `root.rs:3109-3130` (рендер ТОЛЬКО активной: `terminals.get(term_active)`, resize по probe-bounds), `root.rs:3199-3245` (screen_styled → раны + курсор)

## Структура (gpui-дерево кратко)
- `TermSession` держит alacritty-грид, скроллбэк, выделение и writer PTY; жив, пока таб не закрыт (`TermClose`), независимо от того, отображается ли.
- Каждый кадр рендерится грид только активной сессии (`screen_styled()` → строки-раны); переключение таба = рендер другого буфера, «скрытых DOM-хостов» нет по определению immediate-mode gpui.
- Resize: по bounds probe-области `"terminal"` (cols = w/7.2, rows = h/16) — аналог FitAddon.
- `Proxy` (term.rs:38-53): PtyWrite (DSR/CPR-ответы) → обратно в PTY (без них PSReadLine виснет), прочее → notify UI.

## Метрики (из кода, точные)
- Ячейка: TERM_CELL_W=7.2 × TERM_CELL_H=16 (root.rs:117-118); шрифт «JetBrains Mono» 12px.
- Цвета: fg по ранам из палитры терминала, базовый text_secondary #adb3c7; выделение accent_primary@0.3; курсор block bg #89b4fa / глиф #313240.
- Инсеты хоста: p 8 общего terminal-body (нет асимметричных 8/22/10/14).

## Отличия от original.md той же папки
1. Нет `data-pty-id`-хостов и inline display:flex/none — неактивные сессии не присутствуют в дереве (буфер сохраняется в TermSession, поведенчески эквивалентно).
2. Нет асимметричных инсетов `.session` (top 8 / right 22 / bottom 10 / left 14, зазор под scroll-thumb) — единый p 8; скроллбара-thumb нет вообще (скролл колесом).
3. xterm-опции не применимы: fontSize у нас 12 (оригинал 13); cursorBlink нет (статичный block); scrollback задаётся alacritty-конфигом, не 5000 xterm.
4. Тема: не из `--editor-bg/-fg/-cursor` (#1d1c25/#dcdce4/#a0a0d0), а из палитры UI: фон карты bg_mantle, fg text_secondary #adb3c7, курсор accent_primary #89b4fa — грубое цветовое расхождение.
5. Ctrl+V перехватывается вручную (нативного paste нет); Ctrl+C-c-выделением = copy ✓ как оригинал.
6. Сообщение `[process exited with code N]` в буфер не пишется.
7. ResizeObserver → fit: заменён probe-bounds пересчётом каждый рендер ✓ (эквивалент).

## Дополнение атрибутов (цикл 10)

- скругления: тело сессии radius-md 12 (`root.rs:3971`, `metrics/lib.rs:38`) = `.body { border-radius: var(--radius-md) }` (`TerminalView.module.css:39`); у самой сетки/`.session`-инсета скруглений нет ни там (`:43-54`), ни у нас (инсеты 8/22/10/14 — `root.rs:3973-3976`).
- ховер: N/A: ховер — хост сессии некликабелен в смысле подсветки: обработчики есть (фокус по клику, drop файла, выделение мышью — `root.rs:3944-3947,3910-3943`), но hover-стилей нет; у `.session`/`.body` оригинала hover-правил тоже нет.
