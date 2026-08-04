# 84 extensions-panel — наша реализация
Файлы: `crates/shell/src/ui/extensions_panel.rs:197-260` (extensions_panel), `extensions_panel.rs:168-194` (group), `extensions_panel.rs:17-62` (ExtDesc: разбор kamin:extensions:list)

## Структура (gpui-дерево кратко)
```
exts == None → центр «Loading…» text_muted
иначе:
div#extensions-panel (flex-col, size_full, min_h 0, overflow_y_scrollbar)
├─ install-ряд (flex, justify_end, pb 6)
│  └─ #ext-install-vsix: codicon-add(ea60) 12 + «Install from VSIX…»
│     px 10, py 3, rounded 6, gap 6, bg accent_primary 16% (#89b4fa@0.16), 12px, text_primary;
│     hover bg accent 26% → ShellEvent::InstallVsixPrompt
├─ group «Installed» (sideloaded: !builtin)
└─ group «Built-in»

group: div flex-col, mb 16
├─ заголовок — mb 4, FS_SM(12), Semibold, text_secondary #adb3c7
├─ пусто → «None» FS_XS text_muted
└─ ext_row × N (см. 85)
```

## Метрики (из кода, точные)
- Install-кнопка: px 10 / py 3 / rounded 6 / gap 6 / 12px; bg #89b4fa@0.16 → hover @0.26.
- groupHeader: 12px Semibold #adb3c7, mb 4; группа mb 16.

## Отличия от original.md той же папки
1. Нет хедера панели «Extensions» (uppercase FS_XS) — титул даёт CustomizePanel-обёртка; Install-кнопка вынесена в отдельный правый ряд.
2. Кнопка: «Install from VSIX…» + codicon-add; у оригинала «Install» + codicon-cloud-download, есть `border 1px accent 40%`, radius-sm 8 (у нас 6, без бордера).
3. groupHeader без счётчика «— N», без uppercase/letter-spacing; 12px Semibold vs 11px/600 uppercase.
4. Empty: per-group «None» вместо единого «No extensions installed.»; добавлено состояние «Loading…» (в оригинале нет).
5. Нет сортировки по displayName и кэша иконок (иконок нет вообще, см. 85).
6. Паддинги списка свои (нет `0 8 8` у `.list`), скролл на всей панели.

## Дополнение атрибутов (цикл 10)

- шрифты: хедер «EXTENSIONS» fs-xs 11 (`extensions_panel.rs:236`, `metrics/lib.rs:42`); кнопка Install наследует те же 11 + глиф codicon 12 (`extensions_panel.rs:262`); заголовок группы fs-xs 11 + weight 600 SEMIBOLD (`extensions_panel.rs:191-192`); пустой список fs-sm 12 (`extensions_panel.rs:280`); статус загрузки fs-sm 12 (`extensions_panel.rs:214`). Оригинал: `.header`/`.installBtn`/`.groupHeader` — `var(--fs-xs)`, `.groupHeader { font-weight: 600 }`, `.empty { var(--fs-sm) }`, `.installBtn .codicon { 12px }` (`ExtensionsPanel.module.css:14,26,53-54,48,36`) — 1:1; uppercase у нас делается строкой (`to_uppercase()`), letter-spacing .04em в gpui недоступен.
