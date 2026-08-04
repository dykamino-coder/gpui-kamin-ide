# 66 file-panel-mode-tabs — наша реализация
Файлы: crates/shell/src/ui/file_panel_tabs.rs:25-103; вызов root.rs:4520-4531 (modeHeader)

## Структура (gpui-дерево кратко)
```
file_panel_mode_tabs(mode): div .flex .flex_shrink_0
├─ tab("fpm-files", codicon eaf0, "Files", left=true)
│    div .flex .items_center .gap(5) .h(24) .px(10)
│      .border_1 .rounded_l(12) .border_r_0
└─ tab("fpm-web", codicon eb01, "Web", left=false)
     … .rounded_r(12)
```
Клик → ShellEvent::SetFileMode("files"|"web") → layout.file_panel_mode (persist).

## Метрики (из кода, точные)
- Таб: h 24, px 10, gap 5, fs 12 (FS_SM), иконка codicon 14px
- border 1px --divider-soft = tint(text_primary, 0.06); bg bg_surface (#3d3f51 dark / #e6e1d4 light); текст text_secondary (#adb3c7 / #524c43)
- Склейка: левый rounded_l 12 (RADIUS_MD) + border_r_0; правый rounded_r 12 — шов без двойного бордера
- active: bg linear-gradient 90° tint(accent_primary,0.26) → tint(accent_primary,0.14); border tint(accent_primary,0.45); текст text_primary
- hover (неактивный): текст → text_primary (фон не меняется)

## Отличия от original.md той же папки
1. Метрики 1:1: h24/px10/gap5, divider-soft, bg-surface, radius-md по внешним краям, активный градиент 26→14% и бордер 45%, hover-цвет — всё совпадает.
2. `transition` в оригинале не объявлен — у нас его тоже нет. Совпадение.
3. role="tablist"/"tab", aria-selected — нет DOM.
4. Иконки: codicon files U+EAF0 / globe U+EB01 14px — оригинал те же классы codicon (размер иконки в css оригинала не переопределён, наследует fs — возможное расхождение 12 vs 14px, в original.md размер не зафиксирован).

## Дополнение атрибутов (цикл 10)

- шрифты: подпись fs-sm 12; глиф codicon 16 — `.tab` кегль иконки НЕ переопределяет, работает база `.codicon { font-size: 16px }` (`theme/skeleton.css:2-4`), поэтому у оригинала он тоже 16 (ревью ц.11 сняло претензию «должно быть 12»)
