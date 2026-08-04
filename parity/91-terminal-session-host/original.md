# 91 terminal-session-host — оригинал
Файлы: `src/renderer/components/terminal/TerminalSession.tsx` (142-149 — JSX; 43-140 — жизненный цикл), `src/renderer/components/terminal/TerminalView.module.css` (`.session`)

## JSX-структура (кратко, вложенность)
```
div.session (ref=hostRef) [data-pty-id=ptyId] style={display: active ? "flex" : "none"}
└─ (внутрь xterm.open() монтирует свой DOM)
```
Один инстанс на `ptyId`; при неактивности `display:none` — буфер/курсор/скролл выживают. `data-pty-id` используется external-drop.ts для роутинга file-drop paste.

## Метрики (ИЗ CSS, точные значения)
- `.session`: `position:absolute`; `top:8px; right:22px; bottom:10px; left:14px`; flex column
  - right 22px шире прочих — место под scroll thumb xterm; flush-right курсор не налезает на скруглённый угол
  - абсолютные инсеты = гуттеры, чтобы FitAddon (clientWidth/Height) давал корректные cols/rows без клипа нижней строки
- Inline: `display:flex` (active) / `display:none` (неактивен)
- xterm-опции (из TSX): `cursorBlink:true`; fontFamily `var(--font-mono)` (fallback `monospace`); `fontSize:13`; `scrollback:5000`
- Тема xterm из токенов: background `--editor-bg` (fallback `#1d1c25`), foreground `--editor-fg` (`#dcdce4`), cursor `--editor-cursor` (`#a0a0d0`); перечитывается по rAF при смене `resolvedTheme`/`activeContributedThemeId`
- Клавиатура: Ctrl/Cmd+C при выделении — copy+clearSelection (не SIGINT); Ctrl+V не перехватывается (нативный paste)
- hover/active/focus css — нет; transition — нет

## Состояния (классы-варианты с метриками)
- active (`display:flex`) / скрыт (`display:none`, смонтирован)
- exit процесса: в буфер пишется `\r\n\x1b[2m[process exited with code N]\x1b[0m\r\n`
- ResizeObserver на хосте → `fit.fit()` + `pty.resize(cols, rows)`
