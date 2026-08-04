# 128 toasts-stack — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/Toasts.tsx` (21-63), `Toasts.module.css`

## JSX-структура (кратко, вложенность)
```
div.stack role=region aria-label="Notifications"        (null при 0 тостах)
└─ div.toast .{info|success|warning|error} [.leaving] role={error → alert, иначе status} × N
   ├─ span.codicon.codicon-{info|pass|warning|error}.icon
   ├─ div.content
   │  ├─ (title) div.title
   │  ├─ div.message
   │  └─ (actions) div.actions
   │     └─ button.actionBtn {label} × N   (клик резолвит промис pushToast)
   └─ button.dismiss aria-label="Dismiss notification"
      └─ span.codicon.codicon-close
```
- Иконки: info→info, success→pass, warning→warning, error→error.

## Метрики (ИЗ CSS, точные значения)
`.stack`:
- position: fixed; bottom: 36px; right: var(--space-4)
- display: flex; flex-direction: column; gap: var(--space-2)
- z-index: var(--z-toast); pointer-events: none; max-width: 360px

`.toast`:
- display: flex; align-items: flex-start; gap: var(--space-3)
- padding: var(--space-3) var(--space-4)
- border: 1px solid color-mix(in srgb, var(--bg-surface) 70%, transparent)
- border-radius: var(--radius-md)
- background: color-mix(in srgb, var(--bg-surface) 50%, transparent)
- backdrop-filter: blur(8px)
- box-shadow: var(--shadow-card-popup)
- font-size: var(--fs-sm); color: var(--text-primary)
- pointer-events: auto
- animation: slide 0.18s ease-out (from translateX(8px)/opacity 0 → to translateX(0)/opacity 1)

`.icon`: flex-shrink: 0; margin-top: 2px; font-size: var(--fs-md)
`.content`: flex: 1; min-width: 0
`.title`: font-weight: 600; margin-bottom: 2px
`.message`: color: var(--text-secondary); word-break: break-word

`.actions`: display: flex; gap: var(--space-2); margin-top: var(--space-2); flex-wrap: wrap

`.actionBtn`:
- padding: 2px var(--space-3); border-radius: var(--radius-xs)
- border: 1px solid color-mix(in srgb, var(--accent-primary) 40%, transparent)
- background: transparent; color: var(--accent-primary)
- font-size: var(--fs-xs); cursor: pointer; font-family: inherit
- transition: background var(--transition-fast)

`.dismiss`:
- flex-shrink: 0; padding: 0; width: 16px; height: 16px
- display: grid; place-items: center
- background: none; border: none; color: var(--text-disabled); cursor: pointer; font-size: var(--fs-xs)

## Состояния (классы-варианты с метриками)
- `.toast.leaving`: animation: slideOut 0.18s ease-in forwards (to translateX(12px)/opacity 0); pointer-events: none. Длительность = TOAST_EXIT_MS в state.ts.
- `.actionBtn:hover`: background: color-mix(in srgb, var(--accent-primary) 14%, transparent)
- `.dismiss:hover`: color: var(--text-primary)
- Severity — ТОЛЬКО цвет иконки (без рейла/тинта): `.info .icon` var(--accent-blue); `.success .icon` var(--accent-green); `.warning .icon` var(--accent-yellow); `.error .icon` var(--accent-red)
