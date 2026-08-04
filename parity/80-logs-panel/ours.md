# 80 logs-panel — наша реализация

Файлы: `crates/shell/src/ui/logs_panel.rs:87-287` (`logs_panel`), `:25-47`
(`filter_input`), `:49-81` (`tool_btn`), `:82-86` (`matches`),
`crates/shell/src/output_log.rs` (`OutputChannels` — буферы каналов).

## Структура (gpui-дерево)

```
пусто (channels.is_empty(), :92-131)
└─ div size_full, flex-col, center, gap 8, p 20, text_muted
   ├─ fa-inbox 32×32 opacity .6                          (`.empty i`)
   └─ div flex-wrap, baseline, max_w 420, кегль 16 (наследуемый):
      «No output channels yet. Extensions register them via »
      + JetBrains Mono FS_XS(11) «vscode.window.createOutputChannel(name)» + «.»

иначе: div flex, gap 12, size_full, min_h 0                    (:283-291)
├─ #log-channels — w 220, flex_shrink_0, min_h 0, overflow_y_scrollbar,
│  gap 2, pr 8                                                 (:135-145)
│  └─ строка #och-{key}: flex-col, gap 2, px 12, py 8, rounded 8,
│     border 1px transparent (резерв под активную рамку)
│     ├─ name — FS_SM(12), Medium, ellipsis;
│     │    активный → accent_primary, иначе text_secondary
│     └─ extension_id — FS_XS(11), text_muted, JetBrains Mono, ellipsis
│     активный → bg accent_primary 14 % + border accent_primary 35 %
│     иначе hover → bg bg_surface 50 % + text_primary
└─ right — flex_1, min_w 0, min_h 0                            (:200-282)
   ├─ toolbar (есть активный канал): flex, gap 8, flex_shrink_0, pb 8
   │  ├─ filter_input — flex_1, px 8, py 4, rounded 8, bg bg_base,
   │  │    border 1px bg_surface, Input appearance(false), Size FS_SM/0.875
   │  └─ 2 × tool_btn 26×26, rounded 8, text_secondary, глиф codicon 14:
   │       copy `ebcc` «Copy entire buffer», clear-all `eabf` «Clear channel»;
   │       enabled → hover bg bg_surface + text_primary; disabled → opacity .4
   ├─ #log-buffer — flex_1, min_h 0, overflow_y_scrollbar, p 12, rounded 8,
   │    bg bg_base, border 1px bg_surface, JetBrains Mono FS_XS(11),
   │    line-height 11×1.3, text_primary; строки фильтруются `matches`,
   │    рендерится ХВОСТ 400 строк; пусто → text_muted
   │    «Buffer is empty» / «No lines match the filter»
   └─ нет активного канала → центр, text_muted «Select a channel»
```

## Что закрыто (циклы 10-14)

Тулбар растянут (`search flex:1`), обе icon-only кнопки 26×26 с disabled
0.4, активный канал 14 % + рамка 35 % + accent-текст, `extension_id`
моноширинный, буфер `bg-base` + рамка `bg-surface` + padding 12, empty-state
с `fa-inbox` 32 и `<code>`-вставкой, зазор колонок 12.

`.item:hover` поднимает и ЦВЕТ ТЕКСТА до `--text-primary` — правка цикла 13
по ошибке ушла в системный лог (81), в цикле 14 переставлена по месту.

## Осталось

1. Буфер — построчные `div` вместо `<pre>`: нет `white-space: pre-wrap`,
   длинные строки не переносятся.
2. Нет stick-to-bottom: показывается хвост 400 строк без автопрокрутки к низу
   при доливке.
3. Фильтр без debounce 150 мс и не сбрасывается при смене канала.

## Атрибуты (сверка ц.15)

- скругления: строка канала, инпут фильтра, кнопки тулбара 26×26 и тело
  буфера — `--radius-sm` 8 (`radius-sm`); у пустого состояния скруглений нет
  ни там, ни там.
- цвета: имя канала `--text-secondary` #adb3c7 / активное
  `--accent-primary` #89b4fa, `extension_id` `--text-muted` #838aa0,
  буфер `--bg-base` #313240 с рамкой `--bg-surface` #3d3f51,
  активная строка — accent 14 % + рамка accent 35 %.
