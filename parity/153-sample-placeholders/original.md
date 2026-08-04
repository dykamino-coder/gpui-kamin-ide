# 153 sample-placeholders — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:213-221, components/panel-placeholder/ActivityPlaceholder.tsx, ActivityPlaceholder.module.css

## Содержание/структура
`PlaceholdersRow()` в Block «Empty / active panel placeholders», hint: «ActivityPlaceholder is shown once a tool is picked but its renderer isn't ready yet (Phase A).»
Карточка-обёртка (inline): `width:100%; max-width:280px; min-height:160px; border-radius:var(--radius-md); background:var(--bg-mantle); display:flex; flex-direction:column` → внутри `<ActivityPlaceholder icon="terminal" label="Terminal" />`.

ActivityPlaceholder — empty-state АКТИВНОЙ активности без готового рендерера (отличен от PanelPlaceholder — empty-state «активность не выбрана», с Open Tool picker; здесь пикер намеренно опущен):
- `.placeholder` → `<ToolIcon icon size={36} class=glyph>` + `<h2 class=label>{label}</h2>` + `<p class=hint>Nothing to show here yet.</p>`

## Метрики
- `.placeholder`: flex 1; flex column; align-items center; justify-content center; text-align center; gap `--space-2` (8px); padding `--space-5` (20px); color `--text-muted`
- `.glyph`: font-size 36px (GLYPH_SIZE_PX = 36); color `--text-disabled` (#60667b); margin-bottom `--space-1` (4px)
- `.label`: margin 0; font-size `--fs-md` (13px); font-weight 600; color `--text-primary`
- `.hint`: margin 0; font-size `--fs-xs` (11px); color `--text-muted`; line-height `--lh-snug` (1.3); max-width 240px
- карточка-обёртка превью: max-width 280px; min-height 160px; radius 12px; bg `--bg-mantle` (#262533)

## Состояния/варианты
Статичный. Props: icon (строка для ToolIcon), label. Текст hint фиксированный: «Nothing to show here yet.»
