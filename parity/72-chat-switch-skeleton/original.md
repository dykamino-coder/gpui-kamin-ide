# 72 chat-switch-skeleton — оригинал
Файлы: `src/renderer/components/panel-placeholder/ChatSwitchSkeleton.tsx` (10-21), `src/renderer/components/panel-placeholder/ChatSwitchSkeleton.module.css`

## JSX-структура (кратко, вложенность)
```
div.wrap [role=status, aria-label="Loading conversation…"]
├─ div.brand
│  ├─ span.glow (aria-hidden)
│  └─ img.logo (kaminoid.svg, draggable=false, aria-hidden)
├─ span.caption — «Loading conversation…»
└─ span.bar (aria-hidden)
   └─ span.barFill
```

## Метрики (ИЗ CSS, точные значения)
- `.wrap`: `position:absolute; inset:0`; flex column, `align-items:center; justify-content:center`; `gap:18px`; `padding:24px`; `overflow:hidden`; background `var(--editor-bg, var(--bg-base, #1e1e28))`
- `.brand`: `position:relative`; `display:grid; place-items:center`; 96×96px
- `.glow`: `position:absolute`; 150×150px; `border-radius:50%`; background `radial-gradient(circle, color-mix(in srgb, var(--accent-primary, #89b4fa) 28%, transparent) 0%, transparent 66%)`; `filter:blur(8px)`
- `.logo`: `position:relative; z-index:1`; 64×64px; `user-select:none; -webkit-user-drag:none`; `filter:drop-shadow(0 6px 18px rgba(0,0,0,0.35))`
- `.caption`: font-size `var(--fs-sm, 12px)`; `letter-spacing:0.01em`; color `var(--text-muted, #9399b2)`
- `.bar`: `position:relative`; 180×3px; `border-radius:999px`; `overflow:hidden`; background `color-mix(in srgb, var(--text-primary, #cdd6f4) 8%, transparent)`
- `.barFill`: `position:absolute; inset:0`; `border-radius:inherit`; background `linear-gradient(90deg, transparent, var(--accent-primary, #89b4fa), transparent)`; стартовый `transform:translateX(-100%)`
- Анимации:
  - `.glow` — `kaminSwitchBreathe 2.4s ease-in-out infinite`: 0%/100% `opacity:0.5; scale(0.94)` → 50% `opacity:1; scale(1.06)`
  - `.logo` — `kaminSwitchFloat 2.4s ease-in-out infinite`: 0%/100% `translateY(0)` → 50% `translateY(-4px)`
  - `.barFill` — `kaminSwitchSweep 1.15s ease-in-out infinite`: до `translateX(100%)`
- `@media (prefers-reduced-motion: reduce)`: все три `animation:none`; `.barFill` — `transform:none; opacity:0.6`

## Состояния (классы-варианты с метриками)
- Вариантных классов нет; монтируется только пока `covering` (см. 76-persistent-webview-layer), поверх чат-iframe при переключении сессии.
