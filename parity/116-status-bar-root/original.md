# 116 status-bar-root — оригинал
Файлы: `kamin-ide/src/renderer/components/status-bar/StatusBar.tsx` (29-48), `StatusBar.module.css` (1-21)

## JSX-структура (кратко, вложенность)
```
footer.statusBar
├─ div.left
│  ├─ Item "N active"  (icon circle-filled, tone ok)
│  ├─ (failed > 0)   Item "N failed" (icon warning, tone warn)
│  ├─ (disabled > 0) Item "N off"    (icon circle-slash)
│  ├─ Item "N cmds"  (icon symbol-keyword)
│  └─ ContributedItem × N (alignment Left=1, sort priority desc)
└─ div.right
   ├─ ContributedItem × N (alignment Right, sort priority asc)
   ├─ <EditorEncodingItems /> (№119)
   └─ <VersionUpdateItem />   (№120)
```

## Метрики (ИЗ CSS, точные значения)
`.statusBar`:
- height: var(--layout-status-bar-height)
- background: transparent (без бордера; градиент appWrapper просвечивает)
- display: flex; align-items: stretch
- font-size: var(--fs-xs); color: var(--text-muted)
- padding: 0 var(--space-2); gap: var(--space-1)

`.left`, `.right`:
- display: flex; align-items: stretch
- gap: 2px (умышленно плотнее space-1)

`.right`: margin-left: auto

## Состояния (классы-варианты с метриками)
Контейнер статичен; состояния — у item'ов (№117-120).
