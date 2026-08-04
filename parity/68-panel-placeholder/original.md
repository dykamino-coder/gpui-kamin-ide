# 68 panel-placeholder — оригинал
Файлы: kamin-ide/src/renderer/components/panel-placeholder/PanelPlaceholder.tsx (строки 31-42), kamin-ide/src/renderer/components/panel-placeholder/PanelPlaceholder.module.css

## JSX-структура (кратко, вложенность)
```
div.placeholder
├─ span.glyph [aria-hidden="true"]
│  └─ <PanelIcon slot={slot} />        (SVG из титлбарного семейства LayoutToggles)
├─ h2.label  {label}
├─ p.hint    {hint ?? "Open new tool or drag-n-drop tool from other panels"}
└─ <ActivityPicker slot popDirection="up" variant="openTool" />   (только если activitySlot задан)
```
Пилюля «Open Tool» = ActivityPicker с variant openTool; открывает тот же пикер, что «...» activity bar'а — выбор пинит и активирует активность.

## Метрики (ИЗ CSS, точные значения)
### .placeholder
- flex: 1; display: flex; flex-direction: column
- align-items: center; justify-content: center; text-align: center (мёртвый центр карточки)
- gap: var(--space-2)
- padding: var(--space-5) var(--space-5)
- color: var(--text-muted)

### .glyph
- color: var(--text-muted); margin-bottom: var(--space-1); font-size: 0
- `.glyph svg { width: 28px; height: 24px; }` (PanelIcon штатно 14×12 — тут увеличен ×2)

### .label
- margin: 0; font-size: var(--fs-lg); font-weight: 600; color: var(--text-primary)

### .hint
- margin: 0; font-size: var(--fs-sm); color: var(--text-muted); line-height: var(--lh-snug)

### .trigger (пилюля «Open Tool»)
- display: inline-flex; align-items: center; gap: var(--space-2)
- padding: var(--space-1) var(--space-3)
- background: `color-mix(in srgb, var(--accent-primary) 16%, transparent)`
- color: var(--text-primary); border: none
- border-radius: var(--radius-sm)
- font-size: var(--fs-sm); margin-top: var(--space-1)
- transition: background var(--transition-fast)
- `.trigger > i { font-size: 10px; }`

## Состояния (классы-варианты с метриками)
- `.trigger:hover`: background `color-mix(in srgb, var(--accent-primary) 26%, transparent)`
- без activitySlot пикер (пилюля) не рендерится вовсе
