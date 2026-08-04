# 82 settings-panel — наша реализация

Файлы: `crates/shell/src/ui/customize.rs:462-508` (ветка `"settings"`),
`customize.rs:357-431` (`pref_row`), `customize.rs:439-458` (`section`),
`customize.rs:246-355` (`legacy_bridge_card`, элемент 83).

## Структура (gpui-дерево)

```
div flex-col, gap 16 (`.root { gap: var(--space-4) }`)
├─ LegacyBridgeCard (если найден след старого Bridge) — см. 83
├─ section «NOTIFICATIONS» → pref_row #pref-toasts (backgroundToasts)
└─ section «TERMINAL»      → pref_row #pref-conpty (useConptyDll)

section: flex-col, gap 8
└─ sectionTitle — 11px, Semibold, text_muted, UPPERCASE
     (`letter-spacing: .06em` в gpui недоступен)

pref_row: flex, items_start, gap 10, py 4, cursor_pointer,
          БЕЗ фона / радиуса / ховера (`.row` оригинала их не имеет)
├─ чекбокс 16×16 — mt 2, rounded 4, border 1px text_muted;
│    checked → bg + border accent_primary, галка codicon `eab2` 12
│    цветом accent_action_fg
└─ flex-col, flex_1, min_w 0, gap 2
   ├─ label — FS_MD(13), text_primary
   └─ desc  — FS_XS(11), line-height 11×1.5, text_muted
```

Клик по строке шлёт `SetPref(key, !value)`; пока префы не загружены строка
`opacity 0.5` И клик игнорируется (аналог `disabled` на input оригинала).

## Что закрыто (циклы 10-14)

`sectionTitle` 11/600/uppercase/text_muted; строка без фона, радиуса и
ховера, gap 10 + `padding: 4px 0`; `line-height: 1.5` у описания и снятая
своя `max-width`; клик при незагруженных префах больше не проходит;
ритм между секциями даёт `gap: space-4` контейнера, а не margin заголовка
(из-за него у первой секции был лишний отступ 16, а между заголовком и
строкой — 4 вместо 8); LegacyBridgeCard реализован.

## Осталось

1. Чекбокс кастомный (оригинал — нативный `input[type=checkbox]`):
   фокус-кольцо и клавиатурный `Space` не воспроизведены.
2. `letter-spacing: .06em` у заголовка секции — ограничение gpui.

## Атрибуты (сверка ц.15)

- цвета: заголовок секции `--text-muted` #838aa0, label `--text-primary`
  #cfd4e2, описание `--text-muted` #838aa0; чекбокс — рамка
  `--text-muted` #838aa0, включённый `--accent-primary` #89b4fa с галкой
  цветом `--accent-action-fg` #313240.
- шрифты: `sectionTitle` 11/600 uppercase, label `--fs-md` 13,
  описание `--fs-xs` 11 при line-height 1.5.
