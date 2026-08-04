# 120 status-version-update — оригинал
Файлы: `kamin-ide/src/renderer/components/status-bar/StatusBar.tsx` (90-145), `StatusBar.module.css` (23-76)

## JSX-структура (кратко, вложенность)
Три взаимоисключающих состояния (downloading > update-available > idle):
```
1) downloading:
div.item.brand.update.downloading role=progressbar aria-valuemin=0 aria-valuemax=100 [aria-valuenow={pct}]
  data-tooltip="Downloading the KaminIDE update…"
├─ span.progressFill style="width: {pct}%|100%; opacity: 1|0.5"
└─ span.progressLabel
   ├─ span.codicon.codicon-cloud-download
   └─ span "Updating {pct}%" | "Updating {N.n} MB"      (indeterminate: без Content-Length)

2) update available:
button.item.clickable.brand.update data-tooltip="Update to KaminIDE {v} — you have {cur}" onClick=installUpdate
├─ span.codicon.codicon-cloud-download
└─ span "Update {version}"

3) idle:
button.item.clickable.brand data-tooltip="Check for updates" onClick=checkForUpdate
└─ span "KaminIDE {version|0.0.1}"
```
- Indeterminate fill: opacity `INDETERMINATE_FILL_OPACITY = 0.5`, width 100%.

## Метрики (ИЗ CSS, точные значения)
База `.item`: display: flex; align-items: center; gap: 4px; padding: 0 var(--space-2); border-radius: var(--radius-xs); font-size: var(--fs-xs); `.codicon` 12px !important
`.clickable`: cursor: pointer
`.brand`: color: var(--accent-primary); font-weight: 500

`.update`:
- background: color-mix(in srgb, var(--accent-primary) 22%, transparent)
- color: var(--accent-primary); font-weight: 600

`.downloading`: position: relative; overflow: hidden

`.progressFill`:
- position: absolute; left: 0; top: 0; bottom: 0
- background: color-mix(in srgb, var(--accent-primary) 32%, transparent)
- transition: width 120ms linear
- pointer-events: none

`.progressLabel`: position: relative; display: inline-flex; align-items: center; gap: 6px

## Состояния (классы-варианты с метриками)
- `.item:hover` (idle): background: color-mix(in srgb, var(--bg-surface) 60%, transparent); color: var(--text-primary)
- `.update:hover`: background: color-mix(in srgb, var(--accent-primary) 34%, transparent); color: var(--accent-primary)
- downloading: не кнопка (div), прогресс-заливка позади лейбла; ширина трекает байты (120ms linear).
