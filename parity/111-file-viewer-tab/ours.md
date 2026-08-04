# 111 file-viewer-tab — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\editor_tabs.rs:85-209 (таб), 315-440 (RMB-меню editor_tab_menu)

## Структура (gpui-дерево кратко)
```
div#ftab-{i}: flex.items_center.gap(6).h(24).pl(8).pr(4).rounded(8)
├─ file_img (Catppuccin-иконка 14×14)
├─ (pinned) fa "\u{f08d}" (thumbtack) 11px text_muted
├─ name (basename)
└─ dirty ? div 14×14 c кругом 6×6 accent_orange
        : div#ftabx-{i} 16×16 rounded(3) codicon close "\u{ea76}" 11px
tooltip = полный путь; middle-click → CloseEditorTab; RMB → editor_tab_menu
```
RMB-меню (в overlay, w 220): Pin/Unpin Tab, Close, Close Others, Close to the Right, Close All, разделитель, «File actions…» (переход в меню дерева по path).

## Метрики (из кода, точные)
- Таб: h 24, gap 6, pl 8 (SPACE_2), pr 4, rounded 8 (RADIUS_SM), fs 11, weight 500 (MEDIUM), цвет p.text_secondary #adb3c7
- Hover: bg p.bg_surface #3d3f51 a=.5 + text p.text_primary #cfd4e2
- Активный: bg p.accent_primary #89b4fa a=.16 + text p.text_primary (hover не меняет)
- Иконка файла: 14×14; pin: FA f08d 11px p.text_muted #838aa0
- Dirty: круг 6×6 p.accent_orange #fab387 в боксе 14×14
- Close: 16×16, rounded 3, цвет p.text_muted; hover bg p.text_primary a=.12 + text_primary; глиф 11px; tooltip «Close»
- Меню таба: w 220, rounded 12, bg p.bg_surface, border p.text_primary a=.06, p 4; item px 12 py 4 rounded 8 fs 12, hover text_primary a=.08

## Отличия от original.md той же папки
1. Dirty — нарисованный круг 6px accent_orange вместо текстового «●» 10px (визуально близко).
2. Close всегда видим (в оригинале opacity 0 → 0.7 на hover таба → 1 на hover кнопки); hover-фон close: text_primary 12% vs bg-overlay 60%; rounded 3 vs radius-xs 4.
3. Pin-иконка — FontAwesome thumbtack (f08d) text_muted вместо `codicon-pinned` c opacity .7.
4. Padding: pl 8 / pr 4 vs оригинал 4px 6px 4px 10px (слева 10, справа 6); letter-spacing 0.02em отсутствует; transition отсутствуют.
5. RMB-меню содержит Pin/Unpin и «File actions…» (в оригинале pin в самом меню файла; сравнить состав), рендерится в overlay-окне.
6. Меток webview-панелей нет (webview — не таб), TabIcon-ветки для расширений нет.
7. Select — на mouse-up без сдвига (порог 4px в root) — совпадает с оригиналом; middle-click close — совпадает.
