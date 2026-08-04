# 85 extension-row — наша реализация

Файлы: `crates/shell/src/ui/extensions_panel.rs:70-181` (`ext_row`),
`extensions_panel.rs:72-97` (кнопка Enable/Disable),
`extensions_panel.rs:185-196` (`group_header`),
`extensions_panel.rs` (`ExtDesc::status`).

## Структура (gpui-дерево)

```
row — flex, items_center, gap 8, p 8, rounded 8, hover bg bg_surface 60 %;
      выключенное расширение → opacity .55
├─ иконка 26×26 — flex-center, text_muted, codicon-extensions `eae6` 16
│    (`.iconFallback`; реальные data-URL иконки хоста ещё не подставляются)
├─ meta — flex-col, flex_1, min_w 0
│  ├─ #extn-{id} — FS_SM(12), text_primary, ellipsis, nowrap,
│  │    тултип = полный `id`
│  └─ sub — FS_XS(11), text_muted: «{version} · {status}»
└─ actions — flex, gap 4, flex_shrink_0
   ├─ #extt-{id} — px 10, py 2, rounded 8, border 1px text_muted 30 %,
   │    bg bg_surface, FS_XS(11), text_primary, hover bg bg_overlay;
   │    подпись «Disable» / «Enable»
   └─ #extu-{id} (только не-builtin) — 24×22, rounded 8, text_muted,
        глиф trash `ea81` 16, тултип «Uninstall»,
        hover bg accent_red 16 % + accent_red

group_header — px 8, pt 8, pb 4, FS_XS(11), Semibold, text_muted,
               «TITLE — N» (uppercase)
```

## Что закрыто (циклы 10-14)

Иконка-фоллбэк 26×26, кнопка uninstall с красным ховером, текстовая кнопка
Enable/Disable вместо switch-пилюли, подпись «{version} · {status}» второй
строкой, `id` в тултипе имени, `opacity: .55` у выключенного, hover строки
`bg-surface 60 %`, ellipsis на имени.

Глиф корзины — 16: своего кегля у класса нет, действует вендорная база.

## Осталось

1. Реальные иконки расширений (`.icon` с `border-radius: var(--radius-xs)`
   = 4): хост ещё не отдаёт data-URL, рисуется только фоллбэк.
2. Состояние `downloading` у кнопки (спиннер и блокировка повторного клика).

## Атрибуты (сверка ц.15)

- цвета: имя `--text-primary` #cfd4e2, подпись `--text-muted` #838aa0,
  фон кнопки Enable/Disable `--bg-surface` #3d3f51 с рамкой
  `--text-muted` #838aa0 при 30 %, её ховер `--bg-overlay` #515567;
  ховер строки — `--bg-surface` #3d3f51 при 60 %; uninstall на ховере —
  `--accent-red` #f38ba8 (фон 16 %).
