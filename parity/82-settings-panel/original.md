# 82 settings-panel — оригинал
Файлы: `src/renderer/components/settings/SettingsPanel.tsx` (28-74), `src/renderer/components/settings/SettingsPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.root
├─ <LegacyBridgeCard />   (элемент 83, условный)
├─ section.section — «Notifications»
│  ├─ h3.sectionTitle
│  └─ label.row [for=pref-background-toasts]
│     ├─ input[type=checkbox] (disabled пока !loaded)
│     └─ span.rowText — «Show background notifications when KaminIDE is not focused»
│        └─ span.rowDesc — «Raises a native, always-on-top toast when …»
└─ section.section — «Terminal»
   └─ label.row [for=pref-use-conpty]
      ├─ input[type=checkbox]
      └─ span.rowText — «Use the system ConPTY DLL (Windows-signed)»
         └─ span.rowDesc — «Off (default) uses node-pty's bundled ConPTY — …»
```

## Метрики (ИЗ CSS, точные значения)
- `.root`: flex column; `gap:var(--space-4)`
- `.section`: flex column; `gap:var(--space-2)`
- `.sectionTitle`: `margin:0`; `font-size:11px`; `font-weight:600`; `letter-spacing:0.06em`; `text-transform:uppercase`; color `var(--text-muted)`
- `.row`: flex, `align-items:flex-start`; `gap:10px`; padding `4px 0`; `cursor:pointer`; `font-size:13px`; color `var(--text-primary)`
- `.row input`: `margin-top:2px`
- `.rowText`: `display:block`
- `.rowDesc`: `display:block`; `margin-top:2px`; `font-size:11px`; `line-height:1.5`; color `var(--text-muted)`
- `.placeholder` (в css, в текущем JSX не используется): flex column, центр; `gap:var(--space-2)`; padding `var(--space-6) 0`; color `var(--text-muted)`; `.placeholder i` — `font-size:32px; opacity:0.5`
- hover/active/focus — нет; transition — нет

## Состояния (классы-варианты с метриками)
- Чекбоксы `disabled` пока prefs не загрузились (`!loaded`)
- `LegacyBridgeCard` рендерится только при найденном legacy-Bridge
