# 79 design-panel-shell — оригинал
Файлы: `src/renderer/components/main/DesignPanel.tsx` (18-41 — панель, 43-55 — `Section`), `src/renderer/components/main/DesignPanel.module.css`

## JSX-структура (кратко, вложенность)
```
div.root
└─ 6 × section.section (Colors / Typography / Spacing / Radius / Shadows / Components)
   ├─ header.sectionHeader
   │  ├─ h2.sectionTitle — title
   │  └─ p.sectionSubtitle — subtitle
   └─ div.sectionBody — {ColorTokens|TypographyTokens|SpacingTokens|RadiusTokens|ShadowTokens|ComponentSamples}
```
Тексты сабтайтлов: «Theme tokens — resolve from the active dark/light palette.», «Font families + the 5-step size scale.», «space-1..7 — every gap/padding in the codebase resolves to one of these.», «4-step concentric scale anchored at 16px outer.», «Elevation tokens. Lower index = more grounded.», «Live samples — values track the palette above.»

## Метрики (ИЗ CSS, точные значения)
- `.root`: flex column; `gap:var(--space-6)`; `padding-bottom:var(--space-6)`
- `.section`: flex column; `gap:var(--space-3)`
- `.sectionHeader`: flex column; `gap:2px`
- `.sectionTitle`: `margin:0`; font-size `var(--fs-lg)`; `font-weight:600`; color `var(--text-primary)`
- `.sectionSubtitle`: `margin:0`; font-size `var(--fs-sm)`; color `var(--text-muted)`; line-height `var(--lh-snug)`
- `.sectionBody`: border `1px solid color-mix(in srgb, var(--bg-surface) 60%, transparent)`; `border-radius:var(--radius-md)`; background `var(--bg-mantle)`; `padding:var(--space-4)`
- hover/active/focus — нет; transition — нет; позиционирование — поток

## Состояния (классы-варианты с метриками)
- Вариантов нет; read-only контейнер, значения токенов резолвятся из активной темы в рендер-тайме.
