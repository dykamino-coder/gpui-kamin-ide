# 30 session-pin-button — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionItem.tsx` (111-119), `SessionItem.module.css`

## JSX-структура (кратко, вложенность)
```
<button .action.pin[.pinned при session.pinned]
        aria-label="Pin session"|"Unpin session"
        data-tooltip="Pin to top bar"|"Unpin from top bar"
        onClick={stopPropagation; toggleSessionPinned}>
  <i .fas.fa-thumbtack aria-hidden/>
</button>
```
Кнопка «всегда на месте» в строке (последняя перед порталом), но по CSS скрыта до hover, если сессия не запинена.

## Метрики (ИЗ CSS, точные значения)
- `.action` (база):
  - `display: none` (скрыта без hover — нулевая layout-стоимость)
  - `align-items: center; justify-content: center`
  - `width: 20px; height: 20px; flex-shrink: 0; padding: 0`
  - `background: transparent; border: none; border-radius: var(--radius-xs)`
  - `cursor: pointer; color: var(--text-muted)`
- `.action > i`: `font-size: 13px`
- `.pin > i`: `font-size: 10px` (fa-thumbtack чанковее codicons — уменьшен)

## Состояния (классы-варианты с метриками)
- `.row:hover .action`: `display: inline-flex; opacity: 0.7`
- `.action:hover`: `opacity: 1 !important`
- `.pin.pinned`: `display: inline-flex; opacity: 1; color: var(--tab-color)` — запиненная видима всегда, без hover
- `.pin:hover`: `color: var(--tab-color)`
