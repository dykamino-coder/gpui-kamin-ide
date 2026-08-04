# 83 legacy-bridge-card — оригинал
Файлы: `src/renderer/components/settings/LegacyBridgeCard.tsx` (82-101), `src/renderer/components/settings/LegacyBridgeCard.module.css`

## JSX-структура (кратко, вложенность)
```
null при !fp?.found, иначе:
div.card
├─ div.icon — i.fas.fa-box-archive
├─ div.body
│  ├─ h2.title — «Legacy Electron Bridge detected»
│  └─ p.desc — «Found: {installed app | folder "Open with" menu entry | saved config}. KaminIDE has already imported …»
└─ button.remove (disabled при busy) — «Remove old Bridge» | «Removing…»
```
Клик Remove → ConfirmModal (danger) → reimportSessions → `uninstall_electron_bridge` → toast → re-detect (карточка исчезает).

## Метрики (ИЗ CSS, точные значения)
- `.card`: flex, `align-items:flex-start`; `gap:var(--space-3)`; `padding:var(--space-3)`; background `var(--bg-surface)`; border `1px solid var(--divider-soft)`; `border-radius:var(--radius-md)`
- `.icon`: `flex:none`; `display:grid; place-items:center`; 32×32px; `border-radius:var(--radius-sm)`; color `var(--accent-primary)`; `font-size:16px`
- `.body`: `flex:1; min-width:0`
- `.title`: `margin:0`; `font-size:13px`; `font-weight:600`; color `var(--text-primary)`
- `.desc`: margin `var(--space-1) 0 0`; `font-size:12px`; `line-height:1.5`; color `var(--text-muted)`
- `.remove`: `flex:none; align-self:center`; padding `var(--space-1) var(--space-3)`; border `1px solid var(--accent-red)`; `border-radius:var(--radius-sm)`; background transparent; color `var(--accent-red)`; `font-size:12px`; `font-weight:600`; `transition: background 0.12s ease, color 0.12s ease`
  - hover (не disabled): background `var(--accent-red)`; color `#fff`
  - disabled: `opacity:0.6; cursor:default`

## Состояния (классы-варианты с метриками)
- Не найден footprint → компонент возвращает `null`
- `busy` → кнопка disabled, текст «Removing…»
- hover `.remove` — инверсия (красная заливка, белый текст)
