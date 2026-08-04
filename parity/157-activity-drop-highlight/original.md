# 157 activity-drop-highlight — оригинал
Файлы: kamin-ide/src/renderer/theme/global.css:45-67

## Содержание/структура
Глобальные utility-атрибуты для карточки-приёмника при drag pinned-активности. Любой card-host (sidebar, file-panel, right-panel cards, main-bottom, main) вешает `data-activity-drop` вместо форка рецепта в семи module.css. Визуал «over» совпадает со strip-уровневым placeholder внутри (accent-tint + dashed), чтобы карточка и insertion-gap читались одним drop-превью.

## Метрики
Полные правила:
```css
[data-activity-drop="over"] {
  background-color: color-mix(in srgb, var(--accent-primary) 10%, transparent);
  outline: 1px dashed color-mix(in srgb, var(--accent-primary) 60%, transparent);
  outline-offset: -2px;
  transition: background-color var(--transition-fast), outline-color var(--transition-fast);
}

[data-activity-drop="blocked"] {
  background-color: color-mix(in srgb, var(--accent-red) 12%, transparent);
  box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--accent-red) 60%, transparent);
  transition: background-color var(--transition-fast), box-shadow var(--transition-fast);
}
```
`--transition-fast` = 150ms ease. Базовые цвета (dark): `--accent-primary` #89b4fa, `--accent-red` #f38ba8.

## Состояния/варианты
- `over` — карточка примет drop: accent-tint 10% + dashed outline 1px (accent 60%), offset −2px
- `blocked` — drop будет no-op (назначение уже содержит активность): красный tint 12% + inset-«рамка» box-shadow 2px (red 60%); outline намеренно отсутствует

## Дополнение атрибутов (цикл 10)

- отступы: собственных padding/margin правила не задают; единственный инсет — `outline-offset: -2px` у `[data-activity-drop="over"]` (global.css:56); у `blocked` вместо outline рамка внутрь через `box-shadow: inset 0 0 0 2px` (global.css:65)
