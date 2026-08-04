# 81 system-log-panel — наша реализация

Файлы: `crates/shell/src/ui/logs_panel.rs:298-494` (`system_panel`),
`:82-86` (`matches`), `:497-507` (`capitalize`), `:510-526` (`rel_time`),
`crates/shell/src/output_log.rs` (`SysEntry`).

## Структура (gpui-дерево)

```
div flex-col, size_full, min_h 0
├─ toolbar — flex, gap 8, flex_shrink_0, pb 8                  (:307-390)
│  ├─ search — flex_1, h 28, px 10, rounded 8, bg bg_base,
│  │    border 1px text_primary 6 % (`--divider-soft`),
│  │    Input appearance(false), Size FS_SM/0.875
│  ├─ levels — flex, gap 2, flex_shrink_0;
│  │    4 × #syslog-lvl-{all|error|warning|info}: px 10, py 4, rounded 8,
│  │    border 1px transparent (резерв), FS_XS(11), text_muted,
│  │    подпись через `capitalize`;
│  │    активный → bg accent_primary 22 % + text_primary
│  │    иначе hover → bg text_primary 8 % + text_primary
│  └─ #syslog-clear — 28×28, rounded 8, text_muted, тултип «Clear logs»,
│       hover bg text_primary 10 % + text_primary, глиф clear-all `eabf` 16
├─ пусто → flex_1, центр, gap 8, p 16, text_muted              (:406-427)
│    fa-inbox 24 opacity .5 + «No diagnostics yet» /
│    «No entries match the filter»
└─ #syslog-body — flex_1, min_h 0, overflow_y_scrollbar,
     MONO, FS_XS(11)                                           (:430-486)
   └─ строка #syslog-row-{i} (newest-first: `shown.iter().rev()`):
      flex, items_baseline, gap 8, px 8, py 3,
      border-bottom 1px text_primary 3 %, hover bg text_primary 5 %
      ├─ глиф-бокс w 16, center: codicon 16 —
      │    error `ea87` accent_red / warning `ea6c` accent_yellow /
      │    info `ea74` accent_blue
      ├─ source — flex_shrink_0, nowrap, text_muted
      ├─ message — flex_1, min_w 0; error → accent_red, иначе text_primary
      └─ rel_time — flex_shrink_0, nowrap, text_muted (now / 5m / 3h / 2d)
```

Фильтр матчит по `"{level} {source} {message}"` без учёта регистра, поверх
фильтра по уровню.

## Что закрыто (циклы 10-14)

Сегментированный фильтр уровней, warning в `--accent-yellow`, колонка
относительного времени, `.error .message` красным, моноширинный список,
border-bottom + hover 5 %, `search` слева на `flex: 1` высотой 28,
icon-only Clear 28×28 с тултипом, `fa-inbox` 24 в пустом состоянии.

`.row:hover` меняет ТОЛЬКО фон — правка цикла 13, красившая текст,
принадлежала списку каналов Logs (80) и в цикле 14 возвращена туда.

Глиф уровня: `.icon { font-size: 13px }` = (0,1,0) проигрывает вендорной базе
`.codicon[class*='codicon-']` = (0,2,0) → фактический кегль 16, не 13.

## Осталось

1. Абсолютное время в тултипе строки (у нас только относительное в колонке).
2. Раскладка — flex вместо grid `16px max-content 1fr max-content`: колонка
   `source` не выровнена по общей ширине между строками.

## Атрибуты (сверка ц.15)

- скругления: поле поиска, пилюли уровней и кнопка Clear 28×28 —
  `--radius-sm` 8 (`radius-sm`); у строк списка скруглений нет.
- цвета: error `--accent-red` #f38ba8, warning `--accent-yellow` #f9e2af,
  info `--accent-blue` #89b4fa, источник и время `--text-muted` #838aa0,
  активная пилюля уровня — `--accent-primary` #89b4fa при 22 %.
