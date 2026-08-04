# 87 problem-row — наша реализация

Файлы: `crates/shell/src/ui/problems.rs:365-416` (строка диагностики внутри
`problems_panel`), `problems.rs:417-429` («… N more problems in this file»),
`problems.rs:46-60` (`Diag::origin` — `source(code)` / `source` / `code`).

## Структура (gpui-дерево)

```
#prob-{uri}-{i} — flex, items_center, gap 6, w_full, min_h 22,
                  pl 26 / pr 8, nowrap, overflow_hidden, text_secondary,
                  cursor_pointer, hover bg bg_surface 60 % + text_primary
├─ severity — codicon 16, flex_shrink_0:
│    0 error   `ea87` accent_red
│    1 warning `ea6c` accent_yellow
│    2 info    `ea74` accent_blue
│    3 hint    `ea76` text_muted
├─ #prob-msg-{uri}-{i} — flex_1, min_w 0, ellipsis, тултип = полный текст
├─ origin (если непуст) — flex_shrink_0, FS_XS(11), text_muted
└─ «[Ln {line+1}, Col {character+1}]» — flex_shrink_0, FS_XS(11), text_muted

клик → ShellEvent::OpenFileAt(uri, line + 1)

при row_total > ROW_CAP: строка pl 28, py 2, FS_XS(11), text_muted
  «… N more problems in this file»
```

## Что закрыто (циклы 10-14)

Warning в `--accent-yellow`, формат `[Ln x, Col y]`, `origin` со схемой
`source(code)`, левый индент 26 под иконку файла, `min-height: 22`, hover
`bg-surface 60 %` с осветлением текста до `--text-primary`, снятый радиус
строки, тултип на сообщении.

Глиф severity: `.sevIcon { font-size: 14px }` = (0,1,0) проигрывает вендорной
базе `.codicon[class*='codicon-']` = (0,2,0) → фактический кегль 16.

## Осталось

1. Hint-глиф `ea76` (`circle-outline`); в оригинале `codicon-lightbulb`
   — сверить, если hint-диагностики появятся у живых расширений.
