# 86 problems-panel — наша реализация

Файлы: `crates/shell/src/ui/problems.rs:156-471` (`problems_panel`),
`problems.rs:95-153` (`count_btn`), `problems.rs:78-93` (`base_name`,
`dir_name`), `problems.rs:19-46` (`Diag` из `kamin:diag:*`),
`crates/shell/src/root.rs` (подключение как тело тула `"problems"`).

## Структура (gpui-дерево)

```
div flex-col, size_full, min_h 0
├─ header — flex, justify_between, flex_shrink_0, pl 12 / pr 8 / py 8,
│    FS_XS(11), Medium + `ss01`, text_muted; + probe_area("problems-header")
│  ├─ «PROBLEMS»  (`letter-spacing: .08em` в gpui недоступен)
│  └─ counts — flex, gap 4: 2 × count_btn (error `ea87`, warning `ea6c`)
│     count_btn #prob-flt-{sev}: flex, gap 3, px 6, py 1, rounded 9,
│       border 1px transparent (резерв), FS_XS(11), text_muted,
│       глиф codicon 16 (цвет severity при count > 0, иначе text_muted)
│       активный → bg accent_primary 18 % + border 40 % + text_primary
│       count > 0 → cursor_pointer, тултип, hover bg bg_surface 70 %
│                   (ховер работает и у активной пилюли:
│                    `.countBtn:hover:not(:disabled)` (0,3,0) > `.countActive`)
│       count = 0 → opacity .8, кликов нет
└─ #problems-body — flex_1, min_h 0, overflow_y_scrollbar, pb 8, FS_SM(12)
   ├─ пусто → центр, p 20, FS_SM, text_muted, text_center
   │    «No problems have been detected in the workspace.»
   └─ на каждый uri (владельцы слиты по uri, файлов не больше `file_cap`):
      ├─ #prob-file-{uri} — flex, gap 6, w_full, h 24, px 8,
      │    text_secondary, nowrap, overflow_hidden, cursor_pointer,
      │    hover bg bg_surface 60 %; клик → сворачивание группы
      │    ├─ chevron — бокс w 16, center, text_muted, codicon 16
      │    │    (`.chevron{font-size:13px}` (0,1,0) проигрывает базе (0,2,0))
      │    ├─ TreeIcon 16×16
      │    ├─ имя файла — text_primary
      │    ├─ dir_name — flex_1, min_w 0, FS_XS(11), ellipsis (+тултип uri)
      │    └─ пилюля-счётчик — min_w 16, h 16, px 5, rounded 9,
      │         bg bg_surface, FS_XS(11)
      └─ строки диагностик (см. 87) при развёрнутой группе
   └─ «Show N more files» — flex, gap 6, px 10, py 6, FS_XS(11),
        глиф ellipsis `ea7c` 16, hover bg + text_primary
```

## Что закрыто (циклы 10-14)

Хедер со счётчиками-фильтрами и их состояниями, collapse по файлу,
`TreeIcon` + `dir_name` + пилюля-счётчик, капы файлов и строк с «Show more»,
`fileRow` h 24 + hover 60 %, полный текст пустого состояния.

Кегли глифов: chevron и счётчик — 16, а не 13/12 (правило CSS-модуля с
`font-size` на самом `.codicon` (0,1,0) проигрывает вендорной базе (0,2,0);
`.countBtn .codicon` вообще написан без `:global` и не матчится).

## Осталось

1. Порядок файлов — `sort` по uri; в оригинале сортировка по имени файла с
   учётом регистра иная (проверить на смешанном регистре).
2. `letter-spacing: .08em` у «PROBLEMS» — ограничение gpui.

## Атрибуты (сверка ц.15)

- цвета: «PROBLEMS» и dir-часть `--text-muted` #838aa0, имя файла
  `--text-primary` #cfd4e2, пилюля-счётчик `--bg-surface` #3d3f51,
  активная пилюля фильтра — `--accent-primary` #89b4fa 18 % + рамка 40 %,
  глифы severity `--accent-red` #f38ba8 / `--accent-yellow` #f9e2af.
- скругления: пилюли фильтра и счётчика — `radius: 9px`; строки списка без
  скругления; `--radius-sm` 8 у кнопок тулбара нет (их здесь не бывает).
