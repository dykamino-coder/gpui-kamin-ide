# 78 customize-content-panel — оригинал
Файлы: `src/renderer/components/main/CustomizePanel.tsx` (31-48 — панель, 81-88 — `ComingSoon`), `src/renderer/components/main/CustomizePanel.module.css`

## JSX-структура (кратко, вложенность)
```
section.panel
├─ header.header
│  ├─ h1.title — contributed?.name | titleFor(panel): Extensions/Logs/System/Design/Settings
│  └─ p.subtitle — «Contributed by an extension.» | subtitleFor(panel)
└─ div.{bodyFlush|body}   (contributed → bodyFlush)
   └─ contributed → <ContributedViewBody viewId flush />
      | "extensions" → <ExtensionsPanel /> | "logs" → <LogsPanel /> | "system" → <SystemLogPanel />
      | "design" → <DesignPanel /> | "settings" → <SettingsPanel />
      | иначе → <ComingSoon> = div.placeholder (i.fas.fa-screwdriver-wrench + span «Phase B»)
```

## Метрики (ИЗ CSS, точные значения)
- `.panel`: `flex:1`; flex column; `overflow:hidden`
- `.header`: padding `var(--space-5) var(--space-6) var(--space-3)` (top right/left bottom); `border-bottom:1px solid color-mix(in srgb, var(--bg-overlay) 30%, transparent)`
- `.title`: `margin:0`; font-size `var(--fs-xl)`; `font-weight:600`; color `var(--text-primary)`
- `.subtitle`: margin `var(--space-1) 0 0`; font-size `var(--fs-md)`; color `var(--text-muted)`
- `.body`: `flex:1; overflow-y:auto`; padding `var(--space-4) var(--space-6)`
- `.bodyFlush`: `flex:1; display:flex; flex-direction:column; min-height:0; overflow:hidden` — БЕЗ padding (webview edge-to-edge, без card-in-a-card)
- `.placeholder`: flex column, центрирование; `gap:var(--space-2)`; `padding:var(--space-7)`; color `var(--text-muted)`
- `.placeholder i`: `font-size:32px; opacity:0.5`
- hover/active/focus — нет; transition — нет

## Состояния (классы-варианты с метриками)
- `.body` (встроенные страницы, с padding) ↔ `.bodyFlush` (contributed webview-страница, flush)
- `ComingSoon` — фоллбек для неизвестной страницы
