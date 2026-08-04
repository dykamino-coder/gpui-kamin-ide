# 51 tool-icon — оригинал
Файлы: `kamin-ide/src/renderer/components/tool-icon/ToolIcon.tsx:26-48`, `tool-icon/tool-icon-paths.ts`; css-модуля нет (размеры img задают классы вызывающих: `.btnImage`/`.menuItemImage` 18×18, `.tabImage` 13×13)

## JSX-структура (кратко, вложенность)
Три ветки по приоритету:
```
1. isImageIcon(icon)  // /^(?:data:|https?:|file:|\/)/  (signals/activity.ts:89-91)
   <img src={icon} alt="" class={imageClassName} aria-hidden="true"/>

2. TOOL_ICON_PATHS[icon]  // встроенный токен: folders, search, warning, terminal, gear
   <svg class={className} width={size} height={size}
        viewBox="0 0 256 256"            // TOOL_ICON_VIEWBOX = 256
        fill="currentColor" aria-hidden="true">
     <path d={path}/>
   </svg>

3. иначе (codicon-имя)
   <i class="codicon codicon-{icon}[ className]" aria-hidden="true"/>
```

## Метрики (ИЗ CSS, точные значения)
- Собственного CSS нет. Props: `size` (px) — по умолчанию `DEFAULT_SIZE_PX = 18` — идёт в width/height атрибуты SVG; BottomTabBar передаёт 13.
- SVG: `fill="currentColor"` — наследует цвет кнопки (muted → hover primary → active primary; в drag-ghost accent).
- `<img>`-ветка размеров сама не имеет — их дают классы вызывающего:
  - `.btn img`, `.picker img`, `.btnImage`, `.menuItemImage` (ActivityBar.module.css): `width: 18px; height: 18px; object-fit: contain`
  - `.tabImage` (BottomTabBar.module.css): `width: 13px; height: 13px; object-fit: contain`
- codicon-ветка: размер задаёт вызывающий (`.btn/.picker :global(.codicon)` 18px, `.tab :global(.codicon)` 13px, `line-height: 1`).

## Состояния (классы-варианты с метриками)
Состояний нет — чистый рендер. Vendored-иконки: Phosphor regular (одиночный `path`, viewBox 256), ключи-токены: `folders`, `search`, `warning`, `terminal`, `gear`. Неизвестный токен → фоллбек в codicon-шрифт (VSIX-имена работают без изменений).

## Дополнение атрибутов (цикл 10)

- цвета: собственных hex у компонента нет. Ветка Phosphor-SVG: `fill="currentColor"` (`tool-icon/ToolIcon.tsx:39`) — цвет полностью от родителя. Ветка codicon: цвет не задаётся, наследуется (`ToolIcon.tsx:47`). Ветка `<img>`: currentColor НЕ применяется, цвет — свойство ассета, расширения обязаны поставлять монохромные SVG (`ToolIcon.tsx:28`, обоснование в комментарии `activity-bar/ActivityBar.module.css:71-75`). Фактические цвета от вызывающих: `.btn`/`.picker` var(--text-muted) #838aa0, hover var(--text-primary) #cfd4e2, active #cfd4e2 (`ActivityBar.module.css:62,88,96`); `.menuItem` var(--text-primary) #cfd4e2 (`:167`); `.tab` var(--text-secondary) #adb3c7, active #cfd4e2 (`activity-bar/BottomTabBar.module.css:33,66`); `.glyph` плейсхолдера var(--text-disabled) #60667b (`panel-placeholder/ActivityPlaceholder.module.css`, блок `.glyph`)
- отступы: у самого `ToolIcon` ни padding, ни margin — CSS-модуля нет (`ToolIcon.tsx:1-48`). «Отступы» сводятся к размерному боксу: prop `size` по умолчанию 18 (`ToolIcon.tsx:24`, применяется к `width`/`height` SVG `:35-36`); `<img>`-ветка размер берёт из класса вызывающего — `.btnImage`/`.menuItemImage` 18×18 (`ActivityBar.module.css:76-83`), `.tabImage` 13×13 (`BottomTabBar.module.css:50-54`); внешние зазоры до подписи дают контейнеры: `.menuItem { gap: var(--space-2) }` = 8 (`ActivityBar.module.css:161`), `.tab { gap: 6px }` (`BottomTabBar.module.css:27`)
