# 99 tree-icon-img — оригинал
Файлы: `kamin-ide/src/renderer/components/file-tree/TreeIcon.tsx:39-40` (весь компонент 22-41), `kamin-ide/src/renderer/components/file-tree/TreeIcon.module.css`; данные: `file-tree/file-icons.ts`, `vendor/fileIcons.ts`, `vendor/folderIcons.ts`, signals/icon-theme

## JSX-структура (кратко, вложенность)
```
img.img (+ optional className, конкатенация "img className") src={src} alt="" aria-hidden
```
Логика src:
- Синхронно: builtin Catppuccin — `type === "dir" ? folderIconUrl(name, expanded) : fileIconUrl(name)` (строки никогда не мигают пустыми).
- Асинхронный апгрейд: при активной contributed icon-theme (`activeIconDoc`) — `themeIconUrl(name, type, expanded, isRoot)`; если вернул url → setSrc(url), null → остаётся Catppuccin.
- useEffect ресетит на builtin при любой смене входов (name/type/expanded/isRoot/doc/themeKind); guard `live` против гонки.
- Пропсы: `expanded` (папки: open/closed глиф), `isRoot` (rootFolder* карты темы).

## Метрики (ИЗ CSS, точные значения)
`.img`:
- display: block
- размеры в модуле НЕ заданы — бокс задаёт вызывающая сторона (в дереве `.icon` из FileTreeView.module.css: width 16px, height 16px, flex-shrink 0)

Light-тема:
- `:global([data-theme="light"]) .img`: filter: saturate(3.2) brightness(0.7)
  (Catppuccin-пастель на светлых панелях выцветает — насыщение ×3.2, затемнение до 0.7)

## Состояния (классы-варианты с метриками)
- Вариантных классов нет. Два визуальных режима: dark (без фильтра) / light (filter выше). Контент src: builtin Catppuccin ↔ contributed-theme icon.

## Дополнение атрибутов (цикл 10)

- цвета: CSS цвета иконке не задаёт — `.img { display: block }` (`TreeIcon.module.css:5`), краски лежат внутри Catppuccin-SVG, и currentColor строки (`--text-secondary` #adb3c7 dark / #524c43 light, `FileTreeView.module.css:75`; `dark-theme.css:36`, `light-theme.css:46`) на неё НЕ действует — в отличие от codicon-chevron рядом. Единственная цветовая правка — светлая тема: `filter: saturate(3.2) brightness(0.7)` для `[data-theme="light"] .img` (`TreeIcon.module.css:6`), компенсация пастелей на панели `--bg-mantle` #fbf7f4 (`light-theme.css:25`).
- отступы: собственных padding/margin у `.img` нет; горизонтальный ритм даёт строка — `.row { gap: 6px; padding-right: 8px }` (`FileTreeView.module.css:62-68`), а бокс иконки фиксирован `.icon { width: 16px; height: 16px; flex-shrink: 0 }` (`:131-135`); отступ уровня — `indentPx(depth) = depth*12 + 8` (`file-tree-helpers.tsx`).
