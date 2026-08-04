# 129 tooltip — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/Tooltip.tsx` (123-138), `Tooltip.module.css`

## JSX-структура (кратко, вложенность)
```
div.tooltip [ref] [data-tooltip-popup]
  style={ left: {px}, top: {px}, opacity: visible?1:0, visibility: anchor?visible:hidden }
└─ {text}
```
- Единственный инстанс на документ (монтируется в App.tsx); слушает `pointerenter`/`pointerleave` (capture) по `closest("[data-tooltip]")`.
- Двухпроходное позиционирование: стадия 1 — рендер невидимым (opacity 0), стадия 2 — useLayoutEffect измеряет getBoundingClientRect и `clampToViewport({ side: "top", offset: 8 })` (`OFFSET_PX = 8`) до пейнта.
- Скрытие: pointerleave, mousedown, visibilitychange, window blur, scroll (capture).
- Принимает тултипы из вебвью через сигнал `webviewTooltip` (anchor уже в host-координатах).

## Метрики (ИЗ CSS, точные значения)
`.tooltip`:
- position: fixed; pointer-events: none; z-index: var(--z-tooltip)
- background: var(--bg-surface); color: var(--text-primary)
- padding: 4px 8px
- border-radius: var(--radius-xs)
- font-size: var(--fs-xs); line-height: var(--lh-snug)
- max-width: min(640px, calc(100vw - 16px))
- white-space: nowrap; overflow: hidden; text-overflow: ellipsis
- transition: opacity 0.1s
- box-shadow: var(--shadow-mini)
- left/top задаются inline (px), opacity 0→1 по завершении clamp

## Состояния (классы-варианты с метриками)
- Измерение (стадия 1): visibility: visible, opacity: 0.
- Показан: opacity: 1 (fade 0.1s).
- Нет якоря: visibility: hidden.
- hover/active-классов нет.
