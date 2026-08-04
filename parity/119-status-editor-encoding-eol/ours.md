# 119 status-editor-encoding-eol — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\status_bar.rs:273-276 (when_some(eol)), crates\shell\src\root.rs:103 (EditorTab.eol), 3617 (детект), 5374-5377 (передача)

## Структура (gpui-дерево кратко)
```
when_some(eol):
├─ item(None, «UTF-8», text_muted, tooltip «File encoding»)
└─ item(None, eol («LF»|«CRLF»), text_muted, tooltip «End of line»)
```
`eol` = Some только когда есть активный editor tab; определяется ОДИН раз при открытии файла: `text.contains("\r\n") → "CRLF" else "LF"` (root.rs:3617).

## Метрики (из кода, точные)
Как №117 без глифа: gap 4, px 8, rounded 4 (RADIUS_XS), fs 11, p.text_muted #838aa0; hover bg p.bg_surface a=.6 + p.text_primary.

## Отличия от original.md той же папки
1. EOL статичен с момента открытия файла — не реактивен к смене EOL в буфере (оригинал следит за activeEditorEol Monaco-модели).
2. Тултипы «File encoding» / «End of line» vs «Encoding» / «End of line» (первый отличается).
3. Условие показа — непустые editor_tabs (активный таб); режим web скрывает редактор, но табы остаются — возможен показ без видимого редактора (оригинал: только при активном текстовом редакторе).
4. Метрики item — совпадают.

## Дополнение атрибутов (цикл 10)

- шрифты: font-size 11 (FS_XS) (status_bar.rs:154), font-weight 400; глифа у «UTF-8»/«LF»/«CRLF» нет (item вызван с `None`, status_bar.rs:272-273)
