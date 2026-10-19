# Отклонения браузерной таблицы UA

`BROWSER_CSS` накладывает браузерные умолчания поверх таблицы чата.
Нормативные ориентиры — [HTML Rendering](https://html.spec.whatwg.org/multipage/rendering.html)
и [Blink html.css](https://chromium.googlesource.com/chromium/src/+/main/third_party/blink/renderer/core/html/resources/html.css).
Число пройденных WPT не определяет правила этой таблицы.

## Рамка iframe

Ожидаемое правило: `iframe { border: 2px inset }`
([HTML, embedded content](https://html.spec.whatwg.org/multipage/rendering.html#embedded-content-rendering-rules)).
В текущем `BROWSER_CSS` правило отсутствует. Это известное отклонение без
основания в спецификации, а не допустимое браузерное умолчание.

Восстановление требует совместной проверки рамки UA, авторской рамки и
презентационного атрибута `frameborder="0"`: последний должен убирать рамку,
а авторское CSS должно иметь приоритет над презентационным атрибутом.
Приёмочный пример — три iframe без авторской рамки, с `frameborder="0"` и с
обоими `frameborder="0"` и `style="border: 4px solid green"` соответственно;
ожидаются рамки 2px inset, нулевая и 4px solid green. Сравнивать нужно также
наружные размеры при заданных width/height и печатную фрагментацию.
Этот cleanup помечает отклонение и сохраняет текущие декларации CSS.
