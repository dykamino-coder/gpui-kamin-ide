# 152 sample-panel-icon-family — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples-extra.tsx:197-211, components/titlebar/PanelIcon.tsx, design-sections.module.css:418-425 (.codeInline)

## Содержание/структура
`PanelIconFamilyRow()` в Block «Panel icon family», hint: «Same SVG family used by LayoutToggles + PanelPlaceholder — frame + highlighted slot.»
8 слотов: left, main, main-bottom, center, center-bottom, right, right-top, right-bottom. Каждый:
`<span data-tooltip={slot} style="display:inline-flex;flex-direction:column;align-items:center;gap:4px;color:var(--text-secondary)">` → `<PanelIcon slot>` + `<code class=codeInline style="font-size:10px">{slot}</code>`

## Метрики
PanelIcon (SVG 14×12, viewBox 0 0 14 12, aria-hidden):
- рамка: rect x=1 y=1 w=12 h=10, rx/ry 1.5, fill none, stroke currentColor, stroke-width 1.2 (STROKE_INSET 1)
- highlight: fill currentColor, opacity 0.85, rx/ry 1 (SLOT_RADIUS), SLOT_INSET 1.5
- константы: LEFT_HIGHLIGHT_W = RIGHT_HIGHLIGHT_W = CENTER_HIGHLIGHT_W = 4.5; BOTTOM_HIGHLIGHT_INSET_Y = 5; RIGHT_HIGHLIGHT_INSET = 6 → RIGHT_HIGHLIGHT_X = 8; RIGHT_QUARTER_HEIGHT = (12 − 3)/2 = 4.5; RIGHT_QUARTER_BOTTOM_Y = 6
- слоты:
  - main / left: rect x=1.5 y=1.5 w=4.5 h=9 (левая колонка; main = зеркало right)
  - right: x=8 y=1.5 w=4.5 h=9
  - right-top: x=8 y=1.5 w=4.5 h=4.5
  - right-bottom: x=8 y=6 w=4.5 h=4.5
  - center: x=(14−4.5)/2=4.75 y=1.5 w=4.5 h=9
  - center-bottom: x=4.75 y=7 w=4.5 h=3.5
  - main-bottom: x=1.5 y=6 w=4.5 h=4.5
  - default (bottom, legacy): x=1.5 y=7 w=11 h=3.5
- порядок отрисовки: сначала highlight, поверх frame

Подпись: `.codeInline` (mono, color `--accent-primary`, bg `color-mix(accent-primary 10%)`, padding 1px 6px, radius 4px), inline override font-size 10px. Обёртка: gap 4px, color `--text-secondary` (SVG красится currentColor).

## Состояния/варианты
Тип PanelSlot имеет 9 значений (8 показанных + legacy alias "bottom" — full-width полоса, ветка default). data-tooltip на каждой обёртке показывает имя слота при hover.
