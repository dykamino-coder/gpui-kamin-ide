# 77 welcome-placeholder — оригинал
Файлы: `src/renderer/components/main/WelcomePlaceholder.tsx` (10-37), `src/renderer/components/main/WelcomePlaceholder.module.css`

## JSX-структура (кратко, вложенность)
```
section.welcome [aria-label="Welcome to KaminIDE"]
├─ div.logoWrap (::before — радиальный glow)
│  └─ img.logo (kaminoid.svg, draggable=false)
├─ h1.title — «KaminIDE»
├─ span.version — «v{appVersion}» (условно)
├─ p.tagline — «An AI-native workspace — …»
├─ div.actions
│  ├─ button.primary — fas fa-folder-open + «New session in folder…»
│  └─ button.secondary — fas fa-plus + «Empty session»
└─ div.features
   ├─ span.feature — fas fa-comments + «Claude chat + tools»
   ├─ span.feature — fas fa-folder-tree + «Your files & editor»
   └─ span.feature — fas fa-terminal + «Integrated terminal»
```

## Метрики (ИЗ CSS, точные значения)
- `.welcome`: `flex:1; min-height:0`; flex column, центрирование обеих осей; `text-align:center`; `gap:var(--space-4)`; `padding:var(--space-6)`; `overflow:auto`
- `.logoWrap`: `position:relative; display:grid; place-items:center; margin-bottom:var(--space-1)`
- `.logoWrap::before`: `position:absolute`; 220×220px; `border-radius:50%`; background `radial-gradient(circle, color-mix(in srgb, var(--accent-primary) 26%, transparent) 0%, transparent 68%)`; `filter:blur(6px)`; `z-index:0`
- `.logo`: `position:relative; z-index:1`; 112×112px; `user-select:none; -webkit-user-drag:none`; `filter:drop-shadow(0 6px 18px rgba(0,0,0,0.35))`
- `.title`: `margin:0`; font-family `var(--font-display, inherit)`; `font-size:2.4rem`; `font-weight:700`; `letter-spacing:-0.02em`; `line-height:1.05`; color `var(--text-primary)`; `z-index:1`
- `.version`: `inline-block`; padding `2px 10px`; `border-radius:var(--radius-pill, 999px)`; background `color-mix(in srgb, var(--accent-primary) 14%, transparent)`; color `var(--text-primary)`; font-size `var(--fs-xs)`; `font-variant-numeric:tabular-nums`
- `.tagline`: `margin:0; max-width:30rem`; font-size `var(--fs-md)`; line-height `var(--lh-snug)`; color `var(--text-muted)`
- `.actions`: flex, `flex-wrap:wrap`; `gap:var(--space-3)`; `justify-content:center`; `margin-top:var(--space-2)`
- `.primary`/`.secondary` (общее): `inline-flex; align-items:center`; `gap:var(--space-2)`; padding `var(--space-2) var(--space-4)`; `border-radius:var(--radius-sm)`; font-size `var(--fs-sm)`; `font-weight:600`; cursor pointer; `transition: background var(--transition-fast), transform var(--transition-fast)`
- `.primary`: background `var(--accent-primary)`; color `var(--accent-on-primary, #fff)`; border none
  - hover: background `color-mix(in srgb, var(--accent-primary) 86%, #000)`; `transform:translateY(-1px)`
- `.secondary`: background `color-mix(in srgb, var(--text-primary) 6%, transparent)`; color `var(--text-primary)`; border `1px solid var(--divider-soft, color-mix(in srgb, var(--text-primary) 14%, transparent))`
  - hover: background `color-mix(in srgb, var(--text-primary) 12%, transparent)`; `transform:translateY(-1px)`
- `.features`: flex, wrap; gap `var(--space-2) var(--space-5)` (row col); `justify-content:center`; `margin-top:var(--space-3)`; `max-width:34rem`
- `.feature`: `inline-flex; align-items:center; gap:var(--space-2)`; font-size `var(--fs-sm)`; color `var(--text-muted)`
- `.feature > i`: color `var(--accent-primary)`; `font-size:13px`

## Состояния (классы-варианты с метриками)
- `.version` — только при `appVersion.value`
- hover primary/secondary — см. выше (затемнение/подсветка + подъём на 1px)
