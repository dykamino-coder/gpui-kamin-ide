# 42 activity-picker-dots-trigger — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityPicker.tsx:126-138` (+ обёртка 176-181), `ActivityBar.module.css` (`.pickerAnchor`, `.pickerAnchorInline`, `.picker`)

## JSX-структура (кратко, вложенность)
```
<div class="pickerAnchor | pickerAnchorInline" ref={anchorRef}>   // anchor для clamp-позиционирования
  <button type="button"
          class="picker"
          data-tooltip="Add or remove items"
          aria-haspopup="listbox"
          aria-expanded={open}
          aria-label="Add or remove items"
          onClick={e.stopPropagation(); toggle open}>
    <i class="codicon codicon-more" aria-hidden="true"/>
  </button>
  {menu}   // портал-listbox, элемент 44
</div>
```
- variant="dots" (default). Место в DOM: после `.list` при align="top", перед — при align="bottom" (тогда popDirection="down").

## Метрики (ИЗ CSS, точные значения)
`.pickerAnchor`:
- `position: relative; display: flex; justify-content: center; width: 100%`

`.pickerAnchorInline` (inline-вариант для PanelPlaceholder):
- `position: relative; display: inline-flex` (без width:100%)

`.picker` (общий селектор с `.btn`):
- `width: 32px; height: 32px; display: grid; place-items: center`
- `background: transparent; border: none; border-radius: var(--radius-sm)`
- `color: var(--text-muted); font: inherit; cursor: pointer`
- `transition: background var(--transition-fast), color var(--transition-fast)`
- `.picker :global(.codicon)` — `font-size: 18px; line-height: 1`
- `.picker img` — `width: 18px; height: 18px; object-fit: contain`

## Состояния (классы-варианты с метриками)
- `.picker:hover`: `background: color-mix(in srgb, var(--bg-surface) 50%, transparent); color: var(--text-primary)`
- Открытое меню НЕ подсвечивает триггер (нет active-класса), только `aria-expanded="true"`.
- Popup offset от триггера: `POPUP_OFFSET_PX = 6` (TSX-константа, для clampToViewport).

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin у `.picker` НЕТ — кнопка делит правило с `.btn`: 32×32, `display: grid; place-items: center` (`activity-bar/ActivityBar.module.css:53-66`); обёртка `.pickerAnchor { position: relative; display: flex; justify-content: center; width: 100% }` тоже без padding/margin (`:104-109`), инлайн-вариант `.pickerAnchorInline` — только `position: relative; display: inline-flex` (`:114-117`, «No margin-top» в комментарии `:111-113`); вертикальный зазор от списка даёт `.bar { gap: var(--space-2) }` = 8 (`:9`)
