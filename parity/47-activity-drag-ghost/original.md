# 47 activity-drag-ghost — оригинал
Файлы: `kamin-ide/src/renderer/components/activity-bar/ActivityDragGhost.tsx:13-21`, `ActivityDragGhost.module.css` (`.ghost`)

## JSX-структура (кратко, вложенность)
```
// null, если dragGhost.value пуст. Монтируется один раз в App.tsx.
<div class="ghost"
     style="left:{g.x}px; top:{g.y}px"
     aria-hidden="true">
  <ToolIcon icon={g.icon}/>    // default size 18
</div>
```

## Метрики (ИЗ CSS, точные значения)
`.ghost`:
- `position: fixed; z-index: 9999` (hex-литерал числа, не var)
- `transform: translate(-50%, -50%)` — центр на курсоре
- `pointer-events: none`
- `width: 28px; height: 28px`
- `display: grid; place-items: center`
- `border-radius: var(--radius-sm)`
- `background: color-mix(in srgb, var(--accent-primary) 22%, var(--bg-surface))`
- `border: 1px solid color-mix(in srgb, var(--accent-primary) 50%, transparent)`
- `color: var(--accent-primary)` (иконка акцентная — единственное место, где ToolIcon красится в accent)
- `box-shadow: 0 4px 14px rgb(0 0 0 / 35%)` (hex/rgb-литерал)
- `opacity: 0.92`
- transition/анимаций нет; позиция обновляется инлайн-стилем от сигнала

## Состояния (классы-варианты с метриками)
Одно состояние; существует только во время pointer-drag плитки.

## Дополнение атрибутов (цикл 10)

- отступы: padding/margin у `.ghost` НЕТ (`activity-bar/ActivityDragGhost.module.css:1-16`); бокс 28×28 (`:6-7`), глиф центрируется `display: grid; place-items: center` (`:8-9`), рамка 1px (`:12`). «Отступ» относительно курсора задаётся не padding-ом, а `position: fixed` + `transform: translate(-50%, -50%)` — центр ghost строго на курсоре (`:2,4`), позиция подставляется инлайном `left/top` (`ActivityDragGhost.tsx:13-21`)
