# 57 right-panel-width-handle — наша реализация
Файлы: crates/shell/src/ui/splitter.rs:22-86 (v_handle, v_bar); crates/shell/src/root.rs:4740-4758 (file_right_handle), 2959-2975 (DragKind::FileRight), 2998-3008 (персист)

## Структура (gpui-дерево кратко)
```
v_handle("file-right-handle") — сиблинг между file_wrap и right_wrap в body
= div .relative .w(0) .h_full .flex_shrink_0
  └─ div .absolute .left(-4) .top_0 .w(SPACE_2=8) .h_full
       .flex .items_center .justify_center .cursor_col_resize
       .tooltip("Drag to resize") .on_mouse_down(Left) .on_hover
     └─ when(show): v_bar(3px)  — 3 сегмента: fade-in 30% / solid 40% / fade-out 30%
```
show = hovered_handle == "file-right-handle" || dragging(FileRight).

## Метрики (из кода, точные)
- Hit-зона: ширина 8px (SPACE_2), центрирована на стыке (absolute left −4 от нулевого элемента)
- Полоса: ширина 3px, высота 100%; цвет tint(accent_primary, 0.25) — «tint-primary-strong»; концы растворяются 2-стоповыми градиентами 180° (0–30% fade-in, 30–70% solid, 70–100% fade-out)
- idle: полоса не рендерится вовсе
- Drag (FileRight): трейд file↔right: nf=init.0+d, nr=init.1−d, взаимный кламп PANEL_MIN_SIZE=100; персист filePanelWidthRatio + rightPanelWidthPx на mouse-up

## Отличия от original.md той же папки
1. Позиция hit-зоны: оригинал — absolute `left: -8px, width 8px` ЦЕЛИКОМ в левом зазоре; у нас −4..+4 — центр на стыке, 4px заходят на панель.
2. Направление трейда: оригинал «drag влево растит правую панель», торг right↔file; у нас торг file↔right той же ручкой (эквивалент), но fallback-ветки «file скрыт → рост против центра через clampGrowth(MAIN_MIN=100)» нет — при скрытой file-панели ручка всё равно двигает скрытую file-ширину (баг-расхождение; в body ручка привязана к when(rv), а не fv).
3. idle-состояние: оригинал держит полосу 2px bg_overlay с opacity 0 + transition 0.15s; у нас элемент отсутствует и появляется мгновенно (без анимации). Видимое hover/drag-состояние совпадает (3px, tint-primary-strong, растворение 30/70%).
4. `layoutActiveEditorNow()` не нужен — gpui-редактор релэйаутится в том же кадре.
5. z-index var(--z-resize-handle) не нужен: hit-зона живёт в зазоре gap_wrap, перекрытий нет.
6. role/aria-label «Resize right panel» — нет DOM.

## Дополнение атрибутов (цикл 10)

- отступы: собственных padding нет; геометрия — нулевой сиблинг `w(0)` с хит-зоной `absolute; left −4; top 0; w SPACE_2 8; h 100%` (`splitter.rs:63-79`), т.е. инсет −4 против −8 у оригинала: хит центрирован на стыке и 4px заходит на кромку панели.
- цвета: полоса `tint(accent_primary, 0.25)` (`splitter.rs:62`) = #89b4fa α .25 dark / #da8343 α .25 light (`palette.rs:83,121`) — эквивалент `--tint-primary-strong` (`variables.css:110,128`); idle-полосы нет вовсе (элемент не рендерится), у оригинала idle = `--bg-overlay` #515567/#d6d0c0 при opacity 0.
