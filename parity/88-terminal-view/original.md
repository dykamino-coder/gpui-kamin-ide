# 88 terminal-view — оригинал
Файлы: `src/renderer/components/terminal/TerminalView.tsx` (54-77), `src/renderer/components/terminal/TerminalView.module.css`

## JSX-структура (кратко, вложенность)
```
div.root
├─ <TerminalToolbar shells defaultShellId sessions onOpen onSetDefault onClose onSelect />  (элемент 89)
└─ div.body
   ├─ sessions.map → <TerminalSession ptyId active />   (элемент 91; все смонтированы, неактивные display:none)
   └─ sessions.length===0 → div.empty
      ├─ i.codicon.codicon-terminal
      └─ p — «No terminal yet — pick a shell from the “+” menu.»
```
Состояние per-slot (`getTerminalSessions(slot)`/`getActiveTerminal(slot)`); авто-открытие первой сессии один раз на слот (guard hasAutoOpenedRef + openInFlight).

## Метрики (ИЗ CSS, точные значения)
- `.root`: `flex:1`; flex column; `min-height:0`; margin `0 6px 6px` (top 0 — тулбар прижат к activity tab strip; лево/право/низ 6px — гуттеры карточки); background `var(--bg-mantle)`; `border-radius:var(--radius-md)`; `overflow:hidden`
- `.body`: `flex:1; position:relative; min-height:0`; background `var(--editor-bg)` (та же поверхность, что редактор; xterm красит то же значение); `border-radius:var(--radius-md)`; `overflow:hidden`
- `.empty`: `position:absolute; inset:0`; flex column, центрирование; `gap:var(--space-2)`; color `var(--text-muted)`
  - `.empty .codicon`: `font-size:28px; opacity:0.6`
  - `.empty p`: `margin:0`; font-size `var(--fs-sm)`
- hover/active/focus — нет; transition — нет

## Состояния (классы-варианты с метриками)
- empty-state при 0 сессий (absolute поверх `.body`)
- Тулбар на mantle-фоне; активный таб + body образуют единую editor-bg поверхность (см. 89)
