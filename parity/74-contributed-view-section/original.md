# 74 contributed-view-section — оригинал
Файлы: `src/renderer/components/activity-bodies/ContributedContainerBody.tsx` (62-77 — `ViewSection`), `src/renderer/components/activity-bodies/ContributedContainerBody.module.css`

## JSX-структура (кратко, вложенность)
```
section.view
├─ header.title
│  ├─ span — meta?.title ?? name (createTreeView title переопределяет contributed name)
│  ├─ span.viewDescription — meta.description (опционально)
│  └─ span.viewBadge [data-tooltip=badge.tooltip] — badge.value (опционально)
└─ type==="webview" ? <WebviewViewBody viewId /> : <TreeViewBody viewId />
```
Chat view без сессии → вместо всей секции `<WelcomePlaceholder />` (без хедера).

## Метрики (ИЗ CSS, точные значения)
- `.view`: `display:flex; flex-direction:column; flex:1; min-height:0` (виды стекаются; одиночный webview заполняет тело)
- `.title` (хедер): flex, `align-items:center`; padding `var(--space-1) var(--space-3)`; font-size `var(--fs-xs)`; `text-transform:uppercase`; `letter-spacing:0.04em`; color `var(--text-muted)`; `flex-shrink:0`
- `.viewDescription`: `margin-left:var(--space-2)`; `font-weight:400`; `opacity:0.55`
- `.viewBadge`: `margin-left:auto`; `min-width:18px`; padding `0 5px`; `border-radius:9px` (половина min-height — пилюля); background `var(--accent-primary)`; color `var(--bg-base, #fff)`; `font-size:0.75em`; `line-height:16px`; `text-align:center`
- hover/active/focus — нет; transition — нет

## Состояния (классы-варианты с метриками)
- description и badge — условные (только при `treeMeta` от `createTreeView`)
- badge несёт tooltip через `data-tooltip`
