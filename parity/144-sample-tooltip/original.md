# 144 sample-tooltip — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:239-245, design-sections.module.css:230-235 (.btnGhost)

## Содержание/структура
`TooltipDemo()` в Block «Tooltip»: одна кнопка `.btnGhost` «Hover me» с атрибутом `data-tooltip="This is a tooltip — hover for the full text. data-tooltip is set on the element, document-level listener does the rest."`

Механика: тултип объявляется атрибутом `data-tooltip` на элементе; document-level listener рисует сам тултип (компонент тултипа — зона Overlays).

## Метрики
`.btnGhost`: padding 4px 16px; border-radius 8px; font-size 12px; background transparent; color `--text-secondary`; border `1px solid transparent`; transition `background 150ms ease`.

## Состояния/варианты
- `.btnGhost:hover`: background `--bg-surface`; color `--text-primary`
- hover также вызывает показ тултипа через глобальный listener
