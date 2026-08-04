# 34 session-color-swatches — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\SessionContextMenu.tsx` (67-87), `SessionContextMenu.module.css`

## JSX-структура (кратко, вложенность)
```
<div .swatches>                                  ← внутри .menu (элемент 33)
  SESSION_COLORS.map(c =>
    <button .swatch [.swatchActive при s.color === c.value]
            style={background: resolveSessionColor(c.value)}
            aria-label="Set colour {c.value}"
            onClick={setSessionColor(s.id, c.value); close}/>)
  <button .swatchClear aria-label="Clear colour" data-tooltip="Clear colour"
          onClick={setSessionColor(s.id, null); close}>
    <i .codicon.codicon-circle-slash/>
  </button>
</div>
```
Палитра — `SESSION_COLORS` из `signals/sessions.js`; фон свотча — inline через `resolveSessionColor`.

## Метрики (ИЗ CSS, точные значения)
- `.swatches`:
  - `display: flex; align-items: center; gap: 4px; flex-wrap: wrap`
  - `padding: 6px 8px`
- `.swatch`:
  - `width: 16px; height: 16px; border-radius: 50%`
  - `border: 2px solid transparent; padding: 0`
  - `cursor: pointer`
  - background — inline (цвет сессии)
- `.swatchClear`:
  - `width: 18px; height: 18px`
  - `display: grid; place-items: center`
  - `background: transparent; border: none; border-radius: 50%`
  - `color: var(--text-muted); cursor: pointer`
- `.swatchClear .codicon` (`:global`): `font-size: 13px`

## Состояния (классы-варианты с метриками)
- `.swatch:hover`: `transform: scale(1.15)` (transition не задан)
- `.swatchActive` (текущий цвет сессии): `border-color: var(--text-primary)`
- `.swatchClear:hover`: `color: var(--text-primary)`
