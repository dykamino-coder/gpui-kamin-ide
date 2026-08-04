# 158 dragging-body-classes — оригинал
Файлы: kamin-ide/src/renderer/theme/global.css:69-86 (классы вешает useDragHandler)

## Содержание/структура
Глобальные body-классы на время драга:
- `body.kamin-dragging` — драг сплиттера панелей: iframe'ы теряют pointer-events (иначе iframe-документ глотает mousemove и resize замирает); интерактивные элементы тоже теряют pointer-events, чтобы hover/tooltip не реагировали и elementFromPoint проваливался к tagged zone-контейнерам
- `body.kamin-tool-dragging` — драг тула между зонами: курсор grabbing на всём (scoped отдельным классом, чтобы не перебить col/row-resize курсор сплиттера)

## Метрики
Полные правила:
```css
body.kamin-dragging iframe { pointer-events: none; }

body.kamin-dragging :where(button, [role="button"], [role="tab"], a, [data-tooltip]) {
  pointer-events: none;
}

body.kamin-tool-dragging,
body.kamin-tool-dragging * { cursor: grabbing !important; }
```

## Состояния/варианты
Два независимых режима: kamin-dragging (сплиттер) и kamin-tool-dragging (перенос тула). Вне драга классы отсутствуют — правила неактивны.

## Дополнение атрибутов (цикл 10)

- цвета: оба правила задают только `pointer-events: none` и `cursor: grabbing`; ни одного цветового значения или токена в блоке нет (global.css:72, 79-81, 85-86). N/A: цвета
- отступы: N/A: отступы — padding/margin правила не трогают (global.css:72-86); глобальный сброс `* { margin: 0; padding: 0 }` (global.css:12)
