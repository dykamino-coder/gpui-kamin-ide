# 87 problem-row — оригинал
Файлы: `src/renderer/components/problems/ProblemRow.tsx` (26-41), `src/renderer/components/problems/ProblemsPanel.module.css` (секция «Diagnostic row», строки 133-177)

## JSX-структура (кратко, вложенность)
```
button.row (onClick → openFileAt(uri, diag.range))
├─ i.codicon.codicon-{error|warning|info|lightbulb}.sevIcon.{sevError|sevWarning|sevInfo|sevHint}
├─ span.message [data-tooltip=diag.message] — diag.message
├─ origin && span.origin — «source(code)» | «source» | «code»
└─ span.location — «[Ln {startLine+1}, Col {startChar+1}]»
```
Severity map: 0→error/sevError, 1→warning/sevWarning, 2→info/sevInfo, 3→lightbulb/sevHint; неизвестное → error.

## Метрики (ИЗ CSS, точные значения)
- `.row`: flex, `align-items:center`; `gap:6px`; `width:100%; min-height:22px`; padding `0 var(--space-2) 0 26px` (левый отступ 26px — индент под иконку файла); background transparent; border none; color `var(--text-secondary)`; `text-align:left`; `white-space:nowrap; overflow:hidden`; `font:inherit`; font-size `var(--fs-sm)`; `cursor:pointer`
  - hover: background `color-mix(in srgb, var(--bg-surface) 60%, transparent)`; color `var(--text-primary)`
- `.sevIcon`: `flex-shrink:0`; `font-size:14px`
- `.sevError`: color `var(--accent-red)`; `.sevWarning`: `var(--accent-yellow)`; `.sevInfo`: `var(--accent-blue)`; `.sevHint`: `var(--text-muted)`
- `.message`: `flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis`
- `.origin`: `flex-shrink:0`; color `var(--text-muted)`; font-size `var(--fs-xs)`
- `.location`: `flex-shrink:0`; color `var(--text-muted)`; font-size `var(--fs-xs)`
- transition — нет

## Состояния (классы-варианты с метриками)
- 4 severity-класса иконки (цвета выше)
- `.origin` — условный (только при source/code)
- hover — подсветка строки + осветление текста
