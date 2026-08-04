# 06 titlebar-window-controls-cluster — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:346-406 (кластер),
control_button 31-68, DevTools 353-377

## Структура (gpui-дерево кратко)
```
div (flex items-center h_full, pr SPACE_1)
 ├ div#devtools (fa-bug 13px + label "DevTools", radius 12)
 ├ control_button win-min  (CHROME_MINIMIZE \u{eaba})
 ├ control_button win-max  (CHROME_MAXIMIZE \u{eab9} ↔ CHROME_RESTORE \u{eabb}
 │                          по window.is_maximized(); tooltip Maximize↔Restore)
 └ control_button win-close (CHROME_CLOSE \u{eab8}, danger)
```
Кнопки: window_control_area(Min/Max/Close) + on_mouse_down →
minimize_window()/zoom_window()/remove_window().

## Метрики (из кода, точные)
- контейнер: h_full, pr m::SPACE_1 (4), gap нет (кнопки несут mx SPACE_1)
- кнопки — элемент 07
- цвета контейнера: не заданы

## Отличия от original.md той же папки
Структурно и метрически 1:1 (h100%, padding-right 4, порядок
DevTools→min→max→close, смена иконки maximize↔restore). Расхождения
внутри самих кнопок — см. 07-titlebar-button/ours.md.

## Дополнение атрибутов (цикл 10)

- ховер: у контейнера кластера ховера нет (`crates/shell/src/ui/titlebar.rs:419-423`); у детей: control_button bg bg_surface #3d3f51 + fg text_primary #cfd4e2 (`titlebar.rs:43,59`), close (danger) bg accent_red #f38ba8 + fg bg_primary #313240 (`titlebar.rs:41,59`), DevTools bg bg_surface #3d3f51 + fg accent_primary #89b4fa (`titlebar.rs:437`)
